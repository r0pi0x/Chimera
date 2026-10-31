//! `chimera_assets`: the local record store's command line and the asset
//! service's client (docs/plans/active/asset-service.md). `status` lists
//! each game build's manifest, its files, sets and sizes; `verify` reads
//! every record a manifest refers to and checks its hash (exit 1 on a
//! missing or corrupt one); `gc` deletes records no manifest refers to
//! (`--dry-run` lists them). With the service (`[assets] url` and `token`
//! in chimera.local.toml, or `CHIMERA_ASSET_URL` / `CHIMERA_ASSET_TOKEN`):
//! `login [<url> <token>]` checks a token and prints the `[assets]` lines
//! with the server's key to pin; `fetch <game> [<set>...]` fetches a
//! build's manifest (as the games choose it: `[assets] builds`,
//! `assets.lock`, else the newest; the service's copy when it changed and
//! the lock doesn't pin the kept one) and the records of those sets (all its
//! files without a set) in batches on a few connections, showing
//! progress (`progress::Transfer`: a live line on a terminal, a line every
//! 5 s otherwise, then a summary); `lock [<game>...]` pins the service's
//! newest builds in `assets.lock` (the one in effect, else `./assets.lock`).
//! The store is `--store <dir>`, else `CHIMERA_ASSET_STORE`, else
//! `[assets] store`, else the user's (`~/.cache/chimera/assets`,
//! `%LOCALAPPDATA%\chimera\assets`).

use std::{collections::BTreeSet, process::ExitCode};

use chimera_assets::{AssetSettings, AssetsLock, Manifest, Remote, Store, config, lock::Pin, progress::Transfer};

const USAGE: &str = "\
usage: chimera_assets [--store <dir>] <command>
  status                    the store's manifests, sets and sizes
  verify                    check every record's hash
  gc [--dry-run]            delete records no manifest refers to
  login [<url> <token>]     check a token with the asset service; print the [assets] lines
  fetch <game> [<set>...]   fetch a build's manifest and its sets' records (every file without a set)
  lock [<game>...]          pin the service's newest builds in assets.lock";

fn mb(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1e6)
}

fn manifests(store: &Store) -> Vec<Manifest> {
    let mut out = Vec::new();
    for game in store.games() {
        for build in store.builds(&game) {
            match Manifest::load(&store.manifest_path(&game, &build)) {
                Ok(m) => out.push(m),
                Err(e) => eprintln!("warning: {e}"),
            }
        }
    }
    out
}

fn status(store: &Store) -> ExitCode {
    println!("store: {}", store.root.display());
    let ids = store.record_ids();
    let on_disk: u64 = ids.iter().filter_map(|id| std::fs::metadata(store.record_path(id)).ok()).map(|m| m.len()).sum();
    println!("records: {} ({} on disk)", ids.len(), mb(on_disk));
    for m in manifests(store) {
        let records = m.records();
        let plain: u64 = records.values().map(|v| v.0).sum();
        let stored: u64 = records.values().map(|v| v.1).sum();
        let missing = records.keys().filter(|id| !store.has(id)).count();
        println!(
            "{} build {}{}: {} files, {} records, {} plain, {} stored ({:.0}%){}",
            m.game,
            m.build,
            m.app.map(|a| format!(" (app {a})")).unwrap_or_default(),
            m.files.len(),
            records.len(),
            mb(plain),
            mb(stored),
            100.0 * stored as f64 / plain.max(1) as f64,
            if missing > 0 { format!(", {missing} records missing") } else { String::new() },
        );
        for name in m.sets.keys() {
            let (n, plain, stored) = m.set_size(name);
            println!(
                "  set {name}: {} files, {n} records, {} plain, {} stored",
                m.sets[name].len(),
                mb(plain),
                mb(stored)
            );
        }
    }
    ExitCode::SUCCESS
}

fn verify(store: &Store) -> ExitCode {
    let mut checked = BTreeSet::new();
    let mut bad = 0;
    for m in manifests(store) {
        for (path, f) in &m.files {
            if !checked.insert(f.id.clone()) {
                continue;
            }
            if let Err(e) = store.get(&f.id, f.size) {
                bad += 1;
                println!("BAD {} {}: {path}: {e}", m.game, m.build);
            }
        }
    }
    println!("{} records checked, {bad} bad", checked.len());
    if bad == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

fn gc(store: &Store, dry_run: bool) -> ExitCode {
    let used: BTreeSet<String> = manifests(store).iter().flat_map(|m| m.files.values().map(|f| f.id.clone())).collect();
    let (mut n, mut bytes) = (0, 0);
    for id in store.record_ids().into_iter().filter(|id| !used.contains(id)) {
        let path = store.record_path(&id);
        bytes += std::fs::metadata(&path).map_or(0, |m| m.len());
        n += 1;
        if dry_run {
            println!("unused {id}");
        } else if let Err(e) = std::fs::remove_file(&path) {
            eprintln!("{}: {e}", path.display());
        }
    }
    println!("{} {n} unused records ({})", if dry_run { "would delete" } else { "deleted" }, mb(bytes));
    ExitCode::SUCCESS
}

fn service(settings: &AssetSettings) -> Result<std::sync::Arc<Remote>, String> {
    if settings.remote.is_none() {
        return Err("no asset service: set [assets] url and token in chimera.local.toml, or \
                    CHIMERA_ASSET_URL and CHIMERA_ASSET_TOKEN"
            .into());
    }
    settings.remote().ok_or_else(|| "the asset service's settings are unusable (see above)".into())
}

fn login(settings: &AssetSettings, args: &[&str]) -> Result<(), String> {
    let config = match args {
        [url, token] => chimera_assets::RemoteConfig { url: url.to_string(), token: token.to_string(), key: None },
        [] => settings.remote.clone().ok_or("no asset service configured: chimera_assets login <url> <token>")?,
        _ => return Err(USAGE.into()),
    };
    let remote = Remote::new(config.clone())?;
    let who = remote.whoami().map_err(|e| e.to_string())?;
    let key = remote.server_key()?;
    if config.key.as_ref().is_some_and(|k| k.trim() != key) {
        return Err(format!("the server's key isn't the one pinned ([assets] key); it says {key}"));
    }
    let mut lines = who.lines();
    println!("signed in to {} as {}", remote.url(), lines.next().unwrap_or("?"));
    println!("{}", lines.next().unwrap_or(""));
    println!("\nin chimera.local.toml (never committed):\n\n[assets]\nurl = \"{}\"", remote.url());
    if args.is_empty() {
        println!("token = \"...\"            # as configured");
    } else {
        println!("token = \"{}\"", config.token);
    }
    println!("key = \"{key}\"\n\nor CHIMERA_ASSET_URL / CHIMERA_ASSET_TOKEN / CHIMERA_ASSET_KEY.");
    Ok(())
}

fn fetch(settings: &AssetSettings, args: &[&str]) -> Result<(), String> {
    let [game, sets @ ..] = args else {
        return Err(USAGE.into());
    };
    let remote = service(settings)?;
    let store = settings.store.as_ref().ok_or("no store")?;
    let build = settings.build(game).ok_or_else(|| format!("{game}: no manifest in the store or on the service"))?;
    // The service's copy of that build, when it has changed since it was kept
    // (sets added, a republish) and assets.lock doesn't pin the kept one.
    let path = store.manifest_path(game, &build);
    let kept = std::fs::read(&path).map(|b| chimera_assets::sha256_hex(&b)).unwrap_or_default();
    let locked = settings.lock.as_ref().and_then(|l| l.games.get(*game)).is_some_and(|p| p.build == build);
    if !locked
        && let Some((_, _, id)) = remote.games().map_err(|e| e.to_string())?.into_iter().find(|g| g.0 == *game && g.1 == build)
        && id != kept
    {
        remote.fetch_manifest(store, game, &build, Some(&id))?;
        println!("{game} build {build}: the service's manifest is newer; updated");
    }
    let manifest = Manifest::load(&store.manifest_path(game, &build))?;
    let mut wanted: Vec<(String, u64, u64)> = Vec::new();
    if sets.is_empty() {
        wanted.extend(manifest.files.values().map(|f| (f.id.clone(), f.size, f.stored)));
    }
    for set in sets {
        let paths = manifest.sets.get(*set).ok_or_else(|| {
            format!(
                "{game} build {build} has no set {set} (it has: {})",
                manifest.sets.keys().cloned().collect::<Vec<_>>().join(", ")
            )
        })?;
        wanted.extend(paths.iter().map(|p| {
            let f = &manifest.files[p];
            (f.id.clone(), f.size, f.stored)
        }));
    }
    wanted.sort();
    wanted.dedup();
    let (have, need): (Vec<_>, Vec<_>) = wanted.iter().partition(|(id, _, _)| store.has(id));
    let need_bytes: u64 = need.iter().map(|w| w.2).sum();
    let what = if sets.is_empty() { "every file".to_string() } else { format!("set {}", sets.join(", ")) };
    println!(
        "{game} build {build}, {what}: {} records ({} plain), {} in the store, {} to fetch ({}) from {}",
        wanted.len(),
        mb(wanted.iter().map(|w| w.1).sum()),
        have.len(),
        need.len(),
        mb(need_bytes),
        remote.url()
    );
    if need.is_empty() {
        return Ok(());
    }
    let transfer = Transfer::new(game, need.len() as u64, need_bytes);
    let (_, _, failed) = remote.fill(store, &wanted, Some(&transfer));
    println!("{}", transfer.finish());
    for f in failed.iter().take(20) {
        println!("FAILED {f}");
    }
    if failed.is_empty() { Ok(()) } else { Err(format!("{} records failed", failed.len())) }
}

fn lock(settings: &AssetSettings, args: &[&str]) -> Result<(), String> {
    let remote = service(settings)?;
    let path = AssetsLock::find().unwrap_or_else(|| chimera_assets::lock::FILE_NAME.into());
    let mut lock = if path.is_file() { AssetsLock::load(&path)? } else { AssetsLock::default() };
    let mut latest = std::collections::BTreeMap::new();
    for (game, build, id) in remote.games().map_err(|e| e.to_string())? {
        // Oldest first: the last one per game is the newest.
        latest.insert(game, Pin { build, manifest: id });
    }
    let games: Vec<String> =
        if args.is_empty() { latest.keys().cloned().collect() } else { args.iter().map(|g| g.to_string()).collect() };
    for game in games {
        let pin = latest.get(&game).ok_or_else(|| format!("{game}: not on the service (or not for this account)"))?;
        println!("{game}: build {} manifest {}", pin.build, pin.manifest);
        lock.games.insert(game, pin.clone());
    }
    lock.save(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("wrote {}", path.display());
    Ok(())
}

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut store = None;
    if let Some(i) = args.iter().position(|a| a == "--store") {
        if i + 1 >= args.len() {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
        store = Some(Store::new(args.remove(i + 1)));
        args.remove(i);
    }
    let mut settings = match config::load_local() {
        Ok(c) => c.resolve(),
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    if store.is_some() {
        settings.store = store;
    }
    let Some(store) = settings.store.clone() else {
        eprintln!("no store: pass --store <dir> or set CHIMERA_ASSET_STORE");
        return ExitCode::from(2);
    };
    let done = |r: Result<(), String>| match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    };
    match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["status"] => status(&store),
        ["verify"] => verify(&store),
        ["gc"] => gc(&store, false),
        ["gc", "--dry-run"] => gc(&store, true),
        ["login", ref rest @ ..] => done(login(&settings, rest)),
        ["fetch", ref rest @ ..] => done(fetch(&settings, rest)),
        ["lock", ref rest @ ..] => done(lock(&settings, rest)),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
