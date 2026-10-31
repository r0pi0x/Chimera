//! A player holding a CS:S gun on Combat Arms' Warehouse is CS:S's: the
//! party's map sources add CS:S's player bodies, held and view models
//! from the CS:S install to a Combat Arms map (`MapSources::guests`), so
//! the player has a CS:S body (with its hitboxes) instead of the shell's
//! capsule: the CS:S rung's drawn model for everyone on it (rung
//! characters, the default), or without those, in free for all, a
//! different CS:S model per player instead of all the counter-terrorist
//! one. Passes with a note without
//! both installs in chimera.local.toml.

use bevy::prelude::*;
use chimera_core::mount::config::LocalConfig;
use chimera_sim::testkit;
use chimera_world::map::{BodyModel, CharacterModels, LoadedMapName};

use crate::support::*;

fn installs() -> bool {
    let Ok(config) = LocalConfig::load() else { return false };
    let css = config
        .game_path_in(cs_source::game::GAME, cs_source::game::INFO.steam.as_ref())
        .filter(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    let ca = config.game_path(combat_arms::game::GAME).filter(|p| chimera_core::mount::served::usable("combat_arms", p, "Game"));
    css.is_some() && ca.is_some()
}

/// The CS:S bodies four soldiers on the first rung (CS:S's Glock) wear on
/// Warehouse, the server started with `extra` before the minigame; None
/// without the installs.
fn first_rung_bodies(extra: &[&str]) -> Option<Vec<usize>> {
    if !installs() {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
        return None;
    }
    let mut args = extra.to_vec();
    args.extend(["+minigame", "originals:gun_game_dust2", "+map", "combat_arms:warehouse", "+maxplayers", "8"]);
    let mut sim = server(&args)?;
    assert_eq!(sim.app.world().resource::<LoadedMapName>().0, "combat_arms:warehouse");
    let models = sim.app.world().resource::<CharacterModels>().clone();
    let game_of = |i: usize| models.0[i].game;
    assert!(
        models.0.iter().any(|m| m.game == Some(cs_source::game::GAME)),
        "CS:S's player bodies are on Warehouse: {:?}",
        models.0.iter().map(|m| m.game).collect::<Vec<_>>()
    );
    assert!(models.0.iter().any(|m| m.game != Some(cs_source::game::GAME)), "Combat Arms' own bodies stay");
    let at = Vec3::new(0.0, 1.0, 0.0);
    let players: Vec<Entity> = (0..4).map(|_| soldier(&mut sim, at)).collect();
    sim.seconds(2.5);
    let mut seen = Vec::new();
    for p in &players {
        assert_eq!(testkit::active_id(sim.app.world(), *p), Some(cs_source::game::weapons::GLOCK));
        let body = sim.app.world().get::<BodyModel>(*p).copied().expect("a body on the server");
        assert_eq!(game_of(body.0), Some(cs_source::game::GAME), "{p} wears a CS:S body");
        assert!(sim.app.world().get::<chimera_core::core::Hitboxes>(*p).is_some(), "with CS:S's hitboxes");
        seen.push(body.0);
    }
    Some(seen)
}

/// Without rung characters (the host's "Rung extras: characters: off"),
/// the players of a free for all wear different CS:S models instead of
/// all the counter-terrorist one.
#[test]
fn a_cs_source_gun_on_warehouse_has_a_cs_source_body_and_each_player_a_different_one() {
    let Some(mut seen) = first_rung_bodies(&["+mp_rung_extras_character", "false"]) else { return };
    seen.sort_unstable();
    seen.dedup();
    assert!(seen.len() >= 3, "free for all: a varied model per player, not one: {seen:?}");
}

/// With them (the default: `cs_source::game::rungs`), the CS:S rung's
/// drawn model is everyone's on it.
#[test]
fn a_cs_source_rung_on_warehouse_dresses_everyone_on_it_alike() {
    let Some(seen) = first_rung_bodies(&[]) else { return };
    assert!(seen.windows(2).all(|w| w[0] == w[1]), "one rung character for all: {seen:?}");
}

/// Every CS:S gun of the gun game's ladder (and CS:S's knife) has its
/// fire sound in the map's sound bank on both of the pool's maps: on
/// Warehouse the guest kit brings CS:S's sounds along (before, a CS:S gun
/// fired there in silence, the AK-47 among them).
#[test]
fn cs_source_guns_sound_on_both_maps() {
    if !installs() {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
        return;
    }
    // The ladder's CS:S guns (minigames/gun_game_dust2.toml).
    let ladder = ["glock", "deagle", "p90", "ak47", "awp"].map(|g| format!("cs_source:weapon_{g}"));
    let entries: Vec<(String, String)> = cs_source::game::weapons::cs_weapons()
        .weapons
        .iter()
        .filter(|w| ladder.contains(&w.id) || w.id == cs_source::game::weapons::KNIFE)
        .filter_map(|w| w.sounds.as_ref()?.fire.clone().map(|f| (w.id.clone(), f)))
        .collect();
    assert!(entries.iter().any(|(id, _)| id == "cs_source:weapon_ak47"), "{entries:?}");
    for map in ["cs_source:de_dust2", "combat_arms:warehouse"] {
        let data = party_kit::load_map_with(map, &chimera_world::map::MapLoadOptions::lean()).expect("loads");
        for (gun, entry) in &entries {
            let e = data.sounds.entry(entry).unwrap_or_else(|| panic!("{map}: no {entry} for {gun}"));
            assert!(!e.waves.is_empty(), "{map}: {entry} ({gun}) has no wave read");
            for w in &e.waves {
                assert!(!data.sounds.clips[*w].samples.is_empty(), "{map}: {entry}'s wave is empty");
            }
        }
    }
}
