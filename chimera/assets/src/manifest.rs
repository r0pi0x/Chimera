//! A game build's manifest: which record each file path is, and named sets
//! of paths (`de_dust2`: what one map load reads). Plain text, one line per
//! fact, sorted, so the same files always give the same bytes and two
//! builds diff by line:
//!
//! ```text
//! chimera-manifest 1
//! game cs_source
//! build 25338318
//! app 240
//! complete 1
//! file <id> <size> <stored size> <path>
//! set <name> <path>
//! ```
//!
//! The path comes last on its line (it may hold spaces). `complete 1`
//! marks a manifest of every file the game mounts (`chimera_pack publish`
//! of a whole install), not only of some sets: served alone, such a build
//! stands in for a whole install. Ids and sizes are facts about files,
//! never the files' contents.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::Path,
};

/// The format line's version.
pub const FORMAT: u32 = 1;

/// One file of a build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileRecord {
    /// The record's id (SHA-256 of the plain bytes).
    pub id: String,
    /// Plain size in bytes.
    pub size: u64,
    /// Size in the store (compressed).
    pub stored: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Manifest {
    pub game: String,
    pub build: String,
    /// The Steam app, when known.
    pub app: Option<u32>,
    /// Every file the game mounts is listed (a whole install packed).
    pub complete: bool,
    /// Normalized path -> record.
    pub files: BTreeMap<String, FileRecord>,
    /// Set name -> its paths (each in `files`).
    pub sets: BTreeMap<String, BTreeSet<String>>,
}

impl Manifest {
    pub fn new(game: &str, build: &str, app: Option<u32>) -> Self {
        Self { game: game.into(), build: build.into(), app, ..Default::default() }
    }

    /// The manifest as text (deterministic).
    pub fn to_text(&self) -> String {
        let mut out = format!("chimera-manifest {FORMAT}\ngame {}\nbuild {}\n", self.game, self.build);
        if let Some(app) = self.app {
            out.push_str(&format!("app {app}\n"));
        }
        if self.complete {
            out.push_str("complete 1\n");
        }
        for (path, f) in &self.files {
            out.push_str(&format!("file {} {} {} {path}\n", f.id, f.size, f.stored));
        }
        for (name, paths) in &self.sets {
            for path in paths {
                out.push_str(&format!("set {name} {path}\n"));
            }
        }
        out
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let mut m = Manifest::default();
        let mut lines = text.lines().enumerate();
        match lines.next() {
            Some((_, l)) if l == format!("chimera-manifest {FORMAT}") => {}
            _ => return Err(format!("not a version {FORMAT} manifest")),
        }
        for (n, line) in lines {
            let err = |what: &str| format!("line {}: {what}", n + 1);
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, rest) = line.split_once(' ').ok_or_else(|| err("no value"))?;
            match key {
                "game" => m.game = rest.into(),
                "build" => m.build = rest.into(),
                "app" => m.app = Some(rest.parse().map_err(|_| err("bad app id"))?),
                "complete" => m.complete = rest == "1",
                "file" => {
                    let mut parts = rest.splitn(4, ' ');
                    let (Some(id), Some(size), Some(stored), Some(path)) =
                        (parts.next(), parts.next(), parts.next(), parts.next())
                    else {
                        return Err(err("file needs id, size, stored size and path"));
                    };
                    if !crate::store::is_id(id) {
                        return Err(err("bad record id"));
                    }
                    let size = size.parse().map_err(|_| err("bad size"))?;
                    let stored = stored.parse().map_err(|_| err("bad stored size"))?;
                    m.files.insert(path.into(), FileRecord { id: id.into(), size, stored });
                }
                "set" => {
                    let (name, path) = rest.split_once(' ').ok_or_else(|| err("set needs a name and a path"))?;
                    m.sets.entry(name.into()).or_default().insert(path.into());
                }
                _ => return Err(err(&format!("unknown key `{key}`"))),
            }
        }
        if m.game.is_empty() || m.build.is_empty() {
            return Err("manifest without game or build".into());
        }
        if let Some((name, path)) =
            m.sets.iter().find_map(|(n, ps)| ps.iter().find(|p| !m.files.contains_key(*p)).map(|p| (n, p)))
        {
            return Err(format!("set {name}: {path} is not among the files"));
        }
        Ok(m)
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        crate::store::write_atomic(path, self.to_text().as_bytes())
    }

    /// Distinct records the manifest refers to, with their plain and
    /// stored sizes.
    pub fn records(&self) -> BTreeMap<&str, (u64, u64)> {
        self.files.values().map(|f| (f.id.as_str(), (f.size, f.stored))).collect()
    }

    /// Distinct records of set `name` and their total plain and stored
    /// bytes.
    pub fn set_size(&self, name: &str) -> (usize, u64, u64) {
        let ids: BTreeMap<&str, (u64, u64)> = self
            .sets
            .get(name)
            .into_iter()
            .flatten()
            .filter_map(|p| self.files.get(p))
            .map(|f| (f.id.as_str(), (f.size, f.stored)))
            .collect();
        (ids.len(), ids.values().map(|v| v.0).sum(), ids.values().map(|v| v.1).sum())
    }
}
