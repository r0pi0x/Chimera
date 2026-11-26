//! A dedicated server's `changelevel` brings its map in over frames
//! (`chimera_net::dedicated`'s `finish_level_change`; docs/performance.md,
//! "A dedicated server's map change"), on the engine's base with a
//! stand-in game, a map made here (a floor, boxes, a few physics props)
//! and a headless client joined over UDP on loopback: the game plays on
//! while the map loads; then the simulation waits while it spawns (no tick
//! on a part of a map, the whole map in one frame, its colliders in the
//! frames after), a new game starts once it is all in, and the server's map
//! isn't ready until it has settled (`map::LocalMapLoad`), nothing of its
//! match ticking meanwhile, only its physics (`map::MapSettling`). The client is
//! told of the new map once (when it is whole and the new game started),
//! follows, is let in again, and its command clock makes no jump.

use std::time::Duration;

use avian3d::prelude::{Collider, Position, RigidBody};
use bevy::{ecs::message::MessageCursor, prelude::*, time::TimeUpdateStrategy};
use chimera_core::{
    console::ConsoleAppExt,
    core::{LocalPlayer, MapLoads, SimClock},
    game_registry::GameInfo,
    gameplay::{GameplayAppExt, NewGame},
    slots::Loadout,
};
use chimera_net::{
    dedicated::{self, GameServer, Parsed, ServerHooks, ServerMaps, ServerPlugin, StartMap},
    net::{self, NetEvent, NetPlugin, client::Joined, maps::map_name, predict::CommandClock},
};
use chimera_sim::{SimBasePlugins, movement::placeholder};
use chimera_world::{
    greybox::GreyboxMapPlugin,
    harness::Sim,
    map::{
        LoadedMapName, LocalMapLoad, MapBrush, MapCollision, MapComingIn, MapConvex, MapData, MapFile, MapHooks,
        MapModel, MapPart, MapPhysics, MapProp, PropSolid, PushAway, prepare::StepAFrame,
    },
};

const TICK: f64 = 0.02;

static GAME: GameInfo = GameInfo {
    id: "stand_in",
    short_name: "SI",
    name: "SI",
    content_dir: "si",
    tick_interval: TICK,
    open: |_| Err(std::io::ErrorKind::NotFound.into()),
    map_cycle: |_| Vec::new(),
    map_names: |_| Vec::new(),
    unopened: None,
    map_file: |_| None,
    steam: None,
    import_map: |_| Err("no".into()),
    default_binds_cfg: "cfg/default.cfg",
    default_binds: &[],
    old_default_binds: &[],
    movement_cvars: &[],
    meters_per_unit: 0.0254,
    console_namespaces: &[],
};

/// Props in the test map: several of the spawn's steps' worth.
const PROPS: usize = 300;
const PHYSICS_PROPS: usize = 4;

/// A floor with boxes on it and a few physics props above it.
fn map() -> MapData {
    let mut data = MapData {
        name: "test:steps".into(),
        entity_scale: 0.0254,
        ..default()
    };
    let (a, b) = (Vec3::new(-50.0, -1.0, -50.0), Vec3::new(50.0, 0.0, 50.0));
    data.collision_brushes.push(MapBrush::from_box(a, b));
    data.collision_hulls.push(
        (0..8)
            .map(|k| [[a.x, b.x][k & 1], [a.y, b.y][(k >> 1) & 1], [a.z, b.z][(k >> 2) & 1]])
            .collect(),
    );
    data.collision_positions = vec![[a.x, b.y, a.z], [a.x, b.y, b.z], [b.x, b.y, b.z], [b.x, b.y, a.z]];
    data.collision_indices = vec![[0, 1, 2], [0, 2, 3]];
    data.models.push(MapModel {
        bounds: (Vec3::splat(-0.3), Vec3::splat(0.3)),
        ..default()
    });
    let (lo, hi) = (Vec3::new(-0.3, -0.1, -0.2), Vec3::new(0.5, 0.2, 0.2));
    let pick = |lo: f32, hi: f32, bit: usize| if bit == 0 { lo } else { hi };
    let corner = |k: usize| Vec3::new(pick(lo.x, hi.x, k & 1), pick(lo.y, hi.y, k & 2), pick(lo.z, hi.z, k & 4));
    data.models.push(MapModel {
        bounds: (lo, hi),
        collision: Some(MapCollision {
            pieces: vec![MapConvex {
                points: (0..8).map(corner).collect(),
                planes: Vec::new(),
            }],
            mass: 20.0,
            damping: 0.0,
            rotdamping: 0.0,
            inertia: 1.0,
            unit_inertia: None,
            surfaceprop: String::new(),
        }),
        ..default()
    });
    for i in 0..PROPS {
        let physics = i < PHYSICS_PROPS;
        data.props.push(MapProp {
            pose: None,
            ragdoll: None,
            model: usize::from(physics),
            translation: Vec3::new(
                (i % 20) as f32 * 2.0 - 20.0,
                if physics { 1.5 } else { 0.31 },
                (i / 20) as f32 * 2.0 - 15.0 + 20.0,
            ),
            rotation: Quat::IDENTITY,
            solid: if physics { PropSolid::Mesh } else { PropSolid::Box },
            skybox: false,
            lighting: None,
            vertex_light: None,
            casts_shadow: false,
            physics: physics.then(|| MapPhysics {
                mass: 20.0,
                friction: 0.8,
                elasticity: 0.2,
                damping: 0.0,
                rotdamping: 0.0,
                inertia: None,
                push: PushAway::Collide,
                frozen: false,
                asleep: false,
                debris: false,
            }),
            fade: None,
            parent: None,
            entity: None,
            skin: 0,
            body: 0,
            detail: false,
        });
    }
    data.spawns.push((Vec3::new(0.0, 0.1, 0.0), None));
    data
}

static SERVER: GameServer = GameServer {
    info: &GAME,
    title: "Stand-in",
    a_title: "a Stand-in",
    usage: "usage: stand_in_server",
    version: "test",
    sim: |app| {
        app.add_plugins(SimBasePlugins);
    },
    parts: |app| {
        app.insert_resource(Loadout { movement: placeholder::ID })
            // A new game counts its map (as a game's rules do).
            .game_hook(GAME.id, NewGame(|w| w.get_resource_or_init::<MapLoads>().0 += 1));
    },
    load_map: |name, _| Err(format!("no map {name} here")),
    hooks: ServerHooks {
        maps: Some(ServerMaps {
            bare: "test",
            load: |id, _| match id {
                "test:steps" => Ok(map()),
                // The same map once the test lets it come (`GATE`).
                "test:gated" => {
                    while !GATE.load(std::sync::atomic::Ordering::Acquire) {
                        std::thread::yield_now();
                    }
                    Ok(map())
                }
                _ => Err(format!("no map {id}")),
            },
            help: "greybox, test:steps",
        }),
        ..ServerHooks::NONE
    },
};

/// `test:gated` loads once this is set (the start's test: the server's
/// first frames run before its map is in).
static GATE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn frame(sim: &mut Sim) {
    sim.app
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(TICK)));
}

/// The maps the client was told to load.
#[derive(Resource, Default)]
struct Asked(Vec<String>);

/// The client loads what its handshake asks for (`NetEvent::LoadMap`) at
/// once (no world: the handshake needs the map's file only).
fn client_loads(world: &mut World, mut cursor: Local<MessageCursor<NetEvent>>) {
    let asked: Vec<String> = cursor
        .read(world.resource::<Messages<NetEvent>>())
        .filter_map(|e| match e {
            NetEvent::LoadMap { map, .. } => Some(map.clone()),
            _ => None,
        })
        .collect();
    for map in asked {
        world.insert_resource(LoadedMapName(map.clone()));
        world.insert_resource(MapFile {
            name: map_name(&map).to_string(),
            hash: None,
        });
        world.get_resource_or_init::<MapLoads>().0 += 1;
        world.resource_mut::<Asked>().0.push(map);
    }
}

struct Pair {
    server: Sim,
    client: Sim,
}

impl Pair {
    fn new() -> Self {
        let args = ["+ip", "127.0.0.1", "-port", "0", "+maxplayers", "4"].map(String::from);
        let Ok(Parsed::Run(options)) = SERVER.parse(args) else { panic!("options") };
        let start = StartMap::load(&SERVER, &options).expect("the greybox");
        let mut server = Sim::with((), |app| {
            app.add_plugins(ServerPlugin::new(&SERVER, start, &options))
                // A step of the spawn a frame, whatever the machine's speed.
                .insert_resource(StepAFrame);
        });
        frame(&mut server);
        server.app.update();
        let addr = dedicated::listen(server.app.world_mut()).expect("listen");
        let mut client = Sim::with(SimBasePlugins, |app| {
            app.add_plugins((NetPlugin, GreyboxMapPlugin))
                .insert_resource(Loadout { movement: placeholder::ID })
                .init_resource::<Asked>()
                .add_systems(Update, client_loads);
        });
        frame(&mut client);
        let to = std::net::SocketAddr::from(([127, 0, 0, 1], addr.port()));
        net::client::connect(client.app.world_mut(), to).expect("connect");
        Self { server, client }
    }

    fn step(&mut self) {
        self.server.app.update();
        self.client.app.update();
        chimera_world::harness::let_io_run();
    }

    /// Whether the client is in the game on `map`, its player there.
    fn joined(&mut self, map: &str) -> bool {
        let w = self.client.app.world_mut();
        w.contains_resource::<Joined>()
            && net::current_map(w) == map
            && w.query_filtered::<(), With<LocalPlayer>>().iter(w).next().is_some()
            && w.resource::<CommandClock>().tick.is_some()
    }

    fn until(&mut self, what: &str, done: impl Fn(&mut Self) -> bool) {
        for _ in 0..3000 {
            if done(self) {
                return;
            }
            self.step();
        }
        let lines = &self.server.app.world().resource::<chimera_core::console::Console>().output;
        let said: Vec<&str> = lines.iter().rev().take(12).map(|l| l.text.as_str()).collect();
        panic!("never: {what}; the server's console, newest first: {said:#?}");
    }
}

/// New games the server started (`MapLoads`).
fn loads(sim: &Sim) -> u32 {
    sim.app.world().get_resource::<MapLoads>().map_or(0, |l| l.0)
}

fn tick(sim: &Sim) -> u64 {
    sim.app.world().resource::<SimClock>().tick
}

/// The map's parts in the world, and those with a collider.
fn parts(sim: &mut Sim) -> (usize, usize) {
    let w = sim.app.world_mut();
    (
        w.query_filtered::<(), With<MapPart>>().iter(w).count(),
        w.query_filtered::<(), (With<MapPart>, With<Collider>)>().iter(w).count(),
    )
}

#[test]
fn a_changelevel_comes_in_over_frames_and_takes_its_client_along() {
    let mut p = Pair::new();
    p.until("the client joins on the greybox", |p| p.joined("greybox"));
    let before_loads = loads(&p.server);

    dedicated::change_level(p.server.app.world_mut(), "test:steps").expect("changelevel");
    assert!(dedicated::change_level(p.server.app.world_mut(), "test:steps").is_err(), "one load at a time");
    // The game plays on while the map loads.
    let before = tick(&p.server);
    p.until("the map starts to come in", |p| p.server.app.world().contains_resource::<MapComingIn>());
    assert!(tick(&p.server) >= before, "the game played on");
    assert!(p.server.app.world().contains_resource::<LocalMapLoad>(), "the server's map isn't ready");

    // While it comes in: no tick, and the world gets the whole map in one
    // frame, its colliders in the frames after.
    let stood = tick(&p.server);
    let (mut frames, mut map_frame, mut collider_frames) = (0, None, 0);
    let mut had = parts(&mut p.server);
    while p.server.app.world().contains_resource::<MapComingIn>() {
        assert!(dedicated::level_coming_in(p.server.app.world()));
        assert!(p.server.app.world().resource::<Time<Virtual>>().is_paused(), "the simulation waits");
        assert!(p.client.app.world().resource::<Asked>().0.is_empty(), "the client isn't told of part of a map");
        assert_eq!(loads(&p.server), before_loads, "no new game on part of a map");
        p.step();
        frames += 1;
        assert!(frames < 500, "the map never came in");
        let has = parts(&mut p.server);
        if had.0 == 0 && has.0 > 0 {
            assert_eq!(has.1, 0, "the map's frame brings no collider");
            map_frame = Some(frames);
        } else if has.1 > had.1 {
            assert_eq!(has.0, had.0, "the map's parts came in one frame");
            collider_frames += 1;
        }
        had = has;
        if p.server.app.world().contains_resource::<MapComingIn>() {
            assert_eq!(tick(&p.server), stood, "no tick while the map comes in (frame {frames})");
        }
    }
    let map_frame = map_frame.expect("the map came in");
    assert!(map_frame > 2, "the spawn took several frames ({map_frame})");
    assert!(collider_frames >= 1, "the colliders came after the map's frame ({collider_frames})");
    assert!(had.1 > 0, "its colliders are in: {had:?}");

    // All in: a new game on it, the simulation running again.
    let w = p.server.app.world();
    assert_eq!(loads(&p.server), before_loads + 1, "a new game on the map");
    assert!(!w.resource::<Time<Virtual>>().is_paused());
    assert_eq!(w.resource::<LoadedMapName>().0, "test:steps");
    assert_eq!(w.resource::<Time<Fixed>>().timestep(), Duration::from_secs_f64(TICK));

    // It settles with its match held, then the server's map is ready.
    let settled_from = tick(&p.server);
    p.until("the map settles", |p| !dedicated::level_coming_in(p.server.app.world()));
    assert_eq!(tick(&p.server), settled_from, "no tick while it settles");
    assert!(!p.server.app.world().contains_resource::<LocalMapLoad>());

    // The client follows, told once, and is let in again.
    p.until("the client joins on the new map", |p| p.joined("test:steps"));
    assert_eq!(p.client.app.world().resource::<Asked>().0, ["test:steps"], "told of the new map once");
    // Its clock, started on the new map, makes no jump.
    for _ in 0..200 {
        p.step();
    }
    assert!(p.joined("test:steps"));
    assert_eq!(p.client.app.world().resource::<CommandClock>().jumps, 0, "no clock jump after the change");
    let status = net::status(p.server.app.world_mut());
    assert!(status.contains("players : 1 (4 max)"), "{status}");

    // The greybox again, at once, calling nothing off that is left.
    dedicated::change_level(p.server.app.world_mut(), "greybox").expect("changelevel greybox");
    assert_eq!(parts(&mut p.server).0, 0);
    p.until("the client joins on the greybox again", |p| p.joined("greybox"));
}

#[test]
fn the_greybox_calls_a_spawn_under_way_off() {
    let mut p = Pair::new();
    dedicated::change_level(p.server.app.world_mut(), "test:steps").expect("changelevel");
    p.until("the map starts to come in", |p| p.server.app.world().contains_resource::<MapComingIn>());
    dedicated::change_level(p.server.app.world_mut(), "greybox").expect("changelevel greybox");
    let w = p.server.app.world();
    assert!(!dedicated::level_coming_in(w) && !w.contains_resource::<MapComingIn>());
    assert!(!w.contains_resource::<LocalMapLoad>());
    assert!(!w.resource::<Time<Virtual>>().is_paused(), "the simulation runs again");
    let before = tick(&p.server);
    for _ in 0..20 {
        p.step();
    }
    assert!(tick(&p.server) > before);
    assert_eq!(parts(&mut p.server).0, 0, "nothing of the map called off");
    assert_eq!(p.server.app.world().resource::<LoadedMapName>().0, "greybox");
}

#[test]
fn a_changelevel_takes_over_from_a_map_still_settling() {
    let mut p = Pair::new();
    // A settling that is never done (a flood that takes long).
    p.server.app.world_mut().resource_mut::<MapHooks>().settle.push(|_| false);
    dedicated::change_level(p.server.app.world_mut(), "test:steps").expect("changelevel");
    p.until("the map is in, settling", |p| loads(&p.server) == 1);
    for _ in 0..20 {
        p.step();
    }
    let w = p.server.app.world();
    assert!(dedicated::level_coming_in(w) && w.contains_resource::<LocalMapLoad>(), "still settling");
    // The next map loads and comes in all the same.
    dedicated::change_level(p.server.app.world_mut(), "test:steps").expect("changelevel");
    p.until("the map comes in again", |p| loads(&p.server) == 2);
    assert!(parts(&mut p.server).0 > 0);
    assert!(!p.server.app.world().resource::<Time<Virtual>>().is_paused());
}

/// Fixed ticks the match ran (a stand-in for its clock, rounds, respawns
/// and scores: every one of them is a fixed tick's system).
#[derive(Resource, Default)]
struct MatchTicks(u32);

fn count_match_tick(mut ticks: ResMut<MatchTicks>) {
    ticks.0 += 1;
}

/// Frames the settle hook says it is not done yet.
#[derive(Resource)]
struct SettleLeft(u32);

/// The lowest physics prop's height.
fn lowest_prop(sim: &mut Sim) -> f32 {
    let w = sim.app.world_mut();
    w.query::<(&Position, &RigidBody)>()
        .iter(w)
        .filter(|(_, b)| b.is_dynamic())
        .map(|(p, _)| p.0.y)
        .fold(f32::MAX, f32::min)
}

#[test]
fn nothing_of_the_match_ticks_while_a_changelevels_map_settles() {
    let mut p = Pair::new();
    p.server
        .app
        .init_resource::<MatchTicks>()
        .insert_resource(SettleLeft(40))
        .add_systems(FixedUpdate, count_match_tick);
    p.server.app.world_mut().resource_mut::<MapHooks>().settle.push(|w| {
        let mut left = w.resource_mut::<SettleLeft>();
        left.0 = left.0.saturating_sub(1);
        left.0 == 0
    });
    p.until("the client joins on the greybox", |p| p.joined("greybox"));
    dedicated::change_level(p.server.app.world_mut(), "test:steps").expect("changelevel");
    p.until("the map is in, a new match on it", |p| loads(&p.server) == 1);

    // Settling: the fixed loop held, only the physics steps.
    let w = p.server.app.world();
    assert!(w.contains_resource::<chimera_world::map::MapSettling>(), "the match is held");
    assert!(w.contains_resource::<LocalMapLoad>());
    let (stood, ran, clock) = (tick(&p.server), w.resource::<MatchTicks>().0, w.resource::<Time<Fixed>>().elapsed());
    let high = lowest_prop(&mut p.server);
    let mut frames = 0;
    while dedicated::level_coming_in(p.server.app.world()) {
        p.step();
        frames += 1;
        assert!(frames < 500, "the map never settled");
        let w = p.server.app.world();
        assert_eq!(tick(&p.server), stood, "no tick while it settles (frame {frames})");
        assert_eq!(w.resource::<MatchTicks>().0, ran, "no match system runs (frame {frames})");
        assert_eq!(w.resource::<Time<Fixed>>().elapsed(), clock, "the match's clock stands");
    }
    assert!(frames >= 40, "the settle hook ran a slice a frame ({frames})");
    let low = lowest_prop(&mut p.server);
    assert!(low < high - 0.1, "the physics props fell while it settled: {high} -> {low}");
    assert!(!p.server.app.world().contains_resource::<chimera_world::map::MapSettling>());
    assert!(!p.server.app.world().contains_resource::<LocalMapLoad>());

    // Settled: the match plays.
    for _ in 0..10 {
        p.step();
    }
    assert!(tick(&p.server) > stood, "ticks run once it has settled");
    assert!(p.server.app.world().resource::<MatchTicks>().0 > ran);
    p.until("the client joins on the new map", |p| p.joined("test:steps"));
}

/// What a test's `mark` command was given, in order.
#[derive(Resource, Default)]
struct Marks(Vec<String>);

/// A server binary's start (`StartMap::choose_in`, `run`) on `map`, with a
/// `mark` line after the settings.
fn starting_server(map: &str) -> Sim {
    let args = ["+ip", "127.0.0.1", "-port", "0", "+maxplayers", "4", "+map", map, "+mark", "after"].map(String::from);
    let Ok(Parsed::Run(options)) = SERVER.parse(args) else { panic!("options") };
    let start = StartMap::choose_in(&SERVER, &options, Default::default()).expect("a start");
    assert!(matches!(start, StartMap::Later { .. }), "a map of the install comes in as a level change");
    Sim::with((), |app| {
        app.add_plugins(ServerPlugin::new(&SERVER, start, &options))
            .insert_resource(StepAFrame)
            .init_resource::<Marks>();
        app.console_command("mark", "a test's", |w, a| {
            w.resource_mut::<Marks>().0.push(a.join(" "));
            Ok(None)
        });
    })
}

/// A dedicated server listens before its start map is in: the map comes
/// in as a `changelevel` (`dedicated::StartLevel`), the command line's
/// settings run first (`maxplayers`), its other lines once the map has
/// settled, and a client that connects at once is answered and follows the
/// server into the map.
#[test]
fn a_server_listens_before_its_start_map_and_a_client_joining_meanwhile_follows_it_in() {
    let mut server = starting_server("test:gated");
    frame(&mut server);
    let w = server.app.world();
    assert_eq!(w.resource::<chimera_net::net::NetSettings>().maxplayers, 4, "the settings ran");
    assert!(w.resource::<Marks>().0.is_empty(), "the other lines wait for the map");
    assert_eq!(w.resource::<LoadedMapName>().0, "test:gated", "its change is under way");
    assert_eq!(parts(&mut server).0, 0, "no part of the map yet");
    let addr = dedicated::listen(server.app.world_mut()).expect("it listens before its map is in");
    let mut client = Sim::with(SimBasePlugins, |app| {
        app.add_plugins((NetPlugin, GreyboxMapPlugin))
            .insert_resource(Loadout { movement: placeholder::ID })
            .init_resource::<Asked>()
            .add_systems(Update, client_loads);
    });
    frame(&mut client);
    let to = std::net::SocketAddr::from(([127, 0, 0, 1], addr.port()));
    net::client::connect(client.app.world_mut(), to).expect("connect");
    let mut p = Pair { server, client };
    // Answered while the map loads: the client is a player before it is in.
    p.until("the server answers the client", |p| !net::server::players(p.server.app.world_mut()).is_empty());
    assert!(p.server.app.world().contains_resource::<dedicated::StartLevel>(), "still loading its start map");
    GATE.store(true, std::sync::atomic::Ordering::Release);
    // The client may join on the map's name before the server's map is
    // in (it follows the change, `net::maps`); then the server settles.
    p.until("the client joins on the start map, the server's settled", |p| {
        p.joined("test:gated") && !p.server.app.world().contains_resource::<dedicated::StartLevel>()
    });
    // The held lines run on the console's next turn (the frame after).
    p.step();
    let w = p.server.app.world();
    assert_eq!(loads(&p.server), 1, "one new game, on the map");
    assert!(!dedicated::level_coming_in(w), "it settled");
    assert!(!w.contains_resource::<dedicated::StartLevel>());
    assert_eq!(w.resource::<Marks>().0, ["after"], "the held line ran once it had settled");
    assert_eq!(p.client.app.world().resource::<Asked>().0.last().map(String::as_str), Some("test:gated"));
    assert!(p.server.app.should_exit().is_none());
}

/// A start map that doesn't load ends the server with 1, as one loaded
/// before the server did (`run`); its held lines never run.
#[test]
fn a_start_map_that_doesnt_load_ends_the_server_with_1() {
    let mut server = starting_server("test:nope");
    let mut exit = None;
    for _ in 0..500 {
        exit = server.app.should_exit();
        if exit.is_some() {
            break;
        }
        server.app.update();
        chimera_world::harness::let_tasks_run();
    }
    assert_eq!(exit, Some(AppExit::from_code(1)));
    assert!(server.app.world().resource::<Marks>().0.is_empty(), "the held lines never ran");
}
