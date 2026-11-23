//! A spectator's camera over the network (`chimera_net::spectators`,
//! `client::spectate`) on `NetSim`, the greybox and the test kit's guns:
//! a client joining with `cl_spectator 1` watches from the start (no
//! death cam), never spawns, and cycles through the living players with
//! `spec_next` and `spec_prev` (attack and attack2) in first person, as
//! its own copies of them, while they play.

use std::time::Duration;

use bevy::prelude::*;
use chimera_client::client::spectate::{SpecMode, SpecPhase, SpectateStatePlugin, Spectator};
use chimera_core::{
    console::Console,
    core::{Dead, Spectating},
    slots::Loadout,
};
use chimera_net::{harness::NetSim, minigame_net::MinigameNetPlugin, net::memory::LinkConditions, spectators::JoinAsSpectator};
use chimera_sim::{
    SimBasePlugins,
    minigame::{self, MinigameAppExt, MinigamePlugin},
    movement::placeholder,
    testkit,
};
use chimera_world::greybox::GreyboxMapPlugin;

const DEF: &str = r#"
id = "kit:watch_cam"
name = "Kit gun game"
players = { min = 1, max = 8 }
maps = ["chimera:greybox"]
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

const HZ: u64 = 64;

fn setup(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, MinigamePlugin, MinigameNetPlugin, SpectateStatePlugin)).insert_resource(Loadout {
        movement: placeholder::ID,
    });
    testkit::add_weapons(app);
    app.add_minigames(&[("kit_watch_cam.toml", DEF)]).expect("the definition reads");
}

#[test]
fn a_spectator_watches_the_players_and_cycles_through_them() {
    let mut sim = NetSim::new(
        SimBasePlugins,
        LinkConditions { latency: Duration::from_millis(20), jitter: Duration::ZERO, loss: 0.0 },
        13,
        2,
        setup,
    );
    minigame::start(sim.server.app.world_mut(), "kit:watch_cam").expect("it starts");
    sim.until_joined(900);
    let spec = sim.add_client(|w| w.resource_mut::<JoinAsSpectator>().0 = 1);
    sim.until_joined(900);
    sim.ticks(3 * HZ);
    // Its player watches and never spawns; the two players play.
    let me = sim.local_player(spec).unwrap();
    let w = sim.clients[spec].app.world();
    assert!(w.get::<Spectating>(me).is_some() && w.get::<Dead>(me).is_some(), "watching");
    let ids = [sim.client_id(0), sim.client_id(1)];
    let theirs: Vec<Entity> = ids.iter().map(|id| NetSim::owned_by(&mut sim.clients[spec], Some(*id)).expect("seen")).collect();
    {
        let s = sim.clients[spec].app.world().resource::<Spectator>();
        assert_eq!(s.phase, SpecPhase::Watching, "no death cam for a spectator");
        for e in &theirs {
            assert!(s.watchable.contains(e), "{e} may be watched: {:?}", s.watchable);
        }
        assert!(!s.watchable.contains(&me), "not itself");
    }
    // First person, then next and previous through the players.
    let say = |sim: &mut NetSim, line: &str| {
        sim.clients[spec].app.world_mut().resource_mut::<Console>().submit(line);
        sim.ticks(4);
    };
    say(&mut sim, "spec_mode 4");
    say(&mut sim, "spec_next");
    let first = sim.clients[spec].app.world().resource::<Spectator>().target.expect("a target");
    assert!(theirs.contains(&first));
    assert_eq!(sim.clients[spec].app.world().resource::<Spectator>().mode, SpecMode::InEye);
    say(&mut sim, "spec_next");
    let second = sim.clients[spec].app.world().resource::<Spectator>().target.expect("a target");
    assert!(theirs.contains(&second) && second != first, "the other player");
    say(&mut sim, "spec_prev");
    assert_eq!(sim.clients[spec].app.world().resource::<Spectator>().target, Some(first), "back");
    // Still out of play on the server.
    let e = sim.character_of(spec).unwrap();
    assert!(sim.server.app.world().get::<Dead>(e).is_some());
}
