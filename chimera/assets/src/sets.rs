//! Fetching whole sets from the asset service, as the game does it
//! (docs/plans/active/asset-service.md, "Playtest delivery"): the files a
//! join's offer names (`chimera_net::net::assets`), the later maps it
//! prefetches (at a `Pace`), and what the games read at startup
//! (`STARTUP_SETS`, fetched behind the client's progress window). `fetch`
//! reads each game's manifest (fetched when the store lacks it), the sets'
//! records the store lacks, and fills them with a `progress::Transfer` the
//! caller can watch; `missing` says, from the store alone (no network),
//! whether a fetch would have anything to do; `explain` turns a service's
//! error into what a player can act on.

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use crate::{AssetSettings, Manifest, pace::Pace, progress::Transfer};

/// The sets a game reads at startup: what its client fetches before the
/// game's window when the store lacks them.
pub const STARTUP_SETS: &[&str] = &["base"];

/// The sets `sets` of `game`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Wanted {
    pub game: String,
    pub sets: Vec<String>,
}

/// How a fetch goes.
#[derive(Clone, Copy, Default)]
pub struct Options<'a> {
    /// Read at this rate (held while paused, ended when stopped); None:
    /// at full speed on several connections.
    pub pace: Option<&'a Pace>,
    /// Games the service doesn't list are skipped, not an error (the
    /// startup fetch: an app's games the service doesn't serve come from
    /// elsewhere or not at all).
    pub skip_unlisted: bool,
}

/// `wanted`'s records the store lacks, fetched from the service; the
/// transfer (files and bytes in all, so far) put into `shared` once it is
/// known. Ok: the summary line; Err: why, in a player's words (`explain`).
pub fn fetch(
    settings: &AssetSettings,
    wanted: &[Wanted],
    what: &str,
    shared: &Mutex<Option<Arc<Transfer>>>,
    options: Options<'_>,
) -> Result<String, String> {
    let url = settings.remote.as_ref().map(|r| r.url.clone()).unwrap_or_default();
    let remote = settings.remote().ok_or_else(|| explain(&url, "not usable"))?;
    remote.whoami().map_err(|e| explain(&url, &e.to_string()))?;
    let listed: Option<HashSet<String>> = if options.skip_unlisted {
        Some(
            remote
                .games()
                .map_err(|e| explain(&url, &e.to_string()))?
                .into_iter()
                .map(|g| g.0)
                .collect(),
        )
    } else {
        None
    };
    let store = settings.store.clone().ok_or("no asset store folder")?;
    let mut records = Vec::new();
    let mut seen = HashSet::new();
    let mut absent = Vec::new();
    for g in wanted {
        if listed.as_ref().is_some_and(|l| !l.contains(&g.game)) {
            continue;
        }
        let build = settings
            .build(&g.game)
            .ok_or_else(|| format!("the asset service at {url} has no {} (or refused it: the log says)", g.game))?;
        let manifest =
            Manifest::load(&store.manifest_path(&g.game, &build)).map_err(|e| format!("{}: {e}", g.game))?;
        for set in &g.sets {
            let Some(paths) = manifest.sets.get(set) else {
                absent.push(format!("{}:{set}", g.game));
                continue;
            };
            for p in paths {
                if let Some(r) = manifest.files.get(p)
                    && seen.insert(r.id.clone())
                    && !store.has(&r.id)
                {
                    records.push((r.id.clone(), r.size, r.stored));
                }
            }
        }
    }
    let bytes = records.iter().map(|w| w.2).sum();
    let transfer = Arc::new(Transfer::with_live(what, records.len() as u64, bytes, false));
    *shared.lock().unwrap_or_else(|e| e.into_inner()) = Some(transfer.clone());
    let (_, _, failed) = remote.fill_paced(&store, &records, Some(&transfer), options.pace);
    if options.pace.is_some_and(Pace::stopped) {
        return Err("stopped".into());
    }
    if !failed.is_empty() {
        let refused = failed.iter().any(|f| ["HTTP 401", "HTTP 403", "HTTP 429"].iter().any(|s| f.contains(s)));
        if refused || failed.len() == records.len() {
            return Err(explain(&url, &failed[0]));
        }
        eprintln!("{} of the {what} didn't arrive (they are fetched again as needed): {}", failed.len(), failed[0]);
    }
    let mut summary = transfer.finish();
    if !absent.is_empty() {
        summary.push_str(&format!(" (not on the service, so fetched as needed: {})", absent.join(", ")));
    }
    Ok(summary)
}

/// Whether `fetch` of `wanted` would have something to fetch, from the
/// store alone: a game whose manifest the store lacks (at the pinned
/// build, else its newest), or a set's record it lacks. A set the
/// manifest doesn't name has nothing to fetch.
pub fn missing(settings: &AssetSettings, wanted: &[Wanted]) -> bool {
    let Some(store) = settings.store.as_ref() else {
        return false;
    };
    wanted.iter().any(|g| {
        let pinned = settings
            .builds
            .get(&g.game)
            .cloned()
            .or_else(|| settings.lock.as_ref().and_then(|l| l.games.get(&g.game)).map(|p| p.build.clone()));
        let Some(build) = pinned.or_else(|| store.latest_build(&g.game)) else {
            return true;
        };
        let Ok(manifest) = Manifest::load(&store.manifest_path(&g.game, &build)) else {
            return true;
        };
        g.sets.iter().filter_map(|s| manifest.sets.get(s)).flatten().any(|p| {
            manifest.files.get(p).is_some_and(|r| !store.has(&r.id))
        })
    })
}

/// A service error (`e`, from the service at `url`) as what a player can
/// act on.
pub fn explain(url: &str, e: &str) -> String {
    if e.contains("HTTP 401") {
        "the asset service refused your token (revoked or mistyped): ask the server's admin for a new one and \
         enter it with +chimera:asset_token <token>"
            .to_string()
    } else if e.contains("HTTP 403") {
        format!("your account on the asset service may not fetch this game ({e})")
    } else if e.contains("HTTP 429") {
        format!("the asset service is limiting you: {e}")
    } else if e.contains("certificate") {
        format!(
            "the asset service at {url} has another certificate than the one your token is for. If the server's \
             admin says it was replaced, enter your token again with +chimera:asset_token <token>; otherwise tell them."
        )
    } else {
        // The transport's own words, without the URL and layers before them.
        let why = e.rsplit("io: ").next().unwrap_or(e);
        format!("can't reach the asset service at {url} ({why}); is it running and its port open?")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn errors_read_as_what_to_do() {
        let e = super::explain("https://h:1", "/whoami: HTTP 401: the token was refused");
        assert!(e.contains("refused your token") && e.contains("+chimera:asset_token"), "{e}");
        assert!(super::explain("https://h:1", "connection refused").contains("can't reach the asset service"));
        assert!(super::explain("https://h:1", "invalid peer certificate").contains("another certificate"));
        let e = super::explain("https://h:1", "https://h:1/whoami: io: Connection refused (os error 111)");
        assert!(e.contains("(Connection refused (os error 111))"), "{e}");
    }
}
