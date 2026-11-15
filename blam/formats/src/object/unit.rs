//! The unit layer (bipeds and vehicles) and the biped's own fields
//! (specs/halo_ce/objects.md, "The unit base" and "The biped"), with the
//! spec's absolute offsets: the unit layer is 0x17C to 0x2F0, the biped's
//! own part 0x2F0 to 0x4F4.

use std::io;

use super::{GrenadeType, bit_flags};
use crate::cache::{TagRef, TagStruct, View};
use crate::scenario::coded_enum;

bit_flags! {
    /// The unit flags (offset 0x17C). Names are the reference's.
    pub struct UnitFlags(u32) {
        CIRCULAR_AIMING = 0,
        DESTROYED_AFTER_DYING = 1,
        HALF_SPEED_INTERPOLATION = 2,
        /// The Spartan has it.
        FIRES_FROM_CAMERA = 3,
        ENTRANCE_INSIDE_BOUNDING_SPHERE = 4,
        UNUSED = 5,
        CAUSES_PASSENGER_DIALOGUE = 6,
        RESISTS_PINGS = 7,
        MELEE_ATTACK_IS_FATAL = 8,
        DOES_NOT_REORIENT_DURING_PINGS = 9,
        HAS_NO_AIMING = 10,
        SIMPLE_CREATURE = 11,
        IMPACT_MELEE_ATTACHES_TO_TARGET = 12,
        IMPACT_MELEE_DIES_ON_SHIELD = 13,
        CANNOT_OPEN_DOORS_AUTOMATICALLY = 14,
        /// The Spartan has it.
        MELEE_ATTACKERS_CANNOT_ATTACH = 15,
        NEVER_UNSUSPECTING = 16,
        SHIELD_SAPPING = 17,
        RUNS_AROUND_FLAMING = 18,
        INCONSEQUENTIAL = 19,
        SPECIAL_CINEMATIC_UNIT = 20,
        IGNORED_BY_AUTOAIMING = 21,
        SHIELDS_FRY_INFECTION_FORMS = 22,
        INTEGRATED_LIGHT_CONTROLS_WEAPON_DIRECTLY = 23,
        INTEGRATED_LIGHT_LASTS_FOREVER = 24,
    }
}

bit_flags! {
    /// The biped flags (offset 0x2F4); the Spartans have 0x202.
    pub struct BipedFlags(u32) {
        TURNS_WITHOUT_ANIMATING = 0,
        USES_PLAYER_PHYSICS = 1,
        FLYING = 2,
        PHYSICS_PILL_CENTERED_AT_ORIGIN = 3,
        SPHERICAL = 4,
        PASSES_THROUGH_OTHER_BIPEDS = 5,
        CAN_CLIMB_ANY_SURFACE = 6,
        IMMUNE_TO_FALLING_DAMAGE = 7,
        ROTATE_WHILE_AIRBORNE = 8,
        USES_LIMP_BODY_PHYSICS = 9,
        HAS_NO_DYING_AIRBORNE = 10,
        RANDOM_SPEED_INCREASE = 11,
        UNIT_USES_OLD_PLAYER_PHYSICS = 12,
    }
}

bit_flags! {
    /// A seat's flags (the bits the spec names; others exist).
    pub struct SeatFlags(u32) {
        INVISIBLE = 0,
        DRIVER = 2,
        GUNNER = 3,
        THIRD_PERSON_CAMERA = 4,
        THIRD_PERSON_ON_ENTER = 6,
        FIRST_PERSON_CAMERA = 7,
        ALLOWS_NONCOMBATANTS = 10,
    }
}

coded_enum! {
    /// A unit's default team (offset 0x180).
    pub enum UnitTeam {
        Default = 0,
        Player = 1,
        Human = 2,
        Covenant = 3,
        Flood = 4,
        Sentinel = 5,
        Unused6 = 6,
        Unused7 = 7,
        Unused8 = 8,
        Unused9 = 9,
    }
}

coded_enum! {
    /// How far a unit's constant noise carries for AI hearing (inferred
    /// names; the disc has only 0 and 1).
    pub enum SoundVolume {
        Silent = 0,
        Medium = 1,
        Loud = 2,
        Shout = 3,
        Quiet = 4,
    }
}

coded_enum! {
    /// The unit's motion sensor blip size.
    pub enum BlipSize {
        Medium = 0,
        Small = 1,
        Large = 2,
    }
}

coded_enum! {
    /// Where a unit-layer incoming function value comes from (offsets
    /// 0x198 to 0x19E).
    pub enum UnitInput {
        None = 0,
        DriverSeatPower = 1,
        GunnerSeatPower = 2,
        /// The aiming change byte / 255.
        AimingChange = 3,
        MouthAperture = 4,
        IntegratedLightPower = 5,
        /// 1 while alive and allowed to blink, else 0.
        CanBlink = 6,
        ShieldSapping = 7,
    }
}

coded_enum! {
    /// Where a biped-layer incoming function value comes from (offsets
    /// 0x30C to 0x312).
    pub enum BipedInput {
        None = 0,
        /// The speed over the flying velocity, clamped to 0..1.
        FlyingSpeed = 1,
    }
}

crate::tag_struct! {
    /// A camera track reference (element 0x1C).
    pub struct CameraTrack: 0x1C {
        /// Class `trak`.
        0x00 track: TagRef,
    }
}

crate::tag_struct! {
    /// A unit HUD interface (element 0x30): element 0 is used in single
    /// player, element 1 (or the last) in multiplayer.
    pub struct UnitHudInterface: 0x30 {
        /// Class `unhi`.
        0x00 hud: TagRef,
    }
}

crate::tag_struct! {
    /// A dialogue variant (element 0x18).
    pub struct DialogueVariant: 0x18 {
        0x00 variant_number: i16,
        /// Class `udlg`.
        0x08 dialogue: TagRef,
    }
}

crate::tag_struct! {
    /// A powered seat (element 0x44).
    pub struct PoweredSeat: 0x44 {
        /// Seconds.
        0x04 power_up_time: f32,
        /// Seconds.
        0x08 power_down_time: f32,
    }
}

crate::tag_struct! {
    /// A starting weapon of the unit (element 0x24). The multiplayer
    /// game's starting weapons come from the scenario instead.
    pub struct UnitWeapon: 0x24 {
        /// Class `weap`.
        0x00 weapon: TagRef,
    }
}

crate::tag_struct! {
    /// A seat (element 0x11C), the fields the objects spec's summary gives;
    /// vehicles.md owns the rest (camera, HUD).
    pub struct Seat: 0x11C {
        0x00 flags: SeatFlags,
        0x04 label: String,
        0x24 marker_name: String,
        0x64 acceleration_scale: [f32; 3],
        /// The reference's unit is unclear (60, 30, 80, 50 on the disc:
        /// looks like degrees per second).
        0x7C yaw_rate: f32,
        0x80 pitch_rate: f32,
        /// Radians.
        0xF0 yaw_minimum: f32,
        0xF4 yaw_maximum: f32,
        /// Class `actv`.
        0xF8 built_in_actor: TagRef,
    }
}

crate::tag_struct! {
    /// The unit layer of bipeds and vehicles (absolute offsets; `SIZE` is
    /// the layer's end, 0x2F0).
    pub struct UnitBase: 0x2F0 {
        0x17C flags: UnitFlags,
        0x180 default_team: UnitTeam,
        0x182 constant_sound_volume: SoundVolume,
        /// Fraction of a vehicle's damage passed on to its riders (0.18 on
        /// the disc's vehicles).
        0x184 rider_damage_fraction: f32,
        /// Class `effe`: played when the flashlight is toggled.
        0x188 integrated_light_toggle: TagRef,
        0x198 input_a: UnitInput,
        0x19A input_b: UnitInput,
        0x19C input_c: UnitInput,
        0x19E input_d: UnitInput,
        /// Radians (1.2217305 = 70 degrees on the disc).
        0x1A0 camera_field_of_view: f32,
        0x1A4 camera_stiffness: f32,
        /// Empty on bipeds (the biped's camera heights are used).
        0x1A8 camera_marker_name: String,
        0x1C8 camera_submerged_marker_name: String,
        /// Radians.
        0x1E8 pitch_auto_level: f32,
        0x1EC pitch_range_minimum: f32,
        0x1F0 pitch_range_maximum: f32,
        0x1F4 camera_tracks: Vec<CameraTrack>,
        0x200 seat_acceleration_scale: [f32; 3],
        /// Seconds.
        0x20C integrated_light_power_on_time: f32,
        0x210 integrated_light_power_off_time: f32,
        0x214 integrated_light_lifespan: f32,
        /// Fraction of vitality.
        0x218 soft_ping_threshold: f32,
        /// Seconds (0.3 s = 9 ticks).
        0x21C soft_ping_minimum_interrupt_time: f32,
        /// Fraction (1.0: never, in effect).
        0x220 hard_ping_threshold: f32,
        /// Seconds.
        0x224 hard_ping_minimum_interrupt_time: f32,
        0x228 hard_death_threshold: f32,
        0x22C feign_death_threshold: f32,
        /// Seconds.
        0x230 feign_death_time: f32,
        /// World units.
        0x234 distance_of_evade_animation: f32,
        /// World units.
        0x238 distance_of_dive_animation: f32,
        0x240 stunned_movement_threshold: f32,
        /// Probability.
        0x244 feign_death_chance: f32,
        0x248 feign_repeat_chance: f32,
        /// Class `actv`.
        0x24C spawned_actor: TagRef,
        0x25C spawned_actor_count_minimum: i16,
        0x25E spawned_actor_count_maximum: i16,
        0x260 spawned_velocity: f32,
        /// Radians per second.
        0x264 aiming_velocity_maximum: f32,
        /// Radians per second squared.
        0x268 aiming_acceleration_maximum: f32,
        0x26C casual_aiming_modifier: f32,
        /// Radians per second.
        0x270 looking_velocity_maximum: f32,
        /// Radians per second squared.
        0x274 looking_acceleration_maximum: f32,
        /// World units.
        0x280 ai_vehicle_avoidance_radius: f32,
        /// World units.
        0x284 ai_danger_radius: f32,
        /// Class `jpt!`: the unit's melee attack.
        0x288 melee_damage: TagRef,
        0x298 motion_sensor_blip_size: BlipSize,
        0x2A8 new_hud_interfaces: Vec<UnitHudInterface>,
        0x2B4 dialogue_variants: Vec<DialogueVariant>,
        /// World units per second (10 on the disc).
        0x2C0 grenade_velocity: f32,
        0x2C4 grenade_type: GrenadeType,
        /// Grenades the unit starts with (4 on the Spartan).
        0x2C6 grenade_count: i16,
        /// Runtime: interrupt time x 30 (9).
        0x2C8 soft_ping_interrupt_ticks: i16,
        0x2CA hard_ping_interrupt_ticks: i16,
        0x2CC powered_seats: Vec<PoweredSeat>,
        0x2D8 weapons: Vec<UnitWeapon>,
        0x2E4 seats: Vec<Seat>,
    }
}

impl UnitBase {
    /// Read the layer from a view of the tag's main struct (at least 0x2F0
    /// bytes).
    pub fn read_view(v: &View<'_>) -> io::Result<UnitBase> {
        <UnitBase as TagStruct>::read(v)
    }

    /// The four incoming function sources, A to D.
    pub fn inputs(&self) -> [UnitInput; 4] {
        [self.input_a, self.input_b, self.input_c, self.input_d]
    }
}

crate::tag_struct! {
    /// A biped's contact point (element 0x40).
    pub struct ContactPoint: 0x40 {
        /// "left foot", "right foot".
        0x20 marker_name: String,
    }
}

crate::tag_struct! {
    /// The biped's own fields (absolute offsets; `SIZE` is the end of the
    /// struct, 0x4F4). The walking speeds are not here: they come from the
    /// globals tag (movement spec).
    pub struct BipedPart: 0x4F4 {
        /// Radians per second (the engine uses it divided by 30).
        0x2F0 moving_turning_speed: f32,
        0x2F4 flags: BipedFlags,
        /// Radians.
        0x2F8 stationary_turning_threshold: f32,
        0x30C input_a: BipedInput,
        0x30E input_b: BipedInput,
        0x310 input_c: BipedInput,
        0x312 input_d: BipedInput,
        /// Class `jpt!`: not read by the engine.
        0x314 dont_use: TagRef,
        // The flying fields (zero on every disc biped).
        /// Radians.
        0x324 bank_angle: f32,
        /// Seconds.
        0x328 bank_apply_time: f32,
        /// Seconds.
        0x32C bank_decay_time: f32,
        0x330 pitch_ratio: f32,
        /// World units per second.
        0x334 max_velocity: f32,
        /// World units per second.
        0x338 max_sidestep_velocity: f32,
        0x33C acceleration: f32,
        0x340 deceleration: f32,
        /// Radians per second.
        0x344 angular_velocity_maximum: f32,
        /// Radians per second squared.
        0x348 angular_acceleration_maximum: f32,
        0x34C crouch_velocity_modifier: f32,
        /// Radians: steeper ground is not walkable (cos of it at 0x4D0).
        0x358 maximum_slope_angle: f32,
        /// Radians.
        0x35C downhill_falloff_angle: f32,
        0x360 downhill_cutoff_angle: f32,
        /// Walking speed factor at and beyond the cutoff going down.
        0x364 downhill_velocity_scale: f32,
        0x368 uphill_falloff_angle: f32,
        0x36C uphill_cutoff_angle: f32,
        0x370 uphill_velocity_scale: f32,
        /// Class `foot`.
        0x38C footsteps: TagRef,
        /// World units per TICK (0.07 = 2.1 per second), before the stun
        /// penalty.
        0x3B4 jump_velocity: f32,
        /// Seconds.
        0x3D4 maximum_soft_landing_time: f32,
        /// Seconds.
        0x3D8 maximum_hard_landing_time: f32,
        /// World units per second.
        0x3DC minimum_soft_landing_velocity: f32,
        0x3E0 minimum_hard_landing_velocity: f32,
        0x3E4 maximum_hard_landing_velocity: f32,
        /// 0 = none.
        0x3E8 death_hard_landing_velocity: f32,
        /// World units above the origin.
        0x400 standing_camera_height: f32,
        0x404 crouching_camera_height: f32,
        /// Seconds.
        0x408 crouch_transition_time: f32,
        /// World units.
        0x424 standing_collision_height: f32,
        0x428 crouching_collision_height: f32,
        0x42C collision_radius: f32,
        /// World units.
        0x458 autoaim_width: f32,
        /// Runtime: cos of the stationary turning threshold.
        0x4C8 runtime_cosine_stationary_turning_threshold: f32,
        /// Runtime, per tick: 1 / (crouch transition time x 30).
        0x4CC runtime_crouch_transition_velocity: f32,
        /// Runtime: cos of the maximum slope angle.
        0x4D0 runtime_minimum_normal_k: f32,
        /// Runtime: -sin(downhill falloff angle).
        0x4D4 runtime_downhill_k0: f32,
        0x4D8 runtime_downhill_k1: f32,
        0x4DC runtime_uphill_k0: f32,
        0x4E0 runtime_uphill_k1: f32,
        0x4E4 runtime_pelvis_node: i16,
        0x4E6 runtime_head_node: i16,
        0x4E8 contact_points: Vec<ContactPoint>,
    }
}

impl BipedPart {
    /// Read the part from a view of the tag's main struct (at least 0x4F4
    /// bytes).
    pub fn read_view(v: &View<'_>) -> io::Result<BipedPart> {
        <BipedPart as TagStruct>::read(v)
    }

    /// The jump velocity in world units per second (the tag stores per
    /// tick).
    pub fn jump_velocity_per_second(&self) -> f32 {
        self.jump_velocity * 30.0
    }
}
