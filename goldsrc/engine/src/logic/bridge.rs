//! GoldSrc's logic world in the ECS (`GoldSrcLogicPlugin`): built when a
//! map whose first entity names this family (`map::entities::
//! LOGIC_FAMILY_KEY` = `logic::FAMILY`) loads, dropped when it goes,
//! re-created at a round restart (`core::RoundRestarts`); stepped once a
//! fixed tick on the server (`core::authoritative`) after the characters'
//! movement and before the weapons (the spec's order: the players'
//! touches and `+use`, then the entities), with:
//!
//! - the characters as logic players (`Player`: the box of their
//!   collider, their view, `Intent::use_key`), written back when the
//!   logic pushed, carried, teleported or turned them;
//! - the static world (`core::MapBrushes`, ladders left out) as what a
//!   pushed player may not end inside (`world::Solid`);
//! - the mover nodes (`map::MapBrushEntity`) placed by their entity's
//!   origin and angles, hidden (`map::LogicHidden`) and not solid
//!   (`ColliderDisabled`) when gone, off or passable, their brushes and
//!   velocity as `core::MovingSolid` for movements that sweep brushes,
//!   their frame and changed render keys as `map::BrushTint` (frame 1:
//!   the faces' other texture set, `MapMesh::frames`), breakables and
//!   shootable buttons `core::Damageable`;
//! - weapons' `core::Damage` on those nodes into the logic (after the
//!   weapons); `map::entities::FireEntityOutput` (the rules: a bomb
//!   target's when the bomb explodes) and `LogicFire` (a name fired, the
//!   console's `goldsrc_fire`, tests) before the step;
//! - a character as a logic player by `player_of` (its box the
//!   movement's hull), shared with a network client's carry (`net`);
//! - its effects: sounds (`map::PlaySound`, `map::SoundControl`), damage
//!   (`core::MapDamage`; negative heals), explosions (`core::Explosion`,
//!   magnitude × 2.5 units), switchable light styles (`map::LightStyles`
//!   under the writer `goldsrc`).
//!
//! Known shortcuts:
//! - Clients run no GoldSrc logic: its nodes and sprites reach them
//!   through `net` (the node placing here, `place_node`, shared).
//! - Sounds are named by their path under `sound/`, and GoldSrc maps
//!   carry no sound bank yet, so they are silent; a trigger's `message`
//!   is logged, not shown; broken breakables throw no debris.

use std::sync::Arc;

use avian3d::prelude::{ColliderAabb, ColliderDisabled};
use bevy::{ecs::message::MessageCursor, prelude::*};

use super::{
    FAMILY,
    world::{Effect, Id, Player, Solid, UseType, Who, World as Logic},
};
use crate::{
    core::{
        BaseVelocity, Damage, DamageKind, Damageable, Explosion, Health, Hitgroup, Intent, MapBrush, MapBrushes,
        MapDamage, MovementState, MovingSolid, RoundRestarts, SimSet, Velocity,
    },
    map::{
        BrushTint, LightStyles, LogicHidden, MapBrushEntity, MapEntities, MapEntity, PlaySound, RenderLook,
        SoundControl, SoundKey, StartSound, VisClusters,
        entities::{FireEntityOutput, engine_to_entity, entity_rotation, entity_to_engine, logic_family, rotation_to_engine},
    },
};

/// The running logic of the loaded GoldSrc map.
#[derive(Resource)]
pub struct GoldSrcLogic {
    pub world: Logic,
    /// Metres per entity unit.
    pub scale: f32,
    /// Mover entity -> its node.
    pub nodes: Vec<(Id, Entity)>,
    /// What each node was last written as.
    seen: Vec<Option<Seen>>,
    source: Arc<Vec<MapEntity>>,
    restarts: u32,
}

impl GoldSrcLogic {
    /// The map entities it was built from.
    pub fn source(&self) -> Arc<Vec<MapEntity>> {
        self.source.clone()
    }
}

/// A node's state as last written.
#[derive(Clone, Copy, PartialEq)]
struct Seen {
    origin: Vec3,
    angles: Vec3,
    shown: bool,
    solid: bool,
    tint: BrushTint,
}

/// Fire a name in the loaded GoldSrc map's logic (the console's
/// `goldsrc_fire`, tests), with no activator.
#[derive(Message, Clone, Debug)]
pub struct LogicFire(pub String);

/// Where the logic runs in the fixed tick.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct GoldSrcLogicSet;

pub struct GoldSrcLogicPlugin;

impl Plugin for GoldSrcLogicPlugin {
    fn build(&self, app: &mut App) {
        use crate::console::ConsoleAppExt;
        app.add_message::<LogicFire>()
            .add_message::<FireEntityOutput>()
            .add_message::<PlaySound>()
            .add_message::<SoundControl>()
            .add_message::<MapDamage>()
            .add_message::<Explosion>()
            .add_message::<Damage>()
            .configure_sets(
                FixedUpdate,
                GoldSrcLogicSet
                    .after(SimSet::Movement)
                    .before(SimSet::Weapons)
                    .run_if(crate::core::authoritative),
            )
            .add_systems(FixedUpdate, (load, step).chain().in_set(GoldSrcLogicSet))
            .add_systems(FixedUpdate, damage.after(SimSet::Weapons).run_if(crate::core::authoritative));
        super::net::plugin(app);
        app.console_command(
            "goldsrc_fire",
            "goldsrc_fire <name>: fire a name in the GoldSrc map's logic (no activator).",
            |w, a| {
                let name = a.first().ok_or("goldsrc_fire <name>")?;
                w.write_message(LogicFire(name.clone()));
                Ok(None)
            },
        );
    }
}

/// Build, drop or re-create the logic as the map's entities and round
/// restarts change.
fn load(world: &mut World) {
    let current = world
        .get_resource::<MapEntities>()
        .filter(|m| m.scale > 0.0 && logic_family(&m.entities) == Some(FAMILY))
        .cloned();
    let restarts = world.get_resource::<RoundRestarts>().map_or(0, |r| r.0);
    let built = world.get_resource::<GoldSrcLogic>().map(|l| (l.source.clone(), l.restarts));
    match (current, built) {
        (None, Some(_)) => {
            world.write_message(SoundControl::StopAll);
            world.remove_resource::<GoldSrcLogic>();
        }
        (Some(m), b) if b.as_ref().is_none_or(|(s, r)| !Arc::ptr_eq(s, &m.entities) || *r != restarts) => {
            world.write_message(SoundControl::StopAll);
            let logic = Logic::new(&m.entities, 0x5eed_601d);
            let nodes: Vec<(Id, Entity)> = world
                .query::<(Entity, &MapBrushEntity)>()
                .iter(world)
                .map(|(e, n)| (n.0, e))
                .filter(|(i, _)| *i < logic.ents.len())
                .collect();
            let mut l = GoldSrcLogic {
                seen: vec![None; nodes.len()],
                world: logic,
                scale: m.scale,
                nodes,
                source: m.entities.clone(),
                restarts,
            };
            // The looped sounds that play from the start.
            for (key, path, at, volume) in l.world.ambient_loops() {
                start_loop(world, key, path, at, None, volume, l.scale);
            }
            sync_nodes(world, &mut l);
            world.insert_resource(l);
        }
        _ => {}
    }
}

/// The static world as the logic's `Solid` (engine-space brushes).
pub(super) struct WorldSolid<'a> {
    pub brushes: &'a [MapBrush],
    pub scale: f32,
}

impl Solid for WorldSolid<'_> {
    fn blocked(&self, centre: Vec3, half: Vec3) -> bool {
        let c = entity_to_engine(centre, self.scale);
        let h = Vec3::new(half.x, half.z, half.y).max(Vec3::ZERO) * self.scale;
        self.brushes.iter().any(|b| !b.ladder && b.overlaps_box(c, h, 0.0))
    }
}

/// What a character is to the logic (`Player`).
pub(super) type CharacterQuery<'a> = (
    Entity,
    &'a Transform,
    &'a Intent,
    Option<&'a Health>,
    Option<&'a ColliderAabb>,
    Option<&'a Velocity>,
    Option<&'a MovementState>,
);

/// A character as a logic player: its box the movement's hull when it
/// has one, else its collider's size about its position (the same on a
/// network client's carry, `net`, whose replays run no physics).
pub(super) fn player_of(
    (e, t, intent, health, aabb, v, state): bevy::ecs::query::QueryItem<'_, '_, CharacterQuery<'static>>,
    scale: f32,
) -> Player {
    let (lo, hi) = match (state.filter(|s| s.hull_min != s.hull_max), aabb) {
        (Some(s), _) => (s.hull_min, s.hull_max),
        // Its size about its position: the box itself is the physics'
        // as of its last step (a client's replays run none).
        (None, Some(a)) => ((a.min - a.max) / 2.0, (a.max - a.min) / 2.0),
        (None, None) => (Vec3::ZERO, Vec3::ZERO),
    };
    let (mins, maxs) = if lo == hi {
        (Vec3::new(-16.0, -16.0, -36.0), Vec3::new(16.0, 16.0, 36.0))
    } else {
        let (a, b) = (engine_to_entity(lo, scale), engine_to_entity(hi, scale));
        (a.min(b), a.max(b))
    };
    let eye = state.map_or(Vec3::ZERO, |s| engine_to_entity(s.eye_offset, scale));
    Player {
        id: e.to_bits(),
        origin: engine_to_entity(t.translation, scale),
        mins,
        maxs,
        velocity: v.map_or(Vec3::ZERO, |v| engine_to_entity(v.0, scale)),
        view: Vec3::new(-intent.pitch.to_degrees(), intent.yaw.to_degrees() + 90.0, 0.0),
        eye: if eye == Vec3::ZERO { Vec3::new(0.0, 0.0, 28.0) } else { eye },
        use_key: intent.use_key,
        alive: health.is_none_or(|h| h.current > 0.0),
        on_ground: state.is_some_and(|s| s.on_ground),
        ..Default::default()
    }
}

/// The characters as logic players.
fn snapshot(world: &mut World, scale: f32) -> Vec<Player> {
    let mut q = world.query::<CharacterQuery>();
    let mut out: Vec<Player> = q.iter(world).map(|row| player_of(row, scale)).collect();
    out.sort_by_key(|p| p.id);
    out
}

/// What the logic changed on players, onto the characters.
pub(super) fn write_back(world: &mut World, scale: f32, players: &[Player]) {
    for p in players {
        let Ok(mut e) = world.get_entity_mut(Entity::from_bits(p.id)) else { continue };
        if p.moved
            && let Some(mut t) = e.get_mut::<Transform>()
        {
            t.translation = entity_to_engine(p.origin, scale);
        }
        if p.velocity_set
            && let Some(mut v) = e.get_mut::<Velocity>()
        {
            v.0 = entity_to_engine(p.velocity, scale);
        }
        if p.view_set
            && let Some(mut i) = e.get_mut::<Intent>()
        {
            i.yaw = (p.view.y - 90.0).to_radians().rem_euclid(std::f32::consts::TAU);
            i.pitch = (-p.view.x).to_radians().clamp(-89f32.to_radians(), 89f32.to_radians());
        }
        if p.base_velocity != Vec3::ZERO || p.teleported {
            e.insert(BaseVelocity {
                velocity: entity_to_engine(p.base_velocity, scale),
                touched: p.base_velocity != Vec3::ZERO,
                unground: p.teleported,
            });
        }
    }
}

/// One logic step.
fn step(
    world: &mut World,
    mut fires: Local<MessageCursor<LogicFire>>,
    mut outputs: Local<MessageCursor<FireEntityOutput>>,
) {
    let Some(mut l) = world.remove_resource::<GoldSrcLogic>() else {
        // Keep the cursors at the end while no GoldSrc map runs.
        if let Some(m) = world.get_resource::<Messages<LogicFire>>() {
            fires.read(m).for_each(drop);
        }
        if let Some(m) = world.get_resource::<Messages<FireEntityOutput>>() {
            outputs.read(m).for_each(drop);
        }
        return;
    };
    let names: Vec<String> = world
        .get_resource::<Messages<LogicFire>>()
        .map(|m| fires.read(m).map(|f| f.0.clone()).collect())
        .unwrap_or_default();
    let asked: Vec<(usize, Option<Entity>)> = world
        .get_resource::<Messages<FireEntityOutput>>()
        .map(|m| outputs.read(m).map(|f| (f.map_index, f.activator)).collect())
        .unwrap_or_default();
    for name in names {
        l.world.fire(&name, None, None, UseType::Toggle);
    }
    for (index, activator) in asked {
        l.world.fire_entity(index, activator.map(|a| Who::Player(a.to_bits())));
    }
    let scale = l.scale;
    let mut players = snapshot(world, scale);
    // The tick's length as a network client steps the movers (`net`).
    let dt = world
        .get_resource::<crate::core::SimClock>()
        .map_or_else(|| world.resource::<Time<Fixed>>().timestep(), |c| c.delta)
        .as_secs_f64();
    {
        let empty = Vec::new();
        let brushes = world.get_resource::<MapBrushes>().map_or(&empty, |b| &b.0);
        let solid = WorldSolid { brushes, scale };
        l.world.tick(dt, &mut players, &solid);
    }
    write_back(world, scale, &players);
    sync_nodes(world, &mut l);
    let effects = std::mem::take(&mut l.world.effects);
    apply_effects(world, &l, effects);
    // Switchable light styles (lit unless their pattern is dark).
    let styles: Vec<(u8, bool)> = l.world.styles.iter().map(|(s, p)| (*s, p != "a")).collect();
    let current = world.get_resource::<LightStyles>().map(|s| s.styles.clone());
    if !styles.is_empty() && current.as_ref() != Some(&styles) {
        world.insert_resource(LightStyles::of("goldsrc", styles));
    }
    world.insert_resource(l);
}

/// Place, show, hide and tint the mover nodes as the logic says.
fn sync_nodes(world: &mut World, l: &mut GoldSrcLogic) {
    let scale = l.scale;
    for (k, &(id, node)) in l.nodes.iter().enumerate() {
        let e = &l.world.ents[id];
        let look = if e.render_changed {
            RenderLook {
                color: e.render.color.map(|c| c.clamp(0.0, 255.0) as u8),
                alpha: e.render.amount.clamp(0.0, 255.0) as u8,
                mode: e.render.mode.clamp(0, 255) as u8,
            }
        } else {
            RenderLook::default()
        };
        let now = Seen {
            origin: e.origin,
            angles: e.angles,
            shown: !e.removed && e.visible,
            solid: !e.removed && e.solid,
            tint: BrushTint(look, e.frame),
        };
        let before = l.seen[k];
        if before == Some(now) {
            continue;
        }
        l.seen[k] = Some(now);
        let damageable = !e.removed
            && match &e.kind {
                super::world::Kind::Breakable(_) | super::world::Kind::Pushable(Some(_)) => e.flags & 1 == 0,
                super::world::Kind::Button(b) => b.shootable,
                _ => false,
            };
        let velocity = entity_to_engine(e.motion.velocity, scale);
        let moved = before.is_none_or(|b| b.origin != now.origin || b.angles != now.angles);
        place_node(world, node, (now.origin, now.angles), now.shown, now.solid, &e.volumes, velocity, scale, moved);
        let Ok(mut n) = world.get_entity_mut(node) else { continue };
        if before.is_none_or(|b| b.tint != now.tint) && (before.is_some() || !now.tint.is_white()) {
            n.insert(now.tint);
        }
        if damageable {
            n.insert(Damageable);
        } else {
            n.remove::<Damageable>();
        }
    }
}

/// Place mover node `node` at `pose` (entity-space origin and angles;
/// its transform written only when `moved`), shown or hidden, solid or
/// not (its collider and `core::MovingSolid`: `volumes` placed there,
/// moving at `velocity`, engine space). The server's logic and a network
/// client's copy (`net`) both.
#[allow(clippy::too_many_arguments)]
pub(super) fn place_node(
    world: &mut World,
    node: Entity,
    pose: (Vec3, Vec3),
    shown: bool,
    solid: bool,
    volumes: &[super::world::Volume],
    velocity: Vec3,
    scale: f32,
    moved: bool,
) {
    let (origin, angles) = pose;
    let rot = entity_rotation(angles);
    let brushes: Vec<MapBrush> = if solid {
        volumes.iter().map(|v| to_engine(&super::world::place(v, rot, origin), scale)).collect()
    } else {
        Vec::new()
    };
    let Ok(mut n) = world.get_entity_mut(node) else { return };
    if moved {
        let (at, turn) = (entity_to_engine(origin, scale), rotation_to_engine(rot));
        if let Some(mut t) = n.get_mut::<Transform>() {
            t.translation = at;
            t.rotation = turn;
        }
        // The physics' copy too: what spatial queries (the movement's
        // ground) see before the next physics step, which a network
        // client's replays don't run.
        if let Some(mut p) = n.get_mut::<avian3d::prelude::Position>() {
            p.0 = at;
        }
        if let Some(mut r) = n.get_mut::<avian3d::prelude::Rotation>() {
            r.0 = turn;
        }
    }
    set_shown(&mut n, shown);
    if solid {
        if n.contains::<ColliderDisabled>() {
            n.remove::<ColliderDisabled>();
        }
    } else if !n.contains::<ColliderDisabled>() {
        n.insert(ColliderDisabled);
    }
    let same = !moved && n.get::<MovingSolid>().is_some_and(|m| m.solid == solid && m.velocity == velocity);
    if !same {
        n.insert(MovingSolid { brushes, velocity, solid });
    }
}

/// An entity-space brush in engine space.
fn to_engine(b: &MapBrush, scale: f32) -> MapBrush {
    let planes = b.planes.iter().map(|(n, d)| (Vec3::new(n.x, n.z, -n.y), d * scale)).collect();
    let (a, c) = (entity_to_engine(b.min, scale), entity_to_engine(b.max, scale));
    MapBrush { planes, min: a.min(c), max: a.max(c), ladder: false, surface: None }
}

/// Show or hide a node (`LogicHidden`), leaving visibility culling's
/// choice when shown.
fn set_shown(e: &mut EntityWorldMut, visible: bool) {
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

fn node_of(l: &GoldSrcLogic, id: Id) -> Option<Entity> {
    l.nodes.iter().find(|(i, _)| *i == id).map(|(_, n)| *n)
}

#[allow(clippy::too_many_arguments)]
fn start_loop(world: &mut World, key: u64, path: String, at: Vec3, follow: Option<Entity>, volume: f32, scale: f32) {
    world.write_message(SoundControl::Start(StartSound {
        key: SoundKey(1 << 63 | key),
        entry: path,
        at: Some(entity_to_engine(at, scale)),
        follow,
        volume: Some(volume),
        pitch: None,
        level: None,
        clip: None,
        looping: true,
        dry: false,
    }));
}

fn apply_effects(world: &mut World, l: &GoldSrcLogic, effects: Vec<Effect>) {
    let scale = l.scale;
    for fx in effects {
        match fx {
            Effect::Sound { path, at, entity, volume } => {
                let mut s = PlaySound::at(path, entity_to_engine(at, scale));
                s.volume = Some(volume);
                s.source = entity.and_then(|i| node_of(l, i));
                world.write_message(s);
            }
            Effect::Loop { key, path: Some(path), at, entity, volume } => {
                let follow = entity.and_then(|i| node_of(l, i));
                start_loop(world, key, path, at, follow, volume, scale);
            }
            Effect::Loop { key, path: None, .. } => {
                world.write_message(SoundControl::Stop(SoundKey(1 << 63 | key)));
            }
            Effect::Damage { player, points, kind, by } => {
                let target = Entity::from_bits(player);
                if points < 0.0 {
                    if let Some(mut h) = world.get_mut::<Health>(target)
                        && h.current > 0.0
                    {
                        h.current = (h.current - points / 100.0).min(h.max);
                    }
                    continue;
                }
                let point = world.get::<Transform>(target).map_or(Vec3::ZERO, |t| t.translation);
                world.write_message(MapDamage {
                    damage: Damage {
                        target,
                        attacker: None,
                        amount: points / 100.0,
                        point,
                        dir: Vec3::NEG_Y,
                        hitgroup: Hitgroup::Generic,
                        kind,
                        weapon: None,
                        force: Vec3::ZERO,
                    },
                    armour: !matches!(kind, DamageKind::Fall | DamageKind::Drown),
                });
                let _ = by;
            }
            Effect::Explosion { at, magnitude, by } => {
                world.write_message(Explosion {
                    origin: entity_to_engine(at, scale),
                    damage: magnitude / 100.0,
                    radius: magnitude * 2.5 * scale,
                    attacker: None,
                    inflictor: node_of(l, by),
                    sound: None,
                    weapon: None,
                    round_scorch: false,
                    bell_sigma: None,
                    armor_ratio: None,
                    through_walls: false,
                    flags: Default::default(),
                    kind: DamageKind::Blast,
                });
            }
            Effect::Broke { .. } => {}
            Effect::Message { player, text } => {
                bevy::log::info!("map message to {:?}: {text}", Entity::from_bits(player));
            }
        }
    }
}

/// Weapons' damage on breakable and shootable nodes.
fn damage(world: &mut World, mut cursor: Local<MessageCursor<Damage>>) {
    let hits: Vec<Damage> = match world.get_resource::<Messages<Damage>>() {
        Some(m) => cursor.read(m).cloned().collect(),
        None => return,
    };
    let Some(mut l) = world.remove_resource::<GoldSrcLogic>() else { return };
    let scale = l.scale;
    for d in hits {
        let Some(id) = l.nodes.iter().find(|(_, n)| *n == d.target).map(|(i, _)| *i) else { continue };
        let from = engine_to_entity(d.point - d.dir, scale);
        l.world.damage(id, d.amount * 100.0, d.kind, from);
    }
    sync_nodes(world, &mut l);
    let effects = std::mem::take(&mut l.world.effects);
    apply_effects(world, &l, effects);
    world.insert_resource(l);
}
