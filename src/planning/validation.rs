//! Checks a whole path: real numbers, a start near the car, an end near the goal, and room along the way.
use crate::{shared::grid::Grid, shared::types::Point2, Result};

/// The first point has to sit within two cell widths of the car, and the last within the goal tolerance.
/// Every point, and every segment between points, has to clear the car's radius.
pub fn validate_path(
    map: &Grid,
    path: &[Point2],
    start: Point2,
    goal: Point2,
    radius: f64,
    goal_tolerance: f64,
) -> Result<()> {
    map.validate()?;
    if path.is_empty() {
        return Err(crate::error("Invalid path: empty"));
    }
    if path.len() > 100_000 {
        return Err(crate::error("Invalid path: too many waypoints"));
    }
    if path.iter().any(|point| !point.is_finite()) {
        return Err(crate::error(
            "Invalid path: a waypoint is not a real number",
        ));
    }
    if path[0].distance(start) > map.resolution_m * 2.0 {
        return Err(crate::error(
            "Invalid path: the first point is more than two cell widths from the car",
        ));
    }
    if path.last().unwrap().distance(goal) > goal_tolerance {
        return Err(crate::error(
            "Invalid path: the last point is farther from the goal than the tolerance",
        ));
    }
    if !map.clear(start, radius) {
        return Err(crate::error("Invalid path: the car's circle is not clear"));
    }
    if !map.segment_clear(start, path[0], radius) {
        return Err(crate::error(
            "Invalid path: the segment from the car to the first waypoint is blocked",
        ));
    }
    if path.iter().any(|point| !map.clear(*point, radius)) {
        return Err(crate::error("Invalid path: a waypoint circle is not clear"));
    }
    if path
        .windows(2)
        .any(|points| !map.segment_clear(points[0], points[1], radius))
    {
        return Err(crate::error(
            "Invalid path: a segment between waypoints is blocked",
        ));
    }
    Ok(())
}
