//! Sizes (chimera_ui::units).

use chimera_ui::{TickLayout, UiUnits, units};

/// 1 cq is 12.8 px at 720 lines and scales with the height only.
#[test]
fn cq_follows_the_height() {
    assert_eq!(units::cq(1.0, 720.0), 12.8);
    assert!((units::cq(1.0, 1080.0) - 19.2).abs() < 1e-4);
    assert!((units::cq(1.0, 2160.0) - 38.4).abs() < 1e-4);
    // Never under a third of 720 lines.
    assert_eq!(units::cq(1.0, 100.0), units::cq(1.0, 240.0));
    // Snapping at 150 % scale: whole physical pixels.
    let u = UiUnits::new(720.0, 1.5);
    let s = u.snap(10.1);
    assert_eq!((s * 1.5).fract(), 0.0);
    assert_eq!(u.hairline() * 1.5, 1.0);
    // Small text floors at the theme's 9 physical px.
    let u = UiUnits::new(720.0, 1.0);
    assert_eq!(u.text_px(0.42), 9.0);
    assert_eq!(u.text_px(1.0), 13.0);
}

/// The prototype's tick-bar layout: the lobby countdown's 48 ticks in a
/// 20 cq box at 1080p, 1440p and 4K (physical px).
#[test]
fn tick_bar_layout_snaps_to_physical_pixels() {
    let cases = [
        (384, TickLayout { pitch: 8, tick: 3, gap: 5, w: 379, h: 12 }),
        (512, TickLayout { pitch: 10, tick: 4, gap: 6, w: 474, h: 15 }),
        (768, TickLayout { pitch: 16, tick: 6, gap: 10, w: 758, h: 24 }),
    ];
    for (box_w, want) in cases {
        assert_eq!(TickLayout::new(box_w, 100, 48), want, "box {box_w}");
    }
    // Capped by the box's height, never under 3; never under a 2 px pitch.
    assert_eq!(TickLayout::new(768, 10, 48).h, 10);
    assert_eq!(TickLayout::new(768, 1, 48).h, 3);
    let tiny = TickLayout::new(10, 10, 48);
    assert_eq!((tiny.pitch, tiny.tick), (2, 1));
}
