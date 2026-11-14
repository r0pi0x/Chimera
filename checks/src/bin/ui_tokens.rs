//! Writes the UI tokens' CSS (`repo_checks::ui_tokens`) to
//! `apps/ultimate_gun_game/data/tokens.css`, and to `--out <path>` too
//! (the look book's folder). `--check` writes nothing and exits 1 when the
//! committed file is stale.

use std::{path::PathBuf, process::ExitCode};

fn root() -> PathBuf {
    let mut dir = std::env::current_dir().expect("working directory");
    loop {
        if dir.join("Cargo.toml").exists() && dir.join("docs").is_dir() {
            return dir;
        }
        assert!(dir.pop(), "run inside the repository");
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("usage: ui_tokens [--check] [--out <path>]\nWrites apps/ultimate_gun_game/data/tokens.css from the UI theme.");
        return ExitCode::SUCCESS;
    }
    let root = root();
    let css = match repo_checks::ui_tokens::css(&root) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ui_tokens: {e}");
            return ExitCode::FAILURE;
        }
    };
    let path = root.join(repo_checks::ui_tokens::TOKENS_CSS);
    if args.iter().any(|a| a == "--check") {
        let now = std::fs::read_to_string(&path).unwrap_or_default();
        if now != css {
            eprintln!("{} is stale: run `cargo run -q -p repo_checks --bin ui_tokens`", repo_checks::ui_tokens::TOKENS_CSS);
            return ExitCode::FAILURE;
        }
        return ExitCode::SUCCESS;
    }
    std::fs::write(&path, &css).expect("write tokens.css");
    println!("wrote {}", repo_checks::ui_tokens::TOKENS_CSS);
    if let Some(i) = args.iter().position(|a| a == "--out")
        && let Some(out) = args.get(i + 1)
    {
        std::fs::write(out, &css).expect("write --out");
        println!("wrote {out}");
    }
    ExitCode::SUCCESS
}
