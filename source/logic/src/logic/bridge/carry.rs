use avian3d::prelude::{
    AngularVelocity, Collider, ColliderAabb, ColliderDisabled, CollisionLayers, CollisionStart, ComputedMass,
    LayerMask, LinearVelocity, PhysicsSystems, Position, RigidBody, RigidBodyDisabled, Rotation,
};
use bevy::{ecs::message::MessageCursor, prelude::*};
use chimera_core::query_cache::{cached_query, first};

use crate::logic::classes::ServerLine;
use crate::logic::hud::{HudEvent, HudMessages, HudShow, ScreenFades};
use crate::logic::prop_damage::{Hit, Motion, PropExplosion};
use crate::logic::world::{Collision, Effect, EntId, LogicWorld, Player, SOLID_SKIN, SWEEP_EPS, Who};
use crate::console::{Console, ConsoleAppExt, Level};
use crate::core::{
    BaseVelocity, Damage, DamageKind, Damageable, EntityGravity, Explosion, Health, Hitgroup, Intent, LocalPlayer,
    MapBrush, MapBrushes, MapTerrain, MovementState, MovingSolid, RoundRestarts, SimSet, Team, Velocity,
};
use crate::map::breakables::{FallingPane, GibPiece, GlassImpact, GlassShatter, SpawnGibs};
use crate::map::entities::{engine_to_entity, entity_rotation, entity_to_engine, rotation_to_engine};
use crate::map::{LogicHidden, VisClusters};
use crate::map::{
    BreakProp, BrushPanes, EntityPart, LightStyles, MapBrushEntity, MapEntities, PlaySound, PropEntity, PropHome,
    PropIndex, PropLook, PropSequence, SoundControl, SoundKey, SoundLevel, SoundscapeSwitches, SoundscapeTouches,
    StartSound,
};

use super::*;

/// Static collision through the map's brushes and terrain.
pub(super) struct WorldCollision<'a> {
    pub(super) brushes: Option<&'a MapBrushes>,
    pub(super) terrain: Option<&'a MapTerrain>,
    pub(super) scale: f32,
}

impl WorldCollision<'_> {
    /// Engine centre and half size of an entity-space box.
    fn engine_box(&self, mins: Vec3, maxs: Vec3, at: Vec3) -> (Vec3, Vec3) {
        let h = (maxs - mins) / 2.0;
        (
            entity_to_engine(at + (mins + maxs) / 2.0, self.scale),
            Vec3::new(h.x, h.z, h.y) * self.scale,
        )
    }

    fn near(&self, lo: Vec3, hi: Vec3) -> Vec<&MapBrush> {
        let mut out: Vec<&MapBrush> = self.brushes.map(|b| b.0.iter().collect()).unwrap_or_default();
        if let Some(t) = self.terrain {
            let mut idx = Vec::new();
            t.near(lo, hi, &mut idx);
            out.extend(idx.into_iter().map(|i| &t.brushes[i as usize]));
        }
        out
    }
}

impl Collision for WorldCollision<'_> {
    fn sweep(&self, mins: Vec3, maxs: Vec3, from: Vec3, to: Vec3) -> f32 {
        let (a, half) = self.engine_box(mins, maxs, from);
        let (b, _) = self.engine_box(mins, maxs, to);
        let eps = SWEEP_EPS * self.scale;
        self.near(a.min(b) - half, a.max(b) + half)
            .into_iter()
            .filter_map(|br| br.sweep_box(half, a, b, eps))
            .map(|(f, _)| f)
            .fold(1.0, f32::min)
    }

    fn solid(&self, mins: Vec3, maxs: Vec3, at: Vec3) -> bool {
        let (c, half) = self.engine_box(mins, maxs, at);
        let skin = SOLID_SKIN * self.scale;
        self.near(c - half, c + half)
            .into_iter()
            .any(|b| b.overlaps_box(c, half, skin))
    }
}

/// The static world owned (the logic keeps it for traces outside a
/// phase: fires dropping inside an input, their line-of-sight checks).
pub(super) struct StaticCollision {
    brushes: MapBrushes,
    terrain: Option<MapTerrain>,
    /// The static props' brushes (`map::MapPropBrushStart`).
    props: Vec<MapBrush>,
    scale: f32,
}

impl StaticCollision {
    fn view(&self) -> WorldCollision<'_> {
        WorldCollision {
            brushes: Some(&self.brushes),
            terrain: self.terrain.as_ref(),
            scale: self.scale,
        }
    }
}

impl Collision for StaticCollision {
    fn sweep(&self, mins: Vec3, maxs: Vec3, from: Vec3, to: Vec3) -> f32 {
        self.view().sweep(mins, maxs, from, to)
    }
    fn solid(&self, mins: Vec3, maxs: Vec3, at: Vec3) -> bool {
        self.view().solid(mins, maxs, at)
    }
    fn inside_prop(&self, at: Vec3) -> bool {
        let p = entity_to_engine(at, self.scale);
        let skin = SOLID_SKIN * self.scale;
        self.props.iter().any(|b| b.overlaps_box(p, Vec3::ZERO, skin))
    }
}

/// The map's static world for the logic, once the map has one.
pub(super) fn static_collision(world: &World, scale: f32) -> Option<std::sync::Arc<dyn Collision + Send + Sync>> {
    let mut brushes = world.get_resource::<MapBrushes>()?.clone();
    let props = world
        .get_resource::<crate::map::MapPropBrushStart>()
        .and_then(|s| brushes.0.get(s.0..))
        .map(<[MapBrush]>::to_vec)
        .unwrap_or_default();
    // Fire's traces pass player clips and grates.
    if let Some(skip) = world.get_resource::<crate::map::MapTraceSkip>() {
        let skip: std::collections::HashSet<usize> = skip.0.iter().copied().collect();
        let mut i = 0;
        brushes.0.retain(|_| {
            i += 1;
            !skip.contains(&(i - 1))
        });
    }
    Some(std::sync::Arc::new(StaticCollision {
        brushes,
        terrain: world.get_resource::<MapTerrain>().cloned(),
        props,
        scale,
    }))
}

/// An entity-space brush in engine space.
pub fn brush_to_engine(b: &MapBrush, scale: f32) -> MapBrush {
    let planes = b
        .planes
        .iter()
        .map(|(n, d)| (Vec3::new(n.x, n.z, -n.y), d * scale))
        .collect();
    let (a, c) = (entity_to_engine(b.min, scale), entity_to_engine(b.max, scale));
    MapBrush {
        planes,
        min: a.min(c),
        max: a.max(c),
        ladder: b.ladder,
        surface: b.surface.clone(),
    }
}

pub(super) type CharacterQuery<'a> = (
    Entity,
    &'a Transform,
    &'a Velocity,
    &'a MovementState,
    &'a Intent,
    Option<&'a Health>,
    Option<&'a Team>,
    Option<&'a BaseVelocity>,
    Option<&'a EntityGravity>,
    Option<&'a LocalPlayer>,
    Option<&'a ColliderAabb>,
    Option<&'a crate::core::MapControls>,
    Option<&'a crate::core::Bot>,
);

/// The characters as logic players: the local player first (`!player`
/// is slot 1), then by entity.
pub(super) fn snapshot(world: &mut World, logic: &Logic) -> Vec<Player> {
    let scale = logic.scale;
    let ground_of = |g: Entity| {
        logic
            .nodes
            .iter()
            .chain(&logic.props)
            .find(|(_, n)| *n == g)
            .map(|(id, _)| *id)
    };
    let mut list: Vec<(bool, Player)> = cached_query::<CharacterQuery, (), _>(world, |q, world| {
        q.iter(world)
            .map(|row| (row.9.is_some(), as_player(row, scale, ground_of)))
            .collect()
    });
    list.sort_by_key(|(local, p)| (!*local, p.entity));
    list.into_iter().map(|(_, p)| p).collect()
}

/// A character as a logic player; `ground_of` names the logic entity of
/// what it stands on.
pub(super) fn as_player(
    (e, t, v, state, intent, health, team, base, gravity, _, aabb, controls, bot): bevy::ecs::query::QueryItem<
        '_,
        '_,
        CharacterQuery<'static>,
    >,
    scale: f32,
    ground_of: impl Fn(Entity) -> Option<EntId>,
) -> Player {
    let origin = engine_to_entity(t.translation, scale);
    let (lo, hi) = if state.hull_min != state.hull_max {
        (state.hull_min, state.hull_max)
    } else if let Some(a) = aabb {
        (a.min - t.translation, a.max - t.translation)
    } else {
        (Vec3::splat(-0.4), Vec3::splat(0.4))
    };
    let (a, b) = (engine_to_entity(lo, scale), engine_to_entity(hi, scale));
    let mut p = Player::new(e, origin);
    p.mins = a.min(b);
    p.maxs = a.max(b);
    p.eye = engine_to_entity(state.eye_offset, scale);
    p.velocity = engine_to_entity(v.0, scale);
    if let Some(b) = base {
        p.base_velocity = engine_to_entity(b.velocity, scale);
        p.base_touched = b.touched;
    }
    p.on_ground = state.on_ground;
    p.ground = state.ground.and_then(ground_of);
    p.view = Vec3::new(-intent.pitch.to_degrees(), intent.yaw.to_degrees() + 90.0, 0.0);
    p.alive = health.is_none_or(|h| h.current > 0.0);
    // Our teams 1 and 2 are Source's 2 (T) and 3 (CT).
    p.team = team.map_or(0, |t| if t.0 == 0 { 0 } else { t.0 + 1 });
    p.gravity = gravity.map_or(1.0, |g| g.0);
    p.use_key = intent.use_key;
    p.buttons = controls.map_or_else(|| crate::core::buttons::of(intent), |c| c.buttons);
    p.bot = bot.is_some();
    p
}

/// A mover a network client moves itself this tick (`net::movers`), as
/// the server's logic steps it: where it was (entity space, angles in
/// degrees) and how far it moves and turns.
#[derive(Clone, Copy, Debug)]
pub struct CarriedMover<'a> {
    pub node: Entity,
    /// Its map entity index (`map::MapBrushEntity`).
    pub index: u32,
    /// Its hulls (`map::MapEntity::hulls`, entity space).
    pub hulls: &'a [crate::map::MapHull],
    pub origin: Vec3,
    pub angles: Vec3,
    pub d: Vec3,
    pub da: Vec3,
    pub solid: bool,
    pub physics_solid: bool,
    pub unblockable: bool,
}

/// A mover's world brushes at a pose (entity space), as the logic keeps
/// them.
pub fn mover_brushes_at(hulls: &[crate::map::MapHull], origin: Vec3, angles: Vec3) -> Vec<MapBrush> {
    let rot = entity_rotation(angles);
    hulls.iter().map(|h| crate::logic::world::place_hull(h, rot, origin)).collect()
}

/// A network client predicting its own player (`net::movers`): the push
/// of this tick's mover steps (`movers`, in map order) on `player`, as
/// the server's logic does it before movement (riders carried, players in
/// the way shoved; `movers::push_players`, the same code). A step the
/// player would block is left out (the server's mover would stop or
/// reverse: the next correction says how). Returns whether it moved.
pub fn carry_player(world: &mut World, player: Entity, scale: f32, movers: &[CarriedMover]) -> bool {
    let ground_of = |g: Entity| {
        movers.iter().find(|m| m.node == g).map(|m| EntId {
            index: m.index,
            generation: 0,
        })
    };
    let before = cached_query::<CharacterQuery, (), _>(world, |q, world| {
        q.get(world, player).ok().map(|row| as_player(row, scale, ground_of))
    });
    let Some(before) = before else {
        return false;
    };
    let mut players = vec![before.clone()];
    let mut solids: Vec<Option<Vec<MapBrush>>> = movers
        .iter()
        .map(|m| m.solid.then(|| mover_brushes_at(m.hulls, m.origin, m.angles)))
        .collect();
    {
        let col = WorldCollision {
            brushes: world.get_resource::<MapBrushes>(),
            terrain: world.get_resource::<MapTerrain>(),
            scale,
        };
        for (k, m) in movers.iter().enumerate() {
            let (o_new, a_new) = (m.origin + m.d, m.angles + m.da);
            if m.solid && (m.d != Vec3::ZERO || m.da != Vec3::ZERO) {
                let moved = mover_brushes_at(m.hulls, o_new, a_new);
                let others: Vec<&MapBrush> = solids
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != k)
                    .filter_map(|(_, b)| b.as_ref())
                    .flatten()
                    .collect();
                let step = crate::logic::movers::PushStep {
                    id: EntId {
                        index: m.index,
                        generation: 0,
                    },
                    origin: m.origin,
                    angles: m.angles,
                    d: m.d,
                    da: m.da,
                    physics_solid: m.physics_solid,
                    unblockable: m.unblockable,
                };
                let _ = crate::logic::movers::push_players(&mut players, &step, &moved, &others, &col);
                solids[k] = Some(moved);
            }
        }
    }
    let moved = players[0].moved;
    write_back(world, scale, std::slice::from_ref(&before), &players);
    moved
}

/// Write back what the logic changed on players.
pub(super) fn write_back(world: &mut World, scale: f32, before: &[Player], after: &[Player]) {
    for p in after {
        let Some(old) = before.iter().find(|b| b.entity == p.entity) else {
            continue;
        };
        let Ok(mut e) = world.get_entity_mut(p.entity) else {
            continue;
        };
        if p.moved || p.teleported {
            if let Some(mut t) = e.get_mut::<Transform>() {
                t.translation = entity_to_engine(p.origin, scale);
            }
        }
        // Never a velocity that is not a number (NaN differs from itself,
        // so it would be written every tick).
        if p.velocity != old.velocity
            && p.velocity.is_finite()
            && let Some(mut v) = e.get_mut::<Velocity>()
        {
            v.0 = entity_to_engine(p.velocity, scale);
        }
        if p.view != old.view
            && let Some(mut i) = e.get_mut::<Intent>()
        {
            i.yaw = (p.view.y - 90.0).to_radians().rem_euclid(std::f32::consts::TAU);
            i.pitch = (-p.view.x).to_radians().clamp(-89f32.to_radians(), 89f32.to_radians());
        }
        if p.base_velocity != old.base_velocity || p.base_touched != old.base_touched || p.unground || p.teleported {
            let unground = p.unground || p.teleported;
            let base = BaseVelocity {
                velocity: entity_to_engine(p.base_velocity, scale),
                touched: p.base_touched,
                unground,
            };
            e.insert(base);
        }
        if p.gravity != old.gravity {
            e.insert(EntityGravity(p.gravity));
        }
    }
}

/// Mover nodes follow their logic entity; nodes of removed entities
/// (killed, broken) are hidden (a round restart shows them again).
pub(super) fn sync_movers(world: &mut World, logic: &mut Logic) {
    logic.nodes.retain(|(id, node)| {
        let alive = logic.world.get(*id).is_some();
        if !alive {
            hide_node(world, *node);
        }
        alive
    });
    // Each mover's node by its entity's slot (the generation checked): a
    // hash map made here every phase was a tenth of a tick on a map with
    // thousands of movers (docs/performance.md, "Performance pass").
    let mut nodes: Vec<Option<(u32, Entity)>> = Vec::new();
    for (id, node) in &logic.nodes {
        let i = id.index as usize;
        if nodes.len() <= i {
            nodes.resize(i + 1, None);
        }
        nodes[i] = Some((id.generation, *node));
    }
    let every = world.contains_resource::<SyncEveryNode>();
    let riders = logic.world.body_riders();
    let mut cache = std::mem::take(&mut logic.movers_seen);
    // The mover nodes whose components something changed since the last
    // phase (`synced`: the change tick after its writes), sorted.
    let mut touched: Vec<Entity> = match cache.synced {
        Some(t) if !every => world.last_change_tick_scope(t, |world| {
            cached_query::<Entity, (With<MovingSolid>, MoverViewChanged), _>(world, |q, world| q.iter(world).collect())
        }),
        _ => Vec::new(),
    };
    touched.sort_unstable();
    // Which movers share a node (written every phase, as before), worked
    // out again when the movers or their nodes change.
    let mut print = (0u64, 0usize);
    // What the logic wants of each mover's node (the logic alone says):
    // its entity's part kept from the last phase unless the entity may
    // have changed since (`LogicWorld::changed_since`). A mover whose
    // node read as wanted then, and of which nothing it depends on has
    // changed since (its entity, its node's components, whether it rides
    // a body, its brushes), would read so again: it is passed over.
    let mut wants: Vec<MoverWants> = Vec::new();
    let mut slots: Vec<usize> = Vec::new();
    for id in logic.world.ids_of(crate::logic::classes::kinds::PUSHERS) {
        let Some(node) = nodes
            .get(id.index as usize)
            .and_then(|n| n.as_ref())
            .filter(|(generation, _)| *generation == id.generation)
            .map(|(_, node)| *node)
        else {
            continue;
        };
        let i = id.index as usize;
        if cache.seen.len() <= i {
            cache.seen.resize_with(i + 1, || None);
        }
        let unchanged = !every && !logic.world.changed_since(id, cache.mark);
        let seen = match cache.seen[i].as_mut().filter(|seen| unchanged && seen.id == id) {
            Some(seen) => seen,
            None => {
                let Some(look) = MoverLook::of(logic, id) else {
                    cache.seen[i] = None;
                    continue;
                };
                cache.seen[i].insert(SeenMover { id, look, settled: None })
            }
        };
        print.0 ^= (id.index as u64) << 32 ^ id.generation as u64 ^ node.to_bits().rotate_left(17);
        print.1 += 1;
        // A brush riding a physics brush or prop touches no physics body
        // (it would shove the body carrying it) but shots still hit it
        // (cs_havana's monitor screen); players meet its brushes
        // (`MovingSolid`).
        let rides = riders.rides(id);
        let solids = logic.world.mover_solid(id).map_or(0, |b| b.len());
        let settled = seen.settled.take();
        if let Some(was) = settled
            && was.node == node
            && was.rides == rides
            && was.solids == solids
            && world.entities().get_spawned(node).is_ok_and(|l| l.archetype_id == was.archetype)
            && touched.binary_search(&node).is_err()
        {
            seen.settled = Some(was);
            continue;
        }
        let look = &seen.look;
        // A broken window's panes are for shots only
        // (`map::breakables::update_panes`): every game's characters pass
        // them, and touching them shatters them (`touch_breakables`).
        let layers = if look.panes.is_some() {
            CollisionLayers::new(crate::core::TRACE_ONLY_LAYER, LayerMask::NONE)
        } else if rides {
            CollisionLayers::new(LayerMask::DEFAULT, LayerMask::NONE)
        } else {
            CollisionLayers::default()
        };
        wants.push(MoverWants {
            id,
            node,
            transform: look.transform,
            layers,
            rides,
            visible: look.visible,
            tint: look.tint,
            solid: look.solid,
            velocity: look.velocity,
            solids,
            panes: look.panes.clone(),
            shootable: look.shootable,
        });
        slots.push(i);
    }
    drop(riders);
    if cache.print != print {
        // Nodes two movers share, from every mover's node.
        let mut all: Vec<Entity> = nodes.iter().flatten().map(|(_, n)| *n).collect();
        all.sort_unstable();
        cache.shared = all.windows(2).filter(|w| w[0] == w[1]).map(|w| w[0]).collect();
        cache.shared.dedup();
        cache.print = print;
    }
    cache.mark = logic.world.mark();
    // The nodes with something to write: all but those that already read
    // as wanted (most movers, most ticks), each told through a kept query
    // (a node gets its brushes, `MovingSolid`, at its first write; one
    // without them isn't in the query and is written). A node two movers
    // share is written as before: the second would be judged by what the
    // first has yet to write.
    let mut write = vec![true; wants.len()];
    if !every {
        cached_query::<(Entity, MoverView), With<MovingSolid>, _>(world, |q, world| {
            for (k, w) in wants.iter().enumerate() {
                if cache.shared.binary_search(&w.node).is_ok() {
                    continue;
                }
                let Ok((_, have)) = q.get(world, w.node) else { continue };
                if !w.differ(&have) {
                    write[k] = false;
                    let archetype = world.entities().get_spawned(w.node).map(|l| l.archetype_id);
                    if let (Some(Some(seen)), Ok(archetype)) = (cache.seen.get_mut(slots[k]), archetype) {
                        seen.settled = Some(Settled {
                            node: w.node,
                            archetype,
                            rides: w.rides,
                            solids: w.solids,
                        });
                    }
                }
            }
        });
    }
    // The writes, in the movers' order: each only where the node differs.
    for (want, _) in wants.into_iter().zip(write).filter(|(_, write)| *write) {
        let MoverWants {
            id,
            node,
            transform,
            layers,
            rides,
            visible,
            tint,
            solid,
            velocity,
            panes,
            shootable,
            ..
        } = want;
        let Ok(mut e) = world.get_entity_mut(node) else {
            continue;
        };
        if e.get::<CollisionLayers>().is_some_and(|l| *l != layers) || (rides && !e.contains::<CollisionLayers>()) {
            e.insert(layers);
        }
        let mut moved = true;
        if let Some(mut t) = e.get_mut::<Transform>() {
            moved = *t != transform;
            if moved {
                *t = transform;
            }
        }
        set_node_shown(&mut e, visible);
        if e.get::<crate::map::BrushTint>()
            .map_or(!tint.is_white(), |t| *t != tint)
        {
            e.insert(tint);
        }
        // Its brushes follow its pose: rebuilt (and the component written,
        // which movement reads) only when it moved or changed.
        let solids = logic.world.mover_solid(id);
        let same = !moved
            && e.get::<MovingSolid>().is_some_and(|m| {
                m.solid == solid && m.velocity == velocity && m.brushes.len() == solids.map_or(0, |b| b.len())
            });
        if !same {
            let brushes: Vec<MapBrush> = solids
                .map(|b| b.iter().map(|b| brush_to_engine(b, logic.scale)).collect())
                .unwrap_or_default();
            e.insert(MovingSolid {
                brushes,
                velocity,
                solid,
            });
        }
        // Shots and physics pass what is not there (a broken breakable,
        // a disabled func_brush); a broken window keeps its panes.
        if let Some(p) = panes {
            if e.get::<BrushPanes>() != Some(&p) {
                e.insert(p);
            }
        } else if shootable == e.contains::<ColliderDisabled>() {
            if shootable {
                e.remove::<ColliderDisabled>();
            } else {
                e.insert(ColliderDisabled);
            }
        }
    }
    // Changes after this (other systems', later passes') have newer ticks.
    cache.synced = Some(world.increment_change_tick());
    logic.movers_seen = cache;
}

/// With this resource every phase runs the whole of `sync_movers` and
/// `sync_props` for every node, as before the kept queries' checks that
/// pass over the nodes with nothing to write (tests compare the two:
/// games/cs_source/tests/it/map_logic.rs, `skipping_unchanged_nodes_changes_nothing`).
#[derive(Resource, Default)]
pub struct SyncEveryNode;

/// What `sync_movers` reads of a mover's node to tell whether it has
/// anything to write there.
pub(super) type MoverView = (
    Option<&'static CollisionLayers>,
    Option<&'static Transform>,
    Has<LogicHidden>,
    Option<&'static VisClusters>,
    Option<&'static Visibility>,
    Option<&'static crate::map::BrushTint>,
    Option<&'static MovingSolid>,
    Option<&'static BrushPanes>,
    Has<ColliderDisabled>,
);

/// What the logic wants of a mover's node this phase.
pub(super) struct MoverWants {
    id: EntId,
    node: Entity,
    transform: Transform,
    layers: CollisionLayers,
    rides: bool,
    visible: bool,
    tint: crate::map::BrushTint,
    solid: bool,
    velocity: Vec3,
    /// Its brushes' count.
    solids: usize,
    panes: Option<BrushPanes>,
    shootable: bool,
}

/// What `sync_movers` keeps between phases (docs/performance.md, "A
/// change stamp for the movers").
#[derive(Default)]
pub(super) struct MoverCache {
    /// By slot: each mover's `MoverLook` and, if its node read as wanted
    /// at the last phase, what that depended on (`Settled`).
    seen: Vec<Option<SeenMover>>,
    /// The logic's mark (`LogicWorld::mark`) and the change tick taken at
    /// the end of the last phase.
    mark: u64,
    synced: Option<bevy::ecs::change_detection::Tick>,
    /// Nodes two movers share (sorted), and a print of the movers and
    /// their nodes they were worked out for.
    shared: Vec<Entity>,
    print: (u64, usize),
}

pub(super) struct SeenMover {
    id: EntId,
    look: MoverLook,
    settled: Option<Settled>,
}

/// A mover whose node read as wanted (nothing written) at the last
/// phase: its node, the node's archetype (a component added or taken
/// away moves it), and what else its wants were made of.
#[derive(Clone, Copy)]
pub(super) struct Settled {
    node: Entity,
    archetype: bevy::ecs::archetype::ArchetypeId,
    rides: bool,
    solids: usize,
}

/// A change to what `MoverView` reads (`Changed` of each component a
/// node may hold; one added or taken away moves it to another archetype).
pub(super) type MoverViewChanged = Or<(
    Changed<CollisionLayers>,
    Changed<Transform>,
    Changed<VisClusters>,
    Changed<Visibility>,
    Changed<crate::map::BrushTint>,
    Changed<MovingSolid>,
    Changed<BrushPanes>,
)>;

/// What a mover's logic entity alone says its node should read (its
/// pose, whether it shows, its colour, solidity, velocity, broken panes,
/// whether shots hit it): kept between phases while the entity isn't
/// changed (`LogicWorld::changed_since`).
#[derive(Clone)]
pub(super) struct MoverLook {
    transform: Transform,
    visible: bool,
    tint: crate::map::BrushTint,
    solid: bool,
    velocity: Vec3,
    panes: Option<BrushPanes>,
    shootable: bool,
}

impl MoverLook {
    /// `id`'s, if it is a mover (as `LogicWorld::mover_poses` says).
    fn of(logic: &Logic, id: EntId) -> Option<Self> {
        let e = logic.world.get(id)?;
        let p = crate::logic::movers::pusher(&e.class)?;
        let (origin, angles, velocity, visible, solid) = (p.origin, p.angles, p.velocity + p.carry, p.visible, p.solid);
        let shootable = logic.world.shootable(id);
        let panes = logic.world.window(id).filter(|w| w.window_broken).map(|w| {
            let s = logic.scale;
            let n = w.normal;
            let depth = e
                .hulls
                .iter()
                .flat_map(|h| &h.points)
                .map(|p| (*p - w.corner).dot(n))
                .fold((f32::MAX, f32::MIN), |(lo, hi), d| (lo.min(d), hi.max(d)));
            let depth = if depth.0 > depth.1 {
                (0.0, 0.0)
            } else {
                (depth.0 * s, depth.1 * s)
            };
            BrushPanes {
                corner: entity_to_engine(w.corner, s),
                u: entity_to_engine(w.u, s),
                v: entity_to_engine(w.v, s),
                depth,
                cols: w.cols,
                rows: w.rows,
                broken: w.broken.clone(),
            }
        });
        let transform = Transform::from_translation(entity_to_engine(origin, logic.scale))
            .with_rotation(rotation_to_engine(entity_rotation(angles)));
        Some(Self {
            transform,
            visible,
            // Its render colour (rendercolor, the Color input).
            tint: crate::map::BrushTint(e.render_look(), e.texture_frame),
            solid,
            velocity: entity_to_engine(velocity, logic.scale),
            panes,
            shootable,
        })
    }
}

impl MoverWants {
    /// Whether `sync_movers` would write anything to a node that reads
    /// `have`: each of its tests, in its order.
    #[allow(clippy::type_complexity)]
    fn differ(
        &self,
        have: &(
            Option<&CollisionLayers>,
            Option<&Transform>,
            bool,
            Option<&VisClusters>,
            Option<&Visibility>,
            Option<&crate::map::BrushTint>,
            Option<&MovingSolid>,
            Option<&BrushPanes>,
            bool,
        ),
    ) -> bool {
        let (layers, transform, hidden, vis, visibility, tint, moving, panes, disabled) = *have;
        if layers.is_some_and(|l| *l != self.layers) || (self.rides && layers.is_none()) {
            return true;
        }
        // Without a transform its brushes are written every time.
        if transform != Some(&self.transform) {
            return true;
        }
        // `set_node_shown`.
        if self.visible {
            let culled = vis.is_some_and(|v| !v.potentially_visible);
            let want = if culled { Visibility::Hidden } else { Visibility::Inherited };
            if hidden || visibility != Some(&want) {
                return true;
            }
        } else if !hidden || visibility != Some(&Visibility::Hidden) {
            return true;
        }
        if tint.map_or(!self.tint.is_white(), |t| *t != self.tint) {
            return true;
        }
        if !moving.is_some_and(|m| m.solid == self.solid && m.velocity == self.velocity && m.brushes.len() == self.solids) {
            return true;
        }
        match &self.panes {
            Some(p) => panes != Some(p),
            None => self.shootable == disabled,
        }
    }
}

