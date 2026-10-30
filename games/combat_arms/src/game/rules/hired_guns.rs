//! Hired Guns (`combat_arms:hired_guns`, room key `SQM`;
//! specs/combat_arms/game_modes_2.md M11): Alpha against Bravo in rounds
//! of gold collection (300 s) after a warm-up, respawning; G rounds played
//! make the match. Bullion bars lie on the map, players carry them in a
//! pocket that slows them down and bank them in their team's bank; bank
//! money hires AI mercenaries (`mercs`).
//!
//! - Bars (M11.3): at each real round's setup the mode's drop points
//!   (`ObjectSpawnPoint`s of `GameType` `SquadMatch`) are shuffled and a
//!   bar of `bullion_amount` (100) is placed on each of the first
//!   min(`drop_bullion_num`, points) (80); taken with the use key while
//!   the player's view ray meets it within `pickup_reach` (200 u) of the
//!   player. A death in a real round's live phase drops the whole pocket
//!   as a bag at the body (raised by its half-height, then to the floor),
//!   taken by touch within the same reach; a leaver's pocket drops the
//!   same way in any phase. Anyone takes any item; AI never do. The
//!   objects are `objectives::CaObjectives::bullion` (every machine's).
//! - The pocket (M11.4, `objectives::CaCarry::bars`, `money`): no limit;
//!   its weight is the carry penalty (`HiredGunsData::penalty`) as the
//!   movement's custom term (`movement::CustomMovePenalty`), 0 once
//!   banked, dropped or cleared at a round start.
//! - Banks (M11.5): every tick a player's box overlaps its own team's
//!   `BullionSaveTrigger` with money in the pocket (live, real round), the
//!   whole pocket is banked: the player's bank (`CaBank`) and mission
//!   points + its money, the team's banked bars + its bars. When the
//!   banked bars of both teams make all the bars placed and nothing is
//!   left on the ground, the round asks to end.
//! - Bank money (M11.6, not reset between rounds): + `base_bank` (1000)
//!   at each real round's start (+ `round_win_bonus` (500) for the
//!   previous round's winners) and on joining a live real round; +
//!   `kill_bonus` (100) for an enemy player's or an AI's kill (a
//!   mercenary's kill pays its hirer); + `death_bonus` (0) at one's own
//!   death; − a hire's cost.
//! - Deaths (M11.8's table, live real rounds only): every player death
//!   the victim's team's round deaths +1, victim deaths +1, the death
//!   bonus; an enemy kill (a player's or its mercenary's) killer kills +1,
//!   the kill bonus, the killer's team's round kills +1; a suicide victim
//!   suicides +1; a team kill takes the victim's death back, killer
//!   friendly kills +1. A mercenary killed by a player: an enemy kill and
//!   the bonus for that player, a round kill for its team, no death for
//!   anyone.
//! - Round winner (at the round over when none was set): more banked
//!   bars; else more round kills; else fewer round deaths; else none (no
//!   round win, no bonus). At the round over the winners' mission points
//!   + the round-win bonus; every bar, bag and AI removed.
//! - Match end: after real round G, or when a team is empty: the other
//!   team; else more round wins; else Bravo.
//! - Leaving (M11.9): the leaver's mercenaries go, its pocket drops; the
//!   last player of a team leaving asks for the round's end (the winner by
//!   the rule above; the match end test gives the other team the match);
//!   a living leaver with no teammate alive gives the round to the other
//!   team (Quirk 5).
//! - Scoreboard: mission points, kills, fewer deaths (S13).
//!
//! Known shortcuts: a bar's box is 10 u each way and a bag's 16 (the
//! props' sizes aren't in the spec), the view ray isn't stopped by walls;
//! the "saved" event's sounds and effect, the way-points (target
//! triggers: state only, `objects`) and the chat warning (the HUD's objective
//! line says it) aren't there; no headshot statistic for a mercenary
//! kill (`CaStats` has none); the per-map mission file isn't read (Blood
//! Money's values are data/modes.toml's `[hired_guns]`).

use std::collections::HashMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::{
    super::{inventory, movement::CustomMovePenalty},
    CaMode, CaRuleSettings, HIRED_GUNS, Hooks, LeftPlayer, Leaver, NO_HOOKS, Panel, RoundTime, ScoreOrder, Spawns,
    TEAM_POINTS, Teams, flow, mercs,
    objectives::{self, Bullion, CaObjectives, Edits, Roster},
    objects::{CaMapObjects, ObjBox},
    scoring::stats,
    state::{ALPHA, BRAVO, CaMatch, CaStats, Death, DeathKind, Phase, enemy, side},
};
use crate::core::{Died, MovementState, Npc};

/// The drop points' and spawn points' game type (data/modes.toml's
/// `hired_guns.family`).
pub const FAMILY: &str = "SquadMatch";
/// A placed bar's and a bag's box half-extents (u).
pub const BAR_HALF: f32 = 10.0;
pub const BAG_HALF: f32 = 16.0;

/// The mode.
pub const MODE: CaMode = CaMode {
    rules: super::Mode {
        id: HIRED_GUNS,
        key: "SQM",
        shown_as: "Hired Guns",
        description: "Hired Guns: Alpha against Bravo, respawning; carry bullion bars to your bank in 300 s rounds, hire AI \
                      mercenaries with bank money; G rounds played",
        respawns: true,
        warmup: true,
        warmup_revives: true,
        forced_setup: None,
        teams: Teams::Two,
        on_death,
        time_out,
        match_end,
        hooks: Hooks {
            setup,
            round_over,
            warmup_end: match_end,
            ..NO_HOOKS
        },
    },
    ff_scaled: true,
    assists: true,
    timer_shown: true,
    spawns: Spawns {
        family: Some(FAMILY),
        ..TEAM_POINTS
    },
    round_time: RoundTime::Mission,
    fire_ends_protection: false,
    death_drops: true,
    panel: Panel::RoundWins,
    order: ScoreOrder::MissionPoints,
    leaving,
};

/// A player's bank money (M11.6); replicated (the hire menu's and the
/// HUD's).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaBank(pub i32);

/// The real round a player was last paid its base bank money for.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaPaid(pub u32);

/// The server's Hired Guns state.
#[derive(Resource, Clone, Debug, Default)]
pub struct CaHiredGuns {
    /// The next item's id.
    pub next_id: u32,
    /// The previous real round's winner.
    pub last_winner: Option<u8>,
    /// Each player's use key at the last tick (a press is its edge).
    pub prev_use: HashMap<Entity, bool>,
}

/// Add `amount` to `e`'s bank (never below 0).
pub fn pay(world: &mut World, e: Entity, amount: i32) {
    let Ok(mut ent) = world.get_entity_mut(e) else { return };
    let now = ent.get::<CaBank>().map_or(0, |b| b.0);
    ent.insert(CaBank((now + amount).max(0)));
}

/// A bonus: bank and mission points.
fn bonus(world: &mut World, e: Entity, amount: i32) {
    if amount == 0 {
        return;
    }
    pay(world, e, amount);
    stats(world, e).mission_points += amount;
}

/// `e`'s pocket's weight as its custom move penalty.
fn weigh(world: &mut World, e: Entity) {
    let bars = objectives::carry(world, e).bars;
    let p = world.resource::<CaRuleSettings>().hired_guns.penalty(bars);
    let Ok(mut ent) = world.get_entity_mut(e) else { return };
    if p == 0.0 {
        ent.remove::<CustomMovePenalty>();
    } else if ent.get::<CustomMovePenalty>() != Some(&CustomMovePenalty(p)) {
        ent.insert(CustomMovePenalty(p));
    }
}

/// Every team's bars in pockets.
fn count_pockets(world: &mut World) {
    let mut n = [0u32; 2];
    for (e, t) in flow::players(world) {
        if let Some(i) = side(t) {
            n[i] += objectives::carry(world, e).bars;
        }
    }
    if world.resource::<CaObjectives>().in_pockets != n {
        world.resource_mut::<CaObjectives>().in_pockets = n;
    }
}

/// Put an item on the ground (a bag falls to the floor below `at`).
fn place(world: &mut World, at: Vec3, bars: u32, money: i32, bag: bool) {
    let at = if bag { objectives::floor_below(world, at) } else { at };
    let id = {
        let mut h = world.resource_mut::<CaHiredGuns>();
        h.next_id += 1;
        h.next_id
    };
    world.resource_mut::<CaObjectives>().bullion.push(Bullion { id, at, bars, money, bag });
}

/// `e`'s whole pocket dropped as a bag at `at` (its centre; raised by the
/// half-height, then to the floor); the pocket emptied.
pub fn drop_pocket(world: &mut World, e: Option<Entity>, at: Vec3, carry: objectives::CaCarry) {
    if carry.money <= 0 {
        return;
    }
    let half = objectives::player_half().y;
    place(world, at + Vec3::Y * half, carry.bars, carry.money, true);
    if let Some(e) = e {
        let mut c = objectives::carry(world, e);
        c.bars = 0;
        c.money = 0;
        objectives::set_carry(world, e, c);
        weigh(world, e);
    }
    count_pockets(world);
}

/// The round's setup: pockets cleared (every round); in a real round the
/// bank pay and the bars.
fn setup(world: &mut World) {
    {
        let mut o = world.resource_mut::<CaObjectives>();
        o.bullion.clear();
        o.banked = [0; 2];
        o.placed = 0;
        o.in_pockets = [0; 2];
    }
    let weighed: Vec<Entity> = world.query_filtered::<Entity, With<CustomMovePenalty>>().iter(world).collect();
    for e in weighed {
        world.entity_mut(e).remove::<CustomMovePenalty>();
    }
    let (warmup, round) = {
        let m = world.resource::<CaMatch>();
        (m.warmup, m.round)
    };
    if warmup {
        return;
    }
    let h = world.resource::<CaRuleSettings>().hired_guns.clone();
    // The objective's `AIDamageFactor` becomes the console variable's.
    if let Some(mut ai) = world.get_resource_mut::<super::super::weapons::CaAiDamage>() {
        ai.factor = h.ai_damage_factor;
    }
    let winner = world.resource::<CaHiredGuns>().last_winner;

    for (e, team) in flow::players(world) {
        let win = if winner == Some(team) { h.round_win_bonus } else { 0 };
        pay(world, e, h.base_bank + win);
        world.entity_mut(e).insert(CaPaid(round));
    }
    let mut points: Vec<Vec3> = world
        .resource::<CaMapObjects>()
        .drops
        .iter()
        .filter(|d| d.family == h.family)
        .map(|d| d.at)
        .collect();
    objectives::shuffle(world, &mut points);
    let n = (h.drop_bullion_num as usize).min(points.len());
    for at in points.into_iter().take(n) {
        place(world, at, 1, h.bullion_amount, false);
    }
    world.resource_mut::<CaObjectives>().placed = n as u32;
}

/// Joining a live real round: the base pay (the mode's per-tick system,
/// `rules::mode_updates`).
pub(super) fn update(
    m: Res<CaMatch>,
    settings: Res<CaRuleSettings>,
    roster: Roster,
    paid: Query<&CaPaid>,
    mut banks: Query<&mut CaBank>,
    mut commands: Commands,
) {
    if !m.counting() {
        return;
    }
    let base = settings.hired_guns.base_bank;
    let round = m.round;
    let mut bank = Edits::default();
    for (e, _) in roster.players() {
        if paid.get(e).is_ok_and(|p| p.0 == round) {
            continue;
        }
        pay_in(&mut bank, &banks, e, base);
        if let Ok(mut ec) = commands.get_entity(e) {
            ec.try_insert(CaPaid(round));
        }
    }
    bank.apply(&mut banks, &mut commands);
}

/// `pay` in a system.
fn pay_in(bank: &mut Edits<CaBank>, banks: &Query<&mut CaBank>, e: Entity, amount: i32) {
    let now = bank.get(banks, e).map_or(0, |b| b.0);
    bank.set(e, Some(CaBank((now + amount).max(0))));
}

/// What `touches` writes back as it goes.
#[derive(Default)]
struct Pockets {
    carry: Edits<objectives::CaCarry>,
    bank: Edits<CaBank>,
    weight: Edits<CustomMovePenalty>,
}

/// Pick-ups, banking, the early end, after the tick's deaths (the mode's
/// per-tick touches, `rules::mode_touches`).
#[allow(clippy::too_many_arguments)]
pub(super) fn touches(
    settings: Res<CaRuleSettings>,
    objects: Res<CaMapObjects>,
    mut state: ResMut<CaHiredGuns>,
    mut o: ResMut<CaObjectives>,
    mut m: ResMut<CaMatch>,
    roster: Roster,
    eyes: Query<&MovementState>,
    mut carries: Query<&mut objectives::CaCarry>,
    mut banks: Query<&mut CaBank>,
    mut weights: Query<&mut CustomMovePenalty>,
    mut stats: Query<&mut CaStats>,
    mut commands: Commands,
) {
    if !m.counting() {
        state.prev_use.clear();
        return;
    }
    let h = &settings.hired_guns;
    let reach = objectives::units(h.pickup_reach);
    let half = objectives::player_half();
    let mut banked_now = false;
    let mut edits = Pockets::default();
    let weigh = |edits: &mut Pockets, carries: &Query<&mut objectives::CaCarry>, weights: &Query<&mut CustomMovePenalty>, e: Entity| {
        let bars = objectives::carry_in(&edits.carry, carries, e).bars;
        let p = h.penalty(bars);
        let now = edits.weight.get(weights, e);
        if p == 0.0 {
            if now.is_some() {
                edits.weight.set(e, None);
            }
        } else if now != Some(CustomMovePenalty(p)) {
            edits.weight.set(e, Some(CustomMovePenalty(p)));
        }
    };
    for p in roster.takers() {
        let pressed = {
            let before = state.prev_use.insert(p.e, p.intent.use_key).unwrap_or(false);
            p.intent.use_key && !before
        };
        let mut taken: Vec<u32> = Vec::new();
        // A bar by use: the nearest the view ray meets.
        if pressed {
            let eye = p.at + eyes.get(p.e).map_or(Vec3::ZERO, |s| s.eye_offset);
            let dir = crate::weapon::aim_of(p.intent.yaw, p.intent.pitch) * Vec3::NEG_Z;
            let bar = objectives::units(BAR_HALF);
            let aimed = o
                .bullion
                .iter()
                .filter(|b| !b.bag && b.at.distance(p.at) <= reach + 1e-5)
                .filter_map(|b| inventory::ray_box(eye, dir, b.at - Vec3::splat(bar), b.at + Vec3::splat(bar)).map(|d| (d, b.id)))
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, id)| id);
            taken.extend(aimed);
        }
        for id in taken {
            take_in(&mut o, &mut edits, &carries, p.e, id);
            weigh(&mut edits, &carries, &weights, p.e);
        }
        // Bags by touch.
        let bag = objectives::units(BAG_HALF);
        let touched: Vec<u32> = o
            .bullion
            .iter()
            .filter(|b| b.bag && b.at.distance(p.at) <= reach + 1e-5)
            .filter(|b| ObjBox { centre: b.at, half: Vec3::splat(bag) }.touches(p.at, half))
            .map(|b| b.id)
            .collect();
        for id in touched {
            take_in(&mut o, &mut edits, &carries, p.e, id);
            weigh(&mut edits, &carries, &weights, p.e);
        }
        // The bank.
        let carry = objectives::carry_in(&edits.carry, &carries, p.e);
        if carry.money > 0 {
            let in_bank = objects
                .banks
                .iter()
                .any(|b| b.team.is_none_or(|t| t == p.team) && b.area.touches(p.at, half));
            if in_bank {
                // M11.5: the whole pocket into the bank.
                pay_in(&mut edits.bank, &banks, p.e, carry.money);
                objectives::add_stats(&mut stats, &mut commands, p.e, move |s| {
                    s.mission_points += carry.money;
                    s.bars_banked += carry.bars;
                });
                if let Some(i) = side(p.team) {
                    o.banked[i] += carry.bars;
                }
                let mut c = carry;
                c.bars = 0;
                c.money = 0;
                objectives::set_carry_in(&mut edits.carry, p.e, c);
                weigh(&mut edits, &carries, &weights, p.e);
                info!("team {} banked {} bars", p.team, carry.bars);
                banked_now = true;
            }
        }
    }
    // Every team's bars in pockets.
    let mut n = [0u32; 2];
    for (e, t) in roster.players() {
        if let Some(i) = side(t) {
            n[i] += objectives::carry_in(&edits.carry, &carries, e).bars;
        }
    }
    if o.in_pockets != n {
        o.in_pockets = n;
    }
    if banked_now && o.banked[0] + o.banked[1] >= o.placed && o.bullion.is_empty() && !m.decided() {
        m.end_requested = true;
    }
    edits.carry.apply(&mut carries, &mut commands);
    edits.bank.apply(&mut banks, &mut commands);
    edits.weight.apply(&mut weights, &mut commands);
}

/// `take` in a system: `e` takes item `id` whole (`touches` weighs it).
fn take_in(o: &mut ResMut<CaObjectives>, edits: &mut Pockets, carries: &Query<&mut objectives::CaCarry>, e: Entity, id: u32) {
    let Some(i) = o.bullion.iter().position(|b| b.id == id) else { return };
    let item = o.bullion.remove(i);
    let mut c = objectives::carry_in(&edits.carry, carries, e);
    c.bars += item.bars;
    c.money += item.money;
    objectives::set_carry_in(&mut edits.carry, e, c);
}

/// A death in any phase: the pocket drops as a bag in a real round's live
/// phase.
fn died(world: &mut World, d: &Died) {
    if world.get::<Npc>(d.entity).is_some() || !world.resource::<CaMatch>().counting() {
        return;
    }
    let carry = objectives::carry(world, d.entity);
    let at = world.get::<Transform>(d.entity).map_or(Vec3::ZERO, |t| t.translation);
    drop_pocket(world, Some(d.entity), at, carry);
}

/// A player's pocket dropped where it died (live real rounds): the mode's
/// `mode::ModeDeath` observer (plan ecs-cleanup step 5), its world work
/// queued and applied by the death step's flush, so where the mode's hook
/// ran.
pub(super) fn on_mode_death(death: On<crate::mode::ModeDeath>, mut commands: Commands) {
    const MODES: &[&str] = &[HIRED_GUNS];
    if !MODES.contains(&death.mode) {
        return;
    }
    let d = death.died.clone();
    commands.queue(move |world: &mut World| died(world, &d));
}

/// M11.8's counts (live real rounds).
fn on_death(world: &mut World, d: &Death) {
    let h = world.resource::<CaRuleSettings>().hired_guns.clone();
    if let Some(i) = side(d.victim_team) {
        world.resource_mut::<CaMatch>().round_deaths[i] += 1;
    }
    stats(world, d.victim).deaths += 1;
    bonus(world, d.victim, h.death_bonus);
    match d.kind {
        DeathKind::Enemy => {
            if let Some(k) = d.killer {
                stats(world, k).kills += 1;
                bonus(world, k, h.kill_bonus);
            }
            if let Some(i) = side(enemy(d.victim_team)) {
                world.resource_mut::<CaMatch>().round_kills[i] += 1;
            }
        }
        DeathKind::Suicide => stats(world, d.victim).suicides += 1,
        DeathKind::TeamKill => {
            {
                let mut s = stats(world, d.victim);
                s.deaths = s.deaths.saturating_sub(1);
            }
            if let Some(k) = d.killer {
                stats(world, k).friendly_kills += 1;
            }
        }
        DeathKind::NoKiller => {}
    }
}

/// A mercenary killed by player `killer` (M11.7): an enemy kill, the kill
/// bonus, a round kill for its team (live real rounds).
pub fn merc_killed(world: &mut World, killer: Entity, _head: bool) {
    if !world.resource::<CaMatch>().counting() || super::current_mode(world).is_none_or(|m| m.key != MODE.key) {
        return;
    }
    let kill_bonus = world.resource::<CaRuleSettings>().hired_guns.kill_bonus;
    stats(world, killer).kills += 1;
    bonus(world, killer, kill_bonus);
    super::scoring::sync_score(world, killer);
    let team = world.get::<crate::core::Team>(killer).map_or(0, |t| t.0);
    if let Some(i) = side(team) {
        world.resource_mut::<CaMatch>().round_kills[i] += 1;
    }
}

/// Alpha when `a` is greater, Bravo when `b` is, None when equal.
fn pick(a: i64, b: i64) -> Option<u8> {
    super::ctf::pick(a, b)
}

/// More banked bars; more round kills; fewer round deaths; else none.
fn time_out(world: &mut World) {
    let banked = world.resource::<CaObjectives>().banked;
    let mut m = world.resource_mut::<CaMatch>();
    m.winner = pick(banked[0] as i64, banked[1] as i64)
        .or_else(|| pick(m.round_kills[0] as i64, m.round_kills[1] as i64))
        .or_else(|| pick(m.round_deaths[1] as i64, m.round_deaths[0] as i64));
}

/// The round over: the winners' bonus, every item and AI removed.
fn round_over(world: &mut World) {
    let (winner, warmup) = {
        let m = world.resource::<CaMatch>();
        (m.winner, m.warmup)
    };
    world.resource_mut::<CaHiredGuns>().last_winner = if warmup { None } else { winner };
    if !warmup && let Some(w) = winner {
        let bonus_amount = world.resource::<CaRuleSettings>().hired_guns.round_win_bonus;
        for (e, t) in flow::players(world) {
            if t == w {
                stats(world, e).mission_points += bonus_amount;
            }
        }
    }
    world.resource_mut::<CaObjectives>().bullion.clear();
    mercs::remove_all(world);
}

/// After real round G, or a team empty: the other team; more round wins;
/// else Bravo.
fn match_end(world: &World) -> Option<Option<u8>> {
    let goal = world.resource::<CaRuleSettings>().goal();
    let m = world.resource::<CaMatch>();
    let counts = world.resource::<super::CaTeamCounts>().0;
    let empty = counts.contains(&0);
    if !empty && m.round < goal {
        return None;
    }
    let winner = if counts[0] == 0 && counts[1] > 0 {
        BRAVO
    } else if counts[1] == 0 && counts[0] > 0 {
        ALPHA
    } else if m.round_wins[0] > m.round_wins[1] {
        ALPHA
    } else {
        BRAVO
    };
    Some(Some(winner))
}

/// M11.9: the last of a team asks for the round's end; a living leaver
/// with no teammate alive gives the round away.
fn leaving(m: &mut CaMatch, l: &Leaver) {
    if m.phase != Phase::Live || m.warmup || m.decided() {
        return;
    }
    if l.team_players <= 1 {
        m.end_requested = true;
    } else if l.alive && l.team_alive <= 1 {
        m.win(enemy(l.team));
    }
}

/// A leaver's pocket drops (any phase) and its mercenaries go (the
/// mode's part of `rules::on_leave`).
pub(super) fn left(world: &mut World, l: &LeftPlayer) {
    mercs::remove_of(world, l.entity);
    drop_pocket(world, None, l.at, l.carry);
    world.resource_mut::<CaHiredGuns>().prev_use.remove(&l.entity);
}
