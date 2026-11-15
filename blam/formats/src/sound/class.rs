//! The sound classes (specs/halo_ce/sounds.md, "Sound class"): the 51
//! slots of the game's class table, the 31 named ones as a typed enum with
//! the table's per-class defaults (instance limits, preemption, priority,
//! wet gain, and the distances a sound takes when its own are zero).

/// A sound class; the discriminant is the stored class number. The 20
/// slots the game leaves unnamed (2, 3, 11, 12, 16, 17, 20, 21, 24, 25, 31,
/// 36 to 38, 40 to 43, 48, 49) carry no defaults and have no variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum SoundClass {
    ProjectileImpact = 0,
    ProjectileDetonation = 1,
    WeaponFire = 4,
    WeaponReady = 5,
    WeaponReload = 6,
    WeaponEmpty = 7,
    WeaponCharge = 8,
    WeaponOverheat = 9,
    WeaponIdle = 10,
    ObjectImpacts = 13,
    ParticleImpacts = 14,
    SlowParticleImpacts = 15,
    UnitFootsteps = 18,
    UnitDialog = 19,
    VehicleCollision = 22,
    VehicleEngine = 23,
    DeviceDoor = 26,
    DeviceForceField = 27,
    DeviceMachinery = 28,
    DeviceNature = 29,
    DeviceComputers = 30,
    Music = 32,
    AmbientNature = 33,
    AmbientMachinery = 34,
    AmbientComputers = 35,
    FirstPersonDamage = 39,
    ScriptedDialogPlayer = 44,
    ScriptedEffect = 45,
    ScriptedDialogOther = 46,
    ScriptedDialogForceUnspatialized = 47,
    GameEvent = 50,
}

/// How many class slots the game has.
pub const SOUND_CLASS_COUNT: usize = 51;

/// A class's row of the game's table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClassDefaults {
    /// Most voices playing one sound.
    pub per_definition_limit: u8,
    /// Most voices of one sound from one object.
    pub per_object_limit: u8,
    /// Milliseconds before a voice may be taken over.
    pub preemption_ms: u32,
    pub speech: bool,
    /// Higher wins a voice.
    pub priority: u8,
    /// Cache-miss mode: true postpones, false discards (a port ignores it).
    pub postpone_on_cache_miss: bool,
    /// Reverb attenuation.
    pub wet_gain: f32,
    /// Minimum distance, world units, for a sound whose own is zero.
    pub minimum_distance: f32,
    /// Maximum distance, world units, for a sound whose own is zero.
    pub maximum_distance: f32,
}

#[allow(clippy::too_many_arguments)]
const fn row(
    limits: (u8, u8),
    preempt: u32,
    speech: bool,
    priority: u8,
    postpone: bool,
    wet: f32,
    min: f32,
    max: f32,
) -> ClassDefaults {
    ClassDefaults {
        per_definition_limit: limits.0,
        per_object_limit: limits.1,
        preemption_ms: preempt,
        speech,
        priority,
        postpone_on_cache_miss: postpone,
        wet_gain: wet,
        minimum_distance: min,
        maximum_distance: max,
    }
}

impl SoundClass {
    /// Every named class, in class number order.
    pub const ALL: [SoundClass; 31] = {
        use SoundClass::*;
        [
            ProjectileImpact,
            ProjectileDetonation,
            WeaponFire,
            WeaponReady,
            WeaponReload,
            WeaponEmpty,
            WeaponCharge,
            WeaponOverheat,
            WeaponIdle,
            ObjectImpacts,
            ParticleImpacts,
            SlowParticleImpacts,
            UnitFootsteps,
            UnitDialog,
            VehicleCollision,
            VehicleEngine,
            DeviceDoor,
            DeviceForceField,
            DeviceMachinery,
            DeviceNature,
            DeviceComputers,
            Music,
            AmbientNature,
            AmbientMachinery,
            AmbientComputers,
            FirstPersonDamage,
            ScriptedDialogPlayer,
            ScriptedEffect,
            ScriptedDialogOther,
            ScriptedDialogForceUnspatialized,
            GameEvent,
        ]
    };

    /// The class stored as `n`: `None` for an unnamed slot or a number
    /// outside 0..51.
    pub fn from_index(n: i32) -> Option<SoundClass> {
        Self::ALL.iter().copied().find(|c| *c as i32 == n)
    }

    /// The stored class number.
    pub fn index(self) -> u8 {
        self as u8
    }

    /// The class's row of the table.
    pub fn defaults(self) -> ClassDefaults {
        use SoundClass::*;
        match self {
            ProjectileImpact => row((6, 4), 100, false, 4, false, 0.5, 1.4, 8.0),
            ProjectileDetonation => row((4, 1), 200, false, 5, true, 0.5, 8.0, 120.0),
            WeaponFire => row((4, 1), 0, false, 4, true, 0.5, 4.0, 70.0),
            WeaponReady | WeaponReload | WeaponCharge | WeaponOverheat | WeaponIdle => {
                row((4, 1), 500, false, 4, true, 0.5, 1.0, 9.0)
            }
            WeaponEmpty => row((4, 1), 60, false, 4, true, 0.5, 1.0, 9.0),
            ObjectImpacts => row((4, 1), 100, false, 3, true, 0.5, 0.5, 3.0),
            ParticleImpacts => row((4, 1), 100, false, 3, false, 0.5, 0.5, 3.0),
            SlowParticleImpacts => row((4, 1), 1000, false, 3, false, 0.5, 0.5, 3.0),
            UnitFootsteps => row((4, 1), 200, false, 3, false, 0.5, 0.9, 10.0),
            UnitDialog => row((4, 1), 100, true, 3, true, 0.8, 3.0, 20.0),
            VehicleCollision => row((4, 2), 400, false, 3, false, 0.5, 1.4, 8.0),
            VehicleEngine => row((4, 2), 100, false, 3, true, 0.9, 1.4, 8.0),
            DeviceDoor | DeviceForceField => row((4, 1), 100, false, 2, true, 0.5, 0.9, 5.0),
            DeviceMachinery | DeviceNature => row((4, 1), 100, false, 1, true, 0.5, 0.9, 5.0),
            DeviceComputers => row((4, 1), 100, false, 1, true, 0.5, 0.5, 3.0),
            Music => row((4, 4), 100, false, 2, true, 1.0, 0.9, 5.0),
            AmbientNature | AmbientMachinery => row((4, 1), 100, false, 1, true, 1.0, 0.9, 5.0),
            AmbientComputers => row((4, 1), 100, false, 1, true, 1.0, 0.5, 3.0),
            FirstPersonDamage => row((4, 1), 100, false, 4, true, 1.0, 0.5, 3.0),
            ScriptedDialogPlayer | ScriptedDialogForceUnspatialized => row((4, 4), 100, true, 6, true, 0.8, 3.0, 20.0),
            ScriptedEffect => row((4, 4), 100, false, 3, true, 0.8, 2.0, 5.0),
            ScriptedDialogOther => row((4, 4), 100, true, 5, true, 0.8, 3.0, 20.0),
            GameEvent => row((4, 1), 100, false, 5, true, 1.0, 3.0, 20.0),
        }
    }

    /// The scripted dialogue classes (44, 46, 47), which duck every other
    /// class while they play and are never ducked themselves.
    pub fn is_scripted_dialog(self) -> bool {
        matches!(
            self,
            SoundClass::ScriptedDialogPlayer
                | SoundClass::ScriptedDialogOther
                | SoundClass::ScriptedDialogForceUnspatialized
        )
    }
}
