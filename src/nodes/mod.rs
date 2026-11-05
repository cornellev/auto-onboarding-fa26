//! Process bootstrap. Student transport loops live in planning.rs and controls.rs.
pub mod controls;
pub mod perception;
pub mod planning;
use crate::{
    protocol::{self, pb},
    transport::{self, Subscription, Transport},
    Result,
};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

pub struct Context {
    pub transport: Transport,
    pub info: pb::RunInfo,
    pub interrupted: Arc<AtomicBool>,
    info_sub: Subscription,
    last_info: Instant,
    status_sequence: u64,
    node: String,
}
impl Context {
    pub fn connect(node: &str, run: &str, interrupted: Arc<AtomicBool>) -> Result<Self> {
        let transport = Transport::connect(run)?;
        let info_sub = transport.subscribe("run_info", 1)?;
        let deadline = Instant::now() + Duration::from_secs(10);
        let info = loop {
            if interrupted.load(Ordering::SeqCst) {
                return Err(crate::error("Node interrupted during startup"));
            }
            if let Some(info) =
                transport::receive::<pb::RunInfo>(&info_sub, Duration::from_millis(100))?
            {
                break info;
            }
            if Instant::now() > deadline {
                return Err(crate::error("No run_info; start a prepared run first"));
            }
        };
        protocol::check_header(&info.header, run, "map")?;
        if info.schema_hash != protocol::fingerprint() {
            return Err(crate::error("Node schema differs from run schema"));
        }
        let vehicle: crate::types::VehicleSpec = protocol::convert(
            info.vehicle
                .as_ref()
                .ok_or_else(|| crate::error("Missing vehicle spec"))?,
        )?;
        vehicle.validate()?;
        let grid: crate::grid::Grid = protocol::convert(
            info.grid
                .as_ref()
                .ok_or_else(|| crate::error("Missing grid"))?,
        )?;
        grid.validate()?;
        Ok(Self {
            transport,
            info,
            interrupted,
            info_sub,
            last_info: Instant::now(),
            status_sequence: 0,
            node: node.into(),
        })
    }
    pub fn poll(&mut self) -> Result<bool> {
        if self.interrupted.load(Ordering::SeqCst) {
            return Ok(false);
        }
        if let Some(info) = transport::receive::<pb::RunInfo>(&self.info_sub, Duration::ZERO)? {
            if info != self.info {
                return Err(crate::error("Run configuration changed; start a fresh run"));
            }
            self.last_info = Instant::now();
        }
        if self.transport.session.is_closed()
            || self.last_info.elapsed() > Duration::from_millis(1200)
        {
            return Err(crate::error("Run/router lost; no automatic resume"));
        }
        Ok(true)
    }
    pub fn status(&mut self, ready: bool, detail: &str, time_ns: u64) -> Result<()> {
        self.status_sequence += 1;
        self.transport.publish(
            &format!("status/{}", self.node),
            &pb::NodeStatus {
                header: Some(protocol::header(
                    &self.transport.run,
                    self.status_sequence,
                    time_ns,
                    "map",
                )),
                node: self.node.clone(),
                ready,
                detail: detail.into(),
                pid: std::process::id(),
            },
        )
    }
    pub fn wait_for_wiring(&mut self, detail: &str) -> Result<()> {
        println!("{}: {detail}", self.node);
        while self.poll()? {
            self.status(false, detail, 0)?;
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(())
    }
}

/// Provided publication helper: validates the entire path and retains source age.
pub fn planned_update(
    ctx: &Context,
    map: &pb::MapUpdate,
    planner: &mut crate::student::planning::Planner,
) -> Result<pb::PathUpdate> {
    let h = protocol::check_header(&map.header, &ctx.transport.run, "map")?;
    let source = protocol::check_header(&map.source, &ctx.transport.run, "map")?;
    let mut out = pb::PathUpdate {
        header: Some(h.clone()),
        source: Some(source),
        map_sequence: h.sequence,
        path: None,
        availability: pb::Availability::NotReady as i32,
    };
    if map.availability != pb::Availability::Ready as i32 {
        return Ok(out);
    }
    let grid: crate::grid::Grid = protocol::convert(
        map.grid
            .as_ref()
            .ok_or_else(|| crate::error("Missing ready map"))?,
    )?;
    grid.validate()?;
    let state: crate::types::VehicleState = protocol::convert(
        map.state
            .as_ref()
            .ok_or_else(|| crate::error("Missing capture state"))?,
    )?;
    let goal = protocol::convert(
        ctx.info
            .goal
            .as_ref()
            .ok_or_else(|| crate::error("Missing goal"))?,
    )?;
    let spec: crate::types::VehicleSpec = protocol::convert(
        ctx.info
            .vehicle
            .as_ref()
            .ok_or_else(|| crate::error("Missing vehicle"))?,
    )?;
    match planner.plan(&grid, &state, goal) {
        Some(path) => {
            crate::runner::validate_path(
                &grid,
                &path,
                state.pose.position,
                goal,
                spec.clearance_m(),
                ctx.info.goal_tolerance_m,
            )?;
            out.path = Some(pb::Path {
                points: protocol::convert(&path)?,
            });
            out.availability = pb::Availability::Ready as i32;
        }
        None => out.availability = pb::Availability::NoPath as i32,
    }
    Ok(out)
}

/// Provided command helper. Republishing an old path never refreshes its deadline.
pub fn controlled_update(
    ctx: &Context,
    state: &pb::StateUpdate,
    path: Option<&pb::PathUpdate>,
    controller: &mut crate::student::controls::Controller,
    sequence: u64,
    dt_s: f64,
) -> Result<pb::CommandUpdate> {
    let (h, state) = protocol::state(state, &ctx.transport.run)?;
    let mut out = pb::CommandUpdate {
        header: Some(protocol::header(
            &ctx.transport.run,
            sequence,
            h.time_ns,
            "map",
        )),
        state_sequence: h.sequence,
        command: Some(protocol::convert(&crate::types::DriveCommand::STOP)?),
        ..Default::default()
    };
    let Some(path) = path else {
        return Ok(out);
    };
    if path.availability != pb::Availability::Ready as i32 {
        return Ok(out);
    }
    let path_h = protocol::check_header(&path.header, &ctx.transport.run, "map")?;
    let source = protocol::check_header(&path.source, &ctx.transport.run, "map")?;
    if !dt_s.is_finite()
        || dt_s <= 0.
        || h.time_ns
            .checked_sub(source.time_ns)
            .is_none_or(|age| age >= protocol::FRESH_NS)
    {
        return Ok(out);
    }
    let points: Vec<crate::types::Point2> = protocol::convert(
        &path
            .path
            .as_ref()
            .ok_or_else(|| crate::error("Missing ready path"))?
            .points,
    )?;
    if points.is_empty() || points.len() > 100_000 || points.iter().any(|p| !p.is_finite()) {
        return Err(crate::error("Invalid path points"));
    }
    let command = controller.command(&state, &points, dt_s);
    // Keep finite requests for saturation reporting; the actuator boundary clamps them.
    controller.vehicle.limit(command)?;
    out.command = Some(protocol::convert(&command)?);
    out.source = Some(source.clone());
    out.path_sequence = path_h.sequence;
    out.valid_until_ns = source.time_ns + protocol::FRESH_NS;
    Ok(out)
}

impl Drop for Context {
    fn drop(&mut self) {
        if self.node == "controls" {
            let _ = self.transport.publish(
                "command",
                &pb::CommandUpdate {
                    header: Some(protocol::header(&self.transport.run, u64::MAX, 0, "map")),
                    command: Some(pb::DriveCommand::default()),
                    ..Default::default()
                },
            );
        }
    }
}
