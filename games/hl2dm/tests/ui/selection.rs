//! The weapon selection (specs/hl2dm/hud.md 5; test cases 24 to 34).

use bevy::prelude::*;
use hl2dm::ui::{
    frame::{Draw, Screen},
    selection::{Action, SOUND_DENY, SOUND_MOVE},
};

use crate::common::*;

/// Crowbar, gravity gun, pistol, .357, SMG1 and a frag: buckets 0 (crowbar,
/// gravity gun), 1 (pistol, .357), 2 (SMG1), 4 (frag).
fn loadout() -> Vec<hl2dm::ui::hud::Owned> {
    vec![
        owned(1, "weapon_crowbar", 0, 0, true, "#HL2_Crowbar"),
        owned(2, "weapon_physcannon", 0, 1, true, "#HL2_GravityGun"),
        owned(3, "weapon_pistol", 1, 0, true, "9MM PISTOL"),
        owned(4, "weapon_357", 1, 1, true, "#HL2_357Handgun"),
        owned(5, "weapon_smg1", 2, 0, true, "SMG"),
        owned(6, "weapon_frag", 4, 0, true, "#HL2_Grenade"),
    ]
}

fn open(screen: Screen, slot: u8) -> (hl2dm::ui::hud::Hl2Hud, hl2dm::ui::look::Look, hl2dm::ui::hud::Inputs) {
    let (mut hud, look) = setup(screen);
    let mut inp = inputs();
    inp.owned = loadout();
    inp.active = Some(entity(5));
    hud.think(&inp, 0.0);
    hud.select_slot(slot, &inp.owned.clone(), inp.active, 0);
    run(&mut hud, &inp, 0.2);
    (hud, look, inp)
}

fn boxes(f: &hl2dm::ui::frame::Frame) -> Vec<Rect> {
    f.boxes().map(|(r, _)| *r).collect()
}

/// Test 24: `slot2` at 640 by 480 opens bucket 1: x0 is 164, bucket 0 a
/// small box at 164, the column at 204 (pistol y 16 to 100), the others'
/// small boxes at 324 to 444; the pistol's name is under it.
#[test]
fn slot_two_opens_the_pistols_bucket() {
    let (hud, look, inp) = open(Screen::base(), 1);
    assert_eq!(hud.selection.highlighted, Some(entity(3)));
    let f = hud.frame(&look, &inp);
    let b = boxes(&f);
    assert!(b.contains(&Rect::new(164.0, 16.0, 196.0, 48.0)), "bucket 0's small box: {b:?}");
    assert!(b.contains(&Rect::new(204.0, 16.0, 316.0, 100.0)), "the pistol's box");
    assert!(b.contains(&Rect::new(204.0, 108.0, 316.0, 192.0)), "the .357's box");
    for x in [324.0, 364.0, 404.0, 444.0] {
        assert!(b.contains(&Rect::new(x, 16.0, x + 32.0, 48.0)), "small box at {x}");
    }
    // Its bucket number at (208, 20).
    match f.find_text("2") {
        Some(Draw::Text { at, .. }) => assert_eq!(*at, Vec2::new(208.0, 20.0)),
        other => panic!("{other:?} in {:?}", f.texts().map(|t| t.as_text()).collect::<Vec<_>>()),
    }
    // The name at y 80 (16 + 64), under the box.
    match f.find_text("9MM PISTOL") {
        Some(Draw::Text { at, .. }) => assert_eq!(at.y, 80.0),
        other => panic!("{other:?}"),
    }
}

/// Test 25: `slot2` again highlights the .357.
#[test]
fn the_same_slot_again_steps_through_the_bucket() {
    let (mut hud, look, mut inp) = open(Screen::base(), 1);
    let acts = hud.select_slot(1, &inp.owned.clone(), inp.active, 0);
    assert_eq!(acts, [Action::Sound(SOUND_MOVE)]);
    assert_eq!(hud.selection.highlighted, Some(entity(4)));
    run(&mut hud, &inp, 0.2);
    inp.active = Some(entity(5));
    assert!(boxes(&hud.frame(&look, &inp)).contains(&Rect::new(204.0, 108.0, 316.0, 192.0)));
}

/// Test 26: at 1920 by 1080 a small box is 72, a large one 252 by 189, the
/// gap 18; x0 609, y 36.
#[test]
fn the_boxes_scale() {
    let (hud, look, inp) = open(Screen::new(1920.0, 1080.0), 1);
    let b = boxes(&hud.frame(&look, &inp));
    assert!(b.contains(&Rect::new(609.0, 36.0, 609.0 + 72.0, 36.0 + 72.0)), "{b:?}");
    assert!(b.contains(&Rect::new(609.0 + 72.0 + 18.0, 36.0, 609.0 + 90.0 + 252.0, 36.0 + 189.0)));
}

/// Test 27: with a frag (bucket 4 position 0) and a S.L.A.M. (position 2)
/// and no RPG: a column of frag, empty box, S.L.A.M.; with empty slots off
/// the S.L.A.M. sits right under the frag.
#[test]
fn empty_positions_are_boxes_unless_hidden() {
    let (mut hud, look) = base();
    let mut inp = inputs();
    inp.owned = vec![owned(1, "weapon_frag", 4, 0, true, "F"), owned(2, "weapon_slam", 4, 2, true, "S")];
    inp.active = Some(entity(1));
    hud.think(&inp, 0.0);
    hud.select_slot(4, &inp.owned.clone(), inp.active, 0);
    run(&mut hud, &inp, 0.2);
    let large = |inp: &hl2dm::ui::hud::Inputs, hud: &hl2dm::ui::hud::Hl2Hud| {
        boxes(&hud.frame(&look, inp)).into_iter().filter(|r| r.width() == 112.0).map(|r| r.min.y).collect::<Vec<_>>()
    };
    assert_eq!(large(&inp, &hud), [16.0, 108.0, 200.0]);
    inp.hide_empty_slots = true;
    assert_eq!(large(&inp, &hud), [16.0, 108.0]);
}

/// Test 28: with no input after the last command the fade starts at 0.5 s
/// (the values start moving at 1.0), the selection closes at 1.25 s.
#[test]
fn the_selection_fades_and_closes_by_itself() {
    let (mut hud, look) = base();
    let mut inp = inputs();
    inp.owned = loadout();
    inp.active = Some(entity(5));
    hud.think(&inp, 0.0);
    hud.select_slot(1, &inp.owned.clone(), inp.active, 0);
    for k in 1..=20 {
        hud.think(&inp, k as f64 * 0.05);
    }
    assert!(hud.fired(0.45).contains(&"FadeOutWeaponSelectionMenu"));
    // Full brightness until 1.0 s, a quarter of the way into the fade at 1.25.
    assert_eq!(hud.anim.number("HudWeaponSelection", "SelectionAlpha"), Some(255.0));
    hud.think(&inp, 1.2);
    let a = hud.anim.number("HudWeaponSelection", "SelectionAlpha").unwrap();
    assert!((a - 255.0 * 0.8).abs() < 1.0, "{a}");
    hud.think(&inp, 1.3);
    assert!(!hud.selection.open);
    let large = boxes(&hud.frame(&look, &inp)).into_iter().filter(|r| r.width() == 112.0 || r.width() == 32.0).count();
    assert_eq!(large, 0, "the selection is gone");
}

/// Test 29: attack with an unselectable weapon highlighted denies and
/// stays open; test 30: with a good one it switches, plays the sounds and
/// closes.
#[test]
fn attack_confirms_the_highlighted_weapon() {
    let (mut hud, _, mut inp) = open(Screen::base(), 2);
    inp.owned[4].selectable = false;
    // Nothing selectable in the bucket: the slot found nothing, stays closed.
    let acts = hud.confirm(&inp.owned.clone());
    assert!(matches!(acts.as_slice(), [Action::Sound(SOUND_DENY)] | []), "{acts:?}");
    let (mut hud, _, inp) = open(Screen::base(), 1);
    let acts = hud.confirm(&inp.owned.clone());
    assert_eq!(
        acts,
        [Action::Switch(entity(3)), Action::Sound("Player.WeaponSelected"), Action::Sound("Player.WeaponSelectionClose")]
    );
    assert!(!hud.selection.open);
}

/// Tests 31 to 34: fast switch and empty buckets.
#[test]
fn fast_switch_and_empty_buckets() {
    let (mut hud, _) = base();
    let mut inp = inputs();
    inp.owned = loadout();
    inp.active = Some(entity(5));
    hud.think(&inp, 0.0);
    let owned = inp.owned.clone();
    // Test 31: holding the SMG1, `slot3`'s next... bucket 2 holds only it.
    assert_eq!(hud.select_slot(2, &owned, Some(entity(5)), 1), [], "the one in hand: nothing");
    // A bucket with another weapon: `slot2` switches at once to the pistol.
    assert_eq!(hud.select_slot(1, &owned, Some(entity(5)), 1), [Action::Switch(entity(3))]);
    assert!(!hud.selection.open, "nothing is drawn");
    // Holding the pistol: the next in the bucket, the .357.
    assert_eq!(hud.select_slot(1, &owned, Some(entity(3)), 1), [Action::Switch(entity(4))]);
    // Test 33: an empty bucket denies with fast switch, and with buckets
    // only moves.
    assert_eq!(hud.select_slot(3, &owned, Some(entity(5)), 1), [Action::Sound(SOUND_DENY)]);
    assert_eq!(hud.select_slot(3, &owned, Some(entity(5)), 0), [Action::Sound(SOUND_MOVE)]);
    assert!(!hud.selection.open, "no selection opens");
}

/// `invnext` steps through every weapon, wrapping.
#[test]
fn the_wheel_steps_through_the_weapons() {
    let (mut hud, _) = base();
    let mut inp = inputs();
    inp.owned = loadout();
    inp.active = Some(entity(6));
    hud.think(&inp, 0.0);
    let owned = inp.owned.clone();
    hud.step(1, &owned, Some(entity(6)), 0);
    assert_eq!(hud.selection.highlighted, Some(entity(1)), "past the last: the first");
    hud.step(-1, &owned, Some(entity(6)), 0);
    assert_eq!(hud.selection.highlighted, Some(entity(6)));
}
