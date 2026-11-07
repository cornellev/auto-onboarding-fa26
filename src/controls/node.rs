//! Subscribe to state and path, and publish commands. `run` is where you should implement your code, but `controlled_update` is already written.

// Connect to the context
use crate::runtime::node::Context;
use crate::{
    shared::protocol::{self, pb},
    Result,
};

/// Still waiting for the state and path subscriptions, and the command publisher.
/// Copy the shape of `perception::node::run`.
pub fn run(mut ctx: Context) -> Result<()> {
    // Read StateUpdate and PathUpdate. Keep the latest path, even when it says there's nothing.
    // Call controlled_update about 10 times a second and publish on "command".
    ctx.wait_for_wiring(
        "controls still needs its Zenoh subscribers and publisher; see docs/project.md, Connecting the pieces",
    )
}

/// Build a command from the latest state and path.
/// Missing, unready, or expired paths, and a bad `dt_s`, become a stop.
/// Publishing again does not refresh an old path's deadline. The simulator clamps the command later.
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
    // Reject a non-finite command. The number we publish is the one the controller asked for.
    controller.vehicle.limit(command)?;
    out.command = Some(protocol::convert(&command)?);
    out.source = Some(source.clone());
    out.path_sequence = path_h.sequence;
    out.valid_until_ns = source.time_ns + protocol::FRESH_NS;
    Ok(out)
}
