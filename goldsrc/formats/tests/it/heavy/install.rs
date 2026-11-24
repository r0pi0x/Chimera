//! The user's GoldSrc install for the heavy tests: `$CHIMERA_CS16_ROOT`,
//! else `[games.cs16] path` in the local config (`$CHIMERA_CONFIG`, else
//! chimera.local.toml at the repository root): the folder holding
//! `cstrike/` and `valve/`. Tests pass with a note without one.

use std::path::PathBuf;

use chimera_vfs::{LooseDir, Mount};

pub(crate) const SKIP: &str = "skipping: no Counter-Strike 1.6 install ($CHIMERA_CS16_ROOT or [games.cs16] path)";

fn config_path() -> Option<String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let file = [std::env::var_os("CHIMERA_CONFIG"), std::env::var_os("MASHUP_CONFIG")]
        .into_iter()
        .flatten()
        .map(PathBuf::from)
        .chain([root.join("chimera.local.toml"), root.join("mashup.local.toml")])
        .find(|p| p.is_file())?;
    let table: toml::Table = std::fs::read_to_string(&file).ok()?.parse().ok()?;
    Some(table.get("games")?.get("cs16")?.get("path")?.as_str()?.to_string())
}

/// The install root, or `None` (with the skip note printed).
pub(crate) fn root() -> Option<PathBuf> {
    let path = std::env::var_os("CHIMERA_CS16_ROOT")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            let p = config_path()?;
            Some(match p.strip_prefix("~/") {
                Some(rest) => {
                    PathBuf::from(std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?).join(rest)
                }
                None => PathBuf::from(p),
            })
        });
    let found = path.filter(|p| p.join("cstrike").is_dir() && p.join("valve").is_dir());
    if found.is_none() {
        eprintln!("{SKIP}");
    }
    found
}

/// The install's `cstrike` then `valve`, as loose files (case-insensitive).
pub(crate) fn mount() -> Option<Mount> {
    let root = root()?;
    let mut m = Mount::default();
    m.push(LooseDir::new(root.join("cstrike")));
    m.push(LooseDir::new(root.join("valve")));
    Some(m)
}

/// Every file under `<root>/<folder>` with extension `ext` (any case),
/// recursively, sorted.
pub(crate) fn files(folder: &str, ext: &str) -> Vec<PathBuf> {
    fn walk(dir: &std::path::Path, ext: &str, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, ext, out);
            } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case(ext)) {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    if let Some(r) = root() {
        walk(&r.join(folder), ext, &mut out);
    }
    out.sort();
    out
}
