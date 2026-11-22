//! The text fields' context menu (chimera_ui::text_menu): what it offers
//! per field, opening on a right click (and not closing the dialog it is
//! in), choosing by pointer and keys, and the key each choice types.
//! Bevy's UI focus is stood in for by a system that writes the pointer.

use bevy::{
    input::{ButtonState, keyboard::KeyboardInput, mouse::MouseButtonInput},
    prelude::*,
    ui::{DeprecatedInteraction as Interaction, RelativeCursorPosition, UiSystems},
    window::PrimaryWindow,
};
use chimera_ui::{
    ChimeraUiPlugin, Modals, Theme, UiClock, UiUnits,
    clipboard::Clipboard,
    controls::{State, TextCursor, Ui},
    text_menu::{TextField, TextMenu, TextMenuAction as A, TextMenuItem, items_for},
};

const SIGNAL: &str = include_str!("../../../../apps/ultimate_gun_game/data/theme.toml");

/// What the stand-in focus writes: (node, interaction, pointer over it).
#[derive(Resource, Default)]
struct Pointer(Vec<(Entity, Interaction, bool)>);

/// The input the app wrote.
#[derive(Resource, Default)]
struct Seen {
    keys: Vec<(KeyCode, ButtonState)>,
    clicks: Vec<(MouseButton, ButtonState)>,
}

fn fake_focus(p: Res<Pointer>, mut nodes: Query<(Option<&mut Interaction>, Option<&mut RelativeCursorPosition>)>) {
    for (e, i, over) in &p.0 {
        if let Ok((int, rel)) = nodes.get_mut(*e) {
            if let Some(mut int) = int {
                int.set_if_neq(*i);
            }
            if let Some(mut r) = rel {
                r.set_if_neq(RelativeCursorPosition { cursor_over: *over, normalized: over.then_some(Vec2::ZERO) });
            }
        }
    }
}

fn see(mut keys: MessageReader<KeyboardInput>, mut clicks: MessageReader<MouseButtonInput>, mut seen: ResMut<Seen>) {
    seen.keys.extend(keys.read().map(|k| (k.key_code, k.state)));
    seen.clicks.extend(clicks.read().map(|c| (c.button, c.state)));
}

/// Bevy's keyboard input system, stood in for: events press the keys.
fn feed(mut events: MessageReader<KeyboardInput>, mut keys: ResMut<ButtonInput<KeyCode>>) {
    for e in events.read() {
        match e.state {
            ButtonState::Pressed => keys.press(e.key_code),
            ButtonState::Released => keys.release(e.key_code),
        }
    }
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::text::TextPlugin)).add_plugins(ChimeraUiPlugin);
    app.insert_resource(UiUnits::new(1080.0, 1.0));
    app.insert_resource(Theme::from_toml(SIGNAL).unwrap());
    app.insert_resource(UiClock::manual());
    app.insert_resource(Clipboard::memory(Some("pasted")));
    app.init_resource::<ButtonInput<MouseButton>>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<Pointer>().init_resource::<Seen>();
    app.add_systems(PreUpdate, fake_focus.in_set(UiSystems::Focus));
    app.add_systems(PreUpdate, feed.in_set(bevy::input::InputSystems));
    app.add_systems(Update, see);
    let mut window = Window::default();
    window.set_cursor_position(Some(Vec2::new(100.0, 100.0)));
    app.world_mut().spawn((window, PrimaryWindow));
    app
}

fn frame(app: &mut App) {
    app.world_mut().resource_mut::<UiClock>().step(1.0 / 60.0);
    app.update();
    let w = app.world_mut();
    w.resource_mut::<ButtonInput<MouseButton>>().clear();
    w.resource_mut::<ButtonInput<KeyCode>>().clear();
}

/// A text field with `value`, the caret and anchor as given (None: no
/// keys), in a root; the field.
fn field(app: &mut App, parent: Option<Entity>, value: &str, caret: Option<(usize, usize)>, masked: bool) -> Entity {
    let theme = app.world().resource::<Theme>().clone();
    let units = *app.world().resource::<UiUnits>();
    let world = app.world_mut();
    let root = parent.unwrap_or_else(|| world.spawn(Node::default()).id());
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let mut c = Commands::new(&mut queue, world);
    let ui = Ui::new(&theme, None, units);
    let f = ui.text_field(&mut c, root, value, "", caret.map(|(at, anchor)| TextCursor { at, anchor }), masked, 15.0, State::focused(caret.is_some()));
    queue.apply(world);
    frame(app);
    f
}

fn menu(app: &mut App) -> Option<TextMenu> {
    app.world_mut().query::<&TextMenu>().iter(app.world()).next().cloned()
}

fn right_click(app: &mut App) {
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().release(MouseButton::Right);
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().clear();
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Right);
    frame(app);
}

fn over(app: &mut App, f: Entity) {
    app.world_mut().resource_mut::<Pointer>().0 = vec![(f, Interaction::Hovered, true)];
    frame(app);
}

fn enabled(m: &TextMenu) -> Vec<(A, bool)> {
    m.items.clone()
}

#[test]
fn the_menu_offers_what_the_field_can_do() {
    let some = TextField { masked: false, editing: true, selected: true, empty: false };
    assert_eq!(items_for(&some, true), [(A::Cut, true), (A::Copy, true), (A::Paste, true), (A::SelectAll, true)]);
    let none = TextField { selected: false, empty: true, ..some };
    assert_eq!(items_for(&none, false), [(A::Cut, false), (A::Copy, false), (A::Paste, false), (A::SelectAll, false)]);
    let secret = TextField { masked: true, ..some };
    assert_eq!(items_for(&secret, true), [(A::Paste, true)], "a password: Paste only");
}

#[test]
fn the_kits_fields_carry_their_state() {
    let mut app = app();
    let f = field(&mut app, None, "hello", Some((1, 4)), false);
    assert_eq!(app.world().get::<TextField>(f), Some(&TextField { masked: false, editing: true, selected: true, empty: false }));
    let p = field(&mut app, None, "pw", None, true);
    assert_eq!(app.world().get::<TextField>(p), Some(&TextField { masked: true, editing: false, selected: false, empty: false }));
}

#[test]
fn a_right_click_on_a_field_opens_the_menu_as_a_modal() {
    let mut app = app();
    let f = field(&mut app, None, "hello", Some((1, 4)), false);
    over(&mut app, f);
    right_click(&mut app);
    frame(&mut app);
    let m = menu(&mut app).expect("the menu is open");
    assert_eq!(enabled(&m), [(A::Cut, true), (A::Copy, true), (A::Paste, true), (A::SelectAll, true)]);
    assert!(app.world().resource::<Modals>().is_open(), "a modal layer: the screen behind gets no input");
    assert!(app.world().resource::<Seen>().keys.is_empty(), "the right click is not a Back");
    // Nothing on the clipboard: Paste is greyed.
    let mut app = self::app();
    app.insert_resource(Clipboard::memory(None));
    let f = field(&mut app, None, "", Some((0, 0)), false);
    over(&mut app, f);
    right_click(&mut app);
    frame(&mut app);
    assert_eq!(enabled(&menu(&mut app).unwrap()), [(A::Cut, false), (A::Copy, false), (A::Paste, false), (A::SelectAll, false)]);
}

#[test]
fn a_field_without_the_keys_is_clicked_first() {
    let mut app = app();
    let f = field(&mut app, None, "abc", None, false);
    over(&mut app, f);
    right_click(&mut app);
    assert!(menu(&mut app).is_none(), "not yet: the field has no keys");
    assert_eq!(app.world().resource::<Seen>().clicks, [(MouseButton::Left, ButtonState::Pressed)]);
    frame(&mut app);
    assert_eq!(app.world().resource::<Seen>().clicks.last(), Some(&(MouseButton::Left, ButtonState::Released)));
    // The screen redraws the field with the keys: the menu opens on it.
    app.world_mut().entity_mut(f).insert(TextField { masked: false, editing: true, selected: false, empty: false });
    frame(&mut app);
    assert_eq!(enabled(&menu(&mut app).expect("opened")), [(A::Cut, false), (A::Copy, false), (A::Paste, true), (A::SelectAll, true)]);
}

#[test]
fn a_password_field_offers_paste_only() {
    let mut app = app();
    let f = field(&mut app, None, "hunter2", Some((0, 7)), true);
    over(&mut app, f);
    right_click(&mut app);
    frame(&mut app);
    assert_eq!(enabled(&menu(&mut app).unwrap()), [(A::Paste, true)]);
}

/// Pressed Ctrl and the key, then released.
fn typed(app: &App) -> Vec<(KeyCode, ButtonState)> {
    app.world().resource::<Seen>().keys.clone()
}

#[test]
fn choosing_by_pointer_types_the_key_and_closes() {
    let mut app = app();
    let f = field(&mut app, None, "hello", Some((1, 4)), false);
    over(&mut app, f);
    right_click(&mut app);
    frame(&mut app);
    let item = app.world_mut().query::<(Entity, &TextMenuItem)>().iter(app.world()).find(|(_, i)| i.0 == 2).map(|(e, _)| e).unwrap();
    app.world_mut().resource_mut::<Pointer>().0 = vec![(item, Interaction::Pressed, true)];
    frame(&mut app);
    assert!(menu(&mut app).is_none(), "closed by choosing");
    app.world_mut().resource_mut::<Pointer>().0.clear();
    frame(&mut app);
    frame(&mut app);
    assert_eq!(
        typed(&app),
        [(KeyCode::ControlLeft, ButtonState::Pressed), (KeyCode::KeyV, ButtonState::Pressed), (KeyCode::ControlLeft, ButtonState::Released), (KeyCode::KeyV, ButtonState::Released)],
        "Paste is Ctrl+V, down then up"
    );
}

#[test]
fn the_keys_move_choose_and_close() {
    let mut app = app();
    let f = field(&mut app, None, "hello", Some((1, 4)), false);
    over(&mut app, f);
    right_click(&mut app);
    frame(&mut app);
    // Down twice: Cut, Copy; Enter chooses Copy (Ctrl+C).
    for k in [KeyCode::ArrowDown, KeyCode::ArrowDown] {
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(k);
        frame(&mut app);
    }
    assert_eq!(menu(&mut app).unwrap().active, Some(1));
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Enter);
    frame(&mut app);
    frame(&mut app);
    assert!(menu(&mut app).is_none());
    assert!(typed(&app).contains(&(KeyCode::KeyC, ButtonState::Pressed)));
    // Esc closes it and types nothing.
    let mut app = self::app();
    let f = field(&mut app, None, "hello", Some((1, 4)), false);
    over(&mut app, f);
    right_click(&mut app);
    frame(&mut app);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Escape);
    frame(&mut app);
    assert!(menu(&mut app).is_none());
    frame(&mut app);
    assert!(typed(&app).is_empty(), "Esc chose nothing");
}

#[test]
fn a_greyed_item_is_not_chosen() {
    let mut app = app();
    let f = field(&mut app, None, "hello", Some((2, 2)), false);
    over(&mut app, f);
    right_click(&mut app);
    frame(&mut app);
    let cut = app.world_mut().query::<(Entity, &TextMenuItem)>().iter(app.world()).find(|(_, i)| i.0 == 0).map(|(e, _)| e).unwrap();
    app.world_mut().resource_mut::<Pointer>().0 = vec![(cut, Interaction::Pressed, true)];
    frame(&mut app);
    frame(&mut app);
    assert!(menu(&mut app).is_some(), "Cut with nothing selected does nothing");
    assert!(typed(&app).is_empty());
}

/// The dialog's right-click rule: on a field the menu opens and the
/// dialog stays; elsewhere right click is the dialog's Esc.
#[test]
fn in_a_dialog_a_right_click_on_a_field_opens_the_menu_and_elsewhere_closes() {
    let mut app = app();
    let dialog = app.world_mut().spawn((Node::default(), chimera_ui::Modal)).id();
    let f = field(&mut app, Some(dialog), "hello", Some((1, 4)), false);
    let elsewhere = app.world_mut().spawn((Node::default(), chimera_ui::controls::clickable(), ChildOf(dialog))).id();
    // Elsewhere in the dialog: Esc.
    app.world_mut().resource_mut::<Pointer>().0 = vec![(elsewhere, Interaction::Hovered, true)];
    frame(&mut app);
    right_click(&mut app);
    assert!(app.world().resource::<Seen>().keys.contains(&(KeyCode::Escape, ButtonState::Pressed)), "the dialog's Back");
    assert!(menu(&mut app).is_none());
    frame(&mut app);
    app.world_mut().resource_mut::<Seen>().keys.clear();
    // On the field: the menu, no Esc.
    over(&mut app, f);
    right_click(&mut app);
    frame(&mut app);
    assert!(menu(&mut app).is_some(), "the field's menu");
    assert!(app.world().resource::<Seen>().keys.is_empty(), "the dialog was not told to close");
    assert!(app.world().get_entity(dialog).is_ok());
    // A right click away from the menu closes the menu (Esc, for the top layer: the menu).
    app.world_mut().resource_mut::<Pointer>().0.clear();
    right_click(&mut app);
    assert!(menu(&mut app).is_none(), "the right click was the menu's Esc");
    assert!(app.world().get_entity(dialog).is_ok(), "and the dialog stays");
}
