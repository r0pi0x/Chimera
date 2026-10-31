//! The install's string table behind the radio menu and the key names
//! (specs/combat_arms/radio_chat.md R1, options.md K2, ui_system.md R1):
//! the radio file's list texts and the key-name file's trigger names are
//! string ids (`$RadioMsg_Report_list_4`, `$Keyboard_Name_4`); the table
//! `language_pack/language_ENG.txt` resolves them, so the screens never
//! show a raw `$` id with the install present. V1-keyed: skipped without
//! the key or an install.

use chimera_core::mount::{Mount, config::LocalConfig};
use combat_arms::{
    game::{
        GAME, INFO,
        chat::radio::{self, RadioData},
        mount,
        options::keys::{self, KeyNames},
    },
    ui::{install, talk},
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
    if !mount::can_read("V1") {
        eprintln!("skipping: no V1 key in chimera.local.toml");
        return None;
    }
    Some((INFO.open)(&install).unwrap())
}

/// Every radio entry's list text resolves in the install's table.
#[test]
fn the_radio_menus_texts_are_in_the_string_table() {
    let Some(m) = install() else { return };
    let strings = install::strings(&m, "ENG").expect("the string table reads");
    let Ok(bytes) = m.read(radio::FILE) else {
        eprintln!("skipping: {} isn't readable", radio::FILE);
        return;
    };
    let data = RadioData::read(&Document::parse(&bytes).unwrap());
    let mut ids = 0;
    for class in &data.classes {
        for e in class.entries.iter().take(9) {
            let shown = talk::shown_id(&strings, &e.list_text, "?");
            assert!(!shown.starts_with('$') && shown != "?", "{}: {} didn't resolve", class.name, e.list_text);
            if e.list_text.starts_with('$') {
                ids += 1;
            }
        }
    }
    eprintln!("{ids} radio list texts are string ids, all resolved");
}

/// Every radio entry's chat line is a string id the table resolves (the
/// chat shows the text, not `$radio_message_...`).
#[test]
fn the_radio_chat_lines_are_in_the_string_table() {
    let Some(m) = install() else { return };
    let strings = install::strings(&m, "ENG").expect("the string table reads");
    let Ok(bytes) = m.read(radio::FILE) else {
        eprintln!("skipping: {} isn't readable", radio::FILE);
        return;
    };
    let data = RadioData::read(&Document::parse(&bytes).unwrap());
    let (mut ids, mut missing) = (0, Vec::new());
    for class in &data.classes {
        for e in &class.entries {
            for c in e.chat.iter().filter(|c| c.starts_with('$')) {
                ids += 1;
                if strings.resolve(c).starts_with('$') {
                    missing.push(c.clone());
                }
            }
        }
    }
    eprintln!("{ids} radio chat lines are string ids, {} missing from the table", missing.len());
    assert!(missing.is_empty(), "not in the table: {missing:?}");
}

/// Every key name of the install's file resolves in its table.
#[test]
fn the_key_names_are_in_the_string_table() {
    let Some(m) = install() else { return };
    let strings = install::strings(&m, "ENG").expect("the string table reads");
    let Ok(bytes) = m.read(keys::KEY_NAMES_FILE) else {
        eprintln!("skipping: {} isn't readable", keys::KEY_NAMES_FILE);
        return;
    };
    let names = KeyNames::parse(&Document::parse(&bytes).unwrap()).resolved(|id| strings.resolve(id));
    for (real, text) in names.keyboard.iter().chain(&names.mouse) {
        assert!(!text.is_empty(), "{real} has no resolved name");
    }
}
