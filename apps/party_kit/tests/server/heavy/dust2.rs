//! The server plays the gun game on de_dust2 from the install, as its
//! command line asks (`+minigame originals:gun_game_dust2`: the pool's
//! first map, loaded without textures and dressed for Combat Arms):
//! soldiers come in on the map, the match goes live, one
//! shoots another with CS:S's Glock (its aim and trigger, the server's
//! hit) and climbs to Combat Arms' Beretta. Passes with a note without
//! both installs in chimera.local.toml.

use bevy::prelude::*;
use chimera_core::{
    core::{SpawnPoint, Team},
    gameplay::Gameplay,
    mount::config::LocalConfig,
};
use chimera_sim::{
    minigame::{self, MINIGAME, ladder::Rung},
    mode::{Match, Phase},
    testkit,
};
use chimera_world::map::LoadedMapName;

use crate::support::*;

/// The server's command line for these tests: the gun game on dust2,
/// as the host's settings "Knife finale: off" and every rung extra off
/// (the server's `gun_game::no_finale` and `plain_rungs`): the tests of
/// the guns' ten-rung ladder predate the melee finale (on by default)
/// and the rung extras (a drawn character or melee weapon in place of
/// the rung's gun); the party's own tests play those.
const PLAIN_LADDER: [&str; 12] = [
    "+mp_knife_finale",
    "false",
    "+mp_rung_extras_attachments",
    "false",
    "+mp_rung_extras_character",
    "false",
    "+mp_rung_extras_melee",
    "false",
    "+minigame",
    "originals:gun_game_dust2",
    "+maxplayers",
    "4",
];

/// Two spots in the open to duel from: dust2's first spawn point and the
/// nearest other one more than 1.5 m away (the spawns of a side stand
/// together in the open). The gun game spawns anywhere out of sight
/// (`points = "random_hidden"`), so a spot a few metres from where a
/// player came in may be behind a wall.
fn duel_spots(spawns: &[Vec3]) -> (Vec3, Vec3) {
    let near = spawns[1..]
        .iter()
        .copied()
        .filter(|s| s.distance(spawns[0]) > 1.5)
        .min_by(|x, y| x.distance(spawns[0]).total_cmp(&y.distance(spawns[0])))
        .expect("another spawn point");
    (spawns[0], near)
}

fn installs() -> bool {
    let Ok(config) = LocalConfig::load() else { return false };
    let css = config
        .game_path_in(cs_source::game::GAME, cs_source::game::INFO.steam.as_ref())
        .filter(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    let ca = config.game_path(combat_arms::game::GAME).filter(|p| chimera_core::mount::served::usable("combat_arms", p, "Game"));
    css.is_some() && ca.is_some()
}

#[test]
fn the_server_runs_the_gun_game_on_dust2() {
    if !installs() {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
        return;
    }
    let Some(mut sim) = server(&PLAIN_LADDER) else { return };
    assert_eq!(sim.app.world().resource::<LoadedMapName>().0, "cs_source:de_dust2");
    assert_eq!(sim.app.world().resource::<Gameplay>().rules, MINIGAME);
    let spawns: Vec<Vec3> = {
        let w = sim.app.world_mut();
        w.query_filtered::<&Transform, With<SpawnPoint>>().iter(w).map(|t| t.translation).collect()
    };
    assert!(spawns.len() >= 2, "dust2's spawn points: {}", spawns.len());
    let a = soldier(&mut sim, spawns[0]);
    let b = soldier(&mut sim, spawns[1]);
    sim.seconds(1.0);
    assert_eq!(sim.app.world().resource::<Match>().phase, Phase::Live);
    for p in [a, b] {
        // The gun game spawns anywhere on the map, out of sight
        // (`points = "random_hidden"`), not only at dust2's spawn points.
        let at = sim.position(p);
        assert!(at.is_finite(), "{p} came in somewhere on the map ({at})");
        assert!(sim.app.world().get::<Team>(p).unwrap().0 >= chimera_sim::mode::FFA_FIRST, "a team of its own");
        assert_eq!(testkit::active_id(sim.app.world(), p), Some(cs_source::game::weapons::GLOCK));
    }
    // a at the first point, b at the nearest other one (the spawns of a
    // side stand together in the open), protection (2 s) over.
    sim.seconds(1.5);
    let (here, near) = duel_spots(&spawns);
    place(&mut sim, a, here);
    place(&mut sim, b, near);
    sim.seconds(0.5);
    assert!(shoot_until_dead(&mut sim, a, b, 10.0), "the Glock kills on dust2");
    assert_eq!(sim.app.world().get::<Rung>(a).map(|r| r.0), Some(1));
    sim.seconds(1.5);
    assert_eq!(testkit::active_id(sim.app.world(), a), Some("combat_arms:beretta_m92fs"), "Combat Arms' pistol next");
    let status = minigame::runtime::status(sim.app.world_mut()).unwrap();
    assert!(status.contains("rung 2/10"), "{status}");
}

/// Every rung's gun, from the installs' records, has rounds in its
/// magazine when the ladder hands it over and kills (a live game once
/// gave the M60 on rung 9 with no rounds).
#[test]
fn every_rung_of_the_gun_game_has_rounds_and_kills() {
    use chimera_sim::weapon::Magazine;
    if !installs() {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
        return;
    }
    let Some(mut sim) = server(&PLAIN_LADDER) else { return };
    let spawns: Vec<Vec3> = {
        let w = sim.app.world_mut();
        w.query_filtered::<&Transform, With<SpawnPoint>>().iter(w).map(|t| t.translation).collect()
    };
    let a = soldier(&mut sim, spawns[0]);
    let b = soldier(&mut sim, spawns[1]);
    sim.seconds(3.0);
    for rung in 0..10 {
        // Wait out b's respawn and protection.
        for _ in 0..(8.0 * sim.tick_hz()) as u32 {
            let w = sim.app.world();
            if w.get::<chimera_core::core::Dead>(b).is_none() && w.get::<chimera_core::core::God>(b).is_none() {
                break;
            }
            sim.ticks(1);
        }
        let (here, near) = duel_spots(&spawns);
        place(&mut sim, a, here);
        place(&mut sim, b, near);
        sim.seconds(1.8);
        let gun = testkit::active_id(sim.app.world(), a).expect("a gun in hand");
        let mag = {
            let w = sim.app.world();
            let g = testkit::active(w, a).unwrap();
            w.get::<Magazine>(g).map(|m| (m.clip, m.size, m.reserve))
        };
        eprintln!("rung {rung}: {gun} magazine {mag:?}");
        if let Some((clip, _, _)) = mag {
            assert!(clip > 0, "rung {rung}: {gun} has no rounds in the clip: {mag:?}");
        }
        // The ladder gives each gun once: its reserve is kept full (the
        // M60's 200 rounds used to run out and end the climb).
        if mag.is_some() {
            let g = testkit::active(sim.app.world(), a).unwrap();
            sim.app.world_mut().get_mut::<Magazine>(g).unwrap().reserve = 0;
            sim.ticks(3);
            let m = sim.app.world().get::<Magazine>(g).unwrap();
            assert!(m.reserve > 0 && m.reserve == m.reserve_max, "rung {rung}: {gun} reserve not refilled: {m:?}");
        }
        assert!(shoot_until_dead(&mut sim, a, b, 10.0), "rung {rung}: {gun} doesn't kill");
        if rung < 9 {
            assert_eq!(sim.app.world().get::<Rung>(a).map(|r| r.0), Some(rung + 1));
        }
        sim.seconds(0.5);
    }
}

/// A player on any rung who dies comes back with that rung's gun and
/// rounds in it (the ladder's loadout at a respawn, not only at a kill).
#[test]
fn every_rungs_gun_has_rounds_at_a_respawn() {
    use chimera_sim::weapon::Magazine;
    if !installs() {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
        return;
    }
    let Some(mut sim) = server(&PLAIN_LADDER) else { return };
    let spawns: Vec<Vec3> = {
        let w = sim.app.world_mut();
        w.query_filtered::<&Transform, With<SpawnPoint>>().iter(w).map(|t| t.translation).collect()
    };
    let a = soldier(&mut sim, spawns[0]);
    let b = soldier(&mut sim, spawns[1]);
    sim.seconds(3.0);
    for rung in 0..10 {
        sim.app.world_mut().entity_mut(a).insert(Rung(rung));
        // b kills a (b stays on its first rung: its kills climb, so reset).
        sim.app.world_mut().entity_mut(b).insert(Rung(0));
        for _ in 0..(8.0 * sim.tick_hz()) as u32 {
            let w = sim.app.world();
            let ready = |e| w.get::<chimera_core::core::Dead>(e).is_none() && w.get::<chimera_core::core::God>(e).is_none();
            if ready(a) && ready(b) {
                break;
            }
            sim.ticks(1);
        }
        let (here, near) = duel_spots(&spawns);
        place(&mut sim, b, here);
        place(&mut sim, a, near);
        sim.seconds(1.0);
        assert!(shoot_until_dead(&mut sim, b, a, 10.0), "rung {rung}: b kills a");
        for _ in 0..(8.0 * sim.tick_hz()) as u32 {
            let w = sim.app.world();
            if w.get::<chimera_core::core::Dead>(a).is_none() {
                break;
            }
            sim.ticks(1);
        }
        sim.seconds(1.8);
        let gun = testkit::active_id(sim.app.world(), a).expect("a gun in hand");
        let mag = {
            let w = sim.app.world();
            let g = testkit::active(w, a).unwrap();
            w.get::<Magazine>(g).map(|m| (m.clip, m.size, m.reserve))
        };
        eprintln!("respawn on rung {rung}: {gun} magazine {mag:?}");
        if let Some((clip, _, _)) = mag {
            assert!(clip > 0, "rung {rung}: {gun} respawned with no rounds: {mag:?}");
        }
    }
}

/// The server's de_dust2 (loaded without textures, dressed for Combat Arms)
/// carries none of Combat Arms' view models: a server draws none, as on its
/// own maps (`world::DRESSING`; they were 0.5 GB of its memory,
/// docs/performance.md, "The dedicated server's memory"). Its Combat Arms
/// guns time as before, on their own view models' animation lengths
/// (`weapons::timing`, which reads the model's animation list, not the
/// map's): each gun with a timing table built on the server carries the
/// same timing (`CaTimingValues`, which the server sends its clients, and
/// the weapon's own cycle, reload and draw) as one built in a world with
/// no map at all.
#[test]
fn the_servers_combat_arms_guns_time_without_the_dressings_view_models() {
    use chimera_sim::weapon::{self, Magazine, Trigger, Weapon, data::WeaponCatalog};
    use chimera_world::{harness::Sim, map::ViewModels};
    use combat_arms::{
        game::{
            data::CaData,
            view_model::on_demand::ours,
            weapons::timing::{CaTimingValues, TIMING},
        },
        harness::CaSimWith,
    };
    if !installs() {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
        return;
    }
    let Some(mut sim) = server(&PLAIN_LADDER) else { return };
    let ours_drawn: Vec<String> = sim
        .app
        .world()
        .get_resource::<ViewModels>()
        .map(|m| m.0.iter().map(|v| v.key.clone()).filter(|k| ours(k)).collect())
        .unwrap_or_default();
    assert!(ours_drawn.is_empty(), "the server's dust2 carries Combat Arms' view models: {ours_drawn:?}");
    let ca = CaData::install();
    if ca.attributes().and_then(|a| a.weapons.as_ref()).is_none() {
        eprintln!("skipping the timing: the install's weapons file isn't readable (the V1 key)");
        return;
    }
    let ids: Vec<&'static str> = {
        let catalog = sim.app.world().resource::<WeaponCatalog>();
        ca.weapon_set()
            .weapons
            .iter()
            .filter(|d| d.hooks.contains_key(TIMING))
            .filter_map(|d| catalog.get(&d.id).map(|c| c.id.as_str()))
            .collect()
    };
    let timing = |w: &World, e: Entity| {
        (
            w.get::<CaTimingValues>(e).cloned(),
            w.get::<Trigger>(e).map(|t| (t.cycle, t.latency, t.postfire)),
            w.get::<Magazine>(e).map(|m| m.reload_time),
            w.get::<Weapon>(e).map(|w| w.draw_time),
        )
    };
    let owner = soldier(&mut sim, Vec3::new(0.0, 1.0, 0.0));
    let mut bare = Sim::with(CaSimWith(ca.clone()), |_| {});
    let bare_owner = bare.spawn_character(Vec3::new(0.0, 1.0, 0.0), combat_arms::game::movement::ID);
    let (mut read, mut differ) = (0, Vec::new());
    for id in &ids {
        let on_server = {
            let w = sim.app.world_mut();
            let e = weapon::spawn_weapon(w, id, owner).expect("registered on the server");
            let t = timing(w, e);
            w.despawn(e);
            t
        };
        let alone = {
            let w = bare.app.world_mut();
            let Some(e) = weapon::spawn_weapon(w, id, bare_owner) else { continue };
            let t = timing(w, e);
            w.despawn(e);
            t
        };
        assert!(on_server.0.is_some(), "{id}: the server's copy has no settled timing");
        read += 1;
        if on_server != alone {
            differ.push(format!("{id}: server {on_server:?}, no map {alone:?}"));
        }
    }
    eprintln!("{} Combat Arms guns with a timing table on the server; {read} compared", ids.len());
    assert!(read > 0, "no Combat Arms gun with a timing table on the server");
    assert!(differ.is_empty(), "{} guns time differently on the server: {:#?}", differ.len(), &differ[..differ.len().min(10)]);
}

/// The server's de_dust2 (loaded without textures, dressed for Combat Arms)
/// holds no Combat Arms effect library: the parsed effect file was 270 MB of
/// the server's memory, and what reads it draws or plays local sounds
/// (`effects::library::add_looks`; docs/performance.md, "The dedicated
/// server's memory"). Combat Arms' impacts stay registered on the map, so a
/// Combat Arms gun's impacts route to Combat Arms on the server as before.
#[test]
fn the_servers_dust2_routes_combat_arms_impacts_without_the_effect_file() {
    use chimera_sim::weapon::impact::{ImpactBy, ImpactProviders, impact_by};
    use chimera_world::map::impact::ActiveImpacts;
    use combat_arms::game::{GAME, effects::library};
    if !installs() {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
        return;
    }
    let Some(sim) = server(&PLAIN_LADDER) else { return };
    let w = sim.app.world();
    assert!(library::library_in(w).is_none(), "the server's dust2 holds Combat Arms' effect library");
    let impacts = w.resource::<ActiveImpacts>();
    assert!(impacts.0.looks.iter().any(|g| g == GAME), "Combat Arms' impacts aren't registered: {:?}", impacts.0);
    let providers = w.resource::<ImpactProviders>();
    assert_eq!(impact_by(Some(GAME), Some(impacts), &providers.0), ImpactBy::Game(GAME));
}
