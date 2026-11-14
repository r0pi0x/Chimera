//! The scroll container (chimera_ui::scroll): the fling's physics on the
//! manual clock, then a built view with its layout set by hand (no
//! renderer or window here: the sizes Bevy's layout would give, the
//! pointer as `RelativeCursorPosition`): the wheel, the right-button pan
//! and fling, the edges, a row scrolled into view, the bar's thumb.

use bevy::{
    input::mouse::{AccumulatedMouseScroll, MouseScrollUnit},
    prelude::*,
    ui::{ComputedNode, RelativeCursorPosition, ScrollPosition, UiGlobalTransform},
};
use chimera_ui::{
    ChimeraUiPlugin, ReducedMotion, ScrollIntoView, ScrollParts, ScrollViews, Theme, Tone, UiClock, UiSlant, UiStyle, UiUnits,
    controls::Ui,
    scroll::{ScrollState, ScrollTuning, band},
};

const SIGNAL: &str = include_str!("../../../../apps/ultimate_gun_game/data/theme.toml");
const DT: f32 = 1.0 / 60.0;
/// cq to px at 1080 lines.
const PX: f32 = 19.2;
const VIEW: f32 = 20.0;
const CONTENT: f32 = 60.0;

fn tune() -> ScrollTuning {
    ScrollTuning::from_theme(&Theme::from_toml(SIGNAL).unwrap())
}

fn measured() -> ScrollState {
    let mut s = ScrollState::default();
    s.measure(VIEW, CONTENT);
    s.track = VIEW;
    s
}

/// Steps until at rest (at most `n`), returning how many it took.
fn settle(s: &mut ScrollState, t: &ScrollTuning, reduced: bool, n: usize) -> usize {
    for i in 0..n {
        if s.at_rest() {
            return i;
        }
        s.step(DT, t, reduced);
    }
    n
}

/// A pan from `y0` moving `dy` cq a frame for `frames`, then let go.
fn pan(s: &mut ScrollState, t: &ScrollTuning, y0: f32, dy: f32, frames: usize, reduced: bool) -> f64 {
    let mut now = 1.0;
    s.pan_start(y0, now);
    for i in 1..=frames {
        now += DT as f64;
        s.pan_move(y0 + dy * i as f32, now, t, reduced);
    }
    s.pan_end(now, t, reduced);
    now
}

#[test]
fn a_fling_decays_to_rest_the_same_every_time() {
    let t = tune();
    let run = || {
        let mut s = measured();
        s.offset = 10.0;
        pan(&mut s, &t, 10.0, -0.5, 12, false);
        assert!((s.offset - 16.0).abs() < 1e-4, "the pan is 1:1: {}", s.offset);
        let v0 = s.velocity;
        assert!((v0 - 30.0).abs() < 0.5, "the release's speed over the last 0.1 s: {v0}");
        let mut last_v = v0;
        let mut frames = 0;
        while !s.at_rest() && frames < 2000 {
            s.step(DT, &t, false);
            assert!(s.velocity.abs() <= last_v.abs() + 1e-4, "only slows");
            last_v = s.velocity;
            frames += 1;
        }
        assert!(frames < 2000, "comes to rest");
        (s.offset, frames)
    };
    let (a, fa) = run();
    let (b, fb) = run();
    assert_eq!((a.to_bits(), fa), (b.to_bits(), fb), "deterministic on the manual clock");
    // v0 / friction, less the cut-off below min_velocity.
    let ideal = 16.0 + 30.0 / t.friction;
    assert!(a > 16.0 + 5.0 && a < ideal + 0.5, "glides on: {a} (ideal {ideal})");
}

#[test]
fn speed_is_capped_and_a_still_release_does_not_fling() {
    let t = tune();
    let mut s = measured();
    s.offset = 20.0;
    pan(&mut s, &t, 10.0, -20.0, 3, false);
    assert_eq!(s.velocity, t.max_velocity, "capped");
    let mut s = measured();
    s.offset = 20.0;
    let mut now = 1.0;
    s.pan_start(10.0, now);
    for _ in 0..6 {
        now += DT as f64;
        s.pan_move(8.0, now, &t, false);
    }
    // Held still for 0.2 s, then let go.
    for _ in 0..12 {
        now += DT as f64;
        s.pan_move(8.0, now, &t, false);
    }
    s.pan_end(now, &t, false);
    assert_eq!(s.velocity, 0.0, "held still before the release: no fling");
    assert_eq!(s.offset, 22.0);
}

#[test]
fn the_edges_are_elastic_and_pull_back() {
    let t = tune();
    assert_eq!(band(5.0, 40.0, 3.0), 5.0);
    assert!(band(-100.0, 40.0, 3.0) > -3.0 && band(-100.0, 40.0, 3.0) < -2.8, "at most the overscroll");
    assert!((band(41.0, 40.0, 3.0) - 40.75).abs() < 1e-4, "banded past the end");
    // Dragged down past the top, then let go: back to 0.
    let mut s = measured();
    pan(&mut s, &t, 5.0, 0.5, 10, false);
    assert!(s.offset < 0.0 && s.offset > -t.overscroll, "on the band: {}", s.offset);
    settle(&mut s, &t, false, 600);
    assert!(s.at_rest());
    assert_eq!(s.offset, 0.0, "pulled back to the top");
    // A fling into the bottom stops there.
    let mut s = measured();
    s.offset = 35.0;
    pan(&mut s, &t, 10.0, -1.0, 6, false);
    let mut deepest: f32 = 0.0;
    for _ in 0..600 {
        s.step(DT, &t, false);
        deepest = deepest.max(s.over());
    }
    assert!(s.at_rest());
    assert_eq!(s.offset, s.max, "at the end");
    assert!(deepest > 0.0 && deepest <= t.overscroll + 1e-4, "an elastic stop past the end: {deepest}");
}

#[test]
fn the_wheel_eases_and_adds_up() {
    let t = tune();
    let mut s = measured();
    s.wheel(t.wheel_step, false);
    s.step(DT, &t, false);
    assert!(s.offset > 0.0 && s.offset < t.wheel_step, "animated, not a jump: {}", s.offset);
    s.wheel(t.wheel_step, false);
    settle(&mut s, &t, false, 600);
    assert!((s.offset - 2.0 * t.wheel_step).abs() < 1e-4, "two notches: {}", s.offset);
    s.wheel(-1000.0, false);
    settle(&mut s, &t, false, 600);
    assert_eq!(s.offset, 0.0, "clamped at the top");
}

#[test]
fn reduced_motion_jumps_and_never_flings() {
    let t = tune();
    let mut s = measured();
    s.wheel(t.wheel_step, true);
    assert_eq!(s.offset, t.wheel_step, "at once");
    let mut s = measured();
    s.offset = 10.0;
    pan(&mut s, &t, 10.0, -0.5, 12, true);
    assert_eq!(s.velocity, 0.0);
    assert_eq!(s.offset, 16.0);
    let mut s = measured();
    pan(&mut s, &t, 5.0, 0.5, 10, true);
    assert_eq!(s.offset, 0.0, "no band");
}

#[test]
fn a_press_stops_a_fling() {
    let t = tune();
    let mut s = measured();
    s.offset = 10.0;
    pan(&mut s, &t, 10.0, -0.5, 12, false);
    s.step(DT, &t, false);
    let at = s.offset;
    s.stop();
    s.step(DT, &t, false);
    assert_eq!(s.offset, at, "stopped where it was");
}

#[test]
fn reveal_brings_a_row_in_with_its_margin() {
    let t = tune();
    let mut s = measured();
    s.reveal(2.0, 4.9, t.reveal_margin, false);
    assert!(s.target.is_none(), "already in view");
    s.reveal(30.0, 32.9, t.reveal_margin, false);
    settle(&mut s, &t, false, 600);
    assert!((s.offset - (32.9 + t.reveal_margin - VIEW)).abs() < 1e-3, "its bottom at the view's: {}", s.offset);
    s.reveal(5.0, 7.9, t.reveal_margin, false);
    settle(&mut s, &t, false, 600);
    assert!((s.offset - (5.0 - t.reveal_margin)).abs() < 1e-3, "its top at the view's");
}

#[test]
fn the_thumb_is_the_visible_share() {
    let t = tune();
    let mut s = measured();
    let (top, len) = s.thumb(VIEW, &t);
    assert_eq!(top, 0.0);
    assert!((len - VIEW / 3.0).abs() < 1e-4);
    s.offset = s.max;
    let (top, len) = s.thumb(VIEW, &t);
    assert!((top + len - VIEW).abs() < 1e-4, "at the track's end");
    // A press below the thumb pages down, on it drags it.
    let mut s = measured();
    s.track_press(18.0, &t, true);
    assert!((s.offset - VIEW * 0.9).abs() < 1e-4, "a page");
    let mut s = measured();
    s.track_press(1.0, &t, false);
    assert!(s.thumb_dragged());
    s.thumb_move(1.0 + (VIEW - len) * 0.5, &t);
    assert!((s.offset - s.max * 0.5).abs() < 1e-3, "half the track: half the way");
    s.thumb_end();
}

// ---------------------------------------------------------------------
// A built view.

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::text::TextPlugin)).add_plugins(ChimeraUiPlugin);
    app.insert_resource(UiUnits::new(1080.0, 1.0));
    app.insert_resource(Theme::from_toml(SIGNAL).unwrap());
    app.insert_resource(UiClock::manual());
    app.init_resource::<ButtonInput<MouseButton>>();
    app.init_resource::<AccumulatedMouseScroll>();
    app
}

fn node(h_cq: f32, top_cq: f32) -> (ComputedNode, UiGlobalTransform) {
    let size = Vec2::new(600.0, h_cq * PX);
    (
        ComputedNode { size, inverse_scale_factor: 1.0, ..default() },
        UiGlobalTransform::from(bevy::math::Affine2::from_translation(Vec2::new(300.0, top_cq * PX + size.y * 0.5))),
    )
}

/// A view 20 cq tall over 60 cq of content, laid out as Bevy would.
fn view(app: &mut App) -> ScrollParts {
    let theme = app.world().resource::<Theme>().clone();
    let units = *app.world().resource::<UiUnits>();
    let world = app.world_mut();
    let root = world.spawn(Node::default()).id();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let mut c = Commands::new(&mut queue, world);
    let p = Ui::new(&theme, None, units).scroll(&mut c, root, "list", UiStyle::row().h(VIEW), UiStyle::column());
    queue.apply(world);
    world.entity_mut(p.view).insert(node(VIEW, 0.0));
    world.entity_mut(p.content).insert(node(CONTENT, 0.0));
    world.entity_mut(p.track).insert(node(VIEW, 0.0));
    pointer(app, p, 10.0);
    frame(app);
    p
}

/// The pointer `y` cq down the view (and the track).
fn pointer(app: &mut App, p: ScrollParts, y: f32) {
    let w = app.world_mut();
    let rel = RelativeCursorPosition { cursor_over: true, normalized: Some(Vec2::new(0.0, y / VIEW - 0.5)) };
    w.entity_mut(p.view).insert(rel);
    w.entity_mut(p.track).insert(rel);
}

fn frame(app: &mut App) {
    app.world_mut().resource_mut::<UiClock>().step(DT);
    app.update();
    let w = app.world_mut();
    w.resource_mut::<ButtonInput<MouseButton>>().clear();
    *w.resource_mut::<AccumulatedMouseScroll>() = AccumulatedMouseScroll::default();
}

fn state(app: &App) -> ScrollState {
    app.world().resource::<ScrollViews>().get("list").cloned().unwrap_or_default()
}

fn scrolled_px(app: &App, p: ScrollParts) -> f32 {
    app.world().get::<ScrollPosition>(p.view).unwrap().0.y
}

#[test]
fn the_wheel_scrolls_eased() {
    let mut app = app();
    let p = view(&mut app);
    assert!((state(&app).max - (CONTENT - VIEW)).abs() < 1e-3, "measured from the layout");
    let mut w = AccumulatedMouseScroll::default();
    w.unit = MouseScrollUnit::Line;
    w.delta = Vec2::new(0.0, -1.0);
    app.insert_resource(w);
    frame(&mut app);
    let step = tune().wheel_step;
    let first = scrolled_px(&app, p);
    assert!(first > 0.0 && first < step * PX, "eased toward one notch: {first}");
    for _ in 0..60 {
        frame(&mut app);
    }
    assert!((scrolled_px(&app, p) - step * PX).abs() < 0.5, "one notch down: {}", scrolled_px(&app, p));
}

#[test]
fn a_right_drag_pans_then_flings_to_rest() {
    let mut app = app();
    let p = view(&mut app);
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Right);
    frame(&mut app);
    assert!(state(&app).panning());
    for i in 1..=8 {
        pointer(&mut app, p, 10.0 - 0.5 * i as f32);
        frame(&mut app);
    }
    assert!((state(&app).offset - 4.0).abs() < 1e-3, "1:1 with the pointer: {}", state(&app).offset);
    assert!((scrolled_px(&app, p) - 4.0 * PX).abs() < 0.01);
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().release(MouseButton::Right);
    frame(&mut app);
    let after = app.world().resource::<ButtonInput<MouseButton>>();
    assert!(!after.just_released(MouseButton::Right), "the release is the view's");
    let s = state(&app);
    assert!(!s.panning() && s.velocity > 20.0, "flung: {}", s.velocity);
    for _ in 0..600 {
        frame(&mut app);
    }
    let s = state(&app);
    assert!(s.at_rest(), "decays to rest");
    assert!(s.offset > 8.0 && s.offset <= s.max, "glided on: {}", s.offset);
}

#[test]
fn the_right_press_is_taken_from_the_game() {
    let mut app = app();
    let p = view(&mut app);
    // Bevy's input reads it in PreUpdate; the view takes it after the UI's focus.
    app.add_systems(Update, |m: Res<ButtonInput<MouseButton>>, mut seen: Local<bool>, mut out: ResMut<Seen>| {
        *seen |= m.just_pressed(MouseButton::Right);
        out.0 = *seen;
    });
    app.init_resource::<Seen>();
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Right);
    frame(&mut app);
    assert!(!app.world().resource::<Seen>().0, "nothing after the view sees the press");
    // A locked view leaves it.
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().release(MouseButton::Right);
    frame(&mut app);
    app.world_mut().entity_mut(p.view).insert(chimera_ui::scroll::ScrollLocked);
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Right);
    frame(&mut app);
    assert!(app.world().resource::<Seen>().0, "a locked view takes nothing");
}

#[derive(Resource, Default)]
struct Seen(bool);

#[test]
fn a_pan_past_the_top_bands_and_springs_back() {
    let mut app = app();
    let p = view(&mut app);
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Right);
    frame(&mut app);
    for i in 1..=10 {
        pointer(&mut app, p, 10.0 + 1.0 * i as f32);
        frame(&mut app);
    }
    let s = state(&app);
    assert!(s.offset < 0.0 && s.offset > -tune().overscroll, "on the band: {}", s.offset);
    assert_eq!(scrolled_px(&app, p), 0.0, "Bevy's scroll stays at the top");
    let tf = app.world().get::<UiTransform>(p.content).unwrap();
    assert_eq!(tf.translation, Val2::px(0.0, -s.offset * PX), "the band is the content's shift");
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().release(MouseButton::Right);
    frame(&mut app);
    for _ in 0..300 {
        frame(&mut app);
    }
    assert_eq!(state(&app).offset, 0.0, "back at the top");
    assert_eq!(app.world().get::<UiTransform>(p.content).unwrap().translation, Val2::px(0.0, 0.0));
}

#[test]
fn a_row_scrolls_into_view() {
    let mut app = app();
    let p = view(&mut app);
    let row = app.world_mut().spawn((Node::default(), ChildOf(p.content))).id();
    // Not laid out yet: it waits.
    app.world_mut().entity_mut(row).insert(ScrollIntoView);
    frame(&mut app);
    assert!(app.world().get::<ScrollIntoView>(row).is_some(), "waits for its layout");
    app.world_mut().entity_mut(row).insert(node(2.9, 40.0));
    frame(&mut app);
    assert!(app.world().get::<ScrollIntoView>(row).is_none(), "done once laid out");
    for _ in 0..60 {
        frame(&mut app);
    }
    let want = 40.0 + 2.9 + tune().reveal_margin - VIEW;
    assert!((state(&app).offset - want).abs() < 1e-3, "its bottom in view: {}", state(&app).offset);
    // With reduced motion: at once.
    app.insert_resource(ReducedMotion(true));
    app.world_mut().entity_mut(row).insert((node(2.9, 3.0), ScrollIntoView));
    frame(&mut app);
    assert!((state(&app).offset - (3.0 - tune().reveal_margin)).abs() < 1e-3);
}

#[test]
fn the_bar_shows_the_visible_share_and_where_it_is() {
    let mut app = app();
    let p = view(&mut app);
    assert_eq!(app.world().get::<Visibility>(p.track), Some(&Visibility::Inherited), "the content overflows");
    let len = VIEW / 3.0 * PX;
    let n = app.world().get::<Node>(p.thumb).unwrap().clone();
    // Laid flat and turned upright about its centre.
    assert!(matches!(n.width, Val::Px(w) if (w - len).abs() <= 1.0), "{:?}", n.width);
    let bar = match n.height {
        Val::Px(h) => h,
        _ => panic!(),
    };
    let top_of = |n: &Node, w: f32| match n.top {
        Val::Px(t) => t - (w - bar) * 0.5,
        _ => panic!(),
    };
    assert!(top_of(&n, len).abs() <= 1.0);
    assert_eq!(app.world().get::<UiSlant>(p.thumb).unwrap().fill, Tone::Fg3, "at rest");
    app.world_mut().resource_mut::<ScrollViews>().state("list").jump(CONTENT - VIEW);
    frame(&mut app);
    let n = app.world().get::<Node>(p.thumb).unwrap().clone();
    assert!((top_of(&n, len) - (VIEW * PX - len)).abs() <= 1.0, "at the track's end");
    // Hover, then a drag: the theme's tokens.
    pointer(&mut app, p, VIEW - 1.0);
    frame(&mut app);
    assert_eq!(app.world().get::<UiSlant>(p.thumb).unwrap().fill, Tone::Fg, "hovered");
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
    frame(&mut app);
    assert_eq!(app.world().get::<UiSlant>(p.thumb).unwrap().fill, Tone::Acc, "dragged");
    pointer(&mut app, p, VIEW - 1.0 - (VIEW - VIEW / 3.0));
    frame(&mut app);
    assert!(state(&app).offset < 1.0, "dragged to the top: {}", state(&app).offset);
    // Content that fits: no bar.
    app.world_mut().entity_mut(p.content).insert(node(10.0, 0.0));
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().release(MouseButton::Left);
    frame(&mut app);
    frame(&mut app);
    assert_eq!(app.world().get::<Visibility>(p.track), Some(&Visibility::Hidden));
}
