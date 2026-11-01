//! HL2:DM's movement for now (`hl2dm:movement`): Chimera's placeholder
//! walking (`chimera_sim::movement::placeholder`) with HL2:DM's plain
//! numbers from specs/hl2dm/movement.md (H1) and source_deltas.md 1.1:
//! run 190 units/s, sprint (`+speed`) 320, gravity 600 units/s², a jump
//! of 160 units/s, ducked speed a third of the run. The match's movement
//! (`slots::Loadout`).
//!
//! Known shortcuts: not Source movement: no friction, acceleration or
//! air-strafe rules, no walk (150), aux power, ladders, water, step-up
//! or fall damage, and the placeholder's capsule (not the 72-unit hull).
//! Source movement under HL2:DM's config is M3 (lift S6).

use bevy::prelude::*;
use chimera_sim::movement::placeholder::{PlaceholderAppExt, Tuning};

use crate::slots::Loadout;

/// The movement's id.
pub const ID: &str = "hl2dm:movement";

const METERS_PER_UNIT: f32 = 0.0254;

/// H1's numbers in meters: run 190, sprint 320, ducked 190/3 (the shared
/// duck scale), gravity 600, jump 160 units/s.
pub const TUNING: Tuning = Tuning {
    walk: 190.0 * METERS_PER_UNIT,
    sprint: 320.0 * METERS_PER_UNIT,
    crouch: 190.0 / 3.0 * METERS_PER_UNIT,
    gravity: 600.0 * METERS_PER_UNIT,
    jump: 160.0 * METERS_PER_UNIT,
};

/// `hl2dm:movement`, and the match's movement.
pub struct Hl2MovementPlugin;

impl Plugin for Hl2MovementPlugin {
    fn build(&self, app: &mut App) {
        app.placeholder_movement(ID, TUNING).insert_resource(Loadout { movement: ID });
    }
}
