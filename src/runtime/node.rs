//! How a node starts, notices the run is gone, and says what it's doing.
use crate::{
    shared::protocol::{self, pb},
    shared::transport::{self, Subscription, Transport},
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
    /// Connect and wait up to 10 seconds for this run's configuration.
    /// Checks the run id, the schema, the vehicle, and the grid.
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
        let vehicle: crate::shared::types::VehicleSpec = protocol::convert(
            info.vehicle
                .as_ref()
                .ok_or_else(|| crate::error("Missing vehicle spec"))?,
        )?;
        vehicle.validate()?;
        let grid: crate::shared::grid::Grid = protocol::convert(
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
    /// `true` while the run is still there. `false` if you were asked to stop.
    /// Errors if the router disappears or the run's heartbeat is older than 1.2 seconds.
    /// A changed run configuration is also an error. Start a new run instead of continuing.
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
    /// Tell the launcher whether this node is ready, and a short reason.
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
    /// Sit in a loop publishing "not ready" until the run stops. `detail` is the message we print.
    pub fn wait_for_wiring(&mut self, detail: &str) -> Result<()> {
        println!("{}: {detail}", self.node);
        while self.poll()? {
            self.status(false, detail, 0)?;
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(())
    }
}

impl Drop for Context {
    /// If this was the controls node, try to publish a stop on the way out.
    /// A failure here is ignored, because dropping a value can't return an error.
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
