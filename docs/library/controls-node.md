# `controls::node` — From a state and a path to a command

**Source code:** [`src/controls/node.rs`](../../src/controls/node.rs)

This file has two functions. `controlled_update` is the helper that turns the latest state and the latest path into one `CommandUpdate`. `run` is the loop you write around it. Until that loop subscribes and publishes, the process announces that it is not wired. The launcher holds the car still while the command topic stays quiet, and a dropped controls context publishes one stop on the way out.

Wake up once per new state, about ten times a second, and pass `dt_s` from the capture times. Keep the latest path even when that path says there is nothing to follow. When you publish a command, leave the path's original time alone. A fresh command does not refresh an old path.

## The loop you write

`run(ctx)`

The starter body calls `ctx.wait_for_wiring` with the detail `"controls still needs its Zenoh subscribers and publisher; see docs/project.md, Connecting the pieces"`. That prints the line, publishes `ready: false` about ten times a second, and returns when the run stops.

The loop that replaces it should subscribe to `state` and `path`, remember the latest path, and call `controlled_update` once per new state. Publish the returned `CommandUpdate` on `command`. `dt_s` is the gap between this state's capture time and the previous one, in seconds. The perception loop is the shape to copy: `poll`, `receive`, then publish.

## The helper

`controlled_update(ctx, state, path, controller, sequence, dt_s)`

Build a `CommandUpdate` from one state message and an optional path message. `controller` is borrowed mutably. `sequence` becomes the command header's sequence. The helper does not increment it. The caller owns that counter.

Read the state with `protocol::state`, which requires frame `"map"` and a finite pose and speed. The command header uses `sequence`, the state's capture time, and frame `"map"`. `state_sequence` is the state header's sequence. The command field starts as `DriveCommand::STOP`, and `source`, `path_sequence`, and `valid_until_ns` stay empty or zero until a path is actually followed.

The checks run in this order.

1. `path` is `None`. Return the stop. The loop has not seen a path yet.
2. `path.availability` is not `Ready`. Return the stop. `NotReady`, `NoPath`, `Unspecified`, and `Invalid` all take this branch.
3. `check_header` on the path header and on `source`, both in frame `"map"`. A missing or mismatched header is an error, including when `dt_s` would also have been rejected.
4. `dt_s` is non-finite or less than or equal to zero. Return the stop.
5. The state's capture time minus the path source's capture time fails, or the age is greater than or equal to [`FRESH_NS`](protocol.md). Return the stop. The comparison is `>=`. A path that is exactly 300 ms old is a stop.
6. A ready path with no `path` field returns `"Missing ready path"`.
7. The points are converted to plain `Point2` values. An empty list, a list longer than 100,000, or a non-finite point returns `"Invalid path points"`. An empty ready path is an error. The controller is not asked to drive it. The controller should still stop if a loop ever calls `command` with an empty slice directly.

Steps 1, 2, 4, and 5 are successful stops. Steps 3, 6, and 7 are errors.

Otherwise call `controller.command` with the plain state, the points, and `dt_s`. Pass that command through `controller.vehicle.limit`. A non-finite speed or steering angle returns `"Non-finite command rejected; stopping"`. A finite command that sits outside the car's caps still returns `Ok` from `limit`. This helper publishes the original command, the numbers the controller asked for. The runner calls `limit` again and records a saturated update when the clamp would have changed them. The simulator clamps once more before the wheels move.

On that success the message cites the path. `source` is the path's source header. `path_sequence` is the path header's sequence. `valid_until_ns` is `source.time_ns + FRESH_NS`. The path's own header time is not rewritten.

The helper does not decide that the car has arrived. The runner does, and it has its own safety stop. The helper does not publish. `run` publishes on `"command"`.

## See also

- [`types`](types.md) — `DriveCommand` and `VehicleSpec::limit`
- [`messages`](messages.md) — `CommandUpdate` and `Availability`
- [`perception-node`](perception-node.md) — the loop to copy
- [`context`](context.md) — the stop published when a controls context is dropped
