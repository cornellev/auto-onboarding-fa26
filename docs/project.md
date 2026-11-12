# Autonomy onboarding

Over the next month you're building the software that lets a minicar drive from a start to a goal without hitting what's in the way.

That breaks into three questions. How does the car tell where the obstacles are? How does it pick a route? How does it steer and choose a speed so it actually follows that route? You'll answer them in Rust, and you'll check the answers in the simulator before any of this goes on a real car. We're not going to hand you the algorithms. We'll suggest a place to start, and the work sessions are where we walk through the ideas. Each section ends with a few readings, if you want to see how someone else worked that idea out. Get a basic version working before you wander off into something fancier.

The code you already have compiles, talks to the simulator, and holds the car still. The three empty functions are yours: `Perception::update`, `Planner::plan`, and `Controller::command`.

If the tools aren't installed, do [getting started](getting-started.md) first. Remember to look at the project timeline posted in Discord. Just as a reminder, we'll be done with this project by **December 6** - at these sessions, you'll be sharing your code, not just your results.

## Your plan

Before you disappear into the code, remember your plan that you wrote down in the first session. It's important that you have *guidance* for what you're doing, and that you can explain it to someone else. Additionally, remember our concept of completing your MVP first before you add any extra features. You can always add more later, but you need to have a working version first!

### AI Usage

As a team, we use plenty of AI - this project's starter code was built by both me (Jaiden) and my AI agents, alongside a few of these instructions. Additionally, on your side this project can definitely be one-shotted by an AI agent, but we highly highly recommend that you do not do that. The goal of this project is to learn how to build autonomy software, and if you just have an AI agent do it for you, you won't learn anything. Instead, we recommend that you use AI agents as a tool to help you with your work, but not as a replacement for your work. For example, if you're stuck on a problem, you can ask an AI agent for help, but you should still try to solve the problem yourself first.

Additionally, we'll be testing a bit of your understanding at the end of the semester. If you're spending some time implementing your own solution, you'll be able to answer our questions.

GenAI is a very powerful tool that you can use to help you with your work, but you should be ultimately the architect and builder! There's a distinction between telling an AI agent to do something vs using it to build something. This project is small enough in scope that I'd limit it to explaining problems, trouble shooting, and minor code tweaks.

### Library

If you're confused about the code provided to you, you can look at the documentation in [the library](library/overview.md).

## Perception

Where are the obstacles?

The basic version is an occupancy grid. Chop the ground into little squares, a few centimeters on a side. Each cell holds a weight from 0 to 255. `set_occupied` writes 255. A weight of 128 or more is blocked. You can also track "we haven't looked here," but skip that for now.

We give you the grid. You turn the LiDAR points into occupied cells.

A LiDAR point starts where the sensor is looking: x forward, y to the left, in meters. The car's pose is the middle of the rear axle, in the same kind of coordinates, and the heading is in radians, counterclockwise from +x. `sensor_in_vehicle` says where the LiDAR is bolted on, relative to that axle. `Pose2::transform` does one move: rotate by the pose's heading, then shift by its position. Use it twice. First from the sensor into the car, then from the car into the world. If the car is facing 90° left, a point that was straight ahead of the sensor should land toward +y in the world. Check one point from the fixture on paper before you loop over a whole scan. The simulator keeps its own axis convention, and that's already converted before you see the points, so leave +y as left.

`Perception` in `src/perception/algorithm.rs` holds the map. `update` is handed an `Observation`, which is the pose, the scan points, and the sensor mount, and it should return `Some(&self.map)` once you have a map. Right now it returns `None`, which is why the car never moves. The runner builds an empty grid and passes it to `Perception::new`. A new cell is 0, and 0 is free. That's reasonable on these courses, where the obstacles sit still and the car can see them. It would be a bad assumption on a course the car hasn't fully looked at.

The browser's vehicle origin is `base_link`. `rear_axle_in_base.x` is −0.093454 m, so the onboarding spawn — a base_link near `(1.57, −1.59)` in the simulator, facing −x — is the planning start `(1.663454, 1.59)`. The red marker is the course goal `(−1.65, −0.93)`. The pose you receive is already the rear axle, with +x forward and +y left. The simulator frame is different: Y is up, and heading is `(cos yaw, 0, −sin yaw)`. The nodes have already done that conversion. `scenarios/onboarding.json` stores an empty grid. The obstacles for `cargo run -- planning` are in `fixtures/planning`. The live obstacles arrive as LiDAR.

`Grid::cell` gives you the column and row of a world point, or `None` if the point is off the map. Columns follow x, rows follow y, and the cell is stored at `row * width + column`. `set_occupied` writes 255 into one cell. `set_weight` writes a belief from 0 to 255, and 0 clears the cell. Both ignore a column or row that isn't on the map. `is_occupied` is true when the stored weight is at least the threshold. Pass `None` for 128. Pass `Some(100)` to treat a weight of 100 as occupied, which the default does not. Rust has no default arguments, so `None` is how you ask for 128. `center` is the middle of a cell. You'll want that later, when a planner has to turn cells back into points. None of these helpers decide which returns are obstacles.

You don't write the perception networking. `src/perception/node.rs` already subscribes to pose and LiDAR, and `ObservationJoin` already pairs a pose with the scan from the same moment.

A working version marks cells for real hits, ignores a hit that falls off the map, and accepts an empty scan. Look at the same course again and the obstacles you already marked should still be there. Return `Some(&self.map)` after an update, including an empty scan that changed nothing. Return `None` only when you don't have a map yet. Every number you publish has to be a real number.

```sh
cargo run -- perception
cargo run -- perception --fixture fixtures/perception/observation.json
cargo test --test perception -- --ignored
```

The fixture is a small rotated pose with duplicate points, so you can check it by hand. Out in the open, an empty scan is normal, not a broken sensor. The live square has cones, so that scan is where the hits show up. Try one point by hand, an empty scan, a hit off the map, the duplicate points, and then a live scan.

Once that's boring, the usual next steps are tracing rays to mark free space, storing a belief with `set_weight` instead of a hard 255, ignoring noise, or letting old obstacles fade.

### Readings

Check one point from the fixture on paper before you trust a formula from one of these. A couple of them draw the axes differently.

- [Coordinate frames and sensor geometry](https://fixtherobot.dev/perception/theory/coordinate-frames-and-sensor-geometry/). How a point moves from the sensor, into the car, then into the world. Rotate, then shift. That is one call to `Pose2::transform`, and you do it twice. The worked example is the same check as doing one point by hand. The camera section is a different sensor.
- [Grid maps and mapping with known poses](http://ais.informatik.uni-freiburg.de/teaching/ss14/robotics/slides/08-occupancy-mapping-mapping-with-known-poses.pdf). Freiburg's slides on an occupancy grid when the pose is already known, which it is here. Marking the cell a hit lands in is the basic version, and that is `set_occupied`. The log-odds update later in the slides is the `set_weight` version. Their pictures are for a sonar. Yours is a LiDAR point you already have in meters.
- [Lidar to a grid map](https://atsushisakai.github.io/PythonRobotics/modules/3_mapping/lidar_to_grid_map_tutorial/lidar_to_grid_map_tutorial.html). How the cells along a beam get marked, which is the ray-tracing step. The angle-to-xy snippet in that tutorial is a different axis convention. Your scan points are already x forward and y left.

## Planning

Given where the car is, where it wants to go, and the map, what route misses the obstacles?

`Planner::plan` in `src/planning/algorithm.rs` gets the grid, where the car is, the goal, `clearance_m`, and `goal_tolerance_m`. Return the waypoints, or `None` if there's no way through. Right now it returns `None`. A path is a list of world points, from `Grid::center`, starting near the car and ending near the goal. The last point has to land inside `goal_tolerance_m`.

A cell the car can sit on is one where `Grid::clear(center, clearance_m)` is true. A step the car can drive is one where `Grid::segment_clear` is true for that same radius. `Grid::blocked` is a different question: the cell's weight is at least 128, or the column or row is off the map. That is `is_occupied` with `None`. A path of cells that are only not blocked still fails `validate_path`, and a failed check is an error, not "no path." `None` means there is no route. `segment_clear` thickens the strip past the radius you pass, by half the gap between its samples, so two clear endpoints can still be a blocked step. `clearance_m` comes from `VehicleSpec::clearance_m`: a circle around the rear axle big enough to cover the whole car no matter which way it points. On this starter car that circle is about 0.263 m, and every course uses a 0.2 m goal tolerance. Use the arguments. They already carry those numbers. A route that fits a point, and not the car, will clip a wall.

Treat the cells that pass `clear` as places you can step, and pretend every step costs something. You want the cheap route. If you've heard of a cost function, this is that: you pay for each action, and you'd like the smallest bill. Dijkstra's algorithm always expands the cell that's cheapest to reach from the start, so it wanders over a wide area. A* also guesses how far is left to the goal, which pulls the search toward cells that are actually getting closer. The guess has to be honest. If you claim the goal is closer than it is, A* can miss the best route. We'll talk through this in person. If the bookkeeping feels like a lot, start with a plain breadth-first search and add the guess once the search itself works. A* is the version to end up with. `VecDeque` and `BinaryHeap` are in the standard library, so you don't need another crate. `f64` can't be ordered by `BinaryHeap`. Use `total_cmp`, or store the cost as an integer. A helper that returns `impl Iterator` while looking at the grid borrows the grid for the life of that iterator. Return a `Vec` if the compiler complains about a captured lifetime.

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

### Readings

The search is the part to learn. The path you return still has to be world points, from `Grid::center`, that `validate_path` will take.

- [Introduction to A*](https://www.redblobgames.com/pathfinding/a-star/introduction.html), and the [implementation notes](https://www.redblobgames.com/pathfinding/a-star/implementation.html) that go with it. Breadth-first search, then Dijkstra, then A*, on a grid: the queue, the cell you came from, and how you rebuild the path. The guess of the remaining distance has to be no larger than the real distance, or A* can miss the best route. The Manhattan example on that page is the honest guess when every step is up, down, left, or right. A diagonal step that costs less than two of those needs straight-line distance. Their samples are Python.
- [Heuristics](https://theory.stanford.edu/~amitp/GameProgramming/Heuristics.html). Which guesses of the remaining distance stay honest, once the search itself works.
- [Grid-based search](https://atsushisakai.github.io/PythonRobotics/modules/5_path_planning/grid_base_search/grid_base_search.html). The same three searches, drawn on a grid, with a radius so the route fits the car. Those examples grow the obstacles by that radius before searching. Here the grid stays as perception wrote it. Pass `clearance_m` to `Grid::clear` and `Grid::segment_clear`. Theta* on that page is one way to straighten the path. Hybrid A* is a search that also knows the car cannot turn in place.
- [`VecDeque`](https://doc.rust-lang.org/std/collections/struct.VecDeque.html) is the queue for breadth-first search. [`BinaryHeap`](https://doc.rust-lang.org/std/collections/binary_heap/struct.BinaryHeap.html) is the one for Dijkstra and A*. It gives back the largest item first, so store the cost in a way that the cheapest cell comes out on top. `f64` is not ordered. `total_cmp`, or an integer cost, is how you put one in that heap.

## Controls

The path is a list of points. What steering and speed make the car follow it?

The basic version is pure pursuit. Look a fixed distance ahead on the path and aim at that spot. The car moves, so the spot moves with it, and you're always aiming a little further along the route. Bring that spot into the car's frame with `pose.inverse_transform(point)`. In that frame, if the spot is at `(x, y)` and `L²` is the squared distance to it, the curvature you want is `2y / L²`. The steering angle is `atan(wheelbase * curvature)`. The wheelbase is `vehicle.wheelbase_m` on the spec you were given. Measure it from there instead of typing a constant. Positive `y` is a left turn.

You still choose how far ahead to look, which point that is, how you walk along the path, what to do when the leftover points are behind the car, and how fast to go. Too close and the car wiggles. Too far and it cuts the corner. If the distance to the target is zero, don't divide by it. Start with a lookahead longer than `wheelbase_m` and a speed below `max_speed_mps`. The ignored controls test only checks an empty path, a positive speed inside the cap, and the sign of the steering on a straight offset. It passes for many of those choices. It does not tell you that the car tracks the curve. That check is `cargo run -- controls` and `cargo run -- controls --path fixtures/controls/curve.json`.

`Controller` in `src/controls/algorithm.rs` is built with the car's measurements. `command` gets the current pose and speed, the waypoints, and `dt_s`, the seconds since the last update. It returns a `DriveCommand`. Right now that's a full stop. Positive steering is a left turn. Positive speed is forward.

`controlled_update` in `src/controls/node.rs` is the wrapper around your function. It publishes a stop when the path is missing, isn't ready, or is too old, and when `dt_s` isn't a real positive number of seconds. An empty list of points, or a point that isn't a real number, comes back as an error from the wrapper. Your function should still stop if it's handed an empty path. `VehicleSpec::require_finite` rejects a command that is not a real number. The wrapper calls that, then publishes the numbers you returned. It does not clamp them. `VehicleSpec::limit` is what the runner uses, and it notices when that clamp would have changed your numbers. The controls test looks at what `command` returns, so that return has to stay inside the speed and steering caps, and the speed on that fixture has to be greater than zero. The simulator clamps again before anything moves. You don't decide when the car has arrived. The runner does, and it has its own safety stop. The commands below read a path from a file, so your perception and planner stay out of it.

```sh
cargo test --test controls -- --ignored
cargo run -- controls
cargo run -- controls --path fixtures/controls/curve.json
```

Try a straight line, then a correction to the left, then one to the right, before you take on the curve.

After that, people usually vary the lookahead with speed, slow down for corners, or wrap a feedback loop around this steering command.

### Readings

The pose you get is already the middle of the rear axle. Take the wheelbase from `vehicle.wheelbase_m`. If the distance to the target is zero, skip the division.

- [Pure pursuit and geometric path tracking](https://mohammadijoo.github.io/CaaT_Academy/Lectures/Autonomous_Mobile_Robots_Course/Chapter15/Lesson1.html). Where `2y / L²` comes from, with the lookahead point in the car's frame, and why the steering angle is `atan(wheelbase * curvature)`. Positive y is a left turn. Sections 3 and 4 are the part that matches this car. The lookahead distance, the speed, and the points that have fallen behind the car are still yours to decide.
- [Implementation of the Pure Pursuit Path Tracking Algorithm](https://publications.ri.cmu.edu/storage/publications/pub_files/pub3/coulter_r_craig_1992_1/coulter_r_craig_1992_1.pdf) (Coulter, 1992). Section 3.3 is the procedure: find the closest point on the path, walk forward to a point one lookahead away, bring that point into the car's frame, then turn it into a curvature. Section 4 is what a short lookahead and a long one each do. His figure measures the sideways offset along the rear axle. On this car the sideways axis is y, to the left, and the curvature is `2y / L²`.
- [Automatic Steering Methods for Autonomous Automobile Path Tracking](https://publications.ri.cmu.edu/storage/publications/pub_files/2009/2/Automatic_Steering_Methods_for_Autonomous_Automobile_Path_Tracking.pdf) (Snider, 2009). Sections 2.1 and 2.2 are the bicycle model and the same curvature, written as `2 sin(α) / L`. That is `2y / L²` when y is the sideways distance to the lookahead point. The rest of 2.2 is why the lookahead usually grows with speed, and why a short one wiggles while a long one cuts the corner. Section 2.3, the Stanley method, is one feedback law people try after pure pursuit is driving.

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

Use `perception` or `controls` in place of `planning` to practice those. This publisher only talks to the router on your machine. A passing wiring test means a message was published. It accepts an unavailable map or a ready one, "no path" or a real path, and "stop" or a real finite command, including the answers the stubs publish. `cargo test --locked` stays green either way. The perception, planning, and controls tests are the ones that reject a stub.

### Readings

`run` in `src/perception/node.rs` is the loop to copy. It subscribes, reads with `transport::receive`, and publishes with `put`.

- [Your first Zenoh app](https://zenoh.io/docs/getting-started/first-app/). A publisher puts bytes on a key, and a subscriber receives whatever is published there. That is the relationship between the three programs. The sample opens its own session and publishes a string. This program already has a session on `Context`. The key for this run comes from `transport.key`, and the bytes are `encode_to_vec` on a protobuf message.

## Showing your work

Get each piece working on its own files before you combine them. Then:

```sh
cargo run -- autonomous
```

`autonomous` starts all three programs on the onboarding square. Until planning and controls are connected, it tells you what's missing and the car stays put. If the run ends without reaching the goal, the command fails. While you're still working, use `--duration 5` or Ctrl-C. By the end, reach the red marker. The planning maps are separate from that live run: empty is open, detour is a frozen copy of the cones, and blocked adds a wall across the map. Those three are `cargo run -- planning`, and blocked should come back with no path.

Save a result from early on. Find one thing that's wrong, either in the files or in the browser's Analysis and Replay views, change it, and compare. The folder the command prints contains `node-events.jsonl`, `updates.csv`, `summary.json`, and `view.svg`. The CSV is seconds, meters, and radians, on the simulator's clock. In the picture, the path you asked for is blue and the path the car drove is orange. The same course is a fair comparison. The exact trace can still wobble, because the browser's timing isn't perfectly repeatable.

On November 19, bring a working planner and the perception behind it. On December 5, show the car driving a course. Walk through how you did perception, planning, and controls, including anything you tried besides the suggestion, and why you picked it. Show a test or a log of something that failed, and say what you'd do with another week. A run that works here means the algorithms handled these courses. It doesn't tell us that a physical car is calibrated, or that its localization is right.

By December 6, the basic version should drive a simple course, with your own code in all three functions, a test or a saved run that shows it, and a few sentences on how a scan becomes a command. Put that in the pull request, or in a `RESULTS.md` if that's easier. One change per branch, and run the fixture or test for that change before you open the request.

## If the car just sits there

Most of the time it's one of these.

- "Perception unavailable" means `update` is still empty. Start with the perception fixture.
- "No path" means the planner is still empty, or the goal really is walled off. Try the empty planning map.
- "Controller returned stop" means the controller is still the stub, or you told it to stop.
- Zero laser points can happen if nothing is in the beam. The onboarding square has cones, so a live scan should usually show hits.
- A rotated or mirrored map usually means the transforms ran in the wrong order, or y got flipped. Sensor, then car, then world, and +y stays left.
- A transposed map has x and y swapped in the grid. y picks the row, x picks the column.
- A path through a wall checked the cells under the waypoints and skipped the line between them.
- Steering away from the path means the sign is backwards. Positive steering is left. Try both directions on a short path.
- "Command limited" means the numbers were real, and bigger than the car's speed or steering limit.
- Updates slower than 300 ms stop the car. That watchdog is `FRESH_NS` in the Rust nodes and the same 300 ms in orchestrator's command adapter. Leave the run manifest's 100 ms command `timeoutNs` and 200 ms `validityNs` as they are.
- "No connected simulator tab", "More than one connected simulator tab", and "The simulator tab still has a run going" are three different problems. Stop the run, and keep a single simulator tab.
- "The process at … is not the simulator" means `--server` is some other program. Use the simulator from getting started. Another program on port 3000 produces this message.
- "Start the router: zenohd -c config/zenoh-router.json5" means the local Zenoh router is not running.
- `setup` only checks that the simulator already has the run manifest named `onboarding`. It does not install a world. If that manifest is missing, create it in the simulator.
- If you restart orchestrator after the page connected, refresh the page.
- `simulator_passed: false` after the car reached the goal can mean the browser recorded the stop as an interruption. Check `goal_reached` and the termination reason in `summary.json`.

When the compiler complains, read the first error. A reference lets you look at a value. It doesn't store one. Keep the map, the search, and the controller's progress on your struct. If you're still borrowing that struct, finish the borrow before you change it.

`cargo fmt` after you edit. Test one piece at a time with `cargo test --test <component> -- --ignored`. Print a few numbers from a tiny input. Printing every LiDAR point won't make the bug clearer.
