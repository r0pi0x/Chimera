//! Who drives a list's selection, the keys or the pointer (`PointerLead`):
//! one rule for every list and page that both hover and keys can select
//! in. A navigation key (arrows, Tab, Page Up/Down, Home, End) hands the
//! selection to the keys: hover stops moving it, so a pointer resting on
//! a row (or a row that scrolls or is rebuilt under it) never pins the
//! selection. The pointer takes it back only when it really moves (the
//! window's cursor changes place by more than half a pixel; a stale hover
//! state is not a move), and the list then selects the row under it.
//! `track` runs in `PreUpdate`, after Bevy's input, so a list's `Update`
//! systems read this frame's lead; `PointerLead::hover` is what they ask.

use bevy::{prelude::*, window::Window};

/// The lead (module doc). Starts with the pointer.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PointerLead {
    #[default]
    Pointer,
    Keys,
}

impl PointerLead {
    /// Whether hover may move the selection.
    pub fn hover(&self) -> bool {
        *self == PointerLead::Pointer
    }
}

/// The keys that take the lead.
pub const NAV_KEYS: [KeyCode; 9] = [
    KeyCode::ArrowUp,
    KeyCode::ArrowDown,
    KeyCode::ArrowLeft,
    KeyCode::ArrowRight,
    KeyCode::Tab,
    KeyCode::PageUp,
    KeyCode::PageDown,
    KeyCode::Home,
    KeyCode::End,
];

/// The lead from this frame's input (module doc).
pub fn track(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    windows: Query<&Window>,
    mut last: Local<Option<Vec2>>,
    mut lead: ResMut<PointerLead>,
) {
    let at = windows.iter().find_map(Window::cursor_position);
    let moved = match (*last, at) {
        (Some(a), Some(b)) => a.distance(b) > 0.5,
        (None, Some(_)) => true,
        _ => false,
    };
    *last = at;
    if keys.is_some_and(|k| k.any_just_pressed(NAV_KEYS)) {
        lead.set_if_neq(PointerLead::Keys);
    } else if moved {
        lead.set_if_neq(PointerLead::Pointer);
    }
}

/// Adds `PointerLeadPlugin` unless it is there (the kit adds it, and so
/// does each list that reads the lead, whichever comes first).
pub fn add(app: &mut App) {
    if !app.is_plugin_added::<PointerLeadPlugin>() {
        app.add_plugins(PointerLeadPlugin);
    }
}

/// Adds `PointerLead` and `track`.
pub struct PointerLeadPlugin;

impl Plugin for PointerLeadPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PointerLead>()
            .add_systems(PreUpdate, track.after(bevy::input::InputSystems));
    }
}
