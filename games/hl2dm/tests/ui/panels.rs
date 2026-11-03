//! The numeric panels, aux power and the team line (specs/hl2dm/hud.md
//! 4, 7; test cases 6, 8-23, 67, 68), on the stand-in look.

use bevy::prelude::*;
use hl2dm::ui::{
    frame::{Draw, Screen},
    hud::Inputs,
    quick::blink_alpha,
};

use crate::common::*;

fn text_at(f: &hl2dm::ui::frame::Frame, text: &str) -> Vec2 {
    match f.find_text(text) {
        Some(Draw::Text { at, .. }) => *at,
        other => panic!("{text}: {other:?}"),
    }
}

/// Test 8: health 100 to 80 starts the "increased above 20" event, not a
/// "decreased" one; a damage message of 20 starts the damage flash too.
#[test]
fn health_changes_start_events_even_when_it_fell() {
    let (mut hud, look) = base();
    let mut inp = inputs();
    hud.think(&inp, 0.0);
    hud.events.clear();
    inp.health = 80.0;
    hud.think(&inp, 0.1);
    assert_eq!(hud.fired(0.0), ["HealthIncreasedAbove20"]);
    hud.damage_message(&hl2dm::ui::damage::DamageMsg { health: 20.0, angle: Some(90.0), ..Default::default() }, 80.0);
    assert!(hud.fired(0.0).contains(&"HealthDamageTaken"));
    let _ = look;
}

/// Test 9: health 100 to 15 starts both low events; the pulse runs at
/// 1.0, 1.8 and 2.6 s (its blur peaks 0.1 s after each start); the digits
/// end in `DamagedFg`.
#[test]
fn low_health_pulses_every_0_8_seconds() {
    let (mut hud, _) = base();
    let mut inp = inputs();
    hud.think(&inp, 0.0);
    hud.events.clear();
    inp.health = 15.0;
    hud.think(&inp, 10.0);
    assert_eq!(hud.fired(0.0), ["HealthIncreasedBelow20", "HealthLow"]);
    let mut blur = Vec::new();
    for t in 1..=60 {
        let at = t as f64 * 0.05;
        hud.think(&inp, 10.0 + at);
        blur.push((at, hud.anim.number("HudHealth", "Blur").unwrap_or(0.0)));
    }
    let peak = |from: f64, to: f64| blur.iter().filter(|(t, _)| *t >= from && *t <= to).map(|(_, b)| *b).fold(0.0, f32::max);
    for (from, to) in [(1.05, 1.25), (1.85, 2.05), (2.65, 2.85)] {
        assert!(peak(from, to) > 4.9, "the pulse peaks in {from}..{to}: {}", peak(from, to));
    }
    assert!(peak(1.45, 1.55) < 4.5 && peak(2.25, 2.35) < 4.5, "and settles between");
    // Left alone, the digits settle on the damaged colour.
    for t in 61..200 {
        hud.think(&inp, 10.0 + t as f64 * 0.05);
    }
    assert_eq!(hud.anim.rgba("HudHealth", "FgColor"), Some([180, 0, 0, 230]));
}

/// Tests 10 and 11: health back to 25 stops the pulse and resets the box
/// at once; health 0 starts no event and the panel is gone.
#[test]
fn recovering_stops_the_pulse_and_death_is_silent() {
    let (mut hud, look) = base();
    let mut inp = inputs();
    inp.health = 15.0;
    hud.think(&inp, 0.0);
    hud.think(&inp, 1.5);
    inp.health = 25.0;
    hud.think(&inp, 2.0);
    assert_eq!(hud.anim.rgba("HudHealth", "BgColor"), Some([0, 0, 0, 76]));
    assert!(!hud.anim.is_running("HealthPulse") && !hud.anim.is_running("HealthLoop"));
    inp.health = 5.0;
    hud.think(&inp, 3.0);
    hud.events.clear();
    inp.health = 0.0;
    inp.alive = false;
    hud.think(&inp, 4.0);
    assert!(hud.fired(0.0).is_empty());
    assert!(hud.frame(&look, &inp).find_text("HEALTH").is_none(), "hidden while dead");
}

/// Test 6: the damage flash goes to `BrightFg` over a quarter second and
/// halfway back at 0.675 s it is 177 alpha.
#[test]
fn the_flash_colours() {
    let (mut hud, _) = base();
    let inp = inputs();
    hud.think(&inp, 0.0);
    run(&mut hud, &inp, 5.0);
    let t0 = hud.now;
    hud.fire("HealthDamageTaken");
    hud.think(&inp, t0);
    hud.think(&inp, t0 + 0.25);
    assert_eq!(hud.anim.rgba("HudHealth", "FgColor"), Some([255, 220, 0, 255]));
    hud.think(&inp, t0 + 0.675);
    assert_eq!(hud.anim.rgba("HudHealth", "FgColor"), Some([255, 220, 0, 177]));
}

/// Tests 12 to 15: the suit's events.
#[test]
fn the_suit_panel_follows_its_armour() {
    let (mut hud, look) = base();
    let mut inp = inputs();
    hud.think(&inp, 0.0);
    // Armour 0: nothing drawn.
    assert!(hud.frame(&look, &inp).find_text("SUIT").is_none());
    hud.events.clear();
    inp.armour = 15.0;
    hud.think(&inp, 0.1);
    assert_eq!(hud.fired(0.0), ["SuitPowerIncreasedAbove20"], "from 0, whatever the amount");
    hud.think(&inp, 0.2);
    assert!(hud.frame(&look, &inp).find_text("SUIT").is_some());
    // 30 to 18: damage and low.
    inp.armour = 30.0;
    hud.think(&inp, 1.0);
    hud.events.clear();
    inp.armour = 18.0;
    hud.think(&inp, 2.0);
    assert_eq!(hud.fired(0.0), ["SuitDamageTaken", "SuitArmorLow"]);
    // 10 to a battery: 15 is below 20.
    inp.armour = 10.0;
    hud.think(&inp, 3.0);
    hud.events.clear();
    inp.armour = 15.0;
    hud.think(&inp, 4.0);
    assert_eq!(hud.fired(0.0), ["SuitPowerIncreasedBelow20"]);
    // Down to 0: the panel fades out over 0.4 s.
    inp.armour = 0.0;
    hud.think(&inp, 5.0);
    assert_eq!(hud.fired(5.0), ["SuitPowerZero"]);
    hud.think(&inp, 5.4);
    assert_eq!(hud.anim.number("HudSuit", "Alpha"), Some(0.0));
    // Dead with armour left: it stays.
    inp.armour = 40.0;
    hud.think(&inp, 6.0);
    hud.think(&inp, 6.5);
    inp.alive = false;
    inp.health = 0.0;
    assert!(hud.frame(&look, &inp).find_text("SUIT").is_some(), "the suit shows while dead");
}

/// Test 16: the pistol's clip and reserve in the ammo panel at the layout's
/// places; the secondary hidden.
#[test]
fn the_ammo_panel_shows_clip_and_reserve() {
    let (mut hud, look) = base();
    let inp = inputs();
    hud.think(&inp, 0.0);
    run(&mut hud, &inp, 1.0);
    let f = hud.frame(&look, &inp);
    assert_eq!(text_at(&f, "18"), Vec2::new(640.0 - 150.0 + 44.0, 434.0));
    assert_eq!(text_at(&f, "150"), Vec2::new(640.0 - 150.0 + 98.0, 432.0 + 16.0));
    assert!(f.find_text("ALT").is_none() && f.find_text("AMMO").is_some());
}

/// Tests 18 and 19: a weapon without a clip counts the carried ammo with no
/// reserve and moves the box; one without a primary ammo type (the
/// S.L.A.M.) hides it.
#[test]
fn weapons_without_clips_or_primary_ammo() {
    let (mut hud, look) = base();
    let mut inp = inputs();
    hud.think(&inp, 0.0);
    inp.weapon = Some(hl2dm::ui::hud::WeaponNow { class: "weapon_frag".into(), uses_primary: true, clip: None, reserve: 3, ..Default::default() });
    hud.think(&inp, 1.0);
    hud.think(&inp, 1.4);
    let f = hud.frame(&look, &inp);
    let b = f.boxes().find(|(r, _)| r.width() == 100.0).expect("the box, 100 wide").0;
    assert_eq!(b.min.x, 640.0 - 118.0);
    assert!(f.find_text("3").is_some());
    assert!(f.texts().all(|t| t.as_text() != Some("0")), "no reserve number");
    inp.weapon = Some(hl2dm::ui::hud::WeaponNow { class: "weapon_slam".into(), uses_primary: false, clip: None, reserve: 0, ..Default::default() });
    hud.think(&inp, 2.0);
    assert!(hud.frame(&look, &inp).find_text("AMMO").is_none());
}

/// Test 17: pistol to SMG1: the clip weapon events, the box at r150.
#[test]
fn a_clip_weapon_switch_starts_the_weapon_events() {
    let (mut hud, _) = base();
    let mut inp = inputs();
    hud.think(&inp, 0.0);
    hud.events.clear();
    inp.weapon = Some(smg1(45, 0));
    hud.think(&inp, 1.0);
    let fired = hud.fired(0.0);
    assert!(fired.contains(&"WeaponUsesClips") && fired.contains(&"WeaponChanged"), "{fired:?}");
    hud.think(&inp, 1.5);
    assert_eq!(hud.anim.get("HudAmmo", "Position").and_then(|v| v.pair()), Some([640.0 - 150.0, 432.0]));
    assert_eq!(hud.anim.get("HudAmmo", "Size").and_then(|v| v.pair()), Some([132.0, 36.0]));
}

/// Test 20: the last round fired: the empty event, digits red until a
/// reload brings an increase.
#[test]
fn an_empty_clip_stays_red_until_a_reload() {
    let (mut hud, _) = base();
    let mut inp = inputs();
    inp.weapon = Some(pistol(1, 150));
    hud.think(&inp, 0.0);
    run(&mut hud, &inp, 3.0);
    hud.events.clear();
    inp.weapon = Some(pistol(0, 150));
    let t = hud.now + 0.05;
    hud.think(&inp, t);
    assert!(hud.fired(0.0).contains(&"AmmoEmpty"));
    run(&mut hud, &inp, 3.0);
    assert_eq!(hud.anim.rgba("HudAmmo", "FgColor"), Some([180, 0, 0, 230]));
    hud.events.clear();
    inp.weapon = Some(pistol(18, 132));
    let t = hud.now + 0.05;
    hud.think(&inp, t);
    assert!(hud.fired(0.0).contains(&"AmmoIncreased"));
}

/// Tests 21 to 23: aux power starts a bar that grows a line for the sprint,
/// and fades out when it is full again.
#[test]
fn aux_power_shows_below_full_and_fades_out_at_full() {
    let (mut hud, look) = base();
    let mut inp = inputs();
    hud.think(&inp, 0.0);
    // The very first start: full, from "none", fades the red out.
    assert!(hud.fired(0.0).contains(&"SuitAuxPowerMax"));
    run(&mut hud, &inp, 1.0);
    assert!(hud.frame(&look, &inp).find_text("AUX POWER").is_none(), "hidden at full");
    hud.events.clear();
    inp.aux = 99.6;
    inp.sprinting = true;
    let t = hud.now + 0.05;
    hud.think(&inp, t);
    let fired = hud.fired(0.0);
    assert!(fired.contains(&"SuitAuxPowerNotMax") && fired.contains(&"SuitAuxPowerOneItemActive"), "{fired:?}");
    run(&mut hud, &inp, 0.5);
    let f = hud.frame(&look, &inp);
    let at = text_at(&f, "SPRINT");
    assert_eq!(at, Vec2::new(16.0 + 8.0, 390.0 + 22.0));
    assert_eq!(hud.anim.get("HudSuitPower", "Size").and_then(|v| v.pair()), Some([102.0, 36.0]));
    // Ten chunks, nine lit at 99.6.
    let chunks = f.boxes().filter(|(r, _)| r.width() == 6.0 && r.height() == 4.0).count();
    assert_eq!(chunks, 10);
    // Back to 100 with no device: fades, then not drawn.
    inp.aux = 100.0;
    inp.sprinting = false;
    let t = hud.now + 0.05;
    hud.think(&inp, t);
    assert!(hud.fired(t - 0.01).contains(&"SuitAuxPowerMax"));
    run(&mut hud, &inp, 0.5);
    assert!(hud.frame(&look, &inp).find_text("AUX POWER").is_none());
}

/// Low aux (24.99) turns the bar red.
#[test]
fn a_low_aux_bar_is_red() {
    let (mut hud, _) = base();
    let mut inp = inputs();
    inp.aux = 60.0;
    hud.think(&inp, 0.0);
    hud.events.clear();
    inp.aux = 24.99;
    inp.sprinting = true;
    hud.think(&inp, 0.1);
    assert!(hud.fired(0.0).contains(&"SuitAuxPowerDecreasedBelow25"));
    run(&mut hud, &inp, 0.6);
    assert_eq!(hud.anim.rgba("HudSuitPower", "AuxPowerColor"), Some([255, 0, 0, 220]));
}

/// Test 67: in team play the team line fades while aux power is below
/// 100 and comes back at 100.
#[test]
fn the_team_line_makes_way_for_the_aux_bar() {
    let (mut hud, look) = base();
    let mut inp = inputs();
    inp.teamplay = true;
    inp.team = hl2dm::game::rules::COMBINE;
    hud.think(&inp, 0.0);
    run(&mut hud, &inp, 1.0);
    let f = hud.frame(&look, &inp);
    assert!(f.find_text("Team Combine").is_some(), "{:?}", f.texts().map(|t| t.as_text()).collect::<Vec<_>>());
    inp.aux = 90.0;
    let t = hud.now + 0.05;
    hud.think(&inp, t);
    run(&mut hud, &inp, 0.3);
    assert!(hud.frame(&look, &inp).find_text("Team Combine").is_none());
    inp.aux = 100.0;
    let t = hud.now + 0.05;
    hud.think(&inp, t);
    run(&mut hud, &inp, 0.55);
    assert!(hud.frame(&look, &inp).find_text("Team Combine").is_some());
}

/// Test 68: a spawn resets the panels: the health glow runs again, the
/// history is empty; the suit's memory stays.
#[test]
fn a_spawn_resets_the_panels() {
    let (mut hud, look) = base();
    let mut inp: Inputs = inputs();
    inp.armour = 40.0;
    hud.think(&inp, 0.0);
    hud.history.item(Some(("WeaponIcons", '+')), 0.0);
    hud.damage_message(&hl2dm::ui::damage::DamageMsg { health: 10.0, angle: Some(90.0), ..Default::default() }, 100.0);
    hud.think(&inp, 0.1);
    hud.reset(&look);
    assert!(hud.history.entries.is_empty());
    assert_eq!(hud.anim.rgba("HudDamageIndicator", "DmgColorLeft"), Some([0, 0, 0, 0]));
    hud.events.clear();
    hud.think(&inp, 0.2);
    assert!(hud.fired(0.0).contains(&"HealthIncreasedAbove20"), "the glow every spawn");
    assert_eq!(hud.numeric.suit, Some(40), "the suit's memory stays");
    let _ = blink_alpha(1);
    let _ = Screen::base();
}
