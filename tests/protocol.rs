use cev_onboarding::{
    protocol::{self, pb, Freshness},
    transport::ObservationJoin,
    types::*,
};
use prost::Message;
#[test]
fn exact_timestamps_and_physical_types_round_trip() {
    let observation: Observation =
        serde_json::from_str(include_str!("../fixtures/perception.json")).unwrap();
    let mut wire: pb::Observation = protocol::convert(&observation).unwrap();
    wire.time_ns = u64::MAX;
    let decoded: pb::Observation = protocol::decode(&wire.encode_to_vec()).unwrap();
    assert_eq!(decoded.time_ns, u64::MAX);
    let restored: Observation = protocol::convert(&decoded).unwrap();
    assert_eq!(restored.scan, observation.scan);
    let mut absent = decoded;
    absent.state = None;
    assert!(protocol::convert::<_, Observation>(&absent).is_err());
    assert!(protocol::decode::<pb::Observation>(&[255]).is_err());
}
#[test]
fn freshness_rejects_reordering_and_old_sources() {
    let mut f = Freshness::default();
    let h = protocol::header("run", 1, 100, "map");
    f.accept(&h).unwrap();
    assert!(f.fresh(100));
    assert!(!f.fresh(99));
    assert!(!f.fresh(300_000_101));
    assert!(f.accept(&h).is_err());
    assert!(f.accept(&protocol::header("run", 2, 99, "map")).is_err());
    assert!(protocol::check_header(&Some(h), "other", "map").is_err());
}
#[test]
fn observation_join_is_capture_exact_and_latest_only() {
    let observation: Observation =
        serde_json::from_str(include_str!("../fixtures/perception.json")).unwrap();
    let mut join = ObservationJoin::new();
    join.scan(
        protocol::header("run", 1, 100, "front_lidar_frame"),
        observation.scan.clone(),
    );
    join.state(protocol::header("run", 2, 101, "map"), observation.state);
    assert!(join.take().is_none());
    join.state(protocol::header("run", 1, 100, "map"), observation.state);
    assert_eq!(join.take().unwrap().2.time_ns, 100);
    assert!(join.take().is_none());
    for t in 101..200 {
        join.scan(
            protocol::header("run", t, t, "front_lidar_frame"),
            observation.scan.clone(),
        );
        join.state(protocol::header("run", t, t, "map"), observation.state);
    }
    assert_eq!(join.take().unwrap().2.time_ns, 199);
    assert!(join.take().is_none());
}
