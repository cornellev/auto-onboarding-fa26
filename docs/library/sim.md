# `sim` — The simulator connection

**Source code:** [`src/runtime/sim.rs`](../../src/runtime/sim.rs)

This module checks that the simulator already has the onboarding run and holds one live connection. It does not install a world and it does not delete the saved manifest. The world lives in the simulator. A run launched here is paused until `resume`, gets a fresh run id, and is torn down by `finish` or by dropping the value.

The program in `src/main.rs` is the caller. It passes the simulator base URL, taken from `CEV_SIM_SERVER` or `http://localhost:3000`. Pose, lidar, and commands travel on the Zenoh router from [getting started](../getting-started.md), not through a separate orchestrator.

A connection implements [`VehicleIo`](hardware.md): `observe`, `drive`, and `stop`.

## Setup

`setup(server)`

Check that a simulator is answering at `server`, then that its storage has a run manifest named `onboarding`. Print `Run manifest onboarding is installed` and return `Ok(())`.

A missing manifest returns `"Run manifest onboarding is not installed in the simulator"`. This function does not create the manifest or copy a world. `cargo run -- setup` calls this and returns. It does not register a profile anywhere else.

The storage request uses a 15 second timeout, follows no redirects, and sends no bearer token.

`setup_profile()`

Register the bundled profile `config/protobuf-profile.json` at `/api/protobuf/profiles/cev-onboarding-v1` on an orchestrator. The profile's `fingerprint` must equal [`protocol::fingerprint`](protocol.md). A mismatch returns `"Regenerate schema and profile with cargo run --example generate_schema"`. On success print `Registered cev-onboarding-v1 (<fingerprint>)`. The simulator stack in getting started does not run that orchestrator, and `cargo run -- setup` does not call this function.

`api(method, path, value)`

Send one JSON request to an orchestrator. The base URL is `CEV_ORCHESTRATOR_API`, or `http://localhost:8090` when that variable is unset. `path` begins with a slash. `GET`, `DELETE`, and `PUT` are sent as those methods. Any other method is sent as `POST`. `value` is the JSON body for `PUT` and `POST`. `None` sends `{}`. The live simulator path does not use this function.

When `CEV_API_TOKEN` is set, the `Authorization` header is `Bearer <token>`. When it is unset, the header is empty. The timeout is 10 seconds, redirects are disabled, and an HTTP status of 400 or above returns `"<method> <url>: <body>"`. A transport failure or a body that is not JSON returns that error.

Requests to the simulator's own storage URL do not use this function and do not forward the token. The manifest lookup and the manifest `resolve` call are those requests.

## `Sim` objects

*class* `runtime::sim::Sim`

One prepared browser run and the Zenoh session that goes with it.

`transport`

The [`Transport`](transport.md) connected with this run's id. The program subscribes to `map`, `path`, `command`, and `status/<role>` on it.

`run_id`

A new UUID, also the Zenoh run id. Printed as `Run: <id>`. Nodes join it with `cargo run -- node <role> --run <id>`.

`info`

The [`RunInfo`](messages.md) registered for this launch: schema hash, goal, vehicle, grid, tolerances, mode string, and, for controls, the practice path.

The browser handle, the subscriptions, the join, and the sequence counters are private.

`Sim::prepare(course, server, mode)`

Validate `course`, then launch the saved `onboarding` manifest and leave the browser paused.

The simulator client uses a 30 second timeout. Preparation selects the connected browser identified by the server's default `browserId`, which follows the most recently reported human interaction. If no page has reported interaction, the most recently registered connected page wins. Click the intended simulator page before preparing. Only that page's run must be stopped; other connected pages and disconnected history do not block launch. A missing or unavailable default returns `"No connected simulator tab. Open the simulator and leave the run stopped."`. A selected run that is running, paused, ready, or preparing returns `"The simulator tab still has a run going. Stop it, then start again."`. A process that is not the simulator returns `"The process at <server> is not the simulator (...)"`.

The manifest is resolved through the simulator storage API. Its `resolvedHash` is the hash the browser must report after launch. The selected browser ID is passed explicitly to control lease acquisition and launch, and retained for subsequent commands even if focus changes. A thread renews the lease about every 200 ms so it does not expire during launch. The browser is then launched with `autoplay` off and that lease. After launch, the browser's resolved hash must equal the hash from storage, or the error is `"Prepared course changed during launch"`.

The run id comes from that launch. `Transport::connect` opens the session, then the connection subscribes to `state` with capacity 32 and `lidar` with capacity 8. The `RunInfo` is built from the course, including `control_lease_id`, and published on `run_info`. `mode` is stored on it as a string. `control_path` is included only when `mode` is `"controls"`. The simulator opens that router when it starts. A connection failure is the router error from [`transport`](transport.md).

The returned `Sim` is paused. `resume` starts the clock. `launch` is `prepare` with mode `"drive-test"` followed by `resume`.

`Sim::launch(course, server)`

`prepare(course, server, "drive-test")`, then `resume`. This is the drive-test entry. The mode string is always `"drive-test"`, which changes which header `drive` cites.

`Sim::resume(&mut self)`

Resume the browser run. Return `"No browser run"` when `prepare` did not store one. A failure from the simulator client is returned as that error.

`Sim::path(&self, path)`

Publish `path` on `path`, using the latest matched scan as the source and keeping that scan's capture time. Do nothing and return `Ok(())` when no scan has been matched yet. An empty slice is published with availability `NoPath`. A non-empty slice is published with availability `Ready` and those points. The header sequence is a counter private to this connection, starting at 1. `map_sequence` is the scan header's sequence.

This is how the controls mode feeds the practice path to the controls node without going through planning. A path published before the first scan is dropped on the floor here, not queued.

`Sim::status(&mut self)`

Republish `run_info` when the last publish was at least 200 ms ago, then return the browser snapshot from the simulator client. `"No browser run"` covers a missing handle.

The program reads `run_result` on that snapshot, when the simulator has finished, and `error`, when the simulator has failed.

`Sim::finish(&mut self)`

Publish a stop, release the control lease, then stop the browser once that release has paused the tab. Mark the connection done so `Drop` does not do the work twice. Return the snapshot from that stop. The saved `onboarding` manifest stays where it is.

The stop is `drive(DriveCommand::STOP)`. A failure at any step returns that error and leaves `done` clear, so `Drop` still releases the lease and stops the browser. Stopping while the lease is still held, or releasing it and then stopping with that same lease, leaves the tab paused. The stop after the release is what lets the next launch start.

`Sim::observe(&mut self, timeout)`

Wait up to `timeout` for a pose and a scan from the same capture time. This is the [`VehicleIo::observe`](hardware.md) implementation.

About every 250 ms, republish `run_info`. Then drain `state` and `lidar`. Each sample goes through `protocol::state` or `protocol::scan`, then `Freshness::accept`, then the [`ObservationJoin`](synchronizer.md). A repeated sequence or a capture clock that runs backward is an error from `accept`.

When the join has a match, both freshness records have to be [`fresh`](protocol.md) against the latest state capture time. Otherwise return `"Buffered observation expired"`. The scan header's frame is rewritten to `"map"` and both headers are stored for `path` and `drive`. Return `Ok(Some(observation))`.

Return `Ok(None)` when `timeout` expires with no match. The loop sleeps 5 ms between drains. A zero timeout either returns a sample that is already buffered or `None`.

`Sim::drive(&mut self, command)`

Clamp `command` with the course vehicle's [`limit`](types.md), then publish a `CommandUpdate` on `command`. A non-finite command returns the `limit` error and nothing is published.

The cited source depends on `info.mode`. `"drive-test"` uses the last matched state header. Every other mode uses the last matched scan header. A command other than `DriveCommand::STOP` with no source yet returns `"No fresh driving source"`. A stop is published even when no sample has arrived, so shutdown can ask the wheels to stop before the first scan.

The header sequence is this connection's command counter. The header frame is `base_link`. The header time is the last state capture time, or zero when no state has arrived. `path_sequence` is the counter `path` last used. `state_sequence` is the last state header's sequence, or zero. `valid_until_ns` is the source capture time plus `FRESH_NS`, or zero when there is no source. `control_lease_id` is the lease `prepare` acquired. The published command is the clamped one.

`Sim::stop(&mut self)`

`drive(DriveCommand::STOP)`.

## Dropping a connection

If `finish` has not marked the run done, `Drop` releases the lease and stops the browser the same way `finish` does. Errors are ignored. The saved manifest is not deleted. A panic or an early return from the program still takes this path, which is why a stuck browser run is usually a process that never owned the `Sim`.

## See also

- [`runner`](runner.md) — `Course`, the value `prepare` validates
- [`hardware`](hardware.md) — the trait `observe`, `drive`, and `stop` implement
- [`transport`](transport.md) — the session stored on `Sim`
- [`fixtures`](fixtures.md) — a publisher that skips this module and never starts a browser run
