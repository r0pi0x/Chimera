//! A Source game's search path as a mount (docs/plans/active/hl2dm.md,
//! lift S1; specs/hl2dm/source_deltas.md, Part 2, "gameinfo.txt"): the
//! ordered layers a file name is looked up in, first match wins.
//!
//! Two ways to get one:
//!
//! - `SearchPath::from_gameinfo`: the game's own `gameinfo.txt`
//!   (`GameInfo` > `FileSystem` > `SearchPaths`), in the file's order.
//!   Each line is `<path ids> <path>`, the ids joined by `+`. A line whose
//!   ids include `game`, `mod` or `platform` is a layer; the others
//!   (`gamebin`, write-only ids) aren't. `|gameinfo_path|` is the folder
//!   holding `gameinfo.txt`, `|all_source_engine_paths|` the install root
//!   (the base folder), and a path with neither is relative to the base
//!   folder. A path ending in `.vpk` names an archive by its stem: the
//!   file mounted is `<stem>_dir.vpk`. A path ending in `/*` mounts each
//!   folder and each `_dir.vpk` inside it, in alphabetical order (any
//!   case). Every layer from `gameinfo.txt` is optional: an entry whose
//!   folder or archive is missing is skipped. The same folder or archive
//!   named twice (`|gameinfo_path|.` and the game's folder) is mounted
//!   once, at its first place. HL2:DM's file lists every VPK before any
//!   loose folder (after `custom/*`), so its archives win over loose
//!   files at the same path.
//! - `SearchPath::from_specs`: a game's own list (`Spec`: a path relative
//!   to the install, its kind, whether it may be missing), for a game
//!   whose mount keeps an order of its own: CS:S's (`cs_source::game::
//!   mount::SEARCH_PATH`), unchanged from before the lift.
//!
//! `SearchPath::mount` builds the `Mount`: a folder is a `LooseDir`
//! hiding `.vpk` files (archives are layers of their own), an archive a
//! VPK. A layer that isn't optional is mounted even when missing (a
//! missing folder is an empty layer, a missing archive an error). The
//! map's own pak (mounted in front while a map loads) and Chimera's
//! content cache (after the game's layers) are the game's to add.
//!
//! Known shortcuts:
//! - The language: Source inserts a `_<language>` copy above each search
//!   path naming `_english` when the game runs in another language; we
//!   mount English only.
//! - A single-file VPK (`<name>.vpk` with no `_dir` directory file, as a
//!   mod may drop into `custom/`) isn't mounted: the VPK reader
//!   (`source_formats::vpk`) opens directory files only.
//! - Folder names are matched as written (`hl2mp`), not in any case, on
//!   a case-sensitive file system.

use std::{
    fmt, fs, io,
    path::{Component, Path, PathBuf},
};

use chimera_core::mount::{
    LooseDir, Mount,
    keyvalues::{self, Escapes},
};
use source_formats::vpk::Vpk;

/// What a layer mounts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A folder's loose files (its `.vpk` files hidden).
    Loose,
    /// A VPK archive, by its `_dir.vpk` directory file.
    Vpk,
}

/// One layer of a search path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layer {
    /// The folder, or the archive's `_dir.vpk` file.
    pub path: PathBuf,
    pub kind: Kind,
    /// Skipped when missing (every `gameinfo.txt` entry); otherwise
    /// mounted all the same: an empty folder, or an error for an archive.
    pub optional: bool,
    /// Its path ids as `gameinfo.txt` gives them (`game`, `mod`,
    /// `platform`, `download`, ...), lowercase; empty for a game's own
    /// list.
    pub ids: Vec<String>,
}

impl Layer {
    /// Whether its folder or archive is there.
    pub fn exists(&self) -> bool {
        match self.kind {
            Kind::Loose => self.path.is_dir(),
            Kind::Vpk => self.path.is_file(),
        }
    }
}

impl fmt::Display for Layer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self.kind {
            Kind::Loose => "loose",
            Kind::Vpk => "vpk",
        };
        write!(f, "{kind:5} {}", self.path.display())?;
        if !self.ids.is_empty() {
            write!(f, "  [{}]", self.ids.join("+"))?;
        }
        if !self.exists() {
            f.write_str(if self.optional {
                "  (missing: skipped)"
            } else {
                "  (missing)"
            })?;
        }
        Ok(())
    }
}

/// A layer of a game's own list: `path` relative to the install, `/`
/// between folders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spec {
    pub path: &'static str,
    pub kind: Kind,
    pub optional: bool,
}

impl Spec {
    /// A folder, mounted even when missing (an empty layer).
    pub const fn loose(path: &'static str) -> Self {
        Self {
            path,
            kind: Kind::Loose,
            optional: false,
        }
    }

    /// An archive's `_dir.vpk`, which must be there.
    pub const fn vpk(path: &'static str) -> Self {
        Self {
            path,
            kind: Kind::Vpk,
            optional: false,
        }
    }

    /// Skipped when missing.
    pub const fn optional(self) -> Self {
        Self { optional: true, ..self }
    }
}

/// A search path: its layers, first match wins.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchPath {
    pub layers: Vec<Layer>,
}

/// A `gameinfo.txt` search path line: its path ids (lowercase) and its
/// path as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub ids: Vec<String>,
    pub path: String,
}

impl Entry {
    /// Whether the game reads files through it (rather than only writing
    /// there, or loading binaries from it).
    pub fn is_read(&self) -> bool {
        self.ids
            .iter()
            .any(|id| matches!(id.as_str(), "game" | "mod" | "platform"))
    }
}

/// The `SearchPaths` lines of a `gameinfo.txt`, in order. Empty when the
/// text has none.
pub fn entries(gameinfo: &str) -> Vec<Entry> {
    let kv = keyvalues::parse(gameinfo, Escapes::Off);
    let Some(paths) = kv
        .get("GameInfo")
        .and_then(|g| g.get("FileSystem"))
        .and_then(|f| f.get("SearchPaths"))
    else {
        return Vec::new();
    };
    paths
        .items()
        .iter()
        .filter_map(|(ids, value)| {
            let path = value.str_value()?.trim();
            (!path.is_empty()).then(|| Entry {
                ids: ids
                    .split('+')
                    .map(|id| id.trim().to_ascii_lowercase())
                    .filter(|id| !id.is_empty())
                    .collect(),
                path: path.to_string(),
            })
        })
        .collect()
}

/// `path` with its tokens replaced: under `gameinfo_dir` for
/// `|gameinfo_path|`, under `base` for `|all_source_engine_paths|` or no
/// token; `.` parts dropped, either slash.
pub fn resolve(path: &str, base: &Path, gameinfo_dir: &Path) -> PathBuf {
    let path = path.replace('\\', "/");
    let (root, rest) = if let Some(rest) = strip_token(&path, "|gameinfo_path|") {
        (gameinfo_dir, rest)
    } else if let Some(rest) = strip_token(&path, "|all_source_engine_paths|") {
        (base, rest)
    } else {
        (base, path.as_str())
    };
    let joined = if Path::new(rest).is_absolute() {
        PathBuf::from(rest)
    } else {
        root.join(rest)
    };
    joined.components().filter(|c| *c != Component::CurDir).collect()
}

fn strip_token<'a>(path: &'a str, token: &str) -> Option<&'a str> {
    let head = path.get(..token.len())?;
    head.eq_ignore_ascii_case(token).then(|| &path[token.len()..])
}

/// An archive's directory file for a `gameinfo.txt` VPK name:
/// `hl2mp_pak.vpk` mounts `hl2mp_pak_dir.vpk`.
pub fn dir_vpk(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if name.to_ascii_lowercase().ends_with("_dir.vpk") {
        return path.to_path_buf();
    }
    let stem = &name[..name.len().saturating_sub(".vpk".len())];
    path.with_file_name(format!("{stem}_dir.vpk"))
}

fn is_vpk(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".vpk")
}

impl SearchPath {
    /// A game's own list under `install` (`Spec`).
    pub fn from_specs(install: &Path, specs: &[Spec]) -> Self {
        let layers = specs
            .iter()
            .map(|s| {
                // Joined part by part, so the path reads as the system's.
                let path = s
                    .path
                    .split('/')
                    .filter(|p| !p.is_empty())
                    .fold(install.to_path_buf(), |p, part| p.join(part));
                Layer {
                    path,
                    kind: s.kind,
                    optional: s.optional,
                    ids: Vec::new(),
                }
            })
            .collect();
        Self { layers }
    }

    /// The search path `gameinfo` (a `gameinfo.txt`'s text) gives, with
    /// `base` the install root and `gameinfo_dir` the folder the file is
    /// in. Wildcards (`custom/*`) are listed now, from the disk.
    pub fn from_gameinfo(gameinfo: &str, base: &Path, gameinfo_dir: &Path) -> Self {
        let mut out = Self::default();
        for entry in entries(gameinfo).into_iter().filter(Entry::is_read) {
            let layer = |path: PathBuf, kind| Layer {
                path,
                kind,
                optional: true,
                ids: entry.ids.clone(),
            };
            if let Some(folder) = entry.path.strip_suffix("/*").or_else(|| entry.path.strip_suffix("\\*")) {
                for (path, kind) in folder_contents(&resolve(folder, base, gameinfo_dir)) {
                    out.add(layer(path, kind));
                }
            } else if is_vpk(&entry.path) {
                out.add(layer(dir_vpk(&resolve(&entry.path, base, gameinfo_dir)), Kind::Vpk));
            } else {
                out.add(layer(resolve(&entry.path, base, gameinfo_dir), Kind::Loose));
            }
        }
        out
    }

    /// The search path of the game whose folder in `install` is
    /// `game_dir` (`hl2mp`), from its `gameinfo.txt`.
    pub fn read_gameinfo(install: &Path, game_dir: &str) -> io::Result<Self> {
        let dir = install.join(game_dir);
        let file = dir.join("gameinfo.txt");
        let text = fs::read(&file).map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", file.display())))?;
        let text = String::from_utf8_lossy(&text);
        let path = Self::from_gameinfo(&text, install, &dir);
        if path.layers.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{}: no search paths (GameInfo > FileSystem > SearchPaths)",
                    file.display()
                ),
            ));
        }
        Ok(path)
    }

    /// Add `layer` unless the same folder or archive is already a layer.
    fn add(&mut self, layer: Layer) {
        if !self.layers.iter().any(|l| l.kind == layer.kind && l.path == layer.path) {
            self.layers.push(layer);
        }
    }

    /// The layers a mount gets: optional ones only when present.
    pub fn mounted(&self) -> impl Iterator<Item = &Layer> {
        self.layers.iter().filter(|l| !l.optional || l.exists())
    }

    /// The mount: each layer that is there (or isn't optional), in order.
    pub fn mount(&self) -> io::Result<Mount> {
        let mut mount = Mount::default();
        for layer in self.mounted() {
            match layer.kind {
                Kind::Loose => mount.push(LooseDir::new(&layer.path).hiding(&["vpk"])),
                Kind::Vpk => mount.push(Vpk::open(&layer.path)?),
            }
        }
        Ok(mount)
    }
}

/// A wildcard folder's layers: each sub-folder and each `_dir.vpk`, by
/// name in any case; nothing when the folder is missing.
fn folder_contents(folder: &Path) -> Vec<(PathBuf, Kind)> {
    let Ok(read) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut found: Vec<(String, PathBuf, Kind)> = read
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let path = e.path();
            if path.is_dir() {
                Some((name.to_ascii_lowercase(), path, Kind::Loose))
            } else if name.to_ascii_lowercase().ends_with("_dir.vpk") {
                Some((name.to_ascii_lowercase(), path, Kind::Vpk))
            } else {
                None
            }
        })
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found.into_iter().map(|(_, p, k)| (p, k)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Our own text in `gameinfo.txt`'s shape: comments, bare and quoted
    /// words, every token, a write-only and a binaries line, a duplicate.
    const FIXTURE: &str = r#"
"GameInfo"
{
    game "Test Game"
    type multiplayer_only
    hidden_maps { "test_a" 1 }
    FileSystem
    {
        SteamAppId 1
        SearchPaths
        {
            // user content first
            game+mod            mygame/custom/*
            game+mod            mygame/mygame_english.vpk
            "game+mod"          "mygame/mygame_pak.vpk"
            game                |all_source_engine_paths|shared/shared_misc.vpk
            platform            |ALL_SOURCE_ENGINE_PATHS|platform/platform_misc.vpk
            mod+mod_write+default_write_path    |gameinfo_path|.
            game+game_write     mygame
            gamebin             mygame/bin
            game_write          mygame/written
            game                |all_source_engine_paths|shared
            platform            |all_source_engine_paths|platform
            game+download       mygame\download
        }
    }
}
"#;

    #[test]
    fn reads_every_line_in_order() {
        let e = entries(FIXTURE);
        let pairs: Vec<(String, &str)> = e.iter().map(|e| (e.ids.join("+"), e.path.as_str())).collect();
        assert_eq!(
            pairs,
            [
                ("game+mod".into(), "mygame/custom/*"),
                ("game+mod".into(), "mygame/mygame_english.vpk"),
                ("game+mod".into(), "mygame/mygame_pak.vpk"),
                ("game".into(), "|all_source_engine_paths|shared/shared_misc.vpk"),
                ("platform".into(), "|ALL_SOURCE_ENGINE_PATHS|platform/platform_misc.vpk"),
                ("mod+mod_write+default_write_path".into(), "|gameinfo_path|."),
                ("game+game_write".into(), "mygame"),
                ("gamebin".into(), "mygame/bin"),
                ("game_write".into(), "mygame/written"),
                ("game".into(), "|all_source_engine_paths|shared"),
                ("platform".into(), "|all_source_engine_paths|platform"),
                ("game+download".into(), "mygame\\download"),
            ]
        );
        assert!(
            !e[7].is_read() && !e[8].is_read(),
            "binaries and write-only lines aren't read from"
        );
        assert!(entries("\"GameInfo\" { game x }").is_empty());
    }

    #[test]
    fn tokens_resolve_under_their_folders() {
        let base = Path::new("/inst");
        let info = Path::new("/inst/mygame");
        assert_eq!(resolve("|gameinfo_path|.", base, info), Path::new("/inst/mygame"));
        assert_eq!(
            resolve("|all_source_engine_paths|shared", base, info),
            Path::new("/inst/shared")
        );
        assert_eq!(
            resolve("|All_Source_Engine_Paths|shared", base, info),
            Path::new("/inst/shared")
        );
        assert_eq!(
            resolve("mygame\\download", base, info),
            Path::new("/inst/mygame/download")
        );
        assert_eq!(resolve("./mygame/./x", base, info), Path::new("/inst/mygame/x"));
        assert_eq!(
            dir_vpk(Path::new("/inst/mygame/mygame_pak.vpk")),
            Path::new("/inst/mygame/mygame_pak_dir.vpk")
        );
        assert_eq!(dir_vpk(Path::new("/inst/a_dir.vpk")), Path::new("/inst/a_dir.vpk"));
    }

    /// A scratch folder, removed when dropped.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let p = std::env::temp_dir().join(format!("chimera-search-path-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            Self(p)
        }

        fn write(&self, rel: &str, bytes: &[u8]) {
            let p = self.0.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, bytes).unwrap();
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// A VPK directory file (version 1) holding one file, its bytes in
    /// the directory itself.
    fn vpk_with(path: &str, data: &[u8]) -> Vec<u8> {
        let (dir, file) = path.rsplit_once('/').unwrap_or((" ", path));
        let (stem, ext) = file.rsplit_once('.').unwrap();
        let mut tree = Vec::new();
        for s in [ext, dir, stem] {
            tree.extend(s.as_bytes());
            tree.push(0);
        }
        tree.extend(0u32.to_le_bytes()); // CRC (unchecked here)
        tree.extend((data.len() as u16).to_le_bytes()); // preload bytes
        tree.extend(0x7fffu16.to_le_bytes()); // archive: the directory file
        tree.extend(0u32.to_le_bytes()); // offset
        tree.extend(0u32.to_le_bytes()); // length past the preload
        tree.extend(0xffffu16.to_le_bytes());
        tree.extend(data);
        tree.extend([0, 0, 0]); // end of names, paths, extensions
        let mut v = Vec::new();
        v.extend(0x55aa_1234u32.to_le_bytes());
        v.extend(1u32.to_le_bytes());
        v.extend((tree.len() as u32).to_le_bytes());
        v.extend(tree);
        v
    }

    #[test]
    fn builds_the_mount_in_the_files_order() {
        let s = Scratch::new("order");
        let text = FIXTURE;
        // custom/*: a folder and an archive, alphabetical in any case; a
        // stray file and an archive's data part ignored.
        s.write("mygame/custom/B_mod/cfg/who.txt", b"custom folder");
        s.write("mygame/custom/a_pack_dir.vpk", &vpk_with("cfg/who.txt", b"custom vpk"));
        s.write("mygame/custom/a_pack_000.vpk", b"");
        s.write("mygame/custom/readme.txt", b"");
        // mygame_english.vpk is missing: skipped.
        s.write(
            "mygame/mygame_pak_dir.vpk",
            &vpk_with("scripts/thing.txt", b"from the pak"),
        );
        s.write(
            "shared/shared_misc_dir.vpk",
            &vpk_with("scripts/shared.txt", b"from the shared vpk"),
        );
        s.write("mygame/scripts/thing.txt", b"loose game file");
        s.write("mygame/gameinfo.txt", text.as_bytes());
        s.write("shared/scripts/shared.txt", b"loose shared file");
        s.write("shared/scripts/only_loose.txt", b"only loose");
        s.write("mygame/download/maps/fetched.bsp", b"VBSP");
        // platform/ and mygame/bin missing.
        let path = SearchPath::read_gameinfo(&s.0, "mygame").unwrap();
        let rel: Vec<String> = path
            .layers
            .iter()
            .map(|l| l.path.strip_prefix(&s.0).unwrap().to_string_lossy().replace('\\', "/"))
            .collect();
        assert_eq!(
            rel,
            [
                "mygame/custom/a_pack_dir.vpk",
                "mygame/custom/B_mod",
                "mygame/mygame_english_dir.vpk",
                "mygame/mygame_pak_dir.vpk",
                "shared/shared_misc_dir.vpk",
                "platform/platform_misc_dir.vpk",
                "mygame",
                "shared",
                "platform",
                "mygame/download",
            ],
            "mygame named twice is one layer; binaries and write-only folders aren't layers"
        );
        let mounted: Vec<&Layer> = path.mounted().collect();
        assert_eq!(
            mounted.len(),
            7,
            "the missing archives and platform/ skipped: {mounted:#?}"
        );
        let mount = path.mount().unwrap();
        assert_eq!(mount.layers().count(), 7);
        // custom/*'s archive first, then its folder.
        assert_eq!(mount.read("cfg/who.txt").unwrap(), b"custom vpk");
        assert_eq!(mount.read_every("cfg/who.txt").len(), 2);
        // Archives before loose folders.
        assert_eq!(mount.read("scripts/thing.txt").unwrap(), b"from the pak");
        assert_eq!(mount.read("scripts/shared.txt").unwrap(), b"from the shared vpk");
        assert_eq!(mount.read("scripts/only_loose.txt").unwrap(), b"only loose");
        // Any case, either slash.
        assert_eq!(mount.read("SCRIPTS\\Thing.TXT").unwrap(), b"from the pak");
        assert_eq!(mount.read("maps/fetched.bsp").unwrap(), b"VBSP");
        // The loose folders hide their archives.
        assert!(!mount.contains("custom/a_pack_dir.vpk"));
    }

    #[test]
    fn a_games_own_list_keeps_its_order_and_rules() {
        let s = Scratch::new("specs");
        s.write("g/g_pak_dir.vpk", &vpk_with("a.txt", b"pak"));
        s.write("g/a.txt", b"loose");
        const SPECS: &[Spec] = &[
            Spec::loose("g/custom"),
            Spec::loose("g"),
            Spec::vpk("g/g_pak_dir.vpk"),
            Spec::vpk("shared/missing_dir.vpk").optional(),
            Spec::loose("platform").optional(),
        ];
        let path = SearchPath::from_specs(&s.0, SPECS);
        assert_eq!(path.layers[1].path, s.0.join("g"));
        let mount = path.mount().unwrap();
        // The missing custom folder is still a layer; the optional ones go.
        assert_eq!(mount.layers().count(), 3);
        assert_eq!(mount.read("a.txt").unwrap(), b"loose");
        // A required archive that's missing is an error.
        let missing = SearchPath::from_specs(&s.0, &[Spec::vpk("nope_dir.vpk")]);
        assert!(missing.mount().is_err());
    }
}
