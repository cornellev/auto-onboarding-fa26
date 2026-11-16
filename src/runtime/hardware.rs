//! What the Pi has to implement before this can run on a real car. The laptop starter doesn't include it.
use crate::{
    shared::types::{DriveCommand, Observation, VehicleSpec},
    Result,
};
use std::time::Duration;

pub trait VehicleIo {
    /// Wait up to `timeout` for a pose and a scan from the same moment.
    fn observe(&mut self, timeout: Duration) -> Result<Option<Observation>>;
    /// Send a speed and a steering angle. The car itself has to stop if these stop arriving.
    fn drive(&mut self, command: DriveCommand) -> Result<()>;
    /// Ask the car to stop.
    fn stop(&mut self) -> Result<()>;
}

/// The spec has to be valid, and `hardware_calibrated` has to be true. That flag means somebody measured the car.
pub fn validate_hardware_spec(spec: &VehicleSpec) -> Result<()> {
    spec.validate()?;
    if !spec.hardware_calibrated {
        return Err(crate::error(
            "Hardware requires measured geometry, sensor extrinsics, and actuator calibration",
        ));
    }
    Ok(())
}
