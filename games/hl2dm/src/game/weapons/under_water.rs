//! Guns that won't fire with the shooter's eyes under water
//! (specs/hl2dm/weapons.md 5's table, 7.2-7.4): the .357, the SMG1, the
//! AR2 and the shotgun (single and double shot) name
//! `hl2dm:dry_under_water` (`Hl2DryUnderWater`); the pistol, the crossbow
//! and the melee weapons don't. Pressing attack with the eyes under water
//! (`LaunchUnderWater`, the movement's water level 3) when the gun would
//! fire plays its empty sound and sets its next primary `wait` (0.2 s,
//! 14 ticks) on; no round is used and the frame fires nothing this tick.
//! The clip's own rules come first (weapons.md 5, "Order against the
//! empty-clip rules"): an empty clip dry-fires or reloads as on land; a
//! refusal ends the weapon frame (`Inventory::frame_ended`), so a reload
//! pressed with attack held never starts. The machine guns' attack2s are
//! refused in `alt_fire` (the generic frame's refusal, before their own
//! code, with this hook's wait), the shotgun's double shot here.
//!
//! Known shortcuts: a shotgun's shell reload stopped by attack under water
//! refuses on that tick (the spec has the stop remember the shot and fire
//! it once the next primary has passed, which is now on our frame).

use bevy::prelude::*;
use serde::Deserialize;

use chimera_sim::weapon::{
    Inventory, Magazine, ShellStage, WeaponSounds, WeaponState, CHAN_ITEM, projectile::LaunchUnderWater,
};

use super::shotgun::Hl2Shotgun;
use crate::{
    core::{FirstTimePredicted, Health, Intent, SimClock},
    map::PlaySound,
};

/// `hl2dm:dry_under_water`'s parameters: the wait after a refusal.
#[derive(Component, Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Hl2DryUnderWater {
    pub wait: f32,
}

/// Before the weapon frame: a press that would fire under water is refused.
#[allow(clippy::type_complexity)]
pub fn before_frame(
    mut owners: Query<(Entity, &Intent, &mut Inventory, &Transform, &Health), (With<LaunchUnderWater>, Without<crate::map::interp::NetDrawn>)>,
    mut guns: Query<(&Hl2DryUnderWater, &mut WeaponState, &Magazine, Option<&WeaponSounds>, Option<&Hl2Shotgun>)>,
    mut play: MessageWriter<PlaySound>,
    clock: Res<SimClock>,
    first: Res<FirstTimePredicted>,
) {
    let now = clock.now;
    let due = now + 1e-5;
    for (owner, intent, mut inv, at, health) in &mut owners {
        if health.current <= 0.0 || inv.frame_ended {
            continue;
        }
        let Some(Ok((dry, mut st, mag, sounds, shotgun))) = inv.active.map(|a| guns.get_mut(a)) else { continue };
        // The shotgun's attack2 is its double shot.
        let pressed = intent.fire || (shotgun.is_some() && intent.secondary);
        if !pressed
            || mag.clip == 0
            || st.reload_end.is_some()
            || shotgun.is_some_and(|s| s.pump_pending)
            || due < inv.next_attack
            || st.next_primary > due
        {
            continue;
        }
        // A held attack ends a shell reload, as it would to fire.
        if st.shells != ShellStage::Idle {
            st.shells = ShellStage::Idle;
            st.postfire_end = None;
        }
        st.next_primary = now + dry.wait as f64;
        // The refusal ends the frame: the reload key isn't looked at.
        inv.frame_ended = true;
        if first.0
            && let Some(entry) = sounds.and_then(|s| s.empty.clone())
        {
            play.write(PlaySound {
                pitch: None,
                entry,
                at: Some(at.translation),
                volume: None,
                source: Some(owner),
                channel: Some(CHAN_ITEM),
            });
        }
    }
}
