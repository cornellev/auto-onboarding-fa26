//! A session and bounded subscribers, with explicit loopback routing.
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
    /**
     * Opens a Zenoh client session for a validated run ID.
     * Uses CEV_ZENOH_ENDPOINT, or tcp/127.0.0.1:7447 when it is unset.
     * @param run A run ID of 1 to 96 ASCII letters, digits, or hyphens.
     * @return The connected transport, or an error if validation or connection fails.
     */
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
    /**
     * Builds a topic key in this run's namespace.
     * @param suffix The topic suffix, such as state, lidar, or command.
     * @return The full cev/onboarding/v1/{run}/{suffix} key.
     */
    pub fn key(&self, suffix: &str) -> String {
        format!("cev/onboarding/v1/{}/{suffix}", self.run)
    }
    /**
     * Subscribes to a run topic using a bounded ring buffer.
     * @param suffix The topic suffix within this run.
     * @param capacity The maximum number of queued samples; use a positive value.
     * @return The subscription, or an error if Zenoh cannot create it.
     */
    pub fn subscribe(&self, suffix: &str, capacity: usize) -> Result<Subscription> {
        self.session
            .declare_subscriber(self.key(suffix))
            .with(RingChannel::new(capacity))
            .wait()
    }
    /**
     * Publishes a Protobuf message, allowing Zenoh to drop it under congestion.
     * @param suffix The topic suffix within this run.
     * @param value The message to encode and publish.
     * @return Ok(()) if publication succeeds, or a transport error.
     */
    pub fn publish<M: Message>(&self, suffix: &str, value: &M) -> Result<()> {
        self.session
            .put(self.key(suffix), value.encode_to_vec())
            .encoding(zenoh::bytes::Encoding::APPLICATION_PROTOBUF)
            .congestion_control(zenoh::qos::CongestionControl::Drop)
            .wait()?;
        Ok(())
    }
}
/**
 * Waits for one sample and decodes it as a Protobuf message.
 * @param sub The subscription to read from.
 * @param timeout The maximum wait; use Duration::ZERO to poll without waiting.
 * @return Some(message) when received, None on timeout, or a receive or decode error.
 */
pub fn receive<M: Message + Default>(
    sub: &Subscription,
    timeout: std::time::Duration,
) -> Result<Option<M>> {
    sub.recv_timeout(timeout)?
        .map(|s| decode(&s.payload().to_bytes()))
        .transpose()
}
