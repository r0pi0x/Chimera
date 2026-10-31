//! The party apps' main menu (feature `present`; docs/plans/active/
//! main-menu.md M1): Ultimate Gun Game starts at it unless the command
//! line plays (`starts_at_menu`: `--map`, `+minigame`, `+connect`,
//! `--screenshot` and the other tool runs skip it exactly as before);
//! Lucker Party (shelved) still starts in its minigame and has none.
//!
//! The first iteration copies CS:S's main menu (`cs_source::ui::
//! game_menu`), drawn with the same VGUI widgets (`chimera_client::
//! client::widgets`) in the GameUI look of CS:S's install (its scheme's
//! colours and fonts and its title font, read on a thread at start:
//! `cs_source::game::gameui::load`; built-in colours without it): the
//! entries at the left, a fixed distance up from the screen's foot, the
//! app's name over them as text (no logo; in the scheme's menu font,
//! CS:S's title font being its logo's letters only), the build's version in the
//! bottom-right corner; each page a movable VGUI frame. Its code is the
//! party's own (CS:S's menu is that game's front end and holds CS:S's
//! Create Server, options and server browser); what it shares with it is
//! Chimera's (`widgets`, `dialog`, the GameUI reader).
//!
//! - **Entries** (data/main_menu.toml `[[entry]]`): Play, Join, Options,
//!   Quit; each a page or a console line.
//! - **Play** (Create Server's place, its Server and Game tabs): the Server
//!   tab's rows are data (`[[play]]`: map, bots, max. players, server name,
//!   password, port, UPnP), the Game tab host_settings.toml's next-match
//!   rows (`host::settings` with a `key`). Start sets the rows' cvars and
//!   loads the map (`map`, a listen server when `maxplayers` > 1: Play
//!   is joinable by default, bots fill `MaxPlayer`), then plays the app's
//!   minigame (`MenuMinigame`) once the map is in.
//! - **Join**: an address (host:port, the port 27015 unless given) and a
//!   password, Connect (`password ..; connect ..`), the last addresses as
//!   buttons (`RecentFile`: `party_recent.txt` in the config folder).
//! - **Options**: the Escape menu's tabs (data/options.toml) and Binds
//!   (the Controls page).
//! - **Loading** (`Screen::Loading`, `Screen::Joining`): CS:S's loading
//!   dialog's place and parts (its stage line from `JoinProgress`'s stage,
//!   the map's load or the download's bytes, a bar, Cancel =
//!   `disconnect`), also for a map change in a game (a rotation, the
//!   server's level change); Combat Arms' own loading screen instead for
//!   its maps (user decision 5: `combat_arms::ui::loading`, its map
//!   picture), gated here (`ca_loading_gate`).
//! - **The failure box**: a failed or refused join, a drop, a failed load
//!   land on the menu with why (CS:S's "Disconnected" frame; the server's
//!   or the game's words, `failure_text`), Close.
//! - **Background**: a live map (`[background]`: the first that loads,
//!   the app's own map load at start; again after leaving a game) with
//!   the camera on a slow orbit around the middle of its spawn points
//!   (`chimera_freecam` 2's detached camera, `fly`); the HUD is hidden
//!   while the menu shows by drawing the UI on a camera of its own
//!   (`MenuUiCamera`; the shell's UI camera, which the HUD draws on, off).
//!
//! The state is `PartyMenu` (a resource the menu owns, set at known
//! moments: the front end's events `EnteredGame`, `LeftGame`, `Joining`,
//! `JoinFailed`, `MapLoadFailed`, observed here); clicks and tests send
//! `MainMenuAction`s, which one exclusive system turns into console lines.
//! Drawing is Bevy UI rebuilt when the state differs. `mainmenu [page]`,
//! `mainmenu_type <field> <text>`, `mainmenu_press <button>` and
//! `mainmenu_failure <text>` (console) drive it for screenshots.
//!
//! - **Music**: Combat Arms' menu playlist (the install's BGM) while the
//!   menu shows (`combat_arms::game::music::AppMenuShown`), silent
//!   without the install.
//!
//! Known shortcuts: steppers (`<` `>`) in place of CS:S's combo boxes and
//! check boxes; no server browser (M2); bots don't make room for people
//! joining (`MaxPlayer` is a fixed count); the background map after
//! leaving a game keeps the last minigame's rules (no bots on it); the
//! console shows over the menu but egui's debug windows don't; the
//! camera's orbit can see past the map's edges (unlit black) on some
//! maps.

use std::collections::BTreeMap;

use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
};
use chimera_client::client::{
    console::ConsoleRoot,
    fonts::UiFonts,
    front_end as fe,
    widgets::{self, ButtonState as Btn, Caret, Edit, FrameSpec, Look, label, place},
};
use chimera_core::{
    console::{Console, ConsoleAppExt},
    core::{LocalPlayer, NetRole, SpawnPoint},
};
use chimera_net::net::client::{JoinFailure, JoinProgress, JoinStage};
use chimera_world::map::hud::GameUi;

use crate::host::{self, Pick, Setting, setting_of};

/// The menu's data (data/main_menu.toml).
pub const MAIN_MENU_TOML: &str = include_str!("../data/main_menu.toml");

/// Why a host's clients were dropped when it left (`net::client::
/// HOST_LEFT`'s words, shown in their failure box).
pub use chimera_net::net::client::HOST_LEFT;

// ---------------------------------------------------------------------------
// The data.

/// What a main menu entry does.
#[derive(Clone, Debug, PartialEq)]
pub enum EntryDoes {
    Page(MenuPage),
    /// A console line.
    Command(String),
}

/// A main menu entry.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub label: String,
    pub does: EntryDoes,
    /// Half a row before it.
    pub gap: bool,
}

/// What an Escape menu entry does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EscDoes {
    Resume,
    Options,
    Host,
    Vote,
    Disconnect,
    Quit,
}

/// When an Escape menu entry shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum When {
    Always,
    /// Not a network client.
    Host,
    /// The app has this main menu (`PartyMenu` is in).
    MainMenu,
}

/// An Escape menu entry.
#[derive(Clone, Debug, PartialEq)]
pub struct EscEntry {
    pub label: String,
    pub does: EscDoes,
    pub when: When,
}

/// How a Play row is set.
#[derive(Clone, Debug, PartialEq)]
pub enum PlayKind {
    /// "Rotation" or one map of the minigame's pool.
    Map,
    Pick(Pick),
    /// Typed: its byte limit, shown as stars.
    Text {
        max: usize,
        masked: bool,
    },
}

/// A row of the Play page's Server tab.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayRow {
    pub label: String,
    /// The cvar it sets (empty: the map row).
    pub cvar: String,
    pub kind: PlayKind,
    /// Its value when the page first opens (else the cvar's).
    pub default: Option<String>,
}

/// The live map behind the menu and its camera's orbit.
#[derive(Clone, Debug, PartialEq)]
pub struct Background {
    pub maps: Vec<String>,
    pub height: f32,
    pub radius: f32,
    pub period: f32,
    pub pitch: f32,
}

/// data/main_menu.toml read.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct MenuData {
    pub entries: Vec<Entry>,
    pub escape: Vec<EscEntry>,
    pub play: Vec<PlayRow>,
    pub background: Background,
}

impl Default for MenuData {
    fn default() -> Self {
        Self::shipped()
    }
}

impl MenuData {
    /// The shipped file's.
    pub fn shipped() -> Self {
        Self::parse(MAIN_MENU_TOML).expect("data/main_menu.toml reads")
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let doc: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
        let tables = |k: &str| -> Vec<toml::Table> {
            doc.get(k)
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_table().cloned()).collect())
                .unwrap_or_default()
        };
        let s = |t: &toml::Table, k: &str| t.get(k).and_then(|v| v.as_str()).map(str::to_string);
        let mut entries = Vec::new();
        for t in tables("entry") {
            let label = s(&t, "label").ok_or("an entry without a label")?;
            let does = match (s(&t, "page"), s(&t, "command")) {
                (Some(p), None) => EntryDoes::Page(MenuPage::named(&p).ok_or_else(|| format!("{label}: no page {p}"))?),
                (None, Some(c)) => EntryDoes::Command(c),
                _ => return Err(format!("{label}: a page or a command")),
            };
            entries.push(Entry {
                label,
                does,
                gap: t.get("gap").and_then(|v| v.as_bool()).unwrap_or(false),
            });
        }
        let mut escape = Vec::new();
        for t in tables("escape") {
            let label = s(&t, "label").ok_or("an Escape entry without a label")?;
            let does = match s(&t, "action").as_deref() {
                Some("resume") => EscDoes::Resume,
                Some("options") => EscDoes::Options,
                Some("host") => EscDoes::Host,
                Some("vote") => EscDoes::Vote,
                Some("disconnect") => EscDoes::Disconnect,
                Some("quit") => EscDoes::Quit,
                other => return Err(format!("{label}: no action {other:?}")),
            };
            let when = match s(&t, "when").as_deref() {
                None | Some("always") => When::Always,
                Some("host") => When::Host,
                Some("main_menu") => When::MainMenu,
                Some(w) => return Err(format!("{label}: no `when` {w}")),
            };
            escape.push(EscEntry { label, does, when });
        }
        let mut play = Vec::new();
        for t in tables("play") {
            let label = s(&t, "label").ok_or("a play row without a label")?;
            let default = s(&t, "default");
            let row = if t.get("map").and_then(|v| v.as_bool()) == Some(true) {
                PlayRow {
                    label,
                    cvar: String::new(),
                    kind: PlayKind::Map,
                    default,
                }
            } else if let Some(max) = t.get("text").and_then(|v| v.as_integer()) {
                PlayRow {
                    cvar: s(&t, "cvar").ok_or_else(|| format!("{label}: no cvar"))?,
                    label,
                    kind: PlayKind::Text {
                        max: max.max(1) as usize,
                        masked: t.get("masked").and_then(|v| v.as_bool()).unwrap_or(false),
                    },
                    default,
                }
            } else {
                let setting = setting_of(&toml::Value::Table(t.clone()))?;
                PlayRow {
                    label,
                    cvar: setting.cvar,
                    kind: PlayKind::Pick(setting.pick),
                    default,
                }
            };
            play.push(row);
        }
        let bg = doc
            .get("background")
            .and_then(|v| v.as_table())
            .ok_or("no [background]")?;
        let num = |k: &str, d: f32| {
            bg.get(k)
                .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
                .map_or(d, |f| f as f32)
        };
        let background = Background {
            maps: bg
                .get("maps")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                .unwrap_or_default(),
            height: num("height", 15.0),
            radius: num("radius", 25.0),
            period: num("period", 150.0).max(1.0),
            pitch: num("pitch", 20.0),
        };
        Ok(Self {
            entries,
            escape,
            play,
            background,
        })
    }
}

/// Whether the run starts at the main menu: nothing on the command line
/// plays (`Args::starts_in_game`: a map, a placed player), joins
/// (`+connect`), names a minigame (`+minigame`) or captures and exits (a
/// tool run: `--screenshot`, `--views`, `--frames`, `--bench`).
pub fn starts_at_menu(args: &chimera_client::client::Args) -> bool {
    !(args.starts_in_game()
        || args.tool_run()
        || crate::minigame_of(&args.console).is_some()
        || crate::joins_server(&args.console))
}

/// The background map: the first of `[background]`'s maps that `load`
/// takes (its id and data), else None.
pub fn background_map<T>(data: &MenuData, load: impl Fn(&str) -> Result<T, String>) -> Option<(String, T)> {
    data.background
        .maps
        .iter()
        .find_map(|m| load(m).ok().map(|d| (m.clone(), d)))
}

// ---------------------------------------------------------------------------
// The state.

/// What the screen shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Screen {
    /// Out of a game: the menu, over the live map.
    #[default]
    MainMenu,
    /// Joining a server (or following its map change).
    Joining,
    /// A map loading (Play's, a host's map change).
    Loading,
    /// Playing: the menu is shut (the Escape menu is the party's).
    InGame,
}

/// The menu's pages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MenuPage {
    #[default]
    Main,
    Play,
    Join,
    Options,
}

impl MenuPage {
    pub fn named(name: &str) -> Option<Self> {
        Some(match name.to_ascii_lowercase().as_str() {
            "main" => Self::Main,
            "play" => Self::Play,
            "join" => Self::Join,
            "options" => Self::Options,
            _ => return None,
        })
    }

    fn window(self) -> &'static str {
        match self {
            Self::Main => "partymain",
            Self::Play => "partyplay",
            Self::Join => "partyjoin",
            Self::Options => "partyoptions",
        }
    }
}

/// A text field of the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    /// A Play row (its index in `MenuData::play`).
    Play(usize),
    Address,
    Password,
}

/// The menu's state.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct PartyMenu {
    pub screen: Screen,
    pub page: MenuPage,
    /// The Play page's tab: 0 Server, 1 Game.
    pub play_tab: usize,
    /// The Options page's tab.
    pub options_tab: usize,
    /// The Play rows' values (by row; the map row's: "" is the rotation).
    pub play: Vec<String>,
    pub address: String,
    pub password: String,
    /// The text field typed into, and its caret.
    pub focus: Option<Field>,
    pub caret: Caret,
    /// Why the last game ended or a join failed (the failure box).
    pub failure: Option<String>,
    /// What is loading: a map, or the server's address.
    pub loading: Option<String>,
    /// The last addresses joined, newest first.
    pub recent: Vec<String>,
    /// The shown settings' values as the cvars hold them (options and the
    /// Game tab).
    pub values: BTreeMap<String, String>,
    /// Console lines for when the map now loading is in (Play's minigame).
    pub after_load: Vec<String>,
    /// The map loading is the menu's background (not a game).
    pub background_load: bool,
    /// The background map is to load again (after leaving a game).
    pub reload_background: bool,
    /// An options value changed (the config is written on leaving).
    pub dirty: bool,
}

impl Default for PartyMenu {
    fn default() -> Self {
        Self {
            screen: Screen::MainMenu,
            page: MenuPage::Main,
            play_tab: 0,
            options_tab: 0,
            play: Vec::new(),
            address: String::new(),
            password: String::new(),
            focus: None,
            caret: Caret::default(),
            failure: None,
            loading: None,
            recent: Vec::new(),
            values: BTreeMap::new(),
            after_load: Vec::new(),
            background_load: false,
            reload_background: false,
            dirty: false,
        }
    }
}

impl PartyMenu {
    /// The menu covers the screen (anything but playing).
    pub fn shown(&self) -> bool {
        self.screen != Screen::InGame
    }

    fn text_mut(&mut self, field: Field) -> Option<&mut String> {
        match field {
            Field::Address => Some(&mut self.address),
            Field::Password => Some(&mut self.password),
            Field::Play(i) => self.play.get_mut(i),
        }
    }

    /// Back to the menu's first page (out of a game).
    fn to_menu(&mut self) {
        self.screen = Screen::MainMenu;
        self.page = MenuPage::Main;
        self.loading = None;
        self.focus = None;
        self.after_load.clear();
    }
}

/// The minigame Play starts (the app's: Ultimate Gun Game's automatic one).
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct MenuMinigame(pub String);

/// Where the recent addresses are kept (None: not kept; tests).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct RecentFile(pub Option<std::path::PathBuf>);

/// How many recent addresses are kept.
pub const RECENT: usize = 5;

impl RecentFile {
    /// `party_recent.txt` in the config folder (`console::cfg_dir`).
    pub fn in_config() -> Self {
        Self(chimera_core::console::cfg_dir().map(|d| d.join("party_recent.txt")))
    }

    pub fn read(&self) -> Vec<String> {
        let Some(p) = &self.0 else { return Vec::new() };
        std::fs::read_to_string(p)
            .map(|t| {
                t.lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .take(RECENT)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn write(&self, recent: &[String]) {
        if let Some(p) = &self.0 {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Err(e) = std::fs::write(p, recent.join("\n") + "\n") {
                warn!("main menu: writing {}: {e}", p.display());
            }
        }
    }
}

/// `address` first in `recent`, once, at most `RECENT`.
pub fn remember(recent: &mut Vec<String>, address: &str) {
    let a = address.trim().to_string();
    recent.retain(|r| !r.eq_ignore_ascii_case(&a));
    recent.insert(0, a);
    recent.truncate(RECENT);
}

/// What a click on the menu does (tests and `mainmenu` send them too).
#[derive(Component, Message, Clone, Copy, Debug, PartialEq)]
pub enum MainMenuAction {
    /// A main entry (its index in `MenuData::entries`).
    Entry(usize),
    /// Back to the first page (a frame's close box, Cancel).
    Back,
    PlayTab(usize),
    /// A Play row stepped (Server tab: its index; `dir` -1 or 1).
    PlayStep {
        row: usize,
        dir: i8,
    },
    /// A Game tab row stepped (`host::settings()`'s index).
    GameStep {
        index: usize,
        dir: i8,
    },
    /// Start the game (Play).
    Start,
    /// A text field takes the keys.
    Focus(Field),
    /// Join's Connect.
    Connect,
    /// Join a recent address.
    Recent(usize),
    OptionsTab(usize),
    OptionStep {
        index: usize,
        dir: i8,
    },
    Binds,
    /// Cancel the load or the join (`disconnect`).
    Cancel,
    /// Close the failure box.
    CloseFailure,
    Quit,
}

// ---------------------------------------------------------------------------
// The plugin.

/// The main menu. `at_start`: the run starts at it (`starts_at_menu`);
/// else it is in the game and the menu opens on leaving it.
pub struct MainMenuPlugin {
    pub at_start: bool,
    /// The minigame Play starts.
    pub minigame: String,
    /// Keep the recent addresses in the config folder (the app; not
    /// tests).
    pub keep_recent: bool,
}

/// Its systems (in `Update`, before the Escape menu's).
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct MainMenuSystems;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        let data = MenuData::shipped();
        let recent = if self.keep_recent {
            RecentFile::in_config()
        } else {
            RecentFile::default()
        };
        let menu = PartyMenu {
            screen: if self.at_start {
                Screen::MainMenu
            } else {
                Screen::InGame
            },
            play: vec![String::new(); data.play.len()],
            recent: recent.read(),
            ..default()
        };
        app.insert_resource(data)
            .insert_resource(menu)
            .insert_resource(recent)
            .insert_resource(MenuMinigame(self.minigame.clone()))
            .init_resource::<MenuLook>()
            .add_message::<MainMenuAction>()
            .add_message::<KeyboardInput>()
            .add_observer(entered_game)
            .add_observer(left_game)
            .add_observer(joining)
            .add_observer(join_failed)
            .add_observer(map_load_failed)
            .add_systems(Startup, (start_look, init_play).chain())
            .add_systems(
                Update,
                (look_loaded, typing, clicks, act, follow_loads, refresh_values, ca_loading_gate, fly, cameras, show, progress, entry_hover, music)
                    .chain()
                    .in_set(MainMenuSystems)
                    .before(crate::esc_menu::EscMenuSystems),
            )
            .console_command(
                "mainmenu",
                "mainmenu [main|play|join|options]: the main menu's page (out of a game); screenshots and tests.",
                |w, a| {
                    let page = match a.first() {
                        Some(p) => MenuPage::named(p).ok_or_else(|| format!("no page {p}"))?,
                        None => MenuPage::Main,
                    };
                    let mut m = w.resource_mut::<PartyMenu>();
                    if !m.shown() {
                        return Err("in a game: Disconnect first".into());
                    }
                    m.failure = None;
                    m.page = page;
                    m.focus = match page {
                        MenuPage::Join => Some(Field::Address),
                        _ => None,
                    };
                    Ok(None)
                },
            )
            .console_command(
                "mainmenu_type",
                "mainmenu_type <field> <text>: type into the main menu's field (address, password, or a Play row's \
                 number); screenshots and tests.",
                |w, a| {
                    let field = match a.first().map(String::as_str) {
                        Some("address") => Field::Address,
                        Some("password") => Field::Password,
                        Some(n) => Field::Play(n.parse().map_err(|_| format!("no field {n}"))?),
                        None => return Err("mainmenu_type <field> <text>".into()),
                    };
                    let text = a[1..].join(" ");
                    let mut m = w.resource_mut::<PartyMenu>();
                    let t = m.text_mut(field).ok_or("no such field")?;
                    *t = text;
                    let caret = Caret::end_of(t);
                    m.focus = Some(field);
                    m.caret = caret;
                    Ok(None)
                },
            )
            .console_command(
                "mainmenu_press",
                "mainmenu_press <start|connect|cancel|close|back|quit>: press that button of the main menu (screenshots \
                 and tests).",
                |w, a| {
                    let action = match a.first().map(String::as_str) {
                        Some("start") => MainMenuAction::Start,
                        Some("connect") => MainMenuAction::Connect,
                        Some("cancel") => MainMenuAction::Cancel,
                        Some("close") => MainMenuAction::CloseFailure,
                        Some("back") => MainMenuAction::Back,
                        Some("quit") => MainMenuAction::Quit,
                        other => return Err(format!("no button {other:?}")),
                    };
                    w.write_message(action);
                    Ok(None)
                },
            )
            .console_command(
                "mainmenu_failure",
                "mainmenu_failure <text>: show the failure box with that text (screenshots).",
                |w, a| {
                    let mut m = w.resource_mut::<PartyMenu>();
                    m.to_menu();
                    m.failure = Some(a.join(" "));
                    Ok(None)
                },
            );
    }
}

/// The Play rows' first values: the data's defaults, else the cvars'.
fn init_play(world: &mut World) {
    let rows = world.resource::<MenuData>().play.clone();
    let title = world
        .get_resource::<crate::esc_menu::EscMenuTitle>()
        .map_or_else(String::new, |t| t.0.clone());
    let values: Vec<String> = rows
        .iter()
        .map(|r| match (&r.default, r.kind == PlayKind::Map) {
            (_, true) => String::new(),
            (Some(d), _) => d.replace("{title}", &title),
            (None, _) => crate::controls::cvar_value(world, &r.cvar).unwrap_or_default(),
        })
        .collect();
    world.resource_mut::<PartyMenu>().play = values;
}

// ---------------------------------------------------------------------------
// The front end's events.

fn entered_game(_: On<fe::EnteredGame>, mut menu: ResMut<PartyMenu>, mut console: Option<ResMut<Console>>) {
    if std::mem::take(&mut menu.background_load) {
        menu.loading = None;
        if menu.screen == Screen::Loading {
            menu.screen = Screen::MainMenu;
        }
        return;
    }
    menu.screen = Screen::InGame;
    menu.loading = None;
    menu.failure = None;
    menu.focus = None;
    let lines = std::mem::take(&mut menu.after_load);
    if let Some(c) = console.as_deref_mut() {
        for l in lines {
            c.submit(l);
        }
    }
}

fn left_game(_: On<fe::LeftGame>, mut menu: ResMut<PartyMenu>) {
    menu.to_menu();
    menu.background_load = false;
    menu.reload_background = true;
}

fn joining(j: On<fe::Joining>, mut menu: ResMut<PartyMenu>) {
    menu.screen = Screen::Joining;
    menu.page = MenuPage::Main;
    menu.focus = None;
    menu.failure = None;
    menu.background_load = false;
    menu.loading = Some(j.label.clone());
}

fn join_failed(
    _: On<fe::JoinFailed>,
    mut menu: ResMut<PartyMenu>,
    join: Option<Res<JoinProgress>>,
    look: Res<MenuLook>,
) {
    let Some((kind, reason)) = join.and_then(|j| j.failure.clone()) else {
        return;
    };
    menu.to_menu();
    menu.failure = Some(failure_text(look.ui.as_deref(), &kind, &reason));
}

fn map_load_failed(_: On<fe::MapLoadFailed>, mut menu: ResMut<PartyMenu>) {
    if std::mem::take(&mut menu.background_load) {
        return;
    }
    if menu.screen == Screen::Loading {
        let map = menu.loading.clone().unwrap_or_default();
        menu.to_menu();
        menu.failure = Some(format!("Couldn't load the map {map}."));
    }
}

/// Why joining (or the game) failed, as CS:S words it (its GameUI
/// strings, `%s1` filled in), else the server's or our own words.
pub fn failure_text(ui: Option<&GameUi>, kind: &JoinFailure, reason: &str) -> String {
    let game = |token: &str| {
        ui.and_then(|u| u.strings.get(&token.trim_start_matches('#').to_lowercase()))
            .map(|t| t.trim_end().to_string())
    };
    let text = match kind {
        JoinFailure::Full => game("#GameUI_ServerRejectServerFull"),
        JoinFailure::OldServer => game("#GameUI_ServerRejectOldVersion"),
        JoinFailure::NewServer => game("#GameUI_ServerRejectNewVersion"),
        JoinFailure::Timeout => game("#GameUI_ServerConnectionTimeout"),
        JoinFailure::Download { file, error } => {
            game("#GameUI_DownloadFailed").map(|t| format!("{}:\n{error}", t.replace("%s1", file)))
        }
        _ => None,
    };
    text.unwrap_or_else(|| reason.to_string())
}

// ---------------------------------------------------------------------------
// The look.

/// The GameUI look of CS:S's install, read on a thread at start.
#[derive(Resource, Default)]
pub struct MenuLook {
    pub ui: Option<std::sync::Arc<GameUi>>,
    loading: Option<std::sync::Arc<std::sync::Mutex<Option<Option<GameUi>>>>>,
}

fn start_look(mut look: ResMut<MenuLook>) {
    let slot = std::sync::Arc::new(std::sync::Mutex::new(None));
    look.loading = Some(slot.clone());
    std::thread::spawn(move || {
        let loaded = cs_source::game::INFO
            .install()
            .and_then(|p| cs_source::game::mount::open(&p).ok())
            .and_then(|m| cs_source::game::gameui::load(&m));
        if let Ok(mut s) = slot.lock() {
            *s = Some(loaded);
        }
    });
}

fn look_loaded(mut look: ResMut<MenuLook>, mut ui_fonts: Option<ResMut<UiFonts>>) {
    let Some(slot) = look.loading.clone() else { return };
    let Some(loaded) = slot.lock().ok().and_then(|mut s| s.take()) else {
        return;
    };
    look.loading = None;
    let Some(ui) = loaded else {
        info!("main menu: no CS:S GameUI files, built-in look");
        return;
    };
    if let Some(f) = ui_fonts.as_deref_mut() {
        f.set_source(&ui);
    }
    look.ui = Some(std::sync::Arc::new(ui));
}

// ---------------------------------------------------------------------------
// Input.

/// Keys: typing into the focused field, Tab to the next, Enter to its
/// page's button, Esc back to the first page (or closes the failure box).
#[allow(clippy::too_many_arguments)]
fn typing(
    mut events: MessageReader<KeyboardInput>,
    held: Option<Res<ButtonInput<KeyCode>>>,
    console: Option<Res<chimera_client::client::console::ConsoleUi>>,
    controls: Option<Res<crate::controls::ControlsPage>>,
    mut menu: ResMut<PartyMenu>,
    data: Res<MenuData>,
    mut out: MessageWriter<MainMenuAction>,
) {
    let live = menu.shown() && !console.is_some_and(|c| c.open) && !controls.is_some_and(|c| c.open);
    if !live {
        events.clear();
        return;
    }
    let held = |k: [KeyCode; 2]| held.as_ref().is_some_and(|h| h.any_pressed(k));
    let shift = held([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let ctrl = held([KeyCode::ControlLeft, KeyCode::ControlRight]);
    for e in events.read() {
        if e.state != ButtonState::Pressed {
            continue;
        }
        if e.key_code == KeyCode::Escape {
            if menu.failure.is_some() {
                out.write(MainMenuAction::CloseFailure);
            } else if matches!(menu.screen, Screen::Loading | Screen::Joining) {
                out.write(MainMenuAction::Cancel);
            } else if menu.page != MenuPage::Main {
                out.write(MainMenuAction::Back);
            }
            continue;
        }
        if e.key_code == KeyCode::Enter || e.key_code == KeyCode::NumpadEnter {
            match menu.page {
                _ if menu.failure.is_some() => out.write(MainMenuAction::CloseFailure),
                MenuPage::Join => out.write(MainMenuAction::Connect),
                MenuPage::Play => out.write(MainMenuAction::Start),
                _ => continue,
            };
            continue;
        }
        let Some(field) = menu.focus else { continue };
        if e.key_code == KeyCode::Tab {
            let order = fields(&menu, &data);
            if let Some(next) = widgets::focus_step(&order, Some(field), if shift { -1 } else { 1 }) {
                let caret = Caret::end_of(menu.text_mut(next).map_or("", |s| s.as_str()));
                menu.focus = Some(next);
                menu.caret = caret;
            }
            continue;
        }
        let edit = match (&e.logical_key, e.key_code) {
            (_, KeyCode::KeyA) if ctrl => Edit::SelectAll,
            (_, KeyCode::ArrowLeft) => Edit::Left(shift),
            (_, KeyCode::ArrowRight) => Edit::Right(shift),
            (_, KeyCode::Home) => Edit::Home(shift),
            (_, KeyCode::End) => Edit::End(shift),
            (_, KeyCode::Delete) => Edit::Delete,
            (Key::Backspace, _) => Edit::Backspace,
            (Key::Space, _) => Edit::Insert(" ".into()),
            (Key::Character(s), _) if !ctrl => Edit::Insert(s.to_string()),
            _ => continue,
        };
        let max = match field {
            Field::Play(i) => match data.play.get(i).map(|r| &r.kind) {
                Some(PlayKind::Text { max, .. }) => *max,
                _ => 64,
            },
            Field::Address => 128,
            Field::Password => 64,
        };
        let mut caret = menu.caret;
        if let Some(text) = menu.text_mut(field) {
            caret.edit(text, edit, max);
        }
        menu.caret = caret;
    }
}

/// The text fields of the page shown, in Tab order.
fn fields(menu: &PartyMenu, data: &MenuData) -> Vec<Field> {
    match menu.page {
        MenuPage::Join => vec![Field::Address, Field::Password],
        MenuPage::Play if menu.play_tab == 0 => data
            .play
            .iter()
            .enumerate()
            .filter(|(_, r)| matches!(r.kind, PlayKind::Text { .. }))
            .map(|(i, _)| Field::Play(i))
            .collect(),
        _ => Vec::new(),
    }
}

/// Clicks on the menu's buttons as actions.
fn clicks(
    pressed: Query<(&widgets::Interaction, &MainMenuAction), Changed<widgets::Interaction>>,
    mut out: MessageWriter<MainMenuAction>,
) {
    for (i, a) in &pressed {
        if *i == widgets::Interaction::Pressed {
            out.write(*a);
        }
    }
}

fn submit(world: &mut World, line: impl Into<String>) {
    world.resource_mut::<Console>().submit(line.into());
}

/// What the actions do.
fn act(world: &mut World) {
    let actions: Vec<MainMenuAction> = world.resource_mut::<Messages<MainMenuAction>>().drain().collect();
    for a in actions {
        act_one(world, a);
    }
}

fn leave_page(world: &mut World) {
    if std::mem::take(&mut world.resource_mut::<PartyMenu>().dirty) {
        submit(world, "host_writeconfig");
    }
}

fn act_one(world: &mut World, action: MainMenuAction) {
    use MainMenuAction as A;
    let menu = world.resource::<PartyMenu>().clone();
    if !menu.shown() {
        return;
    }
    let loading = matches!(menu.screen, Screen::Loading | Screen::Joining);
    match action {
        A::Cancel => {
            if loading {
                submit(world, "disconnect");
            }
        }
        A::CloseFailure => {
            world.resource_mut::<PartyMenu>().failure = None;
            if let Some(mut j) = world.get_resource_mut::<JoinProgress>() {
                j.dismiss();
            }
        }
        _ if loading || menu.failure.is_some() => {}
        A::Entry(i) => {
            let Some(e) = world.resource::<MenuData>().entries.get(i).cloned() else {
                return;
            };
            leave_page(world);
            match e.does {
                EntryDoes::Page(p) => {
                    let mut m = world.resource_mut::<PartyMenu>();
                    m.page = p;
                    m.focus = (p == MenuPage::Join).then_some(Field::Address);
                    m.caret = Caret::end_of(&m.address.clone());
                    if let Some(mut w) = world.get_resource_mut::<widgets::Windows>() {
                        w.raise(p.window());
                    }
                }
                EntryDoes::Command(c) => submit(world, c),
            }
        }
        A::Back => {
            leave_page(world);
            let mut m = world.resource_mut::<PartyMenu>();
            m.page = MenuPage::Main;
            m.focus = None;
        }
        A::PlayTab(t) => {
            let mut m = world.resource_mut::<PartyMenu>();
            m.play_tab = t.min(1);
            m.focus = None;
        }
        A::PlayStep { row, dir } => {
            let pool = map_pool(world);
            let rows = world.resource::<MenuData>().play.clone();
            let Some(r) = rows.get(row) else { return };
            let mut m = world.resource_mut::<PartyMenu>();
            let now = m.play.get(row).cloned().unwrap_or_default();
            let next = match &r.kind {
                PlayKind::Map => {
                    let mut choices = vec![String::new()];
                    choices.extend(pool);
                    let at = choices.iter().position(|c| *c == now).unwrap_or(0) as i64;
                    choices[(at + dir as i64).rem_euclid(choices.len() as i64) as usize].clone()
                }
                PlayKind::Pick(p) => p.next(&now, dir),
                PlayKind::Text { .. } => return,
            };
            if let Some(v) = m.play.get_mut(row) {
                *v = next;
            }
        }
        A::GameStep { index, dir } => {
            let Some(s) = game_settings().get(index).cloned() else {
                return;
            };
            step_cvar(world, &s, dir);
        }
        A::Start => start(world),
        A::Focus(f) => {
            let mut m = world.resource_mut::<PartyMenu>();
            let caret = Caret::end_of(m.text_mut(f).map_or("", |s| s.as_str()));
            m.focus = Some(f);
            m.caret = caret;
        }
        A::Connect => {
            let (address, password) = (menu.address.trim().to_string(), menu.password.clone());
            connect(world, &address, &password);
        }
        A::Recent(i) => {
            if let Some(a) = menu.recent.get(i).cloned() {
                world.resource_mut::<PartyMenu>().address = a.clone();
                connect(world, &a, &menu.password);
            }
        }
        A::OptionsTab(t) => {
            leave_page(world);
            world.resource_mut::<PartyMenu>().options_tab = t;
        }
        A::OptionStep { index, dir } => {
            let tabs = crate::esc_menu::option_tabs();
            let Some(s) = tabs.get(menu.options_tab).and_then(|(_, s)| s.get(index)).cloned() else {
                return;
            };
            step_cvar(world, &s, dir);
            world.resource_mut::<PartyMenu>().dirty = true;
        }
        A::Binds => {
            if let Some(mut c) = world.get_resource_mut::<crate::controls::ControlsPage>() {
                c.open = true;
                c.capture = None;
            }
        }
        A::Quit => submit(world, "quit"),
    }
}

/// A setting's cvar moved a step.
fn step_cvar(world: &mut World, s: &Setting, dir: i8) {
    let now = crate::controls::cvar_value(world, &s.cvar).unwrap_or_default();
    let next = s.pick.next(&now, dir);
    let quoted = if next.is_empty() {
        "\"\"".to_string()
    } else {
        chimera_core::console::quote(&next)
    };
    submit(world, format!("{} {quoted}", s.cvar));
}

/// The Game tab's rows: the host settings that apply from the next match
/// (those with a `key`).
pub fn game_settings() -> Vec<Setting> {
    host::settings().into_iter().filter(|s| s.key.is_some()).collect()
}

/// The maps of the app's minigame's pool.
pub fn map_pool(world: &World) -> Vec<String> {
    let id = world
        .get_resource::<MenuMinigame>()
        .map(|m| m.0.clone())
        .unwrap_or_default();
    world
        .get_resource::<chimera_sim::minigame::Minigames>()
        .and_then(|m| m.get(&id))
        .map(|d| d.maps.iter().map(|m| chimera_net::net::normalize_map(m)).collect())
        .unwrap_or_default()
}

/// The console lines Play's Start submits (its rows' cvars, then the map)
/// and those for once the map is in (the minigame), for `menu`'s values.
pub fn start_lines(
    data: &MenuData,
    menu: &PartyMenu,
    pool: &[String],
    minigame: &str,
) -> (Vec<String>, Vec<String>, String) {
    let mut lines = Vec::new();
    let mut map = String::new();
    for (row, value) in data.play.iter().zip(&menu.play) {
        match &row.kind {
            PlayKind::Map => map = value.clone(),
            PlayKind::Text { .. } if row.cvar == "hostport" && value.trim().is_empty() => {}
            _ => {
                let v = if value.is_empty() {
                    "\"\"".to_string()
                } else {
                    chimera_core::console::quote(value)
                };
                lines.push(format!("{} {v}", row.cvar));
            }
        }
    }
    if map.is_empty() {
        map = pool.first().cloned().unwrap_or_else(|| "greybox".into());
    }
    lines.push(format!("map {map}"));
    (lines, vec![format!("minigame {minigame}")], map)
}

/// Play's Start: the rows' cvars, the map loading, the minigame once it is
/// in.
fn start(world: &mut World) {
    let pool = map_pool(world);
    let minigame = world.resource::<MenuMinigame>().0.clone();
    let (lines, after, map) = {
        let data = world.resource::<MenuData>();
        start_lines(data, world.resource::<PartyMenu>(), &pool, &minigame)
    };
    for l in lines {
        submit(world, l);
    }
    let mut m = world.resource_mut::<PartyMenu>();
    m.screen = Screen::Loading;
    m.page = MenuPage::Main;
    m.focus = None;
    m.failure = None;
    m.background_load = false;
    m.loading = Some(map.clone());
    m.after_load = after;
    // Combat Arms' maps load behind its own loading screen.
    if map.starts_with("combat_arms:")
        && let Some(mut ca) = world.get_resource_mut::<combat_arms::ui::loading::CaLoading>()
    {
        ca.label = Some(map);
    }
}

/// Join: the password, then `connect`; the address remembered.
fn connect(world: &mut World, address: &str, password: &str) {
    if address.is_empty() {
        world.resource_mut::<PartyMenu>().failure = Some("Type the server's address (host or host:port).".into());
        return;
    }
    let pw = if password.is_empty() {
        "\"\"".to_string()
    } else {
        chimera_core::console::quote(password)
    };
    submit(world, format!("password {pw}"));
    submit(world, format!("connect {}", chimera_core::console::quote(address)));
    let mut recent = world.resource::<PartyMenu>().recent.clone();
    remember(&mut recent, address);
    world.resource::<RecentFile>().write(&recent);
    let mut m = world.resource_mut::<PartyMenu>();
    m.recent = recent;
    m.screen = Screen::Joining;
    m.loading = Some(address.to_string());
    m.focus = None;
}

/// Loads the menu didn't start: a map change in a game (a rotation, a
/// host's `map`) shows the loading panel; after leaving a game the
/// background map loads again.
fn follow_loads(world: &mut World) {
    let loading = chimera_client::client::console::map_loading(world);
    let menu = world.resource::<PartyMenu>();
    if menu.screen == Screen::InGame && loading {
        let map = chimera_client::client::console::loading_map(world).unwrap_or_default();
        let mut m = world.resource_mut::<PartyMenu>();
        m.screen = Screen::Loading;
        m.loading = Some(map);
        return;
    }
    if menu.reload_background && !loading && menu.screen == Screen::MainMenu {
        let data = world.resource::<MenuData>().background.maps.clone();
        let here = chimera_net::net::current_map(world);
        let mut m = world.resource_mut::<PartyMenu>();
        m.reload_background = false;
        // The first of the background maps (already loaded: stay).
        let Some(map) = data.first().cloned() else { return };
        if chimera_net::net::normalize_map(&map) == here {
            return;
        }
        m.background_load = true;
        submit(world, "MaxPlayer 0");
        submit(world, "maxplayers 1");
        submit(world, format!("map {map}"));
    }
}

/// While a page with settings shows, their values as the cvars hold them.
fn refresh_values(world: &mut World) {
    let menu = world.resource::<PartyMenu>();
    if !menu.shown() {
        return;
    }
    let settings: Vec<Setting> = match menu.page {
        MenuPage::Options => crate::esc_menu::option_tabs()
            .into_iter()
            .nth(menu.options_tab)
            .map(|(_, s)| s)
            .unwrap_or_default(),
        MenuPage::Play if menu.play_tab == 1 => game_settings(),
        _ => return,
    };
    for s in settings {
        let v = crate::controls::cvar_value(world, &s.cvar).unwrap_or_default();
        let mut m = world.resource_mut::<PartyMenu>();
        if m.values.get(&s.cvar) != Some(&v) {
            m.values.insert(s.cvar.clone(), v);
        }
    }
}

/// Combat Arms' loading screen shows only for its own maps (user
/// decision 5); for the others the party's loading panel.
fn ca_loading_gate(
    ca: Option<ResMut<combat_arms::ui::loading::CaLoading>>,
    join: Option<Res<JoinProgress>>,
    menu: Res<PartyMenu>,
) {
    let Some(mut ca) = ca else { return };
    let Some(label) = ca.label.clone() else { return };
    let ca_map = |m: &str| m.starts_with("combat_arms:");
    let own = ca_map(&label)
        || join.as_ref().and_then(|j| j.map.as_deref()).is_some_and(ca_map)
        || (menu.screen == Screen::Loading && menu.loading.as_deref().is_some_and(ca_map));
    if !own || menu.background_load {
        ca.label = None;
    }
}

/// Whether Combat Arms' loading screen is up (the party's panel then
/// isn't drawn).
fn ca_shows(world_ca: Option<&combat_arms::ui::loading::CaLoading>) -> bool {
    world_ca.is_some_and(|c| c.label.is_some())
}

// ---------------------------------------------------------------------------
// The background: the camera's orbit, the HUD hidden.

/// Where the orbit turns: the middle of the map's spawn points.
#[derive(Default)]
struct Orbit {
    map: String,
    center: Vec3,
}

/// The camera on its orbit while the menu shows (`FreeCam` 2: placed in
/// the world, the player's controls its own), back on the eye in a game.
#[allow(clippy::too_many_arguments)]
fn fly(
    menu: Res<PartyMenu>,
    data: Res<MenuData>,
    time: Res<Time<Real>>,
    map: Option<Res<chimera_world::map::LoadedMapName>>,
    spawns: Query<&Transform, With<SpawnPoint>>,
    free: Option<ResMut<chimera_client::client::view::FreeCam>>,
    mut orbit: Local<Orbit>,
    mut flying: Local<bool>,
) {
    let Some(mut free) = free else { return };
    if !menu.shown() {
        if std::mem::take(&mut *flying) {
            free.mode = 0;
            free.at = None;
        }
        return;
    }
    let name = map.map_or(String::new(), |m| m.0.clone());
    if orbit.map != name || orbit.center == Vec3::ZERO {
        let n = spawns.iter().count();
        orbit.center = if n == 0 {
            Vec3::ZERO
        } else {
            spawns.iter().map(|t| t.translation).sum::<Vec3>() / n as f32
        };
        orbit.map = name;
    }
    *flying = true;
    let b = &data.background;
    let angle = (time.elapsed_secs_f64() / b.period as f64 * std::f64::consts::TAU) as f32;
    let at = orbit.center + Vec3::new(angle.cos() * b.radius, b.height, angle.sin() * b.radius);
    // Looking at the middle, `pitch` down; yaw 0 looks along -Z.
    let to = orbit.center - at;
    let yaw = (-to.x).atan2(-to.z);
    free.mode = 2;
    free.at = Some((at, yaw, -b.pitch.to_radians()));
}

/// Combat Arms' menu music (its playlist, the install's BGM) while the
/// main menu shows (not while loading or in a game).
fn music(menu: Res<PartyMenu>, shown: Option<Res<combat_arms::game::music::AppMenuShown>>, mut commands: Commands) {
    let want = menu.screen == Screen::MainMenu;
    if want && shown.is_none() {
        commands.insert_resource(combat_arms::game::music::AppMenuShown);
    } else if !want && shown.is_some() {
        commands.remove_resource::<combat_arms::game::music::AppMenuShown>();
    }
}

/// The camera that draws the menu (and the console) while it shows: the
/// shell's UI camera, which the HUD draws on, is off then.
#[derive(Component)]
pub struct MenuUiCamera;

#[allow(clippy::type_complexity)]
fn cameras(
    menu: Res<PartyMenu>,
    ca: Option<Res<combat_arms::ui::loading::CaLoading>>,
    mut shell: Query<&mut Camera, (With<IsDefaultUiCamera>, Without<MenuUiCamera>)>,
    mut ours: Query<(Entity, &mut Camera), With<MenuUiCamera>>,
    consoles: Query<(Entity, Has<UiTargetCamera>), With<ConsoleRoot>>,
    mut commands: Commands,
) {
    if shell.is_empty() {
        return;
    }
    let menu_owns = menu.shown() && !ca_shows(ca.as_deref());
    let Some((cam, mut camera)) = ours.iter_mut().next() else {
        if menu_owns {
            let cam = commands
                .spawn((
                    Name::new("Main menu UI camera"),
                    MenuUiCamera,
                    chimera_client::client::debug::UiCamera,
                    bevy::core_pipeline::tonemapping::Tonemapping::Linear,
                    Camera2d,
                    Camera {
                        order: 101,
                        clear_color: ClearColorConfig::None,
                        ..default()
                    },
                    bevy::camera::visibility::RenderLayers::layer(30),
                ))
                .id();
            for (c, _) in &consoles {
                commands.entity(c).insert(UiTargetCamera(cam));
            }
        }
        return;
    };
    if camera.is_active != menu_owns {
        camera.is_active = menu_owns;
    }
    for mut s in &mut shell {
        if s.is_active == menu_owns {
            s.is_active = !menu_owns;
        }
    }
    for (c, targeted) in &consoles {
        if menu_owns && !targeted {
            commands.entity(c).insert(UiTargetCamera(cam));
        } else if !menu_owns && targeted {
            commands.entity(c).remove::<UiTargetCamera>();
        }
    }
}

// ---------------------------------------------------------------------------
// Drawing.

#[derive(Component)]
struct MenuRoot;

/// The loading panel's stage line and bar.
#[derive(Component)]
struct LoadingLine;
#[derive(Component)]
struct LoadingBar;

/// Where CS:S's game menu stacks (`widgets::GAME_UI_Z`): over the HUDs,
/// under the frames (`widgets::FRAME_Z`, which its backdrop would
/// otherwise take the clicks of), the Escape menu, the Controls page and
/// the console.
pub const MENU_Z: i32 = widgets::GAME_UI_Z;

/// What the menu's frames are drawn over.
pub const FRAME_PLATE: Color = Color::srgb(0.11, 0.12, 0.13);

/// What the menu last drew.
#[derive(Default, PartialEq)]
struct Drawn {
    menu: Option<PartyMenu>,
    size: Vec2,
    look: bool,
    ca: bool,
    pool: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
fn show(
    menu: Res<PartyMenu>,
    data: Res<MenuData>,
    look_res: Res<MenuLook>,
    fonts: Option<Res<UiFonts>>,
    title: Option<Res<crate::esc_menu::EscMenuTitle>>,
    version: Option<Res<chimera_core::core::BuildVersion>>,
    ca: Option<Res<combat_arms::ui::loading::CaLoading>>,
    minigames: Option<Res<chimera_sim::minigame::Minigames>>,
    game_id: Res<MenuMinigame>,
    windows_q: Query<&Window>,
    camera: Query<Entity, With<MenuUiCamera>>,
    roots: Query<Entity, With<MenuRoot>>,
    backing: Option<ResMut<widgets::FrameBacking>>,
    mut drawn: Local<Drawn>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else { return };
    // The frames' see-through colour over a dark plate, as over CS:S's
    // main menu picture (the live map would show through them).
    if let Some(mut b) = backing {
        let want = widgets::FrameBacking(menu.shown().then_some(widgets::Backing::Color(FRAME_PLATE)));
        b.set_if_neq(want);
    }
    let size = windows_q
        .iter()
        .next()
        .map_or(Vec2::new(1280.0, 720.0), |w| Vec2::new(w.width(), w.height()));
    let ca_up = ca_shows(ca.as_deref());
    let pool: Vec<String> = minigames
        .as_ref()
        .and_then(|m| m.get(&game_id.0))
        .map(|d| d.maps.iter().map(|m| chimera_net::net::normalize_map(m)).collect())
        .unwrap_or_default();
    let now = Drawn {
        menu: menu.shown().then(|| menu.clone()),
        size,
        look: look_res.ui.is_some(),
        ca: ca_up,
        pool: pool.clone(),
    };
    if *drawn == now {
        return;
    }
    for e in &roots {
        commands.entity(e).despawn();
    }
    *drawn = now;
    if !menu.shown() || ca_up {
        return;
    }
    let accent = Color::srgb_u8(255, 176, 0);
    let look = Look {
        ui: look_res.ui.as_deref(),
        fonts: &fonts,
        s: (size.y / 720.0).clamp(0.6, 3.0),
        height: size.y,
        accent,
    };
    let shade = match menu.screen {
        Screen::MainMenu => Color::srgba(0.0, 0.0, 0.0, 0.25),
        _ => Color::srgba(0.0, 0.0, 0.0, 0.6),
    };
    let mut root = commands.spawn((
        MenuRoot,
        Node {
            position_type: PositionType::Absolute,
            width: percent(100.0),
            height: percent(100.0),
            ..default()
        },
        BackgroundColor(widgets::display_alpha(shade)),
        bevy::ui::FocusPolicy::Block,
        GlobalZIndex(MENU_Z),
    ));
    if let Some(cam) = camera.iter().next() {
        root.insert(UiTargetCamera(cam));
    }
    let root = root.id();
    let title = title.map_or_else(|| "Ultimate Gun Game".to_string(), |t| t.0.clone());
    if let Some(f) = &menu.failure {
        failure_frame(&mut commands, root, &look, f);
        return;
    }
    if let Some(what) = &menu.loading {
        loading_frame(&mut commands, root, &look, what);
        return;
    }
    main_list(
        &mut commands,
        root,
        &menu,
        &data,
        &look,
        size,
        &title,
        version.map_or("", |v| v.0),
    );
    match menu.page {
        MenuPage::Main => {}
        MenuPage::Play => play_frame(&mut commands, root, &menu, &data, &look, &pool),
        MenuPage::Join => join_frame(&mut commands, root, &menu, &look),
        MenuPage::Options => options_frame(&mut commands, root, &menu, &look),
    }
}

fn text_of(look: &Look, key: &str, fallback: &str) -> String {
    look.ui
        .and_then(|u| u.strings.get(&key.trim_start_matches('#').to_lowercase()))
        .cloned()
        .unwrap_or_else(|| fallback.to_string())
}

/// The left-hand entries, as CS:S's main menu places them: at the left
/// inset, one per `MainMenu.MenuItemHeight`, the bottom one 72 scheme
/// pixels up from the foot, the app's name over them in the menu's font,
/// larger; the open page's entry in the depressed colour.
#[allow(clippy::too_many_arguments)]
fn main_list(
    commands: &mut Commands,
    root: Entity,
    menu: &PartyMenu,
    data: &MenuData,
    look: &Look,
    size: Vec2,
    title: &str,
    version: &str,
) {
    let item_h = look.number("MainMenu.MenuItemHeight", 22.0);
    let inset = look.number("MainMenu.Inset", 32.0);
    let gaps = data.entries.iter().skip(1).filter(|e| e.gap).count() as f32;
    let total = item_h * (data.entries.len() as f32 + gaps * 0.5);
    let bottom = size.y / look.s - 72.0;
    let top = bottom - total;
    let font = look.font("MenuLarge", (12.0, true));
    let normal = look.color("MainMenu.TextColor", [255, 255, 255, 255]);
    let current = look.color("MainMenu.DepressedTextColor", [192, 186, 80, 255]);
    // The scheme's menu font, large: CS:S's title font is its logo's and
    // has only the letters of the game's name.
    let line_h = 48.0;
    let title_text = TextFont {
        font_size: bevy::text::FontSize::Px(36.0 * look.s),
        ..look.font("MenuLarge", (12.0, true))
    };
    label(
        commands,
        root,
        look,
        (inset, top - item_h - line_h, size.x / look.s - inset, line_h),
        &title.to_uppercase(),
        title_text,
        normal,
        -1,
    );
    let mut y = top;
    for (i, e) in data.entries.iter().enumerate() {
        if e.gap && i > 0 {
            y += item_h * 0.5;
        }
        let open = matches!(&e.does, EntryDoes::Page(p) if *p == menu.page && menu.page != MenuPage::Main);
        let node = commands
            .spawn((
                place(look, inset - 4.0, y, 260.0, item_h),
                MainMenuAction::Entry(i),
                widgets::clickable(),
                EntryLabel {
                    normal,
                    armed: look.color("MainMenu.ArmedTextColor", [200, 200, 200, 255]),
                },
                ChildOf(root),
            ))
            .id();
        label(
            commands,
            node,
            look,
            (4.0, 0.0, 256.0, item_h),
            &e.label.to_uppercase(),
            font.clone(),
            if open { current } else { normal },
            -1,
        );
        y += item_h;
    }
    label(
        commands,
        root,
        look,
        (size.x / look.s - 408.0, size.y / look.s - 20.0, 400.0, 16.0),
        version,
        look.font("DefaultSmall", (13.0, false)),
        look.disabled(),
        1,
    );
}

/// An entry's colours (the armed one under the pointer, as GameUI lights
/// it).
#[derive(Component)]
struct EntryLabel {
    normal: Color,
    armed: Color,
}

/// The entry under the pointer in its armed colour.
fn entry_hover(
    entries: Query<(&widgets::Interaction, &EntryLabel, &Children), Changed<widgets::Interaction>>,
    children: Query<&Children>,
    mut texts: Query<&mut TextColor>,
) {
    for (i, e, kids) in &entries {
        let color = if *i == widgets::Interaction::None {
            e.normal
        } else {
            e.armed
        };
        for k in kids.iter() {
            for t in children.get(k).into_iter().flat_map(|c| c.iter()) {
                if let Ok(mut c) = texts.get_mut(t) {
                    c.0 = color;
                }
            }
        }
    }
}

/// A row of a settings frame: its label, `<`, the value, `>`.
#[allow(clippy::too_many_arguments)]
fn step_row(
    commands: &mut Commands,
    frame: Entity,
    look: &Look,
    y: f32,
    w: f32,
    text: &str,
    value: &str,
    back: MainMenuAction,
    fwd: MainMenuAction,
) {
    let font = look.default_font();
    label(
        commands,
        frame,
        look,
        (20.0, y, 220.0, 24.0),
        text,
        font.clone(),
        look.text(),
        -1,
    );
    let x = w - 20.0 - 24.0 - 200.0 - 24.0;
    widgets::button(
        commands,
        frame,
        look,
        (x, y, 24.0, 24.0),
        "<",
        Btn::enabled(true),
        0,
        back,
    );
    let field = commands
        .spawn((
            Node {
                border: UiRect::all(px(1.0)),
                ..place(look, x + 26.0, y, 196.0, 24.0)
            },
            widgets::bevel(look, false),
            BackgroundColor(look.sunken_bg()),
            ChildOf(frame),
        ))
        .id();
    label(
        commands,
        field,
        look,
        (4.0, 0.0, 186.0, 22.0),
        value,
        font,
        look.text(),
        -1,
    );
    widgets::button(
        commands,
        frame,
        look,
        (x + 224.0, y, 24.0, 24.0),
        ">",
        Btn::enabled(true),
        0,
        fwd,
    );
}

fn map_label(id: &str) -> String {
    id.rsplit(':').next().unwrap_or(id).to_string()
}

/// Play: CS:S's Create Server's place, its Server and Game tabs, Start
/// and Cancel.
fn play_frame(commands: &mut Commands, root: Entity, menu: &PartyMenu, data: &MenuData, look: &Look, pool: &[String]) {
    let rows = if menu.play_tab == 0 {
        data.play.len()
    } else {
        game_settings().len()
    };
    let (w, h) = (560.0, 134.0 + rows as f32 * 30.0);
    let parts = widgets::frame(
        commands,
        root,
        look,
        FrameSpec::new(MenuPage::Play.window()),
        (w, h),
        "Play",
    );
    let f = parts.frame;
    if let Some(c) = parts.close {
        commands.entity(c).insert(MainMenuAction::Back);
    }
    let names = vec![("Server".to_string(), true), ("Game".to_string(), true)];
    widgets::tabs(
        commands,
        f,
        look,
        (12.0, 34.0),
        &names,
        menu.play_tab,
        None,
        Some(w - 24.0),
        MainMenuAction::PlayTab,
    );
    // The sheet under the tabs.
    commands.spawn((
        Node {
            border: UiRect::all(px(1.0)),
            ..place(look, 12.0, 60.0, w - 24.0, h - 60.0 - 46.0)
        },
        widgets::bevel(look, true),
        ChildOf(f),
    ));
    let mut y = 74.0;
    if menu.play_tab == 0 {
        for (i, r) in data.play.iter().enumerate() {
            let value = menu.play.get(i).cloned().unwrap_or_default();
            match &r.kind {
                PlayKind::Map => {
                    let shown = if value.is_empty() {
                        format!("Rotation ({} maps)", pool.len())
                    } else {
                        map_label(&value)
                    };
                    step_row(
                        commands,
                        f,
                        look,
                        y,
                        w,
                        &r.label,
                        &shown,
                        MainMenuAction::PlayStep { row: i, dir: -1 },
                        MainMenuAction::PlayStep { row: i, dir: 1 },
                    );
                }
                PlayKind::Pick(p) => {
                    step_row(
                        commands,
                        f,
                        look,
                        y,
                        w,
                        &r.label,
                        &p.shown(&value),
                        MainMenuAction::PlayStep { row: i, dir: -1 },
                        MainMenuAction::PlayStep { row: i, dir: 1 },
                    );
                }
                PlayKind::Text { masked, .. } => {
                    label(
                        commands,
                        f,
                        look,
                        (20.0, y, 220.0, 24.0),
                        &r.label,
                        look.default_font(),
                        look.text(),
                        -1,
                    );
                    let focused = menu.focus == Some(Field::Play(i));
                    widgets::text_entry(
                        commands,
                        f,
                        look,
                        (w - 20.0 - 272.0, y, 272.0, 24.0),
                        &value,
                        focused.then_some(menu.caret),
                        *masked,
                        true,
                        MainMenuAction::Focus(Field::Play(i)),
                    );
                }
            }
            y += 30.0;
        }
    } else {
        for (i, s) in game_settings().iter().enumerate() {
            let v = menu.values.get(&s.cvar).cloned().unwrap_or_default();
            step_row(
                commands,
                f,
                look,
                y,
                w,
                &s.label,
                &s.pick.shown(&v),
                MainMenuAction::GameStep { index: i, dir: -1 },
                MainMenuAction::GameStep { index: i, dir: 1 },
            );
            y += 30.0;
        }
    }
    widgets::button(
        commands,
        f,
        look,
        (w - 20.0 - 72.0 - 8.0 - 72.0, h - 36.0, 72.0, 24.0),
        &text_of(look, "#GameUI_Start", "Start"),
        Btn::enabled(true).default_button(true),
        0,
        MainMenuAction::Start,
    );
    widgets::button(
        commands,
        f,
        look,
        (w - 20.0 - 72.0, h - 36.0, 72.0, 24.0),
        &text_of(look, "#GameUI_Cancel", "Cancel"),
        Btn::enabled(true),
        0,
        MainMenuAction::Back,
    );
}

/// Join: the address and password entries, Connect and Cancel, the recent
/// addresses.
fn join_frame(commands: &mut Commands, root: Entity, menu: &PartyMenu, look: &Look) {
    let (w, h) = (420.0, 160.0 + menu.recent.len().max(1) as f32 * 28.0 + 52.0);
    let parts = widgets::frame(
        commands,
        root,
        look,
        FrameSpec::new(MenuPage::Join.window()),
        (w, h),
        "Join",
    );
    let f = parts.frame;
    if let Some(c) = parts.close {
        commands.entity(c).insert(MainMenuAction::Back);
    }
    let font = look.default_font();
    label(
        commands,
        f,
        look,
        (20.0, 40.0, 120.0, 24.0),
        "Address",
        font.clone(),
        look.text(),
        -1,
    );
    widgets::text_entry(
        commands,
        f,
        look,
        (140.0, 40.0, w - 160.0, 24.0),
        &menu.address,
        (menu.focus == Some(Field::Address)).then_some(menu.caret),
        false,
        true,
        MainMenuAction::Focus(Field::Address),
    );
    label(
        commands,
        f,
        look,
        (20.0, 72.0, 120.0, 24.0),
        "Password",
        font.clone(),
        look.text(),
        -1,
    );
    widgets::text_entry(
        commands,
        f,
        look,
        (140.0, 72.0, w - 160.0, 24.0),
        &menu.password,
        (menu.focus == Some(Field::Password)).then_some(menu.caret),
        true,
        true,
        MainMenuAction::Focus(Field::Password),
    );
    label(
        commands,
        f,
        look,
        (20.0, 104.0, w - 40.0, 20.0),
        "host or host:port (port 27015 unless given)",
        look.font("DefaultSmall", (13.0, false)),
        look.dull(),
        -1,
    );
    label(
        commands,
        f,
        look,
        (20.0, 132.0, w - 40.0, 24.0),
        "Recent",
        font.clone(),
        look.text(),
        -1,
    );
    let mut y = 160.0;
    if menu.recent.is_empty() {
        label(
            commands,
            f,
            look,
            (28.0, y, w - 56.0, 24.0),
            "(none yet)",
            font,
            look.dull(),
            -1,
        );
    }
    for (i, a) in menu.recent.iter().enumerate() {
        widgets::button(
            commands,
            f,
            look,
            (20.0, y, w - 40.0, 24.0),
            a,
            Btn::enabled(true),
            -1,
            MainMenuAction::Recent(i),
        );
        y += 28.0;
    }
    widgets::button(
        commands,
        f,
        look,
        (w - 20.0 - 72.0 - 8.0 - 80.0, h - 36.0, 80.0, 24.0),
        &text_of(look, "#ServerBrowser_Connect", "Connect"),
        Btn::enabled(true).default_button(true),
        0,
        MainMenuAction::Connect,
    );
    widgets::button(
        commands,
        f,
        look,
        (w - 20.0 - 72.0, h - 36.0, 72.0, 24.0),
        &text_of(look, "#GameUI_Cancel", "Cancel"),
        Btn::enabled(true),
        0,
        MainMenuAction::Back,
    );
}

/// Options: the Escape menu's tabs (data/options.toml), Binds, Close.
fn options_frame(commands: &mut Commands, root: Entity, menu: &PartyMenu, look: &Look) {
    let tabs = crate::esc_menu::option_tabs();
    let rows = tabs.iter().map(|(_, s)| s.len()).max().unwrap_or(0);
    let (w, h) = (560.0, 134.0 + rows as f32 * 30.0);
    let parts = widgets::frame(
        commands,
        root,
        look,
        FrameSpec::new(MenuPage::Options.window()),
        (w, h),
        &text_of(look, "#GameUI_Options", "Options"),
    );
    let f = parts.frame;
    if let Some(c) = parts.close {
        commands.entity(c).insert(MainMenuAction::Back);
    }
    let names: Vec<(String, bool)> = tabs.iter().map(|(n, _)| (n.clone(), true)).collect();
    widgets::tabs(
        commands,
        f,
        look,
        (12.0, 34.0),
        &names,
        menu.options_tab,
        None,
        Some(w - 24.0),
        MainMenuAction::OptionsTab,
    );
    commands.spawn((
        Node {
            border: UiRect::all(px(1.0)),
            ..place(look, 12.0, 60.0, w - 24.0, h - 60.0 - 46.0)
        },
        widgets::bevel(look, true),
        ChildOf(f),
    ));
    let mut y = 74.0;
    if let Some((_, settings)) = tabs.get(menu.options_tab) {
        for (i, s) in settings.iter().enumerate() {
            let v = menu.values.get(&s.cvar).cloned().unwrap_or_default();
            step_row(
                commands,
                f,
                look,
                y,
                w,
                &s.label,
                &s.pick.shown(&v),
                MainMenuAction::OptionStep { index: i, dir: -1 },
                MainMenuAction::OptionStep { index: i, dir: 1 },
            );
            y += 30.0;
        }
    }
    widgets::button(
        commands,
        f,
        look,
        (20.0, h - 36.0, 120.0, 24.0),
        "Binds...",
        Btn::enabled(true),
        0,
        MainMenuAction::Binds,
    );
    widgets::button(
        commands,
        f,
        look,
        (w - 20.0 - 72.0, h - 36.0, 72.0, 24.0),
        &text_of(look, "#GameUI_Close", "Close"),
        Btn::enabled(true).default_button(true),
        0,
        MainMenuAction::Back,
    );
}

/// CS:S's loading dialog's place and parts: "Loading...", the stage line,
/// the bar, Cancel.
fn loading_frame(commands: &mut Commands, root: Entity, look: &Look, what: &str) {
    let layout = look.ui.and_then(|u| u.loading.as_ref());
    let at = |name: &str, fallback: (f32, f32, f32, f32)| {
        layout
            .and_then(|l| l.get(name))
            .map_or(fallback, |c| (coord(c.x), coord(c.y), c.wide, c.tall))
    };
    let (_, _, w, h) = at("LoadingDialog", (0.0, 0.0, 380.0, 112.0));
    let title = text_of(look, "#GameUI_Loading", "Loading...");
    let f = widgets::frame(
        commands,
        root,
        look,
        FrameSpec::new("partyloading").no_close(),
        (w, h),
        &title,
    )
    .frame;
    let info = label(
        commands,
        f,
        look,
        at("InfoLabel", (20.0, 34.0, 340.0, 24.0)),
        &format!("Loading {} ...", map_label(what)),
        look.font("Default", (16.0, false)),
        look.text(),
        -1,
    );
    commands.entity(info).insert(LoadingLine);
    let (x, y, bw, bh) = at("Progress", (20.0, 64.0, 260.0, 24.0));
    let bar = commands
        .spawn((
            Node {
                border: UiRect::all(px(1.0)),
                padding: UiRect::all(look.px(2.0)),
                ..place(look, x, y, bw, bh)
            },
            widgets::bevel(look, false),
            BackgroundColor(look.color("ProgressBar.BgColor", [0, 0, 0, 128])),
            ChildOf(f),
        ))
        .id();
    commands.spawn((
        LoadingBar,
        Node {
            width: percent(0.0),
            height: percent(100.0),
            ..default()
        },
        BackgroundColor(look.color("ProgressBar.FgColor", [216, 222, 211, 255])),
        ChildOf(bar),
    ));
    widgets::button(
        commands,
        f,
        look,
        at("CancelButton", (288.0, 64.0, 72.0, 24.0)),
        &text_of(look, "#GameUI_Cancel", "Cancel"),
        Btn::enabled(true).default_button(true),
        0,
        MainMenuAction::Cancel,
    );
}

/// A layout coordinate's number (the dialog's own space).
fn coord(c: chimera_world::map::hud::HudCoord) -> f32 {
    use chimera_world::map::hud::HudCoord as C;
    match c {
        C::Start(v) | C::Centre(v) | C::End(v) => v,
    }
}

/// Why joining failed: CS:S's "Disconnected" frame, the reason, Close.
fn failure_frame(commands: &mut Commands, root: Entity, look: &Look, text: &str) {
    let (w, h) = (380.0, 150.0);
    let title = text_of(look, "#GameUI_Disconnected", "Disconnected");
    let parts = widgets::frame(commands, root, look, FrameSpec::new("partyfailure"), (w, h), &title);
    if let Some(c) = parts.close {
        commands.entity(c).insert(MainMenuAction::CloseFailure);
    }
    let body = commands
        .spawn((
            Node {
                overflow: Overflow::clip(),
                ..place(look, 20.0, 34.0, w - 40.0, h - 34.0 - 40.0)
            },
            ChildOf(parts.frame),
        ))
        .id();
    commands.spawn((
        Text::new(text),
        look.font("Default", (16.0, false)),
        TextColor(look.text()),
        TextLayout::new(Justify::Left, bevy::text::LineBreak::WordBoundary),
        ChildOf(body),
    ));
    widgets::button(
        commands,
        parts.frame,
        look,
        (w - 20.0 - 72.0, h - 34.0, 72.0, 24.0),
        &text_of(look, "#GameUI_Close", "Close"),
        Btn::enabled(true).default_button(true),
        0,
        MainMenuAction::CloseFailure,
    );
}

/// The loading panel's line and bar now: joining's stage (the map's load,
/// the download's bytes), a map's load, "Starting local game server...".
pub fn loading_state(
    ui: Option<&GameUi>,
    join: Option<&JoinProgress>,
    progress: Option<chimera_world::map::loading::LoadProgress>,
    hosting: bool,
    what: &str,
) -> (String, f32) {
    let text = |token: &str, ours: &str| {
        ui.and_then(|u| u.strings.get(&token.to_lowercase()))
            .cloned()
            .unwrap_or_else(|| ours.to_string())
    };
    if let Some(j) = join.filter(|j| j.stage != JoinStage::Idle && j.failure.is_none()) {
        let (token, ours) = j.stage.token();
        let stage = text(token, ours);
        return match j.stage {
            JoinStage::LoadingMap => match progress {
                Some(p) => (text(p.stage, p.stage), p.fraction),
                None => (stage, 0.0),
            },
            JoinStage::Downloading => match j.download.as_ref() {
                Some(d) if d.total > 0 => {
                    let f = (d.done as f64 / d.total as f64) as f32;
                    (
                        format!("{stage} {} of {} KB", d.done / 1024, d.total / 1024),
                        f.clamp(0.0, 1.0),
                    )
                }
                _ => (stage, 0.0),
            },
            JoinStage::Connecting => (stage, 0.02),
            _ => (stage, 0.05),
        };
    }
    match progress {
        Some(p) => (text(p.stage, p.stage), p.fraction),
        None if hosting => (
            text("LoadingProgress_SpawningServer", "Starting local game server..."),
            0.0,
        ),
        None => (format!("Loading {} ...", map_label(what)), 0.0),
    }
}

/// The loading panel follows the load.
#[allow(clippy::too_many_arguments)]
fn progress(
    menu: Res<PartyMenu>,
    look: Res<MenuLook>,
    join: Option<Res<JoinProgress>>,
    settings: Option<Res<chimera_net::net::NetSettings>>,
    role: Option<Res<NetRole>>,
    reporter: Option<Res<chimera_world::map::loading::LoadReporter>>,
    lines: Query<&Children, With<LoadingLine>>,
    mut texts: Query<&mut Text>,
    mut bars: Query<&mut Node, With<LoadingBar>>,
) {
    let Some(what) = menu.loading.as_deref() else { return };
    let hosting = settings.is_some_and(|s| s.maxplayers > 1) && role.as_deref().is_none_or(|r| *r != NetRole::Client);
    let (line, fraction) = loading_state(
        look.ui.as_deref(),
        join.as_deref(),
        reporter.and_then(|r| r.current()),
        hosting,
        what,
    );
    for mut n in &mut bars {
        let w = percent(100.0 * fraction);
        if n.width != w {
            n.width = w;
        }
    }
    for children in &lines {
        for c in children.iter() {
            if let Ok(mut t) = texts.get_mut(c)
                && t.0 != line
            {
                t.0 = line.clone();
            }
        }
    }
}

/// The local player's entity (tests).
pub fn local_player(world: &mut World) -> Option<Entity> {
    world.query_filtered::<Entity, With<LocalPlayer>>().iter(world).next()
}
