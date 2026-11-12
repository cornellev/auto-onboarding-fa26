# Getting started

The first session is mostly setup. What you're building is in the [project writeup](project.md). The code for each piece is in `src/perception`, `src/planning`, and `src/controls`. The algorithm goes in `algorithm.rs`. Planning and controls also have a short message loop in `node.rs`, which is still empty. Perception's `node.rs` is a finished example of that loop.

## Tools

You need Rust 1.88 or newer, including rustfmt and Clippy, Git, and a current desktop browser with WebGL2. On Windows, use the MSVC Rust toolchain and the Visual Studio C++ build tools. The simulator installer brings Node 22.22.2, Python, orchestrator, the Zenoh and Protobuf libraries, and the Zenoh 1.10.2 router. You don't need ROS, Docker, a Protobuf compiler, or a Linux VM for these exercises.

## Install the simulator and orchestrator

Run the following script:

```sh
curl -fsSL https://raw.githubusercontent.com/cornellev/ev-sim/main/install.sh | bash
```

Then, `cd` into the simulator folder and run:

```sh
npm run build
```

## Terminology

When we refer to this repo, we mean the base directory containing the README.md!

## Start the services

Three terminals. From this repo (the base directory containing the README.md), begin the Zenoh router:

```sh
zenohd -c config/zenoh-router.json5
```

Then, assuming you've been able to install orchestrator (it gets installed with sim):

```sh
orch --zenoh-profile cev-onboarding-v1 --zenoh-connect tcp/127.0.0.1:7447
```

And, in the simulator that you just installed before, run this command to start it up!

```sh
npm start
```

Open http://localhost:3000 in one browser tab, and leave the run stopped. If you restart orchestrator after the page connected, refresh the page. Don't start a second controller, or a second simulator tab, on this orchestrator.

## Watch the starter

Let's go and verify that everything is installed!

Back in this repo, run the following commands to build and run the starter code. You should have the simulator open in your browser in the background.

```sh
cargo test --locked
cargo run -- setup
cargo run -- observe --duration 5
cargo run -- drive-test
```

`setup` registers the message types and checks that the simulator already has the run manifest named `onboarding`. It does not copy a world into the simulator. Run it again after you restart orchestrator, because that registration dies with the process.

`observe` should show the timestamps moving and the sensor counts updating, while the car stays still. The square has cones, so you should see hits. `drive-test` creeps at no more than 0.1 m/s and 0.1 rad for one simulated second, then stops.

Every run gets a new router id and a fresh copy of your algorithm's state. The simulator topics stay the ones on the `onboarding` manifest. 

For all of the runs, Ctrl-C stops the run and writes the results. Connecting again doesn't resume the old run, it'll start a new one.

## Work on one piece

The algorithm fixtures only need Rust:

```sh
cargo run -- perception
cargo run -- planning --scenario detour
```

The message-loop fixtures only need the local router. The car in the browser isn't part of them:\
```sh
cargo run -- fixture-publish planning --run planning-practice
# In another terminal:
cargo run -- node planning --run planning-practice
```

Swap `perception` or `controls` in for `planning` to practice those. A fixture runs for 60 seconds unless you pass `--duration`.

When you want the car in the loop:

```sh
cargo run -- controls --path fixtures/controls/curve.json
cargo run -- autonomous
```

`autonomous` starts the three programs. Until the planning and controls loops are written, it tells you what's missing and the car stays stopped. Missing the goal exits with an error. Use `--duration 5`, or Ctrl-C, while you're still working.

To start the three programs yourself, run `autonomous --manual-nodes`. It prints a run id and waits, paused, until all three are up:

```sh
cargo run -- node perception --run RUN_ID
cargo run -- node planning --run RUN_ID
cargo run -- node controls --run RUN_ID
```

## Before a pull request

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo test --test wiring -- --ignored
cargo test --test planning -- --ignored
```

`cargo test --locked` stays green before the algorithms exist and after they do. The wiring and algorithm tests stay ignored until you ask for them. With the router running, the wiring run accepts an unavailable map or a ready one, no path or a real path, and a stop or a real command. `three_processes` is part of that run. The perception, planning, and controls tests are the ones that want the real algorithms.

Make a branch, keep the change focused, run the fixture or test that covers it, and open a pull request that says what changed and what you saw. The command prints a result folder. Look at `node-events.jsonl`, `updates.csv`, `summary.json`, and `view.svg` in there, and use Analysis and Replay in the browser for the simulator's own view. The [project writeup](project.md) says what each part is asking for.

If you're confused about the code provided to you, you can look at the documentation in [the library](library/overview.md).