//! Spy Hunt (`combat_arms:spy_hunt`, room key `BNT`;
//! specs/combat_arms/game_modes.md M7): an intel hunt in two stages, one
//! round of G minutes after a warm-up.
//!
//! - Stage 1 (respawning): everybody starts a hunter (Alpha). At the
//!   round's start `intel_count` (5) intel items lie on the first shuffled
//!   `BountyHunter` drop points (S12). A living player touching one picks
//!   it up (`objectives::CaCarry::intel`, mission points +1) and becomes a
//!   spy with a team of their own (S1's free-for-all ids), so everyone may
//!   shoot them; a spy's death drops their intel where they were (on the
//!   floor) and makes them a hunter again.
//! - Stage 2: the player holding all the intel becomes the final spy
//!   (Bravo), everyone else a hunter; every dead player is revived, every
//!   living one refilled (health, ammunition, armour); respawning stops. The hunters win by
//!   killing the final spy, the final spy when no hunter is left alive.
//! - Deaths: One Man Army's counts (`scoring::free_for_all`); in stage 2
//!   an enemy kill adds no kill and no death, and the final spy's death by
//!   a hunter sets that hunter's mission points to the intel total and the
//!   spy's to 0 (Corrections 2026-10-22, 11).
//! - Leaving: in stage 2 the last of either side leaving, in stage 1 a
//!   leaver who leaves one player or fewer, ends the round with no winner
//!   set: the time-out rule gives it to the hunters and clears the spy's
//!   points (Corrections 2026-10-22, 12).
//! - Spawns: the `BountyHunter` points, one list.
//!
//! - Stage 2's clock (Open question 10, answered): the views on the map
//!   camera for `AgentCameraHoldingTime` (`CaObjectives::camera_until`),
//!   then the round timer restarted with `Stage2PlayTime`; a time-out in
//!   either stage goes to the hunters (Alpha). The numbers are the
//!   mission's (`missions::spy_hunt`: `[IntelObject]` `TotalNum`), else
//!   our data's.
//!
//! Known shortcuts: no spy or agent models (`SpyModelSetting`,
//! `AgentModelSetting` are read, not worn) and no agent weapons; the map
//! camera (`CameraPoint00`) isn't one: the views stay their own during
//! the hold; the mission's `AgentTrigger` isn't run; intel on the ground
//! isn't drawn (no item model; the HUD says how many lie about); its
//! pick-up box is (10, 25, 10) as the spec says, moved to the floor.

use bevy::prelude::*;

use super::{
    CaMode, Hooks, Leaver, NO_HOOKS, Panel, RoundTime, SPY_HUNT, ScoreOrder, Spawns, Teams,
    flow::{alive, players},
    objectives::{self, CaCarry, CaObjectives, Edits, Roster, add_stats, carry_in, set_carry_in},
    objects::{CaMapObjects, FLAG_DIMS, ObjBox},
    scoring::free_for_all,
    spawns,
    state::{ALPHA, BRAVO, CaMatch, CaStats, Death, DeathKind, FFA_FIRST, FFA_LAST, Phase},
};
use super::super::missions::{self, CaMission};
use crate::core::{Dead, Died, Health, Team};

/// The intel drop points' and the start points' game type.
pub const FAMILY: &str = "BountyHunter";

/// The mode.
pub const MODE: CaMode = CaMode {
    rules: super::Mode {
        id: SPY_HUNT,
        key: "BNT",
        shown_as: "Spy Hunt",
        description: "Spy Hunt: pick up intel to become a spy; whoever holds it all is the final spy, hunted by everyone \
                      else",
        respawns: true,
        warmup: true,
        warmup_revives: false,
        forced_setup: None,
        teams: Teams::AllAlpha,
        on_death,
        // Open question 10, answered: the hunters (Alpha) in both stages.
        time_out,
        match_end: |w| Some(w.resource::<CaMatch>().winner),
        hooks: Hooks {
            setup,
            may_respawn: |w, _| w.resource::<CaObjectives>().stage != 2,
            ..NO_HOOKS
        },
    },
    ff_scaled: false,
    assists: false,
    timer_shown: true,
    spawns: Spawns {
        family: Some(FAMILY),
        one_list: true,
        shuffled: false,
        locked_rounds: false,
    },
    round_time: RoundTime::GoalMinutes,
    fire_ends_protection: false,
    death_drops: true,
    panel: Panel::Kills,
    order: ScoreOrder::MissionPoints,
    leaving,
};

/// The intel at the round's start (stage 1).
fn setup(world: &mut World) {
    let warmup = world.resource::<CaMatch>().warmup;
    let count = super::super::missions::spy_hunt(world).0 as usize;
    let mut points: Vec<Vec3> = world
        .resource::<CaMapObjects>()
        .drops
        .iter()
        .filter(|d| d.family == FAMILY)
        .map(|d| d.at)
        .collect();
    objectives::shuffle(world, &mut points);
    let mut o = world.resource_mut::<CaObjectives>();
    o.stage = if warmup { 0 } else { 1 };
    o.intel = if warmup { Vec::new() } else { points.into_iter().take(count).collect() };
}

/// A spy's death drops their intel and makes them a hunter.
fn died(world: &mut World, d: &Died) {
    let c = objectives::carry(world, d.entity);
    if c.intel == 0 || world.resource::<CaObjectives>().stage != 1 {
        return;
    }
    let at = world.get::<Transform>(d.entity).map_or(Vec3::ZERO, |t| t.translation);
    let floor = objectives::floor_below(world, at);
    {
        let mut o = world.resource_mut::<CaObjectives>();
        o.intel.extend(std::iter::repeat_n(floor, c.intel as usize));
    }
    let mut c = c;
    c.intel = 0;
    objectives::set_carry(world, d.entity, c);
    world.entity_mut(d.entity).insert(Team(ALPHA));
}

/// A carrier's intel dropped where it died, the carrier back on Alpha (stage
/// 1): the mode's `mode::ModeDeath` observer (plan ecs-cleanup step 5), its
/// world work queued and applied by the death step's flush, so where the
/// mode's hook ran.
pub(super) fn on_mode_death(death: On<crate::mode::ModeDeath>, mut commands: Commands) {
    const MODES: &[&str] = &[SPY_HUNT];
    if !MODES.contains(&death.mode) {
        return;
    }
    let d = death.died.clone();
    commands.queue(move |world: &mut World| died(world, &d));
}

/// Pick-ups, then stage 2 when one player holds it all (the mode's
/// per-tick system, `rules::mode_updates`; stage 2's respawns are world
/// work, queued after the pick-up's changes).
#[allow(clippy::too_many_arguments)]
pub(super) fn update(
    clock: Res<crate::core::SimClock>,
    mission: Option<Res<CaMission>>,
    settings: Res<super::CaRuleSettings>,
    m: Res<CaMatch>,
    mut o: ResMut<CaObjectives>,
    roster: Roster,
    mut carries: Query<&mut CaCarry>,
    mut stats: Query<&mut CaStats>,
    mut commands: Commands,
) {
    let now = clock.now;
    if o.camera_until.is_some_and(|t| now + 1e-9 >= t) {
        o.camera_until = None;
    }
    if !m.counting() || o.stage != 1 {
        return;
    }
    let total = missions::spy_hunt_in(mission.as_deref(), &settings).0 as u8;
    let half = objectives::player_half();
    // The players' teams as this tick changes them (a free id is free of
    // the ones given before it).
    let mut teams = roster.players();
    let mut carry = Edits::default();
    for p in roster.takers() {
        let Some(i) = o.intel.iter().position(|at| {
            ObjBox {
                centre: *at,
                half: FLAG_DIMS * super::super::inventory::UNIT,
            }
            .touches(p.at, half)
        }) else {
            continue;
        };
        o.intel.remove(i);
        let mut c = carry_in(&carry, &carries, p.e);
        c.intel += 1;
        set_carry_in(&mut carry, p.e, c);
        add_stats(&mut stats, &mut commands, p.e, |s| s.mission_points += 1);
        if p.team == ALPHA
            && let Some(id) = (FFA_FIRST..=FFA_LAST).find(|t| !teams.iter().any(|(_, x)| x == t))
        {
            if let Ok(mut ec) = commands.get_entity(p.e) {
                ec.try_insert(Team(id));
            }
            if let Some(x) = teams.iter_mut().find(|(e, _)| *e == p.e) {
                x.1 = id;
            }
        }
        if c.intel >= total {
            carry.apply(&mut carries, &mut commands);
            let spy = p.e;
            commands.queue(move |world: &mut World| stage_two(world, spy));
            return;
        }
    }
    carry.apply(&mut carries, &mut commands);
}

/// `spy` holds all the intel: the final spy against everyone else. The
/// views go to the map camera for `AgentCameraHoldingTime` (5 s), then the
/// round timer restarts with `Stage2PlayTime` (180 s); the dead are
/// respawned, the living refilled: full health, every weapon's ammunition
/// and their armour (Open question 10, answered).
fn stage_two(world: &mut World, spy: Entity) {
    let now = world.resource::<crate::core::SimClock>().now;
    let (_, stage2, hold) = super::super::missions::spy_hunt(world);
    {
        let mut o = world.resource_mut::<CaObjectives>();
        o.stage = 2;
        o.intel.clear();
        o.camera_until = Some(now + hold as f64);
    }
    world.resource_mut::<CaMatch>().round_ends = Some(now + (hold + stage2) as f64);
    for (e, _) in players(world) {
        world.entity_mut(e).insert(Team(if e == spy { BRAVO } else { ALPHA }));
        if world.get::<Dead>(e).is_some() {
            spawns::respawn(world, e);
        } else {
            refill(world, e);
        }
    }
    info!("Spy Hunt stage 2: the final spy is {spy:?}");
}

/// Full health, every carried weapon's clip and reserve, the armour.
pub fn refill(world: &mut World, e: Entity) {
    if let Some(mut h) = world.get_mut::<Health>(e) {
        h.current = h.max;
    }
    let weapons = world.get::<crate::weapon::Inventory>(e).map(|i| i.weapons.clone()).unwrap_or_default();
    for w in weapons {
        if let Some(mut m) = world.get_mut::<crate::weapon::Magazine>(w) {
            m.clip = m.size;
            m.reserve = m.reserve_max;
        }
    }
    world.entity_mut(e).insert(super::super::weapons::default_armour());
}

/// The hunters win a time-out (and a round a leaver ended), clearing the
/// final spy's mission points in stage 2 (Corrections 2026-10-22, 12).
fn time_out(world: &mut World) {
    world.resource_mut::<CaMatch>().winner = Some(ALPHA);
    if world.resource::<CaObjectives>().stage == 2 {
        clear_spy_points(world);
    }
}

fn clear_spy_points(world: &mut World) {
    for (e, t) in players(world) {
        if t == BRAVO {
            super::scoring::stats(world, e).mission_points = 0;
        }
    }
}

fn on_death(world: &mut World, d: &Death) {
    // Stage 2: an enemy kill adds no kill and no death to anyone (team
    // kills, suicides and falls count as in stage 1); the final spy's
    // death by a hunter sets that hunter's mission points to the total
    // intel count and the spy's to 0 (Corrections 2026-10-22, 11).
    if world.resource::<CaObjectives>().stage == 2 && d.kind == DeathKind::Enemy {
        if d.victim_team == BRAVO
            && let Some(k) = d.killer
            && world.resource::<CaMatch>().winner.is_none()
        {
            let total = super::super::missions::spy_hunt(world).0 as i32;
            super::scoring::stats(world, k).mission_points = total;
            super::scoring::stats(world, d.victim).mission_points = 0;
        }
    } else {
        free_for_all(world, d);
    }
    if world.resource::<CaObjectives>().stage != 2 || world.resource::<CaMatch>().winner.is_some() {
        return;
    }
    if alive(world, BRAVO) == 0 {
        world.resource_mut::<CaMatch>().win(ALPHA);
    } else if alive(world, ALPHA) == 0 {
        world.resource_mut::<CaMatch>().win(BRAVO);
    }
}

/// Stage 2: the last player of either side leaving, or in stage 1 a leaver
/// who leaves one player or fewer, ends the round with no winner set, so
/// the time-out rule gives it to the hunters (Corrections 2026-10-22, 12).
fn leaving(m: &mut CaMatch, l: &Leaver) {
    if m.phase != Phase::Live || m.warmup || m.decided() {
        return;
    }
    let stage_two = l.team == BRAVO || l.others.contains(&BRAVO);
    if (stage_two && l.team_players <= 1) || (!stage_two && l.players <= 2) {
        m.end_requested = true;
    }
}
