//! The hub's services under systemd's user manager (docs/plans/active/hub.md,
//! 2.6): one unit per service (`chimera-front`, `chimera-accounts`,
//! `chimera-assets`; feedback stays in the asset service until S5, the
//! hub's comes with S2), all `PartOf=chimera-services.target`. The
//! templates are `chimera/services/systemd/` (compiled in);
//! `chimera_accounts install-units` fills in the binaries' folder, the
//! store, the ports (`services.toml`) and the front's public address, and
//! writes them to `~/.config/systemd/user/` (or `--to`).
//!
//! Each unit restarts on failure (2 s; at most 5 starts in 2 minutes, then
//! it stays down and `systemctl --user status` shows why), logs to journald
//! (`journalctl --user -u chimera-accounts -f`) and may write only the
//! store (`ProtectSystem=strict`, `ReadWritePaths`; the front reads it
//! only). The binaries are copies in `~/.local/lib/chimera/` (`--bin`), so
//! a `cargo build` never swaps one under a running service. Lingering
//! (`loginctl enable-linger`) keeps them running without a login session.

use std::path::{Path, PathBuf};

use crate::config::ServicesConfig;

/// The units, by file name.
pub const TEMPLATES: &[(&str, &str)] = &[
    ("chimera-services.target", include_str!("../systemd/chimera-services.target")),
    ("chimera-front.service", include_str!("../systemd/chimera-front.service")),
    ("chimera-accounts.service", include_str!("../systemd/chimera-accounts.service")),
    ("chimera-assets.service", include_str!("../systemd/chimera-assets.service")),
];

/// What the units are made for.
#[derive(Clone, Debug)]
pub struct UnitSettings {
    /// Where the binaries are (`~/.local/lib/chimera`).
    pub bin: PathBuf,
    /// The server store.
    pub store: PathBuf,
    pub config: ServicesConfig,
    /// The front's public `ip:port`.
    pub front_listen: String,
    /// More arguments for the asset service (its limits, `--on-report`).
    pub assets_args: Vec<String>,
    /// More arguments for the accounts service (`--public <host:port>`).
    pub accounts_args: Vec<String>,
    /// More folders the asset service may write (an `--on-report`
    /// command's own).
    pub writable: Vec<PathBuf>,
}

/// A word for a unit file's command line: quoted when it holds a space or
/// a quote (systemd's rules), `%` doubled.
fn word(text: &str) -> String {
    let text = text.replace('%', "%%");
    if text.is_empty() || text.contains([' ', '\t', '"', '\'', '\\', ';']) {
        format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        text
    }
}

fn args(list: &[String]) -> String {
    list.iter().map(|a| format!(" {}", word(a))).collect()
}

/// The units as written: (file name, text).
pub fn render(s: &UnitSettings) -> Vec<(String, String)> {
    let path = |p: &Path| word(&p.display().to_string());
    let mut store_rw = path(&s.store);
    for w in &s.writable {
        store_rw.push(' ');
        store_rw.push_str(&path(w));
    }
    TEMPLATES
        .iter()
        .map(|(name, text)| {
            let mut text = text.to_string();
            for bin in ["chimera_asset_server", "chimera_accounts", "chimera_front"] {
                text = text.replace(&format!("@BIN@/{bin}"), &path(&s.bin.join(bin)));
            }
            let mut text = text
                .replace("@ASSETS_PORT@", &s.config.ports.assets.to_string())
                .replace("@ACCOUNTS_PORT@", &s.config.ports.accounts.to_string())
                .replace("@FRONT_LISTEN@", &word(&s.front_listen))
                .replace("@ASSETS_ARGS@", &args(&s.assets_args))
                .replace("@ACCOUNTS_ARGS@", &args(&s.accounts_args));
            text = if *name == "chimera-assets.service" {
                text.replace("ReadWritePaths=@STORE@", &format!("ReadWritePaths={store_rw}"))
            } else {
                text
            };
            (name.to_string(), text.replace("@STORE@", &path(&s.store)))
        })
        .collect()
}

/// `~/.config/systemd/user`.
pub fn user_unit_dir() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("systemd").join("user"))
}

/// Write the units into `dir`; the files written.
pub fn install(s: &UnitSettings, dir: &Path) -> Result<Vec<PathBuf>, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut out = Vec::new();
    for (name, text) in render(s) {
        let path = dir.join(name);
        chimera_assets::store::write_atomic(&path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))?;
        out.push(path);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Unit files are Linux's (paths with `/`).
    #[cfg(unix)]
    #[test]
    fn units_are_filled_in() {
        let s = UnitSettings {
            bin: PathBuf::from("/opt/chimera bin"),
            store: PathBuf::from("/srv/store"),
            config: ServicesConfig::default(),
            front_listen: "0.0.0.0:4443".into(),
            assets_args: vec!["--rate".into(), "50".into()],
            accounts_args: vec!["--public".into(), "hub.example:4443".into()],
            writable: vec![PathBuf::from("/srv/hooks")],
        };
        let units = render(&s);
        assert_eq!(units.len(), 4);
        for (name, text) in &units {
            assert!(!text.contains('@'), "{name}: a placeholder left:\n{text}");
        }
        let assets = &units.iter().find(|(n, _)| n == "chimera-assets.service").unwrap().1;
        assert!(assets.contains("ExecStart=\"/opt/chimera bin/chimera_asset_server\" --store /srv/store --listen 127.0.0.1:8731 --accounts http://127.0.0.1:8741 --rate 50"), "{assets}");
        assert!(assets.contains("ReadWritePaths=/srv/store /srv/hooks"), "{assets}");
        let front = &units.iter().find(|(n, _)| n == "chimera-front.service").unwrap().1;
        assert!(front.contains("--listen 0.0.0.0:4443") && front.contains("ReadOnlyPaths=/srv/store"), "{front}");
        let accounts = &units.iter().find(|(n, _)| n == "chimera-accounts.service").unwrap().1;
        assert!(accounts.contains("serve --store /srv/store --listen 127.0.0.1:8741 --public hub.example:4443"), "{accounts}");
        assert!(accounts.contains("Restart=on-failure") && accounts.contains("PartOf=chimera-services.target"));
    }
}
