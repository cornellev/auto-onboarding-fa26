# Getting started

The first session is the occupancy grid. What you're building is in the [project writeup](project.md). The code for each piece is in `src/perception`, `src/planning`, and `src/controls`. The algorithm goes in `algorithm.rs`. Planning and controls also have a short message loop in `node.rs`, which is still empty. Perception's `node.rs` is a finished example of that loop.

## Tools

You need Rust 1.88 or newer, including rustfmt and Clippy, Git, and a current desktop browser. On Windows, use the MSVC Rust toolchain and the Visual Studio C++ build tools. You don't need ROS, Docker, a Protobuf compiler, or a Linux VM for these exercises.

## Simulator

Coming this weekend!

## Terminology

When we refer to this repo, we mean the base directory containing the README.md!

## Watch the starter

From this repo:

```sh
cargo test --locked
cargo run -- perception
```

A green `cargo test --locked` means the starter builds. It does not mean perception, planning, or controls work. `perception` reads `fixtures/perception/observation.json`, runs `Perception::update`, and writes a result folder. Open `view.svg` in that folder in your browser. Black squares are occupied cells. The fixture is a short rotated scan, so a working map is two cells, not the cone course.

The cell check is:

```sh
cargo test --test perception -- --ignored
```

`setup`, `observe`, `drive-test`, and `autonomous` need the simulator. They arrive with this weekend's install.

## Work on one piece

Perception is the piece for now:

```sh
cargo run -- perception
cargo run -- perception --fixture fixtures/perception/observation.json
```

The planning maps are also offline, when you get there:

```sh
cargo run -- planning --scenario empty
cargo run -- planning --scenario detour
cargo run -- planning --scenario blocked
```

## Before a pull request

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo test --test perception -- --ignored
cargo test --test planning -- --ignored
cargo test --test controls -- --ignored
cargo test --test wiring -- --ignored
```

`cargo test --locked` stays green before the algorithms exist and after they do. The wiring and algorithm tests stay ignored until you ask for them. The perception, planning, and controls tests are the ones that want the real algorithms. The wiring test needs the router from this weekend's install.

Make a branch, keep the change focused, run the fixture or test that covers it, and open a pull request that says what changed and what you saw. The command prints a result folder. Look at `view.svg` in there. The [project writeup](project.md) says what each part is asking for.

If you're confused about the code provided to you, you can look at the documentation in [the library](library/overview.md).