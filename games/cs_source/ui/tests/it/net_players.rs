//! Players in a network game, from the 2026-10-10 playtest with friends
//! (docs/backlog.md): cheat commands from a client run on the server
//! with its `sv_cheats` (`net::cheats`), and noclip is predicted;
//! `chimera_noblock` lets teammates (or everyone) pass through each other;
//! the chat says who joined and left and why, and a line shows once in
//! the console; the player name is saved and renames you online.

use std::time::Duration;

use bevy::prelude::*;
use crate::stack::{
    client::chat::{ChatLine, GameMessagesPlugin},
    console::{Console, execute, parse},
    core::{God, Intent, LocalPlayer, NoBlock, Team},
    games::cs_source::movement::{self as source, SourceMovementPlugin},
    greybox::GreyboxMapPlugin,
    harness::{CsSim, NetSim, Sim},
    net::{memory::LinkConditions, predict::NetGraph},
    slots::{Loadout, MovementSlot},
};

/// Chat lines this app showed, as text.
#[derive(Resource, Default)]
struct Seen(Vec<String>);

fn record(mut lines: MessageReader<ChatLine>, mut seen: ResMut<Seen>) {
    for l in lines.read() {
        seen.0.push(l.0.iter().map(|(_, s)| s.as_str()).collect());
    }
}

fn setup(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, SourceMovementPlugin, GameMessagesPlugin))
        .insert_resource(Loadout { movement: source::ID })
        .init_resource::<Seen>()
        .add_systems(Last, record);
}

fn lan() -> LinkConditions {
    LinkConditions {
        latency: Duration::from_millis(40),
        jitter: Duration::ZERO,
        loss: 0.0,
    }
}

fn run(world: &mut World, line: &str) {
    for words in parse(line) {
        execute(world, &words, 0);
    }
}

fn console_has(world: &World, text: &str) -> usize {
    world
        .resource::<Console>()
        .output
        .iter()
        .filter(|l| l.text.contains(text))
        .count()
}

/// A server with `n` clients joined, respawned and predicting.
fn joined(seed: u64, n: usize) -> NetSim {
    let mut sim = NetSim::new(CsSim, lan(), seed, n, setup);
    sim.until_joined(600);
    sim.ticks(320);
    sim
}

#[test]
fn a_clients_cheats_run_on_the_server_with_sv_cheats() {
    let mut sim = joined(91, 1);
    let theirs = sim.character_of(0).unwrap();
    // sv_cheats 0: the server says no, in Source's words, in our console.
    run(sim.clients[0].app.world_mut(), "noclip");
    sim.ticks(20);
    let c = sim.clients[0].app.world();
    assert_eq!(
        console_has(
            c,
            "Can't use cheat command noclip in multiplayer, unless the server has sv_cheats set to 1."
        ),
        1
    );
    assert_eq!(
        sim.server.app.world().get::<MovementSlot>(theirs).unwrap().0,
        source::ID
    );

    // sv_cheats 1 on the server: noclip and god act on our player there.
    run(sim.server.app.world_mut(), "sv_cheats 1");
    sim.ticks(10);
    run(sim.clients[0].app.world_mut(), "noclip; god");
    sim.ticks(20);
    let s = sim.server.app.world();
    assert_eq!(
        s.get::<MovementSlot>(theirs).unwrap().0,
        crate::stack::movement::noclip::ID,
        "noclip on the server"
    );
    assert!(s.get::<God>(theirs).is_some(), "god on the server");
    assert!(
        console_has(sim.clients[0].app.world(), "noclip ON") == 1,
        "the server's answer in our console"
    );
    // Our prediction follows: flying up through the ceiling of nothing,
    // the server agreeing.
    let me = sim.local_player(0).unwrap();
    assert_eq!(
        sim.clients[0].app.world().get::<MovementSlot>(me).unwrap().0,
        crate::stack::movement::noclip::ID
    );
    *sim.clients[0].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
    let start = sim.server.app.world().get::<Transform>(theirs).unwrap().translation;
    for _ in 0..90 {
        let mut i = sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap();
        i.pitch = 1.2;
        i.move_axis = Vec2::Y;
        sim.step();
    }
    *sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap() = Intent::default();
    sim.ticks(40);
    let end = sim.server.app.world().get::<Transform>(theirs).unwrap().translation;
    assert!(end.y > start.y + 3.0, "flew up on the server: {start} -> {end}");
    let g = sim.clients[0].app.world().resource::<NetGraph>().clone();
    println!("noclip flight: {} states checked, {} errors", g.checked, g.errors);
    assert!(g.checked > 20);
    assert!(g.errors <= 2, "the client predicts noclip ({} errors)", g.errors);
    // Back off.
    run(sim.clients[0].app.world_mut(), "noclip");
    sim.ticks(20);
    assert_eq!(
        sim.server.app.world().get::<MovementSlot>(theirs).unwrap().0,
        source::ID
    );
    // Anything but a cheat isn't run for a client.
    sim.clients[0]
        .app
        .world_mut()
        .write_message(crate::stack::net::cheats::ClientCommand {
            line: "sv_cheats 0; bot_add".into(),
        });
    sim.ticks(10);
    assert_eq!(sim.server.app.world().resource::<crate::stack::console::Cheats>().0, 1);
}

/// A client's `kill` is a request to the server, which kills the
/// client's player there; it needs no `sv_cheats`.
#[test]
fn a_clients_kill_is_a_request_to_the_server() {
    let mut sim = joined(92, 1);
    let theirs = sim.character_of(0).unwrap();
    assert!(sim.server.app.world().get::<crate::stack::rules::Dead>(theirs).is_none());
    assert_eq!(sim.server.app.world().resource::<crate::stack::console::Cheats>().0, 0);
    run(sim.clients[0].app.world_mut(), "kill");
    sim.ticks(20);
    assert!(
        sim.server.app.world().get::<crate::stack::rules::Dead>(theirs).is_some(),
        "the server killed the client's player"
    );
}

/// Two characters started on one spot walk opposite ways for a second;
/// how far apart they get.
fn walk_apart(noblock: u8, teams: (u8, u8)) -> f32 {
    let mut sim = Sim::new(CsSim, (GreyboxMapPlugin, SourceMovementPlugin));
    sim.app.world_mut().insert_resource(NoBlock(noblock));
    let at = Vec3::new(0.0, 1.0, 12.0);
    let a = sim.spawn_character(at, source::ID);
    let b = sim.spawn_character(at, source::ID);
    sim.app.world_mut().entity_mut(a).insert(Team(teams.0));
    sim.app.world_mut().entity_mut(b).insert(Team(teams.1));
    sim.seconds(0.5);
    sim.intent(a).move_axis = Vec2::Y;
    sim.intent(b).move_axis = -Vec2::Y;
    sim.seconds(1.0);
    sim.position(a).distance(sim.position(b))
}

#[test]
fn noblock_lets_teammates_or_everyone_through() {
    let blocked = walk_apart(0, (2, 2));
    let mates = walk_apart(1, (2, 2));
    let foes = walk_apart(1, (1, 2));
    let everyone = walk_apart(2, (1, 2));
    println!("apart after 1 s: 0 {blocked:.2} m, 1 mates {mates:.2}, 1 foes {foes:.2}, 2 {everyone:.2}");
    assert!(blocked < 1.0, "CS:S (0): stuck in each other ({blocked} m)");
    assert!(mates > 6.0, "1: teammates walk apart ({mates} m)");
    assert!(foes < 1.0, "1: enemies still block ({foes} m)");
    assert!(everyone > 6.0, "2: everyone passes ({everyone} m)");
    assert_eq!(NoBlock::default().0, 0, "off by default (parity)");
}

#[test]
fn noblock_is_the_servers_and_predicted() {
    let mut sim = joined(92, 2);
    run(sim.server.app.world_mut(), "chimera_noblock 1");
    sim.ticks(10);
    for i in 0..2 {
        assert_eq!(
            *sim.clients[i].app.world().resource::<NoBlock>(),
            NoBlock(1),
            "client {i} has the server's"
        );
    }
    // A client can't set it.
    run(sim.clients[0].app.world_mut(), "chimera_noblock 0");
    assert_eq!(*sim.clients[0].app.world().resource::<NoBlock>(), NoBlock(1));
    // Both on one team, put on one spot: they walk apart, as predicted.
    let (a, b) = (sim.character_of(0).unwrap(), sim.character_of(1).unwrap());
    let spot = sim.server.app.world().get::<Transform>(a).unwrap().translation;
    for e in [a, b] {
        let mut w = sim.server.app.world_mut().entity_mut(e);
        w.insert(Team(2));
        w.get_mut::<Transform>().unwrap().translation = spot;
    }
    sim.ticks(40);
    for i in 0..2 {
        *sim.clients[i].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
    }
    for _ in 0..64 {
        for (i, dir) in [(0, 1.0), (1, -1.0)] {
            let me = sim.local_player(i).unwrap();
            let mut intent = sim.clients[i].app.world_mut().get_mut::<Intent>(me).unwrap();
            intent.yaw = 0.0;
            intent.move_axis = Vec2::Y * dir;
        }
        sim.step();
    }
    let s = sim.server.app.world();
    let apart = s
        .get::<Transform>(a)
        .unwrap()
        .translation
        .distance(s.get::<Transform>(b).unwrap().translation);
    let errors: u64 = (0..2)
        .map(|i| sim.clients[i].app.world().resource::<NetGraph>().errors)
        .sum();
    println!("teammates {apart:.2} m apart, {errors} prediction errors");
    assert!(apart > 4.0, "teammates walked apart ({apart} m)");
    assert!(errors <= 4, "predicted ({errors} errors)");
}

#[test]
fn the_chat_says_who_joined_and_left() {
    let mut sim = joined(93, 1);
    run(sim.clients[0].app.world_mut(), "name Alice");
    sim.ticks(10);
    let j = sim.add_client(|w| w.resource_mut::<crate::stack::net::NetSettings>().name = "Bob".into());
    sim.until_joined(600);
    sim.ticks(10);
    let seen = |sim: &NetSim, i: usize| sim.clients[i].app.world().resource::<Seen>().0.clone();
    assert!(
        seen(&sim, 0).iter().any(|l| l == "Player Bob has joined the game"),
        "{:?}",
        seen(&sim, 0)
    );
    sim.remove_client(j);
    sim.ticks(20);
    assert!(
        seen(&sim, 0)
            .iter()
            .any(|l| l == "Player Bob left the game (Disconnect by user.)"),
        "{:?}",
        seen(&sim, 0)
    );
    // A kick's reason.
    let k = sim.add_client(|w| w.resource_mut::<crate::stack::net::NetSettings>().name = "Carl".into());
    sim.until_joined(600);
    run(sim.server.app.world_mut(), "kick Carl");
    sim.ticks(120);
    assert!(
        seen(&sim, 0)
            .iter()
            .any(|l| l == "Player Carl left the game (Kicked by Console)"),
        "{:?}",
        seen(&sim, 0)
    );
    let _ = k;
}

#[test]
fn a_chat_line_shows_once_in_the_console() {
    // A listen server: the host reads its line in its chat (which prints
    // it), and nothing else prints it.
    let mut sim = joined(94, 1);
    let host = sim.server.spawn_character(Vec3::new(4.0, 1.0, 12.0), source::ID);
    sim.server
        .app
        .world_mut()
        .entity_mut(host)
        .insert((LocalPlayer, Team(2)));
    sim.ticks(10);
    run(sim.clients[0].app.world_mut(), "say hello there");
    sim.ticks(20);
    let host_lines: Vec<String> = sim.server.app.world().resource::<Seen>().0.clone();
    assert_eq!(
        host_lines.iter().filter(|l| l.contains("hello there")).count(),
        1,
        "{host_lines:?}"
    );
    assert_eq!(
        console_has(sim.server.app.world(), "hello there"),
        0,
        "the server's own print: none"
    );
    let client_lines = sim.clients[0].app.world().resource::<Seen>().0.clone();
    assert_eq!(client_lines.iter().filter(|l| l.contains("hello there")).count(), 1);
    // A dedicated server (no host): its console has it once.
    sim.server.app.world_mut().entity_mut(host).despawn();
    run(sim.clients[0].app.world_mut(), "say again");
    sim.ticks(20);
    assert_eq!(console_has(sim.server.app.world(), ": again"), 1);
}

#[test]
fn the_player_name_is_saved_and_renames_you_online() {
    let mut sim = joined(95, 1);
    let c = sim.clients[0].app.world();
    assert!(
        c.resource::<Console>().cvar("name").unwrap().archive,
        "name is saved in config.cfg"
    );
    run(sim.clients[0].app.world_mut(), "name \"New Name\"");
    sim.ticks(20);
    let theirs = sim.character_of(0).unwrap();
    assert_eq!(
        sim.server
            .app
            .world()
            .get::<crate::stack::net::NetCharacter>(theirs)
            .unwrap()
            .name,
        "New Name"
    );
    let saved = crate::stack::console::config_text(sim.clients[0].app.world_mut());
    assert!(saved.contains("name \"New Name\""), "{saved}");
}
