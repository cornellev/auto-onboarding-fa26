use crate::shared::types::{DriveCommand, Point2, VehicleSpec, VehicleState};

pub struct Controller {
    pub vehicle: VehicleSpec,
}

impl Controller {
    /// `vehicle` has the wheelbase and the speed and steering limits.
    pub fn new(vehicle: VehicleSpec) -> Self {
        Self { vehicle }
    }

    /// Speed and steering to follow `path`. Positive steering is a left turn.
    /// `dt_s` is the time since the last call, in seconds. This starter always stops.
    /// See Controls in `docs/project.md`.
    pub fn command(&mut self, _state: &VehicleState, _path: &[Point2], _dt_s: f64) -> DriveCommand {
        // Edit me...
        DriveCommand::STOP
        // ...till here.
    }
}
