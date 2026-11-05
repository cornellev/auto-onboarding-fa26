//! Student task: subscribe to state/path and publish validated commands.
use super::Context;
use crate::Result;
pub fn run(mut ctx: Context) -> Result<()> {
    // Receive pb::StateUpdate and pb::PathUpdate using bounded subscriptions.
    // Retain the latest path, including unavailable results; tick at 10 Hz.
    // Call super::controlled_update and publish to "command". Empty paths stop.
    ctx.wait_for_wiring(
        "implement controls Zenoh subscriptions and publication; docs/challenges.md#wiring",
    )
}
