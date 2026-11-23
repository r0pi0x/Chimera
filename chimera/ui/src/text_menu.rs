//! The text fields' context menu (right click in a field: Cut, Copy,
//! Paste, Select all), built once in the kit for every field it draws
//! (`Ui::text_field`, `search_field`, `text_area`, a value box typed into,
//! the chat's field).
//!
//! - **What marks a field**: `TextField` (its caret's and selection's
//!   state, as the screen drew it) beside the field's
//!   `RelativeCursorPosition`. A screen adds nothing.
//! - **Opening**: a right click on a field (`shell::back_keys` leaves it
//!   alone: a right click on a field inside a dialog opens this menu
//!   instead of closing the dialog; elsewhere it closes it as ever), or
//!   the Menu key / Shift+F10 in the field that has the keys. A field that
//!   does not have the keys yet first gets a click (a left press and
//!   release written as input), so the keys are its own; the menu opens on
//!   the field as the screen redrew it.
//! - **The menu** is a modal layer (`Modal`, so the keys and the pointer
//!   belong to it and nothing falls through to the screen or the game):
//!   Cut and Copy need a selection, Select all some text, Paste text on the
//!   clipboard (greyed otherwise); a masked field (a password) offers Paste
//!   only. Up, Down and Enter (the arrows' and confirm keys, which a
//!   controller's d-pad and buttons arrive as) choose, Esc, a right click
//!   anywhere, or a click outside closes it, and choosing closes it.
//! - **Doing it**: the choice is typed as the key it stands for (Ctrl+X,
//!   Ctrl+C, Ctrl+V, Ctrl+A written as input for one frame, as
//!   `shell::back_keys` does Esc), so each screen's own text handling
//!   (`text_edit::text_key`, the masked field's no-copy rule, the paste's
//!   one line) does it with no code per screen.
//!
//! Known shortcuts: the console's own line (not a kit field) has no menu;
//! the menu's Paste is enabled when the clipboard can't be read (a
//! screen's own failure to paste then does nothing).

use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
        mouse::MouseButtonInput,
    },
    prelude::*,
    ui::{ComputedNode, DeprecatedInteraction as Interaction, FocusPolicy, RelativeCursorPosition, UiGlobalTransform},
    window::PrimaryWindow,
};

use crate::{
    KitFonts, Role, Theme, Tone, UiStyle, UiUnits,
    clipboard::Clipboard,
    controls::{State, Ui},
};

/// A text field's state as drawn (module doc): on the field's node.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextField {
    /// A password: Paste only.
    pub masked: bool,
    /// It has the keys (a caret is drawn).
    pub editing: bool,
    /// Something is selected.
    pub selected: bool,
    /// It holds no text.
    pub empty: bool,
}

impl TextField {
    /// For a field showing `value` with `caret` (Some while it has the keys).
    pub fn of(value: &str, caret: Option<crate::controls::TextCursor>, masked: bool) -> Self {
        Self { masked, editing: caret.is_some(), selected: caret.is_some_and(|k| k.at != k.anchor), empty: value.is_empty() }
    }
}

/// What the menu offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextMenuAction {
    Cut,
    Copy,
    Paste,
    SelectAll,
}

impl TextMenuAction {
    fn label(self) -> &'static str {
        match self {
            Self::Cut => "Cut",
            Self::Copy => "Copy",
            Self::Paste => "Paste",
            Self::SelectAll => "Select all",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Self::Cut => "Ctrl+X",
            Self::Copy => "Ctrl+C",
            Self::Paste => "Ctrl+V",
            Self::SelectAll => "Ctrl+A",
        }
    }

    fn key(self) -> (KeyCode, &'static str) {
        match self {
            Self::Cut => (KeyCode::KeyX, "x"),
            Self::Copy => (KeyCode::KeyC, "c"),
            Self::Paste => (KeyCode::KeyV, "v"),
            Self::SelectAll => (KeyCode::KeyA, "a"),
        }
    }
}

/// The menu's items for a field, each with whether it can be chosen
/// (`paste`: the clipboard holds text).
pub fn items_for(f: &TextField, paste: bool) -> Vec<(TextMenuAction, bool)> {
    use TextMenuAction::*;
    if f.masked {
        return vec![(Paste, paste)];
    }
    vec![(Cut, f.selected), (Copy, f.selected), (Paste, paste), (SelectAll, !f.empty)]
}

/// The open menu (the root of its layer).
#[derive(Component, Clone, Debug)]
pub struct TextMenu {
    /// Its top left, logical px.
    pub at: Vec2,
    pub items: Vec<(TextMenuAction, bool)>,
    /// The keys' line.
    pub active: Option<usize>,
}

#[derive(Component, Clone, Copy)]
pub struct TextMenuItem(pub usize);

/// The menu's progress between frames.
#[derive(Resource, Default, Debug)]
pub struct TextMenuState {
    /// Clicked a field to give it the keys: where the menu goes, frames left.
    waiting: Option<(Vec2, u8)>,
    /// The click's release, due next frame.
    release_click: bool,
    /// A choice typed: the key, and the frame it is on.
    typing: Option<(TextMenuAction, u8)>,
}

/// A right click, a pointer over a text field.
pub(crate) fn over_field(fields: &Query<&RelativeCursorPosition, With<TextField>>) -> bool {
    fields.iter().any(|r| r.cursor_over)
}

const W: f32 = 11.5;
const ROW_H: f32 = 1.8;

fn build(c: &mut Commands, ui: &Ui, m: &TextMenu) -> Entity {
    let root = c
        .spawn((
            Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, ..default() },
            GlobalZIndex(960),
            FocusPolicy::Block,
            Interaction::default(),
            crate::modal::Modal,
            m.clone(),
        ))
        .id();
    let place = c.spawn((Node { position_type: PositionType::Absolute, left: Val::Px(m.at.x), top: Val::Px(m.at.y), ..default() }, ChildOf(root))).id();
    let plate = ui.boxed(c, place, UiStyle { w: Some(W), pad: Some([0.3, 0.0].into()), ..UiStyle::column() }.bg(Tone::Ink2).outline(Tone::Fg4, 1.0));
    c.entity(plate).insert(FocusPolicy::Block);
    for (i, (a, on)) in m.items.iter().enumerate() {
        let style = UiStyle { h: Some(ROW_H), pad: Some([0.0, 0.9].into()), justify: Some(JustifyContent::SpaceBetween), ..UiStyle::row() };
        let e = ui.item(c, plate, style, State { focus: m.active == Some(i) && *on, disabled: !on, ..State::NONE }, false);
        c.entity(e).insert(TextMenuItem(i));
        ui.text(c, e, Role::Mono, 0.62, if *on { Tone::Fg } else { Tone::Fg4 }, a.label());
        ui.text(c, e, Role::Mono, 0.5, if *on { Tone::Fg3 } else { Tone::Fg5 }, a.hint());
    }
    root
}

fn rect(f: &(&ComputedNode, &UiGlobalTransform)) -> Rect {
    let c = f.1.affine().translation;
    let half = f.0.size * 0.5;
    Rect::from_corners(c - half, c + half)
}

/// Where the menu goes: `at` (logical px), kept inside the window.
fn clamp_in(at: Vec2, window: Option<&Window>, ui: &Ui, rows: usize) -> Vec2 {
    let Some(w) = window else { return at };
    let (mw, mh) = (ui.units.cq(W), ui.units.cq(ROW_H) * rows as f32 + ui.units.cq(0.6));
    Vec2::new(at.x.min((w.width() - mw).max(0.0)), at.y.min((w.height() - mh).max(0.0)))
}

/// Opening and the typed choice (module doc). `PreUpdate`, before Bevy's
/// input systems, with `shell::back_keys`.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn drive(
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mut st: ResMut<TextMenuState>,
    open: Query<(), With<TextMenu>>,
    fields: Query<(&TextField, &RelativeCursorPosition, Option<&ComputedNode>, Option<&UiGlobalTransform>)>,
    window: Query<(Entity, &Window), With<PrimaryWindow>>,
    cursor: Query<&bevy::window::CursorOptions, With<PrimaryWindow>>,
    blocking: Query<(), With<crate::modal::Blocking>>,
    modals: Res<crate::modal::Modals>,
    mut clipboard: Option<ResMut<Clipboard>>,
    theme: Res<Theme>,
    units: Res<UiUnits>,
    kit: Option<Res<KitFonts>>,
    mut input: MessageWriter<KeyboardInput>,
    mut clicks: MessageWriter<MouseButtonInput>,
    mut commands: Commands,
) {
    let Ok((win, w)) = window.single() else { return };
    let ui = Ui::new(&theme, kit.as_deref(), *units);
    // A choice: Ctrl and the key down this frame, up the next.
    if let Some((a, n)) = st.typing.take() {
        let (code, ch) = a.key();
        let state = if n == 0 { ButtonState::Pressed } else { ButtonState::Released };
        let ev = |key_code, logical_key, text: Option<&str>| KeyboardInput { key_code, logical_key, state, text: text.map(Into::into), repeat: false, window: win };
        input.write(ev(KeyCode::ControlLeft, Key::Control, None));
        input.write(ev(code, Key::Character(ch.into()), Some(ch)));
        if n == 0 {
            st.typing = Some((a, 1));
        }
        return;
    }
    if std::mem::take(&mut st.release_click) {
        clicks.write(MouseButtonInput { button: MouseButton::Left, state: ButtonState::Released, window: win });
    }
    let free = cursor.single().map_or(true, |o| o.visible && o.grab_mode == bevy::window::CursorGrabMode::None);
    if !open.is_empty() || !free || modals.top().is_some_and(|t| blocking.contains(t)) {
        st.waiting = None;
        return;
    }
    let paste = |clip: &mut Option<ResMut<Clipboard>>| clip.as_mut().map_or(true, |c| c.get().is_some_and(|t| !t.is_empty()));
    let spawn = |commands: &mut Commands, f: &TextField, at: Vec2, clip: &mut Option<ResMut<Clipboard>>| {
        let items = items_for(f, paste(clip));
        let at = clamp_in(at, Some(w), &ui, items.len());
        build(commands, &ui, &TextMenu { at, items, active: None });
    };
    // The keys' own: the Menu key, or Shift+F10, in the field that has them.
    if let Some(k) = keys.as_deref()
        && (k.just_pressed(KeyCode::ContextMenu) || (k.just_pressed(KeyCode::F10) && k.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight])))
        && let Some((f, _, n, t)) = fields.iter().find(|(f, ..)| f.editing)
    {
        let r = n.zip(t).map_or(Rect::default(), |(n, t)| rect(&(n, t)));
        let s = w.scale_factor();
        spawn(&mut commands, f, Vec2::new(r.min.x, r.max.y) / s, &mut clipboard);
        return;
    }
    if let Some((at, n)) = st.waiting {
        if let Some((f, ..)) = fields.iter().find(|(f, r, ..)| f.editing && r.cursor_over) {
            st.waiting = None;
            spawn(&mut commands, f, at, &mut clipboard);
        } else {
            st.waiting = (n > 1).then_some((at, n - 1));
        }
        return;
    }
    let right = mouse.as_deref().is_some_and(|m| m.just_pressed(MouseButton::Right) && !m.pressed(MouseButton::Left));
    if !right {
        return;
    }
    let Some(at) = w.cursor_position() else { return };
    let Some((f, ..)) = fields.iter().find(|(_, r, ..)| r.cursor_over) else { return };
    if f.editing {
        spawn(&mut commands, f, at, &mut clipboard);
    } else {
        // Give the field the keys first, as a click does.
        clicks.write(MouseButtonInput { button: MouseButton::Left, state: ButtonState::Pressed, window: win });
        st.release_click = true;
        st.waiting = Some((at, 8));
    }
}

/// The menu's keys and pointer; swallows the keyboard while it is up.
/// `PreUpdate`, after Bevy's input and UI focus.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn menu_input(
    menus: Query<(Entity, &TextMenu, &Interaction)>,
    items: Query<(&TextMenuItem, &Interaction), Changed<Interaction>>,
    mut keys: Option<ResMut<ButtonInput<KeyCode>>>,
    mut events: ResMut<Messages<KeyboardInput>>,
    mut st: ResMut<TextMenuState>,
    theme: Res<Theme>,
    units: Res<UiUnits>,
    kit: Option<Res<KitFonts>>,
    mut closes: MessageReader<crate::modal::ClosePopovers>,
    mut commands: Commands,
) {
    let Ok((root, menu, backdrop)) = menus.single() else {
        closes.clear();
        return;
    };
    // Another popover opened, or a modal other than this menu.
    let told = closes.read().any(|m| m.closes("text_menu") && m.closes_layer(root));
    let k = keys.as_deref();
    let just = |c: KeyCode| k.is_some_and(|k| k.just_pressed(c));
    let on = |i: usize| menu.items[i].1;
    let step = |from: Option<usize>, d: isize| -> Option<usize> {
        let n = menu.items.len() as isize;
        let mut i = from.map_or(if d > 0 { -1 } else { n }, |i| i as isize);
        for _ in 0..n {
            i = (i + d).rem_euclid(n);
            if on(i as usize) {
                return Some(i as usize);
            }
        }
        from
    };
    let mut choose = None;
    let mut close = just(KeyCode::Escape) || told;
    let mut active = menu.active;
    if just(KeyCode::ArrowDown) {
        active = step(active, 1);
    }
    if just(KeyCode::ArrowUp) {
        active = step(active, -1);
    }
    if just(KeyCode::Enter) || just(KeyCode::NumpadEnter) || just(KeyCode::Space) {
        choose = active;
    }
    for (i, int) in &items {
        if *int == Interaction::Pressed && on(i.0) {
            choose = Some(i.0);
        }
    }
    if choose.is_none() && *backdrop == Interaction::Pressed {
        close = true;
    }
    // Nothing reaches the screen behind, nor the game.
    events.clear();
    if let Some(k) = keys.as_deref_mut() {
        k.reset_all();
    }
    if let Some(i) = choose {
        st.typing = Some((menu.items[i].0, 0));
        close = true;
    }
    if close {
        commands.entity(root).despawn();
    } else if active != menu.active {
        let ui = Ui::new(&theme, kit.as_deref(), *units);
        let m = TextMenu { active, ..menu.clone() };
        commands.entity(root).despawn();
        build(&mut commands, &ui, &m);
    }
}

/// The menu's systems (in the kit's plugin).
pub fn add(app: &mut App) {
    use bevy::{input::InputSystems, ui::UiSystems};
    app.init_resource::<TextMenuState>()
        .add_message::<MouseButtonInput>()
        .add_systems(PreUpdate, drive.before(InputSystems))
        .add_systems(PreUpdate, menu_input.after(InputSystems).after(UiSystems::Focus).after(crate::modal::ModalSystems));
}
