//! The map vote on the match-end screen (`chimera_sim::minigame::vote`,
//! `chimera_net::minigame_net`, `chimera_client::client::match_end`), on
//! `NetSim`'s in-memory link: a two-rung ladder with `map_vote` on a pool
//! of three maps, played to its end. When the match is decided the server
//! puts the rotation's pick and the pool's other maps on the ballot and
//! the hold waits for the vote; every client sees the ballot on its end
//! screen; the clients vote (a number key on one, the console's
//! `vote_map` on the other, a vote changed), the server counts them and
//! every client sees the counts and its own choice; at the close the most
//! votes win and the server changes to that map. With no votes the
//! rotation's pick stands.

use bevy::prelude::*;
use chimera_client::client::match_end::{MapVoteChoice, MatchEndPlugin, MatchEndView};
use chimera_core::core::SimClock;
use chimera_net::{harness::NetSim, minigame_net};
use chimera_sim::minigame::{NextMatch, vote::MapVote};

use crate::rotation::{Levels, ROTATING, a_won_match_and};

/// `ROTATING` on three maps with a short vote.
fn voting() -> String {
    ROTATING.replace(
        "maps = [\"chimera:greybox\", \"test:next\"]",
        "maps = [\"chimera:greybox\", \"test:next\", \"test:third\"]\nmap_vote = { candidates = 3, time = \"4s\", reveal = \"1s\" }",
    )
}

fn with_screen(app: &mut App) {
    app.add_plugins(MatchEndPlugin);
    app.init_resource::<ButtonInput<KeyCode>>();
}

/// The match won and the vote open on the server and both clients.
fn vote_open() -> NetSim {
    let (mut sim, _a) = a_won_match_and(&voting(), with_screen);
    let ok = sim.until(2000, |s| {
        s.server.app.world().contains_resource::<MapVote>()
            && s.clients.iter().all(|c| c.app.world().resource::<MatchEndView>().vote.is_some())
    });
    assert!(ok, "the vote opens everywhere");
    sim
}

fn server_vote(sim: &NetSim) -> MapVote {
    sim.server.app.world().resource::<MapVote>().clone()
}

#[test]
fn the_players_vote_for_the_next_map_and_the_most_votes_win() {
    let mut sim = vote_open();
    let v = server_vote(&sim);
    assert_eq!(v.candidates.len(), 3, "{v:?}");
    assert_eq!(v.candidates[0], "test:next", "the rotation's pick first");
    let mut rest = v.candidates[1..].to_vec();
    rest.sort();
    assert_eq!(rest, ["greybox", "test:third"], "the pool's others (a short pool: this map too)");
    let w = sim.server.app.world();
    let next = w.resource::<NextMatch>().clone();
    assert_eq!(next.map, None, "no map until the vote closes");
    assert!(next.at >= v.closes + 1.0 - 1e-6, "the hold waits for the vote and the reveal: {next:?} {v:?}");
    let third = v.candidates.iter().position(|m| m == "test:third").unwrap();
    let greybox = v.candidates.iter().position(|m| m == "greybox").unwrap();
    // Every client shows the ballot, open.
    for i in 0..2 {
        let view = sim.clients[i].app.world().resource::<MatchEndView>().clone();
        let vote = view.vote.clone().unwrap();
        let names: Vec<&str> = v.candidates.iter().map(|m| chimera_sim::minigame::vote::map_name(m)).collect();
        assert_eq!(vote.maps, names, "client {i}");
        assert!(vote.open() && vote.mine.is_none(), "client {i}: {vote:?}");
        assert!(view.next.starts_with("Vote for the next map: 1-3"), "client {i}: {}", view.next);
    }
    // Client 0 votes by key (its number), client 1 at the console, then
    // changes its mind.
    {
        let mut keys = sim.clients[0].app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.press([KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3][third]);
    }
    sim.step();
    {
        let mut keys = sim.clients[0].app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release_all();
        keys.clear();
    }
    assert_eq!(sim.clients[0].app.world().resource::<MapVoteChoice>().0, Some(third));
    minigame_net::vote_map(sim.clients[1].app.world_mut(), greybox).unwrap();
    sim.ticks(10);
    assert_eq!(server_vote(&sim).counts.iter().sum::<u32>(), 2, "both counted");
    minigame_net::vote_map(sim.clients[1].app.world_mut(), third).unwrap();
    sim.clients[1].app.world_mut().resource_mut::<MapVoteChoice>().0 = Some(third);
    let ok = sim.until(300, |s| {
        let want = |c: &[u32]| c[third] == 2 && c[greybox] == 0;
        want(&server_vote(s).counts)
            && s.clients.iter().all(|c| c.app.world().resource::<MatchEndView>().vote.as_ref().is_some_and(|v| want(&v.counts)))
    });
    assert!(ok, "a changed vote counted once, everywhere: {:?}", server_vote(&sim).counts);
    for i in 0..2 {
        let v = sim.clients[i].app.world().resource::<MatchEndView>().vote.clone().unwrap();
        assert_eq!(v.mine, Some(third), "client {i} marks its own choice");
    }
    // The close: the most votes win, everyone sees it, the hold names it.
    let ok = sim.until(1500, |s| s.server.app.world().resource::<MapVote>().winner.is_some());
    assert!(ok, "the vote closes");
    assert_eq!(server_vote(&sim).winning_map(), Some("test:third"));
    assert_eq!(sim.server.app.world().resource::<NextMatch>().map.as_deref(), Some("test:third"));
    assert!(minigame_net::vote_map(sim.clients[0].app.world_mut(), 0).is_err() || {
        sim.ticks(10);
        server_vote(&sim).counts[third] == 2
    }, "no votes after the close");
    let ok = sim.until(300, |s| {
        s.clients.iter().all(|c| {
            let v = c.app.world().resource::<MatchEndView>();
            v.vote.as_ref().is_some_and(|x| x.winner == Some(third)) && v.next.starts_with("Next map: third")
        })
    });
    assert!(ok, "every client shows the winner and counts down to it");
    // The server changes to it after the reveal.
    let ok = sim.until(3000, |s| !s.server.app.world().resource::<Levels>().0.is_empty());
    assert!(ok, "the server moves on");
    assert_eq!(sim.server.app.world().resource::<Levels>().0, ["test:third"]);
    let ok = sim.until(3000, |s| s.clients.iter().all(|c| !c.app.world().contains_resource::<MapVote>()));
    assert!(ok, "the vote goes with the new match");
    for i in 0..2 {
        assert_eq!(sim.clients[i].app.world().resource::<MapVoteChoice>().0, None, "client {i}: its choice reset");
    }
}

#[test]
fn with_no_votes_the_rotations_pick_stands() {
    let mut sim = vote_open();
    let v = server_vote(&sim);
    let now = sim.server.app.world().resource::<SimClock>().now;
    assert!(v.closes > now, "open");
    let ok = sim.until(3000, |s| !s.server.app.world().resource::<Levels>().0.is_empty());
    assert!(ok, "the server moves on");
    assert_eq!(sim.server.app.world().resource::<Levels>().0, ["test:next"], "the rotation's pick");
}

/// A listen server's host (its own player on the server): the console's
/// cheats put it on a rung (`ladder_rung`) and make it win
/// (`minigame_win`: the end screen for screenshots); its vote
/// (`vote_map`, the console's) is cast for its own player, a client's
/// beside it, and a tie of one each is drawn between the two.
#[test]
fn a_listen_server_host_votes_for_its_own_player() {
    use chimera_core::{console::Console, core::LocalPlayer};
    use chimera_net::net::memory::LinkConditions;
    use chimera_sim::minigame::{self, MatchResults, MinigameAppExt, ladder::Rung};
    use chimera_world::greybox::GreyboxMapPlugin;
    use combat_arms::{game::movement, harness::CaSim};
    fn setup(app: &mut App) {
        app.add_plugins((GreyboxMapPlugin, party_kit::PartyPartsPlugin));
        with_screen(app);
    }
    let mut sim = NetSim::new(CaSim, LinkConditions::default(), 23, 1, setup);
    let text = voting().replace("players = { min = 2", "players = { min = 1");
    sim.server.app.add_minigames(&[("voting.toml", &text)]).unwrap();
    minigame::start(sim.server.app.world_mut(), "test:rotating").expect("it starts");
    let host = sim.server.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.server.app.world_mut().entity_mut(host).insert(LocalPlayer);
    sim.until_joined(900);
    sim.ticks(120);
    let console = |sim: &mut NetSim, line: &str| sim.server.app.world_mut().resource_mut::<Console>().submit(line.to_string());
    console(&mut sim, "sv_cheats 1");
    console(&mut sim, "ladder_rung 2");
    sim.ticks(10);
    assert_eq!(sim.server.app.world().get::<Rung>(host).map(|r| r.0), Some(1), "the cheat's rung");
    console(&mut sim, "minigame_win");
    let ok = sim.until(2000, |s| s.server.app.world().contains_resource::<MapVote>());
    assert!(ok, "the match won, the vote opens");
    let results = sim.server.app.world().resource::<MatchResults>().clone();
    assert!(results.rows[0].winner && results.rows[0].player == Some(host), "{results:?}");
    console(&mut sim, "vote_map 3");
    minigame_net::vote_map(sim.clients[0].app.world_mut(), 1).unwrap();
    let ok = sim.until(300, |s| s.server.app.world().resource::<MapVote>().counts == [0, 1, 1]);
    assert!(ok, "the host's vote and the client's: {:?}", server_vote(&sim).counts);
    let ok = sim.until(1500, |s| s.server.app.world().resource::<MapVote>().winner.is_some());
    assert!(ok, "it closes");
    let v = server_vote(&sim);
    assert!(matches!(v.winner, Some(1 | 2)), "a tie drawn between the two: {v:?}");
    let next = sim.server.app.world().resource::<NextMatch>().map.clone();
    assert_eq!(next.as_deref(), v.winning_map());
}
