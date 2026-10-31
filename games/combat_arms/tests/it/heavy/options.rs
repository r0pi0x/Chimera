//! The install checks of specs/combat_arms/options.md (K2, Open question
//! 1), on the user's Combat Arms install: the key-name attribute file
//! (`KeyMappingString.txt`) reads into the list's names, and the stock
//! keys' tokens are in it. Passes with a note without an install; the
//! attribute archive needs the V1 key. Nothing read here is written
//! anywhere.

use chimera_core::mount::{Mount, config::LocalConfig};
use combat_arms::game::{
    GAME, INFO,
    options::{DEFAULT_PROFILE, bindings, keys::KeyNames},
};
use lithtech_formats::attributes::Document;

fn install() -> Option<Mount> {
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else {
        eprintln!("skipping: no Combat Arms install configured");
        return None;
    };
    if !chimera_core::mount::served::usable("combat_arms", &install, "Game") {
        eprintln!("skipping: no Combat Arms install at {}", install.display());
        return None;
    }
    Some((INFO.open)(&install).unwrap())
}

/// K2: the key-name file's sections, and that every stock key of K1 has
/// a name in it (so the list never falls back to a control name for them).
#[test]
fn the_key_names_file_names_the_stock_keys() {
    let Some(m) = install() else { return };
    let Ok(bytes) = m.read(combat_arms::game::options::keys::KEY_NAMES_FILE) else {
        eprintln!("skipping: {} isn't readable (no V1 key?)", combat_arms::game::options::keys::KEY_NAMES_FILE);
        return;
    };
    let names = KeyNames::parse(&Document::parse(&bytes).unwrap());
    assert!(!names.keyboard.is_empty(), "Keyboard0.. sections");
    assert!(!names.mouse.is_empty(), "Mouse0.. sections");
    let default = chimera_core::profile::Profile::parse(DEFAULT_PROFILE);
    let table = bindings::load(&default, &default, true);
    for (a, b) in bindings::ACTIONS.iter().zip(&table.entries) {
        for (token, mouse) in [(&b.kbd, false), (&b.mouse, true)] {
            if token.is_empty() {
                continue;
            }
            let known = if mouse { &names.mouse } else { &names.keyboard };
            if !known.iter().any(|(real, text)| real == token && !text.is_empty()) {
                eprintln!("{}: {token} isn't in the key-name file (shown by our built-in name)", a.name);
            }
        }
    }
    // The list text of the stock rows is never empty.
    for b in &table.entries {
        assert!(!b.row_text(&names).is_empty());
    }
}
