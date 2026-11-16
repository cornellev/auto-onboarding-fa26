# `protocol::pb` — Protobuf messages

**Source code:** [`proto/autonomy.proto`](../../proto/autonomy.proto), generated into [`src/shared/generated/cev.onboarding.v1.rs`](../../src/shared/generated/cev.onboarding.v1.rs)

This module is the wire format, `cev.onboarding.v1`, included as `shared::protocol::pb`. Every message on the router is one of these types, encoded with `prost::Message::encode_to_vec`. [`protocol::decode`](protocol.md) reads the bytes back. [`protocol::convert`](protocol.md) copies a message into the matching plain struct, and the other way around.

The plain structs and these messages share field names on purpose. They are not the same types. A `pb::Point2` is the Protobuf value. A `shared::types::Point2` is what `Perception::update`, `Planner::plan`, and `Controller::command` should hold. Nested messages are `Option` here, because a Protobuf message field can be absent. Scalar fields are present, and a missing scalar decodes as zero. `availability` is stored as an `i32`. Compare it with `Availability::Ready as i32`.

Units match the plain structs. Distances are meters, angles are radians, speeds are meters per second, and `time_ns` is nanoseconds since the run started. Heading is counterclockwise from +x. A pose is the middle of the rear axle.

## Values

These mirror [`types`](types.md) and [`grid`](grid.md). Prefer the plain structs in an algorithm. The differences that matter at the boundary are listed with each type.

*class* `pb::Point2`

`x` and `y`, as `f64`.

*class* `pb::Pose2`

`position` is `Option<Point2>`. `yaw_rad` is an `f64`. An absent position does not convert into a plain `Pose2`.

*class* `pb::VehicleState`

`pose` is `Option<Pose2>`. `speed_mps` is an `f64`.

*class* `pb::LidarScan`

`points` is a vector of `Point2`, in the sensor frame. `sensor_in_vehicle` is `Option<Pose2>`.

*class* `pb::Observation`

`time_ns`, an optional `state`, and an optional `scan`. The live nodes pass pose and scan as separate updates and pair them in memory. This message is the combined shape, used by `protocol::scan` while it validates points.

*class* `pb::Grid`

`origin` is `Option<Point2>`. `resolution_m` is an `f64`. `width` and `height` are `u32`. `weights` is a vector of `u32`, row-major, one entry per cell.

The plain [`Grid`](grid.md) uses `usize` for the extent and `u8` for each weight. `convert` carries a weight of 0 to 255 across that gap. `Grid::validate` on the plain grid is the check the nodes run after a conversion.

*class* `pb::Path`

`points`, a vector of `Point2` in world meters. This is the path nested inside a `PathUpdate` and inside `RunInfo.control_path`. The plain path type is `Vec<Point2>` in `shared::types`.

*class* `pb::DriveCommand`

`speed_mps` and `steering_rad`. Zero and zero is a stop. Positive steering is a left turn.

*class* `pb::VehicleSpec`

The same measurements as [`VehicleSpec`](types.md): `wheelbase_m`, `length_m`, `width_m`, `rear_axle_in_base`, `sensor_in_vehicle`, `max_speed_mps`, `max_steering_rad`, and `hardware_calibrated`. The two pose-like fields are `Option`. `hardware_calibrated` decodes as `false` when it was not set.

## The header

*class* `pb::Header`

The little record every update carries.

`run_id`

The run this message belongs to. `check_header` compares it to the node's run.

`sequence`

A counter that starts at 1 for a given stream. Zero is rejected. `Freshness::accept` requires the next value to be strictly larger.

`time_ns`

Capture time, in nanoseconds since the run started. Republishing a message should keep this number. A fresh command does not make an old path new.

`frame_id`

The frame that time is attached to. The helpers expect `"map"` on state, maps, paths, commands, run info, and status, and `"front_lidar_frame"` on a scan.

`protocol::header` builds one of these. `protocol::check_header` checks it.

## Updates

Each update is the body of one topic. The topic suffix is the last component of the key from [`Transport::key`](transport.md).

*class* `pb::StateUpdate`

Topic `state`. `header` plus `state`. The header frame is `"map"`. `protocol::state` is the reader.

*class* `pb::LidarUpdate`

Topic `lidar`. `header` plus `scan`. The header frame is `"front_lidar_frame"`. `protocol::scan` is the reader.

*class* `pb::MapUpdate`

Topic `map`. Perception publishes one of these for every paired observation, including an observation that produced no map.

`header`

The scan's header, with `frame_id` rewritten to `"map"`. The capture time stays the scan's capture time.

`source`

The same header perception used as `header`. Planning copies this onto the path so the path's deadline stays the scan's time.

`state`

The `VehicleState` from the paired pose.

`grid`

The plain grid, converted, when `Perception::update` returned `Some` and the grid validated. Absent when the update returned `None`.

`availability`

`Ready` when `grid` is present. `NotReady` when perception has no map yet.

*class* `pb::PathUpdate`

Topic `path`. `planned_update` builds this from a `MapUpdate`.

`header`

A copy of the map's header, so the path is numbered and timed with the map.

`source`

A copy of the map's `source`. Controls measure the path's age from this header's `time_ns`.

`map_sequence`

The map header's `sequence`.

`path`

The waypoints, when the planner returned a path that passed `validate_path`. Absent otherwise.

`availability`

`NotReady` when the map was not `Ready`. `NoPath` when the planner returned `None`. `Ready` when a path was stored. A path that fails `validate_path` does not produce a `PathUpdate`. `planned_update` returns an error instead.

*class* `pb::CommandUpdate`

Topic `command`. `controlled_update` and `Sim::drive` both publish this shape.

`header`

For a node, a new header: the caller's sequence, the state message's capture time, and frame `"map"`.

`source`

The path's `source` header, when the command follows a ready path. Absent on the stop that `controlled_update` returns for a missing or expired path. `Sim::drive` uses the last matched scan, or the last matched state in drive-test mode.

`path_sequence`

The path header's sequence, or zero when there is no path to cite.

`state_sequence`

The state header's sequence.

`valid_until_ns`

`source.time_ns + FRESH_NS` when the command cites a source. Zero on the early stop from `controlled_update`.

`command`

The speed and steering. The early stop stores `DriveCommand::STOP`.

## Run configuration and status

*class* `pb::RunInfo`

Topic `run_info`. One value describes the run. `Context::connect` waits for it, checks the header against frame `"map"`, and requires `schema_hash` to equal `protocol::fingerprint`. A later copy has to compare equal to the first, including the header, or `Context::poll` treats the run as changed.

`schema_hash`

Hex fingerprint of the schema this run was prepared with.

`goal`

World position of the goal, in meters.

`vehicle`, `grid`

The `VehicleSpec` and the starting `Grid`. `connect` converts both and validates them.

`goal_tolerance_m`, `timeout_s`

How close the car has to get, in meters, and how long the run may last, in seconds. The course file supplies these. Every course in the starter uses a tolerance of 0.2 m.

`mode`

A short name such as `"observe"`, `"controls"`, `"autonomous"`, `"drive-test"`, or `"transport-fixture"`. `Sim::drive` reads this string. Drive-test mode cites the last state as the command's source. The other modes cite the last scan.

`control_path`

The practice path, present when the mode is controls. Absent for the other modes. The controls fixture publisher sends it on `path`.

*class* `pb::NodeStatus`

Topic `status/<node>`, one of `status/perception`, `status/planning`, or `status/controls`.

`header` uses frame `"map"`. `node` is the role name. `ready` is whether the node has produced a usable result. `detail` is the short reason the launcher prints into `node-events.jsonl`. `pid` is the process id. The launcher rejects a second process that publishes status for the same role.

`Context::status` fills this in. The sequence on the header is the node's own status counter, starting at 1.

## Availability

*enum* `pb::Availability`

Stored on `MapUpdate` and `PathUpdate` as an `i32`.

| Value | Number | Meaning on the wire |
| --- | --- | --- |
| `Unspecified` | 0 | The zero value. A missing enum decodes as this. |
| `NotReady` | 1 | The producer has no result yet. Perception publishes this while `update` returns `None`. Planning publishes this while the map is not ready. |
| `Ready` | 2 | `grid` or `path` holds a result the next node can use. |
| `NoPath` | 3 | The planner ran and found no route. An empty path published by `Sim::path` uses this too. |
| `Invalid` | 4 | Reserved by the schema. The helpers in this crate do not publish it. |

`as_str_name` returns the Protobuf spelling, such as `"READY"`. `from_str_name` parses that spelling and returns `None` for anything else. Comparisons in the nodes use the integer, because that is what the field stores.

`controlled_update` treats every value other than `Ready` as a reason to publish a stop. That includes `NotReady`, `NoPath`, `Unspecified`, and `Invalid`.

## See also

- [`protocol`](protocol.md) — `convert`, `header`, `check_header`, `decode`
- [`transport`](transport.md) — the topic names these messages travel on
- [`planning-node`](planning-node.md) — who fills in a `PathUpdate`
- [`controls-node`](controls-node.md) — who fills in a `CommandUpdate`
