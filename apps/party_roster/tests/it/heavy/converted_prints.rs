//! The converter versions' guard (docs/plans/active/converted-cache.md,
//! "Converter versions"): a game's `CONVERTER` is a number bumped by hand
//! and the hash of the sources that shape its output (games/<game>/
//! build.rs, `CONVERTING`), not its gameplay, so a cached or served
//! conversion survives gameplay edits. What that hash leaves out (a gun's
//! model path in its data, a gameplay module a dressing calls) must bump
//! the number instead, or caches would hand out stale conversions. This
//! test converts each game's fixtures (its guest dressing as kept, a small
//! map) from the install here, never from a cache, and compares the
//! output's hash with tests/converted-prints.txt, a list of hashes (never
//! any game's files): one line per game, fixture, system and the install's
//! files read (a hash of their fingerprints).
//!
//! - Same files, other output, the same converter version (number and
//!   hash): fails: bump `VERSION` in games/<game>/build.rs (or add what
//!   changed to its `CONVERTING`), then record the new prints.
//! - Another output under a new version (an edit inside `CONVERTING`, or
//!   the number bumped): fails until the prints are recorded again, so the
//!   next comparison is against the current version.
//! - No print for these files (another install build, another system):
//!   says so and passes.
//!
//! Record: `CHIMERA_UPDATE_CONVERTED_PRINTS=1 cargo test -p party_roster
//! --features dev --test it heavy::converted_prints -- --nocapture`.
//! Each game passes with a note without its install.

use std::{path::PathBuf, sync::Arc};

use chimera_core::mount::{Mount, assets::map_cache::{Key, fingerprint}};
use chimera_world::map::{MapData, MapLoadOptions, kept};

const UPDATE_ENV: &str = "CHIMERA_UPDATE_CONVERTED_PRINTS";

fn prints_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/converted-prints.txt")
}

/// One recorded print.
#[derive(Clone, Debug, PartialEq)]
struct Print {
    game: String,
    fixture: String,
    os: String,
    converter: String,
    inputs: String,
    output: String,
}

impl Print {
    fn line(&self) -> String {
        format!("{} {} {} {} {} {}", self.game, self.fixture, self.os, self.converter, self.inputs, self.output)
    }
}

fn read_prints() -> Vec<Print> {
    std::fs::read_to_string(prints_path())
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            if f.len() != 6 {
                return None;
            }
            Some(Print {
                game: f[0].into(),
                fixture: f[1].into(),
                os: f[2].into(),
                converter: f[3].into(),
                inputs: f[4].into(),
                output: f[5].into(),
            })
        })
        .collect()
}

fn write_prints(prints: &[Print]) {
    let mut lines: Vec<String> = prints.iter().map(Print::line).collect();
    lines.sort();
    let text = format!(
        "# Converted output's hashes per game, fixture, system, converter version and the\n\
         # install files read (heavy::converted_prints; never any game's files).\n\
         # game fixture os converter inputs output\n{}\n",
        lines.join("\n")
    );
    std::fs::write(prints_path(), text).unwrap();
}

/// `convert` on `mount`, tracked: the output's hash and the hash of every
/// file it read with its fingerprint.
fn print(mount: &Mount, convert: impl FnOnce() -> Vec<u8>) -> (String, String) {
    let tracking = mount.track();
    let bytes = convert();
    let accesses = tracking.finish();
    let mut inputs = Key::new("inputs", "");
    for a in &accesses {
        inputs = inputs.part(&format!("{a:?}"), fingerprint(mount, a));
    }
    let short = |h: String| h[..32].to_string();
    (short(inputs.hex()), short(Key::new("output", "").content("bytes", &bytes).hex()))
}

type Fixture = (&'static str, &'static str, &'static str, Box<dyn Fn() -> (String, String)>);

/// Each game's fixtures whose install is here: (game, fixture, converter,
/// its print).
fn fixtures() -> Vec<Fixture> {
    use source_engine::content::bsp::{SourceGame, load_level_bytes_in};
    let mut out: Vec<Fixture> = Vec::new();
    type Guest = fn(&Mount, &mut MapData);
    let source: [(&str, &str, fn() -> Result<Arc<Mount>, String>, Guest, &'static SourceGame, &[&str]); 3] = [
        (
            cs_source::game::GAME,
            cs_source::game::bsp::CONVERTER,
            cs_source::game::guest_mount,
            |m, map| cs_source::game::bsp::dress_guest(m, map, cs_source::game::BUNDLE.kept, false),
            &cs_source::game::bsp::CS_SOURCE,
            &["test_speakers", "de_dust2"],
        ),
        (
            hl2dm::game::GAME,
            hl2dm::game::maps::CONVERTER,
            hl2dm::game::maps::guest_mount,
            |m, map| hl2dm::game::maps::dress_guest_in(m, map, hl2dm::game::BUNDLE.kept, false),
            &hl2dm::game::maps::HL2DM,
            &["halls3", "dm_overwatch", "dm_lockdown"],
        ),
        (
            tf2::game::GAME,
            tf2::game::maps::CONVERTER,
            tf2::game::maps::guest_mount,
            |m, map| tf2::game::maps::dress_guest_in(m, map, tf2::game::BUNDLE.kept, false),
            &tf2::game::maps::TF2,
            &["itemtest", "cp_cloak", "ctf_2fort"],
        ),
    ];
    let installed = |game: &str| match game {
        "cs_source" => cs_source::game::INFO.install().is_some(),
        "hl2dm" => hl2dm::game::INFO.install().is_some(),
        _ => tf2::game::INFO.install().is_some_and(|p| chimera_core::mount::served::usable("tf2", p, "tf")),
    };
    for (game, converter, mount_of, guest, source_game, maps) in source {
        if !installed(game) {
            eprintln!("{game}: no install here, its fixtures skipped");
            continue;
        }
        out.push((
            game,
            "guest",
            converter,
            Box::new(move || {
                let mount = mount_of().unwrap();
                let mount_fn = || Ok(mount.clone());
                let dressing = kept::Dressing { game, converter, mount: &mount_fn, convert: &guest, variant: "", memos: None };
                print(&mount, || {
                    let parts = kept::kept_with(&MapLoadOptions::default(), &dressing, None).unwrap();
                    postcard::to_allocvec(&*parts).unwrap()
                })
            }),
        ));
        // The first of its small maps this mount has (an install, or a
        // store that holds it).
        let Some(map) = mount_of()
            .ok()
            .and_then(|m| maps.iter().copied().find(|n| m.read(&format!("maps/{n}.bsp")).is_ok()))
        else {
            eprintln!("{game}: none of its fixture maps here ({maps:?}), its map fixture skipped");
            continue;
        };
        out.push((
            game,
            map,
            converter,
            Box::new(move || {
                let mount = mount_of().unwrap();
                print(&mount, || {
                    let bytes = mount.read(&format!("maps/{map}.bsp")).unwrap();
                    let (m, _) = load_level_bytes_in(source_game, &mount, map, bytes, &MapLoadOptions::default(), None).unwrap();
                    postcard::to_allocvec(&m).unwrap()
                })
            }),
        ));
    }
    let ca = chimera_core::mount::config::LocalConfig::load()
        .ok()
        .and_then(|c| c.game_path(combat_arms::game::GAME))
        .is_some_and(|p| chimera_core::mount::served::usable("combat_arms", p, "Game"));
    if ca {
        out.push((
            combat_arms::game::GAME,
            "guest",
            combat_arms::game::world::CONVERTER,
            Box::new(|| {
                let options = MapLoadOptions::default();
                let data = combat_arms::game::data::CaData::install_for(&options.context);
                let mount = data.mount().expect("Combat Arms' install mounts");
                print(&mount, || {
                    let mut map = MapData::default();
                    combat_arms::game::world::dress_kept(&mut map, &options, None).unwrap();
                    postcard::to_allocvec(&map).unwrap()
                })
            }),
        ));
    } else {
        eprintln!("combat_arms: no install here, its fixture skipped");
    }
    out
}

#[test]
fn converted_output_matches_its_version() {
    let update = std::env::var(UPDATE_ENV).is_ok_and(|v| v == "1");
    let os = std::env::consts::OS;
    let mut prints = read_prints();
    let mut failures = Vec::new();
    for (game, fixture, converter, make) in fixtures() {
        let (inputs, output) = make();
        let now = Print { game: game.into(), fixture: fixture.into(), os: os.into(), converter: converter.into(), inputs, output };
        let at = prints
            .iter()
            .position(|p| p.game == now.game && p.fixture == now.fixture && p.os == now.os && p.inputs == now.inputs);
        let Some(at) = at else {
            if update {
                eprintln!("{game} {fixture}: recorded");
                prints.push(now);
            } else {
                eprintln!("{game} {fixture}: no print for this install's files on {os}: record them with {UPDATE_ENV}=1");
            }
            continue;
        };
        let was = prints[at].clone();
        if was.output == now.output {
            if was.converter != now.converter && update {
                prints[at] = now;
            }
            eprintln!("{game} {fixture}: the same output");
        } else if was.converter == now.converter {
            // The same version, another output: a cache would hand out
            // the old one.
            failures.push(format!(
                "{game} {fixture}: the converted output changed but the converter's version didn't ({converter}): \
                 a change outside games/{game}/build.rs's CONVERTING changed what the conversion gives. Bump \
                 VERSION there (or add the changed file to CONVERTING), then record the prints again with \
                 {UPDATE_ENV}=1. Without it, caches and the asset service's served entries keep the old output."
            ));
        } else if update {
            eprintln!("{game} {fixture}: recorded for converter {converter}");
            prints[at] = now;
        } else {
            failures.push(format!(
                "{game} {fixture}: a new converter version ({converter}) and a new output: record the prints \
                 with {UPDATE_ENV}=1 (else the next change outside CONVERTING goes unnoticed)"
            ));
        }
    }
    if update {
        write_prints(&prints);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

