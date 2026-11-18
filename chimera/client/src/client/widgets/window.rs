use std::collections::HashMap;

use bevy::{
    prelude::*,
    text::LineBreak,
    ui::{ComputedNode, UiGlobalTransform},
    window::PrimaryWindow,
};

use crate::client::fonts::UiFonts;
use super::*;
use crate::console::ConsoleAppExt;
use crate::map::hud::GameUi;

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
    pub(super) order: Vec<WindowId>,
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
pub(super) fn place_frames(
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
pub(super) fn frame_rects(frames: &Query<(&VguiFrame, &ComputedNode, &UiGlobalTransform)>) -> Vec<(VguiFrame, Rect)> {
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
pub(super) fn frames_pointer(
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

