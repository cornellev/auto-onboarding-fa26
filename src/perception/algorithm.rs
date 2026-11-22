use std::collections::{HashSet, VecDeque};

use crate::shared::{grid::Grid, types::Observation};

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
        let mut queue: VecDeque<(i64, i64, usize)> = VecDeque::new();
        for &pt in observation.scan.points.iter() {
            if let Some((col, row)) = self.map.cell(
                observation
                    .state
                    .pose
                    .transform(observation.scan.sensor_in_vehicle.transform(pt)),
            ) {
                queue.push_back((col as i64, row as i64, 0));
            }
        }
        self.propagate(queue, 2);
        Some(&self.map)
    }

    pub fn propagate(
        &mut self,
        mut preloaded_queue: VecDeque<(i64, i64, usize)>,
        inflation_cells: usize,
    ) {
        let mut seen: HashSet<(i64, i64)> = HashSet::new();
        while let Some(cell) = preloaded_queue.pop_front() {
            let (col, row, depth) = cell;

            if depth > inflation_cells || seen.contains(&(col, row)) {
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
                (col - 1, row + 1, depth + 1),
            ];

            for next in candidates.into_iter() {
                if !seen.contains(&(next.0, next.1)) {
                    preloaded_queue.push_back(next);
                }
            }
        }
    }
}
