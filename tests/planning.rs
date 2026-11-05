use cev_onboarding::{grid::Grid, runner::validate_path, student::planning::Planner, types::*};
use serde::Deserialize;
#[derive(Deserialize)]
struct Fixture {
    grid: Grid,
    start: VehicleState,
    goal: Point2,
    vehicle: VehicleSpec,
}

#[test]
#[ignore = "Student acceptance: implement Planner::plan, then run cargo test --test planning -- --ignored"]
fn reachable_courses_produce_valid_paths() {
    for json in [
        include_str!("../fixtures/planning-empty.json"),
        include_str!("../fixtures/planning-detour.json"),
    ] {
        let f: Fixture = serde_json::from_str(json).unwrap();
        let path = Planner::default()
            .plan(&f.grid, &f.start, f.goal)
            .expect("A reachable goal needs a path");
        validate_path(
            &f.grid,
            &path,
            f.start.pose.position,
            f.goal,
            f.vehicle.clearance_m(),
            0.2,
        )
        .unwrap();
    }
}
#[test]
#[ignore = "Student acceptance: implement Planner::plan"]
fn blocked_goal_returns_none() {
    let f: Fixture =
        serde_json::from_str(include_str!("../fixtures/planning-blocked.json")).unwrap();
    assert!(Planner::default().plan(&f.grid, &f.start, f.goal).is_none());
}
