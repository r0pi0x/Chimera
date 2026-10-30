//! Bombing Run (`combat_arms:bombing_run`, room key `SBT`;
//! specs/combat_arms/game_modes.md M9): one shared bomb, Alpha against
//! Bravo, respawning; a warm-up, then rounds of `TimeLimit` 300 s; G round
//! wins take the match.
//!
//! - The bomb: at each real round's setup it lies on the first of the
//!   shuffled `Sabotage` drop points (S12); either team's living player
//!   takes it by touch (the mission bomb, `objectives::BOMB`, given to the
//!   carrier: `CaCarry::bomb`) and plants it at the enemy's site (a
//!   `Sabotage` `BombZoneTrigger` whose `DefusableTeam` is the enemy;
//!   `bomb`); a carrier's death drops it where they were (on the floor),
//!   a carrier who leaves drops it where they last were.
//! - Round win: the detonation, the planting team; a defuse, the other.
//! - Time-out: both teams get a round win (Quirk 7), the round no winner.
//! - Match end: a team has G round wins or a team is empty; the winner
//!   more round wins, if equal the last round's winner (none after a
//!   time-out).
//! - Deaths: S&D's counts (`scoring::round_mode`, Corrections 2026-10-22,
//!   6: a team kill gives the victim no death, a suicide no friendly
//!   kill). Friendly fire on is full damage (S8).
//! - Leaving (Corrections 2026-10-22, 7): the only player of a team, or
//!   the last of it alive, leaving gives the round to the other team.
//!
//! - Spawns: the `Sabotage` points per team (Open question 19). The
//!   carrier is given its own team's objective weapons of the mission
//!   (`missions::objective_weapons`: `XK-2` for both in stock files).
//!
//! Known shortcuts: the bomb on the ground isn't drawn (no item model:
//! the mission's `[DropObject]` look isn't read; the HUD shows where it
//! is); its pick-up box is (10, 25, 10) as the spec says; a map without
//! `Sabotage` drop points has no bomb.

use bevy::prelude::*;

use super::{
    BOMBING_RUN, CaMode, CaRuleSettings, Hooks, Leaver, NO_HOOKS, Panel, RoundTime, ScoreOrder, Spawns, TEAM_POINTS, Teams, bomb,
    objectives::{self, BOMB, CaCarry, CaObjectives, Edits, Roster, carry_in, set_carry_in},
    objects::{CaMapObjects, FLAG_DIMS, ObjBox},
    scoring::round_mode,
    state::{ALPHA, BRAVO, CaMatch, Phase, enemy, side},
};
use crate::core::{Dead, Died, Intent};
use avian3d::prelude::SpatialQuery;

/// The drop points' and sites' game type.
pub const FAMILY: &str = "Sabotage";

/// The mode.
pub const MODE: CaMode = CaMode {
    rules: super::Mode {
        id: BOMBING_RUN,
        key: "SBT",
        shown_as: "Bombing Run",
        description: "Bombing Run: one bomb for both teams, respawning; plant it at the enemy's site, G round wins take the \
                      match",
        respawns: true,
        warmup: true,
        warmup_revives: true,
        forced_setup: None,
        teams: Teams::Two,
        on_death: |w, d| round_mode(w, d),
        time_out,
        match_end,
        hooks: Hooks {
            setup,
            round_over: bomb::credit_planter,
            warmup_end: match_end,
            round_end: |w| {
                bomb::remove_all(w);
                w.resource_mut::<CaCarrier>().0 = None;
            },
            ..NO_HOOKS
        },
    },
    ff_scaled: false,
    assists: false,
    timer_shown: true,
    spawns: Spawns {
        family: Some(FAMILY),
        ..TEAM_POINTS
    },
    round_time: RoundTime::Mission,
    fire_ends_protection: false,
    death_drops: true,
    panel: Panel::RoundWins,
    order: ScoreOrder::ExplodeDefuse,
    leaving,
};

/// The bomb's carrier and where they last were alive (the server's).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct CaCarrier(pub Option<(Entity, Vec3)>);

/// A real round's setup: the bomb on the first shuffled drop point.
fn setup(world: &mut World) {
    world.resource_mut::<CaCarrier>().0 = None;
    world.resource_mut::<bomb::CaPlanter>().0 = None;
    if world.resource::<CaMatch>().warmup {
        return;
    }
    let mut points: Vec<Vec3> = world
        .resource::<CaMapObjects>()
        .drops
        .iter()
        .filter(|d| d.family == FAMILY)
        .map(|d| d.at)
        .collect();
    objectives::shuffle(world, &mut points);
    world.resource_mut::<CaObjectives>().loose_bomb = points.first().copied();
}

/// Drop the bomb at `at` (on the floor).
fn drop_bomb(world: &mut World, carrier: Entity, at: Vec3) {
    objectives::take_weapon(world, carrier, BOMB);
    let mut c = objectives::carry(world, carrier);
    c.bomb = false;
    objectives::set_carry(world, carrier, c);
    world.resource_mut::<CaCarrier>().0 = None;
    let floor = objectives::floor_below(world, at);
    world.resource_mut::<CaObjectives>().loose_bomb = Some(floor);
}

/// The carrier's death drops the bomb.
fn died(world: &mut World, d: &Died) {
    if !objectives::carry(world, d.entity).bomb {
        return;
    }
    let at = world.get::<Transform>(d.entity).map_or(Vec3::ZERO, |t| t.translation);
    drop_bomb(world, d.entity, at);
}

/// The carrier's bomb dropped where it died: the mode's `mode::ModeDeath`
/// observer (plan ecs-cleanup step 5), its world work queued and applied by
/// the death step's flush, so where the mode's hook ran.
pub(super) fn on_mode_death(death: On<crate::mode::ModeDeath>, mut commands: Commands) {
    const MODES: &[&str] = &[BOMBING_RUN];
    if !MODES.contains(&death.mode) {
        return;
    }
    let d = death.died.clone();
    commands.queue(move |world: &mut World| died(world, &d));
}

/// The carrier kept (a leaver drops it) and the pick-up (the mode's
/// per-tick system, `rules::mode_updates`; then `bombs`). The taker's
/// weapons are world work, queued before its carry.
#[allow(clippy::too_many_arguments)]
pub(super) fn carry(
    mut carrier: ResMut<CaCarrier>,
    mut o: ResMut<CaObjectives>,
    m: Res<CaMatch>,
    places: Query<(&Transform, Has<Dead>)>,
    spatial: Option<SpatialQuery>,
    characters: Query<Entity, With<Intent>>,
    roster: Roster,
    inventories: Query<&crate::weapon::Inventory>,
    mut carries: Query<&mut CaCarry>,
    mut commands: Commands,
) {
    if let Some((e, last)) = carrier.0 {
        match places.get(e) {
            Ok((t, false)) => carrier.0 = Some((e, t.translation)),
            Ok(_) => {}
            Err(_) => {
                carrier.0 = None;
                o.loose_bomb = Some(objectives::floor_below_in(spatial.as_ref(), &characters, last));
            }
        }
    }
    if let Some(at) = o.loose_bomb
        && m.counting()
    {
        let area = ObjBox {
            centre: at,
            half: FLAG_DIMS * super::super::inventory::UNIT,
        };
        let half = objectives::player_half();
        let taker = roster
            .takers()
            .into_iter()
            .find(|p| side(p.team).is_some() && area.touches(p.at, half));
        if let Some(p) = taker
            && inventories.get(p.e).is_ok_and(|i| !i.weapons.is_empty())
        {
            o.loose_bomb = None;
            // The taker's own team's field (missions.md F3: `JSFWeapon` for
            // JSF, `CreedWeapon` for Creed; both `XK-2`).
            let (e, team) = (p.e, p.team);
            commands.queue(move |world: &mut World| {
                let mut ids = super::super::missions::objective_weapons(world, team);
                if ids.is_empty() {
                    ids.push(BOMB);
                }
                for id in ids {
                    objectives::give_weapon(world, e, id);
                }
            });
            let mut edits = Edits::default();
            let mut c = carry_in(&edits, &carries, p.e);
            c.bomb = true;
            set_carry_in(&mut edits, p.e, c);
            edits.apply(&mut carries, &mut commands);
            carrier.0 = Some((p.e, p.at));
        }
    }
}

/// Plants, defuses and the fuse on Bombing Run's sites (after `carry`).
pub(super) fn bombs(mut bombs: bomb::Bombs) {
    bombs.tick(FAMILY);
}

/// The only player of a team leaving, or none of its teammates alive (dead
/// and waiting to respawn; the leaver alive or dead): the other team wins
/// the round (Corrections 2026-10-22, 7).
fn leaving(m: &mut CaMatch, l: &Leaver) {
    if m.phase != Phase::Live || m.winner.is_some() {
        return;
    }
    let others_alive = l.team_alive - usize::from(l.alive);
    if l.team_players <= 1 || others_alive == 0 {
        m.win(enemy(l.team));
    }
}

/// Both teams +1 round win, no winner.
fn time_out(world: &mut World) {
    let mut m = world.resource_mut::<CaMatch>();
    m.round_wins[0] += 1;
    m.round_wins[1] += 1;
    m.winner = None;
}

/// G round wins or an empty team: more wins, else the last round's winner.
fn match_end(world: &World) -> Option<Option<u8>> {
    let goal = world.resource::<CaRuleSettings>().goal();
    let m = world.resource::<CaMatch>();
    let counts = world.resource::<super::CaTeamCounts>().0;
    if !counts.contains(&0) && m.round_wins.iter().all(|w| *w < goal) {
        return None;
    }
    Some(match m.round_wins[0].cmp(&m.round_wins[1]) {
        std::cmp::Ordering::Greater => Some(ALPHA),
        std::cmp::Ordering::Less => Some(BRAVO),
        std::cmp::Ordering::Equal => m.results.last().and_then(|r| r.winner),
    })
}
