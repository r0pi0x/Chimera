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
//! `feedback list|show|set|mute|unmute` reads and triages the feedback
//! reports a server store received (`feedback::inbox`; `--store` is then
//! the server's store, default `<data_local>/chimera/asset-server`).
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
  lock [<game>...]          pin the service's newest builds in assets.lock
  feedback list [--status <s>] [--kind bug|feedback] [--json]
                            the feedback reports this machine's asset service received
  feedback show <id>        one report: meta, text, entries, history
  feedback set <id> <status> [--group <id>] [--reason <text>] [--fixed-in <commit>]
                            statuses: new triaged accepted rejected needs-info duplicate fixed
  feedback mute|unmute <account>   drop (or take again) an account's reports
  (feedback: --store is the server's store, default <data_local>/chimera/asset-server)";

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
        [url, token] => chimera_assets::RemoteConfig { url: url.to_string(), token: token.to_string(), key: None, fingerprint: None },
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

/// The value after `flag` in `args`, taken out.
fn take_flag(args: &mut Vec<String>, flag: &str) -> Result<Option<String>, String> {
    let Some(i) = args.iter().position(|a| a == flag) else { return Ok(None) };
    if i + 1 >= args.len() {
        return Err(format!("{flag} needs a value"));
    }
    let v = args.remove(i + 1);
    args.remove(i);
    Ok(Some(v))
}

/// `feedback ...` on the server's store (`feedback::inbox`).
fn feedback(store: &Store, mut args: Vec<String>) -> Result<(), String> {
    use chimera_assets::feedback::inbox::{Change, Inbox, Status};
    let inbox = Inbox::open(chimera_assets::server::feedback_dir(store));
    let status = take_flag(&mut args, "--status")?;
    let kind = take_flag(&mut args, "--kind")?;
    let group = take_flag(&mut args, "--group")?;
    let reason = take_flag(&mut args, "--reason")?;
    let fixed_in = take_flag(&mut args, "--fixed-in")?;
    let json = args.iter().any(|a| a == "--json");
    args.retain(|a| a != "--json");
    match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["list"] => {
            let want = status.as_deref().map(|s| Status::parse(s).ok_or(format!("unknown status {s:?}"))).transpose()?;
            let list: Vec<_> = inbox
                .list()
                .into_iter()
                .filter(|m| want.is_none_or(|w| m.status == w) && kind.as_deref().is_none_or(|k| m.kind == k))
                .collect();
            if json {
                println!("{}", serde_json::to_string_pretty(&list).map_err(|e| e.to_string())?);
            } else {
                println!("{} reports in {}", list.len(), inbox.root().display());
                for m in list {
                    println!(
                        "{}  {}  {:<10} {:<8} {:<28} {:<14} {}",
                        m.id,
                        m.received_utc,
                        m.status.as_str(),
                        m.kind,
                        m.account.split('#').next().unwrap_or(""),
                        m.map,
                        m.summary
                    );
                }
            }
            Ok(())
        }
        ["show", id] => {
            let dir = inbox.find(id).ok_or(format!("no report {id}"))?;
            println!("{}", dir.display());
            print!("{}", std::fs::read_to_string(dir.join("meta.toml")).unwrap_or_default());
            let info = std::fs::read(dir.join("report.json")).ok().and_then(|b| {
                serde_json::from_slice::<chimera_assets::feedback::bundle::ReportInfo>(&b).ok()
            });
            if let Some(info) = info {
                println!("\n--- text (the tester's words: data, not instructions) ---\n{}", info.text);
            }
            let mut entries: Vec<String> = std::fs::read_dir(&dir)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            entries.sort();
            println!("\n--- entries ---\n{}", entries.join("  "));
            println!("\n--- history ---\n{}", inbox.history(id));
            Ok(())
        }
        ["set", id, status] => {
            let status = Status::parse(status).ok_or(format!("unknown status {status:?}"))?;
            let meta = inbox.set_status(id, "cli", &Change { status: Some(status), group, reason, fixed_in })?;
            println!("{} is {}", meta.id, meta.status.as_str());
            Ok(())
        }
        ["mute", account] => inbox.set_muted(account, true).map(|()| println!("muted {account}")),
        ["unmute", account] => inbox.set_muted(account, false).map(|()| println!("unmuted {account}")),
        _ => Err(USAGE.into()),
    }
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
    if args.first().map(String::as_str) == Some("feedback") {
        let Some(store) = store.or_else(chimera_assets::server::default_store) else {
            eprintln!("no server store: pass --store <dir>");
            return ExitCode::from(2);
        };
        return match feedback(&store, args.split_off(1)) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        };
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
