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
//!
//! Platform accounts (docs/plans/active/hub.md, 2.4): `TicketAuth` (kind
//! `account`) takes the accounts service's tickets (`ticket`, the
//! `chimera2t.` prefix), checked with the ticket public keys alone
//! (`<store>/server/accounts/ticket.pub`, read again when it changes) and
//! the revocation feed, which a thread polls from the accounts service on
//! loopback (`with_feed`: every 30 s, with the services' shared secret;
//! the last list is kept while accounts doesn't answer). Its principal is
//! the account's username; its games are the ticket's `content`.

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

    /// A ticket's `content`: `*` all, else exactly these (none: nothing).
    pub fn from_content(list: &[String]) -> Self {
        if list.iter().any(|g| g == "*") {
            Games::All
        } else {
            Games::Only(list.iter().cloned().collect())
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
    /// Refused for this reason (an expired or forged ticket).
    Refused(String),
}

/// One kind of account: checks the tokens it issued.
pub trait Authenticator: Send + Sync {
    fn kind(&self) -> &str;
    /// None: not a token of this kind (the next authenticator is asked).
    fn authenticate(&self, token: &str) -> Option<Verdict>;
    /// A word for the service's `/health` line (`TicketAuth`: the
    /// revocation feed's age).
    fn status(&self) -> Option<String> {
        None
    }
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

/// Unix seconds as `YYYY-MM-DD HH:MM UTC` (`user list`; civil date from
/// days since the epoch, so no date crate).
pub fn utc_text(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rest = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02} UTC", rest / 3_600, rest % 3_600 / 60)
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

/// The services' shared secret's header (loopback calls between services).
pub const SERVICE_HEADER: &str = "X-Chimera-Service";
/// How often `TicketAuth` asks the accounts service for revocations.
pub const FEED_EVERY: std::time::Duration = std::time::Duration::from_secs(30);

/// The ticket public keys of a services box (`ticket.pub`).
pub fn ticket_keys_path(store_root: &Path) -> PathBuf {
    store_root.join("server").join("accounts").join("ticket.pub")
}

/// The services' shared secret (`<store>/server/services/secret`).
pub fn service_secret_path(store_root: &Path) -> PathBuf {
    store_root.join("server").join("services").join("secret")
}

/// Account tickets (`ticket`), checked locally.
pub struct TicketAuth {
    keys_path: PathBuf,
    keys: Mutex<(Option<(SystemTime, u64)>, crate::ticket::TicketKeys)>,
    revoked: std::sync::Arc<std::sync::RwLock<crate::ticket::Revocations>>,
    /// When the feed last answered (Unix seconds; 0: never).
    fed_at: std::sync::Arc<std::sync::atomic::AtomicU64>,
    feed: Option<Feed>,
}

/// The revocation poller; stopped when dropped.
struct Feed {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for Feed {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            t.thread().unpark();
            let _ = t.join();
        }
    }
}

/// One ask of the feed: the entries after `since`.
pub fn read_feed(accounts_url: &str, secret: &str, since: u64) -> Result<String, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(std::time::Duration::from_secs(5)))
        .max_redirects(0)
        .build()
        .new_agent();
    let url = format!("{}/internal/revoked?since={since}", accounts_url.trim_end_matches('/'));
    let mut response = agent.get(&url).header(SERVICE_HEADER, secret).call().map_err(|e| format!("{url}: {e}"))?;
    let status = response.status().as_u16();
    let body = response.body_mut().read_to_string().map_err(|e| format!("{url}: {e}"))?;
    if status != 200 {
        return Err(format!("{url}: {status} {}", body.trim()));
    }
    Ok(body)
}

impl TicketAuth {
    /// Tickets checked with the keys in `keys_path` (none there: every
    /// ticket refused), with no revocation feed yet.
    pub fn new(keys_path: PathBuf) -> Self {
        Self {
            keys_path,
            keys: Mutex::new((None, Default::default())),
            revoked: Default::default(),
            fed_at: Default::default(),
            feed: None,
        }
    }

    /// Poll the accounts service at `accounts_url` (plain HTTP on loopback)
    /// for revocations every `every`, with the secret in `secret_path`
    /// (read again each time: accounts makes it at its first start).
    pub fn with_feed(mut self, accounts_url: String, secret_path: PathBuf, every: std::time::Duration) -> Self {
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (revoked, stop2, fed_at) = (self.revoked.clone(), stop.clone(), self.fed_at.clone());
        let thread = std::thread::Builder::new()
            .name("revocation-feed".into())
            .spawn(move || {
                let mut said = false;
                while !stop2.load(std::sync::atomic::Ordering::Relaxed) {
                    let secret = fs::read_to_string(&secret_path).map(|s| s.trim().to_string());
                    let since = revoked.read().unwrap_or_else(|e| e.into_inner()).seq;
                    let got = secret
                        .map_err(|e| format!("{}: {e}", secret_path.display()))
                        .and_then(|secret| read_feed(&accounts_url, &secret, since));
                    match got {
                        Ok(text) => {
                            let mut r = revoked.write().unwrap_or_else(|e| e.into_inner());
                            if let Err(e) = r.apply(&text) {
                                eprintln!("tickets: {e}");
                            }
                            fed_at.store(now(), std::sync::atomic::Ordering::Relaxed);
                            said = false;
                        }
                        // Keep the last list; say so once until it answers again.
                        Err(e) if !said => {
                            eprintln!("tickets: no revocation feed ({e}); keeping the last list");
                            said = true;
                        }
                        Err(_) => {}
                    }
                    std::thread::park_timeout(every);
                }
            })
            .expect("a thread");
        self.feed = Some(Feed { stop, thread: Some(thread) });
        self
    }

    /// The revocations as last read (tests, `status`).
    pub fn revocations(&self) -> std::sync::Arc<std::sync::RwLock<crate::ticket::Revocations>> {
        self.revoked.clone()
    }

    /// A `services` ticket's claims at `now`: checked with the keys and
    /// the revocations as last read. `Err(None)`: revoked; `Err(Some)`:
    /// refused, and why (the hub uses the claims' rights).
    pub fn check(&self, token: &str, now: u64) -> Result<crate::ticket::Claims, Option<String>> {
        let claims = self.keys().check(token, crate::ticket::SERVICES, now).map_err(|e| Some(e.to_string()))?;
        if self.revoked.read().unwrap_or_else(|e| e.into_inner()).refuses(&claims) {
            return Err(None);
        }
        Ok(claims)
    }

    fn keys(&self) -> crate::ticket::TicketKeys {
        let stamp = fs::metadata(&self.keys_path).ok().and_then(|m| Some((m.modified().ok()?, m.len())));
        let mut cache = self.keys.lock().unwrap_or_else(|e| e.into_inner());
        if cache.0 != stamp || stamp.is_none() {
            let keys = fs::read_to_string(&self.keys_path)
                .map_err(|e| e.to_string())
                .and_then(|t| crate::ticket::TicketKeys::parse(&t));
            *cache = match keys {
                Ok(k) => (stamp, k),
                Err(_) => (None, Default::default()),
            };
        }
        cache.1.clone()
    }
}

impl Authenticator for TicketAuth {
    fn kind(&self) -> &str {
        "account"
    }

    fn status(&self) -> Option<String> {
        let at = self.fed_at.load(std::sync::atomic::Ordering::Relaxed);
        Some(match (&self.feed, at) {
            (None, _) => "revocations=off".into(),
            (Some(_), 0) => "revocations=never".into(),
            (Some(_), at) => format!("revocations={}s", now().saturating_sub(at)),
        })
    }

    fn authenticate(&self, token: &str) -> Option<Verdict> {
        if !token.starts_with(crate::ticket::PREFIX) {
            return None;
        }
        let claims = match self.check(token, now()) {
            Ok(c) => c,
            Err(None) => return Some(Verdict::Revoked),
            Err(Some(e)) => return Some(Verdict::Refused(e)),
        };
        Some(Verdict::Granted(Principal {
            kind: "account".into(),
            name: claims.username.clone(),
            token: 0,
            games: Games::from_content(&claims.content),
        }))
    }
}
