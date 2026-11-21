//! Hub identities (docs/plans/active/hub.md, 2.8): what a hub presents as
//! itself, as named profiles in one server store, one active at a time.
//!
//! An identity is a folder: its settings (`hub/hub.toml`: the name, MOTD,
//! news, ...), its server list (`hub/hub.db`), its accounts and invites
//! (`accounts/accounts.db`) with their ticket key (`accounts/ticket.key`)
//! and its backups (`backups/`). So each identity is its own community:
//! switching shows another name and signs nobody of one into the other.
//! What the machine is stays shared: the asset store, the certificate
//! (`tls/`), the manifest key, the services' secret, the ports
//! (`services.toml`) and feedback.
//!
//! The identity named `main` is the store's own folders (`server/hub`,
//! `server/accounts`, `server/backups`), so a hub from before identities
//! is `main` with nothing moved; any other lives in
//! `server/identities/<name>/`. `services.toml`'s `[hub] identity` names
//! the active one (none: `main`); the accounts service and the hub read
//! it at start, so a switch is `identity use <name>` and a restart of the
//! services (`chimera_accounts identity ...`). The active accounts
//! service writes its identity's public key to the store's `ticket.pub`,
//! which the asset service and the hub check tickets with: after a
//! switch, the other identity's tickets are refused everywhere.
//!
//! A hub with no name set shows `DEFAULT_NAME`.

use std::path::{Path, PathBuf};

use crate::{config::ServicesConfig, hub::HubConfig};

/// A hub's name when its hub.toml sets none.
pub const DEFAULT_NAME: &str = "Chimera Hub";
/// The identity in the store's own folders.
pub const MAIN: &str = "main";

/// An identity's id: lowercase letters, digits, `-` and `_`, 1 to 32.
pub fn check_id(id: &str) -> Result<(), String> {
    let ok = !id.is_empty()
        && id.len() <= 32
        && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_');
    if ok { Ok(()) } else { Err(format!("identity {id:?}: 1 to 32 of a-z, 0-9, - and _")) }
}

/// The folder holding identity `id`'s `hub/`, `accounts/` and `backups/`.
pub fn root(store: &Path, id: &str) -> PathBuf {
    let server = store.join("server");
    if id == MAIN { server } else { server.join("identities").join(id) }
}

/// The active identity's id (`services.toml`'s `[hub] identity`; none:
/// `main`).
pub fn active(store: &Path) -> Result<String, String> {
    let id = ServicesConfig::load(store)?.hub.identity.unwrap_or_else(|| MAIN.to_string());
    check_id(&id)?;
    Ok(id)
}

/// The active identity's folder.
pub fn active_root(store: &Path) -> Result<PathBuf, String> {
    Ok(root(store, &active(store)?))
}

/// The hub.toml of the identity at `root`.
pub fn config_path(root: &Path) -> PathBuf {
    root.join("hub").join("hub.toml")
}

/// The name the identity at `root` shows: hub.toml's, else
/// `DEFAULT_NAME`.
pub fn name_at(root: &Path) -> String {
    let config = std::fs::read_to_string(config_path(root)).ok().and_then(|t| HubConfig::parse(&t).ok()).unwrap_or_default();
    shown_name(&config.name)
}

/// `name`, or `DEFAULT_NAME` when blank.
pub fn shown_name(name: &str) -> String {
    if name.trim().is_empty() { DEFAULT_NAME.to_string() } else { name.trim().to_string() }
}

/// One identity, for `identity list`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub id: String,
    pub name: String,
    pub active: bool,
    /// Whether it has an accounts database yet (made at its first start).
    pub has_accounts: bool,
}

/// Every identity in the store: `main` first, then the others by id.
pub fn list(store: &Path) -> Result<Vec<Identity>, String> {
    let active = active(store)?;
    let mut ids = vec![MAIN.to_string()];
    let dir = store.join("server").join("identities");
    if let Ok(entries) = std::fs::read_dir(&dir) {
        let mut others: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|id| id != MAIN && check_id(id).is_ok())
            .collect();
        others.sort();
        ids.extend(others);
    }
    if !ids.contains(&active) {
        ids.push(active.clone());
    }
    Ok(ids
        .into_iter()
        .map(|id| {
            let root = root(store, &id);
            Identity {
                name: name_at(&root),
                active: id == active,
                has_accounts: root.join("accounts").join("accounts.db").exists(),
                id,
            }
        })
        .collect())
}

/// Whether identity `id` exists (`main` always does).
pub fn exists(store: &Path, id: &str) -> bool {
    id == MAIN || root(store, id).is_dir()
}

/// A new identity `id` showing `name` (blank: `DEFAULT_NAME`): its folder
/// and hub.toml. Its accounts database and ticket key are made when it
/// first starts; it starts with no accounts, so its first owner invite is
/// made after the switch (`chimera_accounts invite ... --role owner`).
pub fn create(store: &Path, id: &str, name: &str) -> Result<PathBuf, String> {
    check_id(id)?;
    if exists(store, id) {
        return Err(format!("identity {id:?} exists"));
    }
    let root = root(store, id);
    std::fs::create_dir_all(root.join("hub")).map_err(|e| format!("{}: {e}", root.display()))?;
    set_name(store, id, if name.trim().is_empty() { DEFAULT_NAME } else { name })?;
    Ok(root)
}

/// Set identity `id`'s name in its hub.toml, the rest of the file kept
/// as written (comments too); made if missing. The hub reads hub.toml
/// again when it changes, so no restart is needed.
pub fn set_name(store: &Path, id: &str, name: &str) -> Result<(), String> {
    check_id(id)?;
    if !exists(store, id) {
        return Err(format!("no identity {id:?} (chimera_accounts identity list)"));
    }
    let name = name.trim();
    if name.is_empty() || name.len() > 64 || name.chars().any(char::is_control) {
        return Err("a name: 1 to 64 characters, no control characters".into());
    }
    let path = config_path(&root(store, id));
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let updated = with_name(&text, name);
    HubConfig::parse(&updated).map_err(|e| format!("{}: {e}", path.display()))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    chimera_assets::store::write_atomic(&path, updated.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
}

/// `text` (a hub.toml) with its top-level `name` set to `name`: the line
/// replaced where it is, else added first.
pub fn with_name(text: &str, name: &str) -> String {
    let line = format!("name = {}", toml::Value::String(name.to_string()));
    let mut out = Vec::new();
    let mut done = false;
    let mut top = true;
    for l in text.lines() {
        let t = l.trim_start();
        if t.starts_with('[') {
            top = false;
        }
        let is_name = t.strip_prefix("name").is_some_and(|rest| rest.trim_start().starts_with('='));
        if top && !done && is_name {
            out.push(line.clone());
            done = true;
        } else {
            out.push(l.to_string());
        }
    }
    if !done {
        out.insert(0, line);
    }
    let mut s = out.join("\n");
    s.push('\n');
    s
}

/// Make `id` the active identity in `services.toml` (its other keys
/// kept). The services take it at their next start.
pub fn activate(store: &Path, id: &str) -> Result<(), String> {
    check_id(id)?;
    if !exists(store, id) {
        return Err(format!("no identity {id:?} (chimera_accounts identity new {id})"));
    }
    let path = ServicesConfig::path(store);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let mut table: toml::Table = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let hub = table.entry("hub").or_insert_with(|| toml::Value::Table(toml::Table::new()));
    let hub = hub.as_table_mut().ok_or_else(|| format!("{}: [hub] is not a table", path.display()))?;
    if id == MAIN {
        hub.remove("identity");
    } else {
        hub.insert("identity".into(), toml::Value::String(id.into()));
    }
    if hub.is_empty() {
        table.remove("hub");
    }
    let out = toml::to_string(&table).map_err(|e| e.to_string())?;
    ServicesConfig::parse(&out).map_err(|e| format!("{}: {e}", path.display()))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    chimera_assets::store::write_atomic(&path, out.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_names() {
        assert!(check_id("friends-2").is_ok());
        for bad in ["", "Friends", "a/b", "..", &"x".repeat(33)] {
            assert!(check_id(bad).is_err(), "{bad:?}");
        }
        assert_eq!(with_name("", "A"), "name = \"A\"\n");
        let quoted = with_name("# hub\nname = \"Old\"\nmotd = \"hi\"\n", "New \"x\"");
        assert!(quoted.starts_with("# hub\nname = ") && quoted.ends_with("\nmotd = \"hi\"\n"), "{quoted}");
        let c = HubConfig::parse(&quoted).unwrap();
        assert_eq!((c.name.as_str(), c.motd.as_str()), ("New \"x\"", "hi"));
        assert_eq!(
            with_name("motd = \"hi\"\n[[news]]\nname = \"not this\"\n", "B"),
            "name = \"B\"\nmotd = \"hi\"\n[[news]]\nname = \"not this\"\n"
        );
        assert_eq!(shown_name("  "), DEFAULT_NAME);
    }
}
