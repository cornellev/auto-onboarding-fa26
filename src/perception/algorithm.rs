use crate::shared::{grid::Grid, protocol::pb::Point2, types::Observation};

pub struct Perception {
    pub map: Grid,
}

impl Perception {
    /// Keep this grid, and update it as scans come in.
    pub fn new(map: Grid) -> Self {
        Self { map }
    }

    /// Turn one scan into an updated map. Return it when you have one.
    /// This starter returns `None`, so the car stays stopped until you write it.
    /// The writeup is `docs/project.md`, under Perception.
    pub fn update(&mut self, _observation: &Observation) -> Option<&Grid> {
        // Edit me...
        let pose = &_observation.state.pose;
        for pt in _observation.scan.points.iter(){
            if let Some((col, row)) = self.map.cell(pose.transform(*pt)){
                self.map.set_occupied(col, row);
            }
            else{
                {}
            }
        }
        if self.map.validate().is_ok(){Some(&self.map)}
        else{None}
        // ...till here.
    }
}
