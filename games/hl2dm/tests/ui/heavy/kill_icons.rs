//! Every way HL2:DM kills has its death notice's icon from the install
//! (specs/hl2dm/hud.md 10), in both feeds: HL2:DM's own HUD draws
//! `death_<name>` by the notice's name (`kill_notice_name`), and any app's
//! shared feed asks the registry (`WeaponIcons`, as `kill_icons::register`
//! fills it) and writes the gun's name only when it gets none. Each gun,
//! the launchers' and grenades' kills by their inflictors' names, a prop's
//! kill (`physics`); the kills the original draws with the skull (the
//! gravity gun's own punt, the world, a map's blast) have none. Skips
//! without an install (`$CHIMERA_HL2DM_ROOT`, Steam or `[games.hl2dm] path`).

use bevy::prelude::*;
use chimera_client::client::weapon_icons::{WeaponIcon, WeaponIcons};
use chimera_core::core::DamageKind;
use hl2dm::{
    game::{
        INFO, mount,
        physics::PHYSICS_WEAPON,
        rules::deaths::kill_notice_name,
        weapons::{AR2, CROSSBOW, CROWBAR, FRAG, MAGNUM, PHYSCANNON, PISTOL, RPG, SHOTGUN, SLAM, SMG1, STUNSTICK},
    },
    ui::kill_icons::{FONT, TEXTURES, death_glyphs, register_glyphs},
};

/// (weapon id, how it killed, the glyph hud.md 10 gives it).
const KILLS: [(&str, DamageKind, char); 18] = [
    (CROWBAR, DamageKind::Melee, '6'),
    (STUNSTICK, DamageKind::Melee, '!'),
    (PISTOL, DamageKind::Bullet, '-'),
    (MAGNUM, DamageKind::Bullet, '.'),
    (SMG1, DamageKind::Bullet, '/'),
    (SMG1, DamageKind::Blast, '7'),
    (AR2, DamageKind::Bullet, '2'),
    (AR2, DamageKind::Crush, '8'),
    (SHOTGUN, DamageKind::Bullet, '0'),
    (CROSSBOW, DamageKind::Bullet, '1'),
    (RPG, DamageKind::Blast, '3'),
    (FRAG, DamageKind::Blast, '4'),
    (FRAG, DamageKind::Crush, '4'),
    (SLAM, DamageKind::Blast, '*'),
    // A thrown, punted or exploding prop (gravity_gun.md, rules.md).
    (PHYSICS_WEAPON, DamageKind::Crush, '9'),
    (PHYSICS_WEAPON, DamageKind::BlastSurface, '9'),
    (PHYSICS_WEAPON, DamageKind::Blast, '9'),
    (PHYSICS_WEAPON, DamageKind::Generic, '9'),
];

/// The original's skull: no `death_` glyph for these.
const SKULLS: [(&str, DamageKind); 4] = [
    (PHYSCANNON, DamageKind::Crush),
    ("worldspawn", DamageKind::Generic),
    ("env_explosion", DamageKind::Blast),
    ("trigger_hurt", DamageKind::Generic),
];

#[test]
fn every_kill_has_its_icon_in_both_feeds() {
    let Some(mount) = INFO.install().and_then(|i| mount::open(&i).ok()) else {
        eprintln!("skipping: no HL2:DM install (Steam, [games.hl2dm] path, or $CHIMERA_HL2DM_ROOT)");
        return;
    };
    let text = mount.read(TEXTURES).expect("the death notices' glyphs");
    assert!(mount.read(FONT).is_ok_and(|f| !f.is_empty()), "their font");
    let glyphs = death_glyphs(&String::from_utf8_lossy(&text));
    let mut icons = WeaponIcons::default();
    register_glyphs(&mut icons, glyphs.clone(), Handle::default());
    let shared = |id: &str, kind| match icons.kill_icon(id, kind) {
        Some(WeaponIcon::Glyph { ch, .. }) => Some(ch),
        _ => None,
    };
    let mut wrong = vec![];
    for (id, kind, glyph) in KILLS {
        let name = kill_notice_name(id, kind);
        let own = glyphs.get(&name).copied();
        if own != Some(glyph) || shared(id, kind) != Some(glyph) {
            wrong.push(format!("{id} {kind:?}: notice `{name}`, HUD {own:?}, shared feed {:?}, want {glyph:?}", shared(id, kind)));
        }
    }
    assert!(wrong.is_empty(), "kills without their icon:\n{}", wrong.join("\n"));
    for (id, kind) in SKULLS {
        let name = kill_notice_name(id, kind);
        assert_eq!(glyphs.get(&name), None, "{id}: the original's skull");
        assert_eq!(shared(id, kind), None, "{id}: no icon of HL2:DM's");
    }
}
