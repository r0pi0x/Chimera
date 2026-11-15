//! The scenario's scenery as the engine's props, and models in general
//! (specs/halo_ce/scenario.md, "Object placements by type", "Scenery as a
//! renderer needs it", "Rotation and facing"; models.md; collision.md,
//! "Model collision"): a render model's parts as meshes, the placements
//! as `MapProp`s of reusable `MapModel`s, and each placed object's
//! collision model as static collision triangles.
//!
//! - Models (`model_meshes`): the highest level of detail, the first
//!   permutation of each region, every part's triangles counter-clockwise
//!   from the front, parts of one shader (and shader permutation) merged
//!   into a mesh. The vertices are stored in model space in the bind pose
//!   (models.md, "Skinning": every node's skin transform is the identity
//!   there), so a static object needs no node transform whatever its node
//!   count; positions go through the caller's point conversion (metres
//!   for scenery, the sky's miniature for the sky).
//! - Scenery (`scenery`): every created placement (a palette index in
//!   range and the first structure BSP's on-BSP bit set) of an object tag
//!   with a model is one `MapProp` of the model's `MapModel` (one per
//!   model tag, shared by its placements). The prop sits at the placement's
//!   position (`space::point`) with the placement's axes as the engine
//!   rotation (`placement_rotation`: the spec's forward, left and up
//!   through the world's axis mapping).
//! - Lighting: a prop is lit by one `LightProbe` for the whole map
//!   (`sky::prop_light`), baked per vertex by the renderer.
//! - Collision (`add_collision`): a placed object's collision model, each
//!   node's first BSP (the node's region must be set and the BSP have 3D
//!   nodes), its surface polygons fan-triangulated, taken through the
//!   model's bind-pose transform of the same-numbered render node, then
//!   the placement's axes and position, appended to the map's collision
//!   (glass-material surfaces to the glass set). An object without a
//!   collision model adds none.
//!
//! Known shortcuts:
//! - Every placement uses permutation 0 of every region (the "select by
//!   variant" rule needs the placement's variant; every multiplayer
//!   scenery region has one candidate) and the highest level of detail
//!   (no distance levels).
//! - A scenery object's lighting is one probe for the map, not the
//!   spec's sample of the lightmap under the object ("Static lighting"),
//!   and not per object; no shadows.
//! - Scenery animations (one plant has an animation graph) and the
//!   object's modifier shader and creation effect are not used; the
//!   origin offset (0 on the disc) and bounding offset are ignored.
//! - Collision uses BSP 0 of each node, the sphere queries of the
//!   movement and the traces see it as part of the level.

use std::collections::BTreeMap;

use bevy::math::{Mat3, Quat, Vec3};
use blam_formats::{
    cache::{MapFile, TagRef},
    collision::{self, CollisionModel, MaterialType, Triangle},
    model::{Model, Transform},
    scenario::{ObjectHeader, Orientation, Scenario},
};

use super::{space, surfaces};
use crate::map::{LightProbe, MapCollisionSet, MapData, MapMesh, MapModel, MapProp, PropSolid};

/// Where a pending mesh goes once its look is known.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dest {
    /// `MapData::meshes`, the level's.
    Level,
    /// `MapData::meshes`, marked `skybox`.
    Sky,
    /// `MapData::models[i].meshes`.
    Model(usize),
}

/// A mesh waiting for its shader's look (texture, alpha, tint).
pub struct Pending {
    pub mesh: MapMesh,
    pub shader: TagRef,
    /// Lit (by the lightmap or the prop's light) rather than drawn at the
    /// texture's own brightness.
    pub lit: bool,
    pub dest: Dest,
}

/// A model's parts as meshes (module doc), positions through `point`
/// (model-space world units to the mesh's space).
pub fn model_meshes(file: &MapFile, model: &Model, point: &dyn Fn([f32; 3]) -> Vec3) -> Vec<(MapMesh, TagRef)> {
    meshes_of(file, model, point, false).into_iter().map(|(m, s, _)| (m, s)).collect()
}

/// `model_meshes` with each vertex's joints and weights (its two nodes and
/// weight, models.md "Skinning"): a skinned body's meshes
/// (`body::skeleton::joints`).
pub fn skinned_meshes(file: &MapFile, model: &Model, point: &dyn Fn([f32; 3]) -> Vec3) -> Vec<(MapMesh, TagRef)> {
    meshes_of(file, model, point, true).into_iter().map(|(m, s, _)| (m, s)).collect()
}

/// `skinned_meshes` with each mesh's shader permutation (models.md
/// "Shaders": a numeric shader's digit position, `view::displays`).
pub fn skinned_meshes_permuted(
    file: &MapFile,
    model: &Model,
    point: &dyn Fn([f32; 3]) -> Vec3,
) -> Vec<(MapMesh, TagRef, i16)> {
    meshes_of(file, model, point, true)
}

fn meshes_of(file: &MapFile, model: &Model, point: &dyn Fn([f32; 3]) -> Vec3, skin: bool) -> Vec<(MapMesh, TagRef, i16)> {
    let mut out: Vec<(MapMesh, TagRef, i16)> = Vec::new();
    let mut index: BTreeMap<(u32, i16), usize> = BTreeMap::new();
    for d in model.highest_detail_parts() {
        let Some(shader) = d.shader else { continue };
        let key = (shader.shader.id.0, shader.permutation);
        let i = *index.entry(key).or_insert_with(|| {
            let path = file
                .tag(shader.shader.id)
                .map_or_else(String::new, |t| t.path.replace('\\', "/"));
            out.push((
                MapMesh {
                    material: path,
                    ..Default::default()
                },
                shader.shader,
                shader.permutation,
            ));
            out.len() - 1
        });
        let mesh = &mut out[i].0;
        let base = mesh.positions.len() as u32;
        for v in &d.part.vertices {
            mesh.positions.push(point(v.position).to_array());
            mesh.normals
                .push(space::direction(v.normal).normalize_or_zero().to_array());
            mesh.uvs.push(v.texcoord);
            if skin {
                let (j, w) = super::super::body::skeleton::joints(v);
                mesh.joints.push(j);
                mesh.joint_weights.push(w);
            }
        }
        for t in d.part.triangles_ccw() {
            mesh.indices.extend(t.map(|i| base + i as u32));
        }
    }
    out.retain(|(m, ..)| !m.indices.is_empty());
    out
}

/// The engine rotation of a placement's axes: a vector `v` in the model's
/// engine-space frame (`space::direction` of its Halo-space vector `p`)
/// goes to `space::direction(p.x F + p.y L + p.z U)`. With `A` the matrix
/// of the axes (columns F, L, U) and `M` the axis mapping, it is
/// `M A M^T`.
pub fn placement_rotation(o: &Orientation) -> Quat {
    let m = Mat3::from_cols(
        space::direction([1.0, 0.0, 0.0]),
        space::direction([0.0, 1.0, 0.0]),
        space::direction([0.0, 0.0, 1.0]),
    );
    let a = Mat3::from_cols(Vec3::from(o.forward), Vec3::from(o.left), Vec3::from(o.up));
    Quat::from_mat3(&(m * a * m.transpose())).normalize()
}

/// A model's bounding box over its meshes (metres).
fn bounds(meshes: &[(MapMesh, TagRef)]) -> (Vec3, Vec3) {
    let mut lo = Vec3::splat(f32::MAX);
    let mut hi = Vec3::splat(f32::MIN);
    for (m, _) in meshes {
        for p in &m.positions {
            lo = lo.min(Vec3::from(*p));
            hi = hi.max(Vec3::from(*p));
        }
    }
    if lo.x > hi.x {
        (Vec3::ZERO, Vec3::ZERO)
    } else {
        (lo, hi)
    }
}

struct Collision {
    model: CollisionModel,
    /// Per node: its first BSP's triangles (empty: the node takes no part).
    triangles: Vec<Vec<Triangle>>,
}

fn collision_of(file: &MapFile, r: &TagRef, warnings: &mut Vec<String>) -> Option<Collision> {
    let tag = file.tag(r.id)?;
    let model = match collision::read_model(file, tag) {
        Ok(m) => m,
        Err(e) => {
            warnings.push(format!(
                "collision model `{}` unreadable, the object is not solid: {e}",
                tag.path
            ));
            return None;
        }
    };
    let mut triangles = Vec::with_capacity(model.nodes.len());
    for n in &model.nodes {
        let bsp = n.bsps.first().filter(|b| n.region.is_some() && !b.nodes.is_empty());
        match bsp.map(|b| b.triangulate()) {
            None => triangles.push(Vec::new()),
            Some(Ok(t)) => triangles.push(t),
            Some(Err(e)) => {
                warnings.push(format!(
                    "collision model `{}`: a node's surfaces are unreadable: {e}",
                    tag.path
                ));
                triangles.push(Vec::new());
            }
        }
    }
    Some(Collision { model, triangles })
}

/// What the scenery pass learned about one palette entry.
struct Object {
    /// Index into `MapData::models`, when it has a drawable model.
    model: Option<usize>,
    /// Bind-pose transforms of the render model's nodes.
    bind: Vec<Transform>,
    /// Index into the collision list.
    collision: Option<usize>,
}

/// The scenery pass's caches: models by tag, collision models by tag.
#[derive(Default)]
struct Cache {
    models: BTreeMap<u32, (Option<usize>, Vec<Transform>)>,
    collisions: Vec<Collision>,
    collision_index: BTreeMap<u32, Option<usize>>,
    pending: Vec<Pending>,
    warnings: Vec<String>,
}

impl Cache {
    fn model(&mut self, file: &MapFile, r: &TagRef, map: &mut MapData) -> (Option<usize>, Vec<Transform>) {
        if let Some(c) = self.models.get(&r.id.0) {
            return c.clone();
        }
        let result = match file.tag(r.id) {
            None => (None, Vec::new()),
            Some(mt) => match Model::read(file, mt) {
                Ok(m) => {
                    let meshes = model_meshes(file, &m, &|p| space::point(p));
                    let index = map.models.len();
                    if meshes.is_empty() {
                        self.warnings.push(format!("model `{}` has no drawable parts", mt.path));
                    }
                    map.models.push(MapModel {
                        bounds: bounds(&meshes),
                        ..Default::default()
                    });
                    self.pending.extend(meshes.into_iter().map(|(mesh, shader)| Pending {
                        mesh,
                        shader,
                        lit: true,
                        dest: Dest::Model(index),
                    }));
                    (Some(index), m.bind_pose())
                }
                Err(e) => {
                    self.warnings
                        .push(format!("model `{}` unreadable, its scenery is not drawn: {e}", mt.path));
                    (None, Vec::new())
                }
            },
        };
        self.models.insert(r.id.0, result.clone());
        result
    }

    fn collision(&mut self, file: &MapFile, r: &TagRef) -> Option<usize> {
        if r.id.is_none() {
            return None;
        }
        if let Some(c) = self.collision_index.get(&r.id.0) {
            return *c;
        }
        let made = collision_of(file, r, &mut self.warnings).map(|c| {
            self.collisions.push(c);
            self.collisions.len() - 1
        });
        self.collision_index.insert(r.id.0, made);
        made
    }
}

/// Add every created scenery placement of `scenario` to `map` (props,
/// their models, collision), and return the models' meshes waiting for
/// their looks (`Dest::Model`).
pub fn scenery(file: &MapFile, scenario: &Scenario, light: &LightProbe, map: &mut MapData) -> Vec<Pending> {
    let palette = &scenario.scenery_palette;
    let mut cache = Cache::default();
    let mut objects: BTreeMap<usize, Option<Object>> = BTreeMap::new();
    let mut skipped = 0usize;
    for p in &scenario.scenery {
        let h = &p.header;
        if !h.is_created(palette.len(), Some(0)) {
            continue;
        }
        let Some(pi) = h.palette_entry(palette.len()) else {
            continue;
        };
        if let std::collections::btree_map::Entry::Vacant(slot) = objects.entry(pi) {
            let r = &palette[pi].object;
            let object = file.tag(r.id).and_then(|tag| match ObjectHeader::read(file, tag) {
                Ok(header) => Some(header),
                Err(e) => {
                    cache
                        .warnings
                        .push(format!("scenery `{}` unreadable, not placed: {e}", tag.path));
                    None
                }
            });
            let object = object.map(|header| {
                let (model, bind) = cache.model(file, &header.model, map);
                let collision = cache.collision(file, &header.collision_model);
                Object { model, bind, collision }
            });
            slot.insert(object);
        }
        let Some(info) = objects[&pi].as_ref() else {
            skipped += 1;
            continue;
        };
        let o = h.orientation();
        if let Some(model) = info.model {
            map.props.push(MapProp {
                model,
                pose: None,
                ragdoll: None,
                translation: space::point(h.position),
                rotation: placement_rotation(&o),
                solid: PropSolid::None,
                skybox: false,
                lighting: Some(light.clone()),
                vertex_light: None,
                casts_shadow: false,
                physics: None,
                fade: None,
                parent: None,
                entity: None,
                skin: 0,
                body: 0,
                detail: false,
            });
        }
        if let Some(c) = info.collision {
            add_collision(map, &cache.collisions[c], &info.bind, &o, h.position);
        }
    }
    if skipped > 0 {
        cache
            .warnings
            .push(format!("{skipped} scenery placements of unreadable objects not placed"));
    }
    map.warnings.extend(cache.warnings);
    cache.pending
}

/// Append one placed object's collision (module doc).
fn add_collision(map: &mut MapData, c: &Collision, bind: &[Transform], o: &Orientation, position: [f32; 3]) {
    let mut glass: Vec<[u32; 3]> = Vec::new();
    for (ni, node) in c.model.nodes.iter().enumerate() {
        let tris = &c.triangles[ni];
        let Some(bsp) = node.bsps.first().filter(|_| !tris.is_empty()) else {
            continue;
        };
        let node_transform = bind.get(ni).copied().unwrap_or(Transform::IDENTITY);
        let base = map.collision_positions.len() as u32;
        map.collision_positions.extend(bsp.vertices.iter().map(|v| {
            let m = node_transform.apply_point(v.point);
            let r = o.rotate(m);
            space::point([position[0] + r[0], position[1] + r[1], position[2] + r[2]]).to_array()
        }));
        for t in tris {
            let tri = t.vertices.map(|v| base + v);
            let surface = &bsp.surfaces[t.surface as usize];
            let is_glass = usize::try_from(surface.material)
                .ok()
                .and_then(|m| c.model.materials.get(m))
                .and_then(|m| m.material_type())
                == Some(MaterialType::Glass);
            if is_glass {
                glass.push(tri);
            } else {
                map.collision_indices.push(tri);
                let code = usize::try_from(surface.material).ok().and_then(|m| c.model.materials.get(m)).and_then(|m| m.material_type());
                map.extras.entry::<crate::game::weapons::materials::LevelCodes>().0.push(code.map_or(-1, |t| t.code() as i8));
            }
        }
    }
    if glass.is_empty() {
        return;
    }
    match map
        .collision_sets
        .iter_mut()
        .find(|s| s.name == super::collision::GLASS)
    {
        Some(set) => set.indices.extend(glass),
        None => map.collision_sets.push(MapCollisionSet {
            name: super::collision::GLASS.into(),
            indices: glass,
            characters: true,
            traces: true,
            see_through: true,
            surface: surfaces::surface_name(MaterialType::Glass),
        }),
    }
}
