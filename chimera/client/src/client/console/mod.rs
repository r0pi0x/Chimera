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
//! picking one (Enter or Tab takes it and adds a space, Esc hides the list;
//! the box is a child of the console's root, so it draws on whichever UI
//! camera the console does: a party menu's own), a
//! help line for the command typed (its usage from the help text with the
//! argument being typed marked, a cvar's value, default, range and values),
//! history (Up/Down with an empty line, persisted, Ctrl+R), editing keys
//! (Ctrl+Backspace/W/Delete words, Ctrl+U/K, Ctrl+V or Shift+Insert paste, Ctrl+L clear,
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
//!
//! At start: config.cfg, then autoexec.cfg, then the command line; a
//! config written before the party apps' options fix (no `OPTIONS_MARK`)
//! gets `chimera_repair_options` once, which puts back the volumes and
//! brightness that bug saved (`repair_options`).

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
use super::{FirstPersonCamera, fonts::UiFonts, widgets::Interaction};
use crate::{
    console::{Console, ConsoleAppExt, Level, cfg_dir, parse, resource_cvar},
    core::{LocalPlayer, MovementState, Velocity},
    game_registry::{GameInfo, Games},
    map::PlaySound,
};


mod binds;
mod commands;
mod overlays;
#[cfg(test)]
mod tests;
mod ui;

pub use binds::*;
pub use commands::*;
pub use overlays::*;
pub use ui::*;

pub struct ConsoleUiPlugin;

impl Plugin for ConsoleUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ConsoleUi>()
            .init_resource::<MapLists>()
            .init_resource::<Overlays>()
            .init_resource::<HeldActions>()
            .init_resource::<SoundMarks>()
            .add_systems(Startup, (spawn_ui, load_history, autoexec).chain())
            .add_systems(PreUpdate, poll_showscores.after(bevy::input::InputSystems))
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
                         chat: Option<Res<super::chat::ChatInput>>,
                         report: Option<Res<super::feedback::Feedback>>| {
                            !ui.open
                                && !menu.is_some_and(|m| m.open)
                                && !chat.is_some_and(|c| c.open.is_some())
                                && !report.is_some_and(|r| r.shown())
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
