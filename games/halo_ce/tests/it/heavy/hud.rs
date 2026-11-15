//! Halo's HUD from the user's disc image (specs/halo_ce/hud.md, test
//! cases (b) and "The disc's values"): the HUD read from `bloodgulch`
//! (`ui::hud::data::HudData`) and drawn for one window at 640 by 480
//! (`ui::hud::draw`): the assault rifle's and the plasma rifle's elements
//! (pages, rectangles, the battery digits), the pistol's zoom label, the
//! Spartan's shield, health and tracker rectangles, the grenade plate, the
//! shield meter's stages through a hit; and the same HUD on a 16:9
//! canvas. Passes with a note without an install.

use blam_formats::cache::MapFile;
use halo_ce::ui::hud::{
    data::HudData,
    draw::{self, Fill, HudState, Held, Quad, Unit},
    layout::Canvas,
    nodes::weapon_now,
    rules::WeaponNow,
};

fn data() -> Option<HudData> {
    let mount = super::maps::mount()?;
    let bytes = mount.read("maps/bloodgulch.map").unwrap();
    Some(HudData::load(&MapFile::load(&bytes).unwrap()).unwrap())
}

fn page<'a>(d: &'a HudData, q: &Quad) -> &'a str {
    d.pages[&q.picture.page].path.rsplit('\\').next().unwrap()
}

fn rect(q: &Quad) -> [i32; 4] {
    q.rect.map(|v| v as i32)
}

/// The quads of `page`'s pictures, as rectangles.
fn on(d: &HudData, quads: &[Quad], name: &str) -> Vec<[i32; 4]> {
    quads.iter().filter(|q| page(d, q) == name).map(rect).collect()
}

fn held(tag: &str, now: WeaponNow) -> Unit {
    Unit { alive: true, shield: 1.0, body: 1.0, weapon: Some(Held { tag: tag.into(), now }), ..Default::default() }
}

/// Ticks of state, then this frame's quads, at 640 by 480.
fn frame(d: &HudData, unit: &Unit, ticks: i64) -> Vec<Quad> {
    let mut s = HudState::default();
    for t in 100..100 + ticks {
        s.tick(d, unit, &[], 0.0, t);
    }
    draw::draw(d, unit, &s, &Canvas::ORIGINAL, 100 + ticks)
}

#[test]
fn the_assault_rifles_hud() {
    let Some(d) = data() else { return };
    let ar = WeaponNow { loaded: 60, reserve: 180, loaded_max: 60, ready: true, magazine: true, zoom: -1, ..Default::default() };
    let q = frame(&d, &held("weapons\\assault rifle\\assault rifle", ar), 5);
    assert_eq!(on(&d, &q, "hud_reticles"), [[287, 207, 353, 273]], "the reticle");
    assert!(on(&d, &q, "hud_ammo_alphas").contains(&[48, 56, 176, 88]), "the bar's plate");
    assert!(on(&d, &q, "hud_ammo_meters").contains(&[47, 56, 161, 83]), "the bar");
    assert!(on(&d, &q, "hud_weapon_backgrounds").contains(&[60, 37, 188, 69]), "the plate");
    assert!(on(&d, &q, "hud_ammo_outlines").contains(&[60, 37, 188, 69]), "the outline");
    assert_eq!(on(&d, &q, "hud_ammo_type_icons").first(), Some(&[101, 40, 133, 56]), "the bullet icon");
    // The reserve, 180, in three digits at x 68, 77, 86 (left to right).
    let digits: Vec<i32> = on(&d, &q, "hud_counter_numbers").iter().filter(|r| r[1] == 39).map(|r| r[0]).collect();
    assert!(digits.starts_with(&[86, 77, 68]), "{digits:?}");
    let bar = q.iter().find(|q| page(&d, q) == "hud_ammo_meters").unwrap();
    let Fill::Meter(m) = bar.fill else { panic!("a meter") };
    assert!((m.value - 242.0 / 255.0).abs() < 1e-6, "60 rounds: alpha 242");
    // No warnings with a full magazine; "No Ammo" when empty and firing.
    assert!(on(&d, &q, "hud_reticle_warnings").is_empty());
    let empty = WeaponNow { firing: true, ..ar_empty() };
    let q = frame(&d, &held("weapons\\assault rifle\\assault rifle", empty), 3);
    assert!(on(&d, &q, "hud_reticle_warnings").contains(&[269, 258, 311, 282]), "\"No Ammo\"");
    // The Spartan's unit HUD.
    assert_eq!(on(&d, &q, "hud_unit_meters"), [[463, 36, 585, 67], [468, 53, 556, 67]], "the shield and health bars");
    assert!(on(&d, &q, "hud_unit_backgrounds").contains(&[464, 37, 592, 69]), "the shield plate");
    assert!(on(&d, &q, "hud_unit_backgrounds").contains(&[469, 54, 597, 70]), "the health plate");
    assert!(on(&d, &q, "hud_weapon_backgrounds").contains(&[48, 316, 176, 444]), "the sensor's disc");
    assert!(on(&d, &q, "hud_unit_backgrounds").contains(&[48, 316, 176, 444]), "the sensor's cone");
    assert!(on(&d, &q, "hud_weapon_backgrounds").contains(&[130, 37, 258, 69]), "the grenade plate");
    assert!(on(&d, &q, "hud_ammo_type_icons").contains(&[138, 40, 170, 56]), "the frag icon");
    assert!(on(&d, &q, "hud_counter_numbers").contains(&[146, 39, 164, 51]), "the grenade count");
}

fn ar_empty() -> WeaponNow {
    WeaponNow { loaded: 0, reserve: 0, loaded_max: 60, ready: true, magazine: true, zoom: -1, ..Default::default() }
}

#[test]
fn the_plasma_rifles_hud() {
    let Some(d) = data() else { return };
    let pr = WeaponNow { heat: 0.5, age: 0.25, ready: true, zoom: -1, ..Default::default() };
    let q = frame(&d, &held("weapons\\plasma rifle\\plasma rifle", pr), 3);
    assert_eq!(on(&d, &q, "hud_reticles"), [[288, 208, 352, 272]], "the reticle");
    assert_eq!(on(&d, &q, "hud_ammo_meters"), [[52, 57, 156, 73]], "the heat bar");
    let digits: Vec<i32> = on(&d, &q, "hud_counter_numbers").iter().filter(|r| r[1] == 39 && r[0] < 100).map(|r| r[0]).collect();
    assert_eq!(digits, [83, 74, 65], "the battery's 75 percent as 075");
    // "Low Battery" at 10 percent or less.
    let low = WeaponNow { age: 0.95, ..pr };
    let q = frame(&d, &held("weapons\\plasma rifle\\plasma rifle", low), 3);
    assert!(on(&d, &q, "hud_reticle_warnings").contains(&[246, 251, 294, 277]), "\"Low Battery\": {:?}", on(&d, &q, "hud_reticle_warnings"));
}

#[test]
fn the_pistols_zoom_label() {
    let Some(d) = data() else { return };
    let pistol = WeaponNow { loaded: 12, reserve: 36, loaded_max: 12, ready: true, magazine: true, zoom: -1, ..Default::default() };
    let tag = "weapons\\pistol\\pistol";
    let q = frame(&d, &held(tag, pistol), 3);
    assert_eq!(on(&d, &q, "hud_reticles"), [[306, 226, 334, 254]]);
    assert!(on(&d, &q, "hud_reticles_scope").is_empty(), "no label unzoomed");
    let q = frame(&d, &held(tag, WeaponNow { zoom: 0, ..pistol }), 3);
    assert_eq!(on(&d, &q, "hud_reticles_scope"), [[410, 333, 440, 357]], "\"2x\"");
}

#[test]
fn a_hit_marks_the_lost_shield_and_a_wide_canvas_moves_the_corners() {
    let Some(d) = data() else { return };
    let mut unit = held("weapons\\assault rifle\\assault rifle", ar_empty());
    let mut s = HudState::default();
    s.tick(&d, &unit, &[], 0.0, 199);
    unit.shield = 0.6;
    s.tick(&d, &unit, &[], 0.0, 200);
    let shield = |q: &[Quad]| match q.iter().find(|q| page(&d, q) == "hud_unit_meters").unwrap().fill {
        Fill::Meter(m) => (m.value, m.extension),
        _ => panic!(),
    };
    let q = draw::draw(&d, &unit, &s, &Canvas::ORIGINAL, 200);
    assert_eq!(shield(&q), (173.0 / 255.0, 253.0 / 255.0), "the lost part marked");
    for t in 201..=216 {
        s.tick(&d, &unit, &[], 0.0, t);
    }
    let q = draw::draw(&d, &unit, &s, &Canvas::ORIGINAL, 216);
    assert_eq!(shield(&q), (173.0 / 255.0, 173.0 / 255.0), "snapped after 15 ticks");
    // 16:9: the shield bar 7 units in from the right safe edge, the sensor
    // at the left one, the reticle in the middle.
    let wide = Canvas::of(1920.0, 1080.0);
    let q = draw::draw(&d, &unit, &s, &wide, 216);
    assert_eq!(on(&d, &q, "hud_unit_meters")[0], [wide.width - 48 - 7 - 122, 36, wide.width - 55, 67]);
    assert!(on(&d, &q, "hud_weapon_backgrounds").contains(&[48, 316, 176, 444]));
    let r = on(&d, &q, "hud_reticles")[0];
    assert!((r[0] + r[2] - wide.width).abs() <= 1, "centred: {r:?}");
    // The weapon's interface state from the simulation's own.
    let gun = halo_ce::game::weapons::guns().into_iter().find(|(id, _)| id == halo_ce::game::weapons::ASSAULT_RIFLE).unwrap().1;
    let state = halo_ce::game::weapons::HaloGun::new(&gun);
    let w = weapon_now(&state, &halo_ce::game::weapons::HaloWeapon(std::sync::Arc::new(gun.clone())), false);
    assert_eq!((w.loaded, w.reserve, w.loaded_max), (60, 180, 60));
}

