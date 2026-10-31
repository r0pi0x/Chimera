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
//! What needs only the data (the world's colliders, the models' colliders
//! and hulls, the props' brushes: `WorldParts`) comes prepared ahead when
//! the map was loaded in the background (`prepare`, `prepare_world`), else
//! is computed here by the same functions (the props' brushes on threads).

use bevy::ecs::system::{StaticSystemParam, SystemParam, SystemParamItem};

#[allow(unused_imports)]
use super::*;

/// What spawns a map's drawn parts, as a system parameter (the asset
/// stores they go into; each may be missing headless, and then what it
/// would draw isn't spawned). `spawn_map` calls it at fixed places.
pub trait MapDrawer: SystemParam + 'static {
    /// What the drawn phases make that the props and the resources use
    /// (render handles; empty headless).
    type Drawn;
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
    /// beams, ropes and shadows.
    fn spawn_drawn(p: &mut SystemParamItem<Self>, commands: &mut Commands, s: &MapSpawn) -> Self::Drawn;
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

/// Spawns the pending map, its drawn parts by `D`.
pub fn spawn_map<D: MapDrawer>(mut commands: Commands, pending: Res<PendingMap>, mut assets: StaticSystemParam<D>) {
    let assets = &mut *assets;
    let data = &pending.0;
    let view = pending.1;
    let prepared = &pending.2;
    let world = prepared.take::<WorldParts>().unwrap_or_else(|| world_parts(data));
    let (root, statics) = spawn_roots(&mut commands, data, D::world_material(assets));
    spawn_world_collision(&mut commands, data, world.collision, root);
    super::physics_clip::spawn_clips(&mut commands, data, root);
    let entity_nodes = spawn_entity_nodes(&mut commands, data, root);
    let s = MapSpawn {
        data,
        view,
        root,
        statics,
        entity_nodes,
        visibility: data.visibility.as_deref().filter(|_| !merged_world()),
        prepared,
    };
    insert_entities(&mut commands, &s);
    insert_headless_assets(&mut commands, &s, D::draws(assets));
    D::spawn_undrawn(assets, &mut commands, &s);
    let mut drawn = D::spawn_drawn(assets, &mut commands, &s);
    let brushes = spawn_props::<D>(&mut commands, &s, assets, &mut drawn, world.props);
    insert_resources::<D>(&mut commands, &s, &drawn, brushes);
    spawn_sun_and_spawn_points(&mut commands, data);
    // Last: the pending data goes (nothing reads it after the spawn), and
    // with it the textures' texels, unless something kept a share.
    commands.queue(retire_pending::<D>);
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

/// The world's job for `MapHooks::prepare`.
pub(super) fn prepare_world(_: &World) -> prepare::PrepareJob {
    Box::new(|data, prepared| prepared.insert(world_parts(data)))
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
fn spawn_world_collision(commands: &mut Commands, data: &MapData, parts: CollisionParts, root: Entity) {
    if let Some((terrain, surfaces, smooth)) = parts.terrain {
        commands.insert_resource(terrain);
        commands.entity(root).with_child((
            Name::new("Map collision (surfaces)"),
            MapPart,
            MapTerrainCollider,
            RigidBody::Static,
            surfaces,
            // Loose items slide on their own copy (below).
            CollisionLayers::new(LayerMask::DEFAULT, LayerMask::ALL & !crate::core::ITEM_LAYER),
            Transform::default(),
        ));
        if let Some(smooth) = smooth {
            commands.entity(root).with_child((
                Name::new("Map collision (surfaces, for loose items)"),
                MapPart,
                MapTerrainCollider,
                RigidBody::Static,
                smooth,
                CollisionLayers::new(crate::core::ITEM_GROUND_LAYER, crate::core::ITEM_LAYER),
                Transform::default(),
            ));
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
            RigidBody::Static,
            collider,
            layers,
            Transform::default(),
            ChildOf(root),
        ));
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
            RigidBody::Static,
            collider,
            Transform::default(),
            ChildOf(root),
        ));
        if glass {
            e.insert(SeeThrough);
        }
    }
}

/// Mover brush entities: a node each (placed by origin and angles), with
/// a kinematic collider; their meshes are spawned under it later.
/// Anchors (placed weapons with entities parented to them) get a node
/// too, which their props ride and the weapon layer moves.
fn spawn_entity_nodes(commands: &mut Commands, data: &MapData, root: Entity) -> Vec<Option<Entity>> {
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
                    node.insert((collider, PropEntity(i), PropHome(home)));
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
                    node.insert((
                        MapBrushCollider,
                        RigidBody::Kinematic,
                        collider,
                        contact_filter::hooks(),
                    ));
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

/// Placed props: a node each with its collision, culling, meshes,
/// skeleton and what the logic may switch. Returns the map's brushes for
/// movement (the world's, plus props that are (close to) convex).
fn spawn_props<D: MapDrawer>(
    commands: &mut Commands,
    s: &MapSpawn,
    a: &mut SystemParamItem<D>,
    drawn: &mut D::Drawn,
    parts: PropParts,
) -> Vec<MapBrush> {
    let data = s.data;
    // Exact brushes for movement that sweeps them (Source): the world's,
    // plus props that are (close to) convex, in world space.
    let mut brushes = data.collision_brushes.clone();
    info!(
        "props: {} of {} models collide by their collision model, {} more as convex hulls; {} have levels of detail",
        data.models.iter().filter(|m| m.collision.is_some()).count(),
        data.models.len(),
        parts.model_hulls.iter().filter(|h| h.is_some()).count(),
        data.models.iter().filter(|m| !m.lods.is_empty()).count(),
    );
    for ((i, prop), placed) in data.props.iter().enumerate().zip(parts.brushes) {
        spawn_prop::<D>(commands, s, a, drawn, &mut brushes, i, prop, &parts.model_colliders, placed);
    }
    brushes
}

/// One placed prop (`placed`: its brushes, `prop_brushes`).
#[allow(clippy::too_many_arguments)]
fn spawn_prop<D: MapDrawer>(
    commands: &mut Commands,
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
            e.insert((
                collider.clone(),
                prop_physics::dynamic_body(p, data.models[prop.model].bounds),
            ));
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
            e.insert((RigidBody::Static, collider.clone(), MapPropCollider));
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
                let child = (
                    Collider::cuboid(size.x, size.y, size.z),
                    Transform::from_translation(back * ((lo + hi) / 2.0)).with_rotation(back),
                );
                e.insert(RigidBody::Static).with_children(|c| {
                    let mut ec = c.spawn((child, MapPropCollider));
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
    commands.insert_resource(surface_color::SurfaceColors::new(data));
    commands.insert_resource(particles::ParticleMaterials(data.particles.clone()));
    D::insert_drawn(drawn, commands, s, DrawnResource::Particles);
    if let Some(h) = &data.kit.hud {
        let images = h
            .sprites
            .values()
            .filter_map(|s| Some((s.texture, texture_handles.get(s.texture)?.clone())))
            .collect();
        commands.insert_resource(hud::ActiveHud(h.clone(), images));
    }
    if let Some(o) = &data.overview
        && let Some(image) = texture_handles.get(o.texture)
    {
        commands.insert_resource(hud::ActiveOverview(o.clone(), image.clone()));
    }
    D::insert_drawn(drawn, commands, s, DrawnResource::Decals);
    commands.insert_resource(sound::SoundBank(data.sounds.clone()));
    commands.insert_resource(super::impact::ActiveImpacts(data.impacts.clone()));
    if !data.kit.characters.is_empty() {
        commands.insert_resource(CharacterModels(Arc::new(data.kit.characters.clone())));
    }
    if !data.kit.view_models.is_empty() {
        commands.insert_resource(ViewModels(Arc::new(data.kit.view_models.clone())));
    }
    if let Some(field) = &data.light_field {
        commands.insert_resource(LightField(field.clone()));
    }
    if let Some(nav) = &data.kit.nav {
        commands.insert_resource((**nav).clone());
    }
    commands.insert_resource(sound::SurfaceGrid::new(data));
    commands.insert_resource(super::sources::SurfaceNames(Arc::new(data.surface_names.clone())));
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
            commands.insert_resource(Gravity(Vec3::NEG_Y * g));
        }
        commands.insert_resource(MapWater(data.water.clone()));
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
