# Getting started

The first session is mostly setup. What you're building is in the [project writeup](project.md). The code for each piece is in `src/perception`, `src/planning`, and `src/controls`. The algorithm goes in `algorithm.rs`. Planning and controls also have a short message loop in `node.rs`, which is still empty. Perception's `node.rs` is a finished example of that loop.

## Tools

You need Rust 1.88 or newer, including rustfmt and Clippy, Git, and a current desktop browser with WebGL2. On Windows, use the MSVC Rust toolchain and the Visual Studio C++ build tools. The simulator installer brings Node 22.22.2, Python, orchestrator, the Zenoh and Protobuf libraries, and the Zenoh 1.10.2 router. You don't need ROS, Docker, a Protobuf compiler, or a Linux VM for these exercises.

## Install the simulator and orchestrator

The revisions are in `config/services.json`. The simulator changes live in the local `services/sim` checkout, so run the installer from this repo, on macOS or Linux.

```sh
EV_SIM_REPO="$PWD/services/sim" bash services/sim/install.sh \
  --dir "$HOME/cev/ev-sim-onboarding" \
  --ref 4a182c7921448d981d0b5d4543268e348ca251af --onboarding \
  --orchestrator-ref 2128e43691bd205ef56c5266f69505f809a8178e \
  --orchestrator-dir "$HOME/cev/orchestrator"
```

If a directory already has local changes, the installer refuses to check out an exact revision. Give it a clean directory. Open new terminals after it finishes, so they see the new commands. On this path you don't clone orchestrator yourself, and you don't activate a virtualenv. A plain GitHub install of the simulator can't fetch the local simulator revision.

<details>
<summary>Native Windows setup</summary>

Install Node 22.22.2 and Python 3.12, then use PowerShell from this checkout. The simulator source has to be at `services/sim`. A fresh GitHub clone of this repo doesn't include it. The revisions are the same ones in `config/services.json`:

```powershell
git clone ./services/sim ev-sim
git -C ev-sim checkout 4a182c7921448d981d0b5d4543268e348ca251af
cd ev-sim
npm ci
cd ..
git clone https://github.com/cornellev/orchestrator.git
git -C orchestrator checkout 2128e43691bd205ef56c5266f69505f809a8178e
cd orchestrator
py -3.12 -m venv .venv
./.venv/Scripts/python.exe -m pip install -r requirements.txt -r requirements-protobuf.txt
./.venv/Scripts/python.exe scripts/install_zenoh.py
```

The router installer prints where it put `zenohd`. Use that full path if it isn't on PATH. Start orchestrator with `./.venv/Scripts/python.exe main.py` and the options below. You don't have to activate the virtualenv.

</details>

## Start the services

Three terminals. From this checkout, the router:

```sh
zenohd -c config/zenoh-router.json5
```

Orchestrator:

```sh
orch --zenoh-profile cev-onboarding-v1 --zenoh-connect tcp/127.0.0.1:7447
```

And in the simulator checkout you just installed:

```sh
npm run dev
```

Open http://localhost:3000 in one browser tab, and leave the run stopped. If you restart orchestrator after the page connected, refresh the page. Don't start a second controller, or a second simulator tab, on this orchestrator.

## Watch the starter

Back in this checkout:

```sh
cargo test --locked
cargo run -- setup
cargo run -- observe --duration 5
cargo run -- observe --scenario detour --duration 5
cargo run -- drive-test
```

`setup` registers the message types and installs the courses. It leaves a course alone if you've already edited it. Run it again after you restart orchestrator, because that registration dies with the process. Importing the raw simulator bundle is not a substitute. It doesn't bring back the full course.

`observe` should show the timestamps moving and the sensor counts updating, while the car stays still. An empty scan on the empty course is normal. The detour course has obstacles, so you should see hits. `drive-test` creeps at no more than 0.1 m/s and 0.1 rad for one simulated second, then stops.

Every run gets new topic names and a fresh copy of your algorithm's state. Ctrl-C stops the run and writes the results. Connecting again doesn't resume the old run. Start a new one.

## Work on one piece

The algorithm fixtures only need Rust:

```sh
cargo run -- perception
cargo run -- planning --scenario detour
```

The message-loop fixtures only need the local router. The car in the browser isn't part of them:

```sh
cargo run -- fixture-publish planning --run planning-practice
# In another terminal:
cargo run -- node planning --run planning-practice
```

Swap `perception` or `controls` in for `planning` to practice those. A fixture runs for 60 seconds unless you pass `--duration`.

When you want the car in the loop:

```sh
cargo run -- controls --path fixtures/controls/curve.json
cargo run -- autonomous --scenario detour
```

`autonomous` starts the three programs. Until the planning and controls loops are written, it tells you what's missing and the car stays stopped. On empty and detour, missing the goal exits with an error. On blocked, stopping without hitting the wall exits cleanly. Use `--duration 5`, or Ctrl-C, while you're still working.

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
