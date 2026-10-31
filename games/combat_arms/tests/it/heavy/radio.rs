//! The install's radio file (specs/combat_arms/radio_chat.md R1, Open
//! question 1): `RadioMsgSound` in the attributes archive, read through
//! our own `.rez` reader and the radio's reader. V1-keyed: skipped
//! without the key or an install. The file's name is a guess
//! (`radio::FILE`); when it isn't there the test says so and passes, and
//! the game keeps using its built-in menus.

use chimera_core::mount::{Mount, config::LocalConfig};
use combat_arms::game::{
    GAME, INFO,
    chat::radio::{self, RadioData},
    mount,
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

/// The install's radio file parses and has the menus the spec describes:
/// voice sets, the report, answer and request classes, and entries with
/// chat lines.
#[test]
fn the_installs_radio_file_reads() {
    let Some(mount) = install() else { return };
    let bytes = match mount.read(radio::FILE) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("the radio file isn't at {} ({e}): the guessed name needs checking", radio::FILE);
            return;
        }
    };
    let doc = Document::parse(&bytes).expect("the radio file parses");
    let data = RadioData::read(&doc);
    println!(
        "radio: {} voice sets, classes {:?}",
        data.templates.len(),
        data.classes.iter().map(|c| (c.name.as_str(), c.entries.len())).collect::<Vec<_>>()
    );
    assert!(!data.templates.is_empty(), "voice sets");
    assert!(data.classes.len() >= 3, "report, answer and request");
    for class in &data.classes[..3] {
        assert!(!class.entries.is_empty(), "{} has entries", class.name);
        assert!(class.entries.iter().all(|e| !e.chat.is_empty() || e.list_text.is_empty()), "{}: chat lines", class.name);
    }
}
