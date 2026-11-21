//! TF2's view models (specs/tf2/weapons.md 8.1-8.2, install.md 4;
//! docs/plans/active/tf2.md decision 5): the class's arms
//! (`c_<class>_arms.mdl`, its sequences from the `c_<class>_animations.mdl`
//! it includes) with the `c_` weapon bone-merged onto them by bone name
//! (`map::merge_by_bone_name`), one view model per gun keyed by its id on
//! its own class's arms, and one per other class that can hold it
//! (data/classes.toml) keyed `<gun>#<class>` (Chimera's
//! `view_anim::variant_key`) when that class's arms are here (`load`);
//! (a bone of the gun that class's idle poses away from the hand stays with the
//! gun's own: `unmerge_undriven`); the holder's class picks it
//! (`ViewVariant`, `view_variants`: the
//! rung's drawn class, `classes::Classes::held`); their rules on
//! Chimera's view-model driver (`rules`): the
//! draw, idle, fire and the reload (a whole clip, or its start, rounds
//! and end), the Minigun's spin up, spun idle and spin down, the
//! Stickybomb Launcher's charge, from the specs' sequence tables
//! (`SEQUENCES`, and for a gun in another class's hands
//! `CLASS_SEQUENCES`: weapons.md 8.2, weapons_more.md 9.2); drawn at
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
//! The view model bobs as CS:S's does, with `cl_bobcycle` 0.8 and
//! `cl_bobup` 0.5 fixed (cheats in TF2), the speed's change slew-limited,
//! one bob for all the player's guns, and never sways (`MOTION` on
//! `source_engine::view_motion`, specs/tf2/viewmodel.md 3-4). A draw
//! plays at 1.34 x and holds its last frame until its length at its own
//! rate before the idle (`DRAW_RATE`, viewmodel.md 6).
//!
//! The Minigun's barrel spins in first person (`spin_barrels`,
//! minigun.md 5): the gun's own per-tick barrel angle
//! (`minigun::Barrel`: toward 20 rad/s while not idle, to 0 after, 0.1
//! rad/s a tick either way) turns the `c_minigun`'s own `barrel` bone
//! about its axis (`ViewAnimator::bone_rolls`), and in others' hands the
//! held model's `barrel` part (`spin_held_barrels`, `map::HeldTurn`). The
//! Minigun rests in `m_idle`, upright, barrels up, right of centre, and
//! draws with `m_draw`, no offset (specs/tf2/viewmodel.md, "Why ours shows
//! a huge, wrongly angled Minigun", 7; a capture of the real game agrees);
//! the spun sequences aim it: while it spins (`ViewPose` 1, set by
//! `weapons::minigun`) it idles in `m_spool_idle`.
//!
//! A gun's `[models] view_offset` (Source's forward, left, up) moves its
//! view model in the eye's space (`source_offset`).
//!
//! The arms' animation sound events (5004) are the owner's reload, draw
//! and bolt sounds (`event_effect`, specs/tf2/viewmodel_sounds.md), heard
//! by the owner alone as the animation passes them; everyone else hears
//! the gun's world reload sound (`[sounds] world_reload`) from the server.

use bevy::prelude::*;
use chimera_sim::weapon::{
    Inventory, Weapon, WeaponEventKind,
    view_anim::{self, Pick, ViewAnimAppExt, ViewAnimRules, ViewFrame, ViewState, ViewVariant},
};

use source_engine::view_motion::{MotionAppExt, MotionRules, Sway};

use crate::map::{ViewModelEventKind, anim::AnimEvent};

use super::weapons::{
    BAT, BONESAW, BOTTLE, FIREAXE, FISTS, FLAMETHROWER, GRENADE_LAUNCHER, KNIFE, KUKRI, MINIGUN, PISTOL_SCOUT, REVOLVER,
    ROCKET_LAUNCHER, SCATTERGUN, SHOTGUN, SHOVEL, SMG, SNIPER_RIFLE, STICKY_LAUNCHER, SYRINGE_GUN, WRENCH, melee, minigun, sticky,
};
use crate::map::GameFov;

/// TF2's view-model field of view (the spec gives none for TF2's
/// `viewmodel_fov`: 54, as the brief's default and Source's).
pub const VIEWMODEL_FOV: f32 = 54.0;
/// The Heavy's arms' skin with the Fists (RED's: `v_skin_red` 2; BLU's
/// 3 isn't told apart, as the other arms' team skins aren't).
pub const FISTS_SKIN: i32 = 2;
/// TF2's view-model motion (specs/tf2/viewmodel.md 3-4): CS:S's primary
/// bob (0.4 forward and 0.2 right per unit of bob, the speed's change
/// limited to 320 units/s a second) on a fixed 0.8 s cycle rising half of
/// it, and no sway (`cl_wpn_sway_interp` 0).
pub const MOTION: MotionRules = MotionRules {
    cycle: Some((0.8, 0.5)),
    forward: 0.4,
    right: 0.2,
    slew: true,
    sway: Sway::None,
};

/// The draw's playback rate: a stock gun's 0.67 s draw time over its
/// 0.5 s deploy (specs/tf2/viewmodel.md 6); the draw then holds its last
/// frame until its length at its own rate has passed.
pub const DRAW_RATE: f32 = 0.67 / 0.5;

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
    // The Minigun rests upright in `m_idle` (as the real game shows it:
    // small, right of centre, barrels up, only the right hand in view),
    // and aims while it spins (`m_spool_up`, `m_spool_idle`, `m_fire`,
    // back by `m_spool_down`).
    (
        MINIGUN,
        Sequences {
            spin_up: Some(("ACT_PRIMARY_ATTACK_STAND_PREFIRE", "m_spool_up")),
            spin_idle: Some(("ACT_PRIMARY_VM_SECONDARYATTACK", "m_spool_idle")),
            spin_down: Some(("ACT_PRIMARY_ATTACK_STAND_POSTFIRE", "m_spool_down")),
            fire_loops: true,
            ..plain((P_DRAW, "m_draw"), (P_IDLE, "m_idle"), (P_FIRE, "m_fire"))
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

/// The guns' sequences in another class's hands than their own
/// (weapons.md 8.2: the Soldier's, Pyro's and Heavy's secondary Shotgun;
/// weapons_more.md 9.2: the Engineer's Pistol): (gun, class, sequences).
pub const CLASS_SEQUENCES: &[(&str, &str, Sequences)] = &[
    (SHOTGUN, "soldier", SECONDARY_SHOTGUN),
    (SHOTGUN, "pyro", SECONDARY_SHOTGUN),
    (SHOTGUN, "heavy", SECONDARY_SHOTGUN),
    (PISTOL_SCOUT, "engineer", clip((S_DRAW, "pstl_draw"), (S_IDLE, "pstl_idle"), (S_FIRE, "pstl_fire"), (S_RELOAD, "pstl_reload"))),
];

/// The secondary Shotgun's sequences (the same names on the Soldier's,
/// Pyro's and Heavy's arms, `SECONDARY` activities).
const SECONDARY_SHOTGUN: Sequences = shells(
    (S_DRAW, "draw"),
    (S_IDLE, "idle"),
    (S_FIRE, "fire"),
    [("ACT_SECONDARY_RELOAD_START", "reload_start"), (S_RELOAD, "reload_loop"), ("ACT_SECONDARY_RELOAD_FINISH", "reload_end")],
);

/// One melee weapon's first-person sequences on its class's arms
/// (specs/tf2/weapons_stock.md 8.1), by name: a swing is a weighted pick
/// of `swing` (a name listed twice weighs 2), attack2's (the Fists' right
/// fist) of `swing_alt`, a crit swing `crit`; the Knife's raised family
/// in `backstab` (up, idle, down, the stab).
#[derive(Clone, Copy, Debug)]
pub struct MeleeSequences {
    pub draw: &'static str,
    pub idle: &'static str,
    pub swing: &'static [&'static str],
    pub swing_alt: &'static [&'static str],
    pub crit: &'static str,
    pub backstab: Option<[&'static str; 4]>,
}

const fn swings(draw: &'static str, idle: &'static str, swing: &'static [&'static str], crit: &'static str) -> MeleeSequences {
    MeleeSequences { draw, idle, swing, swing_alt: &[], crit, backstab: None }
}

/// The nine melee weapons' sequences (8.1; the weights' "w2" listed twice).
pub const MELEE_SEQUENCES: &[(&str, MeleeSequences)] = &[
    (BAT, swings("b_draw", "b_idle", &["b_swing_a", "b_swing_a", "b_swing_b"], "b_swing_c")),
    (SHOVEL, swings("s_draw", "s_idle", &["s_swing_a", "s_swing_b"], "s_swing_c")),
    (FIREAXE, swings("fa_draw", "fa_idle", &["fa_swing_a", "fa_swing_a", "fa_swing_b"], "fa_swing_c")),
    (BOTTLE, swings("b_draw", "b_idle", &["b_swing_a", "b_swing_b"], "b_swing_c")),
    (FISTS, MeleeSequences { swing_alt: &["f_swing_right"], ..swings("f_draw", "f_idle", &["f_swing_left"], "f_swing_crit") }),
    (WRENCH, swings("pdq_draw", "pdq_idle_tap", &["pdq_swing_a", "pdq_swing_b"], "pdq_swing_c")),
    (BONESAW, swings("bs_draw", "bs_idle", &["bs_swing_a", "bs_swing_a", "bs_swing_b"], "bs_swing_c")),
    (KUKRI, swings("m_draw", "m_idle", &["m_swing_a", "m_swing_a", "m_swing_b"], "m_swing_c")),
    (
        KNIFE,
        MeleeSequences {
            backstab: Some(["knife_backstab_up", "knife_backstab_idle", "knife_backstab_down", "knife_backstab"]),
            ..swings("knife_draw", "knife_idle", &["knife_stab_a", "knife_stab_a", "knife_stab_b", "knife_stab_c"], "knife_backstab")
        },
    ),
];

/// A melee weapon's sequences.
pub fn melee_sequences(weapon: &str) -> Option<&'static MeleeSequences> {
    MELEE_SEQUENCES.iter().find(|(w, _)| *w == weapon).map(|(_, s)| s)
}

fn names(list: &[&str]) -> Pick {
    Pick::Names(list.iter().map(|s| s.to_string()).collect())
}

/// The sequences of view model `key`: a gun's id (on its own class's
/// arms) or `<gun>#<class>` (`for_class`).
pub fn sequences(key: &str) -> Option<&'static Sequences> {
    match key.split_once('#') {
        Some((gun, class)) => for_class(gun, Some(class)),
        None => for_class(key, None),
    }
}

/// `weapon`'s sequences in `class`'s hands (its own class's without one,
/// or for a class with none of its own here).
pub fn for_class(weapon: &str, class: Option<&str>) -> Option<&'static Sequences> {
    class
        .and_then(|c| CLASS_SEQUENCES.iter().find(|(w, k, _)| *w == weapon && *k == c).map(|(.., s)| s))
        .or_else(|| SEQUENCES.iter().find(|(w, _)| *w == weapon).map(|(_, s)| s))
}

/// The sequences the driver's state picks (`ViewState::variant`: the
/// holder's class).
fn of_state(s: &ViewState) -> Option<&'static Sequences> {
    for_class(s.weapon, s.variant)
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
    if let Some(m) = melee_sequences(s.weapon) {
        return names(&[m.draw]);
    }
    named(of_state(s).map(|q| q.draw))
}

fn idle(s: &ViewState) -> Pick {
    // The raised knife idles raised (`ViewPose` 1, `weapons::melee`).
    if let Some(m) = melee_sequences(s.weapon) {
        return names(&[match m.backstab {
            Some([_, raised, ..]) if s.pose == 1 => raised,
            _ => m.idle,
        }]);
    }
    // A spun Minigun (`ViewPose` 1) idles in its spun sequence.
    named(of_state(s).map(|q| if s.pose == 1 { q.spin_idle.unwrap_or(q.idle) } else { q.idle }))
}

/// A melee weapon's reaction: its swing's start (`weapons::melee`'s
/// actions) and the knife raised and lowered.
fn melee_event(f: &mut ViewFrame, m: &MeleeSequences, kind: &WeaponEventKind) {
    let raised = f.state.pose == 1;
    let alt = matches!(kind, WeaponEventKind::Action(melee::SWING_ALT));
    let pick = match kind {
        WeaponEventKind::Action(melee::SWING_CRIT) => names(&[m.crit]),
        WeaponEventKind::Action(melee::SWING) | WeaponEventKind::Action(melee::SWING_ALT) => match m.backstab {
            Some([.., stab]) if raised => names(&[stab]),
            _ if alt && !m.swing_alt.is_empty() => names(m.swing_alt),
            _ => names(m.swing),
        },
        WeaponEventKind::Action(melee::BACKSTAB_UP) => match m.backstab {
            Some([up, ..]) => names(&[up]),
            None => return,
        },
        WeaponEventKind::Action(melee::BACKSTAB_DOWN) => match m.backstab {
            Some([_, _, down, _]) => names(&[down]),
            None => return,
        },
        _ => return,
    };
    f.attack(&pick);
}

fn event(f: &mut ViewFrame, kind: &WeaponEventKind) {
    if let Some(m) = melee_sequences(f.state.weapon) {
        return melee_event(f, m, kind);
    }
    let Some(q) = of_state(&f.state) else { return };
    match kind {
        WeaponEventKind::Fired { .. } if !f.fired => {
            // A looping fire sequence goes on while it fires.
            if q.fire_loops && f.view.sequence() == Some(q.fire.1) {
                f.sustain();
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

/// What an animation event of TF2's view models shows: its sound events
/// (TF2's own entries, `maps::own_sound`) for the owner alone, every
/// reload, draw and bolt sound the shooter hears (specs/tf2/
/// viewmodel_sounds.md 1, 4), and Source's flashes and brass.
pub fn event_effect(e: &AnimEvent) -> Option<ViewModelEventKind> {
    source_engine::view_anim::sound_event(e, crate::game::maps::SOUND_PREFIX).or_else(|| source_engine::view_anim::event_effect(e))
}

/// TF2's view-model rules.
pub fn rules() -> ViewAnimRules {
    ViewAnimRules {
        draw,
        idle,
        event,
        effect: event_effect,
        // A stock gun's 0.67 s draw over its 0.5 s deploy (specs/tf2/viewmodel.md 6).
        draw_rate: DRAW_RATE,
        others_reload: None,
    }
}

/// TF2's view-model rules on the driver, and its players' view-model
/// variants (`view_variants`).
pub fn add_rules(app: &mut App) {
    if !app.is_plugin_added::<view_anim::ViewAnimPlugin>() {
        app.add_plugins(view_anim::ViewAnimPlugin);
    }
    app.view_anim_rules(super::GAME, rules()).view_motion_rules(super::GAME, MOTION);
    app.init_resource::<super::classes::Classes>()
        .add_systems(Update, view_variants.before(crate::map::DriveAnimation))
        .add_systems(Update, (spin_barrels.after(crate::map::DriveAnimation), spin_held_barrels));
}

/// Each Minigun's barrel in its holder's view model (minigun.md 5): the
/// gun's own barrel angle (`minigun::Barrel`, stepped once a tick on every
/// machine), drawn between the last two ticks, turns the model's own
/// `barrel` bone about its axis (`ViewAnimator::bone_rolls`).
pub fn spin_barrels(
    mut holders: Query<(&Inventory, &mut crate::map::ViewAnimator)>,
    guns: Query<&minigun::Tf2Minigun>,
    models: Option<Res<crate::map::ViewModels>>,
    fixed: Res<Time<Fixed>>,
) {
    let frac = fixed.overstep_fraction();
    for (inv, mut view) in &mut holders {
        let Some(gun) = inv.active.and_then(|w| guns.get(w).ok()) else {
            if !view.bone_rolls.is_empty() {
                view.bone_rolls.clear();
            }
            continue;
        };
        let barrel = view
            .key
            .as_deref()
            .and_then(|k| models.as_ref()?.get(k))
            .and_then(|m| m.bones.iter().position(|b| b.name.eq_ignore_ascii_case("barrel")));
        let rolls = barrel.map(|b| vec![(b, gun.barrel.drawn(frac))]).unwrap_or_default();
        if view.bone_rolls != rolls {
            view.bone_rolls = rolls;
        }
    }
}

/// Each Minigun's barrel in its holder's hands as others see it
/// (minigun.md 5): the held model's `barrel` part (`map::HeldTurn`)
/// turned about its own z by the gun's barrel angle, drawn between ticks.
pub fn spin_held_barrels(
    mut parts: Query<(&crate::map::HeldTurn, &mut Transform)>,
    holders: Query<&Inventory>,
    guns: Query<&minigun::Tf2Minigun>,
    fixed: Res<Time<Fixed>>,
) {
    let frac = fixed.overstep_fraction();
    for (turn, mut t) in &mut parts {
        if !turn.bone.eq_ignore_ascii_case("barrel") {
            continue;
        }
        let angle = holders.get(turn.owner).ok().and_then(|i| i.active).and_then(|w| guns.get(w).ok()).map_or(0.0, |g| g.barrel.drawn(frac));
        let rotation = turn.rest.rotation * Quat::from_rotation_z(angle);
        if t.rotation != rotation {
            t.rotation = rotation;
        }
    }
}

/// Each player holding a TF2 gun shows it on its class's arms
/// (`ViewVariant`: the class `Classes::held` makes it, its rung's drawn
/// one when the gun can be it), on every machine.
pub fn view_variants(
    players: Query<(Entity, &Inventory, Option<&super::classes::RungClass>, Option<&ViewVariant>)>,
    weapons: Query<&Weapon>,
    table: Res<super::classes::Classes>,
    mut commands: Commands,
) {
    for (e, inv, drawn, have) in &players {
        let held = inv.active.and_then(|w| weapons.get(w).ok());
        let Some(class) = held.and_then(|w| table.held(&w.id, drawn.map(|d| d.0.as_str()))) else { continue };
        if have.is_none_or(|v| v.0 != class.name) {
            commands.entity(e).insert(ViewVariant(class.name.clone()));
        }
    }
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
/// the arms of its own class with its `c_` model bone-merged, keyed by the
/// gun's id, and with `every_class` one on each other class's arms that
/// can hold it whose arms are here (`Mount::held`: the install, or the
/// class's set fetched), keyed `<gun>#<class>`; never mirrored.
pub fn load(
    materials: &mut source_engine::content::material::MaterialLoader,
    wants: &dyn Fn(&str) -> bool,
    every_class: bool,
    into: &mut Vec<crate::map::MapViewModel>,
    warnings: &mut Vec<String>,
) {
    use source_engine::content::props;
    let set = super::weapons::tf2_weapons();
    let table = super::classes::classes();
    for (gun, own) in super::weapons::arms_by_gun() {
        if !wants(gun) {
            continue;
        }
        let Some(data) = set.weapons.iter().find(|w| w.id == gun) else { continue };
        // The Fists: the arms alone, in their fists' skin (weapons_stock.md
        // 5.3).
        if gun == FISTS {
            match props::load_view_model_skinned(materials, &super::sets::arms_model(&own), gun, true, FISTS_SKIN) {
                Ok(mut v) => {
                    v.allow_flipping = false;
                    if let Err(e) = props::add_view_hitboxes(materials, &super::sets::arms_model(&own), &mut v) {
                        warnings.push(e);
                    }
                    into.push(v);
                }
                Err(e) => warnings.push(e),
            }
            continue;
        }
        let Some(path) = data.models.view.clone() else { continue };
        let offset = data.models.view_offset;
        // Its own class's arms (keyed by the gun), then the others'.
        let mut hands: Vec<(String, String)> = vec![(gun.to_string(), own.clone())];
        if every_class {
            for c in table.of_gun_all(gun).into_iter().filter(|c| c.arms != own) {
                let arms = super::sets::arms_model(&c.arms);
                if materials.mount().held(&arms) {
                    hands.push((view_anim::variant_key(gun, &c.name), c.arms.clone()));
                } else {
                    debug!("TF2 view model {gun} on the {} arms: not here (its class set wasn't offered)", c.name);
                }
            }
        }
        for (key, arms) in hands {
        let idle = sequences(&key).map(|q| q.idle.1);
        let variant = key.contains('#');
        let merged = props::load_view_model(materials, &super::sets::arms_model(&arms), gun, true).and_then(|arms| {
            let mut weapon = props::load_view_model(materials, &path, gun, true)?;
            if variant && let Some(idle) = idle {
                unmerge_undriven(&arms, &mut weapon, idle);
            }
            // The Minigun's barrel keeps a joint of its own to spin (`spin_barrels`).
            crate::map::merge_keeping(arms, &weapon, own_idle(&weapon).as_deref(), &["barrel"])
        });
        match merged {
            Ok(mut v) => {
                v.key = key;
                // The arms' hitboxes (the crit glow's host in first
                // person: effects.md 14.1), after the merged attachments.
                if let Err(e) = props::add_view_hitboxes(materials, &super::sets::arms_model(&arms), &mut v) {
                    warnings.push(e);
                }
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
}

/// The weapon's bones (but `weapon_bone`) whose same-named arms bone
/// `idle` poses away from the hand (more than `NOT_IN_HAND` units from
/// the arms' `weapon_bone`), renamed so the merge leaves them rigid with
/// their matched ancestor: another class's sequences for the gun don't
/// drive the bones only its own class's do (the Engineer's Frontier
/// Justice set moves the Shotgun's `vm_weapon_bone` with the gun; the
/// Soldier's leaves it metres off).
fn unmerge_undriven(arms: &crate::map::MapViewModel, weapon: &mut crate::map::MapViewModel, idle: &str) {
    let Some(set) = arms.animations.clone() else { return };
    let Some(s) = set.sequence(idle) else { return };
    let mut a = crate::map::anim::Animator::new(set);
    a.play(s, 0.0);
    let pose = a.pose(0.0);
    let mut world: Vec<Mat4> = Vec::with_capacity(arms.bones.len());
    for (i, b) in arms.bones.iter().enumerate() {
        let (r, t) = pose.get(i).copied().unwrap_or((b.rotation, b.position));
        let local = Mat4::from_rotation_translation(r, t);
        let m = b.parent.and_then(|p| world.get(p)).map_or(local, |p| *p * local);
        world.push(m);
    }
    let at = |name: &str| arms.bones.iter().position(|b| b.name.eq_ignore_ascii_case(name)).map(|j| world[j].w_axis.truncate());
    let Some(hand) = at("weapon_bone") else { return };
    for b in &mut weapon.bones {
        if b.name.eq_ignore_ascii_case("weapon_bone") {
            continue;
        }
        if at(&b.name).is_some_and(|p| p.distance(hand) > NOT_IN_HAND) {
            b.name = format!("{}#own", b.name);
        }
    }
}

/// How far from the arms' `weapon_bone` (units) a bone of theirs may be
/// posed and still hold a part of the gun (`unmerge_undriven`): about a
/// long gun's length.
const NOT_IN_HAND: f32 = 48.0;

/// A `[models] view_offset` (Source's forward, left, up, units) in the
/// eye's space (metres: right, up, back).
pub fn source_offset(o: [f32; 3]) -> Vec3 {
    Vec3::new(-o[1], o[2], -o[0]) * super::METERS_PER_UNIT
}
