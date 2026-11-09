//! Every sound entry HL2:DM plays resolves in the install's scripts
//! (specs/hl2dm/effects_sounds.md P2): the weapons' (`sounds::entries`),
//! the suit's, pickups', chargers' (`maps::SOUNDS`), each defined and with
//! its waves in the files (except the frag's `common/null.wav`); a
//! duplicated entry keeps its first definition (`NPC_Citizen.die`: the
//! 3-wave `hl2mp` one, spec test 60); the install's `decals_subrect.txt`
//! gives each surface letter a decal. Skips without an install.

use chimera_world::map::{MapLoadOptions, entities::PROP_BREAK_SOUND_KEY};
use hl2dm::game::{maps, sounds};
use source_engine::content::{decals, material::MaterialLoader, sound::SoundScripts};

use super::map_stock::{convert, install_mount};

/// The HL2:DM playtest's silent sounds: each is in a converted map's bank
/// with its waves decoded, so `PlaySound` finds it. (A name missing there
/// plays nothing, with no error.) And what the map's toilets and sinks
/// break with: nothing of their own on `porcelain` and `concrete`
/// (specs/source/prop_damage.md 7.4, the original's too), the pieces'
/// bounces only.
#[test]
fn the_playtests_sounds_are_in_the_maps_bank() {
    let Some(mount) = install_mount() else { return };
    let data = convert(&mount, "dm_lockdown", &MapLoadOptions::default());
    let mut missing = Vec::new();
    for entry in [
        "Weapon_PhysCannon.Pickup",
        "Weapon_PhysCannon.Launch",
        "Weapon_PhysCannon.Drop",
        "Weapon_PhysCannon.HoldSound",
        "Weapon_SMG1.Double",
        "BaseExplosionEffect.Sound",
        "Weapon_IRifle.Single",
        "NPC_CombineBall.Launch",
        "Weapon_Crowbar.Single",
        "Weapon_StunStick.Melee_Hit",
        "Concrete.BulletImpact",
        "NPC_Citizen.RunFootstepLeft",
    ] {
        match data.sounds.entry(entry) {
            Some(e) if !e.waves.is_empty() => {}
            Some(_) => missing.push(format!("{entry}: no waves")),
            None => missing.push(format!("{entry}: not in the bank")),
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));
    let mut sinks = std::collections::BTreeSet::new();
    for e in &data.entities {
        let model = e.get("model").unwrap_or("").to_lowercase();
        if model.contains("toilet") || model.contains("sink") {
            sinks.insert((model, e.get(PROP_BREAK_SOUND_KEY).unwrap_or("").to_string()));
        }
    }
    assert!(!sinks.is_empty(), "the map's toilets and sinks");
    for (model, sound) in &sinks {
        assert!(sound.is_empty(), "{model} breaks with {sound}");
    }
}

/// The playtest's silent footsteps on de_dust2 in Ultimate Gun Game: on
/// another game's map a character is HL2:DM's with what `dress_guest` brought,
/// and it brought no sounds: its footsteps and guns played nothing there.
#[test]
fn another_games_map_gets_the_sounds() {
    let Some(_) = install_mount() else { return };
    let mut map = chimera_world::map::MapData::default();
    maps::dress_guest(&mut map, hl2dm::game::BUNDLE.kept).unwrap();
    for entry in ["NPC_Citizen.RunFootstepLeft", "NPC_CombineS.RunFootstepRight", "Weapon_SMG1.Single", "Weapon_PhysCannon.Launch"] {
        let e = map.sounds.entry(entry).unwrap_or_else(|| panic!("{entry} isn't in the bank"));
        assert!(!e.waves.is_empty() && e.waves.iter().all(|w| *w < map.sounds.clips.len()), "{entry}");
    }
}

#[test]
fn every_entry_resolves() {
    let Some(mount) = install_mount() else { return };
    let mut materials = MaterialLoader::guest(&mount, 0, 0).lean(true);
    let scripts = SoundScripts::load_with(&mut materials, "dm_lockdown", true);
    let mut missing = Vec::new();
    for entry in sounds::entries().iter().map(String::as_str).chain(maps::SOUNDS.iter().copied()) {
        // The spec names it, but the install's scripts don't define it: the
        // original plays nothing there either.
        if entry == "TripmineGrenade.StopSound" {
            continue;
        }
        if !scripts.contains(entry) {
            missing.push(format!("{entry}: no entry"));
            continue;
        }
        for wave in scripts.waves(entry) {
            if wave != "sound/common/null.wav" && materials.read(&wave).is_none() {
                missing.push(format!("{entry}: {wave} is not in the install"));
            }
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}

#[test]
fn the_first_citizen_death_wins() {
    let Some(mount) = install_mount() else { return };
    let mut materials = MaterialLoader::guest(&mount, 0, 0).lean(true);
    let first = SoundScripts::load_with(&mut materials, "dm_lockdown", true);
    assert_eq!(first.waves("NPC_Citizen.die").len(), 6, "three per gender");
    let last = SoundScripts::load_with(&mut materials, "dm_lockdown", false);
    assert!(last.waves("NPC_Citizen.die").len() > 6, "the last definition has nine");
}

#[test]
fn the_install_has_impact_decals() {
    let Some(mount) = install_mount() else { return };
    let mut materials = MaterialLoader::guest(&mount, 0, 0).lean(true);
    let d = decals::impact_decals(&mut materials);
    assert!(!d.decals.is_empty() && !d.groups.is_empty() && !d.by_material.is_empty());
    for letter in ['C', 'M', 'W', 'D'] {
        assert!(d.by_material.contains_key(&letter), "{letter}");
    }
}
