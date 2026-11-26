//! DoD:S's weapons from the install's scripts (specs/dods/weapons.md,
//! "The scripts' fields"): every weapon's script is served and sets its
//! numbers (`script_numbers::GunNumbers::from_mount`), the numbers equal
//! the committed fallback's (the spec's: the script's damage, bullets,
//! accuracy, fire delay rounded to ticks, clip and spare clips), each gun
//! gets its fire sound and models from its script, and the HL2 textures
//! the maps borrow are whole (the dedicated server's `hl2_textures` lacks
//! its archives: another install's stands in, `mount::hl2_donors`).
//! Each test passes with a note without an install.

use chimera_sim::weapon::data::WeaponData;
use dods::game::{GAME, INFO, mount, weapons, weapons::script_numbers::GunNumbers};

fn install_mount() -> Option<chimera_core::mount::Mount> {
    let path = INFO.install().filter(|p| chimera_core::mount::served::usable(GAME, p, "dod"));
    let Some(path) = path else {
        eprintln!("skipping: no DoD:S install");
        return None;
    };
    Some(mount::open(&path).expect("the install mounts"))
}

#[test]
fn every_weapon_reads_its_script_and_matches_the_spec() {
    let Some(m) = install_mount() else { return };
    let scripts = GunNumbers::from_mount(Some(&m));
    let missing: Vec<_> = scripts.sources.iter().filter(|s| !s.from_script).map(|s| s.source.clone()).collect();
    assert!(missing.is_empty(), "{missing:?}");
    let (read, data) = (scripts.parse().unwrap(), weapons::dods_weapons());
    let nums = |w: &WeaponData| {
        (
            w.damage.as_ref().map(|d| d.amount),
            w.hitscan.as_ref().map(|h| (h.pellets, format!("{:?}", h.spread))),
            w.trigger.as_ref().map(|t| t.cycle),
            w.magazine.as_ref().map(|m| m.clip),
            w.melee.as_ref().and_then(|m| m.primary.as_ref()).map(|p| (p.damage, p.hit_refire)),
            format!("{:?}", w.hooks.get(weapons::GUN_HOOK)),
        )
    };
    for id in weapons::GUNS.iter().chain(weapons::MELEE) {
        let (a, b) = (read.get(id).unwrap(), data.get(id).unwrap());
        assert_eq!(nums(a), nums(b), "{id}: the script's numbers against the spec's");
        assert!(a.models.view.is_some() && a.models.world.is_some(), "{id}'s models from its script");
        if weapons::GUNS.contains(id) {
            assert!(a.sounds.as_ref().is_some_and(|s| s.fire.is_some()), "{id}'s fire sound from its script");
        }
    }
}

#[test]
fn the_hl2_textures_the_maps_borrow_are_whole() {
    let Some(path) = INFO.install() else {
        eprintln!("skipping: no DoD:S install");
        return;
    };
    let sp = mount::search_path(&path).expect("the search path");
    let textures = sp.layers.iter().find(|l| l.path.ends_with("hl2_textures_dir.vpk")).expect("hl2_textures in the search path");
    let vpk = source_formats::vpk::Vpk::open(&textures.path).unwrap();
    if vpk.skipped() > 0 && !textures.path.starts_with(&path) {
        panic!("{} borrowed but not whole", textures.path.display());
    }
    if vpk.skipped() > 0 {
        eprintln!("skipping: no other install has hl2_textures whole ({})", textures.path.display());
        return;
    }
    assert!(!vpk.is_empty(), "{}", textures.path.display());
}

/// On the install: the numbers the app starts with, read back from the
/// converted cache (`numbers::from_scripts_kept`), are the numbers the
/// mount gives (`GunNumbers::from_mount`), on the miss that writes them
/// and on the hit that opens no archive.
#[test]
fn the_numbers_kept_between_starts_are_the_mounts() {
    use chimera_core::mount::assets::map_cache::{MapCache, Outcome};
    let Some(m) = install_mount() else { return };
    let direct = GunNumbers::from_mount(Some(&m));
    let dir = std::env::temp_dir().join(format!("chimera-dods-kept-numbers-heavy-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let cache = MapCache::at(&dir, 1 << 30);
    let (first, o1) = dods::game::numbers::from_scripts_kept(Some(&cache));
    let (second, o2) = dods::game::numbers::from_scripts_kept(Some(&cache));
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(first, direct, "the numbers on the miss");
    assert_eq!((o1, o2), (Outcome::Miss, Outcome::Hit));
    assert_eq!(second, direct, "the numbers read back");
}
