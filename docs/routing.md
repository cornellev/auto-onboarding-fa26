# Routing without the jargon

Protobuf defines the form of a message. A topic is its named mailbox. Zenoh delivers
messages. Orchestrator connects the browser simulator to those mailboxes.

1. The simulator sends its existing position and laser messages to orchestrator.
2. Provided converters create Protobuf state and LiDAR messages. Orchestrator sends
   them to the current run's `state` and `lidar` topics.
3. Perception reads both, joins their capture timestamps, and publishes `map`.
4. Planning reads `map` and publishes `path`.
5. Controls reads `state`/`path` and publishes `command`.
6. Orchestrator validates commands and sends safe stamped setpoints to the simulator.

The Rust nodes exchange their maps and paths directly through Zenoh. Orchestrator
also listens for paths to update the existing browser overlay. The launcher manages
the run and reports results; it does not relay the algorithm messages.

Every run has its own prefix, `cev/onboarding/v1/<run-id>/`. Old packets cannot enter
the next run. Headers contain capture time and sequence identity. Map/path/command
messages retain the original scan identity. Republishing an old plan never makes it
fresh. Missing paths stop the vehicle. Invalid/stale inputs or lost transport retire
the active graph; recovery starts a fresh run.

For another team message, register its descriptor and a route, then write producer
code. The simulator's Client API is:

```js
await client.bindProtobuf({ runId, fingerprint });
await client.subscribeProtobuf('reply', (value) => console.log(value));
await client.publishProtobuf('note', { title: 'hello', nonce: '18446744073709551615' });
```

Values between browser and orchestrator use standard ProtoJSON; 64-bit integers
are decimal strings. Orchestrator validates and encodes binary Protobuf for Zenoh.
Its registered route declares the expected message type, destination, and allowed
directions. The reverse route decodes messages into values for browser subscribers.
There is no new Python codec for each message. Sensor decoding, coordinate changes,
and actuator rules still need explicit adapters because a schema cannot describe
their physical meaning.

Visual field wiring is deferred. A future visual builder can call this same API.
The current API and code examples work independently of that editor.

Keep field numbers stable and reserve deleted numbers when evolving the schema.
ProtoJSON field/enum renames are compatibility changes. Regenerate all artifacts
together and start a new run after a schema update. Normal autonomous RunInfo has
no supplied path; paths are included only for explicit controls exercises.
