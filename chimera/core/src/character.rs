//! The components every character has, whatever controls it.
//!
//! `character_bundle`, `spawn_character`, capsule size.
//! `CharacterPartsExt::on_every_character`: a part a game puts on its
//! characters once and keeps, there from the spawn instead (each part a
//! character gains later doubles the archetypes it visits before; Bevy
//! frees none: docs/DESIGN.md, "ECS style").

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::{
    core::{BaseVelocity, EntityGravity, Health, Intent, MovementState, Team, Velocity},
    slots::set_movement,
};

/// Standing collision capsule, shared by movement implementations until a
/// game's spec says otherwise. 1.8 m tall, 0.8 m wide, origin at its center.
pub const CAPSULE_RADIUS: f32 = 0.4;
pub const CAPSULE_HEIGHT: f32 = 1.8;

pub fn character_bundle(transform: Transform, team: Team) -> impl Bundle {
    (
        Name::new("Character"),
        transform,
        Intent::default(),
        Velocity::default(),
        MovementState::default(),
        Health::default(),
        team,
        (BaseVelocity::default(), EntityGravity::default()),
        // A camera or model may be attached as a child; children need a
        // visible parent.
        Visibility::default(),
        RigidBody::Kinematic,
        Collider::capsule(CAPSULE_RADIUS, CAPSULE_HEIGHT - 2.0 * CAPSULE_RADIUS),
        // Ragdolls and loose items pass through characters.
        CollisionLayers::new(LayerMask::DEFAULT, crate::core::CHARACTER_FILTER),
        // Movement implementations move the character; avian must not.
        CustomPositionIntegration,
        // The app's collision hooks drop its contacts with other bodies
        // that aren't dynamic (`map::contact_filter`): nothing uses them.
        ActiveCollisionHooks::FILTER_PAIRS,
        // Drawn between ticks by the client (`map::interp`).
        // A character never sleeps: each sleep and wake was a new
        // archetype for every combination of its other parts (Bevy frees
        // none, and queries walk them all: DESIGN.md, "ECS style").
        (SleepingDisabled, crate::core::Rebased::default()),
    )
}

/// Parts on every character from its spawn (required by `Intent`).
pub trait CharacterPartsExt {
    /// `C` (its default) on every character from its spawn, for a part
    /// put on once and kept, or one whose absence means its default:
    /// gained later, each is a new archetype for every combination of the
    /// character's other parts. Registered in a plugin's `build`, before
    /// any character spawns; registering it twice is harmless.
    fn on_every_character<C: Component + Default>(&mut self) -> &mut Self;
}

impl CharacterPartsExt for App {
    fn on_every_character<C: Component + Default>(&mut self) -> &mut Self {
        use bevy::ecs::component::RequiredComponentsError;
        match self.try_register_required_components::<Intent, C>() {
            Ok(()) | Err(RequiredComponentsError::DuplicateRegistration(..)) => {}
            Err(e) if cfg!(debug_assertions) => {
                panic!("on_every_character::<{}>: {e}", std::any::type_name::<C>())
            }
            Err(e) => warn!("on_every_character::<{}>: {e}", std::any::type_name::<C>()),
        }
        self
    }
}

/// Spawn a character using Movement implementation `movement`.
pub fn spawn_character(commands: &mut Commands, transform: Transform, team: Team, movement: &'static str) -> Entity {
    let entity = commands.spawn(character_bundle(transform, team)).id();
    commands.queue(set_movement(entity, movement));
    entity
}
