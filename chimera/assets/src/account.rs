//! A player's platform account on this machine (docs/plans/active/hub.md,
//! 5.1-5.3): the client side of the accounts service (`chimera_services`).
//!
//! - **The hub link** (`HubLink`): `chimera-hub://<host>:<port>/<fingerprint>`
//!   with `?code=<invite or link code>`, as the owner sends it privately.
//!   The fingerprint pins the hub's certificate (`tls::PinnedTls`); a link
//!   without one uses the usual authorities, or plain HTTP on this machine
//!   (tests).
//! - **Redeeming** (`redeem`): a fresh Ed25519 key pair made here; only its
//!   public half goes to `POST /accounts/redeem` with the code and the
//!   username (none: the invite's). The answer: the account and device ids,
//!   a ticket and, for a new account or a re-invite, the **recovery key**
//!   (shown once). `recover` does the same with a username and recovery
//!   key; `link_code` asks a code that signs another PC into this account.
//! - **The file** (`AccountFile`): `<data_local>/chimera/account.toml`
//!   (`CHIMERA_ACCOUNT` names another), mode 0600 on Unix: the hub's URL
//!   and fingerprint, the asset service's (the same origin unless given),
//!   the ids, username and display name, the device's private key and the
//!   last ticket. Its `Debug` never shows the key or the ticket.
//! - **Tickets** (`Session::ticket`): the cached one until half its life
//!   is gone, then a new one asked with a request the device key signs
//!   (`ticket::request_message`); when accounts doesn't answer, the old one
//!   while it is valid, asked again at most once a minute.
//!   `Remote` sends the live session's current ticket (`fresh_ticket`), and
//!   `AssetsConfig::resolve` uses a signed-in account's asset service
//!   before a bound legacy token (`user_remote`).
//!
//! Known shortcuts: the private key sits in the file in the clear (0600 on
//! Unix; DPAPI on Windows later); no UI yet (S3): `+chimera:hub <link>
//! [username]` redeems at start and writes the recovery key beside the
//! file (`recovery-key.txt`), never to the console.

use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use ed25519_dalek::{Signer, SigningKey};
use serde::{Deserialize, Serialize};

use crate::{
    remote::{RemoteConfig, agent, secure_url},
    store::{hex, unhex, write_atomic},
    ticket,
};

/// The variable naming another account file.
pub const ENV: &str = "CHIMERA_ACCOUNT";
/// The link's scheme.
pub const SCHEME: &str = "chimera-hub://";
/// How soon a failed refresh is tried again.
const RETRY: Duration = Duration::from_secs(60);

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// A hub link: where the hub is, its certificate, perhaps a code.
#[derive(Clone, PartialEq, Eq)]
pub struct HubLink {
    /// `host:port`.
    pub host: String,
    pub fingerprint: Option<String>,
    pub code: Option<String>,
}

impl std::fmt::Debug for HubLink {
    // Never the code.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HubLink").field("host", &self.host).field("fingerprint", &self.fingerprint).finish_non_exhaustive()
    }
}

impl HubLink {
    pub fn parse(text: &str) -> Result<Self, String> {
        let bad = || "not a hub link (chimera-hub://<host>:<port>/<fingerprint>?code=<code>)".to_string();
        let rest = text.trim().strip_prefix(SCHEME).ok_or_else(bad)?;
        let (rest, query) = rest.split_once('?').unwrap_or((rest, ""));
        let (host, fingerprint) = rest.split_once('/').unwrap_or((rest, ""));
        if host.is_empty() || !host.contains(':') || host.contains(['@', ' ']) {
            return Err(bad());
        }
        let fingerprint = match fingerprint.trim_end_matches('/') {
            "" => None,
            f => Some(crate::tls::normalize_fingerprint(f).ok_or("the link's certificate fingerprint is malformed")?),
        };
        let code = query.split('&').find_map(|p| p.strip_prefix("code=")).filter(|c| !c.is_empty()).map(String::from);
        Ok(Self { host: host.to_string(), fingerprint, code })
    }

    /// The hub's origin: `https://host:port`, or `http://` on this machine
    /// without a fingerprint.
    pub fn url(&self) -> String {
        let http = format!("http://{}", self.host);
        if self.fingerprint.is_none() && secure_url(&http) { http } else { format!("https://{}", self.host) }
    }

    /// The link as text (without the code unless asked).
    pub fn to_text(&self, with_code: bool) -> String {
        let mut out = format!("{SCHEME}{}/{}", self.host, self.fingerprint.as_deref().unwrap_or(""));
        if let (true, Some(code)) = (with_code, &self.code) {
            out.push_str(&format!("?code={code}"));
        }
        out
    }
}

// --- The protocol (JSON), shared with the accounts service.

/// `POST /accounts/redeem`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RedeemRequest {
    pub code: String,
    #[serde(default)]
    pub username: Option<String>,
    /// Hex.
    pub device_public_key: String,
    #[serde(default)]
    pub device_label: String,
}

/// `POST /accounts/recover`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RecoverRequest {
    pub username: String,
    pub recovery_key: String,
    pub device_public_key: String,
    #[serde(default)]
    pub device_label: String,
}

/// The answer to a redeem or a recovery.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct SignedIn {
    pub account: String,
    pub device: String,
    pub username: String,
    pub display_name: String,
    pub ticket: String,
    /// A new recovery key (a new account, a re-invite, a recovery): shown once.
    #[serde(default)]
    pub recovery_key: Option<String>,
}

/// `POST /accounts/ticket` (a device's signed request).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TicketRequest {
    pub device: String,
    pub time: u64,
    pub nonce: String,
    pub signature: String,
}

/// A ticket's answer.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct TicketAnswer {
    pub ticket: String,
}

/// `POST /accounts/link`'s answer.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct LinkAnswer {
    pub code: String,
    pub expires: u64,
}

// --- The file.

/// The account file.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountFile {
    /// The hub's origin (the accounts service's routes are under `/accounts/`).
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    /// The asset service, when not the hub's origin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assets_url: Option<String>,
    pub account: String,
    pub device: String,
    pub username: String,
    #[serde(default)]
    pub display_name: String,
    /// The device's Ed25519 private key (hex): never leaves this file.
    pub device_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ticket: Option<String>,
}

impl std::fmt::Debug for AccountFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountFile")
            .field("url", &self.url)
            .field("fingerprint", &self.fingerprint)
            .field("assets_url", &self.assets_url)
            .field("account", &self.account)
            .field("device", &self.device)
            .field("username", &self.username)
            .finish_non_exhaustive()
    }
}

impl AccountFile {
    /// `CHIMERA_ACCOUNT`, else the user's.
    pub fn path() -> Option<PathBuf> {
        match std::env::var_os(ENV).filter(|v| !v.is_empty()) {
            Some(p) => Some(PathBuf::from(p)),
            None => Some(dirs::data_local_dir()?.join("chimera").join("account.toml")),
        }
    }

    pub fn load_from(path: &Path) -> Result<Option<Self>, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map(Some).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        let text = format!(
            "# This PC's account on a Chimera hub. The device key is a secret: never share this file.\n\n{}",
            toml::to_string(self).map_err(|e| e.to_string())?
        );
        write_atomic(path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))?;
        crate::sign::restrict(path);
        Ok(())
    }

    fn key(&self) -> Result<SigningKey, String> {
        unhex::<32>(&self.device_key).map(|b| SigningKey::from_bytes(&b)).ok_or("the account file's device key is malformed".into())
    }

    /// The asset service with the current ticket.
    pub fn remote(&self) -> Option<RemoteConfig> {
        Some(RemoteConfig {
            url: self.assets_url.clone().unwrap_or_else(|| self.url.clone()),
            token: self.ticket.clone()?,
            key: None,
            fingerprint: self.fingerprint.clone(),
        })
    }
}

// --- Calls.

fn post<T: serde::de::DeserializeOwned>(
    url: &str,
    fingerprint: Option<&str>,
    path: &str,
    bearer: Option<&str>,
    body: &impl Serialize,
) -> Result<T, String> {
    if !secure_url(url) {
        return Err(format!("{url}: accounts go over HTTPS only"));
    }
    let agent = agent(fingerprint)?;
    let full = format!("{}{path}", url.trim_end_matches('/'));
    let mut request = agent.post(&full).header("Content-Type", "application/json");
    if let Some(b) = bearer {
        request = request.header("Authorization", &format!("Bearer {b}"));
    }
    let body = serde_json::to_vec(body).map_err(|e| e.to_string())?;
    let response = request.send(&body[..]).map_err(|e| format!("the hub at {url} didn't answer ({e})"))?;
    let status = response.status().as_u16();
    let mut text = String::new();
    let _ = response.into_body().into_reader().take(64 * 1024).read_to_string(&mut text);
    if !(200..300).contains(&status) {
        return Err(format!("the hub refused: {} ({status})", text.trim()));
    }
    serde_json::from_str(&text).map_err(|e| format!("the hub's answer: {e}"))
}

fn new_key() -> SigningKey {
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).expect("the OS's randomness");
    SigningKey::from_bytes(&seed)
}

/// A new account file from a redeem or recovery answer.
fn signed_in(link: &HubLink, key: &SigningKey, answer: SignedIn) -> (AccountFile, Option<String>) {
    let file = AccountFile {
        url: link.url(),
        fingerprint: link.fingerprint.clone(),
        assets_url: None,
        account: answer.account,
        device: answer.device,
        username: answer.username,
        display_name: answer.display_name,
        device_key: hex(&key.to_bytes()),
        ticket: Some(answer.ticket),
    };
    (file, answer.recovery_key)
}

/// This PC's label for its device.
pub fn device_label() -> String {
    let host = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_default();
    let os = std::env::consts::OS;
    if host.is_empty() { os.to_string() } else { format!("{os} {}", host.chars().take(32).collect::<String>()) }
}

/// Redeem `code` (an invite or a link code) at `link`'s hub as
/// `username` (none: the invite's name). The new file and, for a new
/// account or a re-invite, the recovery key.
pub fn redeem(link: &HubLink, code: &str, username: Option<&str>, label: &str) -> Result<(AccountFile, Option<String>), String> {
    let key = new_key();
    let request = RedeemRequest {
        code: code.to_string(),
        username: username.map(String::from),
        device_public_key: hex(key.verifying_key().as_bytes()),
        device_label: label.to_string(),
    };
    let answer: SignedIn = post(&link.url(), link.fingerprint.as_deref(), "/accounts/redeem", None, &request)?;
    Ok(signed_in(link, &key, answer))
}

/// Recover `username` with its recovery key on a new PC (the account's
/// other devices are revoked); the new file and the new recovery key.
pub fn recover(link: &HubLink, username: &str, recovery_key: &str, label: &str) -> Result<(AccountFile, Option<String>), String> {
    let key = new_key();
    let request = RecoverRequest {
        username: username.to_string(),
        recovery_key: recovery_key.to_string(),
        device_public_key: hex(key.verifying_key().as_bytes()),
        device_label: label.to_string(),
    };
    let answer: SignedIn = post(&link.url(), link.fingerprint.as_deref(), "/accounts/recover", None, &request)?;
    Ok(signed_in(link, &key, answer))
}

/// A new ticket for `file`'s device (its key signs the request).
pub fn request_ticket(file: &AccountFile) -> Result<String, String> {
    let key = file.key()?;
    let device = ticket::parse_id(&file.device).ok_or("the account file's device id is malformed")?;
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;
    let time = now();
    let signature = key.sign(&ticket::request_message(&device, time, &nonce));
    let request = TicketRequest {
        device: file.device.clone(),
        time,
        nonce: hex(&nonce),
        signature: hex(&signature.to_bytes()),
    };
    let answer: TicketAnswer = post(&file.url, file.fingerprint.as_deref(), "/accounts/ticket", None, &request)?;
    Ok(answer.ticket)
}

/// A ticket for a legacy token's device (`Authorization: Bearer chimera1_...`).
pub fn legacy_ticket(url: &str, fingerprint: Option<&str>, token: &str) -> Result<String, String> {
    let answer: TicketAnswer = post(url, fingerprint, "/accounts/ticket", Some(token), &serde_json::json!({}))?;
    Ok(answer.ticket)
}

/// A link code that signs another PC into `file`'s account (10 minutes,
/// once).
pub fn link_code(file: &AccountFile, ticket: &str) -> Result<LinkAnswer, String> {
    post(&file.url, file.fingerprint.as_deref(), "/accounts/link", Some(ticket), &serde_json::json!({}))
}

// --- The session.

/// A signed-in account in this process: its file and its current ticket.
pub struct Session {
    path: Option<PathBuf>,
    file: Mutex<AccountFile>,
    failed: Mutex<Option<Instant>>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session").field("path", &self.path).finish_non_exhaustive()
    }
}

impl Session {
    /// A session of `file`, saved to `path` when its ticket changes.
    pub fn new(file: AccountFile, path: Option<PathBuf>) -> Self {
        Self { path, file: Mutex::new(file), failed: Mutex::new(None) }
    }

    /// The user's account file's session, if signed in.
    pub fn load() -> Result<Option<Self>, String> {
        let Some(path) = AccountFile::path() else { return Ok(None) };
        Ok(AccountFile::load_from(&path)?.map(|f| Self::new(f, Some(path))))
    }

    pub fn file(&self) -> AccountFile {
        self.file.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// The current ticket: the cached one until half its life is gone,
    /// then a new one; the old one while still valid if accounts doesn't
    /// answer.
    pub fn ticket(&self) -> Result<String, String> {
        let now = now();
        let current = self.file().ticket.and_then(|t| Some((ticket::peek(&t).ok()?, t)));
        if let Some((claims, t)) = &current
            && now < claims.issued + (claims.expires - claims.issued) / 2
        {
            return Ok(t.clone());
        }
        let recent_failure = self.failed.lock().unwrap_or_else(|e| e.into_inner()).is_some_and(|t| t.elapsed() < RETRY);
        let fresh = if recent_failure { Err("the accounts service didn't answer lately".into()) } else { self.refresh() };
        match (fresh, current) {
            (Ok(t), _) => Ok(t),
            (Err(_), Some((claims, t))) if now < claims.expires => Ok(t),
            (Err(e), _) => Err(e),
        }
    }

    /// Ask a new ticket now (and keep it).
    pub fn refresh(&self) -> Result<String, String> {
        match request_ticket(&self.file()) {
            Ok(t) => {
                *self.failed.lock().unwrap_or_else(|e| e.into_inner()) = None;
                let mut file = self.file.lock().unwrap_or_else(|e| e.into_inner());
                file.ticket = Some(t.clone());
                if let Some(path) = &self.path {
                    file.save_to(path)?;
                }
                Ok(t)
            }
            Err(e) => {
                *self.failed.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
                Err(e)
            }
        }
    }

    /// The asset service with a current ticket.
    pub fn remote(&self) -> Option<RemoteConfig> {
        let ticket = self.ticket().ok()?;
        Some(RemoteConfig { token: ticket, ..self.file().remote()? })
    }
}

static LIVE: RwLock<Option<Arc<Session>>> = RwLock::new(None);

/// This process's signed-in session (`fresh_ticket` uses it).
pub fn set_live(session: Option<Arc<Session>>) {
    *LIVE.write().unwrap_or_else(|e| e.into_inner()) = session;
}

pub fn live() -> Option<Arc<Session>> {
    LIVE.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// For a ticket of the live session's device, its current ticket.
pub fn fresh_ticket(token: &str) -> Option<String> {
    if !token.starts_with(ticket::PREFIX) {
        return None;
    }
    let session = live()?;
    let device = ticket::peek(token).ok()?.device;
    (ticket::parse_id(&session.file().device) == Some(device)).then(|| session.ticket().ok()).flatten()
}

/// The user's signed-in account's asset service (and the session made
/// live), if there is one.
pub fn user_remote() -> Option<RemoteConfig> {
    let session = match live() {
        Some(s) => s,
        None => {
            let s = Arc::new(Session::load().unwrap_or_else(|e| {
                eprintln!("account: {e}");
                None
            })?);
            set_live(Some(s.clone()));
            s
        }
    };
    session.remote()
}

/// `+chimera:hub <link> [username]`: redeem the link's code, keep the
/// account file at `path` and the recovery key beside it
/// (`recovery-key.txt`, 0600); what to tell the player.
pub fn join_hub(link: &str, username: Option<&str>, path: &Path) -> Result<String, String> {
    let link = HubLink::parse(link)?;
    let code = link.code.clone().ok_or("this hub link has no code: ask the hub's owner for an invite")?;
    let (file, recovery) = redeem(&link, &code, username, &device_label())?;
    file.save_to(path)?;
    let mut said = format!("signed in to the hub at {} as {} ({})", link.host, file.username, path.display());
    if let Some(key) = recovery {
        let at = path.with_file_name("recovery-key.txt");
        let text = format!(
            "Your recovery key for {} on the hub at {}. Keep it somewhere safe: it signs you in on a new PC.\n{key}\n",
            file.username, link.host
        );
        write_atomic(&at, text.as_bytes()).map_err(|e| format!("{}: {e}", at.display()))?;
        crate::sign::restrict(&at);
        said.push_str(&format!("; your recovery key is in {} (save it elsewhere)", at.display()));
    }
    set_live(Some(Arc::new(Session::new(file, Some(path.to_path_buf())))));
    Ok(said)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hub_links_parse() {
        let fp = "ab".repeat(32);
        let l = HubLink::parse(&format!("chimera-hub://hub.example:27080/{fp}?code=ABC-DEF-GHJ")).unwrap();
        assert_eq!((l.host.as_str(), l.fingerprint.as_deref(), l.code.as_deref()), ("hub.example:27080", Some(fp.as_str()), Some("ABC-DEF-GHJ")));
        assert_eq!(l.url(), "https://hub.example:27080");
        assert!(!format!("{l:?}").contains("ABC"), "Debug hides the code");
        assert_eq!(HubLink::parse(&l.to_text(true)).unwrap(), l);
        let local = HubLink::parse("chimera-hub://127.0.0.1:9/").unwrap();
        assert_eq!((local.url().as_str(), local.code), ("http://127.0.0.1:9", None));
        assert!(HubLink::parse("https://hub:1/").is_err());
        assert!(HubLink::parse("chimera-hub://hub/").is_err(), "a port");
        assert!(HubLink::parse("chimera-hub://hub:1/xyz").is_err(), "a bad fingerprint");
    }

    #[test]
    fn the_account_files_debug_hides_its_secrets() {
        let f = AccountFile { device_key: "11".repeat(32), ticket: Some("chimera2t.secret".into()), ..Default::default() };
        let text = format!("{f:?}");
        assert!(!text.contains("1111") && !text.contains("secret"), "{text}");
    }
}
