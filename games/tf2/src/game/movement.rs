//! TF2's stand-in movement (`tf2:movement`): Chimera's placeholder walking
//! (`chimera_sim::movement::placeholder`) under TF2's id with the
//! Soldier's plain numbers from specs/tf2/movement.md: run 240 units/s
//! (no sprint in TF2: the same 240), ducked a third of it (80), gravity
//! 800 units/s², a jump of 289 units/s. The match's movement
//! (`slots::Loadout`).
//!
//! Known shortcuts: not Source movement and no classes: no friction,
//! acceleration or air-strafe rules, no backward scale (0.9), air duck,
//! the Scout's double jump, water, step-up or fall damage, the
//! placeholder's capsule (not the 48 x 48 x 82 box). TF2's movement on
//! Source's (`source_engine::source_movement`) with each class's speed is
//! M3.

use bevy::prelude::*;
use chimera_core::slots::Loadout;
use chimera_sim::movement::placeholder::{PlaceholderAppExt, Tuning};

/// The movement's id.
pub const ID: &str = "tf2:movement";

const METERS_PER_UNIT: f32 = super::METERS_PER_UNIT;

/// The Soldier's numbers in metres (movement.md, "Class speeds" and the
/// constants): run 240, no sprint (240), ducked 240/3, gravity 800, jump
/// 289 units/s.
pub const TUNING: Tuning = Tuning {
    walk: 240.0 * METERS_PER_UNIT,
    sprint: 240.0 * METERS_PER_UNIT,
    crouch: 240.0 / 3.0 * METERS_PER_UNIT,
    gravity: 800.0 * METERS_PER_UNIT,
    jump: 289.0 * METERS_PER_UNIT,
};

/// `tf2:movement`, and the match's movement.
pub struct Tf2MovementPlugin;

impl Plugin for Tf2MovementPlugin {
    fn build(&self, app: &mut App) {
        app.placeholder_movement(ID, TUNING).insert_resource(Loadout { movement: ID });
    }
}

/// `tf2:movement` on a composed game's simulation (a party's), not the
/// match's: the party's base game chooses that.
pub fn add_movement(app: &mut App) {
    let registered = app
        .world()
        .get_resource::<chimera_sim::movement::placeholder::Tunings>()
        .is_some_and(|t| t.0.iter().any(|(id, _)| *id == ID));
    if !registered {
        app.placeholder_movement(ID, TUNING);
    }
}
