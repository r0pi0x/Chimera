//! Where a Source game's view model sits relative to the eye (lift S8,
//! docs/plans/active/hl2dm.md; moved from CS:S's `game::view_motion`):
//! the walking bob, tied to distance travelled, then the sway that lags the
//! view's rotation, applied in that order on top of the eye angles (which
//! already include the view punch). Writes the local player's
//! `map::ViewModelOffset`; `map::view_model` mirrors it with the model
//! (or not: a game's models that are never mirrored load so).
//!
//! Each game says how its view model moves (`MotionRules`, registered by
//! game: `MotionAppExt::view_motion_rules`; `update` uses the rules of the
//! game of the weapon in the local player's hand):
//!
//! - **The bob** (specs/cs_source/view_models.md 3): one wave of period P
//!   and rising fraction U in bob time T_b (distance / 320 units), V on
//!   P and L on 2P; CS:S reads P and U from `cl_bobcycle` and `cl_bobup`
//!   (`ViewMotion`) and limits how fast the bob's speed changes; HL2:DM
//!   fixes them at 0.45 and 0.5 with no slew (specs/hl2dm/
//!   player_models.md 12.2). V moves the model along the view's forward
//!   (`MotionRules::forward`, CS:S 0.4, HL2:DM 0.1) and world up (0.1),
//!   L along its right (`right`, 0.2 and 0.8); both turn it (roll 0.5 V,
//!   pitch -0.4 V, yaw -0.3 L).
//! - **The sway** (`Sway`): CS:S's `History` (view_models.md 4: the angles
//!   `cl_wpn_sway_interp` ago, `cl_wpn_sway_scale`), or HL2's `Spring`
//!   (player_models.md 12.3: a lagging forward vector pulled toward the
//!   view's at 5 per second, faster beyond 1.5 apart, the model moved 5
//!   units against the difference; the first frame of a new view model
//!   pops it 5 units back) with its pitch offsets (looking down moves it
//!   back 0.035, left 0.03 and down 0.02 units per degree).
//!
//! Angles here are Source's: degrees (pitch, yaw, roll), pitch positive
//! looking down, yaw positive turning left; vectors in Source axes (x
//! forward, y left, z up) and units.

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::{
    core::{Intent, LocalPlayer, Velocity},
    map::ViewModelOffset,
    weapon::{Inventory, ViewPunch, Weapon},
};

const UNIT: f32 = 0.0254;

/// The bob's speed clamp and slew limit (units/s, units/s^2).
pub const BOB_SPEED_MAX: f32 = 320.0;
/// Bob amplitude per unit/s of speed.
pub const BOB_AMP: f32 = 0.005;
/// Clamp on each bob value.
pub const BOB_CLAMP: (f32, f32) = (-7.0, 4.0);
/// Offsets and angles per unit of vertical (V) or lateral (L) bob, but
/// forward and right (`MotionRules`).
pub const BOB_UP: f32 = 0.1;
pub const BOB_ROLL: f32 = 0.5;
pub const BOB_PITCH: f32 = -0.4;
pub const BOB_YAW: f32 = -0.3;
/// Sway history kept beyond the lag (s).
const SWAY_KEEP: f64 = 0.05;
/// HL2's spring sway: the pull per second, the distance beyond which it
/// pulls harder, the units the model moves per unit of difference.
pub const SPRING_SPEED: f32 = 5.0;
pub const SPRING_FAR: f32 = 1.5;
pub const SPRING_SCALE: f32 = 5.0;
/// HL2's pitch offsets: units along forward, right and up per degree of
/// eye pitch (negative: back, left, down when looking down).
pub const PITCH_OFFSETS: Vec3 = Vec3::new(-0.035, -0.03, -0.02);

/// How the view model sways.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sway {
    /// It shows the angles of `cl_wpn_sway_interp` ago (CS:S).
    History,
    /// A lagging forward vector on a spring, then the pitch offsets
    /// (HL2's).
    Spring,
}

/// A game's view-model motion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionRules {
    /// The bob's period and rising fraction; None: the cvars'
    /// (`ViewMotion`: `cl_bobcycle`, `cl_bobup`).
    pub cycle: Option<(f32, f32)>,
    /// Units along the view's forward per unit of V, along its right per
    /// unit of L.
    pub forward: f32,
    pub right: f32,
    /// The bob's speed changes at most `BOB_SPEED_MAX` per second.
    pub slew: bool,
    pub sway: Sway,
}

impl MotionRules {
    /// The cvar-driven model of specs/cs_source/view_models.md (CS:S's).
    pub const CVARS: MotionRules = MotionRules {
        cycle: None,
        forward: 0.4,
        right: 0.2,
        slew: true,
        sway: Sway::History,
    };
}

/// The motion cvars (CS:S defaults): `cl_bobcycle`, `cl_bobup`,
/// `cl_wpn_sway_interp`, `cl_wpn_sway_scale`.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct ViewMotion {
    pub bobcycle: f32,
    pub bobup: f32,
    pub sway_interp: f32,
    pub sway_scale: f32,
}

impl Default for ViewMotion {
    fn default() -> Self {
        Self {
            bobcycle: 0.8,
            bobup: 0.5,
            sway_interp: 0.1,
            sway_scale: 1.0,
        }
    }
}

/// Each game's `MotionRules`, by game id.
#[derive(Resource, Clone, Debug, Default)]
pub struct GameMotionRules(pub Vec<(&'static str, MotionRules)>);

impl GameMotionRules {
    pub fn get(&self, game: &str) -> Option<MotionRules> {
        self.0.iter().find(|(g, _)| g.eq_ignore_ascii_case(game)).map(|(_, r)| *r)
    }

    pub fn set(&mut self, game: &'static str, rules: MotionRules) {
        match self.0.iter_mut().find(|(g, _)| g.eq_ignore_ascii_case(game)) {
            Some(e) => e.1 = rules,
            None => self.0.push((game, rules)),
        }
    }
}

/// Registering a game's view-model motion.
pub trait MotionAppExt {
    fn view_motion_rules(&mut self, game: &'static str, rules: MotionRules) -> &mut Self;
}

impl MotionAppExt for App {
    fn view_motion_rules(&mut self, game: &'static str, rules: MotionRules) -> &mut Self {
        self.init_resource::<ViewMotion>();
        self.world_mut().get_resource_or_init::<GameMotionRules>().set(game, rules);
        self
    }
}

/// One bob wave at bob time `t_b` (s) for `speed` (units/s): period
/// `period`, rising for the fraction `up` of each cycle (spec 3.5–3.7).
pub fn bob_wave(t_b: f32, speed: f32, period: f32, up: f32) -> f32 {
    let period = if period <= 0.0 { 0.01 } else { period };
    let up = if up <= 0.0 { 0.01 } else { up };
    let c = (t_b / period).fract();
    let theta = if c < up {
        std::f32::consts::PI * c / up
    } else {
        std::f32::consts::PI + std::f32::consts::PI * (c - up) / (1.0 - up)
    };
    (BOB_AMP * speed * (0.3 + 0.7 * theta.sin())).clamp(BOB_CLAMP.0, BOB_CLAMP.1)
}

/// The bob's state between frames (spec 3, "State").
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bob {
    /// Bob time T_b.
    pub time: f32,
    pub last_time: f64,
    pub last_speed: f32,
    /// Vertical and lateral bob (V, L).
    pub vertical: f32,
    pub lateral: f32,
}

impl Bob {
    /// One frame at time `now` (`frametime` 0: paused, nothing changes)
    /// with the player's horizontal `speed` (units/s), the cvars' cycle.
    pub fn update(&mut self, now: f64, frametime: f32, speed: f32, motion: &ViewMotion) {
        self.update_with(now, frametime, speed, (motion.bobcycle, motion.bobup), true);
    }

    /// One frame with the bob's `(period, up)` and, with `slew`, the
    /// speed's change limited.
    pub fn update_with(&mut self, now: f64, frametime: f32, speed: f32, (period, up): (f32, f32), slew: bool) {
        if frametime == 0.0 {
            return;
        }
        let dt = (now - self.last_time) as f32;
        let s = if slew {
            let slew = (dt * BOB_SPEED_MAX).max(0.0);
            speed.clamp(self.last_speed - slew, self.last_speed + slew)
        } else {
            speed
        }
        .clamp(-BOB_SPEED_MAX, BOB_SPEED_MAX);
        self.last_speed = s;
        self.time += dt * s.abs() / BOB_SPEED_MAX;
        self.last_time = now;
        self.vertical = bob_wave(self.time, s.abs(), period, up);
        self.lateral = bob_wave(self.time, s.abs(), 2.0 * period, up);
    }
}

/// What a bob adds (spec 3, "Applying it"): offsets along the view's
/// forward and right and along world up (units), and angles (degrees).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BobOffset {
    pub forward: f32,
    pub right: f32,
    pub world_up: f32,
    /// (pitch, yaw, roll).
    pub angles: Vec3,
}

/// The bob's offset with CS:S's factors (`MotionRules::CVARS`).
pub fn bob_offset(vertical: f32, lateral: f32) -> BobOffset {
    bob_offset_with(&MotionRules::CVARS, vertical, lateral)
}

/// The bob's offset under `rules`.
pub fn bob_offset_with(rules: &MotionRules, vertical: f32, lateral: f32) -> BobOffset {
    BobOffset {
        forward: rules.forward * vertical,
        right: rules.right * lateral,
        world_up: BOB_UP * vertical,
        angles: Vec3::new(BOB_PITCH * vertical, BOB_YAW * lateral, BOB_ROLL * vertical),
    }
}

/// Source angles (pitch, yaw, roll; degrees) as a rotation in Source axes.
pub fn rotation(angles: Vec3) -> Quat {
    let a = angles * std::f32::consts::PI / 180.0;
    Quat::from_euler(EulerRot::ZYX, a.y, a.x, a.z)
}

/// The angles of a rotation in Source axes.
pub fn angles(q: Quat) -> Vec3 {
    let (yaw, pitch, roll) = q.to_euler(EulerRot::ZYX);
    Vec3::new(pitch, yaw, roll) * 180.0 / std::f32::consts::PI
}

/// The sway offset (spec 4.4–4.7) in the frame of the current angles:
/// (forward, right, up), units.
pub fn sway_offset(now: Vec3, lagged: Vec3, scale: f32) -> Vec3 {
    let d = (lagged - now) * std::f32::consts::PI / 180.0;
    // Forward of the angles -D.
    let g = Vec3::new((-d.x).cos() * (-d.y).cos(), (-d.x).cos() * (-d.y).sin(), -(-d.x).sin());
    let k = scale * (Vec3::X - g);
    Vec3::new(k.x, -k.y, k.z)
}

/// Recent view-model angles, for the lagged value (spec 4.2–4.3).
#[derive(Clone, Debug, Default)]
pub struct SwayHistory {
    samples: VecDeque<(f64, Quat)>,
}

impl SwayHistory {
    /// Add the angles at `now` and return those `interp` seconds ago.
    pub fn push(&mut self, now: f64, angles: Vec3, interp: f32) -> Vec3 {
        self.samples.push_back((now, rotation(angles)));
        let target = now - interp as f64;
        // Drop what is older than the lag plus a margin, keeping one
        // sample at or before the lagged time to interpolate from.
        while self.samples.len() > 1 && self.samples[1].0 < target - SWAY_KEEP {
            self.samples.pop_front();
        }
        self.at(target)
    }

    /// The (slerped) angles at time `t`; the oldest sample before the
    /// history starts.
    pub fn at(&self, t: f64) -> Vec3 {
        let Some(&(t0, q0)) = self.samples.front() else {
            return Vec3::ZERO;
        };
        if t <= t0 || self.samples.len() == 1 {
            return angles(q0);
        }
        for w in self.samples.iter().zip(self.samples.iter().skip(1)) {
            let ((ta, qa), (tb, qb)) = (*w.0, *w.1);
            if t >= ta && t <= tb {
                let f = if tb > ta { ((t - ta) / (tb - ta)) as f32 } else { 1.0 };
                return angles(qa.slerp(qb, f));
            }
        }
        angles(self.samples.back().unwrap().1)
    }
}

/// HL2's spring sway (player_models.md 12.3 steps 1-2): the lagging
/// forward `lag` (a unit vector, zero before the first frame) pulled toward
/// `forward` over `dt`; returns the offset (Source axes of the world,
/// units).
pub fn spring_sway(lag: &mut Vec3, forward: Vec3, dt: f32) -> Vec3 {
    if dt == 0.0 {
        return Vec3::ZERO;
    }
    let delta = forward - *lag;
    let d = delta.length();
    let k = if d > SPRING_FAR { SPRING_SPEED * d / SPRING_FAR } else { SPRING_SPEED };
    *lag = (*lag + delta * k * dt).normalize_or(forward);
    -delta * SPRING_SCALE
}

/// The view model's placement relative to the eye, in Source eye axes
/// (units; rotation): bob then sway, for eye angles `eye`, CS:S's rules.
pub fn placement(eye: Vec3, bob: &Bob, history: &mut SwayHistory, now: f64, motion: &ViewMotion) -> (Vec3, Quat) {
    let mut state = MotionState {
        bob: *bob,
        history: std::mem::take(history),
        lag: Vec3::ZERO,
    };
    let out = placement_with(&MotionRules::CVARS, eye, &mut state, now, 0.0, motion);
    *history = state.history;
    out
}

/// The sway's and the bob's state between frames.
#[derive(Clone, Debug, Default)]
pub struct MotionState {
    pub bob: Bob,
    pub history: SwayHistory,
    /// The spring sway's lagging forward (zero: a new view model).
    pub lag: Vec3,
}

/// The placement under `rules` (the bob in `state` already updated),
/// `dt` this frame's length (the spring's step).
pub fn placement_with(rules: &MotionRules, eye: Vec3, state: &mut MotionState, now: f64, dt: f32, motion: &ViewMotion) -> (Vec3, Quat) {
    let b = bob_offset_with(rules, state.bob.vertical, state.bob.lateral);
    let pitch = eye.x.to_radians();
    // World up seen from the eye (roll 0): forward -sin p, up cos p.
    let mut offset = Vec3::new(b.forward - b.world_up * pitch.sin(), -b.right, b.world_up * pitch.cos());
    let bobbed = eye + b.angles;
    let relative = rotation(eye).inverse() * rotation(bobbed);
    match rules.sway {
        Sway::History => {
            if motion.sway_interp > 0.0 {
                let lagged = state.history.push(now, bobbed, motion.sway_interp);
                let s = sway_offset(bobbed, lagged, motion.sway_scale);
                // (forward, right, up) in the bobbed frame, to Source axes.
                offset += relative * Vec3::new(s.x, -s.y, s.z);
            }
        }
        Sway::Spring => {
            let forward = rotation(bobbed) * Vec3::X;
            let world = spring_sway(&mut state.lag, forward, dt);
            offset += rotation(eye).inverse() * world;
            // The pitch offsets, with the eye's pitch (before the bob),
            // along the eye's own axes (right is -y).
            let p = (eye.x + 180.0).rem_euclid(360.0) - 180.0;
            let o = PITCH_OFFSETS * p;
            offset += Vec3::new(o.x, -o.y, o.z);
        }
    }
    (offset, relative)
}

/// Source eye axes and units to the camera's (X right, Y up, -Z forward;
/// meters).
pub fn to_camera(offset: Vec3, rotation: Quat) -> ViewModelOffset {
    ViewModelOffset {
        translation: Vec3::new(-offset.y, offset.z, -offset.x) * UNIT,
        rotation: Quat::from_xyzw(-rotation.y, rotation.z, -rotation.x, rotation.w),
    }
}

/// The local player's bob and sway state, and the weapon it was for.
#[derive(Component, Default)]
pub struct ViewMotionState {
    state: MotionState,
    weapon: Option<Entity>,
}

/// Each frame: the local player's view-model offset, by the rules of the
/// game of the weapon in its hand (none registered: left alone).
#[allow(clippy::type_complexity)]
pub fn update(
    time: Res<Time>,
    motion: Res<ViewMotion>,
    rules: Option<Res<GameMotionRules>>,
    mut players: Query<
        (
            Entity,
            &Intent,
            &Velocity,
            Option<&ViewPunch>,
            Option<&crate::map::interp::RenderedView>,
            Option<&Inventory>,
            Option<&mut ViewMotionState>,
            Option<&mut ViewModelOffset>,
        ),
        With<LocalPlayer>,
    >,
    weapons: Query<&Weapon>,
    mut commands: Commands,
) {
    let now = time.elapsed_secs_f64();
    let dt = time.delta_secs();
    for (e, intent, velocity, punch, view, inv, state, offset) in &mut players {
        let active = inv.and_then(|i| i.active);
        let game = active.and_then(|a| weapons.get(a).ok()).and_then(|w| w.id.split_once(':')).map(|(g, _)| g);
        let Some(r) = game.and_then(|g| rules.as_deref()?.get(g)) else { continue };
        let (Some(mut state), Some(mut offset)) = (state, offset) else {
            commands.entity(e).insert((
                ViewMotionState {
                    state: MotionState {
                        bob: Bob {
                            last_time: now,
                            ..default()
                        },
                        ..default()
                    },
                    weapon: active,
                },
                ViewModelOffset::default(),
            ));
            continue;
        };
        let state = &mut *state;
        if state.weapon != active {
            // A new view model: its spring starts over.
            state.weapon = active;
            state.state.lag = Vec3::ZERO;
        }
        let speed = Vec2::new(velocity.0.x, velocity.0.z).length() / UNIT;
        let cycle = r.cycle.unwrap_or((motion.bobcycle, motion.bobup));
        state.state.bob.update_with(now, dt, speed, cycle, r.slew);
        // The punch as the camera draws it (eased between ticks).
        let p = view.map_or_else(|| punch.map_or(Vec3::ZERO, |p| p.0), |v| v.now.punch);
        let eye = Vec3::new(-(intent.pitch + p.x), intent.yaw + p.y, 0.0) * 180.0 / std::f32::consts::PI;
        let (o, q) = placement_with(&r, eye, &mut state.state, now, dt, &motion);
        let next = to_camera(o, q);
        offset.set_if_neq(next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HL2: MotionRules = MotionRules {
        cycle: Some((0.45, 0.5)),
        forward: 0.1,
        right: 0.8,
        slew: false,
        sway: Sway::Spring,
    };

    /// player_models.md test 44: 0.1 s at 190 units/s from T_b 0.
    #[test]
    fn hl2s_bob_44_to_46() {
        let mut bob = Bob::default();
        for i in 1..=10 {
            bob.update_with(i as f64 * 0.01, 0.01, 190.0, (0.45, 0.5), false);
        }
        assert!((bob.time - 0.059375).abs() < 1e-5, "{}", bob.time);
        assert!((bob.vertical - 0.775289).abs() < 1e-4 && (bob.lateral - 0.552827).abs() < 1e-4, "{bob:?}");
        let b = bob_offset_with(&HL2, bob.vertical, bob.lateral);
        assert!((b.forward - 0.077529).abs() < 1e-4 && (b.world_up - 0.077529).abs() < 1e-4 && (b.right - 0.442261).abs() < 1e-4);
        assert!((b.angles - Vec3::new(-0.310116, -0.165848, 0.387645)).length() < 1e-4, "{:?}", b.angles);
        // Test 45 and 46: the crest and the trough.
        assert!((bob_wave(0.1125, 190.0, 0.45, 0.5) - 0.95).abs() < 1e-4);
        assert!((bob_wave(0.1125, 190.0, 0.9, 0.5) - 0.755226).abs() < 1e-4);
        assert!((bob_wave(0.3375, 320.0, 0.45, 0.5) + 0.64).abs() < 1e-4);
        assert!((bob_wave(0.3375, 320.0, 0.9, 0.5) - 1.271960).abs() < 1e-4);
    }

    /// Tests 48-50: no lag on a steady view, 0.8716 units against a 10
    /// degree turn, the pitch offsets at 30 down.
    #[test]
    fn hl2s_spring_sway_and_pitch_offsets_48_to_50() {
        let mut lag = Vec3::X;
        assert_eq!(spring_sway(&mut lag, Vec3::X, 0.01), Vec3::ZERO);
        let turned = rotation(Vec3::new(0.0, 10.0, 0.0)) * Vec3::X;
        let o = spring_sway(&mut lag, turned, 0.01);
        assert!((o.length() - 0.8716).abs() < 1e-3, "{o}");
        assert!(o.y < 0.0, "against a turn left: {o}");
        let o2 = spring_sway(&mut lag, turned, 0.01);
        assert!((o2.length() - 0.83).abs() < 0.01, "{o2}");
        let mut state = MotionState::default();
        state.lag = rotation(Vec3::new(30.0, 0.0, 0.0)) * Vec3::X;
        let (o, _) = placement_with(&HL2, Vec3::new(30.0, 0.0, 0.0), &mut state, 0.0, 0.01, &ViewMotion::default());
        assert!((o - Vec3::new(-1.05, 0.9, -0.6)).length() < 1e-3, "{o}");
        // A new view model (lag zero) pops 5 units back.
        let mut lag = Vec3::ZERO;
        let pop = spring_sway(&mut lag, Vec3::X, 0.01);
        assert!((pop - Vec3::new(-5.0, 0.0, 0.0)).length() < 1e-4);
    }
}
