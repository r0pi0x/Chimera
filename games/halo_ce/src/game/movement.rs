//! Halo's stand-in movement (`halo_ce:movement`): Chimera's placeholder
//! walking (`chimera_sim::movement::placeholder`) under Halo's id, so a
//! player can stand and walk on the greybox (and on a Halo map) before
//! Halo's own movement exists. The match's movement (`slots::Loadout`).
//!
//! Known shortcuts: PLACEHOLDER numbers throughout: they are the
//! placeholder's own (metres and seconds), not Halo's; no spec number is
//! used. Halo's movement (specs/halo_ce/movement.md) replaces this file's
//! registration, and its state and numbers come with it.

use bevy::prelude::*;
use chimera_core::slots::Loadout;
use chimera_sim::movement::placeholder::{PlaceholderAppExt, Tuning};

/// The movement's id.
pub const ID: &str = "halo_ce:movement";

/// PLACEHOLDER numbers (metres, seconds): walk, sprint, crouch, gravity and
/// jump as the placeholder's own, not Halo's.
pub const TUNING: Tuning = chimera_sim::movement::placeholder::DEFAULT;

/// `halo_ce:movement`, and the match's movement.
pub struct HaloMovementPlugin;

impl Plugin for HaloMovementPlugin {
    fn build(&self, app: &mut App) {
        app.placeholder_movement(ID, TUNING).insert_resource(Loadout { movement: ID });
    }
}
