//! Lighting data for things that aren't lightmapped (props, later
//! characters), per the public BSP v20 description: per-leaf ambient light
//! cubes (lumps 52/56, HDR 51/55; one cube per leaf in older maps) and the compiled light list ("world
//! lights", lump 15). All results are in engine axes and lightmap units
//! (1.0 shows a texture at its own brightness).

use bevy::prelude::*;
use vbsp::Bsp;

use super::{METERS_PER_UNIT, to_engine};

pub fn lump(bytes: &[u8], index: usize) -> &[u8] {
    let at = 8 + index * 16;
    let Some(entry) = bytes.get(at..at + 8) else { return &[] };
    let ofs = i32::from_le_bytes(entry[0..4].try_into().unwrap()).max(0) as usize;
    let len = i32::from_le_bytes(entry[4..8].try_into().unwrap()).max(0) as usize;
    bytes.get(ofs..ofs + len).unwrap_or(&[])
}

/// A BSP leaf as stored (public BSP v20 description, lump 10). vbsp's own
/// leaf list is re-sorted by cluster, so its indices don't match the tree's.
#[derive(Clone, Copy, Debug)]
pub struct RawLeaf {
    pub contents: i32,
    /// Visibility cluster; -1 for solid leaves and leaves in none.
    pub cluster: i16,
    /// Leaf flags: 0x01 sees the 3D sky, 0x04 sees the 2D sky.
    pub flags: u8,
    /// Area (areaportals divide the map into areas; 0: none).
    pub area: u16,
    pub mins: [i16; 3],
    pub maxs: [i16; 3],
    /// Its brushes: a range of the leaf-brush lump.
    pub first_leaf_brush: u16,
    pub leaf_brush_count: u16,
    /// Version 0 leaves (BSP v19 maps) carry their own ambient light cube.
    pub ambient: Option<AmbientCube>,
}

pub const CONTENTS_SOLID: i32 = 0x1;
pub const LEAF_SKY: u8 = 0x01;
pub const LEAF_SKY2D: u8 = 0x04;

/// The world's leaves in tree order.
pub fn raw_leaves(bytes: &[u8]) -> Vec<RawLeaf> {
    const LUMP_LEAFS: usize = 10;
    let version = bytes
        .get(8 + LUMP_LEAFS * 16 + 8..8 + LUMP_LEAFS * 16 + 12)
        .map_or(1, |v| i32::from_le_bytes(v.try_into().unwrap()));
    // Version 0 leaves carry an ambient light cube (24 bytes) before the padding.
    let size = if version == 0 { 56 } else { 32 };
    let i16_at = |b: &[u8], at: usize| i16::from_le_bytes([b[at], b[at + 1]]);
    lump(bytes, LUMP_LEAFS)
        .chunks_exact(size)
        .map(|b| RawLeaf {
            contents: i32_at(b, 0),
            cluster: i16_at(b, 4),
            // Area in the low 9 bits, flags in the high 7.
            flags: (u16::from_le_bytes([b[6], b[7]]) >> 9) as u8,
            area: u16::from_le_bytes([b[6], b[7]]) & 0x1ff,
            mins: [i16_at(b, 8), i16_at(b, 10), i16_at(b, 12)],
            maxs: [i16_at(b, 14), i16_at(b, 16), i16_at(b, 18)],
            first_leaf_brush: u16::from_le_bytes([b[24], b[25]]),
            leaf_brush_count: u16::from_le_bytes([b[26], b[27]]),
            ambient: (version == 0).then(|| compressed_cube(&b[30..54])),
        })
        .collect()
}

fn f32_at(b: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

fn i32_at(b: &[u8], at: usize) -> i32 {
    i32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

/// Ambient cube samples use a 0-255 scale (unlike lightmaps, which decode
/// with a further /255): fitted on de_dust2, where cube values match
/// lightmaps on surfaces facing away from the sun at a ratio of ~221.
fn rgbe(b: &[u8]) -> Vec3 {
    let scale = 2f32.powi(b[3] as i8 as i32);
    Vec3::new(b[0] as f32, b[1] as f32, b[2] as f32) * scale
}

/// A stored light cube (six RGBE colours, Source order +X -X +Y -Y +Z -Z)
/// in engine axes: X, Z up, -Y.
fn compressed_cube(b: &[u8]) -> AmbientCube {
    let c: Vec<Vec3> = b[..24].as_chunks::<4>().0.iter().map(|c| rgbe(c)).collect();
    AmbientCube([c[0], c[1], c[4], c[5], c[3], c[2]])
}

fn src(x: f32, y: f32, z: f32) -> Vec3 {
    to_engine(vbsp::Vector { x, y, z }) / METERS_PER_UNIT
}

/// Light arriving from six directions: +X, -X, +Y, -Y, +Z, -Z (engine axes).
#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct AmbientCube(pub [Vec3; 6]);

impl AmbientCube {
    /// Light on a surface with normal `n`: faces weighted by n squared.
    pub fn eval(&self, n: Vec3) -> Vec3 {
        let c = &self.0;
        let pick = |v: f32, pos: Vec3, neg: Vec3| if v >= 0.0 { pos } else { neg };
        pick(n.x, c[0], c[1]) * n.x * n.x + pick(n.y, c[2], c[3]) * n.y * n.y + pick(n.z, c[4], c[5]) * n.z * n.z
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Sample {
    position: Vec3,
    cube: AmbientCube,
}

/// A compiled light, already converted to engine space.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum WorldLight {
    /// The sun: light travels along `direction`.
    Sky { direction: Vec3, intensity: Vec3 },
    Point {
        position: Vec3,
        intensity: Vec3,
        atten: [f32; 3],
        radius: f32,
    },
    Spot {
        position: Vec3,
        direction: Vec3,
        intensity: Vec3,
        atten: [f32; 3],
        radius: f32,
        stopdot: f32,
        stopdot2: f32,
        exponent: f32,
    },
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct MapLighting {
    leaves: Vec<Vec<Sample>>,
    /// Lights not already folded into the ambient cubes.
    pub lights: Vec<WorldLight>,
    /// No lighting at all (`fullbright`): full light from every side.
    fullbright: bool,
}

/// Set on lights VRAD already added to the leaf ambient cubes.
const DWL_FLAGS_INAMBIENTCUBE: i32 = 1;

impl MapLighting {
    /// The LDR lighting (falling back to HDR where a map has only that).
    pub fn read(bytes: &[u8]) -> Self {
        Self::read_level(bytes, false)
    }

    /// With `hdr`, the HDR ambient cubes (51/55) and world lights (54)
    /// first (mat_hdr_level 2), falling back to LDR where a map lacks them.
    pub fn read_level(bytes: &[u8], hdr: bool) -> Self {
        // (index, samples, lone cubes, world lights), in order of preference.
        let (ldr, hdr_lumps) = ((52, 56, 15), (51, 55, 54));
        let (first, second) = if hdr { (hdr_lumps, ldr) } else { (ldr, hdr_lumps) };
        let (mut index, mut samples) = (lump(bytes, first.0), lump(bytes, first.1));
        if index.is_empty() || samples.is_empty() {
            (index, samples) = (lump(bytes, second.0), lump(bytes, second.1));
        }
        let raw = raw_leaves(bytes);
        let mut leaves = Vec::new();
        // Older maps have one cube per leaf instead of sample lists: inside
        // version 0 leaves (BSP v19: de_aztec, cs_office), or as the
        // ambient lump without an index (early v20: de_nuke, de_train).
        if index.is_empty() {
            let mut lone = lump(bytes, first.1);
            if lone.is_empty() {
                lone = lump(bytes, second.1);
            }
            let cubes: Vec<AmbientCube> = if raw.iter().all(|l| l.ambient.is_some()) {
                raw.iter().filter_map(|l| l.ambient).collect()
            } else if lone.len() == raw.len() * 24 {
                lone.as_chunks::<24>().0.iter().map(|c| compressed_cube(c)).collect()
            } else {
                Vec::new()
            };
            for (leaf, cube) in raw.iter().zip(cubes) {
                let centre = (Vec3::from(leaf.mins.map(f32::from)) + Vec3::from(leaf.maxs.map(f32::from))) / 2.0;
                leaves.push(vec![Sample {
                    position: to_engine(vbsp::Vector {
                        x: centre.x,
                        y: centre.y,
                        z: centre.z,
                    }),
                    cube,
                }]);
            }
        }
        // Each leaf's first sample is a u16: past 65535 samples it wraps
        // (surf_sedona: 82293). Leaves list their samples in order, so a
        // first that goes backwards has wrapped.
        let mut wrapped = 0;
        let mut last_first = 0;
        for (i, entry) in index.as_chunks::<4>().0.iter().enumerate() {
            let count = u16::from_le_bytes([entry[0], entry[1]]) as usize;
            let mut first = u16::from_le_bytes([entry[2], entry[3]]) as usize + wrapped;
            if count > 0 {
                if first < last_first {
                    wrapped += 0x1_0000;
                    first += 0x1_0000;
                }
                last_first = first;
            }
            let mut list = Vec::new();
            if let Some(leaf) = raw.get(i) {
                let lo = Vec3::new(leaf.mins[0] as f32, leaf.mins[1] as f32, leaf.mins[2] as f32);
                let hi = Vec3::new(leaf.maxs[0] as f32, leaf.maxs[1] as f32, leaf.maxs[2] as f32);
                for s in samples.as_chunks::<28>().0.iter().skip(first).take(count) {
                    let frac = Vec3::new(s[24] as f32, s[25] as f32, s[26] as f32) / 255.0;
                    let p = lo + (hi - lo) * frac;
                    // Source order +X -X +Y -Y +Z -Z; engine: X, Z up, -Y.
                    list.push(Sample {
                        position: to_engine(vbsp::Vector { x: p.x, y: p.y, z: p.z }),
                        cube: compressed_cube(s),
                    });
                }
            }
            leaves.push(list);
        }

        let mut lights = Vec::new();
        let mut wl = lump(bytes, first.2);
        if wl.is_empty() {
            wl = lump(bytes, second.2);
        }
        for l in wl.as_chunks::<88>().0 {
            let v = |at: usize| src(f32_at(l, at), f32_at(l, at + 4), f32_at(l, at + 8));
            let rgb = |at: usize| Vec3::new(f32_at(l, at), f32_at(l, at + 4), f32_at(l, at + 8));
            if i32_at(l, 76) & DWL_FLAGS_INAMBIENTCUBE != 0 {
                continue;
            }
            let position = v(0) * METERS_PER_UNIT;
            // Already in lightmap units: dust2's sun (2.25) matches sunlit
            // floor lightmap values (~2.8 including ambient).
            let intensity = rgb(12);
            let direction = v(24).normalize_or_zero();
            let atten = [f32_at(l, 64), f32_at(l, 68), f32_at(l, 72)];
            let radius = f32_at(l, 60);
            match i32_at(l, 40) {
                1 => lights.push(WorldLight::Point {
                    position,
                    intensity,
                    atten,
                    radius,
                }),
                2 => lights.push(WorldLight::Spot {
                    position,
                    direction,
                    intensity,
                    atten,
                    radius,
                    stopdot: f32_at(l, 48),
                    stopdot2: f32_at(l, 52),
                    exponent: f32_at(l, 56),
                }),
                3 => lights.push(WorldLight::Sky { direction, intensity }),
                _ => {}
            }
        }
        Self {
            leaves,
            lights,
            fullbright: false,
        }
    }

    /// A map without lighting (`lightmap::fullbright`), drawn fullbright
    /// as CS:S draws it: props and models at full light, not black from
    /// empty (or all-zero) ambient samples.
    pub fn fullbright() -> Self {
        Self {
            leaves: Vec::new(),
            lights: Vec::new(),
            fullbright: true,
        }
    }

    /// The ambient cube at `p` (engine space): samples in the containing
    /// leaf, weighted by inverse squared distance.
    pub fn ambient_at(&self, bsp: &Bsp, p: Vec3) -> AmbientCube {
        let s = p / METERS_PER_UNIT;
        let point = vbsp::Vector {
            x: s.x,
            y: -s.z,
            z: s.y,
        };
        self.ambient_in(leaf_index(bsp, point), p)
    }

    /// The ambient cube at `p` (engine space) in BSP leaf `leaf`.
    pub fn ambient_in(&self, leaf: Option<usize>, p: Vec3) -> AmbientCube {
        if self.fullbright {
            return AmbientCube([Vec3::ONE; 6]);
        }
        let Some(samples) = leaf.and_then(|i| self.leaves.get(i)) else {
            return AmbientCube::default();
        };
        let mut sum = [Vec3::ZERO; 6];
        let mut total = 0.0;
        for sample in samples {
            let w = 1.0 / (sample.position.distance_squared(p) + 0.01);
            for (acc, c) in sum.iter_mut().zip(sample.cube.0) {
                *acc += c * w;
            }
            total += w;
        }
        if total > 0.0 {
            sum.iter_mut().for_each(|c| *c /= total);
        }
        AmbientCube(sum)
    }
}

/// The leaf containing a Source-space point, walking the world's BSP tree.
pub fn leaf_index(bsp: &Bsp, p: vbsp::Vector) -> Option<usize> {
    let mut node = bsp.models.first()?.head_node;
    while node >= 0 {
        let n = bsp.nodes.get(node as usize)?;
        let plane = bsp.plane(n.plane_index as usize)?;
        let side = plane.normal.x * p.x + plane.normal.y * p.y + plane.normal.z * p.z - plane.dist;
        node = if side >= 0.0 { n.children[0] } else { n.children[1] };
    }
    Some((-node - 1) as usize)
}

/// Direct light from `light` on a surface at `p` with normal `n`, without
/// the shadow test. Returns the light and, for shadow tests, the direction
/// toward the light and how far it is (meters; infinite for the sun).
pub fn direct(light: &WorldLight, p: Vec3, n: Vec3) -> Option<(Vec3, Vec3, f32)> {
    let atten = |a: [f32; 3], d_m: f32| {
        let d = d_m / METERS_PER_UNIT;
        let f = a[0] + a[1] * d + a[2] * d * d;
        if f > 0.0 { 1.0 / f } else { 1.0 }
    };
    match light {
        WorldLight::Sky { direction, intensity } => {
            let to = -*direction;
            let dot = n.dot(to);
            (dot > 0.0).then(|| (*intensity * dot, to, f32::INFINITY))
        }
        WorldLight::Point {
            position,
            intensity,
            atten: a,
            radius,
        } => {
            let d = position.distance(p);
            let to = (*position - p) / d.max(1e-4);
            let dot = n.dot(to);
            if dot <= 0.0 || (*radius > 0.0 && d / METERS_PER_UNIT > *radius) {
                return None;
            }
            Some((*intensity * dot * atten(*a, d), to, d))
        }
        WorldLight::Spot {
            position,
            direction,
            intensity,
            atten: a,
            radius,
            stopdot,
            stopdot2,
            exponent,
        } => {
            let d = position.distance(p);
            let to = (*position - p) / d.max(1e-4);
            let dot = n.dot(to);
            if dot <= 0.0 || (*radius > 0.0 && d / METERS_PER_UNIT > *radius) {
                return None;
            }
            let dot2 = -to.dot(*direction);
            if dot2 <= *stopdot2 {
                return None;
            }
            let mut cone = if *exponent > 0.0 && *exponent != 1.0 {
                dot2.powf(*exponent)
            } else {
                dot2
            };
            if dot2 < *stopdot {
                cone *= (dot2 - stopdot2) / (stopdot - stopdot2);
            }
            Some((*intensity * dot * cone * atten(*a, d), to, d))
        }
    }
}

/// Geometry that blocks light, for shadow rays.
pub struct Occluders {
    /// Convex hulls (one compound).
    colliders: Vec<avian3d::prelude::Collider>,
    /// Terrain triangles (corners, front normal, bounds). One-sided: they
    /// block only rays meeting their front, like the game's displacement
    /// collision (a point under terrain still sees the sun through it;
    /// de_nuke's dumpsters sit with their centres below the ground).
    terrain: Vec<([Vec3; 3], Vec3, Vec3, Vec3)>,
    /// The terrain triangles by the ground cells (engine x, z) their
    /// bounds cover: a ray tests only those of the cells it crosses (a
    /// map with 261k terrain triangles and 935 lights took 30 s and more
    /// to light its props when every ray tried them all).
    grid: TerrainGrid,
    /// The sky's brushes (one compound): a ray toward the sun ends at the
    /// first one it meets (`sky_distance`).
    sky: Option<avian3d::prelude::Collider>,
}

/// Triangles binned on a horizontal grid (engine x and z).
#[derive(Default)]
struct TerrainGrid {
    origin: Vec2,
    cell: f32,
    size: (usize, usize),
    cells: Vec<Vec<u32>>,
}

/// Most cells along each side of a terrain grid.
const TERRAIN_GRID_MAX: usize = 512;

impl TerrainGrid {
    fn new(terrain: &[([Vec3; 3], Vec3, Vec3, Vec3)]) -> Self {
        if terrain.is_empty() {
            return Self::default();
        }
        let lo = terrain.iter().fold(Vec2::MAX, |a, t| a.min(t.2.xz()));
        let hi = terrain.iter().fold(Vec2::MIN, |a, t| a.max(t.3.xz()));
        let extent = (hi - lo).max(Vec2::splat(1e-3));
        // About 128 units a cell, fewer cells on a huge map.
        let cell = (128.0 * METERS_PER_UNIT).max(extent.max_element() / TERRAIN_GRID_MAX as f32);
        let size = (
            ((extent.x / cell).floor() as usize + 1).min(TERRAIN_GRID_MAX),
            ((extent.y / cell).floor() as usize + 1).min(TERRAIN_GRID_MAX),
        );
        let mut cells = vec![Vec::new(); size.0 * size.1];
        let index = |v: f32, o: f32, n: usize| (((v - o) / cell).floor().max(0.0) as usize).min(n - 1);
        for (i, t) in terrain.iter().enumerate() {
            let (x0, x1) = (index(t.2.x, lo.x, size.0), index(t.3.x, lo.x, size.0));
            let (z0, z1) = (index(t.2.z, lo.y, size.1), index(t.3.z, lo.y, size.1));
            for z in z0..=z1 {
                for x in x0..=x1 {
                    cells[z * size.0 + x].push(i as u32);
                }
            }
        }
        Self {
            origin: lo,
            cell,
            size,
            cells,
        }
    }

    /// The triangles of each cell a ray from `p` along `dir` crosses
    /// within `max` (a triangle can come up more than once), until `hit`
    /// says yes.
    fn any_along(&self, p: Vec3, dir: Vec3, max: f32, mut hit: impl FnMut(u32) -> bool) -> bool {
        if self.cells.is_empty() {
            return false;
        }
        let (a, d) = (p.xz(), dir.xz());
        let hi = self.origin + Vec2::new(self.size.0 as f32, self.size.1 as f32) * self.cell;
        // The part of the ray over the grid.
        let (mut t0, mut t1) = (0.0f32, max);
        for k in 0..2 {
            if d[k].abs() < 1e-9 {
                if a[k] < self.origin[k] || a[k] > hi[k] {
                    return false;
                }
                continue;
            }
            let (u, v) = ((self.origin[k] - a[k]) / d[k], (hi[k] - a[k]) / d[k]);
            t0 = t0.max(u.min(v));
            t1 = t1.min(u.max(v));
        }
        if t0 > t1 {
            return false;
        }
        let start = a + d * t0;
        let n = [self.size.0 as i64, self.size.1 as i64];
        let mut c = [0i64; 2];
        let (mut step, mut next, mut delta) = ([0i64; 2], [f32::INFINITY; 2], [f32::INFINITY; 2]);
        for k in 0..2 {
            c[k] = (((start[k] - self.origin[k]) / self.cell).floor() as i64).clamp(0, n[k] - 1);
            if d[k] > 1e-9 {
                step[k] = 1;
                next[k] = (self.origin[k] + (c[k] + 1) as f32 * self.cell - a[k]) / d[k];
                delta[k] = self.cell / d[k];
            } else if d[k] < -1e-9 {
                step[k] = -1;
                next[k] = (self.origin[k] + c[k] as f32 * self.cell - a[k]) / d[k];
                delta[k] = -self.cell / d[k];
            }
        }
        loop {
            let cell = &self.cells[c[1] as usize * self.size.0 + c[0] as usize];
            if cell.iter().any(|i| hit(*i)) {
                return true;
            }
            let k = if next[0] < next[1] { 0 } else { 1 };
            if next[k] > t1 || step[k] == 0 {
                return false;
            }
            c[k] += step[k];
            if c[k] < 0 || c[k] >= n[k] {
                return false;
            }
            next[k] += delta[k];
        }
    }
}

impl Occluders {
    pub fn new(hulls: &[Vec<[f32; 3]>], triangles: (&[[f32; 3]], &[[u32; 3]])) -> Self {
        use avian3d::prelude::Collider;
        let parts: Vec<_> = hulls
            .iter()
            .filter_map(|h| Collider::convex_hull(&h.iter().map(|p| Vec3::from(*p)).collect::<Vec<_>>()))
            .map(|c| (Vec3::ZERO, Quat::IDENTITY, c))
            .collect();
        let colliders = if parts.is_empty() { Vec::new() } else { vec![Collider::compound(parts)] };
        let terrain: Vec<_> = triangles
            .1
            .iter()
            .filter_map(|t| {
                let [a, b, c] = t.map(|i| Vec3::from(triangles.0[i as usize]));
                let n = (b - a).cross(c - a).normalize_or_zero();
                (n != Vec3::ZERO).then(|| ([a, b, c], n, a.min(b).min(c), a.max(b).max(c)))
            })
            .collect();
        let grid = TerrainGrid::new(&terrain);
        Self {
            colliders,
            terrain,
            grid,
            sky: None,
        }
    }

    /// With the sky's brushes (`bsp::sky_hulls`), where sun rays end.
    pub fn with_sky(mut self, hulls: &[Vec<[f32; 3]>]) -> Self {
        use avian3d::prelude::Collider;
        let parts: Vec<_> = hulls
            .iter()
            .filter_map(|h| Collider::convex_hull(&h.iter().map(|p| Vec3::from(*p)).collect::<Vec<_>>()))
            .map(|c| (Vec3::ZERO, Quat::IDENTITY, c))
            .collect();
        self.sky = (!parts.is_empty()).then(|| Collider::compound(parts));
        self
    }

    /// How far a ray from `p` along `dir` goes before it meets the sky
    /// (meters), if it does within the map's extent. The sun lights what
    /// sees the sky toward it: a ray from the 3D skybox ends at the
    /// skybox's own sky, not at the map far above it (de_nuke's cooling
    /// towers were unlit, their sun ray blocked by the map's underside).
    pub fn sky_distance(&self, p: Vec3, dir: Vec3) -> Option<f32> {
        self.sky
            .as_ref()?
            .cast_ray(Vec3::ZERO, Quat::IDENTITY, p, dir, 500.0, false)
            .map(|(t, _)| t)
    }

    /// Whether anything blocks the segment from `p` along `dir` for `max`
    /// meters (infinite: up to the map's extent).
    pub fn blocked(&self, p: Vec3, dir: Vec3, max: f32) -> bool {
        let max = if max.is_finite() { max } else { 500.0 };
        if self
            .colliders
            .iter()
            .any(|c| c.cast_ray(Vec3::ZERO, Quat::IDENTITY, p, dir, max, false).is_some())
        {
            return true;
        }
        let end = p + dir * max;
        let (lo, hi) = (p.min(end), p.max(end));
        self.grid.any_along(p, dir, max, |i| {
            let ([a, b, c], n, tlo, thi) = &self.terrain[i as usize];
            n.dot(dir) < 0.0 && tlo.cmple(hi).all() && thi.cmpge(lo).all() && ray_triangle(p, dir, *a, *b, *c).is_some_and(|t| t > 0.0 && t < max)
        })
    }
}

impl MapLighting {
    /// Total light on a surface at `p` with normal `n`: ambient cube plus
    /// shadow-tested direct light.
    pub fn light_at(&self, bsp: &Bsp, occluders: &Occluders, p: Vec3, n: Vec3) -> Vec3 {
        let p = p + n * 0.02;
        let mut light = self.ambient_at(bsp, p).eval(n);
        for l in &self.lights {
            if let Some((value, to, dist)) = direct(l, p, n)
                && value.max_element() > 0.002
                && !occluders.blocked(p, to, dist)
            {
                light += value;
            }
        }
        light
    }
}

/// Distance along `dir` from `p` to triangle (a, b, c), either side
/// (Moller-Trumbore).
fn ray_triangle(p: Vec3, dir: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let (e1, e2) = (b - a, c - a);
    let h = dir.cross(e2);
    let det = e1.dot(h);
    if det.abs() < 1e-9 {
        return None;
    }
    let inv = 1.0 / det;
    let s = p - a;
    let u = inv * s.dot(h);
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = inv * dir.dot(q);
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    Some(inv * e2.dot(q))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrain_blocks_light_from_its_front_only() {
        // A floor triangle at y = 1 facing up (counter-clockwise from above).
        let positions = [[-10.0, 1.0, 10.0], [10.0, 1.0, 10.0], [0.0, 1.0, -10.0]];
        let o = Occluders::new(&[], (&positions, &[[0, 1, 2]]));
        // From above, looking down through it: blocked.
        assert!(o.blocked(Vec3::new(0.0, 2.0, 0.0), Vec3::NEG_Y, 5.0));
        // From below, toward the sky: passes (one-sided, as in the game).
        assert!(!o.blocked(Vec3::ZERO, Vec3::Y, 5.0));
        // Too short to reach it.
        assert!(!o.blocked(Vec3::new(0.0, 2.0, 0.0), Vec3::NEG_Y, 0.5));
    }

    #[test]
    fn terrain_grid_finds_what_trying_every_triangle_finds() {
        // A bumpy terrain 200 m across in small triangles, and rays every
        // way: the grid's answer is the brute force's.
        let mut positions = Vec::new();
        let mut triangles = Vec::new();
        let n = 40u32;
        for z in 0..=n {
            for x in 0..=n {
                let (fx, fz) = (x as f32 * 5.0 - 100.0, z as f32 * 5.0 - 100.0);
                positions.push([fx, (fx * 0.1).sin() * 3.0 + (fz * 0.07).cos() * 4.0, fz]);
            }
        }
        for z in 0..n {
            for x in 0..n {
                let i = z * (n + 1) + x;
                triangles.push([i, i + n + 1, i + 1]);
                triangles.push([i + 1, i + n + 1, i + n + 2]);
            }
        }
        let o = Occluders::new(&[], (&positions, &triangles));
        let brute = |p: Vec3, dir: Vec3, max: f32| {
            o.terrain.iter().any(|([a, b, c], n, _, _)| {
                n.dot(dir) < 0.0 && ray_triangle(p, dir, *a, *b, *c).is_some_and(|t| t > 0.0 && t < max)
            })
        };
        let mut seed = 7u32;
        let mut r = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed as f32 / u32::MAX as f32) * 2.0 - 1.0
        };
        let (mut hits, mut misses) = (0, 0);
        for _ in 0..3000 {
            let p = Vec3::new(r() * 120.0, r() * 10.0 + 6.0, r() * 120.0);
            let dir = Vec3::new(r(), r() - 0.3, r()).normalize_or(Vec3::NEG_Y);
            let max = (r() + 1.0) * 150.0;
            let want = brute(p, dir, max);
            assert_eq!(o.blocked(p, dir, max), want, "{p} {dir} {max}");
            if want { hits += 1 } else { misses += 1 }
        }
        assert!(hits > 100 && misses > 100, "{hits} {misses}");
    }
}
