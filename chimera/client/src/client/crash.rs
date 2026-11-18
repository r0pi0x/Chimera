//! Crashes and the sessions that died, in a game (docs/plans/active/
//! feedback-reports.md, P3b), over `chimera_core::diagnostics`:
//!
//! - **Previous sessions**: `app::run` starts the process's session and
//!   hands the dead ones it found (`diagnostics::Previous`: a crash, a
//!   freeze killed, a power loss) to `CrashReports`. A few frames in, once
//!   config.cfg has set `cl_crash_reports` (`chimera:cl_crash_reports`:
//!   `ask` by default, `always`, `never`; archived), they are sent
//!   (`always`), settled unsent (`never`), or offered (`ask`:
//!   `CrashReports::offer`, which an app draws: the party's Signal notice
//!   on the main menu, `party_kit::crash_signal`; "The last session ended
//!   unexpectedly. Send a report?"). Send puts each report in the
//!   feedback outbox (its held bundle, else one from its log's tail and
//!   health lines, its minidump) and wakes the outbox
//!   (`feedback::Outbox::wake`); Not now settles them unsent; Always sends
//!   and sets the cvar. `CrashAction` is the notice's clicks.
//! - **This session**: the panic hook's and the native filter's crash
//!   bundles follow the cvar (`session::set_policy`); a render device lost
//!   or out of memory (Bevy's `RenderErrorHandler`, replaced in `finish`)
//!   writes one too (`render error: <type>: <description>`) and quits as
//!   Bevy's own handler does.
//! - **Health and context** (`diagnostics::HealthParts`,
//!   `ContextParts`): the frame time p50/p99 (`FrameMetrics`), the GPU
//!   allocator's bytes (wgpu's report: what this process holds of VRAM;
//!   the driver's total isn't asked), the net (ping, loss, prediction
//!   resyncs, clock jumps) when connected, the map and the local player's
//!   rung; the context says the map, the minigame, the rung, the server's
//!   name, the position. The PC (`system.txt`) is logged once the
//!   adapter is known.
//!
//! Console: `crash_report [send|always|dismiss]` (no argument: what
//! waits), `crash_test <panic|native|render>` (makes that crash, to see
//! the report: native writes through a null pointer).

use bevy::prelude::*;
use chimera_core::diagnostics::{
    ContextParts, CrashPolicy, HealthLine, Previous, SessionLog, add_context_part, add_health_part, session,
};

use crate::console::{Console, ConsoleAppExt};

/// Frames before the dead sessions are dealt with (config.cfg has run).
const SETTLE_FRAMES: u32 = 4;

/// The dead sessions and what the tester said.
#[derive(Resource, Default)]
pub struct CrashReports {
    /// Found at this start (newest first), until settled.
    pub previous: Vec<Previous>,
    /// The notice shows (policy ask, sessions waiting).
    pub offer: bool,
    /// Dealt with (the policy applied).
    pub decided: bool,
    /// Whether the newest crashed (a panic, a native crash) rather than
    /// just ended.
    pub crashed: bool,
    /// The last word after an action ("Crash report queued").
    pub said: Option<String>,
    frames: u32,
}

impl CrashReports {
    /// The notice's line.
    pub fn headline(&self) -> &'static str {
        if self.crashed { "The last session crashed. Send a report?" } else { "The last session ended unexpectedly. Send a report?" }
    }
}

/// `cl_crash_reports`.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct CrashSetting(pub String);

impl Default for CrashSetting {
    fn default() -> Self {
        Self("ask".into())
    }
}

impl CrashSetting {
    pub fn policy(&self) -> CrashPolicy {
        CrashPolicy::parse(&self.0).unwrap_or_default()
    }
}

/// The notice's clicks.
#[derive(Message, Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrashAction {
    Send,
    Always,
    Dismiss,
}

pub struct CrashPlugin;

impl Plugin for CrashPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CrashReports>()
            .init_resource::<CrashSetting>()
            .add_message::<CrashAction>()
            .add_systems(Update, (follow_setting, decide, act, system_line).chain());
        chimera_core::console::resource_cvar::<CrashSetting, String>(
            app,
            "chimera:cl_crash_reports",
            "After a crash or a session that died (a freeze, a power loss): ask to send its report (ask), send \
             without asking (always), or keep nothing (never).",
            |s| &mut s.0,
        );
        {
            let mut console = app.world_mut().get_resource_or_init::<Console>();
            console.archive("chimera:cl_crash_reports");
        }
        add_health_part(app, client_health);
        add_context_part(app, client_context);
        app.console_command(
            "chimera:crash_report",
            "crash_report [send|always|dismiss]: the report of the last session that died (a crash, a freeze, a \
             power loss): send it, send it and every later one, or not; no argument: what waits.",
            |w, a| {
                let action = match a.first().map(String::as_str) {
                    None => return Ok(Some(status(w))),
                    Some("send") => CrashAction::Send,
                    Some("always") => CrashAction::Always,
                    Some("dismiss" | "no") => CrashAction::Dismiss,
                    Some(other) => return Err(format!("crash_report {other}: send, always or dismiss")),
                };
                w.write_message(action);
                Ok(None)
            },
        )
        .console_command(
            "chimera:crash_test",
            "crash_test <panic|native|render>: crash now, to see the crash report (native: a null pointer write; \
             render: as a device lost).",
            |w, a| match a.first().map(String::as_str) {
                Some("panic") => panic!("crash_test: a panic on purpose"),
                Some("native") => {
                    // SAFETY: none; this is the crash asked for.
                    unsafe { std::ptr::null_mut::<u64>().write_volatile(0xC0FFEE) };
                    Ok(None)
                }
                Some("render") => {
                    render_crash(w, "DeviceLost", "crash_test");
                    Ok(None)
                }
                _ => Err("crash_test <panic|native|render>".into()),
            },
        );
    }

    fn finish(&self, app: &mut App) {
        if app.world().contains_resource::<bevy::render::error_handler::RenderErrorHandler>() {
            app.insert_resource(bevy::render::error_handler::RenderErrorHandler(|error, main, _| {
                error!("render error: {:?}: {}", error.ty, error.description);
                render_crash(main, &format!("{:?}", error.ty), &error.description);
                main.write_message(AppExit::error());
                bevy::render::error_handler::RenderErrorPolicy::StopRendering
            }));
        }
    }
}

/// A render error's crash bundle.
fn render_crash(world: &mut World, kind: &str, description: &str) {
    if let Some(s) = world.get_resource::<SessionLog>().map(|s| s.0.clone()) {
        s.crash(&format!("render error: {kind}: {description}"), "");
    }
}

fn follow_setting(setting: Res<CrashSetting>) {
    if setting.is_changed() {
        session::set_policy(setting.policy());
    }
}

/// The dead sessions dealt with, once config.cfg has run.
fn decide(mut reports: ResMut<CrashReports>, setting: Res<CrashSetting>, outbox: Option<Res<super::feedback::FeedbackOutbox>>, console: Option<ResMut<Console>>) {
    if reports.decided {
        return;
    }
    reports.frames += 1;
    if reports.frames < SETTLE_FRAMES {
        return;
    }
    reports.decided = true;
    if reports.previous.is_empty() {
        return;
    }
    reports.crashed = reports.previous[0].crashed();
    match setting.policy() {
        CrashPolicy::Ask => reports.offer = true,
        CrashPolicy::Always => {
            let said = send_all(&mut reports, outbox.as_deref());
            if let Some(mut c) = console {
                c.info(said);
            }
        }
        CrashPolicy::Never => {
            for p in std::mem::take(&mut reports.previous) {
                p.settle();
            }
        }
    }
}

fn send_all(reports: &mut CrashReports, outbox: Option<&super::feedback::FeedbackOutbox>) -> String {
    let mut sent = Vec::new();
    let mut failed = Vec::new();
    for p in std::mem::take(&mut reports.previous) {
        match p.send() {
            Ok(id) => sent.push(id),
            Err(e) => failed.push(e),
        }
    }
    if let Some(o) = outbox {
        o.0.wake();
    }
    reports.offer = false;
    let said = match (sent.len(), failed.first()) {
        (0, Some(e)) => format!("The crash report couldn't be made: {e}"),
        (1, _) => "Crash report queued: it sends with the other reports".to_string(),
        (n, _) => format!("{n} crash reports queued: they send with the other reports"),
    };
    reports.said = Some(said.clone());
    said
}

fn act(
    mut actions: MessageReader<CrashAction>,
    mut reports: ResMut<CrashReports>,
    outbox: Option<Res<super::feedback::FeedbackOutbox>>,
    mut console: Option<ResMut<Console>>,
) {
    for a in actions.read() {
        let said = match a {
            CrashAction::Send | CrashAction::Always => {
                if *a == CrashAction::Always
                    && let Some(c) = console.as_deref_mut()
                {
                    c.submit("chimera:cl_crash_reports always");
                }
                if reports.previous.is_empty() {
                    "No crash report waits".to_string()
                } else {
                    send_all(&mut reports, outbox.as_deref())
                }
            }
            CrashAction::Dismiss => {
                for p in std::mem::take(&mut reports.previous) {
                    p.settle();
                }
                reports.offer = false;
                "Not sent".to_string()
            }
        };
        if let Some(c) = console.as_deref_mut() {
            c.info(format!("crash_report: {said}"));
        }
    }
}

fn status(w: &World) -> String {
    let r = w.resource::<CrashReports>();
    let policy = w.resource::<CrashSetting>().policy().as_str();
    if r.previous.is_empty() {
        return format!("crash_report: nothing waits (cl_crash_reports {policy})");
    }
    let mut out = format!("crash_report: {} session(s) ended without a clean exit (cl_crash_reports {policy}):", r.previous.len());
    for p in &r.previous {
        out += &format!(
            "\n  {} {} started {}{}{}",
            p.info.app,
            p.info.build,
            chimera_core::mount::assets::feedback::utc_text(p.info.started),
            if p.held().is_some() { ", crash report held" } else { "" },
            if p.dump_path().is_some() { ", minidump" } else { "" }
        );
    }
    out
}

/// The PC, once the adapter is known.
fn system_line(session: Option<Res<SessionLog>>, adapter: Option<Res<bevy::render::renderer::RenderAdapterInfo>>, windows: Query<&Window>, mut done: Local<bool>) {
    let (Some(session), Some(a)) = (session, adapter) else { return };
    if *done {
        return;
    }
    *done = true;
    let mut text = format!(
        "os: {} {}\ncpus: {}\ngpu: {} ({:?}, {:?}, driver {} {})\n",
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::thread::available_parallelism().map_or(0, |n| n.get()),
        a.name,
        a.device_type,
        a.backend,
        a.driver,
        a.driver_info
    );
    if let Some(w) = windows.iter().next() {
        text += &format!("window: {}x{}, scale {}, {:?}, {:?}\n", w.physical_width(), w.physical_height(), w.scale_factor(), w.mode, w.present_mode);
    }
    session.0.set_system(&text);
}

/// The local player's rung, when a ladder runs.
fn local_rung(world: &mut World) -> Option<usize> {
    let mut q = world.query_filtered::<&chimera_sim::minigame::ladder::Rung, With<crate::core::LocalPlayer>>();
    q.iter(world).next().map(|r| r.0)
}

fn client_health(world: &mut World, h: &mut HealthLine) {
    if let Some(m) = world.get_resource::<super::frame_metrics::FrameMetrics>()
        && let Some(p) = m.frame_ms.ring.percentiles()
    {
        h.push("frame", format!("{:.1}/{:.1}ms", p.p50, p.p99));
        h.push("frame_max", format!("{:.0}ms", p.max));
    }
    if let Some(d) = world.get_resource::<bevy::render::renderer::RenderDevice>()
        && let Some(r) = d.wgpu_device().generate_allocator_report()
    {
        h.push(
            "gpu",
            format!(
                "{}/{}",
                chimera_core::metrics::memory::short_size(r.total_allocated_bytes),
                chimera_core::metrics::memory::short_size(r.total_reserved_bytes)
            ),
        );
    }
    let client = world.get_resource::<crate::core::NetRole>().is_some_and(|r| *r == crate::core::NetRole::Client);
    if client && let Some(g) = world.get_resource::<crate::net::predict::NetGraph>() {
        h.push("ping", format!("{:.0}ms", g.ping_ms));
        h.push("loss", format!("{:.1}%", g.loss));
        h.push("resyncs", g.resyncs);
        h.push("errors", g.errors);
        h.push("jumps", g.clock_jumps);
    }
    if let Some(m) = world.get_resource::<crate::map::LoadedMapName>() {
        h.push("map", &m.0);
    }
    if let Some(r) = local_rung(world) {
        h.push("rung", r + 1);
    }
    h.push("entities", world.entities().count_spawned());
    // What grows in a long session (Bevy frees no archetype; a material or
    // shader first seen compiles a pipeline): docs/performance.md, "Stutter
    // on Beaver Creek".
    h.push("archetypes", world.archetypes().len());
    if let Some(r) = world.get_resource::<super::memory::RenderWorldStats>() {
        let (images, meshes, pipelines, waiting) = r.health();
        h.push("gpu_textures", images);
        h.push("gpu_meshes", meshes);
        h.push("pipelines", format!("{pipelines}/{waiting}"));
    }
}

fn client_context(world: &World) -> String {
    let mut s = String::new();
    if let Some(m) = world.get_resource::<crate::map::LoadedMapName>() {
        s += &format!("map: {}\n", m.0);
    }
    if let Some(m) = world.get_resource::<chimera_sim::minigame::ActiveMinigame>() {
        s += &format!("minigame: {}\n", m.def.id);
    }
    if let Some(server) = world.get_resource::<crate::net::client::JoinProgress>().and_then(|j| j.server.as_ref()) {
        s += &format!("server: {} ({} of {} players)\n", server.name, server.players, server.max_players);
    }
    if let Some(role) = world.get_resource::<crate::core::NetRole>() {
        s += &format!("role: {role:?}\n");
    }
    // The local player (a read-only world: through the archetypes).
    for archetype in world.archetypes().iter() {
        let Some(local) = world.components().component_id::<crate::core::LocalPlayer>() else { break };
        if !archetype.contains(local) {
            continue;
        }
        if let Some(e) = archetype.entities().first().map(|e| e.id())
            && let Ok(entity) = world.get_entity(e)
        {
            if let Some(t) = entity.get::<Transform>() {
                s += &format!("position: {:.1} {:.1} {:.1}\n", t.translation.x, t.translation.y, t.translation.z);
            }
            if let Some(r) = entity.get::<chimera_sim::minigame::ladder::Rung>() {
                s += &format!("rung: {}\n", r.0 + 1);
            }
        }
        break;
    }
    s
}

/// The dead sessions found by `app::run`, for `CrashReports`.
pub fn found(app: &mut App, previous: Vec<Previous>) {
    let mut r = app.world_mut().get_resource_or_init::<CrashReports>();
    r.previous = previous;
}

/// The context parts' text now (tests).
pub fn context_text(world: &World) -> String {
    world.get_resource::<ContextParts>().map(|p| p.0.iter().map(|f| f(world)).collect()).unwrap_or_default()
}
