# `reporting` — The result folder

**Source code:** [`src/runtime/reporting.rs`](../../src/runtime/reporting.rs)

This module writes the folder a run leaves behind: a CSV of updates, a JSON Lines log of node status, a summary, and an SVG of the map. The program decides when to record and what status string to use. The module formats those decisions and flushes them.

The CSV is seconds, meters, and radians, on the simulator's clock. In the picture, occupied cells are black, the path you asked for is blue, the path the car drove is orange, and the goal is green. The same course is a fair comparison. The exact trace can still wobble, because the browser's timing is not perfectly repeatable.

## The summary

*class* `runtime::reporting::Summary`

The counters written to `summary.json`. `Default` is a run that has not started: `status` is `"not started"`, `goal_reached` is false, `elapsed_s` and `traveled_m` are zero, and every optional measurement is `None`.

`status`

The latest status string. `record` copies it from the [`Update`](runner.md). A failure path in the program overwrites it with the error text before `finish`.

`goal_reached`

`true` only when `record` saw the status `"goal reached"`.

`elapsed_s`

The observation time, in seconds. `record` sets it to `time_ns / 1e9`.

`final_distance_m`

Distance from the latest position to the goal, in meters. `None` until the first `record`.

`traveled_m`

Sum of the straight-line steps between recorded positions, in meters. The first point adds nothing.

`mean_tracking_error_m`

The mean of the tracking errors on rows that had a path. `None` when no row had one. An empty path leaves that row out of the mean.

`collisions`

Unused by this module. The program does not fill it in. `finish` prints it as unavailable and points at the simulator's Replay view.

`simulator_passed`, `termination_reason`

Filled by the program from the browser snapshot, when the simulator attached a run result. `None` until then. A `simulator_passed` of `false` after the car reached the goal can mean the browser recorded the stop as an interruption. `goal_reached` and `termination_reason` are the two fields to read together.

## The reporter

*class* `runtime::reporting::Reporter`

`directory`

The folder passed to `new`. The program uses `results/<mode>-<nanoseconds>`.

`trajectory`

The positions `record` has appended, in world meters. `finish` draws them in orange.

`source_time_ns`

The capture time of the command's source, when the caller has set it. `record` does not write this field. It reads it to compute `source_age_ns` in the CSV: the observation time minus this time, when the subtraction succeeds. An unset value leaves that CSV cell empty.

`summary`

The [`Summary`](#the-summary) so far. Public, so the program can set `status` on a fixture run that never calls `record`, and can copy `simulator_passed` off the browser before `finish`.

`Reporter::new(directory)`

Create `directory`, including parents. Truncate `updates.csv` and `node-events.jsonl` if they already exist. Write the CSV header:

```text
time_s,x_m,y_m,yaw_rad,speed_mps,command_speed_mps,command_steering_rad,tracking_error_m,compute_us,saturated,source_age_ns
```

Return the I/O error if the directory or either file cannot be created.

`Reporter::event(&mut self, node, detail, time_ns, pid)`

Append one JSON object to `node-events.jsonl` and flush. The keys are `node`, `pid`, `detail`, and `time_ns`. This is the line the program writes when a `NodeStatus` arrives. The detail string is the one the node passed to `Context::status`.

`Reporter::record(&mut self, observation, update, goal, compute_us)`

Append the position to `trajectory`, add the step from the previous position to `traveled_m`, and update the summary from `observation` and `update`. Then append one CSV row and flush.

`tracking_error_m` is [`tracking_error`](#distance-to-a-path) of the position against `update.path`. A path that yields `None` leaves the cell empty and does not move the mean. `compute_us` is written as given. The program currently passes 0. `saturated` is `update.saturated`, written as `true` or `false`. `source_age_ns` uses `source_time_ns` as described above.

`goal` is only used for `final_distance_m`. Arrival is not recomputed here. The status string is.

`Reporter::finish(&mut self, map, path, goal)`

Flush the CSV, write `summary.json` as pretty JSON with a trailing newline, and write `view.svg` by calling `write_svg`. Print the status, the goal distance in meters to two places or `unavailable`, the collision count or the Replay hint, and the directory path.

Return a grid error from `write_svg` if `map` fails `validate`, or an I/O or JSON error if a file cannot be written. The print happens after both files are written.

## Distance to a path

`tracking_error(point, path)`

Return the shortest distance, in meters, from `point` to `path`. Both are in the same frame.

Return `None` when `path` is empty. Return the distance to the only point when `path` holds one waypoint. Otherwise consider each segment. Project `point` onto the infinite line through the segment, clamp the fraction to `[0, 1]`, and take the distance to that clamped point. A segment whose endpoints coincide uses the first endpoint. The returned value is the minimum of those distances.

The mean in the summary is the mean of these values over recorded rows. It is a distance to the requested path, not a heading error.

## The picture

`write_svg(file, map, path, trajectory, goal)`

Validate `map`, then overwrite `file` with one SVG. The view box is 900 by 600. The map is drawn in a rectangle inset by 30 pixels, scaled so the grid fits in 840 by 500. World +x goes right. World +y goes up. Occupied cells, those `blocked` at the default cutoff, are filled rectangles. `path` is a blue polyline. `trajectory` is an orange polyline. `goal` is a green circle.

An invalid grid returns the `validate` error and writes nothing. A polyline with no points is an empty `points` attribute, which draws nothing. The legend at the bottom repeats the colors.

`finish` is the caller inside this crate. The function is public so a fixture result can be drawn with the same picture.

## See also

- [`runner`](runner.md) — `Update`, the value `record` consumes
- [`grid`](grid.md) — the map `write_svg` validates and draws
- [`sim`](sim.md) — the run that produces the observations
