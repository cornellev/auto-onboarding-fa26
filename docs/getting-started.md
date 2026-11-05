# Getting started

Budget the first session for setup. Each component lives in
`src/{perception,planning,controls}/`: edit `algorithm.rs` for algorithms and
`node.rs` for planning/controls transport loops. Perception's node is the example.

## Tools

Install Rust 1.88 or newer with rustfmt/Clippy, Git, and a current desktop browser
with WebGL2. Windows uses the MSVC Rust toolchain and Visual Studio C++ build tools.
The combined simulator installer prepares Node 22.22.2, Python, orchestrator,
Zenoh/Protobuf dependencies, and the pinned Zenoh 1.10.2 router. No ROS, Docker,
Protobuf compiler, or Linux VM is needed for the exercises.

## Install simulator and orchestrator together

Use the revisions recorded in `config/services.json`. Orchestrator is available in
[draft PR #1](https://github.com/cornellev/orchestrator/pull/1); the simulator changes
remain local in `services/sim`. The command below uses that local source with the
existing combined installer. Run it from this onboarding checkout on macOS/Linux.
See the [service handoff](service-changes.md) for the local checkout locations.

```sh
EV_SIM_REPO="$PWD/services/sim" bash services/sim/install.sh \
  --dir "$HOME/cev/ev-sim-onboarding" \
  --ref 4a182c7921448d981d0b5d4543268e348ca251af --onboarding \
  --orchestrator-ref 2128e43691bd205ef56c5266f69505f809a8178e \
  --orchestrator-dir "$HOME/cev/orchestrator"
```

Existing modified installations are refused when selecting an exact revision;
use a clean directory. Open new terminals after installation. There is no separate
orchestrator clone, pip, or virtualenv activation step on this path. A GitHub-only
simulator installation cannot fetch the local simulator revision yet.

<details>
<summary>Native Windows setup</summary>

Install Node 22.22.2 and Python 3.12, then use PowerShell from this onboarding
checkout. The local simulator source must be available at `services/sim`; it is
not included in a fresh GitHub clone. Use the revisions from `config/services.json`:

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

The router installer prints its executable location. Use that full path if it is
not on PATH. Start orchestrator with `./.venv/Scripts/python.exe main.py` followed
by the options below. No virtualenv activation is required.

</details>

## Start services

Use separate terminals. From the onboarding checkout, start the router:

```sh
zenohd -c config/zenoh-router.json5
```

Start orchestrator:

```sh
orch --zenoh-profile cev-onboarding-v1 --zenoh-connect tcp/127.0.0.1:7447
```

In the installed simulator checkout:

```sh
npm run dev
```

Open http://localhost:3000 in one desktop browser page. Keep the run stopped.
If orchestrator restarted after the browser connected, refresh the page.
Avoid another controller or simulator publisher on this orchestrator.

## Observe the starter

Back in the onboarding checkout:

```sh
cargo test --locked
cargo run -- setup
cargo run -- observe --duration 5
cargo run -- observe --scenario detour --duration 5
cargo run -- drive-test
```

`setup` registers the schema/profile and installs the complete course resources
through the existing storage API. It does not overwrite edited course resources.
Run it again after restarting orchestrator because registrations are process-local.
The raw simulator bundle importer does not restore the complete scenario/vehicle closure.

Observation shows advancing timestamps and sensor counts while the car stays
stopped. Empty scans are normal on the empty course. Detour provides obstacle hits.
`drive-test` applies at most 0.1 m/s and 0.1 rad for one simulated second, then stops.

Each run gets new simulator topic names, Zenoh keys, and fresh algorithm state.
Ctrl-C stops/finalizes a run. Reconnection never resumes an old autonomy graph.

## Work independently

Offline algorithm fixtures need only Rust:

```sh
cargo run -- perception
cargo run -- planning --scenario detour
```

Transport fixtures need only the local router. They have no actuator connection:

```sh
cargo run -- fixture-publish planning --run planning-practice
# In another terminal:
cargo run -- node planning --run planning-practice
```

Use `perception` or `controls` instead of `planning` to exercise those inputs.
A fixture runs for 60 seconds unless `--duration` is supplied.

For live controls and integration:

```sh
cargo run -- controls --path fixtures/controls/curve.json
cargo run -- autonomous --scenario detour
```

Autonomous launches three independent processes. Unfinished planning/controls wiring
reports what remains and the car stays stopped. A diagnostic run without goal arrival
exits unsuccessfully. Use a short `--duration 5` or Ctrl-C while developing.

To start nodes manually, launch `autonomous --manual-nodes`. It prints the run ID
and waits, paused, for all three nodes:

```sh
cargo run -- node perception --run RUN_ID
cargo run -- node planning --run RUN_ID
cargo run -- node controls --run RUN_ID
```

## Checks and contribution

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo test --test wiring -- --ignored
cargo test --test planning -- --ignored
```

Wiring and algorithm acceptance checks are intentionally ignored until students
implement them. Keep their expectations intact. The provided bootstrap check can
be run independently with a live router:

```sh
cargo test --test wiring three_processes -- --ignored
```

Create a branch, make one focused change, run its fixture/checks, and open a pull
request explaining behavior and evidence. Read `node-events.jsonl`, `updates.csv`,
`summary.json`, and `view.svg` in the printed result directory. Use the browser's
Analysis/Replay for simulator detail.

## Maintainer changes

Regenerate Rust bindings, descriptor bytes, and profile fingerprint together:

```sh
cargo run --example generate_schema
cargo fmt
```

Regenerate courses with the pinned simulator:

```sh
node --experimental-default-type=module scripts/generate-courses.mjs /path/to/ev-sim
```

Commit bundles and matching sidecars together. Course verification allows import
timestamp changes but rejects semantic dependency changes. Each temporary scoped
manifest is resolved before launch and checked against the prepared browser hash.
Keep the legacy external scenario controller disabled; candidate commands must
retain their stamps, sequence checks, and deadlines through ControlRuntime.

Alternate endpoints for isolated testing: `CEV_ORCHESTRATOR_API`,
`CEV_ZENOH_ENDPOINT`, and CLI `--server`. The simulator accepts
`NEXT_PUBLIC_ORCHESTRATOR_API` / `NEXT_PUBLIC_ORCHESTRATOR_WS` at build/start time.
`CEV_API_TOKEN` supplies an existing orchestrator API bearer token.
