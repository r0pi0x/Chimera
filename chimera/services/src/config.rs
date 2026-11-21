//! Where the hub's services listen (docs/plans/active/hub.md, 2.3):
//! `<store>/server/services.toml`, written by the host, every key
//! optional. The services' loopback ports (the asset service 8731, the
//! accounts service 8741, the hub 8742, feedback on the asset service's
//! until S5) and the front's public `listen` address; `chimera_front`,
//! `chimera_accounts status` and `install-units` read it, command-line
//! flags win.
//!
//! ```toml
//! [front]
//! listen = "0.0.0.0:<hub port>"
//! [ports]
//! assets = 8731
//! accounts = 8741
//! hub = 8742
//! [hub]
//! identity = "<id>"   # the active hub identity (crate::identity); none: main
//! ```

use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::front::Routes;

/// The asset service's loopback port by default.
pub const ASSETS_PORT: u16 = chimera_assets::server::DEFAULT_PORT;
/// The hub's loopback port by default (S2).
pub const HUB_PORT: u16 = 8742;

/// The front's settings.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontConfig {
    /// The public `ip:port` (none: `chimera_front --listen` names it).
    #[serde(default)]
    pub listen: Option<String>,
    /// Connections at once per address (`tls::FrontLimits::per_address`).
    #[serde(default)]
    pub per_address: Option<usize>,
}

/// The services' loopback ports.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ports {
    #[serde(default = "assets_port")]
    pub assets: u16,
    #[serde(default = "accounts_port")]
    pub accounts: u16,
    #[serde(default = "hub_port")]
    pub hub: u16,
    /// Feedback's own (S5); none: the asset service's.
    #[serde(default)]
    pub feedback: Option<u16>,
}

fn assets_port() -> u16 {
    ASSETS_PORT
}
fn accounts_port() -> u16 {
    crate::accounts::DEFAULT_PORT
}
fn hub_port() -> u16 {
    HUB_PORT
}

impl Default for Ports {
    fn default() -> Self {
        Self { assets: ASSETS_PORT, accounts: accounts_port(), hub: HUB_PORT, feedback: None }
    }
}

/// `[hub]`: which identity the hub presents (`crate::identity`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HubSection {
    /// The active identity's id; none: `main` (the store's own folders).
    #[serde(default)]
    pub identity: Option<String>,
}

/// `services.toml`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServicesConfig {
    #[serde(default)]
    pub front: FrontConfig,
    #[serde(default)]
    pub ports: Ports,
    #[serde(default)]
    pub hub: HubSection,
}

impl ServicesConfig {
    /// `<store>/server/services.toml`.
    pub fn path(store: &Path) -> PathBuf {
        store.join("server").join("services.toml")
    }

    /// The store's file (none there: the defaults).
    pub fn load(store: &Path) -> Result<Self, String> {
        let path = Self::path(store);
        match std::fs::read_to_string(&path) {
            Ok(text) => Self::parse(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    /// The front's routes: the loopback listeners.
    pub fn routes(&self) -> Routes {
        let at = |port: u16| SocketAddr::from(([127, 0, 0, 1], port));
        Routes {
            assets: at(self.ports.assets),
            accounts: at(self.ports.accounts),
            hub: at(self.ports.hub),
            feedback: at(self.ports.feedback.unwrap_or(self.ports.assets)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_overrides() {
        let d = ServicesConfig::default().routes();
        assert_eq!((d.assets.port(), d.accounts.port(), d.hub.port(), d.feedback.port()), (8731, 8741, 8742, 8731));
        let c: ServicesConfig = toml::from_str("[front]\nlisten = \"0.0.0.0:9\"\n[ports]\naccounts = 9001\n").unwrap();
        assert_eq!(c.front.listen.as_deref(), Some("0.0.0.0:9"));
        assert_eq!((c.ports.assets, c.ports.accounts), (8731, 9001));
    }
}
