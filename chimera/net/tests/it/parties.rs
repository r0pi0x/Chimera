//! Parties on a game server (docs/plans/active/parties.md) on `NetSim`:
//! the clients' `cl_party` (`net::party::PartyTag`) reaches the server
//! at the join and on change; the lobby's rows number each party with
//! two or more players there (`prematch::PartyNumbers`), a party of one
//! is 0; a party's members join on its side (`core::party_side`), where
//! the network's pick would split them. The hub's side and following the
//! leader: `hub::parties_*` (the real services).

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::{core::Team, slots::Loadout};
use chimera_net::{
    harness::NetSim,
    net::{memory::LinkConditions, party::PartyTag},
    prematch,
};
use chimera_sim::{SimBasePlugins, movement::placeholder};
use chimera_world::greybox::GreyboxMapPlugin;

fn setup(app: &mut App) {
    app.add_plugins(GreyboxMapPlugin).insert_resource(Loadout { movement: placeholder::ID });
}

fn in_party(id: &'static str) -> impl FnOnce(&mut World) {
    move |w| w.resource_mut::<PartyTag>().0 = id.into()
}

fn sim() -> NetSim {
    let link = LinkConditions { latency: Duration::from_millis(20), jitter: Duration::ZERO, loss: 0.0 };
    NetSim::new(SimBasePlugins, link, 5, 0, setup)
}

fn team_of(sim: &mut NetSim, i: usize) -> u8 {
    let c = sim.character_of(i).expect("a character");
    sim.server.app.world().get::<Team>(c).expect("a team").0
}

/// The server's lobby rows' party numbers, by client index.
fn numbers(sim: &mut NetSim) -> Vec<u8> {
    let rows = prematch::rows(sim.server.app.world_mut());
    (0..sim.clients.len())
        .map(|i| {
            let id = sim.client_id(i);
            rows.iter().find(|r| r.id == Some(id)).map_or(255, |r| r.party)
        })
        .collect()
}

#[test]
fn a_party_plays_on_one_side_and_is_numbered_in_the_lobby() {
    let mut sim = sim();
    // Alone first (the counter-terrorists on a tie), then a party of two
    // (the second would go to the smaller side without its party), then
    // another alone.
    sim.add_client(|_| {});
    sim.until_joined(600);
    sim.add_client(in_party("aaaaaaaaaaaa"));
    sim.until_joined(600);
    sim.add_client(in_party("aaaaaaaaaaaa"));
    sim.until_joined(600);
    sim.add_client(|_| {});
    sim.until_joined(600);
    let teams: Vec<u8> = (0..4).map(|i| team_of(&mut sim, i)).collect();
    assert_eq!(teams, [2, 1, 1, 2], "the party together on side 1");
    // The lobby: the two share a number, the others none.
    let n = numbers(&mut sim);
    assert!(n[1] != 0 && n[1] == n[2] && n[0] == 0 && n[3] == 0, "{n:?}");
    // A third joins the party by its userinfo; another party forms.
    sim.clients[3].app.world_mut().resource_mut::<PartyTag>().0 = "aaaaaaaaaaaa".into();
    sim.clients[0].app.world_mut().resource_mut::<PartyTag>().0 = "bbbbbbbbbbbb".into();
    assert!(sim.until(600, |s| numbers(s)[3] != 0), "the change reached the server");
    let n = numbers(&mut sim);
    assert_eq!((n[3], n[0]), (n[1], 0), "{n:?}: a party of one here is 0");
    // One leaves the party: the number stays while two remain.
    sim.clients[1].app.world_mut().resource_mut::<PartyTag>().0 = String::new();
    assert!(sim.until(600, |s| numbers(s)[1] == 0));
    let n = numbers(&mut sim);
    assert!(n[2] != 0 && n[2] == n[3], "{n:?}");
}
