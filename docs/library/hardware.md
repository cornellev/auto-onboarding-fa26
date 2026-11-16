# `hardware` — What a physical car would implement

**Source code:** [`src/runtime/hardware.rs`](../../src/runtime/hardware.rs)

This module is the boundary between the runner and a vehicle. The laptop starter does not include a driver for the Pi. The implementation that ships is [`Sim`](sim.md), which satisfies the same three methods against the simulator. A physical car would provide its own type, with measured geometry, and the runner would call these methods on it.

The trait is small on purpose. Something else has to stop the car when `drive` stops being called. The comment on `drive` states that requirement, because a dropped laptop must not leave the wheels at the last speed.

## The trait

*trait* `runtime::hardware::VehicleIo`

`observe(&mut self, timeout)`

Wait up to `timeout` for a pose and a scan from the same moment. Return `Ok(Some(observation))` when a pair is ready, `Ok(None)` when the wait expires, and an error when the source is no longer trustworthy.

The observation uses the plain [`Observation`](types.md): nanoseconds since the run started, the rear-axle pose and speed, and the scan in the sensor frame. `Sim::observe` also rejects a pair whose freshness window has closed, and a routing run that has been replaced.

`drive(&mut self, command)`

Send a forward speed, in meters per second, and a steering angle, in radians. The car itself has to stop if these calls stop arriving. `Sim::drive` clamps the command to the vehicle spec before it publishes, and it refuses a non-finite command.

`stop(&mut self)`

Ask the car to stop. `Sim::stop` is `drive(DriveCommand::STOP)`, so the stop travels through the same command path as a normal command, including the case where no scan has arrived yet.

## A measured car

`validate_hardware_spec(spec)`

Return `Ok(())` when `spec` passes [`VehicleSpec::validate`](types.md) and `hardware_calibrated` is `true`. That flag means somebody measured the geometry, the sensor mount, and the actuator limits. The simulator minicar leaves it `false`, which is correct for the laptop starter and an error here.

Return `"Hardware requires measured geometry, sensor extrinsics, and actuator calibration"` when the flag is false. A spec that fails `validate` returns that error instead, including a speed cap above 1 m/s or a steering cap of π/2 or more.

Nothing in the simulator path calls this function. It is the gate for a future hardware backend, so a course file full of starter numbers cannot be loaded as a physical car by mistake.

## See also

- [`sim`](sim.md) — the `VehicleIo` implementation that ships
- [`types`](types.md) — `Observation`, `DriveCommand`, and `VehicleSpec`
