//! The Flame Thrower (specs/tf2/weapons_more.md 7), the hook `tf2:flame`
//! (`FlameParams`, the weapon part `Tf2Flame`), reduced as 7.1 allows: a
//! cone tested every flame tick, not flying particles.
//!
//! - **Firing**: the frame fires a round of the 200 every 0.08 s while
//!   attack is held (12.5 ammo a second; its rounds trace nothing). The gun
//!   is "flaming" while its last round is at most a round's interval old
//!   (`Tf2Gun::last_shot`).
//! - **Damage** (`burn_cone`, the server's): every `tick` (0.075 s) of
//!   flaming, each living enemy whose centre is within `range` (350) ahead
//!   of the eye along the aim, at most `reach` (24, half a player's width)
//!   plus `cone` (2.8 degrees) of the distance off the aim line, with a
//!   clear line from the eye, takes `damage` x g_age x g_burn rounded
//!   (`tick_damage`): g_age = max(`age_floor`, 1 - age / `age_full`), the
//!   age the distance over the flame's speed (`range` / `life`), g_burn =
//!   `ramp_start` + (1 - `ramp_start`) min(T / `ramp_time`, 1), T the time
//!   it has been hit without a break (`Scorched`); of kind burn, naming the
//!   gun. Then it burns (`Burning`): its remaining burn becomes max(it,
//!   `burn_min`) plus the tick, at most `burn_max`.
//! - **Afterburn** (`afterburn`): `burn_damage` (4) every `burn_interval`
//!   (0.5 s) while it burns, credited to the Pyro with the Flame Thrower;
//!   gone at death.
//! - **Sounds** (`sounds`, predicted): the start and the loop when it
//!   starts flaming, the end when it stops (the weapon channel).
//!
//! The flames' look is `looks`' (TF2's `flamethrower` system, a piece per
//! round).
//!
//! Known shortcuts: the flames drawn are a look only (the damage is the
//! cone's, not the particles'); the
//! Pyro's velocity isn't added; no airblast (attack2); Pyros aren't immune
//! to afterburn (no classes on the victims); water and health kits don't
//! put a burn out; crits are off.

use bevy::prelude::*;
use serde::Deserialize;

use avian3d::prelude::*;
use chimera_sim::weapon::{CHAN_ITEM, CHAN_WEAPON, Inventory, NetPart, Trigger, Weapon, aim_of};

use super::damage::{HP, Tf2Gun, U, round_points};
use crate::{
    core::{Damage, DamageKind, FirstTimePredicted, FriendlyFire, Health, Hitgroup, Intent, MovementState, SOLID_LAYERS, SimClock, Team},
    map::{PlaySound, PredictedSound},
};

/// `tf2:flame`'s parameters (units, points, seconds, degrees).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FlameParams {
    pub damage: f32,
    pub tick: f32,
    pub range: f32,
    pub cone: f32,
    pub reach: f32,
    pub life: f32,
    pub age_floor: f32,
    pub age_full: f32,
    pub ramp_start: f32,
    pub ramp_time: f32,
    pub burn_damage: f32,
    pub burn_interval: f32,
    pub burn_min: f32,
    pub burn_max: f32,
    #[serde(default)]
    pub start_sound: Option<String>,
    #[serde(default)]
    pub loop_sound: Option<String>,
    #[serde(default)]
    pub end_sound: Option<String>,
    #[serde(default)]
    pub hit_sound: Option<String>,
}

impl FlameParams {
    /// One flame tick's damage (points, unrounded) at `distance` units on a
    /// victim hit for `burning` seconds without a break.
    pub fn tick_damage(&self, distance: f32, burning: f32) -> f32 {
        let age = distance / (self.range / self.life);
        let g_age = (1.0 - age / self.age_full).max(self.age_floor);
        let g_burn = self.ramp_start + (1.0 - self.ramp_start) * (burning / self.ramp_time).clamp(0.0, 1.0);
        self.damage * g_age * g_burn
    }

    /// The burn left after a tick that hits a victim with `left` seconds
    /// of burn.
    pub fn burn_after(&self, left: f32) -> f32 {
        (left.max(self.burn_min) + self.tick).min(self.burn_max)
    }
}

/// The Flame Thrower's state (a predicted part): flaming, the next flame
/// tick.
#[derive(Component, Clone, Debug)]
pub struct Tf2Flame {
    pub params: FlameParams,
    pub flaming: bool,
    pub next_tick: f64,
}

impl NetPart for Tf2Flame {
    type Net = (bool, f64);
    fn net(&self) -> Self::Net {
        (self.flaming, self.next_tick)
    }
    fn set_net(&mut self, (flaming, next_tick): Self::Net) {
        self.flaming = flaming;
        self.next_tick = next_tick;
    }
}

/// `tf2:flame` on a weapon.
pub fn attach(e: &mut EntityWorldMut, p: &FlameParams) {
    e.insert(Tf2Flame { params: p.clone(), flaming: false, next_tick: f64::MIN });
}

/// On a character in the flames: since when, and when last touched.
#[derive(Component, Clone, Copy, Debug)]
pub struct Scorched {
    pub since: f64,
    pub last: f64,
}

/// On a burning character: until when, the next afterburn tick, by whom
/// and with what, its damage and interval.
#[derive(Component, Clone, Copy, Debug)]
pub struct Burning {
    pub until: f64,
    pub next: f64,
    pub attacker: Option<Entity>,
    pub weapon: &'static str,
    pub damage: f32,
    pub interval: f32,
}

/// Whether a gun is flaming at `now`: its last round at most an interval
/// (and a tick of slack) old.
fn flaming(gun: &Tf2Gun, trigger: Option<&Trigger>, now: f64) -> bool {
    let cycle = trigger.map_or(0.08, |t| t.cycle) as f64;
    now - gun.last_shot <= cycle + 0.016
}

/// The flaming state and its sounds (predicted, after the frame).
#[allow(clippy::type_complexity)]
pub fn sounds(
    owners: Query<(Entity, &Inventory, &Transform)>,
    mut guns: Query<(&mut Tf2Flame, &Tf2Gun, Option<&Trigger>)>,
    clock: Res<SimClock>,
    first: Res<FirstTimePredicted>,
    mut out: MessageWriter<PredictedSound>,
) {
    for (owner, inv, at) in &owners {
        for &w in &inv.weapons {
            let Ok((mut f, gun, trigger)) = guns.get_mut(w) else { continue };
            let now = inv.active == Some(w) && flaming(gun, trigger, clock.now);
            if now == f.flaming {
                continue;
            }
            f.flaming = now;
            if !first.0 {
                continue;
            }
            let entries: Vec<String> = if now {
                f.params.start_sound.iter().chain(f.params.loop_sound.iter()).cloned().collect()
            } else {
                f.params.end_sound.iter().cloned().collect()
            };
            for (k, e) in entries.into_iter().enumerate() {
                out.write(PredictedSound {
                    by: owner,
                    sound: PlaySound {
                        entry: e,
                        at: Some(at.translation),
                        volume: None,
                        pitch: None,
                        source: Some(owner),
                        // The start on its own channel, the loop and the
                        // end replacing each other on the weapon's.
                        channel: Some(if now && k == 0 { CHAN_ITEM } else { CHAN_WEAPON }),
                    },
                });
            }
        }
    }
}

/// The flame ticks' damage and ignition (the server's).
#[allow(clippy::type_complexity)]
pub fn burn_cone(
    owners: Query<(Entity, &Inventory, &Intent, &Transform, Option<&MovementState>, Option<&Team>, Option<&Health>)>,
    mut guns: Query<(&mut Tf2Flame, &Tf2Gun, &Weapon, Option<&Trigger>)>,
    victims: Query<(Entity, &Transform, Option<&ColliderAabb>, &Health, Option<&Team>, Option<&Scorched>, Option<&Burning>), With<Intent>>,
    spatial: SpatialQuery,
    colliders: Query<&ColliderOf>,
    friendly_fire: Option<Res<FriendlyFire>>,
    clock: Res<SimClock>,
    mut damage: MessageWriter<Damage>,
    mut commands: Commands,
) {
    let now = clock.now;
    let ff = friendly_fire.is_some_and(|f| f.0 != 0);
    for (owner, inv, intent, at, state, team, health) in &owners {
        let Some(w) = inv.active else { continue };
        let Ok((mut f, gun, weapon, trigger)) = guns.get_mut(w) else { continue };
        if health.is_some_and(|h| h.current <= 0.0) || !flaming(gun, trigger, now) || now + 1e-6 < f.next_tick {
            continue;
        }
        let p = f.params.clone();
        f.next_tick = if f.next_tick + (p.tick as f64) * 2.0 < now { now + p.tick as f64 } else { f.next_tick + p.tick as f64 };
        let eye = at.translation + state.map_or(Vec3::ZERO, |s| s.eye_offset);
        let dir = aim_of(intent.yaw, intent.pitch) * Vec3::NEG_Z;
        let filter = SpatialQueryFilter::default().with_mask(SOLID_LAYERS);
        let tan = p.cone.to_radians().tan();
        let mine = team.copied().filter(|t| t.0 != 0);
        for (v, vt, aabb, vh, vteam, scorched, burning) in &victims {
            if v == owner || vh.current <= 0.0 || (!ff && mine.is_some() && vteam.copied() == mine) {
                continue;
            }
            let c = aabb.map_or(vt.translation, |b| (b.min + b.max) / 2.0);
            let along = (c - eye).dot(dir) / U;
            if along <= 0.0 || along > p.range {
                continue;
            }
            let off = ((c - eye) - dir * along * U).length() / U;
            if off > p.reach + along * tan {
                continue;
            }
            if let Ok(d) = Dir3::new(c - eye) {
                let blocked = spatial
                    .cast_ray_predicate(eye, d, c.distance(eye), true, &filter, &|e| {
                        let body = colliders.get(e).map_or(e, |o| o.body);
                        !victims.contains(body) && body != owner
                    })
                    .is_some();
                if blocked {
                    continue;
                }
            }
            let since = match scorched {
                Some(s) if now - s.last <= p.tick as f64 * 1.5 => s.since,
                _ => now,
            };
            let amount = round_points(p.tick_damage(along, (now - since) as f32) * HP);
            if amount > 0.0 {
                damage.write(Damage {
                    target: v,
                    attacker: Some(owner),
                    amount,
                    point: c,
                    dir,
                    hitgroup: Hitgroup::Generic,
                    kind: DamageKind::Burn,
                    weapon: Some(weapon.id),
                    force: Vec3::ZERO,
                });
            }
            let left = burning.map_or(0.0, |b| (b.until - now).max(0.0) as f32);
            let next = burning.map_or(now + p.burn_interval as f64, |b| b.next);
            commands.entity(v).insert((
                Scorched { since, last: now },
                Burning {
                    until: now + p.burn_after(left) as f64,
                    next,
                    attacker: Some(owner),
                    weapon: weapon.id,
                    damage: p.burn_damage,
                    interval: p.burn_interval,
                },
            ));
        }
    }
}

/// Afterburn (the server's).
pub fn afterburn(
    mut burning: Query<(Entity, &mut Burning, &Health, &Transform)>,
    clock: Res<SimClock>,
    mut damage: MessageWriter<Damage>,
    mut commands: Commands,
) {
    let now = clock.now;
    for (e, mut b, h, at) in &mut burning {
        if h.current <= 0.0 {
            commands.entity(e).remove::<(Burning, Scorched)>();
            continue;
        }
        while b.next <= now + 1e-6 && b.next <= b.until + 1e-6 {
            damage.write(Damage {
                target: e,
                attacker: b.attacker,
                amount: b.damage * HP,
                point: at.translation,
                dir: Vec3::Y,
                hitgroup: Hitgroup::Generic,
                kind: DamageKind::Burn,
                weapon: Some(b.weapon),
                force: Vec3::ZERO,
            });
            b.next += b.interval as f64;
        }
        if now + 1e-6 >= b.until {
            commands.entity(e).remove::<Burning>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> FlameParams {
        super::super::hook_params(super::super::FLAMETHROWER, super::super::FLAME_HOOK)
    }

    /// 7.2: 13 fresh and fully ramped, half at the start of contact, the
    /// age factor's floor 0.5.
    #[test]
    fn a_flame_tick_ranges_from_6_5_to_13() {
        let p = params();
        assert!((p.tick_damage(0.0, 1.5) - 13.0).abs() < 1e-4);
        assert!((p.tick_damage(0.0, 0.0) - 6.5).abs() < 1e-4);
        assert!((p.tick_damage(350.0, 1.5) - 6.5).abs() < 1e-4);
    }

    /// 7.3, tests 44, 45: one tick burns 3 s and more; long contact caps at
    /// 10.
    #[test]
    fn the_burn_lasts_3_to_10_s_44_45() {
        let p = params();
        assert!((p.burn_after(0.0) - 3.075).abs() < 1e-5);
        assert_eq!(p.burn_after(10.0), 10.0);
    }
}
