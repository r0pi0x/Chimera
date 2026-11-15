//! `chimera_pack`: a game install's files into the asset store
//! (docs/plans/active/asset-service.md, M1). A host tool: `--features tools`,
//! never in releases.
//!
//! `chimera_pack pack <game> [--set <name> --from <vfs log>]` reads files
//! through the game's own mount of its install (its search order; Combat
//! Arms' archives decrypted here with this host's keys, which are never
//! written anywhere), stores each as a record (SHA-256 id, zstd level 19)
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
//! Combat Arms: its view model with hands and parts' textures, its
//! held model with its textures, its own sounds,
//! `CaWeaponFx::sound_paths`, and what it throws or fires,
//! `special::object_files`), `players`
//! (CS:S: the eight class models; HL2:DM: its 19 player models; TF2: its
//! nine class models), TF2's `weapon/<id>` for M2's guns
//! (`tf2::game::sets`: the `c_` model, the class's arms with their
//! animations, sounds, the rocket, and the script, packed decrypted:
//! `publish tf2` and `pack tf2` read the install's `.ctx` files through
//! `tf2::game::scripts::decrypting` with this host's key, and leave them
//! out, warning loudly, without one), and
//! Combat Arms' `character/<record>` per body record with a file (its
//! model, animation library and textures, as a rung's body loads them:
//! `characters::models::load_body`, and the hand model and skins of the
//! rung looks of that body whose first-person hands aren't the default
//! character's, `rungs::body_hands`; the asset offer names a ladder's
//! drawn bodies' sets, `rungs::body_set`) and `parts/<id>` per gun with
//! parts (its view and held models with each part a rung may draw for
//! it and with its default parts, less the bare gun's files: the offer
//! names a ladder's rung guns' sets, `rungs::parts_set`), and
//! `map/<name>` for every map of the install (`GameInfo::map_names`: the
//! files loading it reads as another game's gameplay loads it, the
//! world with its players' kit, never from the converted-map cache; less
//! the game's `base`, `players` and `common`; the offer names the loaded map's set,
//! in the pool or not). `map/<name>` and Combat Arms' `base` (its effect
//! tables' sounds: impacts, shells, surfaces, the event sounds and the
//! heartbeat, the infection and grenade call-outs, the result screen's
//! win and lose sounds; the default
//! character's first-person hand model and skins; the debris table with
//! its pieces' models, skins and sounds; its cue pack's sounds; the air
//! strike surprise's alarm and its shells' model, sounds and effect
//! groups' files, `special::devices::strike_files`)
//! and CS:S's `base` (its minigame cue pack's sounds, the party match-end
//! screen's winner sound, what its guns draw on other games' maps:
//! `bsp::add_guest_looks`, with its shells' bounce sounds) and HL2:DM's
//! `base` (what its guns draw on any map: `maps::add_guest_looks`, its
//! effect materials, muzzle flashes, impact decals and spent shells, with
//! their bounce sounds) are merged into what the manifest has (sets recorded from
//! logs too; once `lean` has made a `common`, what a game's `base` piece
//! adds goes to `common` instead, fetched with each match); the other
//! sets are replaced. `--only <prefix>` records only
//! the sets whose names start with it (`--only character/`), leaving the
//! others as they are. `--sign` signs the manifest again with
//! the server's key (a server store). `base` (beyond Combat Arms'
//! sounds) comes from `CHIMERA_VFS_LOG` logs (`--from`).
//!
//! `chimera_pack sets` also enters a game's declared files (`declared_files`:
//! Combat Arms' call-out, radio and announcer voices, read by its own loaders
//! from the install) as the `declared` set and into `common`, whether or not
//! a run read them; `lean` keeps them there.
//!
//! `chimera_pack lean <game> --store <dir> --from <log>...` makes the
//! game's `base` lean from client runs' `CHIMERA_VFS_LOG` logs (one per
//! run; `chimera_assets::resolve::lean`): `base` keeps what every run
//! read, the music leaves it for a set per track (`music/<track>`), the
//! files a ladder's sets hold that not every run read leave it (the files
//! only checked for among them), the rest becomes `common` (each match's
//! offer names it). No install needed; `--dry-run` says what it would do.
//! `chimera_pack repack --store <dir>` packs the store's records again
//! at the current zstd level (19; level 3 before 2026-11-15), each
//! written beside its old one and renamed in (a hard-linked copy of a
//! store keeps the other store's files), and sets the manifests' stored
//! sizes (`--only <game>:<set prefix>` for some sets' records, `--sign`;
//! `--restat`: only the manifests' stored sizes, after a manifest from
//! before the repack was put back or published again).
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
       chimera_pack sets <game> [--store <dir>] [--sign] [--only <prefix>]
                                                record weapon/<id>, players, map/<name>,
                                                character/<record> and parts/<id> sets
       chimera_pack lean <game> --store <dir> --from <log> [--from <log>...] [--sign] [--dry-run]
                                                base from client runs' reads: common, music/<track>
       chimera_pack repack --store <dir> [--threads <n>] [--only <game>:<set prefix>,...] [--sign] [--restat]
                                                records again at the current zstd level
                                                (--restat: only the manifests' stored sizes)
       chimera_pack user add <name> [--games <a,b>] [--note <text>]
       chimera_pack user revoke <name> [--token <n>]
       chimera_pack user list                   (user: --store <server store>)
  games: cs_source, combat_arms, hl2dm, tf2
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

fn games() -> [Game; 4] {
    [
        Game { info: &cs_source::game::INFO, open_install: cs_source::game::mount::open_install, app: 240, extra: None },
        Game {
            info: &combat_arms::game::INFO,
            open_install: combat_arms::game::mount::open_install,
            // Combat Arms: the Classic.
            app: 1_263_550,
            extra: Some((combat_arms::game::mount::loose_music, "music")),
        },
        // Half-Life 2: Deathmatch: its search path from `hl2mp/gameinfo.txt`;
        // its pieces' sets: `weapon/<id>`, `players` and `base` (`record_pieces`).
        Game { info: &hl2dm::game::INFO, open_install: hl2dm::game::mount::open_install, app: 320, extra: None },
        // Team Fortress 2: its search path from `tf/gameinfo.txt`, the
        // encrypted scripts read decrypted with this host's key
        // (`tf2_open_install`); its sets: `weapon/<id>` for M2's guns,
        // `players` and `map/<name>` (`record_pieces`).
        Game { info: &tf2::game::INFO, open_install: tf2_open_install, app: 440, extra: None },
    ]
}

/// TF2's install as the packer reads it: its `.ctx` scripts decrypted
/// with this host's key (`CHIMERA_TF2_CTX_KEY` or `[games.tf2.keys.ctx]`,
/// never written anywhere), so the store's records are plain KeyValues a
/// client reads without a key; with no key, the scripts are left out, said
/// loudly.
fn tf2_open_install(install: &std::path::Path) -> std::io::Result<Mount> {
    use tf2::game::scripts;
    let key = scripts::key().map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    if key.is_none() {
        eprintln!(
            "\n*** WARNING: no TF2 .ctx key ({} or [games.tf2.keys.ctx] key): the weapon and class scripts \
             can't be packed decrypted and are LEFT OUT; everything else is packed. ***\n",
            scripts::KEY_ENV
        );
    }
    Ok(scripts::decrypting(tf2::game::mount::open_install(install)?, key))
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
        Some("lean") => return lean(&args[1..]),
        Some("repack") => return repack(&args[1..]),
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
                "\na playtester enters it once (docs/playtest.md):\n  Play-UGG.bat +chimera:asset_token {token} +connect <host address>:<port>"
            );
            println!(
                "\na developer puts it in chimera.local.toml:\n[assets]\nurl = \"https://<the server's name>\"\ntoken = \"{token}\""
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
                    println!("  token {} {state}, created {} {}", t.id, chimera_assets::accounts::utc_text(t.created), t.note);
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

/// Each named piece of `game` (a weapon, the players, a Combat Arms body)
/// whose set name starts with `only`, and the paths loading it read
/// through `mount`.
fn record_pieces(game: &str, mount: &std::sync::Arc<Mount>, only: &str) -> Vec<(String, Vec<String>)> {
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
            // What any match with CS:S's guns plays, into `base` (merged):
            // its minigame cue pack's sounds (a rung up or down, the lead).
            if "base".starts_with(only) {
                take();
                if let Ok(pack) = chimera_core::cues::CuePack::parse(cs_source::game::parts::CUES) {
                    for file in pack.sounds.values() {
                        let _ = tapped.read(file);
                    }
                }
                // The party match-end screen's winner sound (a bot's cheer,
                // `parts::MATCH_WON_SOUND`; the 2026-11-12 rehearsal fetched
                // it on demand at a match's end).
                let _ = tapped.read(cs_source::game::parts::MATCH_WON_SOUND);
                // What its guns draw on another game's map (`bsp::
                // add_guest_looks`: impact decals, effect materials, the
                // muzzle flash, the spent shells' models) and their
                // shells' bounce sounds (played on any map).
                let mut materials = MaterialLoader::guest(&tapped, 0, 0);
                cs_source::game::bsp::add_guest_looks(&mut materials, &mut chimera_world::map::MapData::default());
                for (_, _, bounce) in cs_source::game::view_anim::SHELLS {
                    for wave in scripts.waves(bounce) {
                        let _ = tapped.read(&wave);
                    }
                }
                out.push(("base".into(), take()));
            }
            let options = map_options(false);
            record_maps(source_engine::maps::map_names(mount), only, &take, &mut out, |name| {
                let bytes = tapped.read(&format!("maps/{name}.bsp")).map_err(|e| e.to_string())?;
                cs_source::game::bsp::load_level_bytes_in(&tapped, name, bytes, &options, None).map(|_| ())
            });
        }
        "combat_arms" => {
            use combat_arms::game::{
                attachments::{ATTACHMENTS, AttachParams, SLOTS},
                characters::{models, preload},
                data::CaData,
                rungs,
                effects::data::{CaFxTables, CaWeaponFx, EFFECTS},
                model::normalize,
                view_model::{self, on_demand},
                world,
            };
            // Where a view model's hands load (`on_demand::load_weapon`'s).
            let hands_map = || {
                let mut map = chimera_world::map::MapData::default();
                map.memos.insert(world::BlocksOnly);
                map
            };
            let read = |p: &str| tapped.read(p).ok();
            let ca = CaData::install();
            for w in &ca.weapon_set().weapons {
                // Only weapons whose set `only` may keep.
                let wanted = "weapon/".starts_with(only) || only.starts_with("weapon/");
                if !on_demand::ours(&w.id) || !wanted {
                    continue;
                }
                take();
                let _ = on_demand::load_weapon(&ca, &w.id, &read);
                // Its held (third-person) model: what other players and
                // bots hold of it.
                let _ = combat_arms::game::characters::models::load_held_weapon(&ca, &w.id, &read);
                // Its own sounds (fire, silenced, dry, reload, select,
                // misc keys): played from the install when it first
                // fires, else a request each on demand.
                for path in w.hook::<CaWeaponFx>(EFFECTS).map(|fx| fx.sound_paths()).unwrap_or_default() {
                    let _ = tapped.read(&path);
                }
                // What it throws or fires: that object's sounds in flight
                // and at rest, create sound, flare sprite, model and skins
                // (played or drawn when used, else a request each).
                for path in combat_arms::game::special::object_files(w) {
                    let path = normalize(&path);
                    if path.ends_with(".ltb") {
                        let _ = combat_arms::game::model::load(&path, &read);
                    } else if let Some(bytes) = tapped.read(&path).ok().filter(|_| path.ends_with(".spr"))
                        && let Ok(sprite) = lithtech_formats::spr::Sprite::parse(&bytes)
                    {
                        for frame in &sprite.frames {
                            let _ = tapped.read(&normalize(frame));
                        }
                    }
                }
                let id = w.id.rsplit(':').next().unwrap_or(&w.id);
                out.push((format!("weapon/{id}"), take()));
            }
            // Each gun's fitted parts (`parts/<id>`): the view and held
            // models of the gun with each part a rung may draw for it
            // (`rungs::slot_candidates`), and with its default parts, as
            // `characters::preload` loads them, less the bare gun's files
            // (its `weapon/<id>`).
            let wanted = "parts/".starts_with(only) || only.starts_with("parts/");
            let table = ca.attachments();
            for w in ca.weapon_set().weapons.iter().filter(|w| wanted && on_demand::ours(&w.id)) {
                let name = rungs::parts_set(&w.id);
                let Some(params) = w.hook::<AttachParams>(ATTACHMENTS).filter(|_| name.starts_with(only)) else {
                    continue;
                };
                let mut keys = std::collections::BTreeSet::new();
                keys.insert(preload::fitted_key(table, &params, &w.id, &[]));
                for slot in 0..SLOTS {
                    for part in rungs::slot_candidates(table, &params, slot) {
                        keys.insert(preload::fitted_key(table, &params, &w.id, &[part]));
                    }
                }
                keys.remove(&w.id);
                if keys.is_empty() {
                    continue;
                }
                take();
                let _ = on_demand::load_cached(&ca, &w.id, &read);
                let _ = models::load_held_weapon(&ca, &w.id, &read);
                let bare: std::collections::BTreeSet<String> = take().into_iter().collect();
                for key in &keys {
                    let _ = on_demand::load_weapon(&ca, key, &read);
                    let _ = models::load_held_weapon(&ca, key, &read);
                }
                let got: Vec<String> = take().into_iter().filter(|p| !bare.contains(p)).collect();
                // The bare view model stays cached only while its parts load.
                on_demand::forget(&ca, &Default::default());
                out.push((name, got));
            }
            // Each body a rung may dress its players in (any record with a
            // body file: the player bodies, the NPC bodies, the infected
            // overrides).
            // With it, the first-person hands a rung's look of that body
            // wears when they aren't the default character's
            // (`rungs::body_hands`): its hand model and skins, as the
            // local player's view models (`<key>#h<model>_<face>_<uniform>`)
            // load them over the gun's own (`view_model::on_demand`).
            let bodies: Vec<i32> = ca.characters().model.iter().filter(|m| !m.file.is_empty()).map(|m| m.id).collect();
            let hands = rungs::body_hands(ca.characters());
            for id in bodies {
                let name = rungs::body_set(id);
                if !name.starts_with(only) {
                    continue;
                }
                take();
                if let Err(e) = models::load_body(&ca, id, &read) {
                    println!("{name}: {e}");
                }
                for choice in hands.get(&id).into_iter().flatten() {
                    if let Err(e) = view_model::load_hands(choice, &mut hands_map(), &read) {
                        println!("{name}: hands {}: {e}", choice.model);
                    }
                }
                out.push((name, take()));
            }
            // What any match with Combat Arms' guns plays, into `base`
            // (merged): the effect tables' files (impact, shell and
            // surface sounds, the event sounds, the heartbeat).
            if "base".starts_with(only) {
                take();
                for path in CaFxTables::built_in().sound_files() {
                    let _ = tapped.read(&normalize(path));
                }
                // The minigame cue pack's sounds (a rung up or down, the lead).
                if let Ok(pack) = chimera_core::cues::CuePack::parse(combat_arms::game::parts::CUES) {
                    for file in pack.sounds.values() {
                        let _ = tapped.read(&normalize(file));
                    }
                }
                // The result screen's win and lose sounds (a match's end).
                for file in [combat_arms::game::parts::RESULT_WIN_SOUND, combat_arms::game::parts::RESULT_LOSE_SOUND] {
                    let _ = tapped.read(&normalize(file));
                }
                // The attribute files the install data reads at the first
                // start (`data::attributes`; the 2026-11-13 rehearsal
                // fetched `clientbutes.txt` on demand there).
                {
                    use combat_arms::game::data::attributes as a;
                    for file in [
                        a::WEAPONS_FILE,
                        a::SERVER_FILE,
                        a::MODELS_FILE,
                        a::MOD_FILE,
                        a::MARKS_FILE,
                        a::FX_FILE,
                        a::ATTACHMENTS_FILE,
                        a::LAYOUT_FILE,
                        a::CLIENT_FILE,
                        a::PERKS_FILE,
                        a::SETS_FILE,
                    ] {
                        let _ = tapped.read(file);
                    }
                }
                // The effect looks every map takes, of every gun's groups
                // (`effects::library::add_looks`, on any game's map: the
                // install guns' own FX.TXT records name groups beyond the
                // tables'; the rehearsal fetched 7 of their textures on
                // demand at each join).
                combat_arms::game::effects::library::add_looks(&mut chimera_world::map::MapData::default(), &ca, &read);
                // The default character's first-person hands, on every
                // view model (`view_model::hands::default_choice`).
                if let Some(choice) = view_model::hands::default_choice(ca.characters()) {
                    let _ = view_model::load_hands(&choice, &mut hands_map(), &read);
                }
                // The debris table, read at a prop's first death, and the
                // models, skins and sounds of its records (the pieces).
                use combat_arms::game::effects::debris::{self, CaDebrisTable};
                if let Ok(bytes) = tapped.read(debris::FILE)
                    && let Ok(doc) = lithtech_formats::attributes::Document::parse(&bytes)
                {
                    for path in CaDebrisTable::parse(&doc).files() {
                        let path = normalize(path);
                        if path.ends_with(".ltb") {
                            let _ = combat_arms::game::model::load(&path, &read);
                        } else {
                            let _ = tapped.read(&path);
                        }
                    }
                }
                // A party host's air strike: its alarm, its shells'
                // model, skins, sounds and their effect groups' files
                // (`special::devices::strike_files`), sprites with their
                // frames, models as they load.
                let fx = mount
                    .read(combat_arms::game::effects::library::FX_FILE)
                    .ok()
                    .and_then(|b| lithtech_formats::fxf::FxFile::parse(&String::from_utf8_lossy(&b)).ok());
                for path in combat_arms::game::special::devices::strike_files(ca.weapon_set(), CaFxTables::built_in(), fx.as_ref()) {
                    if path.ends_with(".ltb") {
                        let _ = combat_arms::game::model::load(&path, &read);
                    } else if let Some(bytes) = tapped.read(&path).ok().filter(|_| path.ends_with(".spr"))
                        && let Ok(sprite) = lithtech_formats::spr::Sprite::parse(&bytes)
                    {
                        for frame in &sprite.frames {
                            let _ = tapped.read(&normalize(frame));
                        }
                    }
                }
                out.push(("base".into(), take()));
            }
            // Each map's world (its textures decoded: the sprites are
            // read only so), as another game's gameplay loads it, and the
            // sounds its objects name (the ambient `SoundFX` loops, doors'
            // and triggers' sounds), which play from the install later.
            let options = map_options(true);
            record_maps(world::map_names(mount), only, &take, &mut out, |name| {
                world::load_world_in(&tapped, name, &options, None)?;
                let path = world::find(mount, name).ok_or("no world file")?;
                let bytes = mount.read(&path).map_err(|e| e.to_string())?;
                let parsed = lithtech_formats::world::World::parse(&bytes).map_err(|e| e.to_string())?;
                for o in &parsed.objects {
                    for p in &o.properties {
                        if let lithtech_formats::world::Value::String(v) = &p.value
                            && v.trim().to_ascii_lowercase().ends_with(".wav")
                        {
                            let _ = tapped.read(&normalize(v.trim()));
                        }
                    }
                }
                Ok(())
            });
        }
        "hl2dm" => {
            use hl2dm::game::{bodies, weapons, weapons::inventory::DEFAULT_MODEL};
            use source_engine::content::{material::MaterialLoader, props, sound::SoundScripts};
            let scripts = SoundScripts::load_with(&mut MaterialLoader::guest(mount, 0, 0), "", true);
            let held = bodies::load_one(&tapped, DEFAULT_MODEL).ok().map(|c| c.bones);
            for w in &weapons::hl2_weapons().weapons {
                take();
                let mut materials = MaterialLoader::guest(&tapped, 0, 0);
                if let Some(view) = &w.models.view {
                    let _ = props::load_view_model(&mut materials, view, &w.id, true);
                }
                if let (Some(world), Some(bones)) = (&w.models.world, &held) {
                    let _ = props::load_held(&mut materials, world, &w.id, bones);
                }
                if let Some(bones) = &held {
                    for (key, path, weapon) in weapons::launch::MODELS {
                        if *weapon == w.id {
                            let _ = props::load_held(&mut materials, path, key, bones);
                        }
                    }
                }
                let swings = w.melee.iter().flat_map(|m| [Some(&m.primary), m.secondary.as_ref()]).flatten();
                let swung = swings.flat_map(|s| [&s.sound_hit, &s.sound_hit_world, &s.sound_miss]).flatten();
                for name in w.sounds.iter().flat_map(|s| s.entries()).chain(swung.map(String::as_str)) {
                    for wave in scripts.waves(name) {
                        let _ = tapped.read(&wave);
                    }
                }
                let id = w.id.rsplit(':').next().unwrap_or(&w.id).trim_start_matches("weapon_");
                out.push((format!("weapon/{id}"), take()));
            }
            take();
            for model in bodies::PLAYER_MODELS {
                let _ = bodies::load_one(&tapped, model);
            }
            out.push(("players".into(), take()));
            // What its guns draw on any map, into `base` (merged): the
            // effect materials, muzzle flashes, impact decals and spent
            // shells (models, bounce sounds) its
            // dressing brings to another game's map (`maps::
            // add_guest_looks`; on its own maps the map's kit reads the
            // same). They were in its maps' sets alone: a client without
            // the install fetched them on demand on CS:S's and Combat
            // Arms' maps.
            if "base".starts_with(only) {
                take();
                let mut materials = MaterialLoader::guest(&tapped, 0, 0);
                hl2dm::game::maps::add_guest_looks(&mut materials, &mut chimera_world::map::MapData::default());
                // Its shells' bounce sounds, played on any map.
                for (_, _, bounce) in hl2dm::game::shells::SHELLS {
                    for wave in scripts.waves(bounce) {
                        let _ = tapped.read(&wave);
                    }
                }
                out.push(("base".into(), take()));
            }
            let options = map_options(false);
            record_maps(source_engine::maps::map_names(mount), only, &take, &mut out, |name| {
                let bytes = tapped.read(&format!("maps/{name}.bsp")).map_err(|e| e.to_string())?;
                source_engine::content::bsp::load_level_bytes_in(
                    &hl2dm::game::maps::HL2DM,
                    &tapped,
                    name,
                    bytes,
                    &options,
                    None,
                )
                .map(|_| ())
            });
        }
        "tf2" => {
            // M2's guns (`sets::M2_WEAPONS`: the `c_` model, the class's
            // arms, sounds, the rocket, the decrypted script) and the
            // class models, read by TF2's own list (`sets::pieces`).
            take();
            out.extend(tf2::game::sets::pieces(mount));
            let options = map_options(false);
            record_maps(source_engine::maps::map_names(mount), only, &take, &mut out, |name| {
                let bytes = tapped.read(&format!("maps/{name}.bsp")).map_err(|e| e.to_string())?;
                source_engine::content::bsp::load_level_bytes_in(&tf2::game::maps::TF2, &tapped, name, bytes, &options, None)
                    .map(|_| ())
            });
        }
        _ => {}
    }
    out.retain(|(name, paths)| !paths.is_empty() && name.starts_with(only));
    out
}

/// How `sets` loads a map: as another game's gameplay loads it (the
/// world with its players' kit, `KitLoad::Players`, none of its guns),
/// never from the converted-map cache (a hit reads none of the files);
/// `textures` decoded or only read (Source's loader reads each texture
/// either way).
fn map_options(textures: bool) -> chimera_world::map::MapLoadOptions {
    chimera_world::map::MapLoadOptions {
        textures,
        kit: chimera_world::map::KitLoad::Players(&[]),
        ..Default::default()
    }
}

/// `map/<name>` for each map of `names` whose set starts with `only`:
/// the paths `load` reads of it (a map that fails or panics keeps what
/// it read, with a note).
fn record_maps(
    names: Vec<String>,
    only: &str,
    take: &dyn Fn() -> Vec<String>,
    out: &mut Vec<(String, Vec<String>)>,
    load: impl Fn(&str) -> Result<(), String>,
) {
    for name in names {
        let set = format!("map/{name}");
        if !set.starts_with(only) {
            continue;
        }
        take();
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| load(&name))) {
            Ok(Ok(())) => {}
            Ok(Err(e)) => println!("{set}: {e}"),
            Err(_) => println!("{set}: the load panicked"),
        }
        out.push((set, take()));
    }
}

/// Whether `sets` merges set `name` into what the manifest has (`base`
/// and the maps' sets, also recorded from `CHIMERA_VFS_LOG` logs) rather
/// than replacing it.
/// The voice and call-out files `game` declares it plays (its own loaders
/// read them from the install): `sets` puts them into `common`
/// (`resolve::declare`). CS:S and HL2:DM declare none yet.
fn declared_files(game: &str, mount: &std::sync::Arc<Mount>) -> Vec<String> {
    match game {
        "combat_arms" => combat_arms::game::declared::voice_files(&|p| mount.read(p).ok()),
        _ => Vec::new(),
    }
}

fn merged_set(name: &str) -> bool {
    name == "base" || name == chimera_assets::resolve::COMMON || name.starts_with("map/")
}

/// `sets <game>`: record the pieces' sets into the install build's
/// manifest.
fn sets(args: &[String]) -> Result<(), String> {
    let mut it = args.iter();
    let game_id = it.next().ok_or(USAGE)?;
    let (mut store, mut sign_it, mut only) = (None, false, String::new());
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--store" => store = Some(Store::new(it.next().ok_or("--store needs a value")?)),
            "--sign" => sign_it = true,
            "--only" => only = it.next().ok_or("--only needs a value")?.clone(),
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
    for (name, paths) in record_pieces(game.info.id, &mount, &only) {
        // A lean manifest (`lean`): what any match plays beyond `base`
        // goes to `common`, fetched with each match, not at startup.
        let name = if name == chimera_assets::resolve::BASE && manifest.sets.contains_key(chimera_assets::resolve::COMMON) {
            let base = manifest.sets.get(chimera_assets::resolve::BASE).cloned().unwrap_or_default();
            let paths: Vec<String> = paths.into_iter().filter(|p| !base.contains(p)).collect();
            (chimera_assets::resolve::COMMON.to_string(), paths)
        } else {
            (name, paths)
        };
        let (name, paths) = name;
        let (kept, left): (Vec<String>, Vec<String>) = paths.into_iter().partition(|p| manifest.files.contains_key(p));
        if !left.is_empty() {
            println!("set {name}: {} files not in the manifest left out (pack the whole install first)", left.len());
        }
        let mut kept: std::collections::BTreeSet<String> = kept.into_iter().collect();
        if name.starts_with("map/") {
            // What every match fetches anyway: the game's `base`,
            // `players` and `common` (`resolve::MATCH_SETS`).
            for shared in std::iter::once(chimera_assets::resolve::BASE).chain(chimera_assets::resolve::MATCH_SETS.iter().copied()) {
                if let Some(s) = manifest.sets.get(shared) {
                    kept.retain(|p| !s.contains(p));
                }
            }
        }
        if merged_set(&name)
            && let Some(old) = manifest.sets.get(&name)
        {
            kept.extend(old.iter().filter(|p| manifest.files.contains_key(*p)).cloned());
        }
        manifest.sets.insert(name.clone(), kept);
        let (n, plain, stored) = manifest.set_size(&name);
        println!("set {name}: {n} records, {} plain, {} stored", mb(plain), mb(stored));
    }
    // What the game declares it plays (voice lines, call-outs): in
    // `common` whatever a run read (`resolve::declare`; `lean` keeps it).
    if chimera_assets::resolve::DECLARED.starts_with(only.as_str()) {
        let files = declared_files(game.info.id, &mount);
        if !files.is_empty() {
            let (named, added) = chimera_assets::resolve::declare(&mut manifest, files);
            let paths = manifest.sets.get(chimera_assets::resolve::DECLARED).cloned().unwrap_or_default();
            let records: std::collections::BTreeSet<&str> = paths.iter().filter_map(|p| manifest.files.get(p)).map(|f| f.id.as_str()).collect();
            let stored: u64 = records.iter().map(|id| paths.iter().filter_map(|p| manifest.files.get(p)).find(|f| f.id == *id).map_or(0, |f| f.stored)).sum();
            println!("set declared: {named} files in the manifest, {added} new to common; {} records, {} stored", records.len(), mb(stored));
        }
    }
    manifest.save(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    if sign_it {
        let key = sign::load_or_create_key(&server::key_path(&store)).map_err(|e| e.to_string())?;
        sign::sign_manifest_file(&key, &path).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    println!("manifest: {}", path.display());
    Ok(())
}

/// `lean <game>`: `base` made lean from client runs' `CHIMERA_VFS_LOG`
/// logs (`chimera_assets::resolve::lean`), with `common` and a set per
/// music track, in the store's newest manifest of the game.
fn lean(args: &[String]) -> Result<(), String> {
    use chimera_assets::resolve;
    let mut it = args.iter();
    let game = it.next().ok_or(USAGE)?.clone();
    let (mut store, mut logs, mut sign_it, mut dry, mut build) = (None, Vec::new(), false, false, None);
    while let Some(arg) = it.next() {
        let mut value = || it.next().cloned().ok_or_else(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--store" => store = Some(Store::new(value()?)),
            "--from" => logs.push(PathBuf::from(value()?)),
            "--build" => build = Some(value()?),
            "--sign" => sign_it = true,
            "--dry-run" => dry = true,
            _ => return Err(format!("unknown option {arg}\n{USAGE}")),
        }
    }
    let store = store.ok_or("lean: pass --store")?;
    if logs.is_empty() {
        return Err("lean: pass --from <log> for each client run (at least one)".into());
    }
    let build = build.or_else(|| store.latest_build(&game)).ok_or_else(|| format!("no manifest of {game}"))?;
    let path = store.manifest_path(&game, &build);
    let mut manifest = Manifest::load(&path)?;
    let mut runs = Vec::new();
    for log in &logs {
        let (reads, checks) = chimera_assets::read_log_with_checks(log)?;
        let run = resolve::Run::of(&game, &reads, &checks);
        println!("{}: {} files read, {} only checked for", log.display(), run.reads.len(), run.checks.len());
        runs.push(run);
    }
    let before = manifest.set_size(resolve::BASE);
    let lean = resolve::lean(&manifest, &runs);
    let size = |paths: &std::collections::BTreeSet<String>| {
        let ids: std::collections::BTreeMap<&str, u64> =
            paths.iter().filter_map(|p| manifest.files.get(p)).map(|f| (f.id.as_str(), f.stored)).collect();
        (ids.len(), ids.values().sum::<u64>())
    };
    println!("{game} build {build}: base was {} records, {} stored", before.0, mb(before.2));
    for (what, paths) in [
        ("base (read by every run)", &lean.base),
        ("common (each match)", &lean.common),
        ("music (a set per track)", &lean.music),
        ("a ladder's pieces' (left out)", &lean.ladder),
        ("  of them only checked for", &lean.checked),
    ] {
        let (n, stored) = size(paths);
        println!("  {what}: {} files, {n} records, {}", paths.len(), mb(stored));
    }
    println!("  {} music track sets", lean.tracks.len());
    if dry {
        return Ok(());
    }
    lean.apply(&mut manifest);
    manifest.save(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    if sign_it {
        let key = sign::load_or_create_key(&server::key_path(&store)).map_err(|e| e.to_string())?;
        sign::sign_manifest_file(&key, &path).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("signed with the server's key {}", sign::public_hex(&key));
    }
    println!("manifest: {}", path.display());
    Ok(())
}

/// `repack`: the store's records packed again at `store::LEVEL` (those of
/// `--only`'s sets, else every manifest's), each manifest's stored sizes
/// set to the new records', signed again with `--sign`.
fn repack(args: &[String]) -> Result<(), String> {
    let mut it = args.iter();
    let (mut store, mut threads, mut only, mut sign_it, mut restat) = (None, 4usize, Vec::new(), false, false);
    while let Some(arg) = it.next() {
        let mut value = || it.next().cloned().ok_or_else(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--store" => store = Some(Store::new(value()?)),
            "--threads" => threads = value()?.parse().map_err(|_| "--threads: a number")?,
            "--only" => {
                for g in value()?.split(',') {
                    let (game, prefix) = g.split_once(':').ok_or("--only <game>:<set prefix>,...")?;
                    only.push((game.to_string(), prefix.to_string()));
                }
            }
            "--sign" => sign_it = true,
            "--restat" => restat = true,
            _ => return Err(format!("unknown option {arg}\n{USAGE}")),
        }
    }
    let store = store.ok_or("repack: pass --store")?;
    let mut manifests = Vec::new();
    let mut wanted = std::collections::BTreeMap::new();
    for game in store.games() {
        for build in store.builds(&game) {
            let path = store.manifest_path(&game, &build);
            let m = Manifest::load(&path)?;
            let mine: Vec<&(String, String)> = only.iter().filter(|(g, _)| *g == game).collect();
            if only.is_empty() {
                wanted.extend(m.files.values().map(|f| (f.id.clone(), f.size)));
            } else {
                for (name, paths) in m.sets.iter().filter(|(n, _)| mine.iter().any(|(_, p)| n.starts_with(p.as_str()))) {
                    let _ = name;
                    wanted.extend(paths.iter().filter_map(|p| m.files.get(p)).map(|f| (f.id.clone(), f.size)));
                }
            }
            manifests.push((path, m));
        }
    }
    let wanted: Vec<(String, u64)> = if restat {
        Vec::new()
    } else {
        wanted.into_iter().filter(|(id, _)| store.has(id)).collect()
    };
    println!("repacking {} records at zstd level {} on {threads} threads", wanted.len(), chimera_assets::store::LEVEL);
    let started = std::time::Instant::now();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let totals = std::sync::Mutex::new((0u64, 0u64, 0usize, Vec::new()));
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some((id, size)) = wanted.get(i) else { break };
                    let got = store.repack(id, *size);
                    let mut t = totals.lock().unwrap_or_else(|e| e.into_inner());
                    match got {
                        Ok((before, after)) => {
                            t.0 += before;
                            t.1 += after;
                        }
                        Err(e) => t.3.push(format!("{id}: {e}")),
                    }
                    t.2 += 1;
                    if t.2 % 2000 == 0 {
                        println!("  {} of {} records, {} -> {}", t.2, wanted.len(), mb(t.0), mb(t.1));
                    }
                }
            });
        }
    });
    let (before, after, _, failed) = totals.into_inner().unwrap_or_else(|e| e.into_inner());
    if !restat {
        println!(
        "{} -> {} ({:.1}% less), {:.0} s",
        mb(before),
        mb(after),
        100.0 * (1.0 - after as f64 / before.max(1) as f64),
        started.elapsed().as_secs_f64()
        );
    }
    for f in failed.iter().take(20) {
        println!("  failed: {f}");
    }
    let key = if sign_it {
        Some(sign::load_or_create_key(&server::key_path(&store)).map_err(|e| e.to_string())?)
    } else {
        None
    };
    for (path, mut m) in manifests {
        let n = store.restat(&mut m);
        m.save(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if let Some(key) = &key {
            sign::sign_manifest_file(key, &path).map_err(|e| format!("{}: {e}", path.display()))?;
        }
        println!("{}: {n} stored sizes changed{}", path.display(), if key.is_some() { ", signed" } else { "" });
    }
    if failed.is_empty() { Ok(()) } else { Err(format!("{} records failed", failed.len())) }
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
