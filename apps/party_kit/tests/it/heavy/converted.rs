//! Each game's guest dressing as a converted asset
//! (docs/plans/active/converted-cache.md): converted twice from the same
//! files it serializes (postcard, as the converted-map cache writes it) to
//! the same bytes, the generated-assets rule's "same input, same output"
//! (step 1: the `HashMap`s a map holds are written in key order,
//! chimera_world's `pod_serde::sorted`); a dressing kept for the session
//! (`map::BlocksOnly`) skips decoding its whole-DXT textures and keeps
//! the same textures (step 2); read back from the disk layer
//! (`map::kept::kept_with`), the same bytes as converting (step 4); the
//! disk layer on by default, and a guest's cold first dressing measured
//! per process (step 6).
//!
//! Each game passes with a note without its install (CS:S, HL2:DM, TF2,
//! Combat Arms with its keys).

use chimera_core::mount::config::LocalConfig;
use chimera_world::map::MapData;

/// The guests whose install is here, each with its dressing of an empty
/// map (textures decoded, its kept guns), as its kept dressing converts
/// it.
fn guests() -> Vec<(&'static str, Box<dyn Fn(&mut MapData)>)> {
    let mut out: Vec<(&'static str, Box<dyn Fn(&mut MapData)>)> = Vec::new();
    if cs_source::game::INFO.install().is_some() {
        out.push((
            cs_source::game::GAME,
            Box::new(move |m: &mut MapData| {
                cs_source::game::dress_guest_with(m, cs_source::game::BUNDLE.kept, false).unwrap();
            }),
        ));
    }
    if hl2dm::game::INFO.install().is_some() {
        out.push((
            hl2dm::game::GAME,
            Box::new(move |m: &mut MapData| {
                hl2dm::game::maps::dress_guest_with(m, hl2dm::game::BUNDLE.kept, false).unwrap();
            }),
        ));
    }
    if tf2::game::INFO.install().is_some_and(|p| chimera_core::mount::served::usable("tf2", p, "tf")) {
        out.push((
            tf2::game::GAME,
            Box::new(move |m: &mut MapData| {
                tf2::game::maps::dress_guest_with(m, tf2::game::BUNDLE.kept, false).unwrap();
            }),
        ));
    }
    let ca = LocalConfig::load()
        .ok()
        .and_then(|c| c.game_path(combat_arms::game::GAME))
        .is_some_and(|p| chimera_core::mount::served::usable("combat_arms", p, "Game"));
    if ca {
        out.push((
            combat_arms::game::GAME,
            Box::new(move |m: &mut MapData| {
                let data = combat_arms::game::data::CaData::install();
                let mount = data.mount().expect("Combat Arms' install mounts");
                combat_arms::game::world::dress(&data, &mount, m);
            }),
        ));
    }
    out
}

/// The parts of a map's bytes, in the order they're written, for a
/// failure that says where two encodings part.
fn parts(map: &MapData) -> Vec<(&'static str, Vec<u8>)> {
    let enc = |v: &dyn erased::Ser| v.bytes();
    vec![
        ("textures", enc(&map.textures)),
        ("sounds", enc(&*map.sounds)),
        ("kit", enc(&map.kit)),
        ("particles", enc(&map.particles)),
        ("impacts", enc(&map.impacts)),
        ("surface_names", enc(&map.surface_names)),
    ]
}

#[test]
fn a_guest_dressing_converted_twice_writes_the_same_bytes() {
    let guests = guests();
    if guests.is_empty() {
        eprintln!("skipping: needs an install of CS:S, HL2:DM, TF2 or Combat Arms");
        return;
    }
    for (game, dress) in guests {
        let run = || {
            let mut m = MapData::default();
            dress(&mut m);
            m
        };
        let (a, b) = (run(), run());
        for ((name, x), (_, y)) in parts(&a).iter().zip(parts(&b)) {
            let first = x.iter().zip(&y).position(|(p, q)| p != q);
            assert!(
                x.len() == y.len() && first.is_none(),
                "{game}: its {name} encode differently ({} and {} bytes, first difference at {first:?})",
                x.len(),
                y.len()
            );
        }
        let (x, y) = (postcard::to_allocvec(&a).unwrap(), postcard::to_allocvec(&b).unwrap());
        assert!(x == y, "{game}: the whole dressing encodes differently ({} and {} bytes)", x.len(), y.len());
        eprintln!("{game}: {} bytes, the same twice", x.len());
    }
}

mod erased {
    /// A value's postcard bytes.
    pub trait Ser {
        fn bytes(&self) -> Vec<u8>;
    }
    impl<T: serde::Serialize> Ser for T {
        fn bytes(&self) -> Vec<u8> {
            postcard::to_allocvec(self).unwrap()
        }
    }
}

/// The Source guests' dressings as kept (`map::BlocksOnly`: their
/// whole-DXT textures never decoded, `MaterialLoader::blocks_only`) give
/// the same textures, field by field, and the same bytes as a dressing
/// decoded and then kept as blocks (`MapTexture::keep_blocks_only`), the
/// way the session cache kept them before. Prints each one's instructions
/// (this thread and those it starts) for docs/performance.md, "Guest
/// textures kept as blocks without decoding": each way twice, the second
/// counted (the first warms the OS's cache and the game's per-process
/// memos).
#[test]
fn a_kept_dressing_skips_decoding_and_keeps_the_same_textures() {
    use chimera_core::metrics::counters::InstructionCounter;
    use chimera_world::map::BlocksOnly;
    let guests: Vec<_> = guests().into_iter().filter(|(g, _)| *g != combat_arms::game::GAME).collect();
    if guests.is_empty() {
        eprintln!("skipping: needs an install of CS:S, HL2:DM or TF2");
        return;
    }
    for (game, dress) in guests {
        let run = |blocks: bool| {
            let mut counter = InstructionCounter::current_with_children();
            let at = counter.read().unwrap_or(0);
            let mut m = MapData::default();
            if blocks {
                m.memos.insert(BlocksOnly);
            }
            dress(&mut m);
            m.memos.remove::<BlocksOnly>();
            for t in &mut m.textures {
                t.keep_blocks_only();
            }
            (m, counter.read().unwrap_or(0).saturating_sub(at))
        };
        let _ = (run(false), run(true));
        let (decoded, before) = run(false);
        let (blocks, after) = run(true);
        assert_eq!(decoded.textures.len(), blocks.textures.len(), "{game}: as many textures");
        let mut kept_blocks = 0;
        for (a, b) in decoded.textures.iter().zip(&blocks.textures) {
            let same = a.name == b.name
                && a.srgb == b.srgb
                && (a.width, a.height) == (b.width, b.height)
                && a.rgba8 == b.rgba8
                && a.mips == b.mips
                && a.bc == b.bc;
            assert!(same, "{game}: texture {} differs", a.name);
            kept_blocks += usize::from(b.blocks_only());
        }
        assert!(kept_blocks > 0, "{game}: some textures are whole DXT blocks");
        let (x, y) = (postcard::to_allocvec(&decoded).unwrap(), postcard::to_allocvec(&blocks).unwrap());
        assert!(x == y, "{game}: the dressings encode differently");
        eprintln!(
            "{game}: {} textures ({kept_blocks} blocks only); decoded then kept {:.0} M instructions, kept without decoding {:.0} M ({:.0}% fewer)",
            blocks.textures.len(),
            before as f64 / 1e6,
            after as f64 / 1e6,
            100.0 * (1.0 - after as f64 / before.max(1) as f64)
        );
    }
}

/// The disk layer under the kept dressings (`map::kept::kept_with` on a
/// cache folder; the apps' by default): each Source
/// guest's dressing converted and written by one app is read back by the
/// next, nothing converted, and encodes to the same bytes. Prints each
/// one's instructions (this thread and those it starts) for
/// docs/performance.md, "Guest dressings on disk". The folder is
/// `<cache dir>/chimera/kept-cache-test`, removed after each game.
#[test]
fn a_guest_read_from_disk_is_the_same_as_converting() {
    use chimera_core::metrics::counters::InstructionCounter;
    use std::sync::Arc;

    use chimera_core::mount::Mount;
    use chimera_world::map::{MapLoadOptions, cache::MapCache, kept};
    let Some(dir) = MapCache::default_dir().and_then(|d| d.parent().map(|p| p.join("kept-cache-test"))) else {
        eprintln!("skipping: no cache folder");
        return;
    };
    type Dress = (&'static str, &'static str, fn() -> Result<Arc<Mount>, String>, Box<dyn Fn(&Mount, &mut MapData)>);
    let mut games: Vec<Dress> = Vec::new();
    if cs_source::game::INFO.install().is_some() {
        games.push((
            cs_source::game::GAME,
            cs_source::game::bsp::CONVERTER,
            cs_source::game::guest_mount,
            Box::new(|m, map| cs_source::game::bsp::dress_guest(m, map, cs_source::game::BUNDLE.kept, false)),
        ));
    }
    if hl2dm::game::INFO.install().is_some() {
        games.push((
            hl2dm::game::GAME,
            hl2dm::game::maps::CONVERTER,
            hl2dm::game::maps::guest_mount,
            Box::new(|m, map| hl2dm::game::maps::dress_guest_in(m, map, hl2dm::game::BUNDLE.kept, false)),
        ));
    }
    if tf2::game::INFO.install().is_some_and(|p| chimera_core::mount::served::usable("tf2", p, "tf")) {
        games.push((
            tf2::game::GAME,
            tf2::game::maps::CONVERTER,
            tf2::game::maps::guest_mount,
            Box::new(|m, map| tf2::game::maps::dress_guest_in(m, map, tf2::game::BUNDLE.kept, false)),
        ));
    }
    if games.is_empty() {
        eprintln!("skipping: needs an install of CS:S, HL2:DM or TF2");
        return;
    }
    for (game, converter, mount, convert) in &games {
        let game: &'static str = game;
        let _ = std::fs::remove_dir_all(&dir);
        let disk = MapCache::at(&dir, u64::MAX);
        let dressing = kept::Dressing { game, converter: *converter, mount, convert: &**convert, variant: "", memos: None };
        let app = || {
            let options = MapLoadOptions::default();
            let mut counter = InstructionCounter::current_with_children();
            let at = counter.read().unwrap_or(0);
            let parts = kept::kept_with(&options, &dressing, Some(&disk)).unwrap();
            let spent = counter.read().unwrap_or(0).saturating_sub(at);
            let counts = (kept::conversions(&options.context, game, "", true), kept::disk_reads(&options.context, game, "", true));
            (postcard::to_allocvec(&*parts).unwrap(), counts, spent)
        };
        let (written, w, miss) = app();
        let (read, r, hit) = app();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(w, (1, 0), "{game}: the first app converts");
        assert_eq!(r, (0, 1), "{game}: the next reads it back");
        assert!(written == read, "{game}: read back, the same bytes ({} and {})", written.len(), read.len());
        eprintln!(
            "{game}: converted and written {:.0} M instructions, read back {:.0} M ({:.1}x fewer), {:.0} MB",
            miss as f64 / 1e6,
            hit as f64 / 1e6,
            miss as f64 / hit.max(1) as f64,
            read.len() as f64 / 1e6
        );
    }
}

/// One guest's cold first dressing in this process, through its bundle's
/// dressing (Chimera's `map::kept`, its disk layer as the environment
/// sets it: `CHIMERA_GUEST_CACHE`, `CHIMERA_MAP_CACHE`), for
/// docs/performance.md, "Guest dressings on disk"
/// (plan converted-cache, step 6). The game is `CHIMERA_MEASURE_GUEST`
/// (`cs_source`, `hl2dm`, `tf2`, `combat_arms`); without it, skipped.
/// Prints the instructions (this thread and those it starts) and wall
/// time; for Combat Arms also its install data's weapon set after it
/// (held through the dressing, as an app holds it), which its weapons
/// need whatever the dressing does: none left when the dressing parsed it. One measurement
/// per process (the first dressing is the cold one).
#[test]
fn measure_a_cold_first_dressing() {
    use chimera_core::metrics::counters::InstructionCounter;
    use chimera_world::map::{MapDressing, MapLoadOptions, kept};
    let Ok(game) = std::env::var("CHIMERA_MEASURE_GUEST") else {
        eprintln!("skipping: set CHIMERA_MEASURE_GUEST to a game's id");
        return;
    };
    let dressing: &MapDressing = match game.as_str() {
        "cs_source" => &cs_source::game::bundle::GUEST,
        "hl2dm" => &hl2dm::game::GUEST,
        "tf2" => &tf2::game::GUEST,
        "combat_arms" => &combat_arms::game::world::DRESSING,
        other => panic!("no guest {other}"),
    };
    let counted = |f: &mut dyn FnMut()| {
        let mut c = InstructionCounter::current_with_children();
        let at = c.read().unwrap_or(0);
        let t = std::time::Instant::now();
        f();
        (c.read().unwrap_or(0).saturating_sub(at), t.elapsed().as_millis())
    };
    let options = MapLoadOptions::default();
    // Combat Arms' install data held as an app holds it (else it goes
    // with the dressing and comes again after).
    let held = (game == "combat_arms").then(|| combat_arms::game::data::CaData::install_for(&options.context));
    let mut map = MapData::default();
    let (n, ms) = counted(&mut || (dressing.dress)(&mut map, &options).unwrap());
    let reads: usize = ["", "view_models"].iter().map(|v| kept::disk_reads(&options.context, dressing.game, v, true)).sum();
    let how = if kept::disk_cache().is_none() { "off" } else if reads > 0 { "read" } else { "written" };
    let bytes = postcard::to_allocvec(&map).map_or(0, |b| b.len());
    eprintln!("MEASURE {game} dressing {how}: {:.0} M instructions, {ms} ms, {:.0} MB", n as f64 / 1e6, bytes as f64 / 1e6);
    if let Some(data) = held {
        let (n, ms) = counted(&mut || {
            data.weapon_set();
        });
        eprintln!("MEASURE {game} install data after it: {:.0} M instructions, {ms} ms", n as f64 / 1e6);
    }
}

/// The disk layer is on by default (plan converted-cache, step 6): two
/// apps dressing a map with CS:S's guest parts the apps' way (its
/// bundle's dressing, the environment's cache: the default folder unless
/// `CHIMERA_GUEST_CACHE=0` or `CHIMERA_MAP_CACHE` says otherwise), the
/// second reads the first's parts back, nothing converted, the same
/// bytes. Leaves CS:S's entry in the cache, as an app would. Skipped
/// without the install or with the layer turned off.
#[test]
fn a_guest_dressing_reads_back_from_disk_by_default() {
    use chimera_world::map::{MapLoadOptions, kept};
    if cs_source::game::INFO.install().is_none() {
        eprintln!("skipping: needs CS:S's install");
        return;
    }
    if kept::disk_cache().is_none() {
        eprintln!("skipping: the disk layer is turned off here ({} or CHIMERA_MAP_CACHE)", kept::DISK_ENV);
        return;
    }
    let game = cs_source::game::GAME;
    let app = || {
        let options = MapLoadOptions::default();
        let mut map = MapData::default();
        (cs_source::game::bundle::GUEST.dress)(&mut map, &options).unwrap();
        let counts = (kept::conversions(&options.context, game, "", true), kept::disk_reads(&options.context, game, "", true));
        (postcard::to_allocvec(&map).unwrap(), counts)
    };
    let (first, (c, r)) = app();
    assert_eq!(c + r, 1, "the first app converts or reads an earlier run's entry");
    let (second, counts) = app();
    assert_eq!(counts, (0, 1), "the next reads it back");
    assert!(first == second, "read back, the same bytes");
}
