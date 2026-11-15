//! Free flight through walls. A debug tool, and the second Movement
//! implementation that keeps the slot honest from day one.
//!
//! The `noclip` console command (`chimera_net`'s `player_commands`)
//! remembers the movement it left (`NoclipBack`); a game's spawn ends
//! that noclip (`end_toggled`), as CS:S puts every player it spawns back
//! to walking (specs/cs_source/movement.md "Noclip and spawning").
//! Its marker (`core::Noclip`) keeps damage off the character, as CS:S's
//! noclip does (`core::DamageRules`, which `weapon::HitArmor` asks before armour
//! takes a hit; games may still react to the refused hit, `core::Refusal`).

use bevy::prelude::*;

use crate::{
    core::{Intent, MovementState, Predict, SimClock, Velocity},
    slots::RegisterSlots,
};

pub const ID: &str = "chimera:noclip";

const SPEED: f32 = 10.0;
const FAST_MULTIPLIER: f32 = 3.0;

/// The marker lives in `core` (`core::Noclip`), where damage refuses a
/// noclipping character.
pub use crate::core::Noclip;

/// The movement a character had when the `noclip` command turned noclip
/// on, which it goes back to when noclip ends.
#[derive(Component, Debug, Clone, Copy)]
pub struct NoclipBack(pub &'static str);

/// End a noclip the `noclip` command turned on: the character moves as it
/// did before (`NoclipBack`). A spawn calls it (CS:S: a player respawned
/// or put back at a round's start walks again). A character given noclip
/// as its movement outright (`--movement chimera:noclip`, a test's
/// `spawn_character`) keeps it. True when it ended one.
pub fn end_toggled(world: &mut World, e: Entity) -> bool {
    let current = world.get::<crate::slots::MovementSlot>(e).map(|s| s.0);
    let Some(back) = world.get::<NoclipBack>(e).map(|b| b.0) else {
        return false;
    };
    world.entity_mut(e).remove::<NoclipBack>();
    if current != Some(ID) || back == ID {
        return false;
    }
    crate::slots::set_movement(e, back).apply(world);
    true
}

pub struct NoclipPlugin;

impl Plugin for NoclipPlugin {
    fn build(&self, app: &mut App) {
        app.register_movement::<Noclip>(ID)
            .add_systems(Predict::Movement, step);
    }
}

#[allow(clippy::type_complexity)]
fn step(
    mut q: Query<(&Intent, &mut Transform, &mut Velocity, &mut MovementState), (With<Noclip>, Without<crate::core::Carried>)>,
    clock: Res<SimClock>,
) {
    for (intent, mut transform, mut vel, mut state) in &mut q {
        let up = intent.jump as i8 as f32 - intent.crouch as i8 as f32;
        let local = Vec3::new(intent.move_axis.x, 0.0, -intent.move_axis.y).clamp_length_max(1.0);
        let speed = if intent.sprint { SPEED * FAST_MULTIPLIER } else { SPEED };
        vel.0 = (intent.look_rotation() * local + Vec3::Y * up) * speed;
        transform.translation += vel.0 * clock.dt();
        // The box and the eye stay as they were (`setpos` places the feet).
        *state = MovementState {
            eye_offset: state.eye_offset,
            hull_min: state.hull_min,
            hull_max: state.hull_max,
            ..default()
        };
    }
}
