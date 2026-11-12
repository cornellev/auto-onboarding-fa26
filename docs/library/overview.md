# The onboarding library

This library reference describes the helper code distributed with the freshman onboarding starter. It is the code the nodes call: geometry, the occupancy grid, Protobuf messages, the Zenoh session, and the runner that starts a course and writes a result.

Elsewhere, the [project writeup](../project.md) describes the three functions you implement, and [getting started](../getting-started.md) describes the tools and the commands that launch a run.

The library is the part that already runs. A point, a pose, and a scan live in plain structs, in meters and radians. A grid stores what perception has marked. Messages on the router are Protobuf, and `protocol::convert` copies between those bytes' types and the plain structs. Your algorithms should stay on the plain structs. The message loops call into this code to check a header, pair a pose with a scan, and publish the result.

A few files in the repository sit outside this manual. `src/perception/algorithm.rs`, `src/planning/algorithm.rs`, and `src/controls/algorithm.rs` are the functions you fill in. `src/main.rs` is the command-line program. The `mod.rs` files only declare modules. Examples and tests are not part of the library. The pages below are one page per helper file, in the order a scan travels: from a point, onto a grid, onto the router, and into a saved run.

## Introduction

- [`errors`](errors.md) — `Error`, `Result`, and `error`

## Geometry and maps

- [`types`](types.md) — Points, poses, scans, commands, and the car
  - `Point2` — a point in a plane, in meters. `distance` is the straight-line distance. `is_finite` reports that both coordinates are real numbers.
  - `Pose2` — a position and a heading. `transform` rotates a point by the heading, then adds the position. `inverse_transform` undoes that, taking a world point into the pose's frame. `is_finite` checks the position and the heading.
  - `wrap_angle` — fold an angle, in radians, into −π..π.
  - `VehicleState` — a pose and a forward speed, in meters per second.
  - `LidarScan` — endpoints in the sensor frame, and the pose of the sensor on the car.
  - `Observation` — one moment: a time, a `VehicleState`, and a `LidarScan`. `validate` rejects a pose, speed, mount, or point that is not a real number, and a scan with more than 100,000 points.
  - `Path` — a list of `Point2` waypoints.
  - `DriveCommand` — a forward speed and a steering angle. `DriveCommand::STOP` is zero speed and zero steering.
  - `VehicleSpec` — the car's measurements and limits. `validate` checks that they are real and inside the starter caps. `clearance_m` is the circle around the rear axle that covers the car. `require_finite` rejects a command that is not a real number. `limit` clamps a finite command to those caps. The controls node calls `require_finite` and publishes the controller's numbers. The runner calls `limit`.
- [`grid`](grid.md) — Occupancy grids
  - `Grid` — a rectangle of cells. Columns follow world x, rows follow world y, and cell `(col, row)` lives at `row * width + col`. Each cell holds a weight from 0 to 255.
  - `Grid::empty` — allocate a grid of weight 0. `Grid::validate` checks the origin, the resolution, and the storage.
  - `Grid::cell` — column and row of a world point, or `None` off the map. `Grid::center` is the middle of a cell, back in world meters.
  - `Grid::blocked` — true at a weight of 128 or more, and true off the map. `Grid::is_occupied` uses a threshold (`None` means 128) and is false off the map.
  - `Grid::set_occupied` — write 255. `Grid::set_weight` — write one belief from 0 to 255. A column or row off the map is left unchanged.
  - `Grid::clear` — whether a circle stays on the map and misses every blocked cell. `Grid::segment_clear` asks the same question along the strip between two points.

## Messages and the router

- [`protocol`](protocol.md) — Headers, freshness, and conversion
  - `FRESH_NS` and `FRESH_MS` — 300 milliseconds, the age after which a map, path, or command is too old to drive on. `MAX_DECODE_SIZE` is 8 MB. `DESCRIPTOR` is the bundled schema. `fingerprint` hashes it.
  - `convert` — copy a value into another type with the same JSON shape.
  - `header` — build the small header every message carries. `check_header` requires that header, this run, the expected frame, and a sequence past zero.
  - `state` — pull a `VehicleState` out of a state message. `scan` pulls a `LidarScan` out of a scan message. `decode` turns bytes into one Protobuf message.
  - `Freshness` — remember the last sequence and capture time. `accept` takes a newer header. `time_ns` is that capture time. `fresh` is true when the message arrived recently and its capture time is within 300 ms of `now_ns`.
- [`messages`](messages.md) — The Protobuf messages in `shared::protocol::pb`
  - `Header` — run id, sequence, capture time, and frame id.
  - `StateUpdate`, `LidarUpdate`, `MapUpdate`, `PathUpdate`, `CommandUpdate` — the messages on `state`, `lidar`, `map`, `path`, and `command`.
  - `RunInfo` — the run's goal, vehicle, grid, and tolerances. `NodeStatus` — whether a node is ready, and a short reason.
  - `Availability` — `Unspecified`, `NotReady`, `Ready`, `NoPath`, and `Invalid`.
  - The Protobuf `Point2`, `Pose2`, `Grid`, and the other value messages mirror the plain structs. Algorithms should keep using the plain structs.
- [`transport`](transport.md) — A Zenoh session on this machine
  - `Transport` — one session and one run id. `connect` opens the session. `key` is the full topic name. `subscribe` listens. `publish` sends one Protobuf message.
  - `Subscription` — the queue `subscribe` returns. `receive` reads one message from it, or `None` when the wait expires.
- [`synchronizer`](synchronizer.md) — Pair a pose with the scan from the same moment
  - `ObservationJoin` — a small buffer of poses and scans. `state` and `scan` remember one sample. `take` returns the newest scan that has a pose from the same capture time.

## Nodes

- [`context`](context.md) — How a node starts, notices the run is gone, and reports status (`runtime::node`)
  - `Context` — the session, the `RunInfo`, and the flag that asks the node to stop. `connect` waits for that `RunInfo`. `poll` is true while the run is still there. `status` publishes a ready flag and a short reason. `wait_for_wiring` sits in that loop until the run stops. Dropping a controls context publishes a stop.
- [`perception-node`](perception-node.md) — The finished perception message loop
  - `run` — subscribe to pose and LiDAR, pair them, call `Perception::update`, and publish a map.
- [`validation`](validation.md) — Check a path before it is published
  - `validate_path` — real numbers, a start near the car, an end near the goal, and clearance on every point and every segment.
- [`planning-node`](planning-node.md) — Turn a map message into a path message
  - `planned_update` — call `Planner::plan`, then `validate_path`. A map that is not ready stays not ready. `None` from the planner becomes "no path."
  - `run` — the loop you write. Until you do, it publishes "not ready" and waits.
- [`controls-node`](controls-node.md) — Turn a state and a path into a command
  - `controlled_update` — call `Controller::command`. A missing, unready, or expired path, or a bad `dt_s`, becomes a stop.
  - `run` — the loop you write. Until you do, it publishes "not ready" and waits.

## Running a course

- [`runner`](runner.md) — The course description and one recorded update
  - `Course` — goal, tolerances, vehicle, grid, and the practice path. `validate` checks them.
  - `Mode` — `Observe`, `Controls`, `Autonomous`, or `DriveTest`.
  - `Update` — the command, status, path, and flags for one recorded step.
- [`sim`](sim.md) — The simulator connection
  - `setup` — check that the saved `onboarding` manifest is installed. `setup_profile` registers the Protobuf profile with orchestrator. `api` sends one JSON request to orchestrator.
  - `Sim` — one prepared run. `prepare` launches it paused. `launch` prepares a drive-test and resumes it. `resume` starts the clock. `observe` returns a pose and a scan from the same moment. `path` publishes waypoints. `drive` publishes a limited command. `stop` publishes a stop. `status` reads the browser run. `finish` stops it and removes the routing. Dropping a `Sim` stops a run that `finish` did not.
- [`fixtures`](fixtures.md) — Practice publishers that never move the car
  - `publish` — send `run_info` and the inputs for `perception`, `planning`, or `controls`, about ten times a second.
- [`hardware`](hardware.md) — The trait a physical car would implement
  - `VehicleIo` — `observe`, `drive`, and `stop`. `Sim` implements it. `validate_hardware_spec` requires a measured car.
- [`reporting`](reporting.md) — The result folder
  - `Reporter` — write `updates.csv`, `node-events.jsonl`, `summary.json`, and `view.svg`. `event` appends one node line. `record` appends one update. `finish` writes the summary and the picture.
  - `Summary` — the counters in `summary.json`. `tracking_error` is the distance from a point to a path. `write_svg` draws the map, the path, the trajectory, and the goal.

## The functions this library calls

These structs are constructed for you. The bodies are yours, and the contract is in the [project writeup](../project.md). They are listed here so the boundary is visible.

- `Perception::new` stores the grid the runner built. `Perception::update` turns one `Observation` into `Some(&self.map)`, or `None` when you do not have a map yet. The starter returns `None`.
- `Planner::plan` returns the waypoints from the car to the goal, or `None` when there is no way through. A waypoint has to pass `Grid::clear`, and a step has to pass `Grid::segment_clear`. `validate_path` errors when they do not. The starter returns `None`.
- `Controller::new` stores the `VehicleSpec`. `Controller::command` returns the speed and steering that follow a path. Positive steering is a left turn. Those numbers already have to sit inside the car's caps. The starter returns `DriveCommand::STOP`.
