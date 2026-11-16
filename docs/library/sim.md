# `sim` — The simulator connection

**Source code:** [`src/runtime/sim.rs`](../../src/runtime/sim.rs)

This module checks that the simulator already has the onboarding run, registers the Protobuf profile with orchestrator, and holds one live connection. It does not install a world and it does not delete the saved manifest. The world lives in the simulator. A run launched here is paused until `resume`, gets a fresh run id, and is torn down by `finish` or by dropping the value.

The program in `src/main.rs` is the caller. It passes the simulator base URL, taken from `CEV_SIM_SERVER` or `http://localhost:3000`. The functions that talk to orchestrator use a different base URL, below.

A connection implements [`VehicleIo`](hardware.md): `observe`, `drive`, and `stop`.

## Setup

`setup(server)`

Check that a simulator is answering at `server`, then that its storage has a run manifest named `onboarding`. Print `Run manifest onboarding is installed` and return `Ok(())`.

A missing manifest returns `"Run manifest onboarding is not installed in the simulator"`. This function does not create the manifest, copy a world, or register the Protobuf profile. `setup_profile` is the separate call the `setup` command makes next. Run it again after orchestrator restarts, because that registration dies with the orchestrator process.

The storage request uses a 15 second timeout, follows no redirects, and sends no bearer token.

`setup_profile()`

Register the bundled profile `config/protobuf-profile.json` at `/api/protobuf/profiles/cev-onboarding-v1` on orchestrator. The profile's `fingerprint` must equal [`protocol::fingerprint`](protocol.md). A mismatch returns `"Regenerate schema and profile with cargo run --example generate_schema"`. On success print `Registered cev-onboarding-v1 (<fingerprint>)`.

`api(method, path, value)`

Send one JSON request to orchestrator. The base URL is `CEV_ORCHESTRATOR_API`, or `http://localhost:8090` when that variable is unset. `path` begins with a slash. `GET`, `DELETE`, and `PUT` are sent as those methods. Any other method is sent as `POST`. `value` is the JSON body for `PUT` and `POST`. `None` sends `{}`.

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

The simulator client uses a 30 second timeout. The tab list has to contain exactly one connected browser, and that browser's run has to be stopped. A disconnected tab left over from an earlier session does not count. The errors are the ones the writeup names: `"No connected simulator tab. Open the simulator and leave the run stopped."`, `"More than one connected simulator tab. Close the extras and leave one run stopped."`, and `"The simulator tab still has a run going. Stop it, then start again."` A process that is not the simulator returns `"The process at <server> is not the simulator (...)"`.

The manifest is resolved through the simulator storage API. Its `resolvedHash` is the hash the browser must report after launch. The manifest's `tf`, `front-lidar-points`, and `controls-command` topics are copied into the routing context. A missing topic returns `"Run manifest onboarding is missing topic <id>"`.

A new run id is chosen. `Transport::connect` opens the session, and the connection subscribes to `state` with capacity 32 and `lidar` with capacity 8 before the browser starts, so the first samples are not missed. The `RunInfo` is built from the course. `mode` is stored on it as a string. `control_path` is included only when `mode` is `"controls"`.

The routing context is posted to `/api/protobuf/runs`. The browser is then launched with `autoplay` off and the chosen tab. If the first launch fails because orchestrator transport is not connected, the call waits one second and tries once more. After launch, the browser's resolved hash must equal the hash from storage, or the error is `"Prepared course changed during launch"`. `/api/protobuf/runs/<id>/activate` binds that browser, the simulator run id, and the hash.

The returned `Sim` is paused. `resume` starts the clock. `launch` is `prepare` with mode `"drive-test"` followed by `resume`.

`Sim::launch(course, server)`

`prepare(course, server, "drive-test")`, then `resume`. This is the drive-test entry. The mode string is always `"drive-test"`, which changes which header `drive` cites.

`Sim::resume(&mut self)`

Resume the browser run. Return `"No browser run"` when `prepare` did not store one. A failure from the simulator client is returned as that error.

`Sim::path(&self, path)`

Publish `path` on `path`, using the latest matched scan as the source and keeping that scan's capture time. Do nothing and return `Ok(())` when no scan has been matched yet. An empty slice is published with availability `NoPath`. A non-empty slice is published with availability `Ready` and those points. The header sequence is a counter private to this connection, starting at 1. `map_sequence` is the scan header's sequence.

This is how the controls mode feeds the practice path to the controls node without going through planning. A path published before the first scan is dropped on the floor here, not queued.

`Sim::status(&self)`

Check that orchestrator still attributes `/api/protobuf/runs` to this `run_id` and that the routing `fault` is null. Otherwise return `"Routing run lost or failed: <body>"`. Then return the browser snapshot from the simulator client. `"No browser run"` covers a missing handle.

The program reads `run_result` on that snapshot, when the simulator has finished, and `error`, when the simulator has failed.

`Sim::finish(&mut self)`

Publish a stop, delete `/api/protobuf/runs/<id>`, stop the browser, and mark the connection done so `Drop` does not do the work twice. Return the snapshot from the browser stop. The saved `onboarding` manifest stays where it is.

The stop is `drive(DriveCommand::STOP)`. A failure at any step returns that error and leaves `done` clear, so `Drop` still tries to remove the routing and stop the browser.

`Sim::observe(&mut self, timeout)`

Wait up to `timeout` for a pose and a scan from the same capture time. This is the [`VehicleIo::observe`](hardware.md) implementation.

About every 250 ms, read `/api/protobuf/runs` and return `"Routing run replaced"` when the active run id is no longer this one. Then drain `state` and `lidar`. Each sample goes through `protocol::state` or `protocol::scan`, then `Freshness::accept`, then the [`ObservationJoin`](synchronizer.md). A repeated sequence or a capture clock that runs backward is an error from `accept`.

When the join has a match, both freshness records have to be [`fresh`](protocol.md) against the latest state capture time. Otherwise return `"Buffered observation expired"`. The scan header's frame is rewritten to `"map"` and both headers are stored for `path` and `drive`. Return `Ok(Some(observation))`.

Return `Ok(None)` when `timeout` expires with no match. The loop sleeps 5 ms between drains. A zero timeout either returns a sample that is already buffered or `None`.

`Sim::drive(&mut self, command)`

Clamp `command` with the course vehicle's [`limit`](types.md), then publish a `CommandUpdate` on `command`. A non-finite command returns the `limit` error and nothing is published.

The cited source depends on `info.mode`. `"drive-test"` uses the last matched state header. Every other mode uses the last matched scan header. A command other than `DriveCommand::STOP` with no source yet returns `"No fresh driving source"`. A stop is published even when no sample has arrived, so shutdown can ask the wheels to stop before the first scan.

The header sequence is this connection's command counter. The header time is the last state capture time, or zero when no state has arrived. `path_sequence` is the counter `path` last used. `state_sequence` is the last state header's sequence, or zero. `valid_until_ns` is the source capture time plus `FRESH_NS`, or zero when there is no source. The published command is the clamped one.

`Sim::stop(&mut self)`

`drive(DriveCommand::STOP)`.

## Dropping a connection

If `finish` has not marked the run done, `Drop` deletes the routing run and, when the browser handle still answers `status`, stops the browser. Errors are ignored. The saved manifest is not deleted. A panic or an early return from the program still takes this path, which is why a stuck browser run is usually a process that never owned the `Sim`.

## See also

- [`runner`](runner.md) — `Course`, the value `prepare` validates
- [`hardware`](hardware.md) — the trait `observe`, `drive`, and `stop` implement
- [`transport`](transport.md) — the session stored on `Sim`
- [`fixtures`](fixtures.md) — a publisher that skips this module and never starts a browser run
