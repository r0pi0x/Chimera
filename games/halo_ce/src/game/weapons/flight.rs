//! A Halo projectile's flight, one update per 30 Hz tick
//! (specs/halo_ce/projectiles.md, behaviours 1 to 7 and 12, "Per-tick
//! order"): `Flight` is a projectile's state in world units and WU per
//! tick, in the engine's axes (+Y up: the spec's z is our y), and
//! `Flight::update` runs one update: the arming, deceleration and
//! detonation timers (the timer runs at once, or for "after first bounce"
//! and "when at rest" alike once it stopped after a collision, or while
//! stuck), then up to 10 segments: guidance (`Steer`: the target point
//! and the turn per segment, exactly that angle, never chasing a target
//! behind), the deceleration window (its timer's step lower / v0, the
//! disc's quirk), the final-speed floor (never for a projectile whose
//! initial and final velocities are equal: Quirks 11) and the slowed
//! bolt's disappearance, gravity, the range cut, a trace of the segment
//! through a callback (the level and characters: `super::systems`), and at
//! a hit gravity taken back for the unused part of the tick and the speed
//! clamp (only on a segment the deceleration slowed),
//! the direct-hit damage (a callback, which says which material took it:
//! the shield's when the shield took the larger share), and the material
//! response of behaviours 6 and 7 by the hit's material type
//! (`params::Responses`): the potential response under its angle, speed,
//! units-only and skip conditions, else the default; overpenetrate
//! (through an object at 1 - initial friction, the level as attach with a
//! timer, else detonate), reflect (the normal and sliding parts kept by
//! the two frictions), the noises, the minimum-velocity detonation, the
//! stop (speed squared below 0.0001; at rest on ground, n.y > 0.3, which
//! detonates a projectile without a timer), attach (stuck; flag 2 re-arms
//! the timer to the upper bound), disappear and detonate. A detonation
//! request waits for arming (and is never cancelled); a disappearance
//! ends the projectile at the update's end.
//!
//! There is no hitscan: a bullet traces its whole per-tick speed on the
//! tick it is fired (its first update runs on the firing tick) and goes on
//! from there on the following ticks until its range (projectiles.md 4).
//!
//! `wander` is the guidance's wander offset (the objects spec's wander
//! periodic function).
//!
//! Known shortcuts: no water (the level's media are not known to the
//! simulation: the water windows and gravity are never used); the
//! collision radius's two widened traces are the caller's (`trace` gets
//! the radius); the random spin (presentation) is not kept.

use std::f32::consts::FRAC_PI_2;

use bevy::math::{Quat, Vec3};

/// The callback that deals a direct hit (`Flight::update`).
pub type Struck<'a, T> = dyn FnMut(&Hit<T>, Vec3, f32) -> Option<u8> + 'a;

use super::{
    params::{GRAVITY, Kind, Projectile, projectile_flags},
    random::HaloRng,
};

/// Surfaces a projectile can meet in one update.
pub const MAX_COLLISIONS_PER_UPDATE: u32 = 10;
/// A speed squared below this after a response is "stopped".
pub const REST_SPEED_SQUARED: f32 = 0.0001;
/// A surface whose normal's up part is above this is ground.
pub const FLOOR_NORMAL_UP: f32 = 0.3;

/// What is pending for the update's end (ordered: a later request only
/// raises it).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Request {
    #[default]
    None,
    Detonate,
    Disappear,
}

/// A projectile in flight.
#[derive(Clone, Debug, PartialEq)]
pub struct Flight {
    /// World units, engine axes.
    pub position: Vec3,
    /// WU per tick.
    pub velocity: Vec3,
    pub travelled: f32,
    pub deceleration_timer: f32,
    pub deceleration_step: f32,
    pub deceleration: f32,
    pub arming: f32,
    pub arming_step: f32,
    pub timer: f32,
    pub timer_step: f32,
    /// The detonation timer has run (it then keeps running).
    pub timer_ran: bool,
    /// Stopped after a collision (starts timers of kinds 1 and 2).
    pub stopped: bool,
    /// Lying still on ground: no more movement unless pushed.
    pub at_rest: bool,
    /// Stuck (attached to an object, or fixed in the level).
    pub stuck: bool,
    pub request: Request,
    /// The material type of the last surface met (None: none yet or a
    /// material-less one).
    pub material: Option<u8>,
    /// Updates so far.
    pub updates: u32,
}

/// What a trace of a segment met: at fraction `t`, `point` (WU), the
/// surface normal, the object (None: the level), the material type (None:
/// a material-less surface) and whether the object is a unit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit<T> {
    pub t: f32,
    pub point: Vec3,
    pub normal: Vec3,
    pub object: Option<T>,
    pub material: Option<u8>,
    pub unit: bool,
}

impl<T> Hit<T> {
    /// A level hit on material type `material`.
    pub fn level(t: f32, point: Vec3, normal: Vec3, material: Option<u8>) -> Self {
        Hit { t, point, normal, object: None, material, unit: false }
    }
}

/// How a projectile ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// Its detonation (behaviour 9: its effect's area damage, a stuck
    /// one's attached damage).
    Detonate,
    Disappear,
}

/// One update's outcome: where each segment ended (for tracers and
/// impacts: from, to, the hit), how it ended, if it did, and the hit it
/// stuck at this update, if it did.
#[derive(Clone, Debug, PartialEq)]
pub struct Moved<T> {
    pub segments: Vec<(Vec3, Vec3, Option<Hit<T>>)>,
    pub end: Option<End>,
    pub stuck: Option<Hit<T>>,
}

/// Guidance for one update: the target point (WU) and the turn per
/// segment (radians).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Steer {
    pub target: Vec3,
    pub omega: f32,
}

impl Flight {
    /// Created at `position` along the unit `dir` with the inherited
    /// along-aim speed `inherited` (WU per tick; behaviour 1); `u` the
    /// uniform draw for the timer between its bounds.
    pub fn new(p: &Projectile, position: Vec3, dir: Vec3, inherited: f32) -> Self {
        Self::with_draw(p, position, dir, inherited, 0.0)
    }

    /// `new` with the timer's draw `u` (0 to 1).
    pub fn with_draw(p: &Projectile, position: Vec3, dir: Vec3, inherited: f32, u: f32) -> Self {
        let (timer, step) = p.deceleration_timer(p.air_damage_range);
        let [lo, hi] = p.timer;
        let t = if p.has(projectile_flags::DETONATION_MAX_TIME_IF_ATTACHED) { lo } else { lo + (hi - lo) * u };
        Flight {
            position,
            velocity: dir * (inherited + p.initial_velocity),
            travelled: 0.0,
            deceleration_timer: timer,
            deceleration_step: step,
            deceleration: p.deceleration(p.air_damage_range),
            arming: 0.0,
            arming_step: Projectile::step(p.arming_time),
            timer: 0.0,
            timer_step: if p.has_timer() { Projectile::step(t) } else { 0.0 },
            timer_ran: false,
            stopped: false,
            at_rest: false,
            stuck: false,
            request: Request::None,
            material: None,
            updates: 0,
        }
    }

    /// The speed, WU per tick.
    pub fn speed(&self) -> f32 {
        self.velocity.length()
    }

    /// Armed (or no arming).
    pub fn armed(&self) -> bool {
        self.arming_step == 0.0 || self.arming >= 1.0
    }

    fn ask(&mut self, r: Request) {
        self.request = self.request.max(r);
    }

    /// The timers of behaviour 2, at the update's start.
    fn timers(&mut self, p: &Projectile) {
        self.arming += self.arming_step;
        self.deceleration_timer += self.deceleration_step;
        let runs = self.timer_step > 0.0 && (p.timer_starts == 0 || self.stopped || self.timer_ran || self.stuck);
        if runs {
            self.timer_ran = true;
            self.timer += self.timer_step;
            if self.timer >= 1.0 {
                self.ask(Request::Detonate);
            }
        }
    }

    /// One update. `trace(from, to, ignored)` finds the nearest hit on the
    /// segment, leaving out the ignored object; `struck(hit, velocity at
    /// impact, scale)` deals the direct hit on an object and returns the
    /// material type that took it (None: no damage, the hit's own).
    pub fn update<T: Copy + PartialEq>(
        &mut self,
        p: &Projectile,
        ignored: &mut Option<T>,
        rng: &mut HaloRng,
        steer: Option<Steer>,
        trace: &mut dyn FnMut(Vec3, Vec3, Option<T>) -> Option<Hit<T>>,
        struck: &mut Struck<T>,
    ) -> Moved<T> {
        self.updates += 1;
        self.timers(p);
        let (vf, a) = (p.final_velocity, self.deceleration);
        let g = GRAVITY * p.air_gravity_scale;
        let mut out = Moved { segments: Vec::new(), end: None, stuck: None };
        let mut r = 1.0f32;
        let mut surfaces = 0;
        let moving = |f: &Flight| {
            (f.request == Request::None || (f.request == Request::Detonate && !f.armed())) && !f.stuck && !f.at_rest
        };
        while r > 0.0 && moving(self) {
            // v: the velocity at the segment's start; vt: turned by the
            // guidance (the new velocity's direction; the average keeps v).
            let v = self.velocity;
            let mut vt = v;
            // 1. Guidance.
            if let Some(s) = steer {
                let w = s.target - self.position;
                let axis = v.cross(w);
                if w.dot(v) > 0.0 && axis.length_squared() > 0.0 {
                    vt = Quat::from_axis_angle(axis.normalize(), s.omega) * v;
                }
            }
            let s = v.length();
            let mut v2 = vt;
            let mut avg_v = v;
            let mut avg_s = s;
            // The speed step 2 makes for the segment's end (s where it
            // makes none).
            let mut s_end = s;
            // 2. Deceleration.
            if self.deceleration_timer >= 1.0 {
                if s > vf && a != 0.0 {
                    let s2 = s - a * r;
                    s_end = s2;
                    if s2 > vf {
                        v2 = vt * (s2 / s);
                        avg_v = (v2 + v) / 2.0;
                        avg_s = s - a * r / 2.0;
                    } else {
                        let f = (s - vf) / (a * r);
                        let s2 = 0.99 * vf;
                        s_end = s2;
                        v2 = vt * (s2 / s);
                        avg_s = (s2 + s) * f / 2.0 + (1.0 - f) * vf;
                        avg_v = (v2 + v) * f / 2.0 + v2 * (1.0 - f);
                    }
                } else if p.maximum_range == 0.0
                    && p.timer[1] == 0.0
                    && p.minimum_velocity <= vf
                    && (a != 0.0 || self.travelled >= p.water_damage_range[1])
                {
                    // Gone where it is (no further move once slowed).
                    self.ask(Request::Disappear);
                    break;
                } else if s > 0.0 && s < vf && p.initial_velocity != vf {
                    // Never for a projectile whose initial and final
                    // velocities are equal (Quirks 11: its speed lands a
                    // rounding step under vf by chance).
                    s_end = 0.99 * vf;
                    v2 = vt * (s_end / s);
                }
            }
            // 3. Gravity (engine +Y up).
            v2.y -= g * r;
            avg_v.y -= g * r / 2.0;
            // 4. Range.
            let mut q = 1.0;
            if p.maximum_range != 0.0 && avg_s * r + self.travelled > p.maximum_range {
                q = if avg_s == 0.0 { 0.0 } else { (p.maximum_range - self.travelled) / (avg_s * r) };
                self.ask(Request::Detonate);
            }
            let from = self.position;
            let to = from + avg_v * (q * r);
            // 6. The trace.
            let hit = if surfaces >= MAX_COLLISIONS_PER_UPDATE || self.request == Request::Disappear {
                None
            } else {
                trace(from, to, *ignored)
            };
            let Some(hit) = hit else {
                if surfaces >= MAX_COLLISIONS_PER_UPDATE {
                    self.ask(Request::Detonate);
                }
                self.travelled += (avg_v * (q * r)).length();
                self.position = to;
                self.velocity = v2;
                out.segments.push((from, to, None));
                break;
            };
            surfaces += 1;
            // 7. What remains of the tick: gravity and deceleration taken
            // back.
            let rest = r * (1.0 - hit.t);
            v2.y += g * rest;
            // The speed clamp (behaviour 3 step 7): the deceleration taken
            // back for the unused part, never above the starting speed, on
            // a segment step 2 slowed (s' < s, s' its number, not the
            // length after gravity); elsewhere the factor is 1 and
            // gravity's gain in speed is kept (a grenade's bounce).
            if s_end < s && s_end != 0.0 {
                v2 *= (rest * a + s_end).min(s) / s_end;
            }
            r = rest;
            *ignored = None;
            self.travelled += (hit.point - from).length();
            self.position = hit.point;
            out.segments.push((from, hit.point, Some(hit)));
            // Behaviour 6: the direct hit, then the material.
            let mut material = hit.material;
            if hit.object.is_some()
                && let Some(m) = struck(&hit, v2, p.direct_hit_scale(v2.length()))
            {
                material = Some(m);
            }
            self.material = material;
            let e = p.responses.get(material);
            let n = hit.normal;
            let mut u = -v2.dot(n);
            let vn = e.velocity_noise.unwrap_or(0.0);
            if vn != 0.0 {
                u += rng.real_range(-vn, 0.0);
            }
            let an = e.angular_noise.unwrap_or(0.0);
            let mut alpha = if v2.length_squared() > 0.0 { v2.angle_between(n) - FRAC_PI_2 } else { FRAC_PI_2 };
            if an != 0.0 {
                alpha += rng.real_range(-an, an);
            }
            let potential = e.potential_applies(alpha, u, hit.unit) && e.skip.is_none_or(|k| k <= 0.0 || rng.real() >= k);
            let mut kind = if potential { e.potential } else { e.default };
            // Behaviour 7.
            if kind == Kind::Overpenetrate {
                if hit.object.is_some() {
                    v2 *= 1.0 - e.initial_friction.unwrap_or(0.0);
                    *ignored = hit.object;
                } else if p.has_timer() {
                    kind = Kind::Attach;
                    self.stopped = true;
                } else {
                    kind = Kind::Detonate;
                }
            }
            match kind {
                Kind::Reflect => {
                    let normal = n * v2.dot(n);
                    let along = v2 - normal;
                    v2 = along * (1.0 - e.perpendicular_friction.unwrap_or(0.0))
                        - normal * (1.0 - e.parallel_friction.unwrap_or(0.0));
                }
                Kind::Overpenetrate => {}
                _ => v2 = Vec3::ZERO,
            }
            if an != 0.0 && v2 != Vec3::ZERO {
                let axis = v2.any_orthonormal_vector();
                let spin = Quat::from_axis_angle(v2.normalize(), rng.real_range(0.0, std::f32::consts::TAU));
                v2 = Quat::from_axis_angle(spin * axis, rng.real_range(0.0, an)) * v2;
            }
            if vn != 0.0 && v2 != Vec3::ZERO {
                let len = (v2.length() + rng.real_range(-vn, vn)).max(0.0);
                v2 = v2.normalize() * len;
            }
            if kind != Kind::Attach && v2.length_squared() < p.minimum_velocity * p.minimum_velocity {
                self.ask(Request::Detonate);
            }
            if v2.length_squared() < REST_SPEED_SQUARED {
                self.stopped = true;
                if n.y > FLOOR_NORMAL_UP {
                    self.at_rest = true;
                    if !p.has_timer() {
                        self.ask(Request::Detonate);
                    }
                }
            }
            self.velocity = v2;
            match kind {
                Kind::Disappear => {
                    self.velocity = Vec3::ZERO;
                    self.ask(Request::Disappear);
                }
                Kind::Detonate => self.ask(Request::Detonate),
                Kind::Attach => {
                    self.velocity = Vec3::ZERO;
                    self.stuck = true;
                    self.at_rest = true;
                    if p.has(projectile_flags::DETONATION_MAX_TIME_IF_ATTACHED) {
                        self.timer_step = Projectile::step(p.timer[1]);
                    }
                    out.stuck = Some(hit);
                    break;
                }
                Kind::Reflect | Kind::Overpenetrate => {}
            }
        }
        out.end = match self.request {
            Request::Disappear => Some(End::Disappear),
            Request::Detonate if self.armed() => Some(End::Detonate),
            _ => None,
        };
        out
    }

    /// A push (behaviour 11): `dv` added (WU per tick), no longer at rest;
    /// a stuck projectile is not moved.
    pub fn push(&mut self, dv: Vec3) {
        if self.stuck {
            return;
        }
        self.velocity += dv;
        self.at_rest = false;
    }
}

/// The wander periodic function (objects.md "Periodic functions", type
/// 10): the table's 1024 bytes sampled at `time` (its 25.6 entries per
/// unit), interpolated, clamped to 0..1.
pub fn wander(time: f32) -> f32 {
    let table = wander_table();
    let u = time * 25.6;
    let i = u.floor();
    let f = u - i;
    let i = (i as i64).rem_euclid(1024) as usize;
    let a = table[i] as f32 / 255.0;
    let b = table[(i + 1) % 1024] as f32 / 255.0;
    ((1.0 - f) * a + f * b).clamp(0.0, 1.0)
}

/// The jitter and wander table (objects.md: 1024 samples at x = n x 28 /
/// 1024, normalised by their minimum and maximum, as bytes).
#[allow(clippy::excessive_precision, clippy::approx_constant)]
fn wander_table() -> [u8; 1024] {
    let s: Vec<f32> = (0..1024)
        .map(|n| {
            let x = n as f32 * 0.027343748;
            0.5 * ((25.132742 * x).cos() * (0.89759791 * x).cos() + (43.9823 * x).cos() * (1.5707964 * x).sin())
                + (3.1415927 * x).sin() * (6.2831855 * x).cos()
        })
        .collect();
    let (lo, hi) = s.iter().fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
    let mut out = [0u8; 1024];
    for (o, v) in out.iter_mut().zip(&s) {
        let v = if hi > lo { (v - lo) / (hi - lo) } else { *v };
        *o = (255.0 * v).clamp(0.0, 255.0) as u8;
    }
    out
}

/// The guidance's target point (behaviour 12 step 2): the target's point
/// moved by the wander offset, its length by the distance `e` between the
/// two centres (1 WU beyond 10, (e - 2) / 8 from 2 to 10, else 0), its
/// direction by the wander function at the game tick and a per-projectile
/// `k` (engine axes: +Y up).
pub fn wandered(point: Vec3, e: f32, tick: u64, k: u32) -> Vec3 {
    let len = if e > 10.0 {
        1.0
    } else if e > 2.0 {
        (e - 2.0) / 8.0
    } else {
        0.0
    };
    if len == 0.0 {
        return point;
    }
    let t1 = ((tick + 3 * k as u64) % 65536) as f32 / 90.0;
    let t2 = ((tick + 7 * k as u64) % 65536) as f32 / 90.0;
    let yaw = std::f32::consts::PI - wander(t1) * FRAC_PI_2;
    let pitch = wander(t2) * std::f32::consts::TAU;
    // The direction from angles in Halo's axes, then the engine's.
    let d = crate::game::maps::space::direction([yaw.cos() * pitch.cos(), yaw.sin() * pitch.cos(), pitch.sin()]);
    point + d * len
}
