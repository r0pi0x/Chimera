//! A feedback report's bundle (docs/plans/active/feedback-reports.md, 2.3):
//! a zip built by `build` from a `ReportState` alone, never from files on
//! disk, with only the allow-listed entries (`ENTRIES`), each under its cap,
//! every line through the secret filter (`redact`), cvars named like
//! secrets and server-private ones dropped. `open` reads one back with the
//! same allow-list and caps (the receiver's validation).
//!
//! The game (`chimera_client::client::feedback`) fills the `ReportState`:
//! the text sections (`game`, `system`, `perf`, `world`, `net`) as it
//! formats them, cvars and binds as lists, console and log lines as they
//! were; the log's repeated lines are folded with their counts
//! (`fold_repeats`). A crash
//! report (`Kind::Crash`, made by `session`) adds `backtrace.txt`,
//! `health.txt` and, from a native crash on Windows, `minidump.dmp` (its
//! home folders scrubbed; a binary the filter can't read otherwise).

use serde::{Deserialize, Serialize};

use super::{MAX_BUNDLE, MAX_TEXT_CHARS, new_report_id, redact, utc_text, zip};

/// The bundle's format version (`report.json`'s `schema`).
pub const SCHEMA: u32 = 1;
/// Console lines kept (the last ones).
pub const CONSOLE_LINES: usize = 300;
/// Log lines kept (the last ones).
pub const LOG_LINES: usize = 500;
/// Log lines a crash report keeps (the session's tail).
pub const CRASH_LOG_LINES: usize = 3000;
/// A minidump's first bytes.
pub const MINIDUMP_MAGIC: &[u8] = b"MDMP";

/// Every entry a bundle may hold, with its cap in bytes.
pub const ENTRIES: &[(&str, usize)] = &[
    ("report.json", 64 * 1024),
    ("screenshot.jpg", 7 * 1024 * 1024),
    ("screenshot.png", 7 * 1024 * 1024),
    ("game.txt", 256 * 1024),
    ("system.txt", 64 * 1024),
    ("perf.txt", 256 * 1024),
    ("world.txt", 128 * 1024),
    ("net.txt", 256 * 1024),
    ("cvars.txt", 256 * 1024),
    ("console.txt", 512 * 1024),
    ("log.txt", 384 * 1024),
    // Crash reports (`Kind::Crash`, `session`).
    ("backtrace.txt", 256 * 1024),
    ("health.txt", 256 * 1024),
    ("minidump.dmp", 6 * 1024 * 1024),
];

/// An entry's cap (None: not allowed).
pub fn cap(name: &str) -> Option<usize> {
    ENTRIES.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
}

/// What a report is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    Bug,
    Feedback,
    /// Made by the game itself: a panic, a native crash, a render device
    /// lost, or a session that ended without a clean exit (`session`).
    Crash,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Bug => "bug",
            Kind::Feedback => "feedback",
            Kind::Crash => "crash",
        }
    }
}

/// The screenshot's encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFormat {
    Jpeg,
    Png,
}

/// The frame at the key press, encoded by the game (JPEG ~85, at most
/// 1920 wide, is what the plan asks; PNG is accepted).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Screenshot {
    pub format: ImageFormat,
    pub bytes: Vec<u8>,
}

impl Screenshot {
    /// Its entry's name.
    pub fn entry(&self) -> &'static str {
        match self.format {
            ImageFormat::Jpeg => "screenshot.jpg",
            ImageFormat::Png => "screenshot.png",
        }
    }

    /// Whether the bytes are what the format says.
    pub fn valid(&self) -> bool {
        match self.format {
            ImageFormat::Jpeg => self.bytes.starts_with(&[0xFF, 0xD8, 0xFF]),
            ImageFormat::Png => self.bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        }
    }
}

/// One cvar as the game has it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CvarValue {
    pub name: String,
    pub value: String,
    pub default: String,
    /// A server-scope cvar that isn't replicated (bots, secrets): dropped.
    pub server_private: bool,
}

/// Everything a report says, collected by the game at the key press. The
/// only input of `build`.
#[derive(Clone, Debug, Default)]
pub struct ReportState {
    /// A UUID v4 (`new_report_id`); empty: one is made.
    pub id: String,
    pub kind: Kind,
    /// What the tester wrote (at most `MAX_TEXT_CHARS`; more is cut).
    pub text: String,
    /// Unix seconds, UTC; 0: now.
    pub time: u64,
    /// `BuildVersion`.
    pub build: String,
    /// `NET_VERSION`.
    pub net_version: String,
    /// The app (`ultimate_gun_game`, `cs_source`, ...).
    pub app: String,
    /// A random id kept in the feedback folder (`install_id`).
    pub install_id: String,
    pub screenshot: Option<Screenshot>,
    /// `game.txt`: map, minigame and rung, position, weapon, health, the
    /// server's name (not its address), the asset sets fetched.
    pub game: String,
    /// `system.txt`: OS, CPU, RAM, GPU, window, vsync, `fps_max`.
    pub system: String,
    /// `perf.txt`: the last `chimera_perf` readout, frame distributions,
    /// hitches.
    pub perf: String,
    /// `world.txt`: what the world holds and how it grew (entities by
    /// kind, archetypes), asset stores and memory by kind, the last
    /// minutes' event timeline.
    pub world: String,
    /// `net.txt`: ping, loss, choke, rates, reconnects.
    pub net: String,
    /// Cvars that differ from their default.
    pub cvars: Vec<CvarValue>,
    /// Changed binds: key, command.
    pub binds: Vec<(String, String)>,
    /// The console's last lines (the last `CONSOLE_LINES` are kept).
    pub console: Vec<String>,
    /// The `tracing` log's last lines (the last `LOG_LINES` are kept).
    pub log: Vec<String>,
    /// `backtrace.txt` (a crash): the panic's message, place and stack.
    pub backtrace: String,
    /// `health.txt` (a crash): the session's last health lines.
    pub health: Vec<String>,
    /// `minidump.dmp` (a native crash on Windows), home folders scrubbed
    /// (`session::scrub_dump`).
    pub minidump: Option<Vec<u8>>,
}

/// `report.json`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportInfo {
    pub schema: u32,
    pub id: String,
    pub kind: Kind,
    pub text: String,
    pub time: u64,
    #[serde(default)]
    pub time_utc: String,
    #[serde(default)]
    pub build: String,
    #[serde(default)]
    pub net_version: String,
    #[serde(default)]
    pub app: String,
    #[serde(default)]
    pub install_id: String,
    /// Why there is no screenshot though one was taken ("too large").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshot_dropped: Option<String>,
    /// Why there is no minidump though the crash left one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minidump_dropped: Option<String>,
}

/// A built bundle.
#[derive(Clone, Debug)]
pub struct Bundle {
    pub id: String,
    pub bytes: Vec<u8>,
    /// The screenshot was left out (too large).
    pub screenshot_dropped: bool,
}

/// The last `max` lines, each through the filter, joined.
fn lines(all: &[String], max: usize) -> String {
    let start = all.len().saturating_sub(max);
    let mut out = String::new();
    for l in &all[start..] {
        for part in l.lines() {
            out.push_str(&redact::line(part));
            out.push('\n');
        }
    }
    out
}

/// Log lines with repeats folded: lines alike but for their digits (and
/// the time each starts with) are one, at the last one's place, ending
/// in `(xN since <the first's time>)`: a warning every frame or a sound
/// started each second takes a line, not the whole log.
pub fn fold_repeats(all: &[String]) -> Vec<String> {
    use std::collections::HashMap;
    let shape = |l: &str| -> String {
        let body = l.split_once(' ').map_or(l, |(_, rest)| rest);
        body.chars().map(|c| if c.is_ascii_digit() { '#' } else { c }).collect()
    };
    let shapes: Vec<String> = all.iter().map(|l| shape(l)).collect();
    // Per shape: how many, the first's time, the last's index.
    let mut seen: HashMap<&str, (usize, &str, usize)> = HashMap::new();
    for (i, (l, s)) in all.iter().zip(&shapes).enumerate() {
        let e = seen.entry(s.as_str()).or_insert((0, l.split(' ').next().unwrap_or(""), i));
        e.0 += 1;
        e.2 = i;
    }
    let mut out = Vec::with_capacity(seen.len());
    for (i, (l, s)) in all.iter().zip(&shapes).enumerate() {
        let (n, first, last) = seen[s.as_str()];
        if last != i {
            continue;
        }
        out.push(if n > 1 { format!("{l} (x{n} since {first})") } else { l.clone() });
    }
    out
}

/// `text` cut to `cap` bytes at a character boundary, keeping its end
/// (the latest lines) when `keep_end`.
fn fit(text: String, cap: usize, keep_end: bool) -> String {
    if text.len() <= cap {
        return text;
    }
    if keep_end {
        let mut at = text.len() - cap + 40;
        while !text.is_char_boundary(at) {
            at += 1;
        }
        format!("(earlier lines cut)\n{}", &text[at..])
    } else {
        let mut at = cap - 40;
        while !text.is_char_boundary(at) {
            at -= 1;
        }
        format!("{}\n(cut)\n", &text[..at])
    }
}

fn cut_chars(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

/// The bundle of `state`.
pub fn build(state: &ReportState) -> Result<Bundle, String> {
    let id = if state.id.is_empty() { new_report_id() } else { state.id.clone() };
    if !super::is_report_id(&id) {
        return Err(format!("{id:?} isn't a report id"));
    }
    let time = if state.time == 0 { super::now_secs() } else { state.time };
    let mut info = ReportInfo {
        schema: SCHEMA,
        id: id.clone(),
        kind: state.kind,
        text: redact::text(&cut_chars(&state.text, MAX_TEXT_CHARS)),
        time,
        time_utc: utc_text(time),
        build: redact::line(&cut_chars(&state.build, 200)),
        net_version: redact::line(&cut_chars(&state.net_version, 100)),
        app: redact::line(&cut_chars(&state.app, 100)),
        install_id: cut_chars(&state.install_id, 64).chars().filter(char::is_ascii_alphanumeric).collect(),
        screenshot_dropped: None,
        minidump_dropped: None,
    };
    let mut cvars = String::new();
    for c in state.cvars.iter().filter(|c| !c.server_private && !redact::secret_name(&c.name)) {
        cvars += &redact::line(&format!("{} \"{}\" (default \"{}\")", c.name, c.value, c.default));
        cvars.push('\n');
    }
    if !state.binds.is_empty() {
        cvars += "\nbinds:\n";
        for (key, command) in &state.binds {
            cvars += &redact::line(&format!("bind {key} \"{command}\""));
            cvars.push('\n');
        }
    }
    let texts: [(&str, String, bool); 10] = [
        ("game.txt", redact::text(&state.game), false),
        ("system.txt", redact::text(&state.system), false),
        ("perf.txt", redact::text(&state.perf), false),
        ("world.txt", redact::text(&state.world), false),
        ("net.txt", redact::text(&state.net), true),
        ("cvars.txt", cvars, false),
        ("console.txt", lines(&state.console, CONSOLE_LINES), true),
        ("log.txt", lines(&fold_repeats(&state.log), if state.kind == Kind::Crash { CRASH_LOG_LINES } else { LOG_LINES }), true),
        ("backtrace.txt", redact::text(&state.backtrace), false),
        ("health.txt", lines(&state.health, LOG_LINES), true),
    ];
    let screenshot = state.screenshot.as_ref().filter(|s| s.valid());
    let minidump = state
        .minidump
        .as_ref()
        .filter(|d| d.starts_with(MINIDUMP_MAGIC) && d.len() <= cap("minidump.dmp").expect("listed"));
    if state.minidump.is_some() && minidump.is_none() {
        info.minidump_dropped = Some("too large or not a minidump".into());
    }
    let assemble = |info: &ReportInfo, screenshot: Option<&Screenshot>| {
        let mut w = zip::ZipWriter::new(time);
        let json = serde_json::to_vec_pretty(info).expect("plain data");
        w.add("report.json", &json, false);
        if let Some(s) = screenshot {
            w.add(s.entry(), &s.bytes, true);
        }
        for (name, body, keep_end) in &texts {
            if !body.is_empty() {
                let body = fit(body.clone(), cap(name).expect("listed"), *keep_end);
                w.add(name, body.as_bytes(), false);
            }
        }
        if let Some(d) = minidump {
            w.add("minidump.dmp", d, true);
        }
        w.finish()
    };
    let too_big = screenshot.is_some_and(|s| s.bytes.len() > cap(s.entry()).expect("listed"));
    let mut bytes = assemble(&info, if too_big { None } else { screenshot });
    let mut dropped = too_big;
    if !too_big && screenshot.is_some() && bytes.len() > MAX_BUNDLE {
        dropped = true;
    }
    if dropped {
        info.screenshot_dropped = Some("too large".into());
        bytes = assemble(&info, None);
    }
    if bytes.len() > MAX_BUNDLE {
        return Err(format!("the report is {} bytes (at most {MAX_BUNDLE})", bytes.len()));
    }
    Ok(Bundle { id, bytes, screenshot_dropped: dropped })
}

/// A bundle read back and checked.
#[derive(Clone, Debug)]
pub struct Opened {
    pub info: ReportInfo,
    pub entries: Vec<zip::Entry>,
}

/// Read `bytes` as a bundle: a zip of allow-listed entries under their
/// caps, with a `report.json` that parses, a report id, text at most
/// `MAX_TEXT_CHARS`, and a screenshot that is the image it says.
pub fn open(bytes: &[u8]) -> Result<Opened, String> {
    if bytes.len() > MAX_BUNDLE {
        return Err(format!("{} bytes (at most {MAX_BUNDLE})", bytes.len()));
    }
    let entries = zip::read(bytes, ENTRIES.len(), cap)?;
    let json = entries.iter().find(|e| e.name == "report.json").ok_or("no report.json")?;
    let info: ReportInfo = serde_json::from_slice(&json.data).map_err(|e| format!("report.json: {e}"))?;
    if !super::is_report_id(&info.id) {
        return Err("report.json: the id isn't a report id".into());
    }
    if info.text.chars().count() > MAX_TEXT_CHARS {
        return Err(format!("the text is longer than {MAX_TEXT_CHARS} characters"));
    }
    for e in &entries {
        let format = match e.name.as_str() {
            "minidump.dmp" => {
                if !e.data.starts_with(MINIDUMP_MAGIC) {
                    return Err("minidump.dmp: not a minidump".into());
                }
                continue;
            }
            "screenshot.jpg" => ImageFormat::Jpeg,
            "screenshot.png" => ImageFormat::Png,
            _ => {
                if e.name.ends_with(".txt") && std::str::from_utf8(&e.data).is_err() {
                    return Err(format!("{}: not UTF-8", e.name));
                }
                continue;
            }
        };
        if !(Screenshot { format, bytes: e.data.clone() }).valid() {
            return Err(format!("{}: not that kind of image", e.name));
        }
    }
    Ok(Opened { info, entries })
}
