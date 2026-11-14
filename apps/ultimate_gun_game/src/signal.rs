//! The Ultimate Gun Game's look, "Signal" (feature `present`;
//! docs/plans/active/ugg-ui.md): its theme (`THEME`, data/theme.toml) and
//! its own faces (`FONTS`, fonts/: UGG Display Black, Bold, Regular and
//! UGG Mono, OFL-1.1, ours) embedded in the binary, so nothing is read
//! from disk on Linux or Windows, offscreen or headless. `SignalPlugin`
//! (added last, over the shell's `chimera_ui::ChimeraUiPlugin`) makes
//! the faces into fonts (`chimera_ui::KitFonts`) and puts the theme in
//! place of the shell's default.

use bevy::prelude::*;
use chimera_ui::{FontFiles, KitFonts, Theme};

/// The theme's TOML (over the shell's defaults: `Theme::from_toml`).
pub const THEME: &str = include_str!("../data/theme.toml");

/// The four faces, v0.4 (fonts/FONTLOG.txt).
pub const FONTS: FontFiles<'static> = FontFiles {
    display_black: include_bytes!("../fonts/ugg-display-black.ttf"),
    display_bold: include_bytes!("../fonts/ugg-display-bold.ttf"),
    display_regular: include_bytes!("../fonts/ugg-display-regular.ttf"),
    mono: include_bytes!("../fonts/ugg-mono-regular.ttf"),
};

/// Signal's theme and faces (module doc).
pub struct SignalPlugin;

impl Plugin for SignalPlugin {
    fn build(&self, app: &mut App) {
        let theme = Theme::from_toml(THEME).expect("apps/ultimate_gun_game/data/theme.toml is a valid theme");
        app.insert_resource(theme);
        if let Some(mut fonts) = app.world_mut().get_resource_mut::<Assets<Font>>() {
            let kit = KitFonts::load(&mut fonts, FONTS);
            app.insert_resource(kit);
        }
    }
}
