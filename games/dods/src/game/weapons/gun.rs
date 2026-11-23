//! A DoD:S gun's own rules over the shared weapon frame (`dods:gun`,
//! `DodsGun`; specs/dods/weapons.md):
//!
//! - **Spread**: the script's `Accuracy` as the cone, `SecondaryAccuracy`
//!   while the sights or scope are up (`AltModes` past its first mode),
//!   plus `AccuracyMovePenalty` while its holder moves (`before_frame`).
//! - **Sights block the trigger**: raising or lowering the sights or
//!   scope (the data's `[alt]` scope, `step` 0.49 s) blocks the next shot
//!   for that time (measured on the Garand, weapons.md "Alt fire"): the
//!   hook makes the step block the primary (`AltModes::blocks_primary`).
//! - **Reload only when empty** (`reload_when_empty`, the Garand's:
//!   weapons.md "Fire modes"): the reload key does nothing while rounds
//!   are left (`WeaponState::reload_held`).
//!
//! Known shortcuts (docs/tech-debt.md):
//! - Spread is a plain cone of the script's number; how it combines with
//!   crouch and prone, and the spread pattern, are unmeasured
//!   (weapons.md, open question 1). "Moving" is any ground speed over
//!   `MOVING` (unmeasured).
//! - `Recoil` is kept in the parameters but gives no view kick: its shape
//!   and recovery are unmeasured (weapons.md, open question 8).
//! - The BAR's and StG44's sighted fire delay (0.3, `SecondaryFireDelay`)
//!   is not applied (weapons.md, open question 3); the sights' and
//!   scopes' fields of view are stand-ins (unmeasured); the view model
//!   doesn't move to its iron-sight offset.
//! - The SMGs' stock strike (their alt fire) is not there (unmeasured).

use bevy::prelude::*;
use serde::Deserialize;

use chimera_sim::weapon::{AltModes, Hitscan, Inventory, Magazine, SpreadShape, WeaponState};

use crate::core::Velocity;

/// Ground speed over which a holder counts as moving, m/s (unmeasured:
/// one unit a second).
pub const MOVING: f32 = 0.0254;

/// `dods:gun`'s parameters: the script's numbers (`script_numbers`).
#[derive(Component, Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DodsGun {
    /// The script it reads (`weapon_garand`).
    pub script: String,
    /// `Accuracy`: the cone standing still with the sights down.
    pub accuracy: f32,
    /// `SecondaryAccuracy`: the cone with the sights up (none: the gun
    /// has none).
    #[serde(default)]
    pub alt_accuracy: Option<f32>,
    /// `AccuracyMovePenalty`: added while moving.
    #[serde(default)]
    pub move_penalty: f32,
    /// `Recoil`, degrees (no kick yet: module doc).
    #[serde(default)]
    pub recoil: f32,
    /// The reload key does nothing while the clip holds rounds.
    #[serde(default)]
    pub reload_when_empty: bool,
}

impl DodsGun {
    /// The cone for a shot: sighted or not, moving or not.
    pub fn cone(&self, sighted: bool, moving: bool) -> f32 {
        let base = match (sighted, self.alt_accuracy) {
            (true, Some(a)) => a,
            _ => self.accuracy,
        };
        base + if moving { self.move_penalty } else { 0.0 }
    }
}

/// On a gun's entity as the hook puts it: its sights' step blocks the
/// trigger.
pub fn insert(e: &mut EntityWorldMut, p: &DodsGun) {
    if let Some(mut m) = e.get_mut::<AltModes>() {
        m.blocks_primary = true;
    }
    e.insert(p.clone());
}

/// Before the weapon frame: the cone for this tick's shot and the
/// Garand's reload hold.
#[allow(clippy::type_complexity)]
pub fn before_frame(
    owners: Query<(&Inventory, Option<&Velocity>), Without<crate::map::interp::NetDrawn>>,
    mut guns: Query<(&DodsGun, &mut Hitscan, &mut WeaponState, Option<&AltModes>, Option<&Magazine>)>,
) {
    for (inv, velocity) in &owners {
        let Some(Ok((gun, mut scan, mut st, alt, mag))) = inv.active.map(|a| guns.get_mut(a)) else { continue };
        let moving = velocity.is_some_and(|v| v.0.with_y(0.0).length() > MOVING);
        let sighted = alt.is_some_and(|a| a.current > 0);
        let cone = SpreadShape::Cone(gun.cone(sighted, moving));
        if scan.spread != cone {
            scan.spread = cone;
        }
        if gun.reload_when_empty {
            let held = mag.is_some_and(|m| m.clip > 0);
            if st.reload_held != held {
                st.reload_held = held;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cone_is_the_scripts_by_sights_and_movement() {
        let g = DodsGun {
            script: "weapon_garand".into(),
            accuracy: 0.014,
            alt_accuracy: Some(0.0),
            move_penalty: 0.1,
            recoil: 4.0,
            reload_when_empty: true,
        };
        assert_eq!(g.cone(false, false), 0.014);
        assert_eq!(g.cone(true, false), 0.0);
        assert!((g.cone(false, true) - 0.114).abs() < 1e-6);
        let smg = DodsGun { alt_accuracy: None, ..g };
        assert_eq!(smg.cone(true, false), 0.014, "no sights: the base cone");
    }
}
