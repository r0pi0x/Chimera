//! In-game feedback and bug reports, the data and the receiver
//! (docs/plans/active/feedback-reports.md, P1a). No Bevy: the game's side
//! (`chimera_client::client::feedback`, P1b) collects a `ReportState` and
//! hands it to a `Feedback` (`outbox`), which builds the bundle, queues it
//! on disk and sends it from its own thread.
//!
//! - `bundle`: the zip (allow-listed entries, caps), built from a
//!   `ReportState` only; `redact`: the secret filter every line passes;
//!   `zip`: our minimal zip writer and reader.
//! - `outbox`: the tester's queue (`<data_local>/chimera/feedback/outbox/`),
//!   the sender thread with retry and backoff, where reports go
//!   (`Targets`, `resolve`).
//! - `inbox`: the receiver behind the asset service's `POST /feedback`
//!   (`server`): validation, limits, mute, one folder per report with
//!   `meta.toml` and `history.log`, statuses; `page`: the triage page and
//!   its JSON on the plain listener only (this machine).
//! - `session`: a running game's or server's diagnostics: the rolling log
//!   (`<data_local>/chimera/logs/`), the unclean-exit marker, the panic
//!   hook and (Windows) the minidump filter, crash bundles (`Kind::Crash`)
//!   and their signatures for grouping.
//!
//! `[feedback]` in the local config (`LocalFeedback`): `url`, `token`,
//! `fingerprint` name a service by hand (developers); `on_report` is the
//! receiving host's command run for each new report (a Discord ping:
//! its ids stay in that file, never in the repository).

pub mod bundle;
pub mod inbox;
pub mod outbox;
pub mod page;
pub mod redact;
pub mod session;
pub mod zip;

use serde::Deserialize;

pub use bundle::{CvarValue, ImageFormat, Kind, ReportState, Screenshot};
pub use outbox::{Destination, Feedback, OutboxConfig, OutboxEvent, OutboxStatus, Submitted, Target, Targets};

/// The most bytes a bundle may have (the client refuses more, the
/// receiver answers 413).
pub const MAX_BUNDLE: usize = 8 * 1024 * 1024;
/// The most characters a tester's text may have.
pub const MAX_TEXT_CHARS: usize = 2000;

/// `[feedback]` in chimera.local.toml.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct LocalFeedback {
    /// A report service named by hand (`https://...`).
    pub url: Option<String>,
    pub token: Option<String>,
    /// Its certificate's SHA-256 (hex), pinned.
    pub fingerprint: Option<String>,
    /// The receiving host's command for each new report (`inbox`).
    pub on_report: Option<String>,
}

impl LocalFeedback {
    /// `[feedback]` of the local config (only that section is read).
    pub fn load() -> Result<Self, String> {
        crate::config::local_section("feedback")
    }

    /// The hand-named service, when `url` and `token` are both set.
    pub fn target(&self) -> Option<Target> {
        let url = self.url.clone().filter(|u| !u.trim().is_empty())?;
        let token = self.token.clone().filter(|t| !t.trim().is_empty())?;
        Some(Target { url, token, fingerprint: self.fingerprint.clone().filter(|f| !f.trim().is_empty()) })
    }
}

pub(crate) fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Unix seconds as (year, month, day, hour, minute, second), UTC.
pub(crate) fn civil(secs: u64) -> (u32, u32, u32, u32, u32, u32) {
    let days = (secs / 86_400) as i64;
    let rest = (secs % 86_400) as u32;
    // Days since 0000-03-01 in 400-year eras (Howard Hinnant's method).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = (yoe + era * 400 + i64::from(month <= 2)) as u32;
    (year, month, day, rest / 3_600, rest % 3_600 / 60, rest % 60)
}

/// `YYYY-MM-DD HH:MM:SS UTC`.
pub fn utc_text(secs: u64) -> String {
    let (y, mo, d, h, mi, s) = civil(secs);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02} UTC")
}

/// `YYYY-MM-DD` (the receiver's day folders).
pub fn utc_day(secs: u64) -> String {
    let (y, mo, d, ..) = civil(secs);
    format!("{y:04}-{mo:02}-{d:02}")
}

/// A new report id: a random UUID v4, lowercase.
pub fn new_report_id() -> String {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).expect("the OS's randomness");
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h = crate::store::hex(&b);
    format!("{}-{}-{}-{}-{}", &h[..8], &h[8..12], &h[12..16], &h[16..20], &h[20..])
}

/// Whether `id` is a report id (a lowercase UUID: safe as a folder name).
pub fn is_report_id(id: &str) -> bool {
    id.len() == 36
        && id.char_indices().all(|(i, c)| {
            if matches!(i, 8 | 13 | 18 | 23) { c == '-' } else { c.is_ascii_digit() || ('a'..='f').contains(&c) }
        })
}

/// This machine's install id (`<feedback folder>/install-id`), made on
/// first use: groups one machine's reports; random, no hardware serial.
pub fn install_id(folder: &std::path::Path) -> String {
    let path = folder.join("install-id");
    if let Ok(id) = std::fs::read_to_string(&path)
        && id.trim().len() == 32
        && id.trim().bytes().all(|c| c.is_ascii_hexdigit())
    {
        return id.trim().to_string();
    }
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).expect("the OS's randomness");
    let id = crate::store::hex(&b);
    let _ = crate::store::write_atomic(&path, id.as_bytes());
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_dates() {
        let id = new_report_id();
        assert!(is_report_id(&id), "{id}");
        assert!(!is_report_id("../../etc/passwd"));
        assert!(!is_report_id(&id.to_uppercase()));
        assert_eq!(utc_text(1_763_078_400), "2025-11-14 00:00:00 UTC");
        assert_eq!(utc_day(1_763_078_400 + 86_399), "2025-11-14");
    }
}
