//! The command clock and prediction through a session's rough spots, on
//! `NetSim` with the engine's base, the greybox and a minigame with the
//! pre-match lobby (`mp_lobby`), its players frozen: the join and its
//! lobby, the match, a map change (`MapSettling` and the load's one-tick
//! frames, as `client::console` has them) and its lobby, the server
//! restarted and the client back on its own (`net::reconnect`) into the
//! new server's lobby. The server and the client run on their own frames
//! in real time: the client's are a loaded box's (8-31 ms, a hitch every
//! couple of seconds, some longer than `Time<Virtual>`'s 250 ms, the time
//! beyond dropped).
//!
//! Seen in the 2026-11-13 rehearsal: 6-8 clock jumps in a rejoined
//! client's first 6 s, 115-190 "nothing predicted" restarts per client
//! over a session. Here, before the fix: hitches made the client jump
//! (twice for one, the smoothed lead lagging the step) and restart for
//! every server state that came ahead of its clock. Now, per load, two
//! restarts and no jump: the clock's start (nothing predicted yet) and the
//! server letting the player in (`OwnState::alive`); a hitch that drops
//! time skips the ticks at once (`CommandClock::stalls`, `skipped`), a
//! late frame's states wait for their ticks (`predict`'s module doc).
//! A server hitch that drops time costs each client one jump back (the
//! server's clock lost that time; a client can only follow), one that
//! doesn't (its ticks caught up) none.

use std::time::Duration;

use bevy::{ecs::message::MessageCursor, prelude::*, time::TimeUpdateStrategy};
use chimera_core::{
    console::{Console, ConsoleAppExt},
    core::MapLoads,
    slots::Loadout,
};
use chimera_net::{
    harness::NetSim,
    minigame_net::{self, MinigameNetPlugin},
    net::{
        NetEvent,
        client::Joined,
        maps::map_name,
        memory::LinkConditions,
        predict::{CommandClock, NetGraph},
    },
    prematch,
};
use chimera_sim::{
    SimBasePlugins,
    minigame::{self, MinigameAppExt, MinigamePlugin, PreMatch},
    movement::placeholder,
    testkit,
};
use chimera_world::{
    greybox::GreyboxMapPlugin,
    map::{LoadedMapName, MapFile, MapSettling},
};

const DEF: &str = r#"
id = "kit:clock"
name = "Kit gun game"
players = { min = 1, max = 4 }
maps = ["chimera:greybox", "test:next"]
teams = "free_for_all"
time_limit = "5m"
rotate = false

[spawn]
respawn = { delay = "1s", protection = "0.5s" }
points = "map"

[rules]
module = "chimera:ladder"
ladder = ["kit:pistol", "kit:rifle", "kit:shotgun"]
"#;

const ID: &str = "kit:clock";
const TICK: f64 = 1.0 / 64.0;

fn setup(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, MinigamePlugin, MinigameNetPlugin))
        .insert_resource(Loadout { movement: placeholder::ID });
    testkit::add_weapons(app);
    app.add_minigames(&[("kit_clock.toml", DEF)]).expect("the definition reads");
    prematch::enable(app);
    // Players ready up: the lobby holds until the host starts it.
    app.world_mut().resource_mut::<Console>().submit("mp_lobby_ready_up 1");
    app.world_mut().resource_mut::<Console>().submit("mp_lobby_max_wait 300");
}

/// A test map loading on the server: its id and the frames left.
#[derive(Resource)]
struct ServerLoad(String, u32);

fn loaded(world: &mut World, map: &str) {
    world.insert_resource(LoadedMapName(map.to_string()));
    world.insert_resource(MapFile {
        name: map_name(map).to_string(),
        hash: None,
    });
}

fn server_loads(world: &mut World) {
    let Some(ServerLoad(map, left)) = world.remove_resource::<ServerLoad>() else { return };
    if left > 0 {
        world.insert_resource(ServerLoad(map, left - 1));
        return;
    }
    loaded(world, &map);
    chimera_core::gameplay::new_game(world);
    minigame::restart(world);
}

/// A client's load of the server's map, as the front end's: frames of
/// loading, then the map in (`MapLoads`) and settling behind the loading
/// screen (`MapSettling`), the frames advancing the simulation at most a
/// tick each meanwhile (`LoadCaps`).
#[derive(Resource)]
struct ClientLoad {
    map: String,
    loading: u32,
    settling: u32,
    max_delta: Duration,
}

fn client_loads(world: &mut World, mut cursor: Local<MessageCursor<NetEvent>>) {
    let asked: Vec<String> = cursor
        .read(world.resource::<Messages<NetEvent>>())
        .filter_map(|e| match e {
            NetEvent::LoadMap { map, .. } => Some(map.clone()),
            _ => None,
        })
        .collect();
    if let Some(map) = asked.into_iter().last() {
        let max_delta = match world.remove_resource::<ClientLoad>() {
            Some(l) => l.max_delta,
            None => world.resource::<Time<Virtual>>().max_delta(),
        };
        world.resource_mut::<Time<Virtual>>().set_max_delta(Duration::from_secs_f64(TICK));
        world.insert_resource(MapSettling);
        world.insert_resource(ClientLoad { map, loading: 60, settling: 60, max_delta });
    }
    let Some(mut load) = world.remove_resource::<ClientLoad>() else { return };
    if load.loading > 0 {
        load.loading -= 1;
        if load.loading == 0 {
            let map = load.map.clone();
            loaded(world, &map);
            world.get_resource_or_init::<MapLoads>().0 += 1;
        }
        world.insert_resource(load);
        return;
    }
    if load.settling > 0 {
        load.settling -= 1;
        world.insert_resource(load);
        return;
    }
    world.remove_resource::<MapSettling>();
    world.resource_mut::<Time<Virtual>>().set_max_delta(load.max_delta);
}

/// The server and the client on their own frames in real time.
struct Frames {
    now: f64,
    server_next: f64,
    server_last: f64,
    client_next: f64,
    client_last: f64,
    server_k: u64,
    client_k: u64,
    /// Frame `k`'s length (s), the server's and the client's.
    server: fn(u64) -> f64,
    client: fn(u64) -> f64,
}

impl Frames {
    fn new(server: fn(u64) -> f64, client: fn(u64) -> f64) -> Self {
        Self {
            now: 0.0,
            server_next: 0.0,
            server_last: TICK,
            client_next: 0.0,
            client_last: TICK,
            server_k: 0,
            client_k: 0,
            server,
            client,
        }
    }

    fn advance(&mut self, sim: &mut NetSim, to: f64) {
        if to > self.now {
            sim.link.lock().unwrap().advance(Duration::from_secs_f64(to - self.now));
            self.now = to;
        }
    }

    /// Real time on by `secs`, each app updating when its frame comes:
    /// with the length of the frame before (the time since it last ran).
    fn run(&mut self, sim: &mut NetSim, secs: f64, count: &mut Count) {
        let end = self.now + secs;
        while self.server_next.min(self.client_next) < end {
            if self.client_next <= self.server_next {
                let t = self.client_next;
                self.advance(sim, t);
                let last = Duration::from_secs_f64(self.client_last);
                for c in &mut sim.clients {
                    c.app.insert_resource(TimeUpdateStrategy::ManualDuration(last));
                    c.app.update();
                }
                count.see(sim);
                self.client_last = (self.client)(self.client_k);
                self.client_k += 1;
                self.client_next = t + self.client_last;
            } else {
                let t = self.server_next;
                self.advance(sim, t);
                let last = Duration::from_secs_f64(self.server_last);
                sim.server.app.insert_resource(TimeUpdateStrategy::ManualDuration(last));
                sim.server.app.update();
                self.server_last = (self.server)(self.server_k);
                self.server_k += 1;
                self.server_next = t + self.server_last;
            }
        }
        self.advance(sim, end);
    }

    fn until(&mut self, sim: &mut NetSim, max_secs: f64, count: &mut Count, done: impl Fn(&mut NetSim) -> bool) -> bool {
        let mut waited = 0.0;
        while waited < max_secs {
            if done(sim) {
                return true;
            }
            self.run(sim, 0.1, count);
            waited += 0.1;
        }
        done(sim)
    }
}

/// A dedicated server on a quiet core: a tick a frame.
fn steady(_: u64) -> f64 {
    TICK
}

/// A client on a loaded box: frames of 8-31 ms, and every 90th a hitch:
/// 120 ms (its ticks caught up after), 400 ms or 600 ms (beyond
/// `Time<Virtual>`'s 250 ms: the rest dropped).
fn loaded_box(k: u64) -> f64 {
    if k % 90 == 89 {
        return [0.12, 0.4, 0.12, 0.6][(k / 90 % 4) as usize];
    }
    0.008 + 0.001 * ((k * 7) % 24) as f64
}

/// What the client's readouts went through, summed over its connections
/// and maps (each starts its counts over).
#[derive(Default, Debug, Clone, Copy, PartialEq)]
struct Count {
    jumps: u32,
    restarts: u64,
    stalls: u32,
    last: (u32, u64, u32),
}

impl Count {
    fn see(&mut self, sim: &NetSim) {
        let w = sim.clients[0].app.world();
        let (clock, graph) = (w.resource::<CommandClock>(), w.resource::<NetGraph>());
        let now = (clock.jumps, graph.resyncs, clock.stalls);
        let grew = |a: u64, b: u64| a.saturating_sub(b);
        // A count lower than before started over (a map change, a new
        // connection): all of it is new.
        let since = |a: u64, b: u64| if a < b { a } else { grew(a, b) };
        self.jumps += since(now.0 as u64, self.last.0 as u64) as u32;
        self.restarts += since(now.1, self.last.1);
        self.stalls += since(now.2 as u64, self.last.2 as u64) as u32;
        self.last = now;
    }

    fn since(&self, before: &Count) -> (u32, u64, u32) {
        (self.jumps - before.jumps, self.restarts - before.restarts, self.stalls - before.stalls)
    }
}

fn in_lobby(sim: &NetSim) -> bool {
    sim.server.app.world().contains_resource::<PreMatch>()
}

fn joined(sim: &mut NetSim) -> bool {
    sim.clients[0].app.world().contains_resource::<Joined>()
        && sim.local_player(0).is_some()
        && sim.clients[0].app.world().resource::<CommandClock>().tick.is_some()
}

fn sim(server: fn(u64) -> f64, client: fn(u64) -> f64) -> (NetSim, Frames) {
    let link = LinkConditions {
        latency: Duration::from_millis(30),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(SimBasePlugins, link, 31, 1, setup);
    sim.clients[0].app.add_systems(Update, client_loads);
    (sim, Frames::new(server, client))
}

fn serve(sim: &mut NetSim) {
    sim.server.app.add_systems(Update, server_loads);
    sim.server.app.console_command("changelevel", "changelevel <map>: a test's.", |w, a| {
        let map = a.first().cloned().unwrap_or_default();
        w.insert_resource(LoadedMapName(map.clone()));
        w.insert_resource(ServerLoad(map, 30));
        Ok(None)
    });
    minigame::start(sim.server.app.world_mut(), ID).expect("it starts");
}

#[test]
fn hitches_through_a_join_a_lobby_a_map_change_and_a_rejoin_make_no_clock_jumps() {
    let (mut sim, mut frames) = sim(steady, loaded_box);
    serve(&mut sim);
    let mut count = Count::default();
    let mut phases = Vec::new();

    // The join and 6 s of its lobby, frozen.
    let before = count;
    assert!(frames.until(&mut sim, 20.0, &mut count, joined), "the client joined");
    frames.run(&mut sim, 6.0, &mut count);
    assert!(in_lobby(&sim), "still the lobby");
    phases.push(("join and lobby", count.since(&before)));

    // The match: everyone spawned together.
    let before = count;
    sim.server.app.world_mut().resource_mut::<Console>().submit("lobby_start 0");
    frames.run(&mut sim, 6.0, &mut count);
    assert!(!in_lobby(&sim), "the match plays");
    phases.push(("the match", count.since(&before)));

    // A map change: the client loads and settles, then the new lobby.
    let before = count;
    minigame_net::change_match(sim.server.app.world_mut(), ID, Some("test:next")).expect("the change");
    assert!(frames.until(&mut sim, 5.0, &mut count, |s| !joined(s)), "the client left the old map");
    assert!(frames.until(&mut sim, 30.0, &mut count, |s| joined(s) && in_lobby(s)), "back in, the lobby");
    frames.run(&mut sim, 6.0, &mut count);
    phases.push(("map change and lobby", count.since(&before)));

    // The server restarts: the client joins it again on its own, into
    // its lobby.
    let before = count;
    sim.stop_server();
    frames.run(&mut sim, 2.0, &mut count);
    sim.start_server();
    serve(&mut sim);
    assert!(frames.until(&mut sim, 30.0, &mut count, joined), "the client rejoined");
    frames.run(&mut sim, 6.0, &mut count);
    assert!(in_lobby(&sim), "the new server's lobby");
    phases.push(("rejoin and lobby", count.since(&before)));

    println!("(jumps, restarts, hitches that dropped time) per phase: {phases:?}");
    for (phase, (jumps, restarts, _)) in phases {
        assert_eq!(jumps, 0, "{phase}: the clock jumped");
        // The clock's start and the server letting the player in; none
        // in the match (its spawn is the lobby's).
        let most = if phase == "the match" { 0 } else { 2 };
        assert!(restarts <= most, "{phase}: {restarts} prediction restarts");
    }
    assert!(count.stalls >= 4, "the client's hitches dropped time: {count:?}");
}

/// The server's own hitches: one its frames catch up on (120 ms) makes no
/// jump; one beyond `Time<Virtual>`'s 250 ms drops that much of the
/// server's clock, and the client jumps back to it once.
#[test]
fn a_server_hitch_costs_a_client_at_most_one_jump() {
    fn hitchy(k: u64) -> f64 {
        match k % 200 {
            199 => [0.12, 0.6][(k / 200 % 2) as usize],
            _ => TICK,
        }
    }
    let (mut sim, mut frames) = sim(hitchy, |_| 1.0 / 144.0);
    serve(&mut sim);
    let mut count = Count::default();
    assert!(frames.until(&mut sim, 20.0, &mut count, joined), "the client joined");
    frames.run(&mut sim, 1.0, &mut count);
    let before = count;
    let k = frames.server_k;
    frames.run(&mut sim, 25.0, &mut count);
    let dropping = (k..frames.server_k).filter(|&i| hitchy(i) > 0.25).count() as u32;
    let (jumps, restarts, stalls) = count.since(&before);
    println!("{dropping} server hitches beyond 250 ms: {jumps} jumps, {restarts} restarts");
    assert!(dropping >= 2);
    assert_eq!(stalls, 0, "the client's frames dropped nothing");
    assert!(jumps <= dropping && jumps + 1 >= dropping, "{jumps} jumps for {dropping} hitches");
    assert!(restarts <= jumps as u64, "{restarts} restarts for {jumps} jumps");
}
