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
//! - **What others see**: the charge as a fraction of `charge_max` in
//!   the weapon's `HeldGauge` (sent with what the Sniper holds; the
//!   laser dot's core, `dot`).
//! - **A head hit** is x`headshot` (3, a crit) only while scoped and
//!   `headshot_delay` (0.2 s) or more after the last zoom change (the
//!   re-zoom restarts it); otherwise the head counts as the body.
//!
//! **The variants** (specs/tf2/batch2b_mechanics.md 15, `items::docs`
//! from the item's attributes): the charge rate × `charge_scale` (the
//! Sydney Sleeper's 1.25) × the Bazaar Bargain's heads
//! (`items::meters::decap_factor`); a full charge's damage ×
//! `full_bonus` (the Machina's 1.15), which with `pierce_full` goes on
//! through every player behind (`pierce`, the server's) and fires only
//! scoped (`only_scoped`); the Sydney Sleeper's head no crit (its
//! mini-crit and jarate are `items::effects`'); the Classic (`classic`)
//! charges while attack is held, scoped or not, and fires on release, its
//! body × `body_scale` (0.9), its head a crit only at full charge.
//!
//! Known shortcuts: the Machina's pierce isn't rewound (lag compensation)
//! past the first victim and hits each behind as the body; the headshot kill's icon (`headshot`) isn't told apart
//! from the rifle's in the kill feed; the charge meter isn't
//! drawn; the scope's zoom change is seen a tick after the frame made it
//! (timed from that frame's tick); the rifle can be put away right after
//! a scoped shot; a 0.3 s attack2 block after a cancelled re-zoom isn't
//! kept.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use chimera_sim::weapon::{AltModes, DamageEffect, HeldGauge, Inventory, Magazine, NetPart, Weapon, WeaponEvent, WeaponEventKind, WeaponState};

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
    /// The variants (specs/tf2/batch2b_mechanics.md 15; `items::docs`
    /// from the item's attributes): the charge rate's factor (the Sydney
    /// Sleeper's `mult_sniper_charge_per_sec`), a full charge's damage
    /// factor (the Machina's `sniper_full_charge_damage_bonus`), a full
    /// charge passing through players (`sniper_penetrate_players_when_
    /// charged`), firing only scoped (`sniper_only_fire_zoomed`).
    #[serde(default = "one")]
    pub charge_scale: f32,
    #[serde(default = "one")]
    pub full_bonus: f32,
    #[serde(default)]
    pub pierce_full: bool,
    #[serde(default)]
    pub only_scoped: bool,
    /// The Classic (its class): charging while attack is held, scoped or
    /// not, firing on release; its body hits × `body_scale`
    /// (`bodyshot_damage_modify`), its head a crit only at full charge.
    #[serde(default)]
    pub classic: bool,
    #[serde(default = "one")]
    pub body_scale: f32,
    /// The Sydney Sleeper: no headshot crit, a headshot mini-crits
    /// (`items::effects`), every hit jarates for max(2, `jarate` × charge
    /// / 150) s (`jarate_duration`).
    #[serde(default)]
    pub head_mini: bool,
    #[serde(default)]
    pub jarate: f32,
}

fn yes() -> bool {
    true
}

fn one() -> f32 {
    1.0
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
    /// The Classic's attack held, charging.
    pub held: bool,
}

/// The rifle's part.
#[derive(Component, Clone, Debug)]
pub struct Tf2Sniper {
    pub params: SniperParams,
    pub state: ScopeState,
}

impl Tf2Sniper {
    /// Whether a head hit now counts as a headshot (scoped long enough).
    pub fn headshot_ready(&self, now: f64) -> bool {
        self.state.mode > 0 && (now - self.state.changed) as f32 + 1e-4 >= self.params.headshot_delay
    }

    /// Whether the shot now is fully charged.
    pub fn full(&self) -> bool {
        self.state.charge >= self.params.charge_max - 1e-3
    }
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
    e.insert((
        Tf2Sniper { params: p.clone(), state: ScopeState { changed: f64::MIN, seen: f64::MIN, blocked: f64::MIN, ..default() } },
        HeldGauge::default(),
    ));
}

/// Each tick before the movement and the frame: the scope's timing, the
/// charge, the shot's damage and the head's multiplier (module doc). Not
/// for others drawn from snapshots: their scope and charge are the
/// server's (`NetHeld`).
#[allow(clippy::type_complexity)]
pub fn scope(
    mut owners: Query<(Entity, &mut Intent, &Inventory, Option<&MovementState>), Without<crate::map::interp::NetDrawn>>,
    mut guns: Query<
        (
            &mut Tf2Sniper,
            &mut AltModes,
            &mut DamageEffect,
            &mut WeaponState,
            Option<&Tf2Gun>,
            Option<&Magazine>,
            Option<&mut HeldGauge>,
            Option<&crate::game::items::meters::Tf2Meter>,
        ),
        With<Weapon>,
    >,
    clock: Res<SimClock>,
    first: Res<FirstTimePredicted>,
    mut events: MessageWriter<WeaponEvent>,
) {
    let (now, dt) = (clock.now, clock.dt() as f64);
    for (owner, mut intent, inv, state) in &mut owners {
        let Some(w) = inv.active else { continue };
        let Ok((mut gun, mut modes, mut dmg, mut st, tf2, mag, gauge, meter)) = guns.get_mut(w) else { continue };
        let p = gun.params.clone();
        // The charge's rate: the Sydney Sleeper's factor, the Bazaar
        // Bargain's heads (`items::meters::decap_factor`).
        let rate = p.charge_rate
            * p.charge_scale
            * meter
                .filter(|m| m.params.kind == crate::game::items::meters::MeterKind::Heads)
                .map_or(1.0, |m| crate::game::items::meters::decap_factor(m.heads()));
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
        if p.classic {
            // Held, it charges (scoped or not); released, it fires.
            let ready = inv.ready(now) && now + 1e-6 >= st.next_primary && ammo;
            if intent.fire && ready {
                s.held = true;
                s.charge = (s.charge + rate * dt as f32).min(p.charge_max);
                intent.fire = false;
            } else if s.held && !intent.fire {
                s.held = false;
                intent.fire = true;
            } else if !intent.fire {
                s.held = false;
            }
        } else if s.mode > 0 && !after_shot && !waiting {
            s.charge = (s.charge + rate * dt as f32).min(p.charge_max);
        } else if s.mode == 0 {
            s.charge = 0.0;
        }
        // The Machina fires only scoped.
        if p.only_scoped && s.mode == 0 {
            intent.fire = false;
        }
        // Firing waits after a zoom change.
        let after_change = s.changed + p.toggle_block as f64;
        if st.next_primary < after_change {
            st.next_primary = after_change;
        }
        // The shot's damage and the head.
        let scoped = s.mode > 0;
        let full = s.charge >= p.charge_max - 1e-3;
        let (amount, head, body) = if p.classic {
            // Body max(50, c) x 0.9; the head a crit only at full charge.
            (p.base.max(s.charge), if full { p.headshot } else { 1.0 }, p.body_scale)
        } else {
            let bonus = if scoped && full { p.full_bonus } else { 1.0 };
            let head = if p.head_mini { 1.0 } else { p.head(scoped.then_some((now - s.changed) as f32)) };
            (p.damage(scoped.then_some(s.charge)) * bonus, head, 1.0)
        };
        let amount = amount * HP;
        if dmg.amount != amount {
            dmg.amount = amount;
        }
        for r in BodyRegion::ALL {
            let want = if r.hitgroup() == Hitgroup::Head { head } else { body };
            if dmg.regions.get(r) != want {
                dmg.regions.set(r, want);
            }
        }
        // The charge others see (the laser dot's core).
        if let Some(mut g) = gauge {
            g.set_if_neq(HeldGauge(s.charge / p.charge_max.max(1e-6)));
        }
        if gun.state != s {
            gun.state = s;
        }
    }
}

/// The Machina's full-charge shot goes on through players (batch2b 15):
/// every enemy behind the one it hit, until the world, takes the same
/// body hit (the server's, after the frame; not rewound).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn pierce(
    mut events: MessageReader<WeaponEvent>,
    guns: Query<(&Weapon, &Tf2Sniper, &DamageEffect)>,
    chars: Query<(Option<&crate::core::Team>, &crate::core::Health), With<Intent>>,
    colliders: Query<&avian3d::prelude::ColliderOf>,
    spatial: avian3d::prelude::SpatialQuery,
    friendly: Option<Res<crate::core::FriendlyFire>>,
    mut damage: MessageWriter<crate::core::Damage>,
) {
    use avian3d::prelude::*;
    let ff = friendly.is_some_and(|f| f.0 != 0);
    for e in events.read() {
        if e.replay {
            continue;
        }
        let WeaponEventKind::Shot { from, to, hit: Some(first), .. } = e.kind else { continue };
        let Ok((weapon, gun, dmg)) = guns.get(e.weapon) else { continue };
        if !gun.params.pierce_full || !gun.full() || !chars.contains(first) {
            continue;
        }
        let Ok(dir) = Dir3::new(to - from) else { continue };
        let team = |x: Entity| chars.get(x).ok().and_then(|c| c.0.copied()).filter(|t| t.0 != 0);
        let mut passed = vec![e.owner, first];
        let mut at = to;
        let filter = SpatialQueryFilter::default().with_mask(crate::core::SOLID_LAYERS);
        for _ in 0..8 {
            let Some(hit) = spatial.cast_ray_predicate(at, dir, 56754.0 * super::damage::U, true, &filter, &|c| {
                let body = colliders.get(c).map_or(c, |o| o.body);
                !passed.contains(&body)
            }) else {
                break;
            };
            let body = colliders.get(hit.entity).map_or(hit.entity, |o| o.body);
            let point = at + *dir * hit.distance;
            let Ok((_, h)) = chars.get(body) else { break };
            passed.push(body);
            at = point;
            let mate = !ff && team(body).is_some() && team(body) == team(e.owner);
            if h.current <= 0.0 || mate {
                continue;
            }
            damage.write(crate::core::Damage {
                target: body,
                attacker: Some(e.owner),
                amount: dmg.amount,
                point,
                dir: *dir,
                hitgroup: Hitgroup::Generic,
                kind: crate::core::DamageKind::Bullet,
                weapon: Some(weapon.id),
                force: Vec3::ZERO,
            });
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
