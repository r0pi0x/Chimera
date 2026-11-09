//! The effect materials from the install (`effects::load_materials`):
//! every one the effects name loads, and the `Sprite`-shader glows the
//! effects draw in glow or additive render mode (the stuck bolt's glow,
//! the punt beam, the gravity gun's claws, the RPG's laser beams) add rather than blend (a blended `light_glow02_noz`
//! drew a black square around the bolt). Skips without an install.

use chimera_world::map::particles::ParticleBlend;
use hl2dm::game::{effects, physcannon_fx, projectile_fx, rpg_fx};
use source_engine::content::material::MaterialLoader;

use super::map_stock::install_mount;

#[test]
fn effect_materials_load_and_glows_add() {
    let Some(mount) = install_mount() else { return };
    let mut loader = MaterialLoader::guest(&mount, 0, 0);
    let particles = effects::load_materials(&mut loader);
    let missing: Vec<&str> = effects::MATERIALS
        .iter()
        .chain(projectile_fx::MATERIALS)
        .chain(physcannon_fx::MATERIALS)
        .chain(rpg_fx::MATERIALS)
        .chain(std::iter::once(&hl2dm::game::flashes::SMOKE))
        .copied()
        .filter(|m| particles.find(m).is_none())
        .collect();
    assert!(missing.is_empty(), "missing: {missing:?}");
    for name in effects::ADDITIVE_SPRITES {
        let m = &particles.materials[particles.find(name).unwrap()];
        assert_eq!(m.blend, ParticleBlend::Additive, "{name}");
        assert!(m.vertex_color && m.vertex_alpha, "{name}");
        assert!(m.texture.is_some(), "{name}: no texture");
    }
}

/// Depth comes from what each material's file says, not its name:
/// `glow04_noz` and `orangecore1` have their `$ignorez` commented out.
#[test]
fn ignorez_is_what_the_files_say() {
    let Some(mount) = install_mount() else { return };
    let mut loader = MaterialLoader::guest(&mount, 0, 0);
    let particles = effects::load_materials(&mut loader);
    for (name, no_depth) in [
        (physcannon_fx::CORE, false),
        (physcannon_fx::BLAST, false),
        (physcannon_fx::GLOW, false),
        (physcannon_fx::END_CAP, false),
        (physcannon_fx::WORLD_BEAM, false),
        (physcannon_fx::VIEW_BEAM, true),
        (rpg_fx::VIEW_BEAM, true),
    ] {
        let m = &particles.materials[particles.find(name).unwrap_or_else(|| panic!("{name}"))];
        assert_eq!(m.no_depth, no_depth, "{name}");
    }
}
