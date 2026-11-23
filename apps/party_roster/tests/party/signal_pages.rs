//! The main menu's Play and Join in the Signal look (`party_roster::
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
use party_roster::{
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
    if sim.app.world().resource::<Console>().cvar("mp_guns_off").is_none() {
        sim.app.console_command("mp_guns_off", "a test's: asked only", |w, a| {
            w.resource_mut::<Asked>().0.push(format!("mp_guns_off {}", a.join(" ")));
            Ok(None)
        });
    }
    if sim.app.world().resource::<Console>().cvar("mp_maps_off").is_none() {
        sim.app.console_command("mp_maps_off", "a test's: asked only", |w, a| {
            w.resource_mut::<Asked>().0.push(format!("mp_maps_off {}", a.join(" ")));
            Ok(None)
        });
    }
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
    // Enter on the first entry opens Host.
    press(&mut sim, KeyCode::Enter);
    sim.ticks(2);
    assert_eq!(menu(&sim).page, MenuPage::Play);
    assert_eq!(roots(&mut sim), 1, "drawn");
    assert_eq!(pages(&sim).up, Some(MenuPage::Play));
    assert_eq!(pages(&sim).focus, 0, "the first row");
    let data = MenuData::shipped();
    let name = data.play.iter().position(|r| r.cvar == "hostname").unwrap();
    assert_eq!(name, 0, "Server name, Password and Port come first");
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
    // Q switches to the Game tab, E back; not while typing.
    press(&mut sim, KeyCode::KeyQ);
    sim.ticks(1);
    assert_eq!(menu(&sim).play_tab, 1);
    assert_eq!(pages(&sim).focus, 0, "a tab starts at its first row");
    press(&mut sim, KeyCode::KeyE);
    sim.ticks(1);
    assert_eq!(menu(&sim).play_tab, 0);
    // Switching tabs never takes the keyboard: the server-name row is the
    // cursor, no field has the keys, so Q/E still switch tabs.
    assert_eq!(menu(&sim).focus, None, "a tab switch doesn't focus the text field");
    press(&mut sim, KeyCode::KeyQ);
    sim.ticks(1);
    assert_eq!(menu(&sim).play_tab, 1, "Q switched again: nothing swallowed it");
    assert_eq!(menu(&sim).focus, None);
    press(&mut sim, KeyCode::KeyE);
    sim.ticks(1);
    assert_eq!(menu(&sim).play_tab, 0);
    // An explicit Enter on the text row takes the keys.
    press(&mut sim, KeyCode::Enter);
    sim.ticks(1);
    assert_eq!(menu(&sim).focus, Some(party_roster::main_menu::Field::Play(name)), "Enter focuses the field");
    assert!(asked(&sim).is_empty(), "that Enter didn't start the game");
    press(&mut sim, KeyCode::KeyQ);
    assert_eq!(menu(&sim).play_tab, 0, "typing: Q is a letter");
    press(&mut sim, KeyCode::Enter);
    sim.ticks(4);
    let a = asked(&sim);
    assert!(a.iter().any(|l| l.starts_with("map ")), "{a:?}");
    assert_eq!(menu(&sim).screen, Screen::InGame);
    assert_eq!(pages(&sim).up, None);
    assert_eq!(roots(&mut sim), 0, "gone in the game");
}

/// Host's Maps field opens the picker (Enter on its row): a game's box
/// sets every map under it, a map alone makes its game "some on", Esc
/// cancels and Done keeps; Start sends the maps off (`mp_maps_off`).
#[test]
fn the_maps_picker_sets_the_rotation() {
    use party_roster::maps_picker::{MapsDialog, MapsPicker, PickerAction as P, Tri, TreeRow};
    let mut sim = sim(PAGES_THEME);
    press(&mut sim, KeyCode::Enter);
    sim.ticks(2);
    let data = MenuData::shipped();
    let maps = data.play.iter().position(|r| r.kind == party_roster::main_menu::PlayKind::Map).unwrap();
    for _ in 0..maps {
        press(&mut sim, KeyCode::ArrowDown);
    }
    press(&mut sim, KeyCode::Enter);
    sim.ticks(2);
    let picker = |s: &Sim| s.app.world().resource::<MapsPicker>().clone();
    assert!(picker(&sim).open.is_some(), "Enter on Maps opens the picker");
    {
        // The tree is the kit's recycling list: one line a row, the cursor on the first.
        let lists = sim.app.world().resource::<chimera_ui::VirtualLists>();
        let st = lists.get(party_roster::maps_picker::LIST).expect("the picker's list");
        assert_eq!(st.cursor, Some(chimera_ui::VCursor::Item(0, 0)));
        assert!(!st.sections.is_empty() && st.sections[0].len > 1);
    }
    assert_eq!(menu(&sim).screen, Screen::MainMenu, "not started");
    let dialogs = sim.app.world_mut().query_filtered::<(), With<MapsDialog>>().iter(sim.app.world()).count();
    assert_eq!(dialogs, 1, "drawn over the page");
    // The tree and the tri-state boxes clip (`Overflow::clip` set beside
    // their styles, which merge into the node and keep it).
    let dialog = sim.app.world_mut().query_filtered::<Entity, With<MapsDialog>>().single(sim.app.world()).unwrap();
    let clipped = sim
        .app
        .world_mut()
        .query::<(Entity, &Node, &chimera_ui::UiStyle)>()
        .iter(sim.app.world())
        .filter(|(e, n, _)| {
            let up = |p: Entity| sim.app.world().get::<ChildOf>(p).map(ChildOf::parent);
            n.overflow == Overflow::clip() && std::iter::successors(up(*e), |p| up(*p)).any(|p| p == dialog)
        })
        .count();
    assert!(clipped > 1, "the picker's styled tree and boxes keep their clip ({clipped})");
    use party_roster::maps_picker::MapCatalogue;
    let id = sim.app.world().resource::<party_roster::main_menu::MenuMinigame>().0.clone();
    let pool: Vec<String> = sim
        .app
        .world()
        .get_resource::<chimera_sim::minigame::Minigames>()
        .and_then(|m| m.get(&id))
        .map(|d| d.maps.iter().map(|m| chimera_net::net::normalize_map(m)).collect())
        .unwrap_or_default();
    let games = party_roster::maps_picker::LocalCatalogue.listing(&pool);
    assert!(!games.is_empty(), "the pool's games");
    // Space on the first game: all of it off; again: on.
    press(&mut sim, KeyCode::Space);
    assert_eq!(picker(&sim).state(&games, &TreeRow::Game(0)), Tri::Off);
    press(&mut sim, KeyCode::Space);
    assert_eq!(picker(&sim).state(&games, &TreeRow::Game(0)), Tri::On);
    // A map alone: its game is "some on" (or off, when it has one map).
    sim.app.world_mut().write_message(P::Toggle(2));
    sim.ticks(2);
    let n = picker(&sim).ids_of(&games, &TreeRow::Game(0)).len();
    assert_eq!(picker(&sim).state(&games, &TreeRow::Game(0)), if n > 1 { Tri::Mixed } else { Tri::Off }, "{n} maps");
    assert_eq!(picker(&sim).state(&games, &TreeRow::Item(0, 0, 0)), Tri::Off);
    // Esc cancels: nothing kept.
    press(&mut sim, KeyCode::Escape);
    assert!(picker(&sim).open.is_none() && picker(&sim).off.is_empty(), "cancelled");
    assert_eq!(menu(&sim).page, MenuPage::Play, "Esc only closed the picker");
    // Again, Done keeps it; Start sends it.
    press(&mut sim, KeyCode::Enter);
    sim.app.world_mut().write_message(P::Toggle(2));
    sim.ticks(2);
    press(&mut sim, KeyCode::Enter);
    let off = picker(&sim).off.clone();
    assert_eq!(off.len(), 1, "Done kept one map off");
    act(&mut sim, A::Start);
    sim.ticks(4);
    let sent = match sim.app.world().get_resource::<chimera_net::minigame_net::MapsOff>() {
        Some(m) => m.0.clone(),
        None => asked(&sim).iter().find_map(|l| l.strip_prefix("mp_maps_off ").map(str::to_string)).unwrap_or_default(),
    };
    assert_eq!(sent.trim_matches('"'), off.iter().next().unwrap().as_str(), "mp_maps_off set at the start");
}

/// In the Maps picker a game's header folds (chevron, name, the rest of
/// the row) and only its box, with a little slop, toggles; a map's row
/// toggles. Hover marks the same areas.
#[test]
fn the_maps_picker_header_folds_and_only_its_box_toggles() {
    use party_roster::maps_picker::{MapsPicker, PickerAction as P, TriHit, TreeRow};
    let mut sim = sim(PAGES_THEME);
    press(&mut sim, KeyCode::Enter);
    sim.ticks(2);
    let data = MenuData::shipped();
    let maps = data.play.iter().position(|r| r.kind == party_roster::main_menu::PlayKind::Map).unwrap();
    for _ in 0..maps {
        press(&mut sim, KeyCode::ArrowDown);
    }
    press(&mut sim, KeyCode::Enter);
    sim.ticks(3);
    let picker = |s: &Sim| s.app.world().resource::<MapsPicker>().clone();
    assert!(picker(&sim).open.is_some());
    let acts = |s: &mut Sim| -> Vec<(Entity, P)> {
        s.app.world_mut().query::<(Entity, &P)>().iter(s.app.world()).filter(|(_, a)| matches!(a, P::Toggle(0) | P::Fold(0))).map(|(e, a)| (e, *a)).collect()
    };
    let found = acts(&mut sim);
    let row = found.iter().find(|(_, a)| *a == P::Fold(0)).expect("the header row folds").0;
    let hit = found.iter().find(|(_, a)| *a == P::Toggle(0)).expect("the box toggles").0;
    assert_eq!(found.len(), 2, "the chevron has no action of its own: {found:?}");
    assert!(sim.app.world().get::<TriHit>(hit).is_some());
    assert!(sim.app.world().get::<chimera_ui::controls::Hover>(row).is_some(), "the row hovers");
    assert!(sim.app.world().get::<chimera_ui::controls::Hover>(hit).is_some(), "the box hovers");
    let games = {
        use party_roster::maps_picker::MapCatalogue;
        let id = sim.app.world().resource::<party_roster::main_menu::MenuMinigame>().0.clone();
        let pool: Vec<String> = sim
            .app
            .world()
            .get_resource::<chimera_sim::minigame::Minigames>()
            .and_then(|m| m.get(&id))
            .map(|d| d.maps.iter().map(|m| chimera_net::net::normalize_map(m)).collect())
            .unwrap_or_default();
        party_roster::maps_picker::LocalCatalogue.listing(&pool)
    };
    let row0 = TreeRow::Game(0);
    let click = |sim: &mut Sim, e: Entity| {
        use bevy::ui::DeprecatedInteraction as I;
        *sim.app.world_mut().get_mut::<I>(e).unwrap() = I::Pressed;
        sim.ticks(2);
    };
    // The row (name, the rest): folds, the maps stay as they were.
    let folded = picker(&sim).is_folded(&games, &row0);
    click(&mut sim, row);
    assert_ne!(picker(&sim).is_folded(&games, &row0), folded, "a click on the header folds");
    assert_eq!(picker(&sim).state(&games, &row0), party_roster::maps_picker::Tri::On, "and toggles nothing");
    // The box toggles; the fold stays.
    let folded = picker(&sim).is_folded(&games, &row0);
    let hit = acts(&mut sim).iter().find(|(_, a)| *a == P::Toggle(0)).unwrap().0;
    click(&mut sim, hit);
    assert_eq!(picker(&sim).state(&games, &row0), party_roster::maps_picker::Tri::Off, "the box toggles");
    assert_eq!(picker(&sim).is_folded(&games, &row0), folded, "and does not fold");
}

/// A made-up listing: two games; game a's AK a family of two skins.
struct FakeGuns;

impl party_roster::guns_picker::GunCatalogue for FakeGuns {
    fn listing(&self, _: &World) -> party_roster::guns_picker::GunsListing {
        use chimera_core::game_parts::GunClass;
        use chimera_sim::minigame::power::Band;
        use party_roster::guns_picker::{GunFamily, GunsListing};
        let fam = |id: &str, game: &str, class: GunClass, band: Band, score: f32, members: &[&str]| GunFamily {
            id: id.into(),
            game: game.into(),
            name: id.rsplit(':').next().unwrap().to_uppercase(),
            class,
            band,
            score,
            members: members.iter().map(|m| m.to_string()).collect(),
        };
        GunsListing {
            games: vec![("a".into(), "Game A".into()), ("b".into(), "Game B".into())],
            families: vec![
                fam("a:glock", "a", GunClass::Pistol, Band::WarmUp, 1.0, &["a:glock"]),
                fam("a:ak", "a", GunClass::Rifle, Band::Peak, 3.0, &["a:ak", "a:ak_gold"]),
                fam("a:awp", "a", GunClass::Sniper, Band::Heavy, 4.0, &["a:awp"]),
                fam("b:mp5", "b", GunClass::Smg, Band::Rising, 2.0, &["b:mp5"]),
                fam("b:rocket", "b", GunClass::Launcher, Band::Specials, 5.0, &["b:rocket"]),
            ],
            generation: 0,
        }
    }
}

/// The Host page with the fake guns, a rungs setting and a presets
/// folder of the test's own; the dialog open (Enter on the Guns row).
fn guns_sim(dir: &std::path::Path) -> Sim {
    use party_roster::guns_picker::{GunsPresetDir, GunsSource};
    let mut sim = sim(PAGES_THEME);
    sim.app.insert_resource(GunsSource(Box::new(FakeGuns)));
    sim.app.insert_resource(GunsPresetDir(Some(dir.to_path_buf())));
    sim.app.insert_resource(chimera_sim::minigame::gun_rules::LadderRungs(13));
    sim.app.init_resource::<chimera_sim::minigame::gun_rules::GunRules>();
    sim.app.insert_resource(chimera_ui::clipboard::Clipboard::memory(None));
    press(&mut sim, KeyCode::Enter);
    sim.ticks(2);
    let row = MenuData::shipped().play.iter().position(|r| r.kind == party_roster::main_menu::PlayKind::Guns).expect("a Guns row");
    for _ in 0..row {
        press(&mut sim, KeyCode::ArrowDown);
    }
    press(&mut sim, KeyCode::Enter);
    sim.ticks(3);
    assert!(working(&sim).is_some(), "Enter on Guns opens the dialog");
    sim
}

fn working(sim: &Sim) -> Option<party_roster::guns_picker::Working> {
    sim.app.world().resource::<party_roster::guns_picker::GunsPicker>().open.clone()
}

fn guns_act(sim: &mut Sim, a: party_roster::guns_picker::GunsAct) {
    sim.app.world_mut().write_message(a);
    sim.ticks(2);
}

fn world_rules(sim: &Sim) -> chimera_sim::minigame::gun_rules::GunRules {
    sim.app.world().resource::<chimera_sim::minigame::gun_rules::GunRules>().clone()
}

/// A scratch folder of a test's own (removed at the end).
fn scratch(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("chimera-guns-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

/// The dialog is a modal over the page with its cards; toggling a game
/// and a class writes the off-lists at Done (the server's cvars); a Solo
/// keeps one game; Esc cancels what the copy changed.
#[test]
fn guns_dialog_games_and_classes_write_the_off_lists() {
    use chimera_core::game_parts::GunClass;
    use party_roster::guns_picker::{GunCard, GunsAct as G, GunsDialog};
    let dir = scratch("toggles");
    let mut sim = guns_sim(&dir);
    assert_eq!(sim.app.world_mut().query_filtered::<(), With<GunsDialog>>().iter(sim.app.world()).count(), 1);
    assert!(sim.app.world().resource::<chimera_ui::Modals>().is_open(), "a modal layer");
    sim.ticks(3);
    assert_eq!(sim.app.world_mut().query::<&GunCard>().iter(sim.app.world()).count(), 5, "a card a family");
    guns_act(&mut sim, G::Game(1));
    guns_act(&mut sim, G::Class(GunClass::Sniper));
    let w = working(&sim).unwrap();
    assert_eq!(w.rules.games_off, "b");
    assert_eq!(w.rules.classes_off, "sniper");
    assert_eq!(world_rules(&sim).games_off, "", "nothing set before Done");
    // Esc cancels.
    press(&mut sim, KeyCode::Escape);
    assert!(working(&sim).is_none());
    assert_eq!(world_rules(&sim), Default::default());
    assert_eq!(menu(&sim).page, MenuPage::Play, "Esc only closed the dialog");
    // Again, and Done keeps it.
    guns_act(&mut sim, G::Open);
    guns_act(&mut sim, G::Game(1));
    guns_act(&mut sim, G::Class(GunClass::Sniper));
    guns_act(&mut sim, G::Class(GunClass::Sniper));
    guns_act(&mut sim, G::Class(GunClass::Rifle));
    press(&mut sim, KeyCode::Enter);
    assert!(working(&sim).is_none(), "Enter is Done");
    let r = world_rules(&sim);
    assert_eq!((r.games_off.as_str(), r.classes_off.as_str()), ("b", "rifle"));
    // Solo: game a alone; Solo again: every game on.
    guns_act(&mut sim, G::Open);
    guns_act(&mut sim, G::Solo(1));
    assert_eq!(working(&sim).unwrap().rules.games_off, "a");
    guns_act(&mut sim, G::Solo(1));
    assert_eq!(working(&sim).unwrap().rules.games_off, "");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Rungs −/+ stay within the curve's bounds (5 to 40); Done sets the
/// world's `LadderRungs`.
#[test]
fn guns_dialog_rungs_step_within_5_to_40() {
    use party_roster::guns_picker::GunsAct as G;
    let dir = scratch("rungs");
    let mut sim = guns_sim(&dir);
    assert_eq!(working(&sim).unwrap().rungs, 13);
    for _ in 0..12 {
        guns_act(&mut sim, G::Rungs(-1));
    }
    assert_eq!(working(&sim).unwrap().rungs, 5, "never below 5");
    for _ in 0..40 {
        sim.app.world_mut().write_message(G::Rungs(1));
    }
    sim.ticks(2);
    assert_eq!(working(&sim).unwrap().rungs, 40, "never above 40");
    guns_act(&mut sim, G::Rungs(-1));
    guns_act(&mut sim, G::Done);
    assert_eq!(sim.app.world().resource::<chimera_sim::minigame::gun_rules::LadderRungs>().0, 39);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Presets: a built-in loads (and undoes), Save as writes the host's own
/// file, it lists and loads back the same rules; Copy as text and Paste
/// round-trip through the clipboard.
#[test]
fn guns_dialog_presets_load_save_and_paste_round_trip() {
    use party_roster::guns_picker::{GunsAct as G, GunsPicker, MenuItem, menu_items};
    let dir = scratch("presets");
    let mut sim = guns_sim(&dir);
    let items = |sim: &Sim| {
        let p = sim.app.world().resource::<GunsPicker>();
        menu_items(p, p.open.as_ref().unwrap())
    };
    // No explosives (the second built-in), then undo.
    guns_act(&mut sim, G::Menu);
    assert!(working(&sim).unwrap().menu.is_some(), "the menu opens");
    guns_act(&mut sim, G::MenuPick(1));
    assert_eq!(working(&sim).unwrap().rules.classes_off, "launcher,flamethrower,grenade");
    assert_eq!(working(&sim).unwrap().base, "No explosives");
    guns_act(&mut sim, G::Undo);
    assert_eq!(working(&sim).unwrap().rules.classes_off, "");
    // Edited rules saved as "Friday night".
    guns_act(&mut sim, G::Game(1));
    guns_act(&mut sim, G::Rungs(4));
    guns_act(&mut sim, G::Card(1));
    let saved_as = items(&sim).iter().position(|i| *i == MenuItem::SaveAs).unwrap();
    guns_act(&mut sim, G::MenuPick(saved_as));
    assert!(working(&sim).unwrap().naming.is_some(), "Save as asks for a name");
    type_text(&mut sim, "Friday night");
    press(&mut sim, KeyCode::Enter);
    assert!(dir.join("friday_night.toml").exists(), "written to the presets folder");
    let want = working(&sim).unwrap();
    assert_eq!(want.base, "Friday night");
    let p = sim.app.world().resource::<GunsPicker>().saved.clone();
    assert_eq!(p.len(), 1);
    assert_eq!(p[0].rules(), want.rules);
    assert_eq!(p[0].rungs, Some(17));
    // Load All guns, then Friday night back from the menu.
    guns_act(&mut sim, G::MenuPick(0));
    assert_eq!(working(&sim).unwrap().rules, Default::default());
    let friday = items(&sim).iter().position(|i| matches!(i, MenuItem::Load(k) if sim.app.world().resource::<GunsPicker>().presets()[*k].name == "Friday night")).unwrap();
    guns_act(&mut sim, G::MenuPick(friday));
    assert_eq!(working(&sim).unwrap().rules, want.rules);
    assert_eq!(working(&sim).unwrap().rungs, 17);
    // Copy as text, change, Paste: back.
    let copy = items(&sim).iter().position(|i| *i == MenuItem::Copy).unwrap();
    guns_act(&mut sim, G::MenuPick(copy));
    guns_act(&mut sim, G::Game(1));
    assert_ne!(working(&sim).unwrap().rules, want.rules);
    let paste = items(&sim).iter().position(|i| *i == MenuItem::Paste).unwrap();
    guns_act(&mut sim, G::MenuPick(paste));
    assert_eq!(working(&sim).unwrap().rules, want.rules, "pasted");
    // A fresh dialog lists it again (read from the folder).
    press(&mut sim, KeyCode::Escape);
    sim.app.world_mut().resource_mut::<GunsPicker>().saved.clear();
    guns_act(&mut sim, G::Open);
    assert!(sim.app.world().resource::<GunsPicker>().saved.iter().any(|p| p.name == "Friday night"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A card bans its family (all its skins) when the rules let it in, a
/// second click puts it back; one the rules leave out is allowed. Done
/// writes `mp_guns_ban` / `mp_guns_allow` by family id; the old guns off
/// are read as bans and cleared.
#[test]
fn guns_dialog_bans_and_allows_by_family() {
    use chimera_core::game_parts::GunClass;
    use party_roster::guns_picker::{GunsAct as G, GunsListing, PoolView, Show, pool};
    let dir = scratch("bans");
    let mut sim = guns_sim(&dir);
    // The AK family (two skins): banned, then back.
    guns_act(&mut sim, G::Card(1));
    assert_eq!(working(&sim).unwrap().rules.ban, "a:ak");
    guns_act(&mut sim, G::Card(1));
    assert_eq!(working(&sim).unwrap().rules.ban, "");
    guns_act(&mut sim, G::Card(1));
    // Launchers off: the rocket out, a click allows it.
    guns_act(&mut sim, G::Class(GunClass::Launcher));
    guns_act(&mut sim, G::Card(4));
    let w = working(&sim).unwrap();
    assert_eq!((w.rules.ban.as_str(), w.rules.allow.as_str()), ("a:ak", "b:rocket"));
    let listing = sim.app.world().resource::<GunsListing>().clone();
    let pv: PoolView = pool(&listing, &w.rules, w.rungs);
    assert_eq!((pv.families, pv.exceptions), (4, 2), "the AK out, the rocket in");
    // Show Exceptions: the two cards.
    guns_act(&mut sim, G::Show(Show::Exceptions));
    sim.ticks(3);
    assert_eq!(sim.app.world_mut().query::<&party_roster::guns_picker::GunCard>().iter(sim.app.world()).count(), 2);
    guns_act(&mut sim, G::Done);
    let r = world_rules(&sim);
    assert_eq!((r.ban.as_str(), r.allow.as_str()), ("a:ak", "b:rocket"));
    // The old guns off come in as bans.
    sim.app.insert_resource(chimera_sim::minigame::auto::GunsOff("b:mp5".into()));
    guns_act(&mut sim, G::Open);
    assert_eq!(working(&sim).unwrap().rules.ban, "a:ak,b:mp5");
    guns_act(&mut sim, G::Done);
    assert_eq!(sim.app.world().resource::<chimera_sim::minigame::auto::GunsOff>().0, "");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The pool tile: short ("plays N of M") and empty (Done refused); the
/// Host page's Guns line and preset field (`gun_rules::summary`).
#[test]
fn guns_dialog_short_pool_and_the_summary_line() {
    use party_roster::guns_picker::{GunsAct as G, GunsListing, GunsPicker, pool};
    let dir = scratch("summary");
    let mut sim = guns_sim(&dir);
    let listing = sim.app.world().resource::<GunsListing>().clone();
    let w = working(&sim).unwrap();
    let pv = pool(&listing, &w.rules, w.rungs);
    assert!(pv.short(), "five guns can't fill 13 rungs: {pv:?}");
    assert_eq!(pv.plays, 5 + pv.stages[6]);
    // Repeat fills it.
    guns_act(&mut sim, G::Short(chimera_sim::minigame::gun_rules::WhenShort::Repeat));
    let w = working(&sim).unwrap();
    assert!(!pool(&listing, &w.rules, w.rungs).short());
    // Every game off: empty, Done refused.
    guns_act(&mut sim, G::Game(0));
    guns_act(&mut sim, G::Game(1));
    let w = working(&sim).unwrap();
    assert!(pool(&listing, &w.rules, w.rungs).empty());
    guns_act(&mut sim, G::Done);
    assert!(working(&sim).is_some(), "Done refused with no gun");
    guns_act(&mut sim, G::Game(0));
    guns_act(&mut sim, G::Done);
    // The Host page's line and field.
    let picker = sim.app.world().resource::<GunsPicker>().clone();
    let line = picker.summary(&world_rules(&sim), 13, None);
    assert_eq!(line, "Custom \u{00b7} 1 game off \u{00b7} 13 rungs");
    sim.ticks(2);
    let texts: Vec<String> = sim.app.world_mut().query::<&Text>().iter(sim.app.world()).map(|t| t.0.clone()).collect();
    assert!(texts.iter().any(|t| t.eq_ignore_ascii_case(&line)), "the Guns row shows the line: {texts:?}");
    // The page's preset menu: No explosives.
    guns_act(&mut sim, G::PageMenu);
    guns_act(&mut sim, G::PagePick(1));
    assert_eq!(picker.summary(&world_rules(&sim), 13, None), "No explosives \u{00b7} 13 rungs");
    // Its last entry opens the dialog.
    let n = picker.presets().len();
    guns_act(&mut sim, G::PageMenu);
    guns_act(&mut sim, G::PagePick(n));
    assert!(working(&sim).is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Types `text` (one key message a character, as a keyboard sends them).
fn type_text(sim: &mut Sim, text: &str) {
    use bevy::input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    };
    for ch in text.chars() {
        let s = ch.to_string();
        let (code, key) = if ch == ' ' { (KeyCode::Space, Key::Space) } else { (KeyCode::KeyA, Key::Character(s.as_str().into())) };
        sim.app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: key,
            state: ButtonState::Pressed,
            text: Some(s.as_str().into()),
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        sim.ticks(1);
    }
}

#[test]
fn join_enter_connects_and_refuses_an_empty_address() {
    let mut sim = sim(PAGES_THEME);
    act(&mut sim, A::Entry(1));
    assert_eq!(pages(&sim).up, Some(MenuPage::Join));
    assert_eq!(menu(&sim).focus, Some(party_roster::main_menu::Field::Address), "the address takes the keys");
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
    assert_eq!(menu(&sim).focus, Some(party_roster::main_menu::Field::Password));
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
    assert_eq!(BrowserTab::Lan.step(-1), BrowserTab::Internet);
    assert_eq!(BrowserTab::Internet.step(-1), BrowserTab::Internet);
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
    let direct = signal_pages::DIRECT_LINES; // address, password, spectator; no recent yet
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
    act(&mut sim, A::BrowserTab(BrowserTab::Lan.index()));
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
    act(&mut sim, A::BrowserTab(BrowserTab::History.index()));
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
    // Addresses are hidden until the field's eye key shows them.
    let texts: Vec<String> = sim.app.world_mut().query::<&Text>().iter(sim.app.world()).map(|t| t.0.clone()).collect();
    assert!(!texts.iter().any(|t| t.contains("Home.Example") || t.contains("HOME.EXAMPLE")), "masked by default");
    sim.app.world_mut().resource_mut::<chimera_ui::secret::Reveals>().toggle(party_roster::signal_pages::REVEAL_ADDRESSES);
    sim.ticks(3);
    let texts: Vec<String> = sim.app.world_mut().query::<&Text>().iter(sim.app.world()).map(|t| t.0.clone()).collect();
    // A field's text is one node a character (`controls::EditText`).
    let w = sim.app.world_mut();
    let fields: Vec<String> = w
        .query::<&chimera_ui::controls::EditText>()
        .iter(w)
        .map(|f| f.chars.iter().map(|c| w.get::<Children>(*c).and_then(|ch| ch.iter().find_map(|t| w.get::<Text>(t))).map_or(String::new(), |t| t.0.clone())).collect())
        .collect();
    assert!(fields.iter().any(|t| t == "Home.Example:27015"), "the typed text as typed: {fields:?}");
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
    let recent = party_roster::main_menu::RecentFile(Some(dir.join(format!("chimera_test_recent_{}.txt", std::process::id()))));
    recent.write(&menu(&sim).recent);
    assert_eq!(recent.read(), ["192.0.2.7:27016"], "Recent after a restart");
    let files = BrowserFiles { favourites: None, history: Some(dir.join(format!("chimera_test_recent_hist_{}.txt", std::process::id()))) };
    files.write_history(&sim.app.world().resource::<LanList>().visits);
    assert!(files.read_history().iter().any(|v| v.addr == addr), "History after a restart");
    for p in [recent.0.as_ref(), files.history.as_ref()].into_iter().flatten() {
        let _ = std::fs::remove_file(p);
    }

    // A start with `+connect` remembers its address.
    assert_eq!(party_roster::main_menu::command_line_join(&["connect \"192.0.2.9:27015\"".into()]).as_deref(), Some("192.0.2.9:27015"));
    let mut args = chimera_client::client::Args::default();
    args.console = vec!["connect 192.0.2.9:27015".into()];
    sim.app.insert_resource(chimera_client::client::ClientArgs(args));
    party_roster::main_menu::remember_command_line_join(sim.app.world_mut());
    assert_eq!(menu(&sim).recent, ["192.0.2.9:27015", "192.0.2.7:27016"]);
    let nine: SocketAddr = "192.0.2.9:27015".parse().unwrap();
    assert_eq!(sim.app.world().resource::<LanList>().visits[0].addr, nine, "newest first in the History");
}

// ---------------------------------------------------------------------------
// The Internet tab (the hub's list, docs/plans/active/hub.md 4.3).

use chimera_net::net::hub::{ClientInfo, Discovery, Hub, HubGame, HubNews, Listing, Reach, ServerList};

const OUR_GAME: &str = "ultimate_gun_game";

/// A hub's listing of server `ip` (on this machine, a port nothing
/// answers on: a query of it leaves this box nowhere).
fn listing(ip: u8, name: &str, game: &str, net: &str) -> Listing {
    Listing {
        addr: format!("127.0.0.1:{}", 28200 + ip as u16),
        lan: Vec::new(),
        instance: format!("{:016x}", 0x1000 + ip as u64),
        name: name.into(),
        game: game.into(),
        minigame: Some("ultimate_gun_game:auto".into()),
        map: "de_dust2".into(),
        players: 5,
        max: 12,
        bots: 1,
        password: false,
        dedicated: true,
        net: net.into(),
        reachable: Reach::Yes,
        since: 0,
        mine: false,
        hint: None,
    }
}

/// What the server listed as `l` answers at `addr` (None: its listed
/// address), `ping_ms` away.
fn answer(l: &Listing, addr: Option<&str>, ping_ms: u32) -> QueryResult {
    let addr: SocketAddr = addr.unwrap_or(&l.addr).parse().unwrap();
    let mut r = server(addr.port(), &l.name, ping_ms, l.password, GAME_FOLDER);
    r.addr = addr;
    r.lan = false;
    if let QueryState::Answered { info, .. } = &mut r.state {
        info.instance = u64::from_str_radix(&l.instance, 16).ok();
        info.version = l.net.clone();
    }
    r
}

#[test]
fn the_internet_list_is_the_hubs_asked_directly_and_sorted_by_ping() {
    let far = listing(1, "Far", OUR_GAME, NET_VERSION);
    let near = listing(2, "Near", OUR_GAME, NET_VERSION);
    let asked = listing(3, "Still asked", OUR_GAME, NET_VERSION);
    let silent = listing(4, "Silent", OUR_GAME, NET_VERSION);
    let newer = listing(5, "Newer build", OUR_GAME, "0.9.9/net999");
    let other = listing(6, "Another app", "lucker_party", NET_VERSION);
    let mut home = listing(7, "Same router", OUR_GAME, NET_VERSION);
    home.lan = vec!["192.168.1.40:27015".into()];
    let impostor = listing(8, "Not that one", OUR_GAME, NET_VERSION);
    let mut mine = listing(9, "My own", OUR_GAME, NET_VERSION);
    (mine.mine, mine.reachable, mine.hint) = (true, Reach::No, Some("the hub can't reach UDP 27015".into()));
    let list = vec![far.clone(), near.clone(), asked.clone(), silent.clone(), newer.clone(), other.clone(), home.clone(), impostor.clone(), mine.clone()];
    let mut wrong = answer(&impostor, None, 1);
    if let QueryState::Answered { info, .. } = &mut wrong.state {
        info.instance = Some(0xdead);
    }
    let results = vec![
        answer(&far, None, 140),
        answer(&near, None, 18),
        QueryResult { addr: asked.addr.parse().unwrap(), state: QueryState::Pending, lan: false },
        QueryResult { addr: silent.addr.parse().unwrap(), state: QueryState::NoResponse, lan: false },
        answer(&newer, None, 30),
        answer(&other, None, 5),
        // Reached both ways: the LAN address is the nearer one.
        answer(&home, None, 44),
        answer(&home, Some("192.168.1.40:27015"), 2),
        // Another server answers at the impostor's address.
        wrong,
        QueryResult { addr: mine.addr.parse().unwrap(), state: QueryState::NoResponse, lan: false },
    ];
    let rows = sl::internet_servers(&list, &results, OUR_GAME, false, &|_| true);
    let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
    // Answered by ping, then the one still asked, then the silent ones;
    // the other app's server is left out.
    assert_eq!(&names[..4], ["Same router", "Near", "Newer build", "Far"], "{names:?}");
    assert_eq!(names[4], "Still asked");
    assert_eq!(names.len(), 8, "{names:?}");
    assert!(!names.contains(&"Another app"));
    let row = |n: &str| rows.iter().find(|r| r.name == n).unwrap();
    assert_eq!(row("Same router").addr, "192.168.1.40:27015".parse::<SocketAddr>().unwrap(), "joined over the LAN");
    assert_eq!(row("Same router").ping_ms, 2);
    // The hub's numbers fill a row until its server answers.
    let a = row("Still asked");
    assert_eq!((a.status, a.listed, a.players, a.max_players, a.map.as_str()), (Status::Asking, true, 5, 12, "de_dust2"));
    assert_eq!(row("Silent").status, Status::Down);
    assert_eq!(row("Not that one").status, Status::Down, "another instance's answer isn't its");
    // Another build: greyed, and why.
    let n = row("Newer build");
    assert!(!n.same_version);
    assert_eq!(n.note.as_deref(), Some("needs build 0.9.9/net999"));
    // The player's own unreachable server carries the hub's hint.
    assert_eq!(row("My own").note.as_deref(), Some("the hub can't reach UDP 27015"));
    // Every game asked for: the other app's is listed, barred.
    let all = sl::internet_servers(&list, &results, OUR_GAME, true, &|_| true);
    let o = all.iter().find(|r| r.name == "Another app").expect("listed");
    assert!(o.barred && o.note.as_deref().is_some_and(|n| n.contains("lucker party")), "{o:?}");
    // A game the account isn't invited to: barred.
    let barred = sl::internet_servers(&list, &results, OUR_GAME, false, &|_| false);
    assert!(barred.iter().all(|r| r.barred));
}

#[test]
fn the_hubs_list_is_asked_sixty_four_servers_at_once() {
    let list: Vec<Listing> = (0..150u8).map(|i| Listing { addr: format!("203.0.113.{i}:27015"), ..listing(i, "S", OUR_GAME, NET_VERSION) }).collect();
    let mut q = sl::HubQueue::default();
    q.fill(&list);
    let first = q.next(|_| true);
    assert_eq!(first.len(), sl::MAX_QUERIES);
    assert!(q.next(|_| true).is_empty(), "none while those are awaited");
    // Ten answered: ten more asked.
    let done: Vec<SocketAddr> = first[..10].to_vec();
    assert_eq!(q.next(|a| !done.contains(&a)).len(), 10);
    assert!(q.busy());
    let mut asked = sl::MAX_QUERIES + 10;
    while q.busy() {
        asked += q.next(|_| false).len();
    }
    assert_eq!(asked, 150, "each once");
}

/// A signed-in hub with `list` (its account file nowhere: asking it again
/// fails, as a hub that doesn't answer).
fn hub_with(sim: &mut Sim, list: Vec<Listing>, doc: Option<Discovery>) {
    let mut hub = Hub::at("https://hub.invalid", std::env::temp_dir().join("chimera_test_no_such_account.toml"));
    hub.doc = doc;
    hub.set_list(ServerList { servers: list, now: 0 });
    sim.app.insert_resource(HubGame(OUR_GAME.into()));
    sim.app.insert_resource(hub);
}

#[test]
fn join_opens_on_the_internet_tab_and_joins_a_listed_server() {
    let mut sim = sim(PAGES_THEME);
    let (slow, fast, newer) = (listing(1, "Slow", OUR_GAME, NET_VERSION), listing(2, "Fast", OUR_GAME, NET_VERSION), listing(3, "Newer", OUR_GAME, "0.9.9/net999"));
    hub_with(&mut sim, vec![slow.clone(), fast.clone(), newer.clone(), listing(4, "Elsewhere", "lucker_party", NET_VERSION)], None);
    open_join_with(&mut sim, vec![answer(&slow, None, 90), answer(&fast, None, 9), answer(&newer, None, 20)]);
    let lan = sim.app.world().resource::<LanList>().clone();
    assert_eq!(lan.tab, BrowserTab::Internet, "signed in to a hub: its list first");
    let names: Vec<&str> = lan.internet.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["Fast", "Newer", "Slow"], "by ping; another app's left out");
    assert!(lan.servers.is_empty(), "none of them is a LAN server");
    // The first row joins on Enter.
    let first = lines_of(&sim).iter().position(|l| *l == Line::Server(0)).unwrap();
    for _ in 0..first {
        press(&mut sim, KeyCode::ArrowDown);
    }
    // The other build's row is refused, and says which build it needs.
    press(&mut sim, KeyCode::ArrowDown);
    press(&mut sim, KeyCode::Enter);
    sim.ticks(1);
    assert!(asked(&sim).is_empty());
    assert_eq!(pages(&sim).notice.as_deref(), Some("Newer needs build 0.9.9/net999"));
    press(&mut sim, KeyCode::ArrowUp);
    press(&mut sim, KeyCode::Enter);
    sim.ticks(1);
    assert_eq!(asked(&sim), ["connect 127.0.0.1:28202"]);
    assert_eq!(menu(&sim).screen, Screen::Joining);
}

#[test]
fn a_hub_that_does_not_answer_shows_its_last_list() {
    let mut sim = sim(PAGES_THEME);
    let s = listing(1, "Remembered", OUR_GAME, NET_VERSION);
    hub_with(&mut sim, vec![s.clone()], None);
    open_join_with(&mut sim, vec![answer(&s, None, 30)]);
    // Opening Join asked the hub again; nothing answers (no account file).
    sim.expect_until("the hub's offline answer", |w| w.resource::<LanList>().hub.offline.is_some(), 20_000);
    let lan = sim.app.world().resource::<LanList>();
    assert!(lan.hub.offline.is_some() && lan.hub.has_list, "{:?}", lan.hub);
    assert_eq!(lan.internet.len(), 1, "the last list stays");
    assert_eq!(lan.internet[0].name, "Remembered");
    // LAN is a key away, as ever (F2).
    press(&mut sim, KeyCode::F2);
    assert_eq!(sim.app.world().resource::<LanList>().tab, BrowserTab::Lan);
    press(&mut sim, KeyCode::F1);
    assert_eq!(sim.app.world().resource::<LanList>().tab, BrowserTab::Internet);
}

#[test]
fn without_a_hub_join_opens_on_lan() {
    let mut sim = sim(PAGES_THEME);
    open_join_with(&mut sim, lan_results(2));
    let lan = sim.app.world().resource::<LanList>();
    assert_eq!(lan.tab, BrowserTab::Lan);
    assert!(!lan.hub.signed_in && lan.internet.is_empty());
}

#[test]
fn the_news_card_is_the_hubs_update_notice_then_its_news_then_its_motd() {
    use party_roster::signal_menu::MenuNews;
    let mut sim = sim(PAGES_THEME);
    let doc = Discovery {
        name: "Test hub".into(),
        motd: "Playtest Saturday 13:00 UTC".into(),
        news: vec![
            HubNews { id: "a".into(), date: "2026-11-10".into(), title: "Old".into(), text: "Before.".into() },
            HubNews { id: "b".into(), date: "2026-11-16".into(), title: "Three new maps".into(), text: "In the pool tonight.".into() },
        ],
        ..Default::default()
    };
    hub_with(&mut sim, Vec::new(), Some(doc.clone()));
    sim.ticks(2);
    let card = |sim: &Sim| sim.app.world().resource::<MenuNews>().0.clone();
    let n = card(&sim).expect("the newest news");
    assert_eq!((n.title.as_str(), n.body.as_str(), n.dismiss.as_deref()), ("Three new maps", "In the pool tonight.", Some("b")));
    assert!(n.source.contains("Test hub") && n.source.contains("Playtest Saturday 13:00 UTC"), "the MOTD beside the hub's name: {}", n.source);
    // Dismissed: the one before it, then the MOTD alone.
    act(&mut sim, A::NewsDismiss);
    assert_eq!(card(&sim).unwrap().title, "Old");
    act(&mut sim, A::NewsDismiss);
    let m = card(&sim).expect("the MOTD");
    assert_eq!((m.title.as_str(), m.source.as_str(), m.dismiss), ("Playtest Saturday 13:00 UTC", "Test hub", None));
    assert_eq!(sim.app.world().resource::<Hub>().dismissed, ["b".to_string(), "a".to_string()]);
    // A build under the hub's floor: said first, on the card and over the
    // browser; LAN and the rest work as before.
    let old = Discovery { client: ClientInfo { min_net: 99_999, recommended_build: "9.9.9".into(), download: "example.invalid/releases".into() }, ..doc };
    sim.app.world_mut().resource_mut::<Hub>().doc = Some(old);
    sim.ticks(2);
    let u = card(&sim).expect("the notice");
    assert_eq!(u.title, signal_pages::UPDATE_TITLE);
    assert!(u.body.contains("9.9.9") && u.body.contains("example.invalid/releases"), "{}", u.body);
    open_join_with(&mut sim, lan_results(1));
    let lan = sim.app.world().resource::<LanList>();
    assert_eq!(lan.hub.update, Some(("9.9.9".to_string(), "example.invalid/releases".to_string())));
    let banner = sim.app.world_mut().query::<&chimera_ui::UiParityId>().iter(sim.app.world()).filter(|p| p.0 == "join.update").count();
    assert_eq!(banner, 1, "the banner over the browser");
}

#[test]
fn ui_scene_stages_the_internet_tab() {
    let mut sim = sim(PAGES_THEME);
    signal_pages::stage(sim.app.world_mut(), MenuPage::Join, &["internet".into()]).unwrap();
    sim.ticks(3);
    let lan = sim.app.world().resource::<LanList>().clone();
    assert_eq!(lan.tab, BrowserTab::Internet);
    assert_eq!(lan.internet.len(), 9, "this app's servers of the made-up hub");
    assert!(lan.internet.windows(2).take(6).all(|w| w[0].ping_ms <= w[1].ping_ms), "by ping");
    assert!(lan.internet.iter().any(|r| !r.same_version) && lan.internet.iter().any(|r| r.status == Status::Asking));
    assert_eq!(lan.internet.last().unwrap().status, Status::Down);
    act(&mut sim, A::BrowserAllGames);
    assert_eq!(sim.app.world().resource::<LanList>().internet.len(), 10, "every game's");
    act(&mut sim, A::Back);
    for (arg, check) in [("offline", 0), ("update", 1), ("nohub", 2)] {
        signal_pages::stage(sim.app.world_mut(), MenuPage::Join, &[arg.into()]).unwrap();
        sim.ticks(3);
        let lan = sim.app.world().resource::<LanList>();
        match check {
            0 => assert!(lan.hub.offline.is_some() && !lan.internet.is_empty()),
            1 => assert!(lan.hub.update.is_some()),
            _ => assert!(!lan.hub.signed_in && lan.internet.is_empty()),
        }
        act(&mut sim, A::Back);
    }
    signal_pages::stage(sim.app.world_mut(), MenuPage::Join, &["off".into()]).unwrap();
    sim.ticks(2);
    assert!(sim.app.world().resource::<LanList>().internet.is_empty(), "nothing of the staging stays");
}

/// Left on a map folds its group and puts the cursor on the group's
/// header; on a folded group it goes up to the game; on an open one it
/// folds; Right opens a folded row.
#[test]
fn left_on_a_map_collapses_its_group_and_moves_to_the_header() {
    use party_roster::maps_picker::{MapCatalogue, MapsPicker, TreeRow};
    let games = party_roster::maps_picker::LocalCatalogue.listing(&["g:a".into(), "g:b".into(), "g:c".into()]);
    let mut p = MapsPicker::default();
    assert_eq!(p.rows(&games).len(), 5, "game, group, three maps");
    p.cursor = 3;
    assert!(p.left(&games));
    assert_eq!(p.rows(&games), [TreeRow::Game(0), TreeRow::Group(0, 0)], "the group folded");
    assert_eq!(p.cursor, 1, "the cursor on the group's header");
    assert!(p.left(&games));
    assert_eq!(p.cursor, 0, "a folded group: up to its game");
    assert!(p.left(&games));
    assert_eq!(p.rows(&games), [TreeRow::Game(0)], "an open game folds");
    assert!(!p.left(&games), "nothing further");
    assert!(p.right(&games));
    assert_eq!(p.rows(&games).len(), 2, "the game opens (its group still folded)");
    assert!(!p.right(&games) || p.rows(&games).len() == 5);
}

/// The Host page under its Maps and Guns dialogs (`chimera_ui::modal`):
/// the wheel with the pointer over the page's rows scrolls nothing behind
/// the dialog, the page's rows take no click, the keys are the dialog's,
/// and Esc closes the dialog only (Host stays up). With no dialog the
/// same wheel scrolls the page.
#[test]
fn host_dialogs_take_the_pointer_and_keys_from_the_page() {
    use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
    use bevy::ui::{ComputedNode, DeprecatedInteraction as Interaction, RelativeCursorPosition};
    use chimera_ui::{Modals, ScrollViews, scroll::{ScrollContent, ScrollView}};
    use party_roster::{guns_picker::{GunsPicker, GunsSource}, maps_picker::MapsPicker};
    let mut sim = sim(PAGES_THEME);
    sim.app.insert_resource(GunsSource(Box::new(FakeGuns)));
    sim.app.init_resource::<AccumulatedMouseScroll>();
    press(&mut sim, KeyCode::Enter);
    sim.ticks(2);
    assert_eq!(pages(&sim).up, Some(MenuPage::Play));
    // The page's view laid out by hand (20 cq over 60), the pointer on it.
    let wheel_over_page = |sim: &mut Sim| {
        let w = sim.app.world_mut();
        let view = w.query::<(Entity, &ScrollView)>().iter(w).find(|(_, v)| v.key == signal_pages::PLAY_SCROLL).map(|(e, _)| e).expect("the page's view");
        let content = w.query::<(Entity, &ScrollContent)>().iter(w).find(|(_, c)| c.view == view).map(|(e, _)| e).unwrap();
        let px = |h: f32| ComputedNode { size: Vec2::new(600.0, h * 19.2), inverse_scale_factor: 1.0, ..default() };
        w.entity_mut(view).insert((px(20.0), RelativeCursorPosition { cursor_over: true, normalized: Some(Vec2::ZERO) }));
        w.entity_mut(content).insert(px(60.0));
        w.resource_mut::<ScrollViews>().reset(signal_pages::PLAY_SCROLL);
        sim.ticks(1);
        let w = sim.app.world_mut();
        w.entity_mut(view).insert(RelativeCursorPosition { cursor_over: true, normalized: Some(Vec2::ZERO) });
        w.insert_resource(AccumulatedMouseScroll { unit: MouseScrollUnit::Line, delta: Vec2::new(0.0, -1.0) });
        sim.ticks(1);
        sim.app.world_mut().insert_resource(AccumulatedMouseScroll::default());
        sim.app.world().resource::<ScrollViews>().get(signal_pages::PLAY_SCROLL).is_some_and(|s| s.offset > 0.0 || s.target.is_some_and(|t| t > 0.0))
    };
    assert!(wheel_over_page(&mut sim), "no dialog: the wheel scrolls the page");
    let data = MenuData::shipped();
    for (kind, open) in [
        (party_roster::main_menu::PlayKind::Map, (|s: &Sim| s.app.world().resource::<MapsPicker>().open.is_some()) as fn(&Sim) -> bool),
        (party_roster::main_menu::PlayKind::Guns, |s: &Sim| s.app.world().resource::<GunsPicker>().open.is_some()),
    ] {
        let row = data.play.iter().position(|r| r.kind == kind).unwrap();
        while pages(&sim).focus > row {
            press(&mut sim, KeyCode::ArrowUp);
        }
        while pages(&sim).focus < row {
            press(&mut sim, KeyCode::ArrowDown);
        }
        press(&mut sim, KeyCode::Enter);
        sim.ticks(3);
        assert!(open(&sim), "{kind:?}: the dialog is open");
        assert!(sim.app.world().resource::<Modals>().is_open(), "{kind:?}: a modal layer");
        assert!(!wheel_over_page(&mut sim), "{kind:?}: the wheel over the page behind the dialog scrolls nothing");
        // A click on a page row behind the dialog: no press reaches it.
        let focus = pages(&sim).focus;
        let w = sim.app.world_mut();
        let rows: Vec<Entity> = w.query_filtered::<Entity, With<signal_pages::PageLine>>().iter(w).collect();
        for &e in &rows {
            if let Some(mut i) = w.get_mut::<Interaction>(e) {
                *i = Interaction::Pressed;
            }
        }
        sim.ticks(2);
        assert!(rows.iter().all(|e| sim.app.world().get::<Interaction>(*e).is_none_or(|i| *i == Interaction::None)), "{kind:?}: nothing pressed behind");
        assert_eq!(pages(&sim).focus, focus, "{kind:?}: the click behind moved no focus");
        // Tab and the arrows are the dialog's; Esc closes it, Host stays.
        press(&mut sim, KeyCode::Tab);
        assert_eq!(pages(&sim).focus, focus, "{kind:?}: Tab doesn't drive the page behind");
        press(&mut sim, KeyCode::Escape);
        sim.ticks(2);
        assert!(!open(&sim), "{kind:?}: Esc closed the dialog");
        assert_eq!(pages(&sim).up, Some(MenuPage::Play), "{kind:?}: and only the dialog");
        assert!(!sim.app.world().resource::<Modals>().is_open(), "{kind:?}: no modal left");
    }
}

/// Host's Start, hovered, stays hovered through the page drawn again (a
/// letter typed into the server name redraws it, as any change of the
/// page does): the new button is hovered and lifted from its first frame
/// (the report "the start game button keeps flashing while hovering":
/// each redraw showed it at rest and eased the hover in again;
/// `chimera_ui::controls::carry_hover`).
#[test]
fn a_hovered_start_stays_hovered_through_a_redraw() {
    use bevy::ui::DeprecatedInteraction as Interaction;
    use chimera_ui::controls::HeroSlab;
    let mut sim = sim(PAGES_THEME);
    press(&mut sim, KeyCode::Enter);
    sim.ticks(2);
    assert_eq!(menu(&sim).page, MenuPage::Play);
    press(&mut sim, KeyCode::Enter);
    assert!(menu(&sim).focus.is_some(), "the server name has the keys");
    let start = |sim: &mut Sim| {
        let w = sim.app.world_mut();
        let b = w.query::<(Entity, &A)>().iter(w).find(|(_, a)| **a == A::Start).map(|(e, _)| e).expect("Start");
        let slab = w.get::<Children>(b).unwrap().iter().find(|c| w.get::<HeroSlab>(*c).is_some()).unwrap();
        (b, *w.get::<Interaction>(b).unwrap(), w.get::<HeroSlab>(slab).unwrap().lift)
    };
    let (s0, _, _) = start(&mut sim);
    *sim.app.world_mut().get_mut::<Interaction>(s0).unwrap() = Interaction::Hovered;
    for _ in 0..3 {
        sim.app.world_mut().resource_mut::<chimera_ui::UiClock>().step(0.1);
        sim.ticks(1);
    }
    assert_eq!(start(&mut sim), (s0, Interaction::Hovered, 1.0), "hovered and lifted");
    type_text(&mut sim, "x");
    let (s1, i, lift) = start(&mut sim);
    assert_ne!(s1, s0, "the page was drawn again");
    assert_eq!((i, lift), (Interaction::Hovered, 1.0), "the new Start as the old one was");
}

/// The Guns dialog's own dropdown closes when a modal opens over it or
/// another popover opens; its own opening spares it (`ClosePopovers`).
#[test]
fn guns_dialog_dropdown_closes_for_modals_and_other_popovers() {
    use party_roster::guns_picker::GunsAct as G;
    let dir = scratch("popovers");
    let mut sim = guns_sim(&dir);
    guns_act(&mut sim, G::Menu);
    assert!(working(&sim).unwrap().menu.is_some(), "the menu opens");
    assert!(working(&sim).unwrap().menu.is_some(), "opening it did not close it");
    sim.app.world_mut().write_message(chimera_ui::ClosePopovers::except("host"));
    sim.ticks(2);
    assert!(working(&sim).unwrap().menu.is_none(), "another popover opened: it closes");
    guns_act(&mut sim, G::Menu);
    sim.app.world_mut().spawn((Node::default(), chimera_ui::Modal));
    sim.ticks(3);
    assert!(working(&sim).unwrap().menu.is_none(), "a modal opened over the dialog: it closes");
}

// ---------------------------------------------------------------------------
// Spectating from Join, and a refusal's box (ugg-ui.md, "Spectators").

fn failure_boxes(sim: &mut Sim) -> usize {
    sim.app.world_mut().query_filtered::<(), With<party_roster::failure_signal::FailureSignalRoot>>().iter(sim.app.world()).count()
}

#[test]
fn join_offers_a_spectator_toggle_in_the_kit() {
    let mut sim = sim(PAGES_THEME);
    sim.app.init_resource::<chimera_net::spectators::JoinAsSpectator>();
    act(&mut sim, A::Entry(1));
    assert_eq!(menu(&sim).page, MenuPage::Join);
    let lines = signal_pages::lines_now(sim.app.world());
    let at = lines.iter().position(|l| *l == Line::Spectate).expect("a Spectate line on Join");
    let on = |sim: &Sim| sim.app.world().resource::<chimera_net::spectators::JoinAsSpectator>().0;
    // Down to it (the address and password lines come first), Enter flips it.
    for _ in 0..at {
        press(&mut sim, KeyCode::ArrowDown);
    }
    assert_eq!(pages(&sim).focus, at);
    press(&mut sim, KeyCode::Enter);
    assert_eq!(on(&sim), 1, "Enter turns it on, and does not connect");
    assert!(asked(&sim).is_empty());
    press(&mut sim, KeyCode::Space);
    assert_eq!(on(&sim), 0, "Space turns it off");
    // The toggle is a button of the kit's: pressing it is the same action.
    act(&mut sim, A::ToggleSpectator);
    assert_eq!(on(&sim), 1);
}

#[test]
fn a_failed_join_shows_in_the_kit_box_and_close_closes_it() {
    let mut sim = sim(PAGES_THEME);
    assert_eq!(failure_boxes(&mut sim), 0);
    submit(&mut sim, "mainmenu_failure No room for another spectator on this server.");
    assert!(menu(&sim).failure.is_some());
    assert_eq!(failure_boxes(&mut sim), 1, "the kit's box");
    act(&mut sim, A::CloseFailure);
    assert!(menu(&sim).failure.is_none());
    assert_eq!(failure_boxes(&mut sim), 0);
}
