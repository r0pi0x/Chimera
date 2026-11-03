//! Source's player animation, the half every Source game shares (lift S9
//! of docs/plans/active/hl2dm.md, from CS:S's `player_anim`): what a
//! body's animation state sees each update (`Inputs`: the look as drawn,
//! the velocity, ground, water and duck, and what its weapon did, read
//! from the weapons' events), the drawn bodies' update at the frame rate
//! (`PlayerAnimPlugin`) and a network server's own pose in the fixed tick
//! (`SimAnimPlugin`, into `map::SimAnimator`, which poses the hitboxes the
//! next tick traces and lag compensation keeps), each on the bodies of the
//! game whose state it is (`CharacterModels::is_of`: in a mashup another
//! game's bodies play their own), and the hands locked onto the weapon by
//! the models' IK rules (`lock_hands`).
//!
//! The choice of sequence, the pose parameters and the layers are the
//! game's (`PlayerAnimRules`: CS:S's by weapon suffix, specs/cs_source/
//! animation.md 12; HL2:DM's activities by weapon kind, specs/hl2dm/
//! player_models.md 3-8).

use std::{marker::PhantomData, sync::Arc};

use bevy::{ecs::query::QueryFilter, prelude::*};

use crate::{
    core::{Health, Intent, MovementState, Velocity},
    map::{
        DriveAnimation,
        anim::{Animator, IkGoal},
    },
    weapon::{Inventory, Magazine, ShellReload, Weapon, WeaponEvent, WeaponEventKind},
};

const UNIT: f32 = 0.0254;

/// Angle in degrees into (-180, 180].
pub fn normalize(a: f32) -> f32 {
    let a = a % 360.0;
    if a > 180.0 {
        a - 360.0
    } else if a <= -180.0 {
        a + 360.0
    } else {
        a
    }
}

/// What the body sees this update, in Source terms: degrees, units/s, the
/// game's world axes (yaw 0 = +X, counter-clockwise).
#[derive(Clone, Copy, Debug, Default)]
pub struct Inputs {
    pub eye_yaw: f32,
    /// Positive looking down.
    pub eye_pitch: f32,
    pub velocity: Vec2,
    /// Vertical speed (units/s, up).
    pub vertical: f32,
    pub ducked: bool,
    pub on_ground: bool,
    /// In water above the waist.
    pub swimming: bool,
    /// Held still by the rules (freeze time, intermission).
    pub frozen: bool,
    pub jumped: bool,
    pub fired: bool,
    /// A reload started this update.
    pub reloaded: bool,
    /// Which part of a reload: "" a whole one, else "_start", "_loop" (a
    /// shell going in) or "_end" (shotguns).
    pub reload_part: &'static str,
    /// The hand that fired: "" or, for dual pistols, "_L" or "_R".
    pub shot_side: &'static str,
    /// A grenade's pin is out (or its throw is pending).
    pub primed: bool,
    /// A grenade was released this update.
    pub threw: bool,
    /// Arming stopped short this update (CS:S's bomb): its gesture stops
    /// at once instead of playing out.
    pub fire_stopped: bool,
}

/// A game's player animation state: one per body of the game, driven by
/// `PlayerAnimPlugin` (drawn) and `SimAnimPlugin` (a server's tick).
pub trait PlayerAnimRules: Component<Mutability = bevy::ecs::component::Mutable> + Default + Clone + Send + Sync + 'static {
    /// The game whose bodies it drives (`MapCharacterModel::game`).
    const GAME: &'static str;

    /// One update of `a` from `i`, the weapon in hand `weapon` (its id),
    /// `dt` seconds after the last, at `now`.
    fn update(&mut self, a: &mut Animator, i: &Inputs, weapon: Option<&str>, dt: f32, now: f64);

    /// `update` on a network server's per-tick pose (`SimAnimPlugin`);
    /// the same unless the game's server poses differently.
    fn update_tick(&mut self, a: &mut Animator, i: &Inputs, weapon: Option<&str>, dt: f32, now: f64) {
        self.update(a, i, weapon, dt, now);
    }

    /// The body's render and hitbox yaw (degrees, the game's axes).
    fn feet_yaw(&self) -> f32;

    /// The body died: the next update after it lives again starts afresh.
    fn dead(&mut self);

    /// The hand a shot of `w` came from ("" but for dual pistols).
    fn shot_side(_w: &Weapon, _m: Option<&Magazine>) -> &'static str {
        ""
    }

    /// Whether the body's animator applies its autoplay sequences
    /// (`Animator::autoplay`).
    const AUTOPLAY: bool = false;
}

/// What weapons did this update, per owner: shots (with the hand that
/// fired), reload parts, throws.
#[derive(Default)]
pub struct Happened {
    pub fired: Vec<(Entity, &'static str)>,
    pub reloaded: Vec<(Entity, &'static str)>,
    pub threw: Vec<Entity>,
    /// Arming stopped short (after its start, if both came this update).
    pub arming_stopped: Vec<Entity>,
}

pub type WeaponsQuery<'w, 's> = Query<'w, 's, (&'static Weapon, Option<&'static crate::weapon::grenade::Throwable>)>;
pub type LoadingQuery<'w, 's> = Query<'w, 's, (Option<&'static ShellReload>, Option<&'static Magazine>)>;

impl Happened {
    pub fn of(all: &[&WeaponEvent], weapons: &WeaponsQuery, loading: &LoadingQuery, side: fn(&Weapon, Option<&Magazine>) -> &'static str) -> Self {
        let mut h = Self::default();
        for ev in all {
            let (shells, magazine) = loading.get(ev.weapon).unwrap_or((None, None));
            match ev.kind {
                WeaponEventKind::Shot { .. } | WeaponEventKind::Swing { .. } | WeaponEventKind::ArmingStarted => {
                    let s = weapons.get(ev.weapon).map_or("", |(w, _)| side(w, magazine));
                    h.fired.push((ev.owner, s));
                }
                WeaponEventKind::ReloadStarted => {
                    h.reloaded.push((ev.owner, if shells.is_some() { "_start" } else { "" }));
                }
                WeaponEventKind::ShellInserting => h.reloaded.push((ev.owner, "_loop")),
                WeaponEventKind::Reloaded if shells.is_some() => h.reloaded.push((ev.owner, "_end")),
                WeaponEventKind::Thrown => h.threw.push(ev.owner),
                WeaponEventKind::ArmingStopped => {
                    h.fired.retain(|(o, _)| *o != ev.owner);
                    h.arming_stopped.push(ev.owner);
                }
                _ => {}
            }
        }
        h
    }

    pub fn find(list: &[(Entity, &'static str)], e: Entity) -> Option<&'static str> {
        list.iter().rev().find(|(o, _)| *o == e).map(|(_, p)| *p)
    }
}

/// The character's state as `Inputs` (our axes to the game's: forward at
/// yaw 0 is -Z there +X, left -X there +Y), `jumped` from `was_on_ground`.
#[allow(clippy::too_many_arguments)]
pub fn inputs(
    e: Entity,
    was_on_ground: &mut bool,
    look: crate::map::interp::EyeView,
    velocity: &Velocity,
    movement: &MovementState,
    weapon: Option<(&Weapon, Option<&crate::weapon::grenade::Throwable>)>,
    happened: &Happened,
    swimming: bool,
) -> Inputs {
    let jumped = *was_on_ground && !movement.on_ground && velocity.y > 0.0;
    *was_on_ground = movement.on_ground;
    Inputs {
        eye_yaw: look.yaw.to_degrees(),
        eye_pitch: -look.pitch.to_degrees(),
        velocity: Vec2::new(-velocity.z, -velocity.x) / UNIT,
        vertical: velocity.y / UNIT,
        ducked: movement.crouching,
        on_ground: movement.on_ground,
        swimming,
        frozen: false,
        jumped,
        fired: Happened::find(&happened.fired, e).is_some(),
        reloaded: Happened::find(&happened.reloaded, e).is_some(),
        reload_part: Happened::find(&happened.reloaded, e).unwrap_or(""),
        shot_side: Happened::find(&happened.fired, e).unwrap_or(""),
        primed: weapon.and_then(|(_, t)| t).is_some_and(|t| t.primed()),
        threw: happened.threw.contains(&e),
        fire_stopped: happened.arming_stopped.contains(&e),
    }
}

/// Whether the ground was under the body at its last update (`jumped`).
#[derive(Component, Debug, Clone, Copy)]
pub struct WasOnGround(pub bool);

impl Default for WasOnGround {
    fn default() -> Self {
        Self(true)
    }
}

/// Whether `body` is game `R`'s (or of no game in particular).
fn ours<R: PlayerAnimRules>(models: Option<&crate::map::CharacterModels>, body: Option<&crate::map::BodyModel>) -> bool {
    models.is_none_or(|m| m.is_of(body, R::GAME))
}

/// The drawn bodies of game `R` animated at the frame rate (in
/// `DriveAnimation`), from the look as drawn (eased between ticks,
/// `map::interp`); `F` filters out characters the game animates otherwise
/// (CS:S's hostages).
pub struct PlayerAnimPlugin<R, F = ()>(PhantomData<fn() -> (R, F)>);

impl<R, F> Default for PlayerAnimPlugin<R, F> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<R: PlayerAnimRules, F: QueryFilter + 'static> Plugin for PlayerAnimPlugin<R, F> {
    fn build(&self, app: &mut App) {
        app.add_message::<WeaponEvent>()
            .add_systems(Update, drive::<R, F>.in_set(DriveAnimation).in_set(PlayerAnimSystems));
    }
}

/// The drawn bodies' updates (`PlayerAnimPlugin`), for a game's systems
/// that adjust what they decided.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlayerAnimSystems;

#[allow(clippy::type_complexity)]
fn drive<R: PlayerAnimRules, F: QueryFilter + 'static>(
    time: Res<Time>,
    mut characters: Query<
        (
            Entity,
            &mut Animator,
            Option<(&mut R, &mut WasOnGround)>,
            &Intent,
            &Velocity,
            &MovementState,
            Option<&Health>,
            Option<&Inventory>,
            (
                Option<&crate::map::interp::RenderedView>,
                Option<&crate::map::BodyModel>,
                Option<&crate::source_movement::SourceMovement>,
            ),
        ),
        F,
    >,
    weapons: WeaponsQuery,
    loading: LoadingQuery,
    mut events: MessageReader<WeaponEvent>,
    models: Option<Res<crate::map::CharacterModels>>,
    mut commands: Commands,
) {
    let all: Vec<&WeaponEvent> = events.read().filter(|e| e.shown()).collect();
    let happened = Happened::of(&all, &weapons, &loading, R::shot_side);
    let (dt, now) = (time.delta_secs(), time.elapsed_secs_f64());
    for (e, mut animator, state, intent, velocity, movement, health, inventory, (view, body, source)) in &mut characters {
        // Only the game's bodies (another game's, in a mashup, play its own).
        if !ours::<R>(models.as_deref(), body) {
            if state.is_some() {
                commands.entity(e).remove::<(R, WasOnGround)>();
            }
            continue;
        }
        let Some((mut state, mut ground)) = state else {
            commands.entity(e).insert((R::default(), WasOnGround::default()));
            continue;
        };
        if health.is_some_and(|h| h.current <= 0.0) {
            state.dead();
            continue;
        }
        let look = crate::map::interp::eye_view(view, intent, movement);
        let weapon = inventory.and_then(|i| i.active).and_then(|w| weapons.get(w).ok());
        let swimming = source.is_some_and(|s| s.water_level >= 2);
        let i = inputs(e, &mut ground.0, look, velocity, movement, weapon, &happened, swimming);
        animator.autoplay = R::AUTOPLAY;
        state.update(&mut animator, &i, weapon.map(|(w, _)| w.id), dt, now);
        animator.yaw = Some(state.feet_yaw().to_radians());
    }
}

/// A network server's own animation state of a body (`map::SimAnimator`).
#[derive(Component, Debug, Clone, Default)]
pub struct SimAnimState<R>(pub R, pub WasOnGround);

/// A network server animates every body of game `R` again in the fixed
/// tick, from the tick's own values (the look as simulated, the tick's
/// shots), into its `map::SimAnimator`, which poses the hitboxes the next
/// tick traces and lag compensation keeps (`map::SimPose`). What is drawn
/// keeps its own animation (`PlayerAnimPlugin`).
pub struct SimAnimPlugin<R, F = ()>(PhantomData<fn() -> (R, F)>);

impl<R, F> Default for SimAnimPlugin<R, F> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<R: PlayerAnimRules, F: QueryFilter + 'static> Plugin for SimAnimPlugin<R, F> {
    fn build(&self, app: &mut App) {
        app.add_message::<WeaponEvent>().add_systems(
            FixedLast,
            drive_sim::<R, F>
                .before(crate::map::SimPose)
                .run_if(resource_equals(crate::core::NetRole::Server)),
        );
    }
}

#[allow(clippy::type_complexity)]
fn drive_sim<R: PlayerAnimRules, F: QueryFilter + 'static>(
    clock: Res<crate::core::SimClock>,
    mut characters: Query<
        (
            Entity,
            &Animator,
            Option<&mut crate::map::SimAnimator>,
            Option<&mut SimAnimState<R>>,
            &Intent,
            &Velocity,
            &MovementState,
            Option<&Health>,
            (Option<&Inventory>, Option<&crate::map::BodyModel>, Option<&crate::source_movement::SourceMovement>),
        ),
        F,
    >,
    weapons: WeaponsQuery,
    loading: LoadingQuery,
    mut events: MessageReader<WeaponEvent>,
    models: Option<Res<crate::map::CharacterModels>>,
    mut commands: Commands,
) {
    let all: Vec<&WeaponEvent> = events.read().filter(|e| e.shown()).collect();
    let happened = Happened::of(&all, &weapons, &loading, R::shot_side);
    let (dt, now) = (clock.dt(), clock.now);
    for (e, drawn, sim, state, intent, velocity, movement, health, (inventory, body, source)) in &mut characters {
        if !ours::<R>(models.as_deref(), body) {
            if sim.is_some() || state.is_some() {
                commands.entity(e).remove::<(crate::map::SimAnimator, SimAnimState<R>)>();
            }
            continue;
        }
        let (Some(mut sim), Some(mut state)) = (sim, state) else {
            commands.entity(e).insert((
                crate::map::SimAnimator(Animator::new(drawn.set.clone())),
                SimAnimState::<R>(R::default(), WasOnGround::default()),
            ));
            continue;
        };
        if !Arc::ptr_eq(&sim.0.set, &drawn.set) {
            // Another model (a team change).
            sim.0 = Animator::new(drawn.set.clone());
            state.0 = R::default();
        }
        if health.is_some_and(|h| h.current <= 0.0) {
            state.0.dead();
            continue;
        }
        let look = crate::map::interp::eye_view(None, intent, movement);
        let weapon = inventory.and_then(|i| i.active).and_then(|w| weapons.get(w).ok());
        let SimAnimState(rules, ground) = &mut *state;
        let swimming = source.is_some_and(|s| s.water_level >= 2);
        let i = inputs(e, &mut ground.0, look, velocity, movement, weapon, &happened, swimming);
        sim.0.autoplay = R::AUTOPLAY;
        rules.update_tick(&mut sim.0, &i, weapon.map(|(w, _)| w.id), dt, now);
        sim.0.yaw = Some(rules.feet_yaw().to_radians());
    }
}

/// Seconds a hand takes to settle onto its grip or let go (our choice;
/// the files' IK rules carry no fade we use).
pub const HAND_LOCK_FADE: f32 = 0.2;

/// IK chains of the shared skeleton and the grip bones their ends lock
/// to: `weapon_bone_RHand`/`_LHand` sit where the hands are in each
/// weapon's authored pose, so a locked hand stays on the gun while the
/// aim layers turn the arms (playtest 2026-10-10: a pistol held a hand's
/// width off the hand when aiming up or down).
pub const HAND_CHAINS: [(&str, &str); 2] = [
    ("rhand", "valvebiped.weapon_bone_rhand"),
    ("lhand", "valvebiped.weapon_bone_lhand"),
];

/// The hands' IK locks from the files' rules (`AnimSet::ik_locks`): the
/// layers in order, each sequence then its autolayers, the last rule for
/// a chain wins (`rules`: false ignores them, every hand let go). Each
/// hand's lock (`locks`, 0 to 1) fades toward its rule over
/// `HAND_LOCK_FADE` (at once when `snap`).
pub fn lock_hands(a: &mut Animator, locks: &mut [f32; 2], snap: bool, rules: bool, dt: f32) {
    let set = a.set.clone();
    a.ik.clear();
    if set.ik_chains.is_empty() || set.parents.is_empty() {
        return;
    }
    let mut want = vec![None; set.ik_chains.len()];
    if rules {
        for l in a.layers.iter().flatten().filter(|l| l.weight > 0.0) {
            set.ik_locks(l.sequence, &mut want);
        }
    }
    for (k, (chain, grip)) in HAND_CHAINS.iter().enumerate() {
        let found = set.ik_chains.iter().position(|c| c.name == *chain);
        let target = set.bone_names.iter().position(|n| n == grip);
        let (Some(chain), Some(target)) = (found, target) else {
            locks[k] = 0.0;
            continue;
        };
        let goal = if want[chain] == Some(true) { 1.0 } else { 0.0 };
        let step = if snap { 1.0 } else { dt / HAND_LOCK_FADE };
        let w = &mut locks[k];
        *w = if *w < goal { (*w + step).min(goal) } else { (*w - step).max(goal) };
        if *w > 0.0 {
            a.ik.push(IkGoal {
                chain,
                target,
                weight: *w,
            });
        }
    }
}
