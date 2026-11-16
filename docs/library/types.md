# `types` — Points, poses, scans, and the car

**Source code:** [`src/shared/types.rs`](../../src/shared/types.rs)

This module is the set of plain values the algorithms see. Distances are meters. Angles are radians. A speed is meters per second. Time, when a value carries one, is nanoseconds since the run started, and it lives on `Observation` rather than on the pose.

A vehicle pose is the middle of the rear axle. Heading is counterclockwise from +x. In this frame, +x is forward and +y is left. A LiDAR point starts in the sensor frame, with the same axis convention. `Pose2::transform` moves a point up one frame: rotate by the pose's heading, then add the pose's position. Sensor into the car is one call. Car into the world is a second call.

The Protobuf messages in [`messages`](messages.md) repeat these names. Those are the bytes on the router. `protocol::convert` copies between the two. An algorithm should keep the values from this module.

## Points

*class* `shared::types::Point2`

A point in a plane. `x` and `y` are public `f64` fields, in meters. `Default` is the origin. The struct derives `Copy`, so passing a point does not borrow it.

`Point2::distance(self, other)`

Return the straight-line distance between `self` and `other`, in meters. Both points need to be in the same frame. The result is `(self.x - other.x).hypot(self.y - other.y)`. A non-finite coordinate produces a non-finite distance.

`Point2::is_finite(self)`

Return `true` when `x` and `y` are both finite. `NaN` and infinities return `false`.

## Poses

*class* `shared::types::Pose2`

A pose in the plane. `position` is a `Point2`. `yaw_rad` is the heading in radians, counterclockwise from +x. `Default` is the origin, heading zero, which faces +x.

`Pose2::transform(self, point)`

Rotate `point` by `yaw_rad`, then add `position`. One call moves a point up a single frame.

With `yaw_rad` equal to θ, and `point` equal to `(x, y)`, the result is:

```text
x' = position.x + cos(θ) * x - sin(θ) * y
y' = position.y + sin(θ) * x + cos(θ) * y
```

A heading of π/2 faces +y. A point one meter ahead of that pose, `(1, 0)` in the pose's own frame, lands one meter in +y from `position`.

The scan points on an `Observation` are in the sensor frame. The sensor mount and the vehicle pose are both `Pose2` values, so the world point is two calls:

```rust
let in_vehicle = observation.scan.sensor_in_vehicle.transform(point);
let in_world = observation.state.pose.transform(in_vehicle);
```

The first call uses the mount, whose translation is where the LiDAR is bolted relative to the rear axle. The second uses the car's pose in the world. The order is sensor, then car, then world.

`Pose2::inverse_transform(self, point)`

Undo `transform`. Subtract `position`, then rotate by the opposite heading. A world point becomes a point in this pose's frame. +x is still forward and +y is still left.

With the same θ, and a world point `(x, y)`:

```text
dx = x - position.x
dy = y - position.y
x' = cos(θ) * dx + sin(θ) * dy
y' = -sin(θ) * dx + cos(θ) * dy
```

`transform(inverse_transform(point))` and `inverse_transform(transform(point))` both return the point they started from, for a finite pose and a finite point. Controls uses this to bring a lookahead point from the world into the car. Positive `y` in that frame is a left turn.

`Pose2::is_finite(self)`

Return `true` when `position` is finite and `yaw_rad` is finite.

`wrap_angle(radians)`

Fold an angle into the range −π..π by computing `radians.sin().atan2(radians.cos())`. A `NaN` or an infinity stays `NaN`, because the sine and cosine of those values are `NaN`. Multiples of a full turn land on zero. A value just past π lands just above −π.

## The vehicle at one moment

*class* `shared::types::VehicleState`

The car at one capture. `pose` is a `Pose2` at the rear axle. `speed_mps` is the forward speed in meters per second. `Default` is a stopped car at the origin, facing +x.

*class* `shared::types::LidarScan`

One scan. `points` is a `Vec<Point2>` of endpoints in the sensor frame, with +x forward and +y left. `sensor_in_vehicle` is the `Pose2` of that sensor relative to the rear axle. The simulator's axis convention is converted before these points are published, so +y stays left.

*class* `shared::types::Observation`

One paired sample. `time_ns` is nanoseconds since the run started, at the moment the scan was taken. `state` is the `VehicleState` from that same capture time. `scan` is the `LidarScan`.

`Observation::validate(&self)`

Return `Ok(())` when the pose, the speed, the sensor mount, and every scan point are finite, and when the scan holds at most 100,000 points.

Return an error, `"Invalid pose, speed, or LiDAR points"`, when any of those numbers is non-finite or the scan is longer than that. An empty scan is valid. `time_ns` is not inspected.

## Paths and commands

*type* `Path`

`Vec<Point2>`. A path is an ordered list of world points. The planning helpers treat the first point as the start of the route and the last as the end. An empty vector is an empty path.

*class* `shared::types::DriveCommand`

A command to the wheels. `speed_mps` is forward speed in meters per second. `steering_rad` is the steering angle in radians. Positive steering is a left turn. Positive speed is forward. `Default` is a stop.

`DriveCommand::STOP`

The constant command with `speed_mps == 0.0` and `steering_rad == 0.0`. The controls helper publishes this when it does not have a path it can follow.

## The car's measurements

*class* `shared::types::VehicleSpec`

The measurements and limits for one car. Every field is public.

`wheelbase_m`

Distance between the axles, in meters. Pure pursuit reads the steering angle from this and the curvature.

`length_m`, `width_m`

Body length and width, in meters, used by `clearance_m`.

`rear_axle_in_base`

Where the rear axle sits in the simulator's `base_link` frame, as a `Point2`. The pose the nodes receive is already the rear axle. This offset is how a `base_link` position becomes that axle, and it also enters `clearance_m`.

`sensor_in_vehicle`

Where the LiDAR is mounted, as a `Pose2` relative to the rear axle. A live scan carries its own copy of this mount on `LidarScan`.

`max_speed_mps`, `max_steering_rad`

The caps `limit` applies. Speed is a forward cap. Steering is a symmetric cap, in radians.

`hardware_calibrated`

`false` until someone measures a physical car. The starter numbers describe the simulator minicar. [`validate_hardware_spec`](hardware.md) requires this flag. `validate` does not.

`VehicleSpec::validate(&self)`

Return `Ok(())` when the wheelbase, length, width, max speed, and max steering are finite and strictly positive, the max speed is at most 1 m/s, the max steering is strictly below π/2, and the rear-axle offset and the sensor mount are finite.

Return an error, `"Invalid vehicle specification or speed cap above 1 m/s"`, otherwise. A speed cap of exactly 1 m/s is accepted. A steering cap of exactly π/2 is rejected. `hardware_calibrated` is not part of this check.

`VehicleSpec::clearance_m(&self)`

Return the radius, in meters, of a circle around the rear axle that covers the car whichever way it points:

```text
hypot(length_m / 2 + abs(rear_axle_in_base.x),
      width_m / 2 + abs(rear_axle_in_base.y))
```

Planning passes this radius to `Grid::clear` and `Grid::segment_clear`. On the starter car the value is about 0.263 m. The function does not look at the heading, so the circle is the same at every yaw.

`VehicleSpec::require_finite(&self, command)`

Return `Ok(())` when `speed_mps` and `steering_rad` are both finite. Return `"Non-finite command rejected; stopping"` when either is `NaN` or infinite. This does not look at the caps and does not change the command. `controlled_update` calls it, then publishes the numbers `Controller::command` returned.

`VehicleSpec::limit(&self, command)`

Clamp a finite command to the car. Speed is clamped to `0.0..=max_speed_mps`, so a negative speed becomes zero. Steering is clamped to `-max_steering_rad..=max_steering_rad`.

Return the same non-finite error as `require_finite` when `speed_mps` or `steering_rad` is `NaN` or infinite. The runner and `Sim::drive` publish this clamped result. The controls node does not. The runner records a saturated update when the clamp would have changed the command the node published.

## See also

- [`grid`](grid.md) — turn a world point into a cell
- [`protocol`](protocol.md) — `convert`, `state`, and `scan`
- [`messages`](messages.md) — the Protobuf copies of these structs
- [`validation`](validation.md) — where `clearance_m` is enforced on a path
