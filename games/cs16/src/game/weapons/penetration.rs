//! How Counter-Strike 1.6's bullets pass surfaces (specs/cs16/damage.md,
//! "Wall penetration"): data/penetration.toml as Chimera's power rule
//! (`weapon::PowerRules`, the app's `Cs16Penetration`), which the
//! `cs16:gun` hook puts on every gun with its ammo's power and distance
//! (`gun::insert`).
//!
//! Known shortcuts:
//! - A start inside a solid passes out of it to its far side (the shared
//!   rule's `solid_exit`); how far the original's trace goes inside thick
//!   walls is damage.md's open question 1.

use std::sync::Arc;

use bevy::prelude::*;
use chimera_sim::weapon::PowerRules;

/// The document (data/penetration.toml).
pub const DATA: (&str, chimera_core::game_data::GameData) = ("penetration.toml", chimera_core::game_data!("data/penetration.toml"));

/// The game's rules, parsed.
pub fn rules() -> Arc<PowerRules> {
    Arc::new(PowerRules::parse(DATA.0, DATA.1.text()).unwrap_or_else(|e| panic!("CS 1.6 penetration data: {e}")))
}

/// The rules the guns' hook puts on each gun (`Cs16GunsPlugin` inserts
/// them, parsed once per app).
#[derive(Resource, Clone)]
pub struct Cs16Penetration(pub Arc<PowerRules>);
