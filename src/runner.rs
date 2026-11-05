use crate::{grid::Grid, types::*, Result};
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

pub fn validate_path(
    map: &Grid,
    path: &[Point2],
    start: Point2,
    goal: Point2,
    radius: f64,
    goal_tolerance: f64,
) -> Result<()> {
    map.validate()?;
    if path.is_empty()
        || path.len() > 100_000
        || path.iter().any(|p| !p.is_finite())
        || path[0].distance(start) > map.resolution_m * 2.0
        || path.last().unwrap().distance(goal) > goal_tolerance
        || !map.clear(start, radius)
        || !map.segment_clear(start, path[0], radius)
        || path.iter().any(|p| !map.clear(*p, radius))
        || path
            .windows(2)
            .any(|p| !map.segment_clear(p[0], p[1], radius))
    {
        return Err(crate::error(
            "Invalid path: check endpoints, bounds, clearance, and segments between waypoints",
        ));
    }
    Ok(())
}
