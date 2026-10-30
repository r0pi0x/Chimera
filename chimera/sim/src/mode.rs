//! The mode runtime (docs/plans/active/architecture-review.md D2, L1),
//! lifted from Combat Arms' modes (`CaMode`, specs/combat_arms/game_modes.md):
//! a game mode is data and hooks (`Mode`) registered by
//! id under its game (`Modes`, `ModeAppExt::add_modes`), played on one
//! shared match (`state::Match`: phase, clocks, the sides' counters,
//! round results, the match's result) that one runtime steps through its
//! phases (`flow`).
//!
//! - `Mode`: its id (`game:name`, as its rule module), the key a game
//!   picks it by (Combat Arms' room key), whether the dead respawn, a
//!   warm-up (with revives), a forced setup length, how it makes teams
//!   (`Teams`), what a counted death does (`on_death`), its time-out rule,
//!   when the match ends, and `Hooks` on the steps: a round's setup, the
//!   round over and its end, any death,
//!   respawning, holding joiners (with whether the game's player count
//!   also holds them, `counts_joiners`), before the spawns, after a round,
//!   a live round's decision (`decide`) and the match's end after a
//!   warm-up (`warmup_end`; none: a warm-up never ends the match).
//! - `Flow`: a game's side of the runtime, the same for all its modes:
//!   how often the phases step (`frame`), who plays (`players`, in join
//!   order) and how they respawn, the phases' lengths (`Span`), the
//!   game's own work at each step (`FlowEvent`: its missions, map
//!   messages, statistics, clean-up; then triggered as `ModeStep` for
//!   any part to observe: the minigame's results) and how the phases step
//!   (`FlowOptions`: Combat Arms' missions, once per its frame, waiting
//!   for players, an intermission after each round; CS:S's rounds, every
//!   tick with exact comparisons, started at once, decided by the mode
//!   each tick, back to back, numbered as they start).
//! - Which mode plays: the gameplay's rules game's (`chimera_core::
//!   gameplay`; without one, the first game registered), the one its
//!   `ModeGame::chosen` names (Combat Arms: the `ca_mode` cvar). A game
//!   chains the runtime's step (`flow::mission_update`) into its own
//!   rules' set, gated as its rules are, or steps its own match from its
//!   rules' system (`flow::step_game`, with `current_of`: CS:S, whose
//!   rules' system does its own work around the step).
//! - A mode's per-tick work (its objects, timers, touches) is its own
//!   systems with declared queries, gated by `mode_is(id)` (`mode_in` for
//!   a mode and its variants: the playing mode) and chained where its
//!   game runs the runtime: the update's before the step (a tick's
//!   update comes before the step it may decide), the touches' after the
//!   tick's deaths (Combat Arms' `rules::mode_updates`, `mode_touches`;
//!   the minigame runtime's ladder, `ladder::refill`, `follow_lead`).
//! - `assign_team`: a player's team as the mode makes teams (two sides,
//!   free-for-all, everyone on the first side, or the game's as joined).
//! - `chimera_modes` lists the registered modes, the one playing marked.
//!
//! Not here yet (L2): definitions as data files, the objective
//! primitives, scoring tables, the HUD's named events and modifiers.

pub mod flow;
pub mod state;

use bevy::prelude::*;

pub use state::{
    ALPHA, BRAVO, Death, DeathKind, FFA_FIRST, FFA_LAST, Match, Phase, RoundResult, classify, enemy, plays, side,
};

use crate::{console::ConsoleAppExt, core::Died};
use chimera_core::gameplay;

/// How a mode makes teams.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Teams {
    /// `ALPHA` against `BRAVO`, as they came in (one without a side goes
    /// to the smaller).
    Two,
    /// Every player a team of their own (`FFA_FIRST`..).
    FreeForAll,
    /// Everyone starts on `ALPHA` at a round's setup; the mode moves
    /// players.
    AllAlpha,
    /// The game's own teams, as the players joined them: the runtime
    /// moves nobody (CS:S's, picked in its menus and balanced by its
    /// rules).
    AsJoined,
}

/// A mode's hooks on the runtime's steps (`NO_HOOKS` do nothing).
#[derive(Clone, Copy)]
pub struct Hooks {
    /// The round's setup, after everyone was respawned (`Match::warmup`
    /// says whether it is the warm-up).
    pub setup: fn(&mut World),
    /// The round is over (its winner decided, the win counted).
    pub round_over: fn(&mut World),
    /// The round's end, before the game's clean-up.
    pub round_end: fn(&mut World),
    /// A player died, in any phase, before the counting.
    pub died: fn(&mut World, &Died),
    /// Whether `e` may respawn now beyond `Mode::respawns`.
    pub may_respawn: fn(&World, Entity) -> bool,
    /// A round mode holds joiners now beyond the game's rule.
    pub holds_joiner: fn(&World) -> bool,
    /// At a mission start after the teams are made, before everyone is
    /// respawned.
    pub before_spawns: fn(&mut World),
    /// After a round's end, its intermission set: the mode may lengthen it
    /// or ask for a warm-up next.
    pub after_round: fn(&mut World),
    /// Every live step before the round's end is tested (`time_up`: the
    /// round timer is at 0): the mode may decide the round now
    /// (`Match::win`, `Match::end_without_winner`), which ends it in the
    /// same step.
    pub decide: fn(&mut World, bool),
    /// After a warm-up's end (never a real round's: that is
    /// `Mode::match_end`): the match's result if it is over, without the
    /// round-win goal (an empty team, one player left). `NO_HOOKS`: the
    /// match never ends after a warm-up (CS:S's rounds).
    pub warmup_end: fn(&World) -> Option<Option<u8>>,
    /// The game's own late-join count (`Flow` games: Combat Arms'
    /// `late_join_wait`) also holds this mode's joiners; false: only
    /// `holds_joiner` decides.
    pub counts_joiners: bool,
}

/// No hooks.
pub const NO_HOOKS: Hooks = Hooks {
    setup: |_| {},
    round_over: |_| {},
    round_end: |_| {},
    died: |_, _| {},
    may_respawn: |_, _| true,
    holds_joiner: |_| false,
    before_spawns: |_| {},
    after_round: |_| {},
    decide: |_, _| {},
    warmup_end: |_| None,
    counts_joiners: true,
};

/// A game mode: its data and hooks.
#[derive(Clone, Copy)]
pub struct Mode {
    /// `game:name` (its rule module's name).
    pub id: &'static str,
    /// The key its game picks it by (`ModeGame::chosen`).
    pub key: &'static str,
    /// The name players see.
    pub shown_as: &'static str,
    pub description: &'static str,
    /// The dead respawn on request.
    pub respawns: bool,
    /// The match starts with a warm-up round ...
    pub warmup: bool,
    /// ... that revives the dead every `Span::Revive`.
    pub warmup_revives: bool,
    /// The round setup's length, when the mode forces one.
    pub forced_setup: Option<f32>,
    /// How it makes teams.
    pub teams: Teams,
    /// What a counted death adds and the win test.
    pub on_death: fn(&mut World, &Death),
    /// The winner of a round that ran out of time without one (may stay
    /// None: a draw).
    pub time_out: fn(&mut World),
    /// After a real round's end: the match's result if it is over
    /// (Some(None): a draw).
    pub match_end: fn(&World) -> Option<Option<u8>>,
    pub hooks: Hooks,
}

/// A length the runtime asks its game for, in seconds (`Flow::length`;
/// None: none, for `Round` and `Warmup` no timer and no warm-up).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Span {
    /// The round's setup (unless the mode forces one).
    Setup,
    /// A real round.
    Round,
    /// The warm-up round.
    Warmup,
    /// From a round's result to its end.
    RoundOver,
    /// From a round's end to the next mission start.
    Intermission,
    /// Between the warm-up's revives.
    Revive,
}

/// The game's own work at the runtime's steps (`Flow::event`), in the
/// order a mission runs them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowEvent {
    /// A mission starts (`first`: the match's first), before anything
    /// else (the mission's data resolved).
    Prepare { first: bool },
    /// The match's first mission: everyone's statistics back to nothing.
    ResetStats,
    /// The mission's counters are reset, before the teams are made and
    /// everyone respawned (the map's start messages).
    MissionStart,
    /// A free-for-all round was won by this player.
    RoundWonBy(Entity),
    /// The round is over, after the mode's `round_over`.
    RoundOver,
    /// The round's end, after the mode's `round_end` (the clean-up).
    RoundEnd,
    /// The round's counters cleared, before the match's end is tested.
    RoundEnded,
    /// The match has its result (the round's end that decided it, after
    /// the mode's `after_round`: the intermission on, or the match over
    /// with `FlowOptions::direct`). Once a match.
    MatchEnd,
}

/// A step of the playing mode's flow, triggered as an event
/// (`world.trigger`, so its observers run at once, inside the step)
/// right after the game's own `Flow::event` handler, on the side that
/// steps the match (a host, a standalone game; never a network client).
/// The lifecycle others observe instead of polling the match: the
/// match's start (`Prepare { first: true }`, `ResetStats`), a round's
/// start (`MissionStart`), its end (`RoundOver`, `RoundEnd`,
/// `RoundEnded`) and the match's end (`MatchEnd`); CS:S's rounds,
/// Combat Arms' modes and the minigames alike. What its observers queue
/// on `Commands` is applied before the step goes on.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeStep(pub FlowEvent);

/// A game's side of the runtime: how its matches run, whatever the mode.
#[derive(Clone, Copy)]
pub struct Flow {
    /// Seconds between the phases' steps (the first tick at or after each
    /// frame's time); 0: every tick.
    pub frame: f64,
    /// The players in the match (on a team that plays), in the order they
    /// came in.
    pub players: fn(&mut World) -> Vec<(Entity, u8)>,
    /// Respawn `e` (at a mission start, a warm-up's revive).
    pub respawn: fn(&mut World, Entity),
    /// A phase's length (s).
    pub length: fn(&mut World, Span) -> Option<f64>,
    /// The game's work at a step.
    pub event: fn(&mut World, FlowEvent),
    /// How the phases step (`FlowOptions::MISSIONS`: Combat Arms').
    pub options: FlowOptions,
}

/// How a game's phases step: Combat Arms' missions (`MISSIONS`) or a
/// game whose rounds run back to back, decided and stepped every tick
/// (CS:S's rounds).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlowOptions {
    /// Added to the clock when a phase's end time is compared (`now +
    /// slack >= until`): 1e-9 for frames whose times are sums of a float
    /// step, 0 for exact comparisons.
    pub slack: f64,
    /// The match waits for a player (`Phase::Waiting`, then `Starting`
    /// and its first mission at the next frame); false: the first step
    /// starts it, players or not.
    pub wait_for_players: bool,
    /// The round timer at 0 ends the round (the mode's `time_out` picks
    /// the winner); false: it only tells the mode's `decide` hook
    /// (`time_up`), and the round goes on until the mode ends it.
    pub timer_ends_round: bool,
    /// Round over's end goes straight into the next mission start in the
    /// same step (no round-end frame, no intermission): the round's end
    /// steps, the match's end test, the mode's `after_round`, then the
    /// next round's setup or the match over.
    pub direct: bool,
    /// `Match::round` counts rounds started (1 in the first, from its
    /// mission start) instead of rounds played.
    pub count_at_start: bool,
}

impl FlowOptions {
    /// Combat Arms' missions (the runtime's first game).
    pub const MISSIONS: FlowOptions = FlowOptions {
        slack: 1e-9,
        wait_for_players: true,
        timer_ends_round: true,
        direct: false,
        count_at_start: false,
    };
}

/// A game's modes as the runtime sees them.
#[derive(Clone, Copy)]
pub struct ModeGame {
    /// The game's id (`chimera_core::gameplay::Gameplay::rules`).
    pub game: &'static str,
    /// The key of the mode to play now (None: no mode).
    pub chosen: fn(&World) -> Option<&str>,
    pub flow: Flow,
}

/// The registered games and their modes, in the order registered.
#[derive(Resource, Clone, Default)]
pub struct Modes {
    games: Vec<ModeGame>,
    modes: Vec<(&'static str, &'static Mode)>,
}

impl Modes {
    /// A mode by its id.
    pub fn get(&self, id: &str) -> Option<&'static Mode> {
        self.modes.iter().find(|(_, m)| m.id == id).map(|(_, m)| *m)
    }

    /// `game`'s mode by its key.
    pub fn by_key(&self, game: &str, key: &str) -> Option<&'static Mode> {
        self.modes.iter().find(|(g, m)| *g == game && m.key == key).map(|(_, m)| *m)
    }

    /// Every registered mode with its game, in the order registered.
    pub fn all(&self) -> impl Iterator<Item = (&'static str, &'static Mode)> + '_ {
        self.modes.iter().copied()
    }

    /// The game whose modes play: the gameplay's rules game, else the
    /// first registered.
    pub fn playing_game(&self, world: &World) -> Option<&ModeGame> {
        match world.get_resource::<gameplay::Gameplay>() {
            Some(g) => self.games.iter().find(|m| m.game == g.rules),
            None => self.games.first(),
        }
    }
}

/// The mode playing and its game's flow, if any.
pub fn current(world: &World) -> Option<(&'static Mode, Flow)> {
    let modes = world.get_resource::<Modes>()?;
    let game = modes.playing_game(world)?;
    let key = (game.chosen)(world)?;
    Some((modes.by_key(game.game, key)?, game.flow))
}

/// `game`'s chosen mode and its flow, whichever game plays (a game that
/// steps its own match: `flow::step_game`).
pub fn current_of(world: &World, game: &str) -> Option<(&'static Mode, Flow)> {
    let modes = world.get_resource::<Modes>()?;
    let g = modes.games.iter().find(|g| g.game == game)?;
    let key = (g.chosen)(world)?;
    Some((modes.by_key(g.game, key)?, g.flow))
}

/// The mode playing, if any.
pub fn current_mode(world: &World) -> Option<&'static Mode> {
    current(world).map(|(m, _)| m)
}

/// Run condition: `id` is the mode playing (`current_mode`). A mode's own
/// per-tick systems (its objects, timers and touches) are gated by it and
/// chained where their game runs the runtime: the update's before the
/// phases step (`flow::mission_update`, `step_phases`), the touches' after
/// the tick's deaths. Never a new set (DESIGN.md "ECS style").
pub fn mode_is(id: &'static str) -> impl FnMut(&World) -> bool + Clone {
    move |world: &World| current_mode(world).is_some_and(|m| m.id == id)
}

/// Run condition: the mode playing is one of `ids` (modes sharing a
/// per-tick system: a mode and its variants).
pub fn mode_in(ids: &'static [&'static str]) -> impl FnMut(&World) -> bool + Clone {
    move |world: &World| current_mode(world).is_some_and(|m| ids.contains(&m.id))
}

/// Registering a game's modes.
pub trait ModeAppExt {
    /// `game`'s modes (one of the same id replaces the old in place; the
    /// game registered again replaces its flow).
    fn add_modes(&mut self, game: ModeGame, modes: &[&'static Mode]) -> &mut Self;
}

impl ModeAppExt for App {
    fn add_modes(&mut self, game: ModeGame, modes: &[&'static Mode]) -> &mut Self {
        if !self.world().contains_resource::<Modes>() {
            self.init_resource::<Modes>().console_command(
                "chimera_modes",
                "List the game modes the games registered (id, key, name), the one playing marked *.",
                |w, _| {
                    let playing = current_mode(w).map(|m| m.id);
                    let all: Vec<String> = w
                        .get_resource::<Modes>()
                        .map(|m| {
                            m.all()
                                .map(|(_, m)| {
                                    let mark = if Some(m.id) == playing { "*" } else { " " };
                                    format!("{mark} {} ({}): {}", m.id, m.key, m.shown_as)
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    if all.is_empty() {
                        return Ok(Some("no modes registered".into()));
                    }
                    Ok(Some(all.join("\n")))
                },
            );
        }
        let mut registry = self.world_mut().resource_mut::<Modes>();
        match registry.games.iter_mut().find(|g| g.game == game.game) {
            Some(g) => *g = game,
            None => registry.games.push(game),
        }
        for mode in modes {
            match registry.modes.iter_mut().find(|(_, m)| m.id == mode.id) {
                Some(m) => *m = (game.game, mode),
                None => registry.modes.push((game.game, mode)),
            }
        }
        self
    }
}

/// `e`'s team as the playing mode makes teams: a side (one without goes to
/// the smaller side, `ALPHA` on a tie); every player a team of their own
/// in a free-for-all mode (the lowest free id from `FFA_FIRST`); `ALPHA`
/// for everyone at a round's setup (`reset`) in an all-on-Alpha mode.
/// Observers and players without a team are left alone.
pub fn assign_team(world: &mut World, e: Entity, reset: bool) {
    let Some((mode, flow)) = current(world) else { return };
    assign_team_in(world, e, reset, mode, flow);
}

/// `assign_team` as `mode` makes teams, among `flow`'s players.
pub(crate) fn assign_team_in(world: &mut World, e: Entity, reset: bool, mode: &Mode, flow: Flow) {
    if mode.teams == Teams::AsJoined {
        return;
    }
    let Some(team) = world.get::<crate::core::Team>(e).map(|t| t.0) else { return };
    if !plays(team) {
        return;
    }
    let others: Vec<u8> = (flow.players)(world)
        .into_iter()
        .filter(|(p, _)| *p != e)
        .map(|(_, t)| t)
        .collect();
    if let Some(want) = team_for(mode.teams, team, &others, reset)
        && want != team
    {
        world.entity_mut(e).insert(crate::core::Team(want));
    }
}

/// The team a player on `team` should have, among `others` (the other
/// players' teams), as `teams` makes them; None: keep it.
pub fn team_for(teams: Teams, team: u8, others: &[u8], reset: bool) -> Option<u8> {
    match teams {
        Teams::Two => {
            if side(team).is_some() {
                return None;
            }
            let count = |t: u8| others.iter().filter(|o| **o == t).count();
            Some(if count(BRAVO) < count(ALPHA) { BRAVO } else { ALPHA })
        }
        Teams::AllAlpha => {
            if !reset && side(team).is_some() {
                return None;
            }
            Some(ALPHA)
        }
        Teams::FreeForAll => {
            if team >= FFA_FIRST && !others.contains(&team) {
                return None;
            }
            (FFA_FIRST..=FFA_LAST).find(|t| !others.contains(t))
        }
        Teams::AsJoined => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn teams_as_the_mode_makes_them() {
        // Two sides: a side is kept, none goes to the smaller (Alpha on a
        // tie).
        assert_eq!(team_for(Teams::Two, BRAVO, &[ALPHA], false), None);
        assert_eq!(team_for(Teams::Two, 7, &[ALPHA, ALPHA, BRAVO], false), Some(BRAVO));
        assert_eq!(team_for(Teams::Two, 7, &[ALPHA, BRAVO], false), Some(ALPHA));
        // Free-for-all: a free own id is kept, a taken one or a side moves
        // to the lowest free.
        assert_eq!(team_for(Teams::FreeForAll, 5, &[3, 4], false), None);
        assert_eq!(team_for(Teams::FreeForAll, 3, &[3, 4], false), Some(5));
        assert_eq!(team_for(Teams::FreeForAll, ALPHA, &[3], false), Some(4));
        // All on Alpha: everyone at a setup, only the side-less otherwise.
        assert_eq!(team_for(Teams::AllAlpha, BRAVO, &[], false), None);
        assert_eq!(team_for(Teams::AllAlpha, BRAVO, &[], true), Some(ALPHA));
        assert_eq!(team_for(Teams::AllAlpha, 9, &[], false), Some(ALPHA));
        // As joined: nobody moves.
        assert_eq!(team_for(Teams::AsJoined, 9, &[9], true), None);
    }

    #[test]
    fn the_registry_keeps_one_of_each_id() {
        static A: Mode = Mode {
            id: "t:a",
            key: "A",
            shown_as: "A",
            description: "",
            respawns: true,
            warmup: false,
            warmup_revives: false,
            forced_setup: None,
            teams: Teams::Two,
            on_death: |_, _| {},
            time_out: |_| {},
            match_end: |_| None,
            hooks: NO_HOOKS,
        };
        static B: Mode = Mode { id: "t:b", key: "B", ..A };
        static A2: Mode = Mode { key: "A2", ..A };
        let flow = Flow {
            frame: 0.0,
            players: |_| Vec::new(),
            respawn: |_, _| {},
            length: |_, _| None,
            event: |_, _| {},
            options: FlowOptions::MISSIONS,
        };
        let game = ModeGame {
            game: "t",
            chosen: |_| Some("A2"),
            flow,
        };
        let mut app = App::new();
        app.add_modes(game, &[&A, &B]).add_modes(game, &[&A2]);
        let modes = app.world().resource::<Modes>();
        assert_eq!(modes.all().map(|(_, m)| m.key).collect::<Vec<_>>(), ["A2", "B"]);
        assert_eq!(modes.get("t:b").map(|m| m.key), Some("B"));
        assert_eq!(current_mode(app.world()).map(|m| m.id), Some("t:a"));
    }
}
