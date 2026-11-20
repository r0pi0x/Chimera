//! The Syringe Gun's needles (specs/tf2/weapons_more.md 8), on Chimera's
//! flyers: `chimera:projectile` flies them (1000 units/s from 16 forward,
//! 6 right, 8 down of the eye toward the point a 2000-unit line from the
//! eye meets, turned +-1.5 degrees in pitch and yaw at random, falling at
//! 0.3 of gravity, through teammates, their touches reported:
//! weapons_more.md's corrections from the SDK); `tf2:needle`
//! (`NeedleParams`) answers (`touch`, the server's, in `FlyerSet::Touch`):
//! on an enemy `damage` (10) x the short-range-reduced modifier M_s
//! between the Medic's and the enemy's centres at impact (121.8 % at 0),
//! rounded, of kind bullet naming the gun; any touch ends the needle.
//! A touch on anything but a player marks it as a bullet's impact
//! (`marks`, Chimera's `projectile::FlyerMark`: the surface's decal,
//! debris and sound, shown as the needle drawn late reaches it; effects.md
//! "Needles on a wall").
//!
//! Known shortcuts: the needle is a point, not a 2-unit cube; needles
//! don't destroy stickies; crits are off.

use std::sync::Arc;

use bevy::prelude::*;
use serde::Deserialize;

use chimera_sim::weapon::{
    data::WeaponSet,
    projectile::{FlyerGone, FlyerHit},
};

use super::{
    blast::BlastWorld,
    damage::{HP, U, modifier, round_points},
};
use crate::core::{Damage, DamageKind, Hitgroup};

/// `tf2:needle`'s parameters (points).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NeedleParams {
    pub damage: f32,
}

/// The needles' rules and ramps by weapon id.
#[derive(Resource, Clone, Debug, Default)]
pub struct NeedleRules(pub Vec<(String, Arc<NeedleParams>, f32)>);

impl NeedleRules {
    pub fn from_set(set: &WeaponSet) -> Self {
        let mut out = Vec::new();
        for w in &set.weapons {
            let Some(p) = w.hooks.get(super::NEEDLE_HOOK) else { continue };
            let Ok(p) = p.clone().try_into::<NeedleParams>() else { continue };
            let ramp = super::damage::ramp_of(w.hooks.get(super::GUN_HOOK));
            out.push((w.id.clone(), Arc::new(p), ramp));
        }
        Self(out)
    }

    pub fn get(&self, weapon: &str) -> Option<(&NeedleParams, f32)> {
        self.0.iter().find(|(id, ..)| id == weapon).map(|(_, p, k)| (&**p, *k))
    }
}

/// The damage of a needle at `units` from the shooter (points, rounded).
pub fn needle_damage(p: &NeedleParams, ramp: f32, units: f32) -> f32 {
    round_points(p.damage * modifier(units, ramp) * HP) / HP
}

#[allow(clippy::too_many_arguments)]
pub fn touch(
    mut hits: MessageReader<FlyerHit>,
    rules: Res<NeedleRules>,
    mut world: BlastWorld,
    mut gone: MessageWriter<FlyerGone>,
    mut commands: Commands,
) {
    for hit in hits.read() {
        let Some((p, ramp)) = rules.get(hit.weapon) else { continue };
        if let Some(o) = hit.other
            && hit.character
            && let Some(c) = world.centre(o)
            && hit.owner.is_none_or(|w| !world.mates(w, o))
        {
            let d = hit.owner.and_then(|w| world.centre(w)).map_or(0.0, |w| w.distance(c) / U);
            let amount = needle_damage(p, ramp, d) * HP;
            let dir = hit.velocity.normalize_or(Vec3::NEG_Z);
            world.hurt(
                Damage {
                target: o,
                attacker: hit.owner,
                amount,
                point: hit.point,
                dir,
                hitgroup: Hitgroup::Generic,
                kind: DamageKind::Bullet,
                weapon: Some(hit.weapon),
                force: Vec3::ZERO,
                },
                hit.point - dir,
            );
        }
        gone.write(FlyerGone { flyer: hit.flyer, weapon: hit.weapon, owner: hit.owner, at: hit.point, expired: false });
        commands.entity(hit.flyer).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test case 48: 12.18 / 10 / 5.28 at 0 / 512 / 1024 (M_s), rounded.
    #[test]
    fn a_needle_by_distance_48() {
        use super::super::damage::COMPRESSED;
        let p = NeedleParams { damage: 10.0 };
        assert_eq!(needle_damage(&p, COMPRESSED, 0.0), 12.0);
        assert_eq!(needle_damage(&p, COMPRESSED, 512.0), 10.0);
        assert_eq!(needle_damage(&p, COMPRESSED, 1024.0), 5.0);
    }
}
