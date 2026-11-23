//! Charged launches (specs/tf2/batch2b_mechanics.md 1, 7), the hook
//! `tf2:charge` (`ChargeParams`, the weapon part `Tf2Charge`): holding
//! attack draws the Huntsman or keeps the Loose Cannon's ball in the
//! barrel; letting go fires.
//!
//! - **The charge** (`charge`, predicted, before selection, as the
//!   Stickybomb Launcher's): a press of attack with a round in and the
//!   interval passed starts it; the frame is held back while it charges
//!   (its next attack time a step ahead, the button left alone); letting
//!   go lets one attack through, which fires a round.
//!   The charge c rises from 0 to 1 over `time` (1 s) and stays there.
//!   With `fuse` the round is fired when the fuse would run out in the
//!   barrel too (`fuse` s held), and the button must then be let go.
//! - **The launch** (Chimera's `Launcher::next`, taken by that round's
//!   flyer): speed `speed[0]` + (`speed[1]` − `speed[0]`) c (the bow's
//!   1800 to 2600), gravity `gravity[0]` + (`gravity[1]` − `gravity[0]`)
//!   c (× 800: 0.5 to 0.1), the flyer's life `fuse` − the time held (the
//!   cannon's 1 s fuse, started with the charge), and c carried on the
//!   flyer (`Flyer::charge`: the arrow's damage, `shot`).
//! - **Tiring** (1): held `tire` (5 s) or longer, the round leaves turned
//!   by up to `wobble` degrees in yaw and pitch at random (a new draw
//!   each shot, from the tick), at a full charge's speed.
//!
//! Known shortcuts: the Stickybomb Launcher keeps its own charge
//! (`sticky::charge`, same shape, but it holds the frame back by the
//! button, which an app's once-a-frame input may read as a release on a
//! frame's second tick); a Loose Cannon held to its fuse's end
//! fires a ball that goes off at once (the game's goes off in the
//! barrel, its own blast on its holder); the wobble's random draw is
//! ours (the game's stream is unmeasured).

use std::sync::Arc;

use bevy::prelude::*;
use serde::Deserialize;

use chimera_sim::weapon::{Inventory, Magazine, NetPart, WeaponState, projectile::{LaunchNext, Launcher}};

use super::damage::U;
use crate::core::{Health, Intent, SimClock};

/// Gravity, units/s² (batch2b_mechanics.md "Units and conventions").
pub const GRAVITY: f32 = 800.0;

/// `tf2:charge`'s parameters (units, seconds).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ChargeParams {
    /// Seconds to a full charge.
    pub time: f32,
    /// Launch speed at no and at a full charge (units/s); none: the
    /// launcher's.
    #[serde(default)]
    pub speed: Option<[f32; 2]>,
    /// Gravity scale at no and at a full charge (× `GRAVITY`); none: the
    /// launcher's.
    #[serde(default)]
    pub gravity: Option<[f32; 2]>,
    /// A fuse running from the charge's start (s): the flyer's life is
    /// what is left of it.
    #[serde(default)]
    pub fuse: Option<f32>,
    /// Held this long or more, the shot wobbles by up to `wobble`.
    #[serde(default)]
    pub tire: Option<f32>,
    /// Degrees in yaw and pitch.
    #[serde(default)]
    pub wobble: [f32; 2],
}

impl ChargeParams {
    /// The charge after holding `held` s (0 to 1).
    pub fn charge(&self, held: f32) -> f32 {
        if self.time <= 0.0 { 1.0 } else { (held / self.time).clamp(0.0, 1.0) }
    }

    /// The launch after holding `held` s (meters), `r` two numbers in
    /// [−1, 1] for a tired shot's wobble.
    pub fn launch(&self, held: f32, r: [f32; 2]) -> LaunchNext {
        let c = self.charge(held);
        let lerp = |a: [f32; 2]| a[0] + (a[1] - a[0]) * c;
        let tired = self.tire.is_some_and(|t| held + 1e-6 >= t);
        LaunchNext {
            speed: self.speed.map(|s| lerp(s) * U),
            gravity: self.gravity.map(|g| lerp(g) * GRAVITY * U),
            life: self.fuse.map(|f| (f - held).max(0.0)),
            turn: if tired { Vec2::new(self.wobble[1] * r[1], self.wobble[0] * r[0]) * (std::f32::consts::PI / 180.0) } else { Vec2::ZERO },
            charge: c,
        }
    }
}

/// A charging weapon's state (a predicted part).
#[derive(Component, Clone, Debug)]
pub struct Tf2Charge {
    pub params: Arc<ChargeParams>,
    /// Charging since.
    pub charging: Option<f64>,
    /// A fuse ran out with the button held: wait for its release.
    pub wait_release: bool,
}

impl NetPart for Tf2Charge {
    type Net = (Option<f64>, bool);
    fn net(&self) -> Self::Net {
        (self.charging, self.wait_release)
    }
    fn set_net(&mut self, (charging, wait_release): Self::Net) {
        self.charging = charging;
        self.wait_release = wait_release;
    }
}

/// `tf2:charge` on a weapon.
pub fn attach(e: &mut EntityWorldMut, p: &ChargeParams) {
    e.insert(Tf2Charge { params: Arc::new(p.clone()), charging: None, wait_release: false });
}

/// Two numbers in [−1, 1] from tick `tick` (a tired shot's wobble).
pub fn wobble_random(tick: u64) -> [f32; 2] {
    let mut x = (tick as u32).wrapping_mul(0x9E37_79B9) ^ 0x27d4_eb2d;
    std::array::from_fn(|_| {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        (x % 2001) as f32 / 1000.0 - 1.0
    })
}

/// Each charging weapon this tick (module doc). The frame is held back
/// by its next attack time (one step ahead each tick it charges), not by
/// the attack button: the button stays the player's, so a tick that reads
/// the same input as the last (several ticks to a frame) sees it held.
#[allow(clippy::type_complexity)]
pub fn charge(
    mut owners: Query<(&mut Intent, &Inventory, Option<&Health>)>,
    mut guns: Query<(&mut Tf2Charge, &mut WeaponState, Option<&Magazine>, Option<&mut Launcher>)>,
    clock: Res<SimClock>,
) {
    const HOLD: f64 = 1e-3;
    let now = clock.now;
    for (mut intent, inv, health) in &mut owners {
        let alive = health.is_none_or(|h| h.current > 0.0);
        for &w in &inv.weapons {
            let Ok((mut gun, mut st, mag, launcher)) = guns.get_mut(w) else { continue };
            if inv.active != Some(w) || !alive {
                if gun.charging.is_some() || gun.wait_release {
                    gun.charging = None;
                    gun.wait_release = false;
                }
                continue;
            }
            let held = intent.fire;
            if gun.wait_release {
                if held {
                    st.next_primary = st.next_primary.max(now + HOLD);
                    continue;
                }
                gun.wait_release = false;
            }
            match gun.charging {
                None => {
                    let ready = st.next_primary <= now + 1e-6 && mag.is_none_or(|m| m.clip > 0);
                    if held && ready {
                        gun.charging = Some(now);
                        st.next_primary = now + HOLD;
                    }
                }
                Some(start) => {
                    let h = (now - start) as f32;
                    let burnt = gun.params.fuse.is_some_and(|f| h + 1e-6 >= f);
                    if !held || burnt {
                        if let Some(mut l) = launcher {
                            l.next = Some(gun.params.launch(h, wobble_random(clock.tick)));
                        }
                        gun.charging = None;
                        gun.wait_release = held;
                        // One attack through: the frame fires the round.
                        st.next_primary = now;
                        intent.fire = true;
                    } else {
                        st.next_primary = now + HOLD;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bows_speed_and_gravity_follow_its_charge() {
        let p = ChargeParams { time: 1.0, speed: Some([1800.0, 2600.0]), gravity: Some([0.5, 0.1]), fuse: None, tire: Some(5.0), wobble: [6.0, 5.0] };
        for (held, speed, gravity) in [(0.0, 1800.0, 0.5), (0.5, 2200.0, 0.3), (1.0, 2600.0, 0.1), (3.0, 2600.0, 0.1)] {
            let l = p.launch(held, [1.0, 1.0]);
            assert!((l.speed.unwrap() / U - speed).abs() < 1e-2, "{held}");
            assert!((l.gravity.unwrap() / U / GRAVITY - gravity).abs() < 1e-4, "{held}");
            assert_eq!(l.turn, Vec2::ZERO);
        }
        let tired = p.launch(6.0, [-1.0, 1.0]);
        assert!((tired.turn.y.to_degrees() + 6.0).abs() < 1e-3 && (tired.turn.x.to_degrees() - 5.0).abs() < 1e-3);
        let cannon = ChargeParams { time: 1.0, speed: None, gravity: None, fuse: Some(1.0), tire: None, wobble: [0.0; 2] };
        assert!((cannon.launch(0.5, [0.0; 2]).life.unwrap() - 0.5).abs() < 1e-6);
    }
}
