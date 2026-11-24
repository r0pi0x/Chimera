//! Counter-Strike 1.6 on the engine's headless harness (`chimera_world::harness::Sim`),
//! for its scenario tests: `Cs16Sim`, the base they build on.
//! `Sim::new(Cs16Sim, GreyboxMapPlugin)`.

use bevy::prelude::*;

/// The game's simulation (`crate::SimPlugins` with `game::Cs16Plugins`)
/// as the harness runs it, headless, this game the gameplay (its tick,
/// `game::TICK_INTERVAL`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Cs16Sim;

impl Plugin for Cs16Sim {
    fn build(&self, app: &mut App) {
        use crate::gameplay::{Gameplay, GameplayAppExt};
        app.add_plugins((crate::SimPlugins, crate::game::Cs16Plugins))
            .set_gameplay(Gameplay::of(&crate::game::INFO));
    }
}
