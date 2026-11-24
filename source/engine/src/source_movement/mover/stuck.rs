//! Stuck recovery (the nudge table) and seating: a character's footing for
//! another movement (`footing`), and one switched to this movement put on
//! the old one's feet and freed (`seat`, `Mover::reseat`).

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::source_movement::{
    config::config_of,
    mover::{GROUND_PROBE, Mover, STUCK_NUDGE_GAP, WALKABLE_NORMAL_Z},
    state::SourceMovement,
    units::{METERS_PER_UNIT, ORIGIN_ABOVE_FEET},
};

/// Offsets tried, in order, to free a stuck player (spec, "Stuck recovery").
pub(crate) fn nudge_table() -> Vec<Vec3> {
    let small = [-0.125f32, 0.0, 0.125];
    let mut t = Vec::with_capacity(54);
    t.extend(small.map(|z| Vec3::new(0.0, 0.0, z)));
    t.extend(small.map(|y| Vec3::new(0.0, y, 0.0)));
    t.extend(small.map(|x| Vec3::new(x, 0.0, 0.0)));
    for x in [-0.125f32, 0.125] {
        for y in [-0.125f32, 0.125] {
            for z in [-0.125f32, 0.125] {
                t.push(Vec3::new(x, y, z));
            }
        }
    }
    t.extend([0.0f32, 1.0, 6.0].map(|z| Vec3::new(0.0, 0.0, z)));
    t.extend([-2.0f32, 0.0, 2.0].map(|y| Vec3::new(0.0, y, 0.0)));
    t.extend([-2.0f32, 0.0, 2.0].map(|x| Vec3::new(x, 0.0, 0.0)));
    for z in [0.0f32, 1.0, 6.0] {
        for x in [-2.0f32, 0.0, 2.0] {
            for y in [-2.0f32, 0.0, 2.0] {
                t.push(Vec3::new(x, y, z));
            }
        }
    }
    t.push(Vec3::ZERO);
    t
}

/// Where a Source character stands (`slots::Footing`): its feet are the
/// origin's fixed 36 units down whatever its box.
pub(crate) fn footing(world: &World, e: Entity) -> Option<crate::slots::Footing> {
    let me = world.get::<SourceMovement>(e)?;
    let t = world.get::<Transform>(e)?;
    Some(crate::slots::Footing {
        feet: t.translation - Vec3::Y * ORIGIN_ABOVE_FEET * METERS_PER_UNIT,
        crouched: me.ducked,
        on_ground: me.on_ground || me.seat == Some(true),
    })
}

/// A character switched to a Source movement (`slots::set_movement`) put on
/// the old movement's feet, ducked if it crouched (duck held: let go, it
/// stands up when there's room); the step frees and grounds it
/// (`Mover::reseat`).
pub(crate) fn seat(world: &mut World, e: Entity, f: crate::slots::Footing) {
    let cfg = config_of(world, e);
    let at = f.feet + Vec3::Y * ORIGIN_ABOVE_FEET * METERS_PER_UNIT;
    let Ok(mut ent) = world.get_entity_mut(e) else { return };
    if let Some(mut me) = ent.get_mut::<SourceMovement>() {
        me.ducked = f.crouched;
        me.duck_held = f.crouched;
        me.ducking = false;
        me.duck_timer = 0.0;
        me.eye = if f.crouched { cfg.eye_duck } else { cfg.eye_stand };
        me.on_ground = f.on_ground;
        me.seat = Some(f.on_ground);
    }
    if let Some(mut t) = ent.get_mut::<Transform>() {
        t.translation = at;
    }
    if let Some(mut p) = ent.get_mut::<Position>() {
        p.0 = at;
    }
}

/// Where `Mover::reseat` looks for room, nearest first (Source units, Z
/// up): up to a step up and half a box sideways.
pub(crate) fn seat_offsets() -> Vec<Vec3> {
    const D: f32 = std::f32::consts::FRAC_1_SQRT_2;
    let dirs = [(1.0, 0.0), (D, D), (0.0, 1.0), (-D, D), (-1.0, 0.0), (-D, -D), (0.0, -1.0), (D, -D)];
    let mut out = Vec::new();
    for z in [0.0f32, 1.0, 2.0, 4.0, 8.0, 12.0, 18.0] {
        out.push(Vec3::new(0.0, 0.0, z));
        for r in [2.0f32, 4.0, 6.0, 8.0, 12.0, 16.0] {
            out.extend(dirs.iter().map(|&(x, y)| Vec3::new(x * r, y * r, z)));
        }
    }
    out.sort_by(|a, b| a.length_squared().total_cmp(&b.length_squared()));
    out
}

impl Mover<'_, '_, '_, '_> {
    /// Stuck recovery: about once a second (every tick while recovering),
    /// test the box; if it's stuck, try one nudge from the table, at most
    /// every 0.05 s. Returns false when movement is skipped this tick.
    pub(crate) fn check_stuck(&mut self) -> bool {
        self.me.ticks += 1;
        self.me.time += self.dt;
        let interval = ((1.0 / self.dt) as u64).max(1);
        if self.me.nudge == 0 && !self.me.ticks.is_multiple_of(interval) {
            return true;
        }
        if !self.trace.solid(self.me.ducked, self.feet) {
            self.me.nudge = 0;
            return true;
        }
        if self.me.time - self.me.last_nudge < STUCK_NUDGE_GAP {
            return false;
        }
        self.me.last_nudge = self.me.time;
        let table = nudge_table();
        let at = self.feet + table[self.me.nudge % table.len()];
        self.me.nudge = (self.me.nudge + 1) % table.len();
        if self.trace.solid(self.me.ducked, at) {
            return false;
        }
        self.feet = at;
        self.me.nudge = 0;
        true
    }

    /// After a switch from another movement (`seat`): the box out of any
    /// solid it overlaps (the nearest free place up to a step up or half a
    /// box aside, `seat_offsets`; ducked when only that fits), then, if it
    /// stood on the ground, down onto the floor within a step below.
    pub(crate) fn reseat(&mut self, on_ground: bool) {
        if self.trace.solid(self.me.ducked, self.feet) {
            let offsets = seat_offsets();
            let free = |ducked: bool| {
                offsets
                    .iter()
                    .map(|d| self.feet + *d)
                    .find(|p| !self.trace.solid(ducked, *p))
            };
            if let Some(p) = free(self.me.ducked) {
                self.feet = p;
            } else if !self.me.ducked
                && let Some(p) = free(true)
            {
                self.feet = p;
                self.me.ducked = true;
                self.me.duck_held = true;
                self.me.eye = self.cfg.eye_duck;
            }
        }
        if on_ground {
            let down = self.cfg.stepsize + GROUND_PROBE;
            let t = self.trace.sweep(self.me.ducked, self.feet, self.feet - Vec3::Z * down);
            if t.hit() && !t.start_solid && t.normal.z >= WALKABLE_NORMAL_Z {
                self.feet = t.end;
            }
        }
        self.me.last_feet = None;
        self.me.nudge = 0;
    }
}
