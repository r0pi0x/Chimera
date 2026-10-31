//! Minigames (docs/plans/active/architecture-review.md D2, L2): a match
//! described by a data file (`def::MinigameDef`: players, the map pool,
//! teams, the time limit, the gameplay's slots, spawning, a rules module
//! and its parameters, modifiers), played on the mode runtime (`mode`)
//! by Chimera's own side of it (`runtime::FLOW`), so no game's rules run
//! while one plays (the gameplay's rules slot is `MINIGAME`).
//!
//! - Definitions: parsed at run time (`MinigameAppExt::add_minigames`,
//!   a game or app's built-in files; `minigame_load <file>` from disk),
//!   kept by id (`Minigames`), checked against what the app registered
//!   when added and started (`check`: the rules module and its own
//!   parameters, every weapon in the catalogue and in a set the
//!   definition allows, the slots' games and movement, the modifiers).
//! - Rules modules (`RulesModule`): a mode (`mode::Mode`, its id the
//!   module's name) plus how it reads its parameters, what it gives each
//!   life and its own checks, registered by name; it hears of a join and
//!   of a life's arming as entity events (`Joined`, `Arming`, triggered
//!   by the runtime where they happen), which other parts may observe too
//!   (`MinigameAppExt::add_minigame_rules`); built in, `chimera:ladder`
//!   (`ladder`). A minigame the built-in modules can't express registers
//!   its own (`games/originals`).
//! - Modifiers (`Modifier`): a named hook a definition turns on with a
//!   value (`[modifiers]`), registered by name; none are built in yet.
//! - Starting one (`start`, `minigame <id>`): the definition becomes the
//!   `ActiveMinigame`, the gameplay switches to it (rules `MINIGAME`; the
//!   bots, HUD, binds and tick from its slots, the current ones where it
//!   names none), the match starts over (`mode::Match`), every player joins
//!   again, the module and the modifiers start.
//! - The primitives the runtime gives every minigame (`runtime`): joining
//!   (teams as the definition makes them), the match waiting for
//!   `players.min`, spawning and respawning with protection at the
//!   definition's points, the weapons of a life (the kit, then the
//!   module's), deaths counted (`core::Score`, the mode's `on_death`), the
//!   time limit, a single round whose winner wins the match, then the
//!   match over (`minigame_restart` plays it again). `MatchHold` holds
//!   the match (no joins, respawns or phases) while a server changes map
//!   for the next one; `start` lifts it.
//! - `MinigameEvent`: named events (`rung_up`, `rung_down`, `match_won`,
//!   the ladder's `last_rung`, `suicide_down` and `lead_taken`,
//!   `lead_lost`, `lead_tied`)
//!   for a HUD or an announcer; `hud::MinigameHud` is what the local
//!   player's HUD shows of the minigame (status lines from the rules
//!   module, the match's time, notices from the events): data a game's HUD
//!   draws; and its big callouts (`hud::Callout`, from the module's
//!   `callout`), which the client shell draws and sounds with the
//!   definition's cue pack (`cues`, `chimera_core::cues`).
//!   `moments::MinigameMoments` is the match's moments for a game's music
//!   director (the start of play, the last rung, the respawn countdown,
//!   the result), the game the definition's `music` picks.
//! - The host's settings (`overrides::MinigameOverrides`, cvars an app
//!   registers): keys of any definition set when its match starts.
//! - Console: `minigames` (the definitions, the playing one marked),
//!   `minigame <id>`, `minigame_load <file>`, `minigame_status`,
//!   `minigame_restart`; cheats (`sv_cheats`): `ladder_rung <n>` (the
//!   local player onto a rung), `minigame_win` (the local player wins).
//!
//! - Results: once the match is decided `results::MatchResults` (winner,
//!   each player's final rung, kills and deaths) is kept for the HUDs
//!   (`hud::MinigameHud::results`) and replicated; the match over begins
//!   the match-end hold (`results::NextMatch`, the shell's match-end
//!   screen counts it down); after `runtime::RESULTS_HOLD` s of it a
//!   definition with `rotate = true` (the default) moves on to the next
//!   map of its pool (`chimera_net::minigame_net`): in turn, or at random
//!   skipping the maps played lately (`rotation`: `rotation = "random"`,
//!   `MapHistory`).
//!   With `map_vote` the players vote for it on the end screen
//!   (`vote::MapVote`: the rotation's pick and a few other maps).
//! - Matches won: `wins::MatchWins` counts a decided match for its winner
//!   by name for as long as the process runs (kept over `restart`, a
//!   rotation and `change_match`), `wins::MatchesWon` carries it on each
//!   player (replicated) for the minigame scoreboard.
//!
//! - Automatic gun games (`auto`): a ladder and a map pool made from the
//!   parts every composed game lends (`chimera_core::game_parts`), checked
//!   against the app (`auto::report`), as a definition's text; its
//!   rungs follow the power curve (`power`: bands by class, shares of the
//!   ladder, a power score from each gun's data; `ladder_power` prints it).
//!
//! Known shortcuts: one round per match; the map pool's first map is the
//! app's to load (the map vote is for the next match's); `character` is informational
//! (the character kit isn't a slot yet), `movement` is set on each
//! player at its spawn (with `identity = "weapon"`, the movement and body
//! are the held weapon's game's: `identity`); teams other than the mode's are refused, not
//! adapted; no objective primitives (zones, carriables), scoring table
//! beyond kills and deaths, or built-in modifiers yet; the HUD data is the
//! local player's only (never a spectated player's). Over the network a
//! client plays the server's definition: the match manifest carries its id,
//! hash and text, which a client without the same one adds (`add`,
//! `Minigames::text`; `chimera_net::minigame_net`); a client can't start or
//! restart one itself.

pub mod auto;
pub mod def;
pub mod holders;
pub mod hud;
pub mod ladder;
pub mod moments;
pub mod overrides;
pub mod results;
pub mod rotation;
pub mod power;
pub mod runtime;
pub mod spawns;
pub mod vote;
pub mod wins;

use bevy::prelude::*;

pub use def::MinigameDef;
pub use results::{MatchResults, NextMatch};
pub use runtime::{InMinigame, JoinOrder, Protected};

use crate::{
    console::ConsoleAppExt,
    core::Score,
    mode::{self, Match, Mode, ModeAppExt, ModeGame},
    weapon::data::WeaponCatalog,
};
use chimera_core::{
    game_registry::Games,
    gameplay::{self, Gameplay},
    slots::MovementRegistry,
};

/// The gameplay's rules slot while a minigame plays: the mode runtime's
/// game for Chimera's own flow (`runtime::FLOW`).
pub const MINIGAME: &str = "chimera";

/// A rules module: a mode on the runtime, with its parameters' reading.
#[derive(Clone, Copy)]
pub struct RulesModule {
    /// The mode (its `id` is the module's name, `game:name`).
    pub mode: &'static Mode,
    /// The module's own checks of a definition (its `[rules]` keys, the
    /// weapons it names: each in the catalogue and allowed).
    pub check: fn(&MinigameDef, &WeaponCatalog) -> Result<(), String>,
    /// The definition starts: read its parameters into the world.
    pub start: fn(&mut World, &MinigameDef),
    /// The weapons a life gets from the rules (after the kit; the last
    /// given is drawn).
    pub loadout: fn(&World, Entity) -> Vec<&'static str>,
    /// The life gets the rules' weapons alone, without the definition's
    /// kit (the ladder's knife-only last rung).
    pub no_kit: fn(&World, Entity) -> bool,
    /// A notice for the viewer from one of the module's events.
    pub notice: fn(&World, Entity, &MinigameEvent) -> Option<hud::HudLine>,
    /// A big callout for the viewer from one of the module's events
    /// (`hud::Callout`: centre screen, with a cue's sound).
    pub callout: fn(&World, Entity, &MinigameEvent) -> Option<hud::Callout>,
}

/// A player joins the match while rules module `rules` plays: triggered
/// on it by the runtime's join (`runtime::join`, with `World::trigger`,
/// then a flush: observers' commands apply at once) after its team was
/// made, before its first spawn. The module's start for the player (the
/// ladder's join rung, `ladder::joined`); anything else may observe it.
/// The module's observer filters by `rules` (its name).
#[derive(EntityEvent, Clone, Copy, Debug)]
pub struct Joined {
    #[event_target]
    pub player: Entity,
    /// The playing rules module's name (`mode.id`).
    pub rules: &'static str,
}

/// A life is about to get its weapons while rules module `rules` plays:
/// triggered on the player by `runtime::respawn` (`World::trigger`, then a
/// flush) after its old weapons were taken, before the kit and the
/// module's `loadout` are given. The module's arming of it (the ladder's
/// rung extras, `ladder::arm`, the only observer that may change what the
/// life is given); a game's look or a mode's kit may observe it too. Several
/// observers have no order (plan ecs-cleanup O3): their queued world work
/// all applies in the flush, before the weapons.
#[derive(EntityEvent, Clone, Copy, Debug)]
pub struct Arming {
    #[event_target]
    pub player: Entity,
    /// The playing rules module's name (`mode.id`).
    pub rules: &'static str,
}

/// A modifier a definition turns on by name with a value.
#[derive(Clone, Copy)]
pub struct Modifier {
    pub name: &'static str,
    pub check: fn(&toml::Value) -> Result<(), String>,
    /// The minigame starts with it on.
    pub apply: fn(&mut World, &toml::Value),
}

/// The definitions, the rules modules and the modifiers, in the order
/// added.
#[derive(Resource, Clone, Default)]
pub struct Minigames {
    /// Each definition with the text it was read from (what a network
    /// server offers its clients: `chimera_net::minigame_net`).
    defs: Vec<(&'static MinigameDef, &'static str)>,
    rules: Vec<RulesModule>,
    modifiers: Vec<Modifier>,
}

impl Minigames {
    pub fn get(&self, id: &str) -> Option<&'static MinigameDef> {
        self.defs.iter().find(|(d, _)| d.id == id).map(|(d, _)| *d)
    }

    /// The text definition `id` was read from.
    pub fn text(&self, id: &str) -> Option<&'static str> {
        self.defs.iter().find(|(d, _)| d.id == id).map(|(_, t)| *t)
    }

    pub fn all(&self) -> impl Iterator<Item = &'static MinigameDef> + '_ {
        self.defs.iter().map(|(d, _)| *d)
    }

    pub fn rules(&self, name: &str) -> Option<RulesModule> {
        self.rules.iter().find(|r| r.mode.id == name).copied()
    }

    pub fn modifier(&self, name: &str) -> Option<Modifier> {
        self.modifiers.iter().find(|m| m.name == name).copied()
    }

    /// Add `def`, read from `text` (one of the same id replaced in place).
    fn put(&mut self, def: &'static MinigameDef, text: &'static str) {
        match self.defs.iter_mut().find(|(d, _)| d.id == def.id) {
            Some(d) => *d = (def, text),
            None => self.defs.push((def, text)),
        }
    }
}

/// Add a definition from its text (`name` for errors), checked against
/// what the app registered (`check`); one of the same id is replaced.
/// `minigame_load`'s, and a network client's for the server's minigame
/// (`chimera_net::minigame_net`).
pub fn add(world: &mut World, name: &str, text: &str) -> Result<&'static MinigameDef, String> {
    let def = MinigameDef::parse(name, text)?;
    check(world, &def)?;
    let def: &'static MinigameDef = Box::leak(Box::new(def));
    let text: &'static str = Box::leak(text.to_string().into_boxed_str());
    world.get_resource_or_init::<Minigames>().put(def, text);
    Ok(def)
}

/// The minigame playing (the server's).
#[derive(Resource, Clone, Copy)]
pub struct ActiveMinigame {
    pub def: &'static MinigameDef,
    pub rules: RulesModule,
}

/// The names `MinigameEvent`s have, in a fixed order: a network server
/// sends an event's place in it (`chimera_net::minigame_net`). A rules module's own
/// event names go at the end.
pub const EVENT_NAMES: &[&str] = &[
    "rung_up",
    "rung_down",
    "match_won",
    "last_rung",
    "lead_taken",
    "lead_lost",
    "lead_tied",
    "suicide_down",
];

/// A named moment of the match, for a HUD or an announcer.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct MinigameEvent {
    /// `rung_up`, `rung_down`, `suicide_down`, `match_won`, `last_rung`,
    /// `lead_taken`, ...
    pub name: &'static str,
    pub player: Option<Entity>,
    /// The event's number (a rung, a score).
    pub value: i32,
}

/// What the app registered that a definition must agree with.
pub fn check(world: &World, def: &MinigameDef) -> Result<(), String> {
    let fail = |e: String| format!("{}: {e}", def.id);
    let minigames = world.get_resource::<Minigames>().ok_or("no minigames registered")?;
    let rules = minigames
        .rules(&def.rules.module)
        .ok_or_else(|| fail(format!("no rules module `{}`", def.rules.module)))?;
    if rules.mode.teams != def.teams.teams() {
        return Err(fail(format!("teams {:?}: {} plays {:?}", def.teams, rules.mode.id, rules.mode.teams)));
    }
    let empty = WeaponCatalog::default();
    let catalog = world.get_resource::<WeaponCatalog>().unwrap_or(&empty);
    (rules.check)(def, catalog).map_err(fail)?;
    for id in def.spawn.kit.all() {
        weapon(def, catalog, id).map_err(fail)?;
    }
    for (name, value) in &def.modifiers {
        let m = minigames.modifier(name).ok_or_else(|| fail(format!("no modifier `{name}`")))?;
        (m.check)(value).map_err(|e| fail(format!("modifier {name}: {e}")))?;
    }
    if let Some(games) = world.get_resource::<Games>() {
        let slots = [
            ("hud", &def.slots.hud),
            ("binds", &def.slots.binds),
            ("tick", &def.slots.tick),
            ("bots", &def.slots.bots),
        ];
        for (slot, game) in slots {
            if let Some(g) = game
                && games.get(g).is_none()
            {
                return Err(fail(format!("slots.{slot}: no game `{g}` registered")));
            }
        }
    }
    if let Some(c) = &def.cues
        && let Some(packs) = world.get_resource::<chimera_core::cues::CuePacks>()
        && !packs.valid(c)
    {
        return Err(fail(format!("cues: no cue pack `{c}` registered (or none, default)")));
    }
    if let Some(m) = def.music.as_deref().filter(|m| !m.eq_ignore_ascii_case("none"))
        && let Some(parts) = world.get_resource::<chimera_core::game_parts::PartsRegistry>()
        && parts.get(m).is_none_or(|p| !p.music)
    {
        return Err(fail(format!("music: no game `{m}` lends a music director (or none)")));
    }
    if let Some(m) = &def.slots.movement
        && world.get_resource::<MovementRegistry>().is_some_and(|r| r.get(m).is_none())
    {
        return Err(fail(format!("slots.movement: no movement `{m}` registered")));
    }
    // Following the weapon: each weapon set's game moves its own way.
    if def.slots.identity == Some(def::IdentityDef::Weapon)
        && let Some(registry) = world.get_resource::<MovementRegistry>()
        && let Some(g) = def.slots.weapons.iter().find(|g| registry.of_game(g).is_none())
    {
        return Err(fail(format!("slots.identity = \"weapon\": no movement of `{g}` registered ({g}:...)")));
    }
    Ok(())
}

/// Weapon `id` from the catalogue, in a set `def` allows.
pub fn weapon(def: &MinigameDef, catalog: &WeaponCatalog, id: &str) -> Result<&'static str, String> {
    if !def.allows_weapon(id) {
        return Err(format!("weapon {id}: not from the sets the definition allows ({:?})", def.slots.weapons));
    }
    catalog
        .get(id)
        .map(|w| w.id.as_str())
        .ok_or_else(|| format!("weapon {id}: not in the weapon catalogue"))
}

/// Start minigame `id`: its definition (with the host's overrides,
/// `overrides`) checked and made the active one, the gameplay switched to
/// it, a fresh match that every player joins (a `MatchHold` lifted).
pub fn start(world: &mut World, id: &str) -> Result<&'static MinigameDef, String> {
    if !client(world) {
        overrides::apply(world, id);
    }
    let def = begin(world, id, true)?;
    world.remove_resource::<MatchHold>();
    Ok(def)
}

/// The match held (a server changing map for its next match:
/// `chimera_net::minigame_net`): while it is in, no one joins the match
/// or respawns and its phases don't step, so nothing starts on the map
/// being left nor before the players have the new one. `start` lifts it.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct MatchHold;

/// Whether the match is held (`MatchHold`).
pub fn held(world: &World) -> bool {
    world.contains_resource::<MatchHold>()
}

/// A network client's `start`: the minigame the server plays, made the
/// local gameplay (its slots, the rules module's parameters, the
/// modifiers) with the match and the players as the server replicated
/// them (`chimera_net::minigame_net`), not started over.
pub fn join_remote(world: &mut World, id: &str) -> Result<&'static MinigameDef, String> {
    begin(world, id, false)
}

fn begin(world: &mut World, id: &str, fresh: bool) -> Result<&'static MinigameDef, String> {
    let def = world
        .get_resource::<Minigames>()
        .and_then(|m| m.get(id))
        .ok_or_else(|| format!("no minigame `{id}` (minigames lists them)"))?;
    check(world, def)?;
    let rules = world.resource::<Minigames>().rules(&def.rules.module).expect("checked");
    let games = world.get_resource::<Games>();
    let game_id = |slot: &Option<String>| slot.as_ref().and_then(|g| games.and_then(|gs| gs.get(g))).map(|g| g.id);
    let current = world.get_resource::<Gameplay>().cloned();
    let tick = def
        .slots
        .tick
        .as_ref()
        .and_then(|g| games.and_then(|gs| gs.get(g)))
        .map(|g| g.tick())
        .or(current.as_ref().map(|c| c.tick))
        .unwrap_or_else(|| world.resource::<Time<Fixed>>().timestep());
    let gameplay = Gameplay {
        rules: MINIGAME,
        bots: game_id(&def.slots.bots).or(current.as_ref().map(|c| c.bots)).unwrap_or(MINIGAME),
        hud: game_id(&def.slots.hud).or(current.as_ref().map(|c| c.hud)).unwrap_or(MINIGAME),
        binds: game_id(&def.slots.binds).or(current.as_ref().map(|c| c.binds)).unwrap_or(MINIGAME),
        tick,
    };
    gameplay::switch_gameplay(world, gameplay);
    world.insert_resource(ActiveMinigame { def, rules });
    if fresh {
        restart(world);
    }
    world.remove_resource::<crate::weapon::drop::NoDeathDrops>();
    (rules.start)(world, def);
    let modifiers: Vec<(Modifier, toml::Value)> = def
        .modifiers
        .iter()
        .filter_map(|(n, v)| world.resource::<Minigames>().modifier(n).map(|m| (m, v.clone())))
        .collect();
    for (m, v) in modifiers {
        (m.apply)(world, &v);
    }
    Ok(def)
}

/// The match from the start: waiting for players, every player joining
/// again with nothing scored.
pub fn restart(world: &mut World) {
    world.insert_resource(Match::default());
    world.remove_resource::<MatchResults>();
    world.remove_resource::<NextMatch>();
    world.remove_resource::<vote::MapVote>();
    runtime::lift_end_freeze(world);
    world.insert_resource(mode::flow::MissionFrames::default());
    spawns::reset(world);
    let players: Vec<Entity> = world.query_filtered::<Entity, With<InMinigame>>().iter(world).collect();
    for p in players {
        runtime::unprotect(world, p);
        world.entity_mut(p).remove::<InMinigame>().insert(Score::default());
    }
}

/// Registering minigames, their rules modules and modifiers.
pub trait MinigameAppExt {
    /// Definitions as `(name, text)` (a file's name for errors), parsed
    /// now; each is checked against the registry when it starts.
    fn add_minigames(&mut self, docs: &[(&str, &str)]) -> Result<&mut Self, String>;
    /// A rules module (its mode registered with the runtime under
    /// `MINIGAME`).
    fn add_minigame_rules(&mut self, rules: RulesModule) -> &mut Self;
    fn add_modifier(&mut self, modifier: Modifier) -> &mut Self;
}

impl MinigameAppExt for App {
    fn add_minigames(&mut self, docs: &[(&str, &str)]) -> Result<&mut Self, String> {
        for (name, text) in docs {
            let def = MinigameDef::parse(name, text)?;
            let def: &'static MinigameDef = Box::leak(Box::new(def));
            let text: &'static str = Box::leak(text.to_string().into_boxed_str());
            self.world_mut().get_resource_or_init::<Minigames>().put(def, text);
        }
        Ok(self)
    }

    fn add_minigame_rules(&mut self, rules: RulesModule) -> &mut Self {
        self.add_modes(
            ModeGame {
                game: MINIGAME,
                chosen: |w| w.get_resource::<ActiveMinigame>().map(|a| a.rules.mode.key),
                flow: runtime::FLOW,
            },
            &[rules.mode],
        );
        let mut minigames = self.world_mut().get_resource_or_init::<Minigames>();
        match minigames.rules.iter_mut().find(|r| r.mode.id == rules.mode.id) {
            Some(r) => *r = rules,
            None => minigames.rules.push(rules),
        }
        self
    }

    fn add_modifier(&mut self, modifier: Modifier) -> &mut Self {
        let mut minigames = self.world_mut().get_resource_or_init::<Minigames>();
        match minigames.modifiers.iter_mut().find(|m| m.name == modifier.name) {
            Some(m) => *m = modifier,
            None => minigames.modifiers.push(modifier),
        }
        self
    }
}

/// Why a client can't start or restart a minigame.
const SERVER_PICKS: &str = "the server picks the minigame and starts its matches; you are a client of it";

/// This process is a client of a server (which plays the minigame).
fn client(w: &World) -> bool {
    w.get_resource::<crate::core::NetRole>() == Some(&crate::core::NetRole::Client)
}

/// Minigames: the registry, the runtime's systems (the server's, while a
/// minigame is the gameplay), the built-in `chimera:ladder` and the
/// console. An app (or a test) adds it on its simulation; none is
/// started until `start` (`minigame <id>`).
pub struct MinigamePlugin;

impl Plugin for MinigamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Minigames>()
            .init_resource::<Match>()
            .add_message::<MinigameEvent>()
            .add_message::<crate::core::Died>()
            // The ladder's reaction to a death (`mode::ModeDeath`), a
            // join (`Joined`) and a life's arming (`Arming`).
            .add_observer(ladder::died)
            .add_observer(ladder::joined)
            .add_observer(ladder::arming)
            .init_resource::<hud::MinigameHud>()
            .init_resource::<moments::MinigameMoments>()
            .init_resource::<wins::MatchWins>()
            .init_resource::<ladder::LaddersMade>()
            .add_systems(Update, (hud::update, ladder::status_systems()))
            .add_minigame_rules(ladder::RULES);
        // A map loaded while a minigame is the rules: counted
        // (`core::MapLoads`: a network client waits on the count after a
        // map change, the kill feed starts over); no game's fresh game
        // runs (a server's map change restarts the match itself).
        gameplay::GameplayAppExt::game_hook(
            app,
            MINIGAME,
            gameplay::NewGame(|w| w.get_resource_or_init::<crate::core::MapLoads>().0 += 1),
        );
        if !app.is_plugin_added::<crate::identity::IdentityPlugin>() {
            app.add_plugins(crate::identity::IdentityPlugin);
        }
        runtime::plugin(app);
        power::plugin(app);
        app.console_command(
            "minigames",
            "List the minigames (id: name, players, maps, rules module), the one playing marked *.",
            |w, _| {
                let playing = w.get_resource::<ActiveMinigame>().map(|a| a.def.id.as_str());
                let lines: Vec<String> = w
                    .resource::<Minigames>()
                    .all()
                    .map(|d| {
                        let mark = if Some(d.id.as_str()) == playing { "*" } else { " " };
                        format!(
                            "{mark} {}: {} ({}-{} players, {}; {})",
                            d.id,
                            d.name,
                            d.players.min,
                            d.players.max,
                            d.maps.join(" "),
                            d.rules.module
                        )
                    })
                    .collect();
                Ok(Some(if lines.is_empty() { "no minigames".into() } else { lines.join("\n") }))
            },
        )
        .console_command(
            "minigame",
            "minigame <id>: play that minigame on the map loaded (a fresh match; its rules, slots and spawns); \
             no argument: the one playing.",
            |w, a| match a.first() {
                None => Ok(Some(
                    w.get_resource::<ActiveMinigame>()
                        .map_or("no minigame playing".into(), |m| format!("{}: {}", m.def.id, m.def.name)),
                )),
                Some(_) if client(w) => Err(SERVER_PICKS.into()),
                Some(id) => start(w, id).map(|d| Some(format!("playing {}: {}", d.id, d.name))),
            },
        )
        .console_command(
            "minigame_load",
            "minigame_load <file>: read a minigame definition (TOML) from disk, check it and add it.",
            |w, a| {
                let path = a.first().ok_or("minigame_load <file>")?;
                let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
                let def = add(w, path, &text)?;
                Ok(Some(format!("added {}", def.id)))
            },
        )
        .console_command("minigame_restart", "Play the minigame's match again from the start.", |w, _| {
            if client(w) {
                return Err(SERVER_PICKS.into());
            }
            if !w.contains_resource::<ActiveMinigame>() {
                return Err("no minigame playing".into());
            }
            restart(w);
            Ok(Some("match restarted".into()))
        })
        .console_command(
            "minigame_status",
            "The minigame's match: phase, time left, each player's kills, deaths and the rules' standing.",
            |w, _| runtime::status(w).map(Some),
        )
        .console_command(
            "ladder_rung",
            "ladder_rung <n>: put the local player on the ladder's rung n (from 1), its weapon given (a cheat; \
             screenshots and tests).",
            |w, a| {
                let n: usize = a.first().and_then(|n| n.parse().ok()).filter(|n| *n >= 1).ok_or("ladder_rung <n>")?;
                let ladder = w.get_resource::<ladder::Ladder>().cloned().ok_or("no ladder plays")?;
                let me = local_player(w)?;
                let rung = (n - 1).min(ladder.rungs.len().saturating_sub(1));
                ladder::set_rung(w, me, rung, &ladder);
                Ok(Some(format!("rung {}: {}", rung + 1, ladder.rungs[rung])))
            },
        )
        .console_command(
            "minigame_win",
            "minigame_win: the local player wins the match now (a cheat; the end screen for screenshots).",
            |w, _| {
                let me = local_player(w)?;
                let team = w.get::<crate::core::Team>(me).map(|t| t.0).ok_or("the local player has no team")?;
                w.get_resource_mut::<Match>().ok_or("no match")?.win(team);
                Ok(Some("match won".into()))
            },
        );
    }
}

/// The local player's character (the console cheats').
fn local_player(world: &mut World) -> Result<Entity, String> {
    world
        .query_filtered::<Entity, With<chimera_core::core::LocalPlayer>>()
        .iter(world)
        .next()
        .ok_or_else(|| "no local player".to_string())
}
