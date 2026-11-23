//! DoD:S's view models (specs/dods/weapons.md, "The scripts' fields":
//! `ViewModelFOV` 45 on every gun): the generic driver's rules for its
//! guns (`weapon::view_anim`), which pick the Source shared weapon base's
//! activities its view models carry (`source_engine::view_anim`: the draw,
//! idle, each shot, a dry fire, a reload, a swing's hit or miss), with the
//! models' own sound events (the reload's clip out and in, the bolt) heard
//! by the holder and their muzzle flashes and brass; the view models'
//! field of view in an app of several games (`fov`).
//!
//! Known shortcuts (unmeasured, weapons.md): no view-model bob or sway
//! (no motion rules); the sights' and the prone offsets
//! (`vm_ironsight_offset`, `vm_prone_offset`) are not applied; the base
//! field of view the view model follows is the player's.

use bevy::prelude::*;
use chimera_sim::weapon::{
    WeaponEventKind,
    view_anim::{self, Pick, ViewAnimAppExt, ViewAnimRules, ViewFrame},
};
use source_engine::view_anim::{self as source, DRAW, DRYFIRE, HIT_CENTER, IDLE, MISS_CENTER, PRIMARY, SECONDARY};

use crate::map::GameFov;

/// The scripts' `ViewModelFOV` (45 on every gun, weapons.md).
pub const VIEWMODEL_FOV: f32 = 45.0;

fn acts(list: &[&str]) -> Pick {
    Pick::activities(list)
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
        other => source::common_event(f, other),
    }
}

/// DoD:S's view-model rules.
pub fn rules() -> ViewAnimRules {
    ViewAnimRules {
        draw: |_| acts(&[DRAW, IDLE]),
        idle: |_| acts(&[IDLE]),
        event,
        effect: source::event_effect_with_sounds,
        draw_rate: 1.0,
        others_reload: None,
    }
}

/// Its view models' field of view in an app of several games.
pub fn fov() -> GameFov {
    GameFov { game: super::GAME, fov: VIEWMODEL_FOV, default_fov: 90.0, follows_player: true }
}

/// DoD:S's view-model rules on the driver (once per app).
pub fn add_rules(app: &mut App) {
    if !app.is_plugin_added::<view_anim::ViewAnimPlugin>() {
        app.add_plugins(view_anim::ViewAnimPlugin);
    }
    app.view_anim_rules(super::GAME, rules());
}
