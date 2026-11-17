//! Stand-ins: what shows, for every game, wherever a real model isn't in
//! yet (docs/backlog.md, "Stand-in assets that say loading"). One look
//! everywhere: our own low-poly shapes (`stand_in_mesh`: a humanoid body,
//! a generic gun, a crate) in a loading material (`StandInMaterial`,
//! stand_in.wesl: light and faceted, cobalt accents, light stripes
//! scrolling slowly over it), so a player reads "something comes here",
//! not an error. The real model replaces it in the frame it is drawn.
//!
//! Where they show (each after `StandInSettings::grace` of waiting, so a
//! model that is a frame or two late never flashes one):
//!
//! - **Bodies** (`bodies`): a character (`Intent`, alive, not ragdolled)
//!   with no `bodies::CharacterBody` drawn: before the map's or the
//!   kit's models are in (a sandbox's start: the code-built maps' kit on
//!   its task), a late body not built yet (`build_late_bodies`), a
//!   character whose game has no body on this map. Turned with the
//!   character's yaw, at its feet, as tall as its box (crouched: lower),
//!   accented in its team's colour (`TEAM_ACCENTS`); holding the stand-in
//!   gun when it holds anything (`map::Held`). A body once drawn stays
//!   until the new one is ready (`attach_bodies`): a stand-in never
//!   replaces a real body.
//! - **Held guns** (`held`): a real body holding a model not built yet
//!   (`LateHeldModels`, `build_late_held`): the gun on the held model's
//!   bone, pointing at its muzzle.
//! - **View guns** (`view_guns`): a first-person view whose view model is
//!   wanted but not built (`view_model::draw_view_models` marks its
//!   camera `ViewModelWaiting`; a late model, `build_late`, or a game's
//!   on-demand load): the gun on the view-model layer, right or left hand
//!   (`ViewModelSettings::right_hand`).
//! - **Dropped guns** (`loose`): a `LooseItem` whose held model is still
//!   coming (`LateHeldModels`, or the kit not in), lying on its side.
//! - **Anything else** (`requested`): a game or loader puts `WantsStandIn`
//!   (a kind and a size) on an entity whose model is still coming (a prop:
//!   the crate, scaled to its half size) and takes it off when the model
//!   is in. `standin_spawn <body|gun|prop>` puts one ahead of the local
//!   player, a little to the left (for looking at them).
//!
//! **Settings**: `chimera_standins` (`StandInSettings::mode`): 0 never, 1
//! where a model isn't in yet (default), 2 always (the real bodies and
//! view models hidden: for looking at the stand-ins). The stripes move at
//! `SCROLL_RATE` times `StandInScroll::rate` (the client sets it from
//! `ui_motion_scale`: 0 holds them still, 2 halves their speed); a test
//! run (`tasks::TestRun`) holds them still and waits no grace, so tests
//! and their frames repeat.
//!
//! **Measured** (`StandInStats`, `stand_ins` in the console): each
//! stand-in's time on screen (wall time, `Time<Real>`) by kind; at debug
//! level a line as each goes, and at info one line when the last one
//! showing goes (`stand-ins: 16 bodies 0.41-0.83 s, ...`): how long a
//! load (a sandbox's start, a map change) showed them.
//!
//! Known shortcuts: the stand-in body is not animated (it stands, turned
//! and crouched with its character) and has no hitboxes of its own (its
//! character's are the simulation's); one gun shape for every weapon,
//! knives and grenades included; a `ShownItem` (a thrown grenade) without
//! its model still draws nothing (`render::loose::attach_shown`); a
//! character whose game has no body on the map keeps the stand-in for
//! good (our own default bodies, the backlog's logo characters, are to
//! replace it). Why it's OK for now: they show for a fraction of a second
//! in a normal load (docs/performance.md, "Stand-ins on screen").

use std::collections::HashMap;

use avian3d::prelude::ColliderAabb;
use bevy::{
    asset::embedded_asset,
    camera::visibility::RenderLayers,
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    light::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
    reflect::TypePath,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
use chimera_core::{
    console::{ConsoleAppExt, resource_cvar},
    core::{Dead, Intent, LocalPlayer, Team},
};

use super::{
    bodies::{CharacterBody, HeldAssets, PlayModels},
    loose::LooseItemDrawn,
    stand_in_mesh,
};
use crate::map::{
    Held, LateHeldModels, ShowLocalBody, ViewModelCamera, ViewModelSettings, anim::Animator,
    view_model::VIEW_MODEL_LAYER,
    interp::RenderedView, loose::LooseItem, ragdoll::Ragdolled,
};

/// Seconds a model may be late before its stand-in shows.
pub const GRACE: f64 = 0.15;

/// The stripes' speed: periods a second, at `StandInScroll::rate` 1.
pub const SCROLL_RATE: f32 = 0.45;

/// The stand-ins' surface (srgb).
pub const BASE: Color = Color::srgb(0.72, 0.76, 0.84);
/// The accent of a stand-in of no team (a gun, a prop): the party's cobalt.
pub const COBALT: Color = Color::srgb(0.16, 0.36, 1.0);
/// A body's accent by its team (0, 1; other teams take 0's).
pub const TEAM_ACCENTS: [Color; 2] = [COBALT, Color::srgb(1.0, 0.45, 0.12)];
/// The stripes' colour.
pub const STRIPE: Color = Color::srgb(0.36, 0.52, 1.0);

/// What a stand-in stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StandInKind {
    Body,
    /// A gun in a hand or on the ground.
    Gun,
    /// A gun in first person.
    ViewGun,
    Prop,
}

impl StandInKind {
    pub const ALL: [StandInKind; 4] = [StandInKind::Body, StandInKind::Gun, StandInKind::ViewGun, StandInKind::Prop];

    fn name(self, n: usize) -> &'static str {
        match (self, n == 1) {
            (StandInKind::Body, true) => "body",
            (StandInKind::Body, false) => "bodies",
            (StandInKind::Gun, true) => "gun",
            (StandInKind::Gun, false) => "guns",
            (StandInKind::ViewGun, true) => "view gun",
            (StandInKind::ViewGun, false) => "view guns",
            (StandInKind::Prop, true) => "prop",
            (StandInKind::Prop, false) => "props",
        }
    }
}

/// A stand-in on screen: what it stands for and since when (s of
/// `Time<Real>`). Counted in `StandInStats` as it comes and goes.
#[derive(Component, Clone, Copy, Debug)]
#[component(on_add = stand_in_added, on_remove = stand_in_removed)]
pub struct StandIn {
    pub kind: StandInKind,
    pub since: f64,
}

/// `chimera_standins` and the grace (module doc).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct StandInSettings {
    /// 0 never, 1 where a model isn't in yet, 2 always.
    pub mode: u8,
    /// Seconds a model may be late before its stand-in shows.
    pub grace: f64,
}

impl Default for StandInSettings {
    fn default() -> Self {
        Self { mode: 1, grace: GRACE }
    }
}

/// The stripes' position (periods moved) and speed factor (module doc).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct StandInScroll {
    pub phase: f32,
    pub rate: f32,
}

impl Default for StandInScroll {
    fn default() -> Self {
        Self { phase: 0.0, rate: 1.0 }
    }
}

/// Time on screen of one kind of stand-in: how many went, their total,
/// shortest and longest (s).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shown {
    pub count: usize,
    pub total: f64,
    pub min: f64,
    pub max: f64,
}

impl Shown {
    fn add(&mut self, secs: f64) {
        self.min = if self.count == 0 { secs } else { self.min.min(secs) };
        self.max = self.max.max(secs);
        self.count += 1;
        self.total += secs;
    }
}

/// The stand-ins' times on screen (module doc): this session's by kind,
/// and the current run's (from the first stand-in shown while none was,
/// to the last one gone).
#[derive(Resource, Clone, Debug, Default)]
pub struct StandInStats {
    /// Showing now.
    pub showing: usize,
    pub session: HashMap<StandInKind, Shown>,
    pub run: HashMap<StandInKind, Shown>,
    /// The runs ended so far, each as `summary` put it.
    pub runs: Vec<String>,
}

impl StandInStats {
    /// One line of `shown`'s counts and times by kind.
    pub fn summary(shown: &HashMap<StandInKind, Shown>) -> String {
        let parts: Vec<String> = StandInKind::ALL
            .iter()
            .filter_map(|k| {
                let s = shown.get(k).filter(|s| s.count > 0)?;
                let times = if s.count == 1 || s.max - s.min < 0.005 {
                    format!("{:.2} s", s.max)
                } else {
                    format!("{:.2}-{:.2} s (mean {:.2})", s.min, s.max, s.total / s.count as f64)
                };
                Some(format!("{} {} {times}", s.count, k.name(s.count)))
            })
            .collect();
        if parts.is_empty() { "none".into() } else { parts.join(", ") }
    }
}

fn now_real(world: &DeferredWorld) -> f64 {
    world.get_resource::<Time<Real>>().map_or(0.0, |t| t.elapsed_secs_f64())
}

fn stand_in_added(mut world: DeferredWorld, _ctx: HookContext) {
    if let Some(mut stats) = world.get_resource_mut::<StandInStats>() {
        stats.showing += 1;
    }
}

fn stand_in_removed(mut world: DeferredWorld, ctx: HookContext) {
    let Some(s) = world.get::<StandIn>(ctx.entity).copied() else { return };
    let secs = (now_real(&world) - s.since).max(0.0);
    let Some(mut stats) = world.get_resource_mut::<StandInStats>() else { return };
    debug!("stand-in {:?} gone after {secs:.2} s", s.kind);
    stats.showing = stats.showing.saturating_sub(1);
    stats.session.entry(s.kind).or_default().add(secs);
    stats.run.entry(s.kind).or_default().add(secs);
    if stats.showing == 0 {
        let line = StandInStats::summary(&stats.run);
        info!("stand-ins: {line} on screen until the real models came");
        stats.runs.push(line);
        stats.run.clear();
    }
}

#[derive(Clone, Copy, Debug, Default, bevy::render::render_resource::ShaderType)]
pub struct StandInParams {
    pub base: Vec4,
    pub accent: Vec4,
    /// rgb the stripes' colour, a their strength.
    pub stripe: Vec4,
    /// Stripes moved (periods).
    pub scroll: f32,
    /// Stripes per metre.
    pub density: f32,
    /// The stripes' share of a period.
    pub duty: f32,
    /// The accent rim's strength.
    pub rim: f32,
}

impl StandInParams {
    pub fn new(accent: Color) -> Self {
        let rgba = |c: Color, a: f32| {
            let l = c.to_linear();
            Vec4::new(l.red, l.green, l.blue, a)
        };
        Self {
            base: rgba(BASE, 1.0),
            accent: rgba(accent, 1.0),
            stripe: rgba(STRIPE, 0.5),
            scroll: 0.0,
            density: 7.0,
            duty: 0.38,
            rim: 0.3,
        }
    }
}

/// The loading material (stand_in.wesl).
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct StandInMaterial {
    #[uniform(0)]
    pub params: StandInParams,
}

impl Material for StandInMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://chimera_present/render/stand_in.wesl".into()
    }
}

/// The material's shader and pipeline (in `PresentationPlugins`).
pub struct StandInMaterialPlugin;

impl Plugin for StandInMaterialPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "stand_in.wesl");
        app.add_plugins(MaterialPlugin::<StandInMaterial>::default());
    }
}

/// The stand-ins' meshes and materials, made once and kept for the
/// session (a few kB).
#[derive(Resource, Clone)]
pub struct StandIns {
    pub body: Handle<Mesh>,
    pub gun: Handle<Mesh>,
    pub prop: Handle<Mesh>,
    /// Cobalt: guns, props, a body of no team.
    pub neutral: Handle<StandInMaterial>,
    /// A body's by its team (`TEAM_ACCENTS`).
    pub teams: [Handle<StandInMaterial>; 2],
}

impl StandIns {
    pub fn for_team(&self, team: Option<&Team>) -> Handle<StandInMaterial> {
        match team {
            Some(t) => self.teams[usize::from(t.0 == 1)].clone(),
            None => self.neutral.clone(),
        }
    }

    pub fn materials(&self) -> impl Iterator<Item = &Handle<StandInMaterial>> {
        std::iter::once(&self.neutral).chain(&self.teams)
    }

    fn mesh(&self, kind: StandInKind) -> Handle<Mesh> {
        match kind {
            StandInKind::Body => self.body.clone(),
            StandInKind::Gun | StandInKind::ViewGun => self.gun.clone(),
            StandInKind::Prop => self.prop.clone(),
        }
    }
}

/// Make `StandIns` where there are meshes and the material (not on a
/// server).
pub fn make_assets(
    have: Option<Res<StandIns>>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandInMaterial>>>,
    mut commands: Commands,
) {
    let (None, Some(mut meshes), Some(mut materials)) = (have, meshes, materials) else {
        return;
    };
    let mut material = |c: Color| materials.add(StandInMaterial { params: StandInParams::new(c) });
    let made = StandIns {
        neutral: material(COBALT),
        teams: [material(TEAM_ACCENTS[0]), material(TEAM_ACCENTS[1])],
        body: meshes.add(stand_in_mesh::body()),
        gun: meshes.add(stand_in_mesh::gun()),
        prop: meshes.add(stand_in_mesh::crate_box()),
    };
    commands.insert_resource(made);
}

/// Move the stripes (module doc), written into the materials while any
/// stand-in shows.
pub fn scroll(
    time: Res<Time<Real>>,
    test: Option<Res<chimera_core::tasks::TestRun>>,
    mut scroll: ResMut<StandInScroll>,
    stats: Res<StandInStats>,
    assets: Option<Res<StandIns>>,
    mut materials: Option<ResMut<Assets<StandInMaterial>>>,
) {
    if test.is_none() {
        scroll.phase = (scroll.phase + time.delta_secs() * SCROLL_RATE * scroll.rate.max(0.0)).fract();
    }
    let (Some(assets), Some(materials)) = (assets, materials.as_mut()) else { return };
    if stats.showing == 0 {
        return;
    }
    for h in assets.materials() {
        if materials.get(h).is_some_and(|m| m.params.scroll != scroll.phase)
            && let Some(mut m) = materials.get_mut(h)
        {
            m.params.scroll = scroll.phase;
        }
    }
}

/// Whether `key` has waited out the grace (first asked now: waits).
fn waited(waiting: &mut HashMap<Entity, f64>, key: Entity, now: f64, grace: f64) -> bool {
    now - *waiting.entry(key).or_insert(now) >= grace
}

fn stand_in_bundle(assets: &StandIns, kind: StandInKind, material: Handle<StandInMaterial>, now: f64) -> impl Bundle {
    (
        Name::new(format!("Stand-in {kind:?}")),
        StandIn { kind, since: now },
        Mesh3d(assets.mesh(kind)),
        MeshMaterial3d(material),
    )
}

/// Marks a real body hidden by `chimera_standins 2`.
#[derive(Component)]
pub struct HiddenForStandIn;

/// The gun a stand-in body holds.
#[derive(Component)]
pub struct StandInHeldGun;

/// Give characters with no drawn body a stand-in (module doc).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn bodies(
    characters: Query<
        (
            Entity,
            Option<&Team>,
            Option<&Children>,
            (Option<&ColliderAabb>, &GlobalTransform),
            (Has<LocalPlayer>, Has<Dead>),
            (&Intent, Option<&RenderedView>, Option<&Animator>, Option<&Held>),
        ),
        Without<Ragdolled>,
    >,
    real: Query<(Entity, Has<HiddenForStandIn>), With<CharacterBody>>,
    mut real_visibility: Query<&mut Visibility, With<CharacterBody>>,
    mut shown: Query<(&StandIn, &mut Transform, Option<&Children>), Without<StandInHeldGun>>,
    guns: Query<(), With<StandInHeldGun>>,
    (settings, assets, show_local, time): (Res<StandInSettings>, Option<Res<StandIns>>, Option<Res<ShowLocalBody>>, Res<Time<Real>>),
    mut waiting: Local<HashMap<Entity, f64>>,
    mut commands: Commands,
) {
    let Some(assets) = assets else { return };
    let now = time.elapsed_secs_f64();
    let show_local = show_local.map(|s| *s).unwrap_or_default();
    waiting.retain(|e, _| characters.contains(*e));
    for (e, team, children, (aabb, at), (local, dead), (intent, view, animator, held)) in &characters {
        let kids = || children.into_iter().flatten().copied();
        let real_body = kids().find_map(|c| real.get(c).ok());
        let stand_in = kids().find(|c| shown.get(*c).is_ok_and(|(s, ..)| s.kind == StandInKind::Body));
        // `chimera_standins 2` hides the real body; back to normal shows it.
        if let Some((body, hidden)) = real_body {
            let force = settings.mode == 2 && !dead;
            if let Ok(mut v) = real_visibility.get_mut(body)
                && (force || hidden)
            {
                // Each frame: the local body's own switch shows it again.
                if force {
                    v.set_if_neq(Visibility::Hidden);
                    if !hidden {
                        commands.entity(body).insert(HiddenForStandIn);
                    }
                } else {
                    *v = super::bodies::body_visibility(local, show_local);
                    commands.entity(body).remove::<HiddenForStandIn>();
                }
            }
        }
        // The local player's own body only when shown (`ShowLocalBody`).
        let want = !dead && (!local || show_local.0) && match settings.mode {
            0 => false,
            1 => real_body.is_none(),
            _ => true,
        };
        if !want {
            waiting.remove(&e);
            if let Some(s) = stand_in {
                commands.entity(s).despawn();
            }
            continue;
        }
        let feet = aabb.map_or(-0.5 * stand_in_mesh::STAND_HEIGHT, |a| a.min.y - at.translation().y);
        let height = aabb.map_or(stand_in_mesh::STAND_HEIGHT, |a| a.max.y - a.min.y);
        let yaw = animator.and_then(|a| a.yaw).unwrap_or(view.map_or(intent.yaw, |v| v.now.yaw));
        let placed = Transform::from_xyz(0.0, feet, 0.0)
            .with_rotation(Quat::from_rotation_y(yaw))
            .with_scale(Vec3::new(1.0, (height / stand_in_mesh::STAND_HEIGHT).clamp(0.3, 1.3), 1.0));
        let holds = held.is_some_and(|h| h.0.is_some());
        match stand_in {
            Some(s) => {
                let Ok((_, mut t, kids)) = shown.get_mut(s) else { continue };
                t.set_if_neq(placed);
                let gun = kids.into_iter().flatten().copied().find(|c| guns.contains(*c));
                match (holds, gun) {
                    (true, None) => {
                        commands.spawn(held_gun(&assets, s));
                    }
                    (false, Some(g)) => commands.entity(g).despawn(),
                    _ => {}
                }
            }
            None => {
                if !waited(&mut waiting, e, now, settings.grace) {
                    continue;
                }
                let body = commands
                    .spawn((
                        stand_in_bundle(&assets, StandInKind::Body, assets.for_team(team), now),
                        placed,
                        super::bodies::body_visibility(local, show_local),
                        ChildOf(e),
                    ))
                    .id();
                if holds {
                    commands.spawn(held_gun(&assets, body));
                }
            }
        }
    }
}

/// The gun in a stand-in body's right hand (part of the body: not counted
/// on its own).
fn held_gun(assets: &StandIns, body: Entity) -> impl Bundle {
    (
        StandInHeldGun,
        Mesh3d(assets.gun.clone()),
        MeshMaterial3d(assets.neutral.clone()),
        Transform::from_translation(stand_in_mesh::RIGHT_HAND),
        ChildOf(body),
    )
}

/// A stand-in gun on a real body's bone, for its held model `key`.
#[derive(Component)]
pub struct StandInHeld {
    pub body: Entity,
    pub key: String,
}

/// Put the stand-in gun in a real body's hand while its held model is
/// built (module doc).
#[allow(clippy::too_many_arguments)]
pub fn held(
    characters: Query<(&Held, &Children)>,
    bodies: Query<(Entity, &CharacterBody)>,
    shown: Query<(Entity, &StandInHeld)>,
    joints: Query<&GlobalTransform>,
    (built, late, play): (Option<Res<HeldAssets>>, Option<Res<LateHeldModels>>, Option<Res<PlayModels>>),
    (settings, assets, time): (Res<StandInSettings>, Option<Res<StandIns>>, Res<Time<Real>>),
    mut waiting: Local<HashMap<Entity, f64>>,
    mut commands: Commands,
) {
    let (Some(assets), Some(late), Some(play)) = (assets, late, play) else { return };
    let now = time.elapsed_secs_f64();
    // What should show: (body, key) → (joint, the held model's muzzle).
    let mut wanted = HashMap::new();
    for (Held(key), children) in &characters {
        let Some(key) = key else { continue };
        let Some((body_entity, body)) = children.iter().find_map(|c| bodies.get(c).ok()) else { continue };
        if settings.mode == 0 || body.held.is_some() || built.as_ref().is_some_and(|b| b.0.contains_key(key)) {
            continue;
        }
        let Some((model, _)) = late.0.iter().find(|(m, _)| &m.key == key) else { continue };
        let Some(drawn) = play.body(body.model) else { continue };
        let joint = drawn
            .bones
            .iter()
            .position(|b| b.name.eq_ignore_ascii_case(&model.bone))
            .and_then(|i| body.joints.get(i).copied());
        if let Some(joint) = joint {
            wanted.insert((body_entity, key.clone()), (joint, model.muzzle));
        }
    }
    for (e, s) in &shown {
        if wanted.remove(&(s.body, s.key.clone())).is_none() {
            commands.entity(e).despawn();
        }
    }
    waiting.retain(|b, _| wanted.keys().any(|(w, _)| w == b));
    for ((body, key), (joint, muzzle)) in wanted {
        if !waited(&mut waiting, body, now, settings.grace) {
            continue;
        }
        // In metres whatever the skeleton's units; pointing at the muzzle
        // (+X of the bone without one).
        let scale = joints.get(joint).map_or(1.0, |g| g.compute_transform().scale.x.abs().max(1e-6));
        let toward = muzzle.map(|m| m.translation).filter(|t| t.length() > 1e-4).unwrap_or(Vec3::X);
        commands.spawn((
            stand_in_bundle(&assets, StandInKind::Gun, assets.neutral.clone(), now),
            StandInHeld { body, key },
            Transform::from_rotation(Quat::from_rotation_arc(Vec3::NEG_Z, toward.normalize()))
                .with_scale(Vec3::splat(1.0 / scale)),
            ChildOf(joint),
        ));
    }
}

/// Marks a view-model camera whose view wants a model that isn't built
/// (`view_model::draw_view_models`).
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct ViewModelWaiting;

/// The stand-in gun in first person.
#[derive(Component)]
pub struct ViewStandIn;

/// Where the view gun sits in the camera's space (right hand; m).
pub const VIEW_GUN_AT: Vec3 = Vec3::new(0.15, -0.16, -0.5);
/// The view gun's scale: the view model's narrow field of view shows it
/// larger than in a hand.
pub const VIEW_GUN_SCALE: f32 = 0.5;

/// The stand-in gun on each view-model camera whose model is waiting
/// (module doc).
pub fn view_guns(
    cameras: Query<(Entity, Has<ViewModelWaiting>, Option<&Children>), With<ViewModelCamera>>,
    shown: Query<(), With<ViewStandIn>>,
    mut placed: Query<&mut Transform, With<ViewStandIn>>,
    view_settings: Option<Res<ViewModelSettings>>,
    (settings, assets, time): (Res<StandInSettings>, Option<Res<StandIns>>, Res<Time<Real>>),
    mut waiting: Local<HashMap<Entity, f64>>,
    mut commands: Commands,
) {
    let Some(assets) = assets else { return };
    let now = time.elapsed_secs_f64();
    let right = view_settings.is_none_or(|s| s.right_hand != 0);
    let at = Transform::from_translation(VIEW_GUN_AT * Vec3::new(if right { 1.0 } else { -1.0 }, 1.0, 1.0))
        .with_rotation(Quat::from_euler(EulerRot::YXZ, if right { 0.09 } else { -0.09 }, 0.03, 0.0))
        .with_scale(Vec3::splat(VIEW_GUN_SCALE));
    waiting.retain(|c, _| cameras.contains(*c));
    for (camera, wants, children) in &cameras {
        let gun = children.into_iter().flatten().copied().find(|c| shown.contains(*c));
        let want = wants && settings.mode != 0;
        match (want, gun) {
            (false, Some(g)) => {
                commands.entity(g).despawn();
                waiting.remove(&camera);
            }
            (false, None) => {
                waiting.remove(&camera);
            }
            (true, Some(g)) => {
                if let Ok(mut t) = placed.get_mut(g) {
                    t.set_if_neq(at);
                }
            }
            (true, None) => {
                if !waited(&mut waiting, camera, now, settings.grace) {
                    continue;
                }
                commands.spawn((
                    stand_in_bundle(&assets, StandInKind::ViewGun, assets.neutral.clone(), now),
                    ViewStandIn,
                    at,
                    RenderLayers::layer(VIEW_MODEL_LAYER),
                    NotShadowCaster,
                    NotShadowReceiver,
                    ChildOf(camera),
                ));
            }
        }
    }
}

/// The stand-in on a dropped gun.
#[derive(Component)]
pub struct LooseStandIn;

/// A stand-in gun on each dropped gun whose model is still coming
/// (module doc), gone once it is drawn.
#[allow(clippy::type_complexity)]
pub fn loose(
    items: Query<(Entity, &LooseItem, Has<LooseItemDrawn>, Option<&Children>)>,
    shown: Query<(), With<LooseStandIn>>,
    (built, late): (Option<Res<HeldAssets>>, Option<Res<LateHeldModels>>),
    (settings, assets, time): (Res<StandInSettings>, Option<Res<StandIns>>, Res<Time<Real>>),
    mut waiting: Local<HashMap<Entity, f64>>,
    mut commands: Commands,
) {
    let Some(assets) = assets else { return };
    let now = time.elapsed_secs_f64();
    waiting.retain(|e, _| items.contains(*e));
    for (e, item, drawn, children) in &items {
        let gun = children.into_iter().flatten().copied().find(|c| shown.contains(*c));
        // Coming: the kit isn't in, or its held model is added and not built.
        let coming = built.is_none() || late.as_ref().is_some_and(|l| l.has(&item.0));
        let want = settings.mode != 0 && !drawn && coming;
        match (want, gun) {
            (false, Some(g)) => {
                commands.entity(g).despawn();
            }
            (true, None) if waited(&mut waiting, e, now, settings.grace) => {
                commands.spawn((
                    stand_in_bundle(&assets, StandInKind::Gun, assets.neutral.clone(), now),
                    LooseStandIn,
                    // On its side, its middle at the item's.
                    Transform::from_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2))
                        .with_translation(Vec3::new(0.0, 0.0, 0.1)),
                    ChildOf(e),
                ));
            }
            _ => {}
        }
    }
}

/// Asks for a stand-in on this entity while its model is coming: a game
/// or loader puts it on and takes it off when the model is in (module
/// doc). `half`: the half size it stands for (m; a body's height is
/// `2 * half.y`), centred on the entity (a body: its feet `half.y` below).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct WantsStandIn {
    pub kind: StandInKind,
    pub half: Vec3,
}

/// The stand-in a `WantsStandIn` asked for.
#[derive(Component)]
pub struct RequestedStandIn;

/// Draw what `WantsStandIn` asks for; take it away with the request.
#[allow(clippy::type_complexity)]
pub fn requested(
    wants: Query<(Entity, &WantsStandIn, Option<&Team>, Option<&Children>)>,
    shown: Query<(Entity, &ChildOf), With<RequestedStandIn>>,
    (settings, assets, time): (Res<StandInSettings>, Option<Res<StandIns>>, Res<Time<Real>>),
    mut waiting: Local<HashMap<Entity, f64>>,
    mut commands: Commands,
) {
    let Some(assets) = assets else { return };
    let now = time.elapsed_secs_f64();
    for (e, parent) in &shown {
        if settings.mode == 0 || !wants.contains(parent.parent()) {
            commands.entity(e).despawn();
        }
    }
    waiting.retain(|e, _| wants.contains(*e));
    if settings.mode == 0 {
        return;
    }
    for (e, want, team, children) in &wants {
        if children.into_iter().flatten().any(|c| shown.contains(*c)) || !waited(&mut waiting, e, now, settings.grace) {
            continue;
        }
        let (material, placed) = match want.kind {
            StandInKind::Body => (
                assets.for_team(team),
                Transform::from_xyz(0.0, -want.half.y, 0.0)
                    .with_scale(Vec3::new(1.0, 2.0 * want.half.y / stand_in_mesh::STAND_HEIGHT, 1.0)),
            ),
            StandInKind::Prop => (assets.neutral.clone(), Transform::from_scale(want.half * 2.0)),
            StandInKind::Gun | StandInKind::ViewGun => {
                (assets.neutral.clone(), Transform::from_scale(Vec3::splat(want.half.z / 0.385)))
            }
        };
        commands.spawn((stand_in_bundle(&assets, want.kind, material, now), RequestedStandIn, placed, ChildOf(e)));
    }
}

/// `standin_spawn`: a stand-in this far ahead of the local player.
const SPAWN_AHEAD: f32 = 2.5;
/// ... and this far to the left of the view (rad).
const SPAWN_LEFT: f32 = 0.45;

/// The settings, stats, console and systems (from `MapPlugin`, beside
/// the bodies and view models they stand in for).
pub fn plugin(app: &mut App) {
    let test = app.world().contains_resource::<chimera_core::tasks::TestRun>();
    app.insert_resource(StandInSettings { grace: if test { 0.0 } else { GRACE }, ..default() })
        .init_resource::<StandInScroll>()
        .init_resource::<StandInStats>();
    resource_cvar::<StandInSettings, u8>(
        app,
        "chimera_standins",
        "Stand-ins where a model isn't in yet: 0 never, 1 until the model is in, 2 always (to look at them).",
        |s| &mut s.mode,
    );
    app.console_command("stand_ins", "stand_ins: how long stand-ins were on screen, by kind, this session.", |w, _| {
        let stats = w.resource::<StandInStats>();
        let mut out = format!("session: {}; showing now: {}", StandInStats::summary(&stats.session), stats.showing);
        for (i, r) in stats.runs.iter().enumerate().rev().take(5) {
            out.push_str(&format!("\nrun {}: {r}", i + 1));
        }
        Ok(Some(out))
    })
    .console_command(
        "standin_spawn",
        "standin_spawn <body|gun|prop>: a stand-in 2.5 m ahead of you, a little to the left (removed with the map).",
        |w, a| {
            let kind = match a.first().map(String::as_str) {
                Some("body") => StandInKind::Body,
                Some("gun") => StandInKind::Gun,
                Some("prop") => StandInKind::Prop,
                _ => return Err("standin_spawn <body|gun|prop>".into()),
            };
            let mut q = w.query_filtered::<(&GlobalTransform, &Intent, Option<&ColliderAabb>), With<LocalPlayer>>();
            let (at, yaw, feet) = q
                .iter(w)
                .next()
                .map(|(g, i, a)| (g.translation(), i.yaw, a.map_or(-0.9, |a| a.min.y - g.translation().y)))
                .ok_or("no local player")?;
            // A little to the left of the view, so it doesn't hide what is ahead.
            let ahead = Quat::from_rotation_y(yaw + SPAWN_LEFT) * Vec3::NEG_Z * SPAWN_AHEAD;
            let half = match kind {
                StandInKind::Body => Vec3::new(0.3, 0.5 * stand_in_mesh::STAND_HEIGHT, 0.3),
                StandInKind::Prop => Vec3::splat(0.4),
                _ => Vec3::new(0.05, 0.1, 0.385),
            };
            let ground = at.y + feet + half.y;
            w.spawn((
                Name::new("standin_spawn"),
                WantsStandIn { kind, half },
                crate::map::MapPart,
                Transform::from_translation(Vec3::new(at.x + ahead.x, ground, at.z + ahead.z))
                    .with_rotation(Quat::from_rotation_y(yaw + std::f32::consts::PI)),
                Visibility::default(),
            ));
            Ok(None)
        },
    );
    app.add_systems(
        Update,
        (
            make_assets,
            (
                bodies.after(super::bodies::attach_bodies),
                held.after(super::bodies::attach_held),
                loose.after(super::loose::draw_loose),
                view_guns.after(super::view_model::draw_view_models),
                requested,
            ),
            scroll,
        )
            .chain()
            .after(crate::map::DriveAnimation),
    );
}
