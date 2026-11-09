//! The AR2's vents (specs/hl2dm/weapons.md 7.3, "AR2 vents"; test 61):
//! `hl2dm:vent` (`Hl2Vent`) names a view-model pose parameter
//! (`VentPoses`) that follows the consecutive-shot count n (the frame's
//! `WeaponState::burst`: +1 a shot, 0 when attack is let go): clamp(n /
//! `shots`, 0, 1), written each tick at the start of the weapon's frame,
//! before the shared frame, so it shows the count as the last tick left
//! it (one tick late; 0 on the tick after the release). Not written while
//! the player is busy (reload, draw): the vents hold. It jumps, no easing.
//! The value is kept on the weapon (`Hl2Vent::value`) and set on the
//! holder's view model, whose additive `VentPoses` sequence (autoplay)
//! blends along it (`map::ViewAnimator`).
//!
//! Known shortcuts: a deploy doesn't reset the count (the shared frame
//! resets it when attack is let go only; weapons.md 2 resets it at a
//! deploy too).

use bevy::prelude::*;
use serde::Deserialize;

use chimera_sim::weapon::{Inventory, WeaponState};

use crate::{
    core::{Health, SimClock},
    map::ViewAnimator,
};

/// `hl2dm:vent`'s parameters: the pose parameter and the shots to full.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct VentParams {
    pub pose: String,
    pub shots: u32,
}

/// A weapon whose vents follow its shots: the value last written.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Hl2Vent {
    pub params: VentParams,
    pub value: f32,
}

impl Hl2Vent {
    /// The pose for `n` consecutive shots.
    pub fn pose(&self, n: u32) -> f32 {
        (n as f32 / self.params.shots.max(1) as f32).clamp(0.0, 1.0)
    }
}

/// Before the weapon frame: the held weapon's vents from last tick's count.
pub fn before_frame(
    mut owners: Query<(&Inventory, &Health, Option<&mut ViewAnimator>), Without<crate::map::interp::NetDrawn>>,
    mut guns: Query<(&mut Hl2Vent, &WeaponState)>,
    clock: Res<SimClock>,
) {
    let due = clock.now + 1e-5;
    for (inv, health, view) in &mut owners {
        if health.current <= 0.0 || due < inv.next_attack {
            continue;
        }
        let Some(Ok((mut vent, st))) = inv.active.map(|a| guns.get_mut(a)) else { continue };
        let value = vent.pose(st.burst);
        vent.value = value;
        // On the view model: its parameter, and its additive layers on.
        if let Some(mut view) = view
            && let Some(a) = view.animator.as_mut()
            && let Some(i) = a.set.params.iter().position(|p| p.name.eq_ignore_ascii_case(&vent.params.pose))
        {
            let stored = a.set.params[i].encode(value);
            if let Some(slot) = a.params.get_mut(i) {
                *slot = stored;
            }
            a.autoplay = true;
        }
    }
}
