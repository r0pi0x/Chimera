//! Others' radio calls under `party_voice_lines_3d` (the player's own
//! setting, in their userinfo: `map::sound::PartyVoices`): the server
//! sends a living caller's call to its team as always and also to each
//! other player who asked, within a talking voice's reach; not to one who
//! didn't, nor anyone's call once the caller is dead.

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::{
    console::Console,
    core::{Health, Radio, Team},
    slots::Loadout,
};
use chimera_net::{
    harness::NetSim,
    net::{RadioCall, memory::LinkConditions},
};
use chimera_sim::{SimBasePlugins, movement::placeholder};
use chimera_world::{
    greybox::GreyboxMapPlugin,
    map::sound::{PartyVoices, TALKING, send_range},
};

#[derive(Resource, Default)]
struct Calls(usize);

fn setup(app: &mut App) {
    app.add_plugins(GreyboxMapPlugin)
        .insert_resource(Loadout {
            movement: placeholder::ID,
        })
        .init_resource::<Calls>()
        .add_systems(Update, |mut r: MessageReader<RadioCall>, mut n: ResMut<Calls>| n.0 += r.read().count());
}

fn counts(sim: &mut NetSim) -> Vec<usize> {
    sim.clients.iter_mut().map(|c| std::mem::take(&mut c.app.world_mut().resource_mut::<Calls>().0)).collect()
}

#[test]
fn a_call_reaches_others_who_asked_within_reach() {
    let link = LinkConditions {
        latency: Duration::from_millis(20),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(SimBasePlugins, link, 21, 3, setup);
    sim.until_joined(900);
    for (i, on) in [(0, 1), (1, 0), (2, 1)] {
        let line = format!("party_voice_lines_3d {on}");
        sim.clients[i].app.world_mut().resource_mut::<Console>().submit(&line);
    }
    sim.ticks(120);
    let players: Vec<Entity> = (0..3).map(|i| sim.character_of(i).unwrap()).collect();
    for (i, p) in players.iter().enumerate() {
        sim.server.app.world_mut().entity_mut(*p).insert(Team(10 + i as u8));
    }
    let asked: Vec<bool> = players.iter().map(|p| PartyVoices::wants(sim.server.app.world(), *p)).collect();
    assert_eq!(asked, [true, false, true], "the server has each player's choice");
    let reach = send_range(TALKING).unwrap();
    assert!((reach - 76.2).abs() < 0.01, "{reach}");
    for p in &players[1..] {
        assert!(sim.server.position(*p).distance(sim.server.position(players[0])) < reach);
    }
    sim.ticks(5);
    counts(&mut sim);
    let call = |sim: &mut NetSim| {
        sim.server.app.world_mut().write_message(Radio {
            sender: players[0],
            command: "roger".into(),
        });
        sim.ticks(20);
    };
    call(&mut sim);
    assert_eq!(counts(&mut sim), [1, 0, 1], "the caller (its team), not one who didn't ask, one who did");

    // Out of reach: none for the one who asked.
    let at = sim.server.position(players[0]);
    sim.server.app.world_mut().get_mut::<Transform>(players[2]).unwrap().translation = at + Vec3::X * (reach + 5.0);
    sim.server.app.world_mut().write_message(Radio {
        sender: players[0],
        command: "roger".into(),
    });
    sim.ticks(20);
    assert_eq!(counts(&mut sim)[2], 0, "beyond a talking voice's reach");
}

#[test]
fn a_dead_callers_call_reaches_no_one_else() {
    let link = LinkConditions {
        latency: Duration::from_millis(20),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(SimBasePlugins, link, 22, 2, setup);
    sim.until_joined(900);
    for i in 0..2 {
        sim.clients[i].app.world_mut().resource_mut::<Console>().submit("party_voice_lines_3d 1");
    }
    sim.ticks(120);
    let players: Vec<Entity> = (0..2).map(|i| sim.character_of(i).unwrap()).collect();
    for (i, p) in players.iter().enumerate() {
        sim.server.app.world_mut().entity_mut(*p).insert(Team(10 + i as u8));
    }
    sim.server.app.world_mut().get_mut::<Health>(players[0]).unwrap().current = 0.0;
    sim.ticks(5);
    counts(&mut sim);
    sim.server.app.world_mut().write_message(Radio {
        sender: players[0],
        command: "roger".into(),
    });
    sim.ticks(20);
    assert_eq!(counts(&mut sim), [0, 0]);
}
