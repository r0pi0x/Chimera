//! Combat Arms' world objects (specs/combat_arms/world_objects.md): the
//! map's moving parts and its logic. `game::world` gives every world
//! model that can move, hide or break a node of its own (a mover map
//! entity: drawn and collided under Chimera's `MapBrushEntity`, placed at
//! its BSP's centre, 1) and passes the logic objects through as map
//! entities with their properties (triggers, command and touch triggers,
//! event counters, start-up commands, keyframers with their keys' blind
//! data, explosions, sounds, lights, cameras, points); `runtime` is the
//! object world they make, in the world file's frame (`geometry`):
//!
//! - `movers`: the active world models' machine: sliding, rotating and
//!   spinning motion with pivots and waveforms, waits, auto-close, locks,
//!   reversal, blocking without pushing or crushing, doors and switches
//!   (use to open, links, `TRIGGERCLOSE`), riders carried;
//! - `keyframer`: keys from blind data, time and constant-speed modes,
//!   straight and Bézier segments, rotations blended, riders carried;
//! - `runtime`: breakables (damage, death at 1 HP or less, round reset
//!   through `MEMORIZE`/`REMEMBER`), triggers (delays, counts, weighted,
//!   timed), command and touch triggers, target triggers, event counters,
//!   start-up commands, the use triggers (`ActivateTrigger`), explosions;
//! - `events`: the event objects (timers, hold-to-use spots, captions,
//!   messages, the objective list, cameras and their points, target
//!   triggers' markers), whose notices to the clients are `CaObjectNotice`;
//! - `markers`: the markers' icons from the install's trigger resource
//!   table, by name (`CaMarkers`: what the local player is shown; the
//!   radar and map icons, the HUD's guide markers);
//! - `commands`: the command-string language and its manager (variables,
//!   `when`, delays by real frame time, 512 slots, a failure stops the
//!   rest of its string).
//!
//! The bridge (`CaObjectsPlugin`) runs the object world on the server
//! (authoritative) at the server's 20 Hz frames (`grenades::SERVER_FRAME`;
//! the first tick at or after each frame's time), in the spec's order:
//! players' touches and use keys (the use ray, `inventory::
//! ACTIVATION_DISTANCE`, on a node), damage to nodes (`core::Damage` at a
//! node, `Damageable`: bullets and blasts), the rules' messages
//! (`CaObjectMessage`), the command manager, then each object's update.
//! It then writes what changed: nodes' transforms (and their bodies'
//! poses), visibility and collision layers (solid: everything; ray-hit
//! only: traces; neither: nothing), volumes' state and offset (`volumes::
//! CaVolume`, carried by `Attachments`: HGOILRIG's rising sea), carried
//! characters, explosions'
//! blasts (`grenades::falloff` from a quarter of the radius, a clear line
//! needed) and the messages for the rules (`CaObjectEvent`: mode objects
//! and controllers). Clients run no map logic: they get the objects that
//! differ from the map's start (`net`, late joiners too) and draw them.
//! The objects' sounds (14: transition sounds looping or not, keyframers'
//! active and key sounds, triggers' activation sounds) are state in the
//! object world that the server sends with the objects; `sound` plays
//! them on every machine, following the object (its node, or an anchor
//! entity at it) when attached.
//!
//! Debugging (docs/OBSERVABILITY.md): `ca_objects [filter]` lists objects
//! and their state, `ca_object_msg <name> <message...>` sends a message
//! now, `ca_object_props <name>` prints an object's stored properties and
//! a `setpos` line at it, `ca_object_damage <name> [hp]` damages (breaks)
//! it, `ca_object_cmd <command string>` runs a command string,
//! `ca_object_vars` lists the variables and pending commands.
//!
//! A network client steps the active world models and the running
//! keyframers itself to the tick it predicts and carries its own player
//! on them (`net`, 10's Client).
//!
//! Known shortcuts: the explosion's effect isn't drawn and its damage skips the armour
//! step; a destructible's weapon (`CreateExplosion`, `FIRE`) is only a
//! `CaObjectFire` message (no stock map has one: its weapon id isn't
//! turned into a shot); riders are carried without the spec's 0.1 u drop (by active
//! world models and by the solid world models keyframers move); nodes
//! collide as their polygons' triangles (the engine's own `BoxPhysics`
//! boxes aren't used) while the game's tests use their boxes (`movers`).

pub mod commands;
pub mod events;
pub mod geometry;
pub mod keyframer;
pub mod movers;
pub mod net;
pub mod lights;
pub mod markers;
pub mod runtime;
pub mod sound;

use std::{collections::HashMap, sync::Arc};

use avian3d::prelude::*;
use bevy::{
    ecs::{message::MessageCursor, system::SystemState},
    prelude::*,
};
use lithtech_formats::world as lt;

use self::{
    commands::{Ctx, ObjId, Who},
    geometry::{from_engine, rotation_to_engine, to_engine},
    movers::CharBox,
    runtime::{Fx, Kind, ObjectWorld},
};
use super::{inventory, volumes::CaVolume};
use crate::{
    console::ConsoleAppExt,
    core::{
        Bot, Damage, DamageKind, Damageable, Dead, Health, Hitgroup, Intent, MovementState, Npc, SimClock, SimSet,
        authoritative,
    },
    map::{MapBrushEntity, MapEntities, MapEntity},
};

/// Classes `game::world` passes through for the object world (besides
/// world models, volumes and the modes' objects).
pub const LOGIC_CLASSES: &[&str] = &[
    "Trigger",
    "TargetTrigger",
    "CommandTrigger",
    "TouchTrigger",
    "EventCounter",
    "StartupCommand",
    "KeyFramer",
    "Explosion",
    "SoundFX",
    "BGMTrigger",
    "SpecialFX",
    "LightGroup",
    "Camera",
    "CameraPoint",
    "Point",
    "TimerTrigger",
    "LightActivateTrigger",
    "CaptionTrigger",
    "MessageTrigger",
    "MissionTrigger",
];

/// Whether `game::world` passes objects of `class` through for the object
/// world.
pub fn is_logic_object(class: &str) -> bool {
    LOGIC_CLASSES.contains(&class)
}

/// Whether a world-model object gets a node of its own: an active class,
/// or a `WorldModel` something can move, hide or break (keyframed,
/// starting hidden, destructible, attaching others, or `named` in another
/// object's command, attachment or keyframer).
pub fn is_node_object(o: &lt::Object, named: bool) -> bool {
    if runtime::ACTIVE_CLASSES.contains(&o.class.as_str()) {
        return true;
    }
    if o.class != "WorldModel" {
        return false;
    }
    named
        || o.bool("IsKeyframed") == Some(true)
        || o.bool("StartHidden") == Some(true)
        || o.bool("NeverDestroy") == Some(false)
        || o.string("Attachments").is_some_and(|a| !a.trim().is_empty())
}

/// Every word in `texts` that could name an object (lower case): split
/// on whitespace, `;`, parentheses and quotes.
pub fn names_in_commands<'a>(texts: impl Iterator<Item = &'a str>) -> std::collections::HashSet<String> {
    texts
        .flat_map(|t| t.split(|c: char| c.is_whitespace() || matches!(c, ';' | '(' | ')' | '"')))
        .filter(|w| !w.is_empty())
        .map(|w| w.to_ascii_lowercase())
        .collect()
}

/// Ask the object world to deliver `message` to `target` (a name, or a
/// group of names: `msg`'s targets) at the start of its next frame, as a
/// map command would (the rules: mission start and end triggers).
#[derive(Message, Clone, Debug, PartialEq)]
pub struct CaObjectMessage {
    pub target: String,
    pub message: String,
}

/// A message a map object of a rules class (mode objects, controllers:
/// `runtime::RULE_CLASSES`) received, for the rules.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct CaObjectEvent {
    pub class: String,
    pub name: String,
    pub message: String,
    pub sender: Option<Entity>,
}

/// What the server tells every client about an event object (a timer,
/// caption, message, objective, use spot or camera: `events::Notice`).
#[derive(Message, Clone, Debug, PartialEq)]
pub struct CaObjectNotice {
    pub object: ObjId,
    pub name: String,
    pub notice: events::Notice,
}

/// A destructible fires its weapon (world_objects.md Follow-up
/// (2026-10-31b) 5: `CreateExplosion` at its death, `FIRE`): the game's
/// weapon id, from `at` along `dir` (engine), going off in place when
/// `in_place`, damage x `factor`, by object `shooter`. No stock map has
/// one; nothing turns it into a projectile yet (Known shortcuts).
#[derive(Message, Clone, Debug, PartialEq)]
pub struct CaObjectFire {
    pub weapon: i32,
    pub at: Vec3,
    pub dir: Vec3,
    pub in_place: bool,
    pub factor: f32,
    pub shooter: ObjId,
}

/// A node's object (its map entity index).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaNode(pub ObjId);

/// A volume's object (its map entity index).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaVolumeObject(pub ObjId);

/// A volume as it stood at load (its planes and bounds, which an
/// attachment's moves shift).
#[derive(Component, Clone, Debug)]
pub struct VolumeAtLoad(pub CaVolume);

/// Where the bridge's systems run.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CaObjectsSet;

/// The object world and its links to the ECS.
#[derive(Resource, Default)]
pub struct CaObjects {
    pub world: ObjectWorld,
    /// The entities it was built from.
    pub from: Option<Arc<Vec<MapEntity>>>,
    /// Metres per world unit.
    pub scale: f32,
    /// Node entities by object id.
    pub nodes: HashMap<ObjId, Entity>,
    /// Volume entities by object id.
    pub volumes: HashMap<ObjId, Entity>,
    /// The next server frame's time.
    pub next_frame: f64,
    /// How many nodes the map has (bound when they all are).
    want_nodes: usize,
    /// Each object's pose and flags at the map's start (`net`).
    pub start: Vec<(movers::Pose, u8)>,
    prev_use: HashMap<Entity, bool>,
    damage: MessageCursor<Damage>,
    requests: MessageCursor<CaObjectMessage>,
    /// Since the last server frame (gathered every tick): use presses on
    /// nodes, damage to nodes, the rules' messages.
    pending_uses: Vec<(ObjId, Who)>,
    pending_damage: Vec<Damage>,
    pending_requests: Vec<CaObjectMessage>,
    /// A client: the last sound start the server had made when this
    /// client first heard of the objects (one-shots up to it already
    /// played: `net`).
    pub net_sound_floor: Option<u32>,
    /// A client: the objects whose pose its prediction places (the
    /// stepped active world models and what is attached to them: `net`).
    pub predicted: Vec<ObjId>,
}

impl CaObjects {
    /// Object `i`'s state at the map's start if it is a `LightGroup`.
    pub fn world_start_light(&self, i: usize) -> Option<Kind> {
        let o = self.world.objects.get(i)?;
        (o.class == "LightGroup").then(|| runtime::light_group_start(&o.props))
    }

    /// The character entity of a `Who`.
    pub fn entity(who: Who) -> Option<Entity> {
        match who {
            Who::Character(bits) => Entity::try_from_bits(bits),
            _ => None,
        }
    }

    /// The first object named `name`.
    pub fn named(&self, name: &str) -> Option<ObjId> {
        self.world.named_all(name).first().copied()
    }
}

/// The world objects: built when a map loads, run on the server.
pub struct CaObjectsPlugin;

impl Plugin for CaObjectsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CaObjects>()
            .add_message::<CaObjectMessage>()
            .add_message::<CaObjectEvent>()
            .add_message::<CaObjectNotice>()
            .add_message::<CaObjectFire>()
            .init_resource::<sound::CaObjectSounds>()
            .add_message::<super::view_model::attached::CaLoopSound>()
            .init_resource::<markers::CaMarkers>()
            .add_systems(Update, (sound::play.before(super::effects::sounds::loops), lights::show, markers::show))
            .add_systems(
                FixedUpdate,
                (load, (gather, frame).chain().run_if(authoritative), apply)
                    .chain()
                    .in_set(CaObjectsSet)
                    .in_set(SimSet::Weapons),
            )
            .console_command(
                "ca_objects",
                "ca_objects [filter]: the map's objects whose class or name contains the filter (all world models and \
                 logic objects without one): state, pose (u, the world file's axes), flags, hit points.",
                |w, a| Ok(Some(list(w, a.first().map(String::as_str)))),
            )
            .console_command(
                "ca_object_msg",
                "ca_object_msg <name> <message...>: deliver a message to the objects of that name now (the server's), \
                 as a map command's `msg` would, e.g. `ca_object_msg Door01 trigger`.",
                |w, a| {
                    let (target, message) = a.split_first().ok_or("usage: ca_object_msg <name> <message...>")?;
                    send(w, target, &message.join(" "));
                    let objects = w.resource::<CaObjects>();
                    let ids = objects.world.named_all(target);
                    if ids.is_empty() {
                        return Ok(Some(format!("no object named {target}")));
                    }
                    Ok(Some(ids.iter().map(|i| objects.world.describe(*i)).collect::<Vec<_>>().join("\n")))
                },
            )
            .console_command(
                "ca_object_props",
                "ca_object_props <name>: the objects of that name: their state, then every property as the map \
                 stores it (commands, triggers' targets, doors' times), and where to stand: a `setpos` line at \
                 the object's centre (the feet, inches, as `getpos` prints).",
                |w, a| {
                    let name = a.first().ok_or("usage: ca_object_props <name>")?;
                    Ok(Some(props(w, name)))
                },
            )
            .console_command(
                "ca_object_damage",
                "ca_object_damage <name> [hit points]: damage the objects of that name now (the server's; default \
                 enough to break them), from the local player's view direction (their death sends their debris).",
                |w, a| {
                    let name = a.first().ok_or("usage: ca_object_damage <name> [hit points]")?;
                    let hp = a.get(1).and_then(|h| h.parse::<f32>().ok()).unwrap_or(1.0e6);
                    let dir = w
                        .query_filtered::<&Intent, With<crate::core::LocalPlayer>>()
                        .iter(w)
                        .next()
                        .map_or(Vec3::Z, |i| from_engine(crate::weapon::aim_of(i.yaw, i.pitch) * Vec3::NEG_Z, 1.0).normalize_or_zero());
                    let ids = w.resource::<CaObjects>().world.named_all(name);
                    if ids.is_empty() {
                        return Ok(Some(format!("no object named {name}")));
                    }
                    {
                        let mut o = w.resource_mut::<CaObjects>();
                        for id in &ids {
                            o.world.hit_dir = dir;
                            o.world.damage(*id, hp, Who::Nobody, false);
                        }
                    }
                    apply_effects(w);
                    let o = w.resource::<CaObjects>();
                    Ok(Some(ids.iter().map(|i| o.world.describe(*i)).collect::<Vec<_>>().join("\n")))
                },
            )
            .console_command(
                "ca_object_cmd",
                "ca_object_cmd <command string>: run a command string now (no sender), e.g. `ca_object_cmd msg \
                 (A B) (visible 0); delay 2 (msg C on)`.",
                |w, a| {
                    run(w, &a.join(" "));
                    Ok(None)
                },
            )
            .console_command(
                "ca_object_vars",
                "ca_object_vars: the command manager's variables and pending delayed commands.",
                |w, _| {
                    let o = w.resource::<CaObjects>();
                    let mut lines: Vec<String> =
                        o.world.cmds.vars.iter().map(|(k, v)| format!("{k} = {v:?}")).collect();
                    lines.sort();
                    for (i, p) in o.world.cmds.pending.iter().enumerate() {
                        if let Some(p) = p {
                            lines.push(format!("[{i}] in {:.2} s {:?}: {}", p.delay, p.id, p.command));
                        }
                    }
                    Ok(Some(lines.join("\n")))
                },
            );
        net::register(app);
    }
}

/// Deliver `message` to the objects named `target` now (the console,
/// tests), then write what changed.
pub fn send(world: &mut World, target: &str, message: &str) {
    world.resource_scope(|w, mut o: Mut<CaObjects>| {
        o.world.chars = characters(w, o.scale);
        commands::message(&mut o.world, target, message, Ctx::default());
    });
    apply_effects(world);
}

/// Run a command string now (no sender), then write what changed.
pub fn run(world: &mut World, text: &str) {
    world.resource_scope(|w, mut o: Mut<CaObjects>| {
        o.world.chars = characters(w, o.scale);
        o.world.run(text, Ctx::default());
    });
    apply_effects(world);
}

/// `ca_objects`' listing.
pub fn list(world: &mut World, filter: Option<&str>) -> String {
    let o = world.resource::<CaObjects>();
    let f = filter.map(str::to_ascii_lowercase);
    let lines: Vec<String> = o
        .world
        .objects
        .iter()
        .enumerate()
        .filter(|(_, ob)| match &f {
            Some(f) => ob.class.to_ascii_lowercase().contains(f) || ob.name.to_ascii_lowercase().contains(f),
            None => !matches!(ob.kind, Kind::Plain),
        })
        .map(|(i, _)| o.world.describe(i as ObjId))
        .collect();
    if lines.is_empty() { "no objects".into() } else { lines.join("\n") }
}

/// `ca_object_props`' listing: each object named `name`, its state, its
/// centre as a `setpos` line (CS:S inches: engine metres / 0.0254, z up)
/// and its stored properties.
pub fn props(world: &World, name: &str) -> String {
    let o = world.resource::<CaObjects>();
    let ids = o.world.named_all(name);
    if ids.is_empty() {
        return format!("no object named {name}");
    }
    let mut lines = Vec::new();
    for id in ids {
        lines.push(o.world.describe(id));
        let e = to_engine(o.world.objects[id as usize].pose.pos, o.scale) / 0.0254;
        lines.push(format!("  centre: setpos {:.0} {:.0} {:.0}", e.x, -e.z, e.y));
        for (k, v) in &o.world.objects[id as usize].props.0 {
            lines.push(format!("  {k} = {v}"));
        }
    }
    lines.join("\n")
}

/// The characters as the objects see them (file frame, u).
fn characters(world: &mut World, scale: f32) -> Vec<CharBox> {
    if scale <= 0.0 {
        return Vec::new();
    }
    type Row = (
        Entity,
        &'static Transform,
        &'static MovementState,
        Option<&'static Health>,
        Has<Bot>,
        Has<Npc>,
        Has<Dead>,
        &'static Intent,
    );
    chimera_core::query_cache::cached_query::<Row, With<Intent>, _>(world, |q, world| {
        q.iter(world)
            .map(|(e, t, s, h, bot, npc, dead, intent)| {
                char_box(e, t.translation, s, h, !bot && !npc, dead, intent.use_key, scale)
            })
            .collect()
    })
}

/// A character's box for the objects (file frame, u) with its centre at
/// `at` (engine m).
#[allow(clippy::too_many_arguments)]
fn char_box(
    e: Entity,
    at: Vec3,
    s: &MovementState,
    h: Option<&Health>,
    player: bool,
    dead: bool,
    using: bool,
    scale: f32,
) -> CharBox {
    let (lo, hi) = if s.hull_max != s.hull_min {
        (s.hull_min, s.hull_max)
    } else {
        (Vec3::new(-0.4, -0.9, -0.4), Vec3::new(0.4, 0.9, 0.4))
    };
    let a = from_engine(at + lo, scale);
    let b = from_engine(at + hi, scale);
    CharBox {
        who: Who::Character(e.to_bits()),
        min: a.min(b),
        max: a.max(b),
        player,
        alive: !dead && h.is_none_or(|h| h.current > 0.0),
        using,
    }
}

/// The world model nodes' and volumes' entities, by object id.
fn bind(world: &mut World) {
    let nodes: Vec<(Entity, usize)> =
        world.query::<(Entity, &MapBrushEntity)>().iter(world).map(|(e, n)| (e, n.0)).collect();
    let volumes: Vec<(Entity, usize, CaVolume)> = world
        .query::<(Entity, &super::volumes::VolumeIndex, &CaVolume)>()
        .iter(world)
        .map(|(e, i, v)| (e, i.0, v.clone()))
        .collect();
    let mut o = world.resource_mut::<CaObjects>();
    // Only Combat Arms' world models: another game's mover nodes (a
    // mashup's map) are its own logic's.
    let nodes: Vec<(Entity, usize)> =
        nodes.into_iter().filter(|(_, i)| o.world.objects.get(*i).is_some_and(|ob| ob.node)).collect();
    o.nodes = nodes.iter().map(|(e, i)| (*i as ObjId, *e)).collect();
    o.volumes = volumes.iter().map(|(e, i, _)| (*i as ObjId, *e)).collect();
    let entities = world.get_resource::<MapEntities>().cloned();
    for (e, i) in nodes {
        // Its polygons as one triangle mesh (a world model needn't be
        // convex: HELLGATE's rooms are world models).
        let mesh = entities.as_ref().and_then(|m| {
            let mut points = Vec::new();
            let mut tris = Vec::new();
            for h in &m.entities.get(i)?.hulls {
                let base = points.len() as u32;
                points.extend(h.points.iter().map(|p| crate::map::entities::entity_to_engine(*p, m.scale)));
                for k in 1..h.points.len().saturating_sub(1) as u32 {
                    tris.push([base, base + k, base + k + 1]);
                }
            }
            (!tris.is_empty()).then(|| Collider::trimesh(points, tris))
        });
        let mut ent = world.entity_mut(e);
        ent.insert((CaNode(i as ObjId), Damageable));
        if let Some(c) = mesh {
            ent.insert((
                c,
                RigidBody::Kinematic,
                crate::core::MapBrushCollider,
                crate::map::contact_filter::hooks(),
            ));
        }
    }
    for (e, i, v) in volumes {
        world.entity_mut(e).insert((CaVolumeObject(i as ObjId), VolumeAtLoad(v)));
    }
}

/// The object world of a newly loaded map (on every side: a client keeps
/// it for the objects' starting state).
fn load(world: &mut World) {
    let Some(entities) = world.get_resource::<MapEntities>().cloned() else {
        if world.resource::<CaObjects>().from.is_some() {
            *world.resource_mut::<CaObjects>() = CaObjects::default();
        }
        return;
    };
    let same = world
        .resource::<CaObjects>()
        .from
        .as_ref()
        .is_some_and(|f| Arc::ptr_eq(f, &entities.entities));
    if same {
        // Nodes spawn with the map; bind them once they're there.
        let o = world.resource::<CaObjects>();
        if o.nodes.len() < o.want_nodes && world.query::<&MapBrushEntity>().iter(world).next().is_some() {
            bind(world);
            mark_all(world);
        }
        return;
    }
    let scale = entities.scale;
    let mut w = object_world(&entities.entities);
    let now = world.resource::<SimClock>().now;
    let log = std::mem::take(&mut w.log);
    for l in log {
        warn!("objects: {l}");
    }
    let start = w.objects.iter().map(|o| (o.pose, net::flags_of(o))).collect();
    let want_nodes = w.objects.iter().filter(|o| o.node).count();
    *world.resource_mut::<CaObjects>() = CaObjects {
        want_nodes,
        start,
        world: w,
        from: Some(entities.entities.clone()),
        scale,
        next_frame: now,
        ..default()
    };
    bind(world);
    mark_all(world);
    apply(world);
}

/// The object world of a map's entities (`MapData::entities`): world
/// models' nodes at their centres, volumes at their boxes' centres and
/// switched as their `Hidden` says.
pub fn object_world(entities: &[MapEntity]) -> ObjectWorld {
    let list: Vec<runtime::EntityProps> = entities
        .iter()
        .map(|e| {
            let centre = if e.mover {
                Some(Vec3::new(e.origin().x, e.origin().z, e.origin().y))
            } else if super::volumes::is_volume(e.classname()) {
                volume_centre(e)
            } else {
                None
            };
            (e.keyvalues.clone(), centre)
        })
        .collect();
    let mut w = ObjectWorld::build(&list);
    for (i, o) in w.objects.iter_mut().enumerate() {
        if super::volumes::is_volume(&o.class) {
            o.visible = !entities[i].get("Hidden").is_some_and(|h| h == "1");
            o.kind = Kind::Volume;
        }
    }
    w
}

/// Every node and volume to be written (a fresh map).
fn mark_all(world: &mut World) {
    for o in &mut world.resource_mut::<CaObjects>().world.objects {
        o.dirty = true;
    }
}

/// A volume entity's box centre (file frame).
fn volume_centre(e: &MapEntity) -> Option<Vec3> {
    let points = &e.hulls.first()?.points;
    let (lo, hi) = points.iter().fold((Vec3::MAX, Vec3::MIN), |(a, b), p| (a.min(*p), b.max(*p)));
    let c = (lo + hi) / 2.0;
    // Entity axes (x, z, y of the file's).
    Some(Vec3::new(c.x, c.z, c.y))
}

/// The server frame, when one is due: touches, uses, damage, requests,
/// commands and updates, then what changed.
fn frame(world: &mut World) {
    let now = world.resource::<SimClock>().now;
    {
        let o = world.resource::<CaObjects>();
        if o.from.is_none() || now + 1e-9 < o.next_frame {
            return;
        }
    }
    let scale = world.resource::<CaObjects>().scale;
    let chars = characters(world, scale);
    let damage = node_damage(world, &chars);
    let (uses, requests) = {
        let mut o = world.resource_mut::<CaObjects>();
        (std::mem::take(&mut o.pending_uses), std::mem::take(&mut o.pending_requests))
    };
    let mut o = world.resource_mut::<CaObjects>();
    let at = o.next_frame;
    o.next_frame = at + super::grenades::SERVER_FRAME;
    // A long stall: no catching up frame by frame.
    if o.next_frame < now {
        o.next_frame = now + super::grenades::SERVER_FRAME;
    }
    let w = &mut o.world;
    w.chars = chars;
    w.now = at;
    w.dt = super::grenades::SERVER_FRAME as f32;
    w.touches();
    w.use_triggers();
    w.ai_doors();
    for (id, who) in uses {
        w.use_object(id, who);
    }
    for (id, amount, attacker, player, dir) in damage {
        w.hit_dir = if dir == Vec3::ZERO { Vec3::Z } else { dir };
        w.damage(id, amount, attacker, player);
    }
    for r in requests {
        commands::message(w, &r.target, &r.message, Ctx::default());
    }
    w.frame(at, super::grenades::SERVER_FRAME as f32);
    apply_effects(world);
}

/// Every tick: what the next server frame takes (presses of the use key
/// on nodes, damage messages, the rules' messages), so none is missed
/// between frames.
fn gather(world: &mut World) {
    if world.resource::<CaObjects>().from.is_none() {
        return;
    }
    let uses = use_rays(world);
    let (damage, requests) = world.resource_scope(|w, mut o: Mut<CaObjects>| {
        let d: Vec<Damage> = o.damage.read(w.resource::<Messages<Damage>>()).cloned().collect();
        let r: Vec<CaObjectMessage> = o.requests.read(w.resource::<Messages<CaObjectMessage>>()).cloned().collect();
        (d, r)
    });
    let mut o = world.resource_mut::<CaObjects>();
    o.pending_uses.extend(uses);
    o.pending_damage.extend(damage);
    o.pending_requests.extend(requests);
}

/// The nodes characters' use rays hit on a fresh press of the use key.
fn use_rays(world: &mut World) -> Vec<(ObjId, Who)> {
    let pressed: Vec<(Entity, Vec3, Vec3)> = {
        let rows: Vec<_> =
            chimera_core::query_cache::cached_query::<(Entity, &Transform, &Intent, &MovementState), (), _>(world, |q, world| {
                q.iter(world).map(|(e, t, i, s)| (e, t.translation + s.eye_offset, i.clone())).collect()
            });
        let mut o = world.resource_mut::<CaObjects>();
        rows.into_iter()
            .filter_map(|(e, eye, i)| {
                let before = o.prev_use.insert(e, i.use_key).unwrap_or(false);
                (i.use_key && !before).then(|| (e, eye, crate::weapon::aim_of(i.yaw, i.pitch) * Vec3::NEG_Z))
            })
            .collect()
    };
    if pressed.is_empty() {
        return Vec::new();
    }
    let alive = |w: &World, e: Entity| {
        w.get::<Dead>(e).is_none() && w.get::<Health>(e).is_none_or(|h| h.current > 0.0)
    };
    let pressed: Vec<_> = pressed.into_iter().filter(|(e, ..)| alive(world, *e)).collect();
    let mut st: SystemState<(SpatialQuery, Query<&CaNode>, Query<&ColliderOf>)> = SystemState::new(world);
    let Ok((spatial, nodes, colliders)) = st.get(world) else { return Vec::new() };
    let mut out = Vec::new();
    for (e, eye, dir) in pressed {
        let Ok(d) = Dir3::new(dir) else { continue };
        let filter = SpatialQueryFilter::from_excluded_entities([e]).with_mask(crate::core::SOLID_LAYERS);
        let Some(hit) = spatial.cast_ray(eye, d, inventory::ACTIVATION_DISTANCE + 1e-4, true, &filter) else {
            continue;
        };
        let body = colliders.get(hit.entity).map_or(hit.entity, |c| c.body);
        if let Ok(n) = nodes.get(body).or_else(|_| nodes.get(hit.entity)) {
            out.push((n.0, Who::Character(e.to_bits())));
        }
    }
    out
}

/// This frame's damage to nodes: (object, hit points, attacker, a player
/// attacker, its direction in the file frame).
fn node_damage(world: &mut World, chars: &[CharBox]) -> Vec<(ObjId, f32, Who, bool, Vec3)> {
    let hits = std::mem::take(&mut world.resource_mut::<CaObjects>().pending_damage);
    hits.into_iter()
        .filter_map(|d| {
            let id = world.get::<CaNode>(d.target)?.0;
            let who = d.attacker.map_or(Who::Nobody, |a| Who::Character(a.to_bits()));
            let player = chars.iter().any(|c| c.who == who && c.player);
            Some((id, d.amount / inventory::HP, who, player, from_engine(d.dir, 1.0).normalize_or_zero()))
        })
        .collect()
}

/// Write what the objects changed (nodes, volumes) and their effects.
fn apply(world: &mut World) {
    apply_effects(world);
}

/// Nodes' and volumes' state from the object world, and its effects.
pub fn apply_effects(world: &mut World) {
    // Effects may cause more (an explosion breaking a breakable).
    for _ in 0..8 {
        let (fx, dirty, scale) = {
            let mut o = world.resource_mut::<CaObjects>();
            let fx = std::mem::take(&mut o.world.fx);
            let dirty: Vec<ObjId> = o
                .world
                .objects
                .iter_mut()
                .enumerate()
                .filter_map(|(i, ob)| std::mem::take(&mut ob.dirty).then_some(i as ObjId))
                .collect();
            (fx, dirty, o.scale)
        };
        if fx.is_empty() && dirty.is_empty() {
            return;
        }
        write_objects(world, &dirty, scale);
        for f in fx {
            effect(world, f, scale);
        }
    }
}

/// The collision layers of a node: solid to everything, ray-hit for
/// traces only, else nothing.
fn node_layers(solid: bool, rayhit: bool) -> CollisionLayers {
    match (solid, rayhit) {
        (true, _) => CollisionLayers::new(LayerMask::DEFAULT, LayerMask::ALL & !crate::core::ITEM_LAYER),
        (false, true) => CollisionLayers::new(crate::core::TRACE_ONLY_LAYER, LayerMask::NONE),
        _ => CollisionLayers::new(LayerMask::NONE, LayerMask::NONE),
    }
}

/// Write objects' poses and flags to their nodes and volumes.
fn write_objects(world: &mut World, ids: &[ObjId], scale: f32) {
    for &id in ids {
        let (node, volume, ob) = {
            let o = world.resource::<CaObjects>();
            let ob = &o.world.objects[id as usize];
            (o.nodes.get(&id).copied(), o.volumes.get(&id).copied(), (ob.pose, ob.load_pose, ob.visible && !ob.removed, ob.solid && !ob.removed, ob.rayhit && !ob.removed))
        };
        let (pose, load, visible, solid, rayhit) = ob;
        if let Some(e) = node
            && let Ok(mut ent) = world.get_entity_mut(e)
        {
            let translation = to_engine(pose.pos, scale);
            let rotation = rotation_to_engine(pose.rot);
            if let Some(mut t) = ent.get_mut::<Transform>()
                && (t.translation != translation || t.rotation != rotation)
            {
                t.translation = translation;
                t.rotation = rotation;
            }
            if let Some(mut p) = ent.get_mut::<Position>() {
                p.0 = translation;
            }
            if let Some(mut r) = ent.get_mut::<Rotation>() {
                r.0 = rotation;
            }
            let shown = if visible { Visibility::Inherited } else { Visibility::Hidden };
            if ent.get::<Visibility>() != Some(&shown) {
                ent.insert(shown);
            }
            let layers = node_layers(solid, rayhit);
            if ent.get::<CollisionLayers>() != Some(&layers) {
                ent.insert(layers);
            }
        }
        if let Some(e) = volume
            && let Some(base) = world.get::<VolumeAtLoad>(e).map(|b| b.0.clone())
            && world.get::<CaVolume>(e).is_some()
        {
            let hidden = !visible;
            // Messages' changes (`DamageVal`, `VELOCITY`, `LOCK`).
            let mods = world.resource::<CaObjects>().world.objects[id as usize].vol;
            let mut v = world.get_mut::<CaVolume>(e).expect("checked above");
            if let Some(d) = mods.damage {
                v.damage = d;
            }
            if let Some(speed) = mods.speed
                && let Some(j) = v.jump_velocity
            {
                v.jump_velocity = Some(j.normalize_or_zero() * speed);
            }
            if let Some(l) = mods.locked {
                v.locked = l;
            }
            let offset = to_engine(pose.pos, scale) - to_engine(load.pos, scale);
            if v.hidden != hidden {
                v.hidden = hidden;
            }
            let min = base.min + offset;
            if v.min != min {
                v.min = min;
                v.max = base.max + offset;
                v.planes = base.planes.iter().map(|(n, d)| (*n, d + n.dot(offset))).collect();
            }
        }
    }
}

/// One effect.
fn effect(world: &mut World, f: Fx, scale: f32) {
    match f {
        // Sounds play from the objects' state on every machine (`sound`).
        Fx::Sound { .. } | Fx::StopSound { .. } => {}
        Fx::Music { id, on } => {
            let props = world.resource::<CaObjects>().world.objects[id as usize].props.clone();
            super::music::trigger::on_message(world, id, on, &props);
        }
        Fx::Carry { who, delta } => {
            let Some(e) = CaObjects::entity(who) else { return };
            let d = to_engine(delta, scale);
            if let Ok(mut ent) = world.get_entity_mut(e) {
                if let Some(mut t) = ent.get_mut::<Transform>() {
                    t.translation += d;
                }
                if let Some(mut p) = ent.get_mut::<Position>() {
                    p.0 += d;
                }
            }
        }
        Fx::Explode {
            at,
            radius,
            damage,
            attacker,
            ..
        } => explode(world, at, radius, damage, attacker, scale),
        Fx::ObjectMessage {
            class,
            name,
            message,
            sender,
            ..
        } => {
            world.write_message(CaObjectEvent {
                class,
                name,
                message,
                sender: CaObjects::entity(sender),
            });
        }
        Fx::CharacterMessage { who, message } => debug!("objects: to {who:?}: {message}"),
        Fx::Notice { id, notice } => {
            let name = world.resource::<CaObjects>().world.objects[id as usize].name.clone();
            info!("objects: {name}: {notice:?}");
            world.write_message(CaObjectNotice { object: id, name, notice });
        }
        Fx::Debris { at, debris, dir } => {
            if world.get_resource::<super::effects::debris::CaDebrisTable>().is_none() {
                let t = super::effects::debris::CaDebrisTable::install();
                world.insert_resource(t);
            }
            let table = world.resource::<super::effects::debris::CaDebrisTable>();
            let Some(record) = table.id(&debris) else {
                debug!("objects: no debris record {debris}");
                return;
            };
            let msg = super::effects::debris::CaDebris { at, record, dir };
            if world.get_resource::<crate::core::NetRole>() == Some(&crate::core::NetRole::Server) {
                world.write_message(bevy_replicon::prelude::ToClients {
                    targets: bevy_replicon::prelude::SendTargets::All,
                    message: msg,
                });
            } else {
                world.write_message(msg);
            }
        }
        Fx::Fire {
            weapon,
            at,
            dir,
            in_place,
            factor,
            shooter,
        } => {
            debug!("objects: object {shooter} fires weapon {weapon} along {dir} (in place {in_place}, x {factor})");
            world.write_message(CaObjectFire {
                weapon,
                at: to_engine(at, scale),
                dir: to_engine(dir, 1.0),
                in_place,
                factor,
                shooter,
            });
        }
        Fx::Log(l) => debug!("objects: {l}"),
    }
}

/// A map explosion (15, `Explosion`): characters within `radius` u with a
/// clear line from the centre take `damage` × the grenades' falloff;
/// world models within it are damaged the same (no line test).
fn explode(world: &mut World, at: Vec3, radius: f32, damage: f32, attacker: Who, scale: f32) {
    let centre = to_engine(at, scale);
    type Bodies<'w, 's> = Query<'w, 's, (Entity, &'static Transform, &'static MovementState, &'static Health), With<Intent>>;
    let mut st: SystemState<(SpatialQuery, Bodies)> = SystemState::new(world);
    let Ok((spatial, chars)) = st.get(world) else { return };
    let excluded: Vec<Entity> = chars.iter().map(|(e, ..)| e).collect();
    let filter = SpatialQueryFilter::from_excluded_entities(excluded).with_mask(crate::core::SOLID_LAYERS);
    let clear = |to: Vec3| {
        let d = to - centre;
        let Ok(dir) = Dir3::new(d) else { return true };
        spatial.cast_ray(centre, dir, d.length(), true, &filter).is_none()
    };
    let mut hurt = Vec::new();
    for (e, t, s, h) in &chars {
        if h.current <= 0.0 {
            continue;
        }
        let c = t.translation + (s.hull_min + s.hull_max) / 2.0;
        let d = c.distance(centre) / scale;
        if d > radius || !(clear(c) || clear(t.translation + s.eye_offset)) {
            continue;
        }
        hurt.push(Damage {
            target: e,
            attacker: CaObjects::entity(attacker),
            amount: damage * super::grenades::falloff(radius, d) * inventory::HP,
            point: c,
            dir: (c - centre).normalize_or_zero(),
            hitgroup: Hitgroup::Generic,
            kind: DamageKind::Blast,
            weapon: None,
            force: Vec3::ZERO,
        });
    }
    for d in hurt {
        world.write_message(d);
    }
    let mut o = world.resource_mut::<CaObjects>();
    let breakables: Vec<(ObjId, f32)> = o
        .world
        .objects
        .iter()
        .enumerate()
        .filter(|(_, ob)| ob.node && !ob.removed && ob.hp.as_ref().is_some_and(|d| !d.dead && !d.never_destroy))
        .filter_map(|(i, ob)| {
            let d = ob.pose.pos.distance(at);
            (d <= radius).then(|| (i as ObjId, damage * super::grenades::falloff(radius, d)))
        })
        .collect();
    for (id, amount) in breakables {
        let d = (o.world.objects[id as usize].pose.pos - at).normalize_or_zero();
        o.world.hit_dir = if d == Vec3::ZERO { Vec3::Z } else { d };
        o.world.damage(id, amount, attacker, false);
    }
}

/// A node entity's map entity position (engine), for tests: where the
/// object stands.
pub fn node_position(world: &World, id: ObjId) -> Option<Vec3> {
    let o = world.resource::<CaObjects>();
    let e = o.nodes.get(&id)?;
    world.get::<Transform>(*e).map(|t| t.translation)
}

/// An object's pose in the engine's axes and metres.
pub fn engine_pose(o: &CaObjects, id: ObjId) -> Option<(Vec3, Quat)> {
    let ob = o.world.objects.get(id as usize)?;
    Some((to_engine(ob.pose.pos, o.scale), rotation_to_engine(ob.pose.rot)))
}
