use cev_onboarding::{
    grid::Grid,
    hardware::validate_hardware_spec,
    reporting::tracking_error,
    runner::{validate_path, Course},
    types::*,
};

fn course() -> Course {
    serde_json::from_str(include_str!("../scenarios/empty.json")).unwrap()
}
fn observation(time: u64) -> Observation {
    Observation {
        time_ns: time,
        state: VehicleState {
            pose: Pose2 {
                position: Point2 { x: 1.0, y: 0.0 },
                yaw_rad: 0.0,
            },
            speed_mps: 0.0,
        },
        scan: LidarScan {
            points: vec![],
            sensor_in_vehicle: course().vehicle.sensor_in_vehicle,
        },
    }
}

#[test]
fn fixtures_have_valid_contracts_and_do_not_claim_hardware_calibration() {
    for json in [
        include_str!("../scenarios/empty.json"),
        include_str!("../scenarios/detour.json"),
        include_str!("../scenarios/blocked.json"),
    ] {
        let course: Course = serde_json::from_str(json).unwrap();
        course.validate().unwrap();
        assert!(validate_hardware_spec(&course.vehicle).is_err());
        assert_eq!(course.resolved_hash.len(), 64);
    }
    let obs = observation(123);
    assert_eq!(
        serde_json::from_str::<Observation>(&serde_json::to_string(&obs).unwrap()).unwrap(),
        obs
    );
}

#[test]
fn grid_indexing_edges_and_clearance_use_whole_cells() {
    let mut grid = Grid::empty(Point2 { x: -1.0, y: -1.0 }, 0.1, 20, 20).unwrap();
    assert_eq!(grid.cell(Point2 { x: -1.0, y: -1.0 }), Some((0, 0)));
    assert_eq!(grid.cell(Point2 { x: 1.0, y: 0.0 }), None);
    assert_eq!(grid.cell(Point2 { x: -1.001, y: 0.0 }), None);
    assert_eq!(
        grid.cell(Point2 {
            x: f64::NAN,
            y: 0.0
        }),
        None
    );
    assert!(grid.blocked(20, 0));
    grid.set_occupied(10, 10, true);
    assert!(!grid.clear(Point2 { x: 0.11, y: 0.05 }, 0.02));
    assert!(grid.clear(Point2 { x: 0.3, y: 0.3 }, 0.05));
    assert!(!grid.segment_clear(Point2 { x: -0.5, y: 0.05 }, Point2 { x: 0.5, y: 0.05 }, 0.0));
    assert!(Grid::empty(Point2::default(), 0.0, 2, 2).is_err());
    assert!(Grid::empty(Point2::default(), 0.1, usize::MAX, usize::MAX).is_err());
}

#[test]
fn path_validator_catches_collisions_between_waypoints_and_bad_endpoints() {
    let mut map = Grid::empty(Point2::default(), 0.1, 50, 50).unwrap();
    let (start, goal) = (Point2 { x: 1.0, y: 1.0 }, Point2 { x: 4.0, y: 1.0 });
    validate_path(&map, &[start, goal], start, goal, 0.1, 0.2).unwrap();
    map.set_occupied(25, 10, true);
    assert!(validate_path(&map, &[start, goal], start, goal, 0.1, 0.2).is_err());
    assert!(validate_path(&map, &[], start, goal, 0.1, 0.2).is_err());
    assert!(validate_path(
        &map,
        &[
            start,
            Point2 {
                x: f64::NAN,
                y: 1.0
            }
        ],
        start,
        goal,
        0.1,
        0.2
    )
    .is_err());
    assert!(validate_path(&map, &[goal], start, goal, 0.1, 0.2).is_err());
}

#[test]
fn stop_defaults_nonfinite_rejection_and_command_limits() {
    let spec = course().vehicle;
    assert_eq!(spec.limit(DriveCommand::STOP).unwrap(), DriveCommand::STOP);
    assert_eq!(
        spec.limit(DriveCommand {
            speed_mps: 10.0,
            steering_rad: -5.0
        })
        .unwrap(),
        DriveCommand {
            speed_mps: 0.3,
            steering_rad: -0.5
        }
    );
    assert!(spec
        .limit(DriveCommand {
            speed_mps: f64::NAN,
            steering_rad: 0.0
        })
        .is_err());
    assert!(spec
        .limit(DriveCommand {
            speed_mps: 0.1,
            steering_rad: f64::INFINITY
        })
        .is_err());
}

#[test]
fn unfinished_algorithms_are_safe_and_independent() {
    use cev_onboarding::student::{
        controls::Controller, perception::Perception, planning::Planner,
    };
    let course = course();
    let obs = observation(100_000_000);
    assert!(Perception::new(course.grid.clone()).update(&obs).is_none());
    assert!(Planner::default()
        .plan(&course.grid, &obs.state, course.goal)
        .is_none());
    assert_eq!(
        Controller::new(course.vehicle).command(&obs.state, &[], 0.1),
        DriveCommand::STOP
    );
}

#[test]
fn transforms_and_metrics_use_segments() {
    let pose = Pose2 {
        position: Point2 { x: 1.0, y: 2.0 },
        yaw_rad: std::f64::consts::FRAC_PI_2,
    };
    let p = pose.transform(Point2 { x: 1.0, y: 0.0 });
    assert!((p.x - 1.0).abs() < 1e-10);
    assert!((p.y - 3.0).abs() < 1e-10);
    assert_eq!(
        tracking_error(
            Point2 { x: 5.0, y: 1.0 },
            &[Point2 { x: 0.0, y: 0.0 }, Point2 { x: 10.0, y: 0.0 }]
        ),
        Some(1.0)
    );
    assert_eq!(tracking_error(Point2::default(), &[]), None);
}
