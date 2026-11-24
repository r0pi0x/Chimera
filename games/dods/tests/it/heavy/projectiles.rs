//! DoD:S's explosives in flight on the install (the playtest's "the rifle
//! grenade flew as the gun itself"): every explosive's data names its
//! projectile's own model (`model`), a file of the pak that isn't the
//! weapon's world model (its script's `playermodel`), and the kit
//! `weapons::add_guns` loads holds it under the projectile's key
//! (`explosives::projectile_key`), a model unlike the weapon's.
//! Passes with a note without an install.

use chimera_world::map::GameKit;
use dods::game::{
    GAME, INFO, mount,
    weapons::{self, explosives, script_numbers::GunNumbers},
};
use source_engine::content::{material::MaterialLoader, surfaceprops::SurfaceProps};

#[test]
fn every_explosive_flies_as_its_projectile_not_its_weapon() {
    let path = INFO.install().filter(|p| chimera_core::mount::served::usable(GAME, p, "dod"));
    let Some(path) = path else {
        eprintln!("skipping: no DoD:S install");
        return;
    };
    let mount = mount::open(&path).expect("the install mounts");
    // The scripts' models, as a map's kit reads them.
    let set = GunNumbers::from_mount(Some(&mount)).parse().expect("the scripts parse");
    for id in weapons::EXPLOSIVES {
        let w = set.get(id).unwrap_or_else(|| panic!("{id}'s data"));
        let world = w.models.world.as_deref().unwrap_or_else(|| panic!("{id}'s world model"));
        let model = explosives::projectile_model(w).unwrap_or_else(|| panic!("{id} names no projectile model"));
        assert_ne!(model.to_ascii_lowercase(), world.to_ascii_lowercase(), "{id}: its projectile is its weapon's world model");
        assert!(mount.contains(model), "{id}: {model} isn't in the install");

        let mut materials = MaterialLoader::guest(&mount, 0, 0).lean(true);
        let surfaces = SurfaceProps::load(&mut materials);
        let (mut kit, mut warnings) = (GameKit::default(), Vec::new());
        weapons::add_guns(&mut materials, &surfaces, &|g| g == *id, &mut kit, &mut warnings);
        let key = explosives::projectile_key(id);
        let held = |k: &str| kit.held.iter().find(|h| h.key == k).unwrap_or_else(|| panic!("{id}: no held {k} ({warnings:?})"));
        let (gun, flyer) = (held(id), held(&key));
        assert!(!flyer.model.meshes.is_empty(), "{id}: {model} has no meshes");
        // A different model: its own meshes' size.
        let size = |m: &chimera_world::map::MapModel| {
            let (lo, hi) = m.meshes.iter().flat_map(|m| m.positions.iter()).fold((glam_max(), -glam_max()), |(lo, hi), p| {
                let p = bevy::math::Vec3::from(*p);
                (lo.min(p), hi.max(p))
            });
            hi - lo
        };
        let (a, b) = (size(&gun.model), size(&flyer.model));
        eprintln!("{id}: weapon {world} {a:?}, projectile {model} {b:?}");
        assert!((a - b).length() > 1e-3, "{id}: the projectile's model is the weapon's size ({a:?})");
    }
}

fn glam_max() -> bevy::math::Vec3 {
    bevy::math::Vec3::splat(f32::MAX)
}
