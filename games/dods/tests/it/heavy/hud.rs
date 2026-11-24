//! DoD:S's HUD pictures from the install (specs/dods/hud.md): every
//! weapon's kill icon (test case 1), its current-weapon and magazine
//! pictures where its script names them, the Thompson's clip fields (test
//! case 2's), and the twelve classes' silhouettes; and the layout the HUD
//! draws (`dods_ui::hud::layout`, hud.md's numbers) is the install's
//! (`resource/ui/hudplayerstatuspanel.res`). Passes with a note without an
//! install.

use chimera_core::mount::keyvalues::{self, Escapes};
use dods::game::{GAME, INFO, hud, mount, weapons};

fn install_mount() -> Option<chimera_core::mount::Mount> {
    let path = INFO.install().filter(|p| chimera_core::mount::served::usable(GAME, p, "dod"));
    let Some(path) = path else {
        eprintln!("skipping: no DoD:S install");
        return None;
    };
    Some(mount::open(&path).expect("the install mounts"))
}

#[test]
fn every_weapon_and_class_has_its_hud_pictures() {
    let Some(m) = install_mount() else { return };
    let p = hud::pictures(&m).expect("an install's files are all here");
    let all: Vec<&str> = weapons::GUNS.iter().chain(weapons::MELEE).chain(weapons::EXPLOSIVES).copied().collect();
    assert_eq!(all.len(), 22);
    for id in &all {
        let icon = p.kill_icons.get(*id).unwrap_or_else(|| panic!("{id}: no kill icon"));
        assert!(icon.width > icon.height && icon.rgba8.len() == (icon.width * icon.height * 4) as usize, "{id}");
        let w = p.weapons.get(*id).unwrap_or_else(|| panic!("{id}: no pictures"));
        assert!(w.weapon.is_some(), "{id}: weapon_s");
        if !weapons::MELEE.contains(id) {
            assert!(w.ammo.is_some(), "{id}: ammo");
        }
        // The grenades' clip_size is -1: their count is their rounds.
        assert_eq!(w.clip_size > 0, weapons::GUNS.contains(id) || id.contains("riflegren") || id.contains("bazooka") || id.contains("pschreck"), "{id}: clip_size");
    }
    let t = &p.weapons[weapons::THOMPSON];
    assert_eq!(t.clip, Some(hud::ClipFill { height: 184.0, base: 2.0, bullet: 6.0 }));
    assert_eq!(t.clip_size, 30);
    assert_eq!(t.ammo.as_ref().map(|a| (a.width, a.height)), Some((36, 184)));
    assert_eq!(p.classes.len(), 12, "{:?}", p.classes.keys().collect::<Vec<_>>());
    assert!(p.classes.values().all(|c| c.image.is_some() && c.outline.is_some()));
}

#[test]
fn the_status_strip_layout_is_the_installs() {
    let Some(m) = install_mount() else { return };
    let text = String::from_utf8_lossy(&m.read("resource/ui/hudplayerstatuspanel.res").unwrap()).to_string();
    let kv = keyvalues::parse(&text, Escapes::Off);
    let root = kv.items().first().map(|(_, v)| v.clone()).unwrap();
    let at = |name: &str| -> [f32; 4] {
        let p = root.get(name).unwrap_or_else(|| panic!("{name}"));
        let n = |k: &str| p.str(k).unwrap().trim().parse::<f32>().unwrap();
        [n("xpos"), n("ypos"), n("wide"), n("tall")]
    };
    // hud.md's table (the strip at y 290).
    assert_eq!(at("PlayerStatusMainBackground"), [16.0, 128.0, 240.0, 32.0]);
    assert_eq!(at("PlayerStatusHealth"), [42.0, 91.0, 46.0, 78.0]);
    assert_eq!(at("PlayerStatusCurrentWeapon"), [90.0, 130.0, 67.0, 27.0]);
    assert_eq!(at("PlayerStatusAmmo"), [168.0, 115.0, 84.0, 52.0]);
    assert_eq!(at("PlayerStatusStamina"), [10.0, 161.0, 266.0, 32.0]);
    assert_eq!(at("PlayerStatusLeftLine"), [88.0, 131.0, 1.0, 26.0]);
}
