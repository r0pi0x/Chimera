//! The Sniper Rifle's charge and headshot (specs/tf2/weapons_more.md 3),
//! the hook `tf2:sniper` (`SniperParams`, the weapon part `Tf2Sniper`),
//! before the frame each tick (`charge`, predicted). The scope itself is
//! Chimera's (`[alt] kind = "scope"`: field of view 20, the holder at 80
//! units/s, the view model hidden, a shot unzooming it until the next shot
//! may fire, then zooming back: auto re-zoom).
//!
//! - τ, the time since the scope came up (`scoped_since`, reset by a shot
//!   that unzooms it), gives the charge c = min(`charge_max`, max(0,
//!   `charge_rate` (τ - `charge_delay`))): 0 until 0.3 s, 50 at 1.3 s, 150
//!   at 3.3 s. The shot's damage is max(`base`, c) scoped, `base` (50)
//!   unscoped; no distance modifier (`tf2:gun`'s `flat`).
//! - A head hit is x`headshot` (3, a crit) only while scoped for
//!   `headshot_delay` (0.2 s) or more; otherwise the head counts as the
//!   body.
//!
//! Known shortcuts: the headshot kill's icon (`headshot`) isn't told apart
//! from the rifle's in the kill feed; no laser dot; the charge meter isn't
//! drawn (no TF2 HUD yet); the crouched scoped speed isn't its own.

use bevy::prelude::*;
use serde::Deserialize;

use chimera_sim::weapon::{AltModes, DamageEffect, NetPart};

use super::damage::HP;
use crate::core::{BodyRegion, Hitgroup, SimClock};

/// `tf2:sniper`'s parameters (points, points/s, seconds).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SniperParams {
    pub base: f32,
    pub charge_max: f32,
    pub charge_rate: f32,
    pub charge_delay: f32,
    pub headshot_delay: f32,
    pub headshot: f32,
    #[serde(default)]
    pub headshot_icon: Option<String>,
}

impl SniperParams {
    /// The charge after `tau` seconds scoped (points).
    pub fn charge(&self, tau: f32) -> f32 {
        (self.charge_rate * (tau - self.charge_delay)).clamp(0.0, self.charge_max)
    }

    /// A body hit's damage: scoped for `tau` seconds (None: unscoped).
    pub fn damage(&self, tau: Option<f32>) -> f32 {
        match tau {
            Some(t) => self.base.max(self.charge(t)),
            None => self.base,
        }
    }

    /// The head's multiplier.
    pub fn head(&self, tau: Option<f32>) -> f32 {
        if tau.is_some_and(|t| t + 1e-4 >= self.headshot_delay) { self.headshot } else { 1.0 }
    }
}

/// The rifle's scope time (a predicted part).
#[derive(Component, Clone, Debug)]
pub struct Tf2Sniper {
    pub params: SniperParams,
    pub scoped_since: Option<f64>,
}

impl NetPart for Tf2Sniper {
    type Net = Option<f64>;
    fn net(&self) -> Self::Net {
        self.scoped_since
    }
    fn set_net(&mut self, net: Self::Net) {
        self.scoped_since = net;
    }
}

/// `tf2:sniper` on a weapon.
pub fn attach(e: &mut EntityWorldMut, p: &SniperParams) {
    e.insert(Tf2Sniper { params: p.clone(), scoped_since: None });
}

/// Before the frame: the scope time, the shot's damage and the head's
/// multiplier.
pub fn charge(mut guns: Query<(&mut Tf2Sniper, Option<&AltModes>, &mut DamageEffect)>, clock: Res<SimClock>) {
    let now = clock.now;
    for (mut s, modes, mut d) in &mut guns {
        let scoped = modes.is_some_and(|m| m.current > 0);
        match (scoped, s.scoped_since) {
            (true, None) => s.scoped_since = Some(now),
            (false, Some(_)) => s.scoped_since = None,
            _ => {}
        }
        let tau = s.scoped_since.map(|t| (now - t) as f32);
        let amount = s.params.damage(tau) * HP;
        if d.amount != amount {
            d.amount = amount;
        }
        let head = s.params.head(tau);
        for r in BodyRegion::ALL {
            if r.hitgroup() == Hitgroup::Head && d.regions.get(r) != head {
                d.regions.set(r, head);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> SniperParams {
        super::super::hook_params(super::super::SNIPER_RIFLE, super::super::SNIPER_HOOK)
    }

    /// Tests 10-15: 50 unscoped or uncharged; 50 / 100 / 150 / 150 body at
    /// 1.3 / 2.3 / 3.3 / 5 s scoped; heads x3 after 0.2 s.
    #[test]
    fn the_charge_and_the_headshot_10_to_15() {
        let p = params();
        assert_eq!(p.damage(None), 50.0);
        assert_eq!(p.head(None), 1.0);
        assert_eq!(p.damage(Some(0.1)), 50.0);
        assert_eq!(p.head(Some(0.1)), 1.0);
        assert_eq!(p.head(Some(0.25)), 3.0);
        for (t, want) in [(1.3, 50.0), (2.3, 100.0), (3.3, 150.0), (5.0, 150.0)] {
            assert!((p.damage(Some(t)) - want).abs() < 1e-3, "{t}: {}", p.damage(Some(t)));
        }
        assert!((p.damage(Some(3.3)) * p.head(Some(3.3)) - 450.0).abs() < 1e-3);
        assert!((p.damage(Some(2.3)) * p.head(Some(2.3)) - 300.0).abs() < 1e-3);
    }
}
