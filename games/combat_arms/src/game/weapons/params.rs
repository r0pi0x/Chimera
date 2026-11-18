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
// Hook parameters (data)

pub(super) fn one() -> f32 {
    1.0
}

/// `combat_arms:damage`: damage per ray and the server's spacing.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageParams {
    /// `InstDamage` (ammo), hit points (the document's damage units).
    pub damage: f32,
    /// `InstDamageType` (ammo): 2 is bullet (the only one armour knows yet).
    #[serde(default = "bullet_type")]
    pub damage_type: i32,
    /// `DamageRFacter` (weapon): each hit x U(1 - roll, 1 + roll).
    #[serde(default = "default_roll")]
    pub roll: f32,
    /// `Effectrange0..2` (weapon): band edges, length units (truncated to
    /// whole units).
    #[serde(default)]
    pub effect_range: Vec<f32>,
    /// `Rangedamage0..2` (ammo): each band's multiplier.
    #[serde(default)]
    pub range_damage: Vec<f32>,
    /// `fireDelay` (weapon), ms.
    #[serde(default = "default_fire_delay")]
    pub fire_delay: f32,
    /// `DamageMovePenalty` (ammo): the knock-back strength K on an
    /// infected victim (movement.md, "Hit knock-back").
    #[serde(default)]
    pub move_penalty: f32,
    /// `DamageMovePenaltyDuration` (ammo), s; not above 0: none.
    #[serde(default = "no_duration")]
    pub move_penalty_duration: f32,
    /// The third-person reload's length at rate 1 (s; by stance in the
    /// game, ours one): the server's reload window (weapon_classes.md
    /// Corrections 2). None: the magazine's reload.
    #[serde(default)]
    pub reload_third_person: Option<f32>,
}

pub(super) fn bullet_type() -> i32 {
    BULLET
}
pub(super) fn no_duration() -> f32 {
    -1.0
}
pub(super) fn default_roll() -> f32 {
    0.1
}
pub(super) fn default_fire_delay() -> f32 {
    30.0
}

/// A cone's limits (spread units, thousandths) and the meter's speeds.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ConeParams {
    /// `MinPerturb`, `MaxPerturb`.
    pub min: i32,
    pub max: i32,
    /// `PerturbIncSpeed`, `PerturbDecSpeed`.
    pub inc: f32,
    pub dec: f32,
    /// The weapon's zoomed rates are in use (`CaSpread::cone_params` in
    /// zoomed mode): the attachments' zoomed terms apply.
    #[serde(skip)]
    pub zoomed: bool,
}

/// The ammo's state values (`Fire*Perturb` caps, movement targets).
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct StateParams {
    pub fire: f32,
    pub fire_move: f32,
    pub fire_duck: f32,
    pub fire_move_duck: f32,
    pub walk: f32,
    pub run: f32,
    pub jump: f32,
}

/// `combat_arms:spread`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpreadParams {
    pub cone: ConeParams,
    /// Zoomed values (`Zoomed*`), used while zoomed where non-zero; the
    /// record defaults (1/500, 3.0/3.0) when the document leaves them out
    /// (attribute_data.md Corrections 2).
    #[serde(default = "zoomed_default")]
    pub zoomed_cone: Option<ConeParams>,
    /// Burst values (`TripleShot*Perturb*`), used in burst mode where
    /// non-zero, field by field; the record defaults (7/100, 1.0/1.0) when
    /// left out.
    #[serde(default = "burst_default")]
    pub burst_cone: Option<ConeParams>,
    /// `DynamicPerturbFactor`.
    #[serde(default = "one")]
    pub dynamic_factor: f32,
    pub state: StateParams,
    /// The ammo's `Zoomed*Perturb` values, used while zoomed where non-zero.
    #[serde(default)]
    pub zoomed_state: StateParams,
    /// `DecDelay` (ammo), ms.
    #[serde(default = "default_dec_delay")]
    pub dec_delay: f32,
}

pub(super) fn default_dec_delay() -> f32 {
    150.0
}

/// The record defaults of `ZoomedMinPerturb`/`MaxPerturb` and the zoomed
/// speeds (attribute_data.md, Field table: weapons).
pub(super) fn zoomed_default() -> Option<ConeParams> {
    Some(ConeParams {
        min: 1,
        max: 500,
        inc: 3.0,
        dec: 3.0,
        zoomed: false,
    })
}

/// The record defaults of the `TripleShot*Perturb*` fields.
pub(super) fn burst_default() -> Option<ConeParams> {
    Some(ConeParams {
        min: 7,
        max: 100,
        inc: 1.0,
        dec: 1.0,
        zoomed: false,
    })
}

/// `combat_arms:recoil` (degrees but the two random thresholds, radians;
/// delays ms; rates radians/s). Defaults: attribute_data.md's.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct RecoilParams {
    /// `BaseCamRecoilPitch`.
    pub pitch: f32,
    /// `BaseCamRecoilAccelPitch`, per radian accumulated.
    pub accel_pitch: f32,
    /// `MaxCamRecoilPitch`.
    pub max_pitch: f32,
    /// `MaxRandomPitch`, radians.
    pub random_pitch: f32,
    /// `BaseCamRecoilWagR`, `BaseCamRecoilWagL`.
    pub wag_r: f32,
    pub wag_l: f32,
    /// `MaxCamRecoilWag`.
    pub max_wag: f32,
    /// `MaxRandomwag`, radians.
    pub random_wag: f32,
    /// `BaseCamBackJitter`, `BackJitterRecover`: the back push per shot
    /// and its return, 5 x this per second (`CaRecoil::back_kick`).
    pub back_jitter: f32,
    pub back_jitter_recover: f32,
    /// `TripleShotBaseCamRecoilPitch`, `TripleShotMaxCamRecoilPitch`: the
    /// kick's in burst mode when both are non-zero, the return's cap when
    /// the cap is (record defaults 1.0 and 50.0, attribute_data.md
    /// Corrections 2).
    pub burst_pitch: f32,
    pub burst_max_pitch: f32,
    /// `JitterDecDelay` (ammo), ms.
    pub jitter_dec_delay: f32,
    /// `JitterWaitRecover`, `JitterRecover` (ammo).
    pub jitter_wait_recover: f32,
    pub jitter_recover: f32,
}

impl Default for RecoilParams {
    fn default() -> Self {
        Self {
            pitch: 1.2,
            accel_pitch: 0.0,
            max_pitch: 10.0,
            random_pitch: 0.032,
            wag_r: 1.0,
            wag_l: -1.0,
            max_wag: 30.0,
            random_wag: 0.015,
            back_jitter: 0.5,
            back_jitter_recover: 3.0,
            burst_pitch: 1.0,
            burst_max_pitch: 50.0,
            jitter_dec_delay: 600.0,
            jitter_wait_recover: 0.1,
            jitter_recover: 0.3,
        }
    }
}

