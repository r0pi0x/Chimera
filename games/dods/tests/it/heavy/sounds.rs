//! DoD:S's guns' sounds from the install (docs/plans/active/dods.md, M1
//! gaps): every entry of its weapon sound script (`maps::weapon_sounds`)
//! is named, the view models' draw among them, and on another game's map
//! (`maps::dress_guest_in`) they come under DoD:S's names only
//! (`maps::own_sound`), never shadowing that game's. Passes with a note
//! without the install.

use chimera_world::map::MapData;
use dods::game::{GAME, INFO, maps, mount};

#[test]
fn its_weapon_sounds_come_under_its_own_names_on_another_games_map() {
    let path = INFO.install().filter(|p| chimera_core::mount::served::usable(GAME, p, "dod"));
    let Some(path) = path else {
        eprintln!("skipping: no DoD:S install");
        return;
    };
    let m = mount::open(&path).expect("the install mounts");
    let names = maps::weapon_sounds(&m);
    assert!(names.len() > 100, "{} entries", names.len());
    assert!(names.iter().any(|n| n == "Weapon_Thompson.Draw"));
    let mut map = MapData::default();
    maps::dress_guest_in(&m, &mut map, &["dods:weapon_"], true);
    let entries = &map.sounds.entries;
    assert!(entries.contains_key("dods:weapon_thompson.draw"), "the draw under DoD:S's name");
    assert!(entries.contains_key("dods:weapon_garand.shoot"));
    assert!(!entries.contains_key("weapon_thompson.draw"), "no bare names on another game's map");
}
