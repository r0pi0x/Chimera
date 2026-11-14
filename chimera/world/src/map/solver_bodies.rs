//! A despawned body's solver body goes with it: the fix on our side of an
//! avian 0.8 leak (`physics_plugins` adds it).
//!
//! Avian keeps a solver body (`SolverBodies`, an entry and a
//! `SolverBodyIndex` on the entity) for every dynamic or kinematic body
//! that is awake and enabled, and every substep's passes walk them all
//! (`integrate_positions` and the others: about 340 instructions an entry a
//! tick). It gives a body one when its `Sleeping` (or `RigidBodyDisabled`)
//! is removed, and a despawn removes every component: a body despawned
//! asleep was pushed an entry on its way out, whose index was to be
//! inserted by a command the despawned entity never got, so nothing took
//! the entry out again. Every sleeping prop of a map swapped out, every
//! ragdoll and gib despawned at rest left one for good: 1,422 of the 1,434
//! after three laps of five maps (`ultimate_gun_game`'s
//! `heavy::session_maps`), 183k instructions a tick more each lap
//! (docs/performance.md, "Solver bodies of despawned bodies").
//!
//! Here: a body despawned without a `SolverBodyIndex` queues a look for
//! its entity among the solver bodies once the despawn is done, and an
//! entry found is removed as Avian removes one (the last swapped into its
//! place, that body's index set).
//!
//! Known shortcuts:
//! - The look is a scan of `SolverBodies::entities` from its end (the
//!   entry, when there is one, was just pushed) for each body despawned
//!   asleep or disabled: the whole list when Avian made none (a few
//!   hundred entries at most, a map's awake bodies). Why it's OK for now:
//!   bodies are despawned a few a second in play and a map's at its swap.
//!   Gone when Avian stops making a solver body on a despawn.

use avian3d::{
    dynamics::solver::solver_body::{SolverBodies, SolverBodyIndex},
    prelude::*,
};
use bevy::{ecs::lifecycle::Despawn, prelude::*};

pub struct DespawnedSolverBodies;

impl Plugin for DespawnedSolverBodies {
    fn build(&self, app: &mut App) {
        app.add_observer(forget_on_despawn);
    }
}

/// A body going without a solver body of its own (asleep, disabled):
/// whatever Avian makes it on the way out is looked for afterwards.
fn forget_on_despawn(
    despawn: On<Despawn<RigidBody>>,
    bodies: Query<&RigidBody, Without<SolverBodyIndex>>,
    mut commands: Commands,
) {
    let entity = despawn.entity;
    if bodies.get(entity).is_ok_and(|b| !b.is_static()) {
        commands.queue(move |world: &mut World| forget(world, entity));
    }
}

/// Take `entity`'s solver body out, if it has one, as Avian's
/// `remove_solver_body` does.
fn forget(world: &mut World, entity: Entity) {
    let Some(mut solver) = world.get_resource_mut::<SolverBodies>() else { return };
    let Some(at) = solver.entities().iter().rposition(|e| *e == entity) else { return };
    let index = SolverBodyIndex(at as u32);
    if let Some(moved) = solver.swap_remove(index)
        && let Some(mut i) = world.get_mut::<SolverBodyIndex>(moved)
    {
        *i = index;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A headless app with Avian's plugins, a step a frame; with the fix or
    /// without.
    fn physics_app(fixed: bool) -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            AssetPlugin::default(),
            bevy::mesh::MeshPlugin,
            PhysicsPlugins::default(),
        ))
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f64(
            1.0 / 64.0,
        )));
        if fixed {
            app.add_plugins(DespawnedSolverBodies);
        }
        app.finish();
        app.cleanup();
        app.update();
        app
    }

    fn crate_at(app: &mut App, x: f32) -> Entity {
        app.world_mut()
            .spawn((RigidBody::Dynamic, Collider::cuboid(1.0, 1.0, 1.0), Transform::from_xyz(x, 0.5, 0.0)))
            .id()
    }

    fn solver_bodies(app: &App) -> Vec<Entity> {
        app.world().resource::<SolverBodies>().entities().to_vec()
    }

    /// Bodies despawned asleep, disabled and awake, among others that
    /// stay: the solver bodies left are the awake ones', each at its index.
    fn despawns(fixed: bool) -> (Vec<Entity>, Vec<Entity>) {
        let mut app = physics_app(fixed);
        app.world_mut().spawn((RigidBody::Static, Collider::cuboid(40.0, 1.0, 40.0), Transform::from_xyz(0.0, -0.5, 0.0)));
        let crates: Vec<Entity> = (0..8).map(|i| crate_at(&mut app, i as f32 * 3.0)).collect();
        for _ in 0..5 {
            app.update();
        }
        assert_eq!(solver_bodies(&app).len(), 8);
        let w = app.world_mut();
        w.entity_mut(crates[0]).insert(Sleeping);
        w.entity_mut(crates[1]).insert(Sleeping);
        w.entity_mut(crates[2]).insert(RigidBodyDisabled);
        w.flush();
        assert_eq!(solver_bodies(&app).len(), 5, "a body asleep or disabled has no solver body");
        // Asleep, asleep by a command among others, disabled, and awake.
        app.world_mut().despawn(crates[0]);
        let mut commands = app.world_mut().commands();
        commands.entity(crates[1]).despawn();
        commands.entity(crates[2]).despawn();
        commands.entity(crates[3]).despawn();
        app.world_mut().flush();
        for _ in 0..5 {
            app.update();
        }
        let left = solver_bodies(&app);
        for (at, e) in left.iter().enumerate() {
            if let Some(i) = app.world().get::<SolverBodyIndex>(*e) {
                assert_eq!(i.0 as usize, at, "a body's index is its solver body's place");
            }
        }
        (left, crates[4..].to_vec())
    }

    #[test]
    fn a_body_despawned_asleep_leaves_no_solver_body() {
        let (mut left, mut awake) = despawns(true);
        left.sort();
        awake.sort();
        assert_eq!(left, awake, "the solver bodies are the four awake crates'");
    }

    /// Avian alone keeps the despawned sleepers' (0.8.0): when this fails,
    /// Avian has fixed it and `DespawnedSolverBodies` can go.
    #[test]
    fn avian_alone_leaves_them() {
        let (left, awake) = despawns(false);
        assert!(left.len() > awake.len(), "Avian took the despawned sleepers' solver bodies out itself: {left:?}");
    }
}
