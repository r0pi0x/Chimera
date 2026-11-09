//! The damage indicator, death notices and target ID (specs/hl2dm/hud.md 8,
//! 10, 11; test cases 45 to 51, 58 to 63).

use bevy::prelude::*;
use hl2dm::{
    game::rules::{COMBINE, REBELS},
    ui::{
        damage::{DamageMsg, Kinds},
        frame::{Draw, Screen},
        hud::Hl2Hud,
        notices::Target,
    },
};

use crate::common::*;

fn hit(hud: &mut Hl2Hud, health: f32, angle: Option<f32>) {
    hud.damage_message(&DamageMsg { health, angle, ..Default::default() }, 100.0);
}

/// Test 48: `HudTakeDamageLeft` at t0: the colour at t0 + 0.15 is
/// 255 44 0 200; alpha 100 at 0.425, 0 at 0.8.
#[test]
fn a_left_flash_fades_over_0_8_seconds() {
    let (mut hud, _) = base();
    let inp = inputs();
    hud.think(&inp, 5.0);
    hit(&mut hud, 10.0, Some(90.0));
    hud.think(&inp, 5.0);
    hud.think(&inp, 5.15);
    assert_eq!(hud.anim.rgba("HudDamageIndicator", "DmgColorLeft"), Some([255, 44, 0, 200]));
    hud.think(&inp, 5.425);
    let a = hud.anim.rgba("HudDamageIndicator", "DmgColorLeft").unwrap()[3];
    assert!((i32::from(a) - 100).abs() <= 1, "{a}");
    hud.think(&inp, 5.8);
    assert_eq!(hud.anim.rgba("HudDamageIndicator", "DmgColorLeft").unwrap()[3], 0);
}

/// Test 46: a heavy hit from the left starts the high flash; both sides
/// draw the high shape (the right one's alpha 0: nothing).
#[test]
fn a_heavy_hit_draws_the_high_shape() {
    let (mut hud, look) = base();
    let inp = inputs();
    hud.think(&inp, 0.0);
    hit(&mut hud, 30.0, Some(90.0));
    assert!(hud.fired(0.0).contains(&"HudTakeDamageHighLeft"));
    hud.think(&inp, 0.0);
    hud.think(&inp, 0.1);
    let f = hud.frame(&look, &inp);
    let fades: Vec<Rect> = f.0.iter().filter_map(|d| if let Draw::Fade { rect, .. } = d { Some(*rect) } else { None }).collect();
    assert_eq!(fades, [Rect::new(0.0, 0.0, 320.0, 480.0)], "the left half only");
}

/// Test 51: the normal shape sits at x 30 to 66 (right: 574 to 610).
#[test]
fn the_side_flash_sits_where_the_layout_puts_it() {
    let (mut hud, mut look) = base();
    let mut h = (*look.hud).clone();
    let mut p = h.panels["HudHealth"].clone();
    p.keys.extend([("dmg_xpos", "30"), ("dmg_ypos", "100"), ("dmg_wide", "36"), ("dmg_tall1", "240"), ("dmg_tall2", "200")].map(|(k, v)| (k.to_lowercase(), v.to_string())));
    h.panels.insert("HudDamageIndicator".into(), p);
    look.hud = std::sync::Arc::new(h);
    let inp = inputs();
    hud.think(&inp, 0.0);
    hit(&mut hud, 10.0, Some(270.0));
    hud.think(&inp, 0.0);
    hud.think(&inp, 0.1);
    let f = hud.frame(&look, &inp);
    let r = f.0.iter().find_map(|d| if let Draw::Fade { rect, .. } = d { Some(*rect) } else { None }).expect("the right side's flash");
    assert_eq!((r.min.x, r.max.x), (574.0, 610.0));
}

/// Test 49: fall damage (the world) starts nothing; the message of a death
/// starts the long red fade (test 50: alpha 48 at 0.15 s, 175 at 1.3 s).
#[test]
fn falls_flash_nothing_and_death_fades_red() {
    let (mut hud, _) = base();
    let inp = inputs();
    hud.think(&inp, 0.0);
    hud.events.clear();
    hud.damage_message(&DamageMsg { health: 10.0, world: true, ..Default::default() }, 90.0);
    assert!(!hud.fired(0.0).iter().any(|e| e.starts_with("HudTakeDamage")));
    hud.think(&inp, 10.0);
    hud.damage_message(&DamageMsg { health: 100.0, ..Default::default() }, 0.0);
    assert_eq!(hud.fired(10.0), ["HudPlayerDeath"]);
    hud.think(&inp, 10.0);
    hud.think(&inp, 10.15);
    let a = hud.anim.rgba("HudDamageIndicator", "DmgFullscreenColor").unwrap()[3];
    assert!((i32::from(a) - 48).abs() <= 1, "{a}");
    hud.think(&inp, 11.3);
    let a = hud.anim.rgba("HudDamageIndicator", "DmgFullscreenColor").unwrap()[3];
    assert!((i32::from(a) - 175).abs() <= 6, "{a}");
}

#[test]
fn drowning_flashes_both_sides() {
    let (mut hud, _) = base();
    let inp = inputs();
    hud.think(&inp, 0.0);
    hud.damage_message(
        &DamageMsg { health: 2.0, world: true, kinds: Kinds { drown: true, ..Default::default() }, ..Default::default() },
        98.0,
    );
    assert!(hud.fired(0.0).contains(&"HudTakeDamageDrown"));
}

/// Test 58: A kills B with the shotgun in deathmatch at 640 by 480: line 0
/// at y 16, "B" ending at x 624 with the glyph left of it in 255 80 0, "A"
/// left of that, both names in the unassigned yellow.
#[test]
fn a_notice_is_laid_out_from_the_right() {
    let (mut hud, mut look) = base();
    let mut h = (*look.hud).clone();
    h.icons.insert("death_shotgun".into(), ("HL2MPTypeDeath".into(), '0'));
    look.hud = std::sync::Arc::new(h);
    let inp = inputs();
    hud.think(&inp, 0.0);
    hud.death_notice(Some(("A".into(), 0)), ("B".into(), 0), "shotgun");
    let f = hud.frame(&look, &inp);
    let find = |t: &str| match f.find_text(t) {
        Some(Draw::Text { at, color, align, .. }) => (*at, *color, *align),
        other => panic!("{t}: {other:?}"),
    };
    let (b_at, b_colour, align) = find("B");
    assert_eq!((b_at, b_colour), (Vec2::new(624.0, 16.0), [255, 178, 0, 255]));
    assert_eq!(align, hl2dm::ui::frame::Align::Right);
    assert_eq!(find("0").1, [255, 80, 0, 255]);
    assert_eq!(find("A").1, [255, 178, 0, 255]);
}

/// Tests 59 and 61: five kills inside six seconds show four lines at y 16,
/// 35, 54 and 73; a notice is gone at 6 s with no fade.
#[test]
fn four_notices_live_six_seconds() {
    let (mut hud, look) = base();
    let inp = inputs();
    hud.think(&inp, 0.0);
    for k in 0..5 {
        hud.death_notice(Some((format!("K{k}"), 0)), (format!("V{k}"), 0), "smg1");
        hud.think(&inp, 0.1 * (k + 1) as f64);
    }
    let f = hud.frame(&look, &inp);
    let ys: Vec<f32> = f.texts().filter(|d| d.as_text().is_some_and(|t| t.starts_with('V'))).map(|d| if let Draw::Text { at, .. } = d { at.y } else { 0.0 }).collect();
    assert_eq!(ys, [16.0, 35.0, 54.0, 73.0]);
    assert!(f.find_text("V0").is_none(), "the first was dropped");
    hud.think(&inp, 6.15);
    assert_eq!(hud.notices.list.len(), 3, "the first of the four expired at 6.1");
    hud.think(&inp, 7.0);
    assert!(hud.notices.list.is_empty());
}

/// Test 60: a world kill shows the skull between two names; a suicide the
/// skull and the victim.
#[test]
fn kills_without_an_icon_show_the_skull() {
    let (mut hud, mut look) = base();
    let mut h = (*look.hud).clone();
    h.sprites.insert("d_skull".into(), chimera_world::map::hud::HudSprite { texture: 0, rect: [192.0, 224.0, 32.0, 16.0] });
    look.hud = std::sync::Arc::new(h);
    let inp = inputs();
    hud.think(&inp, 0.0);
    hud.death_notice(Some(("A".into(), 0)), ("B".into(), 0), "physcannon");
    hud.death_notice(None, ("C".into(), 0), "world");
    let f = hud.frame(&look, &inp);
    let skulls = f.0.iter().filter(|d| matches!(d, Draw::Sprite { sprite, .. } if sprite == "d_skull")).count();
    assert_eq!(skulls, 2);
    assert!(f.find_text("A").is_some(), "the killer's name beside the skull");
    assert_eq!(f.texts().filter(|d| d.as_text() == Some("C")).count(), 1);
}

/// Tests 62 and 63: a teammate in team play reads "Friend: name Health:
/// 1%" in the team's colour, centred at y 260; an enemy "Enemy: name".
#[test]
fn the_target_id_names_who_you_look_at() {
    let (mut hud, look) = base();
    let inp = inputs();
    hud.think(&inp, 0.0);
    hud.notices.target = Some(Target { name: "Alex".into(), team: COMBINE, friend: Some(1.0) });
    let mut f = hl2dm::ui::frame::Frame::default();
    hud.draw_target(&look, &inputs(), &mut f);
    match f.find_text("Friend: Alex Health: 1%") {
        Some(Draw::Text { at, color, .. }) => assert_eq!((*at, *color), (Vec2::new(320.0, 260.0), [153, 204, 255, 255])),
        other => panic!("{other:?}"),
    }
    hud.notices.target = Some(Target { name: "Bo".into(), team: REBELS, friend: None });
    let mut f = hl2dm::ui::frame::Frame::default();
    hud.draw_target(&look, &inputs(), &mut f);
    assert!(f.find_text("Enemy: Bo").is_some());
    let _ = Screen::base();
}
