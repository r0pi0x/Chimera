//! Quick info, the crosshair and the suit zoom (specs/hl2dm/hud.md 6; test
//! cases 35 to 44).

use bevy::prelude::*;
use hl2dm::ui::{frame::Draw, hud::Inputs};

use crate::common::*;

/// Test 35: health 30 to 25 starts the low-health warning and its sound
/// once.
#[test]
fn low_health_warns_once() {
    let (mut hud, _) = base();
    let mut inp: Inputs = inputs();
    inp.health = 30.0;
    hud.think(&inp, 0.0);
    inp.health = 25.0;
    hud.think(&inp, 0.1);
    assert!(hud.quick.warn_health);
    assert_eq!(hud.quick.sounds, [hl2dm::ui::quick::LOW_HEALTH_SOUND]);
    inp.health = 20.0;
    hud.think(&inp, 0.2);
    assert_eq!(hud.quick.sounds.len(), 1, "once");
    inp.health = 60.0;
    hud.think(&inp, 0.3);
    assert!(!hud.quick.warn_health, "a change above 25 clears it");
}

/// Tests 37 and 38: a pistol clip 18, 5, 4: the warning at 4, not at 5; a
/// crossbow's clip of one never warns.
#[test]
fn low_ammo_warns_from_a_quarter() {
    let (mut hud, _) = base();
    let mut inp = inputs();
    inp.weapon = Some(pistol(18, 100));
    hud.think(&inp, 0.0);
    inp.weapon = Some(pistol(5, 100));
    hud.think(&inp, 0.1);
    assert!(!hud.quick.warn_ammo);
    inp.weapon = Some(pistol(4, 100));
    hud.think(&inp, 0.2);
    assert!(hud.quick.warn_ammo);
    assert_eq!(hud.quick.sounds, [hl2dm::ui::quick::LOW_AMMO_SOUND]);
    let (mut hud, _) = base();
    let mut inp = inputs();
    let bolt = |clip| hl2dm::ui::hud::WeaponNow { class: "weapon_crossbow".into(), uses_primary: true, clip: Some(clip), max_clip: 1, reserve: 5, ..Default::default() };
    inp.weapon = Some(bolt(1));
    hud.think(&inp, 0.0);
    inp.weapon = Some(bolt(0));
    hud.think(&inp, 0.1);
    assert!(!hud.quick.warn_ammo);
}

/// Test 40: with nothing changing the panel starts dimming after a second
/// and is at 64 alpha by 3 s; a change brings it back.
#[test]
fn the_brackets_dim_when_idle() {
    let (mut hud, _) = base();
    let inp = inputs();
    for k in 0..=70 {
        hud.think(&inp, k as f64 * 0.05);
        if k == 20 {
            assert_eq!(hud.quick.alpha, 255.0, "still full at 1.0 s");
        }
    }
    hud.think(&inp, 3.1);
    assert_eq!(hud.quick.alpha, 64.0);
    let mut inp = inp;
    inp.weapon = Some(pistol(17, 150));
    hud.think(&inp, 3.2);
    hud.think(&inp, 3.7);
    assert_eq!(hud.quick.alpha, 255.0);
}

/// Test 39: health 60 and the SMG1 at 15 of 45: the left bracket's top 40
/// percent is the empty glyph, the right's top 66.7 percent.
#[test]
fn the_brackets_are_two_part_bars() {
    let (mut hud, look) = base();
    let mut inp = inputs();
    inp.health = 60.0;
    inp.weapon = Some(smg1(15, 0));
    hud.think(&inp, 0.0);
    let f = hud.frame(&look, &inp);
    let clips: Vec<(String, Rect)> = f
        .0
        .iter()
        .filter_map(|d| match d {
            Draw::Text { font: "QuickInfo", text, clip: Some(c), .. } => Some((text.clone(), *c)),
            _ => None,
        })
        .collect();
    let share = |glyph: &str| {
        let c = clips.iter().find(|(t, _)| t == glyph).map(|(_, c)| *c).unwrap_or_else(|| panic!("{glyph} in {clips:?}"));
        c.height() / look.tall("QuickInfo")
    };
    assert!((share("{") - 0.4).abs() < 0.04, "{}", share("{"));
    assert!((share("}") - 0.667).abs() < 0.04, "{}", share("}"));
}

/// Tests 41 and 42: FOV 90 draws the weapon's own crosshair, FOV 75 the
/// white default (the crossbow its own in white).
#[test]
fn the_default_field_of_view_draws_the_default_crosshair() {
    let (mut hud, mut look) = base();
    // A look with the pistol's glyph and the default sprite.
    let mut h = (*look.hud).clone();
    h.icons.insert("weapon_pistol/crosshair".into(), ("Crosshairs".into(), 'Q'));
    h.sprites.insert("crosshair_default".into(), chimera_world::map::hud::HudSprite { texture: 0, rect: [0.0, 48.0, 24.0, 24.0] });
    look.hud = std::sync::Arc::new(h);
    let mut inp = inputs();
    hud.think(&inp, 0.0);
    let cross = |inp: &Inputs, hud: &hl2dm::ui::hud::Hl2Hud| hud.frame(&look, inp);
    let f = cross(&inp, &hud);
    assert!(f.0.iter().any(|d| matches!(d, Draw::Text { font: "Crosshairs", color: [255, 208, 64, 255], .. })), "FOV 90: Q in Normal");
    inp.fov = 75.0;
    let f = cross(&inp, &hud);
    assert!(f.0.iter().any(|d| matches!(d, Draw::Sprite { sprite, color: [255, 255, 255, 255], rect, .. } if sprite == "crosshair_default" && rect.width() == 24.0)));
    assert!(!f.0.iter().any(|d| matches!(d, Draw::Text { font: "Crosshairs", .. })));
    // Dead: nothing.
    inp.alive = false;
    assert!(!cross(&inp, &hud).0.iter().any(|d| matches!(d, Draw::Sprite { .. })));
}

/// Tests 43 and 44: the zoom overlay's rings at 33 and 37 units and the
/// shade at half after 0.2 s; released, the rings run back and it ends.
#[test]
fn the_zoom_overlay_grows_and_shrinks() {
    let (mut hud, look) = base();
    let mut inp = inputs();
    hud.think(&inp, 10.0);
    inp.zoomed = true;
    hud.think(&inp, 11.0);
    hud.think(&inp, 11.2);
    let f = hud.frame(&look, &inp);
    let rings: Vec<f32> = f.0.iter().filter_map(|d| if let Draw::Ring { radius, .. } = d { Some(*radius) } else { None }).collect();
    assert_eq!(rings, [33.0, 37.0]);
    let shade = f.0.iter().find_map(|d| if let Draw::Sprite { sprite, color, .. } = d { (sprite == "zoom").then_some(color[3]) } else { None });
    assert_eq!(shade, Some(128), "alpha 0.5");
    inp.zoomed = false;
    hud.think(&inp, 12.0);
    hud.think(&inp, 12.2);
    let f = hud.frame(&look, &inp);
    let rings: Vec<f32> = f.0.iter().filter_map(|d| if let Draw::Ring { radius, .. } = d { Some(*radius) } else { None }).collect();
    assert_eq!(rings, [49.5, 55.5]);
    hud.think(&inp, 12.4);
    assert!(!hud.frame(&look, &inp).0.iter().any(|d| matches!(d, Draw::Ring { .. })), "nothing once it has run its 0.4 s");
}
