//! Sizes (chimera_ui::units).

use bevy::{prelude::*, window::{PrimaryWindow, WindowResolution}};
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

/// The UI fits the largest 16:9 frame in the window: by height at 16:9
/// and wider, by width x 9/16 in a 4:3 or tall window (the main menu's
/// 100 cq frame was cropped at a tall window's sides), with the window's
/// scale factor; 1080p keeps its 1080 lines (the parity numbers).
#[test]
fn units_fit_the_16_9_frame_in_the_window() {
    let cases = [
        // (physical w, h, scale factor, layout height in logical px)
        (1920, 1080, 1.0, 1080.0),
        (1280, 720, 1.0, 720.0),
        (3440, 1440, 1.0, 1440.0),
        (1624, 1558, 1.0, 913.5),
        (1280, 960, 1.0, 720.0),
        (1920, 1080, 1.5, 720.0),
        (1920, 1080, 1.25, 864.0),
    ];
    for (w, h, sf, want) in cases {
        let mut app = App::new();
        app.init_resource::<UiUnits>().add_systems(Update, units::track_window);
        app.world_mut().spawn((
            Window { resolution: WindowResolution::new(w, h).with_scale_factor_override(sf), ..default() },
            PrimaryWindow,
        ));
        app.update();
        let u = *app.world().resource::<UiUnits>();
        assert!((u.height - want).abs() < 0.01, "{w}x{h} @{sf}: {}", u.height);
        assert_eq!(u.scale_factor, sf);
        // The 100 cq frame fits the window's width.
        assert!(u.cq(100.0) <= w as f32 / sf + 0.01, "{w}x{h} @{sf}: frame {}", u.cq(100.0));
    }
    assert_eq!(units::layout_height(0.0, 900.0), 900.0);
}
