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

