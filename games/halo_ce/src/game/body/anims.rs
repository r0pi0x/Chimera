//! The Spartan's third-person animation (specs/halo_ce/animations.md,
//! "Units", "Bipeds", "Playback", "Blending", "Aiming and looking
//! screens"), in two halves.
//!
//! **At load** (`build`): the graph's unit tables become the engine's
//! `AnimSet`, so the body's animations travel with the character model
//! (the converted-map cache, a guest's dressing, a dedicated server) and
//! the simulation needs no graph at run time. For each seat label
//! (`SEATS`: stand, crouch) and weapon label (`LABELS`: unarmed and the
//! multiplayer weapons' labels) the spec's selection rule picks the (seat,
//! weapon class, weapon type) triple (`AnimationGraph::select_unit`: the
//! last match in block order wins), and each state's slot (`State`) of that
//! class becomes a sequence named `"<seat> <label> <slot>"` (`key`) whose
//! one animation is the slot's. The aim screens (`aim-still`, `aim-move`)
//! and the seat's `look` screen are sequences too: their grid is the
//! screen's columns and rows, and their two axes' keys carry the spec's
//! screen bounds (yaw: right and left radians per frame, right and left
//! counts; pitch: down, up, down count, up count), from which the spec's
//! `ScreenBounds::locate` finds the cell at run time. A sequence's loop
//! frame rides in an event named `LOOP_EVENT` (its number is the frame).
//! Each animation used is sampled at every integer frame (the engine never
//! interpolates between frames) into tracks in the usual quaternion
//! convention and the skeleton's units: a base animation tracks every
//! node, an overlay only the parts whose bit is set (so "parts without a
//! bit are untouched" holds). A base animation that does not fit the model
//! (node count, checksum) is left out (the spec's bind pose for it).
//! Sequence 0 is the bind pose (`BIND`), what the engine's `Animator`
//! names as playing while the game lays the pose out itself.
//!
//! **Each tick** (`HaloAnim::tick`, one call per 30 Hz tick): the desired
//! state from the movement (more than 3 ticks in the air: airborne; a
//! landing faster than the biped's soft or hard minimum: land-soft or
//! land-hard while its recovery lasts; else the throttle's direction:
//! front or back when |forward| >= |left|, else left or right, idle below
//! 0.1), the seat from crouching, the weapon label from the holder; a
//! changed state starts its animation at frame 0 with a 6-tick blend (2
//! into a landing), else the animation advances one frame (looping to its
//! loop frame or restarting); then the pose: the base at its frame, the
//! aim screen (`aim-still` standing, airborne and landing; `aim-move`
//! moving; a change of screen blends over 6 ticks) at the aim's yaw and
//! pitch, the seat's look screen at the look minus the aim, and the
//! running transition blend over all of it. Overlay rotations compose on
//! the right in the usual convention (q x q_o), translations add.
//!
//! Known shortcuts:
//! - The throttle is the horizontal velocity over the run speed in the
//!   body's frame (the body always faces the look: no turn-in-place
//!   states, no feet lag), so a remote player's body follows what it does,
//!   not its stick.
//! - One permutation per slot (the chain's first); the stunned-move,
//!   slide, gesture, turn, melee, throw and action layers (reload, fire,
//!   ready), pings, death animations, acceleration overlays and IK are not
//!   played; node scale is ignored (1 on the disc).

use std::collections::BTreeMap;

use bevy::math::{Quat, Vec2, Vec3};
use blam_formats::animation::{AnimationGraph, AnimationType, ScreenBounds, UnitSlot, WeaponClassSlot};

use super::skeleton::{bind_locals, usual};
use crate::map::{
    MapBone,
    anim::{AnimEvent, AnimSet, Animation, Axis, BonePose, Sequence, Track, nlerp},
};

/// The seat labels a player's body uses (unseated: standing or crouching).
pub const SEATS: [&str; 2] = ["stand", "crouch"];

/// The weapon labels tables are built for: unarmed and the multiplayer
/// weapons' (animations.md, "Seat, weapon class and weapon type").
pub const LABELS: [&str; 12] = [
    "unarmed", "ar", "hp", "sg", "pr", "rl", "sr", "pp", "ft", "ne", "b", "f",
];

/// The label a holder without a label of its own plays: a rifle stance
/// with a weapon, none without (`body::WeaponLabel` overrides it).
pub const ARMED_DEFAULT: &str = "ar";

/// The bind pose's sequence (index 0).
pub const BIND: &str = "bind";

/// The event that carries a sequence's loop frame.
pub const LOOP_EVENT: &str = "halo:loop";

/// Ticks of a state change's blend, of one into a landing, of a change of
/// aim screen.
pub const TRANSITION_TICKS: u32 = 6;
pub const LAND_TRANSITION_TICKS: u32 = 2;

/// Ticks in the air after which the desired state is airborne.
pub const AIRBORNE_AFTER: u32 = 3;

/// A throttle below this magnitude counts as zero.
pub const THROTTLE_DEAD: f32 = 0.1;

/// A base state of the body (animations.md, "State to animation").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum State {
    #[default]
    Idle,
    MoveFront,
    MoveBack,
    MoveLeft,
    MoveRight,
    Airborne,
    LandSoft,
    LandHard,
}

impl State {
    pub const ALL: [State; 8] = [
        State::Idle,
        State::MoveFront,
        State::MoveBack,
        State::MoveLeft,
        State::MoveRight,
        State::Airborne,
        State::LandSoft,
        State::LandHard,
    ];

    /// The weapon class slot the state reads.
    pub fn slot(self) -> WeaponClassSlot {
        match self {
            State::Idle => WeaponClassSlot::Idle,
            State::MoveFront => WeaponClassSlot::MoveFront,
            State::MoveBack => WeaponClassSlot::MoveBack,
            State::MoveLeft => WeaponClassSlot::MoveLeft,
            State::MoveRight => WeaponClassSlot::MoveRight,
            State::Airborne => WeaponClassSlot::Airborne,
            State::LandSoft => WeaponClassSlot::LandSoft,
            State::LandHard => WeaponClassSlot::LandHard,
        }
    }

    /// Its slot's name (`move-front`).
    pub fn name(self) -> &'static str {
        match self {
            State::Idle => "idle",
            State::MoveFront => "move-front",
            State::MoveBack => "move-back",
            State::MoveLeft => "move-left",
            State::MoveRight => "move-right",
            State::Airborne => "airborne",
            State::LandSoft => "land-soft",
            State::LandHard => "land-hard",
        }
    }

    /// The aim screen played in it: `aim-move` moving, `aim-still` else.
    pub fn aim(self) -> &'static str {
        match self {
            State::MoveFront | State::MoveBack | State::MoveLeft | State::MoveRight => "aim-move",
            _ => "aim-still",
        }
    }

    /// The blend's length into this state.
    pub fn transition_ticks(self) -> u32 {
        match self {
            State::LandSoft | State::LandHard => LAND_TRANSITION_TICKS,
            _ => TRANSITION_TICKS,
        }
    }
}

/// A table sequence's name.
pub fn key(seat: &str, label: &str, slot: &str) -> String {
    format!("{seat} {label} {slot}").to_ascii_lowercase()
}

/// A seat's look screen's name.
pub fn look_key(seat: &str) -> String {
    format!("{seat} look").to_ascii_lowercase()
}

/// The movement state from a throttle (forward, left), animations.md
/// "Bipeds": zero below `THROTTLE_DEAD` gives idle.
pub fn move_state(throttle: Vec2) -> State {
    if throttle.length() < THROTTLE_DEAD {
        return State::Idle;
    }
    let (forward, left) = (throttle.x, throttle.y);
    if forward.abs() >= left.abs() {
        if forward >= 0.0 {
            State::MoveFront
        } else {
            State::MoveBack
        }
    } else if left >= 0.0 {
        State::MoveLeft
    } else {
        State::MoveRight
    }
}

/// One stored animation sampled into the engine's form (module doc), or
/// why not.
pub fn convert(graph: &AnimationGraph, index: usize, bones: usize, checksum: u32) -> Result<Animation, String> {
    let a = graph.animation(index).ok_or_else(|| format!("no animation {index}"))?;
    if a.kind == AnimationType::Base && !a.fits_model(bones, checksum) {
        return Err(format!("`{}` does not fit the model", a.name));
    }
    let nodes = a.node_count().min(bones);
    if a.node_count() > bones {
        return Err(format!("`{}` has more nodes than the model", a.name));
    }
    let frames = (a.frame_count.max(1)) as usize;
    let base = a.kind == AnimationType::Base;
    let mut rot: Vec<Vec<Quat>> = vec![Vec::with_capacity(frames); nodes];
    let mut pos: Vec<Vec<Vec3>> = vec![Vec::with_capacity(frames); nodes];
    for f in 0..frames {
        let pose = a.sample(f).map_err(|e| format!("`{}` frame {f}: {e}", a.name))?;
        for n in 0..nodes {
            let Some(p) = pose.nodes.get(n) else { break };
            rot[n].push(usual(p.rotation));
            pos[n].push(Vec3::from(p.translation));
        }
    }
    let tracks = (0..nodes)
        .map(|n| Track {
            bone: n,
            rotation: keep(a.has_rotation(n), base, std::mem::take(&mut rot[n])),
            position: keep(a.has_translation(n), base, std::mem::take(&mut pos[n])),
        })
        .collect();
    Ok(Animation {
        name: a.name.clone(),
        fps: 30.0,
        frames,
        delta: !base,
        base: 0,
        tracks,
        ..Default::default()
    })
}

/// A node's per-frame values as a track: all of them with the part's bit,
/// the first alone (constant) for a base animation without it, none for an
/// overlay without it.
fn keep<T>(bit: bool, base: bool, mut v: Vec<T>) -> Option<Vec<T>> {
    match (bit, base) {
        (true, _) => Some(v),
        (false, true) => {
            v.truncate(1);
            Some(v)
        }
        (false, false) => None,
    }
}

/// The bounds as the two axes' keys (module doc).
fn screen_axes(b: &ScreenBounds) -> [Axis; 2] {
    [
        Axis {
            param: None,
            keys: vec![
                b.right_yaw_per_frame,
                b.left_yaw_per_frame,
                f32::from(b.right_frame_count),
                f32::from(b.left_frame_count),
            ],
            range: (0.0, 0.0),
        },
        Axis {
            param: None,
            keys: vec![
                b.down_pitch_per_frame,
                b.up_pitch_per_frame,
                f32::from(b.down_frame_count),
                f32::from(b.up_frame_count),
            ],
            range: (0.0, 0.0),
        },
    ]
}

/// A screen sequence's bounds back (None: not a screen).
pub fn screen_bounds(s: &Sequence) -> Option<ScreenBounds> {
    let (y, p) = (&s.axes[0].keys, &s.axes[1].keys);
    if y.len() != 4 || p.len() != 4 {
        return None;
    }
    Some(ScreenBounds {
        right_yaw_per_frame: y[0],
        left_yaw_per_frame: y[1],
        right_frame_count: y[2] as i16,
        left_frame_count: y[3] as i16,
        down_pitch_per_frame: p[0],
        up_pitch_per_frame: p[1],
        down_frame_count: p[2] as i16,
        up_frame_count: p[3] as i16,
    })
}

/// Builds the set (module doc).
struct Builder<'a> {
    graph: &'a AnimationGraph,
    bones: usize,
    checksum: u32,
    set: AnimSet,
    converted: BTreeMap<usize, Option<usize>>,
    warnings: Vec<String>,
}

impl Builder<'_> {
    fn animation(&mut self, index: usize) -> Option<usize> {
        if let Some(c) = self.converted.get(&index) {
            return *c;
        }
        let made = match convert(self.graph, index, self.bones, self.checksum) {
            Ok(a) => {
                self.set.animations.push(a);
                Some(self.set.animations.len() - 1)
            }
            Err(e) => {
                self.warnings.push(format!("animation left out: {e}"));
                None
            }
        };
        self.converted.insert(index, made);
        made
    }

    fn sequence(&mut self, name: String, index: usize, screen: Option<&ScreenBounds>) {
        if self.set.names.contains_key(&name) {
            return;
        }
        let Some(a) = self.animation(index) else { return };
        let stored = &self.graph.animations[index];
        let mut s = Sequence {
            name: name.clone(),
            looping: true,
            anims: vec![a],
            grid: (1, 1),
            events: vec![AnimEvent {
                cycle: 0.0,
                event: i32::from(stored.loop_frame),
                name: LOOP_EVENT.into(),
                options: String::new(),
            }],
            ..Default::default()
        };
        if let Some(b) = screen {
            s.grid = (b.columns(), b.rows());
            s.axes = screen_axes(b);
            s.delta = true;
        }
        self.set.sequences.push(s);
        self.set.names.insert(name, self.set.sequences.len() - 1);
    }
}

/// The body's animation set from its graph (module doc), for a model of
/// `bones` (its node list checksum `checksum`), and what was left out.
pub fn build(graph: &AnimationGraph, bones: &[MapBone], checksum: u32) -> (AnimSet, Vec<String>) {
    let defaults = bind_locals(bones);
    let mut b = Builder {
        graph,
        bones: bones.len(),
        checksum,
        set: AnimSet {
            defaults: defaults.clone(),
            bases: vec![defaults],
            parents: bones.iter().map(|b| b.parent).collect(),
            bone_names: bones.iter().map(|b| b.name.to_ascii_lowercase()).collect(),
            ..Default::default()
        },
        converted: BTreeMap::new(),
        warnings: Vec::new(),
    };
    // Sequence 0: the bind pose (one frame, no tracks).
    b.set.animations.push(Animation {
        name: BIND.into(),
        fps: 30.0,
        frames: 1,
        ..Default::default()
    });
    b.set.sequences.push(Sequence {
        name: BIND.into(),
        looping: true,
        anims: vec![0],
        grid: (1, 1),
        ..Default::default()
    });
    b.set.names.insert(BIND.into(), 0);
    for seat in SEATS {
        for label in LABELS {
            let Some(sel) = graph.select_unit(seat, label) else {
                continue;
            };
            let (Some(unit), Some(class)) = (graph.seat(sel), graph.weapon_class(sel)) else {
                continue;
            };
            for state in State::ALL {
                if let Some(i) = class.animations.get(state.slot()) {
                    b.sequence(key(seat, label, state.name()), i, None);
                }
            }
            for (slot, name) in [
                (WeaponClassSlot::AimStill, "aim-still"),
                (WeaponClassSlot::AimMove, "aim-move"),
            ] {
                if let Some(i) = class.animations.get(slot) {
                    b.sequence(key(seat, label, name), i, Some(&class.aiming_bounds));
                }
            }
            if let Some(i) = unit.animations.get(UnitSlot::Look) {
                b.sequence(look_key(seat), i, Some(&unit.looking_bounds));
            }
        }
    }
    (b.set, b.warnings)
}

/// A track's value at frame `f` (the last one past its end).
fn at<T: Copy>(v: &[T], f: usize) -> Option<T> {
    v.get(f.min(v.len().checked_sub(1)?)).copied()
}

/// The base pose: the defaults, then animation `a` at frame `f`.
pub fn base_pose(set: &AnimSet, a: usize, f: usize) -> Vec<BonePose> {
    let mut pose = set.defaults.clone();
    let Some(anim) = set.animations.get(a) else { return pose };
    for t in &anim.tracks {
        let Some(bone) = pose.get_mut(t.bone) else { continue };
        if let Some(q) = t.rotation.as_deref().and_then(|r| at(r, f)) {
            bone.0 = q;
        }
        if let Some(p) = t.position.as_deref().and_then(|r| at(r, f)) {
            bone.1 = p;
        }
    }
    pose
}

/// A screen sequence `s` applied at `yaw` and `pitch` (radians, relative)
/// onto `pose` (animations.md, "Aiming and looking screens"): nothing when
/// it is no screen, its animation is short of the grid, or the cell is
/// outside it.
pub fn apply_screen(set: &AnimSet, s: usize, yaw: f32, pitch: f32, pose: &mut [BonePose]) {
    let Some(seq) = set.sequences.get(s) else { return };
    let (Some(bounds), Some(anim)) = (
        screen_bounds(seq),
        seq.anims.first().and_then(|a| set.animations.get(*a)),
    ) else {
        return;
    };
    if anim.frames < bounds.columns() * bounds.rows() {
        return;
    }
    let Some(cell) = bounds.locate(yaw, pitch) else { return };
    let [f00, f10, f01, f11] = cell.frames;
    let (u, p) = (cell.u, cell.p);
    for t in &anim.tracks {
        let Some(bone) = pose.get_mut(t.bone) else { continue };
        if let Some(r) = t.rotation.as_deref()
            && let (Some(a), Some(b), Some(c), Some(d)) = (at(r, f00), at(r, f10), at(r, f01), at(r, f11))
        {
            let q = nlerp(nlerp(a, b, u), nlerp(c, d, u), p);
            bone.0 = (bone.0 * q).normalize();
        }
        if let Some(v) = t.position.as_deref()
            && let (Some(a), Some(b), Some(c), Some(d)) = (at(v, f00), at(v, f10), at(v, f01), at(v, f11))
        {
            bone.1 += a * (1.0 - u) * (1.0 - p) + b * u * (1.0 - p) + c * (1.0 - u) * p + d * u * p;
        }
    }
}

/// A sequence's loop frame (0: play once).
pub fn loop_frame(set: &AnimSet, s: usize) -> usize {
    set.sequences
        .get(s)
        .and_then(|q| q.events.iter().find(|e| e.name == LOOP_EVENT))
        .map_or(0, |e| e.event.max(0) as usize)
}

/// A sequence's frame count.
pub fn frames(set: &AnimSet, s: usize) -> usize {
    set.sequences
        .get(s)
        .and_then(|q| q.anims.first())
        .and_then(|a| set.animations.get(*a))
        .map_or(1, |a| a.frames.max(1))
}

/// One advance of a looping state's frame (animations.md, "Playback"):
/// past the end it goes to the loop frame (held at the last frame when
/// that is past the end), or restarts at 0 when there is none.
pub fn advance(frame: usize, frames: usize, loop_frame: usize) -> usize {
    let f = frame + 1;
    if f < frames {
        f
    } else if loop_frame > 0 {
        loop_frame.min(frames - 1)
    } else {
        0
    }
}

/// What the body reads from the simulation each tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Inputs {
    pub grounded: bool,
    pub crouching: bool,
    /// Forward and left over the run speed (body frame).
    pub throttle: Vec2,
    /// Vertical speed, world units per second (up +).
    pub vertical: f32,
    /// The aim's pitch, radians (up +); the aim's yaw relative to the body
    /// is 0 (the body faces the look).
    pub pitch: f32,
}

/// The landing thresholds (world units per second) and recovery ticks
/// (the biped's, movement.md).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Landing {
    pub soft_min: f32,
    pub hard_min: f32,
    pub soft_ticks: u32,
    pub hard_ticks: u32,
}

/// A running transition blend.
#[derive(Clone, Debug, PartialEq)]
pub struct Blend {
    pub from: Vec<BonePose>,
    pub ticks: u32,
    pub index: u32,
}

/// One body's animation state (module doc).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HaloAnim {
    pub state: State,
    /// The base sequence playing (None: none yet).
    pub sequence: Option<usize>,
    pub frame: usize,
    pub aim: Option<usize>,
    pub air_ticks: u32,
    /// Downward speed while in the air (world units per second).
    pub falling: f32,
    /// Landing recovery ticks left, and whether hard.
    pub landing: u32,
    pub hard: bool,
    pub blend: Option<Blend>,
    /// The last pose made (a new blend's "from").
    pub last: Vec<BonePose>,
}

impl HaloAnim {
    /// Start a blend of `n` ticks from the last pose (a running one with
    /// more ticks left is kept).
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

    /// The state the body wants (module doc), updating the air and
    /// landing counters.
    pub fn desire(&mut self, i: &Inputs, land: &Landing) -> State {
        if i.grounded {
            if self.air_ticks > AIRBORNE_AFTER {
                if self.falling >= land.hard_min {
                    self.landing = land.hard_ticks;
                    self.hard = true;
                } else if self.falling >= land.soft_min {
                    self.landing = land.soft_ticks;
                    self.hard = false;
                }
            }
            self.air_ticks = 0;
            self.falling = 0.0;
        } else {
            self.air_ticks += 1;
            self.falling = self.falling.max(-i.vertical);
            self.landing = 0;
        }
        if self.air_ticks > AIRBORNE_AFTER {
            State::Airborne
        } else if self.landing > 0 {
            self.landing -= 1;
            if self.hard { State::LandHard } else { State::LandSoft }
        } else if !i.grounded {
            // The first ticks in the air keep the ground's state.
            self.state
        } else {
            move_state(i.throttle)
        }
    }

    /// One 30 Hz tick: the state, its frame and the pose (module doc).
    pub fn tick(&mut self, set: &AnimSet, label: &str, i: &Inputs, land: &Landing) -> Vec<BonePose> {
        let want = self.desire(i, land);
        let seat = if i.crouching { SEATS[1] } else { SEATS[0] };
        let found = set.names.get(&key(seat, label, want.name())).copied();
        let changed_state = want != self.state;
        self.state = want;
        match found {
            Some(s) if Some(s) != self.sequence || (changed_state && self.sequence.is_some()) => {
                self.start_blend(want.transition_ticks());
                self.sequence = Some(s);
                self.frame = 0;
            }
            _ => {
                if let Some(s) = self.sequence {
                    self.frame = advance(self.frame, frames(set, s), loop_frame(set, s));
                }
            }
        }
        let aim = set.names.get(&key(seat, label, want.aim())).copied();
        if aim != self.aim {
            if self.aim.is_some() {
                self.start_blend(TRANSITION_TICKS);
            }
            self.aim = aim;
        }
        let mut pose = match self.sequence {
            Some(s) => base_pose(set, set.sequences[s].anims[0], self.frame),
            None => set.defaults.clone(),
        };
        if let Some(a) = self.aim {
            apply_screen(set, a, 0.0, i.pitch, &mut pose);
        }
        if let Some(l) = set.names.get(&look_key(seat)) {
            apply_screen(set, *l, 0.0, 0.0, &mut pose);
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
