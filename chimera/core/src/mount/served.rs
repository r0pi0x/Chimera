//! A game's mount as the asset settings want it (docs/plans/active/
//! asset-service.md, M1): each game's `mount::open` goes through `open`
//! here, so every loader, test and tool reads the same way. With the
//! install in use (`[assets] use_install`, default true) the install's
//! layers come first and the store's `CachedFiles` after them, so the
//! store never changes what the install provides: it only adds files the
//! install lacks (listings are every layer's, merged). The store is
//! layered under an install only at the install's own build (its Steam
//! app manifest; `[assets] builds` must name that build if it names
//! one): a store of another build, or an install whose build isn't known,
//! gets the install alone. With `use_install = false`, or when the path is
//! the store's stand-in for a missing install
//! (`LocalConfig::game_path_in`), the store alone (`alone`), at the
//! pinned build or the newest: then the mount holds what that build's
//! manifest lists: every file the game mounts when it was published whole
//! (`complete`), else only the sets packed so far (`partial`). `usable`
//! is what a test or tool asks before reading a game's files: an install
//! with the game's folder, or the store standing in for it. `CHIMERA_VFS_LOG=<file>` wraps every layer
//! in a `Recording`: each file read is logged with its game, size and
//! hash (`chimera_assets::recording`), which is how `chimera_pack` gets a
//! set. With an asset service set (`[assets] url` and `token`), a
//! manifest the store lacks is fetched from it when the store serves a
//! game alone (`AssetSettings::build`), and every `CachedFiles` fetches
//! the records the store lacks on demand (`AssetSettings::fetcher`).

use std::{io, path::Path};

use chimera_assets::{AssetSettings, CachedFiles, Recorder, Recording};

use super::{FileSource, Mount, config::LocalConfig, install::install_build_id};

/// `game`'s mount: `open_install(install)` and/or the asset store, per
/// the local config; recorded when `CHIMERA_VFS_LOG` is set.
pub fn open(game: &str, install: &Path, open_install: fn(&Path) -> io::Result<Mount>) -> io::Result<Mount> {
    let mut mount = open_with(&settings()?, game, install, open_install)?;
    if let Some(recorder) = Recorder::from_env() {
        mount = record(mount, game, &recorder);
    }
    Ok(mount)
}

/// Whether `game`'s mount at `install` is the asset store alone (the
/// install off, or `install` the store's stand-in for a missing one):
/// only what has been packed, so a check that wants a whole install
/// (every map, every weapon) has nothing to check it against.
pub fn alone(game: &str, install: &Path) -> bool {
    settings().is_ok_and(|a| alone_with(&a, game, install))
}

/// Whether `game`'s mount at `install` is the asset store alone holding
/// only some sets (its build's manifest isn't of a whole install): a check
/// that wants a whole install (every map, every weapon) has nothing to
/// check it against. A whole publish (`complete`) stands in for the
/// install: files it lacks locally are fetched on demand.
pub fn partial(game: &str, install: &Path) -> bool {
    settings().is_ok_and(|a| alone_with(&a, game, install) && !complete(&a, game))
}

/// Whether `install` is somewhere `game`'s files can be read through its
/// mount: an install holding `marker` (the game's folder, `cstrike` or
/// `Game`), or the asset store's stand-in for a missing install (the store
/// or the asset service serves the game). Tests and tools that need an
/// install ask this, so they run from the service as they do from an
/// install; one that reads the install's own files outside the mount (an
/// archive by its file name) asks for the marker alone.
pub fn usable(game: &str, install: impl AsRef<Path>, marker: &str) -> bool {
    let install = install.as_ref();
    install.join(marker).exists() || alone(game, install)
}

/// Whether the build `game` is served at lists every file it mounts.
fn complete(assets: &AssetSettings, game: &str) -> bool {
    let (Some(store), Some(build)) = (assets.store.as_ref(), assets.build(game)) else {
        return false;
    };
    CachedFiles::open(store, game, &build).is_ok_and(|c| c.index().complete)
}

fn settings() -> io::Result<AssetSettings> {
    let config = LocalConfig::load().map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(config.assets.resolve())
}

fn alone_with(assets: &AssetSettings, game: &str, install: &Path) -> bool {
    !assets.use_install || assets.stand_in_install(game).is_some_and(|p| p == install)
}

/// `open` with these settings, unrecorded.
pub fn open_with(
    assets: &AssetSettings,
    game: &str,
    install: &Path,
    open_install: fn(&Path) -> io::Result<Mount>,
) -> io::Result<Mount> {
    let cached = |build: &str| -> io::Result<Option<CachedFiles>> {
        let Some(store) = assets.store.as_ref() else {
            return Ok(None);
        };
        if !store.manifest_path(game, build).is_file() {
            return Ok(None);
        }
        CachedFiles::open(store, game, build)
            .map(|c| Some(c.with_fetcher(assets.fetcher())))
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    };
    if alone_with(assets, game, install) {
        let cached = match assets.build(game) {
            Some(build) => cached(&build)?,
            None => None,
        };
        let cached = cached.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "{game}: the install is off (use_install = false) and the asset store ({}) has no manifest for it",
                    assets.store.as_ref().map_or("none".into(), |s| s.root.display().to_string())
                ),
            )
        })?;
        let mut mount = Mount::default();
        mount.push(cached);
        return Ok(mount);
    }
    let mut mount = open_install(install)?;
    if let Some(cached) = under_install(assets, game, install, &cached)? {
        mount.push(cached);
    }
    Ok(mount)
}

/// The store's files of the build `install` is, if the store has that
/// build (and `[assets] builds` doesn't pin another).
fn under_install(
    assets: &AssetSettings,
    game: &str,
    install: &Path,
    cached: &dyn Fn(&str) -> io::Result<Option<CachedFiles>>,
) -> io::Result<Option<CachedFiles>> {
    let Some(store) = assets.store.as_ref() else {
        return Ok(None);
    };
    // The Steam app is in the store's manifests (every build of a game is
    // of the same app).
    let Some(newest) = store.latest_build(game) else {
        return Ok(None);
    };
    let Some(app) = cached(&newest)?.and_then(|c| c.index().app) else {
        return Ok(None);
    };
    let Some(build) = install_build_id(install, app) else {
        return Ok(None);
    };
    if assets.builds.get(game).is_some_and(|pinned| *pinned != build) {
        return Ok(None);
    }
    cached(&build)
}

/// `mount` with every layer's reads recorded under `game`.
pub fn record(mount: Mount, game: &str, recorder: &std::sync::Arc<Recorder>) -> Mount {
    mount.map_layers(|l| Box::new(Recording::new(l, game, recorder.clone())) as Box<dyn FileSource>)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use chimera_assets::{FileRecord, Manifest, Store};

    use super::*;
    use crate::mount::LooseDir;

    const GAME: &str = "test_game";
    const APP: u32 = 4242;

    /// A scratch folder, removed when dropped.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let p = std::env::temp_dir().join(format!("chimera-served-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write(path: &Path, bytes: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    /// A manifest of `build` in `store` holding `files` (path, bytes).
    fn pack(store: &Store, build: &str, files: &[(&str, &[u8])]) {
        let mut m = Manifest::new(GAME, build, Some(APP));
        for (path, bytes) in files {
            let put = store.put(bytes).unwrap();
            m.files.insert(path.to_string(), FileRecord { id: put.id, size: bytes.len() as u64, stored: put.stored });
        }
        m.save(&store.manifest_path(GAME, build)).unwrap();
    }

    /// An install of loose files (`maps/install.dat`, `shared.txt`) in
    /// `dir`, its Steam app manifest saying `build` (None: no manifest).
    fn install(dir: &Path, build: Option<&str>) -> PathBuf {
        let root = dir.join("install");
        write(&root.join("maps/install.dat"), b"install map");
        write(&root.join("shared.txt"), b"the install's");
        if let Some(build) = build {
            let acf = format!(
                "\"AppState\"\n{{\n\t\"appid\"\t\t\"{APP}\"\n\t\"StateFlags\"\t\t\"4\"\n\t\"installdir\"\t\t\"x\"\n\t\"buildid\"\t\t\"{build}\"\n}}\n"
            );
            write(&root.join(format!("steamapps/appmanifest_{APP}.acf")), acf.as_bytes());
        }
        root
    }

    fn open_loose(path: &Path) -> io::Result<Mount> {
        let mut m = Mount::default();
        m.push(LooseDir::new(path));
        Ok(m)
    }

    fn settings(store: &Store, use_install: bool, pinned: Option<&str>) -> AssetSettings {
        AssetSettings {
            store: Some(store.clone()),
            use_install,
            builds: pinned.map(|b| (GAME.to_string(), b.to_string())).into_iter().collect(),
            remote: None,
            lock: None,
        }
    }

    /// A store of two builds: the install's (100: a map the install lacks
    /// and its own copy of a file the install has) and a newer one.
    fn two_builds(s: &Scratch) -> Store {
        let store = Store::new(s.0.join("store"));
        pack(&store, "100", &[("maps/served.dat", b"served map"), ("shared.txt", b"the store's")]);
        pack(&store, "200", &[("maps/newer.dat", b"newer build's map")]);
        store
    }

    fn sorted(mut v: Vec<String>) -> Vec<String> {
        v.sort();
        v
    }

    #[test]
    fn the_install_comes_first_and_the_store_adds_only_what_it_lacks_at_its_build() {
        let s = Scratch::new("same-build");
        let store = two_builds(&s);
        let root = install(&s.0, Some("100"));
        let mount = open_with(&settings(&store, true, None), GAME, &root, open_loose).unwrap();
        assert_eq!(mount.read("shared.txt").unwrap(), b"the install's");
        assert_eq!(mount.read("maps/served.dat").unwrap(), b"served map");
        assert!(!mount.contains("maps/newer.dat"), "the newest build isn't the install's");
        assert_eq!(sorted(mount.files_in("maps")), ["maps/install.dat", "maps/served.dat"]);
    }

    #[test]
    fn a_store_of_another_build_or_an_unknown_one_is_not_layered_under_the_install() {
        let s = Scratch::new("other-build");
        let store = two_builds(&s);
        let cases = [(Some("300"), None), (None, None), (Some("100"), Some("200"))];
        for (k, (build, pinned)) in cases.into_iter().enumerate() {
            let root = install(&s.0.join(k.to_string()), build);
            let mount = open_with(&settings(&store, true, pinned), GAME, &root, open_loose).unwrap();
            let case = format!("install build {build:?}, pinned {pinned:?}");
            assert_eq!(mount.layers().count(), 1, "{case}");
            assert_eq!(mount.files_in("maps"), ["maps/install.dat"], "{case}");
            assert_eq!(mount.read("shared.txt").unwrap(), b"the install's", "{case}");
        }
    }

    #[test]
    fn alone_the_store_serves_its_newest_or_pinned_build() {
        let s = Scratch::new("alone");
        let store = two_builds(&s);
        let root = install(&s.0, Some("100"));
        let off = settings(&store, false, None);
        assert!(alone_with(&off, GAME, &root));
        assert_eq!(open_with(&off, GAME, &root, open_loose).unwrap().files_in("maps"), ["maps/newer.dat"]);
        let pinned = settings(&store, false, Some("100"));
        assert_eq!(open_with(&pinned, GAME, &root, open_loose).unwrap().read("shared.txt").unwrap(), b"the store's");
        // The stand-in for a missing install is the store alone too.
        let on = settings(&store, true, None);
        let stand_in = on.stand_in_install(GAME).unwrap();
        assert!(alone_with(&on, GAME, &stand_in) && !alone_with(&on, GAME, &root));
        assert!(open_with(&on, GAME, &stand_in, open_loose).unwrap().contains("maps/newer.dat"));
    }

    #[test]
    fn a_whole_publish_stands_in_for_the_install_and_a_partial_one_does_not() {
        let s = Scratch::new("complete");
        let store = two_builds(&s);
        let off = settings(&store, false, None);
        assert!(!complete(&off, GAME), "sets only");
        let path = store.manifest_path(GAME, "200");
        let mut m = Manifest::load(&path).unwrap();
        m.complete = true;
        m.save(&path).unwrap();
        assert!(complete(&off, GAME), "a whole install");
        // What tests ask before reading a game's files.
        let root = install(&s.0, Some("100"));
        let on = settings(&store, true, None);
        assert!(root.join("maps").is_dir());
        assert!(!usable(GAME, &root, "nothing_here"), "an install without the game's folder");
        assert!(alone_with(&on, GAME, &on.stand_in_install(GAME).unwrap()));
    }
}
