//! Every game's guns draw their effects with their own materials on
//! every game's map (the installs'): the party's maps (`party_kit::
//! load_map`, each dressed with the other games' parts) carry each game's
//! effect materials (HL2:DM's energy ball, streaks, flashes, the RPG's
//! laser, the crossbow's and gravity gun's sprites; CS:S's impacts,
//! grenades and fire; Combat Arms' effect library) with a decoded
//! texture, and each game's muzzle flashes (CS:S's kit flash, HL2:DM's
//! by kind). Before, HL2:DM's came only with its own maps' kit: its AR2's
//! ball on de_dust2 found no material and showed its bare model, a white
//! quad. Passes with a note without the three installs.

use chimera_core::mount::config::LocalConfig;
use chimera_world::map::{MapData, MapMuzzleFlash};

const MAPS: [&str; 3] = ["cs_source:de_dust2", "combat_arms:warehouse", "hl2dm:dm_lockdown"];

/// Whether texture `t` of `data` is a real one (decoded, not the 1x1
/// stand-in of a texture that failed).
fn real(data: &MapData, t: Option<usize>) -> bool {
    t.and_then(|i| data.textures.get(i))
        .is_some_and(|t| t.width > 1 && t.height > 1)
}

/// The particle materials of `data` by name, each with whether its
/// texture is real.
fn materials(data: &MapData) -> std::collections::BTreeMap<String, bool> {
    data.particles
        .materials
        .iter()
        .map(|m| (m.name.clone(), real(data, m.texture)))
        .collect()
}

fn flash_real(data: &MapData, f: &MapMuzzleFlash) -> bool {
    real(data, f.texture)
}

#[test]
fn every_games_effects_have_their_materials_on_every_map() {
    let config = LocalConfig::load().ok();
    let usable = config.is_some_and(|c| {
        c.game_path_in(cs_source::game::GAME, cs_source::game::INFO.steam.as_ref())
            .is_some_and(|p| chimera_core::mount::served::usable("cs_source", &p, "cstrike"))
            && c.game_path(combat_arms::game::GAME)
                .is_some_and(|p| chimera_core::mount::served::usable("combat_arms", &p, "Game"))
    }) && hl2dm::game::INFO.install().is_some();
    if !usable {
        eprintln!("skipping: needs the CS:S, Combat Arms and HL2:DM installs");
        return;
    }
    let maps: Vec<(&str, MapData)> = MAPS.iter().map(|m| (*m, party_kit::load_map(m, 0).unwrap())).collect();
    let mut problems = Vec::new();

    // HL2:DM's guns' effect materials: each that its own map has.
    let hl2 = &maps[2].1;
    let mut wanted: Vec<&str> = [
        hl2dm::game::effects::MATERIALS,
        hl2dm::game::projectile_fx::MATERIALS,
        hl2dm::game::physcannon_fx::MATERIALS,
        hl2dm::game::rpg_fx::MATERIALS,
        hl2dm::game::set_piece_fx::MATERIALS,
    ]
    .concat();
    wanted.retain(|n| hl2.particles.find(n).is_some());
    for must in [
        "effects/ar2_altfire1",
        "effects/ar2_altfire1b",
        "effects/gunshiptracer",
        "effects/laser1",
        "sprites/orangecore1",
    ] {
        assert!(wanted.contains(&must), "HL2:DM's own map has no {must}: the install's?");
    }
    // Every material any map's games carry with a real texture is on every
    // map with one (a map's own extras aside: CS:S's slime splashes).
    let all: Vec<_> = maps.iter().map(|(m, d)| (*m, materials(d))).collect();
    for (map, mats) in &all {
        for (other, theirs) in &all {
            for (name, ok) in theirs {
                if *ok && mats.get(name) != Some(&true) {
                    problems.push(format!(
                        "{map}: {name} (on {other}) {}",
                        if mats.contains_key(name) { "white" } else { "missing" }
                    ));
                }
            }
        }
        for name in &wanted {
            if mats.get(*name) != Some(&true) {
                problems.push(format!("{map}: HL2:DM's {name} missing or white"));
            }
        }
    }
    // The muzzle flashes: CS:S's (no kind) and HL2:DM's kinds.
    let mut kinds: Vec<String> = maps.iter().flat_map(|(_, d)| d.kit.muzzle_flash_kinds.iter().map(|(k, _)| k.clone())).collect();
    kinds.sort();
    kinds.dedup();
    if kinds.len() < 5 {
        problems.push(format!("HL2:DM's muzzle flash kinds on no map: {kinds:?}"));
    }
    for (map, data) in &maps {
        match &data.kit.muzzle_flash {
            Some(f) if flash_real(data, f) => {}
            _ => problems.push(format!("{map}: CS:S's muzzle flash missing or white")),
        }
        for k in &kinds {
            match data.kit.muzzle_flash_kinds.iter().find(|(n, _)| n == k) {
                Some((_, f)) if flash_real(data, f) => {}
                _ => problems.push(format!("{map}: HL2:DM's {k} muzzle flash missing or white")),
            }
        }
        // Each game's impacts can be drawn there.
        for game in [cs_source::game::GAME, hl2dm::game::GAME, combat_arms::game::GAME] {
            if !data.impacts.has(game) {
                problems.push(format!("{map}: no {game} impacts"));
            }
        }
    }
    for (map, data) in &maps {
        eprintln!(
            "{map}: {} particle materials, {} flash kinds",
            data.particles.materials.len(),
            data.kit.muzzle_flash_kinds.len()
        );
    }
    problems.sort();
    problems.dedup();
    assert!(
        problems.is_empty(),
        "{} problems:\n{}",
        problems.len(),
        problems.join("\n")
    );
}
