//! The Minigun (specs/tf2/minigun.md, which supersedes weapons.md 7.1
//! and 8.3 where they differ; the rev-up penalty is weapons.md 7.2), the
//! hook `tf2:minigun` (`MinigunParams`, the weapon part `Tf2Minigun`):
//!
//! - **States** (minigun.md 2): idle, winding up, spinning (attack2),
//!   firing, dry (no ammo, still trying), stepped each tick in the
//!   predicted `Predict::Select` before selection (`spin`, `step`) as
//!   minigun.md 2.1 orders it: attack2 held (with ammo) and its time
//!   (`Tf2Minigun::next_alt`) passed runs the state step; then attack held
//!   and its time (`WeaponState::next_primary`) passed runs it too, or
//!   with no ammo the empty step (firing or spinning becomes dry); with
//!   neither held, the empty auto-switch, then idle once `idle_at` has
//!   passed (the wind-down). The step: idle winds up (`wind_up`, 0.75 s
//!   to both times and `idle_at`); a finished wind-up spins (attack2) or
//!   fires (attack), both times `fire_step` (0.1 s) on, so a held attack
//!   fires at tick 57 and then every 0.105 s; firing with only attack2
//!   spins (0.1 s), else each shot due sets `idle_at` `idle_after_fire`
//!   (0.2 s) on; spinning re-sends its spin-idle animation each step
//!   (`idle_at` `spin_idle` (0.3 s) on) and fires (or runs dry) once
//!   attack is held; dry starts the rev-up over and fires again when ammo
//!   comes. A released wind-up completes; a released spin idles 0.3 s
//!   later. A press waits for the draw (`Inventory::ready`). A shot comes
//!   only on a tick whose firing step ran (`WeaponState::next_primary`
//!   held a moment past `now` otherwise), as the original shoots inside
//!   that step.
//! - **Out of ammo** (minigun.md 3): attack2 does nothing (no spin), attack
//!   from idle nothing; with the player's `cl_autoswitch_empty` 1 (ours
//!   defaults to 0: it stays in hand) and nothing held, an empty gun whose times have
//!   passed, idle and past its spool-down switches to the previous weapon
//!   if that has ammo, else to the one with the most `SwitchWeight` (then
//!   the lowest slot) that has; an empty Minigun is `Unselectable`.
//! - **The holder** while not idle: at most `spun_speed` (110 units/s) by
//!   the weapon's `max_speed`, no jump; no switch (`HolsterLocked`) until
//!   the spool-down animation (`spool_down`, 1.333 s at rate 1, fade-out
//!   `spool_down_fade` 0.2 s) after the wind-down counts as finished
//!   (minigun.md 3.3: its cycle past 0.85, seen by the next tick's check
//!   since the switch check runs before the view model's advance: 77
//!   ticks); the empty auto-switch waits for the same.
//! - **The rev-up penalty** (weapons.md 7.2): tau the larger of the time
//!   since the wind-up began less `wind_up` and the time since the firing
//!   run's first shot was due; damage x `damage_floor` (0.5) until
//!   `penalty_flat` (0.2 s), then linear to 1 at `penalty_time` (1 s), the
//!   spread x `spread_ceiling` (1.5) falling to 1 at `penalty_time`.
//! - **What others get** (minigun.md 6): the state and the crit flag as
//!   `HeldState` (`net_state`), sent with what a character holds; others'
//!   copies take their state from it (`barrels`).
//! - **The barrel** (minigun.md 5, `barrels`): every Minigun on every
//!   machine, once per tick before the state step (not in prediction's
//!   replays): its speed 0.1 rad/s toward `BARREL_MAX_SPEED` (20) while
//!   not idle, toward 0 when idle, then its angle on by speed x tick. The
//!   view model (`view::spin_barrels`) and the world model
//!   (`view::spin_held_barrels`) turn their `barrel` bones by it.
//! - **Sounds** (minigun.md 4, `sounds`): not sent by the server; every
//!   machine plays one sound per Minigun from its state as it has it
//!   (predicted for its own player, replicated for others): the wind-up
//!   (weapon channel), the spin, fire (or crit fire) and empty loops (the
//!   static channel: replaced by nothing), the wind-down (weapon channel)
//!   while idle with the barrel still turning, nothing once it stops. A
//!   change stops the old one at once (`StopSound::local`) and starts the
//!   new one from its start; the loops repeat by their WAVs' own loop
//!   points (`audio::sound::loops`), never started again. Out of the
//!   hand the loops stop and a wind-down plays on; a death or a removed
//!   gun stops everything. View-model actions `SPIN_UP` and `SPIN_DOWN`
//!   (`WeaponEventKind::Action`) for its sequences, and its
//!   `view_anim::ViewPose` (1 while not idle).
//!
//! Known shortcuts: crits are off (weapons.md 2.6), so the crit flag is
//! never set and the crit loop never plays; a wind-up begun in the air
//! doesn't keep the air speed (the party's movement's own rule); with no
//! weights in the data the auto-switch's fallback is the lowest slot with
//! ammo.

use bevy::{platform::collections::HashMap, prelude::*};
use serde::{Deserialize, Serialize};

use chimera_sim::weapon::{
    AUTOSWITCH_EMPTY_KEY, CHAN_WEAPON, DamageEffect, HeldState, Hitscan, HolsterLocked, Inventory, Magazine, NetPart, SpreadShape, SwitchWeight,
    Unselectable, Weapon, WeaponEvent, WeaponEventKind, WeaponState, view_anim::ViewPose,
};

use super::damage::U;
use crate::{
    core::{FirstTimePredicted, Health, Intent, SimClock, UserInfo},
    map::{PlaySound, StopSound, interp::NetDrawn},
};

/// The view-model actions (`WeaponEventKind::Action`).
pub const SPIN_UP: &str = "spin_up";
pub const SPIN_DOWN: &str = "spin_down";

/// The barrel's top speed, rad/s (minigun.md 5, `barrel_max_speed`).
pub const BARREL_MAX_SPEED: f32 = 20.0;
/// The barrel's change of speed per tick, rad/s (`barrel_step`).
pub const BARREL_STEP: f32 = 0.1;

/// `tf2:minigun`'s parameters (seconds, units/s, factors, sound entries).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MinigunParams {
    pub wind_up: f32,
    /// From the wind-up's end (or the switch to spinning) to the next
    /// attack time.
    pub fire_step: f32,
    /// After a shot, released, before it winds down.
    pub idle_after_fire: f32,
    /// The spin-idle animation's length: a released spin (or dry gun)
    /// winds down this long after its last step.
    pub spin_idle: f32,
    /// The spool-down animation a switch waits for: its length (at
    /// playback rate 1, which every new sequence starts at) and its
    /// cross-fade-out time; it counts as finished `fade` before its end
    /// (`chimera_world::map::anim::ticks_until_finished`).
    pub spool_down: f32,
    pub spool_down_fade: f32,
    /// The rev-up penalty's end, its flat start, its floors.
    pub penalty_time: f32,
    pub penalty_flat: f32,
    pub damage_floor: f32,
    pub spread_ceiling: f32,
    pub spun_speed: f32,
    pub wind_up_sound: String,
    pub spin_sound: String,
    pub fire_sound: String,
    pub fire_crit_sound: String,
    pub empty_sound: String,
    pub wind_down_sound: String,
}

/// The gun's state (minigun.md 2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    #[default]
    Idle,
    WindingUp,
    /// Turning, not firing (attack2).
    Spinning,
    Firing,
    /// No ammo, a button still held.
    Dry,
}

impl State {
    /// Not idle: the holder slowed and grounded, the barrel driven.
    pub fn turning(self) -> bool {
        self != State::Idle
    }

    const ALL: [State; 5] = [State::Idle, State::WindingUp, State::Spinning, State::Firing, State::Dry];
}

/// The crit flag's bit in `HeldState`.
const CRIT_BIT: u8 = 0x8;

/// The state and crit flag as `HeldState` carries them.
pub fn net_state(state: State, crit: bool) -> HeldState {
    HeldState(State::ALL.iter().position(|s| *s == state).unwrap_or(0) as u8 | if crit { CRIT_BIT } else { 0 })
}

/// `net_state` read back.
pub fn from_net_state(s: HeldState) -> (State, bool) {
    (State::ALL.get((s.0 & 0x7) as usize).copied().unwrap_or_default(), s.0 & CRIT_BIT != 0)
}

/// The sound a Minigun plays (minigun.md 4.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    WindUp,
    Spin,
    Fire,
    FireCrit,
    Empty,
    WindDown,
}

impl Sound {
    /// The one sound for `state`, the crit flag and the barrel's speed
    /// (minigun.md 4.1, 4.3).
    pub fn wanted(state: State, crit: bool, barrel: f32) -> Option<Sound> {
        match state {
            State::Idle if barrel > 0.0 => Some(Sound::WindDown),
            State::Idle => None,
            State::WindingUp => Some(Sound::WindUp),
            State::Spinning => Some(Sound::Spin),
            State::Firing if crit => Some(Sound::FireCrit),
            State::Firing => Some(Sound::Fire),
            State::Dry => Some(Sound::Empty),
        }
    }

    pub fn entry(self, p: &MinigunParams) -> &str {
        match self {
            Sound::WindUp => &p.wind_up_sound,
            Sound::Spin => &p.spin_sound,
            Sound::Fire => &p.fire_sound,
            Sound::FireCrit => &p.fire_crit_sound,
            Sound::Empty => &p.empty_sound,
            Sound::WindDown => &p.wind_down_sound,
        }
    }

    /// The wind-up and wind-down on the weapon channel; the loops on the
    /// static channel, which nothing replaces (None).
    pub fn channel(self) -> Option<u8> {
        match self {
            Sound::WindUp | Sound::WindDown => Some(CHAN_WEAPON),
            _ => None,
        }
    }
}

/// The barrel's spin (minigun.md 5): speed (rad/s), angle (rad, kept
/// within a turn) and the angle a tick before (for drawing between
/// ticks).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Barrel {
    pub speed: f32,
    pub angle: f32,
    pub prev: f32,
}

impl Barrel {
    /// One update: the speed `BARREL_STEP` toward `target`, never past
    /// it, then the angle on by speed x `dt`.
    pub fn update(&mut self, target: f32, dt: f32) {
        let d = target - self.speed;
        // (A hair over the step: 0.1 added up in f32 misses 0 by a ulp.)
        self.speed = if d.abs() <= BARREL_STEP + 1e-4 { target } else { self.speed + BARREL_STEP * d.signum() };
        self.prev = self.angle;
        self.angle += self.speed * dt;
        if self.angle >= std::f32::consts::TAU {
            self.angle -= std::f32::consts::TAU;
            self.prev -= std::f32::consts::TAU;
        }
    }

    /// The angle drawn `frac` (0-1) of the way from the last tick to the
    /// next.
    pub fn drawn(&self, frac: f32) -> f32 {
        self.prev + (self.angle - self.prev) * frac.clamp(0.0, 1.0)
    }
}

/// The Minigun's state (a predicted part; `barrel` is each machine's own).
#[derive(Component, Clone, Debug)]
pub struct Tf2Minigun {
    pub params: MinigunParams,
    pub state: State,
    /// The mode the step last saw: attack2 (true) or attack.
    pub secondary: bool,
    /// Attack2's next time (attack's is `WeaponState::next_primary`).
    pub next_alt: f64,
    /// The earliest time a released gun winds down.
    pub idle_at: f64,
    /// When the wind-up began (the rev-up penalty; none after a dry spell).
    pub wound: Option<f64>,
    /// When the firing run's first shot was due.
    pub run: Option<f64>,
    /// No switch away before this (the spool-down animation).
    pub spool_until: f64,
    /// The current shot is a crit (never: crits are off).
    pub crit: bool,
    /// Its data's damage per bullet (normalized) and spread, before the
    /// penalty.
    pub base_damage: f32,
    pub base_spread: f32,
    pub barrel: Barrel,
}

impl NetPart for Tf2Minigun {
    type Net = (State, bool, f64, f64, Option<f64>, Option<f64>, f64, bool);
    fn net(&self) -> Self::Net {
        (self.state, self.secondary, self.next_alt, self.idle_at, self.wound, self.run, self.spool_until, self.crit)
    }
    fn set_net(&mut self, n: Self::Net) {
        (self.state, self.secondary, self.next_alt, self.idle_at, self.wound, self.run, self.spool_until, self.crit) = n;
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
            state: State::Idle,
            secondary: false,
            next_alt: 0.0,
            idle_at: 0.0,
            wound: None,
            run: None,
            spool_until: 0.0,
            crit: false,
            base_damage,
            base_spread,
            barrel: Barrel::default(),
        },
        ViewPose(0),
        HeldState::default(),
    ));
}

/// The rev-up's spin time tau at `now` (weapons.md 7.2): while firing,
/// the larger of the time since the wind-up began less the wind-up and
/// the time since the run's first shot was due; else 0.
pub fn tau(gun: &Tf2Minigun, now: f64) -> f32 {
    if gun.state != State::Firing {
        return 0.0;
    }
    let wind = gun.wound.map_or(0.0, |w| now - w - gun.params.wind_up as f64);
    let fire = gun.run.map_or(0.0, |r| now - r);
    wind.max(fire).max(0.0) as f32
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

/// What one tick's buttons and ammo are.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    /// Attack held.
    pub a: bool,
    /// Attack2 held.
    pub b: bool,
    /// A round to fire.
    pub ammo: bool,
}

/// What a tick's step did besides the gun's own state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stepped {
    pub wound_up: bool,
    pub wound_down: bool,
    /// A firing step ran: a shot may come this tick.
    pub may_fire: bool,
    /// Nothing held: the empty auto-switch may look.
    pub released: bool,
}

const SLACK: f64 = 1e-6;

/// One tick of the gun at `now` (minigun.md 2.1): `next_primary` is
/// attack's next time (the weapon's), changed where the step sets both.
pub fn step(gun: &mut Tf2Minigun, input: Input, next_primary: &mut f64, now: f64) -> Stepped {
    let mut out = Stepped::default();
    if input.b && input.ammo && now + SLACK >= gun.next_alt {
        state_step(gun, input, next_primary, now, &mut out);
    }
    if input.a && now + SLACK >= *next_primary {
        if input.ammo {
            state_step(gun, input, next_primary, now, &mut out);
        } else if matches!(gun.state, State::Firing | State::Spinning) {
            // The empty step: dry, the spin-idle animation sent.
            gun.state = if gun.secondary { State::Spinning } else { State::Dry };
            gun.idle_at = now + gun.params.spin_idle as f64;
        }
    }
    if !input.a && !input.b {
        out.released = true;
        if gun.state != State::Idle && now + SLACK >= gun.idle_at {
            gun.state = State::Idle;
            gun.idle_at = now + 2.0;
            gun.wound = None;
            gun.run = None;
            out.wound_down = true;
        }
    }
    out
}

/// The state step (minigun.md 2.2).
fn state_step(gun: &mut Tf2Minigun, input: Input, next_primary: &mut f64, now: f64, out: &mut Stepped) {
    let (wind_up, fire_step, after_fire, spin_idle) =
        (gun.params.wind_up as f64, gun.params.fire_step as f64, gun.params.idle_after_fire as f64, gun.params.spin_idle as f64);
    if input.a {
        gun.secondary = false;
    } else if input.b {
        gun.secondary = true;
    }
    // Every place that sets the next attack sets both buttons' times.
    fn both(next_alt: &mut f64, next_primary: &mut f64, t: f64) {
        *next_alt = t;
        *next_primary = t;
    }
    match gun.state {
        State::Idle => {
            gun.state = State::WindingUp;
            let t = now + wind_up;
            both(&mut gun.next_alt, next_primary, t);
            gun.idle_at = t;
            gun.wound = Some(now);
            gun.run = None;
            out.wound_up = true;
        }
        State::WindingUp => {
            let t = now + fire_step;
            gun.state = if gun.secondary { State::Spinning } else { State::Firing };
            if gun.state == State::Firing {
                gun.run = Some(t);
            }
            both(&mut gun.next_alt, next_primary, t);
            gun.idle_at = t;
        }
        State::Firing => {
            if gun.secondary {
                gun.state = State::Spinning;
                let t = now + fire_step;
                both(&mut gun.next_alt, next_primary, t);
                gun.idle_at = t;
            } else if !input.ammo {
                gun.state = State::Dry;
            } else {
                out.may_fire = true;
                if now + SLACK >= *next_primary {
                    gun.idle_at = now + after_fire;
                }
            }
        }
        State::Spinning => {
            if !gun.secondary {
                gun.state = if input.ammo { State::Firing } else { State::Dry };
                if gun.state == State::Firing {
                    gun.run = Some(next_primary.max(now));
                }
            }
            gun.idle_at = now + spin_idle;
        }
        State::Dry => {
            gun.wound = None;
            gun.run = None;
            if input.ammo {
                gun.state = State::Firing;
                gun.run = Some(now);
            } else if gun.secondary {
                gun.state = State::Spinning;
            }
            gun.idle_at = now + spin_idle;
        }
    }
}

/// Whether a weapon has anything to fire (no magazine: a melee).
fn has_ammo(mag: Option<&Magazine>) -> bool {
    mag.is_none_or(|m| m.clip > 0 || m.reserve > 0)
}

/// Each minigun's tick (module doc).
#[allow(clippy::type_complexity)]
pub fn spin(
    mut owners: Query<(Entity, &mut Intent, &mut Inventory, Option<&Health>, Option<&UserInfo>), Without<NetDrawn>>,
    mut guns: Query<(
        Entity,
        &mut Tf2Minigun,
        &mut Weapon,
        &mut WeaponState,
        &mut DamageEffect,
        Option<&mut Hitscan>,
        Has<HolsterLocked>,
        Has<Unselectable>,
        Option<&mut ViewPose>,
        Option<&mut HeldState>,
    )>,
    others: Query<(&Weapon, Option<&Magazine>, Option<&SwitchWeight>), Without<Tf2Minigun>>,
    mags: Query<&Magazine>,
    clock: Res<SimClock>,
    first: Res<FirstTimePredicted>,
    mut events: MessageWriter<WeaponEvent>,
    mut commands: Commands,
) {
    let now = clock.now;
    for (owner, mut intent, mut inv, health, info) in &mut owners {
        // `cl_autoswitch_empty` (the player's, sent as userinfo; off without one).
        let auto_switch = info.and_then(|i| i.flag(AUTOSWITCH_EMPTY_KEY)) == Some(true);
        let alive = health.is_none_or(|h| h.current > 0.0);
        for w in inv.weapons.clone() {
            let Ok((we, mut gun, mut weapon, mut st, mut dmg, scan, locked, unselectable, pose, held)) = guns.get_mut(w) else { continue };
            let mag = mags.get(w).ok();
            let ammo = mag.is_none_or(|m| m.clip > 0);
            // An empty Minigun can't be drawn (minigun.md 3.3).
            match (has_ammo(mag), unselectable) {
                (false, false) => {
                    commands.entity(we).insert(Unselectable);
                }
                (true, true) => {
                    commands.entity(we).remove::<Unselectable>();
                }
                _ => {}
            }
            let active = inv.active == Some(w) && alive;
            let mut stepped = Stepped::default();
            if !active {
                gun.state = State::Idle;
            } else if gun.state != State::Idle || inv.ready(now) {
                // From idle a press winds up only once the gun is drawn.
                let input = Input { a: intent.fire, b: intent.secondary, ammo };
                let mut next = st.next_primary;
                stepped = step(&mut gun, input, &mut next, now);
                if !stepped.may_fire && next <= now + SLACK && (gun.state != State::Idle || intent.fire) {
                    // No firing step this tick: no shot, no dry fire (held
                    // just past now).
                    next = now + 1e-3;
                }
                if st.next_primary != next {
                    st.next_primary = next;
                }
            }
            if stepped.wound_up {
                events.write(WeaponEvent { owner, weapon: we, kind: WeaponEventKind::Action(SPIN_UP), replay: !first.0 });
            }
            if stepped.wound_down {
                // The switch waits until the spool-down counts as finished
                // (minigun.md 3.3): its check runs before the tick's
                // view-model advance, 77 ticks for the stock's.
                let p = &gun.params;
                let ticks = chimera_world::map::anim::ticks_until_finished(p.spool_down, p.spool_down_fade, 1.0, clock.dt());
                gun.spool_until = now + ticks as f64 * clock.dt() as f64 - SLACK;
                events.write(WeaponEvent { owner, weapon: we, kind: WeaponEventKind::Action(SPIN_DOWN), replay: !first.0 });
            }
            // The empty auto-switch (minigun.md 3.3): idle, empty, its
            // times and spool-down passed, nothing held.
            if auto_switch
                && stepped.released
                && !has_ammo(mag)
                && gun.state == State::Idle
                && now + SLACK >= st.next_primary.max(gun.next_alt).max(gun.spool_until)
                && inv.wanted.is_none()
            {
                let usable = |e: Entity| others.get(e).ok().filter(|(_, m, _)| has_ammo(*m));
                let to = inv.last.filter(|l| *l != w && inv.weapons.contains(l) && usable(*l).is_some()).or_else(|| {
                    inv.weapons
                        .iter()
                        .filter_map(|e| usable(*e).map(|(wp, _, sw)| (*e, sw.map_or(0, |s| s.0), wp.slot)))
                        .max_by_key(|(_, weight, slot)| (*weight, std::cmp::Reverse(*slot)))
                        .map(|(e, ..)| e)
                });
                if let Some(to) = to {
                    inv.wanted = Some(to);
                }
            }
            let state = net_state(gun.state, gun.crit);
            if let Some(mut h) = held {
                h.set_if_neq(state);
            }
            // The view model idles lowered and aiming while it turns
            // (`game::view`: `m_spool_idle`), upright otherwise.
            let want_pose = ViewPose(u8::from(gun.state.turning()));
            if let Some(mut pose) = pose
                && *pose != want_pose
            {
                *pose = want_pose;
            }
            // The rev-up penalty at this tick's shot.
            let tau = tau(&gun, now);
            let p = &gun.params;
            let amount = gun.base_damage * damage_factor(p, tau);
            if dmg.amount != amount {
                dmg.amount = amount;
            }
            if let Some(mut scan) = scan {
                let spread = SpreadShape::Template(gun.base_spread * spread_factor(p, tau));
                if scan.spread != spread {
                    scan.spread = spread;
                }
            }
            // The holder: slowed and grounded while it turns, holding on to
            // it until the spool-down ends.
            let turning = gun.state.turning();
            let speed = turning.then_some(p.spun_speed * U);
            if weapon.max_speed != speed {
                weapon.max_speed = speed;
            }
            if turning && active && intent.jump {
                intent.jump = false;
            }
            match (turning || now + SLACK < gun.spool_until, locked) {
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

/// Every Minigun's barrel, once a tick on every machine before the state
/// step (minigun.md 5); others' copies take their state from `HeldState`
/// first.
pub fn barrels(
    mut guns: Query<(&mut Tf2Minigun, &Weapon, Option<&HeldState>)>,
    drawn: Query<(), With<NetDrawn>>,
    clock: Res<SimClock>,
) {
    let dt = clock.delta.as_secs_f32();
    for (mut gun, weapon, held) in &mut guns {
        if let Some(h) = held
            && weapon.owner.is_some_and(|o| drawn.contains(o))
        {
            let (state, crit) = from_net_state(*h);
            if gun.state != state || gun.crit != crit {
                gun.state = state;
                gun.crit = crit;
            }
        }
        let target = if gun.state.turning() { BARREL_MAX_SPEED } else { 0.0 };
        gun.barrel.update(target, dt);
    }
}

/// Each Minigun's one sound on this machine (module doc; minigun.md 4):
/// what it plays now, by gun.
pub fn sounds(
    guns: Query<(Entity, &Tf2Minigun, &Weapon)>,
    owners: Query<(&Inventory, &Transform, Option<&Health>)>,
    mut heard: Local<HashMap<Entity, (Entity, Sound, String)>>,
    mut play: MessageWriter<PlaySound>,
    mut stops: MessageWriter<StopSound>,
) {
    let mut stop = |owner: Entity, entry: &str| {
        stops.write(StopSound { by: None, source: owner, entry: entry.to_string(), local: true });
    };
    // Guns gone (a rung's swap, a strip, a remote copy rebuilt): silent.
    heard.retain(|g, (owner, _, entry)| {
        let keep = guns.contains(*g);
        if !keep {
            stop(*owner, entry);
        }
        keep
    });
    for (g, gun, weapon) in &guns {
        let holder = weapon.owner.and_then(|o| owners.get(o).ok().map(|x| (o, x)));
        let wanted = match holder {
            Some((_, (inv, _, health))) if health.is_none_or(|h| h.current > 0.0) => {
                if inv.active == Some(g) {
                    Sound::wanted(gun.state, gun.crit, gun.barrel.speed)
                } else {
                    // Put away: a wind-down plays on, the rest stops.
                    heard.get(&g).map(|h| h.1).filter(|s| *s == Sound::WindDown)
                }
            }
            _ => None,
        };
        let now = heard.get(&g).map(|h| h.1);
        if now == wanted {
            continue;
        }
        if let Some((owner, _, entry)) = heard.remove(&g) {
            stop(owner, &entry);
        }
        if let (Some(s), Some((owner, (_, at, _)))) = (wanted, holder) {
            let entry = s.entry(&gun.params).to_string();
            play.write(PlaySound {
                entry: entry.clone(),
                at: Some(at.translation),
                volume: None,
                pitch: None,
                source: Some(owner),
                channel: s.channel(),
            });
            heard.insert(g, (owner, s, entry));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> MinigunParams {
        super::super::hook_params(super::super::MINIGUN, super::super::MINIGUN_HOOK)
    }

    fn gun() -> Tf2Minigun {
        Tf2Minigun {
            params: params(),
            state: State::Idle,
            secondary: false,
            next_alt: 0.0,
            idle_at: 0.0,
            wound: None,
            run: None,
            spool_until: 0.0,
            crit: false,
            base_damage: 0.09,
            base_spread: 0.08,
            barrel: Barrel::default(),
        }
    }

    /// Ticks of 0.015 s from 0: (state, may fire) by tick under `input`.
    fn run(gun: &mut Tf2Minigun, next: &mut f64, ticks: std::ops::Range<u32>, input: impl Fn(u32) -> Input) -> Vec<(State, bool)> {
        ticks
            .map(|t| {
                let now = t as f64 * 0.015;
                let s = step(gun, input(t), next, now);
                if s.may_fire && *next <= now + SLACK {
                    *next = now + 0.105;
                }
                (gun.state, s.may_fire && *next > now)
            })
            .collect()
    }

    /// minigun.md 2.3: wind-up 50 ticks, firing; a released wind-up
    /// completes; a released spin idles 0.3 s on.
    #[test]
    fn the_state_machine_by_buttons() {
        let mut g = gun();
        let mut next = 0.0;
        let s = run(&mut g, &mut next, 0..60, |_| Input { a: true, b: false, ammo: true });
        assert_eq!(s[49].0, State::WindingUp);
        assert_eq!(s[50].0, State::Firing);
        // A tap: up to tick 50, then idle.
        let mut g = gun();
        let mut next = 0.0;
        let s = run(&mut g, &mut next, 0..60, |t| Input { a: t < 20, b: false, ammo: true });
        assert_eq!(s[49].0, State::WindingUp);
        assert_eq!(s[50].0, State::Idle);
        // attack2 to tick 149: spinning, idle at tick 169.
        let mut g = gun();
        let mut next = 0.0;
        let s = run(&mut g, &mut next, 0..200, |t| Input { a: false, b: t < 150, ammo: true });
        assert_eq!(s[50].0, State::Spinning);
        assert_eq!(s[168].0, State::Spinning);
        assert_eq!(s[169].0, State::Idle);
        // Empty: attack2 nothing at all.
        let mut g = gun();
        let mut next = 0.0;
        let s = run(&mut g, &mut next, 0..60, |_| Input { a: true, b: true, ammo: false });
        assert!(s.iter().all(|x| x.0 == State::Idle));
    }

    /// minigun.md 5, test 15: speed 0.1 n, angle 0.0015 n(n+1)/2.
    #[test]
    fn the_barrel_by_tick_15_16() {
        let mut b = Barrel::default();
        let mut total = 0.0f64;
        for n in 1..=200 {
            b.update(BARREL_MAX_SPEED, 0.015);
            total += b.speed as f64 * 0.015;
            if n == 50 {
                assert!((b.speed - 5.0).abs() < 1e-4);
                assert!((total - 1.9125).abs() < 1e-3, "{total}");
            }
        }
        assert_eq!(b.speed, 20.0);
        assert!((total - 30.15).abs() < 1e-2, "{total}");
        for n in 1..=200 {
            b.update(0.0, 0.015);
            assert_eq!(b.speed == 0.0, n == 200, "tick {n}: {}", b.speed);
        }
    }

    /// weapons.md 7.2 and its tests 13, 14, 49: 50 % until 0.2 s, 68.75 %
    /// at 0.5 s, full after a second; the spread 1.5 to 1.
    #[test]
    fn the_rev_up_penalty() {
        let p = params();
        assert!((damage_factor(&p, 0.1) - 0.5).abs() < 1e-6);
        assert!((damage_factor(&p, 0.2) - 0.5).abs() < 1e-6);
        assert!((damage_factor(&p, 0.5) - 0.6875).abs() < 1e-6);
        assert!((damage_factor(&p, 2.0) - 1.0).abs() < 1e-6);
        assert!((spread_factor(&p, 0.0) - 1.5).abs() < 1e-6);
        assert!((spread_factor(&p, 0.5) - 1.25).abs() < 1e-6);
        assert!((spread_factor(&p, 1.5) - 1.0).abs() < 1e-6);
        let mut g = gun();
        g.state = State::Firing;
        g.wound = Some(0.0);
        g.run = Some(2.0);
        assert!((tau(&g, 2.0) - 1.25).abs() < 1e-6);
        g.wound = None;
        assert!((tau(&g, 2.3) - 0.3).abs() < 1e-5);
        assert_eq!(net_state(State::Dry, true), HeldState(4 | CRIT_BIT));
        assert_eq!(from_net_state(net_state(State::Spinning, false)), (State::Spinning, false));
    }
}
