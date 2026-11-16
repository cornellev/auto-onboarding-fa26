use crate::{
    shared::grid::Grid,
    shared::types::{Path, Point2, VehicleState},
};

#[derive(Default)]
pub struct Planner {}

impl Planner {
    /// A route from the car to `goal`, or `None` if there is no way through.
    /// `clearance_m` is the circle from `VehicleSpec::clearance_m`.
    /// `goal_tolerance_m` is how close the last point has to be to the goal.
    /// A waypoint has to pass `Grid::clear(point, clearance_m)`. A step has to pass
    /// `Grid::segment_clear` at that same radius. `blocked` is only the cell cutoff.
    /// `validate_path` returns an error, not "no path", when clearance fails.
    /// This starter always returns `None`. See Planning in `docs/project.md`.
    pub fn plan(
        &mut self,
        _map: &Grid,
        _state: &VehicleState,
        _goal: Point2,
        _clearance_m: f64,
        _goal_tolerance_m: f64,
    ) -> Option<Path> {
        // Edit me...
        None
        // ...till here.
    }
}
