# `runner` — The course and one recorded update

**Source code:** [`src/runtime/runner.rs`](../../src/runtime/runner.rs)

This module is the description of a course and the small record the reporter stores for one step. It does not talk to the simulator. [`sim`](sim.md) launches a `Course`. [`reporting`](reporting.md) writes an `Update`. The command-line program in `src/main.rs` owns the loop that fills those records in.

The numbers come from a scenario file, `scenarios/onboarding.json` for a live run and `fixtures/planning/<name>.json` for a planning fixture. The struct checks that the file is usable. It does not invent a goal or a grid.

## Courses

*class* `runtime::runner::Course`

`goal`

World position of the goal, as a [`Point2`](types.md), in meters. On the onboarding course the red marker is `(−1.65, −0.93)`.

`goal_tolerance_m`

How close the car has to get, in meters. The starter courses use 0.2. `validate_path` and the runner's arrival check both read this field.

`timeout_s`

How long the run may last, in seconds. The program can shorten it from `--duration`, and it will not lengthen it past the file's value. The run ends when simulated time reaches this, when the car is inside the tolerance, or when the simulator reports a result.

`vehicle`

The [`VehicleSpec`](types.md) for the course. Clearance, the speed cap, and the steering cap come from here.

`grid`

The starting [`Grid`](grid.md). The onboarding scenario stores an empty grid. Live obstacles arrive as LiDAR. The planning fixtures replace this grid with a map that already has obstacles.

`control_path`

A [`Path`](types.md) the controls practice run publishes, so the controller can be tested without perception or planning. `validate` requires at least one finite point, including for modes that never publish it. The onboarding scenario carries a path for that reason.

`Course::validate(&self)`

Return `Ok(())` when the vehicle and the grid pass their own `validate` methods, `goal` is finite and `grid.cell(goal)` is on the map, `goal_tolerance_m` and `timeout_s` are finite and strictly positive, and `control_path` is non-empty and every point is finite.

Otherwise return `"Invalid course goal, bounds, timeout, or controls fixture"`, or the vehicle or grid error if that check failed first. A goal that sits on the far edge of the grid fails, because `cell` treats that edge as off the map.

## Modes

*enum* `runtime::runner::Mode`

The kind of run the program is in. The variants are `Observe`, `Controls`, `Autonomous`, and `DriveTest`. They are `Copy`.

The string stored on `RunInfo.mode` is the matching name: `"observe"`, `"controls"`, `"autonomous"`, or `"drive-test"`. The fixture publisher uses a fifth string, `"transport-fixture"`, and does not build a `Mode`. `Sim::drive` looks at the string, not at this enum. Drive-test cites the last vehicle state as the command's source. The other strings cite the last scan.

What each mode does with the car lives in the program, not in this type. Observe publishes a stop. Drive-test creeps at 0.1 m/s and 0.1 rad for one simulated second, then stops. Controls publishes `control_path` and checks that the car stays inside its clearance circle. Autonomous lets the three nodes publish the map, the path, and the command.

## Updates

*class* `runtime::runner::Update`

One step, as the reporter should record it. The program builds this. The reporter does not.

`command`

The [`DriveCommand`](types.md) after `VehicleSpec::limit`. This is the clamped command, which is what the recorder writes.

`status`

A short static string. The program uses `"observing"`, `"drive-test"`, `"following student path"`, `"waiting for student map/path and node wiring"`, `"goal reached"`, and `"timeout"`. `Reporter` copies it onto the summary, and treats `"goal reached"` as the arrival flag.

`path`

The waypoints in force at this step. An empty path means the recorder has nothing to draw in blue.

`finished`

`true` when the program is about to leave the loop because the goal was reached or the timeout fired.

`saturated`

`true` when `limit` changed the command the nodes published. The speed or the steering was finite and outside the car's caps. A non-finite command never gets here. `limit` returns an error first.

## See also

- [`sim`](sim.md) — launch a course
- [`reporting`](reporting.md) — write an `Update` to disk
- [`types`](types.md) — the goal, the vehicle, and the command
