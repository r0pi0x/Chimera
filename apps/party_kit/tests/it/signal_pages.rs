//! The main menu's Play and Join in the Signal look (`party_kit::
//! signal_pages`, ugg-ui.md M5, main-menu.md M2), headless on a manual UI
//! clock: drawn only for a theme with `[layout.pages]` (the VGUI frames
//! otherwise); on Play the keys move the focus, change a row and Enter
//! starts the game (its map loaded, the menu in the game); on Join Enter
//! connects to the typed address, refuses an empty one, and joins a LAN
//! server of the list (a locked one asks for the password in a modal); the
//! LAN list's model (ours only, by ping); the browser's scrolling window,
//! Favourites (F, kept in a file) and History (joins, newest first), the
//! password modal; `ui_scene play|join`.

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
    server_list::{self as sl, BrowserFiles, BrowserTab, Status, VISIBLE_ROWS},
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
    // The locked one asks for its password in a modal.
    press(&mut sim, KeyCode::ArrowDown);
    press(&mut sim, KeyCode::Enter);
    sim.ticks(1);
    assert!(asked(&sim).is_empty());
    assert!(pages(&sim).ask.is_some(), "the modal");
    assert_eq!(menu(&sim).focus, Some(party_kit::main_menu::Field::Password));
    press(&mut sim, KeyCode::Escape);
    act(&mut sim, A::PasswordCancel);
    assert!(pages(&sim).ask.is_none());
    assert_eq!(pages(&sim).up, Some(MenuPage::Join), "Cancel stays on Join");
    // The open one joins.
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
    assert_eq!(sim.app.world().resource::<LanList>().servers.len(), 12, "made-up servers");
    assert_eq!(sim.app.world().resource::<LanList>().favourites.len(), 4);
}

// ---------------------------------------------------------------------------
// The browser: the list model.

#[test]
fn the_window_keeps_the_focused_row_in_view() {
    // 12 rows, 8 shown: the first row shown is 0..=4.
    assert_eq!(sl::keep_in_view(0, 3, 12, 8), 0, "in view: stays");
    assert_eq!(sl::keep_in_view(0, 8, 12, 8), 1, "just below: one step");
    assert_eq!(sl::keep_in_view(0, 11, 12, 8), 4, "the last row: the window's end");
    assert_eq!(sl::keep_in_view(4, 2, 12, 8), 2, "above: the focus is the first");
    assert_eq!(sl::keep_in_view(5, 7, 6, 8), 0, "a short list never scrolls");
    assert_eq!(sl::scroll_by(0, -1, 12, 8), 0, "not above the start");
    assert_eq!(sl::scroll_by(3, 5, 12, 8), 4, "not past the end");
    assert_eq!(sl::scroll_by(0, 2, 12, 8), 2);
    assert_eq!(sl::clamp_offset(9, 10, 8), 2);
    assert_eq!(BrowserTab::Lan.step(-1), BrowserTab::Lan);
    assert_eq!(BrowserTab::Lan.step(1), BrowserTab::Favourites);
    assert_eq!(BrowserTab::History.step(1), BrowserTab::History);
}

fn addr(n: u8) -> SocketAddr {
    SocketAddr::from(([192, 0, 2, n], 27015))
}

#[test]
fn favourites_toggle_sort_and_survive_a_file() {
    let mut list = Vec::new();
    assert!(sl::toggle_favourite(&mut list, addr(1)));
    assert!(sl::toggle_favourite(&mut list, addr(2)));
    assert!(!sl::toggle_favourite(&mut list, addr(1)), "again: dropped");
    assert_eq!(list, [addr(2)]);
    let path = std::env::temp_dir().join(format!("chimera_test_favs_{}.txt", std::process::id()));
    let files = BrowserFiles { favourites: Some(path.clone()), history: None };
    files.write_favourites(&[addr(2), addr(3)]);
    assert_eq!(files.read_favourites(), [addr(2), addr(3)]);
    assert_eq!(sl::parse_favourites("192.0.2.9\nnonsense\n192.0.2.9:27015\n\n198.51.100.4:27016"), [addr(9), SocketAddr::from(([198, 51, 100, 4], 27016))]);
    let _ = std::fs::remove_file(path);
    // Their rows: answered by ping, then asking, then silent.
    let mut results = vec![server(1, "Slow", 50, false, GAME_FOLDER), server(2, "Quick", 5, false, GAME_FOLDER)];
    results[0].addr = addr(1);
    results[1].addr = addr(2);
    results.push(QueryResult { addr: addr(3), state: QueryState::NoResponse, lan: false });
    let addrs: Vec<_> = [addr(3), addr(4), addr(1), addr(2)].into_iter().map(|a| (a, None)).collect();
    let mut rows = sl::listed(&addrs, &results);
    sl::sort_favourites(&mut rows);
    let order: Vec<(SocketAddr, Status)> = rows.iter().map(|r| (r.addr, r.status)).collect();
    assert_eq!(order, [(addr(2), Status::Up), (addr(1), Status::Up), (addr(4), Status::Asking), (addr(3), Status::Down)]);
}

#[test]
fn history_is_newest_first_once_and_capped() {
    let mut v = Vec::new();
    sl::record_visit(&mut v, addr(1), 100);
    sl::record_visit(&mut v, addr(2), 200);
    sl::record_visit(&mut v, addr(1), 300);
    assert_eq!(v.iter().map(|x| (x.addr, x.when)).collect::<Vec<_>>(), [(addr(1), 300), (addr(2), 200)]);
    for n in 0..40u8 {
        sl::record_visit(&mut v, addr(n.max(3)), 400 + n as u64);
    }
    assert_eq!(v.len(), sl::HISTORY_MAX);
    let path = std::env::temp_dir().join(format!("chimera_test_hist_{}.txt", std::process::id()));
    let files = BrowserFiles { favourites: None, history: Some(path.clone()) };
    files.write_history(&v);
    assert_eq!(files.read_history(), v);
    let _ = std::fs::remove_file(path);
    assert_eq!(sl::ago(1000, 990), "just now");
    assert_eq!(sl::ago(1000, 1000 - 300), "5 min ago");
    assert_eq!(sl::ago(100_000, 100_000 - 7200), "2 h ago");
    assert_eq!(sl::ago(1_000_000, 1_000_000 - 3 * 86_400), "3 days ago");
    assert_eq!(sl::parse_addr("192.0.2.5"), Some(addr(5)));
    assert_eq!(sl::parse_addr("example.test:27015"), None);
}

// ---------------------------------------------------------------------------
// The browser on Join.

fn lan_results(n: u16) -> Vec<QueryResult> {
    (0..n).map(|i| server(28100 + i, &format!("Server {i:02}"), 3 + i as u32, false, GAME_FOLDER)).collect()
}

fn open_join_with(sim: &mut Sim, results: Vec<QueryResult>) {
    act(sim, A::Entry(1));
    {
        let mut q = sim.app.world_mut().resource_mut::<ServerQueries>();
        q.results = results;
        q.generation += 100;
    }
    sim.ticks(2);
}

fn scroll(sim: &Sim) -> usize {
    sim.app.world().resource::<LanList>().scroll
}

fn lines_of(sim: &Sim) -> Vec<Line> {
    signal_pages::lines(menu(sim), &MenuData::shipped(), sim.app.world().resource::<LanList>())
}

fn page_lines(sim: &mut Sim) -> usize {
    sim.app.world_mut().query::<&signal_pages::PageLine>().iter(sim.app.world()).count()
}

#[test]
fn a_long_list_scrolls_with_the_keys_and_the_wheel() {
    let mut sim = sim(PAGES_THEME);
    open_join_with(&mut sim, lan_results(12));
    assert_eq!(sim.app.world().resource::<LanList>().servers.len(), 12, "all of them, not eight");
    let direct = 2; // the address and password rows; no recent yet
    // Eight server rows and the two Direct ones are on the page.
    assert_eq!(page_lines(&mut sim), VISIBLE_ROWS + direct);
    let first = lines_of(&sim).iter().position(|l| *l == Line::Server(0)).unwrap();
    for _ in 0..first {
        press(&mut sim, KeyCode::ArrowDown);
    }
    assert_eq!(scroll(&sim), 0);
    for _ in 0..7 {
        press(&mut sim, KeyCode::ArrowDown);
    }
    assert_eq!(pages(&sim).focus, first + 7);
    assert_eq!(scroll(&sim), 0, "the eighth row is still in the window");
    press(&mut sim, KeyCode::ArrowDown);
    assert_eq!(scroll(&sim), 1, "the ninth pushes the window down");
    for _ in 0..3 {
        press(&mut sim, KeyCode::ArrowDown);
    }
    assert_eq!(pages(&sim).focus, first + 11);
    assert_eq!(scroll(&sim), 4, "the last row: the window's end");
    assert_eq!(page_lines(&mut sim), VISIBLE_ROWS + direct, "still eight");
    // Up past the window's top moves it back.
    for _ in 0..8 {
        press(&mut sim, KeyCode::ArrowUp);
    }
    assert_eq!(scroll(&sim), 3, "the focused row is the window's first");
    // The wheel moves the window, not the focus.
    let focus = pages(&sim).focus;
    sim.app.world_mut().write_message(bevy::input::mouse::MouseWheel {
        unit: bevy::input::mouse::MouseScrollUnit::Line,
        x: 0.0,
        y: -2.0,
        window: Entity::PLACEHOLDER,
        phase: bevy::input::touch::TouchPhase::Moved,
    });
    sim.ticks(2);
    assert_eq!(scroll(&sim), 4, "clamped at the end");
    assert_eq!(pages(&sim).focus, focus, "the wheel leaves the focus");
    // The focused row is out of the window now: a key brings it back.
    press(&mut sim, KeyCode::ArrowUp);
    assert_eq!(pages(&sim).focus, focus - 1);
    assert_eq!(scroll(&sim), 2, "the focused row is the window's first again");
    sim.app.world_mut().write_message(bevy::input::mouse::MouseWheel {
        unit: bevy::input::mouse::MouseScrollUnit::Line,
        x: 0.0,
        y: 3.0,
        window: Entity::PLACEHOLDER,
        phase: bevy::input::touch::TouchPhase::Moved,
    });
    sim.ticks(2);
    assert_eq!(scroll(&sim), 0, "up to the start");
    // PageDown moves a window of rows.
    press(&mut sim, KeyCode::PageDown);
    assert_eq!(pages(&sim).focus, first + 10, "a window on from server 2");
    assert_eq!(scroll(&sim), 3);
}

#[test]
fn tabs_favourites_and_history_keep_to_their_files() {
    let dir = std::env::temp_dir();
    let files = BrowserFiles {
        favourites: Some(dir.join(format!("chimera_test_scn_favs_{}.txt", std::process::id()))),
        history: Some(dir.join(format!("chimera_test_scn_hist_{}.txt", std::process::id()))),
    };
    let mut sim = sim(PAGES_THEME);
    sim.app.insert_resource(files.clone());
    open_join_with(&mut sim, lan_results(3));
    let first = lines_of(&sim).iter().position(|l| *l == Line::Server(0)).unwrap();
    for _ in 0..first {
        press(&mut sim, KeyCode::ArrowDown);
    }
    // F favourites the focused server (Server 00); typing keeps the F.
    press(&mut sim, KeyCode::KeyF);
    sim.ticks(2);
    let lan = sim.app.world().resource::<LanList>();
    assert_eq!(lan.fav_addrs, [SocketAddr::from(([127, 0, 0, 1], 28100))]);
    assert!(lan.servers[0].favourite && !lan.servers[1].favourite);
    assert_eq!(files.read_favourites(), lan.fav_addrs, "kept in its file");
    // ← → switch the tab (off a text field): the Favourites tab lists it.
    press(&mut sim, KeyCode::ArrowRight);
    sim.ticks(1);
    let lan = sim.app.world().resource::<LanList>();
    assert_eq!(lan.tab, BrowserTab::Favourites);
    assert_eq!(lan.rows().len(), 1);
    assert_eq!((lan.rows()[0].name.as_str(), lan.rows()[0].status), ("Server 00", Status::Up), "asked by address, answered");
    assert_eq!(pages(&sim).focus, first, "the focus is on its first row");
    // F again drops it.
    press(&mut sim, KeyCode::KeyF);
    sim.ticks(2);
    assert!(sim.app.world().resource::<LanList>().rows().is_empty());
    assert!(files.read_favourites().is_empty());
    // Joining a server records it, newest first, in its file; History lists it.
    act(&mut sim, A::BrowserTab(0));
    for _ in 0..6 {
        if lines_of(&sim).get(pages(&sim).focus) != Some(&Line::Server(0)) {
            press(&mut sim, KeyCode::ArrowDown);
        }
    }
    press(&mut sim, KeyCode::Enter);
    sim.ticks(1);
    assert_eq!(asked(&sim), ["connect 127.0.0.1:28100"]);
    let v = sim.app.world().resource::<LanList>().visits.clone();
    assert_eq!(v.len(), 1);
    assert_eq!(files.read_history(), v);
    act(&mut sim, A::Cancel);
    if menu(&sim).page != MenuPage::Main {
        act(&mut sim, A::Back);
    }
    act(&mut sim, A::Entry(1));
    act(&mut sim, A::BrowserTab(2));
    let lan = sim.app.world().resource::<LanList>();
    assert_eq!(lan.tab, BrowserTab::History);
    assert_eq!(lan.rows().len(), 1);
    assert_eq!(lan.rows()[0].when, Some(v[0].when));
    for p in [&files.favourites, &files.history].into_iter().flatten() {
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn a_locked_server_asks_for_its_password_in_a_modal() {
    let mut sim = sim(PAGES_THEME);
    open_join_with(&mut sim, vec![server(28201, "Locked", 4, true, GAME_FOLDER)]);
    let first = lines_of(&sim).iter().position(|l| *l == Line::Server(0)).unwrap();
    for _ in 0..first {
        press(&mut sim, KeyCode::ArrowDown);
    }
    let buttons = |sim: &mut Sim| sim.app.world_mut().query::<&A>().iter(sim.app.world()).filter(|a| **a == A::PasswordJoin).count();
    assert_eq!(buttons(&mut sim), 0);
    press(&mut sim, KeyCode::Enter);
    sim.ticks(2);
    assert_eq!(pages(&sim).ask.as_ref().map(|a| a.name.as_str()), Some("Locked"));
    assert_eq!(buttons(&mut sim), 1, "the modal is drawn");
    assert!(asked(&sim).is_empty());
    // Arrows don't move the page under it; Enter with nothing typed is refused.
    press(&mut sim, KeyCode::ArrowUp);
    assert_eq!(pages(&sim).focus, first);
    press(&mut sim, KeyCode::Enter);
    sim.ticks(2);
    assert!(pages(&sim).ask.is_some());
    assert_eq!(pages(&sim).notice.as_deref(), Some("Type the password"));
    assert!(asked(&sim).is_empty());
    // Esc closes it (and stays on Join).
    // (`main_menu::typing` turns the Esc key into Back, which the modal takes.)
    act(&mut sim, A::Back);
    assert!(pages(&sim).ask.is_none());
    assert_eq!(menu(&sim).page, MenuPage::Join);
    assert_eq!(buttons(&mut sim), 0);
    // Again: type it, Enter joins with it.
    press(&mut sim, KeyCode::Enter);
    sim.ticks(1);
    submit(&mut sim, "mainmenu_type password Hunter2");
    press(&mut sim, KeyCode::Enter);
    sim.ticks(2);
    assert!(pages(&sim).ask.is_none());
    assert_eq!(asked(&sim), ["connect 127.0.0.1:28201"]);
    assert_eq!(menu(&sim).screen, Screen::Joining);
    assert_eq!(menu(&sim).password, "Hunter2");
}

#[test]
fn ui_scene_stages_each_tab_and_the_modal() {
    let mut sim = sim(PAGES_THEME);
    signal_pages::stage(sim.app.world_mut(), MenuPage::Join, &["history".into()]).unwrap();
    sim.ticks(3);
    let lan = sim.app.world().resource::<LanList>();
    assert_eq!(lan.tab, BrowserTab::History);
    assert_eq!(lan.history.len(), 4);
    assert!(lan.history.iter().all(|r| r.when.is_some()));
    assert!(lan.history[3].status == Status::Up || lan.history[3].status == Status::Down);
    act(&mut sim, A::Back);
    signal_pages::stage(sim.app.world_mut(), MenuPage::Join, &["password".into()]).unwrap();
    sim.ticks(3);
    assert!(pages(&sim).ask.is_some(), "the modal over the first locked server");
}

#[test]
fn typed_text_keeps_its_case_and_labels_stay_capitals() {
    let mut sim = sim(PAGES_THEME);
    act(&mut sim, A::Entry(1));
    submit(&mut sim, "mainmenu_type address Home.Example:27015");
    sim.ticks(3);
    let texts: Vec<String> = sim.app.world_mut().query::<&Text>().iter(sim.app.world()).map(|t| t.0.clone()).collect();
    assert!(texts.iter().any(|t| t == "Home.Example:27015"), "the typed text as typed: {texts:?}");
    assert!(texts.iter().any(|t| t == "ADDRESS"), "a label in capitals");
    assert!(!texts.iter().any(|t| t == "HOME.EXAMPLE:27015"));
}

/// A join is kept in Recent and the History: on the next visit to Join it
/// is listed, Enter on it joins it again, and after a restart (the files
/// read again) it is still there; a start that joins from the command
/// line (`+connect`: a join link, the first join's relaunch) is kept too.
#[test]
fn recent_joins_are_listed_again_and_join_on_enter() {
    let mut sim = sim(PAGES_THEME);
    act(&mut sim, A::Entry(1));
    submit(&mut sim, "mainmenu_type address 192.0.2.7:27016");
    press(&mut sim, KeyCode::Enter);
    sim.ticks(1);
    assert_eq!(asked(&sim), ["connect 192.0.2.7:27016"]);
    let addr: SocketAddr = "192.0.2.7:27016".parse().unwrap();
    assert!(sim.app.world().resource::<LanList>().visits.iter().any(|v| v.addr == addr), "in the History");

    // Back at the menu (the join failed or was left): Join lists it.
    sim.app.world_mut().resource_mut::<PartyMenu>().screen = Screen::MainMenu;
    sim.ticks(2);
    act(&mut sim, A::Entry(1));
    assert_eq!(pages(&sim).up, Some(MenuPage::Join));
    let lines = lines_of(&sim);
    let at = lines.iter().position(|l| *l == Line::Recent(0)).expect("the Recent row");
    assert_eq!(menu(&sim).recent, ["192.0.2.7:27016"]);
    sim.app.world_mut().resource_mut::<PartyMenu>().focus = None;
    sim.app.world_mut().resource_mut::<SignalPages>().focus = at;
    sim.ticks(1);
    press(&mut sim, KeyCode::Enter);
    sim.ticks(1);
    assert_eq!(asked(&sim), ["connect 192.0.2.7:27016", "connect 192.0.2.7:27016"], "Enter on Recent joins it");

    // A restart: the files are read again.
    let dir = std::env::temp_dir();
    let recent = party_kit::main_menu::RecentFile(Some(dir.join(format!("chimera_test_recent_{}.txt", std::process::id()))));
    recent.write(&menu(&sim).recent);
    assert_eq!(recent.read(), ["192.0.2.7:27016"], "Recent after a restart");
    let files = BrowserFiles { favourites: None, history: Some(dir.join(format!("chimera_test_recent_hist_{}.txt", std::process::id()))) };
    files.write_history(&sim.app.world().resource::<LanList>().visits);
    assert!(files.read_history().iter().any(|v| v.addr == addr), "History after a restart");
    for p in [recent.0.as_ref(), files.history.as_ref()].into_iter().flatten() {
        let _ = std::fs::remove_file(p);
    }

    // A start with `+connect` remembers its address.
    assert_eq!(party_kit::main_menu::command_line_join(&["connect \"192.0.2.9:27015\"".into()]).as_deref(), Some("192.0.2.9:27015"));
    let mut args = chimera_client::client::Args::default();
    args.console = vec!["connect 192.0.2.9:27015".into()];
    sim.app.insert_resource(chimera_client::client::ClientArgs(args));
    party_kit::main_menu::remember_command_line_join(sim.app.world_mut());
    assert_eq!(menu(&sim).recent, ["192.0.2.9:27015", "192.0.2.7:27016"]);
    let nine: SocketAddr = "192.0.2.9:27015".parse().unwrap();
    assert_eq!(sim.app.world().resource::<LanList>().visits[0].addr, nine, "newest first in the History");
}
