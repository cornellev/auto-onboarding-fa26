use cev_onboarding::{
    error,
    grid::Grid,
    hardware::VehicleIo,
    protocol::{self, pb},
    reporting::Reporter,
    runner::{Course, Mode, Update},
    sim::Sim,
    student::{perception::Perception, planning::Planner},
    transport,
    types::*,
    Result,
};
use serde::Deserialize;
use std::{
    env, fs,
    path::PathBuf,
    process::ExitCode,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn load<T: serde::de::DeserializeOwned>(path: impl AsRef<std::path::Path>) -> Result<T> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn run() -> Result<()> {
    let mut args = env::args().skip(1);
    let mode = args.next().unwrap_or_else(|| "observe".into());
    if matches!(mode.as_str(), "--help" | "-h" | "help") {
        println!("cargo run -- <setup|observe|perception|planning|controls|autonomous|drive-test|node|fixture-publish>\n  --scenario <empty|detour|blocked>  --fixture <file>  --path <file>\n  --server <http://localhost:3000>  --duration <seconds>\n  node <role> --run <id> | fixture-publish <role> [--run <id>]\n  --manual-nodes\nDefault: observe. See README.md for simulator setup.");
        return Ok(());
    }
    let node_role = if mode == "node" || mode == "fixture-publish" {
        Some(
            args.next()
                .ok_or_else(|| error("Specify perception, planning, or controls"))?,
        )
    } else {
        None
    };
    let mut run_id = None;
    let mut manual = false;
    let (mut scenario, mut server) = (
        "empty".to_string(),
        env::var("CEV_SIM_SERVER").unwrap_or_else(|_| "http://localhost:3000".into()),
    );
    let (mut fixture, mut path_file, mut duration): (Option<String>, Option<String>, Option<f64>) =
        (None, None, None);
    while let Some(flag) = args.next() {
        if flag == "--manual-nodes" {
            manual = true;
            continue;
        }
        let value = args
            .next()
            .ok_or_else(|| error(format!("Missing value for {flag}")))?;
        match flag.as_str() {
            "--run" => run_id = Some(value),
            "--scenario" => scenario = value,
            "--server" => server = value,
            "--fixture" => fixture = Some(value),
            "--path" => path_file = Some(value),
            "--duration" => duration = Some(value.parse()?),
            _ => return Err(error(format!("Unknown option {flag}; use --help"))),
        }
    }
    if !matches!(scenario.as_str(), "empty" | "detour" | "blocked") {
        return Err(error("Scenario must be empty, detour, or blocked"));
    }
    if duration.is_some_and(|n| !n.is_finite() || n <= 0.0) {
        return Err(error("Duration must be finite and positive"));
    }
    if mode == "setup" {
        cev_onboarding::sim::setup(&server)?;
        return cev_onboarding::sim::setup_profile();
    }
    let interrupted = Arc::new(AtomicBool::new(false));
    let signal = interrupted.clone();
    ctrlc::set_handler(move || signal.store(true, Ordering::SeqCst))?;
    if mode == "node" {
        let run = run_id.ok_or_else(|| error("node requires --run <id>"))?;
        let role = node_role.as_deref().unwrap();
        let ctx = cev_onboarding::nodes::Context::connect(role, &run, interrupted)?;
        return match role {
            "perception" => cev_onboarding::nodes::perception::run(ctx),
            "planning" => cev_onboarding::nodes::planning::run(ctx),
            "controls" => cev_onboarding::nodes::controls::run(ctx),
            _ => Err(error("Unknown node role")),
        };
    }
    if mode == "fixture-publish" {
        return cev_onboarding::fixtures::publish(
            node_role.as_deref().unwrap(),
            run_id,
            duration,
            interrupted,
        );
    }
    let mut course: Course = load(format!("scenarios/{scenario}.json"))?;
    course.validate()?;
    if let Some(duration) = duration {
        course.timeout_s = duration.min(course.timeout_s);
    }
    if let Some(path) = path_file {
        course.control_path = load(path)?;
    }
    course.validate()?;
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let mut reporter = Reporter::new(PathBuf::from("results").join(format!("{mode}-{stamp}")))?;
    if mode == "perception" {
        let observation: Observation =
            load(fixture.unwrap_or_else(|| "fixtures/perception.json".into()))?;
        observation.validate()?;
        let mut perception = Perception::new(course.grid.clone());
        let map = perception.update(&observation);
        reporter.summary.status = if map.is_some() {
            "perception fixture complete"
        } else {
            "perception unavailable: implement Perception::update"
        }
        .into();
        let map = map.cloned().unwrap_or_else(|| perception.map.clone());
        reporter.finish(&map, &[], course.goal)?;
        return Ok(());
    }
    if mode == "planning" {
        #[derive(Deserialize)]
        struct Fixture {
            grid: Grid,
            start: VehicleState,
            goal: Point2,
            vehicle: VehicleSpec,
        }
        let fixture: Fixture =
            load(fixture.unwrap_or_else(|| format!("fixtures/planning-{scenario}.json")))?;
        fixture.grid.validate()?;
        fixture.vehicle.validate()?;
        let path = Planner::default().plan(&fixture.grid, &fixture.start, fixture.goal);
        if let Some(path) = &path {
            cev_onboarding::runner::validate_path(
                &fixture.grid,
                path,
                fixture.start.pose.position,
                fixture.goal,
                fixture.vehicle.clearance_m(),
                course.goal_tolerance_m,
            )?;
        }
        reporter.summary.status = if path.is_some() {
            "planning fixture complete"
        } else {
            "no path returned: implement Planner::plan, or verify the goal is blocked"
        }
        .into();
        reporter.finish(&fixture.grid, path.as_deref().unwrap_or(&[]), fixture.goal)?;
        return Ok(());
    }
    let mode = match mode.as_str() {
        "observe" => Mode::Observe,
        "controls" => Mode::Controls,
        "autonomous" => Mode::Autonomous,
        "drive-test" => Mode::DriveTest,
        _ => return Err(error("Unknown mode; use --help")),
    };
    if mode == Mode::Controls && scenario != "empty" {
        return Err(error(
            "Controls fixtures run on the empty course. Test obstacle avoidance with autonomous.",
        ));
    }
    let mode_name = match mode {
        Mode::Observe => "observe",
        Mode::Controls => "controls",
        Mode::Autonomous => "autonomous",
        Mode::DriveTest => "drive-test",
    };
    let mut sim = Sim::prepare(&course, &server, mode_name)?;
    let maps = sim.transport.subscribe("map", 1)?;
    let paths = sim.transport.subscribe("path", 1)?;
    let commands = sim.transport.subscribe("command", 1)?;
    let roles: &[&str] = match mode {
        Mode::Autonomous => &["perception", "planning", "controls"],
        Mode::Controls => &["controls"],
        _ => &[],
    };
    let statuses = roles
        .iter()
        .map(|role| sim.transport.subscribe(&format!("status/{role}"), 1))
        .collect::<Result<Vec<_>>>()?;
    let mut children = Children(Vec::new());
    if !manual {
        for role in roles {
            let child = std::process::Command::new(env::current_exe()?)
                .args(["node", role, "--run", &sim.run_id])
                .spawn()?;
            children.0.push(child);
        }
    }
    println!("Run: {}", sim.run_id);
    if manual {
        println!(
            "Start the required nodes with cargo run -- node <role> --run {}",
            sim.run_id
        );
    }
    let mut seen = std::collections::BTreeMap::new();
    let startup = Instant::now();
    while seen.len() < roles.len() {
        for (role, sub) in roles.iter().zip(&statuses) {
            if let Some(status) =
                transport::receive::<pb::NodeStatus>(sub, Duration::from_millis(10))?
            {
                protocol::check_header(&status.header, &sim.run_id, "map")?;
                reporter.event(role, &status.detail, 0, status.pid)?;
                seen.insert(role.to_string(), status.pid);
            }
        }
        if interrupted.load(Ordering::SeqCst) {
            return Err(error("Interrupted waiting for nodes"));
        }
        children.check()?;
        if startup.elapsed() > Duration::from_secs(if manual { 120 } else { 15 }) {
            return Err(error("Required node did not connect"));
        }
    }
    sim.resume()?;
    let mut map = course.grid.clone();
    let mut path = vec![];
    let mut last_command = DriveCommand::STOP;
    let mut source_time = 0;
    let mut last_observation = Instant::now();
    let mut last_status = Instant::now();
    let mut last_print = Instant::now();
    let mut received = false;
    let mut controls_path_checked = false;
    let result: Result<()> = (|| {
        loop {
            if interrupted.load(Ordering::SeqCst) {
                reporter.summary.status = "interrupted".into();
                break;
            }
            children.check()?;
            for (role, sub) in roles.iter().zip(&statuses) {
                if let Some(status) = transport::receive::<pb::NodeStatus>(sub, Duration::ZERO)? {
                    protocol::check_header(&status.header, &sim.run_id, "map")?;
                    if seen.get(*role) != Some(&status.pid) {
                        return Err(error(
                            "Replacement/duplicate node detected; start a fresh run",
                        ));
                    }
                    reporter.event(
                        role,
                        &status.detail,
                        status.header.as_ref().unwrap().time_ns,
                        status.pid,
                    )?;
                }
            }
            if let Some(update) = transport::receive::<pb::MapUpdate>(&maps, Duration::ZERO)? {
                protocol::check_header(&update.header, &sim.run_id, "map")?;
                if update.availability == pb::Availability::Ready as i32 {
                    map = protocol::convert(
                        update
                            .grid
                            .as_ref()
                            .ok_or_else(|| error("Ready map missing grid"))?,
                    )?;
                    map.validate()?;
                } else {
                    path.clear();
                }
            }
            if let Some(update) = transport::receive::<pb::PathUpdate>(&paths, Duration::ZERO)? {
                protocol::check_header(&update.header, &sim.run_id, "map")?;
                path = if update.availability == pb::Availability::Ready as i32 {
                    protocol::convert(
                        &update
                            .path
                            .as_ref()
                            .ok_or_else(|| error("Ready path missing points"))?
                            .points,
                    )?
                } else {
                    vec![]
                };
            }
            if let Some(update) =
                transport::receive::<pb::CommandUpdate>(&commands, Duration::ZERO)?
            {
                protocol::check_header(&update.header, &sim.run_id, "map")?;
                last_command = protocol::convert(
                    update
                        .command
                        .as_ref()
                        .ok_or_else(|| error("Missing driving command"))?,
                )?;
                source_time = update.source.as_ref().map_or(0, |h| h.time_ns);
                reporter.source_time_ns = update.source.as_ref().map(|h| h.time_ns);
            }
            if let Some(observation) = sim.observe(Duration::from_millis(50))? {
                received = true;
                last_observation = Instant::now();
                let finished = observation.time_ns as f64 / 1e9 >= course.timeout_s;
                let reached = observation.state.pose.position.distance(course.goal)
                    <= course.goal_tolerance_m;
                let mut status = if path.is_empty() {
                    "waiting for student map/path and node wiring"
                } else {
                    "following student path"
                };
                match mode {
                    Mode::Observe => {
                        sim.stop()?;
                        status = "observing";
                    }
                    Mode::DriveTest => {
                        last_command = if observation.time_ns < 1_000_000_000 {
                            DriveCommand {
                                speed_mps: 0.1,
                                steering_rad: 0.1,
                            }
                        } else {
                            DriveCommand::STOP
                        };
                        sim.drive(last_command)?;
                        status = "drive-test";
                    }
                    Mode::Controls => {
                        if !controls_path_checked {
                            cev_onboarding::runner::validate_path(
                                &course.grid,
                                &course.control_path,
                                observation.state.pose.position,
                                course.goal,
                                course.vehicle.clearance_m(),
                                course.goal_tolerance_m,
                            )?;
                            controls_path_checked = true;
                        }
                        if !course.grid.clear(
                            observation.state.pose.position,
                            course.vehicle.clearance_m(),
                        ) {
                            return Err(error("Vehicle left the controls fixture bounds"));
                        }
                        sim.path(&course.control_path)?;
                        path = course.control_path.clone();
                    }
                    Mode::Autonomous => {}
                }
                if source_time + protocol::FRESH_NS < observation.time_ns && mode != Mode::DriveTest
                {
                    last_command = DriveCommand::STOP;
                }
                if reached {
                    status = "goal reached";
                } else if finished {
                    status = "timeout";
                }
                let limited = course.vehicle.limit(last_command)?;
                reporter.record(
                    &observation,
                    &Update {
                        command: limited,
                        path: path.clone(),
                        status,
                        finished: finished || reached,
                        saturated: limited != last_command,
                    },
                    course.goal,
                    0,
                )?;
                if last_print.elapsed() > Duration::from_secs(1) {
                    println!(
                        "t={:.1}s pose=({:.2},{:.2}) LiDAR={} points — {status}",
                        observation.time_ns as f64 / 1e9,
                        observation.state.pose.position.x,
                        observation.state.pose.position.y,
                        observation.scan.points.len()
                    );
                    last_print = Instant::now();
                }
                if reached
                    || finished
                    || (mode == Mode::DriveTest && observation.time_ns >= 1_000_000_000)
                {
                    break;
                }
            }
            if last_status.elapsed() > Duration::from_millis(250) {
                let snapshot = sim.status()?;
                if let Some(result) = snapshot.run_result {
                    reporter.summary.termination_reason = result.termination_reason;
                    reporter.summary.simulator_passed = Some(result.passed);
                    break;
                }
                if snapshot.error.is_some() {
                    return Err(error(format!("Simulator error: {:?}", snapshot.error)));
                }
                last_status = Instant::now();
            }
            if last_observation.elapsed()
                > if received {
                    Duration::from_millis(300)
                } else {
                    Duration::from_secs(10)
                }
            {
                return Err(error("Capture-aligned sensors lost; start a fresh run"));
            }
        }
        Ok(())
    })();
    let finished = sim.finish();
    drop(children);
    if let Ok(snapshot) = &finished {
        if let Some(result) = &snapshot.run_result {
            reporter.summary.termination_reason = result.termination_reason.clone();
            reporter.summary.simulator_passed = Some(result.passed);
        }
    }
    if let Err(err) = &result {
        reporter.summary.status = err.to_string();
    }
    reporter.finish(&map, &path, course.goal)?;
    result?;
    finished?;
    if matches!(mode, Mode::Autonomous | Mode::Controls) && !reporter.summary.goal_reached {
        return Err(error("Goal not reached; inspect node-events.jsonl and implement the indicated wiring/algorithm"));
    }
    Ok(())
}

struct Children(Vec<std::process::Child>);
impl Children {
    fn check(&mut self) -> Result<()> {
        for child in &mut self.0 {
            if let Some(status) = child.try_wait()? {
                return Err(error(format!(
                    "Node {} exited ({status}); start a fresh run",
                    child.id()
                )));
            }
        }
        Ok(())
    }
}
impl Drop for Children {
    fn drop(&mut self) {
        for child in &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
