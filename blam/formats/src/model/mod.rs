//! The render model tag (`mode`) reader, from specs/halo_ce/models.md:
//! `Model::read` gives a model's fields, markers, skeleton, regions with
//! their permutations, geometries with their parts (every part's vertices
//! decoded from the Xbox's compressed vertex and its triangle strip turned
//! into triangles) and shader references as plain structs, in the game's
//! own units and axes (no conversion).
//!
//! - `Model::read(map, tag)`: everything eagerly, parts' buffers included
//!   (reached through the tag header's buffer records, `MapFile::
//!   model_vertex_buffers`). Errors name the map, the tag and the offset.
//! - Skeleton: `Model::bind_pose` composes every node's model-space
//!   `Transform` with the spec's quaternion convention (a stored
//!   quaternion is the usual rotation of its conjugate: `rotation_axes`);
//!   `Node::inverse` is the stored default inverse matrix. Markers:
//!   `Model::find_marker` (case-insensitive), `Model::marker_world`.
//! - Parts: `Part::vertices`, `Part::strip` (as stored), `Part::triangles`
//!   (the strip's non-degenerate triangles in the game's winding:
//!   **clockwise** seen from the front, the side the vertex normals point
//!   to, in the right-handed frame) and `Part::triangles_ccw` (the same
//!   with each triangle reversed: counter-clockwise front faces, the usual
//!   convention for right-handed renderers).
//! - `Model::parts_at(level, permutations)` and
//!   `Model::highest_detail_parts` gather the parts to draw for a level of
//!   detail and a permutation per region, with their shader references;
//!   `Model::level_for_size` is the spec's level of detail choice.
//!
//! Levels of detail are numbered as the engine uses them: level 0 is the
//! lowest detail, level 4 the highest (`Permutation::geometry_indices`).
//!
//! Known shortcuts:
//! - Only the Xbox's parts are read: compressed vertices (buffer type 5)
//!   in a triangle strip (type 1). The uncompressed vertex and triangle
//!   list forms of the spec exist on no disc map and refuse the part.
//! - A vertex whose node byte is not a multiple of 3, or names a node past
//!   the node list, refuses the model (the spec allows treating it as
//!   rigid to node 0); the same for a marker instance's node. A marker's
//!   region and permutation bytes are stored as they are.
//! - A weight outside 0..32767 is clamped; a vertex with no second node
//!   gets the weight 1.
//! - The model's flags and node list checksum are stored, not interpreted.

mod math;

use std::io;

use crate::cache::{BUFFER_RECORD_SIZE, MapFile, TagEntry, TagRef, View, class};

pub use math::{Transform, rotation_axes, strip_triangles, unpack_coordinate, unpack_vector};

/// The model's main struct.
pub const MODEL_SIZE: usize = 0xE8;
pub const MARKER_SIZE: usize = 0x40;
pub const MARKER_INSTANCE_SIZE: usize = 0x20;
pub const NODE_SIZE: usize = 0x9C;
pub const REGION_SIZE: usize = 0x4C;
pub const PERMUTATION_SIZE: usize = 0x58;
pub const PERMUTATION_MARKER_SIZE: usize = 0x50;
pub const GEOMETRY_SIZE: usize = 0x30;
pub const PART_SIZE: usize = 0x68;
pub const SHADER_REF_SIZE: usize = 0x20;
/// One compressed model vertex.
pub const VERTEX_SIZE: usize = 0x20;
/// Levels of detail; level 0 is the lowest, level 4 the highest.
pub const DETAIL_LEVELS: usize = 5;
/// The vertex buffer type of every part on the disc.
pub const VERTEX_TYPE_COMPRESSED: i16 = 5;
/// The index buffer type of every part on the disc (a triangle strip).
pub const INDEX_TYPE_STRIP: i16 = 1;
/// Vertex node bytes are the node index times this.
pub const NODE_INDEX_SCALE: i32 = 3;
/// The node byte for "no second node".
pub const NODE_NONE_BYTE: i8 = -3;
/// The vertex weight is an `i16` over this.
pub const WEIGHT_SCALE: f32 = 32767.0;
pub const MAX_MARKERS: usize = 256;
pub const MAX_MARKER_INSTANCES: usize = 32;
pub const MAX_NODES: usize = 64;
pub const MAX_REGIONS: usize = 32;
pub const MAX_PERMUTATIONS: usize = 32;
pub const MAX_GEOMETRIES: usize = 256;
pub const MAX_PARTS: usize = 32;
pub const MAX_SHADERS: usize = 32;
pub const MAX_VERTICES_PER_PART: usize = 65535;
/// The part flag "stripped": the engine does not draw such a part.
pub const PART_STRIPPED: u32 = 1;

/// A render model.
#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    /// Bit 0 blend shared normals, bit 1 parts have local nodes (the PC
    /// format); stored, not interpreted.
    pub flags: u32,
    /// The tools' checksum of the node list; stored, never recomputed.
    pub node_list_checksum: u32,
    /// The projected size in pixels from which each level may be used,
    /// level 0 first.
    pub detail_cutoffs: [f32; DETAIL_LEVELS],
    /// Per level, the highest node index a vertex of that level uses (not
    /// a count); informational.
    pub node_counts: [i16; DETAIL_LEVELS],
    /// Multiplies every decoded u (already applied to `Vertex::texcoord`).
    pub base_map_u_scale: f32,
    /// Multiplies every decoded v (already applied).
    pub base_map_v_scale: f32,
    pub markers: Vec<Marker>,
    pub nodes: Vec<Node>,
    pub regions: Vec<Region>,
    pub geometries: Vec<Geometry>,
    pub shaders: Vec<ShaderRef>,
}

/// A named frame attached to a node (a muzzle, a hand grip, a seat).
#[derive(Clone, Debug, PartialEq)]
pub struct Marker {
    pub name: String,
    pub instances: Vec<MarkerInstance>,
}

/// One frame of a marker. It counts for an object whose current
/// permutation of `region` is `permutation`.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkerInstance {
    pub region: u8,
    pub permutation: u8,
    pub node: usize,
    /// In the node's frame.
    pub translation: [f32; 3],
    /// `[i, j, k, w]` as stored (the game's convention).
    pub rotation: [f32; 4],
}

impl MarkerInstance {
    /// The frame in the node's frame.
    pub fn local(&self) -> Transform {
        Transform::from_quaternion(self.rotation, self.translation)
    }
}

/// A skeleton node.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub name: String,
    pub next_sibling: Option<usize>,
    pub first_child: Option<usize>,
    /// `None` for node 0 only; every other parent index is lower than the
    /// node's own (checked at read).
    pub parent: Option<usize>,
    /// The node's origin in its parent's frame, as stored.
    pub translation: [f32; 3],
    /// `[i, j, k, w]` as stored (the game's convention, not the usual).
    pub rotation: [f32; 4],
    /// The length of the translation (0 for the root).
    pub distance_from_parent: f32,
    /// The stored default inverse matrix: the inverse of the bind pose's
    /// world transform (the scale word is not kept; 1 on the disc).
    pub inverse: Transform,
}

impl Node {
    /// The node's transform into its parent's frame.
    pub fn local(&self) -> Transform {
        Transform::from_quaternion(self.rotation, self.translation)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    pub name: String,
    pub permutations: Vec<Permutation>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Permutation {
    pub name: String,
    /// Bit 0: cannot be chosen randomly.
    pub flags: u32,
    pub variant_number: i16,
    /// One geometry per level, level 0 (lowest detail) first; `None`
    /// draws nothing at that level.
    pub geometry_indices: [Option<usize>; DETAIL_LEVELS],
    /// Never present on the disc.
    pub markers: Vec<PermutationMarker>,
}

impl Permutation {
    /// The geometry for `level` (0 lowest .. 4 highest).
    pub fn geometry_at(&self, level: usize) -> Option<usize> {
        self.geometry_indices.get(level).copied().flatten()
    }

    /// The highest detail's geometry (level 4).
    pub fn highest_detail(&self) -> Option<usize> {
        self.geometry_indices[DETAIL_LEVELS - 1]
    }
}

/// The editing tools' per-permutation marker (never on the disc).
#[derive(Clone, Debug, PartialEq)]
pub struct PermutationMarker {
    pub name: String,
    pub node: Option<usize>,
    pub rotation: [f32; 4],
    pub translation: [f32; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    pub flags: u32,
    pub parts: Vec<Part>,
}

/// One drawable piece: a shader's triangles.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    /// Bit 0: stripped (not drawn).
    pub flags: u32,
    /// Into `Model::shaders`.
    pub shader: Option<usize>,
    pub previous_filthy_part: Option<usize>,
    pub next_filthy_part: Option<usize>,
    pub centroid_primary_node: usize,
    /// 0 when the part uses one node.
    pub centroid_secondary_node: usize,
    pub centroid_primary_weight: f32,
    pub centroid_secondary_weight: f32,
    /// Model space.
    pub centroid: [f32; 3],
    pub vertices: Vec<Vertex>,
    /// The stored strip indices (`index buffer count + 2` of them, empty
    /// when the count is 0 or less), each below `vertices.len()`.
    pub strip: Vec<u16>,
    /// The strip's non-degenerate triangles in the game's winding
    /// (clockwise seen from the front; see `strip_triangles`).
    pub triangles: Vec<[u16; 3]>,
}

impl Part {
    /// `triangles` with each reversed: counter-clockwise front faces.
    pub fn triangles_ccw(&self) -> Vec<[u16; 3]> {
        self.triangles.iter().map(|&[a, b, c]| [a, c, b]).collect()
    }

    pub fn is_stripped(&self) -> bool {
        self.flags & PART_STRIPPED != 0
    }
}

/// A decoded model vertex (model space, the bind pose).
#[derive(Clone, Debug, PartialEq)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub binormal: [f32; 3],
    pub tangent: [f32; 3],
    /// Decoded u and v times the model's base map scales.
    pub texcoord: [f32; 2],
    pub node0: usize,
    /// `None`: rigid to `node0`.
    pub node1: Option<usize>,
    /// The weight of `node0` (1 when rigid); `node1` gets `1 - weight0`.
    pub weight0: f32,
}

/// A shader reference of the model.
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderRef {
    pub shader: TagRef,
    /// Which permutation of the shader (a bitmap of a sequence, say).
    pub permutation: i16,
}

impl ShaderRef {
    /// The shader's tag, if the reference is valid.
    pub fn resolve<'a>(&self, map: &'a MapFile) -> Option<&'a TagEntry> {
        map.tag(self.shader.id)
    }
}

/// A part chosen for drawing by `Model::parts_at`.
#[derive(Clone, Copy, Debug)]
pub struct DrawPart<'a> {
    pub region: usize,
    pub permutation: usize,
    pub geometry: usize,
    /// The part's index in its geometry.
    pub part_index: usize,
    pub part: &'a Part,
    /// `None` when the part's shader index is NONE.
    pub shader: Option<&'a ShaderRef>,
}

fn err(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

/// An index field that must be NONE or below `limit`.
fn index_below(v: &View<'_>, offset: usize, limit: usize, what: &str) -> io::Result<Option<usize>> {
    let i = v.index16(offset)?;
    match i {
        Some(i) if i >= limit => Err(v.error(offset, format_args!("{what} {i} but there are {limit}"))),
        i => Ok(i),
    }
}

impl Model {
    /// Read the model tag `tag` (class `mode`) of `map`.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<Model> {
        if tag.class != class::MODEL {
            return Err(err(format!(
                "{}: {} `{}` is not a model",
                map.header().name,
                tag.class,
                tag.path
            )));
        }
        let v = map.tag_view(tag, MODEL_SIZE)?;
        let flags = v.flags32(0x00)?;
        let node_list_checksum = v.u32(0x04)?;
        let mut detail_cutoffs = [0.0; DETAIL_LEVELS];
        let mut node_counts = [0; DETAIL_LEVELS];
        for l in 0..DETAIL_LEVELS {
            detail_cutoffs[l] = v.f32(0x08 + 4 * l)?;
            node_counts[l] = v.i16(0x1C + 2 * l)?;
        }
        let u_scale = v.f32(0x30)?;
        let v_scale = v.f32(0x34)?;

        let shaders = v
            .block_max(0xDC, SHADER_REF_SIZE, MAX_SHADERS)?
            .iter()
            .map(|e| {
                Ok(ShaderRef {
                    shader: e.tag_ref(0x00)?,
                    permutation: e.i16(0x10)?,
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        let nodes = read_nodes(&v)?;
        let geometries = {
            let block = v.block_max(0xD0, GEOMETRY_SIZE, MAX_GEOMETRIES)?;
            let ctx = PartContext {
                map,
                scale: [u_scale, v_scale],
                nodes: nodes.len(),
                shaders: shaders.len(),
            };
            block
                .iter()
                .map(|g| read_geometry(&ctx, &g))
                .collect::<io::Result<Vec<_>>>()?
        };
        let regions = read_regions(&v, geometries.len(), nodes.len())?;
        let markers = read_markers(&v, nodes.len())?;
        Ok(Model {
            flags,
            node_list_checksum,
            detail_cutoffs,
            node_counts,
            base_map_u_scale: u_scale,
            base_map_v_scale: v_scale,
            markers,
            nodes,
            regions,
            geometries,
            shaders,
        })
    }

    /// Every node's model-space (bind pose) transform: `W(0) = L(0)`,
    /// `W(n) = W(parent) x L(n)`, with the spec's quaternion convention.
    pub fn bind_pose(&self) -> Vec<Transform> {
        let mut world: Vec<Transform> = Vec::with_capacity(self.nodes.len());
        for n in &self.nodes {
            let local = n.local();
            // Parents precede their children (checked at read).
            world.push(match n.parent {
                Some(p) => world[p].then(&local),
                None => local,
            });
        }
        world
    }

    /// The marker called `name`, ignoring ASCII case.
    pub fn find_marker(&self, name: &str) -> Option<&Marker> {
        self.markers.iter().find(|m| m.name.eq_ignore_ascii_case(name))
    }

    /// An instance's frame in model space: `world(node) x local`, `world`
    /// being `bind_pose()` (or a posed skeleton).
    pub fn marker_world(&self, instance: &MarkerInstance, world: &[Transform]) -> Option<Transform> {
        Some(world.get(instance.node)?.then(&instance.local()))
    }

    /// The level of detail for an object whose bounding sphere measures
    /// `size` pixels across: `None` when below the lowest cutoff (not
    /// drawn), else the highest level whose cutoff is at most `size`.
    pub fn level_for_size(&self, size: f32) -> Option<usize> {
        if size < self.detail_cutoffs[0] {
            return None;
        }
        let mut level = DETAIL_LEVELS - 1;
        while level > 0 && size < self.detail_cutoffs[level] {
            level -= 1;
        }
        Some(level)
    }

    /// The parts to draw at detail `level` (0 lowest, 4 highest) with
    /// permutation `permutations[r]` for region `r` (0 for a region past
    /// the slice), in region, then part order. A region whose permutation
    /// has no geometry at that level draws nothing; stripped parts are
    /// left out. A permutation index past the region's is an error.
    pub fn parts_at(&self, level: usize, permutations: &[usize]) -> io::Result<Vec<DrawPart<'_>>> {
        if level >= DETAIL_LEVELS {
            return Err(err(format!(
                "level of detail {level}, the levels are 0..{DETAIL_LEVELS}"
            )));
        }
        let mut out = Vec::new();
        for (r, region) in self.regions.iter().enumerate() {
            let p = permutations.get(r).copied().unwrap_or(0);
            let Some(perm) = region.permutations.get(p) else {
                if p == 0 {
                    // A region with no permutation draws nothing.
                    continue;
                }
                return Err(err(format!(
                    "region {r} `{}` has {} permutations, not {p}",
                    region.name,
                    region.permutations.len()
                )));
            };
            let Some(g) = perm.geometry_at(level) else { continue };
            for (i, part) in self.geometries[g].parts.iter().enumerate() {
                if part.is_stripped() {
                    continue;
                }
                out.push(DrawPart {
                    region: r,
                    permutation: p,
                    geometry: g,
                    part_index: i,
                    part,
                    shader: part.shader.map(|s| &self.shaders[s]),
                });
            }
        }
        Ok(out)
    }

    /// `parts_at(4, &[])`: the first permutation of every region at the
    /// highest detail.
    pub fn highest_detail_parts(&self) -> Vec<DrawPart<'_>> {
        self.parts_at(DETAIL_LEVELS - 1, &[])
            .expect("permutation 0 of each region is never an error")
    }
}

fn read_nodes(v: &View<'_>) -> io::Result<Vec<Node>> {
    let block = v.block_max(0xB8, NODE_SIZE, MAX_NODES)?;
    let n = block.len();
    let mut nodes = Vec::with_capacity(n);
    for (i, e) in block.iter().enumerate() {
        let parent = index_below(&e, 0x24, n, "parent node")?;
        match parent {
            None if i != 0 => return Err(e.error(0x24, format_args!("node {i} has no parent"))),
            Some(p) if p >= i => {
                return Err(e.error(0x24, format_args!("node {i}'s parent {p} is not below its own index")));
            }
            _ => {}
        }
        // The stored matrix: scale (0x68), forward, left, up, position.
        let inverse = Transform {
            forward: e.point3(0x6C)?,
            left: e.point3(0x78)?,
            up: e.point3(0x84)?,
            position: e.point3(0x90)?,
        };
        nodes.push(Node {
            name: e.tag_string(0x00)?.to_owned(),
            next_sibling: index_below(&e, 0x20, n, "next sibling node")?,
            first_child: index_below(&e, 0x22, n, "first child node")?,
            parent,
            translation: e.point3(0x28)?,
            rotation: e.quaternion(0x34)?,
            distance_from_parent: e.f32(0x44)?,
            inverse,
        });
    }
    Ok(nodes)
}

fn read_markers(v: &View<'_>, nodes: usize) -> io::Result<Vec<Marker>> {
    v.block_max(0xAC, MARKER_SIZE, MAX_MARKERS)?
        .iter()
        .map(|m| {
            let instances = m
                .block_max(0x34, MARKER_INSTANCE_SIZE, MAX_MARKER_INSTANCES)?
                .iter()
                .map(|e| {
                    let node = e.u8(0x02)? as usize;
                    if node >= nodes {
                        return Err(e.error(0x02, format_args!("marker node {node} but there are {nodes} nodes")));
                    }
                    Ok(MarkerInstance {
                        region: e.u8(0x00)?,
                        permutation: e.u8(0x01)?,
                        node,
                        translation: e.point3(0x04)?,
                        rotation: e.quaternion(0x10)?,
                    })
                })
                .collect::<io::Result<Vec<_>>>()?;
            Ok(Marker {
                name: m.tag_string(0x00)?.to_owned(),
                instances,
            })
        })
        .collect()
}

fn read_regions(v: &View<'_>, geometries: usize, nodes: usize) -> io::Result<Vec<Region>> {
    v.block_max(0xC4, REGION_SIZE, MAX_REGIONS)?
        .iter()
        .map(|r| {
            let permutations = r
                .block_max(0x40, PERMUTATION_SIZE, MAX_PERMUTATIONS)?
                .iter()
                .map(|p| {
                    let mut geometry_indices = [None; DETAIL_LEVELS];
                    for (l, slot) in geometry_indices.iter_mut().enumerate() {
                        *slot = index_below(&p, 0x40 + 2 * l, geometries, "geometry")?;
                    }
                    let markers = p
                        .block_max(0x4C, PERMUTATION_MARKER_SIZE, MAX_MARKERS)?
                        .iter()
                        .map(|m| {
                            Ok(PermutationMarker {
                                name: m.tag_string(0x00)?.to_owned(),
                                node: index_below(&m, 0x20, nodes, "node")?,
                                rotation: m.quaternion(0x24)?,
                                translation: m.point3(0x34)?,
                            })
                        })
                        .collect::<io::Result<Vec<_>>>()?;
                    Ok(Permutation {
                        name: p.tag_string(0x00)?.to_owned(),
                        flags: p.flags32(0x20)?,
                        variant_number: p.i16(0x24)?,
                        geometry_indices,
                        markers,
                    })
                })
                .collect::<io::Result<Vec<_>>>()?;
            Ok(Region {
                name: r.tag_string(0x00)?.to_owned(),
                permutations,
            })
        })
        .collect()
}

/// What reading a part needs from the model.
struct PartContext<'a> {
    map: &'a MapFile,
    scale: [f32; 2],
    nodes: usize,
    shaders: usize,
}

fn read_geometry(ctx: &PartContext<'_>, g: &View<'_>) -> io::Result<Geometry> {
    let parts = g
        .block_max(0x24, PART_SIZE, MAX_PARTS)?
        .iter()
        .map(|p| read_part(ctx, &p))
        .collect::<io::Result<Vec<_>>>()?;
    Ok(Geometry {
        flags: g.flags32(0x00)?,
        parts,
    })
}

/// The data address a part's buffer record names: the record must be one
/// of the tag header's array (`array` is its address, `list` the records'
/// middle words).
fn buffer_address(p: &View<'_>, offset: usize, array: u32, list: &[u32], what: &str) -> io::Result<u32> {
    let record = p.u32(offset)?;
    let rel = record.checked_sub(array).ok_or_else(|| {
        p.error(
            offset,
            format_args!("{what} buffer record {record:#010x} is before the array at {array:#010x}"),
        )
    })? as usize;
    if !rel.is_multiple_of(BUFFER_RECORD_SIZE) {
        return Err(p.error(
            offset,
            format_args!("{what} buffer record {record:#010x} is not a multiple of 12 from the array at {array:#010x}"),
        ));
    }
    list.get(rel / BUFFER_RECORD_SIZE).copied().ok_or_else(|| {
        p.error(
            offset,
            format_args!(
                "{what} buffer record {} but the array holds {}",
                rel / BUFFER_RECORD_SIZE,
                list.len()
            ),
        )
    })
}

fn read_part(ctx: &PartContext<'_>, p: &View<'_>) -> io::Result<Part> {
    let map = ctx.map;
    let th = map.tag_header();
    let node = |offset: usize| -> io::Result<usize> {
        let n = p.i16(offset)?;
        if n >= 0 && (n as usize) < ctx.nodes {
            Ok(n as usize)
        } else {
            Err(p.error(
                offset,
                format_args!("centroid node {n} but there are {} nodes", ctx.nodes),
            ))
        }
    };
    let shader = match p.index16(0x04)? {
        Some(s) if s >= ctx.shaders => {
            return Err(p.error(0x04, format_args!("shader {s} but there are {}", ctx.shaders)));
        }
        s => s,
    };

    let index_type = p.i16(0x44)?;
    if index_type != INDEX_TYPE_STRIP {
        return Err(p.error(
            0x44,
            format_args!("index buffer type {index_type}: only strips (1) are read"),
        ));
    }
    let vertex_type = p.i16(0x54)?;
    if vertex_type != VERTEX_TYPE_COMPRESSED {
        return Err(p.error(
            0x54,
            format_args!("vertex buffer type {vertex_type}: only compressed vertices (5) are read"),
        ));
    }
    let vertex_offset = p.i32(0x5C)?;
    if vertex_offset != 0 {
        return Err(p.error(0x5C, format_args!("vertex offset {vertex_offset}: only 0 is read")));
    }

    // Vertices.
    let vcount = p.i32(0x58)?;
    if !(0..=MAX_VERTICES_PER_PART as i32).contains(&vcount) {
        return Err(p.error(0x58, format_args!("vertex count {vcount}")));
    }
    let vcount = vcount as usize;
    let vdata = buffer_address(p, 0x64, th.vertex_buffer_address, map.model_vertex_buffers(), "vertex")?;
    let vertices = if vcount == 0 {
        Vec::new()
    } else {
        let bytes = p.follow(vdata, vcount * VERTEX_SIZE)?;
        bytes
            .bytes()
            .as_chunks::<VERTEX_SIZE>()
            .0
            .iter()
            .enumerate()
            .map(|(i, raw)| decode_vertex(ctx, p, i, raw))
            .collect::<io::Result<Vec<_>>>()?
    };

    // Indices.
    let icount = p.i32(0x48)?;
    let idata = buffer_address(p, 0x50, th.index_buffer_address, map.model_index_buffers(), "index")?;
    let direct = p.u32(0x4C)?;
    if direct != idata {
        return Err(p.error(
            0x4C,
            format_args!("index data address {direct:#010x} differs from its record's {idata:#010x}"),
        ));
    }
    let strip: Vec<u16> = if icount <= 0 {
        Vec::new()
    } else {
        let n = (icount as usize)
            .checked_add(2)
            .filter(|&n| n <= (1 << 20))
            .ok_or_else(|| p.error(0x48, format_args!("index count {icount}")))?;
        let bytes = p.follow(idata, n * 2)?;
        bytes
            .bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .collect()
    };
    if let Some(&bad) = strip.iter().find(|&&k| k as usize >= vcount) {
        return Err(p.error(
            0x48,
            format_args!("strip index {bad} but the part has {vcount} vertices"),
        ));
    }
    let triangles = strip_triangles(&strip);

    Ok(Part {
        flags: p.flags32(0x00)?,
        shader,
        previous_filthy_part: p.index8(0x06)?,
        next_filthy_part: p.index8(0x07)?,
        centroid_primary_node: node(0x08)?,
        centroid_secondary_node: node(0x0A)?,
        centroid_primary_weight: p.f32(0x0C)?,
        centroid_secondary_weight: p.f32(0x10)?,
        centroid: p.point3(0x14)?,
        vertices,
        strip,
        triangles,
    })
}

fn decode_vertex(ctx: &PartContext<'_>, p: &View<'_>, i: usize, raw: &[u8; VERTEX_SIZE]) -> io::Result<Vertex> {
    let f = |at: usize| f32::from_le_bytes(raw[at..at + 4].try_into().expect("4 bytes"));
    let u = |at: usize| u32::from_le_bytes(raw[at..at + 4].try_into().expect("4 bytes"));
    let s = |at: usize| i16::from_le_bytes(raw[at..at + 2].try_into().expect("2 bytes"));
    let bad = |what: String| p.error(0x64, format_args!("vertex {i}: {what}"));
    let node = |byte: i8| -> Result<usize, String> {
        let b = i32::from(byte);
        if b < 0 || b % NODE_INDEX_SCALE != 0 || (b / NODE_INDEX_SCALE) as usize >= ctx.nodes {
            return Err(format!("node byte {byte:#04x} with {} nodes", ctx.nodes));
        }
        Ok((b / NODE_INDEX_SCALE) as usize)
    };
    let node0 = node(raw[0x1C] as i8).map_err(bad)?;
    let node1 = if raw[0x1D] as i8 == NODE_NONE_BYTE {
        None
    } else {
        Some(node(raw[0x1D] as i8).map_err(bad)?)
    };
    let weight0 = match node1 {
        None => 1.0,
        Some(_) => (f32::from(s(0x1E)) / WEIGHT_SCALE).clamp(0.0, 1.0),
    };
    Ok(Vertex {
        position: [f(0), f(4), f(8)],
        normal: unpack_vector(u(0x0C)),
        binormal: unpack_vector(u(0x10)),
        tangent: unpack_vector(u(0x14)),
        texcoord: [
            unpack_coordinate(s(0x18)) * ctx.scale[0],
            unpack_coordinate(s(0x1A)) * ctx.scale[1],
        ],
        node0,
        node1,
        weight0,
    })
}
