//! A Halo projectile's flight, one update per 30 Hz tick
//! (specs/halo_ce/projectiles.md, behaviours 1 to 8, "Per-tick order"):
//! `Flight` is a projectile's state in world units and WU per tick, in the
//! engine's axes (+Y up: the spec's z is our y), and `Flight::update`
//! moves it by the spec's segments: the deceleration window (its timer's
//! step lower / v0, the disc's quirk), the final-speed floor and the
//! slowed bolt's disappearance, gravity, the range cut, a trace of the
//! segment through a callback (the level and characters: `super::systems`),
//! and at a hit gravity and deceleration taken back for the unused part
//! of the tick, the direct-hit damage (a callback, which says whether the
//! shield or the body took it) and the material response.
//!
//! There is no hitscan: a bullet traces its whole per-tick speed on the
//! tick it is fired (its first update runs on the firing tick) and goes on
//! from there on the following ticks until its range (projectiles.md 4),
//! so the table's travel delays at 10, 50 and 100 m come out as the spec
//! gives them.
//!
//! Known shortcuts: no detonation timer, arming, guidance, bounce,
//! attach or water (none of the four weapons' projectiles uses them on a
//! multiplayer map: their responses are disappear, and the assault rifle
//! bullet's pass through an unshielded body); the collision radius's two
//! widened traces are not made (all four have radius 0).

use bevy::math::Vec3;

use super::params::{GRAVITY, Projectile, Response};

/// Surfaces a projectile can meet in one update.
pub const MAX_COLLISIONS_PER_UPDATE: u32 = 10;

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
    /// Updates so far.
    pub updates: u32,
}

/// What a trace of a segment met: at fraction `t`, `point` (WU), the
/// surface normal, and the object (None: the level).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit<T> {
    pub t: f32,
    pub point: Vec3,
    pub normal: Vec3,
    pub object: Option<T>,
}

/// How a projectile ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// A detonation request (its range; the four have no detonation
    /// effect: they just vanish).
    Detonate,
    Disappear,
}

/// One update's outcome: where each segment ended (for tracers and
/// impacts: from, to, the hit) and how it ended, if it did.
#[derive(Clone, Debug, PartialEq)]
pub struct Moved<T> {
    pub segments: Vec<(Vec3, Vec3, Option<Hit<T>>)>,
    pub end: Option<End>,
}

impl Flight {
    /// Created at `position` along the unit `dir` with the inherited
    /// along-aim speed `inherited` (WU per tick; projectiles.md 1).
    pub fn new(p: &Projectile, position: Vec3, dir: Vec3, inherited: f32) -> Self {
        let (timer, step) = p.deceleration_timer(p.air_damage_range);
        Flight {
            position,
            velocity: dir * (inherited + p.initial_velocity),
            travelled: 0.0,
            deceleration_timer: timer,
            deceleration_step: step,
            deceleration: p.deceleration(p.air_damage_range),
            updates: 0,
        }
    }

    /// The speed, WU per tick.
    pub fn speed(&self) -> f32 {
        self.velocity.length()
    }

    /// One update. `trace(from, to, ignored)` finds the nearest hit on the
    /// segment, leaving out the ignored object; `struck(hit, velocity at
    /// impact, scale)` deals the direct hit on an object and returns the
    /// response for what took it (`Projectile::on_body`/`on_shield`).
    pub fn update<T: Copy + PartialEq>(
        &mut self,
        p: &Projectile,
        ignored: &mut Option<T>,
        trace: &mut dyn FnMut(Vec3, Vec3, Option<T>) -> Option<Hit<T>>,
        struck: &mut dyn FnMut(&Hit<T>, Vec3, f32) -> Response,
    ) -> Moved<T> {
        self.updates += 1;
        self.deceleration_timer += self.deceleration_step;
        let (vf, a) = (p.final_velocity, self.deceleration);
        let g = GRAVITY * p.air_gravity_scale;
        let mut out = Moved { segments: Vec::new(), end: None };
        let mut r = 1.0f32;
        let mut surfaces = 0;
        while r > 0.0 && out.end.is_none() {
            let v = self.velocity;
            let s = v.length();
            let mut v2 = v;
            let mut avg_v = v;
            let mut avg_s = s;
            if self.deceleration_timer >= 1.0 {
                if s > vf && a != 0.0 {
                    let s2 = s - a * r;
                    if s2 > vf {
                        v2 = v * (s2 / s);
                        avg_v = (v2 + v) / 2.0;
                        avg_s = s - a * r / 2.0;
                    } else {
                        let f = (s - vf) / (a * r);
                        let s2 = 0.99 * vf;
                        v2 = v * (s2 / s);
                        avg_s = (s2 + s) * f / 2.0 + (1.0 - f) * vf;
                        avg_v = (v2 + v) * f / 2.0 + v2 * (1.0 - f);
                    }
                } else if p.maximum_range == 0.0
                    && p.timer[1] == 0.0
                    && p.minimum_velocity <= vf
                    && (a != 0.0 || self.travelled >= p.water_damage_range[1])
                {
                    // Gone where it is (projectiles.md test (a): no
                    // further move once slowed).
                    out.end = Some(End::Disappear);
                    break;
                } else if s > 0.0 && s < vf {
                    v2 = v * (0.99 * vf / s);
                }
            }
            // Gravity (engine +Y up).
            v2.y -= g * r;
            avg_v.y -= g * r / 2.0;
            // Range.
            let mut q = 1.0;
            if p.maximum_range != 0.0 && avg_s * r + self.travelled > p.maximum_range {
                q = if avg_s == 0.0 { 0.0 } else { (p.maximum_range - self.travelled) / (avg_s * r) };
                out.end = Some(End::Detonate);
            }
            let from = self.position;
            let to = from + avg_v * (q * r);
            let hit = if surfaces >= MAX_COLLISIONS_PER_UPDATE { None } else { trace(from, to, *ignored) };
            let Some(hit) = hit else {
                if surfaces >= MAX_COLLISIONS_PER_UPDATE {
                    out.end = Some(End::Detonate);
                }
                self.travelled += (to - from).length();
                self.position = to;
                self.velocity = v2;
                out.segments.push((from, to, None));
                break;
            };
            surfaces += 1;
            // What remains of the tick: gravity and deceleration taken back.
            let rest = r * (1.0 - hit.t);
            v2.y += g * rest;
            let s2 = v2.length();
            if s2 != 0.0 {
                v2 *= (rest * a + s2).min(s) / s2;
            }
            r = rest;
            *ignored = None;
            let response = match hit.object {
                Some(_) => struck(&hit, v2, p.direct_hit_scale(v2.length())),
                None => p.on_level,
            };
            self.travelled += (hit.point - from).length();
            self.position = hit.point;
            out.segments.push((from, hit.point, Some(hit)));
            match response {
                Response::Disappear => {
                    self.velocity = Vec3::ZERO;
                    // A range request already pending is outranked.
                    out.end = Some(End::Disappear);
                }
                Response::Overpenetrate { initial_friction } => {
                    self.velocity = v2 * (1.0 - initial_friction);
                    *ignored = hit.object;
                }
            }
        }
        out
    }
}
