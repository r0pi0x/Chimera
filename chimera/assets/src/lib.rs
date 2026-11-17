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
//! Platform accounts (docs/plans/active/hub.md): `ticket` is the accounts
//! service's signed tickets and its revocation feed, checked by any
//! service with the public key alone; `account` is a player's side (the
//! hub link, the device key, `account.toml`, tickets refreshed at
//! half-life), and `accounts::TicketAuth` lets the asset service take
//! tickets beside legacy tokens. `hub_doc` is the hub's discovery
//! document, heartbeats and server list, both sides, and the client's
//! cache of them; `party_doc` the hub's parties, both sides.
//!
//! `resolve`: which sets a player fetches when (startup, each match,
//! ahead of need), the records a list of sets comes to, and the lean
//! `base` made from client runs' read logs (`chimera_pack lean`).
//!
//! `progress` reports fetches in words people read: a live line for
//! `chimera_assets fetch`, periodic lines while a load fetches on demand.
//! `sets` fetches whole sets as the game does (a join's, the startup's,
//! the streams'); `pace` caps a background fetch's rate, with a pause;
//! `pending` is what a foreground fill is fetching into a store now, which an
//! on-demand miss in that store waits for rather than fetching twice.
//! `units`: a game's set and what it needs, downloaded on demand in the
//! background with its state and progress (the sandbox's guns).
//!
//! `feedback`: in-game feedback and bug reports: the bundle and its secret
//! filter, the tester's outbox and sender, the receiver behind the
//! service's `POST /feedback`, its triage page.
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
pub mod account;
pub mod accounts;
pub mod cached;
pub mod config;
pub mod disk;
pub mod feedback;
pub mod hub_doc;
pub mod lock;
pub mod manifest;
pub mod map_cache;
pub mod pace;
pub mod pack;
pub mod party_doc;
pub mod pending;
pub mod progress;
pub mod recording;
pub mod remote;
pub mod resolve;
pub mod server;
pub mod sets;
pub mod shared;
pub mod sign;
pub mod store;
pub mod ticket;
pub mod tls;
pub mod units;

pub use cached::CachedFiles;
pub use config::{AssetSettings, AssetsConfig};
pub use lock::AssetsLock;
pub use manifest::{FileRecord, Manifest};
pub use pack::{PackReport, pack};
pub use recording::{LogLine, Recorder, Recording, read_log, read_log_with_checks};
pub use remote::{Fetcher, Remote, RemoteConfig};
pub use store::{Store, sha256_hex};
