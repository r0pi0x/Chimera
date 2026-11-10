//! LithTech skeletal animation as data and playback, per
//! specs/combat_arms/animation.md (section names below are the spec's):
//! the maths it defines (quaternion matrix, slerp, the node transform),
//! a model's combined animation list (`AnimList`, "Building a model's
//! animation list"), sampling one animation ("Sampling one animation"),
//! the millisecond tracker with its key scan, looping, end of play and
//! transitions (`Tracker`, "Playback"), key strings split into commands
//! ("String keys"), layered blending ("Layered playback") and node
//! evaluation and skinning matrices ("Node evaluation order", "Skinning").
//! Plain data: the model's bytes are `crate::ltb`'s; a game drives
//! trackers with its own clock and converts poses into its engine.
//!
//! Everything stays in the file's frame (left-handed, +Y up, world
//! units, ms); `ltb::to_rh_*` converts.
//!
//! Known shortcuts:
//! - `AniRateInterpolation` is always 1 (the carry is kept).
//! - At most 8 trackers and the weight-set validity rules ("Layered
//!   playback") are the caller's: `blend_layer` blends one tracker's
//!   pose into another for one node.
//! - Quirk 5 (an invalid weight set freezes the whole pose) is not
//!   reproduced; the spec says to log and skip instead, which is the
//!   caller's.
//! - Vertex animation (the mesh positions of vertex-animated pieces) is
//!   sampled by `vertex_frame`; nothing here builds the mesh.
//! - `AnimList::find` looks names up in an index made on its first call
//!   (a character driver finds several names a tick per body); entries
//!   renamed in place after that aren't seen (a change in their count
//!   falls back to a scan). Lists are built once and then shared.

use crate::ltb::{Animation, Mat4, Model, NODE_ROTATION_ONLY, Quat, Vec3};

/// "Constants".
pub const POS16_SCALE: f32 = 1.0 / 16.0;
pub const QUAT16_SCALE: f32 = 1.0 / 32767.0;
/// "Constants": a first-person weapon rate above this becomes 1.0 (the
/// game's rule; `weapon_rate` applies it).
pub const RATE_CAP: f32 = 2.0;
/// "Constants": at most this many trackers take part in a pose.
pub const MAX_TRACKERS: usize = 8;
/// "Layered playback": a weight-set value meaning "additive".
pub const ADDITIVE_WEIGHT: f32 = 2.0;

pub const IDENTITY_QUAT: Quat = [0.0, 0.0, 0.0, 1.0];
pub const IDENTITY: Mat4 = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];

/// "Units and conventions": R(q), with s = 2 / |q|^2 so non-unit
/// quaternions give a proper rotation (identity for a zero quaternion).
pub fn rotation(q: Quat) -> [[f32; 3]; 3] {
    let [x, y, z, w] = q;
    let n = x * x + y * y + z * z + w * w;
    if n == 0.0 {
        return [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    }
    let s = 2.0 / n;
    [
        [1.0 - s * (y * y + z * z), s * (x * y - w * z), s * (x * z + w * y)],
        [s * (x * y + w * z), 1.0 - s * (x * x + z * z), s * (y * z - w * x)],
        [s * (x * z - w * y), s * (y * z + w * x), 1.0 - s * (x * x + y * y)],
    ]
}

/// "Node evaluation order": the local matrix of a pose (R(q) and p).
pub fn matrix(p: Vec3, q: Quat) -> Mat4 {
    let r = rotation(q);
    [
        [r[0][0], r[0][1], r[0][2], p[0]],
        [r[1][0], r[1][1], r[1][2], p[1]],
        [r[2][0], r[2][1], r[2][2], p[2]],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

pub fn mul(a: &Mat4, b: &Mat4) -> Mat4 {
    // Each element's four products added in order from the left, as the
    // sum over k it was (`-0.0 + x` is x: a float sum starts at -0.0), so
    // the bits are the same; written out, the rows vectorize. A posed
    // skeleton multiplies one of these per node per tick
    // (docs/performance.md, "Performance pass").
    let row = |r: usize| -> [f32; 4] {
        let [a0, a1, a2, a3] = a[r];
        let mut o = [0.0; 4];
        for (c, v) in o.iter_mut().enumerate() {
            *v = -0.0 + a0 * b[0][c] + a1 * b[1][c] + a2 * b[2][c] + a3 * b[3][c];
        }
        o
    };
    [row(0), row(1), row(2), row(3)]
}

/// Inverse of an affine matrix (rotation and scale, then translation;
/// last row (0, 0, 0, 1)).
pub fn inverse_affine(m: &Mat4) -> Mat4 {
    let a = |r: usize, c: usize| m[r][c] as f64;
    let det = a(0, 0) * (a(1, 1) * a(2, 2) - a(1, 2) * a(2, 1)) - a(0, 1) * (a(1, 0) * a(2, 2) - a(1, 2) * a(2, 0))
        + a(0, 2) * (a(1, 0) * a(2, 1) - a(1, 1) * a(2, 0));
    if det == 0.0 {
        return IDENTITY;
    }
    let inv = 1.0 / det;
    let mut r = [[0.0f64; 3]; 3];
    r[0][0] = (a(1, 1) * a(2, 2) - a(1, 2) * a(2, 1)) * inv;
    r[0][1] = (a(0, 2) * a(2, 1) - a(0, 1) * a(2, 2)) * inv;
    r[0][2] = (a(0, 1) * a(1, 2) - a(0, 2) * a(1, 1)) * inv;
    r[1][0] = (a(1, 2) * a(2, 0) - a(1, 0) * a(2, 2)) * inv;
    r[1][1] = (a(0, 0) * a(2, 2) - a(0, 2) * a(2, 0)) * inv;
    r[1][2] = (a(0, 2) * a(1, 0) - a(0, 0) * a(1, 2)) * inv;
    r[2][0] = (a(1, 0) * a(2, 1) - a(1, 1) * a(2, 0)) * inv;
    r[2][1] = (a(0, 1) * a(2, 0) - a(0, 0) * a(2, 1)) * inv;
    r[2][2] = (a(0, 0) * a(1, 1) - a(0, 1) * a(1, 0)) * inv;
    let t = [a(0, 3), a(1, 3), a(2, 3)];
    let mut o = IDENTITY;
    for i in 0..3 {
        for j in 0..3 {
            o[i][j] = r[i][j] as f32;
        }
        o[i][3] = -(r[i][0] * t[0] + r[i][1] * t[1] + r[i][2] * t[2]) as f32;
    }
    o
}

/// M p for a point.
pub fn transform_point(m: &Mat4, p: Vec3) -> Vec3 {
    std::array::from_fn(|r| m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2] + m[r][3])
}

/// M n for a direction (rotation part only).
pub fn transform_vector(m: &Mat4, n: Vec3) -> Vec3 {
    std::array::from_fn(|r| m[r][0] * n[0] + m[r][1] * n[1] + m[r][2] * n[2])
}

pub fn lerp3(a: Vec3, b: Vec3, u: f32) -> Vec3 {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * u)
}

/// "Sampling one animation" step 4: slerp without renormalising.
pub fn slerp(a: Quat, b: Quat, u: f32) -> Quat {
    let mut c = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
    let mut b = b;
    if c < 0.0 {
        c = -c;
        b = b.map(|v| -v);
    }
    let (wa, wb) = if 1.0 - c > 0.0001 {
        let omega = c.min(1.0).acos();
        let s = omega.sin();
        (((1.0 - u) * omega).sin() / s, (u * omega).sin() / s)
    } else {
        (1.0 - u, u)
    };
    std::array::from_fn(|i| wa * a[i] + wb * b[i])
}

/// Hamilton product p q.
pub fn quat_mul(p: Quat, q: Quat) -> Quat {
    let [x1, y1, z1, w1] = p;
    let [x2, y2, z2, w2] = q;
    [
        w1 * x2 + x1 * w2 + y1 * z2 - z1 * y2,
        w1 * y2 - x1 * z2 + y1 * w2 + z1 * x2,
        w1 * z2 + x1 * y2 - y1 * x2 + z1 * w2,
        w1 * w2 - x1 * x2 - y1 * y2 - z1 * z2,
    ]
}

/// The conjugate (the spec's q^-1).
pub fn conjugate(q: Quat) -> Quat {
    [-q[0], -q[1], -q[2], q[3]]
}

/// One entry of a model's playable list: which animation, and the box
/// and root translation the bindings give it.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// Shared with the model it came from and every list built from that
    /// model: a character's animations are megabytes, and bodies share them.
    pub animation: std::sync::Arc<Animation>,
    /// The child slot it came from (0 = the model itself).
    pub child: usize,
    pub dims: Vec3,
    pub translation: Vec3,
}

impl Entry {
    pub fn name(&self) -> &str {
        &self.animation.name
    }

    /// L, ms.
    pub fn length(&self) -> u32 {
        self.animation.length()
    }
}

/// "Building a model's animation list": the model's own animations, then
/// each loaded child's, in child order.
#[derive(Clone, Debug, Default)]
pub struct AnimList {
    pub entries: Vec<Entry>,
    /// Per child slot, its first entry ("animation offset"); None for a
    /// child that did not load.
    pub offsets: Vec<Option<usize>>,
    /// `find`'s index, made on its first call: the entry count it was made
    /// for and each ASCII-lowercased name's first entry (a game's
    /// character driver looks names up several times a tick per body).
    pub by_name: std::sync::OnceLock<(usize, std::collections::HashMap<String, usize>)>,
}

impl PartialEq for AnimList {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries && self.offsets == other.offsets
    }
}

impl AnimList {
    /// `children[i]` is child slot i + 1 (None: not loaded, its
    /// animations absent).
    pub fn build(model: &Model, children: &[Option<&Model>]) -> AnimList {
        let mut list = AnimList::default();
        let own = |m: &Model, child: usize| {
            (0..m.animations.len())
                .map(|i| Entry {
                    animation: m.animations[i].clone(),
                    child,
                    dims: m.animation_dims(i),
                    translation: m.animation_translation(i),
                })
                .collect::<Vec<_>>()
        };
        list.offsets.push(Some(0));
        list.entries.extend(own(model, 0));
        for (i, child) in children.iter().enumerate() {
            let slot = i + 1;
            let Some(child) = child else {
                list.offsets.push(None);
                continue;
            };
            let start = list.entries.len();
            list.offsets.push(Some(start));
            let mut entries = own(child, slot);
            // Step 3: the parent's slot for this child overrides by name.
            for b in model.bindings.get(slot).into_iter().flatten() {
                if let Some(e) = entries.iter_mut().find(|e| e.name().eq_ignore_ascii_case(&b.name)) {
                    e.dims = b.dims;
                    e.translation = b.translation;
                }
            }
            list.entries.extend(entries);
        }
        list
    }

    /// The first entry named `name` (case ignored, ASCII). Through an
    /// index made on the first call; a list whose entry count changed
    /// since is scanned instead (entries renamed in place after a `find`
    /// would not be seen: lists are built once, then shared).
    pub fn find(&self, name: &str) -> Option<usize> {
        let (n, index) = self.by_name.get_or_init(|| {
            let mut index = std::collections::HashMap::new();
            for (i, e) in self.entries.iter().enumerate() {
                index.entry(e.name().to_ascii_lowercase()).or_insert(i);
            }
            (self.entries.len(), index)
        });
        if *n != self.entries.len() {
            return self.entries.iter().position(|e| e.name().eq_ignore_ascii_case(name));
        }
        let b = name.as_bytes();
        let mut buf = [0u8; 64];
        if b.len() > buf.len() {
            return index.get(&name.to_ascii_lowercase()).copied();
        }
        let lower = &mut buf[..b.len()];
        lower.copy_from_slice(b);
        lower.make_ascii_lowercase();
        // ASCII lowercasing keeps UTF-8 valid.
        index.get(std::str::from_utf8(lower).ok()?).copied()
    }

    /// The stand and crouch half-extents ("Bounds and dimensions"): the
    /// `LST` and `LC` entries' boxes, or the fallbacks if either is missing.
    pub fn hull_boxes(&self) -> (Vec3, Vec3) {
        match (self.find(crate::ltb::STAND_ANIMATION), self.find(crate::ltb::CROUCH_ANIMATION)) {
            (Some(s), Some(c)) => (self.entries[s].dims, self.entries[c].dims),
            _ => (crate::ltb::FALLBACK_STAND, crate::ltb::FALLBACK_CROUCH),
        }
    }
}

/// "Sampling one animation" steps 1-2: the keys around `t` and the
/// fraction between them.
pub fn key_span(anim: &Animation, t: f32) -> (usize, usize, f32) {
    let n = anim.keys.len();
    if n == 0 || t <= 0.0 {
        return (0, 0, 0.0);
    }
    let Some(a) = anim.keys.iter().rposition(|k| (k.time as f32) < t) else {
        return (0, 0, 0.0);
    };
    let b = (a + 1).min(n - 1);
    let (ta, tb) = (anim.keys[a].time as f32, anim.keys[b].time as f32);
    let u = if tb > ta { (t - ta) / (tb - ta) } else { 0.0 };
    (a, b, u)
}

/// Node `node`'s pose between keys `a` and `b` at fraction `u`
/// (step 3; no root translation).
pub fn sample_keys(anim: &Animation, node: usize, a: usize, b: usize, u: f32) -> (Vec3, Quat) {
    let (pa, qa) = anim.key_value(node, a);
    let (pb, qb) = anim.key_value(node, b);
    (lerp3(pa, pb, u), slerp(qa, qb, u))
}

/// Node `node`'s pose in `entry` at `t` ms, with the root translation
/// on node 0 (step 5).
pub fn sample(entry: &Entry, node: usize, t: f32) -> (Vec3, Quat) {
    let (a, b, u) = key_span(&entry.animation, t);
    let (mut p, q) = sample_keys(&entry.animation, node, a, b, u);
    if node == 0 {
        p = add(p, entry.translation);
    }
    (p, q)
}

/// "Vertex animation": node `node`'s vertex positions between keys `a`
/// and `b` (the previous list for both when `b`'s is missing); None if
/// the node is not vertex-animated.
pub fn vertex_frame(anim: &Animation, node: usize, a: usize, b: usize, u: f32) -> Option<Vec<Vec3>> {
    let crate::ltb::Channel::Vertices(frames) = anim.channels.get(node)? else {
        return None;
    };
    let fa = frames.get(a)?;
    let fb = frames.get(b).unwrap_or(fa);
    Some(
        fa.iter()
            .enumerate()
            .map(|(i, &p)| lerp3(p, fb.get(i).copied().unwrap_or(p), u))
            .collect(),
    )
}

fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// "Playback": one tracker's state.
#[derive(Clone, Debug, PartialEq)]
pub struct Tracker {
    /// Current entry.
    pub current: usize,
    /// Previous entry (equal to `current` unless a transition runs).
    pub previous: usize,
    /// The previous entry's key b when it was interrupted.
    pub previous_key: usize,
    /// Current time, ms.
    pub time: u32,
    pub a: usize,
    pub b: usize,
    pub u: f32,
    /// The next key the scan visits.
    pub next_key: usize,
    pub playing: bool,
    pub looping: bool,
    pub rate: f32,
    carry: f64,
    /// Transition elapsed ms, or None when not transitioning.
    pub transition: Option<u32>,
    /// The transition fraction.
    pub blend: f32,
    /// "Layered playback": its weight set (index into the model's) and
    /// multiplier.
    pub weight_set: Option<usize>,
    pub multiplier: f32,
}

impl Default for Tracker {
    /// "A new tracker starts on entry 0, playing, looping, at t = 0, not
    /// transitioning."
    fn default() -> Self {
        Tracker {
            current: 0,
            previous: 0,
            previous_key: 0,
            time: 0,
            a: 0,
            b: 0,
            u: 0.0,
            next_key: 0,
            playing: true,
            looping: true,
            rate: 1.0,
            carry: 0.0,
            transition: None,
            blend: 0.0,
            weight_set: None,
            multiplier: 1.0,
        }
    }
}

impl Tracker {
    /// A tracker already playing `entry` from t = 0 (no transition).
    pub fn on(entry: usize, looping: bool) -> Self {
        Tracker {
            current: entry,
            previous: entry,
            looping,
            ..Default::default()
        }
    }

    /// Set the rate (resets the carry).
    pub fn set_rate(&mut self, rate: f32) {
        self.rate = rate;
        self.carry = 0.0;
    }

    /// "Starting an animation and transitions" step 1 (the game always
    /// asks for a transition).
    pub fn start(&mut self, entry: usize) {
        self.previous = self.current;
        self.previous_key = self.b;
        self.current = entry;
        self.time = 0;
        (self.a, self.b, self.u) = (0, 0, 0.0);
        self.next_key = 0;
        self.transition = Some(0);
        self.blend = 0.0;
    }

    /// "Reset": restart the current entry; a running transition's
    /// elapsed time is left alone.
    pub fn reset(&mut self) {
        self.previous = self.current;
        self.previous_key = 0;
        self.time = 0;
        (self.a, self.b, self.u) = (0, 0, 0.0);
        self.next_key = 0;
    }

    /// Set the time directly: re-scan from key 0 without firing strings.
    pub fn set_time(&mut self, list: &AnimList, t: u32) {
        self.time = t;
        self.next_key = 0;
        (self.a, self.b, self.u) = (0, 0, 0.0);
        self.scan(list, &mut |_| {});
    }

    /// "End of play": non-looping and past the last key; an entry with
    /// at most one key is always done.
    pub fn done(&self, list: &AnimList) -> bool {
        let anim = &list.entries[self.current].animation;
        anim.keys.len() <= 1 || (!self.looping && self.time > anim.length())
    }

    /// One update with Δ ms of frame time: rate step, the transition
    /// step, the key scan (each non-empty key string passed to `fire`),
    /// the loop wrap and the fraction.
    pub fn update(&mut self, list: &AnimList, delta_ms: u32, fire: &mut dyn FnMut(&str)) {
        if !self.playing || delta_ms == 0 {
            return;
        }
        let scaled = delta_ms as f64 * self.rate as f64 + self.carry;
        let mut step = scaled.floor().max(0.0);
        self.carry = scaled - step;
        if let Some(elapsed) = self.transition {
            let interp = list.entries[self.current].animation.interp_ms;
            if interp == 0 {
                self.transition = None;
                self.previous = self.current;
            } else {
                let elapsed = elapsed.saturating_add(step as u32);
                if elapsed > interp {
                    self.transition = None;
                    self.previous = self.current;
                    self.time = 0;
                    (self.a, self.b, self.u) = (0, 0, 0.0);
                    self.next_key = 0;
                    step = (elapsed - interp) as f64;
                } else {
                    self.transition = Some(elapsed);
                    self.blend = elapsed as f32 / interp as f32;
                    return;
                }
            }
        }
        self.advance(list, step as u32, fire);
    }

    /// "Key scan" steps 1-5 with the rate-scaled step.
    fn advance(&mut self, list: &AnimList, step: u32, fire: &mut dyn FnMut(&str)) {
        let anim = &list.entries[self.current].animation;
        if anim.keys.len() <= 1 {
            self.time = 0;
            self.u = 0.0;
            return;
        }
        self.time = self.time.saturating_add(step);
        if !self.looping {
            self.time = self.time.min(anim.length() + 1);
        }
        self.scan(list, fire);
    }

    /// Steps 3-5 from the current time.
    fn scan(&mut self, list: &AnimList, fire: &mut dyn FnMut(&str)) {
        let anim = &list.entries[self.current].animation;
        let n = anim.keys.len();
        if n <= 1 {
            self.time = 0;
            self.u = 0.0;
            return;
        }
        while self.next_key < n && anim.keys[self.next_key].time < self.time {
            let i = self.next_key;
            if !anim.keys[i].string.is_empty() {
                fire(&anim.keys[i].string);
            }
            self.a = i;
            self.b = (i + 1).min(n - 1);
            self.next_key += 1;
        }
        let length = anim.length();
        if self.looping && self.next_key == n {
            self.time = if length == 0 { 0 } else { self.time % length };
            self.next_key = 0;
        }
        self.u = if self.a != self.b {
            let (ta, tb) = (anim.keys[self.a].time, anim.keys[self.b].time);
            if tb > ta {
                (self.time as f32 - ta as f32) / (tb - ta) as f32
            } else {
                0.0
            }
        } else if self.looping {
            if length == 0 { 0.0 } else { (self.time as f32 / length as f32).min(1.0) }
        } else {
            1.0
        };
    }

    /// Node `node`'s pose from this tracker: during a transition, the
    /// previous entry at its next key blended with the new one at key 0;
    /// otherwise the current entry between keys a and b. The root
    /// translation is added on node 0.
    pub fn pose(&self, list: &AnimList, node: usize) -> (Vec3, Quat) {
        let cur = &list.entries[self.current];
        if self.transition.is_some() {
            let prev = &list.entries[self.previous];
            let (pp, qp) = prev.animation.key_value(node, self.previous_key);
            let (pc, qc) = cur.animation.key_value(node, 0);
            let mut p = lerp3(pp, pc, self.blend);
            if node == 0 {
                p = add(p, lerp3(prev.translation, cur.translation, self.blend));
            }
            return (p, slerp(qp, qc, self.blend));
        }
        let (mut p, q) = sample_keys(&cur.animation, node, self.a, self.b, self.u);
        if node == 0 {
            p = add(p, cur.translation);
        }
        (p, q)
    }
}

/// The game's first-person rule: a rate above 2.0 becomes 1.0.
pub fn weapon_rate(rate: f32) -> f32 {
    if rate > RATE_CAP { 1.0 } else { rate }
}

/// "Layered playback": blend a further tracker's pose (`layer`) for one
/// node into `base`, with its weight-set weight `w` and multiplier `m`;
/// `additive_base` is the tracker's previous entry at key 0, used when
/// `w` is 2.0.
pub fn blend_layer(base: (Vec3, Quat), layer: (Vec3, Quat), w: f32, m: f32, additive_base: (Vec3, Quat)) -> (Vec3, Quat) {
    let (p, q) = base;
    let (pt, qt) = layer;
    if m == 0.0 || w == 0.0 {
        return base;
    }
    if w == ADDITIVE_WEIGHT {
        let (p0, q0) = additive_base;
        let d = quat_mul(conjugate(q0), qt);
        return (add(p, [pt[0] - p0[0], pt[1] - p0[1], pt[2] - p0[2]]), quat_mul(q, d));
    }
    let f = w * m;
    (lerp3(p, pt, f), slerp(q, qt, f))
}

/// The bind local of node j, B_parent^-1 B_j (B_root for the root).
pub fn bind_local(model: &Model, j: usize) -> Mat4 {
    let n = &model.nodes[j];
    match n.parent {
        Some(p) => mul(&inverse_affine(&model.nodes[p].bind), &n.bind),
        None => n.bind,
    }
}

/// "Node evaluation order": global matrices from local poses (one per
/// node, root translation included), parents before children, without
/// the object matrix. Rotation-only nodes take their bind offset as
/// translation.
pub fn globals(model: &Model, pose: &[(Vec3, Quat)]) -> Vec<Mat4> {
    let mut out: Vec<Mat4> = Vec::with_capacity(model.nodes.len());
    for (j, node) in model.nodes.iter().enumerate() {
        let (mut p, q) = pose.get(j).copied().unwrap_or(([0.0; 3], IDENTITY_QUAT));
        if node.flags & NODE_ROTATION_ONLY != 0 && node.parent.is_some() {
            let l = bind_local(model, j);
            p = [l[0][3], l[1][3], l[2][3]];
        }
        let local = matrix(p, q);
        out.push(match node.parent {
            Some(parent) => mul(&out[parent], &local),
            None => local,
        });
    }
    out
}

/// "Skinning": S_j = G_j B_j^-1 per node.
pub fn skin_matrices(model: &Model, globals: &[Mat4]) -> Vec<Mat4> {
    model
        .nodes
        .iter()
        .zip(globals)
        .map(|(n, g)| mul(g, &inverse_affine(&n.bind)))
        .collect()
}

/// "String keys": a key string split into commands (at `;` and control
/// characters), each into arguments (at spaces; `"..."` and `(...)`
/// group, their delimiters dropped; `%` makes the next character
/// literal). Commands without arguments are dropped.
pub fn key_commands(s: &str) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    let mut args: Vec<String> = Vec::new();
    let mut arg = String::new();
    let mut has_arg = false;
    let mut chars = s.chars();
    let end_arg = |args: &mut Vec<String>, arg: &mut String, has: &mut bool| {
        if *has {
            args.push(std::mem::take(arg));
            *has = false;
        }
    };
    while let Some(ch) = chars.next() {
        match ch {
            ';' | '\0'..='\u{1f}' | '\u{7f}' => {
                end_arg(&mut args, &mut arg, &mut has_arg);
                if !args.is_empty() {
                    out.push(std::mem::take(&mut args));
                }
            }
            ' ' => end_arg(&mut args, &mut arg, &mut has_arg),
            '"' | '(' if !has_arg => {
                let close = if ch == '"' { '"' } else { ')' };
                has_arg = true;
                for c in chars.by_ref() {
                    if c == close {
                        break;
                    }
                    arg.push(c);
                }
                end_arg(&mut args, &mut arg, &mut has_arg);
            }
            '%' => {
                if let Some(c) = chars.next() {
                    arg.push(c);
                    has_arg = true;
                }
            }
            c => {
                arg.push(c);
                has_arg = true;
            }
        }
    }
    end_arg(&mut args, &mut arg, &mut has_arg);
    if !args.is_empty() {
        out.push(args);
    }
    out
}
