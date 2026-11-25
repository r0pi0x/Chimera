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
    let p = hud::pictures(&mount).expect("an install's files are all here");
    let mut problems = Vec::new();
    for name in hud::SHARED {
        if !p.shared.contains_key(*name) {
            problems.push(format!("shared {name}"));
        }
    }
    let size = |i: &chimera_world::map::hud::UiImage| (i.width, i.height);
    assert_eq!(p.shared.get("number_0").map(size), Some((20, 25)));
    assert_eq!(p.shared.get("cross").map(size), Some((24, 24)));
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
