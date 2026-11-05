//! A session and bounded subscribers, with explicit loopback routing.
use crate::{
    protocol::{decode, pb},
    Result,
};
use prost::Message;
use zenoh::{
    handlers::{RingChannel, RingChannelHandler},
    pubsub::Subscriber,
    sample::Sample,
    Wait,
};

pub type Subscription = Subscriber<RingChannelHandler<Sample>>;
pub struct Transport {
    pub session: zenoh::Session,
    pub run: String,
}
impl Transport {
    pub fn connect(run: &str) -> Result<Self> {
        if run.is_empty()
            || run.len() > 96
            || !run.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(crate::error("Invalid run id"));
        }
        let endpoint =
            std::env::var("CEV_ZENOH_ENDPOINT").unwrap_or_else(|_| "tcp/127.0.0.1:7447".into());
        let config = zenoh::Config::from_json5(
            &serde_json::json!({
                "mode":"client", "connect":{"endpoints":[endpoint],"exit_on_failure":true},
                "scouting":{"multicast":{"enabled":false},"gossip":{"enabled":false}}
            })
            .to_string(),
        )?;
        Ok(Self {
            session: zenoh::open(config).wait()?,
            run: run.into(),
        })
    }
    pub fn key(&self, suffix: &str) -> String {
        format!("cev/onboarding/v1/{}/{suffix}", self.run)
    }
    pub fn subscribe(&self, suffix: &str, capacity: usize) -> Result<Subscription> {
        self.session
            .declare_subscriber(self.key(suffix))
            .with(RingChannel::new(capacity))
            .wait()
    }
    pub fn publish<M: Message>(&self, suffix: &str, value: &M) -> Result<()> {
        self.session
            .put(self.key(suffix), value.encode_to_vec())
            .encoding(zenoh::bytes::Encoding::APPLICATION_PROTOBUF)
            .congestion_control(zenoh::qos::CongestionControl::Drop)
            .wait()?;
        Ok(())
    }
}
pub fn receive<M: Message + Default>(
    sub: &Subscription,
    timeout: std::time::Duration,
) -> Result<Option<M>> {
    sub.recv_timeout(timeout)?
        .map(|s| decode(&s.payload().to_bytes()))
        .transpose()
}

/// Join only exact capture times. Different scan/pose arrival orders are supported.
pub struct ObservationJoin {
    states: std::collections::BTreeMap<u64, (pb::Header, crate::types::VehicleState)>,
    scans: std::collections::BTreeMap<u64, (pb::Header, crate::types::LidarScan)>,
}
impl Default for ObservationJoin {
    fn default() -> Self {
        Self::new()
    }
}
impl ObservationJoin {
    pub fn new() -> Self {
        Self {
            states: Default::default(),
            scans: Default::default(),
        }
    }
    pub fn state(&mut self, h: pb::Header, state: crate::types::VehicleState) {
        self.states.insert(h.time_ns, (h, state));
        while self.states.len() > 32 {
            self.states.pop_first();
        }
    }
    pub fn scan(&mut self, h: pb::Header, scan: crate::types::LidarScan) {
        self.scans.insert(h.time_ns, (h, scan));
        while self.scans.len() > 8 {
            self.scans.pop_first();
        }
    }
    pub fn take(&mut self) -> Option<(pb::Header, pb::Header, crate::types::Observation)> {
        let time = self
            .scans
            .keys()
            .rev()
            .find(|t| self.states.contains_key(t))
            .copied()?;
        let (scan_h, scan) = self.scans.remove(&time)?;
        let (state_h, state) = self.states.get(&time)?.clone();
        self.scans.retain(|t, _| *t > time);
        Some((
            scan_h,
            state_h,
            crate::types::Observation {
                time_ns: time,
                state,
                scan,
            },
        ))
    }
}
