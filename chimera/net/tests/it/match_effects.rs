//! A match's effects over the network (`minigame_net`'s
//! `NetMinigame::effects`): the server's `MatchEffects` (a host's
//! surprise) reaches each client with its times on the client's own
//! clock (its impact the same seconds after its start), an effect ended
//! early arrives ended (stopped), and one pruned on the server goes on
//! the clients.

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::{core::SimClock, slots::Loadout};
use chimera_net::{harness::NetSim, minigame_net::MinigameNetPlugin, net::memory::LinkConditions};
use chimera_sim::{
    SimBasePlugins,
    minigame::{
        self, MinigameAppExt, MinigamePlugin,
        effects::{Effect, MatchEffects},
    },
    movement::placeholder,
    testkit,
};
use chimera_world::greybox::GreyboxMapPlugin;

const DEF: &str = r#"
id = "kit:gun_game"
name = "Kit gun game"
players = { min = 2, max = 4 }
maps = ["chimera:greybox"]
teams = "free_for_all"
time_limit = "60s"
rotate = false

[spawn]
respawn = { delay = "1s", protection = "0.5s" }
points = "map"

[rules]
module = "chimera:ladder"
ladder = ["kit:pistol", "kit:rifle", "kit:shotgun"]
"#;

fn setup(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, MinigamePlugin, MinigameNetPlugin)).insert_resource(Loadout {
        movement: placeholder::ID,
    });
    testkit::add_weapons(app);
    app.add_minigames(&[("kit_gun_game.toml", DEF)]).expect("the definition reads");
}

fn effect(sim: &NetSim, i: usize, kind: &str) -> Option<Effect> {
    sim.clients[i].app.world().get_resource::<MatchEffects>()?.get(kind).cloned()
}

#[test]
fn match_effects_replicate() {
    let mut sim = NetSim::new(
        SimBasePlugins,
        LinkConditions { latency: Duration::from_millis(30), jitter: Duration::ZERO, loss: 0.0 },
        21,
        2,
        setup,
    );
    minigame::start(sim.server.app.world_mut(), "kit:gun_game").expect("it starts");
    sim.until_joined(900);
    sim.ticks(60);
    let now = sim.server.app.world().resource::<SimClock>().now;
    let mut fx = MatchEffects::default();
    fx.start(Effect {
        kind: "air_strike".into(),
        title: "Air strike".into(),
        threat: true,
        starts: now,
        impact_at: Some(now + 3.0),
        ends: now + 11.0,
        ..default()
    });
    fx.start(Effect { kind: "moon_gravity".into(), title: "Moon gravity".into(), starts: now, ends: now + 60.0, ..default() });
    sim.server.app.world_mut().insert_resource(fx);
    assert!(sim.until(60, |s| (0..2).all(|i| effect(s, i, "air_strike").is_some())), "every client gets them");
    for i in 0..2 {
        let a = effect(&sim, i, "air_strike").unwrap();
        assert!(a.threat && a.title == "Air strike");
        assert!((a.impact_at.unwrap() - a.starts - 3.0).abs() < 0.02, "client {i}: impact 3 s after the start: {a:?}");
        assert!((a.ends - a.starts - 11.0).abs() < 0.02);
        // On the client's clock: started about now there (a tick or two
        // of latency ago).
        let here = sim.clients[i].app.world().resource::<SimClock>().now;
        assert!((here - a.starts).abs() < 0.5, "client {i}: started {} s ago", here - a.starts);
        assert!(effect(&sim, i, "moon_gravity").is_some());
    }
    // Ended early: arrives stopped; pruned: gone.
    let now = sim.server.app.world().resource::<SimClock>().now;
    sim.server.app.world_mut().resource_mut::<MatchEffects>().end("moon_gravity", now);
    assert!(sim.until(60, |s| (0..2).all(|i| effect(s, i, "moon_gravity").is_some_and(|m| m.stopped))), "stopped on the clients");
    sim.server.app.world_mut().resource_mut::<MatchEffects>().prune(now + 2.0);
    assert!(sim.until(60, |s| (0..2).all(|i| effect(s, i, "moon_gravity").is_none())), "gone on the clients");
    assert!(effect(&sim, 0, "air_strike").is_some(), "the other stays");
}
