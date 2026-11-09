//! LithTech Jupiter model files (`.ltb`, "D3D model") as Combat Arms
//! ships them, model versions 23 and 25 (same layout), per
//! specs/combat_arms/models_ltb.md (section names below are the spec's)
//! and, for the animation section, specs/combat_arms/animation.md ("Byte
//! layout"): the 20-byte file header, the counts block, the model
//! properties, pieces with their levels of detail and mesh bodies, the
//! node tree, weight sets, child-model names, the animations, sockets and
//! the animation bindings, read in one pass.
//!
//! `Model::parse` reads a whole file, bounds-checked: every count is
//! checked against the bytes left before anything is allocated, every
//! node index against the node count, every triangle index against its
//! mesh's vertices, bone sets must tile their mesh, and each LOD's body
//! must take exactly its stated size. Anything else is an error
//! (`LtbError`, with the byte offset). Values stay in the file's frame
//! (left-handed, +Y up, world units); `to_rh_*` is "Converting to a
//! right-handed Y-up frame". Playback, sampling and skinning maths are
//! `crate::anim`.
//!
//! Known shortcuts (our reading where the spec leaves room):
//! - Vertex streams: only the flag word the install uses, 0x13
//!   (position, normal, one UV set), in any stream; other non-zero words
//!   are refused ("Vertex formats": "a reader should reject flag words
//!   other than 0 and 0x13"). A LOD with more than one non-zero stream is
//!   read stream after stream, but only the first stream's data is kept.
//! - Strict where the spec says "never occur": trailing bytes after the
//!   bindings, channel counts other than 0, 1 and N (animation.md "Filling
//!   missing keys"), a child model's binding list naming more entries
//!   than it has, and stored weight counts above 3 are errors.
//! - Strings are 8-bit: each byte becomes the char of the same code
//!   (Latin-1); the engine's truncations (4095, 255) are not applied.
//! - The counts block is kept as read and not checked (the spec says a
//!   reader should trust the sections); `Model::track_bytes` and
//!   `vertex_anim_bytes` compute what its last two fields should be.

use std::fmt;

use chimera_vfs::bytes::{Le, Truncated};

/// "Constants": the only model file type.
pub const FILE_TYPE_MODEL: u8 = 1;
/// "Constants": the container version every non-empty LOD needs.
pub const CONTAINER_VERSION: u16 = 9;
/// "Constants": lower model versions are refused.
pub const MIN_VERSION: u32 = 23;
/// "Constants".
pub const MAX_PIECES: u32 = 64;
/// "Constants": child models, counting the model itself.
pub const MAX_CHILD_MODELS: u32 = 32;
/// "Constants": unused bone-set slot.
pub const NO_BONE: u8 = 0xFF;
/// "Vertex formats": the one flag word the install uses.
pub const STREAM_POS_NORMAL_UV: u32 = 0x13;
/// "Skeleton": node flag, use only the rotation of animation data.
pub const NODE_ROTATION_ONLY: u8 = 0x2;
/// "Constants": the animations whose boxes are the stand and crouch hulls.
pub const STAND_ANIMATION: &str = "LST";
pub const CROUCH_ANIMATION: &str = "LC";
/// "Constants": the game's hull half-extents without those animations.
pub const FALLBACK_STAND: Vec3 = [15.0, 50.0, 15.0];
pub const FALLBACK_CROUCH: Vec3 = [15.0, 40.0, 15.0];

/// Three f32 as stored: x, y, z (+Y up, +Z forward, left-handed).
pub type Vec3 = [f32; 3];
/// Four f32 as stored: x, y, z, w (w the scalar part).
pub type Quat = [f32; 4];
/// Row-major 4 x 4 for column vectors (`m[r][c]`, translation in column 3).
pub type Mat4 = [[f32; 4]; 4];

/// Why a model file was rejected: what, and at which byte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LtbError {
    pub offset: usize,
    pub message: String,
}

impl fmt::Display for LtbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "model file, byte {}: {}", self.offset, self.message)
    }
}

impl std::error::Error for LtbError {}

type Result<T> = std::result::Result<T, LtbError>;

/// "Model version and counts block": the 15 u32 after the version, as
/// read (hints and allocation sizes; nothing is located by them).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub keyframes: u32,
    pub animations: u32,
    pub nodes: u32,
    pub pieces: u32,
    pub child_models: u32,
    pub triangles: u32,
    pub vertices_hint: u32,
    pub weights_hint: u32,
    pub lods: u32,
    pub sockets: u32,
    pub weight_sets: u32,
    pub strings_hint: u32,
    pub string_bytes_hint: u32,
    pub vertex_anim_bytes: u32,
    pub track_bytes: u32,
}

/// A piece: one named part with its levels of detail.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub name: String,
    /// One per LOD, increasing; the first is 0.
    pub lod_distances: Vec<f32>,
    /// "Old minimum/maximum LOD offset" (ignored by the engine).
    pub old_lod_offsets: (u32, u32),
    pub lods: Vec<Lod>,
}

/// "LOD record".
#[derive(Clone, Debug, PartialEq)]
pub struct Lod {
    /// Texture slots used, T.
    pub texture_count: u32,
    /// Indices into the object's skin list (slots beyond T are filler).
    pub textures: [i32; 4],
    pub render_style: i32,
    pub render_priority: u8,
    /// The body's stated size S.
    pub body_size: u32,
    pub mesh: Mesh,
    /// Node indices this LOD needs transforms for.
    pub used_nodes: Vec<u8>,
}

impl Lod {
    /// The texture indices in use (slots 0 .. T-1).
    pub fn used_textures(&self) -> &[i32] {
        &self.textures[..(self.texture_count as usize).min(4)]
    }
}

/// Vertices and triangles of one mesh body, as stored ("Mesh bodies",
/// "Vertex formats").
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Geometry {
    pub stream_flags: [u32; 4],
    pub max_bones_per_triangle: u32,
    pub max_bones_per_vertex: u32,
    pub positions: Vec<Vec3>,
    /// Stored blend weights per vertex (k of them; the rest 0). Empty for
    /// rigid and vertex-animated meshes.
    pub weights: Vec<[f32; 4]>,
    /// Palette meshes: four palette indices per vertex. Empty otherwise.
    pub palette_indices: Vec<[u8; 4]>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<[f32; 2]>,
    /// Absolute vertex indices, a plain list.
    pub triangles: Vec<[u16; 3]>,
    /// k, the stored weights per vertex.
    pub weight_count: usize,
    /// Bytes per vertex.
    pub stride: usize,
}

/// "Bone sets": vertices [first, first + count) and the indices up to
/// `index_end` (counted in indices, 3 per triangle) follow `nodes`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoneSet {
    pub first_vertex: u16,
    pub vertex_count: u16,
    /// Node indices, used slots increasing, unused `NO_BONE`.
    pub nodes: [u8; 4],
    pub index_end: u32,
}

impl BoneSet {
    /// The used node indices, in order.
    pub fn used(&self) -> impl Iterator<Item = u8> + '_ {
        self.nodes.iter().copied().filter(|&n| n != NO_BONE)
    }
}

/// A matrix palette skinned mesh's extra fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    pub min_index: u32,
    pub max_index: u32,
    /// Re-indexed meshes: palette entry i drives node `remap[i]`; empty
    /// otherwise (entry i drives node i).
    pub remap: Vec<u32>,
}

/// One LOD's mesh, by kind ("Mesh bodies").
#[derive(Clone, Debug, PartialEq)]
pub enum Mesh {
    /// Kind 4: every vertex follows `node`.
    Rigid { node: u32, geometry: Geometry },
    /// Kind 5: bone sets, or a matrix palette when `palette` is set.
    Skinned {
        reindexed: bool,
        geometry: Geometry,
        bone_sets: Vec<BoneSet>,
        palette: Option<Palette>,
    },
    /// Kind 6: positions 0 .. `animated_vertices` from `anim_node`'s
    /// vertex track, then the duplicate (source, destination) copies;
    /// drawn with `draw_node`.
    VertexAnimated {
        animated_vertices: u32,
        anim_node: u32,
        draw_node: u32,
        geometry: Geometry,
        duplicates: Vec<(u16, u16)>,
    },
    /// Kind 7: no geometry.
    Empty,
}

impl Mesh {
    pub fn kind(&self) -> u32 {
        match self {
            Mesh::Rigid { .. } => 4,
            Mesh::Skinned { .. } => 5,
            Mesh::VertexAnimated { .. } => 6,
            Mesh::Empty => 7,
        }
    }

    pub fn geometry(&self) -> Option<&Geometry> {
        match self {
            Mesh::Rigid { geometry, .. } | Mesh::Skinned { geometry, .. } | Mesh::VertexAnimated { geometry, .. } => {
                Some(geometry)
            }
            Mesh::Empty => None,
        }
    }

    /// Each vertex's nodes and weights (up to four, weights summing to 1),
    /// per "Skinning": rigid meshes follow their node, bone-set vertices
    /// their set's used bones with the last weight implied, palette
    /// vertices their palette entries' nodes, vertex-animated ones the
    /// draw node.
    pub fn vertex_influences(&self) -> Vec<[(u32, f32); 4]> {
        let one = |n: u32| [(n, 1.0), (0, 0.0), (0, 0.0), (0, 0.0)];
        match self {
            Mesh::Rigid { node, geometry } => vec![one(*node); geometry.positions.len()],
            Mesh::VertexAnimated { draw_node, geometry, .. } => vec![one(*draw_node); geometry.positions.len()],
            Mesh::Empty => Vec::new(),
            Mesh::Skinned {
                geometry,
                bone_sets,
                palette: None,
                ..
            } => {
                let mut out = vec![one(0); geometry.positions.len()];
                for set in bone_sets {
                    let bones: Vec<u32> = set.used().map(u32::from).collect();
                    let first = set.first_vertex as usize;
                    for v in first..first + set.vertex_count as usize {
                        // No stored weights when every set has one bone.
                        out[v] = blend(&bones, geometry.weights.get(v).unwrap_or(&[0.0; 4]));
                    }
                }
                out
            }
            Mesh::Skinned {
                geometry,
                palette: Some(p),
                ..
            } => (0..geometry.positions.len())
                .map(|v| {
                    // k = Bv - 1 stored weights: nodes of indices i_0 .. i_k.
                    let n = geometry.weight_count + 1;
                    let nodes: Vec<u32> = geometry.palette_indices[v][..n.min(4)]
                        .iter()
                        .map(|&i| p.remap.get(i as usize).copied().unwrap_or(i as u32))
                        .collect();
                    blend(&nodes, &geometry.weights[v])
                })
                .collect(),
        }
    }
}

/// Up to four (node, weight) pairs: the first m - 1 stored weights, the
/// last implied.
fn blend(nodes: &[u32], stored: &[f32; 4]) -> [(u32, f32); 4] {
    let mut out = [(0, 0.0); 4];
    let m = nodes.len().clamp(1, 4);
    let mut sum = 0.0;
    for j in 0..m - 1 {
        out[j] = (nodes[j], stored[j]);
        sum += stored[j];
    }
    out[m - 1] = (nodes.get(m - 1).copied().unwrap_or(0), 1.0 - sum);
    out
}

/// "Skeleton (node tree)": one node, in depth-first order.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub name: String,
    /// The index field as stored (the engine renumbers by position).
    pub stored_index: u16,
    pub flags: u8,
    /// Model-space bind transform.
    pub bind: Mat4,
    pub parent: Option<usize>,
}

/// "Weight sets".
#[derive(Clone, Debug, PartialEq)]
pub struct WeightSet {
    pub name: String,
    /// One per node.
    pub weights: Vec<f32>,
}

/// One key: its time (ms) and string (empty for most keys).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Key {
    pub time: u32,
    pub string: String,
}

/// One node's animation data (animation.md "Node channel record").
#[derive(Clone, Debug, PartialEq)]
pub enum Channel {
    /// Stored positions and rotations: 0 (identity), 1 (constant) or one
    /// per key.
    Transform { positions: Vec<Vec3>, rotations: Vec<Quat> },
    /// Compression 0 with the vertex flag: per key, the vertex positions.
    Vertices(Vec<Vec<Vec3>>),
}

/// One animation record (animation.md "One animation record").
#[derive(Clone, Debug, PartialEq)]
pub struct Animation {
    /// Default box (half-extents), overridden by the binding.
    pub dims: Vec3,
    pub name: String,
    /// 0 to 3.
    pub compression: u32,
    /// Interpolation time into it, ms.
    pub interp_ms: u32,
    pub keys: Vec<Key>,
    /// One per node, in node order.
    pub channels: Vec<Channel>,
}

impl Animation {
    /// L: the last key's time (0 without keys).
    pub fn length(&self) -> u32 {
        self.keys.last().map_or(0, |k| k.time)
    }

    /// Node `j`'s value at key `i` ("Filling missing keys"); vertex nodes
    /// and absent channels give identity.
    pub fn key_value(&self, node: usize, i: usize) -> (Vec3, Quat) {
        match self.channels.get(node) {
            Some(Channel::Transform { positions, rotations }) => (
                pick(positions, i).unwrap_or([0.0; 3]),
                pick(rotations, i).unwrap_or([0.0, 0.0, 0.0, 1.0]),
            ),
            _ => ([0.0; 3], [0.0, 0.0, 0.0, 1.0]),
        }
    }

    /// Bytes of node channel payloads (the counts block's last field's
    /// share of this animation).
    pub fn track_bytes(&self) -> usize {
        let (sp, sq) = match self.compression {
            0 | 1 => (12, 16),
            2 => (6, 8),
            _ => (12, 8),
        };
        self.channels
            .iter()
            .map(|c| match c {
                Channel::Transform { .. } if self.compression == 0 => 28 * self.keys.len(),
                Channel::Transform { positions, rotations } => positions.len() * sp + rotations.len() * sq,
                Channel::Vertices(_) => 0,
            })
            .sum()
    }

    /// Bytes of vertex-animation positions.
    pub fn vertex_anim_bytes(&self) -> usize {
        self.channels
            .iter()
            .map(|c| match c {
                Channel::Vertices(frames) => frames.iter().map(|f| 12 * f.len()).sum(),
                _ => 0,
            })
            .sum()
    }
}

fn pick<T: Copy>(values: &[T], i: usize) -> Option<T> {
    match values.len() {
        0 => None,
        1 => Some(values[0]),
        _ => values.get(i).copied(),
    }
}

/// "Sockets".
#[derive(Clone, Debug, PartialEq)]
pub struct Socket {
    pub node: u32,
    pub name: String,
    /// Relative to the node; may be non-unit (stands for its normalised
    /// form).
    pub rotation: Quat,
    /// In the node's space.
    pub position: Vec3,
    pub scale: Vec3,
}

/// "Animation bindings": one entry.
#[derive(Clone, Debug, PartialEq)]
pub struct Binding {
    pub name: String,
    pub dims: Vec3,
    pub translation: Vec3,
}

/// A parsed model file.
#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub version: u32,
    pub counts: Counts,
    pub command: String,
    pub radius: f32,
    pub pieces: Vec<Piece>,
    /// Depth-first pre-order; parents before children.
    pub nodes: Vec<Node>,
    pub weight_sets: Vec<WeightSet>,
    /// File names of children 1 .. K-1 (child 0 is the model itself).
    pub children: Vec<String>,
    /// Shared with the animation lists built from this model (`anim::AnimList`).
    pub animations: Vec<std::sync::Arc<Animation>>,
    pub sockets: Vec<Socket>,
    /// Per child slot (0 = the model itself), its binding list.
    pub bindings: Vec<Vec<Binding>>,
}

impl Model {
    /// Read a whole model file ("Load order").
    pub fn parse(bytes: &[u8]) -> Result<Model> {
        let mut c = Le::new(bytes);
        let head = c.take(20).map_err(|e| LtbError::from(e).context("file header"))?;
        if head[0] != FILE_TYPE_MODEL {
            return Err(LtbError {
                offset: 0,
                message: format!("file type {}, not a model", head[0]),
            });
        }
        let container = u16::from_le_bytes([head[2], head[3]]);
        let version = c.u32()?;
        if version < MIN_VERSION {
            return Err(LtbError {
                offset: 20,
                message: format!("model version {version}, below {MIN_VERSION}"),
            });
        }
        let mut f = [0u32; 15];
        for v in &mut f {
            *v = c.u32()?;
        }
        let counts = Counts {
            keyframes: f[0],
            animations: f[1],
            nodes: f[2],
            pieces: f[3],
            child_models: f[4],
            triangles: f[5],
            vertices_hint: f[6],
            weights_hint: f[7],
            lods: f[8],
            sockets: f[9],
            weight_sets: f[10],
            strings_hint: f[11],
            string_bytes_hint: f[12],
            vertex_anim_bytes: f[13],
            track_bytes: f[14],
        };

        // "Model properties".
        let command = c.string()?;
        let radius = c.f32()?;
        let boxes = c.count(68, "oriented boxes")?;
        c.take(boxes * 68)?;
        let at = c.pos;
        let piece_count = c.u32()?;
        if piece_count > MAX_PIECES {
            return Err(LtbError {
                offset: at,
                message: format!("{piece_count} pieces, more than {MAX_PIECES}"),
            });
        }
        let mut pieces = Vec::with_capacity(piece_count as usize);
        for i in 0..piece_count {
            pieces.push(parse_piece(&mut c, container).map_err(|e| LtbError::from(e).context(&format!("piece {i}")))?);
        }

        // "Skeleton (node tree)".
        let mut nodes = Vec::new();
        parse_node(&mut c, None, &mut nodes, 0)?;
        let n = nodes.len();

        // "Weight sets".
        let sets = c.count(6, "weight sets")?;
        let mut weight_sets = Vec::with_capacity(sets);
        for _ in 0..sets {
            let name = c.string()?;
            let at = c.pos;
            let count = c.count(4, "weights")?;
            if count != n {
                return Err(LtbError {
                    offset: at,
                    message: format!("weight set {name} has {count} weights for {n} nodes"),
                });
            }
            let weights = (0..count).map(|_| Ok(c.f32()?)).collect::<Result<_>>()?;
            weight_sets.push(WeightSet { name, weights });
        }

        // "Child models".
        let at = c.pos;
        let child_count = c.u32()?;
        if !(1..=MAX_CHILD_MODELS).contains(&child_count) {
            return Err(LtbError {
                offset: at,
                message: format!("{child_count} child models (1..{MAX_CHILD_MODELS})"),
            });
        }
        let children = (1..child_count).map(|_| c.string()).collect::<Result<Vec<_>>>()?;

        // "Animation section".
        let anim_count = c.count(26, "animations")?;
        let mut animations = Vec::with_capacity(anim_count);
        for i in 0..anim_count {
            animations.push(parse_animation(&mut c, n).map(std::sync::Arc::new).map_err(|e| LtbError::from(e).context(&format!("animation {i}")))?);
        }

        // "Sockets".
        let socket_count = c.count(46, "sockets")?;
        let mut sockets = Vec::with_capacity(socket_count);
        for _ in 0..socket_count {
            let at = c.pos;
            let node = c.u32()?;
            if node as usize >= n {
                return Err(LtbError {
                    offset: at,
                    message: format!("socket on node {node} of {n}"),
                });
            }
            sockets.push(Socket {
                node,
                name: c.string()?,
                rotation: c.quat()?,
                position: c.vec3()?,
                scale: c.vec3()?,
            });
        }

        // "Animation bindings".
        let mut bindings = Vec::with_capacity(child_count as usize);
        for _ in 0..child_count {
            let count = c.count(26, "bindings")?;
            let list = (0..count)
                .map(|_| {
                    Ok(Binding {
                        name: c.string()?,
                        dims: c.vec3()?,
                        translation: c.vec3()?,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            bindings.push(list);
        }
        if c.pos != bytes.len() {
            return Err(c.err(format!("{} trailing bytes after the bindings", bytes.len() - c.pos)));
        }
        Ok(Model {
            version,
            counts,
            command,
            radius,
            pieces,
            nodes,
            weight_sets,
            children,
            animations,
            sockets,
            bindings,
        })
    }

    /// Node index by name (case ignored).
    pub fn node(&self, name: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.name.eq_ignore_ascii_case(name))
    }

    /// Own animation index by name (case ignored; the first match).
    pub fn animation(&self, name: &str) -> Option<usize> {
        self.animations.iter().position(|a| a.name.eq_ignore_ascii_case(name))
    }

    /// Each node's parent, in node order.
    pub fn parents(&self) -> Vec<Option<usize>> {
        self.nodes.iter().map(|n| n.parent).collect()
    }

    /// "Bounds and dimensions": own animation `i`'s box, from its slot-0
    /// binding when there is one (matched by position), else its own.
    pub fn animation_dims(&self, i: usize) -> Vec3 {
        self.bindings
            .first()
            .and_then(|b| b.get(i))
            .map_or(self.animations[i].dims, |b| b.dims)
    }

    /// Own animation `i`'s root translation from its slot-0 binding
    /// ((0, 0, 0) without one).
    pub fn animation_translation(&self, i: usize) -> Vec3 {
        self.bindings
            .first()
            .and_then(|b| b.get(i))
            .map_or([0.0; 3], |b| b.translation)
    }

    /// "Model properties": casts shadows when the command string is the
    /// single token `ShadowEnable` (case ignored).
    pub fn shadow_enabled(&self) -> bool {
        let mut words = self.command.split_whitespace();
        words.next().is_some_and(|w| w.eq_ignore_ascii_case("ShadowEnable")) && words.next().is_none()
    }

    /// The counts block's track-byte field, from the data.
    pub fn track_bytes(&self) -> usize {
        self.animations.iter().map(|a| a.track_bytes()).sum()
    }

    /// The counts block's vertex-animation byte field, from the data.
    pub fn vertex_anim_bytes(&self) -> usize {
        self.animations.iter().map(|a| a.vertex_anim_bytes()).sum()
    }

    /// Socket `s`'s transform in model space at bind pose ("Socket
    /// placement"): node bind x socket (rotation normalised).
    pub fn socket_bind(&self, s: usize) -> Mat4 {
        let sock = &self.sockets[s];
        let local = crate::anim::matrix(sock.position, sock.rotation);
        crate::anim::mul(&self.nodes[sock.node as usize].bind, &local)
    }

    /// Bounds of every LOD 0 vertex (min, max), as stored.
    pub fn vertex_bounds(&self) -> Option<(Vec3, Vec3)> {
        let mut it = self
            .pieces
            .iter()
            .filter_map(|p| p.lods.first()?.mesh.geometry())
            .flat_map(|g| g.positions.iter().copied());
        let first = it.next()?;
        Some(it.fold((first, first), |(lo, hi), p| {
            (
                [lo[0].min(p[0]), lo[1].min(p[1]), lo[2].min(p[2])],
                [hi[0].max(p[0]), hi[1].max(p[1]), hi[2].max(p[2])],
            )
        }))
    }
}

/// "Level-of-detail choice": the LOD for distance `d` (u) with the
/// `ModelLODOffset` bias.
pub fn lod_index(distances: &[f32], d: f32, bias: i32) -> usize {
    let l = distances.len();
    if l == 0 {
        return 0;
    }
    let base = if d <= 0.0 {
        0
    } else {
        (0..l.saturating_sub(1))
            .find(|&i| d < distances[i + 1])
            .unwrap_or(l - 1)
    };
    (base as i64 + bias as i64).clamp(0, l as i64 - 1) as usize
}

/// "Converting to a right-handed Y-up frame": positions, normals,
/// translations: (x, y, z) -> (x, y, -z).
pub fn to_rh_vec(v: Vec3) -> Vec3 {
    [v[0], v[1], -v[2]]
}

/// Quaternions: (x, y, z, w) -> (-x, -y, z, w).
pub fn to_rh_quat(q: Quat) -> Quat {
    [-q[0], -q[1], q[2], q[3]]
}

/// Matrices: S M S (negate m02, m12, m20, m21 and m23).
pub fn to_rh_mat(m: &Mat4) -> Mat4 {
    let mut o = *m;
    for (r, c) in [(0, 2), (1, 2), (2, 0), (2, 1), (2, 3)] {
        o[r][c] = -o[r][c];
    }
    o
}

/// Triangles: swap the second and third index.
pub fn to_rh_triangle(t: [u16; 3]) -> [u16; 3] {
    [t[0], t[2], t[1]]
}

fn parse_piece(c: &mut Cursor, container: u16) -> Result<Piece> {
    let name = c.string()?;
    let lod_count = c.count(4 + 4 + 16 + 4 + 1 + 4 + 4 + 1, "LODs")?;
    let lod_distances = (0..lod_count).map(|_| Ok(c.f32()?)).collect::<Result<Vec<_>>>()?;
    let old_lod_offsets = (c.u32()?, c.u32()?);
    let mut lods = Vec::with_capacity(lod_count);
    for i in 0..lod_count {
        lods.push(parse_lod(c, container).map_err(|e| LtbError::from(e).context(&format!("LOD {i}")))?);
    }
    Ok(Piece {
        name,
        lod_distances,
        old_lod_offsets,
        lods,
    })
}

fn parse_lod(c: &mut Cursor, container: u16) -> Result<Lod> {
    let texture_count = c.u32()?;
    let textures = [c.i32()?, c.i32()?, c.i32()?, c.i32()?];
    let render_style = c.i32()?;
    let render_priority = c.u8()?;
    let at = c.pos;
    let kind = c.u32()?;
    if !(4..=7).contains(&kind) {
        return Err(LtbError {
            offset: at,
            message: format!("mesh kind {kind}"),
        });
    }
    if kind != 7 && container != CONTAINER_VERSION {
        return Err(LtbError {
            offset: 2,
            message: format!("container version {container}, not {CONTAINER_VERSION}"),
        });
    }
    let body_size = c.u32()?;
    let start = c.pos;
    let body = c.take(body_size as usize)?;
    let mut b = Le::new(body);
    let mesh = match kind {
        4 => parse_rigid(&mut b),
        5 => parse_skinned(&mut b),
        6 => parse_vertex_animated(&mut b),
        _ => Ok(Mesh::Empty),
    }
    .map_err(|mut e| {
        e.offset += start;
        e
    })?;
    if b.pos != body.len() {
        return Err(LtbError {
            offset: start + b.pos,
            message: format!("mesh body takes {} of its {} bytes", b.pos, body.len()),
        });
    }
    let used = c.u8()? as usize;
    let used_nodes = c.take(used)?.to_vec();
    Ok(Lod {
        texture_count,
        textures,
        render_style,
        render_priority,
        body_size,
        mesh,
        used_nodes,
    })
}

fn stream_flags(c: &mut Cursor) -> Result<[u32; 4]> {
    let at = c.pos;
    let f = [c.u32()?, c.u32()?, c.u32()?, c.u32()?];
    if f.iter().any(|&w| w != 0 && w != STREAM_POS_NORMAL_UV) {
        return Err(LtbError {
            offset: at,
            message: format!("stream flags {f:#x?}: only 0 and 0x13 are supported"),
        });
    }
    if f.iter().all(|&w| w == 0) {
        return Err(LtbError {
            offset: at,
            message: "no vertex stream".into(),
        });
    }
    Ok(f)
}

/// The vertex streams (k stored weights, palette indices or not), then
/// the triangles.
fn read_vertices(
    c: &mut Cursor,
    g: &mut Geometry,
    vertices: usize,
    triangles: usize,
    k: usize,
    palette: bool,
) -> Result<()> {
    g.weight_count = k;
    g.stride = 24 + 4 * k + if palette { 4 } else { 0 } + 8;
    let streams = g.stream_flags.iter().filter(|&&w| w != 0).count();
    c.check(vertices.saturating_mul(g.stride).saturating_mul(streams), "vertices")?;
    for s in 0..streams {
        let keep = s == 0;
        for _ in 0..vertices {
            let p = c.vec3()?;
            let mut w = [0.0; 4];
            for x in w.iter_mut().take(k) {
                *x = c.f32()?;
            }
            let idx = if palette { c.array::<4>()? } else { [0; 4] };
            let n = c.vec3()?;
            let uv = [c.f32()?, c.f32()?];
            if keep {
                g.positions.push(p);
                if k > 0 {
                    g.weights.push(w);
                }
                if palette {
                    g.palette_indices.push(idx);
                }
                g.normals.push(n);
                g.uvs.push(uv);
            }
        }
    }
    c.check(triangles.saturating_mul(6), "triangles")?;
    for _ in 0..triangles {
        let at = c.pos;
        let t = [c.u16()?, c.u16()?, c.u16()?];
        if t.iter().any(|&i| i as usize >= vertices) {
            return Err(LtbError {
                offset: at,
                message: format!("triangle {t:?} with {vertices} vertices"),
            });
        }
        g.triangles.push(t);
    }
    Ok(())
}

fn parse_rigid(c: &mut Cursor) -> Result<Mesh> {
    let (v, n) = (c.u32()? as usize, c.u32()? as usize);
    let mut g = Geometry {
        max_bones_per_triangle: c.u32()?,
        max_bones_per_vertex: c.u32()?,
        stream_flags: stream_flags(c)?,
        ..Default::default()
    };
    let node = c.u32()?;
    read_vertices(c, &mut g, v, n, 0, false)?;
    Ok(Mesh::Rigid { node, geometry: g })
}

fn parse_skinned(c: &mut Cursor) -> Result<Mesh> {
    let (v, n) = (c.u32()? as usize, c.u32()? as usize);
    let at = c.pos;
    let bt = c.u32()?;
    let bv = c.u32()?;
    let reindexed = c.u8()? != 0;
    let mut g = Geometry {
        max_bones_per_triangle: bt,
        max_bones_per_vertex: bv,
        stream_flags: stream_flags(c)?,
        ..Default::default()
    };
    let palette_flag = c.u8()? != 0;
    if !palette_flag {
        if !(1..=4).contains(&bt) {
            return Err(LtbError {
                offset: at,
                message: format!("{bt} bones per triangle (1..4)"),
            });
        }
        read_vertices(c, &mut g, v, n, bt as usize - 1, false)?;
        let sets = c.count(12, "bone sets")?;
        let mut bone_sets = Vec::with_capacity(sets);
        let (mut vertex_end, mut index_end) = (0usize, 0usize);
        for _ in 0..sets {
            let at = c.pos;
            let s = BoneSet {
                first_vertex: c.u16()?,
                vertex_count: c.u16()?,
                nodes: c.array()?,
                index_end: c.u32()?,
            };
            // "Bone sets": contiguous in vertices and indices, every
            // index in the set's range, used slots increasing.
            let (first, end) = (s.first_vertex as usize, s.first_vertex as usize + s.vertex_count as usize);
            let used: Vec<u8> = s.used().collect();
            let bad = if first != vertex_end {
                Some(format!("starts at vertex {first}, not {vertex_end}"))
            } else if (s.index_end as usize) < index_end || s.index_end as usize > 3 * n || s.index_end % 3 != 0 {
                Some(format!("index end {} after {index_end} of {}", s.index_end, 3 * n))
            } else if used.is_empty() || used.windows(2).any(|w| w[0] >= w[1]) {
                Some(format!("bones {:?}", s.nodes))
            } else {
                g.triangles[index_end / 3..s.index_end as usize / 3]
                    .iter()
                    .flatten()
                    .find(|&&i| !(first..end).contains(&(i as usize)))
                    .map(|i| format!("index {i} outside vertices {first}..{end}"))
            };
            if let Some(m) = bad {
                return Err(LtbError {
                    offset: at,
                    message: format!("bone set: {m}"),
                });
            }
            vertex_end = end;
            index_end = s.index_end as usize;
            bone_sets.push(s);
        }
        if vertex_end != v || index_end != 3 * n {
            return Err(c.err(format!(
                "bone sets cover {vertex_end} of {v} vertices and {index_end} of {} indices",
                3 * n
            )));
        }
        Ok(Mesh::Skinned {
            reindexed,
            geometry: g,
            bone_sets,
            palette: None,
        })
    } else {
        if !(2..=4).contains(&bv) {
            return Err(LtbError {
                offset: at + 4,
                message: format!("{bv} bones per vertex (2..4) in a palette mesh"),
            });
        }
        let (min_index, max_index) = (c.u32()?, c.u32()?);
        let remap = if reindexed {
            let r = c.count(4, "palette entries")?;
            (0..r).map(|_| Ok(c.u32()?)).collect::<Result<_>>()?
        } else {
            Vec::new()
        };
        read_vertices(c, &mut g, v, n, bv as usize - 1, true)?;
        Ok(Mesh::Skinned {
            reindexed,
            geometry: g,
            bone_sets: Vec::new(),
            palette: Some(Palette {
                min_index,
                max_index,
                remap,
            }),
        })
    }
}

fn parse_vertex_animated(c: &mut Cursor) -> Result<Mesh> {
    let v = c.u32()? as usize;
    let animated_vertices = c.u32()?;
    let n = c.u32()? as usize;
    let mut g = Geometry {
        max_bones_per_triangle: c.u32()?,
        max_bones_per_vertex: c.u32()?,
        stream_flags: stream_flags(c)?,
        ..Default::default()
    };
    let anim_node = c.u32()?;
    let draw_node = c.u32()?;
    read_vertices(c, &mut g, v, n, 0, false)?;
    let d = c.count(4, "duplicates")?;
    let mut duplicates = Vec::with_capacity(d);
    for _ in 0..d {
        let at = c.pos;
        let pair = (c.u16()?, c.u16()?);
        if pair.0 as usize >= v || pair.1 as usize >= v {
            return Err(LtbError {
                offset: at,
                message: format!("duplicate {pair:?} with {v} vertices"),
            });
        }
        duplicates.push(pair);
    }
    if animated_vertices as usize > v {
        return Err(c.err(format!("{animated_vertices} animated of {v} vertices")));
    }
    Ok(Mesh::VertexAnimated {
        animated_vertices,
        anim_node,
        draw_node,
        geometry: g,
        duplicates,
    })
}

/// Deeper trees than this are corrupt (bounds the recursion).
const MAX_DEPTH: usize = 256;

fn parse_node(c: &mut Cursor, parent: Option<usize>, nodes: &mut Vec<Node>, depth: usize) -> Result<()> {
    if depth > MAX_DEPTH {
        return Err(c.err("node tree deeper than 256"));
    }
    let name = c.string()?;
    let stored_index = c.u16()?;
    let flags = c.u8()?;
    let mut bind = [[0.0; 4]; 4];
    for row in &mut bind {
        for v in row.iter_mut() {
            *v = c.f32()?;
        }
    }
    let index = nodes.len();
    nodes.push(Node {
        name,
        stored_index,
        flags,
        bind,
        parent,
    });
    // A node record is at least 2 + 2 + 1 + 64 + 4 bytes.
    let children = c.count(73, "child nodes")?;
    for _ in 0..children {
        parse_node(c, Some(index), nodes, depth + 1)?;
    }
    Ok(())
}

fn parse_animation(c: &mut Cursor, nodes: usize) -> Result<Animation> {
    let dims = c.vec3()?;
    let name = c.string()?;
    let at = c.pos;
    let compression = c.u32()?;
    if compression > 3 {
        return Err(LtbError {
            offset: at,
            message: format!("compression {compression}"),
        });
    }
    let interp_ms = c.u32()?;
    let key_count = c.count(6, "keys")?;
    let keys = (0..key_count)
        .map(|_| {
            Ok(Key {
                time: c.u32()?,
                string: c.string()?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let mut channels = Vec::with_capacity(nodes);
    for j in 0..nodes {
        let ch = if compression == 0 {
            if c.u8()? == 0 {
                c.check(key_count.saturating_mul(28), "keys of node channels")?;
                let positions = (0..key_count).map(|_| c.vec3()).collect::<Result<_>>()?;
                let rotations = (0..key_count).map(|_| c.quat()).collect::<Result<_>>()?;
                Channel::Transform { positions, rotations }
            } else {
                let frames = (0..key_count)
                    .map(|_| {
                        let n = c.count(12, "animated vertices")?;
                        (0..n).map(|_| c.vec3()).collect::<Result<Vec<_>>>()
                    })
                    .collect::<Result<_>>()?;
                Channel::Vertices(frames)
            }
        } else {
            let at = c.pos;
            let p = c.count(if compression == 2 { 6 } else { 12 }, "positions")?;
            check_channel(p, key_count, at, j)?;
            let positions = (0..p)
                .map(|_| {
                    if compression == 2 {
                        Ok([c.i16()? as f32 / 16.0, c.i16()? as f32 / 16.0, c.i16()? as f32 / 16.0])
                    } else {
                        c.vec3()
                    }
                })
                .collect::<Result<_>>()?;
            let at = c.pos;
            let q = c.count(if compression == 1 { 16 } else { 8 }, "rotations")?;
            check_channel(q, key_count, at, j)?;
            let rotations = (0..q)
                .map(|_| {
                    if compression == 1 {
                        c.quat()
                    } else {
                        let s = 1.0 / 32767.0;
                        Ok([c.i16()? as f32 * s, c.i16()? as f32 * s, c.i16()? as f32 * s, c.i16()? as f32 * s])
                    }
                })
                .collect::<Result<_>>()?;
            Channel::Transform { positions, rotations }
        };
        channels.push(ch);
    }
    Ok(Animation {
        dims,
        name,
        compression,
        interp_ms,
        keys,
        channels,
    })
}

/// animation.md "Filling missing keys": 0, 1 or N stored values.
fn check_channel(stored: usize, keys: usize, at: usize, node: usize) -> Result<()> {
    if stored > 1 && stored != keys {
        return Err(LtbError {
            offset: at,
            message: format!("node {node}: {stored} values for {keys} keys"),
        });
    }
    Ok(())
}

fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

impl LtbError {
    fn context(mut self, what: &str) -> Self {
        self.message = format!("{what}: {}", self.message);
        self
    }
}

/// The shared bounds-checked little-endian reader.
type Cursor<'a> = Le<'a>;

impl From<Truncated> for LtbError {
    fn from(t: Truncated) -> Self {
        LtbError {
            offset: t.at,
            message: format!("{} bytes wanted, {} left", t.wanted, t.left),
        }
    }
}

/// What the format adds to the shared reader.
trait ReadLtbError {
    fn err(&self, message: impl Into<String>) -> LtbError;
    fn vec3(&mut self) -> Result<Vec3>;
    fn quat(&mut self) -> Result<Quat>;
    fn string(&mut self) -> Result<String>;
    fn check(&self, n: usize, what: &str) -> Result<()>;
    fn count(&mut self, min_size: usize, what: &str) -> Result<usize>;
}

impl ReadLtbError for Le<'_> {
    fn err(&self, message: impl Into<String>) -> LtbError {
        LtbError {
            offset: self.pos,
            message: message.into(),
        }
    }

    fn vec3(&mut self) -> Result<Vec3> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }

    fn quat(&mut self) -> Result<Quat> {
        Ok([self.f32()?, self.f32()?, self.f32()?, self.f32()?])
    }

    /// "string": u16 byte count, the bytes, no terminator.
    fn string(&mut self) -> Result<String> {
        let n = self.u16()? as usize;
        Ok(latin1(self.take(n)?))
    }

    /// Fails unless `n` more bytes are left.
    fn check(&self, n: usize, what: &str) -> Result<()> {
        let left = self.b.len() - self.pos;
        if n > left {
            return Err(self.err(format!("{what}: {n} bytes can't fit in the {left} left")));
        }
        Ok(())
    }

    /// A u32 count of elements at least `min_size` bytes each, checked
    /// against the bytes left.
    fn count(&mut self, min_size: usize, what: &str) -> Result<usize> {
        let at = self.pos;
        let n = self.u32()? as usize;
        let left = self.b.len() - self.pos;
        if n.saturating_mul(min_size) > left {
            return Err(LtbError {
                offset: at,
                message: format!("{n} {what} can't fit in the {left} bytes left"),
            });
        }
        Ok(n)
    }}
