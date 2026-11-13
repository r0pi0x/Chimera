//! Combat Arms on the engine's headless harness
//! (`chimera_world::harness::Sim`), for its scenario tests and tools:
//! `CaSim`, the base they build on, as CS:S's `CsSim` is its tests'.
//! `Sim::new(CaSim, GreyboxMapPlugin)`. `ladder_models_in`: a match's
//! ladder's models loaded ahead (where the front end loads them).

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


/// Tick `sim` until the match's ladder's models are in: its bodies, held
/// and view models (`characters::preload::ladder_bodies_waiting` and
/// `ladder_keys_waiting` empty for `settle` ticks in a row: every one that
/// can arrive has), at most `cap`; returns those whose load failed (an
/// install without the file), each with its reason. Panics naming the
/// models still waiting at the cap: one that should arrive and didn't.
pub fn ladder_models_in(
    sim: &mut chimera_world::harness::Sim,
    settle: u32,
    cap: std::time::Duration,
) -> Vec<String> {
    use crate::game::characters::{
        bodies_on_demand::BodiesOnDemand,
        held_on_demand::HeldOnDemand,
        preload::{LadderModels, ladder_bodies_waiting, ladder_keys_waiting},
    };
    let started = std::time::Instant::now();
    let mut settled = 0;
    while settled < settle {
        let w = sim.app.world();
        let bodies = ladder_bodies_waiting(w);
        let keys = ladder_keys_waiting(w);
        settled = if bodies.is_empty() && keys.is_empty() { settled + 1 } else { 0 };
        assert!(
            started.elapsed() < cap,
            "the ladder's models never came in within {cap:?}: bodies {bodies:?} (records), {keys:?}; neither \
             loaded nor failed (`ladder_bodies_waiting`, `ladder_keys_waiting`)"
        );
        sim.ticks(1);
    }
    let w = sim.app.world();
    let ladder = w.resource::<LadderModels>();
    let mut failed = Vec::new();
    if let Some(b) = w.get_resource::<BodiesOnDemand>() {
        failed.extend(b.failed().iter().filter(|(id, _)| ladder.bodies.contains(id)).map(|(id, e)| format!("body {id}: {e}")));
    }
    if let Some(h) = w.get_resource::<HeldOnDemand>() {
        failed.extend(h.failed().iter().filter(|(k, _)| ladder.keys.contains(k)).map(|(k, e)| format!("held {k}: {e}")));
    }
    let context = w.get_resource::<crate::map::LoadContext>();
    let ca = context.map_or_else(crate::game::data::CaData::install, crate::game::data::CaData::install_for);
    for k in &ladder.keys {
        if let Some(Err(e)) = crate::game::view_model::on_demand::cached(&ca, k) {
            failed.push(format!("view {k}: {e}"));
        }
    }
    failed
}
