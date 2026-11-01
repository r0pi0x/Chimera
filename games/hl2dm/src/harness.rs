//! HL2:DM on the engine's headless harness (`chimera_world::harness::Sim`),
//! for its scenario tests, the apps' and the tools': `Hl2Sim`, the base
//! they build on, as CS:S's `CsSim` and Combat Arms' `CaSim` are theirs.
//! `Sim::new(Hl2Sim, GreyboxMapPlugin)`.

use bevy::prelude::*;

/// HL2:DM's simulation (`crate::SimPlugins` with `game::Hl2dmPlugins`) as
/// the harness runs it, headless, HL2:DM the gameplay (its tick,
/// `game::TICK_INTERVAL`), map logic's random picks seeded so they repeat
/// run to run.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hl2Sim;

impl Plugin for Hl2Sim {
    fn build(&self, app: &mut App) {
        use chimera_core::gameplay::{Gameplay, GameplayAppExt};
        app.add_plugins((crate::SimPlugins, crate::game::Hl2dmPlugins))
            .set_gameplay(Gameplay::of(&crate::game::INFO))
            .insert_resource(source_logic::logic::LogicSeed(Some(0x9e37_79b9_7f4a_7c15)));
    }
}
