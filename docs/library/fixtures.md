# `fixtures` — Practice publishers

**Source code:** [`src/runtime/fixtures.rs`](../../src/runtime/fixtures.rs)

This module publishes practice messages for one node. The messages go to the local Zenoh router. The simulator, the browser, and the wheels are not involved. A node started with the printed run id can subscribe, run its loop, and publish, and nothing on the car will move.

The command is `cargo run -- fixture-publish <role> --run <id>`. The program calls `publish`. The router has to be up. `Transport::connect` returns the usual message when it is not.

## The publisher

`publish(role, run, duration, interrupted)`

Publish inputs for `role`, which must be `"perception"`, `"planning"`, or `"controls"`. Anything else returns `"Unknown fixture role"`.

`run` is the run id. `None` allocates a UUID. `duration` is how long to publish, in seconds. `None` means 60. `interrupted` stops the loop when another thread sets it, the same flag `main` sets on Ctrl-C.

The course embedded from `scenarios/onboarding.json` becomes a `RunInfo`. The schema hash is `protocol::fingerprint`. The goal, vehicle, and grid are copied from that course. `goal_tolerance_m` is the course's. `timeout_s` is `duration`, or 60. `mode` is `"transport-fixture"`. `control_path` is included only for `"controls"`.

Print `Fixture run: <id>; no simulator or actuator connection. Start node <role> --run <id>` and then publish about ten times a second until the duration elapses or the flag is set. Each tick sleeps 100 ms. The capture time is `sequence * 100_000_000` nanoseconds, so the timestamps step by a tenth of a simulated second. The sequence starts at 1. Every tick republishes the same `RunInfo` on `run_info`, which is what keeps [`Context::poll`](context.md) from declaring the run lost.

The other topic depends on the role.

`"perception"` publishes the state and the scan from `fixtures/perception/observation.json`. The state goes on `state` with frame `"map"`. The scan goes on `lidar` with frame `"front_lidar_frame"`. Both use the tick's sequence and time. The fixture pose is a small rotation with duplicate points, so a perception implementation can be checked by hand.

`"planning"` publishes one `MapUpdate` on `map`. The grid and the starting state come from `fixtures/planning/empty.json`. `header` and `source` are the same header, frame `"map"`. Availability is `Ready`. This is an open map, not the detour or the blocked fixture. Those are loaded by `cargo run -- planning --scenario`, which does not use this publisher.

`"controls"` publishes a `StateUpdate` on `state` and a `PathUpdate` on `path`. The state is the planning empty-fixture start. The path is the course's `control_path`, availability `Ready`, with `map_sequence` equal to the tick sequence. The path message cites the same header as its source. A controls loop that wakes on state and reads the latest path sees a ready path on every tick.

The function returns `Ok(())` when the loop ends because of the duration or the flag. A transport error returns that error and stops publishing. It does not start a node. The node is a second process.

## See also

- [`transport`](transport.md) — `connect` and `publish`
- [`context`](context.md) — what the node does with the `RunInfo` from this loop
- [`messages`](messages.md) — the updates being published
