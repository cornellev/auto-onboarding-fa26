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

Each component keeps its algorithm, node wiring, and provided helpers together:

```text
src/
├── perception/  algorithm.rs, node.rs, synchronizer.rs
├── planning/    algorithm.rs, node.rs, validation.rs
├── controls/    algorithm.rs, node.rs
├── shared/      types, grid, Protobuf codecs/bindings, Zenoh transport
├── runtime/     process lifecycle, simulator/hardware I/O, fixtures, reporting
├── lib.rs
└── main.rs
```

Students edit each component's `algorithm.rs`, plus `planning/node.rs` and
`controls/node.rs` for Zenoh wiring. [Perception's node](src/perception/node.rs) is
the worked transport example. The synchronizer and path validator are provided.
Component fixtures live in `fixtures/<component>/` and acceptance tests in
`tests/<component>/`; existing Cargo commands remain the same.

Provided helpers preserve ordinary Rust algorithm types. Wire types are generated
from `proto/autonomy.proto` into `src/shared/generated/`.

The runtime uses one loopback Zenoh router, orchestrator, and the existing browser
simulator. There is no extra I/O node. Orchestrator converts existing sensor and
actuator interfaces, and also supports generic schema-driven publication and
subscription for other team messages. [Routing explained](docs/routing.md).

The supplied geometry is synthetic training geometry. Physical integration requires
the [maintainer hardware handoff](docs/hardware.md). Visual Protobuf field wiring,
reference autonomy algorithms, and Pi/RP2040 drivers are outside this starter.

See [validation](docs/validation.md) for checks actually performed and remaining gates.
