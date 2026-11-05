pub mod wire;

use crate::{hardware::VehicleIo, runner::Course, types::*, Result};
use orchestrator_protocol::MessageRegistry;
use orchestrator_ws_client::{Client, ClientEvent};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc, Condvar, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

const FRESH_NS: u64 = 300_000_000;

/// Install the authoring closure that ev-sim's run-bundle importer does not restore.
pub fn setup(server: &str) -> Result<()> {
    simcontrol::Client::new(server)?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .max_redirects(0)
        .timeout_global(Some(Duration::from_secs(15)))
        .build()
        .into();
    let request = |path: &str, body: Option<&serde_json::Value>| -> Result<serde_json::Value> {
        let url = format!("{}/api/storage{path}", server.trim_end_matches('/'));
        let mut response = if let Some(body) = body {
            agent.post(&url).send_json(body)?
        } else {
            agent.get(&url).call()?
        };
        let status = response.status().as_u16();
        if status == 404 && body.is_none() {
            return Ok(serde_json::Value::Null);
        }
        let value: serde_json::Value = response.body_mut().read_json()?;
        if status >= 400 {
            return Err(crate::error(format!("Storage API {path}: {value}")));
        }
        Ok(value)
    };
    for name in ["empty", "detour", "blocked"] {
        let bundle: serde_json::Value =
            serde_json::from_slice(&std::fs::read(format!("scenarios/{name}.bundle.json"))?)?;
        let course: Course =
            serde_json::from_slice(&std::fs::read(format!("scenarios/{name}.json"))?)?;
        course.validate()?;
        for vehicle in bundle["resolved"]["vehicles"]
            .as_array()
            .ok_or_else(|| crate::error("Missing bundled vehicles"))?
        {
            let manifest = &vehicle["manifest"];
            let id = manifest["id"]
                .as_str()
                .ok_or_else(|| crate::error("Missing vehicle ID"))?;
            if request(&format!("/vehicles/{id}"), None)?.is_null() {
                request("/vehicles", Some(manifest))?;
            }
        }
        let environment = &bundle["resolved"]["environment"]["manifest"];
        let id = environment["environmentId"]
            .as_str()
            .ok_or_else(|| crate::error("Missing environment ID"))?;
        if request(&format!("/environments/{id}"), None)?.is_null() {
            request(
                "/environments",
                Some(
                    &serde_json::json!({"id":id,"name":environment["name"],"initialManifest":environment,"supportedEditorSourceVersions":[1]}),
                ),
            )?;
        }
        let scenario = &bundle["resolved"]["scenario"]["scenario"];
        let id = scenario["id"]
            .as_str()
            .ok_or_else(|| crate::error("Missing scenario ID"))?;
        if request(&format!("/scenarios/{id}"), None)?.is_null() {
            request("/scenarios", Some(scenario))?;
        }
        let id = &course.manifest_id;
        if request(&format!("/run-manifests/{id}"), None)?.is_null() {
            request("/run-manifests", Some(&bundle["manifest"]))?;
        }
        let resolved = request(
            &format!("/run-manifests/{id}/resolve"),
            Some(&serde_json::json!({})),
        )?;
        verify_course(&resolved, &bundle, &course)?;
        println!("Installed and verified {id}");
    }
    Ok(())
}

fn verify_course(
    resolved: &serde_json::Value,
    bundle: &serde_json::Value,
    course: &Course,
) -> Result<String> {
    // Imports rewrite timestamps in this simulator revision, so resolvedHash alone is not portable.
    let source = &bundle["resolved"];
    if source["resolvedHash"].as_str() != Some(&course.resolved_hash)
        || resolved["definitionHash"] != source["definitionHash"]
        || resolved["dependencyHashes"] != source["dependencyHashes"]
        || resolved["definitionHash"].is_null()
        || resolved["dependencyHashes"].is_null()
    {
        return Err(crate::error(format!("Course {} or its dependencies differ from the supplied bundle. Existing resources were not overwritten; use a fresh simulator data directory or rename edited resources.",course.manifest_id)));
    }
    resolved["resolvedHash"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| crate::error("Missing resolved run hash"))
}

fn resolve_course(server: &str, course: &Course) -> Result<String> {
    let bundle: serde_json::Value = serde_json::from_slice(&std::fs::read(format!(
        "scenarios/{}.bundle.json",
        course.manifest_id.trim_start_matches("onboarding-")
    ))?)?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .max_redirects(0)
        .timeout_global(Some(Duration::from_secs(15)))
        .build()
        .into();
    let resolved: serde_json::Value = agent
        .post(format!(
            "{}/api/storage/run-manifests/{}/resolve",
            server.trim_end_matches('/'),
            course.manifest_id
        ))
        .send_json(serde_json::json!({}))?
        .body_mut()
        .read_json()?;
    verify_course(&resolved, &bundle, course)
}
type Pending = (Option<Observation>, Option<String>);
type Shared = Arc<(Mutex<Pending>, Condvar)>;

enum Outbound {
    Drive(DriveCommand, u64),
    Path(Path),
    Close,
}
struct Request {
    value: Outbound,
    reply: mpsc::Sender<std::result::Result<(), String>>,
}

/// Exact matching, including scans that arrive before their pose.
pub struct Synchronizer {
    poses: BTreeMap<u64, VehicleState>,
    scans: BTreeMap<u64, Vec<Point2>>,
    spec: VehicleSpec,
}
impl Synchronizer {
    pub fn new(spec: VehicleSpec) -> Self {
        Self {
            poses: BTreeMap::new(),
            scans: BTreeMap::new(),
            spec,
        }
    }
    pub fn pose(&mut self, time: u64, state: VehicleState) -> Option<Observation> {
        self.poses.insert(time, state);
        self.take(time)
    }
    pub fn scan(&mut self, time: u64, points: Vec<Point2>) -> Option<Observation> {
        self.scans.insert(time, points);
        self.take(time)
    }
    fn take(&mut self, time: u64) -> Option<Observation> {
        let result = if let (Some(state), Some(points)) =
            (self.poses.get(&time).copied(), self.scans.get(&time))
        {
            let observation = Observation {
                time_ns: time,
                state,
                scan: LidarScan {
                    points: points.clone(),
                    sensor_in_vehicle: self.spec.sensor_in_vehicle,
                },
            };
            self.scans.remove(&time);
            Some(observation)
        } else {
            None
        };
        while self.poses.len() > 32 {
            self.poses.pop_first();
        }
        while self.scans.len() > 8 {
            self.scans.pop_first();
        }
        result
    }
}

fn failure(shared: &Shared, message: String) {
    let (lock, wake) = &**shared;
    if let Ok(mut state) = lock.lock() {
        state.1 = Some(message);
        wake.notify_all();
    }
}

fn decode<T: serde::de::DeserializeOwned>(value: orchestrator_protocol::Value) -> Result<T> {
    Ok(serde_json::from_value(serde_json::to_value(value)?)?)
}

pub struct Sim {
    run: Option<simcontrol::Run>,
    tx: tokio::sync::mpsc::Sender<Request>,
    shared: Shared,
    clock: Arc<AtomicU64>,
    worker: Option<thread::JoinHandle<()>>,
    last_read: Option<u64>,
    last_received: Option<Instant>,
    spec: VehicleSpec,
}

impl Sim {
    pub fn launch(course: &Course, server: &str) -> Result<Self> {
        course.validate()?;
        let client = simcontrol::Client::with_timeout(server, Duration::from_secs(30))?;
        let tabs = client.get_tabs()?;
        if tabs.browsers.iter().any(|t| {
            matches!(
                t.status.snapshot.status.as_str(),
                "running" | "paused" | "ready" | "preparing"
            )
        }) {
            return Err(crate::error(
                "Stop other browser runs first. Use one simulator tab and one onboarding process.",
            ));
        }
        if tabs.browser_id.is_none() {
            return Err(crate::error(
                "Open the simulator page first; see docs/getting-started.md",
            ));
        }
        let expected_hash = resolve_course(server, course)?;
        let shared: Shared = Arc::new((Mutex::new((None, None)), Condvar::new()));
        let clock = Arc::new(AtomicU64::new(0));
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Request>(4);
        let (ready_tx, ready_rx) = mpsc::channel::<std::result::Result<(), String>>();
        let (shared_worker, clock_worker, spec) =
            (shared.clone(), clock.clone(), course.vehicle.clone());
        let worker = thread::spawn(move || {
            let ready_failed = ready_tx.clone();
            let result: Result<()> = (|| {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                runtime.block_on(async {
                    let registry=Arc::new(MessageRegistry::new());
                    orchestrator_ws_client::sync_types_from_server(&registry,"http://localhost:8090",None,None).await?;
                    let (client,mut events)=Client::builder().uri("ws://localhost:8080").registry(registry).reconnect(false).connect().await?;
                    client.publish(wire::COMMAND,"sensor_fusion_msgs/StampedAckermannDrive",wire::command_value(DriveCommand::STOP,0,0)?).await?;
                    client.echo().await?;
                    let mut sync=Synchronizer::new(spec);
                    let mut sequence=1_u32;
                    let mut last_scan=Instant::now();
                    let mut last_clock=None;
                    let mut previous_tf:Option<(u64,VehicleState)>=None;
                    let mut ready=Some(ready_tx);
                    loop {
                        tokio::select! {
                            request=rx.recv()=>{
                                let Some(request)=request else {break;};
                                let close=matches!(request.value,Outbound::Close);
                                let sent:Result<()> = match request.value {
                                    Outbound::Drive(command,stamp)=>{
                                        let now=clock_worker.load(Ordering::SeqCst);
                                        if command != DriveCommand::STOP && (now.saturating_sub(stamp)>FRESH_NS || last_scan.elapsed()>Duration::from_millis(300)) {
                                            Err(crate::error("Observation expired while computing; stopping"))
                                        } else {
                                            let value=wire::command_value(command,now,sequence)?;
                                            sequence=sequence.checked_add(1).ok_or_else(||crate::error("Command sequence exhausted; restart"))?;
                                            client.publish(wire::COMMAND,"sensor_fusion_msgs/StampedAckermannDrive",value).await.map_err(Into::into)
                                        }
                                    },
                                    Outbound::Path(path)=>client.publish(wire::WAYPOINTS,"sensor_fusion_msgs/WaypointList",wire::waypoint_value(&path)?).await.map_err(Into::into),
                                    Outbound::Close=>client.stop().await.map_err(Into::into),
                                };
                                let _=request.reply.send(sent.map_err(|e|e.to_string()));
                                if close {break;}
                            },
                            event=events.recv()=>{
                                let observation=match event {
                                    Some(ClientEvent::Echo(_))=>{if let Some(ready)=ready.take(){let _=ready.send(Ok(()));} None},
                                    Some(ClientEvent::Update(update))=>{
                                        if let Some(value)=update.value {
                                            match update.name.as_str() {
                                                wire::CLOCK=>{
                                                    #[derive(serde::Deserialize)] struct Clock {clock:wire::Stamp}
                                                    let stamp=decode::<Clock>(value)?.clock.ns()?;
                                                    if last_clock.is_some_and(|last|stamp<last){return Err(crate::error("Simulation clock reset; restart the onboarding process"));}
                                                    last_clock=Some(stamp);clock_worker.store(stamp,Ordering::SeqCst);None
                                                },
                                                wire::POSE=>{let (time,state)=wire::decode_pose(decode(value)?,&sync.spec)?;sync.pose(time,state)},
                                                wire::TF=>{
                                                    if let Some((time,mut state))=wire::decode_tf(decode(value)?,&sync.spec)? {
                                                        if previous_tf.is_some_and(|(last,_)|time<last){return Err(crate::error("Simulation clock reset; restart the controller"));}
                                                        if let Some((last,previous))=previous_tf {
                                                            if time>last {
                                                                let p=state.pose.position;let old=previous.pose.position;
                                                                state.speed_mps=((p.x-old.x)*state.pose.yaw_rad.cos()+(p.y-old.y)*state.pose.yaw_rad.sin())/((time-last) as f64/1e9);
                                                            }
                                                        }
                                                        previous_tf=Some((time,state));clock_worker.store(time,Ordering::SeqCst);
                                                        sync.pose(time,state)
                                                    }else{None}
                                                },
                                                wire::CLOUD=>{let (time,points)=wire::decode_cloud(&decode(value)?)?;sync.scan(time,points)},
                                                _=>None,
                                            }
                                        }else{None}
                                    },
                                    Some(ClientEvent::Disconnected)|None=>return Err(crate::error("Orchestrator disconnected; no automatic resume")),
                                    Some(ClientEvent::ProtocolError(e))=>return Err(crate::error(format!("Orchestrator protocol error: {}",e.message))),
                                    Some(ClientEvent::LocalError(e))=>return Err(crate::error(e)),
                                    _=>None,
                                };
                                if let Some(observation)=observation {
                                    observation.validate()?;last_scan=Instant::now();
                                    let (lock,wake)=&*shared_worker;
                                    lock.lock().map_err(|_|crate::error("Observation lock failed"))?.0=Some(observation);
                                    wake.notify_all();
                                }
                            }
                        }
                    }
                    Ok(())
                })
            })();
            if let Err(error) = result {
                failure(&shared_worker, error.to_string());
                let _ = ready_failed.send(Err(error.to_string()));
            }
        });
        let mut sim = Self {
            run: None,
            tx,
            shared,
            clock,
            worker: Some(worker),
            last_read: None,
            last_received: None,
            spec: course.vehicle.clone(),
        };
        match ready_rx.recv_timeout(Duration::from_secs(15)) {
            Ok(Ok(())) => {},
            Ok(Err(message)) => return Err(crate::error(message)),
            Err(_) => return Err(crate::error(sim.fault().unwrap_or_else(||"Orchestrator did not become ready; start it and open the simulator page to sync schemas".into()))),
        }
        let mut config = simcontrol::RunConfig::new(&course.manifest_id);
        config.browser_id = tabs.browser_id;
        config.autoplay = false;
        sim.run = Some(match client.launch(config.clone()) {
            Ok(run) => run,
            // Environment changes recreate the browser client before it connects in this revision.
            Err(e)
                if e.kind == simcontrol::ErrorKind::CommandFailed
                    && e.message
                        .contains("orchestrator transport is not connected") =>
            {
                thread::sleep(Duration::from_secs(1));
                client.launch(config)?
            }
            Err(e) => return Err(e.into()),
        });
        let snapshot = sim.run.as_ref().unwrap().status()?.snapshot;
        if snapshot.resolved_hash.as_deref() != Some(&expected_hash) {
            return Err(crate::error("Saved course changed between validation and launch. Run setup against matching, unmodified resources."));
        }
        sim.request(Outbound::Path(vec![]))?;
        sim.stop()?;
        sim.run.as_mut().unwrap().resume()?;
        Ok(sim)
    }

    fn fault(&self) -> Option<String> {
        self.shared.0.lock().ok().and_then(|s| s.1.clone())
    }

    fn request(&self, value: Outbound) -> Result<()> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .try_send(Request { value, reply: tx })
            .map_err(|e| crate::error(format!("Transport queue unavailable: {e}")))?;
        rx.recv_timeout(Duration::from_secs(2))
            .map_err(|_| {
                crate::error("Transport acknowledgement timed out; watchdog will stop the car")
            })?
            .map_err(crate::error)
    }

    pub fn path(&self, path: &[Point2]) -> Result<()> {
        self.request(Outbound::Path(path.to_vec()))
    }

    pub fn status(&self) -> Result<simcontrol::Snapshot> {
        Ok(self
            .run
            .as_ref()
            .ok_or_else(|| crate::error("No active browser run"))?
            .status()?
            .snapshot)
    }

    pub fn finish(&mut self) -> Result<simcontrol::Snapshot> {
        self.stop()?;
        Ok(self
            .run
            .as_mut()
            .ok_or_else(|| crate::error("No active browser run"))?
            .stop()?)
    }
}

impl VehicleIo for Sim {
    fn observe(&mut self, timeout: Duration) -> Result<Option<Observation>> {
        let (lock, wake) = &*self.shared;
        let state = lock
            .lock()
            .map_err(|_| crate::error("Observation lock failed"))?;
        let (mut state, _) = wake
            .wait_timeout_while(state, timeout, |s| s.0.is_none() && s.1.is_none())
            .map_err(|_| crate::error("Observation wait failed"))?;
        if let Some(fault) = &state.1 {
            return Err(crate::error(fault.clone()));
        }
        let observation = state.0.take();
        if let Some(obs) = &observation {
            if self
                .clock
                .load(Ordering::SeqCst)
                .saturating_sub(obs.time_ns)
                > FRESH_NS
            {
                return Err(crate::error("Buffered observation is stale"));
            }
            self.last_read = Some(obs.time_ns);
            self.last_received = Some(Instant::now());
        }
        Ok(observation)
    }
    fn drive(&mut self, command: DriveCommand) -> Result<()> {
        let command = self.spec.limit(command)?;
        if command != DriveCommand::STOP
            && self
                .last_received
                .is_none_or(|t| t.elapsed() > Duration::from_millis(300))
        {
            return Err(crate::error("No fresh observation; refusing motion"));
        }
        self.request(Outbound::Drive(command, self.last_read.unwrap_or(0)))
    }
    fn stop(&mut self) -> Result<()> {
        self.request(Outbound::Drive(
            DriveCommand::STOP,
            self.clock.load(Ordering::SeqCst),
        ))
    }
}

impl Drop for Sim {
    fn drop(&mut self) {
        // Never stop a replacement run on the shared command topic.
        if self.run.as_ref().is_some_and(|run| run.status().is_ok()) {
            let _ = self.stop();
            if let Some(run) = &mut self.run {
                let _ = run.stop();
            }
        }
        let _ = self.request(Outbound::Close);
        if self
            .worker
            .as_ref()
            .is_some_and(|worker| worker.is_finished())
        {
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn import_timestamps_may_change_but_semantic_dependencies_must_match() {
        let bundle: serde_json::Value =
            serde_json::from_str(include_str!("../scenarios/empty.bundle.json")).unwrap();
        let course: Course = serde_json::from_str(include_str!("../scenarios/empty.json")).unwrap();
        let mut resolved = bundle["resolved"].clone();
        resolved["resolvedHash"] = serde_json::json!("imported-run-hash");
        resolved["environment"]["manifest"]["updatedAt"] =
            serde_json::json!("changed import timestamp");
        assert_eq!(
            verify_course(&resolved, &bundle, &course).unwrap(),
            "imported-run-hash"
        );
        resolved["dependencyHashes"]["world"] = serde_json::json!("different geometry");
        assert!(verify_course(&resolved, &bundle, &course).is_err());
    }
    #[test]
    fn synchronization_matches_exactly_and_bounds_history() {
        let course: Course = serde_json::from_str(include_str!("../scenarios/empty.json")).unwrap();
        let mut s = Synchronizer::new(course.vehicle);
        assert!(s.scan(100, vec![]).is_none());
        assert!(s.pose(99, VehicleState::default()).is_none());
        assert_eq!(s.pose(100, VehicleState::default()).unwrap().time_ns, 100);
        for time in 101..200 {
            s.pose(time, VehicleState::default());
            s.scan(time + 1000, vec![]);
        }
        assert!(s.poses.len() <= 32);
        assert!(s.scans.len() <= 8);
    }
}
