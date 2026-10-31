//! Server capacity (docs/plans/active/multiplayer.md, "Capacity"): a
//! server on de_dust2 full of players (32 by default, CS:S's most), a few
//! of them clients over `NetSim`'s in-memory link and the rest bots, all
//! in deathmatch (everyone alive and moving, the worst case), measured
//! after a warm-up:
//!
//! - the server's CPU time per tick (every thread of the process while
//!   the server's frame runs; the clients run after it, one at a time),
//!   median and p99, against the 66-tick budget (15 ms) with margin, and
//!   its instructions per tick (all threads; stable under load);
//! - bytes sent to each client a second (its link's count), and the
//!   updates its `rate` choked;
//! - each client's CPU time per frame (headless: simulation, prediction,
//!   interpolation; no drawing);
//! - the process's memory;
//! - the share of character pairs in each other's PVS (what Source-style
//!   culling of bodies would keep).
//!
//! `CHIMERA_CAPACITY_PLAYERS` (32), `CHIMERA_CAPACITY_CLIENTS` (4: clients
//! cost the test time, the bots the server time), `CHIMERA_CAPACITY_MAP`
//! (de_dust2; any map of the install or the content cache),
//! `CHIMERA_CAPACITY_SECONDS` (20 game seconds measured) for bigger runs,
//! `CHIMERA_CAPACITY_CONSOLE` server console lines (e.g. `bot_stop 1`, to
//! tell what costs);
//! the budget is asserted only at the default size: instructions a tick
//! (p50, mean, p99; the server's own cost whatever the machine's load),
//! CPU time against the 66-tick budget only while the box is quiet
//! (1-minute load average under half the CPUs) or without hardware
//! counters (Windows: the mean, its thread clock being coarse). Prints a
//! table:
//! `cargo test --features dev --test it heavy::net_capacity -- --nocapture`.
//! Skipped without a CS:S install.

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use bevy::{
    ecs::message::{MessageCursor, Messages},
    prelude::*,
};
use crate::stack::{
    console::Console,
    core::{Health, Intent, LocalPlayer, MovementState, Team},
    games::{
        self,
        cs_source::{
            self,
            movement::{self as source, SourceMovementPlugin},
            weapons::CsWeaponsPlugin,
        },
    },
    greybox::GreyboxMapPlugin,
    harness::{CsSim, NetSim},
    map::{MapData, MapPlugin},
    metrics::AllThreads,
    mount::config::LocalConfig,
    net::{NetCharacter, NetEvent, NetSettings, client::Joined, maps::MapFiles, memory::LinkConditions},
    rules::Dead,
    slots::Loadout,
};

/// The 66-tick budget (CS:S's tick), ms.
const TICK_BUDGET_MS: f64 = 15.0;

/// Instructions a server tick may take, millions (all threads, user
/// space), in the dev build: measured p50 26.6-28.9, mean 28.5-29.9, p99
/// 44-54 over eight runs at load 1-5 per CPU (2026-10-16); the playtest
/// build takes fewer. A fixed 8.6M added to every tick fails the p50.
const INS_P50_MAX: f64 = 35.0;
const INS_MEAN_MAX: f64 = 35.0;
const INS_P99_MAX: f64 = 75.0;

/// Set in the process the test runs itself in under `cargo test`.
const CHILD: &str = "CHIMERA_CAPACITY_CHILD";

/// The 1-minute load average per logical CPU under which CPU time is
/// checked too.
const QUIET_LOAD_PER_CPU: f64 = 0.5;

fn installed() -> bool {
    let ok = LocalConfig::load()
        .ok()
        .and_then(|c| c.game_path_in(cs_source::GAME, cs_source::INFO.steam.as_ref()))
        .is_some_and(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    if !ok {
        eprintln!("skipping: no CS:S install configured");
    }
    ok
}

fn env<T: std::str::FromStr>(key: &str) -> Option<T> {
    chimera_core::env::var(key).ok()?.parse().ok()
}

/// The map, loaded once and handed to every world as a copy.
#[derive(Resource, Clone)]
struct OneMap(Arc<Mutex<Option<(String, MapData)>>>);

fn load_requested(world: &mut World, mut cursor: Local<MessageCursor<NetEvent>>) {
    let events: Vec<NetEvent> = cursor.read(world.resource::<Messages<NetEvent>>()).cloned().collect();
    for e in events {
        if let NetEvent::LoadMap { map, .. } = e {
            let data = world
                .resource::<OneMap>()
                .0
                .lock()
                .unwrap()
                .as_ref()
                .map(|(_, d)| d.clone());
            crate::stack::swap_map(world, &map, data, Duration::from_secs_f64(cs_source::TICK_INTERVAL));
        }
    }
}

/// Resident plus swapped-out memory of this process, MB (Linux).
fn rss_mb() -> Option<f64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let kb = |key: &str| -> Option<f64> {
        status
            .lines()
            .find(|l| l.starts_with(key))?
            .split_whitespace()
            .nth(1)?
            .parse()
            .ok()
    };
    Some((kb("VmRSS:")? + kb("VmSwap:").unwrap_or(0.0)) / 1000.0)
}

fn percentile(v: &[f64], p: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mut v = v.to_vec();
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * p).round() as usize]
}

/// A scripted player: walks about, turns now and then, shoots at the
/// nearest enemy in sight range.
struct Walker {
    rng: u64,
    axis: Vec2,
    yaw: f32,
    next_turn: u64,
}

impl Walker {
    fn next(&mut self) -> f32 {
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        (self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32 / (1u64 << 24) as f32
    }

    fn drive(&mut self, world: &mut World, step: u64) {
        let Some(me) = world.query_filtered::<Entity, With<LocalPlayer>>().iter(world).next() else {
            return;
        };
        let Some(team) = world.get::<Team>(me).map(|t| t.0) else {
            return;
        };
        if world.get::<Dead>(me).is_some() {
            return;
        }
        let eye = world.get::<Transform>(me).map_or(Vec3::ZERO, |t| t.translation)
            + world.get::<MovementState>(me).map_or(Vec3::Y * 0.6, |m| m.eye_offset);
        let enemy = world
            .query::<(Entity, &Team, &Health, &Transform, &NetCharacter)>()
            .iter(world)
            .filter(|(e, t, h, ..)| *e != me && t.0 != team && t.0 != 0 && h.current > 0.0)
            .map(|(.., tr, _)| tr.translation)
            .filter(|p| p.distance(eye) < 30.0)
            .min_by(|a, b| a.distance(eye).total_cmp(&b.distance(eye)));
        if step >= self.next_turn {
            self.next_turn = step + 30 + (self.next() * 120.0) as u64;
            let a = self.next() * std::f32::consts::TAU;
            self.axis = Vec2::new(a.cos(), a.sin());
            self.yaw = (self.yaw + (self.next() - 0.5) * 2.5).rem_euclid(std::f32::consts::TAU);
        }
        let jump = self.next() < 0.01;
        let mut i = world.get_mut::<Intent>(me).unwrap();
        i.move_axis = self.axis;
        i.jump = jump;
        match enemy {
            Some(at) => {
                let d = at + Vec3::Y * 0.4 - eye;
                i.yaw = (-d.x).atan2(-d.z).rem_euclid(std::f32::consts::TAU);
                i.pitch = d.y.atan2(d.xz().length());
                i.fire = !i.fire;
            }
            None => {
                i.yaw = self.yaw;
                i.pitch = 0.0;
                i.fire = false;
            }
        }
    }
}

struct Results {
    players: usize,
    clients: usize,
    tick_p50: f64,
    tick_p99: f64,
    kb_per_client: Vec<f64>,
    client_p50: f64,
    client_p99: f64,
    /// Mean CPU ms per tick: what a coarse clock (Windows' 15.6 ms
    /// steps) can still tell.
    tick_mean: f64,
    rss: Option<f64>,
    /// Instructions per tick, millions, p50 / p99 / mean (None without
    /// counters).
    ins: Option<(f64, f64, f64)>,
    /// The highest 1-minute load average per logical CPU seen at the
    /// start and end of the measured stretch (None outside Linux).
    load: Option<f64>,
}

/// The 1-minute load average per logical CPU (Linux; None elsewhere).
fn load_per_cpu() -> Option<f64> {
    let l: f64 = std::fs::read_to_string("/proc/loadavg")
        .ok()?
        .split_whitespace()
        .next()?
        .parse()
        .ok()?;
    Some(l / std::thread::available_parallelism().ok()?.get() as f64)
}

fn run(map: &str, players: usize, clients: usize, seconds: f64) -> Results {
    let id = if map.contains(':') {
        map.to_string()
    } else {
        format!("{}:{map}", cs_source::GAME)
    };
    let data = games::load_map_lean(&id).unwrap_or_else(|e| panic!("{id}: {e}"));
    let vis = data.visibility.clone();
    let one = OneMap(Arc::new(Mutex::new(Some((id.clone(), data.clone())))));
    let setup = {
        let one = one.clone();
        move |app: &mut App| {
            // A `--features profile` build: spans in a Chrome trace
            // (`TRACE_CHROME=<file>`; docs/performance.md, "Measuring");
            // profile with `CHIMERA_CAPACITY_CLIENTS=0` (one world).
            #[cfg(feature = "profile")]
            app.add_plugins(bevy::log::LogPlugin::default());
            app.add_plugins((
                GreyboxMapPlugin,
                MapPlugin::empty(),
                SourceMovementPlugin,
                CsWeaponsPlugin,
            ))
            .insert_resource(Loadout { movement: source::ID })
            .insert_resource(MapFiles {
                read: Arc::new(games::map_file_bytes),
                cache: None,
            })
            .insert_resource(one.clone())
            .add_systems(Update, load_requested);
        }
    };
    let mut sim = NetSim::new(CsSim, LinkConditions::default(), 7, 0, setup);
    // As the game and the dedicated server run (`CHIMERA_EXECUTOR=multi`:
    // Bevy's default, for comparison).
    let single = chimera_core::env::var("CHIMERA_EXECUTOR").as_deref() != Ok("multi");
    if single {
        crate::stack::single_threaded_schedules(&mut sim.server.app);
    }
    {
        let w = sim.server.app.world_mut();
        w.resource_mut::<NetSettings>().maxplayers = players as u32;
        crate::stack::swap_map(w, &id, Some(data), Duration::from_secs_f64(cs_source::TICK_INTERVAL));
        w.resource_mut::<Console>().submit(format!(
            "chimera_rounds 0; bot_join_after_player 0; bot_quota_mode fill; bot_quota {players}; {}",
            chimera_core::env::var("CHIMERA_CAPACITY_CONSOLE").unwrap_or_default()
        ));
    }
    let link = LinkConditions {
        latency: Duration::from_millis(25),
        jitter: Duration::from_millis(3),
        loss: 0.0,
    };
    for _ in 0..clients {
        let i = sim.add_client(|_| {});
        sim.set_conditions(i, link);
        if single {
            crate::stack::single_threaded_schedules(&mut sim.clients[i].app);
        }
    }
    let mut walkers: Vec<Walker> = (0..clients)
        .map(|i| Walker {
            rng: 0x9E37_79B9_7F4A_7C15 ^ (i as u64 + 1).wrapping_mul(0xD1B5_4A32_D192_ED03),
            axis: Vec2::ZERO,
            yaw: 0.0,
            next_turn: 0,
        })
        .collect();
    let ok = sim.until(4000, |s| {
        (0..s.clients.len()).all(|i| s.clients[i].app.world().contains_resource::<Joined>())
    });
    assert!(ok, "the clients joined");

    let mut server_cpu = AllThreads::default();
    let mut client_cpu = AllThreads::default();
    let mut tick_ms: Vec<f64> = Vec::new();
    let mut main_ms: Vec<f64> = Vec::new();
    // Instructions per tick, millions (all threads; Linux with hardware
    // counters): hardly move with the machine's load.
    let mut ins: Vec<f64> = Vec::new();
    let mut client_ms: Vec<f64> = Vec::new();
    let warm = (10.0 * crate::stack::DEFAULT_TICK_HZ) as u64;
    let measured = (seconds * crate::stack::DEFAULT_TICK_HZ) as u64;
    let frame = Duration::from_secs_f64(1.0 / crate::stack::DEFAULT_TICK_HZ);
    let mut bytes_at_start: Vec<u64> = Vec::new();
    // Pairs (a character, another living one) and those in the first's
    // PVS (its cluster sees the other's): what Source-style culling would
    // keep of the bodies sent.
    let (mut pairs, mut in_pvs) = (0u64, 0u64);
    let (mut alive_sum, mut alive_samples) = (0u64, 0u64);
    let started = Instant::now();
    let mut load = None;
    for step in 0..warm + measured {
        if step == warm {
            load = load_per_cpu();
            let l = sim.link.lock().unwrap();
            bytes_at_start = (0..clients)
                .map(|i| l.bytes_to.get(&sim.client_id(i)).copied().unwrap_or(0))
                .collect();
        }
        for (i, w) in walkers.iter_mut().enumerate() {
            w.drive(sim.clients[i].app.world_mut(), step);
        }
        sim.link.lock().unwrap().advance(frame);
        let before = sim.server.tick();
        let m0 = crate::stack::metrics::counters::thread_cpu_ns().unwrap_or(0);
        let c0 = server_cpu.read();
        sim.server.app.update();
        let c1 = server_cpu.read();
        let m1 = crate::stack::metrics::counters::thread_cpu_ns().unwrap_or(0);
        let ticks = sim.server.tick() - before;
        if step >= warm && ticks > 0 {
            let ms = (c1.cpu_ns - c0.cpu_ns) as f64 / 1e6 / ticks as f64;
            tick_ms.extend(std::iter::repeat_n(ms, ticks as usize));
            // The counter can read lower when the kernel rescales a
            // multiplexed counter under load: skip that sample.
            if let (Some(a), Some(b)) = (c0.hw, c1.hw)
                && let Some(d) = b.instructions.checked_sub(a.instructions)
            {
                ins.push(d as f64 / 1e6 / ticks as f64);
            }
            main_ms.extend(std::iter::repeat_n(
                (m1 - m0) as f64 / 1e6 / ticks as f64,
                ticks as usize,
            ));
        }
        if step >= warm
            && step % 16 == 0
            && let Some(vis) = &vis
        {
            let w = sim.server.app.world_mut();
            let living = w
                .query_filtered::<&Health, With<NetCharacter>>()
                .iter(w)
                .filter(|h| h.current > 0.0)
                .count();
            alive_sum += living as u64;
            alive_samples += 1;
            let at: Vec<Option<u32>> = w
                .query_filtered::<(&Transform, &Health), With<NetCharacter>>()
                .iter(w)
                .filter(|(_, h)| h.current > 0.0)
                .map(|(t, _)| vis.cluster_at(t.translation + Vec3::Y * 0.5))
                .collect();
            for (i, a) in at.iter().enumerate() {
                for (j, b) in at.iter().enumerate() {
                    if i != j {
                        pairs += 1;
                        if let (Some(a), Some(b)) = (a, b) {
                            in_pvs += u64::from(vis.sees(*a, *b));
                        } else {
                            in_pvs += 1;
                        }
                    }
                }
            }
        }
        for c in &mut sim.clients {
            let a = client_cpu.read();
            c.app.update();
            let b = client_cpu.read();
            if step >= warm {
                client_ms.push((b.cpu_ns - a.cpu_ns) as f64 / 1e6);
            }
        }
    }
    let load = load.zip(load_per_cpu()).map(|(a, b)| a.max(b));
    let world = sim.server.app.world_mut();
    // Updates each client was sent and those its `rate` choked.
    let paced: Vec<(u64, u64)> = world
        .query::<&crate::stack::net::server::UpdatePacing>()
        .iter(world)
        .map(|p| (p.sent, p.choked))
        .collect();
    if !paced.is_empty() {
        eprintln!("[capacity] updates sent / choked per client: {paced:?}");
    }
    let characters = world
        .query_filtered::<(), (With<NetCharacter>, With<Intent>)>()
        .iter(world)
        .count();
    eprintln!(
        "[capacity] {id}: {characters} characters, {clients} clients, {:.0} s real for {:.0} game s",
        started.elapsed().as_secs_f64(),
        (warm + measured) as f64 / crate::stack::DEFAULT_TICK_HZ
    );
    assert_eq!(characters, players, "the server holds the players");
    let l = sim.link.lock().unwrap();
    let kb_per_client = (0..clients)
        .map(|i| {
            let now = l.bytes_to.get(&sim.client_id(i)).copied().unwrap_or(0);
            (now - bytes_at_start[i]) as f64 / 1000.0 / seconds
        })
        .collect();
    eprintln!(
        "[capacity] others in a character's PVS: {:.0}% of {pairs} pairs; {:.1} living on average",
        in_pvs as f64 * 100.0 / pairs.max(1) as f64,
        alive_sum as f64 / alive_samples.max(1) as f64
    );
    eprintln!(
        "[capacity] server main thread CPU ms a tick p50 {:.2} p99 {:.2}; instructions a tick (M, all threads) p50 {:.2} mean {:.2}",
        percentile(&main_ms, 0.5),
        percentile(&main_ms, 0.99),
        percentile(&ins, 0.5),
        ins.iter().sum::<f64>() / ins.len().max(1) as f64
    );
    Results {
        players: characters,
        clients,
        tick_p50: percentile(&tick_ms, 0.5),
        tick_p99: percentile(&tick_ms, 0.99),
        tick_mean: tick_ms.iter().sum::<f64>() / tick_ms.len().max(1) as f64,
        kb_per_client,
        client_p50: percentile(&client_ms, 0.5),
        client_p99: percentile(&client_ms, 0.99),
        rss: rss_mb(),
        ins: (!ins.is_empty()).then(|| {
            (
                percentile(&ins, 0.5),
                percentile(&ins, 0.99),
                ins.iter().sum::<f64>() / ins.len() as f64,
            )
        }),
        load,
    }
}

#[test]
fn a_full_server_keeps_its_tick() {
    if !installed() {
        return;
    }
    // The budget counts every thread of the process, so the test needs a
    // process of its own: nextest gives each test one; under `cargo test`
    // other tests run on threads beside it (and share Bevy's task pools),
    // so it runs itself again in a child process and passes or fails
    // with it. (Counting the server's own thread alone isn't steadier:
    // its share of the parallel work grows when the task pools' threads
    // wait for a CPU, p50 16.7M at load 1.7 a CPU, 20.5M at 2.3, against
    // 26.6-29.2M for all threads; 2026-10-19.)
    if std::env::var_os("NEXTEST").is_none() && std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "heavy::net_capacity::a_full_server_keeps_its_tick",
                "--nocapture",
                "--test-threads",
                "1",
            ])
            .env(CHILD, "1")
            .status()
            .expect("the test binary runs again");
        assert!(status.success(), "the capacity run in its own process failed (its output is above)");
        return;
    }
    let players: usize = env("CHIMERA_CAPACITY_PLAYERS").unwrap_or(32);
    let clients: usize = env("CHIMERA_CAPACITY_CLIENTS").unwrap_or(4).min(players);
    let seconds: f64 = env("CHIMERA_CAPACITY_SECONDS").unwrap_or(20.0);
    let map: String = env("CHIMERA_CAPACITY_MAP").unwrap_or_else(|| "de_dust2".into());
    let r = run(&map, players, clients, seconds);
    let kb_mean = r.kb_per_client.iter().sum::<f64>() / r.kb_per_client.len().max(1) as f64;
    let kb_max = r.kb_per_client.iter().copied().fold(0.0, f64::max);
    eprintln!(
        "| map | players | clients | server tick CPU ms p50 / p99 | KB/s a client mean / max | upload KB/s at this size | client frame CPU ms p50 / p99 | memory MB |"
    );
    eprintln!(
        "| {map} | {} | {} | {:.2} / {:.2} | {kb_mean:.1} / {kb_max:.1} | {:.0} | {:.2} / {:.2} | {} |",
        r.players,
        r.clients,
        r.tick_p50,
        r.tick_p99,
        kb_mean * (r.players.saturating_sub(1)) as f64,
        r.client_p50,
        r.client_p99,
        r.rss.map_or("?".into(), |m| format!("{m:.0}")),
    );
    let default_size = chimera_core::env::var("CHIMERA_CAPACITY_PLAYERS").is_err() && map == "de_dust2";
    if let Some((p50, p99, mean)) = r.ins {
        eprintln!("[capacity] server instructions a tick (M, all threads) p50 {p50:.2} p99 {p99:.2} mean {mean:.2}");
    }
    eprintln!(
        "[capacity] server tick CPU ms mean {:.2}; load average per CPU {}",
        r.tick_mean,
        r.load.map_or("?".into(), |l| format!("{l:.2}"))
    );
    if default_size && !cfg!(feature = "profile") {
        // The server's own cost, first in instructions a tick (user
        // space, every thread; Linux hardware counters): unlike CPU time
        // they hardly move with the machine's load (mean within 5%
        // between load 1 and 5 per CPU). Counted over the whole process,
        // which is this test's alone (above).
        if let Some((p50, p99, mean)) = r.ins {
            assert!(
                p50 < INS_P50_MAX,
                "server tick p50 {p50:.1}M instructions (limit {INS_P50_MAX}M)"
            );
            assert!(
                mean < INS_MEAN_MAX,
                "server tick mean {mean:.1}M instructions (limit {INS_MEAN_MAX}M)"
            );
            assert!(
                p99 < INS_P99_MAX,
                "server tick p99 {p99:.1}M instructions (limit {INS_P99_MAX}M)"
            );
        }
        // CPU time against the 66-tick budget, where it means the
        // server's cost. CPU time a tick grows with the box's load
        // (shared cores and caches, clock speed): the same build ran p50
        // 6.5 ms at load 1.2 per CPU, 10.5 ms at 3.8 (over the old 10 ms
        // limit) with the same instructions. So on Linux it is checked
        // only while the 1-minute load average stays under half the
        // logical CPUs at both ends of the measured stretch; without
        // hardware counters always (then it is the only guard). Two
        // thirds of the budget for the median, all of it for the p99, in
        // the dev build. Windows' thread clock moves in 15.6 ms steps:
        // only the mean says anything there. Before the capacity pass
        // (multiplayer.md "Capacity") the p50 was ~13 ms in the playtest
        // build.
        let quiet = r.load.is_none_or(|l| l < QUIET_LOAD_PER_CPU);
        if quiet || r.ins.is_none() {
            if cfg!(target_os = "linux") {
                assert!(
                    r.tick_p50 < TICK_BUDGET_MS * 2.0 / 3.0,
                    "server tick p50 {:.2} ms CPU over two thirds of the 66-tick budget",
                    r.tick_p50
                );
                assert!(
                    r.tick_p99 < TICK_BUDGET_MS,
                    "server tick p99 {:.2} ms CPU over the 66-tick budget",
                    r.tick_p99
                );
            } else {
                assert!(
                    r.tick_mean < TICK_BUDGET_MS * 2.0 / 3.0,
                    "server tick mean {:.2} ms CPU over two thirds of the 66-tick budget",
                    r.tick_mean
                );
            }
        } else {
            eprintln!("[capacity] CPU time not checked: the box is busy (instructions are)");
        }
        // At the default 20 updates a second: the soak's 18 KB/s for
        // twelve characters plus each other character's body.
        assert!(kb_max < 40.0, "{kb_max:.1} KB/s to a client");
    }
}
