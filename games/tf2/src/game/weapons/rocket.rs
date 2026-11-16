//! The Rocket Launcher's rocket (specs/tf2/weapons.md 5), on Chimera's
//! flyers: `chimera:projectile` flies it (1100 units/s, no gravity, from
//! the eye 23.5 forward, 12 right, 3 down (crouched 8 up), through its
//! owner and his teammates, reporting its touch); `tf2:rocket`
//! (`blast::BlastParams`) explodes it where it touched (`explode`, the
//! server's, in `FlyerSet::Touch`) with the shared explosion
//! (`blast::BlastWorld::detonate`: 90 x the splash to 50 % at 146 units
//! x the distance modifier with the gun's ramp 0.5, the touched player
//! full splash, the owner's own 90 x the splash to 121 units x0.6
//! airborne, the push 6 / 10 / 5 per point capped at 1000 units/s).
//!
//! The rocket heads for what a 2000-unit line from the eye meets beyond
//! 200 units (else the line's end), from its offset pulled back by the
//! world (`chimera:projectile` `aim_trace`, `start_trace`; weapons.md 5.1).
//!
//! Known shortcuts: the blast's (`blast`).

use std::sync::Arc;

use bevy::prelude::*;

use chimera_sim::weapon::{
    data::WeaponSet,
    projectile::{Flyer, FlyerGone, FlyerHit},
};

pub use super::blast::{BlastParams as RocketParams, LIFT, STANDING, push, splash};
use super::{blast::{Blast, BlastWorld}, damage::U};

/// The weapon part (the rocket's rules are read from `RocketRules`, as
/// the gun may be gone when its rocket lands).
#[derive(Component, Clone, Debug)]
pub struct Tf2Rocket(pub RocketParams);

/// Each blast weapon's rules and ramp, by weapon id, for one hook
/// (`tf2:rocket` or `tf2:grenade`).
#[derive(Resource, Clone, Debug, Default)]
pub struct RocketRules(pub Vec<(String, Arc<RocketParams>, f32)>);

impl RocketRules {
    /// The rules of every weapon in `set` with `tf2:rocket`.
    pub fn from_set(set: &WeaponSet) -> Self {
        Self::of_hook(set, super::ROCKET_HOOK)
    }

    /// The rules of every weapon in `set` with `hook`.
    pub fn of_hook(set: &WeaponSet, hook: &str) -> Self {
        let mut out = Vec::new();
        for w in &set.weapons {
            let Some(p) = w.hooks.get(hook) else { continue };
            let Ok(p) = p.clone().try_into::<RocketParams>() else { continue };
            let ramp = super::damage::ramp_of(w.hooks.get(super::GUN_HOOK));
            out.push((w.id.clone(), Arc::new(p), ramp));
        }
        Self(out)
    }

    pub fn get(&self, weapon: &str) -> Option<(&RocketParams, f32)> {
        self.0.iter().find(|(id, ..)| id == weapon).map(|(_, p, k)| (&**p, *k))
    }
}

pub fn explode(
    mut hits: MessageReader<FlyerHit>,
    rules: Res<RocketRules>,
    flyers: Query<&Flyer>,
    mut world: BlastWorld,
    mut gone: MessageWriter<FlyerGone>,
    mut commands: Commands,
) {
    for hit in hits.read() {
        let Some((p, ramp)) = rules.get(hit.weapon) else { continue };
        if !flyers.contains(hit.flyer) {
            continue;
        }
        // Just off the surface it touched.
        let centre = hit.point + hit.normal * U;
        let blast = Blast { params: p, radius: p.radius, ramp: p.by_distance.then_some(ramp), scale: 1.0 };
        world.detonate(&blast, centre, hit.owner, hit.weapon, hit.other);
        gone.write(FlyerGone {
            flyer: hit.flyer,
            weapon: hit.weapon,
            owner: hit.owner,
            at: centre,
            expired: false,
        });
        commands.entity(hit.flyer).despawn();
    }
}
