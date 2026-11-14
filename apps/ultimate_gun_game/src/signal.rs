//! The Ultimate Gun Game's look, "Signal" (feature `present`;
//! docs/plans/active/ugg-ui.md): its theme (`THEME`, data/theme.toml) and
//! its own faces (`FONTS`, fonts/: UGG Display Black, Bold, Regular and
//! UGG Mono, OFL-1.1, ours) embedded in the binary, so nothing is read
//! from disk on Linux or Windows, offscreen or headless. `SignalPlugin`
//! (added last, over the shell's `chimera_ui::ChimeraUiPlugin`) makes
//! the faces into fonts (`chimera_ui::KitFonts`) and puts the theme in
//! place of the shell's default, and puts in its UI sound kit
//! (`chimera_client::client::ui_sound::UiSounds`: the event table
//! `UI_SOUND_TABLE`, data/ui_sounds.toml, and the files `UI_SOUNDS`,
//! assets/sounds/ui/: ours, synthesized by tools/sounds/synth.py, CC0 1.0,
//! embedded like the faces), which the theme's `flags.sound.ui` turns on.

use bevy::prelude::*;
use chimera_client::client::ui_sound::UiSounds;
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

/// The UI sounds' event table.
pub const UI_SOUND_TABLE: &str = include_str!("../data/ui_sounds.toml");

macro_rules! ui_sounds {
    ($($name:ident,)*) => {
        &[$((stringify!($name), include_bytes!(concat!("../assets/sounds/ui/", stringify!($name), ".wav")).as_slice()),)*]
    };
}

/// The UI sounds' files (name without `.wav`, bytes): those the table names.
pub const UI_SOUNDS: &[(&str, &[u8])] = ui_sounds![
    air_alert,
    air_impact,
    air_pulse,
    count_go,
    count_tick,
    count_tick_1,
    count_tick_2,
    count_tick_3,
    end_reveal,
    feed_self,
    hud_chip,
    lobby_peer,
    lobby_ready,
    lobby_unready,
    menu_loop_scifi,
    pass_sweep,
    rank_14_knife,
    rank_chain,
    rank_p00,
    rank_p01,
    rank_p02,
    rank_p03,
    rank_p04,
    rank_p05,
    rank_p06,
    rank_p07,
    rank_p08,
    rank_p09,
    rank_p10,
    rank_p11,
    rank_p12,
    rank_p13,
    rank_p14,
    rank_p15,
    rank_p16,
    tab_close,
    tab_open,
    ui_back,
    ui_denied,
    ui_hover,
    ui_select,
    ui_slider,
    vote_cast_1,
    vote_cast_2,
    vote_cast_3,
    vote_cast_4,
    vote_lock,
    vote_other,
];

/// The kit's sounds, decoded.
pub fn ui_sounds() -> Result<UiSounds, String> {
    UiSounds::new(UI_SOUND_TABLE, UI_SOUNDS)
}

/// Signal's theme and faces (module doc).
pub struct SignalPlugin;

impl Plugin for SignalPlugin {
    fn build(&self, app: &mut App) {
        let theme = Theme::from_toml(THEME).expect("apps/ultimate_gun_game/data/theme.toml is a valid theme");
        app.insert_resource(theme);
        match ui_sounds() {
            Ok(sounds) => {
                app.insert_resource(sounds);
            }
            Err(e) => error!("{e}"),
        }
        if let Some(mut fonts) = app.world_mut().get_resource_mut::<Assets<Font>>() {
            let kit = KitFonts::load(&mut fonts, FONTS);
            app.insert_resource(kit);
        }
    }
}
