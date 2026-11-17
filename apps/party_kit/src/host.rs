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
//! - **Who may change them**: the host (a listen server's player, or a
//!   game with no server), and a player its server took as an RCON admin
//!   (`rcon_password`, `chimera_net::net::rcon`): `may_host_settings`.
//!   An admin reads the server's values (`AdminCvars`, which `build`
//!   fills with these settings' cvars) and sets them through `rcon`
//!   (`set`), which the server checks each time. A dedicated server's
//!   console sets the same cvars.
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
//! - **Told in the lobby**: a setting with `lobby = true` is also a
//!   `chimera_net::prematch::LobbyFact`: the pre-match lobby shows its
//!   value (in the choice's words; "Minigame's" says nothing and is left
//!   out) to every player, who can leave before the match they don't want.
//!
//! Known shortcuts: the settings last for the host's process (not
//! archived).

use bevy::prelude::*;
use chimera_core::console::{Console, ConsoleAppExt};
use chimera_core::core::NetRole;
use chimera_net::net::rcon::{AdminCvars, RconAdmin};
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
                format!("{}", step_from(v, *step, dir, *min, *max))
            }
        }
    }

    #[cfg(feature = "present")]
    /// What a stepper over this pick knows (`chimera_ui::controls::
    /// StepperRange`): its range, step and which end `value` is at; for
    /// choices the longest label (they wrap round: no end).
    pub fn stepper_range(&self, value: &str) -> chimera_ui::controls::StepperRange<'_> {
        let r = chimera_ui::controls::StepperRange::new();
        match self {
            Pick::Choices(c) => r.longest(c.iter().map(|(_, l)| l.as_str()).max_by_key(|l| l.chars().count()).unwrap_or("")),
            Pick::Range { min, max, step } => {
                let v = value.trim().parse::<f32>().unwrap_or(*min);
                r.range(*min, *max).step(*step).reached(v <= *min + 1e-4, v >= *max - 1e-4)
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

/// `v` one step of `step` up (`dir` 1) or down (-1), on the step's grid
/// (multiples of it: a range from 63.74 by 1 goes 63.74, 64, 65, ...),
/// within `min..=max`, rounded to hundredths.
pub fn step_from(v: f32, step: f32, dir: i8, min: f32, max: f32) -> f32 {
    let next = if step > 0.0 {
        let k = v / step;
        let k = if dir >= 0 { (k + 1e-3).floor() + 1.0 } else { (k - 1e-3).ceil() - 1.0 };
        k * step
    } else {
        v
    };
    let next = next.clamp(min, max);
    (next * 100.0).round() / 100.0
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
    /// The least and most a typed number may be (inclusive), for values
    /// that are unsafe past them (a negative volume); the range is only
    /// the slider's.
    pub hard_min: Option<f32>,
    pub hard_max: Option<f32>,
    /// A range shown as a percentage (`percent = true`: a volume's 1.6
    /// shows "160%").
    pub percent: bool,
    /// The pre-match lobby tells it to every player (`lobby = true`:
    /// `chimera_net::prematch::LobbyFact`).
    pub lobby: bool,
}

impl Setting {
    #[cfg(feature = "present")]
    /// `Pick::stepper_range`, a percentage's scaled by 100 with its `%`.
    pub fn stepper_range(&self, value: &str) -> chimera_ui::controls::StepperRange<'_> {
        let r = self.pick.stepper_range(value);
        if self.percent && matches!(self.pick, Pick::Range { .. }) {
            r.scale(100.0).unit("%")
        } else {
            r
        }
    }

    /// What `value` shows as (`Pick::shown`, a percentage when `percent`).
    pub fn shown(&self, value: &str) -> String {
        match (&self.pick, value.trim().parse::<f32>()) {
            (Pick::Range { .. }, Ok(f)) if self.percent => format!("{}%", (f * 100.0).round()),
            _ => self.pick.shown(value),
        }
    }
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
        hard_min: t.get("hard_min").and_then(num),
        hard_max: t.get("hard_max").and_then(num),
        percent: t.get("percent").and_then(|v| v.as_bool()).unwrap_or(false),
        lobby: t.get("lobby").and_then(|v| v.as_bool()).unwrap_or(false),
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

/// Whether this process is the host (owns the server): not a network
/// client.
pub fn is_host(world: &World) -> bool {
    world.get_resource::<NetRole>().is_none_or(|r| *r != NetRole::Client)
}

/// Whether one may change the host settings: the host, or a network
/// client its server took as an RCON admin (`rcon::RconAdmin`).
pub fn may_change(role: Option<&NetRole>, admin: Option<&RconAdmin>) -> bool {
    role.is_none_or(|r| *r != NetRole::Client) || admin.is_some_and(|a| a.admin)
}

/// `may_change` for this world.
pub fn may_host_settings(world: &World) -> bool {
    may_change(world.get_resource::<NetRole>(), world.get_resource::<RconAdmin>())
}

/// A host setting's value: the server's (an admin client's copy,
/// `RconAdmin::values`), else the cvar's here.
pub fn value(world: &mut World, cvar: &str) -> Option<String> {
    if RconAdmin::is_admin(world) {
        return world.resource::<RconAdmin>().values.get(cvar).cloned();
    }
    let c = world.resource::<Console>().cvar(cvar).cloned()?;
    (c.get)(world)
}

/// Set a host setting: on the host's own console, or an admin client's
/// through `rcon` (the server checks the password again).
pub fn set(world: &mut World, cvar: &str, value: &str) {
    let quoted = if value.is_empty() { "\"\"".to_string() } else { chimera_core::console::quote(value) };
    if RconAdmin::is_admin(world) {
        // Shown at once; the server's value follows.
        world.resource_mut::<RconAdmin>().values.insert(cvar.to_string(), value.to_string());
        world.resource_mut::<Console>().submit(format!("rcon {cvar} {quoted}"));
    } else {
        world.resource_mut::<Console>().submit(format!("{cvar} {quoted}"));
    }
}

/// The host overrides' cvars and the skip-map issue (`PartyPartsPlugin`).
pub fn build(app: &mut App) {
    // An RCON admin's Escape menu shows the server's values.
    app.init_resource::<AdminCvars>();
    app.world_mut().resource_mut::<AdminCvars>().0.extend(settings().into_iter().map(|s| s.cvar));
    for s in settings() {
        // The lobby tells players the settings that matter to them.
        if s.lobby {
            chimera_net::prematch::add_fact(
                app,
                chimera_net::prematch::LobbyFact {
                    label: s.label.clone(),
                    cvar: s.cvar.clone(),
                    words: match &s.pick {
                        Pick::Choices(c) => c.clone(),
                        Pick::Range { .. } => Vec::new(),
                    },
                    // "": the minigame's own, nothing to tell.
                    hide: vec![String::new()],
                },
            );
        }
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
