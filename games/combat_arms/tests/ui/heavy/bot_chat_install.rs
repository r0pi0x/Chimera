//! Bots' chat lines and the weapons' display names from the user's
//! install (bots.md 10, attribute_data.md `$` strings): every line a bot
//! can say shows as text, never a `$` id, and the install's weapons with
//! a display name get it in the HUD's `ItemNames`. V1-keyed: skipped
//! without the key or an install.

use chimera_core::mount::{Mount, config::LocalConfig};
use combat_arms::{
    game::{GAME, INFO, bots::chat::shown_line, mount},
    ui::install,
};

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

const EVENTS: [&str; 9] = ["Kill", "Death", "Respawn", "Battle", "Search", "Grenade", "Mine", "Conceal", "Escape"];

/// Every line of every chat set and event shows as text with the install's
/// table, the ones it defines as its own words.
#[test]
fn every_bot_chat_line_resolves_without_a_dollar() {
    let Some(m) = install() else { return };
    let strings = install::strings(&m, "ENG").expect("the string table reads");
    let (mut own, mut ours) = (0, 0);
    for set in 0..3 {
        for event in EVENTS {
            for n in 0..10 {
                let key = format!("$Message{set}_{event}_{n}");
                let text = shown_line(|k| strings.resolve(k), &key);
                assert!(!text.is_empty() && !text.contains('$'), "{key}: {text:?}");
                if strings.resolve(&key) == key {
                    ours += 1;
                } else {
                    assert_eq!(text, strings.resolve(&key), "{key} shows the install's text");
                    own += 1;
                }
            }
        }
    }
    eprintln!("{own} bot chat lines are the install's, {ours} fall back");
    assert!(own > 0, "the install defines some chat lines");
}

/// The install's weapons with a display name get it by our id.
#[test]
fn the_installs_weapons_have_their_display_names() {
    let Some(m) = install() else { return };
    let strings = install::strings(&m, "ENG").expect("the string table reads");
    let names = install::weapon_names(&m, &strings);
    eprintln!("{} weapons are named", names.len());
    assert!(names.len() > 100, "{} names", names.len());
    for (id, name) in &names {
        assert!(id.starts_with("combat_arms:") && !name.is_empty() && !name.starts_with('$'), "{id}: {name}");
    }
    let name_of = |id: &str| names.iter().find(|(i, _)| i == id).map(|(_, n)| n.as_str());
    assert_eq!(name_of("combat_arms:beretta_m92fs"), Some("M92FS"));
    assert_eq!(name_of("combat_arms:m9"), Some("M9"));
}
