//! specs/combat_arms/music_ambience.md's install cases (19-22) on the
//! user's Steam install: `BGM.rez` read raw (its payloads are not the
//! plain files), the loose `Game/BGM/` folder (counts, sizes, decoded
//! lengths and rates, and our built-in table's lengths against them),
//! one map's ambience (`training`) and the census of the 33 plain maps'
//! sound objects and music triggers; case 29's interface sounds (the
//! party's cue pack, result and director sounds, inferred by name, are
//! WAVs in the install). They pass with a note without
//! `[games.combat_arms] path` in chimera.local.toml.

use std::path::{Path, PathBuf};

use chimera_core::mount::{Mount, config::LocalConfig};
use combat_arms::game::{
    GAME, INFO,
    music::{ambient::SoundFx, data::MusicData, data::normalize},
    objects::runtime::Props,
    world,
};
use lithtech_formats::{
    rez::{RezArchive, RezKeys},
    world::World,
};

fn install() -> Option<PathBuf> {
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else {
        eprintln!("skipping: no Combat Arms install configured");
        return None;
    };
    if !chimera_core::mount::served::usable("combat_arms", &install, "Game") {
        eprintln!("skipping: no Combat Arms install at {}", install.display());
        return None;
    }
    Some(install)
}

fn mount() -> Option<Mount> {
    Some((INFO.open)(&install()?).unwrap())
}

/// Every file below `dir`, by its path from `prefix` (lower case).
fn files_below(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let key = normalize(&format!("{prefix}/{}", e.file_name().to_string_lossy()));
        if e.path().is_dir() {
            files_below(&e.path(), &key, out);
        } else {
            out.push((key, e.path()));
        }
    }
}

/// A loose music file: on disk in the install, or in the game's mount
/// under `mount::LOOSE_MUSIC` when the asset store stands in for it.
enum Loose {
    Disk(PathBuf),
    Mounted(String),
}

impl Loose {
    fn bytes(&self) -> Vec<u8> {
        match self {
            Loose::Disk(p) => std::fs::read(p).unwrap(),
            Loose::Mounted(p) => combat_arms::game::world::mount().unwrap().read(p).unwrap(),
        }
    }
}

fn loose() -> Option<Vec<(String, Loose)>> {
    let install = install()?;
    let dir = install.join("Game").join("BGM");
    if !dir.is_dir() {
        let mount = combat_arms::game::world::mount()?;
        let prefix = format!("{}/", combat_arms::game::mount::LOOSE_MUSIC);
        let mut out: Vec<(String, Loose)> = mount
            .entries()
            .into_iter()
            .filter_map(|(e, _)| Some((format!("bgm/{}", e.path.strip_prefix(&prefix)?), Loose::Mounted(e.path.clone()))))
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        return Some(out);
    }
    let mut out = Vec::new();
    files_below(&dir, "bgm", &mut out);
    Some(out.into_iter().map(|(k, p)| (k, Loose::Disk(p))).collect())
}

#[test]
fn t19_bgm_rez() {
    let Some(install) = install() else { return };
    if !install.join("Game").is_dir() {
        eprintln!("skipping: checks the install's own BGM.rez (served from the asset store here)");
        return;
    }
    let rez = RezArchive::open(install.join("Game").join("BGM.rez"), &RezKeys::new()).unwrap();
    let files: Vec<_> = rez.files().collect();
    assert_eq!(files.len(), 65, "entries");
    assert!(files.iter().all(|(p, _)| p.starts_with("bgm/")), "all under BGM\\");
    let size = |p: &str| rez.files().find(|(k, _)| k.as_str() == p).map(|(_, f)| f.size);
    assert_eq!(size("bgm/bgm_loop.mp3"), Some(3_439_514));
    assert_eq!(size("bgm/map_loading.mp3"), Some(65_518));
    assert_eq!(size("bgm/modebgm/search&destroy.mp3"), Some(608_029));
    assert_eq!(size("bgm/bgm_halloween.mp3"), Some(2_707_821));
    // The payloads are not the plain files; the last size mod 16 bytes are.
    let loose = loose().unwrap();
    let (key, file) = loose.iter().find(|(k, _)| k == "bgm/bgm_loop.mp3").unwrap();
    let plain = file.bytes();
    let stored = rez.read_raw(key).unwrap().unwrap();
    assert_eq!(stored.len(), plain.len());
    assert_ne!(stored[..16], plain[..16], "the first block differs");
    let tail = plain.len() % 16;
    assert_eq!(
        stored[stored.len() - tail..],
        plain[plain.len() - tail..],
        "the trailing bytes agree"
    );
    // The one entry without a loose copy.
    assert!(!loose.iter().any(|(k, _)| k == "bgm/bgm_halloween.mp3"));
}

#[test]
fn t20_loose_bgm_folder() {
    let Some(files) = loose() else { return };
    assert_eq!(files.len(), 64, "files in Game/BGM");
    let find = |k: &str| {
        files
            .iter()
            .find(|(p, _)| p == k)
            .unwrap_or_else(|| panic!("no {k}"))
            .1
            .bytes()
    };
    let decode = |k: &str| {
        let bytes = find(k);
        (bytes.len(), source_formats::wav::decode_mp3(&bytes).unwrap())
    };
    let secs = |c: &source_formats::wav::SoundClip| c.samples.len() as f64 / c.channels as f64 / c.rate as f64;
    // (file, bytes, seconds, kbit/s or 0, rate, channels)
    for (k, bytes, len, kbps, rate, ch) in [
        ("bgm/bgm_loop.mp3", 3_439_514, 143.31, 192.0, 44_100, 2),
        ("bgm/waitingroom_loop.mp3", 847_270, 52.80, 128.0, 44_100, 2),
        ("bgm/map_loading.mp3", 65_518, 2.02, 0.0, 44_100, 2),
        // The spec says 602.28 s; the decoder gives 600.12 s (the only
        // file that differs; its figure is probably size over bitrate).
        ("bgm/null_60s.mp3", 1_501_457, 600.12, 0.0, 16_000, 1),
        ("bgm/hqm_count_bgm.mp3", 0, 20.83, 0.0, 44_100, 2),
        ("bgm/qrt_count_bgm.mp3", 0, 27.12, 0.0, 44_100, 2),
        ("bgm/modebgm/weaponrace_last.mp3", 0, 36.03, 0.0, 44_100, 2),
    ] {
        let (size, clip) = decode(k);
        if bytes > 0 {
            assert_eq!(size, bytes, "{k}: bytes");
        }
        assert!(
            (secs(&clip) - len).abs() <= 0.06,
            "{k}: {} s, expected {len}",
            secs(&clip)
        );
        assert_eq!((clip.rate, clip.channels as u32), (rate, ch), "{k}: rate and channels");
        if kbps > 0.0 {
            let got = size as f64 * 8.0 / secs(&clip) / 1000.0;
            assert!((got - kbps).abs() < 2.0, "{k}: {got} kbit/s");
        }
    }
}

/// Our built-in table's files exist on disk and its lengths agree with
/// the decoded ones.
#[test]
fn the_built_in_table_matches_the_folder() {
    let Some(files) = loose() else { return };
    let table = MusicData::built_in();
    let mut missing = Vec::new();
    for (key, len) in &table.files {
        let Some((_, file)) = files.iter().find(|(k, _)| k == key) else {
            missing.push(key.clone());
            continue;
        };
        let Ok(clip) = source_formats::wav::decode_any(&file.bytes()) else {
            continue;
        };
        let got = clip.samples.len() as f64 / clip.channels as f64 / clip.rate as f64;
        assert!((got - len).abs() <= 0.06, "{key}: table {len} s, file {got} s");
    }
    assert!(missing.is_empty(), "table files not in the install: {missing:?}");
    // The playlist's inferred entries that exist are listed; the rest are
    // simply not playable.
    let present = table
        .playlist
        .iter()
        .filter(|e| files.iter().any(|(k, _)| *k == e.file))
        .count();
    eprintln!("{present} of {} playlist entries are on disk", table.playlist.len());
    assert!(present >= 3);
    // Every cue and mode track the table names is on disk.
    let cues = &table.cues;
    for f in [
        &cues.loading,
        &cues.quarantine_mode,
        &cues.quarantine_end,
        &cues.capture_progress,
        &cues.last_stage,
    ] {
        assert!(files.iter().any(|(k, _)| k == f), "cue {f}");
    }
    for (m, t) in &table.modes {
        assert!(files.iter().any(|(k, _)| *k == t.start), "mode {m}: {}", t.start);
    }
}

fn props(o: &lithtech_formats::world::Object) -> Props {
    Props(
        o.properties
            .iter()
            .map(|p| {
                let v = match &p.value {
                    lithtech_formats::world::Value::String(s) => s.clone(),
                    lithtech_formats::world::Value::Real(r) => r.to_string(),
                    lithtech_formats::world::Value::Bool(b) => (*b as u8).to_string(),
                    lithtech_formats::world::Value::Long(i) | lithtech_formats::world::Value::Flags(i) => i.to_string(),
                    other => format!("{other:?}"),
                };
                (p.name.clone(), v)
            })
            .collect(),
    )
}

fn plain_maps(mount: &Mount) -> Vec<(String, World)> {
    world::map_names(mount)
        .into_iter()
        .filter_map(|n| {
            let path = world::find(mount, &n)?;
            (path.starts_with("worlds2/") || path.starts_with("worlds3/")).then(|| (n, path))
        })
        .map(|(n, path)| (n, World::parse(&mount.read(&path).unwrap()).unwrap()))
        .collect()
}

#[test]
fn t21_one_maps_ambience() {
    let Some(mount) = mount() else { return };
    let path = world::find(&mount, "training").expect("training");
    assert!(path.starts_with("worlds2/"), "{path}");
    let w = World::parse(&mount.read(&path).unwrap()).unwrap();
    assert_eq!(w.objects.len(), 201, "objects");
    let fx: Vec<_> = w.objects.iter().filter(|o| o.class == "SoundFX").collect();
    assert_eq!(fx.len(), 15, "SoundFX");
    assert_eq!(
        fx.iter()
            .filter(|o| o.name().is_some_and(|n| n.starts_with("Birds")))
            .count(),
        14
    );
    let birds = fx
        .iter()
        .find(|o| o.name() == Some("Birds_Ambient"))
        .expect("Birds_Ambient");
    let p = Vec3::from(birds.pos().unwrap());
    assert!(
        (p.x + 5065.0).abs() < 1.0 && (p.y - 165.5).abs() < 1.0 && (p.z + 5453.0).abs() < 1.0,
        "{p}"
    );
    let f = SoundFx::from_props(0, "Birds_Ambient", &props(birds), p, true, 0);
    assert!(f.start_on);
    assert!(
        f.file.eq_ignore_ascii_case("SND\\Training\\Birds_Ambient.wav"),
        "{}",
        f.file
    );
    assert_eq!((f.outer, f.inner, f.volume), (6000.0, 4000.0, 100.0));
    assert!(f.looping && f.ambient && !f.attached);
    assert_eq!(f.filter, "UnFiltered");
    assert_eq!(f.priority, 0);
    // The sound is in SND.rez, 7.995 s, 44.1 kHz 16-bit stereo.
    let clip = source_formats::wav::decode(&mount.read(&f.file.to_lowercase().replace('\\', "/")).unwrap()).unwrap();
    assert_eq!((clip.rate, clip.channels), (44_100, 2));
    let len = clip.samples.len() as f64 / 2.0 / 44_100.0;
    assert!((len - 7.995).abs() < 0.01, "{len}");
    let end = fx.iter().find(|o| o.name() == Some("End_BGM")).expect("End_BGM");
    let e = SoundFx::from_props(1, "End_BGM", &props(end), Vec3::from(end.pos().unwrap()), true, 0);
    assert!(!e.start_on && !e.looping && e.ambient);
    assert!(
        e.file.eq_ignore_ascii_case("SND\\Training\\Training_BGM.wav"),
        "{}",
        e.file
    );
    assert_eq!((e.outer, e.inner), (8000.0, 6000.0));
    assert!((Vec3::from(end.pos().unwrap()) - p).length() < 1.0, "same position");
}

#[test]
fn t22_census() {
    let Some(mount) = mount() else { return };
    let maps = plain_maps(&mount);
    assert_eq!(maps.len(), 33, "plain world files");
    let (mut sound_fx, mut triggers, mut looping, mut ambient, mut start_on, mut attached) = (0, 0, 0, 0, 0, 0);
    let mut dark = None;
    for (name, w) in &maps {
        for o in &w.objects {
            match o.class.as_str() {
                "SoundFX" => {
                    sound_fx += 1;
                    let f = SoundFx::from_props(0, "", &props(o), Vec3::ZERO, true, 0);
                    looping += f.looping as u32;
                    ambient += f.ambient as u32;
                    start_on += f.start_on as u32;
                    attached += f.attached as u32;
                }
                "BGMTrigger" => {
                    triggers += 1;
                    if o.string("FileName")
                        .is_some_and(|f| f.to_lowercase().contains("hunted_darkhill"))
                    {
                        dark = Some((
                            name.clone(),
                            o.string("GameType").map(str::to_string),
                            o.real("FadingTime"),
                            o.bool("Loop"),
                        ));
                    }
                }
                _ => {}
            }
        }
    }
    assert_eq!(sound_fx, 786, "SoundFX");
    assert_eq!(triggers, 17, "BGMTrigger");
    assert_eq!(
        (looping, ambient, start_on, attached),
        (642, 730, 585, 41),
        "looping, ambient, start on, attached"
    );
    let (_, game_type, fade, looping) = dark.expect("the Dark Hills trigger");
    assert_eq!(game_type.as_deref(), Some("HungerGame"));
    assert_eq!(fade, Some(1.0));
    assert_eq!(looping, Some(false));
}

use bevy::prelude::Vec3;

/// 29. The interface sounds a party match plays (the cue pack's,
/// data/cues.toml; the result screen's, `GameParts::match_won_sound` and
/// `match_lost_sound`; the director's, data/music.toml `[party.interface]`)
/// are in the install as 16-bit WAVs; `WIN.WAV` is stereo, 3.108 s.
#[test]
fn t29_the_partys_interface_sounds_are_in_the_install() {
    let Some(mount) = mount() else { return };
    let parts = &combat_arms::game::parts::PARTS;
    let pack = chimera_core::cues::CuePack::parse(parts.cues.unwrap()).unwrap();
    assert_eq!(pack.game, GAME);
    let mut files: Vec<String> = pack.sounds.values().cloned().collect();
    files.extend(parts.match_won_sound.into_iter().chain(parts.match_lost_sound).map(str::to_string));
    files.extend(MusicData::built_in().party.interface.values().cloned());
    for f in &files {
        let bytes = mount.read(&f.to_lowercase()).unwrap_or_else(|e| panic!("{f}: {e}"));
        let clip = source_formats::wav::decode(&bytes).unwrap_or_else(|e| panic!("{f}: {e:?}"));
        if f.eq_ignore_ascii_case("UI_SND/WIN.WAV") {
            assert_eq!(clip.channels, 2, "{f}");
            let secs = clip.samples.len() as f64 / clip.channels as f64 / clip.rate as f64;
            assert!((secs - 3.108).abs() < 0.01, "{f}: {secs} s");
        }
    }
}
