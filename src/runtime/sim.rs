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
    time::{Duration, Instant},
};

const MANIFEST_ID: &str = "onboarding";

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

fn topic<'a>(manifest: &'a serde_json::Value, id: &str) -> Result<&'a str> {
    let topics = if manifest["topics"].is_array() {
        &manifest["topics"]
    } else {
        &manifest["manifest"]["topics"]
    };
    topics
        .as_array()
        .and_then(|topics| topics.iter().find(|topic| topic["id"].as_str() == Some(id)))
        .and_then(|topic| topic["name"].as_str())
        .ok_or_else(|| crate::error(format!("Run manifest onboarding is missing topic {id}")))
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
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .max_redirects(0)
        .timeout_global(Some(Duration::from_secs(10)))
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
 * Selects the sole connected simulator browser and requires its run to be stopped.
 * Disconnected browser history is ignored.
 * @param tabs The simulator's browser inventory and current run statuses.
 * @return The browser ID, or an error for multiple/no connected browsers or an active run.
 */
fn single_browser(tabs: &simcontrol::Tabs) -> Result<String> {
    let connected: Vec<_> = tabs
        .browsers
        .iter()
        .filter(|t| t.status.connected)
        .collect();
    if connected.is_empty() {
        return Err(crate::error(
            "No connected simulator tab. Open the simulator and leave the run stopped.",
        ));
    }
    if connected.len() != 1 {
        return Err(crate::error(
            "More than one connected simulator tab. Close the extras and leave one run stopped.",
        ));
    }
    if connected.iter().any(|tab| {
        matches!(
            tab.status.snapshot.status.as_str(),
            "running" | "paused" | "ready" | "preparing"
        )
    }) {
        return Err(crate::error(
            "The simulator tab still has a run going. Stop it, then start again.",
        ));
    }
    Ok(connected[0].status.browser_id.clone())
}

pub struct Sim {
    pub transport: Transport,
    pub run_id: String,
    pub info: pb::RunInfo,
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
     * Requires one connected, stopped simulator browser. Does not delete the saved manifest.
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
        let browser = single_browser(&tabs)?;
        let manifest = onboarding_manifest(server)?;
        let run_id = uuid::Uuid::new_v4().to_string();
        let server = server.trim_end_matches('/');
        let tf_topic = topic(&manifest, "tf")?.to_string();
        let lidar_topic = topic(&manifest, "front-lidar-points")?.to_string();
        let command_topic = topic(&manifest, "controls-command")?.to_string();
        let resolved = request(
            "POST",
            &format!("{server}/api/storage/run-manifests/{MANIFEST_ID}/resolve"),
            Some(&serde_json::json!({})),
        )?;
        let expected = resolved["resolvedHash"]
            .as_str()
            .ok_or_else(|| crate::error("Missing derived run hash"))?
            .to_string();
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
            control_path: if mode == "controls" {
                Some(pb::Path {
                    points: protocol::convert(&course.control_path)?,
                })
            } else {
                None
            },
        };
        let context = serde_json::json!({"run_id":run_id,"profile_id":"cev-onboarding-v1","fingerprint":protocol::fingerprint(),
            "mode":mode,"vehicle":serde_json::to_value(&course.vehicle)?,"run_info":info,
            "tf_topic":tf_topic,"lidar_topic":lidar_topic,"command_topic":command_topic});
        api("POST", "/api/protobuf/runs", Some(&context))?;
        let mut sim = Self {
            transport,
            run_id,
            info,
            run: None,
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
        };
        let mut config = simcontrol::RunConfig::new(MANIFEST_ID);
        config.browser_id = Some(browser.clone());
        config.autoplay = false;
        sim.run = Some(match client.launch(config.clone()) {
            Ok(run) => run,
            Err(e)
                if e.kind == simcontrol::ErrorKind::CommandFailed
                    && e.message
                        .contains("orchestrator transport is not connected") =>
            {
                std::thread::sleep(Duration::from_secs(1));
                client.launch(config)?
            }
            Err(e) => return Err(e.into()),
        });
        let snapshot = sim.run.as_ref().unwrap().status()?.snapshot;
        if snapshot.resolved_hash.as_deref() != Some(&expected) {
            return Err(crate::error("Prepared course changed during launch"));
        }
        api(
            "POST",
            &format!("/api/protobuf/runs/{}/activate", sim.run_id),
            Some(
                &serde_json::json!({"browser_id":browser,"sim_run_id":sim.run.as_ref().unwrap().run_id(),"resolved_hash":expected}),
            ),
        )?;
        Ok(sim)
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
     * Checks routing ownership and faults before fetching the browser run's status.
     * @return The current simulator snapshot, or an error for a lost/faulted run or failed request.
     */
    pub fn status(&self) -> Result<simcontrol::Snapshot> {
        let routing = api("GET", "/api/protobuf/runs", None)?;
        if routing["run"]["run_id"] != self.run_id || !routing["run"]["fault"].is_null() {
            return Err(crate::error(format!(
                "Routing run lost or failed: {routing}"
            )));
        }
        Ok(self
            .run
            .as_ref()
            .ok_or_else(|| crate::error("No browser run"))?
            .status()?
            .snapshot)
    }
    /**
     * Sends STOP, removes routing, and stops the browser run.
     * Leaves the saved onboarding manifest in place.
     * @return The final simulator snapshot, or an error from any shutdown step.
     */
    pub fn finish(&mut self) -> Result<simcontrol::Snapshot> {
        self.stop()?;
        api(
            "DELETE",
            &format!("/api/protobuf/runs/{}", self.run_id),
            None,
        )?;
        let snapshot = self
            .run
            .as_mut()
            .ok_or_else(|| crate::error("No browser run"))?
            .stop()?;
        self.done = true;
        Ok(snapshot)
    }
}
impl VehicleIo for Sim {
    /**
     * Reads and joins state and LiDAR samples, rejecting invalid or expired observations.
     * Periodically checks that the active routing run still belongs to this connection.
     * @param timeout The maximum time to wait for a matching capture after the routing check.
     * @return Some(observation) when matched, None on timeout, or a routing, data, or transport error.
     */
    fn observe(&mut self, timeout: Duration) -> Result<Option<Observation>> {
        if self.last_monitor.elapsed() > Duration::from_millis(250) {
            let state = api("GET", "/api/protobuf/runs", None)?;
            if state["run"]["run_id"] != self.run_id {
                return Err(crate::error("Routing run replaced"));
            }
            self.last_monitor = Instant::now();
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
                    "map",
                )),
                source: source.clone(),
                path_sequence: self.path_seq.get(),
                state_sequence: self.last_state.as_ref().map_or(0, |h| h.sequence),
                valid_until_ns: source
                    .as_ref()
                    .map_or(0, |h| h.time_ns + protocol::FRESH_NS),
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
     * Attempts to remove routing and stop the browser run.
     * Skips cleanup after finish has marked the run done and ignores cleanup errors.
     * Does not delete the saved onboarding manifest.
     */
    fn drop(&mut self) {
        if !self.done {
            let _ = api(
                "DELETE",
                &format!("/api/protobuf/runs/{}", self.run_id),
                None,
            );
            if let Some(run) = &mut self.run {
                if run.status().is_ok() {
                    let _ = run.stop();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disconnected_browser_history_does_not_count_as_a_publisher() {
        let tab = serde_json::json!({"browserId":"current","isDefault":true,"connected":true,
            "lastSeenAt":"now","lastInteractionAt":null,"interactionOrder":1,"commandId":null,
            "status":"idle","runId":null,"manifestId":null,"manifestRevision":null,"resolvedHash":null,
            "step":0,"timeNs":"0","stepNs":"20000000","transitioning":false,"degraded":false,
            "error":null,"experimentActive":false,"navigationBlocked":false,"runResult":null});
        let mut old = tab.clone();
        old["connected"] = serde_json::json!(false);
        old["status"] = serde_json::json!("running");
        old["browserId"] = serde_json::json!("expired");
        let mut tabs: simcontrol::Tabs = serde_json::from_value(
            serde_json::json!({"browserId":"current","browsers":[old,tab.clone()]}),
        )
        .unwrap();
        assert_eq!(single_browser(&tabs).unwrap(), "current");
        tabs.browsers.push(serde_json::from_value(tab).unwrap());
        assert!(single_browser(&tabs).is_err());
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
