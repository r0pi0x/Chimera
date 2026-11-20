//! Every character the Signal screens draw is in our faces: a missing one
//! shows as a box (the Maps picker's fold arrows did, revision 8). The
//! string literals of the kit and its screens' source files are checked
//! against UGG Display Regular's and UGG Mono's cmaps (`fonts::has_glyph`);
//! what isn't in both faces must be drawn as a kit icon instead.

use chimera_ui::fonts::{OWN, has_glyph};

/// The files whose literals are drawn in the UGG faces (the kit, the
/// shell's screens, the party apps' Signal screens).
const FILES: &[&str] = &[
    "chimera/ui/src/shell.rs",
    "chimera/ui/src/controls.rs",
    "chimera/ui/src/avatar.rs",
    "chimera/ui/src/picker.rs",
    "chimera/client/src/client/lobby_panel.rs",
    "chimera/client/src/client/prematch_lobby.rs",
    "apps/party_kit/src/signal_menu.rs",
    "apps/party_kit/src/signal_pages.rs",
    "apps/party_kit/src/settings.rs",
    "apps/party_kit/src/maps_picker.rs",
    "apps/party_kit/src/loading_signal.rs",
    "apps/party_kit/src/esc_signal.rs",
    "apps/party_kit/data/main_menu.toml",
    "apps/party_kit/data/options.toml",
];

/// The string literals of a Rust or TOML source, comments left out.
fn literals(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in src.lines() {
        let code = line.trim_start();
        if code.starts_with("//") || code.starts_with('#') {
            continue;
        }
        let mut cur: Option<String> = None;
        let mut chars = line.chars().peekable();
        while let Some(ch) = chars.next() {
            match (&mut cur, ch) {
                (None, '"') => cur = Some(String::new()),
                (Some(s), '\\') => match chars.next() {
                    // `\u{2022}`: the character itself.
                    Some('u') if chars.peek() == Some(&'{') => {
                        chars.next();
                        let hex: String = chars.by_ref().take_while(|c| *c != '}').collect();
                        if let Some(c) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                            s.push(c);
                        }
                    }
                    Some('n' | 't' | 'r') => s.push(' '),
                    Some(c) => s.push(c),
                    None => {}
                },
                (Some(s), '"') => {
                    out.push(std::mem::take(s));
                    cur = None;
                }
                (Some(s), c) => s.push(c),
                (None, _) => {}
            }
        }
    }
    out
}

#[test]
fn every_drawn_character_is_in_our_faces() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let mut missing = Vec::new();
    for f in FILES {
        let src = std::fs::read_to_string(format!("{root}/{f}")).unwrap_or_else(|e| panic!("{f}: {e}"));
        for lit in literals(&src) {
            for ch in lit.chars() {
                // Rust's escapes (`\u{..}`) are read as their letters.
                if !(has_glyph(OWN.display_regular, ch) && has_glyph(OWN.mono, ch)) {
                    missing.push(format!("{f}: {ch:?} (U+{:04X}) in {lit:?}", ch as u32));
                }
            }
        }
    }
    assert!(missing.is_empty(), "characters our faces lack (draw a kit icon instead):\n{}", missing.join("\n"));
}

#[test]
fn the_faces_have_the_shells_characters() {
    for ch in "QE•·×–—°ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789:/%".chars() {
        assert!(has_glyph(OWN.display_regular, ch) && has_glyph(OWN.mono, ch), "{ch:?}");
    }
    // The check sees a box coming: the picker's old fold arrows.
    for ch in ['\u{25be}'] {
        assert!(!has_glyph(OWN.mono, ch), "{ch:?}: drawn as the kit's chevron icons");
    }
}
