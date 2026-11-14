//! The in-game console (docs/plans/active/console.md): toggled with `~`.
//! Drop-down output with colours by severity, scrollback, a filter and
//! timestamps; an input line with Tab completion (names, cvar values,
//! command arguments; repeated Tab cycles), ranked fuzzy suggestions as
//! you type, history (Up/Down, persisted, Ctrl+R reverse search), and the
//! usual editing keys. Binds run when it's closed (`+action` while held).
//! Also the CS:S-style overlays (cl_showpos, cl_showfps, snd_show, watch; `net_graph` is `net_graph`'s)
//! and the client's commands (noclip, getpos/setpos, kill, map, quit, ...).
//!
//! The in-game console UI (`~`): completion (names, cvar values, each argument
//! of the command typed: keys, maps, entities, teams, hitgroups, tabs, ...; on
//! the line's last command after `;`), fuzzy suggestions under it with Up/Down
//! picking one (Enter or Tab takes it and adds a space, Esc hides the list), a
//! help line for the command typed (its usage from the help text with the
//! argument being typed marked, a cvar's value, default, range and values),
//! history (Up/Down with an empty line, persisted, Ctrl+R), editing keys
//! (Ctrl+Backspace/W/Delete words, Ctrl+U/K, Ctrl+V paste, Ctrl+L clear,
//! Ctrl+Home/End), output lines that fit the window's height,
//! scrollback/filter/timestamps, drag-selecting text in the output (Ctrl+C copies it; clicks never
//! grab the mouse while open), binds with `+`/`-` actions (F1 and F2
//! vote while a vote waits for ours: `VOTE_KEYS`), log mirroring,
//! overlays (cl_showpos, cl_showfps, snd_show, watch), client commands (noclip,
//! god, getpos/setpos/setang, kill, `chimera_hurtme` (a bullet hit on yourself
//! at a hitbox), map (loaded on a task, spawned in place a step at a time
//! behind the loading screen with the simulation waiting (until its
//! colliders, which follow the map's frame, are all in),
//! `map::prepare::begin_map_change`, `SPAWN_BUDGET` a frame and the GPU's
//! uploads capped, `LOAD_UPLOAD_BYTES`; then the map's `MapHooks::settle` work
//! a slice a frame, at most `SETTLE_MAX_SECS`, a frame advancing the
//! simulation by at most one tick, before the game is entered; `map greybox` puts the
//! greybox back, `load_greybox`), disconnect (the greybox, then the main menu),
//! toggleconsole, quit), `developer 1` notify lines, the `chimera/console`
//! remote method.

use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex, OnceLock},
};

use bevy::{
    app::AppExit,
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
        mouse::{MouseScrollUnit, MouseWheel},
    },
    log::{BoxedLayer, tracing_subscriber::Layer},
    prelude::*,
    window::CursorOptions,
};

use super::{FirstPersonCamera, fonts::UiFonts, widgets::Interaction};
use crate::{
    console::{Console, ConsoleAppExt, Level, cfg_dir, parse, resource_cvar},
    core::{LocalPlayer, MovementState, Velocity},
    game_registry::{GameInfo, Games},
    map::PlaySound,
};

pub struct ConsoleUiPlugin;

impl Plugin for ConsoleUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ConsoleUi>()
            .init_resource::<MapLists>()
            .init_resource::<Overlays>()
            .init_resource::<HeldActions>()
            .init_resource::<SoundMarks>()
            .add_systems(Startup, (spawn_ui, load_history, autoexec).chain())
            .add_systems(Startup, read_map_lists_ahead)
            .add_systems(Update, take_map_lists_ahead)
            .add_systems(
                Update,
                (
                    toggle,
                    edit.run_if(|ui: Res<ConsoleUi>| ui.open),
                    // Not in the game menu or while typing chat either:
                    // keys there drive those.
                    run_binds.run_if(
                        |ui: Res<ConsoleUi>,
                         menu: Option<Res<super::front_end::MenuStatus>>,
                         chat: Option<Res<super::chat::ChatInput>>| {
                            !ui.open && !menu.is_some_and(|m| m.open) && !chat.is_some_and(|c| c.open.is_some())
                        },
                    ),
                    drain_log,
                    record_sounds,
                    draw_console,
                    select_text,
                    draw_overlays,
                    draw_notify,
                    finish_map_load,
                    draw_sound_marks,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    overlay_text,
                    watch_text,
                    follow_map_cycle.run_if(resource_exists::<crate::core::MapCycle>),
                ),
            )
            .add_systems(Last, save_on_exit);
        overlay_cvars(app);
        client_commands(app);
    }
}

/// Whether the console is open, and its input state.
#[derive(Resource, Default)]
pub struct ConsoleUi {
    pub open: bool,
    input: String,
    /// Cursor position in chars.
    cursor: usize,
    history: Vec<String>,
    /// Position while browsing history (None: editing a new line).
    browsing: Option<usize>,
    /// Tab cycling: the candidates and which one is shown, and the input
    /// they complete.
    cycle: Option<(Vec<String>, usize, String)>,
    /// Ctrl+R: the query and how many matches back.
    search: Option<(String, usize)>,
    /// Lines scrolled up from the bottom.
    scroll: usize,
    pub filter: String,
    pub timestamps: bool,
    /// `con_enable 0`: the console key doesn't open it (the options'
    /// "Enable developer console", `game_menu`).
    pub disabled: bool,
    /// `toggleconsole` asked to open or close it (next frame).
    toggle_request: bool,
    /// The output lines on screen, and a selection of them (first and
    /// last row, in the order dragged) made with the mouse.
    shown: Vec<String>,
    selection: Option<(TextPos, TextPos)>,
    dragging: bool,
    /// `draw_console` rebuilt the rows this frame.
    fresh_rows: bool,
    /// The suggestion picked with Up/Down (an index into all of them).
    highlight: Option<usize>,
    /// The input the suggestions were hidden at with Esc (they come back
    /// when it changes).
    dismissed: Option<String>,
    /// Whether the suggestion list was shown last frame (Esc then hides
    /// it instead of closing the console).
    list_shown: bool,
    /// Output lines that fit the console at the window's size.
    visible_lines: usize,
    /// What the suggestion box shows, to rebuild it only on change.
    suggest_rows: Vec<SuggestRow>,
}

const HISTORY_MAX: usize = 1000;
/// Output lines before the window's size is known.
const DEFAULT_VISIBLE_LINES: usize = 28;
const SUGGESTIONS: usize = 8;
/// Output text size and the height of one of its rows (pixels).
const OUTPUT_FONT: f32 = 14.0;
const ROW_HEIGHT: f32 = 17.0;

impl ConsoleUi {
    fn page(&self) -> usize {
        (self.visible_lines.max(2)) / 2
    }
}

/// One row of the suggestion box: coloured runs, and whether it is the
/// highlighted suggestion.
#[derive(Clone, Debug, PartialEq)]
struct SuggestRow {
    runs: Vec<(String, Color)>,
    highlight: bool,
}

/// Output lines that fit a console half the window tall (padding, the
/// input line and the "more below" line taken off).
fn lines_for_height(window_height: f32) -> usize {
    (((window_height * 0.5 - 16.0 - 26.0) / ROW_HEIGHT).floor() as isize - 1).max(4) as usize
}

#[derive(Component)]
pub struct ConsoleRoot;
#[derive(Component)]
struct ConsoleOutput;
/// One row of the output on screen.
#[derive(Component)]
struct ConsoleRow(usize);
#[derive(Component)]
struct ConsoleInput;
#[derive(Component)]
struct ConsoleSuggest;
#[derive(Component)]
struct OverlayText;

/// The console's text: GameUI's `ConsoleText` face (Lucida Console) at our
/// sizes (the console is laid out by us, `ROW_HEIGHT`).
fn console_font(fonts: &UiFonts, size: f32) -> TextFont {
    fonts.source_face("ConsoleText", "Lucida Console", size)
}

/// The engine's overlay text (notify lines, `cl_showpos`, `snd_show`):
/// GameUI's `DefaultFixedOutline` face.
fn overlay_font(fonts: &UiFonts, size: f32) -> TextFont {
    fonts.source_face("DefaultFixedOutline", "Lucida Console", size)
}

fn spawn_ui(mut commands: Commands, fonts: Res<UiFonts>) {
    let font = |size: f32| console_font(&fonts, size);
    let overlay = |size: f32| overlay_font(&fonts, size);
    commands
        .spawn((
            ConsoleRoot,
            Node {
                position_type: PositionType::Absolute,
                top: px(0.0),
                left: px(0.0),
                width: percent(100.0),
                height: percent(50.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::FlexEnd,
                padding: UiRect::all(px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.07, 0.88)),
            // A click on the console stops there, not on a menu under it.
            bevy::ui::FocusPolicy::Block,
            GlobalZIndex(100),
            Visibility::Hidden,
        ))
        .with_children(|c| {
            c.spawn((
                ConsoleOutput,
                // Fills the console above the input line and clips at its
                // top, whatever the window's size.
                Node {
                    overflow: Overflow::clip(),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::FlexEnd,
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    min_height: px(0.0),
                    ..default()
                },
            ));
            c.spawn((
                ConsoleInput,
                Text::default(),
                font(15.0),
                TextColor(Color::srgb(1.0, 0.95, 0.6)),
                Node {
                    margin: UiRect::top(px(4.0)),
                    ..default()
                },
            ));
        });
    commands.spawn((
        ConsoleSuggest,
        Node {
            position_type: PositionType::Absolute,
            top: percent(50.0),
            left: px(8.0),
            max_width: percent(96.0),
            padding: UiRect::all(px(6.0)),
            flex_direction: FlexDirection::Column,
            ..default()
        },
        BackgroundColor(Color::srgba(0.1, 0.1, 0.14, 0.95)),
        bevy::ui::FocusPolicy::Block,
        GlobalZIndex(101),
        Visibility::Hidden,
    ));
    commands.spawn((
        NotifyText,
        Text::default(),
        overlay(14.0),
        TextColor(Color::srgb(0.95, 0.95, 0.85)),
        Node {
            position_type: PositionType::Absolute,
            // Below the debug overlay's five lines.
            top: px(125.0),
            left: px(8.0),
            ..default()
        },
        GlobalZIndex(50),
        Visibility::Hidden,
    ));
    commands.spawn((
        OverlayText,
        Text::default(),
        overlay(14.0),
        TextColor(Color::srgb(0.9, 0.9, 0.9)),
        Node {
            position_type: PositionType::Absolute,
            top: px(8.0),
            right: px(10.0),
            ..default()
        },
        GlobalZIndex(50),
    ));
}

pub fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mut ui: ResMut<ConsoleUi>,
    mut root: Single<&mut Visibility, With<ConsoleRoot>>,
    mut cursor: Single<&mut CursorOptions>,
    menu: Option<Res<super::front_end::MenuStatus>>,
    debug_ui: Option<Res<super::debug_ui::DebugUi>>,
) {
    // Esc hides the suggestions first (`edit`), then closes.
    let close = ui.open && keys.just_pressed(KeyCode::Escape) && ui.search.is_none() && !ui.list_shown;
    let requested = std::mem::take(&mut ui.toggle_request);
    let key = keys.just_pressed(KeyCode::Backquote) && (ui.open || !ui.disabled);
    if key || close || requested {
        ui.open = !ui.open;
        **root = if ui.open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if ui.open {
            // Typing shouldn't move the player: the game reads input only
            // while the mouse is grabbed.
            super::input::release_cursor(&mut cursor);
        } else if !close && !menu.is_some_and(|m| m.open) && !debug_ui.is_some_and(|d| d.open) {
            // Closed with the console key: straight back to playing, unless
            // the game menu or the debug UI is open under it. (Escape
            // leaves the mouse free.)
            super::input::capture_cursor(&mut cursor);
        }
    }
}

/// Every completion candidate for the current input: names when on the
/// first word, else the command's arguments (the argument being typed
/// decides which). Works on the line's last command (after `;`).
fn candidates(world: &mut World, input: &str) -> (String, Vec<(String, String)>) {
    let at = crate::console::last_command_start(input);
    let (prefix, input) = input.split_at(at);
    let ends_space = input.ends_with(' ');
    let words: Vec<&str> = input.split_whitespace().collect();
    let (head, token) = if words.len() <= 1 && !ends_space {
        (String::new(), words.first().copied().unwrap_or(""))
    } else {
        let token = if ends_space {
            ""
        } else {
            words.last().copied().unwrap_or("")
        };
        let keep = if ends_space { words.len() } else { words.len() - 1 };
        (format!("{} ", words[..keep].join(" ")), token)
    };
    let arg = crate::console::arg_position(input).unwrap_or(0);
    let console = world.resource::<Console>();
    let mut out: Vec<(String, String)> = Vec::new();
    let plain = |v: &[&str]| v.iter().map(|x| (x.to_string(), String::new())).collect::<Vec<_>>();
    // Names as they are typed here: short when that means them in this
    // scope, else in full (`Console::display_name`); with a namespace
    // typed, in full.
    let shown = |name: &str| {
        if token.contains(':') {
            name.to_string()
        } else {
            console.display_name(name)
        }
    };
    if head.is_empty() {
        for v in console.cvars() {
            out.push((shown(&v.name), v.help.clone()));
        }
        for c in console.commands() {
            out.push((shown(&c.name), c.help.clone()));
        }
        for (a, body) in &console.aliases {
            out.push((a.clone(), format!("alias: {body}")));
        }
    } else {
        let typed = words[0].to_lowercase();
        // The command's short name (`chimera_hurtme`, `css:bot_add`: by
        // what they resolve to).
        let cmd = console
            .full_name(&typed)
            .map_or(typed.clone(), |f| crate::console::short_name(&f).to_string());
        if let Some(v) = console.cvar(&typed)
            && arg == 0
        {
            out.extend(v.values.iter().map(|x| (x.clone(), String::new())));
            // A small whole-number range: each value.
            if let Some((a, b)) = v.range
                && v.integer
                && b - a <= 8.0
            {
                out.extend((a as i32..=b as i32).map(|x| (x.to_string(), String::new())));
            }
            out.push((v.default.clone(), "default".into()));
        }
        let bound = |n: &str| console.binds.get(n).map_or(String::new(), |v| format!("bound to \"{v}\""));
        match cmd.as_str() {
            "help" | "reset" | "toggle" | "incrementvar" | "watch" | "unwatch" | "cvarlist" | "find" if arg == 0 => {
                out.extend(console.cvars().map(|v| (shown(&v.name), v.help.clone())));
                if cmd == "help" || cmd == "find" {
                    out.extend(console.commands().map(|c| (shown(&c.name), c.help.clone())));
                }
            }
            "cmdlist" if arg == 0 => out.extend(console.commands().map(|c| (shown(&c.name), c.help.clone()))),
            "exec" | "execifexists" => {
                if let Some(dir) = cfg_dir()
                    && let Ok(files) = std::fs::read_dir(dir)
                {
                    out.extend(files.flatten().filter_map(|f| {
                        let n = f.file_name().to_string_lossy().to_string();
                        n.ends_with(".cfg")
                            .then(|| (n.trim_end_matches(".cfg").to_string(), String::new()))
                    }));
                }
            }
            "unbind" if arg == 0 => {
                out.extend(console.binds.iter().map(|(k, v)| (k.clone(), format!("bound to \"{v}\""))));
            }
            "bind" if arg == 0 => {
                out.extend(
                    super::binds::KEY_NAMES
                        .iter()
                        .map(|(_, n)| *n)
                        .chain(super::binds::MOUSE_NAMES.iter().map(|(_, n)| *n))
                        .chain([super::binds::WHEEL_UP, super::binds::WHEEL_DOWN])
                        .map(|n| (n.to_string(), bound(n))),
                );
            }
            "bind" => {
                out.extend(ACTIONS.iter().map(|a| (format!("+{a}"), String::new())));
                out.extend(console.commands().map(|c| (shown(&c.name), c.help.clone())));
            }
            "map" | "maps" if arg == 0 => {
                let game = crate::game_registry::default_game(world);
                out.extend(map_names(world, game).iter().map(|m| (m.clone(), String::new())))
            }
            "jointeam" if arg == 0 => {
                out.push(("1".into(), "spectators".into()));
                out.push(("2".into(), "terrorists".into()));
                out.push(("3".into(), "counter-terrorists".into()));
            }
            "bot_add" if arg == 0 => {
                out.push(("1".into(), "terrorists".into()));
                out.push(("2".into(), "counter-terrorists".into()));
            }
            "hurtme" if arg == 0 => out.extend(plain(&[
                "head", "chest", "stomach", "leftarm", "rightarm", "leftleg", "rightleg", "generic",
            ])),
            "menu" if arg == 0 => out.extend(plain(&[
                "main",
                "newgame",
                "maps",
                "bots",
                "team",
                "options",
                "keyboard",
                "mouse",
                "audio",
                "video",
                "multiplayer",
                "extras",
                "players",
            ])),
            "debugui" if arg == 0 => {
                out.extend(super::debug_ui::Tab::ALL.iter().map(|t| (t.name().to_string(), t.title().to_string())));
                out.push(("close".into(), String::new()));
            }
            "bot_give" if arg == 0 => {
                if let Some(r) = world.get_resource::<crate::weapon::WeaponRegistry>() {
                    out.extend(r.0.iter().map(|d| (d.id.to_string(), String::new())));
                }
            }
            "watchbot" if arg == 0 => {
                out.push(("0".into(), "back to your own view".into()));
                out.extend(bot_names(world).into_iter().filter_map(|n| {
                    let number = n.strip_prefix("Bot ")?.to_string();
                    Some((number, n))
                }));
            }
            "scope" if arg == 0 => {
                out.push(("auto".into(), "follow the game played".into()));
                out.extend(
                    console
                        .namespace_names()
                        .into_iter()
                        .map(|(n, c)| if n == c { (n, String::new()) } else { (n, format!("alias of {c}")) }),
                );
            }
            "ent_fire" if arg == 0 => out.extend(logic_names(world)),
            "ent_fire" if arg == 1 => out.extend(plain(ENT_INPUTS)),
            _ => {}
        }
        let console = world.resource::<Console>();
        if let Some(c) = console.command(&typed).cloned()
            && let Some(complete) = c.complete
        {
            out.extend(complete(world, token).into_iter().map(|x| (x, String::new())));
        }
    }
    let mut ranked: Vec<(i32, String, String)> = out
        .into_iter()
        .filter_map(|(n, h)| score(&n, token).map(|s| (s, n, h)))
        .collect();
    ranked.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    ranked.dedup_by(|a, b| a.1 == b.1);
    (format!("{prefix}{head}"), ranked.into_iter().map(|(_, n, h)| (n, h)).collect())
}

/// Inputs most map entities take, offered for `ent_fire`.
const ENT_INPUTS: &[&str] = &[
    "Open",
    "Close",
    "Toggle",
    "Lock",
    "Unlock",
    "Press",
    "Use",
    "Enable",
    "Disable",
    "Trigger",
    "Kill",
    "Break",
    "SetHealth",
    "TurnOn",
    "TurnOff",
    "Skin",
    "FireUser1",
    "FireUser2",
    "PlaySound",
    "StopSound",
    "Start",
    "Stop",
    "Wake",
    "Sleep",
    "EnableMotion",
    "Ignite",
    "Extinguish",
    "AddOutput",
];

/// The bots' names ("Bot 3"), in order.
pub fn bot_names(world: &World) -> Vec<String> {
    let Some(mut q) = world.try_query_filtered::<&Name, With<crate::core::Bot>>() else {
        return Vec::new();
    };
    let mut names: Vec<String> = q.iter(world).map(|n| n.to_string()).collect();
    names.sort_by_key(|n| n.strip_prefix("Bot ").and_then(|b| b.parse::<u32>().ok()).unwrap_or(u32::MAX));
    names
}

/// The loaded map's named entities and their classes, then the classes
/// (`ent_fire` takes either).
pub fn logic_names(world: &World) -> Vec<(String, String)> {
    let Some(entities) = world
        .get_resource::<crate::map::logic_view::LogicInspect>()
        .and_then(|inspect| (inspect.names)(world))
    else {
        return Vec::new();
    };
    let mut named = Vec::new();
    let mut classes = std::collections::BTreeSet::new();
    for (name, class) in entities {
        if !name.is_empty() {
            named.push((name, class.clone()));
        }
        classes.insert(class);
    }
    named.sort();
    named.dedup();
    named.extend(classes.into_iter().map(|c| (c, "class".to_string())));
    named
}

/// The help line for what is being typed: the command (or cvar) and its
/// arguments with the one being typed marked, and a description (a
/// cvar's value, default, range and values).
#[derive(Clone, Debug, PartialEq)]
struct ArgHelp {
    name: String,
    args: Vec<String>,
    current: Option<usize>,
    detail: String,
}

fn arg_help(world: &mut World, input: &str) -> Option<ArgHelp> {
    let line = &input[crate::console::last_command_start(input)..];
    let arg = crate::console::arg_position(line)?;
    let name = line.split_whitespace().next()?.to_lowercase();
    let console = world.resource::<Console>();
    if let Some(c) = console.command(&name) {
        let (args, text) = crate::console::usage(&c.name, &c.help);
        // Past the last argument: still on it when it takes the rest of
        // the line (a bind's command, an echo's text).
        let current = match args.len() {
            0 => None,
            n if arg < n => Some(arg),
            n if args[n - 1].contains("command") || args[n - 1].contains("text") || args[n - 1].contains("...") => {
                Some(n - 1)
            }
            _ => None,
        };
        return Some(ArgHelp {
            name: console.display_name(&c.name),
            args,
            current,
            detail: text,
        });
    }
    if let Some(v) = console.cvar(&name).cloned() {
        let now = (v.get)(world).unwrap_or_default();
        let mut detail = format!("= \"{now}\"");
        if now != v.default {
            detail += &format!(" (default \"{}\")", v.default);
        }
        if let Some((a, b)) = v.range {
            detail += &format!("  range {a} to {b}");
        }
        if v.values.iter().any(|x| x != "0" && x != "1") {
            detail += &format!("  values: {}", v.values.join(" "));
        }
        if !v.help.is_empty() {
            detail += &format!("  - {}", v.help);
        }
        let kind = if v.integer { "<whole number>" } else { "<value>" };
        return Some(ArgHelp {
            name: world.resource::<Console>().display_name(&v.name),
            args: vec![kind.to_string()],
            current: (arg == 0).then_some(0),
            detail,
        });
    }
    if let Some(body) = console.aliases.get(&name) {
        return Some(ArgHelp {
            name: name.clone(),
            args: Vec::new(),
            current: None,
            detail: format!("alias: {body}"),
        });
    }
    None
}

/// The suggestions to show for the console's input, with the text they
/// complete (None: no list: nothing typed, browsing history, searching,
/// or hidden with Esc).
fn suggestions(world: &mut World) -> Option<(String, Vec<(String, String)>)> {
    let ui = world.resource::<ConsoleUi>();
    if !ui.open
        || ui.search.is_some()
        || ui.browsing.is_some()
        || ui.input.trim().is_empty()
        || ui.dismissed.as_ref() == Some(&ui.input)
    {
        return None;
    }
    let input = ui.input.clone();
    let (head, list) = candidates(world, &input);
    (!list.is_empty()).then_some((head, list))
}

/// Up/Down through `len` suggestions: Down from none starts at the top,
/// Up from none at the bottom; both wrap.
fn step_highlight(current: Option<usize>, len: usize, up: bool) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(match (current, up) {
        (None, false) => 0,
        (None, true) => len - 1,
        (Some(i), false) => (i + 1) % len,
        (Some(i), true) => (i + len - 1) % len,
    })
}

/// The first suggestion shown so the highlighted one is in view.
fn suggestion_window(highlight: Option<usize>, len: usize) -> usize {
    match highlight {
        Some(h) if h >= SUGGESTIONS => (h + 1 - SUGGESTIONS).min(len.saturating_sub(SUGGESTIONS)),
        _ => 0,
    }
}

/// Fuzzy rank (lower is better): prefix, then substring, then the letters
/// in order; None when it doesn't match.
fn score(name: &str, query: &str) -> Option<i32> {
    let (n, q) = (name.to_lowercase(), query.to_lowercase());
    if q.is_empty() {
        return Some(1000);
    }
    if n.starts_with(&q) {
        return Some(n.len() as i32 - q.len() as i32);
    }
    if let Some(at) = n.find(&q) {
        return Some(100 + at as i32);
    }
    let mut gaps = 0;
    let mut chars = n.chars();
    for qc in q.chars() {
        let mut skipped = 0;
        loop {
            match chars.next() {
                Some(c) if c == qc => break,
                Some(_) => skipped += 1,
                None => return None,
            }
        }
        gaps += skipped;
    }
    Some(300 + gaps)
}

/// Text editing, completion, history and search while open.
fn edit(world: &mut World) {
    let events: Vec<KeyboardInput> = world
        .resource_mut::<Messages<KeyboardInput>>()
        .drain()
        .filter(|e| e.state == ButtonState::Pressed)
        .collect();
    let wheel: Vec<MouseWheel> = world.resource_mut::<Messages<MouseWheel>>().drain().collect();
    let keys = world.resource::<ButtonInput<KeyCode>>().clone();
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    for w in wheel {
        let lines = match w.unit {
            MouseScrollUnit::Line => w.y * 3.0,
            MouseScrollUnit::Pixel => w.y / 16.0,
        };
        let mut ui = world.resource_mut::<ConsoleUi>();
        ui.scroll = (ui.scroll as f32 + lines).max(0.0) as usize;
    }
    for e in events {
        if e.key_code == KeyCode::Backquote {
            continue;
        }
        edit_key(world, &e, ctrl, shift);
    }
}

/// One key pressed in the open console.
fn edit_key(world: &mut World, e: &KeyboardInput, ctrl: bool, shift: bool) {
    let mut ui = world.resource_mut::<ConsoleUi>();
    // Reverse search mode.
    if let Some((query, back)) = ui.search.clone() {
        match (&e.logical_key, e.key_code) {
            (_, KeyCode::KeyR) if ctrl => ui.search = Some((query, back + 1)),
            (_, KeyCode::Escape) => ui.search = None,
            (_, KeyCode::Enter) | (_, KeyCode::ArrowLeft) | (_, KeyCode::ArrowRight) | (_, KeyCode::Tab) => {
                let found = search(&ui.history, &query, back);
                ui.search = None;
                if let Some(found) = found {
                    ui.cursor = found.chars().count();
                    ui.input = found;
                }
                if e.key_code == KeyCode::Enter {
                    submit(world);
                }
            }
            (_, KeyCode::Backspace) => {
                let mut q = query;
                q.pop();
                ui.search = Some((q, 0));
            }
            (Key::Character(s), _) if !ctrl => ui.search = Some((format!("{query}{s}"), 0)),
            (Key::Space, _) => ui.search = Some((format!("{query} "), 0)),
            _ => {}
        }
        return;
    }
    // The suggestion list, when one shows: Up/Down pick, Enter or Tab
    // take the pick, Esc hides it.
    let list_keys = matches!(
        e.key_code,
        KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Tab | KeyCode::Escape
    );
    if list_keys && let Some((head, list)) = suggestions(world) {
        let mut ui = world.resource_mut::<ConsoleUi>();
        match (e.key_code, ui.highlight) {
            (KeyCode::ArrowUp | KeyCode::ArrowDown, h) => {
                ui.highlight = step_highlight(h, list.len(), e.key_code == KeyCode::ArrowUp);
                return;
            }
            (KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Tab, Some(h)) if h < list.len() => {
                ui.input = accept(&head, &list[h].0);
                ui.cursor = ui.input.chars().count();
                ui.highlight = None;
                ui.cycle = None;
                return;
            }
            (KeyCode::Escape, _) => {
                ui.dismissed = Some(ui.input.clone());
                ui.highlight = None;
                return;
            }
            _ => {}
        }
    }
    let mut ui = world.resource_mut::<ConsoleUi>();
    let before = ui.input.clone();
    let chars: Vec<char> = ui.input.chars().collect();
    let cur = ui.cursor.min(chars.len());
    // Where the word before the cursor starts, and where the one after it
    // ends (Ctrl+Backspace, Ctrl+W, Ctrl+Delete, Ctrl+arrows).
    let word_start = {
        let mut c = cur;
        while c > 0 && chars[c - 1] == ' ' {
            c -= 1;
        }
        while c > 0 && chars[c - 1] != ' ' {
            c -= 1;
        }
        c
    };
    let word_end = {
        let mut c = cur;
        while c < chars.len() && chars[c] == ' ' {
            c += 1;
        }
        while c < chars.len() && chars[c] != ' ' {
            c += 1;
        }
        c
    };
    let mut reset_cycle = true;
    match (&e.logical_key, e.key_code) {
        (_, KeyCode::Enter) | (_, KeyCode::NumpadEnter) => {
            submit(world);
            return;
        }
        (_, KeyCode::KeyR) if ctrl => ui.search = Some((String::new(), 0)),
        (_, KeyCode::KeyL) if ctrl => {
            world.resource_mut::<Console>().output.clear();
            return;
        }
        (_, KeyCode::KeyU) if ctrl => {
            ui.input = chars[cur..].iter().collect();
            ui.cursor = 0;
        }
        (_, KeyCode::KeyK) if ctrl => ui.input = chars[..cur].iter().collect(),
        (_, KeyCode::KeyW) | (_, KeyCode::Backspace) if ctrl => {
            ui.input = chars[..word_start].iter().chain(&chars[cur..]).collect();
            ui.cursor = word_start;
        }
        (_, KeyCode::Delete) if ctrl => {
            ui.input = chars[..cur].iter().chain(&chars[word_end..]).collect();
        }
        (_, KeyCode::KeyV) if ctrl => {
            // Paste (one line: newlines become command separators).
            let text = arboard::Clipboard::new().and_then(|mut c| c.get_text()).unwrap_or_default();
            let text = text.trim_end().replace("\r\n", "; ").replace('\n', "; ");
            let ins: Vec<char> = text.chars().collect();
            let mut c = chars.clone();
            c.splice(cur..cur, ins.iter().copied());
            ui.input = c.into_iter().collect();
            ui.cursor = cur + ins.len();
        }
        (_, KeyCode::KeyC) if ctrl => {
            copy_selection(&ui);
        }
        (_, KeyCode::KeyA) if ctrl => ui.cursor = 0,
        (_, KeyCode::KeyE) if ctrl => ui.cursor = chars.len(),
        (_, KeyCode::Backspace) if cur > 0 => {
            ui.input = chars[..cur - 1].iter().chain(&chars[cur..]).collect();
            ui.cursor = cur - 1;
        }
        (_, KeyCode::Delete) if cur < chars.len() => {
            ui.input = chars[..cur].iter().chain(&chars[cur + 1..]).collect();
        }
        (_, KeyCode::ArrowLeft) if ctrl => ui.cursor = word_start,
        (_, KeyCode::ArrowRight) if ctrl => {
            let mut c = cur;
            while c < chars.len() && chars[c] != ' ' {
                c += 1;
            }
            while c < chars.len() && chars[c] == ' ' {
                c += 1;
            }
            ui.cursor = c;
        }
        (_, KeyCode::ArrowLeft) => ui.cursor = cur.saturating_sub(1),
        (_, KeyCode::ArrowRight) => ui.cursor = (cur + 1).min(chars.len()),
        // Ctrl+Home / Ctrl+End: the top and bottom of the output.
        (_, KeyCode::Home) if ctrl => ui.scroll = usize::MAX / 2,
        (_, KeyCode::End) if ctrl => ui.scroll = 0,
        (_, KeyCode::Home) => ui.cursor = 0,
        (_, KeyCode::End) => ui.cursor = chars.len(),
        (_, KeyCode::PageUp) => {
            let page = ui.page();
            ui.scroll += page;
        }
        (_, KeyCode::PageDown) => {
            let page = ui.page();
            ui.scroll = ui.scroll.saturating_sub(page);
        }
        (_, KeyCode::ArrowUp) | (_, KeyCode::ArrowDown) => {
            if ui.history.is_empty() {
                return;
            }
            let n = ui.history.len();
            let next = match (ui.browsing, e.key_code == KeyCode::ArrowUp) {
                (None, true) => Some(n - 1),
                (None, false) => None,
                (Some(i), true) => Some(i.saturating_sub(1)),
                (Some(i), false) if i + 1 < n => Some(i + 1),
                (Some(_), false) => None,
            };
            ui.browsing = next;
            ui.input = next.map(|i| ui.history[i].clone()).unwrap_or_default();
            ui.cursor = ui.input.chars().count();
            return;
        }
        (_, KeyCode::Tab) => {
            reset_cycle = false;
            let input = ui.input.clone();
            let step = if shift { -1 } else { 1 };
            if let Some((list, i, base)) = ui.cycle.clone()
                && !list.is_empty()
            {
                let next = (i as i64 + step).rem_euclid(list.len() as i64) as usize;
                let (head, _) = split_last(&base);
                ui.input = format!("{head}{}", list[next]);
                ui.cursor = ui.input.chars().count();
                ui.cycle = Some((list, next, base));
            } else {
                let (head, list) = candidates(world, &input);
                let mut ui = world.resource_mut::<ConsoleUi>();
                if let Some(first) = list.first() {
                    // One match completes and adds a space.
                    let single = list.len() == 1;
                    ui.input = format!("{head}{}{}", first.0, if single { " " } else { "" });
                    ui.cursor = ui.input.chars().count();
                    ui.cycle = (!single).then(|| (list.into_iter().map(|x| x.0).collect(), 0, input));
                }
            }
        }
        (Key::Character(s), _) if !ctrl => {
            let mut c = chars.clone();
            let ins: Vec<char> = s.chars().collect();
            c.splice(cur..cur, ins.iter().copied());
            ui.input = c.into_iter().collect();
            ui.cursor = cur + ins.len();
        }
        (Key::Space, _) => {
            let mut c = chars.clone();
            c.insert(cur, ' ');
            ui.input = c.into_iter().collect();
            ui.cursor = cur + 1;
        }
        _ => return,
    }
    let mut ui = world.resource_mut::<ConsoleUi>();
    if reset_cycle {
        ui.cycle = None;
    }
    // Typing starts a new line: out of the history, the pick gone.
    if ui.input != before {
        ui.browsing = None;
        ui.highlight = None;
    }
}

/// The input after taking a suggestion: the text it completes, the
/// suggestion and a space for what comes next.
fn accept(head: &str, suggestion: &str) -> String {
    format!("{head}{} ", crate::console::quote(suggestion))
}

/// Split off the token being completed.
fn split_last(input: &str) -> (String, String) {
    match input.rfind(' ') {
        Some(i) => (input[..=i].to_string(), input[i + 1..].to_string()),
        None => (String::new(), input.to_string()),
    }
}

/// The `back`-th most recent history entry containing `query`.
fn search(history: &[String], query: &str, back: usize) -> Option<String> {
    history.iter().rev().filter(|h| h.contains(query)).nth(back).cloned()
}

fn submit(world: &mut World) {
    let mut ui = world.resource_mut::<ConsoleUi>();
    let line = std::mem::take(&mut ui.input);
    ui.cursor = 0;
    ui.browsing = None;
    ui.cycle = None;
    ui.highlight = None;
    ui.scroll = 0;
    if !line.trim().is_empty() && ui.history.last() != Some(&line) {
        ui.history.push(line.clone());
        if ui.history.len() > HISTORY_MAX {
            ui.history.remove(0);
        }
        save_history(&ui.history);
    }
    let mut console = world.resource_mut::<Console>();
    console.print(Level::Input, format!("] {line}"));
    console.submit(line);
}

fn history_path() -> Option<std::path::PathBuf> {
    cfg_dir().map(|d| d.join("history.txt"))
}

fn save_history(history: &[String]) {
    // Unit tests type into the console: never into the player's history.
    if cfg!(test) {
        return;
    }
    if let Some(p) = history_path() {
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        let _ = std::fs::write(p, history.join("\n"));
    }
}

fn load_history(mut ui: ResMut<ConsoleUi>) {
    if let Some(text) = history_path().and_then(|p| std::fs::read_to_string(p).ok()) {
        ui.history = text.lines().filter(|l| !l.is_empty()).map(String::from).collect();
    }
}

/// config.cfg (binds, aliases, changed cvars) then autoexec.cfg, then the
/// command line's `+commands`.
fn autoexec(
    mut console: ResMut<Console>,
    mut ui: ResMut<ConsoleUi>,
    args: Res<super::ClientArgs>,
    games: Option<Res<Games>>,
    gameplay: Option<Res<crate::gameplay::Gameplay>>,
    mut defaults: ResMut<super::binds::BindDefaults>,
    mut root: Query<&mut Visibility, With<ConsoleRoot>>,
) {
    // Every key is a bind: the defaults first, then the player's config.
    // A config written before that has only the player's own binds after
    // its `unbindall`: the defaults go back on the keys it left free.
    // The defaults are the gameplay's binds game's (else the default
    // game's), from its install's file (`GameInfo::default_binds_cfg`,
    // CS:S's config_default.cfg).
    let binds = games.as_ref().and_then(|g| {
        gameplay.as_ref().and_then(|p| g.get(p.binds)).or_else(|| g.default_game())
    });
    defaults.read_install(binds);
    let config = crate::console::read_cfg("config.cfg");
    let marked = config.as_ref().is_none_or(|t| t.contains(super::binds::CONFIG_MARK));
    let current = config.as_ref().is_none_or(|t| t.contains(super::binds::DEFAULTS_MARK));
    console.submit("binddefaults; execifexists config.cfg");
    if !marked {
        console.submit("binddefaults missing");
    } else if !current {
        // Defaults added since the config was written (CS:S's F1, F2,
        // wheel, ...), on the keys it leaves free: once, as the config is
        // written again with the mark.
        console.submit("binddefaults new");
    }
    console.submit("execifexists autoexec.cfg");
    console.log_output = true;
    for line in &args.0.console {
        console.submit(line.clone());
    }
    // A map loaded before the console ran (`--map`): hosting with the
    // command line's `+maxplayers` starts here, as `map` does after its load.
    if args.0.map.is_some() {
        console.submit("chimera_host_if_asked");
    }
    if args.0.console_open {
        ui.open = true;
        for mut v in &mut root {
            *v = Visibility::Visible;
        }
    }
}

/// Save binds and changed cvars when the game closes.
fn save_on_exit(mut exits: MessageReader<AppExit>, args: Res<super::ClientArgs>, mut commands: Commands) {
    // Tool runs (`Args::tool_run`: --screenshot, --views, --frames, --bench)
    // leave the user's config alone.
    let automated = args.0.tool_run();
    if exits.read().next().is_some() && !automated {
        commands.queue(|w: &mut World| {
            if w.resource::<Console>().dirty {
                crate::console::execute(w, &["host_writeconfig".to_string()], 0);
            }
        });
    }
}

fn draw_console(
    console: Res<Console>,
    mut ui: ResMut<ConsoleUi>,
    output: Single<Entity, With<ConsoleOutput>>,
    mut input: Single<&mut Text, (With<ConsoleInput>, Without<ConsoleOutput>)>,
    window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut commands: Commands,
    mut last: Local<(usize, usize, String, bool, f64, usize)>,
    fonts: Res<UiFonts>,
) {
    // As many lines as fit at the window's current size.
    let fit = window.iter().next().map_or(DEFAULT_VISIBLE_LINES, |w| lines_for_height(w.height()));
    if ui.visible_lines != fit {
        ui.visible_lines = fit;
    }
    if !ui.open {
        return;
    }
    let visible = ui.visible_lines;
    // Output, filtered and scrolled, coloured by severity (rebuilt on change).
    let first_time = console.output.first().map_or(0.0, |l| l.time);
    let key = (
        console.output.len(),
        ui.scroll,
        ui.filter.clone(),
        ui.timestamps,
        first_time,
        visible,
    );
    if *last != key {
        *last = key;
        let lines: Vec<_> = console
            .output
            .iter()
            .filter(|l| ui.filter.is_empty() || l.text.to_lowercase().contains(&ui.filter.to_lowercase()))
            .collect();
        let max_scroll = lines.len().saturating_sub(visible);
        ui.scroll = ui.scroll.min(max_scroll);
        let end = lines.len() - ui.scroll;
        let start = end.saturating_sub(visible);
        let stamps = ui.timestamps;
        ui.shown = lines[start..end]
            .iter()
            .map(|l| if stamps { format!("[{:7.2}] {}", l.time, l.text) } else { l.text.clone() })
            .collect();
        ui.selection = None;
        ui.dragging = false;
        ui.fresh_rows = true;
        commands.entity(*output).despawn_related::<Children>();
        commands.entity(*output).with_children(|c| {
            for (row, l) in lines[start..end].iter().enumerate() {
                let color = match l.level {
                    Level::Input => Color::srgb(0.6, 0.8, 1.0),
                    Level::Info => Color::srgb(0.85, 0.85, 0.85),
                    Level::Warn => Color::srgb(1.0, 0.8, 0.3),
                    Level::Error => Color::srgb(1.0, 0.4, 0.35),
                };
                c.spawn((
                    ConsoleRow(row),
                    Text::new(ui.shown[row].clone()),
                    console_font(&fonts, OUTPUT_FONT),
                    TextColor(color),
                    Interaction::default(),
                    BackgroundColor(Color::NONE),
                ));
            }
            if ui.scroll > 0 {
                c.spawn((
                    Text::new(format!("-- {} more below (PageDown) --", ui.scroll)),
                    console_font(&fonts, OUTPUT_FONT),
                    TextColor(Color::srgb(0.5, 0.5, 0.5)),
                ));
            }
        });
    }
    // Input line with a cursor, or the reverse search.
    input.0 = match &ui.search {
        Some((q, back)) => {
            let found = search(&ui.history, q, *back).unwrap_or_default();
            format!("(reverse-i-search)`{q}': {found}")
        }
        None => {
            let chars: Vec<char> = ui.input.chars().collect();
            let cur = ui.cursor.min(chars.len());
            let before: String = chars[..cur].iter().collect();
            let after: String = chars[cur..].iter().collect();
            format!("] {before}|{after}")
        }
    };
}

/// A point in the output: a row on screen and a byte offset in its text.
type TextPos = (usize, usize);

/// The selection's ends in reading order.
fn ordered((a, b): (TextPos, TextPos)) -> (TextPos, TextPos) {
    if a <= b { (a, b) } else { (b, a) }
}

/// The byte range of row `row` the selection covers (`len`: its text's length).
fn row_part(sel: (TextPos, TextPos), row: usize, len: usize) -> Option<std::ops::Range<usize>> {
    let (s, e) = ordered(sel);
    if row < s.0 || row > e.0 {
        return None;
    }
    let from = if row == s.0 { s.1.min(len) } else { 0 };
    let to = if row == e.0 { e.1.min(len) } else { len };
    (from < to).then_some(from..to)
}

/// The selected text (rows joined by newlines).
fn selected_text(rows: &[String], sel: (TextPos, TextPos)) -> String {
    let (s, e) = ordered(sel);
    (s.0..=e.0)
        .filter_map(|r| {
            let t = rows.get(r)?;
            Some(row_part(sel, r, t.len()).map_or("", |p| &t[p]).to_string())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Select text in the output with the mouse: press and drag from one
/// character to another (rows between are covered whole), the selected
/// span drawn highlighted; Ctrl+C copies it (`copy_selection`), as CS:S's
/// console does.
fn select_text(
    mut ui: ResMut<ConsoleUi>,
    mouse: Res<ButtonInput<MouseButton>>,
    window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    rows: Query<(Entity, &ConsoleRow, &bevy::text::ComputedTextBlock, &ComputedNode, &UiGlobalTransform, &TextColor)>,
    mut commands: Commands,
    mut drawn: Local<Option<(TextPos, TextPos)>>,
    mut painted: Local<usize>,
    fonts: Res<UiFonts>,
) {
    if !ui.open {
        return;
    }
    if std::mem::take(&mut ui.fresh_rows) {
        // The console just rebuilt its rows (plain, selection dropped).
        *drawn = None;
        *painted = 0;
        return;
    }
    let cursor = window.iter().next().and_then(Window::cursor_position);
    // The character under the pointer, and whether it is inside a row
    // (else the nearest row's).
    let under = |at: Vec2| -> Option<(TextPos, bool)> {
        let mut nearest: Option<(f32, TextPos, bool)> = None;
        for (_, row, block, node, t, _) in &rows {
            let size = node.size();
            let top_left = t.translation - size / 2.0;
            let local = at / node.inverse_scale_factor() - top_left;
            let inside = local.y >= 0.0 && local.y < size.y && local.x >= 0.0 && local.x < size.x;
            let dist = if local.y >= 0.0 && local.y < size.y { 0.0 } else { (local.y - size.y / 2.0).abs() };
            if nearest.as_ref().is_none_or(|n| dist < n.0) {
                nearest = Some((dist, (row.0, {
                    let (x, y) = (local.x, local.y);
                    let mut best = None;
                    for line in block.buffer().lines() {
                        let hit = y < line.metrics().block_max_coord;
                        best = Some(line);
                        if hit {
                            break;
                        }
                    }
                    match best {
                        None => 0,
                        Some(line) => {
                            let mut pos = line.metrics().offset;
                            let mut at = line.text_range().end;
                            'scan: for run in line.runs() {
                                for cluster in run.visual_clusters() {
                                    let adv = cluster.advance();
                                    if x < pos + adv / 2.0 {
                                        at = cluster.text_range().start;
                                        break 'scan;
                                    }
                                    pos += adv;
                                }
                            }
                            at
                        }
                    }
                }), inside));
            }
        }
        nearest.map(|(_, p, inside)| (p, inside))
    };
    if mouse.just_pressed(MouseButton::Left) {
        ui.selection = cursor.and_then(under).filter(|(_, inside)| *inside).map(|(p, _)| (p, p));
        ui.dragging = ui.selection.is_some();
    } else if ui.dragging && mouse.pressed(MouseButton::Left) {
        if let (Some((p, _)), Some(sel)) = (cursor.and_then(under), ui.selection.as_mut()) {
            sel.1 = p;
        }
    } else if ui.dragging {
        ui.dragging = false;
    }
    let shown_now = ui.selection.filter(|(a, b)| a != b);
    let count = rows.iter().count();
    if *drawn == shown_now && *painted == count {
        return;
    }
    *drawn = shown_now;
    *painted = count;
    for (entity, row, .., color) in &rows {
        let text = ui.shown.get(row.0).cloned().unwrap_or_default();
        let part = shown_now.and_then(|s| row_part(s, row.0, text.len()));
        let font = console_font(&fonts, OUTPUT_FONT);
        let mut e = commands.entity(entity);
        e.despawn_related::<Children>();
        match part {
            None => {
                e.insert(Text::new(text));
            }
            Some(p) => {
                e.insert(Text::new(text[..p.start].to_string()));
                e.with_children(|c| {
                    c.spawn((
                        TextSpan::new(text[p.clone()].to_string()),
                        font.clone(),
                        *color,
                        bevy::text::TextBackgroundColor(Color::srgba(0.3, 0.45, 0.8, 0.7)),
                    ));
                    c.spawn((TextSpan::new(text[p.end..].to_string()), font, *color));
                });
            }
        }
    }
}

/// Ctrl+C with text selected in the output puts it on the clipboard;
/// whether there was a selection to copy.
fn copy_selection(ui: &ConsoleUi) -> bool {
    let Some(sel) = ui.selection.filter(|(a, b)| a != b) else { return false };
    let text = selected_text(&ui.shown, sel);
    if let Err(e) = arboard::Clipboard::new().and_then(|mut c| c.set_text(text)) {
        warn!("clipboard: {e}");
    }
    true
}

/// The suggestion box under the console as you type: the help line for
/// the command being typed (its arguments, the one being typed marked),
/// then the suggestions with the picked one highlighted (needs the world
/// to read cvar values).
fn overlay_text(world: &mut World) {
    let rows = suggest_rows(world);
    let shown = !rows.is_empty();
    world.resource_mut::<ConsoleUi>().list_shown = shown && suggestions(world).is_some();
    let want = if shown { Visibility::Visible } else { Visibility::Hidden };
    let suggest = chimera_core::query_cache::cached_query::<(Entity, &mut Visibility), With<ConsoleSuggest>, _>(world, |q, w| {
        q.single_mut(w).ok().map(|(e, mut v)| {
            v.set_if_neq(want);
            e
        })
    });
    if let Some(e) = suggest {
        if world.resource::<ConsoleUi>().suggest_rows != rows {
            let font = console_font(world.resource::<UiFonts>(), OUTPUT_FONT);
            world.entity_mut(e).despawn_related::<Children>();
            world.entity_mut(e).with_children(|c| {
                for row in &rows {
                    c.spawn((
                        Text::default(),
                        font.clone(),
                        TextColor(Color::NONE),
                        Node {
                            padding: UiRect::horizontal(px(3.0)),
                            ..default()
                        },
                        BackgroundColor(if row.highlight {
                            Color::srgba(0.3, 0.45, 0.8, 0.6)
                        } else {
                            Color::NONE
                        }),
                    ))
                    .with_children(|t| {
                        for (text, color) in &row.runs {
                            t.spawn((
                                TextSpan::new(text.clone()),
                                font.clone(),
                                TextColor(*color),
                            ));
                        }
                    });
                }
            });
            world.resource_mut::<ConsoleUi>().suggest_rows = rows;
        }
    }
}

/// The suggestion box's rows (none: hidden).
fn suggest_rows(world: &mut World) -> Vec<SuggestRow> {
    let (open, input, searching, highlight) = {
        let ui = world.resource::<ConsoleUi>();
        (ui.open, ui.input.clone(), ui.search.is_some(), ui.highlight)
    };
    let mut rows = Vec::new();
    if !open || searching || input.trim().is_empty() {
        return rows;
    }
    let (name_c, arg_c, current_c, text_c, value_c, more_c) = (
        Color::srgb(1.0, 0.95, 0.6),
        Color::srgb(0.6, 0.6, 0.65),
        Color::srgb(0.5, 1.0, 0.6),
        Color::srgb(0.75, 0.75, 0.8),
        Color::srgb(0.6, 0.8, 1.0),
        Color::srgb(0.5, 0.5, 0.5),
    );
    if let Some(help) = arg_help(world, &input) {
        let mut runs = vec![(help.name.clone(), name_c)];
        for (i, a) in help.args.iter().enumerate() {
            runs.push((format!(" {a}"), if help.current == Some(i) { current_c } else { arg_c }));
        }
        if !help.detail.is_empty() {
            let detail: String = help.detail.chars().take(160).collect();
            runs.push((format!("   {detail}"), text_c));
        }
        rows.push(SuggestRow { runs, highlight: false });
    }
    let Some((_, list)) = suggestions(world) else {
        return rows;
    };
    let first = suggestion_window(highlight, list.len());
    if first > 0 {
        rows.push(SuggestRow {
            runs: vec![(format!("... {first} more above"), more_c)],
            highlight: false,
        });
    }
    for (i, (n, h)) in list.iter().enumerate().skip(first).take(SUGGESTIONS) {
        let cvar = world.resource::<Console>().cvar(n).cloned();
        let value = cvar.and_then(|v| {
            let now = (v.get)(world)?;
            Some(if now != v.default { format!(" = {now} *") } else { format!(" = {now}") })
        });
        let help: String = h.chars().take(90).collect();
        let mut runs = vec![(n.clone(), name_c)];
        if let Some(v) = value {
            runs.push((v, value_c));
        }
        if !help.is_empty() {
            runs.push((format!("   {help}"), text_c));
        }
        rows.push(SuggestRow {
            runs,
            highlight: highlight == Some(i),
        });
    }
    let below = list.len().saturating_sub(first + SUGGESTIONS);
    if below > 0 {
        rows.push(SuggestRow {
            runs: vec![(format!("... {below} more (Up/Down, Tab)"), more_c)],
            highlight: false,
        });
    }
    rows
}

/// Watched cvars' values for the overlay (`watch`).
fn watch_text(world: &mut World) {
    // Watches: each watched cvar's value.
    let watches = world.resource::<Console>().watches.clone();
    let mut lines = Vec::new();
    for name in watches {
        let cvar = world.resource::<Console>().cvar(&name).cloned();
        let value = cvar.and_then(|v| (v.get)(world)).unwrap_or_else(|| "?".into());
        lines.push(format!("{name}: {value}"));
    }
    world.resource_mut::<Overlays>().watch_text = lines.join("\n");
}

// ------------------------------------------------------------- overlays

#[derive(Resource)]
pub struct Overlays {
    pub showpos: u8,
    pub showfps: u8,
    pub snd_show: u8,
    /// `developer`: 1 shows console output top left for a few seconds.
    pub developer: u8,
    /// Notify lines and seconds left, and console lines already taken.
    notify: VecDeque<(String, f32)>,
    notify_seen: u64,
    watch_text: String,
    frames: VecDeque<f32>,
}

impl Default for Overlays {
    fn default() -> Self {
        Self {
            showpos: 0,
            showfps: 0,
            snd_show: 0,
            developer: 0,
            notify: VecDeque::new(),
            notify_seen: 0,
            watch_text: String::new(),
            frames: VecDeque::new(),
        }
    }
}

/// Source's con_notifytime and notify line count.
const NOTIFY_SECONDS: f32 = 8.0;
const NOTIFY_LINES: usize = 8;

#[derive(Component)]
struct NotifyText;

/// `developer 1`: new console lines show top left for a few seconds.
fn draw_notify(
    time: Res<Time>,
    console: Res<Console>,
    ui: Res<ConsoleUi>,
    mut o: ResMut<Overlays>,
    mut text: Single<(&mut Text, &mut Visibility), With<NotifyText>>,
) {
    let new = console.printed.saturating_sub(o.notify_seen) as usize;
    o.notify_seen = console.printed;
    if o.developer > 0 {
        let start = console.output.len().saturating_sub(new);
        for line in &console.output[start..] {
            if line.level != crate::console::Level::Input {
                o.notify.push_back((line.text.clone(), NOTIFY_SECONDS));
            }
        }
    }
    let dt = time.delta_secs();
    o.notify.retain_mut(|(_, left)| {
        *left -= dt;
        *left > 0.0
    });
    while o.notify.len() > NOTIFY_LINES {
        o.notify.pop_front();
    }
    let (t, vis) = &mut *text;
    // Hidden while the console itself is open, or when off.
    let show = o.developer > 0 && !ui.open && !o.notify.is_empty();
    **vis = if show {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if show {
        t.0 = o.notify.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>().join("\n");
    }
}

fn overlay_cvars(app: &mut App) {
    resource_cvar::<Overlays, u8>(
        app,
        "developer",
        "1: show console output at the top left for a few seconds.",
        |o| &mut o.developer,
    );
    resource_cvar::<Overlays, u8>(
        app,
        "cl_showpos",
        "1: show map, position, angles and velocity (CS:S units on CS:S maps).",
        |o| &mut o.showpos,
    );
    resource_cvar::<Overlays, u8>(
        app,
        "cl_showfps",
        "1: frame rate; 2: also frame time min/avg/max over the last second.",
        |o| &mut o.showfps,
    );
    resource_cvar::<Overlays, u8>(
        app,
        "snd_show",
        "1: mark where sounds play, with their names, for a few seconds; show the soundscape and room DSP preset.",
        |o| &mut o.snd_show,
    );
    // The room (audio::room) may be added after the console.
    app.init_resource::<crate::audio::room::RoomDsp>();
    resource_cvar::<crate::audio::room::RoomDsp, u8>(app, "dsp_off", "1: no room reverb or echo.", |r| &mut r.off);
    resource_cvar::<crate::audio::room::RoomDsp, f32>(
        app,
        "dsp_volume",
        "Level of the room reverb and echo (1 normal).",
        |r| &mut r.volume,
    );
    app.console_cvar(
        "con_timestamps",
        "1: time stamps on console lines.",
        "0",
        |w| {
            w.get_resource::<ConsoleUi>()
                .map(|u| if u.timestamps { "1" } else { "0" }.to_string())
        },
        |w, v| {
            w.resource_mut::<ConsoleUi>().timestamps = v.trim() != "0";
            Ok(())
        },
    );
    // CS:S's default is 0 (the console key does nothing until the
    // options' Keyboard > Advanced enables it); ours is 1, so the console
    // stays at hand for playtests.
    app.console_cvar(
        "con_enable",
        "1: the console key opens the console (CS:S's default is 0; ours 1).",
        "1",
        |w| {
            w.get_resource::<ConsoleUi>()
                .map(|u| if u.disabled { "0" } else { "1" }.to_string())
        },
        |w, v| {
            w.resource_mut::<ConsoleUi>().disabled = v.trim() == "0";
            Ok(())
        },
    );
    let mut console = app.world_mut().resource_mut::<Console>();
    console.archive("con_timestamps");
    console.archive("con_enable");
    // Set from the game menu's options too.
    console.archive("cl_showfps");
}

fn draw_overlays(
    time: Res<Time>,
    mut o: ResMut<Overlays>,
    mut text: Single<&mut Text, With<OverlayText>>,
    player: Option<Single<(&Transform, &Velocity, &crate::core::Intent), With<LocalPlayer>>>,
    map: Option<Res<crate::map::ActiveMapLook>>,
    console: Res<Console>,
    sound: (
        Option<Res<crate::audio::soundscape::ScapeState>>,
        Option<Res<crate::audio::room::RoomDsp>>,
    ),
) {
    let _ = map;
    let dt = time.delta_secs();
    o.frames.push_back(dt);
    while o.frames.iter().sum::<f32>() > 1.0 && o.frames.len() > 1 {
        o.frames.pop_front();
    }
    let mut lines = Vec::new();
    if o.showfps > 0 {
        let n = o.frames.len().max(1) as f32;
        let avg = o.frames.iter().sum::<f32>() / n;
        lines.push(format!("{:.0} fps", 1.0 / avg.max(1e-6)));
        if o.showfps > 1 {
            let min = o.frames.iter().cloned().fold(f32::MAX, f32::min);
            let max = o.frames.iter().cloned().fold(0.0f32, f32::max);
            lines.push(format!(
                "frame {:.1} / {:.1} / {:.1} ms",
                min * 1e3,
                avg * 1e3,
                max * 1e3
            ));
        }
    }
    if o.showpos > 0
        && let Some(p) = &player
    {
        let (t, v, i) = **p;
        let units = |x: Vec3| Vec3::new(x.x, -x.z, x.y) / 0.0254;
        let pos = units(t.translation);
        let vel = units(v.0);
        lines.push(format!("pos: {:.2} {:.2} {:.2}", pos.x, pos.y, pos.z));
        lines.push(format!(
            "ang: {:.2} {:.2} 0.00",
            -i.pitch.to_degrees(),
            (i.yaw.to_degrees() + 90.0).rem_euclid(360.0)
        ));
        lines.push(format!("vel: {:.2} (xy {:.2})", vel.length(), vel.truncate().length()));
    }
    if o.snd_show > 0
        && let (Some(scape), Some(room)) = &sound
    {
        lines.push(crate::audio::soundscape::readout(scape, room));
    }
    if !o.watch_text.is_empty() {
        lines.push(o.watch_text.clone());
    }
    let _ = console;
    // Only a change marks the text for layout again (`ui_write`).
    crate::client::ui_write::set_text(&mut text, &lines.join("\n"));
}

/// Sounds marked on screen for `snd_show`.
#[derive(Resource, Default)]
struct SoundMarks(Vec<(String, Vec3, f32, Entity)>);

#[derive(Component)]
struct SoundMark;

const MARK_SECONDS: f32 = 3.0;

fn record_sounds(
    mut sounds: MessageReader<PlaySound>,
    o: Res<Overlays>,
    mut marks: ResMut<SoundMarks>,
    fonts: Res<UiFonts>,
    mut commands: Commands,
) {
    for s in sounds.read() {
        if o.snd_show == 0 {
            continue;
        }
        let Some(at) = s.at else { continue };
        let e = commands
            .spawn((
                SoundMark,
                Text::new(s.entry.clone()),
                overlay_font(&fonts, 13.0),
                TextColor(Color::srgb(0.5, 1.0, 0.6)),
                Node {
                    position_type: PositionType::Absolute,
                    ..default()
                },
            ))
            .id();
        marks.0.push((s.entry.clone(), at, MARK_SECONDS, e));
    }
}

fn draw_sound_marks(
    time: Res<Time>,
    mut marks: ResMut<SoundMarks>,
    camera: Query<(&Camera, &GlobalTransform), With<FirstPersonCamera>>,
    mut nodes: Query<(&mut Node, &mut TextColor, &mut Visibility), With<SoundMark>>,
    mut commands: Commands,
) {
    let cam = camera.iter().next();
    marks.0.retain_mut(|(_, at, left, e)| {
        *left -= time.delta_secs();
        if *left <= 0.0 {
            commands.entity(*e).despawn();
            return false;
        }
        if let (Some((c, t)), Ok((mut node, mut color, mut vis))) = (cam, nodes.get_mut(*e)) {
            match c.world_to_viewport(t, *at) {
                Ok(p) => {
                    node.left = px(p.x);
                    node.top = px(p.y);
                    *vis = Visibility::Visible;
                    color.0 = color.0.with_alpha(*left / MARK_SECONDS);
                }
                Err(_) => *vis = Visibility::Hidden,
            }
        }
        true
    });
}

// ------------------------------------------------------------- log

static LOG: OnceLock<Arc<Mutex<Vec<(Level, String)>>>> = OnceLock::new();

/// A tracing layer that copies warnings and errors into the console.
pub fn log_layer(_: &mut App) -> Option<BoxedLayer> {
    struct ToConsole(Arc<Mutex<Vec<(Level, String)>>>);
    impl<S: bevy::log::tracing::Subscriber> Layer<S> for ToConsole {
        fn on_event(
            &self,
            event: &bevy::log::tracing::Event<'_>,
            _: bevy::log::tracing_subscriber::layer::Context<'_, S>,
        ) {
            let level = match *event.metadata().level() {
                bevy::log::tracing::Level::ERROR => Level::Error,
                bevy::log::tracing::Level::WARN => Level::Warn,
                bevy::log::tracing::Level::INFO if event.metadata().target().starts_with("mashup") => Level::Info,
                _ => return,
            };
            struct Msg(String);
            impl bevy::log::tracing::field::Visit for Msg {
                fn record_debug(&mut self, f: &bevy::log::tracing::field::Field, v: &dyn std::fmt::Debug) {
                    if f.name() == "message" {
                        self.0 = format!("{v:?}");
                    }
                }
            }
            let mut m = Msg(String::new());
            event.record(&mut m);
            if let Ok(mut q) = self.0.lock() {
                q.push((level, m.0));
            }
        }
    }
    let queue = LOG.get_or_init(Default::default).clone();
    // A profile build also times every span for the hitch log
    // (`frame_metrics::span_times`).
    #[cfg(feature = "profile")]
    let layer = ToConsole(queue).and_then(super::frame_metrics::span_times::SpanTimes);
    #[cfg(not(feature = "profile"))]
    let layer = ToConsole(queue);
    // A `trace` build records short windows of spans (`trace_capture`).
    #[cfg(feature = "trace")]
    let layer = layer.and_then(super::trace_capture::CaptureLayer);
    Some(Box::new(layer))
}

fn drain_log(mut console: ResMut<Console>) {
    let Some(q) = LOG.get() else { return };
    let lines: Vec<_> = q.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default();
    for (level, text) in lines {
        console.print(level, text);
    }
}

// ------------------------------------------------------------- binds

/// Actions held through binds (`+jump`, ...), read with the keys.
#[derive(Resource, Default, Debug)]
pub struct HeldActions {
    pub jump: bool,
    pub duck: bool,
    pub speed: bool,
    /// `+walk` and `+sprint`: one of `+speed`'s two halves each.
    pub walk: bool,
    pub sprint: bool,
    pub forward: bool,
    pub back: bool,
    pub moveleft: bool,
    pub moveright: bool,
    pub attack: bool,
    pub attack2: bool,
    pub reload: bool,
    /// The use key (`+use`): doors, buttons.
    pub use_key: bool,
    /// The scoreboard (`+showscores`, Tab in CS:S).
    pub showscores: bool,
    /// Free look (`+freelook`, Left Alt): the mouse turns the camera only.
    pub freelook: bool,
    /// The game's named actions held from the console (`+firemode`,
    /// `BindsAppExt::bound_action`).
    pub named: std::collections::BTreeSet<String>,
}

const ACTIONS: &[&str] = &[
    "jump",
    "duck",
    "speed",
    "walk",
    "sprint",
    "forward",
    "back",
    "moveleft",
    "moveright",
    "attack",
    "attack2",
    "reload",
    "use",
    "showscores",
    "freelook",
];

/// Keys that vote while a vote waits for ours (`net::vote::VoteView::
/// awaiting`), in place of their binds: F1 yes, F2 no, as the vote
/// panel's "Press F1 to vote YES" says (CS:S binds F1 and F2 to autobuy
/// and rebuy; whether they still buy during a vote is unmeasured).
pub const VOTE_KEYS: [(&str, &str); 2] = [("f1", "vote option1"), ("f2", "vote option2")];

/// Run bound commands: on press, and the `-` half of `+` actions on
/// release; wheel notches run once. F1 and F2 vote while a vote waits for
/// ours (`VOTE_KEYS`); F3 is the pre-match lobby's while it shows
/// (`prematch_lobby::LOBBY_KEY`: the host starts the match, a player
/// readies up).
fn run_binds(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut wheel: MessageReader<MouseWheel>,
    mut console: ResMut<Console>,
    vote: Option<Res<crate::net::vote::VoteView>>,
    lobby: Option<Res<super::prematch_lobby::LobbyView>>,
    polled: Option<Res<super::binds::PolledBinds>>,
) {
    let voting = vote.is_some_and(|v| v.awaiting());
    let lobby = lobby.filter(|l| l.shown);
    let polled = polled.as_deref().cloned().unwrap_or_default();
    if console.binds.is_empty() {
        wheel.clear();
        return;
    }
    let mut events: Vec<(String, bool)> = Vec::new();
    for (code, name) in super::binds::KEY_NAMES {
        if keys.just_pressed(*code) {
            events.push((name.to_string(), true));
        }
        if keys.just_released(*code) {
            events.push((name.to_string(), false));
        }
    }
    for (b, name) in super::binds::MOUSE_NAMES {
        if mouse.just_pressed(*b) {
            events.push((name.to_string(), true));
        }
        if mouse.just_released(*b) {
            events.push((name.to_string(), false));
        }
    }
    for w in wheel.read() {
        let name = if w.y > 0.0 {
            super::binds::WHEEL_UP
        } else {
            super::binds::WHEEL_DOWN
        };
        events.push((name.into(), true));
        events.push((name.into(), false));
    }
    for (key, pressed) in events {
        if voting && let Some((_, line)) = VOTE_KEYS.iter().find(|(k, _)| *k == key) {
            if pressed {
                console.submit(*line);
            }
            continue;
        }
        if let Some(l) = &lobby
            && key == super::prematch_lobby::LOBBY_KEY
        {
            if pressed && let Some(line) = l.key_line() {
                console.submit(line);
            }
            continue;
        }
        let Some(cmd) = console.binds.get(&key).cloned() else {
            continue;
        };
        // Read from the held keys by the systems that own them; a line
        // of several commands runs the rest.
        let Some(cmd) = polled.runnable(&cmd) else {
            continue;
        };
        // config.cfg is shared: another app's commands aren't this one's.
        let Some(cmd) = super::binds::known_here(&console, &cmd) else {
            continue;
        };
        if pressed {
            console.submit(cmd);
        } else {
            // Releasing runs the "-" half of each "+" command.
            let releases: Vec<String> = parse(&cmd)
                .into_iter()
                .filter(|w| w[0].starts_with('+'))
                .map(|w| format!("-{}", &w[0][1..]))
                .collect();
            if !releases.is_empty() {
                console.submit(releases.join("; "));
            }
        }
    }
}

// ------------------------------------------------------------- commands

/// What each game's install offers for maps, read once (the install is
/// read from disk): the names for `map` completion and the map cycle,
/// keyed by the game's id, until an import changes them. A resource of
/// the console's plugin, so apps in one process keep their own. Behind a
/// lock so the callers that only have a `&World` (completion, the
/// debug UI) can fill it.
#[derive(Resource, Debug, Default)]
pub struct MapLists {
    names: Mutex<BTreeMap<&'static str, Vec<String>>>,
    cycles: Mutex<BTreeMap<&'static str, Vec<String>>>,
}

impl MapLists {
    /// The install changed: find the maps again.
    pub fn forget(&self) {
        self.names.lock().unwrap_or_else(|e| e.into_inner()).clear();
        self.cycles.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    fn names(&self, game: &GameInfo) -> Vec<String> {
        self.names_by(game.id, || read_map_names(game))
    }

    /// Game `id`'s names: `read` them on the first ask only.
    fn names_by(&self, id: &'static str, read: impl FnOnce() -> Vec<String>) -> Vec<String> {
        let mut cached = self.names.lock().unwrap_or_else(|e| e.into_inner());
        cached.entry(id).or_insert_with(read).clone()
    }

    fn cycle(&self, game: &GameInfo) -> Vec<String> {
        let mut cached = self.cycles.lock().unwrap_or_else(|e| e.into_inner());
        cached
            .entry(game.id)
            .or_insert_with(|| game.mount().map(|m| (game.map_cycle)(&m)).unwrap_or_default())
            .clone()
    }
}

/// Every game's map list read on a task when the app starts
/// (`read_map_lists_ahead`): the first `map` command (or completion) read
/// it on its frame (and the map cycle when the map changed), opening the install's archives (Combat Arms': 300 M
/// instructions, a 60 ms frame at the start of the swap; docs/performance.md,
/// "A map's swap in the real app"). An ask before it is in reads as before.
#[derive(Resource)]
struct MapListsAhead(bevy::tasks::Task<Vec<(&'static str, Vec<String>, Vec<String>)>>);

fn read_map_lists_ahead(mut commands: Commands, games: Option<Res<Games>>) {
    let games: Vec<&'static GameInfo> = games.iter().flat_map(|g| g.all()).collect();
    let Some(pool) = bevy::tasks::AsyncComputeTaskPool::try_get().filter(|_| !games.is_empty()) else { return };
    let task = pool.spawn(async move {
        games
            .into_iter()
            .map(|g| {
                let Some(mount) = g.mount() else { return (g.id, Vec::new(), Vec::new()) };
                let mut names = (g.map_names)(&mount);
                names.sort();
                names.dedup();
                (g.id, names, (g.map_cycle)(&mount))
            })
            .collect()
    });
    commands.insert_resource(MapListsAhead(task));
}

fn take_map_lists_ahead(mut commands: Commands, ahead: Option<ResMut<MapListsAhead>>, lists: Res<MapLists>) {
    let Some(mut ahead) = ahead else { return };
    let Some(read) = bevy::tasks::block_on(bevy::tasks::poll_once(&mut ahead.0)) else { return };
    commands.remove_resource::<MapListsAhead>();
    let mut names = lists.names.lock().unwrap_or_else(|e| e.into_inner());
    let mut cycles = lists.cycles.lock().unwrap_or_else(|e| e.into_inner());
    for (id, list, cycle) in read {
        names.entry(id).or_insert(list);
        cycles.entry(id).or_insert(cycle);
    }
}

fn read_map_names(game: &GameInfo) -> Vec<String> {
    let Some(mount) = game.mount() else { return Vec::new() };
    let mut names = (game.map_names)(&mount);
    names.sort();
    names.dedup();
    names
}

/// The install changed: find its maps again.
pub fn forget_map_names(w: &World) {
    if let Some(lists) = w.get_resource::<MapLists>() {
        lists.forget();
    }
}

/// Every map `game` (the default game: `game_registry`) can load: the
/// install's, its downloads and Chimera's cache (cached in `MapLists`;
/// `import` refreshes it). None without a game.
pub fn map_names(w: &World, game: Option<&'static GameInfo>) -> Vec<String> {
    let Some(game) = game else { return Vec::new() };
    match w.get_resource::<MapLists>() {
        Some(lists) => lists.names(game),
        None => read_map_names(game),
    }
}

/// The rules' map cycle follows the map loaded (the end of a game moves
/// on to the next one: `rules::game_end`).
fn follow_map_cycle(
    name: Option<Res<crate::map::LoadedMapName>>,
    games: Option<Res<Games>>,
    lists: Option<Res<MapLists>>,
    mut cycle: ResMut<crate::core::MapCycle>,
) {
    let Some(name) = name.filter(|n| n.is_changed()) else { return };
    let short = crate::net::maps::map_name(&name.0).to_string();
    if cycle.current != short {
        cycle.current = short;
    }
    if cycle.maps.is_empty()
        && let Some(game) = games.and_then(|g| g.default_game())
    {
        cycle.maps = match lists {
            Some(l) => l.cycle(game),
            None => game.mount().map(|m| (game.map_cycle)(&m)).unwrap_or_default(),
        };
    }
}

/// Where a map of `game`'s comes from: the game's own content, its
/// download folder, or Chimera's cache.
fn map_source(game: &GameInfo, name: &str) -> &'static str {
    let Some(file) = (game.map_file)(name) else { return "game" };
    let in_dir = |dir: Option<std::path::PathBuf>| dir.is_some_and(|d| d.join(&file).is_file());
    if in_dir(game.content_cache()) {
        "imported"
    } else if in_dir(game.download_dir()) {
        "downloaded"
    } else {
        "game"
    }
}

/// Whether a `map` command's load is under way.
pub fn map_loading(w: &World) -> bool {
    w.contains_resource::<MapLoad>() || w.contains_resource::<Spawning>() || w.contains_resource::<Settling>()
}

/// Where a `map` command's load is (`map_load_stage`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapLoadStage {
    /// No load under way.
    None,
    /// Loading on a task.
    Loading,
    /// Spawning a step a frame.
    Spawning,
    /// Spawned, settling behind the loading screen.
    Settling,
}

/// Where a `map` command's load is.
pub fn map_load_stage(w: &World) -> MapLoadStage {
    if w.contains_resource::<Spawning>() {
        MapLoadStage::Spawning
    } else if w.contains_resource::<Settling>() {
        MapLoadStage::Settling
    } else if w.contains_resource::<MapLoad>() {
        MapLoadStage::Loading
    } else {
        MapLoadStage::None
    }
}

/// The map a `map` command is loading (its name, without the game).
pub fn loading_map(w: &World) -> Option<String> {
    let id = match (w.get_resource::<MapLoad>(), w.get_resource::<Spawning>()) {
        (Some(load), _) => &load.id,
        (None, Some(spawning)) => &spawning.id,
        (None, None) => &w.get_resource::<Settling>()?.id,
    };
    Some(id.rsplit(':').next().unwrap_or(id).to_string())
}

/// The longest a spawned map stays behind the loading screen (`Settling`); a
/// flood not done by then goes on a slice a tick in play.
const SETTLE_MAX_SECS: f32 = 10.0;

/// Ticks a spawned map runs before its settling starts: the collision is in
/// the spatial queries' trees after the first physics steps.
const SETTLE_AFTER_TICKS: u64 = 3;

/// A spawned map still behind the loading screen, doing what needs its physics
/// a slice a frame (`map::MapHooks::settle`, docs/performance.md, "Nothing
/// blocks the main thread"): the game is entered when that's done.
#[derive(Resource)]
struct Settling {
    id: String,
    tick: u64,
    started: std::time::Instant,
}

/// The most of a frame a loaded map's spawn takes behind the loading screen
/// (`Spawning`): its steps run until this is spent, at least one a frame.
/// With the frame's own work (the HUD, the loading screen, extraction) a
/// frame stays under 33 ms on an optimized build (it was 20 ms, and its
/// frames reached 30-90 ms: docs/performance.md, "A map's swap in the real
/// app"); a map takes a few frames more to come in than in one (each
/// frame's own work, the render world's above all, is paid once per frame).
const SPAWN_BUDGET: std::time::Duration = std::time::Duration::from_millis(12);

/// The most the GPU is sent a frame while a map comes in (Bevy's
/// `RenderAssetBytesPerFrame`, images' and meshes' bytes; at least one
/// asset a frame): the map's are uploaded over frames behind the loading
/// screen, not in one (32 MiB of textures cost the render thread 10-15 ms
/// a frame; meshes have their own pace, `upload_pace`). Unlimited again
/// once in the game, which waits for them (`upload_pace::uploads_settled`).
const LOAD_UPLOAD_BYTES: usize = 16 << 20;

/// A loaded map spawning a step at a time behind the loading screen
/// (`map::prepare::begin_map_change`; docs/performance.md, "A map's
/// swap"). The simulation waits meanwhile (`Time<Virtual>` paused): its
/// first tick on the map is after the whole map is in, as when it was
/// spawned in one frame, the colliders that follow the map's frame too
/// (`spawn_some` says done only then).
#[derive(Resource)]
struct Spawning {
    id: String,
    summary: String,
    /// `Time<Virtual>` was paused already (`host_timescale 0`).
    was_paused: bool,
}

/// What a map's load changes while it comes in, given back on entering the
/// game: the longest a frame may advance the simulation (one tick's worth
/// meanwhile, so a slow frame behind the loading screen leaves no
/// catch-up ticks owed) and the GPU's upload budget.
#[derive(Resource)]
struct LoadCaps {
    max_delta: std::time::Duration,
    upload: Option<usize>,
}

impl LoadCaps {
    /// Caps the frames' advance at one tick and the uploads, keeping what
    /// they were (once per load).
    fn apply(w: &mut World) {
        if w.contains_resource::<LoadCaps>() {
            return;
        }
        let tick = w.get_resource::<Time<Fixed>>().map(|t| t.timestep());
        let max_delta = match (w.get_resource_mut::<Time<Virtual>>(), tick) {
            (Some(mut time), Some(tick)) if !tick.is_zero() => {
                let before = time.max_delta();
                time.set_max_delta(tick);
                before
            }
            (Some(time), _) => time.max_delta(),
            (None, _) => return,
        };
        let upload = w
            .get_resource_mut::<bevy::render::render_asset::RenderAssetBytesPerFrame>()
            .map(|mut b| b.max_bytes.replace(LOAD_UPLOAD_BYTES))
            .unwrap_or(None);
        w.insert_resource(LoadCaps { max_delta, upload });
    }

    /// Gives them back.
    fn lift(w: &mut World) {
        let Some(caps) = w.remove_resource::<LoadCaps>() else { return };
        if let Some(mut time) = w.get_resource_mut::<Time<Virtual>>() {
            time.set_max_delta(caps.max_delta);
        }
        if let Some(mut b) = w.get_resource_mut::<bevy::render::render_asset::RenderAssetBytesPerFrame>() {
            b.max_bytes = caps.upload;
        }
    }
}

/// A map loading in the background for the `map` command.
#[derive(Resource)]
struct MapLoad {
    id: String,
    task: bevy::tasks::Task<Result<(crate::map::MapData, crate::map::prepare::Prepared), String>>,
}

/// Load map `id` in the background (from `file` when given: a copy
/// downloaded from a server), then swap it in place (`finish_map_load`):
/// everyone respawns at the new spawn points.
pub fn start_map_load(w: &mut World, id: &str, file: Option<std::path::PathBuf>) {
    w.remove_resource::<FreshGame>();
    w.insert_resource(crate::map::LoadedMapName(id.to_string()));
    // The local player's map isn't ready until the game is entered
    // (`settle_map`): what starts play waits for it.
    w.insert_resource(crate::map::LocalMapLoad { map: id.to_string() });
    // At the HDR level set now (mat_hdr_level), as the game does.
    let hdr_level = w.get_resource::<super::hdr::HdrSettings>().map_or(0, |s| s.level);
    // This load's own progress (the loading screen reads the resource).
    let progress = crate::map::loading::LoadReporter::default();
    w.insert_resource(progress.clone());
    // The app's context: what its games keep for their loads.
    let context = crate::map::LoadContext::of(w);
    let options = crate::map::MapLoadOptions { hdr_level, progress, context, ..default() };
    // The app loads it (`AppHooks`), then the spawn's own work is done
    // there too (`map::prepare`): the frame that swaps it in only spawns.
    let hooks = w.get_resource::<super::AppHooks>().copied();
    let prepare = crate::map::prepare::preparation(w);
    let task = bevy::tasks::AsyncComputeTaskPool::get().spawn({
        let id = id.to_string();
        async move {
            let hooks = hooks.ok_or("this app loads no maps")?;
            let data = match file {
                Some(f) => (hooks.load_map_file)(&id, &f, &options),
                None => (hooks.load_map)(&id, &options),
            }?;
            let prepared = prepare.run(&data);
            Ok((data, prepared))
        }
    });
    w.insert_resource(MapLoad { id: id.to_string(), task });
}

/// The server's map, for joining it (`net::NetEvent::LoadMap`): the
/// greybox at once, another in the background (from the downloaded file
/// when there is one; a bare name is the default game's). The handshake
/// checks the result.
pub fn load_server_map(w: &mut World, map: &str, file: Option<std::path::PathBuf>) {
    if map == crate::net::GREYBOX {
        load_greybox(w);
        return;
    }
    let id = match crate::game_registry::default_game(w) {
        Some(game) if !map.contains(':') => game.map_id(map),
        _ => map.to_string(),
    };
    start_map_load(w, &id, file);
}

/// Swap in a map once its background load finishes, then settle it behind
/// the loading screen (the console's system; public for a headless test
/// of the whole `map` flow, `heavy::map_swap_frames`).
pub fn finish_map_load(w: &mut World) {
    if w.contains_resource::<Settling>() {
        settle_map(w);
        return;
    }
    if w.contains_resource::<Spawning>() {
        spawn_map_steps(w, SPAWN_BUDGET);
        return;
    }
    let wait = chimera_core::tasks::waits(w);
    let Some(mut load) = w.get_resource_mut::<MapLoad>() else {
        return;
    };
    let Some(result) = chimera_core::tasks::poll(&mut load.task, wait) else {
        return;
    };
    let id = load.id.clone();
    w.remove_resource::<MapLoad>();
    let (data, prepared) = match result {
        Ok(d) => d,
        Err(e) => {
            w.remove_resource::<crate::map::LocalMapLoad>();
            w.resource_mut::<Console>()
                .print(crate::console::Level::Error, format!("map {id}: {e}"));
            super::front_end::map_load_failed(w);
            // Joining a server needs its map.
            match w.get_resource::<crate::core::NetRole>() {
                Some(crate::core::NetRole::Client) => {
                    crate::net::disconnect(w, &format!("couldn't load the server's map {id}: {e}"));
                }
                Some(crate::core::NetRole::Server) => {
                    // Still on the map that plays (its players were told of a
                    // change: `net::maps` calls it off).
                    let game = crate::game_registry::default_game(w);
                    let back = w
                        .get_resource::<crate::map::MapFile>()
                        .zip(game)
                        .map(|(f, game)| game.map_id(&f.name))
                        .unwrap_or_else(|| GREYBOX.into());
                    w.insert_resource(crate::map::LoadedMapName(back));
                    super::front_end::entered_game(w);
                }
                _ => {}
            }
            return;
        }
    };
    let summary = format!(
        "loaded {id}: {} triangles, {} spawns",
        data.triangle_count(),
        data.spawns.len()
    );
    unload_greybox(w);
    // Spawned over the next frames (`spawn_map_steps`), the simulation
    // waiting.
    crate::map::prepare::begin_map_change(w, data, prepared, crate::map::MapDebugView::Normal);
    let was_paused = w.get_resource::<Time<Virtual>>().is_none_or(|t| t.is_paused());
    if let Some(mut time) = w.get_resource_mut::<Time<Virtual>>() {
        time.pause();
    }
    LoadCaps::apply(w);
    w.insert_resource(Spawning { id, summary, was_paused });
    // `CHIMERA_SPAWN_AT_ONCE=1`: every step and the commands in this frame,
    // as before the spawn was spread (an A/B for measuring).
    if chimera_core::env::var("CHIMERA_SPAWN_AT_ONCE").is_ok_and(|v| v == "1") {
        while w.contains_resource::<Spawning>() {
            spawn_map_steps(w, std::time::Duration::MAX);
        }
        return;
    }
    spawn_map_steps(w, SPAWN_BUDGET);
}

/// A frame of `Spawning`: the map's next steps; once it is all in, the
/// game goes on with it (the tick, the look, a new game) behind the loading
/// screen while it settles (`Settling`).
fn spawn_map_steps(w: &mut World, budget: std::time::Duration) {
    if !crate::map::prepare::spawn_some(w, budget) {
        return;
    }
    let Some(Spawning { id, summary, was_paused }) = w.remove_resource::<Spawning>() else { return };
    if !was_paused && let Some(mut time) = w.get_resource_mut::<Time<Virtual>>() {
        time.unpause();
    }
    // At the gameplay's server tick (a mashup plays another game's map at
    // its gameplay's tick); without a gameplay the default game's, else
    // the map's game's.
    let tick = w.get_resource::<crate::gameplay::Gameplay>().map(|g| g.tick).or_else(|| {
        crate::game_registry::default_game(w)
            .or_else(|| {
                id.split_once(':')
                    .and_then(|(game, _)| w.get_resource::<Games>()?.get(game))
            })
            .map(|g| g.tick())
    });
    let tick = tick.unwrap_or(std::time::Duration::from_secs_f64(1.0 / crate::core::DEFAULT_TICK_HZ));
    crate::core::set_tick_length(w, tick);
    // At most a tick's worth a frame until in the game (the cap was set at
    // the old tick).
    if let Some(mut time) = w.get_resource_mut::<Time<Virtual>>() {
        time.set_max_delta(tick);
    }
    // Follow the map's presentation (Source LDR: no tonemapping).
    let tonemapping = w.resource::<crate::map::ActiveMapLook>().0.tonemapping;
    let cams: Vec<Entity> = w
        .query_filtered::<Entity, With<super::FirstPersonCamera>>()
        .iter(w)
        .collect();
    for c in cams {
        let t = if tonemapping {
            bevy::core_pipeline::tonemapping::Tonemapping::default()
        } else {
            bevy::core_pipeline::tonemapping::Tonemapping::Linear
        };
        w.entity_mut(c).insert(t);
    }
    crate::gameplay::new_game(w);
    w.resource_mut::<Console>().info(summary);
    let tick = w.get_resource::<crate::core::SimClock>().map_or(0, |c| c.tick);
    w.insert_resource(Settling {
        id,
        tick,
        started: std::time::Instant::now(),
    });
    // A network client predicts once in the game (`net::predict`).
    w.insert_resource(crate::map::MapSettling);
}

/// A frame of `Settling`: every hook's slice, and into the game when they are
/// all done.
fn settle_map(w: &mut World) {
    let settling = w.resource::<Settling>();
    let (from, started) = (settling.tick, settling.started);
    let tick = w.get_resource::<crate::core::SimClock>().map_or(from + SETTLE_AFTER_TICKS, |c| c.tick);
    if tick < from + SETTLE_AFTER_TICKS {
        return;
    }
    let hooks: Vec<fn(&mut World) -> bool> =
        w.get_resource::<crate::map::MapHooks>().map(|h| h.settle.clone()).unwrap_or_default();
    let mut done = true;
    for hook in hooks {
        done &= hook(w);
    }
    if !done && started.elapsed().as_secs_f32() < SETTLE_MAX_SECS {
        return;
    }
    info!("map settled behind the loading screen in {:.2} s", started.elapsed().as_secs_f32());
    w.remove_resource::<Settling>();
    w.remove_resource::<crate::map::MapSettling>();
    w.remove_resource::<crate::map::LocalMapLoad>();
    LoadCaps::lift(w);
    // A HUD still being shown (out of time) gives the HUD back.
    w.remove_resource::<crate::gameplay::HudWarm>();
    super::front_end::entered_game(w);
    super::net::listen_if_hosting(w);
    if w.remove_resource::<FreshGame>().is_some() {
        super::front_end::start_game(w);
    }
}

/// The map loading is a game the `map` command starts (not a server's
/// map change or a server's map we join): the local player joins it as
/// `rules::JoinChoice` says (`join_menus::start_game`).
#[derive(Resource)]
struct FreshGame;

/// The greybox map in place of whatever map is loaded (`map greybox`, and
/// `disconnect`, which leaves it behind the main menu), everyone respawned
/// on it at the default tick.
pub fn load_greybox(w: &mut World) {
    w.remove_resource::<MapLoad>();
    w.remove_resource::<crate::map::LocalMapLoad>();
    // A spawn under way is called off (`unload_map` drops it).
    if let Some(spawning) = w.remove_resource::<Spawning>()
        && !spawning.was_paused
        && let Some(mut time) = w.get_resource_mut::<Time<Virtual>>()
    {
        time.unpause();
    }
    LoadCaps::lift(w);
    crate::map::unload_map(w);
    unload_greybox(w);
    if let Some(hooks) = w.get_resource::<super::AppHooks>().copied() {
        (hooks.respawn_greybox)(w);
    }
    w.remove_resource::<crate::map::ActiveMapLook>();
    w.insert_resource(crate::map::LoadedMapName(GREYBOX.into()));
    crate::core::set_tick_length(w, std::time::Duration::from_secs_f64(1.0 / crate::core::DEFAULT_TICK_HZ));
    let cams: Vec<Entity> = w
        .query_filtered::<Entity, With<super::FirstPersonCamera>>()
        .iter(w)
        .collect();
    for c in cams {
        let mut e = w.entity_mut(c);
        e.insert(bevy::core_pipeline::tonemapping::Tonemapping::default());
        // The map's sky drew under the view (no clear); the greybox has none.
        if let Some(mut camera) = e.get_mut::<Camera>() {
            camera.clear_color = ClearColorConfig::Default;
        }
    }
    let mut skies = w.query_filtered::<&mut Camera, With<crate::map::SkyboxCamera>>();
    for mut camera in skies.iter_mut(w) {
        camera.is_active = false;
    }
    crate::gameplay::new_game(w);
}

/// The greybox map's name for `map` and `--map`.
pub const GREYBOX: &str = "greybox";

/// What `map <name>` and `changelevel <name>` load: None for the greybox
/// (`greybox`, `chimera:greybox`), else the map's id and whether to check
/// it against the install's list. A map of the default game (`game`) by
/// its name (`de_dust2`) or its id (`cs_source:de_dust2`, as the command
/// line and `--map` take it); another game's id as given (a bare name
/// too, without a default game).
pub fn map_target(game: Option<&GameInfo>, name: &str) -> Option<(String, bool)> {
    if crate::net::normalize_map(name) == crate::net::GREYBOX {
        return None;
    }
    match (name.split_once(':'), game) {
        (Some((id, map)), Some(game)) if id.eq_ignore_ascii_case(game.id) => Some((game.map_id(map), true)),
        (None, Some(game)) => Some((game.map_id(name), true)),
        _ => Some((name.to_string(), false)),
    }
}

/// The player a command acts on (a network client's on its server).
fn local_player(w: &mut World) -> Result<Entity, String> {
    crate::console::command_player(w)
}

/// The eye and the feet relative to a character's origin
/// (`MovementState::eye_offset`, `hull_min`). As CS:S's, `getpos` prints
/// the view's origin (the eye; noclip keeps the offset it was entered
/// with) and `setpos` takes the player's origin, Source's: the feet. So a
/// pasted `setpos` line puts the camera where CS:S's was (64 units above
/// the numbers standing), and `setpos` with `getpos`'s numbers lands one
/// view offset higher, as in CS:S.
fn eye_and_feet(w: &World, p: Entity) -> (Vec3, Vec3) {
    w.get::<MovementState>(p)
        .map_or((Vec3::ZERO, Vec3::ZERO), |s| (s.eye_offset, Vec3::Y * s.hull_min.y))
}

/// The client's console commands (noclip, getpos/setpos/setang, god, kill,
/// map, ...), registered by `ConsoleUiPlugin`; public for tests.
pub fn client_commands(app: &mut App) {
    super::binds::commands(app);
    for a in ACTIONS {
        let a = *a;
        for (sign, on) in [("+", true), ("-", false)] {
            app.console_command(
                &format!("{sign}{a}"),
                &format!("{} {a} (for binds).", if on { "Start" } else { "Stop" }),
                move |w, _| {
                    let mut h = w.resource_mut::<HeldActions>();
                    match a {
                        "jump" => h.jump = on,
                        "duck" => h.duck = on,
                        "speed" => h.speed = on,
                        "walk" => h.walk = on,
                        "sprint" => h.sprint = on,
                        "forward" => h.forward = on,
                        "back" => h.back = on,
                        "moveleft" => h.moveleft = on,
                        "moveright" => h.moveright = on,
                        "attack2" => h.attack2 = on,
                        "reload" => h.reload = on,
                        "use" => h.use_key = on,
                        "showscores" => h.showscores = on,
                        "freelook" => h.freelook = on,
                        _ => h.attack = on,
                    }
                    Ok(None)
                },
            );
        }
    }
    // noclip, god, setpos, setang: a dedicated server runs them too (for a
    // client's player).
    crate::net::cheats::player_commands(app);
    app.console_command(
        "chimera:host_if_asked",
        "Host the loaded map for other players when maxplayers asks for them (what map and the menu's \
         Create Server do after a load); the command line's --map runs it after its +commands.",
        |w, _| {
            super::net::listen_if_hosting(w);
            Ok(None)
        },
    )
    .console_command(
        "getpos",
        "Your position and view, as a setpos/setang line (CS:S units).",
        |w, _| {
            let p = local_player(w)?;
            let t = *w.get::<Transform>(p).ok_or("no transform")?;
            // The view's origin, as CS:S prints it: in third person the
            // camera's (a CS:S capture's getpos was its camera).
            let third = w.get_resource::<super::view::CameraMode>().is_some_and(|m| m.third_person);
            let camera = third
                .then(|| {
                    let mut q = w.query_filtered::<&GlobalTransform, With<FirstPersonCamera>>();
                    q.iter(w).next().map(|g| g.translation())
                })
                .flatten();
            let eye = camera.unwrap_or(t.translation + eye_and_feet(w, p).0);
            let i = w.get::<crate::core::Intent>(p).ok_or("no intent")?;
            let s = Vec3::new(eye.x, -eye.z, eye.y) / 0.0254;
            Ok(Some(format!(
                "setpos {:.2} {:.2} {:.2};setang {:.2} {:.2} 0.00",
                s.x,
                s.y,
                s.z,
                -i.pitch.to_degrees(),
                (i.yaw.to_degrees() + 90.0).rem_euclid(360.0)
            )))
        },
    )
    .console_command(
        "chimera:hurtme",
        "hurtme <head|chest|stomach|leftarm|rightarm|leftleg|rightleg|generic> [amount=100] [from yaw, degrees: 0 = shot from the front] [attacker name]: a bullet hit on yourself at that hitbox (e.g. to see your ragdoll in thirdperson; killed by a named player, its death cam and freeze cam).",
        |w, a| {
            use crate::core::{Damage, DamageKind, Health, Hitgroup, Intent};
            let group = match a.first().map(|s| s.to_ascii_lowercase()).as_deref() {
                Some("head") => Hitgroup::Head,
                Some("chest") => Hitgroup::Chest,
                Some("stomach") => Hitgroup::Stomach,
                Some("leftarm") => Hitgroup::LeftArm,
                Some("rightarm") => Hitgroup::RightArm,
                Some("leftleg") => Hitgroup::LeftLeg,
                Some("rightleg") => Hitgroup::RightLeg,
                Some("generic") => Hitgroup::Generic,
                _ => return Err("chimera_hurtme <hitgroup> [amount] [from yaw] [attacker]".into()),
            };
            let attacker = match a.get(3) {
                Some(name) => {
                    let mut q = w.query::<(Entity, &Name)>();
                    let found = q
                        .iter(w)
                        .find(|(_, n)| n.as_str().eq_ignore_ascii_case(name))
                        .map(|(e, _)| e);
                    Some(found.ok_or_else(|| format!("nobody is called {name}"))?)
                }
                None => None,
            };
            let amount = a.get(1).and_then(|v| v.parse::<f32>().ok()).unwrap_or(100.0) / 100.0;
            let from = a.get(2).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0).to_radians();
            let p = local_player(w)?;
            let t = *w.get::<Transform>(p).ok_or("no transform")?;
            let yaw = w.get::<Intent>(p).ok_or("no intent")?.yaw_rotation();
            let feet = w
                .get::<avian3d::prelude::ColliderAabb>(p)
                .map_or(t.translation, |b| t.translation.with_y(b.min.y));
            let point = crate::core::hitboxes_in(w, p)
                .and_then(|h| h.0.iter().find(|b| b.group() == group).map(|b| feet + yaw * b.center))
                .unwrap_or(t.translation);
            // Shot from `from` around the facing: travelling toward it.
            let dir = yaw * Quat::from_rotation_y(from) * Vec3::Z;
            let _ = w.get::<Health>(p).ok_or("no health")?;
            w.write_message(Damage {
                force: bevy::math::Vec3::ZERO,
                target: p,
                attacker,
                amount,
                point,
                dir,
                hitgroup: group,
                kind: DamageKind::Bullet,
                weapon: None,
            });
            Ok(None)
        },
    )
    .console_command(
        "maps",
        "maps [filter]: list the maps that can be loaded (the game's, its downloads, imported).",
        |w, a| {
            let filter = a.first().map(|f| f.to_lowercase()).unwrap_or_default();
            let Some(game) = crate::game_registry::default_game(w) else {
                return Err("no game registered".into());
            };
            let lines: Vec<String> = map_names(w, Some(game))
                .into_iter()
                .filter(|m| m.contains(&filter))
                .map(|m| {
                    let from = map_source(game, &m);
                    format!(
                        "{m}{}",
                        if from == "game" {
                            String::new()
                        } else {
                            format!("  ({from})")
                        }
                    )
                })
                .collect();
            Ok(Some(if lines.is_empty() {
                "no maps match".into()
            } else {
                format!("{}\n{} maps", lines.join("\n"), lines.len())
            }))
        },
    )
    .console_command(
        "chimera:import",
        "import <file.bsp|file.bsp.bz2>: copy a map into Chimera's content cache (never the game folder).",
        |w, a| {
            let path = a.join(" ");
            if path.is_empty() {
                return Err("import <file.bsp|file.bsp.bz2>".into());
            }
            let game = crate::game_registry::default_game(w).ok_or("no game registered")?;
            let name = (game.import_map)(std::path::Path::new(&path))?;
            forget_map_names(w);
            Ok(Some(format!("imported {name}; load it with: map {name}")))
        },
    )
    .console_command(
        "map",
        "map <name>: load a map of the game (Tab lists the install's maps); map greybox: Chimera's test map. \
         Hosting, it takes the players along (as changelevel); on a client, it leaves the server first.",
        |w, a| {
            let name = a.first().ok_or("map <name>")?.clone();
            if w.get_resource::<crate::core::NetRole>() == Some(&crate::core::NetRole::Client) {
                crate::net::disconnect(w, crate::net::client::BY_USER);
            }
            let game = crate::game_registry::default_game(w);
            let Some((id, check)) = map_target(game, &name) else {
                w.resource_mut::<Console>().submit("bot_kick");
                load_greybox(w);
                super::front_end::entered_game(w);
                super::net::listen_if_hosting(w);
                super::front_end::start_game(w);
                return Ok(Some("loaded the greybox".into()));
            };
            let short = crate::net::maps::map_name(&id).to_string();
            let names = map_names(w, game);
            if check && !names.is_empty() && !names.iter().any(|m| *m == short) {
                return Err(format!("no map \"{short}\" in the install"));
            }
            start_map_load(w, &id, None);
            // A game of our own starts: we join it once it's loaded.
            w.insert_resource(FreshGame);
            Ok(Some(format!("loading {id}...")))
        },
    )
    .console_command(
        "changelevel",
        "changelevel <name>: the server moves to another map; connected players load it and play on there \
         (bots stay). In a party app with no name: the minigame's next match, its map drawn as a rotation's.",
        |w, a| {
            if w.get_resource::<crate::core::NetRole>() != Some(&crate::core::NetRole::Server) {
                // Source's words.
                return Err("Can't changelevel, not running server.".into());
            }
            let Some(name) = a.first().cloned() else {
                // A party app's minigame: its next match, the map drawn
                // as a rotation's (for the players present).
                return crate::net::host_next_level(w).unwrap_or(Err("changelevel <name>".into()));
            };
            let game = crate::game_registry::default_game(w);
            let target = map_target(game, &name);
            // A party app's minigame: a new match there (its lobby), not
            // only the map.
            let id = target.as_ref().map_or(crate::net::GREYBOX, |(id, _)| id.as_str()).to_string();
            if let Some(r) = crate::net::host_changelevel(w, &id) {
                return r;
            }
            let Some((id, check)) = target else {
                load_greybox(w);
                super::front_end::entered_game(w);
                return Ok(Some("changed level to the greybox".into()));
            };
            let short = crate::net::maps::map_name(&id).to_string();
            let names = map_names(w, game);
            if check && !names.is_empty() && !names.iter().any(|m| *m == short) {
                return Err(format!("no map \"{short}\" in the install"));
            }
            start_map_load(w, &id, None);
            Ok(Some(format!("changing level to {id}...")))
        },
    )
    .console_command(
        "disconnect",
        "Leave the game (or stop hosting it): bots kicked, the map unloaded, back to the main menu.",
        |w, _| {
            crate::net::disconnect(w, "Disconnect by user.");
            // A game without bots (HL2:DM) has no such command.
            let mut console = w.resource_mut::<Console>();
            if console.command("bot_kick").is_some() {
                console.submit("bot_kick");
            }
            load_greybox(w);
            super::front_end::left_game(w);
            Ok(None)
        },
    )
    .console_command("toggleconsole", "Open or close the console.", |w, _| {
        w.resource_mut::<ConsoleUi>().toggle_request = true;
        Ok(None)
    })
    .console_command(
        "quit",
        "quit [prompt]: quit (binds and changed cvars are saved); prompt (CS:S's F10): open the game menu, \
         whose Quit quits (no quit dialog yet).",
        |w, a| {
            if a.first().is_some_and(|x| x.eq_ignore_ascii_case("prompt")) {
                w.resource_mut::<Console>().submit("menu main");
                return Ok(None);
            }
            crate::net::disconnect(w, "Quit.");
            w.write_message(AppExit::Success);
            Ok(None)
        },
    )
    .console_command("exit", "Quit.", |w, _| {
        crate::net::disconnect(w, "Quit.");
        w.write_message(AppExit::Success);
        Ok(None)
    })
    .console_command(
        "chimera:con_input",
        "con_input \"<text>\" [n]: open the console with text on its input line and its nth suggestion picked \
         (1: the first), as typing and Down would (for screenshots of the console).",
        |w, a| {
            let text = a.first().cloned().unwrap_or_default();
            let pick = a.get(1).and_then(|n| n.parse::<usize>().ok()).filter(|n| *n > 0);
            let mut ui = w.resource_mut::<ConsoleUi>();
            ui.open = true;
            ui.cursor = text.chars().count();
            ui.input = text;
            ui.highlight = pick.map(|n| n - 1);
            ui.browsing = None;
            ui.dismissed = None;
            ui.cycle = None;
            let mut roots = w.query_filtered::<&mut Visibility, With<ConsoleRoot>>();
            for mut v in roots.iter_mut(w) {
                *v = Visibility::Visible;
            }
            Ok(None)
        },
    )
    .console_command(
        "con_filter",
        "con_filter [text]: show only lines containing text (none: all).",
        |w, a| {
            w.resource_mut::<ConsoleUi>().filter = a.join(" ");
            Ok(None)
        },
    )
    .console_command(
        "chimera:con_dump",
        "con_dump [file]: write the console output to the cfg folder (condump.txt).",
        |w, a| {
            let name = a.first().map_or("condump.txt", |s| s.as_str());
            let text: Vec<String> = w.resource::<Console>().output.iter().map(|l| l.text.clone()).collect();
            let path = cfg_dir().ok_or("no config folder")?.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
            std::fs::write(&path, text.join("\n")).map_err(|e| e.to_string())?;
            Ok(Some(format!("wrote {}", path.display())))
        },
    )
    .console_command(
        "chimera:con_copy",
        "con_copy [lines]: copy the console output (or its last lines) to the clipboard.",
        |w, a| {
            let output = &w.resource::<Console>().output;
            let n = match a.first() {
                Some(n) => n.parse::<usize>().map_err(|_| format!("bad line count \"{n}\""))?,
                None => output.len(),
            };
            let text: Vec<String> = output.iter().skip(output.len().saturating_sub(n)).map(|l| l.text.clone()).collect();
            let count = text.len();
            arboard::Clipboard::new()
                .and_then(|mut c| c.set_text(text.join("\n")))
                .map_err(|e| format!("clipboard: {e}"))?;
            Ok(Some(format!("copied {count} lines")))
        },
    );
}

/// Remote protocol method `chimera/console` (and its old name, `mashup/console`): params `{"line": "..."}` runs
/// the line now and returns the output lines it printed.
#[cfg(feature = "dev")]
pub fn remote_exec(
    bevy::ecs::system::In(params): bevy::ecs::system::In<Option<serde_json::Value>>,
    world: &mut World,
) -> bevy::remote::BrpResult {
    let line = params
        .as_ref()
        .and_then(|p| p.get("line"))
        .and_then(|l| l.as_str())
        .ok_or(bevy::remote::BrpError {
            code: bevy::remote::error_codes::INVALID_PARAMS,
            message: "expected {\"line\": \"command\"}".into(),
            data: None,
        })?
        .to_string();
    let start = world.resource::<Console>().printed;
    {
        let mut c = world.resource_mut::<Console>();
        c.print(Level::Input, format!("] {line}"));
        c.submit(line);
    }
    crate::console::run_queue(world);
    // The lines printed since (the output keeps only the last ones, and
    // logs print into it too).
    let c = world.resource::<Console>();
    let new = ((c.printed - start).saturating_sub(1) as usize).min(c.output.len());
    let out: Vec<String> = c.output[c.output.len() - new..]
        .iter()
        .map(|l| l.text.clone())
        .collect();
    Ok(serde_json::json!({ "output": out }))
}


#[cfg(test)]
mod tests {

    #[test]
    fn closing_with_the_console_key_captures_the_mouse() {
        use bevy::window::{CursorGrabMode, CursorOptions};
        let mut app = App::new();
        app.init_resource::<ConsoleUi>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, toggle);
        app.world_mut().spawn((ConsoleRoot, Visibility::Hidden));
        let window = app.world_mut().spawn(CursorOptions::default()).id();
        let press = |app: &mut App, key: KeyCode| {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.clear();
            keys.press(key);
            app.update();
            app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(key);
        };
        let grabbed = |app: &App| app.world().get::<CursorOptions>(window).unwrap().grab_mode != CursorGrabMode::None;
        press(&mut app, KeyCode::Backquote);
        assert!(app.world().resource::<ConsoleUi>().open && !grabbed(&app));
        press(&mut app, KeyCode::Backquote);
        assert!(
            !app.world().resource::<ConsoleUi>().open && grabbed(&app),
            "tilde should recapture"
        );
        press(&mut app, KeyCode::Backquote);
        press(&mut app, KeyCode::Escape);
        assert!(
            !app.world().resource::<ConsoleUi>().open && !grabbed(&app),
            "escape leaves the mouse free"
        );
    }

    use super::*;
    use crate::console::ConsolePlugin;

    /// Exit with changed settings: a play session writes `config.cfg`, every
    /// tool run leaves it alone. (`CHIMERA_CFG_DIR` is only read by this test
    /// and `cfg_dir`, so the other tests don't care.)
    #[test]
    fn tool_runs_do_not_write_the_config_on_exit() {
        let dir = std::env::temp_dir().join(format!("chimera_toolrun_cfg_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // SAFETY: no other test in this binary reads or writes this variable.
        unsafe { std::env::set_var("CHIMERA_CFG_DIR", &dir) };
        let exits_with = |flags: &str| {
            let args = crate::client::Args::parse_from(flags.split_whitespace().map(String::from)).unwrap();
            let mut app = App::new();
            app.add_plugins(ConsolePlugin)
                .insert_resource(crate::client::ClientArgs(args))
                .add_systems(Last, save_on_exit);
            app.world_mut().resource_mut::<Console>().dirty = true;
            app.world_mut().write_message(AppExit::Success);
            app.update();
            dir.join("config.cfg").exists()
        };
        for flags in ["--screenshot a.png", "--views v.json", "--frames 10", "--bench"] {
            assert!(!exits_with(flags), "{flags} wrote the config");
        }
        assert!(exits_with("+map de_dust2"), "a play session saves the config");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The install is read once per game, games keep separate lists, and
    /// an import (`forget`) reads again; two apps' lists are their own.
    #[test]
    fn map_lists_read_each_games_install_once() {
        let calls = std::cell::Cell::new(0);
        let read = |names: &[&str]| {
            calls.set(calls.get() + 1);
            names.iter().map(|n| n.to_string()).collect::<Vec<_>>()
        };
        let lists = MapLists::default();
        assert_eq!(lists.names_by("a", || read(&["x"])), ["x"]);
        assert_eq!(lists.names_by("a", || read(&["never"])), ["x"]);
        assert_eq!(calls.get(), 1, "asked twice, read once");
        assert_eq!(lists.names_by("b", || read(&["y"])), ["y"]);
        assert_eq!(lists.names_by("a", || read(&["never"])), ["x"], "b didn't drop a's");
        assert_eq!(calls.get(), 2);
        let other = MapLists::default();
        assert_eq!(other.names_by("a", || read(&["z"])), ["z"], "another app's own");
        lists.forget();
        assert_eq!(lists.names_by("a", || read(&["w"])), ["w"]);
        let mut w = World::new();
        w.insert_resource(MapLists::default());
        forget_map_names(&w);
    }

    /// Two apps in one process load a map each: each loading screen sees
    /// its own load's progress (`LoadReporter`), not the other's.
    #[test]
    fn two_apps_keep_their_own_load_progress() {
        use crate::map::loading::LoadReporter;
        fn hooks() -> super::super::AppHooks {
            super::super::AppHooks {
                build: "test",
                load_map: |id, options| {
                    options.progress.report(if id == "g:a" { 0.25 } else { 0.75 }, "stage");
                    Err("stop".into())
                },
                load_map_file: |_, _, _| Err("no".into()),
                map_file_bytes: |_| None,
                unload_greybox: |_| {},
                respawn_greybox: |_| {},
            }
        }
        bevy::tasks::AsyncComputeTaskPool::get_or_init(bevy::tasks::TaskPool::default);
        let (mut a, mut b) = (World::new(), World::new());
        for w in [&mut a, &mut b] {
            w.insert_resource(hooks());
            w.init_resource::<LoadReporter>();
        }
        start_map_load(&mut a, "g:a", None);
        start_map_load(&mut b, "g:b", None);
        for w in [&mut a, &mut b] {
            let task = &mut w.resource_mut::<MapLoad>().task;
            assert!(bevy::tasks::block_on(task).is_err());
        }
        let seen = |w: &World| w.resource::<LoadReporter>().current().map(|p| p.fraction);
        assert_eq!(seen(&a), Some(0.25));
        assert_eq!(seen(&b), Some(0.75));
    }

    #[test]
    fn map_names_with_and_without_their_game() {
        static GAME: GameInfo = GameInfo {
            id: "some_game",
            short_name: "SG",
            content_dir: "sg",
            tick_interval: 0.015,
            open: |_| Err(std::io::ErrorKind::NotFound.into()),
            map_cycle: |_| Vec::new(),
            map_names: |_| Vec::new(),
            map_file: |n| Some(format!("maps/{n}.bsp")),
        steam: None,
            import_map: |_| Err("no".into()),
            default_binds_cfg: "cfg/config_default.cfg",
            default_binds: &[],
            old_default_binds: &[],
            movement_cvars: &[],
            meters_per_unit: 0.0254,
            console_namespaces: &[],
        };
        let target = |name: &str| map_target(Some(&GAME), name);
        let sg = |m: &str| Some((format!("some_game:{m}"), true));
        assert_eq!(target("de_dust2"), sg("de_dust2"));
        // As `+map` and `--map` give it.
        assert_eq!(target("some_game:de_dust2"), sg("de_dust2"));
        assert_eq!(target("SOME_GAME:de_dust2"), sg("de_dust2"));
        assert_eq!(target("greybox"), None);
        assert_eq!(target("mashup:greybox"), None);
        assert_eq!(target("other:arena"), Some(("other:arena".into(), false)));
        // Without a default game, maps need their game's id.
        assert_eq!(map_target(None, "some_game:x"), Some(("some_game:x".into(), false)));
        assert_eq!(map_target(None, "greybox"), None);
    }

    /// Every game's map list (and cycle) is read on a task at startup, so
    /// the first `map` command doesn't open the install on its frame.
    #[test]
    fn map_lists_are_read_ahead_off_the_frame() {
        static GAME: GameInfo = GameInfo {
            id: "ahead_game",
            short_name: "AG",
            content_dir: "ag",
            tick_interval: 0.015,
            open: |_| Err(std::io::ErrorKind::NotFound.into()),
            map_cycle: |_| Vec::new(),
            map_names: |_| Vec::new(),
            map_file: |n| Some(format!("maps/{n}.bsp")),
            steam: None,
            import_map: |_| Err("no".into()),
            default_binds_cfg: "cfg/config_default.cfg",
            default_binds: &[],
            old_default_binds: &[],
            movement_cvars: &[],
            meters_per_unit: 0.0254,
            console_namespaces: &[],
        };
        bevy::tasks::AsyncComputeTaskPool::get_or_init(bevy::tasks::TaskPool::default);
        let mut app = App::new();
        let mut games = Games::default();
        games.register(&GAME);
        app.insert_resource(games)
            .init_resource::<MapLists>()
            .add_systems(Startup, read_map_lists_ahead)
            .add_systems(Update, take_map_lists_ahead);
        for _ in 0..200 {
            app.update();
            if !app.world().contains_resource::<MapListsAhead>() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let lists = app.world().resource::<MapLists>();
        assert!(!app.world().contains_resource::<MapListsAhead>(), "the read finished");
        assert_eq!(lists.names_by("ahead_game", || panic!("read on the frame")), Vec::<String>::new());
        assert!(lists.cycles.lock().unwrap().contains_key("ahead_game"));
    }

    #[test]
    fn fuzzy_ranking() {
        // Prefix beats substring beats scattered letters.
        let mut names = vec!["sv_airaccelerate", "sv_accelerate", "cl_showpos", "sv_autobunnyhopping"];
        names.sort_by_key(|n| score(n, "sv_a").unwrap_or(i32::MAX));
        assert_eq!(names[0], "sv_accelerate");
        assert!(score("sv_enablebunnyhopping", "bunny").unwrap() < score("sv_enablebunnyhopping", "sebh").unwrap());
        assert!(score("sv_enablebunnyhopping", "sebh").is_some(), "letters in order");
        assert!(score("cl_showpos", "xyz").is_none());
    }

    #[test]
    fn selections_cover_text_dragged_either_way() {
        let rows: Vec<String> = ["alpha beta", "gamma", "delta epsilon"].map(String::from).to_vec();
        let sel = ((0, 6), (2, 5));
        assert_eq!(selected_text(&rows, sel), "beta\ngamma\ndelta");
        assert_eq!(selected_text(&rows, (sel.1, sel.0)), "beta\ngamma\ndelta");
        assert_eq!(selected_text(&rows, ((1, 4), (1, 1))), "amm");
        assert_eq!(row_part(sel, 1, 5), Some(0..5));
        assert_eq!(row_part(sel, 3, 5), None);
    }

    /// The console laid out as the game lays it out (Bevy's UI in a small
    /// window): dragging over its output selects the text between the
    /// pointer's two characters, whole rows between, and what is selected
    /// is what Ctrl+C would copy.
    #[test]
    fn dragging_selects_part_of_a_line() {
        use crate::client::widgets::{PointerScript, pointer_test_app};
        let mut app = pointer_test_app();
        app.add_plugins(ConsolePlugin)
            .init_resource::<crate::client::fonts::UiFonts>()
            .init_resource::<ConsoleUi>()
            .add_systems(Startup, spawn_ui)
            .add_systems(Update, (draw_console, select_text).chain());
        app.world_mut().resource_mut::<ConsoleUi>().open = true;
        for line in ["first line of output", "second line of output", "third"] {
            app.world_mut().resource_mut::<Console>().info(line);
        }
        for _ in 0..4 {
            app.update();
        }
        let rows = |app: &mut App| {
            let mut q = app.world_mut().query::<(&ConsoleRow, &ComputedNode, &UiGlobalTransform)>();
            let mut v: Vec<_> = q
                .iter(app.world())
                .map(|(r, n, t)| (r.0, t.translation - n.size() / 2.0, n.size()))
                .collect();
            v.sort_by_key(|r| r.0);
            v
        };
        let r = rows(&mut app);
        assert!(r.len() >= 3, "rows laid out: {r:?}");
        let (second, third) = (r[r.len() - 2], r[r.len() - 1]);
        let y2 = second.1.y + second.2.y / 2.0;
        let y3 = third.1.y + third.2.y / 2.0;
        // From just inside the second row's left edge to well right of the
        // third row's text: the second row whole, the third whole.
        let drag = |app: &mut App, from: Vec2, to: Vec2| {
            app.world_mut().resource_mut::<PointerScript>().drag(from, to, 3);
            for _ in 0..14 {
                app.update();
            }
        };
        drag(&mut app, Vec2::new(second.1.x + 1.0, y2), Vec2::new(390.0, y3));
        let ui = app.world().resource::<ConsoleUi>();
        let sel = ui.selection.expect("a selection");
        assert_eq!(selected_text(&ui.shown, sel), "second line of output\nthird");
        // Inside one line: a part of it, neither end of the row.
        let w = second.2.x.min(300.0);
        drag(&mut app, Vec2::new(second.1.x + w * 0.1, y2), Vec2::new(second.1.x + w * 0.3, y2));
        let ui = app.world().resource::<ConsoleUi>();
        let sel = ui.selection.expect("a selection");
        let text = selected_text(&ui.shown, sel);
        assert!(!text.is_empty() && text.len() < "second line of output".len(), "part of the line: {text:?}");
        assert!("second line of output".contains(&text), "{text:?}");
        // The selected span is drawn as its own span of the row's text.
        let mut spans = app.world_mut().query::<&bevy::text::TextBackgroundColor>();
        assert_eq!(spans.iter(app.world()).count(), 1);
        // A press elsewhere clears it.
        app.world_mut().resource_mut::<PointerScript>().click(Vec2::new(5.0, 290.0));
        for _ in 0..8 {
            app.update();
        }
        assert!(app.world().resource::<ConsoleUi>().selection.is_none());
        assert_eq!(spans.iter(app.world()).count(), 0);
    }

    #[test]
    fn completion_candidates() {
        let mut app = App::new();
        app.add_plugins(ConsolePlugin);
        app.init_resource::<HeldActions>();
        client_commands(&mut app);
        app.console_cvar(
            "sv_enablebunnyhopping",
            "bhop",
            "0",
            |_| Some("0".into()),
            |_, _| Ok(()),
        );
        let w = app.world_mut();
        let (head, list) = candidates(w, "sv_enab");
        assert_eq!(head, "");
        assert_eq!(list[0].0, "sv_enablebunnyhopping");
        // A boolean cvar offers 0 and 1.
        let (head, list) = candidates(w, "sv_enablebunnyhopping ");
        assert_eq!(head, "sv_enablebunnyhopping ");
        assert!(list.iter().any(|(v, _)| v == "1"));
        // bind: key names, then actions.
        let (_, keys) = candidates(w, "bind mwh");
        assert!(keys.iter().any(|(k, _)| k == "mwheeldown"));
        let (_, actions) = candidates(w, "bind space +ju");
        assert_eq!(actions[0].0, "+jump");
        // Commands registered by the client are names too.
        let (_, list) = candidates(w, "noc");
        assert_eq!(list[0].0, "noclip");
    }

    /// A console with its editing system, open, and keys typed into it.
    /// config.cfg is shared by the apps: a bind holding another app's
    /// command doesn't complain on every press (the HL2:DM playtest's
    /// `choose_1`, Combat Arms'); what this app has still runs.
    #[test]
    fn a_key_press_leaves_out_commands_this_app_has_not() {
        let mut app = App::new();
        app.add_plugins(ConsolePlugin)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<MouseWheel>()
            .add_systems(Update, run_binds);
        {
            let mut c = app.world_mut().resource_mut::<Console>();
            c.binds.insert("1".into(), "slot1; choose_1".into());
            c.binds.insert("2".into(), "choose_2; echo still here".into());
        }
        for key in [KeyCode::Digit1, KeyCode::Digit2] {
            app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
            app.update();
            app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(key);
            app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
            app.update();
        }
        let text: Vec<String> = app.world().resource::<Console>().output.iter().map(|l| l.text.clone()).collect();
        assert!(!text.iter().any(|l| l.contains("Unknown command")), "{text:?}");
        assert!(text.iter().any(|l| l.contains("still here")), "{text:?}");
    }

    /// While the pre-match lobby shows, F3 is its key (the host's
    /// `lobby_start`) in place of its bind; after, the bind again.
    #[test]
    fn f3_is_the_lobbys_key_while_it_shows() {
        let mut app = App::new();
        app.add_plugins(ConsolePlugin)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<MouseWheel>()
            .add_systems(Update, run_binds);
        app.console_command("lobby_start", "test", |_, _| Ok(Some("lobby key".into())))
            .console_command("drawcollision_test", "test", |_, _| Ok(Some("bind ran".into())));
        app.world_mut().resource_mut::<Console>().binds.insert("f3".into(), "drawcollision_test".into());
        app.insert_resource(super::super::prematch_lobby::LobbyView {
            shown: true,
            hosting: true,
            ..default()
        });
        let press = |app: &mut App| {
            app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::F3);
            app.update();
            app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::F3);
            app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
            app.update();
            let text: Vec<String> = app.world().resource::<Console>().output.iter().map(|l| l.text.clone()).collect();
            app.world_mut().resource_mut::<Console>().output.clear();
            text
        };
        let text = press(&mut app);
        assert!(text.iter().any(|l| l.contains("lobby key")) && !text.iter().any(|l| l.contains("bind ran")), "{text:?}");
        app.insert_resource(super::super::prematch_lobby::LobbyView::default());
        let text = press(&mut app);
        assert!(text.iter().any(|l| l.contains("bind ran")) && !text.iter().any(|l| l.contains("lobby key")), "{text:?}");
    }

    fn typing_app() -> App {
        let mut app = App::new();
        app.add_plugins(ConsolePlugin)
            .init_resource::<HeldActions>()
            .init_resource::<ConsoleUi>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<KeyboardInput>()
            .add_message::<MouseWheel>()
            .init_resource::<Overlays>()
            .add_systems(Update, edit);
        client_commands(&mut app);
        overlay_cvars(&mut app);
        app.console_cvar("sv_gravity", "Gravity.", "800", |_| Some("800".into()), |_, _| Ok(()));
        app.world_mut().resource_mut::<Console>().set_range("sv_gravity", 0.0, 2000.0);
        app.world_mut().resource_mut::<ConsoleUi>().open = true;
        app
    }

    fn press(app: &mut App, key_code: KeyCode, logical_key: Key) {
        app.world_mut().write_message(KeyboardInput {
            key_code,
            logical_key,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
    }

    fn type_text(app: &mut App, text: &str) {
        for c in text.chars() {
            if c == ' ' {
                press(app, KeyCode::Space, Key::Space);
            } else {
                press(app, KeyCode::KeyQ, Key::Character(c.to_string().into()));
            }
        }
    }

    fn input(app: &App) -> String {
        app.world().resource::<ConsoleUi>().input.clone()
    }

    #[test]
    fn enter_on_a_picked_suggestion_takes_it_and_leaves_room_for_arguments() {
        let mut app = typing_app();
        type_text(&mut app, "sv_grav");
        let (_, list) = suggestions(app.world_mut()).expect("a list shows");
        assert_eq!(list[0].0, "sv_gravity");
        // Down picks the first, Up from there wraps to the last.
        press(&mut app, KeyCode::ArrowDown, Key::ArrowDown);
        assert_eq!(app.world().resource::<ConsoleUi>().highlight, Some(0));
        press(&mut app, KeyCode::ArrowUp, Key::ArrowUp);
        assert_eq!(app.world().resource::<ConsoleUi>().highlight, Some(list.len() - 1));
        press(&mut app, KeyCode::ArrowDown, Key::ArrowDown);
        let before = app.world().resource::<Console>().output.len();
        press(&mut app, KeyCode::Enter, Key::Enter);
        assert_eq!(input(&app), "sv_gravity ", "taken, with a space for the value");
        assert_eq!(app.world().resource::<Console>().output.len(), before, "nothing ran");
        // Now the help is for its value, and the list offers values.
        let typed = input(&app);
        let help = arg_help(app.world_mut(), &typed).unwrap();
        assert_eq!((help.name.as_str(), help.current), ("sv_gravity", Some(0)));
        assert!(help.detail.contains("= \"800\"") && help.detail.contains("range 0 to 2000"), "{}", help.detail);
        // Esc hides the list; Enter then runs the line.
        press(&mut app, KeyCode::Escape, Key::Escape);
        assert!(suggestions(app.world_mut()).is_none());
        press(&mut app, KeyCode::Enter, Key::Enter);
        let out = &app.world().resource::<Console>().output;
        assert!(out.iter().any(|l| l.text == "] sv_gravity "), "{:?}", out.last());
        assert_eq!(input(&app), "");
    }

    #[test]
    fn tab_takes_the_pick_and_up_browses_history_without_a_list() {
        let mut app = typing_app();
        app.world_mut().resource_mut::<ConsoleUi>().history = vec!["noclip".into(), "god".into()];
        // Nothing typed: no list, Up is history.
        press(&mut app, KeyCode::ArrowUp, Key::ArrowUp);
        assert_eq!(input(&app), "god");
        press(&mut app, KeyCode::ArrowUp, Key::ArrowUp);
        assert_eq!(input(&app), "noclip");
        // Typing leaves the history; the list is back and Tab takes a pick.
        type_text(&mut app, "; setp");
        press(&mut app, KeyCode::ArrowDown, Key::ArrowDown);
        press(&mut app, KeyCode::Tab, Key::Tab);
        assert_eq!(input(&app), "noclip; setpos ");
        let help = arg_help(app.world_mut(), "noclip; setpos 10 ").unwrap();
        assert_eq!(help.args, ["<x>", "<y>", "<z>"]);
        assert_eq!(help.current, Some(1));
        // Ctrl+Backspace deletes a word.
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ControlLeft);
        press(&mut app, KeyCode::Backspace, Key::Backspace);
        assert_eq!(input(&app), "noclip; ");
    }

    #[test]
    fn highlight_steps_and_window() {
        assert_eq!(step_highlight(None, 5, false), Some(0));
        assert_eq!(step_highlight(None, 5, true), Some(4));
        assert_eq!(step_highlight(Some(4), 5, false), Some(0));
        assert_eq!(step_highlight(Some(0), 5, true), Some(4));
        assert_eq!(step_highlight(None, 0, false), None);
        assert_eq!(suggestion_window(Some(3), 20), 0);
        assert_eq!(suggestion_window(Some(SUGGESTIONS), 20), 1);
        assert_eq!(suggestion_window(Some(19), 20), 20 - SUGGESTIONS);
        assert_eq!(accept("bind f ", "+jump"), "bind f +jump ");
        assert_eq!(accept("", "say hi"), "\"say hi\" ");
    }

    #[test]
    fn argument_completion_follows_the_argument() {
        let mut app = typing_app();
        let w = app.world_mut();
        let (_, teams) = candidates(w, "jointeam ");
        assert_eq!(teams.iter().map(|t| t.0.as_str()).collect::<Vec<_>>(), ["1", "2", "3"]);
        let (_, hits) = candidates(w, "chimera_hurtme he");
        assert_eq!(hits[0].0, "head");
        // After the hitgroup, no more hitgroups.
        let (_, after) = candidates(w, "chimera_hurtme head ");
        assert!(after.iter().all(|a| a.0 != "chest"));
        let (head, tabs) = candidates(w, "echo x; debugui pe");
        assert_eq!((head.as_str(), tabs[0].0.as_str()), ("echo x; debugui ", "perf"));
        // A cvar with a small whole range offers each value.
        let (_, values) = candidates(w, "con_timestamps ");
        assert!(values.iter().any(|v| v.0 == "1"));
    }

    #[test]
    fn the_console_fits_the_window() {
        assert!(lines_for_height(1080.0) > lines_for_height(720.0));
        assert!(lines_for_height(720.0) >= 15, "{}", lines_for_height(720.0));
        assert_eq!(lines_for_height(10.0), 4);
    }

    #[test]
    fn reverse_search_and_tokens() {
        let h = vec!["sv_gravity 800".to_string(), "noclip".into(), "sv_gravity 400".into()];
        assert_eq!(search(&h, "grav", 0).as_deref(), Some("sv_gravity 400"));
        assert_eq!(search(&h, "grav", 1).as_deref(), Some("sv_gravity 800"));
        assert_eq!(search(&h, "grav", 2), None);
        assert_eq!(
            split_last("bind space +j"),
            ("bind space ".to_string(), "+j".to_string())
        );
    }
}

/// Take the app's greybox out (`AppHooks::unload_greybox`) for a map.
fn unload_greybox(w: &mut World) {
    if let Some(hooks) = w.get_resource::<super::AppHooks>().copied() {
        (hooks.unload_greybox)(w);
    }
}
