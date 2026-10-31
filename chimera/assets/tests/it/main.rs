//! The store, manifests, `CachedFiles`, the packer and the recorder on our
//! own fixture bytes (no game's files).

mod service;

use std::{fs, path::PathBuf, sync::Arc};

use chimera_assets::{CachedFiles, Manifest, Recorder, Recording, Store, pack, read_log, store::sha256_hex};
use chimera_vfs::{FileSource, LooseDir, Mount};

/// A fresh folder for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("chimera_assets_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A loose "install": two layers, the first shadowing one file of the
/// second, and two files with the same bytes.
fn fixture(root: &PathBuf) -> Mount {
    let write = |rel: &str, bytes: &[u8]| {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, bytes).unwrap();
    };
    write("a/Maps/Test.bsp", &[7u8; 5000]);
    write("a/sound/one.wav", b"same bytes");
    write("a/sound/two.wav", b"same bytes");
    write("a/cfg/game.txt", b"from the first layer");
    write("b/cfg/game.txt", b"shadowed");
    write("b/materials/x.vmt", b"\"LightmappedGeneric\" {}");
    let mut mount = Mount::default();
    mount.push(LooseDir::new(root.join("a")));
    mount.push(LooseDir::new(root.join("b")));
    mount
}

fn all_paths(mount: &Mount) -> Vec<String> {
    mount.entries().into_iter().map(|(e, _)| e.path).collect()
}

fn store_files(root: &PathBuf) -> Vec<(String, Vec<u8>)> {
    fn walk(dir: &std::path::Path, root: &std::path::Path, out: &mut Vec<(String, Vec<u8>)>) {
        for e in fs::read_dir(dir).unwrap().flatten() {
            if e.path().is_dir() {
                walk(&e.path(), root, out);
            } else {
                let rel = e.path().strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
                out.push((rel, fs::read(e.path()).unwrap()));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

#[test]
fn packing_is_deterministic_and_dedups_by_hash() {
    let dir = scratch("determinism");
    let mount = fixture(&dir.join("install"));
    let paths = all_paths(&mount);
    let mut outputs = Vec::new();
    for (k, threads) in [1, 4].into_iter().enumerate() {
        let store = Store::new(dir.join(format!("store{k}")));
        let mut m = Manifest::new("fixture", "100", Some(1));
        let report = pack(&mount, &paths, &store, &mut m, Some("all"), threads).unwrap();
        assert_eq!(report.files, 5);
        assert_eq!(report.records, 4, "the two equal sounds are one record");
        assert_eq!(report.new_records, 4);
        m.save(&store.manifest_path("fixture", "100")).unwrap();
        // Packing again writes nothing new.
        let mut again = Manifest::new("fixture", "100", Some(1));
        let second = pack(&mount, &paths, &store, &mut again, Some("all"), threads).unwrap();
        assert_eq!(second.new_records, 0);
        assert_eq!(again, m);
        outputs.push(store_files(&store.root));
    }
    assert_eq!(outputs[0], outputs[1], "same store bytes whatever the thread count");
    let manifest =
        String::from_utf8(outputs[0].iter().find(|(p, _)| p.ends_with("100.txt")).unwrap().1.clone()).unwrap();
    assert!(manifest.contains(&format!("file {} 20 ", sha256_hex(b"from the first layer"))), "the shadowing file wins");
    assert_eq!(outputs[0].iter().filter(|(p, _)| p.starts_with("records/")).count(), 4);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_manifest_round_trips() {
    let mut m = Manifest::new("cs_source", "25338318", Some(240));
    m.files.insert("maps/de x.bsp".into(), chimera_assets::FileRecord { id: sha256_hex(b"x"), size: 1, stored: 10 });
    m.files.insert("a/b.txt".into(), chimera_assets::FileRecord { id: sha256_hex(b"y"), size: 2, stored: 11 });
    m.sets.entry("de_x".into()).or_default().insert("maps/de x.bsp".into());
    let text = m.to_text();
    assert_eq!(Manifest::parse(&text).unwrap(), m);
    assert_eq!(Manifest::parse(&text).unwrap().to_text(), text);
    assert!(text.lines().nth(4).unwrap().ends_with(" a/b.txt"), "sorted by path:\n{text}");
    // A set naming a file the manifest lacks is refused.
    let broken = text.replace("set de_x maps/de x.bsp", "set de_x maps/none.bsp");
    assert!(Manifest::parse(&broken).is_err());
    assert!(Manifest::parse("chimera-manifest 2\n").is_err());
    // A whole install's manifest says so.
    m.complete = true;
    let whole = m.to_text();
    assert!(whole.contains("\ncomplete 1\n"));
    assert!(Manifest::parse(&whole).unwrap().complete && !Manifest::parse(&text).unwrap().complete);
}

/// A missing file's on-demand group: the smallest set holding it, from it
/// on, without what the store has; else the files beside it of the same
/// name before the first dot.
#[test]
fn a_miss_fetches_its_group() {
    let dir = scratch("group");
    let store = Store::new(&dir);
    let mut m = Manifest::new("g", "1", None);
    let mut add = |path: &str, bytes: &[u8]| {
        let put = store.put(bytes).unwrap();
        m.files.insert(path.into(), chimera_assets::FileRecord { id: put.id, size: bytes.len() as u64, stored: put.stored });
    };
    for (i, p) in ["maps/a.bsp", "materials/a1.vtf", "materials/a2.vtf", "materials/big.vtf"].iter().enumerate() {
        add(p, format!("map a {i}").as_bytes());
    }
    add("models/gun.mdl", b"gun mdl");
    add("models/gun.vvd", b"gun vvd");
    add("models/gun.dx90.vtx", b"gun vtx");
    add("models/gunner.mdl", b"another model");
    let set = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<std::collections::BTreeSet<_>>();
    m.sets.insert("map/a".into(), set(&["maps/a.bsp", "materials/a1.vtf", "materials/a2.vtf"]));
    m.sets.insert("all".into(), m.files.keys().cloned().collect());
    // The client's store holds only a2.
    let client = Store::new(dir.join("client"));
    client.put(b"map a 2").unwrap();
    let index = chimera_assets::cached::Index::new(&m);
    let ids = |paths: &[&str]| paths.iter().map(|p| m.files[*p].id.clone()).collect::<Vec<_>>();
    let group = |path: &str| index.group(&client, path).into_iter().map(|g| g.0).collect::<Vec<_>>();
    assert_eq!(group("materials/a1.vtf"), ids(&["materials/a1.vtf", "maps/a.bsp"]), "the map's set, from a1 on, wrapping");
    let mut no_set = m.clone();
    no_set.sets.clear();
    let index = chimera_assets::cached::Index::new(&no_set);
    let group: Vec<String> = index.group(&client, "models/gun.mdl").into_iter().map(|g| g.0).collect();
    assert_eq!(group, ids(&["models/gun.mdl", "models/gun.dx90.vtx", "models/gun.vvd"]), "its companions, not gunner");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn cached_files_read_the_store_and_refuse_a_corrupt_record() {
    let dir = scratch("cached");
    let mount = fixture(&dir.join("install"));
    let store = Store::new(dir.join("store"));
    let mut m = Manifest::new("fixture", "7", None);
    pack(&mount, &all_paths(&mount), &store, &mut m, None, 2).unwrap();
    m.save(&store.manifest_path("fixture", "7")).unwrap();
    let cached = CachedFiles::open(&store, "fixture", "7").unwrap();
    let mut served = Mount::default();
    served.push(cached);
    // Same files, same bytes, same listing as the install.
    for path in all_paths(&mount) {
        assert_eq!(served.read(&path).unwrap(), mount.read(&path).unwrap(), "{path}");
    }
    assert_eq!(served.read("CFG\\Game.txt").unwrap(), b"from the first layer");
    assert_eq!(served.files_in("sound"), vec!["sound/one.wav", "sound/two.wav"]);
    assert!(served.contains("maps/test.bsp") && !served.contains("maps/other.bsp"));
    assert_eq!(served.read("maps/other.bsp").unwrap_err().kind(), std::io::ErrorKind::NotFound);
    // A record changed on disk: refused, not handed on.
    let id = &m.files["materials/x.vmt"].id;
    let record = store.record_path(id);
    fs::write(&record, zstd_of(b"\"UnlitGeneric\" {}...")).unwrap();
    let e = served.read("materials/x.vmt").unwrap_err();
    assert_eq!(e.kind(), std::io::ErrorKind::InvalidData, "{e}");
    fs::write(&record, b"not zstd at all").unwrap();
    assert_eq!(served.read("materials/x.vmt").unwrap_err().kind(), std::io::ErrorKind::InvalidData);
    // A record gone: a miss naming the file.
    fs::remove_file(&record).unwrap();
    let e = served.read("materials/x.vmt").unwrap_err();
    assert_eq!(e.kind(), std::io::ErrorKind::NotFound);
    assert!(e.to_string().contains("materials/x.vmt"), "{e}");
    let _ = fs::remove_dir_all(&dir);
}

/// Any zstd frame (a wrong record's bytes, packed as a record would be).
fn zstd_of(bytes: &[u8]) -> Vec<u8> {
    let dir = scratch("zstd_of");
    let store = Store::new(&dir);
    let put = store.put(bytes).unwrap();
    let out = fs::read(store.record_path(&put.id)).unwrap();
    let _ = fs::remove_dir_all(&dir);
    out
}

#[test]
fn recording_logs_each_file_read_once() {
    let dir = scratch("recording");
    let log = dir.join("vfs.log");
    let recorder = Recorder::to_file(&log).unwrap();
    let mount = fixture(&dir.join("install"))
        .map_layers(|l| Box::new(Recording::new(l, "fixture", recorder.clone())) as Box<dyn FileSource>);
    mount.read("sound/one.wav").unwrap();
    mount.read("Sound/One.wav").unwrap();
    assert!(mount.contains("cfg/game.txt"));
    assert!(!mount.contains("cfg/none.txt"));
    let lines = read_log(&log).unwrap();
    let got: Vec<(&str, u64)> = lines.iter().map(|l| (l.path.as_str(), l.size)).collect();
    assert_eq!(got, [("sound/one.wav", 10), ("cfg/game.txt", 20)]);
    assert_eq!(lines[0].id, sha256_hex(b"same bytes"));
    assert_eq!(lines[1].game, "fixture");
    assert_eq!(recorder.lines().len(), 2);
    // The log is a set the packer takes.
    let store = Store::new(dir.join("store"));
    let mut m = Manifest::new("fixture", "1", None);
    let paths: Vec<String> = lines.iter().map(|l| l.path.clone()).collect();
    let report = pack(&fixture(&dir.join("install")), &paths, &store, &mut m, Some("test"), 1).unwrap();
    assert_eq!((report.files, m.sets["test"].len()), (2, 2));
    let _: Arc<Recorder> = recorder;
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn settings_take_the_environment_over_the_config() {
    use chimera_assets::AssetsConfig;
    let config = AssetsConfig { store: Some("/x/store".into()), use_install: Some(false), ..Default::default() };
    let s = config.resolve_with(None, None);
    assert_eq!(s.store.unwrap().root, PathBuf::from("/x/store"));
    assert!(!s.use_install);
    let s = config.resolve_with(Some("/y".into()), Some("1".into()));
    assert_eq!(s.store.unwrap().root, PathBuf::from("/y"));
    assert!(s.use_install);
    let s = AssetsConfig::default().resolve_with(None, None);
    assert!(s.use_install, "the install is used by default");
    assert!(s.store.unwrap().root.ends_with("chimera/assets"), "the user-wide cache");
}
