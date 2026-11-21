//! Easings, tracks and timelines (chimera_ui::motion).

use bevy::prelude::*;
use chimera_ui::{ChimeraUiPlugin, Ease, Prop, Timeline, Track, UiClock};

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

/// The prototype's curves at sampled points (its formulas, worked by hand).
#[test]
fn easings_match_the_look_book() {
    let cases: &[(Ease, f32, f32)] = &[
        (Ease::ExpoOut, 0.5, 0.96875),
        (Ease::ExpoOut, 1.0, 1.0),
        (Ease::ExpoIn, 0.5, 0.03125),
        (Ease::ExpoIn, 0.0, 0.0),
        (Ease::CubicInOut, 0.25, 0.0625),
        (Ease::CubicInOut, 0.75, 0.9375),
        (Ease::QuintInOut, 0.25, 0.015625),
        (Ease::QuintInOut, 0.75, 0.984375),
        (Ease::Back(0.9), 0.5, 0.9875),
        (Ease::Hold, 0.25, 0.4375),
        (Ease::Hold, 0.5, 0.5),
        (Ease::Hold, 0.75, 0.5625),
        (Ease::Steps(4), 0.3, 0.25),
        (Ease::Steps(4), 1.0, 1.0),
        (Ease::StepsNone(7), 0.5, 0.5),
        (Ease::StepsNone(7), 1.0, 1.0),
        (Ease::Pulse, 0.04, 0.5),
        (Ease::Pulse, 0.4, 0.367_879_4),
        (Ease::Pulse, -0.1, 0.0),
        // CSS `ease` (.25,.1,.25,1) at .5: 0.8024 (the browsers' value).
        (Ease::Bezier([0.25, 0.1, 0.25, 1.0]), 0.5, 0.8024),
    ];
    for (e, p, want) in cases {
        let got = e.sample(*p);
        assert!((got - want).abs() < 2e-4, "{e:?} at {p}: {got}, want {want}");
    }
    // The CSS curves end where they should and overshoot where they should.
    for e in [Ease::OUT, Ease::IN, Ease::IN_OUT, Ease::POP] {
        assert!(close(e.sample(0.0), 0.0) && close(e.sample(1.0), 1.0), "{e:?}");
    }
    assert!((0..100).any(|i| Ease::POP.sample(i as f32 / 100.0) > 1.0), "pop overshoots");
    assert!(Ease::OUT.sample(0.2) > 0.6, "out is fast early");
    // Names as the theme writes them.
    assert_eq!(Ease::parse("back(.9)"), Ok(Ease::Back(0.9)));
    assert_eq!(Ease::parse("bezier(.16, 1, .3, 1)"), Ok(Ease::OUT));
    assert_eq!(Ease::parse("steps_none(7)"), Ok(Ease::StepsNone(7)));
    assert_eq!(Ease::parse("expo_out"), Ok(Ease::ExpoOut));
    assert!(Ease::parse("wobble").is_err());
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(ChimeraUiPlugin);
    app.insert_resource(UiClock::manual());
    app
}

fn run(app: &mut App, fps: f32, secs: f32) {
    let n = (secs * fps).round() as usize;
    for _ in 0..n {
        app.world_mut().resource_mut::<UiClock>().step(1.0 / fps);
        app.update();
    }
}

fn toast() -> Timeline {
    Timeline::new([
        Track::new(Prop::ClipRight, 1.0, 0.0).dur(0.2).ease(Ease::OUT),
        Track::new(Prop::Y, 0.45, 0.0).at(0.08).dur(0.3).ease(Ease::Back(0.9)),
        Track::new(Prop::Opacity, 1.0, 0.0).at(0.92).dur(0.24).ease(Ease::ExpoIn),
    ])
}

/// A timeline samples absolute time: 60 and 144 steps a second land on
/// the same values at the same moments and at the end.
#[test]
fn timeline_is_frame_rate_independent() {
    let mut a = app();
    let mut b = app();
    let ea = a.world_mut().spawn(toast()).id();
    let eb = b.world_mut().spawn(toast()).id();
    // Start both at the same clock time.
    a.update();
    b.update();
    run(&mut a, 60.0, 0.5);
    run(&mut b, 144.0, 0.5);
    for p in [Prop::ClipRight, Prop::Y, Prop::Opacity] {
        let va = a.world().get::<Timeline>(ea).unwrap().value(p).unwrap();
        let vb = b.world().get::<Timeline>(eb).unwrap().value(p).unwrap();
        assert!((va - vb).abs() < 1e-3, "{p:?} at .5 s: {va} at 60 fps, {vb} at 144");
    }
    run(&mut a, 60.0, 1.0);
    run(&mut b, 144.0, 1.0);
    let ta = a.world().get::<Timeline>(ea).unwrap();
    let tb = b.world().get::<Timeline>(eb).unwrap();
    assert_eq!(ta.value(Prop::Opacity), Some(0.0));
    assert_eq!(tb.value(Prop::Opacity), Some(0.0));
    assert_eq!(ta.value(Prop::Y), tb.value(Prop::Y));
    let now = a.world().resource::<UiClock>().now;
    assert!(ta.done(now));
    // Before a track starts it holds its `from`.
    let t = toast();
    assert_eq!(t.sample(Prop::Opacity, 0.5), Some(1.0));
    assert_eq!(t.sample(Prop::Y, 0.0), Some(0.45));
    assert_eq!(t.sample(Prop::Width, 0.0), None);
}

/// Tab released mid-open folds back from where the board is, with no
/// jump; a restarted timeline plays from its start.
#[test]
fn interrupted_tracks_continue_from_where_they_are() {
    let mut app = app();
    let e = app
        .world_mut()
        .spawn((
            Node::default(),
            UiTransform::default(),
            Timeline::new([Track::new(Prop::Height, 3.05, 25.9).dur(0.12).ease(Ease::ExpoOut)]),
        ))
        .id();
    app.update();
    run(&mut app, 100.0, 0.05);
    let mid = app.world().get::<Timeline>(e).unwrap().value(Prop::Height).unwrap();
    assert!(mid > 3.05 && mid < 25.9, "half open: {mid}");
    let now = app.world().resource::<UiClock>().now;
    app.world_mut().get_mut::<Timeline>(e).unwrap().redirect(now, Prop::Height, 3.05, 0.09, Ease::CubicInOut);
    // The next frame is next to where it was, not a jump to either end.
    run(&mut app, 100.0, 0.01);
    let next = app.world().get::<Timeline>(e).unwrap().value(Prop::Height).unwrap();
    assert!((next - mid).abs() < 1.0, "folds from {mid}, got {next}");
    run(&mut app, 100.0, 0.2);
    assert_eq!(app.world().get::<Timeline>(e).unwrap().value(Prop::Height), Some(3.05));
    // The node's height follows in cq at 720 lines (12.8 px a cq).
    let node = app.world().get::<Node>(e).unwrap();
    assert_eq!(node.height, Val::Px((3.05f32 * 12.8).round()));

    // A chain restarts it.
    let now = app.world().resource::<UiClock>().now;
    let mut tl = toast();
    tl.restart(now);
    assert_eq!(tl.sample(Prop::ClipRight, tl.local(now)), Some(1.0));
}

/// A real frame counts at most `MAX_STEP` on the UI clock: one hitch slows
/// the motion for that frame, never skips its beats.
#[test]
fn a_long_frame_is_clamped() {
    let mut clock = UiClock::default();
    clock.advance_real(1.0 / 60.0);
    clock.advance_real(3.0);
    assert!(close(clock.dt, chimera_ui::motion::MAX_STEP), "the hitch counts {}", clock.dt);
    assert!((clock.now - (1.0 / 60.0 + chimera_ui::motion::MAX_STEP as f64)).abs() < 1e-6);
    clock.advance_real(-1.0);
    assert_eq!(clock.dt, 0.0, "time never runs back");
}

/// Warm after `WARM_FRAMES` steady frames in a row; a slow frame starts the
/// count again; `WARM_MAX` seconds of frames make it warm anyway.
#[test]
fn the_app_is_warm_after_steady_frames() {
    use chimera_ui::motion::{WARM_FRAMES, WARM_MAX, Warmth};
    let mut w = Warmth::default();
    let mut now = 0.0;
    // The start's slow frames, then a few steady ones and a hitch.
    for dt in [0.9, 0.4, 0.2, 0.016, 0.016, 0.3] {
        now += dt as f64;
        assert!(!w.observe(dt, now) && !w.is_warm());
    }
    assert_eq!(w.steady, 0, "the hitch started the count again");
    for i in 1..=WARM_FRAMES {
        now += 1.0 / 60.0;
        let became = w.observe(1.0 / 60.0, now);
        assert_eq!(became, i == WARM_FRAMES, "frame {i}");
    }
    assert_eq!(w.warm_at, Some(now));
    assert!(!w.observe(1.0 / 60.0, now + 1.0) && w.warm_at == Some(now), "it stays warm from then");

    // A PC whose frames are never steady: warm after WARM_MAX seconds.
    let mut slow = Warmth::default();
    let mut frames = 0;
    while !slow.observe(0.08, frames as f64 * 0.08) {
        frames += 1;
        assert!(frames < 1000);
    }
    assert!((slow.seen - WARM_MAX).abs() < 0.1, "warm at {} s", slow.seen);
}

/// In an app on a manual clock, `Warmth` counts the clock's steps.
#[test]
fn warmth_counts_the_manual_clocks_steps() {
    let mut app = app();
    for _ in 0..3 {
        app.world_mut().resource_mut::<UiClock>().step(0.5);
        app.update();
    }
    assert!(!app.world().resource::<chimera_ui::Warmth>().is_warm());
    for _ in 0..chimera_ui::motion::WARM_FRAMES {
        app.world_mut().resource_mut::<UiClock>().step(1.0 / 60.0);
        app.update();
    }
    assert!(app.world().resource::<chimera_ui::Warmth>().is_warm());
}
