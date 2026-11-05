//! Wire formats stay here; student code never sees ROS messages or byte buffers.
use crate::{types::*, Result};
use serde::Deserialize;

pub const CLOUD: &str = "/sensors/front_lidar/points";
pub const POSE: &str = "/oracle/vehicle/odometry";
pub const CLOCK: &str = "/clock";
pub const TF: &str = "/tf";
pub const COMMAND: &str = "/controls/command";
pub const WAYPOINTS: &str = "/planning/waypoints";

#[derive(Debug, Clone, Deserialize)]
pub struct Stamp {
    pub sec: i64,
    pub nanosec: u32,
}
impl Stamp {
    pub fn ns(&self) -> Result<u64> {
        if self.sec < 0 || self.nanosec >= 1_000_000_000 {
            return Err(crate::error("Invalid simulation timestamp"));
        }
        (self.sec as u64)
            .checked_mul(1_000_000_000)
            .and_then(|n| n.checked_add(self.nanosec as u64))
            .ok_or_else(|| crate::error("Timestamp overflow"))
    }
}
#[derive(Debug, Clone, Deserialize)]
pub struct Header {
    pub stamp: Stamp,
    pub frame_id: String,
}
#[derive(Debug, Clone, Deserialize)]
pub struct Field {
    pub name: String,
    pub offset: usize,
    pub datatype: u8,
    pub count: usize,
}
#[derive(Debug, Clone, Deserialize)]
pub struct Cloud {
    pub header: Header,
    pub height: usize,
    pub width: usize,
    pub fields: Vec<Field>,
    pub is_bigendian: bool,
    pub point_step: usize,
    pub row_step: usize,
    pub data: Vec<u8>,
}

pub fn decode_cloud(cloud: &Cloud) -> Result<(u64, Vec<Point2>)> {
    if cloud.header.frame_id != "front_lidar_frame" {
        return Err(crate::error("LiDAR frame must be front_lidar_frame"));
    }
    let count = cloud
        .width
        .checked_mul(cloud.height)
        .filter(|n| *n <= 100_000)
        .ok_or_else(|| crate::error("PointCloud2 point count overflow or limit exceeded"))?;
    let row_bytes = cloud
        .width
        .checked_mul(cloud.point_step)
        .ok_or_else(|| crate::error("Point stride overflow"))?;
    let bytes = cloud
        .height
        .checked_mul(cloud.row_step)
        .ok_or_else(|| crate::error("Row stride overflow"))?;
    if cloud.point_step == 0
        || cloud.row_step < row_bytes
        || bytes != cloud.data.len()
        || bytes > 16_000_000
    {
        return Err(crate::error(
            "Invalid PointCloud2 strides or payload length",
        ));
    }
    let field = |name: &str| -> Result<usize> {
        let fields: Vec<_> = cloud.fields.iter().filter(|f| f.name == name).collect();
        if fields.len() != 1 {
            return Err(crate::error(format!(
                "Expected exactly one LiDAR {name} field"
            )));
        }
        let f = fields[0];
        if f.datatype != 7
            || f.count != 1
            || f.offset.checked_add(4).is_none_or(|n| n > cloud.point_step)
        {
            return Err(crate::error(format!(
                "LiDAR {name} must be one FLOAT32 within point_step"
            )));
        }
        Ok(f.offset)
    };
    let offsets = [field("x")?, field("y")?, field("z")?];
    if offsets
        .iter()
        .enumerate()
        .any(|(i, a)| offsets.iter().skip(i + 1).any(|b| a.abs_diff(*b) < 4))
    {
        return Err(crate::error("Overlapping LiDAR coordinate fields"));
    }
    let mut points = Vec::with_capacity(count);
    for row in 0..cloud.height {
        for col in 0..cloud.width {
            let base = row * cloud.row_step + col * cloud.point_step;
            let read = |offset| {
                let bytes: [u8; 4] = cloud.data[base + offset..base + offset + 4]
                    .try_into()
                    .unwrap();
                (if cloud.is_bigendian {
                    f32::from_be_bytes(bytes)
                } else {
                    f32::from_le_bytes(bytes)
                }) as f64
            };
            let (x, y, z) = (read(offsets[0]), read(offsets[1]), read(offsets[2]));
            // Missing returns may be NaN; they are not obstacle endpoints.
            if !x.is_finite() || !y.is_finite() || !z.is_finite() {
                continue;
            }
            if z.abs() > 0.01 {
                return Err(crate::error(
                    "Expected a planar LiDAR scan; check elevation and mount",
                ));
            }
            if x.hypot(y) > 0.001 {
                points.push(Point2 { x, y });
            }
        }
    }
    Ok((cloud.header.stamp.ns()?, points))
}

#[derive(Deserialize)]
struct Vec3 {
    x: f64,
    y: f64,
    z: f64,
}
#[derive(Deserialize)]
struct Quaternion {
    x: f64,
    y: f64,
    z: f64,
    w: f64,
}
#[derive(Deserialize)]
struct Pose {
    position: Vec3,
    orientation: Quaternion,
}
#[derive(Deserialize)]
struct PoseCov {
    pose: Pose,
}
#[derive(Deserialize)]
struct Twist {
    linear: Vec3,
}
#[derive(Deserialize)]
struct TwistCov {
    twist: Twist,
}

#[derive(Deserialize)]
struct Transform {
    translation: Vec3,
    rotation: Quaternion,
}
#[derive(Deserialize)]
struct StampedTransform {
    header: Header,
    child_frame_id: String,
    transform: Transform,
}
#[derive(Deserialize)]
pub struct Transforms {
    transforms: Vec<StampedTransform>,
}

pub fn decode_tf(tf: Transforms, spec: &VehicleSpec) -> Result<Option<(u64, VehicleState)>> {
    let mut base = None;
    for link in tf.transforms {
        if link.header.frame_id == "map" && link.child_frame_id == "odom" {
            let p = link.transform.translation;
            let q = link.transform.rotation;
            if [p.x, p.y, p.z, q.x, q.y, q.z, q.w - 1.0]
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1e-6)
            {
                return Err(crate::error(
                    "Training courses require identity map -> odom",
                ));
            }
        } else if link.header.frame_id == "odom" && link.child_frame_id == "base_link" {
            base = Some(decode_pose(
                Odometry {
                    header: link.header,
                    child_frame_id: link.child_frame_id,
                    pose: PoseCov {
                        pose: Pose {
                            position: link.transform.translation,
                            orientation: link.transform.rotation,
                        },
                    },
                    twist: TwistCov {
                        twist: Twist {
                            linear: Vec3 {
                                x: 0.0,
                                y: 0.0,
                                z: 0.0,
                            },
                        },
                    },
                },
                spec,
            )?);
        }
    }
    Ok(base)
}
#[derive(Deserialize)]
pub struct Odometry {
    header: Header,
    child_frame_id: String,
    pose: PoseCov,
    twist: TwistCov,
}

pub fn decode_pose(odom: Odometry, spec: &VehicleSpec) -> Result<(u64, VehicleState)> {
    if odom.header.frame_id != "odom" || odom.child_frame_id != "base_link" {
        return Err(crate::error("Expected odom -> base_link vehicle pose"));
    }
    let q = odom.pose.pose.orientation;
    let p = odom.pose.pose.position;
    let v = odom.twist.twist.linear;
    let norm = q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w;
    if [p.x, p.y, p.z, v.x, v.y, v.z, q.x, q.y, q.z, q.w]
        .iter()
        .any(|n| !n.is_finite())
        || (norm - 1.0).abs() > 0.01
        || q.x.abs() > 0.01
        || q.y.abs() > 0.01
    {
        return Err(crate::error(
            "Non-finite or non-planar odometry, or invalid quaternion",
        ));
    }
    let yaw = (2.0 * (q.w * q.z + q.x * q.y)).atan2(1.0 - 2.0 * (q.y * q.y + q.z * q.z));
    let base = Pose2 {
        position: Point2 { x: p.x, y: p.y },
        yaw_rad: yaw,
    };
    Ok((
        odom.header.stamp.ns()?,
        VehicleState {
            pose: Pose2 {
                position: base.transform(spec.rear_axle_in_base),
                yaw_rad: yaw,
            },
            speed_mps: v.x,
        },
    ))
}

pub fn command_value(
    command: DriveCommand,
    time_ns: u64,
    sequence: u32,
) -> Result<orchestrator_protocol::Value> {
    let mode = if command == DriveCommand::STOP {
        "stop"
    } else {
        "velocity"
    };
    use orchestrator_protocol::Value as V;
    if !command.speed_mps.is_finite() || !command.steering_rad.is_finite() {
        return Err(crate::error("Cannot encode non-finite commands"));
    }
    let seconds = i32::try_from(time_ns / 1_000_000_000)?;
    Ok(message(vec![
        (
            "header",
            message(vec![
                (
                    "stamp",
                    message(vec![
                        ("sec", V::I32(seconds)),
                        ("nanosec", V::U32((time_ns % 1_000_000_000) as u32)),
                    ]),
                ),
                ("frame_id", V::String("base_link".into())),
            ]),
        ),
        ("sequence", V::U32(sequence)),
        ("mode", V::String(mode.into())),
        (
            "deadline_ns",
            V::F64(time_ns.saturating_add(300_000_000) as f64),
        ),
        ("steering_angle", V::F64(command.steering_rad)),
        ("steering_angle_velocity", V::F64(0.0)),
        ("speed", V::F64(command.speed_mps)),
        ("acceleration", V::F64(0.0)),
        ("jerk", V::F64(0.0)),
    ]))
}

pub fn waypoint_value(path: &[Point2]) -> Result<orchestrator_protocol::Value> {
    use orchestrator_protocol::Value as V;
    if path
        .iter()
        .any(|p| !(p.x as f32).is_finite() || !(p.y as f32).is_finite())
    {
        return Err(crate::error("Waypoints exceed finite FLOAT32 range"));
    }
    let points = path
        .iter()
        .map(|p| {
            message(vec![
                ("x", V::F32(p.x as f32)),
                ("y", V::F32(0.0)),
                ("z", V::F32((0.0 - p.y) as f32)),
            ])
        })
        .collect();
    Ok(message(vec![("points", V::Array(points))]))
}

fn message(fields: Vec<(&str, orchestrator_protocol::Value)>) -> orchestrator_protocol::Value {
    orchestrator_protocol::Value::Message(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn commands_encode_against_the_actual_bundled_schema() {
        let bundle: serde_json::Value =
            serde_json::from_str(include_str!("../../scenarios/empty.bundle.json")).unwrap();
        let registry = orchestrator_protocol::MessageRegistry::new();
        for (name, definition) in bundle["resolved"]["schemas"].as_object().unwrap() {
            orchestrator_protocol::load_message_definition(
                &registry,
                name,
                definition.as_str().unwrap(),
            )
            .unwrap();
        }
        for command in [
            DriveCommand::STOP,
            DriveCommand {
                speed_mps: 0.1,
                steering_rad: -0.2,
            },
        ] {
            let value = command_value(command, 1_234_567_890, 42).unwrap();
            let encoded = orchestrator_protocol::encode(
                &registry,
                "sensor_fusion_msgs/StampedAckermannDrive",
                &value,
            )
            .unwrap();
            let (_, decoded) = orchestrator_protocol::decode(&registry, &encoded).unwrap();
            assert_eq!(decoded, value);
        }
    }
    fn cloud(big: bool) -> Cloud {
        let data = [1.0_f32, -2.0, 0.0]
            .into_iter()
            .flat_map(|v| {
                if big {
                    v.to_be_bytes()
                } else {
                    v.to_le_bytes()
                }
            })
            .collect();
        Cloud {
            header: Header {
                stamp: Stamp {
                    sec: 0,
                    nanosec: 100,
                },
                frame_id: "front_lidar_frame".into(),
            },
            height: 1,
            width: 1,
            fields: ["x", "y", "z"]
                .into_iter()
                .enumerate()
                .map(|(i, n)| Field {
                    name: n.into(),
                    offset: i * 4,
                    datatype: 7,
                    count: 1,
                })
                .collect(),
            is_bigendian: big,
            point_step: 12,
            row_step: 12,
            data,
        }
    }
    #[test]
    fn pointcloud_endianness_padding_and_invalid_inputs() {
        for big in [false, true] {
            assert_eq!(
                decode_cloud(&cloud(big)).unwrap().1,
                vec![Point2 { x: 1.0, y: -2.0 }]
            );
        }
        let mut c = cloud(false);
        c.row_step = 16;
        c.data.extend([0; 4]);
        assert_eq!(decode_cloud(&c).unwrap().1.len(), 1);
        c.fields[1].offset = 2;
        assert!(decode_cloud(&c).is_err());
        let mut c = cloud(false);
        c.data.pop();
        assert!(decode_cloud(&c).is_err());
        let mut c = cloud(false);
        c.fields[0].datatype = 8;
        assert!(decode_cloud(&c).is_err());
        let mut c = cloud(false);
        c.width = usize::MAX;
        assert!(decode_cloud(&c).is_err());
        let mut c = cloud(false);
        c.header.frame_id = "map".into();
        assert!(decode_cloud(&c).is_err());
        let mut c = cloud(false);
        c.data[..4].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(decode_cloud(&c).unwrap().1.is_empty());
    }
    #[test]
    fn timestamps_and_waypoint_basis() {
        assert!(Stamp {
            sec: -1,
            nanosec: 0
        }
        .ns()
        .is_err());
        assert!(Stamp {
            sec: 1,
            nanosec: 1_000_000_000
        }
        .ns()
        .is_err());
        let json =
            serde_json::to_value(waypoint_value(&[Point2 { x: 2.0, y: 3.0 }]).unwrap()).unwrap();
        assert_eq!(json["points"][0]["z"], -3.0);
    }
}
