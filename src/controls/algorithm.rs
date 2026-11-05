use crate::shared::types::{DriveCommand, Point2, VehicleSpec, VehicleState};

pub struct Controller {
    pub vehicle: VehicleSpec,
}

impl Controller {
    pub fn new(vehicle: VehicleSpec) -> Self {
        Self { vehicle }
    }

    /// Select a target and command forward speed and positive-left steering.
    pub fn command(&mut self, _state: &VehicleState, _path: &[Point2], _dt_s: f64) -> DriveCommand {
        // Student task: see docs/challenges.md#controls.
        DriveCommand::STOP
    }
}
