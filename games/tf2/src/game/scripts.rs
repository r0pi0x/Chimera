//! TF2's weapon and class scripts (`scripts/<class name>.ctx`, e.g.
//! `scripts/tf_weapon_scattergun.ctx`, `scripts/playerclasses/scout.ctx`):
//! KeyValues text, ICE-encrypted in the install (specs/tf2/ice.md, "TF2's
//! encrypted script files"; install.md 6), read through the game's mount
//! (`read`). Where the text comes from, in order (docs/plans/active/tf2.md,
//! section 3, decision 3), and `Script::source` says which served:
//!
//! 1. `ScriptSource::Plain`: bytes that already parse as KeyValues, as
//!    they are: the asset store's record (`chimera_pack` packs the
//!    scripts decrypted) or a plain file;
//! 2. `ScriptSource::Decrypted`: the install's encrypted file, decrypted
//!    in memory with the user's key (`CtxCipher::configured`: Thin-ICE
//!    unless `CHIMERA_TF2_ICE_LEVEL` says otherwise);
//! 3. `ScriptSource::Missing`: neither (no file, no key, or a key that
//!    doesn't decrypt it); the caller falls back to the specs' numbers.
//!
//! The key (`key`): `CHIMERA_TF2_CTX_KEY`, else the local config's
//! `[games.tf2.keys.ctx] key` (chimera.local.example.toml): an
//! 8-character text the user supplies. It is never logged, never in an
//! error, never written: decrypted text stays in memory, in the asset
//! store or under `target/`.
//!
//! `decrypting` wraps a mount so its `.ctx` files read decrypted (with
//! a key) or are hidden (without one): what `chimera_pack tf2` packs, so
//! the store's records are plain.
//!
//! `KvView` is a generic view of the parsed tree with typed getters
//! (number, integer, string, block); which fields a weapon reads is
//! M2b's, from a spec written with the key.

use source_formats::keyvalues::{self, Kv};

use super::ice::{self, CtxCipher};
use crate::mount::{Entry, FileSource, Mount, config::LocalConfig};

/// The environment variable holding the `.ctx` key (8 characters).
pub const KEY_ENV: &str = "CHIMERA_TF2_CTX_KEY";

/// The key's entry name in the local config: `[games.tf2.keys.ctx]`.
pub const KEY_NAME: &str = "ctx";

/// The `.ctx` key. Its `Debug` never shows it.
#[derive(Clone, PartialEq, Eq)]
pub struct CtxKey(pub [u8; 8]);

impl std::fmt::Debug for CtxKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CtxKey(..)")
    }
}

impl CtxKey {
    /// An 8-character key text. The error never includes the text.
    pub fn from_text(text: &str, what: &str) -> Result<CtxKey, String> {
        let bytes = text.as_bytes();
        let key: [u8; 8] = bytes.try_into().map_err(|_| format!("{what}: the key must be 8 characters"))?;
        Ok(CtxKey(key))
    }
}

/// The key from `CHIMERA_TF2_CTX_KEY`, else the local config's
/// `[games.tf2.keys.ctx] key`; None when neither is set.
pub fn key() -> Result<Option<CtxKey>, String> {
    if let Ok(text) = std::env::var(KEY_ENV)
        && !text.is_empty()
    {
        return CtxKey::from_text(&text, KEY_ENV).map(Some);
    }
    let config = LocalConfig::load()?;
    match config.game_keys(super::GAME).and_then(|k| k.get(KEY_NAME)) {
        Some(k) => CtxKey::from_text(&k.key, &format!("[games.{}.keys.{KEY_NAME}]", super::GAME)).map(Some),
        None => Ok(None),
    }
}

/// The key if one is configured, a broken one reported once as a warning
/// (without its text).
pub fn key_or_warn() -> Option<CtxKey> {
    key().unwrap_or_else(|e| {
        bevy::log::warn!("tf2: {e}");
        None
    })
}

/// Which source served a script (module doc).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptSource {
    /// Plain KeyValues bytes: the asset store's decrypted record, or a
    /// plain file.
    Plain,
    /// The install's encrypted file, decrypted with the user's key.
    Decrypted,
    /// No script: no file, no key, or the key didn't decrypt it.
    Missing,
}

impl ScriptSource {
    pub fn describe(&self) -> &'static str {
        match self {
            Self::Plain => "plain (the asset store or a plain file)",
            Self::Decrypted => "the install, decrypted with the local key",
            Self::Missing => "none",
        }
    }
}

/// A script read by `read`: its tree (empty when `Missing`), where it
/// came from and why it is missing.
#[derive(Clone, Debug)]
pub struct Script {
    pub name: String,
    pub source: ScriptSource,
    pub root: Kv,
    /// Why it is missing (no key, not found, didn't decrypt; never the
    /// key).
    pub note: Option<String>,
}

impl Script {
    /// The tree's first top-level block (`WeaponData` for a weapon).
    pub fn data(&self) -> Option<KvView<'_>> {
        self.root.items().iter().find(|(_, v)| matches!(v, Kv::Block(_))).map(|(_, v)| KvView(v))
    }

    /// The whole tree.
    pub fn view(&self) -> KvView<'_> {
        KvView(&self.root)
    }
}

/// A KeyValues block with typed getters (keys any case).
#[derive(Clone, Copy, Debug)]
pub struct KvView<'a>(pub &'a Kv);

impl<'a> KvView<'a> {
    pub fn string(&self, key: &str) -> Option<&'a str> {
        self.0.str(key).map(str::trim)
    }

    /// A number (`"0.25"`, `"10"`).
    pub fn number(&self, key: &str) -> Option<f64> {
        self.string(key)?.parse().ok()
    }

    /// An integer (`"10"`; a number with a fraction is not one).
    pub fn integer(&self, key: &str) -> Option<i64> {
        self.string(key)?.parse().ok()
    }

    pub fn block(&self, key: &str) -> Option<KvView<'a>> {
        match self.0.get(key)? {
            b @ Kv::Block(_) => Some(KvView(b)),
            Kv::Value(_) => None,
        }
    }

    /// The entries, in file order.
    pub fn items(&self) -> &'a [(String, Kv)] {
        self.0.items()
    }
}

/// The text of a decrypted script: a byte-order mark and trailing NUL
/// padding left out (ice.md, open question 4). None when it is not
/// UTF-8.
pub fn text_of(bytes: &[u8]) -> Option<&str> {
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    let end = bytes.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
    std::str::from_utf8(&bytes[..end]).ok()
}

/// Whether `bytes` are script text whose KeyValues has a block at its top
/// (`ice::looks_like_text` and a parse).
pub fn parses(bytes: &[u8]) -> bool {
    ice::looks_like_text(bytes)
        && text_of(bytes).is_some_and(|t| keyvalues::parse_pc(t).items().iter().any(|(_, v)| matches!(v, Kv::Block(_))))
}

/// The script's path in the mount: `scripts/<name>.ctx` (`name` may hold
/// a folder: `playerclasses/scout`).
pub fn path_of(name: &str) -> String {
    format!("scripts/{name}.ctx")
}

/// `bytes` (a `.ctx` file's) as a script: as they are when they already
/// parse, else decrypted with `key` and `cipher`.
pub fn from_bytes(name: &str, bytes: &[u8], key: Option<&CtxKey>, cipher: CtxCipher) -> Script {
    let script = |source, text: &str| Script {
        name: name.to_string(),
        source,
        root: keyvalues::parse_pc(text),
        note: None,
    };
    let missing = |note: String| Script {
        name: name.to_string(),
        source: ScriptSource::Missing,
        root: Kv::Block(Vec::new()),
        note: Some(note),
    };
    if parses(bytes) {
        return script(ScriptSource::Plain, text_of(bytes).unwrap_or_default());
    }
    let Some(key) = key else {
        return missing(format!(
            "{}: encrypted and no key ({KEY_ENV} or [games.tf2.keys.{KEY_NAME}])",
            path_of(name)
        ));
    };
    let plain = ice::decrypt_ctx(&cipher.ice(&key.0), bytes);
    if parses(&plain) {
        script(ScriptSource::Decrypted, text_of(&plain).unwrap_or_default())
    } else {
        missing(format!(
            "{}: the key doesn't decrypt it with cipher {} (run `dump tf2 --ice-probe`)",
            path_of(name),
            cipher.name()
        ))
    }
}

/// The script `name` (`tf_weapon_scattergun`, `playerclasses/scout`)
/// through `mount`, with `key` and the configured cipher; logged with its
/// source.
pub fn read(mount: &Mount, name: &str, key: Option<&CtxKey>) -> Script {
    let path = path_of(name);
    let script = match mount.read(&path) {
        Ok(bytes) => from_bytes(name, &bytes, key, CtxCipher::configured()),
        Err(e) => Script {
            name: name.to_string(),
            source: ScriptSource::Missing,
            root: Kv::Block(Vec::new()),
            note: Some(format!("{path}: {e}")),
        },
    };
    match &script.note {
        None => bevy::log::info!("tf2: script {name} from {}", script.source.describe()),
        Some(note) => bevy::log::warn!("tf2: script {name} missing: {note}"),
    }
    script
}

/// `mount` with every layer's `.ctx` files read decrypted with `key` and
/// the configured cipher (one that doesn't decrypt is read as it is, with
/// a warning), or, with no key, hidden: what `chimera_pack tf2` packs.
pub fn decrypting(mount: Mount, key: Option<CtxKey>) -> Mount {
    let cipher = CtxCipher::configured();
    mount.map_layers(|inner| -> Box<dyn FileSource> {
        Box::new(Decrypting { inner, ice: key.as_ref().map(|k| cipher.ice(&k.0)) })
    })
}

fn is_ctx(path: &str) -> bool {
    path.ends_with(".ctx")
}

/// A layer whose `.ctx` files read decrypted (`ice`) or are hidden.
struct Decrypting {
    inner: Box<dyn FileSource>,
    ice: Option<ice::Ice>,
}

impl FileSource for Decrypting {
    fn name(&self) -> String {
        self.inner.name()
    }

    fn entries(&self) -> Vec<Entry> {
        let mut e = self.inner.entries();
        if self.ice.is_none() {
            e.retain(|e| !is_ctx(&e.path));
        }
        e
    }

    fn read(&self, path: &str) -> Option<std::io::Result<Vec<u8>>> {
        if !is_ctx(path) {
            return self.inner.read(path);
        }
        let ice = self.ice.as_ref()?;
        Some(self.inner.read(path)?.map(|bytes| {
            if parses(&bytes) {
                return bytes;
            }
            let plain = ice::decrypt_ctx(ice, &bytes);
            if parses(&plain) {
                plain
            } else {
                bevy::log::warn!("tf2: {path} doesn't decrypt to KeyValues with cipher {}: kept as it is", CtxCipher::configured().name());
                bytes
            }
        }))
    }

    fn contains(&self, path: &str) -> bool {
        (self.ice.is_some() || !is_ctx(path)) && self.inner.contains(path)
    }

    fn files_in(&self, dir: &str) -> Vec<String> {
        let mut f = self.inner.files_in(dir);
        if self.ice.is_none() {
            f.retain(|p| !is_ctx(p));
        }
        f
    }

    fn fingerprint(&self, path: &str) -> Option<String> {
        // The decrypted content differs from the archive's: the reader
        // hashes it instead.
        if is_ctx(path) { None } else { self.inner.fingerprint(path) }
    }
}
