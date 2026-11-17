//! Render interpolation: what the simulation moves at its fixed tick is
//! drawn between its last two ticks, by how far the frame is into the
//! next one (`Time<Fixed>::overstep_fraction`), Source style (drawn one
//! tick behind). The simulation never sees it: each tick starts from
//! the values it left (`restore`, in `FixedFirst`), and only rendering
//! (`RunFixedMainLoop` after the fixed loop, then `Update`) sees the
//! blend.
//!
//! - `Interpolated` on an entity eases its `Transform` (translation,
//!   rotation, scale). Characters (`Intent`), brush entities
//!   (`MapBrushEntity`: doors, trains, platforms) and moving bodies
//!   (dynamic or kinematic `RigidBody`: props, gibs, dropped weapons,
//!   ragdolls) get it from `InterpPlugin`; other layers add it as a
//!   required component (the client: grenades).
//! - `RenderedView` on characters eases what the camera and bodies take
//!   from the simulation besides the position: eye offset (ducking),
//!   view roll and punch, and other characters' look angles (the local
//!   player's come straight from `Intent`, per frame).
//! - A jump further than a body could move in a tick (teleports,
//!   respawns, `setpos`) snaps instead of smearing: `SNAP_SPEED`. A
//!   `Transform` written outside the fixed tick (console, captures) is
//!   taken as the new truth and snaps too.
//!
//! - Characters a network client draws from the server's snapshots
//!   (`NetDrawn`: `net::interp` writes their `Transform` and
//!   `RenderedView` each frame) are left alone.
//!
//! - `GameTicks` is for a game whose own tick is slower than the
//!   engine's (Halo's 30 Hz): values laid at the game's ticks, drawn
//!   between them on the game's clock (`GameClock`); the game eases and
//!   puts back its own (`halo_ce::game::eased`).
//!
//! Plugged in by the client (headless runs and tests without it see the
//! simulation's values only); `Interpolation::enabled` is its cvar
//! (`cl_interpolate`).

use avian3d::prelude::RigidBody;
use bevy::{
    app::RunFixedMainLoopSystems,
    prelude::*,
    transform::systems::{mark_dirty_trees, propagate_parent_transforms, sync_simple_transforms},
};

use chimera_core::later::Later;

use super::anim::BonePose;
use crate::core::{Intent, LocalPlayer, MovementState};

/// Faster than anything moves in one tick (m/s; CS:S's sv_maxvelocity
/// is 3500 units/s, 89 m/s): a jump further than this in one tick is a
/// teleport and is drawn at once.
pub const SNAP_SPEED: f32 = 100.0;
/// A look turned further than this in one tick (radians) snaps too.
pub const SNAP_TURN: f32 = 1.2;

/// The interpolation switch (`cl_interpolate`) and what it did this
/// frame (the perf readout).
#[derive(Resource, Debug, Clone, Copy)]
pub struct Interpolation {
    /// 0: draw the latest tick (stepping at the tick rate, for A/B).
    pub enabled: u8,
    /// The blend this frame: 0 draws the previous tick, 1 the latest.
    pub fraction: f32,
    /// Fixed ticks run this frame.
    pub ticks: u32,
    /// Snaps (teleports) since start.
    pub snaps: u64,
}

impl Default for Interpolation {
    fn default() -> Self {
        Self {
            enabled: 1,
            fraction: 1.0,
            ticks: 0,
            snaps: 0,
        }
    }
}

/// Order of the interpolation systems: in `FixedLast`, `Sample` keeps
/// this tick's values, `Extend` is where other layers add theirs to the
/// `RenderedView` sample (the client: the view punch), `Snap` then drops
/// the blend for teleports.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InterpSystems {
    /// In `FixedFirst`: the simulation's transforms back.
    Restore,
    Sample,
    Extend,
    Snap,
    /// In `RunFixedMainLoop` after the fixed loop: the drawn values.
    Ease,
}

/// Drawn by a network client from the server's snapshots at its own
/// render time (`net::interp`), not between this process's ticks: the
/// systems here skip it.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct NetDrawn;

/// Draw this entity's `Transform` between its last two ticks.
#[derive(Component, Debug, Clone, Default)]
pub struct Interpolated {
    prev: Transform,
    cur: Transform,
    sampled: bool,
    /// What the ease last wrote, to tell another writer's change.
    drawn: Option<Transform>,
}

impl Interpolated {
    /// The simulation's transform at the previous and the latest tick.
    pub fn ticks(&self) -> Option<(Transform, Transform)> {
        self.sampled.then_some((self.prev, self.cur))
    }

    /// Move both kept ticks by `by`: a network client corrected its
    /// prediction (`net::predict`) and eases the error out itself, so the
    /// blend must run along the corrected path.
    pub fn shift(&mut self, by: Vec3) {
        self.prev.translation += by;
        self.cur.translation += by;
    }

    fn adopt(&mut self, t: Transform) {
        self.prev = t;
        self.cur = t;
        self.sampled = true;
    }
}

/// What a camera or body takes from a character besides its position.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct EyeView {
    /// Eye above the origin (`MovementState::eye_offset`), meters.
    pub eye_offset: Vec3,
    /// `MovementState::view_roll`, radians.
    pub view_roll: f32,
    /// Look angles (`Intent`), radians.
    pub yaw: f32,
    pub pitch: f32,
    /// Recoil and flinch on the view (`weapon::ViewPunch`: pitch, yaw,
    /// roll), radians; the client fills it in.
    pub punch: Vec3,
}

impl EyeView {
    pub fn of(intent: &Intent, state: &MovementState) -> Self {
        Self {
            eye_offset: state.eye_offset,
            view_roll: state.view_roll,
            yaw: intent.yaw,
            pitch: intent.pitch,
            punch: Vec3::ZERO,
        }
    }

    pub fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        Self {
            eye_offset: a.eye_offset.lerp(b.eye_offset, t),
            view_roll: a.view_roll + (b.view_roll - a.view_roll) * t,
            yaw: a.yaw + wrap(b.yaw - a.yaw) * t,
            pitch: a.pitch + (b.pitch - a.pitch) * t,
            punch: a.punch.lerp(b.punch, t),
        }
    }
}

/// An angle difference into (-pi, pi].
fn wrap(a: f32) -> f32 {
    let t = std::f32::consts::TAU;
    let a = a.rem_euclid(t);
    if a > std::f32::consts::PI { a - t } else { a }
}

/// A character's eye as drawn this frame (`now`), eased between ticks.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct RenderedView {
    pub now: EyeView,
    prev: EyeView,
    cur: EyeView,
    sampled: bool,
}

impl RenderedView {
    /// The latest tick's sample (others add to it in `FixedLast`, in
    /// `InterpSystems::Extend`).
    pub fn latest_mut(&mut self) -> &mut EyeView {
        &mut self.cur
    }
}

/// The eye to draw: the eased one when there is one, else the
/// simulation's.
pub fn eye_view(view: Option<&RenderedView>, intent: &Intent, state: &MovementState) -> EyeView {
    view.map_or_else(|| EyeView::of(intent, state), |v| v.now)
}

/// Ticks counted this frame (reset by the ease).
#[derive(Resource, Default)]
struct TicksThisFrame(u32);

pub struct InterpPlugin;

impl Plugin for InterpPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Interpolation>()
            .init_resource::<TicksThisFrame>()
            .register_required_components::<Intent, Interpolated>()
            .register_required_components::<Intent, RenderedView>()
            .register_required_components::<super::MapBrushEntity, Interpolated>()
            .configure_sets(FixedLast, (InterpSystems::Sample, InterpSystems::Extend, InterpSystems::Snap).chain())
            .configure_sets(
                RunFixedMainLoop,
                InterpSystems::Ease.in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
            )
            .add_systems(
                FixedFirst,
                (
                    restore,
                    // The simulation reads globals too (this tick's, not
                    // the drawn ones).
                    mark_dirty_trees,
                    propagate_parent_transforms,
                    sync_simple_transforms,
                )
                    .chain()
                    .in_set(InterpSystems::Restore),
            )
            .add_systems(FixedLast, (sample_transforms, sample_views).in_set(InterpSystems::Sample))
            .add_systems(FixedLast, snap.in_set(InterpSystems::Snap))
            .add_systems(RunFixedMainLoop, (ease_transforms, ease_views).in_set(InterpSystems::Ease))
            .add_systems(Update, track_bodies);
    }
}

/// Bodies the physics moves (dynamic, kinematic) are drawn eased; a
/// prop whose motion is enabled later gets it then.
fn track_bodies(
    bodies: Query<(Entity, &RigidBody), (Changed<RigidBody>, Without<Interpolated>)>,
    mut commands: Commands,
) {
    for (e, body) in &bodies {
        if !body.is_static() {
            commands.entity(e).insert(Interpolated::default());
        }
    }
}

/// Start of a tick: put back the simulation's transform where the ease
/// drew another, or take a transform someone else wrote as the truth.
fn restore(
    mut q: Query<(&mut Transform, &mut Interpolated), Without<NetDrawn>>,
    mut ticks: ResMut<TicksThisFrame>,
) {
    ticks.0 += 1;
    for (mut t, mut i) in &mut q {
        let Some(drawn) = i.drawn.take() else { continue };
        if *t != drawn {
            let now = *t;
            i.adopt(now);
        } else if *t != i.cur {
            *t = i.cur;
        }
    }
}

fn sample_transforms(mut q: Query<(&Transform, &mut Interpolated), Without<NetDrawn>>) {
    for (t, mut i) in &mut q {
        if i.sampled {
            i.prev = i.cur;
            i.cur = *t;
        } else {
            i.adopt(*t);
        }
    }
}

fn sample_views(mut q: Query<(&Intent, &MovementState, &mut RenderedView), Without<NetDrawn>>) {
    for (intent, state, mut v) in &mut q {
        let now = EyeView::of(intent, state);
        if v.sampled {
            v.prev = v.cur;
            v.cur = now;
        } else {
            v.prev = now;
            v.cur = now;
            v.sampled = true;
        }
    }
}

/// Teleports: a jump no body makes in one tick is drawn at once.
fn snap(
    time: Res<Time<Fixed>>,
    mut q: Query<(&mut Interpolated, Option<&mut RenderedView>), Without<NetDrawn>>,
    mut views: Query<&mut RenderedView, (Without<Interpolated>, Without<NetDrawn>)>,
    mut stats: ResMut<Interpolation>,
) {
    let far = SNAP_SPEED * time.timestep().as_secs_f32();
    for (mut i, view) in &mut q {
        let jumped = i.prev.translation.distance(i.cur.translation) > far;
        let turned = view
            .as_ref()
            .is_some_and(|v| wrap(v.cur.yaw - v.prev.yaw).abs() > SNAP_TURN);
        if jumped {
            i.prev = i.cur;
            stats.snaps += 1;
        }
        if let Some(mut v) = view
            && (jumped || turned)
        {
            v.prev = v.cur;
        }
    }
    for mut v in &mut views {
        if wrap(v.cur.yaw - v.prev.yaw).abs() > SNAP_TURN {
            v.prev = v.cur;
        }
    }
}

/// How far between the last two ticks to draw: 1 (the latest) when off.
fn fraction(time: &Time<Fixed>, settings: &Interpolation) -> f32 {
    if settings.enabled == 0 {
        1.0
    } else {
        time.overstep_fraction().clamp(0.0, 1.0)
    }
}

fn ease_transforms(
    time: Res<Time<Fixed>>,
    mut settings: ResMut<Interpolation>,
    mut ticks: ResMut<TicksThisFrame>,
    mut q: Query<(&mut Transform, &mut Interpolated), Without<NetDrawn>>,
) {
    let f = fraction(&time, &settings);
    settings.fraction = f;
    settings.ticks = std::mem::take(&mut ticks.0);
    for (mut t, mut i) in &mut q {
        if !i.sampled {
            continue;
        }
        // Written since the last tick by someone else (a frame without
        // ticks): that is where it is now.
        if let Some(drawn) = i.drawn
            && *t != drawn
        {
            let now = *t;
            i.adopt(now);
            i.drawn = Some(now);
            continue;
        }
        let want = if i.prev == i.cur {
            i.cur
        } else {
            Transform {
                translation: i.prev.translation.lerp(i.cur.translation, f),
                rotation: i.prev.rotation.slerp(i.cur.rotation, f),
                scale: i.prev.scale.lerp(i.cur.scale, f),
            }
        };
        if *t != want {
            *t = want;
        }
        i.drawn = Some(want);
    }
}

fn ease_views(
    time: Res<Time<Fixed>>,
    settings: Res<Interpolation>,
    mut q: Query<(&Intent, &MovementState, &mut RenderedView, Has<LocalPlayer>), Without<NetDrawn>>,
) {
    let f = fraction(&time, &settings);
    for (intent, state, mut v, local) in &mut q {
        let mut now = if v.sampled {
            EyeView::lerp(&v.prev, &v.cur, f)
        } else {
            EyeView::of(intent, state)
        };
        if local {
            // The local player's look is per frame (mouse), never eased.
            now.yaw = intent.yaw;
            now.pitch = intent.pitch;
        }
        if v.now != now {
            v.now = now;
        }
    }
}

/// What a game sets at a tick of its own slower than the engine's (Halo's
/// 30 Hz under a 64 Hz engine tick: it changes on only some engine ticks),
/// drawn between the two values around the frame's time on that game's
/// clock (`GameTicks::at`), as `Interpolated` draws the engine's ticks. A
/// value is laid (`lay`) at its game tick (the count of the game's ticks
/// since the run's start, `GameClock::laid`); the frame draws at
/// `GameClock::drawn`, behind the latest by just enough that the next one
/// is always in (`GameClock::lag`). The simulation never sees the blend:
/// the game puts the latest back at the start of each engine tick
/// (`GameTicks::latest`).
#[derive(Clone, Debug)]
pub struct GameTicks<T> {
    /// The last few values, oldest first, with their game ticks.
    laid: std::collections::VecDeque<(u64, T)>,
}

impl<T> Default for GameTicks<T> {
    fn default() -> Self {
        Self { laid: Default::default() }
    }
}

/// How many values a `GameTicks` keeps: the drawn time is at most a game
/// tick and an engine tick behind the latest.
const KEPT: usize = 3;

impl<T: Clone> GameTicks<T> {
    /// `value` is the game's at its tick `tick`; a tick at or before one
    /// laid (the run restarted, a prediction redone) replaces those after.
    pub fn lay(&mut self, tick: u64, value: T) {
        while self.laid.back().is_some_and(|(t, _)| *t >= tick) {
            self.laid.pop_back();
        }
        self.laid.push_back((tick, value));
        while self.laid.len() > KEPT {
            self.laid.pop_front();
        }
    }

    /// Forget what was laid: the next value is drawn as it is (a model
    /// changed, a teleport).
    pub fn clear(&mut self) {
        self.laid.clear();
    }

    /// The latest value laid.
    pub fn latest(&self) -> Option<&T> {
        self.laid.back().map(|(_, v)| v)
    }

    /// The two values around game time `time` (in the game's ticks) and
    /// how far between them (0: the first, 1: the second); before the
    /// first laid, the first; after the latest, the latest.
    pub fn around(&self, time: f64) -> Option<(&T, &T, f32)> {
        let (first, last) = (self.laid.front()?, self.laid.back()?);
        if time <= first.0 as f64 {
            return Some((&first.1, &first.1, 1.0));
        }
        if time >= last.0 as f64 {
            return Some((&last.1, &last.1, 1.0));
        }
        let (a, b) = self.laid.iter().zip(self.laid.iter().skip(1)).find(|(_, b)| time < b.0 as f64)?;
        let f = ((time - a.0 as f64) / (b.0 - a.0) as f64).clamp(0.0, 1.0) as f32;
        Some((&a.1, &b.1, f))
    }
}

impl GameTicks<Transform> {
    /// The transform drawn at game time `time`.
    pub fn at(&self, time: f64) -> Option<Transform> {
        let (a, b, f) = self.around(time)?;
        Some(Transform {
            translation: a.translation.lerp(b.translation, f),
            rotation: a.rotation.slerp(b.rotation, f),
            scale: a.scale.lerp(b.scale, f),
        })
    }
}

impl GameTicks<Later<Vec<BonePose>>> {
    /// The pose drawn at game time `time`, each bone's rotation and
    /// position between the two around it (`blend_poses`); made when first
    /// drawn (`Later`).
    pub fn at(&self, time: f64) -> Option<Later<Vec<BonePose>>> {
        let (a, b, f) = self.around(time)?;
        if f >= 1.0 || Later::ptr_eq(a, b) {
            return Some(b.clone());
        }
        if f <= 0.0 {
            return Some(a.clone());
        }
        let (a, b) = (a.clone(), b.clone());
        Some(Later::new(move || blend_poses(&a, &b, f)))
    }
}

/// Pose `a` toward `b` by `f`: each bone's rotation by the shorter arc,
/// its position straight; poses of different skeletons give `b`.
pub fn blend_poses(a: &[BonePose], b: &[BonePose], f: f32) -> Vec<BonePose> {
    if a.len() != b.len() {
        return b.to_vec();
    }
    a.iter().zip(b).map(|((qa, pa), (qb, pb))| (qa.slerp(*qb, f), pa.lerp(*pb, f))).collect()
}

/// A game's tick counted out of the engine's (`GameTicks`): `per_tick`
/// game ticks an engine tick as a fraction (numerator, denominator; Halo's
/// 30 Hz under a 64 Hz engine tick: 30 / 64), the engine's tick k ending
/// at game time k x per_tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameClock {
    pub per_tick: (u128, u128),
}

impl GameClock {
    /// The game ticks crossed by the end of engine tick `tick`: the game
    /// tick of what the game laid then.
    pub fn laid(&self, tick: u64) -> u64 {
        let (n, d) = self.per_tick;
        (u128::from(tick) * n / d.max(1)) as u64
    }

    /// How far behind the end of the latest engine tick, in game ticks, a
    /// frame draws (`drawn`): the least that always has the next value in,
    /// per_tick + 1 - gcd / d (1 at the game's own tick, which is the
    /// engine's one tick behind; 1.44 for 30 Hz under 64 Hz).
    pub fn lag(&self) -> f64 {
        let (n, d) = self.per_tick;
        let d = d.max(1);
        let g = gcd(n, d);
        (n as f64 + (d - g) as f64) / d as f64
    }

    /// The game time a frame draws, `overstep` (0 to 1) of an engine tick
    /// after the end of engine tick `tick`.
    pub fn drawn(&self, tick: u64, overstep: f32) -> f64 {
        let (n, d) = self.per_tick;
        (tick as f64 + f64::from(overstep.clamp(0.0, 1.0))) * n as f64 / d.max(1) as f64 - self.lag()
    }
}

fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eye_view_turns_the_short_way() {
        let a = EyeView {
            yaw: 6.2,
            ..default()
        };
        let b = EyeView {
            yaw: 0.1,
            ..default()
        };
        let m = EyeView::lerp(&a, &b, 0.5);
        // Halfway across 0 (2 pi), not back around through pi.
        assert!(wrap(m.yaw - (6.2 + (std::f32::consts::TAU + 0.1 - 6.2) / 2.0)).abs() < 1e-4, "{}", m.yaw);
    }

    #[test]
    fn game_clock_lags_just_enough() {
        // Halo's own tick: one tick behind, the overstep the blend.
        let own = GameClock { per_tick: (30, 30) };
        assert_eq!(own.lag(), 1.0);
        assert_eq!(own.drawn(10, 0.25), 9.25);
        // 30 Hz under 64 Hz: the next value is always laid.
        let party = GameClock { per_tick: (30, 64) };
        assert!((party.lag() - 1.4375).abs() < 1e-12);
        for k in 1..2000u64 {
            for o in [0.0, 0.5, 0.999] {
                let t = party.drawn(k, o).max(0.0);
                assert!(t.ceil() as u64 <= party.laid(k), "{k} {o}: {t}");
            }
        }
    }

    #[test]
    fn game_ticks_blend_the_two_around() {
        let mut g = GameTicks::default();
        let p = |x: f32| Later::ready(vec![(Quat::from_rotation_y(x), Vec3::X * x)]);
        g.lay(4, p(0.0));
        g.lay(5, p(1.0));
        let at = g.at(4.25).unwrap();
        let (q, v) = at.get()[0];
        assert!(q.dot(Quat::from_rotation_y(0.25)).abs() > 1.0 - 1e-6 && (v - Vec3::X * 0.25).length() < 1e-6);
        assert!(Later::ptr_eq(&g.at(9.0).unwrap(), g.latest().unwrap()), "after the latest: the latest");
        // A tick laid again (a run restarted) replaces what came after.
        g.lay(5, p(2.0));
        g.lay(3, p(3.0));
        assert_eq!(g.around(0.0).map(|(_, _, f)| f), Some(1.0));
        assert_eq!(g.latest().unwrap().get()[0].1, Vec3::X * 3.0);
    }
}
