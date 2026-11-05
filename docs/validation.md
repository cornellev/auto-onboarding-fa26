# Starter validation

Validated locally on macOS on November 4, 2026, against the simulator and orchestrator revisions pinned in `Cargo.toml`.

- `cargo fmt --check`, Clippy with warnings denied, and the default infrastructure/fixture suite.
- Generated all three courses using ev-sim's validator/exporter and installed their full authoring dependencies through `setup`.
- Live browser smoke test: stationary observation, bounded driving, stop, then a fresh stationary run.
- Live watchdog test: move under candidate control, stop sending commands while continuing to receive observations, and verify the simulator stops the car independently.
- Detour observation: received 19 LiDAR obstacle points per scan while the vehicle remained stationary.

The normal test suite deliberately ignores student acceptance and live-browser tests. The two infrastructure live tests were explicitly enabled during this validation. No completed student algorithms were used.

Windows/Linux native execution and physical Pi/RP2040 integration were not exercised locally. CI is configured for the three desktop platforms. Physical validation requires the maintainer handoff in `hardware.md`; the supplied dimensions are synthetic training geometry.
