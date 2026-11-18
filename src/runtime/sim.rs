use crate::{
    perception::synchronizer::ObservationJoin,
    runtime::hardware::VehicleIo,
    runtime::runner::Course,
    shared::protocol::{self, pb},
    shared::transport::{self, Subscription, Transport},
    shared::types::*,
    Result,
};
use std::{
    cell::Cell,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

const MANIFEST_ID: &str = "onboarding";
const RENEW_PERIOD: Duration = Duration::from_millis(200);
const RENEW_TIMEOUT: Duration = Duration::from_millis(300);
#[cfg(test)]
const LEASE_TTL: Duration = Duration::from_millis(750);

/// Wait this long before the next renew. The period is measured from the attempt start,
/// so a call that already used the 300 ms timeout does not add another 200 ms.
fn renew_delay(elapsed: Duration) -> Duration {
    RENEW_PERIOD.saturating_sub(elapsed)
}

/**
 * Checks that the simulator already has the saved onboarding run.
 * Does not install or replace that run. The world lives in the simulator.
 * @param server The simulator server's base URL.
 * @return Ok(()) if the manifest is present, or an error if the server or the manifest is missing.
 */
pub fn setup(server: &str) -> Result<()> {
    simcontrol::Client::new(server)?;
    onboarding_manifest(server)?;
    println!("Run manifest {MANIFEST_ID} is installed");
    Ok(())
}

/// The stored run document. A missing manifest is an error, not something this repo creates.
fn onboarding_manifest(server: &str) -> Result<serde_json::Value> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .max_redirects(0)
        .timeout_global(Some(Duration::from_secs(15)))
        .build()
        .into();
    let url = format!(
        "{}/api/storage/run-manifests/{MANIFEST_ID}",
        server.trim_end_matches('/')
    );
    let mut response = agent.get(&url).call()?;
    let status = response.status().as_u16();
    if status == 404 {
        return Err(crate::error(
            "Run manifest onboarding is not installed in the simulator",
        ));
    }
    let value: serde_json::Value = response.body_mut().read_json()?;
    if status >= 400 {
        return Err(crate::error(format!(
            "Storage API /run-manifests/{MANIFEST_ID}: {value}"
        )));
    }
    Ok(value)
}

/// The simulator accepts observe, autonomous, and drive-test. Controls drives the car, so it uses autonomous.
fn lease_mode(mode: &str) -> &'static str {
    match mode {
        "observe" => "observe",
        "drive-test" => "drive-test",
        _ => "autonomous",
    }
}

fn acquire_lease(server: &str, browser_id: &str, mode: &str) -> Result<(String, String)> {
    let value = request(
        "POST",
        &format!("{server}/api/interfaces/control-ownership"),
        Some(&serde_json::json!({
            "target": {"kind": "browser", "id": browser_id, "vehicleId": "ego"},
            "mode": lease_mode(mode)
        })),
    )?;
    let id = value["leaseId"]
        .as_str()
        .ok_or_else(|| crate::error("Control lease response is missing leaseId"))?
        .to_string();
    let token = value["token"]
        .as_str()
        .ok_or_else(|| crate::error("Control lease response is missing token"))?
        .to_string();
    Ok((id, token))
}

fn renew_lease(server: &str, lease_id: &str, token: &str) -> Result<()> {
    request_with_timeout(
        "POST",
        &format!("{server}/api/interfaces/control-ownership/{lease_id}/renew"),
        Some(&serde_json::json!({})),
        Some(token),
        RENEW_TIMEOUT,
    )?;
    Ok(())
}

fn release_lease(server: &str, lease_id: &str, token: &str) {
    let _ = request_with_token(
        "DELETE",
        &format!("{server}/api/interfaces/control-ownership/{lease_id}"),
        None,
        Some(token),
    );
}

fn halt_renewal(
    renew_stop: &AtomicBool,
    renew_thread: &mut Option<JoinHandle<()>>,
    server: &str,
    lease_id: &str,
    token: &str,
) {
    renew_stop.store(true, Ordering::SeqCst);
    if let Some(thread) = renew_thread.take() {
        let _ = thread.join();
    }
    release_lease(server, lease_id, token);
}

fn blocks_next_launch(status: &str) -> bool {
    matches!(status, "running" | "paused" | "ready" | "preparing")
}

fn stop_released_run(server: &str, browser_id: &str, run_id: &str) -> Result<simcontrol::Snapshot> {
    let receipt = request(
        "POST",
        &format!("{server}/api/browser-sim/commands"),
        Some(&serde_json::json!({
            "action": "stop",
            "browserId": browser_id,
            "runId": run_id,
        })),
    )?;
    let command_id = receipt["commandId"]
        .as_str()
        .ok_or_else(|| crate::error("Stop did not return a command"))?
        .to_string();
    for _ in 0..50 {
        let receipt = request(
            "GET",
            &format!("{server}/api/browser-sim/commands/{command_id}"),
            None,
        )?;
        match receipt["state"].as_str() {
            Some("succeeded") => {
                return serde_json::from_value(receipt["snapshot"].clone())
                    .map_err(|err| crate::error(format!("Stop snapshot: {err}")));
            }
            Some("failed" | "unknown") => {
                return Err(crate::error(format!("Stop failed: {receipt}")));
            }
            _ => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    Err(crate::error("Stop did not finish"))
}

/// The browser pauses when the lease ends, which puts a tab back into a state that blocks the next launch.
/// Stop only after that pause, and without the released lease, or the pause undoes the stop.
fn stop_after_release(
    server: &str,
    browser_id: &str,
    run_id: &str,
) -> Result<simcontrol::Snapshot> {
    let client = simcontrol::Client::with_timeout(server, Duration::from_secs(10))?;
    let mut sent = false;
    for _ in 0..40 {
        let snapshot = client.get_status(Some(browser_id))?.snapshot;
        if !blocks_next_launch(&snapshot.status) {
            return Ok(snapshot);
        }
        if snapshot.status == "paused" && !sent {
            let stopped = stop_released_run(server, browser_id, run_id)?;
            sent = true;
            if !blocks_next_launch(&stopped.status) {
                return Ok(stopped);
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(crate::error(
        "The simulator tab stayed paused after the run",
    ))
}

/// A lease expires 750 ms after the last renewal, including while the browser is still launching.
/// Each attempt gives up after 300 ms, and the next one starts 200 ms after the previous start.
fn spawn_lease_renewal(
    server: String,
    lease_id: String,
    token: String,
    stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        while !stop.load(Ordering::SeqCst) {
            let started = Instant::now();
            let _ = renew_lease(&server, &lease_id, &token);
            let delay = renew_delay(started.elapsed());
            let mut waited = Duration::ZERO;
            while waited < delay {
                if stop.load(Ordering::SeqCst) {
                    return;
                }
                let slice = Duration::from_millis(10).min(delay - waited);
                std::thread::sleep(slice);
                waited += slice;
            }
        }
    })
}

/**
 * Sends a JSON request to CEV_ORCHESTRATOR_API, defaulting to http://localhost:8090.
 * Uses CEV_API_TOKEN for bearer authentication when it is set.
 * @param method The HTTP method; GET, DELETE, and PUT are explicit, with POST as the fallback.
 * @param path The API path beginning with a slash.
 * @param value The optional JSON body for PUT or POST requests.
 * @return The JSON response, or a transport, JSON-decoding, or HTTP error.
 */
pub fn api(
    method: &str,
    path: &str,
    value: Option<&serde_json::Value>,
) -> Result<serde_json::Value> {
    let base =
        std::env::var("CEV_ORCHESTRATOR_API").unwrap_or_else(|_| "http://localhost:8090".into());
    let token = std::env::var("CEV_API_TOKEN").ok();
    request_with_token(
        method,
        &format!("{}{path}", base.trim_end_matches('/')),
        value,
        token.as_deref(),
    )
}
/**
 * Sends a JSON request without a bearer token, including to simulator storage.
 * @param method The HTTP method; GET, DELETE, and PUT are explicit, with POST as the fallback.
 * @param url The complete request URL.
 * @param value The optional JSON body for PUT or POST requests.
 * @return The JSON response, or a transport, JSON-decoding, or HTTP error.
 */
fn request(
    method: &str,
    url: &str,
    value: Option<&serde_json::Value>,
) -> Result<serde_json::Value> {
    request_with_token(method, url, value, None)
}
/**
 * Sends a JSON request with a 10-second timeout and redirects disabled.
 * @param method The HTTP method; GET, DELETE, and PUT are explicit, with POST as the fallback.
 * @param url The complete request URL.
 * @param value The JSON body for PUT or POST; None sends an empty object.
 * @param token The optional bearer token; None sends an empty Authorization header.
 * @return The JSON response, or an error on transport failure, invalid JSON, or HTTP status >= 400.
 */
fn request_with_token(
    method: &str,
    url: &str,
    value: Option<&serde_json::Value>,
    token: Option<&str>,
) -> Result<serde_json::Value> {
    request_with_timeout(method, url, value, token, Duration::from_secs(10))
}

fn request_with_timeout(
    method: &str,
    url: &str,
    value: Option<&serde_json::Value>,
    token: Option<&str>,
    timeout: Duration,
) -> Result<serde_json::Value> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .max_redirects(0)
        .timeout_global(Some(timeout))
        .build()
        .into();
    let mut response = match method {
        "GET" => agent
            .get(url)
            .header(
                "Authorization",
                &token.map(|s| format!("Bearer {s}")).unwrap_or_default(),
            )
            .call()?,
        "DELETE" => agent
            .delete(url)
            .header(
                "Authorization",
                &token.map(|s| format!("Bearer {s}")).unwrap_or_default(),
            )
            .call()?,
        "PUT" => agent
            .put(url)
            .header(
                "Authorization",
                &token.map(|s| format!("Bearer {s}")).unwrap_or_default(),
            )
            .send_json(value.unwrap_or(&serde_json::json!({})))?,
        _ => agent
            .post(url)
            .header(
                "Authorization",
                &token.map(|s| format!("Bearer {s}")).unwrap_or_default(),
            )
            .send_json(value.unwrap_or(&serde_json::json!({})))?,
    };
    let status = response.status().as_u16();
    let body: serde_json::Value = response.body_mut().read_json()?;
    if status >= 400 {
        return Err(crate::error(format!("{method} {url}: {body}")));
    }
    Ok(body)
}

/**
 * Registers the bundled Protobuf profile after checking its schema fingerprint.
 * @return Ok(()) if registration succeeds, or a profile, fingerprint, or API error.
 */
pub fn setup_profile() -> Result<()> {
    let profile: serde_json::Value =
        serde_json::from_str(include_str!("../../config/protobuf-profile.json"))?;
    if profile["fingerprint"].as_str() != Some(&protocol::fingerprint()) {
        return Err(crate::error(
            "Regenerate schema and profile with cargo run --example generate_schema",
        ));
    }
    api(
        "PUT",
        "/api/protobuf/profiles/cev-onboarding-v1",
        Some(&profile),
    )?;
    println!("Registered cev-onboarding-v1 ({})", protocol::fingerprint());
    Ok(())
}

/**
 * Selects the server's default connected simulator browser and requires its run to be stopped.
 * @param tabs The simulator's browser inventory and current run statuses.
 * @return The browser ID, or an error for an unavailable default or an active run.
 */
fn default_browser(tabs: &simcontrol::Tabs) -> Result<String> {
    let browser = tabs
        .browsers
        .iter()
        .find(|tab| {
            tab.status.connected
                && tabs.browser_id.as_deref() == Some(tab.status.browser_id.as_str())
        })
        .ok_or_else(|| {
            crate::error(
                "No connected simulator tab. Open the simulator and leave the run stopped.",
            )
        })?;
    if matches!(
        browser.status.snapshot.status.as_str(),
        "running" | "paused" | "ready" | "preparing"
    ) {
        return Err(crate::error(
            "The simulator tab still has a run going. Stop it, then start again.",
        ));
    }
    Ok(browser.status.browser_id.clone())
}

/// Stops a half-prepared browser run when `prepare` returns before `Sim` exists.
struct LaunchCleanup {
    renew_stop: Arc<AtomicBool>,
    renew_thread: Option<JoinHandle<()>>,
    server: String,
    lease_id: String,
    lease_token: String,
    run: Option<simcontrol::Run>,
    armed: bool,
}
impl Drop for LaunchCleanup {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let browser_id = self.run.as_ref().map(|run| run.browser_id().to_string());
        let run_id = self.run.as_ref().map(|run| run.run_id().to_string());
        halt_renewal(
            &self.renew_stop,
            &mut self.renew_thread,
            &self.server,
            &self.lease_id,
            &self.lease_token,
        );
        if let (Some(browser_id), Some(run_id)) = (browser_id, run_id) {
            let _ = stop_after_release(&self.server, &browser_id, &run_id);
        }
    }
}

pub struct Sim {
    pub transport: Transport,
    pub run_id: String,
    pub info: pb::RunInfo,
    server: String,
    lease_id: String,
    lease_token: String,
    renew_stop: Arc<AtomicBool>,
    renew_thread: Option<JoinHandle<()>>,
    run: Option<simcontrol::Run>,
    state_sub: Subscription,
    scan_sub: Subscription,
    join: ObservationJoin,
    state_fresh: protocol::Freshness,
    scan_fresh: protocol::Freshness,
    last_state: Option<pb::Header>,
    last_scan: Option<pb::Header>,
    command_seq: Cell<u64>,
    path_seq: Cell<u64>,
    spec: VehicleSpec,
    done: bool,
    last_monitor: Instant,
}
impl Sim {
    /**
     * Prepares and resumes a diagnostic run with only bounded drive-test motion armed.
     * @param course The course configuration to validate and launch.
     * @param server The simulator server's base URL.
     * @return The running simulator connection, or a preparation or resume error.
     */
    pub fn launch(course: &Course, server: &str) -> Result<Self> {
        let mut sim = Self::prepare(course, server, "drive-test")?;
        sim.resume()?;
        Ok(sim)
    }
    /**
     * Launches the saved onboarding manifest, connects transport, and activates routing with the run paused.
     * Uses the server's default connected browser and requires its run to be stopped.
     * @param course The course configuration and vehicle specification to validate.
     * @param server The simulator server's base URL.
     * @param mode The run mode recorded in RunInfo, such as observe, controls, or drive-test.
     * @return The prepared simulator connection, or a validation, file, API, or transport error.
     */
    pub fn prepare(course: &Course, server: &str, mode: &str) -> Result<Self> {
        course.validate()?;
        let client = simcontrol::Client::with_timeout(server, Duration::from_secs(30))?;
        let tabs = match client.get_tabs() {
            Ok(tabs) => tabs,
            Err(err)
                if matches!(
                    err.kind,
                    simcontrol::ErrorKind::Http
                        | simcontrol::ErrorKind::InvalidResponse
                        | simcontrol::ErrorKind::Transport
                ) =>
            {
                return Err(crate::error(format!(
                    "The process at {server} is not the simulator ({err})"
                )));
            }
            Err(err) => return Err(err.into()),
        };
        let browser = default_browser(&tabs)?;
        let server = server.trim_end_matches('/').to_string();
        let resolved = request(
            "POST",
            &format!("{server}/api/storage/run-manifests/{MANIFEST_ID}/resolve"),
            Some(&serde_json::json!({})),
        )?;
        let expected = resolved["resolvedHash"]
            .as_str()
            .ok_or_else(|| crate::error("Missing derived run hash"))?
            .to_string();
        let (lease_id, lease_token) = acquire_lease(&server, &browser, mode)?;
        let renew_stop = Arc::new(AtomicBool::new(false));
        let mut cleanup = LaunchCleanup {
            renew_thread: Some(spawn_lease_renewal(
                server.clone(),
                lease_id.clone(),
                lease_token.clone(),
                Arc::clone(&renew_stop),
            )),
            renew_stop: Arc::clone(&renew_stop),
            server: server.clone(),
            lease_id: lease_id.clone(),
            lease_token: lease_token.clone(),
            run: None,
            armed: true,
        };
        let mut config = simcontrol::RunConfig::new(MANIFEST_ID);
        config.browser_id = Some(browser);
        config.autoplay = false;
        config.control_lease = Some(simcontrol::ControlLease {
            id: lease_id.clone(),
            token: lease_token.clone(),
        });
        cleanup.run = Some(client.launch(config)?);
        let run_id = cleanup.run.as_ref().unwrap().run_id().to_string();
        let snapshot = cleanup.run.as_ref().unwrap().status()?.snapshot;
        if snapshot.resolved_hash.as_deref() != Some(&expected) {
            return Err(crate::error("Prepared course changed during launch"));
        }
        let transport = Transport::connect(&run_id)?;
        let state_sub = transport.subscribe("state", 32)?;
        let scan_sub = transport.subscribe("lidar", 8)?;
        let info = pb::RunInfo {
            header: Some(protocol::header(&run_id, 1, 0, "map")),
            schema_hash: protocol::fingerprint(),
            goal: Some(protocol::convert(&course.goal)?),
            vehicle: Some(protocol::convert(&course.vehicle)?),
            grid: Some(protocol::convert(&course.grid)?),
            goal_tolerance_m: course.goal_tolerance_m,
            timeout_s: course.timeout_s,
            mode: mode.into(),
            control_lease_id: lease_id.clone(),
            control_path: if mode == "controls" {
                Some(pb::Path {
                    points: protocol::convert(&course.control_path)?,
                })
            } else {
                None
            },
        };
        transport.publish("run_info", &info)?;
        cleanup.armed = false;
        Ok(Self {
            transport,
            run_id,
            info,
            server,
            lease_id,
            lease_token,
            renew_stop,
            renew_thread: cleanup.renew_thread.take(),
            run: cleanup.run.take(),
            state_sub,
            scan_sub,
            join: ObservationJoin::new(),
            state_fresh: Default::default(),
            scan_fresh: Default::default(),
            last_state: None,
            last_scan: None,
            command_seq: Cell::new(0),
            path_seq: Cell::new(0),
            spec: course.vehicle.clone(),
            done: false,
            last_monitor: Instant::now(),
        })
    }
    /**
     * Resumes the prepared browser simulation run.
     * @return Ok(()) if the run resumes, or an error if the run is missing or the request fails.
     */
    pub fn resume(&mut self) -> Result<()> {
        self.run
            .as_mut()
            .ok_or_else(|| crate::error("No browser run"))?
            .resume()?;
        Ok(())
    }
    /**
     * Publishes a path using the latest matched scan as its source, preserving capture time.
     * Does nothing until a scan is available; an empty path is published as NoPath.
     * @param path The requested waypoints in map coordinates, in meters.
     * @return Ok(()) if published or no scan is available, or a conversion or transport error.
     */
    pub fn path(&self, path: &[Point2]) -> Result<()> {
        let Some(source) = &self.last_scan else {
            return Ok(());
        };
        self.path_seq.set(self.path_seq.get() + 1);
        self.transport.publish(
            "path",
            &pb::PathUpdate {
                header: Some(protocol::header(
                    &self.run_id,
                    self.path_seq.get(),
                    source.time_ns,
                    "map",
                )),
                source: Some(source.clone()),
                map_sequence: source.sequence,
                path: Some(pb::Path {
                    points: protocol::convert(&path)?,
                }),
                availability: if path.is_empty() {
                    pb::Availability::NoPath as i32
                } else {
                    pb::Availability::Ready as i32
                },
            },
        )
    }
    /**
     * Republishes run info, then fetches the browser run's status.
     * @return The current simulator snapshot, or an error for a lost/faulted run or failed request.
     */
    pub fn heartbeat(&mut self) -> Result<()> {
        if self.last_monitor.elapsed() < Duration::from_millis(200) {
            return Ok(());
        }
        self.transport.publish("run_info", &self.info)?;
        self.last_monitor = Instant::now();
        Ok(())
    }
    /**
     * Fetches the browser run's status and keeps this run's configuration on the router.
     * @return The current simulator snapshot, or an error for a lost run or failed request.
     */
    pub fn status(&mut self) -> Result<simcontrol::Snapshot> {
        self.heartbeat()?;
        Ok(self
            .run
            .as_ref()
            .ok_or_else(|| crate::error("No browser run"))?
            .status()?
            .snapshot)
    }
    /**
     * Sends STOP, releases the lease, then stops the browser once that release has paused it.
     * Leaves the saved onboarding manifest in place.
     * @return The final simulator snapshot, or an error from any shutdown step.
     */
    pub fn finish(&mut self) -> Result<simcontrol::Snapshot> {
        self.stop()?;
        let browser_id = self
            .run
            .as_ref()
            .ok_or_else(|| crate::error("No browser run"))?
            .browser_id()
            .to_string();
        let run_id = self.run.as_ref().unwrap().run_id().to_string();
        halt_renewal(
            &self.renew_stop,
            &mut self.renew_thread,
            &self.server,
            &self.lease_id,
            &self.lease_token,
        );
        let snapshot = stop_after_release(&self.server, &browser_id, &run_id)?;
        self.done = true;
        Ok(snapshot)
    }
}
impl VehicleIo for Sim {
    /**
     * Reads and joins state and LiDAR samples, rejecting invalid or expired observations.
     * About every 250 ms, republishes this run's RunInfo.
     * @param timeout The maximum time to wait for a matching capture.
     * @return Some(observation) when matched, None on timeout, or a data or transport error.
     */
    fn observe(&mut self, timeout: Duration) -> Result<Option<Observation>> {
        if self.last_monitor.elapsed() > Duration::from_millis(250) {
            self.heartbeat()?;
        }
        let deadline = Instant::now() + timeout;
        loop {
            while let Some(update) =
                transport::receive::<pb::StateUpdate>(&self.state_sub, Duration::ZERO)?
            {
                let (h, state) = protocol::state(&update, &self.run_id)?;
                self.state_fresh.accept(&h)?;
                self.join.state(h, state);
            }
            while let Some(update) =
                transport::receive::<pb::LidarUpdate>(&self.scan_sub, Duration::ZERO)?
            {
                let (h, scan) = protocol::scan(&update, &self.run_id)?;
                self.scan_fresh.accept(&h)?;
                self.join.scan(h, scan);
            }
            if let Some((mut scan_h, state_h, obs)) = self.join.take() {
                if !self.scan_fresh.fresh(self.state_fresh.time_ns())
                    || !self.state_fresh.fresh(self.state_fresh.time_ns())
                {
                    return Err(crate::error("Buffered observation expired"));
                }
                scan_h.frame_id = "map".into();
                self.last_scan = Some(scan_h);
                self.last_state = Some(state_h);
                return Ok(Some(obs));
            }
            if Instant::now() >= deadline {
                return Ok(None);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    /**
     * Limits a command to the vehicle specification and publishes it with a source deadline.
     * Drive-test mode uses the last matched state; other modes use the last matched scan.
     * @param command The requested forward speed in m/s and steering angle in radians.
     * @return Ok(()) if published, or an error for non-finite input, missing motion source, or transport failure.
     */
    fn drive(&mut self, command: DriveCommand) -> Result<()> {
        let command = self.spec.limit(command)?;
        let source = if self.info.mode == "drive-test" {
            self.last_state.clone()
        } else {
            self.last_scan.clone()
        };
        if command != DriveCommand::STOP && source.is_none() {
            return Err(crate::error("No fresh driving source"));
        }
        self.command_seq.set(self.command_seq.get() + 1);
        let now = self.last_state.as_ref().map_or(0, |h| h.time_ns);
        self.transport.publish(
            "command",
            &pb::CommandUpdate {
                header: Some(protocol::header(
                    &self.run_id,
                    self.command_seq.get(),
                    now,
                    "base_link",
                )),
                source: source.clone(),
                path_sequence: self.path_seq.get(),
                state_sequence: self.last_state.as_ref().map_or(0, |h| h.sequence),
                valid_until_ns: source
                    .as_ref()
                    .map_or(0, |h| h.time_ns + protocol::FRESH_NS),
                control_lease_id: self.lease_id.clone(),
                command: Some(protocol::convert(&command)?),
            },
        )
    }
    /**
     * Publishes a zero-speed, zero-steering command through the normal command path.
     * @return Ok(()) if the STOP command is published, or a publication error.
     */
    fn stop(&mut self) -> Result<()> {
        self.drive(DriveCommand::STOP)
    }
}
impl Drop for Sim {
    /**
     * Releases the control lease and stops the browser run.
     * Skips cleanup after finish has marked the run done and ignores cleanup errors.
     * Does not delete the saved onboarding manifest.
     */
    fn drop(&mut self) {
        if !self.done {
            let browser_id = self.run.as_ref().map(|run| run.browser_id().to_string());
            let run_id = self.run.as_ref().map(|run| run.run_id().to_string());
            halt_renewal(
                &self.renew_stop,
                &mut self.renew_thread,
                &self.server,
                &self.lease_id,
                &self.lease_token,
            );
            if let (Some(browser_id), Some(run_id)) = (browser_id, run_id) {
                let _ = stop_after_release(&self.server, &browser_id, &run_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn browser_tabs() -> simcontrol::Tabs {
        let tab = serde_json::json!({"browserId":"current","isDefault":true,"connected":true,
            "lastSeenAt":"now","lastInteractionAt":null,"interactionOrder":2,"commandId":null,
            "status":"stopped","runId":null,"manifestId":null,"manifestRevision":null,"resolvedHash":null,
            "step":0,"timeNs":"0","stepNs":"20000000","transitioning":false,"degraded":false,
            "error":null,"experimentActive":false,"navigationBlocked":false,"runResult":null});
        let mut old = tab.clone();
        old["connected"] = serde_json::json!(false);
        old["isDefault"] = serde_json::json!(false);
        old["status"] = serde_json::json!("running");
        old["browserId"] = serde_json::json!("expired");
        let mut other = old.clone();
        other["connected"] = serde_json::json!(true);
        other["browserId"] = serde_json::json!("background");
        other["interactionOrder"] = serde_json::json!(1);
        serde_json::from_value(
            serde_json::json!({"browserId":"current","browsers":[old,other,tab]}),
        )
        .unwrap()
    }
    #[test]
    fn default_browser_uses_server_selection_with_other_connected_runs() {
        let mut tabs = browser_tabs();
        assert_eq!(default_browser(&tabs).unwrap(), "current");
        tabs.browsers[2].status.snapshot.status = "idle".into();
        assert_eq!(default_browser(&tabs).unwrap(), "current");
    }
    #[test]
    fn default_browser_rejects_unavailable_default_without_falling_back() {
        let mut tabs = browser_tabs();
        tabs.browsers[1].status.snapshot.status = "stopped".into();
        for browser_id in [None, Some("missing"), Some("expired")] {
            tabs.browser_id = browser_id.map(str::to_owned);
            assert_eq!(
                default_browser(&tabs).unwrap_err().to_string(),
                "No connected simulator tab. Open the simulator and leave the run stopped."
            );
        }
        tabs.browser_id = Some("current".into());
        tabs.browsers[2].status.connected = false;
        assert!(default_browser(&tabs).is_err());
        tabs.browsers.clear();
        assert!(default_browser(&tabs).is_err());
    }
    #[test]
    fn default_browser_requires_selected_run_to_be_stopped() {
        let mut tabs = browser_tabs();
        for status in ["running", "paused", "ready", "preparing"] {
            tabs.browsers[2].status.snapshot.status = status.into();
            assert_eq!(
                default_browser(&tabs).unwrap_err().to_string(),
                "The simulator tab still has a run going. Stop it, then start again."
            );
        }
    }
    #[test]
    fn renew_attempt_stays_inside_the_lease() {
        assert!(RENEW_TIMEOUT + RENEW_PERIOD < LEASE_TTL);
        assert_eq!(
            renew_delay(Duration::from_millis(50)),
            Duration::from_millis(150)
        );
        assert_eq!(renew_delay(RENEW_TIMEOUT), Duration::ZERO);
    }
    #[test]
    #[ignore = "Loopback socket test; run by the transport CI check"]
    fn http_authorization_is_not_forwarded_to_simulator_storage() {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut headers = String::new();
                {
                    let mut reader = BufReader::new(&mut stream);
                    loop {
                        let mut line = String::new();
                        reader.read_line(&mut line).unwrap();
                        if line == "\r\n" {
                            break;
                        }
                        headers.push_str(&line);
                    }
                }
                stream
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                    )
                    .unwrap();
                requests.push(headers);
            }
            requests
        });
        let old_token = std::env::var("CEV_API_TOKEN").ok();
        std::env::set_var("CEV_API_TOKEN", "task-test-token");
        let result = request_with_token(
            "GET",
            &format!("{base}/api/protobuf/runs"),
            None,
            Some("task-test-token"),
        );
        let storage = request("GET", &format!("{base}/api/storage/probe"), None);
        if let Some(value) = old_token {
            std::env::set_var("CEV_API_TOKEN", value);
        } else {
            std::env::remove_var("CEV_API_TOKEN");
        }
        result.unwrap();
        storage.unwrap();
        let requests = worker.join().unwrap();
        assert!(requests[0].contains("Bearer task-test-token"));
        assert!(!requests[1].contains("task-test-token"));
    }
}
