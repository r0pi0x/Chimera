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
pub(super) fn slider_pointer(
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

