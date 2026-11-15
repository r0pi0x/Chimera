//! Halo's mass-point vehicle physics (specs/halo_ce/vehicles.md, "The
//! mass-point step"), implemented as the spec writes it: Halo's own
//! explicit integrator at its 30 Hz tick, in its own units (world units,
//! per tick, +z up), on the engine's collision through `Level`.
//!
//! **The design.** The spec's step is the original's whole vehicle
//! response: every touching sphere is a linear spring and damper with
//! linear friction, summed and integrated explicitly once per tick, with a
//! penetration freeze of the point centres instead of a solver. Its feel
//! (speed curves, the slide at full lock, the ride height and bounce) comes
//! from exactly these linear terms in per-tick units. So the port runs that
//! step itself (option (a) of the milestone): `step` computes each point's
//! forces from the engine's collision queries (`Level`: the deepest surface
//! within a point's sphere, a sweep of its centre, a ray for antigravity),
//! integrates once per Halo tick and returns the new pose; the ECS side
//! (`super`) writes that pose to a kinematic avian body the engine's other
//! things (characters, shots, other vehicles' queries) collide with. An
//! avian dynamic body driven by forces would re-integrate the same forces
//! with avian's solver and substeps, adding contact impulses and friction
//! of its own that the spec's numbers do not have; matching the spec's
//! simulated cases tick for tick needs the spec's integrator. "The engine's
//! physics" is kept where it matters: avian's colliders and spatial
//! queries are the level, and the vehicle is an avian body for everything
//! else.
//!
//! Frame: a vehicle's position is its centre of mass p; a model-frame point
//! q is at p + R (q − c) with R = [f, l, u] (`Body::frame`).
//!
//! Known shortcuts:
//! - Water (buoyancy, water friction and lift), air lift and thrust are not
//!   modelled: no multiplayer vehicle has a water, lift or thrust channel,
//!   and the multiplayer maps' water is shallow (vehicles.md, Edge cases).
//! - The older model (physics radius > 0) is not modelled (campaign only).
//! - The ground query's surfaces are the level's solid colliders but
//!   vehicles and characters (`level::EngineLevel`); a touched object's
//!   shield depletion and "volatile" flag are not modelled (no machines on
//!   the multiplayer maps).
//!
//! `step` follows the spec's "Order of one step": forces of every point
//! from the start state, the sum (gravity, points, last tick's collision
//! accumulators, the type's magic force and torque), one integration, the
//! penetration freeze (`freeze_response`; a tick whose last pass still
//! hits does not move the vehicle, `StepOut::frozen`), the sleep test.

use bevy::math::{Mat3, Quat, Vec3};

/// The engine's gravity, WU per tick per tick (`movement.md`).
pub const GRAVITY: f32 = 0.003_565_179_2;
/// A mass point is at rest below this squared speed.
pub const POINT_REST_SPEED2: f32 = 0.001_111_111_1;
/// Sleep test thresholds (vehicles.md, Constants).
pub const REST_LINEAR_V2: f32 = 0.001_111_111_1;
pub const REST_ANGULAR_V2: f32 = 0.002_741_556_8;
pub const REST_LINEAR_A2: f32 = 3.086_419_7e-7;
pub const REST_ANGULAR_A2: f32 = 3.046_174_2e-6;
pub const REST_MIN_GROUND_POINTS: usize = 3;
/// Penetration sweeps per tick and the backoff along the hit normal.
pub const FREEZE_PASSES: usize = 4;
pub const FREEZE_EPSILON: f32 = 0.007_812_5;
/// A material's friction scale applies only up to this mass.
pub const MATERIAL_MASS_LIMIT: f32 = 7500.0;

/// How a point's friction is shaped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Friction {
    Point,
    Forward,
    Left,
    Up,
}

/// A mass point as the step reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct PointDef {
    pub position: Vec3,
    pub forward: Vec3,
    pub up: Vec3,
    pub mass: f32,
    pub radius: f32,
    pub channel: Option<usize>,
    pub friction: Friction,
    pub along: f32,
    pub across: f32,
}

/// A powered mass point (channel) as the step reads it.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ChannelDef {
    pub ground_friction: bool,
    pub antigrav: bool,
    pub antigrav_strength: f32,
    pub antigrav_height: f32,
    pub antigrav_damp: f32,
    pub antigrav_k1: f32,
    pub antigrav_k0: f32,
}

/// The physics tag as the step reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct PhysicsDef {
    pub mass: f32,
    pub center_of_mass: Vec3,
    pub gravity_scale: f32,
    pub ground_friction: f32,
    pub ground_depth: f32,
    pub ground_damp: f32,
    pub ground_k1: f32,
    pub ground_k0: f32,
    pub air_friction: f32,
    /// The stored tensor's diagonal (the scout's torque scales).
    pub xx_moment: f32,
    pub yy_moment: f32,
    pub zz_moment: f32,
    /// Model frame.
    pub inverse_inertia: Mat3,
    pub channels: Vec<ChannelDef>,
    pub points: Vec<PointDef>,
}

/// What a channel is told this tick by the vehicle's type.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChannelState {
    /// The ground friction velocity (the commanded wheel speed), WU/t.
    pub ground_speed: f32,
    /// The channel's rotation: a yaw about the model's z axis, radians.
    pub yaw: f32,
    /// The antigravity fraction.
    pub antigrav: f32,
}

/// A material's vehicle terrain scales (globals.md; 0: none).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GroundScales {
    pub friction: f32,
    pub k1: f32,
    pub k0: f32,
    pub depth: f32,
    pub damp: f32,
}

/// The deepest surface within a point's sphere.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    /// The surface's normal, pointing out of it toward the point.
    pub normal: Vec3,
    /// Sphere radius minus the distance from the centre to the surface.
    pub depth: f32,
    /// The surface's material type (collision.md), if known.
    pub material: Option<u8>,
}

/// The level as a vehicle's points see it, in world units, +z up.
pub trait Level {
    /// The deepest surface within `radius` of `center`, if any.
    fn sphere(&self, center: Vec3, radius: f32) -> Option<Contact>;
    /// The first front-facing surface the segment `from` to `to` crosses:
    /// the fraction along it and the surface's normal.
    fn sweep(&self, from: Vec3, to: Vec3) -> Option<(f32, Vec3)>;
}

/// A flat infinite floor at height `z` of material `material` (the spec's
/// test setup).
#[derive(Clone, Copy, Debug)]
pub struct FlatLevel {
    pub z: f32,
    pub material: Option<u8>,
}

impl Level for FlatLevel {
    fn sphere(&self, center: Vec3, radius: f32) -> Option<Contact> {
        let d = center.z - self.z;
        (d < radius).then_some(Contact {
            normal: Vec3::Z,
            depth: radius - d,
            material: self.material,
        })
    }

    fn sweep(&self, from: Vec3, to: Vec3) -> Option<(f32, Vec3)> {
        let (a, b) = (from.z - self.z, to.z - self.z);
        (a >= 0.0 && b < 0.0).then(|| (a / (a - b), Vec3::Z))
    }
}

/// A vehicle's rigid body state, world units per tick, +z up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    /// The centre of mass.
    pub position: Vec3,
    pub forward: Vec3,
    pub up: Vec3,
    pub velocity: Vec3,
    pub angular_velocity: Vec3,
}

impl Body {
    /// A body at rest with its centre of mass at `position`, facing
    /// `heading` radians about +z.
    pub fn at(position: Vec3, heading: f32) -> Body {
        Body {
            position,
            forward: Vec3::new(heading.cos(), heading.sin(), 0.0),
            up: Vec3::Z,
            velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
        }
    }

    /// The vehicle's left, u × f.
    pub fn left(&self) -> Vec3 {
        self.up.cross(self.forward)
    }

    /// R = [f, l, u]: model frame to world.
    pub fn frame(&self) -> Mat3 {
        Mat3::from_cols(self.forward, self.left(), self.up)
    }

    /// The rotation taking the model frame to the world.
    pub fn rotation(&self) -> Quat {
        Quat::from_mat3(&self.frame()).normalize()
    }

    /// Where a model-frame point is, with the centre of mass `c`.
    pub fn point(&self, q: Vec3, c: Vec3) -> Vec3 {
        self.position + self.frame() * (q - c)
    }

    /// The model origin's world position.
    pub fn origin(&self, c: Vec3) -> Vec3 {
        self.point(Vec3::ZERO, c)
    }

    /// The orientation turned by the rotation vector `w` (angle |w| about
    /// w/|w|): forward renormalised, up made perpendicular and
    /// renormalised.
    pub fn turned(&self, w: Vec3) -> (Vec3, Vec3) {
        let angle = w.length();
        let (f, u) = if angle > 0.0 {
            let q = Quat::from_axis_angle(w / angle, angle);
            (q * self.forward, q * self.up)
        } else {
            (self.forward, self.up)
        };
        let f = f.normalize();
        let u = (u - f * u.dot(f)).normalize();
        (f, u)
    }
}

/// What a step found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StepOut {
    /// Points touching the ground.
    pub touching: usize,
    /// Each point's touch (depth, material) this tick, in tag order.
    pub contacts: Vec<Option<Contact>>,
    /// Points the penetration freeze stopped.
    pub stuck: Vec<usize>,
    /// The sleep test passed.
    pub at_rest: bool,
    /// The linear and angular accelerations (per tick per tick).
    pub acceleration: Vec3,
    pub angular_acceleration: Vec3,
    /// Points antigraving.
    pub antigraving: usize,
    /// Every freeze pass hit: the vehicle did not move this tick.
    pub frozen: bool,
}

/// 0 at or below `k0`, 1 at or above `k1`, linear between.
pub fn pin(x: f32, k0: f32, k1: f32) -> f32 {
    if x <= k0 {
        0.0
    } else if x >= k1 {
        1.0
    } else {
        (x - k0) / (k1 - k0)
    }
}

/// Scale `v`'s part along `axis` by `along` and the rest by `across`.
fn shape(v: Vec3, axis: Vec3, along: f32, across: f32) -> Vec3 {
    let a = axis * v.dot(axis);
    a * along + (v - a) * across
}

fn shaped(p: &PointDef, v: Vec3, f: Vec3, u: Vec3) -> Vec3 {
    match p.friction {
        Friction::Point => v,
        Friction::Forward => shape(v, f, p.along, p.across),
        Friction::Left => shape(v, u.cross(f).normalize_or_zero(), p.along, p.across),
        Friction::Up => shape(v, u, p.along, p.across),
    }
}

/// One tick of the mass-point step on `body`: the forces of every point
/// against `level` (with each material's `scales`, by type), the extra
/// "magic" force and torque the type adds and the collision accumulators,
/// integration, and the penetration freeze. Returns the points' flags.
pub fn step(
    def: &PhysicsDef,
    body: &mut Body,
    channels: &[ChannelState],
    level: &dyn Level,
    scales: &[GroundScales],
    extra_force: Vec3,
    extra_torque: Vec3,
) -> StepOut {
    let m_total = def.mass;
    let c = def.center_of_mass;
    let frame = body.frame();
    let g_v = GRAVITY * def.gravity_scale;
    let mut out = StepOut {
        contacts: Vec::with_capacity(def.points.len()),
        ..default_out()
    };
    let mut force = Vec3::new(0.0, 0.0, -m_total * g_v) + extra_force;
    let mut torque = extra_torque;
    let mut all_rest = true;
    let starts: Vec<Vec3> = def.points.iter().map(|p| body.point(p.position, c)).collect();
    for (i, p) in def.points.iter().enumerate() {
        let ch = p
            .channel
            .and_then(|k| Some((def.channels.get(k)?, channels.get(k).copied().unwrap_or_default())));
        let turn = ch.map_or(Quat::IDENTITY, |(_, s)| Quat::from_rotation_z(s.yaw));
        let x = starts[i];
        let f_i = frame * (turn * p.forward);
        let u_i = frame * (turn * p.up);
        let r = x - body.position;
        let v_i = body.velocity + body.angular_velocity.cross(r);
        if v_i.length_squared() >= POINT_REST_SPEED2 {
            all_rest = false;
        }
        let mut f_point = Vec3::ZERO;
        let contact = level.sphere(x, p.radius).filter(|k| k.depth > 0.0);
        if let Some(k) = contact {
            out.touching += 1;
            let s = k
                .material
                .and_then(|m| scales.get(m as usize))
                .copied()
                .unwrap_or_default();
            let scaled = |v: f32, k: f32| if k > 0.0 { v * k } else { v };
            let mu = if m_total <= MATERIAL_MASS_LIMIT {
                scaled(def.ground_friction, s.friction)
            } else {
                def.ground_friction
            };
            let k1 = scaled(def.ground_k1, s.k1);
            let k0 = scaled(def.ground_k0, s.k0);
            let depth = scaled(def.ground_depth, s.depth);
            let damp = scaled(def.ground_damp, s.damp);
            let n = k.normal;
            f_point += m_total * (GRAVITY * k.depth / depth - v_i.dot(n) * damp) * n;
            let w = v_i - n * v_i.dot(n);
            let mut fg = -p.mass * mu * w;
            if let Some((cd, cs)) = ch
                && cd.ground_friction
                && cs.ground_speed != 0.0
            {
                let phi = pin(n.z, k0, k1);
                let alpha = u_i.dot(n).clamp(0.0, 1.0);
                let e = -cs.ground_speed * f_i;
                let e = e - n * e.dot(n);
                fg -= p.mass * mu * alpha * alpha * phi * phi * e;
            }
            f_point += shaped(p, fg, f_i, u_i);
        }
        out.contacts.push(contact);
        // Air friction (no channel of the four has an air speed).
        f_point += shaped(p, -p.mass * def.air_friction * v_i, f_i, u_i);
        // Antigravity.
        if let Some((cd, cs)) = ch
            && cd.antigrav
            && cd.antigrav_height > 0.0
        {
            let len = p.radius + cd.antigrav_height;
            if let Some((t, n_h)) = level.sweep(x, x - Vec3::Z * len) {
                let h = len * t - p.radius;
                let gamma = if h > 0.0 { 1.0 - h / cd.antigrav_height } else { 1.0 };
                let beta = pin(u_i.z, cd.antigrav_k0, cd.antigrav_k1);
                f_point += m_total
                    * cd.antigrav_strength
                    * beta
                    * cs.antigrav
                    * (gamma * gamma * GRAVITY - n_h.dot(v_i) * cd.antigrav_damp)
                    * n_h;
                out.antigraving += 1;
            }
        }
        force += f_point;
        torque += r.cross(f_point);
    }

    // Step 4: integrate once (semi-implicit Euler).
    let a = force / m_total;
    let alpha = frame * def.inverse_inertia * frame.transpose() * torque;
    let start = *body;
    let mut v = body.velocity + a;
    let mut w = body.angular_velocity + alpha;
    let mut pose = {
        let (f, u) = start.turned(w);
        (start.position + v, f, u)
    };
    // Step 5: the penetration freeze. Sweep the centres; on a front-facing
    // hit scale the velocity's normal part and all of the angular velocity
    // by t' and sweep again; if the last pass still hits, nothing moves.
    let mut hit_last = false;
    for _ in 0..FREEZE_PASSES {
        let moved = Body {
            position: pose.0,
            forward: pose.1,
            up: pose.2,
            ..start
        };
        let mut first: Option<(f32, Vec3, Vec3, usize)> = None;
        for (i, p) in def.points.iter().enumerate() {
            let to = moved.point(p.position, c);
            if let Some((t, n)) = level.sweep(starts[i], to)
                && first.is_none_or(|(ft, ..)| t < ft)
            {
                first = Some((t, n, to - starts[i], i));
            }
        }
        let Some((t_h, n_h, delta, i)) = first else {
            hit_last = false;
            break;
        };
        hit_last = true;
        if !out.stuck.contains(&i) {
            out.stuck.push(i);
        }
        (v, w) = freeze_response(t_h, n_h, delta, v, w);
        let (f, u) = start.turned(w);
        pose = (start.position + v, f, u);
    }
    if hit_last {
        out.frozen = true;
        pose = (start.position, start.forward, start.up);
    }
    body.position = pose.0;
    body.forward = pose.1;
    body.up = pose.2;
    body.velocity = v;
    body.angular_velocity = w;
    out.acceleration = a;
    out.angular_acceleration = alpha;
    // Step 6: the sleep test, with step 4's accelerations and the
    // velocities the freeze left.
    out.at_rest = all_rest
        && out.touching >= REST_MIN_GROUND_POINTS
        && v.length_squared() <= REST_LINEAR_V2
        && w.length_squared() <= REST_ANGULAR_V2
        && a.length_squared() <= REST_LINEAR_A2
        && alpha.length_squared() <= REST_ANGULAR_A2;
    out
}

/// One freeze pass's response to the earliest hit (fraction `t_h` along
/// the sweep `delta`, normal `n_h`): t' = max(t_h − ε, 0) with ε =
/// 0.0078125/|Δ·n| (0.03125 when Δ·n = 0); a velocity moving into the
/// surface has its normal part scaled by t' (the tangential part kept),
/// the angular velocity is scaled by t' whole.
pub fn freeze_response(t_h: f32, n_h: Vec3, delta: Vec3, v: Vec3, w: Vec3) -> (Vec3, Vec3) {
    let dn = delta.dot(n_h).abs();
    let eps = if dn > 0.0 { FREEZE_EPSILON / dn } else { 0.031_25 };
    let t = (t_h - eps).max(0.0);
    let v = if n_h.dot(v) < 0.0 {
        v + n_h * (t - 1.0) * n_h.dot(v)
    } else {
        v
    };
    (v, w * t)
}

/// Vehicle-vehicle contact spring depth (vehicles.md, Constants).
pub const COLLISION_DEPTH: f32 = 0.2;

/// The pair rule of two vehicles' mass points (vehicles.md "Other
/// vehicles"): for every pair of spheres closer than their summed radii,
/// a spring of 2 √(M_a M_b) g / 0.2 per unit of half the overlap pushes
/// them apart, applied π_p short of A's sphere surface. Returns the force
/// and torque (about each centre of mass) on A and on B, or `None` when no
/// pair touches.
pub fn pair_contact(a: &Body, pa: &PhysicsDef, b: &Body, pb: &PhysicsDef) -> Option<[(Vec3, Vec3); 2]> {
    let k = 2.0 * (pa.mass * pb.mass).sqrt() * GRAVITY / COLLISION_DEPTH;
    let mut out = [(Vec3::ZERO, Vec3::ZERO); 2];
    let mut any = false;
    for qa in &pa.points {
        let xa = a.point(qa.position, pa.center_of_mass);
        for qb in &pb.points {
            let xb = b.point(qb.position, pb.center_of_mass);
            let d = (xb - xa).length();
            let reach = qa.radius + qb.radius;
            if d >= reach || d <= 0.0 {
                continue;
            }
            any = true;
            let dir = (xb - xa) / d;
            let pen = (reach - d) / 2.0;
            let at = xa + dir * (qa.radius - pen);
            let f = dir * (k * pen);
            out[0].0 -= f;
            out[0].1 += (at - a.position).cross(-f);
            out[1].0 += f;
            out[1].1 += (at - b.position).cross(f);
        }
    }
    any.then_some(out)
}

fn default_out() -> StepOut {
    StepOut::default()
}
