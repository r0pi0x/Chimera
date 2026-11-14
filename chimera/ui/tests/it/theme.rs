//! Theme tokens (chimera_ui::theme).

use bevy::prelude::*;
use chimera_ui::{Ease, Theme, Tone};

/// The Ultimate Gun Game's theme (read here as data; the app embeds it).
const SIGNAL: &str = include_str!("../../../../apps/ultimate_gun_game/data/theme.toml");

fn srgba(c: Color) -> [u8; 4] {
    Srgba::from(c).to_u8_array()
}

/// Signal's tokens and derived colours are the look book's (Cobalt).
#[test]
fn theme_matches_the_look_book() {
    let t = Theme::from_toml(SIGNAL).expect("theme.toml parses");
    assert_eq!(t.name, "signal");
    assert_eq!(srgba(t.tone(Tone::Ink)), [6, 8, 12, 255]);
    assert_eq!(srgba(t.tone(Tone::Acc)), [59, 123, 255, 255]);
    assert_eq!(srgba(t.tone(Tone::AccText)), [106, 152, 255, 255]);
    assert_eq!(srgba(t.tone(Tone::Threat)), [255, 59, 78, 255]);
    // fg_2 is fg at .66, plate is ink at .80.
    assert_eq!(srgba(t.tone(Tone::Fg2)), [232, 237, 245, (0.66f32 * 255.0).round() as u8]);
    assert_eq!(srgba(t.tone(Tone::Plate)), [6, 8, 12, 204]);
    assert!((t.tan() - 0.2126).abs() < 1e-4, "12° slant: {}", t.tan());
    assert_eq!(t.ease("out"), Ease::OUT);
    assert_eq!(t.ease("roll"), Ease::Back(0.9));
    assert_eq!(t.dur("pass", "total"), 1.2);
    assert_eq!(t.dur("pass", "swap"), 0.6);
    // Decision 2: Tab opens in about 120 ms, closes in 90.
    assert_eq!(t.dur("board", "open"), 0.12);
    assert_eq!(t.dur("board", "close"), 0.09);
    assert_eq!(t.layout("toast", "top"), 6.8);
    assert_eq!(t.layout("chip", "w"), 21.0);
    // Every role, the wordmark with ss02.
    let w = t.type_spec("wordmark").unwrap();
    assert_eq!((w.face.as_str(), w.weight), ("display", 900));
    assert_eq!(w.features, vec!["ss02".to_string()]);
    for role in chimera_ui::Role::ALL {
        assert!(t.type_spec(role.key()).is_some(), "{role:?}");
    }
}

/// The default theme is complete on its own, and a theme laid over it
/// keeps what it doesn't name.
#[test]
fn shell_default_is_complete() {
    let s = Theme::shell_default();
    for tone in Tone::ALL {
        let _ = s.tone(tone);
    }
    assert_eq!(s.type_spec("display_bold").unwrap().face, "trebuchet ms");
    let t = Theme::from_toml("name = \"x\"\n[palette]\nacc = \"#FF0000\"\n").unwrap();
    assert_eq!(srgba(t.tone(Tone::Acc)), [255, 0, 0, 255]);
    assert_eq!(t.tone(Tone::Fg), s.tone(Tone::Fg));
    assert!(Theme::from_toml("[palette]\nacc = \"nope\"\n").is_err());
    assert!(Theme::from_toml("[type.mono]\nface = \"mono\"\nfeatures = [\"toolong\"]\n").is_err());
}
