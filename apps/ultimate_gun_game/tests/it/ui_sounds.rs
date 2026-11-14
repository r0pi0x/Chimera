//! The UI sound kit as shipped (`signal::UI_SOUNDS`, data/ui_sounds.toml,
//! assets/sounds/ui/): every file decodes at 48 kHz, mono or stereo; the
//! table names a file for every event the screens send and no file goes
//! unused; the theme turns it on.

use chimera_client::client::ui_sound::{self, UiSounds};
use ultimate_gun_game::signal::{UI_SOUNDS, UI_SOUND_TABLE, ui_sounds};

/// Every event the screens send (`chimera_client::client::ui_cues`,
/// `party_kit::ui_cues`, the main menu's music); `pass_sweep` for M1's
/// logo pass.
const EVENTS: &[&str] = &[
    "ui_hover", "ui_select", "ui_back", "ui_denied", "ui_slider", "pass_sweep", "lobby_ready", "lobby_unready",
    "lobby_peer", "count_tick", "count_tick_3", "count_tick_2", "count_tick_1", "count_go", "rank_up", "rank_knife",
    "rank_chain", "hud_chip", "feed_self", "tab_open", "tab_close", "air_alert", "air_pulse", "air_impact",
    "end_reveal", "vote_cast_1", "vote_cast_2", "vote_cast_3", "vote_cast_4", "vote_other", "vote_lock", "menu_loop",
];

#[test]
fn the_shipped_kit_decodes_and_covers_every_event() {
    let kit: UiSounds = ui_sounds().expect("the kit loads");
    assert_eq!(kit.clips.len(), UI_SOUNDS.len(), "every file decodes");
    for (name, clip) in &kit.clips {
        assert_eq!(clip.rate, 48_000, "{name}");
        assert!(matches!(clip.channels, 1 | 2), "{name}");
        assert!(!clip.samples.is_empty(), "{name}");
    }
    for e in EVENTS {
        assert!(kit.table.events.contains_key(*e), "the table has no {e}");
    }
    let used = kit.table.files();
    for (name, _) in UI_SOUNDS {
        assert!(used.iter().any(|f| f == name), "{name}.wav is shipped but no event plays it");
    }
    // Off by default: Tab, your own feed line. Hover is one voice; the
    // slider's rate follows its value; the loop is music.
    for off in ["tab_open", "tab_close", "feed_self"] {
        assert!(!kit.table.events[off].on, "{off}");
    }
    assert_eq!(kit.table.events["ui_hover"].voices, 1);
    assert!(kit.table.events["ui_slider"].rate_from_value);
    assert!(kit.table.events["menu_loop"].looping);
    let theme = chimera_ui::Theme::from_toml(ultimate_gun_game::signal::THEME).unwrap();
    assert!(ui_sound::active(Some(&theme), Some(&kit)), "Signal turns it on");
    assert!(!ui_sound::active(Some(&chimera_ui::Theme::shell_default()), Some(&kit)));
    assert!(UI_SOUND_TABLE.contains("[bus]"));
}
