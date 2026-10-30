//! The match's phases (specs/combat_arms/game_modes.md S3, S9, S10, S11):
//! the mission update the server runs once per server frame
//! (`grenades::SERVER_FRAME`, 20 Hz: the first tick at or after each
//! frame's time), joining, leaving, and a fresh match. The phases are
//! stepped by Chimera's mode runtime (`chimera_sim::mode::flow`, lifted
//! from here); Combat Arms' side of it is `FLOW` (the frame, the players
//! in join order, the respawn, the lengths below, and the work at each
//! step: the mission resolved, statistics reset, death drops, the map's
//! messages, the clean-up, the team counts).
//!
//! - Waiting → Starting (the first player in a team) → mission start the
//!   next frame (the counters reset), which sets the round up at once:
//!   everyone on Alpha and Bravo respawned (S5, the living too), held
//!   players released, the living frozen (`core::FreezeTime`: movement
//!   keys held, `frozen_intent`) until `object_start` s, or 0 s (one
//!   frame) in the modes that force it (`missions::setup_length`).
//! - Live at the next frame after the setup's end: unfrozen, the winner
//!   cleared, the round timer started from the mission's `TimeLimit` and
//!   `TrainingTimeLimit` by the mode's rule (missions.md "Timers per
//!   mode": `missions::real_round`, `missions::warmup`; the install's
//!   mission file, else our data; none: no timer).
//! - Round over at the first frame that sees a requested end or the timer
//!   at 0: the mode's time-out rule picks a winner if none was set, the
//!   winner's round wins +1 (not in the warm-up), the result recorded
//!   (`CaMatch::results`); `object_end` s on.
//! - Round end at the next frame after that: every body and every weapon
//!   item on the ground removed (S11; grenades stay), the round's counters
//!   cleared, the round counted (not the warm-up); the mode decides
//!   whether the match is over (`CaMode::match_end`, never after the
//!   warm-up); then the intermission (`mission_end` s), then the next
//!   mission start or the match's result.
//! - The map's objects (S15): at mission start the mission's controllers
//!   (`ON`) and start triggers (`TRIGGER`), then the round's; at round
//!   over the round's end triggers; at round end the mission's
//!   (`missions::map_messages`, the stock maps' `LevelNNStartTrigger` /
//!   `LevelNNEndTrigger`), as `objects::CaObjectMessage`s.
//! - The warm-up (S9, `CaMode::warmup`): the match's first round, nothing
//!   counts, no winner; the dead revived every `warmup_revive` s in the
//!   modes that do (`CaMode::warmup_revives`).
//! - Joining (S1, `enter_game`, the network's `JoinRules::enter`): in a
//!   mode that respawns, at once (S4's points, protected); in a round mode,
//!   held until the next setup when the real round has been live for a
//!   whole second and at least `late_join_wait` characters (the joiner and
//!   the observers counted) are in, else at once (a ticking bomb holds
//!   from the count alone; Quarantine holds by its own rule once the hosts
//!   are chosen); with join in progress off, held until the next setup.
//! - Leaving (S10): the mode's rule (`CaMode::leaving`) runs while the
//!   leaver still counts (`rules`' observer on the player's removal).
//! - A fresh match (`new_game`: a map loaded, `ca_restart`): back to
//!   waiting, the cursors at 0, everyone's statistics cleared. A map
//!   started without one (the app's `--map`) finds its mission at the
//!   first mission start (`missions::ensure_resolved`).
//!
//! Known shortcuts: a round's end and the match's result are kept as data
//! (`CaMatch::results`, `result`) and replicated (`game::net`); the reward
//! roulette, the report to the lobby and the late-join closing (S1) aren't
//! there; the match clock (`MissionTimeLimit`) isn't kept (it ends
//! nothing, Quirk 11); the intermission (`MissionEnd`) comes after every
//! round's end (S3 step 6, Open question 21: every stock file's next
//! objective is -1, so a file's `JSFWin`/`CreedWin` aren't read);
//! a match over stays over until `ca_restart` or a map load.

use bevy::prelude::*;

use super::{
    super::{grenades::SERVER_FRAME, inventory::Dropped, missions, objects::CaObjectMessage},
    CaRuleSettings, current_mode, objectives,
    scoring::{CaDamageLog, sync_score},
    spawns::{self, CaHeld, CaSpawnCursors},
    state::{CaMatch, CaStats, Phase, plays},
};
use crate::{
    core::{Connecting, Dead, FreezeTime, Health, Intent, MapLoads, Npc, SimClock, Spectating, Team},
    mode::{Flow, FlowEvent, Span},
    map::ragdoll::{Ragdoll, Ragdolled},
};

/// The mission update's frame counter (one update per server frame): the
/// runtime's.
pub type CaMissionFrames = crate::mode::flow::MissionFrames;

/// The players in the match: on Alpha or Bravo, in the game (not
/// connecting, spectating or an NPC), in the order they came in
/// (`CaJoinOrder`).
pub fn players(world: &mut World) -> Vec<(Entity, u8)> {
    let mut q = world.query_filtered::<(Entity, &Team, Option<&CaJoinOrder>), Playing>();
    in_join_order(q.iter(world))
}

/// Who `players` lists (with a team that plays): in the game, not
/// connecting, spectating or an NPC.
pub type Playing = (With<Intent>, With<Health>, Without<Connecting>, Without<Spectating>, Without<Npc>);

/// `players` from a query's rows (a system's: `objectives::Roster`).
pub fn in_join_order<'a>(rows: impl Iterator<Item = (Entity, &'a Team, Option<&'a CaJoinOrder>)>) -> Vec<(Entity, u8)> {
    let mut all: Vec<(u64, Entity, u8)> = rows
        .filter(|(_, t, _)| plays(t.0))
        .map(|(e, t, o)| (o.map_or(u64::MAX, |o| o.0), e, t.0))
        .collect();
    all.sort();
    all.into_iter().map(|(_, e, t)| (e, t)).collect()
}

/// The order a character came into the game (the server's player list's
/// order): given when it first gets a team.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CaJoinOrder(pub u64);

/// Characters with a team numbered as they come (`CaJoinOrder`).
pub(super) fn number_players(add: On<Add<Team>>, mut next: Local<u64>, mut commands: Commands) {
    *next += 1;
    if let Ok(mut e) = commands.get_entity(add.entity) {
        e.try_insert(CaJoinOrder(*next));
    }
}

/// Every character on the server that is no bot and not connecting, the
/// observers and spectators included (the late-join count).
fn characters(world: &mut World) -> u32 {
    world
        .query_filtered::<(), (With<Intent>, With<Health>, Without<Connecting>, Without<Npc>)>()
        .iter(world)
        .count() as u32
}

/// The living players of `team` in the match.
pub fn alive(world: &mut World, team: u8) -> usize {
    let living: Vec<Entity> = world
        .query_filtered::<(Entity, &Health), (Without<Dead>, Without<CaHeld>)>()
        .iter(world)
        .filter(|(_, h)| h.current > 0.0)
        .map(|(e, _)| e)
        .collect();
    players(world)
        .into_iter()
        .filter(|(e, t)| *t == team && living.contains(e))
        .count()
}

/// Every living player in the match (any team).
pub fn alive_players(world: &mut World) -> Vec<(Entity, u8)> {
    let living: Vec<Entity> = world
        .query_filtered::<(Entity, &Health), (Without<Dead>, Without<CaHeld>)>()
        .iter(world)
        .filter(|(_, h)| h.current > 0.0)
        .map(|(e, _)| e)
        .collect();
    players(world).into_iter().filter(|(e, _)| living.contains(e)).collect()
}

/// S1: `e`'s team as the mode makes teams (`mode::assign_team`): Alpha or
/// Bravo (a player without one goes on the smaller team, Alpha on a tie);
/// every player a team of their own in a free-for-all mode (the lowest
/// free id from `FFA_FIRST`); Alpha for everyone at a round's setup in Spy
/// Hunt and Quarantine (`reset`). Observers and players without a team
/// are left alone.
pub fn assign_team(world: &mut World, e: Entity, reset: bool) {
    crate::mode::assign_team(world, e, reset);
}

/// Combat Arms' side of the mode runtime (`mode::Flow`): the mission
/// update once per server frame (`SERVER_FRAME`), the players in join
/// order, the respawn (S4, S5), the phases' lengths (the mission's and the
/// room's) and its own work at each step (`event`).
pub const FLOW: Flow = Flow {
    frame: SERVER_FRAME,
    players,
    respawn: spawns::respawn,
    length,
    event,
    options: crate::mode::FlowOptions::MISSIONS,
};

/// A phase's length: the setup (`object_start`, 0 in the modes that force
/// it), the round and the warm-up (missions.md "Timers per mode"), round
/// over (`object_end`), the intermission (`mission_end`), the warm-up's
/// revives (`warmup_revive`).
fn length(world: &mut World, span: Span) -> Option<f64> {
    let settings = world.resource::<CaRuleSettings>();
    match span {
        Span::Setup => Some(missions::setup_length(world)),
        Span::Round => missions::real_round(world),
        Span::Warmup => missions::warmup(world),
        Span::RoundOver => Some(settings.object_end),
        Span::Intermission => Some(settings.mission_end),
        Span::Revive => Some(settings.warmup_revive),
    }
    .map(f64::from)
}

/// Combat Arms' work at the runtime's steps (S3, S11, S15).
fn event(world: &mut World, event: FlowEvent) {
    match event {
        // A map started without a mission finds it now.
        FlowEvent::Prepare { first: true } => missions::ensure_resolved(world),
        FlowEvent::Prepare { first: false } => {}
        FlowEvent::ResetStats => reset_stats(world),
        FlowEvent::MissionStart => {
            let drops = current_mode(world).is_some_and(|m| m.death_drops);
            world.resource_mut::<super::super::inventory::CaDeathDrops>().0 = drops;
            objectives::clear_players(world);
            // Damage from an earlier round never earns an assist
            // (Corrections 2026-10-22, 5).
            world.resource_mut::<CaDamageLog>().0.clear();
            world.resource_mut::<super::free_for_all::CaKillsMade>().0 = 0;
            // The map's mode objects: the mission's controllers and start
            // triggers, then the round's (S3 steps 2 and 3, S15).
            map_step(world, missions::MapStep::MissionStart);
            map_step(world, missions::MapStep::RoundSetup);
        }
        // A free-for-all round's winner is a player.
        FlowEvent::RoundWonBy(p) => super::scoring::stats(world, p).round_wins += 1,
        // The round's end triggers (S3 step 5).
        FlowEvent::RoundOver => map_step(world, missions::MapStep::RoundOver),
        FlowEvent::RoundEnd => {
            clean_up(world);
            // Every round ends its mission: the mission's end triggers.
            map_step(world, missions::MapStep::MissionEnd);
        }
        FlowEvent::RoundEnded => super::update_team_counts(world),
        FlowEvent::MatchEnd => {}
    }
}

/// The playing mission's messages of `step` to the map's objects
/// (`objects::CaObjectMessage`, run at the objects' next frame); none
/// without the install's mission or the objects.
fn map_step(world: &mut World, step: missions::MapStep) {
    if !world.contains_resource::<Messages<CaObjectMessage>>() {
        return;
    }
    for (target, message) in missions::map_messages(world, step) {
        world.write_message(CaObjectMessage {
            target,
            message: message.into(),
        });
    }
}

/// S11: every body and every weapon item on the ground goes (live
/// grenades stay).
pub fn clean_up(world: &mut World) {
    let items: Vec<(Entity, Entity)> = world
        .query::<(Entity, &Dropped)>()
        .iter(world)
        .map(|(e, d)| (e, d.weapon))
        .collect();
    for (item, weapon) in items {
        if world
            .get::<crate::weapon::Weapon>(weapon)
            .is_some_and(|w| w.owner.is_none())
        {
            world.despawn(weapon);
        }
        if world.get_entity(item).is_ok() {
            world.despawn(item);
        }
    }
    let ragdolls: Vec<(Entity, Ragdoll)> = world
        .query::<(Entity, &Ragdoll)>()
        .iter(world)
        .filter(|(_, r)| !r.placed)
        .map(|(e, r)| (e, r.clone()))
        .collect();
    for (e, r) in ragdolls {
        for x in r.joints.iter().chain(&r.bodies).chain(&r.visual) {
            if world.get_entity(*x).is_ok() {
                world.despawn(*x);
            }
        }
        world.despawn(e);
        if let Ok(mut owner) = world.get_entity_mut(r.owner) {
            owner.remove::<Ragdolled>();
        }
    }
}

/// Everyone's statistics back to nothing.
fn reset_stats(world: &mut World) {
    let all: Vec<Entity> = world.query_filtered::<Entity, With<CaStats>>().iter(world).collect();
    for e in all {
        world.entity_mut(e).insert(CaStats::default());
        sync_score(world, e);
    }
    world.resource_mut::<CaDamageLog>().0.clear();
}

/// What a frozen player's intent keeps in the setup: everything but the
/// movement keys.
pub fn frozen_intent(i: &Intent) -> Intent {
    Intent {
        move_axis: Vec2::ZERO,
        jump: false,
        ..i.clone()
    }
}

/// The setup's freeze on every intent (the server's and, through
/// `net::Holds`, a client's prediction), everyone's while the match holds
/// them immune (`CaMatch::immune`), and a planter's while the bomb's fire
/// animation plays (M3: movement locked, turning allowed).
pub(super) fn hold_frozen(
    frozen: Res<FreezeTime>,
    m: Option<Res<CaMatch>>,
    mut intents: Query<(&mut Intent, Option<&objectives::CaAction>)>,
) {
    let all = frozen.0 || m.is_some_and(|m| m.immune);
    for (mut i, action) in &mut intents {
        if all || action.is_some_and(|a| a.kind == objectives::ActionKind::Plant) {
            let held = frozen_intent(&i);
            *i = held;
        }
    }
}

/// When the setup's freeze ends (`net::Holds::freeze_ends`).
pub fn freeze_ends(world: &World) -> Option<f64> {
    match world.get_resource::<CaMatch>()?.phase {
        Phase::Setup { until } => Some(until),
        _ => None,
    }
}

/// A player has come into the game (S1; the network's
/// `JoinRules::enter`): spawned or held as the mode says.
pub fn enter_game(world: &mut World, e: Entity) {
    if world.get_entity(e).is_err() {
        return;
    }
    // The lobby keeps a player out of the match until their room's "go".
    if super::super::lobby::host::holds(world, e) {
        return;
    }
    world.entity_mut(e).remove::<Connecting>();
    let Some(mode) = current_mode(world) else {
        crate::net::parts::enter_at_spawn(world, e);
        return;
    };
    let settings = world.resource::<CaRuleSettings>().clone();
    let phase = world.resource::<CaMatch>().phase;
    // A warm-up takes joiners at once (game_modes_2.md test case 34).
    let live = phase == Phase::Live && !world.resource::<CaMatch>().warmup;
    let started = !matches!(phase, Phase::Waiting | Phase::Starting);
    assign_team(world, e, false);
    // Corrections (2026-10-22) 3, 4: the count includes the joiner (and
    // observers); it holds from the live round's first whole second, or
    // while a bomb ticks; a mode without the count (Quarantine) is held
    // by its own rule alone.
    let in_game = characters(world);
    let m = world.resource::<CaMatch>();
    let seconds = (world.resource::<SimClock>().now - m.live_since + 1e-9).floor() > 0.0;
    let no_joining = settings.join_in_progress == 0 || super::no_join_in_progress(mode.key);
    let own = (mode.hooks.holds_joiner)(world);
    let counted = in_game >= settings.late_join_wait && (seconds || own);
    let hold = (started && no_joining)
        || (!mode.respawns && live && if mode.hooks.counts_joiners { counted } else { own });
    if hold {
        spawns::hold(world, e);
    } else {
        spawns::respawn(world, e);
    }
}

/// A fresh match on the map just loaded (`gameplay::NewGame`), or by
/// `ca_restart`: waiting for players, the cursors at 0, the statistics
/// cleared; the map loads counted.
pub fn new_game(world: &mut World) {
    restart(world);
    world.get_resource_or_init::<MapLoads>().0 += 1;
}

/// The match from the start (`ca_restart`): waiting for players (the
/// next frame starts it when there are some).
pub fn restart(world: &mut World) {
    missions::resolve(world);
    *world.resource_mut::<CaMatch>() = CaMatch::default();
    *world.resource_mut::<CaSpawnCursors>() = CaSpawnCursors::default();
    world.resource_mut::<FreezeTime>().0 = false;
    objectives::reset(world);
    reset_stats(world);
    // The modes' own: Hired Guns' banks and mercenaries, VIP Escort's
    // roles and VIP, the clan modes' half.
    super::mercs::remove_all(world);
    world.insert_resource(super::hired_guns::CaHiredGuns::default());
    let banked: Vec<Entity> = world
        .query_filtered::<Entity, Or<(With<super::hired_guns::CaBank>, With<super::hired_guns::CaPaid>)>>()
        .iter(world)
        .collect();
    for e in banked {
        world
            .entity_mut(e)
            .remove::<(super::hired_guns::CaBank, super::hired_guns::CaPaid, super::super::movement::CustomMovePenalty)>();
    }
    super::escort::reset(world);
    super::clan::reset(world);
}
