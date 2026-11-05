//! Provided whole-path clearance and endpoint checks.
use crate::{shared::grid::Grid, shared::types::Point2, Result};

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
