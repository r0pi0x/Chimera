//! TF2's view models (specs/tf2/weapons.md 8.1-8.2, install.md 4;
//! docs/plans/active/tf2.md decision 5): the class's arms
//! (`c_<class>_arms.mdl`, its sequences from the `c_<class>_animations.mdl`
//! it includes) with the `c_` weapon bone-merged onto them by bone name
//! (`map::merge_by_bone_name`), one view model per gun keyed by its id
//! (`load`); their rules on Chimera's view-model driver (`rules`): the
//! draw, idle, fire and the reload (a whole clip, or its start, rounds
//! and end), the Minigun's spin up, spun idle and spin down, the
//! Stickybomb Launcher's charge, from the specs' sequence tables
//! (`SEQUENCES`: weapons.md 8.2, weapons_more.md 9.2); drawn at
//! `VIEWMODEL_FOV` (`tf2_viewmodel_fov` in an app of several games),
//! never mirrored.
//!
//! The sequences are picked by name (the Pyro's `ft_idle` shares its
//! activity with another gun's): the include's real sequences, which
//! Source's loader puts in place of the arms' same-named 1-frame stubs
//! (install.md 4.3). `tests/it/heavy/guns.rs` checks each is there on its
//! class's arms with the spec's activity, and the posed gun sits in front
//! of the eye.
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

use super::weapons::{
    FLAMETHROWER, GRENADE_LAUNCHER, MINIGUN, PISTOL_SCOUT, REVOLVER, ROCKET_LAUNCHER, SCATTERGUN, SHOTGUN, SMG, SNIPER_RIFLE,
    STICKY_LAUNCHER, SYRINGE_GUN, minigun, sticky,
};
use crate::map::GameFov;

/// TF2's view-model field of view (the spec gives none for TF2's
/// `viewmodel_fov`: 54, as the brief's default and Source's).
pub const VIEWMODEL_FOV: f32 = 54.0;
/// The player's base field of view the view model zooms against.
pub const DEFAULT_FOV: f32 = 75.0;

/// One gun's first-person sequences (weapons.md 8.2, weapons_more.md
/// 9.2): (activity, sequence name).
#[derive(Clone, Copy, Debug)]
pub struct Sequences {
    pub draw: (&'static str, &'static str),
    pub idle: (&'static str, &'static str),
    pub fire: (&'static str, &'static str),
    /// The reload's start, or a whole-clip reload.
    pub reload_start: Option<(&'static str, &'static str)>,
    pub reload_loop: Option<(&'static str, &'static str)>,
    pub reload_end: Option<(&'static str, &'static str)>,
    pub spin_up: Option<(&'static str, &'static str)>,
    pub spin_idle: Option<(&'static str, &'static str)>,
    pub spin_down: Option<(&'static str, &'static str)>,
    /// The Stickybomb Launcher's charge (`weapons::sticky::CHARGE`).
    pub charge: Option<(&'static str, &'static str)>,
    /// Its fire sequence loops while it fires (the Minigun's, the Flame
    /// Thrower's): a shot doesn't restart it.
    pub fire_loops: bool,
}

/// A gun with a draw, an idle and a fire sequence, nothing else.
const fn plain(draw: (&'static str, &'static str), idle: (&'static str, &'static str), fire: (&'static str, &'static str)) -> Sequences {
    Sequences {
        draw,
        idle,
        fire,
        reload_start: None,
        reload_loop: None,
        reload_end: None,
        spin_up: None,
        spin_idle: None,
        spin_down: None,
        charge: None,
        fire_loops: false,
    }
}

/// One with a whole-clip reload.
const fn clip(
    draw: (&'static str, &'static str),
    idle: (&'static str, &'static str),
    fire: (&'static str, &'static str),
    reload: (&'static str, &'static str),
) -> Sequences {
    Sequences { reload_start: Some(reload), ..plain(draw, idle, fire) }
}

/// One loading a round at a time.
const fn shells(
    draw: (&'static str, &'static str),
    idle: (&'static str, &'static str),
    fire: (&'static str, &'static str),
    reload: [(&'static str, &'static str); 3],
) -> Sequences {
    Sequences {
        reload_start: Some(reload[0]),
        reload_loop: Some(reload[1]),
        reload_end: Some(reload[2]),
        ..plain(draw, idle, fire)
    }
}

const P_DRAW: &str = "ACT_PRIMARY_VM_DRAW";
const P_IDLE: &str = "ACT_PRIMARY_VM_IDLE";
const P_FIRE: &str = "ACT_PRIMARY_VM_PRIMARYATTACK";
const P_RELOAD: &str = "ACT_PRIMARY_VM_RELOAD";
const S_DRAW: &str = "ACT_SECONDARY_VM_DRAW";
const S_IDLE: &str = "ACT_SECONDARY_VM_IDLE";
const S_FIRE: &str = "ACT_SECONDARY_VM_PRIMARYATTACK";
const S_RELOAD: &str = "ACT_SECONDARY_VM_RELOAD";

/// The guns' sequences on their class's arms (weapons.md 8.2,
/// weapons_more.md 9.2).
pub const SEQUENCES: &[(&str, Sequences)] = &[
    (
        SCATTERGUN,
        shells(
            (P_DRAW, "sg_draw"),
            (P_IDLE, "sg_idle"),
            (P_FIRE, "sg_fire"),
            [("ACT_PRIMARY_RELOAD_START", "sg_reload_start"), (P_RELOAD, "sg_reload_loop"), ("ACT_PRIMARY_RELOAD_FINISH", "sg_reload_end")],
        ),
    ),
    (
        ROCKET_LAUNCHER,
        shells(
            (P_DRAW, "dh_draw"),
            (P_IDLE, "dh_idle"),
            (P_FIRE, "dh_fire"),
            [("ACT_PRIMARY_RELOAD_START", "dh_reload_start"), (P_RELOAD, "dh_reload_loop"), ("ACT_PRIMARY_RELOAD_FINISH", "dh_reload_finish")],
        ),
    ),
    (
        SHOTGUN,
        shells(
            (P_DRAW, "fj_draw"),
            (P_IDLE, "fj_idle"),
            (P_FIRE, "fj_fire"),
            [("ACT_PRIMARY_RELOAD_START", "fj_reload_start"), (P_RELOAD, "fj_reload_loop"), ("ACT_PRIMARY_RELOAD_FINISH", "fj_reload_end")],
        ),
    ),
    // The Minigun's `m_idle` (and the ends of `m_draw` and
    // `m_spool_down`, which settle into it) holds the gun upright below
    // the eye, its barrels at the ceiling and the left hand 28 cm off its
    // grip: decoded per the spec, consistently across those sequences.
    // Until the cause is known, the first person keeps the spun pose:
    // drawn by `m_spool_up` (from that low pose up to the aim), idle and
    // winding down in `m_spool_idle`.
    (
        MINIGUN,
        Sequences {
            spin_up: Some(("ACT_PRIMARY_ATTACK_STAND_PREFIRE", "m_spool_up")),
            spin_idle: Some(("ACT_PRIMARY_VM_SECONDARYATTACK", "m_spool_idle")),
            spin_down: Some(("ACT_PRIMARY_VM_SECONDARYATTACK", "m_spool_idle")),
            fire_loops: true,
            ..plain(
                ("ACT_PRIMARY_ATTACK_STAND_PREFIRE", "m_spool_up"),
                ("ACT_PRIMARY_VM_SECONDARYATTACK", "m_spool_idle"),
                (P_FIRE, "m_fire"),
            )
        },
    ),
    (PISTOL_SCOUT, clip((S_DRAW, "p_draw"), (S_IDLE, "p_idle"), (S_FIRE, "p_fire"), (S_RELOAD, "p_reload"))),
    (SMG, clip((S_DRAW, "smg_draw"), (S_IDLE, "smg_idle"), (S_FIRE, "smg_fire"), (S_RELOAD, "smg_reload"))),
    // No sequence for the scope: the view model is hidden while scoped.
    (SNIPER_RIFLE, plain((P_DRAW, "draw"), (P_IDLE, "idle"), (P_FIRE, "fire"))),
    (REVOLVER, clip((S_DRAW, "draw"), (S_IDLE, "idle"), (S_FIRE, "fire"), (S_RELOAD, "reload"))),
    // The Grenade Launcher animates as a secondary, the Stickybomb
    // Launcher as a primary (`anim_slot`, 1).
    (
        GRENADE_LAUNCHER,
        shells(
            (S_DRAW, "g_draw"),
            (S_IDLE, "g_idle"),
            (S_FIRE, "g_fire"),
            [("ACT_SECONDARY_RELOAD_START", "g_reload_start"), (S_RELOAD, "g_reload_loop"), ("ACT_SECONDARY_RELOAD_FINISH", "g_reload_end")],
        ),
    ),
    (
        STICKY_LAUNCHER,
        Sequences {
            charge: Some(("ACT_PRIMARY_VM_PULLBACK", "sb_autofire")),
            ..shells(
                (P_DRAW, "sb_draw"),
                (P_IDLE, "sb_idle"),
                (P_FIRE, "sb_fire"),
                [("ACT_PRIMARY_RELOAD_START", "sb_reload_start"), (P_RELOAD, "sb_reload_loop"), ("ACT_PRIMARY_RELOAD_FINISH", "sb_reload_end")],
            )
        },
    ),
    // `ft_idle` shares its activity with the Dragon's Fury's `df_idle`:
    // picked by name (open question 17).
    (
        FLAMETHROWER,
        Sequences { fire_loops: true, ..plain((P_DRAW, "ft_draw"), (P_IDLE, "ft_idle"), (P_FIRE, "ft_fire")) },
    ),
    (SYRINGE_GUN, clip((P_DRAW, "sg_draw"), (P_IDLE, "sg_idle"), (P_FIRE, "sg_fire"), (P_RELOAD, "sg_reload"))),
];

/// `weapon`'s sequences.
pub fn sequences(weapon: &str) -> Option<&'static Sequences> {
    SEQUENCES.iter().find(|(w, _)| *w == weapon).map(|(_, s)| s)
}

/// The pick of a table entry: its sequence by name (none: nothing).
fn named(s: Option<(&'static str, &'static str)>) -> Pick {
    Pick::Names(s.map(|(_, n)| n.to_string()).into_iter().collect())
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
            // A looping fire sequence goes on while it fires.
            if q.fire_loops && f.view.sequence() == Some(q.fire.1) {
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
        WeaponEventKind::Action(sticky::CHARGE) => {
            f.attack(&named(q.charge));
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
