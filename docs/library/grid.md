# `grid` — Occupancy grids

**Source code:** [`src/shared/grid.rs`](../../src/shared/grid.rs)

This module stores a map as a rectangle of cells. Columns follow world x. Rows follow world y. Cell `(col, row)` is stored at `row * width + col` in `weights`. Each cell holds a weight from 0 to 255.

A fresh grid is all weight 0. The default cutoff treats a weight below 128 as free, so a new cell is free. That is a reasonable reading of these courses, where the obstacles sit still and the car can see them. The grid does not decide which LiDAR returns are obstacles. `Perception::update` does, and then calls `set_occupied` or `set_weight`.

Off the map, the four lookups disagree on purpose. `cell` returns `None`. `set_weight` and `set_occupied` leave the storage alone. `is_occupied` is false, so a scan does not invent a hit outside the map. `blocked` is true, so a route cannot step outside. `blocked` is the occupancy cutoff, a weight of at least 128. It is not "the car fits here." A planner waypoint has to pass `clear` with the car's clearance, and a step has to pass `segment_clear`.

The Protobuf `Grid` in [`messages`](messages.md) is a different type, with `u32` weights. `protocol::convert` copies between them. Planning and perception in this crate use the struct on this page.

## `Grid` objects

*class* `shared::grid::Grid`

`origin`

World position of the corner of cell `(0, 0)`, in meters. The cell covers `[origin.x, origin.x + resolution_m)` in x and the same interval in y. Columns increase with x. Rows increase with y.

`resolution_m`

Width and height of one cell, in meters. `validate` requires a finite value of at least 0.001, one millimeter.

`width`, `height`

Number of columns and rows. Both are `usize`. A zero extent fails `validate`.

`weights`

Row-major weights, length `width * height`, each a `u8`. Index `row * width + col`. A weight of 0 is the empty value `Grid::empty` writes. A weight of 255 is what `set_occupied` writes.

`Grid::empty(origin, resolution_m, width, height)`

Allocate a grid and fill every cell with 0. Return an error, `"Grid is too large"`, when `width * height` overflows or the product is greater than 1,000,000.

Then call `validate`. A failure there returns `"Invalid grid dimensions, resolution, origin, or storage"`. A `width` or `height` of zero takes that path: the product is zero, which is small enough to allocate, and `validate` rejects the empty extent. The same error covers a non-finite origin, a resolution below one millimeter, and an origin so large that `origin + extent` is not finite.

`Grid::validate(&self)`

Return `Ok(())` when all of the following hold.

- `origin` is finite, and so are `origin.x + width * resolution_m` and `origin.y + height * resolution_m`.
- `resolution_m` is finite and at least 0.001.
- `width` and `height` are both non-zero.
- `weights.len()` equals `width * height`, and that length is at most 1,000,000.

Otherwise return `"Invalid grid dimensions, resolution, origin, or storage"`. The weights themselves may be any byte. This check does not look at occupancy.

`Grid::cell(&self, point)`

Return the column and row of a world point:

```text
col = floor((point.x - origin.x) / resolution_m)
row = floor((point.y - origin.y) / resolution_m)
```

Return `None` when `point` is not finite, or when `col` or `row` is negative, or when `col >= width` or `row >= height`. The covered region is half-open: the near edges, at `origin`, belong to cell `(0, 0)`, and the far edges, at `origin + extent`, are off the map. A point that lands exactly on a grid line belongs to the cell whose lower corner is that line.

`Grid::center(&self, col, row)`

Return the world position of the middle of that cell:

```text
x = origin.x + (col as f64 + 0.5) * resolution_m
y = origin.y + (row as f64 + 0.5) * resolution_m
```

Return `None` when `col >= width` or `row >= height`. A planner that searches cells and has to hand back world points uses this. `validate_path` then checks those points.

`Grid::blocked(&self, col, row)`

Return `true` when the cell is off the map, or when its weight is at least 128. Off the map counts as blocked so a route cannot step outside. On the map this is `is_occupied(col, row, None)`.

`Grid::is_occupied(&self, col, row, threshold)`

Return `true` when the cell is on the map and its weight is at least `threshold`. `None` means 128. `Some(100)` treats a weight of 100 as occupied, which the default cutoff does not.

Return `false` when the column or row is off the map. Rust has no default arguments, so `None` is how a caller asks for 128.

`Grid::set_occupied(&mut self, col, row)`

Write 255 into the cell, by calling `set_weight(col, row, 255)`. A column or row off the map is ignored.

`Grid::set_weight(&mut self, col, row, weight)`

Write `weight` into the cell. The value is a belief from 0 to 255, and 0 clears the cell. A column or row off the map is ignored. The function does not clamp: a `u8` already holds only that range.

`Grid::clear(&self, point, radius_m)`

Return `true` when a circle of radius `radius_m` around `point` stays on the map and misses every blocked cell.

Return `false` immediately when `point` or `radius_m` is not finite, or when `radius_m` is negative. Then take the cells of the two corners `point ± (radius_m, radius_m)`, using `cell`. If either corner is off the map, return `false`. The whole axis-aligned square around the circle has to lie on the grid, so a circle that hangs over the edge is not clear. Because `cell` treats the far edge as outside, a circle that only touches that edge is not clear either.

Inside that rectangle, each blocked cell is tested as a rectangle of side `resolution_m` whose lower corner is `(origin.x + col * resolution_m, origin.y + row * resolution_m)`. If the nearest point of that rectangle to `point` is at a distance of `radius_m` or less, return `false`. Free cells are skipped. The occupancy cutoff inside this test is 128, the same cutoff as `blocked`. There is no threshold argument.

`Grid::segment_clear(&self, a, b, radius_m)`

Return `true` when the strip from `a` to `b`, thickened by `radius_m`, stays clear.

Let `distance` be `a.distance(b)`. Return `false` when that distance is not finite, which covers a non-finite endpoint. The number of steps is `ceil(distance / (resolution_m / 2))`, and at least 1. Return `false` when that count is greater than 100,000.

Sample `count + 1` points from `a` to `b`, including both ends. Sample `i` is `(1 - t) * a + t * b` with `t = i / count`. Each sample is passed to `clear` with radius `radius_m + distance / count / 2`. The added term is half the gap between samples, so the circles overlap and cover the strip between them. That radius is larger than `radius_m`. Two endpoints that each pass `clear` at `radius_m` can still fail the segment. One sample that fails `clear` fails the segment.

A path that only checks the cells under its waypoints can still cut a corner. `segment_clear` is the check for the line between two points. `validate_path` calls it for the step from the car to the first waypoint and for every step after that.

## See also

- [`types`](types.md) — `Point2`, and `VehicleSpec::clearance_m` for the radius
- [`validation`](validation.md) — the path check that calls `clear` and `segment_clear`
- [`messages`](messages.md) — the Protobuf `Grid` on a `MapUpdate`
