//! The party's Controls page (feature `present`): the key binds of the
//! party in layers, saved in the party's config (`config.cfg`, as the
//! console writes it).
//!
//! - The shared binds (`Console::binds`: move, jump, fire, reload, use, the
//!   mouse wheel ...) apply to every player in every game.
//! - Each game has binds of its own (`Console::layers`, `GAME_BINDS` ships
//!   them) that apply while that game is the player's identity (the gun
//!   game's `identity = "weapon"`): Combat Arms sprints on Shift and
//!   walks on C, CS:S walks on Shift (`+speed`). The local input reads the
//!   game's own binds first, then the shared ones (`Console::
//!   effective_binds`), before the intent is sent, so prediction is not
//!   involved. A later game adds a row list (`GAMES`) and ships binds.
//! - The page (`F6`, `controls [tab]`, or the Escape menu's Options > Binds): a tab for "Shared" first, then
//!   one for each game; a row is an action and its keys; clicking the keys
//!   waits for the next key, mouse button or wheel notch (the wheel is a
//!   bind like any: `mwheeldown` on Jump bunny hops in both games); Esc
//!   cancels; "Clear" and "Defaults" per row and tab; "Done" saves the
//!   config. The rows are the page's own small tables over the console's
//!   binds (`chimera_client::client::binds`' `keys_for`, `rebind_line`,
//!   `clear_line`, `first_pressed`), not a copy of either game's options
//!   (CS:S's keyboard dialog, Combat Arms' profile screen).
//!
//! Each tab also has its numbers (`Setting`, a `-` and a `+` over a saved
//! cvar): the shared tab the mouse sensitivity, each game's tab its gun
//! view FOV (`viewmodel_fov`, `ca_viewmodel_fov`).
//!
//! Known shortcuts: the rows are the movement and combat basics (each
//! game's full lists are in its own options); the other settings aren't
//! on the page; a left click doesn't
//! bind while the pointer is on the page's buttons (it picks the row);
//! the page is drawn plainly, not in a game's style.

use std::collections::BTreeMap;

use bevy::{
    input::{ButtonInput, mouse::AccumulatedMouseScroll},
    prelude::*,
    window::CursorOptions,
};
use chimera_client::client::{binds, input::release_cursor, widgets::display_alpha};
use chimera_core::console::{Console, ConsoleAppExt, quote};

/// An action: its words and the console command its keys run.
#[derive(Clone, Copy, Debug)]
pub struct Row {
    pub label: &'static str,
    pub command: &'static str,
}

const fn row(label: &'static str, command: &'static str) -> Row {
    Row { label, command }
}

/// The shared binds' rows.
pub const SHARED: &[Row] = &[
    row("Move forward", "+forward"),
    row("Move back", "+back"),
    row("Move left", "+moveleft"),
    row("Move right", "+moveright"),
    row("Jump", "+jump"),
    row("Fire", "+attack"),
    row("Secondary fire", "+attack2"),
    row("Reload", "+reload"),
    row("Use", "+use"),
    row("Last weapon", "lastinv"),
];

/// A number the page sets (a saved cvar): `-` and `+` buttons move it by
/// `step` within `min..=max`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Setting {
    pub label: &'static str,
    pub cvar: &'static str,
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

/// The shared tab's numbers: the mouse look (every game's; the party keeps
/// it in the shell's `sensitivity`, not in Combat Arms' profile).
pub const SHARED_SETTINGS: &[Setting] =
    &[Setting { label: "Mouse sensitivity", cvar: "sensitivity", min: 0.1, max: 20.0, step: 0.1 }];

/// A game's page: its id, the tab's name, the rows only it has and its
/// numbers.
#[derive(Clone, Copy, Debug)]
pub struct GameRows {
    pub game: &'static str,
    pub label: &'static str,
    pub rows: &'static [Row],
    pub settings: &'static [Setting],
}

/// Each game's own rows, in tab order (after "Shared").
pub const GAMES: &[GameRows] = &[
    GameRows {
        game: combat_arms::game::GAME,
        label: "Combat Arms",
        rows: &[row("Sprint", "+sprint"), row("Walk", "+walk"), row("Crouch", "+duck")],
        settings: &[Setting { label: "Gun view FOV", cvar: "ca_viewmodel_fov", min: 63.74, max: 110.0, step: 1.0 }],
    },
    GameRows {
        game: cs_source::game::GAME,
        label: "Counter-Strike: Source",
        rows: &[row("Walk", "+speed"), row("Duck", "+duck")],
        settings: &[Setting { label: "Gun view FOV", cvar: "viewmodel_fov", min: 54.0, max: 110.0, step: 1.0 }],
    },
];

/// Tab 0 is "Shared", tab `n` is `GAMES[n - 1]`.
pub fn tab_count() -> usize {
    GAMES.len() + 1
}

pub fn tab_label(tab: usize) -> &'static str {
    tab.checked_sub(1).and_then(|i| GAMES.get(i)).map_or("Shared", |g| g.label)
}

/// The game a tab is of (None: the shared binds).
pub fn tab_game(tab: usize) -> Option<&'static str> {
    tab.checked_sub(1).and_then(|i| GAMES.get(i)).map(|g| g.game)
}

pub fn tab_rows(tab: usize) -> &'static [Row] {
    tab.checked_sub(1).and_then(|i| GAMES.get(i)).map_or(SHARED, |g| g.rows)
}

/// The tab named `name` (a game's id or label, or "shared"); case-blind.
pub fn tab_named(name: &str) -> Option<usize> {
    (0..tab_count()).find(|t| {
        let (label, game) = (tab_label(*t), tab_game(*t));
        label.eq_ignore_ascii_case(name) || game.is_some_and(|g| g.eq_ignore_ascii_case(name))
    })
}

pub fn tab_settings(tab: usize) -> &'static [Setting] {
    tab.checked_sub(1).and_then(|i| GAMES.get(i)).map_or(SHARED_SETTINGS, |g| g.settings)
}

/// `value` moved one step of `setting` (dir -1 or 1), within its range.
pub fn nudged(setting: &Setting, value: f32, dir: i8) -> f32 {
    let next = (value + setting.step * dir as f32).clamp(setting.min, setting.max);
    (next * 100.0).round() / 100.0
}

/// The keys `command` has on a tab: the shared binds', or the game's own.
pub fn keys_on(console: &Console, tab: usize, command: &str) -> Vec<String> {
    match tab_game(tab) {
        None => binds::keys_for(&console.binds, command).into_iter().map(String::from).collect(),
        Some(game) => console
            .layers
            .get(game)
            .map(|l| binds::keys_for(l, command).into_iter().map(String::from).collect())
            .unwrap_or_default(),
    }
}

/// The shared keys a game's own bind of `command` hides (its keys in
/// `shared` that the game doesn't bind itself): the game's overlay keeps
/// the command off them, so a game's own key replaces the shared one.
fn shared_keys_to_hide(console: &Console, game: &str, command: &str, keep: Option<&str>) -> Vec<String> {
    let own = console.layers.get(game);
    binds::keys_for(&console.binds, command)
        .into_iter()
        .filter(|k| Some(*k) != keep && own.is_none_or(|l| !l.contains_key(*k)))
        .map(String::from)
        .collect()
}

/// The console line that makes `key` the only key of `command` on `tab`.
pub fn rebind_line(console: &Console, tab: usize, command: &str, key: &str) -> String {
    let Some(game) = tab_game(tab) else {
        return binds::rebind_line(&console.binds, command, key);
    };
    let mut lines: Vec<String> = console
        .layers
        .get(game)
        .map(|l| binds::keys_for(l, command))
        .unwrap_or_default()
        .into_iter()
        .filter(|k| *k != key)
        .map(|k| format!("unbindgame {game} {}", quote(k)))
        .collect();
    lines.extend(
        shared_keys_to_hide(console, game, command, Some(key))
            .into_iter()
            .map(|k| format!("bindgame {game} {} \"\"", quote(&k))),
    );
    lines.push(format!("bindgame {game} {} {}", quote(key), quote(command)));
    lines.join("; ")
}

/// The console line that clears `command`'s keys on `tab` (None: none).
pub fn clear_line(console: &Console, tab: usize, command: &str) -> Option<String> {
    let Some(game) = tab_game(tab) else {
        return binds::clear_line(&console.binds, command);
    };
    let mut lines: Vec<String> = console
        .layers
        .get(game)
        .map(|l| binds::keys_for(l, command))
        .unwrap_or_default()
        .into_iter()
        .map(|k| format!("unbindgame {game} {}", quote(k)))
        .collect();
    lines.extend(
        shared_keys_to_hide(console, game, command, None)
            .into_iter()
            .map(|k| format!("bindgame {game} {} \"\"", quote(&k))),
    );
    (!lines.is_empty()).then(|| lines.join("; "))
}

/// The console line for "Defaults" on `tab`.
pub fn defaults_line(tab: usize) -> String {
    match tab_game(tab) {
        None => "binddefaults".into(),
        Some(game) => format!("bindgamedefaults {game}"),
    }
}

/// The page's state.
#[derive(Resource, Default, Debug, PartialEq)]
pub struct ControlsPage {
    pub open: bool,
    pub tab: usize,
    /// The row waiting for a key: its command.
    pub capture: Option<&'static str>,
    /// The numbers' values as the cvars hold them (refreshed while open).
    pub values: BTreeMap<&'static str, String>,
}

/// What a click on the page does.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Tab(usize),
    Rebind(&'static str),
    Clear(&'static str),
    Defaults,
    Done,
    /// A number's `-` (-1) or `+` (1).
    Nudge(&'static Setting, i8),
}

#[derive(Component)]
struct PageRoot;

/// The page's systems (the Escape menu's run before them: Esc is the
/// page's while it is open).
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ControlsSystems;

pub struct ControlsPlugin;

impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ControlsPage>()
            .console_command("chimera:controls", "controls [shared|combat_arms|cs_source]: the key binds page.", |w, a| {
                let tab = match a.first() {
                    Some(name) => tab_named(name).ok_or_else(|| format!("no tab `{name}`"))?,
                    None => w.resource::<ControlsPage>().tab,
                };
                let mut page = w.resource_mut::<ControlsPage>();
                page.open = !(page.open && a.is_empty());
                page.tab = tab;
                page.capture = None;
                Ok(None)
            })
            .add_systems(Update, (toggle, refresh_values, capture, click, show).chain().in_set(ControlsSystems));
    }
}

fn toggle(keys: Res<ButtonInput<KeyCode>>, mut page: ResMut<ControlsPage>) {
    if keys.just_pressed(KeyCode::F6) {
        page.open = !page.open;
        page.capture = None;
    } else if page.open && page.capture.is_none() && keys.just_pressed(KeyCode::Escape) {
        page.open = false;
    }
}

/// A row waiting for a key: the next key, button or wheel notch becomes
/// its bind; Esc cancels.
fn capture(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    scroll: Res<AccumulatedMouseScroll>,
    hovered: Query<&Interaction, With<Action>>,
    mut page: ResMut<ControlsPage>,
    mut console: ResMut<Console>,
) {
    let Some(command) = page.capture else { return };
    if keys.just_pressed(KeyCode::Escape) {
        page.capture = None;
        return;
    }
    let on_page = hovered.iter().any(|i| *i != Interaction::None);
    // The click that picks a row isn't the key (and a click on another of
    // the page's buttons is a click).
    let mouse_only_off_page = if on_page { ButtonInput::<MouseButton>::default() } else { mouse.clone() };
    if let Some(key) = binds::first_pressed(&keys, &mouse_only_off_page, Some(&scroll)) {
        let line = rebind_line(&console, page.tab, command, key);
        console.submit(line);
        page.capture = None;
    }
}

/// A cvar's value as the console reads it.
pub fn cvar_value(world: &mut World, name: &str) -> Option<String> {
    let cvar = world.resource::<Console>().cvar(name).cloned()?;
    (cvar.get)(world)
}

/// While the page is open, the values of the numbers it shows.
fn refresh_values(world: &mut World) {
    let page = world.resource::<ControlsPage>();
    if !page.open {
        return;
    }
    let tab = page.tab;
    for s in tab_settings(tab) {
        let v = cvar_value(world, s.cvar).unwrap_or_default();
        let mut page = world.resource_mut::<ControlsPage>();
        if page.values.get(s.cvar) != Some(&v) {
            page.values.insert(s.cvar, v);
        }
    }
}

fn click(
    pressed: Query<(&Interaction, &Action), Changed<Interaction>>,
    mut page: ResMut<ControlsPage>,
    mut console: ResMut<Console>,
    mut commands: Commands,
) {
    for (interaction, action) in &pressed {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            Action::Tab(t) => {
                page.tab = t;
                page.capture = None;
            }
            Action::Rebind(command) => page.capture = Some(command),
            Action::Clear(command) => {
                if let Some(line) = clear_line(&console, page.tab, command) {
                    console.submit(line);
                }
            }
            Action::Defaults => {
                console.submit(defaults_line(page.tab));
            }
            Action::Nudge(setting, dir) => {
                commands.queue(move |w: &mut World| {
                    let now = cvar_value(w, setting.cvar).and_then(|v| v.parse::<f32>().ok()).unwrap_or(setting.min);
                    let next = nudged(setting, now, dir);
                    w.resource_mut::<Console>().submit(format!("{} {next}", setting.cvar));
                });
            }
            Action::Done => {
                console.submit("host_writeconfig");
                page.open = false;
                page.capture = None;
            }
        }
    }
}

/// What the page last showed: rebuilt when it differs.
#[derive(Default, PartialEq)]
struct Shown {
    tab: usize,
    capture: Option<&'static str>,
    binds: BTreeMap<String, String>,
    layers: BTreeMap<String, BTreeMap<String, String>>,
    values: BTreeMap<&'static str, String>,
}

fn show(
    page: Res<ControlsPage>,
    console: Res<Console>,
    root: Query<Entity, With<PageRoot>>,
    mut cursor: Query<&mut CursorOptions>,
    mut shown: Local<Option<Shown>>,
    mut commands: Commands,
) {
    if !page.open {
        if shown.take().is_some() {
            for e in &root {
                commands.entity(e).despawn();
            }
        }
        return;
    }
    let now = Shown {
        tab: page.tab,
        capture: page.capture,
        binds: console.binds.clone(),
        layers: console.layers.clone(),
        values: page.values.clone(),
    };
    if shown.as_ref() == Some(&now) {
        return;
    }
    if shown.is_none() {
        // Opening: the mouse is for the page.
        for mut c in &mut cursor {
            release_cursor(&mut c);
        }
    }
    for e in &root {
        commands.entity(e).despawn();
    }
    spawn(&mut commands, &page, &console);
    *shown = Some(now);
}

/// The shade over the world and the panel: alphas as blended in display
/// space (`widgets::display_alpha`).
const SHADE: Color = Color::srgba(0.0, 0.0, 0.0, 0.5);
const PANEL: Color = Color::srgba(0.08, 0.09, 0.11, 0.96);
const BUTTON: Color = Color::srgb(0.2, 0.22, 0.26);
const ACTIVE: Color = Color::srgb(0.75, 0.45, 0.1);

fn text(t: impl Into<String>, size: f32) -> impl Bundle {
    (Text::new(t), TextFont::from_font_size(size), TextColor(Color::WHITE))
}

fn button(label: impl Into<String>, action: Action, color: Color, wide: f32) -> impl Bundle {
    (
        Interaction::default(),
        action,
        Node {
            width: px(wide),
            padding: UiRect::axes(px(8.0), px(4.0)),
            margin: UiRect::all(px(2.0)),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(color),
        children![text(label, 15.0)],
    )
}

fn spawn(commands: &mut Commands, page: &ControlsPage, console: &Console) {
    commands
        .spawn((
            PageRoot,
            GlobalZIndex(1000),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.0),
                height: percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(display_alpha(SHADE)),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(px(16.0)),
                    min_width: px(520.0),
                    row_gap: px(4.0),
                    ..default()
                },
                BackgroundColor(display_alpha(PANEL)),
            ))
            .with_children(|panel| {
                panel.spawn(text("Controls", 22.0));
                panel
                    .spawn(Node { flex_direction: FlexDirection::Row, margin: UiRect::vertical(px(6.0)), ..default() })
                    .with_children(|tabs| {
                        for t in 0..tab_count() {
                            let color = if t == page.tab { ACTIVE } else { BUTTON };
                            tabs.spawn(button(tab_label(t), Action::Tab(t), color, 150.0));
                        }
                    });
                if page.tab == 0 {
                    panel.spawn(text("Every game: the mouse wheel can be a key (Jump on a wheel notch).", 13.0));
                } else {
                    panel.spawn(text(
                        format!("{} only: these keys replace the shared ones while you play it.", tab_label(page.tab)),
                        13.0,
                    ));
                }
                for r in tab_rows(page.tab) {
                    let waiting = page.capture == Some(r.command);
                    let keys = keys_on(console, page.tab, r.command);
                    let shown = if waiting {
                        "press a key, button or wheel (Esc: cancel)".to_string()
                    } else if keys.is_empty() {
                        "-".to_string()
                    } else {
                        keys.iter().map(|k| binds::display(k)).collect::<Vec<_>>().join(", ")
                    };
                    panel
                        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, ..default() })
                        .with_children(|line| {
                            line.spawn((Node { width: px(150.0), ..default() }, children![text(r.label, 15.0)]));
                            let color = if waiting { ACTIVE } else { BUTTON };
                            line.spawn(button(shown, Action::Rebind(r.command), color, 280.0));
                            line.spawn(button("Clear", Action::Clear(r.command), BUTTON, 60.0));
                        });
                }
                for s in tab_settings(page.tab) {
                    let value = page.values.get(s.cvar).cloned().unwrap_or_default();
                    panel
                        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, ..default() })
                        .with_children(|line| {
                            line.spawn((Node { width: px(150.0), ..default() }, children![text(s.label, 15.0)]));
                            line.spawn(button("-", Action::Nudge(s, -1), BUTTON, 40.0));
                            line.spawn((
                                Node { width: px(120.0), justify_content: JustifyContent::Center, ..default() },
                                children![text(value, 15.0)],
                            ));
                            line.spawn(button("+", Action::Nudge(s, 1), BUTTON, 40.0));
                        });
                }
                panel
                    .spawn(Node { flex_direction: FlexDirection::Row, margin: UiRect::top(px(10.0)), ..default() })
                    .with_children(|bottom| {
                        bottom.spawn(button("Defaults", Action::Defaults, BUTTON, 100.0));
                        bottom.spawn(button("Done", Action::Done, BUTTON, 100.0));
                    });
            });
        });
}
