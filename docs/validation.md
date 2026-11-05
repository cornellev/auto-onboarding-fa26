# Migration validation

Validated locally on macOS on November 5, 2026. Service revisions and dependency
pins are recorded in `config/services.json` and Cargo.lock.

## Passed

- Rust formatting, Clippy with warnings denied, and default infrastructure/fixture
  tests. Student wiring, algorithm, and browser acceptance checks remain ignored by
  default. No completed mapper, planner, or controller was supplied.
- Reproducible generated Rust bindings, descriptor bytes, and profile fingerprint.
- API authorization regression: orchestrator tokens are never forwarded to simulator
  storage requests. The loopback check is included in transport CI.
- 57 orchestrator tests, including legacy binary compatibility, generic descriptors,
  unrelated schemas, exact 64-bit values, stale plans, invalid commands, bounded
  queues, no-path invalidation, and the bounded drive-test cutoff.
- Generic round trip: JavaScript Client -> orchestrator -> binary Protobuf over a
  real Zenoh 1.10.2 router -> Rust, followed by a Rust response back to JavaScript.
- The self-contained CI transport script, using fresh loopback services and three
  distinct node processes. Perception publishes NOT_READY before its algorithm is
  implemented; planning/controls report their unfinished wiring and stay stopped.
- Real browser scan/pose reception while stationary; bounded one-second driving;
  stop followed by a new stationary run. The detour course produced 19 LiDAR points
  per scan while stationary.
- Live command-loss stopping while observation reception continued, using the
  separately enabled integration test.
- Focused simulator transport, installer, sensor protocol, and browser-safe hash
  tests. Lint completed with zero errors and three existing React-hook warnings.
- A fresh native macOS combined installation from local committed source revisions,
  including both services, pinned dependencies, checksum-verified router, and orch
  command. Test installation used its own directories and command PATH.
- Browser-safe catalog/calibration hashes match the supplied course's previous
  hashes. The two Node-only hash imports prevented fresh browser bundling and were
  replaced with the already-used browser-safe SHA-256 library.

## Baseline simulator failures

The full simulator suite was run. These five failures were reproduced against a
clean archive of its untouched base revision:

- Headless characterization fixture steering/trajectory mismatch.
- Plan-view control arcs/localization snapshot.
- Plugin range-image devices after parent updates.
- State-sensor mounts after parent updates.
- LiDAR hierarchy row identity.

Their fixtures and expectations were preserved. One additional piped-installer
fixture failure under the full parallel run was corrected by ending its test input
at the intentional exit point; focused installer checks pass. This does not change
installer behavior or weaken student acceptance criteria.

## Remaining release gates

- Orchestrator is published in [draft PR #1](https://github.com/cornellev/orchestrator/pull/1).
  The simulator remains local by request. Remote combined installation and transport
  CI cannot fetch that simulator revision; use the local integration check instead.
- Hosted orchestrator Linux Python CI passes all 57 tests. Windows exposes a
  two-second real-socket timeout in the unchanged client lifecycle test; matrix
  fail-fast cancels macOS. The legacy cross-language job fails on Node 20 module
  loading in the unchanged JavaScript client. These checks remain enabled.
- Native Windows/Linux installation and browser execution have not been exercised
  locally. Cross-platform Rust and transport jobs are configured for all three OSes.
- Empty/detour autonomous goal arrival requires completed student algorithms.
- Pi 5/RP2040 integration, measured geometry, localization, braking, and physical
  emergency stopping require the separate maintainer hardware handoff.
- Visual Protobuf field wiring and driver/firmware implementation are deferred.

SFLog/Replay remains the source for applied simulator commands and collision detail.
Collision counts remain unavailable when that evidence is not exposed; reports do
not fabricate a zero count or treat a manual run stop as simulator success.
