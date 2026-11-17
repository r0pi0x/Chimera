//! Prototype measurement for a converted-asset cache on disk
//! (docs/plans/active/converted-cache.md): each game's guest parts (what
//! its dressing of another game's map converts: the Source guests'
//! `dress_guest_in`, Combat Arms' `world::dress_with`) converted from the
//! install's originals, then written to and read back from the
//! converted-map cache's on-disk format (`map::cache`: a versioned
//! header, the converter's version and the load options in the key, every
//! file read fingerprinted), each in a process of its own.
//!
//! Runs only with `CHIMERA_CONVCACHE_DIR` set (the cache folder: never the
//! repo); `CHIMERA_CONVCACHE_GAMES` (comma-separated: cs_source, hl2dm,
//! tf2, combat_arms) narrows it. Each test prints one line a game:
//! wall time, instructions of this thread and the threads it starts
//! (`InstructionCounter::current_with_children`, opened before any), and
//! for `stages` where a cold dressing's instructions go.
//!
//! - `convert`: from the originals, no cache.
//! - `write`: from the originals into the cache (a miss).
//! - `read`: from the cache (a hit expected).
//! - `stages`: a cold dressing's steps one by one (bodies, guns, sounds,
//!   HUD, looks, the textures' decoding) and the files it read, read
//!   again (the vfs and archives' share).

use std::{sync::Arc, time::Instant};

use chimera_core::{metrics::counters::InstructionCounter, mount::Mount};
use chimera_world::map::{
    LoadContext, MapData, MapLoadOptions,
    cache::{self, MapCache, MapKey, Outcome},
};
use source_engine::content::{material::MaterialLoader, surfaceprops::SurfaceProps};

const GAMES: &[&str] = &["cs_source", "hl2dm", "tf2", "combat_arms"];

fn dir() -> Option<std::path::PathBuf> {
    std::env::var_os("CHIMERA_CONVCACHE_DIR").map(Into::into)
}

fn games() -> Vec<&'static str> {
    let only = std::env::var("CHIMERA_CONVCACHE_GAMES").unwrap_or_default();
    GAMES
        .iter()
        .copied()
        .filter(|g| only.is_empty() || only.split(',').any(|o| o.trim() == *g))
        .collect()
}

/// A game's mount, its converter's version and its dressing of an empty
/// map from that mount (textures decoded, its kept guns).
struct Guest {
    mount: Arc<Mount>,
    converter: &'static str,
    dress: Box<dyn Fn(&Mount, &mut MapData)>,
}

fn guest(game: &str) -> Option<Guest> {
    let open = |install: Option<_>, open: fn(&_) -> Result<Mount, _>| install.and_then(|i| open(&i).ok()).map(Arc::new);
    Some(match game {
        "cs_source" => Guest {
            mount: open(cs_source::game::INFO.install(), |i| {
                cs_source::game::mount::open(i).map_err(|e| e.to_string())
            })?,
            converter: cs_source::game::bsp::CONVERTER,
            dress: Box::new(|m, map| cs_source::game::bsp::dress_guest(m, map, cs_source::game::BUNDLE.kept, false)),
        },
        "hl2dm" => Guest {
            mount: open(hl2dm::game::INFO.install(), |i| {
                hl2dm::game::mount::open(i).map_err(|e| e.to_string())
            })?,
            converter: hl2dm::game::maps::CONVERTER,
            dress: Box::new(|m, map| hl2dm::game::maps::dress_guest_in(m, map, hl2dm::game::BUNDLE.kept, false)),
        },
        "tf2" => Guest {
            mount: open(tf2::game::INFO.install(), |i| {
                tf2::game::mount::open(i).map_err(|e| e.to_string())
            })?,
            converter: tf2::game::maps::CONVERTER,
            dress: Box::new(|m, map| tf2::game::maps::dress_guest_in(m, map, tf2::game::BUNDLE.kept, false)),
        },
        "combat_arms" => {
            let data = combat_arms::game::data::CaData::install_for(&LoadContext::default());
            let mount = data.mount()?;
            Guest {
                mount,
                converter: combat_arms::game::world::CONVERTER,
                dress: Box::new(move |m, map| combat_arms::game::world::dress_with(&data, m, map, true)),
            }
        }
        _ => return None,
    })
}

/// The dressing on an empty map, as `guest::kept` keeps it (textures of
/// whole DXT blocks keep those alone).
fn convert_parts(g: &Guest) -> MapData {
    let mut map = MapData::default();
    (g.dress)(&g.mount, &mut map);
    for t in &mut map.textures {
        t.keep_blocks_only();
    }
    map
}

fn key(game: &str, g: &Guest) -> cache::Key {
    cache::key(&format!("guest:{game}"), g.converter).options(&MapLoadOptions::default())
}

struct Meter(Instant, InstructionCounter, u64);

impl Meter {
    fn start(counter: InstructionCounter) -> Self {
        let mut c = counter;
        let at = c.read().unwrap_or(0);
        Self(Instant::now(), c, at)
    }
    /// Milliseconds and millions of instructions since the start (or the
    /// last `lap`).
    fn lap(&mut self) -> (f64, f64) {
        let ms = self.0.elapsed().as_secs_f64() * 1000.0;
        let now = self.1.read().unwrap_or(0);
        let m = now.saturating_sub(self.2) as f64 / 1e6;
        (self.0, self.2) = (Instant::now(), now);
        (ms, m)
    }
}

fn summary(map: &MapData) -> String {
    let texels: usize = map
        .textures
        .iter()
        .map(|t| {
            t.rgba8.len()
                + t.mips.iter().map(Vec::len).sum::<usize>()
                + t.bc.as_ref().map_or(0, |b| b.levels.iter().map(Vec::len).sum())
        })
        .sum();
    let samples: usize = map.sounds.clips.iter().map(|c| c.samples.len()).sum();
    format!(
        "{} textures ({:.0} MB), {} bodies, {} held, {} view models, {} sound clips ({:.0} MB of samples)",
        map.textures.len(),
        texels as f64 / 1e6,
        map.kit.characters.len(),
        map.kit.held.len(),
        map.kit.view_models.len(),
        map.sounds.clips.len(),
        samples as f64 * 2.0 / 1e6,
    )
}

fn each(what: &str, run: impl Fn(&str, &Guest, InstructionCounter)) {
    if dir().is_none() {
        eprintln!("skipping: set CHIMERA_CONVCACHE_DIR (a scratch folder)");
        return;
    }
    // Opened first: every thread the conversions start is counted.
    for game in games() {
        let counter = InstructionCounter::current_with_children();
        match guest(game) {
            Some(g) => run(game, &g, counter),
            None => eprintln!("{what} {game}: no install"),
        }
    }
}

#[test]
fn convert() {
    each("convert", |game, g, counter| {
        let mut m = Meter::start(counter);
        let map = convert_parts(g);
        let (ms, mi) = m.lap();
        eprintln!("convert {game}: {ms:.0} ms, {mi:.0} M instructions; {}", summary(&map));
    });
}

#[test]
fn write() {
    each("write", |game, g, counter| {
        let cache = MapCache::at(dir().unwrap(), u64::MAX);
        let mut m = Meter::start(counter);
        let mut conversion = (0.0, 0.0);
        let (map, outcome) = cache::cached(Some(&cache), &g.mount, key(game, g), || {
            let mut c = Meter::start(InstructionCounter::current_with_children());
            let map = convert_parts(g);
            conversion = c.lap();
            Ok(map)
        })
        .unwrap();
        let (ms, mi) = m.lap();
        eprintln!(
            "write {game} ({outcome:?}): {ms:.0} ms, {mi:.0} M instructions in all; converting {:.0} ms, {:.0} M; {}",
            conversion.0,
            conversion.1,
            summary(&map)
        );
    });
}

#[test]
fn read() {
    each("read", |game, g, counter| {
        let cache = MapCache::at(dir().unwrap(), u64::MAX);
        let mut m = Meter::start(counter);
        let (map, outcome) =
            cache::cached(Some(&cache), &g.mount, key(game, g), || Err("not in the cache".into())).unwrap();
        let (ms, mi) = m.lap();
        assert_eq!(outcome, Outcome::Hit);
        eprintln!("read {game}: {ms:.0} ms, {mi:.0} M instructions; {}", summary(&map));
    });
}

/// A cold dressing's steps, each counted (the Source guests' steps as
/// their `dress_guest_in` takes them; Combat Arms' three), then the files
/// it read, read again from the mount (warm in the OS's cache).
#[test]
fn stages() {
    each("stages", |game, g, counter| {
        let mut m = Meter::start(counter);
        let mut line = Vec::new();
        let mut lap = |name: &str, m: &mut Meter| {
            let (ms, mi) = m.lap();
            line.push(format!("{name} {ms:.0} ms {mi:.0} M"));
        };
        let tracking = g.mount.track();
        let mut map = MapData::default();
        let mount: &Mount = &g.mount;
        match game {
            "tf2" => {
                // TF2's steps are private to its dressing: the whole of it.
                tf2::game::maps::dress_guest_in(mount, &mut map, tf2::game::BUNDLE.kept, false);
                lap("whole dressing, textures decoded inside", &mut m);
            }
            "cs_source" | "hl2dm" => {
                use cs_source::game::bsp as cs;
                let mut materials = MaterialLoader::guest(mount, 0, 0);
                let surfaces = SurfaceProps::load(&mut materials);
                let mut guest = chimera_world::map::GameKit::default();
                let mut warnings = Vec::new();
                lap("surfaces", &mut m);
                match game {
                    "cs_source" => {
                        let kit = chimera_world::map::KitLoad::Players(cs_source::game::BUNDLE.kept);
                        cs::add_bodies(&mut materials, &surfaces, kit, &mut guest, &mut warnings);
                        lap("bodies", &mut m);
                        cs::add_guns(&mut materials, &surfaces, kit, &mut guest, &mut warnings);
                        lap("guns", &mut m);
                        let _ = cs_source::game::hud::load(&mut materials, "");
                        lap("hud", &mut m);
                        let _ = cs_source::game::sound::load(&mut materials, "", &surfaces, &[]);
                        lap("sounds", &mut m);
                        cs::add_guest_looks(&mut materials, &mut map);
                        lap("looks", &mut m);
                    }
                    "hl2dm" => {
                        use hl2dm::game::{bodies, maps};
                        let kit = chimera_world::map::KitLoad::Players(hl2dm::game::BUNDLE.kept);
                        bodies::add_bodies(&mut materials, &surfaces, kit, &mut guest, &mut warnings);
                        lap("bodies", &mut m);
                        bodies::add_held(&mut materials, &surfaces, kit, &mut guest, &mut warnings);
                        maps::add_view_models(&mut materials, kit, &mut guest.view_models, &mut warnings);
                        lap("guns", &mut m);
                        let _ = hl2dm::game::hud::load(&mut materials);
                        lap("hud", &mut m);
                        let names = maps::SOUNDS
                            .iter()
                            .map(|s| s.to_string())
                            .chain(hl2dm::game::sounds::entries());
                        let _ =
                            source_engine::content::sound::load_with(&mut materials, "", &surfaces, &[], names, true);
                        lap("sounds", &mut m);
                        maps::add_guest_looks(&mut materials, &mut map);
                        lap("looks", &mut m);
                    }
                    _ => unreachable!(),
                }
                materials.finish_textures();
                lap("textures decoded", &mut m);
                for t in &mut materials.textures {
                    t.keep_blocks_only();
                }
            }
            _ => {
                let data = combat_arms::game::data::CaData::install_for(&LoadContext::default());
                let file = |p: &str| mount.read(p).ok();
                combat_arms::game::effects::library::add_looks(&mut map, &data, &file);
                lap("looks", &mut m);
                combat_arms::game::view_model::add_view_models(&mut map, &data, &file);
                lap("view models", &mut m);
                let models = combat_arms::game::characters::models::shared_character_models(&data, &file, true);
                lap("character models", &mut m);
                combat_arms::game::characters::models::append_shared(
                    &mut map,
                    &models,
                    &chimera_world::map::KitLoad::All,
                );
                lap("append", &mut m);
            }
        }
        let accesses = tracking.finish();
        let paths: Vec<String> = accesses
            .iter()
            .filter_map(|a| match a {
                chimera_core::mount::Access::File(p) | chimera_core::mount::Access::Every(p) => Some(p.clone()),
                _ => None,
            })
            .collect();
        let mut bytes = 0usize;
        let mut read = 0;
        for p in &paths {
            if let Ok(b) = mount.read(p) {
                bytes += b.len();
                read += 1;
            }
        }
        lap(
            &format!("its {read} files read again ({:.0} MB)", bytes as f64 / 1e6),
            &mut m,
        );
        eprintln!("stages {game}: {}", line.join("; "));
    });
}

/// What a converted dressing's bytes are (postcard, as the cache encodes
/// it): textures, sounds, and the models' meshes, rigs (skeleton and
/// animations) and LODs, against the original files it read.
#[test]
fn sizes() {
    each("sizes", |game, g, _| {
        let tracking = g.mount.track();
        let map = convert_parts(g);
        let read: usize = tracking
            .finish()
            .iter()
            .filter_map(|a| match a {
                chimera_core::mount::Access::File(p) | chimera_core::mount::Access::Every(p) => {
                    g.mount.read(p).ok().map(|b| b.len())
                }
                _ => None,
            })
            .sum();
        let mb = |n: usize| n as f64 / 1e6;
        let size = |v: &dyn erased::Ser| v.len();
        let models = map
            .kit
            .characters
            .iter()
            .map(|c| &c.model)
            .chain(map.kit.held.iter().map(|h| &h.model))
            .chain(map.kit.view_models.iter().map(|v| &v.model));
        let (mut meshes, mut rigs, mut lods) = (0, 0, 0);
        for m in models {
            meshes += size(&m.meshes);
            rigs += size(&m.rig);
            lods += size(&m.lods);
        }
        let anims: usize = map.kit.characters.iter().map(|c| size(&c.animations)).sum::<usize>()
            + map.kit.view_models.iter().map(|v| size(&v.animations)).sum::<usize>();
        let whole = size(&map);
        let (textures, sounds) = (size(&map.textures), size(&map.sounds));
        eprintln!(
            "sizes {game}: whole {:.0} MB (originals read {:.0} MB): textures {:.0}, sounds {:.0}, meshes {:.0}, rigs {:.0}, animations {:.0}, lods {:.0}, the rest {:.0}",
            mb(whole),
            mb(read),
            mb(textures),
            mb(sounds),
            mb(meshes),
            mb(rigs),
            mb(anims),
            mb(lods),
            mb(whole.saturating_sub(textures + sounds + meshes + rigs + anims + lods)),
        );
    });
}

mod erased {
    /// A value's postcard length.
    pub trait Ser {
        fn len(&self) -> usize;
    }
    impl<T: serde::Serialize> Ser for T {
        fn len(&self) -> usize {
            postcard::to_allocvec(self).map_or(0, |v| v.len())
        }
    }
}
