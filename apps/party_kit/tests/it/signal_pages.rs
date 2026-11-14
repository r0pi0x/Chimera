//! The main menu's Play and Join in the Signal look (`party_kit::
//! signal_pages`, ugg-ui.md M5, main-menu.md M2), headless on a manual UI
//! clock: drawn only for a theme with `[layout.pages]` (the VGUI frames
//! otherwise); on Play the keys move the focus, change a row and Enter
//! starts the game (its map loaded, the menu in the game); on Join Enter
//! connects to the typed address, refuses an empty one, and joins a LAN
//! server of the list (a locked one asks for the password first); the LAN
//! list's model (ours only, by ping); `ui_scene play|join`.

use std::net::SocketAddr;

use bevy::prelude::*;
use chimera_core::console::{Console, ConsoleAppExt};
use chimera_net::net::{
    NET_VERSION,
    query::{GAME_FOLDER, GAME_NAME, LanSettings, PROTOCOL, QueryResult, QueryState, ServerInfo, ServerQueries},
};
use chimera_world::harness::Sim;
use party_kit::{
    main_menu::{MainMenuAction as A, MenuData, MenuPage, PartyMenu, Screen},
    signal_pages::{self, LanList, Line, SignalPages, SignalPagesRoot},
};

use super::signal_menu::sim_with;

/// The menu's and the pages' layouts (numbers default).
const PAGES_THEME: &str = r#"
name = "test"
[shape]
slant_deg = 12.0
[layout.menu]
items_y = 23.4
[layout.pages]
row_h = 2.9
"#;

/// What the stand-in `map` and `connect` were asked.
#[derive(Resource, Default)]
struct Asked(Vec<String>);

fn sim(theme: &str) -> Sim {
    let mut sim = sim_with(theme);
    sim.app
        .init_resource::<Asked>()
        // Nothing on the network: the LAN scan asks a dead port here only.
        .insert_resource(LanSettings { ports: "39871".into(), broadcast: String::new() })
        .console_command("map", "a test's: comes in at once", |w, a| {
            w.resource_mut::<Asked>().0.push(format!("map {}", a.join(" ")));
            chimera_client::client::front_end::entered_game(w);
            Ok(None)
        })
        .console_command("connect", "a test's: asked only", |w, a| {
            w.resource_mut::<Asked>().0.push(format!("connect {}", a.join(" ")));
            Ok(None)
        });
    sim.ticks(2);
    sim
}

fn press(sim: &mut Sim, key: KeyCode) {
    sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
    sim.ticks(1);
    let mut k = sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    k.release(key);
    k.clear();
    sim.ticks(1);
}

fn act(sim: &mut Sim, a: A) {
    sim.app.world_mut().write_message(a);
    sim.ticks(2);
}

fn submit(sim: &mut Sim, line: &str) {
    sim.app.world_mut().resource_mut::<Console>().submit(line);
    sim.ticks(2);
}

fn menu(sim: &Sim) -> &PartyMenu {
    sim.app.world().resource::<PartyMenu>()
}

fn pages(sim: &Sim) -> &SignalPages {
    sim.app.world().resource::<SignalPages>()
}

fn asked(sim: &Sim) -> Vec<String> {
    sim.app.world().resource::<Asked>().0.clone()
}

fn roots(sim: &mut Sim) -> usize {
    sim.app.world_mut().query_filtered::<(), With<SignalPagesRoot>>().iter(sim.app.world()).count()
}

fn server(port: u16, name: &str, ping_ms: u32, password: bool, folder: &str) -> QueryResult {
    QueryResult {
        addr: SocketAddr::from(([127, 0, 0, 1], port)),
        state: QueryState::Answered {
            info: ServerInfo {
                protocol: PROTOCOL,
                name: name.into(),
                map: "de_dust2".into(),
                folder: folder.into(),
                game: GAME_NAME.into(),
                app_id: 0,
                players: 3,
                max_players: 8,
                bots: 2,
                dedicated: false,
                os: 'l',
                password,
                secure: false,
                version: NET_VERSION.into(),
                port: Some(port),
                instance: Some(port as u64),
                keywords: Some("cs_source,minigame=ultimate_gun_game:auto".into()),
            },
            ping_ms,
        },
        lan: true,
    }
}

#[test]
fn without_the_pages_layout_play_is_the_vgui_frame() {
    let mut sim = sim(super::signal_menu::MENU_THEME);
    act(&mut sim, A::Entry(0));
    assert_eq!(menu(&sim).page, MenuPage::Play);
    assert_eq!(roots(&mut sim), 0);
    assert_eq!(pages(&sim).up, None);
}

#[test]
fn play_keys_change_a_row_and_enter_starts_the_game() {
    let mut sim = sim(PAGES_THEME);
    // Enter on the first entry opens Play.
    press(&mut sim, KeyCode::Enter);
    sim.ticks(2);
    assert_eq!(menu(&sim).page, MenuPage::Play);
    assert_eq!(roots(&mut sim), 1, "drawn");
    assert_eq!(pages(&sim).up, Some(MenuPage::Play));
    assert_eq!(pages(&sim).focus, 0, "the map row first");
    let data = MenuData::shipped();
    let bots = data.play.iter().position(|r| r.cvar == "MaxPlayer").unwrap();
    for _ in 0..bots {
        press(&mut sim, KeyCode::ArrowDown);
    }
    assert_eq!(pages(&sim).focus, bots);
    let before: u32 = menu(&sim).play[bots].parse().unwrap();
    press(&mut sim, KeyCode::ArrowRight);
    press(&mut sim, KeyCode::ArrowRight);
    sim.ticks(1);
    assert_eq!(menu(&sim).play[bots], (before + 2).to_string(), "→ steps the bots");
    // A text row takes the keys (the menu's field follows the focus).
    let name = data.play.iter().position(|r| r.cvar == "hostname").unwrap();
    for _ in bots..name {
        press(&mut sim, KeyCode::ArrowDown);
    }
    assert_eq!(menu(&sim).focus, Some(party_kit::main_menu::Field::Play(name)));
    // Q switches to the Game tab, E back; not while typing.
    press(&mut sim, KeyCode::KeyQ);
    assert_eq!(menu(&sim).play_tab, 0, "typing: Q is a letter");
    press(&mut sim, KeyCode::ArrowUp);
    press(&mut sim, KeyCode::KeyQ);
    sim.ticks(1);
    assert_eq!(menu(&sim).play_tab, 1);
    assert_eq!(pages(&sim).focus, 0, "a tab starts at its first row");
    press(&mut sim, KeyCode::KeyE);
    sim.ticks(1);
    assert_eq!(menu(&sim).play_tab, 0);
    press(&mut sim, KeyCode::Enter);
    sim.ticks(4);
    let a = asked(&sim);
    assert!(a.iter().any(|l| l.starts_with("map ")), "{a:?}");
    assert_eq!(menu(&sim).screen, Screen::InGame);
    assert_eq!(pages(&sim).up, None);
    assert_eq!(roots(&mut sim), 0, "gone in the game");
}

#[test]
fn join_enter_connects_and_refuses_an_empty_address() {
    let mut sim = sim(PAGES_THEME);
    act(&mut sim, A::Entry(1));
    assert_eq!(pages(&sim).up, Some(MenuPage::Join));
    assert_eq!(menu(&sim).focus, Some(party_kit::main_menu::Field::Address), "the address takes the keys");
    press(&mut sim, KeyCode::Enter);
    sim.ticks(1);
    assert!(pages(&sim).notice.is_some(), "why");
    assert_eq!(pages(&sim).error, Some(0));
    assert!(asked(&sim).is_empty());
    assert_eq!(menu(&sim).failure, None, "no failure box");
    submit(&mut sim, "mainmenu_type address 192.0.2.7:27016");
    press(&mut sim, KeyCode::Enter);
    sim.ticks(1);
    assert_eq!(asked(&sim), ["connect 192.0.2.7:27016"]);
    assert_eq!(menu(&sim).screen, Screen::Joining, "the loading panel's");
    assert_eq!(menu(&sim).recent, ["192.0.2.7:27016"]);
}

#[test]
fn the_lan_list_is_ours_by_ping() {
    let results = vec![
        server(28001, "Slow", 40, false, GAME_FOLDER),
        server(28002, "Fast", 4, false, GAME_FOLDER),
        server(28003, "A Source server", 1, false, "cstrike"),
        QueryResult { lan: false, ..server(28004, "Asked by address", 2, false, GAME_FOLDER) },
        QueryResult { addr: SocketAddr::from(([127, 0, 0, 1], 28005)), state: QueryState::Pending, lan: true },
    ];
    let list = signal_pages::lan_servers(&results);
    let names: Vec<&str> = list.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["Fast", "Slow"]);
    assert_eq!(list[0].minigame.as_deref(), Some("ultimate_gun_game:auto"));
    assert_eq!((list[0].players, list[0].max_players, list[0].bots), (3, 8, 2));
}

#[test]
fn join_lists_lan_servers_and_enter_joins_one() {
    let mut sim = sim(PAGES_THEME);
    act(&mut sim, A::Entry(1));
    {
        let mut q = sim.app.world_mut().resource_mut::<ServerQueries>();
        q.results = vec![server(28011, "Locked", 9, true, GAME_FOLDER), server(28012, "Open", 3, false, GAME_FOLDER)];
        q.generation += 100;
    }
    sim.ticks(2);
    let names: Vec<String> = sim.app.world().resource::<LanList>().servers.iter().map(|s| s.name.clone()).collect();
    assert_eq!(names, ["Open", "Locked"], "by ping");
    let lines = signal_pages::lines(menu(&sim), &MenuData::shipped(), sim.app.world().resource::<LanList>());
    let open = lines.iter().position(|l| *l == Line::Server(0)).unwrap();
    for _ in 0..open {
        press(&mut sim, KeyCode::ArrowDown);
    }
    assert_eq!(pages(&sim).focus, open);
    assert_eq!(menu(&sim).focus, None, "no text field has the keys");
    // The locked one, with no password typed: the password field asks.
    press(&mut sim, KeyCode::ArrowDown);
    press(&mut sim, KeyCode::Enter);
    sim.ticks(1);
    assert!(asked(&sim).is_empty());
    assert_eq!(menu(&sim).focus, Some(party_kit::main_menu::Field::Password));
    // The open one joins.
    for _ in 0..4 {
        press(&mut sim, KeyCode::ArrowDown);
    }
    press(&mut sim, KeyCode::ArrowUp);
    assert_eq!(pages(&sim).focus, open);
    press(&mut sim, KeyCode::Enter);
    sim.ticks(1);
    assert_eq!(asked(&sim), ["connect 127.0.0.1:28012"]);
    assert_eq!(menu(&sim).screen, Screen::Joining);
}

#[test]
fn ui_scene_stages_play_and_join() {
    let mut sim = sim(PAGES_THEME);
    // What `ui_scene play game` calls.
    signal_pages::stage(sim.app.world_mut(), MenuPage::Play, &["game".into()]).unwrap();
    sim.ticks(2);
    assert_eq!(menu(&sim).page, MenuPage::Play);
    assert_eq!(menu(&sim).play_tab, 1);
    assert_eq!(roots(&mut sim), 1);
    act(&mut sim, A::Back);
    signal_pages::stage(sim.app.world_mut(), MenuPage::Join, &[]).unwrap();
    sim.ticks(2);
    assert_eq!(menu(&sim).page, MenuPage::Join);
    assert_eq!(sim.app.world().resource::<LanList>().servers.len(), 3, "made-up servers");
}
