//! The objectives' state the modes share (specs/combat_arms/game_modes.md
//! M3, M4, M7, M9, M10): what the HUD and every machine see of them
//! (`CaObjectives`: the planted bomb, the flags, the flag pole, items on
//! the ground; the server's, a network client's from `game::net`), what a
//! player carries (`CaCarry`) and the action gauge it is filling
//! (`CaAction`: plant, defuse, seize), and the helpers the modes use:
//! touch tests against the player's box, the floor under a dropped item,
//! giving and taking objective weapons, random draws; and the same for a
//! mode's per-tick systems, without the world: the players from declared
//! queries (`Roster`), writes read back in the same run (`Edits`), the
//! `_in` helpers, weapons given through `Commands` (`give_weapon_later`).
//!
//! Known shortcuts: a player's box is the fallback hull (15, 50, 15) about
//! its position (as S4's occupied test); the floor under a dropped item is
//! the first thing a ray straight down meets (not characters), else the
//! drop point; the draws (drop points, hosts) come from a counter through
//! a hash, not the game's random generator.

use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::{
    ecs::{
        component::Mutable,
        system::{SystemParam, SystemState},
    },
    prelude::*,
};
use serde::{Deserialize, Serialize};

use super::{
    flow,
    spawns::HULL,
    state::{CaMatch, CaStats},
};
use crate::{
    core::{Bot, Dead, Health, Intent, SimClock, Team},
    weapon::{Inventory, Weapon},
};

/// The mission bomb (`XK-2`, inventory class 6; data/weapons/xk2.toml).
pub const BOMB: &str = "combat_arms:xk-2";

/// Where a flag is (M4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum FlagAt {
    #[default]
    Home,
    /// Carried by a player of the other team (`CaCarry::flag`).
    Carried,
    /// On the ground at `at`, going home at `returns` (game seconds).
    Dropped { at: Vec3, returns: f64 },
}

/// A team's flag.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Flag {
    pub team: u8,
    pub home: Vec3,
    pub at: FlagAt,
}

/// A planted bomb (M3, M9).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Planted {
    /// The site's index in `CaMapObjects::bomb_zones`, and its number
    /// (0 A, 1 B).
    pub zone: usize,
    pub number: i32,
    pub at: Vec3,
    /// The planting team.
    pub team: u8,
    /// When it detonates (game seconds).
    pub detonates: f64,
}

/// Seize and Secure's flag pole (M10).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Pole {
    /// The team whose flag rises; None: nobody holds it.
    pub team: Option<u8>,
    /// When its flag started from the bottom (game seconds).
    pub since: f64,
    /// Whole seconds already scored for this hold.
    pub scored: u32,
}

/// Hired Guns' bullion on the ground (game_modes_2.md M11.3): a placed
/// bar (taken with the use key) or a dropped bag (taken by touch).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Bullion {
    pub id: u32,
    pub at: Vec3,
    pub bars: u32,
    pub money: i32,
    /// A dropped pocket (else a placed bar).
    pub bag: bool,
}

/// VIP Escort's exit (game_modes_2.md M13.5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Exit {
    /// It opens then (game seconds) ...
    pub opens: f64,
    /// ... and is open.
    pub open: bool,
    /// The helicopter starts to arrive then (the exit timer's in-time
    /// command) ...
    pub arrives: f64,
    /// ... and is arriving.
    pub arriving: bool,
}

/// The objectives as every machine sees them.
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CaObjectives {
    /// The bomb planted this round.
    pub planted: Option<Planted>,
    /// Bombing Run's bomb on the ground.
    pub loose_bomb: Option<Vec3>,
    /// Capture the flag: Alpha's and Bravo's (empty in other modes).
    pub flags: Vec<Flag>,
    /// Seize and Secure.
    pub pole: Option<Pole>,
    /// Spy Hunt's intel on the ground.
    pub intel: Vec<Vec3>,
    /// Spy Hunt's stage (1, 2; 0 before a round).
    pub stage: u8,
    /// The bomb that went off this round: the site's centre and when (game
    /// seconds), for its explosion show (game_modes.md S15).
    pub detonation: Option<(Vec3, f64)>,
    /// The bomb defused this round: where (its defused sound).
    pub defused: Option<Vec3>,
    /// Spy Hunt's stage-2 camera hold: when it ends (game seconds; every
    /// view on the map camera until then, M7).
    pub camera_until: Option<f64>,
    /// Hired Guns: the bars and bags on the ground, bars banked this round
    /// (Alpha, Bravo), bars placed this round, bars in each team's
    /// pockets.
    pub bullion: Vec<Bullion>,
    pub banked: [u32; 2],
    pub placed: u32,
    pub in_pockets: [u32; 2],
    /// VIP Escort: the escort team (0: none), the exit, the side swap's
    /// notice (its countdown's end), the rounds until the swap shown at a
    /// round over (255: none).
    pub escort: u8,
    pub exit: Option<Exit>,
    pub swap_notice: Option<f64>,
    pub rounds_to_swap: u8,
    /// The clan modes: the half (1, 2; 0 none).
    pub half: u8,
}

impl CaObjectives {
    /// Every time in it moved by `f` (a network client's clock).
    pub fn map_times(&mut self, f: impl Fn(f64) -> f64) {
        if let Some(p) = &mut self.planted {
            p.detonates = f(p.detonates);
        }
        for flag in &mut self.flags {
            if let FlagAt::Dropped { returns, .. } = &mut flag.at {
                *returns = f(*returns);
            }
        }
        if let Some(p) = &mut self.pole {
            p.since = f(p.since);
        }
        if let Some((_, t)) = &mut self.detonation {
            *t = f(*t);
        }
        if let Some(t) = &mut self.camera_until {
            *t = f(*t);
        }
        if let Some(x) = &mut self.exit {
            x.opens = f(x.opens);
        }
        if let Some(n) = &mut self.swap_notice {
            *n = f(*n);
        }
    }

    /// Bars of placed (level) bars still on the ground, and in bags.
    pub fn bars_on_ground(&self) -> (u32, u32) {
        let level = self.bullion.iter().filter(|b| !b.bag).map(|b| b.bars).sum();
        let bags = self.bullion.iter().filter(|b| b.bag).map(|b| b.bars).sum();
        (level, bags)
    }

    /// `team`'s flag.
    pub fn flag(&self, team: u8) -> Option<&Flag> {
        self.flags.iter().find(|f| f.team == team)
    }
}

/// What a player carries (replicated: the HUD's badge, a flag on a back).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaCarry {
    /// The flag of this team (capture the flag).
    pub flag: Option<u8>,
    /// Bombing Run's bomb.
    pub bomb: bool,
    /// Spy Hunt's intel items.
    pub intel: u8,
    /// Hired Guns' pocket: bars and their money (game_modes_2.md M11.4).
    pub bars: u32,
    pub money: i32,
}

impl CaCarry {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// What an action gauge is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionKind {
    Plant,
    Defuse,
    Seize,
}

/// The action gauge a player is filling (S&D's 4 s plant and 7 s defuse,
/// Seize and Secure's 5 s seize): replicated for its HUD.
#[derive(Component, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CaAction {
    pub kind: ActionKind,
    /// Seconds held so far, of `length`.
    pub held: f32,
    pub length: f32,
}

impl CaAction {
    /// The gauge's fill, 0 to 1.
    pub fn fraction(&self) -> f32 {
        if self.length > 0.0 { (self.held / self.length).clamp(0.0, 1.0) } else { 1.0 }
    }
}

/// A living player in the match: its position (the box's centre), team,
/// intent.
#[derive(Clone, Debug)]
pub struct Living {
    pub e: Entity,
    pub team: u8,
    pub at: Vec3,
    pub intent: Intent,
}

/// The living players in the match, in the server's list order.
pub fn living(world: &mut World) -> Vec<Living> {
    let order = flow::players(world);
    let mut out = Vec::new();
    for (e, team) in order {
        let Some(h) = world.get::<Health>(e) else { continue };
        if h.current <= 0.0 || world.get::<Dead>(e).is_some() || world.get::<super::spawns::CaHeld>(e).is_some() {
            continue;
        }
        let Some(t) = world.get::<Transform>(e) else { continue };
        out.push(Living {
            e,
            team,
            at: t.translation,
            intent: world.get::<Intent>(e).cloned().unwrap_or_default(),
        });
    }
    out
}

/// The living players who may take, carry or use an objective (a flag, a
/// bomb, intel, a seize trigger, bullion, a VIP's exit): the human ones
/// (bots_objectives.md 1, Quirk 1: no AI plays an objective; our bots,
/// which play every mode, leave them alone).
pub fn takers(world: &mut World) -> Vec<Living> {
    living(world)
        .into_iter()
        .filter(|p| world.get::<crate::core::Bot>(p.e).is_none())
        .collect()
}

/// A player's box half-extents (engine metres): the fallback hull.
pub fn player_half() -> Vec3 {
    HULL * super::super::inventory::UNIT
}

/// World units to metres.
pub fn units(u: f32) -> f32 {
    u * super::super::inventory::UNIT
}

/// What `e` carries (empty when nothing).
pub fn carry(world: &World, e: Entity) -> CaCarry {
    world.get::<CaCarry>(e).copied().unwrap_or_default()
}

/// Set what `e` carries.
pub fn set_carry(world: &mut World, e: Entity, c: CaCarry) {
    let Ok(mut ent) = world.get_entity_mut(e) else { return };
    if c.is_empty() {
        ent.remove::<CaCarry>();
    } else if ent.get::<CaCarry>() != Some(&c) {
        ent.insert(c);
    }
}

/// The floor under `at` (the first thing straight down within 100 m that
/// isn't a character), else `at`.
pub fn floor_below(world: &mut World, at: Vec3) -> Vec3 {
    let characters: Vec<Entity> = world.query_filtered::<Entity, With<Intent>>().iter(world).collect();
    let mut state: SystemState<SpatialQuery> = SystemState::new(world);
    let Ok(query) = state.get(world) else { return at };
    let filter = SpatialQueryFilter::default().with_excluded_entities(characters);
    match query.cast_ray(at, Dir3::NEG_Y, 100.0, true, &filter) {
        Some(hit) => at - Vec3::Y * hit.distance,
        None => at,
    }
}

/// `e` holds `id` in its list.
pub fn has_weapon(world: &World, e: Entity, id: &str) -> Option<Entity> {
    let inv = world.get::<Inventory>(e)?;
    inv.weapons
        .iter()
        .copied()
        .find(|w| world.get::<Weapon>(*w).is_some_and(|w| w.id == id))
}

/// `e` has `id` in its hand.
pub fn holding(world: &World, e: Entity, id: &str) -> bool {
    world
        .get::<Inventory>(e)
        .and_then(|i| i.active)
        .and_then(|w| world.get::<Weapon>(w))
        .is_some_and(|w| w.id == id)
}

/// Give `e` the weapon `id` without drawing it (an objective weapon at a
/// round's setup); once its loadout is there (`Inventory`).
pub fn give_weapon(world: &mut World, e: Entity, id: &str) -> Option<Entity> {
    if let Some(w) = has_weapon(world, e, id) {
        return Some(w);
    }
    let wanted = world.get::<Inventory>(e)?.wanted;
    let w = crate::weapon::give(world, e, id)?;
    if let Some(mut inv) = world.get_mut::<Inventory>(e) {
        inv.wanted = wanted;
    }
    Some(w)
}

/// Take the weapon `id` from `e` (the bomb planted or removed): out of its
/// list and gone; the first weapon drawn if it was in hand.
pub fn take_weapon(world: &mut World, e: Entity, id: &str) -> bool {
    let Some(w) = has_weapon(world, e, id) else { return false };
    if let Some(mut inv) = world.get_mut::<Inventory>(e) {
        inv.weapons.retain(|x| *x != w);
        if inv.active == Some(w) {
            inv.active = None;
            inv.wanted = inv.weapons.first().copied();
        }
        if inv.wanted == Some(w) {
            inv.wanted = inv.weapons.first().copied();
        }
        if inv.last == Some(w) {
            inv.last = None;
        }
    }
    if let Ok(ent) = world.get_entity_mut(w) {
        ent.despawn();
    }
    true
}

/// A draw in 0..n from the match's counter (`CaDraws`).
pub fn draw(world: &mut World, n: usize) -> usize {
    draw_in(&mut world.get_resource_or_init::<CaDraws>(), n)
}

/// `list` shuffled by the match's counter.
pub fn shuffle<T>(world: &mut World, list: &mut [T]) {
    shuffle_in(&mut world.get_resource_or_init::<CaDraws>(), list);
}

/// The counter the modes' draws run on.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct CaDraws(pub u64);

/// SplitMix64.
pub fn hash(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Fill or reset `e`'s action gauge of `kind` (`length` s) while `on`;
/// true the tick it fills.
pub fn hold_action(world: &mut World, e: Entity, kind: ActionKind, length: f32, on: bool) -> bool {
    let dt = world.resource::<SimClock>().dt() as f32;
    let Ok(mut ent) = world.get_entity_mut(e) else { return false };
    if !on {
        if ent.get::<CaAction>().is_some_and(|a| a.kind == kind) {
            ent.remove::<CaAction>();
        }
        return false;
    }
    let held = match ent.get::<CaAction>() {
        Some(a) if a.kind == kind => a.held + dt,
        _ => 0.0,
    };
    if held + 1e-6 >= length {
        ent.remove::<CaAction>();
        return true;
    }
    ent.insert(CaAction { kind, held, length });
    false
}

/// Every player's carried items and gauges cleared (a round's end, a new
/// match).
pub fn clear_players(world: &mut World) {
    let with: Vec<Entity> = world
        .query_filtered::<Entity, Or<(With<CaCarry>, With<CaAction>)>>()
        .iter(world)
        .collect();
    for e in with {
        world.entity_mut(e).remove::<(CaCarry, CaAction)>();
    }
}

/// The objectives back to nothing (a new match, a mode without them).
pub fn reset(world: &mut World) {
    *world.resource_mut::<CaObjectives>() = CaObjectives::default();
    clear_players(world);
}

/// Team counters' helpers for the modes: the match's enemy kills and
/// deaths per team.
pub fn count_team_kill(world: &mut World, team: u8) {
    if let Some(i) = super::state::side(team) {
        world.resource_mut::<CaMatch>().kills[i] += 1;
    }
}

/// `e`'s statistics, made when missing.
pub fn stats_of(world: &mut World, e: Entity) -> Mut<'_, CaStats> {
    super::scoring::stats(world, e)
}

// ---- The same helpers for a mode's per-tick systems (declared queries,
// no world; plan ecs-cleanup step 4). Structural changes go through
// `Commands`, applied at the sync point after the modes' systems, before
// the phases step (O1); what a system writes and reads back in the same
// run is kept in an `Edits` until then.

/// The match's players and their state for a system: `flow::players`,
/// `living`, `takers` and `flow::alive` from declared queries.
#[derive(SystemParam)]
pub struct Roster<'w, 's> {
    players: Query<'w, 's, (Entity, &'static Team, Option<&'static flow::CaJoinOrder>), flow::Playing>,
    states: Query<
        'w,
        's,
        (
            &'static Health,
            Has<Dead>,
            Has<super::spawns::CaHeld>,
            Option<&'static Transform>,
            Option<&'static Intent>,
            Has<Bot>,
        ),
    >,
}

impl Roster<'_, '_> {
    /// `flow::players`: on a team that plays, in join order.
    pub fn players(&self) -> Vec<(Entity, u8)> {
        flow::in_join_order(self.players.iter())
    }

    /// `e` has health left and isn't dead (held or not).
    pub fn breathing(&self, e: Entity) -> bool {
        self.states.get(e).is_ok_and(|(h, dead, ..)| h.current > 0.0 && !dead)
    }

    fn up(&self, e: Entity) -> bool {
        self.states
            .get(e)
            .is_ok_and(|(h, dead, held, ..)| h.current > 0.0 && !dead && !held)
    }

    /// `living`: the living players in the match, in order.
    pub fn living(&self) -> Vec<Living> {
        self.players()
            .into_iter()
            .filter(|(e, _)| self.up(*e))
            .filter_map(|(e, team)| {
                let (_, _, _, t, intent, _) = self.states.get(e).ok()?;
                Some(Living {
                    e,
                    team,
                    at: t?.translation,
                    intent: intent.cloned().unwrap_or_default(),
                })
            })
            .collect()
    }

    /// `takers`: the living human players.
    pub fn takers(&self) -> Vec<Living> {
        self.living()
            .into_iter()
            .filter(|p| !self.states.get(p.e).is_ok_and(|s| s.5))
            .collect()
    }

    /// `flow::alive`: the living players of `team`, teams as `team_of`
    /// says (a system's team changes not yet applied).
    pub fn alive_as(&self, team: u8, team_of: impl Fn(Entity, u8) -> u8) -> usize {
        self.players()
            .into_iter()
            .filter(|(e, t)| team_of(*e, *t) == team && self.up(*e))
            .count()
    }
}

/// Component writes a system reads back in the same run (an `Edits` per
/// component type): `get` sees them, `apply` writes them at its end
/// (in place where the component is there and differs, else inserted or
/// removed through `Commands`).
pub struct Edits<T>(Vec<(Entity, Option<T>)>);

impl<T> Default for Edits<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T: Component<Mutability = Mutable> + Copy + PartialEq> Edits<T> {
    /// `e`'s value, the edits first.
    pub fn get(&self, q: &Query<&mut T>, e: Entity) -> Option<T> {
        match self.0.iter().rev().find(|(x, _)| *x == e) {
            Some((_, v)) => *v,
            None => q.get(e).ok().copied(),
        }
    }

    /// Set (Some) or remove (None) `e`'s value.
    pub fn set(&mut self, e: Entity, v: Option<T>) {
        self.0.push((e, v));
    }

    /// Write the edits: each entity's last, in the order first edited.
    pub fn apply(self, q: &mut Query<&mut T>, commands: &mut Commands) {
        let mut done: Vec<Entity> = Vec::new();
        for (e, _) in &self.0 {
            if done.contains(e) {
                continue;
            }
            done.push(*e);
            let last = self.0.iter().rev().find(|(x, _)| x == e).and_then(|(_, v)| *v);
            match (last, q.get_mut(*e)) {
                (Some(v), Ok(mut cur)) => {
                    if *cur != v {
                        *cur = v;
                    }
                }
                (Some(v), Err(_)) => {
                    if let Ok(mut ec) = commands.get_entity(*e) {
                        ec.try_insert(v);
                    }
                }
                (None, Ok(_)) => {
                    if let Ok(mut ec) = commands.get_entity(*e) {
                        ec.try_remove::<T>();
                    }
                }
                (None, Err(_)) => {}
            }
        }
    }
}

/// `carry` in a system (empty when nothing).
pub fn carry_in(edits: &Edits<CaCarry>, q: &Query<&mut CaCarry>, e: Entity) -> CaCarry {
    edits.get(q, e).unwrap_or_default()
}

/// `set_carry` in a system.
pub fn set_carry_in(edits: &mut Edits<CaCarry>, e: Entity, c: CaCarry) {
    edits.set(e, (!c.is_empty()).then_some(c));
}

/// `hold_action` in a system (`dt`: the tick's seconds).
pub fn hold_action_in(
    edits: &mut Edits<CaAction>,
    q: &Query<&mut CaAction>,
    dt: f32,
    e: Entity,
    kind: ActionKind,
    length: f32,
    on: bool,
) -> bool {
    let current = edits.get(q, e);
    if !on {
        if current.is_some_and(|a| a.kind == kind) {
            edits.set(e, None);
        }
        return false;
    }
    let held = match current {
        Some(a) if a.kind == kind => a.held + dt,
        _ => 0.0,
    };
    if held + 1e-6 >= length {
        edits.set(e, None);
        return true;
    }
    edits.set(e, Some(CaAction { kind, held, length }));
    false
}

/// `f` on `e`'s statistics in a system: in place, else made through
/// `Commands` (as `scoring::stats` makes them).
pub fn add_stats(
    stats: &mut Query<&mut CaStats>,
    commands: &mut Commands,
    e: Entity,
    f: impl FnOnce(&mut CaStats) + Send + 'static,
) {
    match stats.get_mut(e) {
        Ok(mut s) => f(&mut s),
        Err(_) => commands.queue(move |world: &mut World| {
            if world.get_entity(e).is_ok() {
                f(&mut super::scoring::stats(world, e));
            }
        }),
    }
}

/// `has_weapon` in a system.
pub fn has_weapon_in(inv: Option<&Inventory>, weapons: &Query<&Weapon>, id: &str) -> Option<Entity> {
    inv?.weapons
        .iter()
        .copied()
        .find(|w| weapons.get(*w).is_ok_and(|w| w.id == id))
}

/// `holding` in a system.
pub fn holding_in(inv: Option<&Inventory>, weapons: &Query<&Weapon>, id: &str) -> bool {
    inv.and_then(|i| i.active)
        .and_then(|w| weapons.get(w).ok())
        .is_some_and(|w| w.id == id)
}

/// `take_weapon` in a system: out of the list at once, the weapon
/// despawned through `Commands`.
pub fn take_weapon_in(inv: Option<&mut Inventory>, weapons: &Query<&Weapon>, commands: &mut Commands, id: &str) -> bool {
    let Some(inv) = inv else { return false };
    let Some(w) = has_weapon_in(Some(inv), weapons, id) else { return false };
    inv.weapons.retain(|x| *x != w);
    if inv.active == Some(w) {
        inv.active = None;
        inv.wanted = inv.weapons.first().copied();
    }
    if inv.wanted == Some(w) {
        inv.wanted = inv.weapons.first().copied();
    }
    if inv.last == Some(w) {
        inv.last = None;
    }
    if let Ok(mut ec) = commands.get_entity(w) {
        ec.try_despawn();
    }
    true
}

/// `give_weapon` from a system: building a weapon is world work, queued
/// (`Commands`), in order with the system's other commands.
pub fn give_weapon_later(commands: &mut Commands, e: Entity, id: &'static str) {
    commands.queue(move |world: &mut World| {
        give_weapon(world, e, id);
    });
}

/// `floor_below` in a system (`characters`: every `Intent` holder, left
/// out of the ray).
pub fn floor_below_in(spatial: Option<&SpatialQuery>, characters: &Query<Entity, With<Intent>>, at: Vec3) -> Vec3 {
    let Some(query) = spatial else { return at };
    let filter = SpatialQueryFilter::default().with_excluded_entities(characters.iter());
    match query.cast_ray(at, Dir3::NEG_Y, 100.0, true, &filter) {
        Some(hit) => at - Vec3::Y * hit.distance,
        None => at,
    }
}

/// `draw` on the counter itself (a system's `ResMut<CaDraws>`).
pub fn draw_in(draws: &mut CaDraws, n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    draws.0 = draws.0.wrapping_add(1);
    (hash(draws.0) % n as u64) as usize
}

/// `shuffle` on the counter itself.
pub fn shuffle_in<T>(draws: &mut CaDraws, list: &mut [T]) {
    for i in (1..list.len()).rev() {
        let j = draw_in(draws, i + 1);
        list.swap(i, j);
    }
}
