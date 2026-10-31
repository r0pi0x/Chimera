//! The asset service's client side (docs/plans/active/asset-service.md,
//! M2): a `Remote` is one server (`[assets] url` and `token` in
//! chimera.local.toml, or `CHIMERA_ASSET_URL` / `CHIMERA_ASSET_TOKEN`;
//! the console name will be `chimera:asset_url`). It fetches manifests,
//! checks their signature against the server's key (`[assets] key` /
//! `CHIMERA_ASSET_KEY` pins it; else the key the server names over HTTPS)
//! and, when `assets.lock` pins the build, their id; then saves them into
//! the local store. As a `Fetcher` it gets a record a `CachedFiles` lacks:
//! the bytes are checked against the id and size the manifest gives
//! before they are stored (`Store::put_stored`), so a tampered or wrong
//! record is refused, never handed to a loader. `batch` gets many at once
//! (`chimera_assets fetch`).
//!
//! Tokens and files go over HTTPS only: a URL must be `https://`, except
//! on this machine (`localhost`, `127.0.0.1`, `[::1]`: tests, a server on
//! the same box). The token goes only to the configured server (never
//! along a redirect).

use std::{
    collections::HashMap,
    io::{self, Read},
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

use ureq::Agent;

use crate::{
    Manifest, Store, sign,
    store::{is_id, is_name, sha256_hex, sort_builds, write_atomic},
};

pub const URL_ENV: &str = "CHIMERA_ASSET_URL";
pub const TOKEN_ENV: &str = "CHIMERA_ASSET_TOKEN";
pub const KEY_ENV: &str = "CHIMERA_ASSET_KEY";

/// Gets a record's plain bytes from elsewhere and keeps it in `store`.
pub trait Fetcher: Send + Sync {
    /// Record `id` of `size` plain bytes, checked and stored in `store`.
    fn fetch(&self, store: &Store, id: &str, size: u64) -> io::Result<Vec<u8>>;
}

/// Where the service is and who asks.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RemoteConfig {
    /// `https://host[:port]` (no trailing slash needed).
    pub url: String,
    pub token: String,
    /// The server's public key (hex), pinned.
    pub key: Option<String>,
}

impl std::fmt::Debug for RemoteConfig {
    // Never the token.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteConfig").field("url", &self.url).field("key", &self.key).finish_non_exhaustive()
    }
}

/// Whether `url`'s host is this machine.
fn is_local(url: &str) -> bool {
    let rest = url.strip_prefix("http://").unwrap_or(url);
    let host = rest.split('/').next().unwrap_or("");
    let host = host
        .strip_prefix('[')
        .map_or_else(|| host.rsplit_once(':').map_or(host, |(h, _)| h), |h| h.split(']').next().unwrap_or(""));
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

impl RemoteConfig {
    /// The URL without a trailing slash, if tokens may go there.
    pub fn base(&self) -> Result<String, String> {
        let url = self.url.trim().trim_end_matches('/');
        if url.starts_with("https://") || (url.starts_with("http://") && is_local(url)) {
            Ok(url.to_string())
        } else {
            Err(format!(
                "asset service {url:?}: tokens and files go over HTTPS only (https://...; plain http only on localhost)"
            ))
        }
    }
}

/// One server.
pub struct Remote {
    config: RemoteConfig,
    base: String,
    agent: Agent,
    key: OnceLock<Result<String, String>>,
}

fn status_error(status: u16, what: &str, body: &str) -> io::Error {
    let body = body.trim();
    let kind = match status {
        401 | 403 => io::ErrorKind::PermissionDenied,
        404 => io::ErrorKind::NotFound,
        _ => io::ErrorKind::Other,
    };
    let why = match status {
        401 => "the token was refused (missing, unknown or revoked)",
        403 => "this account may not fetch that game",
        404 => "not on the server",
        _ => "the server failed",
    };
    io::Error::new(kind, format!("{what}: HTTP {status}: {why} ({body})"))
}

impl Remote {
    pub fn new(config: RemoteConfig) -> Result<Self, String> {
        let base = config.base()?;
        let agent = Agent::config_builder()
            .http_status_as_error(false)
            .timeout_connect(Some(Duration::from_secs(10)))
            .timeout_recv_response(Some(Duration::from_secs(60)))
            .timeout_per_read(Some(Duration::from_secs(60)))
            .max_redirects(0)
            .user_agent(concat!("chimera_assets/", env!("CARGO_PKG_VERSION")))
            .build()
            .new_agent();
        Ok(Self { config, base, agent, key: OnceLock::new() })
    }

    /// The process's `Remote` for `config` (connections are kept).
    pub fn shared(config: &RemoteConfig) -> Result<Arc<Self>, String> {
        static REMOTES: Mutex<Vec<(RemoteConfig, Arc<Remote>)>> = Mutex::new(Vec::new());
        let mut remotes = REMOTES.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, r)) = remotes.iter().find(|(c, _)| c == config) {
            return Ok(r.clone());
        }
        let remote = Arc::new(Self::new(config.clone())?);
        remotes.push((config.clone(), remote.clone()));
        Ok(remote)
    }

    pub fn url(&self) -> &str {
        &self.base
    }

    /// GET (or POST `body`) `path`: status, headers wanted, body.
    fn call(&self, path: &str, body: Option<&[u8]>, want: &[&str]) -> io::Result<(u16, Vec<Option<String>>, Vec<u8>)> {
        let url = format!("{}{path}", self.base);
        let auth = format!("Bearer {}", self.config.token);
        let result = match body {
            None => self.agent.get(&url).header("Authorization", &auth).call(),
            Some(b) => {
                self.agent.post(&url).header("Authorization", &auth).header("Content-Type", "text/plain").send(b)
            }
        };
        let mut response = result.map_err(|e| io::Error::other(format!("{url}: {e}")))?;
        let status = response.status().as_u16();
        let headers =
            want.iter().map(|h| response.headers().get(*h).and_then(|v| v.to_str().ok()).map(String::from)).collect();
        let mut data = Vec::new();
        response.body_mut().as_reader().read_to_end(&mut data).map_err(|e| io::Error::other(format!("{url}: {e}")))?;
        Ok((status, headers, data))
    }

    fn text(&self, path: &str) -> io::Result<String> {
        let (status, _, body) = self.call(path, None, &[])?;
        let body = String::from_utf8_lossy(&body).into_owned();
        if status != 200 {
            return Err(status_error(status, path, &body));
        }
        Ok(body)
    }

    /// Who the token is (`<kind> <name> <token>` and its games).
    pub fn whoami(&self) -> io::Result<String> {
        self.text("/whoami")
    }

    /// The server's public key: the pinned one, else the one it names.
    pub fn server_key(&self) -> Result<String, String> {
        if let Some(k) = &self.config.key {
            return Ok(k.trim().to_string());
        }
        self.key
            .get_or_init(|| {
                let key = self.text("/key").map_err(|e| e.to_string())?.trim().to_string();
                crate::store::unhex::<32>(&key).map(|_| key).ok_or_else(|| "/key: not a key".to_string())
            })
            .clone()
    }

    /// `(game, build, manifest id)` of every build the account may fetch,
    /// oldest build first per game.
    pub fn games(&self) -> io::Result<Vec<(String, String, String)>> {
        let mut out = Vec::new();
        for line in self.text("/games")?.lines() {
            let mut parts = line.split(' ');
            if let (Some(g), Some(b), Some(id)) = (parts.next(), parts.next(), parts.next()) {
                out.push((g.to_string(), b.to_string(), id.to_string()));
            }
        }
        Ok(out)
    }

    /// `game`'s newest build on the server.
    pub fn latest_build(&self, game: &str) -> io::Result<Option<String>> {
        let mut builds: Vec<String> = self.games()?.into_iter().filter(|g| g.0 == game).map(|g| g.1).collect();
        sort_builds(&mut builds);
        Ok(builds.pop())
    }

    /// `game`'s manifest at `build`, checked (signature; `expect_id` too
    /// when given) and saved into `store` with its signature.
    pub fn fetch_manifest(
        &self,
        store: &Store,
        game: &str,
        build: &str,
        expect_id: Option<&str>,
    ) -> Result<Manifest, String> {
        if !is_name(game) || !is_name(build) {
            return Err(format!("{game:?} {build:?}: not a game and build name"));
        }
        let path = format!("/m/{game}/{build}");
        let (status, headers, bytes) = self.call(&path, None, &["X-Chimera-Signature"]).map_err(|e| e.to_string())?;
        if status != 200 {
            return Err(status_error(status, &path, &String::from_utf8_lossy(&bytes)).to_string());
        }
        let signature = headers[0].clone().ok_or_else(|| format!("{path}: no signature"))?;
        sign::verify(&self.server_key()?, &bytes, &signature).map_err(|e| format!("{path}: {e}"))?;
        let id = sha256_hex(&bytes);
        if let Some(expect) = expect_id.filter(|e| *e != id) {
            return Err(format!("{path}: manifest {id}, but assets.lock pins {expect}"));
        }
        let text = std::str::from_utf8(&bytes).map_err(|_| format!("{path}: not text"))?;
        let manifest = Manifest::parse(text).map_err(|e| format!("{path}: {e}"))?;
        if manifest.game != game || manifest.build != build {
            return Err(format!("{path}: the manifest is of {} {}", manifest.game, manifest.build));
        }
        let file = store.manifest_path(game, build);
        write_atomic(&sign::sig_path(&file), signature.as_bytes()).map_err(|e| e.to_string())?;
        write_atomic(&file, &bytes).map_err(|e| e.to_string())?;
        Ok(manifest)
    }

    /// Record `id` as stored on the server (zstd), unchecked.
    pub fn record(&self, id: &str) -> io::Result<Vec<u8>> {
        if !is_id(id) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("{id:?} is not a record id")));
        }
        let path = format!("/r/{id}");
        let (status, _, bytes) = self.call(&path, None, &[])?;
        if status != 200 {
            return Err(status_error(status, &path, &String::from_utf8_lossy(&bytes)));
        }
        Ok(bytes)
    }

    /// Many records at once: each id with its status and stored bytes
    /// (unchecked; empty unless 200).
    pub fn batch(&self, ids: &[String]) -> io::Result<Vec<(String, u16, Vec<u8>)>> {
        let body = ids.join("\n");
        let (status, _, data) = self.call("/batch", Some(body.as_bytes()), &[])?;
        if status != 200 {
            return Err(status_error(status, "/batch", &String::from_utf8_lossy(&data)));
        }
        let bad = || io::Error::new(io::ErrorKind::InvalidData, "/batch: a broken answer");
        let mut out = Vec::with_capacity(ids.len());
        let mut rest = data.as_slice();
        while !rest.is_empty() {
            let nl = rest.iter().position(|b| *b == b'\n').ok_or_else(bad)?;
            let head = std::str::from_utf8(&rest[..nl]).map_err(|_| bad())?;
            let mut parts = head.split(' ');
            let (Some(id), Some(status), Some(len)) = (parts.next(), parts.next(), parts.next()) else {
                return Err(bad());
            };
            let status: u16 = status.parse().map_err(|_| bad())?;
            let len: usize = len.parse().map_err(|_| bad())?;
            let body = rest.get(nl + 1..nl + 1 + len).ok_or_else(bad)?;
            out.push((id.to_string(), status, body.to_vec()));
            rest = &rest[nl + 1 + len..];
        }
        Ok(out)
    }

    /// Fetch whichever of `wanted` (id, plain size) `store` lacks, in
    /// batches; returns (records fetched, bytes transferred, failures).
    pub fn fill(&self, store: &Store, wanted: &[(String, u64)]) -> (usize, u64, Vec<String>) {
        let sizes: HashMap<&str, u64> = wanted.iter().map(|(id, s)| (id.as_str(), *s)).collect();
        let missing: Vec<String> = wanted.iter().filter(|(id, _)| !store.has(id)).map(|(id, _)| id.clone()).collect();
        let (mut n, mut bytes, mut failed) = (0, 0, Vec::new());
        for chunk in missing.chunks(256) {
            match self.batch(chunk) {
                Ok(items) => {
                    for (id, status, data) in items {
                        let Some(size) = sizes.get(id.as_str()) else { continue };
                        if status != 200 {
                            failed.push(format!("{id}: HTTP {status}"));
                            continue;
                        }
                        bytes += data.len() as u64;
                        match store.put_stored(&id, *size, &data) {
                            Ok(_) => n += 1,
                            Err(e) => failed.push(e.to_string()),
                        }
                    }
                }
                Err(e) => failed.extend(chunk.iter().map(|id| format!("{id}: {e}"))),
            }
        }
        (n, bytes, failed)
    }
}

impl Fetcher for Remote {
    fn fetch(&self, store: &Store, id: &str, size: u64) -> io::Result<Vec<u8>> {
        let packed = self.record(id)?;
        store.put_stored(id, size, &packed)
    }
}
