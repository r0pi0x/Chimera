//! A hand-written gun game that opts in to a drawn ladder (`ladder =
//! "auto"` in its rules; backlog "A natural power curve for the random
//! ladder"): its guns are drawn at the match's start on the power curve
//! (`chimera_sim::minigame::power`) from both games' guns, warm-up to
//! melee, a pistol first, each band weakest first, another seed another
//! ladder; Lucker Party's own definition keeps its written ladder; over
//! the network the clients play the server's draw.

use bevy::prelude::*;
use chimera_core::game_parts::GunClass;
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_sim::minigame::{
    self, MinigameAppExt, Minigames, auto,
    ladder::{Ladder, RungSeed},
    power::{PowerCurve, PowerReaders},
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{game::TICK_INTERVAL, harness::CaSim};
use party_roster::{DEFAULT_MINIGAME, PartyPartsPlugin};

const DRAWN: &str = "originals:drawn_ladder";

/// Lucker Party's gun game with its ladder drawn: `rungs` in all.
fn drawn_text(app: &App, rungs: usize) -> String {
    let text = app.world().resource::<Minigames>().text(DEFAULT_MINIGAME).unwrap().to_string();
    let start = text.find("ladder = [").expect("a written ladder");
    let end = start + text[start..].find(']').unwrap() + 1;
    format!("{}ladder = \"auto\"\nrungs = {rungs}{}", &text[..start], &text[end..])
        .replace(&format!("id = \"{DEFAULT_MINIGAME}\""), &format!("id = \"{DRAWN}\""))
}

fn add_drawn(app: &mut App, rungs: usize) {
    let text = drawn_text(app, rungs);
    app.add_minigames(&[("drawn.toml", &text)]).unwrap();
}

fn rungs_of(world: &World) -> Vec<&'static str> {
    world.resource::<Ladder>().rungs.clone()
}

#[test]
fn a_definition_opts_in_to_a_drawn_ladder() {
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin));
    sim.app.insert_resource(RungSeed(5));
    add_drawn(&mut sim.app, 20);
    minigame::start(sim.app.world_mut(), DRAWN).expect("the drawn gun game starts");
    let w = sim.app.world();
    let l = w.resource::<Ladder>().clone();
    assert_eq!(l.rungs.len(), 20, "{:?}", l.rungs);
    assert!(l.knife_finale && l.melee_rungs >= 1, "melee last: {:?}", l.rungs);
    let caps = auto::report(w);
    let catalog = w.resource::<chimera_sim::weapon::data::WeaponCatalog>();
    let powers = auto::powers(&caps, catalog, w.resource::<PowerReaders>(), &PowerCurve::shipped());
    let guns = &l.rungs[..l.rungs.len() - l.melee_rungs];
    let class = |id: &str| caps.iter().flat_map(|c| &c.guns).find(|g| g.id == id).map(|g| g.class);
    assert_eq!(class(guns[0]), Some(GunClass::Pistol), "a pistol first: {guns:?}");
    let bands: Vec<_> = guns.iter().map(|id| powers.get(id).expect("a power").band).collect();
    assert!(bands.windows(2).all(|b| b[0] <= b[1]), "the bands climb: {bands:?}");
    for k in 1..guns.len() {
        if bands[k - 1] == bands[k] {
            assert!(powers.get(guns[k - 1]).unwrap().score <= powers.get(guns[k]).unwrap().score, "weakest first: {guns:?}");
        }
    }
    assert!(guns.iter().any(|id| id.starts_with("cs_source:")) && guns.iter().any(|id| id.starts_with("combat_arms:")));
    // A new match, a new draw.
    minigame::start(sim.app.world_mut(), DRAWN).expect("it starts again");
    assert_ne!(rungs_of(sim.app.world()), l.rungs, "another ladder");
    // Lucker Party's own gun game keeps its written ladder.
    minigame::start(sim.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
    let written = sim.app.world().resource::<Minigames>().get(DEFAULT_MINIGAME).unwrap().rules.params["ladder"].clone();
    let written: Vec<&str> = written.as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    let l = sim.app.world().resource::<Ladder>();
    assert_eq!(&l.rungs[..l.rungs.len() - l.melee_rungs], &written[..], "as written");
}

#[test]
fn a_network_client_plays_the_servers_drawn_ladder() {
    let mut sim = NetSim::new(
        CaSim,
        LinkConditions {
            latency: std::time::Duration::from_millis(20),
            jitter: std::time::Duration::ZERO,
            loss: 0.0,
        },
        11,
        2,
        |app| {
            app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin));
            add_drawn(app, 30);
        },
    );
    sim.set_frame(TICK_INTERVAL);
    sim.server.app.insert_resource(RungSeed(9));
    minigame::start(sim.server.app.world_mut(), DRAWN).expect("the drawn gun game starts");
    sim.until_joined(900);
    sim.ticks(300);
    let server = sim.server.app.world().resource::<Ladder>().clone();
    assert_eq!(server.rungs.len(), 30);
    for i in 0..2 {
        let l = sim.clients[i].app.world().resource::<Ladder>();
        assert_eq!(l.rungs, server.rungs, "client {i} plays the server's draw");
        assert_eq!(l.melee_rungs, server.melee_rungs, "client {i}: the server's melee band");
        assert_eq!(l.knife_finale, server.knife_finale);
    }
}
