//! The automatic gun game over the network (`NetSim`, two clients, the
//! party's simulation on the greybox): the clients play the server's
//! definition (its text in the match manifest), and a new ladder
//! (`ugg_new`) leaves out the games a connected player hasn't installed
//! (their mount doctor reports, `minigame_net::installed_by_all`), then
//! takes them back once that player leaves.

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::console::Console;
use chimera_net::{
    harness::NetSim,
    net::{
        manifest::{GameMount, LocalMounts, MountReport},
        memory::LinkConditions,
    },
};
use chimera_sim::minigame::{ActiveMinigame, Minigames, ladder::Ladder};
use chimera_world::greybox::GreyboxMapPlugin;
use combat_arms::{game::TICK_INTERVAL, harness::CaSim};
use party_kit::PartyPartsPlugin;
use ultimate_gun_game::{MINIGAME, UltimateGunGamePlugin};

use crate::parts::{of, rungs};

fn setup(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin, UltimateGunGamePlugin::default()));
}

fn mounts(installed: &[&str]) -> LocalMounts {
    LocalMounts(MountReport {
        games: ["cs_source", "combat_arms"]
            .iter()
            .map(|g| GameMount {
                game: g.to_string(),
                installed: installed.contains(g),
                build: None,
                notes: Vec::new(),
            })
            .collect(),
    })
}

fn ugg_new(sim: &mut NetSim, line: &str) {
    sim.server.app.world_mut().resource_mut::<Console>().submit(line);
    sim.ticks(120);
}

/// Every client plays the server's ladder.
fn assert_clients_play_the_servers(sim: &NetSim, clients: &[usize]) {
    // The server's ladder as played: the definition's guns and the melee
    // finale it drew (the client takes the drawn rungs).
    let server: Vec<String> = sim.server.app.world().resource::<Ladder>().rungs.iter().map(|s| s.to_string()).collect();
    for &i in clients {
        let w = sim.clients[i].app.world();
        assert_eq!(w.resource::<ActiveMinigame>().def.id, MINIGAME, "client {i}");
        let ladder: Vec<String> = w.resource::<Ladder>().rungs.iter().map(|s| s.to_string()).collect();
        assert_eq!(ladder, server, "client {i} plays the server's ladder");
        assert_eq!(
            w.resource::<Ladder>().melee_rungs,
            sim.server.app.world().resource::<Ladder>().melee_rungs,
            "client {i}: the server's melee band"
        );
        assert_eq!(
            w.resource::<Minigames>().text(MINIGAME),
            sim.server.app.world().resource::<Minigames>().text(MINIGAME),
            "client {i}: the server's text"
        );
    }
}

#[test]
fn two_clients_play_the_servers_ladder_made_for_what_they_have() {
    let mut sim = NetSim::new(
        CaSim,
        LinkConditions {
            latency: Duration::from_millis(20),
            jitter: Duration::ZERO,
            loss: 0.0,
        },
        13,
        0,
        setup,
    );
    sim.set_frame(TICK_INTERVAL);
    let full = sim.add_client(|w| w.insert_resource(mounts(&["cs_source", "combat_arms"])));
    let partial = sim.add_client(|w| w.insert_resource(mounts(&["combat_arms"])));
    // On the greybox (no install needed), the ladder of both games.
    chimera_net::minigame_net::change_match(sim.server.app.world_mut(), MINIGAME, Some("greybox")).unwrap();
    sim.until_joined(900);
    sim.ticks(120);
    let both = rungs(sim.server.app.world());
    assert!(of(&both, "cs_source") > 0 && of(&both, "combat_arms") > 0, "{both:?}");
    assert_clients_play_the_servers(&sim, &[full, partial]);
    // A new ladder for the players connected: one lacks CS:S, so Combat
    // Arms' guns only, its maps only.
    let have = chimera_net::minigame_net::installed_by_all(sim.server.app.world_mut(), &["cs_source", "combat_arms", "kit"]);
    assert_eq!(have, ["combat_arms", "kit"], "a stand-in (no install) counts as there");
    ugg_new(&mut sim, "ugg_new 5 12 greybox");
    let ca = rungs(sim.server.app.world());
    let out: Vec<String> =
        sim.server.app.world().resource::<Console>().output.iter().map(|l| l.text.clone()).collect();
    assert!(!ca.is_empty() && of(&ca, "combat_arms") == ca.len(), "{ca:?} {out:?}");
    let def = sim.server.app.world().resource::<Minigames>().get(MINIGAME).unwrap();
    assert_eq!(
        def.maps,
        ["combat_arms:warehouse", "combat_arms:junk_flea", "combat_arms:darkhills", "combat_arms:bloodmoney"],
        "Combat Arms' maps only"
    );
    assert_clients_play_the_servers(&sim, &[full, partial]);
    // That player gone: both games again.
    sim.remove_client(partial);
    sim.ticks(60);
    ugg_new(&mut sim, "ugg_new 5 12 greybox");
    let again = rungs(sim.server.app.world());
    assert!(of(&again, "cs_source") > 0, "{again:?}");
    assert_clients_play_the_servers(&sim, &[full]);
}

/// A server whose players share no game can't build a ladder: told once
/// (not every frame once a match is over), and built when a client with
/// the games arrives.
#[test]
fn a_ladder_no_game_can_make_is_told_once_then_built_when_a_capable_client_joins() {
    use chimera_sim::minigame::NextMatch;
    use chimera_sim::mode::{Match, Phase};
    let mut sim = NetSim::new(
        CaSim,
        LinkConditions {
            latency: Duration::from_millis(20),
            jitter: Duration::ZERO,
            loss: 0.0,
        },
        13,
        0,
        setup,
    );
    sim.set_frame(TICK_INTERVAL);
    // One has CS:S only, the other Combat Arms only: no game in common.
    sim.add_client(|w| w.insert_resource(mounts(&["cs_source"])));
    sim.add_client(|w| w.insert_resource(mounts(&["combat_arms"])));
    chimera_net::minigame_net::change_match(sim.server.app.world_mut(), MINIGAME, Some("greybox")).unwrap();
    sim.until_joined(900);
    sim.ticks(120);
    let told = |sim: &NetSim| {
        sim.server
            .app
            .world()
            .resource::<Console>()
            .output
            .iter()
            .filter(|l| l.text.contains("no game lends guns and movement that every player has"))
            .count()
    };
    let before = told(&sim);
    // The map being played is CS:S's: the CS:S-less player can't play on
    // it, nor on any map of the pool, so the move to the next match fails.
    sim.server.app.world_mut().insert_resource(chimera_world::map::LoadedMapName("cs_source:de_dust2".into()));
    // The match is over and the hold has run out: the rotation moves on.
    {
        let w = sim.server.app.world_mut();
        w.resource_mut::<Match>().phase = Phase::MatchOver;
        w.insert_resource(NextMatch { map: None, at: 0.0 });
    }
    sim.ticks(240);
    let after = told(&sim);
    assert_eq!(after, before + 1, "told once, not every frame");
    // A client with both games joins; the ladder can be made again.
    sim.add_client(|w| w.insert_resource(mounts(&["cs_source", "combat_arms"])));
    sim.remove_client(0);
    sim.remove_client(0);
    sim.ticks(240);
    let w = sim.server.app.world();
    assert_eq!(told(&sim), after, "no further line once a capable client is alone");
    assert!(
        w.resource::<Match>().phase != Phase::MatchOver,
        "the rotation moved on with a ladder for the client's games"
    );
}
