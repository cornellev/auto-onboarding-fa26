# `runtime::node` — How a node starts and reports status

**Source code:** [`src/runtime/node.rs`](../../src/runtime/node.rs)

This module is the handle a node loop holds. `Context::connect` joins a run that someone has already prepared. `poll` tells the loop to keep going, or to stop. `status` publishes a one-line report the launcher writes into `node-events.jsonl`. The perception, planning, and controls loops all take a `Context` and do not open their own session.

The run id, the schema hash, the vehicle, and the grid are checked once, at connect. A later `RunInfo` has to be the same value. A changed configuration is an error, and the remedy is a new run.

## `Context` objects

*class* `runtime::node::Context`

`transport`

The [`Transport`](transport.md) for this run. `transport.key`, `subscribe`, and `publish` are how the loop names topics.

`info`

The [`RunInfo`](messages.md) received at startup. The goal, the vehicle, the grid, `goal_tolerance_m`, and `timeout_s` live here. `planned_update` reads the goal and the vehicle from this value rather than from the map message.

`interrupted`

An `Arc<AtomicBool>` shared with the process signal handler. `poll` returns `false` once another thread stores `true`. The library does not install the handler. `main` does, on Ctrl-C.

The subscription, the last heartbeat time, the status counter, and the node name are private.

`Context::connect(node, run, interrupted)`

Connect to the router for `run`, subscribe to `run_info` with capacity 1, and wait up to 10 seconds for a `RunInfo`.

Each wait is 100 ms. If `interrupted` is set during the wait, return `"Node interrupted during startup"`. If the deadline passes with no message, return `"No run_info; start a prepared run first"`.

On a message, `check_header` requires frame `"map"`. `schema_hash` must equal `protocol::fingerprint`, or the error is `"Node schema differs from run schema"`. The vehicle and the grid must be present, and both must pass their `validate` methods. `"Missing vehicle spec"` and `"Missing grid"` cover an absent field. A vehicle or grid that fails its own check returns that check's error.

`node` is stored and later used as the status topic suffix, `status/<node>`, and as the name inside `NodeStatus`. The perception, planning, and controls processes pass `"perception"`, `"planning"`, and `"controls"`.

`Context::poll(&mut self)`

Return `Ok(true)` while the run is still there and this process has not been asked to stop.

Return `Ok(false)` when `interrupted` is set. The loop should exit cleanly. That is not an error.

If a new `RunInfo` is waiting, it must compare equal to `info`, header included. A difference returns `"Run configuration changed; start a fresh run"`. An equal message refreshes the heartbeat clock. The practice publisher sends the same `RunInfo` on every tick, which is what keeps the heartbeat alive.

Return `"Run/router lost; no automatic resume"` when the Zenoh session is closed, or when no matching `RunInfo` has arrived for 1.2 seconds. The clock starts at `connect`, so the first 1.2 seconds do not require a second copy. There is no resume. The caller starts a new run.

`Context::status(&mut self, ready, detail, time_ns)`

Publish a [`NodeStatus`](messages.md) on `status/<node>`.

The status sequence starts at 1 and increases by one on every call. The header uses that sequence, `time_ns`, frame `"map"`, and this run's id. `ready` is stored as the status flag. `detail` is the short string the launcher logs. `pid` is `std::process::id()`.

`ready` means the node has a result worth consuming: perception sets it from whether a map was produced, and a loop that is still waiting passes `false`. `detail` is a reason, not a second channel for the map or the path. Those travel on their own topics.

`Context::wait_for_wiring(&mut self, detail)`

Print `<node>: <detail>` once, then until `poll` returns `false`, publish `status(false, detail, 0)` and sleep 100 ms. Return `Ok(())` when the run stops. Return `poll`'s error if the router disappears first.

Planning's `run` and controls' `run` call this with the sentence that says the loop is still empty. Replacing `run` replaces this wait. The function is how an unfinished node stays visible without publishing a stale path or a stale command.

## Dropping a context

When a `Context` is dropped, a controls node tries to publish one more command: a default `DriveCommand` on `command`, with sequence `u64::MAX`, time 0, and frame `"map"`. The default command is zero speed and zero steering. The publish's error is ignored, because `Drop` cannot return it.

Perception and planning do not publish on drop. The launcher still stops the car when the run ends. The extra command is a last attempt from the process that owns the command topic, in case the loop exits while a command is still in flight.

## See also

- [`transport`](transport.md) — the session inside a context
- [`messages`](messages.md) — `RunInfo` and `NodeStatus`
- [`perception-node`](perception-node.md) — a finished loop on top of `Context`
- [`planning-node`](planning-node.md) and [`controls-node`](controls-node.md) — the loops that still call `wait_for_wiring`
