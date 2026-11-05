use cev_onboarding::{student::controls::Controller, types::*};

#[test]
#[ignore = "Student acceptance: implement Controller::command, then run cargo test --test controls -- --ignored"]
fn empty_path_stops_and_steering_corrects_both_sides() {
    let spec: VehicleSpec = serde_json::from_str(include_str!("../fixtures/vehicle.json")).unwrap();
    let mut controller = Controller::new(spec.clone());
    let mut state = VehicleState::default();
    assert_eq!(controller.command(&state, &[], 0.1), DriveCommand::STOP);
    let path = [
        Point2 { x: 0.0, y: 0.0 },
        Point2 { x: 1.0, y: 0.0 },
        Point2 { x: 3.0, y: 0.0 },
    ];
    for y in [-0.2, 0.2] {
        state.pose.position.y = y;
        let command = controller.command(&state, &path, 0.1);
        assert!(command.speed_mps > 0.0 && command.speed_mps <= spec.max_speed_mps);
        assert!(
            command.steering_rad.is_finite() && command.steering_rad.abs() <= spec.max_steering_rad
        );
        assert!(
            command.steering_rad * y < 0.0,
            "Steer back toward the path; positive steering is left"
        );
    }
}
