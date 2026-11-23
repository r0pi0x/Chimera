//! The Flame Thrower (specs/tf2/weapons_more.md 7 and its corrections from
//! the SDK), the hook `tf2:flame` (`FlameParams`, the weapon part
//! `Tf2Flame`), as flying flame points:
//!
//! - **Emission**: the frame fires a round every `emit` (0.02 s, `set`
//!   timing: every 2 ticks) while attack is held; each is a flame point
//!   (`flames`, the server's). **Ammo** (`ammo`, predicted, after the
//!   frame): the round is given back and `ammo_rate` x `emit` (0.28) added
//!   to an accumulator whose whole units leave the clip (the rest kept to
//!   two decimals): about 9.3 a second, 200 last about 21.4 s.
//! - **A point** (specs/tf2/flames.md 1; `FlameParams::point`,
//!   `FlamePoint::update`, the same code on every machine) starts from
//!   its round (`Fired`: the eye, angles and seed) `origin` (40 forward, 5
//!   right) of the eye at `speed` (2450) along the aim, each screen axis
//!   deviated by (u1 + u2) x `spread` degrees (u uniform in +-0.5, drawn
//!   from the round's seed), its life `life` +- `life_jitter` (0.6 +- 0.1
//!   s), at most `max_points` (30) a Pyro in slots cycling 0-29; none with
//!   the eye under water. Each update it moves by its velocity + `drift`
//!   (50) up + the Pyro's velocity along its direction at its emission, its
//!   box (half-size `size`, 12) swept against the world (solid only):
//!   on a wall it slides on (the step's part into the surface dropped, the
//!   Pyro's part dropped, the slid step velocity, drift included, its
//!   velocity: it climbs walls), started in a solid or its centre in water
//!   it's gone; then its speed x (1 - `drag` x dt) (8.5): about 340 units
//!   of reach, no range check and no cone. It dies at its life's end.
//! - **At a wall** (`blocked`, predicted, before the frame; weapons_more.md
//!   7.1): each emission due tests the line from the eye to `origin` (40
//!   forward, 5 right, any stance or pitch) against the world; blocked,
//!   the frame sees no attack: no point, no ammo, no burst (no stream, no
//!   start or loop sound); a burst blocked at an emission ends as at a
//!   release, and restarts on the first clear tick.
//! - **The server's points** (`FlameStream`, `flames`, each tick) hurt;
//!   **every drawing machine runs them again** (`FlameShown`,
//!   `shown_flames`, each frame by its time) from the same rounds (its own
//!   player's predicted, others' as `RemoteShot`, played out one `emit`
//!   apart, `RemoteRounds`: they arrive in batches) for the stream drawn on
//!   them (`looks::flame_stream`).
//! - **Hits** (`flames`): each tick every point whose swept box (half-size
//!   `size`, 12) meets an enemy's box touches him; the oldest is the one
//!   that hits. A victim being burned has a **heat** H per Pyro: + 1 per
//!   touching point (up to + 2 for one younger than `fresh`, 0.02 s), -
//!   `heat_decay` (1.25) a tick; he enters with H 1 at his first damaging
//!   hit and is forgotten `hit_interval` (0.075 s) after his last. A hit
//!   damages at most every `hit_interval`: max(1, `damage` (13) x g_age x
//!   g_heat), g_age = max(0.5, 1 - age / its life), g_heat = 0.5 + 0.5
//!   clamp((H - 10) / 40, 0, 1) (`hit_damage`), rounded, of kind burn.
//!   Teammates: nothing at all.
//! - **Afterburn** (`Burning`, `afterburn`): ignition gives `burn_start`
//!   (3 s), every damaging hit `burn_add` (0.4 s) more, at most
//!   `burn_max` (10 s); every `burn_interval` (0.5 s) from ignition, while
//!   any is left, `burn_damage` (4) and `burn_interval` less of it (one
//!   hit: 3.4 s, 7 ticks, 28); out at waist-deep water. An enemy Pyro
//!   takes the hits but only a `pyro_burn` (0.25 s) look, no afterburn.
//! - **Sounds** (`sounds`, predicted; effects.md 14.2): the pilot light
//!   (`pilot_sound`, played once, not looped) from the tick the gun is
//!   drawn; a burst stops it and starts the start sound (volume 1, no
//!   channel: never replaced) and the loop (volume 0, the weapon
//!   channel), ramped in straight lines over `burst_fade` (3.5 s) to 0
//!   and 1 (`map::FadeSound`, one each per burst); the release cuts both
//!   in its tick and plays the end (`end_sound`, `.WindDown`, no channel,
//!   so a new burst doesn't cut it) and the pilot light from its start,
//!   but not when the burst ended with the clip empty. The start, the
//!   loops and the pilot light stop when the gun leaves the hand
//!   (`held_sounds::HeldSounds`: a switch, a rung's swap, a death); a
//!   critical burst (its Pyro crit-boosted, `crit_boost`) loops
//!   `loop_crit_sound`, and a burst turning critical or not while firing
//!   swaps the loops at once at full volume, no start, no end (effects.md
//!   14). Others get them all from the server (`PredictedSound`,
//!   `StopSound`, `FadeSound`).
//!
//! The flames' look is `looks::flame_stream`'s (`new_flame` riding the
//! drawn points).
//!
//! Known shortcuts: the point's random is the round's shot seed (0-255,
//! the same everywhere), not the spawn tick and a per-Pyro seed; a point
//! on a wall stops where its box touches (12 units off an axis-aligned
//! surface; the game's slid remainder is traced once more); others'
//! points start when their round's turn comes (one `emit` after the
//! last, not at the server's spawn tick: no catch-up for the time it
//! missed) and take the Pyro's velocity as this machine has it; this
//! machine's own player's points are predicted (TF2's appear a network
//! update late); a press under 0.2 s emits only while held; no airblast (attack2); random
//! crits are off; the fades run on each machine's own clock from the
//! burst's first tick (TF2 re-sends the volumes every tick); Pyros aren't immune to afterburn (no classes on the victims);
//! health kits and Medics don't put a burn out.

use bevy::prelude::*;
use serde::Deserialize;

use avian3d::prelude::*;
use chimera_sim::weapon::{
    CHAN_WEAPON, Inventory, held_sounds::HeldSounds, Magazine, NetPart, Trigger, Weapon, WeaponEvent, WeaponEventKind, aim_of,
    random::shared_random, remote::RemoteShot,
};
use source_engine::source_movement::SourceMovement;

use super::{
    super::classes::{Classes, Tf2Class},
    blast::boxed,
    damage::{HP, Tf2Gun, U, round_points},
};
use crate::{
    core::{
        Damage, DamageKind, FirstTimePredicted, FriendlyFire, Health, Hitgroup, Intent, MapWater, MovementState, SOLID_LAYERS, SimClock,
        Team, Velocity,
    },
    map::{PlaySound, PredictedSound},
};

/// The Pyro's class name (classes.toml).
pub const PYRO: &str = "pyro";

/// `tf2:flame`'s parameters (units, points, seconds, degrees).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FlameParams {
    pub damage: f32,
    pub emit: f32,
    pub ammo_rate: f32,
    pub speed: f32,
    pub drag: f32,
    pub drift: f32,
    pub origin: [f32; 2],
    pub spread: f32,
    pub life: f32,
    pub life_jitter: f32,
    pub max_points: usize,
    pub size: f32,
    pub hit_interval: f32,
    pub age_floor: f32,
    pub heat_floor: f32,
    pub heat_low: f32,
    pub heat_high: f32,
    pub heat_decay: f32,
    pub fresh: f32,
    pub min_damage: f32,
    pub burn_damage: f32,
    pub burn_interval: f32,
    pub burn_start: f32,
    pub burn_add: f32,
    pub burn_max: f32,
    pub pyro_burn: f32,
    #[serde(default)]
    pub start_sound: Option<String>,
    #[serde(default)]
    pub loop_sound: Option<String>,
    /// The loop of a critical burst (effects.md 14).
    #[serde(default)]
    pub loop_crit_sound: Option<String>,
    #[serde(default)]
    pub end_sound: Option<String>,
    #[serde(default)]
    pub hit_sound: Option<String>,
    /// The pilot light: played once from each draw and release
    /// (effects.md 14.2).
    #[serde(default)]
    pub pilot_sound: Option<String>,
    /// The burst's fades (s): the start sound from 1 to 0, the loop from
    /// 0 to 1 (0: both at full volume).
    #[serde(default)]
    pub burst_fade: f32,
}

impl FlameParams {
    /// A hit's damage (points, unrounded) by a point of `age` of its own
    /// `life` on a victim of heat `heat`.
    pub fn hit_damage(&self, age: f32, life: f32, heat: f32) -> f32 {
        let g_age = (1.0 - age / life.max(1e-6)).max(self.age_floor);
        let span = (self.heat_high - self.heat_low).max(1e-6);
        let g_heat = self.heat_floor + (1.0 - self.heat_floor) * ((heat - self.heat_low) / span).clamp(0.0, 1.0);
        (self.damage * g_age * g_heat).max(self.min_damage)
    }

    /// The heat a touch by a point of `age` adds: 1, up to 2 when fresh.
    pub fn touch_heat(&self, age: f32) -> f32 {
        if age < self.fresh { (2.0 - age / self.fresh).clamp(1.0, 2.0) } else { 1.0 }
    }

    /// The burn left after a damaging hit on a victim with `left` (None:
    /// not burning).
    pub fn burn_after(&self, left: Option<f32>) -> f32 {
        (left.unwrap_or(self.burn_start) + self.burn_add).min(self.burn_max)
    }

    /// How far (units) a point flies in `ticks` ticks of `dt` along its
    /// aim (no drift, a still Pyro).
    pub fn reach(&self, ticks: u32, dt: f32) -> f32 {
        let k = 1.0 - self.drag * dt;
        self.speed * dt * (1.0 - k.powi(ticks as i32)) / (1.0 - k)
    }
}

/// The Flame Thrower's state (a predicted part): flaming, the ammo
/// accumulator.
#[derive(Component, Clone, Debug)]
pub struct Tf2Flame {
    pub params: FlameParams,
    pub flaming: bool,
    pub owed: f32,
    /// The burst playing is critical (its loop the crit one).
    pub crit: bool,
    /// It was its holder's drawn weapon last tick (the pilot light starts
    /// when it becomes so; this machine's own, not sent).
    pub drawn: bool,
}

impl NetPart for Tf2Flame {
    type Net = (bool, f32, bool);
    fn net(&self) -> Self::Net {
        (self.flaming, self.owed, self.crit)
    }
    fn set_net(&mut self, (flaming, owed, crit): Self::Net) {
        self.flaming = flaming;
        self.owed = owed;
        self.crit = crit;
    }
}

/// `tf2:flame` on a weapon.
pub fn attach(e: &mut EntityWorldMut, p: &FlameParams) {
    e.insert((Tf2Flame { params: p.clone(), flaming: false, owed: 0.0, crit: false, drawn: false }, FlameStream::default(), FlameShown::default()));
    // The start, the loops and the pilot light end when it leaves the
    // hand (`held_sounds`).
    let held: Vec<String> =
        p.start_sound.iter().chain(p.loop_sound.iter()).chain(p.loop_crit_sound.iter()).chain(p.pilot_sound.iter()).cloned().collect();
    if !held.is_empty() {
        e.insert(HeldSounds::new(held));
    }
}

/// One flame point (engine space: metres, m/s).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlamePoint {
    pub at: Vec3,
    pub before: Vec3,
    pub velocity: Vec3,
    /// The Pyro's velocity along its direction at its emission (kept
    /// until it meets a wall).
    pub carried: Vec3,
    pub born: f64,
    pub life: f32,
    /// Its slot, 0 to `max_points` - 1 (cycling): its control point is
    /// slot + 1 (flames.md 3.2).
    pub slot: u8,
    /// Walls it has slid on.
    pub wall_hits: u16,
}

/// What a point's box meets on a step (`FlamePoint::update`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sweep {
    Clear,
    /// The box starts inside something solid.
    StartSolid,
    /// It stops after this fraction of the step on a surface of `normal`.
    Hit { fraction: f32, normal: Vec3 },
}

impl FlameParams {
    /// A new point (flames.md 1, the same on every machine): from the
    /// round's eye, angles and seed, the Pyro's velocity (engine space),
    /// born `born` in slot `slot`.
    #[allow(clippy::too_many_arguments)]
    pub fn point(&self, eye: Vec3, yaw: f32, pitch: f32, seed: u32, pyro: Vec3, born: f64, slot: u8) -> FlamePoint {
        let aim = aim_of(yaw, pitch);
        let (fwd, right, up) = (aim * Vec3::NEG_Z, aim * Vec3::X, aim * Vec3::Y);
        let mut rng = shared_random(seed, "tf2:flame", 0);
        let spread = self.spread.to_radians();
        let a = (rng.next() - 0.5 + rng.next() - 0.5) * spread;
        let b = (rng.next() - 0.5 + rng.next() - 0.5) * spread;
        let dir = (fwd + right * a + up * b).normalize_or(fwd);
        let life = self.life + self.life_jitter * (rng.next() * 2.0 - 1.0);
        let start = eye + (fwd * self.origin[0] + right * self.origin[1]) * U;
        FlamePoint {
            at: start,
            before: start,
            velocity: dir * self.speed * U,
            carried: dir * pyro.dot(dir),
            born,
            life,
            slot,
            wall_hits: 0,
        }
    }

    /// A point's box half-size (engine space; the size multiplier is 1
    /// for the stock weapon).
    pub fn half(&self) -> f32 {
        self.size * U
    }
}

/// A little off a wall a point slid on (engine space), so the next
/// step along it doesn't start touching it.
const OFF_WALL: f32 = 0.01 * U;

impl FlamePoint {
    /// Its age at `now`.
    pub fn age(&self, now: f64) -> f32 {
        (now - self.born) as f32
    }

    /// One update of `dt` at `now` (flames.md 1, every machine): false when
    /// it's gone (its life over, in water, started in a solid). `sweep`
    /// casts its box over a step (from, motion); `wet` says whether a
    /// point is in water.
    pub fn update(&mut self, p: &FlameParams, now: f64, dt: f32, sweep: &impl Fn(Vec3, Vec3) -> Sweep, wet: &impl Fn(Vec3) -> bool) -> bool {
        if self.age(now) >= self.life || wet(self.at) {
            return false;
        }
        let step_velocity = self.velocity + Vec3::Y * p.drift * U + self.carried;
        let motion = step_velocity * dt;
        let (to, kept) = match sweep(self.at, motion) {
            Sweep::StartSolid => return false,
            Sweep::Clear => (self.at + motion, self.velocity),
            Sweep::Hit { fraction, normal } => {
                // It slides on: the step velocity loses what goes into the
                // surface (and keeps the drift: flames.md 1, the slide's
                // quirk), the Pyro's part is dropped, and the rest of the
                // step goes along the surface.
                self.wall_hits = self.wall_hits.saturating_add(1);
                self.carried = Vec3::ZERO;
                let into = step_velocity.dot(normal).min(0.0);
                let slid = step_velocity - normal * into;
                let stop = self.at + motion * fraction + normal * OFF_WALL;
                let rest = slid * dt * (1.0 - fraction);
                let to = match sweep(stop, rest) {
                    Sweep::Clear => stop + rest,
                    Sweep::Hit { fraction: f, .. } => stop + rest * f,
                    Sweep::StartSolid => stop,
                };
                (to, slid)
            }
        };
        let speed = kept.length();
        let slower = (speed - dt * p.drag * speed).clamp(0.0, speed);
        self.velocity = kept.normalize_or_zero() * slower;
        self.before = self.at;
        self.at = to;
        true
    }
}

/// A victim's heat from one Pyro.
#[derive(Clone, Copy, Debug)]
pub struct Heat {
    pub victim: Entity,
    pub heat: f32,
    pub last_hit: f64,
}

/// One Flame Thrower's points (its manager): the points oldest first,
/// the next slot, when it last fired a round.
#[derive(Clone, Debug, Default)]
pub struct Points {
    pub points: Vec<FlamePoint>,
    pub next_slot: u8,
    pub last_round: Option<f64>,
    /// Whether the eye was under water at that round.
    pub under_water: bool,
}

impl Points {
    /// A round fired at `now` from `eye` along (`yaw`, `pitch`) with
    /// `seed`, the Pyro at `pyro`: a point unless the eye is under water
    /// or `max_points` live.
    #[allow(clippy::too_many_arguments)]
    pub fn round(&mut self, p: &FlameParams, eye: Vec3, yaw: f32, pitch: f32, seed: u32, pyro: Vec3, now: f64, wet: bool) {
        self.last_round = Some(now);
        self.under_water = wet;
        if wet || self.points.len() >= p.max_points {
            return;
        }
        let slot = self.next_slot;
        self.next_slot = ((slot as usize + 1) % p.max_points.max(1)) as u8;
        self.points.push(p.point(eye, yaw, pitch, seed, pyro, now, slot));
    }

    /// Every point's update of `dt` at `now`; the gone ones dropped.
    pub fn update(&mut self, p: &FlameParams, now: f64, dt: f32, sweep: &impl Fn(Vec3, Vec3) -> Sweep, wet: &impl Fn(Vec3) -> bool) {
        self.points.retain_mut(|pt| pt.update(p, now, dt, sweep, wet));
    }

    /// The newest point.
    pub fn newest(&self) -> Option<&FlamePoint> {
        self.points.last()
    }
}

/// The server's flames of one Flame Thrower (what hurts): its points,
/// moved each tick, and its victims' heat.
#[derive(Component, Clone, Debug, Default)]
pub struct FlameStream {
    pub points: Points,
    pub heat: Vec<Heat>,
}

/// The same points as every machine that draws runs them (flames.md 1,
/// "Client copy"): from the same rounds (its own predicted, others' as
/// the server sends them, `RemoteShot`), the same seeds, moved each frame
/// by the frame's time (`shown_flames`); what the looks put the flame
/// sprites on (`looks::flame_stream`). `now` is the drawing clock's.
#[derive(Component, Clone, Debug, Default)]
pub struct FlameShown {
    pub points: Points,
    pub now: f64,
    /// Others' rounds waiting for their turn (`RemoteRounds`).
    pub remote: RemoteRounds,
}

/// Others' rounds, played out one `emit` apart (flames.md 1, "Client
/// copy": TF2 makes each point at its own spawn tick). The server's
/// rounds reach a client in batches (several in one update); made at
/// once they were points in the same place, the stream drawn as a few
/// separate puffs with every sprite between batches on the newest one.
/// A round is made at the later of its arrival and one `emit` after the
/// last one made; one held more than `REMOTE_HOLD_MAX` behind is made at
/// once.
#[derive(Clone, Debug, Default)]
pub struct RemoteRounds {
    pub waiting: std::collections::VecDeque<RemoteRound>,
    /// When the last one was (or will be) made.
    pub last: Option<f64>,
}

/// One of `RemoteRounds`: when to make it, from where, which way, its
/// seed and the Pyro's velocity.
#[derive(Clone, Copy, Debug)]
pub struct RemoteRound {
    pub at: f64,
    pub eye: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub seed: u32,
    pub pyro: Vec3,
}

/// At most this long (s) a remote round waits behind the ones before it.
pub const REMOTE_HOLD_MAX: f64 = 0.1;

impl RemoteRounds {
    /// A round that arrived at `now`: its turn after the last one, `emit`
    /// s apart.
    pub fn arrive(&mut self, now: f64, emit: f64, mut round: RemoteRound) {
        let turn = self.last.map_or(now, |l| (l + emit).max(now));
        round.at = if turn - now > REMOTE_HOLD_MAX { now } else { turn };
        self.last = Some(round.at);
        self.waiting.push_back(round);
    }

    /// The rounds whose turn has come by `now`, in order.
    pub fn due(&mut self, now: f64) -> Vec<RemoteRound> {
        let mut out = Vec::new();
        while self.waiting.front().is_some_and(|r| r.at <= now + 1e-9) {
            out.extend(self.waiting.pop_front());
        }
        out
    }
}

/// On a burning character: the burn left (s), the next afterburn tick,
/// by whom and with what, its damage and interval; `harmless` the enemy
/// Pyro's look only.
#[derive(Component, Clone, Copy, Debug)]
pub struct Burning {
    pub left: f32,
    pub next: f64,
    pub attacker: Option<Entity>,
    pub weapon: &'static str,
    pub damage: f32,
    pub interval: f32,
    pub harmless: bool,
}

/// The stock Flame Thrower's numbers (its data's).
pub fn stock_params() -> FlameParams {
    super::hook_params(super::FLAMETHROWER, super::FLAME_HOOK)
}

/// A fresh burn by `attacker` with `weapon` at `now`: the Flame
/// Thrower's afterburn as a flame hit lights it (an item's ignite on hit,
/// item_attributes.md 4.11).
/// `p` the Flame Thrower's numbers (`stock_params`).
pub fn ignited(now: f64, attacker: Entity, weapon: &'static str, p: &FlameParams) -> Burning {
    Burning {
        left: p.burn_after(None),
        next: now + p.burn_interval as f64,
        attacker: Some(attacker),
        weapon,
        damage: p.burn_damage,
        interval: p.burn_interval,
        harmless: false,
    }
}

/// Before the frame (predicted; weapons_more.md 7.1 "Blocked at a wall"):
/// a drawn Flame Thrower whose emission is due with attack held tests the
/// line from the eye to the fixed point `origin` (40 forward, 5 right, 0
/// up in the eye's frame, at any stance and pitch) against the world (what
/// can't be damaged: solid, characters aside); blocked, the frame sees no
/// attack that tick, so nothing happens at all: no point, no ammo, the
/// burst never starts (no stream, no start or loop sound), and one firing
/// ends at that emission as a release does. The first clear tick with
/// attack held starts it as from idle. Off emission ticks (a burst's
/// odd ticks) nothing is tested.
#[allow(clippy::type_complexity)]
pub fn blocked(
    mut owners: Query<(&Inventory, &mut Intent, &Transform, Option<&MovementState>, Option<&Health>)>,
    guns: Query<(&Tf2Flame, &chimera_sim::weapon::WeaponState)>,
    world: FlameWorld,
    clock: Res<SimClock>,
) {
    for (inv, mut intent, at, state, health) in &mut owners {
        if !intent.fire || health.is_some_and(|h| h.current <= 0.0) {
            continue;
        }
        let Some((f, st)) = inv.active.and_then(|w| guns.get(w).ok()) else { continue };
        if st.next_primary > clock.now + 1e-5 {
            continue;
        }
        let eye = at.translation + state.map_or(Vec3::ZERO, |s| s.eye_offset);
        let aim = aim_of(intent.yaw, intent.pitch);
        let to = eye + (aim * Vec3::NEG_Z * f.params.origin[0] + aim * Vec3::X * f.params.origin[1]) * U;
        if world.line_blocked(eye, to) {
            intent.fire = false;
        }
    }
}

/// Whether a gun is flaming at `now`: its last round at most an interval
/// (and a tick of slack) old, its interval rounded up to whole ticks.
fn flaming(gun: &Tf2Gun, trigger: Option<&Trigger>, now: f64, dt: f64) -> bool {
    let cycle = trigger.map_or(0.02, |t| t.cycle) as f64;
    let ticks = (cycle / dt - 1e-6).ceil().max(1.0);
    now - gun.last_shot <= ticks * dt + dt * 0.5
}

/// After the frame (predicted): each round the Flame Thrower fired is
/// given back and its 0.28 of ammo owed; whole units leave the clip.
pub fn ammo(mut events: MessageReader<WeaponEvent>, mut guns: Query<(&mut Tf2Flame, &mut Magazine)>) {
    for e in events.read() {
        if !matches!(e.kind, WeaponEventKind::Fired { .. }) {
            continue;
        }
        let Ok((mut f, mut m)) = guns.get_mut(e.weapon) else { continue };
        let owed = ((f.owed + f.params.ammo_rate * f.params.emit) * 100.0 + 1e-3).floor() / 100.0;
        let take = owed.floor();
        f.owed = owed - take;
        m.clip = (m.clip + 1).saturating_sub(take as u32);
    }
}

/// The flaming state and its sounds (predicted, after the frame;
/// effects.md 14 and 14.2; module doc): drawn, the pilot light; starting,
/// the pilot light stopped, the start sound (no channel) and the loop
/// (the crit one when the burst is critical: its owner crit-boosted,
/// `crit_boost::CritBoosted`) on the weapon channel, faded over
/// `burst_fade`; the burst turning critical or not while firing cuts
/// both and starts the other loop at full volume, no start, no end;
/// stopping, both cut, the end (no channel) and, with ammo left and the
/// gun in hand, the pilot light again.
#[allow(clippy::type_complexity)]
pub fn sounds(
    owners: Query<(Entity, &Inventory, &Transform, Option<&Intent>, Has<super::crit_boost::CritBoosted>)>,
    mut guns: Query<(&mut Tf2Flame, &Tf2Gun, Option<&Trigger>, Option<&Magazine>)>,
    clock: Res<SimClock>,
    first: Res<FirstTimePredicted>,
    mut out: MessageWriter<PredictedSound>,
    mut stops: MessageWriter<crate::map::StopSound>,
    mut fades: MessageWriter<crate::map::FadeSound>,
) {
    for (owner, inv, at, intent, boosted) in &owners {
        for &w in &inv.weapons {
            let Ok((mut f, gun, trigger, mag)) = guns.get_mut(w) else { continue };
            let drawn = inv.active == Some(w);
            let deployed = drawn && !f.drawn;
            f.drawn = drawn;
            // A release ends the burst in its tick (not when the last
            // round has aged out).
            let held = intent.is_none_or(|i| i.fire);
            let now = drawn && held && flaming(gun, trigger, clock.now, clock.dt() as f64);
            let crit = now && boosted;
            let changed = now != f.flaming || (now && crit != f.crit);
            let (swap, was_crit) = (now && f.flaming, f.crit);
            f.flaming = now;
            f.crit = crit;
            if !first.0 || !(changed || deployed) {
                continue;
            }
            let p = &f.params;
            let loop_of = |crit: bool| if crit { p.loop_crit_sound.clone().or(p.loop_sound.clone()) } else { p.loop_sound.clone() };
            let play = |out: &mut MessageWriter<PredictedSound>, entry: String, channel: Option<u8>, volume: Option<f32>| {
                out.write(PredictedSound {
                    by: owner,
                    sound: PlaySound { entry, at: Some(at.translation), volume, pitch: None, source: Some(owner), channel },
                });
            };
            let stop = |stops: &mut MessageWriter<crate::map::StopSound>, entry: String| {
                stops.write(crate::map::StopSound { by: Some(owner), source: owner, entry, local: false });
            };
            let fade = |fades: &mut MessageWriter<crate::map::FadeSound>, entry: String, to: f32| {
                fades.write(crate::map::FadeSound { by: Some(owner), source: owner, entry, to, over: p.burst_fade, local: false });
            };
            if !changed {
                // Drawn, not flaming: the pilot light from its start.
                if let Some(pilot) = p.pilot_sound.clone() {
                    play(&mut out, pilot, None, None);
                }
                continue;
            }
            if swap {
                // Both cut at once, the other loop at full volume.
                for e in p.start_sound.iter().cloned().chain(loop_of(was_crit)) {
                    stop(&mut stops, e);
                }
                if let Some(l) = loop_of(crit) {
                    play(&mut out, l, Some(CHAN_WEAPON), None);
                }
                continue;
            }
            let fading = p.burst_fade > 0.0;
            if now {
                // The pilot light out; the start on no channel fading out,
                // the loop on the weapon's fading in.
                if let Some(pilot) = p.pilot_sound.clone() {
                    stop(&mut stops, pilot);
                }
                if let Some(s) = p.start_sound.clone() {
                    play(&mut out, s.clone(), None, None);
                    if fading {
                        fade(&mut fades, s, 0.0);
                    }
                }
                if let Some(l) = loop_of(crit) {
                    play(&mut out, l.clone(), Some(CHAN_WEAPON), fading.then_some(0.0));
                    if fading {
                        fade(&mut fades, l, 1.0);
                    }
                }
            } else {
                // Released (or run dry, or holstered): both cut in this
                // tick, the end, the pilot light again with ammo left.
                for e in p.start_sound.iter().cloned().chain(loop_of(was_crit)) {
                    stop(&mut stops, e);
                }
                if let Some(e) = p.end_sound.clone() {
                    play(&mut out, e, None, None);
                }
                let dry = mag.is_some_and(|m| m.clip == 0);
                if drawn
                    && !dry
                    && let Some(pilot) = p.pilot_sound.clone()
                {
                    play(&mut out, pilot, None, None);
                }
            }
        }
    }
}

/// Whether the segment `a`..`b` meets the box `min`..`max`.
pub fn segment_meets_box(a: Vec3, b: Vec3, min: Vec3, max: Vec3) -> bool {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for i in 0..3 {
        if d[i].abs() < 1e-9 {
            if a[i] < min[i] || a[i] > max[i] {
                return false;
            }
            continue;
        }
        let (mut lo, mut hi) = ((min[i] - a[i]) / d[i], (max[i] - a[i]) / d[i]);
        if lo > hi {
            std::mem::swap(&mut lo, &mut hi);
        }
        t0 = t0.max(lo);
        t1 = t1.min(hi);
        if t0 > t1 {
            return false;
        }
    }
    true
}

type Victims<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        Option<&'static ColliderAabb>,
        Option<&'static MovementState>,
        &'static Health,
        Option<&'static Team>,
        Option<&'static Burning>,
        Option<&'static Tf2Class>,
    ),
    With<Intent>,
>;

/// The world a point's box meets (solid only: characters, the living
/// and those drawn from snapshots, let it through).
#[derive(bevy::ecs::system::SystemParam)]
pub struct FlameWorld<'w, 's> {
    spatial: SpatialQuery<'w, 's>,
    colliders: Query<'w, 's, &'static ColliderOf>,
    characters: Query<'w, 's, (), Or<(With<Intent>, With<crate::map::interp::NetDrawn>)>>,
    water: Option<Res<'w, MapWater>>,
}

impl FlameWorld<'_, '_> {
    /// `half`'s box swept by `motion` from `from` (engine space).
    pub fn sweep(&self, half: f32, from: Vec3, motion: Vec3) -> Sweep {
        let Ok((dir, length)) = Dir3::new_and_length(motion) else { return Sweep::Clear };
        let shape = Collider::cuboid(half * 2.0, half * 2.0, half * 2.0);
        let filter = SpatialQueryFilter::default().with_mask(SOLID_LAYERS);
        let config = ShapeCastConfig { ignore_origin_penetration: false, ..ShapeCastConfig::from_max_distance(length) };
        let through = |e: Entity| {
            let body = self.colliders.get(e).map_or(e, |o| o.body);
            !self.characters.contains(body)
        };
        match self.spatial.cast_shape_predicate(&shape, from, Quat::IDENTITY, dir, &config, &filter, &through) {
            None => Sweep::Clear,
            Some(hit) if hit.distance <= 0.0 && hit.normal1.dot(*dir) >= -1e-4 => Sweep::Clear,
            Some(hit) if hit.distance <= 0.0 && motion.length() > 0.0 && self.inside(half, from) => Sweep::StartSolid,
            Some(hit) => Sweep::Hit { fraction: (hit.distance / length).clamp(0.0, 1.0), normal: hit.normal1.normalize_or(-*dir) },
        }
    }

    /// Whether `half`'s box at `at` overlaps something solid by more than
    /// a touch.
    fn inside(&self, half: f32, at: Vec3) -> bool {
        let shape = Collider::cuboid(half * 1.9, half * 1.9, half * 1.9);
        let filter = SpatialQueryFilter::default().with_mask(SOLID_LAYERS);
        self.spatial.shape_intersections(&shape, at, Quat::IDENTITY, &filter).into_iter().any(|e| {
            let body = self.colliders.get(e).map_or(e, |o| o.body);
            !self.characters.contains(body)
        })
    }

    /// Whether the line from `from` to `to` meets something solid that
    /// isn't a character (the Flame Thrower's wall test).
    pub fn line_blocked(&self, from: Vec3, to: Vec3) -> bool {
        let Ok((dir, length)) = Dir3::new_and_length(to - from) else { return false };
        let filter = SpatialQueryFilter::default().with_mask(SOLID_LAYERS);
        let through = |e: Entity| {
            let body = self.colliders.get(e).map_or(e, |o| o.body);
            !self.characters.contains(body)
        };
        self.spatial.cast_ray_predicate(from, dir, length, true, &filter, &through).is_some()
    }

    /// Whether `at` is in the map's water.
    pub fn wet(&self, at: Vec3) -> bool {
        super::looks::in_water(self.water.as_deref(), at)
    }
}

/// The flame points: emitted from the rounds fired, moved, hitting (the
/// server's).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn flames(
    mut rounds: MessageReader<WeaponEvent>,
    owners: Query<(Entity, &Inventory, Option<&MovementState>, Option<&Team>, Option<&Health>, Option<&Velocity>)>,
    mut guns: Query<(Entity, &Tf2Flame, &Weapon, &mut FlameStream)>,
    victims: Victims,
    world: FlameWorld,
    friendly_fire: Option<Res<FriendlyFire>>,
    classes: Option<Res<Classes>>,
    clock: Res<SimClock>,
    mut damage: MessageWriter<Damage>,
    mut commands: Commands,
) {
    let now = clock.now;
    let dt = clock.dt();
    let ff = friendly_fire.is_some_and(|f| f.0 != 0);
    // This tick's rounds: a point each (flames.md 1), in the hands that
    // fired them.
    for e in rounds.read() {
        let WeaponEventKind::Fired { origin, yaw, pitch, seed, .. } = e.kind else { continue };
        if e.replay {
            continue;
        }
        let Ok((_, f, _, mut stream)) = guns.get_mut(e.weapon) else { continue };
        let Ok((_, inv, _, _, health, velocity)) = owners.get(e.owner) else { continue };
        if health.is_some_and(|h| h.current <= 0.0) || inv.active != Some(e.weapon) {
            continue;
        }
        let pyro = velocity.map_or(Vec3::ZERO, |v| v.0);
        stream.points.round(&f.params, origin, yaw, pitch, seed, pyro, now, world.wet(origin));
    }
    for (owner, inv, _, team, _, _) in &owners {
        for &w in &inv.weapons {
            let Ok((_, f, weapon, mut stream)) = guns.get_mut(w) else { continue };
            let p = &f.params;
            let half = p.half();
            // Each point's move.
            stream.points.update(p, now, dt, &|from, motion| world.sweep(half, from, motion), &|at| world.wet(at));
            let stream = &mut *stream;
            // The heat cools.
            for h in &mut stream.heat {
                h.heat = (h.heat - p.heat_decay).max(0.0);
            }
            // Touches.
            let mine = team.copied().filter(|t| t.0 != 0);
            let half = Vec3::splat(p.size * U);
            for (v, vt, aabb, vstate, vh, vteam, burning, class) in &victims {
                if v == owner || vh.current <= 0.0 || (!ff && mine.is_some() && vteam.copied() == mine) {
                    continue;
                }
                let (min, max) = boxed(vt, aabb, vstate);
                let (min, max) = (min - half, max + half);
                let touching: Vec<FlamePoint> =
                    stream.points.points.iter().filter(|pt| segment_meets_box(pt.before, pt.at, min, max)).copied().collect();
                let Some(oldest) = touching.first().copied() else { continue };
                let entry = stream.heat.iter().position(|h| h.victim == v);
                let added: f32 = touching.iter().map(|pt| p.touch_heat((now - pt.born) as f32)).sum();
                if let Some(i) = entry {
                    stream.heat[i].heat += added;
                }
                let due = entry.is_none_or(|i| now - stream.heat[i].last_hit + 1e-6 >= p.hit_interval as f64);
                if !due {
                    continue;
                }
                let i = match entry {
                    Some(i) => {
                        stream.heat[i].last_hit = now;
                        i
                    }
                    None => {
                        stream.heat.push(Heat { victim: v, heat: 1.0, last_hit: now });
                        stream.heat.len() - 1
                    }
                };
                let heat = stream.heat[i].heat;
                let amount = round_points(p.hit_damage((now - oldest.born) as f32, oldest.life, heat) * HP);
                let c = (min + max) / 2.0;
                damage.write(Damage {
                    target: v,
                    attacker: Some(owner),
                    amount,
                    point: c,
                    dir: (oldest.at - oldest.before).normalize_or(Vec3::NEG_Z),
                    hitgroup: Hitgroup::Generic,
                    kind: DamageKind::Burn,
                    weapon: Some(weapon.id),
                    force: Vec3::ZERO,
                });
                // The burn: an enemy Pyro only looks it.
                let pyro = classes.as_ref().zip(class).is_some_and(|(t, c)| c.class(t).name == PYRO);
                let lit = if pyro {
                    Burning {
                        left: 0.0,
                        next: now + p.pyro_burn as f64,
                        attacker: Some(owner),
                        weapon: weapon.id,
                        damage: 0.0,
                        interval: p.burn_interval,
                        harmless: true,
                    }
                } else {
                    let old = burning.filter(|b| !b.harmless);
                    Burning {
                        left: p.burn_after(old.map(|b| b.left)),
                        next: old.map_or(now + p.burn_interval as f64, |b| b.next),
                        attacker: Some(owner),
                        weapon: weapon.id,
                        damage: p.burn_damage,
                        interval: p.burn_interval,
                        harmless: false,
                    }
                };
                commands.entity(v).insert(lit);
            }
            // Forgotten after a hit interval without a damaging hit.
            stream.heat.retain(|h| now - h.last_hit <= p.hit_interval as f64 + 1e-6 && victims.contains(h.victim));
        }
    }
}

/// Every machine that draws: the points again from the rounds it sees
/// (its own player's predicted the first time, others' as the server
/// sends them), moved by the frame's time (flames.md 1, "Client copy";
/// per-frame order: new points, then every point's update).
#[allow(clippy::type_complexity)]
pub fn shown_flames(
    time: Res<Time>,
    mut rounds: MessageReader<WeaponEvent>,
    mut remote: MessageReader<RemoteShot>,
    owners: Query<(&Inventory, Option<&Velocity>)>,
    mut guns: Query<(Entity, &Tf2Flame, &mut FlameShown)>,
    world: FlameWorld,
) {
    let now = time.elapsed_secs_f64();
    let dt = time.delta_secs();
    let mut add = |weapon: Entity, owner: Entity, eye: Vec3, yaw: f32, pitch: f32, seed: u32| {
        let Ok((_, f, mut shown)) = guns.get_mut(weapon) else { return };
        let pyro = owners.get(owner).ok().and_then(|(_, v)| v).map_or(Vec3::ZERO, |v| v.0);
        shown.points.round(&f.params, eye, yaw, pitch, seed, pyro, now, world.wet(eye));
    };
    for e in rounds.read() {
        if let WeaponEventKind::Fired { origin, yaw, pitch, seed, .. } = e.kind
            && !e.replay
        {
            add(e.weapon, e.owner, origin, yaw, pitch, seed);
        }
    }
    for r in remote.read() {
        let Some(w) = owners.get(r.shooter).ok().and_then(|(i, _)| i.active) else { continue };
        let pyro = owners.get(r.shooter).ok().and_then(|(_, v)| v).map_or(Vec3::ZERO, |v| v.0);
        let Ok((_, f, mut shown)) = guns.get_mut(w) else { continue };
        let round = RemoteRound { at: now, eye: r.origin, yaw: r.yaw, pitch: r.pitch, seed: r.seed, pyro };
        shown.remote.arrive(now, f.params.emit as f64, round);
    }
    for (_, f, mut shown) in &mut guns {
        for r in shown.remote.due(now) {
            let wet = world.wet(r.eye);
            shown.points.round(&f.params, r.eye, r.yaw, r.pitch, r.seed, r.pyro, now, wet);
        }
    }
    for (_, f, mut shown) in &mut guns {
        if shown.points.points.is_empty() && shown.points.last_round.is_none() {
            continue;
        }
        let half = f.params.half();
        shown.now = now;
        shown.points.update(&f.params, now, dt, &|from, motion| world.sweep(half, from, motion), &|at| world.wet(at));
    }
}

/// Afterburn (the server's).
pub fn afterburn(
    mut burning: Query<(Entity, &mut Burning, &Health, &Transform, Option<&SourceMovement>)>,
    clock: Res<SimClock>,
    mut damage: MessageWriter<Damage>,
    mut commands: Commands,
) {
    let now = clock.now;
    for (e, mut b, h, at, movement) in &mut burning {
        // Dead, or waist-deep in water: out.
        if h.current <= 0.0 || movement.is_some_and(|m| m.water_level >= 2) {
            commands.entity(e).remove::<Burning>();
            continue;
        }
        while b.next <= now + 1e-6 {
            if b.left <= 0.0 {
                commands.entity(e).remove::<Burning>();
                break;
            }
            damage.write(Damage {
                target: e,
                attacker: b.attacker,
                amount: b.damage * HP,
                point: at.translation,
                dir: Vec3::Y,
                hitgroup: Hitgroup::Generic,
                kind: DamageKind::Burn,
                weapon: Some(b.weapon),
                force: Vec3::ZERO,
            });
            b.left -= b.interval;
            b.next += b.interval as f64;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> FlameParams {
        super::super::hook_params(super::super::FLAMETHROWER, super::super::FLAME_HOOK)
    }

    /// 7.2: 13 fresh and at full heat, half cold, the age factor's floor
    /// 0.5 at half its life; at least 1.
    #[test]
    fn a_flame_hit_ranges_from_6_5_to_13() {
        let p = params();
        assert!((p.hit_damage(0.0, 0.6, 50.0) - 13.0).abs() < 1e-4);
        assert!((p.hit_damage(0.0, 0.6, 1.0) - 6.5).abs() < 1e-4);
        assert!((p.hit_damage(0.3, 0.6, 50.0) - 6.5).abs() < 1e-4);
        assert!((p.hit_damage(0.5, 0.6, 1.0) - 3.25).abs() < 1e-4);
        assert!((p.hit_damage(0.0, 0.6, 30.0) - 9.75).abs() < 1e-4);
        assert_eq!(p.touch_heat(0.0), 2.0);
        assert_eq!(p.touch_heat(0.05), 1.0);
    }

    /// 7.3, tests 44, 45: one hit burns 3.4 s; long contact caps at 10.
    #[test]
    fn the_burn_lasts_3_4_to_10_s_44_45() {
        let p = params();
        assert!((p.burn_after(None) - 3.4).abs() < 1e-5);
        assert!((p.burn_after(Some(3.4)) - 3.8).abs() < 1e-5);
        assert_eq!(p.burn_after(Some(9.9)), 10.0);
    }

    /// 7.1: a still Pyro's point flies 285 units in 0.5 s, 287 in 0.6 s.
    #[test]
    fn a_point_reaches_about_287_units() {
        let p = params();
        assert!((p.reach(33, 0.015) - 285.0).abs() < 1.5, "{}", p.reach(33, 0.015));
        assert!((p.reach(40, 0.015) - 287.0).abs() < 1.5, "{}", p.reach(40, 0.015));
    }

    /// flames.md test cases: a still Pyro's point, no spread, aim along
    /// -Z (engine; +x in the spec's): at 40 forward and 5 right at its
    /// spawn, after one update 76.75 forward and 0.75 up, at 2137.6.
    #[test]
    fn a_point_spawns_ahead_of_the_eye_and_moves_on_flames_1() {
        let p = FlameParams { spread: 0.0, ..params() };
        let mut pt = p.point(Vec3::ZERO, 0.0, 0.0, 7, Vec3::ZERO, 0.0, 0);
        assert!((pt.at - Vec3::new(5.0, 0.0, -40.0) * U).length() < 1e-5, "{:?}", pt.at / U);
        assert!((0.5..=0.7).contains(&pt.life));
        let clear = |_: Vec3, _: Vec3| Sweep::Clear;
        let dry = |_: Vec3| false;
        assert!(pt.update(&p, 0.0, 0.015, &clear, &dry));
        assert!((pt.at / U - Vec3::new(5.0, 0.75, -76.75)).length() < 1e-3, "{:?}", pt.at / U);
        assert!((pt.velocity.length() / U - 2137.625).abs() < 0.01);
        // 0.5 s on: about 285 forward of the start, 25 up.
        for k in 1..34 {
            assert!(pt.update(&p, k as f64 * 0.015, 0.015, &clear, &dry));
        }
        let moved = pt.at / U - Vec3::new(5.0, 0.0, -40.0);
        assert!((-moved.z - 285.0).abs() < 2.0 && (moved.y - 25.5).abs() < 1.0, "{moved:?}");
        // Its life over: gone.
        assert!(!pt.update(&p, pt.life as f64 + 0.001, 0.015, &clear, &dry));
    }

    /// flames.md test case: a point sliding on a wall of normal +Z (it
    /// flies -Z) drops the Pyro's part, keeps no velocity into it and
    /// gains 50 up an update before drag (its step velocity, drift
    /// included, becomes its velocity).
    #[test]
    fn a_point_slides_on_a_wall_and_rises_flames_1() {
        let p = FlameParams { spread: 0.0, ..params() };
        let mut pt = p.point(Vec3::ZERO, 0.0, 0.0, 7, Vec3::new(0.0, 0.0, -100.0) * U, 0.0, 0);
        assert!(pt.carried.length() > 0.0);
        let wall = |_: Vec3, _: Vec3| Sweep::Hit { fraction: 0.5, normal: Vec3::Z };
        let dry = |_: Vec3| false;
        assert!(pt.update(&p, 0.0, 0.015, &wall, &dry));
        assert_eq!(pt.carried, Vec3::ZERO);
        assert!(pt.velocity.z.abs() < 1e-6, "{:?}", pt.velocity);
        let up = pt.velocity.y / U;
        assert!((up - 50.0 * (1.0 - 8.5 * 0.015)).abs() < 1e-3, "{up}");
        assert_eq!(pt.wall_hits, 1);
        // Still on it: up by 50 more before the drag.
        assert!(pt.update(&p, 0.015, 0.015, &wall, &dry));
        let up2 = pt.velocity.y / U;
        assert!((up2 - (up + 50.0) * (1.0 - 8.5 * 0.015)).abs() < 1e-3, "{up2}");
        // Starting inside a solid, or in water: gone.
        let solid = |_: Vec3, _: Vec3| Sweep::StartSolid;
        assert!(!pt.clone().update(&p, 0.03, 0.015, &solid, &dry));
        assert!(!pt.update(&p, 0.03, 0.015, &|_, _| Sweep::Clear, &|_| true));
    }

    /// The same round makes the same point on every machine (flames.md
    /// 1: the seed), and different seeds spread differently.
    #[test]
    fn a_rounds_point_is_the_same_everywhere() {
        let p = params();
        let a = p.point(Vec3::ONE, 0.3, -0.1, 42, Vec3::X, 1.0, 3);
        assert_eq!(a, p.point(Vec3::ONE, 0.3, -0.1, 42, Vec3::X, 1.0, 3));
        assert_ne!(a.velocity, p.point(Vec3::ONE, 0.3, -0.1, 43, Vec3::X, 1.0, 3).velocity);
    }

    #[test]
    fn others_rounds_arriving_together_are_made_one_emit_apart() {
        let round = RemoteRound { at: 0.0, eye: Vec3::ZERO, yaw: 0.0, pitch: 0.0, seed: 1, pyro: Vec3::ZERO };
        let mut r = RemoteRounds::default();
        // Three in one update at 1.0: made at 1.0, 1.02 and 1.04.
        for _ in 0..3 {
            r.arrive(1.0, 0.02, round);
        }
        assert_eq!(r.due(1.0).len(), 1);
        assert_eq!(r.due(1.019).len(), 0);
        assert_eq!(r.due(1.02).len(), 1);
        assert_eq!(r.due(1.05).len(), 1);
        // A batch far behind isn't held more than REMOTE_HOLD_MAX.
        for _ in 0..10 {
            r.arrive(2.0, 0.02, round);
        }
        assert!(r.waiting.iter().all(|w| w.at - 2.0 <= REMOTE_HOLD_MAX + 1e-9));
        // A round after a pause is made when it arrives.
        r.due(10.0);
        r.arrive(10.0, 0.02, round);
        assert_eq!(r.due(10.0).len(), 1);
    }

    #[test]
    fn a_segment_meets_a_box() {
        let (min, max) = (Vec3::splat(-1.0), Vec3::splat(1.0));
        assert!(segment_meets_box(Vec3::new(-5.0, 0.0, 0.0), Vec3::new(5.0, 0.0, 0.0), min, max));
        assert!(!segment_meets_box(Vec3::new(-5.0, 2.0, 0.0), Vec3::new(5.0, 2.0, 0.0), min, max));
        assert!(!segment_meets_box(Vec3::new(-5.0, 0.0, 0.0), Vec3::new(-3.0, 0.0, 0.0), min, max));
        assert!(segment_meets_box(Vec3::new(0.5, 0.5, 0.5), Vec3::new(0.6, 0.5, 0.5), min, max));
    }
}
