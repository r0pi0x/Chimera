//! Team rounds with money, the Counter-Strike way (`chimera_rounds 1`):
//! a freeze period, a timed round won by eliminating the other team (on a
//! map without objectives the clock ends nothing, as in CS:S; `chimera_untimed_win 1`:
//! the defenders win on time), a pause, then everyone back at their spawns; survivors keep their weapons. Money: a start amount, kill
//! rewards, a win bonus and a loss bonus that grows with a losing streak;
//! buying is open during the freeze and the buy period.
//!
//! The rounds run on Chimera's mode runtime (`chimera_sim::mode`): three
//! modes by the map's objectives (`BOMB`, `HOSTAGE`, `ELIMINATION`, the
//! rule modules' names; none in deathmatch) on one match (`Match`), and
//! CS:S's side (`FLOW`: stepped every tick by `run_rounds`, exact
//! comparisons, a game started at once, rounds numbered as they start, the
//! outcome decided each tick by the `decide` hook and the round over in
//! the same tick, round over straight into the next freeze). The hooks
//! hold CS:S's own: the start money and the bonuses (`event`), the
//! round's setup (`round_setup`), the money, the win panel and a limit
//! reached as a round ends (`round_over`), the game's end or team balance
//! as its pause runs out (`match_end`, `after_round`). `run_rounds` keeps
//! `mp_restartgame`, the map's clock, the game end's intermission and
//! deathmatch around the step, then shows the match as `RoundState` (the
//! HUD's, `NetRound`'s and the tests' view: phase, number, wins; round
//! wins written into it outside the rules are taken back at the next
//! step) and sets the buy window, the freeze and `RoundOpen` from it.
//!
//! Objectives (`objectives`, specs/cs_source/objectives.md 7-8): on bomb
//! maps the bomb ends the round once planted (exploded: terrorists,
//! defused: counter-terrorists) and the defenders win on time; on hostage
//! maps rescuing every hostage wins for the counter-terrorists and the
//! terrorists win on time; each reason pays its own bonus.
//!
//! Defaults are data (games/cs_source/data/rounds.toml, read with
//! `rules::module::parse_settings`; the cvars set them after), CS:S's
//! (mp_freezetime 6, mp_roundtime 2.5, mp_buytime 1.5,
//! mp_startmoney 800: read from a fresh dedicated server over RCON,
//! 2026-10-14, parity-gaps.md C9) and its well-known reward amounts: the
//! kill reward (300), the elimination win (3250) and the loss bonus with its
//! streak step (1400, +500) checked against a SourceTV demo of a CS:S bot
//! round (2026-10-15, `demodump --entity CCSPlayer --props m_iAccount`); the
//! bomb, hostage and time rewards measured on the probe server (2026-10-17,
//! specs/cs_source/objectives.md Q5): the round's bonuses are paid as the
//! next round starts (`RoundState::payouts`), losers alive at a time-out
//! get nothing, hostage maps pay per hostage, an elimination pays 3250 only
//! on a bomb map (3000 elsewhere) and a draw pays nobody.
//!
//! Known shortcuts (docs/tech-debt.md): Minigame flows
//! (docs/plans/active/community-maps.md, "Minigame flows"): with players on
//! one side only, all of them dead ends the round as a draw (objectives.md
//! Q15, measured: as CS:S); our readings: a placed weapon the logic kills leaves the
//! world, from its carrier's hands too (`map::entities::RemovedWeapons`; Source
//! deletes the entity); the map logic's dice get a new seed from the clock at
//! every map load (the headless harness fixes it: `logic::LogicSeed`); only
//! output connections and game_text/env_hudhint/env_message `message`s get the
//! map's own case back (`cs_source::bsp::restore_text_case`; vbsp lower-cases
//! the whole entity lump; names, classes and paths stay lower case, matched
//! without case anyway). Why it's OK for now: 17 mg_ maps' flows pass headless
//! (`games/cs_source/tests/it/heavy/map_flows.rs`).

use bevy::prelude::*;
use chimera_sim::mode::{
    self as runtime, Flow, FlowEvent, FlowOptions, Hooks, Match, Mode, ModeAppExt, ModeGame, NO_HOOKS, Span,
    Teams,
};

use super::{Dead, game_end::GameLimits, put_at_spawn};
use crate::{
    console::{ConsoleAppExt, resource_cvar},
    core::{Died, FreezeTime, Health, Intent, Team},
    objectives::{
        MapKind, MapObjectives, ObjectiveEvent, RoundOpen,
        bomb::{BombOutcome, BombState},
        hostages::{Hostage, HostageTally},
    },
    weapon::economy::{BuyWindow, Money},
};

/// The two sides: `Team(1)` attacks (terrorists), `Team(2)` defends
/// (counter-terrorists) and wins when time runs out on a bomb map.
pub const ATTACKERS: Team = Team(1);
pub const DEFENDERS: Team = Team(2);

/// The module's numbers: CS:S's are data (games/cs_source/data/rounds.toml,
/// `Default`); the cvars set them after.
#[derive(Resource, Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoundSettings {
    /// 1: play rounds (instead of deathmatch respawns): `chimera_rounds`,
    /// not data.
    #[serde(skip)]
    pub enabled: u8,
    /// Seconds frozen at the start of a round (mp_freezetime).
    pub freeze_time: f32,
    /// Minutes a round lasts (mp_roundtime).
    pub round_time: f32,
    /// Minutes after the freeze that buying stays open (mp_buytime).
    pub buy_time: f32,
    /// mp_startmoney.
    pub start_money: u32,
    pub max_money: u32,
    /// Seconds between a round's end and the next one.
    pub end_delay: f32,
    /// The winners' bonus for an elimination on a bomb map.
    pub win_bonus: u32,
    /// Loss bonus: base, step per consecutive loss, cap.
    pub loss_bonus: u32,
    pub loss_step: u32,
    pub loss_max: u32,
    pub kill_reward: u32,
    pub teamkill_penalty: u32,
    /// `mp_tkpunish`: 1 kills this round's team killers as the next round
    /// starts (CS:S's default 0).
    pub tk_punish: u8,
    /// Objective wins (spec 8, measured): the bomb exploded; defused; time
    /// ran out on a bomb map with no plant; all hostages rescued; time ran
    /// out on a hostage map; an elimination on a map without bomb targets
    /// (hostage maps and maps without objectives).
    pub win_bomb: u32,
    pub win_defused: u32,
    pub win_target_saved: u32,
    pub win_rescued: u32,
    pub win_not_rescued: u32,
    pub win_elimination: u32,
    /// `chimera_untimed_win`: 1, on a map without objectives the defenders
    /// win when time runs out (a draw with no defenders); 0, the clock ends
    /// nothing there (CS:S, measured: objectives.md Q5).
    pub untimed_win: u8,
    /// On a hostage map the winners' bonus grows by these for each
    /// hostage: alive and not rescued; led this round (defenders only);
    /// rescued (defenders).
    pub hostage_alive_bonus: u32,
    pub hostage_led_bonus: u32,
    pub hostage_rescued_bonus: u32,
    /// Every terrorist, on top of the loss bonus, when the bomb was planted
    /// and they lost anyway.
    pub plant_loss_bonus: u32,
    pub planter_reward: u32,
    pub defuser_reward: u32,
    /// The defender who first leads a hostage this round (once per
    /// hostage); the one whose hostage is rescued.
    pub hostage_touch: u32,
    pub hostage_rescue: u32,
    /// What hurting a hostage costs a whole point of damage; the killing
    /// hit costs that twice plus `hostage_kill_penalty`.
    pub hostage_hurt_penalty: u32,
    pub hostage_kill_penalty: u32,
}

/// CS:S's round numbers (built in, parsed at run time).
const ROUNDS_DATA: (&str, &str) = ("rounds.toml", include_str!("../../data/rounds.toml"));

impl Default for RoundSettings {
    /// CS:S's (`ROUNDS_DATA`); bad data panics (its tests load it).
    fn default() -> Self {
        static SETTINGS: std::sync::OnceLock<RoundSettings> = std::sync::OnceLock::new();
        SETTINGS
            .get_or_init(|| {
                crate::rules::module::parse_settings(ROUNDS_DATA.0, ROUNDS_DATA.1)
                    .unwrap_or_else(|e| panic!("CS:S round data: {e}"))
            })
            .clone()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// Rounds are off.
    Off,
    /// Frozen until this time.
    Freeze { until: f64 },
    /// Playing; started at, ends at.
    Live { since: f64, ends: f64 },
    /// Over (winner, None: draw); the next round starts at `until`.
    Over { until: f64, winner: Option<Team> },
}

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct RoundState {
    pub phase: Phase,
    /// Rounds started since rounds were turned on.
    pub number: u32,
    /// Rounds won by attackers and defenders.
    pub wins: [u32; 2],
    /// Consecutive losses of attackers and defenders.
    losses: [u32; 2],
    /// When `mp_restartgame` restarts the game (seconds since startup).
    pub restart_at: Option<f64>,
    /// Rounds drawn (with `wins`, the rounds played: `mp_maxrounds`).
    pub draws: u32,
    /// When this game on the map started (`mp_timelimit`; set by the
    /// rules' first step).
    pub map_started: Option<f64>,
    /// The game on this map is over (`game_end`): the intermission runs
    /// until then, everyone held still and the scoreboard up.
    pub game_over: Option<f64>,
    /// What the last round's end pays, as the next round starts (CS:S's
    /// accounts change then, not at the end: spec 8, measured).
    pub payouts: Vec<(Entity, u32)>,
}

impl Default for RoundState {
    fn default() -> Self {
        Self {
            phase: Phase::Off,
            number: 0,
            wins: [0; 2],
            losses: [0; 2],
            restart_at: None,
            draws: 0,
            map_started: None,
            game_over: None,
            payouts: Vec::new(),
        }
    }
}

impl RoundState {
    /// Seconds left on the round clock (the freeze while frozen), if any.
    pub fn clock(&self, now: f64) -> Option<f32> {
        match self.phase {
            Phase::Freeze { until } => Some((until - now).max(0.0) as f32),
            Phase::Live { ends, .. } => Some((ends - now).max(0.0) as f32),
            Phase::Over { .. } | Phase::Off => None,
        }
    }
}

/// A round ended: the winner (None: draw), why, and what the win panel
/// says besides.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct RoundEnded {
    pub winner: Option<Team>,
    pub reason: RoundEndReason,
    pub summary: RoundSummary,
}

/// What CS:S's round win panel (`resource/ui/win_round.res`) shows below
/// the winner: the round's MVP and why, a fun fact, and how long the
/// round was live.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RoundSummary {
    pub mvp: Option<(Entity, Mvp)>,
    pub fun_fact: Option<FunFact>,
    /// Seconds from the end of the freeze to the round's end.
    pub seconds: f32,
}

/// Why the MVP is the MVP (the game's `winpanel_mvp_award_*` strings).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mvp {
    Kills,
    BombPlant,
    BombDefuse,
    Rescue,
}

impl Mvp {
    pub const ALL: [Mvp; 4] = [Mvp::Kills, Mvp::BombPlant, Mvp::BombDefuse, Mvp::Rescue];

    /// The localisation token (no `#`) and its English text (`%s1`: the
    /// player).
    pub fn string(self) -> (&'static str, &'static str) {
        match self {
            Mvp::Kills => ("winpanel_mvp_award_kills", "MVP: %s1 for most kills"),
            Mvp::BombPlant => ("winpanel_mvp_award_bombplant", "MVP: %s1 for planting the bomb"),
            Mvp::BombDefuse => ("winpanel_mvp_award_bombdefuse", "MVP: %s1 for defusing the bomb"),
            Mvp::Rescue => ("winpanel_mvp_award_rescue", "MVP: %s1 for rescuing the last hostage"),
        }
    }
}

/// A fun fact: the game's `funfact_*` string (token without `#`), the
/// player it names (`%s1`) and its other values (`%s2`, `%s3`).
#[derive(Clone, Debug, PartialEq)]
pub struct FunFact {
    pub token: String,
    pub player: Option<Entity>,
    pub values: Vec<String>,
}

/// What happened this round, for its summary (`RoundSummary`): enemies
/// killed and headshot kills per player, deaths per side, the first kill
/// (who, seconds into the live round), the defuser and the last hostage
/// rescuer. Reset as each round starts.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct RoundStats {
    pub kills: std::collections::HashMap<Entity, u32>,
    pub headshots: std::collections::HashMap<Entity, u32>,
    pub deaths: [u32; 2],
    pub first_kill: Option<(Entity, f32)>,
    pub defuser: Option<Entity>,
    pub rescuer: Option<Entity>,
    /// Who killed a teammate (`mp_tkpunish`).
    pub team_killers: Vec<Entity>,
}

/// Count this round's kills and objectives (`RoundStats`).
pub(super) fn track_round_stats(
    mut died: MessageReader<Died>,
    mut events: MessageReader<ObjectiveEvent>,
    teams: Query<&Team>,
    hostages: Query<(), With<Hostage>>,
    rounds: Res<RoundState>,
    time: Res<Time>,
    mut stats: ResMut<RoundStats>,
) {
    let Phase::Live { since, .. } = rounds.phase else {
        died.clear();
        events.clear();
        return;
    };
    for d in died.read() {
        if hostages.contains(d.entity) {
            continue;
        }
        let Ok(victim) = teams.get(d.entity) else { continue };
        if let Some(s) = side(*victim) {
            stats.deaths[s] += 1;
        }
        let Some(killer) = d.attacker.filter(|a| *a != d.entity) else {
            continue;
        };
        if teams.get(killer).is_ok_and(|t| t == victim) {
            if !stats.team_killers.contains(&killer) {
                stats.team_killers.push(killer);
            }
            continue;
        }
        *stats.kills.entry(killer).or_default() += 1;
        if d.damage.hitgroup == crate::core::Hitgroup::Head && d.damage.kind == crate::core::DamageKind::Bullet {
            *stats.headshots.entry(killer).or_default() += 1;
        }
        if stats.first_kill.is_none() {
            stats.first_kill = Some((killer, (time.elapsed_secs_f64() - since).max(0.0) as f32));
        }
    }
    for e in events.read() {
        match e {
            ObjectiveEvent::Defused { who } => stats.defuser = Some(*who),
            ObjectiveEvent::HostageRescued { leader: Some(l), .. } => stats.rescuer = Some(*l),
            _ => {}
        }
    }
}

/// The round's summary for the win panel. The MVP follows CS:S's
/// published award reasons (the bomb's planter when it went off, its
/// defuser, the last hostage's rescuer, else the winners' top killer;
/// none for a draw or winners without kills). Which fun fact shows is
/// ours (CS:S's choice is unmeasured; docs/tech-debt.md): the first that
/// applies of a win without casualties, a win without kills, two or more
/// headshot kills, three or more kills, first blood, a round under 30
/// seconds, else the game's fallback.
pub fn round_summary(
    stats: &RoundStats,
    planter: Option<Entity>,
    teams: &dyn Fn(Entity) -> Option<Team>,
    winner: Option<Team>,
    reason: RoundEndReason,
    seconds: f32,
    round: u32,
) -> RoundSummary {
    let top = |pick: &dyn Fn(Entity) -> bool, counts: &std::collections::HashMap<Entity, u32>| {
        // Most, then the lowest entity (a steady pick on ties).
        counts
            .iter()
            .filter(|(e, n)| **n > 0 && pick(**e))
            .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
            .map(|(e, n)| (*e, *n))
    };
    let on = |team: Option<Team>| move |e: Entity| team.is_some() && teams(e) == team;
    let mvp = match reason {
        RoundEndReason::TargetBombed => planter.map(|p| (p, Mvp::BombPlant)),
        RoundEndReason::BombDefused => stats.defuser.map(|p| (p, Mvp::BombDefuse)),
        RoundEndReason::HostagesRescued => stats.rescuer.map(|p| (p, Mvp::Rescue)),
        _ => None,
    }
    .or_else(|| top(&on(winner), &stats.kills).map(|(e, _)| (e, Mvp::Kills)));
    let fact = |token: &str, player: Option<Entity>, values: Vec<String>| FunFact {
        token: token.to_string(),
        player,
        values,
    };
    let winner_side = winner.and_then(side);
    let winners_kills: u32 = stats.kills.iter().filter(|(e, _)| on(winner)(**e)).map(|(_, n)| n).sum();
    let attackers = winner == Some(ATTACKERS);
    let fun_fact = if let Some(w) = winner_side
        && stats.deaths[w] == 0
        && stats.deaths[1 - w] > 0
    {
        Some(fact(
            if attackers { "funfact_t_win_no_casualties" } else { "funfact_ct_win_no_casualties" },
            None,
            vec![],
        ))
    } else if winner_side.is_some() && winners_kills == 0 {
        Some(fact(
            if attackers { "funfact_t_win_no_kills" } else { "funfact_ct_win_no_kills" },
            None,
            vec![],
        ))
    } else if let Some((e, n)) = top(&|_| true, &stats.headshots).filter(|(_, n)| *n >= 2) {
        Some(fact("funfact_kills_headshots", Some(e), vec![n.to_string()]))
    } else if let Some((e, n)) = top(&|_| true, &stats.kills).filter(|(_, n)| *n >= 3) {
        Some(fact("funfact_killed_enemies", Some(e), vec![n.to_string()]))
    } else if let Some((e, t)) = stats.first_kill {
        Some(fact("funfact_first_blood", Some(e), vec![format!("{t:.0}")]))
    } else if seconds < 30.0 {
        Some(fact("funfact_short_round", None, vec![format!("{seconds:.0}")]))
    } else {
        Some(fact(
            if round % 2 == 0 { "funfact_fallback1" } else { "funfact_fallback2" },
            None,
            vec![],
        ))
    };
    RoundSummary { mvp, fun_fact, seconds }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoundEndReason {
    Eliminated,
    TimeRanOut,
    Draw,
    /// The planted bomb exploded.
    TargetBombed,
    BombDefused,
    /// Time ran out on a bomb map with no bomb planted.
    TargetSaved,
    HostagesRescued,
    /// Time ran out on a hostage map.
    HostagesNotRescued,
}

impl RoundEndReason {
    /// What the game says across the screen (CS:S's words; elimination
    /// and time name the winner).
    pub fn text(self, winner: Option<Team>) -> &'static str {
        match self {
            RoundEndReason::TargetBombed => "Target Successfully Bombed!",
            RoundEndReason::BombDefused => "The bomb has been defused.",
            RoundEndReason::TargetSaved => "Target has been saved!",
            RoundEndReason::HostagesRescued => "All hostages have been rescued!",
            RoundEndReason::HostagesNotRescued => "Hostages have not been rescued!",
            RoundEndReason::Draw => "Round Draw!",
            _ => match winner {
                Some(ATTACKERS) => "Terrorists Win!",
                Some(DEFENDERS) => "Counter-Terrorists Win!",
                _ => "Round Draw!",
            },
        }
    }
}

fn side(team: Team) -> Option<usize> {
    match team {
        ATTACKERS => Some(0),
        DEFENDERS => Some(1),
        _ => None,
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<RoundSettings>()
        .init_resource::<RoundState>()
        .init_resource::<RoundStats>()
        .init_resource::<Match>()
        .init_resource::<RoundStart>()
        .add_modes(
            ModeGame {
                game: crate::objectives::GAME,
                chosen,
                flow: FLOW,
            },
            &[&BOMB, &HOSTAGE, &ELIMINATION],
        )
        .add_message::<RoundEnded>()
        .add_message::<ObjectiveEvent>()
        .add_systems(
            FixedUpdate,
            track_round_stats
                .after(crate::core::apply_damage)
                .run_if(crate::core::authoritative)
                .run_if(crate::objectives::plays),
        );
    let cvars: [(&str, &str, fn(&mut RoundSettings) -> &mut f32); 3] = [
        ("mp_freezetime", "Seconds frozen at the start of each round.", |r| {
            &mut r.freeze_time
        }),
        ("mp_roundtime", "Minutes per round.", |r| &mut r.round_time),
        ("mp_buytime", "Minutes buying stays open after the freeze.", |r| {
            &mut r.buy_time
        }),
    ];
    for (name, help, field) in cvars {
        resource_cvar::<RoundSettings, f32>(app, name, help, field);
    }
    resource_cvar::<RoundSettings, u8>(
        app,
        "chimera_rounds",
        "1: Counter-Strike rounds with money (0: deathmatch respawns).",
        |r| &mut r.enabled,
    );
    resource_cvar::<RoundSettings, u8>(
        app,
        "chimera_untimed_win",
        "1: on a map without objectives the defenders win when the round's time runs out \
         (0: the round goes on, as in CS:S).",
        |r| &mut r.untimed_win,
    );
    resource_cvar::<RoundSettings, u32>(app, "mp_startmoney", "Money each player starts with.", |r| {
        &mut r.start_money
    });
    resource_cvar::<RoundSettings, u8>(
        app,
        "mp_tkpunish",
        "1: a player who killed a teammate dies as the next round starts.",
        |r| &mut r.tk_punish,
    );
    app.console_command(
        "mp_restartgame",
        "mp_restartgame <seconds>: restart the game that many seconds from now (scores, money and the round \
         start over, everyone respawns; in deathmatch everyone respawns). 0 does nothing.",
        |w, a| {
            let secs = a
                .first()
                .and_then(|v| v.parse::<f64>().ok())
                .ok_or("mp_restartgame <seconds>")?
                .clamp(0.0, 60.0);
            if secs == 0.0 {
                return Ok(None);
            }
            // The rules' clock (fixed ticks), which can run behind the
            // frame clock when ticks are slow to simulate.
            let now = w.resource::<Time<Fixed>>().elapsed_secs_f64();
            w.resource_mut::<RoundState>().restart_at = Some(now + secs);
            Ok(Some(format!("Game will restart in {secs} seconds")))
        },
    );
}

/// `mp_restartgame` when its time comes: a fresh game (rounds), or
/// everyone back at a spawn (deathmatch).
fn restart_when_due(world: &mut World, rounds: bool, now: f64) {
    if !world.resource::<RoundState>().restart_at.is_some_and(|t| now >= t) {
        return;
    }
    if rounds {
        // Phase::Off: the next step starts a game with the start money.
        *world.resource_mut::<RoundState>() = RoundState::default();
        reset_match(world);
    } else {
        world.resource_mut::<RoundState>().restart_at = None;
        super::respawn_everyone(world);
    }
}

/// The match from the start (`Phase::Waiting`): the next step starts a
/// fresh game.
pub(super) fn reset_match(world: &mut World) {
    if let Some(mut m) = world.get_resource_mut::<Match>() {
        *m = Match::default();
    }
}

/// Advance the round: start, freeze, play, decide, pause, restart. The
/// phases are the mode runtime's (`chimera_sim::mode::flow::step_game`,
/// `FLOW`); around the step, CS:S's own: `mp_restartgame`, the map's
/// clock, the game end's intermission, deathmatch, money, and after it
/// `RoundState` (the view) and the buy window, the freeze and
/// `RoundOpen` from it.
pub(super) fn run_rounds(world: &mut World) {
    let settings = world.resource::<RoundSettings>().clone();
    let now = world.resource::<Time>().elapsed_secs_f64();
    restart_when_due(world, settings.enabled != 0, now);
    if world.resource::<RoundState>().map_started.is_none() {
        world.resource_mut::<RoundState>().map_started = Some(now);
    }
    // The game on this map is over: the intermission, then the next map.
    if let Some(until) = world.resource::<RoundState>().game_over {
        hold_intermission(world);
        if now >= until {
            super::game_end::next_map(world);
        }
        return;
    }
    let phase = world.resource::<RoundState>().phase;
    // The dead drop their weapon in rounds, not in deathmatch.
    let drops = crate::weapon::drop::DeathDrops(settings.enabled != 0);
    if world.get_resource::<crate::weapon::drop::DeathDrops>() != Some(&drops) {
        world.insert_resource(drops);
    }
    if settings.enabled == 0 {
        if phase != Phase::Off || world.get_resource::<FreezeTime>().is_some_and(|f| f.0) {
            let started = world.resource::<RoundState>().map_started;
            *world.resource_mut::<RoundState>() = RoundState {
                map_started: started,
                ..default()
            };
            world.insert_resource(BuyWindow::default());
            world.insert_resource(FreezeTime(false));
            world.insert_resource(RoundOpen(true));
            reset_match(world);
        }
        let (limits, state) = (world.resource::<GameLimits>(), world.resource::<RoundState>());
        if super::game_end::reached(limits, state, now, false) {
            super::game_end::begin(world, now);
        }
        return;
    }
    // Everyone has money while rounds run.
    let broke: Vec<Entity> = world
        .query_filtered::<Entity, (With<Intent>, With<Health>, Without<Money>, Without<Hostage>)>()
        .iter(world)
        .collect();
    for e in broke {
        world.entity_mut(e).insert(Money(settings.start_money));
    }
    // A fresh game shown (`Phase::Off`) starts the match over: one left in
    // another state (another game's, after a gameplay switch) too.
    if phase == Phase::Off
        && !matches!(
            world.resource::<Match>().phase,
            runtime::Phase::Waiting | runtime::Phase::Starting
        )
    {
        reset_match(world);
    }
    // Round wins written outside the rules (map logic's game_score, a
    // test) count from now.
    let wins = world.resource::<RoundState>().wins;
    if world.resource::<Match>().round_wins != wins {
        world.resource_mut::<Match>().round_wins = wins;
    }
    // The phases: a fresh game at once (`Prepare { first }`), the freeze
    // (`Span::Setup`), live (`Span::Round`: the outcome decided each tick,
    // `decide`), over (`Span::RoundOver`), then the next round's freeze
    // in the same step, or the game's end (`match_end`, `after_round`).
    runtime::flow::step_game(world, crate::objectives::GAME, now);
    show(world);
    // The game ended as the last round's pause ran out (`after_round`):
    // held from now.
    if world.resource::<Match>().phase == runtime::Phase::MatchOver {
        return;
    }
    // Buying: during the freeze and the buy period after it.
    let window = match world.resource::<RoundState>().phase {
        Phase::Freeze { .. } => Ok(()),
        Phase::Live { since, .. } if now - since <= settings.buy_time as f64 * 60.0 => Ok(()),
        // CS:S's words.
        _ => Err(format!(
            "{:.0} seconds have passed.\nYou can't buy anything now.",
            settings.buy_time * 60.0
        )),
    };
    if world.resource::<BuyWindow>().0 != window {
        world.insert_resource(BuyWindow(window));
    }
    let frozen = matches!(world.resource::<RoundState>().phase, Phase::Freeze { .. });
    if world.get_resource::<FreezeTime>().is_none_or(|f| f.0 != frozen) {
        world.insert_resource(FreezeTime(frozen));
    }
    // Objectives (planting) only while the round is live.
    let open = matches!(world.resource::<RoundState>().phase, Phase::Live { .. });
    if world.get_resource::<RoundOpen>().is_none_or(|o| o.0 != open) {
        world.insert_resource(RoundOpen(open));
    }
}

/// This round's `RoundSummary` from its `RoundStats`.
fn summarize(world: &mut World, winner: Option<Team>, reason: RoundEndReason, seconds: f32) -> RoundSummary {
    let stats = world.get_resource::<RoundStats>().cloned().unwrap_or_default();
    let planter = world.get_resource::<BombState>().and_then(|b| b.planter);
    let teams: std::collections::HashMap<Entity, Team> =
        world.query::<(Entity, &Team)>().iter(world).map(|(e, t)| (e, *t)).collect();
    let round = world.resource::<RoundState>().number;
    round_summary(&stats, planter, &|e| teams.get(&e).copied(), winner, reason, seconds, round)
}

/// The intermission: nobody moves, shoots or buys.
fn hold_intermission(world: &mut World) {
    if world.get_resource::<FreezeTime>().is_none_or(|f| !f.0) {
        world.insert_resource(FreezeTime(true));
    }
    if world.resource::<BuyWindow>().0.is_ok() {
        world.insert_resource(BuyWindow(Err("The game is over.".into())));
    }
    if world.get_resource::<RoundOpen>().is_none_or(|o| o.0) {
        world.insert_resource(RoundOpen(false));
    }
}

/// Whether the live round ends now, and how (spec 7's table, in order).
fn live_outcome(world: &mut World, time_up: bool) -> Option<(Option<Team>, RoundEndReason)> {
    let kind = world.get_resource::<MapObjectives>().map_or(MapKind::Neither, |o| o.kind());
    let bomb = world.get_resource::<BombState>().cloned().unwrap_or_default();
    let planted = bomb.planted.is_some() || bomb.outcome.is_some();
    match bomb.outcome {
        Some(BombOutcome::Exploded) => return Some((Some(ATTACKERS), RoundEndReason::TargetBombed)),
        Some(BombOutcome::Defused) => return Some((Some(DEFENDERS), RoundEndReason::BombDefused)),
        None => {}
    }
    let (alive, present) = team_counts(world);
    if present[0] > 0 && present[1] > 0 {
        match (alive[0], alive[1]) {
            (0, 0) if !planted => return Some((None, RoundEndReason::Draw)),
            (_, 0) => return Some((Some(ATTACKERS), RoundEndReason::Eliminated)),
            (0, _) if !planted => return Some((Some(DEFENDERS), RoundEndReason::Eliminated)),
            _ => {}
        }
    } else if present[0] + present[1] > 0 && alive == [0, 0] && !planted {
        // Players on one side only (a lone player; minigame maps with one
        // team's spawns: races, obstacle courses): all of them dead is a
        // draw and the next round, not a wait for the clock (measured,
        // objectives.md question 15).
        return Some((None, RoundEndReason::Draw));
    }
    if kind == MapKind::Hostage && world.get_resource::<HostageTally>().is_some_and(|t| t.all_rescued()) {
        return Some((Some(DEFENDERS), RoundEndReason::HostagesRescued));
    }
    // After a plant the clock no longer ends the round.
    if !time_up || planted {
        return None;
    }
    Some(match kind {
        MapKind::Bomb => (Some(DEFENDERS), RoundEndReason::TargetSaved),
        MapKind::Hostage => (Some(ATTACKERS), RoundEndReason::HostagesNotRescued),
        // CS:S's clock ends nothing on a map without objectives (measured:
        // objectives.md Q5); ours on request.
        MapKind::Neither if world.resource::<RoundSettings>().untimed_win == 0 => return None,
        MapKind::Neither if present[1] > 0 => (Some(DEFENDERS), RoundEndReason::TimeRanOut),
        MapKind::Neither => (None, RoundEndReason::Draw),
    })
}

/// Living and present characters per side.
fn team_counts(world: &mut World) -> ([u32; 2], [u32; 2]) {
    let (mut alive, mut present) = ([0; 2], [0; 2]);
    for (team, health, dead) in world
        .query_filtered::<(&Team, &Health, Has<Dead>), (
            Without<Hostage>,
            Without<crate::core::Connecting>,
            Without<crate::core::ChoosingClass>,
        )>()
        .iter(world)
    {
        if let Some(s) = side(*team) {
            present[s] += 1;
            if !dead && health.current > 0.0 {
                alive[s] += 1;
            }
        }
    }
    (alive, present)
}

/// The round's end for the money: the draws, the losing streaks and what
/// the next round's start pays (the round wins are the match's, counted
/// by the runtime).
fn end_round(world: &mut World, s: &RoundSettings, winner: Option<Team>, reason: RoundEndReason) {
    let planted = world
        .get_resource::<BombState>()
        .is_some_and(|b| b.planted.is_some() || b.outcome.is_some());
    let kind = world.get_resource::<MapObjectives>().map_or(MapKind::Neither, |o| o.kind());
    let hostage_map = kind == MapKind::Hostage;
    let rescued = world.get_resource::<HostageTally>().map_or(0, |t| t.rescued);
    let (alive_hostages, led_hostages) = world
        .query::<(&Hostage, &Health)>()
        .iter(world)
        .filter(|(_, h)| h.current > 0.0)
        .fold((0u32, 0u32), |(a, l), (h, _)| (a + 1, l + h.taken as u32));
    let mut state = world.resource_mut::<RoundState>();
    // Bonuses (spec 8, measured): the winners' by the reason, the losers'
    // growing with the streak; a draw pays nobody (measured) and leaves the
    // streaks. Paid as the next round starts.
    let mut bonus = [0u32; 2];
    if winner.is_none() {
        state.draws += 1;
        state.payouts.clear();
        return;
    }
    for (i, team) in [ATTACKERS, DEFENDERS].into_iter().enumerate() {
        if winner == Some(team) {
            state.losses[i] = 0;
            bonus[i] = match reason {
                RoundEndReason::TargetBombed => s.win_bomb,
                RoundEndReason::BombDefused => s.win_defused,
                RoundEndReason::TargetSaved => s.win_target_saved,
                RoundEndReason::HostagesRescued => s.win_rescued,
                RoundEndReason::HostagesNotRescued => s.win_not_rescued,
                RoundEndReason::Eliminated if kind != MapKind::Bomb => s.win_elimination,
                _ => s.win_bonus,
            };
            // A hostage map's winners: so much a hostage alive, and for
            // defenders a hostage led or rescued.
            if hostage_map {
                bonus[i] += s.hostage_alive_bonus * alive_hostages;
                if team == DEFENDERS {
                    bonus[i] += s.hostage_led_bonus * led_hostages + s.hostage_rescued_bonus * rescued;
                }
            }
        } else {
            bonus[i] = (s.loss_bonus + s.loss_step * state.losses[i]).min(s.loss_max);
            state.losses[i] += 1;
            // A planted bomb pays the terrorists even when they lose.
            if team == ATTACKERS && planted {
                bonus[i] += s.plant_loss_bonus;
            }
        }
    }
    // Losers still alive when time runs out on an objective get nothing
    // (measured on both map kinds).
    let time_out = matches!(reason, RoundEndReason::TargetSaved | RoundEndReason::HostagesNotRescued);
    let payouts: Vec<(Entity, u32)> = world
        .query_filtered::<(Entity, &Team, &Health, Has<Dead>), Without<Hostage>>()
        .iter(world)
        .filter_map(|(e, t, h, dead)| {
            let i = side(*t)?;
            let unpaid = time_out && winner != Some(*t) && !dead && h.current > 0.0;
            (!unpaid).then_some((e, bonus[i]))
        })
        .collect();
    world.resource_mut::<RoundState>().payouts = payouts;
}

/// CS:S's round modes on the mode runtime (`chimera_sim::mode`), one per
/// rule module that plays rounds (`game::rules::MODULES`; deathmatch,
/// `chimera_rounds 0`, is no mode: the runtime idles and `rules`
/// respawns the dead). They differ only by name: the map's objectives
/// decide (`live_outcome`, `end_round`).
pub static ELIMINATION: Mode = Mode {
    id: "cs_source:elimination",
    key: "elimination",
    shown_as: "Elimination",
    description: "rounds with money, won by eliminating the other team (the defenders on time)",
    respawns: false,
    warmup: false,
    warmup_revives: false,
    forced_setup: None,
    teams: Teams::AsJoined,
    on_death: |_, _| {},
    time_out: |_| {},
    match_end,
    hooks: Hooks {
        setup: round_setup,
        decide,
        round_over,
        after_round,
        ..NO_HOOKS
    },
};

pub static BOMB: Mode = Mode {
    id: "cs_source:bomb",
    key: "bomb",
    shown_as: "Bomb",
    description: "rounds with money: the terrorists plant the bomb, the counter-terrorists defend or defuse",
    ..ELIMINATION
};

pub static HOSTAGE: Mode = Mode {
    id: "cs_source:hostage",
    key: "hostage",
    shown_as: "Hostage",
    description: "rounds with money: the counter-terrorists rescue the hostages, the terrorists guard them",
    ..ELIMINATION
};

/// The round mode to play: by the map's objectives with rounds on, none
/// in deathmatch.
fn chosen(world: &World) -> Option<&'static str> {
    if world.get_resource::<RoundSettings>().is_none_or(|r| r.enabled == 0) {
        return None;
    }
    Some(match world.get_resource::<MapObjectives>().map_or(MapKind::Neither, |o| o.kind()) {
        MapKind::Bomb => BOMB.key,
        MapKind::Hostage => HOSTAGE.key,
        MapKind::Neither => ELIMINATION.key,
    })
}

/// CS:S's side of the mode runtime: stepped every tick by `run_rounds`
/// with exact comparisons, a game started at once (no waiting for
/// players), the outcome decided each tick (`decide`), round over
/// straight into the next round's freeze, rounds numbered as they start.
pub const FLOW: Flow = Flow {
    frame: 0.0,
    players,
    respawn,
    length,
    event,
    options: FlowOptions {
        slack: 0.0,
        wait_for_players: false,
        timer_ends_round: false,
        direct: true,
        count_at_start: true,
    },
};

/// What the round's start knows between its steps: a fresh game, and
/// last round's team killers (`mp_tkpunish`).
#[derive(Resource, Clone, Debug, Default)]
struct RoundStart {
    fresh: bool,
    team_killers: Vec<Entity>,
    /// Why the round under way ended (set by `decide`).
    reason: Option<RoundEndReason>,
}

/// Who a round's start puts back at a spawn: every character in the game
/// (not a hostage, connecting, spectating or choosing a class), team or
/// not (0).
fn players(world: &mut World) -> Vec<(Entity, u8)> {
    world
        .query_filtered::<(Entity, Option<&Team>, Has<Dead>), (
            With<Intent>,
            With<Health>,
            Without<Hostage>,
            Without<crate::core::Connecting>,
            Without<crate::core::Spectating>,
            Without<crate::core::ChoosingClass>,
        )>()
        .iter(world)
        .map(|(e, t, _)| (e, t.map_or(0, |t| t.0)))
        .collect()
}

/// Back at a spawn: the dead (or everyone, at the start of a game) with
/// fresh weapons, survivors with theirs.
fn respawn(world: &mut World, e: Entity) {
    let fresh = world.resource::<RoundStart>().fresh || world.get::<Dead>(e).is_some();
    put_at_spawn(world, e, fresh);
}

/// The phases' lengths: the freeze (mp_freezetime), the round
/// (mp_roundtime), the pause after it; no warm-up, intermission or
/// revives.
fn length(world: &mut World, span: Span) -> Option<f64> {
    let s = world.resource::<RoundSettings>();
    match span {
        Span::Setup => Some(s.freeze_time as f64),
        Span::Round => Some(s.round_time as f64 * 60.0),
        Span::RoundOver => Some(s.end_delay as f64),
        Span::Warmup | Span::Intermission | Span::Revive => None,
    }
}

/// CS:S's work at the runtime's steps: a fresh game's start money, the
/// last round's bonuses and a new round's counters.
fn event(world: &mut World, event: FlowEvent) {
    match event {
        FlowEvent::Prepare { first } => {
            world.resource_mut::<RoundStart>().fresh = first;
            if first {
                // A fresh game.
                *world.resource_mut::<RoundState>() = RoundState::default();
                let start = world.resource::<RoundSettings>().start_money;
                let all: Vec<Entity> = world
                    .query_filtered::<Entity, (With<Intent>, With<Health>, Without<Hostage>)>()
                    .iter(world)
                    .collect();
                for e in all {
                    world.entity_mut(e).insert(Money(start));
                }
            }
        }
        FlowEvent::MissionStart => {
            // The last round's bonuses, as the new one starts (before
            // anyone buys).
            let max = world.resource::<RoundSettings>().max_money;
            let payouts = std::mem::take(&mut world.resource_mut::<RoundState>().payouts);
            for (e, amount) in payouts {
                add_money(world, e, amount as i64, max);
            }
            let team_killers = std::mem::take(&mut world.resource_mut::<RoundStats>().team_killers);
            world.resource_mut::<RoundStart>().team_killers = team_killers;
            world.insert_resource(RoundStats::default());
            // The map's entities as they spawned (logic and map layers).
            crate::core::restart_round(world);
        }
        _ => {}
    }
}

/// The live round's outcome, each tick (spec 7's table): a decided round
/// ends now.
fn decide(world: &mut World, time_up: bool) {
    let Some((winner, reason)) = live_outcome(world, time_up) else { return };
    world.resource_mut::<RoundStart>().reason = Some(reason);
    let mut m = world.resource_mut::<Match>();
    match winner {
        Some(team) => m.win(team.0),
        None => m.end_without_winner(),
    }
}

/// The round is over: the money, the win panel's summary (`RoundEnded`)
/// and, with a limit reached, the game's end at once (as CS:S's: its
/// "Intermission_Time_Limit" in the same second as "Round_End";
/// parity-gaps.md C9).
fn round_over(world: &mut World) {
    let s = world.resource::<RoundSettings>().clone();
    let now = world.resource::<Time>().elapsed_secs_f64();
    let (winner, since) = {
        let m = world.resource::<Match>();
        (m.winner.map(Team), m.live_since)
    };
    let reason = world.resource_mut::<RoundStart>().reason.take().unwrap_or(RoundEndReason::Draw);
    show(world);
    end_round(world, &s, winner, reason);
    let summary = summarize(world, winner, reason, (now - since).max(0.0) as f32);
    world.write_message(RoundEnded {
        winner,
        reason,
        summary,
    });
    let (limits, state) = (world.resource::<GameLimits>(), world.resource::<RoundState>());
    if super::game_end::reached(limits, state, now, true) {
        super::game_end::begin(world, now);
    }
}

/// As the round's pause runs out: a limit reached ends the game on the
/// map (the match's result: the side with more round wins, None on a
/// tie).
fn match_end(world: &World) -> Option<Option<u8>> {
    let now = world.resource::<Time>().elapsed_secs_f64();
    let (limits, state) = (world.resource::<GameLimits>(), world.resource::<RoundState>());
    if !super::game_end::reached(limits, state, now, true) {
        return None;
    }
    let [a, d] = state.wins;
    Some((a != d).then_some(if a > d { ATTACKERS.0 } else { DEFENDERS.0 }))
}

/// After the round's end: the game's end begins (held from now), or the
/// teams are balanced for the next round.
fn after_round(world: &mut World) {
    if world.resource::<Match>().result.is_some() {
        let now = world.resource::<Time>().elapsed_secs_f64();
        super::game_end::begin(world, now);
        hold_intermission(world);
    } else {
        super::balance_teams(world);
    }
}

/// `RoundState` from the match (`Match`): the phase (kept while the game
/// is over), the round's number and the round wins. Written only when
/// they change.
fn show(world: &mut World) {
    let m = world.resource::<Match>();
    let phase = match m.phase {
        runtime::Phase::Waiting | runtime::Phase::Starting => Some(Phase::Off),
        runtime::Phase::Setup { until } => Some(Phase::Freeze { until }),
        runtime::Phase::Live => Some(Phase::Live {
            since: m.live_since,
            ends: m.round_ends.unwrap_or(f64::INFINITY),
        }),
        runtime::Phase::RoundOver { until } => Some(Phase::Over {
            until,
            winner: m.winner.map(Team),
        }),
        runtime::Phase::Intermission { .. } | runtime::Phase::MatchOver => None,
    };
    let (number, wins) = (m.round, m.round_wins);
    let state = world.resource::<RoundState>();
    if phase.is_some_and(|p| p != state.phase) || state.number != number || state.wins != wins {
        let mut state = world.resource_mut::<RoundState>();
        if let Some(p) = phase {
            state.phase = p;
        }
        state.number = number;
        state.wins = wins;
    }
}

/// A round's setup, after everyone is back at a spawn: `mp_tkpunish`,
/// the bots' shopping, last round's loose weapons gone, the objectives'
/// round start.
fn round_setup(world: &mut World) {
    let (fresh, team_killers) = {
        let mut start = world.resource_mut::<RoundStart>();
        (start.fresh, std::mem::take(&mut start.team_killers))
    };
    // `mp_tkpunish`: last round's team killers die now (by the world, as
    // a fall would kill them; how CS:S counts it is unmeasured).
    if world.resource::<RoundSettings>().tk_punish != 0 && !fresh {
        for e in team_killers {
            if world.get::<Health>(e).is_some_and(|h| h.current > 0.0) {
                let at = world.get::<Transform>(e).map_or(Vec3::ZERO, |t| t.translation);
                world.write_message(crate::core::Damage {
                    target: e,
                    attacker: None,
                    amount: 1000.0,
                    point: at,
                    dir: Vec3::NEG_Y,
                    hitgroup: crate::core::Hitgroup::Generic,
                    kind: crate::core::DamageKind::Generic,
                    weapon: None,
                    force: Vec3::ZERO,
                });
            }
        }
    }
    // Computer players shop at once.
    let bots: Vec<Entity> = world
        .query_filtered::<Entity, (
            With<Intent>,
            With<Health>,
            Without<crate::core::LocalPlayer>,
            Without<crate::core::RemotePlayer>,
            Without<Hostage>,
        )>()
        .iter(world)
        .collect();
    world.insert_resource(BuyWindow::default());
    for e in bots {
        crate::weapon::economy::autobuy(world, e);
    }
    // Loose weapons from the last round go.
    let loose: Vec<(Entity, Entity)> = world
        .query::<(Entity, &crate::weapon::drop::Loose)>()
        .iter(world)
        .map(|(e, l)| (e, l.weapon))
        .collect();
    for (e, w) in loose {
        world.despawn(e);
        world.despawn(w);
    }
    // The bomb to a terrorist, hostages back.
    crate::objectives::round_start(world);
}

fn add_money(world: &mut World, e: Entity, delta: i64, max: u32) {
    if let Some(mut m) = world.get_mut::<Money>(e) {
        m.0 = (m.0 as i64 + delta).clamp(0, max as i64) as u32;
    }
}

/// Kill rewards (and the team-kill penalty).
pub(super) fn kill_rewards(
    settings: Res<RoundSettings>,
    mut died: MessageReader<Died>,
    teams: Query<&Team>,
    hostages: Query<(), With<Hostage>>,
    mut money: Query<&mut Money>,
) {
    for d in died.read() {
        // Hostages cost money instead (`objective_money`).
        if settings.enabled == 0 || hostages.contains(d.entity) {
            continue;
        }
        let Some(killer) = d.attacker.filter(|a| *a != d.entity) else {
            continue;
        };
        let same = matches!((teams.get(killer), teams.get(d.entity)), (Ok(a), Ok(b)) if a == b);
        if let Ok(mut m) = money.get_mut(killer) {
            m.0 = if same {
                m.0.saturating_sub(settings.teamkill_penalty)
            } else {
                (m.0 + settings.kill_reward).min(settings.max_money)
            };
        }
    }
}

/// While frozen, nobody moves or shoots; they can still look around,
/// switch weapons and reload (playtest 2026-10-10: "the gun does not
/// reload after the round ends": R did nothing in the next round's
/// freeze). Bots think before the rules (`bot::BotPlugin`), so this holds
/// them too. A network client holds its own the same way
/// (`net::predict::hold_local`).
pub(super) fn hold_frozen(frozen: Res<FreezeTime>, mut intents: Query<&mut Intent>) {
    if !frozen.0 {
        return;
    }
    for mut intent in &mut intents {
        *intent = frozen_intent(&intent);
    }
}

/// What a frozen player's intent keeps: the look, weapon picks and reload.
pub fn frozen_intent(intent: &Intent) -> Intent {
    Intent {
        yaw: intent.yaw,
        pitch: intent.pitch,
        select: intent.select,
        last_weapon: intent.last_weapon,
        reload: intent.reload,
        command: intent.command,
        ..default()
    }
}

/// Money for objectives as they happen (spec 8, measured): planting,
/// defusing, leading and rescuing hostages, hurting and killing them.
pub(super) fn objective_money(
    settings: Res<RoundSettings>,
    mut events: MessageReader<ObjectiveEvent>,
    mut money: Query<&mut Money>,
    mut last_hit: Local<std::collections::HashMap<Entity, f32>>,
) {
    // What a hit on a hostage costs: so much a whole point (health 1.0 is
    // 100 points).
    let hurt = |amount: f32| ((amount.max(0.0) * 100.0 + 1e-3).floor() as i64) * settings.hostage_hurt_penalty as i64;
    // Hostages hurt but never killed (re-created each round) don't pile up.
    if last_hit.len() > 64 {
        last_hit.clear();
    }
    for ev in events.read() {
        if settings.enabled == 0 {
            continue;
        }
        let (who, delta) = match ev {
            ObjectiveEvent::Planted { who, .. } => (Some(*who), settings.planter_reward as i64),
            ObjectiveEvent::Defused { who } => (Some(*who), settings.defuser_reward as i64),
            ObjectiveEvent::HostageFollows { leader, first: true, .. } => (Some(*leader), settings.hostage_touch as i64),
            ObjectiveEvent::HostageRescued { leader, .. } => (*leader, settings.hostage_rescue as i64),
            ObjectiveEvent::HostageHurt {
                hostage,
                attacker,
                amount,
            } => {
                last_hit.insert(*hostage, *amount);
                (*attacker, -hurt(*amount))
            }
            // The killing hit costs its hurt penalty again, and the kill.
            ObjectiveEvent::HostageKilled { hostage, attacker } => {
                let hit = last_hit.remove(hostage).unwrap_or(0.0);
                (*attacker, -(hurt(hit) + settings.hostage_kill_penalty as i64))
            }
            _ => (None, 0),
        };
        if delta != 0
            && let Some(mut m) = who.and_then(|w| money.get_mut(w).ok())
        {
            m.0 = (m.0 as i64 + delta).clamp(0, settings.max_money as i64) as u32;
        }
    }
}
