use crate::{shared::types::Point2, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Grid {
    pub origin: Point2,
    pub resolution_m: f64,
    pub width: usize,
    pub height: usize,
    pub weights: Vec<u8>,
}

/// Columns follow world x, rows follow world y, and cell `(col, row)` lives at `row * width + col`.
/// A fresh grid is all weight 0. The default cutoff treats that as free.
impl Grid {
    pub fn empty(origin: Point2, resolution_m: f64, width: usize, height: usize) -> Result<Self> {
        // shortcut: weight 0 is free at the default cutoff; store >= 128 before unseen space must block.
        let count = width
            .checked_mul(height)
            .filter(|n| *n <= 1_000_000)
            .ok_or_else(|| crate::error("Grid is too large"))?;
        let grid = Self {
            origin,
            resolution_m,
            width,
            height,
            weights: vec![0; count],
        };
        grid.validate()?;
        Ok(grid)
    }

    pub fn validate(&self) -> Result<()> {
        if !self.origin.is_finite()
            || !self.resolution_m.is_finite()
            || self.resolution_m < 0.001
            || self.width == 0
            || self.height == 0
            || self.width.checked_mul(self.height) != Some(self.weights.len())
            || self.weights.len() > 1_000_000
            || !(self.origin.x + self.width as f64 * self.resolution_m).is_finite()
            || !(self.origin.y + self.height as f64 * self.resolution_m).is_finite()
        {
            return Err(crate::error(
                "Invalid grid dimensions, resolution, origin, or storage",
            ));
        }
        Ok(())
    }

    /// Column and row of a world point. `None` if the point is off the map, or isn't a real number.
    pub fn cell(&self, point: Point2) -> Option<(usize, usize)> {
        if !point.is_finite() {
            return None;
        }
        let col = ((point.x - self.origin.x) / self.resolution_m).floor();
        let row = ((point.y - self.origin.y) / self.resolution_m).floor();
        if col < 0.0 || row < 0.0 || col >= self.width as f64 || row >= self.height as f64 {
            None
        } else {
            Some((col as usize, row as usize))
        }
    }

    /// World position of the middle of that cell. `None` if the column or row is off the map.
    pub fn center(&self, col: usize, row: usize) -> Option<Point2> {
        (col < self.width && row < self.height).then_some(Point2 {
            x: self.origin.x + (col as f64 + 0.5) * self.resolution_m,
            y: self.origin.y + (row as f64 + 0.5) * self.resolution_m,
        })
    }

    /// At least 128, or off the map. Off the map counts as blocked so a route can't step outside.
    pub fn blocked(&self, col: usize, row: usize) -> bool {
        col >= self.width || row >= self.height || self.is_occupied(col, row, None)
    }

    /// True when the stored weight is at least `threshold`. `None` means 128.
    /// Off the map is not occupied.
    pub fn is_occupied(&self, col: usize, row: usize, threshold: Option<u8>) -> bool {
        let threshold = threshold.unwrap_or(128);
        col < self.width && row < self.height && self.weights[row * self.width + col] >= threshold
    }

    /// Write 255. A column or row off the map is ignored.
    pub fn set_occupied(&mut self, col: usize, row: usize) {
        self.set_weight(col, row, 255);
    }

    /// Write one belief, from 0 to 255. 0 clears the cell. A column or row off the map is ignored.
    pub fn set_weight(&mut self, col: usize, row: usize, weight: u8) {
        if col < self.width && row < self.height {
            self.weights[row * self.width + col] = weight;
        }
    }

    /// Whether a circle around `point` misses every occupied cell and stays on the map.
    pub fn clear(&self, point: Point2, radius_m: f64) -> bool {
        if !point.is_finite() || !radius_m.is_finite() || radius_m < 0.0 {
            return false;
        }
        let Some((min_col, min_row)) = self.cell(Point2 {
            x: point.x - radius_m,
            y: point.y - radius_m,
        }) else {
            return false;
        };
        let Some((max_col, max_row)) = self.cell(Point2 {
            x: point.x + radius_m,
            y: point.y + radius_m,
        }) else {
            return false;
        };
        for row in min_row..=max_row {
            for col in min_col..=max_col {
                if self.blocked(col, row) {
                    let x = self.origin.x + col as f64 * self.resolution_m;
                    let y = self.origin.y + row as f64 * self.resolution_m;
                    let nearest = Point2 {
                        x: point.x.clamp(x, x + self.resolution_m),
                        y: point.y.clamp(y, y + self.resolution_m),
                    };
                    if nearest.distance(point) <= radius_m {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Whether the strip from `a` to `b`, thickened by `radius_m`, stays clear.
    pub fn segment_clear(&self, a: Point2, b: Point2, radius_m: f64) -> bool {
        let distance = a.distance(b);
        if !distance.is_finite() {
            return false;
        }
        let count = (distance / (self.resolution_m / 2.0)).ceil().max(1.0);
        if count > 100_000.0 {
            return false;
        }
        let count = count as usize;
        (0..=count).all(|index| {
            let t = index as f64 / count as f64;
            self.clear(
                Point2 {
                    x: a.x + t * (b.x - a.x),
                    y: a.y + t * (b.y - a.y),
                },
                radius_m + distance / count as f64 / 2.0,
            )
        })
    }
}
