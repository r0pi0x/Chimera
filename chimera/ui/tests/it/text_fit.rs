//! Text cut to a width (`chimera_ui::text_fit`): a text that fits stays
//! whole; a longer one keeps its longest start that fits with `…` after
//! it, trailing spaces dropped; nothing fits: the ellipsis alone; a face
//! that isn't the kit's is measured wide (0.62 em a character).

use bevy::{prelude::*, text::{FontSize, LetterSpacing}};
use chimera_ui::{KitFonts, ellipsize, text_fit::ELLIPSIS};

/// 10 px a character (the ellipsis too).
fn mono(t: &str) -> f32 {
    t.chars().count() as f32 * 10.0
}

#[test]
fn a_text_is_cut_to_its_width_with_an_ellipsis() {
    assert_eq!(ellipsize("ANN", 30.0, mono), "ANN");
    assert_eq!(ellipsize("WWWWWWWWWWWWWWWWWWWW", 100.0, mono), "WWWWWWWWW\u{2026}");
    assert_eq!(ellipsize("SGT. BISCUIT", 60.0, mono), "SGT.\u{2026}", "the space before the cut goes");
    assert_eq!(ellipsize("WWW", 5.0, mono), ELLIPSIS);
    for max in [10.0, 35.0, 99.0, 150.0, 199.0] {
        let out = ellipsize("WWWWWWWWWWWWWWWWWWWW", max, mono);
        assert!(mono(&out) <= max.max(10.0), "{max}: {out}");
    }
}

#[test]
fn a_face_not_the_kits_is_measured_wide() {
    let font = TextFont { font_size: FontSize::Px(20.0), ..default() };
    let w = KitFonts::default().width(&font, Some(&LetterSpacing::Px(1.0)), "ABCDE");
    assert!((w - 5.0 * (20.0 * 0.62 + 1.0)).abs() < 1e-3, "{w}");
}
