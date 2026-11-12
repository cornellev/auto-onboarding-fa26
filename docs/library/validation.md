# `validation` — Check a path before it is published

**Source code:** [`src/planning/validation.rs`](../../src/planning/validation.rs)

This module checks a whole path: real numbers, a start near the car, an end near the goal, and room for the car along the way. `planned_update` calls `validate_path` after `Planner::plan` returns `Some`. A path the checker rejects is an error. The node does not publish it. `None` from the planner never reaches this function. It becomes "no path" one level up.

The grid stays as perception wrote it. The radius is the car's clearance, not a second copy of the obstacles.

## The check

`validate_path(map, path, start, goal, radius, goal_tolerance)`

Check `path` against `map`. `start` is the car's position, the rear axle, in world meters. `goal` is the course goal. `radius` is the circle from `VehicleSpec::clearance_m`. `goal_tolerance` is how close the last point has to land, in meters. The course file sets that tolerance. The starter courses use 0.2 m.

`map.validate()` runs first. A bad grid returns that error and the path is not inspected.

The path checks run in order. The first failure is the error you get.

1. An empty path returns `"Invalid path: empty"`.
2. More than 100,000 waypoints returns `"Invalid path: too many waypoints"`.
3. A non-finite coordinate returns `"Invalid path: a waypoint is not a real number"`.
4. A first point farther than `2 * map.resolution_m` from `start` returns `"Invalid path: the first point is more than two cell widths from the car"`. The distance is `Point2::distance`.
5. A last point farther than `goal_tolerance` from `goal` returns `"Invalid path: the last point is farther from the goal than the tolerance"`.
6. A car whose circle is not `map.clear(start, radius)` returns `"Invalid path: the car's circle is not clear"`.
7. A blocked step from `start` to the first waypoint, tested with `map.segment_clear`, returns `"Invalid path: the segment from the car to the first waypoint is blocked"`.
8. A waypoint whose own circle is not clear returns `"Invalid path: a waypoint circle is not clear"`.
9. A blocked segment between two consecutive waypoints returns `"Invalid path: a segment between waypoints is blocked"`.

A path of one point skips the last step, because there is no pair of waypoints. It still has to clear the point itself and the segment from the car to that point.

`Grid::clear` and `Grid::segment_clear` treat a weight of 128 or more as blocked, and they treat a circle that leaves the map as blocked. The search has to call those two functions with the same radius. A path whose cells are only not `blocked` fails here. A route that fits a point and does not fit the car fails here. A route that jumps a wall between waypoints fails on the segment, which is the usual shape of a path that only tested the cells under the points.

`Ok(())` means every check passed. The function does not modify the path or the grid.

## See also

- [`grid`](grid.md) — `clear`, `segment_clear`, and `center`
- [`types`](types.md) — `VehicleSpec::clearance_m`
- [`planning-node`](planning-node.md) — the caller
