//! DoD:S on the engine's headless harness (`chimera_world::harness::Sim`),
//! for its scenario tests: `DodsSim`, the base they build on, as TF2's
//! `Tf2Sim` is its. `Sim::new(DodsSim, GreyboxMapPlugin)`.

use bevy::prelude::*;

/// DoD:S's simulation (`crate::SimPlugins` with `game::DodsPlugins`) as
/// the harness runs it, headless, DoD:S the gameplay (its tick,
/// `game::TICK_INTERVAL`), the weapons' committed numbers, map logic's random picks seeded so they repeat.
#[derive(Clone, Copy, Debug, Default)]
pub struct DodsSim;

impl Plugin for DodsSim {
    fn build(&self, app: &mut App) {
        use crate::gameplay::{Gameplay, GameplayAppExt};
        // The committed weapon numbers (no install read in a test).
        app.insert_resource(crate::game::weapons::script_numbers::GunNumbers::builtin());
        app.add_plugins((crate::SimPlugins, crate::game::DodsPlugins))
            .set_gameplay(Gameplay::of(&crate::game::INFO))
            .insert_resource(source_logic::logic::LogicSeed(Some(0x9e37_79b9_7f4a_7c15)));
    }
}
