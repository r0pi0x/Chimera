//! The accounts service (docs/plans/active/hub.md, 2.4 and sections 5-7):
//! the platform account, owned by the owner alone. `Accounts` is its logic
//! over `accounts.db` (`db`), `http` its routes.
//!
//! - **Invites** (`invite`): the owner's, single use, 14 days by default,
//!   stored as hashes and printed once; a re-invite (`for_username`) adds a
//!   device to that account and revokes its others unless `keep`.
//! - **Sign-up** (`redeem`): an invite code with a username (none: the
//!   invite's name) and the device's public key makes the account (a taken
//!   or malformed username is refused without using the code), its first
//!   device and its recovery key; a link code (`link`: 10 minutes, once,
//!   made with a ticket) adds a device to the same account.
//! - **Recovery** (`recover`): the username and recovery key add a device
//!   and revoke the others; a new recovery key replaces the old.
//! - **Tickets** (`ticket`): for a device's signed request (its time
//!   within 5 minutes, its nonce unseen in that window) or a legacy
//!   `chimera1_` token imported as a `legacy` device; 6 hours, the ticket
//!   key's (`<store>/server/accounts/ticket.key`, 0600; its public half in
//!   `ticket.pub` for the services on this box).
//! - **Revocations**: revoking a device or an account, a ban and a
//!   recovery add rows to the feed (`revoked_since`) the other services
//!   poll; a ticket issued at or before a row's time is refused.
//! - **Limits**: failed codes and recovery keys count per address (and per
//!   username for recovery): past `Options::failures_per_hour` the next
//!   try is 429 for the hour. Bans (`ban`: an account or an address, for a
//!   time or for good) refuse redeeming, recovering and tickets.
//! - **The owner's side** (`list_accounts`, `list_invites`, `revoke_*`,
//!   `ban`, `import_legacy`, `backup`): through the admin API on loopback
//!   (`http`) and its page (`page`), every change an `audit` row.
//!
//! - **Legacy revocations** (`import_legacy`, `mirror_legacy_revocations`):
//!   the asset service takes a `chimera1_` token from `accounts.toml`
//!   directly, and this service gives it tickets from its `legacy` device,
//!   so both must agree. An import carries the file's revocations over
//!   (again on every re-import, to accounts imported before; an account
//!   whose tokens are all revoked is revoked) and never re-enables what
//!   the database revoked; revoking, deleting or recovering here marks the
//!   account's legacy tokens revoked in the file.
//!
//! Nothing secret is logged: codes, recovery keys, tickets and device
//! keys never reach the access log, the audit rows or stderr.
//!
//! - **The owner's account commands** (`set_account`, `show_account`,
//!   `refresh_account`: its tickets into the feed so its clients ask again
//!   with the new rights; `delete_account`): the same admin API, which the
//!   admin page (`page`) uses too.
//!
//! Known shortcuts: seen nonces are kept in memory (a restart within 5
//! minutes forgets them); a deleted account's username is free at once
//! (the plan's 30 days aren't kept); one connection behind a mutex serves
//! every request (plenty for invite-only accounts).

pub mod http;
pub mod page;

use std::{
    collections::HashMap,
    net::IpAddr,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicI64, Ordering},
    },
};

use chimera_assets::{
    account::{CheckAnswer, CheckRequest, HubPreview, LinkAnswer, NameCheck, RecoverRequest, RedeemRequest, SignedIn, TicketRequest},
    store::{hex, sha256_hex, unhex},
    ticket::{self, Claims, Id, id_hex, parse_id},
};
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use crate::{codes, db};

/// The accounts service's loopback port by default.
pub const DEFAULT_PORT: u16 = 8741;
/// A link code's lifetime.
pub const LINK_SECS: u64 = 600;
/// An invite's lifetime by default.
pub const INVITE_DAYS: u64 = 14;

/// A clock the tests can move.
#[derive(Clone, Default)]
pub struct Clock(Arc<AtomicI64>);

impl Clock {
    pub fn now(&self) -> u64 {
        (crate::now() as i64 + self.0.load(Ordering::Relaxed)).max(0) as u64
    }

    /// Move the clock `secs` ahead.
    pub fn advance(&self, secs: i64) {
        self.0.fetch_add(secs, Ordering::Relaxed);
    }
}

/// The service's settings.
#[derive(Clone, Debug)]
pub struct Options {
    /// Failed codes or recovery keys an address (or a username) may send
    /// in an hour.
    pub failures_per_hour: usize,
    /// The hub's public `host:port`, for the invite links printed.
    pub public: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self { failures_per_hour: 10, public: None }
    }
}

/// A refusal: an HTTP status and words for the player.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub status: u16,
    pub message: String,
}

pub type R<T> = Result<T, Refusal>;

fn refuse(status: u16, message: impl Into<String>) -> Refusal {
    Refusal { status, message: message.into() }
}

fn internal(e: impl std::fmt::Display) -> Refusal {
    eprintln!("accounts: {e}");
    refuse(500, "the accounts service failed; try again later")
}

/// Where the service keeps its files in a server store.
#[derive(Clone, Debug)]
pub struct Paths {
    /// The store (`--store`, as the asset service's).
    pub store: PathBuf,
}

impl Paths {
    pub fn new(store: impl Into<PathBuf>) -> Self {
        Self { store: store.into() }
    }
    pub fn server(&self) -> PathBuf {
        self.store.join("server")
    }
    pub fn dir(&self) -> PathBuf {
        self.server().join("accounts")
    }
    pub fn db(&self) -> PathBuf {
        self.dir().join("accounts.db")
    }
    pub fn ticket_key(&self) -> PathBuf {
        self.dir().join("ticket.key")
    }
    pub fn ticket_pub(&self) -> PathBuf {
        chimera_assets::accounts::ticket_keys_path(&self.store)
    }
    pub fn secret(&self) -> PathBuf {
        chimera_assets::accounts::service_secret_path(&self.store)
    }
    pub fn backups(&self) -> PathBuf {
        self.server().join("backups")
    }
    pub fn tls(&self) -> PathBuf {
        self.server().join("tls")
    }
    pub fn access_log(&self) -> PathBuf {
        self.dir().join("access.log")
    }
}

/// The services' shared secret at `path`, made if missing.
pub fn load_or_create_secret(path: &Path) -> Result<String, String> {
    if let Ok(text) = std::fs::read_to_string(path)
        && text.trim().len() >= 32
    {
        return Ok(text.trim().to_string());
    }
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
    let secret = hex(&bytes);
    chimera_assets::store::write_atomic(path, secret.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))?;
    crate::restrict(path);
    Ok(secret)
}

/// What an invite grants.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct InviteSpec {
    /// The owner's label (what `invite revoke` names). Before intents
    /// (an invite named for its player) it was also the suggested
    /// username; empty: the recipient's or the intent's words.
    #[serde(default)]
    pub name: String,
    /// Why it was made ("for Sam, sent on Discord"): kept with the account
    /// that redeems it, so an unwanted redemption can be traced. An
    /// invite with an intent carries no username: the player chooses one.
    #[serde(default)]
    pub intent: Option<String>,
    /// Who it was meant for, in the owner's words.
    #[serde(default)]
    pub recipient: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    /// `*` or `a,b`: the asset service's content.
    #[serde(default)]
    pub content: Option<String>,
    /// `*` or `a,b`: the hub's games.
    #[serde(default)]
    pub games: Option<String>,
    #[serde(default)]
    pub list: Option<u32>,
    #[serde(default)]
    pub expires_days: Option<u64>,
    /// A re-invite into this account.
    #[serde(default)]
    pub for_username: Option<String>,
    /// With `for_username`: keep its other devices.
    #[serde(default)]
    pub keep: bool,
}

/// A minted invite (the code shown once).
#[derive(Clone, Serialize, Deserialize)]
pub struct Invited {
    pub name: String,
    pub code: String,
    pub expires: u64,
    /// `chimera-hub://<public>/<fingerprint>?code=...` when the hub's
    /// public address is known.
    pub link: Option<String>,
}

/// An invite as the owner lists it (no code).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InviteView {
    pub name: String,
    #[serde(default)]
    pub intent: Option<String>,
    #[serde(default)]
    pub recipient: Option<String>,
    pub role: String,
    pub state: String,
    pub created: u64,
    pub expires: u64,
    pub for_username: Option<String>,
}

/// A device as listed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceView {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub created: u64,
    pub last_seen: Option<u64>,
    pub revoked: bool,
}

/// An account as listed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountView {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub state: String,
    pub content: String,
    pub games: String,
    pub note: String,
    pub created: u64,
    pub last_seen: Option<u64>,
    pub devices: Vec<DeviceView>,
    /// Servers it may list at once (`list` right).
    #[serde(default)]
    pub list_slots: u32,
    /// Its match results count.
    #[serde(default)]
    pub report: bool,
    /// Its feedback is dropped.
    #[serde(default)]
    pub muted: bool,
    /// The avatar chosen (`cl_avatar`'s look byte).
    #[serde(default)]
    pub avatar: Option<u8>,
    /// Its last usernames, newest first (at most `HISTORY`): shown so
    /// nobody passes as someone who renamed.
    #[serde(default)]
    pub previous_usernames: Vec<String>,
    /// The intent (and recipient) of the invite that made it.
    #[serde(default)]
    pub invite_intent: Option<String>,
}

/// Usernames kept per account after renames.
pub const HISTORY: usize = 10;
/// Seconds between a player's renames.
pub const RENAME_SECS: u64 = 60;
/// Free names offered for a taken one.
pub const SUGGESTIONS: usize = 3;

/// What `set_account` changes (None: unchanged).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AccountChange {
    /// The account (its current username).
    pub username: String,
    #[serde(default)]
    pub new_username: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    /// tester, host or owner; its `list` and `report` defaults come with
    /// it unless `list` is given.
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub games: Option<String>,
    #[serde(default)]
    pub list: Option<u32>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub muted: Option<bool>,
}

/// One audit row as the owner reads it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditView {
    pub at: u64,
    pub actor: String,
    pub action: String,
    pub target: String,
    pub detail: String,
}

/// `account show`: the account and its latest audit rows.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountDetail {
    pub account: AccountView,
    pub audit: Vec<AuditView>,
}

/// What `import_legacy` did.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Imported {
    pub accounts: usize,
    pub devices: usize,
    pub skipped: usize,
    /// Devices of accounts imported before, revoked now because the file
    /// revoked their tokens since.
    #[serde(default)]
    pub revoked: usize,
    /// Accounts imported revoked, or revoked now (every token revoked).
    #[serde(default)]
    pub revoked_accounts: usize,
}

#[derive(Default)]
struct Guard {
    /// Failure times per `ip:<addr>` or `user:<name>`.
    failures: HashMap<String, Vec<u64>>,
    /// Nonces seen (with their request's time).
    nonces: HashMap<String, u64>,
}

/// The accounts service.
pub struct Accounts {
    pub paths: Paths,
    conn: Mutex<Connection>,
    key: SigningKey,
    secret: String,
    pub clock: Clock,
    options: Options,
    guard: Mutex<Guard>,
}

fn list_text(list: &str) -> Vec<String> {
    list.split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect()
}

fn clean_list(text: &str) -> R<String> {
    let items = list_text(text);
    let ok = items
        .iter()
        .all(|i| i == "*" || (!i.is_empty() && i.len() <= 64 && i.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')));
    if !ok {
        return Err(refuse(400, format!("{text:?}: `*` or names separated by commas")));
    }
    Ok(items.join(","))
}

fn role_defaults(role: &str) -> R<(&'static str, u32, bool)> {
    // (content, list, report)
    match role {
        "tester" => Ok(("*", 1, false)),
        "host" => Ok(("", 4, true)),
        "owner" => Ok(("*", 4, true)),
        _ => Err(refuse(400, format!("{role:?}: a role is tester, host or owner"))),
    }
}

/// The avatars (`cl_avatar`'s looks: 4 glyphs x 8 colours).
pub const AVATARS: u8 = 32;

/// An invite's row, once it is known to be usable.
struct InviteRow {
    name: String,
    role: String,
    content: String,
    games: String,
    list: i64,
    for_account: Option<Vec<u8>>,
    keep: i64,
    intent: Option<String>,
    recipient: Option<String>,
}

fn blob(id: &Id) -> Vec<u8> {
    id.to_vec()
}

fn id_of(bytes: Vec<u8>) -> Id {
    let mut id = [0u8; 16];
    let n = bytes.len().min(16);
    id[..n].copy_from_slice(&bytes[..n]);
    id
}

impl Accounts {
    /// Open the service's database (migrated), its ticket key and the
    /// services' secret, in `paths`.
    pub fn open(paths: Paths, options: Options, clock: Clock) -> Result<Self, String> {
        std::fs::create_dir_all(paths.dir()).map_err(|e| format!("{}: {e}", paths.dir().display()))?;
        let conn = db::open(&paths.db(), db::ACCOUNTS_MIGRATIONS, &paths.backups())?;
        let key = chimera_assets::sign::load_or_create_key(&paths.ticket_key())
            .map_err(|e| format!("{}: {e}", paths.ticket_key().display()))?;
        let keys = ticket::TicketKeys::single(key.verifying_key());
        chimera_assets::store::write_atomic(&paths.ticket_pub(), keys.to_text().as_bytes())
            .map_err(|e| format!("{}: {e}", paths.ticket_pub().display()))?;
        let secret = load_or_create_secret(&paths.secret())?;
        Ok(Self { paths, conn: Mutex::new(conn), key, secret, clock, options, guard: Mutex::default() })
    }

    /// The ticket public keys (what services check with).
    pub fn ticket_keys(&self) -> ticket::TicketKeys {
        ticket::TicketKeys::single(self.key.verifying_key())
    }

    /// Whether `given` is the services' secret.
    pub fn is_secret(&self, given: &str) -> bool {
        let (a, b) = (given.as_bytes(), self.secret.as_bytes());
        a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
    }

    fn db(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn now(&self) -> u64 {
        self.clock.now()
    }

    fn audit(tx: &Connection, at: u64, actor: &str, action: &str, target: &str, detail: &str) -> R<()> {
        tx.execute(
            "INSERT INTO audit (at, actor, action, target, detail) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![at as i64, actor, action, target, detail],
        )
        .map(drop)
        .map_err(internal)
    }

    /// A row of the revocation feed. Its time is at least one past the
    /// feed's latest: a ticket issued after an earlier row in this same
    /// second (`issue` dates it past that row) is still covered.
    fn revoke_row(tx: &Connection, kind: &str, target: &Id, at: u64) -> R<()> {
        let last: Option<i64> = tx.query_row("SELECT MAX(at) FROM revocations", [], |r| r.get(0)).map_err(internal)?;
        let at = last.map_or(at as i64, |l| (at as i64).max(l + 1));
        tx.execute("INSERT INTO revocations (kind, target, at) VALUES (?1, ?2, ?3)", params![kind, blob(target), at])
            .map(drop)
            .map_err(internal)
    }

    // --- Limits.

    fn failures(&self, key: &str) -> usize {
        let now = self.now();
        let mut g = self.guard.lock().unwrap_or_else(|e| e.into_inner());
        let list = g.failures.entry(key.to_string()).or_default();
        list.retain(|t| now < t + 3600);
        list.len()
    }

    fn fail(&self, keys: &[String]) {
        let now = self.now();
        let mut g = self.guard.lock().unwrap_or_else(|e| e.into_inner());
        for k in keys {
            g.failures.entry(k.clone()).or_default().push(now);
        }
    }

    fn check_limits(&self, keys: &[String]) -> R<()> {
        if keys.iter().any(|k| self.failures(k) >= self.options.failures_per_hour) {
            return Err(refuse(429, "too many wrong codes or keys: try again in an hour"));
        }
        Ok(())
    }

    fn banned_address(&self, conn: &Connection, ip: Option<IpAddr>) -> R<()> {
        let Some(ip) = ip else { return Ok(()) };
        let now = self.now() as i64;
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM bans WHERE address = ?1 AND (until IS NULL OR until > ?2)",
                params![ip.to_string(), now],
                |r| r.get(0),
            )
            .map_err(internal)?;
        if n > 0 {
            return Err(refuse(403, "this address is banned from the hub"));
        }
        Ok(())
    }

    fn banned_account(&self, conn: &Connection, account: &Id) -> R<()> {
        let now = self.now() as i64;
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM bans WHERE account_id = ?1 AND (until IS NULL OR until > ?2)",
                params![blob(account), now],
                |r| r.get(0),
            )
            .map_err(internal)?;
        if n > 0 {
            return Err(refuse(403, "this account is banned from the hub"));
        }
        Ok(())
    }

    // --- Tickets.

    /// A ticket for `device` of `account` (active, not banned, the device
    /// not revoked); notes when and from where it was asked.
    fn issue(&self, conn: &Connection, account: &Id, device: &Id, ip: Option<IpAddr>) -> R<String> {
        let now = self.now();
        let row = conn
            .query_row(
                "SELECT username, role, content, games, list_slots, report, muted, state FROM accounts WHERE id = ?1",
                params![blob(account)],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, i64>(4)?,
                        r.get::<_, i64>(5)?,
                        r.get::<_, i64>(6)?,
                        r.get::<_, String>(7)?,
                    ))
                },
            )
            .optional()
            .map_err(internal)?
            .ok_or_else(|| refuse(401, "no such account"))?;
        let (username, role, content, games, list, report, muted, state) = row;
        if state != "active" {
            return Err(refuse(401, "this account was revoked"));
        }
        self.banned_account(conn, account)?;
        self.banned_address(conn, ip)?;
        let revoked: Option<Option<i64>> = conn
            .query_row("SELECT revoked_at FROM devices WHERE id = ?1 AND account_id = ?2", params![blob(device), blob(account)], |r| {
                r.get(0)
            })
            .optional()
            .map_err(internal)?;
        match revoked {
            None => return Err(refuse(401, "no such device")),
            Some(Some(_)) => return Err(refuse(401, "this device was revoked")),
            Some(None) => {}
        }
        conn.execute(
            "UPDATE devices SET last_seen = ?1, last_address = ?2 WHERE id = ?3",
            params![now as i64, ip.map(|i| i.to_string()), blob(device)],
        )
        .map_err(internal)?;
        conn.execute("UPDATE accounts SET last_seen = ?1 WHERE id = ?2", params![now as i64, blob(account)])
            .map_err(internal)?;
        // After a refresh (`refresh_account`) in this same second the new
        // ticket must postdate it: the feed refuses those issued at or
        // before its entries.
        let last: Option<i64> = conn
            .query_row(
                "SELECT MAX(at) FROM revocations WHERE target = ?1 OR target = ?2",
                params![blob(account), blob(device)],
                |r| r.get(0),
            )
            .map_err(internal)?;
        let issued = last.map_or(now, |at| now.max(at.max(0) as u64 + 1));
        let claims = Claims {
            version: ticket::VERSION,
            key_id: String::new(),
            account: *account,
            device: *device,
            username,
            role,
            content: list_text(&content),
            games: list_text(&games),
            list: list.max(0) as u32,
            report: report != 0,
            muted: muted != 0,
            audience: ticket::SERVICES.into(),
            issued,
            expires: issued + ticket::LIFETIME_SECS,
        };
        Ok(ticket::issue(&self.key, &claims))
    }

    /// The claims of a ticket presented to this service, checked against
    /// the database too (its device and account still good).
    pub fn check_ticket(&self, text: &str) -> R<Claims> {
        let claims = self
            .ticket_keys()
            .check(text, ticket::SERVICES, self.now())
            .map_err(|e| refuse(401, e.to_string()))?;
        let conn = self.db();
        let ok: Option<(Option<i64>, String)> = conn
            .query_row(
                "SELECT d.revoked_at, a.state FROM devices d JOIN accounts a ON a.id = d.account_id WHERE d.id = ?1",
                params![blob(&claims.device)],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(internal)?;
        match ok {
            Some((None, state)) if state == "active" => {}
            _ => return Err(refuse(401, "this device or account was revoked")),
        }
        self.banned_account(&conn, &claims.account)?;
        Ok(claims)
    }

    /// `POST /accounts/ticket`: for a device's signed request, or a legacy
    /// token (`bearer`).
    pub fn ticket(&self, request: Option<&TicketRequest>, bearer: Option<&str>, ip: Option<IpAddr>) -> R<String> {
        let ip_key = format!("ip:{}", ip.map_or("-".into(), |i| i.to_string()));
        self.check_limits(std::slice::from_ref(&ip_key))?;
        let conn = self.db();
        self.banned_address(&conn, ip)?;
        if let Some(token) = bearer.filter(|t| t.starts_with(chimera_assets::accounts::INTERNAL_PREFIX)) {
            let found: Option<(Vec<u8>, Vec<u8>)> = conn
                .query_row(
                    "SELECT account_id, id FROM devices WHERE kind = 'legacy' AND token_hash = ?1",
                    params![sha256_hex(token.as_bytes())],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()
                .map_err(internal)?;
            let Some((account, device)) = found else {
                drop(conn);
                self.fail(&[ip_key]);
                return Err(refuse(401, "unknown token"));
            };
            return self.issue(&conn, &id_of(account), &id_of(device), ip);
        }
        let request = request.ok_or_else(|| refuse(400, "a signed request or a token is needed"))?;
        let device = parse_id(&request.device).ok_or_else(|| refuse(400, "a device id is 32 hex digits"))?;
        let now = self.now();
        if request.time.abs_diff(now) > ticket::REQUEST_WINDOW_SECS {
            return Err(refuse(401, "the request's time is too far from the hub's (check this computer's clock)"));
        }
        let nonce = unhex::<16>(&request.nonce).ok_or_else(|| refuse(400, "a nonce is 32 hex digits"))?;
        let signature = unhex::<64>(&request.signature).ok_or_else(|| refuse(400, "a signature is 128 hex digits"))?;
        let found: Option<(Vec<u8>, Option<Vec<u8>>)> = conn
            .query_row("SELECT account_id, public_key FROM devices WHERE id = ?1 AND kind = 'key'", params![blob(&device)], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()
            .map_err(internal)?;
        let Some((account, Some(public))) = found else {
            drop(conn);
            self.fail(&[ip_key]);
            return Err(refuse(401, "unknown device"));
        };
        let public: [u8; 32] = public.try_into().map_err(|_| internal("a device's key isn't 32 bytes"))?;
        let public = VerifyingKey::from_bytes(&public).map_err(internal)?;
        if public
            .verify_strict(&ticket::request_message(&device, request.time, &nonce), &Signature::from_bytes(&signature))
            .is_err()
        {
            drop(conn);
            self.fail(&[ip_key]);
            return Err(refuse(401, "the request's signature isn't this device's"));
        }
        {
            let mut g = self.guard.lock().unwrap_or_else(|e| e.into_inner());
            g.nonces.retain(|_, t| t.abs_diff(now) <= 2 * ticket::REQUEST_WINDOW_SECS);
            if g.nonces.insert(request.nonce.to_ascii_lowercase(), request.time).is_some() {
                return Err(refuse(401, "this request was seen already"));
            }
        }
        self.issue(&conn, &id_of(account), &device, ip)
    }

    // --- Sign-up.

    fn add_device(tx: &Transaction, account: &Id, public: &[u8; 32], label: &str, at: u64) -> R<Id> {
        let taken: i64 = tx
            .query_row("SELECT COUNT(*) FROM devices WHERE public_key = ?1", params![public.to_vec()], |r| r.get(0))
            .map_err(internal)?;
        if taken > 0 {
            return Err(refuse(409, "this device key is registered already"));
        }
        let id = ticket::new_id();
        let label: String = label.chars().filter(|c| !c.is_control()).take(64).collect();
        tx.execute(
            "INSERT INTO devices (id, account_id, kind, public_key, label, created) VALUES (?1, ?2, 'key', ?3, ?4, ?5)",
            params![blob(&id), blob(account), public.to_vec(), label, at as i64],
        )
        .map_err(internal)?;
        Ok(id)
    }

    /// Revoke `account`'s devices but `keep` (into the feed).
    fn revoke_other_devices(tx: &Transaction, account: &Id, keep: &Id, at: u64) -> R<usize> {
        let ids: Vec<Vec<u8>> = {
            let mut stmt = tx
                .prepare("SELECT id FROM devices WHERE account_id = ?1 AND id != ?2 AND revoked_at IS NULL")
                .map_err(internal)?;
            stmt.query_map(params![blob(account), blob(keep)], |r| r.get(0))
                .map_err(internal)?
                .collect::<Result<_, _>>()
                .map_err(internal)?
        };
        for id in &ids {
            tx.execute("UPDATE devices SET revoked_at = ?1 WHERE id = ?2", params![at as i64, id]).map_err(internal)?;
            Self::revoke_row(tx, "device", &id_of(id.clone()), at)?;
        }
        Ok(ids.len())
    }

    fn new_recovery(tx: &Transaction, account: &Id) -> R<String> {
        let key = codes::generate(codes::RECOVERY_LEN);
        tx.execute("UPDATE accounts SET recovery_hash = ?1 WHERE id = ?2", params![codes::hash(&key), blob(account)])
            .map_err(internal)?;
        Ok(key)
    }

    fn public_key(text: &str) -> R<[u8; 32]> {
        let bytes = unhex::<32>(text).ok_or_else(|| refuse(400, "a device key is 64 hex digits"))?;
        VerifyingKey::from_bytes(&bytes).map_err(|_| refuse(400, "not an Ed25519 public key"))?;
        Ok(bytes)
    }

    fn account_names(tx: &Connection, account: &Id) -> R<(String, String)> {
        tx.query_row("SELECT username, display_name FROM accounts WHERE id = ?1", params![blob(account)], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .map_err(internal)
    }

    /// `POST /accounts/redeem`: an invite or a link code.
    pub fn redeem(&self, request: &RedeemRequest, ip: Option<IpAddr>) -> R<SignedIn> {
        let ip_key = format!("ip:{}", ip.map_or("-".into(), |i| i.to_string()));
        self.check_limits(std::slice::from_ref(&ip_key))?;
        let public = Self::public_key(&request.device_public_key)?;
        let code = codes::normalize(&request.code).unwrap_or_default();
        let hash = codes::hash(&code).unwrap_or_default();
        let now = self.now();
        let mut conn = self.db();
        self.banned_address(&conn, ip)?;
        let tx = conn.transaction().map_err(internal)?;
        let result = match code.len() {
            codes::INVITE_LEN => self.redeem_invite(&tx, &hash, request, &public, now),
            codes::LINK_LEN => self.redeem_link(&tx, &hash, request, &public, now),
            _ => Err(refuse(404, "unknown code")),
        };
        let (account, device, recovery) = match result {
            Ok(r) => r,
            Err(e) => {
                drop(tx);
                drop(conn);
                if matches!(e.status, 404 | 410) {
                    self.fail(&[ip_key]);
                }
                return Err(e);
            }
        };
        let ticket = self.issue(&tx, &account, &device, ip)?;
        let (username, display_name) = Self::account_names(&tx, &account)?;
        tx.commit().map_err(internal)?;
        drop(conn);
        self.mirror_quietly();
        Ok(SignedIn {
            account: id_hex(&account),
            device: id_hex(&device),
            username,
            display_name,
            ticket,
            recovery_key: recovery,
        })
    }

    fn redeem_invite(
        &self,
        tx: &Transaction,
        hash: &str,
        request: &RedeemRequest,
        public: &[u8; 32],
        now: u64,
    ) -> R<(Id, Id, Option<String>)> {
        let InviteRow { name, role, content, games, list, for_account, keep, intent, recipient } = Self::usable_invite(tx, hash, now)?;
        let avatar = request.avatar.filter(|a| *a < AVATARS);
        let (account, recovery) = match for_account {
            Some(account) => {
                let account = id_of(account);
                let device = Self::add_device(tx, &account, public, &request.device_label, now)?;
                tx.execute("UPDATE accounts SET state = 'active', revoked_at = NULL WHERE id = ?1", params![blob(&account)])
                    .map_err(internal)?;
                if keep == 0 {
                    Self::revoke_other_devices(tx, &account, &device, now)?;
                }
                let recovery = Self::new_recovery(tx, &account)?;
                tx.execute(
                    "UPDATE invites SET redeemed_at = ?1, redeemed_device = ?2 WHERE code_hash = ?3",
                    params![now as i64, blob(&device), hash],
                )
                .map_err(internal)?;
                Self::audit(tx, now, &id_hex(&device), "reinvite.redeem", &id_hex(&account), "")?;
                return Ok((account, device, Some(recovery)));
            }
            None => {
                let given = request.username.clone().filter(|u| !u.trim().is_empty());
                let username = match (given, &intent) {
                    (Some(u), _) => codes::choose_username(&u).map_err(|e| refuse(400, e))?,
                    // An invite named for its player (before intents): its name.
                    (None, None) => codes::username(&name).map_err(|e| refuse(400, e))?,
                    (None, Some(_)) => return Err(refuse(400, "choose a username")),
                };
                if Self::username_taken(tx, &username, None)? {
                    return Err(refuse(409, format!("the username {username:?} is taken: pick another")));
                }
                let account = ticket::new_id();
                let (_, _, report) = role_defaults(&role)?;
                let traced = intent.as_ref().map(|i| match &recipient {
                    Some(r) => format!("{i} (for {r})"),
                    None => i.clone(),
                });
                tx.execute(
                    "INSERT INTO accounts (id, username, display_name, created, role, content, games, list_slots, report,
                                           invite_intent, avatar)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        blob(&account),
                        username,
                        username,
                        now as i64,
                        role,
                        content,
                        games,
                        list,
                        i64::from(report),
                        traced,
                        avatar.map(i64::from)
                    ],
                )
                .map_err(internal)?;
                let recovery = Self::new_recovery(tx, &account)?;
                (account, recovery)
            }
        };
        let device = Self::add_device(tx, &account, public, &request.device_label, now)?;
        tx.execute(
            "UPDATE invites SET redeemed_at = ?1, redeemed_device = ?2 WHERE code_hash = ?3",
            params![now as i64, blob(&device), hash],
        )
        .map_err(internal)?;
        Self::audit(tx, now, &id_hex(&device), "invite.redeem", &id_hex(&account), "")?;
        Ok((account, device, Some(recovery)))
    }

    /// The invite stored as `hash`, if it can still be redeemed: 404
    /// unknown, 410 revoked, used or expired.
    fn usable_invite(tx: &Connection, hash: &str, now: u64) -> R<InviteRow> {
        type Row = (String, String, String, String, i64, Option<Vec<u8>>, i64, i64, Option<i64>, Option<i64>, Option<String>, Option<String>);
        let row: Option<Row> = tx
            .query_row(
                "SELECT name, role, content, games, list_slots, for_account, keep_devices, expires, redeemed_at, revoked_at,
                        intent, recipient
                 FROM invites WHERE code_hash = ?1",
                params![hash],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        r.get(6)?,
                        r.get(7)?,
                        r.get(8)?,
                        r.get(9)?,
                        r.get(10)?,
                        r.get(11)?,
                    ))
                },
            )
            .optional()
            .map_err(internal)?;
        let Some((name, role, content, games, list, for_account, keep, expires, redeemed, revoked, intent, recipient)) = row else {
            return Err(refuse(404, "unknown code"));
        };
        if revoked.is_some() {
            return Err(refuse(410, "this invite was revoked"));
        }
        if redeemed.is_some() {
            return Err(refuse(410, "this invite was used already"));
        }
        if now as i64 >= expires {
            return Err(refuse(410, "this invite expired: ask the hub's owner for a new one"));
        }
        Ok(InviteRow { name, role, content, games, list, for_account, keep, intent, recipient })
    }

    /// Whether `username` (folded) is some account's other than `except`.
    fn username_taken(conn: &Connection, username: &str, except: Option<&Id>) -> R<bool> {
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM accounts WHERE username = ?1 AND id != ?2",
                params![username, except.map(blob).unwrap_or_default()],
                |r| r.get(0),
            )
            .map_err(internal)?;
        Ok(n > 0)
    }

    /// `wanted` checked as a chosen username: free, or why not with free
    /// suggestions.
    fn name_check(conn: &Connection, wanted: &str, except: Option<&Id>) -> R<NameCheck> {
        let folded = wanted.trim().to_ascii_lowercase();
        let taken = |n: &str| Self::username_taken(conn, n, except).unwrap_or(true);
        let suggest = || codes::suggestions(&folded, SUGGESTIONS, taken);
        Ok(match codes::choose_username(&folded) {
            Err(problem) => NameCheck { username: folded.clone(), available: false, problem: Some(problem), suggestions: Vec::new() },
            Ok(u) if Self::username_taken(conn, &u, except)? => {
                NameCheck { username: u, available: false, problem: Some("taken".into()), suggestions: suggest() }
            }
            Ok(u) => NameCheck { username: u, available: true, problem: None, suggestions: Vec::new() },
        })
    }

    /// The hub as a good code shows it: hub.toml's name and MOTD (the
    /// hub service's file in the same store), the active accounts.
    fn preview(&self, conn: &Connection) -> HubPreview {
        let config = std::fs::read_to_string(crate::hub::Paths::new(&self.paths.store).config())
            .ok()
            .and_then(|t| crate::hub::HubConfig::parse(&t).ok())
            .unwrap_or_default();
        let players: i64 = conn.query_row("SELECT COUNT(*) FROM accounts WHERE state = 'active'", [], |r| r.get(0)).unwrap_or(0);
        HubPreview {
            name: if config.name.trim().is_empty() { "this hub".into() } else { config.name.clone() },
            motd: config.motd.clone(),
            players: players.max(0) as u32,
        }
    }

    /// `POST /accounts/check`: the code's kind and the hub, and whether a
    /// username is free; nothing used up. A wrong code counts toward the
    /// address's limit as a redeem's does.
    pub fn check(&self, request: &CheckRequest, ip: Option<IpAddr>) -> R<CheckAnswer> {
        let ip_key = format!("ip:{}", ip.map_or("-".into(), |i| i.to_string()));
        self.check_limits(std::slice::from_ref(&ip_key))?;
        let code = codes::normalize(&request.code).unwrap_or_default();
        let hash = codes::hash(&code).unwrap_or_default();
        let now = self.now();
        let conn = self.db();
        self.banned_address(&conn, ip)?;
        let found = match code.len() {
            codes::INVITE_LEN => Self::usable_invite(&conn, &hash, now).and_then(|i| match i.for_account {
                Some(a) => Self::account_names(&conn, &id_of(a)).map(|(u, _)| ("reinvite", Some(u))),
                None => Ok(("invite", None)),
            }),
            codes::LINK_LEN => {
                let row: Option<(Vec<u8>, i64, Option<i64>)> = conn
                    .query_row("SELECT account_id, expires, used_at FROM link_codes WHERE code_hash = ?1", params![hash], |r| {
                        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                    })
                    .optional()
                    .map_err(internal)?;
                match row {
                    None => Err(refuse(404, "unknown code")),
                    Some((_, _, Some(_))) => Err(refuse(410, "this link code was used already")),
                    Some((_, expires, None)) if now as i64 >= expires => Err(refuse(410, "this link code expired: make a new one")),
                    Some((account, _, None)) => Self::account_names(&conn, &id_of(account)).map(|(u, _)| ("link", Some(u))),
                }
            }
            _ => Err(refuse(404, "unknown code")),
        };
        let (kind, account) = match found {
            Ok(f) => f,
            Err(e) => {
                drop(conn);
                if matches!(e.status, 404 | 410) {
                    self.fail(&[ip_key]);
                }
                return Err(e);
            }
        };
        let username = match (&request.username, kind) {
            (Some(u), "invite") => Some(Self::name_check(&conn, u, None)?),
            _ => None,
        };
        Ok(CheckAnswer { kind: kind.into(), hub: self.preview(&conn), account, username })
    }

    /// Change `account`'s username to `new` (folded, checked), keeping the
    /// old one in its history (the last `HISTORY`).
    fn rename_in(tx: &Connection, account: &Id, new: &str, now: u64) -> R<()> {
        let old: String =
            tx.query_row("SELECT username FROM accounts WHERE id = ?1", params![blob(account)], |r| r.get(0)).map_err(internal)?;
        if old == new {
            return Ok(());
        }
        if Self::username_taken(tx, new, Some(account))? {
            return Err(refuse(409, format!("the username {new:?} is taken")));
        }
        tx.execute("UPDATE accounts SET username = ?1, renamed_at = ?2 WHERE id = ?3", params![new, now as i64, blob(account)])
            .map_err(internal)?;
        tx.execute("INSERT INTO username_history (account_id, username, until) VALUES (?1, ?2, ?3)", params![blob(account), old, now as i64])
            .map_err(internal)?;
        tx.execute(
            "DELETE FROM username_history WHERE account_id = ?1 AND rowid NOT IN
               (SELECT rowid FROM username_history WHERE account_id = ?1 ORDER BY until DESC, rowid DESC LIMIT ?2)",
            params![blob(account), HISTORY as i64],
        )
        .map_err(internal)?;
        Ok(())
    }

    /// `POST /accounts/username`: the player renames, at most once a
    /// minute; the old name joins the history. Takes effect in tickets
    /// from the next one.
    pub fn rename(&self, ticket_text: &str, wanted: &str) -> R<AccountView> {
        let claims = self.check_ticket(ticket_text)?;
        let new = codes::choose_username(wanted).map_err(|e| refuse(400, e))?;
        let now = self.now();
        let mut conn = self.db();
        let last: Option<i64> = conn
            .query_row("SELECT renamed_at FROM accounts WHERE id = ?1", params![blob(&claims.account)], |r| r.get(0))
            .map_err(internal)?;
        if last.is_some_and(|t| now < t as u64 + RENAME_SECS) {
            return Err(refuse(429, "one rename a minute: try again shortly"));
        }
        let tx = conn.transaction().map_err(internal)?;
        Self::rename_in(&tx, &claims.account, &new, now)?;
        Self::audit(&tx, now, &id_hex(&claims.device), "rename", &id_hex(&claims.account), &new)?;
        tx.commit().map_err(internal)?;
        drop(conn);
        self.me(ticket_text)
    }

    /// `POST /accounts/avatar`: one of the 32 looks.
    pub fn set_avatar(&self, ticket_text: &str, avatar: u8) -> R<AccountView> {
        let claims = self.check_ticket(ticket_text)?;
        if avatar >= AVATARS {
            return Err(refuse(400, format!("an avatar is 0 to {}", AVATARS - 1)));
        }
        let conn = self.db();
        conn.execute("UPDATE accounts SET avatar = ?1 WHERE id = ?2", params![i64::from(avatar), blob(&claims.account)])
            .map_err(internal)?;
        Self::audit(&conn, self.now(), &id_hex(&claims.device), "avatar", &id_hex(&claims.account), &avatar.to_string())?;
        drop(conn);
        self.me(ticket_text)
    }

    fn redeem_link(
        &self,
        tx: &Transaction,
        hash: &str,
        request: &RedeemRequest,
        public: &[u8; 32],
        now: u64,
    ) -> R<(Id, Id, Option<String>)> {
        let row: Option<(Vec<u8>, i64, Option<i64>)> = tx
            .query_row("SELECT account_id, expires, used_at FROM link_codes WHERE code_hash = ?1", params![hash], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .optional()
            .map_err(internal)?;
        let Some((account, expires, used)) = row else {
            return Err(refuse(404, "unknown code"));
        };
        if used.is_some() {
            return Err(refuse(410, "this link code was used already"));
        }
        if now as i64 >= expires {
            return Err(refuse(410, "this link code expired: make a new one"));
        }
        let account = id_of(account);
        let device = Self::add_device(tx, &account, public, &request.device_label, now)?;
        tx.execute("UPDATE link_codes SET used_at = ?1, used_device = ?2 WHERE code_hash = ?3", params![now as i64, blob(&device), hash])
            .map_err(internal)?;
        Self::audit(tx, now, &id_hex(&device), "link.redeem", &id_hex(&account), "")?;
        Ok((account, device, None))
    }

    /// `POST /accounts/link`: a link code for the ticket's account.
    pub fn link(&self, ticket_text: &str) -> R<LinkAnswer> {
        let claims = self.check_ticket(ticket_text)?;
        let code = codes::generate(codes::LINK_LEN);
        let now = self.now();
        let expires = now + LINK_SECS;
        let conn = self.db();
        conn.execute(
            "INSERT INTO link_codes (code_hash, account_id, made_by_device, created, expires) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![codes::hash(&code), blob(&claims.account), blob(&claims.device), now as i64, expires as i64],
        )
        .map_err(internal)?;
        Self::audit(&conn, now, &id_hex(&claims.device), "link.create", &id_hex(&claims.account), "")?;
        Ok(LinkAnswer { code, expires })
    }

    /// `POST /accounts/recover`.
    pub fn recover(&self, request: &RecoverRequest, ip: Option<IpAddr>) -> R<SignedIn> {
        let by_key = request.username.trim().is_empty();
        let mut keys = vec![format!("ip:{}", ip.map_or("-".into(), |i| i.to_string()))];
        if !by_key {
            keys.push(format!("user:{}", codes::username(&request.username).map_err(|e| refuse(400, e))?));
        }
        self.check_limits(&keys)?;
        let public = Self::public_key(&request.device_public_key)?;
        let now = self.now();
        let mut conn = self.db();
        self.banned_address(&conn, ip)?;
        let given = codes::hash(&request.recovery_key);
        let found: Option<(Vec<u8>, Option<String>, String)> = if by_key {
            conn.query_row(
                "SELECT id, recovery_hash, state FROM accounts WHERE recovery_hash = ?1",
                params![given.clone().unwrap_or_default()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
        } else {
            conn.query_row(
                "SELECT id, recovery_hash, state FROM accounts WHERE username = ?1",
                params![request.username.trim().to_ascii_lowercase()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
        }
        .optional()
        .map_err(internal)?;
        let matched = match &found {
            Some((_, Some(h), state)) => given.as_deref() == Some(h.as_str()) && state == "active",
            _ => false,
        };
        if !matched {
            drop(conn);
            self.fail(&keys);
            return Err(refuse(403, if by_key { "that recovery key doesn't match an account" } else { "that username and recovery key don't match" }));
        }
        let account = id_of(found.expect("matched").0);
        self.banned_account(&conn, &account)?;
        let tx = conn.transaction().map_err(internal)?;
        let device = Self::add_device(&tx, &account, &public, &request.device_label, now)?;
        Self::revoke_other_devices(&tx, &account, &device, now)?;
        let recovery = Self::new_recovery(&tx, &account)?;
        Self::audit(&tx, now, &id_hex(&device), "recover", &id_hex(&account), "")?;
        let ticket = self.issue(&tx, &account, &device, ip)?;
        let (username, display_name) = Self::account_names(&tx, &account)?;
        tx.commit().map_err(internal)?;
        drop(conn);
        self.mirror_quietly();
        Ok(SignedIn {
            account: id_hex(&account),
            device: id_hex(&device),
            username,
            display_name,
            ticket,
            recovery_key: Some(recovery),
        })
    }

    /// `POST /accounts/display-name`: at most 20 characters.
    pub fn set_display_name(&self, ticket_text: &str, name: &str) -> R<String> {
        let claims = self.check_ticket(ticket_text)?;
        let name = codes::display_name(name).map_err(|e| refuse(400, e))?;
        let conn = self.db();
        conn.execute("UPDATE accounts SET display_name = ?1 WHERE id = ?2", params![name, blob(&claims.account)])
            .map_err(internal)?;
        Self::audit(&conn, self.now(), &id_hex(&claims.device), "display_name", &id_hex(&claims.account), "")?;
        Ok(name)
    }

    /// `GET /accounts/me`: the ticket's account.
    pub fn me(&self, ticket_text: &str) -> R<AccountView> {
        let claims = self.check_ticket(ticket_text)?;
        self.list_accounts()?
            .into_iter()
            .find(|a| a.id == id_hex(&claims.account))
            .ok_or_else(|| refuse(404, "no such account"))
    }

    /// The feed's entries after `since`: `<seq> <kind> <id hex> <at>` lines.
    pub fn revoked_since(&self, since: u64) -> R<String> {
        let conn = self.db();
        let mut stmt = conn.prepare("SELECT seq, kind, target, at FROM revocations WHERE seq > ?1 ORDER BY seq").map_err(internal)?;
        let rows = stmt
            .query_map(params![since as i64], |r| {
                Ok(format!("{} {} {} {}\n", r.get::<_, i64>(0)?, r.get::<_, String>(1)?, hex(&r.get::<_, Vec<u8>>(2)?), r.get::<_, i64>(3)?))
            })
            .map_err(internal)?;
        rows.collect::<Result<String, _>>().map_err(internal)
    }

    // --- The owner's side.

    /// Mint an invite (the code is returned once, stored hashed).
    pub fn invite(&self, spec: &InviteSpec) -> R<Invited> {
        let words = |w: &Option<String>, what: &str| -> R<Option<String>> {
            match w.as_deref().map(str::trim).filter(|w| !w.is_empty()) {
                None => Ok(None),
                Some(w) if w.chars().count() > 200 || w.chars().any(char::is_control) => {
                    Err(refuse(400, format!("an invite's {what} is at most 200 characters")))
                }
                Some(w) => Ok(Some(w.to_string())),
            }
        };
        let intent = words(&spec.intent, "intent")?;
        let recipient = words(&spec.recipient, "recipient")?;
        let label: String = match spec.name.trim() {
            "" => recipient.clone().or_else(|| intent.clone()).unwrap_or_default().chars().take(64).collect(),
            n => n.to_string(),
        };
        let name = label.as_str();
        if name.is_empty() || name.chars().count() > 64 || name.chars().any(char::is_control) {
            return Err(refuse(400, "an invite needs an intent (or a name of 1 to 64 characters)"));
        }
        // An invite with a recipient but no intent still carries no username.
        let intent = intent.or_else(|| recipient.as_ref().map(|r| format!("for {r}")));
        let role = spec.role.clone().unwrap_or_else(|| "tester".into());
        let (content, list, _) = role_defaults(&role)?;
        let content = clean_list(spec.content.as_deref().unwrap_or(content))?;
        let games = clean_list(spec.games.as_deref().unwrap_or("*"))?;
        let list = spec.list.unwrap_or(list);
        let now = self.now();
        let expires = now + spec.expires_days.unwrap_or(INVITE_DAYS).clamp(1, 365) * 86_400;
        let conn = self.db();
        let for_account = match &spec.for_username {
            Some(u) => {
                let u = codes::username(u).map_err(|e| refuse(400, e))?;
                let id: Option<Vec<u8>> = conn
                    .query_row("SELECT id FROM accounts WHERE username = ?1", params![u], |r| r.get(0))
                    .optional()
                    .map_err(internal)?;
                Some(id.ok_or_else(|| refuse(404, format!("no account {u:?}")))?)
            }
            None => None,
        };
        let code = codes::generate(codes::INVITE_LEN);
        conn.execute(
            "INSERT INTO invites (code_hash, name, role, content, games, list_slots, for_account, keep_devices, created, expires,
                                  intent, recipient)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                codes::hash(&code),
                name,
                role,
                content,
                games,
                list,
                for_account,
                i64::from(spec.keep),
                now as i64,
                expires as i64,
                intent,
                recipient
            ],
        )
        .map_err(internal)?;
        let detail = format!("role={role} content={content} games={games}{}", if spec.for_username.is_some() { " re-invite" } else { "" });
        Self::audit(&conn, now, "cli", "invite.create", name, &detail)?;
        let fingerprint = chimera_assets::tls::fingerprint_in(&self.paths.tls());
        let link = self.options.public.as_ref().map(|public| {
            chimera_assets::account::HubLink { host: public.clone(), fingerprint: fingerprint.clone(), code: Some(code.clone()) }
                .to_text(true)
        });
        Ok(Invited { name: name.to_string(), code, expires, link })
    }

    pub fn list_invites(&self) -> R<Vec<InviteView>> {
        let now = self.now() as i64;
        let conn = self.db();
        let mut stmt = conn
            .prepare(
                "SELECT i.name, i.role, i.created, i.expires, i.redeemed_at, i.revoked_at, a.username, i.intent, i.recipient
                 FROM invites i LEFT JOIN accounts a ON a.id = i.for_account ORDER BY i.created",
            )
            .map_err(internal)?;
        let rows = stmt
            .query_map([], |r| {
                let (redeemed, revoked, expires): (Option<i64>, Option<i64>, i64) = (r.get(4)?, r.get(5)?, r.get(3)?);
                let state = if revoked.is_some() {
                    "revoked"
                } else if redeemed.is_some() {
                    "redeemed"
                } else if expires <= now {
                    "expired"
                } else {
                    "unused"
                };
                Ok(InviteView {
                    name: r.get(0)?,
                    role: r.get(1)?,
                    state: state.into(),
                    created: r.get::<_, i64>(2)? as u64,
                    expires: expires as u64,
                    for_username: r.get(6)?,
                    intent: r.get(7)?,
                    recipient: r.get(8)?,
                })
            })
            .map_err(internal)?;
        rows.collect::<Result<_, _>>().map_err(internal)
    }

    /// Revoke the unused invites named `name`; how many.
    pub fn revoke_invite(&self, name: &str) -> R<usize> {
        let now = self.now();
        let conn = self.db();
        let n = conn
            .execute(
                "UPDATE invites SET revoked_at = ?1 WHERE name = ?2 AND redeemed_at IS NULL AND revoked_at IS NULL",
                params![now as i64, name.trim()],
            )
            .map_err(internal)?;
        Self::audit(&conn, now, "cli", "invite.revoke", name, &format!("{n}"))?;
        Ok(n)
    }

    pub fn list_accounts(&self) -> R<Vec<AccountView>> {
        let conn = self.db();
        let mut devices: HashMap<Vec<u8>, Vec<DeviceView>> = HashMap::new();
        {
            let mut stmt = conn
                .prepare("SELECT account_id, id, kind, label, created, last_seen, revoked_at FROM devices ORDER BY created")
                .map_err(internal)?;
            let rows = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, Vec<u8>>(0)?,
                        DeviceView {
                            id: hex(&r.get::<_, Vec<u8>>(1)?),
                            kind: r.get(2)?,
                            label: r.get(3)?,
                            created: r.get::<_, i64>(4)? as u64,
                            last_seen: r.get::<_, Option<i64>>(5)?.map(|t| t as u64),
                            revoked: r.get::<_, Option<i64>>(6)?.is_some(),
                        },
                    ))
                })
                .map_err(internal)?;
            for row in rows {
                let (account, device) = row.map_err(internal)?;
                devices.entry(account).or_default().push(device);
            }
        }
        let mut history: HashMap<Vec<u8>, Vec<String>> = HashMap::new();
        {
            let mut stmt =
                conn.prepare("SELECT account_id, username FROM username_history ORDER BY until DESC, rowid DESC").map_err(internal)?;
            let rows = stmt.query_map([], |r| Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, String>(1)?))).map_err(internal)?;
            for row in rows {
                let (account, name) = row.map_err(internal)?;
                history.entry(account).or_default().push(name);
            }
        }
        let mut stmt = conn
            .prepare(
                "SELECT id, username, display_name, role, state, content, games, note, created, last_seen,
                        list_slots, report, muted, avatar, invite_intent
                 FROM accounts ORDER BY username",
            )
            .map_err(internal)?;
        let rows = stmt
            .query_map([], |r| {
                let id: Vec<u8> = r.get(0)?;
                Ok(AccountView {
                    id: hex(&id),
                    username: r.get(1)?,
                    display_name: r.get(2)?,
                    role: r.get(3)?,
                    state: r.get(4)?,
                    content: r.get(5)?,
                    games: r.get(6)?,
                    note: r.get(7)?,
                    created: r.get::<_, i64>(8)? as u64,
                    last_seen: r.get::<_, Option<i64>>(9)?.map(|t| t as u64),
                    devices: devices.remove(&id).unwrap_or_default(),
                    list_slots: r.get::<_, i64>(10)?.max(0) as u32,
                    report: r.get::<_, i64>(11)? != 0,
                    muted: r.get::<_, i64>(12)? != 0,
                    avatar: r.get::<_, Option<i64>>(13)?.and_then(|a| u8::try_from(a).ok()),
                    previous_usernames: history.remove(&id).unwrap_or_default(),
                    invite_intent: r.get(14)?,
                })
            })
            .map_err(internal)?;
        rows.collect::<Result<_, _>>().map_err(internal)
    }

    fn account_by_username(conn: &Connection, username: &str) -> R<Id> {
        let u = username.trim().to_ascii_lowercase();
        let id: Option<Vec<u8>> = conn
            .query_row("SELECT id FROM accounts WHERE username = ?1", params![u], |r| r.get(0))
            .optional()
            .map_err(internal)?;
        id.map(id_of).ok_or_else(|| refuse(404, format!("no account {u:?}")))
    }

    /// Revoke an account and all its devices.
    pub fn revoke_account(&self, username: &str) -> R<()> {
        let now = self.now();
        let mut conn = self.db();
        let account = Self::account_by_username(&conn, username)?;
        let tx = conn.transaction().map_err(internal)?;
        tx.execute("UPDATE accounts SET state = 'revoked', revoked_at = ?1 WHERE id = ?2", params![now as i64, blob(&account)])
            .map_err(internal)?;
        tx.execute("UPDATE devices SET revoked_at = ?1 WHERE account_id = ?2 AND revoked_at IS NULL", params![now as i64, blob(&account)])
            .map_err(internal)?;
        Self::revoke_row(&tx, "account", &account, now)?;
        Self::audit(&tx, now, "cli", "account.revoke", username, "")?;
        tx.commit().map_err(internal)?;
        drop(conn);
        self.mirror_legacy_revocations(&[]).map(drop)
    }

    /// `account set`: change an account's username, display name, role,
    /// rights or note. Takes effect at its devices' next tickets (within 3
    /// hours), at once after `refresh_account`.
    pub fn set_account(&self, change: &AccountChange) -> R<AccountView> {
        let now = self.now();
        let mut conn = self.db();
        let account = Self::account_by_username(&conn, &change.username)?;
        let tx = conn.transaction().map_err(internal)?;
        let mut done = Vec::new();
        if let Some(u) = &change.new_username {
            let u = codes::username(u).map_err(|e| refuse(400, e))?;
            Self::rename_in(&tx, &account, &u, now)?;
            done.push(format!("username={u}"));
        }
        if let Some(d) = &change.display_name {
            let d = codes::display_name(d).map_err(|e| refuse(400, e))?;
            tx.execute("UPDATE accounts SET display_name = ?1 WHERE id = ?2", params![d, blob(&account)]).map_err(internal)?;
            done.push("display_name".into());
        }
        if let Some(role) = &change.role {
            let (_, list, report) = role_defaults(role)?;
            tx.execute(
                "UPDATE accounts SET role = ?1, list_slots = ?2, report = ?3 WHERE id = ?4",
                params![role, list, i64::from(report), blob(&account)],
            )
            .map_err(internal)?;
            done.push(format!("role={role}"));
        }
        if let Some(c) = &change.content {
            let c = clean_list(c)?;
            tx.execute("UPDATE accounts SET content = ?1 WHERE id = ?2", params![c, blob(&account)]).map_err(internal)?;
            done.push(format!("content={c}"));
        }
        if let Some(g) = &change.games {
            let g = clean_list(g)?;
            tx.execute("UPDATE accounts SET games = ?1 WHERE id = ?2", params![g, blob(&account)]).map_err(internal)?;
            done.push(format!("games={g}"));
        }
        if let Some(n) = change.list {
            tx.execute("UPDATE accounts SET list_slots = ?1 WHERE id = ?2", params![n, blob(&account)]).map_err(internal)?;
            done.push(format!("list={n}"));
        }
        if let Some(note) = &change.note {
            let note: String = note.chars().filter(|c| !c.is_control()).take(200).collect();
            tx.execute("UPDATE accounts SET note = ?1 WHERE id = ?2", params![note, blob(&account)]).map_err(internal)?;
            done.push("note".into());
        }
        if let Some(m) = change.muted {
            tx.execute("UPDATE accounts SET muted = ?1 WHERE id = ?2", params![i64::from(m), blob(&account)]).map_err(internal)?;
            done.push(format!("muted={m}"));
        }
        if done.is_empty() {
            return Err(refuse(400, "nothing to change"));
        }
        Self::audit(&tx, now, "cli", "account.set", &id_hex(&account), &done.join(" "))?;
        tx.commit().map_err(internal)?;
        drop(conn);
        self.account_view(&account)
    }

    fn account_view(&self, account: &Id) -> R<AccountView> {
        self.list_accounts()?
            .into_iter()
            .find(|a| a.id == id_hex(account))
            .ok_or_else(|| refuse(404, "no such account"))
    }

    /// `account show`: the account, its devices and rights, and its latest
    /// 50 audit rows.
    pub fn show_account(&self, username: &str) -> R<AccountDetail> {
        let (account, name) = {
            let conn = self.db();
            let id = Self::account_by_username(&conn, username)?;
            (id, username.trim().to_ascii_lowercase())
        };
        let view = self.account_view(&account)?;
        let conn = self.db();
        let mut stmt = conn
            .prepare(
                "SELECT at, actor, action, target, detail FROM audit
                 WHERE target = ?1 OR target = ?2 OR actor IN (SELECT lower(hex(id)) FROM devices WHERE account_id = ?3)
                 ORDER BY seq DESC LIMIT 50",
            )
            .map_err(internal)?;
        let rows = stmt
            .query_map(params![id_hex(&account), name, blob(&account)], |r| {
                Ok(AuditView {
                    at: r.get::<_, i64>(0)? as u64,
                    actor: r.get(1)?,
                    action: r.get(2)?,
                    target: r.get(3)?,
                    detail: r.get(4)?,
                })
            })
            .map_err(internal)?;
        let audit = rows.collect::<Result<_, _>>().map_err(internal)?;
        Ok(AccountDetail { account: view, audit })
    }

    /// `account refresh`: its devices' current tickets go into the
    /// revocation feed, so its clients ask again (with its rights as they
    /// are now) within one poll of the services.
    pub fn refresh_account(&self, username: &str) -> R<()> {
        let now = self.now();
        let mut conn = self.db();
        let account = Self::account_by_username(&conn, username)?;
        let tx = conn.transaction().map_err(internal)?;
        Self::revoke_row(&tx, "account", &account, now)?;
        Self::audit(&tx, now, "cli", "account.refresh", &id_hex(&account), "")?;
        tx.commit().map_err(internal)
    }

    /// `account delete`: the account, its devices, invites into it and
    /// link codes go (into the feed first, so its tickets stop within a
    /// poll); the username is free again.
    pub fn delete_account(&self, username: &str) -> R<()> {
        let now = self.now();
        let mut conn = self.db();
        let account = Self::account_by_username(&conn, username)?;
        let tx = conn.transaction().map_err(internal)?;
        let legacy = Self::legacy_hashes(&tx, &account)?;
        Self::revoke_row(&tx, "account", &account, now)?;
        // Rows that point at its devices without cascading.
        tx.execute(
            "UPDATE invites SET redeemed_device = NULL WHERE redeemed_device IN (SELECT id FROM devices WHERE account_id = ?1)",
            params![blob(&account)],
        )
        .map_err(internal)?;
        tx.execute(
            "UPDATE link_codes SET used_device = NULL WHERE used_device IN (SELECT id FROM devices WHERE account_id = ?1)",
            params![blob(&account)],
        )
        .map_err(internal)?;
        tx.execute("DELETE FROM accounts WHERE id = ?1", params![blob(&account)]).map_err(internal)?;
        Self::audit(&tx, now, "cli", "account.delete", &id_hex(&account), &username.trim().to_ascii_lowercase())?;
        tx.commit().map_err(internal)?;
        drop(conn);
        self.mirror_legacy_revocations(&legacy).map(drop)
    }

    /// Revoke one device (by its id).
    pub fn revoke_device(&self, device: &str) -> R<()> {
        let id = parse_id(device).ok_or_else(|| refuse(400, "a device id is 32 hex digits"))?;
        let now = self.now();
        let mut conn = self.db();
        let tx = conn.transaction().map_err(internal)?;
        let n = tx
            .execute("UPDATE devices SET revoked_at = ?1 WHERE id = ?2 AND revoked_at IS NULL", params![now as i64, blob(&id)])
            .map_err(internal)?;
        if n == 0 {
            return Err(refuse(404, "no such device, or revoked already"));
        }
        Self::revoke_row(&tx, "device", &id, now)?;
        Self::audit(&tx, now, "cli", "device.revoke", device, "")?;
        tx.commit().map_err(internal)?;
        drop(conn);
        self.mirror_legacy_revocations(&[]).map(drop)
    }

    /// Ban a username or an address, for `days` (none: for good).
    pub fn ban(&self, target: &str, days: Option<u64>, reason: &str) -> R<()> {
        let now = self.now();
        let until = days.map(|d| (now + d * 86_400) as i64);
        let mut conn = self.db();
        let tx = conn.transaction().map_err(internal)?;
        match target.trim().parse::<IpAddr>() {
            Ok(ip) => {
                tx.execute(
                    "INSERT INTO bans (address, reason, created, until) VALUES (?1, ?2, ?3, ?4)",
                    params![ip.to_string(), reason, now as i64, until],
                )
                .map_err(internal)?;
            }
            Err(_) => {
                let account = Self::account_by_username(&tx, target)?;
                tx.execute(
                    "INSERT INTO bans (account_id, reason, created, until) VALUES (?1, ?2, ?3, ?4)",
                    params![blob(&account), reason, now as i64, until],
                )
                .map_err(internal)?;
                Self::revoke_row(&tx, "account", &account, now)?;
            }
        }
        Self::audit(&tx, now, "cli", "ban", target, reason)?;
        tx.commit().map_err(internal)
    }

    /// One account per legacy account of `accounts.toml` (role tester, its
    /// games as content), one `legacy` device per token hash; a revoked
    /// token is a revoked device, and an account whose tokens are all
    /// revoked is imported revoked. Accounts already imported (same token
    /// hashes) are skipped, but the file's revocations still reach them:
    /// a token revoked there since revokes its device, and an account left
    /// with no live device is revoked. An import only ever revokes: it
    /// never brings back a device or an account the database revoked.
    pub fn import_legacy(&self, text: &str) -> R<Imported> {
        let legacy: chimera_assets::accounts::Accounts = toml::from_str(text).map_err(|e| refuse(400, e.to_string()))?;
        let now = self.now();
        let mut conn = self.db();
        let tx = conn.transaction().map_err(internal)?;
        let mut out = Imported::default();
        for a in &legacy.accounts {
            let mut known: Vec<(Vec<u8>, Vec<u8>, bool, bool)> = Vec::new();
            for t in &a.tokens {
                let row: Option<(Vec<u8>, Vec<u8>, Option<i64>)> = tx
                    .query_row("SELECT id, account_id, revoked_at FROM devices WHERE token_hash = ?1", params![t.hash], |r| {
                        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                    })
                    .optional()
                    .map_err(internal)?;
                if let Some((device, account, revoked_at)) = row {
                    known.push((device, account, revoked_at.is_some(), t.revoked));
                }
            }
            if !known.is_empty() {
                out.skipped += 1;
                let mut touched = Vec::new();
                for (device, account, revoked_here, revoked_there) in &known {
                    if *revoked_there && !revoked_here {
                        tx.execute("UPDATE devices SET revoked_at = ?1 WHERE id = ?2 AND revoked_at IS NULL", params![now as i64, device])
                            .map_err(internal)?;
                        Self::revoke_row(&tx, "device", &id_of(device.clone()), now)?;
                        out.revoked += 1;
                    }
                    if !touched.contains(account) {
                        touched.push(account.clone());
                    }
                }
                if a.tokens.iter().all(|t| t.revoked) {
                    for account in touched {
                        let live: i64 = tx
                            .query_row(
                                "SELECT COUNT(*) FROM devices WHERE account_id = ?1 AND revoked_at IS NULL",
                                params![account],
                                |r| r.get(0),
                            )
                            .map_err(internal)?;
                        if live == 0 {
                            let n = tx
                                .execute(
                                    "UPDATE accounts SET state = 'revoked', revoked_at = ?1 WHERE id = ?2 AND state = 'active'",
                                    params![now as i64, account],
                                )
                                .map_err(internal)?;
                            if n > 0 {
                                Self::revoke_row(&tx, "account", &id_of(account), now)?;
                                out.revoked_accounts += 1;
                            }
                        }
                    }
                }
                continue;
            }
            if a.tokens.is_empty() {
                out.skipped += 1;
                continue;
            }
            let all_revoked = a.tokens.iter().all(|t| t.revoked);
            let base = codes::username_from(&a.name);
            let mut username = base.clone();
            let mut k = 2;
            while codes::username(&username).is_err()
                || tx
                    .query_row("SELECT COUNT(*) FROM accounts WHERE username = ?1", params![username], |r| r.get::<_, i64>(0))
                    .map_err(internal)?
                    > 0
            {
                let suffix = format!("_{k}");
                username = format!("{}{suffix}", &base[..base.len().min(codes::NAME_MAX - suffix.len())]);
                k += 1;
            }
            let display: String = a.name.chars().filter(|c| !c.is_control()).take(codes::NAME_MAX).collect();
            let content = if a.games.is_empty() { "*".to_string() } else { clean_list(&a.games.join(","))? };
            let account = ticket::new_id();
            tx.execute(
                "INSERT INTO accounts (id, username, display_name, created, role, content, games, list_slots, note, state, revoked_at)
                 VALUES (?1, ?2, ?3, ?4, 'tester', ?5, '*', 1, ?6, ?7, ?8)",
                params![
                    blob(&account),
                    username,
                    display,
                    now as i64,
                    content,
                    format!("imported from accounts.toml ({})", a.name),
                    if all_revoked { "revoked" } else { "active" },
                    all_revoked.then_some(now as i64)
                ],
            )
            .map_err(internal)?;
            if all_revoked {
                out.revoked_accounts += 1;
            }
            for t in &a.tokens {
                let device = ticket::new_id();
                tx.execute(
                    "INSERT INTO devices (id, account_id, kind, token_hash, label, created, revoked_at)
                     VALUES (?1, ?2, 'legacy', ?3, ?4, ?5, ?6)",
                    params![blob(&device), blob(&account), t.hash, t.note, t.created as i64, t.revoked.then_some(now as i64)],
                )
                .map_err(internal)?;
                out.devices += 1;
            }
            out.accounts += 1;
        }
        Self::audit(&tx, now, "cli", "import-legacy", "", &format!("{out:?}"))?;
        tx.commit().map_err(internal)?;
        Ok(out)
    }

    /// The legacy token hashes of `account`'s devices (before a delete).
    fn legacy_hashes(conn: &Connection, account: &Id) -> R<Vec<String>> {
        let mut stmt = conn
            .prepare("SELECT token_hash FROM devices WHERE account_id = ?1 AND kind = 'legacy' AND token_hash IS NOT NULL")
            .map_err(internal)?;
        let rows = stmt.query_map(params![blob(account)], |r| r.get::<_, String>(0)).map_err(internal)?;
        rows.collect::<Result<_, _>>().map_err(internal)
    }

    /// Mark revoked in `accounts.toml` every legacy token whose device or
    /// account is revoked here (and `extra`, a deleted account's): the
    /// asset service takes a `chimera1_` token from that file directly, so
    /// a revocation kept in the database alone would leave the token
    /// working there. Only ever revokes. How many tokens it revoked now.
    pub fn mirror_legacy_revocations(&self, extra: &[String]) -> R<usize> {
        let mut hashes: Vec<String> = {
            let conn = self.db();
            let mut stmt = conn
                .prepare(
                    "SELECT d.token_hash FROM devices d JOIN accounts a ON a.id = d.account_id
                     WHERE d.kind = 'legacy' AND d.token_hash IS NOT NULL AND (d.revoked_at IS NOT NULL OR a.state != 'active')",
                )
                .map_err(internal)?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0)).map_err(internal)?;
            rows.collect::<Result<_, _>>().map_err(internal)?
        };
        hashes.extend(extra.iter().cloned());
        let path = chimera_assets::accounts::Accounts::path(&self.paths.store);
        if !path.exists() {
            return Ok(0);
        }
        let mut legacy = chimera_assets::accounts::Accounts::load(&path).map_err(internal)?;
        let mut n = 0;
        for t in legacy.accounts.iter_mut().flat_map(|a| a.tokens.iter_mut()) {
            if !t.revoked && hashes.contains(&t.hash) {
                t.revoked = true;
                n += 1;
            }
        }
        if n > 0 {
            legacy.save(&path).map_err(internal)?;
        }
        Ok(n)
    }

    /// After a user's own sign-in revoked devices: mirror, logging a
    /// failure instead of failing the sign-in.
    fn mirror_quietly(&self) {
        if let Err(e) = self.mirror_legacy_revocations(&[]) {
            eprintln!("accounts: legacy revocations not mirrored to accounts.toml ({})", e.message);
        }
    }

    /// An online backup to `<store>/server/backups/accounts-<YYYYMMDD>.db`
    /// (today's, replaced), checked, then the old ones pruned.
    pub fn backup(&self) -> R<PathBuf> {
        let day = (self.now() / 86_400) as i64;
        let path = db::daily_path(&self.paths.backups(), "accounts", day);
        db::backup(&self.db(), &path).map_err(internal)?;
        for gone in db::prune(&self.paths.backups(), "accounts", day) {
            eprintln!("accounts: backup {} past retention, deleted", gone.display());
        }
        Ok(path)
    }

    /// Whether today's nightly backup is due (after 04:00 UTC, none yet).
    pub fn backup_due(&self) -> bool {
        let now = self.now();
        let day = (now / 86_400) as i64;
        now % 86_400 >= 4 * 3600 && !db::daily_path(&self.paths.backups(), "accounts", day).exists()
    }

    /// The schema version (health).
    pub fn db_version(&self) -> usize {
        db::version(&self.db()).unwrap_or(0)
    }
}
