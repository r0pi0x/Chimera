//! Combat Arms' game modes (specs/combat_arms/game_modes.md) as rule
//! modules (`rule_module`: registered by name, the one playing picked by
//! the `ca_mode` cvar, a room key): `combat_arms:tdm` (`TDM`, Team Death
//! Match shown as Elimination, M1), `elimination_pro` (`TSV`, M2),
//! `search_destroy` (`TMM`, M3), `capture_the_flag` (`CTF`, M4),
//! `one_man_army` (`FFA`, M5), `last_man_standing` (`LMS`, M6),
//! `spy_hunt` (`BNT`, M7), `quarantine` (`QRT`, M8), `bombing_run`
//! (`SBT`, M9), `seize_and_secure` (`HQM`, M10) and `squad_match_2`
//! (`SQM2`, M11's variant), `vip_elimination` (`VTDM`, M12), `arms_race`
//! (`VWRM`, M14), `hunted` (`HG`, M15), `go4ca_cup` (`ESL`, M17);
//! from specs/combat_arms/game_modes_2.md: `hired_guns` (`SQM`, M11),
//! `vip_escort` (`ESC`, M13) and `clan_vip_escort` (`CESC`), the clan
//! modes `clan_tdm` (`CDM`), `clan_ctf` (`CCTF`), `clan_search_destroy`
//! (`CMM`) and `clan_elimination_pro` (`CTSV`, M18); and
//! `combat_arms:none` (no mode: the rules leave players alone).
//!
//! A mode is data and hooks on Chimera's mode runtime (`chimera_sim::
//! mode`, lifted from here, architecture-review.md L1): `CaMode` is the
//! runtime's `Mode` (`CaMode::rules`, read through `Deref`: its room key,
//! whether it respawns, has a warm-up, its forced setup, how it makes
//! teams (`Teams`: two, free-for-all, everyone Alpha), what a death adds,
//! its time-out rule, when the match ends; `Hooks` for its objects: the
//! round's setup, the round over and its end, a death in any phase,
//! respawning and holding joiners) plus Combat Arms' own
//! (whether it scales friendly fire, gives assists, shows a timer, its
//! spawn family (`Spawns`), its round length (`RoundTime`), whether firing
//! ends protection, its death drops, its score panel and scoreboard
//! order, what a leaver does; what a leaver's objects do is the mode's
//! part of the `on_leave` observer, `mode_left`). Every mode
//! is registered with the runtime (`ModeAppExt::add_modes`, picked by
//! `ca_mode`), which steps the match (`CaMatch`, the runtime's `Match`)
//! through its phases with Combat Arms' flow (`flow::FLOW`). A mode's
//! per-tick work (its objects' update before the step, its touches after
//! the tick's deaths) is its module's systems with declared queries,
//! gated by the mode playing (`mode::mode_is`) and chained in the rules'
//! chain (`mode_updates`, `mode_touches`); what builds or removes weapons
//! or respawns is world work they queue (`Commands`), applied before the
//! step. The shared
//! rules, one submodule each:
//!
//! - `settings`: the room's and the server file's settings, the shared
//!   constants (S2) and the objectives' numbers, data/modes.toml under the
//!   `ca_*` cvars;
//! - `state`: the match (`CaMatch`, the runtime's: phase, clocks, team
//!   counters, round wins and results, the tie-break counters) and each
//!   player's statistics (`CaStats`, S7, with the modes' own);
//! - `flow`: Combat Arms' side of the runtime's phases (`FLOW`: the
//!   mission update once per server frame, S3; the mission's lengths, the
//!   warm-up's revives, S9; round ends' cleanup and map messages, S11,
//!   S15), joining, team assignment and leaving (S1, S10), a fresh match
//!   (`ca_restart`, a map load: `gameplay::NewGame`);
//! - `spawns`: start points by the mode's family and their cursors (S4),
//!   the respawn and its request (S5), telefrags, spawn protection (S6);
//! - `scoring`: deaths, kills and assists (S7), friendly fire (S8);
//! - `objects`: the map's mode objects (`game::world` passes them
//!   through); `objectives`: the objectives' state every machine sees,
//!   what players carry and the action gauges; `bomb`: plant, defuse and
//!   detonation;
//! - one module a mode: `tdm`, `elimination`, `search_destroy`, `ctf`,
//!   `free_for_all`, `last_man`, `spy_hunt`, `quarantine`, `bombing_run`,
//!   `seize`, `vip`, `arms_race`, `hunted`, `go4ca`, `hired_guns` (with
//!   its AI mercenaries, `mercs`), `escort`, `clan`;
//! - `scoreboard`: the scoreboard's rows and each mode's order (S13).
//!
//! What the HUD and a network client read is `CaMatchView` (the server
//! writes it from `CaMatch` each tick; a client from the network,
//! `game::net`), with the team counts (`CaTeamCounts`), and
//! `objectives::CaObjectives`.
//!
//! Teams are the lobby's (S1): a player keeps the team it came in with
//! (the network's join puts it on the smaller team; single player's local
//! player is Bravo, the shell's pick); no auto-balance or switching
//! (Quirk 10). A free-for-all mode gives each player a team of its own
//! (`state::FFA_FIRST`..), Spy Hunt and Quarantine put everyone on Alpha
//! at a round's setup (`flow::assign_team`). Players on no team (0,
//! observers) are left alone.
//!
//! Lifted into Chimera for this (L15, docs/plans/active/combat-arms.md):
//! the dead held still and a character revived at a place
//! (`chimera_sim::spawning`), which CS:S's deathmatch uses too. CS:S's
//! round loop, team joining, bomb and game end stay its own: their
//! phases, money, objectives, `mp_*` settings and win table share nothing
//! with Combat Arms' beyond those (CS:S's bomb is a weapon with its own
//! arming, beeps and blast; CA's is a gauge on a site).
//!
//! Known shortcuts (docs/tech-debt.md; the most literal reading where the
//! spec is unclear, each module's own list): see `flow`, `spawns`,
//! `scoring`, `settings`, `objects`, `objectives`, `bomb` and each mode's.
//! Not there: Snowball Fight (game_modes_2.md M16: no data set enables
//! it, and its snowball weapons aren't in our data), the co-operative and
//! offline modes (M19, M20: mission scripts and PvE enemies; the AI damage
//! cap of M19 applies to Hired Guns' mercenaries) and the modes without
//! rules (M21, M22). Bots fight in every mode (any other team is an
//! enemy) but play no objective: bots_objectives.md says no AI in Combat
//! Arms does (its Quirk 1); the only AI on a versus team is Hired Guns'
//! mercenaries. The server runs the mission update on its 20 Hz frames
//! (`grenades::SERVER_FRAME`) over the game's 15 ms tick.

pub mod arms_race;
pub mod bomb;
pub mod bombing_run;
pub mod clan;
pub mod ctf;
pub mod elimination;
pub mod escort;
pub mod flow;
pub mod free_for_all;
pub mod go4ca;
pub mod hired_guns;
pub mod hunted;
pub mod last_man;
pub mod mutation;
pub mod mercs;
pub mod objectives;
pub mod objects;
pub mod quarantine;
pub mod scoreboard;
pub mod scoring;
pub mod search_destroy;
pub mod seize;
pub mod settings;
pub mod spawns;
pub mod spy_hunt;
pub mod state;
pub mod tdm;
pub mod vip;

use bevy::{
    ecs::{schedule::ScheduleConfigs, system::ScheduleSystem},
    prelude::*,
};
use crate::gameplay::GameplayAppExt as _;

pub use crate::mode::{Hooks, Mode, NO_HOOKS, Teams};
pub use settings::CaRuleSettings;
pub use state::{CaMatch, CaStats, Phase};

use crate::{
    console::ConsoleAppExt,
    core::{Connecting, Dead, Health, Intent, Npc, SimClock, SimSet, Spectating, Team},
    mode::ModeAppExt,
    rule_module::{self, RuleModule, RuleModules},
};

/// No mode: the rules leave players alone.
pub const NO_MODE: &str = "combat_arms:none";
/// Team Death Match (M1).
pub const TDM: &str = "combat_arms:tdm";
/// Elimination Pro (M2).
pub const ELIMINATION_PRO: &str = "combat_arms:elimination_pro";
/// Search & Destroy (M3).
pub const SEARCH_DESTROY: &str = "combat_arms:search_destroy";
/// Capture the Flag (M4).
pub const CAPTURE_THE_FLAG: &str = "combat_arms:capture_the_flag";
/// One Man Army (M5).
pub const ONE_MAN_ARMY: &str = "combat_arms:one_man_army";
/// Last Man Standing (M6).
pub const LAST_MAN_STANDING: &str = "combat_arms:last_man_standing";
/// Spy Hunt (M7).
pub const SPY_HUNT: &str = "combat_arms:spy_hunt";
/// Quarantine Regen (M8).
pub const QUARANTINE: &str = "combat_arms:quarantine";
/// Bombing Run (M9).
pub const BOMBING_RUN: &str = "combat_arms:bombing_run";
/// Seize and Secure (M10) and its variant Squad Match 2 (M11).
pub const SEIZE_AND_SECURE: &str = "combat_arms:seize_and_secure";
pub const SQUAD_MATCH_2: &str = "combat_arms:squad_match_2";
/// VIP Elimination (M12).
pub const VIP_ELIMINATION: &str = "combat_arms:vip_elimination";
/// Arms Race (M14).
pub const ARMS_RACE: &str = "combat_arms:arms_race";
/// Hunted (M15).
pub const HUNTED: &str = "combat_arms:hunted";
/// S&D for the Go4CA Cup (M17).
pub const GO4CA_CUP: &str = "combat_arms:go4ca_cup";
/// Hired Guns (game_modes_2.md M11).
pub const HIRED_GUNS: &str = "combat_arms:hired_guns";
/// VIP Escort (M13) and its clan variant.
pub const VIP_ESCORT: &str = "combat_arms:vip_escort";
pub const CLAN_VIP_ESCORT: &str = "combat_arms:clan_vip_escort";
/// The clan modes (M18).
pub const CLAN_TDM: &str = "combat_arms:clan_tdm";
pub const CLAN_CTF: &str = "combat_arms:clan_ctf";
pub const CLAN_SEARCH_DESTROY: &str = "combat_arms:clan_search_destroy";
pub const CLAN_ELIMINATION_PRO: &str = "combat_arms:clan_elimination_pro";

/// The modes whose rooms have no friendly-fire option (always off:
/// game_modes_2.md M13.1, M18.1).
pub fn no_friendly_fire(key: &str) -> bool {
    key == escort::MODE.key || clan::KEYS.contains(&key)
}

/// The modes whose rooms take no joiners in progress (the clan modes,
/// M18.1).
pub fn no_join_in_progress(key: &str) -> bool {
    clan::KEYS.contains(&key)
}

/// A leaving player, as the leaving rule sees it (S10): its team, its
/// team's players and living players (the leaver counted), whether it was
/// alive; the players in the match (the leaver counted), the kills made
/// in total, and the other players' teams; G and One Man Army's share of
/// it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Leaver {
    pub team: u8,
    pub team_players: usize,
    pub team_alive: usize,
    pub alive: bool,
    pub players: usize,
    pub total_kills: u32,
    pub others: Vec<u8>,
    pub goal: u32,
    pub leave_kill_share: f32,
    /// The leaver was VIP Escort's VIP.
    pub vip: bool,
    /// When it left (game seconds).
    pub now: f64,
}

/// A player who left, for the mode's objects (`on_leave`'s mode part,
/// `mode_left`): where it was and what it carried.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LeftPlayer {
    pub entity: Entity,
    pub team: u8,
    pub at: Vec3,
    pub carry: objectives::CaCarry,
    pub alive: bool,
}

/// Which start points a mode spawns at (S4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spawns {
    /// The `GameType` its points name; None: the spec doesn't name it (the
    /// team modes' `Team Death Match` points stand in).
    pub family: Option<&'static str>,
    /// One list whatever the points' team.
    pub one_list: bool,
    /// The list shuffled once at map load.
    pub shuffled: bool,
    /// Rounds use the locked points, warm-ups the unlocked (Hunted).
    pub locked_rounds: bool,
}

/// The team modes' points.
pub const TEAM_POINTS: Spawns = Spawns {
    family: Some(super::world::SPAWN_GAME_TYPE),
    one_list: false,
    shuffled: false,
    locked_rounds: false,
};

/// A round's length (the warm-up's is always `TrainingTimeLimit`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoundTime {
    /// The mission's `TimeLimit` (none: no timer).
    Mission,
    /// `TimeLimit` x G (capture the flag).
    MissionTimesGoal,
    /// G minutes (Seize and Secure, Spy Hunt).
    GoalMinutes,
}

/// What the score panel's two rows count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    /// The team counters (`CaMatch::score`).
    TeamScore,
    /// Round wins.
    RoundWins,
    /// Free-for-all: the local player's kills and the leader's.
    Kills,
    /// Free-for-all rounds: the local player's round wins and the
    /// leader's.
    PlayerRoundWins,
}

/// The scoreboard's order (S13).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScoreOrder {
    /// Kills, then fewer deaths.
    Kills,
    /// Kills, then fewer deaths; the column shows detonations and defuses
    /// (S&D, Bombing Run).
    ExplodeDefuse,
    /// Captures, returns, kills, fewer deaths.
    Captures,
    /// Mission points, kills, fewer deaths.
    MissionPoints,
    /// Mission points only.
    MissionOnly,
    /// Mission points, time hunted, kills (Hunted).
    Hunted,
}

/// A game mode: the runtime's part (`rules`: its key, respawning,
/// warm-up, teams, death counting, time-out and match end, the hooks on
/// the runtime's steps; read through `Deref`, `mode.key`) and Combat Arms'
/// own (its spawns, panel, scoreboard, friendly fire, leaving).
#[derive(Clone, Copy)]
pub struct CaMode {
    /// The runtime's mode (`Mode::id` its rule module's name, `Mode::key`
    /// the room key, `ca_mode`).
    pub rules: Mode,
    /// Friendly fire on scales a teammate's direct damage (S8).
    pub ff_scaled: bool,
    /// Enemy kills give assists (S7).
    pub assists: bool,
    /// The HUD shows the round timer (hud_crosshair.md D7).
    pub timer_shown: bool,
    /// Its start points (S4).
    pub spawns: Spawns,
    /// Its rounds' length.
    pub round_time: RoundTime,
    /// Firing ends spawn protection (S6: the free-for-all modes).
    pub fire_ends_protection: bool,
    /// The dead drop their primary (inventory.md 12; not in Arms Race).
    pub death_drops: bool,
    /// What the score panel counts.
    pub panel: Panel,
    /// The scoreboard's order.
    pub order: ScoreOrder,
    /// A player leaves (S10). The mode's leaver's objects are its part
    /// of `on_leave` (`mode_left`).
    pub leaving: fn(&mut CaMatch, &Leaver),
}

impl std::ops::Deref for CaMode {
    type Target = Mode;

    fn deref(&self) -> &Mode {
        &self.rules
    }
}

/// Every mode, by room key.
pub const MODES: &[CaMode] = &[
    tdm::MODE,
    elimination::MODE,
    search_destroy::MODE,
    ctf::MODE,
    free_for_all::MODE,
    last_man::MODE,
    spy_hunt::MODE,
    quarantine::MODE,
    bombing_run::MODE,
    seize::MODE,
    seize::SQUAD_MATCH,
    vip::MODE,
    arms_race::MODE,
    hunted::MODE,
    go4ca::MODE,
    hired_guns::MODE,
    escort::MODE,
    escort::CLAN,
    clan::TDM,
    clan::CTF,
    clan::SEARCH_DESTROY,
    clan::ELIMINATION_PRO,
];

/// The mode playing (`ca_mode`), if any.
pub fn current_mode(world: &World) -> Option<&'static CaMode> {
    let key = &world.get_resource::<CaRuleSettings>()?.mode;
    mode_by_key(key)
}

/// A mode by its room key.
pub fn mode_by_key(key: &str) -> Option<&'static CaMode> {
    MODES.iter().find(|m| m.key == key)
}

/// Every mode's own per-tick systems (its objects, timers and touches
/// before the phases step), gated by the mode playing (`mode::mode_is`,
/// `mode_in`) and chained in the rules' chain where the runtime's update
/// ran, before `flow::mission_update`: a mode's update writes before the
/// step that may end its round (plan ecs-cleanup step 4, O1/O4). One mode
/// plays at a time, so the modes need no sync point between them
/// (`chain_ignore_deferred`); a mode's own steps are chained with one
/// (`bombing_run::carry` then `bombs`). A clan mode runs its base mode's
/// systems, then its own (`clan::ctf_half`, `leaver_ends_round`).
fn mode_updates() -> ScheduleConfigs<ScheduleSystem> {
    use crate::mode::{mode_in, mode_is};
    (
        (search_destroy::hand_out, search_destroy::bombs)
            .chain()
            .distributive_run_if(mode_in(search_destroy::RUNS_IN)),
        ctf::update.run_if(mode_in(&[CAPTURE_THE_FLAG, CLAN_CTF])),
        clan::ctf_half.run_if(mode_is(CLAN_CTF)),
        clan::leaver_ends_round.run_if(mode_in(clan::MODES)),
        (bombing_run::carry, bombing_run::bombs)
            .chain()
            .distributive_run_if(mode_is(BOMBING_RUN)),
        seize::update.run_if(mode_in(&[SEIZE_AND_SECURE, SQUAD_MATCH_2])),
        vip::update.run_if(mode_is(VIP_ELIMINATION)),
        arms_race::update.run_if(mode_is(ARMS_RACE)),
        hunted::update.run_if(mode_is(HUNTED)),
        spy_hunt::update.run_if(mode_is(SPY_HUNT)),
        quarantine::update.run_if(mode_is(QUARANTINE)),
        hired_guns::update.run_if(mode_is(HIRED_GUNS)),
        escort::update.run_if(mode_in(escort::RUNS_IN)),
    )
        .chain_ignore_deferred()
        // Never beside a client's clock (run where others are shown,
        // never with the rules): no order between them is right or wrong.
        .ambiguous_with(super::characters::client_clock)
}

/// Every mode's own touches, after the tick's deaths (`scoring::
/// handle_deaths`) and before `publish`, where the runtime's touches ran;
/// gated and chained as `mode_updates`.
fn mode_touches() -> ScheduleConfigs<ScheduleSystem> {
    use crate::mode::{mode_in, mode_is};
    (
        hired_guns::touches.run_if(mode_is(HIRED_GUNS)),
        escort::touches.run_if(mode_in(escort::RUNS_IN)),
    )
        .chain_ignore_deferred()
        // Never beside a client's clock (run where others are shown,
        // never with the rules): no order between them is right or wrong.
        .ambiguous_with(super::characters::client_clock)
}

fn mode_on(settings: Res<CaRuleSettings>) -> bool {
    mode_by_key(&settings.mode).is_some()
}

/// The cvars that set the modes up.
const SETTINGS: &[&str] = &[
    "ca_mode",
    "ca_goal",
    "ca_friendlyfire",
    "ca_join_in_progress",
    "ca_object_start",
    "ca_object_end",
    "ca_mission_end",
    "ca_respawn_wait",
    "ca_respawn_wait_client",
    "ca_respawn_invulnerability",
    "ca_respawn_bullet_protection",
];

/// A mode's rule module.
macro_rules! module_of {
    ($($m:ident)::+) => {
        RuleModule {
            name: $($m)::+.id,
            description: $($m)::+.description,
            settings: SETTINGS,
            applies: |w| current_mode(w).is_some_and(|m| m.id == $($m)::+.id),
        }
    };
}

/// The rule modules (`chimera_rules`): each mode's, picked by `ca_mode`,
/// then no mode.
pub fn modules() -> Vec<RuleModule> {
    vec![
        module_of!(tdm::MODE),
        module_of!(elimination::MODE),
        module_of!(search_destroy::MODE),
        module_of!(ctf::MODE),
        module_of!(free_for_all::MODE),
        module_of!(last_man::MODE),
        module_of!(spy_hunt::MODE),
        module_of!(quarantine::MODE),
        module_of!(bombing_run::MODE),
        module_of!(seize::MODE),
        module_of!(seize::SQUAD_MATCH),
        module_of!(vip::MODE),
        module_of!(arms_race::MODE),
        module_of!(hunted::MODE),
        module_of!(go4ca::MODE),
        module_of!(hired_guns::MODE),
        module_of!(escort::MODE),
        module_of!(escort::CLAN),
        module_of!(clan::TDM),
        module_of!(clan::CTF),
        module_of!(clan::SEARCH_DESTROY),
        module_of!(clan::ELIMINATION_PRO),
        RuleModule {
            name: NO_MODE,
            description: "no game mode (ca_mode none): no teams' rules, rounds, score or respawning; the dead stay dead",
            settings: &["ca_mode"],
            applies: |w| current_mode(w).is_none(),
        },
    ]
}

/// The players on each team (Alpha, Bravo) in the match, kept each tick.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CaTeamCounts(pub [u32; 2]);

/// What the HUD and the scoreboard show of the match: the server's
/// `CaMatch` (or, on a network client, the network's copy), its times on
/// this machine's clock.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct CaMatchView {
    /// The room key; empty: no mode.
    pub mode: String,
    pub goal: u32,
    pub phase: Phase,
    pub warmup: bool,
    pub round: u32,
    pub score: [i32; 2],
    pub round_wins: [u32; 2],
    /// Live: when the round timer reaches 0 (`SimClock` seconds).
    pub round_ends: Option<f64>,
    pub winner: Option<u8>,
    pub result: Option<Option<u8>>,
    pub team_counts: [u32; 2],
}

impl CaMatchView {
    /// The mode it shows.
    pub fn mode(&self) -> Option<&'static CaMode> {
        mode_by_key(&self.mode)
    }
}

/// The server's match into `CaMatchView`.
fn publish(world: &mut World) {
    let settings = world.resource::<CaRuleSettings>();
    let mode = current_mode(world).map(|m| m.key.to_string()).unwrap_or_default();
    let goal = settings.goal();
    let m = world.resource::<CaMatch>();
    let view = CaMatchView {
        mode,
        goal,
        phase: m.phase,
        warmup: m.warmup,
        round: m.round,
        score: m.score,
        round_wins: m.round_wins,
        round_ends: m.round_ends,
        winner: m.winner,
        result: m.result,
        team_counts: world.resource::<CaTeamCounts>().0,
    };
    if *world.resource::<CaMatchView>() != view {
        world.insert_resource(view);
    }
}

/// The team counts now.
pub fn update_team_counts(world: &mut World) {
    let mut counts = [0u32; 2];
    for (_, t) in flow::players(world) {
        if let Some(i) = state::side(t) {
            counts[i] += 1;
        }
    }
    let counts = CaTeamCounts(counts);
    if *world.resource::<CaTeamCounts>() != counts {
        world.insert_resource(counts);
    }
}

/// S10: a player on a team leaves (its `Team` goes as it is despawned):
/// the mode's rule, with the leaver still counted, then the mode's part
/// for its objects (`mode_left`, queued).
#[allow(clippy::type_complexity)]
fn on_leave(
    leave: On<Remove<Team>>,
    players: Query<
        (Entity, &Team, &Health, Has<Dead>, Option<&CaStats>),
        (With<Intent>, Without<Connecting>, Without<Spectating>, Without<Npc>),
    >,
    more: Query<(&Transform, Option<&objectives::CaCarry>, Has<escort::CaVip>)>,
    settings: Option<Res<CaRuleSettings>>,
    role: Option<Res<crate::core::NetRole>>,
    clock: Option<Res<SimClock>>,
    kills_made: Option<Res<free_for_all::CaKillsMade>>,
    mut state: Option<ResMut<CaMatch>>,
    mut commands: Commands,
) {
    if role.is_some_and(|r| *r == crate::core::NetRole::Client) {
        return;
    }
    let (Some(settings), Some(state)) = (settings, state.as_mut()) else { return };
    let Some(mode) = mode_by_key(&settings.mode) else { return };
    let Ok((_, team, health, dead, _)) = players.get(leave.entity) else { return };
    if !state::plays(team.0) {
        return;
    }
    let (at, carry, vip) = more
        .get(leave.entity)
        .map(|(t, c, v)| (t.translation, c.copied().unwrap_or_default(), v))
        .unwrap_or_default();
    let living = |h: &Health, d: bool| !d && h.current > 0.0;
    let in_play: Vec<_> = players.iter().filter(|(_, t, ..)| state::plays(t.0)).collect();
    let mates = in_play.iter().filter(|(_, t, ..)| t.0 == team.0);
    let leaver = Leaver {
        team: team.0,
        team_players: mates.clone().count(),
        team_alive: mates.filter(|(_, _, h, d, _)| living(h, *d)).count(),
        alive: living(health, dead),
        players: in_play.len(),
        total_kills: kills_made.map_or(0, |k| k.0),
        others: in_play
            .iter()
            .filter(|(e, ..)| *e != leave.entity)
            .map(|(_, t, ..)| t.0)
            .collect(),
        goal: settings.goal(),
        leave_kill_share: settings.objectives.leave_kill_share,
        vip,
        now: clock.map_or(0.0, |c| c.now),
    };
    (mode.leaving)(state, &leaver);
    let left = LeftPlayer {
        entity: leave.entity,
        team: team.0,
        at,
        carry,
        alive: leaver.alive,
    };
    if let Some(part) = mode_left(mode.id) {
        commands.queue(move |w: &mut World| part(w, &left));
    }
}

/// A mode's part of a leaving, after its leaving rule (world work, queued
/// by `on_leave`): the leaver's carried objects and its own (Last Man
/// Standing's and Hunted's round end, Hired Guns' pocket and mercenaries).
fn mode_left(mode: &str) -> Option<fn(&mut World, &LeftPlayer)> {
    match mode {
        LAST_MAN_STANDING | HUNTED => Some(last_man::left),
        HIRED_GUNS => Some(hired_guns::left),
        _ => None,
    }
}
/// This machine's player.
fn local_player(world: &mut World) -> Option<Entity> {
    world
        .query_filtered::<Entity, With<crate::core::LocalPlayer>>()
        .iter(world)
        .next()
}

/// Combat Arms' rule modules and the shared rules' systems.
pub struct CaRulesPlugin;

/// Whom the dead may watch (M2, Open question 35): in the team round
/// modes their living teammates, then anyone once none is left
/// (`client::spectate`'s force 2); in the free-for-all modes (Last Man
/// Standing, Hunted) anyone.
fn spectator_rules(settings: Res<CaRuleSettings>, mut rules: ResMut<crate::core::SpectatorRules>) {
    let force = match mode_by_key(&settings.mode).map(|m| m.teams) {
        Some(Teams::Two) => 2,
        _ => 0,
    };
    if rules.force_camera != force {
        rules.force_camera = force;
    }
}

impl Plugin for CaRulesPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<RuleModules>() {
            rule_module::plugin(app);
        }
        for m in modules() {
            RuleModules::register(app, m);
        }
        // The modes on the runtime (`chimera_sim::mode`), picked by
        // `ca_mode`, stepped by Combat Arms' flow (`flow::FLOW`).
        let rules: Vec<&'static Mode> = MODES.iter().map(|m| &m.rules).collect();
        app.add_modes(
            crate::mode::ModeGame {
                game: super::GAME,
                chosen: |w| w.get_resource::<CaRuleSettings>().map(|s| s.mode.as_str()),
                flow: flow::FLOW,
            },
            &rules,
        );
        settings::plugin(app);
        let server = crate::core::authoritative;
        app.init_resource::<CaMatch>()
            .init_resource::<CaMatchView>()
            .init_resource::<CaTeamCounts>()
            .init_resource::<flow::CaMissionFrames>()
            .init_resource::<spawns::CaSpawnCursors>()
            .init_resource::<scoring::CaDamageLog>()
            .init_resource::<free_for_all::CaKillsMade>()
            .init_resource::<objects::CaMapObjects>()
            .init_resource::<objectives::CaObjectives>()
            .init_resource::<objectives::CaDraws>()
            .init_resource::<bomb::CaPlanter>()
            .init_resource::<ctf::CaFlagCarriers>()
            .init_resource::<bombing_run::CaCarrier>()
            .init_resource::<quarantine::CaQuarantine>()
            .init_resource::<super::missions::CaMission>()
            .init_resource::<crate::core::SpectatorRules>()
            .init_resource::<hired_guns::CaHiredGuns>()
            .init_resource::<escort::CaEscort>()
            .init_resource::<clan::CaClan>()
            .game_hook(super::GAME, crate::gameplay::NewGame(flow::new_game))
            .add_observer(on_leave)
            // The modes' reactions to a death (`mode::ModeDeath`).
            .add_observer(ctf::on_mode_death)
            .add_observer(bombing_run::on_mode_death)
            .add_observer(hired_guns::on_mode_death)
            .add_observer(spy_hunt::on_mode_death)
            .add_observer(flow::number_players)
            // Everything below: while Combat Arms is the gameplay's rules
            // (`gameplay::gameplay_is`).
            .add_systems(FixedUpdate, objects::read_map_objects.before(SimSet::Rules).run_if(crate::gameplay::gameplay_is(super::GAME)))
            .add_systems(Update, (ctf::show_flags, spectator_rules).distributive_run_if(crate::gameplay::gameplay_is(super::GAME)))
            .add_systems(
                FixedUpdate,
                (
                    (
                        update_team_counts,
                        mode_updates(),
                        crate::mode::flow::mission_update,
                        spawns::ask_for_respawns,
                        crate::spawning::hold_the_dead,
                        flow::hold_frozen,
                        spawns::end_protection,
                        mercs::expire,
                        update_team_counts,
                        publish,
                    )
                        .chain()
                        .in_set(SimSet::Rules),
                    (scoring::scale_friendly_fire, mercs::ai_damage, escort::immunity)
                        .chain()
                        .after(SimSet::Weapons)
                        .before(crate::core::apply_damage),
                    spawns::firing_ends_protection.after(SimSet::Weapons),
                    (scoring::log_damage, scoring::handle_deaths, mode_touches(), publish)
                        .chain()
                        .after(crate::core::apply_damage),
                )
                    .run_if(server.and_then(mode_on))
                    .run_if(crate::gameplay::gameplay_is(super::GAME)),
            )
            .add_systems(
                FixedUpdate,
                publish.run_if(server.and_then(not(mode_on))).run_if(crate::gameplay::gameplay_is(super::GAME)),
            )
            .console_command(
                "ca_hire",
                "ca_hire <unit 0-9>: Hired Guns: hire a mercenary for the local player (the hire menu's keys 1-9, 0).",
                |w, a| {
                    let unit: usize = a.first().and_then(|u| u.parse().ok()).ok_or("ca_hire <unit 0-9>")?;
                    let me = local_player(w).ok_or("no local player")?;
                    if w.get_resource::<crate::core::NetRole>().is_some_and(|r| *r == crate::core::NetRole::Client) {
                        w.write_message(super::net::CaHireRequest { unit: unit as u8 });
                        return Ok(Some("hire requested".into()));
                    }
                    let e = mercs::hire(w, me, unit)?;
                    let name = w.get::<Name>(e).map_or_else(|| e.to_string(), |n| n.as_str().to_string());
                    Ok(Some(format!("hired {name}")))
                },
            )
            .console_command(
                "ca_order",
                "ca_order <hold|default|follow>: Hired Guns: the local player's mercenaries' order.",
                |w, a| {
                    let order = a
                        .first()
                        .and_then(|o| mercs::MercOrder::named(o))
                        .ok_or("ca_order <hold|default|follow>")?;
                    let me = local_player(w).ok_or("no local player")?;
                    if w.get_resource::<crate::core::NetRole>().is_some_and(|r| *r == crate::core::NetRole::Client) {
                        w.write_message(super::net::CaOrderRequest { order });
                        return Ok(Some("order sent".into()));
                    }
                    mercs::set_order(w, me, order);
                    Ok(Some(format!("order {order:?}")))
                },
            )
            .console_command(
                "ca_restart",
                "Start the match over (ca_mode's mode, ca_goal's goal): everyone respawned at the next server frame.",
                |w, _| {
                    flow::restart(w);
                    Ok(Some("match restarted".into()))
                },
            )
            .console_command(
                "ca_match",
                "The match: mode, phase, team scores, round wins, players per team.",
                |w, _| {
                    let v = w.resource::<CaMatchView>().clone();
                    let now = w.resource::<SimClock>().now;
                    let left = v.round_ends.map(|e| format!(", {:.1} s left", (e - now).max(0.0)));
                    Ok(Some(format!(
                        "{} (G {}): {:?}{}, round {}{}; Alpha {} Bravo {}; round wins {}-{}; players {}-{}; result {:?}",
                        if v.mode.is_empty() { "none" } else { &v.mode },
                        v.goal,
                        v.phase,
                        if v.warmup { " (warm-up)" } else { "" },
                        v.round,
                        left.unwrap_or_default(),
                        v.score[0],
                        v.score[1],
                        v.round_wins[0],
                        v.round_wins[1],
                        v.team_counts[0],
                        v.team_counts[1],
                        v.result
                    )))
                },
            );
    }
}
