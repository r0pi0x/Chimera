//! Hosting from the party apps' main menu goes through the pre-match lobby
//! (`chimera_net::prematch`): Play's Start (its Server tab's lobby rows:
//! `mp_lobby 1`, `mp_lobby_min_players 2`, `mp_lobby_ready_up 1`) on a
//! listen host with its own player and bots holds the gun game's first
//! match in the lobby; the host sees the panel (its row tagged, the bots
//! ready, its key starting the match); alone it doesn't count down; a
//! joiner from the menu joins during the lobby and sees it (not ready
//! until it says so, `lobby_ready` from its key, and back); the host's key
//! starts the countdown and the countdown the match, with everyone in it.
//! A dedicated server's lobby keeps players ready once they have the map
//! (`mp_lobby_ready_up` 0 by default). Ticks only.

use std::time::Duration;

use bevy::prelude::*;
use chimera_client::client::{
    front_end,
    prematch_lobby::{LobbyView, PreMatchLobbyPlugin},
};
use chimera_core::{
    console::{Console, ConsoleAppExt},
    core::LocalPlayer,
};
use chimera_net::{
    harness::NetSim,
    net::{self, memory::LinkConditions},
    prematch::{LobbySettings, NetLobby, status},
};
use chimera_sim::minigame::{ActiveMinigame, PreMatch};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{game::TICK_INTERVAL, harness::CaSim};
use party_kit::{
    DEFAULT_MINIGAME, PartyPartsPlugin,
    main_menu::{MainMenuAction as A, MenuData, PartyMenu, Screen},
};

use crate::main_menu::menu_bits;

/// The joiner's client id on the in-memory link.
const JOINER: u64 = 78;

const HZ: u64 = (1.0 / TICK_INTERVAL) as u64;

/// The `map` of the menu's Start: in at once (the shell's loads it).
fn instant_map(app: &mut App) {
    app.console_command("map", "a test's", |w, _| {
        front_end::entered_game(w);
        Ok(None)
    });
}

/// A listen host at its main menu (the menu, the lobby's panel, its own
/// player), serving on the in-memory link.
fn host_at_menu() -> NetSim {
    let mut sim = NetSim::new(
        CaSim,
        LinkConditions {
            latency: Duration::from_millis(20),
            jitter: Duration::ZERO,
            loss: 0.0,
        },
        47,
        0,
        |app: &mut App| {
            app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin, PreMatchLobbyPlugin));
            menu_bits(app, true);
            instant_map(app);
        },
    );
    sim.set_frame(TICK_INTERVAL);
    let host = sim.server.spawn_character(Vec3::new(0.0, 1.0, 0.0), combat_arms::game::movement::ID);
    sim.server.app.world_mut().entity_mut(host).insert(LocalPlayer);
    sim.ticks(2);
    sim
}

/// A client at the menu, joining with Join's Connect (the in-memory link).
fn add_menu_client(sim: &mut NetSim) -> usize {
    let link = sim.link.clone();
    let mut client = Sim::with(CaSim, |app| {
        app.add_plugins(net::NetPlugin);
        app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin, PreMatchLobbyPlugin));
        menu_bits(app, true);
    });
    client
        .app
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(TICK_INTERVAL)));
    client.app.console_command("connect", "a test's: the in-memory link", move |w, _| {
        net::memory::join(w, link.clone(), JOINER)?;
        Ok(None)
    });
    sim.clients.push(client);
    let i = sim.clients.len() - 1;
    sim.ticks(2);
    i
}

fn server_act(sim: &mut NetSim, a: A) {
    sim.server.app.world_mut().write_message(a);
    sim.ticks(2);
}

fn client_act(sim: &mut NetSim, i: usize, a: A) {
    sim.clients[i].app.world_mut().write_message(a);
    sim.ticks(2);
}

fn host_view(sim: &NetSim) -> LobbyView {
    sim.server.app.world().resource::<LobbyView>().clone()
}

fn client_view(sim: &NetSim, i: usize) -> LobbyView {
    sim.clients[i].app.world().resource::<LobbyView>().clone()
}

/// The lobby's key on an app: the line its view says it runs.
fn press_lobby_key(app: &mut App) {
    let line = app.world().resource::<LobbyView>().key_line().expect("the key does something now");
    app.world_mut().resource_mut::<Console>().submit(line);
}

fn in_lobby(sim: &NetSim) -> bool {
    sim.server.app.world().contains_resource::<PreMatch>()
}

#[test]
fn hosting_from_the_menu_holds_the_match_in_the_lobby_until_the_host_starts_it() {
    let mut sim = host_at_menu();
    // Play, then Start with the Server tab's rows as shipped.
    let data = MenuData::shipped();
    for (cvar, want) in [("mp_lobby", "1"), ("mp_lobby_min_players", "2"), ("mp_lobby_ready_up", "1")] {
        let row = data.play.iter().find(|r| r.cvar == cvar).unwrap_or_else(|| panic!("a {cvar} row"));
        assert_eq!(row.default.as_deref(), Some(want), "{cvar}'s default");
    }
    server_act(&mut sim, A::Entry(0));
    server_act(&mut sim, A::Start);
    assert!(
        sim.until(5 * HZ, |s| s.server.app.world().resource::<PartyMenu>().screen == Screen::InGame
            && s.server.app.world().contains_resource::<ActiveMinigame>()),
        "in the game with the minigame: {:?}",
        sim.server.app.world().resource::<PartyMenu>().screen
    );
    let w = sim.server.app.world();
    assert_eq!(w.resource::<ActiveMinigame>().def.id, DEFAULT_MINIGAME);
    let s = *w.resource::<LobbySettings>();
    assert_eq!((s.enabled, s.min_players, s.ready_up), (1, 2, 1), "Start set the lobby's rows");

    // The lobby: the host sees the panel, its own row tagged and its key
    // starting the match; bots are ready.
    assert!(sim.until(2 * HZ, |s| host_view(s).shown), "the host sees the lobby");
    assert!(in_lobby(&sim), "the match is held");
    let v = host_view(&sim);
    assert!(v.hosting);
    let me = v.rows.iter().find(|r| r.local).expect("the host's own row");
    assert!(me.host && !me.bot, "{me:?}");
    assert_eq!(me.status, status::READY);
    assert!(v.rows.iter().filter(|r| r.bot).all(|r| r.status == status::READY && r.ping == "BOT"), "{v:?}");
    assert_eq!(v.key_line(), Some("lobby_start"));
    assert_eq!(v.hint, "Press F3 to start the match now");
    // Alone (waiting for a second player): no countdown yet.
    sim.ticks(3 * HZ);
    assert!(in_lobby(&sim) && host_view(&sim).countdown.is_none(), "{:?}", host_view(&sim));
    assert!(host_view(&sim).line.starts_with("Waiting for 1 player"), "{}", host_view(&sim).line);

    // A joiner from the menu, during the lobby: it sees it, not ready
    // until it says so.
    let c = add_menu_client(&mut sim);
    client_act(&mut sim, c, A::Entry(1));
    sim.clients[c].app.world_mut().resource_mut::<Console>().submit("mainmenu_type address 127.0.0.1");
    client_act(&mut sim, c, A::Connect);
    let not_ready = |s: &mut NetSim| client_view(s, c).rows.iter().any(|r| r.local && r.status == status::NOT_READY);
    assert!(sim.until(30 * HZ, not_ready), "the joiner sees itself not ready: {:?}", client_view(&sim, c));
    let v = client_view(&sim, c);
    assert!(v.shown && !v.hosting && v.countdown.is_none());
    assert!(v.rows.iter().any(|r| r.host && !r.local), "the joiner sees the host tagged: {v:?}");
    assert_eq!(v.hint, "Press F3 when you are ready");
    assert!(in_lobby(&sim));
    // Ready, then not: the host hears both.
    let id = JOINER;
    let server_status = |s: &mut NetSim| {
        let mut q = s.server.app.world_mut().query::<&NetLobby>();
        q.iter(s.server.app.world()).next().and_then(|l| l.rows.iter().find(|r| r.id == Some(id)).map(|r| r.status))
    };
    // (The host waits for a third friend: everyone ready doesn't count
    // down yet.)
    sim.server.app.world_mut().resource_mut::<Console>().submit("mp_lobby_min_players 3");
    press_lobby_key(&mut sim.clients[c].app);
    assert!(sim.until(HZ, |s| server_status(s) == Some(status::READY)), "ready on the server");
    assert!(
        sim.until(HZ, |s| client_view(s, c).rows.iter().any(|r| r.local && r.status == status::READY)),
        "the joiner sees itself ready"
    );
    assert!(host_view(&sim).countdown.is_none(), "waiting for the third");
    press_lobby_key(&mut sim.clients[c].app);
    assert!(sim.until(HZ, |s| server_status(s) == Some(status::NOT_READY)), "not ready again");

    // The host starts it early: the countdown, then the match with
    // everyone.
    press_lobby_key(&mut sim.server.app);
    assert!(
        sim.until(HZ, |s| host_view(s).countdown.is_some() && client_view(s, c).countdown.is_some()),
        "the countdown on the host and the joiner"
    );
    assert_eq!(host_view(&sim).key_line(), None, "a countdown under way isn't started again");
    assert!(sim.until(7 * HZ, |s| !in_lobby(s)), "the countdown starts the match");
    assert!(
        sim.until(HZ, |s| !host_view(s).shown && !client_view(s, c).shown),
        "the panel goes on the host and the joiner"
    );
    assert!(NetSim::owned_by(&mut sim.server, Some(JOINER)).is_some(), "the joiner plays in the match");
}

#[test]
fn a_dedicated_servers_lobby_keeps_players_ready_once_they_have_the_map() {
    assert_eq!(LobbySettings::default().ready_up, 0, "ready-up is the menu host's, not a server's default");
    assert_eq!(LobbySettings::default().min_players, 1);
}
