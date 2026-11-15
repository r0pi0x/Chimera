//! The Minigun's spin (specs/tf2/weapons.md 7), the hook `tf2:minigun`
//! (`MinigunParams`, the weapon part `Tf2Minigun`), run each tick in the
//! predicted `Predict::Select` before selection (`spin`):
//!
//! - **States** (7.1): idle; winding up for `wind_up` (0.75 s) from a
//!   press of attack or attack2; spun (attack2 held, or attack: firing);
//!   winding down for `wind_down` (1.15 s; `wind_down_alt` 1.4 after a
//!   spin on attack2 alone) once both are let go; a press while winding
//!   down winds up again. The frame fires only when spun: until then the
//!   weapon's next shot waits (`WeaponState::next_primary`), so a held
//!   attack fires 0.75 s after the press and then every 0.105 s.
//! - **The holder** while not idle: at most `spun_speed` (110 units/s;
//!   0 crouched) by the weapon's `max_speed`, no jump (the command's
//!   jump dropped), no switch (`HolsterLocked`).
//! - **The damage ramp** (7.2, the 2017 rule): the shot's damage x
//!   (`ramp_start` + (1 - `ramp_start`) min(t / `ramp_time`, 1)), t the
//!   time spun since the wind-up ended.
//! - **Sounds** (8.3): the wind-up, then a loop by what it does (spun,
//!   firing, dry), the wind-down, each replacing the last on the weapon
//!   channel (`map::PredictedSound`: the shooter's client plays its own);
//!   view-model actions `SPIN_UP` and `SPIN_DOWN` (`WeaponEventKind::
//!   Action`) for its sequences.
//!
//! Known shortcuts: the cone's tightening over the first second isn't
//! modelled (open question 11); a wind-up begun in the air doesn't keep
//! the air speed (the party's movement's own rule).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use chimera_sim::weapon::{
    CHAN_WEAPON, DamageEffect, HolsterLocked, Inventory, Magazine, NetPart, Weapon, WeaponEvent, WeaponEventKind, WeaponState,
};

use super::damage::U;
use crate::{
    core::{FirstTimePredicted, Health, Intent, MovementState, SimClock},
    map::{PlaySound, PredictedSound},
};

/// The view-model actions (`WeaponEventKind::Action`).
pub const SPIN_UP: &str = "spin_up";
pub const SPIN_DOWN: &str = "spin_down";

/// `tf2:minigun`'s parameters (seconds, units/s, sound entries).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MinigunParams {
    pub wind_up: f32,
    pub wind_down: f32,
    pub wind_down_alt: f32,
    pub ramp_time: f32,
    pub ramp_start: f32,
    pub spun_speed: f32,
    pub wind_up_sound: String,
    pub spin_sound: String,
    pub fire_sound: String,
    pub empty_sound: String,
    pub wind_down_sound: String,
}

/// Where the spin is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Spin {
    #[default]
    Idle,
    /// Winding up since `start`.
    Up { start: f64 },
    /// Spun since `since`; `fired`: attack was pressed while spun (the
    /// wind-down's length).
    Spun { since: f64, fired: bool },
    /// Winding down until `until`.
    Down { until: f64 },
}

/// The loop playing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Loop {
    #[default]
    None,
    Spin,
    Fire,
    Empty,
}

/// The Minigun's spin state (a predicted part).
#[derive(Component, Clone, Debug)]
pub struct Tf2Minigun {
    pub params: MinigunParams,
    pub spin: Spin,
    pub playing: Loop,
    /// Its data's damage per bullet (normalized), before the ramp.
    pub base_damage: f32,
}

impl NetPart for Tf2Minigun {
    type Net = (Spin, Loop);
    fn net(&self) -> Self::Net {
        (self.spin, self.playing)
    }
    fn set_net(&mut self, (spin, playing): Self::Net) {
        self.spin = spin;
        self.playing = playing;
    }
}

/// `tf2:minigun` on a weapon.
pub fn attach(e: &mut EntityWorldMut, p: &MinigunParams) {
    let base_damage = e.get::<DamageEffect>().map_or(0.0, |d| d.amount);
    e.insert(Tf2Minigun {
        params: p.clone(),
        spin: Spin::Idle,
        playing: Loop::None,
        base_damage,
    });
}

/// The damage factor `t` seconds after the spin-up ended.
pub fn ramp(p: &MinigunParams, t: f64) -> f32 {
    let x = if p.ramp_time > 0.0 { (t as f32 / p.ramp_time).clamp(0.0, 1.0) } else { 1.0 };
    p.ramp_start + (1.0 - p.ramp_start) * x
}

/// The next state: `held` attack or attack2, `fire` attack, at `now`.
pub fn step(p: &MinigunParams, spin: Spin, held: bool, fire: bool, now: f64) -> Spin {
    const SLACK: f64 = 1e-6;
    match spin {
        Spin::Idle if held => Spin::Up { start: now },
        Spin::Up { .. } if !held => Spin::Down { until: now + p.wind_down as f64 },
        Spin::Up { start } if now + SLACK >= start + p.wind_up as f64 => Spin::Spun {
            since: start + p.wind_up as f64,
            fired: fire,
        },
        Spin::Spun { fired, .. } if !held => Spin::Down {
            until: now + if fired { p.wind_down } else { p.wind_down_alt } as f64,
        },
        Spin::Spun { since, fired } => Spin::Spun { since, fired: fired || fire },
        Spin::Down { .. } if held => Spin::Up { start: now },
        Spin::Down { until } if now + SLACK >= until => Spin::Idle,
        s => s,
    }
}

fn sound(entry: &str, owner: Entity, at: Vec3) -> PlaySound {
    PlaySound {
        entry: entry.to_string(),
        at: Some(at),
        volume: None,
        pitch: None,
        source: Some(owner),
        channel: Some(CHAN_WEAPON),
    }
}

/// Each minigun's spin this tick (module doc).
#[allow(clippy::type_complexity)]
pub fn spin(
    mut owners: Query<(Entity, &mut Intent, &Inventory, &Transform, Option<&Health>, Option<&MovementState>)>,
    mut guns: Query<(Entity, &mut Tf2Minigun, &mut Weapon, &mut WeaponState, &mut DamageEffect, Option<&Magazine>, Has<HolsterLocked>)>,
    clock: Res<SimClock>,
    first: Res<FirstTimePredicted>,
    mut sounds: MessageWriter<PredictedSound>,
    mut events: MessageWriter<WeaponEvent>,
    mut commands: Commands,
) {
    let now = clock.now;
    for (owner, mut intent, inv, at, health, state) in &mut owners {
        let alive = health.is_none_or(|h| h.current > 0.0);
        for &w in &inv.weapons {
            let Ok((we, mut gun, mut weapon, mut st, mut dmg, mag, locked)) = guns.get_mut(w) else { continue };
            let active = inv.active == Some(w) && alive;
            let fire = active && intent.fire;
            let held = active && (intent.fire || intent.secondary);
            let was = gun.spin;
            let next = if active { step(&gun.params, was, held, fire, now) } else { Spin::Idle };
            let p = gun.params.clone();
            let mut play = |entry: &str| {
                if first.0 {
                    sounds.write(PredictedSound { by: owner, sound: sound(entry, owner, at.translation) });
                }
            };
            // Sounds and view-model actions on the changes.
            match (was, next) {
                (Spin::Idle | Spin::Down { .. }, Spin::Up { .. }) => {
                    play(&p.wind_up_sound);
                    events.write(WeaponEvent { owner, weapon: we, kind: WeaponEventKind::Action(SPIN_UP), replay: !first.0 });
                }
                (Spin::Up { .. } | Spin::Spun { .. }, Spin::Down { .. }) => {
                    play(&p.wind_down_sound);
                    events.write(WeaponEvent { owner, weapon: we, kind: WeaponEventKind::Action(SPIN_DOWN), replay: !first.0 });
                }
                (Spin::Up { .. } | Spin::Spun { .. }, Spin::Idle) => play(&p.wind_down_sound),
                _ => {}
            }
            let want_loop = match next {
                Spin::Spun { .. } if fire && mag.is_some_and(|m| m.clip == 0) => Loop::Empty,
                Spin::Spun { .. } if fire => Loop::Fire,
                Spin::Spun { .. } => Loop::Spin,
                _ => Loop::None,
            };
            if want_loop != gun.playing {
                match want_loop {
                    Loop::Spin => play(&p.spin_sound),
                    Loop::Fire => play(&p.fire_sound),
                    Loop::Empty => play(&p.empty_sound),
                    Loop::None => {}
                }
                gun.playing = want_loop;
            }
            if gun.spin != next {
                gun.spin = next;
            }
            // Firing waits for the spin.
            match next {
                Spin::Up { start } => st.next_primary = st.next_primary.max(start + p.wind_up as f64),
                Spin::Down { until } => st.next_primary = st.next_primary.max(until),
                Spin::Idle if active => st.next_primary = st.next_primary.max(now + p.wind_up as f64),
                _ => {}
            }
            // The damage ramp.
            let factor = match next {
                Spin::Spun { since, .. } => ramp(&p, now - since),
                _ => p.ramp_start,
            };
            let amount = gun.base_damage * factor;
            if dmg.amount != amount {
                dmg.amount = amount;
            }
            // The holder: slowed, grounded, holding on to it.
            let spinning = next != Spin::Idle;
            let speed = spinning.then(|| if state.is_some_and(|s| s.crouching) { 0.0 } else { p.spun_speed * U });
            if weapon.max_speed != speed {
                weapon.max_speed = speed;
            }
            if spinning && active && intent.jump {
                intent.jump = false;
            }
            match (spinning, locked) {
                (true, false) => {
                    commands.entity(we).insert(HolsterLocked);
                }
                (false, true) => {
                    commands.entity(we).remove::<HolsterLocked>();
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> MinigunParams {
        super::super::hook_params(super::super::MINIGUN, super::super::MINIGUN_HOOK)
    }

    /// 7.1: up 0.75 s, spun, down 1.15 (1.4 after attack2 alone), idle.
    #[test]
    fn it_winds_up_spins_and_winds_down() {
        let p = params();
        let s = step(&p, Spin::Idle, true, true, 1.0);
        assert_eq!(s, Spin::Up { start: 1.0 });
        assert_eq!(step(&p, s, true, true, 1.5), s);
        let spun = step(&p, s, true, true, 1.75);
        assert_eq!(spun, Spin::Spun { since: 1.75, fired: true });
        let until = |s: Spin| match s {
            Spin::Down { until } => until,
            other => panic!("{other:?}"),
        };
        assert!((until(step(&p, spun, false, false, 3.0)) - 4.15).abs() < 1e-6);
        let alt = Spin::Spun { since: 1.75, fired: false };
        assert!((until(step(&p, alt, false, false, 3.0)) - 4.4).abs() < 1e-6);
        assert_eq!(step(&p, Spin::Down { until: 4.15 }, false, false, 4.15), Spin::Idle);
    }

    /// 7.2 and test cases 13, 14: 50 % at the start, 75 % at 0.5 s, full
    /// after a second.
    #[test]
    fn the_damage_ramps_over_the_first_second() {
        let p = params();
        assert!((ramp(&p, 0.0) - 0.5).abs() < 1e-6);
        assert!((ramp(&p, 0.5) - 0.75).abs() < 1e-6);
        assert!((ramp(&p, 2.0) - 1.0).abs() < 1e-6);
    }
}
