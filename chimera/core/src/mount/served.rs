//! A game's mount as the asset settings want it (docs/plans/active/
//! asset-service.md, M1): each game's `mount::open` goes through `open`
//! here, so every loader, test and tool reads the same way. Which path a
//! game opens is `LocalConfig::game_path_in`'s (`AssetSettings::game_path`):
//! with the service first (the game apps' default whenever an asset
//! service is set, `chimera_assets::config::InstallDefault`; the player's
//! "Use my installed games", `[assets] use_install = true` or
//! `CHIMERA_ASSET_USE_INSTALL=1` turn it off) a game the store and its
//! service serve is the store's stand-in, so it is read from the store
//! alone even when installed (`alone`). With the install in use (tests'
//! and tools' default, an app's with no service or with the install
//! chosen) the install's layers come first and the store's `CachedFiles` after them, so the
//! store never changes what the install provides: it only adds files the
//! install lacks (listings are every layer's, merged). The store is
//! layered under an install only at the install's own build (its Steam
//! app manifest, or for a game without a Steam app its own reading of
//! the install, `open_built`; `[assets] builds` must name that build if
//! it names one): a store of another build, or an install whose build
//! isn't known, gets the install alone. With `use_install = false`, or when the path is
//! the store's stand-in for a missing install
//! (`LocalConfig::game_path_in`), the store alone (`alone`), at the
//! pinned build or the newest: then the mount holds what that build's
//! manifest lists: every file the game mounts when it was published whole
//! (`complete`), else only the sets packed so far (`partial`). `usable`
//! is what a test or tool asks before reading a game's files: an install
//! with the game's folder, or the store standing in for it (a whole
//! publish only with a service to fetch from); `sets_held`
//! and `map_held` whether the store holds whole sets (a map's) with no
//! network (what a load may read without downloading). `CHIMERA_VFS_LOG=<file>` wraps every layer
//! in a `Recording`: each file read is logged with its game, size and
//! hash (`chimera_assets::recording`), which is how `chimera_pack` gets a
//! set. With an asset service set (`[assets] url` and `token`), a
//! manifest the store lacks is fetched from it when the store serves a
//! game alone (`AssetSettings::build`), and every `CachedFiles` fetches
//! the records the store lacks on demand (`AssetSettings::fetcher`; with
//! no service set, from the one a join binds later in the process:
//! `chimera_assets::access::late_fetcher`).

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

/// `open` for a game without a Steam app that reads its install's build
/// itself (`build`: Halo's from its maps' headers): the store's files of
/// that build are layered under the install as a Steam game's are (its
/// generated sets, which no install holds).
pub fn open_built(
    game: &str,
    install: &Path,
    open_install: fn(&Path) -> io::Result<Mount>,
    build: fn(&Path) -> io::Result<String>,
) -> io::Result<Mount> {
    let mut mount = open_in(&settings()?, game, install, open_install, Some(build))?;
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
/// archive by its file name) asks for the marker alone. An empty `marker`
/// means the install path itself: a game whose install is one file (a disc
/// image) or whose folder has no marker. The store stands in only where
/// it can give what it lists (`readable_alone`): a whole publish with no
/// service to fetch from (no install found, no `[assets] url`, no bound
/// access) holds little more than its manifest, so a test or tool skips
/// rather than failing on every file.
pub fn usable(game: &str, install: impl AsRef<Path>, marker: &str) -> bool {
    let install = install.as_ref();
    holds(install, marker) || settings().is_ok_and(|a| alone_with(&a, game, install) && readable_alone(&a, game))
}

/// Whether the store alone can give `game`'s files: a service is set
/// (files it lacks are fetched on demand), or the build is only some sets
/// (`partial`: the ones packed here, which a check of the whole install
/// skips by `partial` anyway).
fn readable_alone(assets: &AssetSettings, game: &str) -> bool {
    assets.fetcher().is_some() || !complete(assets, game)
}

/// Whether `install` holds `marker` (an empty one: `install` itself exists).
fn holds(install: &Path, marker: &str) -> bool {
    if marker.is_empty() {
        install.exists()
    } else {
        install.join(marker).exists()
    }
}

/// Whether the asset store alone, with no network, holds every record of
/// `game`'s `sets` (`chimera_assets::sets::held`): what may be read from
/// it without a download.
pub fn sets_held(game: &str, sets: &[&str]) -> bool {
    settings().is_ok_and(|a| chimera_assets::sets::held(&a, game, sets))
}

/// Whether the asset store alone, with no network, holds map `name` of
/// `game` whole (`chimera_assets::sets::map_held`: its set and the game's
/// `base`).
pub fn map_held(game: &str, name: &str) -> bool {
    settings().is_ok_and(|a| chimera_assets::sets::map_held(&a, game, name))
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
    open_in(assets, game, install, open_install, None)
}

/// `open_with`, the install's build read by `build` when the game has no
/// Steam app (`open_built`).
fn open_in(
    assets: &AssetSettings,
    game: &str,
    install: &Path,
    open_install: fn(&Path) -> io::Result<Mount>,
    build: Option<fn(&Path) -> io::Result<String>>,
) -> io::Result<Mount> {
    let cached = |build: &str| -> io::Result<Option<CachedFiles>> {
        let Some(store) = assets.store.as_ref() else {
            return Ok(None);
        };
        if !store.manifest_path(game, build).is_file() {
            return Ok(None);
        }
        CachedFiles::open(store, game, build)
            // No service yet: the one a join binds later (`late_fetcher`).
            .map(|c| Some(c.with_fetcher(assets.fetcher().or_else(|| Some(chimera_assets::access::late_fetcher())))))
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
    if let Some(cached) = under_install(assets, game, install, &cached, build)? {
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
    own_build: Option<fn(&Path) -> io::Result<String>>,
) -> io::Result<Option<CachedFiles>> {
    let Some(store) = assets.store.as_ref() else {
        return Ok(None);
    };
    // A game that reads its own build (no Steam app).
    if let Some(read) = own_build {
        let Ok(build) = read(install) else { return Ok(None) };
        if assets.builds.get(game).is_some_and(|pinned| *pinned != build) {
            return Ok(None);
        }
        return cached(&build);
    }
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
            service_first: false,
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
    fn an_empty_marker_is_the_install_path_itself_file_or_folder() {
        let s = Scratch::new("marker");
        let file = s.0.join("disc.iso");
        write(&file, b"image");
        let dir = s.0.join("folder");
        write(&dir.join("default.xbe"), b"x");
        // A single-file install has no folder to join a marker to.
        assert!(holds(&file, ""));
        assert!(holds(&dir, ""));
        assert!(!holds(&s.0.join("absent.iso"), ""));
        assert!(usable(GAME, &file, ""));
        // A marker still names something inside a folder, as before.
        assert!(holds(&dir, "default.xbe"));
        assert!(!holds(&dir, "other.xbe"));
        assert!(!holds(&file, "default.xbe"));
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

    /// A game without a Steam app that reads its own build: its store of
    /// that build under the install, none of another.
    #[test]
    fn a_game_reading_its_own_build_gets_the_store_of_that_build_under_its_install() {
        let s = Scratch::new("own-build");
        let store = two_builds(&s);
        let root = install(&s.0, None);
        let assets = settings(&store, true, None);
        let mount = open_in(&assets, GAME, &root, open_loose, Some(|_| Ok("100".into()))).unwrap();
        assert_eq!(mount.read("shared.txt").unwrap(), b"the install's");
        assert_eq!(mount.read("maps/served.dat").unwrap(), b"served map");
        let other = open_in(&assets, GAME, &root, open_loose, Some(|_| Ok("300".into()))).unwrap();
        assert_eq!(other.layers().count(), 1);
        let unread = open_in(&assets, GAME, &root, open_loose, Some(|_| Err(io::Error::other("no map")))).unwrap();
        assert_eq!(unread.layers().count(), 1);
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

    /// The game apps' default with a service set: a client with an install
    /// and a store (with a service) reads a game's file from the store, not
    /// the install; with the install chosen (the setting, `use_install =
    /// true`) the install's; with no service at all the install as before,
    /// and a game the service doesn't serve reads its install either way.
    #[test]
    fn with_a_service_the_store_comes_first_unless_the_install_is_chosen() {
        use chimera_assets::{
            RemoteConfig,
            config::{InstallDefault, policy},
        };
        let s = Scratch::new("service-first");
        let store = two_builds(&s);
        let root = install(&s.0, Some("100"));
        let found = || Some(root.clone());
        let read = |a: &AssetSettings, game: &str| {
            let path = a.game_path(game, found).expect("a path");
            let mount = open_with(a, game, &path, open_loose).unwrap();
            (path, mount.read("shared.txt").ok())
        };
        // Never asked: the records are in the store already.
        let remote = RemoteConfig { url: "https://127.0.0.1:9".into(), token: "t".into(), key: None, fingerprint: None };
        let with = |chosen, default, remote: Option<&RemoteConfig>, waiting| {
            let mut a = settings(&store, true, Some("100"));
            a.remote = remote.cloned();
            (a.use_install, a.service_first) = policy(chosen, default, a.remote.is_some(), waiting);
            a
        };
        let client = InstallDefault::ServiceFirstClient;
        // By default, with a service: the store's file, the install unread.
        let first = with(None, client, Some(&remote), false);
        let (path, bytes) = read(&first, GAME);
        assert_eq!(path, store.manifest_dir(GAME));
        assert_eq!(bytes.as_deref(), Some(&b"the store's"[..]));
        assert!(alone_with(&first, GAME, &path));
        // A dedicated server's default is the same.
        assert_eq!(read(&with(None, InstallDefault::ServiceFirst, Some(&remote), false), GAME).1.as_deref(), Some(&b"the store's"[..]));
        // The opt-in (the setting's `Install`, or `use_install = true` /
        // `CHIMERA_ASSET_USE_INSTALL=1` chosen): the install's.
        for opted in [with(None, InstallDefault::Install, Some(&remote), false), with(Some(true), client, Some(&remote), false)] {
            let (path, bytes) = read(&opted, GAME);
            assert_eq!(path, root);
            assert_eq!(bytes.as_deref(), Some(&b"the install's"[..]));
        }
        // No service at all: the install, as before.
        let offline = with(None, client, None, false);
        assert_eq!(read(&offline, GAME), (root.clone(), Some(b"the install's".to_vec())));
        // A game the service doesn't serve reads its install.
        assert_eq!(first.game_path("unserved", found), Some(root.clone()));
        // A client's token waiting for its first join: what the store has
        // is read from it; a game it lacks waits for the join (hidden).
        let waiting = with(None, client, None, true);
        assert_eq!(read(&waiting, GAME).1.as_deref(), Some(&b"the store's"[..]));
        assert_eq!(waiting.game_path("unserved", found), None);
        // `use_install = false` chosen: the store alone, an unserved game
        // hidden too, as before.
        assert_eq!(with(Some(false), client, Some(&remote), false).game_path("unserved", found), None);
    }
}
