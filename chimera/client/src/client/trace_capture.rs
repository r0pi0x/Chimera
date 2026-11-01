//! A short window of spans as a Chrome trace (feature `trace`, which
//! `profile` and `tracy` include; docs/performance.md, "See a frame
//! yourself"): `trace_capture [seconds]` in the console (default 1), or
//! `CHIMERA_TRACE_SECONDS=<s>` (with `CHIMERA_TRACE_AFTER=<s>`, default 10:
//! when, after start, it begins), records every span entered on any thread
//! for that long and writes `trace-<map>-<unix time>.json` to the working
//! directory (`CHIMERA_TRACE_DIR` names another), for https://ui.perfetto.dev
//! or `tracesum`. Unlike `profile`'s whole-run file (Bevy's `trace_chrome`:
//! about 1.5 GB for 400 frames) it stays a few MB, and it holds only span
//! names, fields and thread names: no source paths.
//!
//! `CaptureLayer` is the tracing layer (added beside the console's log
//! layer, `console::log_layer`); outside a capture it only labels each new
//! span (systems' spans are made once, at start). Events are kept as
//! complete spans and written as properly nested begin/end pairs per
//! thread, the format Bevy's `trace_chrome` writes, so `tracesum` reads
//! both.
//!
//! Known shortcuts: a span still open when the window ends is left out;
//! spans entered on several threads at once (none of Bevy's) would pair up
//! wrongly.

use std::{
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};

use bevy::{
    log::{
        tracing::{Subscriber, field, span},
        tracing_subscriber::{Layer, layer::Context, registry::LookupSpan},
    },
    prelude::*,
};

use crate::console::ConsoleAppExt;

static ACTIVE: AtomicBool = AtomicBool::new(false);
static NEXT_THREAD: AtomicU64 = AtomicU64::new(1);

/// One finished span: label, thread, start and length (µs since `epoch`).
struct Ev {
    name: Arc<str>,
    tid: u64,
    ts: f64,
    dur: f64,
}

#[derive(Default)]
struct Buffer {
    events: Vec<Ev>,
    threads: Vec<(u64, String)>,
}

fn epoch() -> Instant {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

fn buffer() -> &'static Mutex<Buffer> {
    static BUF: OnceLock<Mutex<Buffer>> = OnceLock::new();
    BUF.get_or_init(Default::default)
}

thread_local! {
    static TID: u64 = {
        let id = NEXT_THREAD.fetch_add(1, Ordering::Relaxed);
        let name = std::thread::current().name().unwrap_or("thread").to_string();
        if let Ok(mut b) = buffer().lock() {
            b.threads.push((id, name));
        }
        id
    };
}

struct Label(Arc<str>);
struct Entered(Instant);

/// The span recorder (see the module doc).
pub struct CaptureLayer;

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for CaptureLayer {
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        // As `tracing-chrome` names them: `name: field=value ...`.
        struct Fields(String);
        impl field::Visit for Fields {
            fn record_debug(&mut self, f: &field::Field, v: &dyn std::fmt::Debug) {
                if !self.0.is_empty() {
                    self.0.push(' ');
                }
                self.0 += &format!("{}={v:?}", f.name());
            }
        }
        let mut fields = Fields(String::new());
        attrs.record(&mut fields);
        let label: Arc<str> = format!("{}: {}", attrs.metadata().name(), fields.0).into();
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(Label(label));
        }
    }

    fn on_enter(&self, id: &span::Id, ctx: Context<'_, S>) {
        if !ACTIVE.load(Ordering::Relaxed) {
            return;
        }
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().replace(Entered(Instant::now()));
        }
    }

    fn on_exit(&self, id: &span::Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else { return };
        let Some(Entered(start)) = span.extensions_mut().remove::<Entered>() else {
            return;
        };
        if !ACTIVE.load(Ordering::Relaxed) {
            return;
        }
        let end = Instant::now();
        let name = span
            .extensions()
            .get::<Label>()
            .map_or_else(|| Arc::from(format!("{}: ", span.metadata().name())), |l| l.0.clone());
        let tid = TID.with(|t| *t);
        let at = |i: Instant| i.saturating_duration_since(epoch()).as_secs_f64() * 1e6;
        if let Ok(mut b) = buffer().lock() {
            b.events.push(Ev {
                name,
                tid,
                ts: at(start),
                dur: at(end) - at(start),
            });
        }
    }
}

/// The capture under way: when it ends and the file it goes to.
#[derive(Resource, Default)]
struct Capture {
    until: Option<(Instant, std::path::PathBuf)>,
    /// `CHIMERA_TRACE_SECONDS`'s window: (seconds after start, length).
    planned: Option<(f64, f64)>,
}

fn start(capture: &mut Capture, seconds: f64, map: Option<&str>) -> std::path::PathBuf {
    let dir = std::env::var_os("CHIMERA_TRACE_DIR").map_or_else(|| std::path::PathBuf::from("."), Into::into);
    let map = map.map_or("nomap", |m| m.rsplit(':').next().unwrap_or(m));
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let path = dir.join(format!("trace-{map}-{secs}.json"));
    epoch();
    if let Ok(mut b) = buffer().lock() {
        b.events.clear();
    }
    ACTIVE.store(true, Ordering::Relaxed);
    capture.until = Some((Instant::now() + std::time::Duration::from_secs_f64(seconds), path.clone()));
    path
}

/// The capture started by `CHIMERA_TRACE_SECONDS`, and the end of any.
fn run(mut capture: ResMut<Capture>, time: Res<Time<Real>>, map: Option<Res<crate::map::LoadedMapName>>) {
    if let Some((after, seconds)) = capture.planned
        && time.elapsed_secs_f64() >= after
    {
        capture.planned = None;
        let path = start(&mut capture, seconds, map.as_deref().map(|m| m.0.as_str()));
        info!("trace_capture: recording {seconds} s to {}", path.display());
    }
    let Some((until, _)) = &capture.until else { return };
    if Instant::now() < *until {
        return;
    }
    let (_, path) = capture.until.take().unwrap();
    ACTIVE.store(false, Ordering::Relaxed);
    let taken = buffer().lock().map(|mut b| (std::mem::take(&mut b.events), b.threads.clone()));
    let Ok((events, threads)) = taken else { return };
    // Written off the main thread: a second of spans is a few MB of text.
    std::thread::spawn(move || match std::fs::write(&path, chrome_json(events, &threads)) {
        Ok(()) => info!("trace_capture: wrote {}", path.display()),
        Err(e) => warn!("trace_capture: {}: {e}", path.display()),
    });
}

/// The events as a Chrome trace: thread names, then per thread begin and
/// end events nested by time (one event a line).
fn chrome_json(mut events: Vec<Ev>, threads: &[(u64, String)]) -> String {
    let mut out = String::from("[\n");
    let mut line = |v: serde_json::Value| {
        out += &v.to_string();
        out += ",\n";
    };
    for (tid, name) in threads {
        line(serde_json::json!({"ph": "M", "pid": 1, "tid": tid, "name": "thread_name", "args": {"name": name}}));
    }
    // By thread, then start, the longer (the parent) first.
    events.sort_by(|a, b| a.tid.cmp(&b.tid).then(a.ts.total_cmp(&b.ts)).then(b.dur.total_cmp(&a.dur)));
    let mut open: Vec<(u64, f64, Arc<str>)> = Vec::new();
    let close = |open: &mut Vec<(u64, f64, Arc<str>)>, upto: Option<(u64, f64)>, line: &mut dyn FnMut(serde_json::Value)| {
        while let Some((tid, end, name)) = open.last().cloned() {
            if upto.is_some_and(|(t, ts)| t == tid && end > ts) {
                break;
            }
            open.pop();
            line(serde_json::json!({"ph": "E", "pid": 1, "tid": tid, "name": &*name, "ts": end}));
        }
    };
    for e in events {
        close(&mut open, Some((e.tid, e.ts)), &mut line);
        line(serde_json::json!({"ph": "B", "pid": 1, "tid": e.tid, "name": &*e.name, "ts": e.ts}));
        open.push((e.tid, e.ts + e.dur, e.name));
    }
    close(&mut open, None, &mut line);
    out.truncate(out.trim_end_matches([',', '\n']).len());
    out += "\n]\n";
    out
}

/// `trace_capture` and `CHIMERA_TRACE_SECONDS` (see the module doc).
pub struct TraceCapturePlugin;

impl Plugin for TraceCapturePlugin {
    fn build(&self, app: &mut App) {
        let planned = std::env::var("CHIMERA_TRACE_SECONDS").ok().and_then(|s| s.parse::<f64>().ok()).map(|s| {
            let after = std::env::var("CHIMERA_TRACE_AFTER").ok().and_then(|s| s.parse().ok()).unwrap_or(10.0);
            (after, s.clamp(0.05, 30.0))
        });
        app.insert_resource(Capture { until: None, planned }).add_systems(Last, run);
        app.console_command(
            "chimera:trace_capture",
            "trace_capture [seconds]: record every span for that long (default 1, at most 30) into trace-<map>-<time>.json (a `trace` build).",
            |w, a| {
                let seconds = a.first().map_or(Ok(1.0), |s| s.parse::<f64>().map_err(|_| "trace_capture [seconds]"))?;
                let map = w.get_resource::<crate::map::LoadedMapName>().map(|m| m.0.clone());
                let mut capture = w.resource_mut::<Capture>();
                if capture.until.is_some() {
                    return Err("a capture is already running".into());
                }
                let path = start(&mut capture, seconds.clamp(0.05, 30.0), map.as_deref());
                Ok(Some(format!("recording {seconds} s to {}", path.display())))
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_come_out_nested_per_thread() {
        let ev = |name: &str, tid, ts, dur| Ev {
            name: name.into(),
            tid,
            ts,
            dur,
        };
        // Children finish (and are recorded) before their parent.
        let json = chrome_json(
            vec![ev("child: ", 1, 2.0, 3.0), ev("frame: ", 1, 1.0, 10.0), ev("next: ", 1, 12.0, 1.0), ev("other: ", 2, 0.0, 1.0)],
            &[(1, "main".into()), (2, "Compute Task Pool (0)".into())],
        );
        let v: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        let seq: Vec<String> = v
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["ph"] != "M")
            .map(|e| format!("{}{}{}", e["tid"], e["ph"].as_str().unwrap(), e["name"].as_str().unwrap()))
            .collect();
        assert_eq!(seq, ["1Bframe: ", "1Bchild: ", "1Echild: ", "1Eframe: ", "1Bnext: ", "1Enext: ", "2Bother: ", "2Eother: "]);
    }
}
