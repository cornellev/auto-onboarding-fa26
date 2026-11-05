use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

impl Point2 {
    pub fn distance(self, other: Self) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

/// REP-103, meters and radians. Vehicle poses refer to the rear axle midpoint.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Pose2 {
    pub position: Point2,
    pub yaw_rad: f64,
}

impl Pose2 {
    pub fn transform(self, point: Point2) -> Point2 {
        let (sin, cos) = self.yaw_rad.sin_cos();
        Point2 {
            x: self.position.x + cos * point.x - sin * point.y,
            y: self.position.y + sin * point.x + cos * point.y,
        }
    }

    pub fn is_finite(self) -> bool {
        self.position.is_finite() && self.yaw_rad.is_finite()
    }
}

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
    pub points: Vec<Point2>,
    pub sensor_in_vehicle: Pose2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    pub time_ns: u64,
    pub state: VehicleState,
    pub scan: LidarScan,
}

impl Observation {
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
    /// Rear axle in the simulator's base_link REP-103 frame.
    pub rear_axle_in_base: Point2,
    pub sensor_in_vehicle: Pose2,
    pub max_speed_mps: f64,
    pub max_steering_rad: f64,
    /// False for the supplied synthetic training vehicle. Never arm hardware with it.
    pub hardware_calibrated: bool,
}

impl VehicleSpec {
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
            || self.max_speed_mps > 0.3
            || self.max_steering_rad >= std::f64::consts::FRAC_PI_2
            || !self.rear_axle_in_base.is_finite()
            || !self.sensor_in_vehicle.is_finite()
        {
            return Err(crate::error(
                "Invalid vehicle specification or speed cap above 0.3 m/s",
            ));
        }
        Ok(())
    }

    /// Conservative circle around the rear axle; independent of vehicle heading.
    pub fn clearance_m(&self) -> f64 {
        (self.length_m / 2.0 + self.rear_axle_in_base.x.abs())
            .hypot(self.width_m / 2.0 + self.rear_axle_in_base.y.abs())
    }

    pub fn limit(&self, command: DriveCommand) -> crate::Result<DriveCommand> {
        if !command.speed_mps.is_finite() || !command.steering_rad.is_finite() {
            return Err(crate::error("Non-finite command rejected; stopping"));
        }
        Ok(DriveCommand {
            speed_mps: command.speed_mps.clamp(0.0, self.max_speed_mps),
            steering_rad: command
                .steering_rad
                .clamp(-self.max_steering_rad, self.max_steering_rad),
        })
    }
}
