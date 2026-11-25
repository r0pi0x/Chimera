//! The frame inspector (docs/OBSERVABILITY.md, "The frame inspector"):
//! why a frame stuttered. Every frame's time (and the main and render
//! worlds' shares) goes into a ring of the last ~30 s; a frame over
//! `chimera_hitch_ratio` times the median (the hitch log's rule,
//! `frame_metrics`) is a spike, and its record keeps the frame's
//! breakdown: the main world's schedules (wall and CPU ms) and the render
//! world's sets, the slowest systems of each world (when systems were
//! timed: below), the engine events noted in that frame
//! (`chimera_core::metrics::frame_events`: every INFO-and-above log line,
//! asset stores' additions and frees, pipelines queued and compiled, the
//! game's own notes), the ticks run, RSS and the GPU allocator's bytes.
//! The latest 32 records and the 8 worst are kept; the worst go into the
//! F10 bundle's `perf.txt` (`report`), headed by the systems the latest
//! spikes share (`Inspector::recurring`: a stutter that comes back, which
//! one-off loads would crowd out of the worst), and each logged hitch line
//! ends with its record's summary (systems, events). Each second of the
//! last 5 minutes is a row too (`SecondRow`: frames, mean and slowest
//! frame, spikes, and for a slow second the systems that took most of it
//! when they were timed), at the end of `perf.txt`: a tester's "it went
//! from 180 to 60" shows when, for how long and in what (feedback
//! afd90ba5 had only the last 4096 frames' distribution).
//!
//! A system is named by its schedule and short name, with its module too
//! when another module's system has the same name (`Update
//! server_assets::watch` and `Update rank_toast::watch`:
//! `work::distinct_label`; a bare `Update watch` in the 2026-11-19 report
//! named either).
//!
//! Systems are timed by `metrics::work`'s wrapper (`count_systems` wraps
//! both worlds in `cleanup`; `CHIMERA_SYSTEM_TIMES=0` leaves them
//! unwrapped), switched on by `chimera_frame_inspect`: 0 never, 1 (the
//! default) while the inspector is drawn and for 30 s after any spike (a
//! stutter that comes back is caught whole), 2 always. Off, the always-on
//! part is the frame ring, the event take and the marks the hitch log has
//! anyway (docs/OBSERVABILITY.md has the measured costs).
//!
//! Shown in the debug UI's Frames tab (F8, `debugui frames`; the party apps'
//! Escape menu has a Frame inspector entry while Developer mode is on):
//! a chart of frame times (spikes red, the median and the spike line),
//! a click picks a spike, its breakdown below. `frame_spikes` prints the
//! records on the console; `chimera_stall <ms>` makes the next frame
//! that slow in a system of its own (`stall`), to see a spike's record.
//!
//! Known shortcuts: a spike's first frame has no per-system times unless
//! the inspector was open or `chimera_frame_inspect 2` (timing starts at
//! a spike). The render world's systems are its latest finished frame's
//! (pipelined: the one drawing the main world's frame before). Systems
//! that run on the task pools' threads (`CHIMERA_EXECUTOR=multi`) are timed
//! into those threads' rows and not taken. Events from other threads go
//! to whichever frame takes them first.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use bevy::{
    ecs::message::Messages,
    prelude::*,
    render::{Render, RenderApp, RenderSystems, render_resource::PipelineCache},
};
use bevy_inspector_egui::egui;
use chimera_core::metrics::{
    frame_events::{self, FrameEvent},
    memory::short_size,
    work,
};

use crate::console::{ConsoleAppExt, resource_cvar};

/// Frames kept for the chart: 30 s at 270 fps.
const FRAMES: usize = 8192;
/// Latest spike records kept, and the worst.
const LATEST: usize = 32;
const WORST: usize = 8;
/// Systems named per world in a record.
const TOP_SYSTEMS: usize = 12;
/// Systems named in the report's line of what the latest spikes share.
const RECURRING: usize = 6;
/// Seconds kept as rows (`SecondRow`): 5 minutes.
const SECONDS: usize = 300;
/// Systems named for a slow second.
const SECOND_SYSTEMS: usize = 4;
/// How long systems stay timed after a spike (`chimera_frame_inspect 1`).
const ARMED: Duration = Duration::from_secs(30);

pub struct FrameInspectorPlugin;

/// `chimera_frame_inspect`: 0 no system timing, 1 while the inspector is
/// open and after a spike, 2 always.
#[derive(Resource)]
pub struct InspectMode(pub u8);

/// A `chimera_stall` asked for: the next frame's `stall` sleeps this long.
#[derive(Resource, Default)]
struct Stall(Option<u64>);

impl Plugin for FrameInspectorPlugin {
    fn build(&self, app: &mut App) {
        let render = RenderFacts::default();
        app.insert_resource(FrameInspector::default())
            .insert_resource(InspectMode(1))
            .insert_resource(render.clone())
            .init_resource::<Stall>()
            .add_systems(
                Last,
                (
                    count_assets::<Image>.run_if(resource_exists::<Messages<AssetEvent<Image>>>),
                    count_assets::<Mesh>.run_if(resource_exists::<Messages<AssetEvent<Mesh>>>),
                    count_assets::<StandardMaterial>.run_if(resource_exists::<Messages<AssetEvent<StandardMaterial>>>),
                    count_assets::<bevy::shader::Shader>.run_if(resource_exists::<Messages<AssetEvent<bevy::shader::Shader>>>),
                    sample_memory,
                ),
            )
            .add_systems(Update, stall);
        resource_cvar::<InspectMode, u8>(
            app,
            "chimera:frame_inspect",
            "Time every system for the frame inspector (debugui frames): 0 never, 1 while it is open and for \
             30 s after a spike, 2 always.",
            |m| &mut m.0,
        );
        app.console_command(
            "chimera:frame_spikes",
            "frame_spikes [clear]: print the frame inspector's worst spikes (slow frames: schedules, systems, events).",
            |w, a| {
                let i = w.resource::<FrameInspector>().clone();
                let mut s = i.0.lock().map_err(|e| e.to_string())?;
                if a.first().is_some_and(|a| a == "clear") {
                    s.clear();
                    return Ok(Some("frame inspector cleared".into()));
                }
                Ok(Some(s.report()))
            },
        );
        app.console_command(
            "chimera:stall",
            "chimera_stall <ms>: make the next frame that much slower in a system of its own (frame_inspector::stall), \
             to see a spike's record.",
            |w, a| {
                let ms: u64 = a.first().and_then(|a| a.parse().ok()).ok_or("chimera_stall <ms>")?;
                w.resource_mut::<Stall>().0 = Some(ms.min(5000));
                Ok(None)
            },
        );
        if let Some(r) = app.get_sub_app_mut(RenderApp) {
            r.insert_resource(render)
                .add_systems(Render, render_facts.in_set(RenderSystems::Cleanup));
        }
    }

    fn cleanup(&self, app: &mut App) {
        // Every plugin has added its systems by now (but its own cleanup's):
        // each behind `work`'s wrapper, which times them while asked.
        if chimera_core::env::var("CHIMERA_SYSTEM_TIMES").as_deref() == Ok("0") {
            return;
        }
        work::count_systems(app.world_mut());
        if let Some(r) = app.get_sub_app_mut(RenderApp) {
            work::count_systems(r.world_mut());
        }
    }
}

/// One frame on the chart.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameDot {
    pub frame: u64,
    /// Seconds since the inspector began.
    pub at: f32,
    pub ms: f32,
    pub main_ms: f32,
    pub render_ms: f32,
    pub spike: bool,
    /// Events noted in the frame.
    pub events: u16,
}

/// A spike's breakdown.
#[derive(Clone, Debug, Default)]
pub struct SpikeRecord {
    pub frame: u64,
    /// Seconds since the inspector began.
    pub at: f32,
    pub ms: f64,
    pub median_ms: f64,
    pub main_ms: f64,
    pub main_cpu_ms: f64,
    /// From the main world's last schedule to the next frame's start
    /// (extraction, the window's events, waiting for the render thread).
    pub between_ms: f64,
    pub ticks: u64,
    /// The main world's schedules, slowest first: name, wall ms, CPU ms.
    pub schedules: Vec<(String, f64, f64)>,
    pub render_ms: Option<f64>,
    pub render_cpu_ms: Option<f64>,
    /// The render world's sets, slowest first.
    pub render_phases: Vec<(String, f64, f64)>,
    /// The slowest systems (own ms, runs) of each world; None: not timed.
    pub systems: Option<Vec<(String, f64, u32)>>,
    pub render_systems: Option<Vec<(String, f64, u32)>>,
    pub events: Vec<FrameEvent>,
    pub events_dropped: u32,
    /// Resident bytes at the spike, the latest GPU allocator's bytes.
    pub rss: u64,
    pub vram: Option<u64>,
}

impl SpikeRecord {
    /// Its few words for the hitch log line: the slowest systems, the
    /// events.
    pub fn summary(&self) -> String {
        let mut out = String::new();
        match &self.systems {
            Some(s) if !s.is_empty() => {
                let top: Vec<String> = s.iter().take(4).map(|(n, ms, _)| format!("{n} {ms:.2}")).collect();
                out += &format!("; systems: {}", top.join(", "));
            }
            _ => {}
        }
        if !self.events.is_empty() {
            let top: Vec<String> = self.events.iter().take(4).map(|e| format!("{}: {}", e.kind, short_text(&e.text, 80))).collect();
            out += &format!("; events ({}): {}", self.events.len(), top.join(" | "));
        }
        out
    }

    /// The whole breakdown as text (`perf.txt`, `frame_spikes`).
    pub fn text(&self) -> String {
        let mut out = format!(
            "frame {} at {:.1} s: {:.2} ms = {:.1}x median {:.2}; main world {:.2} ms (cpu {:.2}, {} ticks), between frames {:.2} ms",
            self.frame,
            self.at,
            self.ms,
            self.ms / self.median_ms.max(1e-6),
            self.median_ms,
            self.main_ms,
            self.main_cpu_ms,
            self.ticks,
            self.between_ms
        );
        if let (Some(r), Some(c)) = (self.render_ms, self.render_cpu_ms) {
            out += &format!(", render world {r:.2} ms (cpu {c:.2})");
        }
        out += &format!("; rss {}", short_size(self.rss));
        if let Some(v) = self.vram {
            out += &format!(", gpu allocator {}", short_size(v));
        }
        out += "\n";
        let rows = |out: &mut String, title: &str, rows: &[(String, f64, f64)]| {
            let top: Vec<String> = rows.iter().take(6).map(|(n, w, c)| format!("{n} {w:.2} (cpu {c:.2})")).collect();
            if !top.is_empty() {
                *out += &format!("  {title}: {}\n", top.join(", "));
            }
        };
        rows(&mut out, "schedules", &self.schedules);
        rows(&mut out, "render sets", &self.render_phases);
        let systems = |out: &mut String, title: &str, s: &Option<Vec<(String, f64, u32)>>| match s {
            Some(s) => {
                let top: Vec<String> =
                    s.iter().map(|(n, ms, k)| format!("{n} {ms:.2}{}", if *k > 1 { format!(" x{k}") } else { String::new() })).collect();
                *out += &format!("  {title} by own ms: {}\n", top.join(", "));
            }
            None => *out += &format!("  {title}: not timed (chimera_frame_inspect 2 times every frame)\n"),
        };
        systems(&mut out, "systems", &self.systems);
        if self.render_ms.is_some() {
            systems(&mut out, "render systems", &self.render_systems);
        }
        for e in &self.events {
            out += &format!("  event {}: {}\n", e.kind, e.text);
        }
        if self.events_dropped > 0 {
            out += &format!("  ({} more events)\n", self.events_dropped);
        }
        out
    }
}

/// One second of frames (module doc).
#[derive(Clone, Debug, Default)]
pub struct SecondRow {
    /// Whole seconds since the inspector began.
    pub sec: u32,
    pub frames: u32,
    /// The frames' ms summed, and the slowest.
    pub ms: f64,
    pub max_ms: f64,
    pub spikes: u32,
    /// The median frame at the second's end.
    pub median_ms: f64,
    /// A slow second's costliest systems (own ms summed over it), when
    /// they were timed.
    pub top: Vec<(String, f64)>,
    /// Systems' own ns while the second runs (row, ns).
    systems: Vec<(usize, u64)>,
}

impl SecondRow {
    /// Slow: a spike in it, or a mean frame over 1.5 times the median.
    pub fn slow(&self) -> bool {
        self.spikes > 0 || (self.frames > 0 && self.ms / self.frames as f64 > 1.5 * self.median_ms)
    }

    /// The second is over: a slow one names its systems.
    fn finish(&mut self) {
        let mut rows = std::mem::take(&mut self.systems);
        if !self.slow() {
            return;
        }
        rows.sort_by_key(|r| std::cmp::Reverse(r.1));
        self.top = rows
            .into_iter()
            .filter_map(|(row, ns)| system_label(row).map(|l| (l, ns as f64 / 1e6)))
            .take(SECOND_SYSTEMS)
            .collect();
    }

    /// Its line in the report, `ago` seconds before it.
    pub fn line(&self, ago: f32) -> String {
        let mean = self.ms / self.frames.max(1) as f64;
        let mut out = format!(
            "t-{ago:5.0} {:4} frames, mean {mean:6.2} ms ({:4.0} fps), slowest {:7.2} ms",
            self.frames,
            1000.0 / mean.max(1e-6),
            self.max_ms
        );
        if self.spikes > 0 {
            out += &format!(", {} spikes", self.spikes);
        }
        if self.slow() {
            out += " SLOW";
            if !self.top.is_empty() {
                let top: Vec<String> = self.top.iter().map(|(n, ms)| format!("{n} {ms:.2}")).collect();
                out += &format!(": {}", top.join(", "));
            }
        }
        out
    }
}

/// A timed row's name, unless it is a schedule runner's own or the frame
/// marks'.
fn system_label(row: usize) -> Option<String> {
    // Told apart from a same-named system of another module
    // (`Update server_assets::watch`, not a bare `Update watch`).
    let l = work::distinct_label(row).unwrap_or_else(|| format!("system {row}"));
    (!l.starts_with("Main ") && !l.starts_with("FixedMain ") && !l.starts_with("FrameMark(")).then_some(l)
}

fn short_text(s: &str, n: usize) -> &str {
    if s.len() <= n {
        return s;
    }
    let cut = (0..=n).rev().find(|&i| s.is_char_boundary(i)).unwrap_or(0);
    &s[..cut]
}

/// The inspector's state, shared (the frame marks fill it, the debug UI
/// and the bundle read it).
#[derive(Resource, Clone, Default)]
pub struct FrameInspector(pub Arc<Mutex<Inspector>>);

#[derive(Default)]
pub struct Inspector {
    pub frames: VecDeque<FrameDot>,
    /// The latest spikes' records, oldest first.
    pub latest: VecDeque<SpikeRecord>,
    /// The worst spikes' records, slowest first.
    pub worst: Vec<SpikeRecord>,
    /// Spikes since the start (or the last clear).
    pub spikes: u64,
    pub rss: u64,
    pub vram: Option<u64>,
    /// The spike the UI shows (its frame); until one is picked, the worst.
    pub selected: Option<u64>,
    picked: bool,
    /// The chart's span, seconds.
    pub window_s: f32,
    /// Every frame's events over the last minutes, alike ones folded
    /// (`timeline_text`).
    pub timeline: VecDeque<TimelineRow>,
    /// The last 5 minutes by second, oldest first, and the second running.
    pub seconds: VecDeque<SecondRow>,
    second: Option<SecondRow>,
    began: Option<Instant>,
    armed_until: Option<Instant>,
    /// The UI drew the inspector this recently.
    drawn: Option<Instant>,
    timed: bool,
}

impl Inspector {
    pub fn clear(&mut self) {
        self.frames.clear();
        self.latest.clear();
        self.worst.clear();
        self.spikes = 0;
        self.selected = None;
        self.picked = false;
        self.timeline.clear();
        self.seconds.clear();
        self.second = None;
    }

    /// A frame into its second's row (`at`: seconds since the start).
    pub fn on_second(&mut self, at: f32, ms: f64, median_ms: f64, spike: bool, times: &[(usize, u64, u32)]) {
        let sec = at.max(0.0) as u32;
        if self.second.as_ref().is_some_and(|c| c.sec != sec)
            && let Some(mut done) = self.second.take()
        {
            done.finish();
            self.seconds.push_back(done);
            while self.seconds.len() > SECONDS {
                self.seconds.pop_front();
            }
        }
        let c = self.second.get_or_insert_with(|| SecondRow { sec, ..default() });
        c.frames += 1;
        c.ms += ms;
        c.max_ms = c.max_ms.max(ms);
        c.spikes += spike as u32;
        c.median_ms = median_ms;
        for &(row, ns, _) in times {
            match c.systems.iter_mut().find(|r| r.0 == row) {
                Some(r) => r.1 += ns,
                None => c.systems.push((row, ns)),
            }
        }
    }

    /// The seconds as text, `now` seconds since the start (`report`).
    pub fn seconds_text(&self, now: f32) -> String {
        if self.seconds.is_empty() {
            return String::new();
        }
        let slow = self.seconds.iter().filter(|r| r.slow()).count();
        let mut out = format!(
            "\nper second (the last {} s, t = seconds before the report; {slow} slow: a spike or a mean frame over 1.5x the median, with \
             the systems that took most of it when timed):\n",
            self.seconds.len()
        );
        for r in &self.seconds {
            out += &r.line(now - r.sec as f32);
            out.push('\n');
        }
        out
    }

    /// A spike's record by frame.
    pub fn record(&self, frame: u64) -> Option<&SpikeRecord> {
        self.latest.iter().rev().chain(&self.worst).find(|r| r.frame == frame)
    }

    /// The worst spikes, for the bundle's `perf.txt`.
    pub fn report(&self) -> String {
        let now = self.began.map_or(0.0, |b| b.elapsed().as_secs_f32());
        if self.worst.is_empty() {
            return format!("frame inspector: no spikes ({} frames seen)\n{}", self.frames.len(), self.seconds_text(now));
        }
        let mut out = format!("frame inspector: {} spikes", self.spikes);
        let recurring = self.recurring(RECURRING);
        if !recurring.is_empty() {
            let timed = self.latest.iter().filter(|r| r.systems.is_some()).count();
            let rows: Vec<String> = recurring
                .iter()
                .map(|(n, ms, k)| format!("{n} {ms:.2} ms in {k}"))
                .collect();
            out += &format!(
                "; the latest {timed} timed spikes' systems, by own ms over them (in how many spikes): {}",
                rows.join(", ")
            );
        }
        out += &format!("; the {} worst:\n", self.worst.len());
        for r in &self.worst {
            out += &r.text();
        }
        out += &self.seconds_text(now);
        out
    }

    /// The systems the latest spikes have in common: each system's own ms
    /// summed over the latest timed records and how many it is in, the
    /// most first. A stutter that keeps coming back (a light rebuilt every
    /// other frame, feedback b37d546e) shows here even when one-off map
    /// loads fill the worst.
    pub fn recurring(&self, n: usize) -> Vec<(String, f64, u32)> {
        let mut by: Vec<(String, f64, u32)> = Vec::new();
        for r in &self.latest {
            for (name, ms, _) in r.systems.iter().flatten() {
                match by.iter_mut().find(|b| b.0 == *name) {
                    Some(b) => {
                        b.1 += ms;
                        b.2 += 1;
                    }
                    None => by.push((name.clone(), *ms, 1)),
                }
            }
        }
        by.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        by.truncate(n);
        by
    }

    /// Whether systems should be timed now.
    fn want_timing(&self, mode: u8, now: Instant) -> bool {
        match mode {
            0 => false,
            1 => {
                self.drawn.is_some_and(|d| now - d < Duration::from_millis(500)) || self.armed_until.is_some_and(|t| now < t)
            }
            _ => true,
        }
    }

    fn keep(&mut self, r: SpikeRecord) {
        self.spikes += 1;
        if self.worst.len() < WORST || self.worst.last().is_some_and(|w| w.ms < r.ms) {
            let at = self.worst.partition_point(|w| w.ms >= r.ms);
            self.worst.insert(at, r.clone());
            self.worst.truncate(WORST);
        }
        self.latest.push_back(r);
        while self.latest.len() > LATEST {
            self.latest.pop_front();
        }
    }
}

/// Events kept on the timeline: the last 5 minutes, at most 400 rows.
const TIMELINE_S: f32 = 300.0;
const TIMELINE_ROWS: usize = 400;

/// A timeline row: an event, or alike ones (digits aside) folded.
#[derive(Clone, Debug)]
pub struct TimelineRow {
    /// Seconds since the inspector began: the first and the latest.
    pub first: f32,
    pub last: f32,
    pub kind: &'static str,
    pub text: String,
    shape: String,
    pub count: u32,
}

impl Inspector {
    /// Put an event on the timeline, folded into a recent alike row.
    pub fn on_timeline(&mut self, at: f32, kind: &'static str, text: &str) {
        let shape: String = text.chars().map(|c| if c.is_ascii_digit() { '#' } else { c }).collect();
        if let Some(r) = self.timeline.iter_mut().rev().take(64).find(|r| r.kind == kind && r.shape == shape) {
            r.count += 1;
            r.last = at;
            r.text = text.to_string();
            return;
        }
        self.timeline.push_back(TimelineRow { first: at, last: at, kind, text: text.to_string(), shape, count: 1 });
        while self.timeline.len() > TIMELINE_ROWS || self.timeline.front().is_some_and(|r| at - r.last > TIMELINE_S) {
            self.timeline.pop_front();
        }
    }

    /// The timeline as text, times in seconds before now (the report).
    pub fn timeline_text(&self) -> String {
        let now = self.began.map_or(0.0, |b| b.elapsed().as_secs_f32());
        let mut out = format!("timeline (the last {TIMELINE_S:.0} s; t = seconds before the report; alike events folded):\n");
        for r in &self.timeline {
            let text = short_text(&r.text, 200);
            if r.count > 1 {
                out += &format!("t-{:6.1} .. t-{:6.1} x{:<4} {:<9} {text}\n", now - r.first, now - r.last, r.count, r.kind);
            } else {
                out += &format!("t-{:6.1}               {:<9} {text}\n", now - r.first, r.kind);
            }
        }
        out
    }
}

/// What the frame marks know of the frame that just ended
/// (`frame_metrics`).
pub(crate) struct FrameFacts<'a> {
    pub frame: u64,
    pub ms: f64,
    pub median_ms: f64,
    pub spike: bool,
    pub main_ms: f64,
    pub main_cpu_ms: f64,
    pub between_ms: f64,
    pub ticks: u64,
    /// Schedules in order: name, wall, CPU ms.
    pub schedules: Vec<(String, f64, f64)>,
    pub render: Option<(f64, f64)>,
    pub render_phases: &'a [(&'static str, f64, f64)],
}

/// The render world's latest frame, for the main world's records.
#[derive(Resource, Clone, Default)]
pub struct RenderFacts(Arc<Mutex<Vec<(usize, u64, u32)>>>);

/// Called by the frame marks at each frame's start with the frame before:
/// the chart's dot, the events taken, and for a spike its record;
/// returns the record's summary for the hitch log line.
pub(crate) fn frame_done(world: &World, f: FrameFacts) -> Option<String> {
    let inspector = world.get_resource::<FrameInspector>()?;
    let mode = world.get_resource::<InspectMode>().map_or(1, |m| m.0);
    let now = Instant::now();
    let taken = frame_events::take();
    let mut s = inspector.0.lock().ok()?;
    let began = *s.began.get_or_insert(now);
    let at = (now - began).as_secs_f32();
    // This thread's systems, when they were timed.
    let times = if s.timed || work::timing() { work::take_times() } else { Vec::new() };
    let timed = s.timed;
    for e in &taken.events {
        s.on_timeline(at, e.kind, &e.text);
    }
    s.frames.push_back(FrameDot {
        frame: f.frame,
        at,
        ms: f.ms as f32,
        main_ms: f.main_ms as f32,
        render_ms: f.render.map_or(0.0, |r| r.0 as f32),
        spike: f.spike,
        events: taken.events.len().min(u16::MAX as usize) as u16,
    });
    while s.frames.len() > FRAMES {
        s.frames.pop_front();
    }
    s.on_second(at, f.ms, f.median_ms, f.spike, &times);
    let mut summary = None;
    if f.spike {
        let top = |rows: Vec<(usize, u64, u32)>| {
            let mut rows = rows;
            rows.sort_by_key(|r| std::cmp::Reverse(r.1));
            // Not the schedule runners' own (the whole frame or tick
            // less what they ran) nor the frame marks'.
            rows.into_iter()
                .filter_map(|(row, ns, n)| system_label(row).map(|l| (l, ns as f64 / 1e6, n)))
                .take(TOP_SYSTEMS)
                .collect::<Vec<_>>()
        };
        let render_rows = world.get_resource::<RenderFacts>().and_then(|r| r.0.lock().ok().map(|v| v.clone()));
        let mut schedules = f.schedules;
        schedules.sort_by(|a, b| b.1.total_cmp(&a.1));
        let mut phases: Vec<(String, f64, f64)> = f.render_phases.iter().map(|&(n, w, c)| (n.to_string(), w, c)).collect();
        phases.sort_by(|a, b| b.1.total_cmp(&a.1));
        let r = SpikeRecord {
            frame: f.frame,
            at,
            ms: f.ms,
            median_ms: f.median_ms,
            main_ms: f.main_ms,
            main_cpu_ms: f.main_cpu_ms,
            between_ms: f.between_ms,
            ticks: f.ticks,
            schedules,
            render_ms: f.render.map(|r| r.0),
            render_cpu_ms: f.render.map(|r| r.1),
            render_phases: phases,
            systems: timed.then(|| top(times)),
            render_systems: if timed { render_rows.map(top) } else { None },
            events: taken.events,
            events_dropped: taken.dropped,
            rss: chimera_core::metrics::memory::usage().rss,
            vram: s.vram,
        };
        summary = Some(r.summary());
        s.keep(r);
        s.armed_until = Some(now + ARMED);
    }
    // Timing for the next frame.
    let want = s.want_timing(mode, now);
    if want != work::timing() {
        work::set_timing(want);
    }
    s.timed = want;
    summary
}

/// The render world's frame: its systems' times (when timed) for the
/// next spike's record, pipelines queued and compiled as events.
fn render_facts(facts: Res<RenderFacts>, cache: Option<Res<PipelineCache>>, mut prev: Local<(usize, usize, bool)>) {
    let timing = work::timing();
    if timing || prev.2 {
        let rows = work::take_times();
        if let Ok(mut f) = facts.0.lock() {
            *f = rows;
        }
    }
    prev.2 = timing;
    if let Some(c) = cache {
        let total = c.pipelines().count();
        let waiting = c.waiting_pipelines().count();
        let queued = total.saturating_sub(prev.0);
        let compiled = (prev.1 + queued).saturating_sub(waiting);
        if queued > 0 || compiled > 0 {
            frame_events::note("pipelines", format!("{queued} queued, {compiled} compiled, {waiting} waiting"));
        }
        (prev.0, prev.1) = (total, waiting);
    }
}

/// An asset store's additions, changes and frees this frame, as one event.
fn count_assets<A: Asset>(mut events: MessageReader<AssetEvent<A>>) {
    if events.is_empty() {
        return;
    }
    let (mut added, mut modified, mut removed) = (0u32, 0u32, 0u32);
    for e in events.read() {
        match e {
            AssetEvent::Added { .. } => added += 1,
            AssetEvent::Modified { .. } => modified += 1,
            AssetEvent::Removed { .. } | AssetEvent::Unused { .. } => removed += 1,
            _ => {}
        }
    }
    if added + removed > 0 || modified > 8 {
        let name = std::any::type_name::<A>().rsplit("::").next().unwrap_or("asset");
        frame_events::note("assets", format!("{name}: {added} added, {modified} changed, {removed} freed"));
    }
}

/// RSS each second and the GPU allocator's bytes every 5 s (its report
/// lists every allocation: not for every frame).
fn sample_memory(
    inspector: Res<FrameInspector>,
    device: Option<Res<bevy::render::renderer::RenderDevice>>,
    mut last: Local<Option<(Instant, u32)>>,
) {
    let now = Instant::now();
    if last.is_some_and(|(t, _)| now - t < Duration::from_secs(1)) {
        return;
    }
    let n = last.map_or(0, |l| l.1 + 1);
    *last = Some((now, n));
    let rss = chimera_core::metrics::memory::usage().rss;
    let vram = (n % 5 == 0)
        .then(|| device.and_then(|d| d.wgpu_device().generate_allocator_report()).map(|r| r.total_allocated_bytes))
        .flatten();
    if let Ok(mut s) = inspector.0.lock() {
        s.rss = rss;
        if vram.is_some() {
            s.vram = vram;
        }
    }
}

/// `chimera_stall`'s slow frame.
fn stall(mut asked: ResMut<Stall>) {
    if let Some(ms) = asked.0.take() {
        frame_events::note("stall", format!("chimera_stall {ms} ms"));
        std::thread::sleep(Duration::from_millis(ms));
    }
}

// ------------------------------------------------------------- the tab

/// The debug UI's Frames tab.
pub fn tab(ui: &mut egui::Ui, world: &mut World) {
    let Some(inspector) = world.get_resource::<FrameInspector>().cloned() else {
        ui.label("No frame inspector in this app.");
        return;
    };
    let ratio = world.get_resource::<super::frame_metrics::HitchRatio>().map_or(2.0, |h| h.0);
    let mode = world.get_resource::<InspectMode>().map_or(1, |m| m.0);
    {
        let Ok(mut s) = inspector.0.lock() else { return };
        s.drawn = Some(Instant::now());
        if s.window_s == 0.0 {
            s.window_s = 15.0;
        }
        ui.horizontal(|ui| {
            ui.label(format!(
                "{} spikes (frames over {ratio}x the median); systems {}",
                s.spikes,
                if s.timed { "timed" } else { "not timed" }
            ));
            for w in [10.0, 15.0, 30.0] {
                if ui.selectable_label(s.window_s == w, format!("{w:.0} s")).clicked() {
                    s.window_s = w;
                }
            }
        });
        if !s.picked || s.selected.is_some_and(|f| s.record(f).is_none()) {
            s.selected = s.worst.first().map(|r| r.frame);
        }
        if let Some(f) = chart(ui, &s, ratio) {
            s.selected = Some(f);
            s.picked = true;
        }
        ui.horizontal_wrapped(|ui| {
            ui.label("Worst:");
            let picks: Vec<(u64, f64)> = s.worst.iter().map(|r| (r.frame, r.ms)).collect();
            for (f, ms) in picks {
                if ui.selectable_label(s.selected == Some(f), format!("{ms:.0} ms (#{f})")).clicked() {
                    s.selected = Some(f);
                    s.picked = true;
                }
            }
        });
        ui.separator();
        match s.selected.and_then(|f| s.record(f)) {
            Some(r) => details(ui, r),
            None => {
                ui.label("No spike yet: a frame slower than the spike line shows here (chimera_stall 100 makes one).");
            }
        }
    }
    ui.separator();
    ui.horizontal(|ui| {
        for (label, line) in [("Time systems: after spikes", "chimera_frame_inspect 1"), ("always", "chimera_frame_inspect 2"), ("never", "chimera_frame_inspect 0")] {
            let on = line.ends_with(&mode.to_string());
            if ui.selectable_label(on, label).on_hover_text(line).clicked() {
                world.resource_mut::<crate::console::Console>().submit(line);
            }
        }
        if ui.button("Stall 100 ms").on_hover_text("chimera_stall 100").clicked() {
            world.resource_mut::<crate::console::Console>().submit("chimera_stall 100");
        }
        if ui.button("Clear").on_hover_text("frame_spikes clear").clicked() {
            world.resource_mut::<crate::console::Console>().submit("frame_spikes clear");
        }
    });
}

/// The frame-time chart over the window; returns the spike clicked.
fn chart(ui: &mut egui::Ui, s: &Inspector, ratio: f32) -> Option<u64> {
    let width = ui.available_width().max(100.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, 150.0), egui::Sense::click());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, egui::Color32::from_black_alpha(170));
    let end = s.frames.back().map_or(0.0, |f| f.at);
    let start = end - s.window_s;
    let shown: Vec<&FrameDot> = s.frames.iter().filter(|f| f.at >= start).collect();
    let mut sorted: Vec<f32> = shown.iter().map(|f| f.ms).collect();
    sorted.sort_by(f32::total_cmp);
    let median = sorted.get(sorted.len() / 2).copied().unwrap_or(16.7);
    let top = shown.iter().map(|f| f.ms).fold(median * ratio * 1.5, f32::max).clamp(20.0, 500.0);
    let x = |at: f32| rect.left() + (at - start) / s.window_s.max(1.0) * rect.width();
    let y = |ms: f32| rect.bottom() - (ms / top).min(1.0) * (rect.height() - 14.0);
    let label = |at: f32, text: String, color: egui::Color32| {
        painter.text(egui::pos2(rect.left() + 4.0, at - 1.0), egui::Align2::LEFT_BOTTOM, text, egui::FontId::proportional(11.0), color);
    };
    for (ms, text, color) in [
        (median, format!("median {median:.1} ms"), egui::Color32::from_gray(150)),
        (median * ratio, format!("spike: {ratio}x"), egui::Color32::from_rgb(200, 110, 90)),
    ] {
        painter.line_segment([egui::pos2(rect.left(), y(ms)), egui::pos2(rect.right(), y(ms))], egui::Stroke::new(1.0, color));
        label(y(ms), text, color);
    }
    let line = |pick: fn(&FrameDot) -> f32| -> Vec<egui::Pos2> { shown.iter().map(|f| egui::pos2(x(f.at), y(pick(f)))).collect() };
    painter.line(line(|f| f.render_ms), egui::Stroke::new(1.0, egui::Color32::from_rgb(110, 150, 240)));
    painter.line(line(|f| f.main_ms), egui::Stroke::new(1.0, egui::Color32::from_rgb(90, 220, 120)));
    painter.line(line(|f| f.ms), egui::Stroke::new(1.5, egui::Color32::WHITE));
    for f in shown.iter().filter(|f| f.events > 0) {
        painter.line_segment(
            [egui::pos2(x(f.at), rect.bottom() - 6.0), egui::pos2(x(f.at), rect.bottom())],
            egui::Stroke::new(1.0, egui::Color32::YELLOW),
        );
    }
    for f in shown.iter().filter(|f| f.spike) {
        let picked = s.selected == Some(f.frame);
        let c = egui::pos2(x(f.at), y(f.ms));
        if picked {
            painter.line_segment([egui::pos2(c.x, rect.top()), egui::pos2(c.x, rect.bottom())], egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 200, 80)));
        }
        painter.circle_filled(c, if picked { 5.0 } else { 3.5 }, egui::Color32::from_rgb(240, 70, 60));
    }
    painter.text(
        egui::pos2(rect.right() - 4.0, rect.top() + 2.0),
        egui::Align2::RIGHT_TOP,
        "white frame, green main world, blue render world, yellow ticks: events; click a red spike",
        egui::FontId::proportional(11.0),
        egui::Color32::from_gray(160),
    );
    let at = resp.interact_pointer_pos().filter(|_| resp.clicked())?;
    shown
        .iter()
        .filter(|f| f.spike && s.record(f.frame).is_some())
        .min_by(|a, b| (x(a.at) - at.x).abs().total_cmp(&(x(b.at) - at.x).abs()))
        .filter(|f| (x(f.at) - at.x).abs() < 24.0)
        .map(|f| f.frame)
}

/// A spike's breakdown: bars by share of the frame.
fn details(ui: &mut egui::Ui, r: &SpikeRecord) {
    ui.strong(format!(
        "Frame #{} at {:.1} s: {:.2} ms ({:.1}x the median {:.2})",
        r.frame,
        r.at,
        r.ms,
        r.ms / r.median_ms.max(1e-6),
        r.median_ms
    ));
    let mut info = format!(
        "main world {:.2} ms (cpu {:.2}, {} ticks), between frames {:.2} ms",
        r.main_ms, r.main_cpu_ms, r.ticks, r.between_ms
    );
    if let Some(m) = r.render_ms {
        info += &format!(", render world {m:.2} ms");
    }
    info += &format!("; rss {}", short_size(r.rss));
    if let Some(v) = r.vram {
        info += &format!(", gpu allocator {}", short_size(v));
    }
    ui.label(info);
    let bars = |ui: &mut egui::Ui, title: &str, rows: Vec<(String, f64, String)>| {
        if rows.is_empty() {
            return;
        }
        ui.label(egui::RichText::new(title).strong());
        let most = rows.iter().map(|r| r.1).fold(0.01, f64::max);
        egui::Grid::new(title).num_columns(3).striped(true).show(ui, |ui| {
            for (name, ms, extra) in rows {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(120.0, 12.0), egui::Sense::hover());
                ui.painter().rect_filled(
                    egui::Rect::from_min_size(rect.min, egui::vec2((ms / most) as f32 * rect.width(), rect.height())),
                    1.0,
                    egui::Color32::from_rgb(220, 120, 70),
                );
                ui.monospace(format!("{ms:7.2} {extra}"));
                ui.label(name);
                ui.end_row();
            }
        });
    };
    let sched = |rows: &[(String, f64, f64)]| rows.iter().take(6).map(|(n, w, c)| (n.clone(), *w, format!("cpu {c:6.2}"))).collect::<Vec<_>>();
    let systems = |rows: &Option<Vec<(String, f64, u32)>>| {
        rows.iter().flatten().map(|(n, ms, k)| (n.clone(), *ms, if *k > 1 { format!("x{k:<4}") } else { "     ".into() })).collect::<Vec<_>>()
    };
    bars(ui, "Main world schedules (ms)", sched(&r.schedules));
    match &r.systems {
        Some(_) => bars(ui, "Slowest systems (own ms)", systems(&r.systems)),
        None => {
            ui.label("Systems weren't timed for this frame (they are from a spike on for 30 s, while this tab is open, or always with chimera_frame_inspect 2).");
        }
    }
    bars(ui, "Render world sets (ms)", sched(&r.render_phases));
    bars(ui, "Render world systems (own ms)", systems(&r.render_systems));
    if !r.events.is_empty() {
        ui.label(egui::RichText::new(format!("Events in this frame ({})", r.events.len() + r.events_dropped as usize)).strong());
        for e in &r.events {
            ui.monospace(format!("{:<9} {}", e.kind, e.text));
        }
    } else {
        ui.label("No engine events noted in this frame.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The report's head names what the latest spikes share, though the
    /// worst are a load.
    #[test]
    fn the_report_names_what_the_latest_spikes_share() {
        let mut i = Inspector::default();
        let spike = |ms: f64, systems: &[(&str, f64)]| SpikeRecord {
            ms,
            median_ms: 10.0,
            systems: Some(systems.iter().map(|(n, m)| (n.to_string(), *m, 1)).collect()),
            ..default()
        };
        i.keep(spike(400.0, &[("Update finish_map_load", 380.0)]));
        for _ in 0..20 {
            i.keep(spike(28.0, &[("Update relight", 20.0), ("Update hud", 1.0)]));
        }
        let rows = i.recurring(2);
        assert_eq!(rows[0].0, "Update relight");
        assert_eq!(rows[0].2, 20);
        let report = i.report();
        let head = report.lines().next().unwrap();
        assert!(head.contains("Update relight 400.00 ms in 20"), "{report}");
    }

    /// Each second is a row; a slow one says so, an ordinary one doesn't;
    /// 5 minutes are kept (feedback afd90ba5).
    #[test]
    fn the_seconds_keep_five_minutes_and_mark_the_slow_ones() {
        let mut i = Inspector::default();
        for k in 0..(400 * 60) {
            let at = k as f32 / 60.0;
            let slow = (100.0..103.0).contains(&at);
            i.on_second(at, if slow { 16.7 } else { 5.0 }, 5.0, false, &[]);
        }
        i.on_second(400.0, 5.0, 5.0, false, &[]);
        assert_eq!(i.seconds.len(), SECONDS);
        let text = i.seconds_text(400.0);
        assert_eq!(text.matches(" SLOW").count(), 3, "{text}");
        assert!(text.contains("60 frames, mean  16.70 ms (  60 fps)"), "{text}");
        assert!(text.contains("60 frames, mean   5.00 ms ( 200 fps)"), "{text}");
    }
}
