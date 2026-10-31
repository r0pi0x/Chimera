//! The install's own options layouts (specs/combat_arms/options.md L):
//! the five files read through the UI system's reader and the controls
//! the spec names are in them, so the options screen binds them by name.
//! V1-keyed (XML.rez): skipped without the key or an install. Nothing read
//! here is written anywhere. What is missing is printed (the spec did not
//! read the layouts, only the code that binds them).

use chimera_core::mount::{Mount, config::LocalConfig};
use combat_arms::{
    game::{GAME, INFO, mount, options::bindings},
    ui::{
        install::{self, OPTION_FILES},
        layout::{Layout, xml},
        options::{BUTTONS, DROP_TEXTS, SLIDERS, WINDOWS},
    },
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

fn layouts(m: &Mount) -> Option<Vec<Layout>> {
    OPTION_FILES
        .iter()
        .map(|f| match m.read(f) {
            Ok(b) => Some(xml::read(&String::from_utf8_lossy(&b)).unwrap_or_else(|e| panic!("{f}: {e}"))),
            Err(e) => {
                eprintln!("{f}: {e}");
                None
            }
        })
        .collect()
}

/// The frame has the buttons and tabs the spec names, and each window its
/// root.
#[test]
fn the_five_layouts_read_and_have_the_spec_named_controls() {
    let Some(m) = install() else { return };
    let Some(l) = layouts(&m) else {
        eprintln!("the options layouts aren't all there: the flat ones stay");
        return;
    };
    let find = |name: &str| l.iter().any(|x| x.find(name).is_some());
    for name in ["tab_graphic", "tab_sound", "tab_control", "tab_other", "button_ok", "button_cancel", "button_close", "button_default"] {
        assert!(l[0].find(name).is_some(), "popup_option.xml: {name}");
    }
    for (i, (win, _)) in WINDOWS.iter().enumerate() {
        assert!(l[i + 1].find(win).is_some(), "{}: {win}", OPTION_FILES[i + 1]);
    }
    let mut missing: Vec<&str> = Vec::new();
    missing.extend(BUTTONS.iter().map(|(n, _)| *n).filter(|n| !find(n)));
    missing.extend(SLIDERS.iter().flat_map(|(a, b, _)| [*a, *b]).filter(|n| !find(n)));
    missing.extend(DROP_TEXTS.iter().map(|(n, _)| *n).filter(|n| !find(n)));
    missing.extend(bindings::ACTIONS.iter().filter_map(|a| a.row).filter(|n| !find(n)));
    eprintln!("controls the screen binds that the install's layouts lack: {missing:?}");
    // The control tab's rows: all 38 (K1).
    let rows = bindings::ACTIONS.iter().filter_map(|a| a.row).filter(|n| l[3].find(n).is_some()).count();
    assert!(rows >= 30, "the Control layout has most of K1's rows: {rows}");
    assert!(install::installed(&combat_arms::game::data::CaData::install()).is_none_or(|i| i.options.is_some()));
}
