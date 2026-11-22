//! A map's switchable lights cost nothing while nobody switches them
//! (feedback b37d546e, 2026-11-21: combat_arms:roadkill "very laggy", every
//! other frame 25-30 ms on the tester's PC, `relight` 18-20 ms of it). In
//! the gun game every game's logic runs, so on a Combat Arms map two layers
//! wrote `map::LightStyles`: Source's logic (no styles: `[]`) every tick,
//! and Combat Arms' light groups (`[(32, false)]`, roadkill's group 32 off
//! in play, on at map start) whenever its objects changed, each undoing the
//! other; every write rebuilt the whole lightmap atlas on the CPU. Now each
//! writer keeps its own list in the resource (`LightStyles::with`).
//!
//! Headless, with the drawing's map (`chimera_present::MapPlugin`, the
//! lightmap images and `StyledLightmaps`), the automatic gun game with four
//! bots, repeatable (`Sim::repeatable`), work counts (`Sim::work`): the
//! relighting's instructions over the counted ticks under a limit.
//! `CHIMERA_MAP_LIGHTS=<game:map>` counts another map instead and prints
//! it, checking nothing (the reference: `combat_arms:warehouse`). Passes
//! with a note without the Combat Arms install or hardware counters.

use bevy::{mesh::skinning::SkinnedMeshInverseBindposes, prelude::*};
use chimera_core::{metrics::work::WorkCounts, mount::config::LocalConfig};
use chimera_present::render::prop_material::PropMaterial;
use chimera_sim::minigame;
use chimera_world::harness::Sim;
use combat_arms::{game::TICK_INTERVAL, harness::CaSim};
use party_roster::PartyPartsPlugin;
use ultimate_gun_game::{MINIGAME, UltimateGunGamePlugin};

const MAP: &str = "combat_arms:roadkill";
/// Ticks before the count: the match started, the objects as in play.
const WARM_UP: u64 = 600;
/// Ticks counted (about 10 s at Combat Arms' tick).
const TICKS: usize = 660;
/// The lightmaps' relighting over the counted ticks (two frames each),
/// every system named `relight` (the light styles' and the props' light
/// probes). Measured 2026-11-21: 16,237 M with the two writers undoing each
/// other (~25 M a whole-atlas rebuild, on most frames; the median tick
/// 32.7 M), 9.2 M after (the median tick 8.2 M).
const RELIGHT: u64 = 200_000_000;

fn combat_arms_installed() -> bool {
    let Ok(config) = LocalConfig::load() else { return false };
    config
        .game_path(combat_arms::game::GAME)
        .is_some_and(|p| chimera_core::mount::served::usable("combat_arms", &p, "Game"))
}

#[test]
fn roadkills_lights_are_not_relit_in_play() {
    let other = std::env::var("CHIMERA_MAP_LIGHTS").ok();
    let check = other.is_none();
    let name = other.as_deref().unwrap_or(MAP);
    let Some(map) = party_roster::load_map(name, 0).ok().filter(|_| combat_arms_installed()) else {
        eprintln!("skipping: needs the Combat Arms install");
        return;
    };
    // The light groups the map can switch (roadkill's mission for the
    // gun game, its Team Deathmatch one, switches its group off).
    let groups: Vec<String> = map
        .entities
        .iter()
        .filter(|e| {
            e.classname() == "LightGroup" && e.get(combat_arms::game::objects::runtime::keys::LIGHT_STYLE).is_some()
        })
        .filter_map(|e| e.get("targetname").map(str::to_string))
        .collect();
    assert!(
        !check || !groups.is_empty(),
        "{MAP} has no switchable light group: no longer the map this guards"
    );
    let mut sim = Sim::new(
        CaSim,
        (
            |app: &mut App| {
                app.init_asset::<Image>()
                    .init_asset::<StandardMaterial>()
                    .init_asset::<PropMaterial>()
                    .init_asset::<SkinnedMeshInverseBindposes>();
            },
            chimera_present::MapPlugin::new(map),
            PartyPartsPlugin,
            UltimateGunGamePlugin {
                seed: 36336,
                rungs: 13,
                ..Default::default()
            },
        ),
    );
    sim.set_tick_interval(TICK_INTERVAL);
    // Frames at twice the tick rate, as a client draws them (the tester's
    // 80-200 fps against 66.7 ticks): a frame without a tick sees what the
    // frame before it left.
    sim.app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_secs_f64(TICK_INTERVAL as f64 / 2.0),
    ));
    sim.repeatable();
    {
        let mut s = sim
            .app
            .world_mut()
            .resource_mut::<combat_arms::game::bots::BotSettings>();
        s.quota = 4;
        s.quota_set = true;
    }
    minigame::start(sim.app.world_mut(), MINIGAME).expect("the automatic gun game starts");
    sim.ticks(WARM_UP);
    // Off, as in play.
    for g in &groups {
        sim.app
            .world_mut()
            .write_message(combat_arms::game::objects::CaObjectMessage {
                target: g.clone(),
                message: "off".into(),
            });
    }
    sim.ticks(60);
    assert!(
        !check
            || sim
                .app
                .world()
                .contains_resource::<chimera_present::render::light_styles::StyledLightmaps>(),
        "the map's lightmaps were not built with their styles"
    );
    if check {
        let styles = sim
            .app
            .world()
            .get_resource::<chimera_world::map::LightStyles>()
            .map(|s| s.styles.clone());
        assert!(
            styles
                .as_ref()
                .is_some_and(|s| s.len() == groups.len() && s.iter().all(|s| !s.1)),
            "the light groups switched off: {styles:?}"
        );
    }
    let Some(w) = sim.work(TICKS as u64) else {
        eprintln!("skipping: no hardware counters");
        return;
    };
    let relit: u64 = w
        .systems
        .0
        .iter()
        .filter(|(k, _)| k.ends_with("relight"))
        .map(|(_, v)| v.0)
        .sum();
    let top = |w: &WorkCounts| {
        w.top(15)
            .into_iter()
            .map(|(l, i, n)| format!("{:>10.3} M  {l} ({n} runs)", i as f64 / 1e6))
            .collect::<Vec<_>>()
            .join("\n")
    };
    eprintln!(
        "{name}: {TICKS} ticks, median {:.2} M; relighting {:.1} M in all\nsystems over the ticks:\n{}",
        w.median() as f64 / 1e6,
        relit as f64 / 1e6,
        top(&w.systems)
    );
    if !check {
        return;
    }
    assert!(
        relit < RELIGHT,
        "relighting took {relit} instructions over {TICKS} ticks (limit {RELIGHT}): a light style rewritten while nobody \
         switches it? The systems:\n{}",
        top(&w.systems)
    );
}
