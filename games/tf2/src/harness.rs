//! TF2 on the engine's headless harness (`chimera_world::harness::Sim`),
//! for its scenario tests, the apps' and the tools': `Tf2Sim`, the base
//! they build on, as HL2:DM's `Hl2Sim` and CS:S's `CsSim` are theirs.
//! `Sim::new(Tf2Sim, GreyboxMapPlugin)`.

use bevy::prelude::*;

/// TF2's simulation (`crate::SimPlugins` with `game::Tf2Plugins`) as the
/// harness runs it, headless, TF2 the gameplay (its tick,
/// `game::TICK_INTERVAL`), map logic's random picks seeded so they repeat
/// run to run.
#[derive(Clone, Copy, Debug, Default)]
pub struct Tf2Sim;

impl Plugin for Tf2Sim {
    fn build(&self, app: &mut App) {
        use crate::gameplay::{Gameplay, GameplayAppExt};
        app.add_plugins((crate::SimPlugins, crate::game::Tf2Plugins))
            .set_gameplay(Gameplay::of(&crate::game::INFO))
            .insert_resource(source_logic::logic::LogicSeed(Some(0x9e37_79b9_7f4a_7c15)));
    }
}
