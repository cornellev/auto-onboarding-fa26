//! Maintainer-owned boundary. Implement this on the Pi; no desktop GPIO dependencies.
use crate::{
    types::{DriveCommand, Observation, VehicleSpec},
    Result,
};
use std::time::Duration;

pub trait VehicleIo {
    /// Wait at most `timeout`; return a fresh, capture-aligned observation.
    fn observe(&mut self, timeout: Duration) -> Result<Option<Observation>>;
    /// Forward SI setpoints. The device must stop independently on lost commands.
    fn drive(&mut self, command: DriveCommand) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
}

pub fn validate_hardware_spec(spec: &VehicleSpec) -> Result<()> {
    spec.validate()?;
    if !spec.hardware_calibrated {
        return Err(crate::error(
            "Hardware requires measured geometry, sensor extrinsics, and actuator calibration",
        ));
    }
    Ok(())
}
