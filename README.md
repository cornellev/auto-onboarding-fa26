# CEV freshman autonomy onboarding

Build perception, planning, and controls as three independent Rust programs talking
through **Zenoh and Protobuf**. One Cargo package contains the three nodes, shared
helpers, fixtures, and safe browser-simulator launch/reporting infrastructure.

Start with [getting started](docs/getting-started.md), then [challenges](docs/challenges.md)
and [debugging](docs/debugging.md). The untouched starter compiles, passes its default
tests, receives LiDAR/pose, and stays stopped. Algorithms are intentionally unfinished.

```sh
cargo test --locked
cargo run -- setup
cargo run -- observe
cargo run -- autonomous --scenario detour
```

Students edit `src/student/{perception,planning,controls}.rs` for algorithms, and
`src/nodes/{planning,controls}.rs` for Zenoh wiring. `src/nodes/perception.rs` is the
worked transport example. Provided helpers preserve ordinary Rust algorithm types;
wire types are generated from `proto/autonomy.proto`.

The runtime uses one loopback Zenoh router, orchestrator, and the existing browser
simulator. There is no extra I/O node. Orchestrator converts existing sensor and
actuator interfaces, and also supports generic schema-driven publication and
subscription for other team messages. [Routing explained](docs/routing.md).

The supplied geometry is synthetic training geometry. Physical integration requires
the [maintainer hardware handoff](docs/hardware.md). Visual Protobuf field wiring,
reference autonomy algorithms, and Pi/RP2040 drivers are outside this starter.

See [validation](docs/validation.md) for checks actually performed and remaining gates.
