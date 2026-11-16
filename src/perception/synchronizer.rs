//! Pairs a pose with the LiDAR scan from the same moment. A scan and a pose can arrive in either order.
use crate::shared::protocol::pb;

/// A pose and a scan match only when their capture times are equal.
pub struct ObservationJoin {
    states: std::collections::BTreeMap<u64, (pb::Header, crate::shared::types::VehicleState)>,
    scans: std::collections::BTreeMap<u64, (pb::Header, crate::shared::types::LidarScan)>,
}

impl Default for ObservationJoin {
    fn default() -> Self {
        Self::new()
    }
}

impl ObservationJoin {
    pub fn new() -> Self {
        Self {
            states: Default::default(),
            scans: Default::default(),
        }
    }
    /// Remember a pose. A repeat of the same time replaces the old one. Only the newest 32 are kept.
    pub fn state(&mut self, h: pb::Header, state: crate::shared::types::VehicleState) {
        self.states.insert(h.time_ns, (h, state));
        while self.states.len() > 32 {
            self.states.pop_first();
        }
    }
    /// Remember a scan. A repeat of the same time replaces the old one. Only the newest 8 are kept.
    pub fn scan(&mut self, h: pb::Header, scan: crate::shared::types::LidarScan) {
        self.scans.insert(h.time_ns, (h, scan));
        while self.scans.len() > 8 {
            self.scans.pop_first();
        }
    }
    /// The newest scan that has a pose from the same moment, if there is one.
    /// Older scans are dropped. The poses stay, in case a later scan needs them.
    pub fn take(&mut self) -> Option<(pb::Header, pb::Header, crate::shared::types::Observation)> {
        let time = self
            .scans
            .keys()
            .rev()
            .find(|t| self.states.contains_key(t))
            .copied()?;
        let (scan_h, scan) = self.scans.remove(&time)?;
        let (state_h, state) = self.states.get(&time)?.clone();
        self.scans.retain(|t, _| *t > time);
        Some((
            scan_h,
            state_h,
            crate::shared::types::Observation {
                time_ns: time,
                state,
                scan,
            },
        ))
    }
}
