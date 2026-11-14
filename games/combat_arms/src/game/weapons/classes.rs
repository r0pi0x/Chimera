//! What differs between Combat Arms' weapon classes beyond their numbers
//! (specs/combat_arms/weapon_classes.md; sections below are its): the few
//! rules the data alone doesn't give, on top of `weapons` (firing) and
//! `zoom` (scopes). A class is mostly data (data/weapons/*.toml): pellets
//! (`hitscan.pellets`, each its own square draw of the shot's cone, 5.1),
//! semi-auto and pump or bolt cycles (`trigger`), shell-by-shell reloads
//! (`magazine.shells`, Chimera's `ShellReload` with a closing part and no
//! tick between rounds, 5.5), the fixed unscoped sniper cone
//! (`MinPerturb` = `MaxPerturb`), shotguns' zero movement targets.
//!
//! Rules here (`class_rules`, before the weapon frame, after the hands):
//!
//! - **Trigger released at clip 0** (`weapon_firing.md`, "Trigger"): the
//!   per-press counter clears while the clip is empty, so holding fire on
//!   an empty pump gun fires each shell as it goes in (5.5, case 26).
//! - **Fire stops a shell reload at once** (5.5): while one runs the
//!   weapon may fire now (Chimera's frame then fires and stops it when a
//!   round is in; its own start would hold fire for the start).
//! - **Minigun spin** (3.3, the `combat_arms:spin` hook, `CaSpin`): fire
//!   held in the firing state spins up for the pre-fire (`prefire`, from
//!   the update after the press's) and fires on the update after the one
//!   that sees it end (Corrections 3); released,
//!   the loop (`loop_len`) finishes and the post-fire spins
//!   down. With `reversible` (`AccumPreFire`) a release mid-spin-up starts
//!   the spin-down part-way and a press mid-spin-down the spin-up (the
//!   whole-ms formulas of 3.3); otherwise each restarts from 0.
//! - **Alt-fire volleys** (5.8, the `combat_arms:alt_fire` hook,
//!   `CaAltFire`): attack2's press fires `rounds` rounds `interval` apart
//!   (fewer when the clip has fewer), the weapon busy for `cycle`. The
//!   rounds go through Chimera's burst (`Burst` in a mode no attack2 mode
//!   reaches: its rounds-on-their-own and interval). `CaAltVolley` marks
//!   the volley's span, so its rounds are known for the alternate fire's
//!   (a crossbow's alternate bolt: `special::bolts`).
//! - **Pellet hit carry-over** (5.7, Q1; `PelletCarryOver`, on by default
//!   as `AllowMultipleVectorsPerRoundBug` 1): once a pellet of a shot hits
//!   a character, each later pellet that hits none is a hit on that one too,
//!   at the most recent real hit's node and direction (`CarryOver`; the
//!   delivery in `weapons`).
//!
//! Known shortcuts:
//! - The minigun's loop fires by the trigger's cycle (34 ms: five rounds a
//!   170 ms loop, Open question 3's lower count), not by its fire keys.
//! - The double barrel's `reload_1` (one round left) isn't separate: one
//!   reload time. An alt-fire volley needs a fresh press.
//! - No continuous-fire mode (`ContinuousFire`); the post-fire is the
//!   trigger's (`Trigger::postfire`: an install copy's from its view
//!   model, `timing`; ours have none, the M92FS's 150 ms included); the
//!   pre-fire is the trigger's latency
//!   (`Trigger::latency`: 3 updates from the press, 2 on primary shotguns
//!   and pistols, Corrections 3 and Corrections (2026-10-30) 1; a copy's
//!   longer pre-fire its whole length, `timing::prefire_latency`; the
//!   minigun's 1 with its spin-up), none on an alt-fire
//!   volley; no launchers, M32 or snowball gun (projectiles: their own
//!   specs; the crossbow's alternate fire is a one-round volley here, its
//!   bolt `special::bolts`').
//! - 5.6 (the server's reload bookkeeping): its window drops shots
//!   (`weapons::server_reload_mark`, Corrections 2), but there is one clip,
//!   so nothing refills at its end and a shotgun's shot is let through
//!   whenever its clip has a round.
//! - Ammo pickups by class and the "last primary" record (inventory and
//!   game modes) aren't here.

use bevy::prelude::*;
use serde::Deserialize;

use crate::{
    core::{Health, Intent, SimClock},
    map::interp::NetDrawn,
    weapon::{Burst, Inventory, Magazine, NetPart, ShellStage, WeaponState},
};

/// Hook names.
pub const SPIN: &str = "combat_arms:spin";
pub const ALT_FIRE: &str = "combat_arms:alt_fire";

/// Timers due on a tick count on it (as the weapon frame does).
const SLACK: f64 = 1e-5;

/// Not this update but the next (a tick is longer).
const NEXT_UPDATE: f64 = 1e-3;

/// Whole milliseconds of `secs` (3.3's times).

fn ms(secs: f64) -> f64 {
    (secs * 1000.0 + 1e-6).floor()
}

/// `combat_arms:spin`: a minigun's spin (3.3), seconds. An install
/// copy's table (written by the merge where `AccumPreFire` is set) has
/// only `reversible`: its lengths come from its view model
/// (`timing`; 0 where the model doesn't read).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SpinParams {
    /// The pre-fire animation: the spin-up.
    #[serde(default)]
    pub prefire: f32,
    /// The post-fire animation: the spin-down.
    #[serde(default)]
    pub postfire: f32,
    /// The fire loop's length (`fire_0`).
    #[serde(default)]
    pub loop_len: f32,
    /// `AccumPreFire` non-zero: spin-up and spin-down are reversible.
    #[serde(default)]
    pub reversible: bool,
}

/// Where a minigun's spin is.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SpinState {
    #[default]
    Idle,
    /// Spinning up; its time 0 was `start`.
    Up { start: f64 },
    /// Firing since `since` (loops from there).
    Firing { since: f64 },
    /// Spinning down; its time 0 is `start` (later than now while the last
    /// loop finishes).
    Down { start: f64 },
}

/// A minigun's spin (`combat_arms:spin`).
#[derive(Component, Clone, Debug)]
pub struct CaSpin {
    pub params: SpinParams,
    pub state: SpinState,
}

impl CaSpin {
    /// One update at `now` with fire `held` (and allowed): returns when the
    /// first round may fire while spinning up (the frame waits for it).
    pub fn update(&mut self, held: bool, now: f64) -> Option<f64> {
        let p = self.params;
        let (pre, post, len) = (p.prefire as f64, p.postfire as f64, p.loop_len as f64);
        self.state = match self.state {
            SpinState::Idle if held => SpinState::Up { start: now },
            SpinState::Up { start } if !held => {
                let t = if p.reversible {
                    (post * 1000.0 * (1.0 - ms(now - start) / (pre * 1000.0))).floor().max(0.0) / 1000.0
                } else {
                    0.0
                };
                SpinState::Down { start: now - t }
            }
            SpinState::Up { start } if now + SLACK >= start + pre => SpinState::Firing { since: start + pre },
            SpinState::Firing { since } if !held => {
                let loops = (((now - since) / len.max(1e-6)) - 1e-9).ceil().max(1.0);
                SpinState::Down {
                    start: since + loops * len,
                }
            }
            SpinState::Down { start } if held => {
                let u = ms((now - start).max(0.0));
                let t = if p.reversible {
                    (pre * 1000.0 * (1.0 - u / (post * 1000.0))).floor().max(0.0) / 1000.0
                } else {
                    0.0
                };
                SpinState::Up { start: now - t }
            }
            SpinState::Down { start } if now + SLACK >= start + post => SpinState::Idle,
            s => s,
        };
        match self.state {
            SpinState::Up { start } => Some(start + pre),
            _ => None,
        }
    }

    /// When the spin-down ends (none unless spinning down).
    pub fn idle_at(&self) -> Option<f64> {
        match self.state {
            SpinState::Down { start } => Some(start + self.params.postfire as f64),
            _ => None,
        }
    }
}

impl NetPart for CaSpin {
    type Net = SpinState;
    fn net(&self) -> SpinState {
        self.state
    }
    fn set_net(&mut self, state: SpinState) {
        self.state = state;
    }
}

/// `combat_arms:alt_fire`: attack2 fires a volley (5.8), seconds.
#[derive(Component, Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CaAltFire {
    /// Rounds a volley (the alt-fire animation's fire keys).
    pub rounds: u32,
    /// Between them.
    pub interval: f32,
    /// The alt-fire animation's length: busy until then.
    pub cycle: f32,
}

/// When the last alt-fire volley of a weapon (`CaAltFire`) fires its
/// rounds: from its press to its last round (a tick's slack), so what
/// reads the weapon's `Fired` rounds on that tick knows them for the
/// alternate fire's (a crossbow's alternate bolt, 7 of
/// special_weapons.md).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct CaAltVolley {
    pub span: Option<(f64, f64)>,
}

impl CaAltVolley {
    /// Whether a round fired at `now` is the volley's.
    pub fn at(&self, now: f64) -> bool {
        self.span.is_some_and(|(from, to)| now + SLACK >= from && now <= to + SLACK)
    }
}

impl CaAltFire {
    /// Chimera's burst that fires the volley's rounds (a mode attack2 never
    /// steps to: only `class_rules` starts it).
    pub fn burst(&self) -> Burst {
        Burst {
            mode: u8::MAX,
            count: self.rounds,
            interval: self.interval,
            refire: self.cycle,
            from_last: false,
            once_per_press: true,
        }
    }
}

/// Pellet hit carry-over (5.7): `AllowMultipleVectorsPerRoundBug`, on by
/// default as in the game.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PelletCarryOver(pub bool);

impl Default for PelletCarryOver {
    fn default() -> Self {
        Self(true)
    }
}

/// The client's "hit a player" record over one shot's pellets (5.7).
#[derive(Clone, Copy, Debug)]
pub struct CarryOver<T> {
    pub on: bool,
    last: Option<T>,
}

impl<T: Copy> CarryOver<T> {
    pub fn new(on: bool) -> Self {
        Self { on, last: None }
    }

    /// The next pellet hit `real` (none: no character): the hit it
    /// reports besides its own, the most recent real one when it has none.
    pub fn pellet(&mut self, real: Option<T>) -> Option<T> {
        match real {
            Some(h) => {
                self.last = Some(h);
                None
            }
            None if self.on => self.last,
            None => None,
        }
    }
}

/// Each pellet's reported hit, in order: its own, or the carried one.
pub fn reported_hits<T: Copy>(real: &[Option<T>], on: bool) -> Vec<Option<T>> {
    let mut c = CarryOver::new(on);
    real.iter().map(|r| c.pellet(*r).or(*r)).collect()
}

/// The class rules before the weapon frame (the module doc).
#[allow(clippy::type_complexity)]
pub(super) fn class_rules(
    owners: Query<(&Intent, &Inventory, Option<&Health>), Without<NetDrawn>>,
    mut weapons: Query<(
        &mut WeaponState,
        Option<&Magazine>,
        Option<&mut CaSpin>,
        Option<(&CaAltFire, &mut CaAltVolley)>,
        Has<super::CaDamage>,
        Has<crate::game::special::CaProjectileData>,
        Has<crate::game::special::CaBoltData>,
    )>,
    clock: Res<SimClock>,
) {
    let now = clock.now;
    let due = now + SLACK;
    for (intent, inv, health) in &owners {
        let alive = health.is_none_or(|h| h.current > 0.0);
        for &w in &inv.weapons {
            // A weapon put away stops spinning.
            if Some(w) != inv.active || !alive {
                if let Ok((_, _, Some(mut spin), ..)) = weapons.get_mut(w)
                    && spin.state != SpinState::Idle
                {
                    spin.state = SpinState::Idle;
                }
                continue;
            }
            let Ok((mut st, mag, spin, alt, ours, projectile, bolt)) = weapons.get_mut(w) else { continue };
            if !ours && !projectile && !bolt {
                continue;
            }
            let clip = mag.map_or(0, |m| m.clip);
            // The trigger counts as released at clip 0.
            if mag.is_some() && clip == 0 && st.burst != 0 {
                st.burst = 0;
            }
            // Fire stops a shell reload at once.
            if st.shells != ShellStage::Idle && st.next_primary != now {
                st.next_primary = now;
            }
            let ready = due >= inv.next_attack && !st.reloading();
            if let Some(mut spin) = spin {
                // The spin-up is the pre-fire (Corrections 3): it starts
                // on the update after the one that entered the firing
                // state (`Trigger::latency` 1), and `fire_0` starts on the
                // update that sees it done, the round on the next.
                let held = intent.fire && clip > 0 && ready && st.firing;
                let was = spin.state;
                if let Some(first) = spin.update(held, now) {
                    st.next_primary = first;
                }
                match (was, spin.state) {
                    (SpinState::Up { .. }, SpinState::Firing { .. }) => st.next_primary = now + NEXT_UPDATE,
                    (_, SpinState::Idle | SpinState::Down { .. }) if st.firing || st.armed_round => {
                        // Let go during the spin-up: no round.
                        st.firing = false;
                        st.arming = 0;
                        st.armed_round = false;
                    }
                    _ => {}
                }
            }
            if let Some((alt, mut volley)) = alt
                && intent.secondary
                && !inv.prev_secondary()
                && ready
                && clip > 0
                && st.burst_left == 0
                && st.next_primary <= due
            {
                st.burst_left = alt.rounds.min(clip);
                st.next_burst_round = now;
                st.next_primary = now + alt.cycle as f64;
                st.next_secondary = st.next_primary;
                let last = now + (alt.interval * alt.rounds.min(clip).saturating_sub(1) as f32) as f64;
                volley.span = Some((now, last));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carry_over_reports_the_last_real_hit() {
        let p3 = [None, None, Some(3), None, None, None, None, None];
        let on = reported_hits(&p3, true);
        assert_eq!(on.iter().filter(|h| h.is_some()).count(), 6);
        assert_eq!(reported_hits(&p3, false).iter().filter(|h| h.is_some()).count(), 1);
    }

    #[test]
    fn spin_reverses_part_way() {
        let mut s = CaSpin {
            params: SpinParams {
                prefire: 1.3,
                postfire: 1.333,
                loop_len: 0.17,
                reversible: true,
            },
            state: SpinState::Idle,
        };
        assert!((s.update(true, 0.0).unwrap() - 1.3).abs() < 1e-6);
        s.update(false, 0.65);
        assert!((s.idle_at().unwrap() - 1.317).abs() < 1e-6);
        // 300 ms into that spin-down (966 ms of it): up from 357 ms.
        let first = s.update(true, 0.95).unwrap();
        assert!((first - (0.95 + 1.3 - 0.357)).abs() < 1e-6, "{first}");
    }
}
