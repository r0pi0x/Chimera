//! DoD:S's movement (`dods:movement`, specs/dods/movement.md): Source's
//! player movement (`source_engine::source_movement`) with DoD:S's
//! measured numbers (`config`): run speed 220 in every direction (no
//! back-pedal penalty), accelerate and airaccelerate 10, gravity 800,
//! step 18, the measured ground friction (a drop of max(speed, 50) x 8 per
//! second: friction 8 over a control speed of 50, the same law), the
//! 32 x 32 x 72 standing box with its eye at 58, ducked 45 high with its
//! eye at 34 over 0.4 s (the measured 28 ticks: the shared code's duck),
//! ducked a third of the speed, up again in 0.21 s (the unduck's timer
//! counting its release tick: the measured eye curve), the jump 262.33
//! units/s standing and 268.33 ducked (apex 43 and 45; no stamina cost;
//! one impulse, `config`; the spawn spots' walk climbs 0.8 m as CS:S's,
//! `SpawnClimbs`, and meets the physics props by turbo physics' rule,
//! `prop_physics::PlayerProps::Turbo`: specs/dods/physics_props.md),
//! and the ground speed clamped to the tick's cap (`ground_clamp`: the
//! stamina's caps and the slow cap the speed itself, not the wish:
//! movement.md, "Stamina and sprint"). Its box is the
//! envelope its maps carry (`chimera_core::envelope`, registered by
//! `source_movement`), so other games' characters fit on its maps and its
//! own on theirs.
//!
//! With it go a DoD:S player's own parts, on and off in one move with the
//! movement (`put_on`, `leave`, `take_off`): its stance (`stance`: prone
//! and the deployed machine guns, handed to Source's movement as its
//! `MoveStance` each tick, before the step, with the tick's speed cap)
//! and its stamina and sprint (`stamina`, after the stance: the cap reads
//! the stamina the tick started with).
//! `+alt1` (`stance::PRONE_ACTION`) is a named action.
//!
//! Known shortcuts:
//! - Ladders, water and fall damage are the shared code's (movement.md,
//!   open question 7); the caps clamp the ground speed only (in the air
//!   the air move's own limits).
//! - Players meet physics props as CS:S's (Source's movement's push-away
//!   and shadow: soft multiplayer props, light `prop_physics` shoved),
//!   while DoD:S's turbo physics makes every `prop_physics` and every
//!   settled multiplayer prop a wall (specs/dods/physics_props.md; only the
//!   spots' walker follows it, `PlayerProps::Turbo`). Waits on the shared
//!   movement's move (docs/plans/active/shared-movement.md, S4).

use bevy::prelude::*;
use source_engine::source_movement::{
    QuakeMovement, SourceMovementAppExt, QuakeConfig, QuakeConfigs, MovementStep, MoveStance,
};

use super::{stamina::DodsStamina, stance::DodsStance};
use crate::slots::{Loadout, MovementSlot};

/// The movement's id.
pub const ID: &str = "dods:movement";

/// Run speed (movement.md, "Constants"), units/s.
pub const RUN_SPEED: f32 = 220.0;

/// The standing box's height and eye, the ducked box's and its eye
/// (movement.md, "Constants"), units.
pub const STAND_HEIGHT: f32 = 72.0;
pub const EYE_STAND: f32 = 58.0;
pub const DUCK_HEIGHT: f32 = 45.0;
pub const EYE_DUCK: f32 = 34.0;

/// DoD:S's movement config (movement.md, "Constants").
pub fn config() -> QuakeConfig {
    QuakeConfig {
        accelerate: 10.0,
        airaccelerate: 10.0,
        // The measured law (movement.md, "Ground speed, acceleration and
        // friction"): drop = max(v, 50) x 8 x dt.
        friction: 8.0,
        stopspeed: 50.0,
        gravity: 800.0,
        // `sv_maxspeed`: no lower than the sprint's top (`stamina`).
        maxspeed: super::stamina::SPRINT_TOP,
        stepsize: 18.0,
        // Measured (movement.md, "Jumping"): 262.33 standing, 268.33 ducked
        // as the vertical speed the jump leaves; Source's jump adds its
        // impulse to the half tick of gravity already taken standing and
        // sets it ducked, so one impulse of 268.33 gives both (250.33 and
        // 256.33 after the tick, apex 43.01 and 45).
        jump_impulse: 268.33,
        player_maxspeed: RUN_SPEED,
        client_maxspeed: RUN_SPEED,
        key_speed: 450.0,
        duck_speed: 1.0 / 3.0,
        walk_speed: 1.0,
        stand_height: STAND_HEIGHT,
        duck_height: DUCK_HEIGHT,
        eye_stand: EYE_STAND,
        eye_duck: EYE_DUCK,
        half_width: 16.0,
        // The measured eye curve and the box on the 28th tick (the press
        // tick 0 s in); the unduck 0.2 s with its release tick counted:
        // the box stands on the 14th (0.21 s).
        // The third applies from the duck key's first tick (measured:
        // +11 u/s from rest).
        duck_slows_everywhere: true,
        duck_time: 0.4,
        unduck_counts_first_tick: true,
        ground_clamp: true,
        jump_stamina: 0.0,
        enable_bunnyhopping: false,
        bunnyhop_cap: 0.0,
        // `sv_backspeed` 0: no back-pedal penalty.
        back_speed: 1.0,
        ..QuakeConfig::reference()
    }
}

/// `dods:movement`, and the match's movement.
pub struct DodsMovementPlugin;

impl Plugin for DodsMovementPlugin {
    fn build(&self, app: &mut App) {
        add_movement(app);
        app.insert_resource(Loadout { movement: ID });
    }
}

/// `dods:movement` on a composed game's simulation (a party's) without
/// making it the match's; once.
pub fn add_movement(app: &mut App) {
    if app.world().get_resource::<QuakeConfigs>().is_some_and(|c| c.get(ID).is_some()) {
        return;
    }
    use crate::core::{ActionAppExt, PredictedAppExt};
    app.source_movement(ID, Some(config()), Some(super::GAME))
        .intent_action(super::stance::PRONE_ACTION)
        .predicted_net::<DodsStance>()
        .predicted_net::<MoveStance>()
        .predicted_net::<DodsStamina>()
        .add_systems(
            crate::core::Predict::Movement,
            ((super::stance::step, super::stamina::step).chain().before(MovementStep), super::stance::yaw_limits.after(super::stance::step)),
        );
    // Spawn spots where players get to: up ledges a jump takes (apex 43
    // units standing, 45 ducked: movement.md "Jumping"), CS:S's 0.8 m.
    {
        use crate::map::SpawnClimbAppExt;
        app.add_spawn_climb(super::GAME, 0.8);
    }
    // Its shipped config's turbo physics (specs/dods/physics_props.md):
    // the spots' walk goes through only debris and the multiplayer props
    // still asleep since load; every other physics prop is a wall.
    {
        use crate::map::prop_physics::{PlayerProps, PlayerPropsAppExt};
        app.add_player_props(super::GAME, PlayerProps::Turbo);
    }
    if let Some(mut registry) = app.world_mut().get_resource_mut::<crate::slots::MovementRegistry>()
        && let Some(m) = registry.0.iter_mut().find(|m| m.id == ID)
    {
        m.insert = put_on;
        m.remove = leave;
        m.leave = Some(take_off);
    }
}

/// The player's own parts beside Source's marker.
type Parts = (DodsStance, MoveStance, DodsStamina);

/// `dods:movement`'s insert: the marker, the stance and the stamina, in
/// one move.
fn put_on(e: &mut EntityWorldMut, slot: MovementSlot) {
    e.insert((QuakeMovement::default(), slot, DodsStance::default(), MoveStance::default(), DodsStamina::default()));
}

/// `dods:movement`'s remove: the marker with the parts, in one move.
fn leave(e: &mut EntityWorldMut) {
    e.remove::<(QuakeMovement, Parts)>();
}

/// `dods:movement`'s leave for another movement on Source's marker: the
/// parts off (a prone player's stance with them), in one move.
fn take_off(e: &mut EntityWorldMut) {
    e.remove::<Parts>();
}
