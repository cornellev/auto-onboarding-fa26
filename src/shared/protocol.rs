//! Turn Protobuf messages into ordinary Rust values, and check that they aren't stale.
//! Your algorithms should stay on the plain structs in `types`.
use crate::{error, shared::types::*, Result};
use prost::Message;
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};
use std::time::Instant;

#[allow(clippy::all)]
pub mod pb {
    include!("generated/cev.onboarding.v1.rs");
}
pub const DESCRIPTOR: &[u8] = include_bytes!("../../proto/autonomy.binpb");
/// A map, path, or command older than this is too old to drive on. 300 ms.
pub const FRESH_NS: u64 = 300_000_000;
pub const FRESH_MS: u128 = 300;
pub const MAX_DECODE_SIZE: usize = 8 * 1024 * 1024; // 8 MB

/// A hash of the message definitions, so a node can tell it's talking to a matching run.
pub fn fingerprint() -> String {
    format!("{:x}", Sha256::digest(DESCRIPTOR))
}

/// Copy a value into another type with the same JSON shape.
/// That's how the plain structs and the Protobuf structs trade places.
pub fn convert<T: Serialize, U: DeserializeOwned>(value: &T) -> Result<U> {
    Ok(serde_json::from_value(serde_json::to_value(value)?)?)
}

/// The little header every message carries: which run, which number in the sequence, when, and which frame.
pub fn header(run: &str, sequence: u64, time_ns: u64, frame: &str) -> pb::Header {
    pb::Header {
        run_id: run.into(),
        sequence,
        time_ns,
        frame_id: frame.into(),
    }
}

/// The header has to be present, for this run, in the expected frame, and numbered past zero.
pub fn check_header(h: &Option<pb::Header>, run: &str, frame: &str) -> Result<pb::Header> {
    let h = h.as_ref().ok_or_else(|| error("Missing message header"))?;
    if h.run_id != run || h.frame_id != frame || h.sequence == 0 {
        return Err(error("Wrong run, frame, or zero sequence"));
    }
    Ok(h.clone())
}

/// Pull a vehicle pose and speed out of a state message, after the header checks out.
pub fn state(update: &pb::StateUpdate, run: &str) -> Result<(pb::Header, VehicleState)> {
    let h = check_header(&update.header, run, "map")?;
    let state: VehicleState = convert(
        update
            .state
            .as_ref()
            .ok_or_else(|| error("Missing vehicle state"))?,
    )?;
    if !state.pose.is_finite() || !state.speed_mps.is_finite() {
        return Err(error("Invalid vehicle state"));
    }
    Ok((h, state))
}

/// Pull a LiDAR scan out of a scan message, after the header checks out.
pub fn scan(update: &pb::LidarUpdate, run: &str) -> Result<(pb::Header, LidarScan)> {
    let h = check_header(&update.header, run, "front_lidar_frame")?;
    let scan: LidarScan = convert(update.scan.as_ref().ok_or_else(|| error("Missing scan"))?)?;
    Observation {
        time_ns: h.time_ns,
        state: VehicleState::default(),
        scan: scan.clone(),
    }
    .validate()?;
    Ok((h, scan))
}

/// Decode one Protobuf message. Anything over 8 MB is refused.
pub fn decode<M: Message + Default>(bytes: &[u8]) -> Result<M> {
    if bytes.len() > MAX_DECODE_SIZE {
        return Err(error("Protobuf message exceeds limit"));
    }
    Ok(M::decode(bytes)?)
}

#[derive(Default)]
pub struct Freshness {
    sequence: u64,
    time: u64,
    received: Option<Instant>,
}
/// Remembers the last sequence number and time we accepted, so a repeat or a rewind gets rejected.
impl Freshness {
    /// Take this header if it's newer than the last one. A repeat, a step backward, or a clock going backward means start a new run.
    pub fn accept(&mut self, h: &pb::Header) -> Result<()> {
        if h.sequence <= self.sequence || (self.received.is_some() && h.time_ns < self.time) {
            return Err(error(
                "Repeated/out-of-order update or reset; start a fresh run",
            ));
        }
        self.sequence = h.sequence;
        self.time = h.time_ns;
        self.received = Some(Instant::now());
        Ok(())
    }
    /// When the last accepted message was captured, in nanoseconds since the run started.
    pub fn time_ns(&self) -> u64 {
        self.time
    }
    /// The last message arrived recently, and its capture time isn't more than 300 ms behind `now_ns`.
    pub fn fresh(&self, now_ns: u64) -> bool {
        self.received
            .is_some_and(|t| t.elapsed().as_millis() <= FRESH_MS)
            && now_ns
                .checked_sub(self.time)
                .is_some_and(|age| age <= FRESH_NS)
    }
}
