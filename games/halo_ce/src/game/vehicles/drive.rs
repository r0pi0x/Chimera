//! The driving rules around the mass-point step (specs/halo_ce/
//! vehicles.md, "What the vehicle reads from its riders", "Speed
//! variables", "The jeep (Warthog)", "Sleeping, waking and the slow stop",
//! powered seats, flipping and pushes): `Drive` is a vehicle's per-tick
//! state (speed, slide and turn variables, wheel spin, driver power, sleep
//! and the flip) and `Drive::tick` runs one Halo tick of a vehicle: the
//! variables from the driver's controls, the type's channels, the step.
//!
//! Only the jeep's driving model is here (the Warthog); the scout, tank and
//! fighter are each a `prepare` arm away (their channels and magic force
//! and torque are in the spec).

use bevy::math::Vec3;

use super::data::{Kind, VehicleDef, vehicle_flags};
use super::physics::{self, Body, ChannelState, GroundScales, Level, PhysicsDef, StepOut};

/// The slow stop after falling asleep: ticks and velocity factor.
pub const STOP_TICKS: u8 = 15;
pub const STOP_DAMPING: f32 = 0.835;
/// Flipping: the longest flip, the up.z that ends it, the angular speed
/// scale, the up.z at or below which a vehicle offers the flip.
pub const FLIP_TICKS: u8 = 30;
pub const FLIP_DONE_UP: f32 = 0.9;
pub const FLIP_RATE: f32 = 0.3;
// The spec's own four digits (cos 45 degrees as the globals give it).
#[allow(clippy::approx_constant)]
pub const FLIP_UP_LIMIT: f32 = 0.7071;
/// Riders are thrown out of a grounded vehicle whose up.z is below this.
pub const EJECT_UP: f32 = 0.2;

/// A speed record: positive limit, negative limit, acceleration,
/// deceleration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpeedParams {
    pub pos: f32,
    pub neg: f32,
    pub acc: f32,
    pub dec: f32,
}

/// The variable-speed step S(x; P, δ).
pub fn speed_step(x: f32, p: SpeedParams, delta: f32) -> f32 {
    let a = delta.abs() * p.acc;
    let b = delta.abs() * p.dec;
    if delta > 0.0 {
        let x = if x <= -b {
            x + b
        } else if x >= 0.0 {
            x + a
        } else {
            (x / b + 1.0) * a
        };
        x.min(delta.abs() * p.pos)
    } else if delta < 0.0 {
        let x = if x >= b {
            x - b
        } else if x <= 0.0 {
            x - a
        } else {
            (x / b - 1.0) * a
        };
        x.max(-delta.abs() * p.neg)
    } else {
        x
    }
}

/// The seek T(x → target; P, δ).
pub fn seek(x: f32, target: f32, p: SpeedParams, delta: f32) -> f32 {
    if x > target {
        let y = speed_step(x, p, -delta);
        if y <= target { target } else { y }
    } else if x < target {
        let y = speed_step(x, p, delta);
        if y >= target { target } else { y }
    } else {
        x
    }
}

/// The steering angle σ = atan2(l·d, f·d) of a desired facing `d`.
pub fn steering_angle(body: &Body, d: Vec3) -> f32 {
    body.left().dot(d).atan2(body.forward.dot(d))
}

/// The driver's controls for one tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Controls {
    /// Whether anyone drives.
    pub driver: bool,
    /// The throttle forward (t_i) and left (t_j), each in −1..1.
    pub throttle: [f32; 2],
    /// The desired facing (world, unit).
    pub facing: Vec3,
    /// Jump (the brake) and crouch.
    pub jump: bool,
    pub crouch: bool,
}

/// A flip in progress.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flip {
    /// Approach type 1 to 4.
    pub kind: u8,
    pub ticks: u8,
}

/// A vehicle's driving state.
#[derive(Clone, Debug, PartialEq)]
pub struct Drive {
    pub body: Body,
    /// The speed, slide and turn variables.
    pub speed: f32,
    pub slide: f32,
    pub turn: f32,
    /// The wheel spin in [0, circumference).
    pub wheel: f32,
    /// The driver's seat power P0.
    pub power: f32,
    pub brake: bool,
    pub asleep: bool,
    /// Whether sleeping is allowed (the spec's driving cases turn it off).
    pub sleep_enabled: bool,
    pub stop_ticks: u8,
    pub flip: Option<Flip>,
    pub on_ground: bool,
    /// Consecutive ticks with some point touching the ground (capped 255).
    pub ground_ticks: u32,
    /// Ticks with no point touching and no antigravity trace hitting
    /// (capped 255).
    pub airborne_ticks: u32,
    /// Collision force and torque gathered for the next step.
    pub collision_force: Vec3,
    pub collision_torque: Vec3,
    /// The last step's findings.
    pub last: StepOut,
}

impl Drive {
    /// A vehicle at rest with its centre of mass at `position`.
    pub fn new(body: Body) -> Drive {
        Drive {
            body,
            speed: 0.0,
            slide: 0.0,
            turn: 0.0,
            wheel: 0.0,
            power: 0.0,
            brake: false,
            asleep: false,
            sleep_enabled: true,
            stop_ticks: 0,
            flip: None,
            on_ground: false,
            ground_ticks: 0,
            airborne_ticks: 0,
            collision_force: Vec3::ZERO,
            collision_torque: Vec3::ZERO,
            last: StepOut::default(),
        }
    }

    /// The vehicle's speed parameters.
    pub fn speed_params(def: &VehicleDef) -> SpeedParams {
        SpeedParams {
            pos: def.max_forward_speed,
            neg: def.max_reverse_speed,
            acc: def.speed_acceleration,
            dec: def.speed_deceleration,
        }
    }

    /// The variables' step from the controls (vehicles.md "Speed
    /// variables", steps 1 and 2; the brake flag).
    pub fn update_variables(&mut self, def: &VehicleDef, c: &Controls) {
        let p = Self::speed_params(def);
        let (ti, _tj) = (c.throttle[0], c.throttle[1]);
        let opposite = def.flags & vehicle_flags::CONTROL_OPPOSITE_SPEED_SETS_BRAKE != 0
            && ((ti > 0.0 && self.speed < 0.0) || (ti < 0.0 && self.speed > 0.0));
        self.brake = c.jump || opposite;
        if self.brake {
            self.speed = seek(self.speed, 0.0, p, 1.0);
        } else {
            self.speed = seek(self.speed, ti, p, 1.0);
            // The slide record is 0 on every disc tag: σ_s stays 0.
        }
        let sigma = if c.driver {
            steering_angle(&self.body, c.facing)
        } else {
            0.0
        };
        match def.kind {
            Kind::Tank => {
                // The tank's turn (vehicles.md step 3) comes with its rule.
            }
            _ => {
                let left = def.max_left_turn.to_radians();
                let right = def.max_right_turn.to_radians();
                let target = if self.speed >= 0.0 { sigma } else { -sigma };
                let target = target.clamp(right.min(left), left.max(right));
                let rate = def.turn_rate.to_radians() / 30.0;
                self.turn += (target - self.turn).clamp(-rate, rate);
            }
        }
    }

    /// The driver's power P0 (powered seat 0).
    pub fn update_power(&mut self, def: &VehicleDef, driver: bool) {
        if driver {
            self.power = (self.power + 1.0 / (30.0 * def.power_up_time)).min(1.0);
        } else {
            self.power = (self.power - 1.0 / (30.0 * def.power_down_time)).max(0.0);
        }
    }

    /// The type's channels before the step (the jeep's: both channels at
    /// the speed variable, front yawed +τ, back −τ; the wheel spin).
    pub fn prepare(&mut self, def: &VehicleDef) -> Vec<ChannelState> {
        match def.kind {
            Kind::Jeep => {
                let circ = def.wheel_circumference;
                if circ > 0.0 {
                    self.wheel = (self.wheel + self.speed).rem_euclid(circ);
                }
                vec![
                    ChannelState {
                        ground_speed: self.speed,
                        yaw: self.turn,
                        antigrav: 0.0,
                    },
                    ChannelState {
                        ground_speed: self.speed,
                        yaw: -self.turn,
                        antigrav: 0.0,
                    },
                ]
            }
            _ => vec![
                ChannelState {
                    antigrav: self.power,
                    ..Default::default()
                };
                def.physics.channels.len()
            ],
        }
    }

    /// Whether the vehicle wakes this tick (flags 0 to 2).
    fn wakes(&self, def: &VehicleDef) -> bool {
        (def.flags & vehicle_flags::SPEED_WAKES_PHYSICS != 0 && self.speed != 0.0)
            || (def.flags & vehicle_flags::TURN_WAKES_PHYSICS != 0 && self.turn != 0.0)
            || (def.flags & vehicle_flags::DRIVER_POWER_WAKES_PHYSICS != 0 && self.power != 0.0)
    }

    /// Wake the vehicle (a push, a flip, a contact).
    pub fn wake(&mut self) {
        self.asleep = false;
        self.stop_ticks = 0;
    }

    /// One Halo tick of the vehicle, in the order of the spec's "Per-tick
    /// order" (its step numbers; 1, 3 and 4 are the caller's: the riders'
    /// controls in `c`, aiming, attachment).
    pub fn tick(
        &mut self,
        def: &VehicleDef,
        phys: &PhysicsDef,
        c: &Controls,
        level: &dyn Level,
        scales: &[GroundScales],
    ) {
        // 2. The driver's seat power.
        self.update_power(def, c.driver);
        // 5 to 7. The brake flag and the steering angle, the flip, the
        // speed, slide and turn variables (the flip sets the angular
        // velocity only, so σ is the same before and after it).
        self.flip_step(def);
        self.update_variables(def, c);
        // 8. Wake checks.
        if self.wakes(def) {
            self.wake();
        }
        // 9. The physics, or the slow stop while asleep.
        if !self.asleep {
            let channels = self.prepare(def);
            let (f, t) = (self.collision_force, self.collision_torque);
            self.collision_force = Vec3::ZERO;
            self.collision_torque = Vec3::ZERO;
            self.last = physics::step(phys, &mut self.body, &channels, level, scales, f, t);
            self.on_ground = self.last.touching > 0;
            self.ground_ticks = if self.on_ground { (self.ground_ticks + 1).min(255) } else { 0 };
            self.airborne_ticks = if self.on_ground || self.last.antigraving > 0 {
                0
            } else {
                (self.airborne_ticks + 1).min(255)
            };
            if self.sleep_enabled && self.last.at_rest {
                self.asleep = true;
                self.stop_ticks = STOP_TICKS;
            }
        } else if self.stop_ticks > 0 {
            self.stop_ticks -= 1;
            self.body.velocity *= STOP_DAMPING;
            self.body.angular_velocity *= STOP_DAMPING;
            self.body.position += self.body.velocity;
            let w = self.body.angular_velocity;
            let (f, u) = turned(&self.body, w);
            self.body.forward = f;
            self.body.up = u;
            if self.stop_ticks == 0 {
                self.body.velocity = Vec3::ZERO;
                self.body.angular_velocity = Vec3::ZERO;
            }
        }
    }

    /// An instant push (vehicles.md "Pushes on the vehicle"): v += Δv,
    /// ω += π (z × Δv); the vehicle wakes.
    pub fn push(&mut self, dv: Vec3) {
        self.body.velocity += dv;
        self.body.angular_velocity += std::f32::consts::PI * Vec3::Z.cross(dv);
        self.wake();
    }

    /// Start a flip from a player at `from` (vehicles.md "Flipping").
    pub fn start_flip(&mut self, from: Vec3) {
        let f = self.body.forward;
        let kind = if f.z.abs() > FLIP_UP_LIMIT {
            if f.z > 0.0 { 3 } else { 4 }
        } else {
            let d = self.body.position - from;
            if Vec3::Z.cross(d).dot(f) > 0.0 { 2 } else { 1 }
        };
        self.flip = Some(Flip { kind, ticks: 0 });
        self.wake();
    }

    /// The flip's tick: the angular velocity is set.
    fn flip_step(&mut self, def: &VehicleDef) {
        let Some(mut flip) = self.flip else { return };
        if flip.ticks >= FLIP_TICKS || self.body.up.z > FLIP_DONE_UP {
            self.flip = None;
            return;
        }
        self.body.angular_velocity = flip_omega(&self.body, flip.kind, def.flipping_angular_velocity);
        flip.ticks += 1;
        self.flip = Some(flip);
        self.wake();
    }
}

/// The angular velocity a flip of approach type `kind` (1 to 4) sets on
/// `body` (vehicles.md "Flipping"): strength k = clamp(−2 up.z, min, max),
/// about f × u (types 3, 4) or f + (f × u)(−f_z) (1, 2), at ±0.3 k.
pub fn flip_omega(body: &Body, kind: u8, limits: [f32; 2]) -> Vec3 {
    let (f, u) = (body.forward, body.up);
    let k = (-2.0 * u.z).clamp(limits[0], limits[1]);
    let axis = match kind {
        3 | 4 => f.cross(u),
        _ => f + f.cross(u) * (-f.z),
    };
    let sign = if matches!(kind, 2 | 4) { 1.0 } else { -1.0 };
    axis * (sign * FLIP_RATE * k)
}

fn turned(b: &Body, w: Vec3) -> (Vec3, Vec3) {
    let angle = w.length();
    let (f, u) = if angle > 0.0 {
        let q = bevy::math::Quat::from_axis_angle(w / angle, angle);
        (q * b.forward, q * b.up)
    } else {
        (b.forward, b.up)
    };
    let f = f.normalize();
    (f, (u - f * u.dot(f)).normalize())
}
