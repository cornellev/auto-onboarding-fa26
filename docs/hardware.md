# Maintainer hardware handoff

The required physical demo is autonomous start-to-goal navigation with obstacle avoidance. Freshmen should reuse their three algorithms. Hardware drivers, firmware, calibration, and localization are maintainer deliverables.

## Pi-side contract

Implement `hardware::VehicleIo` for the Raspberry Pi 5 support layer. The desktop starter has no physical adapter or arming command.

- `observe(timeout)` returns a fresh `Observation` or no sample within the timeout. Pose is encoder/IMU odometry at LiDAR capture time, expressed in a surveyed local world frame. The pose reference is the rear axle midpoint.
- LiDAR endpoints remain sensor-local. Supply measured sensor mounting extrinsics relative to the rear axle.
- Timestamps are monotonic integer nanoseconds since the run start. Maintain a bounded pose history and align/interpolate pose at capture time; report failure when alignment or localization is unavailable.
- `drive` accepts finite SI speed/steering commands, positive left, forward speed at most 0.3 m/s. The support layer must independently reject commands outside calibrated limits.
- `stop` must command zero motion. RP2040 firmware must stop independently after no fresh command for at most 300 ms, including Pi crash/link loss. Do not let a transport keep replaying an old motion command indefinitely.

Before connecting the existing runner, call `validate_hardware_spec`. Replace the supplied **synthetic** geometry with measured wheelbase, footprint, rear axle offset, sensor pose, steering limits, and speed limits, then mark the specification calibrated only after the checks below. The boolean is an administrative gate, not proof of safety.

Initial pose is established by physically placing the rear axle on a marked start with a known heading, then zeroing odometry. Survey the goal in that local frame. Encoder/IMU dead reckoning drifts; use a short static course, measure the drift, and stop if the estimate becomes invalid. Do not ask freshmen to implement an EKF or discover an undocumented actuator protocol to complete onboarding.

## Readiness evidence, due November 19

Maintain a dated report with measured values and evidence for:

- Wheel radius/ticks-per-revolution, wheelbase, footprint, sensor extrinsics, steering zero/sign/limits, speed calibration, and braking distance at the allowed speed.
- Correct forward travel and both steering directions with the wheels safely supported first, then a low-speed supervised ground test.
- Straight travel, one turn, and a short return experiment with independent tape/marker measurements of position and heading error.
- Valid scan/pose timestamps and rejection of stale data, non-finite commands, missing sensors, and localization loss.
- Independent stop tests for silence, unplugged Pi/RP2040 link, killed Pi process, and physical emergency stop. Record actual stop latency and distance.
- Safe startup/disarmed behavior and manual reposition/reset between runs. Do not automatically reset a moving physical vehicle.

Before November 21, validate that a conforming adapter can feed the same runner with student stubs and the car remains stationary. Connect a completed student stack only after its simulation tests pass. Any I/O error must leave the drive output stopped; retain the firmware watchdog even when software cleanup works.

## Demonstration course

Use a short, flat, static, low-speed course with one clearly visible obstacle, generous clearance, and space for the measured turning radius. Establish speed from braking/clearance measurements, capped at 0.3 m/s. A supervising maintainer controls the emergency stop.

Require navigation and obstacle avoidance on December 5. Confirm arrival against physical course markings; successful odometry alone is insufficient. Save a video/log and explain drift or tracking limitations. If this support layer is not ready, that is a maintainer integration blocker, not an algorithm failure by freshmen.

Device-specific drivers and the RP2040 wire protocol are intentionally outside this repository until the team supplies the actual hardware interface.
