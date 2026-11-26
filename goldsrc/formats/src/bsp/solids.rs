//! A hull's solid space as convex regions (specs/goldsrc/bsp.md,
//! "Reconstructing shapes"; docs/plans/active/goldsrc.md §3): every path
//! from a hull's root to a solid or sky end is the intersection of the
//! half-spaces it took, cut here out of the model's bounds as a convex
//! polytope (`Convex`: its face planes, vertices and edge directions).
//! `shrink` moves a clip hull region's faces back by the hull's box
//! (`Shrink` picks the rule), giving the solid a box of that size could
//! not enter. `Convex::box_overlap` and `Convex::box_sweep` test an
//! axis-aligned box against a region by separating axes (the faces, the
//! box's axes and their crossings with the region's edges): exact for
//! convex shapes, what the M0 collision spike compares with hull traces.
//!
//! Known shortcuts:
//! - Regions are cut from the model's bounds grown by 64 units: solid
//!   space beyond them (the void around the world) ends at that box.

use super::Bsp;
use super::hull::{HullRef, blocks};

/// Points closer than this to a plane count as on it, in units.
const EPS: f64 = 1e-4;

type V3 = [f64; 3];

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn len(a: V3) -> f64 {
    dot(a, a).sqrt()
}
fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

/// A half-space: points x with `normal · x <= dist` are inside.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HalfSpace {
    pub normal: V3,
    pub dist: f64,
}

/// One solid region: the half-spaces of its path and its contents.
#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    pub contents: i32,
    pub half_spaces: Vec<HalfSpace>,
}

/// Every path from `hull`'s root of `model` to a solid or sky end.
pub fn solid_regions(bsp: &Bsp, model: usize, hull: usize) -> Vec<Region> {
    regions_where(bsp, model, hull, blocks)
}

/// Every path from `hull`'s root of `model` to an end whose contents
/// `keep` takes (liquids: `liquid_regions`).
pub fn regions_where(bsp: &Bsp, model: usize, hull: usize, keep: fn(i32) -> bool) -> Vec<Region> {
    let mut out = Vec::new();
    let mut path = Vec::new();
    walk(bsp, hull, bsp.hull_root(model, hull), keep, &mut path, &mut out);
    out
}

fn walk(bsp: &Bsp, hull: usize, at: HullRef, keep: fn(i32) -> bool, path: &mut Vec<HalfSpace>, out: &mut Vec<Region>) {
    match at {
        HullRef::Contents(c) => {
            if keep(c) {
                out.push(Region {
                    contents: c,
                    half_spaces: path.clone(),
                });
            }
        }
        HullRef::Node(n) => {
            let (plane, kids) = bsp.hull_node(hull, n);
            let nrm = [plane.normal[0] as f64, plane.normal[1] as f64, plane.normal[2] as f64];
            let d = plane.dist as f64;
            // Front: n · x >= d, i.e. −n · x <= −d.
            path.push(HalfSpace {
                normal: scale(nrm, -1.0),
                dist: -d,
            });
            walk(bsp, hull, kids[0], keep, path, out);
            path.pop();
            path.push(HalfSpace { normal: nrm, dist: d });
            walk(bsp, hull, kids[1], keep, path, out);
            path.pop();
        }
    }
}

/// A convex polytope: its faces' planes (only those that bound it), its
/// corners and its edges' directions (unit, one per direction).
#[derive(Clone, Debug, PartialEq)]
pub struct Convex {
    pub planes: Vec<HalfSpace>,
    /// Each plane's face polygon.
    pub faces: Vec<Vec<V3>>,
    pub vertices: Vec<V3>,
    pub edge_dirs: Vec<V3>,
    pub mins: V3,
    pub maxs: V3,
}

impl Convex {
    /// The half-spaces' intersection inside the box `mins`..`maxs`, or
    /// `None` when it is empty or flat.
    pub fn cut(half_spaces: &[HalfSpace], mins: V3, maxs: V3) -> Option<Convex> {
        // Start from the box as six faces, each a polygon on its plane.
        let mut faces: Vec<(HalfSpace, Vec<V3>)> = Vec::with_capacity(16);
        for axis in 0..3 {
            for (sign, d) in [(1.0, maxs[axis]), (-1.0, -mins[axis])] {
                let mut n = [0.0; 3];
                n[axis] = sign;
                faces.push((HalfSpace { normal: n, dist: d }, box_face(mins, maxs, axis, sign)));
            }
        }
        for hs in half_spaces {
            let l = len(hs.normal);
            if l < 1e-9 {
                continue;
            }
            let hs = HalfSpace {
                normal: scale(hs.normal, 1.0 / l),
                dist: hs.dist / l,
            };
            clip(&mut faces, hs)?;
        }
        let mut vertices: Vec<V3> = Vec::new();
        let mut edge_dirs: Vec<V3> = Vec::new();
        for (_, poly) in &faces {
            for (i, &p) in poly.iter().enumerate() {
                if !vertices.iter().any(|v| len(sub(*v, p)) < 1e-3) {
                    vertices.push(p);
                }
                let e = sub(poly[(i + 1) % poly.len()], p);
                let l = len(e);
                if l > 1e-3 {
                    let e = scale(e, 1.0 / l);
                    if !edge_dirs.iter().any(|d| len(cross(*d, e)) < 1e-6) {
                        edge_dirs.push(e);
                    }
                }
            }
        }
        if vertices.len() < 4 {
            return None;
        }
        let mut lo = [f64::MAX; 3];
        let mut hi = [f64::MIN; 3];
        for v in &vertices {
            for a in 0..3 {
                lo[a] = lo[a].min(v[a]);
                hi[a] = hi[a].max(v[a]);
            }
        }
        if (0..3).any(|a| hi[a] - lo[a] < 1e-3) {
            return None;
        }
        let (planes, faces) = faces.into_iter().unzip();
        Some(Convex {
            planes,
            faces,
            vertices,
            edge_dirs,
            mins: lo,
            maxs: hi,
        })
    }

    fn project(&self, axis: V3) -> (f64, f64) {
        self.vertices.iter().fold((f64::MAX, f64::MIN), |(lo, hi), v| {
            let d = dot(*v, axis);
            (lo.min(d), hi.max(d))
        })
    }

    /// The separating axes against an axis-aligned box: the faces' normals,
    /// the box's axes and their crossings with the edges.
    fn axes(&self) -> Vec<V3> {
        let mut axes: Vec<V3> = self.planes.iter().map(|p| p.normal).collect();
        axes.extend([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
        for e in &self.edge_dirs {
            for b in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] {
                let c = cross(b, *e);
                let l = len(c);
                if l > 1e-6 {
                    axes.push(scale(c, 1.0 / l));
                }
            }
        }
        axes
    }

    /// How deep the box centred at `centre` with half sizes `half` is in
    /// this region: the least overlap over every separating axis, ≤ 0 when
    /// they don't touch.
    pub fn box_overlap(&self, centre: V3, half: V3) -> f64 {
        let mut depth = f64::MAX;
        for a in self.axes() {
            let c = dot(centre, a);
            let r = a[0].abs() * half[0] + a[1].abs() * half[1] + a[2].abs() * half[2];
            let (lo, hi) = self.project(a);
            let o = (c + r - lo).min(hi - (c - r));
            if o <= 0.0 {
                return o;
            }
            depth = depth.min(o);
        }
        depth
    }

    /// The first time t in 0..=1 at which the box centred at `start`
    /// moving by `delta` touches this region (`Some(0)` when it starts in
    /// it), `None` if it never does.
    pub fn box_sweep(&self, start: V3, delta: V3, half: V3) -> Option<f64> {
        let (mut enter, mut exit) = (0.0f64, 1.0f64);
        for a in self.axes() {
            let c = dot(start, a);
            let s = dot(delta, a);
            let r = a[0].abs() * half[0] + a[1].abs() * half[1] + a[2].abs() * half[2];
            let (lo, hi) = self.project(a);
            // Overlap while c + t s + r > lo and c + t s − r < hi.
            if s.abs() < 1e-12 {
                if c + r <= lo || c - r >= hi {
                    return None;
                }
                continue;
            }
            let t0 = (lo - r - c) / s;
            let t1 = (hi + r - c) / s;
            let (t0, t1) = if t0 < t1 { (t0, t1) } else { (t1, t0) };
            enter = enter.max(t0);
            exit = exit.min(t1);
            if enter > exit {
                return None;
            }
        }
        Some(enter)
    }
}

fn box_face(mins: V3, maxs: V3, axis: usize, sign: f64) -> Vec<V3> {
    let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
    let fixed = if sign > 0.0 { maxs[axis] } else { mins[axis] };
    let corner = |a: f64, b: f64| {
        let mut p = [0.0; 3];
        p[axis] = fixed;
        p[u] = a;
        p[v] = b;
        p
    };
    vec![
        corner(mins[u], mins[v]),
        corner(maxs[u], mins[v]),
        corner(maxs[u], maxs[v]),
        corner(mins[u], maxs[v]),
    ]
}

/// Cut the polytope (its faces) by `hs`; `None` when nothing is left.
fn clip(faces: &mut Vec<(HalfSpace, Vec<V3>)>, hs: HalfSpace) -> Option<()> {
    let mut cut_points: Vec<V3> = Vec::new();
    let mut any_out = false;
    for (_, poly) in faces.iter_mut() {
        let mut out = Vec::with_capacity(poly.len() + 2);
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            let (da, db) = (dot(hs.normal, a) - hs.dist, dot(hs.normal, b) - hs.dist);
            if da <= EPS {
                out.push(a);
                if da >= -EPS {
                    cut_points.push(a);
                }
            } else {
                any_out = true;
            }
            if (da < -EPS && db > EPS) || (da > EPS && db < -EPS) {
                let p = sub(a, scale(sub(a, b), da / (da - db)));
                out.push(p);
                cut_points.push(p);
            }
        }
        *poly = out;
    }
    faces.retain(|(_, p)| p.len() >= 3 && polygon_area(p) > 1e-6);
    if faces.is_empty() {
        return None;
    }
    if any_out {
        let mut pts: Vec<V3> = Vec::new();
        for p in cut_points {
            if !pts.iter().any(|q| len(sub(*q, p)) < 1e-4) {
                pts.push(p);
            }
        }
        if pts.len() >= 3 {
            let c = scale(
                pts.iter()
                    .fold([0.0; 3], |s, p| [s[0] + p[0], s[1] + p[1], s[2] + p[2]]),
                1.0 / pts.len() as f64,
            );
            let n = hs.normal;
            let u = {
                let t = if n[0].abs() < 0.9 {
                    [1.0, 0.0, 0.0]
                } else {
                    [0.0, 1.0, 0.0]
                };
                let u = cross(n, t);
                scale(u, 1.0 / len(u))
            };
            let v = cross(n, u);
            pts.sort_by(|a, b| {
                let (da, db) = (sub(*a, c), sub(*b, c));
                dot(da, v).atan2(dot(da, u)).total_cmp(&dot(db, v).atan2(dot(db, u)))
            });
            if polygon_area(&pts) > 1e-6 {
                faces.push((hs, pts));
            }
        }
    }
    Some(())
}

fn polygon_area(p: &[V3]) -> f64 {
    let mut s = [0.0; 3];
    for i in 1..p.len().saturating_sub(1) {
        let c = cross(sub(p[i], p[0]), sub(p[i + 1], p[0]));
        s = [s[0] + c[0], s[1] + c[1], s[2] + c[2]];
    }
    len(s) / 2.0
}

/// How a clip hull region's faces are moved back by the hull's box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shrink {
    /// The compiler's growth as measured (bsp.md, "How faces are grown"):
    /// axial faces by the box's half size on that axis; others by the
    /// larger of the horizontal half size × the normal's horizontal length
    /// and the vertical half size × |normal z|.
    Measured,
    /// Every face by the box's full support distance along its normal
    /// (an exact Minkowski difference; shrinks slanted faces too far).
    Support,
}

/// A clip hull region's solid as the box of `half` sizes would meet it:
/// its faces that `exposed` says border open space moved back (`rule`;
/// faces between two solid regions of the tree stay), then cut again.
/// `None` when nothing is left (a region thinner than the box).
pub fn shrink(region: &Convex, half: V3, rule: Shrink, exposed: impl Fn(&HalfSpace, &[V3]) -> bool) -> Option<Convex> {
    let moved: Vec<HalfSpace> = region
        .planes
        .iter()
        .zip(&region.faces)
        .map(|(p, face)| {
            if !exposed(p, face) {
                return *p;
            }
            let n = p.normal;
            let support = n[0].abs() * half[0] + n[1].abs() * half[1] + n[2].abs() * half[2];
            let axial = n.iter().filter(|c| c.abs() > 1e-6).count() == 1;
            let by = match rule {
                Shrink::Support => support,
                Shrink::Measured if axial => support,
                Shrink::Measured => {
                    let horizontal = (n[0] * n[0] + n[1] * n[1]).sqrt();
                    (half[0] * horizontal).max(half[2] * n[2].abs())
                }
            };
            HalfSpace {
                normal: n,
                dist: p.dist - by,
            }
        })
        .collect();
    Convex::cut(&moved, region.mins, region.maxs)
}

/// Whether a face of a region of `hull` of `model` borders open space
/// anywhere: points on it every 8 units at most (at most 32 a side of
/// each fan triangle), moved half a unit out, probed in that hull. A face
/// only partly open is moved back whole: the slab lost next to its solid
/// part is thinner than the box, which the solid beyond it still stops.
pub fn exposed_in(bsp: &Bsp, model: usize, hull: usize) -> impl Fn(&HalfSpace, &[V3]) -> bool + '_ {
    move |plane, face| {
        if face.len() < 3 {
            return true;
        }
        let out = scale(plane.normal, 0.5);
        let open = |p: V3| !blocks(bsp.contents(model, hull, [p[0] + out[0], p[1] + out[1], p[2] + out[2]]));
        for k in 1..face.len() - 1 {
            let (a, b, c) = (face[0], face[k], face[k + 1]);
            let longest = len(sub(b, a)).max(len(sub(c, a))).max(len(sub(c, b)));
            let n = ((longest / 8.0).ceil() as usize).clamp(1, 32);
            for i in 0..=n {
                for j in 0..=n - i {
                    let (u, v) = (i as f64 / n as f64, j as f64 / n as f64);
                    let w = 1.0 - u - v;
                    // Pulled a little toward the centre so edges are not probed.
                    let (u, v, w) = (u * 0.98 + 0.02 / 3.0, v * 0.98 + 0.02 / 3.0, w * 0.98 + 0.02 / 3.0);
                    let p = [0, 1, 2].map(|x| a[x] * w + b[x] * u + c[x] * v);
                    if open(p) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

/// Every solid region of `hull` of `model` as a convex polytope, cut out of
/// the model's bounds grown by 64 units.
pub fn convex_regions(bsp: &Bsp, model: usize, hull: usize) -> Vec<(i32, Convex)> {
    convex_regions_where(bsp, model, hull, blocks)
}

/// Every region of `hull` of `model` whose contents `keep` takes, as a
/// convex polytope cut out of the model's bounds grown by 64 units.
pub fn convex_regions_where(bsp: &Bsp, model: usize, hull: usize, keep: fn(i32) -> bool) -> Vec<(i32, Convex)> {
    let m = &bsp.models[model];
    let mins = [
        m.mins[0] as f64 - 64.0,
        m.mins[1] as f64 - 64.0,
        m.mins[2] as f64 - 64.0,
    ];
    let maxs = [
        m.maxs[0] as f64 + 64.0,
        m.maxs[1] as f64 + 64.0,
        m.maxs[2] as f64 + 64.0,
    ];
    regions_where(bsp, model, hull, keep)
        .into_iter()
        .filter_map(|r| Convex::cut(&r.half_spaces, mins, maxs).map(|c| (r.contents, c)))
        .collect()
}
