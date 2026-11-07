use crate::{shared::types::Point2, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Grid {
    pub origin: Point2,
    pub resolution_m: f64,
    pub width: usize,
    pub height: usize,
    pub occupied: Vec<bool>,
}

/**
 * Grid represents a 2D occupancy grid with a given origin, resolution, width, and height.
 * Each cell in the grid can be either occupied or unoccupied.
 * The grid provides methods to check if a point is within the grid, if a cell is blocked, and if a circle or line segment is clear of obstacles.
 */
impl Grid {
    /**
     * Creates a new empty grid with the given origin, resolution, width, and height.
     * @param origin The origin of the grid in world coordinates.
     * @param resolution_m The resolution of the grid in meters per cell.
     * @param width The width of the grid in cells.
     * @param height The height of the grid in cells.
     * @return A new empty grid.
     */
    pub fn empty(origin: Point2, resolution_m: f64, width: usize, height: usize) -> Result<Self> {
        // shortcut: unmarked cells are optimistic free space; add unknown cells before unseen/dynamic courses.
        let count = width
            .checked_mul(height)
            .filter(|n| *n <= 1_000_000)
            .ok_or_else(|| crate::error("Grid is too large"))?;
        let grid = Self {
            origin,
            resolution_m,
            width,
            height,
            occupied: vec![false; count],
        };
        grid.validate()?;
        Ok(grid)
    }

    /**
     * Validates the grid's dimensions, resolution, origin, and storage.
     * @return Ok(()) if the grid is valid, or an error if it is not.
     */
    pub fn validate(&self) -> Result<()> {
        if !self.origin.is_finite()
            || !self.resolution_m.is_finite()
            || self.resolution_m < 0.001
            || self.width == 0
            || self.height == 0
            || self.width.checked_mul(self.height) != Some(self.occupied.len())
            || self.occupied.len() > 1_000_000
            || !(self.origin.x + self.width as f64 * self.resolution_m).is_finite()
            || !(self.origin.y + self.height as f64 * self.resolution_m).is_finite()
        {
            return Err(crate::error(
                "Invalid grid dimensions, resolution, origin, or storage",
            ));
        }
        Ok(())
    }

    /**
     * Returns the column and the row of the cell containing the given point, or None if the point is outside the grid.
     * @param point The point to check.
     * @return The column and row of the cell containing the point, or None if the point is outside the grid.
     */
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

    /**
     * Returns the center of the cell at the given column and row, or None if the cell is outside the grid.
     * @param col The column of the cell.
     * @param row The row of the cell.
     * @return The center of the cell (Point2), or None if the cell is outside the grid.
     */
    pub fn center(&self, col: usize, row: usize) -> Option<Point2> {
        (col < self.width && row < self.height).then_some(Point2 {
            x: self.origin.x + (col as f64 + 0.5) * self.resolution_m,
            y: self.origin.y + (row as f64 + 0.5) * self.resolution_m,
        })
    }

    /**
     * Returns true if the cell at the given column and row is blocked (occupied or outside the grid).
     * @param col The column of the cell.
     * @param row The row of the cell.
     * @return True if the cell is blocked, false otherwise.
     */
    pub fn blocked(&self, col: usize, row: usize) -> bool {
        col >= self.width || row >= self.height || self.occupied[row * self.width + col]
    }

    /**
     * Mutates the cell at the given column and row to be occupied or unoccupied.
     * @param col The column of the cell.
     * @param row The row of the cell.
     * @param occupied True to mark the cell as occupied, false to mark it as unoccupied.
     */
    pub fn set_occupied(&mut self, col: usize, row: usize, occupied: bool) {
        if col < self.width && row < self.height {
            self.occupied[row * self.width + col] = occupied;
        }
    }

    /**
     * Returns true if the circle with the given center and radius is clear of obstacles, false otherwise.
     * @param point The center of the circle.
     * @param radius_m The radius of the circle in meters.
     * @return True if the circle is clear, false otherwise.
     */
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

    /**
     * Returns true if the line segment from point a to point b is clear of obstacles, false otherwise.
     * @param a The start point of the segment.
     * @param b The end point of the segment.
     * @param radius_m The radius of the segment in meters.
     * @return True if the segment is clear, false otherwise.
     */
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
