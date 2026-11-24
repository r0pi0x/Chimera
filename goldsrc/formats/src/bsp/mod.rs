// Adapted from qbsp (MIT, see THIRD_PARTY.md): src/data/mod.rs, src/data/nodes.rs, src/data/models.rs, src/data/texture.rs, src/data/lighting.rs
// Copyright (c) 2026 Noxmore. The record shapes and the lump directory
// follow qbsp's BSP30 paths; the Quake 1/2, BSP2 and BSPX paths, the
// macros and the embedded palette are left out, and the reading is
// rewritten on `chimera_vfs::bytes::Le`.
//! BSP version 30, GoldSrc's map file (specs/goldsrc/bsp.md): `Bsp::parse`
//! reads the header and every lump into plain records (planes, vertices,
//! edges, faces, texinfo, the render tree's nodes and leaves, the clip
//! nodes, brush models, the texture directory with its embedded mip
//! textures, the raw lighting and visibility bytes) and the entity text
//! into key/value blocks (`Entity`). `Bsp::lightmap_extent` sizes a face's
//! lightmap as the spec says (double precision rounded to single);
//! `vis` decodes the visibility rows; `hull` finds contents and traces the
//! four hulls; `solids` turns a hull's solid space into convex regions.
//!
//! Known shortcuts:
//! - Blue Shift's swapped entities/planes header is not read (out of scope
//!   until that game is ported; the spec's "Header and lumps").

pub mod hull;
pub mod solids;
pub mod vis;

use chimera_vfs::bytes::Le;

use crate::{Result, err, miptex::MipTex};

/// The only version this reads.
pub const VERSION: i32 = 30;
/// One lightmap sample per this many texture units.
pub const LUXEL_SIZE: f64 = 16.0;
/// An unused light style slot.
pub const NO_STYLE: u8 = 255;

/// The fifteen lumps, in the header's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lump {
    Entities,
    Planes,
    Textures,
    Vertices,
    Visibility,
    Nodes,
    TexInfo,
    Faces,
    Lighting,
    ClipNodes,
    Leaves,
    MarkSurfaces,
    Edges,
    SurfEdges,
    Models,
}

impl Lump {
    pub const ALL: [Lump; 15] = [
        Lump::Entities,
        Lump::Planes,
        Lump::Textures,
        Lump::Vertices,
        Lump::Visibility,
        Lump::Nodes,
        Lump::TexInfo,
        Lump::Faces,
        Lump::Lighting,
        Lump::ClipNodes,
        Lump::Leaves,
        Lump::MarkSurfaces,
        Lump::Edges,
        Lump::SurfEdges,
        Lump::Models,
    ];
}

/// Where a lump's bytes are.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct LumpEntry {
    pub offset: u32,
    pub len: u32,
}

/// A plane: points p with `normal · p = dist`; `kind` 0-2 axial (X, Y,
/// Z), 3-5 not (its largest component).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    pub normal: [f32; 3],
    pub dist: f32,
    pub kind: i32,
}

impl Plane {
    /// Signed distance of `p` from the plane; ≥ 0 is the front.
    pub fn side(&self, p: [f64; 3]) -> f64 {
        dot64(self.normal, p) - self.dist as f64
    }
}

pub(crate) fn dot64(n: [f32; 3], p: [f64; 3]) -> f64 {
    n[0] as f64 * p[0] + n[1] as f64 * p[1] + n[2] as f64 * p[2]
}

/// A child of a render tree node: another node or a leaf.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Child {
    Node(u32),
    Leaf(u32),
}

impl Child {
    /// A stored child: ≥ 0 a node, < 0 the leaf −(value + 1).
    pub fn from_i16(v: i16) -> Child {
        if v >= 0 {
            Child::Node(v as u32)
        } else {
            Child::Leaf((-(v as i32 + 1)) as u32)
        }
    }
}

/// An inner node of the render tree (hull 0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Node {
    pub plane: u32,
    /// Front, then back.
    pub children: [Child; 2],
    pub mins: [i16; 3],
    pub maxs: [i16; 3],
    pub first_face: u16,
    pub face_count: u16,
}

/// A leaf of the render tree.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Leaf {
    pub contents: i32,
    /// Offset into the visibility lump, −1 for none.
    pub vis_offset: i32,
    pub mins: [i16; 3],
    pub maxs: [i16; 3],
    pub first_mark_surface: u16,
    pub mark_surface_count: u16,
    pub ambient: [u8; 4],
}

/// A clip node of hulls 1-3: a child ≥ 0 is a clip node, < 0 a contents
/// value itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClipNode {
    pub plane: i32,
    pub children: [i16; 2],
}

/// A polygon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Face {
    pub plane: u16,
    /// Nonzero: the face looks along the plane's negative normal.
    pub side: u16,
    pub first_edge: i32,
    pub edge_count: u16,
    pub texinfo: u16,
    pub styles: [u8; 4],
    /// Byte offset into the lighting lump, −1 for none.
    pub light_offset: i32,
}

/// A texture projection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TexInfo {
    /// s axis and offset.
    pub s: [f32; 4],
    /// t axis and offset.
    pub t: [f32; 4],
    pub texture: i32,
    /// 1: special (sky and tool faces: no lightmap).
    pub flags: i32,
}

/// A brush model: the world (0) or a brush entity's `*n`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Model {
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
    pub origin: [f32; 3],
    /// Hull 0's root node, hulls 1-3's root clip nodes.
    pub heads: [i32; 4],
    pub vis_leaves: i32,
    pub first_face: i32,
    pub face_count: i32,
}

/// One block of the entity text: its key/value pairs as written (keys
/// may repeat; `get` gives the last, ignoring case).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entity {
    pub pairs: Vec<(String, String)>,
}

impl Entity {
    /// The value of `key` (case-insensitive; the last occurrence wins).
    pub fn get(&self, key: &str) -> Option<&str> {
        self.pairs
            .iter()
            .rev()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    pub fn classname(&self) -> &str {
        self.get("classname").unwrap_or("")
    }
}

/// A face's lightmap block: its first sample's texel and its size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightmapExtent {
    /// floor(s_min / 16), floor(t_min / 16).
    pub min: [i32; 2],
    /// Samples along s and t.
    pub size: [u32; 2],
    /// The texel extents as computed (single precision).
    pub s_range: [f32; 2],
    pub t_range: [f32; 2],
}

/// A parsed BSP 30 file.
#[derive(Clone, Debug, Default)]
pub struct Bsp {
    pub lumps: [LumpEntry; 15],
    pub entities_text: String,
    pub planes: Vec<Plane>,
    /// The texture directory: `None` for a −1 (missing) entry.
    pub textures: Vec<Option<MipTex>>,
    pub vertices: Vec<[f32; 3]>,
    pub visibility: Vec<u8>,
    pub nodes: Vec<Node>,
    pub texinfo: Vec<TexInfo>,
    pub faces: Vec<Face>,
    pub lighting: Vec<u8>,
    pub clip_nodes: Vec<ClipNode>,
    pub leaves: Vec<Leaf>,
    pub mark_surfaces: Vec<u16>,
    pub edges: Vec<[u16; 2]>,
    pub surf_edges: Vec<i32>,
    pub models: Vec<Model>,
}

fn records<T>(lump: &[u8], size: usize, name: &str, read: impl Fn(&mut Le) -> Result<T>) -> Result<Vec<T>> {
    if !lump.len().is_multiple_of(size) {
        return err(format!(
            "{name} lump: {} bytes is not a whole number of {size}-byte records",
            lump.len()
        ));
    }
    let mut r = Le::new(lump);
    (0..lump.len() / size).map(|_| read(&mut r)).collect()
}

fn i16x3(r: &mut Le) -> Result<[i16; 3]> {
    Ok([r.i16()?, r.i16()?, r.i16()?])
}

impl Bsp {
    /// Read a whole file.
    pub fn parse(bytes: &[u8]) -> Result<Bsp> {
        let mut r = Le::new(bytes);
        let version = r.i32()?;
        if version != VERSION {
            return err(format!("BSP version {version}, not {VERSION}"));
        }
        let mut lumps = [LumpEntry::default(); 15];
        for (i, l) in lumps.iter_mut().enumerate() {
            let (offset, len) = (r.i32()?, r.i32()?);
            if offset < 0 || len < 0 || offset as usize + len as usize > bytes.len() {
                return err(format!("lump {:?} ({offset}, {len}) is outside the file", Lump::ALL[i]));
            }
            *l = LumpEntry {
                offset: offset as u32,
                len: len as u32,
            };
        }
        let lump = |l: Lump| {
            let e = lumps[l as usize];
            &bytes[e.offset as usize..(e.offset + e.len) as usize]
        };
        let ent = lump(Lump::Entities);
        let ent_end = ent.iter().position(|&b| b == 0).unwrap_or(ent.len());
        Ok(Bsp {
            lumps,
            entities_text: ent[..ent_end].iter().map(|&b| b as char).collect(),
            planes: records(lump(Lump::Planes), 20, "planes", |r| {
                Ok(Plane {
                    normal: r.f32s::<3>()?,
                    dist: r.f32()?,
                    kind: r.i32()?,
                })
            })?,
            textures: read_textures(lump(Lump::Textures))?,
            vertices: records(lump(Lump::Vertices), 12, "vertices", |r| Ok(r.f32s::<3>()?))?,
            visibility: lump(Lump::Visibility).to_vec(),
            nodes: records(lump(Lump::Nodes), 24, "nodes", |r| {
                Ok(Node {
                    plane: r.u32()?,
                    children: [Child::from_i16(r.i16()?), Child::from_i16(r.i16()?)],
                    mins: i16x3(r)?,
                    maxs: i16x3(r)?,
                    first_face: r.u16()?,
                    face_count: r.u16()?,
                })
            })?,
            texinfo: records(lump(Lump::TexInfo), 40, "texinfo", |r| {
                Ok(TexInfo {
                    s: r.f32s::<4>()?,
                    t: r.f32s::<4>()?,
                    texture: r.i32()?,
                    flags: r.i32()?,
                })
            })?,
            faces: records(lump(Lump::Faces), 20, "faces", |r| {
                Ok(Face {
                    plane: r.u16()?,
                    side: r.u16()?,
                    first_edge: r.i32()?,
                    edge_count: r.u16()?,
                    texinfo: r.u16()?,
                    styles: r.array::<4>()?,
                    light_offset: r.i32()?,
                })
            })?,
            lighting: lump(Lump::Lighting).to_vec(),
            clip_nodes: records(lump(Lump::ClipNodes), 8, "clip nodes", |r| {
                Ok(ClipNode {
                    plane: r.i32()?,
                    children: [r.i16()?, r.i16()?],
                })
            })?,
            leaves: records(lump(Lump::Leaves), 28, "leaves", |r| {
                Ok(Leaf {
                    contents: r.i32()?,
                    vis_offset: r.i32()?,
                    mins: i16x3(r)?,
                    maxs: i16x3(r)?,
                    first_mark_surface: r.u16()?,
                    mark_surface_count: r.u16()?,
                    ambient: r.array::<4>()?,
                })
            })?,
            mark_surfaces: records(lump(Lump::MarkSurfaces), 2, "marksurfaces", |r| Ok(r.u16()?))?,
            edges: records(lump(Lump::Edges), 4, "edges", |r| Ok([r.u16()?, r.u16()?]))?,
            surf_edges: records(lump(Lump::SurfEdges), 4, "surfedges", |r| Ok(r.i32()?))?,
            models: records(lump(Lump::Models), 64, "models", |r| {
                Ok(Model {
                    mins: r.f32s::<3>()?,
                    maxs: r.f32s::<3>()?,
                    origin: r.f32s::<3>()?,
                    heads: [r.i32()?, r.i32()?, r.i32()?, r.i32()?],
                    vis_leaves: r.i32()?,
                    first_face: r.i32()?,
                    face_count: r.i32()?,
                })
            })?,
        })
    }

    /// The entity blocks, `worldspawn` first.
    pub fn entities(&self) -> Vec<Entity> {
        parse_entities(&self.entities_text)
    }

    /// A face's corners, in order (each surfedge's first vertex).
    pub fn face_vertices(&self, face: &Face) -> Vec<[f32; 3]> {
        (0..face.edge_count as i32)
            .filter_map(|i| {
                let se = *self.surf_edges.get((face.first_edge + i) as usize)?;
                let e = self.edges.get(se.unsigned_abs() as usize)?;
                let v = if se >= 0 { e[0] } else { e[1] };
                self.vertices.get(v as usize).copied()
            })
            .collect()
    }

    /// The lightmap block of a face with a lightmap (texinfo flag 1 off),
    /// sized as the spec says: texel coordinates in double precision,
    /// rounded to single, then the 16-unit grid.
    pub fn lightmap_extent(&self, face: &Face) -> Option<LightmapExtent> {
        let ti = self.texinfo.get(face.texinfo as usize)?;
        let (mut s_range, mut t_range) = ([f32::MAX, f32::MIN], [f32::MAX, f32::MIN]);
        for v in self.face_vertices(face) {
            let v = [v[0] as f64, v[1] as f64, v[2] as f64];
            let st = |a: [f32; 4]| (a[0] as f64 * v[0] + a[1] as f64 * v[1] + a[2] as f64 * v[2] + a[3] as f64) as f32;
            let (s, t) = (st(ti.s), st(ti.t));
            s_range = [s_range[0].min(s), s_range[1].max(s)];
            t_range = [t_range[0].min(t), t_range[1].max(t)];
        }
        let lo = |x: f32| (x as f64 / LUXEL_SIZE).floor() as i32;
        let hi = |x: f32| (x as f64 / LUXEL_SIZE).ceil() as i32;
        let min = [lo(s_range[0]), lo(t_range[0])];
        let size = [
            (hi(s_range[1]) - min[0] + 1) as u32,
            (hi(t_range[1]) - min[1] + 1) as u32,
        ];
        Some(LightmapExtent {
            min,
            size,
            s_range,
            t_range,
        })
    }

    /// How many styles a face has (slots before the first 255).
    pub fn style_count(face: &Face) -> usize {
        face.styles.iter().take_while(|&&s| s != NO_STYLE).count()
    }

    /// A face's lightmap bytes: `style_count` blocks of w × h RGB.
    pub fn face_lightmap(&self, face: &Face) -> Option<&[u8]> {
        if face.light_offset < 0 {
            return None;
        }
        let ext = self.lightmap_extent(face)?;
        let len = (ext.size[0] * ext.size[1]) as usize * 3 * Self::style_count(face);
        self.lighting
            .get(face.light_offset as usize..face.light_offset as usize + len)
    }
}

/// The texture lump: a count, a directory of offsets (−1 missing), and
/// each mip texture record.
fn read_textures(lump: &[u8]) -> Result<Vec<Option<MipTex>>> {
    if lump.is_empty() {
        return Ok(Vec::new());
    }
    let mut r = Le::new(lump);
    let count = r.i32()?;
    if count < 0 {
        return err(format!("texture lump: count {count}"));
    }
    let offsets: Vec<i32> = (0..count).map(|_| r.i32()).collect::<std::result::Result<_, _>>()?;
    offsets
        .into_iter()
        .map(|o| {
            if o < 0 {
                return Ok(None);
            }
            let rec = lump
                .get(o as usize..)
                .ok_or_else(|| crate::Error(format!("texture at {o}: outside the lump")))?;
            MipTex::parse(rec).map(Some)
        })
        .collect()
}

/// The entity text as blocks of `"key" "value"` lines (no escapes).
pub fn parse_entities(text: &str) -> Vec<Entity> {
    let mut out = Vec::new();
    let mut cur: Option<Entity> = None;
    let mut quoted = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if cur.is_none() => cur = Some(Entity::default()),
            '}' if cur.is_some() => {
                out.extend(cur.take());
                quoted.clear();
            }
            '"' => {
                let mut s = String::new();
                for c in chars.by_ref() {
                    if c == '"' {
                        break;
                    }
                    s.push(c);
                }
                quoted.push(s);
                if quoted.len() == 2 {
                    let v = quoted.pop().unwrap();
                    let k = quoted.pop().unwrap();
                    if let Some(e) = cur.as_mut() {
                        e.pairs.push((k, v));
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// The file name of each `wad` entry of a `worldspawn` (`;`-separated
/// paths from the mapper's machine; only the last component counts).
pub fn wad_names(worldspawn: &Entity) -> Vec<String> {
    worldspawn
        .get("wad")
        .unwrap_or("")
        .split(';')
        .map(|p| p.rsplit(['\\', '/']).next().unwrap_or("").trim().to_string())
        .filter(|n| !n.is_empty())
        .collect()
}

/// A texture's name as the map records it (`None` for a missing entry).
pub fn texture_name<'a>(bsp: &'a Bsp, texinfo: &TexInfo) -> Option<&'a str> {
    bsp.textures
        .get(texinfo.texture as usize)?
        .as_ref()
        .map(|t| t.name.as_str())
}
