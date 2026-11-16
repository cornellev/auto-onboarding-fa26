use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

impl Point2 {
    /// Straight-line distance, in meters. Both points need to be in the same frame.
    pub fn distance(self, other: Self) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

/// Meters and radians. A vehicle pose is the middle of the rear axle.
/// Heading is counterclockwise from +x. In this frame, +x is forward and +y is left.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Pose2 {
    pub position: Point2,
    pub yaw_rad: f64,
}

impl Pose2 {
    /// Rotate `point` by this heading, then add this position.
    /// One call moves a point up a single frame: sensor into the car, or car into the world.
    pub fn transform(self, point: Point2) -> Point2 {
        let (sin, cos) = self.yaw_rad.sin_cos();
        Point2 {
            x: self.position.x + cos * point.x - sin * point.y,
            y: self.position.y + sin * point.x + cos * point.y,
        }
    }

    /// Undo `transform`. Subtract this position, then rotate by the opposite heading.
    /// A world point becomes a point in this pose's frame. +x is forward and +y is left.
    pub fn inverse_transform(self, point: Point2) -> Point2 {
        let dx = point.x - self.position.x;
        let dy = point.y - self.position.y;
        let (sin, cos) = self.yaw_rad.sin_cos();
        Point2 {
            x: cos * dx + sin * dy,
            y: -sin * dx + cos * dy,
        }
    }

    pub fn is_finite(self) -> bool {
        self.position.is_finite() && self.yaw_rad.is_finite()
    }
}

/// Fold an angle into -π..π. A NaN or an infinity stays NaN.
pub fn wrap_angle(radians: f64) -> f64 {
    radians.sin().atan2(radians.cos())
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VehicleState {
    pub pose: Pose2,
    pub speed_mps: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LidarScan {
    /// Endpoints in the sensor frame. +x is forward, +y is left.
    pub points: Vec<Point2>,
    /// Where the LiDAR is mounted, relative to the rear axle.
    pub sensor_in_vehicle: Pose2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    /// Nanoseconds since the run started, at the moment this scan was taken.
    pub time_ns: u64,
    pub state: VehicleState,
    pub scan: LidarScan,
}

impl Observation {
    /// Pose, speed, sensor mount, and scan points all have to be real numbers.
    /// More than 100,000 points is rejected.
    pub fn validate(&self) -> crate::Result<()> {
        if !self.state.pose.is_finite()
            || !self.state.speed_mps.is_finite()
            || !self.scan.sensor_in_vehicle.is_finite()
            || self.scan.points.len() > 100_000
            || self.scan.points.iter().any(|p| !p.is_finite())
        {
            return Err(crate::error("Invalid pose, speed, or LiDAR points"));
        }
        Ok(())
    }
}

pub type Path = Vec<Point2>;

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DriveCommand {
    pub speed_mps: f64,
    pub steering_rad: f64,
}

impl DriveCommand {
    pub const STOP: Self = Self {
        speed_mps: 0.0,
        steering_rad: 0.0,
    };
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VehicleSpec {
    pub wheelbase_m: f64,
    pub length_m: f64,
    pub width_m: f64,
    /// Where the rear axle sits in the simulator's base_link frame.
    pub rear_axle_in_base: Point2,
    pub sensor_in_vehicle: Pose2,
    pub max_speed_mps: f64,
    pub max_steering_rad: f64,
    /// False until someone measures a physical car. The starter numbers are the simulator minicar.
    pub hardware_calibrated: bool,
}

impl VehicleSpec {
    /// Lengths and limits have to be real and positive.
    /// Speed tops out at 1 m/s, the simulator minicar's limit. Steering stays under a right angle.
    pub fn validate(&self) -> crate::Result<()> {
        if [
            self.wheelbase_m,
            self.length_m,
            self.width_m,
            self.max_speed_mps,
            self.max_steering_rad,
        ]
        .iter()
        .any(|v| !v.is_finite() || *v <= 0.0)
            || self.max_speed_mps > 1.0
            || self.max_steering_rad >= std::f64::consts::FRAC_PI_2
            || !self.rear_axle_in_base.is_finite()
            || !self.sensor_in_vehicle.is_finite()
        {
            return Err(crate::error(
                "Invalid vehicle specification or speed cap above 1 m/s",
            ));
        }
        Ok(())
    }

    /// A circle around the rear axle big enough to cover the car, whichever way it's pointing.
    pub fn clearance_m(&self) -> f64 {
        (self.length_m / 2.0 + self.rear_axle_in_base.x.abs())
            .hypot(self.width_m / 2.0 + self.rear_axle_in_base.y.abs())
    }

    /// Reject a command that is not a real number. This does not clamp.
    /// `controlled_update` uses this, then publishes the numbers `command` returned.
    pub fn require_finite(&self, command: DriveCommand) -> crate::Result<()> {
        if !command.speed_mps.is_finite() || !command.steering_rad.is_finite() {
            return Err(crate::error("Non-finite command rejected; stopping"));
        }
        Ok(())
    }

    /// Clamp speed to `0..=max_speed_mps` and steering to the car's limit.
    /// Errors if either number isn't real. The runner and the simulator publish this result.
    /// The controls node does not.
    pub fn limit(&self, command: DriveCommand) -> crate::Result<DriveCommand> {
        self.require_finite(command)?;
        Ok(DriveCommand {
            speed_mps: command.speed_mps.clamp(0.0, self.max_speed_mps),
            steering_rad: command
                .steering_rad
                .clamp(-self.max_steering_rad, self.max_steering_rad),
        })
    }
}
