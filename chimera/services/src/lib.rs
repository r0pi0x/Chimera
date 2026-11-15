//! The hub's services (docs/plans/active/hub.md): a library with a binary
//! per service, so each is updated and restarted on its own (the user's
//! decision, section 11). Now: the accounts service (`accounts`, binary
//! `chimera_accounts`) on its SQLite database (`db`), with the owner's
//! command line (`admin`); the asset service stays `chimera_asset_server`
//! (in `chimera_assets`), which takes this service's tickets
//! (`chimera_assets::accounts::TicketAuth`). Later: the front, the hub and
//! feedback.
//!
//! `codes`: invite and link codes, recovery keys, usernames and display
//! names. Host tools, plain data (no Bevy); SQLite lives only here, so no
//! game or client build compiles it.

pub mod accounts;
pub mod admin;
pub mod codes;
pub mod db;

/// Unix seconds.
pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Only the owner may read `path` (Unix; elsewhere the folder's ACL).
pub fn restrict(path: &std::path::Path) {
    chimera_assets::sign::restrict(path);
}
