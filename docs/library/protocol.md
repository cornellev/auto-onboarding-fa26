# `protocol` — Headers, freshness, and conversion

**Source code:** [`src/shared/protocol.rs`](../../src/shared/protocol.rs)

This module turns Protobuf messages into the plain structs in [`types`](types.md), and it checks that a message belongs to this run and is not a repeat. The algorithms should stay on those plain structs. The message loops call the functions here at the boundary, where bytes become a pose or a grid.

`pb` is the generated message module, documented on [its own page](messages.md). The constants and functions below are the checks around those messages.

## Schema and limits

`DESCRIPTOR`

The bundled file descriptor set, `proto/autonomy.binpb`, included as bytes. Orchestrator uses the same schema. Nodes compare a hash of it so a process built from a different schema does not join the run.

`fingerprint()`

Return the hex SHA-256 of `DESCRIPTOR`. `Context::connect` rejects a `RunInfo` whose `schema_hash` differs. `setup_profile` refuses to register a profile whose fingerprint differs from this value.

`FRESH_NS`

`300_000_000`. A map, path, or command whose capture time is older than this, relative to a newer capture, is too old to drive on. The same 300 ms bound is what orchestrator uses when it accepts a command.

`FRESH_MS`

`300`. The same bound, in milliseconds of wall-clock time, used by `Freshness::fresh` for how long ago the message arrived on this machine.

`MAX_DECODE_SIZE`

`8 * 1024 * 1024`, eight megabytes. `decode` refuses a larger buffer.

## Conversion and headers

`convert(value)`

Copy `value` into another type with the same JSON shape. The implementation serializes to JSON and deserializes into the destination. That is how a `shared::types::VehicleState` becomes a `pb::VehicleState`, and how a `shared::grid::Grid` becomes a `pb::Grid`.

The copy succeeds when the field names match and the numbers fit the destination. The plain `Grid` stores weights as `u8`. The Protobuf `Grid` stores them as `u32`. A weight from 0 to 255 survives the trip in either direction. A missing nested Protobuf message becomes JSON `null` and fails the conversion into the plain struct, which always has that field. Callers that read a message check the nested field first and return their own error.

`header(run, sequence, time_ns, frame)`

Build a `pb::Header`. `run_id` is `run`. `sequence` is the caller's counter. `time_ns` is nanoseconds since the run started. `frame_id` is `frame`, a short name such as `"map"` or `"front_lidar_frame"`.

The header is the little record every message carries: which run, which number in the sequence, when the sample was captured, and which frame that time belongs to.

`check_header(header, run, frame)`

Return a clone of the header when it is present, `run_id` equals `run`, `frame_id` equals `frame`, and `sequence` is not zero.

Return `"Missing message header"` when the `Option` is empty. Return `"Wrong run, frame, or zero sequence"` when the run id, the frame, or a zero sequence fails the check. A header from another run is rejected here, which is why each run has its own topic prefix as well.

The frame strings the helpers expect are `"map"` for state, maps, paths, commands, run info, and node status, and `"front_lidar_frame"` for a LiDAR scan. Perception copies the scan header onto the map and rewrites that frame id to `"map"` before publishing.

`state(update, run)`

Read a `pb::StateUpdate`. Check the header against `run` and the frame `"map"`. Convert `update.state` into a `VehicleState`. Return the header and that state.

Return `"Missing vehicle state"` when `state` is absent. Return `"Invalid vehicle state"` when the pose or the speed is not finite. Header failures propagate from `check_header`.

`scan(update, run)`

Read a `pb::LidarUpdate`. Check the header against `run` and the frame `"front_lidar_frame"`. Convert `update.scan` into a `LidarScan`, then run `Observation::validate` on a temporary observation that carries that scan and a default vehicle state. The temporary state is not returned. The check is the scan: the mount and the points have to be finite, and the scan cannot hold more than 100,000 points.

Return `"Missing scan"` when `scan` is absent. Return the header and the `LidarScan` on success.

`decode(bytes)`

Decode one Protobuf message from `bytes`. The destination type must implement `prost::Message` and `Default`. Return `"Protobuf message exceeds limit"` when `bytes.len()` is greater than `MAX_DECODE_SIZE`. A buffer that is not a valid message returns the decoder's error.

`Transport::publish` encodes with `encode_to_vec` and does not call `decode`. `receive` calls `decode` on each payload.

## Freshness

*class* `shared::protocol::Freshness`

Remembers the last sequence number and capture time that `accept` took, so a repeat or a rewind can be rejected. `Default` is a sequence of zero, a time of zero, and no arrival time, which means nothing has been accepted yet.

The fields are private. The only way to move the record forward is `accept`.

`Freshness::accept(&mut self, header)`

Take `header` when it is strictly newer than the last accepted header. The sequence must be greater than the stored sequence. After the first success, `time_ns` must be greater than or equal to the stored capture time. A sequence of zero fails the first call, because the stored sequence starts at zero and `check_header` already rejects zero.

On success, store the sequence, the capture time, and `Instant::now()` as the arrival time, and return `Ok(())`.

Return `"Repeated/out-of-order update or reset; start a fresh run"` when the sequence does not move forward, or when the capture clock moves backward after a message has been accepted. An equal capture time with a higher sequence is accepted. The function does not clear itself. The caller starts a new run instead of continuing.

`Freshness::time_ns(&self)`

Return the capture time, in nanoseconds since the run started, of the last header `accept` took. Before the first success this is zero.

`Freshness::fresh(&self, now_ns)`

Return `true` when a header has been accepted, that acceptance was at most `FRESH_MS` milliseconds ago on this machine's clock, and `now_ns.checked_sub(stored time)` is present and at most `FRESH_NS`.

Both clocks have to agree. A message that arrived long ago is stale even if its capture time is close to `now_ns`. A capture time in the future relative to `now_ns` is stale, because the subtraction fails. A `Freshness` that has never accepted a header is not fresh.

Perception uses the capture half of this idea directly: a matched observation is rejected when the latest state time is less than the observation time, or more than `FRESH_NS` ahead of it. The simulator connection uses `fresh` on both the state record and the scan record before it yields an observation.

## See also

- [`messages`](messages.md) — the message structs these functions read
- [`transport`](transport.md) — `publish` and `receive`
- [`context`](context.md) — the schema check at node startup
- [`types`](types.md) — the plain structs `convert` produces
