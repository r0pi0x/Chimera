//! Quarantine Regen (`combat_arms:quarantine`, room key `QRT`;
//! specs/combat_arms/game_modes.md M8): infection rounds. Everyone starts
//! human (Alpha); `host_delay` (20 s) after a round goes live, H living
//! humans become infected hosts (Bravo); the infected spread it. A warm-up
//! first, then G rounds played of `TimeLimit` 3 minutes.
//!
//! - Hosts: H = 1 with N <= 3 players on teams, else max(round(N x
//!   `SelectionRate`), `MinHostNum`) (`hosts`), chosen at random among
//!   the living humans, only in real rounds (never in the warm-up); with H
//!   or fewer humans alive then, the round is void (no winner).
//! - Infected (`infect`): on Bravo, `movement::CaInfected` (S14's
//!   knock-back), the mission's infected weapons (a host's: the host's;
//!   stock 191 and 190, our documents for them with the V1 key), else our
//!   knife; after `WaitingTIme` (3 s) standing exactly still they heal
//!   `Heal` (0.1) of their maximum hit points every `TickTime` (1 s), the
//!   first at once when the wait ends; a move, a hit or firing restarts
//!   the wait (Open question 12, answered).
//! - Deaths: S7's personal counts (`scoring::personal`) and the mission
//!   points (`[ExpAndGPSetting]`): a human killing an infected +3, the
//!   infected killed -1; an infected killing a human +2, the human 0, and
//!   that human is back at once where it died (on the floor below), at
//!   full health, infected (M8).
//! - Round win: no human alive, the infected; after the selection, no
//!   infected alive, the humans. Time-out: the humans. The living members
//!   of the winning side get +10 (humans) or +5 (infected) at the round's
//!   end. Match end: after G rounds played, or with one player or fewer
//!   on a team (after the warm-up too), with no winner (players rank by
//!   mission points). A leaver who leaves one player or fewer ends the
//!   round (no side wins). The setup is one frame (Corrections
//!   2026-10-22, 16). Joiners: held once the hosts are chosen, whatever
//!   the player count; before that they play at once, as humans
//!   (Corrections 4).
//! - Spawns: the `Quarantine` points, one list (all `JSF` in the stock
//!   maps). No respawn request.
//!
//! The numbers are the mission's (`missions::quarantine`) or, without the
//! install's, our data's.
//!
//! Known shortcuts: the infected wear their body's infection override's
//! face (`characters::wear_looks`, so its eyes: `effects::attached`), not
//! its render styles or hands, a host its infected override (not the
//! host's own), and no change effect or sound plays;
//! the infected's live projectiles aren't removed (`[DestroyProjectiles]`
//! is read, not applied); any enemy kill by an infected infects (the
//! infected hold only their infection weapon).

use bevy::prelude::*;

use super::{
    CaMode, CaRuleSettings, Hooks, Leaver, NO_HOOKS, Panel, QUARANTINE, RoundTime, ScoreOrder, Spawns, Teams,
    flow::{alive, players},
    objectives,
    scoring::{personal, stats},
    spawns,
    state::{ALPHA, BRAVO, CaMatch, Death, DeathKind, Phase},
};
use crate::core::{Health, SimClock, Team};

/// The mode.
pub const MODE: CaMode = CaMode {
    rules: super::Mode {
        id: QUARANTINE,
        key: "QRT",
        shown_as: "Quarantine Regen",
        description: "Quarantine Regen: humans against the infected they become; hosts picked 20 s in, G rounds played",
        respawns: false,
        warmup: true,
        warmup_revives: false,
        // Always one frame, like TDM (Corrections 2026-10-22, 16).
        forced_setup: Some(0.0),
        teams: Teams::AllAlpha,
        on_death,
        time_out: |w| w.resource_mut::<CaMatch>().winner = Some(ALPHA),
        match_end,
        hooks: Hooks {
            setup,
            round_over,
            holds_joiner: hosts_chosen,
            counts_joiners: false,
            warmup_end: match_end,
            ..NO_HOOKS
        },
    },
    ff_scaled: false,
    assists: false,
    timer_shown: true,
    spawns: Spawns {
        family: Some("Quarantine"),
        one_list: true,
        shuffled: false,
        locked_rounds: false,
    },
    round_time: RoundTime::Mission,
    fire_ends_protection: false,
    death_drops: true,
    panel: Panel::RoundWins,
    order: ScoreOrder::MissionOnly,
    leaving,
};

/// After G rounds played, or with one player or fewer on a team (also
/// after the warm-up, `Hooks::warmup_end`): no winner (Corrections
/// 2026-10-22, 10).
fn match_end(world: &World) -> Option<Option<u8>> {
    let on_teams: u32 = world.resource::<super::CaTeamCounts>().0.iter().sum();
    let over = world.resource::<CaMatch>().round >= world.resource::<CaRuleSettings>().goal() || on_teams <= 1;
    over.then_some(None)
}

/// A leaver who leaves one player or fewer on the server ends the round at
/// once, no side winning (Corrections 2026-10-22, 10).
fn leaving(m: &mut CaMatch, l: &Leaver) {
    if m.phase == Phase::Live && l.players <= 2 && !m.decided() {
        m.end_without_winner();
    }
}

/// A joiner is held once the hosts are chosen (the live round past the
/// host-selection wait), whatever the player count; before, it plays at
/// once as a human (Corrections 2026-10-22, 4).
fn hosts_chosen(world: &World) -> bool {
    let wait = world.resource::<CaRuleSettings>().objectives.host_delay as f64;
    let now = world.resource::<SimClock>().now;
    now + 1e-9 >= world.resource::<CaMatch>().live_since + wait
}

/// The round's infection state (the server's).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct CaQuarantine {
    /// When the round went live.
    pub live_since: Option<f64>,
    /// The hosts were picked.
    pub picked: bool,
}

/// An infected player's stillness (the heal's clock): where it stood,
/// since when (its last move, hit or fire), the next heal, its health then.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct CaStill {
    pub at: Vec3,
    pub since: f64,
    pub next_heal: f64,
    pub health: f32,
}

/// An infected player waiting for its loadout to become the infected's
/// (a host's when `host`).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct CaInfectedKit {
    pub host: bool,
}

/// M8's host count for `n` players on teams.
pub fn hosts(n: usize, rate: f32, min: u32) -> usize {
    if n <= 3 {
        1
    } else {
        ((n as f32 * rate).round() as usize).max(min as usize)
    }
}

/// Everyone human again at the round's setup.
fn setup(world: &mut World) {
    *world.resource_mut::<CaQuarantine>() = CaQuarantine::default();
    for (e, _) in players(world) {
        world
            .entity_mut(e)
            .remove::<(super::super::movement::CaInfected, CaStill, CaInfectedKit)>();
    }
}

/// `e` becomes infected (a host when `host`): Bravo, the mission's
/// infected (or host) weapons, else our knife.
pub fn infect(world: &mut World, e: Entity, host: bool) {
    let now = world.resource::<SimClock>().now;
    let at = world.get::<Transform>(e).map_or(Vec3::ZERO, |t| t.translation);
    let health = world.get::<Health>(e).map_or(0.0, |h| h.current);
    world.entity_mut(e).insert((
        Team(BRAVO),
        super::super::movement::CaInfected,
        CaStill {
            at,
            since: now,
            next_heal: now,
            health,
        },
        CaInfectedKit { host },
    ));
}

/// M8 (Open question 12, answered): a human killed by an infected is
/// back at once where it died, on the floor below, at full health.
fn revive_in_place(world: &mut World, e: Entity) {
    let Some(t) = world.get::<Transform>(e).copied() else { return };
    let half = world
        .get::<crate::core::MovementState>(e)
        .map_or(0.0, |s| -s.hull_min.y)
        .max(0.0);
    let floor = objectives::floor_below(world, t.translation - Vec3::Y * half);
    let at = Transform::from_translation(floor + Vec3::Y * half).with_rotation(t.rotation);
    super::super::inventory::respawn_inventory(world, e);
    crate::spawning::revive(world, e, Some(at));
    world.entity_mut(e).remove::<(spawns::CaAsked, spawns::CaHeld)>();
}

/// The heal's view of an infected player (`update`).
type Healing<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static mut CaStill>,
        &'static Transform,
        &'static mut Health,
        &'static crate::core::Intent,
        Option<&'static super::super::gear::CaLoadout>,
    ),
>;

/// Host selection, the infected's kit and heal, the win tests (the mode's
/// per-tick system, `rules::mode_updates`). The hosts picked this tick are
/// infected through `Commands` and counted as such by the rest of the tick
/// (their kit, their stillness, the win test); the kits' weapons are world
/// work, queued.
#[allow(clippy::too_many_arguments)]
pub(super) fn update(
    clock: Res<SimClock>,
    mission: Option<Res<super::super::missions::CaMission>>,
    settings: Res<CaRuleSettings>,
    data: Res<super::super::data::CaData>,
    mut state: ResMut<CaQuarantine>,
    mut m: ResMut<CaMatch>,
    mut draws: ResMut<objectives::CaDraws>,
    kits: Query<(Entity, &CaInfectedKit)>,
    inventories: Query<&crate::weapon::Inventory>,
    weapons: Query<&crate::weapon::Weapon>,
    mut sets: ParamSet<(objectives::Roster, Healing)>,
    mut commands: Commands,
) {
    let now = clock.now;
    let o = &settings.objectives;
    if !m.counting() {
        return;
    }
    let n = super::super::missions::quarantine_in(mission.as_deref(), &settings, &data);
    let since = match state.live_since {
        Some(s) => s,
        None => {
            state.live_since = Some(now);
            now
        }
    };
    // The hosts picked now, with their new stillness.
    let mut picked: Vec<(Entity, CaStill)> = Vec::new();
    if !state.picked && now + 1e-9 >= since + o.host_delay as f64 {
        state.picked = true;
        let roster = sets.p0();
        let h = hosts(roster.players().len(), n.selection_rate, n.min_hosts);
        let mut humans: Vec<(Entity, Vec3)> = roster
            .living()
            .into_iter()
            .filter(|p| p.team == ALPHA)
            .map(|p| (p.e, p.at))
            .collect();
        if humans.len() <= h {
            m.end_without_winner();
            return;
        }
        objectives::shuffle_in(&mut draws, &mut humans);
        let healing = sets.p1();
        for (e, at) in humans.into_iter().take(h) {
            let health = healing.get(e).map_or(0.0, |(_, _, h, ..)| h.current);
            let still = CaStill {
                at,
                since: now,
                next_heal: now,
                health,
            };
            if let Ok(mut ec) = commands.get_entity(e) {
                ec.try_insert((Team(BRAVO), super::super::movement::CaInfected, still, CaInfectedKit { host: true }));
            }
            picked.push((e, still));
        }
    }
    // The infected's weapons only, once the loadout is there.
    let kit_list: Vec<(Entity, bool)> = kits
        .iter()
        .map(|(e, k)| (e, k.host))
        .chain(picked.iter().map(|(e, _)| (*e, true)))
        .collect();
    for (e, host) in kit_list {
        let mut keep = if host { n.host_weapons.clone() } else { n.infected_weapons.clone() };
        if keep.is_empty() {
            keep.push(super::super::inventory::KNIFE);
        }
        let ids: Vec<&'static str> = inventories
            .get(e)
            .map(|i| i.weapons.iter().filter_map(|w| weapons.get(*w).ok().map(|w| w.id)).collect())
            .unwrap_or_default();
        if ids.is_empty() {
            continue;
        }
        commands.queue(move |world: &mut World| {
            for id in &keep {
                objectives::give_weapon(world, e, id);
            }
            for id in ids.into_iter().filter(|id| !keep.contains(id)) {
                objectives::take_weapon(world, e, id);
            }
            world.entity_mut(e).remove::<CaInfectedKit>();
        });
    }
    // The heal after standing still: `heal` of the maximum hit points
    // every period, the wait restarted by a move, a hit or firing.
    let mut healing = sets.p1();
    // `s` after this tick for a player at `at` with `health` (its new
    // health back).
    let heal = |s: &mut CaStill, at: Vec3, health: &Health, firing: bool, calloused: bool| -> f32 {
        let mut current = health.current;
        if at.distance(s.at) > 1e-3 || current < s.health - 1e-6 || firing {
            s.at = at;
            s.since = now;
            s.next_heal = now + n.heal_wait as f64;
        } else if now + 1e-9 >= s.since + n.heal_wait as f64 && now + 1e-9 >= s.next_heal {
            // Calloused (gear_perks.md 15): an infected with an
            // infected-mercenary item heals at `HPRecoverRate` 2.
            let rate = if calloused { super::super::gear::CALLOUSED_RATE } else { 1.0 };
            current = (current + rate * n.heal * health.max).min(health.max);
            s.next_heal = now + n.heal_period as f64;
        }
        s.health = current;
        current
    };
    for (still, t, mut health, intent, loadout) in &mut healing {
        let Some(mut still) = still else { continue };
        if health.current <= 0.0 {
            continue;
        }
        let mut s = *still;
        let now_health = heal(&mut s, t.translation, &health, intent.fire, loadout.is_some_and(|l| l.infected_item));
        if now_health != health.current {
            health.current = now_health;
        }
        *still = s;
    }
    for (e, still) in &picked {
        let Ok((_, t, mut health, intent, loadout)) = healing.get_mut(*e) else { continue };
        if health.current <= 0.0 {
            continue;
        }
        let mut s = *still;
        let now_health = heal(&mut s, t.translation, &health, intent.fire, loadout.is_some_and(|l| l.infected_item));
        if now_health != health.current {
            health.current = now_health;
        }
        if let Ok(mut ec) = commands.get_entity(*e) {
            ec.try_insert(s);
        }
    }
    // The win tests, the hosts counted as infected.
    if m.winner.is_some() {
        return;
    }
    let roster = sets.p0();
    let team_of = |e: Entity, t: u8| if picked.iter().any(|(x, _)| *x == e) { BRAVO } else { t };
    let (humans, infected) = (roster.alive_as(ALPHA, team_of), roster.alive_as(BRAVO, team_of));
    if humans == 0 {
        m.win(BRAVO);
    } else if state.picked && infected == 0 {
        m.win(ALPHA);
    }
}

/// No human alive: the infected win; after the selection, no infected
/// alive: the humans.
fn test_win(world: &mut World) {
    if world.resource::<CaMatch>().winner.is_some() || !world.resource::<CaMatch>().counting() {
        return;
    }
    let (humans, infected) = (alive(world, ALPHA), alive(world, BRAVO));
    let picked = world.resource::<CaQuarantine>().picked;
    if humans == 0 {
        world.resource_mut::<CaMatch>().win(BRAVO);
    } else if picked && infected == 0 {
        world.resource_mut::<CaMatch>().win(ALPHA);
    }
}

fn on_death(world: &mut World, d: &Death) {
    personal(world, d);
    let n = super::super::missions::quarantine(world);
    if d.kind == DeathKind::Enemy
        && let Some(k) = d.killer
    {
        if d.victim_team == BRAVO {
            stats(world, k).mission_points += n.kill_point;
            stats(world, d.victim).mission_points += n.dead_point;
        } else {
            stats(world, k).mission_points += n.infect_point;
            stats(world, d.victim).mission_points += n.infected_point;
            revive_in_place(world, d.victim);
            infect(world, d.victim, false);
        }
    }
    test_win(world);
}

/// The living members of the winning side: +10 humans, +5 infected.
fn round_over(world: &mut World) {
    let m = world.resource::<CaMatch>();
    if m.warmup {
        return;
    }
    let Some(winner) = m.winner else { return };
    let n = super::super::missions::quarantine(world);
    let points = if winner == ALPHA { n.human_win_point } else { n.infect_win_point };
    for p in objectives::living(world) {
        if p.team == winner {
            stats(world, p.e).mission_points += points;
        }
    }
}
