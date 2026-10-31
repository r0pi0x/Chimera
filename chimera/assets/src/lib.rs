//! Game files as content-addressed records (docs/plans/active/asset-service.md,
//! M1): a `Store` holds each file a game's search path shows as one record,
//! named by the SHA-256 of its plain bytes and stored zstd-compressed
//! (`records/<ab>/<id>.zst`); a `Manifest` per game build maps paths to
//! records and names sets of them (`de_dust2`, `heavy-tests`;
//! `manifests/<game>/<build>.txt`). `CachedFiles` mounts a manifest as a
//! read-only `chimera_vfs::FileSource`, checking each record's hash as it
//! reads; `Recording` wraps a mount's layers and notes every file a load
//! reads (`CHIMERA_VFS_LOG=<file>`: game, path, size, hash per line), which
//! is how sets are recorded; `pack` fills a store from a mount. Settings
//! (`[assets]` in chimera.local.toml, `CHIMERA_ASSET_STORE`,
//! `CHIMERA_ASSET_USE_INSTALL`) are `AssetsConfig`; Chimera's `mount`
//! (chimera_core) layers the store under or instead of an install. The
//! store is per user (`~/.cache/chimera/assets`,
//! `%LOCALAPPDATA%\chimera\assets`), shared by every checkout, never in the
//! repo. Keys never reach it: Combat Arms' files are stored decrypted by
//! the host's packer.
//!
//! The asset service (M2): `server` serves a store over HTTP to the
//! accounts `accounts` knows (an `Authenticator` per account kind;
//! internal accounts for now), with manifests signed by the server's key
//! (`sign`); `remote` is its client, which fetches manifests and, as a
//! `Fetcher` behind `CachedFiles`, missing records on demand, checked
//! before they are stored; `lock` is the repository's `assets.lock`.
//!
//! Known shortcuts: `verify` and `gc` read the whole store single-threaded; an
//! on-demand fetch is one request per record (a set is fetched in batches
//! only by `chimera_assets fetch`); no cache cap.

pub mod accounts;
pub mod cached;
pub mod config;
pub mod lock;
pub mod manifest;
pub mod pack;
pub mod recording;
pub mod remote;
pub mod server;
pub mod sign;
pub mod store;

pub use cached::CachedFiles;
pub use config::{AssetSettings, AssetsConfig};
pub use lock::AssetsLock;
pub use manifest::{FileRecord, Manifest};
pub use pack::{PackReport, pack};
pub use recording::{LogLine, Recorder, Recording, read_log};
pub use remote::{Fetcher, Remote, RemoteConfig};
pub use store::{Store, sha256_hex};
