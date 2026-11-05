//! Wire conversion and freshness checks. Algorithms keep ordinary Rust structs.
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
pub const FRESH_NS: u64 = 300_000_000;

pub fn fingerprint() -> String {
    format!("{:x}", Sha256::digest(DESCRIPTOR))
}

/// Physical struct fields deliberately match the schema. Presence failures are errors.
pub fn convert<T: Serialize, U: DeserializeOwned>(value: &T) -> Result<U> {
    Ok(serde_json::from_value(serde_json::to_value(value)?)?)
}

pub fn header(run: &str, sequence: u64, time_ns: u64, frame: &str) -> pb::Header {
    pb::Header {
        run_id: run.into(),
        sequence,
        time_ns,
        frame_id: frame.into(),
    }
}

pub fn check_header(h: &Option<pb::Header>, run: &str, frame: &str) -> Result<pb::Header> {
    let h = h.as_ref().ok_or_else(|| error("Missing message header"))?;
    if h.run_id != run || h.frame_id != frame || h.sequence == 0 {
        return Err(error("Wrong run, frame, or zero sequence"));
    }
    Ok(h.clone())
}

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

pub fn decode<M: Message + Default>(bytes: &[u8]) -> Result<M> {
    if bytes.len() > 8 * 1024 * 1024 {
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
impl Freshness {
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
    pub fn time_ns(&self) -> u64 {
        self.time
    }
    pub fn fresh(&self, now_ns: u64) -> bool {
        self.received
            .is_some_and(|t| t.elapsed().as_millis() <= 300)
            && now_ns
                .checked_sub(self.time)
                .is_some_and(|age| age <= FRESH_NS)
    }
}
