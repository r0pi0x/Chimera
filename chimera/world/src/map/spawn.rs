//! Spawning a loaded map (`MapData`) into the world: collision always,
//! meshes, materials and effects only when rendering is present.
//!
//! `spawn_map` runs the phases in a fixed order (entity ids, asset ids and
//! archetypes follow it; tests compare them, `heavy::map_spawn_dump`):
//! roots, world collision, entity nodes, headless stand-ins, the drawn
//! parts (images, bodies, lightmaps, sky, water, world meshes, model
//! materials, effects, sprites, beams, ropes, shadows), props, the map's
//! resources, then the sun and spawn points. The drawn parts are spawned
//! by a `MapDrawer` (`render::MapAssets`, which `MapPlugin` picks; `()`
//! draws nothing, `MapSimPlugin`'s), called at their places in that order. `MapDrawer` is a system
//! parameter: `world_material`, `draws`, `spawn_undrawn`, `spawn_drawn`,
//! `spawn_prop`, `textures`, `insert_drawn` (`DrawnResource`).
//!
//! `spawn_map` runs them all at once (a map at startup, a server's change,
//! `change_map`); behind a front end's loading screen they run a few a
//! frame instead (`SpawnProgress`, `spawn_steps`: the drawer's phases in
//! their own steps, the props `PROPS_PER_STEP` at a time), the same order,
//! their commands held until the last step and applied together in a frame
//! of their own, so the world never holds part of a map. The colliders
//! alone follow in the next frames (`Colliders`, `insert_colliders`: each
//! onto its entity, already there, until a frame's budget is spent), as
//! what Avian does on a collider's insert (its mass properties from the
//! shape, twice; its place in the collider tree) was half of that frame;
//! the spawn isn't done, and the simulation waits, until the last is in.
//!
//! What needs only the data (the world's colliders, the models' colliders
//! and hulls, the props' brushes: `WorldParts`) comes prepared ahead when
//! the map was loaded in the background (`prepare`, `prepare_world`), else
//! is computed here by the same functions (the props' brushes on threads).
//! The colliders keep the mass properties Avian's insert hook gave them:
//! the first tick doesn't compute them again (`counted_colliders`).

use bevy::ecs::{
    change_detection::Tick,
    system::{StaticSystemParam, SystemParam, SystemParamItem},
};

#[allow(unused_imports)]
use super::*;

/// What spawns a map's drawn parts, as a system parameter (the asset
/// stores they go into; each may be missing headless, and then what it
/// would draw isn't spawned). `spawn_map` calls it at fixed places.
pub trait MapDrawer: SystemParam + 'static {
    /// What the drawn phases make that the props and the resources use
    /// (render handles; empty headless), and where its steps are.
    type Drawn: Default + Send + Sync + 'static;
    /// Whether the world material exists (for the spawn log).
    fn world_material(p: &SystemParamItem<Self>) -> bool;
    /// Which of the headless stand-ins it draws itself.
    fn draws(p: &SystemParamItem<Self>) -> Draws;
    /// Stand-ins for the drawn parts it can't draw (headless), after the
    /// loose items' and gibs' assets: steam jets still puff, undrawn,
    /// switched by the logic.
    fn spawn_undrawn(p: &mut SystemParamItem<Self>, commands: &mut Commands, s: &MapSpawn);
    /// Everything drawn: images, character assets, lightmaps, sky, water,
    /// world meshes, model materials, detail sprites, effects, sprites,
    /// beams, ropes and shadows, in steps (`step` counts from 0, each
    /// called once, in order): returns whether more steps follow. A
    /// stepped spawn (`SpawnProgress`) may run each in its own frame.
    fn spawn_drawn(
        p: &mut SystemParamItem<Self>,
        commands: &mut Commands,
        s: &MapSpawn,
        drawn: &mut Self::Drawn,
        step: usize,
    ) -> bool;
    /// A placed prop's drawn parts on its node (`prop.id`): its skeleton,
    /// levels of detail, meshes, skins and bodies, and what moves its
    /// bones (a ragdoll or an animation).
    fn spawn_prop(p: &mut SystemParamItem<Self>, commands: &mut Commands, s: &MapSpawn, drawn: &mut Self::Drawn, prop: PropSpawn);
    /// The map textures' images (empty headless).
    fn textures(drawn: &Self::Drawn) -> &[Handle<Image>];
    /// A drawn resource, inserted among the map's own resources at its
    /// place (their order decides their ids).
    fn insert_drawn(drawn: &Self::Drawn, commands: &mut Commands, s: &MapSpawn, which: DrawnResource);
    /// After everything is spawned and `PendingMap` is taken out of the
    /// world: `data` is the map's data, which nothing needs but what the
    /// drawer's resources kept a share of. Dropping it frees the bulk
    /// (the textures' decoded texels, copied into images by now); a
    /// drawer whose resources keep a share (`render`'s shadows) drops the
    /// bulk they don't read instead.
    fn retire(_world: &mut World, _data: Arc<MapData>) {}
}

/// The drawn resources `spawn_map` has a `MapDrawer` insert
/// (`MapDrawer::insert_drawn`), in the order they come.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawnResource {
    /// What runtime decals land on (after the visibility).
    DecalSurfaces,
    /// What draws the particle pool (after `ParticleMaterials`).
    Particles,
    /// Runtime decals' images and materials (after the overview).
    Decals,
    /// The fog shaders take (with the world's other resources, after the
    /// moving water; also headless).
    Fog,
}

/// Which headless stand-ins a `MapDrawer` makes unneeded: dropped
/// weapons' and gibs' assets.
#[derive(Clone, Copy, Debug)]
pub struct Draws {
    /// Dropped weapons (loose items' assets).
    pub loose: bool,
    pub gibs: bool,
}

/// A placed prop for `MapDrawer::spawn_prop`.
pub struct PropSpawn<'a> {
    pub prop: &'a MapProp,
    /// Its node.
    pub id: Entity,
    /// Its fade band, when culled by one (`vis::fade_band`).
    pub fade_range: Option<&'a bevy::camera::visibility::VisibilityRange>,
    /// Where it sits (relative to the mover it rides, if any).
    pub home: Transform,
}

/// How thick terrain surfaces are as solids (`MapTerrain`), meters: 2 units.
pub(super) const TERRAIN_THICKNESS: f32 = 2.0 * 0.0254;

/// How far below the map's lowest point (meters) falling ends.
pub(super) const KILL_MARGIN: f32 = 3.0;

/// What the phases share: the map, the roots its parts go under, the
/// brush entities' and anchors' nodes.
pub struct MapSpawn<'a> {
    pub data: &'a Arc<MapData>,
    pub view: MapDebugView,
    /// The map's root.
    pub root: Entity,
    /// The root of drawn parts that never move.
    pub statics: Entity,
    /// Each map entity's node (movers, anchors), by entity index.
    pub entity_nodes: Vec<Option<Entity>>,
    /// The visibility drawn parts are culled by (none when the world is
    /// spawned merged, `merged_world`).
    pub visibility: Option<&'a vis::MapVisibility>,
    /// Parts computed ahead of the spawn (`prepare`): each phase takes its
    /// own, else computes it.
    pub prepared: &'a prepare::Prepared,
}

impl MapSpawn<'_> {
    /// Map entity `i`'s node, when it has one.
    pub fn node(&self, i: usize) -> Option<Entity> {
        self.entity_nodes.get(i).copied().flatten()
    }

    /// The node a mesh goes under: its entity's, else the map's root.
    pub fn parent_of(&self, m: &MapMesh) -> Entity {
        m.entity.and_then(|i| self.node(i)).unwrap_or(self.root)
    }

    /// Where the map places each mover node (merged brush entities draw
    /// there, `merge`).
    pub fn node_home(&self, i: usize) -> Transform {
        let e = &self.data.entities[i];
        Transform::from_translation(entities::entity_to_engine(e.origin(), self.data.entity_scale))
            .with_rotation(entities::rotation_to_engine(entities::entity_rotation(e.angles())))
    }

    /// Tags a drawn part with the clusters it touches, to be culled by
    /// them (`vis`).
    pub fn tag(&self, e: &mut EntityCommands, clusters: Vec<u32>) {
        if self.visibility.is_some() && !clusters.is_empty() {
            e.insert(VisClusters::new(clusters));
        }
    }
}

/// Spawns the pending map, its drawn parts by `D`, all at once.
pub fn spawn_map<D: MapDrawer>(mut commands: Commands, pending: Res<PendingMap>, mut assets: StaticSystemParam<D>) {
    // Colliders with their entities, at once (`Colliders`).
    let mut progress = SpawnProgress::<D>::default();
    while !advance(&mut commands, &pending, &mut *assets, &mut progress, None) {}
}

/// A map's spawn under way a step at a time (`spawn_steps`; behind a front
/// end's loading screen, docs/performance.md, "A map's swap"): the same
/// phases in the same order as `spawn_map`, their commands kept here and
/// applied together after the last step, so the world sees the whole map
/// at once (and never part of one); only the computing and the asset
/// stores' additions are spread over the frames.
#[derive(Resource)]
pub struct SpawnProgress<D: MapDrawer> {
    queue: bevy::ecs::world::CommandQueue,
    stage: Stage,
    root: Entity,
    statics: Entity,
    entity_nodes: Vec<Option<Entity>>,
    props: Option<PropParts>,
    brushes: Vec<MapBrush>,
    drawn: D::Drawn,
    drawn_step: usize,
    next_prop: usize,
    steps: usize,
    /// Every step is done: the next frame applies the commands, alone.
    built: bool,
    /// The commands are applied: the map is in the world but for the
    /// colliders held back (`insert_colliders`).
    applied: bool,
    /// The colliders, held back by a stepped spawn.
    colliders: Colliders,
    /// The world's change tick when the spawn's commands began to apply
    /// (`counted_colliders`).
    since: std::sync::Arc<std::sync::atomic::AtomicU32>,
    _drawer: std::marker::PhantomData<fn() -> D>,
}

impl<D: MapDrawer> Default for SpawnProgress<D> {
    fn default() -> Self {
        Self {
            queue: default(),
            stage: Stage::Start,
            root: Entity::PLACEHOLDER,
            statics: Entity::PLACEHOLDER,
            entity_nodes: Vec::new(),
            props: None,
            brushes: Vec::new(),
            drawn: default(),
            drawn_step: 0,
            next_prop: 0,
            steps: 0,
            built: false,
            applied: false,
            colliders: default(),
            since: default(),
            _drawer: default(),
        }
    }
}

/// Where the spawn's colliders go. A spawn at once inserts each with its
/// entity's other parts. A stepped spawn keeps them (`later`) and inserts
/// them in the frames after the one its commands are applied in
/// (`insert_colliders`): on a collider's insert Avian computes its mass
/// properties from its shape twice (its insert hook, and the collider
/// tree's observer for its bounds' margin, through the centre of mass; a
/// compound of a map's thousands of hulls, its surfaces' triangles, each
/// prop's convex pieces) and puts it in the collider tree, which was 40-75%
/// of the frame the whole map went into the world (docs/performance.md, "A
/// map's colliders after its swap frame"). Every
/// collider of the spawn goes through `put`, after its entity's other
/// parts in both cases, so the entity is the same either way.
#[derive(Default)]
pub(super) struct Colliders {
    later: Option<Vec<(Entity, Collider)>>,
    /// How many of `later` are in the world.
    next: usize,
}

impl Colliders {
    /// `collider` onto `e`: now (queued with its commands), or held back.
    pub(super) fn put(&mut self, e: &mut EntityCommands, collider: Collider) {
        match &mut self.later {
            Some(later) => later.push((e.id(), collider)),
            None => {
                e.insert(collider);
            }
        }
    }
}

/// Colliders a frame inserts at most (`insert_colliders`), whatever its
/// budget: a prop's box is cheap, so a count alone would take a frame per
/// few costly ones; a budget alone would end where the machine's load
/// says.
const COLLIDERS_PER_FRAME: usize = 256;

/// The share of a frame's spawn budget its colliders get: 1 in this many.
/// A step's work is the frame's only cost of that size; a collider's
/// insert comes with what the world does about the map that just went
/// in (its props' first shadows, its bounds), and one may be far over any
/// budget (a map's solids).
const COLLIDER_BUDGET_SHARE: u32 = 2;

/// Where a spawn is (`SpawnProgress`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    /// The roots, the world's collision, the entities' nodes and the
    /// headless stand-ins.
    Start,
    /// The drawer's steps (`MapDrawer::spawn_drawn`).
    Drawn,
    /// The placed props, from `next_prop`.
    Props,
    /// The map's resources, the sun and the spawn points.
    Finish,
}

/// Props a step spawns at most (each is a few entities and, drawn, its
/// meshes' and materials' assets).
const PROPS_PER_STEP: usize = 16;

/// Runs the spawn's next steps: all of them without a `deadline`, else at
/// least one and then more until the deadline passes. Returns whether the
/// spawn is done (its last commands queued).
fn advance<D: MapDrawer>(
    commands: &mut Commands,
    pending: &PendingMap,
    assets: &mut SystemParamItem<D>,
    progress: &mut SpawnProgress<D>,
    deadline: Option<std::time::Instant>,
) -> bool {
    let data = &pending.0;
    let (view, prepared) = (pending.1, &pending.2);
    loop {
        progress.steps += 1;
        if progress.stage == Stage::Start {
            // First: where the spawn's ticks begin (`counted_colliders`).
            let since = progress.since.clone();
            commands.queue(move |world: &mut World| {
                since.store(world.change_tick().get(), std::sync::atomic::Ordering::Relaxed)
            });
            let world = prepared.take::<WorldParts>().unwrap_or_else(|| world_parts(data));
            let (root, statics) = spawn_roots(commands, data, D::world_material(assets));
            let colliders = &mut progress.colliders;
            spawn_world_collision(commands, colliders, data, world.collision, root);
            super::physics_clip::spawn_clips(commands, colliders, data, root);
            progress.entity_nodes = spawn_entity_nodes(commands, colliders, data, root);
            (progress.root, progress.statics, progress.props) = (root, statics, Some(world.props));
        }
        let s = MapSpawn {
            data,
            view,
            root: progress.root,
            statics: progress.statics,
            entity_nodes: std::mem::take(&mut progress.entity_nodes),
            visibility: data.visibility.as_deref().filter(|_| !merged_world()),
            prepared,
        };
        match progress.stage {
            Stage::Start => {
                insert_entities(commands, &s);
                insert_headless_assets(commands, &s, D::draws(assets));
                D::spawn_undrawn(assets, commands, &s);
                progress.stage = Stage::Drawn;
            }
            Stage::Drawn => {
                let more = D::spawn_drawn(assets, commands, &s, &mut progress.drawn, progress.drawn_step);
                progress.drawn_step += 1;
                if !more {
                    progress.brushes = start_props(data, progress.props.as_ref());
                    progress.stage = Stage::Props;
                }
            }
            Stage::Props => {
                let parts = progress.props.as_mut().expect("the props' parts, from the start");
                let end = (progress.next_prop + PROPS_PER_STEP).min(data.props.len());
                for i in progress.next_prop..end {
                    let placed = std::mem::take(&mut parts.brushes[i]);
                    spawn_prop::<D>(
                        commands,
                        &mut progress.colliders,
                        &s,
                        assets,
                        &mut progress.drawn,
                        &mut progress.brushes,
                        i,
                        &data.props[i],
                        &parts.model_colliders,
                        placed,
                    );
                }
                progress.next_prop = end;
                if end == data.props.len() {
                    progress.stage = Stage::Finish;
                }
            }
            Stage::Finish => {
                insert_resources::<D>(commands, &s, &progress.drawn, std::mem::take(&mut progress.brushes));
                spawn_sun_and_spawn_points(commands, data);
                // Last: the pending data goes (nothing reads it after the
                // spawn), and with it the textures' texels, unless something
                // kept a share.
                commands.queue(retire_pending::<D>);
                // Held-back colliders are counted once they are in
                // (`insert_colliders`).
                if progress.colliders.later.is_none() {
                    let since = progress.since.clone();
                    commands.queue(move |world: &mut World| {
                        counted_colliders(world, Tick::new(since.load(std::sync::atomic::Ordering::Relaxed)))
                    });
                }
                return true;
            }
        }
        progress.entity_nodes = s.entity_nodes;
        if deadline.is_some_and(|d| std::time::Instant::now() >= d) {
            return false;
        }
    }
}

/// One frame's share of the pending map's spawn (`SpawnProgress`, which
/// `begin_spawn` inserted): steps until `budget` is spent (at least one).
/// Once every step is done its commands, held since the first step, go to
/// the world together at the end of this system, in a frame of their own;
/// the colliders follow (`insert_colliders`, which says when the map is
/// spawned), so this returns false.
pub(super) fn spawn_steps<D: MapDrawer>(
    In(budget): In<std::time::Duration>,
    mut commands: Commands,
    pending: Res<PendingMap>,
    mut assets: StaticSystemParam<D>,
    mut progress: ResMut<SpawnProgress<D>>,
) -> bool {
    let progress = &mut *progress;
    if progress.built {
        // A frame of its own: what the world does with a whole new map
        // (the colliders' hooks, the hierarchy, a frame's first look at
        // thousands of entities) is the most of any step.
        info!(
            "map spawned in {} steps, {} colliders to follow",
            progress.steps,
            progress.colliders.later.as_ref().map_or(0, Vec::len)
        );
        commands.append(&mut progress.queue);
        progress.applied = true;
        return false;
    }
    // A budget past any instant: every step now.
    let deadline = std::time::Instant::now().checked_add(budget);
    let mut queue = std::mem::take(&mut progress.queue);
    progress.built = advance(&mut commands.rebound_to(&mut queue), &pending, &mut *assets, progress, deadline);
    progress.queue = queue;
    false
}

/// Starts a stepped spawn of the pending map (`spawn_steps`), its
/// colliders held back (`Colliders`).
pub(super) fn begin_spawn<D: MapDrawer>(world: &mut World) {
    world.insert_resource(SpawnProgress::<D> {
        colliders: Colliders {
            later: Some(Vec::new()),
            next: 0,
        },
        ..default()
    });
}

/// Whether a stepped spawn's commands are applied: what is left of it is
/// its colliders (`insert_colliders`).
pub(super) fn colliders_left<D: MapDrawer>(world: &World) -> bool {
    world.get_resource::<SpawnProgress<D>>().is_some_and(|p| p.applied)
}

/// One frame's share of a stepped spawn's colliders, after its commands
/// were applied: each onto its entity, in the order the spawn made them,
/// until `budget` is spent (at least one). Returns whether the map is
/// spawned: every collider in, each keeping the mass properties its insert
/// gave it (`counted_colliders`). The simulation waits until then (the
/// caller's: `client::console`), so nothing moves through a map whose
/// colliders are partly in.
pub(super) fn insert_colliders<D: MapDrawer>(world: &mut World, budget: std::time::Duration) -> bool {
    let Some(mut progress) = world.remove_resource::<SpawnProgress<D>>() else { return true };
    let deadline = std::time::Instant::now().checked_add(budget / COLLIDER_BUDGET_SHARE);
    let colliders = &mut progress.colliders;
    let later = colliders.later.as_deref().unwrap_or_default();
    let end = colliders.next + COLLIDERS_PER_FRAME;
    while let Some((entity, collider)) = later.get(colliders.next).filter(|_| colliders.next < end) {
        colliders.next += 1;
        // Its shape is shared: the list's copy goes with the list.
        if let Ok(mut e) = world.get_entity_mut(*entity) {
            e.insert(collider.clone());
        }
        if deadline.is_some_and(|d| std::time::Instant::now() >= d) {
            break;
        }
    }
    if colliders.next < later.len() {
        world.insert_resource(progress);
        return false;
    }
    info!("map's {} colliders in", later.len());
    counted_colliders(world, Tick::new(progress.since.load(std::sync::atomic::Ordering::Relaxed)));
    true
}

/// The spawn's colliders have their mass properties already: Avian's
/// insert hook computed them from the shape as inserted, and its
/// `update_collider_mass_properties` would compute them all again in the
/// first tick, the collider and its `ColliderDensity` being new there
/// (`Changed`): 55-75 M instructions of a map's first tick (dev build;
/// docs/performance.md, "A map's swap"). Avian 0.8 has no way to skip
/// either. So each collider the spawn's commands inserted (from `since`,
/// the tick they began to apply at), at its shape's own scale (no
/// `GlobalTransform` given, so the hook's is one too) and with nothing
/// changed after them, gets both change ticks put back to the oldest a
/// tick can be. Avian's other readers of a changed collider get the same
/// from it again (a body's AABB margin, made on its add from this shape)
/// or see its new `Transform`, `ColliderTransform` or `Position` anyway
/// (its scale, a body's size); a later change (a scale, a density, a shape)
/// marks it anew and is computed as before. Nothing in the spawn sets a
/// collider's shape after inserting it.
fn counted_colliders(world: &mut World, since: Tick) {
    use bevy::ecs::change_detection::MAX_CHANGE_AGE;
    let now = world.change_tick();
    let before = Tick::new(since.get().wrapping_sub(1));
    let oldest = Tick::new(now.get().wrapping_sub(MAX_CHANGE_AGE));
    // Inserted and last changed by these commands: the add hook runs
    // systems, so the insert hook's change comes a few ticks after the add.
    let fresh = |added: Tick, changed: Tick| added.is_newer_than(before, now) && changed.is_newer_than(before, now);
    let mut q = world.query::<(Mut<Collider>, Mut<ColliderDensity>)>();
    for (mut collider, mut density) in q.iter_mut(world) {
        if collider.scale() == Vec3::ONE
            && fresh(collider.added(), collider.last_changed())
            && fresh(density.added(), density.last_changed())
        {
            collider.set_last_changed(oldest);
            density.set_last_changed(oldest);
        }
    }
}

/// Takes `PendingMap` out of the world once the map is spawned
/// (`MapDrawer::retire`).
fn retire_pending<D: MapDrawer>(world: &mut World) {
    if let Some(pending) = world.remove_resource::<PendingMap>() {
        D::retire(world, pending.0);
    }
}

/// The map's file, and its two roots: the map's, and one for drawn parts
/// that never move.
fn spawn_roots(commands: &mut Commands, data: &MapData, world_material: bool) -> (Entity, Entity) {
    commands.insert_resource(MapFile {
        name: data.name.clone(),
        hash: data.file_hash,
    });
    info!(
        "spawning map: world material {}, lightmap {}",
        if world_material { "available" } else { "missing" },
        data.lightmap.is_some()
    );
    let root = commands
        .spawn((
            Name::new(format!("Map {}", data.name)),
            MapPart,
            Transform::default(),
            Visibility::default(),
        ))
        .id();
    // Drawn parts that never move (world chunks, static props, merged brush
    // entities, ropes, prop shadows) under a root of their own, beside the
    // map's (both at the origin): any transform written under a root (a
    // rotating brush, a physics prop) makes Bevy's transform propagation
    // recompute every child of that root, several times a frame
    // (surf_demise: 37k map parts, 9.6 ms a frame); a root whose tree
    // didn't change is skipped whole.
    let statics = commands
        .spawn((
            Name::new("Map static parts"),
            MapPart,
            Transform::default(),
            Visibility::default(),
        ))
        .id();
    (root, statics)
}

/// What the world's spawn computes from the data alone (`prepare`): the
/// collision's colliders and the props' colliders and brushes.
pub(super) struct WorldParts {
    collision: CollisionParts,
    props: PropParts,
}

/// The world's job for `MapHooks::prepare`: its parts, the surfaces'
/// colours (`SurfaceColors`) and the light field's first use.
pub(super) fn prepare_world(_: &World) -> prepare::PrepareJob {
    Box::new(|data, prepared| {
        prepared.insert(world_parts(data));
        prepared.insert(surface_color::SurfaceColors::new(data));
        // A light field may build what it needs on its first use (Source's:
        // the occluding brushes' hulls): here, not in the first tick.
        if let Some(field) = &data.light_field {
            (field.0)(Vec3::ZERO);
        }
    })
}

fn world_parts(data: &MapData) -> WorldParts {
    WorldParts {
        collision: collision_parts(data),
        props: prop_parts(data),
    }
}

/// The world's collision as colliders: the surfaces (with the terrain
/// queries' copy and a smoothed collider for loose items), each set's
/// triangles, the solids and the glass.
struct CollisionParts {
    terrain: Option<(MapTerrain, Collider, Option<Collider>)>,
    /// Per collision set, its collider (None: not spawned).
    sets: Vec<Option<Collider>>,
    /// The solids, then the glass and grates (None: no hulls).
    solids: [Option<Collider>; 2],
}

fn collision_parts(data: &MapData) -> CollisionParts {
    let terrain = (!data.collision_indices.is_empty()).then(|| {
        let terrain = MapTerrain::from_triangles(
            data.collision_indices
                .iter()
                .map(|t| t.map(|i| Vec3::from(data.collision_positions[i as usize]))),
            TERRAIN_THICKNESS,
        );
        let positions: Vec<Vec3> = data.collision_positions.iter().map(|p| Vec3::from(*p)).collect();
        let surfaces = Collider::trimesh(positions.clone(), data.collision_indices.clone());
        (terrain, surfaces, smooth_terrain(positions, &data.collision_indices))
    });
    let sets = data
        .collision_sets
        .iter()
        .map(|set| {
            (!set.indices.is_empty() && (set.characters || set.traces)).then(|| {
                let positions: Vec<Vec3> = data.collision_positions.iter().map(|p| Vec3::from(*p)).collect();
                Collider::trimesh(positions, set.indices.clone())
            })
        })
        .collect();
    let see_through: std::collections::HashSet<usize> = data.see_through_hulls.iter().copied().collect();
    let solids = [false, true].map(|glass| {
        let hulls: Vec<_> = data
            .collision_hulls
            .iter()
            .enumerate()
            .filter(|(i, _)| see_through.contains(i) == glass)
            .flat_map(|(_, h)| contact_hulls(&h.iter().map(|p| Vec3::from(*p)).collect::<Vec<_>>()))
            .map(|c| (Vec3::ZERO, Quat::IDENTITY, c))
            .collect();
        (!hulls.is_empty()).then(|| Collider::compound(hulls))
    });
    CollisionParts { terrain, sets, solids }
}

/// The world's collision: its surfaces (and a smoothed copy for loose
/// items) and its solids.
fn spawn_world_collision(
    commands: &mut Commands,
    colliders: &mut Colliders,
    data: &MapData,
    parts: CollisionParts,
    root: Entity,
) {
    if let Some((terrain, surfaces, smooth)) = parts.terrain {
        commands.insert_resource(terrain);
        let mut e = commands.spawn((
            Name::new("Map collision (surfaces)"),
            MapPart,
            MapTerrainCollider,
            super::controllers::WorldCollider,
            RigidBody::Static,
            // Loose items slide on their own copy (below).
            CollisionLayers::new(LayerMask::DEFAULT, LayerMask::ALL & !crate::core::ITEM_LAYER),
            Transform::default(),
            ChildOf(root),
        ));
        colliders.put(&mut e, surfaces);
        if let Some(smooth) = smooth {
            let mut e = commands.spawn((
                Name::new("Map collision (surfaces, for loose items)"),
                MapPart,
                MapTerrainCollider,
                super::controllers::WorldCollider,
                RigidBody::Static,
                CollisionLayers::new(crate::core::ITEM_GROUND_LAYER, crate::core::ITEM_LAYER),
                Transform::default(),
                ChildOf(root),
            ));
            colliders.put(&mut e, smooth);
        }
    }
    // Triangles only some things meet (`MapCollisionSet`): no contacts
    // but with characters for the players-only ones.
    for (set, collider) in data.collision_sets.iter().zip(parts.sets) {
        let Some(collider) = collider else { continue };
        let layers = match (set.characters, set.traces) {
            (true, false) => CollisionLayers::new(crate::core::PLAYER_CLIP_LAYER, LayerMask::DEFAULT),
            (false, true) => CollisionLayers::new(crate::core::TRACE_ONLY_LAYER, LayerMask::NONE),
            _ => CollisionLayers::new(LayerMask::DEFAULT, LayerMask::ALL & !crate::core::ITEM_LAYER),
        };
        let mut e = commands.spawn((
            Name::new(format!("Map collision ({})", set.name)),
            MapPart,
            super::controllers::WorldCollider,
            RigidBody::Static,
            layers,
            Transform::default(),
            ChildOf(root),
        ));
        colliders.put(&mut e, collider);
        if set.see_through {
            e.insert(SeeThrough);
        }
        if let Some(s) = &set.surface {
            e.insert(PropSurface(s.clone()));
        }
    }
    // Glass and grates get a collider of their own, marked `SeeThrough`.
    for (glass, collider) in [false, true].into_iter().zip(parts.solids) {
        let Some(collider) = collider else { continue };
        let mut e = commands.spawn((
            Name::new(if glass {
                "Map collision (glass and grates)"
            } else {
                "Map collision (solids)"
            }),
            MapPart,
            MapBrushCollider,
            super::controllers::WorldCollider,
            RigidBody::Static,
            Transform::default(),
            ChildOf(root),
        ));
        colliders.put(&mut e, collider);
        if glass {
            e.insert(SeeThrough);
        }
    }
}

/// Mover brush entities: a node each (placed by origin and angles), with
/// a kinematic collider; their meshes are spawned under it later.
/// Anchors (placed weapons with entities parented to them) get a node
/// too, which their props ride and the weapon layer moves.
fn spawn_entity_nodes(
    commands: &mut Commands,
    colliders: &mut Colliders,
    data: &MapData,
    root: Entity,
) -> Vec<Option<Entity>> {
    let anchors = entities::anchor_entities(&data.entities);
    data.entities
        .iter()
        .enumerate()
        .map(|(i, e)| {
            if anchors[i] && !e.mover {
                let home = Transform::from_translation(entities::entity_to_engine(e.origin(), data.entity_scale))
                    .with_rotation(entities::rotation_to_engine(entities::entity_rotation(e.angles())));
                return Some(
                    commands
                        .spawn((
                            Name::new(format!("Anchor {i} ({})", e.classname())),
                            MapPart,
                            entities::MapAnchor(i),
                            home,
                            Visibility::default(),
                            interp::Interpolated::default(),
                            ChildOf(root),
                        ))
                        .id(),
                );
            }
            if !e.mover {
                return None;
            }
            let scale = data.entity_scale;
            let rotation = entities::entity_rotation(e.angles());
            let collider = entities::brush_collider(e, scale);
            let home = Transform::from_translation(entities::entity_to_engine(e.origin(), scale))
                .with_rotation(entities::rotation_to_engine(rotation));
            let mut node = commands.spawn((
                Name::new(format!("Brush entity {i} ({})", e.classname())),
                MapPart,
                MapBrushEntity(i),
                home,
                Visibility::default(),
                ChildOf(root),
            ));
            if let Some(s) = entities::brush_surface(&data.meshes, i) {
                node.insert(entities::BrushSurface(s));
            }
            match (collider, &e.physics) {
                // A physics brush (func_physbox): a rigid body like a
                // physics prop, pinned while its motion is disabled.
                (Some(collider), Some(p)) => {
                    let points = e
                        .hulls
                        .iter()
                        .flat_map(|h| &h.points)
                        .map(|q| entities::entity_to_engine(*q, scale));
                    let bounds = points.fold((Vec3::MAX, Vec3::MIN), |(a, b), q| (a.min(q), b.max(q)));
                    node.insert((PropEntity(i), PropHome(home)));
                    colliders.put(&mut node, collider);
                    if p.frozen {
                        node.insert((
                            RigidBody::Static,
                            MapPropCollider,
                            prop_physics::FrozenBody {
                                physics: p.clone(),
                                bounds,
                            },
                        ));
                    } else {
                        node.insert(prop_physics::dynamic_body(p, bounds));
                        if p.asleep {
                            node.insert((prop_physics::StartAsleep, RigidBody::Static));
                        } else {
                            node.insert(prop_physics::WedgeCheck::default());
                        }
                    }
                }
                (Some(collider), None) => {
                    node.insert((MapBrushCollider, RigidBody::Kinematic, contact_filter::hooks()));
                    colliders.put(&mut node, collider);
                    if data.see_through_entities.contains(&i) {
                        node.insert(SeeThrough);
                    }
                }
                _ => {}
            }
            Some(node.id())
        })
        .collect()
}

/// Areaportal windows' brushes, the map's entities and what breaks.
fn insert_entities(commands: &mut Commands, s: &MapSpawn) {
    let data = s.data;
    // Areaportal windows' brushes fade in with distance (`render::vis::fade_windows`).
    if let Some(v) = data.visibility.as_deref() {
        for (portal, p) in v.areas.portals.iter().enumerate() {
            let brush = p.fade.and_then(|f| f.brush);
            if let Some(node) = brush.and_then(|b| s.node(b)) {
                commands.entity(node).insert(vis::WindowBrush { portal, alpha: None });
            }
        }
    }
    if !data.entities.is_empty() {
        commands.insert_resource(MapEntities {
            entities: Arc::new(data.entities.clone()),
            scale: data.entity_scale,
        });
    }
    commands.insert_resource(breakables::prop_breaks(data));
}

/// Headless: dropped weapons and gibs still have their bodies, undrawn.
fn insert_headless_assets(commands: &mut Commands, s: &MapSpawn, draws: Draws) {
    let data = s.data;
    if !draws.loose
        && let Some(loose) = loose::assets(data)
    {
        commands.insert_resource(loose);
    }
    if !draws.gibs
        && let Some(gibs) = breakables::build_assets(data)
    {
        commands.insert_resource(gibs);
    }
}

/// What the props' spawn computes from the data alone (`prepare`): each
/// model's collider and hull, and each prop's brushes for movement.
struct PropParts {
    model_colliders: Vec<Option<Collider>>,
    model_hulls: Vec<Option<Vec<(Vec3, f32)>>>,
    /// Per prop: whether its pieces are brushes for movement, and them
    /// placed in the world.
    brushes: Vec<(bool, Vec<MapBrush>)>,
}

fn prop_parts(data: &MapData) -> PropParts {
    // Prop models: one collider per model (shared by its placements).
    let model_colliders: Vec<Option<Collider>> = super::parallel::par_map(data.models.iter().collect(), model_collider);
    // Comparison hook: CHIMERA_NO_PROP_BRUSHES=1 sweeps props as meshes.
    let no_prop_brushes = chimera_core::env::var("CHIMERA_NO_PROP_BRUSHES").is_ok_and(|v| v == "1");
    // Models without a collision model: their hull, when (nearly) convex.
    let model_hulls: Vec<Option<Vec<(Vec3, f32)>>> = data
        .models
        .iter()
        .map(|m| {
            m.collision
                .is_none()
                .then(|| convex_planes(m, CONVEX_SURFACE_SHARE))
                .flatten()
        })
        .collect();
    // A piece with many planes takes long to bevel (`MapBrush::from_planes`,
    // de_nuke's tens of seconds of props one after another): on threads.
    let brushes = super::parallel::par_map(data.props.iter().collect(), |prop| {
        prop_brushes(data, prop, &model_hulls, no_prop_brushes)
    });
    PropParts {
        model_colliders,
        model_hulls,
        brushes,
    }
}

/// A prop's convex pieces as brushes for movement, placed in the world,
/// and whether it has them (`prop_brush`): the model's collision model
/// (each piece exactly), else its hull when (nearly) convex, else the
/// bounds box; none for bodies that move (swept through physics queries;
/// frozen ones stay put like static props until enabled,
/// `prop_physics::FrozenBody`), riders, the 3D skybox's props.
fn prop_brushes(
    data: &MapData,
    prop: &MapProp,
    model_hulls: &[Option<Vec<(Vec3, f32)>>],
    no_prop_brushes: bool,
) -> (bool, Vec<MapBrush>) {
    let model = &data.models[prop.model];
    let pieces: Vec<Vec<(Vec3, f32)>> = match (prop.solid, &model.collision) {
        (PropSolid::Mesh, Some(c)) => c.pieces.iter().map(|p| p.planes.clone()).collect(),
        (PropSolid::Mesh, None) => model_hulls[prop.model].iter().cloned().collect(),
        (PropSolid::Box, _) => {
            let (lo, hi) = model.bounds;
            ((hi - lo).min_element() > 0.0)
                .then(|| MapBrush::from_box(lo, hi).planes)
                .into_iter()
                .collect()
        }
        (PropSolid::WorldBox, _) => {
            let (lo, hi) = world_box(model.bounds, prop.rotation);
            ((hi - lo).min_element() > 0.0)
                .then(|| MapBrush::from_box(lo, hi).planes)
                .into_iter()
                .collect()
        }
        (PropSolid::None, _) => Vec::new(),
    };
    // A world box's planes are already along the world's axes.
    let rotation = if prop.solid == PropSolid::WorldBox {
        Quat::IDENTITY
    } else {
        prop.rotation
    };
    let dynamic = prop.physics.as_ref().filter(|p| !prop.skybox && !p.frozen);
    let prop_brush =
        prop.parent.is_none() && !prop.skybox && !no_prop_brushes && !pieces.is_empty() && dynamic.is_none();
    let placed = if prop_brush {
        pieces
            .iter()
            .map(|planes| place_brush(planes, prop.translation, rotation, model.surfaceprop.clone()))
            .collect()
    } else {
        Vec::new()
    };
    (prop_brush, placed)
}

/// Before the placed props (`spawn_prop`, a node each with its collision,
/// culling, meshes, skeleton and what the logic may switch): the map's
/// brushes for movement they add to (the world's; each prop that is
/// (close to) convex adds its own).
fn start_props(data: &MapData, parts: Option<&PropParts>) -> Vec<MapBrush> {
    // Exact brushes for movement that sweeps them (Source): the world's,
    // plus props that are (close to) convex, in world space.
    let brushes = data.collision_brushes.clone();
    info!(
        "props: {} of {} models collide by their collision model, {} more as convex hulls; {} have levels of detail",
        data.models.iter().filter(|m| m.collision.is_some()).count(),
        data.models.len(),
        parts.map_or(0, |p| p.model_hulls.iter().filter(|h| h.is_some()).count()),
        data.models.iter().filter(|m| !m.lods.is_empty()).count(),
    );
    brushes
}

/// One placed prop (`placed`: its brushes, `prop_brushes`).
#[allow(clippy::too_many_arguments)]
fn spawn_prop<D: MapDrawer>(
    commands: &mut Commands,
    colliders: &mut Colliders,
    s: &MapSpawn,
    a: &mut SystemParamItem<D>,
    drawn: &mut D::Drawn,
    brushes: &mut Vec<MapBrush>,
    i: usize,
    prop: &MapProp,
    model_colliders: &[Option<Collider>],
    (prop_brush, placed_brushes): (bool, Vec<MapBrush>),
) {
    let (data, root, statics) = (s.data, s.root, s.statics);
    let model = &data.models[prop.model];
    // Bodies that move are swept through physics queries, not as brushes.
    let dynamic = prop.physics.as_ref().filter(|p| !prop.skybox && !p.frozen);
    // A prop entity's brushes go on its node (`MovingSolid`, still), so
    // they stop blocking once the logic removes it; others join the map's.
    let own_solid = (prop.entity.is_some() && !placed_brushes.is_empty()).then(|| MovingSolid {
        brushes: placed_brushes.clone(),
        velocity: Vec3::ZERO,
        solid: true,
    });
    if own_solid.is_none() {
        brushes.extend(placed_brushes);
    }
    let placed = Transform::from_translation(prop.translation).with_rotation(prop.rotation);
    // Riding a mover: under its node, placed relative to it.
    let rider = prop
        .parent
        .and_then(|p| Some((s.node(p)?, data.entities.get(p)?)))
        .map(|(node, ent)| {
            let at = Transform::from_translation(entities::entity_to_engine(ent.origin(), data.entity_scale))
                .with_rotation(entities::rotation_to_engine(entities::entity_rotation(ent.angles())));
            (
                node,
                Transform::from_matrix(at.to_matrix().inverse() * placed.to_matrix()),
            )
        });
    let mut e = commands.spawn((
        Name::new(format!("Prop {i}")),
        PropIndex(i),
        MapPart,
        rider.map_or(placed, |r| r.1),
        Visibility::default(),
        // Static props (no entity, no physics) never move.
        ChildOf(rider.map_or(
            if prop.entity.is_none() && prop.physics.is_none() {
                statics
            } else {
                root
            },
            |r| r.0,
        )),
    ));
    if let Some(solid) = own_solid {
        e.insert(solid);
    }
    let stays_put = dynamic.is_none() && rider.is_none() && model.rig.is_none();
    let fade_range = tag_prop(&mut e, data, prop, model, stays_put);
    if let Some(index) = prop.entity {
        e.insert((PropEntity(index), PropHome(rider.map_or(placed, |r| r.1))));
        // func_fish_pool's fish swim about its origin (`fish`).
        if let Some(pool) = data
            .entities
            .get(index)
            .filter(|p| p.classname().eq_ignore_ascii_case("func_fish_pool"))
        {
            let range = pool
                .get("max_range")
                .map_or(150.0, |v| v.trim().parse().unwrap_or(150.0));
            e.insert(fish::Fish::new(
                entities::entity_to_engine(pool.origin(), data.entity_scale),
                range * data.entity_scale,
                i as u64 + 1,
            ));
        }
    }
    let solid = if rider.is_some() { PropSolid::None } else { prop.solid };
    insert_prop_collider(
        &mut e,
        colliders,
        data,
        prop,
        model,
        solid,
        model_colliders[prop.model].as_ref(),
        dynamic,
        prop_brush,
    );
    let id = e.id();
    D::spawn_prop(
        a,
        commands,
        s,
        drawn,
        PropSpawn {
            prop,
            id,
            fade_range: fade_range.as_ref(),
            home: rider.map_or(placed, |r| r.1),
        },
    );
}

/// Props are hidden where the camera can't see them, and beyond their
/// fade distance; between the near and far fade distances their meshes
/// dither out (Bevy's visibility range, returned: opaque passes, no
/// sorting). Props that stay put are tagged once; moving ones (physics,
/// riders, animated) follow their bounds as they move
/// (`tag_moved_brush_entities`), as Source culls every entity by the
/// view's PVS: other rooms' props must not show through the sky.
fn tag_prop(
    e: &mut EntityCommands,
    data: &MapData,
    prop: &MapProp,
    model: &MapModel,
    stays_put: bool,
) -> Option<bevy::camera::visibility::VisibilityRange> {
    let mut fade_range = None;
    if !prop.skybox && !stays_put && !merged_world() && data.visibility.is_some() {
        let (min, max) = model.bounds;
        if min.x <= max.x {
            e.insert((BrushEntityBounds { min, max }, VisClusters::new(Vec::new())));
        }
        if let Some((near, far)) = prop.fade {
            e.insert(vis::FadeDistance(far));
            fade_range = vis::fade_band(near, far);
        }
    }
    if !prop.skybox && stays_put && !merged_world() {
        let clusters = match data.visibility.as_deref() {
            Some(v) => {
                let (lo, hi) = model.bounds;
                let corners = (0..8).map(|k| {
                    let c = Vec3::new(
                        if k & 1 == 0 { lo.x } else { hi.x },
                        if k & 2 == 0 { lo.y } else { hi.y },
                        if k & 4 == 0 { lo.z } else { hi.z },
                    );
                    prop.translation + prop.rotation * c
                });
                let (min, max) = corners.fold((Vec3::MAX, Vec3::MIN), |(a, b), p| (a.min(p), b.max(p)));
                // Hidden behind occluders by its bounds too.
                if !v.occluders.is_empty() && min.x <= max.x {
                    e.insert(vis::Occludee { min, max });
                }
                vis::box_clusters(v, min, max)
            }
            None => Vec::new(),
        };
        if !clusters.is_empty() || prop.fade.is_some() {
            e.insert(VisClusters::new(clusters));
        }
        if let Some((near, far)) = prop.fade {
            e.insert(vis::FadeDistance(far));
            fade_range = vis::fade_band(near, far);
        }
    }
    fade_range
}

/// The box along the world's axes (relative to the prop's origin) around
/// `bounds` turned by `rotation`.
pub fn world_box(bounds: (Vec3, Vec3), rotation: Quat) -> (Vec3, Vec3) {
    let (lo, hi) = bounds;
    (0..8).fold((Vec3::MAX, Vec3::MIN), |(a, b), k| {
        let c = Vec3::new(
            if k & 1 == 0 { lo.x } else { hi.x },
            if k & 2 == 0 { lo.y } else { hi.y },
            if k & 4 == 0 { lo.z } else { hi.z },
        );
        let p = rotation * c;
        (a.min(p), b.max(p))
    })
}

/// A prop's collision: its model's collider (a dynamic or frozen body
/// for physics props), or its bounds box.
#[allow(clippy::too_many_arguments)]
fn insert_prop_collider(
    e: &mut EntityCommands,
    colliders: &mut Colliders,
    data: &MapData,
    prop: &MapProp,
    model: &MapModel,
    solid: PropSolid,
    collider: Option<&Collider>,
    dynamic: Option<&MapPhysics>,
    prop_brush: bool,
) {
    match (solid, collider) {
        (PropSolid::Mesh, Some(collider)) if dynamic.is_some() => {
            let p = dynamic.unwrap();
            e.insert(prop_physics::dynamic_body(p, data.models[prop.model].bounds));
            colliders.put(e, collider.clone());
            if p.asleep && prop.entity.is_some() {
                e.insert((prop_physics::StartAsleep, RigidBody::Static));
            } else {
                e.insert(prop_physics::WedgeCheck::default());
            }
            if let Some(s) = &model.surfaceprop {
                e.insert(PropSurface(s.clone()));
            }
        }
        (PropSolid::Mesh, Some(collider)) => {
            e.insert((RigidBody::Static, MapPropCollider));
            colliders.put(e, collider.clone());
            // A static prop is part of the world (its collision is the
            // world's: a body hinged to the world with "No Collision"
            // passes through it too).
            if prop.entity.is_none() && !prop.skybox {
                e.insert(super::controllers::WorldCollider);
            }
            // A frozen physics prop can be set moving later.
            if let Some(p) = prop
                .physics
                .as_ref()
                .filter(|p| p.frozen && !prop.skybox && prop.entity.is_some())
            {
                e.insert(prop_physics::FrozenBody {
                    physics: p.clone(),
                    bounds: data.models[prop.model].bounds,
                });
            }
            if let Some(s) = &model.surfaceprop {
                e.insert(PropSurface(s.clone()));
            }
            if prop_brush {
                e.insert(MapBrushCollider);
            }
        }
        (PropSolid::Box | PropSolid::WorldBox, _) => {
            let world = solid == PropSolid::WorldBox;
            let (lo, hi) = if world {
                world_box(data.models[prop.model].bounds, prop.rotation)
            } else {
                data.models[prop.model].bounds
            };
            let size = hi - lo;
            if size.min_element() > 0.0 {
                // A world box: turned back against the prop's rotation.
                let back = if world { prop.rotation.inverse() } else { Quat::IDENTITY };
                let child = Transform::from_translation(back * ((lo + hi) / 2.0)).with_rotation(back);
                e.insert(RigidBody::Static).with_children(|c| {
                    let mut ec = c.spawn((child, MapPropCollider));
                    colliders.put(&mut ec, Collider::cuboid(size.x, size.y, size.z));
                    if let Some(s) = &model.surfaceprop {
                        ec.insert(PropSurface(s.clone()));
                    }
                    if prop_brush {
                        ec.insert(MapBrushCollider);
                    }
                });
            }
        }
        _ => {}
    }
}

/// The loaded map's resources: what queries and effects read (visibility,
/// decals, surface colours, particles, the HUD, sounds, character and view
/// models, the light field, the nav mesh) and, with any brushes, the
/// collision world and what goes with it.
fn insert_resources<D: MapDrawer>(commands: &mut Commands, s: &MapSpawn, drawn: &D::Drawn, brushes: Vec<MapBrush>) {
    let data = s.data;
    let texture_handles = D::textures(drawn);
    if let Some(v) = data.visibility.as_ref().filter(|_| !merged_world()) {
        commands.insert_resource(vis::ActiveVisibility(v.clone()));
    }
    D::insert_drawn(drawn, commands, s, DrawnResource::DecalSurfaces);
    let colors = s.prepared.take::<surface_color::SurfaceColors>();
    commands.insert_resource(colors.unwrap_or_else(|| surface_color::SurfaceColors::new(data)));
    commands.insert_resource(particles::ParticleMaterials(data.particles.clone()));
    D::insert_drawn(drawn, commands, s, DrawnResource::Particles);
    let look = |h: &Arc<hud::GameHud>| {
        let images = h
            .sprites
            .values()
            .filter_map(|s| Some((s.texture, texture_handles.get(s.texture)?.clone())))
            .collect();
        hud::ActiveHud(h.clone(), images)
    };
    if let Some(h) = &data.kit.hud {
        commands.insert_resource(look(h));
    }
    if !data.kit.guest_huds.is_empty() {
        commands.insert_resource(hud::GuestLooks(data.kit.guest_huds.iter().map(look).collect()));
    }
    if let Some(o) = &data.overview
        && let Some(image) = texture_handles.get(o.texture)
    {
        commands.insert_resource(hud::ActiveOverview(o.clone(), image.clone()));
    }
    D::insert_drawn(drawn, commands, s, DrawnResource::Decals);
    commands.insert_resource(sound::SoundBank(data.sounds.clone()));
    // Whose map it is (its ladders' rules: `core::MapLadders`).
    if let Some(game) = data.impacts.map_game() {
        commands.insert_resource(crate::core::MapGame(game.to_string()));
    }
    commands.insert_resource(super::impact::ActiveImpacts(data.impacts.clone()));
    if !data.kit.characters.is_empty() {
        commands.insert_resource(CharacterModels(Arc::new(data.kit.characters.clone())));
    }
    if !data.kit.view_models.is_empty() {
        commands.insert_resource(ViewModels::of(data.kit.view_models.iter().cloned()));
    }
    if let Some(field) = &data.light_field {
        commands.insert_resource(LightField(field.clone()));
    }
    if let Some(nav) = &data.kit.nav {
        commands.insert_resource((**nav).clone());
    }
    commands.insert_resource(sound::SurfaceGrid::new(data));
    commands.insert_resource(super::sources::SurfaceNames(Arc::new(data.surface_names.clone())));
    commands.insert_resource(data.memos.clone());
    if !brushes.is_empty() {
        let floor = data
            .playable
            .map(|(lo, _)| lo.y)
            .unwrap_or_else(|| brushes.iter().map(|b| b.min.y).fold(f32::MAX, f32::min));
        commands.insert_resource(KillHeight(floor - KILL_MARGIN));
        commands.insert_resource(MapBrushes(brushes));
        commands.insert_resource(MapTraceSkip(data.trace_skip.clone()));
        commands.insert_resource(MapPropBrushStart(data.collision_brushes.len()));
        if let Some(tree) = &data.brush_tree {
            commands.insert_resource(tree.clone());
        }
        if let Some(g) = data.gravity {
            // Times the server's scale (`gravity`).
            commands.queue(move |w: &mut World| super::gravity::set_map_gravity(w, Vec3::NEG_Y * g));
        }
        commands.insert_resource(MapWater(data.water.clone()));
        commands.insert_resource(MapSkyFaces::of(&data.sky_surfaces));
        commands.insert_resource(water::WaterMovers(
            data.water_movers
                .iter()
                .filter(|(v, e)| *v < data.water.len() && s.node(*e).is_some())
                .map(|&(v, e)| (v, e, data.water[v].brush.clone(), s.node_home(e)))
                .collect(),
        ));
        D::insert_drawn(drawn, commands, s, DrawnResource::Fog);
        commands.insert_resource(data.kit.round_sounds.clone());
        if let Some(r) = &data.kit.radio {
            commands.insert_resource(r.clone());
        }
        if let Some(p) = &data.kit.bot_profiles {
            commands.insert_resource(p.clone());
        }
    }
}

/// The sun (lighting the few plain materials) and the spawn points.
fn spawn_sun_and_spawn_points(commands: &mut Commands, data: &MapData) {
    commands.spawn((
        Name::new("Sun"),
        MapPart,
        DirectionalLight {
            illuminance: 9000.0,
            // Models are lit from the baked light field (probe_lit), so no
            // shadow maps (they cost several ms a frame); the sun only lights
            // the few plain materials left.
            affects_lightmapped_mesh_diffuse: false,
            ..default()
        },
        Transform::default().looking_at(Vec3::new(-0.4, -1.0, -0.3), Vec3::Y),
    ));
    for (i, (feet, team)) in data.spawns.iter().enumerate() {
        commands.spawn((
            Name::new(format!("Spawn {i}")),
            MapPart,
            SpawnPoint { team: *team },
            Transform::from_translation(*feet + Vec3::Y * SPAWN_LIFT)
                .with_rotation(Quat::from_rotation_y(data.spawn_yaws.get(i).copied().unwrap_or(0.0))),
        ));
    }
}

/// Comparison hook: CHIMERA_MERGED_WORLD=1 spawns the world as one mesh per
/// material with no visibility culling (as before chunking), for A/B
/// measurements against `vis`.
pub fn merged_world() -> bool {
    chimera_core::env::var("CHIMERA_MERGED_WORLD").is_ok_and(|v| v == "1")
}
