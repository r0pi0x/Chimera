//! VIP Escort (`combat_arms:vip_escort`, room key `ESC`; clan VIP Escort
//! `combat_arms:clan_vip_escort`, `CESC`; specs/combat_arms/game_modes_2.md
//! M13): the escort team brings its VIP, a slow player with a pistol, to
//! the exit, which opens a minute into each round; the elimination team
//! stops it. Rounds without respawn after a warm-up; the roles swap every
//! ⌊R / 2⌋ rounds, each swap followed by a new warm-up.
//!
//! - Roles: the escort team starts as Alpha, the elimination team as
//!   Bravo (`CaObjectives::escort`); a swap changes the roles only (team
//!   ids, round wins and statistics stay). The escort side always spawns
//!   at the map's `JSF` points of the `VIPEscort` family (`point_team`).
//! - The VIP (M13.2, US build), chosen at each real round's mission start
//!   (`choose_vip`) from every player in join order sorted by: escort team
//!   first, not the previous VIP first, fewer turns as VIP, more score
//!   (VIP escapes, VIP kills, enemy kills, assists), a random rank. A
//!   warm-up has no VIP. The VIP (`CaVip`): the mission's weapons only
//!   (`escort.vip_weapons`, `weapon::OwnKit`), no pick-ups
//!   (`inventory::NoPickups`), its model's move penalty alone
//!   (`movement::VipMovement`, 0.44), 100 health and armour; it spawns at
//!   an escort point and is moved to the map's `VipTeleportPoint` (its
//!   rotation, on the floor; none: it stays).
//! - The exit (M13.5): the `EscapeZoneTrigger` named by the mission, shut
//!   until `exit_time` (60 s) after each real round's setup
//!   (`CaObjectives::exit`). The VIP, alive, on the escort team, touching
//!   it while the round is live, undecided and the round clock (read raw:
//!   Quirk 7) not run out: the escort wins.
//! - Results (M13.6's table): the escape, the VIP's death, the last
//!   elimination player's death and the VIP leaving alive decide the round
//!   with everyone frozen and immune (`CaMatch::immune`) and a 3 s hold
//!   (`end_hold`) before the round over; the last escort player's death
//!   and a death leaving both teams empty decide it frozen with no hold;
//!   the last living player of a team leaving (not the VIP) and the clock
//!   at 0 decide it unfrozen with the hold. During a hold kills still
//!   count; a VIP death restarts it; the winner stays.
//! - Deaths: enemy kill, killer kills +1, victim deaths +1, the killer's
//!   team's round kills +1, the killer's VIP kills +1 when the victim was
//!   the VIP; suicide, victim deaths and suicides +1; team kill, killer
//!   friendly kills +1; no killer, victim deaths +1. Assists as S7 (none
//!   for the VIP's death: Quirk of M13.4).
//! - Match end, after each real round: a team with R round wins (clan: the
//!   two teams' wins adding up to R) or an empty team; more round wins,
//!   else Bravo.
//! - Side swap (M13.7): after real round k when k is a multiple of
//!   T = ⌊R / 2⌋ and the match goes on: the intermission, a `swap_wait`
//!   (3 s) wait, the swap with a `swap_notice` (3 s) countdown
//!   (`CaObjectives::swap_notice`), then a warm-up, then round k + 1; the
//!   spawn cursors reset. At each round over the HUD gets the rounds until
//!   the swap (T + 1 − (k mod T, T when 0); 255 after the last swap).
//! - Joiners are held until the next round with 3 or more players in a
//!   live round (S1). Friendly fire is always off (no room option).
//!
//! Known shortcuts: the VIP keeps its own model (the VIP model 87 isn't
//! drawn) and its weapon is our M92FS for the G18 (no G18 in our data);
//! the VIP's cell door and its two use triggers (13 s and 1 s) are the
//! map's objects (`objects`), but the rounds' start and end triggers that
//! re-close it come with the mission data (not read yet), so a cell
//! opened stays open for the next rounds; the exit's helicopter, markers, messages and camera, and the satellite
//! scan aren't there; the exit timer starts with the round's setup only
//! (not also with the map's round start trigger); the VIP's perks and
//! vote-kick protection aren't modelled; the swap's notice ends where the
//! warm-up's setup starts (its countdown is the HUD's). Clan VIP Escort
//! (not enabled in any room data, M13.8) plays the public rules but its
//! match end; its spawn family and the reference's unapplied side swap
//! aren't reproduced.

use std::ops::DerefMut;

use bevy::{ecs::message::MessageMutator, prelude::*};
use serde::{Deserialize, Serialize};

use super::{
    super::{inventory::NoPickups, movement::VipMovement},
    CLAN_VIP_ESCORT, CaMode, CaRuleSettings, Hooks, Leaver, NO_HOOKS, Panel, RoundTime, ScoreOrder, Spawns,
    TEAM_POINTS, Teams, VIP_ESCORT,
    flow::{alive, players},
    objectives::{self, CaObjectives, Exit},
    objects::CaMapObjects,
    scoring::stats,
    spawns::CaSpawnCursors,
    state::{ALPHA, BRAVO, CaMatch, CaStats, Death, DeathKind, Phase, enemy, side},
};
use crate::{
    core::{Damage, FreezeTime, Health, Npc, SimClock, Team},
    weapon::OwnKit,
};

/// The points' and the exit's game type.
pub const FAMILY: &str = "VIPEscort";
/// "None" for the rounds until the swap.
pub const NO_SWAP: u8 = 255;

/// The mode.
pub const MODE: CaMode = CaMode {
    rules: super::Mode {
        id: VIP_ESCORT,
        key: "ESC",
        shown_as: "VIP Escort",
        description: "VIP Escort: the escort team brings its VIP to the exit, the elimination team stops it; rounds without \
                      respawn, roles swapping every R/2 rounds; R round wins take the match",
        respawns: false,
        warmup: true,
        warmup_revives: true,
        forced_setup: None,
        teams: Teams::Two,
        on_death,
        time_out: |w| {
            let e = elimination_team(w);
            w.resource_mut::<CaMatch>().winner = Some(e);
        },
        match_end,
        hooks: Hooks {
            before_spawns,
            setup,
            round_over,
            after_round,
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
    order: ScoreOrder::Kills,
    leaving,
};

/// Clan VIP Escort (M13.8): the public rules, R rounds in all.
pub const CLAN: CaMode = CaMode {
    rules: super::Mode {
        id: CLAN_VIP_ESCORT,
        key: "CESC",
        shown_as: "Clan VIP Escort",
        description: "Clan VIP Escort: VIP Escort's rules, R rounds in all (not enabled in any room data)",
        match_end: clan_match_end,
        ..MODE.rules
    },
    ..MODE
};

/// A player is the VIP (replicated: the HUD's, a marker over it).
#[derive(Component, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CaVip;

/// A VIP's own kit, kept while it is the VIP (the server's).
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct CaKeptKit(pub Option<OwnKit>);

/// The server's VIP Escort state.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct CaEscort {
    /// The roles are swapped (Bravo escorts).
    pub swapped: bool,
    /// The previous real round's VIP.
    pub previous: Option<Entity>,
    /// Turns as VIP this match.
    pub turns: Vec<(Entity, u32)>,
    /// The swap happens then (game seconds).
    pub swap_at: Option<f64>,
}

impl CaEscort {
    fn turns(&self, e: Entity) -> u32 {
        self.turns.iter().find(|(x, _)| *x == e).map_or(0, |(_, n)| *n)
    }
}

/// The escort team now.
pub fn escort_team(world: &World) -> u8 {
    if world.get_resource::<CaEscort>().is_some_and(|s| s.swapped) { BRAVO } else { ALPHA }
}

/// The elimination team now.
pub fn elimination_team(world: &World) -> u8 {
    enemy(escort_team(world))
}

/// Whose start points a player of `team` uses (M13.1: the escort side the
/// map's `JSF` points); other modes: its own.
pub fn point_team(world: &World, team: u8) -> u8 {
    let escort = super::current_mode(world).is_some_and(|m| m.key == MODE.key || m.key == CLAN.key);
    if escort && world.get_resource::<CaEscort>().is_some_and(|s| s.swapped) && side(team).is_some() {
        enemy(team)
    } else {
        team
    }
}

/// The swap period T = ⌊R / 2⌋ (at least 1).
pub fn swap_period(goal: u32) -> u32 {
    (goal / 2).max(1)
}

/// The rounds until the swap the HUD gets at real round `k`'s round over
/// (M13.7): T + 1 − (k mod T, T when 0); 255 once the last swap is done.
pub fn rounds_to_swap(k: u32, goal: u32) -> u8 {
    let t = swap_period(goal);
    let last = (2 * goal).saturating_sub(1) / t;
    if k == 0 || (k - 1) / t >= last {
        return NO_SWAP;
    }
    let m = if k % t == 0 { t } else { k % t };
    (t + 1 - m) as u8
}

/// One player's entry in the VIP choice (M13.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VipEntry {
    pub escort: bool,
    pub previous: bool,
    pub turns: u32,
    pub score: u32,
    pub rank: u32,
}

/// The VIP among `entries` (US build): escort team first, not the
/// previous VIP first, fewer turns, more score, higher random rank.
pub fn choose_vip(entries: &[VipEntry]) -> Option<usize> {
    (0..entries.len()).min_by(|&a, &b| {
        let (x, y) = (&entries[a], &entries[b]);
        y.escort
            .cmp(&x.escort)
            .then(x.previous.cmp(&y.previous))
            .then(x.turns.cmp(&y.turns))
            .then(y.score.cmp(&x.score))
            .then(y.rank.cmp(&x.rank))
    })
}

/// A player's VIP score (M13.2): VIP escapes, VIP kills, enemy kills,
/// assists (sub-missions: none).
pub fn vip_score(s: &CaStats) -> u32 {
    s.vip_escapes + s.vip_kills + s.kills + s.assists
}

/// Make `e` the VIP: the mission's weapons, no pick-ups, its move
/// penalty.
fn make_vip(world: &mut World, e: Entity) {
    let s = world.resource::<CaRuleSettings>().escort.clone();
    let registry = world.resource::<crate::weapon::WeaponRegistry>();
    let ids: Vec<&'static str> = s.vip_weapons.iter().filter_map(|id| registry.find(id).map(|d| d.id)).collect();
    let kept = world.get::<OwnKit>(e).cloned();
    world.entity_mut(e).insert((
        CaVip,
        CaKeptKit(kept),
        OwnKit(ids),
        NoPickups,
        VipMovement {
            penalty: s.vip_move_penalty,
        },
    ));
}

/// `e` is no longer the VIP: its own kit back (at its next respawn).
fn unmake_vip(world: &mut World, e: Entity) {
    let Ok(mut ent) = world.get_entity_mut(e) else { return };
    let kept = ent.get::<CaKeptKit>().cloned();
    ent.remove::<(CaVip, NoPickups, VipMovement, CaKeptKit, OwnKit)>();
    if let Some(CaKeptKit(Some(kit))) = kept {
        ent.insert(kit);
    }
}

/// The VIP now, if any.
pub fn vip(world: &mut World) -> Option<Entity> {
    world.query_filtered::<Entity, With<CaVip>>().iter(world).next()
}

/// The mission start, before everyone respawns: a warm-up clears the VIP,
/// a real round chooses one (M13.2).
fn before_spawns(world: &mut World) {
    world.init_resource::<CaEscort>();
    let current = vip(world);
    let warmup = world.resource::<CaMatch>().warmup;
    if warmup {
        if let Some(v) = current {
            unmake_vip(world, v);
        }
        world.resource_mut::<CaEscort>().previous = None;
        return;
    }
    let escort = escort_team(world);
    // The VIP is a human player (bots_objectives.md 2.3).
    let list: Vec<(Entity, u8)> = players(world)
        .into_iter()
        .filter(|(e, _)| world.get::<crate::core::Bot>(*e).is_none())
        .collect();
    let mut ranks: Vec<u32> = (0..list.len() as u32).collect();
    objectives::shuffle(world, &mut ranks);
    let state = world.resource::<CaEscort>().clone();
    let entries: Vec<VipEntry> = list
        .iter()
        .zip(ranks)
        .map(|((e, team), rank)| VipEntry {
            escort: *team == escort,
            previous: state.previous == Some(*e),
            turns: state.turns(*e),
            score: world.get::<CaStats>(*e).map_or(0, vip_score),
            rank,
        })
        .collect();
    let Some(i) = choose_vip(&entries) else { return };
    let chosen = list[i].0;
    if let Some(v) = current.filter(|v| *v != chosen) {
        unmake_vip(world, v);
    }
    if current != Some(chosen) {
        make_vip(world, chosen);
    }
    let mut s = world.resource_mut::<CaEscort>();
    s.previous = Some(chosen);
    match s.turns.iter_mut().find(|(x, _)| *x == chosen) {
        Some((_, n)) => *n += 1,
        None => s.turns.push((chosen, 1)),
    }
    info!("VIP: {chosen}");
}

/// The round's setup, everyone respawned: the VIP to its teleport point,
/// the exit timer (real rounds).
fn setup(world: &mut World) {
    world.init_resource::<CaEscort>();
    let escort = escort_team(world);
    let warmup = world.resource::<CaMatch>().warmup;
    let s = world.resource::<CaRuleSettings>().escort.clone();
    {
        let mut o = world.resource_mut::<CaObjectives>();
        o.escort = escort;
        // Its timer starts with the live phase (`update`).
        o.exit = (!warmup).then_some(Exit {
            opens: f64::INFINITY,
            open: false,
        });
        o.swap_notice = None;
    }
    let at = world.resource::<CaMapObjects>().named(&s.vip_spawn).next().map(|p| p.at);
    if let Some(v) = vip(world)
        && let Some(at) = at
    {
        crate::spawning::revive(world, v, Some(at));
    }
}

/// Decide the round for `winner`: everyone frozen and immune when
/// `freeze`, held `end_hold` s when `hold`.
fn decide(world: &mut World, winner: u8, freeze: bool, hold: bool) {
    let now = world.resource::<SimClock>().now;
    let wait = world.resource::<CaRuleSettings>().escort.end_hold as f64;
    world.resource_scope::<FreezeTime, _>(|world, mut frozen| {
        decide_in(&mut frozen, &mut world.resource_mut::<CaMatch>(), now, wait, winner, freeze, hold);
    });
}

/// `decide` on the resources (`wait`: the escort's `end_hold`).
fn decide_in(
    frozen: &mut impl DerefMut<Target = FreezeTime>,
    m: &mut impl DerefMut<Target = CaMatch>,
    now: f64,
    wait: f64,
    winner: u8,
    freeze: bool,
    hold: bool,
) {
    if freeze {
        frozen.0 = true;
        m.immune = true;
    }
    if hold {
        m.win_after(winner, now, wait);
    } else {
        m.win(winner);
    }
    info!("VIP Escort round to team {winner}");
}

/// The modes playing VIP Escort's per-tick systems (`update`, `touches`).
pub const RUNS_IN: &[&str] = &[VIP_ESCORT, CLAN_VIP_ESCORT];

/// The escort team as `CaEscort` says.
fn escort_of(state: &CaEscort) -> u8 {
    if state.swapped { BRAVO } else { ALPHA }
}

/// The swap, the exit's opening, the time-out (the mode's per-tick
/// system, `rules::mode_updates`).
#[allow(clippy::too_many_arguments)]
pub(super) fn update(
    clock: Res<SimClock>,
    settings: Res<CaRuleSettings>,
    mut state: ResMut<CaEscort>,
    mut cursors: ResMut<CaSpawnCursors>,
    mut o: ResMut<CaObjectives>,
    mut m: ResMut<CaMatch>,
    mut frozen: ResMut<FreezeTime>,
) {
    let now = clock.now;
    // The side swap (M13.7).
    if state.swap_at.is_some_and(|t| now + 1e-9 >= t) {
        let notice = settings.escort.swap_notice as f64;
        state.swap_at = None;
        state.swapped = !state.swapped;
        *cursors = CaSpawnCursors::default();
        let escort = escort_of(&state);
        o.escort = escort;
        o.swap_notice = Some(now + notice);
        info!("VIP Escort sides swapped: team {escort} escorts");
    }
    // The exit's timer from the round's start (the setup of 0 s is the
    // live phase's first frame in the reference: M13.5's "120 s left").
    if let Some(x) = o.exit
        && x.opens.is_infinite()
        && m.phase == Phase::Live
    {
        let exit_time = settings.escort.exit_time as f64;
        let length = settings.mode_data().time_limit.unwrap_or(0.0) as f64;
        let start = m.round_ends.map_or(now, |e| e - length);
        o.exit = Some(Exit {
            opens: start + exit_time,
            ..x
        });
    }
    // The exit opens.
    if let Some(x) = o.exit
        && !x.open
        && now + 1e-9 >= x.opens
        && m.phase == Phase::Live
    {
        o.exit = Some(Exit { open: true, ..x });
        info!("the exit is open");
    }
    // The clock at 0: the elimination team, unfrozen, after the hold.
    if m.counting() && !m.decided() && m.round_ends.is_some_and(|e| now + 1e-9 >= e) {
        let e = enemy(escort_of(&state));
        let wait = settings.escort.end_hold as f64;
        decide_in(&mut frozen, &mut m, now, wait, e, false, true);
    }
}

/// The escape, after the tick's deaths (the mode's per-tick touches,
/// `rules::mode_touches`).
#[allow(clippy::too_many_arguments)]
pub(super) fn touches(
    clock: Res<SimClock>,
    settings: Res<CaRuleSettings>,
    objects: Res<CaMapObjects>,
    state: Res<CaEscort>,
    o: Res<CaObjectives>,
    mut m: ResMut<CaMatch>,
    mut frozen: ResMut<FreezeTime>,
    vips: Query<(Entity, Option<&Team>, Option<&Health>, Has<crate::core::Dead>, Option<&Transform>), With<CaVip>>,
    mut stats: Query<&mut CaStats>,
    mut commands: Commands,
) {
    let now = clock.now;
    if !m.counting() || m.decided() {
        return;
    }
    // The escape: the raw clock (Quirk 7).
    let open = o.exit.is_some_and(|x| x.open);
    let running = m.round_ends.is_some_and(|e| now + 1e-9 < e);
    let escort = escort_of(&state);
    if open
        && running
        && let Some((v, team, health, dead, t)) = vips.iter().next()
        && team.is_some_and(|t| t.0 == escort)
        && health.is_some_and(|h| h.current > 0.0)
        && !dead
        && let Some(at) = t.map(|t| t.translation)
        && escape_zone(&settings, &objects).is_some_and(|z| z.touches(at, objectives::player_half()))
    {
        objectives::add_stats(&mut stats, &mut commands, v, |s| s.vip_escapes += 1);
        let wait = settings.escort.end_hold as f64;
        decide_in(&mut frozen, &mut m, now, wait, escort, true, true);
    }
}

/// The exit's box: the zone the mission names, else the family's first,
/// else the map's first.
fn escape_zone(settings: &CaRuleSettings, objects: &CaMapObjects) -> Option<super::objects::ObjBox> {
    let name = &settings.escort.escape_zone;
    let zones = &objects.escape_zones;
    zones
        .iter()
        .find(|z| z.name.eq_ignore_ascii_case(name))
        .or_else(|| zones.iter().find(|z| z.family == FAMILY))
        .or_else(|| zones.first())
        .map(|z| z.area)
}

/// M13.6's deaths.
fn on_death(world: &mut World, d: &Death) {
    let was_vip = world.get::<CaVip>(d.victim).is_some();
    match d.kind {
        DeathKind::Enemy => {
            if let Some(k) = d.killer {
                let mut s = stats(world, k);
                s.kills += 1;
                if was_vip {
                    s.vip_kills += 1;
                }
            }
            stats(world, d.victim).deaths += 1;
            if let Some(i) = side(enemy(d.victim_team)) {
                world.resource_mut::<CaMatch>().round_kills[i] += 1;
            }
        }
        DeathKind::Suicide => {
            let mut s = stats(world, d.victim);
            s.deaths += 1;
            s.suicides += 1;
        }
        DeathKind::TeamKill => {
            if let Some(k) = d.killer {
                stats(world, k).friendly_kills += 1;
            }
        }
        DeathKind::NoKiller => stats(world, d.victim).deaths += 1,
    }
    if let Some(i) = side(d.victim_team) {
        world.resource_mut::<CaMatch>().round_deaths[i] += 1;
    }
    let now = world.resource::<SimClock>().now;
    if world.resource::<CaMatch>().decided() {
        // A VIP death during the hold restarts its message.
        if was_vip && world.resource::<CaMatch>().hold_until.is_some() {
            let wait = world.resource::<CaRuleSettings>().escort.end_hold as f64;
            world.resource_mut::<CaMatch>().hold_until = Some(now + wait);
        }
        return;
    }
    let (escort, elim) = (escort_team(world), elimination_team(world));
    let (a, b) = (alive(world, escort), alive(world, elim));
    if a == 0 && b == 0 {
        decide(world, elim, true, false);
    } else if was_vip {
        decide(world, elim, true, true);
    } else if b == 0 {
        decide(world, escort, true, true);
    } else if a == 0 {
        decide(world, elim, true, false);
    }
}

/// M13.6's leavers: the VIP alive gives the round to the elimination team
/// (frozen, held); the last living player of a team gives it to the other
/// (unfrozen, held).
fn leaving(m: &mut CaMatch, l: &Leaver) {
    if m.phase != Phase::Live || m.warmup || m.decided() {
        return;
    }
    // The escort team is the VIP's (a VIP is on it at its choice).
    if l.vip && l.alive {
        m.immune = true;
        m.win_after(enemy(l.team), l.now, 3.0);
    } else if l.alive && l.team_alive <= 1 {
        m.win_after(enemy(l.team), l.now, 3.0);
    }
}

/// The rounds until the swap, at a real round's round over.
fn round_over(world: &mut World) {
    let (warmup, k) = {
        let m = world.resource::<CaMatch>();
        (m.warmup, m.round + 1)
    };
    if warmup {
        return;
    }
    let goal = world.resource::<CaRuleSettings>().goal();
    world.resource_mut::<CaObjectives>().rounds_to_swap = rounds_to_swap(k, goal);
    world.resource_mut::<CaObjectives>().exit = None;
}

/// After real round k: when k is a multiple of T and the match goes on,
/// the swap after the intermission and a wait, then a warm-up.
fn after_round(world: &mut World) {
    let m = world.resource::<CaMatch>().clone();
    let real = m.results.last().is_some_and(|r| !r.warmup);
    let t = swap_period(world.resource::<CaRuleSettings>().goal());
    if !real || m.result.is_some() || m.round == 0 || m.round % t != 0 {
        return;
    }
    let Phase::Intermission { until } = m.phase else { return };
    let s = world.resource::<CaRuleSettings>().escort.clone();
    world.init_resource::<CaEscort>();
    world.resource_mut::<CaEscort>().swap_at = Some(until + s.swap_wait as f64);
    let mut m = world.resource_mut::<CaMatch>();
    m.phase = Phase::Intermission {
        until: until + (s.swap_wait + s.swap_notice) as f64,
    };
    m.warmup_next = true;
}

/// R round wins or an empty team; more wins, else Bravo.
fn match_end(world: &World) -> Option<Option<u8>> {
    let goal = world.resource::<CaRuleSettings>().goal();
    let m = world.resource::<CaMatch>();
    let counts = world.resource::<super::CaTeamCounts>().0;
    if !counts.contains(&0) && m.round_wins.iter().all(|w| *w < goal) {
        return None;
    }
    Some(Some(if m.round_wins[0] > m.round_wins[1] { ALPHA } else { BRAVO }))
}

/// Clan: the two teams' wins add up to R, or an empty team.
fn clan_match_end(world: &World) -> Option<Option<u8>> {
    let goal = world.resource::<CaRuleSettings>().goal();
    let m = world.resource::<CaMatch>();
    let counts = world.resource::<super::CaTeamCounts>().0;
    if !counts.contains(&0) && m.round_wins.iter().sum::<u32>() < goal {
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

/// A decided round's immunity: no damage reaches a player (the server's).
pub(super) fn immunity(mut damage: MessageMutator<Damage>, npcs: Query<(), With<Npc>>, m: Res<CaMatch>) {
    if !m.immune {
        return;
    }
    for d in damage.read() {
        if !npcs.contains(d.target) {
            d.amount = 0.0;
        }
    }
}

/// A fresh match: the roles back, nobody VIP.
pub fn reset(world: &mut World) {
    let vips: Vec<Entity> = world.query_filtered::<Entity, With<CaVip>>().iter(world).collect();
    for v in vips {
        unmake_vip(world, v);
    }
    world.insert_resource(CaEscort::default());
}
