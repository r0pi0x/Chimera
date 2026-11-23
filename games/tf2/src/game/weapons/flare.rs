//! The flare guns' flare (specs/tf2/batch2_mechanics.md 4): the hook
//! `tf2:flare` (`FlareParams`) on Chimera's flyers (`chimera:projectile`
//! flies it: 2000 units/s from 23.5 forward, 12 right, 3 down of the eye
//! toward what the eye's line meets, falling at 0.3 of gravity, through
//! teammates, its touches reported). Its `mode` (the item's
//! `set_weapon_mode`, `items::docs`) says what it does (`touch`, the
//! server's, in `FlyerSet::Touch`):
//!
//! - **flare** (the Flare Gun): on an enemy its `damage` (30, no distance
//!   modifier) of kind bullet naming the gun, gone; the world ends it.
//! - **detonator**: the same hit; the world ends it with no explosion;
//!   attack2 with it out explodes every flare of its holder's in flight
//!   where it is (`detonate`).
//! - **scorch** (the Scorch Shot): its hit, then its own push on the
//!   player (`scorch_push` along its horizontal flight and up, × 4 on a
//!   burning one, a grounded one lifted at least `ground_lift`), and it
//!   bounces off him (`bounce`: forward and up, still falling) to explode
//!   on the world, the player it hit taking the explosion whole.
//!
//! Every hit pushes as a bullet's (6 × its damage, `blast::BlastWorld::
//! knock`). The explosion (`explode`) is the shared one
//! (`blast::BlastWorld::detonate`): `damage` × the splash to `edge` at
//! `radius`, no distance modifier; its owner takes `self_damage` (the
//! class's base × `blast dmg to self`) by the provisional law of 4.3
//! (`self_law`), pushed by `push_self`. Crits and mini-crits on burning
//! victims are the items' (`items::effects`), the burn their after-hit's
//! (`burn` s of `burn_damage` every `burn_interval`, from its tick).
//!
//! Known shortcuts (the spec's open questions, provisional): the owner's
//! own explosion damage law (45 × (1 - r / 206), 32 from 56 to 61 units:
//! 34 at 49 units, where 35 was measured) and push (5.6 per point); a flare touching
//! the world goes at once (the game's stays 0.1 s); its life is 60 s;
//! the flare shows no trail and a crit no glow (no spec or script names
//! them: the plan's spec list), its model only; the Flare Gun's flare
//! ending on the world shows nothing (the script's `ExplosionEffect` is
//! drawn for the Detonator's and the Scorch Shot's explosions only); the
//! view model plays the `item1` activities by activity (their
//! `anim_slot`).
//!
//! Looks and sounds (data/weapons/flaregun.toml, the class script's
//! names): the muzzle flash `muzzle_shotgun`; the explosion
//! `flaregun_destroyed` wherever it goes off, with `Default.FlareImpact`
//! (`sound`, `blast::BlastSounds`); the arms' `fg_` sequences' sound
//! events (draw, grab, the tube's reload) and the script's entries are
//! loaded with the guns' (`sets::FLARE_GUN_SOUNDS`).

use std::sync::Arc;

use bevy::prelude::*;
use serde::Deserialize;

use chimera_sim::weapon::{
    Inventory, Weapon,
    data::WeaponSet,
    projectile::{Flyer, FlyerGone, FlyerHit},
};

use super::{
    blast::{Blast, BlastParams, BlastWorld, Placement},
    damage::{HP, U},
};
use crate::core::{Damage, DamageKind, Health, Hitgroup, Intent};

/// What a flare does (module doc).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Flare,
    Detonator,
    Scorch,
}

/// `tf2:flare`'s parameters (units, points, seconds).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FlareParams {
    pub damage: f32,
    pub mode: Mode,
    pub burn: f32,
    pub burn_damage: f32,
    pub burn_interval: f32,
    pub radius: f32,
    pub edge: f32,
    pub self_damage: f32,
    #[serde(default = "one")]
    pub self_scale: f32,
    pub push_other: f32,
    pub push_self: f32,
    pub scorch_push: f32,
    pub scorch_push_burning: f32,
    pub ground_lift: f32,
    pub bounce: [f32; 2],
    /// The explosion's sound entry (the script's `ExplosionSound`),
    /// played with its look (`blast::BlastSounds`).
    #[serde(default)]
    pub sound: Option<String>,
    /// An enemy Pyro's burn, in look only (weapons_more.md 7.3).
    #[serde(default = "pyro_burn")]
    pub pyro_burn: f32,
}

fn one() -> f32 {
    1.0
}

fn pyro_burn() -> f32 {
    0.25
}

impl FlareParams {
    /// Its explosion as the shared blast's numbers.
    pub fn blast(&self) -> BlastParams {
        BlastParams {
            damage: self.damage,
            radius: self.radius,
            edge: self.edge,
            self_radius: Some(self.radius),
            self_damage: Some(self.self_damage * self.self_scale),
            self_air_scale: 1.0,
            self_ground_scale: 1.0,
            fuse_scale: None,
            push_other: self.push_other,
            push_self_air: self.push_self,
            push_self_ground: self.push_self,
            by_distance: false,
            sound: None,
            bounce_sound: None,
            self_scale: 1.0,
            self_harm: true,
            first_touch_keep: None,
            shatter: false,
        }
    }
}

/// The owner's own explosion at r units, as a share of his base
/// (batch2_mechanics.md 4.3, provisional): 1 - r / 206, the measured 32
/// of 45 from 56 to 61 units.
pub fn self_law(r: f32) -> f32 {
    if (56.0..=61.0).contains(&r) { 32.0 / 45.0 } else { (1.0 - r / 206.0).max(0.0) }
}

/// The Scorch Shot's push on the player it hit (units/s): along the
/// flare's horizontal `dir`, × 4 on a burning one, a grounded one lifted
/// at least `ground_lift` (4.4).
pub fn scorch_push(p: &FlareParams, dir: Vec3, burning: bool, grounded: bool) -> Vec3 {
    let push = if burning { p.scorch_push_burning } else { p.scorch_push };
    let flat = dir.with_y(0.0).normalize_or_zero();
    let up = if grounded { push.max(p.ground_lift) } else { push };
    flat * push + Vec3::Y * up
}

/// The flares' rules by weapon id.
#[derive(Resource, Clone, Debug, Default)]
pub struct FlareRules(pub Vec<(String, Arc<FlareParams>)>);

impl FlareRules {
    pub fn from_set(set: &WeaponSet) -> Self {
        Self(
            set.weapons
                .iter()
                .filter_map(|w| Some((w.id.clone(), Arc::new(w.hooks.get(super::FLARE_HOOK)?.clone().try_into::<FlareParams>().ok()?))))
                .collect(),
        )
    }

    pub fn get(&self, weapon: &str) -> Option<&FlareParams> {
        self.0.iter().find(|(id, _)| id == weapon).map(|(_, p)| &**p)
    }
}

/// On a Scorch Shot's flare after it hit a player: him.
#[derive(Component, Clone, Copy, Debug)]
pub struct ScorchHit(pub Entity);

/// A flare's explosion of `weapon` by `owner` at `at`, `touched` taking
/// it whole.
fn explode(world: &mut BlastWorld, p: &FlareParams, at: &Placement, owner: Option<Entity>, weapon: &'static str, touched: Option<Entity>) {
    let params = p.blast();
    let blast = Blast { params: &params, radius: p.radius, ramp: None, scale: 1.0, mini_crit: false, self_law: Some(self_law) };
    world.detonate(&blast, at, owner, weapon, touched);
}

#[allow(clippy::too_many_arguments)]
pub fn touch(
    mut hits: MessageReader<FlyerHit>,
    rules: Res<FlareRules>,
    mut flyers: Query<(&mut Flyer, Option<&ScorchHit>)>,
    burning: Query<&super::flame::Burning>,
    mut world: BlastWorld,
    mut gone: MessageWriter<FlyerGone>,
    mut commands: Commands,
) {
    for hit in hits.read() {
        let Some(p) = rules.get(hit.weapon) else { continue };
        let Ok((mut f, scorched)) = flyers.get_mut(hit.flyer) else { continue };
        let enemy = hit.character && hit.other.is_some_and(|o| hit.owner.is_none_or(|w| !world.mates(w, o)));
        let mut end = true;
        if enemy && let Some(o) = hit.other {
            // Unrounded: the conditions round after their factor
            // (`items::effects::crits`: 22.5 x 1.35 = 30).
            let points = p.damage * HP;
            let dir = hit.velocity.normalize_or(Vec3::NEG_Z);
            world.hurt(
                Damage {
                    target: o,
                    attacker: hit.owner,
                    amount: points,
                    point: hit.point,
                    dir,
                    hitgroup: Hitgroup::Generic,
                    kind: DamageKind::Bullet,
                    weapon: Some(hit.weapon),
                    force: Vec3::ZERO,
                },
                hit.point - dir,
            );
            world.knock(o, hit.point - dir * 16.0 * U, p.damage, super::blast::BULLET_PUSH);
            if p.mode == Mode::Scorch {
                let lit = burning.get(o).is_ok_and(|b| !b.harmless && b.left > 0.0);
                world.add_push(o, scorch_push(p, dir, lit, world.grounded(o)));
                // It bounces off him and flies on (4.4).
                let flat = (hit.velocity / U).with_y(0.0).normalize_or_zero();
                f.velocity = (flat * p.bounce[0] + Vec3::Y * p.bounce[1]) * U;
                f.passed.push(o);
                commands.entity(hit.flyer).insert(ScorchHit(o));
                end = false;
            }
        } else if hit.character {
            // A teammate: it flies on through.
            if let Some(o) = hit.other {
                f.passed.push(o);
                f.velocity = hit.velocity;
            }
            end = false;
        } else if p.mode == Mode::Scorch {
            let at = world.touched(hit.point, hit.normal, None);
            explode(&mut world, p, &at, hit.owner, hit.weapon, scorched.map(|s| s.0));
        }
        if end {
            gone.write(FlyerGone { flyer: hit.flyer, weapon: hit.weapon, owner: hit.owner, at: hit.point, expired: false });
            commands.entity(hit.flyer).despawn();
        }
    }
}

/// Attack2 with a detonating flare gun out: every flare of its holder's
/// in flight explodes where it is (4.3).
#[allow(clippy::too_many_arguments)]
pub fn detonate(
    owners: Query<(&Intent, &Inventory, Option<&Health>)>,
    weapons: Query<&Weapon>,
    flares: Query<(Entity, &Flyer, &Transform)>,
    rules: Res<FlareRules>,
    mut world: BlastWorld,
    mut gone: MessageWriter<FlyerGone>,
    mut commands: Commands,
) {
    if !rules.0.iter().any(|(_, p)| p.mode == Mode::Detonator) {
        return;
    }
    for (e, f, t) in &flares {
        let Some(p) = rules.get(f.weapon).filter(|p| p.mode == Mode::Detonator) else { continue };
        let Some(owner) = f.owner else { continue };
        let Ok((intent, inv, health)) = owners.get(owner) else { continue };
        let holding = inv.active.and_then(|w| weapons.get(w).ok()).is_some_and(|w| rules.get(w.id).is_some_and(|q| q.mode == Mode::Detonator));
        if !intent.secondary || !holding || health.is_some_and(|h| h.current <= 0.0) {
            continue;
        }
        let at = Placement { centre: t.translation, ..default() };
        explode(&mut world, p, &at, Some(owner), f.weapon, None);
        gone.write(FlyerGone { flyer: e, weapon: f.weapon, owner: Some(owner), at: t.translation, expired: false });
        commands.entity(e).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scorch_push_and_the_owners_law_follow_4_3_and_4_4() {
        let doc: toml::Table = super::super::DATA.iter().find(|(n, _)| *n == "flaregun.toml").unwrap().1.text().parse().unwrap();
        let p: FlareParams = doc["hooks"][super::super::FLARE_HOOK].clone().try_into().unwrap();
        let v = scorch_push(&p, Vec3::NEG_Z, false, false);
        assert!((v.z + 100.0).abs() < 1e-3 && (v.y - 100.0).abs() < 1e-3);
        assert!((scorch_push(&p, Vec3::NEG_Z, false, true).y - 268.3).abs() < 1e-3);
        let b = scorch_push(&p, Vec3::NEG_Z, true, false);
        assert!((b.z + 400.0).abs() < 1e-3 && (b.y - 400.0).abs() < 1e-3);
        // 36 and 35 at 41 and 44 units, the clamp's 32 from 56 to 61 (at 49
        // the law gives 34.3: the spec's "35" there is not its law's).
        let own = |r: f32| (45.0 * self_law(r)).round();
        assert_eq!((own(41.0), own(44.0), own(58.0)), (36.0, 35.0, 32.0));
    }
}
