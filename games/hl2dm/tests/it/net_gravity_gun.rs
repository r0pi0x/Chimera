//! A prop held with the gravity gun over the network (`NetSim` on
//! `Hl2Sim`, two clients; lift C6, specs/hl2dm/gravity_gun.md 14): the
//! server grabs and simulates it; its holder's client draws it at its own
//! predicted view (so it follows a turn at once) and passes through it;
//! the other client draws the server's snapshots; let go, both draw the
//! snapshots again.

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::core::{Intent, MovementState};
use chimera_net::{
    harness::NetSim,
    net::{NetHold, memory::LinkConditions},
};
use chimera_world::map::{
    MapBrush, MapCollision, MapConvex, MapData, MapEntity, MapModel, MapPhysics, MapProp, MapSimPlugin, PropIndex,
    PropSolid, PushAway, held::Held,
};
use hl2dm::{
    game::weapons::launch::{U, to_engine, to_source},
    harness::Hl2Sim,
};

/// Looking east (Source and engine +X).
const EAST: f32 = -90.0;

fn solid(data: &mut MapData, lo: Vec3, hi: Vec3) {
    let (a, b) = (to_engine(lo), to_engine(hi));
    let (a, b) = (a.min(b), a.max(b));
    data.collision_brushes.push(MapBrush::from_box(a, b));
    data.collision_hulls.push(
        (0..8)
            .map(|k| [[a.x, b.x][k & 1], [a.y, b.y][(k >> 1) & 1], [a.z, b.z][(k >> 2) & 1]])
            .collect(),
    );
}

/// A floor, a 20 kg crate (±10) on a pillar at (150, 0, 64), the spawns
/// at the origin and 300 to the south.
fn crate_map() -> MapData {
    let mut data = MapData {
        name: "test:held".into(),
        entity_scale: U,
        spawns: vec![
            (to_engine(Vec3::new(0.0, 0.0, 36.0)), None),
            (to_engine(Vec3::new(0.0, -300.0, 36.0)), None),
        ],
        ..default()
    };
    solid(&mut data, Vec3::new(-4096.0, -4096.0, -64.0), Vec3::new(4096.0, 4096.0, 0.0));
    solid(&mut data, Vec3::new(146.0, -4.0, 0.0), Vec3::new(154.0, 4.0, 53.95));
    data.entities.push(MapEntity {
        keyvalues: vec![("classname".into(), "prop_physics".into()), ("origin".into(), "150 0 64".into())],
        hulls: Vec::new(),
        mover: false,
        physics: None,
    });
    let (lo, hi) = (to_engine(Vec3::splat(-10.0)), to_engine(Vec3::splat(10.0)));
    let (lo, hi) = (lo.min(hi), lo.max(hi));
    let corners = (0..8)
        .map(|k| Vec3::new([lo.x, hi.x][k & 1], [lo.y, hi.y][(k >> 1) & 1], [lo.z, hi.z][(k >> 2) & 1]))
        .collect();
    data.models.push(MapModel {
        bounds: (lo, hi),
        collision: Some(MapCollision {
            pieces: vec![MapConvex { points: corners, planes: Vec::new() }],
            mass: 20.0,
            ..default()
        }),
        ..default()
    });
    data.props.push(MapProp {
        pose: None,
        ragdoll: None,
        model: 0,
        translation: to_engine(Vec3::new(150.0, 0.0, 64.0)),
        rotation: Quat::IDENTITY,
        solid: PropSolid::Mesh,
        skybox: false,
        lighting: None,
        vertex_light: None,
        casts_shadow: false,
        physics: Some(MapPhysics {
            mass: 20.0,
            friction: 0.8,
            elasticity: 0.0,
            damping: 0.0,
            rotdamping: 0.0,
            inertia: None,
            push: PushAway::Collide,
            frozen: false,
            asleep: false,
        }),
        fade: None,
        parent: None,
        entity: Some(0),
        skin: 0,
        body: 0,
        detail: false,
    });
    data
}

fn joined() -> NetSim {
    let link = LinkConditions {
        latency: Duration::from_millis(50),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(Hl2Sim, link, 61, 2, |app: &mut App| {
        app.add_plugins(MapSimPlugin::new(crate_map()));
    });
    sim.until_joined(600);
    sim.ticks(100);
    sim
}

fn node(world: &mut World) -> Entity {
    world.query_filtered::<Entity, With<PropIndex>>().iter(world).next().unwrap()
}

fn pos(world: &World, e: Entity) -> Vec3 {
    to_source(world.get::<Transform>(e).unwrap().translation)
}

/// Client `i`'s player put at `feet` (Source) on the server, looking
/// east, settled.
fn place(sim: &mut NetSim, i: usize, feet: Vec3) {
    let theirs = sim.character_of(i).unwrap();
    let at = to_engine(feet + Vec3::Z * 36.0);
    let w = sim.server.app.world_mut();
    w.get_mut::<Transform>(theirs).unwrap().translation = at;
    sim.ticks(10);
    look(sim, i, EAST);
    sim.ticks(60);
}

fn look(sim: &mut NetSim, i: usize, yaw: f32) {
    let me = sim.local_player(i).unwrap();
    let mut intent = sim.clients[i].app.world_mut().get_mut::<Intent>(me).unwrap();
    intent.yaw = yaw.to_radians();
    intent.pitch = 0.0;
}

fn press(sim: &mut NetSim, i: usize, steps: u32) {
    let me = sim.local_player(i).unwrap();
    for k in 0..steps + 1 {
        sim.clients[i].app.world_mut().get_mut::<Intent>(me).unwrap().secondary = k < steps;
        sim.step();
    }
}

/// Where client `i` predicts its eye and view.
fn view_of(sim: &mut NetSim, i: usize) -> (Vec3, Quat) {
    let me = sim.local_player(i).unwrap();
    let w = sim.clients[i].app.world();
    let t = w.get::<Transform>(me).unwrap();
    let s = w.get::<MovementState>(me).unwrap();
    let intent = w.get::<Intent>(me).unwrap();
    (t.translation + s.eye_offset, chimera_sim::weapon::aim_of(intent.yaw, intent.pitch))
}

#[test]
fn a_held_prop_follows_its_holders_view_at_once() {
    let mut sim = joined();
    place(&mut sim, 1, Vec3::new(0.0, -300.0, 0.0));
    place(&mut sim, 0, Vec3::ZERO);
    press(&mut sim, 0, 3);
    sim.ticks(80);
    let theirs = node(sim.server.app.world_mut());
    let holder = sim.character_of(0).unwrap();
    {
        let w = sim.server.app.world();
        assert_eq!(w.get::<Held>(theirs).map(|h| h.holder), Some(holder), "the server holds it for A");
        assert!(w.get::<NetHold>(theirs).is_some(), "sent as held");
    }
    // A's client: marked held by A's player, drawn at its view's offset.
    let mine = node(sim.clients[0].app.world_mut());
    let me = sim.local_player(0).unwrap();
    assert_eq!(sim.clients[0].app.world().get::<Held>(mine).map(|h| h.holder), Some(me));
    let at_server = pos(sim.server.app.world(), theirs);
    let at_a = pos(sim.clients[0].app.world(), mine);
    assert!(at_a.distance(at_server) < 3.0, "A draws it {at_a}, the server has it {at_server}");
    // B's client draws the server's snapshots.
    let other = node(sim.clients[1].app.world_mut());
    let at_b = pos(sim.clients[1].app.world(), other);
    assert!(at_b.distance(at_server) < 3.0, "B draws it {at_b}");
    let b_holder = sim.clients[1].app.world().get::<Held>(other).map(|h| h.holder);
    let a_id = sim.client_id(0);
    let w1 = sim.clients[1].app.world_mut();
    let a_on_b = w1
        .query::<(Entity, &chimera_net::net::NetCharacter)>()
        .iter(w1)
        .find(|(_, c)| c.owner == Some(a_id))
        .map(|(e, _)| e);
    assert_eq!(b_holder, a_on_b, "B's copy is marked held by A's character");

    // A turns 90 degrees left: A's client draws it ahead of the new view
    // at once; B still sees it where the server had it.
    let before = at_b;
    look(&mut sim, 0, 0.0);
    sim.step();
    let (eye, view) = view_of(&mut sim, 0);
    let ahead = to_source(eye + view * Vec3::NEG_Z * 56.627 * U);
    let at_a = pos(sim.clients[0].app.world(), mine);
    assert!(at_a.distance(ahead) < 6.0, "A draws it {at_a}, ahead is {ahead}");
    let at_b = pos(sim.clients[1].app.world(), other);
    assert!(at_b.distance(before) < 10.0, "B: {at_b} (was {before})");
    // B sees it come round after the delay.
    sim.ticks(60);
    let at_b = pos(sim.clients[1].app.world(), other);
    assert!(at_b.distance(ahead) < 6.0, "B: {at_b}, ahead {ahead}");

    // Let go: both draw the snapshots, nobody marks it held.
    sim.ticks(40);
    press(&mut sim, 0, 3);
    sim.ticks(60);
    assert!(sim.server.app.world().get::<Held>(theirs).is_none());
    assert!(sim.clients[0].app.world().get::<Held>(mine).is_none());
    assert!(sim.clients[1].app.world().get::<Held>(other).is_none());
    let at_server = pos(sim.server.app.world(), theirs);
    let at_a = pos(sim.clients[0].app.world(), mine);
    assert!(at_a.distance(at_server) < 3.0, "A {at_a}, server {at_server}");
}
