//! HL2:DM's view models (specs/hl2dm/player_models.md 12, weapons.md 7):
//! its rules on Chimera's view-model driver (`weapon::view_anim`) with
//! Source's activities (`source_engine::view_anim`): the draw, idle, a
//! fire sequence per shot (the shotgun's double shot its secondary
//! attack), the dry fire, the reload (the shotgun's start, a shell, its
//! finish, and the pump), a melee hit or miss, the gravity gun's punt or
//! launch (its secondary attack) and grab or drop (its primary); how they move
//! (`MOTION`: HL2's fixed bob and spring sway with pitch offsets,
//! `source_engine::view_motion`); how they are drawn (`settings`: the view
//! model at 54 degrees minus how far the player's view is zoomed from its
//! base, which is `fov_desired` 75-90 (`GameFov::follows_player`, kept by
//! the client; 75 until it does); right-handed, never
//! mirrored: each is loaded with flipping off).
//!
//! A shot's fire sequence follows its place in a series (`fire_order`,
//! weapons.md 7.1, 7.3): the pistol's shots less than 0.5 s apart count up
//! through `ACT_VM_PRIMARYATTACK` and `ACT_VM_RECOIL1`-`3`; the SMG1's and
//! AR2's first two shots since attack was pressed play the first, then
//! `RECOIL1`-`3`, the last held. A model without one plays the one before.
//!
//! The AR2's vents are `weapons::vent`'s (its `VentPoses` pose parameter).
//!
//! Known shortcuts: the machine guns' count is reset at a release but not
//! at a deploy with attack held (weapons.md 2).

use bevy::prelude::*;
use chimera_sim::weapon::{
    WeaponEventKind,
    view_anim::{self, Pick, ViewAnimAppExt, ViewAnimRules, ViewFrame},
};
use source_engine::{
    view_anim::{
        self as source, DRAW, DRYFIRE, HIT_CENTER, IDLE, MISS_CENTER, PRIMARY, RECOIL1, RECOIL2, RECOIL3, SECONDARY, SHOTGUN_PUMP,
    },
    view_motion::{self, MotionAppExt, MotionRules, Sway, ViewMotion},
};

use crate::map::{DriveAnimation, GameFov, ViewModelSettings};

/// `viewmodel_fov` (54) and the player's base FOV (`fov_desired`: 75 by
/// default, the starting value of `default_fov`).
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
        // The base follows `fov_desired` (12.4): 54 unzoomed whatever it is.
        follows_player: true,
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

/// The pistol's shots closer than this count as a series (weapons.md 7.1).
pub const PISTOL_SERIES: f64 = 0.5;

/// A shot's fire sequences, the first the model has, by its place in a
/// series: `shots` since attack was pressed (the machine guns'), `series`
/// the pistol's count of shots close together (0 for the first).
pub fn fire_order(weapon: &str, shots: u32, series: u32) -> &'static [&'static str] {
    use super::weapons::{AR2, PISTOL, SMG1};
    const ORDER: [&[&str]; 4] = [&[PRIMARY], &[RECOIL1, PRIMARY], &[RECOIL2, RECOIL1, PRIMARY], &[RECOIL3, RECOIL2, RECOIL1, PRIMARY]];
    let i = if weapon == PISTOL {
        series
    } else if weapon == SMG1 || weapon == AR2 {
        shots.saturating_sub(2)
    } else {
        0
    };
    ORDER[i.min(3) as usize]
}

fn event(f: &mut ViewFrame, kind: &WeaponEventKind) {
    match kind {
        WeaponEventKind::Fired { mode, .. } if !f.fired => {
            let series = if f.now - f.dice.last_attack < PISTOL_SERIES { f.dice.series + 1 } else { 0 };
            f.dice.series = series;
            let order: &[&str] = if *mode > 0 { &[SECONDARY, PRIMARY] } else { fire_order(f.state.weapon, f.state.shots, series) };
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
    use super::*;
    use crate::game::weapons::{AR2, MAGNUM, PISTOL, SMG1};

    /// weapons.md 7.1 and 7.3: the pistol's quick shots count up from the
    /// first; the machine guns' first two shots play the first sequence,
    /// then the recoils, the last held; others always the first.
    #[test]
    fn a_series_of_shots_plays_the_recoils() {
        assert_eq!(fire_order(PISTOL, 1, 0)[0], PRIMARY);
        assert_eq!(fire_order(PISTOL, 1, 1)[0], RECOIL1);
        assert_eq!(fire_order(PISTOL, 1, 7)[0], RECOIL3);
        let smg: Vec<&str> = (1..=6).map(|n| fire_order(SMG1, n, 0)[0]).collect();
        assert_eq!(smg, [PRIMARY, PRIMARY, RECOIL1, RECOIL2, RECOIL3, RECOIL3]);
        assert_eq!(fire_order(AR2, 3, 0), &[RECOIL1, PRIMARY]);
        assert_eq!(fire_order(MAGNUM, 5, 5), &[PRIMARY]);
    }

    #[test]
    fn hl2s_view_model_fov_is_54_at_75() {
        let s = super::settings();
        assert_eq!((s.fov, s.default_fov), (54.0, 75.0));
    }
}
