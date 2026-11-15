//! The owner's command line to the accounts service (docs/plans/active/hub.md,
//! 7.1): each command is one call to the running service's admin API on
//! loopback (`accounts::http`, with the services' secret from the store),
//! so the database has one writer. `Admin` is the client; the binary
//! (`chimera_accounts`) parses the words.

use std::{io::Read, time::Duration};

use chimera_assets::accounts::SERVICE_HEADER;
use serde::{Serialize, de::DeserializeOwned};

/// The admin API of the accounts service at `url` (loopback).
pub struct Admin {
    url: String,
    secret: String,
    agent: ureq::Agent,
}

impl Admin {
    pub fn new(url: &str, secret: &str) -> Self {
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(120)))
            .max_redirects(0)
            .build()
            .new_agent();
        Self { url: url.trim_end_matches('/').to_string(), secret: secret.to_string(), agent }
    }

    fn finish<T: DeserializeOwned>(&self, path: &str, response: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<T, String> {
        let response = response.map_err(|e| format!("the accounts service at {} didn't answer ({e}): is it running?", self.url))?;
        let status = response.status().as_u16();
        let mut text = String::new();
        let _ = response.into_body().into_reader().take(16 << 20).read_to_string(&mut text);
        if !(200..300).contains(&status) {
            return Err(format!("{path}: {status} {}", text.trim()));
        }
        serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))
    }

    pub fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let r = self.agent.get(&format!("{}{path}", self.url)).header(SERVICE_HEADER, &self.secret).call();
        self.finish(path, r)
    }

    pub fn post<T: DeserializeOwned>(&self, path: &str, body: &impl Serialize) -> Result<T, String> {
        let bytes = serde_json::to_vec(body).map_err(|e| e.to_string())?;
        let r = self
            .agent
            .post(&format!("{}{path}", self.url))
            .header(SERVICE_HEADER, &self.secret)
            .header("Content-Type", "application/json")
            .send(&bytes[..]);
        self.finish(path, r)
    }

    /// `GET /health`'s line, if the service answers.
    pub fn health(&self) -> Option<String> {
        let mut r = self.agent.get(&format!("{}/health", self.url)).call().ok()?;
        (r.status().as_u16() == 200).then(|| r.body_mut().read_to_string().ok()).flatten()
    }
}
