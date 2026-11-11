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
//! paste; a `Frame` moves by its title bar (kept on screen), comes to the
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

// ---------------------------------------------------------------------------
// Frames: movable, stacked, some sizeable.

/// Which frame: a stable name per dialog (`options`, `servers`, ...).
pub type WindowId = &'static str;

/// Where a frame is on screen: its top-left corner, window pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub pos: Vec2,
    /// Its size as last drawn, window pixels.
    pub size: Vec2,
}

/// Which edges a resize moves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Edges {
    pub left: bool,
    pub right: bool,
    pub top: bool,
    pub bottom: bool,
}

impl Edges {
    pub fn any(self) -> bool {
        self.left || self.right || self.top || self.bottom
    }
}

/// A frame being dragged by its title bar or resized by its edges.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drag {
    pub id: WindowId,
    /// None: moving.
    pub edges: Option<Edges>,
    /// The pointer when it started.
    pub grab: Vec2,
    pub start: Placed,
    /// Window pixels per scheme pixel then, and the frame's minimum size in
    /// scheme pixels.
    pub scale: f32,
    pub min: Vec2,
}

/// The frames' places, sizes set by the user and stacking (back to
/// front), for this session.
#[derive(Resource, Clone, Debug, Default)]
pub struct Windows {
    placed: HashMap<WindowId, Placed>,
    /// Sizes the user gave sizeable frames, scheme pixels.
    sizes: HashMap<WindowId, Vec2>,
    order: Vec<WindowId>,
    pub drag: Option<Drag>,
    /// Bumped when a user size changes (the dialog draws again).
    pub resized: u64,
}

/// Window pixels of the title bar a frame is dragged by (scheme pixels).
pub const CAPTION_H: f32 = 26.0;
/// How far in from a sizeable frame's edge its border grabs (scheme px).
pub const GRIP: f32 = 5.0;
/// The corner grip of a sizeable frame (scheme px).
pub const CORNER: f32 = 14.0;

impl Windows {
    /// A frame `size` window pixels big on a `screen`: where it goes (the
    /// first time centred, then where it was left, kept on screen).
    pub fn place(&mut self, id: WindowId, size: Vec2, screen: Vec2) -> Vec2 {
        let centred = ((screen - size) / 2.0).max(Vec2::ZERO);
        let pos = self.placed.get(id).map_or(centred, |p| p.pos);
        let pos = clamp_on_screen(pos, size, screen);
        self.placed.insert(id, Placed { pos, size });
        if !self.order.contains(&id) {
            self.order.push(id);
        }
        pos
    }

    /// Put frame `id` (`size` window pixels) at `pos` from now on, as if
    /// dragged there (an app moving a dialog out of another's way).
    pub fn put(&mut self, id: WindowId, pos: Vec2, size: Vec2) {
        self.placed.insert(id, Placed { pos, size });
    }

    pub fn placed(&self, id: WindowId) -> Option<Placed> {
        self.placed.get(id).copied()
    }

    /// The size the user gave a sizeable frame (scheme pixels).
    pub fn size(&self, id: WindowId) -> Option<Vec2> {
        self.sizes.get(id).copied()
    }

    /// Bring a frame to the front.
    pub fn raise(&mut self, id: WindowId) {
        self.order.retain(|w| *w != id);
        self.order.push(id);
    }

    /// Its place in the stacking, 0 at the back (frames not yet shown go
    /// in front).
    pub fn rank(&self, id: WindowId) -> usize {
        self.order.iter().position(|w| *w == id).unwrap_or(self.order.len())
    }

    /// The front one of `shown`.
    pub fn front(&self, shown: &[WindowId]) -> Option<WindowId> {
        shown.iter().copied().max_by_key(|id| self.rank(id))
    }

    /// Start moving (`edges` None) or resizing a frame from the pointer at
    /// `at`; `scale` window pixels per scheme pixel, `min` its smallest size
    /// in scheme pixels.
    pub fn begin(&mut self, id: WindowId, edges: Option<Edges>, at: Vec2, scale: f32, min: Vec2) {
        let Some(start) = self.placed(id) else { return };
        self.raise(id);
        self.drag = Some(Drag {
            id,
            edges,
            grab: at,
            start,
            scale,
            min,
        });
    }

    /// The pointer moved to `at` while dragging: the frame's new place
    /// (kept on screen; a resize no smaller than its minimum and no bigger
    /// than the screen).
    pub fn drag_to(&mut self, at: Vec2, screen: Vec2) -> Option<Placed> {
        let d = self.drag?;
        let delta = at - d.grab;
        let placed = match d.edges {
            None => Placed {
                pos: clamp_on_screen(d.start.pos + delta, d.start.size, screen),
                size: d.start.size,
            },
            Some(e) => {
                let min = d.min * d.scale;
                let (mut x0, mut y0) = (d.start.pos.x, d.start.pos.y);
                let (mut x1, mut y1) = (x0 + d.start.size.x, y0 + d.start.size.y);
                if e.left {
                    x0 = (x0 + delta.x).clamp(0.0, x1 - min.x);
                }
                if e.right {
                    x1 = (x1 + delta.x).clamp(x0 + min.x, screen.x.max(x0 + min.x));
                }
                if e.top {
                    y0 = (y0 + delta.y).clamp(0.0, y1 - min.y);
                }
                if e.bottom {
                    y1 = (y1 + delta.y).clamp(y0 + min.y, screen.y.max(y0 + min.y));
                }
                let size = Vec2::new(x1 - x0, y1 - y0);
                let scheme = (size / d.scale).round();
                if self.sizes.get(d.id) != Some(&scheme) {
                    self.sizes.insert(d.id, scheme);
                    self.resized += 1;
                }
                Placed {
                    pos: Vec2::new(x0, y0),
                    size,
                }
            }
        };
        self.placed.insert(d.id, placed);
        Some(placed)
    }

    pub fn end_drag(&mut self) {
        self.drag = None;
    }

    /// Put a frame at `pos` (as a drag would end there; kept on screen when
    /// next placed) and bring it to the front.
    pub fn move_to(&mut self, id: WindowId, pos: Vec2) {
        if let Some(p) = self.placed.get_mut(id) {
            p.pos = pos;
        }
        self.raise(id);
    }

    /// Give a sizeable frame a size (scheme pixels).
    pub fn set_size(&mut self, id: WindowId, size: Vec2) {
        self.sizes.insert(id, size);
        self.resized += 1;
    }

    /// The frames, back to front, with their places (`vgui_windows`).
    pub fn describe(&self) -> String {
        self.order
            .iter()
            .map(|id| match self.placed(id) {
                Some(p) => format!(
                    "{id}: {:.0},{:.0} {:.0}x{:.0}{}",
                    p.pos.x,
                    p.pos.y,
                    p.size.x,
                    p.size.y,
                    self.size(id)
                        .map_or(String::new(), |s| format!(" (sized {:.0}x{:.0})", s.x, s.y))
                ),
                None => id.to_string(),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// A box kept wholly on the screen (its top-left corner when bigger).
pub fn clamp_on_screen(pos: Vec2, size: Vec2, screen: Vec2) -> Vec2 {
    let max = (screen - size).max(Vec2::ZERO);
    pos.clamp(Vec2::ZERO, max).round()
}

/// Which edges of a sizeable frame at `placed` the pointer at `at` grabs
/// (`grip` and `corner` in window pixels).
pub fn edges_at(placed: Placed, at: Vec2, grip: f32, corner: f32) -> Edges {
    let local = at - placed.pos;
    let size = placed.size;
    if local.x < 0.0 || local.y < 0.0 || local.x > size.x || local.y > size.y {
        return Edges::default();
    }
    let in_corner = local.x > size.x - corner && local.y > size.y - corner;
    Edges {
        left: local.x < grip,
        right: local.x > size.x - grip || in_corner,
        top: local.y < grip,
        bottom: local.y > size.y - grip || in_corner,
    }
}

/// A drawn frame: which one, whether it resizes and its minimum size
/// (scheme pixels), its title bar's height and the window pixels per
/// scheme pixel it was drawn at. `place_frames` sets where it is.
#[derive(Component, Clone, Copy, Debug)]
pub struct VguiFrame {
    pub id: WindowId,
    pub size: Vec2,
    pub scale: f32,
    pub sizeable: Option<Vec2>,
    /// Stacked over its owner's frames (a modal dialog).
    pub modal: bool,
}

/// What a frame hides of what's under it: the game menu's entries and
/// title never show through a dialog's see-through colour. The frame is
/// drawn over the picture behind them (the main menu's background, the
/// part under the frame), in a game over the world drawn again, dimmed
/// (`WorldPicture`), or a solid colour; None (no menu open): see-through.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct FrameBacking(pub Option<Backing>);

#[derive(Clone, Debug, PartialEq)]
pub enum Backing {
    /// A picture stretched over the window, and its size in pixels.
    Picture(Handle<Image>, Vec2),
    Color(Color),
    /// In a game: the world (`WorldPicture`, when it is being drawn)
    /// under the frame at `WORLD_UNDER_DIALOGS` of its brightness, else
    /// this colour.
    World(Color),
}

/// The world's 3D view drawn into a picture the window's size, while a
/// dialog is open in a game (`game_menu::world_behind_dialogs`), and the
/// picture's size in pixels: frames there are drawn over it.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct WorldPicture(pub Option<(Handle<Image>, Vec2)>);

/// How bright the world shows under an in-game dialog (the menu's
/// entries under the frame hidden): a darkened, see-through look.
pub const WORLD_UNDER_DIALOGS: f32 = 0.4;

/// `color` with its alpha raised so it covers what is under it as much as
/// that alpha does blended in display (gamma) space, VGUI's way and what
/// a panel's or shade's alpha is chosen for. Bevy's UI blends in linear
/// light: a black shade at 0.55 there leaves the world at ~70% of its
/// brightness (not 45%), and a panel at 0.96 lets bright text behind it
/// show through as a ~25% grey. Exact for black over anything, close for
/// dark panels: `1 - (1 - a)^2.2`.
pub fn display_alpha(color: Color) -> Color {
    let a = color.alpha().clamp(0.0, 1.0);
    color.with_alpha(1.0 - (1.0 - a).powf(2.2))
}

impl Backing {
    /// The part of the picture under a frame at `pos`, `size` (window
    /// pixels) in its own pixels.
    pub fn crop(image: Vec2, screen: Vec2, pos: Vec2, size: Vec2) -> Rect {
        let scale = image / screen.max(Vec2::ONE);
        Rect::from_corners(pos * scale, (pos + size) * scale)
    }
}

/// Each drawn frame where `Windows` puts it, stacked in its order, over
/// `FrameBacking`.
#[allow(clippy::type_complexity)]
fn place_frames(
    mut windows: ResMut<Windows>,
    backing: Res<FrameBacking>,
    world: Option<Res<WorldPicture>>,
    screen: Query<&Window, With<PrimaryWindow>>,
    mut frames: Query<(Entity, &VguiFrame, &mut Node, &mut GlobalZIndex, &mut BackgroundColor, Option<&mut ImageNode>)>,
    mut commands: Commands,
) {
    let Some(win) = screen.iter().next() else { return };
    let screen = Vec2::new(win.width(), win.height());
    for (e, f, mut node, mut z, mut bg, image) in &mut frames {
        let size = f.size * f.scale;
        let pos = match windows.bypass_change_detection().drag {
            Some(d) if d.id == f.id => windows.placed(f.id).map_or(Vec2::ZERO, |p| p.pos),
            _ => windows.bypass_change_detection().place(f.id, size, screen),
        };
        // Under it: the backing (1 px in, the border's width).
        let drawn = windows.placed(f.id).map_or(size, |p| p.size);
        let crop = |handle: &Handle<Image>, image_size: Vec2, tint: f32| {
            let rect = Backing::crop(image_size, screen, pos + 1.0, drawn - 2.0);
            (handle.clone(), rect, Color::srgb(tint, tint, tint))
        };
        let (base, picture) = match &backing.0 {
            Some(Backing::Picture(handle, image_size)) => (Color::BLACK, Some(crop(handle, *image_size, 1.0))),
            Some(Backing::World(c)) => match world.as_ref().and_then(|w| w.0.as_ref()) {
                Some((handle, image_size)) => (Color::BLACK, Some(crop(handle, *image_size, WORLD_UNDER_DIALOGS))),
                None => (*c, None),
            },
            Some(Backing::Color(c)) => (*c, None),
            None => (Color::NONE, None),
        };
        bg.set_if_neq(BackgroundColor(base));
        match (picture, image) {
            (Some((handle, rect, tint)), Some(mut img)) => {
                if img.image != handle || img.rect != Some(rect) || img.color != tint {
                    img.image = handle;
                    img.rect = Some(rect);
                    img.color = tint;
                }
            }
            (Some((handle, rect, tint)), None) => {
                commands.entity(e).insert(ImageNode {
                    image: handle,
                    rect: Some(rect),
                    color: tint,
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                });
            }
            (None, Some(_)) => {
                commands.entity(e).remove::<ImageNode>();
            }
            (None, None) => {}
        }
        let (left, top) = (px(pos.x), px(pos.y));
        if node.left != left || node.top != top {
            node.left = left;
            node.top = top;
        }
        if let Some(d) = windows.drag.filter(|d| d.id == f.id && d.edges.is_some())
            && let Some(p) = windows.placed(d.id)
        {
            node.width = px(p.size.x);
            node.height = px(p.size.y);
        }
        let rank = windows.rank(f.id) as i32;
        let want = GlobalZIndex(if f.modal { MODAL_Z } else { FRAME_Z } + rank * 2);
        if *z != want {
            *z = want;
        }
    }
}

/// The game menu (GameUI: its backdrop, entries and dialogs) stacks from
/// here, over every in-game panel (the HUD 30-45, the message of the day,
/// team, class, buy, radio and spectator menus 50-52, the scoreboard 60),
/// as CS:S's GameUI sits over the game's viewport panels; its backdrop
/// takes the clicks meant for them while it is open.
pub const GAME_UI_Z: i32 = 62;
/// Frames stack from here (over the menu's own backdrop, `GAME_UI_Z`),
/// two apart by stacking rank.
pub const FRAME_Z: i32 = GAME_UI_Z + 2;
/// Modal frames stack from here, over the others, under the console
/// (100).
pub const MODAL_Z: i32 = 90;
/// An open drop-down list: over every frame.
pub const POPUP_Z: i32 = 150;

/// The window's pointer in window pixels, and the frames under it front
/// first.
fn frame_rects(frames: &Query<(&VguiFrame, &ComputedNode, &UiGlobalTransform)>) -> Vec<(VguiFrame, Rect)> {
    frames
        .iter()
        .map(|(f, node, t)| {
            let size = node.size() * node.inverse_scale_factor();
            let centre = t.translation * node.inverse_scale_factor();
            (*f, Rect::from_center_size(centre, size))
        })
        .collect()
}

/// A press on a frame brings it to the front; on its title bar it starts
/// a move, on a sizeable frame's edge a resize; the drag follows the
/// pointer until the button is let go.
fn frames_pointer(
    mouse: Res<ButtonInput<MouseButton>>,
    window: Query<&Window, With<PrimaryWindow>>,
    frames: Query<(&VguiFrame, &ComputedNode, &UiGlobalTransform)>,
    mut windows: ResMut<Windows>,
) {
    let Some(win) = window.iter().next() else { return };
    let screen = Vec2::new(win.width(), win.height());
    let cursor = win.cursor_position();
    if windows.drag.is_some() {
        if !mouse.pressed(MouseButton::Left) {
            windows.end_drag();
        } else if let Some(at) = cursor {
            windows.drag_to(at, screen);
        }
        return;
    }
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(at) = cursor else { return };
    let mut under: Vec<(VguiFrame, Rect)> = frame_rects(&frames)
        .into_iter()
        .filter(|(_, r)| r.contains(at))
        .collect();
    under.sort_by_key(|(f, _)| std::cmp::Reverse(windows.rank(f.id) as i32 * 2 + if f.modal { 1000 } else { 0 }));
    let Some((f, rect)) = under.first().copied() else {
        return;
    };
    windows.raise(f.id);
    let placed = Placed {
        pos: rect.min,
        size: rect.size(),
    };
    windows.bypass_change_detection().placed.insert(f.id, placed);
    if let Some(min) = f.sizeable {
        let edges = edges_at(placed, at, GRIP * f.scale, CORNER * f.scale);
        if edges.any() {
            windows.begin(f.id, Some(edges), at, f.scale, min);
            return;
        }
    }
    // The title bar, short of the close box at its right.
    let local = at - rect.min;
    if local.y < CAPTION_H * f.scale && local.x < rect.width() - 30.0 * f.scale {
        windows.begin(f.id, None, at, f.scale, Vec2::ZERO);
    }
}

// ---------------------------------------------------------------------------
// ComboBox: the drop-down list.

/// A combo box's open list: the entry highlighted and the first shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComboList {
    /// Which combo box it belongs to (the owner's number for it).
    pub owner: usize,
    pub len: usize,
    pub highlight: usize,
    pub first: usize,
}

/// What a key or click does to an open list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComboEvent {
    /// Still open.
    Open,
    /// Picked an entry: closed with it.
    Pick(usize),
    /// Closed without a pick.
    Close,
}

/// Keys an open list takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComboKey {
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    Enter,
    Escape,
}

/// Entries an open list shows at once (VGUI's `ComboBox` default).
pub const COMBO_ROWS: usize = 10;

impl ComboList {
    /// Open on `selected` (scrolled to show it).
    pub fn open(owner: usize, len: usize, selected: usize) -> Self {
        let mut l = Self {
            owner,
            len,
            highlight: selected.min(len.saturating_sub(1)),
            first: 0,
        };
        l.show_highlight();
        l
    }

    pub fn rows(&self) -> usize {
        self.len.min(COMBO_ROWS)
    }

    fn show_highlight(&mut self) {
        let rows = self.rows().max(1);
        if self.highlight < self.first {
            self.first = self.highlight;
        } else if self.highlight >= self.first + rows {
            self.first = self.highlight + 1 - rows;
        }
        self.first = self.first.min(self.len.saturating_sub(rows));
    }

    /// The pointer over entry `i`: highlighted.
    pub fn hover(&mut self, i: usize) {
        if i < self.len {
            self.highlight = i;
        }
    }

    /// Entry `i` highlighted and scrolled to (a typed letter jumps so).
    pub fn select(&mut self, i: usize) {
        if i < self.len {
            self.highlight = i;
            self.show_highlight();
        }
    }

    /// The wheel over the list: scrolled `notches` entries.
    pub fn wheel(&mut self, notches: i32) {
        let max = self.len.saturating_sub(self.rows()) as i32;
        self.first = (self.first as i32 + notches).clamp(0, max) as usize;
    }

    pub fn key(&mut self, key: ComboKey) -> ComboEvent {
        if self.len == 0 {
            return ComboEvent::Close;
        }
        let last = self.len - 1;
        let page = self.rows().max(1);
        self.highlight = match key {
            ComboKey::Up => self.highlight.saturating_sub(1),
            ComboKey::Down => (self.highlight + 1).min(last),
            ComboKey::PageUp => self.highlight.saturating_sub(page),
            ComboKey::PageDown => (self.highlight + page).min(last),
            ComboKey::Home => 0,
            ComboKey::End => last,
            ComboKey::Enter => return ComboEvent::Pick(self.highlight),
            ComboKey::Escape => return ComboEvent::Close,
        };
        self.show_highlight();
        ComboEvent::Open
    }
}

/// A closed combo box stepped (focused: Up/Down, the wheel): the entry
/// `dir` away, clamped as VGUI's (no wrapping).
pub fn combo_step(selected: usize, len: usize, dir: i32) -> usize {
    if len == 0 {
        return 0;
    }
    (selected as i32 + dir).clamp(0, len as i32 - 1) as usize
}

// ---------------------------------------------------------------------------
// Slider.

/// A slider held down: which (its owner and number) and its track in
/// window pixels; the fraction the pointer is at, each frame it's held.
#[derive(Resource, Clone, Debug, Default)]
pub struct SliderDrag {
    pub held: Option<(SliderOwner, usize, Rect)>,
    /// This frame's position along the held slider, 0 to 1.
    pub at: Option<(SliderOwner, usize, f32)>,
}

/// Whose slider it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SliderOwner {
    Menu,
}

/// A slider's track (where the nob runs): pressing it grabs the slider.
#[derive(Component, Clone, Copy, Debug)]
pub struct SliderTrack {
    pub owner: SliderOwner,
    pub id: usize,
    /// The nob's width in scheme pixels (the track's ends are half a nob
    /// in).
    pub inset: f32,
}

/// Where along a track `left`..`left + width` the pointer at `x` sets the
/// value, 0 to 1.
pub fn slider_fraction(x: f32, left: f32, width: f32) -> f32 {
    if width <= 0.0 {
        return 0.0;
    }
    ((x - left) / width).clamp(0.0, 1.0)
}

/// A slider pressed is held (even as dialogs draw again) until the button
/// is let go; each frame it reports where along it the pointer is.
fn slider_pointer(
    mouse: Res<ButtonInput<MouseButton>>,
    window: Query<&Window, With<PrimaryWindow>>,
    tracks: Query<(&SliderTrack, &ComputedNode, &UiGlobalTransform)>,
    windows: Res<Windows>,
    mut drag: ResMut<SliderDrag>,
) {
    let at = window.iter().next().and_then(Window::cursor_position);
    if drag.held.is_some() && !mouse.pressed(MouseButton::Left) {
        drag.held = None;
    }
    if mouse.just_pressed(MouseButton::Left)
        && windows.drag.is_none()
        && let Some(p) = at
    {
        drag.held = tracks.iter().find_map(|(t, node, g)| {
            let size = node.size() * node.inverse_scale_factor();
            let centre = g.translation * node.inverse_scale_factor();
            let rect = Rect::from_center_size(centre, size);
            let inset = (t.inset / 2.0) * (size.y / 24.0).max(0.1);
            rect.contains(p).then(|| {
                (
                    t.owner,
                    t.id,
                    Rect::new(rect.min.x + inset, rect.min.y, rect.max.x - inset, rect.max.y),
                )
            })
        });
    }
    let now = match (drag.held, at) {
        (Some((owner, id, r)), Some(p)) => Some((owner, id, slider_fraction(p.x, r.min.x, r.width()))),
        _ => None,
    };
    if drag.at != now {
        drag.at = now;
    }
}

// ---------------------------------------------------------------------------
// ScrollBar: a list's bar, its view free of the list's selection.

/// The first row a list of `len` rows showing `shown` can start at, from
/// `first` (clamped to the rows there are).
pub fn scroll_clamp(first: i32, len: usize, shown: usize) -> usize {
    first.clamp(0, len.saturating_sub(shown) as i32) as usize
}

/// A scroll bar's thumb in its track (`room` tall, between the arrows):
/// its top and height, None when every row shows (no thumb, as VGUI's).
pub fn scroll_thumb(len: usize, shown: usize, first: usize, room: f32) -> Option<(f32, f32)> {
    if len <= shown || room <= 8.0 {
        return None;
    }
    let thumb = (room * shown as f32 / len as f32).max(10.0).min(room);
    let top = (room - thumb) * first.min(len - shown) as f32 / (len - shown) as f32;
    Some((top, thumb))
}

/// The first row shown with the thumb's top dragged to `top` (track
/// pixels; VGUI's `ScrollBarSlider` maps the nob's travel to the range).
pub fn scroll_first_at(len: usize, shown: usize, room: f32, thumb: f32, top: f32) -> usize {
    let travel = room - thumb;
    if len <= shown || travel <= 0.0 {
        return 0;
    }
    let f = (top / travel).clamp(0.0, 1.0);
    (f * (len - shown) as f32).round() as usize
}

/// A scroll bar's thumb: pressing it grabs it (`ScrollDrag`); `owner` and
/// `id` say whose list it is, the rest where it was drawn (scheme pixels).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct ScrollThumb {
    pub owner: WindowId,
    pub id: usize,
    pub len: usize,
    pub shown: usize,
    /// The track's height between the arrows, the thumb's top in it and
    /// its height.
    pub room: f32,
    pub top: f32,
    pub thumb: f32,
}

/// The owner of an open drop-down's list (`combo_popup`): its thumb's
/// drag scrolls whichever list is open.
pub const COMBO_SCROLL: WindowId = "combo";

/// A scroll bar's thumb held: which list and where it was grabbed; each
/// frame it's held, the first row the pointer puts at the top.
#[derive(Resource, Clone, Debug, Default)]
pub struct ScrollDrag {
    pub held: Option<ScrollHeld>,
    /// This frame's first row of the held list (owner, id, first).
    pub at: Option<(WindowId, usize, usize)>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollHeld {
    pub thumb: ScrollThumb,
    /// The pointer's y when pressed, window pixels, and window pixels per
    /// scheme pixel.
    pub grab: f32,
    pub scale: f32,
}

impl ScrollHeld {
    /// The first row with the pointer at `y` (window pixels).
    pub fn first_at(&self, y: f32) -> usize {
        let t = self.thumb;
        let top = t.top + (y - self.grab) / self.scale.max(0.01);
        scroll_first_at(t.len, t.shown, t.room, t.thumb, top)
    }
}

/// A thumb pressed is held (as lists draw again) until the button is let
/// go; each frame it reports the first row the pointer puts at the top.
fn scroll_pointer(
    mouse: Res<ButtonInput<MouseButton>>,
    window: Query<&Window, With<PrimaryWindow>>,
    thumbs: Query<(&ScrollThumb, &Interaction, &ComputedNode), Changed<Interaction>>,
    mut drag: ResMut<ScrollDrag>,
) {
    let y = window.iter().next().and_then(Window::cursor_position).map(|p| p.y);
    if drag.held.is_some() && !mouse.pressed(MouseButton::Left) {
        drag.held = None;
    }
    if let Some(y) = y {
        for (thumb, interaction, node) in &thumbs {
            if *interaction == Interaction::Pressed {
                let tall = node.size().y * node.inverse_scale_factor();
                drag.held = Some(ScrollHeld {
                    thumb: *thumb,
                    grab: y,
                    scale: tall / thumb.thumb.max(1.0),
                });
            }
        }
    }
    let now = match (drag.held, y) {
        (Some(h), Some(y)) => Some((h.thumb.owner, h.thumb.id, h.first_at(y))),
        _ => None,
    };
    if drag.at != now {
        drag.at = now;
    }
}

/// A `ScrollBar` `h` tall at (x, y) in `parent` for a list of `len` rows
/// showing `shown` from `first`: its arrows (`hit(-1)`, `hit(1)`: a row),
/// the track above and below the thumb (`hit(-shown)`, `hit(shown)`: a
/// page, as VGUI's), the thumb, dragged (`ScrollDrag`, `owner`/`id`).
#[allow(clippy::too_many_arguments)]
pub fn scroll_bar<B: Bundle>(
    commands: &mut Commands,
    parent: Entity,
    look: &Look,
    (x, y, h): (f32, f32, f32),
    (len, shown, first): (usize, usize, usize),
    (owner, id): (WindowId, usize),
    hit: impl Fn(i32) -> B,
) -> Entity {
    let w = look.number("ScrollBar.Wide", 17.0);
    let fg = look.color("ScrollBarButton.FgColor", [255, 255, 255, 255]);
    let track = commands
        .spawn((place(look, x, y, w, h), BackgroundColor(look.sunken_bg()), ChildOf(parent)))
        .id();
    let font = look.font("Marlett", (12.0, false));
    for (ty, text, step) in [(0.0, "\u{25B2}", -1), (h - w, "\u{25BC}", 1)] {
        let b = commands
            .spawn((
                Node {
                    border: UiRect::all(px(1.0)),
                    ..place(look, 0.0, ty, w, w)
                },
                bevel(look, true),
                hit(step),
                clickable(),
                ChildOf(track),
            ))
            .id();
        label(commands, b, look, (0.0, 0.0, w - 2.0, w - 2.0), text, font.clone(), fg, 0);
    }
    let room = h - 2.0 * w;
    if let Some((top, thumb)) = scroll_thumb(len, shown, first, room) {
        let page = shown as i32;
        commands.spawn((
            place(look, 0.0, w, w, top),
            hit(-page),
            clickable(),
            ChildOf(track),
        ));
        commands.spawn((
            place(look, 0.0, w + top + thumb, w, room - top - thumb),
            hit(page),
            clickable(),
            ChildOf(track),
        ));
        commands.spawn((
            Node {
                border: UiRect::all(px(1.0)),
                ..place(look, 1.0, w + top, w - 2.0, thumb)
            },
            bevel(look, true),
            BackgroundColor(look.color("ScrollBarSlider.BgColor", [255, 255, 255, 64])),
            ScrollThumb {
                owner,
                id,
                len,
                shown,
                room,
                top,
                thumb,
            },
            clickable(),
            ChildOf(track),
        ));
    }
    track
}

// ---------------------------------------------------------------------------
// Synthetic clicks: the pointer driven from the console (tests, screenshots).

/// Pointer steps the console queued (`ui_click`, `ui_drag`): each frame
/// the next one moves the cursor and presses or lets go of the left
/// button, as a real pointer would through Bevy's input.
#[derive(Resource, Clone, Debug, Default)]
pub struct PointerScript(pub std::collections::VecDeque<PointerStep>);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerStep {
    /// The cursor's place, window pixels.
    pub at: Vec2,
    /// Some: the left button pressed (true) or let go (false).
    pub button: Option<bool>,
}

impl PointerScript {
    /// A click at `at`: there, pressed, let go a few frames later.
    pub fn click(&mut self, at: Vec2) {
        self.drag(at, at, 0);
    }

    /// Press at `from`, move to `to` in `steps` frames, let go.
    pub fn drag(&mut self, from: Vec2, to: Vec2, steps: usize) {
        let q = &mut self.0;
        q.push_back(PointerStep { at: from, button: None });
        q.push_back(PointerStep { at: from, button: Some(true) });
        q.push_back(PointerStep { at: from, button: None });
        for k in 1..=steps {
            q.push_back(PointerStep {
                at: from.lerp(to, k as f32 / steps as f32),
                button: None,
            });
        }
        q.push_back(PointerStep { at: to, button: None });
        q.push_back(PointerStep { at: to, button: Some(false) });
        q.push_back(PointerStep { at: to, button: None });
    }
}

/// The next queued pointer step: the cursor moved there, the button's
/// press or release sent as Bevy's input messages.
fn run_pointer_script(
    mut script: ResMut<PointerScript>,
    mut window: Query<(Entity, &mut Window), With<PrimaryWindow>>,
    mut buttons: MessageWriter<bevy::input::mouse::MouseButtonInput>,
) {
    let Some(step) = script.0.pop_front() else { return };
    let Ok((entity, mut win)) = window.single_mut() else { return };
    // As the system reports a pointer (no warp: Wayland refuses one).
    win.bypass_change_detection().set_cursor_position(Some(step.at));
    if let Some(down) = step.button {
        buttons.write(bevy::input::mouse::MouseButtonInput {
            button: MouseButton::Left,
            state: if down {
                bevy::input::ButtonState::Pressed
            } else {
                bevy::input::ButtonState::Released
            },
            window: entity,
        });
    }
}

// ---------------------------------------------------------------------------
// TextEntry: caret and selection.

/// A text entry's caret and the other end of its selection (char
/// indices; equal: nothing selected).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Caret {
    pub at: usize,
    pub anchor: usize,
}

/// An edit to a text entry.
#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    /// Typed or pasted text (control characters dropped), over the
    /// selection.
    Insert(String),
    Backspace,
    Delete,
    /// The caret a char left or right (`true`: extend the selection).
    Left(bool),
    Right(bool),
    Home(bool),
    End(bool),
    SelectAll,
    /// The caret to a char index (a click; `true`: extend, a drag).
    To(usize, bool),
}

impl Caret {
    /// The caret at the end of `text` (a click into an entry with nothing
    /// to place it by).
    pub fn end_of(text: &str) -> Self {
        let n = text.chars().count();
        Self { at: n, anchor: n }
    }

    /// The selected chars' range.
    pub fn range(&self) -> std::ops::Range<usize> {
        self.at.min(self.anchor)..self.at.max(self.anchor)
    }

    pub fn selected<'a>(&self, text: &'a str) -> &'a str {
        let r = self.range();
        &text[byte(text, r.start)..byte(text, r.end)]
    }

    /// Apply an edit to `text`, at most `max` bytes long. Returns whether
    /// the text changed.
    pub fn edit(&mut self, text: &mut String, edit: Edit, max: usize) -> bool {
        let n = text.chars().count();
        self.at = self.at.min(n);
        self.anchor = self.anchor.min(n);
        let r = self.range();
        let moved = |c: &mut Self, to: usize, extend: bool| {
            c.at = to;
            if !extend {
                c.anchor = to;
            }
        };
        match edit {
            Edit::Insert(s) => {
                let s: String = s.chars().filter(|c| !c.is_control()).collect();
                text.replace_range(byte(text, r.start)..byte(text, r.end), "");
                let mut fitted = String::new();
                for c in s.chars() {
                    if text.len() + fitted.len() + c.len_utf8() > max {
                        break;
                    }
                    fitted.push(c);
                }
                let at = byte(text, r.start);
                text.insert_str(at, &fitted);
                let to = r.start + fitted.chars().count();
                moved(self, to, false);
                return !fitted.is_empty() || !r.is_empty();
            }
            Edit::Backspace | Edit::Delete => {
                let cut = if !r.is_empty() {
                    r
                } else if edit == Edit::Backspace && self.at > 0 {
                    self.at - 1..self.at
                } else if edit == Edit::Delete && self.at < n {
                    self.at..self.at + 1
                } else {
                    return false;
                };
                text.replace_range(byte(text, cut.start)..byte(text, cut.end), "");
                moved(self, cut.start, false);
                return true;
            }
            Edit::Left(extend) => {
                let to = if !extend && !r.is_empty() {
                    r.start
                } else {
                    self.at.saturating_sub(1)
                };
                moved(self, to, extend);
            }
            Edit::Right(extend) => {
                let to = if !extend && !r.is_empty() {
                    r.end
                } else {
                    (self.at + 1).min(n)
                };
                moved(self, to, extend);
            }
            Edit::Home(extend) => moved(self, 0, extend),
            Edit::End(extend) => moved(self, n, extend),
            Edit::SelectAll => {
                self.anchor = 0;
                self.at = n;
            }
            Edit::To(i, extend) => moved(self, i.min(n), extend),
        }
        false
    }
}

/// The byte offset of char `i` in `s` (its end past the last).
fn byte(s: &str, i: usize) -> usize {
    s.char_indices().nth(i).map_or(s.len(), |(b, _)| b)
}

/// The char boundary nearest `x` given each boundary's offset (`offsets`,
/// one more than the chars).
pub fn char_at(offsets: &[f32], x: f32) -> usize {
    offsets
        .iter()
        .enumerate()
        .min_by(|a, b| (a.1 - x).abs().total_cmp(&(b.1 - x).abs()))
        .map_or(0, |(i, _)| i)
}

// ---------------------------------------------------------------------------
// Focus order.

/// The control `dir` (+1 Tab, -1 Shift+Tab) on from `current` in a
/// dialog's tab order (wrapping; the first when none is focused).
pub fn focus_step<T: PartialEq + Copy>(order: &[T], current: Option<T>, dir: i32) -> Option<T> {
    if order.is_empty() {
        return None;
    }
    let n = order.len() as i32;
    let at = current.and_then(|c| order.iter().position(|o| *o == c));
    let next = match at {
        Some(i) => (i as i32 + dir).rem_euclid(n),
        None if dir < 0 => n - 1,
        None => 0,
    };
    Some(order[next as usize])
}

// ---------------------------------------------------------------------------
// The look and the drawing helpers.

/// Colours, sizes and fonts: the GameUI scheme's, else built in.
pub struct Look<'a> {
    pub ui: Option<&'a GameUi>,
    pub fonts: &'a UiFonts,
    /// Pixels per scheme pixel (GameUI is drawn in screen pixels; larger
    /// windows scale it up).
    pub s: f32,
    pub height: f32,
    pub accent: Color,
}

impl<'a> Look<'a> {
    pub fn color(&self, name: &str, fallback: [u8; 4]) -> Color {
        let [r, g, b, a] = self.ui.and_then(|u| u.color(name)).unwrap_or(fallback);
        Color::srgba_u8(r, g, b, a)
    }

    pub fn number(&self, name: &str, fallback: f32) -> f32 {
        self.ui.and_then(|u| u.numbers.get(name).copied()).unwrap_or(fallback)
    }

    /// Built in colours stand in for the scheme's: a darker panel, our
    /// accent for selections.
    pub fn has_scheme(&self) -> bool {
        self.ui.is_some_and(|u| !u.colors.is_empty())
    }

    /// A GameUI scheme font (`Default`, `UiBold`, `MenuLarge`) at this
    /// window's size: `fallback` scheme pixels tall and bold when the
    /// scheme lacks it.
    pub fn font(&self, name: &str, fallback: (f32, bool)) -> TextFont {
        self.fonts.source(name, self.height, self.s, fallback)
    }

    /// The dialogs' text font.
    pub fn default_font(&self) -> TextFont {
        self.font("Default", (16.0, false))
    }

    pub fn frame_bg(&self) -> Color {
        if self.has_scheme() {
            self.color("Frame.BgColor", [160, 160, 160, 128])
        } else {
            Color::srgba_u8(28, 28, 28, 230)
        }
    }

    pub fn bright(&self) -> Color {
        self.color("Border.Bright", [200, 200, 200, 196])
    }

    pub fn dark(&self) -> Color {
        self.color("Border.Dark", [40, 40, 40, 196])
    }

    pub fn text(&self) -> Color {
        self.color("Label.TextColor", [221, 221, 221, 255])
    }

    pub fn dull(&self) -> Color {
        self.color("Label.TextDullColor", [190, 190, 190, 255])
    }

    pub fn disabled(&self) -> Color {
        self.color("Label.DisabledFgColor1", [117, 117, 117, 255])
    }

    pub fn white(&self) -> Color {
        self.color("Label.TextBrightColor", [255, 255, 255, 255])
    }

    pub fn selected_bg(&self) -> Color {
        if self.has_scheme() {
            self.color("SectionedListPanel.SelectedBgColor", [255, 155, 0, 255])
        } else {
            self.accent
        }
    }

    pub fn selected_text(&self) -> Color {
        self.color("SectionedListPanel.SelectedTextColor", [0, 0, 0, 255])
    }

    pub fn sunken_bg(&self) -> Color {
        self.color("TextEntry.BgColor", [0, 0, 0, 128])
    }

    pub fn px(&self, v: f32) -> Val {
        px((v * self.s).round())
    }
}

/// An absolutely placed box, in scheme pixels, inside `parent`.
pub fn place(look: &Look, x: f32, y: f32, w: f32, h: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: look.px(x),
        top: look.px(y),
        width: look.px(w),
        height: look.px(h),
        ..default()
    }
}

/// Raised (lit top-left) or sunken (lit bottom-right) VGUI borders.
pub fn bevel(look: &Look, raised: bool) -> BorderColor {
    let (a, b) = if raised {
        (look.bright(), look.dark())
    } else {
        (look.dark(), look.bright())
    };
    BorderColor {
        top: a,
        left: a,
        bottom: b,
        right: b,
    }
}

/// Text in a box: one line, vertically centred, `align` -1 left, 0
/// centre, 1 right.
#[allow(clippy::too_many_arguments)]
pub fn label(
    commands: &mut Commands,
    parent: Entity,
    look: &Look,
    (x, y, w, h): (f32, f32, f32, f32),
    text: &str,
    font: TextFont,
    color: Color,
    align: i32,
) -> Entity {
    let node = Node {
        display: Display::Flex,
        align_items: AlignItems::Center,
        justify_content: match align {
            -1 => JustifyContent::FlexStart,
            0 => JustifyContent::Center,
            _ => JustifyContent::FlexEnd,
        },
        overflow: Overflow::clip(),
        ..place(look, x, y, w, h)
    };
    let e = commands.spawn((node, ChildOf(parent))).id();
    commands.spawn((
        Text::new(text),
        font,
        TextColor(color),
        TextLayout::new(Justify::Left, LineBreak::NoWrap),
        ChildOf(e),
    ));
    e
}

/// Disabled text as VGUI draws it: the second disabled colour a pixel
/// down and right, the first over it.
#[allow(clippy::too_many_arguments)]
pub fn engraved(
    commands: &mut Commands,
    parent: Entity,
    look: &Look,
    (x, y, w, h): (f32, f32, f32, f32),
    text: &str,
    font: TextFont,
    align: i32,
) {
    let under = look.color("Label.DisabledFgColor2", [30, 30, 30, 255]);
    label(
        commands,
        parent,
        look,
        (x + 1.0, y + 1.0, w, h),
        text,
        font.clone(),
        under,
        align,
    );
    label(commands, parent, look, (x, y, w, h), text, font, look.disabled(), align);
}

/// How a frame is drawn: its id, whether it's modal (stacked over the
/// rest, solid), sizeable down to a minimum (scheme pixels), and has a
/// close box.
#[derive(Clone, Copy, Debug)]
pub struct FrameSpec {
    pub id: WindowId,
    pub modal: bool,
    pub sizeable: Option<Vec2>,
    pub close: bool,
}

impl FrameSpec {
    pub fn new(id: WindowId) -> Self {
        Self {
            id,
            modal: false,
            sizeable: None,
            close: true,
        }
    }

    pub fn modal(mut self) -> Self {
        self.modal = true;
        self
    }

    pub fn sizeable(mut self, min: Vec2) -> Self {
        self.sizeable = Some(min);
        self
    }

    pub fn no_close(mut self) -> Self {
        self.close = false;
        self
    }
}

/// The close box of a frame drawn by `frame` (its owner gives it a hit).
pub struct FrameParts {
    pub frame: Entity,
    pub close: Option<Entity>,
}

/// A GameUI frame (`Frame`): its background, raised borders, title, the
/// close box at the top right and, sizeable, the grip at the bottom right;
/// placed (centred the first time, then where it was dragged) and stacked
/// by `place_frames`. Children go in scheme pixels from its corner.
pub fn frame(
    commands: &mut Commands,
    root: Entity,
    look: &Look,
    spec: FrameSpec,
    (w, h): (f32, f32),
    title: &str,
) -> FrameParts {
    let bg = look.frame_bg();
    let bg = if spec.modal {
        // Solid: the dialog under a modal one would show through the
        // scheme's see-through frame colour.
        let c = bg.to_srgba();
        Color::srgb(c.red * 0.7, c.green * 0.7, c.blue * 0.7)
    } else {
        bg
    };
    let e = commands
        .spawn((
            Node {
                border: UiRect::all(px(1.0)),
                ..place(look, -10_000.0 / look.s, 0.0, w, h)
            },
            bevel(look, true),
            // What the frame hides (`FrameBacking`, set by `place_frames`),
            // its see-through colour over it.
            BackgroundColor(Color::NONE),
            VguiFrame {
                id: spec.id,
                size: Vec2::new(w, h),
                scale: look.s,
                sizeable: spec.sizeable,
                modal: spec.modal,
            },
            // Clicks on the frame stop at it (not the dialog under it).
            bevy::ui::FocusPolicy::Block,
            Interaction::default(),
            GlobalZIndex(FRAME_Z),
            ChildOf(root),
        ))
        .id();
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            top: px(0.0),
            width: percent(100.0),
            height: percent(100.0),
            ..default()
        },
        BackgroundColor(bg),
        ChildOf(e),
    ));
    let inset = look.number("Frame.TitleTextInsetX", 16.0);
    label(
        commands,
        e,
        look,
        (inset, 4.0, w - inset - 32.0, 22.0),
        title,
        look.font("UiBold", (12.0, true)),
        look.color("FrameTitleBar.TextColor", [255, 255, 255, 255]),
        -1,
    );
    let close = spec.close.then(|| {
        let b = commands
            .spawn((
                Node {
                    border: UiRect::all(px(1.0)),
                    ..place(look, w - 24.0, 6.0, 18.0, 16.0)
                },
                bevel(look, true),
                BackgroundColor(look.color("Button.BgColor", [0, 0, 0, 0])),
                clickable(),
                ChildOf(e),
            ))
            .id();
        // Marlett's close glyph, else an x.
        label(
            commands,
            b,
            look,
            (0.0, 0.0, 16.0, 14.0),
            "\u{00D7}",
            look.font("Default", (14.0, false)),
            look.color("FrameTitleBar.TextColor", [255, 255, 255, 255]),
            0,
        );
        b
    });
    if spec.sizeable.is_some() {
        // The grip: three short diagonals at the corner.
        let c = look.color("FrameGrip.Color1", [200, 200, 200, 196]);
        let d = look.color("FrameGrip.Color2", [40, 40, 40, 196]);
        for k in 0..3 {
            let o = 4.0 + 4.0 * k as f32;
            for (dx, color) in [(0.0, c), (1.0, d)] {
                commands.spawn((
                    place(look, w - o + dx - 2.0, h - 4.0 - dx, 2.0, 2.0),
                    BackgroundColor(color),
                    ChildOf(e),
                ));
                commands.spawn((
                    place(look, w - 4.0 - dx, h - o + dx - 2.0, 2.0, 2.0),
                    BackgroundColor(color),
                    ChildOf(e),
                ));
            }
        }
    }
    FrameParts { frame: e, close }
}

/// A button's states' colours, kept on it for `style_buttons`.
#[derive(Component, Clone, Copy, Debug)]
pub struct ButtonStyle {
    pub enabled: bool,
    /// Depressed for good (a toggle's on state).
    pub latched: bool,
    pub bg: [Color; 3],
    pub fg: [Color; 3],
    pub raised: BorderColor,
    pub sunken: BorderColor,
    pub text: Entity,
}

/// How a button looks: its text, whether it works, is the dialog's
/// default (Enter presses it) or has the keyboard focus, or stays pressed.
#[derive(Clone, Copy, Debug, Default)]
pub struct ButtonState {
    pub enabled: bool,
    pub default: bool,
    pub focused: bool,
    pub latched: bool,
}

impl ButtonState {
    pub fn enabled(enabled: bool) -> Self {
        Self { enabled, ..default() }
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn default_button(mut self, default: bool) -> Self {
        self.default = default;
        self
    }

    pub fn latched(mut self, latched: bool) -> Self {
        self.latched = latched;
        self
    }
}

/// A VGUI `Button`: raised, its text; armed (hovered) and depressed in the
/// scheme's colours (`style_buttons`), sunken while held; the default
/// button and the focused one ringed; disabled text engraved and no hit.
/// `hit` goes on it when enabled (the owner's click target).
#[allow(clippy::too_many_arguments)]
pub fn button(
    commands: &mut Commands,
    parent: Entity,
    look: &Look,
    (x, y, w, h): (f32, f32, f32, f32),
    text: &str,
    state: ButtonState,
    align: i32,
    hit: impl Bundle,
) -> Entity {
    let bg = [
        look.color("Button.BgColor", [0, 0, 0, 0]),
        look.color("Button.ArmedBgColor", [255, 255, 255, 24]),
        look.color("Button.DepressedBgColor", [0, 0, 0, 64]),
    ];
    let fg = [
        look.color("Button.TextColor", [255, 255, 255, 255]),
        look.color("Button.ArmedTextColor", [255, 255, 255, 255]),
        look.color("Button.DepressedTextColor", [255, 255, 255, 255]),
    ];
    let raised = bevel(look, true);
    let sunken = bevel(look, false);
    let mut e = commands.spawn((
        Node {
            border: UiRect::all(px(1.0)),
            ..place(look, x, y, w, h)
        },
        if state.latched { sunken } else { raised },
        BackgroundColor(if state.latched { bg[2] } else { bg[0] }),
        ChildOf(parent),
    ));
    if state.default || state.focused {
        e.insert(Outline::new(
            px(1.0),
            px(0.0),
            look.color("Button.FocusBorderColor", [0, 0, 0, 255]),
        ));
    }
    let e = e.id();
    let font = look.default_font();
    let text_e = if state.enabled {
        label(
            commands,
            e,
            look,
            (6.0, 0.0, w - 12.0, h - 2.0),
            text,
            font,
            fg[0],
            align,
        )
    } else {
        engraved(commands, e, look, (6.0, 0.0, w - 12.0, h - 2.0), text, font, align);
        e
    };
    if state.enabled {
        commands.entity(e).insert((
            clickable(),
            ButtonStyle {
                enabled: true,
                latched: state.latched,
                bg,
                fg,
                raised,
                sunken,
                text: text_e,
            },
            hit,
        ));
    }
    e
}

/// Buttons as the pointer arms (hovers) and presses them: the scheme's
/// armed and depressed colours, sunken while held.
fn style_buttons(
    mut buttons: Query<(&Interaction, &ButtonStyle, &mut BackgroundColor, &mut BorderColor), Changed<Interaction>>,
    children: Query<&Children>,
    mut texts: Query<&mut TextColor>,
) {
    for (interaction, style, mut bg, mut border) in &mut buttons {
        let state = match interaction {
            _ if style.latched => 2,
            Interaction::None => 0,
            Interaction::Hovered => 1,
            Interaction::Pressed => 2,
        };
        bg.0 = style.bg[state];
        *border = if state == 2 { style.sunken } else { style.raised };
        if let Ok(kids) = children.get(style.text) {
            for k in kids.iter() {
                if let Ok(mut t) = texts.get_mut(k) {
                    t.0 = style.fg[state];
                }
            }
        }
    }
}

/// A `CheckButton`: a sunken box (ticked when on) and its text, the whole
/// of it clicked; greyed when disabled.
#[allow(clippy::too_many_arguments)]
pub fn check_button(
    commands: &mut Commands,
    parent: Entity,
    look: &Look,
    rect: (f32, f32, f32, f32),
    text: &str,
    on: bool,
    enabled: bool,
    focused: bool,
    hit: impl Bundle,
) -> Entity {
    check_like(commands, parent, look, rect, text, (on, enabled, focused, false), hit)
}

/// A `RadioButton`: a check button with a round box, one of a group.
#[allow(clippy::too_many_arguments)]
pub fn radio_button(
    commands: &mut Commands,
    parent: Entity,
    look: &Look,
    rect: (f32, f32, f32, f32),
    text: &str,
    on: bool,
    enabled: bool,
    focused: bool,
    hit: impl Bundle,
) -> Entity {
    check_like(commands, parent, look, rect, text, (on, enabled, focused, true), hit)
}

fn check_like(
    commands: &mut Commands,
    parent: Entity,
    look: &Look,
    (x, y, w, h): (f32, f32, f32, f32),
    text: &str,
    (on, enabled, focused, round): (bool, bool, bool, bool),
    hit: impl Bundle,
) -> Entity {
    let radius = if round { BorderRadius::MAX } else { BorderRadius::ZERO };
    let mut e = commands.spawn((place(look, x, y, w, h), ChildOf(parent)));
    if enabled {
        e.insert((clickable(), hit));
    }
    if focused {
        e.insert(Outline::new(
            px(1.0),
            px(0.0),
            look.color("Border.Dark", [40, 40, 40, 196]),
        ));
    }
    let e = e.id();
    let b = commands
        .spawn((
            Node {
                border: UiRect::all(px(1.0)),
                border_radius: radius,
                ..place(look, 2.0, ((h - 14.0) / 2.0).round(), 14.0, 14.0)
            },
            BorderColor {
                top: look.color("CheckButton.Border1", [40, 40, 40, 196]),
                left: look.color("CheckButton.Border1", [40, 40, 40, 196]),
                bottom: look.color("CheckButton.Border2", [200, 200, 200, 196]),
                right: look.color("CheckButton.Border2", [200, 200, 200, 196]),
            },
            BackgroundColor(look.color("CheckButton.BgColor", [0, 0, 0, 128])),
            ChildOf(e),
        ))
        .id();
    if on {
        let check = if enabled {
            look.color("CheckButton.Check", [255, 255, 255, 255])
        } else {
            look.color("CheckButton.DisabledFgColor", [117, 117, 117, 255])
        };
        // Marlett's tick (a dot for a radio button); a square stands in.
        commands.spawn((
            Node {
                border_radius: radius,
                ..place(look, 3.0, 3.0, 6.0, 6.0)
            },
            BackgroundColor(check),
            ChildOf(b),
        ));
    }
    let font = look.default_font();
    // The words run on past a narrow box (VGUI sizes the label to them).
    let text_w = (look.fonts.char_offsets(&font, text).last().copied().unwrap_or(0.0) / look.s + 4.0).max(w - 22.0);
    if text.is_empty() {
    } else if enabled {
        let color = look.color("CheckButton.TextColor", [255, 255, 255, 255]);
        label(commands, e, look, (22.0, 0.0, text_w, h), text, font, color, -1);
    } else {
        engraved(commands, e, look, (22.0, 0.0, text_w, h), text, font, -1);
    }
    e
}

/// A `ComboBox` closed: a sunken box with the value (left), the arrow
/// button at its right (`ComboBoxButton`, raised; sunken while its list
/// is open). `hit` goes on the whole box (a click opens the list).
#[allow(clippy::too_many_arguments)]
pub fn combo_box(
    commands: &mut Commands,
    parent: Entity,
    look: &Look,
    rect: (f32, f32, f32, f32),
    value: &str,
    enabled: bool,
    open: bool,
    focused: bool,
    hit: impl Bundle,
) -> Entity {
    combo_box_aligned(commands, parent, look, rect, value, enabled, open, focused, -1, hit)
}

/// `combo_box` with its value aligned as its `textAlignment` says: -1
/// west (left), 0 centre, 1 east.
#[allow(clippy::too_many_arguments)]
pub fn combo_box_aligned(
    commands: &mut Commands,
    parent: Entity,
    look: &Look,
    (x, y, w, h): (f32, f32, f32, f32),
    value: &str,
    enabled: bool,
    open: bool,
    focused: bool,
    align: i32,
    hit: impl Bundle,
) -> Entity {
    let mut e = commands.spawn((
        Node {
            border: UiRect::all(px(1.0)),
            ..place(look, x, y, w, h)
        },
        bevel(look, false),
        BackgroundColor(look.sunken_bg()),
        ChildOf(parent),
    ));
    if enabled {
        e.insert((clickable(), hit));
    }
    let e = e.id();
    let font = look.default_font();
    let text_w = w - 22.0;
    if !enabled {
        engraved(commands, e, look, (4.0, 0.0, text_w - 4.0, h - 2.0), value, font, align);
    } else if focused && !open {
        // Focused: the value shown selected, as VGUI's text entry.
        let at = label(
            commands,
            e,
            look,
            (2.0, 2.0, text_w - 2.0, h - 6.0),
            "",
            font.clone(),
            Color::NONE,
            -1,
        );
        commands.entity(at).insert(BackgroundColor(
            look.color("TextEntry.SelectedBgColor", [255, 155, 0, 255]),
        ));
        label(
            commands,
            e,
            look,
            (4.0, 0.0, text_w - 4.0, h - 2.0),
            value,
            font,
            look.color("TextEntry.SelectedTextColor", [0, 0, 0, 255]),
            align,
        );
    } else {
        let color = look.color("TextEntry.TextColor", [221, 221, 221, 255]);
        label(
            commands,
            e,
            look,
            (4.0, 0.0, text_w - 4.0, h - 2.0),
            value,
            font,
            color,
            align,
        );
    }
    let a = commands
        .spawn((
            Node {
                border: UiRect::all(px(1.0)),
                ..place(look, w - 20.0, 1.0, 18.0, h - 4.0)
            },
            bevel(look, !open),
            BackgroundColor(look.color("ComboBoxButton.BgColor", [0, 0, 0, 0])),
            ChildOf(e),
        ))
        .id();
    let arrow = if enabled {
        look.color("ComboBoxButton.ArrowColor", [190, 190, 190, 255])
    } else {
        look.color("ComboBoxButton.DisabledBgColor", [117, 117, 117, 255])
    };
    label(
        commands,
        a,
        look,
        (0.0, 0.0, 16.0, h - 6.0),
        "\u{25BC}",
        look.font("Marlett", (9.0, false)),
        arrow,
        0,
    );
    e
}

/// A combo box's open list (VGUI's `Menu`), under its box at (x, y + h)
/// in `parent`: the entries shown, the highlighted one lit, a scroll bar
/// past `COMBO_ROWS`; each entry gets `hit(i)`; behind it, the whole
/// screen catches a click outside (`outside`).
#[allow(clippy::too_many_arguments)]
pub fn combo_popup<B: Bundle>(
    commands: &mut Commands,
    parent: Entity,
    root: Entity,
    look: &Look,
    (x, y, w, h): (f32, f32, f32, f32),
    entries: &[String],
    list: &ComboList,
    hit: impl Fn(usize) -> B,
    outside: impl Bundle,
) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: percent(100.0),
            height: percent(100.0),
            ..default()
        },
        // Blocking (`clickable`): a click passing through reached the
        // box under it too, which opened the list again as the catcher
        // closed it.
        clickable(),
        outside,
        GlobalZIndex(POPUP_Z - 1),
        ChildOf(root),
    ));
    let row_h = 20.0;
    let rows = list.rows();
    let bar = if entries.len() > rows {
        look.number("ScrollBar.Wide", 17.0)
    } else {
        0.0
    };
    // A solid backing (the scheme's menu colour is see-through), the
    // menu's colour over it.
    let backing = {
        let c = look.frame_bg().to_srgba();
        Color::srgb(c.red * 0.45, c.green * 0.45, c.blue * 0.45)
    };
    let solid = commands
        .spawn((
            place(look, x, y + h, w, rows as f32 * row_h + 2.0),
            BackgroundColor(backing),
            bevy::ui::FocusPolicy::Block,
            GlobalZIndex(POPUP_Z),
            ChildOf(parent),
        ))
        .id();
    let menu = commands
        .spawn((
            Node {
                border: UiRect::all(px(1.0)),
                ..place(look, 0.0, 0.0, w, rows as f32 * row_h + 2.0)
            },
            BorderColor::all(look.color("Menu.BorderColor", [0, 0, 0, 255])),
            BackgroundColor(look.color("Menu.BgColor", [40, 40, 40, 255])),
            ChildOf(solid),
        ))
        .id();
    let font = look.default_font();
    for (k, i) in (list.first..(list.first + rows).min(entries.len())).enumerate() {
        let lit = i == list.highlight;
        let e = commands
            .spawn((
                place(look, 0.0, k as f32 * row_h, w - 2.0 - bar, row_h),
                BackgroundColor(if lit {
                    look.color("Menu.ArmedBgColor", [255, 155, 0, 255])
                } else {
                    Color::NONE
                }),
                clickable(),
                hit(i),
                ChildOf(menu),
            ))
            .id();
        let color = if lit {
            look.color("Menu.ArmedTextColor", [0, 0, 0, 255])
        } else {
            look.color("Menu.TextColor", [221, 221, 221, 255])
        };
        label(
            commands,
            e,
            look,
            (4.0, 0.0, w - 10.0 - bar, row_h),
            &entries[i],
            font.clone(),
            color,
            -1,
        );
    }
    if bar > 0.0 {
        let track = commands
            .spawn((
                place(look, w - 2.0 - bar, 0.0, bar, rows as f32 * row_h),
                BackgroundColor(look.sunken_bg()),
                ChildOf(menu),
            ))
            .id();
        let room = rows as f32 * row_h;
        if let Some((top, thumb)) = scroll_thumb(entries.len(), rows, list.first, room) {
            // Dragged, it scrolls the list (`COMBO_SCROLL`).
            commands.spawn((
                Node {
                    border: UiRect::all(px(1.0)),
                    ..place(look, 1.0, top, bar - 2.0, thumb)
                },
                bevel(look, true),
                BackgroundColor(look.color("ScrollBarSlider.BgColor", [255, 255, 255, 64])),
                ScrollThumb {
                    owner: COMBO_SCROLL,
                    id: list.owner,
                    len: entries.len(),
                    shown: rows,
                    room,
                    top,
                    thumb,
                },
                clickable(),
                ChildOf(track),
            ));
        }
    }
}

/// A `Slider`: its sunken track, the nob at `fraction`, `ticks` marks
/// under it (VGUI's `numTicks`; none at 0); pressing or dragging along it
/// sets the value (`SliderDrag`).
#[allow(clippy::too_many_arguments)]
pub fn slider(
    commands: &mut Commands,
    parent: Entity,
    look: &Look,
    (x, y, w, h): (f32, f32, f32, f32),
    fraction: f32,
    ticks: usize,
    enabled: bool,
    track: SliderTrack,
) -> Entity {
    const NOB: f32 = 8.0;
    let mut e = commands.spawn((place(look, x, y, w, h), ChildOf(parent)));
    if enabled {
        e.insert((
            SliderTrack { inset: NOB, ..track },
            clickable(),
            bevy::ui::RelativeCursorPosition::default(),
        ));
    }
    let e = e.id();
    let mid = (h / 2.0 - 4.0).round();
    commands.spawn((
        Node {
            border: UiRect::all(px(1.0)),
            ..place(look, NOB / 2.0, mid, w - NOB, 4.0)
        },
        bevel(look, false),
        BackgroundColor(look.color("Slider.TrackColor", [31, 31, 31, 255])),
        ChildOf(e),
    ));
    if ticks > 1 {
        let tick = look.color("Slider.TextColor", [127, 140, 127, 255]);
        for k in 0..ticks {
            let tx = NOB / 2.0 + (w - NOB) * k as f32 / (ticks - 1) as f32;
            commands.spawn((
                place(look, tx.round() - 0.5, mid + 9.0, 1.0, 4.0),
                BackgroundColor(tick),
                ChildOf(e),
            ));
        }
    }
    let nob_x = (w - NOB) * fraction.clamp(0.0, 1.0);
    commands.spawn((
        Node {
            border: UiRect::all(px(1.0)),
            ..place(look, nob_x, mid - 6.0, NOB, 16.0)
        },
        bevel(look, true),
        BackgroundColor(if enabled {
            look.color("Slider.NobColor", [108, 108, 108, 255])
        } else {
            look.color("Slider.DisabledTextColor1", [117, 117, 117, 255])
        }),
        ChildOf(e),
    ));
    e
}

/// A `TextEntry`: sunken, its text (dots for a password), scrolled so the
/// caret shows; focused: the selection highlighted and the caret drawn.
/// `hit` goes on it (a click focuses it and places the caret).
#[allow(clippy::too_many_arguments)]
pub fn text_entry(
    commands: &mut Commands,
    parent: Entity,
    look: &Look,
    (x, y, w, h): (f32, f32, f32, f32),
    text: &str,
    caret: Option<Caret>,
    hidden: bool,
    enabled: bool,
    hit: impl Bundle,
) -> Entity {
    let mut e = commands.spawn((
        Node {
            border: UiRect::all(px(1.0)),
            overflow: Overflow::clip(),
            ..place(look, x, y, w, h)
        },
        bevel(look, false),
        BackgroundColor(look.sunken_bg()),
        ChildOf(parent),
    ));
    if enabled {
        e.insert((
            clickable(),
            bevy::ui::RelativeCursorPosition::default(),
            hit,
        ));
    }
    let e = e.id();
    let shown: String = if hidden {
        "*".repeat(text.chars().count())
    } else {
        text.to_string()
    };
    let font = look.default_font();
    let offsets = look.fonts.char_offsets(&font, &shown);
    // Scheme pixels of the text before char i.
    let at = |i: usize| offsets.get(i).copied().unwrap_or(0.0) / look.s;
    let inner = w - 8.0;
    let scroll = match caret {
        Some(c) => (at(c.at) - inner + 2.0).max(0.0),
        None => 0.0,
    };
    let color = if enabled {
        look.color("TextEntry.TextColor", [221, 221, 221, 255])
    } else {
        look.disabled()
    };
    if let Some(c) = caret.filter(|c| c.at != c.anchor) {
        let r = c.range();
        let (sx, ex) = (at(r.start) - scroll, at(r.end) - scroll);
        commands.spawn((
            place(look, 4.0 + sx, 3.0, (ex - sx).max(1.0), h - 8.0),
            BackgroundColor(look.color("TextEntry.SelectedBgColor", [255, 155, 0, 255])),
            ChildOf(e),
        ));
    }
    label(
        commands,
        e,
        look,
        (4.0 - scroll, 0.0, inner + scroll, h - 2.0),
        &shown,
        font,
        color,
        -1,
    );
    if let Some(c) = caret {
        commands.spawn((
            place(look, 4.0 + at(c.at) - scroll, 4.0, 1.0, h - 10.0),
            BackgroundColor(look.color("TextEntry.CursorColor", [255, 255, 255, 255])),
            ChildOf(e),
        ));
    }
    e
}

/// The char a click at `x` (0 to 1 across a text entry `w` scheme pixels
/// wide) lands at, the entry's text scrolled as `text_entry` draws it.
pub fn caret_from_click(
    look_fonts: &UiFonts,
    font: &TextFont,
    scale: f32,
    shown: &str,
    w: f32,
    fraction: f32,
    caret: Option<Caret>,
) -> usize {
    let offsets = look_fonts.char_offsets(font, shown);
    let at = |i: usize| offsets.get(i).copied().unwrap_or(0.0) / scale;
    let inner = w - 8.0;
    let scroll = caret.map_or(0.0, |c| (at(c.at) - inner + 2.0).max(0.0));
    let x = (fraction * w - 4.0 + scroll) * scale;
    char_at(&offsets, x)
}

/// A property sheet's tabs (`PropertySheet`): raised, the open one joined
/// to the page under it; each gets `hit(i)` (`enabled` false: greyed and
/// no hit). `widths`: each tab's width, else fitted to its text.
#[allow(clippy::too_many_arguments)]
pub fn tabs<B: Bundle>(
    commands: &mut Commands,
    parent: Entity,
    look: &Look,
    (x, y): (f32, f32),
    names: &[(String, bool)],
    open: usize,
    width: Option<f32>,
    fit: Option<f32>,
    hit: impl Fn(usize) -> B,
) {
    let font = look.default_font();
    let text: Vec<f32> = names
        .iter()
        .map(|(name, _)| look.fonts.char_offsets(&font, name).last().copied().unwrap_or(0.0) / look.s)
        .collect();
    let widths = tab_widths(&text, width, fit);
    let mut tx = x;
    for (i, (name, enabled)) in names.iter().enumerate() {
        let tab_w = widths[i];
        let is_open = i == open;
        let (ty, th) = if is_open { (y, 27.0) } else { (y + 3.0, 24.0) };
        let mut e = commands.spawn((
            Node {
                border: UiRect {
                    left: px(1.0),
                    right: px(1.0),
                    top: px(1.0),
                    bottom: px(0.0),
                },
                ..place(look, tx, ty, tab_w - 2.0, th)
            },
            bevel(look, true),
            BackgroundColor(if is_open { look.frame_bg() } else { Color::NONE }),
            ZIndex(if is_open { 2 } else { 0 }),
            ChildOf(parent),
        ));
        if *enabled {
            e.insert((clickable(), hit(i)));
        }
        let e = e.id();
        let color = if is_open {
            look.color("PropertySheet.SelectedTextColor", [255, 255, 255, 255])
        } else {
            look.color("PropertySheet.TextColor", [221, 221, 221, 255])
        };
        if *enabled {
            label(
                commands,
                e,
                look,
                (0.0, 0.0, tab_w - 4.0, th - 1.0),
                name,
                font.clone(),
                color,
                0,
            );
        } else {
            engraved(
                commands,
                e,
                look,
                (0.0, 0.0, tab_w - 4.0, th - 1.0),
                name,
                font.clone(),
                0,
            );
        }
        tx += tab_w;
    }
}

/// Each tab's width in scheme pixels: `width` each, else its words'
/// width (`text`) plus 12 on each side, at least 56; tabs that would
/// reach past `fit` (the sheet's width) shrink in proportion so the last
/// one ends inside it.
pub fn tab_widths(text: &[f32], width: Option<f32>, fit: Option<f32>) -> Vec<f32> {
    let natural: Vec<f32> = text
        .iter()
        .map(|t| width.unwrap_or_else(|| (t + 24.0).max(56.0).round()))
        .collect();
    let total: f32 = natural.iter().sum();
    match fit {
        Some(fit) if total > fit && total > 0.0 => natural.iter().map(|w| (w * fit / total).floor()).collect(),
        _ => natural,
    }
}

/// The wheel's notches this frame (up positive).
pub fn wheel_notches(scroll: &bevy::input::mouse::AccumulatedMouseScroll) -> i32 {
    use bevy::input::mouse::MouseScrollUnit;
    let notches = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y.round(),
        MouseScrollUnit::Pixel => (scroll.delta.y / 40.0).round(),
    };
    notches as i32
}

/// Bevy's UI as the game runs it (layout, stacking, focus: what the
/// pointer presses) in a 400x300 window, with `block_clicks`, the
/// pointer driven by `PointerScript`; nothing drawn. For tests of what a
/// press reaches through the menus' stacking (here and the games' menus).
pub fn pointer_test_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        bevy::image::ImagePlugin::default(),
        bevy::input::InputPlugin,
        bevy::window::WindowPlugin {
            primary_window: None,
            exit_condition: bevy::window::ExitCondition::DontExit,
            close_when_requested: false,
            ..default()
        },
        TransformPlugin,
        bevy::text::TextPlugin,
        bevy::ui::UiPlugin,
    ))
    // What the UI's picking backend reads (the focus system is what
    // the menus use).
    .add_plugins((bevy::picking::PickingPlugin, bevy::picking::InteractionPlugin))
    .init_asset::<bevy::image::TextureAtlasLayout>()
    // Hidden nodes take no clicks: the focus system reads visibility.
    .add_plugins(bevy::camera::visibility::VisibilityPlugin)
    .init_asset::<Mesh>()
    .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
    .init_resource::<PointerScript>()
    .add_observer(block_clicks)
    .add_systems(PreUpdate, run_pointer_script.before(bevy::input::InputSystems));
    let mut window = Window::default();
    window.resolution.set(400.0, 300.0);
    app.world_mut().spawn((window, PrimaryWindow));
    let size = UVec2::new(400, 300);
    app.world_mut().spawn((
        Camera2d,
        Camera {
            computed: bevy::camera::ComputedCameraValues {
                target_info: Some(bevy::camera::RenderTargetInfo {
                    physical_size: size,
                    scale_factor: 1.0,
                }),
                ..default()
            },
            viewport: Some(bevy::camera::Viewport {
                physical_size: size,
                ..default()
            }),
            ..default()
        },
    ));
    app
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Vec2 = Vec2::new(1280.0, 720.0);

    /// A shade's alpha covers in linear light (Bevy's UI blending) as
    /// the asked alpha does in display space: black at 0.55 over a
    /// display-space grey leaves about 45% of it (the sRGB curve is not a
    /// pure 2.2 power: within 0.03), not ~70%; opaque stays opaque.
    #[test]
    fn display_alpha_darkens_as_a_display_space_blend_would() {
        let over = |under: f32, shade: Color| {
            let a = shade.alpha();
            let lin = LinearRgba::from(Color::srgb(under, under, under)).red * (1.0 - a);
            Srgba::from(LinearRgba::rgb(lin, lin, lin)).red
        };
        let shade = Color::srgba(0.0, 0.0, 0.0, 0.55);
        for under in [0.2, 0.5, 0.9] {
            assert!((over(under, display_alpha(shade)) - under * 0.45).abs() < 0.03, "{under}");
            assert!(over(under, shade) > under * 0.6, "Bevy's own blend: barely darker");
        }
        assert_eq!(display_alpha(Color::BLACK).alpha(), 1.0);
        assert_eq!(display_alpha(Color::srgba(0.0, 0.0, 0.0, 0.0)).alpha(), 0.0);
    }

    /// A scroll bar's thumb: sized for the rows shown, placed for the
    /// first; dragged, its travel maps onto every first row there is, the
    /// ends past it clamped; none when everything shows.
    #[test]
    fn a_scroll_bars_thumb_maps_its_travel_onto_the_rows() {
        let (len, shown, room) = (100, 10, 200.0);
        assert_eq!(scroll_thumb(len, shown, 0, room), Some((0.0, 20.0)));
        assert_eq!(scroll_thumb(len, shown, 90, room), Some((180.0, 20.0)));
        assert_eq!(scroll_thumb(len, shown, 45, room), Some((90.0, 20.0)));
        assert_eq!(scroll_thumb(5, shown, 0, room), None);
        // A long list: no thinner than 10.
        assert_eq!(scroll_thumb(10_000, shown, 0, room).map(|t| t.1), Some(10.0));
        assert_eq!(scroll_first_at(len, shown, room, 20.0, 90.0), 45);
        assert_eq!(scroll_first_at(len, shown, room, 20.0, -50.0), 0);
        assert_eq!(scroll_first_at(len, shown, room, 20.0, 500.0), 90);
        assert_eq!(scroll_clamp(-3, len, shown), 0);
        assert_eq!(scroll_clamp(95, len, shown), 90);
        // Held at its top, drawn at 1.5 window pixels per scheme pixel:
        // the pointer 45 window pixels down is 30 scheme pixels: 15 rows.
        let held = ScrollHeld {
            thumb: ScrollThumb {
                owner: "menu",
                id: 0,
                len,
                shown,
                room,
                top: 0.0,
                thumb: 20.0,
            },
            grab: 300.0,
            scale: 1.5,
        };
        assert_eq!(held.first_at(300.0), 0);
        assert_eq!(held.first_at(345.0), 15);
        assert_eq!(held.first_at(1000.0), 90);
    }

    /// An open drop-down's catcher (the screen behind its list) takes a
    /// click whole: nothing under it, its own box included, sees the
    /// press (else a click on the box closed the list and opened it
    /// again).
    #[test]
    fn an_open_lists_catcher_blocks_the_click() {
        let mut app = App::new();
        let root = app.world_mut().spawn(Node::default()).id();
        let parent = app.world_mut().spawn(Node::default()).id();
        let mut commands_queue = bevy::ecs::world::CommandQueue::default();
        let mut commands = Commands::new(&mut commands_queue, app.world());
        let fonts = UiFonts::default();
        let look = Look {
            ui: None,
            fonts: &fonts,
            s: 1.0,
            height: 720.0,
            accent: Color::WHITE,
        };
        #[derive(Component)]
        struct Outside;
        let list = ComboList::open(0, 3, 0);
        let entries = ["a".to_string(), "b".into(), "c".into()];
        combo_popup(&mut commands, parent, root, &look, (0.0, 0.0, 100.0, 24.0), &entries, &list, |_| (), Outside);
        commands_queue.apply(app.world_mut());
        let mut q = app.world_mut().query_filtered::<&bevy::ui::FocusPolicy, With<Outside>>();
        assert_eq!(q.single(app.world()).ok(), Some(&bevy::ui::FocusPolicy::Block));
    }


    /// Who a press reached.
    #[derive(Component)]
    struct Clicked(&'static str);

    /// A UI spawned through `Commands` as the menus do, in the scheme's
    /// look at 1:1 (rects are window pixels) on a root filling the
    /// window; then a press at `at`: the `Clicked` nodes it reached,
    /// sorted, and whether a scroll bar's thumb took it.
    fn press(at: Vec2, ui: impl FnOnce(&mut Commands, &Look, Entity)) -> (Vec<&'static str>, bool) {
        let mut app = pointer_test_app();
        let fonts = UiFonts::default();
        let look = Look {
            ui: None,
            fonts: &fonts,
            s: 1.0,
            height: 300.0,
            accent: Color::WHITE,
        };
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let mut commands = Commands::new(&mut queue, app.world());
        let root = commands
            .spawn(Node {
                width: percent(100.0),
                height: percent(100.0),
                ..default()
            })
            .id();
        ui(&mut commands, &look, root);
        queue.apply(app.world_mut());
        // Laid out first, then the cursor there, then the press.
        app.update();
        app.update();
        let mut script = PointerScript::default();
        script.0.push_back(PointerStep { at, button: None });
        script.0.push_back(PointerStep { at, button: Some(true) });
        app.insert_resource(script);
        app.update();
        app.update();
        let mut q = app.world_mut().query::<(&Clicked, &Interaction)>();
        let mut hit: Vec<_> = q
            .iter(app.world())
            .filter(|(_, i)| **i == Interaction::Pressed)
            .map(|(c, _)| c.0)
            .collect();
        hit.sort();
        let mut q = app.world_mut().query::<(&ScrollThumb, &Interaction)>();
        let thumb = q.iter(app.world()).any(|(_, i)| *i == Interaction::Pressed);
        (hit, thumb)
    }

    /// A button drawn over another clickable takes the click alone: the
    /// one under it (here a list row) never sees the press. Before
    /// `clickable`, `Node` ahead of `Button` left every button `Pass`, and
    /// both were pressed.
    #[test]
    fn a_button_over_another_takes_the_click_alone() {
        let ui = |commands: &mut Commands, look: &Look, root: Entity| {
            let row = commands
                .spawn((place(look, 10.0, 10.0, 300.0, 40.0), clickable(), Clicked("row"), ChildOf(root)))
                .id();
            let state = ButtonState::enabled(true);
            button(commands, row, look, (200.0, 5.0, 80.0, 30.0), "OK", state, 0, Clicked("button"));
        };
        assert_eq!(press(Vec2::new(250.0, 30.0), ui).0, ["button"]);
        // Beside the button: the row.
        assert_eq!(press(Vec2::new(50.0, 30.0), ui).0, ["row"]);
    }

    /// A scroll bar laid over a list's right edge: its thumb and arrows
    /// take a press without the row under them seeing it (a drag of the
    /// thumb picked the row too).
    #[test]
    fn a_scroll_bars_thumb_over_a_row_takes_the_click_alone() {
        let ui = |commands: &mut Commands, look: &Look, root: Entity| {
            commands.spawn((place(look, 0.0, 0.0, 300.0, 220.0), clickable(), Clicked("row"), ChildOf(root)));
            let bar = scroll_bar(commands, root, look, (283.0, 0.0, 220.0), (100, 10, 0), ("menu", 0), |step| {
                Clicked(if step == 1 { "arrow" } else { "other" })
            });
            commands.entity(bar).insert(ZIndex(1));
        };
        // The thumb: the first rows shown, at the bar's top past its arrow.
        assert_eq!(press(Vec2::new(290.0, 25.0), ui), (vec![], true));
        // The down arrow at its bottom.
        assert_eq!(press(Vec2::new(290.0, 212.0), ui), (vec!["arrow"], false));
        // Off the bar: the row.
        assert_eq!(press(Vec2::new(100.0, 100.0), ui), (vec!["row"], false));
    }

    /// A frame's close box takes the click (not the frame under it); a
    /// frame stops a click on its blank face from reaching the menu row
    /// under the dialog.
    #[test]
    fn a_frame_and_its_close_box_stop_the_click() {
        let ui = |commands: &mut Commands, look: &Look, root: Entity| {
            commands.spawn((place(look, 0.0, 0.0, 400.0, 300.0), clickable(), Clicked("menu row"), ChildOf(root)));
            let parts = frame(commands, root, look, FrameSpec::new("test"), (210.0, 200.0), "Test");
            // `place_frames` puts it on screen; here at the corner.
            commands.entity(parts.frame).insert(Clicked("frame")).entry::<Node>().and_modify(|mut n| {
                n.left = px(0.0);
                n.top = px(0.0);
            });
            commands.entity(parts.close.unwrap()).insert(Clicked("close"));
        };
        assert_eq!(press(Vec2::new(195.0, 14.0), ui).0, ["close"]);
        assert_eq!(press(Vec2::new(100.0, 100.0), ui).0, ["frame"]);
        assert_eq!(press(Vec2::new(300.0, 250.0), ui).0, ["menu row"]);
    }

    /// Anything with an `Interaction` stops the pointer however it was
    /// spawned (`block_clicks`): `Node` first and the `Interaction`
    /// inserted after included; `PassClicks` lets the press through as well.
    #[test]
    fn an_interaction_blocks_unless_it_passes_clicks() {
        for (pass, want) in [(false, vec!["top"]), (true, vec!["top", "under"])] {
            let ui = |commands: &mut Commands, look: &Look, root: Entity| {
                commands.spawn((place(look, 0.0, 0.0, 100.0, 100.0), Clicked("under"), ChildOf(root))).insert(Interaction::default());
                let mut top = commands.spawn((place(look, 0.0, 0.0, 50.0, 50.0), ChildOf(root)));
                top.insert((Interaction::default(), Clicked("top")));
                if pass {
                    top.insert(PassClicks);
                }
            };
            assert_eq!(press(Vec2::new(20.0, 20.0), ui).0, want, "PassClicks {pass}");
        }
    }

    /// `ui_click` and `ui_drag`: the cursor there, the button pressed and
    /// let go as Bevy's input messages, one step a frame.
    #[test]
    fn scripted_pointer_clicks_and_drags() {
        let mut app = App::new();
        app.add_plugins(bevy::input::InputPlugin)
            .init_resource::<PointerScript>()
            .add_systems(PreUpdate, run_pointer_script.before(bevy::input::InputSystems));
        app.world_mut().spawn((Window::default(), PrimaryWindow));
        app.world_mut()
            .resource_mut::<PointerScript>()
            .drag(Vec2::new(10.0, 20.0), Vec2::new(10.0, 60.0), 4);
        let mut seen = Vec::new();
        for _ in 0..12 {
            app.update();
            let down = app.world().resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Left);
            let mut q = app.world_mut().query::<&Window>();
            let at = q.single(app.world()).unwrap().cursor_position();
            seen.push((at.map(|p| p.y), down));
        }
        assert_eq!(seen[0], (Some(20.0), false));
        assert_eq!(seen[1], (Some(20.0), true), "pressed");
        assert!(seen[2..8].iter().all(|(_, down)| *down), "held while it moves");
        assert_eq!(seen[7], (Some(60.0), true));
        assert_eq!(seen[8], (Some(60.0), false), "let go at the end");
        assert!(app.world().resource::<PointerScript>().0.is_empty());
    }

    /// A dialog's frame stacks over the game menu's layer and hides what
    /// of the menu is under it: drawn over the main menu's picture (the
    /// part under the frame), or over black in a game; no menu: see-through.
    #[test]
    fn frames_draw_over_the_menu_and_hide_it() {
        let mut app = App::new();
        app.init_resource::<Windows>()
            .init_resource::<FrameBacking>()
            .add_systems(Update, place_frames);
        let mut window = Window::default();
        window.resolution.set(1280.0, 720.0);
        app.world_mut().spawn((window, PrimaryWindow));
        let frame = app
            .world_mut()
            .spawn((
                VguiFrame {
                    id: "options",
                    size: Vec2::new(512.0, 406.0),
                    scale: 1.0,
                    sizeable: None,
                    modal: false,
                },
                Node::default(),
                GlobalZIndex(FRAME_Z),
                BackgroundColor(Color::NONE),
            ))
            .id();
        let picture = Handle::<Image>::default();
        app.world_mut().resource_mut::<FrameBacking>().0 = Some(Backing::Picture(picture.clone(), Vec2::new(2560.0, 1440.0)));
        app.update();
        app.update();
        let e = app.world().entity(frame);
        assert!(e.get::<GlobalZIndex>().unwrap().0 >= FRAME_Z);
        assert_eq!(e.get::<BackgroundColor>().unwrap().0, Color::BLACK, "opaque under the picture");
        let image = e.get::<ImageNode>().expect("the picture under it");
        // Centred: (384, 157) to (896, 563), 1 px in, at twice the size.
        let rect = image.rect.unwrap();
        assert_eq!((rect.min, rect.max), (Vec2::new(770.0, 316.0), Vec2::new(1790.0, 1124.0)));
        // In a game: over the world drawn again, dimmed.
        let world = Handle::<Image>::default();
        app.insert_resource(WorldPicture(Some((world.clone(), Vec2::new(1280.0, 720.0)))));
        app.world_mut().resource_mut::<FrameBacking>().0 = Some(Backing::World(Color::BLACK));
        app.update();
        app.update();
        let e = app.world().entity(frame);
        let image = e.get::<ImageNode>().expect("the world under it");
        let tint = image.color.to_srgba();
        assert_eq!((tint.red, tint.alpha), (WORLD_UNDER_DIALOGS, 1.0), "dimmed, opaque: the menu under it hidden");
        assert_eq!(image.rect.unwrap().min, Vec2::new(385.0, 158.0));
        // Before the world's picture exists: over black.
        app.insert_resource(WorldPicture(None));
        app.update();
        app.update();
        let e = app.world().entity(frame);
        assert!(e.get::<ImageNode>().is_none());
        assert_eq!(e.get::<BackgroundColor>().unwrap().0, Color::BLACK);
        // No menu: see-through as the scheme says.
        app.world_mut().resource_mut::<FrameBacking>().0 = None;
        app.update();
        assert_eq!(app.world().entity(frame).get::<BackgroundColor>().unwrap().0, Color::NONE);
    }

    #[test]
    fn tabs_are_as_wide_as_their_words_and_stay_on_the_sheet() {
        // The options' six tabs at their words' widths (Tahoma-like).
        let words = [52.0, 38.0, 34.0, 33.0, 32.0, 70.0];
        let w = tab_widths(&words, None, Some(496.0));
        assert_eq!(w, [76.0, 62.0, 58.0, 57.0, 56.0, 94.0], "words plus 24, at least 56");
        // Wider than the sheet (the 96 each the options drew before: 576
        // in 496): shrunk so the last ends inside it.
        let w = tab_widths(&words, Some(96.0), Some(496.0));
        assert!(w.iter().sum::<f32>() <= 496.0, "{w:?}");
        assert!(w.iter().all(|w| *w > 80.0));
        assert_eq!(tab_widths(&[200.0], None, None), [224.0], "no sheet: as wide as it needs");
    }

    #[test]
    fn a_frame_opens_centred_then_stays_where_it_was_dragged() {
        let mut w = Windows::default();
        let size = Vec2::new(400.0, 300.0);
        assert_eq!(w.place("options", size, SCREEN), Vec2::new(440.0, 210.0));
        // Dragged by its title bar 100 right, 50 up.
        w.begin("options", None, Vec2::new(500.0, 220.0), 1.0, Vec2::ZERO);
        let p = w.drag_to(Vec2::new(600.0, 170.0), SCREEN).unwrap();
        assert_eq!(p.pos, Vec2::new(540.0, 160.0));
        w.end_drag();
        // Drawn again (closed and opened): where it was left.
        assert_eq!(w.place("options", size, SCREEN), Vec2::new(540.0, 160.0));
    }

    #[test]
    fn a_dragged_frame_stays_on_screen() {
        let mut w = Windows::default();
        let size = Vec2::new(400.0, 300.0);
        w.place("servers", size, SCREEN);
        w.begin("servers", None, Vec2::new(640.0, 220.0), 1.0, Vec2::ZERO);
        let p = w.drag_to(Vec2::new(-5000.0, 9000.0), SCREEN).unwrap();
        assert_eq!(p.pos, Vec2::new(0.0, 420.0), "left edge and bottom edge");
        let p = w.drag_to(Vec2::new(9000.0, -9000.0), SCREEN).unwrap();
        assert_eq!(p.pos, Vec2::new(880.0, 0.0));
        // A smaller window later: pulled back on.
        w.end_drag();
        assert_eq!(w.place("servers", size, Vec2::new(800.0, 600.0)), Vec2::new(400.0, 0.0));
        // Bigger than the screen: its top-left corner shows.
        assert_eq!(
            w.place("servers", Vec2::new(900.0, 700.0), Vec2::new(800.0, 600.0)),
            Vec2::ZERO
        );
    }

    #[test]
    fn a_clicked_frame_comes_to_the_front() {
        let mut w = Windows::default();
        for id in ["options", "servers", "createserver"] {
            w.place(id, Vec2::splat(100.0), SCREEN);
        }
        assert_eq!(w.front(&["options", "servers"]), Some("servers"));
        w.raise("options");
        assert_eq!(w.front(&["options", "servers", "createserver"]), Some("options"));
        assert!(w.rank("options") > w.rank("createserver"));
        // Dragging raises too.
        w.begin("servers", None, Vec2::ZERO, 1.0, Vec2::ZERO);
        assert_eq!(w.front(&["options", "servers"]), Some("servers"));
        // Not shown yet: in front when it comes.
        assert_eq!(w.rank("advanced"), 3);
    }

    #[test]
    fn a_sizeable_frame_resizes_from_its_edges_down_to_its_minimum() {
        let mut w = Windows::default();
        w.place("servers", Vec2::new(624.0, 384.0), SCREEN);
        let p = w.placed("servers").unwrap();
        let corner = p.pos + p.size - Vec2::splat(2.0);
        let edges = edges_at(p, corner, GRIP, CORNER);
        assert!(edges.right && edges.bottom && !edges.left && !edges.top);
        assert!(
            !edges_at(p, p.pos + p.size / 2.0, GRIP, CORNER).any(),
            "the middle doesn't resize"
        );
        // Scheme scale 2: window pixels are twice scheme pixels.
        w.begin("servers", Some(edges), corner, 2.0, Vec2::new(200.0, 150.0));
        let before = w.resized;
        w.drag_to(corner + Vec2::new(100.0, 40.0), SCREEN);
        assert_eq!(w.size("servers"), Some(Vec2::new(362.0, 212.0)));
        assert!(w.resized > before, "the dialog draws again");
        // Not under the minimum.
        w.drag_to(corner - Vec2::splat(5000.0), SCREEN);
        assert_eq!(w.size("servers"), Some(Vec2::new(200.0, 150.0)));
        // From the left edge: the right edge stays.
        w.end_drag();
        let p = w.placed("servers").unwrap();
        let left = Vec2::new(p.pos.x + 1.0, p.pos.y + p.size.y / 2.0);
        let e = edges_at(p, left, GRIP * 2.0, CORNER * 2.0);
        assert!(e.left && !e.right);
        w.begin("servers", Some(e), left, 2.0, Vec2::new(200.0, 150.0));
        let q = w.drag_to(left - Vec2::new(50.0, 0.0), SCREEN).unwrap();
        assert_eq!(q.pos.x + q.size.x, p.pos.x + p.size.x);
        assert_eq!(q.size.x, p.size.x + 50.0);
    }

    #[test]
    fn a_combo_list_opens_highlights_picks_and_closes() {
        let mut l = ComboList::open(3, 25, 12);
        assert_eq!((l.highlight, l.rows()), (12, COMBO_ROWS));
        assert!(l.first <= 12 && 12 < l.first + COMBO_ROWS, "the selection shows");
        l.hover(14);
        assert_eq!(l.highlight, 14);
        l.hover(99);
        assert_eq!(l.highlight, 14, "past the end: no change");
        assert_eq!(l.key(ComboKey::Down), ComboEvent::Open);
        assert_eq!(l.key(ComboKey::Enter), ComboEvent::Pick(15));
        assert_eq!(l.key(ComboKey::Escape), ComboEvent::Close);
        assert_eq!(l.key(ComboKey::End), ComboEvent::Open);
        assert_eq!((l.highlight, l.first), (24, 15));
        assert_eq!(l.key(ComboKey::Home), ComboEvent::Open);
        assert_eq!((l.highlight, l.first), (0, 0));
        l.key(ComboKey::Up);
        assert_eq!(l.highlight, 0, "no wrapping");
        l.wheel(100);
        assert_eq!(l.first, 15);
        l.wheel(-3);
        assert_eq!(l.first, 12);
        // A short list shows all its entries.
        let s = ComboList::open(0, 3, 0);
        assert_eq!(s.rows(), 3);
        assert_eq!(ComboList::open(0, 0, 0).clone().key(ComboKey::Down), ComboEvent::Close);
    }

    #[test]
    fn a_closed_combo_steps_without_wrapping() {
        assert_eq!(combo_step(0, 4, -1), 0);
        assert_eq!(combo_step(2, 4, 1), 3);
        assert_eq!(combo_step(3, 4, 1), 3);
        assert_eq!(combo_step(0, 0, 1), 0);
    }

    #[test]
    fn a_slider_drag_maps_the_pointer_along_its_track() {
        assert_eq!(slider_fraction(100.0, 100.0, 200.0), 0.0);
        assert_eq!(slider_fraction(200.0, 100.0, 200.0), 0.5);
        assert_eq!(slider_fraction(900.0, 100.0, 200.0), 1.0, "past the end: the end");
        assert_eq!(slider_fraction(0.0, 100.0, 200.0), 0.0);
        assert_eq!(slider_fraction(5.0, 0.0, 0.0), 0.0);
    }

    #[test]
    fn text_entry_editing() {
        let mut t = String::from("de_dust");
        let mut c = Caret::end_of(&t);
        c.edit(&mut t, Edit::Insert("2".into()), 64);
        assert_eq!((t.as_str(), c.at), ("de_dust2", 8));
        // Shift+Home selects to the start; typing replaces it.
        c.edit(&mut t, Edit::Home(true), 64);
        assert_eq!(c.selected(&t), "de_dust2");
        c.edit(&mut t, Edit::Insert("cs_".into()), 64);
        assert_eq!((t.as_str(), c), ("cs_", Caret { at: 3, anchor: 3 }));
        // Left with a selection goes to its start; Backspace and Delete.
        t = "abcdef".into();
        c = Caret { at: 4, anchor: 2 };
        assert_eq!(c.selected(&t), "cd");
        c.edit(&mut t, Edit::Left(false), 64);
        assert_eq!(c, Caret { at: 2, anchor: 2 });
        assert!(c.edit(&mut t, Edit::Backspace, 64));
        assert_eq!((t.as_str(), c.at), ("acdef", 1));
        assert!(c.edit(&mut t, Edit::Delete, 64));
        assert_eq!(t, "adef");
        c.edit(&mut t, Edit::Home(false), 64);
        assert!(!c.edit(&mut t, Edit::Backspace, 64), "nothing before the caret");
        c.edit(&mut t, Edit::SelectAll, 64);
        assert_eq!(c.selected(&t), "adef");
        c.edit(&mut t, Edit::Delete, 64);
        assert_eq!(t, "");
        // Control characters dropped, the limit kept, chars not bytes.
        c.edit(&mut t, Edit::Insert("é\u{7}x\nyz".into()), 4);
        assert_eq!((t.as_str(), c.at), ("éxy", 3));
        c.edit(&mut t, Edit::To(1, false), 64);
        c.edit(&mut t, Edit::Right(true), 64);
        assert_eq!(c.selected(&t), "x");
        assert_eq!(c.range(), 1..2);
    }

    #[test]
    fn a_click_lands_on_the_nearest_char_boundary() {
        let offsets = [0.0, 7.0, 14.0, 21.0];
        assert_eq!(char_at(&offsets, 2.0), 0);
        assert_eq!(char_at(&offsets, 4.0), 1);
        assert_eq!(char_at(&offsets, 100.0), 3);
        assert_eq!(char_at(&[], 3.0), 0);
    }

    #[test]
    fn tab_and_shift_tab_walk_the_focus_order() {
        let order = ['a', 'b', 'c'];
        assert_eq!(focus_step(&order, None, 1), Some('a'));
        assert_eq!(focus_step(&order, None, -1), Some('c'));
        assert_eq!(focus_step(&order, Some('c'), 1), Some('a'));
        assert_eq!(focus_step(&order, Some('a'), -1), Some('c'));
        assert_eq!(focus_step(&order, Some('z'), 1), Some('a'));
        assert_eq!(focus_step::<char>(&[], None, 1), None);
    }
}
