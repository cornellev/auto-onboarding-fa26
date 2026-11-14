use crate::shared::{grid::Grid, types::{Observation}};
use std::{cmp::Ordering, collections::{BinaryHeap, HashSet}, f64::consts::E};

#[derive(Copy, Clone, PartialEq)]
struct InflationState {
    c: usize,
    r: usize,
    c0: usize,
    r0: usize,
    d: f64,
}

impl Ord for InflationState {
    fn cmp(&self, other: &Self) -> Ordering {
        other.d.total_cmp(&self.d)
            .then_with(|| other.c.cmp(&self.c))
            .then_with(|| other.r.cmp(&self.r))
            .then_with(|| other.c0.cmp(&self.c0))
            .then_with(|| other.r0.cmp(&self.r0))
    }
}

impl PartialOrd for InflationState {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for InflationState {}

pub struct Perception {
    pub map: Grid,
}

impl Perception {
    /// Keep this grid, and update it as scans come in.
    pub fn new(map: Grid) -> Self {
        Self { map }
    }

    fn adjacent_coordinates(&mut self, c: usize, r: usize) -> Vec<(usize, usize)> {
        let mut v = vec![];
        if c > 0 { v.push((c-1, r)); }
        if r > 0 { v.push((c, r-1)); }
        if c < self.map.width - 1 { v.push((c+1, r)) }
        if r < self.map.height - 1 { v.push((c, r+1)) }
        v
    }

    /// Turn one scan into an updated map. Return it when you have one.
    /// This starter returns `None`, so the car stays stopped until you write it.
    /// The writeup is `docs/project.md`, under Perception.
    pub fn update(&mut self, _observation: &Observation) -> Option<&Grid> {  // todo: figure out resetting between updates and stuff bc the adding is weird, maybe can pass multiple into other functions
        let mut pq: BinaryHeap<InflationState> = BinaryHeap::new();

        for point in &_observation.scan.points {
            let dp_r = _observation.scan.sensor_in_vehicle.transform(point.clone());  // robot-centric displacement of point
            let dp_g = _observation.state.pose.transform(dp_r);  // global displacement of point (from global origin)
            let cell = self.map.cell(dp_g);

            match cell {
                Some((c, r)) => { pq.push(InflationState { c, r, c0: c, r0 : r, d: 0.0 }); }
                None => ()
            }
        }

        let inflation_radius = 0.5 / self.map.resolution_m;
        
        let mut vis: HashSet<(usize, usize)> = HashSet::new();

        while !pq.is_empty() {
            match pq.pop() {
                Some(InflationState { c, r, c0, r0, d }) => {
                    if vis.contains(&(c, r)) { continue; }
                    vis.insert((c, r));

                    let weight = if d <= 0.00001 {
                        255
                    } else if d <= inflation_radius {
                        254
                    } else {
                        (E.powf(inflation_radius - d) * 255.0) as u8
                    };

                    println!("{c} {r} {c0} {r0} {weight}");

                    if weight == 0 { continue; }

                    self.map.set_weight(c, r, weight);

                    for (c_, r_) in self.adjacent_coordinates(c, r) {
                        pq.push(InflationState { c: c_, r: r_, c0, r0, d: ((c_.abs_diff(c0).pow(2) + r_.abs_diff(r0).pow(2)) as f64).sqrt() });
                    }
                }
                None => ()
            }
        }

        Some(&self.map)
    }
}
