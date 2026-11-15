//! Halo's guns as the engine sees them in hand, from a map file
//! (`looks`): each gun's first-person view model (`view_model`) and its
//! third-person held model (`held_model`), named by the weapon documents
//! (`[models] view` and `world`; the first-person animation graph is the
//! weapon tag's, the tag the `halo_ce:weapon` table names), on Halo's maps
//! (`add_own`) and, from the donor map (`body::load::DONOR_MAP`, read once
//! per app), on any other game's (`GUEST`, which dresses the Spartan
//! body too).
//!
//! **The view model** (animations.md "The graph and its models", models.md
//! "Which models the game draws where"): one skinned model whose skeleton
//! is the first-person graph's node list (parents from the graph), the
//! weapon's first-person model and the globals' first-person arms
//! (`globals::Globals::first_person_arms`) each matched to it by node name
//! (exact; a model node without a namesake: no view model, as in the
//! game). A bone's bind pose is its namesake's in its own model (each
//! model's vertices are in its own model space, skinned to its own
//! nodes), made local to the graph's parent. The root is the camera
//! (graph node 0 composed with the camera's frame): Halo's axes (+X
//! forward, +Y left, +Z up) to the eye's (-Z forward, -X left, +Y up),
//! world units to metres (`body::skeleton::root`). Textures through the
//! model shader's approximation (`maps::apply_looks`). The animation set
//! (`build_set`): bind pose as sequence 0, then one sequence per filled
//! first-person slot named by the slot's label (`machine::sequence_name`),
//! its frames sampled (`body::anims::convert`, no checksum: first-person
//! animations are sampled without a model), its loop and key frames in
//! events, its kind (base, overlay, replacement) in its activity.
//!
//! **For effects**: the view model's attachments are the first-person
//! weapon model's markers by their Halo names, on their nodes' bones in
//! the skeleton's units: index 0 is always `primary trigger` (the muzzle:
//! there is no marker called "muzzle"), then `primary ejection` where the
//! gun has one (the assault rifle and the pistol: where casings leave),
//! then the rest in the model's (name) order; a marker with several
//! instances (the plasma rifle's `vent`) gives one attachment per
//! instance, the same name. The held model's `muzzle` is its `primary
//! trigger`, and its `attachments` all its markers by name, in the hand
//! bone's frame.
//!
//! **Objects** (`object_model`, keyed `object_key(tag)`): the projectiles
//! every gun fires and the two grenades, the guns as items and every
//! equipment tag, each its object's model in its own frame (Halo's axes,
//! world units, as the Spartan's skeleton space is) with its markers, as
//! held models the engine draws as shown items (`map::loose::ShownItem`:
//! a rocket or needle in flight, a grenade, an item on the ground).
//!
//! **The held model**: the weapon's third-person model, rigid (its bind
//! pose), on the Spartan's bone that carries the hand marker the
//! Spartan's graph names for the gun's label (the `stand` seat's weapon
//! class: `right hand`), placed by that marker (the class's grip marker
//! is empty on the disc: the weapon's origin is the grip).
//!
//! Known shortcuts:
//! - The assault rifle's ammunition counter and compass (shader-driven
//!   numbers on the gun) show their static textures.
//! - The held model doesn't animate its own nodes (the pistol's slide, the
//!   shotgun's pump: the weapon's own graph).

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use bevy::prelude::{Mat4, Quat, Transform, Vec3};
use blam_formats::{
    animation::{AnimationGraph, AnimationType, FirstPersonSlot},
    cache::{MapFile, TagRef, class},
    globals::Globals,
    model::Model,
    object::Biped,
    weapon::Weapon,
};

use super::super::{
    body::{
        anims::{BIND, LOOP_EVENT, convert},
        load::{BIPED, DONOR_MAP},
        skeleton::{self, bind_locals, globals, usual},
    },
    maps::{
        self,
        objects::{self, Dest, Pending},
        space,
    },
    weapons::{DATA, GUNS, WEAPON_HOOK},
};
use super::machine::{BASE, KEY_EVENT, OVERLAY, REPLACEMENT, sequence_name};
use crate::map::{
    LoadContext, MapAttachment, MapBone, MapData, MapDressing, MapHeldModel, MapLoadOptions, MapMesh, MapModel,
    MapTexture, MapViewModel,
    anim::{AnimEvent, AnimSet, Animation, Sequence},
};

/// The muzzle and ejection markers (models.md, "Markers").
pub const MUZZLE: &str = "primary trigger";
pub const EJECTION: &str = "primary ejection";

/// The globals tag.
const GLOBALS: &str = "globals\\globals";

/// The four guns' view and held models and their textures (meshes'
/// texture indices point into `textures`).
#[derive(Clone, Debug, Default)]
pub struct Looks {
    pub view_models: Vec<MapViewModel>,
    pub held: Vec<MapHeldModel>,
    pub textures: Vec<MapTexture>,
    pub warnings: Vec<String>,
}

/// What a gun's document names: its weapon tag, first- and third-person
/// models.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GunModels {
    pub id: &'static str,
    pub tag: String,
    pub view: String,
    pub world: String,
}

/// Each gun's model paths, from its weapon document.
pub fn gun_models() -> Vec<GunModels> {
    let set =
        chimera_sim::weapon::data::WeaponSet::parse(DATA, &[]).unwrap_or_else(|e| panic!("Halo weapon data: {e}"));
    GUNS.iter()
        .filter_map(|id| {
            let d = set.get(id)?;
            let tag = d.hooks.get(WEAPON_HOOK)?.get("tag")?.as_str()?.to_string();
            Some(GunModels {
                id,
                tag,
                view: d.models.view.clone()?,
                world: d.models.world.clone()?,
            })
        })
        .collect()
}

fn read_model(file: &MapFile, r: TagRef, what: &str) -> Result<Model, String> {
    let t = file.tag(r.id).ok_or_else(|| format!("no {what}"))?;
    Model::read(file, t).map_err(|e| format!("{what}: {e}"))
}

fn model_at(file: &MapFile, path: &str) -> Result<Model, String> {
    let t = file
        .find(class::MODEL, path)
        .ok_or_else(|| format!("no model `{path}`"))?;
    Model::read(file, t).map_err(|e| format!("`{path}`: {e}"))
}

/// The first-person arms model (the globals' first person interface).
pub fn arms(file: &MapFile) -> Result<Model, String> {
    let t = file.find(class::GLOBALS, GLOBALS).ok_or("no globals")?;
    let g = Globals::read(file, t).map_err(|e| e.to_string())?;
    read_model(
        file,
        g.first_person_arms.ok_or("no first-person arms in the globals")?,
        "first-person arms",
    )
}

/// A model's bind pose in its own space (Halo's axes, world units), each
/// node's rotation and position.
fn model_globals(model: &Model) -> Vec<(Quat, Vec3)> {
    let bones = skeleton::bones(model);
    globals(&bones, &bind_locals(&bones))
}

/// The view model's skeleton (module doc): the graph's nodes, each bound
/// where its namesake is in `models`.
pub fn graph_bones(graph: &AnimationGraph, models: &[&Model]) -> Vec<MapBone> {
    let parents = graph.parents();
    let posed: Vec<Vec<(Quat, Vec3)>> = models.iter().map(|m| model_globals(m)).collect();
    let mut global: Vec<(Quat, Vec3)> = Vec::with_capacity(graph.nodes.len());
    let mut bones = Vec::with_capacity(graph.nodes.len());
    for (i, node) in graph.nodes.iter().enumerate() {
        let parent = parents[i];
        let parent_global = parent
            .and_then(|p| global.get(p).copied())
            .unwrap_or((Quat::IDENTITY, Vec3::ZERO));
        let own = models.iter().zip(&posed).find_map(|(m, g)| {
            let n = m.nodes.iter().position(|n| n.name == node.name)?;
            g.get(n).copied()
        });
        let g = own.unwrap_or(parent_global);
        let inv = parent_global.0.inverse();
        bones.push(MapBone {
            name: node.name.clone(),
            parent,
            position: inv * (g.1 - parent_global.1),
            rotation: (inv * g.0).normalize(),
        });
        global.push(g);
    }
    bones
}

/// The first-person animation set (module doc).
pub fn build_set(graph: &AnimationGraph, bones: &[MapBone]) -> (AnimSet, Vec<String>) {
    let defaults = bind_locals(bones);
    let mut set = AnimSet {
        defaults: defaults.clone(),
        bases: vec![defaults],
        parents: bones.iter().map(|b| b.parent).collect(),
        bone_names: bones.iter().map(|b| b.name.to_ascii_lowercase()).collect(),
        ..Default::default()
    };
    set.animations.push(Animation {
        name: BIND.into(),
        fps: 30.0,
        frames: 1,
        ..Default::default()
    });
    set.sequences.push(Sequence {
        name: BIND.into(),
        looping: true,
        anims: vec![0],
        grid: (1, 1),
        ..Default::default()
    });
    set.names.insert(BIND.into(), 0);
    let mut warnings = Vec::new();
    let Some(table) = graph.first_person_weapons.first() else {
        warnings.push("no first-person table".into());
        return (set, warnings);
    };
    let mut converted: BTreeMap<usize, Option<usize>> = BTreeMap::new();
    for &slot in FirstPersonSlot::ALL {
        let Some(index) = table.animations.get(slot) else {
            continue;
        };
        let Some(stored) = graph.animation(index) else { continue };
        let a = *converted
            .entry(index)
            .or_insert_with(|| match convert(graph, index, bones.len(), 0) {
                Ok(a) => {
                    set.animations.push(a);
                    Some(set.animations.len() - 1)
                }
                Err(e) => {
                    warnings.push(format!("{}: {e}", slot.label()));
                    None
                }
            });
        let Some(a) = a else { continue };
        let kind = match stored.kind {
            AnimationType::Base => BASE,
            AnimationType::Overlay => OVERLAY,
            AnimationType::Replacement => REPLACEMENT,
        };
        let event = |name: &str, n: i16| AnimEvent {
            cycle: 0.0,
            event: i32::from(n),
            name: name.into(),
            options: String::new(),
        };
        set.sequences.push(Sequence {
            name: sequence_name(slot).into(),
            activity: kind.into(),
            looping: stored.loop_frame > 0,
            anims: vec![a],
            grid: (1, 1),
            events: vec![event(LOOP_EVENT, stored.loop_frame), event(KEY_EVENT, stored.key_frame)],
            ..Default::default()
        });
        set.names.insert(sequence_name(slot).into(), set.sequences.len() - 1);
    }
    (set, warnings)
}

/// The markers of `model` as attachments on `bone_of(node)` (module doc:
/// the muzzle first, the ejection port second).
fn attachments(model: &Model, bone_of: &dyn Fn(usize) -> Option<usize>) -> Vec<MapAttachment> {
    let rank = |n: &str| match n {
        MUZZLE => 0,
        EJECTION => 1,
        _ => 2,
    };
    let mut markers: Vec<_> = model.markers.iter().collect();
    markers.sort_by_key(|m| rank(m.name.to_ascii_lowercase().as_str()));
    markers
        .into_iter()
        .flat_map(|m| m.instances.iter().map(move |i| (m, i)))
        .filter_map(|(m, i)| {
            Some(MapAttachment {
                name: m.name.clone(),
                bone: bone_of(i.node)?,
                local: Transform {
                    translation: Vec3::from(i.translation),
                    rotation: usual(i.rotation),
                    scale: Vec3::ONE,
                },
            })
        })
        .collect()
}

/// A model's meshes skinned to the graph's nodes (`graph_node`: a model
/// node's graph index), in the eye's space.
fn skinned_on_graph(file: &MapFile, model: &Model, graph_node: &[Option<usize>]) -> Vec<(MapMesh, TagRef)> {
    let mut found = objects::skinned_meshes(file, model, &|p| space::point(p));
    for (m, _) in &mut found {
        for j in &mut m.joints {
            for k in j.iter_mut() {
                *k = graph_node.get(*k as usize).copied().flatten().unwrap_or(0) as u16;
            }
        }
    }
    found
}

/// A model, its meshes waiting for their looks, and warnings.
pub type Built<T> = (T, Vec<(MapMesh, TagRef)>, Vec<String>);

/// A gun's view model without its meshes' looks: the model and its
/// pending meshes.
pub fn view_model(file: &MapFile, gun: &GunModels, arms: &Model) -> Result<Built<MapViewModel>, String> {
    let wt = file
        .find(class::WEAPON, &gun.tag)
        .ok_or_else(|| format!("no weapon `{}`", gun.tag))?;
    let weapon = Weapon::read(file, wt).map_err(|e| e.to_string())?;
    let gt = file
        .tag(weapon.own.first_person_animations.id)
        .ok_or("no first-person animations")?;
    let graph = AnimationGraph::read(file, gt).map_err(|e| e.to_string())?;
    let model = model_at(file, &gun.view)?;
    let mut warnings = Vec::new();
    if file.tag(weapon.own.first_person_model.id).map(|t| t.path.as_str()) != Some(gun.view.as_str()) {
        warnings.push(format!(
            "the document's view model `{}` is not the weapon tag's",
            gun.view
        ));
    }
    let (on_gun, on_arms) = (graph.match_model(&model), graph.match_model(arms));
    if !on_gun.is_complete() || !on_arms.is_complete() {
        return Err(format!(
            "{}: a node of the first-person models is not in its graph (not shown in first person)",
            gun.id
        ));
    }
    let bones = graph_bones(&graph, &[arms, &model]);
    let (set, left_out) = build_set(&graph, &bones);
    warnings.extend(left_out);
    let mut meshes = skinned_on_graph(file, arms, &on_arms.graph_nodes);
    meshes.extend(skinned_on_graph(file, &model, &on_gun.graph_nodes));
    let bone_of = |n: usize| on_gun.graph_nodes.get(n).copied().flatten();
    Ok((
        MapViewModel {
            key: gun.id.to_string(),
            model: MapModel::default(),
            bones,
            root: skeleton::root(),
            animations: Some(Arc::new(set)),
            right_handed: true,
            allow_flipping: false,
            attachments: attachments(&model, &bone_of),
            light_origin: Vec3::ZERO,
        },
        meshes,
        warnings,
    ))
}

/// The Spartan's model and graph (the hand marker's).
fn spartan(file: &MapFile) -> Result<(Model, AnimationGraph), String> {
    let t = file.find(class::BIPED, BIPED).ok_or("no Spartan")?;
    let b = Biped::read(file, t).map_err(|e| e.to_string())?;
    let model = read_model(file, b.object.model, "the Spartan's model")?;
    let gt = file.tag(b.object.animation_graph.id).ok_or("no Spartan graph")?;
    Ok((model, AnimationGraph::read(file, gt).map_err(|e| e.to_string())?))
}

/// Where `label`'s weapon sits on the Spartan: the hand marker's bone name
/// and its frame there (Halo's axes, world units).
pub fn hand(model: &Model, graph: &AnimationGraph, label: &str) -> Option<(String, Mat4)> {
    let sel = graph.select_unit("stand", label)?;
    let class = graph.weapon_class(sel)?;
    let i = model.find_marker(&class.hand_marker)?.instances.first()?;
    let node = model.nodes.get(i.node)?;
    Some((
        node.name.clone(),
        Mat4::from_rotation_translation(usual(i.rotation), Vec3::from(i.translation)),
    ))
}

/// A gun's held model without its meshes' looks.
fn held_model(
    file: &MapFile,
    gun: &GunModels,
    body: &(Model, AnimationGraph),
    label: &str,
) -> Result<(MapHeldModel, Vec<(MapMesh, TagRef)>), String> {
    let model = model_at(file, &gun.world)?;
    let (bone, grip) =
        hand(&body.0, &body.1, label).ok_or_else(|| format!("{}: no hand marker for `{label}`", gun.id))?;
    let from_game = Quat::from_mat3(&bevy::math::Mat3::from_cols(
        space::direction([1.0, 0.0, 0.0]),
        space::direction([0.0, 1.0, 0.0]),
        space::direction([0.0, 0.0, 1.0]),
    ))
    .inverse();
    let (_, grip_q, _) = grip.to_scale_rotation_translation();
    let mut meshes = objects::model_meshes(file, &model, &|p| grip.transform_point3(Vec3::from(p)));
    for (m, _) in &mut meshes {
        for n in &mut m.normals {
            *n = (grip_q * (from_game * Vec3::from(*n))).to_array();
        }
    }
    let posed = model_globals(&model);
    let marker = |name: &str, i: &blam_formats::model::MarkerInstance| {
        let (q, p) = posed.get(i.node).copied()?;
        let local = Mat4::from_rotation_translation(q, p)
            * Mat4::from_rotation_translation(usual(i.rotation), Vec3::from(i.translation));
        Some((name.to_string(), Transform::from_matrix(grip * local)))
    };
    let all: Vec<(String, Transform)> = model
        .markers
        .iter()
        .flat_map(|m| m.instances.iter().map(move |i| (m.name.as_str(), i)))
        .filter_map(|(n, i)| marker(n, i))
        .collect();
    Ok((
        MapHeldModel {
            key: gun.id.to_string(),
            model: MapModel::default(),
            bone,
            muzzle: all
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(MUZZLE))
                .map(|(_, t)| *t),
            attachments: all,
            merged: Vec::new(),
            own: None,
        },
        meshes,
    ))
}

/// The key a projectile's model is drawn by in flight
/// (`map::loose::ShownItem`), by its tag's path.
pub fn object_key(tag: &str) -> String {
    format!("{OBJECT_PREFIX}{}", tag.to_ascii_lowercase())
}

/// The key an item's model is drawn by lying on the map, by its tag's path
/// (a frag grenade is both a projectile and an item: two keys, one model).
pub fn item_key(tag: &str) -> String {
    format!("{ITEM_PREFIX}{}", tag.to_ascii_lowercase())
}

pub const OBJECT_PREFIX: &str = "halo_ce:obj:";
pub const ITEM_PREFIX: &str = "halo_ce:item:";

/// The model of the object tag at `path` (a projectile, equipment or a
/// weapon lying in the world), in its own frame (Halo's axes, world
/// units: the Spartan's skeleton space, which a shown item's root takes to
/// metres), with its markers.
fn object_model(file: &MapFile, key: String, path: &str) -> Option<(MapHeldModel, Vec<(MapMesh, TagRef)>)> {
    let tag = [class::PROJECTILE, class::EQUIPMENT, class::WEAPON].into_iter().find_map(|c| file.find(c, path))?;
    let object = blam_formats::object::ObjectBase::read(file, tag).ok()?;
    let model = read_model(file, object.model, path).ok()?;
    let from_game = Quat::from_mat3(&bevy::math::Mat3::from_cols(
        space::direction([1.0, 0.0, 0.0]),
        space::direction([0.0, 1.0, 0.0]),
        space::direction([0.0, 0.0, 1.0]),
    ))
    .inverse();
    let mut meshes = objects::model_meshes(file, &model, &|p| Vec3::from(p));
    if meshes.is_empty() {
        return None;
    }
    for (m, _) in &mut meshes {
        for n in &mut m.normals {
            *n = (from_game * Vec3::from(*n)).to_array();
        }
    }
    let posed = model_globals(&model);
    let attachments = model
        .markers
        .iter()
        .filter_map(|m| {
            let i = m.instances.first()?;
            let (q, p) = posed.get(i.node).copied()?;
            let local = Mat4::from_rotation_translation(q, p) * Mat4::from_rotation_translation(usual(i.rotation), Vec3::from(i.translation));
            Some((m.name.clone(), Transform::from_matrix(local)))
        })
        .collect();
    Some((
        MapHeldModel {
            key,
            model: MapModel::default(),
            bone: String::new(),
            muzzle: None,
            attachments,
            merged: Vec::new(),
            own: None,
        },
        meshes,
    ))
}

/// The objects drawn as shown items, by key and tag: the projectiles
/// every gun fires and the grenades (in flight, `object_key`), the guns and
/// every equipment tag (lying on the map, `item_key`).
pub fn object_tags(file: &MapFile) -> Vec<(String, String)> {
    let guns: Vec<String> = gun_models().into_iter().map(|g| g.tag).collect();
    let refs: Vec<&str> = guns.iter().map(String::as_str).collect();
    let flying = super::super::fx::load::projectile_tags(file, &refs).into_iter().map(|t| (object_key(&t), t));
    let lying = guns
        .iter()
        .map(|g| g.to_ascii_lowercase())
        .chain(file.of_class(class::EQUIPMENT).map(|t| t.path.to_ascii_lowercase()))
        .map(|t| (item_key(&t), t));
    let mut all: Vec<(String, String)> = flying.chain(lying).collect();
    all.sort();
    all.dedup();
    all
}

fn bounds(meshes: &[MapMesh]) -> (Vec3, Vec3) {
    let (lo, hi) = meshes
        .iter()
        .flat_map(|m| &m.positions)
        .fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), p| {
            (lo.min((*p).into()), hi.max((*p).into()))
        });
    if lo.x <= hi.x { (lo, hi) } else { Default::default() }
}

/// The four guns' looks from map `file` (module doc); `decode` false:
/// textures as placeholders of their size. A gun that fails is left out
/// with a warning.
pub fn looks(file: &MapFile, decode: bool) -> Looks {
    let mut out = Looks::default();
    let arms = match arms(file) {
        Ok(a) => Some(a),
        Err(e) => {
            out.warnings.push(format!("no first-person arms: {e}"));
            None
        }
    };
    let body = spartan(file)
        .map_err(|e| out.warnings.push(format!("no hand marker: {e}")))
        .ok();
    let mut pending = Vec::new();
    let labels = super::labels();
    for gun in &gun_models() {
        if let Some(arms) = &arms {
            match view_model(file, gun, arms) {
                Ok((v, meshes, w)) => {
                    out.warnings.extend(w.into_iter().map(|w| format!("{}: {w}", gun.id)));
                    out.view_models.push(v);
                    let i = out.view_models.len() - 1;
                    pending.extend(meshes.into_iter().map(|(mesh, shader)| Pending {
                        mesh,
                        shader,
                        lit: true,
                        dest: Dest::Model(i),
                    }));
                }
                Err(e) => out.warnings.push(e),
            }
        }
        let label = labels
            .iter()
            .find(|(id, _)| *id == gun.id)
            .map_or("", |(_, l)| l.as_str());
        if let Some(body) = &body {
            match held_model(file, gun, body, label) {
                Ok((h, meshes)) => {
                    out.held.push(h);
                    let i = 1000 + out.held.len() - 1;
                    pending.extend(meshes.into_iter().map(|(mesh, shader)| Pending {
                        mesh,
                        shader,
                        lit: true,
                        dest: Dest::Model(i),
                    }));
                }
                Err(e) => out.warnings.push(e),
            }
        }
    }
    // The objects drawn as shown items (projectiles, items).
    for (key, tag) in object_tags(file) {
        if let Some((h, meshes)) = object_model(file, key, &tag) {
            out.held.push(h);
            let i = 1000 + out.held.len() - 1;
            pending.extend(meshes.into_iter().map(|(mesh, shader)| Pending {
                mesh,
                shader,
                lit: true,
                dest: Dest::Model(i),
            }));
        }
    }
    for (mesh, dest) in maps::apply_looks(file, pending, &mut out.textures, &mut out.warnings, decode) {
        match dest {
            Dest::Model(i) if i >= 1000 => out.held[i - 1000].model.meshes.push(mesh),
            Dest::Model(i) => out.view_models[i].model.meshes.push(mesh),
            _ => {}
        }
    }
    for v in &mut out.view_models {
        v.model.bounds = bounds(&v.model.meshes);
    }
    for h in &mut out.held {
        h.model.bounds = bounds(&h.model.meshes);
    }
    out
}

/// Move a model's texture indices past `offset`.
fn shift(model: &mut MapModel, offset: usize) {
    let shift = |t: &mut Option<usize>| {
        if let Some(i) = t {
            *i += offset;
        }
    };
    for m in &mut model.meshes {
        shift(&mut m.texture);
        shift(&mut m.normal_map);
        if let Some(d) = &mut m.detail {
            d.texture += offset;
        }
    }
}

/// `looks` added to `map`'s kit (those its load wants), its textures
/// appended to the map's.
pub fn add(map: &mut MapData, looks: &Looks, options: &MapLoadOptions) {
    let offset = map.textures.len();
    for v in looks.view_models.iter().filter(|v| options.kit.wants(&v.key)) {
        let mut v = v.clone();
        shift(&mut v.model, offset);
        map.kit.view_models.push(v);
    }
    for h in looks.held.iter().filter(|h| options.kit.wants(&h.key)) {
        let mut h = h.clone();
        shift(&mut h.model, offset);
        map.kit.held.push(h);
    }
    map.textures.extend(looks.textures.iter().cloned());
    map.warnings
        .extend(looks.warnings.iter().map(|w| format!("Halo's guns: {w}")));
}

/// The guns' looks of a Halo map's own file added to it.
pub fn add_own(file: &MapFile, map: &mut MapData, options: &MapLoadOptions) {
    if options.kit.any_models() {
        add(map, &looks(file, options.textures), options);
    }
}

/// The donor's looks an app's loads share (`LoadContext`): with and
/// without textures.
#[derive(Default)]
struct Donor(Mutex<[Option<Arc<Looks>>; 2]>);

/// The donor map's looks for an app (made once per app and `decode`).
pub fn donor(context: &LoadContext, decode: bool) -> Result<Arc<Looks>, String> {
    let memo = context.get_or_init(Donor::default);
    let mut slots = memo.0.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(l) = &slots[usize::from(decode)] {
        return Ok(l.clone());
    }
    let mount = maps::mount_for(context)?;
    let path = maps::map_path(DONOR_MAP);
    let bytes = mount.read(&path).map_err(|e| format!("{path}: {e}"))?;
    let file = MapFile::load(&bytes).map_err(|e| format!("{path}: {e}"))?;
    let l = Arc::new(looks(&file, decode));
    slots[usize::from(decode)] = Some(l.clone());
    Ok(l)
}

/// The Spartan and Halo's guns on another game's map (the donor's).
pub fn dress(map: &mut MapData, options: &MapLoadOptions) -> Result<(), String> {
    super::super::body::load::dress(map, options)?;
    let looks = donor(&options.context, options.textures)?;
    add(
        map,
        &looks,
        &MapLoadOptions {
            kit: crate::map::KitLoad::All,
            ..options.clone()
        },
    );
    Ok(())
}

/// Halo's players and guns on the maps of other games (`game::BUNDLE`'s
/// guest dressing).
pub static GUEST: MapDressing = MapDressing {
    game: super::super::GAME,
    dress,
};
