use std::collections::{VecDeque,HashSet};
use std::cmp;

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
    pub fn update(&mut self, observation: &Observation) -> Option<&Grid> {
        // Edit me...
        let pose = &observation.state.pose;
        let mut queue:VecDeque<(i64,i64,usize)> = VecDeque::new();
        for pt in observation.scan.points.iter(){
            if let Some((col, row)) = self.map.cell(pose.transform(*pt)){
                queue.push_back((col as i64,row as i64, 0));
            }
            else{
                {}
            }
        }
        Perception::propagate(self, queue, 2);
        if self.map.validate().is_ok(){Some(&self.map)}
        else{None}
        // ...till here.
    }

    pub fn propagate(&mut self, mut preloaded_queue: VecDeque<(i64,i64,usize)>, inflation_sq_units: usize){
        let mut seen:HashSet<(i64,i64)> = HashSet::new();
            while let Some(cell) = preloaded_queue.pop_front(){

                let (col,row,depth) = cell;

                if depth > inflation_sq_units || seen.contains(&(col, row)){
                    continue;
                }
                if col < 0 || row < 0 || col >= self.map.width as i64 || row >= self.map.height as i64 {
                    continue;
                }

                self.map.set_occupied(col as usize, row as usize);
                seen.insert((col, row));

                let candidates = [
                    (col + 1, row, depth + 1),
                    (col, row + 1, depth + 1),
                    (col - 1, row, depth + 1),
                    (col, row - 1, depth + 1),
                    (col - 1, row - 1, depth + 1),
                    (col + 1, row + 1, depth + 1),
                    (col + 1, row - 1, depth + 1), 
                    (col - 1, row + 1, depth + 1) 
                ];

                for next in candidates.into_iter(){
                    preloaded_queue.push_back(next);
                }

            }
    }
}
