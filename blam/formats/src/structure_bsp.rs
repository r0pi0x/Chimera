//! The structure BSP reader (specs/halo_ce/structure_bsp.md): a level's
//! render geometry (lightmap groups, materials, decoded vertices and
//! triangles), clusters, portals, leaves, the potentially visible set, fog,
//! weather and the remaining blocks, read from a `MapFile`'s BSP sub-file.
//!
//! `StructureBsp::read(map, index)` reads everything except the vertices:
//! each `Material` knows where its vertex data is, and
//! `StructureBsp::material_geometry` decodes one material's vertices and
//! triangles on demand (so a converter iterates `materials()` and decodes
//! each once). A `StructureBsp` owns a clone of the `MapFile` (an `Arc`):
//! it is `Send + Sync` and the geometry can be decoded from several
//! threads. The level's collision BSP is `collision::read_level`.
//!
//! Values are the game's own: world units, right-handed, +Z up, nothing
//! converted. Winding is as stored: seen from the front (the side the
//! vertex normals point to) a triangle's vertices run clockwise, so
//! `(p1 - p0) x (p2 - p0)` points away from the normals. A converter to a
//! counter-clockwise-front convention swaps the second and third index.
//!
//! Known shortcuts:
//! - Fog planes, regions, palettes, weather, sound palettes, markers, lens
//!   flares and breakable surfaces are read as the spec's tables give
//!   them; the weather palette's fields after the wind reference, the
//!   leaves' byte bounds and the 6-byte nodes are kept raw or skipped (the
//!   spec does not know their meaning).
//! - Detail objects, runtime decals, weather polyhedra, pathfinding, leaf
//!   map blocks, mirrors, per-cluster surface lists and predicted
//!   resources are counted (`Counts`) or kept as the spec's raw fields
//!   only; their contents are not decoded.
//! - Uncompressed vertex forms (none on the disc) are decoded from the
//!   spec's layout and untested against real data.

use std::io;

use crate::cache::{Bounds, MapFile, Plane3d, RealArgb, RealRect3d, RealRgb, TagEntry, TagRef, View};
use crate::collision::{CollisionMaterial, read_collision_materials};

/// The main struct's size.
pub const STRUCTURE_SIZE: usize = 0x288;
/// A compressed rendered vertex's size.
pub const COMPRESSED_VERTEX_SIZE: usize = 32;
/// A compressed lightmap vertex's size.
pub const COMPRESSED_LIGHTMAP_VERTEX_SIZE: usize = 8;
/// An uncompressed rendered vertex's size.
pub const UNCOMPRESSED_VERTEX_SIZE: usize = 56;
/// An uncompressed lightmap vertex's size.
pub const UNCOMPRESSED_LIGHTMAP_VERTEX_SIZE: usize = 20;
/// The most vertices a material may have (indices are u16).
pub const MAX_MATERIAL_VERTICES: usize = 65_536;

const MAX_GROUPS: usize = 4096;
const MAX_MATERIALS_PER_GROUP: usize = 256;

/// Unpack a packed vector (a u32: x 11 bits, y 11 bits, z 10 bits, each
/// two's complement): components `(2x+1)/2047`, `(2y+1)/2047`,
/// `(2z+1)/1023`. Raw 0 is not (0, 0, 0).
pub fn unpack_vector(w: u32) -> [f32; 3] {
    let x = ((w << 21) as i32) >> 21;
    let y = ((w << 10) as i32) >> 21;
    let z = (w as i32) >> 22;
    [
        (2 * x + 1) as f32 / 2047.0,
        (2 * y + 1) as f32 / 2047.0,
        (2 * z + 1) as f32 / 1023.0,
    ]
}

/// Unpack a 16-bit coordinate: `(2s + 1) / 65535`.
pub fn unpack_coordinate(s: i16) -> f32 {
    (2 * s as i32 + 1) as f32 / 65535.0
}

/// One distant light of a lighting block.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DistantLight {
    pub color: RealRgb,
    pub direction: [f32; 3],
}

/// A lighting block (116 bytes): the BSP's default lighting and each
/// material's.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Lighting {
    pub ambient: RealRgb,
    /// How many of `distant_lights` are used (0..=2).
    pub distant_light_count: i16,
    pub distant_lights: [DistantLight; 2],
    pub point_light_count: i16,
    pub point_light_indices: [i32; 2],
    pub reflection_tint: RealArgb,
    pub shadow_vector: [f32; 3],
    pub shadow_color: RealRgb,
}

impl Lighting {
    fn read(v: &View<'_>, o: usize) -> io::Result<Lighting> {
        let light = |at: usize| -> io::Result<DistantLight> {
            Ok(DistantLight {
                color: v.real_rgb(at)?,
                direction: v.vector3(at + 12)?,
            })
        };
        Ok(Lighting {
            ambient: v.real_rgb(o)?,
            distant_light_count: v.i16(o + 0x0C)?,
            distant_lights: [light(o + 0x10)?, light(o + 0x28)?],
            point_light_count: v.i16(o + 0x40)?,
            point_light_indices: [v.i32(o + 0x44)?, v.i32(o + 0x48)?],
            reflection_tint: v.real_argb(o + 0x4C)?,
            shadow_vector: v.vector3(o + 0x5C)?,
            shadow_color: v.real_rgb(o + 0x68)?,
        })
    }
}

/// A material's flag word.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MaterialFlags(pub u16);

impl MaterialFlags {
    pub fn coplanar(self) -> bool {
        self.0 & 1 != 0
    }
    pub fn fog_plane(self) -> bool {
        self.0 & 2 != 0
    }
}

/// A render material: one shader's share of a lightmap group. The vertices
/// are decoded by `StructureBsp::material_geometry`.
#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub shader: TagRef,
    pub shader_permutation: i16,
    pub flags: MaterialFlags,
    /// First surface (triangle) in the BSP's surface list.
    pub first_surface: u32,
    pub surface_count: u32,
    /// Filled for transparent materials, zero otherwise.
    pub centroid: [f32; 3],
    pub lighting: Lighting,
    /// Filled for transparent materials.
    pub plane: Plane3d,
    /// The breakable surface that must be intact to draw this material.
    pub breakable_surface: Option<u16>,
    pub vertex_count: u32,
    /// Equal to `vertex_count` (lightmapped) or 0.
    pub lightmap_vertex_count: u32,
    /// Whether the vertices are stored compressed (every one on the disc).
    pub compressed: bool,
    /// Which lightmap group (and page) the material belongs to, and its
    /// position there.
    pub group: u32,
    pub index_in_group: u32,
    /// The material's position counting across all groups from 0.
    pub flat_index: u32,
}

impl Material {
    pub fn has_lightmap(&self) -> bool {
        self.lightmap_vertex_count > 0
    }
}

/// A lightmap group: a lightmap page and the materials drawn with it.
#[derive(Clone, Debug, PartialEq)]
pub struct LightmapGroup {
    /// Index into the lightmap bitmap's images; `None` = no page.
    pub bitmap_index: Option<u16>,
    pub materials: Vec<Material>,
}

/// A decoded rendered vertex.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub binormal: [f32; 3],
    pub tangent: [f32; 3],
    /// Base texture coordinates (unbounded, tiling).
    pub uv: [f32; 2],
    /// The packed normal, binormal and tangent as stored (zero for the
    /// uncompressed form).
    pub packed: [u32; 3],
}

/// A decoded lightmap vertex.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightmapVertex {
    /// Incident radiosity direction (not unit length).
    pub incident_direction: [f32; 3],
    /// Lightmap coordinates, 0..1.
    pub uv: [f32; 2],
    /// The packed direction and the raw 16-bit u, v as stored (zero for the
    /// uncompressed form).
    pub packed_direction: u32,
    pub raw_uv: [i16; 2],
}

/// One material's decoded geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialGeometry {
    pub vertices: Vec<Vertex>,
    /// Empty (not lightmapped) or one per vertex.
    pub lightmap_vertices: Vec<LightmapVertex>,
    /// Triangles as index triples local to this material's vertices, as
    /// stored (clockwise seen from the front).
    pub triangles: Vec<[u32; 3]>,
}

/// A cluster's fog field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClusterFog {
    None,
    /// A fog region index.
    Region(u16),
    /// A fog plane index.
    Plane(u16),
}

/// A subcluster: a bounding box and the triangles inside (global surface
/// indices).
#[derive(Clone, Debug, PartialEq)]
pub struct Subcluster {
    pub bounds: RealRect3d,
    pub surface_indices: Vec<u32>,
}

/// A cluster (104 bytes).
#[derive(Clone, Debug, PartialEq)]
pub struct Cluster {
    /// Index into the scenario's sky list.
    pub sky: Option<u16>,
    pub fog: ClusterFog,
    pub background_sound: Option<u16>,
    pub sound_environment: Option<u16>,
    pub weather: Option<u16>,
    pub transition_structure_bsp: i16,
    pub first_runtime_decal: Option<u16>,
    pub runtime_decal_count: u16,
    pub predicted_resource_count: u32,
    pub subclusters: Vec<Subcluster>,
    pub first_lens_flare_marker: u16,
    pub lens_flare_marker_count: u16,
    pub surface_index_count: u32,
    pub mirror_count: u32,
    /// Indices into the cluster portals.
    pub portals: Vec<u16>,
}

/// A cluster portal (64 bytes).
#[derive(Clone, Debug, PartialEq)]
pub struct ClusterPortal {
    pub cluster_a: u16,
    pub cluster_b: u16,
    /// Index into the collision BSP's planes.
    pub plane: u32,
    pub centroid: [f32; 3],
    pub bounding_radius: f32,
    pub flags: u32,
    pub vertices: Vec<[f32; 3]>,
}

/// A leaf (16 bytes); the leaves run parallel to the collision BSP's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Leaf {
    /// Per-axis byte bounds (meaning unknown).
    pub bounds: [u8; 6],
    pub cluster: Option<u16>,
    pub surface_reference_count: u16,
    pub first_surface_reference: u32,
}

/// A leaf's surface reference (8 bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeafSurfaceReference {
    /// Global surface index.
    pub surface: u32,
    /// Collision BSP 3D node index; -1 occurs.
    pub node: i32,
}

/// The potentially visible set: N rows of W u32 words.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Visibility {
    pub cluster_count: usize,
    pub words_per_row: usize,
    pub words: Vec<u32>,
}

impl Visibility {
    /// Whether cluster `to` may be visible from cluster `from`.
    pub fn visible(&self, from: usize, to: usize) -> bool {
        if from >= self.cluster_count || to >= self.cluster_count {
            return false;
        }
        self.words
            .get(from * self.words_per_row + to / 32)
            .is_some_and(|w| (w >> (to % 32)) & 1 != 0)
    }
}

/// A fog plane (32 bytes).
#[derive(Clone, Debug, PartialEq)]
pub struct FogPlane {
    pub fog_region: Option<u16>,
    pub runtime_material_type: i16,
    pub plane: Plane3d,
    pub vertices: Vec<[f32; 3]>,
}

/// A fog region (40 bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FogRegion {
    pub fog_palette: Option<u16>,
    pub weather_palette: Option<u16>,
}

/// A fog palette entry (136 bytes).
#[derive(Clone, Debug, PartialEq)]
pub struct FogPaletteEntry {
    pub name: String,
    /// A `fog ` tag.
    pub fog: TagRef,
    pub scale_function: String,
}

/// A weather palette entry (240 bytes).
#[derive(Clone, Debug, PartialEq)]
pub struct WeatherPaletteEntry {
    pub name: String,
    /// A `rain` tag (none on every multiplayer map).
    pub particle_system: TagRef,
    pub scale_function: String,
    /// A `wind` tag.
    pub wind: TagRef,
    pub wind_direction: [f32; 3],
    pub wind_magnitude: f32,
    pub wind_scale_function: String,
}

/// A palette entry naming a tag: background sounds (`lsnd`) and sound
/// environments (`snde`).
#[derive(Clone, Debug, PartialEq)]
pub struct SoundPaletteEntry {
    pub name: String,
    pub tag: TagRef,
    /// Only background sounds have one (empty for environments).
    pub scale_function: String,
}

/// A breakable surface (48 bytes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BreakableSurface {
    pub centroid: [f32; 3],
    pub radius: f32,
    pub collision_surface: i32,
}

/// A lens flare marker (16 bytes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LensFlareMarker {
    pub position: [f32; 3],
    /// Components are the stored bytes / 127.
    pub direction: [f32; 3],
    pub lens_flare_index: u8,
}

/// A marker (60 bytes).
#[derive(Clone, Debug, PartialEq)]
pub struct Marker {
    pub name: String,
    /// i, j, k, w.
    pub rotation: [f32; 4],
    pub position: [f32; 3],
}

/// Element counts of the blocks that are only counted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub nodes: u32,
    pub weather_polyhedra: u32,
    pub pathfinding_surfaces: u32,
    pub pathfinding_edges: u32,
    pub detail_objects: u32,
    pub runtime_decals: u32,
    pub leaf_map_leaves: u32,
    pub leaf_map_portals: u32,
    /// The collision BSP's own counts (`collision::read_level` reads it).
    pub collision_nodes: u32,
    pub collision_planes: u32,
    pub collision_leaves: u32,
    pub collision_surfaces: u32,
}

/// A structure BSP of a map.
#[derive(Clone, Debug)]
pub struct StructureBsp {
    map: MapFile,
    index: usize,
    /// The lightmap bitmap (`bitm`); null when none.
    pub lightmap_bitmap: TagRef,
    pub vehicle_floor: f32,
    pub vehicle_ceiling: f32,
    pub default_lighting: Lighting,
    pub collision_materials: Vec<CollisionMaterial>,
    pub world_bounds: RealRect3d,
    pub lightmap_groups: Vec<LightmapGroup>,
    /// Every triangle of the BSP (the surface list), indices local to the
    /// owning material.
    triangles: Vec<[u16; 3]>,
    pub clusters: Vec<Cluster>,
    pub cluster_portals: Vec<ClusterPortal>,
    pub leaves: Vec<Leaf>,
    pub leaf_surface_references: Vec<LeafSurfaceReference>,
    pub visibility: Visibility,
    pub sound_cluster_data: Vec<u8>,
    pub breakable_surfaces: Vec<BreakableSurface>,
    pub fog_planes: Vec<FogPlane>,
    pub fog_regions: Vec<FogRegion>,
    pub fog_palette: Vec<FogPaletteEntry>,
    pub weather_palette: Vec<WeatherPaletteEntry>,
    pub background_sound_palette: Vec<SoundPaletteEntry>,
    pub sound_environment_palette: Vec<SoundPaletteEntry>,
    pub lens_flares: Vec<TagRef>,
    pub lens_flare_markers: Vec<LensFlareMarker>,
    pub markers: Vec<Marker>,
    pub counts: Counts,
}

fn opt_u16(v: &View<'_>, off: usize) -> io::Result<Option<u16>> {
    Ok(v.index16(off)?.map(|i| i as u16))
}

fn bounds_of(v: &View<'_>, off: usize) -> io::Result<RealRect3d> {
    v.real_rect3d(off)
}

fn read_palette(v: &View<'_>, off: usize, size: usize, scale_at: Option<usize>) -> io::Result<Vec<SoundPaletteEntry>> {
    v.block(off, size)?
        .iter()
        .map(|e| {
            Ok(SoundPaletteEntry {
                name: e.tag_string(0)?.to_owned(),
                tag: e.tag_ref(0x20)?,
                scale_function: match scale_at {
                    Some(at) => e.tag_string(at)?.to_owned(),
                    None => String::new(),
                },
            })
        })
        .collect()
}

impl StructureBsp {
    /// Read structure BSP `index` of `map` (its position in the scenario's
    /// BSP list), except the vertices.
    pub fn read(map: &MapFile, index: usize) -> io::Result<StructureBsp> {
        let v = map.bsp_view(index, STRUCTURE_SIZE)?;
        let bad = |off: usize, what: String| v.error(off, what);

        let lightmap_bitmap = v.tag_ref(0x00)?;
        let triangles: Vec<[u16; 3]> = v
            .block(0xF8, 6)?
            .iter()
            .map(|e| Ok([e.u16(0)?, e.u16(2)?, e.u16(4)?]))
            .collect::<io::Result<_>>()?;

        // Render geometry.
        let mut groups = Vec::new();
        let mut flat = 0u32;
        let mut covered = 0u64;
        for (gi, g) in v.block_max(0x104, 32, MAX_GROUPS)?.iter().enumerate() {
            let bitmap_index = opt_u16(&g, 0)?;
            let mut materials = Vec::new();
            for (mi, m) in g.block_max(0x14, 256, MAX_MATERIALS_PER_GROUP)?.iter().enumerate() {
                let first = m.i32(0x14)?;
                let count = m.i32(0x18)?;
                if first < 0 || count < 0 || first as i64 + count as i64 > triangles.len() as i64 {
                    return Err(bad(
                        0x104,
                        format!(
                            "group {gi} material {mi}: surfaces {first}+{count} of {}",
                            triangles.len()
                        ),
                    ));
                }
                covered += count as u64;
                let rendered = (m.i16(0xB0)?, m.i32(0xB4)?);
                let lightmap = (m.i16(0xC4)?, m.i32(0xC8)?);
                if rendered.1 < 0 || rendered.1 as usize > MAX_MATERIAL_VERTICES || lightmap.1 < 0 {
                    return Err(bad(
                        0x104,
                        format!(
                            "group {gi} material {mi}: vertex counts {} and {}",
                            rendered.1, lightmap.1
                        ),
                    ));
                }
                let compressed = match rendered.0 {
                    0 => false,
                    1 => true,
                    f => {
                        return Err(bad(
                            0x104,
                            format!("group {gi} material {mi}: rendered vertex form {f}"),
                        ));
                    }
                };
                if lightmap.1 != 0 && (lightmap.1 != rendered.1 || lightmap.0 != rendered.0 + 2) {
                    return Err(bad(
                        0x104,
                        format!(
                            "group {gi} material {mi}: {} lightmap vertices of form {} for {} rendered",
                            lightmap.1, lightmap.0, rendered.1
                        ),
                    ));
                }
                materials.push(Material {
                    shader: m.tag_ref(0)?,
                    shader_permutation: m.i16(0x10)?,
                    flags: MaterialFlags(m.u16(0x12)?),
                    first_surface: first as u32,
                    surface_count: count as u32,
                    centroid: m.point3(0x1C)?,
                    lighting: Lighting::read(&m, 0x28)?,
                    plane: m.plane3d(0x9C)?,
                    breakable_surface: opt_u16(&m, 0xAC)?,
                    vertex_count: rendered.1 as u32,
                    lightmap_vertex_count: lightmap.1 as u32,
                    compressed,
                    group: gi as u32,
                    index_in_group: mi as u32,
                    flat_index: flat,
                });
                flat += 1;
            }
            groups.push(LightmapGroup {
                bitmap_index,
                materials,
            });
        }
        if covered != triangles.len() as u64 {
            return Err(bad(
                0x104,
                format!("materials own {covered} surfaces, the BSP has {}", triangles.len()),
            ));
        }

        // Clusters.
        let portal_block = v.block_max(0x154, 64, 65_536)?;
        let cluster_block = v.block_max(0x134, 104, 65_536)?;
        let n_clusters = cluster_block.len();
        let n_portals = portal_block.len();
        let mut clusters = Vec::with_capacity(n_clusters);
        for c in cluster_block.iter() {
            let fog = match c.i16(0x02)? {
                -1 => ClusterFog::None,
                f if f < 0 => ClusterFog::Plane((f & 0x7FFF) as u16),
                f => ClusterFog::Region(f as u16),
            };
            let mut subclusters = Vec::new();
            for s in c.block(0x34, 36)?.iter() {
                let indices = s
                    .block(0x18, 4)?
                    .iter()
                    .map(|i| {
                        let s = i.i32(0)?;
                        if s < 0 || s as usize >= triangles.len() {
                            return Err(i.error(0, format_args!("subcluster surface {s} of {}", triangles.len())));
                        }
                        Ok(s as u32)
                    })
                    .collect::<io::Result<Vec<_>>>()?;
                subclusters.push(Subcluster {
                    bounds: bounds_of(&s, 0)?,
                    surface_indices: indices,
                });
            }
            let portals = c
                .block(0x5C, 2)?
                .iter()
                .map(|p| {
                    let i = p.i16(0)?;
                    if i < 0 || i as usize >= n_portals {
                        return Err(p.error(0, format_args!("cluster portal {i} of {n_portals}")));
                    }
                    Ok(i as u16)
                })
                .collect::<io::Result<Vec<_>>>()?;
            clusters.push(Cluster {
                sky: opt_u16(&c, 0x00)?,
                fog,
                background_sound: opt_u16(&c, 0x04)?,
                sound_environment: opt_u16(&c, 0x06)?,
                weather: opt_u16(&c, 0x08)?,
                transition_structure_bsp: c.i16(0x0A)?,
                first_runtime_decal: opt_u16(&c, 0x0C)?,
                runtime_decal_count: c.u16(0x0E)?,
                predicted_resource_count: c.raw_block(0x28)?.count.max(0) as u32,
                subclusters,
                first_lens_flare_marker: c.u16(0x40)?,
                lens_flare_marker_count: c.u16(0x42)?,
                surface_index_count: c.raw_block(0x44)?.count.max(0) as u32,
                mirror_count: c.raw_block(0x50)?.count.max(0) as u32,
                portals,
            });
        }
        let mut cluster_portals = Vec::with_capacity(n_portals);
        for p in portal_block.iter() {
            let (a, b) = (p.i16(0)?, p.i16(2)?);
            for c in [a, b] {
                if c < 0 || c as usize >= n_clusters {
                    return Err(p.error(0, format_args!("portal cluster {c} of {n_clusters}")));
                }
            }
            cluster_portals.push(ClusterPortal {
                cluster_a: a as u16,
                cluster_b: b as u16,
                plane: p.i32(4)? as u32,
                centroid: p.point3(8)?,
                bounding_radius: p.f32(0x14)?,
                flags: p.u32(0x18)?,
                vertices: p
                    .block_max(0x34, 12, 4096)?
                    .iter()
                    .map(|e| e.point3(0))
                    .collect::<io::Result<_>>()?,
            });
        }

        // Leaves.
        let leaves = v
            .block(0xE0, 16)?
            .iter()
            .map(|e| {
                let cluster = opt_u16(&e, 8)?;
                if cluster.is_some_and(|c| c as usize >= n_clusters) {
                    return Err(e.error(8, format_args!("leaf cluster {cluster:?} of {n_clusters}")));
                }
                Ok(Leaf {
                    bounds: e.slice(0, 6)?.try_into().expect("6 bytes"),
                    cluster,
                    surface_reference_count: e.i16(10)?.max(0) as u16,
                    first_surface_reference: e.i32(12)?.max(0) as u32,
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        let leaf_surface_references = v
            .block(0xEC, 8)?
            .iter()
            .map(|e| {
                Ok(LeafSurfaceReference {
                    surface: e.i32(0)? as u32,
                    node: e.i32(4)?,
                })
            })
            .collect::<io::Result<Vec<_>>>()?;

        // Potentially visible set.
        let pvs_bytes = v.data(0x140)?;
        let words_per_row = n_clusters.div_ceil(32);
        let expected = 4 * n_clusters * words_per_row;
        if pvs_bytes.len() != expected {
            return Err(bad(
                0x140,
                format!(
                    "cluster data of {} bytes for {n_clusters} clusters (expected {expected})",
                    pvs_bytes.len()
                ),
            ));
        }
        let visibility = Visibility {
            cluster_count: n_clusters,
            words_per_row,
            words: pvs_bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| u32::from_le_bytes(*c))
                .collect(),
        };

        let breakable_surfaces = v
            .block(0x16C, 48)?
            .iter()
            .map(|e| {
                Ok(BreakableSurface {
                    centroid: e.point3(0)?,
                    radius: e.f32(12)?,
                    collision_surface: e.i32(16)?,
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        let fog_planes = v
            .block(0x178, 32)?
            .iter()
            .map(|e| {
                Ok(FogPlane {
                    fog_region: opt_u16(&e, 0)?,
                    runtime_material_type: e.i16(2)?,
                    plane: e.plane3d(4)?,
                    vertices: e
                        .block(0x14, 12)?
                        .iter()
                        .map(|p| p.point3(0))
                        .collect::<io::Result<_>>()?,
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        let fog_regions = v
            .block(0x184, 40)?
            .iter()
            .map(|e| {
                Ok(FogRegion {
                    fog_palette: opt_u16(&e, 0x24)?,
                    weather_palette: opt_u16(&e, 0x26)?,
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        let fog_palette = v
            .block(0x190, 136)?
            .iter()
            .map(|e| {
                Ok(FogPaletteEntry {
                    name: e.tag_string(0)?.to_owned(),
                    fog: e.tag_ref(0x20)?,
                    scale_function: e.tag_string(0x34)?.to_owned(),
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        let weather_palette = v
            .block(0x1B4, 240)?
            .iter()
            .map(|e| {
                Ok(WeatherPaletteEntry {
                    name: e.tag_string(0)?.to_owned(),
                    particle_system: e.tag_ref(0x20)?,
                    scale_function: e.tag_string(0x34)?.to_owned(),
                    wind: e.tag_ref(0x80)?,
                    wind_direction: e.point3(0x90)?,
                    wind_magnitude: e.f32(0x9C)?,
                    wind_scale_function: e.tag_string(0xA4)?.to_owned(),
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        let background_sound_palette = read_palette(&v, 0x1FC, 116, Some(0x34))?;
        let sound_environment_palette = read_palette(&v, 0x208, 80, None)?;
        let lens_flares = v
            .block(0x11C, 16)?
            .iter()
            .map(|e| e.tag_ref(0))
            .collect::<io::Result<Vec<_>>>()?;
        let lens_flare_markers = v
            .block(0x128, 16)?
            .iter()
            .map(|e| {
                let d = |o: usize| e.i8(o).map(|b| b as f32 / 127.0);
                Ok(LensFlareMarker {
                    position: e.point3(0)?,
                    direction: [d(12)?, d(13)?, d(14)?],
                    lens_flare_index: e.u8(15)?,
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        let markers = v
            .block(0x240, 60)?
            .iter()
            .map(|e| {
                Ok(Marker {
                    name: e.tag_string(0)?.to_owned(),
                    rotation: e.quaternion(0x20)?,
                    position: e.point3(0x30)?,
                })
            })
            .collect::<io::Result<Vec<_>>>()?;

        let count = |off: usize| -> io::Result<u32> { Ok(v.raw_block(off)?.count.max(0) as u32) };
        let collision = v.raw_block(0xB0)?;
        let counts = Counts {
            nodes: count(0xBC)?,
            weather_polyhedra: count(0x1C0)?,
            pathfinding_surfaces: count(0x1E4)?,
            pathfinding_edges: count(0x1F0)?,
            detail_objects: count(0x24C)?,
            runtime_decals: count(0x258)?,
            leaf_map_leaves: count(0x270)?,
            leaf_map_portals: count(0x27C)?,
            collision_nodes: 0,
            collision_planes: 0,
            collision_leaves: 0,
            collision_surfaces: 0,
        };
        let counts = if collision.count == 1 {
            let cv = v.block(0xB0, 0x60)?.get(0)?;
            Counts {
                collision_nodes: cv.raw_block(0x00)?.count.max(0) as u32,
                collision_planes: cv.raw_block(0x0C)?.count.max(0) as u32,
                collision_leaves: cv.raw_block(0x18)?.count.max(0) as u32,
                collision_surfaces: cv.raw_block(0x3C)?.count.max(0) as u32,
                ..counts
            }
        } else {
            counts
        };

        Ok(StructureBsp {
            map: map.clone(),
            index,
            lightmap_bitmap,
            vehicle_floor: v.f32(0x10)?,
            vehicle_ceiling: v.f32(0x14)?,
            default_lighting: Lighting::read(&v, 0x2C)?,
            collision_materials: read_collision_materials(&v)?,
            world_bounds: bounds_of(&v, 0xC8)?,
            lightmap_groups: groups,
            triangles,
            clusters,
            cluster_portals,
            leaves,
            leaf_surface_references,
            visibility,
            sound_cluster_data: v.data(0x214)?.to_vec(),
            breakable_surfaces,
            fog_planes,
            fog_regions,
            fog_palette,
            weather_palette,
            background_sound_palette,
            sound_environment_palette,
            lens_flares,
            lens_flare_markers,
            markers,
            counts,
        })
    }

    /// The BSP's position in the scenario's list.
    pub fn index(&self) -> usize {
        self.index
    }

    pub fn map(&self) -> &MapFile {
        &self.map
    }

    /// The number of triangles (the surface list's length).
    pub fn triangle_count(&self) -> usize {
        self.triangles.len()
    }

    /// Every material, across the groups in order.
    pub fn materials(&self) -> impl Iterator<Item = &Material> {
        self.lightmap_groups.iter().flat_map(|g| g.materials.iter())
    }

    pub fn material_count(&self) -> usize {
        self.lightmap_groups.iter().map(|g| g.materials.len()).sum()
    }

    /// The tag a material's shader names, if it resolves.
    pub fn shader_tag(&self, m: &Material) -> Option<&TagEntry> {
        self.map.tag(m.shader.id)
    }

    /// The sum of the materials' vertex counts (vertices are not shared).
    pub fn vertex_count(&self) -> usize {
        self.materials().map(|m| m.vertex_count as usize).sum()
    }

    pub fn lightmap_vertex_count(&self) -> usize {
        self.materials().map(|m| m.lightmap_vertex_count as usize).sum()
    }

    /// The cluster a leaf belongs to.
    pub fn cluster_of_leaf(&self, leaf: usize) -> Option<u16> {
        self.leaves.get(leaf)?.cluster
    }

    /// The sound propagation byte for two distinct clusters (0 = same,
    /// 255 = unreachable); `None` outside the data.
    pub fn sound_distance(&self, a: usize, b: usize) -> Option<u8> {
        let n = self.clusters.len();
        if a == b {
            return (a < n).then_some(0);
        }
        let (i, j) = (a.min(b), a.max(b));
        if j >= n {
            return None;
        }
        let at = i * (n - 1) - i * (i + 1) / 2 + j - 1;
        self.sound_cluster_data.get(at).copied()
    }

    /// Decode material `flat_index`'s vertices and triangles.
    pub fn material_geometry(&self, flat_index: usize) -> io::Result<MaterialGeometry> {
        let m = self.materials().nth(flat_index).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: no material {flat_index}", self.map.header().name),
            )
        })?;
        self.decode(m)
    }

    /// Decode a material of this BSP.
    pub fn decode(&self, m: &Material) -> io::Result<MaterialGeometry> {
        let g = self
            .lightmap_groups
            .get(m.group as usize)
            .and_then(|g| g.materials.get(m.index_in_group as usize))
            .filter(|x| *x == m)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "material is not of this BSP"))?;
        let v = self.map.bsp_view(self.index, STRUCTURE_SIZE)?;
        let group = v.block(0x104, 32)?.get(g.group as usize)?;
        let mv = group.block(0x14, 256)?.get(g.index_in_group as usize)?;
        let (n, ln) = (g.vertex_count as usize, g.lightmap_vertex_count as usize);
        let (vsize, lsize, data_at) = if g.compressed {
            (COMPRESSED_VERTEX_SIZE, COMPRESSED_LIGHTMAP_VERTEX_SIZE, 0xEC)
        } else {
            (UNCOMPRESSED_VERTEX_SIZE, UNCOMPRESSED_LIGHTMAP_VERTEX_SIZE, 0xD8)
        };
        let data = mv.data(data_at)?;
        let want = n * vsize + ln * lsize;
        if data.len() != want {
            return Err(mv.error(
                data_at,
                format_args!(
                    "vertex data of {} bytes for {n} vertices and {ln} lightmap vertices (expected {want})",
                    data.len()
                ),
            ));
        }
        let (rendered, lightmap) = data.split_at(n * vsize);
        let f = |b: &[u8], o: usize| f32::from_le_bytes(b[o..o + 4].try_into().expect("4 bytes"));
        let u = |b: &[u8], o: usize| u32::from_le_bytes(b[o..o + 4].try_into().expect("4 bytes"));
        let vertices = rendered
            .chunks_exact(vsize)
            .map(|b| {
                let position = [f(b, 0), f(b, 4), f(b, 8)];
                if g.compressed {
                    let packed = [u(b, 12), u(b, 16), u(b, 20)];
                    Vertex {
                        position,
                        normal: unpack_vector(packed[0]),
                        binormal: unpack_vector(packed[1]),
                        tangent: unpack_vector(packed[2]),
                        uv: [f(b, 24), f(b, 28)],
                        packed,
                    }
                } else {
                    Vertex {
                        position,
                        normal: [f(b, 12), f(b, 16), f(b, 20)],
                        binormal: [f(b, 24), f(b, 28), f(b, 32)],
                        tangent: [f(b, 36), f(b, 40), f(b, 44)],
                        uv: [f(b, 48), f(b, 52)],
                        packed: [0; 3],
                    }
                }
            })
            .collect();
        let lightmap_vertices = lightmap
            .chunks_exact(lsize)
            .map(|b| {
                if g.compressed {
                    let raw_uv = [i16::from_le_bytes([b[4], b[5]]), i16::from_le_bytes([b[6], b[7]])];
                    LightmapVertex {
                        incident_direction: unpack_vector(u(b, 0)),
                        uv: [unpack_coordinate(raw_uv[0]), unpack_coordinate(raw_uv[1])],
                        packed_direction: u(b, 0),
                        raw_uv,
                    }
                } else {
                    LightmapVertex {
                        incident_direction: [f(b, 0), f(b, 4), f(b, 8)],
                        uv: [f(b, 12), f(b, 16)],
                        packed_direction: 0,
                        raw_uv: [0; 2],
                    }
                }
            })
            .collect();
        let first = g.first_surface as usize;
        let tris = &self.triangles[first..first + g.surface_count as usize];
        let triangles = tris
            .iter()
            .map(|t| {
                for &i in t {
                    if i as usize >= n {
                        return Err(mv.error(0, format_args!("triangle index {i} past the material's {n} vertices")));
                    }
                }
                Ok([t[0] as u32, t[1] as u32, t[2] as u32])
            })
            .collect::<io::Result<Vec<_>>>()?;
        Ok(MaterialGeometry {
            vertices,
            lightmap_vertices,
            triangles,
        })
    }

    /// The material's triangles as stored (indices local to the material),
    /// without decoding any vertex.
    pub fn material_triangles(&self, m: &Material) -> &[[u16; 3]] {
        let first = m.first_surface as usize;
        &self.triangles[first..first + m.surface_count as usize]
    }

    /// Whether `Bounds` of the world is a sane box (lower <= upper).
    pub fn bounds_ok(&self) -> bool {
        let b = &self.world_bounds;
        [b.x, b.y, b.z].iter().all(|r: &Bounds<f32>| r.lower <= r.upper)
    }
}
