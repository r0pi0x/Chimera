//! An arena slice's wall over the network (docs/plans/active/
//! arena-slices.md, A1's leftover; `chimera_world::map::play_area`,
//! `chimera_net::minigame_net`'s `NetMinigame::arena`): the party's server
//! on the greybox playing a ladder of CS:S's and Halo's guns on a slice
//! (x -5..5, z -4..4) and clients that only connect (`NetSim`, 20 ms each
//! way). The client builds the same wall from the match; a client walking
//! and jumping into it as a CS:S (Source movement) and as a Halo character
//! is stopped by its own prediction where the server stops it, with no
//! prediction error (`NetGraph::errors`, the client's corrections); and a
//! client joining mid-match has the wall from the first tick it has a
//! player of its own to move.

use std::time::Duration;

use bevy::prelude::*;
use chimera_net::{
    harness::NetSim,
    net::{memory::LinkConditions, predict::NetGraph},
};
use chimera_sim::minigame::{self, MinigameAppExt, ladder::Ladder};
use chimera_world::{
    greybox::GreyboxMapPlugin,
    map::{
        CharacterModels, MapCharacterModel, MapSimPlugin,
        play_area::{ArenaMap, ArenaPlan, ArenaSlice, ArenaWall, PlayArea},
    },
};
use combat_arms::{game::TICK_INTERVAL, harness::CaSim};
use party_roster::PartyPartsPlugin;

const MAP: &str = "chimera:greybox";
/// CS:S's Glock, then Halo's assault rifle (rung 0 and 1).
const LADDER: [&str; 3] = ["cs_source:weapon_glock", halo_ce::game::weapons::ASSAULT_RIFLE, "cs_source:weapon_deagle"];

fn square() -> ArenaSlice {
    ArenaSlice {
        map: MAP.into(),
        id: "test".into(),
        name: "Greybox: Test".into(),
        points: [[-5.0, -4.0], [5.0, -4.0], [5.0, 4.0], [-5.0, 4.0]].iter().map(|p| Vec2::new(p[0], p[1])).collect(),
        seed: Vec3::new(0.0, 0.1, 0.0),
    }
}

fn bodies(app: &mut App) {
    let body = |game| MapCharacterModel {
        game: Some(game),
        team: None,
        name: None,
        class: None,
        model: default(),
        hitboxes: Vec::new(),
        bones: Vec::new(),
        root: Transform::IDENTITY,
        animations: None,
        boxes: Vec::new(),
        ragdoll: None,
    };
    let games = ["combat_arms", "cs_source", "halo_ce"];
    app.insert_resource(CharacterModels(std::sync::Arc::new(games.iter().map(|g| body(*g)).collect())));
}

/// The map's name in every world (the greybox has no loaded map's name).
fn name_the_map(sim: &mut NetSim) {
    for s in std::iter::once(&mut sim.server).chain(&mut sim.clients) {
        s.app.world_mut().insert_resource(ArenaMap(MAP.into()));
    }
}

/// The party playing `LADDER` with `clients` joined.
fn party(clients: usize) -> NetSim {
    let text = crate::play::ladder_text(&LADDER, false).replace(
        "weapons = [\"cs_source\", \"hl2dm\", \"combat_arms\"]",
        "weapons = [\"cs_source\", \"hl2dm\", \"halo_ce\", \"combat_arms\"]",
    );
    let link = LinkConditions {
        latency: Duration::from_millis(20),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(CaSim, link, 5, clients, |app| {
        app.add_plugins((GreyboxMapPlugin, MapSimPlugin::empty(), PartyPartsPlugin));
        bodies(app);
    });
    crate::play::plain_rungs(sim.server.app.world_mut());
    crate::play::no_finale(sim.server.app.world_mut());
    sim.server.app.add_minigames(&[("arena_net.toml", &text)]).unwrap();
    minigame::start(sim.server.app.world_mut(), "test:gun_game_hl2dm").expect("the gun game starts");
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(900);
    sim.until_looks_taken(600);
    sim.ticks(120);
    sim
}

/// The server plays `square` from now (as the party kit's `arena` on a
/// map just in).
fn play(sim: &mut NetSim) {
    name_the_map(sim);
    sim.server.app.world_mut().insert_resource(ArenaPlan(Some(square())));
    sim.ticks(30);
    assert!(sim.server.app.world().contains_resource::<PlayArea>(), "the server plays the slice");
}

fn walls(world: &mut World) -> usize {
    world.query::<&ArenaWall>().iter(world).count()
}

fn has_the_wall(sim: &mut NetSim, i: usize) -> bool {
    let w = sim.clients[i].app.world_mut();
    w.get_resource::<PlayArea>().is_some_and(|a| a.slice == square()) && walls(w) == 4
}

/// The server's character of client `i` at `at` (x, z) on the floor.
fn place(sim: &mut NetSim, i: usize, at: Vec2) {
    let e = sim.character_of(i).unwrap();
    let p = Vec3::new(at.x, 1.0, at.y);
    let w = sim.server.app.world_mut();
    w.get_mut::<Transform>(e).unwrap().translation = p;
    w.get_mut::<avian3d::prelude::Position>(e).unwrap().0 = p;
}

#[test]
fn a_client_predicts_the_wall_where_the_server_has_it() {
    let mut sim = party(2);
    play(&mut sim);
    assert!(has_the_wall(&mut sim, 0), "client 0 built the wall from the match");
    let poly = sim.server.app.world().resource::<PlayArea>().polygon;
    let ladder = sim.server.app.world().resource::<Ladder>().clone();
    for (rung, what) in [(0usize, "CS:S"), (1, "Halo")] {
        let e = sim.character_of(0).unwrap();
        minigame::ladder::set_rung(sim.server.app.world_mut(), e, rung, &ladder);
        sim.ticks(90);
        // Apart from each other and from the seed's spawn point (where the
        // slice put both back from where they were outside).
        place(&mut sim, 0, Vec2::new(2.5, 1.5));
        place(&mut sim, 1, Vec2::new(-4.0, 3.0));
        sim.ticks(60);
        let put_backs = sim.server.app.world().resource::<PlayArea>().put_backs;
        *sim.clients[0].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
        let me = sim.local_player(0).unwrap();
        // Run and jump into the -z wall, then into the +x, +z corner.
        let mut worst: f32 = 0.0;
        for (yaw, n) in [(0.0f32, 120u32), (-std::f32::consts::FRAC_PI_4 * 3.0, 120)] {
            for t in 0..n {
                {
                    let mut i = sim.clients[0].intent(me);
                    i.yaw = yaw;
                    i.pitch = 0.0;
                    i.move_axis = Vec2::Y;
                    i.jump = t % 30 < 10;
                }
                sim.step();
                let at = sim.server.position(e);
                worst = worst.max(poly.outside_by(at.xz()));
            }
        }
        sim.clients[0].intent(me).move_axis = Vec2::ZERO;
        sim.clients[0].intent(me).jump = false;
        // Landed from the last jump (Halo's is long).
        sim.ticks(120);
        let server = sim.server.position(e);
        let client = sim.clients[0].position(me);
        let g = sim.clients[0].app.world().resource::<NetGraph>().clone();
        println!(
            "{what}: server at {server}, client at {client}; {} states checked, {} errors (worst {:.3} m, {:?}), {} resyncs",
            g.checked, g.errors, g.worst_error, g.last_differing, g.resyncs
        );
        assert_eq!(worst, 0.0, "{what}: the server's character got {worst} m past the wall");
        assert!(poly.edge_distance(server.xz()) < 1.5, "{what}: it reached the wall ({server})");
        assert!(server.distance(client) < 0.05, "{what}: the client predicted {client}, the server has {server}");
        assert!(g.checked > 0, "{what}: compared no state");
        assert_eq!(g.errors, 0, "{what}: mispredicted at the wall (worst {} m, {:?})", g.worst_error, g.last_differing);
        assert_eq!(sim.server.app.world().resource::<PlayArea>().put_backs, put_backs, "{what}: put back");
    }
}

#[test]
fn a_client_joining_mid_match_has_the_wall_before_it_moves() {
    let mut sim = party(1);
    play(&mut sim);
    let late = sim.add_client(|w| {
        w.insert_resource(ArenaMap(MAP.into()));
    });
    let mut moved_without = 0;
    let mut ticks = 0;
    while ticks < 900 {
        sim.step();
        ticks += 1;
        let joined = sim.clients[late].app.world().contains_resource::<chimera_net::net::client::Joined>();
        if joined && sim.local_player(late).is_some() {
            if !has_the_wall(&mut sim, late) {
                moved_without += 1;
            } else {
                break;
            }
        }
    }
    assert!(has_the_wall(&mut sim, late), "the late client never built the wall");
    assert_eq!(moved_without, 0, "the late client had its player {moved_without} ticks before the wall");
}
