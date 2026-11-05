//! Student task: subscribe to state/path and publish validated commands.
use crate::runtime::node::Context;
use crate::{
    shared::protocol::{self, pb},
    Result,
};
pub fn run(mut ctx: Context) -> Result<()> {
    // Receive pb::StateUpdate and pb::PathUpdate using bounded subscriptions.
    // Retain the latest path, including unavailable results; tick at 10 Hz.
    // Call controlled_update and publish to "command". Empty paths stop.
    ctx.wait_for_wiring(
        "implement controls Zenoh subscriptions and publication; docs/challenges.md#wiring",
    )
}

/// Provided command helper. Republishing an old path never refreshes its deadline.
pub fn controlled_update(
    ctx: &Context,
    state: &pb::StateUpdate,
    path: Option<&pb::PathUpdate>,
    controller: &mut crate::controls::Controller,
    sequence: u64,
    dt_s: f64,
) -> Result<pb::CommandUpdate> {
    let (h, state) = protocol::state(state, &ctx.transport.run)?;
    let mut out = pb::CommandUpdate {
        header: Some(protocol::header(
            &ctx.transport.run,
            sequence,
            h.time_ns,
            "map",
        )),
        state_sequence: h.sequence,
        command: Some(protocol::convert(
            &crate::shared::types::DriveCommand::STOP,
        )?),
        ..Default::default()
    };
    let Some(path) = path else {
        return Ok(out);
    };
    if path.availability != pb::Availability::Ready as i32 {
        return Ok(out);
    }
    let path_h = protocol::check_header(&path.header, &ctx.transport.run, "map")?;
    let source = protocol::check_header(&path.source, &ctx.transport.run, "map")?;
    if !dt_s.is_finite()
        || dt_s <= 0.
        || h.time_ns
            .checked_sub(source.time_ns)
            .is_none_or(|age| age >= protocol::FRESH_NS)
    {
        return Ok(out);
    }
    let points: Vec<crate::shared::types::Point2> = protocol::convert(
        &path
            .path
            .as_ref()
            .ok_or_else(|| crate::error("Missing ready path"))?
            .points,
    )?;
    if points.is_empty() || points.len() > 100_000 || points.iter().any(|p| !p.is_finite()) {
        return Err(crate::error("Invalid path points"));
    }
    let command = controller.command(&state, &points, dt_s);
    // Keep finite requests for saturation reporting; the actuator boundary clamps them.
    controller.vehicle.limit(command)?;
    out.command = Some(protocol::convert(&command)?);
    out.source = Some(source.clone());
    out.path_sequence = path_h.sequence;
    out.valid_until_ns = source.time_ns + protocol::FRESH_NS;
    Ok(out)
}
