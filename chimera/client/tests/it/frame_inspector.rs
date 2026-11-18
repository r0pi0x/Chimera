//! The frame inspector (`client::frame_inspector`): a frame made slow by
//! one system, in which an engine event and a log line happened, is kept
//! as a spike whose record names that system, the event and the line; the
//! F10 bundle's `perf.txt` carries it; and what it costs a frame, by
//! instruction counts.

use std::time::Duration;

use bevy::prelude::*;
use chimera_client::client::{
    feedback,
    frame_inspector::{FrameInspector, FrameInspectorPlugin, InspectMode},
    frame_metrics::FrameMetricsPlugin,
    world_report::WorldReportPlugin,
};
use chimera_core::{
    console::ConsolePlugin,
    metrics::{frame_events, work},
};

#[derive(Resource, Default)]
struct UpdateCount(u32);

/// Frame 150 notes an event and sleeps 200 ms (the others take
/// microseconds).
fn decode_a_huge_file(mut n: ResMut<UpdateCount>) {
    n.0 += 1;
    if n.0 == 150 {
        frame_events::note("test", "huge file decoded");
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn app(mode: u8, inspector: bool, extra_systems: usize) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, ConsolePlugin, FrameMetricsPlugin))
        .init_resource::<UpdateCount>()
        .add_systems(Update, decode_a_huge_file);
    for _ in 0..extra_systems {
        app.add_systems(Update, |n: Res<UpdateCount>| {
            std::hint::black_box(n.0);
        });
    }
    if inspector {
        app.add_plugins((FrameInspectorPlugin, WorldReportPlugin)).insert_resource(InspectMode(mode));
    }
    // As the client runs them (`app::run_app`): systems on this thread.
    chimera_world::harness::single_threaded_schedules(&mut app);
    app.finish();
    app.cleanup();
    app
}

/// The spike's record names the slow system first among the main world's
/// systems, its schedule first among the schedules, and the event noted
/// in it; the bundle's perf.txt holds the same.
#[test]
fn a_spike_names_its_slow_system_and_events() {
    // Timing is process-wide: each test has a process of its own.
    if !chimera_world::harness::own_process("frame_inspector::a_spike_names_its_slow_system_and_events") {
        return;
    }
    let mut app = app(2, true, 0);
    for _ in 0..200 {
        app.update();
    }
    work::set_timing(false);
    let record = {
        let i = app.world().resource::<FrameInspector>().clone();
        let s = i.0.lock().unwrap();
        assert!(s.frames.len() >= 190, "every frame on the chart: {}", s.frames.len());
        s.worst.first().cloned().expect("a spike kept")
    };
    assert!(record.ms >= 200.0, "{}", record.text());
    let systems = record.systems.as_ref().expect("systems timed (chimera_frame_inspect 2)");
    assert!(systems[0].0.ends_with("decode_a_huge_file") && systems[0].1 >= 200.0, "{}", record.text());
    assert_eq!(record.schedules[0].0, "Update", "{}", record.text());
    assert!(record.events.iter().any(|e| e.kind == "test" && e.text == "huge file decoded"), "{}", record.text());
    assert!(record.summary().contains("decode_a_huge_file"), "the hitch line's words: {}", record.summary());
    let state = feedback::collect_state(app.world_mut(), "");
    assert!(
        state.perf.contains("decode_a_huge_file") && state.perf.contains("huge file decoded"),
        "perf.txt:\n{}",
        state.perf
    );
    // world.txt: the census and the timeline, the event on it.
    assert!(state.world.contains("census:") && state.world.contains("huge file decoded"), "world.txt:\n{}", state.world);
}

/// With the default mode (timing after a spike) the first spike has no
/// system times, and timing is then on.
#[test]
fn timing_starts_at_a_spike() {
    // Timing is process-wide: each test has a process of its own.
    if !chimera_world::harness::own_process("frame_inspector::timing_starts_at_a_spike") {
        return;
    }
    let mut app = app(1, true, 0);
    for _ in 0..160 {
        app.update();
    }
    let i = app.world().resource::<FrameInspector>().clone();
    let s = i.0.lock().unwrap();
    let r = s.worst.first().expect("a spike kept");
    assert!(r.ms >= 200.0);
    assert!(r.systems.is_none() || r.frame != 149, "{}", r.text());
    assert!(work::timing(), "timed for the next 30 s");
    work::set_timing(false);
}

/// What it costs: instructions on this thread per frame of 300 trivial
/// systems, without the inspector, with it and timing off (the always-on
/// part: the wrapper's check, the frame's dot, the event take), and with
/// every system timed. Printed (`--nocapture`); the always-on part is
/// held under 200 instructions a system and 100k a frame (measured about
/// 85 a system and 6k a frame).
#[test]
fn what_the_inspector_costs() {
    // Timing is process-wide: each test has a process of its own.
    if !chimera_world::harness::own_process("frame_inspector::what_the_inspector_costs") {
        return;
    }
    if work::now().is_none() {
        eprintln!("skipping: no hardware counters ({})", chimera_core::metrics::hw_status());
        return;
    }
    let per_frame = |mut app: App, timing: bool| {
        for _ in 0..80 {
            app.update();
        }
        let mut v = Vec::new();
        for _ in 0..200 {
            work::set_timing(timing);
            let a = work::now().unwrap();
            app.update();
            v.push(work::now().unwrap() - a);
        }
        work::set_timing(false);
        v.sort();
        v[v.len() / 2]
    };
    // Frame 150's sleep is in every run alike; the median skips it.
    let bare = per_frame(app(0, false, 300), false);
    let mut wrapped = app(0, false, 300);
    work::count_systems(wrapped.world_mut());
    let wrapped = per_frame(wrapped, false);
    let off = per_frame(app(0, true, 300), false);
    let on = per_frame(app(2, true, 300), true);
    eprintln!(
        "frame inspector cost per frame of 300 systems: bare {bare}, wrapped {wrapped}, inspector off {off} (+{}), timing every system {on} (+{})",
        off as i64 - bare as i64,
        on as i64 - bare as i64
    );
    // The wrapper's share by system, the inspector's own (its systems,
    // the frame's dot) a fixed part.
    assert!(off < bare + 300 * 200 + 100_000, "always-on part: {bare} -> {off}");
}
