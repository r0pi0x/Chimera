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
//!   required component (the client: grenades and every game's flyers,
//!   `weapon::projectile::Flyer`).
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
//! - An entity its game draws on its own clock (`DrawnByGame`: Halo's
//!   vehicles, eased between Halo's ticks) keeps its ticks sampled here
//!   but is neither eased nor put back: the game does both.
//! - A carried character (`core::Carried`: a vehicle's seat) is drawn
//!   with its carrier (`ease_carried`, `InterpSystems::Carry`, after every
//!   ease): where it sits in the carrier's latest tick, moved with the
//!   carrier as drawn this frame, so it rides smoothly however the carrier
//!   is drawn (the engine's ticks or a game's own clock).
//!
//! - `GameTicks` is for a game whose own tick is slower than the
//!   engine's (Halo's 30 Hz): values laid at the game's ticks, drawn
//!   between them on the game's clock (`GameClock`); the game eases and
//!   puts back its own (`halo_ce::game::eased`).
//! - `PoseStream` is for a pose sampled from keyframed animation at a
//!   game's tick: drawn at the frame's own time (`GameClock::now`, no
//!   trail), between the tick's pose and the poses the game predicts for
//!   its next ticks (the same animation played on with the same inputs);
//!   a tick that turns out otherwise than predicted cross-fades from the
//!   old prediction over one game tick (`FADE_TICKS`), unless the game
//!   says the change is an event to show at once (a shot).
//!
//! Plugged in by the client (headless runs and tests without it see the
//! simulation's values only); `Interpolation::enabled` is its cvar
//! (`cl_interpolate`).

use avian3d::prelude::RigidBody;
use bevy::{
    app::RunFixedMainLoopSystems,
    prelude::*,
    transform::systems::{propagate_parent_transforms, sync_simple_transforms},
};

use chimera_core::later::Later;

use super::anim::BonePose;
use crate::core::{Carried, Intent, LocalPlayer, MovementState};

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
    /// After `Ease`: carried characters drawn with their carriers.
    Carry,
}

/// Drawn by a network client from the server's snapshots at its own
/// render time (`net::interp`), not between this process's ticks: the
/// systems here skip it.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct NetDrawn;

/// Drawn by its game on the game's own clock (Halo's vehicles: between
/// Halo's ticks, `halo_ce::game::eased`): its `Interpolated` ticks are
/// still sampled (a carried rider is drawn by them, `ease_carried`), but
/// the engine neither eases it nor puts it back; the game does both.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct DrawnByGame;

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
    /// `MovementState::view_shift`, meters: the camera only.
    pub view_shift: f32,
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
            view_shift: state.view_shift,
            view_roll: state.view_roll,
            yaw: intent.yaw,
            pitch: intent.pitch,
            punch: Vec3::ZERO,
        }
    }

    /// The eye the camera is drawn at, above the origin (with the shift).
    pub fn camera_offset(&self) -> Vec3 {
        self.eye_offset + Vec3::Y * self.view_shift
    }

    pub fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        Self {
            eye_offset: a.eye_offset.lerp(b.eye_offset, t),
            view_shift: a.view_shift + (b.view_shift - a.view_shift) * t,
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
                (InterpSystems::Ease, InterpSystems::Carry)
                    .chain()
                    .in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
            )
            .add_systems(
                FixedFirst,
                (
                    restore,
                    // The simulation reads globals too (this tick's, not
                    // the drawn ones). Marked without Bevy's tasks
                    // (`transform_marks`).
                    super::transform_marks::mark_changed_trees,
                    propagate_parent_transforms,
                    sync_simple_transforms,
                )
                    .chain()
                    .in_set(InterpSystems::Restore),
            )
            .add_systems(FixedLast, (sample_transforms, sample_views).in_set(InterpSystems::Sample))
            .add_systems(FixedLast, snap.in_set(InterpSystems::Snap))
            .add_systems(RunFixedMainLoop, (ease_transforms, ease_views).in_set(InterpSystems::Ease))
            .add_systems(RunFixedMainLoop, ease_carried.in_set(InterpSystems::Carry))
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
    mut q: Query<(&mut Transform, &mut Interpolated), (Without<NetDrawn>, Without<DrawnByGame>)>,
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
    mut q: Query<(&mut Transform, &mut Interpolated), (Without<NetDrawn>, Without<DrawnByGame>)>,
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

/// Carried characters with their carriers (module doc): the rider's
/// place in its carrier's latest tick, moved with the carrier as drawn;
/// its rotation as eased. Only where both were sampled, and only
/// interpolating (`cl_interpolate 0` draws the latest ticks).
#[allow(clippy::type_complexity)]
fn ease_carried(
    settings: Res<Interpolation>,
    mut riders: Query<(&Carried, &mut Transform, &mut Interpolated), Without<NetDrawn>>,
    carriers: Query<(&Transform, &Interpolated), Without<Carried>>,
) {
    if settings.enabled == 0 {
        return;
    }
    for (carried, mut t, mut i) in &mut riders {
        let Ok((drawn, carrier)) = carriers.get(carried.0) else { continue };
        if !i.sampled || !carrier.sampled {
            continue;
        }
        let local = carrier.cur.compute_affine().inverse().transform_point3(i.cur.translation);
        let want = Transform {
            translation: drawn.compute_affine().transform_point3(local),
            ..*t
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
#[derive(Clone, Debug, PartialEq)]
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

    /// The latest value laid with its game tick.
    pub fn last(&self) -> Option<(u64, &T)> {
        self.laid.back().map(|(t, v)| (*t, v))
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

/// Pose `a` toward `b` by `f`: each bone's rotation by the shorter arc,
/// its position straight; poses of different skeletons give `b`.
pub fn blend_poses(a: &[BonePose], b: &[BonePose], f: f32) -> Vec<BonePose> {
    if a.len() != b.len() {
        return b.to_vec();
    }
    a.iter().zip(b).map(|((qa, pa), (qb, pb))| (qa.slerp(*qb, f), pa.lerp(*pb, f))).collect()
}

/// How long, in game ticks, a pose stream fades from a prediction that
/// turned out wrong to the new tick's (`PoseStream`).
pub const FADE_TICKS: f64 = 1.0;

/// A pose sampled from keyframed animation at a game's tick, drawn at the
/// frame's own time (module doc): `lay` the tick's pose with a maker of
/// the next ticks' (`GameClock::ahead` of them), `at` the frame's game
/// time (`GameClock::now`). Between the tick and the next the pose runs
/// on the predicted keyframes (each bone between two of them, as the
/// animation itself would be sampled), so a playing animation shows no
/// trail; past the predicted ones it holds the last. The simulation keeps
/// the tick's (`latest`).
#[derive(Clone, Debug, Default)]
pub struct PoseStream {
    run: Option<PoseRun>,
    /// The run a tick replaced otherwise than it predicted, fading out
    /// from the first frame drawn after (its time).
    old: Option<(PoseRun, Option<f64>)>,
}

#[derive(Clone, Debug)]
struct PoseRun {
    tick: u64,
    now: Later<Vec<BonePose>>,
    ahead: Later<Vec<Vec<BonePose>>>,
}

impl PoseRun {
    /// The pose at game time `time`: the tick's before it, between the
    /// keyframes around it after, the last predicted past them.
    fn sample(&self, time: f64) -> Later<Vec<BonePose>> {
        let s = time - self.tick as f64;
        if s <= 0.0 {
            return self.now.clone();
        }
        let i = s.floor();
        let f = (s - i) as f32;
        let i = i.min(1e6) as usize;
        let (now, ahead) = (self.now.clone(), self.ahead.clone());
        Later::new(move || {
            let pick = |k: usize| -> &Vec<BonePose> {
                if k == 0 {
                    return now.get();
                }
                let a = ahead.get();
                a.get(k - 1).or(a.last()).unwrap_or(now.get())
            };
            if f <= 0.0 { pick(i).clone() } else { blend_poses(pick(i), pick(i + 1), f) }
        })
    }

    /// What this run predicted for game tick `tick`, where it was made
    /// (drawn).
    fn predicted(&self, tick: u64) -> Option<&Vec<BonePose>> {
        let k = tick.checked_sub(self.tick)?.checked_sub(1)?;
        if !self.ahead.is_made() {
            return None;
        }
        let a = self.ahead.get();
        a.get(k as usize).or(a.last())
    }
}

impl PoseStream {
    /// The game's pose `now` at its tick `tick`, and the maker of the
    /// poses of its next ticks (called when first drawn). `fade`: a pose
    /// other than predicted fades in from the old prediction (input that
    /// drifted, a state the animation changed to); false shows it at once
    /// (an event: a shot, a hit).
    pub fn lay(
        &mut self,
        tick: u64,
        now: Later<Vec<BonePose>>,
        ahead: impl FnOnce() -> Vec<Vec<BonePose>> + Send + 'static,
        fade: bool,
    ) {
        let old = self.run.take();
        self.old = None;
        if let Some(old) = old
            && fade
            && old.tick < tick
            && old.predicted(tick).is_some_and(|p| p != now.get())
        {
            self.old = Some((old, None));
        }
        self.run = Some(PoseRun {
            tick,
            now,
            ahead: Later::new(ahead),
        });
    }

    /// Forget what was laid (a model changed).
    pub fn clear(&mut self) {
        self.run = None;
        self.old = None;
    }

    /// The latest tick's pose (the simulation's).
    pub fn latest(&self) -> Option<&Later<Vec<BonePose>>> {
        self.run.as_ref().map(|r| &r.now)
    }

    /// The pose drawn at game time `time` (module doc); a time that isn't
    /// finite (the interpolation off) draws the latest. Made when first
    /// posed (`Later`).
    pub fn at(&mut self, time: f64) -> Option<Later<Vec<BonePose>>> {
        let run = self.run.as_ref()?;
        if !time.is_finite() {
            return Some(run.now.clone());
        }
        let new = run.sample(time);
        let Some((old, start)) = &mut self.old else { return Some(new) };
        let u = ((time - *start.get_or_insert(time)) / FADE_TICKS) as f32;
        if u >= 1.0 {
            self.old = None;
            return Some(new);
        }
        let from = old.sample(time);
        Some(Later::new(move || blend_poses(from.get(), new.get(), u)))
    }
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

    /// The game time of a frame `overstep` (0 to 1) of an engine tick
    /// after the end of engine tick `tick`, not behind (`PoseStream`).
    pub fn now(&self, tick: u64, overstep: f32) -> f64 {
        let (n, d) = self.per_tick;
        (tick as f64 + f64::from(overstep.clamp(0.0, 1.0))) * n as f64 / d.max(1) as f64
    }

    /// The game ticks a `PoseStream` predicts past its latest: as far as a
    /// frame is ever past the latest (`lag`) and the fade from the one
    /// before (`FADE_TICKS`); 2 at the game's own tick, 3 for 30 Hz under
    /// 64 Hz.
    pub fn ahead(&self) -> usize {
        (self.lag() + FADE_TICKS).ceil() as usize
    }

    /// The game time a frame draws, `overstep` (0 to 1) of an engine tick
    /// after the end of engine tick `tick`, `lag` behind.
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
        let p = |x: f32| Transform::from_translation(Vec3::X * x).with_rotation(Quat::from_rotation_y(x));
        g.lay(4, p(0.0));
        g.lay(5, p(1.0));
        let at = g.at(4.25).unwrap();
        assert!(at.rotation.dot(Quat::from_rotation_y(0.25)).abs() > 1.0 - 1e-6);
        assert!((at.translation - Vec3::X * 0.25).length() < 1e-6);
        assert_eq!(g.at(9.0), g.latest().copied(), "after the latest: the latest");
        // A tick laid again (a run restarted) replaces what came after.
        g.lay(5, p(2.0));
        g.lay(3, p(3.0));
        assert_eq!(g.around(0.0).map(|(_, _, f)| f), Some(1.0));
        assert_eq!(g.latest().unwrap().translation, Vec3::X * 3.0);
    }

    /// A one-bone animation turning `step` radians a frame: frame `f`.
    fn key(f: f32, step: f32) -> Vec<BonePose> {
        vec![(Quat::from_rotation_z(f * step), Vec3::Y * f)]
    }

    fn close(a: &[BonePose], b: &[BonePose]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|((qa, pa), (qb, pb))| qa.dot(*qb).abs() > 1.0 - 1e-6 && pa.distance(*pb) < 1e-5)
    }

    /// Laid at tick `t` playing frame `t` of the animation, predicting
    /// the next frames.
    fn lay_frame(s: &mut PoseStream, t: u64, step: f32, fade: bool) {
        s.lay(t, Later::ready(key(t as f32, step)), move || (1..=3).map(|k| key((t + k) as f32, step)).collect(), fade);
    }

    #[test]
    fn a_pose_stream_samples_the_keyframes_past_its_tick() {
        let mut s = PoseStream::default();
        lay_frame(&mut s, 10, 0.1, true);
        // At the tick, between it and the next, past it: the animation at
        // that time, no trail.
        for t in [10.0, 10.25, 10.5, 11.0, 11.4] {
            let got = s.at(t).unwrap();
            assert!(close(got.get(), &key(t as f32, 0.1)), "{t}: {:?}", got.get());
        }
        assert!(close(s.at(f64::INFINITY).unwrap().get(), &key(10.0, 0.1)), "off: the tick's");
        assert!(close(s.latest().unwrap().get(), &key(10.0, 0.1)));
        // The next tick as predicted: no fade, the same path.
        lay_frame(&mut s, 11, 0.1, true);
        assert!(s.old.is_none());
        assert!(close(s.at(11.5).unwrap().get(), &key(11.5, 0.1)));
    }

    #[test]
    fn a_changed_tick_fades_from_the_prediction() {
        let mut s = PoseStream::default();
        lay_frame(&mut s, 10, 0.1, true);
        let before = s.at(10.95).unwrap().get().clone();
        // The next tick plays another animation (twice as fast): the first
        // frame after draws the old prediction, then fades to the new one
        // over a tick.
        lay_frame(&mut s, 11, 0.2, true);
        let first = s.at(11.0).unwrap().get().clone();
        assert!(close(&first, &key(11.0, 0.1)), "the old path at first: {first:?}");
        let step = before[0].0.angle_between(first[0].0);
        assert!(step < 0.1, "no pop: {step}");
        let mid = s.at(11.5).unwrap().get().clone();
        let want = blend_poses(&key(11.5, 0.1), &key(11.5, 0.2), 0.5);
        assert!(close(&mid, &want), "{mid:?} vs {want:?}");
        assert!(close(s.at(12.0).unwrap().get(), &key(12.0, 0.2)), "faded: the new path");
        // An event shows at once.
        lay_frame(&mut s, 12, 0.5, false);
        assert!(close(s.at(12.0).unwrap().get(), &key(12.0, 0.5)));
    }
}
