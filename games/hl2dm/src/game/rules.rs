//! HL2:DM's deathmatch and team deathmatch (specs/hl2dm/rules.md, H6) as
//! two modes on Chimera's mode runtime (`chimera_sim::mode`, `hl2dm:dm`
//! and `hl2dm:tdm`): no rounds, one live "round" from the map's load to a
//! frag or time limit, then the intermission and the map cycle's next map.
//! `mp_teamplay` is read once, at the map's load (`new_game`).
//!
//! - `players`: who plays (`Hl2Player`: HL2:DM's team number, model and
//!   timers), joining (DM: team 0, every player an enemy; TDM: the team
//!   with fewer players, a random one on a tie), the 19 models and their
//!   round-robin replacement, `cl_playermodel` changes and their 5 s limit
//!   (a remote client's setting rewritten by the server: `chimera_net`'s
//!   `UserInfoSet`),
//!   `jointeam`/`spectate` and their 0.3 s rate, a TDM team change's
//!   suicide charged to the new team, leaving.
//! - `spawns`: the spot (the class ring walk from the class's last spot,
//!   N in 1-5 with the empty slot taking the first spot, the first free one
//!   within 128 units, the 48-unit telefrag when none is, the team classes
//!   falling back to deathmatch's, `info_player_start` last) and the spawn
//!   itself (1 unit up, the spot's angles, the suit and loadout fresh, the
//!   0.5 s refusal of pure blast).
//! - `deaths`: scoring (frags, deaths, the team-point rule, team kills
//!   and suicides), the death notice's weapon name, `player_death`, the
//!   log lines, the respawn (all buttons released once, then a press or
//!   `mp_forcerespawn`'s 5 s, more than 3 s after the death), the suicide
//!   commands' 5 s limit.
//! - `limits`: the time and frag limits (the runtime's `decide`), the
//!   intermission (everyone frozen, the scoreboard up, nothing counted but
//!   team points, `mp_chattime`), the next map (`nextlevel`, else the map
//!   cycle), `mp_restartgame` (ragdolls, projectiles and delayed entity
//!   I/O gone), `mp_readyrestart`, `timeleft`.
//!
//! The match is on once a map is loaded as a new game (`gameplay::
//! NewGame`) or an app starts it (`start`); scenario tests start it when
//! they test the rules (`Hl2Sim` alone runs none, so its characters stay
//! where the tests put them). Server-side, while HL2:DM is the gameplay.
//!
//! The dead stay at their body (no observer phase): the view is
//! `game::bodies`' (`core::OrbitDeadView`). `say_team` in DM reaches only its sender
//! (every DM player is a team of its own to Chimera, `mode::FFA_FIRST`
//! on); the dead are heard by the living (`map::radio::SayFormats::
//! dead_heard`, HL2:DM's chat formats).
//!
//! Known shortcuts (C5 wasn't needed: the mode runtime's phases, its
//! `decide` hook and the sides' counters carry DM and TDM; nothing was
//! lifted from CS:S's rules):
//! - The message of the day and the chat lines
//!   the rules print ("you are on team ...", "must wait N seconds") are
//!   console lines; the scoreboard is the HUD's (M9, `ui::scoreboard`;
//!   `standing` replicates the intermission, `Hl2Match::over`).
//! - The map-start freeze (3 s of zeroed commands, the view free) comes
//!   with every map loaded as a new game, a `changelevel` included (Open
//!   questions 10 leaves that to be measured); tests' `start` has none.
//! - A spot's `master` isn't read (no stock map has one).

pub mod deaths;
pub mod limits;
pub mod players;
pub mod spawns;

use bevy::prelude::*;
use chimera_sim::mode::{self, Flow, FlowEvent, FlowOptions, Match, Mode, ModeAppExt, ModeGame, Span, Teams};

use crate::{
    console::{ConsoleAppExt, resource_cvar},
    core::{FriendlyFire, SimClock, SimSet},
};

pub use players::Hl2Player;

/// HL2:DM's team numbers.
pub const UNASSIGNED: u8 = 0;
pub const SPECTATOR: u8 = 1;
pub const COMBINE: u8 = 2;
pub const REBELS: u8 = 3;

/// A team's name (scoreboard, logs).
pub fn team_name(team: u8) -> &'static str {
    match team {
        SPECTATOR => "Spectator",
        COMBINE => "Combine",
        REBELS => "Rebels",
        _ => "Unassigned",
    }
}

/// The modes' ids and keys.
pub const DM: &str = "hl2dm:dm";
pub const TDM: &str = "hl2dm:tdm";

/// Seconds: a respawn needs more than `RESPAWN_MIN_DEAD` since the death,
/// `mp_forcerespawn` forces one after more than `FORCE_RESPAWN_AFTER`.
pub const RESPAWN_MIN_DEAD: f64 = 3.0;
pub const FORCE_RESPAWN_AFTER: f64 = 5.0;
/// Seconds between model changes, and between accepted `jointeam`s and
/// `spectate`s.
pub const MODEL_CHANGE_INTERVAL: f64 = 5.0;
pub const TEAM_CMD_RATE: f64 = 0.3;
/// A spot with another player within this (units) is occupied.
pub const SPAWN_CHECK_RADIUS: f32 = 128.0;
/// The player stands this far (units) above its spot.
pub const SPAWN_RAISE: f32 = 1.0;
/// After a spawn, pure blast damage is refused this long (s).
pub const SPAWN_BLAST_GRACE: f64 = 0.5;
/// The spawn telefrag's damage (points).
pub const TELEFRAG_DAMAGE: f32 = 300.0;
/// Seconds between suicides.
pub const SUICIDE_INTERVAL: f64 = 5.0;
/// After a map loaded as a new game, players' commands are zeroed this
/// long (s; rules.md Edge cases, with `developer` 0).
pub const MAP_START_FREEZE: f64 = 3.0;
/// The restart and ready checks' period (s).
pub const RULES_PERIOD: f64 = 1.0;
/// All ready: the restart this long after (s).
pub const READY_RESTART_DELAY: f64 = 5.0;
/// The most `mp_restartgame` waits (s).
pub const RESTART_MAX: f64 = 60.0;

/// HL2:DM's shipped map cycle (`cfg/mapcycle_default.txt`), for a game
/// without its install's.
pub const DEFAULT_CYCLE: &[&str] = &[
    "dm_lockdown",
    "dm_overwatch",
    "dm_powerhouse",
    "dm_resistance",
    "dm_runoff",
    "dm_steamlab",
    "dm_underpass",
];

/// The rules' cvars (rules.md "Constants"; `mp_forcerespawn` 0, as the
/// shipped `cfg/valve.rc` sets it and the M3 probe read on the stock
/// dedicated server).
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Hl2Rules {
    pub teamplay: u8,
    pub fraglimit: f32,
    pub timelimit: f32,
    pub chattime: f32,
    pub forcerespawn: u8,
    pub friendlyfire: u8,
    pub allowspectators: u8,
    pub frag_fallback_radius: f32,
    pub nextlevel: String,
    pub restartgame: f32,
    pub readyrestart: u8,
    pub ready_signal: String,
}

impl Default for Hl2Rules {
    fn default() -> Self {
        Self {
            teamplay: 0,
            fraglimit: 0.0,
            timelimit: 0.0,
            chattime: 10.0,
            forcerespawn: 0,
            friendlyfire: 0,
            allowspectators: 1,
            frag_fallback_radius: 48.0,
            nextlevel: String::new(),
            restartgame: 0.0,
            readyrestart: 0,
            ready_signal: "ready".into(),
        }
    }
}

/// The match's state beyond the runtime's (`mode::Match`: the phases and
/// the sides' team scores, Combine first).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct Hl2Match {
    /// The rules play (a map loaded as a new game, or `start`).
    pub on: bool,
    /// `mp_teamplay` as read at the map's load.
    pub teamplay: bool,
    /// The time limit counts from here (s).
    pub game_start: f64,
    /// The game is over: the intermission (`intermission_end`), then the
    /// map change.
    pub over: bool,
    pub intermission_end: Option<f64>,
    /// The last spot taken per class: deathmatch, Combine, Rebels (index
    /// into the class's spots).
    pub last_spot: [Option<usize>; 3],
    /// The once-a-second checks' next time.
    pub next_check: f64,
    /// A pending `mp_restartgame` (s).
    pub restart_at: Option<f64>,
    /// Waiting for every player to say `mp_ready_signal`.
    pub waiting_ready: bool,
    /// The map the game goes to (decided at the intermission's end).
    pub next_map: Option<String>,
    /// Dice for the spawn walk and the team tie (a seeded generator).
    pub dice: u64,
    /// The map-start freeze: players' commands are zeroed until this (s;
    /// a map loaded as a new game, `MAP_START_FREEZE`).
    pub freeze_until: Option<f64>,
}

impl Hl2Match {
    /// A roll in `0..n`.
    pub fn roll(&mut self, n: u64) -> u64 {
        // splitmix64.
        self.dice = self.dice.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.dice;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        z % n.max(1)
    }
}

/// An app's HL2:DM plays its rules from the first map that loads
/// (`new_game` once the map's entities are in); `Hl2Sim` alone doesn't, so its
/// scenario tests put characters where they want.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AutoStart(pub bool);

fn auto_start(auto: Option<Res<AutoStart>>, m: Res<Hl2Match>, map: Option<Res<crate::map::MapEntities>>, mut commands: Commands) {
    if auto.is_some_and(|a| a.0) && !m.on && map.is_some() {
        // The first map is a map loaded as a new game: its freeze too.
        commands.queue(new_game);
    }
}

/// Whether the rules play now (and HL2:DM is the gameplay).
pub fn rules_on(m: Option<Res<Hl2Match>>) -> bool {
    m.is_some_and(|m| m.on)
}

/// The modes as the runtime plays them (`mode::Mode`): the dead respawn
/// (by HL2:DM's own rule, `deaths::respawns`), the teams are the rules'
/// own (`Teams::AsJoined`), the round never times out (the limits decide
/// it, `limits::decide`) and the match is over after it.
pub static DM_MODE: Mode = Mode {
    id: DM,
    key: "dm",
    shown_as: "Deathmatch",
    description: "Everyone for themselves; the frag or time limit ends the map.",
    respawns: true,
    warmup: false,
    warmup_revives: false,
    forced_setup: Some(0.0),
    teams: Teams::AsJoined,
    on_death: |_, _| {},
    time_out: |_| {},
    match_end: |_| Some(None),
    hooks: mode::Hooks {
        decide: limits::decide,
        round_over: limits::begin_intermission,
        ..mode::NO_HOOKS
    },
};

pub static TDM_MODE: Mode = Mode {
    id: TDM,
    key: "tdm",
    shown_as: "Team Deathmatch",
    description: "Combine against Rebels; the team score or the time limit ends the map.",
    ..DM_MODE
};

/// HL2:DM's side of the runtime: every tick, exact comparisons (a hair
/// under, for the strict ones), no waiting for players, the limits
/// deciding, the match over straight after its one round.
pub const FLOW: Flow = Flow {
    frame: 0.0,
    players: players::players,
    respawn: spawns::spawn,
    length,
    event,
    options: FlowOptions {
        slack: -1e-9,
        wait_for_players: false,
        timer_ends_round: false,
        direct: true,
        count_at_start: true,
    },
};

fn length(world: &mut World, span: Span) -> Option<f64> {
    match span {
        Span::Setup => Some(0.0),
        // The intermission: `mp_chattime`, whole seconds, 1 to 120.
        Span::RoundOver => {
            let t = world.get_resource::<Hl2Rules>().map_or(10.0, |r| r.chattime);
            Some((t.trunc() as f64).clamp(1.0, 120.0))
        }
        _ => None,
    }
}

fn event(world: &mut World, event: FlowEvent) {
    match event {
        FlowEvent::ResetStats => players::reset_scores(world),
        FlowEvent::MatchEnd => limits::change_map(world),
        _ => {}
    }
}

/// HL2:DM's rules (`hl2dm:dm`, `hl2dm:tdm`).
pub struct Hl2RulesPlugin;

impl Plugin for Hl2RulesPlugin {
    fn build(&self, app: &mut App) {
        use crate::gameplay::GameplayAppExt;
        app.add_modes(
            ModeGame {
                game: super::GAME,
                chosen: |w| {
                    let m = w.get_resource::<Hl2Match>().filter(|m| m.on)?;
                    Some(if m.teamplay { "tdm" } else { "dm" })
                },
                flow: FLOW,
            },
            &[&DM_MODE, &TDM_MODE],
        )
        .init_resource::<Hl2Rules>()
        .init_resource::<Hl2Match>()
        .init_resource::<Match>()
        .init_resource::<players::ModelCounters>()
        .init_resource::<crate::core::RoundRestarts>()
        .init_resource::<FriendlyFire>()
        .add_message::<crate::core::Damage>()
        .add_message::<chimera_core::game_log::GameEvent>()
        .game_hook(super::GAME, crate::gameplay::NewGame(new_game))
        .add_observer(players::left)
        .add_systems(
            FixedUpdate,
            auto_start
                .before(SimSet::Rules)
                .run_if(crate::core::authoritative)
                .run_if(crate::gameplay::gameplay_is(super::GAME)),
        )
        .add_systems(
            FixedUpdate,
            (
                (
                    sync_cvars,
                    players::join,
                    players::model_changes,
                    deaths::respawns,
                    mode::flow::mission_update,
                    limits::checks,
                    chimera_sim::spawning::hold_the_dead,
                    limits::hold_frozen,
                )
                    .chain()
                    .in_set(SimSet::Rules),
                deaths::refuse.after(SimSet::Weapons).before(crate::core::apply_damage),
                deaths::deaths.after(crate::core::apply_damage),
            )
                .run_if(rules_on)
                .run_if(crate::core::authoritative)
                .run_if(crate::gameplay::gameplay_is(super::GAME)),
        );
        let cvars: [(&str, &str, fn(&mut Hl2Rules) -> &mut f32); 4] = [
            ("hl2dm:mp_fraglimit", "Frags (DM: a player's; TDM: a team's score) that end the map (0: none).", |r| &mut r.fraglimit),
            ("hl2dm:mp_timelimit", "Whole minutes on a map (0: none).", |r| &mut r.timelimit),
            ("hl2dm:mp_chattime", "Seconds of intermission before the next map (1-120).", |r| &mut r.chattime),
            ("hl2dm:mp_restartgame", "Restart the game in this many seconds (at most 60).", |r| &mut r.restartgame),
        ];
        for (name, help, field) in cvars {
            resource_cvar::<Hl2Rules, f32>(app, name, help, field);
        }
        let flags: [(&str, &str, fn(&mut Hl2Rules) -> &mut u8); 5] = [
            ("hl2dm:mp_teamplay", "1: team deathmatch (Combine against Rebels), read at the next map's load.", |r| &mut r.teamplay),
            ("hl2dm:mp_forcerespawn", "1: the dead respawn by themselves 5 s after death (else on a button).", |r| &mut r.forcerespawn),
            ("hl2dm:mp_friendlyfire", "1: teammates hurt each other (TDM).", |r| &mut r.friendlyfire),
            ("hl2dm:mp_allowspectators", "0: nobody may spectate.", |r| &mut r.allowspectators),
            ("hl2dm:mp_readyrestart", "1: restart once every player says mp_ready_signal.", |r| &mut r.readyrestart),
        ];
        for (name, help, field) in flags {
            resource_cvar::<Hl2Rules, u8>(app, name, help, field);
        }
        resource_cvar::<Hl2Rules, f32>(
            app,
            "hl2dm:hl2mp_spawn_frag_fallback_radius",
            "With every spawn spot taken, players this near (units) the chosen one die.",
            |r| &mut r.frag_fallback_radius,
        );
        resource_cvar::<Hl2Rules, String>(app, "hl2dm:nextlevel", "The next map, once (cleared at each map load).", |r| {
            &mut r.nextlevel
        });
        resource_cvar::<Hl2Rules, String>(app, "hl2dm:mp_ready_signal", "What every player says to be ready.", |r| {
            &mut r.ready_signal
        });
        players::commands(app);
        deaths::commands(app);
        limits::commands(app);
        app.console_command(
            "hl2dm:hl2dm_match",
            "The match: mode, frags and team scores, time left, the intermission.",
            |w, _| {
                let m = w.resource::<Hl2Match>().clone();
                let score = w.resource::<Match>().score;
                Ok(Some(format!(
                    "{} {}; Combine {} Rebels {}; over {} next map {:?}",
                    if m.on { "on" } else { "off" },
                    if m.teamplay { "TDM" } else { "DM" },
                    score[0],
                    score[1],
                    m.over,
                    m.next_map
                )))
            },
        );
    }
}

/// `mp_friendlyfire` to the engine's (`core::FriendlyFire`).
fn sync_cvars(rules: Res<Hl2Rules>, mut ff: ResMut<FriendlyFire>) {
    if ff.0 != rules.friendlyfire {
        ff.0 = rules.friendlyfire;
    }
}

/// A fresh game on the map just loaded (`gameplay::NewGame`), or the
/// first (`start`): `mp_teamplay` read, the time limit from now, the last
/// spots and `nextlevel` cleared, the runtime's match from its start (its
/// first step respawns everyone, scores at 0); a map load freezes
/// everyone's commands for its first 3 s (`MAP_START_FREEZE`).
pub fn new_game(world: &mut World) {
    begin(world, true);
}

fn begin(world: &mut World, freeze: bool) {
    let teamplay = world.get_resource::<Hl2Rules>().is_some_and(|r| r.teamplay != 0);
    let now = world.get_resource::<SimClock>().map_or(0.0, |c| c.now);
    let dice = world.get_resource::<Hl2Match>().map_or(0x5eed, |m| m.dice);
    world.insert_resource(Hl2Match {
        on: true,
        teamplay,
        game_start: now,
        next_check: now,
        dice,
        freeze_until: freeze.then_some(now + MAP_START_FREEZE),
        ..default()
    });
    world.insert_resource(Match::default());
    world.insert_resource(crate::core::FreezeTime(false));
    if let Some(mut r) = world.get_resource_mut::<Hl2Rules>() {
        r.nextlevel.clear();
    }
    world.get_resource_or_init::<crate::core::MapLoads>().0 += 1;
    // The cycle's place: the map that plays (the install's cycle if the
    // app gave one, HL2:DM's shipped one otherwise).
    let current = world.get_resource::<crate::map::MapFile>().map(|f| f.name.clone());
    let mut cycle = world.get_resource_or_init::<crate::core::MapCycle>();
    if cycle.maps.is_empty() {
        cycle.maps = DEFAULT_CYCLE.iter().map(|m| m.to_string()).collect();
    }
    if let Some(name) = current {
        cycle.current = name.rsplit(':').next().unwrap_or(&name).to_string();
    }
    players::new_map(world);
}

/// Start the rules on the map that plays (a test, or an app starting
/// them itself), without the map-start freeze.
pub fn start(world: &mut World) {
    begin(world, false);
}
