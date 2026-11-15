//! Halo's own match: its game types (specs/halo_ce/multiplayer_rules.md)
//! on Chimera's mode runtime (`chimera_sim::mode`), in Halo's app and its
//! dedicated server.
//!
//! **Design.**
//! - *The game type is data*: a game variant (`blam_formats::game_type`:
//!   the base type, teams, the flags, respawn times, lives, health, score
//!   to win, weapon and vehicle sets, the base type's options), chosen by
//!   the `halo_ce:gametype` cvar (`+gametype ctf` on a command line; a
//!   base word, a stock name such as `slayer_pro` or `iron_ctf`, or a saved
//!   file's path: `variant`) and cleaned as the game cleans it (1.4). The
//!   disc holds no saved variants: its 26 stock types are the executable's
//!   (section 2), `blam_formats::game_type::STOCK_GAME_TYPES`. The chosen
//!   variant is read when a game begins (`new_game`: a map loaded as a new
//!   game, an app's first map with `AutoStart`, `halo_ce:restart`) and is
//!   `HaloGameType` until the next. The PC game's time limit and friendly
//!   fire (1.6) are cvars beside it (`halo_ce:timelimit`,
//!   `halo_ce:friendlyfire`).
//! - *One mode per base type and teams* (`halo_ce:slayer`, `team_slayer`,
//!   `ctf`, `king`, `team_king`, `oddball`, `team_oddball`) is registered with the runtime, which steps the match
//!   through Halo's flow (`FLOW`: every tick, no waiting, one round from
//!   the game's start to its end, the round over being the 7 s postgame,
//!   the match over after it, `direct`); the end test is the runtime's
//!   `decide` hook (`decide`: the score to win, one team left, the time
//!   limit), so `chimera:modes`, the runtime's `Match` (its two sides'
//!   scores: red and blue) and its `ModeDeath`/`ModeStep` events serve
//!   Halo as every game. Teams: red is `mode::ALPHA`, blue `BRAVO` (the
//!   maps' capture the flag spawns' `Team(1)` and `Team(2)`); free for all,
//!   a team id of one's own (`Teams::FreeForAll`).
//! - *A player* (`HaloPlayer`, put on once when a character joins, with
//!   `core::Score` and the movement's `SpeedMultiplier`; nothing is put on
//!   or taken off per event but `Dead`, the engine's "no unit") holds the
//!   spec's per-player state: slot, score, kills, deaths, the respawn timer
//!   and penalty, the multi-kill and spree counters, kill in order's target.
//!   `spawning` brings players in (4.1 to 4.6), `scoring` counts deaths and
//!   slayer's score (6.1, 6.2, 4.2), `slayer` its per-tick work, `items`
//!   the weapon set's substitutions (5.3: the pickups' item clock reads
//!   `items::ItemRules`), `netgame` the map's starts, flags and starting
//!   equipment (the loader writes them as map entities); the objective
//!   game types are `ctf` (the flags, 6.4), `king` (the hill, 6.6) and
//!   `oddball` (the ball, 6.5), each set up by its mode's `setup` hook and
//!   stepped each Halo tick after the spawns.
//! - *Over the network* the engine's seams carry what they can: a player's
//!   score and deaths are `core::Score` (the network's `NetScore`), its
//!   team `core::Team`; the rest a client needs (the game type, the team
//!   scores, the clock, the postgame and the winner) is one replicated
//!   entity, `view::HaloMatchView`, the net part `halo_ce_match`.
//! - *Only Halo's apps play it*: the plugin is in `HaloPlugins` (never in
//!   `BUNDLE`'s composed simulation: a party app's Halo maps and guns keep
//!   their party's rules), and its systems run while Halo is the gameplay
//!   and a game is on (`rules_on`). `HaloSim` alone starts none (its
//!   scenario tests put characters where they want); tests start one with
//!   `start`.
//!
//! The per-tick order (section 10): the game's deaths after the tick's
//! damage (`scoring::deaths`), then at the next tick's rules: the map's
//! data followed, joiners in, the players' spawns (`spawning::
//! players_tick`), the game type's per-player update, the runtime's step
//! (the end test, the postgame), the dead held; the player settings (no
//! shields, invisible players) after the vitality's update; the health
//! setting divides every hit on a Halo player before it is delivered.
//!
//! Known shortcuts: race is not played (`supported`: it needs the race
//! vehicles); the vehicles are not deleted at the game's end (they belong
//! to `vehicles`, which can read the game type's set from `HaloGameType`);
//! friendly fire is on or off (the PC's shields-only and explosives-only
//! modes and the team killer's penalty are not played); auto team balance
//! is not played; the results screen is the HUD's game-over line.

pub mod ctf;
pub mod items;
pub mod king;
pub mod netgame;
pub mod oddball;
pub mod scoring;
pub mod slayer;
pub mod spawning;
pub mod variant;
pub mod view;

use bevy::prelude::*;
use blam_formats::game_type::{BaseGameType, GameVariant, Rules, STOCK_GAME_TYPES};
use chimera_sim::mode::{self, Flow, FlowEvent, FlowOptions, Hooks, Match, Mode, ModeAppExt, ModeGame, NO_HOOKS, Span, Teams};

use super::weapons::random::HaloRng;
use chimera_core::console::{ConsoleAppExt, resource_cvar};

use crate::{
    core::{Connecting, Dead, FriendlyFire, Health, Intent, Npc, Score, SimClock, SimSet, Team},
};

/// Red and blue (the runtime's two sides).
pub const RED: u8 = mode::ALPHA;
pub const BLUE: u8 = mode::BRAVO;
/// Seconds from a game's end to the results (6.3).
pub const POSTGAME_SECONDS: f64 = 7.0;
/// Player slots (the Xbox's 16).
pub const SLOTS: u8 = 16;

/// The rules' settings (cvars): `respawn::HaloRules`, which the game
/// type's choice, time limit and friendly fire extend (an app inserts it).
pub use super::respawn::HaloRules;

/// The game type in play (cleaned).
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct HaloGameType(pub GameVariant);

impl Default for HaloGameType {
    fn default() -> Self {
        Self(STOCK_GAME_TYPES[0].variant())
    }
}

/// The match's state beyond the runtime's `Match`.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct HaloMatch {
    /// A game is on (begun, not replaced).
    pub on: bool,
    /// Game time: Halo ticks since the game began.
    pub time: i64,
    /// Players when the game began (never goes down: later joiners raise
    /// it; the grenade rules, 4.6, 5.3).
    pub players_at_start: u32,
    /// The game is over: the postgame, then the results.
    pub over: bool,
    /// The postgame is done: the results.
    pub results: bool,
    /// Odd man out's place: the player who died most recently.
    pub odd_man: Option<Entity>,
    /// The game's generator (spawn draws, targets, starting equipment).
    pub rng: u32,
}

impl HaloMatch {
    /// A uniform draw in [0, 1].
    pub fn real(&mut self) -> f32 {
        let mut r = HaloRng(self.rng);
        let v = r.real();
        self.rng = r.0;
        v
    }

    /// An integer in [lo, hi).
    pub fn int_range(&mut self, lo: i32, hi: i32) -> i32 {
        let mut r = HaloRng(self.rng);
        let v = r.int_range(lo, hi);
        self.rng = r.0;
        v
    }

    /// An integer in [0, n).
    pub fn roll(&mut self, n: u32) -> u32 {
        self.int_range(0, n.max(1) as i32) as u32
    }
}

/// A Halo player's state in the match (module doc).
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct HaloPlayer {
    /// The player table's index (0 to 15).
    pub slot: u8,
    /// The player's score (slayer: kills less suicides and betrayals;
    /// capture the flag: captures; king and oddball: ticks).
    pub score: i32,
    pub kills: u32,
    pub deaths: u32,
    pub suicides: u32,
    /// Friendly fire credits (betrayals and suicides).
    pub betrayals: u32,
    pub kills_in_row: u32,
    pub multi_kill: u32,
    /// Game time of the last kill.
    pub last_kill: Option<i64>,
    /// Ticks left before a respawn (4.2).
    pub respawn: i32,
    /// The respawn penalty P (4.2).
    pub penalty: i32,
    /// Kill in order's target.
    pub target: Option<Entity>,
    /// The grenades of the last spawn (infinite grenades keeps them).
    pub grenades: [u8; 2],
}

/// Out of lives (a lives game whose deaths reached them).
pub fn out_of_lives(r: &Rules, p: &HaloPlayer) -> bool {
    r.number_of_lives > 0 && p.deaths >= r.number_of_lives as u32
}

/// An app's Halo plays its rules from the first map that loads
/// (`new_game`); `HaloSim` alone doesn't.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AutoStart(pub bool);

/// Whether a game is on (and Halo the gameplay: the systems' other gate).
pub fn rules_on(m: Option<Res<HaloMatch>>) -> bool {
    m.is_some_and(|m| m.on)
}

/// The base types a game can play now (race needs the race vehicles).
pub fn supported(base: BaseGameType) -> bool {
    base != BaseGameType::Race
}

/// The modes (module doc).
pub static SLAYER: Mode = Mode {
    id: "halo_ce:slayer",
    key: "slayer",
    shown_as: "Slayer",
    description: "Halo's free-for-all slayer: kills score, the score to win ends the game.",
    respawns: true,
    warmup: false,
    warmup_revives: false,
    forced_setup: Some(0.0),
    teams: Teams::FreeForAll,
    on_death: |_, _| {},
    time_out: |_| {},
    match_end: |w| Some(w.resource::<Match>().winner),
    hooks: Hooks { decide, round_over: game_over, ..NO_HOOKS },
};
pub static TEAM_SLAYER: Mode = Mode {
    id: "halo_ce:team_slayer",
    key: "team_slayer",
    shown_as: "Team Slayer",
    description: "Red against blue: a team's kills score.",
    teams: Teams::Two,
    ..SLAYER
};
pub static CTF: Mode = Mode {
    id: "halo_ce:ctf",
    key: "ctf",
    shown_as: "Capture the Flag",
    description: "Red against blue: take the enemy flag to your own.",
    teams: Teams::Two,
    hooks: Hooks { decide, round_over: game_over, setup: ctf::setup, ..NO_HOOKS },
    ..SLAYER
};

pub static KING: Mode = Mode {
    id: "halo_ce:king",
    key: "king",
    shown_as: "King of the Hill",
    description: "Hold the hill: time on it scores.",
    hooks: Hooks { decide, round_over: game_over, setup: king::setup, ..NO_HOOKS },
    ..SLAYER
};
pub static TEAM_KING: Mode = Mode {
    id: "halo_ce:team_king",
    key: "team_king",
    shown_as: "Team King",
    description: "Red against blue on the hill.",
    teams: Teams::Two,
    ..KING
};
pub static ODDBALL: Mode = Mode {
    id: "halo_ce:oddball",
    key: "oddball",
    shown_as: "Oddball",
    description: "Hold the ball: time with it scores.",
    hooks: Hooks { decide, round_over: game_over, setup: oddball::setup, ..NO_HOOKS },
    ..SLAYER
};
pub static TEAM_ODDBALL: Mode = Mode {
    id: "halo_ce:team_oddball",
    key: "team_oddball",
    shown_as: "Team Ball",
    description: "Red against blue for the ball.",
    teams: Teams::Two,
    ..ODDBALL
};

/// Halo's side of the runtime (module doc).
pub const FLOW: Flow = Flow {
    frame: 0.0,
    players,
    respawn: |w, e| {
        spawning::spawn_now(w, e);
    },
    length,
    event,
    options: FlowOptions { slack: -1e-9, wait_for_players: false, timer_ends_round: false, direct: true, count_at_start: true },
};

/// The players in the game, in slot order.
pub fn players(world: &mut World) -> Vec<(Entity, u8)> {
    let mut all: Vec<(u8, Entity, u8)> = world
        .query_filtered::<(Entity, &HaloPlayer, &Team), Without<Connecting>>()
        .iter(world)
        .map(|(e, p, t)| (p.slot, e, t.0))
        .collect();
    all.sort();
    all.into_iter().map(|(_, e, t)| (e, t)).collect()
}

fn length(world: &mut World, span: Span) -> Option<f64> {
    match span {
        Span::Setup => Some(0.0),
        Span::Round => world.get_resource::<HaloRules>().map(|r| r.timelimit as f64 * 60.0).filter(|t| *t > 0.0),
        Span::RoundOver => Some(POSTGAME_SECONDS),
        _ => None,
    }
}

fn event(world: &mut World, event: FlowEvent) {
    if event == FlowEvent::MatchEnd {
        results(world);
    }
}

/// The game's end test (6.2, each live tick; also the PC time limit):
/// the score to win reached, fewer than two teams with players alive or
/// with lives (more than one player), or the time up; the winner then the
/// higher team score (teams) or the one first on the score (free for
/// all), a tie nobody.
pub fn decide(world: &mut World, time_up: bool) {
    let r = world.resource::<HaloGameType>().0.rules;
    let limit = variant::winning_score(&r);
    let list: Vec<(u8, i32, bool)> = world
        .query::<(&HaloPlayer, &Team, Has<Dead>)>()
        .iter(world)
        .map(|(p, t, dead)| (t.0, p.score, !dead || !out_of_lives(&r, p)))
        .collect();
    let team_scores = world.resource::<Match>().score;
    let reached = limit > 0
        && if r.team_play { team_scores.iter().any(|s| *s >= limit) } else { list.iter().any(|(_, s, _)| *s >= limit) };
    let mut teams: Vec<u8> = list.iter().filter(|(_, _, in_play)| *in_play).map(|(t, ..)| *t).collect();
    teams.sort();
    teams.dedup();
    let one_left = list.len() > 1 && teams.len() < 2;
    if !(reached || one_left || time_up) {
        return;
    }
    let winner = if r.team_play {
        match team_scores[0].cmp(&team_scores[1]) {
            std::cmp::Ordering::Greater => Some(RED),
            std::cmp::Ordering::Less => Some(BLUE),
            std::cmp::Ordering::Equal => None,
        }
    } else {
        let best = list.iter().map(|(_, s, _)| *s).max();
        let first: Vec<u8> = list.iter().filter(|(_, s, _)| Some(*s) == best).map(|(t, ..)| *t).collect();
        (first.len() == 1).then(|| first[0])
    };
    let mut m = world.resource_mut::<Match>();
    match winner {
        Some(t) => m.win(t),
        None => m.end_without_winner(),
    }
}

/// The game ends (6.3): the postgame (the runtime's round over, 7 s), the
/// "game over" line; play goes on, nobody spawns.
fn game_over(world: &mut World) {
    world.resource_mut::<HaloMatch>().over = true;
    world.write_message(scoring::HaloAnnounce { line: scoring::Line::GameOver, player: None });
}

/// The postgame's end (6.3): every player's unit killed (no killer, no
/// score), the results.
fn results(world: &mut World) {
    let now = world.resource::<SimClock>().now;
    let alive: Vec<Entity> = world.query_filtered::<Entity, (With<HaloPlayer>, Without<Dead>)>().iter(world).collect();
    for e in alive {
        if let Some(mut h) = world.get_mut::<Health>(e) {
            h.current = 0.0;
        }
        world.entity_mut(e).insert((Dead { since: now }, avian3d::prelude::ColliderDisabled));
    }
    world.resource_mut::<HaloMatch>().results = true;
}

/// Characters not yet in the game come in: a slot, a score, the speed
/// multiplier, a team as the mode makes them, and a spawn at once (no
/// unit until then).
#[allow(clippy::type_complexity)]
fn join(
    new: Query<Entity, (With<Intent>, With<Health>, With<Team>, Without<HaloPlayer>, Without<Connecting>, Without<Npc>)>,
    mut commands: Commands,
) {
    for e in &new {
        commands.queue(move |w: &mut World| join_one(w, e));
    }
}

/// `e` joins the game (module doc).
pub fn join_one(world: &mut World, e: Entity) {
    if world.get::<HaloPlayer>(e).is_some() || world.get_entity(e).is_err() {
        return;
    }
    let taken: Vec<u8> = world.query::<&HaloPlayer>().iter(world).map(|p| p.slot).collect();
    let Some(slot) = (0..SLOTS).find(|s| !taken.contains(s)) else { return };
    world.entity_mut(e).insert((
        HaloPlayer { slot, ..default() },
        Score::default(),
        super::movement::SpeedMultiplier(1.0),
    ));
    let count = taken.len() as u32 + 1;
    {
        let mut m = world.resource_mut::<HaloMatch>();
        m.players_at_start = m.players_at_start.max(count);
    }
    if let Some(mut ir) = world.get_resource_mut::<items::ItemRules>() {
        ir.players = ir.players.max(count);
    }
    set_team(world, e);
    if world.resource::<HaloMatch>().over || !spawning::spawn_now(world, e) {
        let now = world.resource::<SimClock>().now;
        world.entity_mut(e).insert((Dead { since: now }, avian3d::prelude::ColliderDisabled));
        if let Some(mut p) = world.get_mut::<HaloPlayer>(e) {
            p.respawn = 1;
        }
    }
}

/// `e`'s team as the game makes them (3): in a team game the side with
/// fewer players (red on a tie), else a team id of its own; one it has
/// that fits is kept.
pub fn set_team(world: &mut World, e: Entity) {
    let Some((m, flow)) = mode::current(world) else { return };
    let team = world.get::<Team>(e).map_or(0, |t| t.0);
    if team == u8::MAX {
        return;
    }
    let others: Vec<u8> = (flow.players)(world).into_iter().filter(|(p, _)| *p != e).map(|(_, t)| t).collect();
    let now = if mode::plays(team) { team } else { 0 };
    if let Some(want) = mode::team_for(m.teams, now, &others, false)
        && want != team
    {
        world.entity_mut(e).insert(Team(want));
    }
}

/// A fresh game on the map just loaded (`gameplay::NewGame`).
pub fn new_game(world: &mut World) {
    start(world);
}

/// Begin a game now with the chosen game type (module doc): its variant
/// read and cleaned, the match from its start (the runtime's first step
/// respawns everyone at a start), every player's score at 0, the items'
/// clock again.
pub fn start(world: &mut World) {
    let rules = world.get_resource::<HaloRules>().cloned().unwrap_or_default();
    begin(world, rules);
}

/// Begin a game of `gametype` now (a test's, a tool's), whatever the
/// settings say (they are kept).
pub fn start_as(world: &mut World, gametype: &str) {
    let mut rules = world.get_resource::<HaloRules>().cloned().unwrap_or_default();
    rules.gametype = gametype.into();
    begin(world, rules);
}

fn begin(world: &mut World, rules: HaloRules) {
    let mut v = match variant::by_name(&rules.gametype) {
        Ok(v) if supported(v.rules.base()) => v,
        Ok(v) => {
            warn!("halo_ce: {} ({:?}) is not played yet: slayer instead", v.name, v.rules.base());
            STOCK_GAME_TYPES[0].variant()
        }
        Err(e) => {
            warn!("halo_ce: {e}: slayer instead");
            STOCK_GAME_TYPES[0].variant()
        }
    };
    v.rules = variant::clean(v.rules);
    let r = v.rules;
    // The variant's respawn numbers, as the settings show them.
    if let Some(mut s) = world.get_resource_mut::<HaloRules>() {
        s.respawn_time = r.respawn_time;
        s.growth = r.respawn_time_growth;
        s.suicide = r.suicide_penalty;
    }
    let players = world.query::<&HaloPlayer>().iter(world).count() as u32;
    let rng = world.get_resource::<HaloMatch>().map_or(0x5eed, |m| m.rng);
    world.insert_resource(HaloGameType(v));
    world.insert_resource(HaloMatch { on: true, players_at_start: players, rng, ..default() });
    world.insert_resource(Match::default());
    world.insert_resource(crate::core::FreezeTime(false));
    world.insert_resource(items::ItemRules { rules: r, players, game: variant::game_type(&r) });
    world.insert_resource(FriendlyFire(rules.friendlyfire));
    super::weapons::pickups::restart(world);
    ctf::clear(world);
    oddball::clear(world);
    let all: Vec<Entity> = world.query_filtered::<Entity, With<HaloPlayer>>().iter(world).collect();
    for e in all {
        let slot = world.get::<HaloPlayer>(e).map_or(0, |p| p.slot);
        world.entity_mut(e).insert((HaloPlayer { slot, ..default() }, Score::default()));
        if let Some(mut m) = world.get_mut::<super::movement::SpeedMultiplier>(e) {
            m.0 = 1.0;
        }
    }
}

fn auto_start(auto: Option<Res<AutoStart>>, m: Option<Res<HaloMatch>>, map: Option<Res<crate::map::MapEntities>>, mut commands: Commands) {
    if auto.is_some_and(|a| a.0) && !m.is_some_and(|m| m.on) && map.is_some() {
        commands.queue(start);
    }
}

/// One Halo tick of the rules (exclusive): the players' spawns, the game
/// type's per-player update, the clock.
fn halo_tick(clock: Res<SimClock>, mut commands: Commands) {
    for _ in 0..super::ticks::halo_ticks(&clock) {
        commands.queue(tick_once);
    }
}

/// One Halo tick's world work: the spawns (revivals, weapons given), the
/// game type's objects, the clock.
fn tick_once(world: &mut World) {
    spawning::players_tick(world);
    match world.resource::<HaloGameType>().0.rules.base() {
        BaseGameType::Slayer => slayer::tick(world),
        BaseGameType::CaptureTheFlag => ctf::tick(world),
        BaseGameType::KingOfTheHill => king::tick(world),
        BaseGameType::Oddball => oddball::tick(world),
        BaseGameType::Race => {}
    }
    world.resource_mut::<HaloMatch>().time += 1;
}

/// The variant's health divides every hit on a Halo player (4.7).
/// With oddball's traits: extra damage dealt, damage resistance taken.
fn health_setting(
    mut hits: MessageMutator<super::damage::HaloHit>,
    players: Query<(), With<HaloPlayer>>,
    game: Res<HaloGameType>,
    balls: Query<&oddball::HaloBall>,
    state: Option<Res<oddball::OddballState>>,
) {
    let h = game.0.rules.health.clamp(0.25, 4.0);
    let odd = oddball::options(&game.0.rules);
    if h == 1.0 && odd.is_none() {
        return;
    }
    let holds = |e: Entity| balls.iter().any(|b| b.carrier == Some(e)) || state.as_ref().is_some_and(|s| s.holders.contains(&Some(e)));
    for hit in hits.read() {
        if !players.contains(hit.target) {
            continue;
        }
        hit.amount /= h;
        if let Some(o) = &odd {
            let by = hit.instigator.filter(|i| players.contains(*i)).map(holds);
            hit.amount *= oddball::trait_factor(o, by, holds(hit.target));
        }
    }
}

/// Halo's rules (module doc).
pub struct HaloRulesPlugin;

impl Plugin for HaloRulesPlugin {
    fn build(&self, app: &mut App) {
        use crate::gameplay::{GameplayAppExt, gameplay_is};
        let game = || gameplay_is(super::GAME);
        app.add_modes(
            ModeGame {
                game: super::GAME,
                chosen: |w| {
                    w.get_resource::<HaloMatch>().filter(|m| m.on)?;
                    Some(variant::mode_key(&w.get_resource::<HaloGameType>()?.0.rules))
                },
                flow: FLOW,
            },
            &[&SLAYER, &TEAM_SLAYER, &CTF, &KING, &TEAM_KING, &ODDBALL, &TEAM_ODDBALL],
        )
        .init_resource::<HaloGameType>()
        .init_resource::<HaloMatch>()
        .init_resource::<Match>()
        .init_resource::<FriendlyFire>()
        .add_message::<scoring::HaloAnnounce>()
        .add_message::<crate::core::Died>()
        .game_hook(super::GAME, crate::gameplay::NewGame(new_game))
        .add_plugins(view::HaloMatchViewPlugin)
        .add_systems(
            FixedUpdate,
            (auto_start.before(SimSet::Rules), netgame::follow_map.before(SimSet::Rules))
                .run_if(crate::core::authoritative)
                .run_if(game()),
        )
        .add_systems(
            FixedUpdate,
            (
                (join, halo_tick, mode::flow::mission_update, ctf::carriers, oddball::carriers, chimera_sim::spawning::hold_the_dead)
                    .chain()
                    .in_set(SimSet::Rules)
                    .after(netgame::follow_map),
                health_setting.after(SimSet::Weapons).before(super::damage::deliver_hits),
                scoring::deaths.after(crate::core::apply_damage),
                spawning::player_settings.after(super::vitality::update),
            )
                .run_if(rules_on)
                .run_if(crate::core::authoritative)
                .run_if(game()),
        );
        resource_cvar::<HaloRules, String>(
            app,
            "halo_ce:gametype",
            "The game type the next game plays: slayer, team_slayer, ctf (a base word), a stock name (slayer_pro, iron_ctf ...) or a saved game type file.",
            |r| &mut r.gametype,
        );
        resource_cvar::<HaloRules, f32>(app, "halo_ce:timelimit", "Minutes a game lasts at most (0: none; the PC game's option).", |r| {
            &mut r.timelimit
        });
        resource_cvar::<HaloRules, u8>(app, "halo_ce:friendlyfire", "1: teammates' hits hurt (the Xbox's rule); 0: they don't.", |r| {
            &mut r.friendlyfire
        });
        app.console_command("halo_ce:restart", "Begin a new game now with halo_ce:gametype.", |w, _| {
            start(w);
            Ok(Some(w.resource::<HaloGameType>().0.name.clone()))
        });
        app.console_command("halo_ce:gametypes", "List the stock game types (names for halo_ce:gametype).", |_, _| {
            Ok(Some(STOCK_GAME_TYPES.iter().map(|s| s.name.to_ascii_lowercase().replace(' ', "_")).collect::<Vec<_>>().join(" ")))
        });
        app.console_command("halo_ce:game", "The game in play: type, scores, time, whether over.", |w, _| {
            let g = w.resource::<HaloGameType>().0.clone();
            let m = w.resource::<HaloMatch>().clone();
            let score = w.resource::<Match>().score;
            Ok(Some(format!(
                "{} ({:?}{}) on {}; red {} blue {}; time {} ticks; over {} results {}",
                g.name,
                g.rules.base(),
                if g.rules.team_play { ", teams" } else { "" },
                m.on,
                score[0],
                score[1],
                m.time,
                m.over,
                m.results
            )))
        });
    }
}
