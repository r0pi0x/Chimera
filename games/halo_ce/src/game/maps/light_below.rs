//! The level's light under a point (specs/halo_ce/structure_bsp.md, "World
//! bounds, lighting, vehicles and sky": the game lights objects from the
//! lightmap below them; scenario.md, "Static lighting": a placed object's
//! lighting is the lightmap sample under it): `LightBelow` finds the
//! highest lit level triangle straight below a point (a grid of the
//! triangles over the ground plane) and samples the lightmap atlas there
//! (bilinear, the triangle's lightmap coordinates at the hit); `probe`
//! makes it the engine's light probe for objects, characters and view
//! models (`maps::load_in`'s light field, `objects::scenery`).
//!
//! The probe (our reading; the spec gives where the light comes from, not
//! its shape on a model): an ambient cube of the sample's colour shaped
//! by the scenery probe's sky profile (`sky::prop_light`'s cube, brighter
//! above and darker below, scaled to the sample: its mean is the sample),
//! with no directional light (the lightmap already holds the sun where it
//! reaches). A point with no lit triangle below it (over the void, above
//! the sky) gets None: the caller's fallback.
//!
//! Known shortcuts: one sample under the point (the game's object lighting
//! also has dynamic lights and the lighting block's colours, unused on
//! the disc's maps: "Default lighting" is all zero); the point is the
//! object's origin, not its bounding centre.

use bevy::math::{Vec2, Vec3, Vec3Swizzles};

use crate::map::{LightProbe, MapLightmap, MapMesh};

/// The grid's cell size (m).
const CELL: f32 = 4.0;

/// How far above a point a triangle may be and still count as below it
/// (m): a point on the floor, or an object's origin a hair under it.
const ABOVE: f32 = 0.1;

/// One lit level triangle: its corners (engine space, m) and atlas
/// coordinates.
#[derive(Clone, Copy, Debug)]
struct Tri {
    p: [Vec3; 3],
    uv: [Vec2; 3],
}

/// The lit level triangles over a ground-plane grid, and the atlas.
#[derive(Clone, Debug)]
pub struct LightBelow {
    tris: Vec<Tri>,
    origin: Vec2,
    cols: usize,
    rows: usize,
    cells: Vec<Vec<u32>>,
    atlas: MapLightmap,
}

impl LightBelow {
    /// The lit triangles of `meshes` (the level's: lightmap coordinates
    /// and not the sky) over `atlas`; None without any.
    pub fn new<'a>(meshes: impl IntoIterator<Item = &'a MapMesh>, atlas: &MapLightmap) -> Option<LightBelow> {
        let mut tris = Vec::new();
        for m in meshes
            .into_iter()
            .filter(|m| !m.skybox && m.lightmap_uvs.len() == m.positions.len() && !m.positions.is_empty())
        {
            for t in m.indices.chunks_exact(3) {
                let [a, b, c] = [t[0], t[1], t[2]].map(|i| i as usize);
                tris.push(Tri {
                    p: [a, b, c].map(|i| Vec3::from(m.positions[i])),
                    uv: [a, b, c].map(|i| Vec2::from(m.lightmap_uvs[i])),
                });
            }
        }
        if tris.is_empty() || atlas.width == 0 || atlas.height == 0 {
            return None;
        }
        let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        for t in &tris {
            for p in t.p {
                lo = lo.min(p.xz());
                hi = hi.max(p.xz());
            }
        }
        let cols = (((hi.x - lo.x) / CELL).ceil() as usize).clamp(1, 4096);
        let rows = (((hi.y - lo.y) / CELL).ceil() as usize).clamp(1, 4096);
        let mut cells = vec![Vec::new(); cols * rows];
        let cell = |v: Vec2| {
            let c = ((v - lo) / CELL).floor();
            (
                (c.x.max(0.0) as usize).min(cols - 1),
                (c.y.max(0.0) as usize).min(rows - 1),
            )
        };
        for (i, t) in tris.iter().enumerate() {
            let (a, b) = (
                t.p.iter().map(|p| p.xz()).fold(Vec2::splat(f32::MAX), Vec2::min),
                t.p.iter().map(|p| p.xz()).fold(Vec2::splat(f32::MIN), Vec2::max),
            );
            let ((x0, y0), (x1, y1)) = (cell(a), cell(b));
            for y in y0..=y1 {
                for x in x0..=x1 {
                    cells[y * cols + x].push(i as u32);
                }
            }
        }
        Some(LightBelow {
            tris,
            origin: lo,
            cols,
            rows,
            cells,
            atlas: atlas.clone(),
        })
    }

    /// The lightmap's light (linear) on the highest lit triangle straight
    /// below `p` (or within `ABOVE` over it); None with none.
    pub fn sample(&self, p: Vec3) -> Option<Vec3> {
        let c = ((p.xz() - self.origin) / CELL).floor();
        if c.x < 0.0 || c.y < 0.0 || c.x as usize >= self.cols || c.y as usize >= self.rows {
            return None;
        }
        let mut best: Option<(f32, Vec2)> = None;
        for &i in &self.cells[c.y as usize * self.cols + c.x as usize] {
            let t = &self.tris[i as usize];
            let Some(w) = barycentric(p.xz(), t.p.map(|q| q.xz())) else {
                continue;
            };
            let y = w[0] * t.p[0].y + w[1] * t.p[1].y + w[2] * t.p[2].y;
            if y > p.y + ABOVE || best.is_some_and(|(b, _)| b >= y) {
                continue;
            }
            best = Some((y, t.uv[0] * w[0] + t.uv[1] * w[1] + t.uv[2] * w[2]));
        }
        best.map(|(_, uv)| self.texel(uv))
    }

    /// The atlas bilinearly at `uv` (texel centres at (i + 1/2) / size).
    fn texel(&self, uv: Vec2) -> Vec3 {
        let (w, h) = (self.atlas.width as f32, self.atlas.height as f32);
        let x = (uv.x * w - 0.5).clamp(0.0, w - 1.0);
        let y = (uv.y * h - 0.5).clamp(0.0, h - 1.0);
        let (x0, y0) = (x.floor() as u32, y.floor() as u32);
        let (x1, y1) = ((x0 + 1).min(self.atlas.width - 1), (y0 + 1).min(self.atlas.height - 1));
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let at = |x: u32, y: u32| Vec3::from(self.atlas.rgb[(y * self.atlas.width + x) as usize]);
        let top = at(x0, y0).lerp(at(x1, y0), fx);
        let bottom = at(x0, y1).lerp(at(x1, y1), fx);
        top.lerp(bottom, fy)
    }

    /// The probe at `p` (module doc) shaped by `profile`'s cube; None with
    /// no lit triangle below.
    pub fn probe(&self, p: Vec3, profile: &LightProbe) -> Option<LightProbe> {
        Some(shaped(self.sample(p)?, profile))
    }
}

/// An ambient cube of colour `light`, shaped by `profile`'s cube (its
/// mean scaled to `light`), with no directional light.
pub fn shaped(light: Vec3, profile: &LightProbe) -> LightProbe {
    let mean = profile.cube.iter().copied().sum::<Vec3>() / 6.0;
    let cube = profile.cube.map(|c| {
        let k = Vec3::new(
            if mean.x > 0.0 { c.x / mean.x } else { 1.0 },
            if mean.y > 0.0 { c.y / mean.y } else { 1.0 },
            if mean.z > 0.0 { c.z / mean.z } else { 1.0 },
        );
        light * k
    });
    LightProbe {
        cube,
        lights: Vec::new(),
    }
}

/// `p`'s weights in triangle `t` (2D), None outside it or for a triangle
/// seen edge-on from above (a wall).
fn barycentric(p: Vec2, t: [Vec2; 3]) -> Option<[f32; 3]> {
    let (a, b, c) = (t[0], t[1], t[2]);
    let d = (b - a).perp_dot(c - a);
    if d.abs() < 1e-6 {
        return None;
    }
    let u = (b - p).perp_dot(c - p) / d;
    let v = (c - p).perp_dot(a - p) / d;
    let w = 1.0 - u - v;
    let e = -1e-5;
    (u >= e && v >= e && w >= e).then_some([u, v, w])
}
