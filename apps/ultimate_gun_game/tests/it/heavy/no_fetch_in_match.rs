//! No game file fetched on demand while a match runs, for a player
//! without the installs (docs/playtest-hosting.md, "Publishing the game
//! files"; `chimera_net::net::on_demand`): a seeded session as a tester
//! plays it, the match's files all arriving with the join's or level
//! change's download (the offered sets).
//!
//! - **The asset service**, in this process: the host's published store
//!   (`CHIMERA_GUARD_STORE`, else this machine's server store,
//!   `chimera_assets::server::default_store`) read through a scratch
//!   store: its manifests copied (signed again with the scratch service's
//!   own key), its records linked, a scratch account; HTTPS with a
//!   certificate of its own (`tls::front`).
//! - **The game server**, in this process: Ultimate Gun Game's dedicated
//!   server (`chimera_net::dedicated::app`, as its binary builds it) on
//!   the installs, a fixed rung seed, prefetch off, three bots.
//! - **The client**: this crate's `ultimate_gun_game` binary, offscreen,
//!   with no install (its own home, an empty config, no Steam), the
//!   service's token bound in its access file (no restart), its own empty
//!   store: it downloads at start and at each join as a tester does.
//!
//! The session (`SESSION`): a ladder (`ugg_new <seed> 13 <map>`) per map;
//! once the client says its match started, every character on the server
//! (the bots and the client's player) climbs the whole ladder, a rung every
//! `RUNG_SECONDS`, so the client draws each rung's gun, body and parts,
//! held and in first person; then the next map. On the last map the
//! host's surprises follow (`SURPRISES`: an air strike, its siren,
//! shells and explosions, then moon gravity). Any on-demand fetch the
//! client logs during a match (`assets: fetched on demand during the match
//! ...`) fails the test, listed with what covers it. Skipped with a note
//! without the CS:S and Combat Arms installs, without a published store
//! holding both games' manifests, or off Unix (the records are linked);
//! `CHIMERA_FETCH_GUARD=0` skips it. A client that fails to start (no GPU)
//! fails it with its log. About 200 s (2026-11-12, a loaded box: each map
//! in its match 30-75 s after the ladder, 27 s of climbing).
//!
//! `cargo test -p ultimate_gun_game --features dev --test it heavy::no_fetch_in_match -- --nocapture`

use std::{
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use bevy::prelude::*;
use chimera_core::{
    console::Console,
    mount::{
        assets::{Store, access::Access, accounts::Accounts, server, tls},
        config::LocalConfig,
    },
};
use chimera_net::dedicated::{self, GameServer, ServerHooks, ServerMinigame, StartMap};
use chimera_sim::minigame::ladder::{Ladder, Rung, RungSeed, set_rung};
use party_kit::server::PARTY;

/// Each map's ladder: its seed and map (ladder seeds 217377-217379, as the
/// measured session in docs/playtest-hosting.md).
const SESSION: [(u64, &str); 3] = [
    (217_377, "combat_arms:warehouse"),
    (217_378, "cs_source:de_aztec"),
    (217_379, "cs_source:de_dust2"),
];
/// The rungs' extras' seed (bodies, parts, melee).
const RUNG_SEED: u64 = 4242;
/// How long each rung is held.
const RUNG_SECONDS: f64 = 2.0;
/// The host's surprises on the last map (`party_kit::surprise`), and how
/// long they are watched.
const SURPRISES: [&str; 2] = ["surprise airstrike 12 3", "surprise gravity 6"];
const SURPRISE_SECONDS: f64 = 10.0;
/// The longest the client may take to reach a match (its first start
/// downloads every game's `base`).
const JOIN_LIMIT: Duration = Duration::from_secs(600);

/// The dedicated server's own (apps/ultimate_gun_game_server), with the
/// launch seed fixed by the test.
static SERVER: GameServer = GameServer {
    title: "Ultimate Gun Game",
    a_title: "an Ultimate Gun Game",
    usage: "",
    hooks: ServerHooks {
        minigame: Some(ServerMinigame {
            default: ultimate_gun_game::MINIGAME,
            first_map: ultimate_gun_game::first_map,
        }),
        launch: Some(|app| {
            party_kit::server::launch(app);
            app.add_plugins(ultimate_gun_game::UltimateGunGamePlugin::launch());
        }),
        ..PARTY.hooks
    },
    ..PARTY
};

fn installed(config: &LocalConfig, game: &str, marker: &str) -> bool {
    config.game_path(game).is_some_and(|p| chimera_core::mount::served::usable(game, &p, marker))
}

/// The host's published store, when it has every game the session plays.
fn published() -> Option<Store> {
    let store = std::env::var_os("CHIMERA_GUARD_STORE")
        .map(Store::new)
        .or_else(server::default_store)?;
    ["cs_source", "combat_arms"]
        .iter()
        .all(|g| store.root.join("manifests").join(g).is_dir())
        .then_some(store)
}

/// The client's lines, kept as they arrive.
#[derive(Clone, Default)]
struct Lines(Arc<Mutex<Vec<String>>>);

impl Lines {
    fn follow(&self, from: impl std::io::Read + Send + 'static) {
        let lines = self.0.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(from).lines().map_while(Result::ok) {
                lines.lock().unwrap().push(line);
            }
        });
    }

    fn all(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }

    fn count(&self, pat: &str) -> usize {
        self.0.lock().unwrap().iter().filter(|l| l.contains(pat)).count()
    }

    fn tail(&self, n: usize) -> String {
        let all = self.all();
        all[all.len().saturating_sub(n)..].join("\n")
    }
}

/// The client process, killed with its process group when dropped (after
/// its startup download the game runs as a child of the first process,
/// which waits for it: `relaunch::start_game`).
struct Client(Child);

impl Drop for Client {
    fn drop(&mut self) {
        #[cfg(unix)]
        let _ = Command::new("kill").args(["-KILL", "--", &format!("-{}", self.0.id())]).status();
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The scratch folder, removed when dropped.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A scratch store reading `published`'s records (linked) with copies of
/// its manifests, which the scratch service signs again with its own key.
#[cfg(unix)]
fn scratch_store(published: &Store, dir: &Path) -> Store {
    let store = Store::new(dir.join("service"));
    std::fs::create_dir_all(&store.root).unwrap();
    std::os::unix::fs::symlink(published.root.join("records"), store.root.join("records")).unwrap();
    for game in std::fs::read_dir(published.root.join("manifests")).unwrap().flatten() {
        let to = store.root.join("manifests").join(game.file_name());
        std::fs::create_dir_all(&to).unwrap();
        for m in std::fs::read_dir(game.path()).unwrap().flatten() {
            if m.path().extension().is_some_and(|e| e == "txt") {
                std::fs::copy(m.path(), to.join(m.file_name())).unwrap();
            }
        }
    }
    store
}

/// One server frame, at most `dedicated::FPS_MAX` a second.
fn step(app: &mut App) {
    app.update();
    std::thread::sleep(Duration::from_secs_f64(1.0 / dedicated::FPS_MAX));
}

/// Step the server until `done` or `limit`; false at the limit.
fn until(app: &mut App, client: &mut Client, limit: Duration, mut done: impl FnMut(&mut App) -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        if done(app) {
            return true;
        }
        if let Ok(Some(status)) = client.0.try_wait() {
            panic!("the client exited ({status})");
        }
        step(app);
    }
    false
}

/// Every character with a rung on the server (bots and players).
fn climbers(world: &mut World) -> Vec<Entity> {
    world.query_filtered::<Entity, With<Rung>>().iter(world).collect()
}

#[test]
fn a_client_without_installs_fetches_nothing_on_demand_during_a_match() {
    if std::env::var("CHIMERA_FETCH_GUARD").is_ok_and(|v| v.trim() == "0") {
        eprintln!("skipping: CHIMERA_FETCH_GUARD=0");
        return;
    }
    if !cfg!(unix) {
        eprintln!("skipping: links the published store's records (Unix only)");
        return;
    }
    let Ok(config) = LocalConfig::load() else {
        eprintln!("skipping: no local config");
        return;
    };
    if !installed(&config, "cs_source", "cstrike") || !installed(&config, "combat_arms", "Game") {
        eprintln!("skipping: needs the CS:S and Combat Arms installs");
        return;
    }
    let Some(published) = published() else {
        eprintln!("skipping: no published asset store with CS:S's and Combat Arms' manifests (CHIMERA_GUARD_STORE)");
        return;
    };
    #[cfg(unix)]
    run(&published);
}

#[cfg(unix)]
fn run(published: &Store) {
    let started = Instant::now();
    let scratch = Scratch(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("no_fetch_in_match_{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&scratch.0);
    let store = scratch_store(published, &scratch.0);
    let mut accounts = Accounts::default();
    let (_, token) = accounts.add("guard", None, "test").unwrap();
    accounts.save(&Accounts::path(&store.root)).unwrap();
    let running = server::serve(server::Service::new(store).unwrap(), "127.0.0.1:0", 8).unwrap();
    let identity = tls::Identity::from_seed(&[7; 32], [9; 16]);
    let front = tls::front("127.0.0.1:0", running.addr, &identity, Default::default(), Default::default()).unwrap();
    let (url, fingerprint) = (front.url(), identity.fingerprint());

    // The game server on the installs.
    let (first_seed, first_map) = SESSION[0];
    let args: Vec<String> = [
        "-port",
        "0",
        "+maxplayers",
        "4",
        "+MaxPlayer",
        "3",
        "+map",
        first_map,
        "+chimera:asset_url",
        &url,
        "+chimera:asset_fingerprint",
        &fingerprint,
        "+chimera:asset_prefetch_max",
        "0",
        "+mp_lobby_countdown",
        "2",
        "+mp_lobby_max_wait",
        "30",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let options = match SERVER.parse(args).unwrap() {
        dedicated::Parsed::Run(o) => o,
        dedicated::Parsed::Help => unreachable!(),
    };
    let start = StartMap::load(&SERVER, &options).expect("the first map loads");
    let mut app = dedicated::app(&SERVER, start, &options);
    app.insert_resource(RungSeed(RUNG_SEED));
    chimera_world::harness::single_threaded_schedules(&mut app);
    app.finish();
    app.cleanup();
    step(&mut app);
    let addr = dedicated::listen(app.world_mut()).expect("the server listens");
    app.world_mut().resource_mut::<Console>().submit(&format!("ugg_new {first_seed} 13 {first_map}"));
    step(&mut app);

    // The client: no install, its token bound to the service.
    let home = scratch.0.join("client");
    for d in [".config", ".cache", ".local/share", "cfg"] {
        std::fs::create_dir_all(home.join(d)).unwrap();
    }
    std::fs::write(home.join("empty.toml"), "").unwrap();
    Access { token: Some(token), url: Some(url.clone()), fingerprint: Some(fingerprint.clone()) }
        .save_to(&home.join("access.toml"))
        .unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ultimate_gun_game"));
    for (k, _) in std::env::vars_os() {
        let k = k.to_string_lossy().to_string();
        if k.starts_with("CHIMERA_") || k.starts_with("MASHUP_") {
            cmd.env_remove(k);
        }
    }
    cmd.current_dir(&home)
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("XDG_CACHE_HOME", home.join(".cache"))
        .env("XDG_DATA_HOME", home.join(".local/share"))
        .env("CHIMERA_CONFIG", home.join("empty.toml"))
        .env("CHIMERA_STEAM_ROOT", "")
        .env("CHIMERA_CFG_DIR", home.join("cfg"))
        .env("CHIMERA_ASSET_ACCESS", home.join("access.toml"))
        .env("CHIMERA_ASSET_STORE", home.join("store"))
        .env("CHIMERA_ASSET_FETCH_LOG", home.join("fetch.log"))
        .args(["--offscreen", "320x180", "+fps_max", "30", "+connect", &format!("127.0.0.1:{}", addr.port())])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    let mut child = cmd.spawn().expect("the client starts");
    let lines = Lines::default();
    lines.follow(child.stdout.take().unwrap());
    lines.follow(child.stderr.take().unwrap());
    let mut client = Client(child);

    let mut rows = Vec::new();
    for (i, (seed, map)) in SESSION.iter().enumerate() {
        let map_started = Instant::now();
        if i > 0 {
            app.world_mut().resource_mut::<Console>().submit(&format!("ugg_new {seed} 13 {map}"));
        }
        let name = map.split(':').nth(1).unwrap_or(map);
        let pattern = format!("assets: the match on {name} started");
        let pattern_full = format!("assets: the match on {map} started");
        let before = lines.count(&pattern) + lines.count(&pattern_full);
        assert!(
            until(&mut app, &mut client, JOIN_LIMIT, |_| lines.count(&pattern) + lines.count(&pattern_full) > before),
            "{map}: the client's match didn't start within {JOIN_LIMIT:?}; its last lines:\n{}",
            lines.tail(40)
        );
        let joined = map_started.elapsed();
        // Everyone climbs the whole ladder.
        let ladder = app.world().resource::<Ladder>().clone();
        for r in 0..ladder.rungs.len() {
            let who = climbers(app.world_mut());
            for (k, e) in who.iter().enumerate() {
                if app.world().get_entity(*e).is_ok() {
                    set_rung(app.world_mut(), *e, (r + k) % ladder.rungs.len(), &ladder);
                }
            }
            let t = Instant::now();
            until(&mut app, &mut client, Duration::from_secs_f64(RUNG_SECONDS), |_| t.elapsed().as_secs_f64() >= RUNG_SECONDS);
        }
        if i + 1 == SESSION.len() {
            for c in SURPRISES {
                app.world_mut().resource_mut::<Console>().submit(c);
            }
            let t = Instant::now();
            until(&mut app, &mut client, Duration::from_secs_f64(SURPRISE_SECONDS), |_| t.elapsed().as_secs_f64() >= SURPRISE_SECONDS);
        }
        rows.push(format!(
            "{map}: in the match {:.0} s after the ladder ({} rungs climbed in {:.0} s)",
            joined.as_secs_f64(),
            ladder.rungs.len(),
            map_started.elapsed().as_secs_f64() - joined.as_secs_f64()
        ));
    }
    let all = lines.all();
    drop(client);
    drop(app);
    drop(front);
    drop(running);
    let fetched: Vec<&String> = all.iter().filter(|l| l.contains("assets: fetched on demand during the match")).collect();
    for r in &rows {
        eprintln!("{r}");
    }
    for l in all.iter().filter(|l| l.contains("assets: the match on")) {
        eprintln!("client: {l}");
    }
    eprintln!("session took {:.0} s", started.elapsed().as_secs_f64());
    assert!(
        fetched.is_empty(),
        "{} on-demand fetches while a match ran (each should come with the join's or level change's download):\n{}\n\
         Cover each file with a set the offer names (docs/playtest-hosting.md, \"Publishing the game files\"): \
         a gun's own files go in its `weapon/<id>` set, a rung body's in `character/<record>`, fitted parts in \
         `parts/<id>`, what the map's load reads in `map/<name>` (`chimera_pack sets <game> --store <server store> \
         --sign`, `--only <prefix>` for one kind), what every match of a game reads in its `base`; a set the offer \
         doesn't name yet goes in `chimera_net::net::assets::offer` or the game's `AssetHooks`. Then publish again.",
        fetched.len(),
        fetched.iter().map(|l| format!("  {l}")).collect::<Vec<_>>().join("\n")
    );
}
