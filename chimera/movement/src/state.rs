//! The movement's per-character state (`QuakeMovement`, predicted and sent
//! as it is: its field order is its wire form) and `SettleOnGround`.

use bevy::prelude::*;

use crate::mover::lines::ForcedMove;

/// A character that rests on the floor it stands on (hostages): when the
/// ground test finds a floor within its reach (2 units), the feet go down
/// onto it. A player doesn't (Source's movement keeps the gap); CS:S's
/// hostages, with their own movement, land 1/32 above the floor from any
/// height (measured, objectives.md 9 "Spawn").
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct SettleOnGround;

/// Per-character movement state. Units and seconds as in the spec.
#[derive(Component, Clone, Debug, serde::Serialize, serde::Deserialize)]
#[component(storage = "SparseSet")]
pub struct QuakeMovement {
    pub on_ground: bool,
    pub ground_normal: Vec3,
    pub jump_held: bool,
    pub(crate) duck_held: bool,
    /// Small box in use.
    pub ducked: bool,
    /// In a duck or unduck transition.
    pub ducking: bool,
    pub(crate) duck_timer: f32,
    /// Eye height above the feet.
    pub eye: f32,
    pub fall_speed: f32,
    pub surface_friction: f32,
    /// Speed of the last landing (for fall damage and sounds), units/s.
    pub last_landing_speed: f32,
    /// Stuck recovery: ticks run, the next nudge to try (0 = not stuck),
    /// and when the last nudge was tried (seconds of movement time).
    pub(crate) ticks: u64,
    pub(crate) nudge: usize,
    pub(crate) time: f32,
    pub(crate) last_nudge: f32,
    /// Feet at the end of the last tick, to notice teleports.
    pub(crate) last_feet: Option<Vec3>,
    /// The moving solid stood on (`MovingSolid`), if any. Not sent to a
    /// predicting client (entity ids differ).
    #[serde(skip)]
    pub ground_entity: Option<Entity>,
    /// Jump stamina left, ms (CS:S).
    pub stamina: f32,
    /// On a ladder: its surface normal (Source axes, out of the ladder).
    pub ladder: Option<Vec3>,
    /// 0 dry, 1 feet, 2 waist, 3 eyes.
    pub water_level: u8,
    pub in_slime: bool,
    /// Water jump: time left (ms) and the horizontal velocity it holds.
    pub water_jump_time: f32,
    pub(crate) water_jump_vel: Vec3,
    /// Footsteps (sound spec 3): time to the next step (ms), which foot
    /// steps next, the surface last stood on, swim-stroke timer (ms).
    pub step_timer: f32,
    pub(crate) step_left: bool,
    pub surface: Option<String>,
    pub(crate) swim_timer: f32,
    /// The punch angle's roll (degrees) and its velocity: hard landings
    /// and wall slams kick it (movement spec, "Falling and landing").
    pub punch_roll: f32,
    pub(crate) punch_roll_vel: f32,
    /// CS:S's velocity modifier (weapons.md M18): a bullet hit sets it
    /// (0.5 or 0.65, `games::cs_source::hit_reaction`), the max speed is
    /// multiplied by it, and it climbs back to 1 by 1/3 a second.
    pub velocity_modifier: f32,
    /// Just switched to from another movement (`seat`): this tick's step
    /// first frees the hull and, when it stood on the ground (true), puts
    /// it down on the floor below.
    #[serde(default)]
    pub seat: Option<bool>,
    /// On a `func_useableladder` (`useable_ladders`): its index in
    /// `UseableLadders`.
    #[serde(default)]
    pub useable: Option<u32>,
    /// Being pulled onto or off one (`lines::ForcedMove`).
    #[serde(default)]
    pub forced: Option<ForcedMove>,
    /// `+use` was held last tick (a fresh press mounts and dismounts).
    #[serde(default)]
    pub use_held: bool,
    /// Air jumps made and duck releases in the air since leaving the
    /// ground (`QuakeConfig::air_jumps`, `air_ducks_max`).
    #[serde(default)]
    pub air_jumps_used: u8,
    #[serde(default)]
    pub air_ducks: u8,
    /// The duck key as last pressed (before any filter), and the re-duck
    /// delay left (s, `QuakeConfig::reduck_delay`).
    #[serde(default)]
    pub duck_key: bool,
    #[serde(default)]
    pub reduck_timer: f32,
    /// Held in a game's low stance last tick (`MoveStance::height`),
    /// and whether it was a crouch (`MoveStance::crouch`).
    #[serde(default)]
    pub stance_low: bool,
    #[serde(default)]
    pub stance_crouch: bool,
}

impl Default for QuakeMovement {
    fn default() -> Self {
        Self {
            on_ground: false,
            ground_normal: Vec3::Z,
            jump_held: false,
            duck_held: false,
            ducked: false,
            ducking: false,
            duck_timer: 0.0,
            eye: 64.0,
            fall_speed: 0.0,
            surface_friction: 1.0,
            last_landing_speed: 0.0,
            ticks: 0,
            nudge: 0,
            time: 0.0,
            last_nudge: f32::NEG_INFINITY,
            last_feet: None,
            ground_entity: None,
            stamina: 0.0,
            ladder: None,
            water_level: 0,
            in_slime: false,
            water_jump_time: 0.0,
            water_jump_vel: Vec3::ZERO,
            step_timer: 0.0,
            step_left: false,
            surface: None,
            swim_timer: 0.0,
            punch_roll: 0.0,
            punch_roll_vel: 0.0,
            velocity_modifier: 1.0,
            seat: None,
            useable: None,
            forced: None,
            use_held: false,
            air_jumps_used: 0,
            air_ducks: 0,
            duck_key: false,
            reduck_timer: 0.0,
            stance_low: false,
            stance_crouch: false,
        }
    }
}
