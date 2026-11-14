//! The host's side of the party's Escape menu (the simulation's, so the
//! dedicated servers have it too): its settings and the skip-map vote.
//!
//! - **Host settings** (`SETTINGS`, data/host_settings.toml): each a
//!   server cvar with a label and its choices or range, the page the
//!   Escape menu shows the host (`esc_menu`). Those with a `key` are
//!   registered here as host overrides of every minigame's definition
//!   (`chimera_sim::minigame::overrides`: the rung extras by part, demote
//!   on suicide, the knife finale, the map rotation and `avoid_recent`,
//!   spawn protection), applied when the next match starts; the others
//!   are existing cvars that apply at once (`MaxPlayer`'s bots,
//!   `party_even_footsteps`, `sv_allow_votes`).
//! - **The admin is the host**: a listen server's player (or a game with
//!   no server). A dedicated server's console sets the same cvars.
//! - **Vote to skip the map** (`SKIP_ISSUE`, `skipmap`: an issue of the
//!   engine's votes, `chimera_net::net::vote`, so the server decides
//!   everything and every client sees it): passed, the server goes to
//!   the next map of the rotation (`minigame_net::next_match`). The party's
//!   vote rules (`VOTE_RULES`, set over CS:S's defaults): a majority of
//!   the players who may vote (`sv_vote_quorum_ratio 0.51`; bots never
//!   vote, `sv_vote_bots_allowed 0`), 20 s to vote, a player may call
//!   one every 60 s and a failed one waits 60 s. `party_skipmap` calls it
//!   (alone, with no server: the map is skipped at once, the one player
//!   being the majority).
//!
//! Known shortcuts: no RCON admins (only the host sees the page); the
//! settings last for the host's process (not archived).

use bevy::prelude::*;
use chimera_core::console::{Console, ConsoleAppExt};
use chimera_core::core::NetRole;
use chimera_net::net::vote::{CallRefusal, VoteAppExt, VoteIssue, VoteSettings};
use chimera_sim::minigame::{ActiveMinigame, overrides};

/// The host settings' data (data/host_settings.toml).
pub const SETTINGS_TOML: &str = include_str!("../data/host_settings.toml");

/// The skip-map vote's issue name (`callvote skipmap`).
pub const SKIP_ISSUE: &str = "skipmap";

/// How a setting's value is picked.
#[derive(Clone, Debug, PartialEq)]
pub enum Pick {
    /// Values with their labels, in order (a click takes the next).
    Choices(Vec<(String, String)>),
    /// `-` and `+` by `step` within `min..=max`.
    Range { min: f32, max: f32, step: f32 },
}

impl Pick {
    /// The value after `value` (`dir` 1 forward, -1 back).
    pub fn next(&self, value: &str, dir: i8) -> String {
        match self {
            Pick::Choices(c) => {
                let at = c.iter().position(|(v, _)| same_value(v, value)).unwrap_or(0) as i64;
                let n = c.len() as i64;
                c[((at + dir as i64).rem_euclid(n)) as usize].0.clone()
            }
            Pick::Range { min, max, step } => {
                let v = value.trim().parse::<f32>().unwrap_or(*min);
                let next = (v + step * dir as f32).clamp(*min, *max);
                let next = (next * 100.0).round() / 100.0;
                format!("{next}")
            }
        }
    }

    /// What `value` shows as.
    pub fn shown(&self, value: &str) -> String {
        match self {
            Pick::Choices(c) => c
                .iter()
                .find(|(v, _)| same_value(v, value))
                .map_or_else(|| value.to_string(), |(_, l)| l.clone()),
            Pick::Range { .. } => {
                let v = value.trim().parse::<f32>().map_or_else(|_| value.to_string(), |f| format!("{}", (f * 100.0).round() / 100.0));
                v
            }
        }
    }
}

/// Two cvar values the same (`1` and `1.0`, any case).
pub fn same_value(a: &str, b: &str) -> bool {
    match (a.trim().parse::<f32>(), b.trim().parse::<f32>()) {
        (Ok(x), Ok(y)) => (x - y).abs() < 1e-4,
        _ => a.trim().eq_ignore_ascii_case(b.trim()),
    }
}

/// A setting on a page (the host's or the player's options).
#[derive(Clone, Debug, PartialEq)]
pub struct Setting {
    pub label: String,
    pub cvar: String,
    pub pick: Pick,
    /// The definition key it overrides (from the next match), if any.
    pub key: Option<String>,
    pub help: String,
    /// It applies at once (else from the next match).
    pub live: bool,
    /// Shown only where this holds (`when = "ui_sound"`: an app with UI
    /// sound, `esc_menu::option_tabs_for`); None: always.
    pub when: Option<String>,
    /// How the Signal look's Settings shows it (`widget = "dropdown"`;
    /// None: by its pick, `settings::widget`).
    pub widget: Option<String>,
}

/// A `[[setting]]` (or a tab's `setting` entry) read.
pub fn setting_of(v: &toml::Value) -> Result<Setting, String> {
    let t = v.as_table().ok_or("a setting is a table")?;
    let s = |k: &str| t.get(k).and_then(|v| v.as_str()).map(str::to_string);
    let label = s("label").ok_or("a setting without a label")?;
    let cvar = s("cvar").ok_or_else(|| format!("{label}: no cvar"))?;
    let num = |v: &toml::Value| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)).map(|f| f as f32);
    let pick = if let Some(c) = t.get("choices").and_then(|c| c.as_array()) {
        let choices = c
            .iter()
            .map(|p| {
                let p = p.as_array().filter(|p| p.len() == 2).ok_or_else(|| format!("{label}: a choice is [value, label]"))?;
                Ok((p[0].as_str().unwrap_or_default().to_string(), p[1].as_str().unwrap_or_default().to_string()))
            })
            .collect::<Result<Vec<_>, String>>()?;
        if choices.is_empty() {
            return Err(format!("{label}: no choices"));
        }
        Pick::Choices(choices)
    } else if let Some(r) = t.get("range").and_then(|r| r.as_array()) {
        let r: Vec<f32> = r.iter().filter_map(num).collect();
        let [min, max, step] = r[..] else { return Err(format!("{label}: range is [min, max, step]")) };
        Pick::Range { min, max, step }
    } else {
        return Err(format!("{label}: choices or range"));
    };
    let key = s("key");
    Ok(Setting {
        live: t.get("live").and_then(|v| v.as_bool()).unwrap_or(key.is_none()),
        label,
        cvar,
        pick,
        key,
        help: s("help").unwrap_or_default(),
        when: s("when"),
        widget: s("widget"),
    })
}

/// The host settings (`SETTINGS_TOML`), in order.
pub fn settings() -> Vec<Setting> {
    let doc: toml::Table = toml::from_str(SETTINGS_TOML).expect("data/host_settings.toml reads");
    doc.get("setting")
        .and_then(|s| s.as_array())
        .map(|a| a.iter().map(|v| setting_of(v).expect("data/host_settings.toml")).collect())
        .unwrap_or_default()
}

/// The party's vote rules over CS:S's defaults (`net::vote`): cvar,
/// value.
pub const VOTE_RULES: &[(&str, &str)] = &[
    ("sv_vote_quorum_ratio", "0.51"),
    ("sv_vote_timer_duration", "20"),
    ("sv_vote_creation_timer", "60"),
    ("sv_vote_failure_timer", "60"),
    ("sv_vote_bots_allowed", "0"),
];

/// Whether this process is the host (may change the host settings): not
/// a network client.
pub fn is_host(world: &World) -> bool {
    world.get_resource::<NetRole>().is_none_or(|r| *r != NetRole::Client)
}

/// The host overrides' cvars and the skip-map issue (`PartyPartsPlugin`).
pub fn build(app: &mut App) {
    for s in settings() {
        if let Some(key) = &s.key {
            let (name, key): (&'static str, &'static str) =
                (Box::leak(s.cvar.clone().into_boxed_str()), Box::leak(key.clone().into_boxed_str()));
            overrides::override_cvar(app, name, key, &s.help);
        }
    }
    app.vote_issue(VoteIssue {
        name: SKIP_ISSUE,
        check: |w, _, _| {
            if !w.contains_resource::<ActiveMinigame>() {
                return Err(CallRefusal::IssueDisabled);
            }
            if w.contains_resource::<chimera_net::minigame_net::MapChange>() {
                return Err(CallRefusal::InProgress);
            }
            Ok(chimera_net::net::maps::map_name(&chimera_net::net::current_map(w)).to_string())
        },
        run: |w, _| {
            if let Some(map) = chimera_net::minigame_net::next_match(w) {
                info!("the vote skipped the map: next, {map}");
            }
        },
        choices: |_, _| Vec::new(),
    })
    .console_command(
        "chimera:party_skipmap",
        "party_skipmap: call a vote to skip the map (the next of the rotation); alone, with no server, skip it.",
        |w, _| {
            if w.get_resource::<NetRole>().is_none_or(|r| *r == NetRole::Standalone) {
                if !w.contains_resource::<ActiveMinigame>() {
                    return Err("no minigame playing".into());
                }
                let map = chimera_net::minigame_net::next_match(w).ok_or("no minigame playing")?;
                return Ok(Some(format!("Skipping the map: next, {map}")));
            }
            w.resource_mut::<Console>().submit(format!("callvote {SKIP_ISSUE}"));
            Ok(None)
        },
    );
}

/// The party's vote rules (`VOTE_RULES`), once the network's votes are in.
pub fn finish(app: &mut App) {
    let world = app.world_mut();
    if !world.contains_resource::<VoteSettings>() {
        return;
    }
    for (name, value) in VOTE_RULES {
        let cvar = world.resource::<Console>().cvar(name).cloned();
        if let Some(c) = cvar {
            let _ = (c.set)(world, value);
            world.resource_mut::<Console>().set_default(name, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_host_settings_read_and_step() {
        let s = settings();
        assert!(s.len() >= 10);
        let knife = s.iter().find(|s| s.cvar == "mp_knife_finale").unwrap();
        assert_eq!(knife.key.as_deref(), Some("rules.knife_finale"));
        assert!(!knife.live);
        assert_eq!(knife.pick.next("", 1), "true");
        assert_eq!(knife.pick.next("false", 1), "");
        assert_eq!(knife.pick.shown(""), "Minigame's");
        let bots = s.iter().find(|s| s.cvar == "MaxPlayer").unwrap();
        assert!(bots.live);
        assert_eq!(bots.pick.next("3", 1), "4");
        assert_eq!(bots.pick.next("0", -1), "0");
        assert_eq!(bots.pick.next("15", 1), "15");
    }
}
