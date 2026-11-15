//! Node poses and everything that is pure math on them
//! (specs/halo_ce/animations.md, "Sampling a frame", "Building the pose",
//! "Blending", "Aiming and looking screens"): `NodePose` (a node's
//! rotation, translation and scale), `Pose` (one per node) with the three
//! animation types applied to it (base, replacement, overlay; scaled and
//! continuous overlays; the transition blend), the aim and look grid
//! (`ScreenBounds::locate`, `Pose::apply_screen`), the quaternion helpers
//! (stored convention: the Hamilton product with the overlay on the left)
//! and the transforms a pose gives (`Pose::world_transforms`, composing
//! with `model::Model::bind_pose`'s `Transform`s).
//!
//! Quaternions are `[i, j, k, w]` as stored, in the game's convention (a
//! stored quaternion is the usual rotation of its conjugate), never
//! converted. A scaled node's `Transform` has its three axes multiplied by
//! the scale, so `parent.then(local)` composes scales as the spec's
//! matrix product does.

use std::io;

use super::{Animation, AnimationType, ScreenBounds};
use crate::model::Transform;

/// The identity rotation.
pub const IDENTITY_ROTATION: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// The Hamilton product `a * b` of stored `[i, j, k, w]` quaternions.
pub fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let [ai, aj, ak, aw] = a;
    let [bi, bj, bk, bw] = b;
    [
        aw * bi + ai * bw + aj * bk - ak * bj,
        aw * bj + aj * bw + ak * bi - ai * bk,
        aw * bk + ak * bw + ai * bj - aj * bi,
        aw * bw - ai * bi - aj * bj - ak * bk,
    ]
}

/// `a` scaled to unit length; a zero quaternion gives the identity.
pub fn quat_normalize(a: [f32; 4]) -> [f32; 4] {
    let n = a[0] * a[0] + a[1] * a[1] + a[2] * a[2] + a[3] * a[3];
    if n == 0.0 {
        return IDENTITY_ROTATION;
    }
    let s = 1.0 / n.sqrt();
    [a[0] * s, a[1] * s, a[2] * s, a[3] * s]
}

/// The length of `a`.
pub fn quat_length(a: [f32; 4]) -> f32 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2] + a[3] * a[3]).sqrt()
}

fn quat_dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]
}

/// The shortest-path normalised linear blend the engine uses (never
/// spherical): `(1 - t) a + t' b` with `t' = -t` when `a . b < 0`.
pub fn quat_nlerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let tb = if quat_dot(a, b) < 0.0 { -t } else { t };
    let ta = 1.0 - t;
    quat_normalize([
        ta * a[0] + tb * b[0],
        ta * a[1] + tb * b[1],
        ta * a[2] + tb * b[2],
        ta * a[3] + tb * b[3],
    ])
}

/// `(1 - t) a + t b`.
pub fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        (1.0 - t) * a[0] + t * b[0],
        (1.0 - t) * a[1] + t * b[1],
        (1.0 - t) * a[2] + t * b[2],
    ]
}

/// One node's orientation: rotation, translation (in the parent's frame)
/// and scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodePose {
    /// `[i, j, k, w]` as stored.
    pub rotation: [f32; 4],
    pub translation: [f32; 3],
    pub scale: f32,
}

impl NodePose {
    pub const IDENTITY: NodePose = NodePose {
        rotation: IDENTITY_ROTATION,
        translation: [0.0; 3],
        scale: 1.0,
    };

    /// The node's matrix into its parent's frame: `p -> s R(q) p + t`,
    /// as a `Transform` whose axes are scaled by `s`.
    pub fn local(&self) -> Transform {
        let mut t = Transform::from_quaternion(self.rotation, self.translation);
        let s = self.scale;
        for axis in [&mut t.forward, &mut t.left, &mut t.up] {
            for c in axis.iter_mut() {
                *c *= s;
            }
        }
        t
    }
}

/// One `NodePose` per node, in node order.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Pose {
    pub nodes: Vec<NodePose>,
}

impl Pose {
    /// `n` identity nodes.
    pub fn identity(n: usize) -> Pose {
        Pose {
            nodes: vec![NodePose::IDENTITY; n],
        }
    }

    /// A model's bind pose as node orientations (each node's stored
    /// rotation and translation, scale 1): what an object takes when its
    /// animation does not fit its model.
    pub fn from_model(model: &crate::model::Model) -> Pose {
        Pose {
            nodes: model
                .nodes
                .iter()
                .map(|n| NodePose {
                    rotation: n.rotation,
                    translation: n.translation,
                    scale: 1.0,
                })
                .collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Every node's parent-relative matrix.
    pub fn local_transforms(&self) -> Vec<Transform> {
        self.nodes.iter().map(NodePose::local).collect()
    }

    /// Every node's matrix in the object's frame: `W(0) = L(0)` and
    /// `W(n) = W(parent) x L(n)`. `parents[n]` is node n's parent (`None`
    /// for a root), which must come before it; `AnimationGraph::parents`
    /// and `model.nodes[..].parent` give them. Scales compose as the spec
    /// says (rotation `Ra Rb`, position `pa + sa Ra pb`, scale `sa sb`).
    pub fn world_transforms(&self, parents: &[Option<usize>]) -> io::Result<Vec<Transform>> {
        if parents.len() < self.nodes.len() {
            return Err(err(format!(
                "{} nodes in the pose but {} parents",
                self.nodes.len(),
                parents.len()
            )));
        }
        let mut world: Vec<Transform> = Vec::with_capacity(self.nodes.len());
        for (n, node) in self.nodes.iter().enumerate() {
            let local = node.local();
            world.push(match parents[n] {
                None => local,
                Some(p) if p < n => world[p].then(&local),
                Some(p) => return Err(err(format!("node {n} has parent {p}, which does not come before it"))),
            });
        }
        Ok(world)
    }

    /// Apply `animation` at integer `frame` by its own type: a base
    /// animation replaces the whole pose, a replacement sets the parts
    /// whose bits it has, an overlay adds. `Ok(false)`: not applied (an
    /// overlay or replacement with more nodes than the pose).
    pub fn apply(&mut self, animation: &Animation, frame: usize) -> io::Result<bool> {
        match animation.kind {
            AnimationType::Base => {
                *self = animation.sample(frame)?;
                Ok(true)
            }
            AnimationType::Replacement => self.apply_replacement(animation, frame),
            AnimationType::Overlay => self.apply_overlay(animation, frame),
        }
    }

    fn fits(&self, animation: &Animation) -> bool {
        animation.node_count() <= self.nodes.len()
    }

    /// A replacement animation: for each node, the parts (rotation,
    /// translation, scale) whose bit the animation sets take the frame's
    /// value; the rest is left alone.
    pub fn apply_replacement(&mut self, animation: &Animation, frame: usize) -> io::Result<bool> {
        if !self.fits(animation) {
            return Ok(false);
        }
        let s = animation.sample(frame)?;
        for n in 0..animation.node_count() {
            if animation.has_rotation(n) {
                self.nodes[n].rotation = s.nodes[n].rotation;
            }
            if animation.has_translation(n) {
                self.nodes[n].translation = s.nodes[n].translation;
            }
            if animation.has_scale(n) {
                self.nodes[n].scale = s.nodes[n].scale;
            }
        }
        Ok(true)
    }

    /// An overlay: for each node and each part whose bit is set, `q =
    /// q_o * q` (the stored product, overlay on the left), `t = t + t_o`,
    /// `s = s x s_o`.
    pub fn apply_overlay(&mut self, animation: &Animation, frame: usize) -> io::Result<bool> {
        if !self.fits(animation) {
            return Ok(false);
        }
        let s = animation.sample(frame)?;
        self.overlay_sample(animation, &s, None, false);
        Ok(true)
    }

    /// A scaled overlay of weight `w` in `0..=1`: `q_o' = (1 - w) e + w'
    /// q_o` (`e` the identity, `w'` signed as in nlerp), **not**
    /// normalised, `q = q_o' * q`, `t = t + w t_o`, `s = s (w s_o + 1 -
    /// w)`.
    pub fn apply_scaled_overlay(&mut self, animation: &Animation, frame: usize, weight: f32) -> io::Result<bool> {
        if !self.fits(animation) {
            return Ok(false);
        }
        let s = animation.sample(frame)?;
        self.overlay_sample(animation, &s, Some(weight), false);
        Ok(true)
    }

    /// A continuous overlay at fractional frame `x` (`Animation::
    /// sample_continuous`), applied as a plain overlay.
    pub fn apply_continuous_overlay(&mut self, animation: &Animation, x: f32) -> io::Result<bool> {
        if !self.fits(animation) {
            return Ok(false);
        }
        let s = animation.sample_continuous(x)?;
        self.overlay_sample(animation, &s, None, false);
        Ok(true)
    }

    /// A continuous scaled overlay: the continuous sample, then the scaled
    /// rule with the weighted rotation **normalised**.
    pub fn apply_continuous_scaled_overlay(&mut self, animation: &Animation, x: f32, weight: f32) -> io::Result<bool> {
        if !self.fits(animation) {
            return Ok(false);
        }
        let s = animation.sample_continuous(x)?;
        self.overlay_sample(animation, &s, Some(weight), true);
        Ok(true)
    }

    fn overlay_sample(&mut self, animation: &Animation, sample: &Pose, weight: Option<f32>, normalize: bool) {
        for n in 0..animation.node_count() {
            let o = &sample.nodes[n];
            let node = &mut self.nodes[n];
            if animation.has_rotation(n) {
                node.rotation = quat_mul(scaled_rotation(o.rotation, weight, normalize), node.rotation);
            }
            if animation.has_translation(n) {
                let w = weight.unwrap_or(1.0);
                for k in 0..3 {
                    node.translation[k] += w * o.translation[k];
                }
            }
            if animation.has_scale(n) {
                node.scale *= match weight {
                    Some(w) => w * o.scale + 1.0 - w,
                    None => o.scale,
                };
            }
        }
    }

    /// The transition blend toward this (the new, final) pose from the
    /// pose `from` the state change started at: each node becomes
    /// `nlerp(q_from, q, u)`, `lerp(t_from, t, u)`, `lerp(s_from, s, u)`.
    /// `transition_weight(i, n)` gives `u`.
    pub fn blend_from(&mut self, from: &Pose, u: f32) {
        for (node, f) in self.nodes.iter_mut().zip(&from.nodes) {
            node.rotation = quat_nlerp(f.rotation, node.rotation, u);
            node.translation = lerp3(f.translation, node.translation, u);
            node.scale = (1.0 - u) * f.scale + u * node.scale;
        }
    }

    /// An aiming or looking screen (`animation` an overlay whose frames
    /// form the grid of `bounds`) applied for relative `yaw` and `pitch`
    /// in radians: the four surrounding frames blended bilinearly (`q_o =
    /// nlerp(nlerp(q00, q10, u), nlerp(q01, q11, u), p)`, translations by
    /// the four weights, scale ignored), then `q = q_o * q`. `Ok(false)`:
    /// nothing applied (not an overlay, fewer frames than the grid, more
    /// nodes than the pose, or the cell outside the grid).
    pub fn apply_screen(
        &mut self,
        animation: &Animation,
        bounds: &ScreenBounds,
        yaw: f32,
        pitch: f32,
    ) -> io::Result<bool> {
        if animation.kind != AnimationType::Overlay || !self.fits(animation) {
            return Ok(false);
        }
        let (w, h) = (bounds.columns(), bounds.rows());
        if (animation.frame_count.max(0) as usize) < w * h {
            return Ok(false);
        }
        let Some(cell) = bounds.locate(yaw, pitch) else {
            return Ok(false);
        };
        let s: Vec<Pose> = cell
            .frames
            .iter()
            .map(|&f| animation.sample(f))
            .collect::<io::Result<_>>()?;
        let weights = cell.weights();
        for n in 0..animation.node_count() {
            let node = &mut self.nodes[n];
            if animation.has_rotation(n) {
                let y0 = quat_nlerp(s[0].nodes[n].rotation, s[1].nodes[n].rotation, cell.u);
                let y1 = quat_nlerp(s[2].nodes[n].rotation, s[3].nodes[n].rotation, cell.u);
                node.rotation = quat_mul(quat_nlerp(y0, y1, cell.p), node.rotation);
            }
            if animation.has_translation(n) {
                for k in 0..3 {
                    for (m, wt) in weights.iter().enumerate() {
                        node.translation[k] += wt * s[m].nodes[n].translation[k];
                    }
                }
            }
        }
        Ok(true)
    }
}

/// The overlay rotation after the weight rule: unchanged without a weight;
/// else `(1 - w) e + w' q_o`, normalised only when asked.
fn scaled_rotation(q: [f32; 4], weight: Option<f32>, normalize: bool) -> [f32; 4] {
    let Some(w) = weight else {
        return q;
    };
    let wb = if q[3] < 0.0 { -w } else { w };
    let r = [wb * q[0], wb * q[1], wb * q[2], (1.0 - w) + wb * q[3]];
    if normalize { quat_normalize(r) } else { r }
}

/// The blend weight on tick `i` of an `n`-tick transition: `(i + 1) / n`.
pub fn transition_weight(i: u32, n: u32) -> f32 {
    if n == 0 {
        1.0
    } else {
        ((i + 1) as f32 / n as f32).min(1.0)
    }
}

/// Where a yaw and pitch fall on an aiming or looking grid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AimCell {
    pub column: usize,
    pub row: usize,
    /// The fraction along the columns.
    pub u: f32,
    /// The fraction along the rows.
    pub p: f32,
    /// The frames `f00`, `f10`, `f01`, `f11`: (column, row), (next column,
    /// row), (column, next row), (next column, next row).
    pub frames: [usize; 4],
}

impl AimCell {
    /// The bilinear weights of `frames`: `(1-u)(1-p)`, `u(1-p)`, `(1-u)p`,
    /// `u p`.
    pub fn weights(&self) -> [f32; 4] {
        let (u, p) = (self.u, self.p);
        [(1.0 - u) * (1.0 - p), u * (1.0 - p), (1.0 - u) * p, u * p]
    }
}

impl ScreenBounds {
    /// Columns of the grid: right count + left count + 1 (at least 0).
    pub fn columns(&self) -> usize {
        (i32::from(self.right_frame_count) + i32::from(self.left_frame_count) + 1).max(0) as usize
    }

    /// Rows of the grid: down count + up count + 1 (at least 0).
    pub fn rows(&self) -> usize {
        (i32::from(self.down_frame_count) + i32::from(self.up_frame_count) + 1).max(0) as usize
    }

    /// The grid cell for `yaw` and `pitch` (radians, relative; the spec's
    /// steps 1 to 5): `None` when the cell falls outside the grid (only
    /// possible with zero counts).
    pub fn locate(&self, yaw: f32, pitch: f32) -> Option<AimCell> {
        let (column, u) = axis_cell(
            yaw,
            self.left_yaw_per_frame,
            self.right_yaw_per_frame,
            i32::from(self.left_frame_count),
            i32::from(self.right_frame_count),
        );
        let (row, p) = axis_cell(
            pitch,
            self.up_pitch_per_frame,
            self.down_pitch_per_frame,
            i32::from(self.up_frame_count),
            i32::from(self.down_frame_count),
        );
        let (w, h) = (self.columns() as i32, self.rows() as i32);
        if column < 0 || column >= w || row < 0 || row >= h {
            return None;
        }
        let c1 = if column + 1 < w { column + 1 } else { column };
        let r1 = if row + 1 < h { row + 1 } else { row };
        let at = |c: i32, r: i32| (c + r * w) as usize;
        Some(AimCell {
            column: column as usize,
            row: row as usize,
            u,
            p,
            frames: [at(column, row), at(c1, row), at(column, r1), at(c1, r1)],
        })
    }
}

/// One axis of `locate`: the index (counted from the far negative end) and
/// the fraction. `positive_step` is used for a non-negative angle.
fn axis_cell(
    angle: f32,
    positive_step: f32,
    negative_step: f32,
    positive_count: i32,
    negative_count: i32,
) -> (i32, f32) {
    let d = if angle >= 0.0 { positive_step } else { negative_step };
    let v = if d == 0.0 { 0.0 } else { angle / d };
    let mut c = v.trunc();
    let mut u = v - c;
    if u < 0.0 {
        u += 1.0;
        c -= 1.0;
    }
    let mut c = c as i32;
    if c >= positive_count {
        c = positive_count - 1;
        u = 1.0;
    }
    if c < -negative_count {
        c = -negative_count;
        u = 0.0;
    }
    (c + negative_count, u)
}

fn err(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}
