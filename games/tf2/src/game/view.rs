//! TF2's view models (specs/tf2/weapons.md 8.1-8.2, install.md 4;
//! docs/plans/active/tf2.md decision 5): the class's arms
//! (`c_<class>_arms.mdl`, its sequences from the `c_<class>_animations.mdl`
//! it includes) with the `c_` weapon bone-merged onto them by bone name
//! (`map::merge_by_bone_name`), one view model per gun keyed by its id
//! (`load`); their rules on Chimera's view-model driver (`rules`): the
//! draw, idle, fire and the reload's start, shells and end, the
//! Minigun's spin up, spun idle and spin down, from the spec's sequence
//! table (`SEQUENCES`); drawn at `VIEWMODEL_FOV` (`tf2_viewmodel_fov` in
//! an app of several games), never mirrored.
//!
//! The sequences are picked by their activities (the spec's table names
//! each with its sequence): the include's real sequences, which Source's
//! loader puts in place of the arms' same-named 1-frame stubs
//! (install.md 4.3). `tests/it/heavy/guns.rs` checks each activity picks
//! exactly the spec's sequence on its class's arms, and the posed gun
//! sits in front of the eye.
//!
//! Known shortcuts: no view-model bob or sway of TF2's own yet (Source's
//! motion rules are its own spec); the minigun's barrel doesn't spin in
//! first person (its spin is the world model's own sequence). The
//! Minigun's idle `m_idle` as decoded holds the gun upright, low and off
//! the left hand's grip (M2b's "low and angled up"); not the bone merge
//! (its `weapon_bone` itself points up), not the include's root (the same
//! in the spun sequences, which aim right) and not `min_viewmodel_offset`
//! (the minimal view models' only). `m_draw` and `m_spool_down` end in the
//! same pose, so it is in the data as we read it; the Minigun shows its
//! spun pose instead (`SEQUENCES`), raised by its data's `view_offset`,
//! until a capture of the real game settles it.
//!
//! A gun's `[models] view_offset` (Source's forward, left, up) moves its
//! view model in the eye's space (`source_offset`).

use bevy::prelude::*;
use chimera_sim::weapon::{
    WeaponEventKind,
    view_anim::{self, Pick, ViewAnimAppExt, ViewAnimRules, ViewFrame, ViewState},
};

use super::weapons::{MINIGUN, ROCKET_LAUNCHER, SCATTERGUN, SHOTGUN, minigun};
use crate::map::GameFov;

/// TF2's view-model field of view (the spec gives none for TF2's
/// `viewmodel_fov`: 54, as the brief's default and Source's).
pub const VIEWMODEL_FOV: f32 = 54.0;
/// The player's base field of view the view model zooms against.
pub const DEFAULT_FOV: f32 = 75.0;

/// One gun's first-person sequences (8.2): (activity, sequence name).
#[derive(Clone, Copy, Debug)]
pub struct Sequences {
    pub draw: (&'static str, &'static str),
    pub idle: (&'static str, &'static str),
    pub fire: (&'static str, &'static str),
    pub reload_start: Option<(&'static str, &'static str)>,
    pub reload_loop: Option<(&'static str, &'static str)>,
    pub reload_end: Option<(&'static str, &'static str)>,
    pub spin_up: Option<(&'static str, &'static str)>,
    pub spin_idle: Option<(&'static str, &'static str)>,
    pub spin_down: Option<(&'static str, &'static str)>,
}

/// The four guns' sequences on their class's arms (8.2).
pub const SEQUENCES: &[(&str, Sequences)] = &[
    (
        SCATTERGUN,
        Sequences {
            draw: ("ACT_PRIMARY_VM_DRAW", "sg_draw"),
            idle: ("ACT_PRIMARY_VM_IDLE", "sg_idle"),
            fire: ("ACT_PRIMARY_VM_PRIMARYATTACK", "sg_fire"),
            reload_start: Some(("ACT_PRIMARY_RELOAD_START", "sg_reload_start")),
            reload_loop: Some(("ACT_PRIMARY_VM_RELOAD", "sg_reload_loop")),
            reload_end: Some(("ACT_PRIMARY_RELOAD_FINISH", "sg_reload_end")),
            spin_up: None,
            spin_idle: None,
            spin_down: None,
        },
    ),
    (
        ROCKET_LAUNCHER,
        Sequences {
            draw: ("ACT_PRIMARY_VM_DRAW", "dh_draw"),
            idle: ("ACT_PRIMARY_VM_IDLE", "dh_idle"),
            fire: ("ACT_PRIMARY_VM_PRIMARYATTACK", "dh_fire"),
            reload_start: Some(("ACT_PRIMARY_RELOAD_START", "dh_reload_start")),
            reload_loop: Some(("ACT_PRIMARY_VM_RELOAD", "dh_reload_loop")),
            reload_end: Some(("ACT_PRIMARY_RELOAD_FINISH", "dh_reload_finish")),
            spin_up: None,
            spin_idle: None,
            spin_down: None,
        },
    ),
    (
        SHOTGUN,
        Sequences {
            draw: ("ACT_PRIMARY_VM_DRAW", "fj_draw"),
            idle: ("ACT_PRIMARY_VM_IDLE", "fj_idle"),
            fire: ("ACT_PRIMARY_VM_PRIMARYATTACK", "fj_fire"),
            reload_start: Some(("ACT_PRIMARY_RELOAD_START", "fj_reload_start")),
            reload_loop: Some(("ACT_PRIMARY_VM_RELOAD", "fj_reload_loop")),
            reload_end: Some(("ACT_PRIMARY_RELOAD_FINISH", "fj_reload_end")),
            spin_up: None,
            spin_idle: None,
            spin_down: None,
        },
    ),
    // The Minigun's `m_idle` (and the ends of `m_draw` and
    // `m_spool_down`, which settle into it) holds the gun upright below
    // the eye, its barrels at the ceiling and the left hand 28 cm off its
    // grip: decoded per the spec, consistently across those sequences
    //. Until the cause is known, the first person keeps
    // the spun pose: drawn by `m_spool_up` (from that low pose up to the
    // aim), idle and winding down in `m_spool_idle`.
    (
        MINIGUN,
        Sequences {
            draw: ("ACT_PRIMARY_ATTACK_STAND_PREFIRE", "m_spool_up"),
            idle: ("ACT_PRIMARY_VM_SECONDARYATTACK", "m_spool_idle"),
            fire: ("ACT_PRIMARY_VM_PRIMARYATTACK", "m_fire"),
            reload_start: None,
            reload_loop: None,
            reload_end: None,
            spin_up: Some(("ACT_PRIMARY_ATTACK_STAND_PREFIRE", "m_spool_up")),
            spin_idle: Some(("ACT_PRIMARY_VM_SECONDARYATTACK", "m_spool_idle")),
            spin_down: Some(("ACT_PRIMARY_VM_SECONDARYATTACK", "m_spool_idle")),
        },
    ),
];

/// `weapon`'s sequences.
pub fn sequences(weapon: &str) -> Option<&'static Sequences> {
    SEQUENCES.iter().find(|(w, _)| *w == weapon).map(|(_, s)| s)
}

/// The pick of a table entry: its activity (none: nothing).
fn named(s: Option<(&'static str, &'static str)>) -> Pick {
    Pick::Activities(s.map(|(a, _)| a.to_string()).into_iter().collect())
}

/// Its view models' field of view in an app of several games.
pub fn fov() -> GameFov {
    GameFov {
        game: super::GAME,
        fov: VIEWMODEL_FOV,
        default_fov: DEFAULT_FOV,
        follows_player: true,
    }
}

/// `tf2_viewmodel_fov`: TF2's view-model field of view in an app of
/// several games (54 to 110, saved).
pub fn viewmodel_fov_cvar(app: &mut App) {
    view_anim::fov_cvar(
        app,
        "tf2_viewmodel_fov",
        "TF2's view model field of view (horizontal degrees, 54 to 110; zooms with the view).",
        super::GAME,
        (VIEWMODEL_FOV, 110.0),
    );
}

fn draw(s: &ViewState) -> Pick {
    named(sequences(s.weapon).map(|q| q.draw))
}

fn idle(s: &ViewState) -> Pick {
    named(sequences(s.weapon).map(|q| q.idle))
}

fn event(f: &mut ViewFrame, kind: &WeaponEventKind) {
    let Some(q) = sequences(f.state.weapon) else { return };
    match kind {
        WeaponEventKind::Fired { .. } if !f.fired => {
            // The Minigun's fire loop goes on while it fires.
            if f.state.weapon == MINIGUN && f.view.sequence() == Some(q.fire.1) {
                f.fired = true;
                return;
            }
            f.fired = f.attack(&named(Some(q.fire)));
        }
        WeaponEventKind::ReloadStarted => {
            f.attack(&named(q.reload_start));
        }
        WeaponEventKind::ShellInserting => {
            f.attack(&named(q.reload_loop));
        }
        WeaponEventKind::Reloaded => {
            f.attack(&named(q.reload_end));
        }
        WeaponEventKind::Action(minigun::SPIN_UP) => {
            f.attack(&named(q.spin_up));
        }
        WeaponEventKind::Action(minigun::SPIN_DOWN) => {
            f.attack(&named(q.spin_down));
        }
        _ => {}
    }
}

/// TF2's view-model rules.
pub fn rules() -> ViewAnimRules {
    ViewAnimRules {
        draw,
        idle,
        event,
        effect: source_engine::view_anim::event_effect,
    }
}

/// TF2's view-model rules on the driver.
pub fn add_rules(app: &mut App) {
    if !app.is_plugin_added::<view_anim::ViewAnimPlugin>() {
        app.add_plugins(view_anim::ViewAnimPlugin);
    }
    app.view_anim_rules(super::GAME, rules());
}

/// A `c_` weapon's own idle pose (its `idle` sequence's first frame):
/// where its bones without an arms counterpart sit (the Minigun's barrel,
/// install.md 4.4); None without one.
fn own_idle(weapon: &crate::map::MapViewModel) -> Option<Vec<crate::map::anim::BonePose>> {
    let set = weapon.animations.clone()?;
    let idle = set.sequence("idle")?;
    let mut a = crate::map::anim::Animator::new(set);
    a.play(idle, 0.0);
    Some(a.pose(0.0))
}

/// The view models of TF2's guns that `wants`, through `materials`: each
/// the arms of its class with its `c_` model bone-merged, keyed by the
/// gun's id, never mirrored.
pub fn load(
    materials: &mut source_engine::content::material::MaterialLoader,
    wants: &dyn Fn(&str) -> bool,
    into: &mut Vec<crate::map::MapViewModel>,
    warnings: &mut Vec<String>,
) {
    use source_engine::content::props;
    let set = super::weapons::tf2_weapons();
    for (gun, arms) in super::weapons::arms_by_gun() {
        if !wants(gun) {
            continue;
        }
        let Some(data) = set.weapons.iter().find(|w| w.id == gun) else { continue };
        let Some(path) = data.models.view.clone() else { continue };
        let offset = data.models.view_offset;
        let merged = props::load_view_model(materials, &super::sets::arms_model(&arms), gun, true).and_then(|arms| {
            let weapon = props::load_view_model(materials, &path, gun, true)?;
            crate::map::merge_by_bone_name(arms, &weapon, own_idle(&weapon).as_deref())
        });
        match merged {
            Ok(mut v) => {
                v.key = gun.to_string();
                if let Some(o) = offset {
                    // The meshes are in the eye's space of the bind pose:
                    // they move with the root.
                    let o = source_offset(o);
                    v.root.translation += o;
                    for m in &mut v.model.meshes {
                        for p in &mut m.positions {
                            *p = (Vec3::from(*p) + o).to_array();
                        }
                    }
                }
                v.allow_flipping = false;
                into.push(v);
            }
            Err(e) => warnings.push(e),
        }
    }
}

/// A `[models] view_offset` (Source's forward, left, up, units) in the
/// eye's space (metres: right, up, back).
pub fn source_offset(o: [f32; 3]) -> Vec3 {
    Vec3::new(-o[1], o[2], -o[0]) * super::METERS_PER_UNIT
}
