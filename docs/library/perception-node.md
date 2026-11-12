# `perception::node` — The perception message loop

**Source code:** [`src/perception/node.rs`](../../src/perception/node.rs)

This file is the finished message loop. The map still comes from `Perception::update`. The loop subscribes, pairs a pose with the scan from the same moment, calls `update`, and publishes a `MapUpdate`. Planning and controls are meant to follow this shape. Their `run` functions are still empty.

The loop stops on nonsense input, on a sample that arrives out of order, and on a source older than the watchdog allows. A normal empty scan is not one of those. `update` decides what an empty scan does to the map, and the loop publishes either way.

## The loop

`run(ctx)`

Take ownership of a [`Context`](context.md) that is already connected as `"perception"`. Subscribe to `state` with room for 32 samples and to `lidar` with room for 8. Declare a publisher on `map` with Protobuf encoding and drop-on-congestion. Build an `ObservationJoin`, two `Freshness` records, and a `Perception` whose grid is `ctx.info.grid` converted into a plain [`Grid`](grid.md).

Publish a ready status, `"subscribed; waiting for capture-aligned observations"`, then loop while `ctx.poll()` is true.

On each pass, drain `state` and `lidar` with `receive` and a zero timeout. `protocol::state` and `protocol::scan` check the header, the frame, and the numbers. `Freshness::accept` rejects a repeated or rewound sample. Accepted samples go into the join.

When `join.take()` returns a triple, compute the age of the observation against the latest accepted state time. Return `"Matched observation is stale"` when that subtraction fails, or when the age is greater than [`FRESH_NS`](protocol.md). Rewrite the scan header's `frame_id` to `"map"` and keep its capture time.

Call `perception.update`. If that returns `Some`, `validate` the grid and `convert` it into a Protobuf grid. A grid that fails `validate` is an error from `run`, not a `NotReady` map. The clock starts just before `update` and includes that validation and conversion. If more than 300 ms of wall time have passed, return `"Perception exceeded freshness budget"`.

Publish a `MapUpdate` whose `header` and `source` are that rewritten scan header, whose `state` is the paired vehicle state, and whose `availability` is `Ready` when a grid was produced and `NotReady` otherwise. The `grid` field is present only in the ready case. Then publish status: `"map published"` with `ready` true, or `"implement Perception::update"` with `ready` false, timed with the observation.

After the first observation, return `"Perception source expired"` when 300 ms of wall time pass with no newer match. The sleep at the bottom of the loop is 5 ms, so the expiry check runs often enough to see that gap.

`poll` returning `false` ends the function with `Ok(())`. A router loss, a schema problem, or a stale source returns an error and the process exits. The car stops because commands stop arriving, and because a controls context publishes a stop on the way out.

## What the loop asks of `update`

`update` receives an [`Observation`](types.md) and returns `Option<&Grid>`. `Some` is published as a ready map, after `Grid::validate`. `None` is published as `NotReady` with no grid, which is why the car stays stopped while the starter body is still there. The loop does not retry a `None`. The next paired scan is a new call.

The grid inside `Perception` is the one the runner passed to `Perception::new`. `update` is expected to fill that grid and return it. The loop converts whatever reference `update` returns, so the returned grid is the one that gets published.

## See also

- [`synchronizer`](synchronizer.md) — `ObservationJoin`, the pairing step
- [`protocol`](protocol.md) — `state`, `scan`, and `Freshness`
- [`messages`](messages.md) — `MapUpdate`
- [`planning-node`](planning-node.md) — the loop that should look like this one
