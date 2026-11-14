use crate::shared::{grid::Grid, protocol::pb::Point2, types::Observation, types::Pose2};

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
        //we use Bresenham's algo here and for now just rewrite entire map each time instead of doing like prob stuff
        //for each endpoint, run the algo and open points get a 0 and closed ones get a 255 or a 1, to be determined
        //everytime, I think im just going to make a new map instead of factoring in old measurements to start, but
        //potentially if on a new scan there was open area, then could reduce number in a space and vice versa.
        //implementation likely needs these: `sensor_in_vehicle` and `Pose2::transform`
        for pt in _observation.scan.points.iter(){
            if let Some((col, row)) = self.map.cell(*pt){
                
                self.map.set_occupied(col, row);
            }
            else{
                {}
            }
        }
        true.then_some(&self.map)
        // ...till here.
    }
}
