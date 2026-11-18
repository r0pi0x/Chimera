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

use chimera_ui::clipboard::Clipboard;
use crate::client::{FirstPersonCamera, fonts::UiFonts, widgets::Interaction};
use super::*;
use crate::{
    console::{Console, ConsoleAppExt, Level, cfg_dir, parse, resource_cvar},
    core::{LocalPlayer, MovementState, Velocity},
    game_registry::{GameInfo, Games},
    map::PlaySound,
};


/// Whether the console is open, and its input state.
#[derive(Resource, Default)]
pub struct ConsoleUi {
    pub open: bool,
    pub(super) input: String,
    /// Cursor position in chars.
    pub(super) cursor: usize,
    pub(super) history: Vec<String>,
    /// Position while browsing history (None: editing a new line).
    pub(super) browsing: Option<usize>,
    /// Tab cycling: the candidates and which one is shown, and the input
    /// they complete.
    pub(super) cycle: Option<(Vec<String>, usize, String)>,
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
    pub(super) toggle_request: bool,
    /// The output lines on screen, and a selection of them (first and
    /// last row, in the order dragged) made with the mouse.
    pub(super) shown: Vec<String>,
    pub(super) selection: Option<(TextPos, TextPos)>,
    dragging: bool,
    /// `draw_console` rebuilt the rows this frame.
    fresh_rows: bool,
    /// The suggestion picked with Up/Down (an index into all of them).
    pub(super) highlight: Option<usize>,
    /// The input the suggestions were hidden at with Esc (they come back
    /// when it changes).
    pub(super) dismissed: Option<String>,
    /// Whether the suggestion list was shown last frame (Esc then hides
    /// it instead of closing the console).
    list_shown: bool,
    /// Output lines that fit the console at the window's size.
    visible_lines: usize,
    /// What the suggestion box shows, to rebuild it only on change.
    suggest_rows: Vec<SuggestRow>,
}

pub(super) const HISTORY_MAX: usize = 1000;
/// Output lines before the window's size is known.
pub(super) const DEFAULT_VISIBLE_LINES: usize = 28;
pub(super) const SUGGESTIONS: usize = 8;
/// Output text size and the height of one of its rows (pixels).
pub(super) const OUTPUT_FONT: f32 = 14.0;
pub(super) const ROW_HEIGHT: f32 = 17.0;

impl ConsoleUi {
    /// The input line as typed.
    pub fn input(&self) -> &str {
        &self.input
    }

    fn page(&self) -> usize {
        (self.visible_lines.max(2)) / 2
    }
}

/// One row of the suggestion box: coloured runs, and whether it is the
/// highlighted suggestion.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct SuggestRow {
    runs: Vec<(String, Color)>,
    highlight: bool,
}

/// Output lines that fit a console half the window tall (padding, the
/// input line and the "more below" line taken off).
pub(super) fn lines_for_height(window_height: f32) -> usize {
    (((window_height * 0.5 - 16.0 - 26.0) / ROW_HEIGHT).floor() as isize - 1).max(4) as usize
}

#[derive(Component)]
pub struct ConsoleRoot;
#[derive(Component)]
pub(super) struct ConsoleOutput;
/// One row of the output on screen.
#[derive(Component)]
pub(super) struct ConsoleRow(pub(super) usize);
#[derive(Component)]
pub(super) struct ConsoleInput;
/// The suggestion box (a child of `ConsoleRoot`).
#[derive(Component)]
pub struct ConsoleSuggest;
#[derive(Component)]
pub(super) struct OverlayText;

/// The console's text: our Mono (`UiFonts::mono`), in its own case (cvar
/// names in lower case), at our sizes (`ROW_HEIGHT`).
pub(super) fn console_font(fonts: &UiFonts, size: f32) -> TextFont {
    fonts.mono(size)
}

/// The engine's overlay text (notify lines, `cl_showpos`, `snd_show`):
/// our Mono too.
pub(super) fn overlay_font(fonts: &UiFonts, size: f32) -> TextFont {
    fonts.mono(size)
}

pub(super) fn spawn_ui(mut commands: Commands, fonts: Res<UiFonts>) {
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
            // The suggestion box, just under the console: a child, so it
            // draws wherever the console does (a party app's menu moves
            // the console onto its own UI camera, `UiTargetCamera` on the
            // root, while the menu shows; a box of its own stayed on the
            // camera that is off then, and no list showed).
            c.spawn((
                ConsoleSuggest,
                Node {
                    position_type: PositionType::Absolute,
                    top: percent(100.0),
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
        });
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
    menu: Option<Res<crate::client::front_end::MenuStatus>>,
    debug_ui: Option<Res<crate::client::debug_ui::DebugUi>>,
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
            crate::client::input::release_cursor(&mut cursor);
        } else if !close && !menu.is_some_and(|m| m.open) && !debug_ui.is_some_and(|d| d.open) {
            // Closed with the console key: straight back to playing, unless
            // the game menu or the debug UI is open under it. (Escape
            // leaves the mouse free.)
            crate::client::input::capture_cursor(&mut cursor);
        }
    }
}

/// Every completion candidate for the current input: names when on the
/// first word, else the command's arguments (the argument being typed
/// decides which). Works on the line's last command (after `;`).
pub(super) fn candidates(world: &mut World, input: &str) -> (String, Vec<(String, String)>) {
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
                    crate::client::binds::KEY_NAMES
                        .iter()
                        .map(|(_, n)| *n)
                        .chain(crate::client::binds::MOUSE_NAMES.iter().map(|(_, n)| *n))
                        .chain([crate::client::binds::WHEEL_UP, crate::client::binds::WHEEL_DOWN])
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
                out.extend(crate::client::debug_ui::Tab::ALL.iter().map(|t| (t.name().to_string(), t.title().to_string())));
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
pub(super) const ENT_INPUTS: &[&str] = &[
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
pub(super) struct ArgHelp {
    pub(super) name: String,
    pub(super) args: Vec<String>,
    pub(super) current: Option<usize>,
    pub(super) detail: String,
}

pub(super) fn arg_help(world: &mut World, input: &str) -> Option<ArgHelp> {
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
pub(super) fn suggestions(world: &mut World) -> Option<(String, Vec<(String, String)>)> {
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
pub(super) fn step_highlight(current: Option<usize>, len: usize, up: bool) -> Option<usize> {
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
pub(super) fn suggestion_window(highlight: Option<usize>, len: usize) -> usize {
    match highlight {
        Some(h) if h >= SUGGESTIONS => (h + 1 - SUGGESTIONS).min(len.saturating_sub(SUGGESTIONS)),
        _ => 0,
    }
}

/// Fuzzy rank (lower is better): prefix, then substring, then the letters
/// in order; None when it doesn't match.
pub(super) fn score(name: &str, query: &str) -> Option<i32> {
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
pub(super) fn edit(world: &mut World) {
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
pub(super) fn edit_key(world: &mut World, e: &KeyboardInput, ctrl: bool, shift: bool) {
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
    // Paste (Ctrl+V, Shift+Insert): the clipboard's text, read before
    // the console's state is borrowed (`chimera_ui::clipboard`).
    let paste = (ctrl && e.key_code == KeyCode::KeyV) || (shift && e.key_code == KeyCode::Insert);
    let pasted = paste.then(|| world.get_resource_or_init::<Clipboard>().get().unwrap_or_default());
    let mut copied = None;
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
        _ if paste => {
            // Paste (one line: newlines become command separators).
            let text = pasted.unwrap_or_default();
            let text = text.trim_end().replace("\r\n", "; ").replace('\n', "; ");
            let ins: Vec<char> = text.chars().collect();
            let mut c = chars.clone();
            c.splice(cur..cur, ins.iter().copied());
            ui.input = c.into_iter().collect();
            ui.cursor = cur + ins.len();
        }
        (_, KeyCode::KeyC) | (_, KeyCode::Insert) if ctrl => {
            copied = selection_text(&ui);
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
    if let Some(text) = copied {
        world.get_resource_or_init::<Clipboard>().set(&text);
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
pub(super) fn accept(head: &str, suggestion: &str) -> String {
    format!("{head}{} ", crate::console::quote(suggestion))
}

/// Split off the token being completed.
pub(super) fn split_last(input: &str) -> (String, String) {
    match input.rfind(' ') {
        Some(i) => (input[..=i].to_string(), input[i + 1..].to_string()),
        None => (String::new(), input.to_string()),
    }
}

/// The `back`-th most recent history entry containing `query`.
pub(super) fn search(history: &[String], query: &str, back: usize) -> Option<String> {
    history.iter().rev().filter(|h| h.contains(query)).nth(back).cloned()
}

pub(super) fn submit(world: &mut World) {
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

pub(super) fn history_path() -> Option<std::path::PathBuf> {
    cfg_dir().map(|d| d.join("history.txt"))
}

pub(super) fn save_history(history: &[String]) {
    // Unit tests type into the console: never into the player's history.
    if cfg!(test) {
        return;
    }
    if let Some(p) = history_path() {
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        let _ = std::fs::write(p, history.join("\n"));
    }
}

pub(super) fn load_history(mut ui: ResMut<ConsoleUi>) {
    if let Some(text) = history_path().and_then(|p| std::fs::read_to_string(p).ok()) {
        ui.history = text.lines().filter(|l| !l.is_empty()).map(String::from).collect();
    }
}

/// config.cfg (binds, aliases, changed cvars) then autoexec.cfg, then the
/// command line's `+commands`.
pub(super) fn autoexec(
    mut console: ResMut<Console>,
    mut ui: ResMut<ConsoleUi>,
    args: Res<crate::client::ClientArgs>,
    games: Option<Res<Games>>,
    gameplay: Option<Res<crate::gameplay::Gameplay>>,
    mut defaults: ResMut<crate::client::binds::BindDefaults>,
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
    let marked = config.as_ref().is_none_or(|t| t.contains(crate::client::binds::CONFIG_MARK));
    let current = config.as_ref().is_none_or(|t| t.contains(crate::client::binds::DEFAULTS_MARK));
    console.submit("binddefaults; execifexists config.cfg");
    if !marked {
        console.submit("binddefaults missing");
    } else if !current {
        // Defaults added since the config was written (CS:S's F1, F2,
        // wheel, ...), on the keys it leaves free: once, as the config is
        // written again with the mark.
        console.submit("binddefaults new");
    }
    if !config.as_ref().is_none_or(|t| t.contains(OPTIONS_MARK)) {
        // Written before the party's options fix: undo what it saved.
        console.submit("chimera_repair_options");
    }
    // The app's own defaults (`binds::AppBind`: Ultimate Gun Game's F10)
    // on a saved config, once: where the key is free or still holds the
    // default they replace.
    // The mark is written by an app that has them, or read them (so an
    // app without its own keeps the mark of one that has).
    let app_marked = config.as_ref().is_some_and(|t| t.contains(crate::client::binds::APP_BINDS_MARK));
    if !defaults.app_binds().is_empty() {
        if config.is_some() && !app_marked {
            console.submit("binddefaults app");
        }
        console.config_marks.insert(crate::client::binds::APP_BINDS_MARK.into());
    } else if app_marked {
        console.config_marks.insert(crate::client::binds::APP_BINDS_MARK.into());
    }
    // Each game's own binds (`Console::layers`) a saved config lacks
    // whose command no key runs in that game (a config written before
    // the game had them: Halo's throw), back on their keys if free.
    console.submit("bindgamedefaults missing");
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

/// A config.cfg written since the party apps keep their own sound and
/// brightness says so (`console::config_text`'s fourth line). One without
/// it may carry what Combat Arms' options, its profile not loaded, set at
/// each start of the party apps until the fix of 2026-11-14 (`combat_arms::ui::
/// options_apply::ShellOptions`) and the config then saved: `volume 0`
/// with `snd_musicvolume 0`, and `mat_monitorgamma` at Combat Arms' default
/// gamma's (2.2 / 1.22): `chimera_repair_options` puts those back to the
/// defaults, once (the config is written again with the mark).
pub const OPTIONS_MARK: &str = "// shell options: 1";

/// The brightness Combat Arms' default gamma (1.22) set in the party apps
/// (`options_apply::monitor_gamma`), the saved bug's signature.
pub const CA_GAMMA_SAVED: f32 = 2.2 / 1.22;

/// `chimera_repair_options` (`OPTIONS_MARK`): the lines that put back the
/// options the party's bug saved, from the values the config gave
/// (`volume`, `snd_musicvolume`, `mat_monitorgamma`; None: unset).
pub fn repair_options(volume: Option<f32>, music: Option<f32>, gamma: Option<f32>) -> Vec<String> {
    let mut out = Vec::new();
    if volume == Some(0.0) && music == Some(0.0) {
        out.push(format!("volume {}", crate::client::audio::DEFAULT_VOLUME));
        out.push("snd_musicvolume 1".to_string());
    }
    if gamma.is_some_and(|g| (g - CA_GAMMA_SAVED).abs() < 1e-3) {
        out.push(format!("mat_monitorgamma {}", crate::client::gamma::DEFAULT_GAMMA));
    }
    out
}

/// Save binds and changed cvars when the game closes.
pub(super) fn save_on_exit(mut exits: MessageReader<AppExit>, args: Res<crate::client::ClientArgs>, mut commands: Commands) {
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

pub(super) fn draw_console(
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
pub(super) type TextPos = (usize, usize);

/// The selection's ends in reading order.
pub(super) fn ordered((a, b): (TextPos, TextPos)) -> (TextPos, TextPos) {
    if a <= b { (a, b) } else { (b, a) }
}

/// The byte range of row `row` the selection covers (`len`: its text's length).
pub(super) fn row_part(sel: (TextPos, TextPos), row: usize, len: usize) -> Option<std::ops::Range<usize>> {
    let (s, e) = ordered(sel);
    if row < s.0 || row > e.0 {
        return None;
    }
    let from = if row == s.0 { s.1.min(len) } else { 0 };
    let to = if row == e.0 { e.1.min(len) } else { len };
    (from < to).then_some(from..to)
}

/// The selected text (rows joined by newlines).
pub(super) fn selected_text(rows: &[String], sel: (TextPos, TextPos)) -> String {
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
/// span drawn highlighted; Ctrl+C copies it (`selection_text`), as CS:S's
/// console does.
pub(super) fn select_text(
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

/// The text selected in the output, for Ctrl+C (Ctrl+Insert) to put on
/// the clipboard; none without a selection.
pub(super) fn selection_text(ui: &ConsoleUi) -> Option<String> {
    let sel = ui.selection.filter(|(a, b)| a != b)?;
    Some(selected_text(&ui.shown, sel))
}

/// The suggestion box under the console as you type: the help line for
/// the command being typed (its arguments, the one being typed marked),
/// then the suggestions with the picked one highlighted (needs the world
/// to read cvar values).
pub(super) fn overlay_text(world: &mut World) {
    let rows = suggest_rows(world);
    let shown = !rows.is_empty();
    world.resource_mut::<ConsoleUi>().list_shown = shown && suggestions(world).is_some();
    // Shown: as the console (its parent) is.
    let want = if shown { Visibility::Inherited } else { Visibility::Hidden };
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
pub(super) fn suggest_rows(world: &mut World) -> Vec<SuggestRow> {
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
pub(super) fn watch_text(world: &mut World) {
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

