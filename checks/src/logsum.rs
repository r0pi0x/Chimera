//! `logsum`: a build or test log cut down to what a decision needs — how big
//! it was, what failed first, and what the end says. A passing test prints a
//! line of its own: the fast tier's log held 4,417 lines, 3,929 of them
//! `... ok` for tests that passed (2026-10-30), and that is what a reader pays
//! for on every turn it keeps the log in context.
//!
//! `cargo run -q -p repo_checks --bin logsum -- <log> [--tail N] [--patterns N]`
//!
//! Reads UTF-8 or UTF-16: PowerShell 5.1 writes UTF-16LE from `>` and
//! `Tee-Object`, which also makes the file look binary to tools that expect
//! UTF-8. The output is always UTF-8.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io;
use std::path::Path;

/// Longest line repeated in the output, in characters.
pub const CLIP: usize = 200;
/// Lines in the tail when the caller gives no count.
pub const TAIL: usize = 40;
/// Repeated patterns listed when the caller gives no count.
pub const PATTERNS: usize = 8;

/// A log read as text, whichever of the two encodings it was written in.
pub fn read(path: &Path) -> io::Result<String> {
    Ok(decode(&std::fs::read(path)?))
}

/// `bytes` as text: UTF-16 through its byte-order mark (or its tell-tale NUL
/// bytes), UTF-8 otherwise. A leading UTF-8 byte-order mark is dropped.
pub fn decode(bytes: &[u8]) -> String {
    let text = match bytes {
        [0xFF, 0xFE, rest @ ..] => from_utf16(rest, false),
        [0xFE, 0xFF, rest @ ..] => from_utf16(rest, true),
        _ if looks_like_utf16(bytes) => from_utf16(bytes, false),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    };
    text.strip_prefix('\u{feff}').unwrap_or(&text).to_string()
}

/// UTF-16 without a byte-order mark: text has NUL bytes where the high half
/// of most units sits, which never happens in the UTF-8 these tools write.
fn looks_like_utf16(bytes: &[u8]) -> bool {
    let (units, _) = bytes[..bytes.len().min(400)].as_chunks::<2>();
    !units.is_empty() && units.iter().filter(|u| u[1] == 0).count() * 10 > units.len() * 6
}

fn from_utf16(bytes: &[u8], big_endian: bool) -> String {
    let (units, _) = bytes.as_chunks::<2>();
    let units: Vec<u16> = units
        .iter()
        .map(|c| {
            if big_endian {
                u16::from_be_bytes(*c)
            } else {
                u16::from_le_bytes(*c)
            }
        })
        .collect();
    String::from_utf16_lossy(&units)
}

/// What a log reduces to.
pub struct Summary {
    pub lines: usize,
    pub failures: usize,
    pub warnings: usize,
    /// The first failure, as its line number and text.
    pub first_failure: Option<(usize, String)>,
    pub tail: Vec<String>,
    /// Repeated line shapes (digits collapsed), most frequent first.
    pub patterns: Vec<(usize, String)>,
}

/// Whether a line reports a failure.
fn is_failure(line: &str) -> bool {
    line.contains("error[")
        || line.contains("error:")
        || line.contains("panicked at")
        || line.contains(" FAILED")
        || line.contains("... FAILED")
        || line.contains(": FAILED")
}

/// A line's shape, so counts of near-identical lines group: digit runs
/// collapse to `#`, and a test's own name drops out, because a passing test
/// prints a line of its own (`test a::b ... ok`) and the shape is what says
/// how many there were.
fn pattern(line: &str) -> String {
    let trimmed = line.trim();
    if let Some(rest) = trimmed.strip_prefix("test ")
        && let Some((_, outcome)) = rest.rsplit_once(" ... ")
    {
        return format!("test ... {outcome}");
    }
    let mut out = String::with_capacity(80);
    let mut digits = false;
    for c in trimmed.chars().take(80) {
        if c.is_ascii_digit() {
            if !digits {
                out.push('#');
            }
            digits = true;
        } else {
            digits = false;
            out.push(c);
        }
    }
    out.trim().to_string()
}

/// `line` cut to `CLIP` characters on a character boundary.
fn clip(line: &str) -> String {
    match line.char_indices().nth(CLIP) {
        Some((i, _)) => format!("{}...", &line[..i]),
        None => line.trim_end().to_string(),
    }
}

/// Reduce `text` to its counts, first failure, tail and repeated patterns.
pub fn summarize(text: &str, tail: usize, patterns: usize) -> Summary {
    let lines: Vec<&str> = text.lines().collect();
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut failures = 0;
    let mut warnings = 0;
    let mut first_failure = None;
    for (i, line) in lines.iter().enumerate() {
        if is_failure(line) {
            failures += 1;
            first_failure.get_or_insert_with(|| (i + 1, clip(line)));
        }
        if line.contains("warning:") {
            warnings += 1;
        }
        let key = pattern(line);
        if !key.is_empty() {
            *counts.entry(key).or_default() += 1;
        }
    }
    let mut ranked: Vec<(usize, String)> = counts.into_iter().map(|(k, n)| (n, k)).collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    ranked.truncate(patterns);
    Summary {
        lines: lines.len(),
        failures,
        warnings,
        first_failure,
        tail: lines[lines.len().saturating_sub(tail)..]
            .iter()
            .map(|l| clip(l))
            .collect(),
        patterns: ranked,
    }
}

/// `summary` as the text to print: a few lines, never the log.
pub fn render(name: &str, summary: &Summary, tail: usize, patterns: usize) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{name}: {} lines, {} failures, {} warnings",
        summary.lines, summary.failures, summary.warnings
    );
    match &summary.first_failure {
        Some((line, text)) => {
            let _ = writeln!(out, "first failure, line {line}: {text}");
        }
        None => out.push_str("first failure: none\n"),
    }
    let _ = writeln!(out, "tail ({tail} lines):");
    for line in &summary.tail {
        let _ = writeln!(out, "  {line}");
    }
    let _ = writeln!(out, "repeated patterns ({patterns}):");
    for (n, shape) in &summary.patterns {
        let _ = writeln!(out, "  {n}x {shape}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_is_read_through_its_byte_order_mark() {
        let mut bytes = vec![0xFF, 0xFE];
        for unit in "ok\r\nfailed\n".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        assert_eq!(decode(&bytes), "ok\r\nfailed\n");
    }

    #[test]
    fn utf16_without_a_mark_is_recognised_by_its_nul_bytes() {
        let mut bytes = Vec::new();
        for unit in "cargo test\n".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        assert_eq!(decode(&bytes), "cargo test\n");
    }

    #[test]
    fn utf8_passes_through() {
        assert_eq!(decode("plain\ntext\n".as_bytes()), "plain\ntext\n");
    }

    #[test]
    fn counts_and_the_first_failure() {
        let log = "test a ... ok\ntest b ... ok\nwarning: unused\n\
                   test c ... ok\nerror[E0308]: mismatched types\ntest d ... ok\n";
        let s = summarize(log, 2, 2);
        assert_eq!(s.lines, 6);
        assert_eq!(s.failures, 1);
        assert_eq!(s.warnings, 1);
        assert_eq!(s.first_failure, Some((5, "error[E0308]: mismatched types".into())));
        assert_eq!(s.tail.len(), 2);
    }

    #[test]
    fn passing_tests_collapse_into_one_pattern() {
        let log = "test a::b ... ok\ntest a::c ... ok\ntest a::d ... ok\n";
        let s = summarize(log, 0, 5);
        assert_eq!(s.patterns[0], (3, "test ... ok".to_string()));
    }

    #[test]
    fn other_lines_keep_their_shape_with_digits_collapsed() {
        let log = "warning: unused import in src/a.rs:12\nwarning: unused import in src/a.rs:934\n";
        let s = summarize(log, 0, 5);
        assert_eq!(s.patterns[0], (2, "warning: unused import in src/a.rs:#".to_string()));
    }

    #[test]
    fn the_tail_is_the_end_of_the_log() {
        let log = "one\ntwo\nthree\n";
        let s = summarize(log, 1, 1);
        assert_eq!(s.tail, ["three"]);
        assert!(render("x.log", &s, 1, 1).starts_with("x.log: 3 lines"));
    }
}
