//! The Stickybomb Launcher (specs/tf2/weapons_more.md 5, 6), the hook
//! `tf2:sticky` (`StickyParams`, the weapon part `Tf2Sticky`):
//!
//! - **The charge** (5.1, `charge`, predicted, before selection): a press
//!   of attack with a round in and the interval passed starts it (its
//!   sound, the view model's `CHARGE`); the frame sees no attack while it
//!   charges; letting go, or holding `charge_time` (4 s), lets one attack
//!   through, which fires a round, and the launch speed is set by the time
//!   held: `speed_min` + (`speed_max` - `speed_min`) min(h / 4, 1) (900 to
//!   2400). After a charge that ran out the button must be let go.
//! - **The launch** (`launch`, the server's, `FlyerSet::Launch`): each
//!   round fired is a sticky (Chimera's flyer, gravity 800) from 16
//!   forward, 8 right and 6 down of the eye at that speed along the aim
//!   and `up_speed` (200) up, plus up to `jitter` (10) along the aim's
//!   right and up at random (`launch_random`, from the shot's seed), the
//!   whole vector's length capped at `max_speed` (2000, the physics
//!   engine's, its direction kept: a full charge leaves at 2000),
//!   through teammates, tumbling at `spin` ((600, a whole number in
//!   ±1200, 0) degrees/s about its own axes added twice: (1200, 2r, 0),
//!   `flight::LaunchSpin`) until it sticks, losing speed and spin to the
//!   air (`drag`, 4.4: Chimera's `flight::AirDrag` at `tf2:air_density`,
//!   2).
//! - **Touches** (5.2, `touch`): the world (what doesn't move) stops it
//!   dead and it stays (`Flyer::stuck_until` far away; not for 1 s after
//!   an airblast moved it, `Unstuck`); anything else
//!   bounces it off at half speed; it never explodes on contact.
//! - **Detonation** (5.3, 5.4, `detonate`): attack2 with the launcher, or any
//!   other TF2 weapon, out detonates every sticky of its owner armed (`arm_time`: `arm` +
//!   the launcher's `sticky_arm_time`, 0.8 s after launch on the stock one;
//!   the detonate sound); over `max` (8) the oldest goes off; a dead owner's
//!   vanish. Each explodes with the shared blast (`blast`): 120 x the
//!   splash x the short-range-reduced modifier M_s (121.8 % at 0, the
//!   owner's place then: `damage::COMPRESSED`) if at most `flat_after`
//!   (5 s) old, else flat; one not stuck reaches `air_radius_start` (85 %)
//!   of the radius at arming, all of it `air_radius_time` (2 s) later
//!   (0.8 s to 2.8 s); the Demoman's own the base 120 x the splash to 146
//!   units x0.75 grounded or airborne, pushed at scale 9. Where
//!   (effects.md 13, `BlastWorld::fused`): snapped onto a floor within 24
//!   units below it (the scorch there), sending the normal of the surface
//!   it stuck to (`StuckNormal`), else the floor's, else none.
//!
//! Known shortcuts: a ninth sticky blows the oldest at once (the game: at
//! its next 0.2 s fuse check); bullets don't destroy stickies; the
//! launch's jitter and spin come from the shot's seed, not the game's
//! random stream, and a bounce off a player keeps the spin (the game:
//! −0.5 times it); the stickies' 2-unit box is ours (the game's 4-unit
//! cube); attack2 detonates with any TF2 weapon out (the game: any of the
//! Demoman's; a party's players have no fixed class), and unarmed ones play no deny sound.

use std::sync::Arc;

use bevy::prelude::*;
use serde::Deserialize;

use chimera_sim::weapon::{
    CHAN_WEAPON, Inventory, Magazine, NetPart, Weapon, WeaponEvent, WeaponEventKind, WeaponState, aim_of,
    flight::{DragParams, LaunchSpin},
    projectile::{Flyer, FlyerHit, spawn_flyer},
};

use super::{
    blast::{Blast, BlastParams, BlastWorld},
    damage::U,
};
use crate::{
    core::{FirstTimePredicted, Health, Intent, SimClock},
    map::{PlaySound, PredictedSound},
};

/// The view-model action of a charge's start.
pub const CHARGE: &str = "sticky_charge";

/// The model key the stickies are drawn as.
pub const SHOWN: &str = "tf2:tf_weapon_pipebomblauncher/sticky";
/// Its model.
pub const MODEL: &str = "models/weapons/w_models/w_stickybomb.mdl";

/// `tf2:sticky`'s parameters (units, points, seconds).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StickyParams {
    pub damage: f32,
    pub radius: f32,
    pub edge: f32,
    #[serde(default)]
    pub self_radius: Option<f32>,
    #[serde(default)]
    pub self_damage: Option<f32>,
    pub self_air_scale: f32,
    #[serde(default = "one")]
    pub self_ground_scale: f32,
    pub push_other: f32,
    pub push_self_air: f32,
    pub push_self_ground: f32,
    pub flat_after: f32,
    pub arm: f32,
    pub air_radius_start: f32,
    pub air_radius_time: f32,
    pub max: usize,
    pub speed_min: f32,
    pub speed_max: f32,
    pub up_speed: f32,
    /// The physics engine's speed cap on the launched body (units/s): the
    /// launch velocity's length at most this, its direction kept (5.1:
    /// 2000, so a full charge, 2400 + 200 up, leaves at 2000); none: no
    /// cap.
    #[serde(default)]
    pub max_speed: Option<f32>,
    pub charge_time: f32,
    pub gravity: f32,
    pub offset: [f32; 3],
    /// Random launch jitter: up to this along the aim's right and its up
    /// (units/s).
    #[serde(default)]
    pub jitter: f32,
    /// The launch spin (weapons_more.md 4.1: (600, a whole number in
    /// ±1200, 0) degrees/s added twice; `flight::LaunchSpin`).
    #[serde(default)]
    pub spin: Option<LaunchSpin>,
    /// Its air drag in flight (4.4; `flight::DragParams`).
    #[serde(default)]
    pub drag: Option<DragParams>,
    /// Added to `arm` (the launcher's `sticky_arm_time` attribute, 5.3:
    /// none on the stock launcher, −0.2 on the Quickiebomb).
    #[serde(default)]
    pub sticky_arm_time: f32,
    /// A full charge's damage factor f (an item's
    /// `stickybomb_charge_damage_increase`, item_attributes.md 4.6): the
    /// damage × (1 + (f − 1) c), c the charge 0…1.
    #[serde(default = "one")]
    pub charge_damage: f32,
    /// The owner's own blast (`blast::BlastParams`'s).
    #[serde(default = "one")]
    pub self_scale: f32,
    #[serde(default = "yes")]
    pub self_harm: bool,
    #[serde(default)]
    pub sound: Option<String>,
    #[serde(default)]
    pub charge_sound: Option<String>,
    #[serde(default)]
    pub detonate_sound: Option<String>,
    /// Drawn in the past on every machine that draws, by the
    /// interpolation delay (`map::loose::DrawnLate`, effects.md 11).
    #[serde(default)]
    pub interp: bool,
    /// Drawn late, its model hidden its first this many seconds from its
    /// arrival (its trail shows): 0.1 (effects.md 11.1).
    #[serde(default)]
    pub model_hidden: f32,
    /// Drawn late, its first received place in whole units of the data
    /// (effects.md 11.2: Source sends origins in whole inches).
    #[serde(default)]
    pub whole_units: bool,
    /// The data's length unit (m), from its `units`.
    #[serde(skip)]
    pub unit: f32,
}

fn one() -> f32 {
    1.0
}

fn yes() -> bool {
    true
}

impl StickyParams {
    /// Its explosion's numbers.
    pub fn blast(&self) -> BlastParams {
        BlastParams {
            damage: self.damage,
            radius: self.radius,
            edge: self.edge,
            self_radius: self.self_radius,
            self_damage: self.self_damage,
            self_air_scale: self.self_air_scale,
            self_ground_scale: self.self_ground_scale,
            fuse_scale: None,
            push_other: self.push_other,
            push_self_air: self.push_self_air,
            push_self_ground: self.push_self_ground,
            by_distance: true,
            sound: self.sound.clone(),
            bounce_sound: None,
            self_scale: self.self_scale,
            self_harm: self.self_harm,
            first_touch_keep: None,
            shatter: false,
        }
    }

    /// Its arm time L (s): `arm` + `sticky_arm_time` (5.3).
    pub fn arm_time(&self) -> f32 {
        self.arm + self.sticky_arm_time
    }

    /// The launch speed (units/s) after holding `held` seconds.
    pub fn speed(&self, held: f32) -> f32 {
        self.speed_min + (self.speed_max - self.speed_min) * (held / self.charge_time).clamp(0.0, 1.0)
    }

    /// The radius (units) a sticky of `age` seconds reaches, stuck or not.
    pub fn radius_at(&self, age: f32, stuck: bool) -> f32 {
        if stuck {
            return self.radius;
        }
        // From the stock arm time, whatever the item's (4.6's quirk).
        let t = ((age - self.arm) / self.air_radius_time).clamp(0.0, 1.0);
        self.radius * (self.air_radius_start + (1.0 - self.air_radius_start) * t)
    }
}

/// The launcher's charge (a predicted part).
#[derive(Component, Clone, Debug)]
pub struct Tf2Sticky {
    pub params: Arc<StickyParams>,
    /// Charging since.
    pub charging: Option<f64>,
    /// A charge ran out with the button held: wait for its release.
    pub wait_release: bool,
    /// The next launch's speed (units/s).
    pub speed: f32,
}

impl NetPart for Tf2Sticky {
    type Net = (Option<f64>, bool, f32);
    fn net(&self) -> Self::Net {
        (self.charging, self.wait_release, self.speed)
    }
    fn set_net(&mut self, (charging, wait_release, speed): Self::Net) {
        self.charging = charging;
        self.wait_release = wait_release;
        self.speed = speed;
    }
}

/// `tf2:sticky` on a weapon.
pub fn attach(e: &mut EntityWorldMut, p: &StickyParams) {
    e.insert(Tf2Sticky {
        params: Arc::new(p.clone()),
        charging: None,
        wait_release: false,
        speed: p.speed_min,
    });
}

/// The stickies' rules by weapon id (with the gun's ramp).
#[derive(Resource, Clone, Debug, Default)]
pub struct StickyRules(pub Vec<(String, Arc<StickyParams>, Arc<BlastParams>, f32)>);

impl StickyRules {
    pub fn from_set(set: &chimera_sim::weapon::data::WeaponSet) -> Self {
        let mut out = Vec::new();
        for w in &set.weapons {
            let Some(p) = w.hooks.get(super::STICKY_HOOK) else { continue };
            let Ok(mut p) = p.clone().try_into::<StickyParams>() else { continue };
            p.unit = w.units.length;
            let ramp = super::damage::ramp_of(w.hooks.get(super::GUN_HOOK));
            let blast = Arc::new(p.blast());
            out.push((w.id.clone(), Arc::new(p), blast, ramp));
        }
        Self(out)
    }

    /// The stickies of launchers whose data says `interp` drawn late
    /// (`map::loose::DrawnLate`; their models hidden `model_hidden`).
    pub fn draw_late(&self, late: &mut chimera_world::map::loose::DrawnLate) {
        if let Some((_, p, ..)) = self.0.iter().find(|(_, p, ..)| p.interp) {
            late.add(SHOWN.to_string(), p.model_hidden);
            if p.whole_units {
                late.set_unit(SHOWN, p.unit);
            }
        }
    }

    pub fn get(&self, weapon: &str) -> Option<(&StickyParams, &BlastParams, f32)> {
        self.0.iter().find(|(id, ..)| id == weapon).map(|(_, p, b, k)| (&**p, &**b, *k))
    }
}

/// Each launcher's charge this tick (module doc).
#[allow(clippy::type_complexity)]
pub fn charge(
    mut owners: Query<(Entity, &mut Intent, &Inventory, &Transform, Option<&Health>)>,
    mut guns: Query<(Entity, &mut Tf2Sticky, &WeaponState, Option<&Magazine>)>,
    clock: Res<SimClock>,
    first: Res<FirstTimePredicted>,
    mut sounds: MessageWriter<PredictedSound>,
    mut events: MessageWriter<WeaponEvent>,
) {
    let now = clock.now;
    for (owner, mut intent, inv, at, health) in &mut owners {
        let alive = health.is_none_or(|h| h.current > 0.0);
        for &w in &inv.weapons {
            let Ok((we, mut gun, st, mag)) = guns.get_mut(w) else { continue };
            if inv.active != Some(w) || !alive {
                if gun.charging.is_some() || gun.wait_release {
                    gun.charging = None;
                    gun.wait_release = false;
                }
                continue;
            }
            let held = intent.fire;
            if gun.wait_release {
                if !held {
                    gun.wait_release = false;
                }
                intent.fire = false;
                continue;
            }
            match gun.charging {
                None => {
                    let ready = st.next_primary <= now + 1e-6 && mag.is_none_or(|m| m.clip > 0);
                    if held && ready {
                        gun.charging = Some(now);
                        intent.fire = false;
                        if let Some(s) = gun.params.charge_sound.clone()
                            && first.0
                        {
                            sounds.write(PredictedSound {
                                by: owner,
                                sound: PlaySound {
                                    entry: s,
                                    at: Some(at.translation),
                                    volume: None,
                                    pitch: None,
                                    source: Some(owner),
                                    channel: Some(CHAN_WEAPON),
                                },
                            });
                        }
                        events.write(WeaponEvent { owner, weapon: we, kind: WeaponEventKind::Action(CHARGE), replay: !first.0 });
                    } else if held && mag.is_some_and(|m| m.clip > 0) {
                        // Not ready: nothing fires before a charge.
                        intent.fire = false;
                    }
                }
                Some(start) => {
                    let h = (now - start) as f32;
                    let full = h + 1e-6 >= gun.params.charge_time;
                    if !held || full {
                        let speed = gun.params.speed(h);
                        gun.speed = speed;
                        gun.charging = None;
                        gun.wait_release = held;
                        // One attack through: the frame fires the round.
                        intent.fire = true;
                    } else {
                        intent.fire = false;
                    }
                }
            }
        }
    }
}

/// Each sticky fired (the server's): its flyer.
pub fn launch(
    mut events: MessageReader<WeaponEvent>,
    guns: Query<(&Tf2Sticky, &Weapon)>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    for e in events.read() {
        let WeaponEventKind::Fired { origin, yaw, pitch, seed, .. } = e.kind else { continue };
        if e.replay {
            continue;
        }
        let Ok((gun, weapon)) = guns.get(e.weapon) else { continue };
        let p = &gun.params;
        let aim = aim_of(yaw, pitch);
        let dir = aim * Vec3::NEG_Z;
        let r = launch_random(seed);
        let (right, up) = (p.jitter * r[0], p.jitter * r[1]);
        let mut velocity = dir * gun.speed + aim * Vec3::X * right + aim * Vec3::Y * (p.up_speed + up);
        if let Some(cap) = p.max_speed {
            velocity = velocity.clamp_length_max(cap);
        }
        let velocity = velocity * U;
        let mut f = Flyer::new(weapon.id, Some(e.owner), velocity, &clock);
        f.spin = p.spin.map_or(Vec3::ZERO, |s| s.draw(seed));
        f.drag = p.drag.map(|d| d.drag(U));
        f.gravity = p.gravity * U;
        f.half = Vec3::splat(2.0 * U);
        f.pass_teammates = true;
        let o = Vec3::from(p.offset) * U;
        let start = origin + dir * o.x + aim * Vec3::X * o.y + aim * Vec3::Y * o.z;
        let flyer = spawn_flyer(&mut commands, f, start, SHOWN.to_string(), &clock);
        if p.charge_damage != 1.0 {
            // Its damage by its charge (item_attributes.md 4.6).
            let c = ((gun.speed - p.speed_min) / (p.speed_max - p.speed_min).max(1e-6)).clamp(0.0, 1.0);
            commands.entity(flyer).insert(ChargeScale(1.0 + (p.charge_damage - 1.0) * c));
        }
    }
}

/// Three numbers in [-1, 1] from a shot's seed (the launch's jitter
/// right and up; the third unused: the spin draws its own).
pub fn launch_random(seed: u32) -> [f32; 3] {
    let mut x = seed.wrapping_mul(0x9E37_79B9) ^ 0x5bd1_e995;
    std::array::from_fn(|_| {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        (x % 2001) as f32 / 1000.0 - 1.0
    })
}

/// On a sticky of a launcher whose damage grows with the charge: its
/// damage factor (others only; the owner's own blast is the base).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct ChargeScale(pub f32);

/// When a stuck sticky is gone: never (it goes by detonation).
const FOREVER: f64 = 1.0e12;

/// On a stuck sticky: the normal of the surface it stuck to, out of it,
/// sent with its blast (effects.md 13).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct StuckNormal(pub Vec3);

/// On a sticky knocked loose or turned by an airblast (weapons_more.md
/// 7.4): it doesn't stick again before this time.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
#[component(storage = "SparseSet")]
pub struct Unstuck(pub f64);

/// Stickies' touches: stuck on the world (not while `Unstuck`), bounced
/// off the rest.
pub fn touch(
    mut hits: MessageReader<FlyerHit>,
    rules: Res<StickyRules>,
    mut flyers: Query<(&mut Flyer, Option<&Unstuck>)>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    for hit in hits.read() {
        if rules.get(hit.weapon).is_none() {
            continue;
        }
        let Ok((mut f, unstuck)) = flyers.get_mut(hit.flyer) else { continue };
        if hit.world && unstuck.is_none_or(|u| clock.now + 1e-6 >= u.0) {
            f.velocity = Vec3::ZERO;
            f.stuck_until = Some(FOREVER);
            commands.entity(hit.flyer).insert(StuckNormal(hit.normal));
        } else {
            f.velocity = chimera_sim::weapon::projectile::reflect(hit.velocity, hit.normal) * 0.5;
        }
    }
}

/// Detonations: attack2, the limit, dead owners (module doc).
#[allow(clippy::type_complexity)]
pub fn detonate(
    owners: Query<(Entity, &Intent, &Inventory, Option<&Health>)>,
    launchers: Query<&Tf2Sticky>,
    weapons: Query<&Weapon>,
    stickies: Query<(Entity, &Flyer, &Transform, Option<&StuckNormal>)>,
    charged: Query<&ChargeScale>,
    rules: Res<StickyRules>,
    clock: Res<SimClock>,
    mut world: BlastWorld,
    mut commands: Commands,
) {
    if rules.0.is_empty() {
        return;
    }
    let now = clock.now;
    let mut mine: Vec<(Entity, Entity, &'static str, f64, Option<Vec3>, Vec3, bool)> = stickies
        .iter()
        .filter(|(_, f, ..)| rules.get(f.weapon).is_some())
        .filter_map(|(e, f, t, n)| Some((f.owner?, e, f.weapon, f.born, n.map(|n| n.0), t.translation, f.stuck_until.is_some())))
        .collect();
    if mine.is_empty() {
        return;
    }
    mine.sort_by(|a, b| a.3.total_cmp(&b.3));
    let mut blow: Vec<usize> = Vec::new();
    let mut vanish: Vec<Entity> = Vec::new();
    let mut owners_seen: Vec<Entity> = mine.iter().map(|m| m.0).collect();
    owners_seen.dedup();
    owners_seen.sort();
    owners_seen.dedup();
    for owner in owners_seen {
        let of: Vec<usize> = (0..mine.len()).filter(|&i| mine[i].0 == owner).collect();
        let Ok((_, intent, inv, health)) = owners.get(owner) else {
            vanish.extend(of.iter().map(|&i| mine[i].1));
            continue;
        };
        if health.is_some_and(|h| h.current <= 0.0) {
            vanish.extend(of.iter().map(|&i| mine[i].1));
            continue;
        }
        let Some((p, ..)) = rules.get(mine[of[0]].2) else { continue };
        // Over the limit: the oldest.
        let over = of.len().saturating_sub(p.max);
        blow.extend(of.iter().take(over));
        // With the launcher out, or any of TF2's weapons (the game: any of
        // the Demoman's; a ladder's rung swap takes the launcher away and
        // the stickies laid with it still go off).
        let out = inv
            .active
            .is_some_and(|a| launchers.contains(a) || weapons.get(a).is_ok_and(|w| chimera_sim::weapon::impact::weapon_game(w.id) == Some(super::super::GAME)));
        if out && intent.secondary {
            let mut any = false;
            for &i in of.iter().skip(over) {
                if now - mine[i].3 + 1e-6 >= p.arm_time() as f64 {
                    blow.push(i);
                    any = true;
                }
            }
            if any && let Some(s) = &p.detonate_sound {
                let at = world.centre(owner).unwrap_or(mine[of[0]].5);
                world.sound(s, at);
            }
        }
    }
    for e in vanish {
        commands.entity(e).despawn();
    }
    for i in blow {
        let (owner, e, weapon, born, normal, at, stuck) = mine[i];
        let Some((p, b, ramp)) = rules.get(weapon) else { continue };
        let age = (now - born) as f32;
        let blast = Blast {
            params: b,
            radius: p.radius_at(age, stuck),
            ramp: (age <= p.flat_after).then_some(ramp),
            scale: charged.get(e).map_or(1.0, |c| c.0),
            mini_crit: false,
            self_law: None,
        };
        // Snapped onto a floor within 24 units below, with the normal of
        // the surface it stuck to (effects.md 13).
        let at = world.fused(at, normal);
        world.detonate(&blast, &at, Some(owner), weapon, None);
        commands.entity(e).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> StickyParams {
        super::super::hook_params(super::super::STICKY_LAUNCHER, super::super::STICKY_HOOK)
    }

    /// Test cases 28, 29: 900 at once, 1650 after 2 s, 2400 at 4 s.
    #[test]
    fn the_charge_sets_the_speed_28_29() {
        let p = params();
        assert_eq!(p.speed(0.0), 900.0);
        assert_eq!(p.speed(2.0), 1650.0);
        assert_eq!(p.speed(4.0), 2400.0);
        assert_eq!(p.speed(9.0), 2400.0);
    }

    /// weapons_more.md 4.1 (5.1: the same), test 29b: ±10 jitter right
    /// and up; (600, r, 0) degrees/s added twice: 1200 about its forward
    /// and an even whole number in ±2400 about its left (the engine's −z),
    /// none about its up. 4.4: its drag per body axis.
    #[test]
    fn the_launch_jitters_and_spins_29b() {
        let p = params();
        assert_eq!(p.jitter, 10.0);
        let spin = p.spin.expect("a spin");
        assert_eq!((spin.base, spin.random, spin.times), ([600.0, 0.0, 0.0], [0.0, 1200.0, 0.0], 2));
        let mut seen = [f32::MAX, f32::MIN];
        for seed in 0..500 {
            let r = launch_random(seed);
            assert!(r.iter().all(|x| (-1.0..=1.0).contains(x)), "{r:?}");
            let s = spin.draw(seed).to_array().map(f32::to_degrees);
            assert!((s[0] - 1200.0).abs() < 1e-2 && s[1] == 0.0, "{s:?}");
            assert!(s[2].abs() <= 2400.0 + 1e-2 && (s[2].round() as i32) % 2 == 0, "{s:?}");
            seen = [seen[0].min(s[2]), seen[1].max(s[2])];
        }
        assert!(seen[0] < -2000.0 && seen[1] > 2000.0, "the middle term spreads over ±2400: {seen:?}");
        let d = p.drag.expect("a drag");
        assert_eq!((d.lin, d.ang, d.per), ([1.43e-6, 1.41e-6, 1.45e-6], [7.27e-7, 7.36e-7, 7.3e-7], 0.015));
        assert_eq!((p.arm, p.sticky_arm_time, p.arm_time()), (0.8, 0.0, 0.8));
    }

    /// 5.4, tests 35, 36: in the air 85 % at arming (0.8 s), all of it at
    /// 2.8 s; stuck always 146.
    #[test]
    fn the_air_radius_grows_35_36() {
        let p = params();
        assert!((p.radius_at(0.8, false) - 124.1).abs() < 0.01);
        assert!((p.radius_at(2.8, false) - 146.0).abs() < 1e-3);
        assert_eq!(p.radius_at(0.8, true), 146.0);
    }
}
