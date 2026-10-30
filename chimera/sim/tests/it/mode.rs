//! The mode runtime's contract (`chimera_sim::mode`) on the test kit: a
//! made-up game (`kit`) registers two modes and its flow, and the runtime
//! steps the match through its phases at the game's frames, calling the
//! game's steps and the mode's hooks in the documented order; a counted
//! death's win ends the round at the next frame; the gameplay picks
//! whose modes play; free-for-all teams. With the options the other way
//! (CS:S's rounds, a game stepping its own match with `flow::step_game`):
//! the match starts at the first step, rounds numbered as they start,
//! phases due exactly, the timer only telling the mode, a decided round
//! over in the same step and the next round's setup straight after round
//! over, with no intermission.

use bevy::prelude::*;
use chimera_core::{
    core::{Dead, FreezeTime, SimSet, Team},
    gameplay::Gameplay,
};
use chimera_sim::{
    mode::{
        self, ALPHA, BRAVO, Death, DeathKind, FFA_FIRST, Flow, FlowEvent, FlowOptions, Match, Mode, ModeAppExt, ModeGame,
        NO_HOOKS, Phase, Span, Teams, flow,
    },
    movement::placeholder,
    testkit::kit_sim,
};
use chimera_world::{greybox, harness::Sim};

/// What the kit's game and modes were asked to do, in order.
#[derive(Resource, Default)]
struct Log(Vec<String>);

fn log(world: &mut World, line: impl Into<String>) {
    world.get_resource_or_init::<Log>().0.push(line.into());
}

/// The mode the kit plays (`ca_mode`'s stand-in).
#[derive(Resource)]
struct Chosen(&'static str);

/// First side to 3 kills; respawning; no rounds' timer.
static DEATHMATCH: Mode = Mode {
    id: "kit:deathmatch",
    key: "dm",
    shown_as: "Deathmatch",
    description: "first side to 3 kills",
    respawns: true,
    warmup: false,
    warmup_revives: false,
    forced_setup: Some(0.0),
    teams: Teams::Two,
    on_death: |w, d| {
        let mut m = w.resource_mut::<Match>();
        let Some(i) = mode::side(mode::enemy(d.victim_team)) else { return };
        m.score[i] += 1;
        if m.score[i] >= 3 {
            m.win(i as u8 + 1);
        }
    },
    time_out: |_| {},
    match_end: |w| Some(w.resource::<Match>().winner),
    hooks: NO_HOOKS,
};

/// Rounds after a warm-up; Alpha wins a time-out; 2 round wins take the
/// match.
static ROUNDS: Mode = Mode {
    id: "kit:rounds",
    key: "rounds",
    shown_as: "Rounds",
    description: "rounds, 2 wins take the match",
    respawns: false,
    warmup: true,
    warmup_revives: true,
    forced_setup: None,
    teams: Teams::Two,
    on_death: |_, _| {},
    time_out: |w| w.resource_mut::<Match>().winner = Some(ALPHA),
    match_end: |w| {
        let m = w.resource::<Match>();
        m.round_wins.iter().any(|r| *r >= 2).then_some(Some(ALPHA))
    },
    hooks: mode::Hooks {
        setup: |w| log(w, "mode setup"),
        before_spawns: |w| log(w, "mode before_spawns"),
        round_over: |w| log(w, "mode round_over"),
        round_end: |w| log(w, "mode round_end"),
        after_round: |w| log(w, "mode after_round"),
        ..NO_HOOKS
    },
};

/// Whether `WARMUP_ENDS`' match is over after its warm-up.
#[derive(Resource, Default)]
struct EndAfterWarmup(bool);

/// `ROUNDS`, but its warm-up's end can end the match (`Hooks::warmup_end`).
static WARMUP_ENDS: Mode = Mode {
    id: "kit:warmup_ends",
    key: "warmup_ends",
    hooks: mode::Hooks {
        warmup_end: |w| w.get_resource::<EndAfterWarmup>().is_some_and(|e| e.0).then_some(Some(ALPHA)),
        ..NO_HOOKS
    },
    ..ROUNDS
};

/// Every player a team of their own.
static FREE_FOR_ALL: Mode = Mode {
    id: "kit:ffa",
    key: "ffa",
    teams: Teams::FreeForAll,
    ..DEATHMATCH
};

const FRAME: f64 = 0.1;

const FLOW: Flow = Flow {
    frame: FRAME,
    players: |w| {
        let mut all: Vec<(Entity, u8)> = w
            .query::<(Entity, &Team)>()
            .iter(w)
            .filter(|(_, t)| mode::plays(t.0))
            .map(|(e, t)| (e, t.0))
            .collect();
        all.sort();
        all
    },
    respawn: |w, e| {
        chimera_sim::spawning::revive(w, e, None);
        log(w, "respawn");
    },
    length: |w, span| match span {
        Span::Setup => Some(1.0),
        Span::Round => (!mode::current_mode(w).is_some_and(|m| m.respawns)).then_some(5.0),
        Span::Warmup => Some(2.0),
        Span::RoundOver => Some(1.0),
        Span::Intermission => Some(1.0),
        Span::Revive => Some(0.5),
    },
    event: |w, e| log(w, format!("{e:?}")),
    options: FlowOptions::MISSIONS,
};

fn kit_game(game: &'static str) -> ModeGame {
    ModeGame {
        game,
        chosen: |w| w.get_resource::<Chosen>().map(|c| c.0),
        flow: FLOW,
    }
}

/// The kit's sim playing `key`, its runtime's systems in the rules' set.
fn sim(key: &'static str) -> Sim {
    let mut sim = kit_sim();
    sim.app
        .insert_resource(Chosen(key))
        .init_resource::<Log>()
        .init_resource::<Match>()
        .add_modes(kit_game("kit"), &[&DEATHMATCH, &ROUNDS, &WARMUP_ENDS, &FREE_FOR_ALL])
        .add_systems(
            FixedUpdate,
            flow::mission_update.in_set(SimSet::Rules),
        );
    sim
}

fn player(sim: &mut Sim, team: u8) -> Entity {
    let p = sim.spawn_character(greybox::SPAWNS[0], placeholder::ID);
    sim.app.world_mut().entity_mut(p).insert(Team(team));
    p
}

fn phase(sim: &Sim) -> Phase {
    sim.app.world().resource::<Match>().phase
}

fn now(sim: &Sim) -> f64 {
    sim.app.world().resource::<chimera_core::core::SimClock>().now
}

/// Tick until `want` holds (at most `secs`), the time it took.
fn until(sim: &mut Sim, secs: f64, want: impl Fn(Phase) -> bool) -> f64 {
    let start = now(sim);
    while !want(phase(sim)) {
        assert!(now(sim) - start <= secs, "still {:?} after {secs} s", phase(sim));
        sim.ticks(1);
    }
    now(sim) - start
}

fn log_of(sim: &mut Sim) -> Vec<String> {
    std::mem::take(&mut sim.app.world_mut().resource_mut::<Log>().0)
}

#[test]
fn a_match_walks_its_phases_in_order() {
    let mut sim = sim("rounds");
    sim.seconds(0.5);
    assert_eq!(phase(&sim), Phase::Waiting, "nobody in");
    let a = player(&mut sim, ALPHA);
    let _b = player(&mut sim, BRAVO);
    until(&mut sim, 0.2, |p| p == Phase::Starting);
    log_of(&mut sim);
    until(&mut sim, 0.2, |p| matches!(p, Phase::Setup { .. }));
    // The first mission start: the game's steps and the mode's hooks in
    // order, everyone respawned.
    assert_eq!(
        log_of(&mut sim),
        [
            "Prepare { first: true }",
            "ResetStats",
            "MissionStart",
            "mode before_spawns",
            "respawn",
            "respawn",
            "mode setup"
        ]
    );
    assert!(sim.app.world().resource::<FreezeTime>().0, "frozen in the setup");
    let m = sim.app.world().resource::<Match>().clone();
    assert!(m.warmup, "the match starts with its warm-up");
    // Live at the frame after the setup's end, unfrozen, the warm-up's
    // timer.
    let t = until(&mut sim, 1.2, |p| p == Phase::Live);
    assert!((1.0..=1.0 + FRAME + 0.02).contains(&t), "setup {t}");
    assert!(!sim.app.world().resource::<FreezeTime>().0);
    let ends = sim.app.world().resource::<Match>().round_ends.expect("a timer");
    assert!((ends - now(&sim) - 2.0).abs() < 1e-6);
    // The warm-up revives the dead.
    let since = now(&sim);
    sim.app.world_mut().entity_mut(a).insert(Dead { since });
    sim.seconds(0.7);
    assert!(sim.app.world().get::<Dead>(a).is_none(), "revived in the warm-up");
    // The warm-up's time-out: no winner, nothing counted.
    until(&mut sim, 2.0, |p| matches!(p, Phase::RoundOver { .. }));
    let m = sim.app.world().resource::<Match>().clone();
    assert_eq!((m.winner, m.round_wins, m.results.len()), (None, [0, 0], 1));
    until(&mut sim, 1.2, |p| matches!(p, Phase::Intermission { .. }));
    assert_eq!(sim.app.world().resource::<Match>().round, 0, "the warm-up isn't a round");
    log_of(&mut sim);
    // The real rounds: the time-out's winner (Alpha) takes each.
    until(&mut sim, 1.2, |p| matches!(p, Phase::Setup { .. }));
    assert_eq!(log_of(&mut sim)[0], "Prepare { first: false }", "not the first mission");
    until(&mut sim, 1.2, |p| p == Phase::Live);
    until(&mut sim, 5.2, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(sim.app.world().resource::<Match>().round_wins, [1, 0]);
    until(&mut sim, 1.2, |p| matches!(p, Phase::Intermission { .. }));
    assert_eq!(
        log_of(&mut sim),
        ["mode round_over", "RoundOver", "mode round_end", "RoundEnd", "RoundEnded", "mode after_round"]
    );
    until(&mut sim, 10.0, |p| p == Phase::MatchOver);
    let m = sim.app.world().resource::<Match>().clone();
    assert_eq!((m.result, m.round, m.round_wins), (Some(Some(ALPHA)), 2, [2, 0]));
}

/// Each step of a match reaches an observer (`ModeStep`) once, in order,
/// right after the game's own handler and before the step goes on (the
/// mode's next hook); the match's end (`MatchEnd`) once, after the
/// deciding round's `after_round`, with the result set and the
/// intermission on, in the tick the match was decided.
#[test]
fn an_observer_sees_each_step_of_a_match_once_in_order() {
    let mut sim = sim("rounds");
    sim.app.add_observer(|s: On<mode::ModeStep>, mut commands: Commands| {
        let e = s.0;
        commands.queue(move |w: &mut World| {
            let (phase, result) = {
                let m = w.resource::<Match>();
                (m.phase, m.result)
            };
            log(w, format!("step {e:?}"));
            if e == FlowEvent::MatchEnd {
                assert!(matches!(phase, Phase::Intermission { .. }), "{phase:?}");
                assert_eq!(result, Some(Some(ALPHA)));
                w.insert_resource(EndedAt(w.resource::<chimera_core::core::SimClock>().tick));
            }
        });
    });
    player(&mut sim, ALPHA);
    player(&mut sim, BRAVO);
    let mut decided = None;
    while phase(&sim) != Phase::MatchOver {
        assert!(now(&sim) < 30.0, "the match never ended");
        sim.ticks(1);
        if decided.is_none() && sim.app.world().resource::<Match>().result.is_some() {
            decided = Some(sim.app.world().resource::<chimera_core::core::SimClock>().tick);
        }
    }
    assert_eq!(sim.app.world().get_resource::<EndedAt>().map(|e| e.0), decided, "in the deciding tick");
    let log = log_of(&mut sim);
    // Every game step is followed at once by its event.
    for (i, line) in log.iter().enumerate() {
        if let Some(e) = line.strip_prefix("step ") {
            assert_eq!(log[i - 1], e, "right after the game's handler: {log:?}");
        }
    }
    let steps: Vec<&str> = log.iter().filter_map(|l| l.strip_prefix("step ")).collect();
    let round = ["MissionStart", "RoundOver", "RoundEnd", "RoundEnded"];
    let mut want = vec!["Prepare { first: true }", "ResetStats"];
    want.extend(round);
    for _ in 0..2 {
        want.push("Prepare { first: false }");
        want.extend(round);
    }
    want.push("MatchEnd");
    assert_eq!(steps, want, "the warm-up, two rounds, the match's end: {log:?}");
    let end = log.iter().position(|l| l == "step MatchEnd").unwrap();
    assert_eq!(log[end - 2], "mode after_round", "after the deciding round's after_round");
}

/// When the match's end was seen.
#[derive(Resource)]
struct EndedAt(u64);

#[test]
fn a_warm_ups_end_can_end_the_match_by_the_modes_hook_only() {
    // No hook (`NO_HOOKS`): the warm-up never ends the match (CS:S's).
    let mut plain = sim("rounds");
    player(&mut plain, ALPHA);
    player(&mut plain, BRAVO);
    until(&mut plain, 6.0, |p| matches!(p, Phase::Intermission { .. }));
    until(&mut plain, 3.0, |p| matches!(p, Phase::Setup { .. }));
    assert_eq!(plain.app.world().resource::<Match>().result, None);
    // The hook says the match is over: after the warm-up's round over and
    // intermission, the match is over, no real round played.
    let mut sim = sim("warmup_ends");
    sim.app.insert_resource(EndAfterWarmup(true));
    player(&mut sim, ALPHA);
    player(&mut sim, BRAVO);
    until(&mut sim, 6.0, |p| matches!(p, Phase::Intermission { .. }));
    assert_eq!(sim.app.world().resource::<Match>().result, Some(Some(ALPHA)));
    until(&mut sim, 1.2, |p| p == Phase::MatchOver);
    let m = sim.app.world().resource::<Match>().clone();
    assert_eq!((m.round, m.results.len(), m.results[0].warmup), (0, 1, true), "only the warm-up ran");
    // Not over: the real round follows.
    let mut sim = sim_with_flag_off();
    until(&mut sim, 6.0, |p| matches!(p, Phase::Intermission { .. }));
    until(&mut sim, 1.2, |p| matches!(p, Phase::Setup { .. }));
    assert_eq!(sim.app.world().resource::<Match>().result, None);
}

fn sim_with_flag_off() -> Sim {
    let mut sim = sim("warmup_ends");
    sim.app.insert_resource(EndAfterWarmup(false));
    player(&mut sim, ALPHA);
    player(&mut sim, BRAVO);
    sim
}

#[test]
fn a_counted_win_ends_the_round_at_the_next_frame() {
    let mut sim = sim("dm");
    let a = player(&mut sim, ALPHA);
    let b = player(&mut sim, BRAVO);
    until(&mut sim, 1.0, |p| p == Phase::Live);
    assert_eq!(sim.app.world().resource::<Match>().round_ends, None, "no timer");
    // A game counts deaths through the playing mode's rule.
    let count = |sim: &mut Sim| {
        let world = sim.app.world_mut();
        let mode = mode::current_mode(world).expect("a mode plays");
        let death = Death {
            victim: b,
            victim_team: BRAVO,
            killer: Some(a),
            kind: DeathKind::Enemy,
        };
        (mode.on_death)(world, &death);
    };
    for _ in 0..3 {
        count(&mut sim);
    }
    let m = sim.app.world().resource::<Match>().clone();
    assert!(m.decided() && m.phase == Phase::Live, "decided, still live until the frame");
    let t = until(&mut sim, 0.2, |p| matches!(p, Phase::RoundOver { .. }));
    assert!(t <= FRAME + 0.02);
    until(&mut sim, 3.0, |p| p == Phase::MatchOver);
    assert_eq!(sim.app.world().resource::<Match>().result, Some(Some(ALPHA)));
}

#[test]
fn free_for_all_players_get_teams_of_their_own() {
    let mut sim = sim("ffa");
    let a = player(&mut sim, ALPHA);
    let b = player(&mut sim, ALPHA);
    until(&mut sim, 1.0, |p| matches!(p, Phase::Setup { .. }));
    let team = |sim: &Sim, e| sim.app.world().get::<Team>(e).unwrap().0;
    let mut teams = [team(&sim, a), team(&sim, b)];
    teams.sort();
    assert_eq!(teams, [FFA_FIRST, FFA_FIRST + 1], "the lowest free ids");
}

#[test]
fn the_gameplay_picks_whose_modes_play() {
    let mut sim = sim("dm");
    sim.app.add_modes(kit_game("other"), &[]);
    assert_eq!(mode::current_mode(sim.app.world()).map(|m| m.id), Some("kit:deathmatch"), "the first game");
    let mut other = Gameplay {
        rules: "other",
        bots: "other",
        hud: "other",
        binds: "other",
        tick: std::time::Duration::from_millis(15),
    };
    sim.app.world_mut().insert_resource(other.clone());
    assert_eq!(mode::current_mode(sim.app.world()).map(|m| m.id), None, "the other game has no modes");
    other.rules = "kit";
    sim.app.world_mut().insert_resource(other);
    assert_eq!(mode::current_mode(sim.app.world()).map(|m| m.id), Some("kit:deathmatch"));
    // The registry lists them by id.
    let modes = sim.app.world().resource::<mode::Modes>();
    assert_eq!(modes.get("kit:ffa").map(|m| m.teams), Some(Teams::FreeForAll));
}

// --- Rounds back to back (`FlowOptions` the other way: CS:S's rounds)

/// The round's decision the `BACK_TO_BACK` mode makes at its next live
/// step (None: undecided; Some(0): no winner).
#[derive(Resource, Default)]
struct Decision(Option<u8>);

/// Decided each live step by `Decision`, never by the timer; 2 round
/// wins take the match; teams as joined.
static BACK_TO_BACK: Mode = Mode {
    id: "ticked:rounds",
    key: "rounds",
    shown_as: "Rounds back to back",
    description: "decided each tick, 2 wins take the match",
    respawns: false,
    warmup: true,
    warmup_revives: false,
    forced_setup: None,
    teams: Teams::AsJoined,
    on_death: |_, _| {},
    time_out: |w| w.resource_mut::<Match>().winner = Some(ALPHA),
    match_end: |w| {
        let m = w.resource::<Match>();
        m.round_wins.iter().any(|r| *r >= 2).then_some(Some(ALPHA))
    },
    hooks: mode::Hooks {
        setup: |w| log(w, "mode setup"),
        round_over: |w| log(w, "mode round_over"),
        round_end: |w| log(w, "mode round_end"),
        after_round: |w| log(w, "mode after_round"),
        decide: |w, time_up| {
            if time_up {
                log(w, "time up");
            }
            match w.resource_mut::<Decision>().0.take() {
                Some(0) => w.resource_mut::<Match>().end_without_winner(),
                Some(team) => w.resource_mut::<Match>().win(team),
                None => {}
            }
        },
        ..NO_HOOKS
    },
};

const TICKED: Flow = Flow {
    frame: 0.0,
    // No warm-up length: the mode's warm-up never plays.
    length: |_, span| match span {
        Span::Setup => Some(0.5),
        Span::Round => Some(1.0),
        Span::RoundOver => Some(0.5),
        Span::Warmup | Span::Intermission | Span::Revive => None,
    },
    options: FlowOptions {
        slack: 0.0,
        wait_for_players: false,
        timer_ends_round: false,
        direct: true,
        count_at_start: true,
    },
    ..FLOW
};

/// A sim whose own rules' system steps the `ticked` game's match
/// (`flow::step_game`), while the gameplay names another game.
fn ticked_sim() -> Sim {
    let mut sim = kit_sim();
    sim.app
        .insert_resource(Chosen("rounds"))
        .init_resource::<Log>()
        .init_resource::<Decision>()
        .init_resource::<Match>()
        .add_modes(kit_game("kit"), &[&DEATHMATCH])
        .add_modes(
            ModeGame {
                game: "ticked",
                chosen: |w| w.get_resource::<Chosen>().map(|c| c.0),
                flow: TICKED,
            },
            &[&BACK_TO_BACK],
        )
        .add_systems(
            FixedUpdate,
            (|w: &mut World| {
                let now = w.resource::<chimera_core::core::SimClock>().now;
                assert!(flow::step_game(w, "ticked", now), "a mode chosen");
            })
            .in_set(SimSet::Rules),
        );
    sim
}

fn tick(sim: &Sim) -> f64 {
    sim.app.world().resource::<chimera_core::core::SimClock>().delta.as_secs_f64()
}

#[test]
fn rounds_back_to_back_start_at_once_and_are_numbered_as_they_start() {
    let mut sim = ticked_sim();
    assert_eq!(mode::current_mode(sim.app.world()).map(|m| m.id), None, "the first game has no `rounds`");
    sim.ticks(1);
    // Nobody in, and the match has started: its first round's setup.
    let m = sim.app.world().resource::<Match>().clone();
    assert!(matches!(m.phase, Phase::Setup { .. }), "{:?}", m.phase);
    assert_eq!((m.round, m.warmup), (1, false), "the first round, numbered at its start; no warm-up length");
    assert_eq!(log_of(&mut sim), ["Prepare { first: true }", "ResetStats", "MissionStart", "mode setup"]);
    // Live at the first tick at or after the setup's end, exactly.
    let Phase::Setup { until: live_at } = m.phase else { unreachable!() };
    until(&mut sim, 1.0, |p| p == Phase::Live);
    assert!(now(&sim) >= live_at && now(&sim) < live_at + tick(&sim), "live at {} for {live_at}", now(&sim));
    let m = sim.app.world().resource::<Match>().clone();
    assert_eq!(m.live_since, now(&sim), "when it went live");
    let ends = m.round_ends.expect("a timer");
    // The timer at 0 ends nothing: it tells the mode.
    sim.seconds(1.5);
    assert!(now(&sim) > ends);
    assert_eq!(phase(&sim), Phase::Live, "the timer ends nothing");
    let told = log_of(&mut sim);
    assert!(!told.is_empty() && told.iter().all(|l| l == "time up"), "the mode is told: {told:?}");
}

#[test]
fn a_decided_round_is_over_at_once_and_the_next_follows_without_an_intermission() {
    let mut sim = ticked_sim();
    until(&mut sim, 1.0, |p| p == Phase::Live);
    log_of(&mut sim);
    // Decided at a live step: over in the same step.
    sim.app.world_mut().resource_mut::<Decision>().0 = Some(BRAVO);
    sim.ticks(1);
    let m = sim.app.world().resource::<Match>().clone();
    let Phase::RoundOver { until: over } = m.phase else { panic!("{:?}", m.phase) };
    assert!((over - now(&sim) - 0.5).abs() < 1e-9, "round over from this step");
    assert_eq!((m.winner, m.round_wins, m.results.last().map(|r| r.round)), (Some(BRAVO), [0, 1], Some(1)));
    assert_eq!(log_of(&mut sim), ["mode round_over", "RoundOver"]);
    // Round over's end: the round's end and the next round's setup in one
    // step, no intermission.
    let mut seen = Vec::new();
    while matches!(phase(&sim), Phase::RoundOver { .. }) {
        sim.ticks(1);
        seen.push(phase(&sim));
    }
    assert!(matches!(seen.last(), Some(Phase::Setup { .. })), "{seen:?}");
    assert!(seen.iter().all(|p| !matches!(p, Phase::Intermission { .. })));
    assert!(now(&sim) >= over && now(&sim) < over + tick(&sim), "exactly when due");
    assert_eq!(
        log_of(&mut sim),
        [
            "mode round_end",
            "RoundEnd",
            "RoundEnded",
            "mode after_round",
            "Prepare { first: false }",
            "MissionStart",
            "mode setup"
        ]
    );
    assert_eq!(sim.app.world().resource::<Match>().round, 2, "the second round started");
    // A round decided without a winner is a draw, not the time-out's.
    until(&mut sim, 1.0, |p| p == Phase::Live);
    sim.app.world_mut().resource_mut::<Decision>().0 = Some(0);
    sim.ticks(1);
    let m = sim.app.world().resource::<Match>().clone();
    assert_eq!((m.winner, m.round_wins), (None, [0, 1]), "a draw");
    // Two Bravo wins: the match is over as that round's pause runs out.
    until(&mut sim, 1.0, |p| p == Phase::Live);
    sim.app.world_mut().resource_mut::<Decision>().0 = Some(BRAVO);
    until(&mut sim, 1.0, |p| p == Phase::MatchOver);
    let m = sim.app.world().resource::<Match>().clone();
    assert_eq!((m.result, m.round, m.round_wins), (Some(Some(ALPHA)), 3, [0, 2]));
}

/// A mode's own per-tick systems (`mode::mode_is`): each runs only while
/// its mode plays, where its game chains it (here before the phases'
/// step: a tick's update comes before the step it may decide), and
/// switching the mode switches which one runs.
#[test]
fn a_modes_tick_runs_only_while_it_plays_before_the_step() {
    let mut sim = kit_sim();
    sim.app
        .insert_resource(Chosen("rounds"))
        .init_resource::<Log>()
        .init_resource::<Match>()
        .add_modes(kit_game("kit"), &[&DEATHMATCH, &ROUNDS])
        .add_systems(
            FixedUpdate,
            (
                (|w: &mut World| log(w, "tick deathmatch")).run_if(mode::mode_is(DEATHMATCH.id)),
                (|w: &mut World| log(w, "tick rounds")).run_if(mode::mode_is(ROUNDS.id)),
                (|w: &mut World| log(w, "tick either")).run_if(mode::mode_in(&["kit:deathmatch", "kit:rounds"])),
                flow::mission_update,
            )
                .chain()
                .in_set(SimSet::Rules),
        );
    let _a = player(&mut sim, ALPHA);
    let _b = player(&mut sim, BRAVO);
    until(&mut sim, 0.2, |p| p == Phase::Starting);
    log_of(&mut sim);
    until(&mut sim, 0.2, |p| matches!(p, Phase::Setup { .. }));
    let log = log_of(&mut sim);
    // The mission start's tick: the mode's own systems, then the step.
    let start = log.iter().position(|l| l == "Prepare { first: true }").expect("a mission start");
    assert_eq!(log[start - 2..start], ["tick rounds", "tick either"], "{log:?}");
    assert!(!log.iter().any(|l| l == "tick deathmatch"), "{log:?}");
    sim.app.insert_resource(Chosen("dm"));
    sim.ticks(1);
    assert_eq!(log_of(&mut sim)[..2], ["tick deathmatch", "tick either"]);
    sim.app.insert_resource(Chosen("none"));
    sim.ticks(1);
    assert!(log_of(&mut sim).is_empty(), "no mode: none runs");
}
