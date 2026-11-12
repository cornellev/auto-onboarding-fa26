# `planning::node` — From a map message to a path message

**Source code:** [`src/planning/node.rs`](../../src/planning/node.rs)

This file has two functions. `planned_update` is the helper that turns one `MapUpdate` into one `PathUpdate`. `run` is the loop you write around it. Until that loop subscribes and publishes, the process announces that it is not wired and the car keeps whatever path it already had, which at the start of a run is none.

Publish a path for every map, including a map that is not ready and a planner that returned `None`. A missing publication leaves the previous path in place. Copy the map's source time onto the path. A new plan does not make an old scan new.

## The loop you write

`run(ctx)`

The starter body calls `ctx.wait_for_wiring` with the detail `"planning still needs its Zenoh subscriber and publisher; see docs/project.md, Connecting the pieces"`. That prints the line, publishes `ready: false` about ten times a second, and returns when the run stops.

The loop that replaces it should follow [`perception::node::run`](perception-node.md). Subscribe to `map`. Keep one `Planner` alive across messages. On each `MapUpdate`, call `planned_update`, and publish the `PathUpdate` on `path`. `Context` holds the session, `transport.key`, the goal, and the vehicle. `prost::Message` turns a path into bytes. `Context::poll` is how the loop hears that it should quit. `Context::status` is how a wiring problem stays distinct from an algorithm problem.

An error from `planned_update` means the path was rejected or the message was malformed. Publishing that error as a path is not what the helper does. It returns `Err` so the loop can stop.

## The helper

`planned_update(ctx, map, planner)`

Build a `PathUpdate` from one `MapUpdate`. `planner` is borrowed mutably so it can keep search state across calls. The helper does not construct the planner.

Check `map.header` and `map.source` with `check_header`, both against this run and frame `"map"`. A missing or mismatched header is an error. The outgoing message copies both headers. `map_sequence` is the map header's sequence. The path therefore carries the map's identity and the scan time the map cited as its source. The helper does not stamp the path with the wall clock.

The outgoing availability starts at `NotReady`, with no path. If `map.availability` is anything other than `Ready`, return that message. The planner is not called. A perception node that is still returning `None` shows up here as `NotReady`, and the published path says the same.

When the map is ready, `grid` and `state` have to be present. `"Missing ready map"` and `"Missing capture state"` cover the holes. The grid is converted and `validate`d. The goal and the vehicle come from `ctx.info`, not from the map. `"Missing goal"` and `"Missing vehicle"` cover an incomplete `RunInfo`, which `Context::connect` would already have rejected. The vehicle is converted to a plain `VehicleSpec` so `clearance_m` can be read.

Call `planner.plan` with the grid, the state, the goal, `clearance_m()`, and `ctx.info.goal_tolerance_m`.

- `Some(path)` is passed to [`validate_path`](validation.md) with the car's position, the goal, the same radius, and the same tolerance. On success the points are converted, stored on the message, and the availability becomes `Ready`. A failed check returns that error and no message.
- `None` sets the availability to `NoPath` and leaves `path` empty. That is a successful return. The caller publishes it.

The function does not smooth the path, and it does not publish. Publishing is `run`'s job, with `Transport::publish` or a publisher declared on the session. Either way the bytes are a `PathUpdate` and the suffix is `"path"`.

## See also

- [`validation`](validation.md) — the clearance check
- [`perception-node`](perception-node.md) — the loop to copy
- [`messages`](messages.md) — `MapUpdate` and `PathUpdate`
- [`context`](context.md) — `poll`, `status`, and `info`
