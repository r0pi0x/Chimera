//! Modal layers (chimera_ui::modal): a page with a scroll view and a
//! button, a dialog (`Modal`) over it with its own scroll view. Bevy's UI
//! focus is stood in for by a system in its set that writes what it would
//! (the pointer over the page and the dialog alike, as when the dialog
//! has no scrim under the pointer): the wheel scrolls only the dialog, a
//! click or hover reaches nothing behind it, something drawn above the
//! top modal still takes the pointer, and Esc closes the top modal only.

use bevy::{
    input::mouse::{AccumulatedMouseScroll, MouseScrollUnit},
    prelude::*,
    ui::{ComputedNode, ComputedStackIndex, DeprecatedInteraction as Interaction, RelativeCursorPosition, UiGlobalTransform, UiSystems},
};
use chimera_ui::{
    ChimeraUiPlugin, Modal, Modals, ScrollParts, ScrollViews, Theme, UiClock, UiStyle, UiUnits,
    controls::{Hover, Ui},
};

const SIGNAL: &str = include_str!("../../../../apps/ultimate_gun_game/data/theme.toml");
const PX: f32 = 19.2;
const BASE: Color = Color::srgb(0.1, 0.1, 0.1);
const OVER: Color = Color::srgb(0.9, 0.9, 0.9);

/// What the stand-in focus writes each frame: (node, interaction,
/// pointer over it).
#[derive(Resource, Default)]
struct Pointer(Vec<(Entity, Interaction, bool)>);

/// The presses an `Update` system saw.
#[derive(Resource, Default)]
struct Pressed(Vec<Entity>);

/// A modal Esc closes when it is the top.
#[derive(Component)]
struct Closes;

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

fn see_presses(q: Query<(Entity, &Interaction), Changed<Interaction>>, mut seen: ResMut<Pressed>) {
    for (e, i) in &q {
        if *i == Interaction::Pressed {
            seen.0.push(e);
        }
    }
}

/// Esc closes the top modal (a dialog's own key system asks the stack).
fn esc_closes_top(keys: Res<ButtonInput<KeyCode>>, modals: Res<Modals>, q: Query<Entity, With<Closes>>, mut c: Commands) {
    if keys.just_pressed(KeyCode::Escape) {
        for e in &q {
            if modals.is_top(e) {
                c.entity(e).despawn();
            }
        }
    }
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::text::TextPlugin)).add_plugins(ChimeraUiPlugin);
    app.insert_resource(UiUnits::new(1080.0, 1.0));
    app.insert_resource(Theme::from_toml(SIGNAL).unwrap());
    app.insert_resource(UiClock::manual());
    app.init_resource::<ButtonInput<MouseButton>>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<AccumulatedMouseScroll>();
    app.init_resource::<Pointer>().init_resource::<Pressed>();
    app.add_systems(PreUpdate, fake_focus.in_set(UiSystems::Focus));
    app.add_systems(Update, (see_presses, esc_closes_top));
    app
}

fn node(h_cq: f32) -> (ComputedNode, UiGlobalTransform) {
    let size = Vec2::new(600.0, h_cq * PX);
    (ComputedNode { size, inverse_scale_factor: 1.0, ..default() }, UiGlobalTransform::from(bevy::math::Affine2::from_translation(size * 0.5)))
}

struct Scene {
    page: ScrollParts,
    button: Entity,
    dialog: Entity,
    inner: ScrollParts,
}

/// A page (a 20 cq view over 60 cq, a hoverable button) and a dialog
/// over it (a `Modal` with its own view).
fn scene(app: &mut App) -> Scene {
    let theme = app.world().resource::<Theme>().clone();
    let units = *app.world().resource::<UiUnits>();
    let world = app.world_mut();
    let root = world.spawn(Node::default()).id();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let mut c = Commands::new(&mut queue, world);
    let ui = Ui::new(&theme, None, units);
    let page = ui.scroll(&mut c, root, "page", UiStyle::row().h(20.0), UiStyle::column());
    let button = c.spawn((Node::default(), Hover { base: BASE, over: OVER }, BackgroundColor(BASE), chimera_ui::controls::clickable(), ChildOf(root))).id();
    let dialog = c.spawn((Node::default(), Modal, Closes, ChildOf(root))).id();
    let inner = ui.scroll(&mut c, dialog, "dialog", UiStyle::row().h(20.0), UiStyle::column());
    queue.apply(world);
    for p in [page, inner] {
        world.entity_mut(p.view).insert(node(20.0));
        world.entity_mut(p.content).insert(node(60.0));
        world.entity_mut(p.track).insert(node(20.0));
    }
    frame(app);
    Scene { page, button, dialog, inner }
}

fn frame(app: &mut App) {
    app.world_mut().resource_mut::<UiClock>().step(1.0 / 60.0);
    app.update();
    let w = app.world_mut();
    w.resource_mut::<ButtonInput<MouseButton>>().clear();
    w.resource_mut::<ButtonInput<KeyCode>>().clear();
    *w.resource_mut::<AccumulatedMouseScroll>() = AccumulatedMouseScroll::default();
}

fn wheel(app: &mut App) {
    app.insert_resource(AccumulatedMouseScroll { unit: MouseScrollUnit::Line, delta: Vec2::new(0.0, -1.0) });
}

fn moved(app: &App, key: &str) -> bool {
    app.world().resource::<ScrollViews>().get(key).is_some_and(|s| s.offset > 0.0 || s.target.is_some_and(|t| t > 0.0))
}

#[test]
fn the_wheel_in_a_dialog_scrolls_only_the_dialog() {
    let mut app = app();
    let s = scene(&mut app);
    app.world_mut().resource_mut::<Pointer>().0 =
        vec![(s.page.view, Interaction::None, true), (s.inner.view, Interaction::None, true)];
    wheel(&mut app);
    frame(&mut app);
    assert_eq!(app.world().resource::<Modals>().top(), Some(s.dialog));
    assert!(moved(&app, "dialog"), "the dialog's list scrolls");
    assert!(!moved(&app, "page"), "the page behind stays put");
    // The dialog gone: the same wheel scrolls the page.
    app.world_mut().entity_mut(s.dialog).despawn();
    app.world_mut().resource_mut::<Pointer>().0 = vec![(s.page.view, Interaction::None, true)];
    wheel(&mut app);
    frame(&mut app);
    assert!(!app.world().resource::<Modals>().is_open());
    assert!(moved(&app, "page"), "no modal: the page scrolls");
}

#[test]
fn a_click_or_hover_behind_a_dialog_reaches_nothing() {
    let mut app = app();
    let s = scene(&mut app);
    app.world_mut().resource_mut::<Pointer>().0 = vec![(s.button, Interaction::Hovered, true)];
    frame(&mut app);
    assert_eq!(app.world().get::<Interaction>(s.button), Some(&Interaction::None), "no hover behind");
    assert_eq!(app.world().get::<BackgroundColor>(s.button).unwrap().0, BASE, "no hover tint behind");
    app.world_mut().resource_mut::<Pointer>().0 = vec![(s.button, Interaction::Pressed, true)];
    frame(&mut app);
    assert!(app.world().resource::<Pressed>().0.is_empty(), "no press behind: {:?}", app.world().resource::<Pressed>().0);
    // Drawn above the top modal (a tooltip, a screen over it): it takes the pointer.
    app.world_mut().entity_mut(s.dialog).insert(ComputedStackIndex(5));
    app.world_mut().entity_mut(s.button).insert(ComputedStackIndex(9));
    frame(&mut app);
    assert_eq!(app.world().resource::<Pressed>().0, vec![s.button], "above the modal: pressed");
    // The dialog gone: hover lifts the button.
    app.world_mut().entity_mut(s.button).insert(ComputedStackIndex(0));
    app.world_mut().entity_mut(s.dialog).despawn();
    app.world_mut().resource_mut::<Pointer>().0 = vec![(s.button, Interaction::Hovered, true)];
    frame(&mut app);
    assert_eq!(app.world().get::<BackgroundColor>(s.button).unwrap().0, OVER);
}

#[test]
fn esc_closes_the_top_modal_only() {
    let mut app = app();
    let s = scene(&mut app);
    // A confirm over the dialog, and a button in the dialog under it.
    let in_dialog = app.world_mut().spawn((Node::default(), chimera_ui::controls::clickable(), ChildOf(s.dialog))).id();
    let confirm = app.world_mut().spawn((Node::default(), Modal, Closes, ChildOf(s.dialog))).id();
    app.world_mut().resource_mut::<Pointer>().0 = vec![(in_dialog, Interaction::Hovered, true)];
    frame(&mut app);
    assert_eq!(app.world().resource::<Modals>().stack(), &[s.dialog, confirm], "the confirm over the dialog");
    assert_eq!(app.world().get::<Interaction>(in_dialog), Some(&Interaction::None), "the dialog under the confirm: no hover");
    assert!(!app.world().resource::<Modals>().keys_to(None), "the page's keys are not its own");
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Escape);
    frame(&mut app);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::Escape);
    frame(&mut app);
    assert!(app.world().get_entity(confirm).is_err(), "Esc closed the confirm");
    assert!(app.world().get_entity(s.dialog).is_ok(), "and not the dialog");
    assert_eq!(app.world().resource::<Modals>().top(), Some(s.dialog));
    assert_eq!(app.world().get::<Interaction>(in_dialog), Some(&Interaction::Hovered), "the dialog takes the pointer again");
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Escape);
    frame(&mut app);
    assert!(app.world().get_entity(s.dialog).is_err(), "the next Esc closes the dialog");
    frame(&mut app);
    assert!(app.world().resource::<Modals>().keys_to(None), "the page has the keys again");
}

#[test]
fn a_hidden_modal_is_not_up() {
    let mut app = app();
    let s = scene(&mut app);
    app.world_mut().entity_mut(s.dialog).insert(Visibility::Hidden);
    frame(&mut app);
    assert!(!app.world().resource::<Modals>().is_open());
}

/// The Esc messages written since the last call.
fn escapes(app: &mut App) -> Vec<bevy::input::ButtonState> {
    #[derive(Resource, Default)]
    struct Seen(bevy::ecs::message::MessageCursor<bevy::input::keyboard::KeyboardInput>);
    app.world_mut().init_resource::<Seen>();
    app.world_mut().resource_scope(|w, mut seen: Mut<Seen>| {
        let msgs = w.resource::<Messages<bevy::input::keyboard::KeyboardInput>>();
        seen.0.read(msgs).filter(|k| k.key_code == KeyCode::Escape).map(|k| k.state).collect()
    })
}

#[test]
fn right_click_presses_esc_for_the_top_modal_unless_it_is_blocking() {
    use bevy::input::ButtonState;
    let mut app = app();
    let s = scene(&mut app);
    // The dialog: a right click is Esc, pressed once then released; the
    // button held down is not a Back.
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Right);
    frame(&mut app);
    assert_eq!(escapes(&mut app), [ButtonState::Pressed], "right click is the dialog's Esc");
    frame(&mut app);
    assert_eq!(escapes(&mut app), [ButtonState::Released]);
    frame(&mut app);
    assert!(escapes(&mut app).is_empty(), "once per press");
    // A blocking confirm over it takes the right click and does nothing.
    let confirm = app.world_mut().spawn((Node::default(), Modal, chimera_ui::Blocking, ChildOf(s.dialog))).id();
    frame(&mut app);
    assert_eq!(app.world().resource::<Modals>().top(), Some(confirm));
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Right);
    frame(&mut app);
    frame(&mut app);
    assert!(escapes(&mut app).is_empty(), "a blocking modal ignores right click");
    // Gone, the dialog under it closes on right click again.
    app.world_mut().entity_mut(confirm).despawn();
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().release(MouseButton::Right);
    frame(&mut app);
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Right);
    frame(&mut app);
    assert_eq!(escapes(&mut app), [ButtonState::Pressed]);
}

/// The kit's `Ui::modal` closes on right click, `modal_blocking` does not.
#[test]
fn the_kits_modal_builders_set_the_flag() {
    let mut app = app();
    let theme = app.world().resource::<Theme>().clone();
    let units = *app.world().resource::<UiUnits>();
    let world = app.world_mut();
    let (a, b) = (world.spawn(Node::default()).id(), world.spawn(Node::default()).id());
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let mut c = Commands::new(&mut queue, world);
    let ui = Ui::new(&theme, None, units);
    ui.modal(&mut c, a, 30.0, false);
    ui.modal_blocking(&mut c, b, 30.0, true);
    queue.apply(world);
    assert!(world.get::<Modal>(a).is_some() && world.get::<chimera_ui::Blocking>(a).is_none());
    assert!(world.get::<Modal>(b).is_some() && world.get::<chimera_ui::Blocking>(b).is_some());
}

#[derive(Resource, Default)]
struct Closed(u32);

fn count_closes(mut r: MessageReader<chimera_ui::ClosePopovers>, mut n: ResMut<Closed>) {
    n.0 += r.read().count() as u32;
}

/// A modal opening tells every popover to close, once (a dropdown open
/// under it stays open nowhere); a modal already up says nothing again.
#[test]
fn opening_a_modal_closes_every_popover_once() {
    let mut app = app();
    app.init_resource::<Closed>().add_systems(Update, count_closes);
    let root = app.world_mut().spawn(Node::default()).id();
    frame(&mut app);
    frame(&mut app);
    assert_eq!(app.world().resource::<Closed>().0, 0, "no modal: nothing closes");
    let first = app.world_mut().spawn((Node::default(), Modal, ChildOf(root))).id();
    frame(&mut app);
    frame(&mut app);
    assert_eq!(app.world().resource::<Closed>().0, 1, "a dialog opens: the popovers close once");
    // A second modal over it (a confirm) closes them again; the first staying up does not.
    app.world_mut().spawn((Node::default(), Modal, ChildOf(first)));
    frame(&mut app);
    frame(&mut app);
    assert_eq!(app.world().resource::<Closed>().0, 2, "a confirm over it closes them again");
}
