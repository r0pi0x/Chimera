//! On the ground: friction, acceleration, the walk move, the slide move with
//! its clip planes, stair steps, staying on the ground, the ground test
//! (`categorize`) and the ground material's factors.

use bevy::prelude::*;

use crate::source_movement::{
    mover::{
        BUMPS, GROUND_PROBE, MAX_CLIP_PLANES, Mover, SNAP_MIN, STAY_ON_GROUND_UP, STEP_EPSILON, UPWARD_AIR_FRICTION,
        WALKABLE_NORMAL_Z,
    },
    trace::HitSurface,
    units::{METERS_PER_UNIT, to_engine},
};

/// out = in - n (in . n) b, then remove any remaining push into the plane.
#[inline]
pub(crate) fn clip_velocity(v: Vec3, n: Vec3, overbounce: f32) -> Vec3 {
    let mut out = v - n * v.dot(n) * overbounce;
    let into = out.dot(n);
    if into < 0.0 {
        out -= n * into;
    }
    out
}

impl Mover<'_, '_, '_, '_> {
    /// Ground changes: leaving a moving solid adds its velocity to the
    /// base velocity, landing on one takes it off (doors_buttons.md,
    /// "Riding").
    pub(crate) fn set_ground(&mut self, on: bool, owner: Option<Entity>) {
        let (was_on, was) = (self.me.on_ground, self.me.ground_entity);
        if !was_on && on && let Some(e) = owner {
            let mv = (self.mover_velocity)(e);
            self.base.x -= mv.x;
            self.base.y -= mv.y;
            self.base.z = mv.z;
        } else if was_on && !on && let Some(e) = was {
            let mv = (self.mover_velocity)(e);
            self.base.x += mv.x;
            self.base.y += mv.y;
            self.base.z = mv.z;
        }
        self.me.on_ground = on;
        self.me.ground_entity = if on { owner } else { None };
        if !on {
            self.ground_collider = None;
        }
    }

    pub(crate) fn friction(&mut self) {
        let speed = self.v.length();
        if speed < 0.1 {
            return;
        }
        let control = speed.max(self.cfg.stopspeed);
        // A push that took the footing (`core::ExternalPush`): sliding,
        // a share of the friction.
        let share = self.push.friction_share(self.v.truncate().length() * METERS_PER_UNIT);
        let drop = control * self.cfg.friction * self.me.surface_friction * share * self.dt;
        let new_speed = (speed - drop).max(0.0);
        self.v *= new_speed / speed;
    }

    #[inline]
    pub(crate) fn accelerate(&mut self, dir: Vec3, wish_speed: f32, accel: f32) {
        let add = wish_speed - self.v.dot(dir);
        if add <= 0.0 {
            return;
        }
        let amount = (accel * self.dt * wish_speed * self.me.surface_friction).min(add);
        self.v += dir * amount;
    }

    /// Collide and slide for the rest of the tick.
    pub(crate) fn slide(&mut self) {
        let before = self.v.truncate().length();
        self.slide_move();
        // Wall slam: a big loss of horizontal speed in one move.
        let loss = before - self.v.truncate().length();
        if loss > 1160.0 {
            self.impact_step(1.0);
        } else if loss > 580.0 {
            self.impact_step(0.85);
        }
    }

    pub(crate) fn slide_move(&mut self) {
        // `CHIMERA_SLIDE_DEBUG=1`: every sweep printed (docs/OBSERVABILITY.md,
        // the surf rides).
        fn dbg_slide() -> bool {
            static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
            *ON.get_or_init(|| chimera_core::env::var("CHIMERA_SLIDE_DEBUG").is_ok())
        }
        let primal = self.v;
        let mut original = self.v;
        let mut planes: Vec<Vec3> = Vec::with_capacity(MAX_CLIP_PLANES);
        let mut time_left = self.dt;
        let mut all_fraction = 0.0;
        for _ in 0..BUMPS {
            if self.v.length_squared() == 0.0 {
                break;
            }
            let end = self.feet + self.v * time_left;
            let tr = self.trace.sweep_by(self.me.ducked, self.feet, self.v * time_left);
            all_fraction += tr.fraction;
            if dbg_slide() {
                let hit = match tr.surface {
                    Some(HitSurface::Collider(e)) => format!("collider {e}"),
                    Some(HitSurface::Brush(i)) => format!("brush {i}"),
                    None => "-".to_string(),
                };
                eprintln!(
                    "    slide: feet {:.3} v {:.2} frac {:.5} start_solid {} normal {:.4} ({hit}) planes {planes:?} \
                     ground {}",
                    self.feet, self.v, tr.fraction, tr.start_solid, tr.normal, self.me.on_ground
                );
            }
            if tr.start_solid && tr.fraction == 0.0 && self.trace.solid(self.me.ducked, end) {
                self.v = Vec3::ZERO;
                return;
            }
            if tr.fraction > 0.0 {
                // A full sweep is re-tested at its end (it can end inside
                // terrain); partial ones aren't.
                if tr.fraction >= 1.0 && self.trace.solid(self.me.ducked, tr.end) {
                    if dbg_slide() {
                        let (lo, hi) = self.trace.hull(self.me.ducked);
                        let full = hi - lo;
                        let brush = self.trace.in_brush(
                            Vec3::new(full.x, full.z, full.y) * METERS_PER_UNIT / 2.0,
                            to_engine(tr.end + (lo + hi) / 2.0),
                        );
                        eprintln!("    slide: end {:.3} solid (brush {brush})", tr.end);
                    }
                    self.v = Vec3::ZERO;
                    break;
                }
                self.feet = tr.end;
                original = self.v;
                planes.clear();
            }
            if !tr.hit() {
                break;
            }
            if let Some(HitSurface::Collider(e)) = tr.surface
                && !self.touched.contains(&e)
            {
                self.touched.push(e);
            }
            time_left -= time_left * tr.fraction;
            if planes.len() >= MAX_CLIP_PLANES {
                self.v = Vec3::ZERO;
                break;
            }
            planes.push(tr.normal);
            if planes.len() == 1 && !self.me.on_ground && self.me.ladder.is_none() {
                let overbounce = if tr.normal.z > WALKABLE_NORMAL_Z {
                    1.0
                } else {
                    1.0 + self.cfg.bounce * (1.0 - self.me.surface_friction)
                };
                self.v = clip_velocity(original, tr.normal, overbounce) + tr.normal * self.cfg.clip_push_out;
            } else {
                let found = planes.iter().enumerate().find_map(|(i, n)| {
                    let clipped = clip_velocity(original, *n, 1.0);
                    planes
                        .iter()
                        .enumerate()
                        .all(|(j, m)| j == i || clipped.dot(*m) >= 0.0)
                        .then_some(clipped)
                });
                match found {
                    Some(v) => self.v = v + tr.normal * self.cfg.clip_push_out,
                    None if planes.len() == 2 => {
                        let dir = planes[0].cross(planes[1]).normalize_or_zero();
                        self.v = dir * dir.dot(self.v);
                    }
                    None => {
                        self.v = Vec3::ZERO;
                        break;
                    }
                }
                if self.v.dot(primal) <= 0.0 {
                    self.v = Vec3::ZERO;
                    break;
                }
            }
        }
        if all_fraction == 0.0 {
            self.v = Vec3::ZERO;
        }
    }

    /// Try the move flat and stepped up; keep whichever goes farther.
    pub(crate) fn step_move(&mut self) {
        let (p0, v0) = (self.feet, self.v);
        self.slide();
        let (down_pos, down_vel) = (self.feet, self.v);

        self.feet = p0;
        self.v = v0;
        let rise = self.cfg.stepsize + STEP_EPSILON;
        let up = self.trace.sweep(self.me.ducked, p0, p0 + Vec3::Z * rise);
        if !up.start_solid {
            self.feet = up.end;
        }
        self.slide();
        let down = self.trace.sweep(self.me.ducked, self.feet, self.feet - Vec3::Z * rise);
        if !down.hit() || down.normal.z < WALKABLE_NORMAL_Z {
            self.feet = down_pos;
            self.v = down_vel;
            return;
        }
        if !down.start_solid {
            self.feet = down.end;
        }
        let flat = (down_pos - p0).truncate().length_squared();
        let stepped = (self.feet - p0).truncate().length_squared();
        if flat > stepped {
            self.feet = down_pos;
            self.v = down_vel;
        } else {
            self.v.z = down_vel.z;
        }
    }

    pub(crate) fn stay_on_ground(&mut self) {
        let up = self
            .trace
            .sweep(self.me.ducked, self.feet, self.feet + Vec3::Z * STAY_ON_GROUND_UP);
        let start = up.end;
        let end = self.feet - Vec3::Z * self.cfg.stepsize;
        let tr = self.trace.sweep(self.me.ducked, start, end);
        if tr.fraction > 0.0
            && tr.fraction < 1.0
            && !tr.start_solid
            && tr.normal.z >= WALKABLE_NORMAL_Z
            && (self.feet.z - tr.end.z).abs() > SNAP_MIN
        {
            self.feet = tr.end;
        }
    }

    #[inline]
    pub(crate) fn wish(&self, f: f32, s: f32, forward: Vec3, right: Vec3, max_speed: f32) -> (Vec3, f32) {
        let mut wish = forward * f + right * s;
        wish.z = 0.0;
        let speed = wish.length();
        if speed < 1e-6 {
            return (Vec3::ZERO, 0.0);
        }
        (wish / speed, speed.min(max_speed))
    }

    pub(crate) fn walk(&mut self, f: f32, s: f32, forward: Vec3, right: Vec3, max_speed: f32) {
        let (dir, speed) = self.wish(f, s, forward, right, max_speed);
        self.wish = dir * speed;
        if self.me.stamina > 0.0 {
            let r = 1.0 - self.cfg.stamina_ground_scale * self.me.stamina;
            self.v.x *= r;
            self.v.y *= r;
        }
        self.v.z = 0.0;
        // TF2: a slow wish beats friction (raised acceleration).
        let f = self.cfg.friction * self.me.surface_friction;
        let accel = if self.cfg.low_wish_accel && speed > 0.0 && speed < 100.0 * f / self.cfg.accelerate {
            self.v.length().max(100.0) * f / speed + 1.0
        } else {
            self.cfg.accelerate
        };
        self.accelerate(dir, speed, accel);
        self.v.z = 0.0;
        if self.cfg.ground_clamp {
            self.clamp_flat(max_speed);
        }
        self.back_clamp(forward, max_speed);
        // Base velocity moves the player but is not kept (trigger_push).
        let base = Vec3::new(self.base.x, self.base.y, 0.0);
        self.v += base;
        if self.v.length() < 1.0 {
            self.v = Vec3::ZERO;
            return;
        }
        let dest = self.feet + Vec3::new(self.v.x, self.v.y, 0.0) * self.dt;
        let tr = self.trace.sweep(self.me.ducked, self.feet, dest);
        if !tr.hit() {
            self.feet = tr.end;
        } else {
            self.step_move();
        }
        self.stay_on_ground();
        self.v -= base;
    }

    /// The horizontal speed down to `max` (TF2's ground clamp).
    #[inline]
    pub(crate) fn clamp_flat(&mut self, max: f32) {
        let flat = self.v.truncate().length();
        if flat > max && flat > 0.0 {
            self.v.x *= max / flat;
            self.v.y *= max / flat;
        }
    }

    /// TF2's backpedal clamp (`SourceMovementConfig::back_speed`): on the
    /// ground, a velocity faster than `back_min_speed` pointing behind the
    /// view has its backward part capped at `back_speed` x `max`, then the
    /// whole clamped to `max`.
    pub(crate) fn back_clamp(&mut self, forward: Vec3, max: f32) {
        if self.cfg.back_speed >= 1.0 {
            return;
        }
        let flat = Vec3::new(self.v.x, self.v.y, 0.0);
        let along = flat.dot(forward);
        if flat.length() <= self.cfg.back_min_speed || along >= 0.0 {
            return;
        }
        let cap = self.cfg.back_speed * max;
        if -along > cap {
            let side = flat - forward * along;
            let v = side - forward * cap;
            self.v.x = v.x;
            self.v.y = v.y;
            self.clamp_flat(max);
        }
    }

    /// Ground detection: a 2-unit drop test with the full box, then with
    /// each quarter of it, so edges and ridges still count as ground.
    pub(crate) fn categorize(&mut self) {
        self.me.surface_friction = 1.0;
        // Moving up on a ladder always counts as airborne.
        // Without footing (`core::ExternalPush::no_footing`) any speed
        // away from the ground keeps the body in the air.
        let footless = self.push.no_footing > 0.0 && self.v.z > 0.0;
        if self.v.z > self.cfg.leave_ground_vz || (self.me.ladder.is_some() && self.v.z > 0.0) || footless {
            self.set_ground(false, None);
        } else {
            let (lo, hi) = self.trace.hull(self.me.ducked);
            let down = self.feet - Vec3::Z * GROUND_PROBE;
            let full = self.trace.sweep_box(lo, hi, self.feet, down);
            let mut ground = (full.hit() && full.normal.z >= WALKABLE_NORMAL_Z).then_some(full);
            if ground.is_none() {
                let mid = Vec3::ZERO;
                let quarters = [
                    (Vec3::new(lo.x, lo.y, lo.z), Vec3::new(mid.x, mid.y, hi.z)),
                    (Vec3::new(mid.x, lo.y, lo.z), Vec3::new(hi.x, mid.y, hi.z)),
                    (Vec3::new(lo.x, mid.y, lo.z), Vec3::new(mid.x, hi.y, hi.z)),
                    (Vec3::new(mid.x, mid.y, lo.z), Vec3::new(hi.x, hi.y, hi.z)),
                ];
                ground = quarters.iter().find_map(|(a, b)| {
                    let tr = self.trace.sweep_box(*a, *b, self.feet, down);
                    (tr.hit() && tr.normal.z >= WALKABLE_NORMAL_Z).then_some(tr)
                });
            }
            // `CHIMERA_GROUND_DEBUG=1`: every ground test printed
            // (docs/OBSERVABILITY.md, movecmp).
            static DEBUG: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
            if *DEBUG.get_or_init(|| chimera_core::env::var("CHIMERA_GROUND_DEBUG").is_ok()) {
                eprintln!(
                    "    ground: feet {:.4} full frac {:.5} n {:.4} surf {:?} -> {:?}",
                    self.feet,
                    full.fraction,
                    full.normal,
                    full.surface,
                    ground.map(|g| (g.fraction, g.normal, g.surface))
                );
            }
            self.set_ground(ground.is_some(), ground.and_then(|g| g.owner));
            self.ground_collider = match ground.and_then(|g| g.surface) {
                Some(HitSurface::Collider(e)) => Some(e),
                _ => None,
            };
            if let Some(tr) = ground {
                if self.settle && !tr.start_solid {
                    self.feet = tr.end;
                }
                self.me.ground_normal = tr.normal;
                self.v.z = 0.0;
                let hit = tr.surface.and_then(|h| self.trace.surface_name(h));
                self.record_surface(hit);
            } else if self.v.z > 0.0 {
                self.me.surface_friction = UPWARD_AIR_FRICTION;
            }
        }
        self.water_check();
    }

    /// The ground material's max speed and jump factors (surface
    /// properties' maxspeedfactor and jumpfactor, 1 by default): the
    /// surface stood on at the tick's start, none in the air.
    pub(crate) fn ground_factors(&self) -> (f32, f32) {
        let surface = self
            .me
            .surface
            .as_deref()
            .filter(|_| self.me.on_ground)
            .and_then(|n| self.trace.sounds?.surface(n));
        (
            surface.and_then(|s| s.max_speed_factor).unwrap_or(1.0),
            surface.and_then(|s| s.jump_factor).unwrap_or(1.0),
        )
    }
}
