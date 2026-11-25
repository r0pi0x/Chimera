//! CS 1.6's HUD pictures from the user's install (`game::hud`,
//! specs/cs16/hud.md "Sprites"): every shared entry the HUD draws, every
//! weapon's `weapon`, `weapon_s` and `ammo` (hud.md's last test case;
//! the knife's too), a kill icon for every weapon but the smoke grenade
//! (which has none: the feed shows `d_skull`), the HE's `d_grenade` and
//! the AWP's at its size, and the pain sprite's four arcs. Passes with a
//! note without an install.

use cs16::game::{hud, weapons};

use super::maps::mount;

#[test]
fn every_weapons_hud_pictures_and_kill_icon_load() {
    let Some(mount) = mount() else { return };
    let p = hud::pictures(&mount, 640).expect("an install's files are all here");
    let mut problems = Vec::new();
    for name in hud::SHARED {
        if !p.shared.contains_key(*name) {
            problems.push(format!("shared {name}"));
        }
    }
    let size = |i: &chimera_world::map::hud::UiImage| (i.width, i.height);
    assert_eq!(p.shared.get("number_0").map(size), Some((20, 25)));
    assert_eq!(p.shared.get("cross").map(size), Some((24, 24)));
    // The round timer's and the weapon menu's pictures (hud.md, "Sprites").
    assert_eq!(p.shared.get("stopwatch").map(size), Some((24, 24)));
    assert_eq!(p.shared.get("selection").map(size), Some((170, 45)));
    for b in ["bucket0", "bucket1", "bucket2", "bucket3", "bucket4", "bucket5"] {
        assert_eq!(p.shared.get(b).map(size), Some((20, 20)), "{b}");
    }
    for w in &weapons::weapons().weapons {
        match p.weapons.get(&w.id) {
            Some(pics) => {
                for (what, pic) in [("weapon", &pics.weapon), ("weapon_s", &pics.weapon_s), ("ammo", &pics.ammo)] {
                    match pic {
                        Some(i) if what == "ammo" && size(i) != (24, 24) => problems.push(format!("{}: ammo {:?}", w.id, size(i))),
                        Some(i) if what != "ammo" && size(i) != (170, 45) => problems.push(format!("{}: {what} {:?}", w.id, size(i))),
                        Some(_) => {}
                        None => problems.push(format!("{}: no {what}", w.id)),
                    }
                }
            }
            None => problems.push(format!("{}: no list", w.id)),
        }
        let has_icon = p.kill_icons.contains_key(&w.id);
        if has_icon == (w.id == weapons::SMOKEGRENADE) {
            problems.push(format!("{}: kill icon {has_icon}", w.id));
        }
    }
    assert_eq!(p.kill_icons.get(weapons::AWP).map(size), Some((48, 16)));
    assert_eq!(p.kill_icons.get(weapons::HEGRENADE), p.shared.get("d_grenade"));
    assert_eq!(p.pain.iter().map(size).collect::<Vec<_>>(), vec![(128, 48), (48, 128), (128, 48), (48, 128)]);
    // Grey and lit somewhere: the cross isn't blank.
    let cross = &p.shared["cross"];
    assert!(cross.rgba8.chunks_exact(4).any(|px| px[0] > 100 && px[0] == px[1] && px[1] == px[2]));
    assert!(problems.is_empty(), "{problems:#?}");
}

/// The 320 layout's own pictures (hud.md: screens under 640 wide): the
/// digits' and icons' cells, each weapon's menu pictures and the pain arcs
/// from `320_pain.spr`.
#[test]
fn the_320_layout_has_its_own_pictures() {
    let Some(mount) = mount() else { return };
    let p = hud::pictures(&mount, 320).expect("an install's files are all here");
    let size = |i: &chimera_world::map::hud::UiImage| (i.width, i.height);
    assert_eq!(p.shared.get("number_0").map(size), Some((12, 16)));
    assert_eq!(p.shared.get("cross").map(size), Some((16, 16)));
    assert_eq!(p.shared.get("selection").map(size), Some((80, 20)));
    assert_eq!(p.shared.get("bucket1").map(size), Some((12, 12)));
    assert_eq!(p.weapons[weapons::AK47].weapon.as_ref().map(size), Some((80, 20)));
    assert_eq!(p.pain.len(), 4);
    assert_eq!(hud::resolution_for(639), 320);
}
