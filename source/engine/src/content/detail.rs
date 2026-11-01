//! Detail props: the grass, weeds and small bushes vbsp scatters over
//! displacements and floors from the map's detail.vbsp (the BSP's `dprp`
//! game lump; public BSP description, "Detail props", and the public
//! env_detail_controller / detail.vbsp pages). Sprites and the "cross" and
//! "tri" sprite shapes become `MapDetailProps` quads (`render::detail` draws
//! them); model entries become static props without collision. The lump
//! itself is read by `source_formats::detail`.
//!
//! How the sprite shapes are laid out and how sway moves them is our
//! reading of those pages (docs/tech-debt.md): not measured against CS:S.

use bevy::prelude::*;
pub use source_formats::detail::{DetailLump, DetailObject, SpriteDict, read};
use source_formats::detail::{ORIENT_SCREEN, ORIENT_SCREEN_VERTICAL, TYPE_CROSS, TYPE_MODEL, TYPE_SPRITE, TYPE_TRI};

use super::bsp::{METERS_PER_UNIT, to_engine};
use crate::map::{DetailQuad, MapDetailProps};

/// `cl_detaildist` and `cl_detailfade` defaults (units): with no
/// env_detail_controller, detail props fade out over the last
/// `cl_detailfade` units before `cl_detaildist`.
pub const DETAIL_DIST: f32 = 1200.0;
pub const DETAIL_FADE: f32 = 400.0;

/// The map's env_detail_controller fade (start, end), units: its
/// `fademindist` and `fademaxdist`; a negative start means the end less
/// `cl_detailfade` (public entity docs). None without one.
pub fn controller_fade(bsp: &vbsp::Bsp) -> Option<(f32, f32)> {
    let e = bsp
        .entities
        .iter()
        .find(|e| e.prop("classname") == Some("env_detail_controller"))?;
    let num = |k: &'static str, d: f32| e.prop(k).and_then(|v| v.trim().parse::<f32>().ok()).unwrap_or(d);
    let end = num("fademaxdist", DETAIL_DIST);
    let start = num("fademindist", end - DETAIL_FADE);
    let start = if start < 0.0 { end - DETAIL_FADE } else { start };
    Some((start.clamp(0.0, end), end))
}

/// The model entries: (model path, origin, angles), Source units and
/// degrees; drawn as static props without collision.
pub fn model_entries(lump: &DetailLump) -> Vec<(String, Vec3, Vec3)> {
    lump.objects
        .iter()
        .filter(|o| o.kind == TYPE_MODEL)
        .filter_map(|o| Some((lump.models.get(o.index as usize)?.clone(), o.origin, o.angles)))
        .collect()
}

/// Source rotation of an object's angles (pitch, yaw, roll) in Source
/// axes.
fn source_rotation(angles: Vec3) -> Quat {
    Quat::from_rotation_z(angles.y.to_radians())
        * Quat::from_rotation_y(angles.x.to_radians())
        * Quat::from_rotation_x(angles.z.to_radians())
}

fn engine(v: Vec3) -> Vec3 {
    to_engine(vbsp::Vector { x: v.x, y: v.y, z: v.z })
}

/// One blade of a sprite: its quad in the object's own frame (x forward,
/// y left, z up; the blade spans y and z, facing x), Source units, as
/// [bottom-left, top-left, top-right, bottom-right] seen from the front,
/// plus how far along its height each corner is (0 bottom, 1 top).
fn blade(s: &SpriteDict, scale: f32) -> [Vec3; 4] {
    let (x0, x1) = (s.ul.x * scale, s.lr.x * scale);
    let (y0, y1) = (s.lr.y * scale, s.ul.y * scale);
    // Across the blade is -y (its right seen from the front), up is z.
    [
        Vec3::new(0.0, -x0, y0),
        Vec3::new(0.0, -x0, y1),
        Vec3::new(0.0, -x1, y1),
        Vec3::new(0.0, -x1, y0),
    ]
}

/// The quads of every sprite, cross and tri entry (engine space), with the
/// map's detail texture.
pub fn quads(lump: &DetailLump) -> Vec<DetailQuad> {
    let mut out = Vec::new();
    for o in &lump.objects {
        let Some(s) = lump.sprites.get(o.index as usize) else {
            continue;
        };
        let scale = if o.scale > 0.0 { o.scale } else { 1.0 };
        let uv = [
            Vec2::new(s.tex_ul.x, s.tex_lr.y),
            s.tex_ul,
            Vec2::new(s.tex_lr.x, s.tex_ul.y),
            s.tex_lr,
        ];
        let origin = engine(o.origin);
        let sway = o.sway as f32 / 255.0;
        let push = |out: &mut Vec<DetailQuad>, corners: [Vec3; 4], rot: Quat| {
            out.push(DetailQuad {
                origin,
                corners: corners.map(|c| engine(rot * c)),
                billboard: None,
                uv,
                light: o.light,
                sway,
            });
        };
        let rot = source_rotation(o.angles);
        match o.kind {
            TYPE_SPRITE if matches!(o.orientation, ORIENT_SCREEN | ORIENT_SCREEN_VERTICAL) => {
                let (x0, x1) = (s.ul.x * scale, s.lr.x * scale);
                let (y0, y1) = (s.lr.y * scale, s.ul.y * scale);
                out.push(DetailQuad {
                    origin,
                    corners: [Vec3::ZERO; 4],
                    billboard: Some(crate::map::DetailBillboard {
                        vertical: o.orientation == ORIENT_SCREEN_VERTICAL,
                        lo: Vec2::new(x0, y0) * METERS_PER_UNIT,
                        hi: Vec2::new(x1, y1) * METERS_PER_UNIT,
                    }),
                    uv,
                    light: o.light,
                    sway,
                });
            }
            TYPE_SPRITE => push(&mut out, blade(s, scale), rot),
            TYPE_CROSS => {
                // Two blades crossing at the origin, square to each other.
                for k in 0..2 {
                    let turn = Quat::from_rotation_z(k as f32 * std::f32::consts::FRAC_PI_2);
                    push(&mut out, blade(s, scale).map(|c| turn * c), rot);
                }
            }
            TYPE_TRI => {
                // Three blades around the origin, a third of a turn apart,
                // each moved out from the centre by its size share of the
                // blade's width and leaning out by the shape angle.
                let width = (s.lr.x - s.ul.x).abs() * scale;
                let out_by = o.shape_size as f32 / 255.0 * width;
                let lean = Quat::from_rotation_y((o.shape_angle as f32).to_radians());
                for k in 0..3 {
                    let turn = Quat::from_rotation_z(k as f32 * std::f32::consts::TAU / 3.0);
                    let corners = blade(s, scale).map(|c| turn * (lean * c + Vec3::X * out_by));
                    push(&mut out, corners, rot);
                }
            }
            _ => {}
        }
    }
    out
}

/// The map's detail sprites as `MapDetailProps` (None without any or
/// without its sprite material), with the controller's fade and env_wind's
/// sway; `texture` resolves the material to a texture index.
pub fn detail_props(
    lump: &DetailLump,
    bsp: &vbsp::Bsp,
    texture: impl FnOnce(&str) -> Option<usize>,
) -> Option<MapDetailProps> {
    let quads = quads(lump);
    if quads.is_empty() {
        return None;
    }
    let material = bsp
        .entities
        .iter()
        .find(|e| e.prop("classname") == Some("worldspawn"))
        .and_then(|e| e.prop("detailmaterial"))
        .map(|m| m.trim().to_lowercase().replace('\\', "/"))
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| "detail/detailsprites".to_string());
    let texture = texture(&material)?;
    let fade = controller_fade(bsp).map(|(a, b)| (a * METERS_PER_UNIT, b * METERS_PER_UNIT));
    let wind = bsp
        .entities
        .iter()
        .find(|e| e.prop("classname") == Some("env_wind"))
        .map(|e| {
            let num = |k: &'static str| e.prop(k).and_then(|v| v.trim().parse::<f32>().ok()).unwrap_or(0.0);
            let yaw = e
                .prop("angles")
                .and_then(|a| a.split_whitespace().nth(1).and_then(|y| y.parse::<f32>().ok()))
                .unwrap_or(0.0);
            let dir = engine(Vec3::new(yaw.to_radians().cos(), yaw.to_radians().sin(), 0.0)).normalize_or_zero();
            (dir, (num("minwind") + num("maxwind")) / 2.0 * METERS_PER_UNIT)
        });
    Some(MapDetailProps {
        texture,
        quads,
        fade,
        wind,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A lump as `source_formats::detail::read` returns it: one model, one
    /// sprite, and one object per (type, orientation).
    fn lump(objects: &[(u8, u8)]) -> DetailLump {
        DetailLump {
            models: vec!["models/props/bush.mdl".into()],
            sprites: vec![SpriteDict {
                ul: Vec2::new(-6.0, 12.0),
                lr: Vec2::new(6.0, 0.0),
                tex_ul: Vec2::new(0.5, 0.25),
                tex_lr: Vec2::new(0.75, 0.5),
            }],
            objects: objects
                .iter()
                .map(|&(kind, orientation)| DetailObject {
                    origin: Vec3::new(10.0, 20.0, 30.0),
                    angles: Vec3::new(0.0, 90.0, 0.0),
                    index: 0,
                    light: [128.0 * 2.0 / 255.0, 64.0 * 2.0 / 255.0, 32.0 * 2.0 / 255.0],
                    sway: 127,
                    shape_angle: 30,
                    shape_size: 25,
                    orientation,
                    kind,
                    scale: 2.0,
                })
                .collect(),
        }
    }

    #[test]
    fn model_entries_are_the_model_objects() {
        let lump = lump(&[(TYPE_MODEL, 0), (TYPE_SPRITE, ORIENT_SCREEN_VERTICAL)]);
        assert_eq!(
            model_entries(&lump),
            [(
                "models/props/bush.mdl".to_string(),
                Vec3::new(10.0, 20.0, 30.0),
                Vec3::new(0.0, 90.0, 0.0)
            )]
        );
    }

    #[test]
    fn sprite_shapes_make_their_blades() {
        let q = quads(&lump(&[
            (TYPE_SPRITE, ORIENT_SCREEN_VERTICAL),
            (TYPE_SPRITE, 0),
            (TYPE_CROSS, 0),
            (TYPE_TRI, 0),
        ]));
        assert_eq!(q.len(), 1 + 1 + 2 + 3);
        // The facing sprite: its quad in meters, scale 2, from 0 up to 24
        // units and 12 either side.
        let b = q[0].billboard.unwrap();
        assert!(b.vertical);
        assert!((b.hi.y - 24.0 * METERS_PER_UNIT).abs() < 1e-6);
        assert!((b.lo.x + 12.0 * METERS_PER_UNIT).abs() < 1e-6);
        // The fixed sprite stands up from its origin: two corners at its
        // height, two at its foot.
        let ys: Vec<f32> = q[1].corners.iter().map(|c| c.y / METERS_PER_UNIT).collect();
        assert!((ys[1] - 24.0).abs() < 1e-3 && ys[0].abs() < 1e-3, "{ys:?}");
        // The tri's blades sit out from the centre and lean out.
        for t in &q[4..] {
            let foot = (t.corners[0] + t.corners[3]) / 2.0;
            let top = (t.corners[1] + t.corners[2]) / 2.0;
            assert!(foot.xz().length() > 0.0);
            assert!(top.xz().length() > foot.xz().length(), "leans out");
        }
        // Texture corners: bottom-left takes (u of the upper left, v of
        // the lower right).
        assert_eq!(q[1].uv[0], Vec2::new(0.5, 0.5));
        assert_eq!(q[1].uv[1], Vec2::new(0.5, 0.25));
    }
}
