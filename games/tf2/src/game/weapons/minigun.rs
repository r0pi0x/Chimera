//! The Minigun's spin (specs/tf2/weapons.md 7 and its corrections from
//! the SDK), the hook `tf2:minigun` (`MinigunParams`, the weapon part
//! `Tf2Minigun`), run each tick in the predicted `Predict::Select` before
//! selection (`spin`):
//!
//! - **States** (7.1): idle; winding up for `wind_up` (0.75 s) from a
//!   press of attack or attack2 in idle (a release doesn't cancel it: it
//!   completes, then the gun stops); then spun (attack2), firing (attack,
//!   with ammo) or dry (attack, none), its next shot `fire_step` (0.1 s)
//!   after the wind-up ends, so a held attack fires about 0.85 s (57
//!   ticks) after the press and then every 0.105 s; firing to spun waits
//!   `fire_step` again. With neither button held it stops: at once when
//!   only spun, `idle_after_fire` (0.2 s) after the last shot when firing.
//!   There is **no gameplay wind-down**: the stop is idle at once (speed,
//!   jump and a new wind-up back); only switching weapons waits for the
//!   spool-down animation, `spool_down` (1.33 s: `Spin::Down`).
//! - **The holder** while winding up or spun: at most `spun_speed` (110
//!   units/s, crouching scales it as any speed) by the weapon's
//!   `max_speed`, no jump (the command's jump dropped); no switch
//!   (`HolsterLocked`) until the spool-down ends.
//! - **The rev-up penalty** (7.2, the 2017 rule): with tau the larger of
//!   the time since the wind-up began less `wind_up` and the time since
//!   the firing run's first shot (`tau`), each bullet's damage is x
//!   `damage_floor` (0.5) until `penalty_flat` (0.2 s), then linear to 1
//!   at `penalty_time` (1 s) (`damage_factor`), and the spread x
//!   `spread_ceiling` (1.5) falling linearly to 1 at `penalty_time`
//!   (`spread_factor`). A dry spell counts the wind-up as 0 until the
//!   next wind-up from idle; going to spun or dry restarts the run.
//! - **Sounds** (8.3): the wind-up, then a loop by what it does (spun,
//!   firing, dry), the wind-down when it stops, each replacing the last on
//!   the weapon channel (`map::PredictedSound`: the shooter's client plays
//!   its own); view-model actions `SPIN_UP` and `SPIN_DOWN`
//!   (`WeaponEventKind::Action`) for its sequences, and its
//!   `view_anim::ViewPose` (1 while winding up or spun: the view model
//!   idles in its spun sequence).
//!
//! Known shortcuts: crits are off (weapons.md 2.6), so the crit loop
//! (`...FireCrit`) never plays; a wind-up begun in the air doesn't keep
//! the air speed (the party's movement's own rule).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use chimera_sim::weapon::{
    CHAN_WEAPON, DamageEffect, Hitscan, HolsterLocked, Inventory, Magazine, NetPart, SpreadShape, Weapon, WeaponEvent,
    WeaponEventKind, WeaponState, view_anim::ViewPose,
};

use super::damage::{Tf2Gun, U};
use crate::{
    core::{FirstTimePredicted, Health, Intent, SimClock},
    map::{PlaySound, PredictedSound},
};

/// The view-model actions (`WeaponEventKind::Action`).
pub const SPIN_UP: &str = "spin_up";
pub const SPIN_DOWN: &str = "spin_down";

/// `tf2:minigun`'s parameters (seconds, units/s, factors, sound entries).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MinigunParams {
    pub wind_up: f32,
    /// From the wind-up's end to the first shot.
    pub fire_step: f32,
    /// After the last shot, released, before it stops.
    pub idle_after_fire: f32,
    /// The spool-down animation a switch waits for.
    pub spool_down: f32,
    /// The rev-up penalty's end, its flat start, its floors.
    pub penalty_time: f32,
    pub penalty_flat: f32,
    pub damage_floor: f32,
    pub spread_ceiling: f32,
    pub spun_speed: f32,
    pub wind_up_sound: String,
    pub spin_sound: String,
    pub fire_sound: String,
    pub empty_sound: String,
    pub wind_down_sound: String,
}

/// What a spun gun does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// Turning, not firing (attack2).
    #[default]
    Spinning,
    /// Firing (attack, ammo).
    Firing,
    /// Attack held with no ammo.
    Dry,
}

/// Where the spin is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Spin {
    #[default]
    Idle,
    /// Winding up since `start` (the press).
    Up { start: f64 },
    /// Spun: `wound` the wind-up's start (None after a dry spell),
    /// `run` when the firing run's first shot is due (firing only).
    Spun { wound: Option<f64>, mode: Mode, run: f64 },
    /// Idle in play, its spool-down animation running until `until` (no
    /// switch before).
    Down { until: f64 },
}

impl Spin {
    /// Winding up or spun: the holder slowed and grounded.
    pub fn turning(self) -> bool {
        matches!(self, Spin::Up { .. } | Spin::Spun { .. })
    }
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
    /// Its data's damage per bullet (normalized) and spread, before the
    /// penalty.
    pub base_damage: f32,
    pub base_spread: f32,
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
    let base_spread = match e.get::<Hitscan>().map(|h| &h.spread) {
        Some(SpreadShape::Template(s)) => *s,
        _ => 0.0,
    };
    e.insert((
        Tf2Minigun {
            params: p.clone(),
            spin: Spin::Idle,
            playing: Loop::None,
            base_damage,
            base_spread,
        },
        ViewPose(0),
    ));
}

/// The rev-up's spin time tau at `now` (7.2): the larger of the time
/// since the wind-up began less the wind-up and the time firing; 0 when
/// not spun.
pub fn tau(p: &MinigunParams, spin: Spin, now: f64) -> f32 {
    match spin {
        Spin::Spun { wound, mode, run } => {
            let wind = wound.map_or(0.0, |w| now - w - p.wind_up as f64);
            let fire = if mode == Mode::Firing { now - run } else { 0.0 };
            wind.max(fire).max(0.0) as f32
        }
        _ => 0.0,
    }
}

/// The damage factor g(tau): the floor until `penalty_flat`, then linear
/// to 1 at `penalty_time`.
pub fn damage_factor(p: &MinigunParams, tau: f32) -> f32 {
    let span = p.penalty_time - p.penalty_flat;
    let x = if span > 0.0 { ((tau - p.penalty_flat) / span).clamp(0.0, 1.0) } else { 1.0 };
    p.damage_floor + (1.0 - p.damage_floor) * x
}

/// The spread factor h(tau): the ceiling at 0, linear to 1 at
/// `penalty_time`.
pub fn spread_factor(p: &MinigunParams, tau: f32) -> f32 {
    let x = if p.penalty_time > 0.0 { (tau / p.penalty_time).clamp(0.0, 1.0) } else { 1.0 };
    p.spread_ceiling + (1.0 - p.spread_ceiling) * x
}

/// What one tick's step asks of the weapon's next shot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Due {
    /// Leave it.
    Keep,
    /// Not before this.
    AtLeast(f64),
}

/// The next state: `held` attack or attack2, `fire` attack, `ammo` a
/// round to fire, the gun's `last_shot` and its pending `next` shot, at
/// `now`; and what the next shot's time must become.
#[allow(clippy::too_many_arguments)]
pub fn step(p: &MinigunParams, spin: Spin, held: bool, fire: bool, ammo: bool, last_shot: f64, next: f64, now: f64) -> (Spin, Due) {
    const SLACK: f64 = 1e-6;
    let fire_step = p.fire_step as f64;
    let stop = || Spin::Down { until: now + p.spool_down as f64 };
    match spin {
        Spin::Idle | Spin::Down { .. } if held => (Spin::Up { start: now }, Due::AtLeast(now + (p.wind_up + p.fire_step) as f64)),
        Spin::Down { until } if now + SLACK >= until => (Spin::Idle, Due::Keep),
        Spin::Up { start } if now + SLACK >= start + p.wind_up as f64 => {
            if !held {
                return (stop(), Due::Keep);
            }
            let mode = match (fire, ammo) {
                (false, _) => Mode::Spinning,
                (true, true) => Mode::Firing,
                (true, false) => Mode::Dry,
            };
            let due = now + fire_step;
            let wound = (mode != Mode::Dry).then_some(start);
            (Spin::Spun { wound, mode, run: due }, Due::AtLeast(due))
        }
        Spin::Up { start } => (spin, Due::AtLeast(start + (p.wind_up + p.fire_step) as f64)),
        Spin::Spun { wound, mode, run } => {
            if !held {
                let waiting = mode == Mode::Firing && now + SLACK < last_shot + p.idle_after_fire as f64 && last_shot >= run - SLACK;
                return if waiting { (spin, Due::Keep) } else { (stop(), Due::Keep) };
            }
            match (fire, ammo, mode) {
                (true, true, Mode::Firing) => (spin, Due::Keep),
                (true, true, _) => (Spin::Spun { wound, mode: Mode::Firing, run: next.max(now) }, Due::Keep),
                (true, false, _) => (Spin::Spun { wound: None, mode: Mode::Dry, run }, Due::Keep),
                (false, _, Mode::Firing) => (Spin::Spun { wound, mode: Mode::Spinning, run }, Due::AtLeast(now + fire_step)),
                (false, _, _) => (Spin::Spun { wound, mode: Mode::Spinning, run }, Due::Keep),
            }
        }
        s => (s, Due::Keep),
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
    mut owners: Query<(Entity, &mut Intent, &Inventory, &Transform, Option<&Health>)>,
    mut guns: Query<(
        Entity,
        &mut Tf2Minigun,
        &mut Weapon,
        &mut WeaponState,
        &mut DamageEffect,
        Option<&mut Hitscan>,
        Option<&Magazine>,
        Option<&Tf2Gun>,
        Has<HolsterLocked>,
        Option<&mut ViewPose>,
    )>,
    clock: Res<SimClock>,
    first: Res<FirstTimePredicted>,
    mut sounds: MessageWriter<PredictedSound>,
    mut events: MessageWriter<WeaponEvent>,
    mut commands: Commands,
) {
    let now = clock.now;
    for (owner, mut intent, inv, at, health) in &mut owners {
        let alive = health.is_none_or(|h| h.current > 0.0);
        for &w in &inv.weapons {
            let Ok((we, mut gun, mut weapon, mut st, mut dmg, scan, mag, tf2, locked, pose)) = guns.get_mut(w) else { continue };
            let active = inv.active == Some(w) && alive;
            let fire = active && intent.fire;
            let held = active && (intent.fire || intent.secondary);
            let ammo = mag.is_none_or(|m| m.clip > 0);
            let last_shot = tf2.map_or(f64::MIN, |g| g.last_shot);
            let was = gun.spin;
            let (next, due) = if active {
                step(&gun.params, was, held, fire, ammo, last_shot, st.next_primary, now)
            } else {
                (Spin::Idle, Due::Keep)
            };
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
                Spin::Spun { mode: Mode::Dry, .. } => Loop::Empty,
                Spin::Spun { mode: Mode::Firing, .. } => Loop::Fire,
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
            // The view model idles lowered and aiming while it spins up or
            // is spun (`game::view`: `m_spool_idle`), upright otherwise.
            let want_pose = ViewPose(u8::from(next.turning()));
            if let Some(mut pose) = pose
                && *pose != want_pose
            {
                *pose = want_pose;
            }
            // Firing waits for the spin.
            if let Due::AtLeast(t) = due
                && st.next_primary < t
            {
                st.next_primary = t;
            }
            // The rev-up penalty at this tick's shot.
            let tau = tau(&p, next, now);
            let amount = gun.base_damage * damage_factor(&p, tau);
            if dmg.amount != amount {
                dmg.amount = amount;
            }
            if let Some(mut scan) = scan {
                let spread = SpreadShape::Template(gun.base_spread * spread_factor(&p, tau));
                if scan.spread != spread {
                    scan.spread = spread;
                }
            }
            // The holder: slowed and grounded while it turns, holding on to
            // it until the spool-down ends.
            let turning = next.turning();
            let speed = turning.then_some(p.spun_speed * U);
            if weapon.max_speed != speed {
                weapon.max_speed = speed;
            }
            if turning && active && intent.jump {
                intent.jump = false;
            }
            match (next != Spin::Idle, locked) {
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

    /// 7.1: up 0.75 s, spun with the first shot 0.1 s on; released while
    /// spun: idle at once (the spool-down only holds the switch); a
    /// release during the wind-up lets it complete.
    #[test]
    fn it_winds_up_spins_and_stops_at_once() {
        let p = params();
        let (s, due) = step(&p, Spin::Idle, true, true, true, f64::MIN, 0.0, 1.0);
        assert_eq!(s, Spin::Up { start: 1.0 });
        assert!(matches!(due, Due::AtLeast(t) if (t - 1.85).abs() < 1e-6));
        // Let go during the wind-up: it goes on.
        let (s2, _) = step(&p, s, false, false, true, f64::MIN, 1.85, 1.5);
        assert_eq!(s2, s);
        let (spun, due) = step(&p, s, true, true, true, f64::MIN, 1.85, 1.75);
        assert!(matches!(spun, Spin::Spun { wound: Some(1.0), mode: Mode::Firing, run } if (run - 1.85).abs() < 1e-6), "{spun:?}");
        assert!(matches!(due, Due::AtLeast(t) if (t - 1.85).abs() < 1e-6));
        // Only spun: let go, down (cosmetic) at once.
        let only = Spin::Spun { wound: Some(1.0), mode: Mode::Spinning, run: 1.85 };
        let (down, _) = step(&p, only, false, false, true, f64::MIN, 1.85, 3.0);
        assert!(matches!(down, Spin::Down { until } if (until - 4.33).abs() < 1e-6));
        assert!(!down.turning());
        // Firing: 0.2 s after the last shot.
        let firing = Spin::Spun { wound: Some(1.0), mode: Mode::Firing, run: 1.85 };
        assert_eq!(step(&p, firing, false, false, true, 2.9, 3.0, 3.0).0, firing);
        assert!(matches!(step(&p, firing, false, false, true, 2.9, 3.0, 3.1).0, Spin::Down { .. }));
        // A press while it spools down winds up again.
        assert_eq!(step(&p, down, true, false, true, 2.9, 3.0, 3.5).0, Spin::Up { start: 3.5 });
    }

    /// 7.2 and test cases 13, 14, 49: 50 % until 0.2 s, 68.75 % at 0.5 s,
    /// full after a second; the spread 1.5 to 1.
    #[test]
    fn the_rev_up_penalty() {
        let p = params();
        assert!((damage_factor(&p, 0.1) - 0.5).abs() < 1e-6);
        assert!((damage_factor(&p, 0.2) - 0.5).abs() < 1e-6);
        assert!((damage_factor(&p, 0.5) - 0.6875).abs() < 1e-6);
        assert!((damage_factor(&p, 2.0) - 1.0).abs() < 1e-6);
        assert!((spread_factor(&p, 0.0) - 1.5).abs() < 1e-6);
        assert!((spread_factor(&p, 0.1) - 1.45).abs() < 1e-6);
        assert!((spread_factor(&p, 0.5) - 1.25).abs() < 1e-6);
        assert!((spread_factor(&p, 1.5) - 1.0).abs() < 1e-6);
        // Spun on attack2 for 2 s from the press: the first shot's tau is
        // the wind's, 1.25 s.
        let s = Spin::Spun { wound: Some(0.0), mode: Mode::Firing, run: 2.0 };
        assert!((tau(&p, s, 2.0) - 1.25).abs() < 1e-6);
        // After a dry spell only the run counts.
        let s = Spin::Spun { wound: None, mode: Mode::Firing, run: 2.0 };
        assert!((tau(&p, s, 2.3) - 0.3).abs() < 1e-5);
    }
}
