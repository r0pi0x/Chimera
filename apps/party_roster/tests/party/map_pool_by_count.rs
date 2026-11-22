//! The map pool by player count (`chimera_sim::minigame::map_fit`,
//! `chimera_net::minigame_net`), on `NetSim`'s in-memory link: a two-rung
//! ladder rotating at random with a map vote on a pool of four maps, one
//! of them far too big for the players present (its measurements a test
//! game's, registered as a game's data/map_fitness.toml is). The match is
//! played to its end: the ballot never offers the big map and the
//! rotation never picks it; a host's `changelevel` with no map
//! (`minigame_net::host_next_level`) draws the same way; and the console's
//! `map_pool` says why the big map is out.

use bevy::prelude::*;
use chimera_core::console::Console;
use chimera_net::minigame_net;
use chimera_sim::minigame::{map_fit::MapPoolAppExt, vote::MapVote};

use crate::rotation::{Levels, ROTATING, a_won_match_and};

/// Made-up measurements: `test:big` takes minutes to meet anyone at a few
/// players; `test:next` and `test:third` are small (always in).
const FITNESS: &str = r#"
[maps.big]
area_m2 = 14000
sight_m = 2
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

/// `ROTATING` at random over four maps, with the vote.
fn by_count() -> String {
    ROTATING.replace(
        "maps = [\"chimera:greybox\", \"test:next\"]",
        "maps = [\"chimera:greybox\", \"test:next\", \"test:big\", \"test:third\"]\nrotation = \"random\"\n\
         avoid_recent = 1\nmap_vote = { candidates = 3, time = \"2s\", reveal = \"1s\" }",
    )
}

fn with_fitness(app: &mut App) {
    app.add_map_fitness("test", "the test's measurements", FITNESS);
}

#[test]
fn a_map_too_big_for_the_players_present_is_neither_offered_nor_drawn() {
    let (mut sim, _) = a_won_match_and(&by_count(), with_fitness);
    assert!(sim.until(2000, |s| s.server.app.world().contains_resource::<MapVote>()), "the vote opens");
    let vote = sim.server.app.world().resource::<MapVote>().clone();
    assert_eq!(vote.candidates.len(), 3, "{vote:?}");
    assert!(!vote.candidates.iter().any(|m| m == "test:big"), "the big map isn't on the ballot: {vote:?}");
    assert!(sim.until(3000, |s| !s.server.app.world().resource::<Levels>().0.is_empty()), "the server moves on");
    let went = sim.server.app.world().resource::<Levels>().0.clone();
    assert_ne!(went[0], "test:big", "{went:?}");
    // The console says why.
    let start = sim.server.app.world().resource::<Console>().output.len();
    sim.server.app.world_mut().resource_mut::<Console>().submit("map_pool");
    sim.ticks(2);
    let out = &sim.server.app.world().resource::<Console>().output[start..];
    let text = out.iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("\n");
    assert!(text.contains("test:big out:") && text.contains("to meet at"), "{text}");
    assert!(text.contains("greybox") && text.contains("(no measurements)"), "a map without them is in: {text}");
}

#[test]
fn a_host_changelevel_without_a_map_draws_by_the_count() {
    let (mut sim, _) = a_won_match_and(&by_count(), with_fitness);
    for _ in 0..6 {
        // Wait out a map change under way (its own `changelevel`).
        assert!(sim.until(3000, |s| !s.server.app.world().contains_resource::<minigame_net::MapChange>()));
        let before = sim.server.app.world().resource::<Levels>().0.len();
        let said = minigame_net::host_next_level(sim.server.app.world_mut());
        assert!(matches!(said, Some(Ok(Some(_)))), "{said:?}");
        let levels = sim.server.app.world().resource::<Levels>().0.clone();
        if levels.len() > before {
            assert_ne!(levels[levels.len() - 1], "test:big", "{levels:?}");
        }
        sim.ticks(200);
    }
    let levels = sim.server.app.world().resource::<Levels>().0.clone();
    assert!(!levels.is_empty() && !levels.contains(&"test:big".to_string()), "{levels:?}");
}
