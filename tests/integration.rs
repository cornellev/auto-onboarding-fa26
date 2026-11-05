use serde_json::Value;
use std::{fs, process::Command};

fn run(args: &[&str]) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cev-onboarding"));
    command.args(args);
    if let Ok(server) = std::env::var("CEV_SIM_SERVER") {
        command.args(["--server", &server]);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    let directory = text
        .lines()
        .find_map(|line| line.strip_prefix("Results: "))
        .expect("Results directory");
    serde_json::from_slice(&fs::read(format!("{directory}/summary.json")).unwrap()).unwrap()
}

#[test]
fn offline_modes_work_before_student_algorithms() {
    for mode in ["perception", "planning"] {
        let summary = run(&[mode]);
        assert!(!summary["goal_reached"].as_bool().unwrap());
        assert!(summary["collisions"].is_null());
    }
}

#[test]
#[ignore = "Live simulator + orchestrator + imported bundles + one open page required; run with --test-threads=1"]
fn browser_observation_drive_stop_and_fresh_run() {
    let observe = run(&["observe", "--duration", "2"]);
    assert_eq!(observe["traveled_m"], 0.0);
    let drive = run(&["drive-test"]);
    assert!(drive["traveled_m"].as_f64().unwrap() > 0.01);
    let observe = run(&["observe", "--duration", "2"]);
    assert_eq!(observe["traveled_m"], 0.0);
}

#[test]
#[ignore = "Student end-to-end acceptance; requires completed algorithms and a live browser; run with --test-threads=1"]
fn student_navigation_reaches_empty_and_detour_without_scenario_failure() {
    for scenario in ["empty", "detour"] {
        let summary = run(&["autonomous", "--scenario", scenario]);
        assert_eq!(summary["goal_reached"], true);
        assert_ne!(summary["termination_reason"], "ego-collision");
    }
}

#[test]
#[ignore = "Student controls acceptance; requires a live browser; run with --test-threads=1"]
fn student_follows_straight_and_gentle_curve() {
    for args in [
        vec!["controls"],
        vec!["controls", "--path", "fixtures/path-curve.json"],
    ] {
        let summary = run(&args);
        assert_eq!(summary["goal_reached"], true);
        assert_ne!(summary["termination_reason"], "ego-collision");
        assert!(summary["mean_tracking_error_m"].as_f64().unwrap() < 0.3);
    }
}

#[test]
#[ignore = "Live safety test; requires one browser and installed courses; run with --test-threads=1"]
fn candidate_watchdog_stops_when_commands_stop() {
    use cev_onboarding::{hardware::VehicleIo, runner::Course, sim::Sim, types::*};
    use std::time::{Duration, Instant};
    let course: Course = serde_json::from_str(include_str!("../scenarios/empty.json")).unwrap();
    let server = std::env::var("CEV_SIM_SERVER").unwrap_or_else(|_| "http://localhost:3000".into());
    let mut sim = Sim::launch(&course, &server).unwrap();
    let next = |sim: &mut Sim| {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(obs) = sim.observe(Duration::from_millis(100)).unwrap() {
                return obs;
            }
            assert!(Instant::now() < deadline, "No fresh observation");
        }
    };
    let start = next(&mut sim);
    for _ in 0..5 {
        next(&mut sim);
        sim.drive(DriveCommand {
            speed_mps: 0.1,
            steering_rad: 0.0,
        })
        .unwrap();
    }
    let moving = next(&mut sim);
    assert!(
        moving
            .state
            .pose
            .position
            .distance(start.state.pose.position)
            > 0.01
    );
    // Keep receiving data but send no command: the simulator, not Rust cleanup, must stop motion.
    let mut stopped = moving.clone();
    while stopped.time_ns < moving.time_ns + 1_000_000_000 {
        stopped = next(&mut sim);
    }
    assert!(
        stopped.state.speed_mps.abs() < 0.01,
        "Watchdog failed to stop the candidate"
    );
    sim.finish().unwrap();
}

#[test]
#[ignore = "Student integration acceptance for unreachable goals; requires live browser; run with --test-threads=1"]
fn blocked_course_stays_stopped() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cev-onboarding"));
    command.args(["autonomous", "--scenario", "blocked", "--duration", "3"]);
    if let Ok(server) = std::env::var("CEV_SIM_SERVER") {
        command.args(["--server", &server]);
    }
    let output = command.output().unwrap();
    assert!(
        !output.status.success(),
        "An unreachable goal must not report success"
    );
    let text = String::from_utf8(output.stdout).unwrap();
    let directory = text
        .lines()
        .find_map(|line| line.strip_prefix("Results: "))
        .expect("Results directory");
    let summary: Value =
        serde_json::from_slice(&fs::read(format!("{directory}/summary.json")).unwrap()).unwrap();
    assert_eq!(summary["goal_reached"], false);
    assert!(summary["traveled_m"].as_f64().unwrap() < 0.01);
}
