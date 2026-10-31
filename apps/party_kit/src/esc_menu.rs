//! The party apps' Escape menu (feature `present`; Ultimate Gun Game's
//! and Lucker Party's): their own, in place of Combat Arms' in-game menu
//! (which yields Esc to it: `front_end::AppMenu`), drawn above everything
//! but the console (the HUDs, the match-end screen, the Tab scoreboard).
//!
//! - **Esc** opens it, closes it from its first page, and goes back to the
//!   first page from the others (not while the console, chat or the
//!   Controls page has the key). While it is open the mouse is free and
//!   the game gets no input: `AppMenu::open` counts in `MenuStatus::open`
//!   (the binds wait, a click doesn't grab the mouse, the local intent is
//!   cleared with the mouse free) and every key pressed is captured
//!   (`binds::CapturedKeys`), so a key held across its closing stays
//!   dead until let go. Closing takes the mouse back.
//! - **Resume.**
//! - **Options**: a tab per page of data/options.toml (video, audio,
//!   mouse and the party's own: `cl_hitmarker`, `party_swap_effect`,
//!   `party_music`, `hud_radar`), each an existing cvar of the player's
//!   (archived; leaving the page writes the config), and "Binds", which
//!   opens the Controls page (`controls`, F6).
//! - **Host settings** (`host::SETTINGS`, data/host_settings.toml): only
//!   for the host (`host::is_host`: not a network client): each cvar set
//!   on the host's own server; those that take effect at the next match
//!   say so.
//! - **Vote to skip the map** (`party_skipmap`, `host::SKIP_ISSUE`): on
//!   everyone's menu; while a vote runs the menu shows it with Yes and No.
//!   A refused call says why (a vote running, the caller's wait).
//! - **Disconnect** (only in an app with the main menu, `main_menu`):
//!   `disconnect`, back to the menu; a host with people connected is
//!   asked first ("End the game for 2 players?", `Page::
//!   ConfirmDisconnect`), and its clients are told the host left.
//! - **Quit.**
//!
//! The entries and their order are data (data/main_menu.toml
//! `[[escape]]`, `when` gating the host's and the main menu's). While the
//! main menu shows (out of a game) Esc is the main menu's and this menu
//! stays shut; `AppMenu::open` then counts the main menu too.
//!
//! The vote panel (`VotePanel`): while a vote runs, everyone sees a
//! small panel ("Ann wants to skip the map", F1 yes / F2 no, the counts,
//! the votes needed, the seconds left), then how it ended for a few
//! seconds; from the server's news (`net::vote::VoteView`).
//!
//! `EscMenu` is the state (headless too: tests drive it by keys and by
//! `EscMenuAction`s); drawing is Bevy UI, rebuilt when what it shows
//! changes, in the party's panel look (the Controls page's colours).
//! `escmenu [options|host]` (console) toggles it and `escmenu_press
//! <entry>` presses one, for screenshots.
//!
//! Known shortcuts: options are the data file's cvars only (each game's full options are in its own
//! menus); no keyboard navigation of the entries.

use std::collections::BTreeMap;

use bevy::{input::ButtonInput, prelude::*, window::CursorOptions};
use chimera_client::client::{
    binds::CapturedKeys,
    front_end::AppMenu,
    input::{capture_cursor, release_cursor},
    widgets::display_alpha,
};
use chimera_core::console::{Console, ConsoleAppExt};
use chimera_net::net::vote::{CallRefusal, VoteFailure, VoteNews, VoteSettings, VoteView};

use crate::host::{self, Setting, setting_of};

/// The options' data (data/options.toml).
pub const OPTIONS_TOML: &str = include_str!("../data/options.toml");

/// The options' tabs: name and settings, in order.
pub fn option_tabs() -> Vec<(String, Vec<Setting>)> {
    let doc: toml::Table = toml::from_str(OPTIONS_TOML).expect("data/options.toml reads");
    doc.get("tab")
        .and_then(|t| t.as_array())
        .map(|tabs| {
            tabs.iter()
                .map(|t| {
                    let name = t.get("name").and_then(|n| n.as_str()).unwrap_or("?").to_string();
                    let settings = t
                        .get("setting")
                        .and_then(|s| s.as_array())
                        .map(|a| a.iter().map(|v| setting_of(v).expect("data/options.toml")).collect())
                        .unwrap_or_default();
                    (name, settings)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The menu's pages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Main,
    /// The options' tab.
    Options(usize),
    Host,
    /// A host's Disconnect asks first: it ends the game for the others
    /// (how many people are connected).
    ConfirmDisconnect(usize),
}

/// The menu's title: the app's name (`client_app`'s).
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct EscMenuTitle(pub String);

impl Default for EscMenuTitle {
    fn default() -> Self {
        Self("Lucker Party".into())
    }
}

/// The menu's state.
#[derive(Resource, Debug, Default, PartialEq)]
pub struct EscMenu {
    pub open: bool,
    pub page: Page,
    /// The shown settings' values as the cvars hold them.
    pub values: BTreeMap<String, String>,
    /// An options value changed since the page opened (the config is
    /// written on leaving it).
    pub dirty: bool,
    /// A line under the entries (why a vote call was refused, what
    /// happened).
    pub notice: Option<String>,
}

impl EscMenu {
    pub fn set_open(&mut self, open: bool) {
        self.open = open;
        self.page = Page::Main;
        self.notice = None;
    }
}

/// What a click on the menu does (also sent by tests: `EscMenuAction`).
#[derive(Component, Message, Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Resume,
    Options,
    Host,
    Back,
    /// An options tab.
    Tab(usize),
    Binds,
    /// A setting's value moved: host page or options tab, its index, and
    /// the direction.
    Step { host: bool, index: usize, dir: i8 },
    SkipMap,
    VoteYes,
    VoteNo,
    /// Leave the game for the main menu (`disconnect`; a host with people
    /// connected is asked first).
    Disconnect,
    /// The confirm's Yes.
    DisconnectYes,
    Quit,
}

/// An action as a message (tests, `escmenu`).
pub type EscMenuAction = Action;

/// The settings a page shows (none on the first).
pub fn page_settings(page: Page) -> Vec<Setting> {
    match page {
        Page::Main | Page::ConfirmDisconnect(_) => Vec::new(),
        Page::Host => host::settings(),
        Page::Options(t) => option_tabs().into_iter().nth(t).map(|(_, s)| s).unwrap_or_default(),
    }
}

#[derive(Component)]
struct MenuRoot;

#[derive(Component)]
struct VoteRoot;

/// Its systems (in `Update`, before the Controls page's).
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct EscMenuSystems;

pub struct EscMenuPlugin;

impl Plugin for EscMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EscMenu>()
            .init_resource::<EscMenuTitle>()
            .init_resource::<AppMenu>()
            .init_resource::<CapturedKeys>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<VotePanel>()
            .add_message::<Action>()
            .console_command("escmenu", "escmenu [options|host]: open or close the Escape menu (at that page).", |w, a| {
                let page = match a.first().map(String::as_str) {
                    Some("options") => Page::Options(0),
                    Some("host") => Page::Host,
                    _ => Page::Main,
                };
                let mut m = w.resource_mut::<EscMenu>();
                let open = !(m.open && a.is_empty());
                m.set_open(open);
                m.page = page;
                Ok(None)
            })
            .console_command(
                "escmenu_press",
                "escmenu_press <resume|options|host|disconnect|confirm|back|quit>: press that entry of the open \
                 Escape menu (screenshots and tests).",
                |w, a| {
                    let action = match a.first().map(String::as_str) {
                        Some("resume") => Action::Resume,
                        Some("options") => Action::Options,
                        Some("host") => Action::Host,
                        Some("disconnect") => Action::Disconnect,
                        Some("confirm") => Action::DisconnectYes,
                        Some("back") => Action::Back,
                        Some("quit") => Action::Quit,
                        other => return Err(format!("no entry {other:?}")),
                    };
                    w.write_message(action);
                    Ok(None)
                },
            )
            .add_systems(
                Update,
                (toggle, clicks, act, refresh_values, sync, show, show_vote)
                    .chain()
                    .in_set(EscMenuSystems)
                    .before(crate::controls::ControlsSystems),
            );
    }
}

/// Esc: open, back to the first page, or close (unless the console, chat
/// or the Controls page has it).
fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    console: Option<Res<chimera_client::client::console::ConsoleUi>>,
    chat: Option<Res<chimera_client::client::chat::ChatInput>>,
    controls: Option<Res<crate::controls::ControlsPage>>,
    main_menu: Option<Res<crate::main_menu::PartyMenu>>,
    mut menu: ResMut<EscMenu>,
    mut console_cmds: Option<ResMut<Console>>,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    // Out of a game the main menu has Esc.
    if main_menu.is_some_and(|m| m.shown()) {
        if menu.open {
            menu.set_open(false);
        }
        return;
    }
    if console.is_some_and(|c| c.open) || chat.is_some_and(|c| c.open.is_some()) || controls.is_some_and(|c| c.open) {
        return;
    }
    if !menu.open {
        menu.set_open(true);
    } else if menu.page != Page::Main {
        leave_page(&mut menu, console_cmds.as_deref_mut());
        menu.page = Page::Main;
    } else {
        leave_page(&mut menu, console_cmds.as_deref_mut());
        menu.set_open(false);
    }
}

/// Leaving a page: the options written to the config if changed.
fn leave_page(menu: &mut EscMenu, console: Option<&mut Console>) {
    if std::mem::take(&mut menu.dirty)
        && let Some(c) = console
    {
        c.submit("host_writeconfig");
    }
}

/// Clicks on the menu's buttons as actions.
fn clicks(pressed: Query<(&Interaction, &Action), Changed<Interaction>>, mut out: MessageWriter<Action>) {
    for (i, a) in &pressed {
        if *i == Interaction::Pressed {
            out.write(*a);
        }
    }
}

/// What the actions do.
fn act(world: &mut World) {
    let actions: Vec<Action> = world.resource_mut::<Messages<Action>>().drain().collect();
    for a in actions {
        act_one(world, a);
    }
}

fn submit(world: &mut World, line: impl Into<String>) {
    world.resource_mut::<Console>().submit(line.into());
}

fn act_one(world: &mut World, action: Action) {
    let (open, page) = {
        let m = world.resource::<EscMenu>();
        (m.open, m.page)
    };
    if !open {
        return;
    }
    world.resource_mut::<EscMenu>().notice = None;
    let leave = |world: &mut World| {
        if std::mem::take(&mut world.resource_mut::<EscMenu>().dirty) {
            submit(world, "host_writeconfig");
        }
    };
    match action {
        Action::Resume => {
            leave(world);
            world.resource_mut::<EscMenu>().set_open(false);
        }
        Action::Options => world.resource_mut::<EscMenu>().page = Page::Options(0),
        Action::Tab(t) => world.resource_mut::<EscMenu>().page = Page::Options(t),
        Action::Host => {
            if host::is_host(world) {
                world.resource_mut::<EscMenu>().page = Page::Host;
            }
        }
        Action::Back => {
            leave(world);
            world.resource_mut::<EscMenu>().page = Page::Main;
        }
        Action::Binds => {
            if let Some(mut c) = world.get_resource_mut::<crate::controls::ControlsPage>() {
                c.open = true;
                c.capture = None;
            }
        }
        Action::Step { host: on_host, index, dir } => {
            let settings = if on_host {
                if page != Page::Host || !host::is_host(world) {
                    return;
                }
                host::settings()
            } else {
                page_settings(page)
            };
            let Some(s) = settings.get(index) else { return };
            let now = crate::controls::cvar_value(world, &s.cvar).unwrap_or_default();
            let next = s.pick.next(&now, dir);
            let quoted = if next.is_empty() { "\"\"".to_string() } else { chimera_core::console::quote(&next) };
            submit(world, format!("{} {quoted}", s.cvar));
            if !on_host {
                world.resource_mut::<EscMenu>().dirty = true;
            }
        }
        Action::Disconnect => {
            let people = people_connected(world);
            if people > 0 && host::is_host(world) {
                world.resource_mut::<EscMenu>().page = Page::ConfirmDisconnect(people);
            } else {
                leave(world);
                world.resource_mut::<EscMenu>().set_open(false);
                submit(world, "disconnect");
            }
        }
        Action::DisconnectYes => {
            leave(world);
            world.resource_mut::<EscMenu>().set_open(false);
            submit(world, "disconnect");
        }
        Action::SkipMap => submit(world, "party_skipmap"),
        Action::VoteYes => submit(world, "vote option1"),
        Action::VoteNo => submit(world, "vote option2"),
        Action::Quit => submit(world, "quit"),
    }
}

/// The people connected to this host's server (not bots, not the host).
pub fn people_connected(world: &mut World) -> usize {
    world.query::<&chimera_net::net::server::Player>().iter(world).count()
}

/// While open, the values of the settings it shows; a refused vote call's
/// words.
fn refresh_values(world: &mut World) {
    let refused = world.get_resource_mut::<VoteView>().and_then(|mut v| v.refused.take());
    let menu = world.resource::<EscMenu>();
    if !menu.open {
        return;
    }
    if let Some((r, _)) = refused {
        world.resource_mut::<EscMenu>().notice = Some(refusal_text(&r));
    }
    let page = world.resource::<EscMenu>().page;
    for s in page_settings(page) {
        let v = crate::controls::cvar_value(world, &s.cvar).unwrap_or_default();
        let mut m = world.resource_mut::<EscMenu>();
        if m.values.get(&s.cvar) != Some(&v) {
            m.values.insert(s.cvar.clone(), v);
        }
    }
}

/// Why a vote call was refused, in words.
pub fn refusal_text(r: &CallRefusal) -> String {
    match r {
        CallRefusal::Disabled => "Votes are off on this server.".into(),
        CallRefusal::IssueDisabled => "There is no map to skip.".into(),
        CallRefusal::InProgress => "A vote is already running.".into(),
        CallRefusal::Spam(s) => format!("You can call another vote in {s} s."),
        CallRefusal::Recently(s) => format!("A skip vote just failed; again in {s} s."),
        CallRefusal::Spectator => "Spectators can't call votes.".into(),
        CallRefusal::Loading => "Wait until the map has loaded.".into(),
        CallRefusal::NoGame => "Votes need a network game.".into(),
        other => format!("The vote was refused ({other:?})."),
    }
}

/// `AppMenu` as the menu is; the mouse freed on opening and taken back on
/// closing; every key pressed while open captured.
#[allow(clippy::too_many_arguments)]
fn sync(
    menu: Res<EscMenu>,
    main_menu: Option<Res<crate::main_menu::PartyMenu>>,
    controls: Option<Res<crate::controls::ControlsPage>>,
    console: Option<Res<chimera_client::client::console::ConsoleUi>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut app_menu: ResMut<AppMenu>,
    mut captured: ResMut<CapturedKeys>,
    mut cursor: Query<&mut CursorOptions>,
) {
    let open = menu.open || main_menu.is_some_and(|m| m.shown());
    if app_menu.open != open {
        app_menu.open = open;
        for mut c in &mut cursor {
            if open {
                release_cursor(&mut c);
            } else if !console.as_ref().is_some_and(|c| c.open) && !controls.as_ref().is_some_and(|c| c.open) {
                capture_cursor(&mut c);
            }
        }
    }
    if open {
        for k in keys.get_pressed() {
            if !captured.holds(*k) {
                captured.capture(*k);
            }
        }
    }
}

/// What the menu last showed: rebuilt when it differs.
#[derive(Default, PartialEq)]
struct Shown {
    page: Page,
    values: BTreeMap<String, String>,
    notice: Option<String>,
    vote: Option<(u32, u32, u32, Option<u8>, bool)>,
    host: bool,
    title: String,
    /// The app has the main menu (Disconnect shows).
    main_menu: bool,
}

#[allow(clippy::too_many_arguments)]
fn show(
    menu: Res<EscMenu>,
    title: Res<EscMenuTitle>,
    vote: Option<Res<VoteView>>,
    role: Option<Res<chimera_core::core::NetRole>>,
    main_menu: Option<Res<crate::main_menu::PartyMenu>>,
    root: Query<Entity, With<MenuRoot>>,
    mut shown: Local<Option<Shown>>,
    mut commands: Commands,
) {
    if !menu.open {
        if shown.take().is_some() {
            for e in &root {
                commands.entity(e).despawn();
            }
        }
        return;
    }
    let active = vote.as_ref().and_then(|v| v.active.clone());
    let now = Shown {
        page: menu.page,
        values: menu.values.clone(),
        notice: menu.notice.clone(),
        vote: active.as_ref().map(|v| (v.id, v.yes, v.no, v.mine, v.may_vote)),
        host: role.as_deref().is_none_or(|r| *r != chimera_core::core::NetRole::Client),
        title: title.0.clone(),
        main_menu: main_menu.is_some(),
    };
    if shown.as_ref() == Some(&now) {
        return;
    }
    for e in &root {
        commands.entity(e).despawn();
    }
    spawn_menu(&mut commands, &menu, &now, active.as_ref());
    *shown = Some(now);
}

/// Over the match-end screen (75) and the Tab scoreboard (80), under the
/// console (100) and the Controls page.
pub const MENU_Z: i32 = 90;
/// The vote panel: over the HUDs and the match-end screen, under the menu.
pub const VOTE_Z: i32 = 88;

/// The shade over the world and the panels: alphas as blended in display
/// space (`widgets::display_alpha`).
const SHADE: Color = Color::srgba(0.0, 0.0, 0.0, 0.55);
const PANEL: Color = Color::srgba(0.08, 0.09, 0.11, 0.96);
const BUTTON: Color = Color::srgb(0.2, 0.22, 0.26);
const ACTIVE: Color = Color::srgb(0.75, 0.45, 0.1);
const TEXT: Color = Color::srgb(0.95, 0.95, 0.95);
const DIM: Color = Color::srgb(0.68, 0.7, 0.74);
const ACCENT: Color = Color::srgb(1.0, 0.72, 0.3);

fn text(t: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (Text::new(t), TextFont::from_font_size(size), TextColor(color))
}

fn button(label: impl Into<String>, action: Action, color: Color, wide: f32) -> impl Bundle {
    (
        Interaction::default(),
        action,
        Node {
            width: px(wide),
            padding: UiRect::axes(px(10.0), px(7.0)),
            margin: UiRect::all(px(3.0)),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(color),
        children![text(label, 17.0, TEXT)],
    )
}

fn spawn_menu(commands: &mut Commands, menu: &EscMenu, shown: &Shown, vote: Option<&chimera_net::net::vote::VoteState>) {
    commands
        .spawn((
            MenuRoot,
            GlobalZIndex(MENU_Z),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.0),
                height: percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(display_alpha(SHADE)),
            bevy::ui::FocusPolicy::Block,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(px(20.0)),
                    min_width: px(360.0),
                    row_gap: px(4.0),
                    border: UiRect::top(px(3.0)),
                    ..default()
                },
                BackgroundColor(display_alpha(PANEL)),
                BorderColor::all(ACCENT),
            ))
            .with_children(|panel| {
                panel.spawn(text(shown.title.to_uppercase(), 24.0, ACCENT));
                match menu.page {
                    Page::Main => main_page(panel, shown, vote),
                    Page::Options(tab) => options_page(panel, menu, tab),
                    Page::Host => host_page(panel, menu),
                    Page::ConfirmDisconnect(n) => confirm_page(panel, n),
                }
                if let Some(n) = &menu.notice {
                    panel.spawn((Node { margin: UiRect::top(px(8.0)), ..default() }, children![text(n.clone(), 15.0, ACCENT)]));
                }
            });
        });
}

fn main_page(panel: &mut ChildSpawnerCommands, shown: &Shown, vote: Option<&chimera_net::net::vote::VoteState>) {
    panel.spawn((Node { margin: UiRect::bottom(px(8.0)), ..default() }, children![text("Paused for you only: the match goes on", 13.0, DIM)]));
    const WIDE: f32 = 320.0;
    for e in crate::main_menu::MenuData::shipped().escape {
        let visible = match e.when {
            crate::main_menu::When::Always => true,
            crate::main_menu::When::Host => shown.host,
            crate::main_menu::When::MainMenu => shown.main_menu,
        };
        if !visible {
            continue;
        }
        use crate::main_menu::EscDoes;
        let action = match e.does {
            EscDoes::Resume => Action::Resume,
            EscDoes::Options => Action::Options,
            EscDoes::Host => Action::Host,
            EscDoes::Disconnect => Action::Disconnect,
            EscDoes::Quit => Action::Quit,
            EscDoes::Vote => {
                vote_rows(panel, vote, &e.label, WIDE);
                continue;
            }
        };
        panel.spawn(button(e.label, action, BUTTON, WIDE));
    }
}

/// The vote's lines and buttons while one runs, else the button that calls
/// the skip vote.
fn vote_rows(panel: &mut ChildSpawnerCommands, vote: Option<&chimera_net::net::vote::VoteState>, label: &str, wide: f32) {
    match vote {
        Some(v) => {
            panel.spawn((Node { margin: UiRect::top(px(6.0)), ..default() }, children![text(vote_title(v), 15.0, TEXT)]));
            panel.spawn(text(format!("Yes {}  ·  No {}", v.yes, v.no), 14.0, DIM));
            if v.may_vote && v.mine.is_none() {
                panel
                    .spawn(Node { flex_direction: FlexDirection::Row, ..default() })
                    .with_children(|row| {
                        row.spawn(button("Vote yes (F1)", Action::VoteYes, ACTIVE, 154.0));
                        row.spawn(button("Vote no (F2)", Action::VoteNo, BUTTON, 154.0));
                    });
            } else if let Some(m) = v.mine {
                panel.spawn(text(if m == 1 { "You voted yes" } else { "You voted no" }, 14.0, DIM));
            }
        }
        None => {
            panel.spawn(button(label.to_string(), Action::SkipMap, BUTTON, wide));
        }
    }
}

/// A host's Disconnect: it ends the game for the people connected.
fn confirm_page(panel: &mut ChildSpawnerCommands, people: usize) {
    let who = if people == 1 { "1 player".to_string() } else { format!("{people} players") };
    panel.spawn((
        Node { margin: UiRect::vertical(px(8.0)), ..default() },
        children![text(format!("End the game for {who}?"), 17.0, TEXT)],
    ));
    panel.spawn((
        Node { margin: UiRect::bottom(px(8.0)), ..default() },
        children![text("You are the host: leaving closes the server.", 13.0, DIM)],
    ));
    panel
        .spawn(Node { flex_direction: FlexDirection::Row, ..default() })
        .with_children(|row| {
            row.spawn(button("End the game", Action::DisconnectYes, ACTIVE, 154.0));
            row.spawn(button("Stay", Action::Back, BUTTON, 154.0));
        });
}

fn setting_row(panel: &mut ChildSpawnerCommands, s: &Setting, index: usize, on_host: bool, value: &str) {
    panel
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, ..default() })
        .with_children(|line| {
            line.spawn((Node { width: px(220.0), ..default() }, children![text(s.label.clone(), 16.0, TEXT)]));
            line.spawn(button("<", Action::Step { host: on_host, index, dir: -1 }, BUTTON, 36.0));
            line.spawn((
                Node { width: px(170.0), justify_content: JustifyContent::Center, ..default() },
                children![text(s.pick.shown(value), 16.0, ACCENT)],
            ));
            line.spawn(button(">", Action::Step { host: on_host, index, dir: 1 }, BUTTON, 36.0));
            if on_host {
                let when = if s.live { "now" } else { "next match" };
                line.spawn((Node { width: px(90.0), margin: UiRect::left(px(8.0)), ..default() }, children![text(when, 13.0, DIM)]));
            }
        });
}

fn options_page(panel: &mut ChildSpawnerCommands, menu: &EscMenu, tab: usize) {
    let tabs = option_tabs();
    panel
        .spawn(Node { flex_direction: FlexDirection::Row, margin: UiRect::vertical(px(8.0)), ..default() })
        .with_children(|row| {
            for (i, (name, _)) in tabs.iter().enumerate() {
                row.spawn(button(name.clone(), Action::Tab(i), if i == tab { ACTIVE } else { BUTTON }, 96.0));
            }
            row.spawn(button("Binds", Action::Binds, BUTTON, 96.0));
        });
    if let Some((_, settings)) = tabs.get(tab) {
        for (i, s) in settings.iter().enumerate() {
            let v = menu.values.get(&s.cvar).cloned().unwrap_or_default();
            setting_row(panel, s, i, false, &v);
        }
    }
    panel.spawn((Node { margin: UiRect::top(px(10.0)), ..default() }, children![button("Back", Action::Back, BUTTON, 120.0)]));
}

fn host_page(panel: &mut ChildSpawnerCommands, menu: &EscMenu) {
    panel.spawn((
        Node { margin: UiRect::vertical(px(6.0)), ..default() },
        children![text("Host settings: your server's. \"next match\": from the next match on.", 13.0, DIM)],
    ));
    for (i, s) in host::settings().iter().enumerate() {
        let v = menu.values.get(&s.cvar).cloned().unwrap_or_default();
        setting_row(panel, s, i, true, &v);
    }
    panel.spawn((Node { margin: UiRect::top(px(10.0)), ..default() }, children![button("Back", Action::Back, BUTTON, 120.0)]));
}

/// A vote's first line ("Ann wants to skip the map").
pub fn vote_title(v: &chimera_net::net::vote::VoteState) -> String {
    if v.issue.eq_ignore_ascii_case(host::SKIP_ISSUE) {
        format!("{} wants to skip the map", v.caller)
    } else {
        format!("{} called a vote: {} {}", v.caller, v.issue, v.detail).trim_end().to_string()
    }
}

/// The vote panel's lines now (empty: nothing shown): the vote running,
/// else how the last ended (for `ENDED_SHOWN` s).
pub fn vote_lines(view: &VoteView, quorum: f32, now: f64) -> Vec<String> {
    if let Some(v) = &view.active {
        let left = (view.ends - now).max(0.0).ceil() as u32;
        let need = chimera_net::net::vote::needed(v.eligible, quorum);
        let ask = match v.mine {
            _ if !v.may_vote => "You can't vote on this one".to_string(),
            None => "F1: yes    F2: no".to_string(),
            Some(1) => "You voted yes".to_string(),
            Some(_) => "You voted no".to_string(),
        };
        return vec![vote_title(v), ask, format!("Yes {}  ·  No {}  ·  {need} needed  ·  {left} s", v.yes, v.no)];
    }
    if let Some((VoteNews::Ended { issue, failure, .. }, at)) = &view.ended
        && now - at < ENDED_SHOWN
    {
        let what = if issue.eq_ignore_ascii_case(host::SKIP_ISSUE) { "skipping the map" } else { issue.as_str() };
        return vec![match failure {
            None => format!("Vote passed: {what}"),
            Some(VoteFailure::Quorum) => "Vote failed: not enough players voted".to_string(),
            Some(VoteFailure::YesNo) => "Vote failed: not enough players voted yes".to_string(),
        }];
    }
    Vec::new()
}

/// Seconds a vote's end stays on the panel.
pub const ENDED_SHOWN: f64 = 5.0;

/// The vote panel's lines as last drawn.
#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct VotePanel(pub Vec<String>);

fn show_vote(
    view: Option<Res<VoteView>>,
    settings: Option<Res<VoteSettings>>,
    time: Res<Time<Real>>,
    root: Query<Entity, With<VoteRoot>>,
    mut panel: ResMut<VotePanel>,
    mut commands: Commands,
) {
    let Some(view) = view else { return };
    let quorum = settings.map_or(0.6, |s| s.quorum_ratio);
    let lines = vote_lines(&view, quorum, time.elapsed_secs_f64());
    if panel.0 == lines {
        return;
    }
    for e in &root {
        commands.entity(e).despawn();
    }
    if !lines.is_empty() {
        commands
            .spawn((
                VoteRoot,
                GlobalZIndex(VOTE_Z),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(16.0),
                    top: percent(28.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::axes(px(14.0), px(10.0)),
                    row_gap: px(3.0),
                    border: UiRect::left(px(3.0)),
                    ..default()
                },
                BackgroundColor(display_alpha(PANEL)),
                BorderColor::all(ACCENT),
            ))
            .with_children(|p| {
                for (i, l) in lines.iter().enumerate() {
                    let (size, color) = if i == 0 { (17.0, TEXT) } else { (14.0, if i == 1 { ACCENT } else { DIM }) };
                    p.spawn(text(l.clone(), size, color));
                }
            });
    }
    panel.0 = lines;
}
