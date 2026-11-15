//! The HUD tag readers on the user's Halo disc image, against "The disc's
//! values" and test cases (b) of specs/halo_ce/hud.md: the tag counts of
//! the five classes on every multiplayer map, every tag of them read on
//! `bloodgulch` with its references resolving to the expected classes,
//! the assault rifle's and the plasma rifle's chains and element counts,
//! the sniper rifle's counts, the Spartan's unit HUD numbers, the grenade
//! HUDs, the HUD globals and the number tag. Each test passes with a note
//! when there is no image.

use blam_formats::cache::{Argb8, ClassCode, MapFile, TagRef, class};
use blam_formats::hud::{
    Anchor, UnitHud,
    globals::{HudGlobals, HudNumber},
    grenade::GrenadeHud,
    weapon::{WeaponHud, chain},
};

use super::maps::{MULTIPLAYER, load};

fn argb(v: u32) -> Argb8 {
    Argb8 { a: (v >> 24) as u8, r: (v >> 16) as u8, g: (v >> 8) as u8, b: v as u8 }
}

/// The path of a reference that must resolve to `want`.
fn path_of(map: &MapFile, r: &TagRef, want: &[ClassCode]) -> String {
    let t = map.tag(r.id).unwrap_or_else(|| panic!("a dangling reference {r:?}"));
    assert!(want.contains(&t.class), "{} is a {}", t.path, t.class);
    t.path.clone()
}

/// Every non-null reference of `r` is a `want`.
fn check(map: &MapFile, r: &TagRef, want: &[ClassCode]) {
    if !r.is_null() {
        path_of(map, r, want);
    }
}

fn wphi<'a>(map: &'a MapFile, path: &str) -> Vec<(&'a blam_formats::cache::TagEntry, WeaponHud)> {
    chain(map, map.find(class::WEAPON_HUD_INTERFACE, path).unwrap_or_else(|| panic!("no {path}"))).unwrap()
}

#[test]
fn every_multiplayer_map_has_the_hud_tags() {
    for name in MULTIPLAYER {
        let Some(map) = load(name) else {
            eprintln!("no Halo disc image: skipped");
            return;
        };
        let count = |c| map.of_class(c).count();
        assert_eq!(
            [
                count(class::UNIT_HUD_INTERFACE),
                count(class::WEAPON_HUD_INTERFACE),
                count(class::GRENADE_HUD_INTERFACE),
                count(class::HUD_GLOBALS),
                count(class::HUD_NUMBER),
                count(class::HUD_MESSAGE_TEXT),
                count(class::UNICODE_STRING_LIST),
            ],
            [5, 17, 2, 1, 1, 1, 8],
            "{name}"
        );
    }
}

#[test]
fn every_hud_tag_of_bloodgulch_reads_and_its_references_resolve() {
    let Some(map) = load("bloodgulch") else {
        eprintln!("no Halo disc image: skipped");
        return;
    };
    let bitm = [class::BITMAP];
    let sounds = [class::SOUND, class::SOUND_LOOPING];
    let (mut statics, mut meters, mut numbers, mut crosshairs, mut items, mut overlays, mut effects) = (0, 0, 0, 0, 0, 0, 0);
    for t in map.of_class(class::WEAPON_HUD_INTERFACE) {
        let h = WeaponHud::read(&map, t).unwrap_or_else(|e| panic!("{}: {e}", t.path));
        check(&map, &h.parent, &[class::WEAPON_HUD_INTERFACE]);
        chain(&map, t).unwrap();
        for s in &h.statics {
            check(&map, &s.element.bitmap, &bitm);
        }
        for m in &h.meters {
            check(&map, &m.element.bitmap, &bitm);
        }
        for c in &h.crosshairs {
            check(&map, &c.bitmap, &bitm);
            items += c.items.len();
        }
        for o in &h.overlays {
            check(&map, &o.bitmap, &bitm);
        }
        for s in &h.sounds {
            check(&map, &s.sound, &sounds);
        }
        statics += h.statics.len();
        meters += h.meters.len();
        numbers += h.numbers.len();
        crosshairs += h.crosshairs.len();
        overlays += h.overlays.iter().map(|o| o.items.len()).sum::<usize>();
        effects += h.screen_effects.len();
    }
    assert_eq!((statics, meters, numbers, crosshairs, items, overlays, effects), (15, 8, 6, 31, 39, 3, 3));
    for t in map.of_class(class::UNIT_HUD_INTERFACE) {
        let h = UnitHud::read(&map, t).unwrap_or_else(|e| panic!("{}: {e}", t.path));
        for r in [&h.background.bitmap, &h.shield.background.bitmap, &h.shield.meter.bitmap, &h.health.meter.bitmap] {
            check(&map, r, &bitm);
        }
        for s in &h.sounds {
            check(&map, &s.sound, &sounds);
        }
    }
    let mut grenade_items = 0;
    for t in map.of_class(class::GRENADE_HUD_INTERFACE) {
        let h = GrenadeHud::read(&map, t).unwrap();
        check(&map, &h.background.bitmap, &bitm);
        check(&map, &h.overlay_bitmap, &bitm);
        grenade_items += h.overlays.len();
    }
    assert_eq!(grenade_items, 4);
    let g = HudGlobals::read(&map, map.of_class(class::HUD_GLOBALS).next().unwrap()).unwrap();
    check(&map, &g.messaging.font, &[class::FONT]);
    check(&map, &g.messaging.item_messages, &[class::UNICODE_STRING_LIST]);
    check(&map, &g.messaging.hud_messages, &[class::HUD_MESSAGE_TEXT]);
    check(&map, &g.waypoints.bitmap, &bitm);
    check(&map, &g.damage.bitmap, &bitm);
    assert_eq!(path_of(&map, &g.defaults.weapon_hud, &[class::WEAPON_HUD_INTERFACE]), "ui\\hud\\empty");
}

#[test]
fn the_assault_rifles_and_the_plasma_rifles_chains() {
    let Some(map) = load("bloodgulch") else {
        eprintln!("no Halo disc image: skipped");
        return;
    };
    let ar = wphi(&map, "weapons\\assault rifle\\assault rifle");
    let paths: Vec<&str> = ar.iter().map(|(t, _)| t.path.as_str()).collect();
    assert_eq!(paths, ["weapons\\assault rifle\\assault rifle", "ui\\hud\\master rounds", "ui\\hud\\master"]);
    let leaf = &ar[0].1;
    assert_eq!(
        (leaf.statics.len(), leaf.meters.len(), leaf.numbers.len(), leaf.crosshairs.len(), leaf.overlays.len(), leaf.screen_effects.len()),
        (1, 1, 0, 1, 0, 0)
    );
    assert_eq!(leaf.statics[0].element.sequence, 1);
    assert_eq!(path_of(&map, &leaf.statics[0].element.bitmap, &[class::BITMAP]), "ui\\hud\\bitmaps\\combined\\hud_ammo_alphas");
    assert_eq!(leaf.meters[0].element.sequence, 1);
    assert_eq!(path_of(&map, &leaf.meters[0].element.bitmap, &[class::BITMAP]), "ui\\hud\\bitmaps\\combined\\hud_ammo_meters");
    let x = &leaf.crosshairs[0];
    assert_eq!((x.kind, x.items.len(), x.items[0].sequence, x.items[0].flags), (0, 1, 0, 1));
    assert_eq!(path_of(&map, &x.bitmap, &[class::BITMAP]), "ui\\hud\\bitmaps\\combined\\hud_reticles");
    assert_eq!((leaf.cutoffs.total_ammo, leaf.cutoffs.loaded_ammo), (61, 10));
    assert_eq!(leaf.valid_crosshairs, 1);
    assert_eq!(leaf.messaging_icon.sequence, 8);
    let rounds = &ar[1].1;
    assert_eq!((rounds.statics.len(), rounds.meters.len(), rounds.numbers.len(), rounds.crosshairs.len(), rounds.overlays.len()), (2, 0, 3, 3, 2));
    assert_eq!(rounds.crosshairs.iter().map(|c| c.kind).collect::<Vec<_>>(), [8, 3, 10]);
    assert_eq!((rounds.valid_crosshairs, rounds.cutoffs.flags), (0x508, 1));
    assert_eq!(rounds.statics[0].element.placement.offset, [12, 1]);
    assert_eq!(rounds.numbers[0].element.placement.offset, [29, 3]);
    let master = &ar[2].1;
    assert_eq!((master.crosshairs.len(), master.crosshairs[0].kind, master.valid_crosshairs), (1, 9, 0x200));

    let pr = wphi(&map, "weapons\\plasma rifle\\plasma rifle");
    let paths: Vec<&str> = pr.iter().map(|(t, _)| t.path.as_str()).collect();
    assert_eq!(paths, ["weapons\\plasma rifle\\plasma rifle", "ui\\hud\\master plasma", "ui\\hud\\master"]);
    let leaf = &pr[0].1;
    assert_eq!((leaf.statics.len(), leaf.meters.len(), leaf.numbers.len(), leaf.crosshairs.len()), (0, 0, 0, 2));
    assert_eq!(leaf.crosshairs.iter().map(|c| c.kind).collect::<Vec<_>>(), [0, 6]);
    assert_eq!(leaf.crosshairs[0].items[0].sequence, 4);
    let c = &leaf.cutoffs;
    assert_eq!((c.total_ammo, c.loaded_ammo, c.heat, c.age), (0, 0, 75, 10));
    let plasma = &pr[1].1;
    assert_eq!((plasma.statics.len(), plasma.meters.len(), plasma.numbers.len(), plasma.crosshairs.len(), plasma.overlays.len()), (5, 2, 1, 1, 1));
    assert_eq!(plasma.valid_crosshairs, 0x40000);
    assert_eq!(plasma.meters[0].element.placement.offset, [4, 21]);
    assert_eq!(plasma.meters[0].element.max_color, argb(0x00FF0000));

    let sniper = wphi(&map, "weapons\\sniper rifle\\sniper rifle");
    let leaf = &sniper[0].1;
    assert_eq!((leaf.statics.len(), leaf.meters.len(), leaf.numbers.len(), leaf.crosshairs.len(), leaf.screen_effects.len()), (3, 1, 2, 6, 1));
    // The pistol's zoom label and scope mask.
    let pistol = wphi(&map, "weapons\\pistol\\pistol");
    let zoom = pistol[0].1.crosshairs.iter().find(|c| c.kind == 1 && c.map_type == 1).expect("the one-window zoom label");
    assert_eq!(zoom.items[0].placement.offset, [105, 105]);
    assert_eq!(pistol[0].1.screen_effects.len(), 1);
}

#[test]
fn the_spartans_unit_hud_the_grenades_the_globals_and_the_digits() {
    let Some(map) = load("bloodgulch") else {
        eprintln!("no Halo disc image: skipped");
        return;
    };
    let h = UnitHud::read(&map, map.find(class::UNIT_HUD_INTERFACE, "ui\\hud\\cyborg").unwrap()).unwrap();
    assert_eq!(Anchor::of(h.anchor), Anchor::TopRight);
    assert!(h.background.bitmap.is_null());
    let m = &h.shield.meter;
    assert_eq!((m.placement.offset, m.sequence, m.alpha_multiplier, m.alpha_bias, m.value_scale, m.fade), ([7, 0], 0, 1, 55, 198, 0.5));
    assert_eq!((m.min_color, m.max_color), (argb(0x0000509F), argb(0x002896FF)));
    assert_eq!(path_of(&map, &m.bitmap, &[class::BITMAP]), "ui\\hud\\bitmaps\\combined\\hud_unit_meters");
    assert_eq!(h.shield.background.placement.offset, [0, 1]);
    assert_eq!(h.shield.background.colors.color, argb(0xDCAAD5FF));
    let health = &h.health;
    assert_eq!((health.meter.placement.offset, health.meter.flags, health.meter.minimum, health.meter.alpha_multiplier, health.meter.alpha_bias), ([36, 17], 1, 31, 30, 15));
    assert_eq!(health.medium_color, argb(0x00FFDF00));
    assert_eq!((health.max_cutoff, health.min_cutoff), (0.67, 0.33));
    assert_eq!(health.background.placement.offset, [-5, 18]);
    assert_eq!(h.sensor.background.sequence, 3);
    assert_eq!(h.sensor.foreground.sequence, 6);
    assert_eq!(h.blip_placement.offset, [41, 41]);
    assert_eq!(h.sounds.len(), 5);
    assert_eq!(h.aux_meters.len(), 1);
    assert_eq!(h.aux_meters[0].panel.aux_cutoff(), 0.25);

    for (path, icon, at) in [("ui\\hud\\default", 2, [90, 4]), ("ui\\hud\\plasma_flare", 3, [88, 2])] {
        let g = GrenadeHud::read(&map, map.find(class::GRENADE_HUD_INTERFACE, path).unwrap()).unwrap();
        assert_eq!(g.background.placement.offset, [82, 1]);
        assert_eq!(g.count.placement.offset, [107, 3]);
        assert_eq!((g.count.digits, g.flash_cutoff), (1, 1));
        let item = g.overlays.iter().find(|o| o.kind == 4).unwrap();
        assert_eq!((item.sequence, item.placement.offset), (icon, at), "{path}");
    }

    let g = HudGlobals::read(&map, map.of_class(class::HUD_GLOBALS).next().unwrap()).unwrap();
    let d = &g.defaults;
    assert_eq!((d.sensor_range, d.sensor_velocity_sensitivity, d.sensor_scale), (20.0, 0.0002, 32.0));
    assert_eq!(g.messaging.button_icons.len(), 18);
    assert_eq!(g.waypoints.arrows.len(), 10);
    assert_eq!([g.damage.top, g.damage.bottom, g.damage.left, g.damage.right], [10; 4]);
    assert_eq!(g.damage.color, argb(0xC8FF0000));
    assert_eq!((g.messaging.up_time, g.messaging.fade_time, g.messaging.spacing), (2.0, 2.0, 1.35));

    let n = HudNumber::read(&map, map.of_class(class::HUD_NUMBER).next().unwrap()).unwrap();
    assert_eq!((n.character_width, n.screen_width, n.decimal_point_width, n.colon_width), (12, 9, 6, 6));
    assert_eq!(path_of(&map, &n.bitmap, &[class::BITMAP]), "ui\\hud\\bitmaps\\combined\\hud_counter_numbers");
}
