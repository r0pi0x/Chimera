//! The games' installs as Bevy asset sources (architecture review L4,
//! F6): `<game>://<path>` (`cs_source://materials/console/background01.vtf`,
//! `combat_arms://sfx/...`) reads `<path>` through the game's mounted
//! install (`chimera_vfs::Mount`: its search path, archives and loose
//! files, case-insensitive, either slash), read-only, so the
//! `AssetServer` loads the games' content on its own threads, once per
//! handle, freed when the last handle drops.
//!
//! An app adds `InstallSourcesPlugin` before `DefaultPlugins` (Bevy takes
//! sources only before its `AssetPlugin`), one `InstallSource` per game it
//! plays: `InstallSource::of(&INFO)` mounts the game's install
//! (`GameInfo::mount`) on the first read, or `InstallSource::shared` takes
//! a mount the game keeps for the session (Combat Arms' `world::mount`).
//! A game without an install reads nothing (every load fails as not
//! found); the mount is tried again on the next read, so an install set
//! later (the first-run dialog) is picked up. The dedicated servers never
//! add it (this crate is the presentation's).
//!
//! `texture_bc` tells texture loaders whether to keep block-compressed
//! texels (X9's `render::bc_wanted`, which needs the device: a loader is
//! made in its plugin's `finish`, as Bevy's own image loader).
//!
//! A read that fails (no mount yet, or a file the mount couldn't give) is
//! remembered (at most `UNMOUNTED_MAX` paths); `reload_unmounted` loads
//! those paths again once their files may be there (a join bound the
//! asset service and fetched the game's files: `net::assets::AssetsArrived`),
//! for the handles still held.
//!
//! Known shortcuts: a directory listing reads the whole mount's entries
//! for archives that don't override `FileSource::files_in`; there is no
//! watcher (the installs don't change while playing); a mount once opened
//! stays for the session (an install changed by `chimera_install` needs a
//! restart for assets; the menus' own reads follow it at once).

use std::{
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use bevy::{
    asset::io::{AssetReader, AssetReaderError, AssetSourceBuilder, PathStream, VecReader},
    image::CompressedImageFormatSupport,
    prelude::*,
};
use chimera_core::{game_registry::GameInfo, mount::Mount};

/// Opens a game's install as a mount (None: no install, or it doesn't
/// open).
pub type OpenMount = Arc<dyn Fn() -> Option<Arc<Mount>> + Send + Sync>;

/// One game's install as an asset source named by the game's id.
#[derive(Clone)]
pub struct InstallSource {
    /// The source's name: the game's id (`cs_source`).
    pub id: &'static str,
    open: OpenMount,
}

impl InstallSource {
    /// The game's install, mounted by its `GameInfo` on the first read.
    pub fn of(game: &'static GameInfo) -> Self {
        Self {
            id: game.id,
            open: Arc::new(move || game.mount().map(Arc::new)),
        }
    }

    /// A mount the game shares for the session (`open` is called until it
    /// gives one).
    pub fn shared(id: &'static str, open: fn() -> Option<Arc<Mount>>) -> Self {
        Self {
            id,
            open: Arc::new(open),
        }
    }

    /// Any mount (tests: a loose folder).
    pub fn with(id: &'static str, open: impl Fn() -> Option<Arc<Mount>> + Send + Sync + 'static) -> Self {
        Self { id, open: Arc::new(open) }
    }
}

/// Registers each `InstallSource` with the asset server. Add it before
/// `DefaultPlugins` (or `AssetPlugin`).
pub struct InstallSourcesPlugin(pub Vec<InstallSource>);

impl Plugin for InstallSourcesPlugin {
    fn build(&self, app: &mut App) {
        for source in &self.0 {
            let reader = MountReader {
                id: source.id,
                open: source.open.clone(),
                mount: Arc::new(RwLock::new(None)),
            };
            app.register_asset_source(
                source.id,
                AssetSourceBuilder::new(move || Box::new(reader.clone())),
            );
        }
    }
}

/// `<game>://<path>` as an asset path.
pub fn install_path(game: &str, path: &str) -> String {
    format!("{game}://{}", chimera_core::mount::normalize(path))
}

/// Reads an install's files through its mount, opened on first use.
#[derive(Clone)]
struct MountReader {
    id: &'static str,
    open: OpenMount,
    mount: Arc<RwLock<Option<Arc<Mount>>>>,
}

impl MountReader {
    /// The mount, opening it if it isn't yet.
    fn mount(&self) -> Option<Arc<Mount>> {
        if let Some(m) = self.mount.read().unwrap_or_else(|e| e.into_inner()).as_ref() {
            return Some(m.clone());
        }
        let opened = (self.open)()?;
        let mut slot = self.mount.write().unwrap_or_else(|e| e.into_inner());
        Some(slot.get_or_insert(opened).clone())
    }

    fn read_file(&self, path: &Path) -> Result<Vec<u8>, AssetReaderError> {
        let not_found = || AssetReaderError::NotFound(path.to_path_buf());
        let read = self.mount().ok_or_else(not_found).and_then(|mount| {
            mount.read(&path.to_string_lossy()).map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => not_found(),
                _ => AssetReaderError::Io(Arc::new(e)),
            })
        });
        if read.is_err() {
            let mut failed = UNMOUNTED.lock().unwrap_or_else(|e| e.into_inner());
            if failed.len() < UNMOUNTED_MAX {
                failed.insert(install_path(self.id, &path.to_string_lossy()));
            }
        }
        read
    }
}

impl AssetReader for MountReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<VecReader, AssetReaderError> {
        self.read_file(path).map(VecReader::new)
    }

    /// No `.meta` files: every asset loads with its loader's defaults (or
    /// the settings its `load_with_settings` gives).
    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<VecReader, AssetReaderError> {
        Err(AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn read_directory<'a>(&'a self, path: &'a Path) -> Result<Box<PathStream>, AssetReaderError> {
        let mount = self.mount().ok_or_else(|| AssetReaderError::NotFound(path.to_path_buf()))?;
        let files: Vec<PathBuf> = mount
            .files_in(&chimera_core::mount::normalize(&path.to_string_lossy()))
            .into_iter()
            .map(PathBuf::from)
            .collect();
        Ok(Box::new(bevy::tasks::futures_lite::stream::iter(files)))
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        let Some(mount) = self.mount() else { return Ok(false) };
        Ok(!mount.files_in(&chimera_core::mount::normalize(&path.to_string_lossy())).is_empty())
    }
}

/// The most failed reads remembered for `reload_unmounted`.
pub const UNMOUNTED_MAX: usize = 4096;

/// Asset paths (`<game>://<path>`) whose read failed, to load again
/// (`reload_unmounted`).
static UNMOUNTED: std::sync::Mutex<std::collections::BTreeSet<String>> =
    std::sync::Mutex::new(std::collections::BTreeSet::new());

/// Load again every install file whose read failed (module doc), for the
/// handles still held; the list starts again. Run once a game's files
/// arrived (a join bound the asset service: `net::assets::AssetsArrived`).
pub fn reload_unmounted(server: &AssetServer) {
    let failed = std::mem::take(&mut *UNMOUNTED.lock().unwrap_or_else(|e| e.into_inner()));
    for path in &failed {
        server.reload(path.clone());
    }
    if !failed.is_empty() {
        info!("loading again {} game files that weren't there before", failed.len());
    }
}

/// Whether texture loaders keep block-compressed texels for the GPU
/// (`render::bc_wanted`): call it from a plugin's `finish`, where the
/// renderer has said what the device takes (`CompressedImageFormatSupport`).
pub fn texture_bc(app: &App) -> bool {
    crate::render::bc_wanted(app.world().get_resource::<CompressedImageFormatSupport>())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::{AssetLoader, LoadContext, io::Reader};

    /// A file's bytes as an asset.
    #[derive(Asset, TypePath)]
    struct Bytes(Vec<u8>);

    #[derive(TypePath)]
    struct BytesLoader;

    impl AssetLoader for BytesLoader {
        type Asset = Bytes;
        type Settings = ();
        type Error = std::io::Error;

        async fn load(&self, reader: &mut dyn Reader, _: &(), _: &mut LoadContext<'_>) -> Result<Bytes, Self::Error> {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            Ok(Bytes(bytes))
        }

        fn extensions(&self) -> &[&str] {
            &["txt"]
        }
    }

    fn app(dir: PathBuf) -> App {
        let mut app = App::new();
        app.add_plugins((
            InstallSourcesPlugin(vec![InstallSource::with("a_game", move || {
                let mut mount = Mount::default();
                mount.push(chimera_core::mount::LooseDir::new(dir.clone()));
                Some(Arc::new(mount))
            })]),
            MinimalPlugins,
            AssetPlugin::default(),
        ))
        .init_asset::<Bytes>()
        .register_asset_loader(BytesLoader);
        app
    }

    fn wait(app: &mut App, handle: &Handle<Bytes>) -> bevy::asset::LoadState {
        for _ in 0..2000 {
            app.update();
            let state = app.world().resource::<AssetServer>().load_state(handle);
            if !matches!(state, bevy::asset::LoadState::Loading | bevy::asset::LoadState::NotLoaded) {
                return state;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("still loading");
    }

    /// A file in the install loads through `<game>://`, by any case and
    /// either slash; a missing one fails as not found.
    #[test]
    fn installs_load_as_asset_sources() {
        let dir = std::env::temp_dir().join(format!("chimera_install_source_{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub").join("note.txt"), b"in the install").unwrap();
        let mut app = app(dir.clone());
        let server = app.world().resource::<AssetServer>().clone();
        let handle: Handle<Bytes> = server.load(install_path("a_game", "SUB\\Note.txt"));
        assert!(wait(&mut app, &handle).is_loaded());
        assert_eq!(app.world().resource::<Assets<Bytes>>().get(&handle).unwrap().0, b"in the install");
        let missing: Handle<Bytes> = server.load(install_path("a_game", "sub/missing.txt"));
        assert!(wait(&mut app, &missing).is_failed());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn paths_are_normalized() {
        assert_eq!(install_path("cs_source", "Materials\\Console//BG.vtf"), "cs_source://materials/console/bg.vtf");
    }
}
