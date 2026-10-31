//! Sums up a build or test log instead of printing it: counts, the first
//! failure, the tail and the repeated line shapes; see `repo_checks::logsum`.
//!
//! `cargo run -q -p repo_checks --bin logsum -- <log> [--tail N] [--patterns N]`

use std::path::PathBuf;
use std::process::ExitCode;

fn usage() {
    println!(
        "usage: logsum <log> [--tail N] [--patterns N]\n\
         Prints the log's line, failure and warning counts, its first failure, the last N lines \
         (default {}) and the N most repeated line shapes (default {}).\n\
         Reads UTF-8 or UTF-16; the output is UTF-8.",
        repo_checks::logsum::TAIL,
        repo_checks::logsum::PATTERNS
    );
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        usage();
        return ExitCode::FAILURE;
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        usage();
        return ExitCode::SUCCESS;
    }
    let (mut tail, mut patterns) = (repo_checks::logsum::TAIL, repo_checks::logsum::PATTERNS);
    let mut log = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--tail" => tail = rest.next().and_then(|n| n.parse().ok()).unwrap_or(tail),
            "--patterns" => patterns = rest.next().and_then(|n| n.parse().ok()).unwrap_or(patterns),
            other if other.starts_with("--") => {
                eprintln!("logsum: unknown option {other}");
                usage();
                return ExitCode::FAILURE;
            }
            other => log = Some(PathBuf::from(other)),
        }
    }
    let Some(log) = log else {
        usage();
        return ExitCode::FAILURE;
    };
    let text = match repo_checks::logsum::read(&log) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("logsum: {}: {e}", log.display());
            return ExitCode::FAILURE;
        }
    };
    let summary = repo_checks::logsum::summarize(&text, tail, patterns);
    print!(
        "{}",
        repo_checks::logsum::render(&log.display().to_string(), &summary, tail, patterns)
    );
    ExitCode::SUCCESS
}
