//! A session's diagnostics in a running app (docs/plans/active/
//! feedback-reports.md, P3b), over `chimera_assets::feedback::session`
//! (the rolling log on disk, the unclean-exit marker, crash bundles):
//!
//! - `start`: the process's session (`<data_local>/chimera/logs/
//!   <prefix>-<time>.log`), the panic hook, the native-crash filter
//!   (Windows); the client's `app::run` and the dedicated server's `run`
//!   call it, never a test's app (they start their own in a scratch
//!   folder: `Session::start`). The session's lines come from `log_layer`
//!   (every `tracing` event at INFO and above, Bevy's `LogPlugin::
//!   custom_layer`) and from the console (`DiagnosticsPlugin`: every
//!   line it prints, typed lines too).
//! - **Health lines** (`HealthLine`): every `chimera:health_interval`
//!   seconds (10 in a game, 20 on a server) one line into the session's
//!   log, `health up=<s> rss=<M> peak=<M> avail=<free>/<total>
//!   commit=<used>/<limit> tick=<p50>/<p99>ms ...` then what the layers
//!   above add (`HealthParts`: the client's frame times, GPU memory, net,
//!   map and rung; the server's players, its ticks' instructions and its
//!   map). Keys and values have no spaces (`HealthLine::parse` reads one
//!   back); `HealthExtras` add lines after it (the server's `tick_stats`).
//!   `HealthSettings::print` also prints them on the console (the
//!   dedicated server's stdout, its own log; then they are written with
//!   no session too). `chimera:health` prints one now.
//! - **Context** (`ContextParts`): what the game is doing, refreshed each
//!   second into the session (`Session::set_context`: a crash's
//!   `game.txt`, logged when it changes).
//! - A clean exit (`AppExit`) ends the session (`Session::end`: the
//!   marker goes): in `Last`, or with the quick exit
//!   (`chimera_core::shutdown`) as its `Stage::Persist` step "session
//!   log", after the exit's own lines; a panic, a native crash or a power loss leaves it for
//!   the next start (`chimera_client::client::crash`).

use std::sync::Arc;

use bevy::prelude::*;

pub use chimera_assets::feedback::session::{self, CrashPolicy, Previous, Session, SessionOptions};

use crate::{
    console::{Console, ConsoleAppExt, resource_cvar},
    metrics::memory::{short_size, usage},
    shutdown::{ExitMode, ShutdownAppExt, Stage, asked_to_exit},
};

/// The process's session, in the world (absent: none runs).
#[derive(Resource, Clone)]
pub struct SessionLog(pub Arc<Session>);

/// How often health lines are written, and whether on the console too.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct HealthSettings {
    /// Seconds (0: never).
    pub interval: f32,
    pub print: bool,
}

impl Default for HealthSettings {
    fn default() -> Self {
        Self { interval: 10.0, print: false }
    }
}

/// One health line: `health key=value ...`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HealthLine {
    pub parts: Vec<(String, String)>,
}

impl HealthLine {
    /// A key and its value (spaces become `_`).
    pub fn push(&mut self, key: &str, value: impl std::fmt::Display) {
        let clean = |s: &str| s.trim().replace(char::is_whitespace, "_").replace('=', ":");
        let v = clean(&value.to_string());
        self.parts.push((clean(key), if v.is_empty() { "-".into() } else { v }));
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.parts.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn text(&self) -> String {
        let mut s = String::from("health");
        for (k, v) in &self.parts {
            s.push(' ');
            s.push_str(k);
            s.push('=');
            s.push_str(v);
        }
        s
    }

    /// A line back (with or without the log's time in front); None when it
    /// isn't one.
    pub fn parse(line: &str) -> Option<Self> {
        let (head, rest) = line.split_once("health ")?;
        if !(head.is_empty() || head.ends_with(' ')) {
            return None;
        }
        let mut h = Self::default();
        for word in rest.split_whitespace() {
            let (k, v) = word.split_once('=')?;
            h.parts.push((k.to_string(), v.to_string()));
        }
        Some(h)
    }
}

/// What layers above add to a health line.
#[derive(Resource, Default, Clone)]
pub struct HealthParts(pub Vec<fn(&mut World, &mut HealthLine)>);

/// Lines a layer writes after each health line (the dedicated server's
/// `tick_stats`), each a one-line text or None.
#[derive(Resource, Default, Clone)]
pub struct HealthExtras(pub Vec<fn(&mut World) -> Option<String>>);

/// Add a line after each health line (once per function).
pub fn add_health_extra(app: &mut App, extra: fn(&mut World) -> Option<String>) {
    let mut extras = app.world_mut().get_resource_or_init::<HealthExtras>();
    if !extras.0.iter().any(|p| std::ptr::fn_addr_eq(*p, extra)) {
        extras.0.push(extra);
    }
}

/// What layers above say the game is doing (each a few `key: value`
/// lines).
#[derive(Resource, Default, Clone)]
pub struct ContextParts(pub Vec<fn(&World) -> String>);

/// Add a health part (once per function).
pub fn add_health_part(app: &mut App, part: fn(&mut World, &mut HealthLine)) {
    let mut parts = app.world_mut().get_resource_or_init::<HealthParts>();
    if !parts.0.iter().any(|p| std::ptr::fn_addr_eq(*p, part)) {
        parts.0.push(part);
    }
}

/// Add a context part (once per function).
pub fn add_context_part(app: &mut App, part: fn(&World) -> String) {
    let mut parts = app.world_mut().get_resource_or_init::<ContextParts>();
    if !parts.0.iter().any(|p| std::ptr::fn_addr_eq(*p, part)) {
        parts.0.push(part);
    }
}

/// Start the process's session (a binary's, once): the log, the marker,
/// the panic hook, the native-crash filter. The dead sessions found (the
/// client offers them). None when the folder can't be written or a
/// session already runs.
pub fn start(options: SessionOptions) -> Option<(Arc<Session>, Vec<Previous>)> {
    if let Some(s) = session::current() {
        return Some((s.clone(), Vec::new()));
    }
    match Session::start(options) {
        Ok((s, previous)) => {
            s.make_current();
            session::install_panic_hook();
            session::native::install(&s);
            Some((s, previous))
        }
        Err(e) => {
            eprintln!("diagnostics: no session log: {e}");
            None
        }
    }
}

/// The data folder sessions live in: `CHIMERA_DATA_DIR`, else
/// `<data_local>/chimera` (its old name while only that exists).
pub fn data_root() -> Option<std::path::PathBuf> {
    if let Some(d) = crate::env::var_os("CHIMERA_DATA_DIR") {
        return Some(d.into());
    }
    Some(crate::env::user_folder(&dirs::data_local_dir()?))
}

/// The `tracing` layer into the process's session (INFO and above).
pub struct SessionLayer;

impl<S: bevy::log::tracing::Subscriber> bevy::log::tracing_subscriber::Layer<S> for SessionLayer {
    fn on_event(&self, event: &bevy::log::tracing::Event<'_>, _: bevy::log::tracing_subscriber::layer::Context<'_, S>) {
        use bevy::log::tracing::Level;
        let Some(session) = session::current() else { return };
        let meta = event.metadata();
        let level = match *meta.level() {
            Level::ERROR => "ERROR",
            Level::WARN => "WARN",
            Level::INFO => "INFO",
            _ => return,
        };
        struct Fields(String, String);
        impl bevy::log::tracing::field::Visit for Fields {
            fn record_debug(&mut self, f: &bevy::log::tracing::field::Field, v: &dyn std::fmt::Debug) {
                if f.name() == "message" {
                    self.0 = format!("{v:?}");
                } else {
                    self.1 += &format!(" {}={v:?}", f.name());
                }
            }
        }
        let mut fields = Fields(String::new(), String::new());
        event.record(&mut fields);
        session.line(&format!("{level} {}: {}{}", meta.target(), fields.0, fields.1));
    }
}

/// `LogPlugin::custom_layer` for an app with nothing else to add (the
/// dedicated server).
pub fn log_layer(_: &mut App) -> Option<bevy::log::BoxedLayer> {
    Some(Box::new(SessionLayer))
}

/// Health lines, context, the console's lines and the clean exit into
/// the world's `SessionLog` (nothing without one).
pub struct DiagnosticsPlugin {
    pub health: HealthSettings,
}

impl Default for DiagnosticsPlugin {
    fn default() -> Self {
        Self { health: HealthSettings::default() }
    }
}

impl Plugin for DiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.health)
            .init_resource::<HealthParts>()
            .init_resource::<HealthExtras>()
            .init_resource::<ContextParts>()
            .add_systems(Update, (console_lines, health, context))
            .add_systems(Last, clean_exit)
            // With the quick exit (`ShutdownPlugin`) the session ends as a
            // `Persist` step, so the exit's own lines reach its log first.
            .shutdown_flush(Stage::Persist, "session log", end_session);
        if app.world().contains_resource::<Console>() {
            resource_cvar::<HealthSettings, f32>(
                app,
                "chimera:health_interval",
                "Seconds between health lines in the session's log (memory, frame and tick times, net, map; \
                 0: none).",
                |h| &mut h.interval,
            );
            app.console_command(
                "chimera:health",
                "health: the health line now (memory, frame and tick times, net, map), as the session's log gets it.",
                |w, _| Ok(Some(health_line(w).text())),
            );
        }
    }
}

/// The health line now: memory, ticks, then the `HealthParts`.
pub fn health_line(world: &mut World) -> HealthLine {
    let mut h = HealthLine::default();
    let up = world.get_resource::<Time<Real>>().map_or(0.0, |t| t.elapsed_secs_f64());
    h.push("up", format!("{up:.0}s"));
    let u = usage();
    h.push("rss", short_size(u.rss));
    h.push("peak", short_size(u.peak));
    h.push("avail", format!("{}/{}", short_size(u.available), short_size(u.total)));
    h.push("commit", format!("{}/{}", short_size(u.commit), short_size(u.commit_limit)));
    if let Some(m) = world.get_resource::<crate::metrics::TickMetrics>()
        && let Some(p) = m.work.wall_ms.ring.percentiles()
    {
        h.push("tick", format!("{:.2}/{:.2}ms", p.p50, p.p99));
        if let Some(i) = m.work.instructions.ring.percentiles().filter(|i| i.max > 0.0) {
            h.push("tick_instr", crate::metrics::dist::short(i.p50));
        }
    }
    let parts = world.get_resource::<HealthParts>().cloned().unwrap_or_default();
    for part in parts.0 {
        part(world, &mut h);
    }
    h
}

fn health(world: &mut World, mut last: Local<Option<f64>>) {
    let session = world.get_resource::<SessionLog>().map(|s| s.0.clone());
    let settings = *world.resource::<HealthSettings>();
    if session.is_none() && !settings.print {
        return;
    }
    let now = world.get_resource::<Time<Real>>().map_or(0.0, |t| t.elapsed_secs_f64());
    if settings.interval <= 0.0 {
        return;
    }
    if last.is_some_and(|l| now - l < settings.interval as f64) {
        return;
    }
    *last = Some(now);
    let mut lines = vec![health_line(world).text()];
    let extras = world.get_resource::<HealthExtras>().cloned().unwrap_or_default();
    lines.extend(extras.0.iter().filter_map(|e| e(world)));
    for (i, line) in lines.into_iter().enumerate() {
        if let Some(s) = &session {
            if i == 0 { s.health(&line) } else { s.line(&line) }
        }
        if settings.print
            && let Some(mut c) = world.get_resource_mut::<Console>()
        {
            c.info(line);
        }
    }
}

fn context(world: &mut World, mut last: Local<Option<f64>>) {
    let Some(session) = world.get_resource::<SessionLog>().map(|s| s.0.clone()) else { return };
    let now = world.get_resource::<Time<Real>>().map_or(0.0, |t| t.elapsed_secs_f64());
    if last.is_some_and(|l| now - l < 1.0) {
        return;
    }
    *last = Some(now);
    let parts = world.get_resource::<ContextParts>().cloned().unwrap_or_default();
    let text: String = parts.0.iter().map(|p| p(world)).collect();
    session.set_context(&text);
}

/// Every line the console prints (typed ones too) into the log.
fn console_lines(session: Option<Res<SessionLog>>, console: Option<Res<Console>>, mut printed: Local<u64>) {
    let (Some(session), Some(console)) = (session, console) else { return };
    let new = console.printed.saturating_sub(*printed) as usize;
    if new == 0 {
        return;
    }
    *printed = console.printed;
    let lines = &console.output;
    for line in &lines[lines.len().saturating_sub(new)..] {
        // The health lines printed are in the log already.
        if line.text.starts_with("health ") || line.text.starts_with("tick_stats ") {
            continue;
        }
        session.0.line(&format!("console {}", line.text));
    }
}

fn clean_exit(session: Option<Res<SessionLog>>, quick: Option<Res<ExitMode>>, mut exits: MessageReader<AppExit>) {
    let Some(session) = session else { return };
    if let Some(exit) = exits.read().last() {
        if quick.is_some() {
            // `ShutdownPlugin`'s steps end it (`end_session`).
            return;
        }
        session.0.line(&format!("exit: {exit:?}"));
        session.0.end();
    }
}

/// The `Persist` step of the quick exit: the exit line, then the session
/// ended (its log synced, the marker gone).
fn end_session(world: &mut World) {
    let Some(session) = world.get_resource::<SessionLog>() else { return };
    let code = asked_to_exit(world).unwrap_or(0);
    session.0.line(&format!("exit: code {code}"));
    session.0.end();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With the quick exit the session ends as a `Persist` step: the exit
    /// line, "session ended" last, the marker gone, before the exit call.
    #[test]
    fn a_quick_exit_ends_the_session_in_its_persist_step() {
        use crate::shutdown::{ExitCall, ShutdownPlugin};
        let root = std::env::temp_dir().join(format!("chimera_core_quick_exit_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (session, _) = Session::start(SessionOptions {
            root: root.clone(),
            prefix: "session".into(),
            app: "test".into(),
            build: "test".into(),
            net_version: "net".into(),
        })
        .unwrap();
        let marker = session.marker_path().to_path_buf();
        let log = session.log_path().to_path_buf();
        let mut app = App::new();
        let (exit, codes) = ExitCall::record();
        app.add_plugins((MinimalPlugins, crate::console::ConsolePlugin, ShutdownPlugin))
            .add_plugins(DiagnosticsPlugin::default())
            .insert_resource(exit)
            .insert_resource(SessionLog(session));
        app.finish();
        assert_eq!(
            app.world().resource::<crate::shutdown::ShutdownFlush>().names(),
            ["session log", "logs"]
        );
        app.update();
        assert!(marker.exists());
        app.world_mut().write_message(AppExit::from_code(2));
        app.update();
        assert_eq!(*codes.lock().unwrap(), [2]);
        assert!(!marker.exists(), "the marker goes with the session");
        let text = std::fs::read_to_string(&log).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines.iter().any(|l| l.ends_with("exit: code 2")), "{text}");
        assert!(lines.last().unwrap().ends_with("session ended"), "{text}");
        assert_eq!(lines.iter().filter(|l| l.ends_with("session ended")).count(), 1, "{text}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn health_lines_read_back() {
        let mut h = HealthLine::default();
        h.push("rss", "812M");
        h.push("map", "cs source:de dust2");
        h.push("gpu", "");
        assert_eq!(h.text(), "health rss=812M map=cs_source:de_dust2 gpu=-");
        let back = HealthLine::parse(&format!("12:00:00.000 {}", h.text())).unwrap();
        assert_eq!(back, h);
        assert_eq!(HealthLine::parse("unhealthy x=1"), None);
    }
}
