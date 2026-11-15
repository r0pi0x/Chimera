//! The HUD tag readers on synthetic tags (specs/halo_ce/hud.md, test cases
//! S1 and S2): a weapon HUD interface with one number and one crosshair,
//! a unit HUD interface's shield meter, a grenade HUD, the HUD globals'
//! motion sensor numbers and a HUD number tag; the parent chain; the
//! class and damage refusals.

use blam_formats::cache::{Argb8, MapFile};
use blam_formats::hud::{
    Anchor, NumberElement, UnitHud,
    globals::{HudGlobals, HudNumber},
    grenade::GrenadeHud,
    weapon::{CrosshairItem, WeaponHud, chain},
};

use crate::map_fixture::{CacheFile, Struct, TagData, null_ref};

const SCNR: usize = 0;
const WPHI: usize = 1;
const PARENT: usize = 2;
const BITM: usize = 3;
const UNHI: usize = 4;
const GRHI: usize = 5;
const HUDG: usize = 6;
const NUMBER: usize = 7;

fn tags() -> Vec<(&'static str, &'static str)> {
    vec![
        ("scnr", "scenarios\\syn"),
        ("wphi", "ui\\hud\\test"),
        ("wphi", "ui\\hud\\master"),
        ("bitm", "ui\\hud\\bitmaps\\page"),
        ("unhi", "ui\\hud\\cyborg"),
        ("grhi", "ui\\hud\\default"),
        ("hudg", "ui\\hud\\default"),
        ("hud#", "ui\\hud\\counter"),
    ]
}

fn argb(v: u32) -> Argb8 {
    Argb8 { a: (v >> 24) as u8, r: (v >> 16) as u8, g: (v >> 8) as u8, b: v as u8 }
}

/// A colour block's 0x20 bytes at `off` of `s`.
fn colors(s: Struct, off: usize, plain: u32, flash: u32, period: f32, length: f32, disabled: u32) -> Struct {
    s.u32(off, plain)
        .u32(off + 4, flash)
        .f32(off + 8, period)
        .f32(off + 0x0C, 0.0)
        .i16(off + 0x10, 1)
        .f32(off + 0x14, length)
        .u32(off + 0x18, disabled)
}

/// A placement's offset and unit scale at `off`.
fn placement(s: Struct, off: usize, x: i16, y: i16) -> Struct {
    s.i16(off, x).i16(off + 2, y).f32s(off + 4, &[1.0, 1.0])
}

fn map() -> MapFile {
    let mut td = TagData::new(&tags());
    td.set_struct(SCNR, &Struct::new(0x5B0).0);
    td.set_struct(BITM, &Struct::new(0x6C).0);
    // S1's number element (0xA0).
    let mut number = Struct::new(0xA0).i16(0x00, 1).i16(0x04, 1);
    number = placement(number, 0x24, 29, 3);
    number = colors(number, 0x48, 0xFFE6F0FF, 0xFFFFE6E6, 1.0, 1.0, 0xFFFFE6E6);
    number = number.u8(0x68, 3).u8(0x69, 1).u8(0x6A, 0).u16(0x78, 1);
    // S1's crosshair item (0x6C) and element (0x68).
    let mut item = placement(Struct::new(0x6C), 0x00, 0, 0);
    item = colors(item, 0x24, 0x002896FF, 0x00FF0000, 1.0, 1.0, 0);
    item = item.i16(0x44, 0).i16(0x46, 3).u32(0x48, 1);
    let items = td.block(&[item.0]);
    let page = td.tag_ref(BITM);
    let crosshair = Struct::new(0x68).i16(0x00, 0).i16(0x04, 0).put(0x24, &page).put(0x34, &items);
    let numbers = td.block(&[number.0]);
    let crosshairs = td.block(&[crosshair.0]);
    let parent_ref = td.tag_ref(PARENT);
    let wphi = Struct::new(0x17C)
        .put(0x00, &parent_ref)
        .u16(0x10, 1)
        .i16(0x14, 5)
        .i16(0x16, 3)
        .i16(0x3C, 0)
        .put(0x78, &numbers)
        .put(0x84, &crosshairs)
        .u32(0x9C, 1);
    td.set_struct(WPHI, &wphi.0);
    td.set_struct(PARENT, &Struct::new(0x17C).put(0x00, &null_ref("wphi")).u32(0x9C, 0x200).0);
    // S2's unit HUD: anchor 1, the shield meter at 0x08C + 0x68.
    let m = 0xF4;
    let mut unhi = Struct::new(0x56C).i16(0x000, 1);
    unhi = placement(unhi, m, 7, 0)
        .put(m + 0x24, &page)
        .u32(m + 0x34, 0x0000509F)
        .u32(m + 0x38, 0x002896FF)
        .u32(m + 0x3C, 0x00FFFFFF)
        .u32(m + 0x40, 0)
        .u8(m + 0x44, 0)
        .u8(m + 0x45, 0)
        .i16(m + 0x46, 0)
        .u8(m + 0x48, 1)
        .u8(m + 0x49, 55)
        .i16(m + 0x4A, 198)
        .f32(m + 0x4C, 0.0)
        .f32(m + 0x50, 0.5);
    // The health panel's extras.
    unhi = unhi.u32(0x17C + 0xD0, 0x00FFDF00).f32(0x17C + 0xD4, 0.67).f32(0x17C + 0xD8, 0.33);
    unhi = placement(unhi, 0x35C, 41, 41);
    td.set_struct(UNHI, &unhi.0);
    // A grenade HUD: the plate at (82, 1), the count at (107, 3), one
    // overlay item of type 1.
    let mut overlay = placement(Struct::new(0x88), 0x00, 90, 4);
    overlay = colors(overlay, 0x24, 0xFFFFE6E6, 0, 0.0, 0.0, 0).i16(0x48, 2).i16(0x4A, 1);
    let overlays = td.block(&[overlay.0]);
    let mut grhi = placement(Struct::new(0x1F8), 0x024, 82, 1).put(0x024 + 0x24, &page).i16(0x024 + 0x54, 0);
    grhi = placement(grhi, 0x0F4, 107, 3).u8(0x0F4 + 0x44, 1).u8(0x0F4 + 0x45, 1);
    grhi = grhi.i16(0x148, 1).put(0x14C, &page).put(0x15C, &overlays);
    td.set_struct(GRHI, &grhi.0);
    // The HUD globals' motion sensor numbers and damage indicators.
    let hudg = Struct::new(0x450)
        .f32(0x2D0, 20.0)
        .f32(0x2D4, 0.0002)
        .f32(0x2D8, 32.0)
        .i16(0x310, 10)
        .i16(0x314, 10)
        .u32(0x34C, 0xC8FF0000)
        .f32(0x1BC, 0.75);
    td.set_struct(HUDG, &hudg.0);
    let number_tag = Struct::new(0x64).put(0x00, &page).u8(0x10, 12).u8(0x11, 9).u8(0x12, 1).u8(0x14, 6).u8(0x15, 6);
    td.set_struct(NUMBER, &number_tag.0);
    MapFile::from_image(CacheFile::new("syn", 1, td.finish()).image()).unwrap()
}

#[test]
fn s1_a_weapon_hud_with_one_number_and_one_crosshair() {
    let map = map();
    let hud = WeaponHud::read(&map, &map.tags()[WPHI]).unwrap();
    assert!(!hud.parent.is_null(), "the parent reference");
    let c = &hud.cutoffs;
    assert_eq!((c.flags, c.total_ammo, c.loaded_ammo, c.heat, c.age), (1, 5, 3, 0, 0));
    assert_eq!(Anchor::of(hud.anchor), Anchor::TopLeft);
    assert!(hud.statics.is_empty() && hud.meters.is_empty() && hud.overlays.is_empty());
    assert!(hud.sounds.is_empty() && hud.screen_effects.is_empty());
    assert_eq!(hud.numbers.len(), 1);
    let n = &hud.numbers[0];
    assert_eq!((n.state, n.map_type), (1, 1));
    assert_eq!(n.element.placement.offset, [29, 3]);
    assert_eq!(n.element.digits, 3);
    assert_eq!(n.element.flags & NumberElement::LEADING_ZEROS, 1);
    assert_eq!(n.element.flags & NumberElement::ONLY_ZOOMED, 0);
    assert_eq!(n.element.flags & NumberElement::METRES, 0);
    assert_eq!(n.element.fractional_digits, 0);
    assert_eq!(n.weapon_flags & 1, 1, "divide by clip size");
    assert_eq!(n.element.colors.color, argb(0xFFE6F0FF));
    assert_eq!(n.element.colors.flash_color, argb(0xFFFFE6E6));
    assert_eq!(hud.crosshairs.len(), 1);
    let x = &hud.crosshairs[0];
    assert_eq!((x.kind, x.map_type, x.items.len()), (0, 0, 1));
    let i = &x.items[0];
    assert_eq!(i.placement.offset, [0, 0]);
    assert_eq!(i.sequence, 3);
    assert!(i.flags & CrosshairItem::FLASHES != 0);
    assert!(i.flags & CrosshairItem::NOT_A_SPRITE == 0);
    assert_eq!(i.colors.color, argb(0x002896FF));
    assert_eq!(i.colors.flash_color, argb(0x00FF0000));
    assert_eq!(hud.valid_crosshairs, 1);
    // The chain: itself, then its parent.
    let chain = chain(&map, &map.tags()[WPHI]).unwrap();
    assert_eq!(chain.len(), 2);
    assert_eq!(chain[1].0.path, "ui\\hud\\master");
    assert_eq!(chain[1].1.valid_crosshairs, 0x200);
}

#[test]
fn s2_a_unit_huds_shield_meter() {
    let map = map();
    let hud = UnitHud::read(&map, &map.tags()[UNHI]).unwrap();
    assert_eq!(Anchor::of(hud.anchor), Anchor::TopRight);
    let m = &hud.shield.meter;
    assert_eq!(m.placement.offset, [7, 0]);
    assert_eq!(m.placement.scale, [1.0, 1.0]);
    assert_eq!((m.alpha_multiplier, m.alpha_bias, m.value_scale), (1, 55, 198));
    assert_eq!((m.fade, m.opacity), (0.5, 0.0));
    assert_eq!(m.min_color, argb(0x0000509F));
    assert_eq!(m.max_color, argb(0x002896FF));
    assert_eq!(m.flash_color, argb(0x00FFFFFF));
    let h = &hud.health;
    assert_eq!(h.medium_color, argb(0x00FFDF00));
    assert_eq!((h.max_cutoff, h.min_cutoff), (0.67, 0.33));
    assert_eq!(hud.blip_placement.offset, [41, 41]);
    assert!(hud.sounds.is_empty() && hud.aux_meters.is_empty());
}

#[test]
fn a_grenade_hud_the_hud_globals_and_the_number_tag() {
    let map = map();
    let g = GrenadeHud::read(&map, &map.tags()[GRHI]).unwrap();
    assert_eq!(g.background.placement.offset, [82, 1]);
    assert_eq!(g.count.placement.offset, [107, 3]);
    assert_eq!((g.count.digits, g.count.flags, g.flash_cutoff), (1, 1, 1));
    assert_eq!(g.overlays.len(), 1);
    assert_eq!((g.overlays[0].sequence, g.overlays[0].kind), (2, 1));
    let h = HudGlobals::read(&map, &map.tags()[HUDG]).unwrap();
    let d = &h.defaults;
    assert_eq!((d.sensor_range, d.sensor_velocity_sensitivity, d.sensor_scale), (20.0, 0.0002, 32.0));
    assert_eq!((h.damage.top, h.damage.left), (10, 10));
    assert_eq!(h.damage.color, argb(0xC8FF0000));
    assert_eq!(h.multiplayer_scale, 0.75);
    let n = HudNumber::read(&map, &map.tags()[NUMBER]).unwrap();
    assert_eq!((n.character_width, n.screen_width, n.x_offset, n.decimal_point_width), (12, 9, 1, 6));
}

#[test]
fn the_wrong_class_is_refused() {
    let map = map();
    assert!(WeaponHud::read(&map, &map.tags()[UNHI]).is_err());
    assert!(UnitHud::read(&map, &map.tags()[WPHI]).is_err());
    assert!(GrenadeHud::read(&map, &map.tags()[HUDG]).is_err());
    assert!(HudNumber::read(&map, &map.tags()[BITM]).is_err());
}

#[test]
fn a_damaged_block_is_refused() {
    let mut td = TagData::new(&tags());
    td.set_struct(SCNR, &Struct::new(0x5B0).0);
    // A crosshairs block of count 3 at an address past the tag data.
    let bad = Struct::new(0x17C).put(0x00, &null_ref("wphi")).i32(0x84, 3).u32(0x88, 0x8FFF_0000);
    td.set_struct(WPHI, &bad.0);
    for i in [PARENT, UNHI, GRHI, HUDG, NUMBER, BITM] {
        td.set_struct(i, &Struct::new(0x600).0);
    }
    let map = MapFile::from_image(CacheFile::new("syn", 1, td.finish()).image()).unwrap();
    let e = WeaponHud::read(&map, &map.tags()[WPHI]).unwrap_err();
    assert!(e.to_string().contains("ui\\hud\\test"), "{e}");
}
