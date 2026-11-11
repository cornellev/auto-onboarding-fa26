//! The message loop for perception is done. The map still comes from `Perception::update`.
//! Ignore me! This is already wired up to Zenoh for you... but there's going to be a chance to actually wire up Zenoh yourself later on!
use crate::runtime::node::Context;
use crate::{
    perception::synchronizer::ObservationJoin,
    perception::Perception,
    shared::protocol::{self, pb, Freshness},
    shared::transport,
    Result,
};
use prost::Message;
use std::time::{Duration, Instant};
use zenoh::{handlers::RingChannel, Wait};


/// Subscribe to pose and LiDAR, pair the ones from the same moment, and publish a map.
/// Stops if the input is nonsense, out of order, or older than the watchdog allows.
pub fn run(mut ctx: Context) -> Result<()> {
    let states = ctx
        .transport
        .session
        .declare_subscriber(ctx.transport.key("state"))
        .with(RingChannel::new(32))
        .wait()?;
    
    let scans = ctx
        .transport
        .session
        .declare_subscriber(ctx.transport.key("lidar"))
        .with(RingChannel::new(8))
        .wait()?;


    let maps = ctx
        .transport
        .session
        .declare_publisher(ctx.transport.key("map"))
        .encoding(zenoh::bytes::Encoding::APPLICATION_PROTOBUF)
        .congestion_control(zenoh::qos::CongestionControl::Drop)
        .wait()?;


    let mut join = ObservationJoin::new();
    let (mut state_fresh, mut scan_fresh) = (Freshness::default(), Freshness::default());
    let mut perception = Perception::new(protocol::convert(ctx.info.grid.as_ref().unwrap())?);
    let mut last_observation = Instant::now();
    let mut received = false;

    // alright, we're ready to go! publish a "ready" status and start the loop.
    ctx.status(
        true,
        "subscribed; waiting for capture-aligned observations",
        0,
    )?;

    while ctx.poll()? {
        while let Some(update) = transport::receive::<pb::StateUpdate>(&states, Duration::ZERO)? {
            let (h, state) = protocol::state(&update, &ctx.transport.run)?;
            state_fresh.accept(&h)?;
            join.state(h, state);
        }
        while let Some(update) = transport::receive::<pb::LidarUpdate>(&scans, Duration::ZERO)? {
            let (h, scan) = protocol::scan(&update, &ctx.transport.run)?;
            scan_fresh.accept(&h)?;
            join.scan(h, scan);
        }
        if let Some((mut source, _, observation)) = join.take() {
            if state_fresh
                .time_ns()
                .checked_sub(observation.time_ns)
                .is_none_or(|age| age > protocol::FRESH_NS)
            {
                return Err(crate::error("Matched observation is stale"));
            }
            source.frame_id = "map".into();
            let started = Instant::now();
            let map = perception.update(&observation);
            let grid = map
                .map(|map| {
                    map.validate()?;
                    protocol::convert(map)
                })
                .transpose()?;
            if started.elapsed() > Duration::from_millis(300) {
                return Err(crate::error("Perception exceeded freshness budget"));
            }
            let ready = grid.is_some();
            let update = pb::MapUpdate {
                header: Some(source.clone()),
                source: Some(source),
                state: Some(protocol::convert(&observation.state)?),
                grid,
                availability: if ready {
                    pb::Availability::Ready as i32
                } else {
                    pb::Availability::NotReady as i32
                },
            };
            maps.put(update.encode_to_vec()).wait()?;
            ctx.status(
                ready,
                if ready {
                    "map published"
                } else {
                    "implement Perception::update"
                },
                observation.time_ns,
            )?;
            last_observation = Instant::now();
            received = true;
        }
        if received && last_observation.elapsed() > Duration::from_millis(300) {
            return Err(crate::error("Perception source expired"));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}
