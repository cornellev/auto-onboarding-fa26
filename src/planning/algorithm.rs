use crate::{
    shared::grid::Grid,
    shared::types::{Path, Point2, VehicleState},
};

#[derive(Default)]
pub struct Planner {}

impl Planner {
    /// Return a route from the current pose to the goal, or None if unavailable.
    pub fn plan(&mut self, _map: &Grid, _state: &VehicleState, _goal: Point2) -> Option<Path> {
        // Student task: see docs/challenges.md#planning.
        None
    }
}
