//! The party apps' main menu (`party_kit::main_menu`, M1 of
//! docs/plans/active/main-menu.md), headless: startup at the menu unless
//! the command line plays; Play's console lines and the game it starts
//! (the minigame once the map is in); Join on `NetSim`'s in-memory link
//! (in the game; a refused join, a full server and a host leaving land on
//! the menu with why); Cancel while joining and loading; the Escape
//! menu's Disconnect (back at the menu, a host with people connected asked
//! first); Combat Arms' loading screen kept to its own maps; the data.

use bevy::prelude::*;
use chimera_client::client::{AppHooks, front_end};
use chimera_core::console::{Console, ConsoleAppExt};
use chimera_net::{
    harness::NetSim,
    net::{self, NetSettings, client::JoinProgress},
};
use chimera_sim::minigame::ActiveMinigame;
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{game::TICK_INTERVAL, harness::CaSim};
use party_kit::{
    DEFAULT_MINIGAME, PartyPartsPlugin,
    esc_menu::{self, EscMenu, EscMenuPlugin},
    main_menu::{self, Field, MainMenuAction as A, MainMenuPlugin, MenuData, MenuPage, PartyMenu, PlayKind, Screen},
};

/// The menu's plugins on an app (the client's side: the shell's network
/// glue and console commands, the Escape menu).
fn menu_bits(app: &mut App, at_start: bool) {
    app.insert_resource(AppHooks {
        build: "test",
        load_map: |id, _| Err(format!("{id}: not in tests")),
        load_map_file: |id, _, _| Err(format!("{id}: not in tests")),
        map_file_bytes: |_| None,
        unload_greybox: |_| {},
        respawn_greybox: |_| {},
    })
    .init_resource::<ButtonInput<KeyCode>>()
    .insert_resource(chimera_client::client::ClientArgs(
        chimera_client::client::Args::default(),
    ))
    .add_plugins((
        chimera_client::client::net::ClientNetPlugin,
        EscMenuPlugin,
        MainMenuPlugin {
            at_start,
            minigame: DEFAULT_MINIGAME.into(),
            keep_recent: false,
        },
    ));
    chimera_client::client::console::client_commands(app);
}

/// A single-player party app at the menu (or in the game).
fn local(at_start: bool) -> Sim {
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin, net::NetPlugin));
        menu_bits(app, at_start);
        // `map` as the shell's, without loading: the map "comes in" at once.
        app.console_command("map", "a test's", |w, a| {
            w.resource_mut::<Maps>().0.push(a.join(" "));
            if w.contains_resource::<NeverLoads>() {
                return Ok(None);
            }
            front_end::entered_game(w);
            Ok(None)
        });
        app.init_resource::<Maps>();
    });
    sim.ticks(2);
    sim
}

#[derive(Resource, Default)]
struct Maps(Vec<String>);

/// `map` loads nothing: the load never finishes (the shell's would load in
/// the background).
fn replace_map_never_loads(sim: &mut Sim) {
    sim.app.world_mut().resource_mut::<Maps>().0.clear();
    sim.app.world_mut().insert_resource(NeverLoads);
}

#[derive(Resource)]
struct NeverLoads;

fn act(sim: &mut Sim, a: A) {
    sim.app.world_mut().write_message(a);
    sim.ticks(2);
}

fn menu(sim: &Sim) -> &PartyMenu {
    sim.app.world().resource::<PartyMenu>()
}

fn submit(sim: &mut Sim, line: &str) {
    sim.app.world_mut().resource_mut::<Console>().submit(line);
    sim.ticks(2);
}

#[test]
fn the_command_line_decides_the_menu() {
    use chimera_client::client::Args;
    let args = |s: &str| Args::parse_from(s.split_whitespace().map(str::to_string)).unwrap();
    assert!(main_menu::starts_at_menu(&args("")));
    assert!(main_menu::starts_at_menu(&args("--window 1280x720 +name Ann")));
    for playing in [
        "--map cs_source:de_dust2",
        "+minigame ultimate_gun_game:auto",
        "+connect 127.0.0.1:28000",
        "--screenshot a.png",
        "--frames 90",
        "--spawn 0,1,0",
    ] {
        assert!(!main_menu::starts_at_menu(&args(playing)), "{playing} skips the menu");
    }
}

#[test]
fn startup_without_a_map_opens_the_menu_and_with_one_does_not() {
    let sim = local(true);
    assert_eq!(menu(&sim).screen, Screen::MainMenu);
    let w = sim.app.world();
    assert!(
        w.resource::<chimera_client::client::front_end::AppMenu>().open,
        "the menu has the mouse"
    );
    let sim = local(false);
    assert_eq!(menu(&sim).screen, Screen::InGame);
    assert!(
        !sim.app
            .world()
            .resource::<chimera_client::client::front_end::AppMenu>()
            .open
    );
}

#[test]
fn every_entry_and_row_of_the_data_works() {
    let data = MenuData::shipped();
    let labels: Vec<&str> = data.entries.iter().map(|e| e.label.as_str()).collect();
    assert_eq!(labels, ["Play", "Join", "Options", "Quit"]);
    assert!(
        data.escape
            .iter()
            .any(|e| e.does == main_menu::EscDoes::Disconnect && e.when == main_menu::When::MainMenu)
    );
    assert!(!data.background.maps.is_empty());
    // Every Play row's cvar and every console line an entry runs exists.
    let sim = local(true);
    let console = sim.app.world().resource::<Console>();
    for r in &data.play {
        if r.kind != PlayKind::Map {
            assert!(
                console.cvar(&r.cvar).is_some(),
                "Play row {}: no cvar {}",
                r.label,
                r.cvar
            );
        }
    }
    for e in &data.entries {
        if let main_menu::EntryDoes::Command(c) = &e.does {
            let name = c.split_whitespace().next().unwrap();
            assert!(
                console.command(name).is_some() || console.cvar(name).is_some(),
                "{}: no command {name}",
                e.label
            );
        }
    }
    assert!(MenuData::parse("[background]\n[[entry]]\nlabel = \"x\"\npage = \"nowhere\"").is_err());
}

#[test]
fn play_sets_the_server_up_loads_the_map_and_starts_the_minigame() {
    let mut sim = local(true);
    act(&mut sim, A::Entry(0));
    assert_eq!(menu(&sim).page, MenuPage::Play);
    // Five bots, a password.
    let data = MenuData::shipped();
    let bots = data.play.iter().position(|r| r.cvar == "MaxPlayer").unwrap();
    act(&mut sim, A::PlayStep { row: bots, dir: 1 });
    act(&mut sim, A::PlayStep { row: bots, dir: 1 });
    let pw = data.play.iter().position(|r| r.cvar == "sv_password").unwrap();
    act(&mut sim, A::Focus(Field::Play(pw)));
    submit(&mut sim, &format!("mainmenu_type {pw} secret"));
    let (lines, after, map) = main_menu::start_lines(
        &data,
        menu(&sim),
        &main_menu::map_pool(sim.app.world()),
        DEFAULT_MINIGAME,
    );
    assert!(
        lines.contains(&"MaxPlayer 5".to_string()) && lines.contains(&"maxplayers 8".to_string()),
        "{lines:?}"
    );
    assert!(lines.contains(&"sv_password secret".to_string()));
    assert_eq!(lines.last().unwrap(), &format!("map {map}"));
    assert_eq!(map, "cs_source:de_dust2", "the rotation's first map");
    assert_eq!(after, [format!("minigame {DEFAULT_MINIGAME}")]);
    act(&mut sim, A::Start);
    sim.ticks(4);
    let w = sim.app.world();
    assert_eq!(w.resource::<Maps>().0, ["cs_source:de_dust2"]);
    assert_eq!(
        w.resource::<NetSettings>().maxplayers,
        8,
        "a joinable server: it listens once the map is in"
    );
    assert_eq!(menu(&sim).screen, Screen::InGame);
    assert_eq!(
        w.get_resource::<ActiveMinigame>().map(|m| m.def.id.as_str()),
        Some(DEFAULT_MINIGAME)
    );
}

#[test]
fn a_map_of_the_pool_is_picked_by_its_row() {
    let mut sim = local(true);
    let pool = main_menu::map_pool(sim.app.world());
    assert!(pool.len() >= 2, "{pool:?}");
    let row = MenuData::shipped()
        .play
        .iter()
        .position(|r| r.kind == PlayKind::Map)
        .unwrap();
    act(&mut sim, A::Entry(0));
    act(&mut sim, A::PlayStep { row, dir: 1 });
    act(&mut sim, A::PlayStep { row, dir: 1 });
    assert_eq!(menu(&sim).play[row], pool[1]);
    act(&mut sim, A::Start);
    assert_eq!(sim.app.world().resource::<Maps>().0, [pool[1].clone()]);
}

#[test]
fn cancel_while_loading_returns_to_the_menu() {
    let mut sim = local(true);
    // A load that doesn't finish at once (the shell's `map` would load in
    // the background): the menu shows the loading panel.
    replace_map_never_loads(&mut sim);
    act(&mut sim, A::Entry(0));
    act(&mut sim, A::Start);
    assert_eq!(menu(&sim).screen, Screen::Loading);
    assert!(menu(&sim).loading.is_some());
    act(&mut sim, A::Cancel);
    sim.ticks(2);
    assert_eq!(
        menu(&sim).screen,
        Screen::MainMenu,
        "Cancel is `disconnect`: the menu again"
    );
    assert!(menu(&sim).failure.is_none(), "leaving by choice isn't a failure");
}

#[test]
fn a_map_change_in_a_game_shows_the_loading_panel_until_it_is_in() {
    let mut sim = local(false);
    submit(&mut sim, "map cs_source:de_nuke");
    // The test's `map` is in at once.
    assert_eq!(menu(&sim).screen, Screen::InGame);
    front_end::joining(sim.app.world_mut(), "de_nuke");
    sim.ticks(1);
    assert_eq!(menu(&sim).screen, Screen::Joining, "a server's level change");
    front_end::entered_game(sim.app.world_mut());
    sim.ticks(1);
    assert_eq!(menu(&sim).screen, Screen::InGame);
}

/// A server playing the gun game on the greybox, and a client at the main
/// menu whose `connect` joins it over the in-memory link.
fn server_and_menu_client() -> (NetSim, usize) {
    with_menu_client(crate::net_party::party(0, None))
}

/// `sim` with a client at the main menu added.
fn with_menu_client(mut sim: NetSim) -> (NetSim, usize) {
    let link = sim.link.clone();
    let mut client = Sim::with(CaSim, |app| {
        app.add_plugins(net::NetPlugin);
        app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin));
        menu_bits(app, true);
    });
    client
        .app
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f64(TICK_INTERVAL),
        ));
    client
        .app
        .console_command("connect", "a test's: the in-memory link", move |w, a| {
            assert!(!a.is_empty());
            net::memory::join(w, link.clone(), 77)?;
            Ok(None)
        });
    sim.clients.push(client);
    let i = sim.clients.len() - 1;
    sim.ticks(2);
    (sim, i)
}

fn client_menu(sim: &NetSim, i: usize) -> &PartyMenu {
    sim.clients[i].app.world().resource::<PartyMenu>()
}

fn client_act(sim: &mut NetSim, i: usize, a: A) {
    sim.clients[i].app.world_mut().write_message(a);
    sim.ticks(2);
}

fn type_in(sim: &mut NetSim, i: usize, line: &str) {
    sim.clients[i].app.world_mut().resource_mut::<Console>().submit(line);
    sim.ticks(2);
}

#[test]
fn join_connects_with_the_password_and_ends_in_the_game() {
    let (mut sim, c) = server_and_menu_client();
    client_act(&mut sim, c, A::Entry(1));
    assert_eq!(client_menu(&sim, c).page, MenuPage::Join);
    type_in(&mut sim, c, "mainmenu_type address 127.0.0.1:28015");
    type_in(&mut sim, c, "mainmenu_type password hunter2");
    client_act(&mut sim, c, A::Connect);
    assert!(matches!(client_menu(&sim, c).screen, Screen::Joining | Screen::InGame));
    let joined = sim.until(900, |s| {
        s.clients[c].app.world().resource::<PartyMenu>().screen == Screen::InGame
    });
    assert!(joined, "in the game: {:?}", client_menu(&sim, c));
    // The Escape menu's sync gives the mouse back the next frame.
    sim.ticks(2);
    let w = sim.clients[c].app.world();
    assert_eq!(
        w.resource::<chimera_net::net::query::JoinPassword>().0,
        "hunter2",
        "the password went with it"
    );
    assert_eq!(client_menu(&sim, c).recent, ["127.0.0.1:28015"], "remembered");
    assert!(!w.resource::<chimera_client::client::front_end::AppMenu>().open, "the game has the mouse");
}

#[test]
fn a_refused_join_lands_on_the_menu_with_why() {
    let (mut sim, c) = server_and_menu_client();
    // The in-memory link carries no password: any is wrong.
    sim.server
        .app
        .world_mut()
        .resource_mut::<Console>()
        .submit("sv_password letmein");
    sim.ticks(2);
    type_in(&mut sim, c, "mainmenu_type address 127.0.0.1");
    client_act(&mut sim, c, A::Connect);
    let back = sim.until(900, |s| {
        s.clients[c].app.world().resource::<PartyMenu>().failure.is_some()
    });
    assert!(back, "{:?}", client_menu(&sim, c));
    let m = client_menu(&sim, c);
    assert_eq!(m.screen, Screen::MainMenu);
    assert!(
        m.failure.as_deref().unwrap().to_lowercase().contains("password"),
        "{:?}",
        m.failure
    );
    // Close: the menu again.
    client_act(&mut sim, c, A::CloseFailure);
    assert!(client_menu(&sim, c).failure.is_none());
    assert!(!sim.clients[c].app.world().resource::<JoinProgress>().showing());
}

#[test]
fn a_full_server_refuses_and_the_menu_says_so() {
    let mut sim = crate::net_party::party(1, None);
    sim.server.app.world_mut().resource_mut::<NetSettings>().maxplayers = 1;
    sim.until_joined(600);
    let (mut sim, c) = with_menu_client(sim);
    type_in(&mut sim, c, "mainmenu_type address 127.0.0.1");
    client_act(&mut sim, c, A::Connect);
    let back = sim.until(900, |s| {
        s.clients[c].app.world().resource::<PartyMenu>().failure.is_some()
    });
    assert!(back, "{:?}", client_menu(&sim, c));
    assert!(
        client_menu(&sim, c).failure.as_deref().unwrap().contains("full"),
        "{:?}",
        client_menu(&sim, c).failure
    );
}

#[test]
fn cancel_while_joining_returns_to_the_menu() {
    let (mut sim, c) = server_and_menu_client();
    type_in(&mut sim, c, "mainmenu_type address 127.0.0.1");
    client_act(&mut sim, c, A::Connect);
    assert_eq!(client_menu(&sim, c).screen, Screen::Joining);
    client_act(&mut sim, c, A::Cancel);
    sim.ticks(4);
    let m = client_menu(&sim, c);
    assert_eq!(m.screen, Screen::MainMenu, "{m:?}");
    assert!(m.failure.is_none());
    assert_eq!(
        sim.clients[c].app.world().resource::<chimera_core::core::NetRole>(),
        &chimera_core::core::NetRole::Standalone
    );
}

#[test]
fn disconnect_from_the_escape_menu_goes_back_to_the_menu() {
    let (mut sim, c) = server_and_menu_client();
    type_in(&mut sim, c, "mainmenu_type address 127.0.0.1");
    client_act(&mut sim, c, A::Connect);
    assert!(
        sim.until(900, |s| s.clients[c].app.world().resource::<PartyMenu>().screen
            == Screen::InGame)
    );
    // A client leaves at once (no confirm).
    let w = sim.clients[c].app.world_mut();
    w.resource_mut::<EscMenu>().set_open(true);
    w.write_message(esc_menu::Action::Disconnect);
    sim.ticks(4);
    let m = client_menu(&sim, c);
    assert_eq!(m.screen, Screen::MainMenu);
    assert!(m.failure.is_none(), "leaving by choice: {:?}", m.failure);
    assert!(!sim.clients[c].app.world().resource::<EscMenu>().open);
    // The local player is fresh, the menu has the mouse.
    let w = sim.clients[c].app.world_mut();
    assert!(main_menu::local_player(w).is_some(), "a local player behind the menu");
    assert!(w.resource::<chimera_client::client::front_end::AppMenu>().open);
}

#[test]
fn a_hosts_disconnect_asks_and_drops_its_clients_with_host_left() {
    // The server is a listen server: its own player, the menu's plugins.
    let (mut sim, c) = with_menu_client(crate::net_party::party_and(0, None, |app| menu_bits(app, false)));
    let host = sim
        .server
        .spawn_character(Vec3::new(0.0, 1.0, 0.0), combat_arms::game::movement::ID);
    sim.server
        .app
        .world_mut()
        .entity_mut(host)
        .insert(chimera_core::core::LocalPlayer);
    sim.ticks(2);
    type_in(&mut sim, c, "mainmenu_type address 127.0.0.1");
    client_act(&mut sim, c, A::Connect);
    assert!(
        sim.until(900, |s| s.clients[c].app.world().resource::<PartyMenu>().screen
            == Screen::InGame)
    );
    sim.ticks(30);
    // Disconnect asks first: one player would lose the game.
    let w = sim.server.app.world_mut();
    w.resource_mut::<EscMenu>().set_open(true);
    w.write_message(esc_menu::Action::Disconnect);
    sim.ticks(2);
    assert_eq!(
        sim.server.app.world().resource::<EscMenu>().page,
        esc_menu::Page::ConfirmDisconnect(1)
    );
    assert_eq!(
        sim.server.app.world().resource::<chimera_core::core::NetRole>(),
        &chimera_core::core::NetRole::Server,
        "not yet"
    );
    sim.server
        .app
        .world_mut()
        .write_message(esc_menu::Action::DisconnectYes);
    let told = sim.until(600, |s| {
        s.clients[c].app.world().resource::<PartyMenu>().failure.is_some()
    });
    assert!(told, "{:?}", client_menu(&sim, c));
    assert_eq!(client_menu(&sim, c).failure.as_deref(), Some(main_menu::HOST_LEFT));
    assert_eq!(sim.server.app.world().resource::<PartyMenu>().screen, Screen::MainMenu);
}

#[test]
fn combat_arms_loading_screen_shows_only_for_its_maps() {
    use combat_arms::ui::loading::CaLoading;
    let mut sim = local(false);
    sim.app.world_mut().init_resource::<CaLoading>();
    sim.app.world_mut().init_resource::<JoinProgress>();
    let mut set = |sim: &mut Sim, label: &str, map: Option<&str>| {
        sim.app.world_mut().resource_mut::<CaLoading>().label = Some(label.into());
        sim.app.world_mut().resource_mut::<JoinProgress>().map = map.map(str::to_string);
        sim.ticks(1);
        sim.app.world().resource::<CaLoading>().label.clone()
    };
    assert_eq!(
        set(&mut sim, "127.0.0.1:27015", None),
        None,
        "the address: the party's panel"
    );
    assert_eq!(
        set(&mut sim, "de_dust2", Some("cs_source:de_dust2")),
        None,
        "CS:S's map: the party's panel"
    );
    assert_eq!(
        set(&mut sim, "warehouse", Some("combat_arms:warehouse")).as_deref(),
        Some("warehouse"),
        "its own map"
    );
}

#[test]
fn recent_addresses_are_kept_newest_first() {
    let mut r = vec!["a".to_string(), "b".to_string()];
    main_menu::remember(&mut r, "b");
    assert_eq!(r, ["b", "a"]);
    for i in 0..10 {
        main_menu::remember(&mut r, &format!("h{i}"));
    }
    assert_eq!(r.len(), main_menu::RECENT);
    assert_eq!(r[0], "h9");
}
