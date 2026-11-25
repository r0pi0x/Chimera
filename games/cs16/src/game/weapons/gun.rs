//! Counter-Strike 1.6's gun rules (specs/cs16/weapons.md, "Firing"), the
//! `cs16:gun` hook every gun's data names (`GunParams`; the keys are
//! docs/weapon-data.md's), on Chimera's shared weapon frame:
//!
//! - **Spread** (`SpreadTable`): before the frame, the held gun's spread
//!   from its state, first matching row of in the air, moving (2D speed
//!   above a row's threshold, in order), ducked, standing: c + k × t,
//!   where t is the accuracy (automatic guns) or 1 − accuracy (pistols,
//!   the G3SG1 and SG550), plus the scoped guns' unzoomed terms
//!   (`unzoomed` after the scaling, `unzoomed_scaled` into it); the
//!   shot's direction is the template's two summed uniforms per axis
//!   (`SpreadShape::Template`). Mode 1 (silenced, burst) reads
//!   `alt_spread` when given.
//! - **Accuracy** (`Accuracy`): the automatic guns' shots count and
//!   f(shots) with the original's whole-number division (`whole`), the
//!   spray's accuracy left until the next shot (the quirk kept), the
//!   count capped at 15 and falling by one every 0.0225 s from 0.4 s
//!   after both buttons are let go; the pistols' accuracy by the time
//!   since the previous shot (`Time`); the G3SG1's and SG550's by the
//!   time since theirs, before or after the shot (`Since`). A shot's
//!   spread uses the accuracy left by the shot before; the update is
//!   after it. Deploy and reload restore the start values.
//! - **Kick** (`KickModel`): the automatic guns' seven-number rows by
//!   state (rifles: moving first, then air, ducked, standing; SMGs and
//!   the M249: air first), the side flag flipping with odds 1 / (D + 1)
//!   from the command's shared random numbers; the pistols' and bolt
//!   snipers' 2° up; the G3SG1's and SG550's random kick, less when
//!   already kicked; the shotguns' whole degrees. The punch decays by
//!   (10 + 0.5 L) °/s while a CS 1.6 gun owns it
//!   (`weapon::decay_punch`, `impact::RecoilGame`).
//! - **Modes**: the silencers' own damage (`alt_damage`), the bursts'
//!   follow-ups (`follow_ups`: the Glock's fixed spread on the next
//!   commands, the FAMAS's first round's spread; their damage; no kick,
//!   not counted as shots), the FAMAS burst's first round (`burst_first`),
//!   a held Glock in burst mode bursting every 0.5 s (`held_bursts`:
//!   `AutoMode`).
//! - **Penetration**: the data's `Penetration` (count, the bullet type's
//!   power and distance) as GoldSrc's power rule
//!   (`weapon::PowerPenetration`, damage.md "Wall penetration") with the
//!   game's material table (data/penetration.toml).
//! - **Shotguns** (`pellet_range`): each target's pellets by
//!   20 × (1 − distance / range) (`DamageEffect::by_distance`), no
//!   penetration.
//!
//! Known shortcuts:
//! - A shotgun's pellets are summed per target before the distance rule
//!   (the original truncates each pellet), the distance between the
//!   shooter's and target's centres; their directions come from the
//!   command's seed, not the server's own random numbers.
//! - The kick's side flips and the random kicks use the command's shared
//!   random numbers (`weapon::random::shared_random`), not the original's
//!   generator.
//! - The Elites' bullets start at the eye, not 5 units to the firing
//!   gun's side.
//! - Friendly fire's × 0.35 on teammates (damage.md, "Teams") is not
//!   applied.

use std::sync::Arc;

use bevy::prelude::*;
use serde::Deserialize;

use chimera_sim::weapon::{
    AltModes, AutoMode, Burst, DamageEffect, Hitscan, Inventory, NetPart, Penetration, PowerPenetration, SpreadShape,
    ViewPunch, WeaponEvent, WeaponEventKind, WeaponState, Zoom, decay_punch, impact::RecoilGame,
};

use crate::core::{Intent, MovementState, Velocity};

/// The hook's name.
pub const HOOK: &str = "cs16:gun";

/// GoldSrc's unit in metres.
const UNIT: f32 = super::super::METERS_PER_UNIT;

/// Faster than this (units/s) counts as moving where a row's threshold
/// is 0 (the original's "above 0"; float noise at rest left out).
const STILL: f32 = 0.01;

/// The shots count's recovery (weapons.md, "Automatic guns"): capped at
/// this, falling by one every `RECOVER_STEP` from `RECOVER_AFTER` after
/// the release.
const RECOVER_CAP: u32 = 15;
const RECOVER_AFTER: f64 = 0.4;
const RECOVER_STEP: f64 = 0.0225;

/// `cs16:gun`'s parameters (module doc).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GunParams {
    /// None: no accuracy (the shotguns' fixed pellet spread).
    #[serde(default)]
    pub accuracy: Option<Accuracy>,
    /// None: the data's own spread stays (the shotguns).
    #[serde(default)]
    pub spread: Option<SpreadTable>,
    /// In mode 1 (silenced, burst), when it differs.
    #[serde(default)]
    pub alt_spread: Option<SpreadTable>,
    /// None: no punch (the Glock).
    #[serde(default)]
    pub kick: Option<KickModel>,
    /// The `table` kick's rows.
    #[serde(default)]
    pub kick_rows: Option<KickRows>,
    /// Damage and range modifier in mode 1 (a silencer on).
    #[serde(default)]
    pub alt_damage: Option<DamageRow>,
    /// A burst's second and third rounds.
    #[serde(default)]
    pub follow_ups: Option<FollowUps>,
    /// A burst's first round's damage (the FAMAS's 34).
    #[serde(default)]
    pub burst_first: Option<f32>,
    /// Held in burst mode, a burst every refire (no fresh press).
    #[serde(default)]
    pub held_bursts: bool,
    /// The shotguns: each pellet by 20 × (1 − distance / this), units.
    #[serde(default)]
    pub pellet_range: Option<f32>,
    /// The Elites: shots alternate between the guns (the view model).
    #[serde(default)]
    pub alternate_hands: bool,
}

/// How a gun's accuracy moves (weapons.md, "Automatic guns", "Pistols",
/// "Snipers").
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(tag = "model", rename_all = "snake_case", deny_unknown_fields)]
pub enum Accuracy {
    /// After each shot: offset + n^power / divisor (floored when
    /// `whole`), at most `cap`; `start` on deploy, `reload_start` (else
    /// `start`) on a reload.
    Shots {
        start: f32,
        divisor: f32,
        power: f32,
        offset: f32,
        cap: f32,
        #[serde(default = "yes")]
        whole: bool,
        #[serde(default)]
        reload_start: Option<f32>,
    },
    /// After a shot with one before it: a − (period − Δt) × rate, kept
    /// within [low, start]; `start` on deploy and reload.
    Time { start: f32, period: f32, rate: f32, low: f32 },
    /// base + rate × Δt since the previous shot, at most `cap`: before the
    /// shot (`before`, the G3SG1: `start` with none before) or after it
    /// (the SG550: no change on the first, `start` until then).
    Since { base: f32, rate: f32, cap: f32, before: bool, start: f32 },
    /// No accuracy (the AWP and Scout: fixed spreads).
    Fixed,
}

fn yes() -> bool {
    true
}

impl Accuracy {
    /// The value on deploy (`reload`: on a reload).
    pub fn start(&self, reload: bool) -> f32 {
        match *self {
            Accuracy::Shots { start, reload_start, .. } => if reload { reload_start.unwrap_or(start) } else { start },
            Accuracy::Time { start, .. } | Accuracy::Since { start, .. } => start,
            Accuracy::Fixed => 0.0,
        }
    }

    /// The spread term the accuracy gives a table's k (module doc).
    pub fn term(&self, a: f32) -> f32 {
        match self {
            Accuracy::Shots { .. } => a,
            Accuracy::Time { .. } | Accuracy::Since { .. } => 1.0 - a,
            Accuracy::Fixed => 0.0,
        }
    }

    /// An automatic gun's accuracy after its `n`-th shot.
    pub fn of_shots(&self, n: u32) -> Option<f32> {
        let Accuracy::Shots { divisor, power, offset, cap, whole, .. } = *self else { return None };
        let raised = (n as f64).powi(power.round() as i32);
        let raised = if (power - power.round()).abs() > 1e-6 { (n as f64).powf(power as f64) } else { raised };
        let step = raised / divisor as f64;
        let step = if whole { step.floor() } else { step };
        Some((offset as f64 + step).min(cap as f64) as f32)
    }
}

/// Spread by state: [c, k] rows, `moving` as [threshold units/s, c, k]
/// (the first whose threshold the 2D speed is above), `ducked` none: the
/// standing row; `unzoomed` added after, `unzoomed_scaled` to k, while a
/// scoped gun is not zoomed.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SpreadTable {
    pub air: [f32; 2],
    #[serde(default)]
    pub moving: Vec<[f32; 3]>,
    #[serde(default)]
    pub ducked: Option<[f32; 2]>,
    pub still: [f32; 2],
    #[serde(default)]
    pub unzoomed: f32,
    #[serde(default)]
    pub unzoomed_scaled: f32,
}

/// What a shooter is doing, for the spread and the kick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stance {
    pub air: bool,
    /// 2D speed, units/s.
    pub speed: f32,
    pub ducked: bool,
    /// A scoped gun zoomed (any gun without a scope counts as zoomed).
    pub zoomed: bool,
}

impl SpreadTable {
    /// The spread for `term` (`Accuracy::term`) in `s`.
    pub fn spread(&self, term: f32, s: Stance) -> f32 {
        let [c, k] = if s.air {
            self.air
        } else if let Some(m) = self.moving.iter().find(|m| s.speed > m[0].max(STILL)) {
            [m[1], m[2]]
        } else if let (true, Some(d)) = (s.ducked, self.ducked) {
            d
        } else {
            self.still
        };
        let (add, scaled) = if s.zoomed { (0.0, 0.0) } else { (self.unzoomed, self.unzoomed_scaled) };
        c + (k + scaled) * term + add
    }
}

/// A burst's follow-up rounds: a fixed spread (none: the first round's),
/// their damage and range modifier.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FollowUps {
    #[serde(default)]
    pub spread: Option<f32>,
    pub amount: f32,
    pub falloff: f32,
}

/// A damage and range modifier.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DamageRow {
    pub amount: f32,
    pub falloff: f32,
}

/// The kick (weapons.md, "Recoil: the kick function").
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(tag = "model", rename_all = "snake_case", deny_unknown_fields)]
pub enum KickModel {
    /// `kick_rows` in the order of `order`.
    Table { order: KickOrder },
    /// Straight up this many degrees a shot, no cap.
    Up { pitch: f32 },
    /// Up U(up) less `lift` × the punch already up, sideways U(±side).
    Random { up: [f32; 2], lift: f32, side: f32 },
    /// Up a whole number of degrees in [lo, hi], `air` off the ground.
    Whole { ground: [f32; 2], air: [f32; 2] },
}

/// Which state's row a `Table` kick checks first.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KickOrder {
    /// Moving (2D speed above 0), then in the air, ducked, standing.
    Rifle,
    /// In the air, then moving, ducked, standing.
    Smg,
}

/// The table kick's rows: [up base, side base, up per shot, side per
/// shot, up cap, side cap, flip odds D] (degrees).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct KickRows {
    pub moving: [f32; 7],
    pub air: [f32; 7],
    pub ducked: [f32; 7],
    pub standing: [f32; 7],
}

impl KickRows {
    /// The row for `s` in `order`.
    pub fn row(&self, order: KickOrder, s: Stance) -> [f32; 7] {
        let moving = s.speed > STILL;
        match order {
            KickOrder::Rifle if moving => self.moving,
            KickOrder::Rifle if s.air => self.air,
            KickOrder::Smg if s.air => self.air,
            KickOrder::Smg if moving => self.moving,
            _ if s.ducked => self.ducked,
            _ => self.standing,
        }
    }
}

/// A CS 1.6 gun's state (module doc): its parameters, its data's damage,
/// the accuracy, shots count, last shot and the recovery's clock, the
/// kick's side flag, a burst's first spread and whether this tick's round
/// is a burst's follow-up.
#[derive(Component, Clone, Debug)]
pub struct Cs16Gun {
    pub params: Arc<GunParams>,
    pub amount: f32,
    pub falloff: f32,
    pub accuracy: f32,
    pub shots: u32,
    pub last_fire: Option<f64>,
    /// When both buttons were let go (None: one is held).
    pub released: Option<f64>,
    pub next_decay: f64,
    /// The kick's side flag (false: the punch goes right).
    pub side: bool,
    pub burst_spread: Option<f32>,
    pub follow_up: bool,
}

impl Cs16Gun {
    pub fn new(params: GunParams, amount: f32, falloff: f32) -> Self {
        let accuracy = params.accuracy.map_or(0.0, |a| a.start(false));
        Self {
            params: Arc::new(params),
            amount,
            falloff,
            accuracy,
            shots: 0,
            last_fire: None,
            released: None,
            next_decay: 0.0,
            side: false,
            burst_spread: None,
            follow_up: false,
        }
    }

    /// The spread of a shot now (module doc), from the accuracy left by
    /// the last one; None: the data's own (no table).
    pub fn spread(&self, stance: Stance, mode: u8, now: f64) -> Option<f32> {
        let table = match (mode, &self.params.alt_spread) {
            (1.., Some(alt)) => alt,
            _ => self.params.spread.as_ref()?,
        };
        let accuracy = self.params.accuracy.unwrap_or(Accuracy::Fixed);
        let a = match accuracy {
            Accuracy::Since { base, rate, cap, before: true, start } => {
                self.last_fire.map_or(start, |t| (base + rate * (now - t) as f32).min(cap))
            }
            _ => self.accuracy,
        };
        Some(table.spread(accuracy.term(a), stance))
    }

    /// The accuracy after a shot at `now` (the shots count already
    /// counted).
    fn after_shot(&mut self, now: f64) {
        match self.params.accuracy {
            Some(a @ Accuracy::Shots { .. }) => self.accuracy = a.of_shots(self.shots).unwrap_or(self.accuracy),
            Some(Accuracy::Time { start, period, rate, low }) => {
                if let Some(t) = self.last_fire {
                    let dt = (now - t) as f32;
                    self.accuracy = (self.accuracy - (period - dt) * rate).clamp(low, start);
                }
            }
            Some(Accuracy::Since { base, rate, cap, before: false, .. }) => {
                if let Some(t) = self.last_fire {
                    self.accuracy = (base + rate * (now - t) as f32).min(cap);
                }
            }
            _ => {}
        }
        self.last_fire = Some(now);
    }

    /// Back to the start (a deploy, or a `reload`).
    fn reset(&mut self, reload: bool) {
        self.shots = 0;
        if let Some(a) = self.params.accuracy {
            self.accuracy = a.start(reload);
        }
        self.burst_spread = None;
    }
}

impl NetPart for Cs16Gun {
    /// Accuracy, shots, last shot, release, next decay, side flag, the
    /// burst's spread.
    type Net = (f32, u32, Option<f64>, Option<f64>, f64, bool, Option<f32>);
    fn net(&self) -> Self::Net {
        (self.accuracy, self.shots, self.last_fire, self.released, self.next_decay, self.side, self.burst_spread)
    }
    fn set_net(&mut self, n: Self::Net) {
        (self.accuracy, self.shots, self.last_fire, self.released, self.next_decay, self.side, self.burst_spread) = n;
    }
}

/// Damage by distance for the shotguns: each target's pellets' sum ×
/// (1 − distance / range), none past it (`DamageEffect::by_distance`;
/// `param` the range, m).
pub fn pellet_distance(sum: f32, distance: f32, range: f32) -> f32 {
    if range <= 0.0 {
        return sum;
    }
    sum * (1.0 - distance / range).max(0.0)
}

/// The hook: the gun's state, GoldSrc's penetration, the shotguns' rule.
pub fn insert(e: &mut EntityWorldMut, p: &GunParams) {
    let (amount, falloff) = e.get::<DamageEffect>().map_or((0.0, 1.0), |d| (d.amount, d.falloff));
    if let Some(range) = p.pellet_range
        && let Some(mut d) = e.get_mut::<DamageEffect>()
    {
        d.by_distance = Some(chimera_sim::weapon::ByDistance { rule: pellet_distance, param: range * UNIT });
    }
    if let Some(pen) = e.get::<Penetration>().cloned() {
        e.remove::<Penetration>();
        if pen.objects > 0 && pen.power > 0.0 {
            e.insert(PowerPenetration {
                count: pen.objects,
                power: pen.power,
                max_distance: pen.max_distance,
                rules: e
                    .world()
                    .get_resource::<super::penetration::Cs16Penetration>()
                    .map_or_else(super::penetration::rules, |r| r.0.clone()),
            });
        }
    }
    if p.held_bursts {
        e.insert(AutoMode(1));
    }
    e.insert(Cs16Gun::new(p.clone(), amount, falloff));
}

/// The shooter's stance for a gun.
fn stance(state: &MovementState, velocity: &Velocity, zoomed: bool) -> Stance {
    Stance {
        air: !state.on_ground,
        speed: Vec2::new(velocity.0.x, velocity.0.z).length() / UNIT,
        ducked: state.crouching,
        zoomed,
    }
}

/// Before the weapon frame: the punch decays, the shots count recovers,
/// and the held gun's spread and damage are set for this tick's round.
#[allow(clippy::type_complexity)]
pub fn before_frame(
    mut owners: Query<
        (&Inventory, &MovementState, &Velocity, &Intent, Option<&mut ViewPunch>, Option<&RecoilGame>),
        Without<crate::map::interp::NetDrawn>,
    >,
    mut guns: Query<(
        &mut Cs16Gun,
        &mut Hitscan,
        Option<&mut DamageEffect>,
        &WeaponState,
        Option<&AltModes>,
        Option<&Zoom>,
        Option<&Burst>,
    )>,
    clock: Res<crate::core::SimClock>,
) {
    let (now, dt) = (clock.now, clock.dt());
    for (inv, state, velocity, intent, punch, recoil) in &mut owners {
        let ours = RecoilGame::is(recoil, super::super::GAME) || inv.active.is_some_and(|a| guns.contains(a));
        if let Some(mut p) = punch.filter(|_| ours) {
            let next = decay_punch(p.0, dt);
            if p.0 != next {
                p.0 = next;
            }
        }
        let Some(active) = inv.active else { continue };
        let Ok((mut gun, mut scan, effect, st, modes, zoom, burst)) = guns.get_mut(active) else { continue };
        // The shots count's recovery, both buttons up.
        if intent.fire || intent.secondary {
            gun.released = None;
        } else if matches!(gun.params.accuracy, Some(Accuracy::Shots { .. })) {
            if gun.released.is_none() {
                gun.released = Some(now);
                gun.next_decay = now + RECOVER_AFTER;
                gun.shots = gun.shots.min(RECOVER_CAP);
            }
            while gun.shots > 0 && now + 1e-6 >= gun.next_decay {
                gun.shots -= 1;
                gun.next_decay += RECOVER_STEP;
            }
        }
        let mode = modes.map_or(0, |m| m.current);
        let zoomed = zoom.is_none() || mode > 0;
        let s = stance(state, velocity, zoomed);
        // This tick's round: a burst's follow-up (the frame fires it
        // first), else the trigger's.
        let follow_up = st.burst_left > 0 && st.next_burst_round <= now + 1e-5;
        if gun.follow_up != follow_up {
            gun.follow_up = follow_up;
        }
        let bursting = burst.is_some_and(|b| b.mode == mode && b.count > 1);
        let (spread, amount, falloff) = if follow_up && let Some(f) = gun.params.follow_ups {
            (f.spread.or(gun.burst_spread), f.amount, f.falloff)
        } else {
            let (amount, falloff) = match (bursting, gun.params.burst_first, gun.params.alt_damage) {
                (true, Some(first), _) => (first, gun.falloff),
                (_, _, Some(alt)) if mode > 0 => (alt.amount, alt.falloff),
                _ => (gun.amount, gun.falloff),
            };
            (gun.spread(s, mode, now), amount, falloff)
        };
        if let Some(s) = spread {
            let shape = SpreadShape::Template(s);
            if scan.spread != shape {
                scan.spread = shape;
            }
        }
        if let Some(mut d) = effect
            && (d.amount != amount || d.falloff != falloff)
        {
            d.amount = amount;
            d.falloff = falloff;
        }
    }
}

/// After the weapon frame: each round fired updates its gun's accuracy
/// and kicks the view (module doc); a deploy or reload resets them.
#[allow(clippy::type_complexity)]
pub fn after_frame(
    mut events: MessageReader<WeaponEvent>,
    mut guns: Query<(&mut Cs16Gun, Option<&Burst>, Option<&AltModes>)>,
    mut owners: Query<(&MovementState, &Velocity, &Intent, Option<&mut ViewPunch>)>,
    mut commands: Commands,
    clock: Res<crate::core::SimClock>,
) {
    let now = clock.now;
    for e in events.read() {
        let Ok((mut gun, burst, modes)) = guns.get_mut(e.weapon) else { continue };
        let (mode, spread) = match &e.kind {
            WeaponEventKind::Deployed => {
                gun.reset(false);
                continue;
            }
            WeaponEventKind::ReloadStarted => {
                gun.reset(true);
                continue;
            }
            WeaponEventKind::Fired { mode, spread, .. } => (*mode, *spread),
            _ => continue,
        };
        if gun.follow_up {
            continue;
        }
        let bursting = burst.is_some_and(|b| b.mode == mode && b.count > 1);
        if bursting {
            gun.burst_spread = match spread {
                SpreadShape::Template(s) => Some(s),
                _ => None,
            };
        }
        gun.shots += 1;
        gun.after_shot(now);
        let _ = modes;
        let Some(kick) = gun.params.kick else { continue };
        let Ok((state, velocity, intent, punch)) = owners.get_mut(e.owner) else { continue };
        let s = stance(state, velocity, true);
        let old = punch.as_ref().map_or(Vec3::ZERO, |p| p.0);
        let mut dice = chimera_sim::weapon::random::shared_random(intent.command, "cs16:kick", gun.shots as i32);
        let (mut pitch, mut yaw) = (old.x.to_degrees(), old.y.to_degrees());
        match kick {
            KickModel::Table { order } => {
                let Some(rows) = gun.params.kick_rows else { continue };
                let [ub, lb, um, lm, umax, lmax, d] = rows.row(order, s);
                let n = gun.shots as f32;
                let (up, side) = if gun.shots == 1 { (ub, lb) } else { (ub + n * um, lb + n * lm) };
                pitch = (pitch + up).min(umax);
                yaw = if gun.side { (yaw + side).min(lmax) } else { (yaw - side).max(-lmax) };
                if dice.next() < 1.0 / (d + 1.0) {
                    gun.side = !gun.side;
                }
            }
            KickModel::Up { pitch: up } => pitch += up,
            KickModel::Random { up, lift, side } => {
                let u = dice.range(up[0], up[1]);
                pitch += u - lift * pitch;
                yaw += dice.range(-side, side);
            }
            KickModel::Whole { ground, air } => {
                let [lo, hi] = if s.air { air } else { ground };
                pitch += (lo + (dice.next() * (hi - lo + 1.0)).floor()).min(hi);
            }
        }
        let kicked = Vec3::new(pitch.to_radians(), yaw.to_radians(), old.z);
        match punch {
            Some(mut p) => p.0 = kicked,
            None => {
                commands.entity(e.owner).insert(ViewPunch(kicked));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ak() -> Accuracy {
        Accuracy::Shots { start: 0.2, divisor: 200.0, power: 3.0, offset: 0.35, cap: 1.25, whole: true, reload_start: None }
    }

    /// The AK-47's whole-number steps: 0.35 for shots 1-5, capped 1.25
    /// from the sixth (weapons.md's test cases).
    #[test]
    fn whole_number_division_steps() {
        let a = ak();
        for n in 1..=5 {
            assert_eq!(a.of_shots(n), Some(0.35), "shot {n}");
        }
        assert_eq!(a.of_shots(6), Some(1.25));
        let mp5 = Accuracy::Shots { start: 0.0, divisor: 220.1, power: 2.0, offset: 0.45, cap: 0.75, whole: false, reload_start: None };
        assert!((mp5.of_shots(3).unwrap() - (9.0 / 220.1 + 0.45)).abs() < 1e-6);
    }

    #[test]
    fn spread_rows_in_order() {
        let t = SpreadTable { air: [0.04, 0.4], moving: vec![[140.0, 0.04, 0.07]], ducked: None, still: [0.0, 0.0275], unzoomed: 0.0, unzoomed_scaled: 0.0 };
        let still = Stance { zoomed: true, ..default() };
        assert!((t.spread(0.2, still) - 0.0055).abs() < 1e-7);
        assert!((t.spread(0.2, Stance { speed: 100.0, ..still }) - 0.0055).abs() < 1e-7, "140 and under is standing");
        assert!((t.spread(0.2, Stance { speed: 200.0, ..still }) - (0.04 + 0.014)).abs() < 1e-7);
        assert!((t.spread(0.2, Stance { air: true, ..still }) - (0.04 + 0.08)).abs() < 1e-7);
        let awp = SpreadTable { air: [0.85, 0.0], moving: vec![[140.0, 0.25, 0.0], [10.0, 0.1, 0.0]], ducked: Some([0.0, 0.0]), still: [0.001, 0.0], unzoomed: 0.08, unzoomed_scaled: 0.0 };
        assert!((awp.spread(0.0, Stance::default()) - 0.081).abs() < 1e-7, "unzoomed standing");
        assert_eq!(awp.spread(0.0, Stance { ducked: true, zoomed: true, ..default() }), 0.0);
        assert!((awp.spread(0.0, Stance { speed: 50.0, zoomed: true, ..default() }) - 0.1).abs() < 1e-7);
    }

    /// The Deagle's case (weapons.md): two shots 0.225 s apart.
    #[test]
    fn pistol_accuracy_by_time() {
        let p = GunParams {
            accuracy: Some(Accuracy::Time { start: 0.9, period: 0.4, rate: 0.35, low: 0.55 }),
            spread: Some(SpreadTable { air: [0.0, 1.5], moving: vec![[0.0, 0.0, 0.25]], ducked: Some([0.0, 0.115]), still: [0.0, 0.13], unzoomed: 0.0, unzoomed_scaled: 0.0 }),
            alt_spread: None,
            kick: None,
            kick_rows: None,
            alt_damage: None,
            follow_ups: None,
            burst_first: None,
            held_bursts: false,
            pellet_range: None,
            alternate_hands: false,
        };
        let mut g = Cs16Gun::new(p, 54.0, 0.81);
        let s = Stance { zoomed: true, ..default() };
        assert!((g.spread(s, 0, 0.0).unwrap() - 0.013).abs() < 1e-6);
        g.after_shot(0.0);
        assert!((g.spread(s, 0, 0.225).unwrap() - 0.013).abs() < 1e-6, "the second shot uses the first's value");
        g.after_shot(0.225);
        assert!((g.accuracy - 0.83875).abs() < 1e-6, "{}", g.accuracy);
        assert!((g.spread(s, 0, 0.45).unwrap() - 0.13 * (1.0 - 0.83875)).abs() < 1e-6);
    }

    #[test]
    fn pellets_fall_off_linearly() {
        assert_eq!(pellet_distance(180.0, 0.0, 76.2), 180.0);
        assert!((pellet_distance(180.0, 38.1, 76.2) - 90.0).abs() < 1e-4);
        assert_eq!(pellet_distance(180.0, 80.0, 76.2), 0.0);
    }
}
