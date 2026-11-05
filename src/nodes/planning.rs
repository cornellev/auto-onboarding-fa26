//! Student task: connect map subscription to Planner and path publication.
use super::Context;
use crate::Result;
pub fn run(mut ctx: Context) -> Result<()> {
    // See perception.rs for the worked subscription/publication loop.
    // Receive pb::MapUpdate; call super::planned_update; publish to "path".
    // Keep source metadata unchanged. Report readiness after wiring is complete.
    ctx.wait_for_wiring(
        "implement planning Zenoh subscription and publication; docs/challenges.md#wiring",
    )
}
