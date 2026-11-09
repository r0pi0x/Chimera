//! Drawn breakables: gibs and windows' panes. The simulation's side is
//! `map::breakables` (what flies, where, for how long; which panes are
//! whole); each gib carries a `map::breakables::GibLook` (which model of
//! which gib list, skin, glass, fade distances) and a window node its
//! `BrushPanes`, and these systems draw them: a gib's meshes with its own
//! material instances, lit where it starts and faded out after its life;
//! a window's meshes clipped to its unbroken panes (`clip_panes`), its
//! broken looks (`PanePart`: the crack face, jagged edge strips,
//! `edge_strips`) shown in place of the face, and whole again when the
//! node's panes go (a round restart, `restore_panes`).
//!
//! `GibDraws` (each gib list's meshes and skins, in `render::bodies::PlayModels`),
//! `draw_gibs`/`fade_gibs`, `clip_panes`/`restore_panes`, `clip_to_panes`,
//! `clip_to_rects`.

use bevy::{
    mesh::{Indices, VertexAttributeValues},
    prelude::*,
};

use super::{bodies::PlayModels, decal::RuntimeDecal, prop_material::PropMaterial};
use crate::map::{
    LightField, MapData, MapDebugView, PaneLook,
    breakables::{BrushPanes, FlyingGib, GibAssets, GibLook, MapGibPhysics, gib_alpha},
};

/// Each gib list's models' meshes and their materials per skin family.
pub type GibDraws = Vec<Vec<GibParts>>;

/// A gib model's meshes, and each skin family's materials for them when
/// it has several.
pub struct GibParts {
    parts: Vec<(Handle<Mesh>, Handle<PropMaterial>)>,
    skins: Vec<Vec<Handle<PropMaterial>>>,
}

/// The gib lists' meshes and materials (`MapData::gibs`, in order).
pub fn build_gib_draws(
    data: &MapData,
    textures: &[Handle<Image>],
    view: MapDebugView,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<PropMaterial>,
    mut dormant: Option<&mut super::prop_material::DormantProps>,
) -> GibDraws {
    // Templates: each gib draws its own copy (`draw_gibs`).
    let mut add = |m: PropMaterial| match dormant.as_deref_mut() {
        Some(d) => d.add(materials, m),
        None => materials.add(m),
    };
    data.gibs
        .iter()
        .map(|s| {
            s.models
                .iter()
                .map(|model| {
                    let parts = model
                        .meshes
                        .iter()
                        .map(|m| {
                            let mut material = crate::render::lit_prop_material(m, textures, data, view, false);
                            material.params.dynamic = 0.0;
                            (meshes.add(crate::render::build_mesh(m, false)), add(material))
                        })
                        .collect();
                    let skins = model
                        .skins
                        .iter()
                        .map(|looks| {
                            model
                                .meshes
                                .iter()
                                .zip(looks)
                                .map(|(m, look)| {
                                    let mut material =
                                        crate::render::lit_prop_material(&look.apply(m), textures, data, view, false);
                                    material.params.dynamic = 0.0;
                                    add(material)
                                })
                                .collect()
                        })
                        .collect();
                    GibParts { parts, skins }
                })
                .collect()
        })
        .collect()
}

/// A gib's own material instances (faded as it goes).
#[derive(Component)]
pub struct GibMaterials(Vec<Handle<PropMaterial>>);

/// New gibs' meshes (`GibLook`): their own materials, lit where they
/// start (their centre), translucent for glass; each mesh at the model's
/// origin from the centre, casting no shadow, faded out between the
/// look's distances (a band dithers out; equal ones pop).
pub fn draw_gibs(
    gibs: Query<(Entity, &GibLook, &Transform), Added<GibLook>>,
    models: Option<Res<PlayModels>>,
    assets: Option<Res<GibAssets>>,
    light_field: Option<Res<LightField>>,
    materials: Option<ResMut<Assets<PropMaterial>>>,
    dormant: Option<Res<super::prop_material::DormantProps>>,
    mut commands: Commands,
) {
    let (Some(models), Some(assets), Some(mut materials)) = (models, assets, materials) else {
        return;
    };
    for (e, look, at) in &gibs {
        let Some(model) = models.gibs.get(look.set).and_then(|s| s.get(look.model)) else {
            continue;
        };
        let probe = light_field.as_ref().map(|f| (f.0.0)(at.translation));
        // Its skin's materials (a skin it lacks shows skin 0).
        let skin = usize::try_from(look.skin).ok().and_then(|s| model.skins.get(s));
        let mut own = Vec::with_capacity(model.parts.len());
        for (i, (_, m)) in model.parts.iter().enumerate() {
            let m = skin.and_then(|s| s.get(i)).unwrap_or(m);
            let Some(mut material) = super::prop_material::prop(&materials, dormant.as_deref(), m).cloned() else {
                continue;
            };
            if let Some(probe) = &probe {
                material.params.set_probe(probe, assets.light_scale);
            }
            if look.glass {
                material.alpha_mode = AlphaMode::Blend;
                material.params.translucent = 1.0;
                material.params.base_color.w = assets.physics.glass_alpha;
            }
            own.push(materials.add(material));
        }
        let range = look.fade.map(|(near, far)| {
            crate::map::vis::fade_band(near, far)
                .unwrap_or_else(|| bevy::camera::visibility::VisibilityRange::abrupt(0.0, far))
        });
        let offset = look.offset;
        commands.entity(e).insert(GibMaterials(own.clone())).with_children(|g| {
            for ((mesh, _), material) in model.parts.iter().zip(own) {
                let mut c = g.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::from_translation(offset),
                    bevy::light::NotShadowCaster,
                ));
                if let Some(range) = &range {
                    c.insert(range.clone());
                }
            }
        });
    }
}

/// Fading gibs' materials take their opacity.
pub fn fade_gibs(
    gibs: Query<(&FlyingGib, &GibMaterials)>,
    assets: Option<Res<GibAssets>>,
    materials: Option<ResMut<Assets<PropMaterial>>>,
) {
    let (Some(assets), Some(mut materials)) = (assets, materials) else {
        return;
    };
    for (gib, own) in &gibs {
        fade(gib, &own.0, assets.physics, &mut materials);
    }
}

/// A fading gib's materials take its opacity.
fn fade(gib: &FlyingGib, own: &[Handle<PropMaterial>], p: MapGibPhysics, materials: &mut Assets<PropMaterial>) {
    let Some(alpha) = gib_alpha(gib.state.age, gib.life, p.fade).filter(|a| *a < 1.0) else {
        return;
    };
    let base = if gib.glass { p.glass_alpha } else { 1.0 };
    for m in own {
        if let Some(mut material) = materials.get_mut(m) {
            material.alpha_mode = AlphaMode::Blend;
            material.params.translucent = 1.0;
            material.params.base_color.w = alpha * base;
        }
    }
}

/// A window's broken look on one of its meshes (`MapMesh::pane_look`):
/// hidden until the window breaks.
#[derive(Component, Clone, Copy, Debug)]
pub struct PanePart(pub PaneLook);

/// How far (in panes) a jagged edge piece reaches into the broken pane
/// beside it: a whole pane, the square edge texture square on it (ours:
/// the spec leaves edge pieces to the client; half a pane drew shards a
/// third the length of CS:S's on cs_militia's windows).
pub const EDGE_DEPTH: f32 = 1.0;

/// A rectangle of pane space to cut a mesh to; `uv` maps a point `p` of
/// it to mask coordinates ((p − o)·x, (p − o)·y), which ride in the
/// vertex colour's red and green, and `tile` in its blue, for the edge
/// piece's jagged mask
/// (`MapMesh::pane_mask`; the mesh's own coordinates are kept, so the
/// piece's texture lines up with its pane's).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaneRect {
    pub lo: Vec2,
    pub hi: Vec2,
    pub uv: Option<(Vec2, Vec2, Vec2)>,
    /// Which tile of the mask texture (the vertex colour's blue).
    pub tile: u8,
}

/// The edge strips of edge material `k` of `kinds`: along each side of an
/// unbroken pane whose neighbour inside the window broke, reaching
/// `EDGE_DEPTH` into the neighbour; its mask's u runs along the side
/// and its solid bottom (v = 1) lies on the pane, its shards rising into
/// the hole. Each side picks one of the `kinds` edge masks by its place
/// (ours).
pub fn edge_strips(panes: &BrushPanes, k: u8, kinds: u8) -> Vec<PaneRect> {
    let (cols, rows) = (panes.cols as i32, panes.rows as i32);
    let broken = |c: i32, r: i32| c >= 0 && r >= 0 && c < cols && r < rows && !panes.whole(c as usize, r as usize);
    let d = EDGE_DEPTH;
    let mut out = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            if !panes.whole(c as usize, r as usize) {
                continue;
            }
            let (x, y) = (c as f32, r as f32);
            // (neighbour, rect, origin, u axis, v axis)
            let sides = [
                (
                    (c + 1, r),
                    Vec2::new(x + 1.0, y),
                    Vec2::new(x + 1.0 + d, y + 1.0),
                    Vec2::new(x + 1.0 + d, y),
                    Vec2::Y,
                    Vec2::NEG_X / d,
                ),
                (
                    (c - 1, r),
                    Vec2::new(x - d, y),
                    Vec2::new(x, y + 1.0),
                    Vec2::new(x - d, y + 1.0),
                    Vec2::NEG_Y,
                    Vec2::X / d,
                ),
                (
                    (c, r + 1),
                    Vec2::new(x, y + 1.0),
                    Vec2::new(x + 1.0, y + 1.0 + d),
                    Vec2::new(x + 1.0, y + 1.0 + d),
                    Vec2::NEG_X,
                    Vec2::NEG_Y / d,
                ),
                (
                    (c, r - 1),
                    Vec2::new(x, y - d),
                    Vec2::new(x + 1.0, y),
                    Vec2::new(x, y - d),
                    Vec2::X,
                    Vec2::Y / d,
                ),
            ];
            for (i, ((nc, nr), lo, hi, o, ux, vy)) in sides.into_iter().enumerate() {
                if !broken(nc, nr) {
                    continue;
                }
                let pick = (c as u32).wrapping_mul(7) ^ (r as u32).wrapping_mul(13) ^ (i as u32).wrapping_mul(5);
                if kinds == 0 || (pick % kinds as u32) as u8 != k {
                    continue;
                }
                out.push(PaneRect {
                    lo,
                    hi,
                    uv: Some((o, ux, vy)),
                    tile: k,
                });
            }
        }
    }
    out
}

/// The part of a triangle mesh over unbroken panes: every triangle
/// clipped to each unbroken pane it overlaps (in the window's plane), its
/// float attributes interpolated.
pub fn clip_to_panes(mesh: &Mesh, panes: &BrushPanes) -> Option<Mesh> {
    let mut rects = Vec::new();
    for r in 0..panes.rows {
        for c in 0..panes.cols {
            if panes.whole(c, r) {
                let lo = Vec2::new(c as f32, r as f32);
                rects.push(PaneRect {
                    lo,
                    hi: lo + Vec2::ONE,
                    uv: None,
                    tile: 0,
                });
            }
        }
    }
    clip_to_rects(mesh, panes, &rects)
}

/// A triangle mesh cut to rectangles of pane space: every triangle
/// clipped to each rectangle it overlaps (in the window's plane), its
/// float attributes interpolated, and the rectangles' own mask
/// coordinates, where they have them, as the vertex colour (u, v, tile,
/// 1).
pub fn clip_to_rects(mesh: &Mesh, panes: &BrushPanes, rects: &[PaneRect]) -> Option<Mesh> {
    let positions: Vec<Vec3> = match mesh.try_attribute(Mesh::ATTRIBUTE_POSITION).ok()? {
        VertexAttributeValues::Float32x3(v) => v.iter().map(|p| Vec3::from(*p)).collect(),
        _ => return None,
    };
    let indices: Vec<u32> = match mesh.try_indices_option().ok()? {
        Some(i) => i.iter().map(|i| i as u32).collect(),
        None => (0..positions.len() as u32).collect(),
    };
    let st: Vec<Vec2> = positions.iter().map(|p| panes.pane_at(*p)).collect();
    // Output vertices as weights over a source triangle, and their own
    // texture coordinates.
    let mut out: Vec<([u32; 3], Vec3, Option<Vec3>)> = Vec::new();
    let mut tris: Vec<[u32; 3]> = Vec::new();
    for t in indices.chunks_exact(3) {
        let tri = [t[0], t[1], t[2]];
        let pts = tri.map(|i| st[i as usize]);
        let lo = pts[0].min(pts[1]).min(pts[2]);
        let hi = pts[0].max(pts[1]).max(pts[2]);
        for rect in rects {
            if rect.hi.x <= lo.x || rect.lo.x >= hi.x || rect.hi.y <= lo.y || rect.lo.y >= hi.y {
                continue;
            }
            let poly = clip_triangle(pts, rect.lo, rect.hi);
            if poly.len() < 3 {
                continue;
            }
            let base = out.len() as u32;
            out.extend(poly.iter().map(|w| {
                let uv = rect.uv.map(|(o, x, y)| {
                    let p = pts[0] * w.x + pts[1] * w.y + pts[2] * w.z - o;
                    Vec3::new(p.dot(x), p.dot(y), rect.tile as f32)
                });
                (tri, *w, uv)
            }));
            for k in 1..poly.len() as u32 - 1 {
                tris.push([base, base + k, base + k + 1]);
            }
        }
    }
    let mut clipped = Mesh::new(mesh.primitive_topology(), mesh.asset_usage);
    for (attribute, values) in mesh.try_attributes().ok()? {
        let mix = |get: &dyn Fn(usize) -> Vec4| -> Vec<Vec4> {
            out.iter()
                .map(|(t, w, _)| get(t[0] as usize) * w.x + get(t[1] as usize) * w.y + get(t[2] as usize) * w.z)
                .collect()
        };
        let values: VertexAttributeValues = match values {
            VertexAttributeValues::Float32x2(v) => mix(&|i| Vec2::from(v[i]).extend(0.0).extend(0.0))
                .into_iter()
                .map(|x| [x.x, x.y])
                .collect::<Vec<_>>()
                .into(),
            VertexAttributeValues::Float32x3(v) => mix(&|i| Vec3::from(v[i]).extend(0.0))
                .into_iter()
                .map(|x| [x.x, x.y, x.z])
                .collect::<Vec<_>>()
                .into(),
            VertexAttributeValues::Float32x4(v) => mix(&|i| Vec4::from(v[i]))
                .into_iter()
                .map(|x| x.to_array())
                .collect::<Vec<_>>()
                .into(),
            _ => continue,
        };
        clipped.insert_attribute(attribute.clone(), values);
    }
    if out.iter().any(|(_, _, uv)| uv.is_some()) {
        let mask: Vec<[f32; 4]> = out
            .iter()
            .map(|(_, _, uv)| {
                let uv = uv.unwrap_or_default();
                [uv.x, uv.y, uv.z, 1.0]
            })
            .collect();
        clipped.insert_attribute(Mesh::ATTRIBUTE_COLOR, mask);
    }
    clipped.insert_indices(Indices::U32(tris.into_iter().flatten().collect()));
    Some(clipped)
}

/// A triangle (2D points) clipped to a rectangle: the polygon's corners as
/// barycentric weights over the triangle.
fn clip_triangle(pts: [Vec2; 3], lo: Vec2, hi: Vec2) -> Vec<Vec3> {
    let mut poly: Vec<Vec3> = vec![Vec3::X, Vec3::Y, Vec3::Z];
    let at = |w: Vec3| pts[0] * w.x + pts[1] * w.y + pts[2] * w.z;
    // Half-planes: (axis, bound, keep below?).
    for (axis, bound, below) in [(0, lo.x, false), (0, hi.x, true), (1, lo.y, false), (1, hi.y, true)] {
        let inside = |w: Vec3| {
            let v = at(w)[axis];
            if below { v <= bound + 1e-5 } else { v >= bound - 1e-5 }
        };
        let mut next = Vec::with_capacity(poly.len() + 2);
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            let (ia, ib) = (inside(a), inside(b));
            if ia {
                next.push(a);
            }
            if ia != ib {
                let (va, vb) = (at(a)[axis], at(b)[axis]);
                let t = ((bound - va) / (vb - va)).clamp(0.0, 1.0);
                next.push(a + (b - a) * t);
            }
        }
        poly = next;
        if poly.len() < 3 {
            return Vec::new();
        }
    }
    poly
}

/// A mesh drawn under a node with `BrushPanes`: its whole (unclipped)
/// mesh, from which the unbroken panes are cut.
#[derive(Component)]
pub struct PaneSource(pub(crate) Handle<Mesh>);

/// Keep a window's meshes to its unbroken panes (`map::breakables::update_panes`
/// its collider).
#[allow(clippy::type_complexity)]
pub fn clip_panes(
    nodes: Query<(&BrushPanes, Option<&Children>), Changed<BrushPanes>>,
    mut parts: Query<(&mut Mesh3d, Option<&PaneSource>, Option<&PanePart>, &mut Visibility, Has<RuntimeDecal>)>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    mut commands: Commands,
) {
    let mut meshes = meshes;
    for (panes, children) in &nodes {
        let (Some(meshes), Some(children)) = (meshes.as_mut(), children) else {
            continue;
        };
        // With a broken look the face as placed gives way to it.
        let looks: Vec<PaneLook> = children
            .iter()
            .filter_map(|c| parts.get(c).ok().map(|p| p.2.map_or(PaneLook::Whole, |l| l.0)))
            .collect();
        let cracked = looks.contains(&PaneLook::Cracked);
        let edges = looks.iter().filter(|l| matches!(l, PaneLook::Edge(_))).count() as u8;
        for child in children.iter() {
            let Ok((mut mesh3d, source, look, mut visibility, decal)) = parts.get_mut(child) else {
                continue;
            };
            let look = look.map_or(PaneLook::Whole, |l| l.0);
            let source = match source {
                Some(s) => s.0.clone(),
                None => {
                    let s = mesh3d.0.clone();
                    commands.entity(child).insert(PaneSource(s.clone()));
                    s
                }
            };
            // A bullet mark stays on the panes left, over the crack look.
            if look == PaneLook::Whole && cracked && !decal {
                visibility.set_if_neq(Visibility::Hidden);
                continue;
            }
            let Some(clipped) = meshes.get(&source).and_then(|m| match look {
                PaneLook::Edge(k) => clip_to_rects(m, panes, &edge_strips(panes, k, edges)),
                _ => clip_to_panes(m, panes),
            }) else {
                continue;
            };
            // Nothing left of it: hidden (an empty mesh upsets the
            // renderer's allocator).
            if clipped.indices().is_none_or(|i| i.is_empty()) {
                visibility.set_if_neq(Visibility::Hidden);
                continue;
            }
            visibility.set_if_neq(Visibility::Inherited);
            let old = std::mem::replace(&mut mesh3d.0, meshes.add(clipped));
            if old != source {
                meshes.remove(&old);
            }
        }
    }
}

/// A window whose panes went (a round restart makes it whole,
/// `map::breakables::round_restart`): its meshes uncut, its broken looks
/// hidden.
pub fn restore_panes(
    mut removed: RemovedComponents<BrushPanes>,
    nodes: Query<&Children, Without<BrushPanes>>,
    mut parts: Query<(&mut Mesh3d, &PaneSource, Has<PanePart>, &mut Visibility)>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut commands: Commands,
) {
    for node in removed.read() {
        let Ok(children) = nodes.get(node) else { continue };
        for child in children.iter() {
            let Ok((mut mesh3d, source, broken_look, mut visibility)) = parts.get_mut(child) else {
                continue;
            };
            // The face as placed again, its broken looks hidden.
            visibility.set_if_neq(if broken_look {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            });
            let old = std::mem::replace(&mut mesh3d.0, source.0.clone());
            if let Some(meshes) = meshes.as_mut()
                && old != source.0
            {
                meshes.remove(&old);
            }
            commands.entity(child).remove::<PaneSource>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panes(cols: usize, rows: usize) -> BrushPanes {
        BrushPanes {
            corner: Vec3::ZERO,
            u: Vec3::X,
            v: Vec3::Y,
            depth: (-0.1, 0.0),
            cols,
            rows,
            broken: vec![false; cols * rows],
        }
    }

    #[test]
    fn clipping_keeps_only_unbroken_panes() {
        // A 2x1 quad over two panes; break the right one.
        let mut mesh = Mesh::new(
            bevy::mesh::PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::default(),
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_UV_0,
            vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        );
        mesh.insert_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]));
        let mut p = panes(2, 1);
        let whole = clip_to_panes(&mesh, &p).unwrap();
        let area = |m: &Mesh| {
            let Some(VertexAttributeValues::Float32x3(pos)) = m.attribute(Mesh::ATTRIBUTE_POSITION) else {
                panic!()
            };
            let idx: Vec<usize> = m.indices().unwrap().iter().collect();
            idx.chunks(3)
                .map(|t| {
                    let [a, b, c] = [t[0], t[1], t[2]].map(|i| Vec3::from(pos[i]));
                    (b - a).cross(c - a).length() / 2.0
                })
                .sum::<f32>()
        };
        assert!((area(&whole) - 2.0).abs() < 1e-4);
        p.broken[1] = true;
        let half = clip_to_panes(&mesh, &p).unwrap();
        assert!((area(&half) - 1.0).abs() < 1e-4, "{}", area(&half));
        // UVs follow: nothing past u = 0.5 is left.
        let Some(VertexAttributeValues::Float32x2(uv)) = half.attribute(Mesh::ATTRIBUTE_UV_0) else {
            panic!()
        };
        assert!(uv.iter().all(|u| u[0] <= 0.5 + 1e-5));
    }

    /// An edge piece keeps the face's texture coordinates (its crack
    /// texture runs on from the pane's) and carries its mask's in the
    /// vertex colour: solid (v = 1) on the pane, shards (v = 0) toward the
    /// hole.
    #[test]
    fn edge_strips_keep_the_face_coordinates_and_carry_the_mask() {
        let mut mesh = Mesh::new(
            bevy::mesh::PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::default(),
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_UV_0,
            vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        );
        mesh.insert_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]));
        let mut p = panes(2, 1);
        p.broken[1] = true;
        // One edge kind: the left pane's right side, into the broken one.
        let rects = edge_strips(&p, 0, 1);
        assert_eq!(rects.len(), 1);
        // The square edge texture square on the broken pane: the strip
        // covers all of it.
        assert_eq!((rects[0].lo, rects[0].hi), (Vec2::new(1.0, 0.0), Vec2::new(2.0, 1.0)));
        let strip = clip_to_rects(&mesh, &p, &rects).unwrap();
        let Some(VertexAttributeValues::Float32x3(pos)) = strip.attribute(Mesh::ATTRIBUTE_POSITION) else {
            panic!()
        };
        let Some(VertexAttributeValues::Float32x2(uv)) = strip.attribute(Mesh::ATTRIBUTE_UV_0) else {
            panic!()
        };
        let Some(VertexAttributeValues::Float32x4(mask)) = strip.attribute(Mesh::ATTRIBUTE_COLOR) else {
            panic!("mask coordinates in the colour")
        };
        for ((p, uv), m) in pos.iter().zip(uv).zip(mask) {
            assert!((1.0 - 1e-4..=1.0 + EDGE_DEPTH + 1e-4).contains(&p[0]), "in the strip: {p:?}");
            assert!((uv[0] - p[0] / 2.0).abs() < 1e-4, "the face's own u: {uv:?} at {p:?}");
            // v = 1 on the pane's edge (x = 1), 0 at the strip's far side.
            assert!((m[1] - (1.0 - (p[0] - 1.0) / EDGE_DEPTH)).abs() < 1e-4, "mask v {m:?} at {p:?}");
            assert!((m[0] - p[1]).abs() < 1e-4, "mask u along the side: {m:?} at {p:?}");
            assert_eq!(m[2], 0.0, "the one kind's tile");
        }
    }
}
