# Getting started

The first session is the occupancy grid. What you're building is in the [project writeup](project.md). The code for each piece is in `src/perception`, `src/planning`, and `src/controls`. The algorithm goes in `algorithm.rs`. Planning and controls also have a short message loop in `node.rs`, which is still empty. Perception's `node.rs` is a finished example of that loop.

## Tools

You need Rust 1.88 or newer, including rustfmt and Clippy, Git, and a current desktop browser. On Windows, use the MSVC Rust toolchain and the Visual Studio C++ build tools. You don't need ROS, Docker, a Protobuf compiler, or a Linux VM for these exercises.

## Simulator

Offline commands (`perception`, and `planning` from a saved map) do not need the simulator. `observe`, `drive-test`, `controls` on the live course, and `autonomous` do.

From the simulator checkout:

```sh
npm run dev
```

The simulator starts the Zenoh router. The first launch can download it. Wait until stderr prints:

```text
[zenoh-router] ready tcp/127.0.0.1:7447 ws://127.0.0.1:10000/ owner
```

Rust in this repo connects to that TCP address. The page uses the WebSocket. Leave `CEV_ZENOH_ENDPOINT` unset. Set it only when the simulator was started with `CEV_SIM_ZENOH_TCP_PORT` on another port, and then set it to `tcp/127.0.0.1:<that port>`. `CEV_SIM_ZENOH_ROUTER=off`, or a simulator configured for some other router address, does not open this port. Do not start `zenohd`, and do not start `config/zenoh-router.json5`. Quitting the simulator stops the router it started.

Open the URL it prints, usually http://localhost:3000, in a normal desktop browser window. Click the simulator page you want to use and leave its run stopped. Onboarding uses the server's most recently interacted-with connected page; other connected tabs or embedded previews do not block launch. If another program already owns port 3000, start the simulator with `PORT=3002 npm run dev` and pass `--server http://localhost:3002` on the commands below.

The simulator has to already contain a saved run named `onboarding`. From this repo:

```sh
cargo run -- setup
```

That checks for the manifest. It does not install a world. If the manifest is missing, create the onboarding run in the simulator and run `setup` again.

With the simulator still running and the selected page stopped:

```sh
cargo run -- observe --duration 5
```

`observe` prints a pose and a lidar count. `autonomous` is in the [project writeup](project.md), after the three pieces work.

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

`setup`, `observe`, `drive-test`, and `autonomous` need the simulator stack above.

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

`cargo test --locked` stays green before the algorithms exist and after they do. The wiring and algorithm tests stay ignored until you ask for them. The perception, planning, and controls tests are the ones that want the real algorithms. The wiring test needs the simulator running, so `tcp/127.0.0.1:7447` is open. It does not need the browser tab.

Make a branch, keep the change focused, run the fixture or test that covers it, and open a pull request that says what changed and what you saw. The command prints a result folder. Look at `view.svg` in there. The [project writeup](project.md) says what each part is asking for.

If you're confused about the code provided to you, you can look at the documentation in [the library](library/overview.md).