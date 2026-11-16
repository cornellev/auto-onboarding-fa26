# `synchronizer` — Pair a pose with a scan

**Source code:** [`src/perception/synchronizer.rs`](../../src/perception/synchronizer.rs)

A pose and a scan are published on different topics, and they can arrive in either order. This module keeps a few of each and hands back the newest scan whose capture time equals a stored pose. Capture times match as `u64` equality on `header.time_ns`. A near miss is not a match.

Perception's message loop and `Sim::observe` both use one `ObservationJoin`. The loop still has to check freshness itself. The join only answers "which pose belongs to this scan."

## `ObservationJoin` objects

*class* `perception::synchronizer::ObservationJoin`

Two maps from capture time to a header plus a value. One map holds vehicle states. The other holds scans. `Default` is `new`.

`ObservationJoin::new()`

Return an empty join.

`ObservationJoin::state(&mut self, header, state)`

Remember a pose under `header.time_ns`. A second pose with the same time replaces the first. After the insert, drop the oldest poses until at most 32 remain. The bound is on the number of distinct times, not on bytes.

`state` here is a plain [`VehicleState`](types.md). The header is the Protobuf header that arrived with it, kept so the caller can stamp a later message with the same capture.

`ObservationJoin::scan(&mut self, header, scan)`

Remember a scan under `header.time_ns`. A second scan with the same time replaces the first. After the insert, drop the oldest scans until at most 8 remain. Scans are larger than poses, so the queue is shorter.

`ObservationJoin::take(&mut self)`

Return the newest scan that has a pose at the same capture time, as `(scan header, state header, observation)`.

Search the scan times from newest to oldest. The first time that is also a key in the pose map is the match. Remove that scan. Clone the pose and leave it stored, so a later scan at an older time can still find it. Drop every stored scan whose time is less than or equal to the match, so an older unmatched scan does not sit behind a newer one forever. Poses stay until the 32-entry bound evicts them.

The observation's `time_ns` is the matched time. Its `state` and `scan` are the stored values. Return `None` when no scan time is present in the pose map. Calling `take` again without a new scan returns `None`, because the matched scan was removed.

A scan that arrives before its pose sits in the scan map until the pose shows up or a newer match causes `take` to drop it. A pose that arrives before its scan sits in the pose map until a scan at that time arrives, or until newer poses push it past 32.

## See also

- [`perception-node`](perception-node.md) — the loop that feeds this join and publishes a map
- [`sim`](sim.md) — `Sim::observe`, which uses the same join on the launcher side
- [`protocol`](protocol.md) — `Freshness`, the age check that wraps a successful `take`
