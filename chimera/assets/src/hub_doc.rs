//! The hub's discovery document and server list, both sides
//! (docs/plans/active/hub.md, section 4): the JSON the hub service
//! (`chimera_services::hub`) answers and the calls a client or a game
//! server makes, small and SQLite-free like `ticket` and `account`.
//!
//! - **The document** (`Discovery`, `GET /hub` with a ticket): the hub's
//!   name, the accounts service and its ticket keys, the server list and
//!   heartbeat endpoints, the asset service's URL with its certificate's
//!   fingerprint and manifest key, the feedback endpoint, the MOTD and the
//!   news, the oldest network version its servers take (`client.min_net`)
//!   with a download link, and `next_fingerprint` (a certificate the hub
//!   moves to). Endpoints are relative to the hub's origin unless absolute
//!   (`Discovery::url`). The body is signed with the hub's document key
//!   (the asset service's manifest key, `X-Chimera-Signature`); a client
//!   pins that key on its first fetch (`GET /hub/key`, public) and refuses
//!   a body it doesn't verify (`verify`), so a cached copy is trusted
//!   offline.
//! - **Heartbeats** (`Heartbeat`, `POST /hub/heartbeat` with a ticket that
//!   may list): a game server's info every `HEARTBEAT_SECS`; the answer
//!   (`HeartbeatAnswer`) names the address the hub lists it at and
//!   whether the hub reached its UDP port (`Reach`). `POST /hub/leave`
//!   delists at once.
//! - **The list** (`ServerList`, `GET /hub/servers` with a ticket): the
//!   live listings (`Listing`), a server the hub couldn't reach shown only
//!   to its own account, with a hint.
//! - **The client's cache** (`HubCache`): `<data_local>/chimera/hub.toml`
//!   (`CHIMERA_HUB` names another): the hub's URL and fingerprint, the
//!   pinned key, the news dismissed; the last document beside it
//!   (`hub-discovery.json` and its `.sig`), used at once at the next start.
//!
//! The calls (`fetch_document`, `fetch_servers`, `heartbeat`, `leave`)
//! block; callers run them on a thread. Errors are words for a status
//! line, with the HTTP status where there was one (`Failure`).

use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    remote::{agent, secure_url},
    store::write_atomic,
};

/// The document's format.
pub const SCHEMA: u32 = 1;
/// The header carrying the document's signature (hex).
pub const SIGNATURE_HEADER: &str = "X-Chimera-Signature";
/// How often a listed server sends a heartbeat, s.
pub const HEARTBEAT_SECS: u64 = 30;
/// A listing without a heartbeat for this long goes, s (3 missed).
pub const EXPIRY_SECS: u64 = 90;
/// The variable naming another cache file.
pub const ENV: &str = "CHIMERA_HUB";
/// Listings the hub answers at most.
pub const MAX_LISTED: usize = 500;

/// An endpoint: a path on the hub's origin, or an absolute URL with its
/// own certificate.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Endpoint {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
}

/// The accounts service and the keys its tickets check with.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountsEndpoint {
    pub url: String,
    /// Key id to public key (hex).
    #[serde(default)]
    pub ticket_keys: BTreeMap<String, String>,
}

/// The asset service: where, its certificate, its manifest key.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetsEndpoint {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    /// The manifest key (hex).
    #[serde(default)]
    pub key: String,
}

/// A news item.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct News {
    pub id: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub text: String,
}

/// What clients the hub's servers take.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientInfo {
    /// The oldest network version (`NET_VERSION`'s `netNN`) the hub's
    /// servers take; 0: any.
    #[serde(default)]
    pub min_net: u32,
    #[serde(default)]
    pub recommended_build: String,
    /// Where a newer build is.
    #[serde(default)]
    pub download: String,
}

/// The discovery document.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Discovery {
    pub schema: u32,
    pub name: String,
    /// Unix seconds.
    pub issued: u64,
    pub expires: u64,
    pub accounts: AccountsEndpoint,
    pub servers: Endpoint,
    pub register: Endpoint,
    pub leave: Endpoint,
    pub assets: AssetsEndpoint,
    pub feedback: Endpoint,
    #[serde(default)]
    pub news: Vec<News>,
    #[serde(default)]
    pub motd: String,
    #[serde(default)]
    pub client: ClientInfo,
    #[serde(default)]
    pub next_fingerprint: Option<String>,
}

impl Discovery {
    /// An endpoint's URL: `url` itself when absolute, else on `origin`.
    pub fn url(origin: &str, url: &str) -> String {
        if url.starts_with("https://") || url.starts_with("http://") {
            url.trim_end_matches('/').to_string()
        } else {
            format!("{}/{}", origin.trim_end_matches('/'), url.trim_start_matches('/'))
        }
    }

    /// Whether a client of network version `net` (`NET_VERSION`) is too
    /// old for this hub's servers.
    pub fn too_old(&self, net: &str) -> bool {
        self.client.min_net > 0 && net_number(net).is_some_and(|n| n < self.client.min_net)
    }

    /// The newest news item not in `dismissed`.
    pub fn newest_news<'a>(&'a self, dismissed: &[String]) -> Option<&'a News> {
        self.news.iter().filter(|n| !dismissed.contains(&n.id)).max_by(|a, b| a.date.cmp(&b.date))
    }
}

/// The `NN` of a `NET_VERSION` (`0.1.0/net54`: 54).
pub fn net_number(version: &str) -> Option<u32> {
    version.rsplit_once("/net")?.1.parse().ok()
}

/// A game server's heartbeat.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Heartbeat {
    /// The A2S instance id, 16 hex digits.
    pub instance: String,
    /// The game port (the listed address is the one the hub sees).
    pub port: u16,
    /// Its addresses on private networks (`ip:port`), for players behind
    /// the same router.
    #[serde(default)]
    pub lan: Vec<String>,
    pub name: String,
    /// The app (`ultimate_gun_game`).
    pub game: String,
    #[serde(default)]
    pub minigame: Option<String>,
    pub map: String,
    pub players: u8,
    pub max: u8,
    pub bots: u8,
    pub password: bool,
    pub dedicated: bool,
    /// `NET_VERSION`.
    pub net: String,
}

/// Whether the hub reached a server's UDP port.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Reach {
    #[default]
    Unknown,
    Yes,
    No,
}

/// The hub's answer to a heartbeat.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeartbeatAnswer {
    /// Where it is listed (`ip:port`).
    pub address: String,
    pub reachable: Reach,
    /// What to tell the host when unreachable.
    #[serde(default)]
    pub hint: Option<String>,
    /// The next heartbeat in, s.
    pub every: u64,
}

/// `POST /hub/leave`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Leave {
    pub instance: String,
}

/// A listed server.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Listing {
    /// `ip:port` as the hub saw it.
    pub addr: String,
    #[serde(default)]
    pub lan: Vec<String>,
    pub instance: String,
    pub name: String,
    pub game: String,
    #[serde(default)]
    pub minigame: Option<String>,
    pub map: String,
    pub players: u8,
    pub max: u8,
    pub bots: u8,
    pub password: bool,
    pub dedicated: bool,
    pub net: String,
    pub reachable: Reach,
    /// Listed since (unix seconds).
    pub since: u64,
    /// The asking account's own.
    #[serde(default)]
    pub mine: bool,
    #[serde(default)]
    pub hint: Option<String>,
}

/// `GET /hub/servers`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerList {
    pub servers: Vec<Listing>,
    /// The hub's time (unix seconds).
    pub now: u64,
}

/// `[hub]` in chimera.local.toml: a game process's own settings.
///
/// ```toml
/// [hub]
/// list = true                      # list servers this machine hosts (chimera:hub 1)
/// account = "<path>/account.toml"  # list with this account (a host's), not the user's
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HubSettings {
    #[serde(default)]
    pub list: bool,
    #[serde(default)]
    pub account: Option<PathBuf>,
}

// --- The calls.

/// Why a call failed: the HTTP status if one came, and words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub status: Option<u16>,
    pub message: String,
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.status {
            Some(s) => write!(f, "{} ({s})", self.message),
            None => f.write_str(&self.message),
        }
    }
}

fn fail(status: Option<u16>, message: impl Into<String>) -> Failure {
    Failure { status, message: message.into() }
}

/// One call: `(status, body, signature header)`.
fn call(
    method: &str,
    url: &str,
    fingerprint: Option<&str>,
    ticket: Option<&str>,
    body: Option<&[u8]>,
) -> Result<(u16, Vec<u8>, Option<String>), Failure> {
    if ticket.is_some() && !secure_url(url) {
        return Err(fail(None, format!("{url}: tickets go over HTTPS only")));
    }
    let agent = agent(fingerprint).map_err(|e| fail(None, e))?;
    let response = match (method, body) {
        ("POST", body) => {
            let mut r = agent.post(url).header("Content-Type", "application/json");
            if let Some(t) = ticket {
                r = r.header("Authorization", &format!("Bearer {t}"));
            }
            r.send(body.unwrap_or(b"{}"))
        }
        _ => {
            let mut r = agent.get(url);
            if let Some(t) = ticket {
                r = r.header("Authorization", &format!("Bearer {t}"));
            }
            r.call()
        }
    }
    .map_err(|e| fail(None, format!("the hub didn't answer ({e})")))?;
    let status = response.status().as_u16();
    let signature = response.headers().get(SIGNATURE_HEADER).and_then(|v| v.to_str().ok()).map(str::to_string);
    let mut bytes = Vec::new();
    response
        .into_body()
        .into_reader()
        .take(4 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|e| fail(Some(status), format!("the hub's answer didn't arrive whole ({e})")))?;
    if !(200..300).contains(&status) {
        let text = String::from_utf8_lossy(&bytes).trim().to_string();
        return Err(fail(Some(status), if text.is_empty() { "the hub refused".into() } else { text }));
    }
    Ok((status, bytes, signature))
}

/// `POST url` with `ticket` and a JSON body: the answer's bytes (the
/// party calls, `party_doc`).
pub fn post_json(url: &str, fingerprint: Option<&str>, ticket: &str, body: &[u8]) -> Result<Vec<u8>, Failure> {
    call("POST", url, fingerprint, Some(ticket), Some(body)).map(|(_, b, _)| b)
}

/// The hub's document key (`GET /hub/key`: hex, public).
pub fn fetch_key(origin: &str, fingerprint: Option<&str>) -> Result<String, Failure> {
    let (_, body, _) = call("GET", &Discovery::url(origin, "/hub/key"), fingerprint, None, None)?;
    let key = String::from_utf8_lossy(&body).trim().to_string();
    if key.len() != 64 || !key.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(fail(None, "the hub's key is not a 32-byte hex key"));
    }
    Ok(key)
}

/// A document's body checked against `key` (hex) and read.
pub fn verify(body: &[u8], signature: &str, key: &str) -> Result<Discovery, String> {
    crate::sign::verify(key, body, signature)
        .map_err(|_| "the hub's document doesn't verify with the hub's key (tampered, or another hub)".to_string())?;
    let doc: Discovery = serde_json::from_slice(body).map_err(|e| format!("the hub's document: {e}"))?;
    if doc.schema != SCHEMA {
        return Err(format!("the hub's document is format {} (this build reads {SCHEMA})", doc.schema));
    }
    Ok(doc)
}

/// A fetched document: what it says, its bytes and signature as served
/// (for the cache), and the key it verified with.
#[derive(Clone, Debug)]
pub struct Fetched {
    pub doc: Discovery,
    pub body: Vec<u8>,
    pub signature: String,
    pub key: String,
}

/// `GET /hub` with `ticket`, verified with `pinned` (none: the hub's key,
/// fetched and pinned now).
pub fn fetch_document(origin: &str, fingerprint: Option<&str>, ticket: &str, pinned: Option<&str>) -> Result<Fetched, Failure> {
    let (_, body, signature) = call("GET", &Discovery::url(origin, "/hub"), fingerprint, Some(ticket), None)?;
    let signature = signature.ok_or_else(|| fail(None, "the hub's document came unsigned"))?;
    let key = match pinned {
        Some(k) => k.to_string(),
        None => fetch_key(origin, fingerprint)?,
    };
    let doc = verify(&body, &signature, &key).map_err(|e| fail(None, e))?;
    Ok(Fetched { doc, body, signature, key })
}

/// `GET <servers>` with `ticket`.
pub fn fetch_servers(url: &str, fingerprint: Option<&str>, ticket: &str) -> Result<ServerList, Failure> {
    let (_, body, _) = call("GET", url, fingerprint, Some(ticket), None)?;
    serde_json::from_slice(&body).map_err(|e| fail(None, format!("the hub's list: {e}")))
}

/// `POST <register>` with `ticket`.
pub fn heartbeat(url: &str, fingerprint: Option<&str>, ticket: &str, beat: &Heartbeat) -> Result<HeartbeatAnswer, Failure> {
    let body = serde_json::to_vec(beat).map_err(|e| fail(None, e.to_string()))?;
    let (_, body, _) = call("POST", url, fingerprint, Some(ticket), Some(&body))?;
    serde_json::from_slice(&body).map_err(|e| fail(None, format!("the hub's answer: {e}")))
}

/// `POST <leave>` with `ticket`.
pub fn leave(url: &str, fingerprint: Option<&str>, ticket: &str, instance: &str) -> Result<(), Failure> {
    let body = serde_json::to_vec(&Leave { instance: instance.to_string() }).map_err(|e| fail(None, e.to_string()))?;
    call("POST", url, fingerprint, Some(ticket), Some(&body)).map(drop)
}

// --- The client's cache.

/// `hub.toml`: the hub this client knows and what it pinned.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HubCache {
    /// The hub's origin.
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    /// The document key pinned at the first fetch (hex).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// News items the player dismissed.
    #[serde(default)]
    pub dismissed: Vec<String>,
}

impl HubCache {
    /// `CHIMERA_HUB`, else the user's.
    pub fn path() -> Option<PathBuf> {
        match std::env::var_os(ENV).filter(|v| !v.is_empty()) {
            Some(p) => Some(PathBuf::from(p)),
            None => Some(dirs::data_local_dir()?.join("chimera").join("hub.toml")),
        }
    }

    pub fn load_from(path: &Path) -> Option<Self> {
        toml::from_str(&std::fs::read_to_string(path).ok()?).ok()
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        let text = toml::to_string(self).map_err(|e| e.to_string())?;
        write_atomic(path, format!("# The hub this PC knows (written by the game).\n\n{text}").as_bytes())
            .map_err(|e| format!("{}: {e}", path.display()))?;
        crate::sign::restrict(path);
        Ok(())
    }

    /// The document's file beside `path` (and its `.sig`).
    pub fn document_path(path: &Path) -> PathBuf {
        path.with_file_name("hub-discovery.json")
    }

    /// Keep a fetched document beside `path`.
    pub fn save_document(path: &Path, fetched: &Fetched) -> Result<(), String> {
        let doc = Self::document_path(path);
        write_atomic(&doc, &fetched.body).map_err(|e| format!("{}: {e}", doc.display()))?;
        let sig = doc.with_extension("sig");
        write_atomic(&sig, fetched.signature.as_bytes()).map_err(|e| format!("{}: {e}", sig.display()))
    }

    /// The document kept beside `path`, if it still verifies with the
    /// pinned key.
    pub fn cached_document(&self, path: &Path) -> Option<Discovery> {
        let doc = Self::document_path(path);
        let body = std::fs::read(&doc).ok()?;
        let signature = std::fs::read_to_string(doc.with_extension("sig")).ok()?;
        verify(&body, signature.trim(), self.key.as_deref()?).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_and_versions() {
        assert_eq!(Discovery::url("https://192.0.2.1:9000/", "/hub/servers"), "https://192.0.2.1:9000/hub/servers");
        assert_eq!(Discovery::url("https://192.0.2.1:9000", "https://203.0.113.4:443/"), "https://203.0.113.4:443");
        assert_eq!(net_number("0.1.0/net54"), Some(54));
        assert_eq!(net_number("dev"), None);
        let mut d = Discovery::default();
        assert!(!d.too_old("0.1.0/net12"));
        d.client.min_net = 54;
        assert!(d.too_old("0.1.0/net53") && !d.too_old("0.1.0/net54") && !d.too_old("0.1.0/net60"));
    }

    #[test]
    fn signed_documents_verify_and_tampered_ones_do_not() {
        let key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
        let doc = Discovery { schema: SCHEMA, name: "Test hub".into(), motd: "hello".into(), ..Default::default() };
        let body = serde_json::to_vec(&doc).unwrap();
        let sig = crate::sign::sign(&key, &body);
        let public = crate::sign::public_hex(&key);
        assert_eq!(verify(&body, &sig, &public).unwrap(), doc);
        let mut bad = body.clone();
        let at = bad.iter().position(|b| *b == b'h').unwrap();
        bad[at] = b'j';
        assert!(verify(&bad, &sig, &public).is_err());
        let other = crate::sign::public_hex(&ed25519_dalek::SigningKey::from_bytes(&[8; 32]));
        assert!(verify(&body, &sig, &other).is_err());
    }

    #[test]
    fn the_newest_news_not_dismissed() {
        let n = |id: &str, date: &str| News { id: id.into(), date: date.into(), ..Default::default() };
        let d = Discovery { news: vec![n("a", "2026-11-01"), n("b", "2026-11-14"), n("c", "2026-11-07")], ..Default::default() };
        assert_eq!(d.newest_news(&[]).unwrap().id, "b");
        assert_eq!(d.newest_news(&["b".into()]).unwrap().id, "c");
    }
}
