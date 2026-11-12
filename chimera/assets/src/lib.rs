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
//! For players over the internet ("Playtest delivery" in the plan): `tls`
//! gives the service HTTPS with its own self-signed certificate (a TLS
//! front before the plain service) and the client a connection that trusts
//! exactly the certificate whose fingerprint it was given; `access` keeps a
//! player's token and the service a game server bound it to.
//!
//! `progress` reports fetches in words people read: a live line for
//! `chimera_assets fetch`, periodic lines while a load fetches on demand.
//! `sets` fetches whole sets as the game does (a join's, the startup's,
//! the prefetch's); `pace` caps a background fetch's rate, with a pause;
//! `pending` is what a foreground fill is fetching into a store now, which an
//! on-demand miss in that store waits for rather than fetching twice.
//!
//! `map_cache`: converted maps on disk beside the store, keyed by the
//! converter and checked against every file the conversion read
//! (`~/.cache/chimera/maps`, `CHIMERA_MAP_CACHE`).
//!
//! `shared`: values read from an install that the apps of one process
//! share while any of them holds one (Combat Arms' attribute data and
//! mount), freed after the last.
//!
//! `disk`: free space (`statvfs`, `GetDiskFreeSpaceExW`) and the floor
//! under which the map cache and the store's on-demand writes skip the
//! write (`CHIMERA_DISK_FLOOR_GB`).
//!
//! Known shortcuts: `verify` and `gc` read the whole store single-threaded; no
//! cap on the store; `chimera_pack`'s `put` doesn't check free space (an
//! explicit fill fails loudly instead).

pub mod access;
pub mod accounts;
pub mod cached;
pub mod config;
pub mod disk;
pub mod lock;
pub mod manifest;
pub mod map_cache;
pub mod pace;
pub mod pack;
pub mod pending;
pub mod progress;
pub mod recording;
pub mod remote;
pub mod server;
pub mod sets;
pub mod shared;
pub mod sign;
pub mod store;
pub mod tls;

pub use cached::CachedFiles;
pub use config::{AssetSettings, AssetsConfig};
pub use lock::AssetsLock;
pub use manifest::{FileRecord, Manifest};
pub use pack::{PackReport, pack};
pub use recording::{LogLine, Recorder, Recording, read_log};
pub use remote::{Fetcher, Remote, RemoteConfig};
pub use store::{Store, sha256_hex};
