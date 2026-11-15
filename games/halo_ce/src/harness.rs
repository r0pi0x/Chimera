//! Halo on the engine's headless harness (`chimera_world::harness::Sim`),
//! for its scenario tests, the apps' and the tools': `HaloSim`, the base
//! they build on, as HL2:DM's `Hl2Sim` and Combat Arms' `CaSim` are theirs.
//! `Sim::new(HaloSim, GreyboxMapPlugin)`.

use bevy::prelude::*;

/// Halo's simulation (`crate::SimPlugins` with `game::HaloPlugins`) as the
/// harness runs it, headless, Halo the gameplay (its tick,
/// `game::TICK_INTERVAL`).
#[derive(Clone, Copy, Debug, Default)]
pub struct HaloSim;

impl Plugin for HaloSim {
    fn build(&self, app: &mut App) {
        use crate::gameplay::{Gameplay, GameplayAppExt};
        app.add_plugins((crate::SimPlugins, crate::game::HaloPlugins))
            .set_gameplay(Gameplay::of(&crate::game::INFO));
    }
}
