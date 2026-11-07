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
pub const FRESH_MS: u128 = 300;
pub const MAX_DECODE_SIZE: usize = 8 * 1024 * 1024; // 8 MB

/**
 * Returns a fingerprint of the protobuf descriptor for the autonomy package.
 * @return A hexadecimal string representing the SHA-256 hash of the descriptor.
 */
pub fn fingerprint() -> String {
    format!("{:x}", Sha256::digest(DESCRIPTOR))
}

/**
 * Converts a serializable value to another type by serializing it to JSON and then deserializing it.
 * @param value The value to convert.
 * @return The converted value of type U, or an error if the conversion fails.
 */
pub fn convert<T: Serialize, U: DeserializeOwned>(value: &T) -> Result<U> {
    Ok(serde_json::from_value(serde_json::to_value(value)?)?)
}

/**
 * Creates a new protobuf header with the given run ID, sequence number, timestamp, and frame ID.
 * @param run The run ID.
 * @param sequence The sequence number of the message.
 * @param time_ns The timestamp of the message in nanoseconds.
 * @param frame The frame ID of the message.
 * @return A new protobuf header.
 */
pub fn header(run: &str, sequence: u64, time_ns: u64, frame: &str) -> pb::Header {
    pb::Header {
        run_id: run.into(),
        sequence,
        time_ns,
        frame_id: frame.into(),
    }
}

/**
 * Checks the header of a protobuf message against the expected run ID and frame ID, and ensures that the sequence number is non-zero.
 * @param h The optional protobuf header to check.
 * @param run The expected run ID.
 * @param frame The expected frame ID.
 * @return The validated protobuf header, or an error if the header is missing, has the wrong run ID or frame ID, or has a zero sequence number.
 */
pub fn check_header(h: &Option<pb::Header>, run: &str, frame: &str) -> Result<pb::Header> {
    let h = h.as_ref().ok_or_else(|| error("Missing message header"))?;
    if h.run_id != run || h.frame_id != frame || h.sequence == 0 {
        return Err(error("Wrong run, frame, or zero sequence"));
    }
    Ok(h.clone())
}

/**
 * Validates and extracts a vehicle-state update from a protobuf message, ensuring that the pose and speed are finite.
 * @param update The protobuf state update message.
 * @param run The expected run ID.
 * @return A tuple containing the validated protobuf header and the vehicle state, or an error if the header is invalid, the vehicle state is missing, or the pose or speed are not finite.
 */
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

/**
 * Validates and extracts a LIDAR scan from a protobuf message, ensuring that the scan is valid.
 * @param update The protobuf LIDAR update message.
 * @param run The expected run ID.
 * @return A tuple containing the validated protobuf header and the LIDAR scan, or an error if the header is invalid, the scan is missing, or the scan is not valid.
 */
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

/**
 * Decodes a protobuf message from a byte slice, ensuring that the message does not exceed 8 MB in size.
 * @param bytes The byte slice containing the encoded protobuf message.
 * @return The decoded protobuf message of type M, or an error if the message exceeds the size limit or cannot be decoded.
 */
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
/**
 * Freshness tracks the freshness of incoming messages based on their sequence numbers and timestamps.
 * It provides methods to accept new messages, check their timestamps, and determine if the data is fresh.
 */
impl Freshness {
    /**
     * Accepts a new protobuf header, updating the freshness state if the header is valid and in order.
     * @param h The protobuf header to accept.
     * @return Ok(()) if the header is accepted, or an error if the header is repeated, out-of-order, or indicates a reset.
     */
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
    /**
     * Returns the timestamp of the last accepted message in nanoseconds.
     * @return The timestamp of the last accepted message in nanoseconds.
     */
    pub fn time_ns(&self) -> u64 {
        self.time
    }
    /**
     * Returns true if the last accepted message is considered fresh based on its timestamp and the current time.
     * @param now_ns The current time in nanoseconds.
     * @return True if the last accepted message is fresh, false otherwise.
     */
    pub fn fresh(&self, now_ns: u64) -> bool {
        self.received
            .is_some_and(|t| t.elapsed().as_millis() <= FRESH_MS)
            && now_ns
                .checked_sub(self.time)
                .is_some_and(|age| age <= FRESH_NS)
    }
}
