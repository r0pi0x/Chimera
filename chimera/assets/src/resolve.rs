//! What a player needs, and when (docs/performance.md, "First join
//! download, 2026-11-14"): the sets a published manifest names, sorted by
//! when a client fetches them, and the records a list of sets comes to.
//!
//! - **Startup** (`sets::STARTUP_SETS`): a game's `base`, fetched behind
//!   the client's window: what the menus read and what every match reads
//!   (the doc's classes (a) and (b)), nothing more.
//! - **Each match, before it starts** (the join's or level change's
//!   download, during the loading screen and the pre-match lobby): the
//!   match's own sets, which the server's offer names
//!   (`chimera_net::net::assets::offer`): per game in play `players` and
//!   `common` (`MATCH_SETS`: what a match of that game may play, the event
//!   sounds and images not every match reads), the map's `map/<name>`, the
//!   ladder's `weapon/<id>` and `parts/<id>`, its bodies'
//!   `character/<record>`; and the client's own (a game's music:
//!   `music/<track>`, its first tracks). Each piece's set holds what loading
//!   that piece reads (`chimera_pack sets` loads each through the game's own
//!   loaders: a gun's view and held models with their materials, its own
//!   sounds and what it fires; a body's model, head, faces and hands; a
//!   map's world, materials, models and the sounds its objects name), so
//!   a ladder's gun pulls its sounds with it.
//! - **In the background, ahead of need**: the next map once the vote has
//!   locked it in, and the next music track against the current one's
//!   time left.
//!
//! `lean` makes the startup set lean from what client runs read
//! (`CHIMERA_VFS_LOG` logs, one per run): `base` keeps the files every run
//! read; the music goes to a set per track; the files a ladder's sets
//! hold that no run read every time leave it (they come with those sets,
//! among them the files the runs only checked for: `contains`, logged
//! apart); the rest becomes `common` (a file only checked for that no
//! ladder set holds too: another client, a networked one, may read it).

use std::collections::{BTreeMap, BTreeSet};

use crate::Manifest;

/// The set a client fetches at startup (`sets::STARTUP_SETS`).
pub const BASE: &str = "base";
/// What a match of a game may play beyond `base` (event sounds, images).
pub const COMMON: &str = "common";
/// The sets the offer names per game in play, each match.
pub const MATCH_SETS: &[&str] = &["players", COMMON];
/// The sets of a ladder's pieces: their files come with the pieces the
/// offer names, never in `base` or `common` unless every run read them.
pub const LADDER_PREFIXES: &[&str] = &["weapon/", "parts/", "character/"];
/// A game's loose music, whole (`chimera_pack`'s extra set).
pub const MUSIC: &str = "music";
/// A music track's own set: `music/` and its path less `loose/`
/// (`music/bgm/modebgm/weaponrace_start.mp3`).
pub const TRACK_PREFIX: &str = "music/";

/// The set of the music file at `path` (`loose/bgm/bgm_05.mp3` ->
/// `music/bgm/bgm_05.mp3`).
pub fn track_set(path: &str) -> String {
    format!("{TRACK_PREFIX}{}", path.strip_prefix("loose/").unwrap_or(path))
}

/// The set of a music file by the game's own key under its install's
/// `Game/` (`bgm/bgm_05.mp3`, any case) -> `music/bgm/bgm_05.mp3`.
pub fn track_set_of_key(key: &str) -> String {
    format!("{TRACK_PREFIX}{}", key.replace('\\', "/").to_ascii_lowercase())
}

/// What one client run read of a game, and what it only checked for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Run {
    pub reads: BTreeSet<String>,
    pub checks: BTreeSet<String>,
}

impl Run {
    /// `game`'s lines of a `CHIMERA_VFS_LOG` log
    /// (`recording::read_log_with_checks`).
    pub fn of(game: &str, reads: &[crate::LogLine], checks: &[(String, String)]) -> Self {
        Self {
            reads: reads.iter().filter(|l| l.game == game).map(|l| l.path.clone()).collect(),
            checks: checks.iter().filter(|(g, _)| g == game).map(|(_, p)| p.clone()).collect(),
        }
    }
}

/// `lean`'s answer: the sets to enter, and what left `base` and why.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lean {
    pub base: BTreeSet<String>,
    pub common: BTreeSet<String>,
    /// `music/<track>` -> its file.
    pub tracks: BTreeMap<String, BTreeSet<String>>,
    /// Left `base`: music (in `tracks`).
    pub music: BTreeSet<String>,
    /// Left `base`: in a ladder's sets and not read by every run.
    pub ladder: BTreeSet<String>,
    /// Of `ladder`: only checked for by the runs, never read (a body's
    /// model `install_has` asks about).
    pub checked: BTreeSet<String>,
}

/// Whether `set` is a ladder piece's set.
pub fn is_ladder_set(set: &str) -> bool {
    LADDER_PREFIXES.iter().any(|p| set.starts_with(p))
}

/// `manifest`'s `base` made lean from `runs` (each a client run's reads
/// and checks of this game; at least one): see the module doc. The music
/// is the `music` set's files (else the files under `loose/bgm/`).
pub fn lean(manifest: &Manifest, runs: &[Run]) -> Lean {
    let empty = BTreeSet::new();
    let base = manifest.sets.get(BASE).unwrap_or(&empty);
    let music: BTreeSet<String> = match manifest.sets.get(MUSIC) {
        Some(m) => m.clone(),
        None => manifest.files.keys().filter(|p| p.starts_with("loose/bgm/")).cloned().collect(),
    };
    let ladder: BTreeSet<&String> = manifest
        .sets
        .iter()
        .filter(|(name, _)| is_ladder_set(name))
        .flat_map(|(_, paths)| paths)
        .collect();
    let every = |p: &String| !runs.is_empty() && runs.iter().all(|r| r.reads.contains(p));
    let any_read = |p: &String| runs.iter().any(|r| r.reads.contains(p));
    let any_check = |p: &String| runs.iter().any(|r| r.checks.contains(p));
    let mut out = Lean::default();
    for p in base {
        if music.contains(p) {
            out.music.insert(p.clone());
        } else if every(p) {
            out.base.insert(p.clone());
        } else if ladder.contains(p) {
            if any_check(p) && !any_read(p) {
                out.checked.insert(p.clone());
            }
            out.ladder.insert(p.clone());
        } else {
            out.common.insert(p.clone());
        }
    }
    for p in music.iter().filter(|p| manifest.files.contains_key(*p)) {
        out.tracks.entry(track_set(p)).or_default().insert(p.clone());
    }
    out
}

impl Lean {
    /// Enter the sets into `manifest`: `base` and `common` replaced, a
    /// set per track; `music` (whole) kept.
    pub fn apply(&self, manifest: &mut Manifest) {
        manifest.sets.insert(BASE.into(), self.base.clone());
        if self.common.is_empty() {
            manifest.sets.remove(COMMON);
        } else {
            manifest.sets.insert(COMMON.into(), self.common.clone());
        }
        for (name, paths) in &self.tracks {
            manifest.sets.insert(name.clone(), paths.clone());
        }
    }
}

/// The distinct records `sets` of `manifest` come to, with their plain and
/// stored sizes (a set the manifest doesn't name adds nothing).
pub fn records<'a>(manifest: &'a Manifest, sets: &[&str]) -> BTreeMap<&'a str, (u64, u64)> {
    sets.iter()
        .filter_map(|s| manifest.sets.get(*s))
        .flatten()
        .filter_map(|p| manifest.files.get(p))
        .map(|f| (f.id.as_str(), (f.size, f.stored)))
        .collect()
}

/// Stored bytes of `sets` beyond the records in `have` (what fetching
/// them after `have` costs).
pub fn stored_beyond(manifest: &Manifest, sets: &[&str], have: &BTreeSet<String>) -> u64 {
    records(manifest, sets).iter().filter(|(id, _)| !have.contains(**id)).map(|(_, (_, s))| s).sum()
}

/// A match's sets of one game, as the offer names them: `base`, then
/// `MATCH_SETS`, then `extra` (the map's, the ladder's, the client's),
/// each once.
pub fn match_sets(extra: &[String]) -> Vec<String> {
    let mut out: Vec<String> = std::iter::once(BASE).chain(MATCH_SETS.iter().copied()).map(String::from).collect();
    for s in extra {
        if !out.contains(s) {
            out.push(s.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FileRecord;

    fn manifest(files: &[(&str, &[&str])]) -> Manifest {
        let mut m = Manifest::new("g", "1", None);
        for (i, (path, sets)) in files.iter().enumerate() {
            let id = format!("{:064x}", i + 1);
            m.files.insert(path.to_string(), FileRecord { id, size: 100, stored: 10 * (i as u64 + 1) });
            for s in *sets {
                m.sets.entry(s.to_string()).or_default().insert(path.to_string());
            }
        }
        m
    }

    fn run(reads: &[&str], checks: &[&str]) -> Run {
        Run {
            reads: reads.iter().map(|s| s.to_string()).collect(),
            checks: checks.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// A gun's set pulls its sounds: a ladder with the gun fetches its
    /// model and sounds before the match, though `base` no longer holds
    /// them; another ladder's gun stays out of every match's download.
    #[test]
    fn a_gun_pulls_its_sounds_and_other_guns_stay_out() {
        let m = manifest(&[
            ("ui/font.ttf", &["base"]),
            ("models/v_ak47.mdl", &["base", "weapon/ak47"]),
            ("sound/weapons/ak47/fire.wav", &["base", "weapon/ak47"]),
            ("sound/weapons/ak47/reload.wav", &["weapon/ak47"]),
            ("models/v_awp.mdl", &["base", "weapon/awp"]),
            ("sound/weapons/awp/fire.wav", &["base", "weapon/awp"]),
        ]);
        let runs = [run(&["ui/font.ttf", "models/v_ak47.mdl"], &[]), run(&["ui/font.ttf"], &[])];
        let lean = lean(&m, &runs);
        assert_eq!(lean.base, BTreeSet::from(["ui/font.ttf".to_string()]));
        assert!(lean.common.is_empty(), "{:?}", lean.common);
        let mut m = m.clone();
        lean.apply(&mut m);
        let sets = match_sets(&["weapon/ak47".into()]);
        let sets: Vec<&str> = sets.iter().map(String::as_str).collect();
        let ids: BTreeSet<&str> = records(&m, &sets).keys().copied().collect();
        for path in ["models/v_ak47.mdl", "sound/weapons/ak47/fire.wav", "sound/weapons/ak47/reload.wav"] {
            assert!(ids.contains(m.files[path].id.as_str()), "{path} comes with the ladder's gun");
        }
        for path in ["models/v_awp.mdl", "sound/weapons/awp/fire.wav"] {
            assert!(!ids.contains(m.files[path].id.as_str()), "{path}: another ladder's gun");
        }
    }

    /// A file only checked for (a body model `install_has` asks about)
    /// leaves `base` with its body's set; one no ladder set holds goes to
    /// `common` (a networked client may read it); an event sound no run
    /// played goes to `common`, fetched with each match, not at startup;
    /// the music leaves `base` for a set per track.
    #[test]
    fn check_only_files_leave_base_and_rare_sounds_go_to_common() {
        let m = manifest(&[
            ("ui/font.ttf", &["base"]),
            ("chars/body_07.ltb", &["base", "character/7"]),
            ("materials/hud/freeze.vmt", &["base"]),
            ("chars/body_08.ltb", &["base", "character/8"]),
            ("snd/impacts/glass.wav", &["base"]),
            ("snd/explosion.wav", &["base"]),
            ("loose/bgm/bgm_05.mp3", &["base", "music"]),
            ("loose/bgm/modebgm/weaponrace_start.mp3", &["music"]),
        ]);
        let runs = [
            run(&["ui/font.ttf", "snd/explosion.wav"], &["chars/body_07.ltb", "chars/body_08.ltb"]),
            run(&["ui/font.ttf"], &["chars/body_07.ltb", "materials/hud/freeze.vmt"]),
        ];
        let lean = lean(&m, &runs);
        assert_eq!(lean.base, BTreeSet::from(["ui/font.ttf".to_string()]));
        assert_eq!(lean.checked, BTreeSet::from(["chars/body_07.ltb".to_string(), "chars/body_08.ltb".to_string()]));
        assert_eq!(lean.ladder, BTreeSet::from(["chars/body_07.ltb".to_string(), "chars/body_08.ltb".to_string()]));
        assert_eq!(
            lean.common,
            BTreeSet::from([
                "materials/hud/freeze.vmt".to_string(),
                "snd/explosion.wav".to_string(),
                "snd/impacts/glass.wav".to_string()
            ])
        );
        assert_eq!(lean.music, BTreeSet::from(["loose/bgm/bgm_05.mp3".to_string()]));
        assert_eq!(
            lean.tracks.keys().cloned().collect::<Vec<_>>(),
            ["music/bgm/bgm_05.mp3", "music/bgm/modebgm/weaponrace_start.mp3"]
        );
        assert_eq!(track_set_of_key("BGM/modebgm/weaponrace_start.mp3"), "music/bgm/modebgm/weaponrace_start.mp3");
        let mut m = m;
        lean.apply(&mut m);
        let text = m.to_text();
        assert_eq!(Manifest::parse(&text).unwrap(), m, "the manifest's format is unchanged");
        // The startup is base alone; common costs nothing at startup.
        let have = BTreeSet::new();
        assert_eq!(stored_beyond(&m, &["base"], &have), 10);
    }
}
