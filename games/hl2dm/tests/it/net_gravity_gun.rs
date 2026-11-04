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

/// effects_sounds.md 11 on a network client: the server's effect state of
/// each player's gun reaches the others (`Hl2GunEffect`), so B draws A's
/// claws on A's world model, beams while A holds, the idle glow after.
#[test]
fn others_see_the_claws_hold() {
    use chimera_world::map::{
        attached::{AttachedFx, AttachedPiece},
        particles::{MapParticles, ParticleMaterial, ParticleMaterials},
    };
    use hl2dm::game::physcannon_fx::{GunEffect, Hl2GunEffect, MATERIALS, WORLD_BEAM};
    let mut sim = joined();
    let materials = MATERIALS.iter().map(|n| ParticleMaterial { name: n.to_string(), ..default() }).collect();
    sim.clients[1].app.insert_resource(ParticleMaterials(MapParticles { materials }));
    place(&mut sim, 1, Vec3::new(0.0, -300.0, 0.0));
    place(&mut sim, 0, Vec3::ZERO);
    let a_id = sim.client_id(0);
    let a_on_b = |sim: &mut NetSim| {
        let w1 = sim.clients[1].app.world_mut();
        w1.query::<(Entity, &chimera_net::net::NetCharacter)>()
            .iter(w1)
            .find(|(_, c)| c.owner == Some(a_id))
            .map(|(e, _)| e)
            .unwrap()
    };
    let beams = |sim: &mut NetSim| {
        let a = a_on_b(sim);
        let w1 = sim.clients[1].app.world();
        let beam = w1.resource::<ParticleMaterials>().0.find(WORLD_BEAM).unwrap();
        let state = w1.get::<Hl2GunEffect>(a).map(|e| e.0);
        let n = w1.get::<AttachedFx>(a).map_or(0, |fx| {
            fx.world.iter().filter(|p| matches!(p, AttachedPiece::Beam { material, .. } if *material == beam)).count()
        });
        (state, n)
    };
    assert_eq!(beams(&mut sim), (Some(GunEffect::Ready), 0), "idle glow");
    press(&mut sim, 0, 3);
    sim.ticks(40);
    assert_eq!(beams(&mut sim), (Some(GunEffect::Holding), 3), "beams while A holds");
    press(&mut sim, 0, 3);
    sim.ticks(40);
    assert_eq!(beams(&mut sim), (Some(GunEffect::Ready), 0), "let go");
}

/// `hl2mp_allow_pickup` 1 over the network (gravity_gun.md 8): the client
/// of the carrier predicts the carry's weapon side and its sprint, so its
/// predicted state is the server's on every tick while it carries, sprints,
/// presses primary (the throw, the weapon deployed again a draw time on)
/// and holds it down after: no prediction error, the same state bit for
/// bit; the pick-up itself is the server's.
#[test]
fn a_client_predicts_the_carry_as_the_server_runs_it() {
    use chimera_core::core::{PredictedComponents, SimTick};
    use chimera_net::net::predict::{NetGraph, PredictionHistory};
    use chimera_sim::weapon::Inventory;
    use hl2dm::game::{carry::{Carried, CarryRules, Carrying}, suit::Suit};

    #[derive(Resource, Default)]
    struct ServerStates(std::collections::HashMap<u64, Vec<u8>>);
    let mut sim = joined();
    sim.clients[0].app.init_resource::<ServerStates>();
    sim.server.app.world_mut().resource_mut::<CarryRules>().allow_pickup = 1;
    // The SMG1 in hand (draw 0.8333 s).
    {
        let theirs = sim.character_of(0).unwrap();
        let w = sim.server.app.world_mut();
        let gun = chimera_sim::weapon::give(w, theirs, "hl2dm:weapon_smg1").unwrap();
        w.get_mut::<Inventory>(theirs).unwrap().wanted = Some(gun);
    }
    place(&mut sim, 1, Vec3::new(0.0, -300.0, 0.0));
    sim.ticks(100);
    place(&mut sim, 0, Vec3::new(90.0, 0.0, 0.0));
    let theirs = sim.character_of(0).unwrap();
    let me = sim.local_player(0).unwrap();
    let intent = |sim: &mut NetSim, f: &dyn Fn(&mut Intent)| f(&mut sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap());

    // `+use` at the crate: the server carries it, the snapshot tells the client.
    intent(&mut sim, &|i| i.use_key = true);
    sim.step();
    intent(&mut sim, &|i| i.use_key = false);
    sim.ticks(40);
    assert!(sim.server.app.world().get::<Carrying>(theirs).is_some(), "the server carries it");
    assert!(sim.server.app.world().get::<Carried>(theirs).is_some());
    assert!(sim.clients[0].app.world().get::<Carried>(me).is_some(), "the client knows");
    *sim.clients[0].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();

    // After each step, every tick the client has predicted (the history
    // is short) is compared with the server's state of that tick, once.
    let mut compared = std::collections::BTreeSet::new();
    let mut check = |sim: &mut NetSim| {
        let w = sim.server.app.world();
        let tick = w.resource::<SimTick>().0;
        let state = w.resource::<PredictedComponents>().encode(w, theirs);
        sim.clients[0].app.world_mut().resource_mut::<ServerStates>().0.insert(tick, state);
        let (server, history) = {
            let c = sim.clients[0].app.world();
            (c.resource::<ServerStates>().0.clone(), c.resource::<PredictionHistory>().0.clone())
        };
        for p in history.iter().filter(|p| !p.state.is_empty()) {
            if let Some(s) = server.get(&p.tick) {
                assert!(*s == p.state, "the prediction for tick {} isn't the server's", p.tick);
                compared.insert(p.tick);
            }
        }
        compared.len()
    };
    // Sprint backwards while carrying: the run speed, aux power untouched.
    intent(&mut sim, &|i| {
        i.sprint = true;
        i.move_axis = Vec2::NEG_Y;
    });
    for _ in 0..40 {
        sim.step();
        check(&mut sim);
    }
    {
        let w = sim.clients[0].app.world();
        assert!(!w.get::<Suit>(me).unwrap().sprinting, "no sprint while carrying");
        assert!(w.get::<Carried>(me).is_some(), "still carrying");
    }
    // Primary throws it, and held down it can't fire the redeployed weapon.
    intent(&mut sim, &|i| {
        i.sprint = false;
        i.move_axis = Vec2::ZERO;
        i.fire = true;
    });
    // The command reaches the server a latency on; the client threw it when
    // it pressed.
    let mut thrown = 0;
    for k in 0..60 {
        sim.step();
        check(&mut sim);
        if k == 0 {
            assert!(sim.clients[0].app.world().get::<Carried>(me).is_none(), "the client threw it at once");
        }
        let w = sim.server.app.world();
        if thrown == 0 && w.get::<Carrying>(theirs).is_none() {
            thrown = k + 1;
            assert!(w.get::<Carried>(theirs).is_none(), "thrown");
            let now = w.resource::<chimera_core::core::SimClock>().now;
            let inv = w.get::<Inventory>(theirs).unwrap();
            let draw = inv.active.and_then(|a| w.get::<chimera_sim::weapon::Weapon>(a)).map(|x| x.draw_time);
            assert!((inv.next_attack - now - 0.8333).abs() < 0.05, "deployed again: {} at {now}, draw {draw:?}", inv.next_attack);
        }
    }
    assert!(thrown > 0, "the server threw it");
    intent(&mut sim, &|i| i.fire = false);
    for _ in 0..40 {
        sim.step();
        check(&mut sim);
    }
    let g = sim.clients[0].app.world().resource::<NetGraph>().clone();
    assert_eq!(g.errors, 0, "prediction errors ({g:?})");
    let n = check(&mut sim);
    assert!(n > 100, "predicted ticks compared: {n}");
}
