//! specs/combat_arms/world_objects.md in play, on worlds built in code
//! and converted (`game::world::convert`): a door's world model is a node
//! of its own that the use key opens (`inventory::ACTIVATION_DISTANCE`)
//! and that stops a player while closed; a breakable breaks under rifle
//! fire and stops nothing after; a trigger's delayed command switches a
//! volume; a client of a network game sees the server's door open (and a
//! late joiner too).

use std::time::Duration;

use bevy::prelude::*;
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use combat_arms::{
    game::{
        TICK_INTERVAL, movement,
        objects::{CaObjectMessage, CaObjects, CaNode, geometry::from_engine, movers::State},
        volumes::CaVolume,
        world,
    },
    harness::CaSim,
};
use lithtech_formats::world::{
    BspNode, BspPolygon, Object, Plane, Property, Surface, Value, World, WorldModel, model_flags, surface_flags,
};

pub(crate) fn prop(name: &str, value: Value) -> Property {
    Property {
        name: name.into(),
        flags: 0,
        len: 0,
        value,
    }
}

/// A box world model from `min` to `max` (file frame) named `name`
/// (`physics`: the level's own physics BSP).
pub(crate) fn cuboid(name: &str, min: [f32; 3], max: [f32; 3], physics: bool) -> WorldModel {
    let [x0, y0, z0] = min;
    let [x1, y1, z1] = max;
    let quads: [([f32; 3], [[f32; 3]; 4]); 6] = [
        ([0.0, 1.0, 0.0], [[x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0]]),
        ([0.0, -1.0, 0.0], [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]]),
        ([1.0, 0.0, 0.0], [[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]]),
        ([-1.0, 0.0, 0.0], [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]]),
        ([0.0, 0.0, 1.0], [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]]),
        ([0.0, 0.0, -1.0], [[x0, y0, z0], [x0, y1, z0], [x1, y1, z0], [x1, y0, z0]]),
    ];
    let mut m = WorldModel {
        flags: if physics { model_flags::MAIN_WORLD | model_flags::PHYSICS } else { 0 },
        name: name.into(),
        textures: vec!["Default".into()],
        surfaces: vec![Surface {
            flags: surface_flags::SOLID,
            ..default()
        }],
        min,
        max,
        translation: [(x0 + x1) / 2.0, (y0 + y1) / 2.0, (z0 + z1) / 2.0],
        root: -1,
        ..default()
    };
    for (n, corners) in quads {
        let d = n[0] * corners[0][0] + n[1] * corners[0][1] + n[2] * corners[0][2];
        m.planes.push(Plane { normal: n, dist: d });
        let first = m.point_indices.len() as u32;
        for c in corners {
            let at = m.points.iter().position(|p| *p == c).unwrap_or_else(|| {
                m.points.push(c);
                m.points.len() - 1
            });
            m.point_indices.push(at as u32);
        }
        m.polygons.push(BspPolygon {
            surface: 0,
            plane: m.planes.len() as u32 - 1,
            first,
            count: 4,
        });
        m.nodes.push(BspNode {
            polygon: m.polygons.len() as u32 - 1,
            back: -2,
            front: -1,
        });
    }
    m
}

/// A floor (y 0, ±2000 u) with a start point at the origin facing +Z,
/// and `objects` with their world models.
pub(crate) fn world_with(models: Vec<WorldModel>, objects: Vec<Object>) -> World {
    let mut w = World {
        models: vec![cuboid("PhysicsBSP", [-2000.0, -20.0, -2000.0], [2000.0, 0.0, 2000.0], true)],
        ..default()
    };
    w.models.extend(models);
    w.objects.push(Object {
        class: "GameStartPoint".into(),
        properties: vec![
            prop("Name", Value::String("GameStartPoint00".into())),
            prop("Pos", Value::Vector([0.0, 66.0, 0.0])),
            prop("Rotation", Value::Rotation([0.0; 4])),
            prop("GameType", Value::String("Team Death Match".into())),
            prop("Teams", Value::String("JSF".into())),
        ],
    });
    w.objects.extend(objects);
    w
}

/// A sliding door across +Z at z 300..310 (x ±100, 150 high) opening 160
/// u up in 1 s.
fn door_world() -> World {
    world_with(
        vec![cuboid("Door", [-100.0, 0.0, 300.0], [100.0, 150.0, 310.0], false)],
        vec![Object {
            class: "SlidingDoor".into(),
            properties: vec![
                prop("Name", Value::String("Door".into())),
                prop("Pos", Value::Vector([0.0, 75.0, 305.0])),
                prop("MoveDir", Value::Vector([0.0, 1.0, 0.0])),
                prop("MoveDist", Value::Real(160.0)),
                prop("OpeningTime", Value::Real(1.0)),
                prop("Solid", Value::Bool(true)),
                prop("Rayhit", Value::Bool(true)),
            ],
        }],
    )
}

fn state_of(sim: &Sim, name: &str) -> State {
    let o = sim.app.world().resource::<CaObjects>();
    o.world.objects[o.named(name).unwrap() as usize].awm().unwrap().state
}

/// The door is a node; a player walking at it stops; the use key opens
/// it (it rises); then the player walks through.
#[test]
fn a_door_stops_a_player_and_opens_by_use() {
    let s = movement::movement_data().meters_per_unit;
    let map = world::convert(&door_world(), "door", s, &|_| None);
    assert_eq!(map.entities.iter().filter(|e| e.mover).count(), 1, "the door's node");
    assert_eq!(map.collision_indices.len(), 12, "the door isn't in the level's collision");
    let (feet, yaw) = (map.spawns[0].0, map.spawn_yaws[0]);
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
    sim.ticks(3);
    {
        let w = sim.app.world_mut();
        assert_eq!(w.query::<&CaNode>().iter(w).count(), 1, "bound");
    }
    let p = sim.spawn_character(feet + Vec3::Y, movement::ID);
    sim.intent(p).yaw = yaw;
    sim.seconds(1.0);
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(4.0);
    let z = from_engine(sim.position(p), s).z;
    assert!(z < 300.0 && z > 250.0, "stopped at the door: {z}");
    assert_eq!(state_of(&sim, "Door"), State::Off);
    sim.intent(p).move_axis = Vec2::ZERO;
    sim.intent(p).use_key = true;
    sim.ticks(2);
    sim.intent(p).use_key = false;
    // The next server frame (20 Hz) takes the press.
    sim.ticks(4);
    assert_eq!(state_of(&sim, "Door"), State::PowerOn, "the use key opened it");
    sim.seconds(1.5);
    assert_eq!(state_of(&sim, "Door"), State::On);
    let o = sim.app.world().resource::<CaObjects>();
    let pos = o.world.objects[o.named("Door").unwrap() as usize].pose.pos;
    assert!((pos.y - (75.0 + 160.0)).abs() < 1e-3, "risen 160 u: {pos}");
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(1.5);
    let z = from_engine(sim.position(p), s).z;
    assert!(z > 330.0, "through: {z}");
}

/// A breakable crate in front of the shooter (10 HP) breaks under fire,
/// hidden, not ray-hit, and no longer stops shots: the target behind it
/// is hurt.
#[test]
fn bullets_break_a_breakable() {
    let s = movement::movement_data().meters_per_unit;
    let w = world_with(
        vec![cuboid("Crate", [-40.0, 0.0, 150.0], [40.0, 160.0, 170.0], false)],
        vec![Object {
            class: "Breakable".into(),
            properties: vec![
                prop("Name", Value::String("Crate".into())),
                prop("HitPoints", Value::Real(10.0)),
                prop("MaxHitPoints", Value::Real(10.0)),
                prop("RemoveOnDeath", Value::Bool(false)),
                prop("Solid", Value::Bool(true)),
                prop("Rayhit", Value::Bool(true)),
            ],
        }],
    );
    let map = world::convert(&w, "crate", s, &|_| None);
    let feet = map.spawns[0].0;
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
    sim.ticks(3);
    let shooter = sim.spawn_character(feet + Vec3::Y, movement::ID);
    let target = sim.spawn_character(feet + Vec3::new(0.0, 1.0, -400.0 * s), movement::ID);
    sim.seconds(1.5);
    {
        let d = sim.position(target) - (sim.position(shooter) + sim.state(shooter).eye_offset);
        sim.intent(shooter).pitch = d.y.atan2(d.xz().length());
    }
    sim.intent(shooter).fire = true;
    sim.seconds(1.0);
    sim.intent(shooter).fire = false;
    sim.ticks(4);
    let o = sim.app.world().resource::<CaObjects>();
    let crate_ = &o.world.objects[o.named("Crate").unwrap() as usize];
    assert!(crate_.hp.as_ref().unwrap().dead, "broken");
    assert!(!crate_.visible && !crate_.solid && !crate_.rayhit);
    let node = *o.nodes.get(&o.named("Crate").unwrap()).unwrap();
    assert_eq!(sim.app.world().get::<Visibility>(node), Some(&Visibility::Hidden));
    let health = sim.app.world().get::<chimera_core::core::Health>(target).unwrap().current;
    assert!(health < 1.0, "the target behind it was hit once it broke: {health}");
}

/// A trigger's delayed command switches a kill volume on, which then
/// kills; a start-up command switched it off first.
#[test]
fn a_delayed_command_switches_a_volume() {
    let s = movement::movement_data().meters_per_unit;
    let w = world_with(
        vec![cuboid("Pit", [-100.0, 0.0, -100.0], [100.0, 100.0, 100.0], false)],
        vec![
            Object {
                class: "EndlessFall".into(),
                properties: vec![prop("Name", Value::String("Pit".into()))],
            },
            Object {
                class: "StartupCommand".into(),
                properties: vec![
                    prop("Name", Value::String("Start".into())),
                    prop("Command", Value::String("msg Pit off;".into())),
                ],
            },
            Object {
                class: "Trigger".into(),
                properties: vec![
                    prop("Name", Value::String("Arm".into())),
                    prop("Pos", Value::Vector([0.0, -500.0, 0.0])),
                    prop("Command1", Value::String("delay 2 (msg Pit on)".into())),
                ],
            },
        ],
    );
    let map = world::convert(&w, "pit", s, &|_| None);
    let feet = map.spawns[0].0;
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
    sim.ticks(3);
    let p = sim.spawn_character(feet + Vec3::Y, movement::ID);
    sim.seconds(1.0);
    let health = |sim: &Sim| sim.app.world().get::<chimera_core::core::Health>(p).unwrap().current;
    assert_eq!(health(&sim), 1.0, "off at the start");
    sim.app.world_mut().write_message(CaObjectMessage {
        target: "Arm".into(),
        message: "trigger".into(),
    });
    sim.seconds(1.5);
    assert_eq!(health(&sim), 1.0, "not yet");
    sim.seconds(1.0);
    assert!(health(&sim) <= 0.0, "killed once it was on");
    let w = sim.app.world_mut();
    assert!(w.query::<&CaVolume>().iter(w).all(|v| !v.hidden));
}

/// A network game: the server opens the door; a client sees its node
/// rise; a client that joins after sees it open.
#[test]
fn clients_see_the_door() {
    let s = movement::movement_data().meters_per_unit;
    let map = world::convert(&door_world(), "door", s, &|_| None);
    let link = LinkConditions {
        latency: Duration::from_millis(40),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let m1 = map.clone();
    let mut sim = NetSim::new(CaSim, link, 3, 1, move |app| {
        app.add_plugins(MapSimPlugin::new(m1.clone()));
    });
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    sim.ticks(60);
    let door_y = |app: &App| {
        let o = app.world().resource::<CaObjects>();
        let id = o.named("Door").unwrap();
        let node = *o.nodes.get(&id).expect("a node");
        from_engine(app.world().get::<Transform>(node).unwrap().translation, s).y
    };
    assert!((door_y(&sim.clients[0].app) - 75.0).abs() < 1e-3);
    sim.server.app.world_mut().write_message(CaObjectMessage {
        target: "Door".into(),
        message: "trigger".into(),
    });
    sim.ticks(150);
    assert!((door_y(&sim.server.app) - 235.0).abs() < 1e-3, "open on the server");
    assert!((door_y(&sim.clients[0].app) - 235.0).abs() < 1e-3, "and on the client");
    sim.add_client(|_| {});
    sim.until_joined(600);
    sim.ticks(30);
    assert!((door_y(&sim.clients[1].app) - 235.0).abs() < 1e-3, "a late joiner sees it open");
}

/// world_dat.md "Light groups": after each message the object sends
/// every client its group's colour: a client relights from the server's
/// switch, and a late joiner gets the group as it is.
#[test]
fn clients_see_the_light_groups() {
    use chimera_world::map::LightStyles;
    let map = world::convert(&crate::world::light_group_world(false), "lg", 0.01, &|_| None);
    let link = LinkConditions {
        latency: Duration::from_millis(40),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(CaSim, link, 3, 1, move |app| {
        app.add_plugins(MapSimPlugin::new(map.clone()));
    });
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    sim.ticks(30);
    let styles = |app: &App| app.world().get_resource::<LightStyles>().map(|s| s.styles.clone());
    sim.server.app.world_mut().write_message(CaObjectMessage {
        target: "G".into(),
        message: "on".into(),
    });
    sim.ticks(30);
    assert_eq!(styles(&sim.server.app), Some(vec![(32, true)]));
    assert_eq!(styles(&sim.clients[0].app), Some(vec![(32, true)]), "the client relights");
    sim.add_client(|_| {});
    sim.until_joined(600);
    sim.ticks(30);
    assert_eq!(styles(&sim.clients[1].app), Some(vec![(32, true)]), "a late joiner too");
}
