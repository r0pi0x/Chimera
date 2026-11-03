//! The pickup history's layout and fade (specs/hl2dm/hud.md 9; test cases
//! 52, 53). The entries' rules are the module's own tests.

use bevy::prelude::*;
use hl2dm::ui::frame::Draw;

use crate::common::*;

/// Test 52: an SMG1 ammo box of 45: row 0 at panel y 264 (screen 304), the
/// SMG1 ammo glyph at x 248 less its width less 38 from the panel's left
/// (r252), "45" at panel x 212 (screen r40).
#[test]
fn an_ammo_row_sits_bottom_up() {
    let (mut hud, look) = base();
    let inp = inputs();
    hud.think(&inp, 0.0);
    hud.history.ammo("hl2dm:smg1", 45, Some(("WeaponIconsSmall", 'r')), 0.0);
    let f = hud.frame(&look, &inp);
    let find = |t: &str| match f.find_text(t) {
        Some(Draw::Text { at, .. }) => *at,
        other => panic!("{t}: {other:?}"),
    };
    // No font files: the glyph's width is 0, the icon's top at 304.
    assert_eq!(find("r"), Vec2::new(640.0 - 252.0 + 248.0 - 38.0, 304.0));
    let n = find("45");
    assert_eq!(n.x, 640.0 - 252.0 + 212.0);
}

/// Test 53: alpha 255 until 3.19 s are left, 80 at one second, gone at 5.
#[test]
fn entries_fade_over_their_last_3_seconds() {
    let (mut hud, look) = base();
    let inp = inputs();
    hud.think(&inp, 0.0);
    hud.history.ammo("hl2dm:pistol", 20, Some(("WeaponIconsSmall", 'p')), 0.0);
    let alpha = |hud: &hl2dm::ui::hud::Hl2Hud| match hud.frame(&look, &inp).find_text("20") {
        Some(Draw::Text { color, .. }) => Some(color[3]),
        _ => None,
    };
    let mut hud2 = hud.clone();
    hud2.think(&inp, 1.8125);
    assert_eq!(alpha(&hud2), Some(255));
    hud2.think(&inp, 4.0);
    assert_eq!(alpha(&hud2), Some(80));
    hud2.think(&inp, 5.0);
    assert_eq!(alpha(&hud2), None);
}
