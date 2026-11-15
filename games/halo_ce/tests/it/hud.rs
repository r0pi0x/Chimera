//! Halo's HUD rules on their own (specs/halo_ce/hud.md, test cases (a)):
//! placement rectangles by anchor (S3), the widescreen canvas, meter
//! fills of the shield, health and weapon bars (S4), the shield's hit
//! episode (S5), the flash function (S6), the weapons' flashing thresholds
//! (S7), crosshair states and latches (S8), number layout (S9), blip
//! positions, fades and the sweep (S10) and who shows on the tracker
//! (S11). Pure functions of `halo_ce::ui::hud::{layout, rules}`; the disc's
//! own layout is `heavy::hud`.

// The spec's numbers are written as it states them.
#![allow(clippy::excessive_precision)]

use blam_formats::{
    cache::{Argb8, TagRef},
    hud::{Anchor, ColorBlock, MeterElement, MeteredPanel, Placement, StaticElement, weapon::FlashCutoffs},
};
use halo_ce::ui::hud::{
    layout::{self, Canvas, Value},
    rules::{self, CrosshairStates, ShieldMark, WeaponNow, crosshair},
};

const FRAME: [i32; 4] = [48, 36, 592, 444];

fn argb(v: u32) -> Argb8 {
    Argb8 { a: (v >> 24) as u8, r: (v >> 16) as u8, g: (v >> 8) as u8, b: v as u8 }
}

#[test]
fn s3_placement_rectangles() {
    let r = |a, o: [i16; 2], w: f32, h: f32| layout::rect(a, o, [w, h], [1.0, 1.0], FRAME);
    assert_eq!(r(Anchor::TopRight, [7, 0], 122.0, 31.0), [463, 36, 585, 67]);
    assert_eq!(r(Anchor::BottomRight, [10, 20], 64.0, 32.0), [518, 392, 582, 424]);
    assert_eq!(layout::anchor_point(Anchor::BottomLeft, [41, 41], FRAME), [89, 403]);
    assert_eq!(r(Anchor::TopLeft, [12, 1], 128.0, 32.0), [60, 37, 188, 69]);
    assert_eq!(r(Anchor::Center, [0, 0], 66.0, 66.0), [287, 207, 353, 273]);
    assert_eq!(r(Anchor::Center, [0, 0], 28.0, 28.0), [306, 226, 334, 254]);
    assert_eq!(r(Anchor::TopRight, [-5, 18], 128.0, 16.0), [469, 54, 597, 70]);
    let no_ammo = r(Anchor::Center, [-30, 30], 42.0, 25.0);
    assert_eq!([no_ammo[0], no_ammo[2]], [269, 311]);
    assert!((no_ammo[1] - 258).abs() <= 1 && (no_ammo[3] - 282).abs() <= 1, "{no_ammo:?}");
}

/// Section 16 (ours): a wide window keeps the height's scale; a right
/// anchor keeps its offset from the right safe edge, the centre stays
/// central, a 4:3 window is the original.
#[test]
fn the_canvas_scales_by_height_and_anchors_to_the_safe_edges() {
    let wide = Canvas::of(1920.0, 1080.0);
    assert!((wide.scale - 2.25).abs() < 1e-6);
    let shield = layout::rect(Anchor::TopRight, [7, 0], [122.0, 31.0], [1.0, 1.0], wide.frame());
    assert_eq!(shield[2], wide.width - 48 - 7, "7 units in from the right safe edge");
    assert_eq!(shield[2] - shield[0], 122, "the bar's own width, not stretched");
    let reticle = layout::rect(Anchor::Center, [0, 0], [66.0, 66.0], [1.0, 1.0], wide.frame());
    let centre = (reticle[0] + reticle[2]) as f32 / 2.0 * wide.scale;
    assert!((centre - 960.0).abs() <= wide.scale, "centred on the screen: {centre}");
    let screen = wide.to_screen(reticle.map(|v| v as f32));
    assert!((screen[2] - 66.0 * 2.25).abs() < 1e-3 && (screen[3] - 66.0 * 2.25).abs() < 1e-3, "square: {screen:?}");
    let square = Canvas::of(1280.0, 960.0);
    assert_eq!((square.width, square.frame()), (640, FRAME));
    assert_eq!(Canvas::of(640.0, 480.0), Canvas::ORIGINAL);
}

fn shield_meter() -> MeterElement {
    MeterElement {
        placement: Placement { offset: [7, 0], scale: [1.0, 1.0], scaling_flags: 0 },
        bitmap: TagRef { class: blam_formats::cache::ClassCode::new(b"bitm"), path_address: 0, id: blam_formats::cache::TagId::NONE },
        min_color: argb(0x0000509F),
        max_color: argb(0x002896FF),
        flash_color: argb(0x00FFFFFF),
        empty_color: argb(0),
        flags: 0,
        minimum: 0,
        sequence: 0,
        alpha_multiplier: 1,
        alpha_bias: 55,
        value_scale: 198,
        opacity: 0.0,
        fade: 0.5,
        disabled_color: argb(0),
        overlays: Vec::new(),
    }
}

fn health_panel() -> MeteredPanel {
    let mut meter = shield_meter();
    meter.min_color = argb(0x00FF0000);
    meter.max_color = argb(0x002896FF);
    meter.flags = 1;
    meter.minimum = 31;
    meter.alpha_multiplier = 30;
    meter.alpha_bias = 15;
    meter.value_scale = 0;
    let background = StaticElement {
        placement: meter.placement.clone(),
        bitmap: meter.bitmap,
        colors: block(0, 0, 0.0, 0.0, 0.0, 0),
        sequence: 0,
        overlays: Vec::new(),
    };
    MeteredPanel { background, meter, medium_color: argb(0x00FFDF00), max_cutoff: 0.67, min_cutoff: 0.33 }
}

fn block(plain: u32, flash: u32, period: f32, delay: f32, length: f32, count: i16) -> ColorBlock {
    ColorBlock {
        color: argb(plain),
        flash_color: argb(flash),
        flash_period: period,
        flash_delay: delay,
        flash_count: count,
        flash_flags: 0,
        flash_length: length,
        disabled_color: argb(0),
    }
}

#[test]
fn s4_meter_fills() {
    let m = shield_meter();
    let fill = |s: f32| rules::meter_alpha(&m, (198.0 * s) as i32);
    assert_eq!([fill(0.0), fill(0.1), fill(0.5), fill(1.0)], [55, 74, 154, 253]);
    // An overshield of 2.5 (hud.md 6, the stages in the corrected order):
    // the tag's stage full, stage 1 red full, stage 2 green half; stages 3
    // and 4 yellow and violet.
    let stages = rules::shield_stages(2.5, &ShieldMark { last: 2.5, fade: -1.0, tick: 0 }, 198);
    assert_eq!(stages.len(), 3);
    assert_eq!(stages.iter().map(|s| rules::meter_alpha(&m, s.min_value)).collect::<Vec<_>>(), [253, 253, 154]);
    assert_eq!(stages[0].colour, None);
    assert_eq!(stages[1].colour, Some([1.0, 0.0, 0.0]));
    assert_eq!(stages[2].colour, Some([0.0, 1.0, 0.0]));
    let stages = rules::shield_stages(4.5, &ShieldMark { last: 4.5, fade: -1.0, tick: 0 }, 198);
    assert_eq!(stages[3].colour, Some([1.0, 1.0, 0.0]));
    assert_eq!(stages[4].colour, Some([127.0 / 255.0, 0.0, 1.0]));
    let h = health_panel();
    let blue = [40.0 / 255.0, 150.0 / 255.0, 1.0];
    let yellow = [1.0, 223.0 / 255.0, 0.0];
    let red = [1.0, 0.0, 0.0];
    for (b, segments, alpha, colour) in
        [(1.0, 8, 255, blue), (0.5, 4, 135, yellow), (0.34, 2, 75, yellow), (0.33, 2, 75, red), (0.1, 0, 31, red), (0.0, 0, 31, red)]
    {
        let (value, c) = rules::health(&h, b);
        assert_eq!(value, segments, "{b}");
        assert_eq!(rules::meter_alpha(&h.meter, value), alpha, "{b}");
        assert_eq!(c, colour, "{b}");
    }
    // Weapon bars: the raw count through the leaf's alpha.
    let mut ar = shield_meter();
    (ar.alpha_multiplier, ar.alpha_bias) = (4, 2);
    assert_eq!([rules::meter_alpha(&ar, 60), rules::meter_alpha(&ar, 30)], [242, 122]);
    let mut rl = shield_meter();
    (rl.alpha_multiplier, rl.alpha_bias) = (120, 1);
    assert_eq!([2, 1, 0].map(|v| rules::meter_alpha(&rl, v)), [241, 121, 1]);
    let mut pistol = shield_meter();
    (pistol.alpha_multiplier, pistol.alpha_bias) = (20, 0);
    assert_eq!(rules::meter_alpha(&pistol, 12), 240);
    let mut heat = shield_meter();
    (heat.alpha_multiplier, heat.alpha_bias) = (1, 0);
    let w = |h: f32| rules::weapon_states(&WeaponNow { heat: h, ..Default::default() }, &cutoffs(0, 0, 75, 10))[2].value;
    assert_eq!([w(1.0), w(0.5)], [255, 127]);
    assert_eq!(rules::meter_alpha(&heat, 255), 255);
}

#[test]
fn s5_a_shield_hit_is_marked_for_15_ticks_then_snaps() {
    let m = shield_meter();
    let mut mark = ShieldMark::default();
    mark.update(1.0, 199);
    for tick in 200..=214 {
        mark.update(0.6, tick);
        assert_eq!((mark.last, mark.fade), (1.0, 0.0), "tick {tick}");
        let stage = rules::shield_stages(0.6, &mark, 198)[0];
        assert_eq!((rules::meter_alpha(&m, stage.min_value), rules::meter_alpha(&m, stage.max_value)), (173, 253));
        let d = rules::meter_draw(&m, stage.min_value, stage.max_value, false, false, stage.reference, stage.colour);
        assert_eq!(d.flash, [1.0, 1.0, 1.0], "the lost part in the flash colour at full strength");
    }
    mark.update(0.6, 215);
    assert_eq!((mark.last, mark.fade, mark.tick), (0.6, 0.5, 215));
    mark.update(0.6, 216);
    let stage = rules::shield_stages(0.6, &mark, 198)[0];
    assert_eq!((stage.min_value, stage.max_value, stage.reference), (118, 118, -1.0));
    // The recharge: each tick the shield rises, L follows, f is -1.
    for (i, tick) in (380..390).enumerate() {
        let s = 0.6 + 0.01 * (i + 1) as f32;
        mark.update(s, tick);
        assert_eq!((mark.last, mark.fade), (s, -1.0));
    }
}

#[test]
fn s6_the_flash_function() {
    let b = block(0x00000000, 0x002896FF, 3.0, 0.225, 0.225, 3);
    let at = |t: f32| rules::flash(&b, 100, 100 + (t * 30.0).round() as i64);
    let near = |a: [f32; 4], e: [f32; 4]| a.iter().zip(e).all(|(x, y)| (x - y).abs() < 2e-3);
    let peak = [40.0 / 255.0, 150.0 / 255.0, 1.0, 0.0];
    // Ticks are 1/30 s: plain at rest (0, 1.5 and 2.0 s), the pulse at
    // 0.9848 of the way at 1.0 s and at 0.1 s (phase' 0.1 of 0.225).
    for t in [0.0, 1.5, 2.0] {
        assert!(near(at(t), [0.0; 4]), "plain at {t}: {:?}", at(t));
    }
    assert!((at(0.1)[1] - 0.9848 * 150.0 / 255.0).abs() < 2e-3, "{:?}", at(0.1));
    // 1.0 s: the pulse at 0.9848 of the way (phase' 0.1 of 0.225).
    let one = at(1.0);
    assert!((one[1] - 0.9848 * 150.0 / 255.0).abs() < 2e-3, "{one:?}");
    // Near the middle of a pulse it is (almost) the flash colour.
    let mid = rules::flash(&b, 0, 0);
    assert!(near(mid, peak), "start tick 0: the flash colour");
    let mut reversed = b.clone();
    reversed.flash_flags = ColorBlock::REVERSE;
    assert!(near(rules::flash(&reversed, 100, 100), peak), "reversed: the flash colour at rest");
    let never = block(0x11223344, 0x55667788, 0.0, 0.225, 0.225, 3);
    assert_eq!(rules::flash(&never, 100, 103), rules::rgba(argb(0x11223344)));
}

fn cutoffs(total: i16, loaded: i16, heat: i16, age: i16) -> FlashCutoffs {
    FlashCutoffs { flags: 0, total_ammo: total, loaded_ammo: loaded, heat, age }
}

#[test]
fn s7_flashing_thresholds() {
    let ar = cutoffs(61, 10, 0, 0);
    let s = |reserve, loaded, reloading| rules::weapon_states(&WeaponNow { reserve, loaded, reloading, ..Default::default() }, &ar);
    assert!(s(61, 60, false)[0].flashing);
    assert!(!s(62, 60, false)[0].flashing);
    assert!(s(62, 10, false)[1].flashing);
    assert!(!s(62, 11, false)[1].flashing);
    assert!(!s(62, 10, true)[1].flashing);
    let pr = cutoffs(0, 0, 75, 10);
    let p = |heat, age| rules::weapon_states(&WeaponNow { heat, age, ..Default::default() }, &pr);
    assert!(p(0.75, 0.0)[2].flashing);
    assert!(!p(0.74, 0.0)[2].flashing);
    assert!(p(0.0, 0.91)[3].flashing);
    assert!(!p(0.0, 0.89)[3].flashing);
    assert_eq!(p(0.0, 0.25)[3].value, 75, "the battery percentage");
}

#[test]
fn s8_crosshair_states_and_the_no_ammo_latch() {
    let ar = cutoffs(61, 10, 0, 0);
    let mut x = CrosshairStates::default();
    let valid = (1 << crosshair::AIM) | (1 << crosshair::ZOOM) | (1 << crosshair::NO_AMMO);
    let hold = |_| 90;
    let empty = |firing| WeaponNow { firing, zoom: -1, ..Default::default() };
    x.update(valid, &WeaponNow { zoom: -1, ..Default::default() }, &ar, true, false, 1, hold);
    assert_eq!(x.zoom, 0);
    x.update(valid, &WeaponNow { zoom: 0, ..Default::default() }, &ar, true, false, 2, hold);
    assert_eq!(x.zoom, 1);
    x.update(valid, &WeaponNow { zoom: 1, ..Default::default() }, &ar, true, false, 3, hold);
    assert_eq!(x.zoom, 2);
    let mut x = CrosshairStates::default();
    for tick in 100..=110 {
        x.update(valid, &empty(true), &ar, true, false, tick, hold);
    }
    assert_eq!(x.start[crosshair::NO_AMMO], Some(100));
    for tick in 111..=189 {
        x.update(valid, &empty(tick == 150), &ar, true, false, tick, hold);
        assert!(x.renders(crosshair::NO_AMMO), "still drawn at {tick}");
    }
    assert_eq!(x.start[crosshair::NO_AMMO], Some(100), "a new pull while latched keeps the start");
    x.update(valid, &empty(false), &ar, true, false, 190, hold);
    assert!(!x.renders(crosshair::NO_AMMO), "cleared at 190");
    assert!(x.renders(crosshair::AIM) && x.aim == 0, "aim assist off: the plain reticle");
}

#[test]
fn s9_number_layout() {
    let g = |v, digits, lead, origin| layout::number_glyphs(Value::Int(v), digits, 0, lead, false, Anchor::TopLeft, origin, 9, 6);
    assert_eq!(g(7, 3, true, [77, 39]), [(7, 86), (0, 77), (0, 68)]);
    assert_eq!(layout::rect_at(Anchor::TopLeft, [86, 39], [18.0, 12.0], [1.0, 1.0]), [86, 39, 104, 51]);
    assert_eq!(g(7, 3, false, [77, 39]), [(7, 86)]);
    assert!(g(0, 3, false, [77, 39]).is_empty(), "0 without leading zeros: nothing");
    assert_eq!(g(60, 2, true, [73, 39]), [(0, 73), (6, 64)]);
    let right = layout::number_glyphs(Value::Int(12), 3, 0, false, false, Anchor::TopRight, [500, 39], 9, 6);
    assert_eq!(right, [(2, 500), (1, 491)]);
    assert_eq!(layout::rect_at(Anchor::TopRight, [500, 39], [18.0, 12.0], [1.0, 1.0])[2], 500, "the right edge on the origin");
    let metres = layout::number_glyphs(Value::Int(12), 3, 0, false, true, Anchor::TopLeft, [77, 39], 9, 6);
    assert_eq!(metres.iter().map(|g| g.0).collect::<Vec<_>>(), [13, 2, 1]);
    let sniper = layout::number_glyphs(Value::Real(12.345), 4, 3, true, false, Anchor::TopLeft, [445, 385], 9, 6);
    assert_eq!(sniper, [(5, 505), (4, 496), (3, 487), (10, 481), (2, 472), (1, 463), (0, 454), (0, 445)]);
    assert!(layout::number_glyphs(Value::Real(f32::NAN), 4, 3, true, false, Anchor::TopLeft, [445, 385], 9, 6).is_empty());
}

#[test]
fn s10_blips_their_fades_and_the_sweep() {
    let at = |dx: f32, dy: f32, yaw: f32| {
        let (qx, qy) = (rules::quantise(dx, 20.0), rules::quantise(dy, 20.0));
        rules::blip_offset(qx, qy, yaw, 20.0, 32.0, 42.0)
    };
    let near = |a: Option<[f32; 2]>, e: [f32; 2]| a.is_some_and(|a| (a[0] - e[0]).abs() < 0.1 && (a[1] - e[1]).abs() < 0.1);
    assert_eq!((rules::quantise(10.0, 20.0), rules::quantise(19.9, 20.0), rules::quantise(20.0, 20.0)), (63, 126, 127));
    assert!(near(at(10.0, 0.0, 0.0), [0.0, -25.7]), "ahead: up {:?}", at(10.0, 0.0, 0.0));
    assert!(near(at(0.0, 10.0, 0.0), [-25.7, 0.0]), "left: left");
    assert!(near(at(-10.0, 0.0, 0.0), [0.0, 25.7]), "behind: down");
    assert!(near(at(0.0, 10.0, std::f32::consts::FRAC_PI_2), [0.0, -25.7]), "facing it: up");
    assert!(near(at(19.9, 0.0, 0.0), [0.0, -41.8]));
    assert!(near(at(0.1, 0.0, 0.0), [0.0, 0.0]), "the centre");
    // hud.md 10/S10 (fourth errata): the zero quantised vector draws exactly
    // at the centre, whichever way the viewer faces (the distance's floor
    // moves nothing).
    for yaw in [0.0, 1.0, std::f32::consts::PI] {
        assert_eq!(rules::blip_offset(0, 0, yaw, 20.0, 32.0, 42.0), Some([0.0, 0.0]), "yaw {yaw}");
    }
    // hud.md 11: byte 127 decodes to exactly 20, and the drawing is strict: scanned, not drawn.
    assert!(at(20.0, 0.0, 0.0).is_none(), "at the range: not drawn");
    assert!(near(at(19.95, 0.0, 0.0), [0.0, -41.8]), "just inside: byte 126");
    assert!(rules::blip_offset(127, 127, 0.0, 20.0, 32.0, 42.0).is_none(), "past the range");
    // The table's quantised column: the bytes, truncated toward zero.
    let q = |dx: f32, dy: f32| (rules::quantise(dx, 20.0), rules::quantise(dy, 20.0));
    assert_eq!([q(10.0, 0.0), q(0.0, 10.0), q(-10.0, 0.0)], [(63, 0), (0, 63), (-63, 0)]);
    assert_eq!([q(19.9, 0.0), q(0.1, 0.0), q(20.0, 0.0), q(19.99, 0.0)], [(126, 0), (0, 0), (127, 0), (126, 0)]);
    assert!(near(at(19.99, 0.0, 0.0), [0.0, -41.8]), "19.99: byte 126, drawn");
    assert!(at(25.0, 0.0, 0.0).is_none(), "out of range");
    // hud.md 11 step 4: a unit 1 WU ahead is byte 6 (0.945 WU), 5.0
    // pixels out (5.2 unquantised); 10 WU decodes to 9.92 WU.
    assert_eq!(q(1.0, 0.0), (6, 0));
    let one = at(1.0, 0.0, 0.0).unwrap();
    assert!(one[0].abs() < 1e-4 && (one[1] + 4.96).abs() < 0.01, "1 WU: {one:?}");
    assert!((42.0 * (1.0f32 / 20.0).powf(0.7) - 5.16).abs() < 0.01, "unquantised");
    assert!((f32::from(q(10.0, 0.0).0) * 20.0 / 127.0 - 9.92).abs() < 0.005);
    let fade = |h| rules::blip_fade(h);
    let close = |a: f32, b: f32| (a - b).abs() < 1e-3;
    assert!(close(fade(0).0, 1.0) && close(fade(0).1, 1.0));
    assert!(close(fade(1).0, 0.81) && close(fade(1).1, 1.0022));
    assert!(close(fade(5).0, 0.25) && close(fade(5).1, 1.6187));
    assert!(close(fade(9).0, 0.01) && close(fade(9).1, 5.8411));
    assert_eq!(rules::BlipKind::Enemy.colour(), [1.0, 0.0, 0.0]);
    assert_eq!(rules::BlipKind::Friend.colour(), [1.0, 1.0, 0.0]);
    assert_eq!(rules::BlipKind::Own.colour(), [1.0, 0.5, 0.0]);
    assert!((rules::sweep(0) - 7.27).abs() < 0.01, "{}", rules::sweep(0));
    assert!((rules::sweep(30) * 2.0 - 0.8556).abs() < 1e-3);
    assert!((rules::sweep(62) * 2.0 - 0.4).abs() < 1e-6, "2.0667 s");
    assert_eq!(rules::sweep(63), rules::sweep(0), "2.1 s wraps");
}

#[test]
fn s11_who_shows_on_the_tracker() {
    let s = |speed, firing, d| rules::shows(speed, firing, d, 0.0002, 20.0);
    assert!(s(0.0142, false, 5.0));
    assert!(!s(0.0141, false, 5.0));
    assert!(s(0.0, true, 5.0), "holding the trigger");
    assert!(!s(1.0, false, 20.1), "out of range");
    // The history: a scan every 15th tick, copies between, a stopped unit
    // leaves the newest entry at once and older ones keep its echo.
    let mut t = rules::Tracker::default();
    let unit = |shows| rules::Seen { id: 1, kind: rules::BlipKind::Enemy, at: [5.0, 0.0], shows };
    t.frame(15, &[unit(true)], 0.0, 20.0);
    t.frame(16, &[unit(true)], 0.0, 20.0);
    assert_eq!(t.entries.len(), 2);
    assert!(t.entries.iter().all(|e| e.len() == 1));
    t.frame(17, &[unit(false)], 0.0, 20.0);
    assert!(t.entries[0].is_empty() && t.entries[1].len() == 1, "the echo stays in older entries");
    t.frame(40, &[unit(true)], 0.0, 20.0);
    assert_eq!(t.entries.len(), rules::RADAR_HISTORY);
}

/// The party layout's move (`layout::party_offset`): the block's
/// bottom-left corner onto the tracker's, pulled back inside the safe
/// frame when it would poke out.
#[test]
fn the_party_layouts_offset() {
    use halo_ce::ui::hud::layout::party_offset;
    let frame = [48, 36, 592, 444];
    // A top-right block onto a bottom-left tracker.
    assert_eq!(party_offset([440.0, 40.0, 590.0, 80.0], [50.0, 330.0, 140.0, 440.0], frame, 0.0), [-390.0, 360.0]);
    // A block reaching above the frame once moved is pulled back down.
    assert_eq!(party_offset([440.0, 40.0, 590.0, 500.0], [50.0, 330.0, 140.0, 440.0], frame, 0.0), [-390.0, -4.0]);
    // Under the party's status band's top (`Canvas::party_floor`): lower.
    assert_eq!(party_offset([440.0, 40.0, 590.0, 80.0], [50.0, 330.0, 140.0, 440.0], frame, 420.0), [-390.0, 380.0]);
}
