//! `chimera_assets sign <game> --store <store>`: signing one game's manifest
//! in a server's store with the store's key, as the asset server's owner
//! does after copying a manifest packed elsewhere into it, run as the
//! built binary on scratch stores.

use std::{fs, path::Path, process::Command};

use chimera_assets::{Manifest, Store, server::key_path, sign};

use crate::scratch;

fn run(store: &Path, args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_chimera_assets"))
        .arg("--store")
        .arg(store)
        .args(args)
        .output()
        .expect("the binary runs");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn put(store: &Store, game: &str, build: &str) {
    Manifest::new(game, build, None).save(&store.manifest_path(game, build)).unwrap();
}

/// A store with a key signs the named game's newest manifest, verifiably
/// with the client-side check; another game's manifest and signature are
/// byte for byte what they were, and an older build stays unsigned.
#[test]
fn signs_one_games_newest_manifest_and_nothing_else() {
    let dir = scratch("sign_cli");
    let store = Store::new(&dir);
    put(&store, "halo_ce", "01.10.12.2276");
    put(&store, "halo_ce", "01.01.14.2342");
    put(&store, "other", "7");
    // Another game's, already signed by the same key.
    let key = sign::load_or_create_key(&key_path(&store)).unwrap();
    let other = store.manifest_path("other", "7");
    sign::sign_manifest_file(&key, &other).unwrap();
    let (other_manifest, other_sig) = (fs::read(&other).unwrap(), fs::read(sign::sig_path(&other)).unwrap());
    let other_key = fs::read(key_path(&store)).unwrap();

    let (ok, out, err) = run(&dir, &["sign", "halo_ce"]);
    assert!(ok, "{out}{err}");
    let path = store.manifest_path("halo_ce", "01.10.12.2276");
    assert!(out.contains("signed halo_ce build 01.10.12.2276"), "{out}");
    assert!(out.contains(&chimera_assets::store::sha256_hex(&fs::read(&path).unwrap())), "manifest id: {out}");
    let signature = fs::read_to_string(sign::sig_path(&path)).unwrap();
    sign::verify(&sign::public_hex(&key), &fs::read(&path).unwrap(), &signature).unwrap();
    assert!(!sign::sig_path(&store.manifest_path("halo_ce", "01.01.14.2342")).exists(), "the older build");
    assert_eq!(fs::read(&other).unwrap(), other_manifest);
    assert_eq!(fs::read(sign::sig_path(&other)).unwrap(), other_sig);
    assert_eq!(fs::read(key_path(&store)).unwrap(), other_key, "the key is untouched");

    // A named build.
    let (ok, out, err) = run(&dir, &["sign", "halo_ce", "--build", "01.01.14.2342"]);
    assert!(ok && out.contains("build 01.01.14.2342"), "{out}{err}");
    assert!(sign::sig_path(&store.manifest_path("halo_ce", "01.01.14.2342")).exists());
    let _ = fs::remove_dir_all(&dir);
}

/// A store without the server's key is refused and no key is made; an
/// unknown game or build is refused, with what is wrong.
#[test]
fn refuses_a_store_without_a_key_and_an_unknown_game() {
    let dir = scratch("sign_cli_refuse");
    let store = Store::new(&dir);
    put(&store, "halo_ce", "1");
    let (ok, _, err) = run(&dir, &["sign", "halo_ce"]);
    assert!(!ok && err.contains("no signing key"), "{err}");
    assert!(!key_path(&store).exists(), "no key created");
    assert!(!sign::sig_path(&store.manifest_path("halo_ce", "1")).exists());

    sign::load_or_create_key(&key_path(&store)).unwrap();
    let (ok, _, err) = run(&dir, &["sign", "absent_game"]);
    assert!(!ok && err.contains("no manifest of absent_game"), "{err}");
    let (ok, _, err) = run(&dir, &["sign", "halo_ce", "--build", "2"]);
    assert!(!ok && err.contains("no such manifest"), "{err}");
    let _ = fs::remove_dir_all(&dir);
}
