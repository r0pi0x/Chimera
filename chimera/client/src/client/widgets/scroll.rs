use std::collections::HashMap;

use bevy::{
    prelude::*,
    text::LineBreak,
    ui::{ComputedNode, UiGlobalTransform},
    window::PrimaryWindow,
};

use crate::client::fonts::UiFonts;
use super::*;
use bevy::ui::Interaction;
use crate::console::ConsoleAppExt;
use crate::map::hud::GameUi;

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
pub(super) fn scroll_pointer(
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
pub(super) fn run_pointer_script(
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

