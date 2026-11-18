//! VGUI's controls, one layer shared by the GameUI dialogs (`game_menu`:
//! the options, Create Server, Keyboard Advanced, the loading dialog;
//! `server_browser`: Find Servers, Add Server, the password dialog;
//! `first_run`): the state machines (a drop-down's open list, a slider's
//! drag, a text entry's caret and selection, tab focus order, movable
//! frames' places and stacking) are plain structs the dialogs' models
//! hold and unit-test; the drawing helpers draw each control in the
//! GameUI scheme's look (`Look`); the systems here move, raise and resize
//! frames, restyle buttons as the pointer arms and presses them, and
//! report a slider dragged.
//!
//! As VGUI: a `ComboBox` opens a list under itself on a click (its
//! arrow button drawn as the scheme's `ComboBoxButton`), the pointer
//! highlights an entry, a click picks it, Esc or a click outside closes
//! it, a click on its box toggles it, Up/Down/wheel step a focused closed
//! one; a `ScrollBar` (`scroll_bar`) scrolls its list a row by its arrows,
//! a page by its track, and follows the pointer by its thumb while held
//! (`ScrollDrag`), the view free of the list's selection; a `Slider` follows the
//! mouse while held, with tick marks under it; a `TextEntry` has a caret,
//! a selection (Shift with the arrows, a drag, Ctrl+A), copy, cut and
//! paste (one-line fields: `text_key` maps the keys, `Caret::key` applies
//! them with `chimera_ui::clipboard::Clipboard`); a `Frame` moves by its title bar (kept on screen), comes to the
//! front when clicked, closes from its X, and the sizeable ones (the
//! server browser) resize from their edges and corner down to their
//! minimum size. Nothing is remembered across runs: a frame is centred
//! the first time it opens in a session, then stays where it was left.
//!
//! Clicks: a press reaches the topmost node under the pointer that stops
//! it (`FocusPolicy::Block`) and every node above that one, so whatever
//! is clickable must block, or a click on it also lands on what is under
//! it (a row under a scroll bar's thumb, the box under an open list's
//! catcher). An `Interaction` (Bevy UI's pointer state; Bevy 0.20
//! deprecates that name and `Button`: see `Interaction` here) doesn't stop
//! the pointer by itself and `Node` requires `Pass`; so clickables spawn
//! with `clickable()`,
//! and `block_clicks` makes anything that gains an `Interaction` block
//! (`PassClicks` opts out, for a node that only watches the pointer).
//! Plain nodes keep Bevy's `Pass`: labels and pictures on a button, HUD
//! panels (scoreboard, chat, spectator bars, radio menu: nothing there
//! takes a click), full-screen roots whose controls alone take clicks
//! (the spectator menu, the server browser, the MOTD and class menus,
//! which the game menu must stay clickable under). Panels that swallow
//! clicks say `Block` themselves: frames, an open list's backing, the
//! game menu's backdrop, the team and buy menus' dim screen, the
//! console.
//!
//! VGUI's controls, one layer shared by the GameUI dialogs (`game_menu`,
//! `server_browser`, `first_run`): the state machines the dialogs' models hold
//! (`ComboList`: a drop-down's open list, highlight, pick, wheel, keys;
//! `Caret`: a text entry's caret and selection, editing; `focus_step`: Tab
//! order; `Windows`: movable frames' places for the session, drags kept on
//! screen, resizes down to a minimum, stacking), the drawing of each control in
//! the scheme's look (`Look`, `frame` with its title, close box and grip,
//! `button` (armed, depressed, disabled, default and focus states, restyled by
//! `style_buttons`), `check_button`, `radio_button`, `combo_box` and
//! `combo_popup`, `slider` with tick marks, `text_entry` with caret and
//! selection placed by `UiFonts::char_offsets`, `tabs`), and the systems
//! moving, raising and resizing frames, holding a scroll bar's thumb
//! (`scroll_pointer`) and running `ui_click` / `ui_drag` (`PointerScript`: the
//! pointer driven from the console through Bevy's input, for checking the menus
//! live) (`frames_pointer`, `place_frames`: each
//! frame over what it hides, `FrameBacking`: the main menu's picture, in a game
//! the world drawn again at `WORLD_UNDER_DIALOGS` brightness from
//! `WorldPicture`) and following a held slider (`SliderDrag`); `vgui_windows`
//! lists and moves frames; also the spectator menu's drop-downs
//! (`spectator_menu.rs`).
//!
//! Known shortcuts (docs/tech-debt.md): the menus read Bevy UI's
//! `Interaction` (named here by Bevy's undeprecated `DeprecatedInteraction`),
//! which Bevy 0.20 deprecates with `Button` for picking's `Hovered` and
//! `ui::Pressed`. Why it's OK for now: Bevy 0.20 still runs it; moving every
//! menu to the new press semantics is its own change (docs/backlog.md).

use std::collections::HashMap;

use bevy::{
    prelude::*,
    text::LineBreak,
    ui::{ComputedNode, UiGlobalTransform},
    window::PrimaryWindow,
};

use super::fonts::UiFonts;
use crate::console::ConsoleAppExt;
use crate::map::hud::GameUi;


mod combo;
mod look;
mod scroll;
mod slider;
#[cfg(test)]
mod tests;
mod text_edit;
mod window;

pub use combo::*;
pub use look::*;
pub use scroll::*;
pub use slider::*;
pub use text_edit::*;
pub use window::*;

pub struct WidgetsPlugin;

impl Plugin for WidgetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Windows>()
            .init_resource::<SliderDrag>()
            .init_resource::<FrameBacking>()
            .init_resource::<WorldPicture>()
            .init_resource::<ScrollDrag>()
            .init_resource::<PointerScript>()
            .add_observer(block_clicks)
            .add_systems(PreUpdate, run_pointer_script.before(bevy::input::InputSystems))
            .add_systems(
                PreUpdate,
                (frames_pointer, slider_pointer).after(bevy::input::InputSystems),
            )
            .add_systems(PreUpdate, scroll_pointer.after(bevy::ui::UiSystems::Focus))
            .add_systems(Update, style_buttons)
            .add_systems(PostUpdate, place_frames.before(bevy::ui::UiSystems::Layout));
        app.console_command(
            "chimera:vgui_windows",
            "vgui_windows [<frame> <x> <y> [<wide> <tall>]]: the menus' frames (servers, options, createserver, \
             ...) with their places and stacking, back to front; with a frame, move it there (window pixels) as \
             a drag of its title bar would, and size it (scheme pixels) when it's sizeable; screenshots and tests.",
            |w, a| {
                let mut windows = w.resource_mut::<Windows>();
                if let Some(id) = a.first() {
                    let num = |i: usize| a.get(i).and_then(|v| v.parse::<f32>().ok());
                    let (Some(x), Some(y)) = (num(1), num(2)) else {
                        return Err("vgui_windows <frame> <x> <y> [<wide> <tall>]".into());
                    };
                    let Some(key) = windows.order.iter().copied().find(|k| k.eq_ignore_ascii_case(id)) else {
                        return Err(format!("no frame \"{id}\" has been shown"));
                    };
                    windows.move_to(key, Vec2::new(x, y));
                    if let (Some(wide), Some(tall)) = (num(3), num(4)) {
                        windows.set_size(key, Vec2::new(wide, tall));
                    }
                }
                Ok(Some(windows.describe()))
            },
        );
        app.console_command(
            "chimera:ui_click",
            "ui_click <x> <y>: click the left button there (window pixels), as the pointer would; tests and \
             screenshots of the menus.",
            |w, a| {
                let at = point(a, 0).ok_or("ui_click <x> <y>")?;
                w.resource_mut::<PointerScript>().click(at);
                Ok(None)
            },
        );
        app.console_command(
            "chimera:ui_drag",
            "ui_drag <x0> <y0> <x1> <y1> [<frames>]: press the left button at x0 y0, move to x1 y1 over that \
             many frames (default 10) and let go (window pixels): a scroll bar's thumb, a slider, a frame.",
            |w, a| {
                let (Some(from), Some(to)) = (point(a, 0), point(a, 2)) else {
                    return Err("ui_drag <x0> <y0> <x1> <y1> [<frames>]".into());
                };
                let steps = a.get(4).and_then(|n| n.parse().ok()).unwrap_or(10);
                w.resource_mut::<PointerScript>().drag(from, to, steps);
                Ok(None)
            },
        );
    }
}

/// Two numbers of a command's words from `i` on, as a point.
fn point(a: &[String], i: usize) -> Option<Vec2> {
    let num = |k: usize| a.get(k).and_then(|v| v.parse::<f32>().ok());
    Some(Vec2::new(num(i)?, num(i + 1)?))
}

// ---------------------------------------------------------------------------
// Clicks: what stops them.

/// The pointer on a node that takes clicks: pressed, hovered or neither,
/// as Bevy UI's focus system (`ui_focus_system`) sets it each frame.
/// Bevy 0.20 deprecates the name `Interaction` (and `Button`, which only
/// added it) for picking's `Hovered` and `ui::Pressed`, but still runs the
/// system; every menu reads this one's changes (a press on mouse down), so
/// it stays under the name Bevy keeps for it until the menus move
/// (docs/backlog.md).
pub type Interaction = bevy::ui::DeprecatedInteraction;

/// What a clickable node spawns with: an `Interaction` that stops the
/// pointer (`FocusPolicy::Block`), so a press reaches it and nothing under
/// it (what Bevy's deprecated `Button` gave, without its accessibility
/// role). Spelled out because `Node` requires `Pass`; `block_clicks` makes
/// the same correction for anything that forgets.
pub fn clickable() -> impl Bundle {
    (Interaction::default(), bevy::ui::FocusPolicy::Block)
}

/// A node with an `Interaction` that lets the pointer through to what is
/// under it as well (`Pass`, whichever of it and the `Interaction` came
/// first; `block_clicks` leaves it so): for a node that only watches the
/// pointer over a region other clickables sit in.
#[derive(Component, Clone, Copy, Debug, Default)]
#[component(on_add = pass_clicks)]
pub struct PassClicks;

fn pass_clicks(mut world: bevy::ecs::world::DeferredWorld, ctx: bevy::ecs::lifecycle::HookContext) {
    match world.get_mut::<bevy::ui::FocusPolicy>(ctx.entity) {
        Some(mut policy) => *policy = bevy::ui::FocusPolicy::Pass,
        None => {
            world.commands().entity(ctx.entity).try_insert(bevy::ui::FocusPolicy::Pass);
        }
    }
}

/// Anything the pointer can press or hover (an `Interaction` added: a
/// clickable, a hover-lit row) stops the pointer at itself, as Bevy's `Button`
/// means to, unless marked `PassClicks`.
fn block_clicks(
    add: On<Add<Interaction>>,
    mut nodes: Query<&mut bevy::ui::FocusPolicy, Without<PassClicks>>,
    pass: Query<(), With<PassClicks>>,
    mut commands: Commands,
) {
    let e = add.entity;
    match nodes.get_mut(e) {
        Ok(mut policy) => {
            if *policy != bevy::ui::FocusPolicy::Block {
                *policy = bevy::ui::FocusPolicy::Block;
            }
        }
        Err(_) if !pass.contains(e) => {
            commands.entity(e).try_insert(bevy::ui::FocusPolicy::Block);
        }
        Err(_) => {}
    }
}

