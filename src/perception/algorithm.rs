use crate::{shared::grid::Grid, shared::types::Observation};

pub struct Perception {
    pub map: Grid,
}

impl Perception {
    pub fn new(map: Grid) -> Self {
        Self { map }
    }

    /// Convert this scan into your persistent map. Return None until ready.
    pub fn update(&mut self, _observation: &Observation) -> Option<&Grid> {
        // Student task: see docs/challenges.md#perception.
        None
    }
}
