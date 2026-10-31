//! `chimera_pack`: a game install's files into the asset store
//! (docs/plans/active/asset-service.md, M1). A host tool: `--features tools`,
//! never in releases.
//!
//! `chimera_pack pack <game> [--set <name> --from <vfs log>]` reads files
//! through the game's own mount of its install (its search order; Combat
//! Arms' archives decrypted here with this host's keys, which are never
//! written anywhere), stores each as a record (SHA-256 id, zstd level 3)
//! and enters it in the manifest of the install's build
//! (`manifests/<game>/<build>.txt`). With `--from`, only the files of that
//! game in a `CHIMERA_VFS_LOG` log, entered as set `<name>` too; without,
//! every file the mount shows (CS:S's includes the game's download
//! folder and Chimera's content cache: the publisher's community maps go
//! with it), and the manifest is marked `complete` (a whole install:
//! served alone, it stands in for one), with the game's
//! loose files outside its archives added (Combat Arms' music,
//! `Game/BGM/`, as set `music`). Records already stored are skipped, so
//! packing again or a new build stores only what changed. The store is
//! `--store`, else `[assets] store`/`CHIMERA_ASSET_STORE`, else the user's
//! (`~/.cache/chimera/assets`).
//!
//! The asset service (M2): `chimera_pack publish <game> [the same
//! options]` packs into the server's store (`--store`, else this machine's
//! `<data_local>/chimera/asset-server`, which `chimera_asset_server`
//! serves), copies the sets recorded for that build in the user's store
//! (`--sets-from <store>`; only sets whose files were all published) and
//! signs the manifest with the server's key, so on the same box a publish
//! goes straight into what the server serves (no restart). `chimera_pack
//! user add <name> [--games <a,b>] [--note <text>]` makes an internal
//! account (or a new token for one: one per device) and prints the token
//! once; `user revoke <name> [--token <n>]` takes one or all of its tokens
//! back (at the server's next request); `user list` shows the accounts.
//!
//! `chimera_pack sets <game> [--store <dir>] [--sign]` records the sets a
//! game's pieces read, by loading each through the game's own loaders on
//! the install (a tap on the mount notes every path read) and enters them
//! in the manifest of the install's build (which must list those files: a
//! whole pack or publish): `weapon/<id>` per weapon (CS:S: its view and
//! world models with their materials and the waves of its sound entries;
//! Combat Arms: its view model with hands and parts' textures), `players`
//! (CS:S: the eight class models). `--sign` signs the manifest again with
//! the server's key (a server store). `base` and the maps' sets come from
//! `CHIMERA_VFS_LOG` logs (`--from`).
//!
//! Known shortcuts: publishing to a server on another machine (upload over
//! HTTPS with a publisher token) isn't built: on a VPS, publish there or
//! copy the store.

use std::{path::PathBuf, process::ExitCode};

use chimera_assets::{
    Manifest, Store,
    accounts::{Accounts, is_account_name},
    pack, read_log, server, sign,
};
use chimera_core::{
    game_registry::GameInfo,
    mount::{Mount, config::LocalConfig, install},
};

const USAGE: &str = "\
usage: chimera_pack pack <game> [options]       into a store (default the user's)
       chimera_pack publish <game> [options]    into the asset server's store, signed
       chimera_pack sets <game> [--store <dir>] [--sign]   record weapon/<id> and players sets
       chimera_pack user add <name> [--games <a,b>] [--note <text>]
       chimera_pack user revoke <name> [--token <n>]
       chimera_pack user list                   (user: --store <server store>)
  games: cs_source, combat_arms
  --set <name>      enter the packed files as set <name> (needs --from)
  --from <log>      only the game's files in a CHIMERA_VFS_LOG log
  --store <dir>     the store (default: [assets] store, CHIMERA_ASSET_STORE, ~/.cache/chimera/assets)
  --install <dir>   the install (default: as the game finds it)
  --build <id>      the build id (default: the install's Steam app manifest)
  --threads <n>     reading and packing threads (default 4)
  --sets-from <dir> publish: the store whose sets of this build to copy (default the user's)";

/// A game the packer knows: its info, its install's own mount (without the
/// store or the log) and its Steam app.
struct Game {
    info: &'static GameInfo,
    open_install: fn(&std::path::Path) -> std::io::Result<Mount>,
    app: u32,
    /// Files the game reads from its install outside the mount, as a layer
    /// and the set they make (whole packs only).
    extra: Option<(fn(&std::path::Path) -> Option<Box<dyn chimera_core::mount::FileSource>>, &'static str)>,
}

fn games() -> [Game; 2] {
    [
        Game { info: &cs_source::game::INFO, open_install: cs_source::game::mount::open_install, app: 240, extra: None },
        Game {
            info: &combat_arms::game::INFO,
            open_install: combat_arms::game::mount::open_install,
            // Combat Arms: the Classic.
            app: 1_263_550,
            extra: Some((combat_arms::game::mount::loose_music, "music")),
        },
    ]
}

fn mb(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1e6)
}

fn run(args: &[String]) -> Result<(), String> {
    let mut it = args.iter();
    let publish = match it.next().map(String::as_str) {
        Some("pack") => false,
        Some("publish") => true,
        Some("user") => return user(&args[1..]),
        Some("sets") => return sets(&args[1..]),
        _ => return Err(USAGE.into()),
    };
    let game_id = it.next().ok_or(USAGE)?;
    let (mut set, mut from, mut store, mut install, mut build, mut threads) = (None, None, None, None, None, 4);
    let mut sets_from = None;
    while let Some(arg) = it.next() {
        let mut value = || it.next().cloned().ok_or_else(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--set" => set = Some(value()?),
            "--from" => from = Some(PathBuf::from(value()?)),
            "--store" => store = Some(Store::new(value()?)),
            "--install" => install = Some(PathBuf::from(value()?)),
            "--build" => build = Some(value()?),
            "--threads" => threads = value()?.parse().map_err(|_| "--threads: a number")?,
            "--sets-from" if publish => sets_from = Some(Store::new(value()?)),
            _ => return Err(format!("unknown option {arg}\n{USAGE}")),
        }
    }
    if set.is_some() != from.is_some() {
        return Err("--set and --from go together".into());
    }
    let games = games();
    let game = games.iter().find(|g| g.info.id == game_id).ok_or_else(|| format!("unknown game {game_id}\n{USAGE}"))?;
    let config = LocalConfig::load()?;
    let user_store = config.assets.resolve().store;
    let store = if publish { store.or_else(server::default_store) } else { store.or_else(|| user_store.clone()) }
        .ok_or("no store folder: pass --store")?;
    // The real install, whatever `[assets] use_install` says.
    let install = match install {
        Some(p) => p,
        None => install::resolve(&config, game.info.id, game.info.steam.as_ref())
            .install
            .map(|i| i.path)
            .ok_or_else(|| format!("no {} install found: pass --install", game.info.id))?,
    };
    let build = match build {
        Some(b) => b,
        None => install::install_build_id(&install, game.app).ok_or_else(|| {
            format!("{}: no Steam app manifest for app {}: pass --build", install.display(), game.app)
        })?,
    };
    let mut mount = (game.open_install)(&install).map_err(|e| format!("{}: {e}", install.display()))?;
    let whole = from.is_none();
    let mut extra_set = None;
    if let Some((layer, set)) = game.extra.as_ref().and_then(|(f, set)| Some((f(&install)?, *set))) {
        if whole {
            extra_set = Some((set, layer.entries().into_iter().map(|e| e.path).collect::<Vec<_>>()));
        }
        mount.push_boxed(layer);
    }
    let paths: Vec<String> = match &from {
        Some(log) => {
            let lines = read_log(log)?;
            let mut paths: Vec<String> = lines.into_iter().filter(|l| l.game == game.info.id).map(|l| l.path).collect();
            paths.sort();
            paths.dedup();
            if paths.is_empty() {
                return Err(format!("{}: no {} files in the log", log.display(), game.info.id));
            }
            paths
        }
        None => mount.entries().into_iter().map(|(e, _)| e.path).collect(),
    };
    let manifest_path = store.manifest_path(game.info.id, &build);
    let mut manifest = if manifest_path.is_file() {
        Manifest::load(&manifest_path)?
    } else {
        Manifest::new(game.info.id, &build, Some(game.app))
    };
    println!(
        "packing {} files of {} build {} from {} into {}",
        paths.len(),
        game.info.id,
        build,
        install.display(),
        store.root.display()
    );
    let report = pack(&mount, &paths, &store, &mut manifest, set.as_deref(), threads).map_err(|e| e.to_string())?;
    if whole && report.missing.is_empty() {
        manifest.complete = true;
    }
    if let Some((name, paths)) = extra_set {
        println!("set {name}: {} files (outside the archives)", paths.len());
        manifest.sets.insert(name.to_string(), paths.into_iter().collect());
    }
    if publish {
        let from = sets_from.or(user_store).filter(|s| *s != store);
        if let Some(from) = from.filter(|s| s.manifest_path(game.info.id, &build).is_file()) {
            let theirs = Manifest::load(&from.manifest_path(game.info.id, &build))?;
            for (name, paths) in theirs.sets {
                let same = |p: &String| {
                    manifest.files.get(p).is_some_and(|f| theirs.files.get(p).is_some_and(|g| g.id == f.id))
                };
                if paths.iter().all(same) {
                    println!("set {name}: {} files (from {})", paths.len(), from.root.display());
                    manifest.sets.insert(name, paths);
                } else {
                    println!("set {name} of {} left out: not all its files were published", from.root.display());
                }
            }
        }
    }
    manifest.save(&manifest_path).map_err(|e| format!("{}: {e}", manifest_path.display()))?;
    if publish {
        let key = sign::load_or_create_key(&server::key_path(&store)).map_err(|e| e.to_string())?;
        sign::sign_manifest_file(&key, &manifest_path).map_err(|e| format!("{}: {e}", manifest_path.display()))?;
        println!("signed with the server's key {}", sign::public_hex(&key));
    }
    println!(
        "{} files, {} read, {} records ({} new), {} stored ({:.0}% of what was read), {:.2} s",
        report.files,
        mb(report.bytes_read),
        report.records,
        report.new_records,
        mb(report.stored),
        100.0 * report.stored as f64 / report.bytes_read.max(1) as f64,
        report.took.as_secs_f64()
    );
    if !report.missing.is_empty() {
        println!("{} paths not in the install (left out):", report.missing.len());
        for p in &report.missing {
            println!("  {p}");
        }
    }
    println!("manifest: {}", manifest_path.display());
    Ok(())
}

/// `user add | revoke | list` on the server store's accounts file.
fn user(args: &[String]) -> Result<(), String> {
    let mut it = args.iter();
    let command = it.next().ok_or(USAGE)?.clone();
    let mut name = None;
    let (mut store, mut games, mut note, mut token) = (None, None, String::new(), None);
    while let Some(arg) = it.next() {
        let mut value = || it.next().cloned().ok_or_else(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--store" => store = Some(Store::new(value()?)),
            "--games" => {
                games = Some(value()?.split(',').map(|g| g.trim().to_string()).filter(|g| !g.is_empty()).collect())
            }
            "--note" => note = value()?,
            "--token" => token = Some(value()?.parse::<u32>().map_err(|_| "--token: a number")?),
            a if !a.starts_with("--") && name.is_none() => name = Some(a.to_string()),
            _ => return Err(format!("unknown option {arg}\n{USAGE}")),
        }
    }
    let store = store.or_else(server::default_store).ok_or("no store folder: pass --store")?;
    let path = Accounts::path(&store.root);
    let mut accounts = Accounts::load(&path)?;
    match (command.as_str(), name) {
        ("add", Some(name)) if is_account_name(&name) => {
            let (id, token) = accounts.add(&name, games, &note)?;
            accounts.save(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            println!("account {name}, token {id} (shown once; revoke: chimera_pack user revoke {name} --token {id}):");
            println!("{token}");
            println!(
                "\nfor the tester's chimera.local.toml:\n[assets]\nurl = \"https://<the server's name>\"\ntoken = \"{token}\""
            );
        }
        ("add", name) => return Err(format!("{name:?}: an account name is letters, digits, `_`, `-` or `.`")),
        ("revoke", Some(name)) => {
            let n = accounts.revoke(&name, token)?;
            accounts.save(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            println!("{name}: {n} token(s) revoked");
        }
        ("list", None) => {
            for a in &accounts.accounts {
                let games = if a.games.is_empty() { "*".to_string() } else { a.games.join(",") };
                println!("{} (games {games})", a.name);
                for t in &a.tokens {
                    let state = if t.revoked { "revoked" } else { "active" };
                    println!("  token {} {state}, created {} {}", t.id, t.created, t.note);
                }
            }
        }
        _ => return Err(USAGE.into()),
    }
    Ok(())
}

/// A mount layer that notes every path read through it.
struct Tap {
    inner: std::sync::Arc<Mount>,
    seen: std::sync::Arc<std::sync::Mutex<std::collections::BTreeSet<String>>>,
}

impl chimera_core::mount::FileSource for Tap {
    fn name(&self) -> String {
        "tap".into()
    }

    fn entries(&self) -> Vec<chimera_core::mount::Entry> {
        self.inner.entries().into_iter().map(|(e, _)| e).collect()
    }

    fn read(&self, path: &str) -> Option<std::io::Result<Vec<u8>>> {
        let got = self.inner.read(path);
        if got.is_ok() {
            self.seen.lock().unwrap_or_else(|e| e.into_inner()).insert(path.to_string());
        }
        Some(got)
    }

    fn contains(&self, path: &str) -> bool {
        self.inner.contains(path)
    }

    fn files_in(&self, dir: &str) -> Vec<String> {
        self.inner.files_in(dir)
    }
}

/// Each named piece of `game` (a weapon, the players) and the paths
/// loading it read through `mount`.
fn record_pieces(game: &str, mount: &std::sync::Arc<Mount>) -> Vec<(String, Vec<String>)> {
    let seen = std::sync::Arc::new(std::sync::Mutex::new(std::collections::BTreeSet::new()));
    let mut tapped = Mount::default();
    tapped.push(Tap { inner: mount.clone(), seen: seen.clone() });
    let take = || std::mem::take(&mut *seen.lock().unwrap_or_else(|e| e.into_inner())).into_iter().collect::<Vec<_>>();
    let mut out = Vec::new();
    match game {
        "cs_source" => {
            use cs_source::game::{bsp::CLASS_MODELS, material::MaterialLoader, props, sound::SoundScripts, weapons};
            let scripts = SoundScripts::load(&mut MaterialLoader::guest(mount, 0, 0), "");
            for w in &weapons::cs_weapons().weapons {
                take();
                let mut materials = MaterialLoader::guest(&tapped, 0, 0);
                if let Some(view) = &w.models.view {
                    let _ = props::load_view_model(&mut materials, view, &w.id, w.models.view_right_handed);
                }
                for world in [&w.models.world, &w.models.world_alt].into_iter().flatten() {
                    let _ = props::load_held(&mut materials, world, &w.id, &[]);
                }
                for name in w.sounds.iter().flat_map(|s| s.entries()) {
                    for wave in scripts.waves(name) {
                        let _ = tapped.read(&wave);
                    }
                }
                let id = w.id.rsplit(':').next().unwrap_or(&w.id).trim_start_matches("weapon_");
                out.push((format!("weapon/{id}"), take()));
            }
            let mut materials = MaterialLoader::guest(&tapped, 0, 0);
            for model in CLASS_MODELS {
                let _ = props::load_held(&mut materials, model, model, &[]);
            }
            out.push(("players".into(), take()));
        }
        "combat_arms" => {
            use combat_arms::game::{data::CaData, view_model::on_demand};
            let read = |p: &str| tapped.read(p).ok();
            for w in &CaData::install().weapon_set().weapons {
                if !on_demand::ours(&w.id) {
                    continue;
                }
                take();
                let _ = on_demand::load_weapon(&w.id, &read);
                let id = w.id.rsplit(':').next().unwrap_or(&w.id);
                out.push((format!("weapon/{id}"), take()));
            }
        }
        _ => {}
    }
    out.retain(|(_, paths)| !paths.is_empty());
    out
}

/// `sets <game>`: record the pieces' sets into the install build's
/// manifest.
fn sets(args: &[String]) -> Result<(), String> {
    let mut it = args.iter();
    let game_id = it.next().ok_or(USAGE)?;
    let (mut store, mut sign_it) = (None, false);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--store" => store = Some(Store::new(it.next().ok_or("--store needs a value")?)),
            "--sign" => sign_it = true,
            _ => return Err(format!("unknown option {arg}\n{USAGE}")),
        }
    }
    let games = games();
    let game = games.iter().find(|g| g.info.id == game_id).ok_or_else(|| format!("unknown game {game_id}\n{USAGE}"))?;
    let config = LocalConfig::load()?;
    let store = store.or_else(|| config.assets.resolve().store).ok_or("no store folder: pass --store")?;
    let install = install::resolve(&config, game.info.id, game.info.steam.as_ref())
        .install
        .map(|i| i.path)
        .ok_or_else(|| format!("no {} install found", game.info.id))?;
    let build = install::install_build_id(&install, game.app)
        .ok_or_else(|| format!("{}: no Steam app manifest for app {}", install.display(), game.app))?;
    let path = store.manifest_path(game.info.id, &build);
    let mut manifest = Manifest::load(&path)?;
    let mount = std::sync::Arc::new((game.open_install)(&install).map_err(|e| format!("{}: {e}", install.display()))?);
    for (name, paths) in record_pieces(game.info.id, &mount) {
        let (kept, left): (Vec<String>, Vec<String>) = paths.into_iter().partition(|p| manifest.files.contains_key(p));
        if !left.is_empty() {
            println!("set {name}: {} files not in the manifest left out (pack the whole install first)", left.len());
        }
        manifest.sets.insert(name.clone(), kept.into_iter().collect());
        let (n, plain, stored) = manifest.set_size(&name);
        println!("set {name}: {n} records, {} plain, {} stored", mb(plain), mb(stored));
    }
    manifest.save(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    if sign_it {
        let key = sign::load_or_create_key(&server::key_path(&store)).map_err(|e| e.to_string())?;
        sign::sign_manifest_file(&key, &path).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    println!("manifest: {}", path.display());
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
