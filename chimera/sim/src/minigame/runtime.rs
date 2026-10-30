//! The primitives every minigame plays on: Chimera's side of the mode
//! runtime (`FLOW`) and the server's systems while a minigame is the
//! gameplay (`plugin`), lifted in shape from Combat Arms' flow and CS:S's
//! deathmatch, naming neither.
//!
//! - Players (`players`): characters on a team that plays, in the game
//!   (not connecting, spectating or an NPC), in the order they joined
//!   (`JoinOrder`). A new one (`join`) goes on the team the mode makes
//!   (one with no team first on the first side), comes in at once unless
//!   the match is over, and is marked `InMinigame`; the rules module
//!   hears of it first (`RulesModule::joined`: the ladder's join rung).
//! - The match waits (`Phase::Waiting`) until `players.min` are in; then
//!   the runtime starts it (everyone respawned, the module's
//!   `before_spawns` first), live at once (no setup), for `time_limit`;
//!   a round over of `ROUND_OVER` s, an intermission of `INTERMISSION` s,
//!   then the match over, its winner the round's (`Mode::match_end`).
//! - Spawning (`respawn`): at a spawn point the definition's `points`
//!   pick (`spawn_point`), revived (`spawning::revive`), the definition's
//!   movement set, the weapons taken and the life's given (the kit, then
//!   the rules module's `loadout`, the last drawn, after its `arming`;
//!   no kit when the module says so, `RulesModule::no_kit`; a kit by game
//!   is the loadout's last weapon's game's, `refit_kit` when the rules
//!   swap that weapon; a `KitMelee` on the player, a ladder rung's, is
//!   its kit's weapon of that slot, the knife's); with `identity =
//!   "weapon"` the player follows its weapon's game (`slots::
//!   FollowsWeapon`; `identity::follow` at once for the drawn one),
//!   protected (`Protected`, with `core::SpawnProtected`'s two windows:
//!   `core::God` for `protection.damage` s, attacks passing through the
//!   player for `protection.bullets` s, a hit showing
//!   nothing while either runs) until they run out or, with
//!   `protection.fire_ends`, it attacks (the fire key, or any round,
//!   swing or throw of its weapons: `end_protection`). The dead respawn after `delay` s (`respawn_dead`) unless the
//!   match is over or the mode refuses (`respawn_due`: when, which the
//!   HUD's countdown and the shell's death view read as
//!   `core::RespawnDue`).
//! - Held (`MatchHold`, a server changing map for the next match): no
//!   one joins or respawns, the phases don't step.
//! - The match's end pauses everyone (`hold_the_end`): from the win (the
//!   round over) through the intermission, the match over and a map
//!   change's hold, every intent (players' and bots') keeps only its look
//!   and `core::FreezeTime` is on (bots stand, a network client's
//!   prediction holds); lifted when the next match plays.
//! - Deaths (`handle_deaths`, after the tick's damage): the dead marked
//!   (`Dead`, not solid), `core::Score`'s deaths and an enemy kill's kill
//!   counted while the match counts, the mode's `on_death`, then its
//!   `died` hook in any phase.
//! - `take_weapon`: a weapon out of a character's hands and inventory, for
//!   the rules' swaps.
//! - The per-tick work (`plugin`, in `SimSet::Rules`, chained: `join`, the
//!   mode's update, `step_phases`, `respawn_dead`, the dead held,
//!   `end_protection`, `wins::publish`, `hold_the_end`) is normal
//!   systems with declared queries (the results are an observer of the
//!   flow's steps, `results::on_step`, at the match's end inside
//!   `step_phases`); the world work they find (a joiner
//!   taken in, a respawn) is queued on `Commands` and applied at the sync
//!   point before the next system of the chain, so the order and the
//!   tick are as before. `end_protection`'s removals apply before the
//!   tick's damage.

use avian3d::prelude::ColliderDisabled;
use bevy::{ecs::message::MessageCursor, prelude::*};

use super::{
    ActiveMinigame, MINIGAME, MinigameDef,
    def::{IdentityDef, Kit, Points, SpawnDef},
};
use crate::{
    core::{
        Connecting, Dead, Died, FreezeTime, God, Health, Intent, Npc, Score, SimClock, SimSet, SpawnPoint, SpawnProtected,
        Spectating, Team,
    },
    mode::{
        self, ALPHA, Death, DeathKind, Flow, FlowEvent, Match, Phase, Span, classify, current_mode, plays,
    },
    weapon::{Inventory, give},
};
use chimera_core::query_cache::cached_query;
use chimera_core::slots::{FollowsWeapon, MovementSlot};

/// A decided round's results stay up this long before the round ends.
pub const ROUND_OVER: f32 = 5.0;
/// From the round's end to the match over.
pub const INTERMISSION: f32 = 3.0;
/// How long (s) the match stays over, its results up, before a server
/// rotates to the next map (`MinigameDef::rotate`).
pub const RESULTS_HOLD: f64 = 8.0;
/// How close (m) another living character may be to a spawn point for it
/// to be free (about a player's width).
pub const SPAWN_CLEARANCE: f32 = 0.8;

/// Chimera's side of the mode runtime while a minigame plays.
pub const FLOW: Flow = Flow {
    frame: 0.0,
    players,
    respawn,
    length,
    event,
    // Combat Arms' missions' stepping: wait for players, the timer ends
    // the round, a round over and an intermission before the end.
    options: crate::mode::FlowOptions::MISSIONS,
};

/// The order a character joined (numbered as it gets a team).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct JoinOrder(pub u64);

/// A character the minigame took in (`join`). Replicated to clients
/// (`minigame_net`), whose HUD counts the players by it.
#[derive(Component, Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct InMinigame;

/// Spawn protection the runtime gave: its windows are the character's
/// `core::SpawnProtected`, its `core::God` (while damage is blocked) the
/// runtime's own; `fire_ends`: the first shot ends both.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Protected {
    pub fire_ends: bool,
}

/// The `Points::Map` cursor.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct SpawnCursor(pub usize);

/// The players in the match, in join order.
pub fn players(world: &mut World) -> Vec<(Entity, u8)> {
    type Playing = (With<Intent>, With<Health>, With<InMinigame>, Without<Connecting>, Without<Spectating>, Without<Npc>);
    let mut all: Vec<(u64, Entity, u8)> =
        cached_query::<(Entity, &Team, Option<&JoinOrder>), Playing, _>(world, |q, world| {
            q.iter(world)
                .filter(|(_, t, _)| plays(t.0))
                .map(|(e, t, o)| (o.map_or(u64::MAX, |o| o.0), e, t.0))
                .collect()
        });
    all.sort();
    all.into_iter().map(|(_, e, t)| (e, t)).collect()
}

fn length(world: &mut World, span: Span) -> Option<f64> {
    let def = world.get_resource::<ActiveMinigame>()?.def;
    match span {
        Span::Setup => Some(0.0),
        Span::Round => Some(def.time_limit as f64),
        Span::RoundOver => Some(ROUND_OVER as f64),
        Span::Intermission => Some(INTERMISSION as f64),
        _ => None,
    }
}

fn event(world: &mut World, event: FlowEvent) {
    match event {
        FlowEvent::ResetStats => {
            for (e, _) in players(world) {
                world.entity_mut(e).insert(Score::default());
            }
        }
        FlowEvent::RoundOver => {
            if let Some(w) = world.resource::<Match>().winner {
                let player = players(world).into_iter().find(|(_, t)| *t == w).map(|(e, _)| e);
                world.write_message(super::MinigameEvent {
                    name: "match_won",
                    player,
                    value: w as i32,
                });
            }
        }
        _ => {}
    }
}

/// Bring `e` into play for a life: at a spawn point, revived, its movement
/// set, the life's weapons given, protected.
pub fn respawn(world: &mut World, e: Entity) {
    let Some(active) = world.get_resource::<ActiveMinigame>().copied() else { return };
    let def = active.def;
    let at = spawn_point_in(world, e, def.spawn.points, def.spawn.min_distance, def.spawn.facing);
    if !crate::spawning::revive(world, e, at) {
        return;
    }
    if let Some(m) = def.slots.movement.as_deref()
        && world.get::<MovementSlot>(e).is_none_or(|s| s.0 != m)
        && let Some(id) = world
            .get_resource::<chimera_core::slots::MovementRegistry>()
            .and_then(|r| r.get(m))
            .map(|i| i.id)
    {
        chimera_core::slots::set_movement(e, id).apply(world);
    }
    let follows = def.slots.identity == Some(IdentityDef::Weapon);
    if follows {
        world.entity_mut(e).insert(FollowsWeapon);
    } else if world.get::<FollowsWeapon>(e).is_some() {
        crate::identity::unfollow(world, e);
    }
    crate::weapon::equip::strip(world, e);
    (active.rules.arming)(world, e);
    let loadout = (active.rules.loadout)(world, e);
    let kit = if (active.rules.no_kit)(world, e) { Vec::new() } else { kit_for(world, def, e, loadout.last().copied()) };
    for id in kit.into_iter().chain(loadout) {
        give(world, e, id);
    }
    // Following the weapon: the life starts as the drawn weapon's game.
    if follows {
        crate::identity::follow(world, e);
    }
    let protection = def.spawn.respawn.protection;
    // Not over a `god` of the player's own (the cheat's: not the
    // runtime's to lift), nor again while a protection runs (the match's
    // start respawning everyone just after they joined).
    let free = world.get::<God>(e).is_none() && world.get::<Protected>(e).is_none();
    if protection.any() && free {
        let now = world.resource::<SimClock>().now;
        let mut ent = world.entity_mut(e);
        ent.insert((
            SpawnProtected {
                damage_until: now + protection.damage as f64,
                bullets_until: now + protection.bullets as f64,
            },
            Protected {
                fire_ends: protection.fire_ends,
            },
        ));
        if protection.damage > 0.0 {
            ent.insert(God);
        }
    }
}

/// Where `e` comes back, as `points` picks among the map's spawn points
/// (None: none, it stays where it is).
pub fn spawn_point(world: &mut World, e: Entity, points: Points) -> Option<Transform> {
    let d = SpawnDef::default();
    spawn_point_in(world, e, points, d.min_distance, d.facing)
}

/// `spawn_point` with `random_hidden`'s least distance from an enemy (m)
/// and the way it faces.
pub fn spawn_point_in(
    world: &mut World,
    e: Entity,
    points: Points,
    min_distance: f32,
    facing: super::def::Facing,
) -> Option<Transform> {
    let mut all: Vec<(Entity, Transform, Option<Team>)> = world
        .query_filtered::<(Entity, &Transform, &SpawnPoint), Without<Intent>>()
        .iter(world)
        .map(|(p, t, s)| (p, *t, s.team))
        .collect();
    all.sort_by_key(|(p, ..)| *p);
    if all.is_empty() {
        return None;
    }
    let others: Vec<Vec3> = world
        .query_filtered::<(Entity, &Transform, &Health), (With<Intent>, Without<Dead>)>()
        .iter(world)
        .filter(|(o, _, h)| *o != e && h.current > 0.0)
        .map(|(_, t, _)| t.translation)
        .collect();
    let free = |t: &Transform| {
        others
            .iter()
            .all(|o| (o.xz() - t.translation.xz()).length() > SPAWN_CLEARANCE || (o.y - t.translation.y).abs() > 2.0)
    };
    let points = match points {
        Points::RandomHidden => match super::spawns::pick(world, e, min_distance, facing) {
            Some(at) => return Some(at),
            None => Points::FarFromEnemies,
        },
        p => p,
    };
    match points {
        Points::RandomHidden => unreachable!("handled above"),
        Points::Map => {
            let n = all.len();
            let start = world.get_resource_or_init::<SpawnCursor>().0;
            let pick = (0..n).map(|i| (start + i) % n).find(|i| free(&all[*i].1)).unwrap_or(start % n);
            world.resource_mut::<SpawnCursor>().0 = pick + 1;
            Some(all[pick].1)
        }
        Points::Team => {
            let team = world.get::<Team>(e).copied();
            let own: Vec<_> = all.iter().filter(|(_, _, t)| t.is_some() && *t == team).cloned().collect();
            let from = if own.is_empty() { all } else { own };
            from.iter().find(|(_, t, _)| free(t)).or(from.first()).map(|(_, t, _)| *t)
        }
        Points::FarFromEnemies => {
            let nearest = |t: &Transform| {
                others
                    .iter()
                    .map(|o| o.distance(t.translation))
                    .fold(f32::INFINITY, f32::min)
            };
            // The farthest from its nearest other player (a free one if
            // any is); ties: the first.
            let mut best: Option<(f32, Transform)> = None;
            let any_free = all.iter().any(|(_, t, _)| free(t));
            for (_, t, _) in all.iter().filter(|(_, t, _)| !any_free || free(t)) {
                let d = nearest(t);
                if best.is_none_or(|(b, _)| d > b + 1e-3) {
                    best = Some((d, *t));
                }
            }
            best.map(|(_, t)| t)
        }
    }
}

/// The kit (`SpawnDef::kit`) that goes with the rules' weapon `gun`: the
/// list, or `gun`'s game's.
/// A melee weapon in place of the kit's on this player (a ladder rung's,
/// `ladder::RungExtras::melee`): the kit's weapon of its slot (the
/// knife's) is this one; a kit with none in that slot gets none.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct KitMelee(pub &'static str);

/// `e`'s kit for a life whose rules weapon is `gun` (a kit by game: that
/// gun's game's), its weapon of `KitMelee`'s slot that one.
fn kit_for(world: &World, def: &MinigameDef, e: Entity, gun: Option<&str>) -> Vec<&'static str> {
    let empty = crate::weapon::data::WeaponCatalog::default();
    let catalog = world.get_resource::<crate::weapon::data::WeaponCatalog>().unwrap_or(&empty);
    let game = gun.and_then(|g| g.split_once(':')).map(|(g, _)| g);
    let swap = world.get::<KitMelee>(e).map(|m| m.0);
    let slot = |id: &str| catalog.get(id).map(|w| w.slot);
    let mut out: Vec<&'static str> = Vec::new();
    for id in def.spawn.kit.for_game(game).into_iter().filter_map(|id| super::weapon(def, catalog, id).ok()) {
        let id = match swap {
            Some(m) if slot(m).is_some() && slot(id) == slot(m) => m,
            _ => id,
        };
        if !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

/// `e`'s kit changed to go with its new rules weapon `gun` (the ladder's
/// swap): with a kit by game (`Kit::ByGame`), the other games' kit
/// weapons taken and the new game's given; a melee weapon in place of the
/// kit's (`KitMelee`) given, and the ones `also` names (the other rungs'
/// melee weapons) taken when not wanted now; all before the rules give
/// `gun`, which is then drawn. A kit list without either stays as it is.
pub fn refit_kit(world: &mut World, e: Entity, gun: &str, also: &[&'static str]) {
    let Some(def) = world.get_resource::<ActiveMinigame>().map(|a| a.def) else { return };
    if !matches!(def.spawn.kit, Kit::ByGame(_)) && world.get::<KitMelee>(e).is_none() && also.is_empty() {
        return;
    }
    let wanted = kit_for(world, def, e, Some(gun));
    let mut any = def.spawn.kit.all();
    any.extend(also.iter().copied());
    let have: Vec<(Entity, &'static str)> = world
        .get::<Inventory>(e)
        .map(|i| i.weapons.iter().filter_map(|w| world.get::<crate::weapon::Weapon>(*w).map(|x| (*w, x.id))).collect())
        .unwrap_or_default();
    for &(w, id) in &have {
        if any.contains(&id) && !wanted.contains(&id) {
            take_weapon(world, e, w);
        }
    }
    for id in wanted {
        if !have.iter().any(|(_, h)| *h == id) {
            give(world, e, id);
        }
    }
}

/// Take `weapon` out of `owner`'s inventory (and hands) and despawn it.
pub fn take_weapon(world: &mut World, owner: Entity, weapon: Entity) {
    if let Some(mut inv) = world.get_mut::<Inventory>(owner) {
        let inv = &mut *inv;
        inv.weapons.retain(|w| *w != weapon);
        for slot in [&mut inv.active, &mut inv.last, &mut inv.wanted] {
            if *slot == Some(weapon) {
                *slot = None;
            }
        }
    }
    if let Ok(w) = world.get_entity_mut(weapon) {
        w.despawn();
    }
}

/// A character the minigame may take in (`join`): in the game, not yet in
/// the minigame.
type Joining = (With<Intent>, With<Health>, Without<InMinigame>, Without<Connecting>, Without<Spectating>, Without<Npc>);

/// New characters into the minigame: a team as the mode makes them, at
/// once into play unless the match is over. The joiners are found by the
/// declared query; taking them in (the mode's team, the rules' `joined`,
/// the spawn) is world work, queued and applied at the sync point before
/// the phases step (`plugin`'s chain), in the query's order.
fn join(joining: Query<(Entity, &Team), Joining>, hold: Option<Res<super::MatchHold>>, mut commands: Commands) {
    if hold.is_some() {
        return;
    }
    let new: Vec<(Entity, u8)> = joining.iter().map(|(e, t)| (e, t.0)).filter(|(_, t)| *t != 255).collect();
    if new.is_empty() {
        return;
    }
    commands.queue(move |world: &mut World| take_in(world, &new));
}

/// `join`'s world part: each of `new` (with its team then) into the
/// minigame, in order.
fn take_in(world: &mut World, new: &[(Entity, u8)]) {
    let over = world.resource::<Match>().phase == Phase::MatchOver;
    for &(e, team) in new {
        let Ok(mut ent) = world.get_entity_mut(e) else { continue };
        if !plays(team) {
            ent.insert(Team(ALPHA));
        }
        ent.insert(InMinigame);
        mode::assign_team(world, e, false);
        if let Some(rules) = world.get_resource::<ActiveMinigame>().map(|a| a.rules) {
            (rules.joined)(world, e);
        }
        if !over {
            respawn(world, e);
        }
    }
}

/// The runtime's phases, once `players.min` are in.
fn step_phases(world: &mut World) {
    let Some(min) = world.get_resource::<ActiveMinigame>().map(|a| a.def.players.min) else { return };
    if super::held(world) {
        return;
    }
    if world.resource::<Match>().phase == Phase::Waiting && (players(world).len() as u32) < min {
        return;
    }
    mode::flow::mission_update(world);
}

/// When dead `e` comes back (`SimClock` s): its death plus the
/// definition's `delay`, while a minigame plays that respawns it (not held,
/// the match not over, the mode willing). A network client, which has no
/// mode of its own, goes by the definition alone. What `respawn_dead`
/// waits for and what the HUD's countdown shows (`hud::update`, into
/// `core::RespawnDue`).
pub fn respawn_due(world: &World, e: Entity) -> Option<f64> {
    let def = world.get_resource::<ActiveMinigame>()?.def;
    let dead = world.get::<Dead>(e)?;
    if super::held(world) || world.get_resource::<Match>().is_none_or(|m| m.phase == Phase::MatchOver) {
        return None;
    }
    if world.get::<Spectating>(e).is_some() {
        return None;
    }
    if let Some(mode) = current_mode(world)
        && (!mode.respawns || !(mode.hooks.may_respawn)(world, e))
    {
        return None;
    }
    Some(dead.since + def.spawn.respawn.delay as f64)
}

/// The players (`players`' filter) that are dead.
type DeadPlaying = (With<Intent>, With<Health>, With<InMinigame>, Without<Connecting>, Without<Spectating>, Without<Npc>);

/// The dead back after the definition's delay: the declared query finds
/// the players dead that long (not while held or the match is over); the
/// mode's say (`respawn_due`, whose `may_respawn` reads the world) and the
/// spawn are world work, queued and applied at the sync point after this
/// system (`plugin`'s chain), in join order as `players` gives them.
fn respawn_dead(
    dead: Query<(Entity, &Team, &Dead, Option<&JoinOrder>), DeadPlaying>,
    clock: Res<SimClock>,
    active: Option<Res<ActiveMinigame>>,
    game: Option<Res<Match>>,
    hold: Option<Res<super::MatchHold>>,
    mut commands: Commands,
) {
    let Some(active) = active else { return };
    if hold.is_some() || game.is_none_or(|m| m.phase == Phase::MatchOver) {
        return;
    }
    let (now, delay) = (clock.now, active.def.spawn.respawn.delay as f64);
    let mut due: Vec<(u64, Entity)> = dead
        .iter()
        .filter(|(_, t, d, _)| plays(t.0) && now + 1e-9 >= d.since + delay)
        .map(|(e, _, _, o)| (o.map_or(u64::MAX, |o| o.0), e))
        .collect();
    if due.is_empty() {
        return;
    }
    due.sort();
    commands.queue(move |world: &mut World| {
        if current_mode(world).is_none() {
            return;
        }
        let due: Vec<Entity> =
            due.into_iter().map(|(_, e)| e).filter(|e| respawn_due(world, *e).is_some_and(|at| now + 1e-9 >= at)).collect();
        for e in due {
            respawn(world, e);
        }
    });
}

/// Protection ends with its windows or (`fire_ends`) the first attack:
/// the fire key held (a gun, a knife's swing, a grenade's pin), or any
/// weapon's round, swing or throw (a knife's secondary stab); `God` goes
/// with the damage window. The removals apply at the sync point after
/// this system (`plugin`'s chain), before the tick's damage.
fn end_protection(
    mut cursor: Local<MessageCursor<crate::weapon::WeaponEvent>>,
    events: Option<Res<Messages<crate::weapon::WeaponEvent>>>,
    clock: Res<SimClock>,
    protected: Query<(Entity, &SpawnProtected, &Protected, &Intent, Has<God>)>,
    mut commands: Commands,
) {
    use crate::weapon::WeaponEventKind as K;
    let now = clock.now;
    let attacked: Vec<Entity> = match events {
        Some(events) => cursor
            .read(&events)
            .filter(|e| matches!(e.kind, K::Fired { .. } | K::Swing { .. } | K::Thrown | K::Shot { .. }))
            .map(|e| e.owner)
            .collect(),
        None => Vec::new(),
    };
    for (e, sp, p, i, god) in &protected {
        if (p.fire_ends && (i.fire || attacked.contains(&e))) || sp.over(now) {
            commands.entity(e).remove::<(Protected, SpawnProtected, God)>();
        } else if god && now + 1e-9 >= sp.damage_until {
            commands.entity(e).remove::<God>();
        }
    }
}

/// `e`'s spawn protection (the runtime's) gone, its `God` with it.
pub fn unprotect(world: &mut World, e: Entity) {
    if let Ok(mut ent) = world.get_entity_mut(e)
        && ent.contains::<Protected>()
    {
        ent.remove::<(Protected, SpawnProtected, God)>();
    }
}

/// The tick's deaths: marked, counted, the mode told.
fn handle_deaths(world: &mut World, mut cursor: Local<MessageCursor<Died>>) {
    let deaths: Vec<Died> = cursor.read(world.resource::<Messages<Died>>()).cloned().collect();
    for d in deaths {
        on_died(world, &d);
    }
}

fn on_died(world: &mut World, d: &Died) {
    let Some(mode) = current_mode(world) else { return };
    // What the killer held when it killed, for the hooks: the rules' swap
    // (a rung up) may take it before `died` runs (the ladder's melee
    // death reads it).
    let mut d = d.clone();
    if d.damage.weapon.is_none()
        && let Some(id) = d
            .attacker
            .and_then(|k| world.get::<Inventory>(k))
            .and_then(|i| i.active)
            .and_then(|w| world.get::<crate::weapon::Weapon>(w))
            .map(|w| w.id)
    {
        d.damage.weapon = Some(id);
    }
    let d = &d;
    let victim = d.entity;
    if world.get::<Intent>(victim).is_none() || world.get::<InMinigame>(victim).is_none() {
        return;
    }
    let now = world.resource::<SimClock>().now;
    unprotect(world, victim);
    world.entity_mut(victim).insert((Dead { since: now }, ColliderDisabled));
    let team_of = |w: &World, e: Entity| w.get::<Team>(e).map(|t| t.0);
    let Some(victim_team) = team_of(world, victim).filter(|t| plays(*t)) else { return };
    let fall = d.damage.kind == crate::core::DamageKind::Fall;
    let kind = classify(victim, d.attacker, d.attacker.and_then(|k| team_of(world, k)), victim_team, fall);
    if world.resource::<Match>().counting() {
        add_score(world, victim, 0, 1);
        if let Some(k) = d.attacker {
            match kind {
                DeathKind::Enemy => add_score(world, k, 1, 0),
                DeathKind::TeamKill | DeathKind::Suicide => add_score(world, k, -1, 0),
                DeathKind::NoKiller => {}
            }
        }
        let death = Death {
            victim,
            victim_team,
            killer: d.attacker,
            kind,
        };
        (mode.on_death)(world, &death);
    }
    (mode.hooks.died)(world, d);
}

fn add_score(world: &mut World, e: Entity, kills: i32, deaths: u32) {
    let Ok(mut ent) = world.get_entity_mut(e) else { return };
    let mut s = ent.get::<Score>().copied().unwrap_or_default();
    s.kills += kills;
    s.deaths += deaths;
    ent.insert(s);
}

/// The match as `minigame_status` prints it.
pub fn status(world: &mut World) -> Result<String, String> {
    let active = *world.get_resource::<ActiveMinigame>().ok_or("no minigame playing")?;
    let now = world.resource::<SimClock>().now;
    let m = world.resource::<Match>().clone();
    let left = m.round_ends.map(|e| format!(", {:.0} s left", (e - now).max(0.0))).unwrap_or_default();
    let mut lines = vec![format!(
        "{} ({}): {:?}{left}; winner {:?}; result {:?}",
        active.def.id, active.rules.mode.id, m.phase, m.winner, m.result
    )];
    for (e, team) in players(world) {
        let name = world.get::<Name>(e).map_or_else(|| e.to_string(), |n| n.as_str().to_string());
        let s = world.get::<Score>(e).copied().unwrap_or_default();
        let standing = super::ladder::standing(world, e).unwrap_or_default();
        lines.push(format!("  {name} team {team}: {} kills, {} deaths{standing}", s.kills, s.deaths));
    }
    Ok(lines.join("\n"))
}

/// The match's end froze everyone (`hold_the_end`), so the freeze lifts
/// with the end (here or at `minigame::restart`), not a game's own.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct EndFreeze;

/// What a player held by the match's end keeps of its intent: the look
/// (and the command's number); no moving, jumping, firing or weapon use.
pub fn end_frozen(i: &Intent) -> Intent {
    Intent {
        yaw: i.yaw,
        pitch: i.pitch,
        command: i.command,
        ..default()
    }
}

/// Whether the match's end holds everyone: from the win (the round over)
/// through the intermission, the match over and a map change's
/// `MatchHold`, until the next match.
pub fn end_holds(world: &World) -> bool {
    holds_at_end(super::held(world), world.get_resource::<Match>())
}

/// `end_holds` from the hold and the match.
fn holds_at_end(held: bool, game: Option<&Match>) -> bool {
    held || game.is_some_and(|m| matches!(m.phase, Phase::RoundOver { .. } | Phase::Intermission { .. } | Phase::MatchOver))
}

/// The match's end pauses the game for everyone (players and bots, the
/// server's rule, so a network client's prediction is held too through
/// `core::FreezeTime`): every intent kept to its look (`end_frozen`)
/// while `end_holds`; the freeze lifted when the next match plays.
/// `FreezeTime` and the intents are written in place (seen by the rest of
/// the tick); `EndFreeze` through commands.
fn hold_the_end(
    hold: Option<Res<super::MatchHold>>,
    game: Option<Res<Match>>,
    ours: Option<Res<EndFreeze>>,
    freeze: Option<ResMut<FreezeTime>>,
    mut intents: Query<&mut Intent>,
    mut commands: Commands,
) {
    if holds_at_end(hold.is_some(), game.as_deref()) {
        if ours.is_none() {
            commands.insert_resource(EndFreeze);
        }
        match freeze {
            Some(mut f) if !f.0 => f.0 = true,
            Some(_) => {}
            None => commands.insert_resource(FreezeTime(true)),
        }
        for mut i in &mut intents {
            *i = end_frozen(&i);
        }
    } else if ours.is_some() {
        commands.remove_resource::<EndFreeze>();
        match freeze {
            Some(mut f) => f.0 = false,
            None => commands.insert_resource(FreezeTime(false)),
        }
    }
}

/// The match's end freeze lifted (if it is on).
pub fn lift_end_freeze(world: &mut World) {
    if world.remove_resource::<EndFreeze>().is_some() {
        world.insert_resource(FreezeTime(false));
    }
}

/// The runtime's systems (the server's, while a minigame is the
/// gameplay's rules) and the join order.
pub(super) fn plugin(app: &mut App) {
    let on = |a: Option<Res<ActiveMinigame>>, g: Option<Res<chimera_core::gameplay::Gameplay>>| {
        a.is_some() && g.is_none_or(|g| g.rules == MINIGAME)
    };
    use crate::map::MapScopedApp;
    app.init_resource::<SpawnCursor>()
        .map_scoped::<super::spawns::SpawnSpots>()
        .add_observer(number_players)
        .add_observer(super::results::on_step)
        .add_systems(
            FixedUpdate,
            (
                (
                    join,
                    super::ladder::per_tick(),
                    step_phases,
                    respawn_dead,
                    crate::spawning::hold_the_dead,
                    end_protection,
                    super::wins::publish,
                    hold_the_end,
                )
                    .chain()
                    .in_set(SimSet::Rules),
                handle_deaths.after(crate::core::apply_damage),
            )
                .run_if(crate::core::authoritative)
                .run_if(on),
        );
}

fn number_players(add: On<Add<Team>>, mut next: Local<u64>, mut commands: Commands) {
    *next += 1;
    if let Ok(mut e) = commands.get_entity(add.entity) {
        e.try_insert(JoinOrder(*next));
    }
}
