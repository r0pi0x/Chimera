//! The kit's icons (chimera_ui::icons).

use bevy::prelude::*;
use chimera_ui::icons::{self, GUNS, IconFor, IconSource, Icons};

/// Every gun and mark rasterizes to its aspect at the asked height; a
/// hole stays empty and a solid part full (the pistol's trigger guard).
#[test]
fn icons_rasterize_with_holes() {
    let set = Icons::shipped();
    for key in GUNS.iter().copied().chain(["mark", "crown", "trophy", "kill", "death", "warn", "bot", "ping", "loss"]) {
        let icon = set.get(key).unwrap_or_else(|| panic!("{key} in icons.toml"));
        for px in [16, 32, 48] {
            let (w, h, cov) = icons::rasterize(icon, px);
            assert_eq!(h, px);
            assert_eq!(w, (icon.aspect() * px as f32).ceil() as u32, "{key}");
            assert!(cov.iter().any(|c| *c == 255), "{key} at {px}: some pixel full");
        }
    }
    let pistol = set.get("pistol").unwrap();
    let (w, _, cov) = icons::rasterize(pistol, 16);
    assert_eq!(w, 24);
    let at = |x: usize, y: usize| cov[y * w as usize + x];
    // The guard's hole spans x 11..15, y 9..11; the frame above it is solid.
    assert_eq!(at(12, 9), 0, "in the hole");
    assert_eq!(at(13, 10), 0, "in the hole");
    assert_eq!(at(12, 4), 255, "the slide");
    assert_eq!(at(0, 0), 0, "outside");
    // Whole-unit edges at 16 px: no half-covered pixel along the slide's top.
    assert_eq!(at(12, 1), 0);
    assert_eq!(at(12, 2), 255);
}

/// Triangles cover each icon's even-odd area exactly (holes left out) and never
/// fall outside it.
#[test]
fn icons_tessellate_with_holes() {
    let set = Icons::shipped();
    for key in GUNS.iter().copied().chain(["crown", "trophy", "kill", "death", "warn", "bot", "ping", "loss"]) {
        let icon = set.get(key).unwrap();
        let tris = icons::tessellate(icon);
        let area: f32 = tris.iter().map(|[a, b, c]| ((*b - *a).perp_dot(*c - *a) / 2.0).abs()).sum();
        // The even-odd area, measured by the rasterizer at a large size.
        let (_, _, big) = icons::rasterize(icon, 512);
        let s2 = (512.0 / icon.h).powi(2);
        let want = big.iter().map(|c| *c as f32 / 255.0).sum::<f32>() / s2;
        assert!((area - want).abs() < 0.01 * want.max(1.0), "{key}: triangles {area}, shape {want}");
        let (_, _, cov) = icons::rasterize(icon, 64);
        let w = (icon.aspect() * 64.0).ceil() as usize;
        let s = 64.0 / icon.h;
        for [a, b, c] in &tris {
            let m = (*a + *b + *c) / 3.0 * s;
            let (x, y) = (m.x as usize, m.y as usize);
            assert!(cov[y * w + x] > 0, "{key}: a triangle's centre ({m}) outside the icon");
        }
    }
    // The pistol's hole is no triangle's.
    let tris = icons::tessellate(set.get("pistol").unwrap());
    let hole = bevy::math::Vec2::new(13.0, 10.0);
    for [a, b, c] in &tris {
        let d = |p: bevy::math::Vec2, q: bevy::math::Vec2| (q - p).perp_dot(hole - p);
        let (d1, d2, d3) = (d(*a, *b), d(*b, *c), d(*c, *a));
        let inside = (d1 > 0.0 && d2 > 0.0 && d3 > 0.0) || (d1 < 0.0 && d2 < 0.0 && d3 < 0.0);
        assert!(!inside, "the trigger guard's hole is covered");
    }
}

/// Which icon: classes map to the set, melee by slot; the game's own
/// icon wherever it has one when the theme prefers it, ours otherwise.
#[test]
fn icon_rules() {
    assert_eq!(IconFor::for_class("heavy"), "lmg");
    assert_eq!(IconFor::for_class("carbine"), "rifle");
    assert_eq!(IconFor::for_class("heavy_pistol"), "revolver");
    assert_eq!(IconFor::for_class("marksman"), "sniper");
    assert_eq!(IconFor::for_class("flamethrower"), "launcher");
    assert_eq!(IconFor::for_class("shotgun"), "shotgun");
    assert_eq!(IconFor::gun_class(chimera_core::game_parts::GunClass::Heavy), "lmg");
    assert_eq!(IconFor::for_melee(true), "knife");
    assert_eq!(IconFor::for_melee(false), "blunt");
    assert_eq!(icons::choose(true, true), IconSource::Game);
    assert_eq!(icons::choose(true, false), IconSource::Fallback);
    assert_eq!(icons::choose(false, true), IconSource::Fallback);
    let signal = chimera_ui::Theme::from_toml(include_str!("../../../../apps/ultimate_gun_game/data/theme.toml")).unwrap();
    assert!(signal.icons.prefer_game);
}

/// The ping meter's levels (Look Book V): quiet in light green at 70 %,
/// fine, amber, bad; bots none. Thresholds are the theme's.
#[test]
fn ping_levels() {
    use chimera_ui::{Tone, widgets::PingLevel};
    let t = chimera_ui::Theme::from_toml(include_str!("../../../../apps/ultimate_gun_game/data/theme.toml")).unwrap();
    assert_eq!(PingLevel::from_ms(&t, 12), PingLevel { lit: 4, tone: Tone::Good, alpha: 0.7 });
    assert_eq!(PingLevel::from_ms(&t, 72), PingLevel { lit: 3, tone: Tone::Fg, alpha: 1.0 });
    assert_eq!(PingLevel::from_ms(&t, 140).tone, Tone::Warn);
    assert_eq!(PingLevel::from_ms(&t, 240), PingLevel { lit: 1, tone: Tone::Threat, alpha: 1.0 });
    assert_eq!(PingLevel::BOT.lit, 0);
    assert_eq!(Srgba::from(t.tone(Tone::Warn)).to_u8_array(), [255, 181, 71, 255]);
    assert_eq!(Srgba::from(t.tone(Tone::Good)).to_u8_array(), [155, 227, 180, 255]);
}
