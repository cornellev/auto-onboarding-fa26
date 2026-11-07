//! Maintainer-owned boundary. Implement this on the Pi; no desktop GPIO dependencies.
use crate::{
    shared::types::{DriveCommand, Observation, VehicleSpec},
    Result,
};
use std::time::Duration;

pub trait VehicleIo {
    /**
     * Waits for a fresh observation with capture-aligned vehicle state and LiDAR data.
     * @param timeout The maximum time to wait for an observation.
     * @return Some(observation) when available, None on timeout, or an I/O or data error.
     */
    fn observe(&mut self, timeout: Duration) -> Result<Option<Observation>>;
    /**
     * Forwards speed and steering setpoints to the vehicle.
     * The device must stop independently when commands are lost.
     * @param command The requested speed in m/s and steering angle in radians.
     * @return Ok(()) if the command is accepted, or a validation or I/O error.
     */
    fn drive(&mut self, command: DriveCommand) -> Result<()>;
    /**
     * Requests that the vehicle stop moving.
     * @return Ok(()) if the stop request succeeds, or an I/O error.
     */
    fn stop(&mut self) -> Result<()>;
}

/**
 * Validates a vehicle specification and requires measured hardware calibration.
 * @param spec The geometry, sensor transforms, and actuator limits to check.
 * @return Ok(()) if the specification is valid and calibrated, or an error otherwise.
 */
pub fn validate_hardware_spec(spec: &VehicleSpec) -> Result<()> {
    spec.validate()?;
    if !spec.hardware_calibrated {
        return Err(crate::error(
            "Hardware requires measured geometry, sensor extrinsics, and actuator calibration",
        ));
    }
    Ok(())
}
