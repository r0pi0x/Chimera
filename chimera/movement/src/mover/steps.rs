//! Footsteps: the surface underfoot, the step timer, the surface's or the
//! model's step sounds, impact steps.

use bevy::prelude::*;

use crate::{
    mover::{LAND_PUNCH_SCALE, Mover},
    units::{METERS_PER_UNIT, to_engine},
};

impl Mover<'_, '_, '_, '_> {
    /// The surface under the feet: the ground trace's own (a prop's), else
    /// the world face below (sound spec 3: the ground trace's surface).
    pub(crate) fn record_surface(&mut self, hit: Option<String>) {
        if hit.is_some() {
            self.me.surface = hit;
        } else if let Some(grid) = self.trace.surfaces
            && let Some(s) = grid.below(to_engine(self.feet + Vec3::Z), 6.0 * METERS_PER_UNIT)
        {
            self.me.surface = Some(s.to_string());
        }
    }

    /// Play a footstep on `surface` (unknown names: "default"): its left or
    /// right step, alternating
    /// (the first after spawn is a right step), at `volume`. With the feet
    /// in a footstep volume that replaces `surface` (func_footstep_control,
    /// `MapSounds::step_surface`), its destination surface's step plays.
    pub(crate) fn play_step(&mut self, surface: &str, volume: f32) {
        if !self.cfg.footsteps {
            return;
        }
        let Some(bank) = self.trace.sounds else { return };
        let surface = bank.step_surface(surface, to_engine(self.feet));
        // A name the surface scripts don't define (dust2's "stone" props)
        // uses "default", whose steps every surface inherits.
        let Some(s) = bank.surface(surface).or_else(|| bank.surface("default")) else {
            return;
        };
        let entry = if self.me.step_left { &s.step_left } else { &s.step_right };
        let Some(entry) = entry.clone() else { return };
        self.me.step_left = !self.me.step_left;
        self.sounds.push((entry, self.feet, Some(volume)));
    }

    /// One of the model's own steps (`StepSounds`), alternating and left
    /// first; none while ducked (HL2:DM, specs/hl2dm/movement.md "Step
    /// sounds").
    pub(crate) fn model_step(&mut self, volume: f32) {
        if !self.cfg.footsteps || self.me.ducked {
            return;
        }
        let Some(steps) = self.step_sounds else { return };
        self.me.step_left = !self.me.step_left;
        let entry = if self.me.step_left { &steps.left } else { &steps.right };
        self.sounds.push((entry.clone(), self.feet, Some(volume)));
    }

    /// HL2's footstep timing (the shared code's, movement.md "Step
    /// sounds"): bands 90/220 standing, 60/80 ducked or on a ladder; the
    /// interval and volume by the ladder, the water level or the ground's
    /// material; the model's step (`model_step`).
    pub(crate) fn model_footsteps(&mut self) {
        if self.me.step_timer > 0.0 {
            self.me.step_timer = (self.me.step_timer - 1000.0 * self.dt).max(0.0);
            if self.me.step_timer > 0.0 {
                return;
            }
        }
        let ladder = self.me.ladder.is_some() || self.me.useable.is_some();
        let speed = self.v.length();
        let (walk, run) = if ladder || self.me.ducked { (60.0, 80.0) } else { (90.0, 220.0) };
        if speed < walk || !(ladder || (self.me.on_ground && self.v.truncate().length() > 1e-4)) {
            return;
        }
        let slow = speed < run;
        let (mut volume, mut period) = if ladder {
            (0.5, 350.0)
        } else if self.me.water_level == 2 {
            // HL2: of every four attempts waist-deep (anyone's) the first
            // returns, the timer not set: that step comes a tick later.
            if self.cfg.waist_step_skip {
                let n = *self.waist_steps;
                *self.waist_steps = n.wrapping_add(1);
                if n % 4 == 0 {
                    return;
                }
            }
            (0.65, 600.0)
        } else if self.me.water_level == 1 {
            if slow { (0.2, 400.0) } else { (0.5, 300.0) }
        } else {
            let Some(surface) = self.me.surface.clone() else { return };
            let material = self
                .trace
                .sounds
                .and_then(|b| b.surface(&surface))
                .map_or('C', |s| s.game_material);
            let volume = match (material, slow) {
                ('D', true) => 0.25,
                ('D', false) => 0.55,
                ('V', true) => 0.4,
                ('V', false) => 0.7,
                (_, true) => 0.2,
                (_, false) => 0.5,
            };
            (volume, if slow { 400.0 } else { 300.0 })
        };
        if self.me.ducked || ladder {
            period += 100.0;
        }
        if self.me.ducked {
            volume *= 0.65;
        }
        self.me.step_timer = period;
        self.model_step(volume);
    }

    /// Footsteps (sound spec 3, "When a footstep is checked"), from the
    /// state at the start of the tick.
    pub(crate) fn footsteps(&mut self) {
        if self.cfg.model_footsteps {
            self.model_footsteps();
            return;
        }
        if self.me.step_timer > 0.0 {
            self.me.step_timer = (self.me.step_timer - 1000.0 * self.dt).max(0.0);
            if self.me.step_timer > 0.0 {
                return;
            }
        }
        let ladder = self.me.ladder.is_some();
        let speed = self.v.length();
        let (walk, run) = if ladder {
            (60.0, 80.0)
        } else if self.me.ducked {
            (self.cfg.step_min_speed.max(60.0), 80.0_f32.max(self.cfg.step_min_speed))
        } else {
            (self.cfg.step_min_speed, 220.0)
        };
        if speed < walk || !(ladder || (self.me.on_ground && self.v.truncate().length() > 1e-4)) {
            return;
        }
        let walking = speed < run;
        let height = if self.me.ducked {
            self.trace.heights.1
        } else {
            self.trace.heights.0
        };
        let knee = self.trace.point_water(self.feet + Vec3::Z * (0.2 * height)).is_some();
        let (surface, mut volume, mut period) = if ladder {
            (self.cfg.ladder_surface.to_string(), 0.5, 350.0)
        } else if knee {
            (self.cfg.wade_surface.to_string(), 0.65, 600.0)
        } else if self.me.water_level == 1 {
            (
                self.cfg.water_surface.to_string(),
                if walking { 0.2 } else { 0.5 },
                if walking { 400.0 } else { 300.0 },
            )
        } else {
            let Some(surface) = self.me.surface.clone() else { return };
            let material = self
                .trace
                .sounds
                .and_then(|b| b.surface(&surface))
                .map_or('C', |s| s.game_material);
            let volume = match (material, walking) {
                ('D', true) => 0.25,
                ('D', false) => 0.55,
                ('V', true) => 0.4,
                ('V', false) => 0.7,
                (_, true) => 0.2,
                (_, false) => 0.5,
            };
            (surface, volume, if walking { 400.0 } else { 300.0 })
        };
        if self.me.ducked || ladder {
            period += 100.0;
        }
        if self.me.ducked {
            volume *= 0.65;
        }
        self.me.step_timer = period;
        self.play_step(&surface, volume);
    }

    /// A landing or wall-slam step on the current surface.
    pub(crate) fn impact_step(&mut self, volume: f32) {
        if volume <= 0.0 {
            return;
        }
        self.me.step_timer = 400.0;
        // The view rolls by the current fall speed (a wall slam's is
        // usually 0).
        self.me.punch_roll = self.me.fall_speed * LAND_PUNCH_SCALE;
        if self.cfg.model_footsteps {
            self.model_step(volume);
        } else if let Some(s) = self.me.surface.clone() {
            self.play_step(&s, volume);
        }
    }
}
