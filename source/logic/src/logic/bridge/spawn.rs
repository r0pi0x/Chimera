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

/// A fixed seed for the map logic's random choices (`LogicWorld::seed`);
/// None (the game): a new one at every map load. The headless harness
/// fixes it so tests repeat.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LogicSeed(pub Option<u64>);

/// A seed from the clock.
pub(super) fn time_seed() -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    (now.as_nanos() as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
}

/// Build (or drop) the logic world when the map's entities change, and
/// re-create it at a round restart (`core::RoundRestarts`).
pub(super) fn load(world: &mut World) {
    // A map with no entity space of its own (scale 0: entities added to
    // a `MapData` whose scale nobody set, as a code-built map's kit,
    // `map::sources::CODE_MAP_KIT`, was before it got its metres) has no
    // logic: with one, every conversion into its space divided by zero,
    // and a player's velocity came back NaN (the sandbox crashed a few
    // frames in, in the view bob's clamp).
    let current = world.get_resource::<MapEntities>().filter(|m| m.scale > 0.0).cloned();
    let built = world.get_resource::<Logic>().map(|l| l.source.clone());
    let restarts = world.get_resource::<RoundRestarts>().map_or(0, |r| r.0);
    match (current, built) {
        (None, Some(_)) => {
            restore_settings(world);
            world.write_message(SoundControl::StopAll);
            world.remove_resource::<Logic>();
            world.remove_resource::<LightStyles>();
            world.remove_resource::<crate::map::TonemapInputs>();
            world.remove_resource::<crate::map::AreaPortalStates>();
            world.remove_resource::<crate::map::OccluderStates>();
            world.remove_resource::<SoundscapeTouches>();
            world.remove_resource::<SoundscapeSwitches>();
            world.remove_resource::<crate::map::fire::MapFires>();
        }
        (Some(m), b) if b.as_ref().is_none_or(|b| !std::sync::Arc::ptr_eq(b, &m.entities)) => {
            restore_settings(world);
            world.write_message(SoundControl::StopAll);
            let dt = world.resource::<Time<Fixed>>().timestep().as_secs_f32();
            let mut logic = LogicWorld::new(dt);
            // The map's dice (logic_case PickRandom, logic_timer random
            // times): new at every map load, as the game's are, unless
            // fixed (tests).
            logic.seed(world.get_resource::<LogicSeed>().and_then(|s| s.0).unwrap_or_else(time_seed));
            logic.collision = static_collision(world, m.scale);
            logic.game_classes = world
                .get_resource::<crate::map::entities::GameEntityClasses>()
                .map(|g| g.0.iter().map(|c| c.to_string()).collect())
                .unwrap_or_default();
            logic.break_keeps_props = world.get_resource::<KeepPropsOnBreak>().is_some_and(|k| k.0);
            let ids = logic.load_map(&m.entities);
            logic.announce_game_entities();
            let nodes = attach_nodes(world, &logic, &ids);
            let props = attach_props(world, &logic, &ids, false);
            world.insert_resource(Logic {
                world: logic,
                scale: m.scale,
                nodes,
                props,
                restore: Vec::new(),
                weapons: m
                    .entities
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| crate::map::entities::anchor_class(e.classname()))
                    .map(|(i, _)| i)
                    .collect(),
                source: m.entities.clone(),
                restarts,
                movers_seen: MoverCache::default(),
            });
        }
        _ => {
            let Some(mut logic) = world.remove_resource::<Logic>() else {
                return;
            };
            if logic.restarts != restarts {
                logic.restarts = restarts;
                // The entities' sounds stop; re-created ones start again.
                world.write_message(SoundControl::StopAll);
                // Templates' copies go; the map's entities start again.
                let copies: Vec<Entity> = world
                    .query_filtered::<Entity, With<crate::map::copies::MapCopy>>()
                    .iter(world)
                    .collect();
                for c in copies {
                    world.entity_mut(c).despawn();
                }
                let source = logic.source.clone();
                let ids = logic.world.round_restart(&source);
                logic.world.announce_game_entities();
                logic.nodes = attach_nodes(world, &logic.world, &ids);
                logic.props = attach_props(world, &logic.world, &ids, true);
                // HUD messages from the last round go too.
                if let Some(mut hud) = world.get_resource_mut::<HudMessages>() {
                    *hud = HudMessages::default();
                }
            }
            world.insert_resource(logic);
        }
    }
}

/// Pair every mover node with its logic entity (`ids` in map order).
/// Nodes whose entity is gone are hidden; the others are shown and solid
/// (the next sync places them), and breakables take damage from weapons.
pub(super) fn attach_nodes(world: &mut World, logic: &LogicWorld, ids: &[EntId]) -> Vec<(EntId, Entity)> {
    let all: Vec<(Entity, usize)> = world
        .query::<(Entity, &MapBrushEntity)>()
        .iter(world)
        .map(|(e, n)| (e, n.0))
        .collect();
    let mut nodes = Vec::new();
    for (node, index) in all {
        let Some(id) = ids.get(index).copied().filter(|id| logic.get(*id).is_some()) else {
            hide_node(world, node);
            continue;
        };
        let breakable = matches!(
            logic.get(id).map(|e| &e.class),
            Some(crate::logic::classes::Class::Breakable(_))
        );
        let mut e = world.entity_mut(node);
        e.remove::<(ColliderDisabled, MovingSolid)>();
        set_node_shown(&mut e, true);
        if breakable {
            e.insert(Damageable);
        } else {
            e.remove::<Damageable>();
        }
        nodes.push((id, node));
    }
    nodes
}

/// A node whose logic entity is gone (killed, broken): not drawn, hit or
/// stood on, until a round restart brings the entity back.
pub(super) fn hide_node(world: &mut World, node: Entity) {
    if let Ok(mut e) = world.get_entity_mut(node) {
        // Only when missing: re-inserting ColliderDisabled replaces it,
        // and avian puts a replaced one's collider back in its query tree.
        if !e.contains::<ColliderDisabled>() {
            e.insert(ColliderDisabled);
        }
        set_node_shown(&mut e, false);
        e.insert(MovingSolid::default()).remove::<Damageable>();
    }
}

/// Show or hide a mover node for the logic (`LogicHidden`): shown, it is
/// drawn when visibility culling (`VisClusters`, by its bounds) allows.
pub fn set_node_shown(e: &mut EntityWorldMut, visible: bool) {
    let hidden = e.contains::<LogicHidden>();
    if visible {
        let culled = e.get::<VisClusters>().is_some_and(|v| !v.potentially_visible);
        if hidden {
            e.remove::<LogicHidden>();
        }
        let want = if culled { Visibility::Hidden } else { Visibility::Inherited };
        if e.get::<Visibility>() != Some(&want) {
            e.insert(want);
        }
    } else if !hidden || e.get::<Visibility>() != Some(&Visibility::Hidden) {
        e.insert((LogicHidden, Visibility::Hidden));
    }
}

/// Pair every prop node (`map::PropEntity`) with its logic prop (`ids` in
/// map order). They are shown and solid again; with `restart`, a moved
/// physics prop goes back where the map placed it, at rest.
pub(super) fn attach_props(world: &mut World, logic: &LogicWorld, ids: &[EntId], restart: bool) -> Vec<(EntId, Entity)> {
    let all: Vec<(Entity, usize, Option<Transform>)> = world
        .query::<(Entity, &PropEntity, Option<&PropHome>)>()
        .iter(world)
        .map(|(e, p, h)| (e, p.0, h.map(|h| h.0)))
        .collect();
    let states = logic.prop_states();
    let mut out = Vec::new();
    for (node, index, home) in all {
        let Some(id) = ids.get(index).copied() else { continue };
        let Some(state) = states.iter().find(|s| s.id == id) else {
            // Taken out of the map (a template's member): never there.
            if logic.get(id).is_none() {
                set_prop_shown(world, node, false, false, false);
            }
            continue;
        };
        let (visible, solid, damageable) = (state.visible, state.solid, state.damageable);
        if restart
            && let Some(home) = home
            && let Ok(mut e) = world.get_entity_mut(node)
            && e.contains::<RigidBody>()
        {
            if let Some(mut t) = e.get_mut::<Transform>() {
                *t = home;
            }
            if let Some(mut p) = e.get_mut::<Position>() {
                p.0 = home.translation;
            }
            if let Some(mut r) = e.get_mut::<Rotation>() {
                *r = Rotation::from(home.rotation);
            }
            if let Some(mut v) = e.get_mut::<LinearVelocity>() {
                v.0 = Vec3::ZERO;
            }
            if let Some(mut v) = e.get_mut::<AngularVelocity>() {
                v.0 = Vec3::ZERO;
            }
        }
        if restart {
            crate::map::prop_physics::reset(world, node, state.asleep);
        }
        set_prop_shown(world, node, visible, solid, true);
        set_prop_look(world, node, state, logic.round);
        if let Ok(mut e) = world.get_entity_mut(node) {
            if damageable {
                e.insert(Damageable);
            } else {
                e.remove::<Damageable>();
            }
        }
        out.push((id, node));
    }
    out
}

/// Show or hide a prop node and turn its collision (its own collider and
/// its children's) on or off; `exists` false also stops its body.
pub(super) fn set_prop_shown(world: &mut World, node: Entity, visible: bool, solid: bool, exists: bool) {
    let Ok(e) = world.get_entity(node) else { return };
    let hidden = e.contains::<LogicHidden>();
    if visible == hidden {
        let shown_by_vis = e.get::<VisClusters>().is_none_or(|v| v.potentially_visible);
        let mut e = world.entity_mut(node);
        if visible {
            e.remove::<LogicHidden>();
            if shown_by_vis {
                e.insert(Visibility::Inherited);
            }
        } else {
            e.insert((LogicHidden, Visibility::Hidden));
        }
    }
    // The body comes back before its colliders do: avian's query tree
    // copies "body disabled" into a collider's proxy when the collider
    // joins it and doesn't clear it when RigidBodyDisabled goes away
    // (`tests::avian_still_leaves_body_disabled_on_the_proxy`). The broad
    // phase skips proxies so marked; with avian 0.7 a stale flag also made
    // each new contact pair of that collider link into an island twice
    // (games/cs_source/tests/it/map_logic.rs
    // `restored_prop_contacts_join_islands_once`).
    let enable_body = {
        let mut e = world.entity_mut(node);
        let enable = exists && e.contains::<RigidBody>() && e.contains::<RigidBodyDisabled>();
        if enable {
            e.remove::<RigidBodyDisabled>();
        }
        enable
    };
    let colliders: Vec<Entity> = std::iter::once(node)
        .chain(world.get::<Children>(node).map(|c| c.to_vec()).unwrap_or_default())
        .filter(|c| world.get::<Collider>(*c).is_some())
        .collect();
    for c in colliders {
        let mut e = world.entity_mut(c);
        // Only on change: a re-inserted ColliderDisabled re-adds the
        // collider to avian's query tree (see `hide_node`).
        if solid == e.contains::<ColliderDisabled>() {
            if solid {
                e.remove::<ColliderDisabled>();
            } else {
                e.insert(ColliderDisabled);
            }
        } else if solid && enable_body {
            // Solid all along under a disabled body: rejoin the tree so
            // its proxy forgets the disabled body.
            e.insert(ColliderDisabled).remove::<ColliderDisabled>();
        }
    }
    let mut e = world.entity_mut(node);
    // A prop's own brushes (movement) follow its collision.
    if let Some(mut m) = e.get_mut::<MovingSolid>()
        && m.solid != solid
    {
        m.solid = solid;
    }
    if !exists && e.contains::<RigidBody>() && !e.contains::<RigidBodyDisabled>() {
        e.insert(RigidBodyDisabled);
    }
    if !exists {
        e.remove::<Damageable>();
    }
    // A map ragdoll's bodies follow (`map::placed_ragdoll::sync`).
    if let Some(mut r) = e.get_mut::<crate::map::placed_ragdoll::PlacedRagdoll>()
        && (r.gone == exists || r.solid != solid)
    {
        r.gone = !exists;
        r.solid = solid;
    }
}

/// Placed weapons the logic removed (`map::entities::RemovedWeapons`;
/// written only when it changes).
pub(super) fn sync_removed_weapons(world: &mut World, logic: &Logic) {
    let gone: Vec<usize> = logic
        .weapons
        .iter()
        .copied()
        .filter(|i| !logic.world.map_entity_alive(*i))
        .collect();
    let now = world.get_resource::<crate::map::entities::RemovedWeapons>();
    if now.is_none_or(|n| n.0 != gone) {
        world.insert_resource(crate::map::entities::RemovedWeapons(gone));
    }
}

/// Physics clips (`map::physics_clip`) follow their logic entity: solid
/// to physics objects while it exists and is enabled; a filtered one
/// blocks the props and brush entities its filter passes (of those, only
/// physics bodies meet it). Components
/// are written only when they change.
pub(super) fn sync_physics_clips(world: &mut World, logic: &Logic) {
    use crate::map::physics_clip::{ClipPasses, PhysicsClip};
    let clips: Vec<(Entity, usize)> = world
        .query::<(Entity, &PhysicsClip)>()
        .iter(world)
        .map(|(e, c)| (e, c.0))
        .collect();
    for (clip, index) in clips {
        let w = &logic.world;
        let id = w.by_map_index(index).filter(|id| w.get(*id).is_some_and(|e| !e.killed));
        let on = id.is_some_and(|id| {
            matches!(w.get(id).map(|e| &e.class), Some(crate::logic::classes::Class::Part(p)) if p.on)
        });
        let filter = id
            .and_then(|id| w.get(id))
            .and_then(|e| e.kv("filtername"))
            .filter(|f| !f.trim().is_empty())
            .map(|f| w.find(f));
        let passes = ClipPasses(filter.map(|f| {
            // Props and physics brushes, templates' copies too (the
            // football's ball is a func_physbox a template spawns).
            logic
                .props
                .iter()
                .chain(&logic.nodes)
                .filter(|(p, _)| f.is_none_or(|f| crate::logic::classes::filter_passes(w, f, Some(Who::Ent(*p)))))
                .map(|(_, node)| *node)
                .collect()
        }));
        let Ok(mut e) = world.get_entity_mut(clip) else { continue };
        if on == e.contains::<ColliderDisabled>() {
            if on {
                e.remove::<ColliderDisabled>();
            } else {
                e.insert(ColliderDisabled);
            }
        }
        if e.get::<ClipPasses>() != Some(&passes) {
            e.insert(passes);
        }
    }
}

/// Prop nodes follow their logic prop: hidden and not solid once it is
/// gone (broken, killed) until a round restart, or as its inputs say.
pub(super) fn sync_props(world: &mut World, logic: &mut Logic) {
    // In entity order (`prop_states`): found by slot, the generation
    // checked.
    let states = logic.world.prop_states();
    let props = std::mem::take(&mut logic.props);
    let every = world.contains_resource::<SyncEveryNode>();
    for &(id, node) in &props {
        let state = states
            .binary_search_by_key(&id.index, |s| s.id.index)
            .ok()
            .map(|i| &states[i])
            .filter(|s| s.id == id);
        // Nothing to write (most props, most ticks): told from the node
        // as it is, read through kept queries, by the tests of
        // `set_prop_shown` and `set_prop_look`.
        let (visible, solid, exists) = state.map_or((false, false, false), |s| (s.visible, s.solid, !s.gone));
        let round = logic.world.round;
        if !every && !prop_differs(world, node, visible, solid, exists, state.map(|s| (s, round))) {
            continue;
        }
        match state {
            Some(state) => {
                set_prop_shown(world, node, state.visible, state.solid, !state.gone);
                set_prop_look(world, node, state, logic.world.round);
            }
            None => set_prop_shown(world, node, false, false, false),
        }
    }
    logic.props = props;
}

/// What `sync_props` reads of a prop's node to tell whether it has
/// anything to write there.
pub(super) type PropView = (
    (Has<LogicHidden>, Has<RigidBody>, Has<RigidBodyDisabled>, Has<Collider>, Has<ColliderDisabled>),
    Option<&'static Children>,
    Option<&'static MovingSolid>,
    Has<Damageable>,
    Option<&'static crate::map::placed_ragdoll::PlacedRagdoll>,
    Option<&'static PropLook>,
    Option<&'static PropSequence>,
);

/// Whether `set_prop_shown(world, node, visible, solid, exists)` and, with
/// `look` (the prop's state and the round), `set_prop_look` would write
/// anything: each of their tests. A node that is gone has nothing to
/// write.
pub(super) fn prop_differs(
    world: &mut World,
    node: Entity,
    visible: bool,
    solid: bool,
    exists: bool,
    look: Option<(&crate::logic::props::PropState, u32)>,
) -> bool {
    cached_query::<PropView, (), _>(world, |view, world| {
        let Ok(((hidden, body, body_off, collider, collider_off), children, moving, damageable, ragdoll, have_look, have_sequence)) =
            view.get(world, node)
        else {
            return false;
        };
        // `set_prop_shown`: shown or hidden, its body back, its own
        // collider, its brushes, a body that is gone, its damage, its
        // placed ragdoll.
        if visible == hidden
            || (exists && body && body_off)
            || (collider && solid == collider_off)
            || moving.is_some_and(|m| m.solid != solid)
            || (!exists && body && !body_off)
            || (!exists && damageable)
            || ragdoll.is_some_and(|r| r.gone == exists || r.solid != solid)
        {
            return true;
        }
        // `set_prop_look`.
        if let Some((state, round)) = look
            && (have_look.is_none_or(|l| l.skin != state.skin || l.body_group != state.body_group)
                || have_sequence
                    .is_none_or(|s| s.play != state.sequence || s.default != state.default_sequence || s.round != round))
        {
            return true;
        }
        // Its children's colliders.
        children.is_some_and(|children| {
            children.iter().any(|c| {
                world
                    .get_entity(c)
                    .is_ok_and(|c| c.contains::<Collider>() && solid == c.contains::<ColliderDisabled>())
            })
        })
    })
}

/// A prop node shows the logic prop's skin, body group and sequence
/// (`map::PropLook`, `map::PropSequence`; written only when they change).
pub(super) fn set_prop_look(world: &mut World, node: Entity, state: &crate::logic::props::PropState, round: u32) {
    let Ok(mut e) = world.get_entity_mut(node) else { return };
    let look = PropLook {
        skin: state.skin,
        body_group: state.body_group,
    };
    if e.get::<PropLook>() != Some(&look) {
        e.insert(look);
    }
    let sequence = PropSequence {
        play: state.sequence.clone(),
        default: state.default_sequence.clone(),
        round,
    };
    if e.get::<PropSequence>() != Some(&sequence) {
        e.insert(sequence);
    }
}

/// Sprites and dust volumes (`map::EntityPart`) follow their entity: on,
/// off, or gone (killed) until a round restart; light styles follow the
/// lights (`map::LightStyles`), areaportals their doors and inputs
/// (`map::AreaPortalStates`), occluders their inputs
/// (`map::OccluderStates`).
pub(super) fn sync_visuals(world: &mut World, logic: &Logic) {
    let mut states = logic.world.part_states();
    states.sort_by_key(|s| s.0);
    // Kept query states (`query_cache`): this runs every tick.
    cached_query::<&mut EntityPart, (), _>(world, |parts, world| {
        for mut part in parts.iter_mut(world) {
            let want = match states.binary_search_by_key(&part.entity, |s| s.0) {
                Ok(i) => (states[i].2, true),
                Err(_) => (false, false),
            };
            if (part.on, part.exists) != want {
                part.on = want.0;
                part.exists = want.1;
            }
        }
    });
    // Steam jets' value inputs (meters; the Rate input does nothing).
    let mut jets = logic.world.steam_jets();
    jets.sort_by_key(|j| j.0);
    let scale = logic.scale;
    let mut changes = Vec::new();
    cached_query::<(Entity, &EntityPart, Option<&crate::map::SteamInputs>), (), _>(world, |parts, world| {
        for (e, part, have) in parts.iter(world) {
            let want = jets
                .binary_search_by_key(&part.entity, |j| j.0)
                .ok()
                .map(|i| crate::map::SteamInputs {
                    length: jets[i].1.length.map(|v| v * scale),
                    spread: jets[i].1.spread.map(|v| v * scale),
                    speed: jets[i].1.speed.map(|v| v * scale),
                });
            if want.as_ref() != have {
                changes.push((e, want));
            }
        }
    });
    for (e, want) in changes {
        match want {
            Some(w) => world.entity_mut(e).insert(w),
            None => world.entity_mut(e).remove::<crate::map::SteamInputs>(),
        };
    }
    // Parts whose entity follows a parent go where it is now (a trail
    // behind a spinning prop, a sprite on a lift). Their visibility
    // clusters were where they spawned: dropped once they move.
    let mut poses = logic.world.part_poses();
    if !poses.is_empty() {
        poses.sort_by_key(|p| p.0);
        let scale = logic.scale;
        let mut moved = Vec::new();
        let mut parts =
            world.query_filtered::<(Entity, &EntityPart, &mut Transform), With<crate::map::FollowsEntity>>();
        for (e, part, mut t) in parts.iter_mut(world) {
            let Ok(i) = poses.binary_search_by_key(&part.entity, |p| p.0) else {
                continue;
            };
            let (_, origin, angles) = poses[i];
            let at = crate::map::entities::entity_to_engine(origin, scale);
            let rotation = crate::map::entities::rotation_to_engine(crate::map::entities::entity_rotation(angles));
            if t.translation.distance_squared(at) > 1e-10 || t.rotation.dot(rotation).abs() < 1.0 - 1e-7 {
                t.translation = at;
                t.rotation = rotation;
                moved.push(e);
            }
        }
        for e in moved {
            let mut part = world.entity_mut(e);
            if part.take::<crate::map::VisClusters>().is_some() && !part.contains::<LogicHidden>() {
                part.insert(Visibility::Inherited);
            }
        }
    }
    // How players are drawn (invisibility, colours).
    for (player, look) in &logic.world.player_looks {
        if let Ok(mut e) = world.get_entity_mut(*player)
            && e.get::<crate::map::RenderLook>() != Some(look)
        {
            e.insert(*look);
        }
    }
    let styles = LightStyles {
        styles: logic.world.light_styles().to_vec(),
    };
    if world.get_resource::<LightStyles>() != Some(&styles) {
        world.insert_resource(styles);
    }
    let tonemap = logic.world.tonemap();
    if world.get_resource::<crate::map::TonemapInputs>() != Some(tonemap) {
        world.insert_resource(tonemap.clone());
    }
    let portals = crate::map::AreaPortalStates {
        closed: logic.world.closed_area_portals(),
    };
    if world.get_resource::<crate::map::AreaPortalStates>() != Some(&portals) {
        world.insert_resource(portals);
    }
    let occluders = crate::map::OccluderStates {
        inactive: logic.world.inactive_occluders(),
    };
    if world.get_resource::<crate::map::OccluderStates>() != Some(&occluders) {
        world.insert_resource(occluders);
    }
}

/// The trigger_soundscape volumes the local player touches, for the
/// soundscape selection (`map::SoundscapeTouches`).
pub(super) fn sync_soundscapes(world: &mut World, logic: &Logic) {
    let local = first::<With<LocalPlayer>>(world);
    let touched: Vec<usize> = match local {
        Some(p) => logic
            .world
            .ids_of(crate::logic::classes::kinds::TRIGGER)
            .into_iter()
            .filter_map(|id| {
                let e = logic.world.get(id)?;
                if !e.classname.eq_ignore_ascii_case("trigger_soundscape") {
                    return None;
                }
                let crate::logic::classes::Class::Trigger(t) = &e.class else {
                    return None;
                };
                (t.enabled && t.touching.contains(&crate::logic::world::Who::Player(p))).then_some(e.map_index?)
            })
            .collect(),
        None => Vec::new(),
    };
    let want = SoundscapeTouches(Some(touched));
    if world.get_resource::<SoundscapeTouches>() != Some(&want) {
        world.insert_resource(want);
    }
    // env_soundscapes the Enable/Disable inputs left on.
    let enabled: Vec<usize> = logic
        .world
        .part_states()
        .into_iter()
        .filter(|(_, kind, on)| *kind == crate::logic::visuals::PartKind::Soundscape && *on)
        .map(|(i, ..)| i)
        .collect();
    let switches = SoundscapeSwitches(Some(enabled));
    if world.get_resource::<SoundscapeSwitches>() != Some(&switches) {
        world.insert_resource(switches);
    }
}

pub(super) fn restore_settings(world: &mut World) {
    let Some(restore) = world
        .get_resource_mut::<Logic>()
        .map(|mut l| std::mem::take(&mut l.restore))
    else {
        return;
    };
    if let Some(mut c) = world.get_resource_mut::<Console>() {
        for (name, value) in restore {
            info!("point_servercommand: map unloaded, {name} back to {value}");
            c.info(format!("point_servercommand: map unloaded, {name} back to {value}"));
            c.submit(format!("{name} {}", crate::console::quote(&value)));
        }
    }
}

