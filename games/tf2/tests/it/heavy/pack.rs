//! The M2 guns' asset sets (`game::sets`, what `chimera_pack sets tf2`
//! records) packed from the install into a scratch store under `target/`
//! (never the user's or the server's), then read back from that store
//! alone (the install off, as `CHIMERA_ASSET_USE_INSTALL=0` with the
//! scratch store): every file as the packer's view of the install has
//! it, each `c_` weapon model and its class's arms loading, and, with the
//! `.ctx` key at run time, each gun's script served plain from its record
//! (`ScriptSource::Plain`: the store holds it decrypted). Without the key
//! the scripts are left out of the store and that part passes with a
//! note; without the install the whole test does.

use std::{collections::BTreeMap, path::Path};

use chimera_core::mount::{
    assets::{AssetSettings, Manifest, Store, pack},
    served,
};
use source_engine::content::{material::MaterialLoader, props};
use tf2::game::{
    GAME, mount,
    scripts::{self, ScriptSource},
    sets,
};

use super::maps::install;

#[test]
fn the_weapon_sets_pack_into_a_scratch_store_and_read_back_from_it_alone() {
    let Some(install) = install() else { return };
    if !install.join("tf").is_dir() {
        eprintln!("skipping: packs from the install's own files (served from the asset store here)");
        return;
    }
    let key = scripts::key().expect("the key's config");
    if key.is_none() {
        eprintln!("note: no .ctx key ({}): the scripts are left out of the store", scripts::KEY_ENV);
    }
    let plain = mount::open_install(&install).unwrap();
    for w in sets::M2_WEAPONS {
        assert!(plain.contains(&scripts::path_of(w.script)), "{}: the install's script", w.script);
    }
    let source = scripts::decrypting(plain, key.clone());

    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("tf2-pack-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let store = Store::new(&root);
    let build = "m2a-test";
    let mut manifest = Manifest::new(GAME, build, Some(440));
    let sounds = sets::sound_scripts(&source);
    let mut files = BTreeMap::new();
    for w in sets::M2_WEAPONS {
        let paths = sets::weapon_files(&source, w, &sounds);
        let set = format!("weapon/{}", w.id);
        let report = pack(&source, &paths, &store, &mut manifest, Some(&set), 2).unwrap();
        eprintln!("{set}: {} files, {} bytes, missing {:?}", report.files, report.bytes_read, report.missing);
        let script = scripts::path_of(w.script);
        let missing: Vec<&String> = report.missing.iter().filter(|p| **p != script).collect();
        assert!(missing.is_empty(), "{set}: {missing:?}");
        assert_eq!(report.missing.contains(&script), key.is_none(), "{set}: the script is packed iff there is a key");
        assert!(paths.iter().any(|p| p.ends_with(".vtf")), "{set}: textures");
        assert!(paths.iter().any(|p| p.starts_with("sound/")), "{set}: sounds");
        assert!(paths.iter().any(|p| p.ends_with("_animations.mdl")), "{set}: the arms' animations");
        files.insert(w.id, paths);
    }
    manifest.save(&store.manifest_path(GAME, build)).unwrap();

    // The store alone.
    let settings = AssetSettings {
        store: Some(store.clone()),
        use_install: false,
        service_first: false,
        builds: [(GAME.to_string(), build.to_string())].into(),
        remote: None,
        lock: None,
    };
    let served = served::open_with(&settings, GAME, Path::new("no-install"), mount::open_install).unwrap();
    for w in sets::M2_WEAPONS {
        for p in &files[w.id] {
            if *p == scripts::path_of(w.script) && key.is_none() {
                assert!(!served.contains(p));
                continue;
            }
            assert_eq!(served.read(p).ok(), source.read(p).ok(), "{p}");
        }
        let mut materials = MaterialLoader::guest(&served, 0, 0);
        props::load_view_model(&mut materials, w.model, w.id, true).unwrap_or_else(|e| panic!("{}: {e}", w.model));
        for class in w.classes {
            let arms = props::load_view_model(&mut materials, &sets::arms_model(class), class, true).unwrap();
            assert!(arms.animations.as_ref().is_some_and(|a| !a.sequences.is_empty()), "{class}'s arms animate");
        }
        let script = scripts::read(&served, w.script, None);
        if key.is_some() {
            assert_eq!(script.source, ScriptSource::Plain, "{}: {:?}", w.script, script.note);
            assert!(script.data().is_some_and(|d| !d.items().is_empty()), "{}", w.script);
        } else {
            assert_eq!(script.source, ScriptSource::Missing);
            eprintln!("note: {} not checked (no key)", w.script);
        }
    }
    let _ = std::fs::remove_dir_all(&root);
}

/// The class sets (`sets::player_pieces`: `players` and `players/<class>`)
/// packed into a scratch store, then the records of every class set but
/// the Heavy's and the Scout's taken out again, as on a machine whose
/// match offered those two: the bodies a map's kit asks for every TF2 gun
/// (`bodies::add_bodies`) are exactly the Heavy's and the Scout's, loaded
/// from the store with nothing missing and nothing fetched (no fetcher:
/// a read of a missing record would be a warning); the kill icons read
/// from `players` alone; every class set holds its model, its animations
/// and its arms.
#[test]
fn the_class_sets_load_only_the_offered_classes_bodies() {
    use chimera_world::map::{GameKit, KitLoad};
    use source_engine::content::surfaceprops::SurfaceProps;
    use tf2::game::{bodies, classes, hud};

    let Some(install) = install() else { return };
    if !install.join("tf").is_dir() {
        eprintln!("skipping: packs from the install's own files (served from the asset store here)");
        return;
    }
    let source = mount::open_install(&install).unwrap();
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("tf2-class-sets-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let store = Store::new(&root);
    let build = "class-sets-test";
    let mut manifest = Manifest::new(GAME, build, Some(440));
    let pieces = sets::player_pieces(&source);
    for (set, paths) in &pieces {
        let report = pack(&source, paths, &store, &mut manifest, Some(set), 2).unwrap();
        assert!(report.missing.is_empty(), "{set}: {:?}", report.missing);
        eprintln!("{set}: {} files, {} bytes stored", report.files, report.stored);
    }
    for c in &classes::classes().classes {
        let files = &manifest.sets[&classes::set_of(&c.name)];
        for want in [c.model.clone(), c.model.replace(".mdl", "_animations.mdl"), sets::arms_model(&c.arms)] {
            assert!(files.contains(&want), "{}: {want}", c.name);
        }
    }
    let shared = &manifest.sets["players"];
    assert!(shared.contains("scripts/mod_textures.txt") && shared.contains("materials/hud/d_images.vtf"), "the kill icons");
    manifest.save(&store.manifest_path(GAME, build)).unwrap();
    // Only `players`, the Heavy's and the Scout's records stay.
    let kept: std::collections::BTreeSet<&str> = ["players", "players/heavy", "players/scout"]
        .iter()
        .flat_map(|s| manifest.sets[*s].iter())
        .map(|p| manifest.files[p].id.as_str())
        .collect();
    for id in store.record_ids() {
        if !kept.contains(id.as_str()) {
            std::fs::remove_file(store.record_path(&id)).unwrap();
        }
    }

    let settings = AssetSettings {
        store: Some(store.clone()),
        use_install: false,
        service_first: false,
        builds: [(GAME.to_string(), build.to_string())].into(),
        remote: None,
        lock: None,
    };
    // What the install itself lacks (materials no file has) isn't the store's.
    let mut from_install = MaterialLoader::guest(&source, 0, 0).lean(true);
    let surfaces = SurfaceProps::load(&mut from_install);
    let (mut all, mut ignored) = (GameKit::default(), Vec::new());
    bodies::add_bodies(&mut from_install, &surfaces, KitLoad::Players(&["tf2:"]), &mut all, &mut ignored);
    assert_eq!(all.characters.len(), 9, "the install has every class");
    let served = served::open_with(&settings, GAME, Path::new("no-install"), mount::open_install).unwrap();
    let mut materials = MaterialLoader::guest(&served, 0, 0).lean(true);
    let surfaces = SurfaceProps::load(&mut materials);
    let (mut kit, mut warnings) = (GameKit::default(), Vec::new());
    bodies::add_bodies(&mut materials, &surfaces, KitLoad::Players(&["tf2:"]), &mut kit, &mut warnings);
    warnings.extend(materials.missing.iter().filter(|m| !from_install.missing.contains(m)).cloned());
    assert!(warnings.is_empty(), "nothing missing: {warnings:?}");
    let mut names: Vec<String> = kit.characters.iter().filter_map(|c| c.name.clone()).collect();
    names.sort();
    assert_eq!(names, ["models/player/heavy.mdl", "models/player/scout.mdl"]);
    let icons = hud::kill_icon_pictures(&served).expect("the kill icons' files are in players");
    assert!(icons.len() >= 10, "{} kill icons", icons.len());
    let _ = std::fs::remove_dir_all(&root);
}
