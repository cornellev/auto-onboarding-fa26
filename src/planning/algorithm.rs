use crate::{
    shared::grid::Grid,
    shared::types::{Path, Point2, VehicleState},
};

#[derive(Default)]
pub struct Planner {}

impl Planner {
    /// A route from the car to `goal`, or `None` if there isn't one.
    /// This starter always returns `None`. See Planning in `docs/project.md`.
    pub fn plan(&mut self, _map: &Grid, _state: &VehicleState, _goal: Point2) -> Option<Path> {
        None
    }
}
