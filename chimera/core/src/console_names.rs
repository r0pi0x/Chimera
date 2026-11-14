//! Which namespace a Source console name belongs to
//! (docs/plans/active/console-namespaces.md, "Engine and game names"; user
//! decision of 2026-11-01): the one table that sorts the names a plugin
//! registers without a namespace, and the full names saved before the
//! table existed.
//!
//! - **Engine names** (`ENGINE_NAMES`, `ENGINE_PREFIXES`) are what
//!   Chimera's engine implements for any Source game, in Source's
//!   spelling: loading maps and connecting (`map`, `connect`, `quit`,
//!   `changelevel`, `kick`), video, audio, network and input settings
//!   (`fps_max`, `r_*`, `snd_*`, `rate`, `cl_interp`, `sensitivity`,
//!   `+attack`), the console and its tools, demos, screenshots, chat, the
//!   voting, the spectator commands, and the cheats the engine implements
//!   (`noclip`, `god`, `setpos`). They are `chimera:`, shared by every game.
//! - **Game names** are everything else a bare name can be: a game's own
//!   rules and values (`mp_*`, `bot_*`, the buy menu, the bomb, CS:S's HUD
//!   and crosshair), and the physics and movement values each Source game
//!   tunes on its own (`sv_gravity`, `sv_maxspeed`, `sv_accelerate`,
//!   `phys_*`, ...), so a second game can have `hl2dm:sv_gravity`. They stay
//!   `cs_source:` (`SOURCE_NAMESPACE`) while CS:S is the only Source game.
//!
//! The old full names still work: `cs_source:fps_max` (what configs, binds
//! and scripts saved before the move say) is `chimera:fps_max`
//! (`Console::resolve`, `canonical_full`), for exactly the names the table
//! lists. Tests: `console::tests`, `net::cheats`.

/// Engine names, exactly (lower case): commands and cvars.
pub const ENGINE_NAMES: &[&str] = &[
    // Maps, connection, the server and its operators.
    "map",
    "maps",
    "changelevel",
    "quit",
    "exit",
    "connect",
    "disconnect",
    "listen",
    "status",
    "users",
    "ping",
    "version",
    "kick",
    "kickid",
    "kill",
    "hostname",
    "hostport",
    "ip",
    "maxplayers",
    "password",
    "sv_password",
    "sv_cheats",
    "sv_logfile",
    "sv_logecho",
    "sv_allow_votes",
    "sv_allowdownload",
    "sv_downloadurl",
    "sv_voiceenable",
    "callvote",
    "vote",
    // The pre-match lobby (`chimera_net::prematch`): the engine's, any game's.
    "mp_lobby",
    "mp_lobby_countdown",
    "mp_lobby_max_wait",
    "mp_lobby_min_players",
    "mp_lobby_ready_up",
    "lobby_start",
    "lobby_ready",
    "lobby",
    // The network: rates, interpolation, lag compensation, downloads.
    "rate",
    "sv_maxrate",
    "sv_minrate",
    "sv_maxupdaterate",
    "sv_minupdaterate",
    "sv_unlag",
    "sv_maxunlag",
    "sv_showlagcompensation",
    "cl_updaterate",
    "cl_interp",
    "cl_interp_ratio",
    "cl_interpolate",
    "cl_extrapolate",
    "cl_extrapolate_amount",
    "cl_lagcompensation",
    "cl_smoothtime",
    "cl_downloadfilter",
    "cl_autoreconnect",
    // Video, audio and the camera.
    "fps_max",
    "fov_desired",
    "fog_enable",
    "cc_subtitles",
    "closecaption",
    "volume",
    "musicvolume",
    "violence_hblood",
    "cl_ragdoll_physics_enable",
    "cam_idealdist",
    "cam_idealyaw",
    "firstperson",
    "thirdperson",
    // Sprays and decals (the engine's).
    "decalfrequency",
    "mp_decals",
    "cl_logofile",
    // Input, chat, weapon switching.
    "sensitivity",
    "zoom_sensitivity_ratio",
    "name",
    "cl_autowepswitch",
    "say",
    "say_team",
    "messagemode",
    "messagemode2",
    "drop",
    "use",
    "invnext",
    "invprev",
    "lastinv",
    "slot1",
    "slot2",
    "slot3",
    "slot4",
    "slot5",
    "slot6",
    "slot7",
    "slot8",
    "slot9",
    "slot10",
    "spectate",
    "spec_mode",
    "spec_next",
    "spec_prev",
    "spec_player",
    // The console, tools, demos, screenshots.
    "developer",
    "log",
    "toggleconsole",
    "bug",
    "bugreport",
    "screenshot",
    "jpeg",
    "host_timescale",
    "playdemo",
    "stopdemo",
    "cl_showerror",
    "cl_showfps",
    "cl_showpos",
    // Cheats the engine implements.
    "noclip",
    "god",
    "setpos",
    "setang",
    "getpos",
    "give",
    "impulse",
    "ent_fire",
    "ent_dump",
];

/// Engine names by prefix (lower case): video `r_*` and `mat_*`, audio
/// `snd_*`, `dsp_*` and `voice_*`, network `net_*`, console `con_*`, demos
/// `demo_*`, mouse `m_*`, the voting's `sv_vote_*`, and the `+`/`-` input
/// actions (`+attack`, `-jump`).
pub const ENGINE_PREFIXES: &[&str] = &[
    "r_", "mat_", "snd_", "dsp_", "voice_", "net_", "con_", "demo_", "m_", "sv_vote_", "+", "-",
];

/// Whether `short` (a name without its namespace, any case) is an engine
/// name.
pub fn is_engine(short: &str) -> bool {
    let lower = short.to_lowercase();
    ENGINE_NAMES.contains(&lower.as_str()) || ENGINE_PREFIXES.iter().any(|p| lower.starts_with(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_names_are_chimeras_and_game_values_stay_per_game() {
        for n in ["map", "quit", "fps_max", "connect", "noclip", "r_shadows", "mat_vsync", "+attack", "net_graph", "m_pitch", "mp_lobby", "mp_lobby_min_players", "lobby_start"] {
            assert!(is_engine(n), "{n}");
        }
        for n in [
            "sv_gravity", "sv_maxspeed", "sv_accelerate", "mp_roundtime", "mp_restartgame", "buy", "bot_add", "cl_crosshairsize",
            "mp_c4timer", "hud_fastswitch", "phys_pushscale", "viewmodel_fov", "sv_alltalk",
        ] {
            assert!(!is_engine(n), "{n}");
        }
    }

    #[test]
    fn the_table_is_lower_case_and_has_no_duplicates() {
        let mut seen = std::collections::BTreeSet::new();
        for n in ENGINE_NAMES {
            assert_eq!(*n, n.to_lowercase());
            assert!(seen.insert(*n), "{n} twice");
        }
    }
}
