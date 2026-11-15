//! Deaths and scores (specs/halo_ce/multiplayer_rules.md 6.1, 6.2, 4.2, 9):
//! the kill bookkeeping every game type shares (`died`: the dead player's
//! counters, who is credited, the multi-kill window and the spree), the
//! game type's kill handler (slayer's, `slayer_kill`; capture the flag,
//! king and oddball score elsewhere), the respawn timer (`respawn_timer`),
//! the announcer's choice for a kill (`announcement`), and slayer's speed
//! rule (`kill_penalty`, `death_bonus`, `speed_tick`).
//!
//! Who is credited is the damage seam's (`damage`: the killing hit's
//! instigator, or the enemy who did the most damage within the last 6 s
//! when that hit was friendly or had none): `Died::attacker`. The killer
//! (a suicide's test) is the killing hit's own instigator
//! (`Died::damage.attacker`).
//!
//! Known shortcuts: the announcer's lines are told as `HaloAnnounce` (the
//! HUD may show them as text); their sounds are not played
//! (the globals' sound list is not loaded yet).

use avian3d::prelude::ColliderDisabled;
use bevy::prelude::*;
use blam_formats::game_type::{GameTypeOptions, Rules};
use chimera_sim::mode::{self, Match, side};

use super::{HaloGameType, HaloMatch, HaloPlayer};
use crate::core::{Dead, Died, Score, SimClock, Team};

/// Ticks of the multi-kill window (6.1: `last kill >= now - 120`).
pub const MULTIKILL_WINDOW: i64 = 120;
/// No death waits less (4.2).
pub use super::super::respawn::MIN_RESPAWN_TICKS;

/// An announcer line for a kill (6.1, 9), or the game's other lines the
/// rules tell (`HaloAnnounce`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line {
    DoubleKill,
    TripleKill,
    Killtacular,
    KillingSpree,
    RunningRiot,
    GameOver,
    /// Capture the flag: a team has the flag, scored, its flag returned
    /// (the team id: `rules::RED` or `BLUE`).
    HasTheFlag(u8),
    Score(u8),
    FlagReturned(u8),
    /// The flag scoring refused (the failure sound).
    FlagFailure,
    /// King of the hill: the hill moved; oddball: a ball spawns, reset.
    HillMove,
    PlayBall,
    BallReset,
}

/// A line for the announcer and the HUD (`for_player`: only that player's
/// HUD; None: everyone's).
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct HaloAnnounce {
    pub line: Line,
    pub player: Option<Entity>,
}

/// The line a kill of an enemy plays (6.1): first match of multi-kill
/// count 4 or more, 3, 2, kills in a row exactly 5, a multiple of 5.
pub fn announcement(multi_kill: u32, in_a_row: u32) -> Option<Line> {
    match multi_kill {
        4.. => Some(Line::Killtacular),
        3 => Some(Line::TripleKill),
        2 => Some(Line::DoubleKill),
        _ if in_a_row == 5 => Some(Line::KillingSpree),
        _ if in_a_row >= 1 && in_a_row.is_multiple_of(5) => Some(Line::RunningRiot),
        _ => None,
    }
}

/// The multi-kill count after a kill at `now` by a player whose last
/// kill was at `last` with count `count` (6.1).
pub fn multi_kill(last: Option<i64>, count: u32, now: i64) -> u32 {
    match last {
        Some(t) if t >= now - MULTIKILL_WINDOW => count + 1,
        _ => 1,
    }
}

/// The respawn timer of a death (4.2), in ticks: `penalty` is the dead
/// player's P (updated), `killer_penalty` the credited enemy's P (updated)
/// when the death was a kill.
pub fn respawn_timer(r: &Rules, penalty: &mut i32, killer_penalty: Option<&mut i32>, kill: bool) -> i32 {
    let g = r.respawn_time_growth;
    let mut timer = *penalty + r.respawn_time;
    if g > 0 {
        *penalty = (*penalty + g).min(5 * g);
        if kill && let Some(k) = killer_penalty {
            *k = (*k - g).max(0);
        }
    }
    if !kill {
        timer += r.suicide_penalty;
    }
    timer.max(MIN_RESPAWN_TICKS)
}

/// Slayer's kill penalty on a killer's speed multiplier (6.2).
pub fn kill_penalty(m: f32) -> f32 {
    let mut m = m - 0.02;
    if m >= 1.0 {
        m = (m - 0.15).max(1.0);
    }
    m.max(0.9)
}

/// Slayer's death bonus on a dead player's speed multiplier (6.2).
pub fn death_bonus(m: f32) -> f32 {
    let mut m = m + 0.1;
    if m <= 1.0 {
        m = (m + 0.1).min(1.0);
    }
    m.min(1.5)
}

/// A tick's drift back to 1.0 of a speed multiplier when the option that
/// moved it is off (6.2).
pub fn speed_tick(m: f32, kill_penalty_on: bool, death_bonus_on: bool) -> f32 {
    if !kill_penalty_on && m > 1.0 {
        (m - 0.000111).max(1.0)
    } else if !death_bonus_on && m < 1.0 {
        (m + 0.0000111).min(1.0)
    } else {
        m
    }
}

/// Slayer's options: (kill penalty on, death bonus on, kill in order).
pub fn slayer_options(r: &Rules) -> Option<(bool, bool, bool)> {
    match r.options {
        GameTypeOptions::Slayer { no_death_bonus, no_kill_penalty, kill_in_order } => {
            Some((!no_kill_penalty, !no_death_bonus, kill_in_order))
        }
        _ => None,
    }
}

/// Whether `a` and `b` (engine team ids) are teammates in this game:
/// only in a team game, on the same side.
pub fn teammates(team_play: bool, a: u8, b: u8) -> bool {
    team_play && side(a).is_some() && a == b
}

fn team_of(world: &World, e: Entity) -> u8 {
    world.get::<Team>(e).map_or(0, |t| t.0)
}

/// Add `n` to `e`'s score and its team's (6.2: free for all, the two are
/// one).
pub fn add_score(world: &mut World, e: Entity, n: i32) {
    let team = team_of(world, e);
    if let Some(mut p) = world.get_mut::<HaloPlayer>(e) {
        p.score += n;
    }
    if let Some(mut s) = world.get_mut::<Score>(e) {
        s.kills += n;
    }
    if let Some(i) = side(team)
        && let Some(mut m) = world.get_resource_mut::<Match>()
    {
        m.score[i] += n;
    }
}

/// The tick's deaths (after the damage), each in order.
pub(super) fn deaths(mut reader: MessageReader<Died>, mut commands: Commands) {
    for d in reader.read() {
        let d = d.clone();
        commands.queue(move |w: &mut World| died(w, &d));
    }
}

/// A Halo player died (6.1, then the game type's handler, the respawn
/// timer 4.2, the announcer): marked dead (no unit).
pub fn died(world: &mut World, d: &Died) {
    let victim = d.entity;
    if world.get::<HaloPlayer>(victim).is_none() || world.get::<Dead>(victim).is_some() {
        return;
    }
    let now = world.resource::<HaloMatch>().time;
    let r = world.resource::<HaloGameType>().0.rules;
    let v_team = team_of(world, victim);
    let killer = d.damage.attacker;
    let credited = d.attacker.filter(|a| world.get::<HaloPlayer>(*a).is_some());
    // 1. The dead player's counters.
    if let Some(mut p) = world.get_mut::<HaloPlayer>(victim) {
        p.deaths += 1;
        p.kills_in_row = 0;
        p.last_kill = None;
        p.multi_kill = 0;
        if killer == Some(victim) {
            p.suicides += 1;
        }
    }
    if let Some(mut s) = world.get_mut::<Score>(victim) {
        s.deaths += 1;
    }
    // 2. The credit: a teammate or oneself is friendly fire.
    let friendly = credited.is_some_and(|k| k == victim || teammates(r.team_play, team_of(world, k), v_team));
    let mut line = None;
    if let Some(k) = credited
        && let Some(mut p) = world.get_mut::<HaloPlayer>(k)
    {
        if friendly {
            p.betrayals += 1;
        } else {
            p.kills += 1;
            p.kills_in_row += 1;
            p.multi_kill = multi_kill(p.last_kill, p.multi_kill, now);
            p.last_kill = Some(now);
            line = announcement(p.multi_kill, p.kills_in_row);
        }
    }
    // 3. The game type's handler.
    if r.base() == blam_formats::game_type::BaseGameType::Slayer {
        slayer_kill(world, credited, victim, friendly);
    }
    // 4. The respawn timer.
    let kill = credited.is_some() && !friendly;
    let mut killer_p = credited.filter(|_| kill).and_then(|k| world.get::<HaloPlayer>(k).map(|p| p.penalty));
    let mut p = world.get::<HaloPlayer>(victim).map_or(0, |p| p.penalty);
    let timer = respawn_timer(&r, &mut p, killer_p.as_mut(), kill);
    if let Some(mut v) = world.get_mut::<HaloPlayer>(victim) {
        v.penalty = p;
        v.respawn = timer;
    }
    if let (Some(k), Some(kp)) = (credited, killer_p)
        && let Some(mut kp_) = world.get_mut::<HaloPlayer>(k)
    {
        kp_.penalty = kp;
    }
    // 5. The announcer.
    if let (Some(line), Some(k)) = (line, credited) {
        world.write_message(HaloAnnounce { line, player: Some(k) });
    }
    // The local player's countdown (the shell's death view).
    let now = world.resource::<SimClock>().now;
    if world.get::<crate::core::LocalPlayer>(victim).is_some()
        && let Some(mut due) = world.get_resource_mut::<crate::core::RespawnDue>()
    {
        due.0 = Some(crate::core::RespawnAt { at: now + timer as f64 / 30.0, delay: timer as f64 / 30.0 });
    }
    if r.odd_man_out {
        world.resource_mut::<HaloMatch>().odd_man = Some(victim);
    }
    let since = world.resource::<SimClock>().now;
    world.entity_mut(victim).insert((Dead { since }, ColliderDisabled));
    if let Some(id) = mode::current_mode(world).map(|m| m.id) {
        mode::mode_death(world, id, d);
    }
}

/// Slayer's kill handler (6.2): an enemy kill scores +1 (kill in order:
/// only the target's, then a new target) with the speed rule; a friendly
/// fire credit -1; no credit, nothing. Scores change in the postgame too.
pub fn slayer_kill(world: &mut World, credited: Option<Entity>, victim: Entity, friendly: bool) {
    let Some(k) = credited else { return };
    let r = world.resource::<HaloGameType>().0.rules;
    let Some((penalty_on, bonus_on, in_order)) = slayer_options(&r) else { return };
    if friendly {
        add_score(world, k, -1);
        return;
    }
    if penalty_on && let Some(mut m) = world.get_mut::<super::super::movement::SpeedMultiplier>(k) {
        m.0 = kill_penalty(m.0);
    }
    if bonus_on && let Some(mut m) = world.get_mut::<super::super::movement::SpeedMultiplier>(victim) {
        m.0 = death_bonus(m.0);
    }
    if in_order {
        if world.get::<HaloPlayer>(k).and_then(|p| p.target) == Some(victim) {
            add_score(world, k, 1);
            let next = super::slayer::pick_target(world, k, Some(victim));
            if let Some(mut p) = world.get_mut::<HaloPlayer>(k) {
                p.target = next;
            }
        }
    } else {
        add_score(world, k, 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blam_formats::game_type::STOCK_GAME_TYPES;

    fn stock(name: &str) -> Rules {
        STOCK_GAME_TYPES.iter().find(|s| s.name == name).unwrap().rules
    }

    #[test]
    fn multi_kill_window() {
        assert_eq!(multi_kill(None, 0, 1000), 1);
        assert_eq!(multi_kill(Some(1100), 1, 1220), 2);
        assert_eq!(multi_kill(Some(1100), 1, 1221), 1);
    }

    #[test]
    fn respawn_of_slayer_and_team_slayer() {
        let (s, t) = (stock("Slayer"), stock("Team Slayer"));
        assert_eq!(respawn_timer(&s, &mut 0, None, true), 90);
        assert_eq!(respawn_timer(&t, &mut 0, None, true), 300);
        assert_eq!(respawn_timer(&s, &mut 0, None, false), 300);
        assert_eq!(respawn_timer(&t, &mut 0, None, false), 600);
    }
}
