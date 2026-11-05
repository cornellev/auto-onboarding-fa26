# Getting started

Budget a first work session for setup. Later work should be mostly in three Rust files.

## Install tools

- Rust **1.88 or newer** with Cargo, rustfmt, and Clippy. Use the official Rust installer at <https://rustup.rs/>. On Windows, select the MSVC toolchain and install the Visual Studio C++ build tools if prompted.
- Git and curl. The simulator installer prepares Node.js **22.22.2**, the orchestrator's Python environment, and service dependencies.
- A current browser with WebGL2. No ROS, Zenoh, Docker, GPU toolkit, or Linux VM is required for these courses.

The Rust client dependencies are pinned to the tested simulator/orchestrator revisions below. The first Cargo build needs internet access; later builds can use the cache. Commit `Cargo.lock` when changing dependencies.

## Install the simulator and orchestrator together

On macOS/Linux, run the simulator's combined installer from the directory where you want its `ev-sim` checkout:

```sh
curl -fsSL https://raw.githubusercontent.com/cornellev/ev-sim/main/install.sh \
  | bash -s -- --ref 6c095b97ff1b5eed90777ccd15a53726b2ce86e2 \
    --orchestrator --orchestrator-dir "$HOME/cev/orchestrator"
```

This installs ev-sim in `./ev-sim`, installs the orchestrator in `~/cev/orchestrator`, creates its virtual environment, installs both services' dependencies, and installs the `orch` command. Accept the Node.js setup prompt if needed. Marketplace features are not needed for onboarding. `--orchestrator` explicitly includes orch even when the installer has no interactive terminal.

The installer's `--ref` selects the simulator revision only. Keep the installed orchestrator at the tested revision too:

```sh
git -C "$HOME/cev/orchestrator" checkout c6fe352fd5cdee3b8f70a5695b1b5433e118f61b
```

Open new terminals after installation so `orch` and any installed Node.js version are on PATH. There is no separate orchestrator clone, pip installation, or virtualenv activation step on this path.

<details>
<summary>Native Windows setup</summary>

The current Bash installer assumes Unix `.venv/bin/python` paths, so Git Bash alone does not provide a native Windows installation. To keep the services native, install Node.js 22.22.2 and Python 3.10–3.13, then use PowerShell:

```sh
git clone https://github.com/cornellev/ev-sim.git
git -C ev-sim checkout 6c095b97ff1b5eed90777ccd15a53726b2ce86e2
cd ev-sim
npm ci
cd ..
git clone https://github.com/cornellev/orchestrator.git
git -C orchestrator checkout c6fe352fd5cdee3b8f70a5695b1b5433e118f61b
cd orchestrator
python -m venv .venv
./.venv/Scripts/python.exe -m pip install -r requirements.txt
```

Start the orchestrator there with `./.venv/Scripts/python.exe main.py` instead of `orch`; start the simulator in its checkout with `npm run dev`. No virtualenv activation is required.

</details>

## Start the two existing services

**Terminal 1: orchestrator**

```sh
orch
```

Leave the default standalone mode: `ROS_ENABLED` and `ZENOH_ENABLED` are false. It listens locally on ports **8080** and **8090**.

**Terminal 2: simulator** (from the installation's parent directory)

```sh
cd ev-sim
npm run dev
```

Open <http://localhost:3000>. Keep exactly one simulator page and one onboarding controller active. Do not run another autonomy stack on the same orchestrator. If you started the simulator before the orchestrator, refresh the simulator page so it connects and syncs message definitions.

Leave the page on Simulation with its run stopped. Save or discard other workspace edits. The onboarding `setup` command below installs the three course bundles' environments, vehicle, scenarios, and manifests through the existing storage API. The browser's bare run-bundle import does not restore the full scenario/vehicle closure, so use this command for initial setup.

Setup creates only missing resources, then verifies the resolved course hashes. It never overwrites existing resources. The saved course IDs are `onboarding-empty`, `onboarding-detour`, and `onboarding-blocked`.

## Run your checkout

**Terminal 3: onboarding repository**

```sh
cargo test --locked
cargo run -- setup
cargo run -- observe
```

You should see a stationary vehicle, advancing simulation timestamps, and received point counts. The empty course can legitimately produce zero obstacle returns. `cargo run -- observe --scenario detour` shows returns from the obstacle. Ctrl-C stops the run; `--duration 5` runs a short check automatically.

```sh
cargo run -- perception
cargo run -- planning --scenario detour
cargo run -- controls --path fixtures/path-curve.json
cargo run -- drive-test
```

`drive-test` is an isolated, one-second simulation command test. It is not an autonomous controller. Controls fixtures only run on the empty course. Use autonomous mode for obstacle avoidance.

Unfinished fixture modes still write a view and explain which function needs work. A live controls/autonomous run that never reaches the goal exits unsuccessfully; a blocked goal is expected to remain stopped. Stop that diagnostic run with Ctrl-C rather than waiting for its 90-second bound.

The latest result directory is printed by every completed command. Open its `view.svg` in a browser, inspect `summary.json`, or graph `updates.csv`. The simulator records its own run for Analysis/Replay when logging is available.

## First contribution

1. Make a branch: `git switch -c your-name/perception`.
2. Change one student module. Keep the provided function signature.
3. Run that fixture and its acceptance test. Explain a result, including failures.
4. Run `cargo fmt`, `cargo clippy --locked --all-targets -- -D warnings`, and `cargo test --locked`.
5. Commit the change and open a GitHub pull request with what changed, why, and the test evidence. Ask another freshman or mentor to review it.

Use `cargo test --test perception -- --ignored` to enable perception's acceptance checks. The normal test suite must remain green before any algorithms are written. Do not delete or weaken acceptance checks to make incomplete work appear finished.

## Maintainer course updates

The checked-in bundles are generated with ev-sim's own storage, normalization, validation, and export code. They contain no custom simulator implementation.

```sh
node --experimental-default-type=module scripts/generate-courses.mjs /path/to/ev-sim
```

Use the pinned simulator revision. Commit updated bundles and matching JSON sidecars together. Run `setup` against a fresh simulator data directory, then rerun the live smoke test. The adapter verifies manifest/dependency identities, then matches the prepared run hash to the just-validated run. Import timestamp changes alone are allowed; altered geometry or watchdog settings are rejected.

The legacy scenario `external-ros` controller loses command stamps and submits reference commands in this revision. These courses leave that route controller disabled and unmatched. Candidate `/controls/command` packets go directly to `ControlRuntime`, retaining deadlines, sequence checks, and the watchdog. Do not wire that topic into the scenario controller.

Live checks need the services and one open simulator page:

```sh
cargo test --test integration browser_observation_drive_stop_and_fresh_run -- --ignored --test-threads=1
cargo test --test integration candidate_watchdog_stops_when_commands_stop -- --ignored --test-threads=1
```

The other ignored integration checks require completed student algorithms. CI does not launch a browser or real hardware.
