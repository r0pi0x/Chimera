//! The game's end and restarts (specs/hl2dm/rules.md "Frag and time
//! limits", "Intermission", "Map change", "Restart", "Ready-up"):
//!
//! - `decide` (the runtime's hook, every live tick, time first): the time
//!   limit (whole minutes, strictly past) and the frag limit (DM: any
//!   player's frags; TDM: a team's score) end the round, which starts the
//!   intermission (`begin_intermission`): everyone frozen where they look,
//!   the dead staying dead, `mp_chattime` (whole seconds, 1-120); then the
//!   next map (`change_map`: `nextlevel`, else the map cycle's next, the
//!   first when the map isn't in it).
//! - `checks`, once a second: `mp_restartgame N` (the countdown, at most
//!   60 s), `mp_readyrestart 1` (wait for every player to say
//!   `mp_ready_signal`, then restart 5 s later); the restart (`restart`).
//! - `timeleft`.
//! - Chat's log lines (`log_says`, "Server log lines"): `say`, and
//!   `say_team` only in TDM (measured: a DM player's writes none).

use bevy::prelude::*;
use chimera_core::game_log::{EventValue, GameEvent, Said};

use super::{
    DEFAULT_CYCLE, Hl2Match, Hl2Player, Hl2Rules, READY_RESTART_DELAY, RESPAWN_MIN_DEAD, RESTART_MAX, RULES_PERIOD, SPECTATOR,
};
use crate::{
    console::{Console, ConsoleAppExt},
    core::{Dead, FreezeTime, Intent, MapCycle, NetRole, Score, SimClock},
};
use chimera_sim::mode::Match;

/// The runtime's `decide`: the time limit, then the frag limit, end the
/// round (the intermission).
pub fn decide(world: &mut World, _time_up: bool) {
    let now = world.resource::<SimClock>().now;
    let rules = world.resource::<Hl2Rules>().clone();
    let m = world.resource::<Hl2Match>().clone();
    if m.over {
        return;
    }
    let minutes = rules.timelimit.max(0.0).trunc() as f64;
    let time = minutes > 0.0 && m.game_start + 60.0 * minutes - now < -1e-9;
    let frags = rules.fraglimit != 0.0 && {
        let limit = rules.fraglimit as f64;
        if m.teamplay {
            let score = world.resource::<Match>().score;
            score.iter().any(|s| *s as f64 >= limit)
        } else {
            world
                .query_filtered::<&Score, With<Hl2Player>>()
                .iter(world)
                .any(|s| s.kills as f64 >= limit)
        }
    };
    if time || frags {
        world.resource_mut::<Match>().end_without_winner();
    }
}

/// The round is over (the runtime's `round_over`): the game is over and
/// the intermission runs to `mp_chattime` from now.
pub fn begin_intermission(world: &mut World) {
    let now = world.resource::<SimClock>().now;
    let chat = world.get_resource::<Hl2Rules>().map_or(10.0, |r| r.chattime.trunc().clamp(1.0, 120.0)) as f64;
    {
        let mut m = world.resource_mut::<Hl2Match>();
        m.over = true;
        m.intermission_end = Some(now + chat);
    }
    world.insert_resource(FreezeTime(true));
    // Everyone's look held where it was.
    let all: Vec<(Entity, f32, f32)> = world
        .query_filtered::<(Entity, &Intent), With<Hl2Player>>()
        .iter(world)
        .map(|(e, i)| (e, i.yaw, i.pitch))
        .collect();
    for (e, yaw, pitch) in all {
        world.get_mut::<Hl2Player>(e).unwrap().frozen_look = Some((yaw, pitch));
    }
    crate::rules::log_text(world, "World triggered \"Round_End\"".into());
}

/// The map after this one: `nextlevel`, else the cycle's next (the first
/// when this one isn't in it; this one again without a cycle).
pub fn next_map(world: &World) -> Option<String> {
    let rules = world.resource::<Hl2Rules>();
    if !rules.nextlevel.trim().is_empty() {
        return Some(rules.nextlevel.trim().to_string());
    }
    let cycle = world.get_resource::<MapCycle>().cloned().unwrap_or_default();
    let cycle = if cycle.maps.is_empty() {
        MapCycle {
            maps: DEFAULT_CYCLE.iter().map(|m| m.to_string()).collect(),
            current: cycle.current,
        }
    } else {
        cycle
    };
    cycle.next().or_else(|| Some(cycle.current.clone()).filter(|c| !c.is_empty()))
}

/// The intermission is over (the runtime's match end): on to the next
/// map (`changelevel` on a server, `map` otherwise); the game stays over
/// until it loads (`super::new_game`).
pub fn change_map(world: &mut World) {
    let next = next_map(world);
    world.resource_mut::<Hl2Match>().next_map = next.clone();
    let Some(map) = next else { return };
    let server = world.get_resource::<NetRole>() == Some(&NetRole::Server);
    let verb = if server { "changelevel" } else { "map" };
    if let Some(mut console) = world.get_resource_mut::<Console>() {
        console.submit(format!("{verb} hl2dm:{map}"));
    }
}

/// Frozen in the intermission: no movement, buttons or impulses, the look
/// where it was.
pub(super) fn hold_frozen(m: Res<Hl2Match>, mut players: Query<(&Hl2Player, &mut Intent)>) {
    if !m.over {
        return;
    }
    for (p, mut i) in &mut players {
        let (yaw, pitch) = p.frozen_look.unwrap_or((i.yaw, i.pitch));
        let held = Intent {
            yaw,
            pitch,
            command: i.command,
            ..default()
        };
        *i = held;
    }
}

/// The once-a-second checks (not in the intermission): all ready, then
/// `mp_restartgame` and `mp_readyrestart`; a restart that is due.
pub(super) fn checks(
    mut m: ResMut<Hl2Match>,
    mut rules: ResMut<Hl2Rules>,
    players: Query<&Hl2Player>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    let now = clock.now;
    if m.over {
        return;
    }
    if now > m.next_check {
        m.next_check = now + RULES_PERIOD;
        if m.waiting_ready && players.iter().all(|p| p.ready) {
            let line = format!("All players ready. Game will restart in {} seconds", READY_RESTART_DELAY as i64);
            commands.queue(move |w: &mut World| tell_all(w, line));
            m.waiting_ready = false;
            m.restart_at = Some(now + READY_RESTART_DELAY);
        }
        if rules.restartgame > 0.0 {
            let n = (rules.restartgame.trunc() as f64).min(RESTART_MAX);
            let line = format!("Game will restart in {n} {}", if n == 1.0 { "SECOND" } else { "SECONDS" });
            commands.queue(move |w: &mut World| tell_all(w, line));
            m.restart_at = Some(now + n);
            rules.restartgame = 0.0;
        }
        if rules.readyrestart != 0 {
            rules.readyrestart = 0;
            m.waiting_ready = true;
            m.restart_at = None;
        }
    }
    if m.restart_at.is_some_and(|at| now + 1e-9 >= at) {
        m.restart_at = None;
        commands.queue(restart);
    }
}

/// A line to everyone (the console of whoever plays here).
fn tell_all(world: &mut World, text: String) {
    if let Some(mut c) = world.get_resource_mut::<Console>() {
        c.info(text);
    }
}

/// `mp_restartgame`'s restart: the time limit from now, the map's
/// entities again (its pickups, weapons, breakables: `core::
/// restart_round`), everyone respawned but the dead of 3 s or less (who
/// lose their weapons and wait), frags, deaths and team scores 0,
/// `round_start`.
pub fn restart(world: &mut World) {
    let (now, tick, dt) = {
        let c = world.resource::<SimClock>();
        (c.now, c.tick, c.delta.as_secs_f64())
    };
    {
        let mut m = world.resource_mut::<Hl2Match>();
        m.game_start = now;
        m.restart_at = None;
        m.intermission_end = None;
    }
    crate::core::restart_round(world);
    let all: Vec<(Entity, Option<u64>, bool)> = world
        .query::<(Entity, &Hl2Player, Has<Dead>)>()
        .iter(world)
        .filter(|(_, p, _)| p.team != SPECTATOR)
        .map(|(e, p, dead)| (e, p.death_tick, dead))
        .collect();
    for (e, died, dead) in all {
        let recent = dead && died.is_some_and(|d| (tick - d) as f64 * dt <= RESPAWN_MIN_DEAD + 1e-9);
        if recent {
            chimera_sim::weapon::equip::strip(world, e);
        } else {
            super::spawns::spawn(world, e);
        }
        world.entity_mut(e).insert(Score::default());
    }
    world.resource_mut::<Match>().score = [0; 2];
    crate::rules::event(
        world,
        GameEvent::new("round_start")
            .with("timelimit", EventValue::Int(0))
            .with("fraglimit", EventValue::Int(0))
            .with("objective", EventValue::Str("DEATHMATCH".into())),
    );
}

/// A chat line said (`player_say`): the ready signal marks its speaker.
pub fn said(world: &mut World, who: Entity, text: &str) {
    let waiting = world.resource::<Hl2Match>().waiting_ready;
    let signal = world.resource::<Hl2Rules>().ready_signal.clone();
    if waiting
        && text == signal
        && let Some(mut p) = world.get_mut::<Hl2Player>(who)
    {
        p.ready = true;
    }
}

/// Chat lines (`player_say` events) for the ready-up.
pub(super) fn hear_says(mut events: MessageReader<GameEvent>, mut commands: Commands) {
    for e in events.read() {
        if e.name != "player_say" {
            continue;
        }
        let (Some(who), Some(EventValue::Str(text))) = (e.player("userid"), e.get("text")) else { continue };
        let text = text.clone();
        commands.queue(move |w: &mut World| said(w, who, &text));
    }
}

/// Chat lines' log lines (rules.md "Server log lines"; measured
/// 2026-11-04, probe round 4): `"<player>" say "text"`, and `say_team`
/// only in TDM (a DM player's `say_team` reaches only itself and writes no
/// line).
pub(super) fn log_says(mut said: MessageReader<Said>, mut commands: Commands) {
    for s in said.read() {
        let s = s.clone();
        commands.queue(move |w: &mut World| {
            if s.team_only && !w.resource::<Hl2Match>().teamplay {
                return;
            }
            let who = super::deaths::subject(w, s.from);
            let verb = if s.team_only { "say_team" } else { "say" };
            crate::rules::log_text(w, format!("{who} {verb} \"{}\"", s.text));
        });
    }
}

/// Minutes and whole seconds left (None: no limit, or the last second).
pub fn time_left(world: &World) -> Option<(i64, i64)> {
    let minutes = world.resource::<Hl2Rules>().timelimit.max(0.0).trunc() as f64;
    if minutes <= 0.0 {
        return None;
    }
    let now = world.resource::<SimClock>().now;
    let left = (world.resource::<Hl2Match>().game_start + 60.0 * minutes - now).floor().max(0.0) as i64;
    (left > 0).then_some((left / 60, left % 60))
}

pub(super) fn commands(app: &mut App) {
    app.console_command("hl2dm:timeleft", "Time left on this map (mp_timelimit).", |w, _| {
        Ok(Some(match time_left(w) {
            Some((m, s)) => format!("Time remaining on map: {m}:{s:02}"),
            None => "* No Time Limit *".into(),
        }))
    });
    app.add_systems(
        FixedUpdate,
        (hear_says, log_says)
            .run_if(super::rules_on)
            .run_if(crate::core::authoritative)
            .in_set(crate::core::SimSet::Rules),
    );
}
