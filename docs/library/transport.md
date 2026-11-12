# `transport` — A Zenoh session on this machine

**Source code:** [`src/shared/transport.rs`](../../src/shared/transport.rs)

This module opens one Zenoh session to the local router and names the topics for a single run. A publisher puts bytes on a key. A subscriber receives whatever is published there. The session is already connected by the time a node has a [`Context`](context.md). The key for this run comes from `Transport::key`. The bytes are a Protobuf message.

Each run has its own prefix, so a leftover message from the last run is not a message for this one. The router still has to be running. A connection failure tells you to start it.

## The session

*type* `Subscription`

`zenoh::pubsub::Subscriber<RingChannelHandler<Sample>>`. This is the queue `subscribe` returns. `receive` waits on it. A ring channel drops the oldest sample when the publisher gets ahead of the reader.

*class* `shared::transport::Transport`

`session`

The open `zenoh::Session`. Public, so a node can declare a publisher directly. `perception::node::run` does that for `map`, with Protobuf encoding and drop-on-congestion. `publish` is the shorter path for one message.

`run`

The run id, owned as a `String`. The same string is the middle component of every key.

`Transport::connect(run)`

Open a client session for `run`.

The id has to be 1 to 96 bytes, each an ASCII letter, digit, or hyphen. An empty id, a longer id, or any other byte returns `"Invalid run id"`.

The endpoint is the environment variable `CEV_ZENOH_ENDPOINT` when it is set, and `tcp/127.0.0.1:7447` otherwise. The session is a client, connects to that endpoint, exits if the connection fails, and leaves multicast and gossip disabled. Return `"Start the router: zenohd -c config/zenoh-router.json5 (...)"` when the router does not answer. The parenthetical text is the Zenoh error.

`Transport::key(&self, suffix)`

Return `cev/onboarding/v1/<run>/<suffix>`. `suffix` is a short name such as `"state"`, `"lidar"`, `"map"`, `"path"`, `"command"`, `"run_info"`, or `"status/perception"`. The full list of routed keys is in `config/protobuf-profile.json`.

`Transport::subscribe(&self, suffix, capacity)`

Subscribe to `key(suffix)` with a ring channel of `capacity` samples. When the queue is full, the oldest sample is dropped. Perception and the simulator connection use 32 for state and 8 for scans. `Context` uses 1 for `run_info`, because only the latest configuration matters.

Return the Zenoh error if the declaration fails.

`Transport::publish(&self, suffix, value)`

Encode `value` with `encode_to_vec` and put the bytes on `key(suffix)`. The encoding is `application/protobuf`. Congestion control is `Drop`, so a backed-up network may discard the message instead of blocking the publisher. `value` is any type that implements `prost::Message`, which the `pb` messages do.

Return the Zenoh error if the put fails. A dropped sample is not an error. The function returns after Zenoh accepts the put.

## Reading one message

`receive(subscription, timeout)`

Wait up to `timeout` for one sample and decode it. The destination type must implement `prost::Message` and `Default`.

Return `Ok(None)` when the wait expires with an empty queue. `Duration::ZERO` means look once and do not wait, which is how a loop drains a subscription without sleeping inside `receive`. Return `Ok(Some(message))` when a sample arrives and `protocol::decode` accepts the bytes. A payload larger than 8 MB, or a payload that is not the requested message, returns the decode error. An error from the subscription returns that error.

`receive` does not check the header, the run id, or freshness. `protocol::check_header`, `protocol::state`, `protocol::scan`, and `Freshness` do that after a successful read.

## See also

- [`protocol`](protocol.md) — `decode`, headers, and freshness
- [`messages`](messages.md) — the types `publish` and `receive` carry
- [`context`](context.md) — a session plus the run's `RunInfo`
- [`fixtures`](fixtures.md) — a publisher that uses this module and never touches the car
