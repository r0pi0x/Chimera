//! Active world models (specs/combat_arms/world_objects.md 2-8): sliding,
//! rotating and spinning world models, doors, switches and breakables
//! share one state machine (off, powering on, on, powering off, paused)
//! driven by messages, the use key, timers and their own commands.
//!
//! `Awm` is the machine of one object; the object world (`runtime`)
//! owns its pose and runs what a transition asks for (`Out`: commands,
//! sounds, the door link). Motion advances by speed × frame time each
//! frame shaped by the waveform (`geometry::Waveform`), and stops (never
//! pushes, never crushes) while a character is in the way: the game's
//! own test refuses a step that would overlap a player (unless forced),
//! the engine's refuses one that would overlap any solid character
//! (`Blocking`). Boxes are the world model's box (its BSP's half-extents
//! about its centre, turned with it). `update_when_due` is one update as
//! the object frame runs it, with the riders it carries (`carried`): the
//! server's frame and a network client's prediction both run it, the
//! client with its own copy of the machine and what changes as it runs
//! from the server (`AwmMotion`, `net`).
//!
//! Known shortcuts: the game's player test uses the true pose (not
//! Quirk 6's unconjugated rotation); `ACTIVATE` by bots is accepted with
//! `AIActivate` but nothing in our bots sends it; `MAXHITPOINT` is the
//! destructible's (`runtime`).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::{
    commands::Who,
    geometry::{Obb, Waveform, rotation},
};

/// "Update next frame" (next_frame), s.
pub const NEXT_FRAME: f32 = 0.001;
/// The rotate-direction threshold (min_rotate_delta), radians.
pub const MIN_ROTATE_DELTA: f32 = 0.017_453_3;
/// The world models' `SlowOn`/`SlowOff` additive term (awm_slow_add).
pub const SLOW_ADD: f32 = 0.001;
/// Inner radius of an active world model's sounds × `SoundRadius`.
pub const SOUND_INNER_FRACTION: f32 = 0.25;
/// Ours: how far below a character's box a mover's top may be for it to
/// stand on it (u).
pub const STAND_PROBE: f32 = 2.0;

/// How it moves (`AWMType`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Motion {
    #[default]
    Static,
    Sliding,
    Rotating,
    Spinning,
}

/// Its state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    #[default]
    Off,
    PowerOn,
    On,
    PowerOff,
    Pause,
}

/// Transition sounds and commands, in this order.
pub const POWER_ON: usize = 0;
pub const ON: usize = 1;
pub const POWER_OFF: usize = 2;
pub const OFF: usize = 3;
pub const LOCKED: usize = 4;

/// The pose of a world model: its position and rotation (file frame)
/// and its box's half-extents.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub pos: Vec3,
    pub rot: Quat,
    pub half: Vec3,
}

impl Pose {
    pub fn obb(&self) -> Obb {
        Obb {
            centre: self.pos,
            rot: self.rot,
            half: self.half,
        }
    }
}

/// A character as the objects see it this frame (file frame, u).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharBox {
    pub who: Who,
    pub min: Vec3,
    pub max: Vec3,
    /// A player (bots aren't).
    pub player: bool,
    pub alive: bool,
    /// Holding the use key.
    pub using: bool,
}

impl CharBox {
    pub fn centre(&self) -> Vec3 {
        (self.min + self.max) / 2.0
    }
}

/// What a transition asks the object world for.
#[derive(Clone, Debug, PartialEq)]
pub enum Out {
    /// Run this command string (the world model the sender, the activator
    /// the default target).
    Command(String),
    /// Start this transition sound (stops the one before); `looping` while
    /// the transition lasts.
    Sound { file: String, looping: bool },
    /// Tell the linked door (this one was triggered into opening).
    DoorLink(String),
    /// Stop the object's looping transition sound (Corrections
    /// (2026-10-22) 3).
    StopSound,
}

/// What the characters let a mover do.
pub struct Blocking<'a> {
    pub chars: &'a [CharBox],
    /// The world model collides (`Solid`): the engine stops it at solid
    /// characters.
    pub solid: bool,
}

impl Blocking<'_> {
    /// Characters standing on the box at `pose` (their box a little
    /// lowered meets it, their own box doesn't); none on a world model
    /// that doesn't collide: nothing stands on it (HGOILRIG's rising sea,
    /// `Solid` false, passes through the players above it, which its
    /// water volume, not the sea, then holds).
    pub fn riders(&self, pose: &Pose) -> Vec<usize> {
        if !self.solid {
            return Vec::new();
        }
        let obb = pose.obb();
        self.chars
            .iter()
            .enumerate()
            .filter(|(_, c)| c.alive)
            .filter(|(_, c)| !obb.overlaps(c.min, c.max) && obb.overlaps(c.min - Vec3::Y * STAND_PROBE, c.max))
            .map(|(i, _)| i)
            .collect()
    }
}

/// A transition's spin (spinning world models, 8): angular speed per axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Spin {
    pub omega: Vec3,
    pub omega_final: Vec3,
    pub last: f64,
}

/// One active world model's machine.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Awm {
    pub motion: Motion,
    pub state: State,
    /// `PAUSE`: the state and position to return to.
    pub paused: Option<(State, Vec3)>,
    pub door: bool,
    pub player_activate: bool,
    pub ai_activate: bool,
    pub trigger_off: bool,
    pub remain_on: bool,
    pub force_move: bool,
    pub force_on: bool,
    pub force_off: bool,
    pub locked: bool,
    pub rotate_away: bool,
    pub waveform: Waveform,
    pub sounds: [String; 5],
    pub sound_pos: Vec3,
    pub sound_radius: f32,
    pub loop_sounds: bool,
    pub commands: [String; 5],
    pub activate_type: String,
    /// The world slide direction d̂ and distance (u).
    pub dir: Vec3,
    pub dist: f32,
    /// The set-aside `Rotation` Q.
    pub q: Quat,
    /// The original position (the box centre at load, or as last re-read).
    pub orig: Vec3,
    pub on_pos: Vec3,
    pub off_pos: Vec3,
    pub pivot: Vec3,
    /// Running angles (pitch, yaw, roll), radians.
    pub angles: Vec3,
    /// `RotationAngles` in radians (spinning: seconds per turn, as read).
    pub stored_angles: Vec3,
    /// The on angles of this opening (negated when rotating away).
    pub on_angles: Vec3,
    /// Per-axis direction (−1, 0, +1) chosen at load, and this opening's.
    pub stored_dirs: Vec3,
    pub dirs: Vec3,
    pub power_on_time: f32,
    pub power_off_time: f32,
    pub move_delay: f32,
    pub on_wait: f32,
    pub off_wait: f32,
    pub move_start: f64,
    pub stop_time: f64,
    /// The move timer τ.
    pub tau: f32,
    pub multiplier: f32,
    pub activator: Who,
    pub door_link: String,
    pub spin: Spin,
    /// Update countdown (s; None: not updating).
    pub countdown: Option<f32>,
}

/// The part of a machine that changes as it runs: with its own copy of
/// the map's machine (the same properties), a network client steps it as
/// the server does (10, Client; `net`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AwmMotion {
    pub state: State,
    pub paused: Option<(State, Vec3)>,
    pub locked: bool,
    pub orig: Vec3,
    pub on_pos: Vec3,
    pub off_pos: Vec3,
    pub pivot: Vec3,
    pub angles: Vec3,
    pub on_angles: Vec3,
    pub dirs: Vec3,
    pub move_start: f64,
    pub stop_time: f64,
    pub tau: f32,
    pub multiplier: f32,
    pub activator: Who,
    pub spin: Spin,
    pub countdown: Option<f32>,
}

impl Awm {
    /// What changes as it runs (`AwmMotion`).
    pub fn motion(&self) -> AwmMotion {
        AwmMotion {
            state: self.state,
            paused: self.paused,
            locked: self.locked,
            orig: self.orig,
            on_pos: self.on_pos,
            off_pos: self.off_pos,
            pivot: self.pivot,
            angles: self.angles,
            on_angles: self.on_angles,
            dirs: self.dirs,
            move_start: self.move_start,
            stop_time: self.stop_time,
            tau: self.tau,
            multiplier: self.multiplier,
            activator: self.activator,
            spin: self.spin,
            countdown: self.countdown,
        }
    }

    /// Take `m` as what changes as it runs.
    pub fn set_motion(&mut self, m: &AwmMotion) {
        self.state = m.state;
        self.paused = m.paused;
        self.locked = m.locked;
        self.orig = m.orig;
        self.on_pos = m.on_pos;
        self.off_pos = m.off_pos;
        self.pivot = m.pivot;
        self.angles = m.angles;
        self.on_angles = m.on_angles;
        self.dirs = m.dirs;
        self.move_start = m.move_start;
        self.stop_time = m.stop_time;
        self.tau = m.tau;
        self.multiplier = m.multiplier;
        self.activator = m.activator;
        self.spin = m.spin;
        self.countdown = m.countdown;
    }
}

/// One active world model's update when its countdown runs out (2-8), as
/// the object frame runs it: the pose it moved to and the characters
/// riding it (by index in `chars`), when it moved. The server's frame and
/// a network client's prediction (`net`) both run this.
#[allow(clippy::too_many_arguments)]
pub fn update_when_due(
    a: &mut Awm,
    pose: &Pose,
    solid: bool,
    chars: &[CharBox],
    now: f64,
    dt: f32,
    out: &mut Vec<Out>,
) -> Option<(Pose, Vec<usize>)> {
    let c = a.countdown.as_mut()?;
    *c -= dt;
    if *c > 0.0 {
        return None;
    }
    let f = Frame {
        now,
        dt,
        blocking: Blocking { chars, solid },
    };
    let next = a.update(&f, pose, out)?;
    Some((next, f.blocking.riders(pose)))
}

/// How far a character riding a world model that moved from `from` to
/// `to` is carried (10: its centre turned about the world model and
/// moved with it).
pub fn carried(c: &CharBox, from: &Pose, to: &Pose) -> Vec3 {
    let turn = to.rot * from.rot.inverse();
    let centre = c.centre();
    to.pos + turn * (centre - from.pos) - centre
}

/// The per-axis direction for on angles `on` (Quirk 4).
pub fn directions(on: Vec3) -> Vec3 {
    let d = |a: f32| {
        let delta = -a;
        if delta > MIN_ROTATE_DELTA {
            -1.0
        } else if delta < MIN_ROTATE_DELTA {
            1.0
        } else {
            0.0
        }
    };
    Vec3::new(d(on.x), d(on.y), d(on.z))
}

/// Context of one call: the time, the frame's dt and the characters.
pub struct Frame<'a> {
    pub now: f64,
    pub dt: f32,
    pub blocking: Blocking<'a>,
}

impl Awm {
    /// The duration of the move of `state` (s).
    pub fn duration(&self, state: State) -> f32 {
        match state {
            State::PowerOff if self.power_off_time > 0.0 => self.power_off_time,
            State::PowerOff => self.power_on_time,
            _ => self.power_on_time * self.multiplier,
        }
    }

    /// Whether the use key / bots can activate it.
    pub fn activatable(&self) -> bool {
        self.player_activate || self.ai_activate
    }

    /// The rotation and position for running angles `a`.
    pub fn rotated(&self, a: Vec3) -> (Vec3, Quat) {
        let r = (self.q * rotation(a.x, a.y, a.z) * self.q.inverse()).normalize();
        (r * (self.orig - self.pivot) + self.pivot, r)
    }

    /// Initial state at load: `StartOn` enters PowerOn (with sound and
    /// command), else Off silently with no waits.
    pub fn start(&mut self, start_on: bool, pose: &Pose, now: f64, out: &mut Vec<Out>) {
        self.multiplier = 1.0;
        self.stop_time = -10.0;
        self.move_start = -10.0;
        self.dirs = self.stored_dirs;
        self.on_angles = self.stored_angles;
        if start_on {
            self.enter(State::PowerOn, pose, None, now, out);
        } else {
            self.state = State::Off;
        }
    }

    fn sound(&self, i: usize, out: &mut Vec<Out>) {
        if !self.sounds[i].is_empty() {
            out.push(Out::Sound {
                file: self.sounds[i].clone(),
                looping: self.loop_sounds && i != LOCKED,
            });
        }
    }

    /// `REMEMBER`: the remembered state's transition sound starts again
    /// when the world model loops its sounds (Corrections (2026-10-22)
    /// 15).
    pub fn restart_sound(&self, out: &mut Vec<Out>) {
        if !self.loop_sounds {
            return;
        }
        out.push(Out::StopSound);
        match self.state {
            State::PowerOn => self.sound(POWER_ON, out),
            State::On => self.sound(ON, out),
            State::PowerOff => self.sound(POWER_OFF, out),
            State::Off => self.sound(OFF, out),
            State::Pause => {}
        }
    }

    fn command(&self, i: usize, out: &mut Vec<Out>) {
        if !self.commands[i].is_empty() {
            out.push(Out::Command(self.commands[i].clone()));
        }
    }

    /// Enter `state` (2, "Entering a state"); `activator_at` is where the
    /// activator stands, for rotating away.
    pub fn enter(&mut self, state: State, pose: &Pose, activator_at: Option<Vec3>, now: f64, out: &mut Vec<Out>) {
        let from = self.state;
        self.state = state;
        // Every state change first stops a looping transition sound, even
        // when the new state has none of its own.
        if self.loop_sounds && state != State::Pause {
            out.push(Out::StopSound);
        }
        match state {
            State::PowerOn => {
                if from == State::Off {
                    let shift = pose.pos - self.orig;
                    self.off_pos = pose.pos;
                    self.on_pos = self.off_pos + self.dir * self.dist;
                    if matches!(self.motion, Motion::Rotating | Motion::Spinning) {
                        self.pivot += shift;
                    }
                    self.orig = pose.pos;
                    if self.motion == Motion::Rotating {
                        self.choose_away(activator_at);
                    }
                }
                self.sound(POWER_ON, out);
                self.command(POWER_ON, out);
                self.move_start = now + self.move_delay as f64;
                self.tau = 0.0;
                if self.motion == Motion::Spinning {
                    self.spin.last = now;
                }
                self.countdown = Some(NEXT_FRAME);
            }
            State::On => {
                self.countdown = if self.trigger_off || self.remain_on {
                    None
                } else {
                    Some(self.on_wait + NEXT_FRAME)
                };
                if self.motion == Motion::Spinning {
                    // Spinning keeps turning while on.
                    self.countdown = Some(NEXT_FRAME);
                }
                self.sound(ON, out);
                self.command(ON, out);
                self.stop_time = now;
                self.multiplier = 1.0;
            }
            State::PowerOff => {
                if from == State::On {
                    self.on_pos = pose.pos;
                    self.off_pos = self.on_pos - self.dir * self.dist;
                }
                self.sound(POWER_OFF, out);
                self.command(POWER_OFF, out);
                self.move_start = now + self.move_delay as f64;
                self.tau = 0.0;
                self.countdown = Some(NEXT_FRAME);
            }
            State::Off => {
                self.countdown = None;
                self.dirs = self.stored_dirs;
                self.on_angles = self.stored_angles;
                self.sound(OFF, out);
                self.command(OFF, out);
                self.stop_time = now;
            }
            State::Pause => {}
        }
    }

    /// Rotate away (4): trial poses at ±1 rad on each turning axis; when
    /// the first's centre is closer to the activator, this opening turns
    /// the other way (the roll sign from the yaw angle, Quirk 5).
    fn choose_away(&mut self, activator_at: Option<Vec3>) {
        self.dirs = self.stored_dirs;
        self.on_angles = self.stored_angles;
        let Some(at) = activator_at.filter(|_| self.rotate_away) else { return };
        let a = self.stored_angles;
        let sign = |x: f32| if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 };
        let trial = Vec3::new(sign(a.x), sign(a.y), sign(a.y) * (a.z != 0.0) as u8 as f32);
        let (c1, _) = self.rotated(trial);
        let (c2, _) = self.rotated(-trial);
        if c1.distance(at) < c2.distance(at) {
            self.on_angles = -self.stored_angles;
            self.dirs = -self.stored_dirs;
        }
    }

    /// A trigger (`TRIGGER`, an accepted `ON`/`OFF` or activation).
    pub fn trigger(&mut self, f: &Frame, pose: &Pose, activator_at: Option<Vec3>, out: &mut Vec<Out>) -> bool {
        if self.locked {
            if self.loop_sounds {
                out.push(Out::StopSound);
            }
            if !self.sounds[LOCKED].is_empty() {
                out.push(Out::Sound {
                    file: self.sounds[LOCKED].clone(),
                    looping: false,
                });
            }
            self.command(LOCKED, out);
            return false;
        }
        match self.state {
            State::Off => {
                if f.now + 1e-6 >= self.stop_time + self.off_wait as f64 {
                    self.enter(State::PowerOn, pose, activator_at, f.now, out);
                    return true;
                }
            }
            State::PowerOn | State::PowerOff => {
                let forced = self.force_move
                    || (self.force_on && self.state == State::PowerOn)
                    || (self.force_off && self.state == State::PowerOff);
                if !forced {
                    self.reverse(f, pose, activator_at, out);
                }
            }
            State::On => {
                if self.trigger_off {
                    if f.now + 1e-6 >= self.stop_time + self.on_wait as f64 {
                        self.enter(State::PowerOff, pose, activator_at, f.now, out);
                    }
                } else {
                    // Re-arm the auto-off without sound or command.
                    self.countdown = if self.remain_on { None } else { Some(self.on_wait + NEXT_FRAME) };
                    self.stop_time = f.now;
                }
            }
            State::Pause => {}
        }
        false
    }

    /// Reversing (6): unless the activator would stand in the pose one
    /// frame's step along the reversed move (speed at p = 0.1 of the
    /// opposite state's plain duration; a rotation's rate from its
    /// current angle), the opposite state, the timer from 0 (Corrections
    /// (2026-10-22) 1).
    fn reverse(&mut self, f: &Frame, pose: &Pose, activator_at: Option<Vec3>, out: &mut Vec<Out>) {
        let opposite = if self.state == State::PowerOn { State::PowerOff } else { State::PowerOn };
        if activator_at.is_some()
            && let Some(who) = self.activator_box(f)
        {
            let mut trial = self.clone();
            trial.state = opposite;
            trial.multiplier = 1.0;
            trial.tau = 0.1 * trial.duration(opposite);
            let (p, r, _) = trial.next_pose(pose, f.dt, true);
            let obb = Pose { pos: p, rot: r, ..*pose }.obb();
            if obb.overlaps(who.min, who.max) {
                return;
            }
        }
        self.enter(opposite, pose, activator_at, f.now, out);
    }

    fn activator_box<'a>(&self, f: &'a Frame) -> Option<&'a CharBox> {
        f.blocking.chars.iter().find(|c| c.who == self.activator && c.who != Who::Nobody)
    }

    /// The pose after one step of `dt` toward the state's target (sliding
    /// and rotating), with whether it arrives; `test` is the reversal
    /// test (a rotation's rate is from its current angle).
    fn next_pose(&self, pose: &Pose, dt: f32, test: bool) -> (Vec3, Quat, Option<Vec3>) {
        let t = self.duration(self.state);
        let p = if t <= 0.001 { 1.0 } else { (self.tau / t).clamp(0.0, 1.0) };
        match self.motion {
            Motion::Sliding => {
                let target = if self.state == State::PowerOn { self.on_pos } else { self.off_pos };
                let r = if t <= 0.001 { self.dist } else { self.dist / t };
                let s = self.waveform.speed(r, p, SLOW_ADD, SLOW_ADD) * dt;
                let left = target - pose.pos;
                // (Float error aside: a step a hair short arrives.)
                let pos = if s >= left.length() - 1e-4 || p >= 1.0 {
                    target
                } else {
                    pose.pos + s * left.normalize_or_zero()
                };
                (pos, pose.rot, None)
            }
            Motion::Rotating => {
                let (a0, a1, s) = if self.state == State::PowerOn {
                    (Vec3::ZERO, self.on_angles, self.dirs)
                } else {
                    (self.on_angles, Vec3::ZERO, -self.dirs)
                };
                let mut a = self.angles;
                for k in 0..3 {
                    a[k] = self.step_axis(a[k], a0[k], a1[k], s[k], t, p, dt, test);
                }
                let (pos, rot) = self.rotated(a);
                (pos, rot, Some(a))
            }
            _ => (pose.pos, pose.rot, None),
        }
    }

    /// One axis's step (4).
    #[allow(clippy::too_many_arguments)]
    fn step_axis(&self, a: f32, a0: f32, a1: f32, s: f32, t: f32, p: f32, dt: f32, test: bool) -> f32 {
        if p >= 1.0 {
            return a1;
        }
        let going = (s > 0.0 && a < a1) || (s < 0.0 && a > a1);
        if s == 0.0 {
            return a;
        }
        if !going {
            return a1;
        }
        let from = if test { a } else { a0 };
        let rho = if t <= 0.001 { a1 - from } else { (a1 - from) / t };
        let next = a + self.waveform.speed(rho, p, SLOW_ADD, SLOW_ADD) * dt;
        // Within float error of the target: there.
        if (a1 - next).abs() < 1e-5 {
            return a1;
        }
        if s > 0.0 { next.min(a1) } else { next.max(a1) }
    }

    /// Whether every axis is done at angles `a` in the current move.
    fn rotation_done(&self, a: Vec3) -> bool {
        let (a1, s) = if self.state == State::PowerOn {
            (self.on_angles, self.dirs)
        } else {
            (Vec3::ZERO, -self.dirs)
        };
        (0..3).all(|k| s[k] == 0.0 || (s[k] > 0.0 && a[k] >= a1[k]) || (s[k] < 0.0 && a[k] <= a1[k]))
    }

    /// Whether a step to `next` is refused (6): the game's player test
    /// (unless forced), then the engine's (any solid character not
    /// riding).
    fn refused(&self, f: &Frame, pose: &Pose, next: &Pose, riders: &[usize]) -> bool {
        let forced = self.force_move
            || (self.force_on && self.state == State::PowerOn)
            || (self.force_off && self.state == State::PowerOff);
        let obb = next.obb();
        let now = pose.obb();
        // (A character already in the box where it stands, as our boxes
        // allow at its edges, doesn't hold it: the engine can't push it out
        // either way.)
        if !forced {
            for c in f.blocking.chars.iter().filter(|c| c.player && c.alive) {
                if !obb.overlaps(c.min, c.max) || now.overlaps(c.min, c.max) {
                    continue;
                }
                // A rotating one swinging away from its activator.
                let away = matches!(self.motion, Motion::Rotating | Motion::Spinning)
                    && c.who == self.activator
                    && next.pos.distance(c.centre()) > pose.pos.distance(c.centre());
                if !away {
                    return true;
                }
            }
        }
        f.blocking.solid
            && f.blocking
                .chars
                .iter()
                .enumerate()
                .any(|(i, c)| c.alive && !riders.contains(&i) && obb.overlaps(c.min, c.max) && !now.overlaps(c.min, c.max))
    }

    /// One update (each frame while it updates): the move's step, or the
    /// auto-off. Returns the new pose when it moved.
    pub fn update(&mut self, f: &Frame, pose: &Pose, out: &mut Vec<Out>) -> Option<Pose> {
        match self.state {
            State::PowerOn | State::PowerOff if self.motion == Motion::Spinning => self.spin_step(f, pose, out),
            State::On if self.motion == Motion::Spinning => self.spin_step(f, pose, out),
            State::PowerOn | State::PowerOff => {
                let wait = if self.state == State::PowerOff { self.move_delay as f64 } else { 0.0 };
                self.countdown = Some(NEXT_FRAME);
                if f.now + 1e-9 < self.move_start + wait {
                    return None;
                }
                let (pos, rot, angles) = self.next_pose(pose, f.dt, false);
                let next = Pose { pos, rot, ..*pose };
                let moved = next != *pose;
                if moved {
                    let riders = f.blocking.riders(pose);
                    if self.refused(f, pose, &next, &riders) {
                        return None;
                    }
                }
                if let Some(a) = angles {
                    self.angles = a;
                }
                let arrived = match self.motion {
                    Motion::Sliding => {
                        let target = if self.state == State::PowerOn { self.on_pos } else { self.off_pos };
                        next.pos == target
                    }
                    Motion::Rotating => self.rotation_done(self.angles),
                    _ => true,
                };
                if arrived {
                    let s = if self.state == State::PowerOn { State::On } else { State::Off };
                    self.enter(s, &next, None, f.now, out);
                }
                self.tau += f.dt;
                moved.then_some(next)
            }
            State::On => {
                // The auto-off.
                self.enter(State::PowerOff, pose, None, f.now, out);
                None
            }
            State::Off | State::Pause => {
                self.countdown = None;
                None
            }
        }
    }

    /// A spinning world model's frame (8).
    fn spin_step(&mut self, f: &Frame, pose: &Pose, out: &mut Vec<Out>) -> Option<Pose> {
        self.countdown = Some(NEXT_FRAME);
        let t = self.duration(self.state) as f64;
        let w = self.spin.omega_final;
        if matches!(self.state, State::PowerOn | State::PowerOff) {
            let p = if t <= 0.0 { f32::INFINITY } else { ((f.now - self.move_start) / t) as f32 };
            let mut done = true;
            for k in 0..3 {
                let rate = if t <= 0.0 { f32::INFINITY } else { w[k] / t as f32 };
                let dv = self.waveform.speed(rate, p.min(1.0), SLOW_ADD, SLOW_ADD) * f.dt;
                let (lo, hi) = (w[k].min(0.0), w[k].max(0.0));
                if self.state == State::PowerOn {
                    let v = if dv.is_finite() { self.spin.omega[k] + dv } else { w[k] };
                    self.spin.omega[k] = v.clamp(lo, hi);
                    done &= self.spin.omega[k] == w[k];
                } else {
                    let v = if dv.is_finite() { self.spin.omega[k] - dv } else { 0.0 };
                    self.spin.omega[k] = v.clamp(lo, hi);
                    done &= self.spin.omega[k] == 0.0;
                }
            }
            if done || p > 1.0 {
                if self.state == State::PowerOn {
                    self.spin.omega = w;
                    self.enter(State::On, pose, None, f.now, out);
                } else {
                    self.spin.omega = Vec3::ZERO;
                    self.enter(State::Off, pose, None, f.now, out);
                    self.spin.last = f.now;
                    return None;
                }
            }
        }
        let since = (f.now - self.spin.last) as f32;
        let mut a = self.angles + self.spin.omega * since;
        let tau = std::f32::consts::TAU;
        for k in 0..3 {
            if a[k] > tau {
                a[k] -= tau;
            } else if a[k] <= -tau {
                a[k] += tau;
            }
        }
        let (pos, rot) = self.rotated(a);
        let next = Pose { pos, rot, ..*pose };
        if next == *pose {
            self.spin.last = f.now;
            return None;
        }
        let riders = f.blocking.riders(pose);
        if self.refused(f, pose, &next, &riders) {
            self.move_start += f.dt as f64;
            self.spin.last = f.now;
            return None;
        }
        self.angles = a;
        self.spin.last = f.now;
        Some(next)
    }

    /// `PAUSE`: while powering, remember the state and position.
    pub fn pause(&mut self, pose: &Pose) {
        if matches!(self.state, State::PowerOn | State::PowerOff) {
            self.paused = Some((self.state, pose.pos));
            self.state = State::Pause;
            self.countdown = None;
        }
    }

    /// `RESUME`: back to the remembered state, positions shifted by how
    /// far something else moved it meanwhile.
    pub fn resume(&mut self, pose: &Pose) {
        if self.state != State::Pause {
            return;
        }
        let Some((s, at)) = self.paused.take() else { return };
        let shift = pose.pos - at;
        self.on_pos += shift;
        self.off_pos += shift;
        self.orig += shift;
        self.state = s;
        self.countdown = Some(NEXT_FRAME);
    }

    /// `INIT`: Off at the original position at once, timers cleared, no
    /// sound or command; the angles are kept (Quirk 3). Returns the
    /// position to teleport to.
    pub fn init(&mut self) -> Vec3 {
        self.state = State::Off;
        self.paused = None;
        self.move_start = -10.0;
        self.stop_time = -10.0;
        self.tau = 0.0;
        self.on_pos = self.orig;
        self.off_pos = self.orig;
        self.dirs = self.stored_dirs;
        self.on_angles = self.stored_angles;
        self.countdown = None;
        self.orig
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_test_is_quirk_4() {
        let one = MIN_ROTATE_DELTA;
        assert_eq!(directions(Vec3::new(0.0, 90f32.to_radians(), 0.0)), Vec3::new(1.0, 1.0, 1.0));
        assert_eq!(directions(Vec3::new(0.0, -90f32.to_radians(), 0.0)).y, -1.0);
        assert_eq!(directions(Vec3::new(0.0, -one, 0.0)).y, 0.0);
    }
}
