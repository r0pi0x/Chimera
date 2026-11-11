//! The console's core (docs/plans/active/console.md): a registry of cvars
//! and commands any system can add to, a command queue with Source-style
//! syntax (`;` separates commands, quotes group words, `+`/`-` actions for
//! binds), aliases, binds, `wait`, and output lines with a severity. The
//! client draws it and feeds it keys; tests and the remote protocol queue
//! lines directly. Everything runs in one exclusive system each frame.
//!
//! The console's core: cvar/command registry
//! (`ConsoleAppExt::console_cvar`/`console_command`, `resource_cvar`),
//! Source-style parsing (`;`, quotes, `//`), aliases, binds, `wait`,
//! exec/config files, built-ins (help, find, cvarlist, differences, reset,
//! toggle, incrementvar, watch, scope, host_writeconfig, ...).
//!
//! Namespaces (docs/plans/active/console-namespaces.md): every cvar and
//! command has a full name `<namespace>:<name>` (`cs_source:sv_cheats`,
//! `combat_arms:MaxPlayer`, `chimera:noblock`), which always works. A
//! name registered without one is Source's: the engine's Source names
//! (maps, connection, video, audio, network, input, demos, cheats:
//! `console_names::ENGINE_NAMES`, one table) are `chimera:` with the
//! console's language, our tools and general settings (`CHIMERA`), and a
//! game's own rules and tuned values (`mp_*`, `sv_gravity`, the buy menu)
//! are CS:S's (`SOURCE_NAMESPACE`) until another Source game registers
//! its own; the full names saved before (`cs_source:fps_max`) still work
//! (`Console::resolve`). A game's `GameInfo::console_namespaces` names its canonical
//! namespace and its aliases (`css`, `ca`: `Console::add_namespace`). A
//! short name resolves in the scope's game, then `chimera`, then wherever
//! it is the only one (`Console::resolve`); several left is an error with
//! a hint. The scope is `scope <game>` when pinned, else the local
//! player's `GameIdentity` (the party apps: the gun in hand), else the
//! default game (`console_scope`, which also gives the console the games'
//! namespaces). Old names still resolve, hidden: the
//! `chimera_*`/`mashup_*` prefixes (`OLD_PREFIXES`) and renamed settings
//! (`OLD_NAMES`: `cl_hitmarker` is `chimera:hitmarker`). Configs save
//! cvars by full name, and `bind`/`bindgame`/`alias` store command names
//! in full (`Console::qualify_line`; Chimera's unshared names, `+`/`-`
//! actions and polled binds, `Console::keep_short`, stay short).
//!
//! Each cvar's `CvarScope` comes from its name (`SERVER_PREFIXES`,
//! `SERVER_NAMES`: replicated or server-only in a network game; a network
//! client can't set them, `net::cvars` sends the replicated ones by full
//! name); `sv_cheats` (`Cheats`) gates `CHEATS` (noclip, god, setpos,
//! setang, give, impulse but `PLAYER_IMPULSES` (100, 201), ent_fire,
//! `chimera:set*`; `is_cheat`) in network games; a network client sends
//! its cheats to the server with full names (`Console::to_server`,
//! `net::cheats`; `command_player` is the player a command acts on);
//! `chimera:noblock` (`core::NoBlock`) is ours and replicated; a network
//! client can't run `SERVER_COMMANDS` (bots, `changelevel`,
//! `mp_restartgame`); userinfo cvars (`Console::userinfo`, `userinfo()`:
//! e.g. `cl_autowepswitch`) are the local player's `core::UserInfo`.

use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};

use bevy::prelude::*;

pub type RunFn = Arc<dyn Fn(&mut World, &[String]) -> Result<Option<String>, String> + Send + Sync>;
pub type GetFn = Arc<dyn Fn(&mut World) -> Option<String> + Send + Sync>;
pub type SetFn = Arc<dyn Fn(&mut World, &str) -> Result<(), String> + Send + Sync>;
pub type CompleteFn = Arc<dyn Fn(&World, &str) -> Vec<String> + Send + Sync>;

/// A console variable: read and written through closures, so it can live
/// in any resource or component.
#[derive(Clone)]
pub struct Cvar {
    pub name: String,
    pub help: String,
    pub default: String,
    /// Values to offer for completion (enums, booleans).
    pub values: Vec<String>,
    /// Saved to config.cfg (Source's FCVAR_ARCHIVE): player preferences,
    /// not test or server settings.
    pub archive: bool,
    /// Sent to the server as the player's own setting (Source's
    /// FCVAR_USERINFO): the local player's `core::UserInfo`, and a network
    /// client's on joining and on change (`net`).
    pub userinfo: bool,
    /// Sensible bounds (Source's FCVAR min/max, here only a hint): the
    /// console's argument help shows them, the debug UI's sliders use them.
    pub range: Option<(f32, f32)>,
    /// Takes whole numbers only (a field of an integer type).
    pub integer: bool,
    /// Who owns it in a network game (from its name, `CvarScope::of`).
    pub scope: CvarScope,
    pub get: GetFn,
    pub set: SetFn,
}

/// Who owns a cvar in a network game (docs/plans/active/multiplayer.md,
/// "Console and cvars"): a network client can't set the server's
/// (`execute` refuses), and gets the replicated ones' values from the
/// server (`net::cvars`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CvarScope {
    /// The player's own (`cl_*`, `m_*`, video, audio, `chimera_*` tools).
    Local,
    /// The server's, never sent to clients (bots, secrets).
    Server,
    /// The server's, sent to clients on connect and on change (Source's
    /// FCVAR_REPLICATED): movement and game rules clients predict or show.
    Replicated,
}

/// Name prefixes of server and game-rule settings: movement and physics
/// `sv_*`, round rules `mp_*`, `phys_*`, `bot_*`, `ammo_*`. Maps may set
/// these through point_servercommand (`logic::classes::SETTING_PREFIXES`).
pub const SERVER_PREFIXES: &[&str] = &["sv_", "mp_", "phys_", "bot_", "ammo_"];

/// Game rules without a server prefix that are the server's and
/// replicated all the same, by full name: ours (`chimera:rounds`,
/// `chimera:noblock`, the kill announcer `chimera:announcer`, the party's
/// `chimera:party_even_footsteps`, which every machine mixes by), and
/// Source's `decalfrequency` (the seconds between a player's sprays) and
/// the freeze cam's timings (`spec_freeze_time`, `spec_freeze_traveltime`;
/// replicated in CS:S).
pub const SERVER_NAMES: &[&str] = &[
    "chimera:rounds",
    "chimera:noblock",
    "chimera:announcer",
    "chimera:party_even_footsteps",
    "chimera:decalfrequency",
    "cs_source:spec_freeze_time",
    "cs_source:spec_freeze_traveltime",
];

/// The player's own settings with a server prefix (Source's client cvars:
/// `mp_decals`, Multiplayer > Advanced's decal limit), by full name.
pub const LOCAL_NAMES: &[&str] = &["chimera:mp_decals"];

impl CvarScope {
    /// The scope of a cvar by its full name (`ns:name`; a bare name is
    /// Source's), as written in `SERVER_NAMES` (`Console::cvar_scope`
    /// follows namespace aliases too): the server's when its name has a
    /// server prefix; of those, bots' settings and secrets stay on the
    /// server and the rest replicate.
    pub fn of(full: &str) -> Self {
        let full = full.to_lowercase();
        let (ns, name) = split(&full);
        let full = format!("{}:{name}", source_home(ns, name));
        Self::by_rules(name, |table| table.contains(&full.as_str()))
    }

    fn by_rules(name: &str, listed: impl Fn(&[&str]) -> bool) -> Self {
        if listed(SERVER_NAMES) {
            CvarScope::Replicated
        } else if listed(LOCAL_NAMES) {
            CvarScope::Local
        } else if !SERVER_PREFIXES.iter().any(|p| name.starts_with(p)) {
            CvarScope::Local
        } else if name.starts_with("bot_")
            || name.starts_with("sv_log")
            || name.contains("password")
            || name.contains("rcon")
        {
            CvarScope::Server
        } else {
            CvarScope::Replicated
        }
    }
}

#[derive(Clone)]
pub struct Command {
    pub name: String,
    pub help: String,
    pub run: RunFn,
    /// Completion for the arguments (the text after the name).
    pub complete: Option<CompleteFn>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// A line the user typed.
    Input,
    Info,
    Warn,
    Error,
}

#[derive(Clone, Debug)]
pub struct Line {
    pub text: String,
    pub level: Level,
    /// Seconds since startup.
    pub time: f64,
}

/// The console's state.
#[derive(Resource, Default)]
pub struct Console {
    /// By full name, lower case (`cs_source:sv_gravity`).
    cvars: BTreeMap<String, Cvar>,
    commands: BTreeMap<String, Command>,
    /// A short name (lower case) -> the full names (lower case) of the
    /// cvars and commands it is short for.
    shorts: BTreeMap<String, Vec<String>>,
    /// A namespace (lower case, canonical or an alias) -> its canonical
    /// one (`Console::add_namespace`: `css` -> `cs_source`).
    namespaces: BTreeMap<String, String>,
    /// The scope pinned by `scope <game>` (canonical), over `auto_scope`.
    pub scope: Option<String>,
    /// The scope when none is pinned (`follow_scope`): the local player's
    /// identity's game, else the default game.
    pub auto_scope: Option<String>,
    /// Command words a stored line keeps as typed (`qualify_line`): the
    /// binds the client polls by their short name (`slot1`, `buymenu`).
    keep_short: std::collections::BTreeSet<String>,
    /// Commands config.cfg's binds leave out (`bound_for_session`).
    session_commands: std::collections::BTreeSet<String>,
    pub aliases: BTreeMap<String, String>,
    /// Key name (lower-case) -> command line.
    pub binds: BTreeMap<String, String>,
    /// Per-game overlays on `binds` (a game id -> key -> line): the keys
    /// that mean something else, or nothing (an empty line), while that
    /// game is the player's identity (`effective_binds`). Saved in the
    /// config (`bindgame` lines).
    pub layers: BTreeMap<String, BTreeMap<String, String>>,
    /// Each overlay as its game ships it (`binddefaults` for a game
    /// restores it: `bindgamedefaults`).
    pub layer_defaults: BTreeMap<String, BTreeMap<String, String>>,
    pub output: Vec<Line>,
    /// Lines waiting to run, and frames to wait before the next.
    queue: VecDeque<String>,
    wait: u32,
    /// Expressions shown on the HUD (`watch`), by full name.
    pub watches: Vec<String>,
    /// Bumped when something should be saved (binds, cvars changed).
    pub dirty: bool,
    /// Lines printed since startup (the output keeps only the last ones).
    pub printed: u64,
    /// Cvars registered again under a name taken (the later one replaced
    /// the earlier): two settings answering to one name, a bug
    /// (`hostname` had two, so the server's name couldn't be set).
    pub replaced: Vec<String>,
    /// Lines a network client sends its server to run for its player
    /// (cheats: `net::cheats` takes them), with full names.
    pub to_server: Vec<String>,
    /// Commands a game marks as acting on the player who runs them
    /// (`player_command`; beside `PLAYER_COMMANDS`): a network client
    /// sends them to its server, full names, lower case.
    player_commands: std::collections::BTreeSet<String>,
    /// Commands a game marks as the server operator's (`server_command`;
    /// beside `SERVER_COMMANDS`): a network client can't run them. Full
    /// names, lower case, as the game wrote them.
    server_commands: std::collections::BTreeSet<String>,
    /// Also log what commands print (info, errors as `error: ...`, target
    /// `console`): the client sets it, so a run's log holds the results of
    /// its `+commands` (the dedicated servers print them on stdout).
    pub log_output: bool,
}

/// Chimera's own namespace: the console's language (`bind`, `alias`,
/// `exec`, `scope`, ...), the input actions (`+jump`), our tools and our
/// general settings (`chimera:noblock`).
pub const CHIMERA: &str = "chimera";

/// The namespace of a Source name that is a game's own (its rules, the
/// movement values it tunes: `sv_gravity`, `mp_roundtime`, the buy menu),
/// registered without one: CS:S's, the first Source game. Engine names
/// (`map`, `fps_max`, `noclip`) are `chimera:` (`console_names`).
pub const SOURCE_NAMESPACE: &str = "cs_source";

/// The namespace a name has: its own when it says one, but CS:S's
/// (`cs_source`, or none) for a Source engine name (`console_names::ENGINE_NAMES`)
/// is `chimera`, where the engine's names moved; a bare name goes to
/// `console_names::home`. Namespace aliases (`css`) are not followed here
/// (`Console::canonical_full` does).
pub fn source_home<'a>(ns: Option<&'a str>, short: &str) -> &'a str {
    match ns {
        None => home(short),
        Some(ns) if ns == SOURCE_NAMESPACE => home(short),
        Some(ns) => ns,
    }
}

/// The namespace of a Source name that has none: `chimera` for an engine
/// name (`console_names::ENGINE_NAMES`), else CS:S's (`SOURCE_NAMESPACE`).
fn home(short: &str) -> &'static str {
    if crate::console_names::is_engine(short) { CHIMERA } else { SOURCE_NAMESPACE }
}

/// Old prefixes of our own names (before namespaces; `mashup_` before the
/// rename to Chimera): `chimera_noblock` and `mashup_noblock` are
/// `chimera:noblock`. Hidden aliases for saved configs, binds and scripts.
pub const OLD_PREFIXES: &[(&str, &str)] = &[("chimera_", CHIMERA), ("mashup_", CHIMERA)];

/// Our settings and commands renamed with namespaces (old name -> full
/// name): our general settings lost their `sv_`/`cl_`/`net_` prefixes; a
/// `chimera_*` name that would have met a built-in got a new one. Hidden
/// aliases, as `OLD_PREFIXES`.
pub const OLD_NAMES: &[(&str, &str)] = &[
    ("cl_hitmarker", "chimera:hitmarker"),
    ("sv_announcer", "chimera:announcer"),
    ("cl_predict_projectiles", "chimera:predict_projectiles"),
    ("hud_radar", "chimera:radar"),
    ("net_upnp", "chimera:upnp"),
    ("net_lan_ports", "chimera:lan_ports"),
    ("net_lan_broadcast", "chimera:lan_broadcast"),
    ("chimera_watch", "chimera:watchbot"),
    ("mashup_watch", "chimera:watchbot"),
    ("cl_tracer_through_walls", "combat_arms:tracer_through_walls"),
];

/// A name's namespace and short name (`cs_source:sv_gravity`:
/// `(Some("cs_source"), "sv_gravity")`; a bare name has none).
pub fn split(name: &str) -> (Option<&str>, &str) {
    match name.split_once(':') {
        Some((ns, short)) if !ns.is_empty() && !short.is_empty() => (Some(ns), short),
        _ => (None, name),
    }
}

/// The short name of a full one (the part after its namespace).
pub fn short_name(name: &str) -> &str {
    split(name).1
}

/// What a name typed or stored resolves to (`Console::resolve`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolved {
    /// The full name (lower case) of a cvar or command.
    Found(String),
    /// A short name several namespaces have, none of them the scope's or
    /// Chimera's: their full names.
    Ambiguous(Vec<String>),
    Unknown,
}

const MAX_OUTPUT: usize = 4000;
const MAX_ALIAS_DEPTH: usize = 32;

impl Console {
    /// The binds in force for a player whose identity is `game` (None: no
    /// identity): the game's overlay first, then the shared binds. A key
    /// the overlay holds means its line there, even an empty one (nothing).
    pub fn effective_binds(&self, game: Option<&str>) -> std::borrow::Cow<'_, BTreeMap<String, String>> {
        match game.and_then(|g| self.layers.get(g)).filter(|l| !l.is_empty()) {
            None => std::borrow::Cow::Borrowed(&self.binds),
            Some(layer) => {
                let mut all = self.binds.clone();
                all.extend(layer.iter().map(|(k, v)| (k.clone(), v.clone())));
                std::borrow::Cow::Owned(all)
            }
        }
    }

    /// Set `game`'s own binds as shipped (the defaults `bindgamedefaults`
    /// restores), in force now.
    pub fn ship_layer(&mut self, game: &str, binds: &[(&str, &str)]) {
        let layer: BTreeMap<String, String> = binds.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        self.layers.insert(game.to_string(), layer.clone());
        self.layer_defaults.insert(game.to_string(), layer);
    }

    /// A game's namespaces (`GameInfo::console_namespaces`): the first is
    /// canonical (what configs save and the network sends), the rest are
    /// aliases accepted wherever a namespace is (`css:sv_gravity`,
    /// `scope ca`). Names registered under an alias move to the canonical.
    pub fn add_namespace(&mut self, names: &[&str]) {
        let Some(canonical) = names.first().map(|n| n.to_lowercase()) else {
            return;
        };
        for n in names {
            self.namespaces.insert(n.to_lowercase(), canonical.clone());
        }
        // Anything that pointed at a namespace now an alias points on.
        for target in self.namespaces.values_mut() {
            if names.iter().any(|n| n.eq_ignore_ascii_case(target)) {
                *target = canonical.clone();
            }
        }
        self.namespaces.insert(canonical.clone(), canonical.clone());
        let moved = |key: &str| {
            let (ns, short) = split(key);
            ns.filter(|ns| *ns != canonical && names.iter().any(|n| n.eq_ignore_ascii_case(ns)))
                .map(|_| short.to_string())
        };
        let rename = |name: &mut String| {
            if let Some(short) = moved(&name.to_lowercase()) {
                let keep = split(name).1.to_string();
                debug_assert_eq!(keep.to_lowercase(), short);
                *name = format!("{canonical}:{keep}");
            }
        };
        let cvars = std::mem::take(&mut self.cvars);
        for (_, mut c) in cvars {
            rename(&mut c.name);
            self.cvars.insert(c.name.to_lowercase(), c);
        }
        let commands = std::mem::take(&mut self.commands);
        for (_, mut c) in commands {
            rename(&mut c.name);
            self.commands.insert(c.name.to_lowercase(), c);
        }
        self.reindex();
    }

    /// A namespace's canonical name (an alias's game; any other as it is).
    pub fn canonical_namespace(&self, ns: &str) -> String {
        let ns = ns.to_lowercase();
        self.namespaces.get(&ns).cloned().unwrap_or(ns)
    }

    /// Whether `ns` names a namespace: a game's (or an alias), Chimera's,
    /// or one some name is registered in.
    pub fn is_namespace(&self, ns: &str) -> bool {
        let ns = self.canonical_namespace(ns);
        ns == CHIMERA
            || self.namespaces.contains_key(&ns)
            || self
                .cvars
                .keys()
                .chain(self.commands.keys())
                .any(|k| split(k).0 == Some(ns.as_str()))
    }

    /// Every namespace name `scope` takes: the games' and their aliases
    /// (each with its canonical one), Chimera's.
    pub fn namespace_names(&self) -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> = self.namespaces.iter().map(|(k, c)| (k.clone(), c.clone())).collect();
        v.push((CHIMERA.into(), CHIMERA.into()));
        v
    }

    /// The scope short names resolve in first: the pinned one, else the
    /// automatic one.
    pub fn current_scope(&self) -> Option<&str> {
        self.scope.as_deref().or(self.auto_scope.as_deref())
    }

    /// `name` as a full name in canonical form, lower case (a bare name is
    /// Source's, `SOURCE_NAMESPACE`), whether or not it is registered: for
    /// comparing with tables of full names.
    pub fn canonical_full(&self, name: &str) -> String {
        let lower = name.to_lowercase();
        let (ns, short) = split(&lower);
        format!("{}:{short}", self.home_of(ns, short))
    }

    /// The canonical namespace of `short` typed with `ns` (None: bare):
    /// the one it names, but Source's names go where the table sends them
    /// (`source_home`: `cs_source:fps_max` is `chimera:fps_max`).
    fn home_of(&self, ns: Option<&str>, short: &str) -> String {
        let source = self.canonical_namespace(SOURCE_NAMESPACE);
        match ns.map(|n| self.canonical_namespace(n)) {
            Some(ns) if ns != source => ns,
            _ => self.canonical_namespace(home(short)),
        }
    }

    /// The full name (cased as registered) a cvar or command registered
    /// as `name` gets.
    fn registered_name(&self, name: &str) -> String {
        let lower = name.to_lowercase();
        if let (Some(ns), short) = split(name) {
            return format!("{}:{short}", self.home_of(Some(ns), short));
        }
        if let Some((_, new)) = OLD_NAMES.iter().find(|(old, _)| *old == lower) {
            let (ns, short) = split(new);
            return format!("{}:{short}", self.canonical_namespace(ns.unwrap_or(CHIMERA)));
        }
        for (prefix, ns) in OLD_PREFIXES {
            if lower.starts_with(prefix) {
                return format!("{}:{}", self.canonical_namespace(ns), &name[prefix.len()..]);
            }
        }
        format!("{}:{name}", self.home_of(None, name))
    }

    fn known(&self, full: &str) -> bool {
        self.cvars.contains_key(full) || self.commands.contains_key(full)
    }

    fn index(&mut self, full: &str) {
        let short = short_name(full).to_string();
        let entry = self.shorts.entry(short).or_default();
        if !entry.iter().any(|f| f == full) {
            entry.push(full.to_string());
            entry.sort();
        }
    }

    fn reindex(&mut self) {
        self.shorts.clear();
        let keys: Vec<String> = self.cvars.keys().chain(self.commands.keys()).cloned().collect();
        for k in keys {
            self.index(&k);
        }
    }

    /// What `typed` names: a full name (its namespace may be an alias); a
    /// short name in the scope's game, then Chimera's, then the only
    /// namespace that has it (several: ambiguous); else an old name
    /// (`OLD_NAMES`, `OLD_PREFIXES`).
    pub fn resolve(&self, typed: &str) -> Resolved {
        self.resolve_in(typed, self.current_scope())
    }

    /// `resolve` with `scope` for the scope.
    pub fn resolve_in(&self, typed: &str, scope: Option<&str>) -> Resolved {
        let lower = typed.to_lowercase();
        if let (Some(ns), short) = split(&lower) {
            let full = format!("{}:{short}", self.home_of(Some(ns), short));
            return if self.known(&full) {
                Resolved::Found(full)
            } else {
                Resolved::Unknown
            };
        }
        if let Some(all) = self.shorts.get(&lower) {
            for ns in scope.into_iter().chain([CHIMERA]) {
                if let Some(full) = all.iter().find(|f| split(f).0 == Some(ns)) {
                    return Resolved::Found(full.clone());
                }
            }
            match all.len() {
                0 => {}
                1 => return Resolved::Found(all[0].clone()),
                _ => return Resolved::Ambiguous(all.clone()),
            }
        }
        if let Some((_, new)) = OLD_NAMES.iter().find(|(old, _)| *old == lower) {
            let full = self.canonical_full(new);
            if self.known(&full) {
                return Resolved::Found(full);
            }
        }
        for (prefix, ns) in OLD_PREFIXES {
            if let Some(rest) = lower.strip_prefix(prefix) {
                let full = format!("{}:{rest}", self.canonical_namespace(ns));
                if self.known(&full) {
                    return Resolved::Found(full);
                }
            }
        }
        Resolved::Unknown
    }

    /// The full name (lower case) `typed` resolves to, if one.
    pub fn full_name(&self, typed: &str) -> Option<String> {
        match self.resolve(typed) {
            Resolved::Found(full) => Some(full),
            _ => None,
        }
    }

    /// The registered spelling of a full name (`combat_arms:MaxPlayer`).
    pub fn spelled(&self, full: &str) -> String {
        let key = full.to_lowercase();
        self.cvars
            .get(&key)
            .map(|c| c.name.clone())
            .or_else(|| self.commands.get(&key).map(|c| c.name.clone()))
            .unwrap_or(key)
    }

    /// How to show a full name: its short name when that resolves to it
    /// here (in the current scope), else the full name.
    pub fn display_name(&self, full: &str) -> String {
        let spelled = self.spelled(full);
        let short = short_name(&spelled);
        if self.full_name(short).as_deref() == Some(full.to_lowercase().as_str()) {
            short.to_string()
        } else {
            spelled
        }
    }

    /// The hint for a short name several namespaces have.
    pub fn ambiguity_hint(&self, typed: &str, fulls: &[String]) -> String {
        let names: Vec<String> = fulls.iter().map(|f| self.spelled(f)).collect();
        format!(
            "\"{typed}\" is ambiguous: {}. Type the full name, or pick a game with `scope <game>`.",
            names.join(", ")
        )
    }

    /// `command` (its short name) is left out of the binds config.cfg
    /// saves: a game's command it binds again at every start (Combat Arms'
    /// `choose_N`, on keys its profile holds). config.cfg is shared by every
    /// app, where this one's command is nothing.
    pub fn bound_for_session(&mut self, command: &str) {
        self.session_commands.insert(command.trim().to_lowercase());
    }

    /// `line` as config.cfg saves it: without `bound_for_session` commands
    /// (None when nothing is left). Other lines keep their text.
    fn saved_bind(&self, line: &str) -> Option<String> {
        let commands = parse(line);
        let session = |words: &Vec<String>| {
            let lower = words[0].to_lowercase();
            self.session_commands.contains(short_name(&lower))
        };
        if !commands.iter().any(session) {
            return Some(line.to_string());
        }
        let kept: Vec<String> = commands
            .iter()
            .filter(|w| !session(w))
            .map(|w| w.iter().map(|x| quote(x)).collect::<Vec<_>>().join(" "))
            .collect();
        (!kept.is_empty()).then(|| kept.join("; "))
    }

    /// A word a stored line keeps as typed (a bind the client polls by its
    /// short name: `slot1`, `buymenu`).
    pub fn keep_short(&mut self, word: &str) {
        self.keep_short.insert(word.trim().to_lowercase());
    }

    /// How a stored line names the command `typed`: None to keep it as
    /// typed (`+`/`-` actions, `keep_short` words, aliases, names that
    /// resolve to nothing or to several); else its full name, but a name
    /// of Chimera's that no game shares stays short.
    fn stored_name(&self, typed: &str, scope: Option<&str>) -> Option<String> {
        let lower = typed.to_lowercase();
        if lower.starts_with(['+', '-']) || self.keep_short.contains(&lower) || self.aliases.contains_key(&lower) {
            return None;
        }
        let Resolved::Found(full) = self.resolve_in(typed, scope) else {
            return None;
        };
        let spelled = self.spelled(&full);
        let (ns, short) = split(&full);
        if ns == Some(CHIMERA) && self.shorts.get(short).is_some_and(|v| v.len() == 1) {
            return Some(short_name(&spelled).to_string());
        }
        Some(spelled)
    }

    /// `line` as `bind`, `bindgame`, `alias` and config.cfg store it: each
    /// command's name in full (`stored_name`), so it means the same in any
    /// scope and app; an old name (`chimera_noblock`) is written as the
    /// new one. Unchanged lines keep their text. Also the hints for names
    /// left short because several games have them.
    pub fn qualify_line_with_hints(&self, line: &str) -> (String, Vec<String>) {
        self.qualify_line_in(line, self.current_scope())
    }

    /// `qualify_line_with_hints` with `scope` for the scope (a game's own
    /// binds, `bindgame`, in that game's).
    pub fn qualify_line_in(&self, line: &str, scope: Option<&str>) -> (String, Vec<String>) {
        let commands = parse(line);
        let mut changed = false;
        let mut hints = Vec::new();
        let out: Vec<String> = commands
            .into_iter()
            .map(|mut words| {
                if let Resolved::Ambiguous(all) = self.resolve_in(&words[0], scope)
                    && !self.aliases.contains_key(&words[0].to_lowercase())
                {
                    hints.push(self.ambiguity_hint(&words[0], &all));
                }
                if let Some(name) = self.stored_name(&words[0], scope)
                    && name != words[0]
                {
                    words[0] = name;
                    changed = true;
                }
                words.iter().map(|w| quote(w)).collect::<Vec<_>>().join(" ")
            })
            .collect();
        (if changed { out.join("; ") } else { line.to_string() }, hints)
    }

    /// `qualify_line_with_hints`'s line.
    pub fn qualify_line(&self, line: &str) -> String {
        self.qualify_line_with_hints(line).0
    }

    /// `words` (a command) as a line with its name in full, canonical: what
    /// goes over the network (a client's commands for its server).
    pub fn wire_line(&self, words: &[String]) -> String {
        let mut words = words.to_vec();
        if let Some(first) = words.first_mut()
            && let Some(full) = self.full_name(first)
        {
            *first = self.spelled(&full);
        }
        words.iter().map(|w| quote(w)).collect::<Vec<_>>().join(" ")
    }

    /// A cvar's `CvarScope` by its full name, its namespace's aliases
    /// followed (`CvarScope::of`).
    pub fn cvar_scope(&self, name: &str) -> CvarScope {
        let full = self.canonical_full(name);
        CvarScope::by_rules(short_name(&full), |table| {
            table.iter().any(|t| self.canonical_full(t) == full)
        })
    }

    /// Add a cvar under its full name (`registered_name`); its scope comes
    /// from that name (`cvar_scope`).
    pub fn add_cvar(&mut self, mut cvar: Cvar) {
        cvar.name = self.registered_name(&cvar.name);
        cvar.scope = self.cvar_scope(&cvar.name);
        let key = cvar.name.to_lowercase();
        self.index(&key);
        if let Some(old) = self.cvars.insert(key, cvar) {
            self.replaced.push(old.name);
        }
    }

    /// The full name (lower case) a registering plugin means by `name`:
    /// the name it registered (`registered_name`) when there is one, else
    /// what `name` resolves to.
    fn own_name(&self, name: &str) -> Option<String> {
        let registered = self.registered_name(name).to_lowercase();
        if self.known(&registered) {
            Some(registered)
        } else {
            self.full_name(name)
        }
    }

    fn cvar_mut(&mut self, name: &str) -> Option<&mut Cvar> {
        let full = self.own_name(name)?;
        self.cvars.get_mut(&full)
    }

    /// Mark a cvar to be saved in config.cfg.
    pub fn archive(&mut self, name: &str) {
        if let Some(c) = self.cvar_mut(name) {
            c.archive = true;
        }
    }

    /// Change a cvar's default (what it is when the config says nothing,
    /// so the config leaves it out while it has that value): an app whose
    /// starting value is not the shell's.
    pub fn set_default(&mut self, name: &str, value: &str) {
        if let Some(c) = self.cvar_mut(name) {
            c.default = value.into();
        }
    }

    /// Mark command `name` (full) as acting on the player who runs it: a
    /// network client sends it to its server, which runs it for that
    /// client's player, no `sv_cheats` needed (a game's `jointeam`).
    pub fn player_command(&mut self, name: &str) {
        let full = self.canonical_full(name);
        self.player_commands.insert(full);
    }

    /// Whether command `name` (any name `resolve` takes) acts on its
    /// player (`PLAYER_COMMANDS`, or a game's `player_command`).
    pub fn is_player_command(&self, name: &str) -> bool {
        in_table(self, PLAYER_COMMANDS, name)
            || self.full_name(name).is_some_and(|f| self.player_commands.contains(&self.canonical_full(&f)))
    }

    /// Mark command `name` (full) as the server operator's: it changes
    /// the server's game (a game's bots, its match restart), so a network
    /// client can't run it (`SERVER_COMMANDS` holds Chimera's own).
    pub fn server_command(&mut self, name: &str) {
        self.server_commands.insert(name.to_lowercase());
    }

    /// Whether command `name` (any name `resolve` takes) is the server
    /// operator's (`SERVER_COMMANDS`, or a game's `server_command`;
    /// namespace aliases followed).
    pub fn is_server_command(&self, name: &str) -> bool {
        in_table(self, SERVER_COMMANDS, name)
            || self
                .full_name(name)
                .is_some_and(|full| self.server_commands.iter().any(|t| self.canonical_full(t) == full))
    }

    /// Mark a cvar as the player's own setting the simulation reads
    /// (`core::UserInfo`).
    pub fn userinfo(&mut self, name: &str) {
        if let Some(c) = self.cvar_mut(name) {
            c.userinfo = true;
        }
    }

    /// Give a cvar a range for hints and sliders.
    pub fn set_range(&mut self, name: &str, min: f32, max: f32) {
        if let Some(c) = self.cvar_mut(name) {
            c.range = Some((min, max));
        }
    }

    /// Give a command argument completion.
    pub fn set_completion(&mut self, name: &str, complete: CompleteFn) {
        if let Some(full) = self.own_name(name)
            && let Some(c) = self.commands.get_mut(&full)
        {
            c.complete = Some(complete);
        }
    }

    pub fn add_command(&mut self, mut command: Command) {
        command.name = self.registered_name(&command.name);
        let key = command.name.to_lowercase();
        self.index(&key);
        self.commands.insert(key, command);
    }

    /// A cvar by name: full, short (`resolve`) or old, any case.
    pub fn cvar(&self, name: &str) -> Option<&Cvar> {
        if let Some(c) = self.cvars.get(name) {
            return Some(c);
        }
        self.cvars.get(&self.full_name(name)?)
    }

    /// A command by name (as `cvar`).
    pub fn command(&self, name: &str) -> Option<&Command> {
        if let Some(c) = self.commands.get(name) {
            return Some(c);
        }
        self.commands.get(&self.full_name(name)?)
    }

    pub fn cvars(&self) -> impl Iterator<Item = &Cvar> {
        self.cvars.values()
    }

    pub fn commands(&self) -> impl Iterator<Item = &Command> {
        self.commands.values()
    }

    /// Queue a line (it may hold several `;`-separated commands).
    pub fn submit(&mut self, line: impl Into<String>) {
        self.queue.push_back(line.into());
    }

    pub fn print(&mut self, level: Level, text: impl Into<String>) {
        let time = self.output.last().map_or(0.0, |l| l.time);
        for t in text.into().lines() {
            self.printed += 1;
            self.output.push(Line {
                text: t.to_string(),
                level,
                time,
            });
        }
        if self.output.len() > MAX_OUTPUT {
            let extra = self.output.len() - MAX_OUTPUT;
            self.output.drain(..extra);
        }
    }

    pub fn info(&mut self, text: impl Into<String>) {
        self.print(Level::Info, text);
    }

    /// Every name the console offers: cvars and commands as shown here
    /// (`display_name`), and aliases.
    pub fn names(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .cvars
            .keys()
            .chain(self.commands.keys())
            .map(|k| self.display_name(k))
            .chain(self.aliases.keys().cloned())
            .collect();
        v.sort();
        v.dedup();
        v
    }
}

/// Split a line into commands at `;` (outside quotes), and each command
/// into words (quotes group words and are removed; `//` starts a comment).
pub fn parse(line: &str) -> Vec<Vec<String>> {
    let mut commands = Vec::new();
    let mut words = Vec::new();
    let mut word = String::new();
    let (mut quoted, mut in_word) = (false, false);
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                quoted = !quoted;
                in_word = true;
            }
            '/' if !quoted && chars.peek() == Some(&'/') => break,
            ';' if !quoted => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
                if !words.is_empty() {
                    commands.push(std::mem::take(&mut words));
                }
            }
            c if c.is_whitespace() && !quoted => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            c => {
                word.push(c);
                in_word = true;
            }
        }
    }
    if in_word {
        words.push(word);
    }
    if !words.is_empty() {
        commands.push(words);
    }
    commands
}

/// Where the last command of a line starts (after its last `;` outside
/// quotes, and the spaces after it): completion and argument help work
/// on that command.
pub fn last_command_start(line: &str) -> usize {
    let mut quoted = false;
    let mut start = 0;
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ';' if !quoted => start = i + 1,
            _ => {}
        }
    }
    start + line[start..].len() - line[start..].trim_start().len()
}

/// Which argument of a command is being typed (0: the first after the
/// name), or None while the name itself is.
pub fn arg_position(command: &str) -> Option<usize> {
    let words = command.split_whitespace().count();
    let ends_space = command.ends_with(char::is_whitespace);
    match (words, ends_space) {
        (0, _) | (1, false) => None,
        (n, true) => Some(n - 1),
        (n, false) => Some(n - 2),
    }
}

/// A command's usage from the start of its help, Source style
/// (`"setpos <x> <y> <z>: move there"`): the argument words (brackets
/// kept, so `[amount=100]` is optional) and the description after the
/// colon. Help that doesn't start with the name has no usage.
pub fn usage(name: &str, help: &str) -> (Vec<String>, String) {
    // A full name's help starts with its short name.
    let short = short_name(name);
    let name = if help.get(..name.len()).is_some_and(|h| h.eq_ignore_ascii_case(name)) {
        name
    } else {
        short
    };
    let Some(rest) = help
        .get(..name.len())
        .filter(|h| h.eq_ignore_ascii_case(name))
        .map(|_| &help[name.len()..])
        .filter(|r| r.starts_with(' ') || r.starts_with(':'))
    else {
        return (Vec::new(), help.to_string());
    };
    let (mut args, mut word, mut depth) = (Vec::new(), String::new(), 0i32);
    let mut chars = rest.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        match c {
            '<' | '[' | '(' | '{' => depth += 1,
            '>' | ']' | ')' | '}' => depth -= 1,
            _ => {}
        }
        if depth <= 0 && c == ':' && chars.peek().is_none_or(|(_, n)| n.is_whitespace()) {
            if !word.is_empty() {
                args.push(std::mem::take(&mut word));
            }
            return (args, rest[i + 1..].trim().to_string());
        }
        if depth <= 0 && c.is_whitespace() {
            if !word.is_empty() {
                args.push(std::mem::take(&mut word));
            }
        } else {
            word.push(c);
        }
    }
    // No colon: the help is all description.
    (Vec::new(), help.to_string())
}

/// Quote a word if it needs it, for writing configs and echoing commands
/// (a URL's `//` too, which unquoted starts a comment: `parse`).
pub fn quote(word: &str) -> String {
    if word.is_empty() || word.contains(|c: char| c.is_whitespace() || c == ';' || c == '"') || word.contains("//") {
        format!("\"{}\"", word.replace('"', "'"))
    } else {
        word.to_string()
    }
}

/// Run queued lines (until a `wait`), in an exclusive system.
pub fn run_queue(world: &mut World) {
    let mut budget = 1024;
    loop {
        let next = {
            let mut c = world.resource_mut::<Console>();
            if c.wait > 0 {
                c.wait -= 1;
                return;
            }
            c.queue.pop_front()
        };
        let Some(line) = next else { return };
        let commands = parse(&line);
        for (i, words) in commands.iter().enumerate() {
            execute(world, words, 0);
            budget -= 1;
            if world.resource::<Console>().wait > 0 || budget == 0 {
                // The rest of this line runs after the wait.
                let rest: Vec<String> = commands[i + 1..]
                    .iter()
                    .map(|w| w.iter().map(|x| quote(x)).collect::<Vec<_>>().join(" "))
                    .collect();
                if !rest.is_empty() {
                    world.resource_mut::<Console>().queue.push_front(rest.join("; "));
                }
                return;
            }
        }
    }
}

/// Run one command (its words), with aliases expanded.
pub fn execute(world: &mut World, words: &[String], depth: usize) {
    let Some(typed) = words.first().map(|w| w.to_lowercase()) else {
        return;
    };
    let args = &words[1..];
    let (alias, resolved) = {
        let c = world.resource::<Console>();
        (c.aliases.get(&typed).cloned(), c.resolve(&typed))
    };
    if let Some(body) = alias {
        if depth >= MAX_ALIAS_DEPTH {
            world
                .resource_mut::<Console>()
                .print(Level::Error, format!("{typed}: alias loop"));
            return;
        }
        for w in parse(&body) {
            execute(world, &w, depth + 1);
        }
        return;
    }
    // The full name (an old `chimera_*` name runs its `chimera:` one).
    let (name, command, cvar) = match &resolved {
        Resolved::Found(full) => {
            let c = world.resource::<Console>();
            (full.clone(), c.commands.get(full).cloned(), c.cvars.get(full).cloned())
        }
        _ => (typed.clone(), None, None),
    };
    let client = world.get_resource::<crate::core::NetRole>() == Some(&crate::core::NetRole::Client);
    let result = if let Some(cmd) = command {
        let console = world.resource::<Console>();
        let (server_command, cheat, player_command) = (
            console.is_server_command(&name),
            is_cheat(console, &name, args),
            console.is_player_command(&name),
        );
        if server_command && client {
            Err(format!(
                "Can't use server command {typed} from a client: only the server operator can run it."
            ))
        } else if (cheat || player_command) && client {
            // A network client's cheat acts on its player on the server,
            // which runs it if its `sv_cheats` is 1 and says what came of
            // it (`net::cheats`), as Source's client sends them on. The
            // name goes in full: the server's scope may differ.
            let line = console.wire_line(words);
            world.resource_mut::<Console>().to_server.push(line);
            Ok(None)
        } else if cheat && !cheats_allowed(world) {
            // Source's words.
            Err(format!(
                "Can't use cheat command {typed} in multiplayer, unless the server has sv_cheats set to 1."
            ))
        } else {
            (cmd.run)(world, args)
        }
    } else if let Some(cvar) = cvar {
        if args.is_empty() {
            let value = (cvar.get)(world).unwrap_or_default();
            let changed = if value != cvar.default {
                format!(" (default \"{}\")", cvar.default)
            } else {
                String::new()
            };
            Ok(Some(format!(
                "\"{}\" = \"{}\"{changed}\n - {}",
                cvar.name, value, cvar.help
            )))
        } else if cvar.scope != CvarScope::Local && client {
            // The server's (sent to us, `net::cvars`), Source's words.
            Err(format!(
                "Can't change replicated ConVar {} from console of client, only server operator can change its value",
                short_name(&cvar.name)
            ))
        } else {
            (cvar.set)(world, &args.join(" ")).map(|_| {
                world.resource_mut::<Console>().dirty = true;
                None
            })
        }
    } else if let Resolved::Ambiguous(all) = &resolved {
        Err(world.resource::<Console>().ambiguity_hint(&typed, all))
    } else {
        Err(format!("Unknown command \"{typed}\""))
    };
    let mut c = world.resource_mut::<Console>();
    let log = c.log_output;
    match result {
        Ok(Some(out)) => {
            if log {
                bevy::log::info!(target: "console", "{typed}: {out}");
            }
            c.print(Level::Info, out)
        }
        Ok(None) => {}
        Err(e) => {
            if log {
                // Not `warn!`/`error!`: the client's log layer copies those
                // back into the console.
                bevy::log::info!(target: "console", "{typed}: error: {e}");
            }
            c.print(Level::Error, e)
        }
    }
}

/// Register cvars and commands on the app.
pub trait ConsoleAppExt {
    fn console_cvar(
        &mut self,
        name: &str,
        help: &str,
        default: &str,
        get: impl Fn(&mut World) -> Option<String> + Send + Sync + 'static,
        set: impl Fn(&mut World, &str) -> Result<(), String> + Send + Sync + 'static,
    ) -> &mut Self;
    fn console_command(
        &mut self,
        name: &str,
        help: &str,
        run: impl Fn(&mut World, &[String]) -> Result<Option<String>, String> + Send + Sync + 'static,
    ) -> &mut Self;
}

impl ConsoleAppExt for App {
    fn console_cvar(
        &mut self,
        name: &str,
        help: &str,
        default: &str,
        get: impl Fn(&mut World) -> Option<String> + Send + Sync + 'static,
        set: impl Fn(&mut World, &str) -> Result<(), String> + Send + Sync + 'static,
    ) -> &mut Self {
        self.init_resource::<Console>();
        self.world_mut().resource_mut::<Console>().add_cvar(Cvar {
            name: name.into(),
            help: help.into(),
            default: default.into(),
            values: if default == "0" || default == "1" {
                vec!["0".into(), "1".into()]
            } else {
                Vec::new()
            },
            archive: false,
            userinfo: false,
            range: None,
            integer: false,
            scope: CvarScope::of(name),
            get: Arc::new(get),
            set: Arc::new(set),
        });
        self
    }

    fn console_command(
        &mut self,
        name: &str,
        help: &str,
        run: impl Fn(&mut World, &[String]) -> Result<Option<String>, String> + Send + Sync + 'static,
    ) -> &mut Self {
        self.init_resource::<Console>();
        self.world_mut().resource_mut::<Console>().add_command(Command {
            name: name.into(),
            help: help.into(),
            run: Arc::new(run),
            complete: None,
        });
        self
    }
}

/// A cvar backed by a field of a resource, parsed and printed with
/// `FromStr`/`Display`.
pub fn resource_cvar<R, T>(app: &mut App, name: &str, help: &str, field: fn(&mut R) -> &mut T)
where
    R: Resource + bevy::ecs::component::Component<Mutability = bevy::ecs::component::Mutable>,
    T: std::str::FromStr + std::fmt::Display + Send + Sync + 'static,
{
    let default = app
        .world_mut()
        .get_resource_mut::<R>()
        .map(|mut r| field(&mut r).to_string())
        .unwrap_or_default();
    app.console_cvar(
        name,
        help,
        &default,
        move |w| {
            // One accessor serves both; reading doesn't count as a change.
            w.get_resource_mut::<R>()
                .map(|mut r| field(r.bypass_change_detection()).to_string())
        },
        move |w, v| {
            let mut r = w.get_resource_mut::<R>().ok_or("not available")?;
            *field(&mut r) = v.trim().parse().map_err(|_| format!("bad value \"{v}\""))?;
            Ok(())
        },
    );
    // Whole numbers only: the type reads "1" but not "0.5". A fractional
    // one isn't on/off even when its default is 0 or 1.
    let integer = "1".parse::<T>().is_ok() && "0.5".parse::<T>().is_err();
    let mut console = app.world_mut().resource_mut::<Console>();
    if let Some(c) = console.cvar_mut(name) {
        c.integer = integer;
        if "0.5".parse::<T>().is_ok() {
            c.values.clear();
        }
    }
}

/// Commands that change the simulation for the one who runs them
/// (Source's FCVAR_CHEAT ones), by full name: in a network game (server
/// or client) only with `sv_cheats 1` on the server (replicated). Single
/// player always has them.
pub const CHEATS: &[&str] = &[
    "chimera:noclip",
    "chimera:god",
    "chimera:setpos",
    "chimera:setang",
    "chimera:give",
    "chimera:impulse",
    "chimera:ent_fire",
    "chimera:hurtme",
    "chimera:sethealth",
    "chimera:setarmor",
    "chimera:setmoney",
    "chimera:ladder_rung",
    "chimera:minigame_win",
];

/// Commands a network client sends to its server to act on its player,
/// with no `sv_cheats` needed (`net::cheats`): CS:S's `kill`; a game adds
/// its own (`Console::player_command`).
pub const PLAYER_COMMANDS: &[&str] = &["chimera:kill"];

/// Impulses that aren't cheats (Source's player impulses): `impulse 100`
/// (the flashlight) and `impulse 201` (the spray). Every other impulse
/// (`impulse 101`: all weapons) is one.
pub const PLAYER_IMPULSES: &[&str] = &["100", "201"];

/// Whether the command `name` (any name `resolve` takes) is in `table`
/// (full names; namespace aliases followed).
pub fn in_table(console: &Console, table: &[&str], name: &str) -> bool {
    let Some(full) = console.full_name(name) else {
        return false;
    };
    table.iter().any(|t| console.canonical_full(t) == full)
}

/// Whether the command `name` with `args` is a cheat (`CHEATS`, but for
/// `PLAYER_IMPULSES`).
pub fn is_cheat(console: &Console, name: &str, args: &[String]) -> bool {
    in_table(console, CHEATS, name)
        && !(in_table(console, &["chimera:impulse"], name)
            && args.first().is_some_and(|a| PLAYER_IMPULSES.contains(&a.trim())))
}

/// Chimera's commands that change the server's game (the map, who
/// plays), by full name: a network client can't run them (its world isn't
/// the game's; the server's operator runs them on the server's console).
/// A game adds its own (`Console::server_command`: CS:S's bots and
/// `mp_restartgame`).
pub const SERVER_COMMANDS: &[&str] = &["chimera:changelevel", "chimera:kick", "chimera:kickid"];

/// `sv_cheats`.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cheats(pub u8);

/// The player a command runs for, while a server runs a network client's
/// (`net::cheats`); else commands act on the local player.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandPlayer(pub Entity);

/// The player a command acts on: the client's whose command the server
/// runs (`CommandPlayer`), else the local player.
pub fn command_player(world: &mut World) -> Result<Entity, String> {
    if let Some(p) = world.get_resource::<CommandPlayer>() {
        return Ok(p.0);
    }
    world
        .query_filtered::<Entity, With<crate::core::LocalPlayer>>()
        .single(world)
        .map_err(|_| "no local player".to_string())
}

/// Whether cheat commands work here: single player, or `sv_cheats 1`.
pub fn cheats_allowed(world: &World) -> bool {
    world
        .get_resource::<crate::core::NetRole>()
        .is_none_or(|r| *r == crate::core::NetRole::Standalone)
        || world.get_resource::<Cheats>().is_some_and(|c| c.0 != 0)
}

/// The core commands.
pub struct ConsolePlugin;

impl Plugin for ConsolePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Console>()
            .init_resource::<Cheats>()
            .add_systems(PreUpdate, (run_queue, local_userinfo).chain());
        builtins(app);
        resource_cvar::<Cheats, u8>(
            app,
            "sv_cheats",
            "1: cheat commands (noclip, god, setpos, give, ...) work in a network game (single player always has them).",
            |c| &mut c.0,
        );
        app.init_resource::<crate::core::NoBlock>();
        resource_cvar::<crate::core::NoBlock, u8>(
            app,
            "chimera:noblock",
            "Players passing through each other (ours; the server's, sent to clients): 0 nobody (CS:S), 1 teammates, 2 everyone.",
            |n| &mut n.0,
        );
        app.world_mut()
            .resource_mut::<Console>()
            .set_range("chimera:noblock", 0.0, 2.0);
    }
}

/// The userinfo cvars' values (`Cvar::userinfo`), by short name: the
/// keys Source's userinfo has (`cl_autowepswitch`), which the simulation
/// reads on any machine.
pub fn userinfo(world: &mut World) -> std::collections::BTreeMap<String, String> {
    let Some(console) = world.get_resource::<Console>() else {
        return Default::default();
    };
    let getters: Vec<(String, GetFn)> = console
        .cvars
        .values()
        .filter(|c| c.userinfo)
        .map(|c| (short_name(&c.name).to_string(), c.get.clone()))
        .collect();
    getters
        .into_iter()
        .filter_map(|(name, get)| Some((name, get(world)?)))
        .collect()
}

/// The local player's `core::UserInfo`: its userinfo cvars' values.
fn local_userinfo(world: &mut World) {
    let local = crate::query_cache::cached_query::<
        (Entity, Option<&crate::core::UserInfo>),
        With<crate::core::LocalPlayer>,
        _,
    >(world, |q, world| {
        q.iter(world).next().map(|(e, have)| (e, have.cloned()))
    });
    let Some((me, have)) = local else { return };
    let info = crate::core::UserInfo(userinfo(world));
    if have.as_ref() != Some(&info) {
        world.entity_mut(me).insert(info);
    }
}

fn console(w: &mut World) -> Mut<'_, Console> {
    w.resource_mut::<Console>()
}

/// Whether a full name or its short name starts with `prefix` (lower
/// case): `cvarlist sv_` and `cvarlist cs_source:sv_` both list
/// `cs_source:sv_gravity`.
fn starts_with_either(name: &str, prefix: &str) -> bool {
    let lower = name.to_lowercase();
    lower.starts_with(prefix) || short_name(&lower).starts_with(prefix)
}

/// Warn about names a stored line left short (`qualify_line_with_hints`).
fn hint(c: &mut Console, hints: Vec<String>) -> Result<Option<String>, String> {
    for h in hints {
        c.print(Level::Warn, h);
    }
    Ok(None)
}

fn builtins(app: &mut App) {
    app.console_command("chimera:echo", "Print text.", |_, a| Ok(Some(a.join(" "))))
        .console_command("chimera:clear", "Clear the console output.", |w, _| {
            console(w).output.clear();
            Ok(None)
        })
        .console_command("chimera:help", "help <name>: what a command or cvar does.", |w, a| {
            let c = w.resource::<Console>();
            let Some(name) = a.first() else {
                return Ok(Some(
                    "Type a command or cvar name: suggestions and the arguments' help show as you type.\n\
                     Up/Down pick a suggestion (or browse history with an empty line), Enter or Tab takes it, Esc hides them;\n\
                     Tab completes and cycles, Ctrl+R searches history, Ctrl+L clears, Ctrl+V pastes.\n\
                     find <text> searches names and help; cvarlist / cmdlist list them; differences shows changed cvars."
                        .into(),
                ));
            };
            if let Some(cmd) = c.command(name) {
                return Ok(Some(format!("{}: {}", cmd.name, cmd.help)));
            }
            if let Some(v) = c.cvar(name) {
                let range = v.range.map_or(String::new(), |(a, b)| format!(", range {a} to {b}"));
                return Ok(Some(format!("{} (cvar, default \"{}\"{range}): {}", v.name, v.default, v.help)));
            }
            if let Some(body) = c.aliases.get(&name.to_lowercase()) {
                return Ok(Some(format!("{name} is an alias for: {body}")));
            }
            Err(format!("no command or cvar \"{name}\""))
        })
        .console_command("chimera:find", "find <text>: cvars and commands whose name or help mention it.", |w, a| {
            let text = a.join(" ").to_lowercase();
            let c = w.resource::<Console>();
            let mut out: Vec<String> = c
                .cvars()
                .filter(|v| v.name.to_lowercase().contains(&text) || v.help.to_lowercase().contains(&text))
                .map(|v| format!("{:<28} cvar  {}", v.name, v.help))
                .chain(
                    c.commands()
                        .filter(|v| v.name.to_lowercase().contains(&text) || v.help.to_lowercase().contains(&text))
                        .map(|v| format!("{:<28} cmd   {}", v.name, v.help)),
                )
                .collect();
            out.sort();
            Ok(Some(if out.is_empty() { format!("nothing matches \"{text}\"") } else { out.join("\n") }))
        })
        .console_command("chimera:cvarlist", "cvarlist [prefix]: cvars with their values.", |w, a| {
            let prefix = a.first().map(|p| p.to_lowercase()).unwrap_or_default();
            let cvars: Vec<Cvar> = w
                .resource::<Console>()
                .cvars()
                .filter(|v| starts_with_either(&v.name, &prefix))
                .cloned()
                .collect();
            let lines: Vec<String> = cvars
                .iter()
                .map(|v| {
                    let value = (v.get)(w).unwrap_or_default();
                    let mark = if value != v.default { "*" } else { " " };
                    format!("{mark} {:<28} = {:<10} {}", v.name, value, v.help)
                })
                .collect();
            Ok(Some(format!("{}\n{} cvars (* changed from default)", lines.join("\n"), lines.len())))
        })
        .console_command("chimera:cmdlist", "cmdlist [prefix]: commands.", |w, a| {
            let prefix = a.first().map(|p| p.to_lowercase()).unwrap_or_default();
            let lines: Vec<String> = w
                .resource::<Console>()
                .commands()
                .filter(|v| starts_with_either(&v.name, &prefix))
                .map(|v| format!("{:<28} {}", v.name, v.help))
                .collect();
            Ok(Some(lines.join("\n")))
        })
        .console_command("chimera:differences", "Cvars changed from their defaults.", |w, _| {
            let cvars: Vec<Cvar> = w.resource::<Console>().cvars().cloned().collect();
            let lines: Vec<String> = cvars
                .iter()
                .filter_map(|v| {
                    let value = (v.get)(w)?;
                    (value != v.default).then(|| format!("{} \"{}\" (default \"{}\")", v.name, value, v.default))
                })
                .collect();
            Ok(Some(if lines.is_empty() { "no cvars changed".into() } else { lines.join("\n") }))
        })
        .console_command("chimera:reset", "reset <cvar>: back to its default.", |w, a| {
            let name = a.first().ok_or("reset <cvar>")?;
            let v = w.resource::<Console>().cvar(name).cloned().ok_or(format!("no cvar \"{name}\""))?;
            (v.set)(w, &v.default)?;
            Ok(Some(format!("{} = \"{}\"", v.name, v.default)))
        })
        .console_command("chimera:resetall", "Every cvar back to its default.", |w, _| {
            let cvars: Vec<Cvar> = w.resource::<Console>().cvars().cloned().collect();
            let mut n = 0;
            for v in &cvars {
                if (v.get)(w).is_some_and(|x| x != v.default) {
                    (v.set)(w, &v.default)?;
                    n += 1;
                }
            }
            Ok(Some(format!("{n} cvars reset")))
        })
        .console_command("chimera:alias", "alias <name> \"<commands>\": a new command; alias alone lists them.", |w, a| {
            let mut c = console(w);
            match a {
                [] => Ok(Some(
                    c.aliases.iter().map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join("\n"),
                )),
                [name] => Ok(c.aliases.get(&name.to_lowercase()).map(|v| format!("{name}: {v}"))),
                [name, rest @ ..] => {
                    let (body, hints) = c.qualify_line_with_hints(&rest.join(" "));
                    c.aliases.insert(name.to_lowercase(), body);
                    c.dirty = true;
                    hint(&mut c, hints)
                }
            }
        })
        .console_command("chimera:toggle", "toggle <cvar> [a b ...]: flip 0/1, or cycle through the values given.", |w, a| {
            let name = a.first().ok_or("toggle <cvar> [values]")?;
            let v = w.resource::<Console>().cvar(name).cloned().ok_or(format!("no cvar \"{name}\""))?;
            let now = (v.get)(w).unwrap_or_default();
            let cycle: Vec<String> = if a.len() > 1 { a[1..].to_vec() } else { vec!["0".into(), "1".into()] };
            let i = cycle.iter().position(|x| x == &now).map_or(0, |i| (i + 1) % cycle.len());
            (v.set)(w, &cycle[i])?;
            Ok(Some(format!("{} = \"{}\"", v.name, cycle[i])))
        })
        .console_command("chimera:incrementvar", "incrementvar <cvar> <min> <max> <step>: add step, wrapping.", |w, a| {
            let [name, min, max, step] = a else {
                return Err("incrementvar <cvar> <min> <max> <step>".into());
            };
            let num = |s: &String| s.parse::<f64>().map_err(|_| format!("bad number \"{s}\""));
            let (min, max, step) = (num(min)?, num(max)?, num(step)?);
            let v = w.resource::<Console>().cvar(name).cloned().ok_or(format!("no cvar \"{name}\""))?;
            let now: f64 = (v.get)(w).and_then(|x| x.parse().ok()).unwrap_or(min);
            let mut next = now + step;
            if next > max + 1e-9 {
                next = min;
            } else if next < min - 1e-9 {
                next = max;
            }
            let text = if next.fract() == 0.0 { format!("{next:.0}") } else { format!("{next}") };
            (v.set)(w, &text)?;
            Ok(Some(format!("{} = \"{text}\"", v.name)))
        })
        .console_command("chimera:wait", "wait [frames]: pause the rest of the queue (default 1 frame).", |w, a| {
            let frames = a.first().and_then(|f| f.parse().ok()).unwrap_or(1);
            console(w).wait = frames;
            Ok(None)
        })
        .console_command("chimera:watch", "watch <cvar>: show its value on screen (unwatch to remove).", |w, a| {
            let typed = a.first().ok_or("watch <cvar>")?;
            let mut c = console(w);
            let name = c.full_name(typed).unwrap_or_else(|| typed.to_lowercase());
            if !c.watches.contains(&name) {
                c.watches.push(name);
            }
            Ok(None)
        })
        .console_command("chimera:unwatch", "unwatch [cvar]: stop showing it (all with no name).", |w, a| {
            let mut c = console(w);
            match a.first() {
                Some(n) => {
                    let name = c.full_name(n).unwrap_or_else(|| n.to_lowercase());
                    c.watches.retain(|x| x != &name)
                }
                None => c.watches.clear(),
            }
            Ok(None)
        })
        .console_command("chimera:exec", "exec <file>: run a config file (cfg folder; .cfg optional).", |w, a| {
            let name = a.first().ok_or("exec <file>")?;
            let text = read_cfg(name).ok_or(format!("couldn't exec {name}"))?;
            run_cfg(w, &text);
            Ok(None)
        })
        .console_command("chimera:execifexists", "execifexists <file>: exec, silently skipped if missing.", |w, a| {
            if let Some(text) = a.first().and_then(|n| read_cfg(n)) {
                run_cfg(w, &text);
            }
            Ok(None)
        })
        .console_command("chimera:bind", "bind <key> [command]: run a command on a key (+action: held).", |w, a| {
            let mut c = console(w);
            match a {
                [] => Err("bind <key> [command]".into()),
                [key] => Ok(Some(match c.binds.get(&key.to_lowercase()) {
                    Some(cmd) => format!("\"{key}\" = \"{cmd}\""),
                    None => format!("\"{key}\" is not bound"),
                })),
                [key, rest @ ..] => {
                    let (line, hints) = c.qualify_line_with_hints(&rest.join(" "));
                    c.binds.insert(key.to_lowercase(), line);
                    c.dirty = true;
                    hint(&mut c, hints)
                }
            }
        })
        .console_command(
            "chimera:bindgame",
            "bindgame <game> <key> [command]: a key's command while <game> is the player's identity, over the \
             shared binds (no command: hides the shared bind there; a bare game and key says what it is).",
            |w, a| {
                let mut c = console(w);
                match a {
                    [] | [_] => Err("bindgame <game> <key> [command]".into()),
                    [game, key] => Ok(Some(match c.layers.get(game).and_then(|l| l.get(&key.to_lowercase())) {
                        Some(cmd) => format!("\"{key}\" = \"{cmd}\" in {game}"),
                        None => format!("\"{key}\" has no bind of its own in {game}"),
                    })),
                    [game, key, rest @ ..] => {
                        let scope = c.canonical_namespace(game);
                        let (line, hints) = c.qualify_line_in(&rest.join(" "), Some(&scope));
                        c.layers.entry(game.clone()).or_default().insert(key.to_lowercase(), line);
                        c.dirty = true;
                        hint(&mut c, hints)
                    }
                }
            },
        )
        .console_command(
            "chimera:scope",
            "scope [game|auto]: the game whose names short names mean first (then chimera's): a game's id or alias \
             (cs_source, css, combat_arms, ca); auto follows the game played (the gun in hand in the party apps).",
            |w, a| {
                let mut c = console(w);
                match a.first().map(|s| s.to_lowercase()) {
                    None => {}
                    Some(x) if x == "auto" => c.scope = None,
                    Some(game) => {
                        if !c.is_namespace(&game) {
                            return Err(format!("no game \"{game}\" (scope <game>: a game's id or alias, or auto)"));
                        }
                        c.scope = Some(c.canonical_namespace(&game));
                    }
                }
                let now = match (&c.scope, &c.auto_scope) {
                    (Some(s), _) => format!("scope: {s} (pinned; scope auto follows the game played)"),
                    (None, Some(s)) => format!("scope: {s} (the game played)"),
                    (None, None) => "scope: none (chimera's names, then names only one game has)".into(),
                };
                Ok(Some(now))
            },
        )
        .console_command("chimera:unbindgameall", "Remove every game's own binds (bindgamedefaults brings the shipped ones back).", |w, _| {
            let mut c = console(w);
            c.layers.clear();
            c.dirty = true;
            Ok(None)
        })
        .console_command("chimera:unbindgame", "unbindgame <game> <key>: the shared bind shows through in <game> again.", |w, a| {
            let [game, key] = a else { return Err("unbindgame <game> <key>".into()) };
            let mut c = console(w);
            if let Some(l) = c.layers.get_mut(game) {
                l.remove(&key.to_lowercase());
            }
            c.dirty = true;
            Ok(None)
        })
        .console_command("chimera:bindgamedefaults", "bindgamedefaults [game]: every game's (or <game>'s) own binds as shipped.", |w, a| {
            let mut c = console(w);
            let defaults = c.layer_defaults.clone();
            match a.first() {
                Some(game) => {
                    c.layers.insert(game.clone(), defaults.get(game).cloned().unwrap_or_default());
                }
                None => c.layers = defaults,
            }
            c.dirty = true;
            Ok(None)
        })
        .console_command("chimera:unbind", "unbind <key>", |w, a| {
            let key = a.first().ok_or("unbind <key>")?.to_lowercase();
            let mut c = console(w);
            c.binds.remove(&key);
            c.dirty = true;
            Ok(None)
        })
        .console_command("chimera:unbindall", "Remove every bind.", |w, _| {
            let mut c = console(w);
            c.binds.clear();
            c.dirty = true;
            Ok(None)
        })
        .console_command("chimera:bindlist", "Every bind.", |w, _| {
            let c = w.resource::<Console>();
            Ok(Some(c.binds.iter().map(|(k, v)| format!("{k:<12} \"{v}\"")).collect::<Vec<_>>().join("\n")))
        })
        .console_command("chimera:host_writeconfig", "host_writeconfig [file]: save binds, aliases and changed cvars (config.cfg).", |w, a| {
            let name = a.first().map_or("config.cfg", |s| s.as_str());
            let text = config_text(w);
            let path = cfg_dir().ok_or("no config folder")?.join(with_cfg(name));
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            std::fs::write(&path, text).map_err(|e| e.to_string())?;
            console(w).dirty = false;
            Ok(Some(format!("wrote {}", path.display())))
        });
}

/// Where configs live: the user's data folder (Linux ~/.local/share/chimera/cfg,
/// Windows %APPDATA%\\chimera\\cfg; the old `mashup` folder while only it
/// exists, `env::user_folder`). `CHIMERA_CFG_DIR` names another folder
/// (a throwaway one for tests and agents).
pub fn cfg_dir() -> Option<std::path::PathBuf> {
    if let Some(dir) = crate::env::var_os("CHIMERA_CFG_DIR") {
        return Some(dir.into());
    }
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("APPDATA").map(std::path::PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share")))?;
    Some(crate::env::user_folder(&base).join("cfg"))
}

fn with_cfg(name: &str) -> String {
    if name.to_lowercase().ends_with(".cfg") {
        name.to_string()
    } else {
        format!("{name}.cfg")
    }
}

/// A config file's text: an absolute or relative path as given, else from
/// the cfg folder.
pub fn read_cfg(name: &str) -> Option<String> {
    let given = std::path::Path::new(name);
    let candidates = [
        given.to_path_buf(),
        given.with_extension("cfg"),
        cfg_dir()?.join(with_cfg(name)),
    ];
    candidates.iter().find_map(|p| std::fs::read_to_string(p).ok())
}

/// Run a config's lines before anything else queued, in file order.
fn run_cfg(w: &mut World, text: &str) {
    let mut c = console(w);
    for line in text.lines().rev() {
        if !line.trim().is_empty() {
            c.queue.push_front(line.to_string());
        }
    }
}

/// Binds, aliases and changed cvars, as a config file.
pub fn config_text(w: &mut World) -> String {
    // The second line tells the client every key's bind is in here
    // (`client::binds::CONFIG_MARK`), the third that the defaults it was
    // written over were the game's current ones (`client::binds::DEFAULTS_MARK`;
    // its text, "cs", from when CS:S's changed, kept for saved configs).
    let mut out = String::from("// Written by Chimera (host_writeconfig).\n// binds: all\n// bind defaults: cs\n");
    let (binds, layers, aliases, cvars) = {
        let c = w.resource::<Console>();
        (
            c.binds.clone(),
            c.layers.clone(),
            c.aliases.clone(),
            c.cvars().cloned().collect::<Vec<_>>(),
        )
    };
    let console = w.resource::<Console>();
    let qualified = |line: &str, scope: Option<&str>| console.qualify_line_in(line, scope).0;
    let scope = console.current_scope();
    out += "unbindall\n";
    for (k, v) in binds {
        if let Some(v) = console.saved_bind(&v) {
            out += &format!("bind {} {}\n", quote(&k), quote(&qualified(&v, scope)));
        }
    }
    // The games' own binds: all of them anew, so one removed stays removed.
    out += "unbindgameall\n";
    for (game, keys) in &layers {
        let game_scope = console.canonical_namespace(game);
        for (k, v) in keys {
            out += &format!(
                "bindgame {} {} {}\n",
                quote(game),
                quote(k),
                quote(&qualified(v, Some(&game_scope)))
            );
        }
    }
    for (k, v) in aliases {
        out += &format!("alias {} {}\n", quote(&k), quote(&qualified(&v, scope)));
    }
    for v in cvars.into_iter().filter(|v| v.archive) {
        if let Some(value) = (v.get)(w)
            && value != v.default
        {
            out += &format!("{} {}\n", v.name, quote(&value));
        }
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A URL on the command line (`+chimera:asset_url https://...`) keeps
    /// its `//`: quoted, it isn't a comment.
    #[test]
    fn a_url_survives_quoting_and_parsing() {
        let line = ["chimera:asset_url", "https://203.0.113.5:27031"].map(quote).join(" ");
        assert_eq!(parse(&line), [vec!["chimera:asset_url".to_string(), "https://203.0.113.5:27031".to_string()]]);
        assert_eq!(quote("plain"), "plain");
    }

    #[test]
    fn cvar_scopes_follow_the_server_prefixes() {
        assert_eq!(CvarScope::of("sv_gravity"), CvarScope::Replicated);
        assert_eq!(CvarScope::of("mp_friendlyfire"), CvarScope::Replicated);
        assert_eq!(CvarScope::of("mp_decals"), CvarScope::Local, "the player's own");
        assert_eq!(CvarScope::of("spec_freeze_time"), CvarScope::Replicated);
        assert_eq!(CvarScope::of("spec_freeze_traveltime"), CvarScope::Replicated);
        assert_eq!(CvarScope::of("cl_disablefreezecam"), CvarScope::Local);
        assert_eq!(CvarScope::of("bot_stop"), CvarScope::Server);
        assert_eq!(CvarScope::of("sv_password"), CvarScope::Server);
        assert_eq!(CvarScope::of("cl_crosshairscale"), CvarScope::Local);
        assert_eq!(CvarScope::of("sensitivity"), CvarScope::Local);
    }

    #[test]
    fn usage_and_argument_positions() {
        let (args, text) = usage("setpos", "setpos <x> <y> <z>: move there (CS:S units).");
        assert_eq!(args, ["<x>", "<y>", "<z>"]);
        assert_eq!(text, "move there (CS:S units).");
        // Brackets keep their spaces and colons.
        let (args, text) = usage(
            "chimera_hurtme",
            "chimera_hurtme <head|chest> [amount=100] [from yaw, degrees: 0 = front]: a hit on yourself.",
        );
        assert_eq!(args, ["<head|chest>", "[amount=100]", "[from yaw, degrees: 0 = front]"]);
        assert_eq!(text, "a hit on yourself.");
        // Help without a usage is all description.
        assert_eq!(
            usage("god", "Toggle taking no damage."),
            (vec![], "Toggle taking no damage.".to_string())
        );
        assert_eq!(
            usage("give", "Give the local player a weapon by ID (e.g. give x).").0,
            Vec::<String>::new()
        );
        // A name that only starts the help isn't a usage.
        assert_eq!(usage("bot", "bot_add [team]: add a bot.").0, Vec::<String>::new());
        assert_eq!(arg_position("setpos"), None);
        assert_eq!(arg_position("setpos "), Some(0));
        assert_eq!(arg_position("setpos 1"), Some(0));
        assert_eq!(arg_position("setpos 1 2 "), Some(2));
        assert_eq!(last_command_start("echo a; sv_gr"), 8);
        assert_eq!(last_command_start("bind x \"+jump; say\" "), 0);
        assert_eq!(last_command_start("noclip"), 0);
    }

    #[test]
    fn parsing() {
        assert_eq!(parse("a b; c \"d e\""), vec![vec!["a", "b"], vec!["c", "d e"]]);
        assert_eq!(
            parse("bind x \"+jump; say hi\" // note"),
            vec![vec!["bind", "x", "+jump; say hi"]]
        );
        assert_eq!(parse("  ;; "), Vec::<Vec<String>>::new());
        assert_eq!(parse("echo \"\""), vec![vec!["echo", ""]]);
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(ConsolePlugin);
        #[derive(Resource)]
        struct Speed(f32);
        app.insert_resource(Speed(250.0));
        resource_cvar::<Speed, f32>(&mut app, "sv_speed", "a speed", |s| &mut s.0);
        app
    }

    pub(crate) fn run(app: &mut App, line: &str) -> Vec<String> {
        let start = app.world().resource::<Console>().output.len();
        app.world_mut().resource_mut::<Console>().submit(line);
        app.update();
        app.world().resource::<Console>().output[start..]
            .iter()
            .map(|l| l.text.clone())
            .collect()
    }

    #[test]
    fn cvars_aliases_and_builtins() {
        let mut app = app();
        assert!(run(&mut app, "sv_speed")[0].contains("\"250\""));
        run(&mut app, "sv_speed 300");
        assert!(run(&mut app, "differences")[0].contains("sv_speed \"300\""));
        run(&mut app, "alias fast \"sv_speed 500; echo zoom\"");
        assert_eq!(run(&mut app, "fast"), vec!["zoom"]);
        assert!(run(&mut app, "sv_speed")[0].contains("\"500\""));
        run(&mut app, "reset sv_speed");
        assert!(run(&mut app, "sv_speed")[0].contains("\"250\""));
        run(&mut app, "toggle sv_speed 250 320");
        assert!(run(&mut app, "sv_speed")[0].contains("\"320\""));
        run(&mut app, "incrementvar sv_speed 250 330 10");
        assert!(run(&mut app, "sv_speed")[0].contains("\"330\""));
        run(&mut app, "incrementvar sv_speed 250 330 10");
        assert!(run(&mut app, "sv_speed")[0].contains("\"250\""), "wraps");
        assert!(run(&mut app, "nope")[0].contains("Unknown command"));
        assert!(run(&mut app, "find speed").iter().any(|l| l.contains("sv_speed")));
    }

    #[test]
    fn config_round_trip() {
        let mut first = app();
        // Only archived cvars are saved.
        first.world_mut().resource_mut::<Console>().archive("sv_speed");
        run(
            &mut first,
            "bind space \"+jump\"; bind mwheeldown +jump; alias hop \"echo hi; wait\"; sv_speed 320",
        );
        let text = config_text(first.world_mut());
        assert!(text.contains("bind space +jump"), "{text}");
        assert!(text.contains("alias hop \"echo hi; wait\""), "{text}");
        assert!(
            text.contains("\ncs_source:sv_speed 320\n"),
            "saved by full name: {text}"
        );
        // A fresh console running the file ends up the same.
        let mut other = app();
        run_cfg(other.world_mut(), &text);
        other.update();
        let c = other.world().resource::<Console>();
        assert_eq!(c.binds.get("space").map(String::as_str), Some("+jump"));
        assert_eq!(c.aliases.get("hop").map(String::as_str), Some("echo hi; wait"));
        assert!(run(&mut other, "sv_speed")[0].contains("\"320\""));
        // Not archived: not saved.
        other
            .world_mut()
            .resource_mut::<Console>()
            .cvars
            .get_mut("cs_source:sv_speed")
            .unwrap()
            .archive = false;
        assert!(!config_text(other.world_mut()).contains("sv_speed"));
    }

    /// An app with the same short names in several namespaces: `x` in
    /// both games, `y` in Combat Arms and Chimera, `only` in CS:S alone.
    pub(crate) fn namespaced() -> App {
        let mut app = app();
        #[derive(Resource, Default)]
        struct V([u8; 5]);
        app.init_resource::<V>();
        resource_cvar::<V, u8>(&mut app, "cs_source:x", "CS:S's x", |v| &mut v.0[0]);
        resource_cvar::<V, u8>(&mut app, "combat_arms:x", "Combat Arms' x", |v| &mut v.0[1]);
        resource_cvar::<V, u8>(&mut app, "chimera:y", "ours", |v| &mut v.0[2]);
        resource_cvar::<V, u8>(&mut app, "combat_arms:y", "Combat Arms' y", |v| &mut v.0[3]);
        resource_cvar::<V, u8>(&mut app, "only", "CS:S's alone (registered bare)", |v| &mut v.0[4]);
        let mut c = app.world_mut().resource_mut::<Console>();
        c.add_namespace(&["cs_source", "css"]);
        c.add_namespace(&["combat_arms", "ca"]);
        app
    }

    fn found(app: &App, name: &str) -> Resolved {
        app.world().resource::<Console>().resolve(name)
    }

    fn is(full: &str) -> Resolved {
        Resolved::Found(full.into())
    }

    #[test]
    fn short_names_resolve_by_scope_then_chimera_then_alone() {
        let mut app = namespaced();
        // No scope: Chimera's first, then the only one; `x` is ambiguous.
        assert_eq!(found(&app, "y"), is("chimera:y"));
        assert_eq!(found(&app, "only"), is("cs_source:only"));
        assert_eq!(
            found(&app, "x"),
            Resolved::Ambiguous(vec!["combat_arms:x".into(), "cs_source:x".into()])
        );
        // Full names always work, with aliases for the namespace.
        assert_eq!(found(&app, "combat_arms:x"), is("combat_arms:x"));
        assert_eq!(found(&app, "CA:X"), is("combat_arms:x"));
        assert_eq!(found(&app, "css:x"), is("cs_source:x"));
        assert_eq!(found(&app, "ca:only"), Resolved::Unknown);
        // The scope's game first, then Chimera's.
        run(&mut app, "scope ca");
        assert_eq!(found(&app, "x"), is("combat_arms:x"));
        assert_eq!(found(&app, "y"), is("combat_arms:y"));
        assert_eq!(found(&app, "only"), is("cs_source:only"));
        run(&mut app, "x 7");
        assert!(run(&mut app, "combat_arms:x")[0].contains("\"7\""));
        assert!(run(&mut app, "cs_source:x")[0].contains("\"0\""));
        run(&mut app, "scope cs_source");
        assert_eq!(found(&app, "x"), is("cs_source:x"));
        assert_eq!(found(&app, "y"), is("chimera:y"));
        // The automatic scope when none is pinned (`console_scope`).
        run(&mut app, "scope auto");
        app.world_mut().resource_mut::<Console>().auto_scope = Some("combat_arms".into());
        assert_eq!(found(&app, "x"), is("combat_arms:x"));
        assert!(run(&mut app, "scope")[0].contains("combat_arms (the game played)"));
        assert!(run(&mut app, "scope nogame")[0].contains("no game"));
    }

    #[test]
    fn an_ambiguous_short_name_prints_a_hint() {
        let mut app = namespaced();
        let out = run(&mut app, "x 1");
        assert!(
            out[0].contains("\"x\" is ambiguous: combat_arms:x, cs_source:x") && out[0].contains("scope <game>"),
            "{out:?}"
        );
        // Nothing was set.
        assert!(run(&mut app, "cs_source:x")[0].contains("\"0\""));
        // A bind of it keeps the short name and says why.
        let out = run(&mut app, "bind k \"x 1\"");
        assert!(out.iter().any(|l| l.contains("is ambiguous")), "{out:?}");
        assert_eq!(app.world().resource::<Console>().binds["k"], "x 1");
    }

    #[test]
    fn old_names_still_work() {
        let mut app = app();
        #[derive(Resource, Default)]
        struct Ours(u8, u8);
        app.init_resource::<Ours>();
        // Registered under their old names, as plugins did.
        resource_cvar::<Ours, u8>(&mut app, "chimera:hitmarker", "", |o| &mut o.0);
        resource_cvar::<Ours, u8>(&mut app, "chimera:watchbot", "", |o| &mut o.1);
        let c = app.world().resource::<Console>();
        assert_eq!(c.cvar("cl_hitmarker").unwrap().name, "chimera:hitmarker");
        assert_eq!(c.cvar("hitmarker").unwrap().name, "chimera:hitmarker");
        assert_eq!(c.cvar("chimera_watch").unwrap().name, "chimera:watchbot");
        assert_eq!(c.cvar("mashup_watch").unwrap().name, "chimera:watchbot");
        assert_eq!(c.cvar("chimera:noblock").unwrap().name, "chimera:noblock");
        assert_eq!(c.cvar("chimera_noblock").unwrap().name, "chimera:noblock");
        assert_eq!(c.cvar("MASHUP_NOBLOCK").unwrap().name, "chimera:noblock");
        // The built-in `watch` keeps its name.
        assert_eq!(c.command("watch").unwrap().name, "chimera:watch");
        // Old names aren't offered.
        let names = c.names();
        assert!(names.contains(&"noblock".to_string()) && !names.iter().any(|n| n.starts_with("chimera_")));
        run(&mut app, "chimera_noblock 2; cl_hitmarker 1");
        assert!(run(&mut app, "noblock")[0].contains("\"2\""));
        assert!(run(&mut app, "chimera:hitmarker")[0].contains("\"1\""));
        // A bind or alias of an old name is stored with the new one.
        run(
            &mut app,
            "bind n \"chimera_noblock 1\"; alias nb \"mashup_noblock 0; echo off\"",
        );
        let c = app.world().resource::<Console>();
        assert_eq!(c.binds["n"], "noblock 1");
        assert_eq!(c.aliases["nb"], "noblock 0; echo off");
    }

    #[test]
    fn configs_read_short_names_and_save_full_ones() {
        let mut first = namespaced();
        {
            let mut c = first.world_mut().resource_mut::<Console>();
            for n in ["sv_speed", "combat_arms:x", "chimera:y", "ca:y"] {
                c.archive(n);
            }
            c.keep_short("slot1");
        }
        // A config from before namespaces: short names, in a scope.
        let old = "bind space +jump\nbind 1 slot1\nbind f \"only 1; echo hi\"\nscope ca\nx 3\nsv_speed 320\ny 4\n";
        run_cfg(first.world_mut(), old);
        first.update();
        let text = config_text(first.world_mut());
        for line in [
            "bind space +jump",
            "bind 1 slot1",
            "bind f \"cs_source:only 1; echo hi\"",
            "\ncombat_arms:x 3\n",
            "\ncs_source:sv_speed 320\n",
            "\ncombat_arms:y 4\n",
        ] {
            assert!(text.contains(line), "{line}: {text}");
        }
        // Another app in another scope reads it back the same.
        let mut other = namespaced();
        run(&mut other, "scope css");
        run_cfg(other.world_mut(), &text);
        other.update();
        assert!(run(&mut other, "combat_arms:x")[0].contains("\"3\""));
        assert!(run(&mut other, "combat_arms:y")[0].contains("\"4\""));
        assert!(run(&mut other, "chimera:y")[0].contains("\"0\""));
        assert!(run(&mut other, "sv_speed")[0].contains("\"320\""));
        assert_eq!(
            other.world().resource::<Console>().binds["f"],
            "cs_source:only 1; echo hi"
        );
    }

    #[test]
    fn binds_store_full_names_in_their_scope() {
        let mut app = namespaced();
        run(
            &mut app,
            "scope combat_arms; bind a \"x 1\"; bind b +attack; bind c \"toggle y\"",
        );
        // A game's own binds qualify in that game's scope.
        run(&mut app, "bindgame cs_source d \"x 2\"");
        let c = app.world().resource::<Console>();
        assert_eq!(c.binds["a"], "combat_arms:x 1");
        assert_eq!(c.binds["b"], "+attack", "actions stay as typed");
        assert_eq!(
            c.binds["c"], "toggle y",
            "Chimera's unshared names stay short (arguments as typed)"
        );
        assert_eq!(c.layers["cs_source"]["d"], "cs_source:x 2");
    }

    /// The engine's Source names are `chimera:`; the full names saved
    /// before the move (`cs_source:fps_max`, `css:fps_max`) still work, in
    /// configs, binds and tables, while game-tuned names stay CS:S's.
    #[test]
    fn engine_names_moved_to_chimera_and_the_old_full_names_still_work() {
        #[derive(Resource, Default)]
        struct W([u8; 3]);
        let mut app = namespaced();
        app.init_resource::<W>();
        resource_cvar::<W, u8>(&mut app, "fps_max", "engine", |v| &mut v.0[0]);
        resource_cvar::<W, u8>(&mut app, "sv_gravity", "tuned per game", |v| &mut v.0[1]);
        resource_cvar::<W, u8>(&mut app, "cs_source:r_shadows", "engine, said the old way", |v| &mut v.0[2]);
        assert_eq!(found(&app, "fps_max"), is("chimera:fps_max"));
        assert_eq!(found(&app, "sv_gravity"), is("cs_source:sv_gravity"));
        assert_eq!(found(&app, "r_shadows"), is("chimera:r_shadows"));
        // The old full names are aliases (the namespace's alias too).
        assert_eq!(found(&app, "cs_source:fps_max"), is("chimera:fps_max"));
        assert_eq!(found(&app, "CSS:FPS_MAX"), is("chimera:fps_max"));
        assert_eq!(found(&app, "cs_source:sv_gravity"), is("cs_source:sv_gravity"));
        assert_eq!(found(&app, "chimera:sv_gravity"), Resolved::Unknown, "game-tuned stays per game");
        // Short names find the engine's from any scope.
        run(&mut app, "scope ca");
        assert_eq!(found(&app, "fps_max"), is("chimera:fps_max"));
        // A config written before the move sets the engine's value.
        run(&mut app, "cs_source:fps_max 144; css:sv_gravity 9");
        let c = app.world().resource::<Console>();
        assert_eq!(c.cvar("chimera:fps_max").unwrap().name, "chimera:fps_max");
        assert!(run(&mut app, "chimera:fps_max")[0].contains("\"144\""));
        // Tables of full names take both spellings.
        let c = app.world().resource::<Console>();
        assert!(in_table(c, &["chimera:fps_max"], "cs_source:fps_max"));
        assert!(in_table(c, &["cs_source:fps_max"], "chimera:fps_max"));
        // A saved bind keeps working and is written with the new name.
        assert_eq!(c.qualify_line("cs_source:fps_max 60"), "fps_max 60");
        assert_eq!(c.cvar_scope("cs_source:sv_cheats"), CvarScope::Replicated);
    }

    #[test]
    fn a_game_alias_can_become_canonical() {
        let mut app = namespaced();
        // The user picks `css` as CS:S's canonical name.
        app.world_mut()
            .resource_mut::<Console>()
            .add_namespace(&["css", "cs_source"]);
        let c = app.world().resource::<Console>();
        assert_eq!(c.cvar("cs_source:x").unwrap().name, "css:x");
        assert_eq!(c.cvar("sv_speed").unwrap().name, "css:sv_speed");
        assert_eq!(c.cvar("sv_cheats").unwrap().scope, CvarScope::Replicated);
        assert!(in_table(c, CHEATS, "css:noclip") || c.command("noclip").is_none());
        assert_eq!(c.canonical_full("cs_source:buy"), "css:buy");
        // Engine names stay Chimera's whichever way the old namespace is spelled.
        assert_eq!(c.canonical_full("css:kill"), "chimera:kill");
        assert_eq!(c.canonical_full("kill"), "chimera:kill");
    }

    #[test]
    fn wait_pauses_the_queue() {
        let mut app = app();
        assert_eq!(run(&mut app, "echo a; wait 2; echo b"), vec!["a"]);
        // Two frames of waiting, then the rest of the line runs.
        let mut frames = 0;
        while !app.world().resource::<Console>().output.iter().any(|l| l.text == "b") {
            app.update();
            frames += 1;
            assert!(frames <= 3, "b never ran");
        }
        assert_eq!(frames, 3);
    }
}
