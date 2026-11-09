//! CS:S's own front end, drawn from the player's install: the GameUI main
//! and Esc menus and their dialogs (`game_menu`, `options`, `first_run`,
//! `server_browser`), the join flow (`join_menus`: the message of the day,
//! team and class menus), the buy, team and radio menus, the game HUD
//! (`objectives_hud`, `bomb_fx`, `target_id`, `voice_hud`, the automatic
//! hints: `hints`; a playing minigame's panel, `minigame_hud`, for an app
//! with minigames), the spectator menu and freeze cam panel, the vote panels
//! (`vote_hud`), the
//! scoreboard, the map logic's debug overlays (`debug_world`: sound radii,
//! triggers), the window icon and `playdemo` (`demo`). Built on the client
//! shell (`client`: input, console, fonts, the VGUI-like widgets); the
//! shell reaches these only through `client::front_end`'s hooks, and the
//! app adds them (`CsSourceUiPlugin`). The simulation side of `cs_source`
//! never uses this module.

pub mod bomb_fx;
pub mod buy_menu;
pub mod debug;
pub mod debug_world;
pub mod demo;
pub mod freeze_cam;
pub mod game_hud;
pub mod hints;
pub mod hud_sprites;
pub mod join_menus;
pub mod minigame_hud;
pub mod objectives_hud;
pub mod overview;
pub mod party;
pub mod radar;
pub mod radio;
pub mod scoreboard;
pub mod spectator_hud;
pub mod spectator_menu;
pub mod target_id;
pub mod team_menu;
pub mod textures;
pub mod voice_hud;
pub mod vote_hud;
pub mod weapon_select;
pub mod win_panel;
pub mod window_icon;

use bevy::{app::PluginGroupBuilder, prelude::*};
/// GameUI's menus and dialogs (Source's, shared with HL2:DM).
pub use source_gameui::{first_run, game_menu, options, server_browser};
use source_gameui::{BotMenu, MenuGame, QuickStart};

use crate::client;

/// What's CS:S's own in GameUI's menus: its bots, teams, rounds and quick
/// start.
pub static MENU_GAME: MenuGame = MenuGame {
    info: &crate::game::INFO,
    name: "Counter-Strike: Source",
    steam_folder: "Counter-Strike Source",
    strings: crate::game::gameui::STRINGS,
    bots: Some(BotMenu {
        difficulties: &crate::bot::DIFFICULTIES,
        teams: [("", "Any"), ("#Cstrike_Team_T", "Terrorists"), ("#Cstrike_Team_CT", "Counter-Terrorists")],
        cvars: [
            ("BotPrefixEntry", "bot_prefix", "#CStrike_Bot_NamePrefix"),
            ("BotJoinAfterPlayerCheck", "bot_join_after_player", "#CStrike_Bot_JoinAfterPlayer"),
        ],
        include_token: "#Cstrike_Bot_IncludeBots",
        count_token: "#Cstrike_Bot_NumberOfBots",
        difficulty_token: "#Cstrike_Bot_Difficulty",
        join_team_token: "#CStrike_Bot_JoinTeam",
        team_words: ["terrorist", "counter-terrorist"],
    }),
    rounds_cvar: Some("chimera_rounds"),
    teams: Some(["Terrorists", "Counter-Terrorists"]),
    quick_start: Some(QuickStart { map: "de_dust2", bots: 9 }),
    layout_hides: false,
    player_models: false,
    sprays: false,
    server_types: false,
};

/// The client shell's plugins (`client::ClientPlugins`) with CS:S's HUD
/// among them, each where it was when the HUD was the shell's (system
/// order kept): the HUD panels and sprites after the fallback HUD, the
/// radar after the map's HUD text, the weapon selection after the camera
/// views, the spectator's HUD after spectating, the overview after the
/// gamma, the bots' debug views after the shell's.
pub fn with_hud(shell: PluginGroupBuilder) -> PluginGroupBuilder {
    shell
        .add_after::<client::debug_views::DebugViewsPlugin>(debug::BotDebugPlugin)
        .add_after::<client::debug_views::DebugViewsPlugin>(debug_world::WorldDebugPlugin)
        .add_after::<client::hud::HudPlugin>(game_hud::GameHudPlugin)
        .add_after::<game_hud::GameHudPlugin>(hud_sprites::HudSpritesPlugin)
        .add_after::<client::game_text::GameTextPlugin>(radar::RadarPlugin)
        .add_after::<client::view::ViewPlugin>(weapon_select::WeaponSelectPlugin)
        .add_after::<client::spectate::SpectatePlugin>(spectator_hud::SpectatorHudPlugin)
        .add_after::<client::gamma::GammaPlugin>(overview::OverviewPlugin)
}

/// CS:S's menus, panels and HUD for a windowed game (after
/// `client::ClientPlugin`).
pub struct CsSourceUiPlugin;

impl Plugin for CsSourceUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            scoreboard::ScoreboardPlugin,
            buy_menu::BuyMenuPlugin,
            team_menu::TeamMenuPlugin,
            radio::RadioPlugin,
            objectives_hud::ObjectivesHudPlugin,
            bomb_fx::BombFxPlugin,
            game_menu::GameMenuPlugin(&MENU_GAME),
            window_icon::WindowIconPlugin,
            server_browser::ServerBrowserPlugin,
            first_run::FirstRunPlugin(&MENU_GAME),
            target_id::TargetIdPlugin,
            join_menus::JoinMenusPlugin,
            spectator_menu::SpectatorMenuPlugin,
            freeze_cam::FreezeCamPlugin,
            demo::DemoUiPlugin,
        ));
        app.add_plugins((
            voice_hud::VoiceHudPlugin,
            vote_hud::VoteHudPlugin,
            hints::HintsHudPlugin,
            textures::CsTexturesPlugin,
        ));
    }
}

#[cfg(test)]
mod tests {
    /// Create Server's Normal is what bots start at.
    #[test]
    fn normal_difficulty_is_the_bots_default() {
        let c = crate::bot::BotConfig::default();
        let (_, r, a, t) = super::MENU_GAME.bots.as_ref().unwrap().difficulties[1];
        assert_eq!((r, a, t), (c.reaction, c.aim_error, c.turn_rate));
    }

    /// The options' two cvars CS:S's own panels register, as the shared
    /// ones (`source_gameui::options`): saved, at CS:S's defaults.
    #[test]
    fn the_options_own_cvars_of_css_are_saved() {
        let mut app = bevy::prelude::App::new();
        app.add_plugins(crate::console::ConsolePlugin);
        super::radar::radar_cvars(&mut app);
        super::objectives_hud::c4_bar_cvar(&mut app);
        for (name, default) in [("cl_radar_locked", "0"), ("cl_c4progressbar", "1")] {
            let c = app.world().resource::<crate::console::Console>().cvar(name).cloned().unwrap();
            assert!(c.archive && c.default == default, "{name}");
        }
    }

    use crate::{
        client::binds::{self, cfg_binds, defaults_from},
        game::{DEFAULT_BINDS, INFO},
    };

    /// CS:S's `cfg/config_default.cfg` as it reads (its bind lines; the
    /// file itself is the install's and never in the repo): the built-in
    /// table must match it.
    const SAMPLE_CONFIG_DEFAULT: &str = r#"// first run
unbindall

bind "ESCAPE" "cancelselect"
bind "`" "toggleconsole"

bind "TAB" "+showscores"
bind "SPACE" "+jump"
bind "," "buyammo1"
bind "." "buyammo2"
bind "0" "slot10"
bind "1" "slot1"
bind "2" "slot2"
bind "3" "slot3"
bind "4" "slot4"
bind "5" "slot5"
bind "6" "slot6"
bind "7" "slot7"
bind "8" "slot8"
bind "9" "slot9"
bind "a" "+moveleft"
bind "b" "buymenu"
bind "c" "radio3"
bind "d" "+moveright"
bind "e" "+use"
bind "f" "impulse 100"
bind "g" "drop"
bind "h" "commandmenu"
bind "i" "showbriefing"
bind "j" "cheer"
bind "k" "+voicerecord"
bind "m" "chooseteam"
bind "n" "nightvision"
bind "o" "buyequip"
bind "q" "lastinv"
bind "r" "+reload"
bind "s" "+back"
bind "t" "impulse 201"
bind "u" "messagemode2"
bind "w" "+forward"
bind "x" "radio2"
bind "y" "messagemode"
bind "z" "radio1"
bind "CTRL" "+duck"
bind "SHIFT" "+speed"
bind "F1" "autobuy"
bind "F2" "rebuy"
bind "F4" "bug"
bind "F5" "jpeg"
bind "F6" "save quick"
bind "F7" "load quick"
bind "F10" "quit prompt"
bind "MWHEELDOWN" "invnext"
bind "MWHEELUP" "invprev"
bind "MOUSE1" "+attack"
bind "MOUSE2" "+attack2"
bind "PAUSE" "pause"
"#;

    fn owned(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn the_default_binds_are_config_default_cfgs() {
        // The built-in table is the file's binds, in its order (Esc and `
        // aren't binds).
        assert_eq!(cfg_binds(SAMPLE_CONFIG_DEFAULT), owned(DEFAULT_BINDS));
        // Only with CS:S installed (CHIMERA_CONFIG or a Steam library): the
        // install's file says the same.
        let mut d = binds::BindDefaults::default();
        if d.read_install(Some(&INFO)) {
            assert_eq!(d.default_binds(), defaults_from(&owned(DEFAULT_BINDS)));
        }
        // A config from before the defaults followed the file gets those
        // added since (CS:S's `OLD_DEFAULT_BINDS`), on free keys.
        let added = d.added_defaults();
        assert!(added.contains(&("f1".into(), "autobuy".into())), "{added:?}");
        assert!(added.contains(&("f11".into(), "chimera_inspector".into())), "{added:?}");
        assert!(!added.iter().any(|(k, _)| k == "w" || k == "alt"), "{added:?}");
    }

    /// Bind migration (architecture review X5): the commands polled before
    /// `POLLED` came from the games are still polled for CS:S, so a saved
    /// config.cfg (`bind b buymenu`, `bind z radio1`) and the default table
    /// run exactly what they ran.
    #[test]
    fn saved_binds_keep_their_polled_commands() {
        use bevy::prelude::App;
        use crate::client::binds::PolledBinds;
        const POLLED_BEFORE: &[&str] = &[
            "+forward", "+back", "+moveleft", "+moveright", "+jump", "+duck", "+speed", "+walk", "+sprint", "+use",
            "+reload", "+attack", "+attack2", "+freelook", "+showscores", "slot1", "slot2", "slot3", "slot4", "slot5",
            "lastinv", "drop", "buymenu", "chooseteam", "radio1", "radio2", "radio3", "messagemode", "messagemode2",
            "bug", "bugreport",
        ];
        let mut app = App::new();
        app.add_plugins((
            super::buy_menu::BuyMenuPlugin,
            super::team_menu::TeamMenuPlugin,
            super::radio::RadioPlugin,
        ));
        let polled = app.world().resource::<PolledBinds>();
        for c in POLLED_BEFORE {
            assert!(polled.is_polled(c), "{c} is no longer polled");
        }
        assert_eq!(polled.all().count(), POLLED_BEFORE.len(), "a command was added to the polled set");
        // An old config's lines, as saved: the polled ones run nothing.
        let saved = cfg_binds("bind b buymenu\nbind z radio1\nbind q \"+reload; lastinv\"\nbind m \"chooseteam; say hi\"\n");
        let run: Vec<_> = saved.iter().map(|(_, l)| polled.runnable(l)).collect();
        assert_eq!(run, [None, None, None, Some("say hi".into())]);
        // And every default is polled exactly as before.
        for (_, line) in DEFAULT_BINDS {
            let was = binds::parts(line).iter().all(|p| p.is_empty() || POLLED_BEFORE.contains(&p.to_lowercase().as_str()));
            assert_eq!(polled.runnable(line).is_none(), was, "{line}");
        }
    }
}
