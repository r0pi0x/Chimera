//! Key binds, as Source keeps them: key name -> console line
//! (`console::Console::binds`), saved in config.cfg as `bind` lines.
//! Every game key is a bind: the defaults (`default_binds`) go in before
//! config.cfg runs, `binddefaults` puts them back (the options' "Use
//! Defaults"), and the options' keyboard tab rebinds them.
//!
//! The defaults are the game's data (`game_registry::GameInfo`; nothing
//! here names a game's keys): the default game's, from its install's file
//! (`GameInfo::default_binds_cfg`, CS:S's `cfg/config_default.cfg`, read at
//! start by `BindDefaults::read_install`), else its built-in table
//! (`GameInfo::default_binds`); plus Chimera's own, the only engine
//! defaults, on the keys the game leaves free (`CHIMERA_BINDS`: Alt free
//! look, V noclip, F9 `bug`, the debug keys F3 `toggle
//! chimera_drawcollision`, F8 `debugui`, F11 `chimera_inspector`).
//!
//! Saved configs: `binddefaults missing` for configs written before every
//! key was a bind (they lack `CONFIG_MARK`); `binddefaults new` once after
//! a config lacking `DEFAULTS_MARK`, which puts `added_defaults` (the
//! defaults not among the game's `GameInfo::old_default_binds`) on free
//! keys, so older configs get defaults added since; saved binds win.
//!
//! Binds whose whole line is polled (Chimera's `POLLED`: `+forward`,
//! `slot1`, `lastinv` ...; a game's own, `BindsAppExt::polled_bind`:
//! CS:S's `buymenu`, `radio1`; the `+name` of each named action,
//! `BindsAppExt::bound_action`) are read by the system that owns the
//! action, from the keys held this frame (`pressed`, `just_pressed`,
//! `wheel_notches`), with that system's own rules (only while playing,
//! not while typing ...); `PolledBinds::runnable`, used by
//! `console::run_binds`, leaves them alone. Every other bind runs its
//! line through the console. A key may hold several commands (`a; b`, a
//! game's two actions on one key): `keys_for` finds it for each, and only
//! the parts that are not polled run. Source key names, rebind and clear
//! lines for the options.
//!
//! config.cfg is shared by every app on the machine (`console::cfg_dir`),
//! so a bind may name a command this app doesn't have (Combat Arms' keys 1
//! to 6 hold `slotN; choose_N`, `choose_N` being its own). A key press
//! leaves those parts out (`known_here`), without a word: typing one in the
//! console still says "Unknown command", and the line stays as it is, so
//! config.cfg keeps it for the app that has the command.
//!
//! Named actions: a bind is still a line, so `bind mouse4 +throw` is the
//! same in saved configs and profiles as before; `+throw` names the action
//! `throw` the game registered (`core::ActionRegistry`), and the local
//! intent source sets its bit in `Intent::actions` while a key bound to it
//! is held. The fixed buttons (`+jump`, `+attack` ...) stay Chimera's own
//! aliases (the fields of `Intent`).
//!
//! Menus that read keys take them (`CapturedKeys`): the polled binds of
//! `input` and `weapon_menu` don't see a captured key until it is up.
//!
//! Known shortcuts: a named action is only a held button: it has no
//! console `+throw` typed-hold (`console::HeldActions` keeps Chimera's
//! own) and no wheel notch. The defaults are read once at start for the
//! gameplay's `binds` game (`BindDefaults`): a gameplay switch later
//! doesn't re-read them.

use std::collections::BTreeMap;

use bevy::{
    input::mouse::{AccumulatedMouseScroll, MouseScrollUnit},
    prelude::*,
};

use crate::{
    console::{Console, ConsoleAppExt},
    game_registry::GameInfo,
};

/// Source key names for keyboard keys.
pub const KEY_NAMES: &[(KeyCode, &str)] = &[
    (KeyCode::KeyA, "a"),
    (KeyCode::KeyB, "b"),
    (KeyCode::KeyC, "c"),
    (KeyCode::KeyD, "d"),
    (KeyCode::KeyE, "e"),
    (KeyCode::KeyF, "f"),
    (KeyCode::KeyG, "g"),
    (KeyCode::KeyH, "h"),
    (KeyCode::KeyI, "i"),
    (KeyCode::KeyJ, "j"),
    (KeyCode::KeyK, "k"),
    (KeyCode::KeyL, "l"),
    (KeyCode::KeyM, "m"),
    (KeyCode::KeyN, "n"),
    (KeyCode::KeyO, "o"),
    (KeyCode::KeyP, "p"),
    (KeyCode::KeyQ, "q"),
    (KeyCode::KeyR, "r"),
    (KeyCode::KeyS, "s"),
    (KeyCode::KeyT, "t"),
    (KeyCode::KeyU, "u"),
    (KeyCode::KeyV, "v"),
    (KeyCode::KeyW, "w"),
    (KeyCode::KeyX, "x"),
    (KeyCode::KeyY, "y"),
    (KeyCode::KeyZ, "z"),
    (KeyCode::Digit0, "0"),
    (KeyCode::Digit1, "1"),
    (KeyCode::Digit2, "2"),
    (KeyCode::Digit3, "3"),
    (KeyCode::Digit4, "4"),
    (KeyCode::Digit5, "5"),
    (KeyCode::Digit6, "6"),
    (KeyCode::Digit7, "7"),
    (KeyCode::Digit8, "8"),
    (KeyCode::Digit9, "9"),
    (KeyCode::Space, "space"),
    (KeyCode::Enter, "enter"),
    (KeyCode::Tab, "tab"),
    (KeyCode::Backspace, "backspace"),
    (KeyCode::CapsLock, "capslock"),
    (KeyCode::ShiftLeft, "shift"),
    (KeyCode::ShiftRight, "rshift"),
    (KeyCode::ControlLeft, "ctrl"),
    (KeyCode::ControlRight, "rctrl"),
    (KeyCode::AltLeft, "alt"),
    (KeyCode::AltRight, "ralt"),
    (KeyCode::ArrowUp, "uparrow"),
    (KeyCode::ArrowDown, "downarrow"),
    (KeyCode::ArrowLeft, "leftarrow"),
    (KeyCode::ArrowRight, "rightarrow"),
    (KeyCode::Insert, "ins"),
    (KeyCode::Delete, "del"),
    (KeyCode::Home, "home"),
    (KeyCode::End, "end"),
    (KeyCode::PageUp, "pgup"),
    (KeyCode::PageDown, "pgdn"),
    (KeyCode::Pause, "pause"),
    (KeyCode::F1, "f1"),
    (KeyCode::F2, "f2"),
    (KeyCode::F3, "f3"),
    (KeyCode::F4, "f4"),
    (KeyCode::F5, "f5"),
    (KeyCode::F6, "f6"),
    (KeyCode::F7, "f7"),
    (KeyCode::F8, "f8"),
    (KeyCode::F9, "f9"),
    (KeyCode::F10, "f10"),
    (KeyCode::F11, "f11"),
    (KeyCode::F12, "f12"),
    (KeyCode::Numpad0, "kp_ins"),
    (KeyCode::Numpad1, "kp_end"),
    (KeyCode::Numpad2, "kp_downarrow"),
    (KeyCode::Numpad3, "kp_pgdn"),
    (KeyCode::Numpad4, "kp_leftarrow"),
    (KeyCode::Numpad5, "kp_5"),
    (KeyCode::Numpad6, "kp_rightarrow"),
    (KeyCode::Numpad7, "kp_home"),
    (KeyCode::Numpad8, "kp_uparrow"),
    (KeyCode::Numpad9, "kp_pgup"),
    (KeyCode::NumpadDecimal, "kp_del"),
    (KeyCode::NumpadDivide, "kp_slash"),
    (KeyCode::NumpadMultiply, "kp_multiply"),
    (KeyCode::NumpadSubtract, "kp_minus"),
    (KeyCode::NumpadAdd, "kp_plus"),
    (KeyCode::NumpadEnter, "kp_enter"),
    (KeyCode::Minus, "-"),
    (KeyCode::Equal, "="),
    (KeyCode::BracketLeft, "["),
    (KeyCode::BracketRight, "]"),
    (KeyCode::Backslash, "\\"),
    (KeyCode::Semicolon, "semicolon"),
    (KeyCode::Quote, "'"),
    (KeyCode::Comma, ","),
    (KeyCode::Period, "."),
    (KeyCode::Slash, "/"),
];

/// Source names for mouse buttons.
pub const MOUSE_NAMES: &[(MouseButton, &str)] = &[
    (MouseButton::Left, "mouse1"),
    (MouseButton::Right, "mouse2"),
    (MouseButton::Middle, "mouse3"),
    (MouseButton::Back, "mouse4"),
    (MouseButton::Forward, "mouse5"),
];

/// The wheel's two "keys".
pub const WHEEL_UP: &str = "mwheelup";
pub const WHEEL_DOWN: &str = "mwheeldown";

/// Chimera's own binds, each on a key the game's defaults leave free (and
/// only while it is free): Alt looks around, V flies, F9 files a bug report
/// (as F4 does), and the debug keys: F3 collision shapes, F8 the debug UI,
/// F11 the entity inspector.
pub const CHIMERA_BINDS: &[(&str, &str)] = &[
    ("alt", "+freelook"),
    ("v", "noclip"),
    ("f3", "toggle chimera_drawcollision"),
    ("f8", "debugui"),
    ("f9", "bug"),
    ("f11", "chimera_inspector"),
];

/// One game's default binds and its old defaults.
#[derive(Clone, Debug, Default)]
struct GameBinds {
    binds: Vec<(String, String)>,
    old: &'static [(&'static str, &'static str)],
}

/// The default binds of the games an app read them for (`read_install`,
/// `use_game`), keyed by game id, and the game whose are used (the
/// gameplay's `binds` slot, set by the shell at start). Empty before (and
/// without a game): Chimera's own binds only. Each app has its own.
#[derive(Resource, Clone, Debug, Default)]
pub struct BindDefaults {
    games: BTreeMap<&'static str, GameBinds>,
    active: Option<&'static str>,
}

/// Whether a key name is one we know (keyboard, mouse or wheel).
pub fn known_key(name: &str) -> bool {
    KEY_NAMES.iter().any(|(_, n)| *n == name)
        || MOUSE_NAMES.iter().any(|(_, n)| *n == name)
        || [WHEEL_UP, WHEEL_DOWN].contains(&name)
}

/// The binds a config's text leaves (`bind`, `unbind`, `unbindall`), in
/// the order first bound; keys we don't know (Esc, `) are left out.
pub fn cfg_binds(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        for words in crate::console::parse(line) {
            let Some(cmd) = words.first() else { continue };
            match (cmd.to_lowercase().as_str(), words.get(1)) {
                ("unbindall", _) => out.clear(),
                ("unbind", Some(key)) => {
                    let key = key.to_lowercase();
                    out.retain(|(k, _)| *k != key);
                }
                ("bind", Some(key)) if words.len() > 2 => {
                    let key = key.to_lowercase();
                    if !known_key(&key) {
                        continue;
                    }
                    let line = words[2..].join(" ");
                    match out.iter_mut().find(|(k, _)| *k == key) {
                        Some(b) => b.1 = line,
                        None => out.push((key, line)),
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// The defaults from the game's: those, then Chimera's own on the keys
/// they leave free (`CHIMERA_BINDS`).
pub fn defaults_from(game: &[(String, String)]) -> Vec<(String, String)> {
    let mut out = game.to_vec();
    for (key, command) in CHIMERA_BINDS {
        if !out.iter().any(|(k, _)| k == key) {
            out.push((key.to_string(), command.to_string()));
        }
    }
    out
}

impl BindDefaults {
    /// Read `game`'s defaults from its install's file
    /// (`GameInfo::default_binds_cfg`: CS:S's `cfg/config_default.cfg`, in
    /// the configured, saved or found install) and make them the ones in
    /// use. Without the file they are the game's built-in table
    /// (`GameInfo::default_binds`); without a game, none (Chimera's own
    /// only). Returns whether the file was read.
    pub fn read_install(&mut self, game: Option<&GameInfo>) -> bool {
        let Some(game) = game else {
            self.active = None;
            return false;
        };
        let binds = game
            .default_binds_text()
            .map(|t| cfg_binds(&t))
            .filter(|b| !b.is_empty());
        let found = binds.is_some();
        match binds {
            Some(b) => self.set(game.id, b, game.old_default_binds),
            None => self.use_game(game),
        }
        found
    }

    /// Make `game`'s built-in table (`GameInfo::default_binds`) the
    /// defaults in use.
    pub fn use_game(&mut self, game: &GameInfo) {
        let owned = game.default_binds.iter().map(|(k, c)| (k.to_string(), c.to_string())).collect();
        self.set(game.id, owned, game.old_default_binds);
    }

    fn set(&mut self, id: &'static str, binds: Vec<(String, String)>, old: &'static [(&'static str, &'static str)]) {
        self.games.insert(id, GameBinds { binds, old });
        self.active = Some(id);
    }

    fn active(&self) -> Option<&GameBinds> {
        self.active.and_then(|id| self.games.get(id))
    }

    /// The binds a fresh config starts with (and "Use Defaults" restores):
    /// the game's in use (from its install's file when it was read, else
    /// its built-in table), plus Chimera's own (`CHIMERA_BINDS`).
    pub fn default_binds(&self) -> Vec<(String, String)> {
        defaults_from(self.active().map_or(&[], |g| &g.binds))
    }

    /// Defaults a config written before the game's current ones lacks
    /// (those not among its `GameInfo::old_default_binds`): `binddefaults
    /// new` (run after such a config) puts each on its key when the key
    /// is free and no key runs the command.
    pub fn added_defaults(&self) -> Vec<(String, String)> {
        let old = self.active().map_or(&[][..], |g| g.old);
        self.default_binds()
            .into_iter()
            .filter(|(k, c)| !old.contains(&(k.as_str(), c.as_str())))
            .collect()
    }
}

/// Put the added defaults on free keys (see `BindDefaults::added_defaults`).
pub fn bind_added_defaults(binds: &mut BTreeMap<String, String>, added: &[(String, String)]) {
    for (key, command) in added.iter().cloned() {
        if !binds.contains_key(&key) && keys_for(binds, &command).is_empty() {
            binds.insert(key, command);
        }
    }
}

/// Chimera's own polled commands: read from the held keys by the systems
/// that own them instead of the console running them (see the module
/// docs). A game adds its own with `BindsAppExt::polled_bind` (CS:S:
/// `buymenu`, `chooseteam`, `radio1`..`radio3`) and its named actions with
/// `bound_action`.
pub const POLLED: &[&str] = &[
    "+forward",
    "+back",
    "+moveleft",
    "+moveright",
    "+jump",
    "+duck",
    "+speed",
    "+walk",
    "+sprint",
    "+use",
    "+reload",
    "+attack",
    "+attack2",
    "+freelook",
    "+showscores",
    "slot1",
    "slot2",
    "slot3",
    "slot4",
    "slot5",
    "lastinv",
    "drop",
    "messagemode",
    "messagemode2",
    "bug",
    "bugreport",
];

/// A bind's line normalised for comparing (trimmed, lower-case).
fn norm(line: &str) -> String {
    line.trim().to_lowercase()
}

/// The commands beyond `POLLED` that systems read from the held keys: a
/// game's own (`BindsAppExt::polled_bind`) and the `+name` of each named
/// action (`bound_action`).
#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct PolledBinds {
    extra: std::collections::BTreeSet<String>,
}

impl PolledBinds {
    /// Whether a bind's line is read by its system rather than run.
    pub fn is_polled(&self, line: &str) -> bool {
        let l = norm(line);
        POLLED.contains(&l.as_str()) || self.extra.contains(&l)
    }

    /// Every polled command, Chimera's and the game's.
    pub fn all(&self) -> impl Iterator<Item = &str> {
        POLLED.iter().copied().chain(self.extra.iter().map(String::as_str))
    }

    /// The part of a bind's line that is run when its key goes down: the
    /// commands that are not polled (those are read by the systems that
    /// own them), joined with `; `. None when nothing is left to run.
    pub fn runnable(&self, line: &str) -> Option<String> {
        let rest: Vec<String> = parts(line)
            .into_iter()
            .filter(|p| !p.is_empty() && !self.is_polled(p))
            .collect();
        (!rest.is_empty()).then(|| rest.join("; "))
    }
}

/// `line` without the commands this app doesn't know (see the module
/// docs); None when nothing is left to run. Aliases and names several
/// games share count as known.
pub fn known_here(console: &Console, line: &str) -> Option<String> {
    let rest: Vec<String> = parts(line)
        .into_iter()
        .filter(|p| {
            crate::console::parse(p).first().is_some_and(|words| {
                let name = words[0].to_lowercase();
                console.aliases.contains_key(&name) || !matches!(console.resolve(&name), crate::console::Resolved::Unknown)
            })
        })
        .collect();
    (!rest.is_empty()).then(|| rest.join("; "))
}

/// A game plugin telling the client what its binds mean.
pub trait BindsAppExt {
    /// `command` is read from the held keys by the game's own system
    /// (`pressed`, `just_pressed`), so a bind to it is not run.
    fn polled_bind(&mut self, command: &str) -> &mut Self;
    /// Register the named action `name` (`core::ActionRegistry`): the
    /// keys bound to `+name` set its bit in the local `Intent::actions`
    /// while held, and `+name` is polled.
    fn bound_action(&mut self, name: &str) -> &mut Self;
}

impl BindsAppExt for App {
    fn polled_bind(&mut self, command: &str) -> &mut Self {
        self.world_mut()
            .get_resource_or_insert_with(PolledBinds::default)
            .extra
            .insert(norm(command));
        // Read by its short name, so binds store it so
        // (`Console::keep_short`).
        self.world_mut().get_resource_or_init::<Console>().keep_short(command);
        self
    }

    fn bound_action(&mut self, name: &str) -> &mut Self {
        use crate::core::ActionAppExt;
        self.intent_action(name);
        self.polled_bind(&format!("+{}", norm(name)))
    }
}

/// A bind's commands: its line split at `;` outside quotes (one key may
/// hold several: a game puts two actions on one key), trimmed.
pub fn parts(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let (mut quoted, mut start) = (false, 0);
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ';' if !quoted => {
                out.push(line[start..i].trim().to_string());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(line[start..].trim().to_string());
    out
}

/// The keys bound to exactly `command`, in key order.
/// (Polled by input systems several times a frame: compares without
/// allocating, as `norm` would: trimmed, case ignored.)
pub fn keys_for<'a>(binds: &'a BTreeMap<String, String>, command: &str) -> Vec<&'a str> {
    binds
        .iter()
        .filter(|(_, v)| runs(v, command))
        .map(|(k, _)| k.as_str())
        .collect()
}

/// Whether a bind's line is `command` or holds it among its `;` parts.
fn runs(line: &str, command: &str) -> bool {
    same_command(line, command) || (line.contains(';') && parts(line).iter().any(|p| same_command(p, command)))
}

/// Whether a key or button in `down` (held, or gone down this frame) is
/// bound to `command`: the keys down looked up by name (usually none or
/// a few) instead of every bind compared with the command, as
/// `keys_for` would (input polls this several times a frame; the names
/// are unique, `key_names_are_unique`).
fn bound_down(
    binds: &BTreeMap<String, String>,
    keys: impl Iterator<Item = KeyCode>,
    buttons: impl Iterator<Item = MouseButton>,
    command: &str,
) -> bool {
    let bound = |name: &str| binds.get(name).is_some_and(|line| runs(line, command));
    keys.flat_map(|k| KEY_NAMES.iter().filter(move |(c, _)| *c == k))
        .any(|(_, n)| bound(n))
        || buttons
            .flat_map(|b| MOUSE_NAMES.iter().filter(move |(c, _)| *c == b))
            .any(|(_, n)| bound(n))
}

/// `norm(a) == norm(b)` without the two new strings.
fn same_command(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim(), b.trim());
    if a.is_ascii() && b.is_ascii() {
        return a.eq_ignore_ascii_case(b);
    }
    a.chars().flat_map(char::to_lowercase).eq(b.chars().flat_map(char::to_lowercase))
}

/// A key name as the options list shows it (`SPACE`, `MOUSE1`).
pub fn display(key: &str) -> String {
    key.to_uppercase()
}

/// Whether a key (by Source name) is held.
pub fn key_held(name: &str, keys: &ButtonInput<KeyCode>, mouse: &ButtonInput<MouseButton>) -> bool {
    KEY_NAMES
        .iter()
        .find(|(_, n)| *n == name)
        .is_some_and(|(k, _)| keys.pressed(*k))
        || MOUSE_NAMES
            .iter()
            .find(|(_, n)| *n == name)
            .is_some_and(|(b, _)| mouse.pressed(*b))
}

/// Whether a key bound to `command` is held.
pub fn pressed(
    binds: &BTreeMap<String, String>,
    keys: &ButtonInput<KeyCode>,
    mouse: &ButtonInput<MouseButton>,
    command: &str,
) -> bool {
    bound_down(binds, keys.get_pressed().copied(), mouse.get_pressed().copied(), command)
}

/// Whether a key bound to `command` went down this frame.
pub fn just_pressed(
    binds: &BTreeMap<String, String>,
    keys: &ButtonInput<KeyCode>,
    mouse: &ButtonInput<MouseButton>,
    command: &str,
) -> bool {
    bound_down(binds, keys.get_just_pressed().copied(), mouse.get_just_pressed().copied(), command)
}

/// Keys a menu has taken: the game's polled binds (`pressed_free`,
/// `just_pressed_free`: slot keys, attack ...) don't see them until they
/// are let go, so the number that picks a radio command never also takes
/// out a weapon, in the frame it picks or after the menu has closed with
/// the key still down. A menu calls `capture` for the keys it reads (in
/// `PreUpdate`, after `InputSystems`); `release_captured` forgets a key
/// once it is up.
#[derive(Resource, Default, Clone, Debug, PartialEq, Eq)]
pub struct CapturedKeys(std::collections::HashSet<KeyCode>);

impl CapturedKeys {
    pub fn capture(&mut self, key: KeyCode) {
        self.0.insert(key);
    }

    pub fn holds(&self, key: KeyCode) -> bool {
        self.0.contains(&key)
    }

    /// Forget the keys that are up.
    pub fn release_up(&mut self, keys: &ButtonInput<KeyCode>) {
        self.0.retain(|k| keys.pressed(*k));
    }
}

/// Forget captured keys that are let go.
pub fn release_captured(keys: Res<ButtonInput<KeyCode>>, mut captured: ResMut<CapturedKeys>) {
    if !captured.0.is_empty() {
        captured.release_up(&keys);
    }
}

/// `pressed`, not counting the keys a menu has taken.
pub fn pressed_free(
    binds: &BTreeMap<String, String>,
    keys: &ButtonInput<KeyCode>,
    mouse: &ButtonInput<MouseButton>,
    captured: Option<&CapturedKeys>,
    command: &str,
) -> bool {
    let free = |k: &KeyCode| !captured.is_some_and(|c| c.holds(*k));
    bound_down(binds, keys.get_pressed().copied().filter(free), mouse.get_pressed().copied(), command)
}

/// `just_pressed`, not counting the keys a menu has taken.
pub fn just_pressed_free(
    binds: &BTreeMap<String, String>,
    keys: &ButtonInput<KeyCode>,
    mouse: &ButtonInput<MouseButton>,
    captured: Option<&CapturedKeys>,
    command: &str,
) -> bool {
    let free = |k: &KeyCode| !captured.is_some_and(|c| c.holds(*k));
    bound_down(binds, keys.get_just_pressed().copied().filter(free), mouse.get_just_pressed().copied(), command)
}

/// Wheel notches this frame on wheel directions bound to `command`.
pub fn wheel_notches(binds: &BTreeMap<String, String>, scroll: &AccumulatedMouseScroll, command: &str) -> u32 {
    /// Pixel-unit scrolling (touchpads, smooth wheels): pixels per notch.
    const PIXELS_PER_NOTCH: f32 = 40.0;
    let y = scroll.delta.y;
    if y == 0.0 {
        return 0;
    }
    let key = if y > 0.0 { WHEEL_UP } else { WHEEL_DOWN };
    if !keys_for(binds, command).contains(&key) {
        return 0;
    }
    (match scroll.unit {
        MouseScrollUnit::Line => y.abs().round(),
        MouseScrollUnit::Pixel => (y.abs() / PIXELS_PER_NOTCH).ceil(),
    }) as u32
}

/// The first key, button or wheel notch that went down this frame, by
/// Source name (for rebinding). Esc and ` are not offered.
pub fn first_pressed(
    keys: &ButtonInput<KeyCode>,
    mouse: &ButtonInput<MouseButton>,
    scroll: Option<&AccumulatedMouseScroll>,
) -> Option<&'static str> {
    KEY_NAMES
        .iter()
        .find(|(k, _)| keys.just_pressed(*k))
        .map(|(_, n)| *n)
        .or_else(|| MOUSE_NAMES.iter().find(|(b, _)| mouse.just_pressed(*b)).map(|(_, n)| *n))
        .or_else(|| {
            let y = scroll?.delta.y;
            (y != 0.0).then_some(if y > 0.0 { WHEEL_UP } else { WHEEL_DOWN })
        })
}

/// Put the default binds in: every one (`all`, after clearing the rest),
/// or only those whose key is free and whose command no key runs (for a
/// config written before its keys were binds).
pub fn bind_defaults(binds: &mut BTreeMap<String, String>, all: bool, defaults: &[(String, String)]) {
    if all {
        binds.clear();
    }
    for (key, command) in defaults.iter().cloned() {
        if all || (!binds.contains_key(&key) && keys_for(binds, &command).is_empty()) {
            binds.insert(key, command);
        }
    }
}

/// The line that sets `command`'s key to `key` alone: its other keys
/// unbound, `key` bound to it (whatever it ran before).
pub fn rebind_line(binds: &BTreeMap<String, String>, command: &str, key: &str) -> String {
    let mut parts: Vec<String> = keys_for(binds, command)
        .into_iter()
        .filter(|k| *k != key)
        .map(|k| format!("unbind {}", crate::console::quote(k)))
        .collect();
    parts.push(format!(
        "bind {} {}",
        crate::console::quote(key),
        crate::console::quote(command)
    ));
    parts.join("; ")
}

/// The line that clears every key of `command` (None: it has none).
pub fn clear_line(binds: &BTreeMap<String, String>, command: &str) -> Option<String> {
    let keys = keys_for(binds, command);
    (!keys.is_empty()).then(|| {
        keys.iter()
            .map(|k| format!("unbind {}", crate::console::quote(k)))
            .collect::<Vec<_>>()
            .join("; ")
    })
}

/// A config.cfg written since every key became a bind says so; one without
/// the mark gets the default binds its `unbindall` dropped back.
pub const CONFIG_MARK: &str = "// binds: all";

/// A config.cfg written over the game's current defaults says so; one with
/// `CONFIG_MARK` but not this gets the defaults added since
/// (`binddefaults new`) once, on free keys. Its text names the defaults'
/// change it marks (CS:S's `config_default.cfg`, 2026-10-13), and stays so
/// saved configs keep reading.
pub const DEFAULTS_MARK: &str = "// bind defaults: cs";

pub fn commands(app: &mut App) {
    // The polled binds are read by their short names: binds store them so
    // (`Console::keep_short`).
    {
        let mut console = app.world_mut().get_resource_or_init::<Console>();
        for word in POLLED {
            console.keep_short(word);
        }
    }
    app.init_resource::<BindDefaults>().console_command(
        "chimera:binddefaults",
        "binddefaults [missing|new]: put the default binds back (missing: only on free keys for unbound commands; \
         new: only the defaults added lately, likewise).",
        |w, a| {
            let mode = a.first().map(String::as_str);
            let defaults = w.get_resource::<BindDefaults>().cloned().unwrap_or_default();
            let mut c = w.resource_mut::<Console>();
            let before = c.binds.clone();
            match mode {
                Some("new") => {
                    bind_added_defaults(&mut c.binds, &defaults.added_defaults());
                    // Written again with `DEFAULTS_MARK`, so it runs once.
                    c.dirty = true;
                }
                Some("missing") => bind_defaults(&mut c.binds, false, &defaults.default_binds()),
                _ => bind_defaults(&mut c.binds, true, &defaults.default_binds()),
            }
            if c.binds != before {
                c.dirty = true;
            }
            Ok(None)
        },
    )
    .console_command("bug", "bug [note]: file a bug report (CS:S's name for `bugreport`).", |w, a| {
        let note: Vec<String> = a.iter().map(|x| crate::console::quote(x)).collect();
        w.resource_mut::<Console>()
            .submit(format!("bugreport {}", note.join(" ")).trim_end().to_string());
        Ok(None)
    });
}

/// For tests of systems that read binds: the mouse's buttons and `game`'s
/// built-in default binds in an app with the console (here and in a
/// game front end's tests).
pub fn test_binds(app: &mut App, game: &GameInfo) {
    app.init_resource::<ButtonInput<MouseButton>>();
    let mut defaults = BindDefaults::default();
    defaults.use_game(game);
    bind_defaults(&mut app.world_mut().resource_mut::<Console>().binds, true, &defaults.default_binds());
    app.insert_resource(defaults);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binds(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    fn owned(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn a_key_can_hold_several_commands() {
        let b = binds(&[("q", "+reload; lastinv"), ("w", "+forward"), ("e", "say \"a; b\"; +use")]);
        assert_eq!(keys_for(&b, "lastinv"), ["q"]);
        assert_eq!(keys_for(&b, "+reload"), ["q"]);
        assert_eq!(keys_for(&b, "+forward"), ["w"]);
        assert_eq!(keys_for(&b, "+use"), ["e"]);
        // Polled parts are read by their systems; the rest runs.
        let p = PolledBinds::default();
        assert_eq!(p.runnable("+reload; lastinv"), None);
        assert_eq!(p.runnable("+reload; choose_1"), Some("choose_1".into()));
        assert_eq!(p.runnable("say \"a; b\"; +use"), Some("say \"a; b\"".into()));
        assert_eq!(p.runnable("+forward"), None);
        // A game's own polled commands are polled once it says so.
        assert_eq!(p.runnable("buymenu"), Some("buymenu".into()));
        let mut app = App::new();
        app.polled_bind("BuyMenu").bound_action("Throw");
        let p = app.world().resource::<PolledBinds>();
        assert_eq!(p.runnable("buymenu; +throw"), None);
        assert!(p.all().any(|c| c == "+throw") && p.all().any(|c| c == "+forward"));
        let reg = app.world().resource::<crate::core::ActionRegistry>();
        assert_eq!(reg.bit("throw"), 1);
        assert_eq!(reg.bit("nope"), 0);
    }

    /// A saved config shared with another app: the key still selects the
    /// weapon (`slot1` is polled) and the other app's `choose_1` isn't run
    /// or complained about; the line stays for that app.
    #[test]
    fn a_bind_naming_another_apps_command_is_left_out() {
        let mut app = App::new();
        app.add_plugins(crate::console::ConsolePlugin);
        let shared = "slot1; choose_1";
        let binds = binds(&[("1", shared), ("3", "combat_arms:choose_3")]);
        assert_eq!(keys_for(&binds, "slot1"), ["1"], "the polled read still finds the key");
        let polled = PolledBinds::default();
        let left = polled.runnable(shared).unwrap();
        assert_eq!(left, "choose_1");
        let console = app.world().resource::<Console>();
        assert_eq!(known_here(console, &left), None);
        assert_eq!(known_here(console, "combat_arms:choose_3"), None);
        assert_eq!(known_here(console, "echo hi; choose_1").as_deref(), Some("echo hi"), "what this app has still runs");
        let mut console = app.world_mut().resource_mut::<Console>();
        console.aliases.insert("mine".into(), "echo hi".into());
        assert_eq!(known_here(&console, "mine").as_deref(), Some("mine"));
    }

    /// A game's default binds file as it might read (CS:S's
    /// `config_default.cfg` has this shape; its own test checks the real
    /// table, games/cs_source/src/ui/mod.rs).
    const SAMPLE_DEFAULTS_CFG: &str = r#"// first run
unbindall

bind "ESCAPE" "cancelselect"
bind "`" "toggleconsole"

bind "TAB" "+showscores"
bind "SPACE" "+jump"
bind "a" "+moveleft"
bind "d" "+moveright"
bind "s" "+back"
bind "w" "+forward"
bind "F1" "autobuy"
bind "F2" "rebuy"
bind "MWHEELDOWN" "invnext"
bind "MWHEELUP" "invprev"
bind "MOUSE1" "+attack"
"#;

    /// The sample game's defaults before its current ones.
    const SAMPLE_OLD: &[(&str, &str)] = &[
        ("w", "+forward"),
        ("s", "+back"),
        ("a", "+moveleft"),
        ("d", "+moveright"),
        ("space", "+jump"),
        ("tab", "+showscores"),
        ("alt", "+freelook"),
        ("v", "noclip"),
        ("f9", "bug"),
    ];

    /// The sample file's binds as a game's defaults.
    fn sample_resource() -> BindDefaults {
        let mut d = BindDefaults::default();
        d.set("sample", cfg_binds(SAMPLE_DEFAULTS_CFG), SAMPLE_OLD);
        d
    }

    fn sample_defaults() -> Vec<(String, String)> {
        cfg_binds(SAMPLE_DEFAULTS_CFG)
    }

    #[test]
    fn defaults_are_config_default_cfgs_binds() {
        // The file's binds, in its order (Esc and ` aren't binds here).
        let read = sample_defaults();
        assert_eq!(read.first(), Some(&("tab".into(), "+showscores".into())));
        assert!(!read.iter().any(|(k, _)| k == "escape" || k == "`"));
        // Chimera's own go only on keys the game leaves free.
        let defaults = defaults_from(&read);
        assert_eq!(&defaults[..read.len()], &read[..]);
        for (k, c) in CHIMERA_BINDS {
            assert!(!read.iter().any(|(rk, _)| rk == k), "{k} is the game's");
            assert!(defaults.contains(&(k.to_string(), c.to_string())));
        }
        // A file binding one of Chimera's keys keeps it.
        let mut changed = read.clone();
        changed.push(("f8".into(), "say eight".into()));
        let d = defaults_from(&changed);
        assert_eq!(d.iter().filter(|(k, _)| k == "f8").count(), 1);
        assert!(d.contains(&("f8".into(), "say eight".into())));
        // Later lines win; unbind and unbindall drop keys; unknown keys
        // are left out.
        let b = cfg_binds("bind w +back\nbind W \"+forward\"; bind nope x\nbind q lastinv\nunbind Q\n");
        assert_eq!(b, owned(&[("w", "+forward")]));
        assert!(cfg_binds("bind a +jump\nunbindall").is_empty());
    }

    #[test]
    fn defaults_have_known_keys_and_one_line_per_key() {
        let game = sample_defaults();
        let d = defaults_from(&game);
        for (k, _) in &d {
            assert!(known_key(k), "{k} is not a key name");
        }
        let mut keys: Vec<&str> = d.iter().map(|(k, _)| k.as_str()).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), d.len());
        // The debug keys stay off the game's: F1, F2 and the wheel are its.
        assert!(d.contains(&("f1".into(), "autobuy".into())));
        assert!(d.contains(&("f2".into(), "rebuy".into())));
        assert!(d.contains(&("mwheelup".into(), "invprev".into())));
        assert!(d.contains(&("mwheeldown".into(), "invnext".into())));
        for c in ["debugui", "chimera_inspector", "toggle chimera_drawcollision"] {
            let k = &d.iter().find(|(_, v)| v == c).unwrap().0;
            assert!(!game.iter().any(|(ck, _)| ck == k), "{c} on the game's {k}");
        }
    }

    #[test]
    fn added_defaults_reach_old_configs_on_free_keys() {
        // A config listing every bind, from before the game's defaults
        // changed.
        let d = sample_resource();
        let mut b = binds(&[("w", "+forward"), ("f2", "debugui"), ("mwheelup", "+jump")]);
        bind_added_defaults(&mut b, &d.added_defaults());
        assert_eq!(b.get("f1").map(String::as_str), Some("autobuy"));
        assert_eq!(b.get("f11").map(String::as_str), Some("chimera_inspector"));
        // The key taken, or the command on another key: left alone.
        assert_eq!(b["f2"], "debugui");
        assert!(keys_for(&b, "rebuy").is_empty());
        assert_eq!(b["mwheelup"], "+jump");
        assert_eq!(b.get("f8"), None, "debugui already has a key");
        let added = d.added_defaults();
        assert!(!added.iter().any(|(k, _)| k == "w"), "old defaults aren't added");
        for a in &added {
            assert!(d.default_binds().contains(a), "{a:?} is a default too");
        }
    }

    #[test]
    fn rebinding_moves_the_action_to_the_new_key() {
        let b = binds(&[("w", "+forward"), ("uparrow", "+forward"), ("f", "+use")]);
        assert_eq!(keys_for(&b, "+FORWARD "), ["uparrow", "w"]);
        for (a, b) in [("+Forward", " +forward"), ("ÄRGER", "ärger"), ("say Ü", "say ü"), ("a", "b"), ("+use", "+used")] {
            assert_eq!(same_command(a, b), norm(a) == norm(b), "{a} vs {b}");
        }
        assert_eq!(
            rebind_line(&b, "+forward", "f"),
            "unbind uparrow; unbind w; bind f +forward"
        );
        assert_eq!(rebind_line(&b, "+forward", "w"), "unbind uparrow; bind w +forward");
        assert_eq!(clear_line(&b, "+use").as_deref(), Some("unbind f"));
        assert_eq!(clear_line(&b, "+jump"), None);
        // Keys that need quoting stay one word.
        assert_eq!(rebind_line(&b, "say hi", ";"), "bind \";\" \"say hi\"");
    }

    #[test]
    fn missing_defaults_fill_only_free_keys_for_unbound_commands() {
        // An old config: only the player's own binds survived unbindall.
        let d = sample_resource();
        let mut b = binds(&[("f", "+jump"), ("w", "say hello")]);
        bind_defaults(&mut b, false, &d.default_binds());
        assert_eq!(b.get("space"), None, "+jump already has a key");
        assert_eq!(b["w"], "say hello", "a taken key is kept");
        assert!(keys_for(&b, "+forward").is_empty(), "w was taken, so +forward stays unbound");
        assert_eq!(b["a"], "+moveleft");
        bind_defaults(&mut b, true, &d.default_binds());
        assert_eq!(b.len(), d.default_binds().len());
        assert_eq!(b["w"], "+forward");
    }

    #[test]
    fn binds_save_to_config_and_load_back() {
        let mut app = App::new();
        app.add_plugins(crate::console::ConsolePlugin);
        commands(&mut app);
        app.insert_resource(sample_resource());
        app.world_mut()
            .resource_mut::<Console>()
            .submit("binddefaults; unbind space; bind k +jump; bind [ \"say hi\"");
        app.update();
        let saved = app.world().resource::<Console>().binds.clone();
        assert_eq!(saved.get("k").map(String::as_str), Some("+jump"));
        let text = crate::console::config_text(app.world_mut());
        assert!(text.contains(CONFIG_MARK) && text.contains(DEFAULTS_MARK), "{text}");
        assert!(text.contains("bind k +jump\n"), "{text}");

        // A fresh start: the defaults, then the config.
        let mut fresh = App::new();
        fresh.add_plugins(crate::console::ConsolePlugin);
        commands(&mut fresh);
        fresh.insert_resource(sample_resource());
        let mut c = fresh.world_mut().resource_mut::<Console>();
        c.submit("binddefaults");
        for line in text.lines() {
            c.submit(line);
        }
        fresh.update();
        assert_eq!(fresh.world().resource::<Console>().binds, saved);
    }

    #[test]
    fn held_and_pressed_keys_by_command() {
        let b = binds(&[("f", "+duck"), ("mouse4", "+duck"), ("b", "buymenu")]);
        let mut keys = ButtonInput::<KeyCode>::default();
        let mut mouse = ButtonInput::<MouseButton>::default();
        assert!(!pressed(&b, &keys, &mouse, "+duck"));
        mouse.press(MouseButton::Back);
        assert!(pressed(&b, &keys, &mouse, "+duck"));
        keys.press(KeyCode::KeyB);
        assert!(just_pressed(&b, &keys, &mouse, "buymenu"));
        keys.clear();
        mouse.clear();
        assert!(!just_pressed(&b, &keys, &mouse, "buymenu"));
        assert_eq!(first_pressed(&keys, &mouse, None), None);
        keys.press(KeyCode::Numpad5);
        assert_eq!(first_pressed(&keys, &mouse, None), Some("kp_5"));
    }

    #[test]
    fn key_names_are_unique() {
        // `pressed` looks a held key's bind up by its name; `key_held`
        // finds a name's first key: the same only with one key per name.
        let names: Vec<&str> = KEY_NAMES.iter().map(|(_, n)| *n).chain(MOUSE_NAMES.iter().map(|(_, n)| *n)).collect();
        let unique: std::collections::BTreeSet<&str> = names.iter().copied().collect();
        assert_eq!(unique.len(), names.len());
    }

    #[test]
    fn pressed_is_any_bound_key_held() {
        // As before: the keys bound to the command (`keys_for`), any held.
        let b = binds(&[
            ("w", "+forward"),
            ("uparrow", " +FORWARD"),
            ("q", "+reload; lastinv"),
            ("mouse1", "+attack"),
            ("e", "say \"+use; x\""),
            ("mwheelup", "invprev"),
        ]);
        let commands = ["+forward", "+reload", "lastinv", "+attack", "+use", "invprev", "+jump"];
        let old = |keys: &ButtonInput<KeyCode>, mouse: &ButtonInput<MouseButton>, c: &str| {
            keys_for(&b, c).into_iter().any(|k| key_held(k, keys, mouse))
        };
        let (mut keys, mut mouse) = (ButtonInput::<KeyCode>::default(), ButtonInput::<MouseButton>::default());
        let check = |keys: &ButtonInput<KeyCode>, mouse: &ButtonInput<MouseButton>| {
            for c in commands {
                assert_eq!(pressed(&b, keys, mouse, c), old(keys, mouse, c), "{c}");
            }
        };
        check(&keys, &mouse);
        keys.press(KeyCode::ArrowUp);
        assert!(pressed(&b, &keys, &mouse, "+forward"));
        assert!(just_pressed(&b, &keys, &mouse, "+forward"));
        check(&keys, &mouse);
        keys.clear();
        assert!(pressed(&b, &keys, &mouse, "+forward"), "still held");
        assert!(!just_pressed(&b, &keys, &mouse, "+forward"), "not new");
        keys.press(KeyCode::KeyQ);
        keys.press(KeyCode::KeyE);
        mouse.press(MouseButton::Left);
        check(&keys, &mouse);
        assert!(pressed(&b, &keys, &mouse, "lastinv") && pressed(&b, &keys, &mouse, "+attack"));
        assert!(!pressed(&b, &keys, &mouse, "+use"), "quoted: one command");
        keys.release_all();
        mouse.release_all();
        check(&keys, &mouse);
    }
}
