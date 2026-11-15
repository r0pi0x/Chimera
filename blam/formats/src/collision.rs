//! The collision BSP (specs/halo_ce/collision.md): the structure shared by
//! levels and model collision tags. `CollisionBsp` holds the eight blocks as
//! plain structs, validated once at read time (every index in range), and
//! offers surface polygon recovery (the edge-ring walk), fan
//! triangulation, the point test and the line trace as the spec describes
//! them. `read_level` reads a level's (the structure BSP's block at 0xB0
//! and its collision materials), `read_model` a `coll` tag's regions,
//! materials and nodes with their per-permutation BSPs.
//!
//! Values are the game's own: world units, right-handed, +Z up; planes are
//! `n . p = d` (`Plane3d`), a signed plane index carries a flip bit in its
//! top bit. Children and 2D roots are raw `i32`s as stored; `Child` decodes
//! them. The trace is bounded (a depth limit on the 3D walk, a step limit
//! on 2D walks, a cap on the leaf list) and never panics on corrupt data:
//! a bad index is refused when the BSP is read, and anything the walk still
//! cannot follow counts as solid space.
//!
//! Known shortcuts:
//! - The sphere query, the pill mover and the pill sweep (collision.md,
//!   "reference only") are not read: they belong to the movement code.
//! - The trace's media (fog plane) check and "keep the location valid"
//!   step are the caller's; so is the object trace of a model.
//! - A model's health, shield and damage fields (`health_shields.md`) and
//!   its pathfinding spheres are skipped.
//! - Block maxima of a model's own blocks (materials, regions, nodes,
//!   permutations) are our sanity bounds; the spec gives none.

use std::io;

use crate::cache::{MapFile, Plane3d, RealRect3d, TagEntry, TagRef, View, class};

/// The most vertices a surface's ring may have.
pub const MAX_RING: usize = 8;
/// The deepest 3D walk the trace allows (collision.md, Constants).
pub const MAX_DEPTH_3D: usize = 128;
/// The most leaves a trace records.
pub const MAX_TRACE_LEAVES: usize = 256;
/// A collision BSP struct's size.
pub const BSP_SIZE: usize = 0x60;

const MAX_NODES_3D: usize = 131_072;
const MAX_PLANES: usize = 65_536;
const MAX_LEAVES: usize = 65_536;
const MAX_REFERENCES: usize = 131_072;
const MAX_NODES_2D: usize = 65_535;
const MAX_SURFACES: usize = 131_072;
const MAX_EDGES: usize = 262_144;
const MAX_VERTICES: usize = 131_072;

/// The structure BSP's collision materials block (offset, element size).
const LEVEL_MATERIALS: (usize, usize) = (0xA4, 0x14);
/// The structure BSP's collision BSP block.
const LEVEL_BSP: usize = 0xB0;
/// The structure BSP's main struct size (structure_bsp.md).
const STRUCTURE_SIZE: usize = 0x288;
/// A `coll` tag's main struct size.
const COLL_SIZE: usize = 0x298;

/// A decoded child index of a 3D or 2D tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Child {
    /// A tree node (index into the nodes block).
    Node(u32),
    /// A terminal: a leaf in the 3D tree, a surface in a 2D tree.
    Terminal(u32),
    /// -1: in the 3D tree solid space.
    None,
}

impl Child {
    pub fn from_raw(raw: i32) -> Child {
        if raw == -1 {
            Child::None
        } else if raw < 0 {
            Child::Terminal(raw as u32 & 0x7FFF_FFFF)
        } else {
            Child::Node(raw as u32)
        }
    }
}

/// A signed plane index: the plane's index and whether it is used flipped.
pub fn split_plane(raw: i32) -> (usize, bool) {
    ((raw as u32 & 0x7FFF_FFFF) as usize, raw < 0)
}

/// A plane with the flip applied (normal and distance negated).
pub fn flipped(p: Plane3d, flip: bool) -> Plane3d {
    if flip {
        Plane3d {
            normal: [-p.normal[0], -p.normal[1], -p.normal[2]],
            d: -p.d,
        }
    } else {
        p
    }
}

/// The projection axis (0 x, 1 y, 2 z) of a normal.
pub fn projection_axis(n: [f32; 3]) -> usize {
    let (x, y, z) = (n[0].abs(), n[1].abs(), n[2].abs());
    if z >= y && z >= x {
        2
    } else if y >= x {
        1
    } else {
        0
    }
}

/// The 2D coordinates of `p` for a projection axis and sign (the spec's
/// table).
pub fn project(axis: usize, sign: bool, p: [f32; 3]) -> [f32; 2] {
    match (axis, sign) {
        (0, false) => [p[2], p[1]],
        (0, true) => [p[1], p[2]],
        (1, false) => [p[0], p[2]],
        (1, true) => [p[2], p[0]],
        (_, false) => [p[1], p[0]],
        (_, true) => [p[0], p[1]],
    }
}

/// The projection a 2D reference uses: the axis of the stored plane's
/// normal and the sign (the normal's component along the axis is positive,
/// XOR the reference's flip bit).
pub fn reference_projection(stored_normal: [f32; 3], flip: bool) -> (usize, bool) {
    let axis = projection_axis(stored_normal);
    (axis, (stored_normal[axis] > 0.0) != flip)
}

/// A 3D tree node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node3d {
    /// Index into the planes (never flipped on the disc).
    pub plane: i32,
    /// Child taken when the point is behind the plane (dist < 0).
    pub back: i32,
    /// Child taken when dist >= 0.
    pub front: i32,
}

/// A leaf (open space).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Leaf {
    /// Bit 0: contains two-sided surfaces.
    pub flags: u16,
    pub reference_count: i16,
    pub first_reference: i32,
}

impl Leaf {
    pub fn contains_two_sided(&self) -> bool {
        self.flags & 1 != 0
    }
}

/// A 2D reference: a boundary plane of a leaf and the root of the 2D tree
/// partitioning it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reference2d {
    /// Signed plane index.
    pub plane: i32,
    /// A 2D node, or a surface as a terminal.
    pub root: i32,
}

/// A 2D tree node: `side = a*u + b*v - c`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Node2d {
    pub plane: [f32; 3],
    /// Taken when side < 0.
    pub left: i32,
    /// Taken when side >= 0.
    pub right: i32,
}

/// A surface's flag byte.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SurfaceFlags(pub u8);

impl SurfaceFlags {
    pub fn two_sided(self) -> bool {
        self.0 & 1 != 0
    }
    pub fn invisible(self) -> bool {
        self.0 & 2 != 0
    }
    pub fn climbable(self) -> bool {
        self.0 & 4 != 0
    }
    pub fn breakable(self) -> bool {
        self.0 & 8 != 0
    }
}

/// A convex polygon of the winged-edge mesh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Surface {
    /// Signed plane index; the flipped plane is the outward (front) plane.
    pub plane: i32,
    pub first_edge: i32,
    pub flags: SurfaceFlags,
    /// Index into the structure BSP's breakable surfaces (with bit 3).
    pub breakable_surface: i8,
    /// Index into the level's (or the model's) materials; -1 none.
    pub material: i16,
}

/// A winged edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edge {
    pub start: i32,
    pub end: i32,
    /// Next edge of the left surface's ring.
    pub forward: i32,
    /// Next edge of the right surface's ring.
    pub reverse: i32,
    /// The surface whose ring runs start to end (-1 none).
    pub left: i32,
    /// The surface whose ring runs end to start (-1 none).
    pub right: i32,
}

/// A mesh vertex.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub point: [f32; 3],
    pub first_edge: i32,
}

/// One triangle of a surface's fan: three indices into the vertices, in
/// counter-clockwise order about the outward normal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Triangle {
    pub surface: u32,
    pub vertices: [u32; 3],
}

/// A collision BSP: the eight blocks of collision.md.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct CollisionBsp {
    pub nodes: Vec<Node3d>,
    pub planes: Vec<Plane3d>,
    pub leaves: Vec<Leaf>,
    pub references: Vec<Reference2d>,
    pub nodes_2d: Vec<Node2d>,
    pub surfaces: Vec<Surface>,
    pub edges: Vec<Edge>,
    pub vertices: Vec<Vertex>,
}

/// What a trace may hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TraceFlags(pub u8);

impl TraceFlags {
    /// Surfaces crossed from open space into solid (from their front).
    pub const FRONT_FACING: TraceFlags = TraceFlags(1);
    /// Surfaces crossed from solid into open space.
    pub const BACK_FACING: TraceFlags = TraceFlags(2);
    pub const IGNORE_TWO_SIDED: TraceFlags = TraceFlags(4);
    pub const IGNORE_INVISIBLE: TraceFlags = TraceFlags(8);
    pub const IGNORE_BREAKABLE: TraceFlags = TraceFlags(16);
    pub const FRONT_AND_BACK: TraceFlags = TraceFlags(3);

    pub fn has(self, f: TraceFlags) -> bool {
        self.0 & f.0 != 0
    }
}

impl std::ops::BitOr for TraceFlags {
    type Output = TraceFlags;
    fn bitor(self, o: TraceFlags) -> TraceFlags {
        TraceFlags(self.0 | o.0)
    }
}

/// What a trace hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TraceHit {
    pub surface: u32,
    /// The surface's outward plane (flip applied).
    pub plane: Plane3d,
    pub flags: SurfaceFlags,
    pub breakable_surface: i8,
    pub material: i16,
    /// `start + t * vector`.
    pub point: [f32; 3],
}

/// A trace's result.
#[derive(Clone, Debug, PartialEq)]
pub struct TraceResult {
    /// The fraction along the vector of the hit; 1 when nothing was hit.
    pub t: f32,
    pub hit: Option<TraceHit>,
    /// The leaves visited in order (at most 256; later ones overwrite the
    /// last entry).
    pub leaves: Vec<u32>,
}

fn invalid_at(v: &View<'_>, what: impl std::fmt::Display) -> io::Error {
    v.error(0, format_args!("collision bsp: {what}"))
}

fn child_ok(raw: i32, nodes: usize, terminals: usize) -> bool {
    match Child::from_raw(raw) {
        Child::None => true,
        Child::Node(n) => (n as usize) < nodes,
        Child::Terminal(t) => (t as usize) < terminals,
    }
}

impl CollisionBsp {
    /// Read a collision BSP from its 0x60-byte struct and check every index.
    pub fn read(v: &View<'_>) -> io::Result<CollisionBsp> {
        let nodes: Vec<Node3d> = v
            .block_max(0x00, 0x0C, MAX_NODES_3D)?
            .iter()
            .map(|e| {
                Ok(Node3d {
                    plane: e.i32(0)?,
                    back: e.i32(4)?,
                    front: e.i32(8)?,
                })
            })
            .collect::<io::Result<_>>()?;
        let planes = v
            .block_max(0x0C, 0x10, MAX_PLANES)?
            .iter()
            .map(|e| e.plane3d(0))
            .collect::<io::Result<Vec<_>>>()?;
        let leaves = v
            .block_max(0x18, 0x08, MAX_LEAVES)?
            .iter()
            .map(|e| {
                Ok(Leaf {
                    flags: e.u16(0)?,
                    reference_count: e.i16(2)?,
                    first_reference: e.i32(4)?,
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        let references = v
            .block_max(0x24, 0x08, MAX_REFERENCES)?
            .iter()
            .map(|e| {
                Ok(Reference2d {
                    plane: e.i32(0)?,
                    root: e.i32(4)?,
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        let nodes_2d = v
            .block_max(0x30, 0x14, MAX_NODES_2D)?
            .iter()
            .map(|e| {
                Ok(Node2d {
                    plane: e.point3(0)?,
                    left: e.i32(12)?,
                    right: e.i32(16)?,
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        let surfaces = v
            .block_max(0x3C, 0x0C, MAX_SURFACES)?
            .iter()
            .map(|e| {
                Ok(Surface {
                    plane: e.i32(0)?,
                    first_edge: e.i32(4)?,
                    flags: SurfaceFlags(e.u8(8)?),
                    breakable_surface: e.i8(9)?,
                    material: e.i16(10)?,
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        let edges = v
            .block_max(0x48, 0x18, MAX_EDGES)?
            .iter()
            .map(|e| {
                Ok(Edge {
                    start: e.i32(0)?,
                    end: e.i32(4)?,
                    forward: e.i32(8)?,
                    reverse: e.i32(12)?,
                    left: e.i32(16)?,
                    right: e.i32(20)?,
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        let vertices = v
            .block_max(0x54, 0x10, MAX_VERTICES)?
            .iter()
            .map(|e| {
                Ok(Vertex {
                    point: e.point3(0)?,
                    first_edge: e.i32(12)?,
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        let bsp = CollisionBsp {
            nodes,
            planes,
            leaves,
            references,
            nodes_2d,
            surfaces,
            edges,
            vertices,
        };
        bsp.check().map_err(|e| invalid_at(v, e))?;
        Ok(bsp)
    }

    /// Every index in range, so the queries cannot index out of bounds.
    fn check(&self) -> Result<(), String> {
        let (np, nl, nn2, ns, ne, nv) = (
            self.planes.len(),
            self.leaves.len(),
            self.nodes_2d.len(),
            self.surfaces.len(),
            self.edges.len(),
            self.vertices.len(),
        );
        let plane_ok = |raw: i32| split_plane(raw).0 < np;
        for (i, n) in self.nodes.iter().enumerate() {
            if !plane_ok(n.plane) {
                return Err(format!("3d node {i}: plane {} of {np}", n.plane));
            }
            for c in [n.back, n.front] {
                if !child_ok(c, self.nodes.len(), nl) {
                    return Err(format!("3d node {i}: child {c:#x} out of range"));
                }
            }
        }
        for (i, l) in self.leaves.iter().enumerate() {
            let end = l.first_reference as i64 + l.reference_count as i64;
            // A leaf without references may name any first index (-1 on the disc).
            if l.reference_count < 0
                || (l.reference_count > 0 && (l.first_reference < 0 || end > self.references.len() as i64))
            {
                return Err(format!(
                    "leaf {i}: references {}+{} of {}",
                    l.first_reference,
                    l.reference_count,
                    self.references.len()
                ));
            }
        }
        for (i, r) in self.references.iter().enumerate() {
            if !plane_ok(r.plane) {
                return Err(format!("2d reference {i}: plane {} of {np}", r.plane));
            }
            if matches!(Child::from_raw(r.root), Child::None) || !child_ok(r.root, nn2, ns) {
                return Err(format!("2d reference {i}: root {:#x} out of range", r.root));
            }
        }
        for (i, n) in self.nodes_2d.iter().enumerate() {
            for c in [n.left, n.right] {
                if matches!(Child::from_raw(c), Child::None) || !child_ok(c, nn2, ns) {
                    return Err(format!("2d node {i}: child {c:#x} out of range"));
                }
            }
        }
        for (i, s) in self.surfaces.iter().enumerate() {
            if !plane_ok(s.plane) {
                return Err(format!("surface {i}: plane {} of {np}", s.plane));
            }
            if s.first_edge < 0 || s.first_edge as usize >= ne {
                return Err(format!("surface {i}: first edge {} of {ne}", s.first_edge));
            }
        }
        let idx = |x: i32, n: usize| x >= 0 && (x as usize) < n;
        for (i, e) in self.edges.iter().enumerate() {
            if !(idx(e.start, nv) && idx(e.end, nv) && idx(e.forward, ne) && idx(e.reverse, ne)) {
                return Err(format!("edge {i}: a vertex or edge index out of range"));
            }
            for s in [e.left, e.right] {
                if s != -1 && !idx(s, ns) {
                    return Err(format!("edge {i}: surface {s} of {ns}"));
                }
            }
        }
        for (i, v) in self.vertices.iter().enumerate() {
            if v.first_edge != -1 && !idx(v.first_edge, ne) {
                return Err(format!("vertex {i}: first edge {} of {ne}", v.first_edge));
            }
        }
        Ok(())
    }

    /// The plane a signed plane index names, flip applied.
    pub fn signed_plane(&self, raw: i32) -> Option<Plane3d> {
        let (i, flip) = split_plane(raw);
        self.planes.get(i).map(|&p| flipped(p, flip))
    }

    /// A surface's outward (front) plane.
    pub fn surface_plane(&self, surface: usize) -> Option<Plane3d> {
        self.signed_plane(self.surfaces.get(surface)?.plane)
    }

    /// A surface's ring as vertex indices, in the game's order (the spec's
    /// ring walk). Counter-clockwise seen from the surface's front. A ring
    /// that leaves the surface, or runs past `MAX_RING`, is an error.
    pub fn polygon(&self, surface: usize) -> io::Result<Vec<u32>> {
        let bad = |what: String| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("collision surface {surface}: {what}"),
            )
        };
        let s = self
            .surfaces
            .get(surface)
            .ok_or_else(|| bad("no such surface".into()))?;
        let first = s.first_edge as usize;
        let me = surface as i32;
        let mut e = first;
        let mut out = Vec::with_capacity(4);
        loop {
            let edge = self.edges.get(e).ok_or_else(|| bad(format!("edge {e} out of range")))?;
            let (vertex, next) = if edge.right == me {
                (edge.end, edge.reverse)
            } else if edge.left == me {
                (edge.start, edge.forward)
            } else {
                return Err(bad(format!("edge {e} does not border it")));
            };
            out.push(vertex as u32);
            if out.len() > MAX_RING {
                return Err(bad(format!("ring longer than {MAX_RING} edges")));
            }
            if next as usize == first {
                break;
            }
            e = next as usize;
        }
        if out.len() < 3 {
            return Err(bad(format!("ring of {} edges", out.len())));
        }
        Ok(out)
    }

    /// A surface's ring as points.
    pub fn polygon_points(&self, surface: usize) -> io::Result<Vec<[f32; 3]>> {
        self.polygon(surface)?
            .into_iter()
            .map(|v| {
                self.vertices.get(v as usize).map(|v| v.point).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("collision surface {surface}: vertex {v} out of range"),
                    )
                })
            })
            .collect()
    }

    /// Every surface fan-triangulated from its first vertex, in surface
    /// order; each triangle keeps the polygon's counter-clockwise winding.
    pub fn triangulate(&self) -> io::Result<Vec<Triangle>> {
        let mut out = Vec::with_capacity(self.surfaces.len() * 2);
        for s in 0..self.surfaces.len() {
            let ring = self.polygon(s)?;
            for i in 1..ring.len() - 1 {
                out.push(Triangle {
                    surface: s as u32,
                    vertices: [ring[0], ring[i], ring[i + 1]],
                });
            }
        }
        Ok(out)
    }

    /// The signed distance of `p` to a plane index (flip applied).
    fn dist(&self, plane: i32, p: [f32; 3]) -> f32 {
        let (i, flip) = split_plane(plane);
        let Some(pl) = self.planes.get(i) else { return f32::NAN };
        let d = pl.normal[0] * p[0] + pl.normal[1] * p[1] + pl.normal[2] * p[2] - pl.d;
        if flip { -d } else { d }
    }

    /// The point test: the leaf containing `p`, or `None` for solid space
    /// (everything outside the level too).
    pub fn point_leaf(&self, p: [f32; 3]) -> Option<u32> {
        let mut child = Child::from_raw(if self.nodes.is_empty() { -1 } else { 0 });
        for _ in 0..=MAX_DEPTH_3D {
            match child {
                Child::Node(n) => {
                    let node = self.nodes.get(n as usize)?;
                    let d = self.dist(node.plane, p);
                    child = Child::from_raw(if d >= 0.0 { node.front } else { node.back });
                }
                Child::Terminal(l) => return ((l as usize) < self.leaves.len()).then_some(l),
                Child::None => return None,
            }
        }
        None
    }

    /// The surface a 2D tree reaches for `uv`, or `None`.
    fn walk_2d(&self, root: i32, uv: [f32; 2]) -> Option<u32> {
        let mut c = Child::from_raw(root);
        for _ in 0..=self.nodes_2d.len() {
            match c {
                Child::Node(n) => {
                    let node = self.nodes_2d.get(n as usize)?;
                    let side = node.plane[0] * uv[0] + node.plane[1] * uv[1] - node.plane[2];
                    c = Child::from_raw(if side < 0.0 { node.left } else { node.right });
                }
                Child::Terminal(s) => return ((s as usize) < self.surfaces.len()).then_some(s),
                Child::None => return None,
            }
        }
        None
    }

    /// The line trace of collision.md: the segment `start + t * vector`,
    /// `t` in `[0, min(max_t, 1)]`. `is_broken(i)` says whether breakable
    /// surface `i` is broken (a broken breakable surface is skipped); pass
    /// `&|_| false` when nothing is.
    pub fn trace(
        &self,
        start: [f32; 3],
        vector: [f32; 3],
        flags: TraceFlags,
        max_t: f32,
        is_broken: &dyn Fn(u8) -> bool,
    ) -> TraceResult {
        let both = !flags.has(TraceFlags::FRONT_FACING) && !flags.has(TraceFlags::BACK_FACING);
        let mut w = Walk {
            bsp: self,
            p: start,
            v: vector,
            front: both || flags.has(TraceFlags::FRONT_FACING),
            back: both || flags.has(TraceFlags::BACK_FACING),
            flags,
            is_broken,
            prev: None,
            plane_last: None,
            best_t: max_t.max(0.0),
            hit: None,
            leaves: Vec::new(),
        };
        let t1 = max_t.min(1.0);
        let hit_t = if w.nodes_exist() && w.visit(Child::Node(0), 0.0, t1, 0) {
            w.hit.map(|(t, _)| t)
        } else {
            None
        };
        TraceResult {
            t: hit_t.unwrap_or(1.0),
            hit: w.hit.map(|(_, h)| h),
            leaves: w.leaves,
        }
    }

    /// `trace` with nothing broken and no limit on `t`.
    pub fn trace_ray(&self, start: [f32; 3], vector: [f32; 3], flags: TraceFlags) -> TraceResult {
        self.trace(start, vector, flags, f32::INFINITY, &|_| false)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Contents {
    Solid,
    Open,
    OpenTwoSided,
}

struct Walk<'a> {
    bsp: &'a CollisionBsp,
    p: [f32; 3],
    v: [f32; 3],
    front: bool,
    back: bool,
    flags: TraceFlags,
    is_broken: &'a dyn Fn(u8) -> bool,
    /// The previous leaf's contents and index.
    prev: Option<(Contents, Option<u32>)>,
    /// The plane last crossed (index, flip ignored).
    plane_last: Option<usize>,
    best_t: f32,
    hit: Option<(f32, TraceHit)>,
    leaves: Vec<u32>,
}

impl Walk<'_> {
    fn nodes_exist(&self) -> bool {
        !self.bsp.nodes.is_empty()
    }

    fn visit(&mut self, child: Child, t0: f32, t1: f32, depth: usize) -> bool {
        match child {
            Child::Node(n) => {
                if depth >= MAX_DEPTH_3D {
                    return false;
                }
                let Some(&node) = self.bsp.nodes.get(n as usize) else {
                    return false;
                };
                let d = self.bsp.dist(node.plane, self.p);
                let (pi, flip) = split_plane(node.plane);
                let Some(pl) = self.bsp.planes.get(pi) else {
                    return false;
                };
                let mut k = pl.normal[0] * self.v[0] + pl.normal[1] * self.v[1] + pl.normal[2] * self.v[2];
                if flip {
                    k = -k;
                }
                let (d0, d1) = (d + k * t0, d + k * t1);
                let reaches_back = d0 < 0.0 || d1 < 0.0;
                let reaches_front = d0 >= 0.0 || d1 >= 0.0;
                match (reaches_back, reaches_front) {
                    (false, false) => false,
                    (true, false) => self.visit(Child::from_raw(node.back), t0, t1, depth + 1),
                    (false, true) => self.visit(Child::from_raw(node.front), t0, t1, depth + 1),
                    (true, true) => {
                        let t = -d / k;
                        if !t.is_finite() {
                            return false;
                        }
                        let (near, far) = if k > 0.0 {
                            (node.back, node.front)
                        } else {
                            (node.front, node.back)
                        };
                        if self.visit(Child::from_raw(near), t0, t, depth + 1) {
                            return true;
                        }
                        if self.best_t <= t {
                            return false;
                        }
                        self.plane_last = Some(pi);
                        self.visit(Child::from_raw(far), t, t1, depth + 1)
                    }
                }
            }
            terminal => self.terminal(terminal, t0),
        }
    }

    fn terminal(&mut self, child: Child, t0: f32) -> bool {
        let (cur, leaf) = match child {
            Child::Terminal(l) => match self.bsp.leaves.get(l as usize) {
                Some(leaf) if leaf.contains_two_sided() => (Contents::OpenTwoSided, Some(l)),
                Some(_) => (Contents::Open, Some(l)),
                None => (Contents::Solid, None),
            },
            _ => (Contents::Solid, None),
        };
        if let Some((prev, prev_leaf)) = self.prev {
            let open = |c: Contents| c != Contents::Solid;
            let two_sided_ok = !self.flags.has(TraceFlags::IGNORE_TWO_SIDED);
            let test = if self.front && open(prev) && cur == Contents::Solid {
                Some((prev_leaf, false))
            } else if self.back && prev == Contents::Solid && open(cur) {
                Some((leaf, false))
            } else if two_sided_ok && prev == Contents::OpenTwoSided && cur == Contents::OpenTwoSided {
                Some((if self.front { prev_leaf } else { leaf }, true))
            } else {
                None
            };
            if let Some((Some(l), check_polygon)) = test
                && self.test_leaf(l, t0, check_polygon)
            {
                return true;
            }
        }
        if let Some(l) = leaf {
            if self.leaves.len() < MAX_TRACE_LEAVES {
                self.leaves.push(l);
            } else if let Some(last) = self.leaves.last_mut() {
                *last = l;
            }
        }
        self.prev = Some((cur, leaf));
        false
    }

    /// Test leaf `l` at `t0` against the plane last crossed; on a hit,
    /// record it.
    fn test_leaf(&mut self, l: u32, t0: f32, check_polygon: bool) -> bool {
        let Some(q) = self.plane_last else { return false };
        let bsp = self.bsp;
        let Some(leaf) = bsp.leaves.get(l as usize) else {
            return false;
        };
        let point = [
            self.p[0] + t0 * self.v[0],
            self.p[1] + t0 * self.v[1],
            self.p[2] + t0 * self.v[2],
        ];
        let first = leaf.first_reference.max(0) as usize;
        let count = leaf.reference_count.max(0) as usize;
        for r in bsp.references.iter().skip(first).take(count) {
            let (pi, flip) = split_plane(r.plane);
            if pi != q {
                continue;
            }
            let Some(stored) = bsp.planes.get(pi) else { continue };
            let (axis, sign) = reference_projection(stored.normal, flip);
            let uv = project(axis, sign, point);
            let Some(s) = bsp.walk_2d(r.root, uv) else { continue };
            let Some(&surface) = bsp.surfaces.get(s as usize) else {
                continue;
            };
            if surface.flags.breakable() && (self.is_broken)(surface.breakable_surface as u8) {
                continue;
            }
            if check_polygon {
                let Ok(ring) = bsp.polygon(s as usize) else { continue };
                let mut inside = true;
                for i in 0..ring.len() {
                    let (Some(a), Some(b)) = (
                        bsp.vertices.get(ring[i] as usize),
                        bsp.vertices.get(ring[(i + 1) % ring.len()] as usize),
                    ) else {
                        inside = false;
                        break;
                    };
                    let a = project(axis, sign, a.point);
                    let b = project(axis, sign, b.point);
                    let (u1, v1) = (uv[0] - a[0], uv[1] - a[1]);
                    let (u2, v2) = (b[0] - a[0], b[1] - a[1]);
                    if u1 * v2 - v1 * u2 > 0.0 {
                        inside = false;
                        break;
                    }
                }
                if !inside {
                    continue;
                }
            }
            if (surface.flags.invisible() && self.flags.has(TraceFlags::IGNORE_INVISIBLE))
                || (surface.flags.breakable() && self.flags.has(TraceFlags::IGNORE_BREAKABLE))
            {
                continue;
            }
            let Some(plane) = bsp.signed_plane(surface.plane) else {
                continue;
            };
            self.best_t = t0;
            self.hit = Some((
                t0,
                TraceHit {
                    surface: s,
                    plane,
                    flags: surface.flags,
                    breakable_surface: surface.breakable_surface,
                    material: surface.material,
                    point,
                },
            ));
            return true;
        }
        false
    }
}

/// The material types, as the tags enumerate them (collision.md,
/// "Materials"); the code is the enumeration's position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MaterialType {
    Dirt,
    Sand,
    Stone,
    Snow,
    Wood,
    MetalHollow,
    MetalThin,
    MetalThick,
    Rubber,
    Glass,
    ForceField,
    Grunt,
    HunterArmor,
    HunterSkin,
    Elite,
    Jackal,
    JackalEnergyShield,
    Engineer,
    EngineerForceField,
    FloodCombatForm,
    FloodCarrierForm,
    Cyborg,
    CyborgEnergyShield,
    ArmoredHuman,
    Human,
    Sentinel,
    Monitor,
    Plastic,
    Water,
    Leaves,
    EliteEnergyShield,
    Ice,
    HunterShield,
}

impl MaterialType {
    /// Every type, in code order.
    pub const ALL: [MaterialType; 33] = {
        use MaterialType::*;
        [
            Dirt,
            Sand,
            Stone,
            Snow,
            Wood,
            MetalHollow,
            MetalThin,
            MetalThick,
            Rubber,
            Glass,
            ForceField,
            Grunt,
            HunterArmor,
            HunterSkin,
            Elite,
            Jackal,
            JackalEnergyShield,
            Engineer,
            EngineerForceField,
            FloodCombatForm,
            FloodCarrierForm,
            Cyborg,
            CyborgEnergyShield,
            ArmoredHuman,
            Human,
            Sentinel,
            Monitor,
            Plastic,
            Water,
            Leaves,
            EliteEnergyShield,
            Ice,
            HunterShield,
        ]
    };

    /// The type for a stored code; a code outside 0..=32 (and -1) is none.
    pub fn from_code(code: i16) -> Option<MaterialType> {
        usize::try_from(code).ok().and_then(|c| Self::ALL.get(c).copied())
    }

    pub fn code(self) -> i16 {
        self as i16
    }
}

/// A level collision material (0x14 bytes): the shader drawn on the
/// surface and its precomputed type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CollisionMaterial {
    pub shader: TagRef,
    /// As stored; `material_type()` decodes it.
    pub material_type_code: i16,
}

impl CollisionMaterial {
    pub fn material_type(&self) -> Option<MaterialType> {
        MaterialType::from_code(self.material_type_code)
    }
}

/// A level's collision data: the collision materials and the BSP.
#[derive(Clone, Debug, PartialEq)]
pub struct LevelCollision {
    pub materials: Vec<CollisionMaterial>,
    pub bsp: CollisionBsp,
}

impl LevelCollision {
    /// The type of a surface's material (`None` for material -1, an index
    /// past the list, or a type outside 0..=32).
    pub fn surface_material_type(&self, surface: usize) -> Option<MaterialType> {
        let m = usize::try_from(self.bsp.surfaces.get(surface)?.material).ok()?;
        self.materials.get(m)?.material_type()
    }
}

/// Read a structure BSP's collision materials block (offset 0xA4).
pub fn read_collision_materials(v: &View<'_>) -> io::Result<Vec<CollisionMaterial>> {
    let (off, size) = LEVEL_MATERIALS;
    v.block(off, size)?
        .iter()
        .map(|e| {
            Ok(CollisionMaterial {
                shader: e.tag_ref(0)?,
                material_type_code: e.i16(0x12)?,
            })
        })
        .collect()
}

/// Read the level collision of structure BSP `bsp_index` of `map`: the
/// collision materials (0xA4) and the single collision BSP (0xB0).
pub fn read_level(map: &MapFile, bsp_index: usize) -> io::Result<LevelCollision> {
    let v = map.bsp_view(bsp_index, STRUCTURE_SIZE)?;
    let materials = read_collision_materials(&v)?;
    let block = v.block_max(LEVEL_BSP, BSP_SIZE, 1)?;
    if block.len() != 1 {
        return Err(v.error(LEVEL_BSP, format_args!("{} collision BSPs, expected 1", block.len())));
    }
    let bsp = CollisionBsp::read(&block.get(0)?)?;
    for (i, s) in bsp.surfaces.iter().enumerate() {
        if s.material < -1 || s.material as i64 >= materials.len() as i64 {
            return Err(v.error(
                LEVEL_BSP,
                format_args!("collision surface {i}: material {} of {}", s.material, materials.len()),
            ));
        }
    }
    Ok(LevelCollision { materials, bsp })
}

/// A collision model material (0x48 bytes).
#[derive(Clone, Debug, PartialEq)]
pub struct ModelMaterial {
    pub name: String,
    /// Bit 0: head.
    pub flags: u32,
    pub material_type_code: i16,
}

impl ModelMaterial {
    pub fn material_type(&self) -> Option<MaterialType> {
        MaterialType::from_code(self.material_type_code)
    }
}

/// A collision model region (0x54 bytes).
#[derive(Clone, Debug, PartialEq)]
pub struct ModelRegion {
    pub name: String,
    pub flags: u32,
    /// The permutations' names; a node's BSPs are one per permutation of
    /// its region.
    pub permutations: Vec<String>,
}

/// A collision model node (0x40 bytes) with its BSPs.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelNode {
    pub name: String,
    pub region: Option<u16>,
    pub parent: Option<u16>,
    pub next_sibling: Option<u16>,
    pub first_child: Option<u16>,
    /// One per permutation of the node's region.
    pub bsps: Vec<CollisionBsp>,
}

/// A `coll` tag: materials, regions and nodes (health, shields and damage
/// are `health_shields.md`'s).
#[derive(Clone, Debug, PartialEq)]
pub struct CollisionModel {
    pub materials: Vec<ModelMaterial>,
    pub regions: Vec<ModelRegion>,
    pub nodes: Vec<ModelNode>,
}

const MAX_MODEL_MATERIALS: usize = 256;
const MAX_MODEL_REGIONS: usize = 256;
const MAX_MODEL_NODES: usize = 256;
const MAX_NODE_BSPS: usize = 256;

fn opt_index(v: &View<'_>, off: usize, limit: usize, what: &str) -> io::Result<Option<u16>> {
    match v.index16(off)? {
        None => Ok(None),
        Some(i) if i < limit => Ok(Some(i as u16)),
        Some(i) => Err(v.error(off, format_args!("{what} index {i} of {limit}"))),
    }
}

/// Read a `coll` tag.
pub fn read_model(map: &MapFile, tag: &TagEntry) -> io::Result<CollisionModel> {
    if tag.class != class::MODEL_COLLISION_GEOMETRY {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{}: {} `{}` is not a collision model",
                map.header().name,
                tag.class,
                tag.path
            ),
        ));
    }
    let v = map.tag_view(tag, COLL_SIZE)?;
    let materials = v
        .block_max(0x234, 0x48, MAX_MODEL_MATERIALS)?
        .iter()
        .map(|e| {
            Ok(ModelMaterial {
                name: e.tag_string(0)?.to_owned(),
                flags: e.u32(0x20)?,
                material_type_code: e.i16(0x24)?,
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    let regions = v
        .block_max(0x240, 0x54, MAX_MODEL_REGIONS)?
        .iter()
        .map(|e| {
            Ok(ModelRegion {
                name: e.tag_string(0)?.to_owned(),
                flags: e.u32(0x20)?,
                permutations: e
                    .block_max(0x48, 32, MAX_NODE_BSPS)?
                    .iter()
                    .map(|p| p.tag_string(0).map(str::to_owned))
                    .collect::<io::Result<Vec<_>>>()?,
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    let node_block = v.block_max(0x28C, 0x40, MAX_MODEL_NODES)?;
    let n = node_block.len();
    let mut nodes = Vec::with_capacity(n);
    for e in node_block.iter() {
        let bsps = e
            .block_max(0x34, BSP_SIZE, MAX_NODE_BSPS)?
            .iter()
            .map(|b| CollisionBsp::read(&b))
            .collect::<io::Result<Vec<_>>>()?;
        for b in &bsps {
            for (i, s) in b.surfaces.iter().enumerate() {
                if s.material < -1 || s.material as i64 >= materials.len() as i64 {
                    return Err(e.error(
                        0x34,
                        format_args!("collision surface {i}: material {} of {}", s.material, materials.len()),
                    ));
                }
            }
        }
        nodes.push(ModelNode {
            name: e.tag_string(0)?.to_owned(),
            region: opt_index(&e, 0x20, regions.len(), "region")?,
            parent: opt_index(&e, 0x22, n, "parent node")?,
            next_sibling: opt_index(&e, 0x24, n, "sibling node")?,
            first_child: opt_index(&e, 0x26, n, "child node")?,
            bsps,
        });
    }
    Ok(CollisionModel {
        materials,
        regions,
        nodes,
    })
}

/// The bounding box of a BSP's vertices (`None` without vertices).
pub fn vertex_bounds(bsp: &CollisionBsp) -> Option<RealRect3d> {
    use crate::cache::Bounds;
    let first = bsp.vertices.first()?.point;
    let mut lo = first;
    let mut hi = first;
    for v in &bsp.vertices {
        for a in 0..3 {
            lo[a] = lo[a].min(v.point[a]);
            hi[a] = hi[a].max(v.point[a]);
        }
    }
    Some(RealRect3d {
        x: Bounds {
            lower: lo[0],
            upper: hi[0],
        },
        y: Bounds {
            lower: lo[1],
            upper: hi[1],
        },
        z: Bounds {
            lower: lo[2],
            upper: hi[2],
        },
    })
}
