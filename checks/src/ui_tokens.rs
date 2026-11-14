//! The UI tokens as CSS (`ui_tokens`): the one source of the Ultimate Gun
//! Game's look is its theme (chimera/ui/data/shell.toml with
//! apps/ultimate_gun_game/data/theme.toml laid over it, as
//! `chimera_ui::Theme` reads them); the look book's HTML mock-ups take
//! the same values from `apps/ultimate_gun_game/data/tokens.css`, which
//! this writes, so a colour, a padding or a duration can't differ between
//! mock-up and game. A test fails when the file is stale
//! (`ui_tokens_are_fresh`); `cargo run -q -p repo_checks --bin ui_tokens`
//! writes it.
//!
//! Names follow the look book's CSS custom properties: `--ink`,
//! `--ink-rgb`, `--fg-2` (`rgb(var(--fg-rgb)/.66)`), `--acc-tx`,
//! `--slant`, `--tan`, `--e-out` (CSS curves); new ones for the rest:
//! `--t-<piece>-<key>` (seconds), `--l-<screen>-<key>` (cqw),
//! `--type-<role>-family|weight|tracking`, `--ping-*`.

use std::{fmt::Write, path::Path};

/// Where the generated CSS lives (committed).
pub const TOKENS_CSS: &str = "apps/ultimate_gun_game/data/tokens.css";
const SHELL: &str = "chimera/ui/data/shell.toml";
const THEME: &str = "apps/ultimate_gun_game/data/theme.toml";

fn overlay(a: &mut toml::Table, b: toml::Table) {
    for (k, v) in b {
        match (a.get_mut(&k), v) {
            (Some(toml::Value::Table(at)), toml::Value::Table(bt)) => overlay(at, bt),
            (_, v) => {
                a.insert(k, v);
            }
        }
    }
}

fn num(v: &toml::Value) -> Option<f64> {
    v.as_float().or_else(|| v.as_integer().map(|i| i as f64))
}

/// A number as CSS writes it: `.66`, `12`, `-2.6`.
fn n(v: f64) -> String {
    let s = format!("{:.4}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    let s = if let Some(r) = s.strip_prefix("0.") { format!(".{r}") } else { s };
    if let Some(r) = s.strip_prefix("-0.") { format!("-.{r}") } else { s }
}

fn kebab(s: &str) -> String {
    s.replace('_', "-")
}

fn rgb(hex: &str) -> Result<String, String> {
    let h = hex.trim().trim_start_matches('#');
    if h.len() != 6 {
        return Err(format!("colour `{hex}`"));
    }
    let c = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).map_err(|e| format!("colour `{hex}`: {e}"));
    Ok(format!("{} {} {}", c(0)?, c(2)?, c(4)?))
}

/// An easing as CSS (`bezier(..)` -> `cubic-bezier(..)`, `steps(n)`), or
/// `None` for curves CSS has no word for (they stay in app.js).
fn ease_css(e: &str) -> Option<String> {
    let e: String = e.chars().filter(|c| !c.is_whitespace()).collect();
    if let Some(args) = e.strip_prefix("bezier(") {
        return Some(format!("cubic-bezier({}", args));
    }
    if e.starts_with("steps(") {
        return Some(e);
    }
    (e == "linear").then(|| e)
}

/// The tokens' CSS from the repository at `root`.
pub fn css(root: &Path) -> Result<String, String> {
    let read = |p: &str| -> Result<toml::Table, String> {
        std::fs::read_to_string(root.join(p)).map_err(|e| format!("{p}: {e}"))?.parse().map_err(|e| format!("{p}: {e}"))
    };
    let mut t = read(SHELL)?;
    overlay(&mut t, read(THEME)?);
    let table = |k: &str| t.get(k).and_then(|v| v.as_table()).cloned().unwrap_or_default();
    let mut o = String::new();
    o.push_str(&format!(
        "/* Generated from {SHELL} and {THEME} by `cargo run -q -p repo_checks --bin ui_tokens`; do not edit.\n   The game's theme is the one source: change it there and regenerate. */\n:root{{\n"
    ));
    // Palette: hex and rgb triplets (the look book derives alphas from these).
    let pal = table("palette");
    let names = |k: &str| match k {
        "acc_text" => "acc-tx".to_string(),
        k => kebab(k),
    };
    for (k, v) in &pal {
        let hex = v.as_str().ok_or(format!("palette.{k}"))?;
        writeln!(o, "  --{}:{};", names(k), hex.to_uppercase()).unwrap();
        writeln!(o, "  --{}-rgb:{};", names(k), rgb(hex)?).unwrap();
    }
    // Derived tones, display-space alphas (as the theme writes them).
    let alpha = table("alpha");
    for (k, v) in &alpha {
        let a = num(v).ok_or(format!("alpha.{k}"))?;
        let base = if k.starts_with("acc") {
            "acc"
        } else if k == "plate" {
            "ink"
        } else {
            "fg"
        };
        writeln!(o, "  --{}:rgb(var(--{base}-rgb)/{});", kebab(k), n(a)).unwrap();
    }
    // The slant.
    let shape = table("shape");
    let deg = shape.get("slant_deg").and_then(num).unwrap_or(0.0);
    writeln!(o, "  --slant:{}deg; --tan:{};", n(-deg), n(deg.to_radians().tan())).unwrap();
    if let Some(m) = shape.get("min_text_px").and_then(num) {
        writeln!(o, "  --min-text:{}px;", n(m)).unwrap();
    }
    // Easings.
    let motion = table("motion");
    if let Some(eases) = motion.get("ease").and_then(|e| e.as_table()) {
        for (k, v) in eases {
            if let Some(c) = v.as_str().and_then(ease_css) {
                let name = match k.as_str() {
                    "io" => "io".to_string(),
                    k => kebab(k),
                };
                writeln!(o, "  --e-{name}:{c};").unwrap();
            }
        }
    }
    // Durations.
    if let Some(durs) = motion.get("dur").and_then(|d| d.as_table()) {
        for (piece, keys) in durs {
            for (k, v) in keys.as_table().into_iter().flatten() {
                if let Some(x) = num(v) {
                    writeln!(o, "  --t-{}-{}:{}s;", kebab(piece), kebab(k), n(x)).unwrap();
                }
            }
        }
    }
    // Layout numbers.
    for (screen, keys) in &table("layout") {
        for (k, v) in keys.as_table().into_iter().flatten() {
            if let Some(x) = num(v) {
                writeln!(o, "  --l-{}-{}:{}cqw;", kebab(screen), kebab(k), n(x)).unwrap();
            }
        }
    }
    // Type roles.
    for (role, spec) in &table("type") {
        let spec = spec.as_table().ok_or(format!("type.{role}"))?;
        let face = spec.get("face").and_then(|f| f.as_str()).unwrap_or("");
        let family = match face {
            "display" => "\"UGG Display\"".to_string(),
            "mono" => "\"UGG Mono\"".to_string(),
            f => format!("\"{f}\""),
        };
        let r = kebab(role);
        writeln!(o, "  --type-{r}-family:{family};").unwrap();
        writeln!(o, "  --type-{r}-weight:{};", spec.get("weight").and_then(num).unwrap_or(400.0)).unwrap();
        writeln!(o, "  --type-{r}-tracking:{}em;", n(spec.get("tracking").and_then(num).unwrap_or(0.0))).unwrap();
        let feats: Vec<String> = spec
            .get("features")
            .and_then(|f| f.as_array())
            .into_iter()
            .flatten()
            .filter_map(|f| f.as_str().map(|f| format!("'{f}' 1")))
            .collect();
        writeln!(o, "  --type-{r}-features:{};", if feats.is_empty() { "normal".into() } else { feats.join(",") }).unwrap();
    }
    // The ping meter.
    for (k, v) in &table("network") {
        if let Some(x) = num(v) {
            writeln!(o, "  --{}:{};", kebab(k).replace("-ms", ""), n(x)).unwrap();
        }
    }
    o.push_str("}\n");
    Ok(o)
}
