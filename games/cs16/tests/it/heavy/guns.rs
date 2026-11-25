//! Every CS 1.6 weapon's files on the user's install (`game::weapons`,
//! `goldsrc_engine::content::weapon_models`, `sounds`): each view model
//! reads with its meshes, bones, sequences (the ones the view rules play
//! among them) and the waves of its sound events; each held model sits on
//! a hand of the player skeleton; each dropped `w_` model parses; every
//! sound the data and the view models name decodes. A kit with them all
//! keeps the player bodies' hands and holds the muzzle flashes the view
//! models' events name. Passes with a note without an install.

use chimera_world::map::MapData;
use cs16::game::{bodies, weapons};
use goldsrc_engine::content::{sounds, weapon_models};

use super::maps::mount;

#[test]
fn every_weapons_models_and_sounds_load() {
    let Some(mount) = mount() else { return };
    let mut problems = Vec::new();
    for w in &weapons::weapons().weapons {
        let view = w.models.view.as_deref().expect("a view model");
        match weapon_models::view_model(&mount, view, &w.id, false) {
            Ok((v, textures, waves)) => {
                let set = v.animations.as_ref().expect("sequences");
                if v.model.meshes.is_empty() || v.bones.is_empty() || textures.is_empty() {
                    problems.push(format!("{view}: empty"));
                }
                if set.sequence("idle").or(set.sequence("idle1")).is_none() {
                    problems.push(format!("{view}: no idle"));
                }
                for wave in waves {
                    let mut table = Default::default();
                    if !sounds::add_entry(&mount, &wave, &mut table) {
                        problems.push(format!("{view}: event sound {wave}"));
                    }
                }
            }
            Err(e) => problems.push(e),
        }
        let held = w.models.world.as_deref().expect("a held model");
        match weapon_models::held(&mount, held, &w.id, weapons::SKELETON, false) {
            Ok((h, _)) => {
                if !h.bone.ends_with("Hand") {
                    problems.push(format!("{held}: held on {}", h.bone));
                }
            }
            Err(e) => problems.push(e),
        }
        let dropped = weapons::dropped_model(w).expect("a dropped model");
        match mount.read(&dropped) {
            Ok(bytes) => {
                if let Err(e) = goldsrc_formats::mdl::Mdl::parse(bytes) {
                    problems.push(format!("{dropped}: {e}"));
                }
            }
            Err(e) => problems.push(format!("{dropped}: {e}")),
        }
        for entry in weapons::data_sounds(w) {
            for wave in sounds::waves(&entry) {
                let mut table = Default::default();
                if !sounds::add_entry(&mount, wave, &mut table) {
                    problems.push(format!("{}: sound {wave}", w.id));
                }
            }
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
    // CS 1.6's view models are built left of the eye (mirrored for the
    // right hand, as `cl_righthand 1` does).
    let ak = weapons::data(weapons::AK47).unwrap();
    let (v, _, _) = weapon_models::view_model(&mount, ak.models.view.as_deref().unwrap(), weapons::AK47, false).unwrap();
    assert!(!v.right_handed && v.allow_flipping);
    // The Elites hold a gun in each hand.
    let elite = weapons::data(weapons::ELITE).unwrap();
    let (h, _) = weapon_models::held(&mount, elite.models.world.as_deref().unwrap(), weapons::ELITE, weapons::SKELETON, false).unwrap();
    assert_eq!(h.merged.len(), 1, "the left gun on its hand");
    // A whole kit beside the bodies.
    let mut map = MapData::default();
    bodies::add_bodies(&mount, false, &mut map);
    let names = weapons::skeleton_names(&map);
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    weapons::add_guns(&mount, &|_| true, &names, false, &mut map);
    assert_eq!(map.kit.view_models.len(), weapons::weapons().weapons.len(), "{:?}", map.warnings);
    assert_eq!(map.kit.held.len(), weapons::weapons().weapons.len(), "{:?}", map.warnings);
    assert!(map.sounds.entries.contains_key("weapons/ak47-1.wav;weapons/ak47-2.wav"));
    assert!(map.sounds.entries.contains_key("weapons/ak47_clipout.wav"));
    // The AK-47's muzzle flash (its shots' event code 22: the second
    // sprite at scale 2).
    let flash = map.kit.muzzle_flash_kinds.iter().find(|(k, _)| k == "cs16:22").map(|(_, f)| f).expect("the AK's flash");
    assert_eq!(map.textures[flash.texture.unwrap()].name, "sprites/muzzleflash2.spr");
}
