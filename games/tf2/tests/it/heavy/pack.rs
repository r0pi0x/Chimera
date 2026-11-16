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
