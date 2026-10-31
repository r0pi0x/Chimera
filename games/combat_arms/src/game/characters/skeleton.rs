//! The character's skeleton as posed (specs/combat_arms/characters.md
//! sections 3, 4, 8 and 11): the four trackers layered by the body's
//! weight sets (`Rig::layered`), the aim's bone rotations after the
//! animation (`Posed::aim`), and what sits on the posed nodes: hit-sphere
//! centres (a node of the entry's name, else a socket of it) and sockets.
//! Every pose of a rig shares its names, parents and sockets (`Shape`,
//! made once in `Rig::new`), and names are found through a case-blind
//! index: posing cloned every name and each lookup scanned them all (the
//! largest per-bot cost in a party match: docs/performance.md, "Party
//! frame-rate pass", third pass).
//!
//! Everything here is LithTech's frame: left-handed (+X right, +Y up, +Z
//! forward), world units, model space relative to the hull centre (the
//! object position). `to_body` turns a point into the character's frame
//! as the rest of the game keeps it (right-handed, facing -Z, feet at the
//! origin).
//!
//! Without a body model (no V2 key), `fallback` poses the spec's
//! skeleton: every node, socket sphere and socket at the position the
//! spec measured (`LST` at t = 0 for nodes and spheres, the "stand" pose
//! for sockets), squashed towards the feet by the crouch, with the same
//! aim rotations; orientations stay identity (only positions are known).

use bevy::prelude::*;
use lithtech_formats::{
    anim::{self as lta, AnimList, Tracker},
    ltb,
};

use super::records::SkeletonRecord;

/// The upper-body twist limit and the third-person pitch clamp (degrees).
pub const AIM_LIMIT_DEG: f32 = 45.0;
/// Yaw shares of the seven aim nodes (`PelvisYawRatio`, `LowerYawRatio`,
/// `UpperYawRatio`, then none).
pub const YAW_SHARES: [f32; 7] = [0.0, 7.0 / 17.0, 10.0 / 17.0, 0.0, 0.0, 0.0, 0.0];
/// Pitch shares looking up and looking down, per aim node.
pub const PITCH_UP: [f32; 7] = [0.0, 0.15, 0.15, 0.25, 0.50, 0.45, 0.45];
pub const PITCH_DOWN: [f32; 7] = [0.0, 0.35, 0.45, 0.10, 0.35, 0.10, 0.10];
/// Each aim node's "parent" for the yaw sum (8's table).
pub const AIM_PARENT: [Option<usize>; 7] = [None, Some(0), Some(1), Some(2), Some(3), Some(3), Some(3)];
/// Steps per radian of the legacy aim bytes.
pub const AIM_BYTE_SCALE: f32 = 127.0 / std::f32::consts::FRAC_PI_2;

/// Pitch (radians, positive down) clamped to the third-person limit.
pub fn clamp_aim(theta: f32) -> f32 {
    let limit = AIM_LIMIT_DEG.to_radians();
    theta.clamp(-limit, limit)
}

/// The legacy transport's byte for an aim angle (truncated).
pub fn aim_byte(theta: f32) -> i8 {
    (theta * AIM_BYTE_SCALE).trunc().clamp(-128.0, 127.0) as i8
}

/// The angle a received aim byte stands for.
pub fn from_aim_byte(b: i8) -> f32 {
    b as f32 / AIM_BYTE_SCALE
}

/// A skeleton's names and sockets, shared by every pose of it (a rig's
/// once, `Rig::new`; `fallback`'s per call), with the names looked up
/// case-blind through an index instead of a scan per lookup.
#[derive(Clone, Debug, Default)]
pub struct Shape {
    pub names: Vec<String>,
    pub parents: Vec<Option<usize>>,
    /// Name, parent node, offset and rotation in the parent's space.
    pub sockets: Vec<(String, usize, Vec3, Quat)>,
    /// ASCII-lowercased name -> its first node, its first socket.
    nodes_by_name: bevy::platform::collections::HashMap<String, usize>,
    sockets_by_name: bevy::platform::collections::HashMap<String, usize>,
}

impl Shape {
    pub fn new(names: Vec<String>, parents: Vec<Option<usize>>, sockets: Vec<(String, usize, Vec3, Quat)>) -> Self {
        let index = |names: &mut dyn Iterator<Item = &String>| {
            let mut by = bevy::platform::collections::HashMap::default();
            for (i, n) in names.enumerate() {
                by.entry(n.to_ascii_lowercase()).or_insert(i);
            }
            by
        };
        Self {
            nodes_by_name: index(&mut names.iter()),
            sockets_by_name: index(&mut sockets.iter().map(|s| &s.0)),
            names,
            parents,
            sockets,
        }
    }

    /// A node by name (case ignored, ASCII as `eq_ignore_ascii_case`;
    /// the first of that name).
    pub fn node(&self, name: &str) -> Option<usize> {
        lowered(name, |l| self.nodes_by_name.get(l).copied())
    }

    /// A socket's index by name (case ignored; the first of that name).
    pub fn socket(&self, name: &str) -> Option<usize> {
        lowered(name, |l| self.sockets_by_name.get(l).copied())
    }
}

/// `f` of `name` ASCII-lowercased, without allocating for a short name.
fn lowered<T>(name: &str, f: impl FnOnce(&str) -> T) -> T {
    let b = name.as_bytes();
    let mut buf = [0u8; 64];
    if b.len() > buf.len() {
        return f(&name.to_ascii_lowercase());
    }
    let out = &mut buf[..b.len()];
    out.copy_from_slice(b);
    out.make_ascii_lowercase();
    // ASCII lowercasing keeps UTF-8 valid.
    f(std::str::from_utf8(out).expect("lowercased UTF-8"))
}

/// A posed skeleton: global transforms per node, and the sockets on them.
#[derive(Clone, Debug, Default)]
pub struct Posed {
    /// Names, parents and sockets (shared with the rig's other poses).
    pub shape: std::sync::Arc<Shape>,
    /// Model space (position, rotation).
    pub globals: Vec<(Vec3, Quat)>,
    /// The feet's height in model space.
    pub feet: f32,
}

impl Posed {
    /// A node by name (case ignored).
    pub fn node(&self, name: &str) -> Option<usize> {
        self.shape.node(name)
    }

    /// A socket's transform by name (case ignored).
    pub fn socket(&self, name: &str) -> Option<(Vec3, Quat)> {
        let (_, parent, offset, rotation) = &self.shape.sockets[self.shape.socket(name)?];
        let (p, q) = self.globals[*parent];
        Some((p + q * *offset, q * *rotation))
    }

    /// A hit sphere's centre (11): the node of that name, else the socket.
    pub fn sphere(&self, name: &str) -> Option<Vec3> {
        match self.node(name) {
            Some(j) => Some(self.globals[j].0),
            None => self.socket(name).map(|s| s.0),
        }
    }

    /// A socket the game uses, or (the fallback's points) a node of that
    /// name.
    pub fn place(&self, name: &str) -> Option<(Vec3, Quat)> {
        self.socket(name).or_else(|| self.node(name).map(|j| self.globals[j]))
    }

    fn descends(&self, mut j: usize, k: usize) -> bool {
        while let Some(p) = self.shape.parents[j] {
            if p == k {
                return true;
            }
            j = p;
        }
        false
    }

    /// Section 8: the aim's bone rotations on the seven aim nodes `aim`
    /// (by name; a missing node is skipped), parent before child, each
    /// moving its subtree. `pitch` positive looks down, `twist` positive
    /// turns the upper body right (radians); both clamped to 45 degrees.
    /// The server pitches `Torso` by nothing.
    pub fn aim(&mut self, aim: &[String], pitch: f32, twist: f32, server: bool) {
        let (pitch, twist) = (clamp_aim(pitch), clamp_aim(twist));
        let mut sums = [0.0f32; 7];
        for i in 0..7 {
            let y = YAW_SHARES[i];
            sums[i] = y * twist + AIM_PARENT[i].map_or(0.0, |p| sums[p]);
            let Some(k) = aim.get(i).and_then(|n| self.node(n)) else {
                continue;
            };
            let mut share = if pitch < 0.0 { PITCH_UP[i] } else { PITCH_DOWN[i] };
            if server && i == 1 {
                share = 0.0;
            }
            let axis = Vec3::new(sums[i].cos(), 0.0, -sums[i].sin());
            let turn = Quat::from_axis_angle(axis, pitch * share) * Quat::from_rotation_y(y * twist);
            if turn == Quat::IDENTITY {
                continue;
            }
            let pivot = self.globals[k].0;
            for j in 0..self.globals.len() {
                if j == k || self.descends(j, k) {
                    let (p, q) = self.globals[j];
                    self.globals[j] = (pivot + turn * (p - pivot), (turn * q).normalize());
                }
            }
        }
    }

    /// Each node's pose relative to its parent, right-handed (Chimera's
    /// `BonePose`: rotation, translation), for drawing.
    pub fn locals_rh(&self) -> Vec<(Quat, Vec3)> {
        (0..self.globals.len())
            .map(|j| {
                let (p, q) = self.globals[j];
                let (lp, lq) = match self.shape.parents[j] {
                    Some(parent) => {
                        let (pp, pq) = self.globals[parent];
                        let inv = pq.inverse();
                        (inv * (p - pp), inv * q)
                    }
                    None => (p, q),
                };
                (Quat::from_xyzw(-lq.x, -lq.y, lq.z, lq.w), Vec3::new(lp.x, lp.y, -lp.z))
            })
            .collect()
    }
}

/// A model-space point (hull-centre relative, left-handed) in the
/// character's frame: right-handed, facing -Z, feet at the origin.
pub fn to_body(p: Vec3, feet: f32) -> Vec3 {
    Vec3::new(p.x, p.y - feet, -p.z)
}

/// The stock feet height while standing (the stand animations' root
/// translation).
pub const STAND_FEET: f32 = -50.0;

/// The spec's skeleton as a pose (no model): nodes at their measured
/// `LST` t = 0 positions, socket spheres and sockets as points under
/// their parent, every height squashed towards the feet by `squash` (1
/// standing; the crouched hull's height over the standing one).
pub fn fallback(skel: &SkeletonRecord, squash: f32) -> Posed {
    let (mut names, mut parents, mut globals): (Vec<String>, Vec<Option<usize>>, Vec<(Vec3, Quat)>) = (Vec::new(), Vec::new(), Vec::new());
    let squashed = |a: [f32; 3]| Vec3::new(a[0], STAND_FEET + (a[1] - STAND_FEET) * squash, a[2]);
    let mut push = |name: &str, parent: Option<&str>, at: Option<[f32; 3]>| {
        let parent = parent.and_then(|p| names.iter().position(|n| n.eq_ignore_ascii_case(p)));
        let at = match at {
            Some(a) => squashed(a),
            None => parent.map_or(Vec3::ZERO, |p| globals[p].0),
        };
        names.push(name.to_string());
        parents.push(parent);
        globals.push((at, Quat::IDENTITY));
    };
    for n in &skel.nodes {
        push(&n.name, n.parent.as_deref(), n.at);
    }
    for s in &skel.spheres {
        if let Some(place) = &s.socket {
            push(&s.name, Some(&place.parent), s.at);
        }
    }
    for s in &skel.sockets {
        push(&s.name, Some(&s.parent), s.at);
    }
    Posed {
        shape: std::sync::Arc::new(Shape::new(names, parents, Vec::new())),
        globals,
        feet: STAND_FEET,
    }
}

/// A body model loaded for posing: its nodes, sockets, weight sets and
/// combined animation list.
#[derive(Clone, Debug)]
pub struct Rig {
    pub model: ltb::Model,
    pub list: AnimList,
    /// The `upper` and `Lower` weight sets (case ignored).
    pub upper: Option<usize>,
    pub lower: Option<usize>,
    /// Standing and crouched half-heights (the `LST` and `LC` boxes).
    pub half_heights: (f32, f32),
    /// The model's node and socket names, parents and sockets, as every
    /// pose shares them (`Posed::shape`; made once, not per pose).
    pub shape: std::sync::Arc<Shape>,
}

impl Rig {
    pub fn new(model: ltb::Model, list: AnimList) -> Self {
        let set = |name: &str| model.weight_sets.iter().position(|w| w.name.eq_ignore_ascii_case(name));
        let (stand, crouch) = list.hull_boxes();
        // A body without the player's stand and crouch names whose first
        // animation puts its root deeper than the fallback hull's half
        // height (a boss, characters.md 13.6, Open question 35): its feet
        // where that animation puts them, standing and crouched.
        let deep = (list.find(ltb::STAND_ANIMATION).is_none() || list.find(ltb::CROUCH_ANIMATION).is_none())
            .then(|| list.entries.first().map(|e| -e.translation[1]))
            .flatten()
            .filter(|d| *d > stand[1]);
        let (stand, crouch) = match deep {
            Some(d) => ([stand[0], d, stand[2]], [crouch[0], d, crouch[2]]),
            None => (stand, crouch),
        };
        let shape = Shape::new(
            model.nodes.iter().map(|n| n.name.clone()).collect(),
            model.parents(),
            model
                .sockets
                .iter()
                .map(|s| {
                    let q = Quat::from_array(s.rotation);
                    let q = if q.length_squared() > 0.0 { q.normalize() } else { Quat::IDENTITY };
                    (s.name.clone(), s.node as usize, Vec3::from(s.position), q)
                })
                .collect(),
        );
        Self {
            upper: set("upper"),
            lower: set("lower"),
            half_heights: (stand[1], crouch[1]),
            shape: std::sync::Arc::new(shape),
            model,
            list,
        }
    }

    /// Section 4: the main tracker's pose with the upper, lower and
    /// crouch trackers blended in by their weight sets (the crouch's
    /// multiplier `crouch`). A missing weight set leaves its trackers out.
    pub fn layered(&self, main: &Tracker, upper: &Tracker, lower: &Tracker, crouch: &Tracker, r: f32) -> Vec<(ltb::Vec3, ltb::Quat)> {
        let weight = |set: Option<usize>, j: usize| set.map_or(0.0, |s| self.model.weight_sets[s].weights.get(j).copied().unwrap_or(0.0));
        let base = |t: &Tracker, j: usize| self.list.entries[t.previous].animation.key_value(j, 0);
        (0..self.model.nodes.len())
            .map(|j| {
                let mut pose = main.pose(&self.list, j);
                pose = lta::blend_layer(pose, upper.pose(&self.list, j), weight(self.upper, j), 1.0, base(upper, j));
                pose = lta::blend_layer(pose, lower.pose(&self.list, j), weight(self.lower, j), 1.0, base(lower, j));
                lta::blend_layer(pose, crouch.pose(&self.list, j), weight(self.lower, j), r, base(crouch, j))
            })
            .collect()
    }

    /// The posed skeleton from local poses (`layered`), the feet where the
    /// crouch `r` puts them.
    pub fn posed(&self, pose: &[(ltb::Vec3, ltb::Quat)], r: f32) -> Posed {
        let globals = lta::globals(&self.model, pose)
            .iter()
            .map(|m| {
                let (_, q, p) = Mat4::from_cols_array_2d(m).transpose().to_scale_rotation_translation();
                (p, q.normalize())
            })
            .collect();
        let (hs, hc) = self.half_heights;
        Posed {
            shape: self.shape.clone(),
            globals,
            feet: -(hs + (hc - hs) * r),
        }
    }

    /// The bind pose (every node at its stored bind transform).
    pub fn bind(&self) -> Posed {
        let locals: Vec<(ltb::Vec3, ltb::Quat)> = (0..self.model.nodes.len())
            .map(|j| {
                let m = Mat4::from_cols_array_2d(&lta::bind_local(&self.model, j)).transpose();
                let (_, q, p) = m.to_scale_rotation_translation();
                (p.to_array(), q.to_array())
            })
            .collect();
        self.posed(&locals, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_lookups_match_a_case_blind_scan() {
        let names: Vec<String> = ["Torso", "Left_foot", "left_FOOT", "Héad", "Neck", &"x".repeat(80)]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let sockets = vec![("RightHand".to_string(), 0, Vec3::ZERO, Quat::IDENTITY), ("righthand".to_string(), 1, Vec3::X, Quat::IDENTITY)];
        let shape = Shape::new(names.clone(), vec![None; names.len()], sockets.clone());
        let long = "X".repeat(80);
        for q in ["torso", "TORSO", "Left_Foot", "left_foot", "Héad", "HéAD", "hÉad", "neck", "nope", "", &long] {
            assert_eq!(shape.node(q), names.iter().position(|n| n.eq_ignore_ascii_case(q)), "{q}");
        }
        for q in ["RIGHTHAND", "righthand", "RightHand", "left"] {
            assert_eq!(shape.socket(q), sockets.iter().position(|s| s.0.eq_ignore_ascii_case(q)), "{q}");
        }
    }

    #[test]
    fn aim_bytes() {
        assert_eq!(aim_byte(30f32.to_radians()), 42);
        assert!((from_aim_byte(42).to_degrees() - 29.764).abs() < 1e-3);
        assert_eq!(aim_byte(-45f32.to_radians()), -63);
        assert!((from_aim_byte(-63).to_degrees() + 44.646).abs() < 1e-3);
    }
}
