use cev_onboarding::{
    shared::protocol::pb,
    shared::transport::{self, Transport},
};
use std::{
    process::{Child, Command},
    time::{Duration, Instant},
};
struct Processes(Vec<Child>);
impl Drop for Processes {
    fn drop(&mut self) {
        for p in &mut self.0 {
            let _ = p.kill();
            let _ = p.wait();
        }
    }
}
#[test]
#[ignore = "Live local zenohd required; checks the provided node bootstrap"]
fn three_processes_bootstrap_and_perception_publishes_unavailable_map() {
    let run = uuid::Uuid::new_v4().to_string();
    let t = Transport::connect(&run).unwrap();
    let map = t.subscribe("map", 1).unwrap();
    let statuses = ["perception", "planning", "controls"]
        .map(|n| t.subscribe(&format!("status/{n}"), 1).unwrap());
    let exe = env!("CARGO_BIN_EXE_cev-onboarding");
    let mut processes = Processes(vec![Command::new(exe)
        .args([
            "fixture-publish",
            "perception",
            "--run",
            &run,
            "--duration",
            "8",
        ])
        .spawn()
        .unwrap()]);
    let mut ids = std::collections::BTreeSet::new();
    for node in ["perception", "planning", "controls"] {
        let child = Command::new(exe)
            .args(["node", node, "--run", &run])
            .spawn()
            .unwrap();
        ids.insert(child.id());
        processes.0.push(child);
    }
    assert_eq!(ids.len(), 3);
    for status in statuses {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(value) =
                transport::receive::<pb::NodeStatus>(&status, Duration::from_millis(100)).unwrap()
            {
                assert!(ids.contains(&value.pid));
                break;
            }
            assert!(Instant::now() < deadline, "No node status");
        }
    }
    let value = transport::receive::<pb::MapUpdate>(&map, Duration::from_secs(3))
        .unwrap()
        .unwrap();
    assert_eq!(value.availability, pb::Availability::NotReady as i32);
    assert!(value.grid.is_none());
}
#[test]
#[ignore = "Student wiring acceptance: implement planning transport; requires local zenohd"]
fn planning_wiring_publishes_no_path_before_algorithm_is_implemented() {
    let run = uuid::Uuid::new_v4().to_string();
    let t = Transport::connect(&run).unwrap();
    let paths = t.subscribe("path", 1).unwrap();
    let exe = env!("CARGO_BIN_EXE_cev-onboarding");
    let _processes = Processes(vec![
        Command::new(exe)
            .args([
                "fixture-publish",
                "planning",
                "--run",
                &run,
                "--duration",
                "8",
            ])
            .spawn()
            .unwrap(),
        Command::new(exe)
            .args(["node", "planning", "--run", &run])
            .spawn()
            .unwrap(),
    ]);
    let update = transport::receive::<pb::PathUpdate>(&paths, Duration::from_secs(5))
        .unwrap()
        .expect("Wire the planner to publish a result, including NO_PATH");
    assert!(matches!(update.availability, 2 | 3));
}
#[test]
#[ignore = "Student wiring acceptance: implement controls transport; requires local zenohd"]
fn controls_wiring_publishes_a_stop_before_algorithm_is_implemented() {
    let run = uuid::Uuid::new_v4().to_string();
    let t = Transport::connect(&run).unwrap();
    let commands = t.subscribe("command", 1).unwrap();
    let exe = env!("CARGO_BIN_EXE_cev-onboarding");
    let _processes = Processes(vec![
        Command::new(exe)
            .args([
                "fixture-publish",
                "controls",
                "--run",
                &run,
                "--duration",
                "8",
            ])
            .spawn()
            .unwrap(),
        Command::new(exe)
            .args(["node", "controls", "--run", &run])
            .spawn()
            .unwrap(),
    ]);
    let update = transport::receive::<pb::CommandUpdate>(&commands, Duration::from_secs(5))
        .unwrap()
        .expect("Wire the controller to publish commands");
    let command = update.command.unwrap();
    assert!(command.speed_mps.is_finite() && command.steering_rad.is_finite());
    assert!((0.0..=0.3).contains(&command.speed_mps));
}
