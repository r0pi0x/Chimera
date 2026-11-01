//! Players' sprays (CS:S's `impulse 201`): the player's `cl_logofile` logo
//! on the wall or floor it looks at within reach, one per player (a new
//! one replaces the old), `decalfrequency` seconds apart, with the spray
//! sound; kept through a round restart, gone with the map; over the
//! network the server sprays for a client and every client gets the spray.

use std::time::Duration;

use bevy::{ecs::message::Messages, prelude::*};
use crate::stack::{
    console::Console,
    core::{Intent, LocalPlayer, Team},
    games::cs_source::{
        TICK_INTERVAL,
        movement::{self as source, SourceMovementPlugin},
        weapons::{CsWeaponsPlugin, SPRAY_SOUND},
    },
    greybox::{GreyboxMapPlugin, NORTH_WALL_Z},
    harness::{CsSim, NetSim, Sim},
    map::{GameSound, PlaySound, decal::Spray},
    movement::placeholder,
    net::{NetCharacter, memory::LinkConditions},
    slots::Loadout,
    weapon::spray::{LOGO_KEY, clear_sprays},
};

fn sprays(world: &mut World) -> Vec<Spray> {
    world.query::<&Spray>().iter(world).cloned().collect()
}

fn say(sim: &mut Sim, line: &str) {
    sim.app.world_mut().resource_mut::<Console>().submit(line);
    sim.ticks(2);
}

fn setup() -> (Sim, Entity) {
    let mut sim = Sim::new(CsSim, (GreyboxMapPlugin, CsWeaponsPlugin));
    sim.set_tick_interval(TICK_INTERVAL);
    // 2 m from the north wall, looking at it (yaw 0 looks down -Z).
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, NORTH_WALL_Z + 2.0), placeholder::ID);
    // The local player's userinfo comes from its cvars.
    sim.app.world_mut().entity_mut(me).insert((Team(2), LocalPlayer));
    say(&mut sim, &format!("{LOGO_KEY} materials/vgui/logos/spray_kilroy.vtf"));
    sim.ticks(1);
    (sim, me)
}

#[test]
fn a_spray_goes_on_the_wall_once_per_player_and_decalfrequency() {
    let (mut sim, me) = setup();
    let mut cursor = sim.app.world().resource::<Messages<PlaySound>>().get_cursor_current();
    let mut game = sim.app.world().resource::<Messages<GameSound>>().get_cursor_current();
    say(&mut sim, "impulse 201");
    let all = sprays(sim.app.world_mut());
    assert_eq!(all.len(), 1, "{all:?}");
    let s = &all[0];
    assert_eq!((s.owner, s.logo.as_str()), (me, "spray_kilroy"));
    assert!((s.point.z - NORTH_WALL_Z).abs() < 0.05, "on the wall: {:?}", s.point);
    assert!(s.normal.z > 0.99 && s.up.y > 0.99, "upright, facing us: {s:?}");
    let w = sim.app.world();
    let heard: Vec<String> = cursor
        .read(w.resource::<Messages<PlaySound>>())
        .map(|p| p.entry.clone())
        .chain(
            game.read(w.resource::<Messages<GameSound>>())
                .map(|g| g.0.entry.clone()),
        )
        .collect();
    assert!(heard.iter().any(|e| e == SPRAY_SOUND), "{heard:?}");
    // Too soon (decalfrequency 10): the first stays where it is.
    sim.intent(me).pitch = -1.4;
    say(&mut sim, "impulse 201");
    assert_eq!(sprays(sim.app.world_mut()), all);
    // Later: the new one (on the floor) replaces it.
    sim.seconds(10.0);
    say(&mut sim, "impulse 201");
    let now = sprays(sim.app.world_mut());
    assert_eq!(now.len(), 1);
    assert!(
        now[0].normal.y > 0.99 && now[0].point.y.abs() < 0.05,
        "on the floor: {:?}",
        now[0]
    );
    // Turned with its top away from us (we look down -Z).
    assert!(now[0].up.z < -0.99, "{:?}", now[0].up);
    // `decalfrequency 0`: as often as you like, still one.
    say(&mut sim, "decalfrequency 0");
    sim.intent(me).pitch = 0.0;
    say(&mut sim, "impulse 201");
    assert_eq!(sprays(sim.app.world_mut()).len(), 1);
    assert!(sprays(sim.app.world_mut())[0].normal.z > 0.99);
    // A spray is not a cheat (impulse 101 still is, in a network game).
    let console = sim.app.world().resource::<crate::stack::console::Console>();
    assert!(!crate::stack::console::is_cheat(console, "impulse", &["201".into()]));
    assert!(crate::stack::console::is_cheat(console, "impulse", &["101".into()]));
    // Gone with the map.
    clear_sprays(sim.app.world_mut());
    assert!(sprays(sim.app.world_mut()).is_empty());
}

#[test]
fn nothing_in_reach_or_dead_sprays_nothing() {
    let (mut sim, me) = setup();
    // Turned round: the nearest wall is far behind.
    sim.intent(me).yaw = std::f32::consts::PI;
    say(&mut sim, "impulse 201");
    assert!(sprays(sim.app.world_mut()).is_empty());
    sim.intent(me).yaw = 0.0;
    sim.app.world_mut().get_mut::<crate::stack::core::Health>(me).unwrap().current = 0.0;
    say(&mut sim, "impulse 201");
    assert!(sprays(sim.app.world_mut()).is_empty());
}

#[test]
fn sprays_stay_through_a_round_restart() {
    let (mut sim, me) = setup();
    say(&mut sim, "mp_freezetime 0; mp_roundtime 5; chimera_rounds 1");
    sim.ticks(5);
    // Rounds put us at a spawn: back to the wall.
    let at = Vec3::new(0.0, 1.0, NORTH_WALL_Z + 2.0);
    sim.app.world_mut().get_mut::<Transform>(me).unwrap().translation = at;
    if let Some(mut p) = sim.app.world_mut().get_mut::<avian3d::prelude::Position>(me) {
        p.0 = at;
    }
    sim.intent(me).yaw = 0.0;
    sim.intent(me).pitch = 0.0;
    sim.ticks(2);
    say(&mut sim, "impulse 201");
    assert_eq!(sprays(sim.app.world_mut()).len(), 1);
    say(&mut sim, "mp_restartgame 1");
    sim.seconds(3.0);
    assert_eq!(sprays(sim.app.world_mut()).len(), 1, "kept, as CS:S keeps them");
}

fn net_setup(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, SourceMovementPlugin, CsWeaponsPlugin))
        .insert_resource(Loadout { movement: source::ID })
        .insert_resource(Time::<Fixed>::from_seconds(TICK_INTERVAL));
}

#[test]
fn a_clients_spray_reaches_every_client() {
    let link = LinkConditions {
        latency: Duration::from_millis(30),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(CsSim, link, 61, 0, net_setup);
    for i in 0..2 {
        sim.add_client(|w| w.resource_mut::<crate::stack::net::NetSettings>().name = format!("Player {}", i + 1));
    }
    sim.clients[0]
        .app
        .world_mut()
        .resource_mut::<Console>()
        .submit("cl_logofile materials/vgui/logos/spray_nowar.vtf");
    sim.until_joined(600);
    sim.ticks(60);
    // The server puts player 1 before the north wall, looking at it.
    let e = sim.character_of(0).unwrap();
    let at = Vec3::new(0.0, 1.0, NORTH_WALL_Z + 2.0);
    {
        let w = sim.server.app.world_mut();
        w.get_mut::<Transform>(e).unwrap().translation = at;
        if let Some(mut p) = w.get_mut::<avian3d::prelude::Position>(e) {
            p.0 = at;
        }
    }
    let me = sim.local_player(0).unwrap();
    {
        let mut i = sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap();
        i.yaw = 0.0;
        i.pitch = 0.0;
    }
    sim.ticks(30);
    sim.clients[0]
        .app
        .world_mut()
        .resource_mut::<Console>()
        .submit("impulse 201");
    let ok = sim.until(120, |s| {
        (0..2).all(|i| !sprays(s.clients[i].app.world_mut()).is_empty())
    });
    assert!(ok, "every client has the spray");
    let server = sprays(sim.server.app.world_mut());
    assert_eq!(server.len(), 1);
    assert_eq!((server[0].owner, server[0].logo.as_str()), (e, "spray_nowar"));
    // On player 2's client the spray is player 1's character there.
    let other = sprays(sim.clients[1].app.world_mut()).remove(0);
    let w = sim.clients[1].app.world();
    assert_eq!(
        w.get::<NetCharacter>(other.owner).map(|c| c.name.as_str()),
        Some("Player 1")
    );
    assert!((other.point - server[0].point).length() < 1e-3);
    // Night vision: carried goggles reach their owner's client.
    sim.server
        .app
        .world_mut()
        .entity_mut(e)
        .insert(crate::stack::weapon::economy::NightVision);
    let ok = sim.until(120, |s| {
        let me = s.local_player(0).unwrap();
        s.clients[0]
            .app
            .world()
            .get::<crate::stack::weapon::economy::NightVision>(me)
            .is_some()
    });
    assert!(ok, "the owner's client knows it has goggles");
}
