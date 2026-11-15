//! The accounts service end to end on scratch stores and free loopback
//! ports (docs/plans/active/hub.md, S1a): invites, sign-up, link codes,
//! recovery, tickets at the asset service, revocation, legacy tokens, the
//! database's migrations and backups.

mod accounts;
mod db;

use std::{fs, path::PathBuf};

/// A fresh folder for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("chimera_services_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}
