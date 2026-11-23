//! DoD:S's movement (`dods:movement`, specs/dods/movement.md): Source's
//! player movement (`source_engine::source_movement`) with DoD:S's
//! measured numbers (`config`): run speed 220 in every direction (no
//! back-pedal penalty), accelerate and airaccelerate 10, gravity 800,
//! step 18, the measured ground friction (a drop of max(speed, 50) x 8 per
//! second: friction 8 over a control speed of 50, the same law), the
//! 32 x 32 x 72 standing box with its eye at 58, ducked 45 high with its
//! eye at 34 over ≈0.42 s, ducked a third of the speed. Its box is the
//! envelope its maps carry (`chimera_core::envelope`, registered by
//! `source_movement`), so other games' characters fit on its maps and its
//! own on theirs.
//!
//! With it go a DoD:S player's own parts, on and off in one move with the
//! movement (`put_on`, `leave`, `take_off`): its stance (`stance`: prone
//! and the deployed machine guns, handed to Source's movement as its
//! `SourceStance` each tick, before the step) and its stamina (`stamina`).
//! `+alt1` (`stance::PRONE_ACTION`) is a named action.
//!
//! Known shortcuts:
//! - The jump is CS:S's stand-in (unmeasured: movement.md, open question
//!   4), with no stamina cost; no sprint (unmeasured: `stamina`); ladders,
//!   water and fall damage are the shared code's.

use bevy::prelude::*;
use source_engine::source_movement::{
    SourceMovement, SourceMovementAppExt, SourceMovementConfig, SourceMovementConfigs, SourceMovementStep, SourceStance,
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
pub fn config() -> SourceMovementConfig {
    SourceMovementConfig {
        accelerate: 10.0,
        airaccelerate: 10.0,
        // The measured law (movement.md, "Ground speed, acceleration and
        // friction"): drop = max(v, 50) x 8 x dt.
        friction: 8.0,
        stopspeed: 50.0,
        gravity: 800.0,
        maxspeed: RUN_SPEED,
        stepsize: 18.0,
        // Unmeasured: CS:S's √(2 · 800 · 57) stand-in.
        jump_impulse: 301.993,
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
        // 28 ticks of 0.015 s.
        duck_time: 0.42,
        jump_stamina: 0.0,
        enable_bunnyhopping: false,
        bunnyhop_cap: 0.0,
        // `sv_backspeed` 0: no back-pedal penalty.
        back_speed: 1.0,
        ..SourceMovementConfig::shared_code()
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
    if app.world().get_resource::<SourceMovementConfigs>().is_some_and(|c| c.get(ID).is_some()) {
        return;
    }
    use crate::core::{ActionAppExt, PredictedAppExt};
    app.source_movement(ID, Some(config()), Some(super::GAME))
        .intent_action(super::stance::PRONE_ACTION)
        .predicted_net::<DodsStance>()
        .predicted_net::<SourceStance>()
        .add_systems(crate::core::Predict::Movement, super::stance::step.before(SourceMovementStep));
    if let Some(mut registry) = app.world_mut().get_resource_mut::<crate::slots::MovementRegistry>()
        && let Some(m) = registry.0.iter_mut().find(|m| m.id == ID)
    {
        m.insert = put_on;
        m.remove = leave;
        m.leave = Some(take_off);
    }
}

/// The player's own parts beside Source's marker.
type Parts = (DodsStance, SourceStance, DodsStamina);

/// `dods:movement`'s insert: the marker, the stance and the stamina, in
/// one move.
fn put_on(e: &mut EntityWorldMut, slot: MovementSlot) {
    e.insert((SourceMovement::default(), slot, DodsStance::default(), SourceStance::default(), DodsStamina::default()));
}

/// `dods:movement`'s remove: the marker with the parts, in one move.
fn leave(e: &mut EntityWorldMut) {
    e.remove::<(SourceMovement, Parts)>();
}

/// `dods:movement`'s leave for another movement on Source's marker: the
/// parts off (a prone player's stance with them), in one move.
fn take_off(e: &mut EntityWorldMut) {
    e.remove::<Parts>();
}
