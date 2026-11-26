//! The light a studio model gets (specs/goldsrc/mdl.md, "Lighting"):
//! one light for the whole model from the world at its origin, then each
//! vertex shaded from its normal through the lightmaps' curve without the
//! gain (`map::LightCurve::vertex`). `field` is the map's light field
//! (`MapData::light_field`, the light at a point for models that move):
//! it works the model's light out at the point and hands the renderer its
//! six axis-facing vertex lights as the probe's ambient cube, which the
//! renderer blends by the squares of a normal's components.
//!
//! 1. The sample point is the origin raised 8 units. When the map's sky
//!    colour (`light_environment`'s `_light`, `sky_light`) is not black
//!    and a trace from the point toward the sun reaches a sky face first,
//!    the light is the sky colour from the sun's direction.
//! 2. Else the floor: a trace 2048 units down finds the first lit face,
//!    whose lightmap sample nearest the hit (styles at their map-start
//!    letters, summed, / 256, capped at 255) is the light's colour; four
//!    more floor samples at (+-16, +-16) tilt the direction toward the
//!    brighter side.
//! 3. Total T = the brightest channel, shade = 0.9 T, ambient = 0.1 T,
//!    capped (ambient 128, ambient + shade 255); colour = C / its
//!    brightest channel.
//! 4. A vertex's illum = ambient + shade - shade max(0, (cos + w - 1) / w)
//!    with cos = n . l and w = 1.4953; its light is the curve of
//!    floor(4 illum), times the colour.
//!
//! Known shortcuts:
//! - Dynamic lights add nothing to a model's light and no entity is drawn
//!   fullbright or lit from the ceiling.
//! - The floor sample follows the light styles' letters at map start, not
//!   as they play (a flickering or switched light changes the map, not a
//!   model standing under it); the sample point is the one the renderer
//!   asks for (the model's origin, or what it sets as its centre), not
//!   each model's own origin.
//! - The renderer shades by the probe's cube (six axis directions, blended
//!   by squared components), not per vertex: exact for a normal along an
//!   axis and a smooth blend between, where the original's lambert
//!   (`w` 1.4953, open question 4 in mdl.md) is a ramp of the cosine.
//! - The light environment's pitch key is taken as the spec words it
//!   ("-60" points 60 degrees down): the angle's own pitch is replaced by
//!   its negation; `sky_light` of other kinds is not read.
//! - The sun's angles are the first `light_environment`'s; a map with
//!   more than one takes the first.

use std::sync::Arc;

use bevy::math::Vec3;
use goldsrc_formats::bsp::{Bsp, Face};

use super::{parse_vector, to_game};
use crate::map::{LightCurve, LightProbe, MapLightField};

/// The shade's share of the total (the `direct` setting).
const DIRECT: f32 = 0.9;
/// The lambert weight (mdl.md: 1.4953, shown as 1.5).
const LAMBERT: f32 = 1.4953;
/// How far the floor trace goes, units.
const FLOOR_REACH: f64 = 2048.0;
/// How far the sky trace goes, units.
const SKY_REACH: f64 = 8192.0;

/// The sky light: the colour (channels may exceed 255) and the direction
/// the light travels (downward), in GoldSrc's axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sky {
    pub color: [f32; 3],
    pub dir: [f32; 3],
}

/// The sky light a map's `light_environment` sets (mdl.md, "Lighting":
/// the sky colour and direction): `_light` "r g b v" (one number: grey),
/// each channel times v / 255 rounded down, then floor(264 (c / 114)^0.6).
pub fn sky_of(bsp: &Bsp) -> Option<Sky> {
    let ents = bsp.entities();
    let e = ents.iter().find(|e| e.classname() == "light_environment")?;
    let nums: Vec<f32> = e.get("_light")?.split_whitespace().filter_map(|w| w.parse().ok()).collect();
    let rgb: [f32; 3] = match nums.len() {
        0 => return None,
        1 => [nums[0]; 3],
        _ => {
            let k = nums.get(3).copied().unwrap_or(255.0) / 255.0;
            [0, 1, 2].map(|i| (nums[i.min(nums.len() - 1)] * k).floor())
        }
    };
    let color = rgb.map(|c| (264.0 * (c / 114.0).powf(0.6)).floor());
    // The sun: the entity's angles, the pitch key replacing the pitch
    // (module doc, shortcuts).
    let mut angles = e.get("angles").map(parse_vector).unwrap_or([0.0; 3]);
    if let Some(y) = e.get("angle").and_then(|a| a.trim().parse::<f32>().ok()).filter(|_| e.get("angles").is_none()) {
        angles = [0.0, y, 0.0];
    }
    if let Some(p) = e.get("pitch").and_then(|p| p.trim().parse::<f32>().ok()) {
        angles[0] = -p;
    }
    let (p, y) = (angles[0].to_radians(), angles[1].to_radians());
    let dir = [p.cos() * y.cos(), p.cos() * y.sin(), -p.sin()];
    Some(Sky { color, dir })
}

/// A model's light (mdl.md, "Lighting", steps 4 and 5): the colour
/// (brightest channel 1), the ambient and the shade in 0-255 units, and
/// the direction the light travels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lit {
    pub color: [f32; 3],
    pub ambient: f32,
    pub shade: f32,
    pub dir: Vec3,
}

impl Lit {
    /// The light of colour `c` (0-255, may exceed) travelling along `dir`.
    pub fn new(c: [f32; 3], dir: Vec3) -> Lit {
        let t = c.iter().cloned().fold(0.0, f32::max);
        let (t, color) = if t <= 0.0 { (1.0, [1.0; 3]) } else { (t, c.map(|v| v / t)) };
        let shade = t * DIRECT;
        let ambient = (t - shade).min(128.0);
        Lit { color, ambient, shade: shade.min(255.0 - ambient), dir: dir.normalize_or(Vec3::NEG_Z) }
    }

    /// The illum of a vertex with unit normal `n` (game axes).
    pub fn illum(&self, n: Vec3) -> f32 {
        let cos = n.dot(self.dir);
        let ramp = ((cos + LAMBERT - 1.0) / LAMBERT).max(0.0);
        (self.ambient + self.shade - self.shade * ramp).clamp(0.0, 255.0)
    }

    /// The light a vertex with normal `n` shows per channel, 0 to 1.
    pub fn vertex(&self, curve: &LightCurve, n: Vec3) -> Vec3 {
        Vec3::from(self.color) * curve.vertex(self.illum(n))
    }

    /// The six axis normals' lights as a probe's ambient cube: engine
    /// axes +X, -X, +Y, -Y, +Z, -Z, each looked up as the game's normal.
    pub fn probe(&self, curve: &LightCurve) -> LightProbe {
        // Engine (x, y, z) is the game's (x, z, -y): game = (x, -z, y).
        let axes = [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z];
        let cube = axes.map(|e| self.vertex(curve, Vec3::new(e.x, -e.z, e.y)));
        LightProbe { cube, lights: Vec::new() }
    }
}

/// A face of the world as the traces find it.
struct FaceBox {
    index: usize,
    mins: [f32; 3],
    maxs: [f32; 3],
}

/// The world's faces for the model light's traces.
pub struct World {
    bsp: Bsp,
    faces: Vec<FaceBox>,
    sky: Option<Sky>,
    /// Texture indices whose name is `sky`.
    sky_textures: Vec<i32>,
    /// Each style's letter value at map start (264 = `m`).
    style_value: Box<dyn Fn(u8) -> f32 + Send + Sync>,
}

impl World {
    /// The world of `bsp`; `style_value(style)` its letter value now (264
    /// for `m`, 0 dark).
    pub fn new(bsp: Bsp, style_value: impl Fn(u8) -> f32 + Send + Sync + 'static) -> World {
        let model = bsp.models.first().copied();
        let (first, count) = model.map_or((0, 0), |m| (m.first_face.max(0) as usize, m.face_count.max(0) as usize));
        let faces = (first..(first + count).min(bsp.faces.len()))
            .map(|index| {
                let v = bsp.face_vertices(&bsp.faces[index]);
                let mut mins = [f32::MAX; 3];
                let mut maxs = [f32::MIN; 3];
                for p in &v {
                    for k in 0..3 {
                        mins[k] = mins[k].min(p[k]);
                        maxs[k] = maxs[k].max(p[k]);
                    }
                }
                FaceBox { index, mins, maxs }
            })
            .collect();
        let sky_textures = bsp
            .textures
            .iter()
            .enumerate()
            .filter(|(_, t)| t.as_ref().is_some_and(|t| t.name.eq_ignore_ascii_case("sky")))
            .map(|(i, _)| i as i32)
            .collect();
        let sky = sky_of(&bsp);
        World { bsp, faces, sky, sky_textures, style_value: Box::new(style_value) }
    }

    /// The map's sky light.
    pub fn sky(&self) -> Option<Sky> {
        self.sky
    }

    /// The face of the world a hit at `p` on a plane with `normal` lies on.
    fn face_at(&self, p: [f64; 3], normal: [f64; 3]) -> Option<&Face> {
        let eps = 1.5f32;
        for fb in &self.faces {
            if (0..3).any(|k| (p[k] as f32) < fb.mins[k] - eps || (p[k] as f32) > fb.maxs[k] + eps) {
                continue;
            }
            let face = &self.bsp.faces[fb.index];
            let plane = self.bsp.planes.get(face.plane as usize)?;
            let n = plane.normal.map(f64::from);
            let align = n[0] * normal[0] + n[1] * normal[1] + n[2] * normal[2];
            if align.abs() < 0.99 || plane.side(p).abs() > 1.0 {
                continue;
            }
            // Inside the polygon: on the inner side of every edge.
            let v = self.bsp.face_vertices(face);
            // The same side of every edge (either winding), a unit's slack.
            let dots: Vec<(f64, f64)> = (0..v.len())
                .map(|i| {
                    let (a, b) = (v[i], v[(i + 1) % v.len()]);
                    let e = [b[0] as f64 - a[0] as f64, b[1] as f64 - a[1] as f64, b[2] as f64 - a[2] as f64];
                    let d = [p[0] - a[0] as f64, p[1] - a[1] as f64, p[2] - a[2] as f64];
                    let c = [e[1] * d[2] - e[2] * d[1], e[2] * d[0] - e[0] * d[2], e[0] * d[1] - e[1] * d[0]];
                    (c[0] * n[0] + c[1] * n[1] + c[2] * n[2], (e[0] * e[0] + e[1] * e[1] + e[2] * e[2]).sqrt())
                })
                .collect();
            let inside = dots.iter().all(|(c, len)| *c >= -len) || dots.iter().all(|(c, len)| *c <= *len);
            if inside {
                return Some(face);
            }
        }
        None
    }

    /// The first world surface along `from` -> `to`: the hit point, its
    /// plane's normal and the face there, if any.
    fn first_surface(&self, from: [f64; 3], to: [f64; 3]) -> Option<([f64; 3], Option<&Face>)> {
        let t = self.bsp.trace(0, 0, from, to);
        if t.fraction >= 1.0 || t.all_solid {
            return None;
        }
        Some((t.end, self.face_at(t.end, t.plane_normal)))
    }

    /// The lightmap colour (0-255) under `p` (units): the first lit face a
    /// trace straight down meets, its sample nearest the hit.
    pub fn floor_light(&self, p: [f64; 3]) -> Option<[f32; 3]> {
        let mut at = p;
        let end_z = p[2] - FLOOR_REACH;
        for _ in 0..8 {
            let (hit, face) = self.first_surface(at, [at[0], at[1], end_z])?;
            if let Some(face) = face
                && face.light_offset >= 0
                && let Some(c) = self.sample(face, hit)
            {
                return Some(c);
            }
            at = [hit[0], hit[1], hit[2] - 1.0];
        }
        None
    }

    /// `face`'s lightmap sample nearest `p`, the styles summed with their
    /// letter values, / 256 rounded down, capped at 255.
    fn sample(&self, face: &Face, p: [f64; 3]) -> Option<[f32; 3]> {
        let ext = self.bsp.lightmap_extent(face)?;
        let ti = self.bsp.texinfo.get(face.texinfo as usize)?;
        let st = |a: [f32; 4]| a[0] as f64 * p[0] + a[1] as f64 * p[1] + a[2] as f64 * p[2] + a[3] as f64;
        let i = ((st(ti.s) / 16.0 - ext.min[0] as f64).round().max(0.0) as u32).min(ext.size[0] - 1);
        let j = ((st(ti.t) / 16.0 - ext.min[1] as f64).round().max(0.0) as u32).min(ext.size[1] - 1);
        let bytes = self.bsp.face_lightmap(face)?;
        let per = (ext.size[0] * ext.size[1]) as usize * 3;
        let at = (j * ext.size[0] + i) as usize * 3;
        let mut sum = [0.0f32; 3];
        for k in 0..Bsp::style_count(face) {
            let v = (self.style_value)(face.styles[k]);
            for c in 0..3 {
                sum[c] += *bytes.get(k * per + at + c)? as f32 * v;
            }
        }
        Some(sum.map(|s| (s / 256.0).floor().min(255.0)))
    }

    /// The model light at `p` (units, game axes): mdl.md, "Lighting".
    pub fn light_at(&self, origin: [f64; 3]) -> Lit {
        let p = [origin[0], origin[1], origin[2] + 8.0];
        if let Some(sky) = self.sky.filter(|s| s.color.iter().any(|c| *c > 0.0)) {
            let to = [0, 1, 2].map(|k| p[k] - SKY_REACH * sky.dir[k] as f64);
            if let Some((_, Some(face))) = self.first_surface(p, to)
                && self.sky_textures.contains(&(self.bsp.texinfo.get(face.texinfo as usize).map_or(-1, |t| t.texture)))
            {
                return Lit::new(sky.color, Vec3::from(sky.dir));
            }
        }
        let c = self.floor_light(p).unwrap_or([0.0; 3]);
        let near = |dx: f64, dy: f64| {
            self.floor_light([p[0] + dx, p[1] + dy, p[2]]).map_or(0.0, |c| (c[0] + c[1] + c[2]) / 768.0)
        };
        let (s0, s1, s2, s3) = (near(-16.0, -16.0), near(16.0, -16.0), near(16.0, 16.0), near(-16.0, 16.0));
        let dir = Vec3::new((s0 + s3) - (s1 + s2), (s0 + s1) - (s2 + s3), -1.0);
        Lit::new(c, dir)
    }
}

/// The map's light field for moving models: the model light at the point
/// (engine space), as a probe.
pub fn field(bsp: Bsp, style_value: impl Fn(u8) -> f32 + Send + Sync + 'static) -> MapLightField {
    let world = Arc::new(World::new(bsp, style_value));
    let curve = LightCurve::default();
    MapLightField(Arc::new(move |p: Vec3| {
        let g = to_game(p);
        world.light_at([g[0] as f64, g[1] as f64, g[2] as f64]).probe(&curve)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn light(c: [f32; 3], dir: Vec3, n: Vec3) -> (Lit, f32, f32) {
        let l = Lit::new(c, dir);
        let curve = LightCurve::default();
        (l, l.illum(n), curve.vertex(l.illum(n)))
    }

    #[test]
    fn the_specs_worked_lights() {
        let down = Vec3::NEG_Z;
        // On a floor of stored 29 at m: C 29, T 29, shade 26.1, ambient 2.9.
        let (l, facing, v) = light([29.0; 3], down, Vec3::Z);
        assert!((l.shade - 26.1).abs() < 0.01 && (l.ambient - 2.9).abs() < 0.01);
        assert!((facing - 29.0).abs() < 0.01);
        assert!((v - 0.163).abs() < 0.001, "{v}");
        let (_, side, v) = light([29.0; 3], down, Vec3::X);
        assert!((side - 20.4).abs() < 0.1 && (v - 0.113).abs() < 0.002, "{side} {v}");
        let (_, away, v) = light([29.0; 3], down, Vec3::NEG_Z);
        assert!((away - 2.9).abs() < 0.01 && (v - 0.015).abs() < 0.001, "{away} {v}");
        // A floor of stored 50: C 51.
        let (_, facing, v) = light([51.0; 3], down, Vec3::Z);
        assert!((facing - 51.0).abs() < 0.01 && (v - 0.287).abs() < 0.001, "{facing} {v}");
        // Under cs_militia's sky: (229, 229, 151).
        let (l, facing, v) = light([229.0, 229.0, 151.0], down, Vec3::Z);
        assert!((l.shade - 206.1).abs() < 0.01 && (l.ambient - 22.9).abs() < 0.01);
        assert!((l.color[2] - 0.659).abs() < 0.001);
        assert!((facing - 229.0).abs() < 0.01 && (v - 0.904).abs() < 0.001, "{v}");
        let (_, side, v) = light([229.0, 229.0, 151.0], down, Vec3::X);
        assert!((side - 160.8).abs() < 0.1 && (v - 0.670).abs() < 0.002, "{side} {v}");
        let (_, away, v) = light([229.0, 229.0, 151.0], down, Vec3::NEG_Z);
        assert!((away - 22.9).abs() < 0.01 && (v - 0.128).abs() < 0.001, "{away} {v}");
    }

    #[test]
    fn black_light_is_white_with_nothing_to_show() {
        let l = Lit::new([0.0; 3], Vec3::NEG_Z);
        assert_eq!(l.color, [1.0; 3]);
        assert!(l.ambient + l.shade <= 1.0 + 1e-3);
    }

    #[test]
    fn the_cube_follows_the_engine_axes() {
        let l = Lit::new([200.0; 3], Vec3::NEG_Z);
        let p = l.probe(&LightCurve::default());
        // Engine +Y is up: facing the sun, brighter than engine -Y.
        assert!(p.cube[2].x > p.cube[3].x);
        assert!(p.cube[2].x > p.cube[0].x);
    }

    #[test]
    fn the_sky_colour_and_sun_come_from_light_environment() {
        let mut bsp = Bsp::default();
        bsp.entities_text =
            "{\n\"classname\" \"light_environment\"\n\"_light\" \"255 255 128 90\"\n\"pitch\" \"-60\"\n\"angles\" \"0 90 0\"\n}\n"
                .into();
        let sky = sky_of(&bsp).unwrap();
        assert_eq!(sky.color, [229.0, 229.0, 151.0]);
        // 60 degrees down, toward +y.
        assert!((sky.dir[2] + 0.866).abs() < 0.001 && sky.dir[1] > 0.0);
        bsp.entities_text = "{\n\"classname\" \"light_environment\"\n\"_light\" \"114\"\n}\n".into();
        assert_eq!(sky_of(&bsp).unwrap().color, [264.0; 3]);
    }
}
