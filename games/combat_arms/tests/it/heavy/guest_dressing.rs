//! Combat Arms' guest dressing (`world::DRESSING`, its parts on another
//! game's map) on Chimera's kept dressings (`map::kept`,
//! docs/plans/active/converted-cache.md step 5): where a session's cold
//! first dressing goes (the install data, the effect file, the effect
//! looks, the view models, the character models), printed for
//! docs/performance.md. Passes with a note without the install.

use chimera_core::{metrics::InstructionCounter, mount::config::LocalConfig};
use chimera_world::map::MapData;
use combat_arms::game::{GAME, data::CaData, effects::library, view_model, world};

fn install() -> bool {
    let here = LocalConfig::load()
        .ok()
        .and_then(|c| c.game_path(GAME))
        .is_some_and(|p| chimera_core::mount::served::usable("combat_arms", &p, "Game"));
    if !here {
        eprintln!("skipping: no Combat Arms install configured");
    }
    here
}

fn counted<T>(f: impl FnOnce() -> T) -> (T, u64) {
    let mut c = InstructionCounter::current_with_children();
    let a = c.read().unwrap_or(0);
    let out = f();
    (out, c.read().unwrap_or(0).saturating_sub(a))
}

/// Run alone (a fresh process: the install data cold).
#[test]
fn where_a_cold_dressing_goes() {
    if !install() {
        return;
    }
    let started = std::time::Instant::now();
    let data = CaData::install();
    let mut total = 0u64;
    let mut step = |name: &str, f: &mut dyn FnMut()| {
        let t = std::time::Instant::now();
        let ((), n) = counted(f);
        total += n;
        eprintln!(
            "{name}: {:.0} M instructions, {} ms",
            n as f64 / 1e6,
            t.elapsed().as_millis()
        );
    };
    let mount = std::cell::OnceCell::new();
    step("mount", &mut || {
        mount.set(data.mount().expect("mounts")).ok();
    });
    let mount = mount.get().unwrap();
    let file = |p: &str| mount.read(p).ok();
    step("attributes (ATTRIBUTES.rez read and parsed)", &mut || {
        data.attributes();
    });
    {
        // The parts of the install data, apart (their results dropped).
        let bytes = std::cell::RefCell::new(std::collections::BTreeMap::<String, Vec<u8>>::new());
        step("  attribute files read again (decrypted)", &mut || {
            let attrs_mount = {
                let keys = combat_arms::game::mount::keys().unwrap();
                let install = LocalConfig::load().unwrap().game_path(GAME).unwrap();
                let a = lithtech_formats::rez::RezArchive::open(&install.join("Game").join("ATTRIBUTES.rez"), &keys)
                    .unwrap();
                let mut m = chimera_core::mount::Mount::default();
                m.push(a);
                m
            };
            combat_arms::game::data::read_attributes(|n| {
                let b = attrs_mount.read(n)?;
                bytes.borrow_mut().insert(n.to_string(), b.clone());
                Ok(b)
            });
        });
        step("  attribute files parsed again (from memory)", &mut || {
            let b = bytes.borrow();
            combat_arms::game::data::read_attributes(|n| {
                b.get(n).cloned().ok_or_else(|| std::io::Error::other("none"))
            });
        });
        let merged = std::cell::RefCell::new(Vec::new());
        step("  weapon documents merged again", &mut || {
            let docs: Vec<(&str, &str)> = combat_arms::game::data::WEAPON_DATA.to_vec();
            let a = data.attributes().unwrap();
            let mpu = combat_arms::game::movement::movement_data().meters_per_unit as f64;
            *merged.borrow_mut() = combat_arms::game::data::merge_weapon_docs_with_mods(
                &docs,
                a.weapons.as_ref().unwrap(),
                a.mods.as_ref(),
                mpu,
            )
            .unwrap();
        });
        step("  merged documents parsed again (WeaponSet::parse)", &mut || {
            let m = merged.borrow();
            let refs: Vec<(&str, &str)> = m.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
            chimera_sim::weapon::data::WeaponSet::parse(&refs, &[]).unwrap();
        });
        let m = merged.borrow();
        eprintln!(
            "  merged: {} documents, {:.1} MB of TOML; attribute files {:.1} MB",
            m.len(),
            m.iter().map(|(n, t)| n.len() + t.len()).sum::<usize>() as f64 / 1e6,
            bytes.borrow().values().map(Vec::len).sum::<usize>() as f64 / 1e6
        );
    }
    step("weapon set (merged and parsed)", &mut || {
        data.weapon_set();
    });
    step("character data", &mut || {
        data.characters();
    });
    step("effect file read and parsed", &mut || {
        let b = file(library::FX_FILE).unwrap();
        lithtech_formats::fxf::FxFile::parse(&String::from_utf8_lossy(&b)).unwrap();
    });
    let mut map = MapData::default();
    map.memos.insert(chimera_world::map::BlocksOnly);
    step("effect looks (add_looks)", &mut || {
        library::add_looks(&mut map, &data, &file)
    });
    step("view models", &mut || {
        view_model::add_view_models(&mut map, &data, &file)
    });
    step("body files parsed (rigs)", &mut || {
        let shared = combat_arms::game::model::ChildModels::default();
        let chars = data.characters();
        for id in combat_arms::game::characters::models::LOADED {
            if let Some(r) = chars.model(*id) {
                let _ = combat_arms::game::model::load_sharing(&r.file, &file, &shared);
            }
        }
    });
    step("character models", &mut || {
        let models = combat_arms::game::characters::models::shared_character_models(&data, &file, true);
        combat_arms::game::characters::models::append_shared(&mut map, &models, &chimera_world::map::KitLoad::All);
    });
    eprintln!(
        "whole: {:.0} M instructions, {} ms; {} textures, {} bodies, {} held, {} view models",
        total as f64 / 1e6,
        started.elapsed().as_millis(),
        map.textures.len(),
        map.kit.characters.len(),
        map.kit.held.len(),
        map.kit.view_models.len()
    );
    let _ = world::DRESSING.game;
}

/// What two kept memos hold, compared (`map::kept::KeptMemos`): each
/// rig (its model, animation list, weight sets and heights), the body
/// groups, the held sockets and the effect library.
fn same_memos(a: &chimera_world::map::MapMemos, b: &chimera_world::map::MapMemos, what: &str) {
    use combat_arms::game::characters::models::CaModelIndex;
    let (x, y) = (
        a.get::<CaModelIndex>().expect("an index"),
        b.get::<CaModelIndex>().expect("an index"),
    );
    assert_eq!(
        x.rigs().keys().collect::<Vec<_>>(),
        y.rigs().keys().collect::<Vec<_>>(),
        "{what}: rigs' paths"
    );
    for ((path, r), s) in x.rigs().iter().zip(y.rigs().values()) {
        assert!(r.model == s.model, "{what}: {path}'s model");
        assert!(*r.list == *s.list, "{what}: {path}'s animation list");
        assert_eq!(
            (r.upper, r.lower, r.half_heights),
            (s.upper, s.lower, s.half_heights),
            "{what}: {path}"
        );
    }
    // Rigs shared by paths stay shared.
    let shared = |i: &CaModelIndex| {
        let rigs: Vec<_> = i.rigs().values().collect();
        rigs.iter()
            .map(|r| rigs.iter().position(|s| std::sync::Arc::ptr_eq(r, s)))
            .collect::<Vec<_>>()
    };
    assert_eq!(shared(x), shared(y), "{what}: rigs shared alike");
    assert_eq!(x.groups(), y.groups(), "{what}: body groups");
    assert_eq!(x.held_sockets(), y.held_sockets(), "{what}: held sockets");
    let (l, m) = (
        library::library(a).expect("a library"),
        library::library(b).expect("a library"),
    );
    // By their text: the file holds NaN numbers, never equal to themselves.
    assert!(
        format!("{:?}", l.file) == format!("{:?}", m.file),
        "{what}: effect file"
    );
    assert_eq!(
        postcard::to_allocvec(&l.saved()).unwrap(),
        postcard::to_allocvec(&m.saved()).unwrap(),
        "{what}: effect library"
    );
}

/// Combat Arms' guest dressing kept (`world::dress_kept`, Chimera's
/// `map::kept`): dressed on an empty map it gives the same parts and
/// memos as `dress_with` (textures kept as their blocks, as every kept
/// dressing's); written to the disk layer by one app and read back by the
/// next, nothing converted, the same bytes and memos. Prints each one's
/// instructions for docs/performance.md, "Guest dressings on disk". The
/// folder is `<cache dir>/chimera/kept-cache-ca-test`, removed after.
#[test]
fn a_kept_dressing_reads_back_the_same_parts_and_memos() {
    use chimera_world::map::{BlocksOnly, MapLoadOptions, cache::MapCache, kept};
    if !install() {
        return;
    }
    let Some(dir) = MapCache::default_dir().and_then(|d| d.parent().map(|p| p.join("kept-cache-ca-test"))) else {
        eprintln!("skipping: no cache folder");
        return;
    };
    let _ = std::fs::remove_dir_all(&dir);
    let disk = MapCache::at(&dir, u64::MAX);
    let app = || {
        let options = MapLoadOptions::default();
        let mut map = MapData::default();
        let ((), spent) = counted(|| world::dress_kept(&mut map, &options, Some(&disk)).unwrap());
        let count = |f: fn(&chimera_world::map::LoadContext, &'static str, &'static str, bool) -> usize| {
            f(&options.context, GAME, "", true) + f(&options.context, GAME, "view_models", true)
        };
        (map, (count(kept::conversions), count(kept::disk_reads)), spent)
    };
    let (written, w, miss) = app();
    let (read, r, hit) = app();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(w, (1, 0), "the first app converts");
    assert_eq!(r, (0, 1), "the next reads it back");
    let (x, y) = (
        postcard::to_allocvec(&written).unwrap(),
        postcard::to_allocvec(&read).unwrap(),
    );
    assert!(x == y, "read back, the same bytes ({} and {})", x.len(), y.len());
    same_memos(&written.memos, &read.memos, "read back");

    // As the dressing itself gives them.
    let data = CaData::install();
    let mount = data.mount().unwrap();
    let mut fresh = MapData::default();
    fresh.memos.insert(BlocksOnly);
    world::dress_with(&data, &mount, &mut fresh, true);
    fresh.memos.remove::<BlocksOnly>();
    for t in &mut fresh.textures {
        t.keep_blocks_only();
    }
    let z = postcard::to_allocvec(&fresh).unwrap();
    assert!(x == z, "kept, the same bytes as dressing ({} and {})", x.len(), z.len());
    same_memos(&fresh.memos, &written.memos, "kept");
    // Where a read goes: the parts' payload, the memos' index and library.
    {
        let (_, n) = counted(|| postcard::from_bytes::<MapData>(&y).unwrap());
        eprintln!("  the parts decoded: {:.0} M", n as f64 / 1e6);
        let index = read
            .memos
            .get::<combat_arms::game::characters::models::CaModelIndex>()
            .unwrap();
        let (bytes, n) = counted(|| combat_arms::game::characters::kept::save(index, &data).unwrap());
        eprintln!(
            "  the index saved: {:.0} M, {:.0} MB",
            n as f64 / 1e6,
            bytes.len() as f64 / 1e6
        );
        let (_, n) = counted(|| combat_arms::game::characters::kept::load(&bytes, &data).unwrap());
        eprintln!("  the index loaded: {:.0} M", n as f64 / 1e6);
        let file = |p: &str| mount.read(p).ok();
        let lib = library::library(&read.memos).unwrap().saved();
        let (_, n) = counted(|| library::FxLibrary::from_saved(lib, &data, &file).unwrap());
        eprintln!("  the library loaded: {:.0} M", n as f64 / 1e6);
    }
    eprintln!(
        "combat_arms: converted and written {:.0} M instructions, read back {:.0} M ({:.1}x fewer), {:.0} MB",
        miss as f64 / 1e6,
        hit as f64 / 1e6,
        miss as f64 / hit.max(1) as f64,
        y.len() as f64 / 1e6
    );
}

/// Combat Arms' merged weapon documents kept on disk
/// (`CaData::weapon_docs_merged`, its own entry beside the kept
/// dressings): merged twice, the same documents; read back, the same
/// again, for fewer instructions. Prints both for docs/performance.md.
#[test]
fn the_merged_weapon_documents_read_back_the_same() {
    use chimera_world::map::cache::{MapCache, Outcome};
    if !install() {
        return;
    }
    let Some(dir) = MapCache::default_dir().and_then(|d| d.parent().map(|p| p.join("kept-cache-ca-docs-test"))) else {
        eprintln!("skipping: no cache folder");
        return;
    };
    let _ = std::fs::remove_dir_all(&dir);
    let disk = MapCache::at(&dir, u64::MAX);
    let data = CaData::install();
    data.attributes();
    let run = |disk: Option<&MapCache>| counted(|| data.weapon_docs_merged(disk).unwrap().expect("install weapons"));
    let ((plain, how), merging) = run(None);
    assert_eq!(how, Outcome::Off);
    let ((written, how), _) = run(Some(&disk));
    assert_eq!(how, Outcome::Miss);
    let ((read, how), reading) = run(Some(&disk));
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(how, Outcome::Hit);
    assert!(plain == written && written == read, "the same documents");
    eprintln!(
        "combat_arms: {} weapon documents merged {:.0} M instructions, read back {:.0} M",
        read.len(),
        merging as f64 / 1e6,
        reading as f64 / 1e6
    );
}
