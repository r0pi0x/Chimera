//! Every HL2:DM weapon's view model from the install: it loads through
//! `maps::add_view_models` (as a map's kit does), with its hands, and each
//! mesh's material resolves to a base texture (none missing or the error
//! texture). `--nocapture` prints each model's lighting origin and every
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
