use crate::{runtime::runner::Update, shared::grid::Grid, shared::types::*, Result};
use serde::Serialize;
use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::{Path as FilePath, PathBuf},
};

#[derive(Debug, Serialize)]
pub struct Summary {
    pub status: String,
    pub goal_reached: bool,
    pub elapsed_s: f64,
    pub final_distance_m: Option<f64>,
    pub traveled_m: f64,
    pub mean_tracking_error_m: Option<f64>,
    pub collisions: Option<u64>,
    pub simulator_passed: Option<bool>,
    pub termination_reason: Option<String>,
}

impl Default for Summary {
    /**
     * Creates a summary for a run that has not started.
     * @return A summary with zero counters and unavailable measurements set to None.
     */
    fn default() -> Self {
        Self {
            status: "not started".into(),
            goal_reached: false,
            elapsed_s: 0.0,
            final_distance_m: None,
            traveled_m: 0.0,
            mean_tracking_error_m: None,
            collisions: None,
            simulator_passed: None,
            termination_reason: None,
        }
    }
}

pub struct Reporter {
    pub directory: PathBuf,
    csv: BufWriter<File>,
    events: BufWriter<File>,
    pub trajectory: Vec<Point2>,
    pub source_time_ns: Option<u64>,
    tracking_sum: f64,
    tracking_count: usize,
    pub summary: Summary,
}

impl Reporter {
    /**
     * Creates a results directory and opens the update CSV and node-event log.
     * Existing updates.csv and node-events.jsonl files in the directory are truncated.
     * @param directory The directory where run artifacts will be written.
     * @return A new reporter, or a filesystem or CSV-header write error.
     */
    pub fn new(directory: impl AsRef<FilePath>) -> Result<Self> {
        let directory = directory.as_ref().to_path_buf();
        fs::create_dir_all(&directory)?;
        let mut csv = BufWriter::new(File::create(directory.join("updates.csv"))?);
        writeln!(csv,"time_s,x_m,y_m,yaw_rad,speed_mps,command_speed_mps,command_steering_rad,tracking_error_m,compute_us,saturated,source_age_ns")?;
        Ok(Self {
            events: BufWriter::new(File::create(directory.join("node-events.jsonl"))?),
            directory,
            csv,
            trajectory: vec![],
            source_time_ns: None,
            tracking_sum: 0.0,
            tracking_count: 0,
            summary: Summary::default(),
        })
    }

    /**
     * Appends a node event to the JSON Lines log and flushes it to the file.
     * @param node The name of the node that produced the event.
     * @param detail The event description.
     * @param time_ns The event timestamp in simulation nanoseconds.
     * @param pid The process ID of the node.
     * @return Ok(()) if the event is written and flushed, or an I/O error.
     */
    pub fn event(&mut self, node: &str, detail: &str, time_ns: u64, pid: u32) -> Result<()> {
        writeln!(
            self.events,
            "{}",
            serde_json::json!({"node":node,"pid":pid,"detail":detail,"time_ns":time_ns})
        )?;
        self.events.flush()?;
        Ok(())
    }

    /**
     * Records an update, extends the trajectory, and refreshes distance and tracking statistics.
     * @param observation The capture-aligned vehicle state, scan, and simulation timestamp.
     * @param update The command, path, status, and saturation result for this update.
     * @param goal The target position in map coordinates, in meters.
     * @param compute_us The computation time for the update, in microseconds.
     * @return Ok(()) if the CSV row is written and flushed, or an I/O error.
     */
    pub fn record(
        &mut self,
        observation: &Observation,
        update: &Update,
        goal: Point2,
        compute_us: u128,
    ) -> Result<()> {
        let point = observation.state.pose.position;
        if let Some(last) = self.trajectory.last() {
            self.summary.traveled_m += last.distance(point);
        }
        self.trajectory.push(point);
        let tracking = tracking_error(point, &update.path);
        if let Some(error) = tracking {
            self.tracking_sum += error;
            self.tracking_count += 1;
        }
        self.summary.status = update.status.into();
        self.summary.goal_reached = update.status == "goal reached";
        self.summary.elapsed_s = observation.time_ns as f64 / 1e9;
        self.summary.final_distance_m = Some(point.distance(goal));
        self.summary.mean_tracking_error_m =
            (self.tracking_count > 0).then(|| self.tracking_sum / self.tracking_count as f64);
        writeln!(
            self.csv,
            "{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{},{},{},{}",
            self.summary.elapsed_s,
            point.x,
            point.y,
            observation.state.pose.yaw_rad,
            observation.state.speed_mps,
            update.command.speed_mps,
            update.command.steering_rad,
            tracking.map(|v| v.to_string()).unwrap_or_default(),
            compute_us,
            update.saturated,
            self.source_time_ns
                .and_then(|t| observation.time_ns.checked_sub(t))
                .map(|n| n.to_string())
                .unwrap_or_default()
        )?;
        self.csv.flush()?;
        Ok(())
    }

    /**
     * Flushes updates, writes summary.json and view.svg, and prints the run result.
     * @param map The occupancy grid to draw in the final view.
     * @param path The requested route in map coordinates, in meters.
     * @param goal The goal position to mark in the final view.
     * @return Ok(()) if the artifacts are written, or a validation, serialization, or I/O error.
     */
    pub fn finish(&mut self, map: &Grid, path: &[Point2], goal: Point2) -> Result<()> {
        self.csv.flush()?;
        fs::write(
            self.directory.join("summary.json"),
            serde_json::to_string_pretty(&self.summary)? + "\n",
        )?;
        write_svg(
            &self.directory.join("view.svg"),
            map,
            path,
            &self.trajectory,
            goal,
        )?;
        println!(
            "{}; goal distance: {}; collisions: {}\nResults: {}",
            self.summary.status,
            self.summary
                .final_distance_m
                .map(|d| format!("{d:.2} m"))
                .unwrap_or_else(|| "unavailable".into()),
            self.summary
                .collisions
                .map(|n| n.to_string())
                .unwrap_or_else(|| "unavailable; inspect simulator results/Replay".into()),
            self.directory.display()
        );
        Ok(())
    }
}

/**
 * Finds the shortest distance from a point to the segments of a path.
 * @param point The vehicle position in the same coordinate frame as the path.
 * @param path The route waypoints; a single waypoint is treated as a point target.
 * @return The distance in meters, or None if the path is empty.
 */
pub fn tracking_error(point: Point2, path: &[Point2]) -> Option<f64> {
    if path.is_empty() {
        return None;
    }
    if path.len() == 1 {
        return Some(point.distance(path[0]));
    }
    path.windows(2)
        .map(|p| {
            let (dx, dy) = (p[1].x - p[0].x, p[1].y - p[0].y);
            let length = dx * dx + dy * dy;
            let t = if length > 0.0 {
                (((point.x - p[0].x) * dx + (point.y - p[0].y) * dy) / length).clamp(0.0, 1.0)
            } else {
                0.0
            };
            point.distance(Point2 {
                x: p[0].x + t * dx,
                y: p[0].y + t * dy,
            })
        })
        .reduce(f64::min)
}

/**
 * Writes an SVG showing occupied cells, the requested route, the actual trajectory, and the goal.
 * @param file The output file path; an existing file is overwritten.
 * @param map The occupancy grid defining the view's bounds and obstacles.
 * @param path The requested route in map coordinates, in meters.
 * @param trajectory The recorded vehicle positions in map coordinates, in meters.
 * @param goal The target position in map coordinates, in meters.
 * @return Ok(()) if the view is written, or a grid-validation or I/O error.
 */
pub fn write_svg(
    file: &FilePath,
    map: &Grid,
    path: &[Point2],
    trajectory: &[Point2],
    goal: Point2,
) -> Result<()> {
    map.validate()?;
    let mut out = BufWriter::new(File::create(file)?);
    let w = map.width as f64 * map.resolution_m;
    let h = map.height as f64 * map.resolution_m;
    let scale = (840.0 / w).min(500.0 / h);
    let pixel = |p: Point2| {
        (
            (p.x - map.origin.x) * scale + 30.0,
            540.0 - (p.y - map.origin.y) * scale,
        )
    };
    writeln!(out,"<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 900 600\" role=\"img\" aria-labelledby=\"title desc\"><title id=\"title\">Onboarding map and trajectory</title><desc id=\"desc\">Black: occupied cells. Blue: requested path. Orange: actual trajectory. Green: goal. Coordinates are meters in the world frame.</desc><rect width=\"900\" height=\"600\" fill=\"#fafafa\"/><text x=\"30\" y=\"24\" font-family=\"sans-serif\" font-size=\"16\">World map · x right, y up · meters</text>")?;
    writeln!(
        out,
        "<rect x=\"30\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"white\" stroke=\"#888\"/>",
        540.0 - h * scale,
        w * scale,
        h * scale
    )?;
    for row in 0..map.height {
        for col in 0..map.width {
            if map.blocked(col, row) {
                let p = map.center(col, row).unwrap();
                let (x, y) = pixel(p);
                let size = map.resolution_m * scale;
                writeln!(
                    out,
                    "<rect x=\"{}\" y=\"{}\" width=\"{size}\" height=\"{size}\" fill=\"#242424\"/>",
                    x - size / 2.0,
                    y - size / 2.0
                )?;
            }
        }
    }
    for (points, color) in [(path, "#1757b8"), (trajectory, "#c24f00")] {
        let points = points
            .iter()
            .map(|p| {
                let (x, y) = pixel(*p);
                format!("{x:.2},{y:.2}")
            })
            .collect::<Vec<_>>()
            .join(" ");
        writeln!(
            out,
            "<polyline points=\"{points}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"3\"/>"
        )?;
    }
    let (x, y) = pixel(goal);
    writeln!(out,"<circle cx=\"{x}\" cy=\"{y}\" r=\"7\" fill=\"#147445\"/><text x=\"30\" y=\"575\" font-family=\"sans-serif\" font-size=\"14\">■ Occupied   Blue: path   Orange: actual   Green: goal</text></svg>")?;
    Ok(())
}
