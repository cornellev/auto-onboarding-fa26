//! Provided capture-time alignment for pose and LiDAR.
use crate::shared::protocol::pb;

/// Join only exact capture times. Different scan/pose arrival orders are supported.
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
    pub fn state(&mut self, h: pb::Header, state: crate::shared::types::VehicleState) {
        self.states.insert(h.time_ns, (h, state));
        while self.states.len() > 32 {
            self.states.pop_first();
        }
    }
    pub fn scan(&mut self, h: pb::Header, scan: crate::shared::types::LidarScan) {
        self.scans.insert(h.time_ns, (h, scan));
        while self.scans.len() > 8 {
            self.scans.pop_first();
        }
    }
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
