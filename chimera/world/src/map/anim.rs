//! Skeletal animation of character bodies: a set of sampled animations,
//! sequences that blend them by pose parameters, and the layering that
//! builds a body's pose (specs/cs_source/animation.md §3–§10). Games load
//! an `AnimSet` and drive an `Animator`; this module only does the math
//! and writes the pose onto a body's joints.
//!
//! Poses are in the skeleton's own axes and units (as `MapBone`).
//!
//! Skeletal animation for any game: `AnimSet` (sampled animations, sequences on
//! pose-parameter grids, autolayers with linear or spline ramps on the
//! parent's cycle or a pose parameter, IK chains, the sequences' IK lock
//! rules and IK locks), the blend/layer math (a layer's IK-locked chain
//! ends put back where the pose underneath had them when the animator
//! asks, `Animator::ik_locks`, the middle joint bent toward the chain's
//! knee direction where it has one: TF2's feet under aim matrices,
//! specs/tf2/player_animation.md 4a, 5a; real-time autolayers on the
//! clock, `Sequence::realtime`; a request to snap a body's feet onto its
//! eyes, `SnapFeet`; the animations' IK rules' stored
//! places, a chain's end held relative to a bone, blended by the grid's
//! cell weights and folded through the layers, then solved on the whole
//! pose when the animator asks, `Animator::ik_rules`: TF2's Heavy's left
//! hand on the Minigun, 4a), two-bone IK (`solve_ik`: a chain's end onto a
//! bone, e.g. hands onto a weapon's grips), and the `Animator` component that
//! `pose_bodies` turns into body joint transforms; games drive it in
//! `DriveAnimation`.

use std::{collections::HashMap, sync::Arc};

use bevy::prelude::*;

/// One bone's local rotation and position.
pub type BonePose = (Quat, Vec3);

/// Everything one skeleton can play. Bone indices are the target
/// skeleton's (`MapCharacterModel::bones`).
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct AnimSet {
    /// The target skeleton's own default pose.
    pub defaults: Vec<BonePose>,
    /// Base poses of the models the animations came from, mapped onto the
    /// target's bones (`Animation::base`).
    pub bases: Vec<Vec<BonePose>>,
    pub animations: Vec<Animation>,
    pub sequences: Vec<Sequence>,
    pub params: Vec<PoseParam>,
    /// Sequence index by lower-case name (filled by loaders; optional).
    /// Serialized in name order, so a cached map's bytes are the same
    /// each time (`cache`).
    #[serde(serialize_with = "crate::map::pod_serde::sorted")]
    pub names: HashMap<String, usize>,
    /// The target skeleton's bone parents (empty: unknown; inverse
    /// kinematics then does nothing).
    pub parents: Vec<Option<usize>>,
    /// The target skeleton's bone names (lower case), as `parents`.
    pub bone_names: Vec<String>,
    /// Two-bone chains inverse kinematics can bend (`IkGoal`).
    pub ik_chains: Vec<IkChain>,
}

/// A request on a character: its game's animation state puts the body's
/// feet (its render yaw) on the eyes' yaw plus this many degrees
/// (counter-clockwise) at its next update, then takes this off. TF2's
/// `setang_exact` "snap yaw" (specs/tf2/player_animation.md 4, 4c 5);
/// scenes line their targets' feet up with a reference's
/// (`target set <n> feet <deg>`). A game without a feet yaw ignores it.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct SnapFeet(pub f32);

/// A limb inverse kinematics can bend: root (shoulder), middle (elbow)
/// and end (hand) bones, target skeleton indices.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct IkChain {
    /// Lower-case name (games pick chains by it).
    pub name: String,
    pub bones: [usize; 3],
    /// The direction the middle joint (knee) bends toward, in the root
    /// bone's frame (zero: none; then a lock keeps the plane the chain
    /// bends in). specs/tf2/player_animation.md 4a.
    #[serde(default)]
    pub knee: Vec3,
}

/// What a sequence's animation says about an IK chain while it plays:
/// lock the chain's end to where the animation put it relative to a bone,
/// or release it. Games decide what a lock aims at (`Animator::ik`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IkRule {
    /// `AnimSet::ik_chains` index.
    pub chain: usize,
    pub lock: bool,
}

/// One chain's end pulled onto a bone's place and turn (both model space,
/// from this frame's pose) by `weight`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IkGoal {
    pub chain: usize,
    pub target: usize,
    pub weight: f32,
}

/// Sampled frames of one animation.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Animation {
    pub name: String,
    pub fps: f32,
    pub frames: usize,
    /// Additive: bones start at identity and tracks are offsets.
    pub delta: bool,
    /// Which `AnimSet::bases` entry untracked bones start from (non-delta).
    pub base: usize,
    /// How fast its authored motion moves the character (skeleton units
    /// per second; 0: in place), for matching playback to real speed.
    pub speed: f32,
    /// Its authored horizontal velocity (skeleton units per second, the
    /// skeleton's X and Y): `speed` with its direction, for blending
    /// directions (`ground_motion`).
    #[serde(default)]
    pub motion: Vec2,
    pub tracks: Vec<Track>,
    /// Its IK rules' stored places: where a chain's end sits relative to a
    /// bone in each frame (Source's IK rules with stored data; TF2's
    /// Heavy's aim cells hold the left hand on the Minigun,
    /// specs/tf2/player_animation.md 4a). Used when the animator asks
    /// (`Animator::ik_rules`).
    #[serde(default)]
    pub ik_targets: Vec<IkTarget>,
}

/// An animation's IK rule with stored places: chain `chain`'s end held at
/// `frames[f]` (rotation, position) in bone `bone`'s frame, weighted by
/// `ramp` (start, peak, tail, end) on the cycle (all equal: always 1).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct IkTarget {
    /// `AnimSet::ik_chains` index.
    pub chain: usize,
    /// Target bone index.
    pub bone: usize,
    pub ramp: [f32; 4],
    pub frames: Vec<BonePose>,
}

impl IkTarget {
    /// The rule's weight at `cycle` from its ramp.
    pub fn weight(&self, cycle: f32) -> f32 {
        let [start, peak, tail, end] = self.ramp;
        if start == end {
            return 1.0;
        }
        if cycle < start || cycle > end {
            0.0
        } else if cycle < peak {
            (cycle - start) / (peak - start)
        } else if cycle > tail {
            (end - cycle) / (end - tail)
        } else {
            1.0
        }
    }

    /// The stored place at `cycle` (frames interpolated).
    pub fn at(&self, cycle: f32) -> Option<BonePose> {
        let n = self.frames.len();
        let first = *self.frames.first()?;
        if n == 1 {
            return Some(first);
        }
        let f = cycle.clamp(0.0, 1.0) * (n - 1) as f32;
        let i = (f as usize).min(n - 2);
        let t = f - i as f32;
        let (a, b) = (self.frames[i], self.frames[i + 1]);
        Some((nlerp(a.0, b.0, t), a.1.lerp(b.1, t)))
    }
}

/// One chain's goal from the IK rules so far (`AnimSet::rule_goals`): its
/// end's place in the target bone's frame and how strongly it is held.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuleGoal {
    pub bone: usize,
    pub place: BonePose,
    pub weight: f32,
}

/// One bone's values per frame (one entry: constant); None keeps the
/// starting value.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Track {
    pub bone: usize,
    pub rotation: Option<Vec<Quat>>,
    pub position: Option<Vec<Vec3>>,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Sequence {
    pub name: String,
    pub activity: String,
    /// Selection weight among sequences with the same activity (its
    /// absolute value counts).
    pub activity_weight: i32,
    pub looping: bool,
    /// No cross-fade into it.
    pub snap: bool,
    pub delta: bool,
    /// With `delta`: applied in the bone's frame after the base.
    pub post: bool,
    /// Played on every body each frame whatever the main sequence
    /// (Source's autoplay flag), driven by its pose parameters; only where
    /// the game asks for it (`Animator::autoplay`).
    #[serde(default)]
    pub autoplay: bool,
    pub fade_in: f32,
    pub fade_out: f32,
    /// Grid width and height, and its animations row-major.
    pub grid: (usize, usize),
    pub anims: Vec<usize>,
    /// Per axis (X, Y): pose parameter, keys (may be empty) and the range
    /// used without keys.
    pub axes: [Axis; 2],
    pub autolayers: Vec<AutoLayer>,
    /// Per target bone; 0 leaves the bone alone.
    pub bone_weights: Vec<f32>,
    /// Animation events, by cycle (games decide what they mean).
    pub events: Vec<AnimEvent>,
    /// Its animation's IK rules, in order (later ones win).
    pub ik_rules: Vec<IkRule>,
    /// Chains whose end stays where the pose underneath had it while this
    /// sequence (and its autolayers) is layered on (Source's sequence IK
    /// locks: TF2's aim matrices keep the feet planted,
    /// specs/tf2/player_animation.md 4a, 5a).
    #[serde(default)]
    pub ik_locks: Vec<IkLock>,
    /// Its cycle comes from the clock, not its host's (Source's real-time
    /// flag 0x100): as an autolayer under a timed pose
    /// (`AnimSet::accumulate_timed`, `Animator::pose`) it plays at
    /// frac(t x its cycles per second). TF2's Medic's idle noise
    /// (specs/tf2/player_animation.md 4c 3).
    #[serde(default)]
    pub realtime: bool,
}

/// A sequence's IK lock: chain `chain`'s end kept at its place before the
/// sequence by `position`, and its model-space turn by `1 - rotation`
/// (Source's local-rotation weight: 0 keeps the old turn).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IkLock {
    /// `AnimSet::ik_chains` index.
    pub chain: usize,
    pub position: f32,
    pub rotation: f32,
}

/// An event a sequence fires when its cycle passes `cycle`.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AnimEvent {
    pub cycle: f32,
    /// Its number (0 for named events).
    pub event: i32,
    /// Its name, for named events.
    pub name: String,
    pub options: String,
}

impl Sequence {
    /// Events whose cycle the playback passed going from `from` to `to`
    /// (`from` excluded, `to` included); `from` None: the sequence was
    /// (re)started, so events at cycle 0 fire too. A looping sequence that
    /// wrapped fires the end of the old loop and the start of the new.
    pub fn events_between(&self, from: Option<f32>, to: f32) -> impl Iterator<Item = &AnimEvent> {
        self.events.iter().filter(move |e| match from {
            None => e.cycle <= to,
            Some(f) if to >= f => e.cycle > f && e.cycle <= to,
            Some(f) => e.cycle > f || e.cycle <= to,
        })
    }
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct Axis {
    pub param: Option<usize>,
    pub keys: Vec<f32>,
    pub range: (f32, f32),
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct AutoLayer {
    pub sequence: usize,
    pub start: f32,
    pub peak: f32,
    pub tail: f32,
    pub end: f32,
    /// The ramp eased by 3r² − 2r³ (Source's spline flag).
    #[serde(default)]
    pub spline: bool,
    /// The ramp runs on this pose parameter's value instead of the
    /// parent's cycle (`AnimSet::params` index; Source's pose flag).
    #[serde(default)]
    pub param: Option<usize>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PoseParam {
    pub name: String,
    pub start: f32,
    pub end: f32,
    /// Wrap range (0: none).
    pub looping: f32,
}

impl PoseParam {
    /// The stored (normalized, 0..1) value for `value` (§5).
    pub fn encode(&self, mut value: f32) -> f32 {
        if self.looping != 0.0 {
            let l = self.looping;
            let mid = (self.start + self.end) / 2.0;
            value -= l * ((value + l - (mid + l / 2.0)) / l).floor();
        }
        if self.end == self.start {
            return 0.0;
        }
        ((value - self.start) / (self.end - self.start)).clamp(0.0, 1.0)
    }

    pub fn default_value(&self) -> f32 {
        if self.start < 0.0 && 0.0 < self.end {
            self.encode(0.0)
        } else {
            0.5
        }
    }
}

impl AnimSet {
    pub fn sequence(&self, name: &str) -> Option<usize> {
        if self.names.is_empty() {
            return self.sequences.iter().position(|s| s.name.eq_ignore_ascii_case(name));
        }
        self.names.get(&name.to_lowercase()).copied()
    }

    /// The first sequence for an activity (e.g. `ACT_RUN`).
    pub fn activity(&self, activity: &str) -> Option<usize> {
        self.sequences
            .iter()
            .position(|s| s.activity.eq_ignore_ascii_case(activity))
    }

    /// Every sequence for an activity with its selection weight
    /// (|activity weight|, at least 1 when all are 0).
    pub fn activities(&self, activity: &str) -> Vec<(usize, u32)> {
        let mut out: Vec<(usize, u32)> = self
            .sequences
            .iter()
            .enumerate()
            .filter(|(_, s)| s.activity.eq_ignore_ascii_case(activity))
            .map(|(i, s)| (i, s.activity_weight.unsigned_abs()))
            .collect();
        if out.iter().all(|(_, w)| *w == 0) {
            out.iter_mut().for_each(|(_, w)| *w = 1);
        }
        out
    }

    /// One sequence for an activity, chosen by weight with `roll` in
    /// [0, 1) (spec weapons.md 3.8: weighted random when several).
    pub fn pick_activity(&self, activity: &str, roll: f32) -> Option<usize> {
        let options = self.activities(activity);
        let total: u32 = options.iter().map(|(_, w)| w).sum();
        if total == 0 {
            return None;
        }
        let mut at = (roll.clamp(0.0, 0.999_999) * total as f32) as u32;
        for (s, w) in &options {
            if at < *w {
                return Some(*s);
            }
            at -= w;
        }
        options.last().map(|(s, _)| *s)
    }

    /// The sequence an activity finds (specs/tf2/player_animation.md 1a):
    /// `current` kept when it has the activity and a negative weight, else
    /// one drawn by |weight| with `roll` in [0, 1), walking the candidates
    /// in order (hidden sequences count). None: no sequence has it.
    pub fn select_activity(&self, activity: &str, current: Option<usize>, roll: f32) -> Option<usize> {
        if let Some(c) = current
            && let Some(seq) = self.sequences.get(c)
            && seq.activity.eq_ignore_ascii_case(activity)
            && seq.activity_weight < 0
        {
            return Some(c);
        }
        self.pick_activity(activity, roll)
    }

    /// Seconds sequence `s` takes to play once at its own rate (spec
    /// weapons.md 3.8: (frames - 1) / fps; 0 for a single frame).
    pub fn duration(&self, s: usize) -> f32 {
        let rate = self.cycle_rate(s, &self.default_params());
        if rate > 0.0 { 1.0 / rate } else { 0.0 }
    }

    pub fn param(&self, name: &str) -> Option<usize> {
        self.params.iter().position(|p| p.name.eq_ignore_ascii_case(name))
    }

    /// Stored values of all pose parameters before anything sets them.
    pub fn default_params(&self) -> Vec<f32> {
        self.params.iter().map(PoseParam::default_value).collect()
    }

    /// Animation `a` at `cycle` into `out` (one entry per target bone),
    /// §3.
    pub fn sample(&self, a: usize, cycle: f32, out: &mut [BonePose]) {
        let anim = &self.animations[a];
        if anim.delta {
            out.fill((Quat::IDENTITY, Vec3::ZERO));
        } else {
            out.copy_from_slice(&self.bases[anim.base]);
        }
        let f = cycle * (anim.frames.max(1) - 1) as f32;
        let i = (f as usize).min(anim.frames.saturating_sub(1));
        let t = f - i as f32;
        let blend = t > 0.001;
        for track in &anim.tracks {
            let Some(bone) = out.get_mut(track.bone) else { continue };
            if let Some(r) = &track.rotation {
                bone.0 = if r.len() == 1 {
                    r[0]
                } else if blend && i + 1 < r.len() {
                    nlerp(r[i], r[i + 1], t)
                } else {
                    r[i.min(r.len() - 1)]
                };
            }
            if let Some(p) = &track.position {
                bone.1 = if p.len() == 1 {
                    p[0]
                } else if blend && i + 1 < p.len() {
                    p[i].lerp(p[i + 1], t)
                } else {
                    p[i.min(p.len() - 1)]
                };
            }
        }
    }

    /// Grid cell and fraction on one axis of a sequence (§5).
    pub fn axis(&self, seq: &Sequence, axis: usize, params: &[f32]) -> (usize, f32) {
        let a = &seq.axes[axis];
        let size = if axis == 0 { seq.grid.0 } else { seq.grid.1 };
        let Some(pi) = a.param else { return (0, 0.0) };
        let p = &self.params[pi];
        let n = params.get(pi).copied().unwrap_or_else(|| p.default_value());
        if a.keys.len() >= 2 {
            let v = n * (p.end - p.start) + p.start;
            let mut i = 0;
            let frac = |i: usize| (v - a.keys[i]) / (a.keys[i + 1] - a.keys[i]);
            let mut s = frac(i);
            while s > 1.0 && i + 2 < size.min(a.keys.len()) {
                i += 1;
                s = frac(i);
            }
            (i, s.clamp(0.0, 1.0))
        } else {
            let span = p.end - p.start;
            let (lo, hi) = ((a.range.0 - p.start) / span, (a.range.1 - p.start) / span);
            let s = ((n - lo) / (hi - lo)).clamp(0.0, 1.0);
            if size > 2 {
                let i = ((s * (size - 1) as f32) as usize).min(size - 2);
                (i, s * (size - 1) as f32 - i as f32)
            } else {
                (0, s)
            }
        }
    }

    fn cell(seq: &Sequence, x: usize, y: usize) -> usize {
        let (w, h) = seq.grid;
        seq.anims[y.min(h.max(1) - 1) * w + x.min(w.max(1) - 1)]
    }

    /// The pose of sequence `s` alone at `cycle` (§5). Bones with weight 0
    /// keep the first animation's values.
    pub fn sequence_pose(&self, s: usize, cycle: f32, params: &[f32], out: &mut [BonePose]) {
        let seq = &self.sequences[s];
        let cycle = wrap_cycle(cycle, seq.looping);
        let (i0, s0) = self.axis(seq, 0, params);
        let (i1, s1) = self.axis(seq, 1, params);
        let n = out.len();
        let mut tmp = vec![(Quat::IDENTITY, Vec3::ZERO); n];
        let mut tmp2 = vec![(Quat::IDENTITY, Vec3::ZERO); n];
        let edge = |s: f32| {
            if s < 0.001 {
                Some(0)
            } else if s > 0.999 {
                Some(1)
            } else {
                None
            }
        };
        // Sample cell (x, y) and blend it into `out` by `w` (or set it).
        let put = |out: &mut [BonePose], x: usize, y: usize, w: Option<f32>, tmp: &mut [BonePose]| match w {
            None => self.sample(Self::cell(seq, x, y), cycle, out),
            Some(w) => {
                self.sample(Self::cell(seq, x, y), cycle, tmp);
                blend(out, tmp, w, &seq.bone_weights);
            }
        };
        match (edge(s0), edge(s1)) {
            (Some(e0), Some(e1)) => put(out, i0 + e0, i1 + e1, None, &mut tmp),
            (Some(e0), None) => {
                put(out, i0 + e0, i1, None, &mut tmp);
                put(out, i0 + e0, i1 + 1, Some(s1), &mut tmp);
            }
            (None, Some(e1)) => {
                put(out, i0, i1 + e1, None, &mut tmp);
                put(out, i0 + 1, i1 + e1, Some(s0), &mut tmp);
            }
            (None, None) => {
                let ([c0, c1, c2], [w0, w1, w2]) = triangle(i0, i1, s0, s1);
                put(out, c0.0, c0.1, None, &mut tmp);
                if w1 == 0.0 {
                    put(out, c2.0, c2.1, Some(w2 / (w0 + w2)), &mut tmp);
                } else {
                    put(out, c1.0, c1.1, Some(w1 / (w0 + w1)), &mut tmp);
                    put(out, c2.0, c2.1, Some(w2), &mut tmp2);
                }
            }
        }
    }

    /// Cycles per second of sequence `s` at `params` (§9).
    pub fn cycle_rate(&self, s: usize, params: &[f32]) -> f32 {
        let seq = &self.sequences[s];
        let (i0, s0) = self.axis(seq, 0, params);
        let (i1, s1) = self.axis(seq, 1, params);
        let mut rate = 0.0;
        for (dx, dy, w) in [
            (0, 0, (1.0 - s0) * (1.0 - s1)),
            (1, 0, s0 * (1.0 - s1)),
            (0, 1, (1.0 - s0) * s1),
            (1, 1, s0 * s1),
        ] {
            let a = &self.animations[Self::cell(seq, i0 + dx, i1 + dy)];
            if w > 0.0 && a.frames > 1 {
                rate += w * a.fps / (a.frames - 1) as f32;
            }
        }
        rate
    }

    /// The authored ground speed of sequence `s` at `params`: its
    /// animations' `speed` blended like `cycle_rate`.
    pub fn ground_speed(&self, s: usize, params: &[f32]) -> f32 {
        let seq = &self.sequences[s];
        let (i0, s0) = self.axis(seq, 0, params);
        let (i1, s1) = self.axis(seq, 1, params);
        [
            (0, 0, (1.0 - s0) * (1.0 - s1)),
            (1, 0, s0 * (1.0 - s1)),
            (0, 1, (1.0 - s0) * s1),
            (1, 1, s0 * s1),
        ]
        .into_iter()
        .map(|(dx, dy, w)| w * self.animations[Self::cell(seq, i0 + dx, i1 + dy)].speed)
        .sum()
    }

    /// The length of sequence `s`'s blended authored velocity at
    /// `params`: its animations' `motion` vectors weighted like
    /// `cycle_rate` (between two directions, shorter than either:
    /// specs/hl2dm/player_models.md 5).
    pub fn ground_motion(&self, s: usize, params: &[f32]) -> f32 {
        let seq = &self.sequences[s];
        let (i0, s0) = self.axis(seq, 0, params);
        let (i1, s1) = self.axis(seq, 1, params);
        [
            (0, 0, (1.0 - s0) * (1.0 - s1)),
            (1, 0, s0 * (1.0 - s1)),
            (0, 1, (1.0 - s0) * s1),
            (1, 1, s0 * s1),
        ]
        .into_iter()
        .map(|(dx, dy, w)| w * self.animations[Self::cell(seq, i0 + dx, i1 + dy)].motion)
        .sum::<Vec2>()
        .length()
    }

    /// Fold sequence `s`'s IK rules, then its autolayers', into `locks`
    /// (per chain: Some(true) locked, Some(false) released, None
    /// untouched), as `accumulate` visits them.
    pub fn ik_locks(&self, s: usize, locks: &mut [Option<bool>]) {
        self.ik_locks_depth(s, locks, 0);
    }

    fn ik_locks_depth(&self, s: usize, locks: &mut [Option<bool>], depth: u32) {
        let Some(seq) = self.sequences.get(s).filter(|_| depth <= 8) else {
            return;
        };
        for r in &seq.ik_rules {
            if let Some(l) = locks.get_mut(r.chain) {
                *l = Some(r.lock);
            }
        }
        for l in &seq.autolayers {
            self.ik_locks_depth(l.sequence, locks, depth + 1);
        }
    }

    /// Bend each goal's chain so its end bone lands on the target bone
    /// (place and turn, blended by the goal's weight), keeping the elbow
    /// in the plane it bent in. `pose` is local (as `Animator::pose`);
    /// needs `parents`.
    pub fn solve_ik(&self, pose: &mut [BonePose], goals: &[IkGoal]) {
        if self.parents.len() != pose.len() || goals.is_empty() {
            return;
        }
        for g in goals {
            if g.target >= pose.len() {
                continue;
            }
            let target = model_space(pose, &self.parents)[g.target];
            self.reach(pose, g.chain, target, g.weight);
        }
    }

    /// Bend chain `chain` so its end moves toward `target` (model-space
    /// turn and place) by `weight`, keeping the elbow in the plane it bent
    /// in.
    fn reach(&self, pose: &mut [BonePose], chain: usize, target: BonePose, weight: f32) {
        let (Some(chain), true) = (self.ik_chains.get(chain), weight > 0.0) else {
            return;
        };
        let global = model_space(pose, &self.parents);
        let [a, b, c] = chain.bones;
        if [a, b, c].iter().any(|i| *i >= pose.len()) {
            return;
        }
        let w = weight.min(1.0);
        let (target_q, target_p) = target;
        let goal_p = global[c].1.lerp(target_p, w);
        let goal_q = slerp(global[c].0, target_q, w);
        let (sa, eb, hc) = (global[a].1, global[b].1, global[c].1);
        let (upper, lower) = (eb.distance(sa), hc.distance(eb));
        let to_goal = goal_p - sa;
        let d = to_goal.length().clamp((upper - lower).abs() + 1e-3, upper + lower - 1e-3);
        let Some(dir) = to_goal.try_normalize() else { return };
        // The elbow stays on its side of the shoulder-goal line.
        let bend = (eb - sa)
            .reject_from_normalized(dir)
            .try_normalize()
            .unwrap_or_else(|| dir.any_orthonormal_vector());
        let along = (upper * upper - lower * lower + d * d) / (2.0 * d);
        let elbow = sa + dir * along + bend * (upper * upper - along * along).max(0.0).sqrt();
        // Turn the upper bone onto the new elbow, then the lower one onto the goal.
        let turn_a = Quat::from_rotation_arc((eb - sa).normalize_or_zero(), (elbow - sa).normalize_or_zero());
        let qa = turn_a * global[a].0;
        let lower_now = turn_a * (hc - eb);
        let reach = sa + dir * d;
        let turn_b = Quat::from_rotation_arc(lower_now.normalize_or_zero(), (reach - elbow).normalize_or_zero());
        let qb = turn_b * turn_a * global[b].0;
        let parent_q = |i: usize, own: &[(Quat, Vec3)]| self.parents[i].map_or(Quat::IDENTITY, |p| own[p].0);
        let mut own = global.clone();
        own[a].0 = qa;
        own[b].0 = qb;
        own[c].0 = goal_q;
        pose[a].0 = (parent_q(a, &own).inverse() * qa).normalize();
        pose[b].0 = (parent_q(b, &own).inverse() * qb).normalize();
        pose[c].0 = (parent_q(c, &own).inverse() * goal_q).normalize();
    }

    /// Merge sequence `s` at `cycle` and `weight` into `pose`, then its
    /// autolayers (§6, §7).
    pub fn accumulate(&self, pose: &mut [BonePose], s: usize, cycle: f32, weight: f32, params: &[f32]) {
        self.accumulate_depth(pose, s, cycle, weight, params, false, None, 0);
    }

    /// `accumulate` (with the IK locks when `locks`) at clock time `now`:
    /// real-time autolayers (`Sequence::realtime`) take their cycle from
    /// it.
    #[allow(clippy::too_many_arguments)]
    pub fn accumulate_timed(&self, pose: &mut [BonePose], s: usize, cycle: f32, weight: f32, params: &[f32], locks: bool, now: f64) {
        self.accumulate_depth(pose, s, cycle, weight, params, locks, Some(now), 0);
    }

    /// `accumulate` keeping each sequence's IK-locked chain ends where the
    /// pose underneath had them (`Sequence::ik_locks`; a game whose
    /// players have Source's IK context, `Animator::ik_locks`).
    pub fn accumulate_locked(&self, pose: &mut [BonePose], s: usize, cycle: f32, weight: f32, params: &[f32]) {
        self.accumulate_depth(pose, s, cycle, weight, params, true, None, 0);
    }

    #[allow(clippy::too_many_arguments)]
    fn accumulate_depth(&self, pose: &mut [BonePose], s: usize, cycle: f32, weight: f32, params: &[f32], locks: bool, now: Option<f64>, depth: u32) {
        let weight = weight.clamp(0.0, 1.0);
        if weight <= 0.0 || depth > 8 || s >= self.sequences.len() {
            return;
        }
        let seq = &self.sequences[s];
        // The locked chains' ends before this layer (5a step 1).
        let locked = if locks { self.lock_targets(pose, &seq.ik_locks) } else { Vec::new() };
        let mut own = vec![(Quat::IDENTITY, Vec3::ZERO); pose.len()];
        self.sequence_pose(s, cycle, params, &mut own);
        for (b, (cur, new)) in pose.iter_mut().zip(&own).enumerate() {
            let s = (weight * seq.bone_weights.get(b).copied().unwrap_or(0.0)).min(1.0);
            if s <= 0.0 {
                continue;
            }
            if !seq.delta {
                cur.0 = slerp(new.0, cur.0, 1.0 - s);
                cur.1 = cur.1 * (1.0 - s) + new.1 * s;
            } else if seq.post {
                cur.0 = product(cur.0, scale(new.0, s)).normalize();
                cur.1 += new.1 * s;
            } else {
                cur.0 = product(scale(new.0, s), cur.0).normalize();
                cur.1 += new.1 * s;
            }
        }
        for l in &seq.autolayers {
            if let Some((mut c, r)) = self.layer_ramp(l, cycle, params) {
                if let Some(t) = now
                    && self.sequences.get(l.sequence).is_some_and(|q| q.realtime)
                {
                    c = (t * f64::from(self.cycle_rate(l.sequence, params))).fract() as f32;
                }
                self.accumulate_depth(pose, l.sequence, c, weight * r, params, locks, now, depth + 1);
            }
        }
        self.solve_locks(pose, &locked);
    }

    /// An autolayer's cycle and weight under a parent at `cycle` (5a step
    /// 4), None when it is off: the ramp on the parent's cycle or a pose
    /// parameter's value (start = end: always on, on the parent's cycle),
    /// eased by a spline when flagged.
    fn layer_ramp(&self, l: &AutoLayer, cycle: f32, params: &[f32]) -> Option<(f32, f32)> {
        let at = match l.param.and_then(|p| Some((self.params.get(p)?, params.get(p)?))) {
            Some((p, n)) => n * (p.end - p.start) + p.start,
            None => cycle,
        };
        let ramped = l.start != l.end && !(l.start.is_nan() || l.end.is_nan());
        if !ramped {
            return Some((cycle, 1.0));
        }
        if at < l.start || at >= l.end {
            return None;
        }
        let mut r = 1.0;
        if at < l.peak && l.start != l.peak {
            r = (at - l.start) / (l.peak - l.start);
        } else if at > l.tail && l.end != l.tail {
            r = (l.end - at) / (l.end - l.tail);
        }
        if l.spline {
            r = 3.0 * r * r - 2.0 * r * r * r;
        }
        // A layer ramped on a parameter plays on the parent's cycle.
        let c = if l.param.is_some() { cycle } else { (cycle - l.start) / (l.end - l.start) };
        Some((c, r))
    }

    /// The grid cells sequence `s` blends at `params` and their weights
    /// (the triangle blend of `sequence_pose`, §5).
    pub fn cell_weights(&self, s: usize, params: &[f32]) -> Vec<(usize, f32)> {
        let seq = &self.sequences[s];
        let (i0, s0) = self.axis(seq, 0, params);
        let (i1, s1) = self.axis(seq, 1, params);
        let cell = |x: usize, y: usize| Self::cell(seq, x, y);
        let edge = |s: f32| if s < 0.001 { Some(0) } else if s > 0.999 { Some(1) } else { None };
        match (edge(s0), edge(s1)) {
            (Some(e0), Some(e1)) => vec![(cell(i0 + e0, i1 + e1), 1.0)],
            (Some(e0), None) => vec![(cell(i0 + e0, i1), 1.0 - s1), (cell(i0 + e0, i1 + 1), s1)],
            (None, Some(e1)) => vec![(cell(i0, i1 + e1), 1.0 - s0), (cell(i0 + 1, i1 + e1), s0)],
            (None, None) => {
                let (c, w) = triangle(i0, i1, s0, s1);
                c.iter().zip(w).filter(|(_, w)| *w > 0.0).map(|(c, w)| (cell(c.0, c.1), w)).collect()
            }
        }
    }

    /// Fold sequence `s`'s IK rules at `cycle` and `weight`, then its
    /// autolayers', into `goals` (per chain; specs/tf2/player_animation.md
    /// 4a): a release lets the chain go by the weight; a lock with stored
    /// places (its cells' blended by the grid's weights, each by its
    /// ramp) pulls the goal toward its place by the weight.
    pub fn rule_goals(&self, s: usize, cycle: f32, weight: f32, params: &[f32], goals: &mut [Option<RuleGoal>]) {
        self.rule_goals_depth(s, cycle, weight, params, goals, 0);
    }

    fn rule_goals_depth(&self, s: usize, cycle: f32, weight: f32, params: &[f32], goals: &mut [Option<RuleGoal>], depth: u32) {
        let weight = weight.clamp(0.0, 1.0);
        let Some(seq) = self.sequences.get(s).filter(|_| weight > 0.0 && depth <= 8) else {
            return;
        };
        let cycle = wrap_cycle(cycle, seq.looping);
        for r in seq.ik_rules.iter().filter(|r| !r.lock) {
            if let Some(Some(g)) = goals.get_mut(r.chain) {
                g.weight *= 1.0 - weight;
            }
        }
        let cells = if seq.anims.iter().any(|&a| self.animations.get(a).is_some_and(|a| !a.ik_targets.is_empty())) {
            self.cell_weights(s, params)
        } else {
            Vec::new()
        };
        for chain in 0..goals.len() {
            // The cells' places for this chain, blended by their weights.
            let (mut sum, mut bone, mut q, mut p) = (0.0f32, None, Quat::IDENTITY, Vec3::ZERO);
            for &(a, w) in &cells {
                let Some(t) = self.animations.get(a).and_then(|a| a.ik_targets.iter().find(|t| t.chain == chain)) else {
                    continue;
                };
                let (Some(place), w) = (t.at(cycle), w * t.weight(cycle)) else { continue };
                if w <= 0.0 || bone.is_some_and(|b| b != t.bone) {
                    continue;
                }
                bone = Some(t.bone);
                sum += w;
                q = if sum == w { place.0 } else { nlerp(q, place.0, w / sum) };
                p = p.lerp(place.1, w / sum);
            }
            let Some(bone) = bone.filter(|_| sum > 0.0) else { continue };
            let w = weight * sum.min(1.0);
            let g = &mut goals[chain];
            *g = Some(match *g {
                Some(old) if old.bone == bone && old.weight > 0.0 => {
                    let total = old.weight + (1.0 - old.weight) * w;
                    let t = w / total;
                    RuleGoal { bone, place: (nlerp(old.place.0, q, t), old.place.1.lerp(p, t)), weight: total }
                }
                _ => RuleGoal { bone, place: (q, p), weight: w },
            });
        }
        for l in &seq.autolayers {
            if let Some((c, r)) = self.layer_ramp(l, cycle, params) {
                self.rule_goals_depth(l.sequence, c, weight * r, params, goals, depth + 1);
            }
        }
    }

    /// Bend each goal's chain so its end reaches the goal's place in its
    /// bone's frame (from this pose), by the goal's weight
    /// (`rule_goals`).
    pub fn solve_rule_goals(&self, pose: &mut [BonePose], goals: &[Option<RuleGoal>]) {
        if self.parents.len() != pose.len() {
            return;
        }
        for (chain, g) in goals.iter().enumerate() {
            let Some(g) = g.filter(|g| g.weight > 0.0 && g.bone < pose.len()) else { continue };
            let global = model_space(pose, &self.parents);
            let (bq, bp) = global[g.bone];
            let target = (bq * g.place.0, bp + bq * g.place.1);
            self.reach(pose, chain, target, g.weight);
        }
    }

    /// Each lock's chain end in model space now (with the lock), and its
    /// knee direction turned into model space by the root's turn now (4a).
    fn lock_targets(&self, pose: &[BonePose], locks: &[IkLock]) -> Vec<(IkLock, BonePose, Vec3)> {
        if locks.is_empty() || self.parents.len() != pose.len() {
            return Vec::new();
        }
        let global = model_space(pose, &self.parents);
        locks
            .iter()
            .filter_map(|l| {
                let chain = self.ik_chains.get(l.chain)?;
                let knee = global.get(chain.bones[0])?.0 * chain.knee;
                Some((*l, *global.get(chain.bones[2])?, knee))
            })
            .collect()
    }

    /// Put each locked chain's end back where it was (4a): the two-bone
    /// chain bent to reach the old place, then the end's old model-space
    /// turn. A chain with a knee direction bends its knee toward it (the
    /// aim point 100 x the reach out along it from the model origin; the
    /// reach clamped to 0.9998 of the leg's length and to at least
    /// max(1.15 |L1 - L2|, 0.15 min(L1, L2))); one without keeps the plane
    /// it bends in, a leg within 1 degree of straight left alone.
    fn solve_locks(&self, pose: &mut [BonePose], locked: &[(IkLock, BonePose, Vec3)]) {
        for (l, (old_q, old_p), knee_dir) in locked {
            let Some(chain) = self.ik_chains.get(l.chain) else { continue };
            let [a, b, c] = chain.bones;
            let global = model_space(pose, &self.parents);
            let (sa, eb, hc) = (global[a].1, global[b].1, global[c].1);
            let w = l.position.clamp(0.0, 1.0);
            let goal = hc.lerp(*old_p, w);
            let (upper, lower) = (eb - sa, hc - eb);
            let (lu, ll) = (upper.length(), lower.length());
            let solved = if *knee_dir != Vec3::ZERO {
                (w > 0.0 && lu > 1e-4 && ll > 1e-4).then(|| {
                    let mut to_goal = goal - sa;
                    let reach = lu + ll;
                    let near = (1.15 * (lu - ll).abs()).max(0.15 * lu.min(ll));
                    let d = to_goal.length();
                    if d > 0.9998 * reach {
                        to_goal *= 0.9998 * reach / d;
                    } else if d < near {
                        to_goal = (hc - sa).try_normalize().unwrap_or(Vec3::Z) * near;
                    }
                    let k = 100.0 * reach.max(to_goal.length() - lu.min(ll));
                    let aim = -sa + *knee_dir * k;
                    (to_goal, aim)
                })
            } else {
                let straight = upper.angle_between(lower) < 1f32.to_radians();
                (w > 0.0 && !straight && goal.distance(hc) > 1e-4).then(|| {
                    let to_goal = goal - sa;
                    let d = to_goal.length().clamp((lu - ll).abs() + 1e-3, lu + ll - 1e-3);
                    // The bend plane's normal: kept from the current chain.
                    let dir = to_goal.normalize_or_zero();
                    let side = upper.reject_from_normalized(dir);
                    let bend = upper
                        .cross(lower)
                        .try_normalize()
                        .map(|n| n.cross(dir).normalize())
                        .map(|v| if v.dot(side) < 0.0 { -v } else { v })
                        .unwrap_or_else(|| side.normalize_or_zero());
                    (dir * d, bend)
                })
            };
            if let Some((to_goal, toward)) = solved
                && let Some(dir) = to_goal.try_normalize()
            {
                let d = to_goal.length().clamp((lu - ll).abs() + 1e-3, lu + ll - 1e-3);
                let bend = toward.reject_from_normalized(dir).try_normalize().unwrap_or_else(|| upper.reject_from_normalized(dir).normalize_or_zero());
                let along = (lu * lu - ll * ll + d * d) / (2.0 * d);
                let knee = sa + dir * along + bend * (lu * lu - along * along).max(0.0).sqrt();
                let turn_a = Quat::from_rotation_arc(upper.normalize_or_zero(), (knee - sa).normalize_or_zero());
                let qa = turn_a * global[a].0;
                let lower_now = turn_a * lower;
                let turn_b = Quat::from_rotation_arc(lower_now.normalize_or_zero(), (sa + dir * d - knee).normalize_or_zero());
                let qb = turn_b * turn_a * global[b].0;
                let parent_q = |i: usize| self.parents[i].map_or(Quat::IDENTITY, |p| global[p].0);
                pose[a].0 = (parent_q(a).inverse() * qa).normalize();
                let parent_b = if self.parents[b] == Some(a) { qa } else { parent_q(b) };
                pose[b].0 = (parent_b.inverse() * qb).normalize();
            }
            // The end's turn: the old one by 1 - rotation weight.
            let global = model_space(pose, &self.parents);
            let parent_q = self.parents[c].map_or(Quat::IDENTITY, |p| global[p].0);
            let keep = (1.0 - l.rotation).clamp(0.0, 1.0);
            let q = slerp(global[c].0, *old_q, keep).normalize();
            pose[c].0 = (parent_q.inverse() * q).normalize();
        }
    }
}

/// Each bone's model-space rotation and position from a local pose
/// (parents before children).
pub fn model_space(pose: &[BonePose], parents: &[Option<usize>]) -> Vec<BonePose> {
    let mut out: Vec<BonePose> = Vec::with_capacity(pose.len());
    for (i, &(q, p)) in pose.iter().enumerate() {
        out.push(match parents.get(i).copied().flatten().filter(|p| *p < i) {
            Some(pi) => (out[pi].0 * q, out[pi].1 + out[pi].0 * p),
            None => (q, p),
        });
    }
    out
}

/// Grid corners (offsets added to (i0, i1)) and weights of the triangle
/// blend (§5).
pub fn triangle(i0: usize, i1: usize, s0: f32, s1: f32) -> ([(usize, usize); 3], [f32; 3]) {
    let (c, w0, mut w1): ([(usize, usize); 3], f32, f32) = if (i0 + i1) % 2 == 0 {
        if s0 > s1 {
            ([(0, 0), (1, 0), (1, 1)], 1.0 - s0, s0 - s1)
        } else {
            ([(1, 1), (0, 1), (0, 0)], s0, s1 - s0)
        }
    } else if s0 + s1 > 1.0 {
        ([(1, 0), (1, 1), (0, 1)], 1.0 - s1, s0 + s1 - 1.0)
    } else {
        ([(0, 1), (0, 0), (1, 0)], s1, 1.0 - s0 - s1)
    };
    if w1 < 0.001 {
        w1 = 0.0;
    }
    let cells = c.map(|(x, y)| (i0 + x, i1 + y));
    (cells, [w0, w1, 1.0 - w0 - w1])
}

/// Blend `to` into `from` by `t` for bones with weight > 0: nlerp of
/// rotations, lerp of positions.
fn blend(from: &mut [BonePose], to: &[BonePose], t: f32, weights: &[f32]) {
    for (b, (p, q)) in from.iter_mut().zip(to).enumerate() {
        if weights.get(b).copied().unwrap_or(0.0) > 0.0 {
            p.0 = nlerp(p.0, q.0, t);
            p.1 = p.1.lerp(q.1, t);
        }
    }
}

/// Looping cycles wrap into [0, 1); others clamp to [0, 1].
pub fn wrap_cycle(c: f32, looping: bool) -> f32 {
    if looping { c - c.floor() } else { c.clamp(0.0, 1.0) }
}

/// `q`, or `-q` when that is closer to `p`.
pub fn align(p: Quat, q: Quat) -> Quat {
    let (a, b) = (Vec4::from(p), Vec4::from(q));
    if (a - b).length_squared() > (a + b).length_squared() {
        -q
    } else {
        q
    }
}

pub fn nlerp(p: Quat, q: Quat, t: f32) -> Quat {
    let q = align(p, q);
    Quat::from_vec4(Vec4::from(p) * (1.0 - t) + Vec4::from(q) * t).normalize()
}

/// Slerp without renormalization (§4).
pub fn slerp(p: Quat, q: Quat, t: f32) -> Quat {
    let q = align(p, q);
    let (a, b) = (Vec4::from(p), Vec4::from(q));
    let cos = a.dot(b);
    let (wa, wb) = if 1.0 - cos > 1e-6 {
        let o = cos.clamp(-1.0, 1.0).acos();
        let s = o.sin();
        (((1.0 - t) * o).sin() / s, (t * o).sin() / s)
    } else {
        (1.0 - t, t)
    };
    Quat::from_vec4(a * wa + b * wb)
}

/// The rotation `q` with its angle scaled by `s` (§4).
pub fn scale(q: Quat, s: f32) -> Quat {
    let v = Vec3::new(q.x, q.y, q.z);
    let sigma = v.length().min(1.0);
    let sp = (s * sigma.asin()).sin();
    let v = v * (sp / (sigma + f32::EPSILON));
    let w = (1.0 - sp * sp).max(0.0).sqrt().copysign(q.w);
    Quat::from_xyzw(v.x, v.y, v.z, w)
}

/// `p · q` with `q` first aligned to `p`.
pub fn product(p: Quat, q: Quat) -> Quat {
    p * align(p, q)
}

/// Euler angles (radians, fixed axes X then Y then Z) to a quaternion
/// (§2).
pub fn euler(x: f32, y: f32, z: f32) -> Quat {
    let (sx, cx) = (x / 2.0).sin_cos();
    let (sy, cy) = (y / 2.0).sin_cos();
    let (sz, cz) = (z / 2.0).sin_cos();
    Quat::from_xyzw(
        sx * cy * cz - cx * sy * sz,
        cx * sy * cz + sx * cy * sz,
        cx * cy * sz - sx * sy * cz,
        cx * cy * cz + sx * sy * sz,
    )
}

/// A sequence that is fading out after being replaced (§9).
#[derive(Debug, Clone)]
pub struct Fading {
    pub sequence: usize,
    pub cycle: f32,
    pub playback: f32,
    pub stopped: f64,
    pub fade: f32,
    /// A weight given from outside, whatever the time (a client posing
    /// another player as its server had it at a tick: Source's
    /// `anim_net`); None: from `stopped` and `fade`.
    pub held: Option<f32>,
}

impl Fading {
    /// The smoothstep-shaped weight at `now` (0 once gone), or the held one.
    pub fn weight(&self, now: f64) -> f32 {
        if let Some(w) = self.held {
            return w;
        }
        if self.fade <= 0.0 {
            return 0.0;
        }
        let u = (1.0 - (now - self.stopped) as f32 / self.fade).clamp(0.0, 1.0);
        3.0 * u * u - 2.0 * u * u * u
    }
}

/// An overlay layer (§10).
#[derive(Debug, Clone, Copy)]
pub struct Layer {
    pub sequence: usize,
    pub cycle: f32,
    pub weight: f32,
}

/// What a body plays: its main sequence (with the ones fading out), the
/// overlay layers in order, and the pose parameters (stored values).
/// Games drive it; `pose_bodies` turns it into joint transforms.
#[derive(Component, Debug, Default, Clone)]
#[component(storage = "SparseSet")]
pub struct Animator {
    pub set: Arc<AnimSet>,
    pub params: Vec<f32>,
    pub main: Option<usize>,
    pub cycle: f32,
    pub playback: f32,
    pub fading: Vec<Fading>,
    pub layers: Vec<Option<Layer>>,
    /// The body's yaw (radians, as `Intent::yaw`); None: the character's
    /// look yaw.
    pub yaw: Option<f32>,
    /// Limbs pulled onto bones after the layers (`AnimSet::solve_ik`).
    pub ik: Vec<IkGoal>,
    /// Keep IK-locked chain ends in place under each layer
    /// (`AnimSet::accumulate_locked`): TF2's players' feet under the aim
    /// matrices and gestures.
    pub ik_locks: bool,
    /// Pull chains to their animations' IK rules' stored places after the
    /// layers (`AnimSet::rule_goals`, `solve_rule_goals`): TF2's Heavy's
    /// left hand on the Minigun.
    pub ik_rules: bool,
    /// A pose the game laid out itself (its own layering and bone
    /// controllers: Combat Arms' trackers and aim), one per bone:
    /// `pose` returns it as it is, and the sequences above are ignored.
    /// Made when first drawn (`later::Later`): a body nobody draws (out of
    /// view, `render::bodies::pose_bodies`) is never posed for drawing.
    pub fixed: Option<chimera_core::later::Later<Vec<BonePose>>>,
    /// Apply the set's autoplay sequences (`Sequence::autoplay`) after the
    /// layers, at weight 1 (HL2:DM's head and spine turns: specs/hl2dm/
    /// player_models.md 7 step 5); off for games that don't ask.
    pub autoplay: bool,
    /// The skeleton this animator's pose is for (`skeleton_signature`):
    /// `pose_bodies` poses only a body on that skeleton, so one
    /// character's skeleton never drives another's model. None: any (a
    /// game that says nothing; a corpse's own pose).
    pub skeleton: Option<u64>,
}

/// A skeleton's shape as one number: its bones' names (case ignored) and
/// parents, in order. Two skeletons with the same signature take each
/// other's poses (Combat Arms' stock bodies of one tree); two with
/// different ones must not.
pub fn skeleton_signature<'a>(bones: impl IntoIterator<Item = (&'a str, Option<usize>)>) -> u64 {
    const PRIME: u64 = 0x100_0000_01b3;
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut eat = |b: u8| h = (h ^ u64::from(b)).wrapping_mul(PRIME);
    for (name, parent) in bones {
        name.bytes().for_each(|b| eat(b.to_ascii_lowercase()));
        eat(0);
        parent.map_or(u64::MAX, |p| p as u64).to_le_bytes().into_iter().for_each(&mut eat);
    }
    h
}

impl Animator {
    /// Whether this animator has a pose for a body on skeleton `skeleton`
    /// (`skeleton_signature`): something playing, on that skeleton or on
    /// one it doesn't name.
    pub fn poses(&self, skeleton: u64) -> bool {
        self.main.is_some() && self.drives(skeleton)
    }

    /// Whether this animator's pose may go on a body on skeleton
    /// `skeleton`.
    pub fn drives(&self, skeleton: u64) -> bool {
        self.skeleton.is_none_or(|s| s == skeleton)
    }

    pub fn new(set: Arc<AnimSet>) -> Self {
        Self {
            params: set.default_params(),
            set,
            playback: 1.0,
            ..default()
        }
    }

    pub fn set_param(&mut self, i: usize, value: f32) {
        if let (Some(p), Some(v)) = (self.set.params.get(i), self.params.get_mut(i)) {
            *v = p.encode(value);
        }
    }

    /// Switch the main sequence at time `now` (§9): non-looping outgoing
    /// sequences restart the cycle, and the outgoing one fades out unless
    /// the new one snaps in.
    pub fn play(&mut self, s: usize, now: f64) {
        if self.main == Some(s) {
            return;
        }
        let set = self.set.clone();
        let new = &set.sequences[s];
        if let Some(old) = self.main {
            let o = &set.sequences[old];
            if new.snap {
                self.fading.clear();
            } else {
                self.fading.insert(
                    0,
                    Fading {
                        sequence: old,
                        cycle: self.cycle,
                        playback: self.playback,
                        stopped: now,
                        fade: o.fade_out.min(new.fade_in),
                        held: None,
                    },
                );
            }
            if !o.looping {
                self.cycle = 0.0;
            }
        }
        self.main = Some(s);
        self.playback = 1.0;
    }

    /// Start sequence `s` from cycle 0 even if it is already playing (a
    /// view model replaying its fire sequence, spec weapons.md 3.8); the
    /// outgoing one fades out as in `play`.
    pub fn restart(&mut self, s: usize, now: f64) {
        if let Some(old) = self.main.filter(|&old| old == s) {
            let fade = self.set.sequences[old].fade_out.min(self.set.sequences[s].fade_in);
            if self.set.sequences[s].snap {
                self.fading.clear();
            } else {
                self.fading.insert(
                    0,
                    Fading {
                        sequence: old,
                        cycle: self.cycle,
                        playback: self.playback,
                        stopped: now,
                        fade,
                        held: None,
                    },
                );
            }
            self.playback = 1.0;
        } else {
            self.play(s, now);
        }
        self.cycle = 0.0;
    }

    /// Whether the main sequence is non-looping and has reached its end.
    pub fn finished(&self) -> bool {
        self.main
            .is_some_and(|s| !self.set.sequences[s].looping && self.cycle >= 1.0)
    }

    /// Advance the main and fading cycles by `dt`; drop finished fades.
    pub fn advance(&mut self, dt: f32, now: f64) {
        let set = self.set.clone();
        if let Some(s) = self.main {
            let looping = set.sequences[s].looping;
            self.cycle = wrap_cycle(
                self.cycle + dt * set.cycle_rate(s, &self.params) * self.playback,
                looping,
            );
        }
        for f in &mut self.fading {
            let looping = set.sequences[f.sequence].looping;
            f.cycle = wrap_cycle(
                f.cycle + dt * set.cycle_rate(f.sequence, &self.params) * f.playback,
                looping,
            );
        }
        self.fading.retain(|f| f.weight(now) > 0.0);
    }

    /// The whole body's pose (§ "Bone setup"), or the game's own
    /// (`fixed`).
    pub fn pose(&self, now: f64) -> Vec<BonePose> {
        if let Some(fixed) = &self.fixed {
            return fixed.to_vec();
        }
        let set = &*self.set;
        let acc = |pose: &mut [BonePose], s: usize, c: f32, w: f32, p: &[f32]| set.accumulate_timed(pose, s, c, w, p, self.ik_locks, now);
        let mut pose = set.defaults.clone();
        if let Some(s) = self.main {
            acc(&mut pose, s, self.cycle, 1.0, &self.params);
        }
        for f in &self.fading {
            acc(&mut pose, f.sequence, f.cycle, f.weight(now), &self.params);
        }
        for l in self.layers.iter().flatten() {
            let Some(seq) = set.sequences.get(l.sequence) else {
                continue;
            };
            let cycle = if seq.looping {
                wrap_cycle(l.cycle, true)
            } else {
                l.cycle.clamp(0.0, 0.999)
            };
            acc(&mut pose, l.sequence, cycle, l.weight.min(1.0), &self.params);
        }
        if self.autoplay {
            for s in (0..set.sequences.len()).filter(|&s| set.sequences[s].autoplay) {
                acc(&mut pose, s, 0.0, 1.0, &self.params);
            }
        }
        if self.ik_rules && !set.ik_chains.is_empty() {
            // The rules in the layers' order, then solved on the whole pose.
            let mut goals = vec![None; set.ik_chains.len()];
            if let Some(s) = self.main {
                set.rule_goals(s, self.cycle, 1.0, &self.params, &mut goals);
            }
            for f in &self.fading {
                set.rule_goals(f.sequence, f.cycle, f.weight(now), &self.params, &mut goals);
            }
            for l in self.layers.iter().flatten() {
                if let Some(seq) = set.sequences.get(l.sequence) {
                    let cycle = if seq.looping { wrap_cycle(l.cycle, true) } else { l.cycle.clamp(0.0, 0.999) };
                    set.rule_goals(l.sequence, cycle, l.weight.min(1.0), &self.params, &mut goals);
                }
            }
            set.solve_rule_goals(&mut pose, &goals);
        }
        set.solve_ik(&mut pose, &self.ik);
        pose
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn param(start: f32, end: f32, looping: f32) -> PoseParam {
        PoseParam {
            name: String::new(),
            start,
            end,
            looping,
        }
    }

    /// A game's own pose is the pose, whatever the sequences say.
    #[test]
    fn fixed_pose_wins() {
        let set = Arc::new(AnimSet {
            defaults: vec![(Quat::IDENTITY, Vec3::ZERO)],
            parents: vec![None],
            ..default()
        });
        let mut a = Animator::new(set);
        let pose = vec![(Quat::from_rotation_y(0.5), Vec3::X)];
        a.fixed = Some(chimera_core::later::Later::ready(pose.clone()));
        assert_eq!(a.pose(0.0), pose);
    }

    #[test]
    fn pose_parameters_encode() {
        let body_yaw = param(-90.0, 90.0, 360.0);
        assert!((body_yaw.encode(30.0) - 0.666667).abs() < 1e-5);
        assert_eq!(body_yaw.encode(120.0), 1.0);
        assert!((param(-180.0, 180.0, 360.0).encode(270.0) - 0.25).abs() < 1e-6);
        assert_eq!(param(-90.0, 90.0, 360.0).encode(-100.0), 0.0);
        assert_eq!(body_yaw.default_value(), 0.5);
        assert_eq!(param(-1.0, 1.0, 0.0).default_value(), 0.5);
    }

    #[test]
    fn triangle_blend_weights() {
        let close = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5);
        let (c, w) = triangle(0, 0, 0.5, 0.133975);
        assert_eq!(c, [(0, 0), (1, 0), (1, 1)]);
        assert!(close(w, [0.5, 0.366025, 0.133975]), "{w:?}");
        let (c, w) = triangle(0, 0, 0.2, 0.6);
        assert_eq!(c, [(1, 1), (0, 1), (0, 0)]);
        assert!(close(w, [0.2, 0.4, 0.4]), "{w:?}");
        let (c, w) = triangle(1, 0, 1.0 / 3.0, 0.5);
        assert_eq!(c, [(1, 1), (1, 0), (2, 0)]);
        assert!(close(w, [0.5, 0.166667, 0.333333]), "{w:?}");
        let (c, w) = triangle(1, 0, 0.8, 0.6);
        assert_eq!(c, [(2, 0), (2, 1), (1, 1)]);
        assert!(close(w, [0.4, 0.4, 0.2]), "{w:?}");
    }

    /// A two-bone arm (lengths 10 and 8) reaches a bone 12 units off,
    /// with its turn; out of reach it stretches straight toward it.
    #[test]
    fn ik_bends_a_chain_onto_its_target() {
        let x = Vec3::X;
        let defaults = vec![
            (Quat::IDENTITY, Vec3::ZERO),
            (Quat::IDENTITY, x * 10.0),
            (Quat::from_rotation_z(0.3), x * 8.0),
            (Quat::from_rotation_y(0.5), Vec3::new(6.0, 7.0, 1.0)),
        ];
        let set = AnimSet {
            defaults: defaults.clone(),
            parents: vec![None, Some(0), Some(1), None],
            ik_chains: vec![IkChain {
                name: "arm".into(),
                bones: [0, 1, 2],
                ..default()
            }],
            ..default()
        };
        let goal = IkGoal {
            chain: 0,
            target: 3,
            weight: 1.0,
        };
        let mut pose = defaults.clone();
        set.solve_ik(&mut pose, &[goal]);
        let g = model_space(&pose, &set.parents);
        assert!(g[2].1.distance(g[3].1) < 1e-3, "{:?}", g[2].1);
        assert!(g[2].0.angle_between(g[3].0) < 1e-3);
        assert!((g[1].1.distance(g[0].1) - 10.0).abs() < 1e-3, "lengths kept");
        let far = (Quat::IDENTITY, Vec3::new(0.0, 30.0, 0.0));
        let mut pose = defaults.clone();
        pose[3] = far;
        set.solve_ik(&mut pose, &[goal]);
        let g = model_space(&pose, &set.parents);
        assert!(g[2].1.distance(Vec3::new(0.0, 18.0, 0.0)) < 0.01, "{:?}", g[2].1);
    }

    /// IK rules with stored places (specs/tf2/player_animation.md 4a):
    /// two grid cells hold the arm's end at different places relative to
    /// bone 3; half-way between them the goal is the blend, the solved
    /// end lands there in bone 3's frame, a release from a later layer
    /// lets it go by its weight, and without `ik_rules` nothing moves.
    #[test]
    fn ik_rules_hold_a_chain_relative_to_a_bone() {
        let x = Vec3::X;
        let defaults = vec![
            (Quat::IDENTITY, Vec3::ZERO),
            (Quat::IDENTITY, x * 10.0),
            (Quat::from_rotation_z(0.3), x * 8.0),
            (Quat::from_rotation_y(0.5), Vec3::new(6.0, 7.0, 1.0)),
        ];
        let place = |p: Vec3| IkTarget { chain: 0, bone: 3, ramp: [0.0, 0.0, 1.0, 1.0], frames: vec![(Quat::IDENTITY, p)] };
        let anim = |t: Option<IkTarget>| Animation { fps: 30.0, frames: 1, delta: true, ik_targets: t.into_iter().collect(), ..default() };
        let sequence = |anims: Vec<usize>, rules: Vec<IkRule>| Sequence {
            delta: true,
            looping: true,
            grid: (anims.len(), 1),
            anims,
            axes: [Axis { param: Some(0), keys: vec![], range: (0.0, 1.0) }, Axis::default()],
            bone_weights: vec![1.0; 4],
            ik_rules: rules,
            ..default()
        };
        let set = Arc::new(AnimSet {
            defaults: defaults.clone(),
            bases: vec![defaults.clone()],
            parents: vec![None, Some(0), Some(1), None],
            ik_chains: vec![IkChain { name: "arm".into(), bones: [0, 1, 2], ..default() }],
            animations: vec![anim(Some(place(Vec3::new(1.0, 2.0, 0.0)))), anim(Some(place(Vec3::new(3.0, 2.0, 0.0)))), anim(None)],
            sequences: vec![sequence(vec![0, 1], vec![]), sequence(vec![2], vec![IkRule { chain: 0, lock: false }])],
            params: vec![PoseParam { name: "p".into(), start: 0.0, end: 1.0, looping: 0.0 }],
            ..default()
        });
        let mut a = Animator::new(set.clone());
        a.main = Some(0);
        a.params[0] = 0.5;
        let end_in_bone = |a: &Animator| {
            let g = model_space(&a.pose(0.0), &set.parents);
            g[3].0.inverse() * (g[2].1 - g[3].1)
        };
        let free = end_in_bone(&a);
        a.ik_rules = true;
        let held = end_in_bone(&a);
        assert!(held.distance(Vec3::new(2.0, 2.0, 0.0)) < 1e-3, "the cells' blend: {held}");
        assert!(free.distance(held) > 1.0, "without ik_rules the end stays: {free}");
        // A release at half weight: half-way back.
        a.layers = vec![Some(Layer { sequence: 1, cycle: 0.0, weight: 0.5 })];
        let half = end_in_bone(&a);
        assert!(half.distance(held) > 0.1 && half.distance(free) > 0.1, "{half}");
    }

    #[test]
    fn euler_matches_the_spec() {
        let q = euler(0.1, 0.2, 0.3);
        let want = Quat::from_xyzw(0.034271, 0.106021, 0.143572, 0.983347);
        assert!(Vec4::from(q).distance(Vec4::from(want)) < 1e-5, "{q}");
    }

    #[test]
    fn fade_weights() {
        let f = Fading {
            sequence: 0,
            cycle: 0.0,
            playback: 1.0,
            stopped: 0.0,
            fade: 0.2,
            held: None,
        };
        assert!((f.weight(0.05) - 0.84375).abs() < 1e-5);
        assert!((f.weight(0.1) - 0.5).abs() < 1e-5);
        assert_eq!(f.weight(0.2), 0.0);
    }
}
