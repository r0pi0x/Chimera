//! The Sniper Rifle's scope, charge and headshot (specs/tf2/weapons_more.md
//! 3 and its corrections from the SDK), the hook `tf2:sniper`
//! (`SniperParams`, the weapon part `Tf2Sniper`), each tick in the
//! predicted `Predict::Select` before the movement and the frame (`scope`).
//! The scope's look is Chimera's (`[alt] kind = "scope"`: field of view 20,
//! the holder at 80 units/s, the view model hidden, attack2 toggling it at
//! most every 0.3 s); its timing is TF2's here (the mode set directly,
//! with `WeaponEventKind::ModeChanged`):
//!
//! - **After a scoped shot** the scope stays up `drop_after` (0.5 s: no
//!   jump meanwhile), then drops; with `autorezoom` (`cl_autorezoom` 1)
//!   it comes back `rezoom_after` (1.4 s) after the shot, unless attack2
//!   was pressed meanwhile or that was the last round. **After an unscoped
//!   shot** it can't be raised for `unscoped_block` (1.6667 s, the fire
//!   animation). Nor with no ammo, nor in a jump (from the press until
//!   landing); a jump drops it.
//! - **Firing** waits `toggle_block` (0.1 s) after any zoom change.
//! - **The charge** c is stored: while scoped, not in the `drop_after`
//!   after a scoped shot and not within `charge_delay` (0.3 s) of raising
//!   it with attack2 (an auto re-zoom has no wait), c rises by
//!   `charge_rate` (50) a second to `charge_max` (150); unscoping or a
//!   shot sets it to 0. A shot deals max(`base`, c) scoped, `base` (50)
//!   unscoped; no distance modifier (`tf2:gun`'s `flat`).
//! - **A head hit** is x`headshot` (3, a crit) only while scoped and
//!   `headshot_delay` (0.2 s) or more after the last zoom change (the
//!   re-zoom restarts it); otherwise the head counts as the body.
//!
//! Known shortcuts: the headshot kill's icon (`headshot`) isn't told apart
//! from the rifle's in the kill feed; no laser dot; the charge meter isn't
//! drawn; the scope's zoom change is seen a tick after the frame made it
//! (timed from that frame's tick); the rifle can be put away right after
//! a scoped shot; a 0.3 s attack2 block after a cancelled re-zoom isn't
//! kept.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use chimera_sim::weapon::{AltModes, DamageEffect, Inventory, Magazine, NetPart, Weapon, WeaponEvent, WeaponEventKind, WeaponState};

use super::damage::{HP, Tf2Gun};
use crate::core::{BodyRegion, FirstTimePredicted, Hitgroup, Intent, MovementState, SimClock};

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
    pub drop_after: f32,
    pub rezoom_after: f32,
    pub toggle_block: f32,
    pub unscoped_block: f32,
    #[serde(default = "yes")]
    pub autorezoom: bool,
    #[serde(default)]
    pub headshot_icon: Option<String>,
}

fn yes() -> bool {
    true
}

impl SniperParams {
    /// The charge after `tau` seconds scoped from a raise by attack2
    /// (points; the stored charge's course).
    pub fn charge(&self, tau: f32) -> f32 {
        (self.charge_rate * (tau - self.charge_delay)).clamp(0.0, self.charge_max)
    }

    /// A body hit's damage with charge `c` scoped (None: unscoped).
    pub fn damage(&self, c: Option<f32>) -> f32 {
        match c {
            Some(c) => self.base.max(c),
            None => self.base,
        }
    }

    /// The head's multiplier, `since` seconds after the last zoom change
    /// (None: unscoped).
    pub fn head(&self, since: Option<f32>) -> f32 {
        if since.is_some_and(|t| t + 1e-4 >= self.headshot_delay) { self.headshot } else { 1.0 }
    }
}

/// The rifle's scope state (predicted).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ScopeState {
    /// The mode last seen (0 unscoped).
    pub mode: u8,
    /// The last zoom change (in or out).
    pub changed: f64,
    /// Raised by attack2 at (the charge's wait); None after a re-zoom.
    pub raised: Option<f64>,
    pub charge: f32,
    /// A scoped shot's time, while its drop or re-zoom is pending, and
    /// whether it dropped yet.
    pub shot: Option<f64>,
    pub dropped: bool,
    /// Raising blocked until (after an unscoped shot).
    pub blocked: f64,
    /// The last shot seen.
    pub seen: f64,
    /// In a jump (pressed on the ground, until landing), and off the
    /// ground since.
    pub jumping: bool,
    pub left: bool,
}

/// The rifle's part.
#[derive(Component, Clone, Debug)]
pub struct Tf2Sniper {
    pub params: SniperParams,
    pub state: ScopeState,
}

impl NetPart for Tf2Sniper {
    type Net = ScopeState;
    fn net(&self) -> Self::Net {
        self.state
    }
    fn set_net(&mut self, net: Self::Net) {
        self.state = net;
    }
}

/// `tf2:sniper` on a weapon.
pub fn attach(e: &mut EntityWorldMut, p: &SniperParams) {
    e.insert(Tf2Sniper { params: p.clone(), state: ScopeState { changed: f64::MIN, seen: f64::MIN, blocked: f64::MIN, ..default() } });
}

/// Each tick before the movement and the frame: the scope's timing, the
/// charge, the shot's damage and the head's multiplier (module doc).
#[allow(clippy::type_complexity)]
pub fn scope(
    mut owners: Query<(Entity, &mut Intent, &Inventory, Option<&MovementState>)>,
    mut guns: Query<(&mut Tf2Sniper, &mut AltModes, &mut DamageEffect, &mut WeaponState, Option<&Tf2Gun>, Option<&Magazine>), With<Weapon>>,
    clock: Res<SimClock>,
    first: Res<FirstTimePredicted>,
    mut events: MessageWriter<WeaponEvent>,
) {
    let (now, dt) = (clock.now, clock.dt() as f64);
    for (owner, mut intent, inv, state) in &mut owners {
        let Some(w) = inv.active else { continue };
        let Ok((mut gun, mut modes, mut dmg, mut st, tf2, mag)) = guns.get_mut(w) else { continue };
        let p = gun.params.clone();
        let mut s = gun.state;
        let ammo = mag.is_none_or(|m| m.clip > 0);
        let on_ground = state.is_none_or(|m| m.on_ground);
        let mut set = |modes: &mut AltModes, s: &mut ScopeState, mode: u8| {
            if modes.current != mode {
                modes.current = mode;
                events.write(WeaponEvent { owner, weapon: w, kind: WeaponEventKind::ModeChanged { mode }, replay: !first.0 });
            }
            s.mode = mode;
            s.changed = now;
            if mode == 0 {
                s.charge = 0.0;
            }
        };
        // The player's own toggle (attack2, last tick's frame).
        if modes.current != s.mode {
            let at = now - dt;
            if modes.current > 0 && (s.jumping || now < s.blocked || !ammo) {
                set(&mut modes, &mut s, 0);
            } else {
                s.mode = modes.current;
                s.changed = at;
                s.shot = None;
                s.raised = (s.mode > 0).then_some(at);
                if s.mode == 0 {
                    s.charge = 0.0;
                }
            }
        }
        // A shot last tick.
        if let Some(t) = tf2.map(|g| g.last_shot).filter(|t| *t > s.seen) {
            s.seen = t;
            s.charge = 0.0;
            if s.mode > 0 {
                s.shot = Some(t);
                s.dropped = false;
            } else {
                s.blocked = t + p.unscoped_block as f64;
            }
        }
        // The jump: no jump just after a scoped shot; one drops the scope.
        if let Some(t) = s.shot
            && !s.dropped
            && now <= t + p.drop_after as f64
        {
            intent.jump = false;
        }
        if intent.jump && on_ground && !s.jumping {
            s.jumping = true;
            s.left = false;
            if s.mode > 0 {
                set(&mut modes, &mut s, 0);
                s.shot = None;
            }
        } else if s.jumping {
            if !on_ground {
                s.left = true;
            } else if s.left {
                s.jumping = false;
            }
        }
        // The drop and the re-zoom after a scoped shot.
        if let Some(t) = s.shot {
            if !s.dropped && now > t + p.drop_after as f64 {
                set(&mut modes, &mut s, 0);
                s.dropped = true;
                if !p.autorezoom || !ammo {
                    s.shot = None;
                }
            } else if s.dropped && now > t + p.rezoom_after as f64 {
                s.shot = None;
                if !s.jumping {
                    set(&mut modes, &mut s, 1);
                    s.raised = None;
                }
            }
        }
        // The charge.
        let after_shot = s.shot.is_some_and(|t| !s.dropped && now <= t + p.drop_after as f64);
        let waiting = s.raised.is_some_and(|r| now - r < p.charge_delay as f64 - 1e-6);
        if s.mode > 0 && !after_shot && !waiting {
            s.charge = (s.charge + p.charge_rate * dt as f32).min(p.charge_max);
        } else if s.mode == 0 {
            s.charge = 0.0;
        }
        // Firing waits after a zoom change.
        let after_change = s.changed + p.toggle_block as f64;
        if st.next_primary < after_change {
            st.next_primary = after_change;
        }
        // The shot's damage and the head.
        let scoped = s.mode > 0;
        let amount = p.damage(scoped.then_some(s.charge)) * HP;
        if dmg.amount != amount {
            dmg.amount = amount;
        }
        let head = p.head(scoped.then_some((now - s.changed) as f32));
        for r in BodyRegion::ALL {
            if r.hitgroup() == Hitgroup::Head && dmg.regions.get(r) != head {
                dmg.regions.set(r, head);
            }
        }
        if gun.state != s {
            gun.state = s;
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
        let d = |t: f32| p.damage(Some(p.charge(t)));
        assert_eq!(p.damage(None), 50.0);
        assert_eq!(p.head(None), 1.0);
        assert_eq!(d(0.1), 50.0);
        assert_eq!(p.head(Some(0.1)), 1.0);
        assert_eq!(p.head(Some(0.25)), 3.0);
        for (t, want) in [(1.3, 50.0), (2.3, 100.0), (3.3, 150.0), (5.0, 150.0)] {
            assert!((d(t) - want).abs() < 1e-3, "{t}: {}", d(t));
        }
        assert!((d(3.3) * p.head(Some(3.3)) - 450.0).abs() < 1e-3);
        assert!((d(2.3) * p.head(Some(2.3)) - 300.0).abs() < 1e-3);
    }
}
