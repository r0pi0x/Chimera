//! Network play: breakables a client sees as the server breaks them
//! (`net::breakables`). A window (func_breakable_surf) shot on the server
//! is drawn with the same broken panes on a client (`map::BrushPanes`,
//! replicated), its shards, glass impact and break sound arrive; a glass
//! func_breakable broken there is hidden on the client and its gibs
//! arrive; a round restart makes the window whole on the client too.
//!
//! `cargo test --features dev --test it net_breakables -- --nocapture`

use std::time::Duration;

use avian3d::prelude::ColliderDisabled;
use bevy::prelude::*;
use crate::stack::{
    core::{Damage, DamageKind, Hitgroup, MovingSolid},
    games::cs_source::movement::{self as source, SourceMovementPlugin, to_engine},
    harness::{CsSim, NetSim},
    logic::{Logic, Value},
    map::{
        BrushPanes, GlassImpact, GlassShatter, MapBrush, MapBrushEntity, MapData, MapEntity, MapHull, MapPlugin,
        PlaySound, SpawnGibs,
    },
    net::memory::LinkConditions,
    slots::Loadout,
};

const SCALE: f32 = 0.0254;

fn hull(lo: Vec3, hi: Vec3) -> MapHull {
    let b = MapBrush::from_box(lo, hi);
    let points = (0..8)
        .map(|i| {
            Vec3::new(
                if i & 1 == 0 { lo.x } else { hi.x },
                if i & 2 == 0 { lo.y } else { hi.y },
                if i & 4 == 0 { lo.z } else { hi.z },
            )
        })
        .collect();
    MapHull { planes: b.planes, points }
}

fn entity(pairs: &[(&str, &str)], hulls: Vec<MapHull>) -> MapEntity {
    MapEntity {
        keyvalues: pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        hulls,
        mover: true,
        physics: None,
    }
}

/// A floor; a 120 x 96 window of 10 x 8 glass panes (entity 0) facing
/// -y at y = 200; a glass func_breakable `pane` (entity 1) beside it.
fn glass_map() -> MapData {
    let (a, b) = (to_engine(Vec3::new(-2048.0, -2048.0, -64.0)), to_engine(Vec3::new(2048.0, 2048.0, 0.0)));
    MapData {
        name: "test:glass".into(),
        collision_brushes: vec![MapBrush::from_box(a.min(b), a.max(b))],
        entities: vec![
            entity(
                &[
                    ("classname", "func_breakable_surf"),
                    ("health", "1"),
                    ("fragility", "100"),
                    ("surfacetype", "0"),
                    ("lowerleft", "60 200 0"),
                    ("lowerright", "-60 200 0"),
                    ("upperleft", "60 200 96"),
                    ("upperright", "-60 200 96"),
                ],
                vec![hull(Vec3::new(-60.0, 199.0, 0.0), Vec3::new(60.0, 201.0, 96.0))],
            ),
            entity(
                &[
                    ("classname", "func_breakable"),
                    ("targetname", "pane"),
                    ("material", "0"),
                    ("health", "1"),
                    ("origin", "-300 200 48"),
                ],
                vec![hull(Vec3::new(-32.0, -2.0, -32.0), Vec3::new(32.0, 2.0, 32.0))],
            ),
        ],
        entity_scale: SCALE,
        spawns: vec![(to_engine(Vec3::new(0.0, -300.0, 1.0)), None)],
        ..default()
    }
}

/// What a world heard of breaking glass.
#[derive(Resource, Default, Debug)]
struct Heard {
    shatters: usize,
    impacts: usize,
    glass_gibs: usize,
    sounds: Vec<String>,
}

fn listen(
    mut shatters: MessageReader<GlassShatter>,
    mut impacts: MessageReader<GlassImpact>,
    mut gibs: MessageReader<SpawnGibs>,
    mut sounds: MessageReader<PlaySound>,
    mut heard: ResMut<Heard>,
) {
    heard.shatters += shatters.read().count();
    heard.impacts += impacts.read().count();
    heard.glass_gibs += gibs.read().filter(|g| g.glass).map(|g| g.pieces.len()).sum::<usize>();
    heard.sounds.extend(sounds.read().map(|s| s.entry.clone()));
}

fn glass_world(app: &mut App) {
    app.add_plugins((MapPlugin::new(glass_map()), SourceMovementPlugin))
        .insert_resource(Loadout { movement: source::ID })
        .init_resource::<Heard>()
        .add_systems(Update, listen);
}

fn node(world: &mut World, index: usize) -> Entity {
    world
        .query::<(Entity, &MapBrushEntity)>()
        .iter(world)
        .find(|(_, n)| n.0 == index)
        .unwrap()
        .0
}

fn panes(world: &World, node: Entity) -> Option<BrushPanes> {
    world.get::<BrushPanes>(node).cloned()
}

#[test]
fn clients_see_glass_break_as_the_server_breaks_it() {
    let link = LinkConditions {
        latency: Duration::from_millis(60),
        jitter: Duration::from_millis(10),
        loss: 0.0,
    };
    let mut sim = NetSim::new(CsSim, link, 91, 1, glass_world);
    sim.until_joined(600);
    sim.ticks(64);
    let server_window = node(sim.server.app.world_mut(), 0);
    let client_window = node(sim.clients[0].app.world_mut(), 0);
    assert!(panes(sim.clients[0].app.world(), client_window).is_none(), "whole at first");

    // A bullet into pane (4, 3) of the window on the server.
    sim.server.app.world_mut().write_message(Damage {
        target: server_window,
        attacker: None,
        amount: 0.26,
        point: to_engine(Vec3::new(6.0, 200.0, 42.0)),
        dir: Vec3::Z,
        hitgroup: Hitgroup::Generic,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(64);
    let theirs = panes(sim.server.app.world(), server_window).expect("the server's window broke");
    assert!(theirs.broken.iter().any(|b| *b), "a pane gone on the server");
    let ours = panes(sim.clients[0].app.world(), client_window);
    assert_eq!(ours.as_ref(), Some(&theirs), "the client draws the server's panes");
    {
        let w = sim.clients[0].app.world();
        assert!(!w.entity(client_window).contains::<ColliderDisabled>(), "its panes still take shots");
        assert!(w.get::<MovingSolid>(client_window).is_some_and(|m| !m.solid), "players pass");
        let heard = w.resource::<Heard>();
        println!("client heard after the window: {heard:?}");
        assert!(heard.shatters > 0, "the shards of the shot pane");
        assert!(heard.impacts > 0, "the glass impact");
        assert!(heard.sounds.iter().any(|s| s == "Glass.Break"), "the break sound");
    }

    // The glass breakable, broken outright.
    sim.server
        .app
        .world_mut()
        .resource_mut::<Logic>()
        .world
        .queue_input("pane", "Break", Value::Void, 0.0, None);
    sim.ticks(64);
    let client_pane = node(sim.clients[0].app.world_mut(), 1);
    {
        let w = sim.clients[0].app.world();
        assert_eq!(w.get::<Visibility>(client_pane), Some(&Visibility::Hidden), "the breakable gone");
        let heard = w.resource::<Heard>();
        println!("client heard after the breakable: {heard:?}");
        assert!(heard.glass_gibs > 0, "its glass gibs");
    }
    // The host hears each once (nothing doubled by the network).
    let host = sim.server.app.world().resource::<Heard>();
    let client = sim.clients[0].app.world().resource::<Heard>();
    assert_eq!(client.glass_gibs, host.glass_gibs, "the same gibs");
    assert_eq!(client.shatters, host.shatters, "the same shards");

    // A round restart: whole again on the client too.
    crate::stack::core::restart_round(sim.server.app.world_mut());
    sim.ticks(64);
    assert!(panes(sim.server.app.world(), server_window).is_none(), "whole on the server");
    assert!(panes(sim.clients[0].app.world(), client_window).is_none(), "whole on the client");
}

/// A new round makes a broken window whole in the tick it starts
/// (`map::breakables::round_restart`, an observer of `core::RoundRestart`,
/// which CS:S's round start triggers inside the rules' step), as when the
/// restart was polled later in that tick.
#[test]
fn a_new_round_makes_the_window_whole_in_its_tick() {
    use crate::stack::{
        console::Console,
        core::{RoundRestarts, Team},
        harness::Sim,
    };
    let mut sim = Sim::new(CsSim, glass_world);
    let spawn = to_engine(Vec3::new(0.0, -300.0, 1.0));
    let t = sim.spawn_character(spawn, source::ID);
    let ct = sim.spawn_character(spawn + Vec3::X * 2.0, source::ID);
    sim.app.world_mut().entity_mut(t).insert(Team(1));
    sim.app.world_mut().entity_mut(ct).insert(Team(2));
    sim.app
        .world_mut()
        .resource_mut::<Console>()
        .submit("mp_freezetime 1; mp_roundtime 0.5; chimera_rounds 1");
    sim.seconds(1.5);
    let window = node(sim.app.world_mut(), 0);
    sim.app.world_mut().write_message(Damage {
        target: window,
        attacker: None,
        amount: 0.26,
        point: to_engine(Vec3::new(6.0, 200.0, 42.0)),
        dir: Vec3::Z,
        hitgroup: Hitgroup::Generic,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(8);
    assert!(panes(sim.app.world(), window).is_some(), "the window broke");
    // The terrorists win: the next round starts after the round's end.
    sim.app.world_mut().write_message(Damage {
        target: ct,
        attacker: Some(t),
        amount: 10.0,
        point: Vec3::ZERO,
        dir: Vec3::X,
        hitgroup: Hitgroup::Chest,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    let restarts = sim.app.world().resource::<RoundRestarts>().0;
    let mut ticks = 0;
    while sim.app.world().resource::<RoundRestarts>().0 == restarts {
        assert!(panes(sim.app.world(), window).is_some(), "broken until the new round");
        ticks += 1;
        assert!(ticks < 1000, "no new round");
        sim.ticks(1);
    }
    assert!(panes(sim.app.world(), window).is_none(), "whole in the new round's first tick");
}
