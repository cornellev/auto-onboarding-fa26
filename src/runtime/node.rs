//! Shared process startup, run identity, status, and shutdown.
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
    /**
     * Connects a node and waits up to 10 seconds for a valid run configuration.
     * Validates the run header, schema fingerprint, vehicle specification, and grid.
     * @param node The node name used for status publication and shutdown handling.
     * @param run The expected run ID and transport namespace.
     * @param interrupted The shared flag used to cancel startup and later polling.
     * @return The connected context, or a startup, transport, or validation error.
     */
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
    /**
     * Checks interruption, configuration stability, and the run heartbeat.
     * A closed session or a heartbeat older than 1.2 seconds ends the run with an error.
     * @return True while the node may continue, false on interruption, or a run/transport error.
     */
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
    /**
     * Publishes this node's status with a new sequence number and its process ID.
     * @param ready Whether the node is ready to provide its output.
     * @param detail The human-readable status message.
     * @param time_ns The status timestamp in simulation nanoseconds.
     * @return Ok(()) if publication succeeds, or a transport error.
     */
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
    /**
     * Keeps an unfinished node alive, publishing not-ready status every 100 milliseconds.
     * @param detail The wiring instructions printed locally and included in status messages.
     * @return Ok(()) on interruption, or a polling or status-publication error.
     */
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
    /**
     * Attempts to publish a final STOP command when the controls context is dropped.
     * Publication errors are ignored because cleanup cannot return an error.
     */
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
