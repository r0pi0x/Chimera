//! Arena slices in the draw (docs/plans/active/arena-slices.md, A2;
//! `chimera_sim::minigame::map_fit`, `arena`, `chimera_net::minigame_net`)
//! on `NetSim`'s in-memory link: the rotating ladder of `map_pool_by_count`
//! on a pool with a big map that has two measured slices (made-up data:
//! a test game's slices registered as a game's data/arena_slices.toml is).
//! At the two players present the big map is out and its slices in, at 12
//! the map in and its slices out (`map_pool`); the ballot never has the big
//! map nor two of its slices; a host's `changelevel test:big@west` loads
//! the map and plays the slice, its wall built on the server and on both
//! clients (the slice goes with the match), and another slice of the same
//! map loads the map again first; the entry in play is the
//! slice, and the next match's draw holds back the big map with it.

use bevy::prelude::*;
use chimera_core::console::Console;
use chimera_net::minigame_net;
use chimera_sim::minigame::{map_fit::MapPoolAppExt, vote::MapVote};
use chimera_world::map::{
    LoadedMapName,
    play_area::{self, ArenaWall, PlayArea},
};

use crate::rotation::{Levels, ROTATING, a_won_match_and};

/// `test:big` is out at 2 players (96 s to meet) and in at 12 (8.8 s).
const FITNESS: &str = r#"
[maps.big]
area_m2 = 14000
sight_m = 9
spawns = 32
spots = 9000
[maps.next]
area_m2 = 1200
sight_m = 9
spawns = 32
spots = 1500
[maps.third]
area_m2 = 1400
sight_m = 9
spawns = 32
spots = 1500
"#;

/// Two slices of `test:big`, measured and safe: small (always in by
/// size), seen at spawn nowhere, made for 2 to 4 players.
const SLICES: &str = r#"
[[slice]]
map = "big"
id = "west"
name = "West"
points = [[-5.0, -4.0], [5.0, -4.0], [5.0, 4.0], [-5.0, 4.0]]
seed = [0.0, 0.1, 0.0]
players = [2, 4]
area_m2 = 1400
sight_m = 10
spots = 600
see_pairs = 0.1
seen_at_spawn = { 2 = 0.0, 4 = 0.0, 8 = 0.0, 12 = 0.0, 16 = 0.0 }
cover = 0.5
long_sight_p95_m = 30
point_floor_seen = 0.1
point_seen_by = 0.2

[[slice]]
map = "big"
id = "east"
name = "East"
points = [[10.0, -4.0], [20.0, -4.0], [20.0, 4.0], [10.0, 4.0]]
seed = [15.0, 0.1, 0.0]
players = [2, 4]
area_m2 = 1400
sight_m = 10
spots = 600
see_pairs = 0.1
seen_at_spawn = { 2 = 0.0, 4 = 0.0, 8 = 0.0, 12 = 0.0, 16 = 0.0 }
cover = 0.5
long_sight_p95_m = 30
"#;

/// `ROTATING` at random over the four maps (and big's slices), a ballot
/// of four.
fn by_count() -> String {
    ROTATING.replace(
        "maps = [\"chimera:greybox\", \"test:next\"]",
        "maps = [\"chimera:greybox\", \"test:next\", \"test:big\", \"test:third\"]\nrotation = \"random\"\n\
         avoid_recent = 1\nmap_vote = { candidates = 4, time = \"2s\", reveal = \"1s\" }",
    )
}

fn with_slices(app: &mut App) {
    app.add_map_fitness("test", "the test's measurements", FITNESS)
        .add_arena_slices("test", "the test's slices", SLICES)
        // A map's spawn as the game's loader leaves it (the test's loads
        // only name the map): its play area made from the plan.
        .add_systems(Update, play_area::map_spawned.run_if(resource_exists_and_changed::<LoadedMapName>));
}

fn console(sim: &mut chimera_net::harness::NetSim, line: &str) -> String {
    let start = sim.server.app.world().resource::<Console>().output.len();
    sim.server.app.world_mut().resource_mut::<Console>().submit(line);
    sim.ticks(2);
    let out = &sim.server.app.world().resource::<Console>().output[start..];
    out.iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("\n")
}

#[test]
fn a_big_maps_slices_are_drawn_where_it_is_too_big_one_at_a_time() {
    let (mut sim, _) = a_won_match_and(&by_count(), with_slices);
    // The pool by count: at 2 the slices, at 12 the whole map.
    let at2 = console(&mut sim, "map_pool 2");
    assert!(at2.contains("test:big out:") && at2.contains("test:big@west ") && at2.contains("test:big@east "), "{at2}");
    assert!(!at2.contains("test:big@west out"), "{at2}");
    let at12 = console(&mut sim, "map_pool 12");
    assert!(!at12.contains("test:big out") && at12.contains("test:big@west out: test:big is played whole"), "{at12}");
    // The ballot: never the big map, one of its slices at most (here, with
    // four places and three other maps, exactly one).
    assert!(sim.until(2000, |s| s.server.app.world().contains_resource::<MapVote>()), "the vote opens");
    let vote = sim.server.app.world().resource::<MapVote>().clone();
    assert!(!vote.candidates.iter().any(|m| m == "test:big"), "{vote:?}");
    let slices = vote.candidates.iter().filter(|m| m.starts_with("test:big@")).count();
    assert_eq!(slices, 1, "one slice of the big map: {vote:?}");
}

#[test]
fn a_map_change_to_a_slice_builds_its_wall_on_the_host_and_the_clients() {
    let (mut sim, _) = a_won_match_and(&by_count(), with_slices);
    assert!(sim.until(3000, |s| !s.server.app.world().contains_resource::<minigame_net::MapChange>()));
    let said = minigame_net::host_changelevel(sim.server.app.world_mut(), "test:big@west");
    assert!(matches!(said, Some(Ok(Some(_)))), "{said:?}");
    // The map loads by its own name; the match waits for it, on the slice.
    assert!(sim.until(3000, |s| !s.server.app.world().contains_resource::<minigame_net::MapChange>()), "the match starts");
    assert_eq!(sim.server.app.world().resource::<Levels>().0.last().map(String::as_str), Some("test:big"));
    let area = sim.server.app.world().resource::<PlayArea>().slice.key();
    assert_eq!(area, "test:big@west");
    assert_eq!(minigame_net::current_entry(sim.server.app.world()), "test:big@west");
    let walls = |app: &mut App| app.world_mut().query::<&ArenaWall>().iter(app.world()).count();
    assert_eq!(walls(&mut sim.server.app), 4, "the server's wall");
    // The clients build the same wall from the match.
    let ok = sim.until(600, |s| s.clients.iter_mut().all(|c| walls(&mut c.app) == 4));
    assert!(ok, "every client's wall");
    for c in &sim.clients {
        assert_eq!(c.app.world().get_resource::<PlayArea>().map(|a| a.slice.key()).as_deref(), Some("test:big@west"));
    }
    // The next match's draw holds back the big map (whole or a slice).
    for _ in 0..3 {
        let mut probe = sim.server.app.world().resource::<chimera_sim::minigame::rotation::MapHistory>().clone();
        let def = sim.server.app.world().resource::<chimera_sim::minigame::ActiveMinigame>().def;
        let pool = minigame_net::rotation_pool(sim.server.app.world(), def);
        assert!(pool.contains(&"test:big@east".to_string()), "{pool:?}");
        let next = probe.next(def, &pool, "test:big@west", &|_| true).unwrap();
        assert!(!next.starts_with("test:big"), "{next}");
        sim.server.app.world_mut().resource_mut::<chimera_sim::minigame::rotation::MapHistory>().rng.next();
    }
    // Another slice of the same map: the map loads again, the match waits
    // for that load, and plays the other slice.
    let said = minigame_net::host_changelevel(sim.server.app.world_mut(), "test:big@east");
    assert!(matches!(said, Some(Ok(Some(_)))), "{said:?}");
    sim.ticks(5);
    assert!(sim.server.app.world().contains_resource::<minigame_net::MapChange>(), "waits for the map's reload");
    assert!(sim.until(3000, |s| !s.server.app.world().contains_resource::<minigame_net::MapChange>()));
    let levels = sim.server.app.world().resource::<Levels>().0.clone();
    assert_eq!(levels.iter().filter(|l| *l == "test:big").count(), 2, "loaded again: {levels:?}");
    assert_eq!(minigame_net::current_entry(sim.server.app.world()), "test:big@east");
    assert_eq!(walls(&mut sim.server.app), 4, "the other slice's wall only");
    let ok = sim.until(600, |s| {
        s.clients.iter().all(|c| c.app.world().get_resource::<PlayArea>().is_some_and(|a| a.slice.id == "east"))
    });
    assert!(ok, "the clients play the other slice");
    // A whole map again: no slice.
    let said = minigame_net::host_changelevel(sim.server.app.world_mut(), "test:next");
    assert!(matches!(said, Some(Ok(Some(_)))), "{said:?}");
    assert!(sim.until(3000, |s| !s.server.app.world().contains_resource::<minigame_net::MapChange>()));
    assert!(sim.server.app.world().get_resource::<PlayArea>().is_none());
    assert_eq!(minigame_net::current_entry(sim.server.app.world()), "test:next");
}
