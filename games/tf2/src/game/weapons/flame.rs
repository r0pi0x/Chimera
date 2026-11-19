//! The Flame Thrower (specs/tf2/weapons_more.md 7 and its corrections from
//! the SDK), the hook `tf2:flame` (`FlameParams`, the weapon part
//! `Tf2Flame`), as flying flame points:
//!
//! - **Emission**: the frame fires a round every `emit` (0.02 s, `set`
//!   timing: every 2 ticks) while attack is held; each is a flame point
//!   (`flames`, the server's). **Ammo** (`ammo`, predicted, after the
//!   frame): the round is given back and `ammo_rate` x `emit` (0.28) added
//!   to an accumulator whose whole units leave the clip (the rest kept to
//!   two decimals): about 9.3 a second, 200 last about 21.4 s.
//! - **A point** starts `origin` (40 forward, 5 right) of the eye at
//!   `speed` (2450) along the aim, each screen axis deviated by
//!   (u1 + u2) x `spread` degrees (u uniform in +-0.5), its life `life` +-
//!   `life_jitter` (0.6 +- 0.1 s), at most `max_points` (30) a Pyro. Each
//!   tick it moves by its velocity + `drift` (50) up + the Pyro's velocity
//!   along its direction at its emission, then its own velocity x (1 -
//!   `drag` x tick) (8.5): about 340 units of reach. It dies at its life's
//!   end or on the world.
//! - **Hits** (`flames`): each tick every point whose swept box (half-size
//!   `size`, 12) meets an enemy's box touches him; the oldest is the one
//!   that hits. A victim being burned has a **heat** H per Pyro: + 1 per
//!   touching point (up to + 2 for one younger than `fresh`, 0.02 s), -
//!   `heat_decay` (1.25) a tick; he enters with H 1 at his first damaging
//!   hit and is forgotten `hit_interval` (0.075 s) after his last. A hit
//!   damages at most every `hit_interval`: max(1, `damage` (13) x g_age x
//!   g_heat), g_age = max(0.5, 1 - age / its life), g_heat = 0.5 + 0.5
//!   clamp((H - 10) / 40, 0, 1) (`hit_damage`), rounded, of kind burn.
//!   Teammates: nothing at all.
//! - **Afterburn** (`Burning`, `afterburn`): ignition gives `burn_start`
//!   (3 s), every damaging hit `burn_add` (0.4 s) more, at most
//!   `burn_max` (10 s); every `burn_interval` (0.5 s) from ignition, while
//!   any is left, `burn_damage` (4) and `burn_interval` less of it (one
//!   hit: 3.4 s, 7 ticks, 28); out at waist-deep water. An enemy Pyro
//!   takes the hits but only a `pyro_burn` (0.25 s) look, no afterburn.
//! - **Sounds** (`sounds`, predicted): the start and the loop when it
//!   starts flaming, the end when it stops (the weapon channel); the
//!   start and the loop stop when the gun leaves the hand
//!   (`held_sounds::HeldSounds`: a rung's swap, a death).
//!
//! The flames' look is `looks`' (TF2's `flamethrower` system, a piece per
//! round); the flame points themselves aren't drawn.
//!
//! Known shortcuts: a point stops on the world (the game slides it along
//! and drops the Pyro's part); the line from the eye to the muzzle isn't
//! checked and a press under 0.2 s emits only while held; no airblast
//! (attack2); crits are off; Pyros aren't immune to afterburn (no classes
//! on the victims); health kits and Medics don't put a burn out.

use bevy::prelude::*;
use serde::Deserialize;

use avian3d::prelude::*;
use chimera_sim::weapon::{
    CHAN_ITEM, CHAN_WEAPON, Inventory, held_sounds::HeldSounds, Magazine, NetPart, Trigger, Weapon, WeaponEvent, WeaponEventKind, aim_of,
    random::shared_random,
};
use source_engine::source_movement::SourceMovement;

use super::{
    super::classes::{Classes, Tf2Class},
    blast::boxed,
    damage::{HP, Tf2Gun, U, round_points},
};
use crate::{
    core::{
        Damage, DamageKind, FirstTimePredicted, FriendlyFire, Health, Hitgroup, Intent, MovementState, SOLID_LAYERS, SimClock, Team,
        Velocity,
    },
    map::{PlaySound, PredictedSound},
};

/// The Pyro's class name (classes.toml).
pub const PYRO: &str = "pyro";

/// `tf2:flame`'s parameters (units, points, seconds, degrees).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FlameParams {
    pub damage: f32,
    pub emit: f32,
    pub ammo_rate: f32,
    pub speed: f32,
    pub drag: f32,
    pub drift: f32,
    pub origin: [f32; 2],
    pub spread: f32,
    pub life: f32,
    pub life_jitter: f32,
    pub max_points: usize,
    pub size: f32,
    pub hit_interval: f32,
    pub age_floor: f32,
    pub heat_floor: f32,
    pub heat_low: f32,
    pub heat_high: f32,
    pub heat_decay: f32,
    pub fresh: f32,
    pub min_damage: f32,
    pub burn_damage: f32,
    pub burn_interval: f32,
    pub burn_start: f32,
    pub burn_add: f32,
    pub burn_max: f32,
    pub pyro_burn: f32,
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
    /// A hit's damage (points, unrounded) by a point of `age` of its own
    /// `life` on a victim of heat `heat`.
    pub fn hit_damage(&self, age: f32, life: f32, heat: f32) -> f32 {
        let g_age = (1.0 - age / life.max(1e-6)).max(self.age_floor);
        let span = (self.heat_high - self.heat_low).max(1e-6);
        let g_heat = self.heat_floor + (1.0 - self.heat_floor) * ((heat - self.heat_low) / span).clamp(0.0, 1.0);
        (self.damage * g_age * g_heat).max(self.min_damage)
    }

    /// The heat a touch by a point of `age` adds: 1, up to 2 when fresh.
    pub fn touch_heat(&self, age: f32) -> f32 {
        if age < self.fresh { (2.0 - age / self.fresh).clamp(1.0, 2.0) } else { 1.0 }
    }

    /// The burn left after a damaging hit on a victim with `left` (None:
    /// not burning).
    pub fn burn_after(&self, left: Option<f32>) -> f32 {
        (left.unwrap_or(self.burn_start) + self.burn_add).min(self.burn_max)
    }

    /// How far (units) a point flies in `ticks` ticks of `dt` along its
    /// aim (no drift, a still Pyro).
    pub fn reach(&self, ticks: u32, dt: f32) -> f32 {
        let k = 1.0 - self.drag * dt;
        self.speed * dt * (1.0 - k.powi(ticks as i32)) / (1.0 - k)
    }
}

/// The Flame Thrower's state (a predicted part): flaming, the ammo
/// accumulator.
#[derive(Component, Clone, Debug)]
pub struct Tf2Flame {
    pub params: FlameParams,
    pub flaming: bool,
    pub owed: f32,
}

impl NetPart for Tf2Flame {
    type Net = (bool, f32);
    fn net(&self) -> Self::Net {
        (self.flaming, self.owed)
    }
    fn set_net(&mut self, (flaming, owed): Self::Net) {
        self.flaming = flaming;
        self.owed = owed;
    }
}

/// `tf2:flame` on a weapon.
pub fn attach(e: &mut EntityWorldMut, p: &FlameParams) {
    e.insert(Tf2Flame { params: p.clone(), flaming: false, owed: 0.0 });
    // The start and the loop end when it leaves the hand (`held_sounds`).
    let held: Vec<String> = p.start_sound.iter().chain(p.loop_sound.iter()).cloned().collect();
    if !held.is_empty() {
        e.insert(HeldSounds::new(held));
    }
}

/// One flame point (units, m/s).
#[derive(Clone, Copy, Debug)]
pub struct FlamePoint {
    pub at: Vec3,
    pub before: Vec3,
    pub velocity: Vec3,
    /// The Pyro's velocity along its direction at its emission (kept).
    pub carried: Vec3,
    pub born: f64,
    pub life: f32,
}

/// A victim's heat from one Pyro.
#[derive(Clone, Copy, Debug)]
pub struct Heat {
    pub victim: Entity,
    pub heat: f32,
    pub last_hit: f64,
}

/// The server's flames of one Flame Thrower: its points, the shot they
/// were emitted for, its victims' heat.
#[derive(Component, Clone, Debug, Default)]
pub struct FlameStream {
    pub points: Vec<FlamePoint>,
    pub emitted: f64,
    pub count: u32,
    pub heat: Vec<Heat>,
}

/// On a burning character: the burn left (s), the next afterburn tick,
/// by whom and with what, its damage and interval; `harmless` the enemy
/// Pyro's look only.
#[derive(Component, Clone, Copy, Debug)]
pub struct Burning {
    pub left: f32,
    pub next: f64,
    pub attacker: Option<Entity>,
    pub weapon: &'static str,
    pub damage: f32,
    pub interval: f32,
    pub harmless: bool,
}

/// Whether a gun is flaming at `now`: its last round at most an interval
/// (and a tick of slack) old, its interval rounded up to whole ticks.
fn flaming(gun: &Tf2Gun, trigger: Option<&Trigger>, now: f64, dt: f64) -> bool {
    let cycle = trigger.map_or(0.02, |t| t.cycle) as f64;
    let ticks = (cycle / dt - 1e-6).ceil().max(1.0);
    now - gun.last_shot <= ticks * dt + dt * 0.5
}

/// After the frame (predicted): each round the Flame Thrower fired is
/// given back and its 0.28 of ammo owed; whole units leave the clip.
pub fn ammo(mut events: MessageReader<WeaponEvent>, mut guns: Query<(&mut Tf2Flame, &mut Magazine)>) {
    for e in events.read() {
        if !matches!(e.kind, WeaponEventKind::Fired { .. }) {
            continue;
        }
        let Ok((mut f, mut m)) = guns.get_mut(e.weapon) else { continue };
        let owed = ((f.owed + f.params.ammo_rate * f.params.emit) * 100.0 + 1e-3).floor() / 100.0;
        let take = owed.floor();
        f.owed = owed - take;
        m.clip = (m.clip + 1).saturating_sub(take as u32);
    }
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
            let now = inv.active == Some(w) && flaming(gun, trigger, clock.now, clock.dt() as f64);
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

/// Whether the segment `a`..`b` meets the box `min`..`max`.
pub fn segment_meets_box(a: Vec3, b: Vec3, min: Vec3, max: Vec3) -> bool {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for i in 0..3 {
        if d[i].abs() < 1e-9 {
            if a[i] < min[i] || a[i] > max[i] {
                return false;
            }
            continue;
        }
        let (mut lo, mut hi) = ((min[i] - a[i]) / d[i], (max[i] - a[i]) / d[i]);
        if lo > hi {
            std::mem::swap(&mut lo, &mut hi);
        }
        t0 = t0.max(lo);
        t1 = t1.min(hi);
        if t0 > t1 {
            return false;
        }
    }
    true
}

type Victims<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        Option<&'static ColliderAabb>,
        Option<&'static MovementState>,
        &'static Health,
        Option<&'static Team>,
        Option<&'static Burning>,
        Option<&'static Tf2Class>,
    ),
    With<Intent>,
>;

/// The flame points: emitted, moved, hitting (the server's).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn flames(
    owners: Query<(Entity, &Inventory, &Intent, &Transform, Option<&MovementState>, Option<&Team>, Option<&Health>, Option<&Velocity>)>,
    mut guns: Query<(Entity, &Tf2Flame, &Tf2Gun, &Weapon, Option<&mut FlameStream>)>,
    victims: Victims,
    spatial: SpatialQuery,
    colliders: Query<&ColliderOf>,
    friendly_fire: Option<Res<FriendlyFire>>,
    classes: Option<Res<Classes>>,
    clock: Res<SimClock>,
    mut damage: MessageWriter<Damage>,
    mut commands: Commands,
) {
    let now = clock.now;
    let dt = clock.dt();
    let ff = friendly_fire.is_some_and(|f| f.0 != 0);
    let filter = SpatialQueryFilter::default().with_mask(SOLID_LAYERS);
    for (owner, inv, intent, at, state, team, health, velocity) in &owners {
        for &w in &inv.weapons {
            let Ok((we, f, gun, weapon, stream)) = guns.get_mut(w) else { continue };
            let Some(mut stream) = stream else {
                commands.entity(we).insert(FlameStream { emitted: gun.last_shot, ..default() });
                continue;
            };
            let p = &f.params;
            // A new round fired: a point.
            let alive = health.is_none_or(|h| h.current > 0.0);
            if gun.last_shot > stream.emitted {
                stream.emitted = gun.last_shot;
                if alive && inv.active == Some(w) {
                    let aim = aim_of(intent.yaw, intent.pitch);
                    let (fwd, right, up) = (aim * Vec3::NEG_Z, aim * Vec3::X, aim * Vec3::Y);
                    let eye = at.translation + state.map_or(Vec3::ZERO, |s| s.eye_offset);
                    let mut rng = shared_random(intent.command, "tf2:flame", stream.count as i32);
                    stream.count = stream.count.wrapping_add(1);
                    let spread = p.spread.to_radians();
                    let a = (rng.next() - 0.5 + rng.next() - 0.5) * spread;
                    let b = (rng.next() - 0.5 + rng.next() - 0.5) * spread;
                    let dir = (fwd + right * a + up * b).normalize_or(fwd);
                    let life = p.life + p.life_jitter * (rng.next() * 2.0 - 1.0);
                    let pyro = velocity.map_or(Vec3::ZERO, |v| v.0);
                    let start = eye + (fwd * p.origin[0] + right * p.origin[1]) * U;
                    stream.points.push(FlamePoint {
                        at: start,
                        before: start,
                        velocity: dir * p.speed * U,
                        carried: dir * pyro.dot(dir),
                        born: now,
                        life,
                    });
                    while stream.points.len() > p.max_points {
                        stream.points.remove(0);
                    }
                }
            }
            // Each point's move.
            let drift = Vec3::Y * p.drift * U;
            stream.points.retain_mut(|pt| {
                if (now - pt.born) as f32 >= pt.life {
                    return false;
                }
                pt.before = pt.at;
                let step = (pt.velocity + drift + pt.carried) * dt;
                let hit_world = Dir3::new(step).ok().is_some_and(|d| {
                    spatial
                        .cast_ray_predicate(pt.at, d, step.length(), true, &filter, &|e| {
                            let body = colliders.get(e).map_or(e, |o| o.body);
                            !victims.contains(body) && body != owner
                        })
                        .is_some()
                });
                if hit_world {
                    return false;
                }
                pt.at += step;
                pt.velocity *= (1.0 - p.drag * dt).max(0.0);
                true
            });
            // The heat cools.
            for h in &mut stream.heat {
                h.heat = (h.heat - p.heat_decay).max(0.0);
            }
            // Touches.
            let mine = team.copied().filter(|t| t.0 != 0);
            let half = Vec3::splat(p.size * U);
            for (v, vt, aabb, vstate, vh, vteam, burning, class) in &victims {
                if v == owner || vh.current <= 0.0 || (!ff && mine.is_some() && vteam.copied() == mine) {
                    continue;
                }
                let (min, max) = boxed(vt, aabb, vstate);
                let (min, max) = (min - half, max + half);
                let touching: Vec<FlamePoint> =
                    stream.points.iter().filter(|pt| segment_meets_box(pt.before, pt.at, min, max)).copied().collect();
                let Some(oldest) = touching.first().copied() else { continue };
                let entry = stream.heat.iter().position(|h| h.victim == v);
                let added: f32 = touching.iter().map(|pt| p.touch_heat((now - pt.born) as f32)).sum();
                if let Some(i) = entry {
                    stream.heat[i].heat += added;
                }
                let due = entry.is_none_or(|i| now - stream.heat[i].last_hit + 1e-6 >= p.hit_interval as f64);
                if !due {
                    continue;
                }
                let i = match entry {
                    Some(i) => {
                        stream.heat[i].last_hit = now;
                        i
                    }
                    None => {
                        stream.heat.push(Heat { victim: v, heat: 1.0, last_hit: now });
                        stream.heat.len() - 1
                    }
                };
                let heat = stream.heat[i].heat;
                let amount = round_points(p.hit_damage((now - oldest.born) as f32, oldest.life, heat) * HP);
                let c = (min + max) / 2.0;
                damage.write(Damage {
                    target: v,
                    attacker: Some(owner),
                    amount,
                    point: c,
                    dir: (oldest.at - oldest.before).normalize_or(Vec3::NEG_Z),
                    hitgroup: Hitgroup::Generic,
                    kind: DamageKind::Burn,
                    weapon: Some(weapon.id),
                    force: Vec3::ZERO,
                });
                // The burn: an enemy Pyro only looks it.
                let pyro = classes.as_ref().zip(class).is_some_and(|(t, c)| c.class(t).name == PYRO);
                let lit = if pyro {
                    Burning {
                        left: 0.0,
                        next: now + p.pyro_burn as f64,
                        attacker: Some(owner),
                        weapon: weapon.id,
                        damage: 0.0,
                        interval: p.burn_interval,
                        harmless: true,
                    }
                } else {
                    let old = burning.filter(|b| !b.harmless);
                    Burning {
                        left: p.burn_after(old.map(|b| b.left)),
                        next: old.map_or(now + p.burn_interval as f64, |b| b.next),
                        attacker: Some(owner),
                        weapon: weapon.id,
                        damage: p.burn_damage,
                        interval: p.burn_interval,
                        harmless: false,
                    }
                };
                commands.entity(v).insert(lit);
            }
            // Forgotten after a hit interval without a damaging hit.
            stream.heat.retain(|h| now - h.last_hit <= p.hit_interval as f64 + 1e-6 && victims.contains(h.victim));
        }
    }
}

/// Afterburn (the server's).
pub fn afterburn(
    mut burning: Query<(Entity, &mut Burning, &Health, &Transform, Option<&SourceMovement>)>,
    clock: Res<SimClock>,
    mut damage: MessageWriter<Damage>,
    mut commands: Commands,
) {
    let now = clock.now;
    for (e, mut b, h, at, movement) in &mut burning {
        // Dead, or waist-deep in water: out.
        if h.current <= 0.0 || movement.is_some_and(|m| m.water_level >= 2) {
            commands.entity(e).remove::<Burning>();
            continue;
        }
        while b.next <= now + 1e-6 {
            if b.left <= 0.0 {
                commands.entity(e).remove::<Burning>();
                break;
            }
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
            b.left -= b.interval;
            b.next += b.interval as f64;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> FlameParams {
        super::super::hook_params(super::super::FLAMETHROWER, super::super::FLAME_HOOK)
    }

    /// 7.2: 13 fresh and at full heat, half cold, the age factor's floor
    /// 0.5 at half its life; at least 1.
    #[test]
    fn a_flame_hit_ranges_from_6_5_to_13() {
        let p = params();
        assert!((p.hit_damage(0.0, 0.6, 50.0) - 13.0).abs() < 1e-4);
        assert!((p.hit_damage(0.0, 0.6, 1.0) - 6.5).abs() < 1e-4);
        assert!((p.hit_damage(0.3, 0.6, 50.0) - 6.5).abs() < 1e-4);
        assert!((p.hit_damage(0.5, 0.6, 1.0) - 3.25).abs() < 1e-4);
        assert!((p.hit_damage(0.0, 0.6, 30.0) - 9.75).abs() < 1e-4);
        assert_eq!(p.touch_heat(0.0), 2.0);
        assert_eq!(p.touch_heat(0.05), 1.0);
    }

    /// 7.3, tests 44, 45: one hit burns 3.4 s; long contact caps at 10.
    #[test]
    fn the_burn_lasts_3_4_to_10_s_44_45() {
        let p = params();
        assert!((p.burn_after(None) - 3.4).abs() < 1e-5);
        assert!((p.burn_after(Some(3.4)) - 3.8).abs() < 1e-5);
        assert_eq!(p.burn_after(Some(9.9)), 10.0);
    }

    /// 7.1: a still Pyro's point flies 285 units in 0.5 s, 287 in 0.6 s.
    #[test]
    fn a_point_reaches_about_287_units() {
        let p = params();
        assert!((p.reach(33, 0.015) - 285.0).abs() < 1.5, "{}", p.reach(33, 0.015));
        assert!((p.reach(40, 0.015) - 287.0).abs() < 1.5, "{}", p.reach(40, 0.015));
    }

    #[test]
    fn a_segment_meets_a_box() {
        let (min, max) = (Vec3::splat(-1.0), Vec3::splat(1.0));
        assert!(segment_meets_box(Vec3::new(-5.0, 0.0, 0.0), Vec3::new(5.0, 0.0, 0.0), min, max));
        assert!(!segment_meets_box(Vec3::new(-5.0, 2.0, 0.0), Vec3::new(5.0, 2.0, 0.0), min, max));
        assert!(!segment_meets_box(Vec3::new(-5.0, 0.0, 0.0), Vec3::new(-3.0, 0.0, 0.0), min, max));
        assert!(segment_meets_box(Vec3::new(0.5, 0.5, 0.5), Vec3::new(0.6, 0.5, 0.5), min, max));
    }
}
