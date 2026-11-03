//! Every sound entry HL2:DM plays resolves in the install's scripts
//! (specs/hl2dm/effects_sounds.md P2): the weapons' (`sounds::entries`),
//! the suit's, pickups', chargers' (`maps::SOUNDS`), each defined and with
//! its waves in the files (except the frag's `common/null.wav`); a
//! duplicated entry keeps its first definition (`NPC_Citizen.die`: the
//! 3-wave `hl2mp` one, spec test 60); the install's `decals_subrect.txt`
//! gives each surface letter a decal. Skips without an install.

use hl2dm::game::{maps, sounds};
use source_engine::content::{decals, material::MaterialLoader, sound::SoundScripts};

use super::map_stock::install_mount;

#[test]
fn every_entry_resolves() {
    let Some(mount) = install_mount() else { return };
    let mut materials = MaterialLoader::guest(&mount, 0, 0).lean(true);
    let scripts = SoundScripts::load_with(&mut materials, "dm_lockdown", true);
    let mut missing = Vec::new();
    for entry in sounds::entries().iter().map(String::as_str).chain(maps::SOUNDS.iter().copied()) {
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
