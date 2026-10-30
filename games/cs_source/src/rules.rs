//! Match rules. For now one ruleset, deathmatch: the dead stop acting,
//! then respawn at a spawn point with full health and fresh starting
//! weapons; kills and deaths are counted.
//!
//! Deathmatch: the dead (`Dead`, with their collider disabled so bodies don't
//! block anyone) stop acting and respawn at their team's spawn points (one
//! nobody alive stands on) with fresh weapons after `mp_respawn_delay`;
//! every spawn (`put_at_spawn`: a respawn, a round's start, survivors too)
//! ends a noclip the `noclip` command turned on, as CS:S's spawns put
//! players back to walking (specs/cs_source/movement.md "Noclip and
//! spawning"); a network server's remote players are kept out of the game while their client
//! loads the map (`hold_out`, `core::Connecting`) and come in as the rules
//! allow (`enter_game`: at once in deathmatch, in the freeze time and when
//! nobody of its team is alive, else at the next round); `MapLoads` counts map
//! loads (`new_game`); `Score` (kills, deaths); `mp_friendlyfire` (applied in
//! `core::apply_damage` through `core::FriendlyFire`); `jointeam 0|1|2|3` (0:
//! auto-assign); the join flow (`JoinChoice`: the menus or a team at once;
//! `make_unassigned`: in the game on no team, `core::Unassigned`; `join_team`
//! from it waits for a class, `core::ChoosingClass`, neither spawned nor
//! counted; `join_class`/`joinclass`: `core::PlayerClass`, 0 auto-selects, then
//! in as `enter_game` would); `mp_fadetoblack` (`FadeToBlack`, drawn by
//! `client::senses`); `mp_autoteambalance` (`balance_teams`, as a round ends
//! into the next: a side two or more larger gives players to the other, bots
//! first, never onto a side without spawns); the spectator team
//! (`join_spectators`, `spectate`, `jointeam 1`, `mp_allowspectators`:
//! `core::Spectating`, no team, dead with no body or weapons, never spawned or
//! counted for a round end, kept over map changes; `join_team` from it plays as
//! `enter_game` would); `rounds.rs`: Counter-Strike rounds (`chimera_rounds 1`:
//! freeze (`core::FreezeTime`: intents held, bots' too), timed rounds, the
//! objectives' round-end table (bomb exploded/defused, eliminations with and
//! without a planted bomb, all hostages rescued, time out by map kind:
//! `RoundEndReason` with the game's words), per-reason money and objective
//! rewards, `BuyWindow`, `RoundOpen`, `RoundEnded` with its `RoundSummary`
//! (MVP, fun fact, live time; counted in `RoundStats`), `mp_tkpunish`; a new
//! round counts `core::RoundRestarts` up (`core::restart_round`, triggering
//! `core::RoundRestart`) and calls `objectives::round_start`;
//! `mp_restartgame <seconds>` starts the game over (in deathmatch: everyone
//! respawns)); `game_end.rs`: the end of the game on a map (`mp_timelimit`,
//! with rounds also `mp_maxrounds` and `mp_winlimit`, checked as a round's end
//! pause runs out; `timeleft`): `RoundState::game_over` holds everyone for the
//! intermission (`mp_chattime`; the scoreboard shows, replicated in
//! `NetRound`), then `map`/`changelevel` to the next of `MapCycle` (filled by
//! the client and `mashup_server` from the install's map cycle,
//! `games::cs_source::map_cycle`), or a fresh game without one;
//! `hints.rs`: the automatic hints (`cl_autohelp`: data-named triggers,
//! each hint once per player per map); `events.rs`: CS:S's game events and
//! server log lines (bomb, hostages, rounds, weapons, deaths; `game_log`).
//!
//! Known shortcuts (docs/tech-debt.md): Team balance (`rules::balance_teams`,
//! `mp_autoteambalance`; bots never move, measured; the rest unmeasured):
//! checked as a round's end pause runs out; a side two or more players larger
//! gives human players until they differ by one; which human moves is ours
//! (fewest kills, the highest entity); no "*** Auto-Team Balance next round ***"
//! warning (the game's `Cstrike_TitlesTXT_Auto_Team_Balance_Next_Round`) or
//! message beyond the usual team-join chat line; never onto a side the map has
//! no spawns for. Why it's OK for now: Capture C11 in
//! docs/plans/active/parity-gaps.md.

pub mod events;
pub mod game_end;
pub mod hints;
pub mod rounds;

// The rule-module interface (chimera/sim), at its old path.
pub use chimera_sim::rule_module as module;

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::{
    console::{ConsoleAppExt, resource_cvar},
    core::{Died, Health, Intent, SimSet, SpawnPoint, Team, Velocity},
    weapon::{Inventory, StartingWeapons, give},
};

pub struct DeathmatchPlugin;

impl Plugin for DeathmatchPlugin {
    fn build(&self, app: &mut App) {
        // A map loaded (the client's `map`, the greybox) starts a new game
        // (while CS:S is the gameplay's rules).
        crate::gameplay::GameplayAppExt::game_hook(app, crate::game::GAME, crate::gameplay::NewGame(new_game));
        app.init_resource::<Deathmatch>().add_message::<Died>().add_systems(
            FixedUpdate,
            (
                (rounds::run_rounds, respawn, hold_the_dead, rounds::hold_frozen)
                    .chain()
                    .in_set(SimSet::Rules),
                (count_deaths, rounds::kill_rewards, rounds::objective_money, apply_score_changes).after(SimSet::Weapons),
            )
                // The rules are the server's.
                .run_if(crate::core::authoritative)
                // While CS:S is the gameplay.
                .run_if(crate::objectives::plays),
        );
        rounds::plugin(app);
        game_end::plugin(app);
        events::plugin(app);
        hints::plugin(app);
        module::plugin(app);
        resource_cvar::<Deathmatch, f32>(app, "mp_respawn_delay", "Seconds before the dead respawn.", |d| {
            &mut d.respawn_delay
        });
        app.init_resource::<crate::core::FriendlyFire>();
        resource_cvar::<crate::core::FriendlyFire, u8>(app, "mp_friendlyfire", "1: teammates hurt each other.", |f| {
            &mut f.0
        });
        app.init_resource::<FadeToBlack>();
        resource_cvar::<FadeToBlack, u8>(
            app,
            "mp_fadetoblack",
            "1: a dead player's view fades to black until the next round (no spectating).",
            |f| &mut f.0,
        );
        app.init_resource::<crate::core::SpectatorRules>();
        resource_cvar::<crate::core::SpectatorRules, u8>(
            app,
            "mp_forcecamera",
            "Spectating while dead: 0 any living player, 1 only your team.",
            |r| &mut r.force_camera,
        );
        resource_cvar::<crate::core::SpectatorRules, f32>(
            app,
            "spec_freeze_traveltime",
            "Seconds the freeze cam takes to zoom toward the killer.",
            |r| &mut r.freeze_travel,
        );
        resource_cvar::<crate::core::SpectatorRules, f32>(
            app,
            "spec_freeze_time",
            "Seconds the freeze cam's picture holds still.",
            |r| &mut r.freeze_hold,
        );
        app.init_resource::<crate::core::PlayerIdMode>();
        resource_cvar::<crate::core::PlayerIdMode, u8>(
            app,
            "mp_playerid",
            "Whose names show under the crosshair: 0 everyone's, 1 teammates' only, 2 nobody's.",
            |m| &mut m.0,
        );
        resource_cvar::<Deathmatch, u32>(
            app,
            "mp_limitteams",
            "In a network game, most players one team may have over the other (0: no limit).",
            |d| &mut d.limit_teams,
        );
        resource_cvar::<Deathmatch, u8>(
            app,
            "mp_autoteambalance",
            "1: with rounds, a side two or more players larger gives players to the other as the next round starts.",
            |d| &mut d.auto_team_balance,
        );
        resource_cvar::<Deathmatch, u8>(
            app,
            "mp_allowspectators",
            "1: players may join the spectators (jointeam 1, spectate).",
            |d| &mut d.allow_spectators,
        );
        app.init_resource::<JoinChoice>()
            .add_message::<TeamRequested>()
            .add_message::<ClassRequested>()
            .console_command(
                "jointeam",
                "jointeam <0|1|2|3>: auto-assign (0: the smaller team), the spectators (1), the terrorists (2) or \
                 counter-terrorists (3); a new team respawns at its spawn.",
                |w, a| {
                    let team = match a.first().map(String::as_str) {
                        Some("0") => auto_assign(w),
                        Some("1") => Team(0),
                        Some("2") => Team(1),
                        Some("3") => Team(2),
                        _ => {
                            return Err(
                                "jointeam <0|1|2|3> (0: auto-assign, 1: spectators, 2: terrorists, 3: counter-terrorists)"
                                    .into(),
                            );
                        }
                    };
                    request_team(w, team)
                },
            )
            .console_command("spectate", "Join the spectators (jointeam 1).", |w, _| request_team(w, Team(0)))
            .console_command(
                "joinclass",
                "joinclass <0-8>: the player model, after picking a team: 0 auto-select, 1-4 the terrorists' \
                 (Phoenix Connexion, Elite Crew, Arctic Avengers, Guerilla Warfare), 5-8 the counter-terrorists' \
                 (SEAL Team 6, GSG-9, SAS, GIGN).",
                |w, a| {
                    let class = a
                        .first()
                        .and_then(|c| c.parse::<u8>().ok())
                        .filter(|c| *c <= 2 * crate::core::PlayerClass::PER_TEAM)
                        .ok_or("joinclass <0-8>")?;
                    if w.get_resource::<crate::core::NetRole>() == Some(&crate::core::NetRole::Client) {
                        w.write_message(ClassRequested(class));
                        return Ok(None);
                    }
                    let player = w
                        .query_filtered::<Entity, With<crate::core::LocalPlayer>>()
                        .iter(w)
                        .next()
                        .ok_or("no local player")?;
                    join_class(w, player, class).map(Some)
                },
            );
    }
}

/// `jointeam` and `spectate`: `Team(0)` is the spectators. A network
/// client asks the server (`net`); otherwise the local player joins.
fn request_team(w: &mut World, team: Team) -> Result<Option<String>, String> {
    if w.get_resource::<crate::core::NetRole>() == Some(&crate::core::NetRole::Client) {
        w.write_message(TeamRequested(team));
        return Ok(None);
    }
    let player = w
        .query_filtered::<Entity, With<crate::core::LocalPlayer>>()
        .iter(w)
        .next()
        .ok_or("no local player")?;
    if team.0 == 0 {
        join_spectators(w, player).map(Some)
    } else {
        join_team(w, player, team).map(Some)
    }
}

/// A network client's `jointeam`: the team it asks the server for
/// (`Team(0)`: the spectators).
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct TeamRequested(pub Team);

/// A network client's `joinclass`: the class it asks the server for.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct ClassRequested(pub u8);

/// `jointeam 0`, auto-assign: the team with fewer players (the
/// terrorists on a tie), not counting the local player itself.
pub fn auto_assign(w: &mut World) -> Team {
    let me = w
        .query_filtered::<Entity, With<crate::core::LocalPlayer>>()
        .iter(w)
        .next();
    let mut counts = [0usize; 3];
    for (e, t) in w
        .query_filtered::<(Entity, &Team), (With<Intent>, Without<crate::objectives::hostages::Hostage>)>()
        .iter(w)
    {
        if Some(e) != me && (1..=2).contains(&t.0) {
            counts[t.0 as usize] += 1;
        }
    }
    if counts[2] < counts[1] { Team(2) } else { Team(1) }
}

/// Put `player` in the game on no team (`core::Unassigned`, as CS:S has
/// a player who just joined): out of play with no body, as a spectator
/// (`join_spectators`) but whatever `mp_allowspectators` says, until it
/// picks a team (`join_team`, `join_spectators`).
pub fn make_unassigned(w: &mut World, player: Entity) {
    if w.get_entity(player).is_err() {
        return;
    }
    leave_play(w, player);
    w.entity_mut(player)
        .remove::<crate::core::ChoosingClass>()
        .insert((crate::core::Unassigned, crate::core::PlayerClass(0)));
}

/// `joinclass`: `player`'s class (`core::PlayerClass`; 0 auto-select:
/// one of its team's at random, as CS:S picks). Picked for the first time
/// after joining a team (`core::ChoosingClass`), it comes into play as a
/// player coming into the game does (`enter_game`); later, it is worn
/// from the next spawn.
pub fn join_class(w: &mut World, player: Entity, class: u8) -> Result<String, String> {
    let team = w.get::<Team>(player).copied().filter(|t| (1..=2).contains(&t.0));
    let Some(team) = team.filter(|_| w.get::<crate::core::Spectating>(player).is_none()) else {
        return Err("Join a team first.".into());
    };
    let per_team = crate::core::PlayerClass::PER_TEAM;
    let class = if class == 0 {
        // Auto-select: one of the team's, by the clock and the player.
        let clock = w.resource::<Time>().elapsed().as_nanos() as u64;
        let roll = (clock ^ player.to_bits()).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 32;
        (team.0 - 1) * per_team + 1 + (roll % per_team as u64) as u8
    } else {
        class
    };
    if crate::core::PlayerClass(class).team() != Some(team) {
        return Err("That class is the other team's.".into());
    }
    w.entity_mut(player).insert(crate::core::PlayerClass(class));
    if w.entity_mut(player).take::<crate::core::ChoosingClass>().is_some() {
        spawn_if_room(w, player);
    }
    Ok(format!("class {class}"))
}

/// Put `player` on `team`: it respawns at the new team's spawn (with
/// rounds, at the next round; no death counted). In a network game
/// (`NetRole::Server`) a team may not get more than `mp_limitteams`
/// players ahead of the other.
pub fn join_team(w: &mut World, player: Entity, team: Team) -> Result<String, String> {
    if w.get::<Team>(player) == Some(&team) {
        return Ok("already on that team".into());
    }
    let limit = w.resource::<Deathmatch>().limit_teams;
    if limit > 0 && w.get_resource::<crate::core::NetRole>() == Some(&crate::core::NetRole::Server) {
        let mut counts = [0u32; 3];
        for (e, t) in w
            .query_filtered::<(Entity, &Team), (With<Intent>, Without<crate::objectives::hostages::Hostage>)>()
            .iter(w)
        {
            if e != player && (1..=2).contains(&t.0) {
                counts[t.0 as usize] += 1;
            }
        }
        let other = if team.0 == 1 { 2 } else { 1 };
        if counts[team.0 as usize] + 1 > counts[other] + limit {
            return Err(format!(
                "Too many {} (mp_limitteams {limit}): join the other team.",
                if team.0 == 1 { "Terrorists" } else { "Counter-Terrorists" }
            ));
        }
    }
    // Switching teams: respawn at the new team's spawn now (no death
    // counted).
    let was_spectating = w.entity_mut(player).take::<crate::core::Spectating>().is_some();
    // Just come into the game (the team menu): no body until it picks a
    // class (`join_class`), as CS:S shows the class menu first.
    let first_team = w.entity_mut(player).take::<crate::core::Unassigned>().is_some();
    w.entity_mut(player).insert((team, Dead { since: f64::MIN }, ColliderDisabled));
    // Another team's class doesn't fit: auto-select's at the next pick.
    if w.get::<crate::core::PlayerClass>(player)
        .and_then(|c| c.team())
        .is_some_and(|t| t != team)
    {
        w.entity_mut(player).insert(crate::core::PlayerClass(0));
    }
    if first_team {
        w.entity_mut(player).insert(crate::core::ChoosingClass);
    } else if was_spectating {
        // Off the spectators, with rounds: into this round when a player
        // coming into the game would be (`enter_game`), else the next.
        spawn_if_room(w, player);
    }
    Ok(format!(
        "joined the {}",
        if team.0 == 1 {
            "terrorists"
        } else {
            "counter-terrorists"
        }
    ))
}

/// Put `player` on the spectators (CS:S's team 1; `mp_allowspectators`):
/// out of play with no body (`core::Spectating`). A living player leaves
/// the round as the dead do: the bomb and a kit drop where it stood, its
/// other weapons go; no death is counted (as with `join_team`). It
/// stays a spectator over rounds and map changes until it joins a team.
pub fn join_spectators(w: &mut World, player: Entity) -> Result<String, String> {
    let unassigned = w.get::<crate::core::Unassigned>(player).is_some();
    if w.get::<crate::core::Spectating>(player).is_some() && !unassigned {
        return Ok("already spectating".into());
    }
    if w.resource::<Deathmatch>().allow_spectators == 0 {
        return Err("You cannot become a spectator.".into());
    }
    w.entity_mut(player)
        .remove::<(crate::core::Unassigned, crate::core::ChoosingClass)>();
    leave_play(w, player);
    Ok("joined the spectators".into())
}

/// Out of play with no body, on no team (`core::Spectating`): a living
/// player leaves the round as the dead do (its bomb and kit dropped, its
/// other weapons gone, no death counted).
fn leave_play(w: &mut World, player: Entity) {
    let alive = w.get::<Dead>(player).is_none() && w.get::<Health>(player).is_some_and(|h| h.current > 0.0);
    if alive {
        crate::objectives::bomb::drop_objective_items(w, player);
    }
    let weapons = w.get::<Inventory>(player).map(|i| i.weapons.clone()).unwrap_or_default();
    for weapon in weapons {
        if w.get_entity(weapon).is_ok() {
            w.despawn(weapon);
        }
    }
    let mut e = w.entity_mut(player);
    e.insert((
        crate::core::Spectating,
        Team(0),
        Dead { since: f64::MIN },
        ColliderDisabled,
        Inventory::default(),
    ));
    if let Some(mut h) = e.get_mut::<Health>() {
        h.current = 0.0;
    }
    if let Some(mut v) = e.get_mut::<Velocity>() {
        v.0 = Vec3::ZERO;
    }
}

/// `mp_autoteambalance`: as a round ends into the next, while one side
/// has two or more players more than the other, move players from it
/// until they differ by one at most. Bots count but never move: CS:S
/// left four terrorist bots against one counter-terrorist bot for five
/// rounds (`mp_autoteambalance 1`, `mp_limitteams 0`; parity-gaps.md
/// C11, 2026-10-14). Which human moves is ours (unmeasured): the fewest
/// kills, then the highest entity (a steady pick). They respawn on the
/// new side with the round's start (`join_team`, no death counted).
/// Returns who moved.
pub fn balance_teams(world: &mut World) -> Vec<Entity> {
    if world.resource::<Deathmatch>().auto_team_balance == 0 {
        return Vec::new();
    }
    let mut sides: [Vec<(bool, i32, Entity)>; 2] = [Vec::new(), Vec::new()];
    for (e, team, score, local, remote) in world
        .query_filtered::<(
            Entity,
            &Team,
            Option<&Score>,
            Has<crate::core::LocalPlayer>,
            Has<crate::core::RemotePlayer>,
        ), (
            With<Intent>,
            Without<crate::objectives::hostages::Hostage>,
            Without<crate::core::Connecting>,
            Without<crate::core::Spectating>,
            Without<crate::core::ChoosingClass>,
        )>()
        .iter(world)
    {
        if (1..=2).contains(&team.0) {
            let human = local || remote;
            sides[team.0 as usize - 1].push((human, score.map_or(0, |s| s.kills), e));
        }
    }
    // Only onto a side the map has spawns for (minigame maps with one
    // side's spawns keep everyone there).
    let mut spawns = [false; 2];
    for s in world.query::<&SpawnPoint>().iter(world) {
        match s.team.map(|t| t.0) {
            Some(t @ 1..=2) => spawns[t as usize - 1] = true,
            Some(_) => {}
            None => spawns = [true; 2],
        }
    }
    let mut moved = Vec::new();
    loop {
        let (big, small) = if sides[0].len() > sides[1].len() { (0, 1) } else { (1, 0) };
        if sides[big].len() < sides[small].len() + 2 || !spawns[small] {
            break;
        }
        // Humans only: the fewest kills, the highest entity.
        sides[big].sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(b.2.cmp(&a.2)));
        if !sides[big].first().is_some_and(|p| p.0) {
            break;
        }
        let who = sides[big].remove(0);
        if join_team(world, who.2, Team(small as u8 + 1)).is_err() {
            break;
        }
        sides[small].push(who);
        moved.push(who.2);
    }
    moved
}

#[derive(Resource, Clone, Debug)]
pub struct Deathmatch {
    pub respawn_delay: f32,
    /// `mp_limitteams` (network games only).
    pub limit_teams: u32,
    /// `mp_autoteambalance` (with rounds: `balance_teams`).
    pub auto_team_balance: u8,
    /// `mp_allowspectators`: 0 refuses `jointeam 1` and `spectate`.
    pub allow_spectators: u8,
    /// Spawn points picked so far (round robin, so players spread out).
    next_spawn: usize,
}

impl Default for Deathmatch {
    fn default() -> Self {
        Self {
            respawn_delay: 2.0,
            // CS:S's default.
            limit_teams: 2,
            // CS:S's default.
            auto_team_balance: 1,
            allow_spectators: 1,
            next_spawn: 0,
        }
    }
}

// Kills and deaths, the dead, the map loads counted, how a player joins
// and `mp_fadetoblack`: the engine's vocabulary (`core`), at their old
// paths here.
pub use crate::core::{Dead, FadeToBlack, JoinChoice, MapLoads, Score};

/// Respawn every character on the next tick (after a map change), with
/// fresh weapons, at the spawn points.
/// A fresh game on a newly loaded map, as a map change is in CS:S: with
/// rounds, the next step starts a new game (everyone at a spawn, start
/// money); without, everyone respawns now. Without this the dead stayed
/// dead (spectating) until the old round's clock ran out on the new map.
///
/// Remote players' clients load the new map too: they're out of the game
/// (`core::Connecting`) until they have it (`enter_game`). The map loads
/// counted (`MapLoads`) go up by one.
pub fn new_game(world: &mut World) {
    if let Some(mut state) = world.get_resource_mut::<rounds::RoundState>() {
        *state = rounds::RoundState::default();
    }
    rounds::reset_match(world);
    respawn_everyone(world);
    let remote: Vec<Entity> = world
        .query_filtered::<Entity, With<crate::core::RemotePlayer>>()
        .iter(world)
        .collect();
    for e in remote {
        hold_out(world, e);
    }
    world.get_resource_or_init::<MapLoads>().0 += 1;
}


/// Keep a remote player out of the game while its client loads the map
/// (`core::Connecting`): dead, not solid, no health, so nothing hits it
/// and the rules neither spawn it nor count it on its team.
pub fn hold_out(world: &mut World, e: Entity) {
    let Ok(mut ent) = world.get_entity_mut(e) else { return };
    ent.insert((crate::core::Connecting, Dead { since: f64::MIN }, ColliderDisabled));
    if let Some(mut h) = ent.get_mut::<Health>() {
        h.current = 0.0;
    }
    if let Some(mut v) = ent.get_mut::<Velocity>() {
        v.0 = Vec3::ZERO;
    }
}

/// A player's client has the map: it is in the game, and spawns as the
/// rules allow. Deathmatch: at once. Rounds: at once during the freeze
/// (or before the first round starts: with everyone) or when nobody of
/// its team is alive (so its coming in dead doesn't end the round), else
/// at the next round (CS:S lets players who join in the freeze time play
/// the round; the exact grace, and CS:S's restart when an empty team
/// gets a player, aren't reproduced).
pub fn enter_game(world: &mut World, e: Entity) {
    if world.get::<crate::core::Connecting>(e).is_none() {
        return;
    }
    world.entity_mut(e).remove::<crate::core::Connecting>();
    // A spectator stays one (a map change).
    if world.get::<crate::core::Spectating>(e).is_none() {
        spawn_if_room(world, e);
    }
}

/// With rounds, a player coming into the game (`enter_game`, or off the
/// spectators) plays now as described there, else at the next round.
/// Deathmatch respawns it as the dead.
fn spawn_if_room(world: &mut World, e: Entity) {
    let rounds = world
        .get_resource::<rounds::RoundSettings>()
        .is_some_and(|r| r.enabled != 0);
    let freeze = world
        .get_resource::<rounds::RoundState>()
        .is_some_and(|r| matches!(r.phase, rounds::Phase::Freeze { .. }));
    // Nobody of its team alive in the round (it was empty: everyone else
    // still loading, or it is the first on it): it plays now, rather than
    // its team losing the round the moment it comes in dead.
    let team = world.get::<Team>(e).copied();
    let mates_alive = world
        .query_filtered::<(Entity, &Team, &Health), (With<Intent>, Without<Dead>, Without<crate::core::Connecting>)>()
        .iter(world)
        .any(|(o, t, h)| o != e && Some(*t) == team && h.current > 0.0);
    if rounds && (freeze || !mates_alive) {
        if let Some(start) = world.get_resource::<rounds::RoundSettings>().map(|s| s.start_money)
            && world.get::<crate::weapon::economy::Money>(e).is_none()
        {
            world.entity_mut(e).insert(crate::weapon::economy::Money(start));
        }
        put_at_spawn(world, e, true);
    }
}

pub fn respawn_everyone(world: &mut World) {
    let all: Vec<Entity> = world
        .query_filtered::<Entity, (With<Intent>, With<Health>, Without<crate::objectives::hostages::Hostage>)>()
        .iter(world)
        .collect();
    // Unseen until they respawn: nobody bumps into them (the server's
    // characters; a network client's are the server's to say).
    let authority = world.get_resource::<crate::core::NetRole>() != Some(&crate::core::NetRole::Client);
    for e in all {
        world.entity_mut(e).insert(Dead { since: f64::MIN });
        if authority {
            world.entity_mut(e).insert(ColliderDisabled);
        }
    }
}

/// Score changes from map logic (game_score; specs/source/
/// game_entities.md 4): a player's kills (a negative change without
/// "Allow Negative" stops at 0 and does nothing below it), or its team's
/// rounds won (our reading of "team score", spec open question 8; ours
/// can't go below 0).
fn apply_score_changes(
    mut changes: MessageReader<crate::core::ScoreChange>,
    mut scores: Query<&mut Score>,
    teams: Query<&Team>,
    rounds: Option<ResMut<rounds::RoundState>>,
    mut commands: Commands,
) {
    let mut rounds = rounds;
    for c in changes.read() {
        if c.team {
            let Some(r) = rounds.as_mut() else { continue };
            let Some(i) = teams.get(c.target).ok().and_then(|t| (t.0 as usize).checked_sub(1)).filter(|i| *i < 2) else {
                continue;
            };
            r.wins[i] = (r.wins[i] as i64 + c.points as i64).max(0) as u32;
            continue;
        }
        let kills = scores.get(c.target).map_or(0, |s| s.kills);
        let new = if c.points >= 0 || c.allow_negative {
            kills + c.points
        } else if kills < 0 {
            kills
        } else {
            (kills + c.points).max(0)
        };
        if let Ok(mut s) = scores.get_mut(c.target) {
            s.kills = new;
        } else if let Ok(mut e) = commands.get_entity(c.target) {
            e.insert(Score { kills: new, deaths: 0 });
        }
    }
}

fn count_deaths(mut died: MessageReader<Died>, mut scores: Query<&mut Score>, mut commands: Commands, time: Res<Time>) {
    let now = time.elapsed_secs_f64();
    for d in died.read() {
        // A death by the world or by one's own hand (a fall, a fire, `kill`)
        // costs a kill (CS:S, measured 2026-10-17: "committed suicide with
        // world", frags −1, for a fall, a fire and `kill`).
        let suicide = d.attacker.is_none_or(|a| a == d.entity);
        let lost = if suicide { -1 } else { 0 };
        if let Ok(mut s) = scores.get_mut(d.entity) {
            s.deaths += 1;
            s.kills += lost;
        } else if let Ok(mut e) = commands.get_entity(d.entity) {
            e.insert(Score { kills: lost, deaths: 1 });
        }
        if let Some(a) = d.attacker.filter(|a| *a != d.entity) {
            if let Ok(mut s) = scores.get_mut(a) {
                s.kills += 1;
            } else if let Ok(mut e) = commands.get_entity(a) {
                e.insert(Score { kills: 1, deaths: 0 });
            }
        }
        // The dead body stops being solid (a ragdoll takes its place), so
        // it neither blocks the living nor holds them up.
        if let Ok(mut e) = commands.get_entity(d.entity) {
            e.insert((Dead { since: now }, ColliderDisabled));
        }
    }
}

// The dead neither move nor shoot (their intent is cleared each tick):
// every game's (`chimera_sim::spawning`).
use chimera_sim::spawning::hold_the_dead;

fn respawn(world: &mut World) {
    // Rounds respawn everyone at the round's start instead.
    if world
        .get_resource::<rounds::RoundSettings>()
        .is_some_and(|r| r.enabled != 0)
    {
        return;
    }
    let now = world.resource::<Time>().elapsed_secs_f64();
    let delay = world.resource::<Deathmatch>().respawn_delay as f64;
    let ready: Vec<Entity> = world
        .query_filtered::<(Entity, &Dead), (
            Without<crate::objectives::hostages::Hostage>,
            Without<crate::core::Connecting>,
            Without<crate::core::Spectating>,
            Without<crate::core::ChoosingClass>,
        )>()
        .iter(world)
        .filter(|(_, d)| now - d.since >= delay)
        .map(|(e, _)| e)
        .collect();
    for e in ready {
        put_at_spawn(world, e, true);
    }
}

/// How close (m, across) another living character may stand to a spawn
/// point for it to be free: a player's box is 32 units wide.
const SPAWN_CLEARANCE: f32 = 32.0 * 0.0254;

/// Bring a character back at one of its team's spawn points (round robin,
/// skipping those another living character stands on) with full health,
/// no velocity and out of a toggled noclip; with `fresh` weapons its old ones go and it gets the
/// starting weapons, otherwise it keeps what it carries.
pub fn put_at_spawn(world: &mut World, e: Entity, fresh: bool) {
    let spawns: Vec<(Transform, Option<Team>)> = world
        .query::<(&Transform, &SpawnPoint)>()
        .iter(world)
        .map(|(t, s)| (*t, s.team))
        .collect();
    let starting = world.resource::<StartingWeapons>().clone();
    {
        // The character's own team's spawns, else any.
        let team = world.get::<Team>(e).copied();
        let own: Vec<Transform> = spawns
            .iter()
            .filter(|(_, t)| t.is_some() && *t == team)
            .map(|(t, _)| *t)
            .collect();
        let pool: Vec<Transform> = if own.is_empty() {
            spawns.iter().map(|(t, _)| *t).collect()
        } else {
            own
        };
        // The next spawn in turn that no living character stands on (two
        // players put in one spot are stuck in each other for good), else
        // the next in turn anyway.
        let taken: Vec<Vec3> = world
            .query_filtered::<(Entity, &Transform, &Health), (With<Intent>, Without<Dead>)>()
            .iter(world)
            .filter(|(o, _, h)| *o != e && h.current > 0.0)
            .map(|(_, t, _)| t.translation)
            .collect();
        let free = |t: &Transform| {
            !taken.iter().any(|p| {
                let d = *p - t.translation;
                d.x.abs() < SPAWN_CLEARANCE && d.z.abs() < SPAWN_CLEARANCE && d.y.abs() < 2.0
            })
        };
        let at = if pool.is_empty() {
            None
        } else {
            let mut dm = world.resource_mut::<Deathmatch>();
            let start = dm.next_spawn;
            let i = (0..pool.len())
                .map(|k| (start + k) % pool.len())
                .find(|i| free(&pool[*i]))
                .unwrap_or(start % pool.len());
            dm.next_spawn = start + 1;
            Some(pool[i])
        };
        // Fresh weapons: drop the old ones.
        if fresh {
            let old = world.get::<Inventory>(e).map(|i| i.weapons.clone()).unwrap_or_default();
            for w in old {
                world.despawn(w);
            }
        }
        // Alive, still, at full health, at the spawn facing its way, and
        // out of a noclip turned on with the console command: CS:S spawns
        // every player walking (a respawn, a round start, survivors too;
        // specs/cs_source/movement.md "Noclip and spawning").
        if !chimera_sim::spawning::revive(world, e, at) {
            return;
        }
        if fresh {
            world.entity_mut(e).insert(Inventory::default());
        }
        // A map's spawn equipment (game_player_equip) replaces the kit,
        // and comes at every spawn (survivors of a round too).
        let equipment = world
            .get_resource::<crate::weapon::equip::SpawnEquipment>()
            .and_then(|s| s.0.clone());
        if let Some(items) = equipment {
            for (name, count) in items {
                crate::weapon::equip::give_item(world, e, &name, count);
            }
        } else if fresh {
            let team = world.get::<Team>(e).map(|t| t.0);
            // Rounds start from the team's own kit (you buy the rest);
            // deathmatch adds the extras everyone gets.
            let rounds = world
                .get_resource::<rounds::RoundSettings>()
                .is_some_and(|r| r.enabled != 0);
            let mut kit = starting.for_team(team);
            if rounds {
                kit.retain(|id| !starting.all.contains(id));
            }
            for id in kit {
                let weapon = give(world, e, id);
                // Rounds: you buy ammo, so the kit's reserve is the game's
                // starting amount.
                if rounds && let Some(w) = weapon {
                    crate::weapon::economy::starting_reserve(world, w);
                }
            }
        }
    }
}
