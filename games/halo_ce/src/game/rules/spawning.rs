//! When and where a Halo player spawns, and what it holds
//! (specs/halo_ce/multiplayer_rules.md 4.1 to 4.6):
//!
//! - **When** (`players_tick`, each Halo tick, in slot order, for every
//!   player with no unit): never once out of lives, nor while holding odd
//!   man out's place, nor in the postgame; a player with no deaths yet
//!   spawns now; else the respawn timer counts down (the first decrement
//!   on the first tick after the death) and the player may spawn on the
//!   tick it reaches 0, and, after game time 3, only on a tick whose
//!   number is the slot modulo 32 (the stagger). No usable start: the
//!   timer is 1 and the test repeats.
//! - **Where** (`candidates`, `rate`, `choose`): the starts eligible for the
//!   base game type (`GameTypes::eligible_for`); in capture the flag only
//!   the player's team's (index 0 red, 1 blue) and none nearer the enemy's
//!   flag spot than its own; each rated by the distances to the other
//!   living players (an enemy within 2 WU or anyone within 0.25 WU blocks,
//!   anyone within 1 WU x 0.1, an enemy 2 to 5 WU away x (d - 2) / 3; a
//!   teammate beyond 0.25 WU skipped), the friends bonus in a team game, a
//!   vehicle within 0.1 WU blocking, assault's pull to the own base; then
//!   times u^0.5 (a fresh draw per start from the match's generator), the
//!   largest product above 0 wins, ties to the earlier start.
//! - **What** (`equip`): the multiplayer biped's own weapons (the assault
//!   rifle then the pistol) through the weapon set, a duplicate dropped;
//!   with custom starting equipment (the variant's generic bit clear) the
//!   first eligible entry's draws (the first weapon replacing the unit's,
//!   the rest added); the grenades (`items::starting_grenades`). The unit
//!   is placed on the start's feet facing its heading, alive with a fresh
//!   vitality (`chimera_sim::spawning::revive`).
//!
//! A map with no Halo starts (the greybox) uses its spawn points as starts
//! eligible for every game type (their team as the start's index).

use avian3d::prelude::ColliderDisabled;
use bevy::prelude::*;
use blam_formats::{
    game_type::{GameFlags, GameTypeOptions},
    scenario::{GameType, GameTypes},
};
use chimera_sim::{mode::side, weapon::Inventory};

use super::{
    HaloGameType, HaloMatch, HaloPlayer, items,
    netgame::{Netgame, Start, WU},
    out_of_lives, scoring, variant,
};
use crate::core::{Dead, SpawnPoint, Team};

/// Up to this game time the stagger does not apply.
pub const STAGGER_FREE_UNTIL: i64 = 3;

/// Another player as a start's rating sees it: its distance in world
/// units, whether an enemy (a different team) and whether a teammate (a
/// team game's same side).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Other {
    pub d: f32,
    pub enemy: bool,
    pub teammate: bool,
}

/// A start's rating before the random draw (4.4 steps 1 to 4): `others`
/// the other living players, `vehicles` the distances (WU) to every
/// vehicle.
pub fn rate(others: &[Other], team_game: bool, vehicles: &[f32]) -> f32 {
    let mut rating = 1.0f32;
    for q in others {
        if team_game && q.teammate && q.d > 0.25 {
            continue;
        }
        if q.d < 0.25 {
            rating = 0.0;
        } else if q.d < 1.0 {
            rating *= 0.1;
        }
        if q.enemy {
            if q.d < 2.0 {
                rating = 0.0;
            } else if q.d <= 5.0 {
                rating *= (q.d - 2.0) / 3.0;
            }
        }
    }
    if team_game && rating > 0.0 {
        let sum: f32 = others
            .iter()
            .filter(|q| q.teammate && (1.0..=6.0).contains(&q.d))
            .map(|q| (1.0 - 0.2 * (q.d - 1.0)).powf(0.6))
            .sum();
        rating *= 3.0 * sum.min(3.0) + 1.0;
    }
    if vehicles.iter().any(|d| *d < 0.1) {
        rating = 0.0;
    }
    rating
}

/// Assault capture the flag's factor (4.4 step 5): `d2` the squared
/// distance (WU) from the start to the player's own flag spot.
pub fn assault_factor(d2: f32, time: i64) -> f32 {
    let d2 = d2.clamp(0.5, 10.0);
    let mut r = 1.0 / d2;
    if time > 30 && d2 > 1.0 {
        r = r.powf(0.33);
    }
    r.clamp(0.5, 2.0)
}

/// The start that wins (4.4): the largest rating x draw^0.5 above 0, ties
/// to the earlier; `draws` one per rating.
pub fn choose(ratings: &[f32], draws: &[f32]) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, (r, u)) in ratings.iter().zip(draws).enumerate() {
        let p = r * u.sqrt();
        if p > 0.0 && best.is_none_or(|(_, b)| p > b) {
            best = Some((i, p));
        }
    }
    best.map(|(i, _)| i)
}

/// The engine team of a capture the flag team index (0 red, 1 blue).
pub fn team_of_index(index: i16) -> Option<u8> {
    match index {
        0 => Some(super::RED),
        1 => Some(super::BLUE),
        _ => None,
    }
}

fn distance2(a: Vec3, b: Vec3) -> f32 {
    a.distance_squared(b) / (WU * WU)
}

/// Whether a capture the flag start survives the game's check (4.3): not
/// nearer the enemy team's flag spot than its own (both spots known).
pub fn ctf_usable(n: &Netgame, s: &Start) -> bool {
    if !matches!(s.team, 0 | 1) {
        return false;
    }
    match (n.flag_spot(s.team), n.flag_spot(1 - s.team)) {
        (Some(own), Some(enemy)) => distance2(s.at, enemy.at) >= distance2(s.at, own.at),
        _ => true,
    }
}

/// The map's starts: its Halo starts, else its spawn points (eligible for
/// every game type).
pub fn starts(world: &mut World) -> Vec<Start> {
    if let Some(n) = world.get_resource::<Netgame>()
        && !n.starts.is_empty()
    {
        return n.starts.clone();
    }
    let lift = crate::map::SPAWN_LIFT;
    world
        .query::<(&Transform, &SpawnPoint)>()
        .iter(world)
        .map(|(t, s)| Start {
            at: t.translation - Vec3::Y * lift,
            yaw: t.rotation.to_euler(EulerRot::YXZ).0,
            team: match s.team.map(|t| t.0) {
                Some(super::RED) => 0,
                Some(super::BLUE) => 1,
                _ => 2,
            },
            game_types: GameTypes([blam_formats::scenario::netgame::SLOT_ALL, 0, 0, 0]),
        })
        .collect()
}

/// The starts `e` may use in `game` (4.3), by index into `all`.
pub fn candidates(n: Option<&Netgame>, all: &[Start], game: GameType, team: u8) -> Vec<usize> {
    let empty = Netgame::default();
    let n = n.unwrap_or(&empty);
    (0..all.len())
        .filter(|i| {
            let s = &all[*i];
            if !s.game_types.eligible_for(game) {
                return false;
            }
            if game != GameType::CaptureTheFlag {
                return true;
            }
            team_of_index(s.team) == Some(team) && ctf_usable(n, s)
        })
        .collect()
}

/// The feet of a unit (its `Transform` is the pill's centre).
pub fn feet(t: &Transform) -> Vec3 {
    t.translation - Vec3::Y * centre_above_feet()
}

/// Metres from the feet to a standing unit's centre.
pub fn centre_above_feet() -> f32 {
    super::super::movement::MovementData::builtin().centre_above_origin(0.0) * WU
}

/// Choose `e`'s start now (4.3, 4.4), or None.
pub fn pick_start(world: &mut World, e: Entity) -> Option<Start> {
    let r = world.resource::<HaloGameType>().0.rules;
    let game = variant::game_type(&r);
    let team = world.get::<Team>(e).map_or(0, |t| t.0);
    let all = starts(world);
    let list = candidates(world.get_resource::<Netgame>(), &all, game, team);
    if list.is_empty() {
        return None;
    }
    let others: Vec<(Vec3, u8)> = world
        .query_filtered::<(Entity, &Transform, &Team), (With<HaloPlayer>, Without<Dead>)>()
        .iter(world)
        .filter(|(o, ..)| *o != e)
        .map(|(_, t, team)| (feet(t), team.0))
        .collect();
    let vehicles: Vec<Vec3> = world
        .query_filtered::<&Transform, With<super::super::vehicles::HaloVehicle>>()
        .iter(world)
        .map(|t| t.translation)
        .collect();
    let assault = matches!(r.options, GameTypeOptions::CaptureTheFlag { assault: true, .. });
    let own_spot = side(team).and_then(|i| world.get_resource::<Netgame>()?.flag_spot(i as i16).map(|f| f.at));
    let time = world.resource::<HaloMatch>().time;
    let ratings: Vec<f32> = list
        .iter()
        .map(|i| {
            let s = &all[*i];
            let o: Vec<Other> = others
                .iter()
                .map(|(at, t)| Other {
                    d: at.distance(s.at) / WU,
                    enemy: !scoring::teammates(r.team_play, *t, team),
                    teammate: scoring::teammates(r.team_play, *t, team),
                })
                .collect();
            let v: Vec<f32> = vehicles.iter().map(|p| p.distance(s.at) / WU).collect();
            let mut rating = rate(&o, r.team_play, &v);
            if assault && let Some(spot) = own_spot {
                rating *= assault_factor(distance2(s.at, spot), time);
            }
            rating
        })
        .collect();
    let draws: Vec<f32> = {
        let mut m = world.resource_mut::<HaloMatch>();
        list.iter().map(|_| m.real()).collect()
    };
    choose(&ratings, &draws).map(|i| all[list[i]].clone())
}

/// Spawn `e` now if a start is usable (4.3 to 4.6): true when it did.
pub fn spawn_now(world: &mut World, e: Entity) -> bool {
    let Some(start) = pick_start(world, e) else { return false };
    place(world, e, &start);
    true
}

/// `e` alive at `start`, equipped.
pub fn place(world: &mut World, e: Entity, start: &Start) {
    let at = Transform::from_translation(start.at + Vec3::Y * centre_above_feet()).with_rotation(Quat::from_rotation_y(start.yaw));
    chimera_sim::spawning::revive(world, e, Some(at));
    world.entity_mut(e).remove::<ColliderDisabled>();
    equip(world, e);
    if let Some(mut p) = world.get_mut::<HaloPlayer>(e) {
        p.respawn = 0;
    }
    if world.get::<crate::core::LocalPlayer>(e).is_some()
        && let Some(mut due) = world.get_resource_mut::<crate::core::RespawnDue>()
    {
        due.0 = None;
    }
}

/// Draw one item tag from a collection's (weight, tag) list (5.2).
fn draw(items: &[(f32, String)], world: &mut World) -> Option<String> {
    let sum: f32 = items.iter().map(|i| i.0).sum();
    let total = if sum.is_finite() && sum > 0.0 { sum as i32 } else { 0 };
    if total == 0 {
        return None;
    }
    let mut r = world.resource_mut::<HaloMatch>().int_range(0, total) as f32;
    for (w, tag) in items {
        r -= w;
        if r < 0.0 {
            return Some(tag.clone());
        }
    }
    None
}

/// What the unit holds on spawning (4.6).
pub fn equip(world: &mut World, e: Entity) {
    use super::super::weapons::{ASSAULT_RIFLE, PISTOL, grenades::GrenadeData, pickups};
    let r = world.resource::<HaloGameType>().0.rules;
    let players = world.resource::<HaloMatch>().players_at_start;
    // The held weapons go.
    let held: Vec<Entity> = world.get::<Inventory>(e).map(|i| i.weapons.clone()).unwrap_or_default();
    for w in held {
        if world.get_entity(w).is_ok() {
            world.despawn(w);
        }
    }
    if let Some(mut inv) = world.get_mut::<Inventory>(e) {
        inv.weapons.clear();
        inv.active = None;
        inv.last = None;
        inv.wanted = None;
    }
    // 1. The unit's own weapons through the weapon set.
    let mut ids: Vec<&'static str> = Vec::new();
    for id in [ASSAULT_RIFLE, PISTOL] {
        let to = items::substitute(r.weapon_set, id);
        if !ids.contains(&to) {
            ids.push(to);
        }
    }
    // 2. Custom starting equipment.
    let mut flags = None;
    if !r.flags.contains(GameFlags::GENERIC_STARTING_EQUIPMENT) {
        let game = variant::game_type(&r);
        let entry = world.get_resource::<Netgame>().and_then(|n| n.equipment_for(game)).cloned();
        if let Some(entry) = entry {
            flags = Some(entry.flags);
            let grenades = world.get_resource::<GrenadeData>().cloned();
            let spartan = world.get_resource::<super::super::vitality::Spartan>().cloned();
            let mut first = true;
            for c in entry.collections.iter().filter(|c| !c.is_empty()).take(5) {
                let Some(tag) = draw(c, world) else { continue };
                let (Some(g), Some(s)) = (&grenades, &spartan) else { continue };
                let Some(pickups::Item::Weapon { id, .. }) = pickups::item_of(&tag, g, s) else { continue };
                let id = items::substitute(r.weapon_set, id);
                if first {
                    ids.clear();
                    first = false;
                }
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
    }
    let mut given = Vec::new();
    for id in &ids {
        if let Some(w) = chimera_sim::weapon::give(world, e, id) {
            given.push(w);
        }
    }
    if let (Some(first), Some(mut inv)) = (given.first().copied(), world.get_mut::<Inventory>(e)) {
        inv.wanted = Some(first);
    }
    // 3. The grenades.
    let counts = items::starting_grenades(&r, players, flags);
    if let Some(mut h) = world.get_mut::<super::super::weapons::HaloHands>(e) {
        h.throw.counts = counts;
        h.armed = true;
    }
    if let Some(mut p) = world.get_mut::<HaloPlayer>(e) {
        p.grenades = counts;
    }
}

/// Whether a player in `slot` whose timer allows it spawns at game time
/// `time` (4.1 step 6): up to time 3 always, then only when the time is
/// the slot modulo 32.
pub fn stagger_allows(time: i64, slot: u8) -> bool {
    time <= STAGGER_FREE_UNTIL || super::super::respawn::staggered(time.max(0) as u32, slot as u32)
}

/// The players' step of a Halo tick (4.1): every player with no unit, in
/// slot order, may spawn.
pub fn players_tick(world: &mut World) {
    let (time, over) = {
        let m = world.resource::<HaloMatch>();
        (m.time, m.over)
    };
    if over {
        return;
    }
    let r = world.resource::<HaloGameType>().0.rules;
    let odd = world.resource::<HaloMatch>().odd_man;
    let mut dead: Vec<(u8, Entity)> = world
        .query_filtered::<(Entity, &HaloPlayer), With<Dead>>()
        .iter(world)
        .map(|(e, p)| (p.slot, e))
        .collect();
    dead.sort();
    for (slot, e) in dead {
        let Some(p) = world.get::<HaloPlayer>(e).cloned() else { continue };
        if out_of_lives(&r, &p) || (r.odd_man_out && odd == Some(e)) {
            continue;
        }
        if p.deaths > 0 {
            let mut timer = p.respawn;
            if timer > 0 {
                timer -= 1;
                world.get_mut::<HaloPlayer>(e).unwrap().respawn = timer;
                if timer > 0 {
                    continue;
                }
            }
            if !stagger_allows(time, slot) {
                continue;
            }
        }
        if !spawn_now(world, e) {
            world.get_mut::<HaloPlayer>(e).unwrap().respawn = 1;
        }
    }
}

/// The game type's player settings each tick (4.7), after the vitality's
/// update: no shields (shield 0), invisible players (camouflage that
/// never lapses), infinite grenades (the spawn's counts kept).
#[allow(clippy::type_complexity)]
pub fn player_settings(
    game: Option<Res<HaloGameType>>,
    mut q: Query<
        (
            &HaloPlayer,
            Option<&mut super::super::vitality::Vitality>,
            Option<&mut chimera_sim::weapon::Armor>,
            Option<&mut super::super::vitality::Mirrored>,
            Option<&mut super::super::weapons::HaloHands>,
        ),
        Without<Dead>,
    >,
) {
    let Some(r) = game.map(|g| g.0.rules) else { return };
    let no_shields = r.flags.contains(GameFlags::NO_SHIELDS);
    let invisible = r.flags.contains(GameFlags::INVISIBLE_PLAYERS);
    let infinite = r.flags.contains(GameFlags::INFINITE_GRENADES);
    if !(no_shields || invisible || infinite) {
        return;
    }
    for (p, v, armor, mirrored, hands) in &mut q {
        if let Some(mut v) = v {
            if no_shields && v.shield != 0.0 {
                v.shield = 0.0;
                v.overcharging = false;
            }
            if invisible {
                v.camouflaged = true;
                v.camo_ticks = v.camo_ticks.max(15);
            }
        }
        if no_shields {
            if let Some(mut a) = armor
                && a.amount != 0.0
            {
                a.amount = 0.0;
            }
            if let Some(mut m) = mirrored {
                m.shield = 0.0;
            }
        }
        if infinite && let Some(mut h) = hands {
            for k in 0..2 {
                if h.throw.counts[k] < p.grenades[k] {
                    h.throw.counts[k] = p.grenades[k];
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_distances() {
        let enemy = |d| [Other { d, enemy: true, teammate: false }];
        assert_eq!(rate(&enemy(2.0), false, &[]), 0.0);
        assert_eq!(rate(&enemy(5.0), false, &[]), 1.0);
        assert_eq!(rate(&enemy(0.5), false, &[]), 0.0);
        let mate = |d| [Other { d, enemy: false, teammate: true }];
        assert_eq!(rate(&mate(0.2), true, &[]), 0.0);
        assert_eq!(rate(&mate(0.5), true, &[]), 1.0);
        assert_eq!(rate(&[], false, &[0.05]), 0.0);
        assert_eq!(rate(&[], false, &[0.15]), 1.0);
    }
}
