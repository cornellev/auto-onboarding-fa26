use cev_onboarding::{
    error,
    grid::Grid,
    hardware::VehicleIo,
    reporting::Reporter,
    runner::{Course, Mode, Runner},
    sim::Sim,
    student::{perception::Perception, planning::Planner},
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
        println!("cargo run -- <setup|observe|perception|planning|controls|autonomous|drive-test>\n  --scenario <empty|detour|blocked>  --fixture <file>  --path <file>\n  --server <http://localhost:3000>  --duration <seconds>\nDefault: observe. See README.md for simulator setup.");
        return Ok(());
    }
    let (mut scenario, mut server) = ("empty".to_string(), "http://localhost:3000".to_string());
    let (mut fixture, mut path_file, mut duration): (Option<String>, Option<String>, Option<f64>) =
        (None, None, None);
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| error(format!("Missing value for {flag}")))?;
        match flag.as_str() {
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
        return cev_onboarding::sim::setup(&server);
    }
    let mut course: Course = load(format!("scenarios/{scenario}.json"))?;
    course.validate()?;
    if let Some(duration) = duration {
        course.timeout_s = duration.min(course.timeout_s);
    }
    if let Some(path) = path_file {
        course.control_path = load(path)?;
    }
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
    let mut runner = Runner::new(course.clone(), mode)?;
    let interrupted = Arc::new(AtomicBool::new(false));
    let signal = interrupted.clone();
    ctrlc::set_handler(move || signal.store(true, Ordering::SeqCst))?;
    let mut sim = Sim::launch(&course, &server)?;
    println!(
        "Connected to {}. Mode: {mode:?}. Ctrl-C stops and finalizes the run.",
        course.manifest_id
    );
    let mut path = vec![];
    let mut last_observation = Instant::now();
    let mut last_status = Instant::now();
    let mut last_print = Instant::now();
    let mut received = false;
    let result: Result<()> = (|| {
        loop {
            if interrupted.load(Ordering::SeqCst) {
                reporter.summary.status = "interrupted".into();
                break;
            }
            if let Some(observation) = sim.observe(Duration::from_millis(100))? {
                received = true;
                last_observation = Instant::now();
                let start = Instant::now();
                let update = runner.update(&observation)?;
                let elapsed = start.elapsed();
                if elapsed > Duration::from_millis(300) {
                    return Err(error("Student update exceeded 300 ms; stopping"));
                }
                sim.drive(update.command)?;
                if path != update.path {
                    sim.path(&update.path)?;
                    path = update.path.clone();
                }
                reporter.record(&observation, &update, course.goal, elapsed.as_micros())?;
                if last_print.elapsed() > Duration::from_secs(1) {
                    println!(
                        "t={:.1}s pose=({:.2},{:.2}) LiDAR={} points — {}{}",
                        observation.time_ns as f64 / 1e9,
                        observation.state.pose.position.x,
                        observation.state.pose.position.y,
                        observation.scan.points.len(),
                        update.status,
                        if update.saturated {
                            " (command limited)"
                        } else {
                            ""
                        }
                    );
                    last_print = Instant::now();
                }
                if update.finished {
                    break;
                }
            }
            if last_status.elapsed() > Duration::from_secs(1) {
                let status = sim.status()?;
                if status.error.is_some() {
                    return Err(error(format!("Simulator error: {:?}", status.error)));
                }
                if let Some(result) = status.run_result {
                    reporter.summary.termination_reason = result.termination_reason;
                    reporter.summary.simulator_passed = Some(result.passed);
                    reporter.summary.status = "simulator ended".into();
                    break;
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
                return Err(error("No fresh capture-aligned LiDAR/pose; stopped. Check simulator, orchestrator, and timestamps."));
            }
        }
        Ok(())
    })();
    let finished = sim.finish();
    if let Ok(snapshot) = &finished {
        if let Some(result) = &snapshot.run_result {
            reporter.summary.simulator_passed = Some(result.passed);
            reporter.summary.termination_reason = result.termination_reason.clone();
        }
    }
    if let Err(error) = &result {
        reporter.summary.status = error.to_string();
    }
    reporter.finish(&runner.perception.map, &path, course.goal)?;
    result?;
    finished?;
    if mode != Mode::Observe && mode != Mode::DriveTest && !reporter.summary.goal_reached {
        return Err(error(
            "Goal not reached; inspect the summary and implement the indicated student component",
        ));
    }
    Ok(())
}
