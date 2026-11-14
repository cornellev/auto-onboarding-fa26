use crate::{shared::grid::Grid, shared::types::Observation};
use std::{f64::consts::E, thread::{self, JoinHandle}};

struct Update(usize, usize, u8);

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
        for point in &_observation.scan.points {
            let dp_r = _observation.scan.sensor_in_vehicle.transform(point.clone());  // robot-centric displacement of point
            let dp_g = _observation.state.pose.transform(dp_r);  // global displacement of point (from global origin)
            let cell = self.map.cell(dp_g);
            match cell {
                Some((c, r)) => {
                    self.map.set_occupied(c, r);
                }
                None => ()
            }
        }
        Some(&self.map)
        // let inflation_radius = (10.0 / self.map.resolution_m).ceil() as i32;
        // for point in &_observation.scan.points {
        //     let dp_r = _observation.scan.sensor_in_vehicle.transform(point.clone());  // robot-centric displacement of point
        //     let dp_g = _observation.state.pose.transform(dp_r);  // global displacement of point (from global origin)

        //     let cell = self.map.cell(dp_g);

        //     match cell {
        //         Some((c, r)) => {
        //             let mut is_edge_cell = false;
        //             for (c_, r_) in vec![(c-1, r), (c, r-1), (c+1, r), (c, r+1)] {
        //                 is_edge_cell |= !self.map.is_occupied(c_, r_, None);
        //             }
                    
        //             if !is_edge_cell {
        //                 self.map.set_occupied(c, r);
        //                 continue; 
        //             }

        //             let mut c_handles: Vec<JoinHandle<Vec<Update>>> = vec![];
        //             for ofs_c in -inflation_radius..=inflation_radius {
        //                 let cp = c as i32 + ofs_c;
        //                 if cp < 0 { continue; }

        //                 c_handles.push(thread::spawn(move || {
        //                     let cp = cp as usize;

        //                     let ofs_r_mag = ((inflation_radius.pow(2) - ofs_c.pow(2)) as f64).sqrt().ceil() as i32;

        //                     let mut r_handles: Vec<JoinHandle<Update>> = vec![];
        //                     for ofs_r in -ofs_r_mag..=ofs_r_mag {
        //                         let rp = r as i32 + ofs_r;
        //                         if rp < 0 { continue; }
                                
        //                         r_handles.push(thread::spawn(move || {
        //                             let rp = rp as usize;

        //                             let d2 = ((ofs_c.pow(2) + ofs_r.pow(2)) as f64).sqrt();
        //                             let weight = (E.powf(-d2) * 255.0).ceil() as u8;

        //                             Update(cp, rp, weight)
        //                         }));
        //                     }

        //                     let mut updates: Vec<Update> = vec![];
        //                     for handle in r_handles {
        //                         updates.push(handle.join().unwrap());
        //                     }

        //                     updates
        //                 }));
        //             }

        //             for handle in c_handles {
        //                 for update in handle.join().unwrap() {
        //                     self.map.set_weight(update.0, update.1, update.2);
        //                 }
        //             }
        //         }
        //         None => (),
        //     }
        // }
        // Some(&self.map)
    }
}
