//! HL2:DM's view models (specs/hl2dm/player_models.md 12, weapons.md 7):
//! its rules on Chimera's view-model driver (`weapon::view_anim`) with
//! Source's activities (`source_engine::view_anim`): the draw, idle, a
//! fire sequence per shot (the shotgun's double shot its secondary
//! attack), the dry fire, the reload (the shotgun's start, a shell, its
//! finish, and the pump), a melee hit or miss, the gravity gun's punt or
//! launch (its secondary attack) and grab or drop (its primary); how they move
//! (`MOTION`: HL2's fixed bob and spring sway with pitch offsets,
//! `source_engine::view_motion`); how they are drawn (`settings`: the view
//! model at 54 degrees, the player's default 75; right-handed, never
//! mirrored: each is loaded with flipping off).
//!
//! Known shortcuts: the pistol's and machine guns' series of fire
//! sequences (`ACT_VM_RECOIL1`-`3` after the first shots) play the first
//! one each time; the AR2's vent pose parameter isn't set (both
//! cosmetic); the shells aren't drawn (M10: `flashes`, `effects`).

use bevy::prelude::*;
use chimera_sim::weapon::{
    WeaponEventKind,
    view_anim::{self, Pick, ViewAnimAppExt, ViewAnimRules, ViewFrame},
};
use source_engine::{
    view_anim::{self as source, DRAW, DRYFIRE, HIT_CENTER, IDLE, MISS_CENTER, PRIMARY, SECONDARY, SHOTGUN_PUMP},
    view_motion::{self, MotionAppExt, MotionRules, Sway, ViewMotion},
};

use crate::map::{DriveAnimation, GameFov, ViewModelSettings};

/// `viewmodel_fov` (54) and the player's base FOV (`fov_desired` 75).
pub const VIEWMODEL_FOV: f32 = 54.0;
pub const DEFAULT_FOV: f32 = 75.0;

/// HL2:DM's view-model motion (player_models.md 12.2-12.3).
pub const MOTION: MotionRules = MotionRules {
    cycle: Some((0.45, 0.5)),
    forward: 0.1,
    right: 0.8,
    slew: false,
    sway: Sway::Spring,
};

/// The view-model settings HL2:DM starts with.
pub fn settings() -> ViewModelSettings {
    ViewModelSettings {
        fov: VIEWMODEL_FOV,
        default_fov: DEFAULT_FOV,
        right_hand: 1,
        draw: 1,
    }
}

/// Its view models' field of view in an app of several games.
pub fn fov() -> GameFov {
    GameFov {
        game: super::GAME,
        fov: VIEWMODEL_FOV,
        default_fov: DEFAULT_FOV,
    }
}

fn acts(list: &[&str]) -> Pick {
    Pick::activities(list)
}

/// HL2:DM's view-model rules.
pub fn rules() -> ViewAnimRules {
    ViewAnimRules {
        draw: |_| acts(&[DRAW, IDLE]),
        idle: |_| acts(&[IDLE]),
        event,
        effect: super::flashes::event_effect,
    }
}

fn event(f: &mut ViewFrame, kind: &WeaponEventKind) {
    match kind {
        WeaponEventKind::Fired { mode, .. } if !f.fired => {
            let order: &[&str] = if *mode > 0 { &[SECONDARY, PRIMARY] } else { &[PRIMARY] };
            f.fired = f.attack(&acts(order));
        }
        WeaponEventKind::DryFire => {
            f.attack(&acts(&[DRYFIRE]));
        }
        WeaponEventKind::Swing { hit, .. } => {
            let order: &[&str] = if *hit { &[HIT_CENTER, PRIMARY] } else { &[MISS_CENTER, HIT_CENTER, PRIMARY] };
            f.attack(&acts(order));
        }
        // The gravity gun: a punt or launch, a grab or drop.
        WeaponEventKind::Action(super::weapons::physcannon::PUNT) => {
            f.attack(&acts(&[SECONDARY, PRIMARY]));
        }
        WeaponEventKind::Action(super::weapons::physcannon::PICKUP) => {
            f.attack(&acts(&[PRIMARY]));
        }
        WeaponEventKind::Action(super::weapons::shotgun::PUMP) => {
            f.attack(&acts(&[SHOTGUN_PUMP]));
        }
        other => source::common_event(f, other),
    }
}

/// HL2:DM's view-model rules and motion on the driver, for an app of
/// several games (whose motion system runs for every game's).
pub fn add_rules(app: &mut App) {
    if !app.is_plugin_added::<view_anim::ViewAnimPlugin>() {
        app.add_plugins(view_anim::ViewAnimPlugin);
    }
    app.view_anim_rules(super::GAME, rules()).view_motion_rules(super::GAME, MOTION);
}

/// HL2:DM's view models in its own app: the rules, the motion's system
/// and its settings.
pub struct Hl2ViewPlugin;

impl Plugin for Hl2ViewPlugin {
    fn build(&self, app: &mut App) {
        add_rules(app);
        app.insert_resource(settings())
            .init_resource::<ViewMotion>()
            .add_systems(Update, view_motion::update.in_set(DriveAnimation));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn hl2s_view_model_fov_is_54_at_75() {
        let s = super::settings();
        assert_eq!((s.fov, s.default_fov), (54.0, 75.0));
    }
}
