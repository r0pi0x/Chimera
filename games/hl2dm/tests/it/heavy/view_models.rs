//! Every HL2:DM weapon's view model from the install: it loads through
//! `maps::add_view_models` (as a map's kit does), with its hands, and each
//! mesh's material resolves to a base texture (none missing or the error
//! texture); each weapon's shells are the spec's (the brass events and
//! the shell models). `--nocapture` prints each model's lighting origin and every
//! mesh's material, texture and look. Skips without an install.
//!
//! (Checked 2026-11-03: the RPG drawing black and the frag not showing on
//! dm_lockdown's spawn corridor are the lighting there, not the materials:
//! that corridor is reached by none of the map's 57 `light_spot`s, so the
//! view model gets only the ambient cube (about 0.01-0.1), and the RPG's
//! base texture is the darkest of the weapons (mean sRGB about 17; the
//! AR2's about 110). Under a light both draw textured.)

use hl2dm::game::{maps, weapons};
use source_engine::content::material::MaterialLoader;

use super::map_stock::install_mount;

#[test]
fn every_view_model_loads_textured() {
    let Some(mount) = install_mount() else { return };
    let mut materials = MaterialLoader::guest(&mount, 0, 0);
    let mut models = Vec::new();
    let mut warnings = Vec::new();
    maps::add_view_models(&mut materials, chimera_world::map::KitLoad::All, &mut models, &mut warnings);
    materials.finish_textures();
    let mut wrong: Vec<String> = warnings.iter().map(|w| format!("warning: {w}")).collect();
    let want = weapons::view_models().len();
    if models.len() != want {
        wrong.push(format!("{} of {want} view models loaded", models.len()));
    }
    for v in &models {
        if !v.model.meshes.iter().any(|x| x.material == "v_hand_sheet") {
            wrong.push(format!("{}: no hands", v.key));
        }
        eprintln!("{}: light origin {:?}", v.key, v.light_origin);
        for x in &v.model.meshes {
            let tex = x.texture.map(|t| &materials.textures[t]);
            eprintln!(
                "  {} -> {:?} alpha {:?} selfillum {} envmap {} unlit {}",
                x.material,
                tex.map(|t| &t.name),
                x.alpha,
                x.selfillum.is_some(),
                x.envmap.is_some(),
                x.unlit
            );
            match tex {
                None => wrong.push(format!("{}: {} untextured", v.key, x.material)),
                Some(t) if t.name.contains("error") || t.name.is_empty() => {
                    wrong.push(format!("{}: {} draws {}", v.key, x.material, t.name))
                }
                _ => {}
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}


/// specs/cs_source/shaders_phong.md, Stock content: 17 of HL2:DM's view
/// model materials take the Phong shader (crowbar 2, crossbow 3, frag 4,
/// hands 1, AR2 1, gravity gun 2, pistol 1, RPG 1, shotgun 1, SMG1 1; one
/// of the crossbow's and one of the gravity gun's are other skins'), the
/// frag's body (`$phong 1`, no normal map) the plain model shader, none
/// the cheap variant; the pistol's fresnel ranges are [.1 5 1].
#[test]
fn view_model_materials_route_to_phong_as_the_spec_counts() {
    let Some(mount) = install_mount() else { return };
    let mut materials = MaterialLoader::guest(&mount, 0, 0).lean(true);
    let mut models = Vec::new();
    let mut warnings = Vec::new();
    maps::add_view_models(&mut materials, chimera_world::map::KitLoad::All, &mut models, &mut warnings);
    // By (view model, material): two models may each have a material of
    // one name; the hands are one material.
    let mut phong = std::collections::BTreeMap::new();
    for v in &models {
        for x in &v.model.meshes {
            let key = if x.material == "v_hand_sheet" { String::new() } else { v.key.clone() };
            if let Some(p) = &x.phong {
                phong.insert((key, x.material.clone()), (**p).clone());
            }
        }
    }
    for (name, p) in &phong {
        eprintln!("{name:?}: {p:?}");
    }
    // The default skins draw 15; the other two are the crossbow's glowing
    // skin and the super gravity gun's.
    for alt in ["models/weapons/v_crossbow/crossbow dirtmap_glow", "models/weapons/v_physcannon/v_superphyscannon_sheet"] {
        assert!(materials.resolve(alt).phong.is_some(), "{alt}");
    }
    assert_eq!(phong.len(), 15, "{:?}", phong.keys().collect::<Vec<_>>());
    assert!(!phong.keys().any(|(_, m)| m == "grenade body"));
    assert!(phong.values().all(|p| !p.cheap));
    assert!(phong.values().any(|p| p.fresnel_ranges == [0.1, 5.0, 1.0]));
}

/// effects_sounds.md, "Gravity gun sprites and beams": the view model has
/// the claws' `fork1b` to `fork2t` attachments and the world model `fork1m`
/// to `fork3t` (`game::physcannon_fx` draws on them; attachment 1 is the
/// core's), loaded into `MapViewModel::attachments` and
/// `MapHeldModel::attachments`.
#[test]
fn the_gravity_gun_models_have_the_claws_attachments() {
    use hl2dm::game::physcannon_fx::{VIEW_GLOWS, WORLD_GLOWS};
    let Some(mount) = install_mount() else { return };
    let mut materials = MaterialLoader::guest(&mount, 0, 0).lean(true);
    let mut models = Vec::new();
    let mut warnings = Vec::new();
    maps::add_view_models(&mut materials, chimera_world::map::KitLoad::All, &mut models, &mut warnings);
    let v = models.iter().find(|v| v.key == weapons::PHYSCANNON).expect("the gun's view model");
    let names: Vec<&str> = v.attachments.iter().map(|a| a.name.as_str()).collect();
    eprintln!("v_physcannon: {names:?}");
    for want in VIEW_GLOWS {
        assert!(names.iter().any(|n| n.eq_ignore_ascii_case(want)), "view model: no {want} in {names:?}");
    }
    let held = source_engine::content::props::load_held(&mut materials, "models/weapons/w_physics.mdl", "w", &[])
        .expect("w_physics");
    let names: Vec<&str> = held.attachments.iter().map(|(n, _)| n.as_str()).collect();
    eprintln!("w_physics: {names:?}");
    for want in WORLD_GLOWS {
        assert!(names.iter().any(|n| n.eq_ignore_ascii_case(want)), "world model: no {want} in {names:?}");
    }
}

/// effects_sounds.md, "Projectile visuals" (`rpg_beam_1st`, `rpg_beam_3rd`,
/// open question 6): the RPG's view model has attachment 2 `laser` and 3
/// `laser_end` (`game::rpg_fx` draws the beam between them), and the world
/// model `w_rocket_launcher.mdl` 4 `guide` and 5 `guide_target` (read
/// 2026-11-04: 5 attachments, `muzzle`, `missile`, `exhaust`, `guide`,
/// `guide_target`; the spec's open question 6 counted 3), where the
/// third-person beam goes.
#[test]
fn the_rpg_models_have_the_lasers_attachments() {
    use hl2dm::game::rpg_fx::{VIEW_FROM, VIEW_TO, WORLD_FROM, WORLD_TO};
    let Some(mount) = install_mount() else { return };
    let mut materials = MaterialLoader::guest(&mount, 0, 0).lean(true);
    let mut models = Vec::new();
    let mut warnings = Vec::new();
    maps::add_view_models(&mut materials, chimera_world::map::KitLoad::All, &mut models, &mut warnings);
    let v = models.iter().find(|v| v.key == weapons::RPG).expect("the RPG's view model");
    let names: Vec<&str> = v.attachments.iter().map(|a| a.name.as_str()).collect();
    eprintln!("v_rpg: {names:?}");
    assert!(names.get(VIEW_FROM).is_some_and(|n| n.eq_ignore_ascii_case("laser")), "{names:?}");
    assert!(names.get(VIEW_TO).is_some_and(|n| n.eq_ignore_ascii_case("laser_end")), "{names:?}");
    let held = source_engine::content::props::load_held(&mut materials, "models/weapons/w_rocket_launcher.mdl", "w", &[])
        .expect("w_rocket_launcher");
    let names: Vec<&str> = held.attachments.iter().map(|(n, _)| n.as_str()).collect();
    eprintln!("w_rocket_launcher: {names:?}");
    assert!(names.get(WORLD_FROM).is_some_and(|n| n.eq_ignore_ascii_case("guide")), "{names:?}");
    assert!(names.get(WORLD_TO).is_some_and(|n| n.eq_ignore_ascii_case("guide_target")), "{names:?}");
}

/// Each weapon's shell type, as its view model's events from the install
/// give it (specs/hl2dm/effects_sounds.md, Constants "Shells", probe case
/// P6): the SMG1 ejects type 0 at cycle 0 of each fire sequence, the
/// shotgun type 2 at 0.3125 of its pump, every other weapon nothing; both
/// shell models load with meshes. And each gun's fire sequences give its
/// muzzle flash (their named `AE_MUZZLEFLASH` event).
#[test]
fn each_weapons_shells_and_flashes_are_the_specs() {
    use chimera_world::map::ViewModelEventKind;
    let Some(mount) = install_mount() else { return };
    let mut materials = MaterialLoader::guest(&mount, 0, 0);
    let mut models = Vec::new();
    let mut warnings = Vec::new();
    maps::add_view_models(&mut materials, chimera_world::map::KitLoad::All, &mut models, &mut warnings);
    let mut wrong = Vec::new();
    for v in &models {
        // (sequence, cycle, shell type) of every brass event.
        let mut brass: Vec<(String, f32, String)> = Vec::new();
        let mut fires = Vec::new();
        let mut flashes = 0;
        for s in v.animations.iter().flat_map(|a| a.sequences.iter()) {
            if s.activity == "ACT_VM_PRIMARYATTACK" || s.activity.starts_with("ACT_VM_RECOIL") {
                fires.push(s.name.clone());
            }
            for e in &s.events {
                if matches!(hl2dm::game::flashes::event_effect(e), Some(ViewModelEventKind::MuzzleFlashOf { .. })) {
                    flashes += 1;
                }
                if let Some(ViewModelEventKind::EjectBrass { attachment, shell, .. }) = hl2dm::game::flashes::event_effect(e) {
                    if attachment >= v.attachments.len() {
                        wrong.push(format!("{}: brass from attachment {attachment} of {}", v.key, v.attachments.len()));
                    }
                    brass.push((s.name.clone(), e.cycle, shell));
                }
            }
        }
        eprintln!("{}: {brass:?}, {flashes} muzzle flashes", v.key);
        // The guns' fire sequences flash (spec 4: the named event).
        let guns = [weapons::PISTOL, weapons::MAGNUM, weapons::SMG1, weapons::AR2, weapons::SHOTGUN];
        if guns.contains(&v.key.as_str()) && flashes == 0 {
            wrong.push(format!("{}: no muzzle flash event read", v.key));
        }
        let types: Vec<&str> = brass.iter().map(|b| b.2.as_str()).collect();
        if v.key == weapons::SMG1 {
            if brass.len() != fires.len() || brass.iter().any(|b| b.1 != 0.0 || b.2 != "0" || !fires.contains(&b.0)) {
                wrong.push(format!("{}: {brass:?}, fire sequences {fires:?}", v.key));
            }
        } else if v.key == weapons::SHOTGUN {
            if brass.len() != 1 || (brass[0].1 - 0.3125).abs() > 0.01 || types != ["2"] {
                wrong.push(format!("{}: {brass:?}", v.key));
            }
        } else if !brass.is_empty() {
            wrong.push(format!("{}: ejects {brass:?}", v.key));
        }
    }
    let mut warnings = Vec::new();
    let shells = hl2dm::game::shells::load(&mut materials, &mut warnings);
    wrong.extend(warnings);
    if shells.len() != hl2dm::game::shells::SHELLS.len() || shells.iter().any(|s| s.model.meshes.is_empty()) {
        wrong.push(format!("{} shell models loaded", shells.len()));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
