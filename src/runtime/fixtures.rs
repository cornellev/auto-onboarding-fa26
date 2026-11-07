//! Transport exercises use their own namespace and have no actuator binding.
use crate::{
    shared::grid::Grid,
    shared::protocol::{self, pb},
    shared::transport::Transport,
    shared::types::*,
    Result,
};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
/**
 * Publishes transport fixtures at 10 Hz without a simulator or actuator binding.
 * @param role The component to exercise: perception, planning, or controls.
 * @param run The fixture run ID, or None to generate a new UUID.
 * @param duration The publication duration in seconds, or None for 60 seconds.
 * @param interrupted The shared flag used to stop publication early.
 * @return Ok(()) when the duration expires or interruption is requested, or a setup/publication error.
 */
pub fn publish(
    role: &str,
    run: Option<String>,
    duration: Option<f64>,
    interrupted: Arc<AtomicBool>,
) -> Result<()> {
    if !matches!(role, "perception" | "planning" | "controls") {
        return Err(crate::error("Unknown fixture role"));
    }
    let run = run.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let io = Transport::connect(&run)?;
    let course: crate::runtime::runner::Course =
        serde_json::from_str(include_str!("../../scenarios/empty.json"))?;
    let info = pb::RunInfo {
        header: Some(protocol::header(&run, 1, 0, "map")),
        schema_hash: protocol::fingerprint(),
        goal: Some(protocol::convert(&course.goal)?),
        vehicle: Some(protocol::convert(&course.vehicle)?),
        grid: Some(protocol::convert(&course.grid)?),
        goal_tolerance_m: course.goal_tolerance_m,
        timeout_s: duration.unwrap_or(60.),
        mode: "transport-fixture".into(),
        control_path: if role == "controls" {
            Some(pb::Path {
                points: protocol::convert(&course.control_path)?,
            })
        } else {
            None
        },
    };
    let observation: Observation =
        serde_json::from_str(include_str!("../../fixtures/perception/observation.json"))?;
    #[derive(serde::Deserialize)]
    struct Planning {
        grid: Grid,
        start: VehicleState,
    }
    let planning: Planning =
        serde_json::from_str(include_str!("../../fixtures/planning/empty.json"))?;
    println!(
        "Fixture run: {run}; no simulator or actuator connection. Start node {role} --run {run}"
    );
    let start = Instant::now();
    let mut seq = 0;
    while !interrupted.load(Ordering::SeqCst)
        && start.elapsed().as_secs_f64() < duration.unwrap_or(60.)
    {
        seq += 1;
        let time = seq * 100_000_000;
        let h = protocol::header(&run, seq, time, "map");
        io.publish("run_info", &info)?;
        match role {
            "perception" => {
                io.publish(
                    "state",
                    &pb::StateUpdate {
                        header: Some(h.clone()),
                        state: Some(protocol::convert(&observation.state)?),
                    },
                )?;
                io.publish(
                    "lidar",
                    &pb::LidarUpdate {
                        header: Some(protocol::header(&run, seq, time, "front_lidar_frame")),
                        scan: Some(protocol::convert(&observation.scan)?),
                    },
                )?;
            }
            "planning" => io.publish(
                "map",
                &pb::MapUpdate {
                    header: Some(h.clone()),
                    source: Some(h),
                    state: Some(protocol::convert(&planning.start)?),
                    grid: Some(protocol::convert(&planning.grid)?),
                    availability: pb::Availability::Ready as i32,
                },
            )?,
            "controls" => {
                io.publish(
                    "state",
                    &pb::StateUpdate {
                        header: Some(h.clone()),
                        state: Some(protocol::convert(&planning.start)?),
                    },
                )?;
                io.publish(
                    "path",
                    &pb::PathUpdate {
                        header: Some(h.clone()),
                        source: Some(h),
                        map_sequence: seq,
                        path: info.control_path.clone(),
                        availability: pb::Availability::Ready as i32,
                    },
                )?;
            }
            _ => unreachable!(),
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}
