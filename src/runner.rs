use crate::{
    grid::Grid,
    student::{controls::Controller, perception::Perception, planning::Planner},
    types::*,
    Result,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Course {
    pub manifest_id: String,
    pub resolved_hash: String,
    pub goal: Point2,
    pub goal_tolerance_m: f64,
    pub timeout_s: f64,
    pub vehicle: VehicleSpec,
    pub grid: Grid,
    pub control_path: Path,
}

impl Course {
    pub fn validate(&self) -> Result<()> {
        self.vehicle.validate()?;
        self.grid.validate()?;
        if !self.goal.is_finite()
            || self.grid.cell(self.goal).is_none()
            || !self.goal_tolerance_m.is_finite()
            || self.goal_tolerance_m <= 0.0
            || !self.timeout_s.is_finite()
            || self.timeout_s <= 0.0
            || self.control_path.is_empty()
            || self.control_path.iter().any(|p| !p.is_finite())
        {
            return Err(crate::error(
                "Invalid course goal, bounds, timeout, or controls fixture",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Observe,
    Controls,
    Autonomous,
    DriveTest,
}

#[derive(Debug)]
pub struct Update {
    pub command: DriveCommand,
    pub status: &'static str,
    pub path: Path,
    pub finished: bool,
    pub saturated: bool,
}

pub struct Runner {
    pub perception: Perception,
    pub planner: Planner,
    pub controller: Controller,
    pub course: Course,
    pub mode: Mode,
    last_time_ns: Option<u64>,
}

impl Runner {
    pub fn new(course: Course, mode: Mode) -> Result<Self> {
        course.validate()?;
        Ok(Self {
            perception: Perception::new(course.grid.clone()),
            planner: Planner::default(),
            controller: Controller::new(course.vehicle.clone()),
            course,
            mode,
            last_time_ns: None,
        })
    }

    pub fn update(&mut self, observation: &Observation) -> Result<Update> {
        observation.validate()?;
        let time = observation.time_ns;
        if self.last_time_ns.is_some_and(|last| time <= last) {
            return Err(crate::error(
                "Observation timestamp repeated or moved backwards; restart the run",
            ));
        }
        let dt = self
            .last_time_ns
            .map_or(0.1, |last| (time - last) as f64 / 1e9);
        self.last_time_ns = Some(time);
        let stopped = |status, finished| Update {
            command: DriveCommand::STOP,
            status,
            path: vec![],
            finished,
            saturated: false,
        };
        if time as f64 / 1e9 >= self.course.timeout_s {
            return Ok(stopped("timeout", true));
        }
        if self.mode == Mode::Observe {
            return Ok(stopped("observing", false));
        }
        if self.mode == Mode::DriveTest {
            return Ok(if time >= 1_000_000_000 {
                stopped("drive-test complete", true)
            } else {
                Update {
                    command: DriveCommand {
                        speed_mps: 0.1,
                        steering_rad: 0.1,
                    },
                    ..stopped("drive-test", false)
                }
            });
        }
        if observation.state.pose.position.distance(self.course.goal)
            <= self.course.goal_tolerance_m
        {
            return Ok(stopped("goal reached", true));
        }
        let path = if self.mode == Mode::Controls {
            self.course.control_path.clone()
        } else {
            let Some(map) = self.perception.update(observation) else {
                return Ok(stopped(
                    "perception unavailable: implement Perception::update",
                    false,
                ));
            };
            map.validate()?;
            let Some(path) = self.planner.plan(map, &observation.state, self.course.goal) else {
                return Ok(stopped(
                    "no path returned: planner unfinished or goal unreachable",
                    false,
                ));
            };
            validate_path(
                map,
                &path,
                observation.state.pose.position,
                self.course.goal,
                self.course.vehicle.clearance_m(),
                self.course.goal_tolerance_m,
            )?;
            path
        };
        if path.is_empty() || path.iter().any(|p| !p.is_finite()) {
            return Err(crate::error("Empty or non-finite path"));
        }
        let requested = self.controller.command(&observation.state, &path, dt);
        let command = self.course.vehicle.limit(requested)?;
        Ok(Update {
            command,
            status: if command == DriveCommand::STOP {
                "controller returned stop"
            } else {
                "driving"
            },
            path,
            finished: false,
            saturated: command != requested,
        })
    }
}

pub fn validate_path(
    map: &Grid,
    path: &[Point2],
    start: Point2,
    goal: Point2,
    radius: f64,
    goal_tolerance: f64,
) -> Result<()> {
    map.validate()?;
    if path.is_empty()
        || path.len() > 100_000
        || path.iter().any(|p| !p.is_finite())
        || path[0].distance(start) > map.resolution_m * 2.0
        || path.last().unwrap().distance(goal) > goal_tolerance
        || !map.clear(start, radius)
        || !map.segment_clear(start, path[0], radius)
        || path.iter().any(|p| !map.clear(*p, radius))
        || path
            .windows(2)
            .any(|p| !map.segment_clear(p[0], p[1], radius))
    {
        return Err(crate::error(
            "Invalid path: check endpoints, bounds, clearance, and segments between waypoints",
        ));
    }
    Ok(())
}
