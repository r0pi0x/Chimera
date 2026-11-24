// Adapted from qbsp (MIT, see THIRD_PARTY.md): src/query.rs (leaf_at_point_in_node)
// Copyright (c) 2026 Noxmore. Only the leaf lookup's walk comes from qbsp;
// the clip hulls, contents and the hull trace are ours, from
// specs/goldsrc/bsp.md ("Hulls and collision").
//! The four hulls of a brush model (specs/goldsrc/bsp.md, "Hulls and
//! collision"): hull 0 is the render tree (nodes ending in leaves), hulls
//! 1-3 are clip trees in which the world is already grown by a box
//! (`HULL_BOXES`), ending in contents values. `Bsp::leaf_at` finds the
//! render leaf holding a point; `Bsp::contents` a point's contents in any
//! hull; `Bsp::trace` moves a point through one hull and reports the first
//! contact (fraction, end, plane, started/all solid, open/water), with the
//! end kept 1/32 unit off the plane hit.

use super::{Bsp, Child, Plane};

pub const CONTENTS_EMPTY: i32 = -1;
pub const CONTENTS_SOLID: i32 = -2;
pub const CONTENTS_WATER: i32 = -3;
pub const CONTENTS_SLIME: i32 = -4;
pub const CONTENTS_LAVA: i32 = -5;
pub const CONTENTS_SKY: i32 = -6;

/// How far a trace's end is kept off the plane it hit, in units.
pub const TRACE_BACKOFF: f64 = 1.0 / 32.0;

/// Each hull's box (minimum, maximum) around the traced point.
pub const HULL_BOXES: [([f32; 3], [f32; 3]); 4] = [
    ([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
    ([-16.0, -16.0, -36.0], [16.0, 16.0, 36.0]),
    ([-32.0, -32.0, -32.0], [32.0, 32.0, 32.0]),
    ([-16.0, -16.0, -18.0], [16.0, 16.0, 18.0]),
];

/// Whether a contents value blocks movement (solid or sky).
pub fn blocks(contents: i32) -> bool {
    contents == CONTENTS_SOLID || contents == CONTENTS_SKY
}

/// A step in one hull's tree: an inner node (by index) or an end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HullRef {
    Node(i32),
    /// Contents value (hulls 1-3) or a reached leaf's contents (hull 0).
    Contents(i32),
}

/// The result of a hull trace.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trace {
    /// 0 to 1: how far along start → end the point got (1: no contact).
    pub fraction: f64,
    pub end: [f64; 3],
    /// The plane hit, facing the open side (zero when nothing was hit).
    pub plane_normal: [f64; 3],
    pub plane_dist: f64,
    pub started_solid: bool,
    pub all_solid: bool,
    pub in_open: bool,
    pub in_water: bool,
}

impl Bsp {
    /// The render leaf holding `p`, from model `model`'s hull 0 root.
    pub fn leaf_at(&self, model: usize, p: [f64; 3]) -> usize {
        let mut node = Child::Node(self.models[model].heads[0] as u32);
        loop {
            match node {
                Child::Leaf(l) => return l as usize,
                Child::Node(n) => {
                    let n = &self.nodes[n as usize];
                    node = if self.planes[n.plane as usize].side(p) >= 0.0 {
                        n.children[0]
                    } else {
                        n.children[1]
                    };
                }
            }
        }
    }

    /// The root of `hull` (0-3) of `model`.
    pub fn hull_root(&self, model: usize, hull: usize) -> HullRef {
        HullRef::Node(self.models[model].heads[hull])
    }

    /// A node's plane and its front and back children, in `hull`.
    pub fn hull_node(&self, hull: usize, node: i32) -> (&Plane, [HullRef; 2]) {
        if hull == 0 {
            let n = &self.nodes[node as usize];
            let child = |c: Child| match c {
                Child::Node(i) => HullRef::Node(i as i32),
                Child::Leaf(l) => HullRef::Contents(self.leaves.get(l as usize).map_or(CONTENTS_SOLID, |l| l.contents)),
            };
            (
                &self.planes[n.plane as usize],
                [child(n.children[0]), child(n.children[1])],
            )
        } else {
            let n = &self.clip_nodes[node as usize];
            let child = |c: i16| {
                if c >= 0 {
                    HullRef::Node(c as i32)
                } else {
                    HullRef::Contents(c as i32)
                }
            };
            (
                &self.planes[n.plane as usize],
                [child(n.children[0]), child(n.children[1])],
            )
        }
    }

    /// The contents of `p` in `hull` of `model`.
    pub fn contents(&self, model: usize, hull: usize, p: [f64; 3]) -> i32 {
        self.contents_from(hull, self.hull_root(model, hull), p)
    }

    fn contents_from(&self, hull: usize, mut at: HullRef, p: [f64; 3]) -> i32 {
        loop {
            match at {
                HullRef::Contents(c) => return c,
                HullRef::Node(n) => {
                    let (plane, kids) = self.hull_node(hull, n);
                    at = if plane.side(p) >= 0.0 { kids[0] } else { kids[1] };
                }
            }
        }
    }

    /// Move a point from `start` to `end` through `hull` of `model` and
    /// report the first contact (specs/goldsrc/bsp.md, "Hull traces").
    pub fn trace(&self, model: usize, hull: usize, start: [f64; 3], end: [f64; 3]) -> Trace {
        let mut t = Trace {
            fraction: 1.0,
            end,
            plane_normal: [0.0; 3],
            plane_dist: 0.0,
            started_solid: false,
            all_solid: true,
            in_open: false,
            in_water: false,
        };
        let root = self.hull_root(model, hull);
        let mut w = Walk {
            bsp: self,
            hull,
            root,
            t: &mut t,
        };
        w.walk(root, 0.0, 1.0, start, end);
        if t.all_solid {
            t.started_solid = true;
        }
        if t.fraction == 1.0 {
            t.end = end;
        }
        t
    }
}

struct Walk<'a> {
    bsp: &'a Bsp,
    hull: usize,
    root: HullRef,
    t: &'a mut Trace,
}

fn lerp(a: [f64; 3], b: [f64; 3], f: f64) -> [f64; 3] {
    [
        a[0] + (b[0] - a[0]) * f,
        a[1] + (b[1] - a[1]) * f,
        a[2] + (b[2] - a[2]) * f,
    ]
}

impl Walk<'_> {
    /// Walk the segment p1 → p2 (fractions f1 → f2 of the whole) down from
    /// `at`; false once a contact has been recorded.
    fn walk(&mut self, at: HullRef, f1: f64, f2: f64, p1: [f64; 3], p2: [f64; 3]) -> bool {
        let n = match at {
            HullRef::Contents(c) => {
                if blocks(c) {
                    self.t.started_solid = true;
                } else {
                    self.t.all_solid = false;
                    if c == CONTENTS_EMPTY {
                        self.t.in_open = true;
                    } else {
                        self.t.in_water = true;
                    }
                }
                return true;
            }
            HullRef::Node(n) => n,
        };
        let (plane, kids) = self.bsp.hull_node(self.hull, n);
        let (d1, d2) = (plane.side(p1), plane.side(p2));
        if d1 >= 0.0 && d2 >= 0.0 {
            return self.walk(kids[0], f1, f2, p1, p2);
        }
        if d1 < 0.0 && d2 < 0.0 {
            return self.walk(kids[1], f1, f2, p1, p2);
        }
        // Straddling: split 1/32 short of the plane on the near side.
        let near_back = d1 < 0.0;
        let mut frac = if near_back {
            (d1 + TRACE_BACKOFF) / (d1 - d2)
        } else {
            (d1 - TRACE_BACKOFF) / (d1 - d2)
        }
        .clamp(0.0, 1.0);
        let mut fm = f1 + (f2 - f1) * frac;
        let mut mid = lerp(p1, p2, frac);
        let (near, far) = if near_back {
            (kids[1], kids[0])
        } else {
            (kids[0], kids[1])
        };
        if !self.walk(near, f1, fm, p1, mid) {
            return false;
        }
        if !blocks(self.bsp.contents_from(self.hull, far, mid)) {
            return self.walk(far, fm, f2, mid, p2);
        }
        if self.t.all_solid {
            return false;
        }
        // The far side is solid: the contact, on the plane facing p1.
        let n3 = [plane.normal[0] as f64, plane.normal[1] as f64, plane.normal[2] as f64];
        let s = if near_back { -1.0 } else { 1.0 };
        self.t.plane_normal = [n3[0] * s, n3[1] * s, n3[2] * s];
        self.t.plane_dist = plane.dist as f64 * s;
        // Pull back while the end is still solid near other planes.
        while blocks(self.bsp.contents_from(self.hull, self.root, mid)) {
            frac -= 0.1;
            if frac < 0.0 {
                self.t.fraction = fm;
                self.t.end = mid;
                return false;
            }
            fm = f1 + (f2 - f1) * frac;
            mid = lerp(p1, p2, frac);
        }
        self.t.fraction = fm;
        self.t.end = mid;
        false
    }
}
