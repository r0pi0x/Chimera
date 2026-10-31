//! The mashup on a real map (the user's installs): de_dust2 loaded by
//! CS:S's loader and dressed for Combat Arms (`mashup_demo::MAP_SOURCES`)
//! carries Combat Arms' view models and none of CS:S's parts, its
//! terrorist and counter-terrorist spawns are Alpha's and Bravo's; on it,
//! headless with Combat Arms' simulation, a Combat Arms soldier stands on
//! the floor at Combat Arms' size (the two games' units each applied by
//! its own loader), walks, fires the M4A1 into a wall, and Combat Arms'
//! bots, navigating areas generated from dust2's collision, fight a Team
//! Death Match: they kill each other and respawn. Passes with a note
//! without both installs in chimera.local.toml.

use std::collections::HashSet;

use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::{ecs::system::SystemState, prelude::*};
use chimera_core::{
    core::{Dead, SpawnPoint, Team},
    mount::config::LocalConfig,
};
use chimera_sim::weapon::{Inventory, Magazine, Weapon};
use chimera_world::{
    harness::Sim,
    map::{MapData, MapSimPlugin, nav::NavMesh},
};
use combat_arms::{
    game::{
        bots::{self, CaBot},
        movement,
        rules::{
            CaMatch,
            state::{ALPHA, BRAVO},
        },
        weapons,
    },
    harness::{CaSim, with_mode},
};

/// de_dust2 as the mashup loads it; None (with a note) without both
/// installs.
fn dust2() -> Option<MapData> {
    let config = LocalConfig::load().ok()?;
    let css = config.game_path_in(cs_source::game::GAME, cs_source::game::INFO.steam.as_ref()).filter(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    let ca = config.game_path(combat_arms::game::GAME).filter(|p| chimera_core::mount::served::usable("combat_arms", p, "Game"));
    if css.is_none() || ca.is_none() {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
        return None;
    }
    Some(mashup_demo::load_map("cs_source:de_dust2", 0).unwrap())
}

#[test]
fn dust2_is_dressed_for_combat_arms() {
    let Some(map) = dust2() else { return };
    assert!(map.kit.radio.is_none() && map.kit.hud.is_none() && map.kit.nav.is_none(), "CS:S's parts taken off");
    let keys: Vec<&str> = map.kit.view_models.iter().map(|v| v.key.as_str()).collect();
    assert!(
        keys.iter().any(|k| k.contains("m4a1")) || map.warnings.iter().any(|w| w.contains("view model")),
        "Combat Arms' view models (or why not): {keys:?}"
    );
    assert!(keys.iter().all(|k| !k.starts_with("models/weapons/v_")), "no CS:S view model: {keys:?}");
    let teams: HashSet<Option<u8>> = map.spawns.iter().map(|(_, t)| t.map(|t| t.0)).collect();
    assert_eq!(teams, HashSet::from([Some(ALPHA), Some(BRAVO)]), "T and CT spawns as Alpha and Bravo");
}

/// This process's resident memory in MB (Linux; 0 elsewhere).
fn rss_mb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("VmRSS:"))
                .and_then(|l| l.split_whitespace().next()?.parse::<u64>().ok())
        })
        .map_or(0, |kb| kb / 1024)
}

/// A mashup's CS:S map is loaded as its world only (`MapLoadOptions::kit`):
/// CS:S's character bodies, HUD, radio, bot names, sounds, muzzle flash,
/// shells and navigation are never read from the install (its impact
/// decals are: the map's impact looks'), and the guns the app keeps are
/// the ones a whole load has. Run alone with
/// `--nocapture` it prints the time and the process's resident memory.
#[test]
fn a_mashup_map_loads_only_its_world() {
    use chimera_world::map::{KitLoad, MapLoadOptions};
    let started = std::time::Instant::now();
    let Some(map) = dust2() else { return };
    eprintln!(
        "mashup de_dust2: {:.1} s, {} textures, resident {} MB",
        started.elapsed().as_secs_f32(),
        map.textures.len(),
        rss_mb()
    );
    // The world as CS:S's loader makes it with the kit asked for no part.
    let kept = mashup_demo::KEPT;
    let world = (cs_source::game::MAP_LOADER.load)("de_dust2", &MapLoadOptions { kit: KitLoad::Only(kept), ..default() }).unwrap();
    let kit = &world.kit;
    assert!(kit.characters.is_empty(), "no bodies");
    // The impact decals stay with any kit: they are the map's impact
    // looks' (a CS:S gun's bullet holes on a CS:S map in the party apps).
    assert!(!kit.decals.groups.is_empty(), "the map's impact decals");
    assert!(kit.hud.is_none() && kit.radio.is_none() && kit.bot_profiles.is_none() && kit.nav.is_none());
    assert!(kit.muzzle_flash.is_none() && kit.shells.is_empty() && kit.shell_physics.is_none());
    let held = |m: &MapData| -> Vec<String> { m.kit.held.iter().map(|h| h.key.clone()).filter(|k| k.starts_with("cs_source:weapon_")).collect() };
    let whole = (cs_source::game::MAP_LOADER.load)("de_dust2", &MapLoadOptions::default()).unwrap();
    assert!(!held(&world).is_empty(), "the kept guns' world models load");
    assert_eq!(held(&world), held(&whole), "the same kept guns as a whole load's");
    assert!(
        world.textures.len() < whole.textures.len(),
        "fewer textures: {} against {}",
        world.textures.len(),
        whole.textures.len()
    );
    assert_eq!(world.meshes.len(), whole.meshes.len(), "the same world");
}

fn clip(sim: &Sim, p: Entity) -> u32 {
    let w = sim.app.world().get::<Inventory>(p).unwrap().active.unwrap();
    sim.app.world().get::<Magazine>(w).unwrap().clip
}

#[test]
fn a_combat_arms_soldier_plays_dust2_against_bots() {
    let Some(map) = dust2() else { return };
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
    sim.ticks(2);
    with_mode(sim.app.world_mut(), "TDM");
    let alpha = {
        let w = sim.app.world_mut();
        let mut q = w.query::<(&Transform, &SpawnPoint)>();
        q.iter(w).find(|(_, s)| s.team == Some(Team(ALPHA))).map(|(t, _)| t.translation).unwrap()
    };

    // The soldier: on dust2's floor, at Combat Arms' size.
    let p = sim.spawn_character(alpha, movement::ID);
    sim.app.world_mut().entity_mut(p).insert(Team(ALPHA));
    sim.seconds(1.0);
    assert!(sim.state(p).on_ground, "standing on dust2");
    let eye = sim.state(p).eye_offset.y;
    let at = sim.position(p);
    let floor = {
        let w = sim.app.world_mut();
        let mut state = SystemState::<SpatialQuery>::new(w);
        let q = state.get(w).unwrap();
        let filter = SpatialQueryFilter::default().with_excluded_entities([p]);
        q.cast_ray(at, Dir3::NEG_Y, 10.0, true, &filter).map(|h| at.y - h.distance)
    };
    let feet_to_eye = at.y + eye - floor.expect("a floor below");
    eprintln!("dust2: the soldier's eye is {feet_to_eye:.2} m above the floor");
    assert!((1.4..2.0).contains(&feet_to_eye), "Combat Arms' eye height in metres: {feet_to_eye}");

    // Walks.
    let start = sim.position(p);
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(1.0);
    sim.intent(p).move_axis = Vec2::ZERO;
    let walked = (sim.position(p) - start).xz().length();
    assert!(walked > 1.5, "walked {walked} m");

    // Fires the M4A1.
    let w = sim.app.world().get::<Inventory>(p).unwrap().active.unwrap();
    assert_eq!(sim.app.world().get::<Weapon>(w).unwrap().id, weapons::RIFLE);
    let full = clip(&sim, p);
    sim.intent(p).fire = true;
    sim.seconds(0.5);
    sim.intent(p).fire = false;
    sim.ticks(2);
    assert!(full - clip(&sim, p) >= 2, "the M4A1 fired: {} of {full} left", clip(&sim, p));

    // Bots, three a side with the soldier, fight (the soldier stays: a
    // match with no one in it is over).
    for team in [ALPHA, BRAVO, ALPHA, BRAVO, ALPHA, BRAVO] {
        bots::add_bot(sim.app.world_mut(), Some(team)).unwrap();
    }
    let mut deaths = 0;
    let mut respawned = 0;
    let mut dead_now: HashSet<Entity> = HashSet::new();
    for _ in 0..(120.0 / combat_arms::game::TICK_INTERVAL) as usize {
        sim.ticks(1);
        let w = sim.app.world_mut();
        let dead: HashSet<Entity> = w.query_filtered::<Entity, (With<CaBot>, With<Dead>)>().iter(w).collect();
        deaths += dead.difference(&dead_now).count();
        respawned += dead_now.difference(&dead).count();
        dead_now = dead;
    }
    let areas = sim.app.world().get_resource::<NavMesh>().map_or(0, |m| m.areas.len());
    let score = sim.app.world().resource::<CaMatch>().score;
    eprintln!("dust2: {areas} generated areas, {deaths} deaths, {respawned} respawns, score {score:?}");
    assert!(areas > 50, "navigation generated from dust2's collision: {areas} areas");
    assert!(deaths >= 2, "bots killed each other: {deaths}");
    assert!(score.iter().sum::<i32>() >= 2, "kills scored: {score:?}");
    assert!(respawned >= 1, "the dead came back");
}
