//! CS:S's GameUI look: Source's reader (`source_engine::hud::gameui`) with
//! CS:S's strings file.

use crate::{map::hud::GameUi, mount::Mount};
pub use source_engine::hud::gameui::*;

/// CS:S's strings over GameUI's and Valve's.
pub const STRINGS: &str = "resource/cstrike_english.txt";

/// CS:S's game menu and options look from its install.
pub fn load(mount: &Mount) -> Option<GameUi> {
    source_engine::hud::gameui::load(mount, STRINGS)
}
