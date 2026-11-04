//! Loose items lying in the world, for any game (dropped weapons): a held
//! model (`MapHeldModel`) as a small physics body that players walk
//! through (`core::ITEM_LAYER`), like Source's dropped weapons. Shots push
//! it like any physics prop; it slides and tumbles over displacements.
//!
//! Loose items for any game (`LooseItem`: a held model's key; `ShownItem`: the
//! meshes only (drawn by render), for things that move themselves such as
//! grenades; `SitOnOrigin`: resting on its origin, turned, such as the planted
//! bomb): a dynamic box body sized from the held model (also headless), swept
//! against fast moves, on `core::ITEM_LAYER` so characters neither collide with
//! it nor walk into it (`PushAway::Ignore`), its meshes; shots push it like any
//! body; its contacts are reported (`CollisionEventsEnabled`, with its model's
//! surface as `core::PropSurface`) for games' impact sounds and dust. The
//! item components are requests for drawing too: `render::loose`
//! adds the meshes (none headless).

use std::collections::HashMap;

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{MapHeldModel, MapPart, PhysicsProp, PushAway};

/// A loose item drawn with a held model (its `MapHeldModel` key).
/// Spawners give it a `Transform` and velocities; the map adds the body
/// (`render::loose` the meshes) and removes it with the map.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct LooseItem(pub String);

/// Kilograms (a rifle is about 4; the game's own weight isn't known).
const MASS: f32 = 3.0;
/// Friction (multiplied with the world's 0.8: 0.4, as when it was 0.3
/// averaged with the old 0.5). Measured target (2026-10-15): an AK-47
/// thrown standing on de_dust2 stops 95 units from the hand in CS:S; ours
/// lands a little sooner and stops at about 72 with this (0.5 gave 66;
/// lower barely helps).
const FRICTION: f32 = 0.5;

#[derive(Resource, Default)]
pub struct LooseAssets(pub HashMap<String, LooseAsset>);

pub struct LooseAsset {
    /// From the held model's space (skeleton axes and units) to the item's
    /// frame, centred on its bounds (where its meshes are drawn).
    pub frame: Transform,
    pub half: Vec3,
    /// The model's surface property (impact sounds), lower-case.
    pub surface: Option<String>,
}

/// Each held model's loose form, sized by the first character's scale
/// (none without characters).
pub fn assets(data: &super::MapData) -> Option<LooseAssets> {
    let root = data.kit.characters.first()?.root;
    Some(LooseAssets(
        data.kit.held.iter().map(|h| (h.key.clone(), asset(h, root))).collect(),
    ))
}

/// A held model's loose form: `root` takes the skeleton's space to meters
/// (a character model's `root`; its translation is ignored).
pub fn asset(held: &MapHeldModel, root: Transform) -> LooseAsset {
    let to = Transform::from_rotation(root.rotation).with_scale(root.scale);
    let (lo, hi) = held
        .model
        .meshes
        .iter()
        .flat_map(|m| &m.positions)
        .map(|p| to.transform_point(Vec3::from(*p)))
        .fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), p| {
            (lo.min(p), hi.max(p))
        });
    let (centre, half) = if lo.x <= hi.x {
        ((lo + hi) / 2.0, ((hi - lo) / 2.0).max(Vec3::splat(0.01)))
    } else {
        (Vec3::ZERO, Vec3::splat(0.1))
    };
    LooseAsset {
        frame: to.with_translation(-centre),
        half,
        surface: held.model.surfaceprop.clone(),
    }
}

/// Give new loose items their body (`render::loose` their meshes).
pub fn attach_loose(
    items: Query<(Entity, &LooseItem), Without<RigidBody>>,
    assets: Option<Res<LooseAssets>>,
    mut commands: Commands,
) {
    let Some(assets) = assets else { return };
    for (e, item) in &items {
        let half = assets.0.get(&item.0).map_or(Vec3::splat(0.1), |a| a.half);
        let mut ent = commands.entity(e);
        // Its landings and knocks sound by its model's surface, and kick up
        // dust when hard (the game's impact sounds and effects listen).
        if let Some(s) = assets.0.get(&item.0).and_then(|a| a.surface.clone()) {
            ent.try_insert(crate::core::PropSurface(s));
        }
        // `try_`: an item can be gone by the time this applies (picked up,
        // or a network client's copy despawned, in the same frame; the
        // network soak saw it: a failed command each time).
        ent.try_insert((
            RigidBody::Dynamic,
            Collider::cuboid(half.x * 2.0, half.y * 2.0, half.z * 2.0),
            CollisionLayers::new(crate::core::ITEM_LAYER, LayerMask::ALL),
            Mass(MASS),
            Friction::new(FRICTION).with_combine_rule(CoefficientCombine::Multiply),
            Restitution::new(0.1),
            LinearDamping(0.1),
            AngularDamping(0.5),
            // Small and fast when shot: swept so it can't pass through
            // thin surfaces, against everything (avian sweeps a dynamic
            // body against static and kinematic ones by default). Only
            // while it moves: avian 0.8 sweeps a body once it travels more
            // than half its thinnest collider in a step (`threshold`); an
            // unthresholded sweep (avian 0.7's default) of every resting
            // item against everything its box touched cost 19 ms a tick
            // with 250 placed weapons on mg_swag_multigames_v1.
            SweptCcd::default().with_filter(CcdFilter::ALL),
            MaxLinearSpeed(2000.0 * 0.0254),
            MaxAngularSpeed(3600f32.to_radians()),
            PhysicsProp {
                push: PushAway::Ignore,
                mass: MASS,
                bounds: (-half, half),
            },
            (MapPart, Visibility::default(), CollisionEventsEnabled),
            // Hidden where the camera can't see it, wherever it goes
            // (`tag_moved_brush_entities`): placed weapons in other rooms
            // showed through the sky.
            (
                super::BrushEntityBounds { min: -half, max: half },
                super::VisClusters::new(Vec::new()),
            ),
        ));
    }
}

/// An item drawn with a held model's meshes (its `MapHeldModel` key) and
/// no physics body: something that moves itself (a thrown grenade).
/// Spawners give it a `Transform`; `render::loose` adds the meshes.
/// Reflected: a live game's projectiles in flight are a remote query
/// (`world.query` with it and `Transform`, docs/OBSERVABILITY.md 3).
#[derive(Component, Reflect, Clone, Debug, PartialEq)]
#[reflect(Component)]
pub struct ShownItem(pub String);

/// A `ShownItem` whose model rests on its transform's origin (its
/// bounds' bottom there) instead of being centred on it: something set
/// down on the ground (a planted bomb).
/// The rotation turns the model first (one whose own up isn't the
/// held-model frame's).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct SitOnOrigin(pub Quat);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{MapMesh, MapModel};

    #[test]
    fn loose_items_are_centred_and_scaled() {
        let mut model = MapModel::default();
        model.meshes.push(MapMesh {
            positions: vec![[0.0, 0.0, 0.0], [40.0, 4.0, 2.0]],
            ..default()
        });
        let held = MapHeldModel {
            merged: Vec::new(),
            own: None,
            key: "gun".into(),
            model,
            bone: "b".into(),
            muzzle: None,
            attachments: Vec::new(),
        };
        let a = asset(&held, Transform::from_scale(Vec3::splat(0.0254)));
        assert!((a.half - Vec3::new(0.508, 0.0508, 0.0254)).length() < 1e-5);
        // The model's far corner lands at +half.
        let corner = a.frame.transform_point(Vec3::new(40.0, 4.0, 2.0));
        assert!((corner - a.half).length() < 1e-5);
    }
}
