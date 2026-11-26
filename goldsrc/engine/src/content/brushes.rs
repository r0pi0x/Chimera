//! The world's and the drawn brush entities' faces as meshes
//! (specs/goldsrc/bsp.md, "Faces", "Texinfo", "Lightmaps", "Brush
//! entities and render modes"): each face a fan of its corners, wound
//! counter-clockwise about its front (the plane's normal, flipped for
//! faces on its back side), its texture coordinates in repeats (s / width,
//! t / height), its lightmap's place in the atlas (`lightmap`); faces
//! grouped into one mesh per texture, part (the world, a static brush
//! entity placed by its `origin`, a mover in its own frame) and render
//! look. `sky` faces are not drawn but kept as the map's sky surfaces;
//! tool textures are not drawn (`textures::Kind`). A random-tiling
//! texture's face shows the variant `textures::Families::variant` picks;
//! liquids (`!`) warp (`textures::warp`); a mover's face whose texture
//! has an alternate animation set carries both sets' first frames
//! (`MapMesh::frames`: frame 1 is the toggled-on look).
//!
//! Known shortcuts:
//! - `{` textures are cut out at index 255 whatever the render mode (the
//!   original shows the hole only in render mode 4).
//! - Liquids keep the lightmap the file gives them (bsp.md open question
//!   4) and are not handed over as water volumes or Chimera's water
//!   material (refraction, fog): the warp is all of their look.
//! - Only movers carry both sets: an unnamed `func_wall` or
//!   `func_illusionary` (never used) shows its own set for good, as the
//!   game does; the logic (`crate::logic`) picks a node's set by its frame.
//! - Render effects (`renderfx`) are ignored.

use std::collections::BTreeMap;

use bevy::math::{Vec2, Vec3};
use goldsrc_formats::{bsp::Bsp, materials::Materials};

use super::{
    lightmap::{Atlas, Packed, StyleStart},
    textures::{Families, Kind, kind, warp},
    to_engine, to_engine_dir,
};
use crate::map::{MapAlpha, MapData, MapMesh};

/// One brush model to draw and how.
#[derive(Clone, Debug, Default)]
pub struct Drawn {
    /// Its index in the BSP's models (0: the world).
    pub model: usize,
    /// Where its faces are placed (units): its `origin` for a static
    /// brush entity, 0 for the world and for movers (drawn in their own
    /// frame).
    pub offset: [f32; 3],
    /// A mover's index in `MapData::entities` (its faces under its node).
    pub mover: Option<usize>,
    /// A translucent render mode: blended or added, and its opacity.
    pub render: Option<(MapAlpha, f32)>,
    /// Its `rendercolor` when not white.
    pub color: Option<[u8; 3]>,
    /// Render mode 1: drawn as the flat colour.
    pub flat: bool,
}

/// A surface letter of `sound/materials.txt` (mount.md) as Chimera's
/// neutral surface kind (Source's surface property names).
pub fn surface_kind(letter: char) -> &'static str {
    match letter {
        'M' => "metal",
        'D' => "dirt",
        'V' => "metalvent",
        'G' => "metalgrate",
        'T' => "tile",
        'S' => "slipperyslime",
        'W' => "wood",
        'P' => "computer",
        'Y' => "glass",
        'N' => "snow",
        'X' => "grass",
        _ => "concrete",
    }
}

/// The meshes of `models` into `data.meshes` and the lightmap atlas into
/// `data.lightmap`; `slots[t]` is BSP texture t's index into
/// `data.textures` (None: drawn flat); `liquids[t]` its fog material in
/// `data.water_materials` (a liquid texture's); `families` the lump's texture
/// families; `styles` how each light style other than 0 starts.
pub fn add_meshes(
    bsp: &Bsp,
    models: &[Drawn],
    slots: &[Option<usize>],
    liquids: &[Option<usize>],
    families: &Families,
    materials: &Materials,
    styles: &dyn Fn(u8) -> StyleStart,
    data: &mut MapData,
) {
    struct Pending {
        mesh: MapMesh,
        luxels: Vec<(usize, Vec2)>,
    }
    let mut atlas = Atlas::default();
    let white = atlas.white();
    let mut by_key: BTreeMap<(usize, i64, String), Pending> = BTreeMap::new();
    for d in models {
        let Some(m) = bsp.models.get(d.model) else { continue };
        let first = m.first_face.max(0) as usize;
        for fi in first..first + m.face_count.max(0) as usize {
            let Some(face) = bsp.faces.get(fi) else { continue };
            let Some(ti) = bsp.texinfo.get(face.texinfo as usize) else { continue };
            let texture = families.variant(ti.texture as usize, fi);
            let Some(tex) = bsp.textures.get(texture).and_then(|t| t.as_ref()) else { continue };
            let k = kind(&tex.name);
            let corners: Vec<[f32; 3]> = bsp
                .face_vertices(face)
                .into_iter()
                .map(|v| [v[0] + d.offset[0], v[1] + d.offset[1], v[2] + d.offset[2]])
                .collect();
            if corners.len() < 3 {
                continue;
            }
            if k == Kind::Sky {
                if d.model == 0 {
                    for i in 1..corners.len() - 1 {
                        data.sky_surfaces
                            .extend([corners[0], corners[i], corners[i + 1]].map(|p| to_engine(p).to_array()));
                    }
                }
                continue;
            }
            if k == Kind::Tool {
                continue;
            }
            let Some(plane) = bsp.planes.get(face.plane as usize) else { continue };
            let sign = if face.side != 0 { -1.0 } else { 1.0 };
            let normal = to_engine_dir(plane.normal.map(|c| c * sign)).normalize_or_zero();
            // Its lightmap blocks (each style's), or the white one.
            let block = match (bsp.lightmap_extent(face), bsp.face_lightmap(face)) {
                (Some(ext), Some(bytes)) if Bsp::style_count(face) > 0 => {
                    let styles = &face.styles[..Bsp::style_count(face)];
                    Some((atlas.add_styled(ext.size[0], ext.size[1], bytes, styles), ext))
                }
                _ => None,
            };
            let slot = slots.get(texture).copied().flatten();
            let render_key = format!("{:?}{:?}{}", d.render, d.color, d.flat);
            let key = (texture, d.mover.map_or(-1, |i| i as i64), render_key);
            // A mover's two animation sets (off, toggled on).
            let frames: Vec<usize> = match (d.mover, families.toggle(texture)) {
                (Some(_), Some((a, b))) if !d.flat => match (slots.get(a).copied().flatten(), slots.get(b).copied().flatten()) {
                    (Some(a), Some(b)) => vec![a, b],
                    _ => Vec::new(),
                },
                _ => Vec::new(),
            };
            let pending = by_key.entry(key).or_insert_with(|| Pending {
                mesh: MapMesh {
                    material: tex.name.to_ascii_lowercase(),
                    texture: if d.flat { None } else { slot },
                    color: [255; 3],
                    alpha: if k == Kind::Masked { MapAlpha::Mask(0.5) } else { MapAlpha::Opaque },
                    surface: Some(surface_kind(materials.surface(&tex.name)).to_string()),
                    entity: d.mover,
                    render: d.render,
                    render_color: d.color,
                    flat_color: d.flat,
                    warp: (k == Kind::Water).then(|| warp(tex.width, tex.height)),
                    // A liquid's face fogs the view under it (`water::fog_material`).
                    water: if d.mover.is_none() { liquids.get(texture).copied().flatten() } else { None },
                    frames,
                    ..Default::default()
                },
                luxels: Vec::new(),
            });
            // Texel coordinates in double, as the lightmap sizing.
            let st = |p: [f32; 3], a: [f32; 4]| {
                let q = [
                    (p[0] - d.offset[0]) as f64,
                    (p[1] - d.offset[1]) as f64,
                    (p[2] - d.offset[2]) as f64,
                ];
                a[0] as f64 * q[0] + a[1] as f64 * q[1] + a[2] as f64 * q[2] + a[3] as f64
            };
            let engine: Vec<Vec3> = corners.iter().map(|&p| to_engine(p)).collect();
            for i in 1..corners.len() - 1 {
                let mut tri = [0, i, i + 1];
                let (a, b, c) = (engine[tri[0]], engine[tri[1]], engine[tri[2]]);
                if (b - a).cross(c - a).dot(normal) < 0.0 {
                    tri.swap(1, 2);
                }
                for v in tri {
                    let p = corners[v];
                    let (s, t) = (st(p, ti.s), st(p, ti.t));
                    let mesh = &mut pending.mesh;
                    mesh.indices.push(mesh.positions.len() as u32);
                    mesh.positions.push(engine[v].to_array());
                    mesh.normals.push(normal.to_array());
                    mesh.uvs.push([(s / tex.width as f64) as f32, (t / tex.height as f64) as f32]);
                    pending.luxels.push(match &block {
                        Some((b, ext)) => (
                            *b,
                            Vec2::new(
                                (s / 16.0 - ext.min[0] as f64) as f32,
                                (t / 16.0 - ext.min[1] as f64) as f32,
                            ),
                        ),
                        None => (white, Vec2::ZERO),
                    });
                }
            }
        }
    }
    let packed: Packed = atlas.finish(styles);
    for (_, mut p) in by_key {
        p.mesh.lightmap_uvs = p.luxels.iter().map(|&(b, l)| packed.uv(b, l)).collect();
        if !p.mesh.indices.is_empty() {
            data.meshes.push(p.mesh);
        }
    }
    data.lightmap = Some(packed.map);
}
