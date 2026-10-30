//! The match's phases as the runtime steps them (lifted from Combat Arms'
//! mission update, specs/combat_arms/game_modes.md S3, S9): once per the
//! game's frame (`Flow::frame`: the first tick at or after each frame's
//! time), at most one phase on, except where a phase starts the next
//! directly. The game's work at each step is its `Flow::event`
//! (`FlowEvent`), the mode's its `Hooks`; how the phases step, its
//! `FlowOptions` (below, Combat Arms' `MISSIONS` first).
//!
//! - Waiting → Starting (the first player in) → mission start the next
//!   frame (`Prepare`; the match's counters reset on the first,
//!   `ResetStats`), which sets the round up at once (`MissionStart`):
//!   everyone's team made (`assign_team`), the mode's `before_spawns`,
//!   everyone respawned (`Flow::respawn`, the living too), the mode's
//!   `setup`, everyone frozen (`core::FreezeTime`) until the setup's end
//!   (`Mode::forced_setup`, else `Span::Setup`).
//! - Live at the next frame after the setup's end: unfrozen, the winner
//!   cleared, the round timer started (`Span::Round`, the warm-up's
//!   `Span::Warmup`; none: no timer).
//! - Round over at the first frame that sees a requested end, the timer
//!   at 0 or a hold's end: the mode's time-out rule picks a winner if none
//!   was set (not in a warm-up or a void round), a free-for-all winner's
//!   round counted (`RoundWonBy`), the side's round wins +1 (not in the
//!   warm-up), the result recorded, the mode's `round_over`, `RoundOver`;
//!   `Span::RoundOver` on.
//! - Round end at the next frame after that: the mode's `round_end`,
//!   `RoundEnd`, unfrozen, the round's counters cleared, the round counted
//!   (not the warm-up), `RoundEnded`, the mode's `match_end` (after
//!   the warm-up its `Hooks::warmup_end`, never ends by default), the
//!   intermission (`Span::Intermission`), the mode's
//!   `after_round`, `MatchEnd` when the match has its result; then the
//!   next mission start or the match over.
//! - The warm-up (`Mode::warmup`, when the game has a `Span::Warmup`):
//!   the match's first round, nothing counts, no winner; the dead revived
//!   every `Span::Revive` in the modes that do (`Mode::warmup_revives`).
//!
//! The options (`FlowOptions`, CS:S's rounds the other way from Combat
//! Arms' on each): the phases' end times compared with a slack (`now +
//! slack >= until`; 0: exactly); the first step starting the match
//! without waiting for a player (no `Starting`: the mission start in the
//! same step); the round timer only telling the mode's `decide` hook
//! (every live step, before the end test: a round it decides is over in
//! the same step) instead of ending the round; round over's end going
//! straight into the next mission start in the same step (`direct`: the
//! round end's steps, `match_end`, `after_round`, no intermission); the
//! round number counted as rounds start (`count_at_start`: at the
//! mission start, before `MissionStart`).
//!
//! Each `FlowEvent` goes to the game's `Flow::event`, then is triggered
//! as `mode::ModeStep` (`world.trigger`: its observers run there, inside
//! the step, and what they queue is applied before the step goes on).
//!
//! The system a game chains into its rules: `mission_update` (the mode's
//! own per-tick systems before it, its touches after the tick's deaths:
//! `mode::mode_is`); a game that does its own work around the step calls
//! `step_game` from its rules' system instead.

use bevy::prelude::*;

use super::{Flow, FlowEvent, Mode, ModeStep, Span, assign_team_in, current, current_of, state::*};
use crate::core::{Dead, FreezeTime, SimClock};

/// The phases' frame counter (one step per `Flow::frame`).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MissionFrames(pub u64);

/// The phases' step, once per the game's frame (the server's, with a
/// mode).
pub fn mission_update(world: &mut World) {
    let Some((mode, flow)) = current(world) else { return };
    let now = world.resource::<SimClock>().now;
    if due(world, now, flow) {
        step(world, now, mode, flow);
    }
}

/// One step of `game`'s chosen mode at `now`, once per its frame, from a
/// game's own rules' system (whichever game the gameplay names: the
/// caller gates it). False: `game` has no mode chosen (nothing stepped).
pub fn step_game(world: &mut World, game: &str, now: f64) -> bool {
    let Some((mode, flow)) = current_of(world, game) else { return false };
    if due(world, now, flow) {
        step(world, now, mode, flow);
    }
    true
}

/// Whether a frame is due at `now` (every tick with no frame).
fn due(world: &mut World, now: f64, flow: Flow) -> bool {
    if flow.frame <= 0.0 {
        return true;
    }
    let due = ((now + 1e-9) / flow.frame).floor() as u64;
    let mut frames = world.get_resource_or_init::<MissionFrames>();
    if frames.0 >= due {
        return false;
    }
    frames.0 = due;
    true
}

/// One step: at most one phase on, except where a phase starts the next
/// directly.
fn step(world: &mut World, now: f64, mode: &'static Mode, flow: Flow) {
    let at = now + flow.options.slack;
    let phase = world.resource::<Match>().phase;
    match phase {
        Phase::Waiting => {
            if !flow.options.wait_for_players {
                mission_start(world, now, true, mode, flow);
            } else if !(flow.players)(world).is_empty() {
                world.resource_mut::<Match>().phase = Phase::Starting;
            }
        }
        Phase::Starting => mission_start(world, now, true, mode, flow),
        Phase::Setup { until } => {
            if at >= until {
                go_live(world, now, flow);
            }
        }
        Phase::Live => {
            let (hold, requested, ends) = {
                let m = world.resource::<Match>();
                (m.hold_until, m.end_requested, m.round_ends)
            };
            if hold.is_none() && !requested {
                (mode.hooks.decide)(world, ends.is_some_and(|e| at >= e));
            }
            let (requested, ends, warmup, hold, next_revive) = {
                let m = world.resource::<Match>();
                (m.end_requested, m.round_ends, m.warmup, m.hold_until, m.next_revive)
            };
            let time_up = flow.options.timer_ends_round && ends.is_some_and(|e| at >= e);
            if let Some(h) = hold {
                if at >= h {
                    round_over(world, now, mode, flow);
                }
            } else if requested || time_up {
                round_over(world, now, mode, flow);
            } else if warmup && mode.warmup_revives && at >= next_revive {
                let period = (flow.length)(world, Span::Revive).unwrap_or(0.0);
                world.resource_mut::<Match>().next_revive = now + period;
                revive_the_dead(world, flow);
            }
        }
        Phase::RoundOver { until } => {
            if at >= until {
                round_end(world, now, mode, flow);
            }
        }
        Phase::Intermission { until } => {
            if at >= until {
                if world.resource::<Match>().result.is_some() {
                    world.resource_mut::<Match>().phase = Phase::MatchOver;
                } else {
                    mission_start(world, now, false, mode, flow);
                }
            }
        }
        Phase::MatchOver => {}
    }
}

/// A step's event: the game's handler (`Flow::event`), then `ModeStep`
/// triggered for its observers, whose queued commands apply here, before
/// the step goes on.
fn emit(world: &mut World, flow: Flow, event: FlowEvent) {
    (flow.event)(world, event);
    world.trigger(ModeStep(event));
    world.flush();
}

/// The freeze on or off (`core::FreezeTime`, put in when missing).
fn freeze(world: &mut World, on: bool) {
    match world.get_resource_mut::<FreezeTime>() {
        Some(mut f) => f.0 = on,
        None => world.insert_resource(FreezeTime(on)),
    }
}

/// A mission start: `first`, the match's counters reset; then the round's
/// setup.
fn mission_start(world: &mut World, now: f64, first: bool, mode: &'static Mode, flow: Flow) {
    emit(world, flow, FlowEvent::Prepare { first });
    let has_warmup = (flow.length)(world, Span::Warmup).is_some();
    {
        let mut m = world.resource_mut::<Match>();
        if first {
            let warmup = mode.warmup && has_warmup;
            *m = Match {
                warmup,
                ..default()
            };
        } else if std::mem::take(&mut m.warmup_next) {
            m.warmup = has_warmup;
        }
        m.hold_until = None;
        m.immune = false;
        m.round_kills = [0; 2];
        m.round_deaths = [0; 2];
        m.winner = None;
        m.end_requested = false;
        m.void = false;
        m.round_ends = None;
        if flow.options.count_at_start && !m.warmup {
            m.round += 1;
        }
    }
    if first {
        emit(world, flow, FlowEvent::ResetStats);
    }
    emit(world, flow, FlowEvent::MissionStart);
    // Everyone in the match respawned, on the team the mode gives.
    if mode.teams != super::Teams::AsJoined {
        for (e, _) in (flow.players)(world) {
            assign_team_in(world, e, true, mode, flow);
        }
    }
    (mode.hooks.before_spawns)(world);
    for (e, _) in (flow.players)(world) {
        (flow.respawn)(world, e);
    }
    (mode.hooks.setup)(world);
    let setup = mode
        .forced_setup
        .map(f64::from)
        .unwrap_or_else(|| (flow.length)(world, Span::Setup).unwrap_or(0.0));
    world.resource_mut::<Match>().phase = Phase::Setup { until: now + setup };
    freeze(world, true);
}

/// Live.
fn go_live(world: &mut World, now: f64, flow: Flow) {
    let warmup = world.resource::<Match>().warmup;
    let length = (flow.length)(world, if warmup { Span::Warmup } else { Span::Round });
    let revive = (flow.length)(world, Span::Revive).unwrap_or(0.0);
    freeze(world, false);
    let mut m = world.resource_mut::<Match>();
    m.phase = Phase::Live;
    m.winner = None;
    m.end_requested = false;
    m.round_ends = length.map(|l| now + l);
    m.live_since = now;
    m.next_revive = now + revive;
}

/// Round over.
fn round_over(world: &mut World, now: f64, mode: &'static Mode, flow: Flow) {
    let (warmup, void) = {
        let m = world.resource::<Match>();
        (m.warmup, m.void)
    };
    if warmup || void {
        world.resource_mut::<Match>().winner = None;
    } else if world.resource::<Match>().winner.is_none() {
        (mode.time_out)(world);
    }
    // A free-for-all round's winner is a player (its own team id).
    if !warmup
        && let Some(w) = world.resource::<Match>().winner.filter(|w| side(*w).is_none())
        && let Some((p, _)) = (flow.players)(world).into_iter().find(|(_, t)| *t == w)
    {
        emit(world, flow, FlowEvent::RoundWonBy(p));
    }
    let over = (flow.length)(world, Span::RoundOver).unwrap_or(0.0);
    let mut m = world.resource_mut::<Match>();
    if !warmup && let Some(i) = m.winner.and_then(side) {
        m.round_wins[i] += 1;
    }
    let result = RoundResult {
        round: m.round,
        warmup,
        winner: m.winner,
        score: m.score,
    };
    m.results.push(result);
    m.phase = Phase::RoundOver { until: now + over };
    m.round_ends = None;
    m.hold_until = None;
    (mode.hooks.round_over)(world);
    emit(world, flow, FlowEvent::RoundOver);
}

/// Round end, then the intermission (`direct`: the next mission start or
/// the match over, now).
fn round_end(world: &mut World, now: f64, mode: &'static Mode, flow: Flow) {
    (mode.hooks.round_end)(world);
    emit(world, flow, FlowEvent::RoundEnd);
    freeze(world, false);
    world.resource_mut::<Match>().immune = false;
    let warmup = {
        let mut m = world.resource_mut::<Match>();
        m.round_kills = [0; 2];
        m.round_deaths = [0; 2];
        let warmup = m.warmup;
        if !warmup && !flow.options.count_at_start {
            m.round += 1;
        }
        m.warmup = false;
        warmup
    };
    emit(world, flow, FlowEvent::RoundEnded);
    let result = if warmup { (mode.hooks.warmup_end)(world) } else { (mode.match_end)(world) };
    if let Some(result) = result {
        world.resource_mut::<Match>().result = Some(result);
    }
    if flow.options.direct {
        (mode.hooks.after_round)(world);
        if world.resource::<Match>().result.is_some() {
            world.resource_mut::<Match>().phase = Phase::MatchOver;
            emit(world, flow, FlowEvent::MatchEnd);
        } else {
            mission_start(world, now, false, mode, flow);
        }
        return;
    }
    let intermission = (flow.length)(world, Span::Intermission).unwrap_or(0.0);
    world.resource_mut::<Match>().phase = Phase::Intermission { until: now + intermission };
    (mode.hooks.after_round)(world);
    if world.resource::<Match>().result.is_some() {
        emit(world, flow, FlowEvent::MatchEnd);
    }
}

/// The warm-up's revive: every dead player in the match respawned.
fn revive_the_dead(world: &mut World, flow: Flow) {
    let dead: Vec<Entity> = world.query_filtered::<Entity, With<Dead>>().iter(world).collect();
    for (e, _) in (flow.players)(world) {
        if dead.contains(&e) {
            (flow.respawn)(world, e);
        }
    }
}
