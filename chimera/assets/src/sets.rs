//! Fetching whole sets from the asset service, as the game does it
//! (docs/plans/active/asset-service.md, "Playtest delivery"): the files a
//! join's offer names (`chimera_net::net::assets`), the later maps it
//! prefetches (at a `Pace`), and what the games read at startup
//! (`STARTUP_SETS`, fetched behind the client's progress window). `fetch`
//! reads each game's manifest (fetched when the store lacks it, and again
//! when the service lists another one for the same build: `refresh`), the
//! sets' records the store lacks, and fills them with a `progress::Transfer` the
//! caller can watch (what didn't arrive is asked again after
//! `RETRY_DELAYS`, unless the token was refused); `missing` says, from the store alone (no network),
//! whether a fetch would have anything to do, `held` and `map_held`
//! whether the store already holds whole sets (a map's); `explain` turns a service's
//! error into what a player can act on.

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{AssetSettings, Manifest, Remote, pace::Pace, progress::Transfer, sha256_hex};

/// The sets a game reads at startup: what its client fetches before the
/// game's window when the store lacks them.
pub const STARTUP_SETS: &[&str] = &["base"];

/// Waits before each new try at the records a fetch didn't get (a
/// connection closed or refused, a 429): the service restarted or busy
/// for a few seconds shouldn't leave a player without files.
pub const RETRY_DELAYS: &[Duration] = &[Duration::from_secs(2), Duration::from_secs(5), Duration::from_secs(10)];

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
    /// Waits before each new try at what didn't arrive (None:
    /// `RETRY_DELAYS`).
    pub retry_delays: Option<&'a [Duration]>,
    /// A foreground fill begun by the caller (`pending::starting`, taken
    /// before this runs): marked started once the fill has claimed what it
    /// fetches, so on-demand misses meanwhile wait for that claim.
    pub starting: Option<&'a crate::pending::Starting>,
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
    let store = settings.store.clone().ok_or("no asset store folder")?;
    // The service's builds and manifest ids: to skip games it doesn't
    // list, and to see whether a kept manifest is older than its own.
    let kept = wanted.iter().any(|g| store.latest_build(&g.game).is_some());
    let listing = (options.skip_unlisted || kept).then(|| remote.games());
    let listed: Option<HashSet<String>> = match &listing {
        Some(l) if options.skip_unlisted => {
            Some(l.as_ref().map_err(|e| explain(&url, &e.to_string()))?.iter().map(|g| g.0.clone()).collect())
        }
        _ => None,
    };
    if let Some(Ok(listing)) = &listing {
        let games: Vec<&str> = wanted.iter().map(|g| g.game.as_str()).collect();
        refresh(settings, &remote, listing, &games);
    }
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
    // The fill claims its records as it begins (`pending::claim`); misses
    // waiting on the start go on now and find them pending.
    let (_, _, mut failed) = {
        let starting = options.starting;
        remote.fill_paced_then(&store, &records, Some(&transfer), options.pace, &|| {
            if let Some(s) = starting {
                s.started();
            }
        })
    };
    let mut summary = transfer.finish();
    // What didn't arrive is asked again after a wait, unless the token was
    // refused (401, 403: no wait helps); each try's progress is its own.
    for wait in options.retry_delays.unwrap_or(RETRY_DELAYS) {
        let refused = failed.iter().any(|f| ["HTTP 401", "HTTP 403"].iter().any(|s| f.contains(s)));
        if failed.is_empty() || refused || options.pace.is_some_and(Pace::stopped) {
            break;
        }
        eprintln!(
            "{} of the {what} didn't arrive ({}): asking again in {} s",
            failed.len(),
            failed[0],
            wait.as_secs_f32()
        );
        std::thread::sleep(*wait);
        let left: Vec<(String, u64, u64)> = records.iter().filter(|r| !store.has(&r.0)).cloned().collect();
        let again = Arc::new(Transfer::with_live(what, left.len() as u64, left.iter().map(|w| w.2).sum(), false));
        *shared.lock().unwrap_or_else(|e| e.into_inner()) = Some(again.clone());
        failed = remote.fill_paced(&store, &left, Some(&again), options.pace).2;
        summary.push_str(&format!("; again: {}", again.finish()));
    }
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
    if !absent.is_empty() {
        summary.push_str(&format!(" (not on the service, so fetched as needed: {})", absent.join(", ")));
    }
    Ok(summary)
}

/// The kept manifests of `games` (at the pinned build, else the store's
/// newest) fetched again where the service's `listing` (`Remote::games`)
/// names another manifest for that build: the host published again
/// (new or bigger sets, new files) without a new game build. Without
/// this, a player who fetched before keeps the old sets for good: a set's
/// added files are then never fetched with it, only one by one as the
/// loaders read them. A build `assets.lock` pins keeps the locked
/// manifest (`AssetSettings::build`). The games whose manifest was
/// replaced; one that couldn't be is said and keeps the old one. Mounts
/// already open keep reading the manifest they opened (a new process
/// reads the new one).
pub fn refresh(settings: &AssetSettings, remote: &Remote, listing: &[(String, String, String)], games: &[&str]) -> Vec<String> {
    let Some(store) = settings.store.as_ref() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for game in games {
        if out.iter().any(|g| g == game) || settings.lock.as_ref().is_some_and(|l| l.games.contains_key(*game)) {
            continue;
        }
        let Some(build) = settings.builds.get(*game).cloned().or_else(|| store.latest_build(game)) else {
            continue;
        };
        let Ok(kept) = std::fs::read(store.manifest_path(game, &build)) else {
            continue;
        };
        let Some((_, _, id)) = listing.iter().find(|(g, b, _)| g == game && *b == build) else {
            continue;
        };
        if sha256_hex(&kept) == *id {
            continue;
        }
        match remote.fetch_manifest(store, game, &build, None) {
            Ok(_) => {
                eprintln!("assets: {game} build {build}: the service has a newer manifest (sets published again): fetched it");
                out.push(game.to_string());
            }
            Err(e) => eprintln!("assets: {game} build {build}: the service's newer manifest didn't arrive ({e}); the kept one is used"),
        }
    }
    out
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

/// Whether the store alone holds every record of `game`'s `sets` (at the
/// pinned build, else its newest), with no network: each set named by
/// its manifest and none of their records missing. A set the manifest
/// doesn't name isn't held. What may be read without downloading (the
/// main menu's background map).
pub fn held(settings: &AssetSettings, game: &str, sets: &[&str]) -> bool {
    kept_manifest(settings, game).is_some_and(|(store, manifest)| sets.iter().all(|s| whole(&store, &manifest, s)))
}

/// Whether the store alone holds map `name` of `game` whole, as `held`:
/// its set (`map/<name>`, else an older manifest's `<name>`), and the
/// game's `base` set when the manifest names one.
pub fn map_held(settings: &AssetSettings, game: &str, name: &str) -> bool {
    let Some((store, manifest)) = kept_manifest(settings, game) else {
        return false;
    };
    let map = [format!("map/{name}"), name.to_string()]
        .into_iter()
        .find(|s| manifest.sets.contains_key(s));
    map.is_some_and(|m| whole(&store, &manifest, &m))
        && (!manifest.sets.contains_key("base") || whole(&store, &manifest, "base"))
}

/// Set `set` named by `manifest` with every record in `store`.
fn whole(store: &crate::Store, manifest: &Manifest, set: &str) -> bool {
    manifest
        .sets
        .get(set)
        .is_some_and(|paths| paths.iter().all(|p| manifest.files.get(p).is_some_and(|r| store.has(&r.id))))
}

/// `game`'s manifest the store keeps (at the pinned build, else its
/// newest), with no network.
fn kept_manifest(settings: &AssetSettings, game: &str) -> Option<(crate::Store, Manifest)> {
    let store = settings.store.as_ref()?;
    let pinned = settings
        .builds
        .get(game)
        .cloned()
        .or_else(|| settings.lock.as_ref().and_then(|l| l.games.get(game)).map(|p| p.build.clone()));
    let build = pinned.or_else(|| store.latest_build(game))?;
    let manifest = Manifest::load(&store.manifest_path(game, &build)).ok()?;
    Some((store.clone(), manifest))
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
