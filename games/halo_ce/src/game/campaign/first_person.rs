//! A campaign body's first-person view made from its own third-person
//! model (the owner's idea, 2026-11-16; characters.md section 7: no
//! character has first-person animations, and the Hunter's cannon has no
//! first-person model): the part of the body the player would see, the
//! arm that holds the gun, drawn in the first-person view as a gun's view
//! model is, posed by the body's own animations. The Hunter's is the one
//! made now (`BodyDef::first_person` in data/campaign.toml: its right arm
//! with the cannon); the seam is general, so a later character can show
//! its own hands the same way.
//!
//! **The model** (`view_model`, made with the body's unit and kept in
//! `BodyViews`, added to each map's view models by `add_to_maps`, since a
//! late view model goes with its map): the body's character model with
//! only the triangles whose vertices are skinned, by half their weight or
//! more, to a kept bone (`BodyView::keep`, each with every bone below it);
//! the rest is left out (no scaled bones: the engine's pose has none), and
//! the meshes are moved from the character's frame to the eye's (a view
//! model's meshes are in the eye's space). Its skeleton, bones and
//! animation set are the body's. Its root takes the
//! skeleton's space to the eye's: the body's own root (Halo's axes, world
//! units to metres, feet at the origin), then down by the eye's height
//! (the body's standing camera), turned by `BodyView::turn` and moved by
//! `BodyView::offset` (metres in the eye's axes: +X right, +Y up, -Z
//! forward): the data that sets where the gun arm sits in the view. Its
//! key is the gun's (`BodyView::guns`), so the engine shows it as that
//! gun's view model (`map::ViewAnimator::show`); Halo's own first-person
//! machine (`view::drive`) leaves such a gun alone.
//!
//! **The pose** (`drive`, after `view::drive`, each Halo tick): the
//! body's idle for the gun's label (`stand <label> idle`, looping), its
//! level aim (`aim-still` at yaw and pitch 0: the view turns with the
//! camera, so the arm keeps its aim in the view), and over it the body's
//! own fire (`fire-1`, an overlay: composed on the idle) on a shot and its
//! melee (`melee`, a base animation: in place of the idle) on a melee,
//! each played once from its first frame (a new one restarts it,
//! `anims::apply_action`). The Hunter's view keeps its cannon from the
//! elbow down (tuned from pictures; data/campaign.toml keeps two
//! alternates, one with the shield forearm, whose bash is its melee).
//!
//! Known shortcuts: its fire-1 overlay swings the cannon up and to the
//! right, out of the view's lower right for some frames; the triangles that cross from a kept bone to a left
//! one (the shoulder's) are cut at the weight rule, so the arm's edge is
//! open; the view keeps the body's own textures and their tint (the
//! player's colour) as the third-person body has them for a player given
//! none (`skins`' last); the muzzle is a fixed point in the eye's space
//! (`BodyView::muzzle`, data), not a marker of this model (it has no
//! attachments): it does not follow the arm's fire or melee.

use std::{collections::BTreeMap, sync::Arc};

use bevy::prelude::*;
use chimera_core::later::Later;
use chimera_sim::weapon::{Inventory, Weapon, WeaponEvent, WeaponEventKind};
use serde::Deserialize;

use super::super::body::anims::{self, base_pose, key};
use crate::map::{MapCharacterModel, MapTexture, MapViewModel, ViewAnimator, ViewModels, add_view_model, anim::BonePose};

/// A body's first-person view (`BodyDef::first_person`).
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyView {
    /// The guns whose view it is (its view model's keys).
    pub guns: Vec<String>,
    /// The bones kept (names; each with every bone below it).
    pub keep: Vec<String>,
    /// The label of the gun class its animations are under (`stand
    /// <label> idle`).
    pub label: String,
    /// Metres in the eye's axes (+X right, +Y up, -Z forward), after the
    /// turn.
    #[serde(default)]
    pub offset: [f32; 3],
    /// Degrees: yaw (about up, to the left), pitch (about right, up) and
    /// roll, about the eye.
    #[serde(default)]
    pub turn: [f32; 3],
    /// Where its gun's muzzle shows, world units from the eye in Halo's
    /// axes (forward, left, up): its firing effects and muzzle light are
    /// placed there (the gun's first-person marker, `fx::load::own_weapons`)
    /// and its shots are drawn leaving from there (the trace stays the
    /// eye's). None: the eye.
    #[serde(default)]
    pub muzzle: Option<[f32; 3]>,
}

/// The bones `keep` names and every bone below them.
pub fn kept(bones: &[crate::map::MapBone], keep: &[String]) -> Vec<bool> {
    let mut out = vec![false; bones.len()];
    for (i, b) in bones.iter().enumerate() {
        let named = keep.iter().any(|k| k.eq_ignore_ascii_case(&b.name));
        // Parents come before their children.
        out[i] = named || b.parent.is_some_and(|p| out.get(p).copied().unwrap_or(false));
    }
    out
}

/// The root from the body's skeleton space to the eye's (module doc):
/// `body_root` (the character's), the eye `eye` metres above the feet.
pub fn root(body_root: Transform, eye: f32, view: &BodyView) -> Transform {
    let [yaw, pitch, roll] = view.turn.map(f32::to_radians);
    let turn = Quat::from_euler(EulerRot::YXZ, yaw, pitch, roll);
    let m = Mat4::from_translation(Vec3::from(view.offset))
        * Mat4::from_quat(turn)
        * Mat4::from_translation(-Vec3::Y * eye)
        * body_root.to_matrix();
    Transform::from_matrix(m)
}

/// Body `model` as the first-person view of gun `key` (module doc), the
/// eye `eye` metres above its feet.
pub fn view_model(model: &MapCharacterModel, view: &BodyView, key: &str, eye: f32) -> MapViewModel {
    let keep = kept(&model.bones, &view.keep);
    let on_kept = |m: &crate::map::MapMesh, v: usize| -> bool {
        match (m.joints.get(v), m.joint_weights.get(v)) {
            (Some(j), Some(w)) => {
                j.iter().zip(w).filter(|(b, _)| keep.get(usize::from(**b)).copied().unwrap_or(false)).map(|(_, w)| *w).sum::<f32>() >= 0.5
            }
            _ => false,
        }
    };
    let root = root(model.root, eye, view);
    // A view model's meshes are in the eye's space, a character's in its
    // own frame (its root applied): moved by the difference.
    let to_eye = root.to_matrix() * model.root.to_matrix().inverse();
    let mut body = model.model.clone();
    let mut left = Vec::new();
    for (i, m) in body.meshes.iter_mut().enumerate() {
        for p in &mut m.positions {
            *p = to_eye.transform_point3(Vec3::from(*p)).to_array();
        }
        for n in &mut m.normals {
            *n = to_eye.transform_vector3(Vec3::from(*n)).normalize_or_zero().to_array();
        }
        let tris: Vec<u32> = m
            .indices
            .chunks_exact(3)
            .filter(|t| t.iter().all(|v| on_kept(m, *v as usize)))
            .flatten()
            .copied()
            .collect();
        m.indices = tris;
        if m.indices.is_empty() {
            left.push(i);
        }
    }
    // The meshes with nothing kept go, with their looks and names.
    let gone = |i: &usize| left.binary_search(i).is_ok();
    let mut i = 0;
    body.meshes.retain(|_| {
        i += 1;
        !gone(&(i - 1))
    });
    for skin in &mut body.skins {
        let mut i = 0;
        skin.retain(|_| {
            i += 1;
            !gone(&(i - 1))
        });
    }
    let mut i = 0;
    body.mesh_names.retain(|_| {
        i += 1;
        !gone(&(i - 1))
    });
    // A player given no colour: the body's last skin (`body::load`).
    if let Some(last) = body.skins.last().cloned() {
        for (m, look) in body.meshes.iter_mut().zip(&last) {
            m.tint = look.tint;
            m.texture = look.texture;
        }
    }
    body.skins.clear();
    body.lods.clear();
    body.body_parts.clear();
    for m in &mut body.meshes {
        m.body = None;
    }
    MapViewModel {
        key: key.to_string(),
        model: body,
        bones: model.bones.clone(),
        root,
        animations: model.animations.clone(),
        right_handed: true,
        allow_flipping: false,
        attachments: Vec::new(),
        light_origin: Vec3::ZERO,
    }
}

/// The body views made (module doc): each with its textures and the body
/// view's data, by key.
#[derive(Resource, Clone, Default)]
pub struct BodyViews(pub BTreeMap<String, (Arc<MapViewModel>, Arc<Vec<MapTexture>>, BodyView)>);

impl BodyViews {
    /// Whether gun `key` is drawn by a body view.
    pub fn has(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }
}

/// Every machine: the body views the loaded map's view models lack, added
/// (a late view model goes with its map).
pub fn add_to_maps(views: Option<Res<BodyViews>>, models: Option<Res<ViewModels>>, mut commands: Commands) {
    let (Some(views), Some(models)) = (views, models) else { return };
    for (m, t, _) in views.0.values().filter(|(m, _, _)| models.get(&m.key).is_none()) {
        debug!("halo_ce: the body view {} joins the map's view models ({} meshes)", m.key, m.model.meshes.len());
        let (m, t) = (m.clone(), t.clone());
        commands.queue(move |world: &mut World| add_view_model(world, m, t));
    }
}

/// A holder's body view playing (`drive`): the gun, the idle's frame and
/// the action over it (its sequence and frame).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Playing {
    pub key: String,
    pub frame: usize,
    pub action: Option<(usize, usize)>,
}

/// Each holder's (`drive`).
#[derive(Resource, Clone, Debug, Default)]
pub struct BodyViewPlay(pub BTreeMap<Entity, Playing>);

/// The pose of a body view (module doc): the idle `idle` at `frame`, the
/// action `(sequence, frame)` over it (an overlay composed on it, a base
/// animation in place of it on the bones it tracks), then the level aim
/// `aim`.
pub fn pose(set: &crate::map::anim::AnimSet, idle: Option<usize>, frame: usize, action: Option<(usize, usize)>, aim: Option<usize>) -> Vec<BonePose> {
    let mut pose = match idle {
        Some(s) => base_pose(set, set.sequences[s].anims[0], frame),
        None => set.defaults.clone(),
    };
    if let Some(a) = action {
        anims::apply_action(set, a, &mut pose);
    }
    if let Some(a) = aim {
        anims::apply_screen(set, a, 0.0, 0.0, &mut pose);
    }
    pose
}

/// Every machine, each Halo tick: the pose of each holder's body view
/// (module doc).
pub fn drive(
    clock: Res<crate::core::SimClock>,
    views: Option<Res<BodyViews>>,
    mut playing: ResMut<BodyViewPlay>,
    mut holders: Query<(Entity, &Inventory, &mut ViewAnimator)>,
    guns: Query<&Weapon>,
    mut events: MessageReader<WeaponEvent>,
) {
    let events: Vec<&WeaponEvent> = events.read().filter(|e| e.shown()).collect();
    let Some(views) = views else { return };
    let n = super::super::weapons::halo_ticks(&clock);
    let mut seen = Vec::new();
    for (e, inv, mut view) in &mut holders {
        let Some(id) = inv.active.and_then(|w| guns.get(w).ok()).map(|w| w.id) else { continue };
        let Some((_, _, data)) = views.0.get(id) else { continue };
        let Some(set) = view.animator.as_ref().map(|a| a.set.clone()) else { continue };
        seen.push(e);
        let p = playing.0.entry(e).or_default();
        let find = |slot: &str| set.names.get(&key("stand", &data.label, slot)).copied();
        let (idle, aim, fire, melee) = (find("idle"), find("aim-still"), find("fire-1"), find("melee"));
        if p.key != id {
            *p = Playing { key: id.to_string(), ..Default::default() };
            debug!(
                "halo_ce: {e} sees {id} as its body's arm (view {:?}, idle {idle:?}, aim {aim:?}, fire {fire:?}, melee {melee:?})",
                view.key
            );
        }
        for ev in events.iter().filter(|ev| ev.owner == e) {
            let start = match ev.kind {
                WeaponEventKind::Action("fire" | "fire_last" | "fire_charged") => fire,
                WeaponEventKind::Action("melee") => melee,
                _ => None,
            };
            if let Some(s) = start {
                p.action = Some((s, 0));
            }
        }
        for _ in 0..n {
            if let Some(s) = idle {
                p.frame = anims::advance(p.frame, anims::frames(&set, s), anims::loop_frame(&set, s));
            }
            if let Some((s, f)) = p.action {
                p.action = (f + 1 < anims::frames(&set, s)).then_some((s, f + 1));
            }
        }
        let pose = pose(&set, idle, p.frame, p.action, aim);
        if let Some(a) = view.animator.as_mut() {
            a.fixed = Some(Later::ready(pose));
            a.main = Some(0);
        }
    }
    playing.0.retain(|e, _| seen.contains(e));
}
