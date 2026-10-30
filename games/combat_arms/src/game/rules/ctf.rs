//! Capture the Flag (`combat_arms:capture_the_flag`, room key `CTF`;
//! specs/combat_arms/game_modes.md M4): Alpha against Bravo, respawning,
//! one round of `TimeLimit` x G (180 x G s); the first team to G captures
//! wins.
//!
//! - Objects: one flag per team (`FlagItem`, `GameType`
//!   `Capturetheflag`: its home and its (10, 25, 10) box) and one flag
//!   zone per team (`FlagZoneTrigger`) at the flag's base
//!   (`objects::CaMapObjects`). The flags' state is
//!   `objectives::CaObjectives::flags`, every machine's.
//! - Taking: a living player whose box touches the enemy flag (at home or
//!   dropped) takes it (`objectives::CaCarry::flag`); the flag leaves the
//!   world while carried.
//! - Returning: a living player touching their own flag while it is
//!   dropped sends it home at once: team returns +1, player returns +1.
//!   Touching your own flag at home does nothing.
//! - Dropping: a carrier's death drops it where the carrier was, moved
//!   down to the floor, going home after `flag_return` (30 s) untouched,
//!   no credit; a carrier killed by a kill zone (a volume's damage, which
//!   has no attacker, or a telefrag's crush) sends it home at once; a
//!   carrier who leaves drops it where they last were.
//! - Capture: a carrier of the enemy flag touching their own team's flag
//!   zone while their own flag is home, in any phase but the warm-up (a
//!   capture in the round-over hold still counts, Quirk 13): team captures
//!   (the team counter, `CaMatch::score`) +1, player captures +1, the
//!   carried flag home. Every capture that leaves a team at G or more
//!   re-picks the winner (more captures; equal keeps the old one).
//! - Deaths (S7's CTF column): enemy kill, killer kills +1, victim deaths
//!   +1, the killer's team's kills +1; team kill, killer friendly kills
//!   +1, victim deaths +1, the enemy team's kills +1; own damage, victim
//!   deaths, suicides and friendly kills +1, the enemy team's kills +1; no
//!   killer, victim deaths and suicides +1; every death, the victim's
//!   team's deaths +1 (`CaMatch::kills`, `deaths`: tie-breaks only).
//! - Time-out: more captures; else more returns; else more team kills;
//!   else fewer deaths; else a draw.
//! - Leaving: the last player of a team leaving gives the round to the
//!   other team at once. Spawns: the `Capturetheflag` points per team.
//! - The flags' props (`props`: the maps' `FlagItem` models) are shown
//!   only in capture the flag, at home or where dropped, hidden while
//!   carried (`show_flags`, every machine).
//!
//! Taking and returning work in every phase; returns, like captures,
//! count except in the warm-up (Open question 34, answered). No assists
//! (S7's list hasn't the mode).
//!
//! Known shortcuts: a carried flag isn't drawn on the carrier's back (the
//! attachments file's `Flag#` models on `1W_N` aren't loaded); no
//! late-join closing.

use bevy::prelude::*;

use super::{
    CAPTURE_THE_FLAG, CLAN_CTF, CaMode, CaRuleSettings, Hooks, NO_HOOKS, Panel, RoundTime, ScoreOrder, Spawns, Teams,
    objectives::{self, CaCarry, CaObjectives, Edits, Flag, FlagAt, Roster, add_stats, carry_in, set_carry_in},
    objects::CaMapObjects,
    scoring::personal,
    state::{CaMatch, CaStats, Death, DeathKind, Phase, enemy, side},
};
use crate::core::{DamageKind, Died, Intent, SimClock};
use avian3d::prelude::SpatialQuery;
use std::ops::DerefMut;

/// The flags' and points' game type.
pub const FAMILY: &str = "Capturetheflag";

/// The mode.
pub const MODE: CaMode = CaMode {
    rules: super::Mode {
        id: CAPTURE_THE_FLAG,
        key: "CTF",
        shown_as: "Capture the Flag",
        description: "Capture the Flag: Alpha against Bravo, respawning; take the enemy flag to your zone while yours is \
                      home, the first team to G captures wins",
        respawns: true,
        warmup: false,
        warmup_revives: false,
        forced_setup: None,
        teams: Teams::Two,
        on_death,
        time_out,
        match_end: |w| Some(w.resource::<CaMatch>().winner),
        hooks: Hooks {
            setup,
            round_end: |w| {
                let carriers = w.resource::<CaFlagCarriers>().0;
                for (e, _) in carriers.into_iter().flatten() {
                    let mut c = objectives::carry(w, e);
                    c.flag = None;
                    objectives::set_carry(w, e, c);
                }
                *w.resource_mut::<CaFlagCarriers>() = CaFlagCarriers::default();
            },
            ..NO_HOOKS
        },
    },
    ff_scaled: true,
    assists: false,
    timer_shown: true,
    spawns: Spawns {
        family: Some(FAMILY),
        one_list: false,
        shuffled: false,
        locked_rounds: false,
    },
    round_time: RoundTime::MissionTimesGoal,
    fire_ends_protection: false,
    death_drops: true,
    panel: Panel::TeamScore,
    order: ScoreOrder::Captures,
    leaving: super::tdm::leaving,
};

/// Who carries each team's flag (Alpha's, Bravo's) and where they last
/// were alive (the server's).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct CaFlagCarriers(pub [Option<(Entity, Vec3)>; 2]);

/// The flags home at the round's setup.
fn setup(world: &mut World) {
    let objects = world.resource::<CaMapObjects>();
    let of_family: Vec<_> = objects.flags.iter().filter(|f| f.family == FAMILY).cloned().collect();
    let list = if of_family.is_empty() { objects.flags.clone() } else { of_family };
    let mut flags: Vec<Flag> = Vec::new();
    for f in list {
        if flags.iter().any(|x| x.team == f.team) {
            continue;
        }
        flags.push(Flag {
            team: f.team,
            home: f.area.centre,
            at: FlagAt::Home,
        });
    }
    flags.sort_by_key(|f| f.team);
    world.resource_mut::<CaObjectives>().flags = flags;
    *world.resource_mut::<CaFlagCarriers>() = CaFlagCarriers::default();
}

/// The flag's box half-extents (its object's, else the default).
fn flag_half(objects: &CaMapObjects, team: u8) -> Vec3 {
    objects
        .flags
        .iter()
        .find(|f| f.team == team)
        .map_or(super::objects::FLAG_DIMS * super::super::inventory::UNIT, |f| f.area.half)
}

/// Send `team`'s flag home.
fn send_home(o: &mut impl DerefMut<Target = CaObjectives>, team: u8) {
    if let Some(f) = o.flags.iter_mut().find(|f| f.team == team) {
        f.at = FlagAt::Home;
    }
}

/// Drop `team`'s flag on `floor` (going home at `returns`), or send it
/// home when `floor` is None.
fn drop_flag_at(
    carriers: &mut impl DerefMut<Target = CaFlagCarriers>,
    o: &mut impl DerefMut<Target = CaObjectives>,
    team: u8,
    floor: Option<(Vec3, f64)>,
) {
    if let Some(i) = side(team) {
        carriers.0[i] = None;
    }
    let Some((floor, returns)) = floor else {
        send_home(o, team);
        return;
    };
    if let Some(f) = o.flags.iter_mut().find(|f| f.team == team) {
        f.at = FlagAt::Dropped { at: floor, returns };
    }
}

/// When a flag dropped now goes home.
fn returns_at(clock: &SimClock, settings: &CaRuleSettings) -> f64 {
    clock.now + settings.objectives.flag_return as f64
}

/// Drop `team`'s flag at `at` (moved down to the floor), or send it home
/// when `home`.
fn drop_flag(world: &mut World, team: u8, at: Vec3, home: bool) {
    let floor = (!home).then(|| {
        let floor = objectives::floor_below(world, at);
        (floor, returns_at(world.resource(), world.resource()))
    });
    world.resource_scope::<CaFlagCarriers, _>(|world, mut carriers| {
        drop_flag_at(&mut carriers, &mut world.resource_mut::<CaObjectives>(), team, floor);
    });
}

/// A carrier's death drops the flag (a kill zone sends it home).
fn died(world: &mut World, d: &Died) {
    let c = objectives::carry(world, d.entity);
    let Some(team) = c.flag else { return };
    let kill_zone = (d.damage.kind == DamageKind::Generic && d.attacker.is_none())
        || (d.damage.kind == DamageKind::Crush && d.damage.weapon.is_some());
    let at = world.get::<Transform>(d.entity).map_or(Vec3::ZERO, |t| t.translation);
    world.entity_mut(d.entity).remove::<CaCarry>();
    drop_flag(world, team, at, kill_zone);
}

/// A carrier's flag dropped where it died (Capture the Flag, Clan Capture the
/// Flag): the mode's `mode::ModeDeath` observer (plan ecs-cleanup step 5),
/// its world work queued and applied by the death step's flush, so where the
/// mode's hook ran.
pub(super) fn on_mode_death(death: On<crate::mode::ModeDeath>, mut commands: Commands) {
    const MODES: &[&str] = &[CAPTURE_THE_FLAG, CLAN_CTF];
    if !MODES.contains(&death.mode) {
        return;
    }
    let d = death.died.clone();
    commands.queue(move |world: &mut World| died(world, &d));
}

/// Carriers, return timers, touches and captures (the mode's per-tick
/// system, `rules::mode_updates`; Clan Capture the Flag's too).
#[allow(clippy::too_many_arguments)]
pub(super) fn update(
    clock: Res<SimClock>,
    settings: Res<CaRuleSettings>,
    objects: Res<CaMapObjects>,
    mut carriers: ResMut<CaFlagCarriers>,
    mut o: ResMut<CaObjectives>,
    mut m: ResMut<CaMatch>,
    places: Query<(&Transform, Has<crate::core::Dead>)>,
    roster: Roster,
    spatial: Option<SpatialQuery>,
    characters: Query<Entity, With<Intent>>,
    mut carries: Query<&mut CaCarry>,
    mut stats: Query<&mut CaStats>,
    mut commands: Commands,
) {
    let now = clock.now;
    // Carriers: their last living place; one gone (left) drops the flag.
    let snapshot = carriers.0;
    for (i, c) in snapshot.iter().enumerate() {
        let Some((e, last)) = *c else { continue };
        let team = i as u8 + 1;
        match places.get(e) {
            Ok((t, false)) => carriers.0[i] = Some((e, t.translation)),
            Ok(_) => {}
            Err(_) => {
                let floor = objectives::floor_below_in(spatial.as_ref(), &characters, last);
                let returns = returns_at(&clock, &settings);
                drop_flag_at(&mut carriers, &mut o, team, Some((floor, returns)));
            }
        }
    }
    // Return timers.
    let expired: Vec<u8> = o
        .flags
        .iter()
        .filter(|f| matches!(f.at, FlagAt::Dropped { returns, .. } if now + 1e-9 >= returns))
        .map(|f| f.team)
        .collect();
    for team in expired {
        send_home(&mut o, team);
    }
    // Touches.
    let half = objectives::player_half();
    let goal = settings.goal() as i32;
    let mut carry = Edits::default();
    for p in roster.takers() {
        let flags = o.flags.clone();
        let held = carry_in(&carry, &carries, p.e);
        for f in &flags {
            let at = match f.at {
                FlagAt::Home => f.home,
                FlagAt::Dropped { at, .. } => at,
                FlagAt::Carried => continue,
            };
            let area = super::objects::ObjBox {
                centre: at,
                half: flag_half(&objects, f.team),
            };
            if !area.touches(p.at, half) {
                continue;
            }
            if f.team != p.team && held.flag.is_none() {
                // `p` takes the enemy flag.
                if let Some(taken) = o.flags.iter_mut().find(|x| x.team == f.team) {
                    taken.at = FlagAt::Carried;
                    if let Some(i) = side(f.team) {
                        carriers.0[i] = Some((p.e, p.at));
                    }
                    let mut c = carry_in(&carry, &carries, p.e);
                    c.flag = Some(f.team);
                    set_carry_in(&mut carry, p.e, c);
                }
                break;
            }
            if f.team == p.team && matches!(f.at, FlagAt::Dropped { .. }) {
                send_home(&mut o, f.team);
                if credits(&m) {
                    if let Some(i) = side(p.team) {
                        m.returns[i] += 1;
                    }
                    add_stats(&mut stats, &mut commands, p.e, |s| s.returns += 1);
                }
            }
        }
        // A carrier of the enemy flag at its own flag zone with its own
        // flag home captures.
        let c = carry_in(&carry, &carries, p.e);
        let Some(flag) = c.flag else { continue };
        if flag == p.team || !credits(&m) {
            continue;
        }
        let own_home = o.flag(p.team).is_none_or(|f| f.at == FlagAt::Home);
        let in_zone = objects
            .flag_zones
            .iter()
            .any(|z| z.team == p.team && z.area.touches(p.at, half));
        if !own_home || !in_zone {
            continue;
        }
        let mut cleared = c;
        cleared.flag = None;
        set_carry_in(&mut carry, p.e, cleared);
        if let Some(i) = side(flag) {
            carriers.0[i] = None;
        }
        send_home(&mut o, flag);
        add_stats(&mut stats, &mut commands, p.e, |s| s.captures += 1);
        let Some(i) = side(p.team) else { continue };
        m.score[i] += 1;
        // Every capture that leaves a team at G or more re-picks the
        // winner: the team with more captures, an equal count keeping the
        // old one (Corrections 2026-10-22, 15; the capturer when there is
        // none).
        if m.score[i] >= goal {
            let w = match m.score[0].cmp(&m.score[1]) {
                std::cmp::Ordering::Greater => super::state::ALPHA,
                std::cmp::Ordering::Less => super::state::BRAVO,
                std::cmp::Ordering::Equal => m.winner.unwrap_or(p.team),
            };
            m.win(w);
        }
        info!("flag captured by team {}: {:?}", p.team, m.score);
    }
    carry.apply(&mut carries, &mut commands);
}

/// Captures and returns count in every phase of the match but the
/// warm-up (Open question 34, Quirk 13: a capture in the round-over hold
/// still scores).
fn credits(m: &CaMatch) -> bool {
    !m.warmup && !matches!(m.phase, Phase::Waiting | Phase::Starting | Phase::MatchOver)
}

fn on_death(world: &mut World, d: &Death) {
    personal(world, d);
    let scorer = match d.kind {
        DeathKind::Enemy => Some(enemy(d.victim_team)),
        DeathKind::TeamKill | DeathKind::Suicide => Some(enemy(d.victim_team)),
        DeathKind::NoKiller => None,
    };
    let mut m = world.resource_mut::<CaMatch>();
    if let Some(i) = scorer.and_then(side) {
        m.kills[i] += 1;
    }
    if let Some(i) = side(d.victim_team) {
        m.deaths[i] += 1;
    }
}

/// More captures; more returns; more kills; fewer deaths; else none.
fn time_out(world: &mut World) {
    let mut m = world.resource_mut::<CaMatch>();
    m.winner = pick(m.score[0] as i64, m.score[1] as i64)
        .or_else(|| pick(m.returns[0] as i64, m.returns[1] as i64))
        .or_else(|| pick(m.kills[0] as i64, m.kills[1] as i64))
        .or_else(|| pick(m.deaths[1] as i64, m.deaths[0] as i64));
}

/// Alpha when `a` is greater, Bravo when `b` is, None when equal.
pub fn pick(a: i64, b: i64) -> Option<u8> {
    match a.cmp(&b) {
        std::cmp::Ordering::Greater => Some(super::state::ALPHA),
        std::cmp::Ordering::Less => Some(super::state::BRAVO),
        std::cmp::Ordering::Equal => None,
    }
}

/// The flags' props: shown in capture the flag only, at home or where
/// dropped, hidden while carried (every machine, from the objectives it
/// sees).
pub fn show_flags(
    view: Res<super::CaMatchView>,
    objectives: Res<CaObjectives>,
    entities: Option<Res<crate::map::MapEntities>>,
    mut props: Query<(&crate::map::PropEntity, &crate::map::PropHome, &mut Transform, &mut Visibility)>,
) {
    let Some(entities) = entities else { return };
    let ctf = view.mode == MODE.key || view.mode == super::clan::CTF.key;
    for (index, home, mut t, mut vis) in &mut props {
        let Some(e) = entities.entities.get(index.0) else { continue };
        if e.classname() != "FlagItem" {
            continue;
        }
        let team = e.get("Teams").and_then(super::objects::team_named);
        let flag = team.and_then(|t| objectives.flag(t));
        let (shown, at) = match flag.map(|f| f.at) {
            _ if !ctf => (false, home.0.translation),
            Some(FlagAt::Carried) => (false, home.0.translation),
            Some(FlagAt::Dropped { at, .. }) => (true, at),
            _ => (true, home.0.translation),
        };
        let want = if shown { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
        if t.translation != at {
            t.translation = at;
        }
    }
}
