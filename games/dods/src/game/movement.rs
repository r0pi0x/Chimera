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
//! Known shortcuts (M0; M2 is DoD:S's own movement):
//! - No prone (hull 24, eye 10, 50 u/s crawl, 1.2 s to go down), stamina
//!   sprint or deployed MGs: Chimera's movement has stand and duck only
//!   (docs/plans/active/dods.md, "Risks").
//! - The jump is CS:S's stand-in (unmeasured: movement.md, open question
//!   4), with no stamina cost; ladders, water and fall damage are the
//!   shared code's.

use bevy::prelude::*;
use source_engine::source_movement::{SourceMovementAppExt, SourceMovementConfig, SourceMovementConfigs};

use crate::slots::Loadout;

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
    app.source_movement(ID, Some(config()), Some(super::GAME));
}
