use crate::{shared::grid::Grid, shared::types::*, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Course {
    pub manifest_id: String,
    pub resolved_hash: String,
    pub goal: Point2,
    pub goal_tolerance_m: f64,
    pub timeout_s: f64,
    pub vehicle: VehicleSpec,
    pub grid: Grid,
    pub control_path: Path,
}

impl Course {
    /// The goal has to land on the grid, the tolerances and timeout have to be positive, and the practice path has to be real numbers.
    pub fn validate(&self) -> Result<()> {
        self.vehicle.validate()?;
        self.grid.validate()?;
        if !self.goal.is_finite()
            || self.grid.cell(self.goal).is_none()
            || !self.goal_tolerance_m.is_finite()
            || self.goal_tolerance_m <= 0.0
            || !self.timeout_s.is_finite()
            || self.timeout_s <= 0.0
            || self.control_path.is_empty()
            || self.control_path.iter().any(|p| !p.is_finite())
        {
            return Err(crate::error(
                "Invalid course goal, bounds, timeout, or controls fixture",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Observe,
    Controls,
    Autonomous,
    DriveTest,
}

#[derive(Debug)]
pub struct Update {
    pub command: DriveCommand,
    pub status: &'static str,
    pub path: Path,
    pub finished: bool,
    pub saturated: bool,
}
