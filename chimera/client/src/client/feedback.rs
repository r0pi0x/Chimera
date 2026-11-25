//! The in-game report (docs/plans/active/feedback-reports.md, P1b): a
//! tester presses the report key (`feedback`, F10 in Ultimate Gun Game:
//! `binds::AppBind`) or the Escape menu's "Report a bug", the game takes a
//! screenshot of that very frame first (an open menu closed and gone
//! first: `SETTLE_FRAMES`), then the report dialog opens over the match
//! (which goes on). The state lives here, for every app; an app draws the
//! dialog (`Feedback::dialog`: the party's Signal one,
//! `party_kit::feedback_signal`). An app without one gets the console's
//! `feedback_send <text>`, which sends the screenshot just taken.
//!
//! - **The key** is a polled bind (`binds::POLLED`), read here in menus
//!   too, not while the console or chat is typing, the dialog is open or
//!   an app holds it off (`Feedback::key_blocked`: a bind listening for a
//!   key).
//! - **The capture** goes through `FeedbackCapture` (the window's
//!   `Screenshot` by default; tests put in `FeedbackCapture::fake`): the
//!   dialog opens on its `ScreenshotCaptured`, never before. The frame
//!   is the window's whole image, every camera's UI over the world (the
//!   HUD, the menu, the lobby, the console); nothing drawn reacts to the
//!   report until its dialog shows (`Feedback::shown`; `active` is for
//!   input), so the frame is what the player saw at the key. The rest of
//!   what the report says (`capture::report_text`: build, map, `setpos`,
//!   weapon, health, perf, the last console lines) is taken at the key
//!   press.
//! - **While open** the dialog has the keys (as chat): typed text goes
//!   into the report (2000 characters; Enter a new line), Tab switches
//!   Bug / Feedback, the clipboard's keys work as in the kit's text
//!   fields (`widgets::text_key`: Ctrl+V and Shift+Insert paste, line
//!   breaks kept; Ctrl+A selects the whole text, `Feedback::selected`,
//!   which Ctrl+C or Ctrl+Insert copies, Ctrl+X or Shift+Delete cuts and
//!   typing replaces), **Ctrl+Enter** sends, **Esc** cancels; every key is
//!   reset after, so no bind runs and the game sees nothing
//!   (`console::run_binds` waits too). Clicks arrive as `FeedbackAction`s.
//!   The UI cues sound (`ui_select` on opening and sending, `ui_hover` on
//!   a switch, `ui_back` on cancel, `ui_denied` on an empty send).
//! - **Send** hands a `Report` (the words, the frame, and `collect_state`'s
//!   `ReportState` from the key press: build, app, the game and the
//!   screens up (`screen_text`), the PC, perf,
//!   net, changed cvars and binds, console lines) to the `FeedbackOutbox`
//!   (`Outbox`). The app's is `AssetsOutbox`: the JPEG and the zip on a
//!   thread (`chimera_assets::feedback`: its secret filter is the only
//!   door), `<data_local>/chimera/feedback/outbox/<id>.zip`, sent by the
//!   outbox's thread to the first destination that applies (the
//!   `chimera:feedback_url`/`_token`/`_fingerprint` cvars, the signed-in
//!   account's hub with its ticket, the joined server's asset service
//!   when it is that hub, `net::assets::JoinedService`; without an
//!   account a service with no hub and its own token, `[feedback]` or
//!   `[assets]` in the local config: `chimera_assets::feedback::outbox`),
//!   else kept and sent when one answers (and at the next start). The
//!   account comes from the live session (`account::live`, polled:
//!   `signed_in`), so signing in (onboarding, Settings > Account) sends
//!   what was kept. The notice (`Feedback::notice`): "Sending
//!   report...", then the delivery's "Report sent" or why it waits;
//!   without an account (none, or a service refusing the token as
//!   unknown) "Sign in to send reports; kept on this PC" (`SIGN_IN`); a
//!   kept report sent later says so; a console line for each.
//!
//! Console: `feedback` (the key's command; `feedback menu` waits for a
//! menu to go), `feedback_send <text>`, `feedback_status` (where reports
//! go, what waits and why), `feedback_press
//! <send|cancel|bug|feedback|shot|details>` (tests, screenshots),
//! `feedback_shot <file.jpg>` (the frame the report holds, as it would be
//! sent: checks of what a report shows).
//!
//! `log.txt` is the session's log tail (`diagnostics`: every `tracing`
//! line at INFO and above and every console line, filtered), when a
//! session runs.
//!
//! Quitting waits (at most `FLUSH_LIMIT`) for reports still being
//! written (`Outbox::flush`, a `chimera_core::shutdown` step); a report
//! not sent yet stays in the outbox for the next start.
//!
//! Known shortcuts: the text has one caret, at its end (no arrows, no
//! selection, no paste: the kit's `text_field` gets paste).

use std::{
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
};

use bevy::{
    asset::RenderAssetUsages,
    input::{
        ButtonState, InputSystems,
        keyboard::KeyboardInput,
    },
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
};

use chimera_core::mount::assets::feedback::{
    CvarValue, Feedback as OutboxThread, ImageFormat, Kind, OutboxConfig, ReportState, Screenshot as ShotBytes, Target, Targets,
    outbox::{SIGN_IN, SIGN_IN_AGAIN, rate_limit_minutes},
};

use super::widgets::{Edit, TextKey};
use crate::console::{Console, ConsoleAppExt, Level};

/// How long a quit waits for reports still being written.
pub const FLUSH_LIMIT: std::time::Duration = std::time::Duration::from_secs(5);

/// Most characters a report's text takes.
pub const MAX_TEXT: usize = 2000;

/// Frames a menu open at the key press gets to go (it closes, then its
/// HUD cover and root are gone) before the capture.
pub const SETTLE_FRAMES: u8 = 3;

/// Frames to wait for a capture before opening without one (no window:
/// a headless run without the fake).
const CAPTURE_GIVE_UP: u32 = 120;

/// Seconds the notice after Send stays.
pub const NOTICE_SECS: f64 = 5.0;

/// What a report is about.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Category {
    #[default]
    Bug,
    Feedback,
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Category::Bug => "Bug",
            Category::Feedback => "Feedback",
        }
    }

    /// Its id in a report (`bug`, `feedback`).
    pub fn id(self) -> &'static str {
        match self {
            Category::Bug => "bug",
            Category::Feedback => "feedback",
        }
    }

    pub fn other(self) -> Self {
        match self {
            Category::Bug => Category::Feedback,
            Category::Feedback => Category::Bug,
        }
    }
}

/// Where the report is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Closed,
    /// A menu was open: frames left before the capture.
    Settling(u8),
    /// The screenshot asked for; frames waited.
    Capturing(u32),
    /// The dialog shows.
    Open,
}

/// The line after Send.
#[derive(Clone, Debug, PartialEq)]
pub struct Notice {
    pub text: String,
    /// Real seconds (`Time<Real>`) it goes at.
    pub until: f64,
    /// It went to the service (else saved to send later).
    pub sent: bool,
}

/// The report being written.
#[derive(Resource, Default)]
pub struct Feedback {
    pub phase: Phase,
    pub category: Category,
    pub text: String,
    pub include_screenshot: bool,
    /// "What's sent" unfolded.
    pub details_open: bool,
    /// The frame at the key press, and the same as an asset (the
    /// thumbnail).
    pub shot: Option<Image>,
    pub thumb: Option<Handle<Image>>,
    /// What the report says besides the text (`capture::report_text`),
    /// taken at the key press.
    pub snapshot: String,
    /// The rest of the bundle (`collect_state`), taken at the key press.
    pub state: ReportState,
    /// The report sent last, until its delivery is told (its notice).
    pub awaiting: Option<String>,
    /// The last Send's notice.
    pub notice: Option<Notice>,
    /// An empty Send was refused (the field shows it until typed in).
    pub refused: bool,
    /// The whole text is selected (Ctrl+A): copy and cut take it, typing
    /// replaces it.
    pub selected: bool,
    /// The app draws the dialog (else the console's `feedback_send`).
    pub dialog: bool,
    /// The app holds the key off (a bind listening for one).
    pub key_blocked: bool,
    /// Reports sent this run.
    pub sent: u32,
}

impl Feedback {
    /// The dialog shows.
    pub fn shown(&self) -> bool {
        self.phase == Phase::Open
    }

    /// Asked for and not closed (the capture too). Not for what is drawn:
    /// a screen that changes on it changes the frame being reported
    /// (what draws differently for the report waits for `shown`).
    pub fn active(&self) -> bool {
        self.phase != Phase::Closed
    }

    fn reset(&mut self) {
        self.phase = Phase::Closed;
        self.text.clear();
        self.details_open = false;
        self.refused = false;
        self.selected = false;
        self.shot = None;
        self.thumb = None;
        self.snapshot.clear();
        self.state = ReportState::default();
    }

    /// What "What's sent" lists: (what, the value or a summary).
    pub fn sent_lines(&self) -> Vec<(&'static str, String)> {
        let line = |key: &str| {
            self.snapshot
                .lines()
                .find_map(|l| l.strip_prefix(key).map(|v| v.trim().to_string()))
                .unwrap_or_else(|| "-".into())
        };
        let build = self.snapshot.lines().next().unwrap_or("-").trim_start_matches("Chimera build ").to_string();
        let pos = self.snapshot.lines().find(|l| l.starts_with("setpos")).map_or("-".into(), |l| {
            let w: Vec<&str> = l.split(|c| c == ' ' || c == ';').collect();
            w.get(1..4).map_or("-".into(), |v| v.join(" "))
        });
        vec![
            ("Build", build),
            ("Map", line("map:")),
            ("Position", pos),
            ("Weapon", line("weapon:")),
            ("Screenshot", if self.include_screenshot && self.shot.is_some() { "this frame".into() } else { "no".into() }),
            ("Also", "PC, perf, recent console".into()),
        ]
    }
}

/// A click or a key on the dialog (its buttons carry one).
#[derive(Component, Message, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedbackAction {
    Category(Category),
    ToggleCategory,
    ToggleScreenshot,
    ToggleDetails,
    Send,
    Cancel,
}

/// A report as handed to the outbox.
#[derive(Clone, Debug)]
pub struct Report {
    pub category: Category,
    pub text: String,
    /// The frame at the key press (None: left out, or none was taken).
    pub screenshot: Option<Image>,
    /// `capture::report_text` at the key press.
    pub snapshot: String,
    /// The rest of what the bundle says (`collect_state` at the key
    /// press: system, perf, net, changed cvars and binds, console lines);
    /// the outbox fills in the id, kind, text and screenshot.
    pub state: ReportState,
}

/// What became of a report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Submitted {
    /// The service has it.
    Sent { id: String },
    /// Queued, on its way to `to` (the outbox's thread sends it; an
    /// `Delivery` says how it went).
    Sending { id: String, to: String },
    /// Saved on this PC; `waiting` reports wait to be sent (this one
    /// too); `folder` where.
    Queued { id: String, waiting: usize, folder: PathBuf },
}

/// How a report sent in the background went (`Outbox::deliveries`).
pub use chimera_core::mount::assets::feedback::OutboxEvent as Delivery;

/// Where reports go: `AssetsOutbox` (the bundle, the queue on disk and its
/// sender thread); tests put in their own.
pub trait Outbox: Send + Sync + 'static {
    /// Where Send sends, in words ("To <server>'s report service").
    fn destination(&self) -> String;
    /// Take a report (quickly: slow work on a thread of its own).
    fn submit(&self, report: Report) -> Result<Submitted, String>;
    /// What the sender did since last asked.
    fn deliveries(&self) -> Vec<Delivery> {
        Vec::new()
    }
    /// The server joined: its asset service (URL, fingerprint) and name.
    fn joined(&self, _service: Option<(String, String)>, _server: Option<String>) {}
    /// A service named by hand (`chimera:feedback_url`, `_token`,
    /// `_fingerprint`; None: not all set).
    fn named(&self, _target: Option<Target>) {}
    /// The signed-in account changed (signed in, out, or another).
    fn account(&self, _session: Option<Arc<chimera_core::mount::assets::account::Session>>) {}
    /// `feedback_status`'s lines.
    fn status(&self) -> String {
        String::new()
    }
    /// The app started (reports from an earlier run may wait).
    fn startup(&self) {}
    /// Reports were put in the outbox from outside (a crash's,
    /// `crash`): send what waits now.
    fn wake(&self) {}
    /// The game quits: the reports still being written are, within
    /// `limit` (`chimera_core::shutdown`'s `Persist` step). What waits to be
    /// sent stays in the outbox for the next start.
    fn flush(&self, _limit: std::time::Duration) {}
}

/// A report service named by hand: `chimera:feedback_url`,
/// `chimera:feedback_token`, `chimera:feedback_fingerprint` (not saved:
/// the command line's; all but the fingerprint set, or none is used).
#[derive(Resource, Clone, Default, PartialEq, Eq)]
pub struct FeedbackService {
    pub url: String,
    pub token: String,
    pub fingerprint: String,
}

impl FeedbackService {
    fn target(&self) -> Option<Target> {
        // Quotes a command line kept are not part of the value.
        let clean = |s: &str| s.trim().trim_matches(|c| c == '"' || c == '\'').trim().to_string();
        let (url, token) = (clean(&self.url), clean(&self.token));
        (!url.is_empty() && !token.is_empty()).then(|| Target {
            url,
            token,
            fingerprint: Some(clean(&self.fingerprint)).filter(|f| !f.is_empty()),
        })
    }
}

/// The app's outbox.
#[derive(Resource, Clone)]
pub struct FeedbackOutbox(pub Arc<dyn Outbox>);

impl Default for FeedbackOutbox {
    fn default() -> Self {
        Self(Arc::new(AssetsOutbox::user()))
    }
}

/// The data folder's `feedback/` (`<data_local>/chimera/feedback`;
/// `CHIMERA_DATA_DIR` names another data folder: `diagnostics::data_root`).
pub fn feedback_folder() -> Option<PathBuf> {
    Some(chimera_core::diagnostics::data_root()?.join("feedback"))
}

/// Reports through `chimera_assets::feedback`'s outbox: the zip built
/// from the `ReportState` (its secret filter), written to
/// `<folder>/outbox/<id>.zip`, sent by its thread to the first
/// destination that applies (the joined server's asset service with the
/// tester's token, `[feedback]` or `[assets]` in the local config;
/// `Targets`), else kept until one answers. The outbox starts on first
/// use, or at startup when reports wait (`start_if_waiting`).
pub struct AssetsOutbox {
    folder: Option<PathBuf>,
    /// Where reports may go (the outbox's own once it runs).
    targets: Mutex<Targets>,
    /// Read the local config and the access file when starting (not in
    /// tests).
    user_targets: bool,
    started: OnceLock<Result<Arc<OutboxThread>, String>>,
    /// Failures of the encoding thread (the outbox's own come from it).
    failed: Arc<Mutex<Vec<Delivery>>>,
    /// The bundles' threads (the JPEG and the zip), for `flush`.
    writing: Mutex<Vec<std::thread::JoinHandle<()>>>,
}

impl AssetsOutbox {
    /// The user's: the data folder, the local config's and the access
    /// file's services.
    pub fn user() -> Self {
        Self {
            folder: feedback_folder(),
            targets: Mutex::new(Targets::default()),
            user_targets: true,
            started: OnceLock::new(),
            failed: Arc::default(),
            writing: Mutex::default(),
        }
    }

    /// In `folder`, to these targets only (tests).
    pub fn at(folder: PathBuf, targets: Targets) -> Self {
        Self {
            folder: Some(folder),
            targets: Mutex::new(targets),
            user_targets: false,
            started: OnceLock::new(),
            failed: Arc::default(),
            writing: Mutex::default(),
        }
    }

    fn outbox(&self) -> Result<Arc<OutboxThread>, String> {
        self.started
            .get_or_init(|| {
                let folder = self.folder.clone().ok_or("no per-user data folder")?;
                let mut targets = self.targets.lock().map_err(|e| e.to_string())?.clone();
                if self.user_targets {
                    targets.local = chimera_core::mount::assets::feedback::LocalFeedback::load()
                        .ok()
                        .and_then(|f| f.target());
                    targets.access = Some(chimera_core::mount::assets::access::Access::load());
                    targets.assets = chimera_core::mount::config::LocalConfig::load()
                        .ok()
                        .and_then(|c| c.assets.resolve().remote);
                    targets.account = chimera_core::mount::assets::account::user_session();
                }
                OutboxThread::start(folder, targets, OutboxConfig::default())
                    .map(Arc::new)
                    .map_err(|e| e.to_string())
            })
            .clone()
    }

    /// Start now when reports wait from an earlier run (they go at once).
    pub fn start_if_waiting(&self) {
        let waiting = self.folder.as_ref().and_then(|f| std::fs::read_dir(f.join("outbox")).ok()).is_some_and(|d| {
            d.filter_map(Result::ok).any(|e| e.path().extension().is_some_and(|x| x == "zip"))
        });
        if waiting && let Err(e) = self.outbox() {
            warn!("feedback: the outbox didn't start: {e}");
        }
    }
}

/// The destination in words.
fn destination_text(d: Option<&chimera_core::mount::assets::feedback::Destination>) -> String {
    use chimera_core::mount::assets::feedback::outbox::Source;
    match d {
        Some(d) if d.source == Source::Server => format!("To {}'s report service", d.label),
        Some(d) if d.source == Source::Account => "To your hub's report service".into(),
        Some(_) => "To the report service".into(),
        None => "Not signed in: reports kept on this PC".into(),
    }
}

/// The frame as the bundle's JPEG: at most 1920 wide, quality 85.
pub fn encode_screenshot(image: Image) -> Result<ShotBytes, String> {
    let rgb = image.try_into_dynamic().map_err(|e| e.to_string())?.to_rgb8();
    let rgb = if rgb.width() > 1920 {
        let h = (rgb.height() as u64 * 1920 / rgb.width() as u64).max(1) as u32;
        image::imageops::resize(&rgb, 1920, h, image::imageops::FilterType::Triangle)
    } else {
        rgb
    };
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 85)
        .encode_image(&rgb)
        .map_err(|e| e.to_string())?;
    Ok(ShotBytes { format: ImageFormat::Jpeg, bytes })
}

impl Outbox for AssetsOutbox {
    fn destination(&self) -> String {
        match self.outbox() {
            Ok(o) => destination_text(o.destination().as_ref()),
            Err(_) => "No report service and no folder: not saved".into(),
        }
    }

    fn submit(&self, report: Report) -> Result<Submitted, String> {
        let outbox = self.outbox()?;
        let id = chimera_core::mount::assets::feedback::new_report_id();
        let mut state = report.state;
        state.id = id.clone();
        state.kind = match report.category {
            Category::Bug => Kind::Bug,
            Category::Feedback => Kind::Feedback,
        };
        state.text = report.text;
        state.install_id = chimera_core::mount::assets::feedback::install_id(outbox.folder());
        let destination = outbox.destination();
        let waiting = outbox.status().pending() + 1;
        let folder = outbox.folder().to_path_buf();
        let (shot, failed, thread_id) = (report.screenshot, self.failed.clone(), id.clone());
        // The JPEG and the zip off the main thread; the outbox's thread
        // sends it.
        let handle = std::thread::Builder::new()
            .name("feedback-bundle".into())
            .spawn(move || {
                if let Some(img) = shot {
                    match encode_screenshot(img) {
                        Ok(s) => state.screenshot = Some(s),
                        Err(e) => warn!("feedback: the screenshot: {e}"),
                    }
                }
                if let Err(reason) = outbox.submit(&state)
                    && let Ok(mut f) = failed.lock()
                {
                    f.push(Delivery::Failed { id: thread_id, reason });
                }
            })
            .map_err(|e| e.to_string())?;
        if let Ok(mut w) = self.writing.lock() {
            w.retain(|h| !h.is_finished());
            w.push(handle);
        }
        Ok(match destination {
            Some(d) => Submitted::Sending { id, to: destination_text(Some(&d)) },
            None => Submitted::Queued { id, waiting, folder: folder.join("outbox") },
        })
    }

    fn deliveries(&self) -> Vec<Delivery> {
        let mut out: Vec<Delivery> = self.failed.lock().map(|mut f| std::mem::take(&mut *f)).unwrap_or_default();
        if let Some(Ok(o)) = self.started.get() {
            out.extend(o.events());
        }
        out
    }

    fn startup(&self) {
        self.start_if_waiting();
    }

    fn wake(&self) {
        match self.started.get() {
            Some(Ok(o)) => o.retry_now(),
            _ => self.start_if_waiting(),
        }
    }

    fn flush(&self, limit: std::time::Duration) {
        let writing = self.writing.lock().map(|mut w| std::mem::take(&mut *w)).unwrap_or_default();
        let until = std::time::Instant::now() + limit;
        for h in writing {
            while !h.is_finished() && std::time::Instant::now() < until {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            if h.is_finished() {
                let _ = h.join();
            } else {
                warn!("feedback: a report still being written after {} s", limit.as_secs());
            }
        }
    }

    fn named(&self, target: Option<Target>) {
        if let Ok(mut t) = self.targets.lock() {
            t.cvar = target.clone();
        }
        if let Some(Ok(o)) = self.started.get() {
            o.set_targets(|t| t.cvar = target);
        }
    }

    fn account(&self, session: Option<Arc<chimera_core::mount::assets::account::Session>>) {
        if let Ok(mut t) = self.targets.lock() {
            t.account = session.clone();
        }
        if let Some(Ok(o)) = self.started.get() {
            o.set_targets(|t| t.account = session);
        }
    }

    fn joined(&self, service: Option<(String, String)>, server: Option<String>) {
        if let Ok(mut t) = self.targets.lock() {
            t.offer = service.clone();
            t.server_name = server.clone();
        }
        if let Some(Ok(o)) = self.started.get() {
            o.set_targets(|t| {
                t.offer = service;
                t.server_name = server;
            });
        }
    }

    fn status(&self) -> String {
        let o = match self.outbox() {
            Ok(o) => o,
            Err(e) => return format!("feedback: no outbox: {e}"),
        };
        let s = o.status();
        let mut out = format!(
            "feedback: {}; {} waiting, {} sent this session; folder {}\n",
            destination_text(s.destination.as_ref()),
            s.pending(),
            s.sent_this_session,
            s.folder.display()
        );
        for w in &s.waiting {
            out += &format!(
                "  {} {} bytes, {} tries{}{}\n",
                w.id,
                w.bytes,
                w.attempts,
                if w.failed { ", refused for good" } else { "" },
                w.last_error.as_ref().map_or(String::new(), |e| format!(": {e}"))
            );
        }
        out
    }
}

/// Everything the bundle says besides the tester's words and the frame,
/// taken now (at the key press): build, app, the game (`report_text`'s
/// first part and the server's name), the PC, perf, net, the cvars that
/// differ from their default, the binds that differ from the defaults,
/// the console's last lines. The bundle's secret filter runs over all of
/// it (`chimera_assets::feedback::bundle`).
pub fn collect_state(w: &mut World, snapshot: &str) -> ReportState {
    use crate::console::Console;
    let build = w.get_resource::<crate::core::BuildVersion>().map_or("unknown", |v| v.0).to_string();
    let app = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_default();
    // The game: the snapshot up to its perf and console parts.
    let cut = ["\nperformance (", "\nlast console lines:"]
        .iter()
        .filter_map(|m| snapshot.find(m))
        .min()
        .unwrap_or(snapshot.len());
    let mut game = snapshot[..cut].to_string();
    if let Some(server) = w.get_resource::<crate::net::client::JoinProgress>().and_then(|j| j.server.clone()) {
        game += &format!("server: {} ({} of {} players)\n", server.name, server.players, server.max_players);
    }
    if let Some(m) = w.get_resource::<chimera_sim::minigame::ActiveMinigame>() {
        game += &format!("minigame: {}\n", m.def.id);
    }
    game += &screen_text(w);
    game += &super::world_report::setup_text(w);
    game += &super::world_report::hand_text(w);
    // What the last hits did (a listen host's or a local game's server).
    if let Some(h) = w.get_resource::<chimera_core::core::RecentHits>() {
        game += &h.text();
    }
    let world = super::world_report::world_text(w);
    let mut system = format!(
        "os: {} {}\ncpus: {}\n",
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::thread::available_parallelism().map_or(0, |n| n.get())
    );
    if let Some(a) = w.get_resource::<bevy::render::renderer::RenderAdapterInfo>() {
        system += &format!("gpu: {} ({:?}, driver {} {})\n", a.name, a.backend, a.driver, a.driver_info);
    }
    if let Some(win) = w.query::<&Window>().iter(w).next() {
        system += &format!(
            "window: {}x{} physical, scale {}, mode {:?}, present {:?}\n",
            win.physical_width(),
            win.physical_height(),
            win.scale_factor(),
            win.mode,
            win.present_mode
        );
    }
    let mut perf = w.get_resource::<super::perf::PerfReport>().map_or(String::new(), |p| p.lines.join("\n"));
    // The frame inspector's worst spikes, each with its breakdown.
    if let Some(i) = w.get_resource::<super::frame_inspector::FrameInspector>()
        && let Ok(i) = i.0.lock()
    {
        perf += &format!("\n\n{}", i.report());
    }
    let net = w.get_resource::<crate::net::predict::NetGraph>().map_or(String::new(), |g| {
        format!(
            "ping {:.0} ms, loss {:.1}%, in {:.1} KB/s, out {:.1} KB/s, commands {:.0}/s\n\
             clock: {} jumps, {} stalls ({} ticks skipped); server: {} buffered, {} missed, {} late\n",
            g.ping_ms,
            g.loss,
            g.in_kbs,
            g.out_kbs,
            g.cmd_rate,
            g.clock_jumps,
            g.clock_stalls,
            g.clock_skipped,
            g.buffered,
            g.missed,
            g.late
        )
    });
    let (cvars_list, console_lines, binds) = match w.get_resource::<Console>() {
        Some(c) => (
            c.cvars().cloned().collect::<Vec<_>>(),
            c.output.iter().map(|l| l.text.clone()).collect::<Vec<_>>(),
            c.binds.clone(),
        ),
        None => Default::default(),
    };
    let mut cvars = Vec::new();
    for c in cvars_list {
        if let Some(value) = (c.get)(w)
            && value != c.default
        {
            cvars.push(CvarValue {
                server_private: c.scope == chimera_core::console::CvarScope::Server,
                name: c.name,
                value,
                default: c.default,
            });
        }
    }
    let defaults: std::collections::BTreeMap<String, String> =
        w.get_resource::<super::binds::BindDefaults>().map(|d| d.default_binds().into_iter().collect()).unwrap_or_default();
    let binds = binds.into_iter().filter(|(k, v)| defaults.get(k) != Some(v)).collect();
    let log = w.get_resource::<chimera_core::diagnostics::SessionLog>().map(|s| s.0.tail()).unwrap_or_default();
    ReportState {
        log,
        health: w.get_resource::<chimera_core::diagnostics::SessionLog>().map(|s| s.0.health_lines()).unwrap_or_default(),
        build,
        net_version: crate::net::NET_VERSION.to_string(),
        app,
        game,
        system,
        perf,
        world,
        net,
        cvars,
        binds,
        console: console_lines,
        ..Default::default()
    }
}

/// The screens up at the key press, one line (`screen: play` with none):
/// a menu, the console, the chat (open, docked), the lobby, the match end
/// and its vote, which take keys a report's typing must not reach.
pub fn screen_text(w: &World) -> String {
    let mut up: Vec<String> = Vec::new();
    if w.get_resource::<super::front_end::AppMenu>().is_some_and(|m| m.open) {
        up.push("app menu".into());
    }
    if w.get_resource::<super::front_end::MenuStatus>().is_some_and(|m| m.open) {
        up.push("menu".into());
    }
    if w.get_resource::<super::console::ConsoleUi>().is_some_and(|c| c.open) {
        up.push("console".into());
    }
    if w.get_resource::<super::prematch_lobby::LobbyView>().is_some_and(|l| l.shown) {
        let over = w.get_resource::<super::prematch_lobby::LobbyOverlay>().is_some_and(|o| o.0);
        up.push(format!("pre-match lobby{}", if over { " (an overlay over it)" } else { "" }));
    }
    if let Some(e) = w.get_resource::<super::match_end::MatchEndView>().filter(|e| e.shown) {
        let vote = e.vote.as_ref().is_some_and(|v| v.open());
        up.push(format!("match end{}", if vote { " (map vote open)" } else { "" }));
    }
    if let Some(c) = w.get_resource::<super::chat::ChatInput>().and_then(|c| c.open) {
        up.push(format!("chat open ({})", if c { "team" } else { "all" }));
    }
    if up.is_empty() {
        up.push("play".into());
    }
    format!("screen: {}\n", up.join(", "))
}

/// How the frame is taken: the window's screenshot, or (tests, no window)
/// a made-up frame at once.
#[derive(Resource, Clone, Copy)]
pub struct FeedbackCapture(pub fn(&mut World));

impl Default for FeedbackCapture {
    fn default() -> Self {
        Self(window_capture)
    }
}

impl FeedbackCapture {
    /// A 16 x 9 grey frame, taken at once.
    pub fn fake() -> Self {
        Self(|w| captured(w, fake_frame()))
    }
}

/// A made-up 16 x 9 frame (tests, staged scenes).
pub fn fake_frame() -> Image {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    Image::new_fill(
        Extent3d { width: 16, height: 9, depth_or_array_layers: 1 },
        TextureDimension::D2,
        &[60, 70, 90, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

fn window_capture(w: &mut World) {
    w.spawn(Screenshot::primary_window()).observe(|ev: On<ScreenshotCaptured>, mut commands: Commands| {
        let image = ev.image.clone();
        commands.queue(move |w: &mut World| captured(w, image));
    });
}

/// The frame is in: kept, its thumbnail made, and the dialog opens (or,
/// without one, the console says how to send).
pub fn captured(w: &mut World, mut image: Image) {
    if !matches!(w.resource::<Feedback>().phase, Phase::Capturing(_)) {
        return;
    }
    image.asset_usage = RenderAssetUsages::default();
    let thumb = w.get_resource_mut::<Assets<Image>>().map(|mut a| a.add(image.clone()));
    open_with(w, Some(image), thumb);
}

fn open_with(w: &mut World, shot: Option<Image>, thumb: Option<Handle<Image>>) {
    let dialog = {
        let mut f = w.resource_mut::<Feedback>();
        f.shot = shot;
        f.thumb = thumb;
        f.include_screenshot = f.shot.is_some();
        f.phase = if f.dialog { Phase::Open } else { Phase::Closed };
        f.dialog
    };
    if dialog {
        cue(w, "ui_select");
    } else {
        w.resource_mut::<Console>().print(
            Level::Info,
            "report: screenshot taken; `feedback_send <what happened>` sends it with your words",
        );
    }
}

fn cue(w: &mut World, event: &'static str) {
    if let Some(mut m) = w.get_resource_mut::<Messages<super::ui_sound::UiCue>>() {
        m.write(super::ui_sound::UiCue::new(event));
    }
}

/// The key, the capture and the actions (in `Update`; an app's dialog
/// runs before them).
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct FeedbackSystems;

/// The dialog's keys (`typing`, which resets them): the chat's run after
/// (`signal_chat`), so Y or Enter typed here never opens it.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct FeedbackKeys;

pub struct FeedbackPlugin;

impl Plugin for FeedbackPlugin {
    fn build(&self, app: &mut App) {
        use chimera_core::shutdown::{ShutdownAppExt, Stage};
        app.shutdown_flush(Stage::Persist, "feedback outbox", |w| {
            if let Some(o) = w.get_resource::<FeedbackOutbox>() {
                o.0.flush(FLUSH_LIMIT);
            }
        });
        app.init_resource::<Feedback>()
            .init_resource::<FeedbackOutbox>()
            .init_resource::<FeedbackCapture>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<FeedbackAction>()
            .add_message::<KeyboardInput>();
        if !app.world().contains_resource::<Messages<super::ui_sound::UiCue>>() {
            app.add_message::<super::ui_sound::UiCue>();
        }
        {
            let mut console = app.world_mut().get_resource_or_init::<Console>();
            console.keep_short("feedback");
        }
        app.init_resource::<FeedbackService>();
        chimera_core::console::resource_cvar::<FeedbackService, String>(
            app,
            "chimera:feedback_url",
            "Where reports go, named by hand (https://host:port; with chimera:feedback_token), over the joined \
             server's asset service.",
            |s| &mut s.url,
        );
        chimera_core::console::resource_cvar::<FeedbackService, String>(
            app,
            "chimera:feedback_token",
            "The token for chimera:feedback_url.",
            |s| &mut s.token,
        );
        chimera_core::console::resource_cvar::<FeedbackService, String>(
            app,
            "chimera:feedback_fingerprint",
            "chimera:feedback_url's certificate SHA-256 (hex), pinned.",
            |s| &mut s.fingerprint,
        );
        app.add_systems(PreUpdate, typing.in_set(FeedbackKeys).after(InputSystems))
            .add_systems(Startup, start_if_waiting)
            .init_resource::<FeedbackService>()
            .add_systems(Update, (key, advance, act, deliveries, joined, named, signed_in).chain().in_set(FeedbackSystems))
            .console_command(
                "chimera:feedback",
                "feedback [menu]: report a bug or feedback: take a screenshot of this frame, then open the report \
                 dialog (menu: wait for an open menu to close first).",
                |w, a| {
                    request(w, a.first().is_some_and(|a| a == "menu"));
                    Ok(None)
                },
            )
            .console_command(
                "chimera:feedback_send",
                "feedback_send <text>: send a report with these words (and the screenshot `feedback` took, if any).",
                |w, a| {
                    let text = a.join(" ");
                    if text.trim().is_empty() {
                        return Err("feedback_send <what happened>".into());
                    }
                    if w.resource::<Feedback>().snapshot.is_empty() {
                        let snap = super::capture::report_text(w, "");
                        let state = collect_state(w, &snap);
                        let mut f = w.resource_mut::<Feedback>();
                        f.snapshot = snap;
                        f.state = state;
                    }
                    w.resource_mut::<Feedback>().text = text;
                    send(w);
                    Ok(None)
                },
            )
            .console_command(
                "chimera:feedback_status",
                "feedback_status: where reports go, what waits to be sent and why.",
                |w, _| Ok(Some(w.resource::<FeedbackOutbox>().0.status().trim_end().to_string())),
            )
            .console_command(
                "chimera:feedback_shot",
                "feedback_shot <file.jpg>: save the frame the open report holds, as the report's JPEG (checks, \
                 screenshots).",
                |w, a| {
                    let path = PathBuf::from(a.first().ok_or("feedback_shot <file.jpg>")?);
                    let shot = w.resource::<Feedback>().shot.clone().ok_or("no report frame taken")?;
                    let jpeg = encode_screenshot(shot)?;
                    std::fs::write(&path, &jpeg.bytes).map_err(|e| format!("{}: {e}", path.display()))?;
                    Ok(Some(format!("report frame saved to {}", path.display())))
                },
            )
            .console_command(
                "chimera:feedback_press",
                "feedback_press <send|cancel|bug|feedback|shot|details>: press that part of the open report dialog.",
                |w, a| {
                    let action = match a.first().map(String::as_str) {
                        Some("send") => FeedbackAction::Send,
                        Some("cancel") => FeedbackAction::Cancel,
                        Some("bug") => FeedbackAction::Category(Category::Bug),
                        Some("feedback") => FeedbackAction::Category(Category::Feedback),
                        Some("shot") => FeedbackAction::ToggleScreenshot,
                        Some("details") => FeedbackAction::ToggleDetails,
                        other => return Err(format!("no part {other:?}")),
                    };
                    w.write_message(action);
                    Ok(None)
                },
            );
    }
}

/// The key press (or the Escape menu's entry): the snapshot now, the
/// capture once any open menu is gone.
pub fn request(w: &mut World, after_menu: bool) {
    if w.resource::<Feedback>().active() {
        return;
    }
    let menu = after_menu
        || w.get_resource::<super::front_end::AppMenu>().is_some_and(|m| m.open)
        || w.get_resource::<super::front_end::MenuStatus>().is_some_and(|m| m.open);
    let snapshot = super::capture::report_text(w, "");
    let state = collect_state(w, &snapshot);
    let mut f = w.resource_mut::<Feedback>();
    f.reset();
    f.snapshot = snapshot;
    f.state = state;
    f.phase = Phase::Settling(if menu { SETTLE_FRAMES } else { 0 });
}

/// The report key (`feedback`), in menus too.
fn key(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    console: Option<ResMut<Console>>,
    ui: Option<Res<super::console::ConsoleUi>>,
    chat: Option<Res<super::chat::ChatInput>>,
    feedback: Res<Feedback>,
) {
    let Some(mut console) = console else { return };
    if feedback.active() || feedback.key_blocked || ui.is_some_and(|u| u.open) || chat.is_some_and(|c| c.open.is_some()) {
        return;
    }
    if super::binds::just_pressed(&console.binds, &keys, &mouse, "feedback") {
        console.submit("feedback");
    }
}

/// Settling, then the capture (given up on after a while: opened
/// without a screenshot).
fn advance(world: &mut World) {
    let phase = world.resource::<Feedback>().phase;
    match phase {
        Phase::Settling(0) => {
            world.resource_mut::<Feedback>().phase = Phase::Capturing(0);
            let capture = world.resource::<FeedbackCapture>().0;
            capture(world);
        }
        Phase::Settling(n) => world.resource_mut::<Feedback>().phase = Phase::Settling(n - 1),
        Phase::Capturing(n) if n >= CAPTURE_GIVE_UP => {
            warn!("report: no screenshot came; the dialog opens without one");
            open_with(world, None, None);
        }
        Phase::Capturing(n) => world.resource_mut::<Feedback>().phase = Phase::Capturing(n + 1),
        _ => {}
    }
}

/// The dialog's keys while it shows: typing, Tab, Ctrl+Enter, Esc, the
/// clipboard's; then every key reset (no bind, no game key, no other menu
/// sees them).
#[allow(clippy::too_many_arguments)]
fn typing(
    mut feedback: ResMut<Feedback>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut events: MessageReader<KeyboardInput>,
    mut out: MessageWriter<FeedbackAction>,
    mut mods: Local<(bool, bool)>,
    mut own: Local<chimera_ui::clipboard::Clipboard>,
    shared: Option<ResMut<chimera_ui::clipboard::Clipboard>>,
) {
    if !feedback.shown() {
        events.clear();
        *mods = (false, false);
        return;
    }
    let clipboard = match shared {
        Some(c) => c.into_inner(),
        None => &mut *own,
    };
    let (ctrl, shift) = &mut *mods;
    *ctrl |= keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    *shift |= keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    for e in events.read() {
        match e.key_code {
            KeyCode::ControlLeft | KeyCode::ControlRight => {
                *ctrl = e.state == ButtonState::Pressed;
                continue;
            }
            KeyCode::ShiftLeft | KeyCode::ShiftRight => {
                *shift = e.state == ButtonState::Pressed;
                continue;
            }
            _ => {}
        }
        if e.state != ButtonState::Pressed {
            continue;
        }
        match (&e.logical_key, e.key_code) {
            (_, KeyCode::Enter | KeyCode::NumpadEnter) if *ctrl => {
                out.write(FeedbackAction::Send);
            }
            (_, KeyCode::Escape) => {
                out.write(FeedbackAction::Cancel);
            }
            (_, KeyCode::Tab) => {
                out.write(FeedbackAction::ToggleCategory);
            }
            (_, KeyCode::Enter | KeyCode::NumpadEnter) => type_text(&mut feedback, "\n"),
            (logical, code) => match super::widgets::text_key(logical, code, *ctrl, *shift) {
                Some(TextKey::Paste) => {
                    if let Some(t) = clipboard.get() {
                        type_text(&mut feedback, &t.replace("\r\n", "\n").replace('\r', "\n"));
                    }
                }
                Some(TextKey::Copy) if feedback.selected => clipboard.set(&feedback.text),
                Some(TextKey::Cut) if feedback.selected => {
                    clipboard.set(&feedback.text);
                    feedback.text.clear();
                    feedback.selected = false;
                }
                Some(TextKey::Edit(Edit::SelectAll)) => feedback.selected = !feedback.text.is_empty(),
                Some(TextKey::Edit(Edit::Backspace | Edit::Delete)) if feedback.selected => {
                    feedback.text.clear();
                    feedback.selected = false;
                }
                Some(TextKey::Edit(Edit::Backspace)) => {
                    feedback.text.pop();
                }
                Some(TextKey::Edit(Edit::Insert(s))) => type_text(&mut feedback, &s),
                // The arrows, Home and End: the selection goes (the text
                // has no caret but its end).
                Some(TextKey::Edit(_)) => feedback.selected = false,
                _ => {}
            },
        }
    }
    keys.reset_all();
}

/// Add typed text, without control characters but new lines, up to
/// `MAX_TEXT` characters (over the selection, which it replaces).
fn type_text(f: &mut Feedback, s: &str) {
    if f.selected {
        f.selected = false;
        f.text.clear();
    }
    let mut n = f.text.chars().count();
    for c in s.chars().filter(|c| *c == '\n' || !c.is_control()) {
        if n >= MAX_TEXT {
            return;
        }
        f.text.push(c);
        n += 1;
        f.refused = false;
    }
}

/// What the dialog's actions do.
fn act(world: &mut World) {
    let actions: Vec<FeedbackAction> = world.resource_mut::<Messages<FeedbackAction>>().drain().collect();
    for a in actions {
        if !world.resource::<Feedback>().shown() {
            continue;
        }
        match a {
            FeedbackAction::Category(c) => {
                world.resource_mut::<Feedback>().category = c;
                cue(world, "ui_hover");
            }
            FeedbackAction::ToggleCategory => {
                let mut f = world.resource_mut::<Feedback>();
                f.category = f.category.other();
                cue(world, "ui_hover");
            }
            FeedbackAction::ToggleScreenshot => {
                let mut f = world.resource_mut::<Feedback>();
                if f.shot.is_some() {
                    f.include_screenshot = !f.include_screenshot;
                    cue(world, "ui_select");
                } else {
                    cue(world, "ui_denied");
                }
            }
            FeedbackAction::ToggleDetails => {
                let mut f = world.resource_mut::<Feedback>();
                f.details_open = !f.details_open;
                cue(world, "ui_select");
            }
            FeedbackAction::Cancel => {
                world.resource_mut::<Feedback>().reset();
                cue(world, "ui_back");
            }
            FeedbackAction::Send => {
                if world.resource::<Feedback>().text.trim().is_empty() {
                    world.resource_mut::<Feedback>().refused = true;
                    cue(world, "ui_denied");
                } else {
                    send(world);
                }
            }
        }
    }
}

/// The report to the outbox; the notice and a console line; the dialog
/// closed.
fn send(world: &mut World) {
    let report = {
        let f = world.resource::<Feedback>();
        Report {
            category: f.category,
            text: f.text.trim_end().to_string(),
            screenshot: if f.include_screenshot || !f.dialog { f.shot.clone() } else { None },
            snapshot: f.snapshot.clone(),
            state: f.state.clone(),
        }
    };
    let outbox = world.resource::<FeedbackOutbox>().0.clone();
    let now = world.get_resource::<Time<Real>>().map_or(0.0, |t| t.elapsed_secs_f64());
    let mut awaiting = None;
    let (notice, line, level) = match outbox.submit(report) {
        Ok(Submitted::Sent { id }) => (
            Notice { text: "Report sent".into(), until: now + NOTICE_SECS, sent: true },
            format!("report {id} sent"),
            Level::Info,
        ),
        Ok(Submitted::Sending { id, to }) => {
            let line = format!("report {id}: sending ({to})");
            awaiting = Some(id);
            (Notice { text: "Sending report...".into(), until: now + NOTICE_SECS, sent: false }, line, Level::Info)
        }
        Ok(Submitted::Queued { id, waiting, folder }) => (
            Notice {
                text: format!("Sign in to send reports; kept on this PC, sent once you're signed in ({waiting} waiting)"),
                until: now + NOTICE_SECS,
                sent: false,
            },
            format!("report {id} saved in {} ({waiting} waiting)", folder.display()),
            Level::Info,
        ),
        Err(e) => (
            Notice { text: "Report not saved: see the console".into(), until: now + NOTICE_SECS, sent: false },
            format!("report not saved: {e}"),
            Level::Error,
        ),
    };
    world.resource_mut::<Console>().print(level, line);
    let mut f = world.resource_mut::<Feedback>();
    f.reset();
    f.awaiting = awaiting;
    f.notice = Some(notice);
    f.sent += 1;
    f.include_screenshot = true;
    cue(world, "ui_select");
}

/// Reports left from an earlier run go at once (the outbox starts when
/// some wait; else on first use).
fn start_if_waiting(outbox: Res<FeedbackOutbox>) {
    outbox.0.startup();
}

/// The sender's news: the notice for the report sent last ("Report
/// sent", or why it waits), a console line for each.
fn deliveries(outbox: Res<FeedbackOutbox>, time: Res<Time<Real>>, mut feedback: ResMut<Feedback>, console: Option<ResMut<Console>>) {
    let news = outbox.0.deliveries();
    if news.is_empty() {
        return;
    }
    let Some(mut console) = console else { return };
    let until = time.elapsed_secs_f64() + NOTICE_SECS;
    for d in news {
        let (id, text, sent, line, level) = match d {
            Delivery::Sent { id } => (id.clone(), "Report sent".to_string(), true, format!("report {id} sent"), Level::Info),
            Delivery::Queued { id, waiting, reason } => (
                id.clone(),
                if reason == SIGN_IN || reason == SIGN_IN_AGAIN {
                    format!("{}{} ({waiting} waiting)", reason[..1].to_uppercase(), &reason[1..])
                } else if let Some(minutes) = rate_limit_minutes(&reason) {
                    match minutes {
                        Some(m) => format!("Rate limit reached, retries in {m} min ({waiting} waiting)"),
                        None => format!("Rate limit reached, retries later ({waiting} waiting)"),
                    }
                } else {
                    format!("Saved, sends when the service answers ({waiting} waiting)")
                },
                false,
                format!("report {id} waits ({waiting} waiting): {reason}"),
                Level::Warn,
            ),
            Delivery::Failed { id, reason } => (
                id.clone(),
                "Report refused: see the console".to_string(),
                false,
                format!("report {id} refused (kept on this PC): {reason}"),
                Level::Error,
            ),
        };
        console.print(level, line);
        if feedback.awaiting.as_deref() == Some(id.as_str()) {
            feedback.awaiting = None;
            feedback.notice = Some(Notice { text, until, sent });
        } else if sent && feedback.awaiting.is_none() {
            // A report kept earlier (this run or an earlier one) went.
            feedback.notice = Some(Notice { text: "A kept report was sent".into(), until, sent });
        }
    }
}

/// The server joined: its asset service is where reports go.
fn joined(
    outbox: Res<FeedbackOutbox>,
    service: Option<Res<crate::net::assets::JoinedService>>,
    progress: Option<Res<crate::net::client::JoinProgress>>,
    mut last: Local<Option<(Option<(String, String)>, Option<String>)>>,
) {
    let now = (
        service.and_then(|s| s.0.clone()),
        progress.and_then(|p| p.server.as_ref().map(|s| s.name.clone())),
    );
    if last.as_ref() != Some(&now) || outbox.is_changed() {
        if last.is_some() || now != (None, None) {
            outbox.0.joined(now.0.clone(), now.1.clone());
        }
        *last = Some(now);
    }
}

/// The signed-in account changed (`account::live`: onboarding, Settings >
/// Account, a join's sign-in): reports go with its ticket, and kept ones
/// go now.
fn signed_in(outbox: Res<FeedbackOutbox>, mut last: Local<Option<Option<usize>>>) {
    let live = chimera_core::mount::assets::account::live();
    let now = live.as_ref().map(|s| Arc::as_ptr(s) as usize);
    if *last != Some(now) {
        if last.is_some() || live.is_some() {
            outbox.0.account(live);
        }
        *last = Some(now);
    }
}

/// The service named by hand changed (its cvars).
fn named(outbox: Res<FeedbackOutbox>, service: Res<FeedbackService>, mut last: Local<Option<FeedbackService>>) {
    if last.as_ref() != Some(&*service) || outbox.is_changed() {
        if last.is_some() || service.target().is_some() {
            outbox.0.named(service.target());
        }
        *last = Some(service.clone());
    }
}
