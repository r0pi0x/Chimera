//! CS:S's join flow (`games::cs_source::ui::join_menus`, `rules::JoinChoice`): a player
//! who comes into a game reads the message of the day, picks a team, then
//! a class, and spawns as the round rules allow; tests and scripted runs
//! skip it and join a team at once.
//!
//! - Over the network: a client asking for the menus comes in on no team
//!   with the server's MOTD, its "has joined the game" line before its
//!   team's "is joining the ... force", no body until it has a class, and
//!   in the freeze time it plays that round with the class's model.
//! - A client asking for a team (`--join 2`, `+jointeam 2`) is on it at
//!   once, with no menus.
//! - Single player: a game started with the menus leaves the player on no
//!   team with the MOTD open; `jointeam` then `joinclass` bring it in.
//! - `cl_disablehtmlmotd` picks the plain-text message.

use bevy::prelude::*;
use crate::stack::{
    client::chat::{ChatLine, GameMessagesPlugin},
    games::cs_source::ui::{
        join_menus::{self, DisableHtmlMotd, JoinMenus, JoinMenusStatePlugin},
        team_menu::TeamMenu,
    },
    console::Console,
    core::{ChoosingClass, LocalPlayer, PlayerClass, Spectating, Team, Unassigned},
    games::cs_source::{
        TICK_INTERVAL,
        movement::{self as source, SourceMovementPlugin},
        weapons::CsWeaponsPlugin,
    },
    greybox::GreyboxMapPlugin,
    harness::{CsSim, NetSim, Sim},
    map::radio::{RadioCommands, SayFormats},
    net::{ServerMotd, memory::LinkConditions},
    rules::{
        Dead, JoinChoice,
        rounds::{Phase, RoundState},
    },
    slots::Loadout,
};

fn setup(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, SourceMovementPlugin, CsWeaponsPlugin))
        .add_plugins(GameMessagesPlugin)
        .add_plugins(JoinMenusStatePlugin)
        .insert_resource(Loadout { movement: source::ID })
        // The game's join lines (as CS:S's strings say them).
        .insert_resource(RadioCommands {
            say: SayFormats {
                joins: vec![
                    (1, "%s1 is joining the Terrorist force".into()),
                    (2, "%s1 is joining the Counter-Terrorist force".into()),
                ],
                ..default()
            },
            ..default()
        })
        .insert_resource(Time::<Fixed>::from_seconds(TICK_INTERVAL));
}

fn client_console(sim: &mut NetSim, i: usize, line: &str) {
    sim.clients[i].app.world_mut().resource_mut::<Console>().submit(line);
}

/// Chat lines' text, as one string each.
fn lines(world: &mut World, cursor: &mut bevy::ecs::message::MessageCursor<ChatLine>) -> Vec<String> {
    let messages = world.resource::<bevy::ecs::message::Messages<ChatLine>>();
    cursor
        .read(messages)
        .map(|l| l.0.iter().map(|(_, t)| t.as_str()).collect::<String>())
        .collect()
}

/// A rounds server (a long freeze) with one plain client on the
/// counter-terrorists.
fn server() -> NetSim {
    let mut sim = NetSim::new(CsSim, LinkConditions::default(), 77, 1, setup);
    sim.server
        .app
        .world_mut()
        .resource_mut::<Console>()
        .submit("mp_freezetime 30; mp_roundtime 1; chimera_rounds 1; hostname Test Server");
    sim.until_joined(600);
    sim.ticks(10);
    sim
}

#[test]
fn a_client_reads_the_motd_then_picks_a_team_and_a_class() {
    let mut sim = server();
    let i = sim.add_client(|w| {
        w.resource_mut::<crate::stack::net::NetSettings>().name = "Newcomer".into();
        w.insert_resource(JoinChoice::Menus);
    });
    let mut cursor = sim.clients[i].app.world().resource::<bevy::ecs::message::Messages<ChatLine>>().get_cursor();
    let mut heard = Vec::new();
    let ok = sim.until(600, |s| {
        heard.extend(lines(s.clients[i].app.world_mut(), &mut cursor));
        s.clients[i].app.world().contains_resource::<crate::stack::net::client::Joined>()
            && s.local_player(i).is_some_and(|me| s.clients[i].app.world().get::<Unassigned>(me).is_some())
    });
    assert!(ok, "joined on no team");
    for _ in 0..10 {
        sim.step();
        heard.extend(lines(sim.clients[i].app.world_mut(), &mut cursor));
    }

    // On the server: in the game, on no team, no body.
    let theirs = sim.character_of(i).unwrap();
    let w = sim.server.app.world();
    assert!(w.get::<Unassigned>(theirs).is_some() && w.get::<Spectating>(theirs).is_some());
    assert_eq!(w.get::<Team>(theirs), Some(&Team(0)));
    assert!(w.get::<Dead>(theirs).is_some());
    // The client: the message of the day, from the server, open.
    let cw = sim.clients[i].app.world();
    let motd = cw.resource::<ServerMotd>();
    assert_eq!(motd.server, "Test Server");
    assert!(cw.resource::<JoinMenus>().motd, "the MOTD is open");
    assert!(!cw.resource::<TeamMenu>().0);
    // Its OK opens the team menu.
    join_menus::motd_ok(sim.clients[i].app.world_mut());
    assert!(sim.clients[i].app.world().resource::<TeamMenu>().0);

    // The terrorists: on the team, no body yet, the class menu opens.
    sim.clients[i].app.world_mut().resource_mut::<TeamMenu>().0 = false;
    client_console(&mut sim, i, "jointeam 2");
    let ok = sim.until(200, |s| {
        heard.extend(lines(s.clients[i].app.world_mut(), &mut cursor));
        let me = s.local_player(i).unwrap();
        s.clients[i].app.world().get::<ChoosingClass>(me).is_some()
    });
    assert!(ok, "waiting for a class");
    for _ in 0..5 {
        sim.step();
        heard.extend(lines(sim.clients[i].app.world_mut(), &mut cursor));
    }
    let w = sim.server.app.world();
    assert_eq!(w.get::<Team>(theirs), Some(&Team(1)));
    assert!(w.get::<Unassigned>(theirs).is_none() && w.get::<Spectating>(theirs).is_none());
    assert!(w.get::<Dead>(theirs).is_some(), "no body before a class");
    assert_eq!(sim.clients[i].app.world().resource::<JoinMenus>().class, Some(1), "the class menu");
    // CS:S's order: the connect line, then the team's.
    let joined = heard.iter().position(|l| l == "Player Newcomer has joined the game");
    let team = heard.iter().position(|l| l == "Newcomer is joining the Terrorist force");
    println!("chat: {heard:?}");
    assert!(joined.is_some() && team.is_some() && joined < team, "{heard:?}");

    // The other team's class is refused; Elite Crew in the freeze time
    // plays this round.
    assert!(matches!(sim.server.app.world().resource::<RoundState>().phase, Phase::Freeze { .. }));
    client_console(&mut sim, i, "joinclass 6");
    sim.ticks(20);
    assert!(sim.server.app.world().get::<ChoosingClass>(theirs).is_some(), "a CT class refused");
    client_console(&mut sim, i, "joinclass 2");
    let ok = sim.until(200, |s| s.server.app.world().get::<Dead>(theirs).is_none());
    assert!(ok, "spawned");
    sim.ticks(20);
    let w = sim.server.app.world();
    assert_eq!(w.get::<PlayerClass>(theirs), Some(&PlayerClass(2)));
    assert!(w.get::<ChoosingClass>(theirs).is_none());
    let me = sim.local_player(i).unwrap();
    let cw = sim.clients[i].app.world();
    assert_eq!(cw.get::<PlayerClass>(me), Some(&PlayerClass(2)), "replicated");
    assert!(cw.get::<Dead>(me).is_none());
}

#[test]
fn a_client_asking_for_a_team_skips_the_menus() {
    let mut sim = server();
    let i = sim.add_client(|w| {
        w.insert_resource(JoinChoice::Team(Team(1)));
    });
    sim.until_joined(600);
    sim.ticks(30);
    let theirs = sim.character_of(i).unwrap();
    let w = sim.server.app.world();
    assert_eq!(w.get::<Team>(theirs), Some(&Team(1)));
    assert!(w.get::<Unassigned>(theirs).is_none() && w.get::<ChoosingClass>(theirs).is_none());
    assert!(w.get::<Dead>(theirs).is_none(), "plays in the freeze time");
    assert!(!sim.clients[i].app.world().resource::<JoinMenus>().motd, "no menus");
    // The first client (the default: auto-assign) is on a team too.
    let first = sim.character_of(0).unwrap();
    assert_eq!(sim.server.app.world().get::<Team>(first), Some(&Team(2)));
}

#[test]
fn single_player_starts_at_the_menus() {
    let mut sim = Sim::new(CsSim, JoinMenusStatePlugin);
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), crate::stack::movement::placeholder::ID);
    sim.app.world_mut().entity_mut(me).insert((LocalPlayer, Team(2)));
    sim.app.insert_resource(JoinChoice::Menus);
    sim.ticks(2);
    join_menus::start_game(sim.app.world_mut());
    sim.ticks(2);
    let w = sim.app.world();
    assert!(w.get::<Unassigned>(me).is_some() && w.get::<Spectating>(me).is_some());
    assert_eq!(w.get::<Team>(me), Some(&Team(0)));
    assert!(w.resource::<JoinMenus>().motd);
    // Deathmatch: no respawn while on no team, nor before a class.
    sim.ticks(200);
    assert!(sim.app.world().get::<Dead>(me).is_some());
    sim.app.world_mut().resource_mut::<Console>().submit("jointeam 3");
    sim.ticks(200);
    let w = sim.app.world();
    assert_eq!(w.get::<Team>(me), Some(&Team(2)));
    assert!(w.get::<ChoosingClass>(me).is_some() && w.get::<Dead>(me).is_some());
    assert_eq!(w.resource::<JoinMenus>().class, Some(2));
    // Auto-select: one of the counter-terrorists' classes, then a spawn.
    sim.app.world_mut().resource_mut::<Console>().submit("joinclass 0");
    sim.ticks(200);
    let w = sim.app.world();
    let class = w.get::<PlayerClass>(me).unwrap().0;
    assert!((5..=8).contains(&class), "class {class}");
    assert!(w.get::<Dead>(me).is_none() && w.get::<ChoosingClass>(me).is_none());

    // Without the menus (scripted runs): straight onto the team asked for.
    sim.app.insert_resource(JoinChoice::Team(Team(1)));
    join_menus::start_game(sim.app.world_mut());
    sim.ticks(200);
    let w = sim.app.world();
    assert_eq!(w.get::<Team>(me), Some(&Team(1)));
    assert!(w.get::<Unassigned>(me).is_none() && w.get::<ChoosingClass>(me).is_none());
    assert!(w.get::<Dead>(me).is_none());
}

#[test]
fn cl_disablehtmlmotd_shows_the_plain_text_message() {
    let mut sim = Sim::new(CsSim, JoinMenusStatePlugin);
    let motd = ServerMotd {
        server: "x".into(),
        html: "<html><body><pre>Welcome &amp; have fun</pre></body></html>".into(),
        text: "Plain welcome".into(),
    };
    let disabled = |sim: &Sim| sim.app.world().resource::<DisableHtmlMotd>().0 != 0;
    assert!(!disabled(&sim), "CS:S's default: 0");
    assert_eq!(join_menus::motd_shown(&motd, disabled(&sim)), "Welcome & have fun");
    sim.app.world_mut().resource_mut::<Console>().submit("cl_disablehtmlmotd 1");
    sim.ticks(1);
    assert!(disabled(&sim));
    assert_eq!(join_menus::motd_shown(&motd, disabled(&sim)), "Plain welcome");
}
