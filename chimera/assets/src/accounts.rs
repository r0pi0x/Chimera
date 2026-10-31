//! Who may fetch from the asset service (docs/plans/active/asset-service.md,
//! "Decisions": internal phase without Steam). A request's bearer token is
//! asked of each `Authenticator` in turn, one per account kind; the first
//! that knows it says who it is (`Principal`: kind, name, the games it may
//! fetch) or that it was revoked. Internal accounts (`Internal`, kind
//! `internal`) come first: the server admin runs `chimera_pack user add
//! <name>`, which prints a new token for the tester's `chimera.local.toml`;
//! each device gets its own (`user add` again), and `user revoke` takes one
//! or all back. Neither Steam sign-in nor the ownership check runs for them:
//! the server stays private (Tailscale) and ownership rests on the testers'
//! word. Steam sign-in and the ownership check become another
//! `Authenticator` (its principal's games = the games the account owns),
//! which must be on before the server is opened up.
//!
//! The accounts file (`<store>/server/accounts.toml`) keeps only each
//! token's SHA-256, never the token; it is read again when it changes, so
//! a revoke takes effect at the next request without a restart.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::store::{hex, sha256_hex, write_atomic};

/// Internal tokens start with this (so other kinds' tokens never collide).
pub const INTERNAL_PREFIX: &str = "chimera1_";

/// What an account may fetch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Games {
    All,
    Only(BTreeSet<String>),
}

impl Games {
    pub fn may(&self, game: &str) -> bool {
        match self {
            Games::All => true,
            Games::Only(games) => games.contains(game),
        }
    }

    fn from_list(list: &[String]) -> Self {
        if list.is_empty() || list.iter().any(|g| g == "*") {
            Games::All
        } else {
            Games::Only(list.iter().cloned().collect())
        }
    }
}

/// Who a token is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Principal {
    /// The account kind (`internal`; later `steam`).
    pub kind: String,
    /// The account's name (later: the SteamID64).
    pub name: String,
    /// Which token (for the log): its number on the account.
    pub token: u32,
    pub games: Games,
}

impl Principal {
    /// `kind:name#token`, as the access log writes it.
    pub fn label(&self) -> String {
        format!("{}:{}#{}", self.kind, self.name, self.token)
    }
}

/// A token's verdict.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    Granted(Principal),
    /// Known, but taken back.
    Revoked,
}

/// One kind of account: checks the tokens it issued.
pub trait Authenticator: Send + Sync {
    fn kind(&self) -> &str;
    /// None: not a token of this kind (the next authenticator is asked).
    fn authenticate(&self, token: &str) -> Option<Verdict>;
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Token {
    pub id: u32,
    /// SHA-256 of the token, hex.
    pub hash: String,
    /// Unix seconds.
    pub created: u64,
    #[serde(default)]
    pub revoked: bool,
    /// What it is for (a device), free text.
    #[serde(default)]
    pub note: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub name: String,
    /// Games it may fetch; empty or `*`: all.
    #[serde(default)]
    pub games: Vec<String>,
    #[serde(default, rename = "token")]
    pub tokens: Vec<Token>,
}

/// The accounts file.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Accounts {
    #[serde(default, rename = "account")]
    pub accounts: Vec<Account>,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Whether `name` is a usable account name.
pub fn is_account_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
}

impl Accounts {
    /// The accounts file of a server store.
    pub fn path(store_root: &Path) -> PathBuf {
        store_root.join("server").join("accounts.toml")
    }

    /// The file at `path`; none yet is no accounts.
    pub fn load(path: &Path) -> Result<Self, String> {
        match fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let text = format!(
            "# The asset server's internal accounts (chimera_pack user add | revoke | list).\n\
             # Token hashes only; the tokens themselves were printed once.\n\n{}",
            toml::to_string(self).map_err(std::io::Error::other)?
        );
        write_atomic(path, text.as_bytes())?;
        crate::sign::restrict(path);
        Ok(())
    }

    /// A new token for `name` (the account is created if new; `games`
    /// replaces its games when given). Returns the token: shown once.
    pub fn add(&mut self, name: &str, games: Option<Vec<String>>, note: &str) -> Result<(u32, String), String> {
        if !is_account_name(name) {
            return Err(format!("{name:?}: an account name is letters, digits, `_`, `-` or `.`"));
        }
        let mut secret = [0u8; 32];
        getrandom::fill(&mut secret).map_err(|e| format!("no randomness: {e}"))?;
        let token = format!("{INTERNAL_PREFIX}{}", hex(&secret));
        let account = match self.accounts.iter().position(|a| a.name == name) {
            Some(i) => &mut self.accounts[i],
            None => {
                self.accounts.push(Account { name: name.into(), ..Default::default() });
                self.accounts.last_mut().unwrap()
            }
        };
        if let Some(games) = games {
            account.games = games;
        }
        let id = account.tokens.iter().map(|t| t.id).max().unwrap_or(0) + 1;
        account.tokens.push(Token {
            id,
            hash: sha256_hex(token.as_bytes()),
            created: now(),
            revoked: false,
            note: note.into(),
        });
        Ok((id, token))
    }

    /// Revoke `name`'s token `id`, or all of its tokens; how many were
    /// revoked now.
    pub fn revoke(&mut self, name: &str, id: Option<u32>) -> Result<usize, String> {
        let account =
            self.accounts.iter_mut().find(|a| a.name == name).ok_or_else(|| format!("no account {name:?}"))?;
        let mut n = 0;
        for t in account.tokens.iter_mut().filter(|t| id.is_none_or(|id| t.id == id)) {
            n += usize::from(!t.revoked);
            t.revoked = true;
        }
        if id.is_some_and(|id| !account.tokens.iter().any(|t| t.id == id)) {
            return Err(format!("{name} has no token {}", id.unwrap_or(0)));
        }
        Ok(n)
    }

    fn verdict(&self, token: &str) -> Option<Verdict> {
        let hash = sha256_hex(token.as_bytes());
        self.accounts.iter().find_map(|a| {
            let t = a.tokens.iter().find(|t| t.hash == hash)?;
            Some(if t.revoked {
                Verdict::Revoked
            } else {
                Verdict::Granted(Principal {
                    kind: "internal".into(),
                    name: a.name.clone(),
                    token: t.id,
                    games: Games::from_list(&a.games),
                })
            })
        })
    }
}

/// Internal accounts from the accounts file, re-read when it changes.
pub struct Internal {
    path: PathBuf,
    cache: Mutex<(Option<(SystemTime, u64)>, Accounts)>,
}

impl Internal {
    pub fn new(path: PathBuf) -> Self {
        Self { path, cache: Mutex::new((None, Accounts::default())) }
    }
}

impl Authenticator for Internal {
    fn kind(&self) -> &str {
        "internal"
    }

    fn authenticate(&self, token: &str) -> Option<Verdict> {
        if !token.starts_with(INTERNAL_PREFIX) {
            return None;
        }
        let stamp = fs::metadata(&self.path).ok().and_then(|m| Some((m.modified().ok()?, m.len())));
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        if cache.0 != stamp || stamp.is_none() {
            match Accounts::load(&self.path) {
                Ok(accounts) => *cache = (stamp, accounts),
                Err(e) => {
                    // Refuse everyone rather than serve on a stale file.
                    eprintln!("accounts: {e}");
                    *cache = (None, Accounts::default());
                }
            }
        }
        cache.1.verdict(token)
    }
}
