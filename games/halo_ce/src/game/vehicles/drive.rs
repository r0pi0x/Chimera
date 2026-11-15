//! The driving rules around the mass-point step (specs/halo_ce/
//! vehicles.md, "What the vehicle reads from its riders", "Speed
//! variables", "The jeep (Warthog)", "Sleeping, waking and the slow stop",
//! powered seats, flipping and pushes): `Drive` is a vehicle's per-tick
//! state (speed, slide and turn variables, wheel spin, driver power, sleep
//! and the flip) and `Drive::tick` runs one Halo tick of a vehicle: the
//! variables from the driver's controls, the type's channels, the step.
//!
//! The jeep's driving model (the Warthog: `prepare`'s channels) and the
//! scout's (the Ghost: every channel's antigravity at the driver's power,
//! `scout`'s drive, yaw, levelling and jump force and torque, the hover
//! fraction after the step) and the tank's (the Scorpion: its turn
//! variable from the speed variable, its treads at s ∓ τ); the fighter is
//! a `prepare` arm away. The vehicles' guns (the Ghost's and Scorpion's
//! drivers are their gunners: seat flag 3 in the data) come with the
//! turret work.
//!
//! Known shortcuts:
//! - The scout's water cut-off (drive, yaw and levelling stop in 0.5 WU of
//!   water) is not applied: the multiplayer maps' water is shallow and the
//!   engine has no water volumes for it.

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
/// The scout (vehicles.md "The scout", Constants): its yaw rate's change
/// limit, the steering beyond which grounded drive is cut, the hover
/// fraction's rate, the desired yaw rate's gain, the up.z at or below which
/// it neither drives nor steers, the stick and levelling torque scales, and
/// the jump's pitch torque and lift.
pub const SCOUT_YAW_ACCEL: f32 = 0.003_490_658_7;
pub const SCOUT_STEER_SHARP: f32 = std::f32::consts::FRAC_PI_4;
pub const HOVER_RATE: f32 = 0.1;
pub const SCOUT_YAW_GAIN: f32 = 0.006_981_317_4;
pub const SCOUT_UPSIDE_DOWN: f32 = -0.2;
pub const SCOUT_STICK_TORQUE: f32 = 0.001_551_403_8;
pub const SCOUT_LEVEL_TORQUE: f32 = 0.003_878_509_4;
pub const SCOUT_JUMP_PITCH: f32 = -0.005_817_764;
pub const SCOUT_JUMP_LIFT: f32 = 0.004;

/// The scout's desired yaw rate for steering angle `sigma`: sign(σ)
/// √(0.0069813174 |σ|), or σ/2 when σ is under twice that (|σ| < 0.0279
/// rad).
pub fn scout_yaw_rate(sigma: f32) -> f32 {
    if sigma == 0.0 {
        return 0.0;
    }
    let wd = sigma.signum() * (SCOUT_YAW_GAIN * sigma.abs()).sqrt();
    if wd.abs() > 1e-6 && sigma / wd < 2.0 { sigma / 2.0 } else { wd }
}

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
    /// The left and right tread positions in [0, circumference).
    pub treads: [f32; 2],
    /// The driver's seat power P0.
    pub power: f32,
    /// The scout's hover fraction h_v.
    pub hover: f32,
    /// The steering angle σ of the last controls.
    pub sigma: f32,
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
            treads: [0.0; 2],
            power: 0.0,
            hover: 0.0,
            sigma: 0.0,
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
        self.sigma = sigma;
        match def.kind {
            Kind::Tank => {
                // Step 3: no turn without speed; else toward the steering
                // angle's share of the forward maximum, at twice the speed
                // record's steps and limits.
                self.turn = if self.speed == 0.0 {
                    seek(self.turn, 0.0, p, 1.0)
                } else {
                    let target = (sigma * 2.0 / std::f32::consts::PI).clamp(-1.0, 1.0) * def.max_forward_speed;
                    seek(self.turn, target, p, 2.0)
                };
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
            Kind::Tank => {
                // The treads at s ∓ τ, each position advancing by its speed.
                let (left, right) = (self.speed - self.turn, self.speed + self.turn);
                let circ = def.wheel_circumference;
                if circ > 0.0 {
                    self.treads[0] = (self.treads[0] + left).rem_euclid(circ);
                    self.treads[1] = (self.treads[1] + right).rem_euclid(circ);
                }
                vec![
                    ChannelState {
                        ground_speed: left,
                        ..Default::default()
                    },
                    ChannelState {
                        ground_speed: right,
                        ..Default::default()
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

    /// The scout's preparation (vehicles.md "The scout", step 2): its drive
    /// toward the stick's velocity, the capped yaw torque toward the
    /// camera, the levelling and stick torques while partly grounded, the
    /// jump's lift; all times the driver's power. Returns the magic force
    /// and torque.
    fn scout(&self, def: &VehicleDef, phys: &PhysicsDef, c: &Controls, sigma: f32) -> (Vec3, Vec3) {
        let b = &self.body;
        let (f, l, u) = (b.forward, b.left(), b.up);
        let (mut fm, mut tm) = (Vec3::ZERO, Vec3::ZERO);
        // No water reaches 0.5 WU on the multiplayer maps (not modelled).
        if u.z <= SCOUT_UPSIDE_DOWN {
            return (fm, tm);
        }
        let m = phys.mass;
        let hv = self.hover;
        let v = b.velocity;
        let w = b.angular_velocity;
        let top = def.max_forward_speed;
        if hv > 0.0 {
            let vmax = top * if self.brake { 0.8 } else { 1.0 };
            let mut a = Vec3::new(vmax * c.throttle[0] - v.dot(f), vmax * c.throttle[1] - v.dot(l), 0.0);
            let mut amax = def.speed_acceleration;
            if self.ground_ticks > 0 && sigma.abs() > SCOUT_STEER_SHARP {
                amax *= 1.0 - (0.05 * self.ground_ticks as f32).min(0.98);
            }
            if a.length() > amax {
                a = a.normalize() * amax;
            }
            fm += b.frame() * a * (m * hv);
            let wu = u.dot(w);
            let wd = scout_yaw_rate(sigma);
            let e = (wd - wu).clamp(-SCOUT_YAW_ACCEL, SCOUT_YAW_ACCEL);
            tm += u * (e * phys.zz_moment * hv);
        }
        if hv < 1.0 {
            let f2 = Vec3::new(f.x, f.y, 0.0).normalize_or_zero();
            let l2 = Vec3::new(l.x, l.y, 0.0).normalize_or_zero();
            let t = [c.throttle[0], c.throttle[1]];
            let comp = if u.z > 0.0 {
                let align = [u.dot(f2), u.dot(l2)];
                let rate = [w.dot(l2), -w.dot(f2)];
                let lev = [-align[0] - 15.0 * rate[0], -align[1] - 15.0 * rate[1]];
                [0, 1].map(|k| {
                    let s = if t[k] * lev[k] < 0.0 { -1.0 } else { 1.0 };
                    t[k] * (s * lev[k].abs() + 1.0).clamp(0.3, 2.5) * SCOUT_STICK_TORQUE
                        + (1.0 - u.z) * SCOUT_LEVEL_TORQUE * lev[k]
                })
            } else {
                [t[0] * SCOUT_STICK_TORQUE, t[1] * SCOUT_STICK_TORQUE]
            };
            tm += (l * (comp[0] * phys.yy_moment) - f * (comp[1] * phys.xx_moment)) * (1.0 - hv);
        }
        if self.brake {
            let k = (f.dot(v) / top).clamp(0.0, 1.0);
            if k > 0.0 {
                tm += l * (phys.yy_moment * k * hv * SCOUT_JUMP_PITCH);
                fm += Vec3::Z * (m * k * hv * SCOUT_JUMP_LIFT);
            }
            if self.airborne_ticks > 0 {
                let rho = (1.0 - self.airborne_ticks as f32 / 30.0).clamp(0.0, 1.0);
                let ahead = l.cross(Vec3::Z).normalize_or_zero();
                fm += (ahead * 0.002 + Vec3::Z * 0.001) * ((1.0 - hv) * m * rho);
            }
        }
        (fm * self.power, tm * self.power)
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
            let (mut f, mut t) = (self.collision_force, self.collision_torque);
            if def.kind == Kind::Scout {
                let (fm, tm) = self.scout(def, phys, c, self.sigma);
                f += fm;
                t += tm;
            }
            self.collision_force = Vec3::ZERO;
            self.collision_torque = Vec3::ZERO;
            self.last = physics::step(phys, &mut self.body, &channels, level, scales, f, t);
            if def.kind == Kind::Scout {
                // The hover fraction toward (antigraving channel points /
                // channel points) · max(up.z, 0.4).
                let points = phys.points.iter().filter(|p| p.channel.is_some()).count().max(1);
                let target =
                    (self.last.antigraving as f32 / points as f32 * self.body.up.z.max(0.4)).clamp(0.0, 1.0);
                self.hover += (target - self.hover).clamp(-HOVER_RATE, HOVER_RATE);
            }
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
