use std::{
    collections::HashMap,
    sync::{LazyLock, OnceLock},
};

use avian3d::prelude::*;
use bevy::prelude::*;
use chimera_core::{character::CharacterPartsExt, later::Later};
use chimera_sim::server_docs::DocsAppExt;
use serde::{Deserialize, Serialize};

use crate::{
    core::{
        Damage, DamageKind, DamageRules, Damageable, FirstTimePredicted, Health, Hitgroup, Intent, MovementState, NetRole,
        Predict, PredictedAppExt, PropSurface, SOLID_LAYERS, SimClock, Velocity,
    },
    map::{SurfaceNames, sound::SurfaceGrid},
    weapon::{
        AltModes, Burst, CarriedAppExt, Hitscan, ParamsAppExt, Inventory, NetPart, SpreadShape, ViewBackPush, ViewPunch,
        WeaponEvent,
        Armor, ArmorPieces, WeaponEventKind, WeaponFrame, Zoom, aim_of,
        hits,
        data::{WeaponData, WeaponDataAppExt},
        lagcomp,
        random::{Rng, shared_random},
    },
};

use crate::game::characters;
use crate::game::attachments::{CaNpc, CaSpecialRoll, DamageMods, RecoilMods, SpecialKind, SpreadMods, Victim};

pub use self::{
    classes::{ALT_FIRE, CaAltFire, CaAltVolley, CaSpin, CarryOver, PelletCarryOver, SPIN, SpinParams, SpinState, reported_hits},
    zoom::{CaFocusBlur, CaZoom, CaZoomSound, Scope, Sway, ZOOM, ZoomParams},
};

use super::*;

// ---------------------------------------------------------------------------
// Parts

/// A weapon's damage (`combat_arms:damage`), in engine units.
#[derive(Component, Clone, Debug)]
pub struct CaDamage {
    /// Per ray, health (1.0 = 100 HP).
    pub damage: f32,
    pub damage_type: i32,
    /// The random roll's half-width (already 0.2 when the data's was
    /// outside [0, 1]).
    pub roll: f32,
    /// (upper edge in metres, multiplier), three bands; none: no banding.
    pub bands: Option<[(f32, f32); 3]>,
    /// The server's spacing, s.
    pub fire_delay: f64,
    /// Metres per the data's length unit.
    pub unit: f32,
    /// When the last round that passed the server's spacing arrived (for
    /// `fire_delay`).
    pub last_round: Option<f64>,
    /// The knock-back on an infected victim: K and its duration (s).
    pub move_penalty: f32,
    pub move_penalty_duration: f32,
    /// The fitted attachments' terms (`attachments`).
    pub mods: DamageMods,
    /// The third-person reload's length at rate 1, s.
    pub reload_third_person: f32,
    /// The server's reload window ends then (weapon_classes.md
    /// Corrections 2): shots before it are dropped.
    pub reload_mark: Option<f64>,
}

/// The server's reload window (weapon_classes.md 5.6, Corrections 2): the
/// third-person reload `length` (s) / (min(`rate`, 2) x (2 -
/// `ReloadCheckTime`)); the rate is 1 + the reload bonuses, capped at 2
/// (not the client's "above 2 plays at 1").
pub fn server_reload_mark(length: f64, rate: f64, check: f64) -> f64 {
    length / (rate.min(2.0) * (2.0 - check))
}

/// The server's settings firing reads (attribute_data.md Corrections 3):
/// `ReloadCheckTime` (the install's `[TimeWeight]`, else 0.8).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct CaServerSettings {
    pub reload_check_time: f64,
}

impl CaServerSettings {
    /// The settings `data` gives.
    pub fn of(data: &crate::game::data::CaData) -> Self {
        Self {
            reload_check_time: data.reload_check_time(),
        }
    }
}

impl CaDamage {
    pub fn new(p: &DamageParams, unit: f32, health_per_point: f32) -> Self {
        let bands = (p.effect_range.len() >= 3 && p.range_damage.len() >= 3).then(|| {
            std::array::from_fn(|i| (p.effect_range[i].trunc() * unit, p.range_damage[i]))
        });
        Self {
            damage: p.damage * health_per_point,
            damage_type: p.damage_type,
            roll: if (0.0..=1.0).contains(&p.roll) { p.roll } else { DAMAGE_ROLL_INVALID },
            bands,
            fire_delay: p.fire_delay.max(0.0) as f64 / 1000.0,
            unit,
            last_round: None,
            move_penalty: p.move_penalty,
            move_penalty_duration: p.move_penalty_duration,
            mods: DamageMods::default(),
            reload_third_person: p.reload_third_person.unwrap_or(0.0),
            reload_mark: None,
        }
    }
}

/// Which values a weapon's spread and recoil use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FireMode {
    Normal,
    Zoomed,
    Burst,
}

/// The mode of a weapon from its attack2 mode (`AltModes`): burst when its
/// `Burst` is on, zoomed in a scope's modes.
pub fn fire_mode(mode: u8, burst: Option<&Burst>, zoom: Option<&Zoom>) -> FireMode {
    match (burst, zoom) {
        (Some(b), _) if b.mode == mode && b.count > 0 => FireMode::Burst,
        (_, Some(_)) if mode > 0 => FireMode::Zoomed,
        _ => FireMode::Normal,
    }
}

/// `fire_mode`, zoomed whenever Combat Arms' own zoom is (`CaZoom::zoomed`:
/// a level or a transition, weapon_classes.md 7).
pub fn mode_of(mode: u8, burst: Option<&Burst>, zoom: Option<&Zoom>, ca_zoom: Option<&CaZoom>) -> FireMode {
    if ca_zoom.is_some_and(CaZoom::zoomed) {
        FireMode::Zoomed
    } else {
        fire_mode(mode, burst, zoom)
    }
}

/// A weapon's spread (`combat_arms:spread`): the data and the two meters.
#[derive(Component, Clone, Debug)]
pub struct CaSpread {
    pub params: SpreadParams,
    /// Metres per the data's length unit (for speeds).
    pub unit: f32,
    /// The firing meter F and the movement meter M.
    pub fire: f32,
    pub movement: f32,
    /// When the last shot stamped the firing meter.
    pub last_shot: Option<f64>,
    /// Time not yet stepped, s.
    pub clock: f64,
    /// The body's (pitch, yaw) at the last tick (pitch: 0, never measured).
    pub prev_view: Option<Vec2>,
    /// The body yaw from the mouse (without the side kick): it lags the
    /// view by the upper body's twist.
    pub body_yaw: Option<f32>,
    /// Airborne after a jump (until landing).
    pub jumped: bool,
    pub was_on_ground: bool,
    /// The fitted attachments' terms (`attachments`; zero: stock).
    pub mods: SpreadMods,
    /// The perks' and sets' cuts of the cone's limits (`perks`, `gear`):
    /// Eagle Eye's fraction of both, a set's fraction of the maximum.
    pub perk_cut: f32,
    pub set_max_cut: f32,
}

impl CaSpread {
    pub fn new(params: SpreadParams, unit: f32) -> Self {
        Self {
            params,
            unit,
            fire: 0.0,
            movement: 0.0,
            last_shot: None,
            clock: 0.0,
            prev_view: None,
            body_yaw: None,
            jumped: false,
            was_on_ground: true,
            mods: SpreadMods::default(),
            perk_cut: 0.0,
            set_max_cut: 0.0,
        }
    }

    /// The cone limits and speeds in `mode` (zoomed ones where non-zero).
    pub fn cone_params(&self, mode: FireMode) -> ConeParams {
        let base = self.params.cone;
        match mode {
            FireMode::Normal => base,
            // Each burst field replaces its plain one when non-zero
            // (Corrections 12).
            FireMode::Burst => match self.params.burst_cone {
                Some(z) => ConeParams {
                    min: if z.min != 0 { z.min } else { base.min },
                    max: if z.max != 0 { z.max } else { base.max },
                    inc: if z.inc != 0.0 { z.inc } else { base.inc },
                    dec: if z.dec != 0.0 { z.dec } else { base.dec },
                    zoomed: false,
                },
                None => base,
            },
            FireMode::Zoomed => match self.params.zoomed_cone {
                Some(z) => ConeParams {
                    min: if z.min != 0 { z.min } else { base.min },
                    max: if z.max != 0 { z.max } else { base.max },
                    inc: if z.inc != 0.0 { z.inc } else { base.inc },
                    dec: if z.dec != 0.0 { z.dec } else { base.dec },
                    zoomed: true,
                },
                None => base,
            },
        }
    }

    /// The state values in `mode` (zoomed ones where non-zero, alone),
    /// with the attachments' terms (attachments.md, "Spread"): the walk
    /// and run targets and the still firing caps (standing, crouched) add
    /// theirs; jumping and the moving caps have none.
    pub fn states(&self, mode: FireMode) -> StateParams {
        let (b, z, m) = (self.params.state, self.params.zoomed_state, self.mods);
        let zoomed = mode == FireMode::Zoomed;
        let pick = |z: f32, b: f32, add: f32| if zoomed && z != 0.0 { z } else { b + add };
        StateParams {
            fire: pick(z.fire, b.fire, m.fire),
            fire_move: pick(z.fire_move, b.fire_move, 0.0),
            fire_duck: pick(z.fire_duck, b.fire_duck, m.fire_duck),
            fire_move_duck: pick(z.fire_move_duck, b.fire_move_duck, 0.0),
            walk: pick(z.walk, b.walk, m.walk),
            run: pick(z.run, b.run, m.run),
            jump: pick(z.jump, b.jump, 0.0),
        }
    }

    /// The spread fraction p = max(F, M) (0..1) a shot fired now uses: the
    /// HUD's r.
    pub fn fraction(&self) -> f32 {
        self.fire.max(self.movement)
    }

    /// The cone `n` (thousandths) a shot fired now gets in `mode`, from
    /// the meters as they stand: trunc(n_min + floor((n_max - n_min) p) g),
    /// each limit truncated after multiplying by g (weapon_classes.md,
    /// "Cone integers"; Q7: g the 32-bit value, the products in double
    /// precision, so 50 x 1.3 is 64).
    /// With attachments (attachments.md, "Cone"): g adds their
    /// `DynamicPerturbFactor`; zoomed, a non-zero zoomed limit adds their
    /// `ZoomedMinPerturb` / `ZoomedMaxPerturb`, truncated.
    pub fn cone(&self, mode: FireMode) -> f32 {
        let c = self.cone_params(mode);
        let m = self.mods;
        let (mut c_min, mut c_max) = (c.min as f64, c.max as f64);
        if mode == FireMode::Zoomed
            && let Some(z) = self.params.zoomed_cone
        {
            if z.min != 0 {
                c_min = (z.min as f64 + m.zoomed_min as f64).trunc();
            }
            if z.max != 0 {
                c_max = (z.max as f64 + m.zoomed_max as f64).trunc();
            }
        }
        // Eagle Eye (gear_perks.md 14): both limits lose a tenth, whole
        // units; a set's spread effect cuts the maximum (12).
        if self.perk_cut > 0.0 {
            let cut = self.perk_cut as f64;
            c_min -= (c_min * cut).floor();
            c_max -= (c_max * cut).floor();
        }
        if self.set_max_cut > 0.0 {
            c_max = (c_max * (1.0 - self.set_max_cut as f64)).trunc();
        }
        let g = (self.params.dynamic_factor + m.dynamic) as f64;
        let max = (c_max * g).trunc();
        let min = (c_min * g).trunc().min(max);
        if max <= 0.0 {
            return 0.0;
        }
        let p = self.fraction() as f64;
        (min + ((max - min) * p).floor() * g).trunc() as f32
    }

    /// One firing-meter sub-step at `now`: a shot grows it toward `cap`
    /// and stamps the time; otherwise it waits while less than `DecDelay`
    /// whole milliseconds have passed since the stamp, then recovers (the
    /// first recovery step clears the stamp). The attachments' rates
    /// (attachments.md, "Firing meter"): growth at I + [zoomed] Z_dec +
    /// sum inc (Q4: the zoomed decrease sum), recovery at R + [zoomed]
    /// Z_dec + sum dec.
    pub fn fire_substep(&mut self, scales: &SpreadScales, cone: ConeParams, fired: bool, cap: f32, now: f64) {
        let dt = SPREAD_SUBSTEP as f32;
        let m = self.mods;
        let z_dec = if cone.zoomed { m.zoomed_dec } else { 0.0 };
        if fired {
            self.fire = cap.min(self.fire + dt * scales.fire_inc * (cone.inc + z_dec + m.inc));
            self.last_shot = Some(now);
        } else if self
            .last_shot
            .is_some_and(|t| (((now - t) * 1000.0) + 1e-6).floor() < self.params.dec_delay as f64)
        {
        } else {
            self.last_shot = None;
            if self.fire > 0.0 {
                self.fire = (self.fire - dt * scales.fire_dec * (cone.dec + z_dec + m.dec)).max(0.0);
            }
        }
    }

    /// One movement-meter sub-step toward `target` (S), with `mu_s` (mu S)
    /// and the turn term `rho`; `jumping`: airborne after a jump.
    #[allow(clippy::too_many_arguments)]
    pub fn move_substep(
        &mut self,
        scales: &SpreadScales,
        cone: ConeParams,
        target: f32,
        mu_s: f32,
        rho: f32,
        jumping: bool,
        jump_cap: f32,
    ) {
        let dt = SPREAD_SUBSTEP as f32;
        let a = rho.max(mu_s);
        let m = self.movement;
        // The attachments' terms (attachments.md, "Movement meter"): rising,
        // the rate x (1 + sum inc) and, zoomed, + Z_inc a sub-step; falling,
        // R + [zoomed] Z_dec + sum dec; in a jump, I + [zoomed] Z_inc +
        // sum inc.
        let k = self.mods;
        let (z_inc, z_dec) = if cone.zoomed { (k.zoomed_inc, k.zoomed_dec) } else { (0.0, 0.0) };
        if a > m {
            let grown = m + (dt * scales.move_inc).min(a - m) * cone.inc * (1.0 + k.inc) + z_inc;
            self.movement = grown.min(target).min(mu_s);
        } else if a < m {
            let next = if jumping {
                // Q12: in a jump it rises instead, by the decrease step at
                // the increase speed.
                m + (dt * scales.move_dec).min(m - a) * (cone.inc + z_inc + k.inc)
            } else {
                m - (dt * scales.move_dec).min(m - a) * (cone.dec + z_dec + k.dec)
            };
            self.movement = next.min(jump_cap).max(0.0);
        }
    }

    /// The body yaw's change this tick (Q13): mouse yaw `mouse` turns the
    /// upper body first, up to `TWIST_MAX` either way, while no movement
    /// key is held (`moving` off); the body follows only beyond that (with
    /// a key held, at once). The recoil's side kick `wag` (view yaw) always
    /// turns it. Pitch never counts.
    pub fn body_turn(&mut self, mouse: f32, wag: f32, moving: bool) -> f32 {
        let body = self.body_yaw.get_or_insert(mouse);
        if moving {
            *body = mouse;
        } else {
            let twist = wrap_angle(mouse - *body);
            if twist > TWIST_MAX {
                *body = mouse - TWIST_MAX;
            } else if twist < -TWIST_MAX {
                *body = mouse + TWIST_MAX;
            }
        }
        let yaw = *body + wag;
        let turned = self.prev_view.map_or(0.0, |p| wrap_angle(yaw - p.y));
        self.prev_view = Some(Vec2::new(0.0, yaw));
        turned
    }

    /// The turn term rho for a view change of (pitch, yaw) radians.
    pub fn turn(scales: &SpreadScales, d_pitch: f32, d_yaw: f32) -> f32 {
        let dt = SPREAD_SUBSTEP as f32;
        (scales.turn_effect * (d_pitch.abs() + d_yaw.abs()) / (2.0 + scales.fast_turn_rate * dt)).min(1.0)
    }
}

/// A player's view kick (weapon_firing.md Corrections 1): the vertical
/// accumulator J and the side one J_y (radians, half the view offset each
/// caused, Q4), the side direction and the last kick's time. One per
/// player, never reset by a switch, a death or a respawn (only returned);
/// every kick and return reads the data of the weapon in hand then
/// (`CaRecoil`). Predicted with the player (`game::net`).
#[derive(Component, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CaKick {
    pub pitch: f32,
    pub yaw: f32,
    /// The side kick goes right (`wag_r`); None until the first one picks
    /// at random (the game draws it as the player enters the map).
    pub right: Option<bool>,
    pub last_kick: Option<f64>,
}

/// A weapon's view-kick data (`combat_arms:recoil`) and its fitted
/// attachments' terms; the state is its holder's (`CaKick`).
#[derive(Component, Clone, Debug)]
pub struct CaRecoil {
    pub params: RecoilParams,
    /// The fitted attachments' terms (`attachments`; zero: stock).
    pub mods: RecoilMods,
}

/// The camera moves back along its forward axis by this times the back
/// push's offset, units (view_models.md 4).
pub const BACK_PUSH_SCALE: f32 = 10.0;
/// The back push returns at this times `BackJitterRecover` a second
/// (weapon_firing.md, "Back push").
pub const BACK_RECOVER_SCALE: f32 = 5.0;

/// The recoil of a weapon with no `combat_arms:recoil` table: the record
/// defaults (a knife or a grenade in hand still returns the view).
pub static DEFAULT_RECOIL: LazyLock<CaRecoil> = LazyLock::new(|| CaRecoil::new(RecoilParams::default()));

impl CaRecoil {
    pub fn new(params: RecoilParams) -> Self {
        Self {
            params,
            mods: RecoilMods::default(),
        }
    }

    /// (base kick, cap) radians in this mode, the kick x (1 + p_b) and
    /// the cap x (1 + `cap`) (attachments.md, "Recoil").
    fn pitch_params(&self, burst: bool, cap: f32) -> (f32, f32) {
        let p = &self.params;
        let (b, max) = if burst && p.burst_pitch != 0.0 && p.burst_max_pitch != 0.0 {
            (p.burst_pitch, p.burst_max_pitch)
        } else {
            (p.pitch, p.max_pitch)
        };
        ((b * (1.0 + self.mods.pitch)).to_radians(), (max * (1.0 + cap)).to_radians())
    }

    /// The return's cap, radians: in burst mode the burst cap whenever it
    /// is non-zero (the kick needs both burst values, the return only
    /// this: Corrections 2), else the normal one; scaled by the muzzle
    /// part's `MaxCamRecoilPitchPer` alone.
    pub fn return_cap(&self, burst: bool) -> f32 {
        let p = &self.params;
        let max = if burst && p.burst_max_pitch != 0.0 { p.burst_max_pitch } else { p.max_pitch };
        (max * (1.0 + self.mods.return_max)).to_radians()
    }

    /// One shot's kick on `k` at `now`: returns how far the view goes up,
    /// radians (2h).
    pub fn kick(&self, k: &mut CaKick, burst: bool, now: f64) -> f32 {
        let (b, max) = self.pitch_params(burst, self.mods.max_pitch);
        if max <= 0.0 {
            return 0.0;
        }
        let j = k.pitch.clamp(0.0, max);
        let kick = b * (1.0 - j / max);
        let accel = self.params.accel_pitch + self.mods.accel;
        let kick = if kick > 0.0 { kick + accel * j } else { kick };
        let h = (kick / 2.0).min(max - j);
        k.pitch = j + h;
        k.last_kick = Some(now);
        2.0 * h
    }

    /// The side wag after a kick: returns the yaw change in the engine's
    /// sense (radians, + left), 0 without one. `roll` in [0, 1) picks the
    /// first direction and the flips.
    pub fn wag(&self, k: &mut CaKick, roll: f32) -> f32 {
        let p = self.params;
        // The threshold adds the attachments' `MaxRandomPitch` (a
        // silencer's -2 kicks from the first shot: Q11).
        if k.pitch.abs() <= p.random_pitch + self.mods.random_pitch {
            return 0.0;
        }
        let right = *k.right.get_or_insert(roll < 0.5);
        let max = p.max_wag.to_radians();
        let q = if max != 0.0 { 1.0 - k.yaw.abs() / max } else { 1.0 };
        let w = q * if right { p.wag_r } else { p.wag_l }.to_radians();
        k.yaw += w / 2.0;
        if k.yaw.abs() > p.random_wag && (roll * 2.0).fract() < 0.5 {
            k.right = Some(!right);
        }
        // Combat Arms turns yaw by -w (+ right); the engine's yaw is + left.
        w
    }

    /// One shot's back push on the backward offset `back` (the data's
    /// units; the camera sits `BACK_PUSH_SCALE` x it units back).
    pub fn back_kick(&self, back: &mut f32) {
        *back += self.params.back_jitter;
    }

    /// The back push's return over `dt` seconds: `BACK_RECOVER_SCALE` x
    /// `BackJitterRecover` a second, down to none.
    pub fn back_recover(&self, back: &mut f32, dt: f32) {
        let step = dt * BACK_RECOVER_SCALE * self.params.back_jitter_recover;
        *back = if *back > 0.0 { (*back - step).max(0.0) } else { (*back + step).min(0.0) };
    }

    /// The return of `k` over `dt` seconds at `now` (`burst`: the weapon
    /// in hand is in burst mode): (pitch down, yaw change in the engine's
    /// sense), radians.
    pub fn recover(&self, k: &mut CaKick, dt: f32, now: f64, burst: bool) -> (f32, f32) {
        let p = self.params;
        let waiting = k.last_kick.is_some_and(|t| now - t < p.jitter_dec_delay as f64 / 1000.0);
        let rate = if waiting { p.jitter_wait_recover } else { p.jitter_recover };
        let step = |acc: f32, max: f32| {
            let scale = if max > 0.0 { 1.0 + acc / max } else { 1.0 };
            acc.min(dt * rate * scale)
        };
        let mut down = 0.0;
        if k.pitch > 0.0 {
            down = step(k.pitch, self.return_cap(burst));
            k.pitch -= down / 2.0;
        }
        let mut side = 0.0;
        if k.yaw != 0.0 {
            let v = step(k.yaw.abs(), p.max_wag.to_radians());
            let sign = k.yaw.signum();
            k.yaw -= sign * v / 2.0;
            if k.yaw.signum() != sign {
                k.yaw = 0.0;
            }
            side = -sign * v;
        }
        (down, side)
    }
}

