//! The unlocks' own projectiles (specs/tf2/batch2b_mechanics.md 1-6):
//! what a flyer does on its touches, the hook `tf2:shot` (`ShotParams`),
//! and the bats' thrown balls, the hook `tf2:throw` (`ThrowParams`, the
//! weapon part `Tf2Throw`). Chimera's flyers fly them (`chimera:projectile`
//! for the bow, the crossbow, the Bison and the cleaver; a `LaunchRequest`
//! for a thrown ball); `touch` (the server's, `FlyerSet::Touch`) answers.
//!
//! - **The hit** on an enemy: `damage` + `charge_damage` × the flyer's
//!   charge (the arrow's 50 + 70 c: `charge::Tf2Charge`), × (1 + f) with
//!   f = min(1, (t − `ramp_lag`) / `ramp_time`) by its flight time t, at
//!   most `ramp_cap` (the crossbow's 38 to 75), × the distance modifier
//!   from the shooter with its `ramp` when `by_distance` (the Bison's), ×
//!   `moonshot_damage` (and a crit with `moonshot_crit`) from `moonshot` s
//!   of flight; a crit (× 3,
//!   no distance modifier, `TFPlayer.CritHit` at him) on a headshot with
//!   `head_crit` (the arrow: Chimera's `FlyerHit::hitgroup`). It pushes as
//!   a bullet's (6 × its damage). Then: a slow (`slow`: clamp(`per_s` ×
//!   t, `min`, `max`) s at `factor`, none under `min_flight`, the most
//!   on a moonshot; Chimera's `status::Slowed`), a bleed (`bleed` s of 4
//!   every 0.5, the first with the hit: `items::effects::BLEED_TICK`), a
//!   full stop (`stop` s, `Slowed` at 0), the ornament's glass `burst`
//!   (the shared blast at the break: 6 to 9 by flight, its thrower too).
//! - A teammate: healed `heal` × (1 + f) (flight time with no lag) up to
//!   his max (the crossbow; it hits teammates: no `pass_teammates`), else
//!   passed through.
//! - **Piercing** (`pierce`): it flies on through players, hitting each
//!   one `hits` times `rehit` s apart (the Bison's 2, two ticks).
//! - **The world**: an arrow stays stuck there `stick` s; a ball
//!   (`bounce`) bounces off it and is spent (it hurts no one after);
//!   anything else is gone there, an ornament with its burst.
//! - **Thrown** (`tf2:throw`, `throw`): attack2 with the bat out and its
//!   meter full launches the ball (or ornament) from `offset` (forward,
//!   right, up of the eye) at `speed` along the aim turned `up_angle`
//!   degrees up, under `gravity` with its air `drag`; the meter refills
//!   in `recharge` s (× an item's `effectbar_recharge_rate`).
//!
//! Known shortcuts (the spec's open questions, provisional): the Bison's
//! near ramp is 0.37 (its test case's 22 at 270; the spec's fit says 0.4 to
//! 0.5), its second hit lands two ticks after the first wherever the ring
//! is, and the ring is drawn as nothing; the crossbow's damage lags its
//! heal by `ramp_lag` (0.03 s, fitted to the tables); the ornament's burst
//! (radius 128, edge 0.5, by flight from 0.09 to 0.57 s) and the balls'
//! drags are fitted; a spent ball stays `life` s and is no pickup; the
//! cleaver and an arrow's stick time are ours; the bats' meters are the
//! server's, not shown on any HUD; the crossbow bolt is drawn as the
//! needle's model.

use std::sync::Arc;

use bevy::{platform::collections::HashSet, prelude::*};
use serde::Deserialize;

use chimera_sim::{
    status::{Bleeding, Slowed},
    weapon::{
        Inventory, Weapon, aim_of,
        flight::DragParams,
        projectile::{Flyer, FlyerGone, FlyerHit, LaunchRequest, SkyEnd},
    },
};

use super::{
    blast::{Blast, BlastParams, BlastWorld, Placement},
    damage::{HP, U, modifier, round_points},
};
use crate::core::{Damage, DamageKind, Health, Hitgroup, Intent, MovementState, SimClock};

/// The hook of a flyer's hit rules.
pub const SHOT_HOOK: &str = "tf2:shot";
/// The hook of a bat's thrown ball.
pub const THROW_HOOK: &str = "tf2:throw";

/// A slow by flight time (batch2b_mechanics.md 3).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SlowLaw {
    pub per_s: f32,
    pub min: f32,
    pub max: f32,
    pub min_flight: f32,
    pub factor: f32,
}

impl SlowLaw {
    /// Its seconds after `t` s of flight (none: no slow); a moonshot the
    /// most.
    pub fn secs(&self, t: f32, moonshot: bool) -> Option<f32> {
        if moonshot {
            return Some(self.max);
        }
        (t >= self.min_flight).then(|| (self.per_s * t).clamp(self.min, self.max))
    }
}

/// The ornament's glass burst (4): `damage[0]` at `from` s of flight or
/// less rising to `damage[1]` at `to`, at `radius` with `edge`.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Burst {
    pub radius: f32,
    pub edge: f32,
    pub damage: [f32; 2],
    pub from: f32,
    pub to: f32,
}

impl Burst {
    /// Its damage at the centre after `t` s of flight.
    pub fn at(&self, t: f32) -> f32 {
        let f = ((t - self.from) / (self.to - self.from).max(1e-6)).clamp(0.0, 1.0);
        (self.damage[0] + (self.damage[1] - self.damage[0]) * f).round()
    }

    fn blast(&self, damage: f32) -> BlastParams {
        BlastParams {
            damage,
            radius: self.radius,
            edge: self.edge,
            self_radius: Some(self.radius),
            self_damage: Some(damage),
            self_air_scale: 1.0,
            self_ground_scale: 1.0,
            fuse_scale: None,
            push_other: 0.0,
            push_self_air: 0.0,
            push_self_ground: 0.0,
            by_distance: false,
            sound: None,
            bounce_sound: None,
            self_scale: 1.0,
            self_harm: true,
            first_touch_keep: None,
            shatter: false,
            impact: None,
        }
    }
}

/// `tf2:shot`'s parameters (module doc; units, points, seconds).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ShotParams {
    pub damage: f32,
    #[serde(default)]
    pub charge_damage: f32,
    #[serde(default)]
    pub head_crit: bool,
    #[serde(default)]
    pub ramp_time: f32,
    #[serde(default)]
    pub ramp_lag: f32,
    /// The most the flight-time law reaches (the crossbow's 75, not
    /// 2 × 38; 0: no cap).
    #[serde(default)]
    pub ramp_cap: f32,
    #[serde(default)]
    pub heal: f32,
    #[serde(default)]
    pub by_distance: bool,
    #[serde(default = "one")]
    pub ramp: f32,
    #[serde(default)]
    pub pierce: bool,
    #[serde(default = "one_hit")]
    pub hits: u32,
    #[serde(default)]
    pub rehit: f32,
    #[serde(default)]
    pub slow: Option<SlowLaw>,
    #[serde(default)]
    pub moonshot: f32,
    #[serde(default = "one")]
    pub moonshot_damage: f32,
    #[serde(default)]
    pub moonshot_crit: bool,
    #[serde(default)]
    pub bleed: f32,
    #[serde(default)]
    pub stop: f32,
    #[serde(default)]
    pub burst: Option<Burst>,
    #[serde(default)]
    pub stick: f32,
    #[serde(default)]
    pub bounce: bool,
}

fn one() -> f32 {
    1.0
}

fn one_hit() -> u32 {
    1
}

impl ShotParams {
    /// The flight-time factor f after `t` s, lagged by `lag`.
    pub fn ramp_f(&self, t: f32, lag: f32) -> f32 {
        if self.ramp_time <= 0.0 { 0.0 } else { ((t - lag) / self.ramp_time).clamp(0.0, 1.0) }
    }

    /// Whether `t` s of flight is a moonshot.
    pub fn moonshot_at(&self, t: f32) -> bool {
        self.moonshot > 0.0 && t + 1e-4 >= self.moonshot
    }

    /// The hit's damage (points, unrounded) and whether it crits: flight
    /// `t` s, charge `c`, `d` units from the shooter, on the head or not.
    pub fn hit(&self, t: f32, c: f32, d: f32, head: bool) -> (f32, bool) {
        let moon = self.moonshot_at(t);
        let crit = (self.head_crit && head) || (moon && self.moonshot_crit);
        let mut x = (self.damage + self.charge_damage * c) * (1.0 + self.ramp_f(t, self.ramp_lag));
        if self.ramp_cap > 0.0 {
            x = x.min(self.ramp_cap);
        }
        if moon {
            x *= self.moonshot_damage;
        }
        if crit {
            x *= super::melee::CRIT_MULT;
        } else if self.by_distance {
            x *= modifier(d, self.ramp);
        }
        (x, crit)
    }

    /// A teammate's heal (points) after `t` s.
    pub fn heal_at(&self, t: f32) -> f32 {
        self.heal * (1.0 + self.ramp_f(t, 0.0))
    }
}

/// The shots' rules by weapon id.
#[derive(Resource, Clone, Debug, Default)]
pub struct ShotRules(pub Vec<(String, Arc<ShotParams>)>);

impl ShotRules {
    pub fn from_set(set: &chimera_sim::weapon::data::WeaponSet) -> Self {
        Self(
            set.weapons
                .iter()
                .filter_map(|w| Some((w.id.clone(), Arc::new(w.hooks.get(SHOT_HOOK)?.clone().try_into::<ShotParams>().ok()?))))
                .collect(),
        )
    }

    pub fn get(&self, weapon: &str) -> Option<&ShotParams> {
        self.0.iter().find(|(id, _)| id == weapon).map(|(_, p)| &**p)
    }
}

/// `tf2:throw`'s parameters (units, seconds).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ThrowParams {
    pub speed: f32,
    pub up_angle: f32,
    pub offset: [f32; 3],
    pub gravity: f32,
    #[serde(default)]
    pub drag: Option<DragParams>,
    pub half: f32,
    pub shown: String,
    pub recharge: f32,
    pub life: f32,
}

/// A bat's meter (its ball ready at `ready`, sim time): the server's.
#[derive(Component, Clone, Debug)]
pub struct Tf2Throw {
    pub params: Arc<ThrowParams>,
    pub ready: f64,
}

/// `tf2:throw` on a weapon: its ball ready.
pub fn attach(e: &mut EntityWorldMut, p: &ThrowParams) {
    e.insert(Tf2Throw { params: Arc::new(p.clone()), ready: 0.0 });
}

/// Hits still to land (a piercing ring's second): due time, the damage,
/// from where.
#[derive(Resource, Clone, Debug, Default)]
pub struct Rehits(pub Vec<(f64, Damage, Vec3)>);

/// Balls spent on the world (no more hits).
#[derive(Resource, Clone, Debug, Default)]
pub struct Spent(pub HashSet<Entity>);

/// Attack2 with a bat out and its meter full: the ball (module doc).
pub fn throw(
    owners: Query<(Entity, &Intent, &Inventory, &Transform, Option<&MovementState>, Option<&Health>)>,
    mut bats: Query<(&Weapon, &mut Tf2Throw)>,
    clock: Res<SimClock>,
    mut launch: MessageWriter<LaunchRequest>,
) {
    let now = clock.now;
    for (owner, intent, inv, at, state, health) in &owners {
        if !intent.secondary || health.is_some_and(|h| h.current <= 0.0) {
            continue;
        }
        let Some(w) = inv.active else { continue };
        let Ok((weapon, mut bat)) = bats.get_mut(w) else { continue };
        if now + 1e-6 < bat.ready {
            continue;
        }
        let p = bat.params.clone();
        bat.ready = now + p.recharge as f64;
        let aim = aim_of(intent.yaw, intent.pitch);
        let right = aim * Vec3::X;
        let dir = Quat::from_axis_angle(right, p.up_angle.to_radians()) * (aim * Vec3::NEG_Z);
        let eye = at.translation + state.map_or(Vec3::ZERO, |s| s.eye_offset);
        let o = Vec3::from(p.offset) * U;
        let start = eye + aim * Vec3::NEG_Z * o.x + right * o.y + aim * Vec3::Y * o.z;
        let mut f = Flyer::new(weapon.id, Some(owner), dir * p.speed * U, &clock);
        f.gravity = p.gravity * U;
        f.drag = p.drag.map(|d| d.drag(U));
        f.half = Vec3::splat(p.half * U);
        f.hull_boxes = true;
        f.pass_teammates = true;
        f.expires = Some(now + p.life as f64);
        f.sky_end = Some(SkyEnd { probe: 16.0 * U, expired: false });
        launch.write(LaunchRequest { flyer: f, start, shown: Some(p.shown.clone()), tag: None });
    }
}

/// The shots' touches (module doc).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn touch(
    mut hits: MessageReader<FlyerHit>,
    rules: Res<ShotRules>,
    mut flyers: Query<&mut Flyer>,
    mut world: BlastWorld,
    (bleeds, slows): (Query<&Bleeding>, Query<&Slowed>),
    (clock, mut rehits, mut spent): (Res<SimClock>, ResMut<Rehits>, ResMut<Spent>),
    mut gone: MessageWriter<FlyerGone>,
    mut commands: Commands,
) {
    let now = clock.now;
    // A piercing ring's hits due.
    let due: Vec<(f64, Damage, Vec3)> = rehits.0.extract_if(.., |(t, ..)| *t <= now + 1e-6).collect();
    for (_, d, from) in due {
        if world.chars.get(d.target).is_ok_and(|c| c.3.current > 0.0) {
            world.hurt(d, from);
        }
    }
    spent.0.retain(|e| flyers.contains(*e));
    for hit in hits.read() {
        let Some(p) = rules.get(hit.weapon) else { continue };
        let Ok(mut f) = flyers.get_mut(hit.flyer) else { continue };
        let t = (now - f.born) as f32;
        let is_spent = spent.0.contains(&hit.flyer);
        let mut end = true;
        let mut burst_on: Option<Option<Entity>> = None;
        match hit.other.filter(|_| hit.character) {
            Some(o) if is_spent => {
                f.passed.push(o);
                f.velocity = hit.velocity;
                end = false;
            }
            Some(o) if hit.owner.is_some_and(|w| world.mates(w, o)) => {
                if p.heal > 0.0 {
                    let gain = p.heal_at(t).round() * HP;
                    commands.entity(o).queue(move |mut w: EntityWorldMut| {
                        if let Some(mut h) = w.get_mut::<Health>()
                            && h.current > 0.0
                            && h.current < h.max
                        {
                            h.current = (h.current + gain).min(h.max);
                        }
                    });
                } else {
                    f.passed.push(o);
                    f.velocity = hit.velocity;
                    end = false;
                }
            }
            Some(o) => {
                let from = hit.owner.and_then(|w| world.centre(w));
                let d = from.zip(world.centre(o)).map_or(0.0, |(a, b)| a.distance(b) / U);
                let (points, crit) = p.hit(t, f.charge, d, hit.hitgroup == Hitgroup::Head);
                let dir = hit.velocity.normalize_or(Vec3::NEG_Z);
                let damage = Damage {
                    target: o,
                    attacker: hit.owner,
                    amount: round_points(points * HP),
                    point: hit.point,
                    dir,
                    hitgroup: hit.hitgroup,
                    kind: DamageKind::Bullet,
                    weapon: Some(hit.weapon),
                    force: Vec3::ZERO,
                };
                world.hurt(damage.clone(), hit.point - dir);
                world.knock(o, hit.point - dir * 16.0 * U, points, super::blast::BULLET_PUSH);
                if crit {
                    world.sound_from(super::melee::CRIT_HIT, hit.point, o);
                }
                for k in 1..p.hits {
                    rehits.0.push((now + (p.rehit * k as f32) as f64, damage.clone(), hit.point - dir));
                }
                if let Some(s) = p.slow.and_then(|s| Some((s.secs(t, p.moonshot_at(t))?, s.factor))) {
                    let new = Slowed { until: now + s.0 as f64, factor: s.1 };
                    commands.entity(o).insert(Slowed::with(slows.get(o).ok(), new));
                }
                if p.stop > 0.0 {
                    let new = Slowed { until: now + p.stop as f64, factor: 0.0 };
                    commands.entity(o).insert(Slowed::with(slows.get(o).ok(), new));
                }
                if p.bleed > 0.0 {
                    use crate::game::items::effects::{BLEED_TICK, DOT_PERIOD};
                    let new = Bleeding::start(now, p.bleed, BLEED_TICK * HP, DOT_PERIOD, hit.owner, Some(hit.weapon));
                    commands.entity(o).insert(Bleeding::refresh(bleeds.get(o).ok(), new, now));
                }
                if p.burst.is_some() {
                    burst_on = Some(Some(o));
                }
                if p.pierce {
                    f.passed.push(o);
                    f.velocity = hit.velocity;
                    end = false;
                }
            }
            None if hit.world && p.stick > 0.0 => {
                f.velocity = Vec3::ZERO;
                f.gravity = 0.0;
                f.stuck_until = Some(now + p.stick as f64);
                end = false;
            }
            None if p.bounce => {
                f.velocity = super::grenade::bounce(hit.velocity, hit.normal);
                spent.0.insert(hit.flyer);
                end = false;
            }
            None => {
                if p.burst.is_some() {
                    burst_on = Some(None);
                }
            }
        }
        if let (Some(b), Some(touched)) = (p.burst, burst_on) {
            let params = b.blast(b.at(t));
            let blast = Blast { params: &params, radius: b.radius, ramp: None, scale: 1.0, mini_crit: false, self_law: None };
            let at = Placement { centre: hit.point + hit.normal * U, ..default() };
            world.detonate(&blast, &at, hit.owner, hit.weapon, touched);
        }
        if end {
            gone.write(FlyerGone { flyer: hit.flyer, weapon: hit.weapon, owner: hit.owner, at: hit.point, expired: false });
            commands.entity(hit.flyer).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(text: &str) -> ShotParams {
        toml::from_str(text).unwrap()
    }

    #[test]
    fn the_laws_follow_the_tables() {
        // The arrow (1): 50 + 70 c, a headshot x 3.
        let arrow = p("damage = 50.0\ncharge_damage = 70.0\nhead_crit = true");
        assert_eq!(arrow.hit(0.3, 0.555, 500.0, false).0.round(), 89.0);
        assert_eq!(arrow.hit(0.3, 1.0, 500.0, true), (360.0, true));
        // The bolt (2): flight time, the damage a little behind the heal.
        let bolt = p("damage = 38.0\nramp_time = 0.6\nramp_lag = 0.03\nramp_cap = 75.0\nheal = 75.0");
        assert_eq!(bolt.hit(0.255, 0.0, 900.0, true).0.round(), 52.0);
        assert_eq!(bolt.hit(0.03, 0.0, 900.0, false).0.round(), 38.0);
        assert_eq!(bolt.hit(0.9, 0.0, 900.0, false).0.round(), 75.0);
        assert_eq!(bolt.heal_at(0.45).round(), 131.0);
        assert_eq!(bolt.heal_at(0.7).round(), 150.0);
        // The ball (3).
        let ball = p("damage = 15.0\nmoonshot = 0.8\nmoonshot_damage = 1.5\nslow = { per_s = 7.5, min = 2.0, max = 7.0, min_flight = 0.08, factor = 0.5 }");
        let s = ball.slow.unwrap();
        assert_eq!(s.secs(0.06, false), None);
        assert!((s.secs(0.525, false).unwrap() - 3.9375).abs() < 1e-4);
        assert_eq!(s.secs(0.85, ball.moonshot_at(0.85)), Some(7.0));
        assert_eq!(ball.hit(0.85, 0.0, 0.0, false).0.round(), 23.0);
        // The Bison (6): 22 at 270, 20 at 470; a crit 60.
        let ring = p("damage = 20.0\nby_distance = true\nramp = 0.37");
        assert_eq!(ring.hit(0.2, 0.0, 270.0, false).0.round(), 22.0);
        assert_eq!(ring.hit(0.4, 0.0, 470.0, false).0.round(), 20.0);
        assert_eq!(ring.hit(0.1, 0.0, 80.0, false).0.round(), 24.0);
        // The ornament's burst (4).
        let b = Burst { radius: 128.0, edge: 0.5, damage: [6.0, 9.0], from: 0.09, to: 0.57 };
        assert_eq!((b.at(0.05), b.at(0.27), b.at(0.42), b.at(0.9)), (6.0, 7.0, 8.0, 9.0));
    }
}
