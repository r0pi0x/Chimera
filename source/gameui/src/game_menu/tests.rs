use super::*;
use crate::options::setting_index;
use crate::map::hud::{GameUiItem, ServerSetting};

/// A game with everything the menus can hold of a game's own (CS:S's
/// shape, as `cs_source::ui::MENU_GAME`; CS:S's own numbers aren't
/// needed here).
const DIFFICULTIES: [(&str, f32, f32, f32); 4] =
    [("Easy", 0.6, 5.0, 220.0), ("Normal", 0.35, 2.5, 360.0), ("Hard", 0.2, 1.5, 540.0), ("Expert", 0.12, 0.75, 720.0)];
const QUICK_MAP: &str = "de_dust2";
const QUICK_BOTS: u8 = 9;
const ROUNDS_CVAR: &str = "chimera_rounds";
const BOT_TEAMS: [(&str, &str); 3] = [("", "Any"), ("#Team_T", "Terrorists"), ("#Team_CT", "Counter-Terrorists")];

static TEST_INFO: crate::game_registry::GameInfo = crate::game_registry::GameInfo {
    id: "a_game",
    short_name: "A",
    content_dir: "a",
    tick_interval: 0.015,
    open: |_| Err(std::io::Error::other("no install")),
    map_cycle: |_| Vec::new(),
    map_names: |_| Vec::new(),
    map_file: |n| Some(format!("maps/{n}.bsp")),
    steam: None,
    import_map: |_| Err("no".into()),
    default_binds_cfg: "cfg/default.cfg",
    default_binds: &[
        ("tab", "+showscores"),
        ("space", "+jump"),
        ("mouse1", "+attack"),
        ("w", "+forward"),
        ("ctrl", "+duck"),
        ("f1", "autobuy"),
        ("mwheelup", "invprev"),
    ],
    old_default_binds: &[],
    movement_cvars: &[],
    meters_per_unit: 0.0254,
    console_namespaces: &[],
};

static TEST_GAME: crate::MenuGame = crate::MenuGame {
    info: &TEST_INFO,
    name: "A Game",
    steam_folder: "A Game",
    strings: "resource/a_english.txt",
    bots: Some(crate::BotMenu {
        difficulties: &DIFFICULTIES,
        teams: BOT_TEAMS,
        cvars: [
            ("BotPrefixEntry", "bot_prefix", "#Bot_NamePrefix"),
            ("BotJoinAfterPlayerCheck", "bot_join_after_player", "#Bot_JoinAfterPlayer"),
        ],
        include_token: "#Bot_IncludeBots",
        count_token: "#Bot_NumberOfBots",
        difficulty_token: "#Bot_Difficulty",
        join_team_token: "#Bot_JoinTeam",
        team_words: ["terrorist", "counter-terrorist"],
    }),
    rounds_cvar: Some(ROUNDS_CVAR),
    teams: Some(["Terrorists", "Counter-Terrorists"]),
    quick_start: Some(crate::QuickStart { map: QUICK_MAP, bots: QUICK_BOTS }),
    layout_hides: false,
    player_models: false,
    sprays: false,
    server_types: false,
};

const TEST: crate::GameRef = crate::GameRef(Some(&TEST_GAME));

const CVARS: [(&str, &str); 19] = [
    ("sensitivity", "3"),
    ("m_pitch", "0.022"),
    ("zoom_sensitivity_ratio", "1.2"),
    ("volume", "1"),
    ("viewmodel_fov", "54"),
    ("cl_righthand", "1"),
    ("cl_showfps", "0"),
    ("chimera_drawhitboxes", "0"),
    ("snd_show", "1"),
    ("mat_vsync", "1"),
    ("mat_antialias", "4"),
    ("chimera_resolution", ""),
    ("chimera_rounds", "1"),
    ("bot_reaction", "0.35"),
    // Ours by default (CS:S's own is 0).
    ("hud_fastswitch", "1"),
    ("con_enable", "1"),
    ("cl_autowepswitch", "1"),
    ("hud_centerid", "1"),
    ("cl_disablefreezecam", "0"),
];

fn get(n: &str) -> Option<String> {
    CVARS
        .iter()
        .chain(&[("hostname", "Mine"), ("mp_friendlyfire", "0"), ("bot_prefix", "")])
        .find(|(k, _)| *k == n)
        .map(|(_, v)| v.to_string())
}

/// The menu opened in a game (Esc).
fn menu() -> GameMenu {
    let mut m = GameMenu {
        game: TEST,
        in_game: true,
        ..default()
    };
    m.open(Page::Main, vec!["cs_office".into(), "de_dust2".into(), "de_nuke".into()], Some("de_dust2"), get);
    m
}

/// The menu at startup: the main menu, out of a game.
fn main_menu() -> GameMenu {
    let mut m = GameMenu::for_game(&TEST_GAME);
    m.open(Page::Main, vec!["cs_office".into(), "de_dust2".into(), "de_nuke".into()], None, |_| None);
    m
}

#[test]
fn cancel_stops_a_load_and_nothing_else_works_meanwhile() {
    let mut m = main_menu();
    m.loading = Some("de_dust2".into());
    let before = m.clone();
    assert!(m.handle(Input::Click(Target::Main(0), 0)).lines.is_empty());
    assert_eq!(m, before, "the dialog takes no other input");
    let out = m.handle(Input::Click(Target::Cancel, 0));
    assert_eq!(out.lines, vec!["disconnect".to_string()]);
    assert!(m.loading.is_none() && m.open && !m.in_game);
}

#[test]
fn a_map_loaded_from_inside_a_game_shows_the_loading_dialog() {
    let mut m = menu();
    assert!(m.in_game);
    m.begin_loading("de_nuke".into());
    assert_eq!(m.loading.as_deref(), Some("de_nuke"));
    assert!(m.open && !m.in_game && m.page == Page::Main);
    // Once per load: a second call keeps the first.
    m.begin_loading("de_dust2".into());
    assert_eq!(m.loading.as_deref(), Some("de_nuke"));
    // Cancel is `disconnect`.
    assert_eq!(m.handle(Input::Click(Target::Cancel, 0)).lines, ["disconnect"]);
}

#[test]
fn the_loading_line_follows_the_load() {
    let m = main_menu();
    assert_eq!(loading_text(&m, None, "de_nuke"), "Loading de_nuke ...");
    let p = crate::map::loading::LoadProgress {
        fraction: 0.5,
        stage: "LoadingProgress_LoadResources",
    };
    // Without the game's strings, the token itself.
    assert_eq!(loading_text(&m, Some(p), "de_nuke"), "LoadingProgress_LoadResources");
}

#[test]
fn joining_shows_its_stages_and_the_download() {
    use crate::net::client::{DownloadInfo, JoinProgress, JoinStage};
    let m = main_menu();
    let mut j = JoinProgress {
        stage: JoinStage::Connecting,
        ..default()
    };
    assert_eq!(dialog_state(&m, Some(&j), None, false, "x").0, "Connecting to server...");
    j.stage = JoinStage::ServerInfo;
    assert_eq!(dialog_state(&m, Some(&j), None, false, "x").0, "Retrieving server info...");
    j.stage = JoinStage::ChangingLevel;
    assert_eq!(dialog_state(&m, Some(&j), None, false, "x").0, "Server is changing level...");
    j.stage = JoinStage::Downloading;
    j.download = Some(DownloadInfo {
        file: "maps/a.bsp".into(),
        from: "server".into(),
        done: 250,
        total: 1000,
    });
    assert_eq!(
        dialog_state(&m, Some(&j), None, false, "x"),
        ("Verifying and downloading resources...".to_string(), 0.25)
    );
    // Loading the map: the map's own stages.
    j.stage = JoinStage::LoadingMap;
    let p = crate::map::loading::LoadProgress {
        fraction: 0.5,
        stage: "LoadingProgress_LoadMap",
    };
    assert_eq!(dialog_state(&m, Some(&j), Some(p), false, "x").1, 0.5);
    // A host starting its server, before the map reports.
    assert_eq!(
        dialog_state(&m, None, None, true, "de_dust2").0,
        "Starting local game server..."
    );
    assert_eq!(dialog_state(&m, None, None, false, "de_dust2").0, "Loading de_dust2 ...");
    let (stage, text) = detail_stage(Some(&JoinProgress {
        stage: JoinStage::Downloading,
        download: j.download.clone(),
        ..default()
    }), None);
    assert_eq!(stage, "downloading the map");
    assert_eq!(text, "0 KB of 1 KB (25%) of maps/a.bsp from server");
}

#[test]
fn a_failure_shows_until_closed() {
    use crate::net::client::JoinFailure;
    let mut m = main_menu();
    assert_eq!(net_failure_text(&m, &JoinFailure::Full, "Server is full."), "Server is full.");
    m.failure = Some("Server is full.".into());
    m.loading = Some(String::new());
    assert!(m.handle(Input::Click(Target::Main(0), 0)).lines.is_empty());
    assert!(m.failure.is_some(), "only Close closes it");
    let out = m.handle(Input::Click(Target::Cancel, 0));
    assert!(out.lines.is_empty(), "already disconnected");
    assert!(m.failure.is_none() && m.loading.is_none() && m.open);
}

/// A shown entry's index.
fn at(m: &GameMenu, item: MainItem) -> usize {
    m.entries().iter().position(|e| e.item == item).unwrap()
}

/// Click a shown entry.
fn click(m: &mut GameMenu, item: MainItem) -> Outcome {
    let i = at(m, item);
    press(m, &[Input::Click(Target::Main(i), 0)])
}

fn shown(m: &GameMenu) -> Vec<MainItem> {
    m.entries().iter().map(|e| e.item).collect()
}

fn press(m: &mut GameMenu, inputs: &[Input]) -> Outcome {
    let mut all = Outcome::default();
    for i in inputs {
        let o = m.handle(i.clone());
        all.lines.extend(o.lines);
        all.after_load.extend(o.after_load);
        all.close |= o.close;
        all.clipboard = o.clipboard.or(all.clipboard);
    }
    all
}

/// Click the row `row` finds.
fn click_on(m: &mut GameMenu, row: impl Fn(&GameMenu) -> usize) -> Outcome {
    let i = row(m);
    press(m, &[Input::Click(Target::Row(i), 0)])
}

fn setting(cvar: &str) -> usize {
    SETTINGS.iter().position(|s| s.cvar == cvar).unwrap()
}

/// The row of the open page with this field.
fn row_with(m: &GameMenu, field: Field) -> usize {
    m.rows().iter().position(|r| r.field() == Some(field)).unwrap_or_else(|| panic!("no {field:?}"))
}

fn row_of(m: &GameMenu, cvar: &str) -> usize {
    row_with(m, Field::Setting(setting(cvar)))
}

fn button(m: &GameMenu, action: Action) -> usize {
    m.rows()
        .iter()
        .position(|r| matches!(r, Row::Button { action: a, .. } if *a == action))
        .unwrap()
}

fn control(m: &GameMenu, i: usize) -> Control {
    match &m.rows()[i] {
        Row::Control { control, .. } => control.clone(),
        r => panic!("{r:?}"),
    }
}

/// Options > Multiplayer's "Player name" (`name`): typed, set on Apply
/// or OK (one rename, not one per key), dropped by Cancel.
#[test]
fn the_player_name_in_the_multiplayer_options() {
    let named = |n: &str| if n == "name" { Some("Player".to_string()) } else { get(n) };
    let mut m = GameMenu {
        game: TEST,
        in_game: true,
        ..default()
    };
    m.open(Page::Settings, vec!["de_dust2".into()], Some("de_dust2"), named);
    m.set_tab(Tab::Multiplayer);
    let row = row_of(&m, "name");
    assert!(matches!(control(&m, row), Control::Text { ref text, numeric: false, max: 32 } if text == "Player"));
    let out = press(&mut m, &[Input::FocusRow(row), Input::Type("Zed".into())]);
    assert!(out.lines.is_empty(), "nothing set while typing: {:?}", out.lines);
    let typed = match control(&m, row_of(&m, "name")) {
        Control::Text { text, .. } => text,
        c => panic!("{c:?}"),
    };
    assert!(typed.ends_with("Zed"), "{typed}");
    let out = click_on(&mut m, |m| button(m, Action::Apply));
    assert_eq!(out.lines, [format!("name {}", crate::console::quote(&typed))]);
    // Typed again, then Cancel: nothing new set (the value goes back).
    let row = row_of(&m, "name");
    press(&mut m, &[Input::FocusRow(row), Input::Type("X".into())]);
    let out = click_on(&mut m, |m| button(m, Action::Cancel));
    assert!(out.lines.iter().all(|l| l == &format!("name {}", crate::console::quote(&typed))), "{:?}", out.lines);
}

/// The install's Multiplayer layout has no name entry (current CS:S
/// names you from Steam): it gets one in the spray words' place.
#[test]
fn the_multiplayer_layout_gets_a_name_entry() {
    let root = source_engine::hud::gameui::layout_of_res(
        r#""Resource/OptionsSubMultiplayer.res" {
            "Label2" { "ControlName" "Label" "fieldName" "Label2" "xpos" "16" "ypos" "148" "wide" "150" "tall" "24" "labelText" "Spray" }
            "Label3" { "ControlName" "Label" "fieldName" "Label3" "xpos" "95" "ypos" "175" "wide" "150" "tall" "80" "labelText" "Note" }
        }"#,
    );
    let mut layout = root.expect("a layout");
    let strings = HashMap::from([("gameui_playername".to_string(), "Player name".to_string())]);
    source_engine::hud::gameui::add_player_name(&mut layout, &strings);
    let entry = layout.get("NameEntry").expect("added");
    let label = layout.get("NameLabel").expect("added");
    assert_eq!(label.text, "Player name");
    assert!(class_of(entry).eq_ignore_ascii_case("TextEntry"));
    assert!(!layout.get("Label2").unwrap().visible && !layout.get("Label3").unwrap().visible);
    let before = layout.controls.len();
    source_engine::hud::gameui::add_player_name(&mut layout, &strings);
    assert_eq!(layout.controls.len(), before, "once");
}

#[test]
fn opens_with_the_game_as_it_is() {
    let m = menu();
    assert!(m.open);
    assert_eq!((m.page, m.focus), (Page::Main, 0));
    assert_eq!(m.new_game.map, 1, "the loaded map");
    assert_eq!(m.new_game.difficulty, NORMAL);
    assert!(!m.new_game.bots_on, "no bots in the game now");
    // No crosshair colour cvar given: not available.
    assert_eq!(m.values[setting("cl_crosshaircolor")], None);
    assert_eq!(m.entries().len(), MAIN.len());
}

#[test]
fn in_game_entries_show_only_in_a_game() {
    use MainItem::*;
    let m = menu();
    assert_eq!(
        shown(&m),
        [
            Resume,
            Disconnect,
            PlayerList,
            FindServers,
            NewGame,
            BugReport,
            Options,
            Quit
        ]
    );
    let e = m.entries();
    assert!(e[3].gap && e[3].enabled, "a gap under the player list, then Find Servers");
    assert_eq!(e.iter().filter(|e| e.gap).count(), 1, "none of ours after a gap");
    let m = main_menu();
    assert_eq!(shown(&m), [FindServers, NewGame, BugReport, Options, Quit]);
    let e = m.entries();
    assert!(!e[0].gap, "no gap over the first entry");
    assert_eq!(m.focus, 0, "the first entry: Find Servers");
    assert_eq!(e[1].label, "CREATE SERVER");
}

#[test]
fn find_servers_opens_the_server_browser() {
    let mut m = main_menu();
    let find = at(&m, MainItem::FindServers);
    let o = press(&mut m, &[Input::Hover(Target::Main(find)), Input::Click(Target::Main(find), 0)]);
    assert_eq!(o.lines, ["openserverbrowser"]);
    assert_eq!(m.focus, find);
    assert!(m.open && m.page == Page::Main, "the browser shows over the menu");
    // Up from it wraps to Quit.
    press(&mut m, &[Input::Up]);
    assert_eq!(m.entries()[m.focus].item, MainItem::Quit);
}

#[test]
fn the_main_menu_stays_open() {
    let mut m = main_menu();
    let o = press(&mut m, &[Input::Close]);
    assert!(!o.close && m.open && m.page == Page::Main, "Esc does nothing there");
    // Esc cancels a dialog, back to the main menu.
    click(&mut m, MainItem::Options);
    assert_eq!(m.page, Page::Settings);
    let o = press(&mut m, &[Input::Close]);
    assert!(!o.close && m.open && m.page == Page::Main);
    // A bug report from it keeps it open.
    let o = click(&mut m, MainItem::BugReport);
    assert_eq!(o.lines, ["bugreport"]);
    assert!(m.open && !o.close);
}

#[test]
fn create_server_starts_a_map_from_the_main_menu() {
    let mut m = main_menu();
    click(&mut m, MainItem::NewGame);
    assert_eq!((m.page, m.create_tab), (Page::NewGame, 0));
    // Enter: Start, the default button.
    let o = press(&mut m, &[Input::Activate]);
    assert_eq!(o.lines.last().map(String::as_str), Some("map cs_office"));
    // Create Server hosts a server others can join: never single player.
    let max = o.lines.iter().find_map(|l| l.strip_prefix("maxplayers ")).expect("maxplayers set");
    assert!(max.parse::<u32>().unwrap() > 1, "maxplayers {max}");
    // The menu stays up, loading, and takes no input until the map is in.
    assert!(!o.close && m.open);
    assert_eq!(m.loading.as_deref(), Some("cs_office"));
    assert_eq!(press(&mut m, &[Input::Close, Input::Activate]), Outcome::default());
    assert!(m.enter_game(), "the load closes the open menu");
    assert!(m.in_game && !m.open && m.loading.is_none());
    // Esc now opens the in-game menu, with Resume.
    m.open(Page::Main, vec!["cs_office".into()], Some("cs_office"), |_| None);
    assert_eq!(m.entries()[m.focus].item, MainItem::Resume);
}

/// `chimera_quickstart` (no menu entry: CS:S's menus have none).
#[test]
fn quick_start() {
    let mut m = main_menu();
    let mut o = Outcome::default();
    m.quick_start(&mut o);
    assert_eq!(
        o.lines,
        ["bot_kick", "chimera_rounds 1", "bot_reaction 0.35", "bot_aim_error 2.5", "bot_turn_rate 360", "map de_dust2"]
    );
    assert_eq!(o.after_load.len(), QUICK_BOTS as usize);
    assert_eq!(o.after_load.iter().filter(|l| *l == "bot_add 1").count(), 5, "five terrorists, four CTs");
    assert_eq!(m.loading.as_deref(), Some(QUICK_MAP));
    assert!(m.open && !o.close, "out of a game: the main menu shows the load");
    // From a game it closes the menu at once.
    let mut m = menu();
    let mut o = Outcome::default();
    m.quick_start(&mut o);
    assert!(o.close && !m.open && o.lines.last().map(String::as_str) == Some("map de_dust2"));
}

/// First launch (a fresh config): the main menu opens before the
/// install's GameUI scripts are read (the first-run dialog picks the
/// install, then they load). Create Server's Game page still lists
/// `settings.scr`'s options with their values once they are in.
#[test]
fn the_game_page_lists_the_scripts_options_read_after_the_menu_opened() {
    let mut app = App::new();
    app.add_plugins(crate::console::ConsolePlugin)
        .init_resource::<crate::net::NetSettings>()
        .init_resource::<GameMenu>()
        .init_resource::<MenuUi>()
        .init_resource::<RegrabCursor>()
        .init_resource::<Assets<Image>>()
        .init_resource::<Assets<Font>>()
        .init_resource::<crate::client::fonts::UiFonts>();
    crate::console::resource_cvar::<crate::net::NetSettings, u32>(&mut app, "maxplayers", "", |s| &mut s.maxplayers);
    let w = app.world_mut();
    open_menu(w, Page::Main);
    // The scripts come in with the main menu open.
    let ui = GameUi {
        server_settings: vec![ServerSetting {
            cvar: "maxplayers".into(),
            label: "Max players".into(),
            kind: ServerSettingKind::Number { min: Some(1.0), max: Some(32.0) },
            default: "32".into(),
        }],
        ..default()
    };
    w.resource_mut::<MenuUi>().loading = Some(Arc::new(Mutex::new(Some(Some(ui)))));
    w.run_system_cached(ui_loaded).unwrap();
    w.flush();
    let mut m = w.resource::<GameMenu>().clone();
    click(&mut m, MainItem::NewGame);
    m.set_create_tab(1);
    let max = m
        .new_game
        .cvars
        .iter()
        .position(|c| c.cvar == "maxplayers")
        .expect("maxplayers read");
    assert!(
        m.rows()
            .iter()
            .any(|r| matches!(r, Row::Control { field: Field::ServerCvar(i), .. } if *i == max)),
        "the Game page shows Max players"
    );
    // Single player's 1 shows the game's default: Create Server hosts.
    assert_eq!(m.new_game.cvars[max].value, "32");
}

/// Playtest 2026-10-10: the crosshair sat right of and below the centre
/// after Options > Multiplayer > Advanced on a scaled display (Windows at
/// 125 %). An in-game dialog draws the world into a picture
/// (`world_behind_dialogs`); back on the window, Bevy kept the picture's
/// size and scale for the camera. Each dialog opened and closed in a game
/// on a 1.25-scaled window: the camera's size is the window's again after,
/// and the crosshair's centre with it.
#[test]
fn the_camera_and_crosshair_are_the_windows_after_every_dialog() {
    use bevy::{
        camera::RenderTarget,
        window::{PrimaryWindow, WindowCreated, WindowResized, WindowResolution, WindowScaleFactorChanged},
    };
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), crate::console::ConsolePlugin))
        .init_asset::<Image>()
        .init_asset::<Font>()
        .add_message::<WindowResized>()
        .add_message::<WindowCreated>()
        .add_message::<WindowScaleFactorChanged>()
        .init_resource::<bevy::render::texture::ManualTextureViews>()
        .insert_resource(GameMenu {
            game: TEST,
            in_game: true,
            ..default()
        })
        .init_resource::<MenuUi>()
        .init_resource::<RegrabCursor>()
        .init_resource::<LoadingDetails>()
        .init_resource::<crate::client::fonts::UiFonts>()
        .init_resource::<Windows>()
        .init_resource::<widgets::FrameBacking>()
        .init_resource::<widgets::WorldPicture>()
        .add_systems(Update, (draw, world_behind_dialogs).chain())
        .add_systems(PostUpdate, bevy::render::camera::camera_system);
    let window = Window {
        resolution: WindowResolution::new(2000, 1125).with_scale_factor_override(1.25),
        ..default()
    };
    let logical = Vec2::new(window.width(), window.height());
    assert_eq!(logical, Vec2::new(1600.0, 900.0));
    app.world_mut().spawn((window, PrimaryWindow));
    let camera = app
        .world_mut()
        .spawn((
            Camera::default(),
            RenderTarget::Window(bevy::window::WindowRef::Primary),
            Projection::default(),
            crate::client::FirstPersonCamera,
        ))
        .id();
    app.update();
    let size = |app: &App| app.world().get::<Camera>(camera).unwrap().logical_viewport_size();
    assert_eq!(size(&app), Some(logical));
    for page in [
        Page::Settings,
        Page::KeyboardAdvanced,
        Page::VideoAdvanced,
        Page::Gamma,
        Page::MultiplayerAdvanced,
        Page::Extras,
        Page::Debug,
        Page::NewGame,
        Page::Bots,
        Page::Team,
    ] {
        open_menu(app.world_mut(), page);
        app.update();
        app.update();
        let on_picture = matches!(app.world().get::<RenderTarget>(camera), Some(RenderTarget::Image(_)));
        assert!(on_picture, "{page:?}: the world drawn into a picture");
        // The picture's pixels: physical ones.
        assert_eq!(size(&app), Some(Vec2::new(2000.0, 1125.0)), "{page:?} open");
        app.world_mut().resource_mut::<GameMenu>().open = false;
        app.update();
        app.update();
        assert!(matches!(app.world().get::<RenderTarget>(camera), Some(RenderTarget::Window(_))));
        assert_eq!(size(&app), Some(logical), "{page:?} closed: the window's size again");
        let cam = app.world().get::<Camera>(camera).unwrap();
        assert_eq!(
            crate::client::hud::crosshair_space(None, cam).map(|s| s / 2.0),
            Some(logical / 2.0),
            "{page:?}: the crosshair at the window's centre"
        );
    }
}

/// The app's greybox in `disconnect_and_the_greybox_on_the_world`.
#[derive(Component)]
struct GreyboxPart;

/// `disconnect` and `map greybox` on a world (headless): the map gives
/// way to the greybox, the menu follows.
#[test]
fn disconnect_and_the_greybox_on_the_world() {
    let mut app = App::new();
    app.add_plugins(crate::console::ConsolePlugin)
        .insert_resource(GameMenu {
            game: TEST,
            in_game: true,
            ..default()
        })
        .init_resource::<RegrabCursor>()
        .insert_resource(crate::map::LoadedMapName("cs_source:de_dust2".into()))
        // The app's greybox (`client::AppHooks`): a marker here.
        .insert_resource(crate::client::AppHooks {
            build: "test",
            load_map: |_, _| Err("no maps here".into()),
            load_map_file: |_, _, _| Err("no maps here".into()),
            map_file_bytes: |_| None,
            unload_greybox: |_| {},
            respawn_greybox: |w| {
                w.spawn(GreyboxPart);
            },
        });
    let w = app.world_mut();
    crate::client::console::load_greybox(w);
    left_game(w);
    let parts = w
        .query_filtered::<(), With<GreyboxPart>>()
        .iter(w)
        .count();
    assert!(parts > 0, "the greybox is back behind the main menu");
    assert_eq!(w.resource::<crate::map::LoadedMapName>().0, "greybox");
    let m = w.resource::<GameMenu>();
    assert!(m.open && !m.in_game && m.page == Page::Main);
    assert!(!shown(m).contains(&MainItem::Resume));
    // The greybox from the main menu: in a game, the menu closed and
    // the mouse grabbed again.
    w.resource_mut::<GameMenu>().loading = Some("greybox".into());
    entered_game(w);
    let m = w.resource::<GameMenu>();
    assert!(m.in_game && !m.open && m.loading.is_none());
    assert!(w.resource::<RegrabCursor>().0);
    // A failed load gives the main menu back.
    let mut m = w.resource_mut::<GameMenu>();
    m.leave_game();
    m.loading = Some("nope".into());
    map_load_failed(w);
    assert!(w.resource::<GameMenu>().loading.is_none());
}

#[test]
fn disconnect_returns_to_the_main_menu() {
    let mut m = menu();
    let o = click(&mut m, MainItem::Disconnect);
    assert_eq!(o.lines, ["disconnect"]);
    assert!(!o.close && m.open && !m.in_game && m.page == Page::Main);
    assert!(!shown(&m).contains(&MainItem::Resume) && !shown(&m).contains(&MainItem::Disconnect));
    assert!(m.entries()[m.focus].enabled);
    // Esc no longer leaves the menu.
    press(&mut m, &[Input::Close]);
    assert!(m.open);
}

#[test]
fn escape_and_resume_close_with_no_lines() {
    let mut m = menu();
    let o = m.handle(Input::Close);
    assert!(o.close && o.lines.is_empty() && !m.open);
    let mut m = menu();
    let o = m.handle(Input::Activate);
    assert!(o.close && o.lines.is_empty() && !m.open);
    // A closed menu ignores input.
    assert_eq!(m.handle(Input::Activate), Outcome::default());
}

#[test]
fn keys_move_through_pages_and_back() {
    let mut m = menu();
    m.go_to(Page::Bots);
    // The info line can't be focused.
    assert_eq!(m.focus, 1);
    press(&mut m, &[Input::Up]);
    assert_eq!(m.focus, 4, "wraps past the info line to Close");
    press(&mut m, &[Input::Focus(1)]);
    assert_eq!(m.focus, 1, "Tab wraps too");
    press(&mut m, &[Input::Back]);
    assert_eq!((m.page, m.focus), (Page::Main, 0), "no entry of its own: back on the first");
    // Up wraps to Quit.
    press(&mut m, &[Input::Up]);
    assert_eq!(m.entries()[m.focus].item, MainItem::Quit);
    assert_eq!(press(&mut m, &[Input::Activate]).lines, ["quit"]);
}

#[test]
fn bots_and_team_pages_run_their_commands() {
    let mut m = menu();
    m.go_to(Page::Bots);
    let o = press(
        &mut m,
        &[
            Input::Click(Target::Row(1), 0),
            Input::Click(Target::Row(2), 0),
            Input::Click(Target::Row(3), 0),
        ],
    );
    assert_eq!(o.lines, ["bot_add 1", "bot_add 2", "bot_kick"]);
    assert!(!o.close && m.open, "adding bots keeps the menu open");
    // Info rows ignore clicks.
    assert!(press(&mut m, &[Input::Click(Target::Row(0), 0)]).lines.is_empty());

    m.go_to(Page::Team);
    let o = press(&mut m, &[Input::Down, Input::Activate]);
    assert_eq!(o.lines, ["jointeam 3"]);
    assert!(o.close && !m.open, "joining a team goes back to playing");

    // Auto-assign joins the smaller team.
    let mut m = menu();
    m.set_counts([0, 0], [3, 1]);
    m.go_to(Page::Team);
    assert_eq!(press(&mut m, &[Input::Click(Target::Row(2), 0)]).lines, ["jointeam 3"]);

    // Spectate, as CS:S's team menu; greyed when the server refuses
    // spectators (`mp_allowspectators 0`).
    let mut m = menu();
    m.go_to(Page::Team);
    assert_eq!(press(&mut m, &[Input::Click(Target::Row(3), 0)]).lines, ["jointeam 1"]);
    let mut m = menu();
    m.no_spectators = true;
    m.go_to(Page::Team);
    assert!(press(&mut m, &[Input::Click(Target::Row(3), 0)]).lines.is_empty(), "greyed");
}

/// Create Server driven as CS:S's: the map from its drop-down, bots
/// included, how many typed, a difficulty button, the Bot page's team,
/// the Game page's options; Start sets what changed, loads the map,
/// then adds the bots.
#[test]
fn create_server_by_its_controls() {
    let mut m = menu();
    let ui = GameUi {
        server_settings: vec![
            ServerSetting {
                cvar: "hostname".into(),
                label: "Server Name".into(),
                kind: ServerSettingKind::Text,
                default: "Counter-Strike Source".into(),
            },
            ServerSetting {
                cvar: "mp_friendlyfire".into(),
                label: "Friendly fire".into(),
                kind: ServerSettingKind::Bool,
                default: "0".into(),
            },
            ServerSetting {
                cvar: "mp_timelimit".into(),
                label: "Time limit".into(),
                kind: ServerSettingKind::Number { min: Some(0.0), max: None },
                default: "20".into(),
            },
        ],
        ..default()
    };
    m.set_ui(Some(Arc::new(ui)));
    m.open(Page::Main, vec!["cs_office".into(), "de_dust2".into(), "de_nuke".into()], Some("de_dust2"), get);
    click(&mut m, MainItem::NewGame);
    // The map: a click opens its list on the map now, Down and Enter
    // pick the next.
    let map = row_with(&m, Field::Map);
    press(&mut m, &[Input::Click(Target::Row(map), 0)]);
    // CS:S's `< Random Map >` first.
    assert_eq!(m.combo.map(|c| (c.owner, c.highlight, c.len)), Some((map, 2, 4)));
    press(&mut m, &[Input::Down, Input::Activate]);
    assert_eq!((m.new_game.map, m.combo), (2, None));
    // Include bots: their count and difficulty show.
    assert!(m.rows().iter().all(|r| r.field() != Some(Field::BotCount)));
    click_on(&mut m, |m| row_with(m, Field::BotsOn));
    let count = row_with(&m, Field::BotCount);
    // Tab to the count (its text selected), type over it; only digits.
    press(&mut m, &[Input::Focus(1)]);
    assert_eq!(m.focus, count);
    press(&mut m, &[Input::Type("3x".into())]);
    assert_eq!(m.new_game.bot_count, "3");
    press(&mut m, &[Input::Type("45".into())]);
    assert_eq!(m.new_game.bot_count, "3", "two digits at most");
    click_on(&mut m, |m| row_with(m, Field::Difficulty(2)));
    assert_eq!(m.new_game.difficulty, 2);
    assert_eq!(control(&m, row_with(&m, Field::Difficulty(2))), Control::Radio(true));
    // The Bot page: counter-terrorists only.
    press(&mut m, &[Input::Click(Target::Tab(2), 0)]);
    assert_eq!(m.create_tab, 2);
    let team = row_with(&m, Field::BotTeam);
    press(&mut m, &[Input::Click(Target::Row(team), 0), Input::Click(Target::ComboItem(2), 0)]);
    assert_eq!(m.new_game.bot_team, 2);
    // The Game page: friendly fire on (Ctrl+Tab back to it).
    press(&mut m, &[Input::NextTab(-1)]);
    assert_eq!(m.create_tab, 1);
    let rows = m.rows();
    let ff = rows
        .iter()
        .position(|r| matches!(r, Row::Control { label, .. } if label == "Friendly fire"))
        .unwrap();
    assert!(!rows.iter().any(|r| matches!(r, Row::Control { label, .. } if label == "Time limit")), "mashup lacks it");
    let o = press(&mut m, &[Input::Click(Target::Row(ff), 0)]);
    assert!(o.lines.is_empty(), "nothing runs before Start: {:?}", o.lines);
    let o = click_on(&mut m, |m| button(m, Action::Start));
    assert_eq!(
        o.lines,
        [
            "bot_kick",
            "chimera_rounds 1",
            "mp_friendlyfire \"1\"",
            "maxplayers 32",
            "bot_reaction 0.2",
            "bot_aim_error 1.5",
            "bot_turn_rate 540",
            "map de_nuke",
        ]
    );
    assert_eq!(o.after_load, ["bot_add 2", "bot_add 2", "bot_add 2"]);
    assert!(o.close && !m.open);
}

#[test]
fn create_servers_combo_boxes_take_the_wheel_letters_and_escape() {
    let mut m = menu();
    click(&mut m, MainItem::NewGame);
    let map = row_with(&m, Field::Map);
    // Closed and focused: Up/Down and the wheel step it, no wrapping.
    press(&mut m, &[Input::FocusRow(map), Input::Down, Input::Down, Input::Down]);
    assert_eq!(m.new_game.map, 2);
    press(&mut m, &[Input::Wheel(map, 1)]);
    assert_eq!(m.new_game.map, 1);
    // A letter jumps.
    press(&mut m, &[Input::Char('c')]);
    assert_eq!(m.new_game.map, 0);
    // Open: hover highlights, Esc closes without a pick, a click outside
    // too.
    press(&mut m, &[Input::Space, Input::Hover(Target::ComboItem(2))]);
    assert_eq!(m.combo.map(|c| c.highlight), Some(2));
    press(&mut m, &[Input::Close]);
    assert_eq!((m.combo, m.new_game.map, m.page), (None, 0, Page::NewGame), "Esc closed only the list");
    press(&mut m, &[Input::Space, Input::Click(Target::Outside, 0)]);
    assert_eq!((m.combo, m.new_game.map), (None, 0));
    // Its wheel scrolls the list, not the value.
    press(&mut m, &[Input::Space, Input::Scroll(1)]);
    assert_eq!(m.new_game.map, 0);
    press(&mut m, &[Input::Char('d'), Input::Activate]);
    assert_eq!(m.new_game.map, 1, "a letter in the open list, Enter picks");
}

#[test]
fn no_maps_no_start() {
    let mut m = GameMenu::for_game(&TEST_GAME);
    m.open(Page::NewGame, Vec::new(), None, |_| None);
    let start = button(&m, Action::Start);
    let o = press(&mut m, &[Input::Click(Target::Row(start), 0)]);
    assert!(o.lines.is_empty() && m.open);
    // The empty map box doesn't open a list.
    press(&mut m, &[Input::Click(Target::Row(0), 0)]);
    assert_eq!((m.page, m.combo), (Page::NewGame, None));
}

#[test]
fn options_ok_cancel_and_apply() {
    let mut m = menu();
    click(&mut m, MainItem::Options);
    assert_eq!((m.page, m.tab), (Page::Settings, Tab::Keyboard));
    let apply = button(&m, Action::Apply);
    assert!(matches!(m.rows()[apply], Row::Button { enabled: false, .. }), "nothing to apply yet");
    press(&mut m, &[Input::NextTab(1)]);
    assert_eq!(m.tab, Tab::Mouse);
    // Reverse mouse, then the sensitivity slider pressed at its end and
    // dragged: once per change.
    let reverse = row_of(&m, "m_pitch");
    let o = press(&mut m, &[Input::Click(Target::Row(reverse), 0)]);
    assert_eq!(o.lines, ["m_pitch -0.022"]);
    let slider = row_of(&m, "sensitivity");
    let o = press(&mut m, &[Input::Slide(slider, 1.0), Input::Slide(slider, 1.0), Input::Slide(slider, 0.0)]);
    assert_eq!(o.lines, ["sensitivity 20.0", "sensitivity 0.1"]);
    // Focused, Right steps it.
    assert_eq!(press(&mut m, &[Input::Right]).lines, ["sensitivity 0.2"]);
    // Apply keeps them; Cancel puts back only what changed after.
    let apply = button(&m, Action::Apply);
    assert!(matches!(m.rows()[apply], Row::Button { enabled: true, .. }));
    press(&mut m, &[Input::Click(Target::Row(apply), 0)]);
    assert!(matches!(m.rows()[apply], Row::Button { enabled: false, .. }), "applied");
    assert_eq!(press(&mut m, &[Input::FocusRow(slider), Input::Right]).lines, ["sensitivity 0.3"]);
    let o = click_on(&mut m, |m| button(m, Action::Cancel));
    assert_eq!(o.lines, ["sensitivity 0.2"]);
    assert_eq!(m.page, Page::Main);
    // OK keeps; Esc is Cancel.
    click(&mut m, MainItem::Options);
    press(&mut m, &[Input::Click(Target::Tab(1), 0)]);
    let reverse = row_of(&m, "m_pitch");
    press(&mut m, &[Input::Click(Target::Row(reverse), 0)]);
    assert!(click_on(&mut m, |m| button(m, Action::Ok)).lines.is_empty());
    click(&mut m, MainItem::Options);
    press(&mut m, &[Input::Click(Target::Tab(1), 0), Input::Click(Target::Row(reverse), 0)]);
    assert_eq!(press(&mut m, &[Input::Close]).lines, ["m_pitch 0.022"], "back to what OK kept");
    // The close box is Cancel too.
    click(&mut m, MainItem::Options);
    press(&mut m, &[Input::Click(Target::Tab(1), 0), Input::Click(Target::Row(reverse), 0)]);
    assert_eq!(press(&mut m, &[Input::Click(Target::Close, 0)]).lines, ["m_pitch 0.022"]);
    assert!(m.open && m.in_game, "the menu stays");
}

#[test]
fn video_options_and_its_advanced_dialog() {
    let mut m = menu();
    m.resolutions = vec!["1920x1080".into(), "1280x720".into()];
    click(&mut m, MainItem::Options);
    press(&mut m, &[Input::Click(Target::Tab(3), 0)]);
    assert_eq!(m.tab, Tab::Video);
    // The resolution's drop-down lists the window sizes.
    let res = row_of(&m, "chimera_resolution");
    press(&mut m, &[Input::Click(Target::Row(res), 0)]);
    assert_eq!(m.combo.map(|c| c.len), Some(2));
    let o = press(&mut m, &[Input::Click(Target::ComboItem(1), 0)]);
    assert_eq!(o.lines, ["chimera_resolution 1280x720"]);
    // Advanced...: vsync as a drop-down, antialiasing; Cancel puts them
    // back, the options stay open.
    let adv = button(&m, Action::VideoAdvanced);
    press(&mut m, &[Input::Click(Target::Row(adv), 0)]);
    assert_eq!(m.page, Page::VideoAdvanced);
    let aa = row_of(&m, "mat_antialias");
    assert!(matches!(control(&m, aa), Control::Combo { selected: Some(2), .. }));
    let o = press(&mut m, &[Input::Click(Target::Row(aa), 0), Input::Up, Input::Up, Input::Activate]);
    assert_eq!(o.lines, ["mat_antialias 0"]);
    let o = click_on(&mut m, |m| button(m, Action::Cancel));
    assert_eq!(o.lines, ["mat_antialias 4"]);
    assert_eq!((m.page, m.focus), (Page::Settings, adv));
    // Show FPS isn't CS:S's: in our own dialog.
    assert!(m.rows().iter().all(|r| r.field() != Some(Field::Setting(setting("cl_showfps")))));
    m.go_to(Page::Extras);
    assert!(m.rows().iter().any(|r| r.field() == Some(Field::Setting(setting("cl_showfps")))));
}

/// Chimera Debug Options (`menu debug`): the overlay cvars as check
/// boxes, set at once like the other options, Cancel puts them back.
#[test]
fn the_debug_options_page_lists_the_overlays_as_check_boxes() {
    let mut m = menu();
    m.go_to(Page::Debug);
    // Only the cvars this build has (the test's table has two).
    let hit = row_of(&m, "chimera_drawhitboxes");
    let snd = row_of(&m, "snd_show");
    assert!(matches!(control(&m, hit), Control::Check(false)), "{:?}", control(&m, hit));
    assert!(matches!(control(&m, snd), Control::Check(true)));
    assert!(m.rows().iter().all(|r| r.field() != Some(Field::Setting(setting("cl_showfps")))));
    let o = press(&mut m, &[Input::Click(Target::Row(hit), 0)]);
    assert_eq!(o.lines, ["chimera_drawhitboxes 1"]);
    let o = click_on(&mut m, |m| button(m, Action::Cancel));
    assert_eq!(o.lines, ["chimera_drawhitboxes 0"]);
}

#[test]
fn the_sensitivity_text_entry_beside_its_slider() {
    // With the game's mouse layout: the slider and the text entry.
    let mut layout = UiLayout::default();
    for (name, kind) in [("ReverseMouse", "CCvarNegateCheckButton"), ("Slider", "CCvarSlider"), ("SensitivityLabel", "TextEntry")] {
        layout
            .controls
            .push(UiControl::new(name, UiKind::Other(kind.into()), 0.0, 0.0, 100.0, 24.0));
    }
    let ui = GameUi {
        options: HashMap::from([("mouse".to_string(), layout)]),
        ..default()
    };
    let mut m = menu();
    m.set_ui(Some(Arc::new(ui)));
    click(&mut m, MainItem::Options);
    press(&mut m, &[Input::Click(Target::Tab(1), 0)]);
    let entry = row_with(&m, Field::SettingText(setting("sensitivity")));
    assert!(matches!(control(&m, entry), Control::Text { text, .. } if text == "3.0"));
    // A click places the caret; the text typed sets the cvar once it
    // reads as a number in range.
    press(&mut m, &[Input::Click(Target::Row(entry), 0), Input::Caret(entry, 3), Input::Edit(Edit::Home(true))]);
    assert_eq!(m.caret, Caret { at: 0, anchor: 3 });
    let o = press(&mut m, &[Input::Copy]);
    assert_eq!(o.clipboard.as_deref(), Some("3.0"));
    let o = press(&mut m, &[Input::Type("2".into()), Input::Type("5".into())]);
    assert_eq!(o.lines, ["sensitivity 2"], "25 is out of range");
    assert!(matches!(control(&m, entry), Control::Text { text, .. } if text == "25"), "as typed");
    assert_eq!(m.values[setting("sensitivity")].as_deref(), Some("2"));
    // Without the layout no entry: the slider shows the number.
    let mut m = menu();
    click(&mut m, MainItem::Options);
    press(&mut m, &[Input::Click(Target::Tab(1), 0)]);
    assert!(m.rows().iter().all(|r| !matches!(r.field(), Some(Field::SettingText(_)))));
}

fn keyboard() -> GameMenu {
    let mut m = menu();
    let mut b = BTreeMap::new();
    let mut defaults = binds::BindDefaults::default();
    defaults.use_game(&TEST_INFO);
    let defaults = defaults.default_binds();
    binds::bind_defaults(&mut b, true, &defaults);
    let known = binds::POLLED.iter().map(|c| c.to_string()).chain(["bugreport".to_string()]);
    m.set_binds(b, known.collect(), defaults);
    click(&mut m, MainItem::Options);
    m
}

fn bind_row(m: &GameMenu, command: &str) -> usize {
    m.rows()
        .iter()
        .position(|r| matches!(r, Row::Bind { command: c, .. } if c == command))
        .unwrap()
}

#[test]
fn the_keyboard_tab_lists_actions_with_their_keys() {
    let m = keyboard();
    let rows = m.rows();
    assert!(matches!(&rows[0], Row::Heading(t) if t == "Movement"));
    assert!(matches!(&rows[bind_row(&m, "+jump")], Row::Bind { keys, known: true, .. } if keys == "SPACE"));
    // CS:S's defaults: the wheel changes weapons, F1 autobuys.
    assert!(matches!(&rows[bind_row(&m, "invprev")], Row::Bind { keys, .. } if keys == "MWHEELUP"));
    assert!(matches!(&rows[bind_row(&m, "autobuy")], Row::Bind { keys, .. } if keys == "F1"));
    assert!(matches!(&rows[bind_row(&m, "debugui")], Row::Bind { keys, .. } if keys == "F8"));
    assert_eq!(m.focus, 1, "the first action, past the heading");
    assert_eq!(m.list(), (rows.len() - 7, KEY_ROWS));
}

/// Keyboard > Advanced...: CS:S's two check boxes (fast weapon switch,
/// developer console) in its order; ticking applies at once, Cancel
/// (or Esc) puts them back, OK keeps them.
#[test]
fn the_keyboard_tabs_advanced_dialog() {
    let mut m = keyboard();
    let at = button(&m, Action::Advanced);
    press(&mut m, &[Input::Click(Target::Row(at), 0)]);
    assert_eq!(m.page, Page::KeyboardAdvanced);
    let rows = m.rows();
    let labels: Vec<String> = rows.iter().map(label_of).collect();
    assert_eq!(labels, ["Fast weapon switch", "Enable developer console", "OK", "Cancel"]);
    // Fast switch is on by default (ours).
    assert!(matches!(&rows[0], Row::Control { control: Control::Check(true), .. }), "{:?}", rows[0]);
    assert_eq!(m.focus, 0);
    let o = press(&mut m, &[Input::Space]);
    assert_eq!(o.lines, ["hud_fastswitch 0"]);
    let o = press(&mut m, &[Input::Click(Target::Row(1), 0)]);
    assert_eq!(o.lines, ["con_enable 0"]);
    // Cancel: both back.
    let o = press(&mut m, &[Input::Click(Target::Row(3), 0)]);
    assert_eq!(o.lines, ["hud_fastswitch 1", "con_enable 1"]);
    assert_eq!((m.page, m.tab, m.focus), (Page::Settings, Tab::Keyboard, at));
    // Enter on the check box: OK, the default button, keeps it.
    press(&mut m, &[Input::Activate, Input::Space]);
    let o = press(&mut m, &[Input::Activate]);
    assert!(o.lines.is_empty());
    assert_eq!(m.page, Page::Settings);
    assert_eq!(m.values[setting_index("hud_fastswitch").unwrap()].as_deref(), Some("0"));
    // Esc on the dialog cancels it, leaving the options open.
    press(&mut m, &[Input::Activate, Input::Focus(1), Input::Space]);
    let o = press(&mut m, &[Input::Close]);
    assert_eq!(o.lines, ["con_enable 1"]);
    assert!(m.open && m.page == Page::Settings);
}

#[test]
fn rebinding_through_the_keyboard_tab() {
    let mut m = keyboard();
    let jump = bind_row(&m, "+jump");
    // One click selects, the next waits for a key.
    press(&mut m, &[Input::Click(Target::Row(jump), 0)]);
    assert_eq!((m.key_row, m.capture.as_deref()), (Some(jump), None));
    press(&mut m, &[Input::Click(Target::Row(jump), 0)]);
    assert_eq!(m.capture.as_deref(), Some("+jump"));
    // Keys while waiting go to the bind, not the menu.
    let o = press(&mut m, &[Input::Down, Input::BindKey("f")]);
    assert_eq!(o.lines, ["unbind space; bind f +jump"]);
    assert_eq!(m.capture, None);
    assert!(matches!(&m.rows()[jump], Row::Bind { keys, .. } if keys == "F"));
    // Esc while waiting gives up, leaving the menu open.
    press(&mut m, &[Input::Activate]);
    assert!(m.capture.is_some());
    let o = press(&mut m, &[Input::Close]);
    assert!(o.lines.is_empty() && m.open && m.capture.is_none());
    // Edit key and Clear key act on the selected action.
    let duck = bind_row(&m, "+duck");
    press(&mut m, &[Input::Click(Target::Row(duck), 0)]);
    let edit = button(&m, Action::EditKey);
    let clear = edit + 1;
    let o = press(&mut m, &[Input::Click(Target::Row(clear), 0)]);
    assert_eq!(o.lines, ["unbind ctrl"]);
    press(&mut m, &[Input::Click(Target::Row(edit), 0)]);
    assert_eq!(m.capture.as_deref(), Some("+duck"));
    let o = press(&mut m, &[Input::BindKey("mouse4")]);
    assert_eq!(o.lines, ["bind mouse4 +duck"]);
    // Use Defaults puts every bind back.
    let o = press(&mut m, &[Input::Click(Target::Row(edit - 1), 0)]);
    assert_eq!(o.lines, ["binddefaults"]);
    assert_eq!(m.binds.get("space").map(String::as_str), Some("+jump"));
    assert_eq!(m.binds.get("f1").map(String::as_str), Some("autobuy"), "CS:S's");
    assert_eq!(m.binds.get("mwheelup").map(String::as_str), Some("invprev"));
    assert_eq!(m.binds.get("mouse4"), None);
}

#[test]
fn the_keyboard_list_scrolls_to_the_focus() {
    let mut m = keyboard();
    for _ in 0..20 {
        press(&mut m, &[Input::Down]);
    }
    let (len, shown) = m.list();
    assert!(m.focus < len);
    assert!(m.scroll <= m.focus && m.focus < m.scroll + shown, "{} in {}+{shown}", m.focus, m.scroll);
}

/// The wheel, the scroll bar's arrows, its track and its thumb move the
/// view free of the selection (VGUI's lists): past the selected row, and
/// the pointer over rows drawn again (hovers) leaves it there; moving the
/// selection brings the view back to it.
#[test]
fn the_keyboard_list_scrolls_past_the_selection() {
    let mut m = keyboard();
    let selected = m.key_row;
    assert_eq!(selected, Some(1));
    let (len, shown) = m.list();
    press(&mut m, &[Input::Scroll(9)]);
    assert_eq!(m.scroll, 9, "the wheel: three notches");
    press(&mut m, &[Input::Hover(Target::Row(12)), Input::Click(Target::Scroll, 1), Input::Hover(Target::Scroll)]);
    assert_eq!(m.scroll, 10, "an arrow: a row; hovers keep it");
    press(&mut m, &[Input::Click(Target::Scroll, shown as i32)]);
    assert_eq!(m.scroll, 10 + shown, "the track: a page");
    // The thumb dragged to the bottom (`ScrollDrag` as a step).
    press(&mut m, &[Input::Scroll(1000), Input::Hover(Target::Row(len - 1))]);
    assert_eq!(m.scroll, len - shown, "to the last row, not past it");
    assert_eq!((m.key_row, m.focus), (selected, 1), "the selection stays");
    // A row clicked down there selects it and the view stays.
    let row = (len - 3..len).find(|&i| matches!(m.rows()[i], Row::Bind { .. })).unwrap();
    press(&mut m, &[Input::Click(Target::Row(row), 0)]);
    assert_eq!((m.key_row, m.scroll), (Some(row), len - shown));
    // The selection moved by keys: the view follows it.
    press(&mut m, &[Input::Scroll(-1000)]);
    assert_eq!(m.scroll, 0);
    press(&mut m, &[Input::Up]);
    assert!(m.scroll <= m.focus && m.focus < m.scroll + shown, "{} in {}+{shown}", m.focus, m.scroll);
}

/// A click on an open drop-down's box closes it (VGUI's `ComboBox`
/// toggles), as a click anywhere outside its list does; the next opens it.
#[test]
fn a_click_on_an_open_drop_downs_box_closes_it() {
    let mut m = menu();
    m.resolutions = vec!["1920x1080".into(), "1280x720".into()];
    click(&mut m, MainItem::Options);
    press(&mut m, &[Input::Click(Target::Tab(3), 0)]);
    let mode = row_of(&m, "chimera_resolution");
    press(&mut m, &[Input::Click(Target::Row(mode), 0)]);
    assert_eq!(m.combo.map(|c| c.owner), Some(mode));
    let o = press(&mut m, &[Input::Click(Target::Row(mode), 0)]);
    assert_eq!((m.combo, o.lines.len()), (None, 0), "closed, nothing picked");
    press(&mut m, &[Input::Click(Target::Row(mode), 0)]);
    assert!(m.combo.is_some(), "open again");
    press(&mut m, &[Input::Click(Target::Outside, 0)]);
    assert!(m.combo.is_none());
}

/// The video tab shows the window as it is: its size, picked in the
/// resolution list (added to the monitor's modes when not one of them),
/// never "As started"; its real display mode. Each mode picked sets
/// exactly that one; Cancel puts back the one it was.
#[test]
fn the_video_tab_shows_the_window_as_it_is() {
    let mut values: HashMap<String, String> =
        CVARS.iter().map(|(k, v)| (k.to_string(), v.to_string())).chain([("chimera_fullscreen".into(), "0".into())]).collect();
    let monitor = [(UVec2::new(1920, 1080), vec![UVec2::new(1920, 1080), UVec2::new(1280, 720)])];
    let window = UVec2::new(1600, 900);
    window_values(&mut values, Some(window), Some(2));
    let mut m = GameMenu {
        game: TEST,
        in_game: true,
        resolutions: crate::client::video::resolutions(&monitor, Some(window)),
        ..default()
    };
    m.open(Page::Main, vec!["de_dust2".into()], None, |n| values.get(n).cloned());
    click(&mut m, MainItem::Options);
    press(&mut m, &[Input::Click(Target::Tab(3), 0)]);
    let res = row_of(&m, "chimera_resolution");
    let Control::Combo { entries, selected, text } = control(&m, res) else { panic!() };
    assert_eq!(entries, ["1920x1080", "1600x900", "1280x720"]);
    assert_eq!((selected, text.as_str()), (Some(1), "1600x900"));
    let mode = row_of(&m, "chimera_fullscreen");
    let Control::Combo { entries, selected, text } = control(&m, mode) else { panic!() };
    assert_eq!(entries, ["Fullscreen", "Windowed", "Borderless fullscreen"]);
    assert_eq!((selected, text.as_str()), (Some(2), "Borderless fullscreen"));
    // Each entry: exactly its mode.
    for (k, value) in [(1, "0"), (0, "1"), (2, "2")] {
        let o = press(&mut m, &[Input::Click(Target::Row(mode), 0), Input::Click(Target::ComboItem(k), 0)]);
        assert_eq!(o.lines, [format!("chimera_fullscreen {value}")]);
    }
    press(&mut m, &[Input::Click(Target::Row(mode), 0), Input::Click(Target::ComboItem(1), 0)]);
    let o = click_on(&mut m, |m| button(m, Action::Cancel));
    assert_eq!(o.lines, ["chimera_fullscreen 2"], "back to the mode it was");
    // No window (headless): the cvar as it is.
    let mut values = HashMap::from([("chimera_resolution".to_string(), String::new())]);
    window_values(&mut values, None, None);
    assert_eq!(values["chimera_resolution"], "");
}

#[test]
fn the_games_menu_entries_and_none_of_ours() {
    let item = |label: &str, command: &str| GameUiItem {
        label: label.into(),
        command: command.into(),
        in_game_only: false,
    };
    let ui = GameUi {
        menu: vec![
            item("RESUME GAME", "ResumeGame"),
            item("DISCONNECT", "Disconnect"),
            item("", ""),
            item("FIND SERVERS", "OpenServerBrowser"),
            item("CREATE SERVER", "OpenCreateMultiplayerGameDialog"),
            item("REPORT BUG", "engine bug"),
            item("OPTIONS", "OpenOptionsDialog"),
            item("QUIT", "Quit"),
        ],
        strings: HashMap::from([("gameui_gamemenu_newgame".to_string(), "NEW GAME".to_string())]),
        ..default()
    };
    let e = main_entries(Some(&ui));
    let items: Vec<MainItem> = e.iter().map(|e| e.item).collect();
    use MainItem::*;
    // The player list the file lacks: under Disconnect, as CS:S's.
    assert_eq!(
        items,
        [
            Resume,
            Disconnect,
            PlayerList,
            FindServers,
            NewGame,
            BugReport,
            Options,
            Quit
        ]
    );
    assert_eq!(e[4].label, "CREATE SERVER", "the game's words");
    assert!(e[3].gap && !e[3].gap_in_game_only, "the file's blank entry");
    assert!(e[0].in_game_only && e[1].in_game_only && e[2].in_game_only, "resume, disconnect, the player list only in a game");
    assert_eq!(e[5].label, "REPORT BUG");
    // Without the file: CS:S's entries, none of ours.
    assert_eq!(main_entries(None).len(), MAIN.len());
    let mut m = menu();
    m.set_ui(Some(Arc::new(ui)));
    press(&mut m, &[Input::Click(Target::Main(6), 0)]);
    assert_eq!(m.page, Page::Settings);
    press(&mut m, &[Input::Back]);
    assert_eq!(m.focus, 6, "back on its entry");
}

#[test]
fn the_install_menu_file_hides_its_gap_out_of_a_game() {
    let item = |label: &str, command: &str, in_game_only: bool| GameUiItem {
        label: label.into(),
        command: command.into(),
        in_game_only,
    };
    // As CS:S's GameMenu.res: the blank entry is only for a game.
    let ui = GameUi {
        menu: vec![
            item("RESUME GAME", "ResumeGame", true),
            item("DISCONNECT", "Disconnect", true),
            item("PLAYERS", "OpenPlayerListDialog", true),
            item("", "", true),
            item("FIND SERVERS", "OpenServerBrowser", false),
            item("CREATE SERVER", "OpenCreateMultiplayerGameDialog", false),
            item("ACHIEVEMENTS", "OpenCSAchievementsDialog", false),
            item("QUIT", "Quit", false),
        ],
        ..default()
    };
    let mut m = main_menu();
    m.set_ui(Some(Arc::new(ui)));
    let e = m.entries();
    assert_eq!(e[0].item, MainItem::FindServers);
    assert!(!e[0].gap);
    m.in_game = true;
    let e = m.entries();
    assert_eq!(e[2].item, MainItem::PlayerList);
    assert_eq!(e[3].item, MainItem::FindServers);
    assert!(e[3].gap);
    assert!(!e.iter().any(|e| e.label == "ACHIEVEMENTS"), "what Chimera lacks is left out");
}

#[test]
fn hover_moves_focus_only_on_the_main_page() {
    let mut m = menu();
    let options = at(&m, MainItem::Options);
    press(&mut m, &[Input::Hover(Target::Main(options))]);
    assert_eq!(m.focus, options);
    press(&mut m, &[Input::Activate, Input::NextTab(1)]);
    assert_eq!(m.page, Page::Settings);
    let focus = m.focus;
    // Neither the left-hand list nor a hover takes the dialog's focus.
    press(&mut m, &[Input::Hover(Target::Main(1)), Input::Hover(Target::Row(focus + 1))]);
    assert_eq!(m.focus, focus);
}

#[test]
fn escape_opens_and_closes_and_the_mouse_follows() {
    use bevy::window::CursorGrabMode;
    let mut app = App::new();
    app.add_plugins(crate::console::ConsolePlugin)
        .init_resource::<crate::client::console::ConsoleUi>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .insert_resource(GameMenu {
            game: TEST,
            in_game: true,
            ..default()
        })
        .init_resource::<AfterLoad>()
        .init_resource::<RegrabCursor>()
        .add_systems(
            Update,
            (
                keys.before(crate::client::console::toggle),
                crate::client::console::toggle,
                cursor,
            )
                .chain(),
        );
    app.world_mut()
        .spawn((crate::client::console::ConsoleRoot, Visibility::Hidden));
    let window = app
        .world_mut()
        .spawn(CursorOptions {
            grab_mode: CursorGrabMode::Locked,
            ..default()
        })
        .id();
    let press = |app: &mut App, key: KeyCode| {
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
        app.update();
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release(key);
        keys.clear();
    };
    let grabbed = |app: &App| app.world().get::<CursorOptions>(window).unwrap().grab_mode != CursorGrabMode::None;
    let open = |app: &App| app.world().resource::<GameMenu>().open;

    press(&mut app, KeyCode::Escape);
    assert!(open(&app) && !grabbed(&app), "Esc opens the menu and frees the mouse");
    press(&mut app, KeyCode::Escape);
    assert!(!open(&app) && grabbed(&app), "Esc closes it and grabs the mouse");

    // Esc in the console only closes the console.
    press(&mut app, KeyCode::Backquote);
    press(&mut app, KeyCode::Escape);
    assert!(!open(&app) && !app.world().resource::<crate::client::console::ConsoleUi>().open);

    // Resume clicked: the mouse is grabbed once the button is up, so
    // the click doesn't fire.
    press(&mut app, KeyCode::Escape);
    assert!(open(&app));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    press(&mut app, KeyCode::Enter);
    assert!(!open(&app) && !grabbed(&app));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    app.update();
    assert!(grabbed(&app));
}

/// Rebinding in the open menu with real keys: the console's binds
/// change, and the new key then drives the action (headless).
#[test]
fn a_key_rebound_in_the_options_drives_the_action() {
    let mut app = App::new();
    app.add_plugins(crate::console::ConsolePlugin)
        .init_resource::<crate::client::console::ConsoleUi>()
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(GameMenu {
            game: TEST,
            in_game: true,
            ..default()
        })
        .init_resource::<AfterLoad>()
        .init_resource::<RegrabCursor>()
        .add_systems(Update, keys.before(crate::client::console::toggle))
        .add_systems(Update, crate::client::console::toggle);
    crate::client::binds::test_binds(&mut app, &TEST_INFO);
    binds::commands(&mut app);
    app.world_mut()
        .spawn((crate::client::console::ConsoleRoot, Visibility::Hidden));
    let tap = |app: &mut App, key: KeyCode| {
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
        app.update();
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release(key);
        keys.clear();
        app.update();
    };
    // Open on the options (the keyboard tab), as `menu keyboard` does.
    app.world_mut().commands().queue(|w: &mut World| {
        open_menu(w, Page::Settings);
    });
    app.update();
    assert!(app.world().resource::<GameMenu>().open);
    // To "Jump", Enter to wait for a key, then K.
    let jump = bind_row(app.world().resource::<GameMenu>(), "+jump");
    for _ in 1..jump {
        tap(&mut app, KeyCode::ArrowDown);
    }
    assert_eq!(app.world().resource::<GameMenu>().focus, jump);
    tap(&mut app, KeyCode::Enter);
    assert_eq!(app.world().resource::<GameMenu>().capture.as_deref(), Some("+jump"));
    tap(&mut app, KeyCode::KeyK);
    let console = app.world().resource::<Console>();
    assert_eq!(console.binds.get("k").map(String::as_str), Some("+jump"));
    assert_eq!(console.binds.get("space"), None);
    assert!(console.dirty, "saved to config.cfg on quit");
    // K now holds jump; Space doesn't.
    let mut keys = ButtonInput::<KeyCode>::default();
    let mouse = ButtonInput::<MouseButton>::default();
    keys.press(KeyCode::KeyK);
    assert!(binds::pressed(&console.binds, &keys, &mouse, "+jump"));
    keys.release(KeyCode::KeyK);
    keys.press(KeyCode::Space);
    assert!(!binds::pressed(&console.binds, &keys, &mouse, "+jump"));
}

#[test]
fn bug_report_closes_first() {
    let mut m = menu();
    let o = click(&mut m, MainItem::BugReport);
    assert_eq!(o.lines, ["bugreport"]);
    assert!(o.close && !m.open);
}

#[test]
fn menuinput_words() {
    let mut m = menu();
    click(&mut m, MainItem::NewGame);
    assert_eq!(menu_input(&m, &["focus".into(), "map".into()]), Ok(Input::FocusRow(0)));
    assert_eq!(menu_input(&m, &["open".into()]), Ok(Input::Space));
    assert_eq!(menu_input(&m, &["pick".into(), "2".into()]), Ok(Input::Click(Target::ComboItem(2), 0)));
    assert!(menu_input(&m, &["focus".into(), "nothing".into()]).is_err());
    assert!(m.describe().starts_with("NewGame, focus 0"));
}

#[test]
fn create_server_offers_a_random_map() {
    let mut m = main_menu();
    m.random_seed = 8;
    click(&mut m, MainItem::NewGame);
    let map = row_with(&m, Field::Map);
    match control(&m, map) {
        Control::Combo { entries, selected, .. } => {
            assert_eq!(entries, ["< Random Map >", "cs_office", "de_dust2", "de_nuke"]);
            assert_eq!(selected, Some(1), "the first map, not random, at first");
        }
        c => panic!("{c:?}"),
    }
    press(&mut m, &[Input::Click(Target::Row(map), 0), Input::Click(Target::ComboItem(0), 0)]);
    assert!(m.new_game.random);
    assert!(matches!(control(&m, map), Control::Combo { text, selected: Some(0), .. } if text == "< Random Map >"));
    // Start plays one of the maps, picked by the seed (8 % 3: de_nuke).
    let o = click_on(&mut m, |m| button(m, Action::Start));
    assert!(o.lines.contains(&"map de_nuke".to_string()), "{:?}", o.lines);
    assert_eq!(m.loading.as_deref(), Some("de_nuke"));
    // Another seed, another map; picking a map leaves random.
    let mut m = main_menu();
    m.random_seed = 3;
    m.new_game.random = true;
    assert_eq!(m.chosen_map(), Some(0));
    m.new_game.set_map_entry(3);
    assert_eq!((m.new_game.random, m.chosen_map()), (false, Some(2)));
}

#[test]
fn water_detail_sets_both_water_cvars() {
    let mut m = GameMenu {
        game: TEST,
        in_game: true,
        ..default()
    };
    let water = |n: &str| match n {
        "r_waterforceexpensive" => Some("1".to_string()),
        "r_waterforcereflectentities" => Some("0".to_string()),
        n => get(n),
    };
    m.open(Page::Main, vec!["de_dust2".into()], None, water);
    click(&mut m, MainItem::Options);
    press(&mut m, &[Input::Click(Target::Tab(3), 0)]);
    let adv = button(&m, Action::VideoAdvanced);
    press(&mut m, &[Input::Click(Target::Row(adv), 0)]);
    let w = row_of(&m, "r_waterforceexpensive r_waterforcereflectentities");
    assert!(matches!(control(&m, w), Control::Combo { selected: Some(1), text, .. } if text == "Reflect world"));
    let o = press(&mut m, &[Input::Click(Target::Row(w), 0), Input::Click(Target::ComboItem(2), 0)]);
    assert_eq!(o.lines, ["r_waterforceexpensive 1", "r_waterforcereflectentities 1"], "Reflect all");
    let o = press(&mut m, &[Input::Click(Target::Row(w), 0), Input::Click(Target::ComboItem(0), 0)]);
    assert_eq!(o.lines, ["r_waterforceexpensive 0", "r_waterforcereflectentities 0"], "Simple reflections");
    // Cancel puts both back.
    let o = click_on(&mut m, |m| button(m, Action::Cancel));
    assert_eq!(o.lines, ["r_waterforceexpensive 1", "r_waterforcereflectentities 0"]);
}

#[test]
fn shadow_detail_sets_cs_s_shadow_cvars() {
    let mut m = GameMenu {
        game: TEST,
        in_game: true,
        ..default()
    };
    let shadows = |n: &str| match n {
        "r_shadowrendertotexture" => Some("1".to_string()),
        "r_flashlightdepthtexture" => Some("0".to_string()),
        "r_rootlod" => Some("0".to_string()),
        n => get(n),
    };
    m.open(Page::Main, vec!["de_dust2".into()], None, shadows);
    click(&mut m, MainItem::Options);
    press(&mut m, &[Input::Click(Target::Tab(3), 0)]);
    let adv = button(&m, Action::VideoAdvanced);
    press(&mut m, &[Input::Click(Target::Row(adv), 0)]);
    let s = row_of(&m, "r_shadowrendertotexture r_flashlightdepthtexture");
    assert!(matches!(control(&m, s), Control::Combo { selected: Some(1), text, .. } if text == "Medium"));
    let o = press(&mut m, &[Input::Click(Target::Row(s), 0), Input::Click(Target::ComboItem(0), 0)]);
    assert_eq!(o.lines, ["r_shadowrendertotexture 0", "r_flashlightdepthtexture 0"], "Low: blobs");
    let o = press(&mut m, &[Input::Click(Target::Row(s), 0), Input::Click(Target::ComboItem(2), 0)]);
    assert_eq!(o.lines, ["r_shadowrendertotexture 1", "r_flashlightdepthtexture 1"], "High");
    // Model detail: r_rootlod 2 / 1 / 0.
    let d = row_of(&m, "r_rootlod");
    let o = press(&mut m, &[Input::Click(Target::Row(d), 0), Input::Click(Target::ComboItem(0), 0)]);
    assert_eq!(o.lines, ["r_rootlod 2"], "Low");
}

#[test]
fn the_aspect_ratio_filters_the_resolutions() {
    let mut m = GameMenu {
        game: TEST,
        in_game: true,
        ..default()
    };
    let res = |n: &str| if n == "chimera_resolution" { Some("1920x1080".to_string()) } else { get(n) };
    m.resolutions = vec!["1920x1080".into(), "1680x1050".into(), "1280x1024".into(), "1280x720".into(), "1024x768".into()];
    m.open(Page::Main, vec!["de_dust2".into()], None, res);
    assert_eq!(m.aspect, Some(1), "16:9 from the size now");
    click(&mut m, MainItem::Options);
    press(&mut m, &[Input::Click(Target::Tab(3), 0)]);
    let r = row_of(&m, "chimera_resolution");
    assert!(matches!(control(&m, r), Control::Combo { entries, .. } if entries == ["1920x1080", "1280x720"]));
    // Normal (4:3, 5:4): its sizes, and the size goes to its largest.
    let a = row_with(&m, Field::Aspect);
    let o = press(&mut m, &[Input::Click(Target::Row(a), 0), Input::Click(Target::ComboItem(0), 0)]);
    assert_eq!(o.lines, ["chimera_resolution 1280x1024"]);
    let r = row_of(&m, "chimera_resolution");
    assert!(matches!(control(&m, r), Control::Combo { entries, .. } if entries == ["1280x1024", "1024x768"]));
    assert_eq!(aspect_of("1680x1050"), Some(2));
}

/// Multiplayer > Advanced from a script like the install's
/// `cfg/user.scr`: the cvars Chimera has as controls, the rest greyed.
fn advanced_menu() -> GameMenu {
    let list = |pairs: &[(&str, &str)]| ServerSettingKind::List(pairs.iter().map(|(l, v)| (l.to_string(), v.to_string())).collect());
    let ui = GameUi {
        user_settings: vec![
            ServerSetting {
                cvar: "mp_decals".into(),
                label: "Decal limit".into(),
                kind: ServerSettingKind::Number { min: Some(0.0), max: Some(4096.0) },
                default: "512".into(),
            },
            ServerSetting {
                cvar: "cl_righthand".into(),
                label: "Weapon alignment".into(),
                kind: list(&[("Left handed", "0"), ("Right handed", "1")]),
                default: "0".into(),
            },
            ServerSetting {
                cvar: "cl_clanid".into(),
                label: "Clan Tag".into(),
                kind: list(&[("None", "0")]),
                default: "0".into(),
            },
            ServerSetting {
                cvar: "cl_c4progressbar".into(),
                label: "Defuse progress bar".into(),
                kind: ServerSettingKind::Bool,
                default: "1".into(),
            },
            ServerSetting {
                cvar: "cl_autohelp".into(),
                label: "Auto-help".into(),
                kind: ServerSettingKind::Bool,
                default: "1".into(),
            },
        ],
        ..default()
    };
    let mut m = GameMenu {
        game: TEST,
        in_game: true,
        ..default()
    };
    m.set_ui(Some(Arc::new(ui)));
    let has = |n: &str| match n {
        "mp_decals" => Some("200".to_string()),
        "cl_c4progressbar" => Some("1".to_string()),
        n => get(n),
    };
    m.open(Page::Main, vec!["de_dust2".into()], None, has);
    click(&mut m, MainItem::Options);
    press(&mut m, &[Input::Click(Target::Tab(5), 0)]);
    assert_eq!(m.tab, Tab::Multiplayer);
    let adv = button(&m, Action::MultiplayerAdvanced);
    press(&mut m, &[Input::Click(Target::Row(adv), 0)]);
    assert_eq!(m.page, Page::MultiplayerAdvanced);
    m
}

#[test]
fn multiplayer_advanced_lists_the_scripts_options() {
    let mut m = advanced_menu();
    let rows = m.rows();
    let labels: Vec<String> = rows.iter().map(crate::game_menu::label_of).collect();
    assert_eq!(labels, ["Decal limit", "Weapon alignment", "Clan Tag", "Defuse progress bar", "Auto-help", "OK", "Cancel"]);
    // Ours as controls with their values now; Chimera lacks the clan tag
    // and auto-help: greyed, showing the script's defaults, not focused.
    assert_eq!(control(&m, 0), Control::Text { text: "200".into(), numeric: true, max: 8 });
    assert!(matches!(control(&m, 1), Control::Combo { text, .. } if text == "Right handed"));
    assert!(matches!(&rows[2], Row::Greyed { control: Control::Combo { text, .. }, .. } if text == "None"));
    assert!(matches!(&rows[4], Row::Greyed { control: Control::Check(true), .. }));
    assert!(!rows[2].focusable() && !rows[4].focusable());
    assert_eq!(m.list(), (5, ADVANCED_ROWS));
    // Changes wait for OK (CS:S's dialog): left-handed, fewer decals, no bar.
    let o = press(&mut m, &[Input::Click(Target::Row(1), 0), Input::Click(Target::ComboItem(0), 0)]);
    assert!(o.lines.is_empty());
    press(&mut m, &[Input::Click(Target::Row(0), 0), Input::Edit(Edit::SelectAll), Input::Type("50".into())]);
    let o = press(&mut m, &[Input::Click(Target::Row(3), 0)]);
    assert!(o.lines.is_empty());
    let o = click_on(&mut m, |m| button(m, Action::Ok));
    assert_eq!(o.lines, ["mp_decals 50", "cl_righthand 0", "cl_c4progressbar 0"]);
    assert_eq!((m.page, m.focus), (Page::Settings, button(&m, Action::MultiplayerAdvanced)));
    // Opened again: as they are now; Cancel (Esc) runs nothing.
    let adv = button(&m, Action::MultiplayerAdvanced);
    press(&mut m, &[Input::Click(Target::Row(adv), 0)]);
    assert_eq!(control(&m, 0), Control::Text { text: "50".into(), numeric: true, max: 8 });
    let o = press(&mut m, &[Input::Click(Target::Row(3), 0), Input::Close]);
    assert!(o.lines.is_empty());
    assert_eq!(m.page, Page::Settings);
    let o = press(&mut m, &[Input::Click(Target::Row(adv), 0), Input::Activate]);
    assert!(o.lines.is_empty(), "nothing changed: {:?}", o.lines);
}

#[test]
fn multiplayer_advanced_without_the_install_has_the_weapon_hand() {
    let mut m = menu();
    click(&mut m, MainItem::Options);
    press(&mut m, &[Input::Click(Target::Tab(5), 0)]);
    click_on(&mut m, |m| button(m, Action::MultiplayerAdvanced));
    let hand = row_with(&m, Field::UserCvar(0));
    assert!(matches!(control(&m, hand), Control::Combo { text, .. } if text == "Right handed"));
    press(&mut m, &[Input::Click(Target::Row(hand), 0), Input::Click(Target::ComboItem(0), 0)]);
    let o = click_on(&mut m, |m| button(m, Action::Ok));
    assert_eq!(o.lines, ["cl_righthand 0"]);
}

/// The three options this game's code reads (`cl_autowepswitch`,
/// `hud_centerid`, `cl_disablefreezecam`) are check boxes there, with
/// `user.scr`'s defaults, and OK sets the cvars.
#[test]
fn multiplayer_advanced_has_the_switch_names_and_freeze_cam() {
    let mut m = menu();
    click(&mut m, MainItem::Options);
    press(&mut m, &[Input::Click(Target::Tab(5), 0)]);
    click_on(&mut m, |m| button(m, Action::MultiplayerAdvanced));
    let rows: Vec<String> = m.rows().iter().map(crate::game_menu::label_of).collect();
    assert_eq!(
        rows[1..4],
        [
            "Automatically switch to picked up weapons (if more powerful)",
            "Center player names",
            "Disable freeze cam"
        ]
    );
    let switch = row_with(&m, Field::UserCvar(1));
    let names = row_with(&m, Field::UserCvar(2));
    let freeze = row_with(&m, Field::UserCvar(3));
    assert_eq!(control(&m, switch), Control::Check(true));
    assert_eq!(control(&m, names), Control::Check(true));
    assert_eq!(control(&m, freeze), Control::Check(false));
    for r in [switch, names, freeze] {
        press(&mut m, &[Input::Click(Target::Row(r), 0)]);
    }
    let o = click_on(&mut m, |m| button(m, Action::Ok));
    assert_eq!(o.lines, ["cl_autowepswitch 0", "hud_centerid 0", "cl_disablefreezecam 1"]);
}

#[test]
fn the_voice_tab_is_there_and_greyed() {
    let mut m = menu();
    click(&mut m, MainItem::Options);
    press(&mut m, &[Input::Click(Target::Tab(4), 0)]);
    assert_eq!(m.tab, Tab::Voice);
    // Without voice chat's cvars: only the dialog's buttons.
    assert!(m.rows().iter().all(|r| matches!(r, Row::Button { .. })));
}

/// With voice chat (`client::voice`'s cvars and `voice_test`) the tab's
/// enable box, receive volume and microphone boost work, and Test
/// Microphone starts the test (and, once running, stops it).
#[test]
fn the_voice_tab_sets_voice_chats_cvars() {
    let mut m = GameMenu {
        game: TEST,
        in_game: true,
        ..default()
    };
    let cvars = |n: &str| match n {
        "voice_modenable" | "voice_scale" | "voice_transmit" => Some("1".to_string()),
        "voice_boost" => Some("0".to_string()),
        n => get(n),
    };
    m.open(Page::Main, vec!["de_dust2".into()], None, cvars);
    m.known.insert("voice_test".into());
    click(&mut m, MainItem::Options);
    press(&mut m, &[Input::Click(Target::Tab(4), 0)]);
    assert_eq!(m.tab, Tab::Voice);
    let rows = m.rows();
    let settings = rows.iter().filter(|r| !matches!(r, Row::Button { .. })).count();
    assert_eq!(settings, 4, "{rows:?}");
    let boost = rows
        .iter()
        .position(|r| matches!(r, Row::Control { field: Field::Setting(i), .. } if SETTINGS[*i].cvar == "voice_boost"))
        .unwrap();
    assert_eq!(press(&mut m, &[Input::Click(Target::Row(boost), 0)]).lines, ["voice_boost 1"]);
    let test = |m: &GameMenu| {
        m.rows()
            .iter()
            .position(|r| matches!(r, Row::Button { action: Action::Run(l, _), .. } if l.starts_with("voice_test")))
            .unwrap()
    };
    let t = test(&m);
    assert!(matches!(&m.rows()[t], Row::Button { label, .. } if label == "Test Microphone"));
    assert_eq!(press(&mut m, &[Input::Click(Target::Row(t), 0)]).lines, ["voice_test 1"]);
    m.voice_testing = true;
    let t = test(&m);
    assert!(matches!(&m.rows()[t], Row::Button { label, .. } if label == "Stop Microphone Test"));
    assert_eq!(press(&mut m, &[Input::Click(Target::Row(t), 0)]).lines, ["voice_test 0"]);
}

/// The Voice tab's meter: none lit in silence, all at full scale, on a
/// decibel scale between.
#[test]
fn the_microphone_meter_is_in_decibels() {
    assert_eq!(draw::meter_bars(0.0, 19), 0);
    assert_eq!(draw::meter_bars(1.0, 19), 19);
    assert_eq!(draw::meter_bars(0.001, 19), 0, "-60 dB: under the floor");
    // -24 dB, half way down to the floor.
    assert_eq!(draw::meter_bars(10f32.powf(-24.0 / 20.0), 19), 10);
}

/// The menu with the cvars of the Audio tab's drop-downs, the texture
/// settings and the brightness.
fn av_menu() -> GameMenu {
    let mut m = GameMenu {
        game: TEST,
        in_game: true,
        ..default()
    };
    let cvars = |n: &str| match n {
        "snd_surround_speakers" => Some("2".to_string()),
        "snd_pitchquality" => Some("1".to_string()),
        "dsp_slow_cpu" => Some("0".to_string()),
        "closecaption" | "cc_subtitles" => Some("0".to_string()),
        "chimera_spoken_language" => Some("english".to_string()),
        "mat_picmip" => Some("0".to_string()),
        "mat_trilinear" => Some("0".to_string()),
        "mat_forceaniso" => Some("1".to_string()),
        "mat_monitorgamma" => Some("2.2".to_string()),
        n => get(n),
    };
    m.open(Page::Main, vec!["de_dust2".into()], None, cvars);
    m
}

/// The Audio tab's speaker, quality, captioning and language drop-downs
/// list CS:S's entries and set its cvars (quality and captioning two
/// each).
#[test]
fn the_audio_tabs_drop_downs_have_their_entries() {
    let mut m = av_menu();
    click(&mut m, MainItem::Options);
    press(&mut m, &[Input::Click(Target::Tab(2), 0)]);
    assert_eq!(m.tab, Tab::Audio);
    let entries = |m: &GameMenu, cvar: &str| match control(m, row_of(m, cvar)) {
        Control::Combo { entries, selected, .. } => (entries, selected),
        c => panic!("{c:?}"),
    };
    assert_eq!(
        entries(&m, "snd_surround_speakers"),
        (
            ["Headphones", "2 Speakers", "4 Speakers", "5.1 Speakers", "7.1 Speakers"].map(String::from).to_vec(),
            Some(1)
        )
    );
    assert_eq!(entries(&m, "snd_pitchquality dsp_slow_cpu"), (["Low", "Medium", "High"].map(String::from).to_vec(), Some(2)));
    assert_eq!(
        entries(&m, "closecaption cc_subtitles"),
        (
            ["No captions", "Subtitles (dialog only)", "Closed Captions"].map(String::from).to_vec(),
            Some(0)
        )
    );
    assert_eq!(entries(&m, "chimera_spoken_language"), (vec!["English".to_string()], Some(0)));
    let speakers = row_of(&m, "snd_surround_speakers");
    let o = press(&mut m, &[Input::Click(Target::Row(speakers), 0), Input::Click(Target::ComboItem(0), 0)]);
    assert_eq!(o.lines, ["snd_surround_speakers 0"], "headphones");
    let quality = row_of(&m, "snd_pitchquality dsp_slow_cpu");
    let o = press(&mut m, &[Input::Click(Target::Row(quality), 0), Input::Click(Target::ComboItem(0), 0)]);
    assert_eq!(o.lines, ["snd_pitchquality 0", "dsp_slow_cpu 1"], "low");
    let captions = row_of(&m, "closecaption cc_subtitles");
    let o = press(&mut m, &[Input::Click(Target::Row(captions), 0), Input::Click(Target::ComboItem(1), 0)]);
    assert_eq!(o.lines, ["closecaption 1", "cc_subtitles 1"], "subtitles");
    let o = click_on(&mut m, |m| button(m, Action::Cancel));
    assert_eq!(
        o.lines,
        ["snd_surround_speakers 2", "snd_pitchquality 1", "dsp_slow_cpu 0", "closecaption 0", "cc_subtitles 0"],
        "Cancel puts them back"
    );
}

/// Video > Advanced's texture detail and filtering drop-downs set CS:S's
/// cvars.
#[test]
fn texture_detail_and_filtering_set_their_cvars() {
    let mut m = av_menu();
    click(&mut m, MainItem::Options);
    press(&mut m, &[Input::Click(Target::Tab(3), 0)]);
    click_on(&mut m, |m| button(m, Action::VideoAdvanced));
    let detail = row_of(&m, "mat_picmip");
    assert!(matches!(control(&m, detail), Control::Combo { selected: Some(2), text, .. } if text == "High"));
    let o = press(&mut m, &[Input::Click(Target::Row(detail), 0), Input::Click(Target::ComboItem(0), 0)]);
    assert_eq!(o.lines, ["mat_picmip 2"], "low");
    let filter = row_of(&m, "mat_trilinear mat_forceaniso");
    assert!(matches!(control(&m, filter), Control::Combo { selected: Some(0), text, .. } if text == "Bilinear"));
    let o = press(&mut m, &[Input::Click(Target::Row(filter), 0), Input::Click(Target::ComboItem(4), 0)]);
    assert_eq!(o.lines, ["mat_trilinear 0", "mat_forceaniso 8"], "anisotropic 8x");
}

/// "Adjust brightness levels..." opens the gamma dialog over the options:
/// its slider and the entry beside it set `mat_monitorgamma`; Cancel
/// puts it back.
#[test]
fn the_brightness_dialog_sets_the_gamma() {
    let mut layout = UiLayout::default();
    for (name, kind) in [("Gamma", "CCvarSlider"), ("GammaEntry", "TextEntry"), ("OKButton", "Button"), ("Button1", "Button")] {
        layout
            .controls
            .push(UiControl::new(name, UiKind::Other(kind.into()), 0.0, 0.0, 100.0, 24.0));
    }
    let ui = GameUi {
        options: HashMap::from([("video_gamma".to_string(), layout)]),
        ..default()
    };
    let mut m = av_menu();
    m.set_ui(Some(Arc::new(ui)));
    click(&mut m, MainItem::Options);
    press(&mut m, &[Input::Click(Target::Tab(3), 0)]);
    let gamma_button = button(&m, Action::Gamma);
    assert_eq!(crate::game_menu::label_of(&m.rows()[gamma_button]), "Adjust brightness levels...");
    press(&mut m, &[Input::Click(Target::Row(gamma_button), 0)]);
    assert_eq!(m.page, Page::Gamma);
    let slider = row_of(&m, "mat_monitorgamma");
    assert!(matches!(control(&m, slider), Control::Slider { .. }));
    let entry = row_with(&m, Field::SettingText(setting("mat_monitorgamma")));
    assert!(matches!(control(&m, entry), Control::Text { text, .. } if text == "2.20"));
    // A step to the right: darker.
    let o = press(&mut m, &[Input::Click(Target::Row(slider), 0), Input::Right]);
    assert_eq!(o.lines, ["mat_monitorgamma 2.25"]);
    let o = click_on(&mut m, |m| button(m, Action::Cancel));
    assert_eq!(o.lines, ["mat_monitorgamma 2.2"], "as it was");
    assert_eq!((m.page, m.focus), (Page::Settings, gamma_button));
    // VR mode is left out of the video tab on purpose.
    assert!(crate::game_menu::draw::LEFT_OUT.contains(&"VRMode"));
}

/// The dialogs' frames (`client::widgets`) draw over the menu's title and
/// entries; the menu over every in-game panel (the message of the day,
/// team, class, buy, radio and spectator menus at 50-52, the scoreboard
/// at 60), as CS:S's GameUI sits over the viewport's panels.
#[test]
fn frames_draw_over_the_menu_and_the_menu_over_the_panels() {
    assert!(MENU_Z < crate::client::widgets::FRAME_Z);
    assert!(MENU_Z > 60, "over the scoreboard and the VGUI panels");
}

/// Opening the game menu keeps the in-game panels for when it closes
/// (CS:S's GameUI over the viewport), the mouse left free for them.
#[test]
fn opening_the_game_menu_keeps_the_panels_and_the_mouse_for_them() {
    use crate::client::vgui::VguiOpen;
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, crate::console::ConsolePlugin))
        .init_resource::<crate::net::NetSettings>()
        .insert_resource(GameMenu {
            game: TEST,
            in_game: true,
            ..default()
        })
        .init_resource::<RegrabCursor>()
        .init_resource::<crate::client::console::ConsoleUi>()
        .init_resource::<ButtonInput<MouseButton>>()
        .insert_resource(VguiOpen {
            team: true,
            ..default()
        })
        .add_systems(Update, cursor);
    app.world_mut().spawn(CursorOptions::default());
    open_menu(app.world_mut(), Page::Main);
    app.update();
    // Resume.
    app.world_mut().resource_mut::<GameMenu>().open = false;
    app.world_mut().resource_mut::<RegrabCursor>().0 = true;
    app.update();
    let mut q = app.world_mut().query::<&CursorOptions>();
    assert!(!crate::client::input::cursor_grabbed(q.single(app.world()).unwrap()), "the team menu has the mouse");
    // With no panel up the mouse is grabbed again.
    *app.world_mut().resource_mut::<VguiOpen>() = VguiOpen::default();
    app.world_mut().resource_mut::<GameMenu>().open = true;
    app.update();
    app.world_mut().resource_mut::<GameMenu>().open = false;
    app.world_mut().resource_mut::<RegrabCursor>().0 = true;
    app.update();
    let mut q = app.world_mut().query::<&CursorOptions>();
    assert!(crate::client::input::cursor_grabbed(q.single(app.world()).unwrap()));
}

/// The player list (CS:S's `PlayerListDialog`): a player selected, Mute
/// in-game voice mutes it by user id; once muted the button unmutes;
/// oneself and bots can't be muted.
#[test]
fn the_player_list_mutes_the_selected_player() {
    let mut m = menu();
    let entries = m.entries();
    let at = entries.iter().position(|e| e.item == MainItem::PlayerList).expect("an in-game entry");
    assert_eq!(at, 2, "under Disconnect, as CS:S's");
    press(&mut m, &[Input::Click(Target::Main(at), 0)]);
    assert_eq!(m.page, Page::PlayerList);
    let player = |id: Option<u64>, name: &str, me: bool, muted: bool| ListedPlayer {
        id,
        name: name.into(),
        me,
        muted,
    };
    m.set_roster(vec![
        player(Some(3), "alice", false, false),
        player(None, "Bot Bob", false, false),
        player(Some(0), "me", true, false),
    ]);
    let rows = m.rows();
    assert_eq!(rows.len(), 5, "three players, Mute, Close");
    assert!(matches!(&rows[1], Row::Player { properties, .. } if properties == "Bot"));
    // Nothing selected: Mute greyed.
    assert!(matches!(&rows[3], Row::Button { enabled: false, .. }));
    let out = press(&mut m, &[Input::Click(Target::Row(0), 0), Input::Click(Target::Row(3), 0)]);
    assert_eq!(out.lines, vec!["voice_mute 3".to_string()]);
    // The game muted it: the list says so and the button unmutes.
    m.set_roster(vec![
        player(Some(3), "alice", false, true),
        player(None, "Bot Bob", false, false),
        player(Some(0), "me", true, false),
    ]);
    assert_eq!(m.key_row, Some(0), "still selected");
    let rows = m.rows();
    assert!(matches!(&rows[0], Row::Player { properties, .. } if properties == "Muted"));
    assert!(matches!(&rows[3], Row::Button { label, enabled: true, .. } if label.contains("Unmute")));
    // Enter on the selected player presses the default button.
    m.focus = 0;
    assert_eq!(press(&mut m, &[Input::Activate]).lines, vec!["voice_unmute 3".to_string()]);
    // A bot and oneself: greyed.
    for r in [1, 2] {
        press(&mut m, &[Input::Click(Target::Row(r), 0)]);
        assert!(matches!(&m.rows()[3], Row::Button { enabled: false, .. }), "row {r}");
    }
    // Close: back to the menu.
    press(&mut m, &[Input::Click(Target::Row(4), 0)]);
    assert_eq!(m.page, Page::Main);
}

fn issues() -> Vec<crate::net::vote::IssueInfo> {
    use crate::net::vote::IssueInfo;
    vec![
        IssueInfo {
            name: "kick".into(),
            choices: vec![("2".into(), "Bob".into()), ("3".into(), "Eve".into())],
        },
        IssueInfo {
            name: "changelevel".into(),
            choices: vec![("de_dust2".into(), "de_dust2".into()), ("cs_office".into(), "cs_office".into())],
        },
        IssueInfo {
            name: "restartgame".into(),
            choices: Vec::new(),
        },
    ]
}

fn vote_row(m: &GameMenu, action: Action) -> usize {
    m.rows()
        .iter()
        .position(|r| matches!(r, Row::Button { action: a, .. } if *a == action))
        .unwrap_or_else(|| panic!("no {action:?} row"))
}

fn call_enabled(m: &GameMenu) -> bool {
    m.rows()
        .iter()
        .any(|r| matches!(r, Row::Button { action: Action::CallVote, enabled: true, .. }))
}

#[test]
fn the_vote_setup_dialog_asks_the_server_then_calls_the_picked_vote() {
    // `callvote` alone: the server's issues open the dialog
    // (`vote_setup_arrived`).
    let mut m = menu();
    m.go_to(Page::Vote);
    assert!(!call_enabled(&m), "nothing to call yet");
    m.show_vote_setup(issues());
    let labels: Vec<String> = m.rows().iter().map(label_of).collect();
    assert_eq!(&labels[..3], ["Kick Player", "Change Level", "Restart Game"]);
    // A kick needs a player: picked, the players show, one is picked.
    let kick = vote_row(&m, Action::VoteIssue(0));
    m.handle(Input::Click(Target::Row(kick), 0));
    assert!(!call_enabled(&m));
    let eve = vote_row(&m, Action::VoteChoice(1));
    assert_eq!(label_of(&m.rows()[eve]), "Eve");
    m.handle(Input::Click(Target::Row(eve), 0));
    assert!(call_enabled(&m));
    // The issues again (the server's answer to another `callvote`): the
    // picks stay.
    m.show_vote_setup(issues());
    assert_eq!((m.vote.issue, m.vote.choice), (Some(0), Some(1)));
    // Another issue drops the choice; the maps show.
    let level = vote_row(&m, Action::VoteIssue(1));
    m.handle(Input::Click(Target::Row(level), 0));
    assert_eq!(m.vote.choice, None);
    let office = vote_row(&m, Action::VoteChoice(1));
    assert_eq!(label_of(&m.rows()[office]), "cs_office");
    m.handle(Input::Click(Target::Row(office), 0));
    let call = vote_row(&m, Action::CallVote);
    let out = m.handle(Input::Click(Target::Row(call), 0));
    assert_eq!(out.lines, vec!["callvote changelevel cs_office".to_string()]);
    assert!(out.close && !m.open, "calling closes the menu");
}

#[test]
fn the_vote_setup_dialog_opened_by_callvote_closes_back_to_the_game() {
    let mut m = menu();
    m.open = false;
    m.show_vote_setup(issues());
    m.open(Page::Vote, Vec::new(), None, get);
    let restart = vote_row(&m, Action::VoteIssue(2));
    m.handle(Input::Click(Target::Row(restart), 0));
    // No choices: callable at once.
    assert_eq!(m.vote.line().as_deref(), Some("callvote restartgame"));
    let close = m.rows().len() - 1;
    let out = m.handle(Input::Click(Target::Row(close), 0));
    assert!(out.close && !m.open, "Close goes back to the game");
}

// ---- HL2:DM's pickers, Import Spray and Server Type
// (specs/hl2dm/game_ui.md 2.1-2.4, 4; its test cases 3-15).

static HL2_GAME: crate::MenuGame = crate::MenuGame {
    player_models: true,
    sprays: true,
    server_types: true,
    bots: None,
    rounds_cvar: None,
    teams: None,
    quick_start: None,
    layout_hides: true,
    ..TEST_GAME_BASE
};

/// A game like `TEST_GAME` without its extras.
const TEST_GAME_BASE: crate::MenuGame = crate::MenuGame {
    info: &TEST_INFO,
    name: "A Game",
    steam_folder: "A Game",
    strings: "resource/a_english.txt",
    bots: None,
    rounds_cvar: None,
    teams: None,
    quick_start: None,
    layout_hides: false,
    player_models: false,
    sprays: false,
    server_types: false,
};

const MODELS: [&str; 5] = ["combine_soldier", "humans/group03/female_01", "humans/group03/male_05", "humans/group03/male_09", "police"];
const SPRAYS: [&str; 3] = ["spray_canned", "spray_combine", "spray_lambda"];

fn hl2_ui() -> GameUi {
    let layout = source_engine::hud::gameui::layout_of_res(
        r#""x" {
            "TextEntry1" { "ControlName" "TextEntry" "fieldName" "TextEntry1" "xpos" "277" "ypos" "33" "wide" "190" "tall" "203" "editable" "0" "zpos" "-1" }
            "ModelImage" { "ControlName" "Panel" "fieldName" "ModelImage" "xpos" "277" "ypos" "34" "wide" "240" "tall" "256" }
            "Player model" { "ControlName" "CLabeledCommandComboBox" "fieldName" "Player model" "xpos" "277" "ypos" "266" "wide" "190" "tall" "24" }
            "LogoImage" { "ControlName" "ImagePanel" "fieldName" "LogoImage" "xpos" "28" "ypos" "61" "wide" "64" "tall" "64" }
            "SpraypaintList" { "ControlName" "CLabeledCommandComboBox" "fieldName" "SpraypaintList" "xpos" "100" "ypos" "61" "wide" "124" "tall" "24" }
            "ImportSprayImage" { "ControlName" "Button" "fieldName" "ImportSprayImage" "xpos" "100" "ypos" "93" "wide" "124" "tall" "24" "labelText" "Import Spray..." }
        }"#,
    )
    .expect("layout");
    let server = source_engine::hud::gameui::layout_of_res(
        r#""x" {
            "MapList" { "ControlName" "ComboBox" "fieldName" "MapList" "xpos" "56" "ypos" "20" "wide" "200" "tall" "24" }
            "VisibilityType" { "ControlName" "ComboBox" "fieldName" "VisibilityType" "xpos" "16" "ypos" "74" "wide" "200" "tall" "24" }
        }"#,
    )
    .expect("layout");
    let scr = source_engine::hud::gameui::scr_settings(
        r#"VERSION 1.0
        {
            "hostname" { "Hostname" { STRING } { "Half-Life 2 Deathmatch" } }
            "sv_password" { "Server password" { STRING } { "" } }
            "maxplayers" { "Max. players" { NUMBER 1 32 } { "32" } }
            "sv_password" { "Server password" { STRING } { "" } }
            "mp_timelimit" { "Time Limit (Min.)" { NUMBER 0 -1 } { "0" } }
            "mp_fraglimit" { "Frag Limit" { NUMBER 0 -1 } { "0" } }
            "mp_teamplay" { "Teamplay" { BOOL } { "0" } }
            "mp_weaponstay" { "Weapons Stay" { BOOL } { "0" } }
            "mp_footsteps" { "Footsteps" { BOOL } { "1" } }
            "mp_flashlight" { "Flashlight" { BOOL } { "0" } }
            "mp_forcerespawn" { "Force Respawn" { BOOL } { "0" } }
        }"#,
        &HashMap::new(),
    );
    GameUi {
        options: HashMap::from([("multiplayer".to_string(), layout), ("create_server".to_string(), server)]),
        player_models: MODELS.iter().map(|s| s.to_string()).collect(),
        sprays: SPRAYS.iter().map(|s| s.to_string()).collect(),
        server_settings: scr,
        ..default()
    }
}

fn hl2_menu(cvars: &'static [(&'static str, &'static str)], logos: Option<std::path::PathBuf>) -> GameMenu {
    let mut m = GameMenu::for_game(&HL2_GAME);
    m.in_game = true;
    m.logos_dir = logos;
    m.set_ui(Some(Arc::new(hl2_ui())));
    let g = move |n: &str| {
        cvars
            .iter()
            .chain(&[("hostname", "Half-Life 2 Deathmatch"), ("maxplayers", "32"), ("mp_teamplay", "0"), ("mp_timelimit", "0")])
            .find(|(k, _)| *k == n)
            .map(|(_, v)| v.to_string())
    };
    m.open(Page::Settings, vec!["dm_lockdown".into(), "dm_overwatch".into()], Some("dm_lockdown"), g);
    m.set_tab(Tab::Multiplayer);
    m
}

fn picker_row(m: &GameMenu, k: PickerKind) -> usize {
    row_with(m, Field::Picker(k))
}

fn choose_in(m: &mut GameMenu, kind: PickerKind, k: usize) {
    let row = picker_row(m, kind);
    choose(m, row, k);
}

fn choose(m: &mut GameMenu, row: usize, k: usize) {
    press(m, &[Input::Click(Target::Row(row), 0), Input::Click(Target::ComboItem(k), 0)]);
}

/// Case 3 and 2.4: the pickers, the import button; no crosshair controls.
#[test]
fn the_multiplayer_page_lists_the_models_and_sprays() {
    let m = hl2_menu(&[], None);
    let rows = m.rows();
    match control(&m, picker_row(&m, PickerKind::Model)) {
        Control::Combo { entries, selected, .. } => {
            assert_eq!(entries, ["combine_soldier", "female_01", "male_05", "male_09", "police"]);
            assert_eq!(selected, Some(0), "nothing matches `none`: the first, shown only");
        }
        c => panic!("{c:?}"),
    }
    match control(&m, picker_row(&m, PickerKind::Spray)) {
        Control::Combo { entries, selected, .. } => {
            assert_eq!(entries, SPRAYS);
            assert_eq!(selected, Some(0));
        }
        c => panic!("{c:?}"),
    }
    assert!(rows.iter().any(|r| matches!(r, Row::Button { action: Action::ImportSpray, .. })));
    assert!(rows.iter().all(|r| !matches!(r, Row::Control { field: Field::Setting(i), .. } if SETTINGS[*i].cvar.starts_with("cl_crosshair"))));
    // The layout keeps its negative `zpos` box and the picture panel.
    let layout = m.layout().unwrap();
    assert_eq!(layout.get("TextEntry1").unwrap().keys.get("zpos").map(String::as_str), Some("-1"));
    assert!(layout.get("ModelImage").is_some());
    // CS:S's page (no flags) has neither picker.
    let mut css = GameMenu::for_game(&TEST_GAME_BASE);
    css.set_ui(Some(Arc::new(hl2_ui())));
    css.open(Page::Settings, vec![], None, get);
    css.set_tab(Tab::Multiplayer);
    assert!(css.rows().iter().all(|r| !matches!(r.field(), Some(Field::Picker(_)))));
}

/// Cases 4 and 5: OK runs the chosen entries' lines.
#[test]
fn ok_runs_the_chosen_model_and_spray() {
    let mut m = hl2_menu(&[], None);
    choose_in(&mut m, PickerKind::Model, 2);
    choose_in(&mut m, PickerKind::Spray, 2);
    assert_eq!(m.preview_picture(PickerKind::Model), Some("playermodels/humans/group03/male_05"));
    assert_eq!(m.preview_picture(PickerKind::Spray), Some("logos/spray_lambda"));
    let out = click_on(&mut m, |m| button(m, Action::Ok));
    assert_eq!(
        out.lines,
        ["cl_playermodel models/humans/group03/male_05.mdl", "cl_logofile materials/vgui/logos/spray_lambda.vtf"]
    );
}

/// Cases 6 and 7: Cancel runs nothing; Apply runs at once and the page stays.
#[test]
fn cancel_drops_and_apply_runs_the_choice() {
    let mut m = hl2_menu(&[], None);
    assert!(!matches!(m.rows()[button(&m, Action::Apply)], Row::Button { enabled: true, .. }), "Apply greyed with nothing chosen");
    choose_in(&mut m, PickerKind::Model, 4);
    assert_eq!(m.preview_picture(PickerKind::Model), Some("playermodels/police"));
    assert!(matches!(m.rows()[button(&m, Action::Apply)], Row::Button { enabled: true, .. }));
    let out = click_on(&mut m, |m| button(m, Action::Cancel));
    assert!(out.lines.is_empty());
    assert_eq!(m.page, Page::Main);
    // Again, then Apply: the line now, the page open.
    m.open(Page::Settings, vec![], None, |_| None);
    m.set_tab(Tab::Multiplayer);
    choose_in(&mut m, PickerKind::Model, 4);
    let out = click_on(&mut m, |m| button(m, Action::Apply));
    assert_eq!(out.lines, ["cl_playermodel models/police.mdl"]);
    assert_eq!(m.page, Page::Settings);
    // OK after Apply with no other change: nothing runs twice.
    let out = click_on(&mut m, |m| button(m, Action::Ok));
    assert!(out.lines.is_empty(), "{:?}", out.lines);
}

/// Case 8 and the opening selection: `none` stays; a model the cvar names
/// is selected and not run again.
#[test]
fn nothing_runs_unless_the_selection_changes() {
    let mut m = hl2_menu(&[], None);
    let out = click_on(&mut m, |m| button(m, Action::Ok));
    assert!(out.lines.is_empty(), "cl_playermodel stays none: {:?}", out.lines);
    let mut m = hl2_menu(
        &[("cl_playermodel", "models/humans/group03/male_09.mdl"), ("cl_logofile", "materials/vgui/logos/spray_combine.vtf")],
        None,
    );
    assert!(matches!(control(&m, picker_row(&m, PickerKind::Model)), Control::Combo { selected: Some(3), .. }));
    assert!(matches!(control(&m, picker_row(&m, PickerKind::Spray)), Control::Combo { selected: Some(1), .. }));
    assert!(click_on(&mut m, |m| button(m, Action::Ok)).lines.is_empty());
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("chimera_menu_{name}_{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write_tga(path: &std::path::Path, side: u32) {
    // An uncompressed 32-bit Targa.
    let mut f = vec![0u8, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    f.extend_from_slice(&(side as u16).to_le_bytes());
    f.extend_from_slice(&(side as u16).to_le_bytes());
    f.extend_from_slice(&[32, 8]);
    f.extend(std::iter::repeat_n(128u8, (side * side * 4) as usize));
    std::fs::write(path, f).unwrap();
}

fn type_path_and_import(m: &mut GameMenu, path: &std::path::Path) {
    click_on(m, |m| button(m, Action::ImportSpray));
    assert_eq!(m.page, Page::ImportSpray);
    let row = row_with(m, Field::ImportPath);
    press(m, &[Input::FocusRow(row), Input::Type(path.to_string_lossy().into_owned())]);
    click_on(m, |m| button(m, Action::ImportDo));
}

/// Case 9: a 100 x 100 TGA is refused in the error box; the list is unchanged.
#[test]
fn a_wrong_sized_spray_shows_the_error_box() {
    let dir = temp_dir("bad");
    let src = dir.join("big.tga");
    write_tga(&src, 100);
    let mut m = hl2_menu(&[], Some(dir.join("logos")));
    type_path_and_import(&mut m, &src);
    assert_eq!(m.page, Page::SprayError);
    assert!(m.import_error.as_deref().unwrap().starts_with("Source image is not the correct size."));
    assert!(matches!(&m.rows()[0], Row::Info(t) if t.starts_with("Source image")));
    click_on(&mut m, |m| button(m, Action::Ok));
    assert_eq!(m.page, Page::Settings);
    assert_eq!(m.pickers.spray.entries.len(), SPRAYS.len());
    std::fs::remove_dir_all(&dir).ok();
}

/// Case 10: a 128 x 128 `myspray.tga` joins the list, selected; OK runs its line.
#[test]
fn an_imported_spray_is_listed_and_selected() {
    let dir = temp_dir("good");
    let src = dir.join("myspray.tga");
    write_tga(&src, 128);
    let logos = dir.join("logos");
    let mut m = hl2_menu(&[], Some(logos.clone()));
    type_path_and_import(&mut m, &src);
    assert_eq!(m.page, Page::Settings);
    assert!(logos.join("myspray.vtf").is_file(), "written to the user's folder");
    let texts: Vec<&str> = m.pickers.spray.entries.iter().map(|e| e.text.as_str()).collect();
    assert_eq!(texts, ["myspray", "spray_canned", "spray_combine", "spray_lambda"]);
    assert_eq!(m.pickers.spray.current().unwrap().text, "myspray");
    let out = click_on(&mut m, |m| button(m, Action::Ok));
    assert_eq!(out.lines, ["cl_logofile materials/vgui/logos/myspray.vtf"]);
    std::fs::remove_dir_all(&dir).ok();
}

fn create_server(m: &mut GameMenu) {
    m.open(Page::NewGame, vec!["dm_lockdown".into(), "dm_overwatch".into()], Some("dm_lockdown"), |n| {
        match n {
            "hostname" => Some("Half-Life 2 Deathmatch".into()),
            "maxplayers" => Some("32".into()),
            "mp_teamplay" => Some("0".into()),
            "mp_timelimit" | "mp_fraglimit" | "mp_weaponstay" | "mp_flashlight" | "mp_forcerespawn" | "sv_password" => Some("0".into()),
            "mp_footsteps" => Some("1".into()),
            _ => None,
        }
    });
}

/// Case 11: the Server tab has the map and Server Type, in that order; no bot, name or password.
#[test]
fn the_server_tab_has_the_server_type() {
    let mut m = hl2_menu(&[], None);
    create_server(&mut m);
    let rows = m.rows();
    let fields: Vec<_> = rows.iter().filter_map(Row::field).collect();
    assert_eq!(fields, [Field::Map, Field::Visibility]);
    match control(&m, row_with(&m, Field::Visibility)) {
        Control::Combo { entries, selected, .. } => {
            assert_eq!(entries, ["Local Network (LAN)", "Public (Unlisted)", "Public (Listed)", "Friends Only"]);
            assert_eq!(selected, Some(0));
        }
        c => panic!("{c:?}"),
    }
}

/// Case 12: untouched, Start runs `sv_lan 1` (the LAN default) before the map.
#[test]
fn start_defaults_to_a_lan_server() {
    let mut m = hl2_menu(&[], None);
    create_server(&mut m);
    let out = click_on(&mut m, |m| button(m, Action::Start));
    assert_eq!(&out.lines[..3], ["sv_lan 1", "sv_allow_server_adverisement_to_master_server 0", "sv_friends_only 0"]);
    assert_eq!(out.lines.last().unwrap(), "map dm_lockdown");
}

/// Case 13: Public (Listed), and the choice is remembered.
#[test]
fn a_listed_server_is_advertised_and_remembered() {
    let mut m = hl2_menu(&[], None);
    create_server(&mut m);
    let row = row_with(&m, Field::Visibility);
    choose(&mut m, row, 2);
    let out = click_on(&mut m, |m| button(m, Action::Start));
    assert_eq!(
        &out.lines[..5],
        ["sv_lan 0", "setmaster enable", "sv_allow_server_adverisement_to_master_server 1", "sv_friends_only 0", "chimera_server_type 2"]
    );
    // Next time it opens on the last choice.
    let mut m = hl2_menu(&[("chimera_server_type", "3")], None);
    create_server(&mut m);
    assert_eq!(m.new_game.visibility, 0, "this opener's closure has no such cvar");
    m.open(Page::NewGame, vec!["a".into()], None, |n| (n == SERVER_TYPE_CVAR).then(|| "3".to_string()));
    assert_eq!(m.new_game.visibility, 3);
    let lines = m.visibility_lines();
    assert_eq!(lines[0], "sv_lan 0");
    assert!(lines.contains(&"sv_friends_only 1".to_string()));
}

/// Case 14: ten rows, the password once, at position 2.
#[test]
fn the_game_tab_has_ten_rows() {
    let mut m = hl2_menu(&[], None);
    create_server(&mut m);
    m.set_create_tab(1);
    let labels: Vec<String> = m
        .rows()
        .iter()
        .filter_map(|r| match r {
            Row::Control { label, .. } => Some(label.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(labels.len(), 10, "{labels:?}");
    assert_eq!(labels[1], "Server password");
    assert_eq!(labels.iter().filter(|l| *l == "Server password").count(), 1);
}

/// Case 15 and the row formats: 20 players and Teamplay on, then the map.
#[test]
fn the_game_rows_become_commands_in_the_scripts_formats() {
    let mut m = hl2_menu(&[], None);
    create_server(&mut m);
    m.set_create_tab(1);
    let max = m.new_game.cvars.iter().position(|c| c.cvar == "maxplayers").unwrap();
    m.new_game.cvars[max].value = "20".into();
    let limit = m.new_game.cvars.iter().position(|c| c.cvar == "mp_timelimit").unwrap();
    m.new_game.cvars[limit].value = "30".into();
    click_on(&mut m, |m| row_with(m, Field::ServerCvar(m.new_game.cvars.iter().position(|c| c.cvar == "mp_teamplay").unwrap())));
    let out = click_on(&mut m, |m| button(m, Action::Start));
    assert!(out.lines.contains(&"mp_teamplay \"1\"".to_string()), "{:?}", out.lines);
    assert!(out.lines.contains(&"mp_timelimit \"30.000000\"".to_string()), "{:?}", out.lines);
    assert!(out.lines.contains(&"maxplayers 20".to_string()));
    let (tp, mp, map) = (
        out.lines.iter().position(|l| l.starts_with("mp_teamplay")).unwrap(),
        out.lines.iter().position(|l| l == "maxplayers 20").unwrap(),
        out.lines.iter().position(|l| l.starts_with("map ")).unwrap(),
    );
    assert!(tp < map && mp < map);
}

#[test]
fn row_formats_by_kind() {
    let c = |kind, cvar: &str| ServerCvar {
        cvar: cvar.into(),
        label: String::new(),
        kind,
        value: String::new(),
        before: None,
        page: 1,
        field: None,
    };
    assert_eq!(scr_line(&c(ServerSettingKind::Bool, "a"), "1"), "a \"1\"");
    assert_eq!(scr_line(&c(ServerSettingKind::Bool, "a"), "0"), "a \"0\"");
    let n = c(ServerSettingKind::Number { min: Some(1.0), max: Some(32.0) }, "maxplayers");
    assert_eq!(scr_line(&n, "20"), "maxplayers \"20.000000\"");
    assert_eq!(scr_line(&n, "99"), "maxplayers \"32.000000\"");
    assert_eq!(scr_line(&n, "0"), "maxplayers \"1.000000\"");
    assert_eq!(scr_line(&c(ServerSettingKind::Text, "hostname"), "My \"best\"; server"), "hostname \"My best server\"");
    let l = c(ServerSettingKind::List(vec![("A".into(), "5".into()), ("B".into(), "7".into())]), "x");
    assert_eq!(scr_line(&l, "7"), "x \"7\"");
    assert_eq!(scr_line(&l, "9"), "x \"0\"", "no item matches");
}
