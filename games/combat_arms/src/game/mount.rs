//! Combat Arms' install as a mount: every `.rez` archive in its `Game/`
//! folder, in name order (`lithtech_formats::rez::open_dir`, per
//! specs/combat_arms/rez_archive.md), with the keys from the local config
//! (`[games.combat_arms.keys.<scheme>]` in chimera.local.toml, hex `key`
//! and `iv`; never in the repo; the scheme picks the cipher: V1 Twofish,
//! V2 AES). Without a key, a keyed archive's files list but fail to read
//! with `PermissionDenied`. `open` layers the asset store under or instead
//! of the install (`mount::served`). The music the game plays from loose
//! files (`Game/BGM/`, not an archive) is `loose_music`: the packer adds it
//! under `loose/bgm/` to a whole publish, so a served Combat Arms has its
//! music too (`ui::music` reads it through the mount when there is no
//! install folder).

use std::{io, path::Path};

use lithtech_formats::rez::{self, RezCipher, RezKeys};

use crate::mount::{Entry, FileSource, LooseDir, Mount, config::LocalConfig};

/// Where `loose_music` puts the install's `Game/BGM/` in a mount.
pub const LOOSE_MUSIC: &str = "loose/bgm";

/// The keys the local config has for Combat Arms, by scheme (`V1`, `V2`).
/// Errors never include key text.
pub fn keys() -> Result<RezKeys, String> {
    let config = LocalConfig::load()?;
    let mut keys = RezKeys::new();
    for (scheme, k) in config.game_keys(super::GAME).into_iter().flatten() {
        keys.insert(
            scheme.clone(),
            RezCipher::from_hex(scheme, &k.key, &k.iv, &format!("{} {scheme}", super::GAME))?,
        );
    }
    Ok(keys)
}

/// Whether files of `scheme`'s archives (`V1`, `V2`) can be read: its key
/// is configured, or Combat Arms comes from the asset store alone (whose
/// records were decrypted by the packer: no key needed). Tests and tools
/// that need keyed files ask this.
pub fn can_read(scheme: &str) -> bool {
    keys().is_ok_and(|k| k.contains_key(scheme)) || super::INFO.served_alone()
}

/// Mount Combat Arms: its install (`open_install`) and/or the asset store
/// (decrypted records: no keys needed), as the local config's `[assets]`
/// says (`mount::served`).
pub fn open(install: &Path) -> io::Result<Mount> {
    crate::mount::served::open(super::GAME, install, open_install)
}

/// Mount a Combat Arms install (the folder holding `Game/`).
pub fn open_install(install: &Path) -> io::Result<Mount> {
    let game = install.join("Game");
    if !game.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{}: no Game/ folder; is this a Combat Arms install?", install.display()),
        ));
    }
    let keys = keys().map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(rez::open_dir(&game, &keys)?.0)
}

/// The install's loose music folder (`Game/BGM/`) as a mount layer, its
/// files under `LOOSE_MUSIC` (`loose/bgm/<path>`); None without the folder.
pub fn loose_music(install: &Path) -> Option<Box<dyn FileSource>> {
    let dir = install.join("Game").join("BGM");
    dir.is_dir().then(|| Box::new(Under { prefix: LOOSE_MUSIC, inner: LooseDir::new(dir) }) as Box<dyn FileSource>)
}

/// A layer whose files are seen under a folder.
struct Under<S> {
    prefix: &'static str,
    inner: S,
}

impl<S: FileSource> Under<S> {
    fn inner_path<'a>(&self, path: &'a str) -> Option<&'a str> {
        path.strip_prefix(self.prefix)?.strip_prefix('/')
    }
}

impl<S: FileSource> FileSource for Under<S> {
    fn name(&self) -> String {
        format!("{} (as {})", self.inner.name(), self.prefix)
    }

    fn entries(&self) -> Vec<Entry> {
        self.inner.entries().into_iter().map(|e| Entry { path: format!("{}/{}", self.prefix, e.path), size: e.size }).collect()
    }

    fn read(&self, path: &str) -> Option<io::Result<Vec<u8>>> {
        self.inner.read(self.inner_path(path)?)
    }

    fn contains(&self, path: &str) -> bool {
        self.inner_path(path).is_some_and(|p| self.inner.contains(p))
    }

    fn files_in(&self, dir: &str) -> Vec<String> {
        let inner = if dir == self.prefix { Some("") } else { self.inner_path(dir) };
        inner
            .map(|d| self.inner.files_in(d).into_iter().map(|p| format!("{}/{p}", self.prefix)).collect())
            .unwrap_or_default()
    }
}
