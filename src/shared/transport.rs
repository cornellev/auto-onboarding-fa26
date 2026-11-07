//! A Zenoh session on this machine, and a helper for reading one message.
use crate::{shared::protocol::decode, Result};
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
    /// Connect to the local router. Uses `CEV_ZENOH_ENDPOINT`, or `tcp/127.0.0.1:7447` if that's unset.
    /// `run` is 1 to 96 letters, digits, or hyphens.
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
    /// The full topic name for this run: `cev/onboarding/v1/<run>/<suffix>`.
    pub fn key(&self, suffix: &str) -> String {
        format!("cev/onboarding/v1/{}/{suffix}", self.run)
    }
    /// Subscribe to one topic for this run. `capacity` is how many messages can queue before old ones drop.
    pub fn subscribe(&self, suffix: &str, capacity: usize) -> Result<Subscription> {
        self.session
            .declare_subscriber(self.key(suffix))
            .with(RingChannel::new(capacity))
            .wait()
    }
    /// Publish one Protobuf message. If the network is backed up, Zenoh may drop it.
    pub fn publish<M: Message>(&self, suffix: &str, value: &M) -> Result<()> {
        self.session
            .put(self.key(suffix), value.encode_to_vec())
            .encoding(zenoh::bytes::Encoding::APPLICATION_PROTOBUF)
            .congestion_control(zenoh::qos::CongestionControl::Drop)
            .wait()?;
        Ok(())
    }
}
/// Read one message, or `None` if nothing arrives within `timeout`.
/// `Duration::ZERO` means "look once, don't wait."
pub fn receive<M: Message + Default>(
    sub: &Subscription,
    timeout: std::time::Duration,
) -> Result<Option<M>> {
    sub.recv_timeout(timeout)?
        .map(|s| decode(&s.payload().to_bytes()))
        .transpose()
}
