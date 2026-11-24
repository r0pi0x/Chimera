//! When a DoD:S player's weapon can't fire (specs/dods/weapons.md, "When a
//! gun can't fire"), whatever it holds, before the weapon and throw frames
//! (`no_fire`, predicted): **sprinting** (the sprint flag,
//! `stamina::DodsStamina`), **in the air** (after the tick's movement) and
//! **crawling** (prone and moving), no gun fires and no grenade's pin is
//! pulled; a swing of the alt fire (the SMGs' stock strike) waits too.
//! The attack's next time is held one tick on, so fire held through the
//! block fires on the first tick it ends (measured: the tick a sprint ends,
//! the landing tick, once the crawl has stopped); a press inside it is
//! lost unless still held.

use bevy::prelude::*;

use chimera_sim::weapon::{Inventory, Melee, WeaponState};
use source_engine::source_movement::SourceMovement;

use crate::core::{SimClock, Velocity};
use crate::game::{stamina::DodsStamina, stance::DodsStance};

/// Whether a DoD:S player in this state can't attack: sprinting, off the
/// ground, or prone and moving.
pub fn blocked(sprinting: bool, on_ground: bool, crawling: bool) -> bool {
    sprinting || !on_ground || crawling
}

/// Before the weapon and throw frames: the active weapon's attacks held a
/// tick on while its holder can't attack.
#[allow(clippy::type_complexity)]
pub fn no_fire(
    owners: Query<(&Inventory, &DodsStamina, &DodsStance, &SourceMovement, Option<&Velocity>), Without<crate::map::interp::NetDrawn>>,
    mut weapons: Query<(&mut WeaponState, Option<&Melee>)>,
    clock: Res<SimClock>,
) {
    let next = clock.now + clock.dt() as f64;
    for (inv, stamina, stance, me, velocity) in &owners {
        let crawling = stance.prone() && velocity.is_some_and(|v| v.0.with_y(0.0).length_squared() > 0.0);
        if !blocked(stamina.sprinting, me.on_ground, crawling) {
            continue;
        }
        let Some(Ok((mut st, melee))) = inv.active.map(|a| weapons.get_mut(a)) else { continue };
        if st.next_primary < next {
            st.next_primary = next;
        }
        if melee.is_some_and(|m| m.secondary.is_some()) && st.next_secondary < next {
            st.next_secondary = next;
        }
    }
}
