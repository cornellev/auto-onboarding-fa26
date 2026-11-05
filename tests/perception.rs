use cev_onboarding::{runner::Course, student::perception::Perception, types::*};

#[test]
#[ignore = "Student acceptance: implement Perception::update, then run cargo test --test perception -- --ignored"]
fn rotated_mount_duplicate_hits_and_empty_scans() {
    let course: Course = serde_json::from_str(include_str!("../scenarios/empty.json")).unwrap();
    let mut observation: Observation =
        serde_json::from_str(include_str!("../fixtures/perception.json")).unwrap();
    let mut perception = Perception::new(course.grid);
    let map = perception
        .update(&observation)
        .expect("Return your map after processing the scan");
    for point in [Point2 { x: 1.0, y: 1.1 }, Point2 { x: 2.0, y: 2.1 }] {
        let (col, row) = map.cell(point).unwrap();
        assert!(map.blocked(col, row), "Expected obstacle near {point:?}");
    }
    assert!(!map.blocked(0, 0), "Do not fill unrelated cells");
    observation.time_ns += 100_000_000;
    observation.scan.points.clear();
    let map = perception
        .update(&observation)
        .expect("An empty scan must not panic");
    map.validate().unwrap();
    observation.time_ns += 100_000_000;
    observation.scan.points = vec![Point2 { x: 1000.0, y: 0.0 }];
    perception
        .update(&observation)
        .expect("Ignore points outside the map")
        .validate()
        .unwrap();
}
