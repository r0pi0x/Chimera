//! Machine-local settings from `chimera.local.toml` (gitignored; its old
//! name `mashup.local.toml`, `OLD_FILE_NAME`, is read when there is no new
//! one): install paths (optional: `install` finds them otherwise),
//! Combat Arms' and TF2's keys and `[assets]` (the record store, `mount::served`). See
//! `chimera.local.example.toml`.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

pub const FILE_NAME: &str = "chimera.local.toml";

/// Its name before the rename to Chimera, still read (after `FILE_NAME`
/// in each place).
pub const OLD_FILE_NAME: &str = "mashup.local.toml";

#[derive(Debug, Default, Deserialize)]
pub struct LocalConfig {
    #[serde(default)]
    pub games: BTreeMap<String, GameConfig>,
    /// `[assets]`: the record store and whether installs are used
    /// (`chimera_assets::AssetsConfig`; `mount::served`).
    #[serde(default)]
    pub assets: chimera_assets::AssetsConfig,
    /// `[hub]`: listing this machine's servers on its account's hub
    /// (`chimera_net::net::hub::apply_config`).
    #[serde(default)]
    pub hub: chimera_assets::hub_doc::HubSettings,
    /// The file it was read from.
    #[serde(skip)]
    pub file: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
pub struct GameConfig {
    /// The install (optional: a section may hold only keys, as TF2's
    /// `[games.tf2.keys.ctx]`; the install is then found as without one).
    #[serde(default)]
    pub path: Option<PathBuf>,
    /// Decryption keys by scheme name (Combat Arms: `V1`, `V2`; TF2: `ctx`).
    #[serde(default)]
    pub keys: BTreeMap<String, KeyConfig>,
}

/// A key and IV as text (Combat Arms': hexadecimal; TF2's `ctx`: the
/// 8-character key, no IV). Never logged or echoed in errors.
#[derive(Deserialize)]
pub struct KeyConfig {
    pub key: String,
    #[serde(default)]
    pub iv: String,
}

impl std::fmt::Debug for KeyConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("KeyConfig { .. }")
    }
}

/// Decode hex text into exactly `N` bytes. Errors never include the text.
pub fn hex_bytes<const N: usize>(text: &str, what: &str) -> Result<[u8; N], String> {
    let text = text.trim();
    if text.len() != N * 2 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("{what}: expected {} hexadecimal characters", N * 2));
    }
    let mut out = [0u8; N];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16).unwrap();
    }
    Ok(out)
}

impl LocalConfig {
    /// Load from `$CHIMERA_CONFIG` (or the old `$MASHUP_CONFIG`), else
    /// `./chimera.local.toml`, else the one in the source checkout (for
    /// `cargo run`/`cargo test`); in each folder the old name
    /// (`mashup.local.toml`) when the new one isn't there. Missing file
    /// means an empty config.
    pub fn load() -> Result<Self, String> {
        let candidates = [
            crate::env::var_os("CHIMERA_CONFIG").map(PathBuf::from),
            Some(PathBuf::from(FILE_NAME)),
            Some(PathBuf::from(OLD_FILE_NAME)),
            Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(FILE_NAME)),
            Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(OLD_FILE_NAME)),
        ];
        let Some(path) = candidates.into_iter().flatten().find(|p| p.is_file()) else {
            return Ok(Self::default());
        };
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut config: Self = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        config.file = Some(fs::canonicalize(&path).unwrap_or(path));
        Ok(config)
    }

    /// Install path for `game` (e.g. `cs_source`), with `~` expanded:
    /// this file's, else the one the user saved. Steam is searched only
    /// for a game that names its Steam app (`game_path_in`;
    /// `GameInfo::install` does).
    pub fn game_path(&self, game: &str) -> Option<PathBuf> {
        self.game_path_in(game, None)
    }

    /// `game_path`, then Steam's for a game with a Steam app
    /// (`install::resolve`, which also says how it was found), as the
    /// asset settings want it (`AssetSettings::game_path`): with
    /// `[assets] use_install = false` (or `CHIMERA_ASSET_USE_INSTALL=0`) a
    /// game the asset store serves never looks for its install: its path
    /// is the store's stand-in (`mount::served`); so it is with the
    /// service first (the game apps' default with a service set,
    /// `chimera_assets::config::InstallDefault`), a game the service
    /// doesn't serve reading its install; and when no install is found but
    /// the store serves the game.
    pub fn game_path_in(&self, game: &str, steam: Option<&super::install::SteamGame>) -> Option<PathBuf> {
        self.assets
            .resolve()
            .game_path(game, || super::install::resolve(self, game, steam).install.map(|i| i.path))
    }
}

impl LocalConfig {
    /// Keys configured for `game`, by scheme name.
    pub fn game_keys(&self, game: &str) -> Option<&BTreeMap<String, KeyConfig>> {
        self.games.get(game).map(|g| &g.keys)
    }
}

/// Chimera's own content cache for a game (maps fetched or imported, laid
/// out like the game's download folder): a per-user data folder, never
/// the game install and never the repo (docs/plans/active/custom-maps.md).
pub fn content_dir(game: &str) -> Option<PathBuf> {
    Some(crate::env::user_folder(&dirs::data_local_dir()?).join("content").join(game))
}

/// Where `dump` writes by default: a per-user data folder, never the repo.
pub fn default_dump_dir(game: &str) -> Option<PathBuf> {
    Some(crate::env::user_folder(&dirs::data_local_dir()?).join("dump").join(game))
}
