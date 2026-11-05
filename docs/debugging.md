# Debugging a stopped or confused car

| Symptom | Check |
| --- | --- |
| `perception unavailable` | This is the initial stub. Implement `Perception::update`; start with its fixture. |
| `no path returned` | The planner is unfinished or the goal is unreachable. Try the empty planning fixture first. |
| `controller returned stop` | This is the initial controls stub, or your controller deliberately stopped. |
| No browser tabs | Start ev-sim, open localhost:3000, and return to Simulation. |
| Orchestrator/types error | Start `python main.py`, then refresh the simulator page to sync schemas. Use the pinned revisions. |
| Run conflict | Stop the current run; save or discard unsaved configuration edits. Run one controller and one simulator page. |
| Hash mismatch | The saved course differs from its sidecar. Use `setup` against a fresh simulator data directory; do not edit the sidecar hash to bypass the check. |
| No fresh LiDAR/pose | Check both services, selected course, frame names, and that simulation time advances. Pause/reset requires a fresh controller run. |
| Zero LiDAR points | Expected in empty space. Use observe mode on the detour course to see obstacle returns. |
| Non-planar scan | The course must use the supplied horizontal LiDAR rig. A ground-looking scan is a different perception exercise. |
| Map rotates or mirrors | Check sensor → vehicle → world order. REP-103 +y is left. Do not apply Three.js conversion inside student code. |
| Occupancy is transposed | Storage is `row * width + column`; world y chooses row, x chooses column. |
| Path goes through a wall | Check full segments and vehicle clearance, not only the waypoint cells. |
| Car steers away from path | Positive steering is left. Test the two controls sign cases. |
| Good path, bad tracking | A grid path may require an impossible turn. Try a straight fixture, then gentle curve, then generous-clearance detour. |
| Command limited | Your finite command exceeded speed or steering limits. Compare it with `VehicleSpec`. |
| Stale/slow update | Keep the algorithm below the scan interval where possible. A >300 ms delay stops motion; do not disable the watchdog. |
| Collision count unavailable | The browser lifecycle API supplies compact results, not contact counts. Inspect simulator Analysis/Replay and `simulator_passed`. |
| `simulator_passed: false` after reaching a goal | The browser labels explicit stop/finalize as interrupted (`run-stopped`). Check the runner's `goal_reached` and termination reason; the raw simulator flag alone does not grade your algorithm. |

## Rust errors

Read the first compiler error first. `&T` borrows input data; it does not move it. Store map/controller state on your struct instead of using globals. If you keep a reference to your map while mutating the same struct, the borrow checker may ask you to finish using that reference first.

Run `cargo fmt` after editing and `cargo test --test <component> -- --ignored` for one component at a time. Use a tiny input and print a few intermediate values rather than dumping every LiDAR point.

## Results and timing

CSV uses seconds, meters, and radians. Its timestamp is simulation time. The SVG shows a requested path in blue and actual motion in orange. `compute_us` is wall-clock computation time; it does not advance simulation.

The live browser loop receives asynchronous transport messages. Exact pose and simulation timestamps come from the live `/tf` stream in this simulator revision; speed is estimated from successive poses. Identical seeds and courses make useful repeatable experiments, but browser/network timing can change exact trajectories. A reset starts a new episode; restart the Rust process so maps, search state, and controller state reset together.

If Ctrl-C cannot complete because a service disappeared, the independent watchdog is the last stop mechanism. Check the simulator before restarting. A process exit does not itself prove a physical minicar stopped.

## Distributed nodes

Read `node-events.jsonl` first. An unchanged starter reports perception's unfinished
mapper and planning/controls wiring separately. Each node is an independent process;
a child crash or a changed PID stops the graph. The launcher checks routing/browser
identity every 250 ms; losing it also stops motion at the orchestrator boundary.

- Missing RunInfo: verify router endpoint, run ID, and the active launch.
- Schema mismatch: regenerate artifacts together and rerun setup while stopped.
- No matched observation: check capture timestamps and separate state/LiDAR channels.
- No path after an algorithm change: run the independent planning fixture and check
  endpoints/clearance, including every segment between waypoints.
- Stale command: inspect its original source timestamp. Fresh publication does not
  refresh an old map or plan.
- Disconnected browser: refresh after restarting orchestrator, then start a new run.
- Multiple publishers: close extra simulator pages and terminate duplicate nodes.
  Immediately after a refresh, wait for the old page heartbeat to expire if it is
  still listed as connected; the runner deliberately refuses ambiguous publishers.
- Port in use: leave unrelated services alone and configure the alternate endpoints.

Only generic Protobuf subscribers receive the new WebSocket text messages. Existing
binary `.msg` clients continue using their previous topic catalog and protocol.
The checked-in routing profile is data; changing it does not supply sensor values
or write a planning algorithm.

CSV command columns describe received, limited requests. Actual vehicle speed is
recorded separately; use simulator Replay for commands applied by the plant.
