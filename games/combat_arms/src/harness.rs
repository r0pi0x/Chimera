//! Combat Arms on the engine's headless harness
//! (`chimera_world::harness::Sim`), for its scenario tests and tools:
//! `CaSim`, the base they build on, as CS:S's `CsSim` is its tests'.
//! `Sim::new(CaSim, GreyboxMapPlugin)`. `ladder_bodies_in`: a match's
//! ladder's bodies loaded ahead (where the front end loads them).

use bevy::prelude::*;

use crate::game::rules::CaRuleSettings;

/// Combat Arms' whole simulation (`crate::SimPlugins` with
/// `game::CombatArmsPlugins`: every part, placeholders included) as the
/// harness runs it, headless, as the gameplay (its tick, `game::TICK_INTERVAL`),
/// on the built-in data only (`data::CaData::builtin`: never the
/// install's attribute data; an app's own `CaData` is its resource), with
/// no game mode (`ca_mode none`: the rules leave the test's characters
/// alone; a mode's tests turn one on, `with_mode`).
#[derive(Clone, Copy, Debug, Default)]
pub struct CaSim;

impl Plugin for CaSim {
    fn build(&self, app: &mut App) {
        CaSimWith(crate::game::data::CaData::builtin()).build(app);
    }
}

/// `CaSim` on `data` instead of the built-in data (tests of an app's own
/// data: weapon tuning, an install merged by hand). Each app has its own,
/// so two `Sim`s in one process can differ.
#[derive(Clone)]
pub struct CaSimWith(pub crate::game::data::CaData);

impl Plugin for CaSimWith {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.0.clone());
        use crate::gameplay::{Gameplay, GameplayAppExt};
        app.add_plugins((crate::SimPlugins, crate::game::CombatArmsPlugins))
            .set_gameplay(Gameplay::of(&crate::game::INFO));
        app.world_mut().resource_mut::<CaRuleSettings>().mode = "none".into();
    }
}

/// Play `mode` (a room key: `TDM`, `TSV`) from the next server frame on,
/// the match from its start (`ca_mode` and `ca_restart`).
pub fn with_mode(world: &mut World, mode: &str) {
    world.resource_mut::<CaRuleSettings>().mode = mode.into();
    crate::game::rules::flow::restart(world);
}


/// Tick `sim` until the match's ladder's bodies are in (`characters::
/// preload::ladder_bodies_waiting` empty for `settle` ticks in a row: every
/// one that can arrive has), at most `cap`; returns the bodies whose load
/// failed (an install without the body's file, each with its reason).
/// Panics naming the bodies still waiting at the cap: a body that should
/// arrive and didn't.
pub fn ladder_bodies_in(
    sim: &mut chimera_world::harness::Sim,
    settle: u32,
    cap: std::time::Duration,
) -> Vec<(i32, String)> {
    use crate::game::characters::{bodies_on_demand::BodiesOnDemand, preload::ladder_bodies_waiting};
    let started = std::time::Instant::now();
    let mut settled = 0;
    while settled < settle {
        let waiting = ladder_bodies_waiting(sim.app.world());
        settled = if waiting.is_empty() { settled + 1 } else { 0 };
        assert!(
            started.elapsed() < cap,
            "the ladder's bodies {waiting:?} (body records) never came in within {cap:?}: neither a loaded group \
             has them nor did their load fail (`ladder_bodies_waiting`)"
        );
        sim.ticks(1);
    }
    let failed = sim.app.world().get_resource::<BodiesOnDemand>().map(|b| b.failed().clone()).unwrap_or_default();
    let ladder = sim.app.world().resource::<crate::game::characters::preload::LadderModels>();
    failed.into_iter().filter(|(id, _)| ladder.bodies.contains(id)).collect()
}
