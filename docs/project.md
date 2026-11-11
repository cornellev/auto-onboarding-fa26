# Autonomy onboarding

Over the next month you're building the software that lets a minicar drive from a start to a goal without hitting what's in the way.

That breaks into three questions. How does the car tell where the obstacles are? How does it pick a route? How does it steer and choose a speed so it actually follows that route? You'll answer them in Rust, and you'll check the answers in the simulator before any of this goes on a real car. We're not going to hand you the algorithms. We'll suggest a place to start, and the work sessions are where we walk through the ideas. Get a basic version working before you wander off into something fancier.

The code you already have compiles, talks to the simulator, and holds the car still. The three empty functions are yours: `Perception::update`, `Planner::plan`, and `Controller::command`.

If the tools aren't installed, do [getting started](getting-started.md) first. November 7 is that, plus Git and Rust. November 12 we introduce perception, and November 14 is for building it. November 19 is the planning introduction and a checkpoint, so we'll look at what you have. November 21 is a planning work session. There's no required session over the November 25–29 break. December 3 is controls, and putting the three pieces together. December 5 you show it. December 6 is the deadline.

## Your plan

Before you disappear into the code, write a short plan for each of the three parts. It doesn't need to be a design document. We want to see that you've thought about the problem. You can change it later, and we'll tell you if the basic version looks unrealistic.

For perception, planning, and controls:

1. What do you want to try? Our suggestion is fine. So is something else, if you can say why.
2. If the semester gets messy, what's the smallest version you'd still be happy to finish?
3. If that works and you have time left, what do you want to poke at?
4. How will you know it's working?

Send it with the [onboarding plan form](https://docs.google.com/forms/d/e/1FAIpQLSeEyxzGgWWwgnLyo-RrHzSE_mUFvAUE2RkKRe8w2cJUkc6xzA/viewform?usp=header).

## Perception

Where are the obstacles?

The basic version is an occupancy grid. Chop the ground into little squares, a few centimeters on a side. Each cell holds a weight from 0 to 255. `set_occupied` writes 255. A weight of 128 or more is blocked. You can also track "we haven't looked here," but skip that for now.

We give you the grid. You turn the LiDAR points into occupied cells.

A LiDAR point starts where the sensor is looking: x forward, y to the left, in meters. The car's pose is the middle of the rear axle, in the same kind of coordinates, and the heading is in radians, counterclockwise from +x. `sensor_in_vehicle` says where the LiDAR is bolted on, relative to that axle. `Pose2::transform` does one move: rotate by the pose's heading, then shift by its position. Use it twice. First from the sensor into the car, then from the car into the world. If the car is facing 90° left, a point that was straight ahead of the sensor should land toward +y in the world. Check one point from the fixture on paper before you loop over a whole scan. The simulator keeps its own axis convention, and that's already converted before you see the points, so leave +y as left.

`Perception` in `src/perception/algorithm.rs` holds the map. `update` is handed an `Observation`, which is the pose, the scan points, and the sensor mount, and it should return `Some(&self.map)` once you have a map. Right now it returns `None`, which is why the car never moves. The runner builds an empty grid and passes it to `Perception::new`. A new cell is 0, and 0 is free. That's reasonable on these courses, where the obstacles sit still and the car can see them. It would be a bad assumption on a course the car hasn't fully looked at.

The browser's vehicle origin is `base_link`. `rear_axle_in_base.x` is −0.1 m, so a base_link pose at x = 1.1 is the planning fixture's start at x = 1.0, and the bundle's finish waypoint at x = 8.1 is the course goal `(8, 0)`. The pose you receive is already the rear axle, with +x forward and +y left. The bundle itself is a different frame: Y is up, and heading is `(cos yaw, 0, −sin yaw)`. The nodes have already done that conversion. `scenarios/detour.json` and `scenarios/blocked.json` store an empty grid. The obstacles for `cargo run -- planning` are in `fixtures/planning`. The live obstacle arrives as LiDAR.

`Grid::cell` gives you the column and row of a world point, or `None` if the point is off the map. Columns follow x, rows follow y, and the cell is stored at `row * width + column`. `set_occupied` writes 255 into one cell. `set_weight` writes a belief from 0 to 255, and 0 clears the cell. Both ignore a column or row that isn't on the map. `is_occupied` is true when the stored weight is at least the threshold. Pass `None` for 128. Pass `Some(100)` to treat a weight of 100 as occupied, which the default does not. Rust has no default arguments, so `None` is how you ask for 128. `center` is the middle of a cell. You'll want that later, when a planner has to turn cells back into points. None of these helpers decide which returns are obstacles.

You don't write the perception networking. `src/perception/node.rs` already subscribes to pose and LiDAR, and `ObservationJoin` already pairs a pose with the scan from the same moment.

A working version marks cells for real hits, ignores a hit that falls off the map, and accepts an empty scan. Look at the same course again and the obstacles you already marked should still be there. Return `Some(&self.map)` after an update, including an empty scan that changed nothing. Return `None` only when you don't have a map yet. Every number you publish has to be a real number.

```sh
cargo run -- perception
cargo run -- perception --fixture fixtures/perception/observation.json
cargo test --test perception -- --ignored
```

The fixture is a small rotated pose with duplicate points, so you can check it by hand. Out in the open, an empty scan is normal, not a broken sensor. The detour course is where the hits show up. Try one point by hand, an empty scan, a hit off the map, the duplicate points, and then a live detour scan.

Once that's boring, the usual next steps are tracing rays to mark free space, storing a belief with `set_weight` instead of a hard 255, ignoring noise, or letting old obstacles fade.

## Planning

Given where the car is, where it wants to go, and the map, what route misses the obstacles?

Treat the open cells as places you can step, and pretend every step costs something. You want the cheap route. If you've heard of a cost function, this is that: you pay for each action, and you'd like the smallest bill. Dijkstra's algorithm always expands the cell that's cheapest to reach from the start, so it wanders over a wide area. A* also guesses how far is left to the goal, which pulls the search toward cells that are actually getting closer. The guess has to be honest. If you claim the goal is closer than it is, A* can miss the best route. We'll talk through this in person. If the bookkeeping feels like a lot, start with a plain breadth-first search and add the guess once the search itself works. A* is the version to end up with. `VecDeque` and `BinaryHeap` are in the standard library, so you don't need another crate. `f64` can't be ordered by `BinaryHeap`. Use `total_cmp`, or store the cost as an integer. A helper that returns `impl Iterator` while looking at the grid borrows the grid for the life of that iterator. Return a `Vec` if the compiler complains about a captured lifetime.

`Planner::plan` in `src/planning/algorithm.rs` gets the grid, where the car is, the goal, `clearance_m`, and `goal_tolerance_m`. Return the waypoints, or `None` if there's no way through. Right now it returns `None`. A path is a list of world points, starting near the car and ending near the goal. The last point has to land inside `goal_tolerance_m`.

`Grid::blocked` is true for a cell whose weight is at least 128, and also for a column or row off the map. That is `is_occupied` with `None`. `Grid::clear` asks whether a circle around a point is empty. Pass `clearance_m` as that radius. `Grid::segment_clear` asks the same thing for the strip between two points, with the same radius. `Grid::center` turns a cell back into a point on the path. `clearance_m` comes from `VehicleSpec::clearance_m`: a circle around the rear axle big enough to cover the whole car no matter which way it points. On this starter car that circle is about 0.266 m, and every course uses a 0.2 m goal tolerance. Use the arguments. They already carry those numbers. A route that fits a point, and not the car, will clip a wall.

`planned_update` in `src/planning/node.rs` calls your planner and then `validate_path`. That check wants real numbers, a first point within two cell widths of the car, a last point close enough to the goal, and clearance on every point and on every segment between them. `None` from you becomes "no path." A path the checker rejects is an error, so build one it will take.

```sh
cargo run -- planning --scenario empty
cargo run -- planning --scenario detour
cargo run -- planning --scenario blocked
cargo test --test planning -- --ignored
```

Those three commands load maps we already made. They don't run your perception. Empty and detour should come back with a route. Blocked should come back with no path, and the message says that is the expected result. An empty or detour run that returns nothing says the planner is still unfinished. `cargo run -- perception` saying a map was returned only means `update` returned `Some`. The cell check is `cargo test --test perception -- --ignored`.

One warning. A path that stays on empty cells can still include a corner the car can't turn. Leave some room, keep the speed down, and let the controller look ahead.

If you get there with time left, smoothing, a different guess of the remaining distance, the car's turning radius, or a completely different search are all reasonable.

## Controls

The path is a list of points. What steering and speed make the car follow it?

The basic version is pure pursuit. Look a fixed distance ahead on the path and aim at that spot. The car moves, so the spot moves with it, and you're always aiming a little further along the route. In the car's own frame, if the spot is at `(x, y)` and `L²` is the squared distance to it, the curvature you want is `2y / L²`. The steering angle is `atan(wheelbase * curvature)`. The wheelbase is `vehicle.wheelbase_m` on the spec you were given. Measure it from there instead of typing a constant.

You still choose how far ahead to look, which point that is, how you walk along the path, what to do when the leftover points are behind the car, and how fast to go. Too close and the car wiggles. Too far and it cuts the corner. If the distance to the target is zero, don't divide by it.

`Controller` in `src/controls/algorithm.rs` is built with the car's measurements. `command` gets the current pose and speed, the waypoints, and `dt_s`, the seconds since the last update. It returns a `DriveCommand`. Right now that's a full stop. Positive steering is a left turn. Positive speed is forward.

`controlled_update` in `src/controls/node.rs` is the wrapper around your function. It publishes a stop when the path is missing, isn't ready, or is too old, and when `dt_s` isn't a real positive number of seconds. An empty list of points, or a point that isn't a real number, comes back as an error from the wrapper. Your function should still stop if it's handed an empty path. `VehicleSpec::limit` clamps a real command to the car's max forward speed and max steering angle. The wrapper uses that to reject NaN and infinity, then publishes the numbers you returned. The runner notices when `limit` would have changed them. The controls test looks at that return, so `command` itself has to stay inside the limits, and the speed on that fixture has to be greater than zero. The simulator clamps again before anything moves. You don't decide when the car has arrived. The runner does, and it has its own safety stop. The commands below read a path from a file, so your perception and planner stay out of it.

```sh
cargo test --test controls -- --ignored
cargo run -- controls
cargo run -- controls --path fixtures/controls/curve.json
```

Try a straight line, then a correction to the left, then one to the right, before you take on the curve.

After that, people usually vary the lookahead with speed, slow down for corners, or wrap a feedback loop around this steering command.

## Connecting the pieces

One scan takes this trip:

1. The simulator tells orchestrator where the car is and what the laser sees.
2. Orchestrator publishes `state` and `lidar` for this run.
3. Perception reads both, matches them by time, and publishes a `map`.
4. Planning reads the map and publishes a `path`.
5. Controls reads the state and the path and publishes a `command`.
6. Orchestrator checks the command and tells the simulator what to do with the wheels.

Each run has its own topic names, so a leftover message from the last run can't drive this one. A missing or old path stops the car. Publishing an old plan again doesn't make it new.

The three pieces are three programs. Perception's `run`, in `src/perception/node.rs`, is the finished example. It subscribes, reads with `transport::receive`, and publishes with `put`. Yours should look like that. The functions that turn your answer into a message are already written.

Planning's `run`, in `src/planning/node.rs`, is the empty one. Subscribe to `map`, keep one `Planner` alive across messages, call `planned_update`, and publish on `path`. Publish something for every map, including a map that isn't ready and a planner that returned `None`. Otherwise the car keeps the previous path. `Context` has the Zenoh session, `transport.key(...)`, the goal, and the vehicle. The message types live in `shared::protocol::pb`, and `prost::Message` turns them into bytes and back. `Context::poll` is how the loop hears that it should quit. `Context::status` is how we can tell a wiring problem from an algorithm problem.

Controls' `run`, in `src/controls/node.rs`, subscribes to `state` and `path`, remembers the latest path even when that path says there's nothing to follow, and publishes `command`. Wake up once per new state, about 10 times a second, and pass `dt_s` from the capture times. Call `controlled_update`. When you publish a command, leave the path's original time alone. A fresh command doesn't refresh an old path.

```sh
cargo run -- fixture-publish planning --run planning-practice
cargo run -- node planning --run planning-practice
cargo test --test wiring -- --ignored
```

Use `perception` or `controls` in place of `planning` to practice those. This publisher only talks to the router on your machine. The wiring tests accept an unavailable map or a ready one, "no path" or a real path, and "stop" or a real finite command. `cargo test --locked` stays green either way. The tests in the earlier sections still want the real algorithms.

## Showing your work

Get each piece working on its own files before you combine them. Then:

```sh
cargo run -- autonomous --scenario empty
cargo run -- autonomous --scenario detour
cargo run -- autonomous --scenario blocked
```

`autonomous` starts all three programs. Until planning and controls are connected, it tells you what's missing and the car stays put. If the empty or detour run ends without reaching the goal, the command fails. The blocked course is the exception: stopping without an `ego-collision` exits 0, and hitting the wall exits with an error. While you're still working, use `--duration 5` or Ctrl-C. By the end, reach the goal on the empty course and the detour course, and stop on the blocked course.

Save a result from early on. Find one thing that's wrong, either in the files or in the browser's Analysis and Replay views, change it, and compare. The folder the command prints contains `node-events.jsonl`, `updates.csv`, `summary.json`, and `view.svg`. The CSV is seconds, meters, and radians, on the simulator's clock. In the picture, the path you asked for is blue and the path the car drove is orange. The same course is a fair comparison. The exact trace can still wobble, because the browser's timing isn't perfectly repeatable.

On November 19, bring a working planner and the perception behind it. On December 5, show the car driving a course. Walk through how you did perception, planning, and controls, including anything you tried besides the suggestion, and why you picked it. Show a test or a log of something that failed, and say what you'd do with another week. A run that works here means the algorithms handled these courses. It doesn't tell us that a physical car is calibrated, or that its localization is right.

By December 6, the basic version should drive a simple course, with your own code in all three functions, a test or a saved run that shows it, and a few sentences on how a scan becomes a command. Put that in the pull request, or in a `RESULTS.md` if that's easier. One change per branch, and run the fixture or test for that change before you open the request.

## If the car just sits there

Most of the time it's one of these.

- "Perception unavailable" means `update` is still empty. Start with the perception fixture.
- "No path" means the planner is still empty, or the goal really is walled off. Try the empty planning map.
- "Controller returned stop" means the controller is still the stub, or you told it to stop.
- Zero laser points on the empty course is normal. Watch the detour course if you want hits.
- A rotated or mirrored map usually means the transforms ran in the wrong order, or y got flipped. Sensor, then car, then world, and +y stays left.
- A transposed map has x and y swapped in the grid. y picks the row, x picks the column.
- A path through a wall checked the cells under the waypoints and skipped the line between them.
- Steering away from the path means the sign is backwards. Positive steering is left. Try both directions on a short path.
- "Command limited" means the numbers were real, and bigger than the car's speed or steering limit.
- Updates slower than 300 ms stop the car. That watchdog is `FRESH_NS` in the Rust nodes and the same 300 ms in orchestrator's command adapter. Leave the bundle's 100 ms command `timeoutNs` and 200 ms `validityNs` as they are.
- "No connected simulator tab", "More than one connected simulator tab", and "The simulator tab still has a run going" are three different problems. Stop the run, and keep a single simulator tab.
- "The process at … is not the simulator" means `--server` is some other program. Use the simulator from getting started. Another program on port 3000 produces this message.
- "Start the router: zenohd -c config/zenoh-router.json5" means the local Zenoh router is not running.
- A hash mismatch means the saved course disagrees with its sidecar. Run `setup` against a fresh simulator data directory. Don't edit the hash.
- If you restart orchestrator after the page connected, refresh the page.
- `simulator_passed: false` after the car reached the goal can mean the browser recorded the stop as an interruption. Check `goal_reached` and the termination reason in `summary.json`.

When the compiler complains, read the first error. A reference lets you look at a value. It doesn't store one. Keep the map, the search, and the controller's progress on your struct. If you're still borrowing that struct, finish the borrow before you change it.

`cargo fmt` after you edit. Test one piece at a time with `cargo test --test <component> -- --ignored`. Print a few numbers from a tiny input. Printing every LiDAR point won't make the bug clearer.
