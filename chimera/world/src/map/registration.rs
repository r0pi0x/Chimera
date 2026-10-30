//! Map's registration: every system, message and resource of the map, in
//! one fixed order (`build`), with what draws and plays the map
//! (`MapLayers`) registered at fixed places among them. The order is the
//! client's: it decides how Bevy orders systems that aren't ordered
//! explicitly, and resources are entities, so their insert order decides
//! their ids (`heavy::map_spawn_dump` dumps both).
//!
//! `MapSimPlugin` is map's systems alone (`Undrawn`: nothing drawn or
//! played, the map spawned with the `()` drawer): the dedicated server's.
//! `chimera_present`'s `MapPlugin` builds the same with render and audio
//! filling the places (`MapLayers`); a chain that mixes map's systems and
//! drawing ones takes the drawing ones from the layers
//! (`MapLayers::gibs` and its siblings), so it stays one chain.

use std::sync::Arc;

use bevy::ecs::{schedule::ScheduleConfigs, system::ScheduleSystem};

#[allow(unused_imports)]
use super::*;

/// Systems with their configuration, as `MapLayers` hands them over.
pub type LayerSystems = ScheduleConfigs<ScheduleSystem>;

/// Map's systems, messages and resources with nothing drawn or played
/// (the dedicated server's); the map spawned with the `()` drawer.
pub struct MapSimPlugin {
    /// The map to spawn at startup; None starts without one (another map
    /// can be loaded later with `change_map`).
    pub data: Option<Arc<MapData>>,
    pub view: MapDebugView,
}

impl MapSimPlugin {
    pub fn new(data: MapData) -> Self {
        Self {
            data: Some(Arc::new(data)),
            view: MapDebugView::Normal,
        }
    }

    /// The map systems without a map loaded yet.
    pub fn empty() -> Self {
        Self {
            data: None,
            view: MapDebugView::Normal,
        }
    }
}

impl Plugin for MapSimPlugin {
    fn build(&self, app: &mut App) {
        build(app, self.data.as_ref(), self.view, &Undrawn);
    }
}

/// No layers: nothing drawn or played (`MapSimPlugin`).
pub struct Undrawn;

impl MapLayers for Undrawn {
    type Drawer = ();
}

/// Draws nothing: no drawn parts, no headless stand-ins of them, no
/// drawn resources (a dedicated server's map).
impl MapDrawer for () {
    type Drawn = ();

    fn world_material(_: &()) -> bool {
        false
    }

    fn draws(_: &()) -> Draws {
        Draws {
            loose: false,
            gibs: false,
        }
    }

    fn spawn_undrawn(_: &mut (), _: &mut Commands, _: &MapSpawn) {}

    fn spawn_drawn(_: &mut (), _: &mut Commands, _: &MapSpawn) {}

    fn spawn_prop(_: &mut (), _: &mut Commands, _: &MapSpawn, _: &mut (), _: PropSpawn) {}

    fn textures(_: &()) -> &[Handle<Image>] {
        &[]
    }

    fn insert_drawn(_: &(), _: &mut Commands, _: &MapSpawn, _: DrawnResource) {}
}

/// What draws and plays the map, registered at fixed places in map's
/// registration (`build`), each named after what it adds there in the
/// client (`chimera_present`'s `MapPlugin`). Every place is optional: the
/// defaults add nothing (`Undrawn`).
pub trait MapLayers {
    /// What spawns the map's drawn parts (`spawn_map`).
    type Drawer: MapDrawer;
    /// After the ragdolls' simulation: drawing the bodies they move.
    fn ragdolls(&self, _app: &mut App) {}
    /// After the placed ragdolls: the light styles' drawing.
    fn light_styles(&self, _app: &mut App) {}
    /// After the controllers, buoyancy and `MonitorCamera`: the monitor
    /// screens and the audio.
    fn screens_and_sound(&self, _app: &mut App) {}
    /// After `ShowLocalBody`: the culling's resources.
    fn vis_resources(&self, _app: &mut App) {}
    /// After the `PlaceDecal` message: the decals' resources.
    fn decal_resources(&self, _app: &mut App) {}
    /// After `MuzzleCache`: the tracers' resources.
    fn tracer_resources(&self, _app: &mut App) {}
    /// After the `Tracer` message, the last of map's resources: the
    /// lighting's.
    fn lighting(&self, _app: &mut App) {}
    /// After the panes' update: drawing the broken panes.
    fn panes(&self, _app: &mut App) {}
    /// The systems before and after `animate_props` in one `Update` group:
    /// what shows the logic's switches (parts, light styles, prop looks,
    /// beams, ropes).
    fn looks(&self) -> Option<[LayerSystems; 2]> {
        None
    }
    /// The chain of breaking things: drawing the gibs (after
    /// `spawn_gibs`, before `fly_gibs`) and fading them (after
    /// `fly_gibs`).
    fn gibs(&self) -> Option<[LayerSystems; 2]> {
        None
    }
    /// The effects' `Update` group, around `step_particles`.
    fn effects(&self) -> Option<Effects> {
        None
    }
    /// After the loose items' attaching, before the culling chain: the
    /// rest of the drawing (shadows, levels of detail, the sky camera).
    fn drawing(&self, _app: &mut App) {}
    /// The culling chain around `tag_moved_brush_entities` (in
    /// `PostUpdate`, after transform propagation, before visibility's):
    /// the chained systems before it and after it.
    fn culling(&self) -> Option<[LayerSystems; 2]> {
        None
    }
}

/// The effects' `Update` group (`MapLayers::effects`): `before`, then
/// the chain `tracers`, `particles::step_particles`, `emitters`, `draw`,
/// then `after`.
pub struct Effects {
    pub before: LayerSystems,
    pub tracers: LayerSystems,
    pub emitters: LayerSystems,
    pub draw: LayerSystems,
    pub after: LayerSystems,
}

/// Map's registration with `layers` at their places, the map to spawn at
/// startup (`data`; None starts without one) and the spawner `change_map`
/// runs (`MapHooks::spawn`), both with `L::Drawer`.
pub fn build<L: MapLayers>(app: &mut App, data: Option<&Arc<MapData>>, view: MapDebugView, layers: &L) {
    if let Some(data) = data {
        app.insert_resource(PendingMap(data.clone(), view, prepare::Prepared::default()))
            .insert_resource(ActiveMapLook(data.look.clone()));
    }
    ragdoll::plugin(app);
    layers.ragdolls(app);
    placed_ragdoll::plugin(app);
    layers.light_styles(app);
    app.add_plugins((controllers::ControllersPlugin, buoyancy::BuoyancyPlugin));
    app.init_resource::<MonitorCamera>();
    layers.screens_and_sound(app);
    // What the simulation writes for the audio (registered by it first
    // when there is one: then these change nothing).
    app.add_message::<PlaySound>()
        .add_message::<SoundControl>()
        .add_message::<entities::FireEntityOutput>();
    app.init_resource::<ShowLocalBody>();
    layers.vis_resources(app);
    app.add_message::<PlaceDecal>();
    layers.decal_resources(app);
    app.add_message::<ViewModelEvent>()
        .add_message::<SpawnGibs>()
        .add_message::<breakables::BreakProp>()
        .add_message::<prop_physics::PropAwakened>()
        .add_message::<props::PropAnimationDone>()
        .add_message::<GlassShatter>()
        .add_message::<TeslaSpark>()
        .add_message::<BeamStrike>()
        .add_message::<MuzzleFlashFire>()
        .add_message::<ViewKick>()
        .add_message::<GlassImpact>()
        .add_message::<psys::StartParticleSystem>()
        .add_message::<breakables::FallingPane>()
        .add_message::<CollisionStart>()
        .init_resource::<particles::Particles>()
        .configure_sets(
            Update,
            particles::ParticleSet::Step.before(particles::ParticleSet::Draw),
        )
        .init_resource::<tracer::MuzzleCache>();
    layers.tracer_resources(app);
    app.add_message::<tracer::Tracer>();
    layers.lighting(app);
    app.add_systems(
        FixedUpdate,
        (attach_hitboxes, ragdoll::record_poses, pose_hitboxes)
            .chain()
            .before(crate::core::SimSet::Movement),
    )
    .add_systems(FixedUpdate, fall_out_of_map.after(crate::core::SimSet::Movement))
    // Character models a game added late, into the map's (out of the
    // fixed tick: its order is the ratchet's, cs_source schedules.rs).
    .add_systems(PreUpdate, append_late_characters)
    .add_systems(FixedLast, pose_sim_hitboxes.in_set(SimPose))
    // A round restart: gibs, particles and panes reset at once, inside the
    // rules' step (an observer of `core::RoundRestart`).
    .add_observer(breakables::round_restart)
    // A sync point after the rules, where the round restart was polled (an
    // exclusive system) until it became the observer: the rules' commands
    // apply before the map logic and movement, and the single-threaded
    // sort keeps the tick's unordered systems as they were (the schedule
    // ratchets, cs_source and combat_arms `schedules.rs`).
    .add_systems(
        FixedUpdate,
        ApplyDeferred
            .after(crate::core::SimSet::Rules)
            .before(crate::core::SimSet::Movement),
    )
    .add_systems(FixedPostUpdate, breakables::update_panes);
    layers.panes(app);
    app.add_systems(
        FixedPostUpdate,
        prop_physics::wake_on_contact.after(PhysicsSystems::Writeback),
    )
    .add_systems(
        FixedPostUpdate,
        prop_physics::settle_props.after(PhysicsSystems::Writeback),
    )
    .add_systems(
        FixedPostUpdate,
        (prop_physics::apply_stored_inertia, prop_physics::air_drag).before(PhysicsSystems::StepSimulation),
    )
    // The world's surfaces' friction, which physics bodies' own multiplies.
    .insert_resource(DefaultFriction(Friction::new(prop_physics::WORLD_FRICTION)))
    // Moving water follows its brush entity (before movement and
    // buoyancy read it).
    .add_systems(FixedUpdate, water::move_water.before(crate::core::SimSet::Movement));
    match layers.looks() {
        Some([before, after]) => app.add_systems(Update, (before, animate_props, after)),
        None => app.add_systems(Update, animate_props),
    };
    match layers.gibs() {
        Some([draw, fade]) => app.add_systems(
            Update,
            (
                breakables::break_props,
                breakables::fall_panes,
                breakables::spawn_gibs,
                draw,
                breakables::fly_gibs,
                fade,
            )
                .chain(),
        ),
        None => app.add_systems(
            Update,
            (
                breakables::break_props,
                breakables::fall_panes,
                breakables::spawn_gibs,
                breakables::fly_gibs,
            )
                .chain(),
        ),
    };
    app.add_systems(
        FixedPostUpdate,
        breakables::shatter_pane_pieces.after(PhysicsSystems::Writeback),
    )
    // Before the physics step reads it (headless too).
    .add_systems(FixedUpdate, loose::attach_loose.before(crate::core::SimSet::Movement));
    let step = particles::step_particles.in_set(particles::ParticleSet::Step);
    match layers.effects() {
        Some(e) => app.add_systems(
            Update,
            (e.before, (e.tracers, step, e.emitters, e.draw).chain(), e.after),
        ),
        None => app.add_systems(Update, step),
    };
    layers.drawing(app);
    // From this frame's camera, before visibility propagates.
    let culling = match layers.culling() {
        Some([before, after]) => (before, tag_moved_brush_entities, after).chain().into_configs(),
        None => tag_moved_brush_entities.into_configs(),
    };
    app.add_systems(
        PostUpdate,
        culling
            .after(bevy::transform::TransformSystems::Propagate)
            .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
    );
    app.add_systems(Startup, spawn_map::<L::Drawer>.run_if(resource_exists::<PendingMap>));
    app.init_resource::<MapHooks>();
    let mut hooks = app.world_mut().resource_mut::<MapHooks>();
    hooks.spawn = Some(spawn_pending_map::<L::Drawer>);
    hooks.prepare.push(spawn::prepare_world);
    scoped_resources(app);
}

/// The resources `spawn_map` inserts for the loaded map, which
/// `unload_map` removes (`MapScopedApp::map_scoped`). A resource added to
/// the spawn phases goes here; the layers above register theirs where they
/// are built (`MapPlugin`).
fn scoped_resources(app: &mut App) {
    app.map_scoped::<PendingMap>()
        .map_scoped::<MapFile>()
        .map_scoped::<PlayableArea>()
        .map_scoped::<vis::ActiveVisibility>()
        .map_scoped::<sound::SoundBank>()
        .map_scoped::<super::impact::ActiveImpacts>()
        .map_scoped::<CharacterModels>()
        .map_scoped::<hud::ActiveHud>()
        .map_scoped::<hud::ActiveOverview>()
        .map_scoped::<surface_color::SurfaceColors>()
        .map_scoped::<particles::ParticleMaterials>()
        .map_scoped::<ScreenOverlayImages>()
        .map_scoped::<breakables::PropBreaks>()
        .map_scoped::<breakables::GibAssets>()
        .map_scoped::<nav::NavMesh>()
        .map_scoped::<sound::SurfaceGrid>()
        .map_scoped::<sources::SurfaceNames>()
        .map_scoped::<MapBrushes>()
        .map_scoped::<MapBrushTree>()
        .map_scoped::<KillHeight>()
        .map_scoped::<MapWater>()
        .map_scoped::<water::WaterMovers>()
        .map_scoped::<RoundSounds>()
        .map_scoped::<radio::RadioCommands>()
        .map_scoped::<bot_profiles::BotProfiles>()
        .map_scoped::<MapEntities>()
        .map_scoped::<water::WaterView>()
        .map_scoped::<MapTerrain>()
        .map_scoped::<MapTraceSkip>()
        .map_scoped::<MapPropBrushStart>();
}

/// `change_map`'s spawner: the pending map, drawn by `D`.
fn spawn_pending_map<D: MapDrawer>(world: &mut World) {
    if let Err(e) = world.run_system_cached(spawn_map::<D>) {
        error!("spawning the map: {e}");
    }
}
