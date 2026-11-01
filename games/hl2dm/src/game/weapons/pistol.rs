//! The pistol's own rules over the shared frame (specs/hl2dm/weapons.md
//! 7.1), `hl2dm:pistol` (`Hl2Pistol`):
//!
//! - **Accuracy penalty** P (s): +0.2 after each shot (unclamped), and
//!   once a tick while attack isn't held and the last shot is more than
//!   0.1 s old, -one tick, then clamped to [0, 1.5]; a reload's start
//!   zeroes it. The cone (`SpreadShape::Cone`) is 1 degree at P 0 and 6 at
//!   1.5, linearly between.
//! - **Click refire**: attack let go more than 0.1 s after the last shot
//!   (0.2 after a dry fire) makes the next press fire at once (the next
//!   primary goes to now - 0.1): 8 ticks between the fastest clicks.
//! - **Attack2 blocks it**: while attack2 is held (not reloading) the next
//!   primary, the unlock and the last shot are pushed to now + 0.1.
//! - **Dry fire**: its sequence's 0.3667 s before the next attack.
//!
//! Its kick is `recoil::Hl2Kick` (zeroed punch, then down).

use bevy::prelude::*;
use serde::Deserialize;

use chimera_sim::weapon::{Hitscan, Inventory, NetPart, SpreadShape, Weapon, WeaponEvent, WeaponEventKind, WeaponState};

use crate::core::{Intent, SimClock};

/// `hl2dm:pistol`'s parameters (weapons.md, constants).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PistolParams {
    pub penalty_per_shot: f32,
    pub penalty_max: f32,
    /// The cone at P 0 and at `penalty_max`.
    pub spread: [f32; 2],
    pub click_unlock: f32,
    pub dry_unlock: f32,
    pub dry: f32,
}

/// The pistol's penalty and click timers.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Hl2Pistol {
    pub params: PistolParams,
    /// The accuracy penalty P, seconds.
    pub penalty: f32,
    pub last_shot: f64,
    /// Attack let go after this makes the next press fire at once.
    pub unlock_after: f64,
}

impl Hl2Pistol {
    pub fn new(params: PistolParams) -> Self {
        Self {
            params,
            penalty: 0.0,
            last_shot: f64::NEG_INFINITY,
            unlock_after: f64::NEG_INFINITY,
        }
    }

    /// The cone for the penalty now.
    pub fn spread(&self) -> f32 {
        let [rest, most] = self.params.spread;
        let ramp = (self.penalty / self.params.penalty_max).clamp(0.0, 1.0);
        rest + ramp * (most - rest)
    }
}

impl NetPart for Hl2Pistol {
    type Net = (f32, f64, f64);
    fn net(&self) -> Self::Net {
        (self.penalty, self.last_shot, self.unlock_after)
    }
    fn set_net(&mut self, (penalty, last_shot, unlock_after): Self::Net) {
        self.penalty = penalty;
        self.last_shot = last_shot;
        self.unlock_after = unlock_after;
    }
}

/// Before the weapon frame: attack2's block, the penalty's decay and the
/// cone for this tick's shot.
pub fn before_frame(
    owners: Query<(&Intent, &Inventory), Without<crate::map::interp::NetDrawn>>,
    mut pistols: Query<(&mut Hl2Pistol, &mut WeaponState, &mut Hitscan)>,
    clock: Res<SimClock>,
) {
    let (now, dt) = (clock.now, clock.dt());
    for (intent, inv) in &owners {
        let Some(Ok((mut p, mut st, mut scan))) = inv.active.map(|a| pistols.get_mut(a)) else { continue };
        if intent.secondary && !st.reloading() {
            let t = now + p.params.click_unlock as f64;
            st.next_primary = t;
            p.last_shot = t;
            p.unlock_after = t;
        }
        if !intent.fire && now > p.last_shot + p.params.click_unlock as f64 {
            p.penalty = (p.penalty - dt).clamp(0.0, p.params.penalty_max);
        }
        let cone = SpreadShape::Cone(p.spread());
        if scan.spread != cone {
            scan.spread = cone;
        }
    }
}

/// After the weapon frame: each shot's penalty, a dry fire's wait, a
/// reload's reset and the click refire.
pub fn after_frame(
    mut events: MessageReader<WeaponEvent>,
    owners: Query<(&Intent, &Inventory), Without<crate::map::interp::NetDrawn>>,
    mut pistols: Query<(&mut Hl2Pistol, &mut WeaponState), With<Weapon>>,
    clock: Res<SimClock>,
) {
    let now = clock.now;
    for e in events.read() {
        let Ok((mut p, mut st)) = pistols.get_mut(e.weapon) else { continue };
        match e.kind {
            WeaponEventKind::Fired { .. } => {
                p.penalty += p.params.penalty_per_shot;
                p.last_shot = now;
                p.unlock_after = now + p.params.click_unlock as f64;
            }
            WeaponEventKind::DryFire => {
                st.next_primary = now + p.params.dry as f64;
                p.unlock_after = now + p.params.dry_unlock as f64;
            }
            WeaponEventKind::ReloadStarted => p.penalty = 0.0,
            _ => {}
        }
    }
    for (intent, inv) in &owners {
        let Some(Ok((p, mut st))) = inv.active.map(|a| pistols.get_mut(a)) else { continue };
        if intent.fire || st.reloading() || now < inv.next_attack {
            continue;
        }
        if now > p.unlock_after {
            let t = now - p.params.click_unlock as f64;
            if st.next_primary > t {
                st.next_primary = t;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cone_ramps_from_one_to_six_degrees() {
        let mut p = Hl2Pistol::new(PistolParams {
            penalty_per_shot: 0.2,
            penalty_max: 1.5,
            spread: [0.00873, 0.05234],
            click_unlock: 0.1,
            dry_unlock: 0.2,
            dry: 0.3667,
        });
        assert!((p.spread() - 0.00873).abs() < 1e-6);
        // weapons.md test 7: P 0.37 gives 0.01949.
        p.penalty = 0.37;
        assert!((p.spread() - 0.01949).abs() < 1e-4, "{}", p.spread());
        p.penalty = 1.6;
        assert!((p.spread() - 0.05234).abs() < 1e-6);
    }
}
