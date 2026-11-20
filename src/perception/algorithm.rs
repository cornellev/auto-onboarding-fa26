use crate::{shared::grid::Grid, shared::types::Observation};

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
        for &point in &_observation.scan.points {
            let relative_to_car = _observation.scan.sensor_in_vehicle.transform(point);

            let relative_to_world = _observation.state.pose.transform(relative_to_car);
            let gridPoints = self.map.cell(relative_to_world);
            if gridPoints.is_some(){
                self.map.set_occupied(gridPoints.unwrap().0, gridPoints.unwrap().0);
            }

        }
        Some(&self.map)
    }
}
