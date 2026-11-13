//! Physics clips: brushes solid to physics objects only (Source's
//! func_clip_vphysics). The football pitches' walls that keep the ball in
//! (mg_creative_multigames_v8_ns, mg_lego_multigames_v2), the start wall
//! that holds mg_boatrace_scramble's boats until the race, the course ends
//! of mg_lt_galaxy_v5.
//!
//! Each clip entity with volumes gets a static collider (`PhysicsClip`) on
//! `core::PHYSICS_CLIP_LAYER` (specs/hl2dm/maps_entities.md 11): what
//! moves by physics collides with it: props and physics brushes (held ones
//! too) on the default layer, and loose items on `core::ITEM_LAYER`
//! (dropped and placed weapons, items, frag grenades, S.L.A.M.s); a flyer
//! marked `Flyer::meets_clips` (HL2:DM's energy ball) meets it in its
//! sweep. Characters (their physics shadows too), ragdolls and spatial
//! queries (shots, sight, bolts and rockets) pass. The logic layer
//! (`logic::bridge`) switches it: off while the entity is disabled
//! (StartDisabled, Enable/Disable) or gone (Kill), and, when it names a
//! filter (`filtername`), it blocks only the bodies that filter passes
//! (`ClipPasses`, checked in `MapCollisionHooks` and by flyers).
//!
//! Known shortcuts (docs/tech-debt.md): ragdolls and gibs' debris pass it
//! (the spec's list doesn't name them; players' ragdolls are the
//! clients' in Source, which have no clips). Why it's OK for now: no
//! stock clip stands where bodies fall.

use std::collections::HashSet;

use avian3d::prelude::*;
use bevy::{ecs::system::SystemParam, prelude::*};

use super::{MapData, MapPart, entities};

/// The class whose brushes are solid to physics objects only.
pub const CLASS: &str = "func_clip_vphysics";

/// What a physics clip collides with: physics props and brushes (the
/// default layer) and loose items (`core::ITEM_LAYER`).
pub const CLIPPED: LayerMask = LayerMask(LayerMask::DEFAULT.0 | crate::core::ITEM_LAYER.0);

/// A physics clip's collider: the map entity it was placed by (index into
/// `MapData::entities`).
#[derive(Component, Debug, Clone, Copy)]
pub struct PhysicsClip(pub usize);

/// The bodies a filtered clip blocks (None: no filter, it blocks every
/// physics object). The logic bridge writes it.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct ClipPasses(pub Option<HashSet<Entity>>);

/// A collider for each physics clip with volumes, under `root`.
pub(super) fn spawn_clips(
    commands: &mut Commands,
    colliders: &mut super::spawn::Colliders,
    data: &MapData,
    root: Entity,
) {
    for (i, e) in data.entities.iter().enumerate() {
        if !e.classname().eq_ignore_ascii_case(CLASS) {
            continue;
        }
        let Some(collider) = entities::brush_collider(e, data.entity_scale) else {
            continue;
        };
        let home = Transform::from_translation(entities::entity_to_engine(e.origin(), data.entity_scale))
            .with_rotation(entities::rotation_to_engine(entities::entity_rotation(e.angles())));
        let mut clip = commands.spawn((
            Name::new(format!("Physics clip {i}")),
            MapPart,
            PhysicsClip(i),
            ClipPasses::default(),
            RigidBody::Static,
            CollisionLayers::new(crate::core::PHYSICS_CLIP_LAYER, CLIPPED),
            home,
            ChildOf(root),
        ));
        colliders.put(&mut clip, collider);
        if e.get("filtername").is_some_and(|f| !f.trim().is_empty()) {
            clip.insert(ActiveCollisionHooks::FILTER_PAIRS);
        }
    }
}

/// For `MapCollisionHooks`: whether a pair with a filtered clip collides.
#[derive(SystemParam)]
pub struct ClipContacts<'w, 's> {
    clips: Query<'w, 's, &'static ClipPasses>,
    of: Query<'w, 's, &'static ColliderOf>,
}

impl ClipContacts<'_, '_> {
    /// Some(collides) when `a` or `b` is a clip with a filter; None
    /// otherwise (left to the other rules).
    pub fn filter(&self, a: Entity, b: Entity) -> Option<bool> {
        let check = |clip: Entity, other: Entity| {
            let passes = self.clips.get(clip).ok()?.0.as_ref()?;
            let body = self.of.get(other).map_or(other, |o| o.body);
            Some(passes.contains(&body) || passes.contains(&other))
        };
        check(a, b).or_else(|| check(b, a))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            AssetPlugin::default(),
            bevy::mesh::MeshPlugin,
            PhysicsPlugins::default().with_collision_hooks::<super::super::MapCollisionHooks>(),
        ))
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / 64.0,
        )));
        app.finish();
        app.cleanup();
        app.update();
        app
    }

    /// A floor clip: a ball falls onto it and stays; a ball the filter
    /// doesn't pass falls through; a ray (a shot) passes it.
    #[test]
    fn clips_hold_physics_objects_the_filter_passes() {
        let mut app = app();
        let blocked = app
            .world_mut()
            .spawn((RigidBody::Dynamic, Collider::sphere(0.2), Transform::from_xyz(0.0, 1.0, 0.0)))
            .id();
        let passed = app
            .world_mut()
            .spawn((RigidBody::Dynamic, Collider::sphere(0.2), Transform::from_xyz(3.0, 1.0, 0.0)))
            .id();
        app.world_mut().spawn((
            PhysicsClip(0),
            ClipPasses(Some([blocked].into_iter().collect())),
            ActiveCollisionHooks::FILTER_PAIRS,
            RigidBody::Static,
            Collider::cuboid(10.0, 0.2, 10.0),
            CollisionLayers::new(crate::core::PHYSICS_CLIP_LAYER, CLIPPED),
            Transform::default(),
        ));
        for _ in 0..90 {
            app.update();
        }
        let y = |app: &App, e: Entity| app.world().get::<Transform>(e).unwrap().translation.y;
        assert!(y(&app, blocked) > 0.0, "the filtered ball rests on the clip: {}", y(&app, blocked));
        assert!(y(&app, passed) < -1.0, "the other falls through: {}", y(&app, passed));
        let mut q = bevy::ecs::system::SystemState::<SpatialQuery>::new(app.world_mut());
        let spatial = q.get(app.world()).unwrap();
        let filter = SpatialQueryFilter::default().with_mask(crate::core::SOLID_LAYERS);
        let hit = spatial.cast_ray(Vec3::new(-3.0, 1.0, 0.0), Dir3::NEG_Y, 3.0, true, &filter);
        assert!(hit.is_none(), "shots pass clips: {hit:?}");
    }
}
