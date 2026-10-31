//! The special weapons' data: the parameters of the weapon hooks
//! (`combat_arms:projectile`, `:flame`, `:device`, `:bolt`) as the
//! documents in data/weapons/ give them. Every key says in the document
//! which spec section and which attribute-data field it is; lengths are
//! the document's world units (`units`), times seconds, damage hit points.

use serde::{Deserialize, Serialize};

use super::super::{grenades::OverTimeParams, weapons::DamageParams};

/// Hook names (docs/weapon-data.md, "Hooks").
pub const PROJECTILE: &str = "combat_arms:projectile";
pub const FLAME: &str = "combat_arms:flame";
pub const DEVICE: &str = "combat_arms:device";
pub const BOLT: &str = "combat_arms:bolt";

fn explode() -> u32 {
    7
}

/// `combat_arms:projectile`: an impact projectile the weapon fires (1.3):
/// rockets, launcher shells, flame puffs, the air strike's shells.
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProjectileParams {
    /// The projectile record's `Velocity`, u/s.
    pub speed: f32,
    /// `Gravity` 1: the object gravity applies in flight.
    #[serde(default)]
    pub gravity: bool,
    /// `Lifetime`, s: it detonates in the air when this is over.
    pub lifetime: f32,
    /// The ammo's `Health`: hit points to shoot it down.
    pub health: f32,
    /// The box (half-extents, u): the model's first animation x `ModelScale`.
    pub box_half: [f32; 3],
    /// `CanHitSameKind`.
    #[serde(default)]
    pub hits_same_kind: bool,
    /// `MinPerturb`, `MaxPerturb` and `DynamicPerturbFactor` (the cone
    /// with the meters at 0: `grenades::throw_perturb`).
    pub perturb: [u32; 2],
    #[serde(default)]
    pub dynamic_perturb: f32,
    /// The ammo's `AreaDamage`, `AreaDamageRadius`, `AreaDamageType`.
    #[serde(default)]
    pub area_damage: f32,
    #[serde(default)]
    pub area_radius: f32,
    #[serde(default = "explode")]
    pub area_type: u32,
    /// The lasting sphere (`ProgDamage*`): the flames, the incendiary shell.
    #[serde(default)]
    pub over_time: Option<OverTimeParams>,
    /// The shooter never takes the sphere's burn (a `Guntype` 9 weapon's).
    #[serde(default)]
    pub shooter_exempt: bool,
    /// The projectile record's world model (`Model`, an archive path of
    /// the install), drawn in flight with `ModelScale` and `Skin#`
    /// (archive paths by texture index); None: no model (the flame puff),
    /// or no install.
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default = "unit_scale")]
    pub model_scale: [f32; 3],
    #[serde(default)]
    pub skins: Vec<String>,
    /// Its looks and sounds in flight (the projectile record's, from the
    /// install: special_weapons.md Follow-up (2026-10-30) 1-3).
    #[serde(default)]
    pub look: ProjectileLook,
    /// `CreateSound` and `CreateSoundRadius` (archive path, u): the
    /// server's sound when a player's shot makes it.
    #[serde(default)]
    pub create_sound: String,
    #[serde(default)]
    pub create_radius: f32,
}

fn unit_scale() -> [f32; 3] {
    [1.0; 3]
}

/// A projectile's looks and sounds in flight (special_weapons.md
/// Follow-up (2026-10-30) 1-3): the projectile record's `FXName` group
/// (`FXLoop`, `FXSmoothShutdown`), `FlySound` with `SoundRadius`, the
/// flare (`FlareSprite`, `FlareScale`, `FlareYOffset` u) and the smoke
/// trail's `SmokeTrailType`. Empty: nothing.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectileLook {
    pub fx: String,
    pub fx_loop: bool,
    pub fx_smooth: bool,
    pub fly_sound: String,
    pub sound_radius: f32,
    pub flare: Option<FlareLook>,
    pub smoke_trail: Option<u32>,
    /// `ActivateSound` (looped while the projectile is at rest),
    /// `FinishSound` (once, `finish_time` s before the lifetime `lifetime`
    /// ends) with `SoundRadius` for all three (special_weapons.md
    /// Corrections (2026-10-31) 3).
    pub activate_sound: String,
    pub finish_sound: String,
    pub finish_time: f32,
    pub lifetime: f32,
}

/// A projectile's flare sprite (special_weapons.md Follow-up 3).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct FlareLook {
    pub sprite: String,
    pub scale: [f32; 2],
    pub y_offset: f32,
}

impl ProjectileLook {
    /// As text after a drawn key (`shown_key`), so whatever draws a
    /// projectile, a network client's too, reads its look from the key
    /// it is drawn under (`parse`): `fx=..;loop=1;smooth=1;fly=..;r=..;
    /// flare=sprite,sx,sy,dy;smoke=n` (fields left out when empty).
    pub fn key(&self) -> String {
        let mut out: Vec<String> = Vec::new();
        if !self.fx.is_empty() {
            out.push(format!("fx={}", self.fx));
            if self.fx_loop {
                out.push("loop=1".into());
            }
            if self.fx_smooth {
                out.push("smooth=1".into());
            }
        }
        if !self.fly_sound.is_empty() {
            out.push(format!("fly={}", self.fly_sound));
        }
        if !self.activate_sound.is_empty() {
            out.push(format!("act={}", self.activate_sound));
        }
        if !self.finish_sound.is_empty() {
            out.push(format!("fin={}", self.finish_sound));
            out.push(format!("fint={}", self.finish_time));
            out.push(format!("life={}", self.lifetime));
        }
        if self.has_sounds() {
            out.push(format!("r={}", self.sound_radius));
        }
        if let Some(f) = &self.flare {
            out.push(format!("flare={},{},{},{}", f.sprite, f.scale[0], f.scale[1], f.y_offset));
        }
        if let Some(t) = self.smoke_trail {
            out.push(format!("smoke={t}"));
        }
        out.join(";")
    }

    /// The look a drawn key carries (`key`; none: the default).
    pub fn parse(shown: &str) -> Self {
        let mut look = Self::default();
        let Some((_, text)) = shown.split_once(LOOK_SEPARATOR) else { return look };
        for field in text.split(';') {
            let Some((k, v)) = field.split_once('=') else { continue };
            match k {
                "fx" => look.fx = v.to_string(),
                "loop" => look.fx_loop = v == "1",
                "smooth" => look.fx_smooth = v == "1",
                "fly" => look.fly_sound = v.to_string(),
                "r" => look.sound_radius = v.parse().unwrap_or(0.0),
                "act" => look.activate_sound = v.to_string(),
                "fin" => look.finish_sound = v.to_string(),
                "fint" => look.finish_time = v.parse().unwrap_or(0.0),
                "life" => look.lifetime = v.parse().unwrap_or(0.0),
                "flare" => {
                    let p: Vec<&str> = v.split(',').collect();
                    if let [sprite, sx, sy, dy] = p[..] {
                        look.flare = Some(FlareLook {
                            sprite: sprite.to_string(),
                            scale: [sx.parse().unwrap_or(1.0), sy.parse().unwrap_or(1.0)],
                            y_offset: dy.parse().unwrap_or(0.0),
                        });
                    }
                }
                "smoke" => look.smoke_trail = v.parse().ok(),
                _ => {}
            }
        }
        look
    }

    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Whether it names a fly, activate or finish sound.
    pub fn has_sounds(&self) -> bool {
        !(self.fly_sound.is_empty() && self.activate_sound.is_empty() && self.finish_sound.is_empty())
    }
}

/// Between a projectile's drawn key and its look (`ProjectileLook::key`).
pub const LOOK_SEPARATOR: char = '~';

impl ProjectileParams {
    /// The key its projectile is drawn under (`map::loose::ShownItem`, the
    /// held-model kit): one per model, scale, skins and look, whichever
    /// weapon fires it; a model-less projectile's never has a model. The
    /// look follows `LOOK_SEPARATOR` (none when it has none).
    pub fn shown_key(&self) -> String {
        let base = match &self.model {
            Some(m) => format!(
                "{}{m}|{:?}|{}",
                PROJECTILE_KEY,
                self.model_scale,
                self.skins.join(",")
            ),
            None => Self::NO_MODEL_KEY.to_string(),
        };
        if self.look.is_empty() {
            base
        } else {
            format!("{base}{LOOK_SEPARATOR}{}", self.look.key())
        }
    }

    /// Whether a drawn key is a model-less projectile's.
    pub fn is_modelless(shown: &str) -> bool {
        shown.split(LOOK_SEPARATOR).next() == Some(Self::NO_MODEL_KEY)
    }

    /// The drawn key of a projectile without a model.
    pub const NO_MODEL_KEY: &'static str = "combat_arms:projectile#none";
}

/// What every impact projectile's drawn key starts with.
pub const PROJECTILE_KEY: &str = "combat_arms:projectile#";

/// `combat_arms:flame`: the flamethrower's fire keys (4): a pre-fire, then
/// a loop with `keys` fire keys `interval` apart.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FlameParams {
    pub prefire: f32,
    pub loop_len: f32,
    pub keys: u32,
    pub interval: f32,
    pub postfire: f32,
}

/// What kind of device (6).
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    LandMine,
    Hornet,
    Claymore,
    RemoteGrenade,
    Caltrops,
    NerveMine,
    Turret,
    FirstAid,
    Medkit,
    AirStrike,
}

/// The claymore's cone (6.3): `AreaArcDamage`, `AreaArcDamageRadius`,
/// `AreaArcDamageAngle` (half-angle, degrees).
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ArcParams {
    pub damage: f32,
    pub radius: f32,
    pub angle: f32,
}

/// The caltrops' cylinder and bleeding (6.5).
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CaltropsParams {
    /// `ProgDamage` HP/s for `ProgDamageDuration` s.
    pub rate: f32,
    pub duration: f32,
    /// `ProgDamageRadius`, `ProgDamageHeight`.
    pub radius: f32,
    pub height: f32,
    /// `DamageMaxCount`, and the time between counts of one victim.
    pub counts: u32,
    pub recount: f32,
    /// `StunnedMovePenalty`, `StunnedTime`.
    pub stun_penalty: f32,
    pub stun_time: f32,
}

/// The nerve mine's per-frame effect (6.6): damage type 44 with `amount`
/// for `duration` s on enemies within `radius`.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NerveParams {
    pub damage_type: i32,
    pub amount: f32,
    pub duration: f32,
    pub radius: f32,
}

/// The auto turret (6.7): the `Turret#` record and its gun.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct TurretParams {
    /// `Lower_set`, s: the time from landing to firing.
    pub setup: f32,
    /// `MaxAmmo`, `FireDelay`, `Perturb` (u, each axis), `SensorOffset`.
    pub ammo: u32,
    pub fire_delay: f32,
    pub perturb: f32,
    pub sensor_offset: [f32; 3],
    /// `ShootingAngle` (below, above), degrees.
    pub angles: [f32; 2],
    /// `SensorAreaRadius`, u.
    pub search_radius: f32,
    /// The top gun's ray (`Turret-TopGun`): damage, type, roll and range
    /// bands as a weapon's `combat_arms:damage`, and its range.
    pub gun: DamageParams,
    pub range: f32,
}

/// The first-aid kit (6.8): `AreaDamageTickTime`, `AreaDamageTickDamage`,
/// `AreaDamageTotal`, `AreaDamageRadius`, the heal's damage type.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct KitParams {
    pub tick: f32,
    pub heal: f32,
    pub pool: f32,
    pub radius: f32,
}

/// The medkit's heal over time (6.9): HP/s for s, at most `total`.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MedkitParams {
    pub rate: f32,
    pub duration: f32,
    pub total: f32,
}

/// An air strike designator (6.10): the marker's flight (`speed`,
/// `lifetime` are the device's), the shells and where they drop.
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StrikeParams {
    /// `AirDropHeight`, `AirDropDelay` (s between shells).
    pub height: f32,
    pub delay: f32,
    /// `AirDropPoint#`: (right, forward) offsets along the course, u.
    pub points: Vec<[f32; 2]>,
    /// The shell (the `AirStrikeID` weapon's projectile).
    pub shell: ProjectileParams,
}

/// `combat_arms:device`: a thrown or placed device (6) and how it is used.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DeviceParams {
    pub kind: DeviceKind,
    /// `SpawnedAmount`, `MaxAmount`.
    pub spawned: u32,
    pub max: u32,
    /// The view-model animations (6.12): pre-fire, the throw play that
    /// repeats while fire is held (0: none), the fire key's time in the
    /// post-fire (or `fire_0`), the animation's length after the press,
    /// and the redraw.
    #[serde(default)]
    pub prefire: f32,
    #[serde(default)]
    pub throw_loop: f32,
    pub key: f32,
    pub postfire: f32,
    pub redraw: f32,
    /// The thrown cone (`MinPerturb`, `MaxPerturb`, the dynamic factor).
    #[serde(default)]
    pub perturb: [u32; 2],
    #[serde(default)]
    pub dynamic_perturb: f32,
    /// The projectile record's `Velocity` (0: placed, not thrown) and
    /// `Lifetime`, and the ammo's `Health`.
    pub speed: f32,
    pub lifetime: f32,
    pub health: f32,
    /// The box (half-extents, u).
    pub box_half: [f32; 3],
    /// `OnceActivate`: a new one replaces the old.
    #[serde(default)]
    pub once_only: bool,
    /// `SubControlWeaponName`: the detonator that goes with it.
    #[serde(default)]
    pub controller: Option<String>,
    /// The blast (`AreaDamage`, `AreaDamageRadius`, `AreaDamageType`).
    #[serde(default)]
    pub area_damage: f32,
    #[serde(default)]
    pub area_radius: f32,
    #[serde(default = "explode")]
    pub area_type: u32,
    /// The `active` animation between the trigger and the blast, s.
    #[serde(default)]
    pub active: f32,
    /// The hornet's `ready` animation, s, and `SensorAreaRadius`, u.
    #[serde(default)]
    pub ready: f32,
    #[serde(default)]
    pub sensor_radius: f32,
    /// The weak points: damage types it takes more of.
    #[serde(default)]
    pub weak: Vec<(i32, f32)>,
    #[serde(default)]
    pub arc: Option<ArcParams>,
    #[serde(default)]
    pub caltrops: Option<CaltropsParams>,
    #[serde(default)]
    pub nerve: Option<NerveParams>,
    /// Its looks and sounds in flight and at rest (the projectile
    /// record's `ActivateSound`, `FinishSound`, ...), and its create
    /// sound; `auto_throw`: the weapon's `PlayAutoThrowSound` is on, so
    /// its call-out stands in for the create sound.
    #[serde(default)]
    pub look: ProjectileLook,
    #[serde(default)]
    pub create_sound: String,
    #[serde(default)]
    pub create_radius: f32,
    #[serde(default)]
    pub auto_throw: bool,
    #[serde(default)]
    pub turret: Option<TurretParams>,
    #[serde(default)]
    pub kit: Option<KitParams>,
    #[serde(default)]
    pub medkit: Option<MedkitParams>,
    #[serde(default)]
    pub strike: Option<StrikeParams>,
}

/// `combat_arms:bolt`: a crossbow's bolt (7).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct BoltParams {
    /// `Velocity` (u/s), `Lifetime`.
    pub speed: f32,
    pub lifetime: f32,
    /// The ammo's `Health`.
    pub health: f32,
    /// The direct damage (`InstDamage`, its type, the roll, the range
    /// bands, measured from the map's origin: Quirk 4).
    pub damage: DamageParams,
    /// The cone (`MinPerturb`, `MaxPerturb`, the dynamic factor).
    pub perturb: [u32; 2],
    #[serde(default)]
    pub dynamic_perturb: f32,
    /// The explosive bolt: the delay after the detonator press, the blast
    /// and the weapon's `Range` (it goes off beyond it).
    #[serde(default)]
    pub explosive: Option<ExplosiveParams>,
    /// The detonator that goes with it (`SubControlWeaponName`).
    #[serde(default)]
    pub controller: Option<String>,
    /// Its sounds in flight (the explosive bolt's `FinishSound` ...).
    #[serde(default)]
    pub look: ProjectileLook,
}

/// The explosive bolt (7): `ExplosionDelayTime`, `AreaDamage`,
/// `AreaDamageRadius`, `AreaDamageType`, `Range`.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ExplosiveParams {
    pub delay: f32,
    pub area_damage: f32,
    pub area_radius: f32,
    pub area_type: u32,
    pub range: f32,
}

/// `combat_arms:controller`: a detonator (a weapon whose fire detonates
/// the owner's devices of `of`).
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ControllerParams {
    /// The weapon ID of the devices it sets off.
    pub of: String,
}
