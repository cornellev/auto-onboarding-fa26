# Your three challenges

Aim for a clear, tested minimum before exploring. You have one stack, with optional hints, not separate beginner and advanced projects.

## Shared conventions

- World coordinates are `x/y` in meters. Heading is radians, counterclockwise from +x. Positive steering is left.
- Vehicle pose is the rear axle midpoint. In vehicle/sensor coordinates, +x is forward and +y is left.
- An observation contains the scan and the vehicle pose **at scan capture time**. `sensor_in_vehicle` locates the LiDAR relative to the rear axle.
- Grid rows increase with world y; columns with x. Storage is `row * width + column`. `Grid::cell` returns `None` outside the map; `Grid::center` returns cell centers.
- `Path` is a vector of world-coordinate points, beginning near the car and ending near the goal.
- Values leaving your code must be finite. Stop rather than inventing a path or command when inputs are unavailable.

The simulator internally uses Three.js x/z. The adapter handles that boundary. Do not negate y or steering again in your algorithms.

## Perception

**Problem:** Which map cells contain observed obstacles?

Edit `src/perception/algorithm.rs`. `update(&Observation)` updates the map held by your `Perception` struct and returns `Some(&self.map)` when ready. The starter returns `None`.

Minimum: process valid scan endpoints, handle the sensor mounting offset and vehicle heading, mark obstacle cells, ignore out-of-map hits, and tolerate empty scans. Repeated observations should preserve the static map. The runner supplies an initially empty grid.

```sh
cargo run -- perception
cargo run -- perception --fixture fixtures/perception/observation.json
cargo test --test perception -- --ignored
```

The tiny fixture uses a known rotated pose and duplicate points. It is deliberately easy to check by hand. Live runs use real simulator captures.

<details><summary>Optional hints</summary>

A point starts in the sensor frame. First place it in the vehicle frame; then place it in the world. `Pose2::transform` is a general rigid-transform helper. Check one point by hand before processing a full scan. A 90° left rotation sends forward points toward world +y.

`Grid::cell` translates a world point into a column/row. `set_occupied` edits a cell. Neither helper chooses which observations are obstacles or processes the scan for you.

</details>

Explore later: free-space ray tracing, explicit unknown cells, probabilistic occupancy, noise rejection, or map aging. The minimum treats unmarked cells as free; this is optimistic and suitable only for the provided static, visible training courses.

## Planning

**Problem:** Which route can the car follow from its current location to the goal?

Edit `src/planning/algorithm.rs`. Return `Some(path)` for a reachable goal, or `None` when no route is available. Start with BFS if it makes the graph-search idea clearer; A* is the suggested next step. Other methods are welcome.

Minimum: avoid obstacles and map boundaries, account for the car's size, reconstruct a world-coordinate path, and handle unreachable goals. Your implementation must pass both reachable fixtures and the blocked fixture.

```sh
cargo run -- planning --scenario empty
cargo run -- planning --scenario detour
cargo run -- planning --scenario blocked
cargo test --test planning -- --ignored
```

These maps are provided fixtures; your perception code is not called.

<details><summary>Optional hints</summary>

Think of traversable cells as graph nodes. Decide how you will remember visited nodes and how to recover the route after reaching the goal. Rust's standard `VecDeque` and `BinaryHeap` are available; no new dependency is needed.

For A*, distinguish the cost already traveled from the estimated remaining distance. `Grid::clear(point, vehicle.clearance_m())` checks a conservative vehicle clearance circle. `segment_clear` checks between points. `center` converts graph cells back to waypoint positions.

Grid paths can have sharp corners. Use generous clearance, low speed, and a controller with lookahead; a clear grid path does not prove that every turn is feasible for an Ackermann vehicle.

</details>

Explore later: smoothing, better heuristics, turning-radius constraints, RRT*, or a different internal representation. Keep your public inputs/outputs compatible unless you intentionally update the connected infrastructure and tests.

## Controls

**Problem:** Which speed and steering should move the car along the supplied path?

Edit `src/controls/algorithm.rs`. Your struct receives the vehicle specification and can hold progress/state. The starter returns `DriveCommand::STOP`.

Minimum: select a forward target, steer toward it, choose a bounded forward speed, and stop for an empty path. The runner handles goal completion and the external safety limits.

```sh
cargo test --test controls -- --ignored
cargo run -- controls
cargo run -- controls --path fixtures/controls/curve.json
```

These paths are fixtures; neither your perception nor your planner is called. The existing simulator supplies all vehicle dynamics.

<details><summary>Optional hints</summary>

For pure pursuit, express a target ahead of the rear axle in the vehicle frame. If its coordinates are `(x, y)` and squared distance is `L²`, the desired curvature is `2y/L²`. A bicycle steering angle is `atan(wheelbase * curvature)`. You still decide which target to choose, how to advance along the path, how to avoid a target behind you, and what to do near the end.

Use the measured/configured wheelbase, not a magic constant. Too little lookahead can cause oscillation; too much can cut a corner. Test left and right corrections before a full course. Guard a zero target distance.

</details>

Explore later: adaptive lookahead, speed profiles, feedback controllers, or disturbance experiments.

## Show that it works

Run each component independently before integrating. Then navigate empty and detour courses, and show safe failure on blocked. Save one initial result, identify a problem from data, make an improvement, and compare the result.

For the demonstration, explain your data flow, algorithm choices, test evidence, one failure, and one improvement. Write short notes in your PRs or a `RESULTS.md` you create. A successful simulation does not validate physical calibration or localization.

Schedule: setup November 7; perception November 12/14; planning checkpoint November 19; optional work November 21; no required Thanksgiving work; controls/integration December 3; demonstration December 5; submission December 6, 2026.

## Wiring

Perception's node is the worked transport example. It explicitly declares bounded
Zenoh subscribers and a Protobuf publisher. Keep its algorithm unfinished until you
complete the perception challenge.

Implement transport in `src/planning/node.rs` and `src/controls/node.rs`. The
provided `runtime::node::Context` opens the configured session and loads the
immutable RunInfo. Use its `transport.session`, `transport.key(...)`, and generated
`shared::protocol::pb` types. `prost::Message` encodes/decodes binary Protobuf.
`transport::receive` is a bounded-subscription decoding helper in
`shared::transport`. `Context::poll` handles shutdown and run loss.

Planning owns a map subscriber, path publisher, and persistent `Planner`. Every map
update needs a result, including unavailable/no-path updates that clear older paths.
`planning::node::planned_update` invokes the algorithm and validates endpoints,
clearance, and segments before constructing a result with source metadata intact.

Controls owns state/path subscribers, a command publisher, and persistent Controller.
Keep the latest path, including unavailable results. Run once per fresh state at
10 Hz; compute `dt_s` from capture time. `controls::node::controlled_update` creates a
safe command and retains the path's source deadline. Stop for missing or expired paths.
Never stamp an old path as though it came from a new observation. Publish node
status with `Context::status` so mentors can distinguish wiring from algorithms.

```sh
cargo run -- fixture-publish planning --run planning-practice
cargo run -- node planning --run planning-practice
cargo test --test wiring -- --ignored
```

The fixture publisher has no browser/actuator binding. Wiring tests can pass before
algorithms: planning must publish NO_PATH and controls must publish STOP. Once an
algorithm is implemented, valid READY paths and bounded finite commands are accepted.
The separately runnable algorithm tests continue to require actual completion.

<details>
<summary>Wiring hints</summary>

Read perception's calls to `declare_subscriber`, `declare_publisher`, and `put`.
Use ring channels so slow consumers do not accumulate old updates. Keep algorithm
objects outside the receive loop. Validate run/frame headers. Keep all existing
loop/error/shutdown checks and use provided conversion helpers for plain structs.
Publish unavailable results as messages so downstream nodes discard previous output.

</details>
