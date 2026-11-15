//! TF2's maps in the party (docs/plans/active/tf2.md, M1): `tf2:ctf_2fort`
//! loads through the party's map sources (`party_kit::MAP_GAMES`) as the
//! dedicated server loads it, with the party's parts and the gun game
//! started; its spawn points are RED's and BLU's `info_player_teamspawn`s,
//! none of them a spot a standing player is stuck in; and a Combat Arms
//! player dropped at one lands on the floor and walks. Passes with a note
//! without the TF2 and Combat Arms installs.

use bevy::prelude::*;
use chimera_core::mount::config::LocalConfig;
use chimera_world::{
    harness::Sim,
    map::{KitLoad, LoadContext, MapLoadOptions, MapSimPlugin},
};
use combat_arms::harness::CaSim;
use party_kit::{DEFAULT_MINIGAME, PartyPartsPlugin, spot_checks};

fn installed() -> bool {
    let ca = LocalConfig::load()
        .ok()
        .and_then(|c| c.game_path(combat_arms::game::GAME))
        .is_some_and(|p| chimera_core::mount::served::usable("combat_arms", p, "Game"));
    let tf2 = tf2::game::INFO.install().is_some_and(|p| chimera_core::mount::served::usable("tf2", p, "tf"));
    ca && tf2
}

#[test]
fn a_player_lands_and_walks_on_ctf_2fort() {
    if !installed() {
        eprintln!("skipping: needs the TF2 and Combat Arms installs");
        return;
    }
    let context = LoadContext::default();
    let options = MapLoadOptions { kit: KitLoad::Only(&[]), ..MapLoadOptions::lean().in_context(&context) };
    let data = party_kit::load_map_with("tf2:ctf_2fort", &options).expect("tf2:ctf_2fort loads");
    assert_eq!(data.name, "tf2:ctf_2fort");
    assert_eq!(data.spawns.len(), 30, "RED's and BLU's 15 each");
    let mut sim = Sim::new(CaSim, (MapSimPlugin::new(data), PartyPartsPlugin));
    chimera_sim::minigame::start(sim.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
    sim.ticks(3);
    let w = sim.app.world_mut();
    let feet = spot_checks::spawn_feet(w);
    assert_eq!(feet.len(), 30, "every spawn point placed");
    let bots = spot_checks::bots_mesh(w, &feet);
    let r = spot_checks::report(w, &feet, &bots);
    eprintln!("tf2:ctf_2fort: {} spots, stuck {}", r.spots.len(), r.stuck.len());
    assert!(r.stuck.is_empty(), "spots a standing player is stuck in: {:?}", r.stuck);
    // A Combat Arms player at the first spawn: lands, then walks.
    let at = feet[0];
    let p = sim.spawn_character(at + Vec3::Y, combat_arms::game::movement::ID);
    sim.seconds(1.5);
    let low = sim.position(p).y + sim.state(p).hull_min.y;
    eprintln!("spawn {at}: feet at {low:.3}");
    assert!(low > at.y - 1.0 && low < at.y + 0.3, "spawn {at}: feet at {low}, not on the floor");
    let start = sim.position(p);
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(0.5);
    sim.intent(p).move_axis = Vec2::ZERO;
    sim.seconds(0.5);
    let moved = sim.position(p).xz().distance(start.xz());
    eprintln!("walked {moved:.2} m");
    assert!(moved > 0.5, "walked only {moved} m");
    assert!(sim.position(p).y > start.y - 1.0, "fell walking");
}
