//! Keyframers and their keys (specs/combat_arms/world_objects.md 9): a
//! `KeyFramer` moves the objects its `ObjectName` names along keys
//! stored as blind data in the world file (`parse_keys`), by the keys'
//! time stamps (time mode) or at a constant path speed (velocity mode,
//! `TotalPathTime` > 0, shaped by its `Wavetype`), along straight or
//! cubic Bézier segments, the rotation interpolated between the keys'.
//!
//! `Keyframer::frame` is one update of its clock and path: which keys it
//! passed (their sound and command for the object world to run) and
//! where the path stands; `placements` puts the controlled objects there
//! with their offsets, and characters standing on a solid world model it
//! moves are carried (10, `movers::Blocking::riders`). The server's object
//! world (`runtime`) and a network client's prediction (`net`) both run
//! these, the client with its own copy of the keyframer and what changes
//! as it runs from the server (`KfMotion`).
//!
//! Known shortcuts: `AlignToPath` faces the motion with up +Y only (the
//! keys' pitch and roll aren't added); the Bézier length is measured by
//! 64 chords (the engine's own subdivision is Open question 7);
//! `PushObjects` teleports like the rest (no stock keyframer pushes);
//! every frame's move carries riders on the server (the jump back to the
//! start of a looping keyframer, or of one switched on again, is that
//! frame's placement too), and so does `GOTO`'s jump (world_objects.md
//! Follow-up (2026-10-31b) 2.3: every teleport carries on the server);
//! a client steps the frames' moves and carries its own player on them
//! even without `PushObjects`, where the original's are teleports a client
//! never carries on (`net`'s Known shortcuts: our prediction).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::{
    geometry::{Waveform, rotation},
    movers::Pose,
};

/// The blind-data chunk id of keyframer keys.
pub const KEY_CHUNK_ID: u32 = 0x6AAF_0884;
/// The keyframers' `SlowOn` additive term (kf_slowon_add).
pub const SLOW_ON_ADD: f32 = 0.003;
/// Ours: chords per Bézier segment for its length.
const CHORDS: usize = 64;

/// One key.
#[derive(Clone, Debug, PartialEq)]
pub struct Key {
    pub pos: Vec3,
    /// Pitch, yaw, roll (radians).
    pub angles: Vec3,
    /// Seconds after the previous key (the first key's: its start time).
    pub stamp: f32,
    pub sound_radius: f32,
    pub sound: String,
    pub command: String,
    /// Tangents relative to the key position.
    pub prev: Option<Vec3>,
    pub next: Option<Vec3>,
}

impl Key {
    pub fn rot(&self) -> Quat {
        rotation(self.angles.x, self.angles.y, self.angles.z)
    }
}

fn f32_at(b: &[u8], at: usize) -> Option<f32> {
    Some(f32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn vec_at(b: &[u8], at: usize) -> Option<Vec3> {
    Some(Vec3::new(f32_at(b, at)?, f32_at(b, at + 4)?, f32_at(b, at + 8)?))
}

/// A keys chunk's keys (9, "Where the keys are"); None when it's cut
/// short.
pub fn parse_keys(b: &[u8]) -> Option<Vec<Key>> {
    let n = u32::from_le_bytes(b.get(0..4)?.try_into().ok()?) as usize;
    let mut at = 4;
    let mut keys = Vec::with_capacity(n.min(4096));
    for _ in 0..n {
        let control = u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?);
        let s = *b.get(at + 2)? as usize;
        let c = *b.get(at + 3)? as usize;
        let pos = vec_at(b, at + 4)?;
        let angles = vec_at(b, at + 16)?;
        let stamp = f32_at(b, at + 28)?;
        let sound_radius = f32_at(b, at + 32)?;
        at += 36;
        let sound = String::from_utf8_lossy(b.get(at..at + s)?).into_owned();
        at += s;
        let command = String::from_utf8_lossy(b.get(at..at + c)?).into_owned();
        at += c;
        let prev = if control & 1 != 0 {
            at += 12;
            Some(vec_at(b, at - 12)?)
        } else {
            None
        };
        let next = if control & 2 != 0 {
            at += 12;
            Some(vec_at(b, at - 12)?)
        } else {
            None
        };
        keys.push(Key {
            pos,
            angles,
            stamp,
            sound_radius,
            sound,
            command,
            prev,
            next,
        });
    }
    Some(keys)
}

/// A key's name: the base followed by `0`, `01`..`09`, then plain
/// decimal.
pub fn key_name(base: &str, i: usize) -> String {
    match i {
        0 => format!("{base}0"),
        1..=9 => format!("{base}0{i}"),
        _ => format!("{base}{i}"),
    }
}

/// The key named `name` (the base compared without case, the number
/// exactly, at most 10 characters after it).
pub fn find_key(base: &str, count: usize, name: &str) -> Option<usize> {
    let head = name.get(..base.len())?;
    if !head.eq_ignore_ascii_case(base) {
        return None;
    }
    let rest = &name[base.len()..];
    if rest.len() > 10 {
        return None;
    }
    (0..count).find(|&i| key_name("", i) == rest)
}

/// The segment from key `a` to key `b`: its point at `k`.
pub fn segment_point(a: &Key, b: &Key, k: f32) -> Vec3 {
    match (a.next, b.prev) {
        (Some(na), Some(pb)) => {
            let (p0, p1, p2, p3) = (a.pos, a.pos + na, b.pos + pb, b.pos);
            let u = 1.0 - k;
            p0 * u * u * u + p1 * 3.0 * u * u * k + p2 * 3.0 * u * k * k + p3 * k * k * k
        }
        _ => a.pos.lerp(b.pos, k),
    }
}

/// Whether the segment is a Bézier.
pub fn is_curve(a: &Key, b: &Key) -> bool {
    a.next.is_some() && b.prev.is_some()
}

/// The segment's length (Bézier: by chords).
pub fn segment_length(a: &Key, b: &Key) -> f32 {
    if !is_curve(a, b) {
        return a.pos.distance(b.pos);
    }
    let mut len = 0.0;
    let mut last = a.pos;
    for i in 1..=CHORDS {
        let p = segment_point(a, b, i as f32 / CHORDS as f32);
        len += p.distance(last);
        last = p;
    }
    len
}

/// A keyframer's state and properties.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Keyframer {
    pub keys: Vec<Key>,
    pub base: String,
    pub object_names: Vec<String>,
    pub looping: bool,
    pub align: bool,
    pub ignore_offsets: bool,
    pub push: bool,
    pub total_time: f32,
    pub wave: Waveform,
    pub target_name: String,
    pub target_offset: Vec3,
    pub active_sound: String,
    pub sound_radius: f32,
    pub start_active: bool,
    pub start_paused: bool,
    // State.
    pub built: bool,
    pub active: bool,
    pub paused: bool,
    pub finished: bool,
    pub forward: bool,
    /// The clock (s).
    pub t: f32,
    /// The next key to pass (in the direction of travel).
    pub next: usize,
    /// Velocity mode: the path stands exactly on key `next`.
    pub at_key: bool,
    /// Velocity mode: the fraction along the current segment.
    pub k: f32,
    /// `MOVETO`'s key and command.
    pub dest: Option<(usize, String)>,
    /// Messages received before the first update.
    pub queued: Vec<String>,
    /// A `GOTO` snap: the next placing snaps the rotation.
    pub snap: bool,
    /// Each controlled object's offset from key 0 (its id, position, and
    /// quaternion difference component-wise, Quirk 7), taken at the
    /// first update.
    pub offsets: Vec<(u32, Vec3, Vec4)>,
}

/// What one frame of a keyframer did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KfFrame {
    /// Keys passed this frame, in order (their sound and command run).
    pub passed: Vec<usize>,
    /// Where the path stands (position, rotation), when the objects move.
    pub place: Option<(Vec3, Quat)>,
    /// The `MOVETO` destination reached: its command.
    pub arrived: Option<String>,
    /// The list ended (the keyframer goes back to its start).
    pub ended: bool,
}

/// What changes in a keyframer as it runs (its clock, path place,
/// direction, destination, target and offsets): with the map's keyframer
/// (the same keys and properties), a network client steps it as the
/// server does (`net`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct KfMotion {
    pub active: bool,
    pub paused: bool,
    pub finished: bool,
    pub forward: bool,
    pub t: f32,
    pub next: u32,
    pub at_key: bool,
    pub k: f32,
    pub dest: Option<(u32, String)>,
    pub snap: bool,
    pub target_name: String,
    pub target_offset: Vec3,
    pub offsets: Vec<(u32, Vec3, Vec4)>,
}

/// A rotation facing `d` with up +Y.
pub fn look(d: Vec3) -> Quat {
    let f = d.normalize_or_zero();
    if f == Vec3::ZERO {
        return Quat::IDENTITY;
    }
    let yaw = f.x.atan2(f.z);
    let pitch = -f.y.asin();
    rotation(pitch, yaw, 0.0)
}

/// Where keyframer `k` puts its controlled `objects` (ids and current
/// poses, in order) for the path point `pos`, `rot` (9, "Placing"): the
/// offsets added unless `IgnoreOffsets`; facing `target_at` (the target's
/// position plus `TargetOffset`) when it has one, else the motion with
/// `AlignToPath`, else the keys' rotation plus the rotation offset.
pub fn placements(k: &Keyframer, pos: Vec3, rot: Quat, objects: &[(u32, Pose)], target_at: Option<Vec3>) -> Vec<Pose> {
    objects
        .iter()
        .map(|(o, old)| {
            let (off, roff) = k.offsets.iter().find(|x| x.0 == *o).map_or((Vec3::ZERO, Vec4::ZERO), |x| (x.1, x.2));
            let at = if k.ignore_offsets { pos } else { pos + off };
            let r = if let Some(t) = target_at {
                look(t - at)
            } else if k.align {
                let d = at - old.pos;
                if d.length_squared() > 1e-8 { look(d) } else { old.rot }
            } else if k.ignore_offsets {
                rot
            } else {
                Quat::from_vec4(Vec4::from(rot) + roff).normalize()
            };
            Pose { pos: at, rot: r, ..*old }
        })
        .collect()
}

impl Keyframer {
    /// Whether its update moves its objects (built, active, not paused).
    pub fn running(&self) -> bool {
        self.built && self.active && !self.paused
    }

    /// What changes as it runs (`KfMotion`).
    pub fn motion(&self) -> KfMotion {
        KfMotion {
            active: self.active,
            paused: self.paused,
            finished: self.finished,
            forward: self.forward,
            t: self.t,
            next: self.next as u32,
            at_key: self.at_key,
            k: self.k,
            dest: self.dest.as_ref().map(|(i, c)| (*i as u32, c.clone())),
            snap: self.snap,
            target_name: self.target_name.clone(),
            target_offset: self.target_offset,
            offsets: self.offsets.clone(),
        }
    }

    /// Take `m` as what changes as it runs (a built keyframer's).
    pub fn set_motion(&mut self, m: &KfMotion) {
        self.built = true;
        self.queued.clear();
        self.active = m.active;
        self.paused = m.paused;
        self.finished = m.finished;
        self.forward = m.forward;
        self.t = m.t;
        self.next = m.next as usize;
        self.at_key = m.at_key;
        self.k = m.k;
        self.dest = m.dest.as_ref().map(|(i, c)| (*i as usize, c.clone()));
        self.snap = m.snap;
        self.target_name = m.target_name.clone();
        self.target_offset = m.target_offset;
        self.offsets = m.offsets.clone();
    }

    /// Each key's absolute time.
    pub fn times(&self) -> Vec<f32> {
        let mut t = 0.0;
        self.keys
            .iter()
            .map(|k| {
                t += k.stamp;
                t
            })
            .collect()
    }

    /// The path's length (velocity mode).
    pub fn length(&self) -> f32 {
        self.keys.windows(2).map(|w| segment_length(&w[0], &w[1])).sum()
    }

    fn velocity_mode(&self) -> bool {
        self.total_time > 0.0
    }

    /// The key after `i` in the direction of travel.
    fn step(&self, i: usize) -> Option<usize> {
        if self.forward {
            (i + 1 < self.keys.len()).then_some(i + 1)
        } else {
            i.checked_sub(1)
        }
    }

    /// The key before `next` in the direction of travel (the segment's
    /// start).
    fn before(&self, next: usize) -> Option<usize> {
        if self.forward {
            next.checked_sub(1)
        } else {
            (next + 1 < self.keys.len()).then_some(next + 1)
        }
    }

    /// Back to the start of the direction of travel (clock and position).
    pub fn reset(&mut self) {
        let times = self.times();
        if self.forward {
            self.t = 0.0;
            self.next = 0;
        } else {
            self.t = times.last().copied().unwrap_or(0.0);
            self.next = self.keys.len().saturating_sub(1);
        }
        self.at_key = true;
        self.k = 0.0;
    }

    /// Go active (from inactive: back to the start).
    pub fn activate(&mut self) {
        if !self.active {
            self.reset();
        }
        self.active = true;
        self.paused = false;
        self.finished = false;
    }

    /// Change direction: the segment's next key becomes the one behind.
    pub fn toggle_dir(&mut self) {
        let behind = self.before(self.next);
        self.forward = !self.forward;
        if let Some(b) = behind
            && !self.at_key
        {
            self.next = b;
            self.k = (1.0 - self.k).clamp(0.0, 1.0);
        }
    }

    /// `GOTO`: the clock to key `i`'s time, at that key.
    pub fn goto(&mut self, i: usize) {
        let times = self.times();
        self.t = times.get(i).copied().unwrap_or(0.0);
        self.next = i;
        self.at_key = true;
        self.k = 0.0;
        self.active = true;
        self.paused = false;
        self.finished = false;
        self.snap = true;
    }

    /// The path point between `a` and `b` at `k`, with the rotation.
    fn point(&self, a: usize, b: usize, k: f32) -> (Vec3, Quat) {
        let (ka, kb) = (&self.keys[a], &self.keys[b]);
        (segment_point(ka, kb, k), ka.rot().slerp(kb.rot(), k))
    }

    /// Pass key `i`: the segment advances; the end of the list.
    fn pass(&mut self, i: usize, out: &mut KfFrame) -> bool {
        out.passed.push(i);
        if let Some((d, cmd)) = self.dest.clone()
            && d == i
        {
            self.dest = None;
            self.paused = true;
            out.arrived = Some(cmd);
        }
        match self.step(i) {
            Some(n) => {
                self.next = n;
                false
            }
            None => {
                out.ended = true;
                true
            }
        }
    }

    /// One update while active and not paused, with frame time `dt`.
    pub fn frame(&mut self, dt: f32) -> KfFrame {
        let mut out = KfFrame::default();
        if self.keys.is_empty() {
            self.active = false;
            return out;
        }
        let times = self.times();
        let last = if self.forward { self.keys.len() - 1 } else { 0 };
        if self.velocity_mode() {
            if self.at_key {
                let i = self.next;
                self.at_key = false;
                if self.pass(i, &mut out) {
                    out.place = Some((self.keys[last].pos, self.keys[last].rot()));
                    self.end();
                    return out;
                }
                if self.paused {
                    out.place = Some((self.keys[i].pos, self.keys[i].rot()));
                    return out;
                }
            }
            let Some(a) = self.before(self.next) else { return out };
            let b = self.next;
            let total = self.total_time;
            let q = (self.t.abs() / total).clamp(0.0, 1.0);
            let q = if self.forward { q } else { 1.0 - q };
            let v = self.length() / total;
            let d = self.wave.speed(v, q, SLOW_ON_ADD, 0.0) * dt;
            let (ka, kb) = (&self.keys[a], &self.keys[b]);
            let l = segment_length(ka, kb);
            let mut k = if l <= 1e-4 { 1.0 } else { self.k + d / l };
            if is_curve(ka, kb) && k < 1.0 && d > 0.0 {
                let covered = segment_point(ka, kb, self.k).distance(segment_point(ka, kb, k));
                if covered > 1e-6 {
                    k = self.k + (k - self.k) * d / covered;
                }
            }
            let ends = (self.forward && self.t + 1e-4 >= total) || (!self.forward && self.t <= 1e-4);
            if k >= 1.0 || ends {
                self.k = if k >= 1.0 { k - 1.0 } else { 0.0 };
                self.at_key = true;
                out.place = Some((kb.pos, kb.rot()));
            } else {
                self.k = k;
                out.place = Some(self.point(a, b, k));
            }
        } else {
            // Time mode: every key the clock reached.
            loop {
                let i = self.next;
                let reached = if self.forward { times[i] <= self.t + 1e-4 } else { times[i] + 1e-4 >= self.t };
                if !reached {
                    break;
                }
                if self.pass(i, &mut out) {
                    out.place = Some((self.keys[last].pos, self.keys[last].rot()));
                    self.end();
                    return out;
                }
                if self.paused {
                    break;
                }
            }
            if let Some(a) = self.before(self.next) {
                let b = self.next;
                let (ta, tb) = (times[a], times[b]);
                let (lo, hi) = (ta.min(tb), ta.max(tb));
                if lo <= self.t && self.t <= hi {
                    let k = if (tb - ta).abs() < 0.001 { 0.0 } else { (self.t - ta) / (tb - ta) };
                    out.place = Some(self.point(a, b, k));
                }
            }
        }
        if !self.paused {
            self.t += if self.forward { dt } else { -dt };
        }
        out
    }

    /// The end of the keys: back to the start; stopped unless looping.
    fn end(&mut self) {
        self.reset();
        if !self.looping {
            self.active = false;
            self.finished = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(pos: Vec3, stamp: f32) -> Key {
        Key {
            pos,
            angles: Vec3::ZERO,
            stamp,
            sound_radius: 0.0,
            sound: String::new(),
            command: String::new(),
            prev: None,
            next: None,
        }
    }

    /// Test case 15: key names.
    #[test]
    fn key_names() {
        assert_eq!(find_key("Lift", 20, "Lift0"), Some(0));
        assert_eq!(find_key("Lift", 20, "Lift01"), Some(1));
        assert_eq!(find_key("Lift", 20, "Lift1"), None);
        assert_eq!(find_key("Lift", 20, "Lift10"), Some(10));
        assert_eq!(find_key("Lift", 20, "lift05"), Some(5));
    }

    /// Test case 13: time mode.
    #[test]
    fn time_mode() {
        let mut kf = Keyframer {
            keys: vec![
                key(Vec3::ZERO, 0.0),
                key(Vec3::new(100.0, 0.0, 0.0), 1.0),
                key(Vec3::new(100.0, 0.0, 200.0), 2.0),
            ],
            ignore_offsets: true,
            forward: true,
            ..default()
        };
        kf.activate();
        let mut at = Vec::new();
        for _ in 0..80 {
            if !kf.active {
                break;
            }
            let t = kf.t;
            let f = kf.frame(0.05);
            at.push((t, f.place.map(|p| p.0), f.ended));
        }
        let find = |t: f32| at.iter().find(|(c, ..)| (c - t).abs() < 1e-3).unwrap();
        assert!(find(0.5).1.unwrap().distance(Vec3::new(50.0, 0.0, 0.0)) < 1e-2);
        assert!(find(2.0).1.unwrap().distance(Vec3::new(100.0, 0.0, 100.0)) < 1e-2);
        let end = find(3.0);
        assert!(end.2 && end.1.unwrap().distance(Vec3::new(100.0, 0.0, 200.0)) < 1e-3);
        assert!(!kf.active && kf.finished);
    }
}
