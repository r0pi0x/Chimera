//! The first-person view's state machine and pose (specs/halo_ce/
//! animations.md, "First person": "The pose each drawn frame", "Smoothed
//! inputs", "States", "Shotgun reloading"; "Playback", "Blending"), as
//! pure functions of a view model's animation set (`load::build_set`: one
//! sequence per filled first-person slot, named by the slot's label,
//! `sequence_name`) and what one 30 Hz tick brings (`Tick`: the weapon's
//! cues, the throttle, the look's change).
//!
//! Each tick (`HaloView::tick`), in the spec's order: advance the state
//! animation (a play-once animation that reported "ends next tick" on the
//! last tick leaves its state now: so it occupied exactly its frame count
//! in ticks, frames 0 .. N - 1), the moving overlay (starts above a
//! throttle of 0.1 on the ground, loops), the smoothed inputs (position,
//! turning, push back), the posing timer; then the weapon's cues of the
//! tick enter their states (`Cue`, `ViewState::slot`: a state whose slot
//! is empty is not entered; fire and misfire only from the states the
//! spec lists; ready is ignored in ready and becomes overheating-again on
//! an overheated weapon), each showing its frame 0 this tick. The pose:
//! the base at its frame, the moving overlay, the 9-frame `overlays`
//! animation's frames scaled by the smoothed inputs (frame 8 by the push
//! back), then the transition blend (0 ticks into overheated, melee,
//! ready and after a shotgun exit-empty, 3 into fire and misfire, 6
//! otherwise; moving stopping in idle blends 6).
//!
//! Known shortcuts:
//! - The scaled overlays' rotations are applied unnormalised as the spec
//!   says, then each final rotation is normalised (the engine's joints
//!   take unit quaternions).
//! - No light on/off, throw-grenade, charged, overcharged jitter or
//!   ammunition overlay (none of the four guns uses them, or no grenades
//!   yet); one permutation per slot (the slot's first).
//! - The posing timer's draws use a per-holder generator of our own, not
//!   the game's (posing is display only).

use bevy::math::{Quat, Vec2};
use blam_formats::animation::{FirstPersonSlot, smooth_step};

use super::super::body::anims::{LOOP_EVENT, base_pose};
use crate::map::anim::{AnimSet, BonePose, nlerp};

/// The event carrying a sequence's key frame (its number).
pub const KEY_EVENT: &str = "halo:key";

/// A sequence's kind rides in its activity: `base`, `overlay` or
/// `replacement` (animations.md, "Blending").
pub const BASE: &str = "base";
pub const OVERLAY: &str = "overlay";
pub const REPLACEMENT: &str = "replacement";

/// The globals' first-person idle numbers (globals.md, the disc's
/// values): the wait before posing, seconds, and the skip fraction.
pub const IDLE_TIME: (f32, f32) = (8.0, 12.0);
pub const IDLE_SKIP: f32 = 0.5;

/// A first-person slot's sequence name: its spec label (`fire-1`).
pub fn sequence_name(slot: FirstPersonSlot) -> &'static str {
    slot.label()
}

/// A state of the view (animations.md, "States"), each playing one slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ViewState {
    #[default]
    Idle,
    Posing,
    Fire,
    Misfire,
    Melee,
    ReloadEmpty,
    ReloadFull,
    Enter,
    ExitEmpty,
    ExitFull,
    PutAway,
    Ready,
    Overheating,
    OverheatingAgain,
    Overheated,
    OverheatedExit,
}

impl ViewState {
    /// The slot it plays.
    pub fn slot(self) -> FirstPersonSlot {
        use FirstPersonSlot as S;
        match self {
            ViewState::Idle => S::Idle,
            ViewState::Posing => S::Posing,
            ViewState::Fire => S::Fire1,
            ViewState::Misfire => S::Misfire1,
            ViewState::Melee => S::Melee,
            ViewState::ReloadEmpty => S::ReloadEmpty,
            ViewState::ReloadFull => S::ReloadFull,
            ViewState::Enter => S::Enter,
            ViewState::ExitEmpty => S::ExitEmpty,
            ViewState::ExitFull => S::ExitFull,
            ViewState::PutAway => S::PutAway,
            ViewState::Ready => S::Ready,
            ViewState::Overheating => S::Overheating,
            ViewState::OverheatingAgain => S::OverheatingAgain,
            ViewState::Overheated => S::Overheated,
            ViewState::OverheatedExit => S::OverheatedExit,
        }
    }

    /// The blend into it (ticks).
    fn blend_ticks(self) -> u32 {
        match self {
            ViewState::Overheated | ViewState::Melee | ViewState::Ready => 0,
            ViewState::Fire | ViewState::Misfire => 3,
            _ => 6,
        }
    }

    /// Whether fire and misfire may be entered from it.
    fn fires_from(self) -> bool {
        matches!(
            self,
            ViewState::Idle
                | ViewState::Posing
                | ViewState::Fire
                | ViewState::Enter
                | ViewState::ExitEmpty
                | ViewState::ExitFull
                | ViewState::OverheatedExit
                | ViewState::ReloadEmpty
                | ViewState::ReloadFull
        )
    }
}

/// What the weapon tells the view on a tick (from its `WeaponEvent`s:
/// `view::cue`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    /// Readied (a switch or a new weapon).
    Ready,
    /// Put away.
    PutAway,
    /// A primary round (the push back, and fire-1 where there is one).
    Fire,
    Misfire,
    Melee,
    /// A reload began with `loaded` rounds in and `needed` to load (the
    /// shotgun counts its shells).
    Reload {
        loaded: u32,
        needed: u32,
    },
    /// The shotgun's next shell.
    Shell,
    /// Overheated.
    Overheat,
    /// The overheat is over (`exit`: the weapon showed its exit).
    OverheatCleared,
}

/// One tick's inputs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tick {
    pub cues: Vec<Cue>,
    /// Forward and left over the run speed.
    pub throttle: Vec2,
    pub airborne: bool,
    /// The look's yaw (left +) and pitch (up +) change since the last
    /// tick, radians.
    pub turn: Vec2,
    pub zoomed: bool,
    /// The weapon is overheated, and about to leave it (its exit).
    pub overheated: bool,
    pub overheat_exit: bool,
}

/// A play-once animation's step (animations.md, "Playback").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Looped,
    Restarted,
    EndsNext,
    Running,
}

/// One advance of `frame` in an animation of `frames` frames, loop frame
/// `looping` (0: play once).
pub fn advance(frame: usize, frames: usize, looping: usize) -> (usize, Step) {
    let n = frames.max(1);
    let f = frame + 1;
    if f >= n {
        if looping > 0 {
            return (looping.min(n - 1), Step::Looped);
        }
        return (0, Step::Restarted);
    }
    if f + 1 == n && looping == 0 {
        return (f, Step::EndsNext);
    }
    (f, Step::Running)
}

/// A sequence's frame count, loop frame and key frame.
pub fn numbers(set: &AnimSet, s: usize) -> (usize, usize, usize) {
    let Some(q) = set.sequences.get(s) else {
        return (1, 0, 0);
    };
    let frames = q
        .anims
        .first()
        .and_then(|a| set.animations.get(*a))
        .map_or(1, |a| a.frames.max(1));
    let event = |name: &str| {
        q.events
            .iter()
            .find(|e| e.name == name)
            .map_or(0, |e| e.event.max(0) as usize)
    };
    (frames, event(LOOP_EVENT), event(KEY_EVENT))
}

/// A slot's sequence in `set`.
pub fn sequence(set: &AnimSet, slot: FirstPersonSlot) -> Option<usize> {
    set.names.get(sequence_name(slot)).copied()
}

/// The smoothing step's numbers (animations.md, "Smoothed inputs").
const POSITION: (f32, f32) = (0.08, 0.5);
const TURNING: (f32, f32) = (0.03, 0.2);
const PUSH: (f32, f32) = (0.01, 0.2);
/// What each primary shot adds to the push back's velocity.
pub const PUSH_PER_SHOT: f32 = 0.05;

/// A running transition blend.
#[derive(Clone, Debug, PartialEq)]
struct Blend {
    from: Vec<BonePose>,
    ticks: u32,
    index: u32,
}

/// One holder's first-person view (module doc).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HaloView {
    pub state: ViewState,
    /// The state's sequence and frame.
    pub sequence: Option<usize>,
    pub frame: usize,
    /// The state animation reported "ends next tick".
    pub ends: bool,
    /// The moving overlay's frame while it runs.
    pub moving: Option<usize>,
    /// Smoothed position (x forward, y left), turning, push back: value
    /// and velocity.
    pub position: [(f32, f32); 2],
    pub turning: [(f32, f32); 2],
    pub push: (f32, f32),
    /// The shotgun's reload: shells to load, loaded, the tube was empty.
    pub shells: Option<(u32, u32, bool)>,
    /// After a shotgun exit-empty (no blend out of it).
    after_exit_empty: bool,
    posing_counter: u32,
    posing_wait: u32,
    rng: u32,
    blend: Option<Blend>,
    last: Vec<BonePose>,
}

impl HaloView {
    /// A view whose posing draws come from `seed`.
    pub fn new(seed: u32) -> Self {
        let mut v = Self {
            rng: seed | 1,
            ..Default::default()
        };
        v.posing_wait = v.draw_wait();
        v
    }

    fn roll(&mut self) -> f32 {
        let mut x = self.rng.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    fn draw_wait(&mut self) -> u32 {
        let r = self.roll();
        ((IDLE_TIME.0 + (IDLE_TIME.1 - IDLE_TIME.0) * r) * 30.0) as u32
    }

    fn start_blend(&mut self, n: u32) {
        if let Some(b) = &self.blend
            && b.ticks.saturating_sub(b.index) > n
        {
            return;
        }
        if self.last.is_empty() || n == 0 {
            return;
        }
        self.blend = Some(Blend {
            from: self.last.clone(),
            ticks: n,
            index: 0,
        });
    }

    /// Enter `state` (module doc): false when its slot is empty.
    pub fn enter(&mut self, set: &AnimSet, state: ViewState) -> bool {
        let Some(s) = sequence(set, state.slot()) else {
            return false;
        };
        let blend = if self.after_exit_empty { 0 } else { state.blend_ticks() };
        self.after_exit_empty = state == ViewState::ExitEmpty;
        if state == ViewState::Ready {
            self.blend = None;
        }
        self.start_blend(blend);
        self.state = state;
        self.sequence = Some(s);
        self.frame = 0;
        self.ends = false;
        true
    }

    /// The state after a play-once one ended.
    fn next(&self) -> Option<ViewState> {
        use ViewState as V;
        Some(match self.state {
            V::Overheating | V::OverheatingAgain => V::Overheated,
            V::PutAway => return None,
            V::Enter | V::ReloadEmpty | V::ReloadFull => match self.shells {
                Some((needed, done, empty)) if done >= needed => {
                    if empty {
                        V::ExitEmpty
                    } else {
                        V::ExitFull
                    }
                }
                _ => V::Idle,
            },
            _ => V::Idle,
        })
    }

    /// A cue of the weapon (module doc).
    pub fn cue(&mut self, set: &AnimSet, cue: Cue, overheated: bool) {
        use ViewState as V;
        match cue {
            Cue::Ready if self.state == V::Ready && self.sequence.is_some() => {}
            Cue::Ready if overheated => {
                self.enter(set, V::OverheatingAgain);
            }
            Cue::Ready => {
                self.shells = None;
                self.enter(set, V::Ready);
            }
            Cue::PutAway => {
                self.enter(set, V::PutAway);
            }
            Cue::Fire => {
                self.push.1 += PUSH_PER_SHOT;
                if self.state.fires_from() {
                    self.enter(set, V::Fire);
                }
            }
            Cue::Misfire if self.state.fires_from() => {
                self.enter(set, V::Misfire);
            }
            Cue::Misfire => {}
            Cue::Melee => {
                self.enter(set, V::Melee);
            }
            Cue::Reload { loaded, needed } => {
                if sequence(set, FirstPersonSlot::Enter).is_some() {
                    self.shells = Some((needed.max(1), 1, loaded == 0));
                    self.enter(set, V::Enter);
                } else {
                    self.shells = None;
                    let full = loaded > 0 && self.enter(set, V::ReloadFull);
                    if !full {
                        self.enter(set, V::ReloadEmpty);
                    }
                }
            }
            Cue::Shell => {
                if let Some(s) = &mut self.shells {
                    s.1 += 1;
                }
                self.enter(set, V::ReloadEmpty);
            }
            Cue::Overheat => {
                self.enter(set, V::Overheating);
            }
            Cue::OverheatCleared => {
                if matches!(self.state, V::Overheated | V::Overheating | V::OverheatingAgain) {
                    self.enter(set, V::Idle);
                }
            }
        }
    }

    /// One 30 Hz tick (module doc): the pose to draw.
    pub fn tick(&mut self, set: &AnimSet, t: &Tick) -> Vec<BonePose> {
        // The overheat's exit, when the weapon shows it.
        if t.overheat_exit && self.state == ViewState::Overheated {
            self.enter(set, ViewState::OverheatedExit);
        }
        // The state animation.
        if let Some(s) = self.sequence {
            if self.ends {
                self.ends = false;
                match self.next() {
                    // Idle again: the same animation from its start.
                    Some(n) if n == self.state => self.frame = 0,
                    Some(n) => {
                        if !self.enter(set, n) && n != ViewState::Idle {
                            self.enter(set, ViewState::Idle);
                        }
                    }
                    // Put away holds its last frame.
                    None => self.ends = true,
                }
            } else {
                let (frames, looping, _) = numbers(set, s);
                let (f, step) = advance(self.frame, frames, looping);
                self.frame = f;
                if step == Step::EndsNext || (step == Step::Restarted && self.state != ViewState::Idle) {
                    self.ends = step == Step::EndsNext;
                    if step == Step::Restarted {
                        // A one-frame animation: it ended at once.
                        self.frame = frames - 1;
                        self.ends = true;
                    }
                }
            }
        }
        // The moving overlay.
        if t.throttle.length() > 0.1 && !t.airborne {
            self.moving = Some(match (self.moving, sequence(set, FirstPersonSlot::Moving)) {
                (Some(f), Some(m)) => (f + 1) % numbers(set, m).0,
                _ => 0,
            });
        } else if self.moving.take().is_some() && self.state == ViewState::Idle {
            self.start_blend(6);
        }
        // Smoothed inputs and the push back.
        let target = [t.throttle.x.clamp(-1.0, 1.0), t.throttle.y.clamp(-1.0, 1.0)];
        for (p, x) in self.position.iter_mut().zip(target) {
            *p = smooth_step(p.0, p.1, x, POSITION.0, POSITION.1, -1.0, 1.0);
        }
        let turn = [(t.turn.x * 30.0).clamp(-1.0, 1.0), (t.turn.y * -30.0).clamp(-1.0, 1.0)];
        for (p, x) in self.turning.iter_mut().zip(turn) {
            *p = smooth_step(p.0, p.1, x, TURNING.0, TURNING.1, -1.0, 1.0);
        }
        self.push = smooth_step(self.push.0, self.push.1, 0.0, PUSH.0, PUSH.1, 0.0, 1.0);
        if self.push.0 == 1.0 {
            self.push.1 = 0.0;
        }
        // Posing: a wait while nothing moves the view.
        let still = !t.zoomed
            && self
                .position
                .iter()
                .chain(&self.turning)
                .all(|p| p.0 == 0.0 && p.1 == 0.0)
            && self.push == (0.0, 0.0);
        match self.state {
            ViewState::Idle if still => {
                self.posing_counter += 1;
                if self.posing_counter > self.posing_wait {
                    self.posing_counter = 0;
                    self.posing_wait = self.draw_wait();
                    if self.roll() >= IDLE_SKIP {
                        self.enter(set, ViewState::Posing);
                    }
                }
            }
            ViewState::Posing if !still => {
                self.enter(set, ViewState::Idle);
            }
            _ => self.posing_counter = 0,
        }
        // The weapon's cues of this tick.
        for c in &t.cues {
            self.cue(set, *c, t.overheated);
        }
        self.pose(set)
    }

    /// The pose of the current state (module doc).
    pub fn pose(&mut self, set: &AnimSet) -> Vec<BonePose> {
        let mut pose = match self.sequence.and_then(|s| set.sequences.get(s)) {
            Some(q) => base_pose(set, q.anims[0], self.frame),
            None => set.defaults.clone(),
        };
        if let (Some(f), Some(m)) = (self.moving, sequence(set, FirstPersonSlot::Moving)) {
            overlay(set, m, f, 1.0, &mut pose);
        }
        if let Some(o) = sequence(set, FirstPersonSlot::Overlays)
            && numbers(set, o).0 >= 9
        {
            let (px, py) = (self.position[0].0, self.position[1].0);
            let (tx, ty) = (self.turning[0].0, self.turning[1].0);
            let weights = [
                (0, px),
                (1, -px),
                (3, py),
                (2, -py),
                (4, tx),
                (5, -tx),
                (7, ty),
                (6, -ty),
                (8, self.push.0),
            ];
            for (frame, w) in weights {
                if w > 0.0 {
                    overlay(set, o, frame, w.min(1.0), &mut pose);
                }
            }
        }
        for (q, _) in &mut pose {
            *q = q.normalize();
        }
        if let Some(b) = &mut self.blend {
            let u = (b.index + 1) as f32 / b.ticks as f32;
            for ((q, p), (fq, fp)) in pose.iter_mut().zip(&b.from) {
                *q = nlerp(*fq, *q, u);
                *p = fp.lerp(*p, u);
            }
            b.index += 1;
            if b.index >= b.ticks {
                self.blend = None;
            }
        }
        self.last = pose.clone();
        pose
    }
}

/// Sequence `s`'s animation at `frame` applied onto `pose` as a scaled
/// overlay of weight `w` (animations.md, "Blending": the rotation
/// (1 - w) e + w q_o, sign-corrected, on the right in the usual
/// convention; the translation added times w).
pub fn overlay(set: &AnimSet, s: usize, frame: usize, w: f32, pose: &mut [BonePose]) {
    let Some(anim) = set
        .sequences
        .get(s)
        .and_then(|q| q.anims.first())
        .and_then(|a| set.animations.get(*a))
    else {
        return;
    };
    let at = |n: usize| frame.min(n.saturating_sub(1));
    for t in &anim.tracks {
        let Some(bone) = pose.get_mut(t.bone) else { continue };
        if let Some(r) = t.rotation.as_deref().filter(|r| !r.is_empty()) {
            let mut q = r[at(r.len())];
            if q.w < 0.0 {
                q = -q;
            }
            let scaled = Quat::from_xyzw(w * q.x, w * q.y, w * q.z, (1.0 - w) + w * q.w);
            bone.0 *= scaled;
        }
        if let Some(v) = t.position.as_deref().filter(|v| !v.is_empty()) {
            bone.1 += v[at(v.len())] * w;
        }
    }
}
