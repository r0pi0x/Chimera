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
    /// The scoreboard (`+showscores`, Tab in CS:S) held from the console.
    pub showscores: bool,
    /// A key bound to `+showscores` is held (`poll_showscores`: the bind
    /// is polled, so its key never runs `+showscores` through the console).
    pub showscores_key: bool,
    /// Free look (`+freelook`, Left Alt): the mouse turns the camera only.
    pub freelook: bool,
    /// The game's named actions held from the console (`+firemode`,
    /// `BindsAppExt::bound_action`).
    pub named: std::collections::BTreeSet<String>,
}

impl HeldActions {
    /// The scoreboard is asked for: its key held or `+showscores` run.
    pub fn scores(&self) -> bool {
        self.showscores || self.showscores_key
    }
}

/// Keeps `HeldActions::showscores_key` from the keys bound to
/// `+showscores` (a polled bind: `binds::POLLED`), for the shell's boards
/// (the race chip's Tab board, the minigame scoreboard), which read
/// `HeldActions::scores`. Not while the console, a menu or the chat line
/// takes the keys, nor for a key a menu has captured. (Playtest7: the
/// boards read only the console's `+showscores`, so Tab never opened
/// them.)
#[allow(clippy::too_many_arguments)]
pub fn poll_showscores(
    console: Option<Res<Console>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    captured: Option<Res<crate::client::binds::CapturedKeys>>,
    ui: Option<Res<ConsoleUi>>,
    menu: Option<Res<crate::client::front_end::MenuStatus>>,
    chat: Option<Res<crate::client::chat::ChatInput>>,
    mut held: ResMut<HeldActions>,
) {
    let busy = ui.is_some_and(|u| u.open) || menu.is_some_and(|m| m.open) || chat.is_some_and(|c| c.open.is_some());
    let down = match (console, keys, mouse) {
        (Some(c), Some(k), Some(m)) if !busy => crate::client::binds::pressed_free(&c.binds, &k, &m, captured.as_deref(), "+showscores"),
        _ => false,
    };
    if held.showscores_key != down {
        held.showscores_key = down;
    }
}

pub(super) const ACTIONS: &[&str] = &[
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
pub(super) fn run_binds(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut wheel: MessageReader<MouseWheel>,
    mut console: ResMut<Console>,
    vote: Option<Res<crate::net::vote::VoteView>>,
    lobby: Option<Res<crate::client::prematch_lobby::LobbyView>>,
    polled: Option<Res<crate::client::binds::PolledBinds>>,
) {
    let voting = vote.is_some_and(|v| v.awaiting());
    let lobby = lobby.filter(|l| l.shown);
    let polled = polled.as_deref().cloned().unwrap_or_default();
    if console.binds.is_empty() {
        wheel.clear();
        return;
    }
    let mut events: Vec<(String, bool)> = Vec::new();
    for (code, name) in crate::client::binds::KEY_NAMES {
        if keys.just_pressed(*code) {
            events.push((name.to_string(), true));
        }
        if keys.just_released(*code) {
            events.push((name.to_string(), false));
        }
    }
    for (b, name) in crate::client::binds::MOUSE_NAMES {
        if mouse.just_pressed(*b) {
            events.push((name.to_string(), true));
        }
        if mouse.just_released(*b) {
            events.push((name.to_string(), false));
        }
    }
    for w in wheel.read() {
        let name = if w.y > 0.0 {
            crate::client::binds::WHEEL_UP
        } else {
            crate::client::binds::WHEEL_DOWN
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
            && key == crate::client::prematch_lobby::LOBBY_KEY
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
        let Some(cmd) = crate::client::binds::known_here(&console, &cmd) else {
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

