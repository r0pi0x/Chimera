//! CS 1.6's light styles (specs/goldsrc/entities.md "light styles") on a
//! made-up map: a named light's style goes dark and lit again as a relay
//! fires it, a light with a `pattern` key plays it, and a network client
//! gets the server's styles (`net::movers::NetLightStyles`) without
//! running the logic.

use std::time::Duration;

use bevy::prelude::*;
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_world::map::{LightStyles, MapData, MapEntity, MapSimPlugin, style_levels};
use cs16::harness::Cs16Sim;
use goldsrc_engine::logic::{FAMILY, GoldSrcLogic, LogicFire};

fn ent(keys: &[(&str, &str)]) -> MapEntity {
    MapEntity {
        keyvalues: keys.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        ..Default::default()
    }
}

/// Worldspawn, a switchable light `CLIC` (style 32, lit), another `dark`
/// (style 34, initially dark), a flickering one `flick` (style 33, its own
/// pattern) and a relay `relay` that toggles `CLIC`.
fn map() -> MapData {
    let mut m = MapData::default();
    m.entity_scale = 1.0;
    m.entities = vec![
        ent(&[("classname", "worldspawn"), ("chimera_logic", FAMILY)]),
        ent(&[("classname", "light"), ("targetname", "CLIC"), ("style", "32")]),
        ent(&[("classname", "light"), ("targetname", "flick"), ("style", "33"), ("pattern", "mamamz")]),
        ent(&[("classname", "light_spot"), ("targetname", "dark"), ("style", "34"), ("spawnflags", "1")]),
        ent(&[("classname", "trigger_relay"), ("targetname", "relay"), ("target", "CLIC"), ("triggerstate", "2")]),
    ];
    m
}

fn lit(world: &World, style: u8) -> Option<bool> {
    world.get_resource::<LightStyles>()?.styles.iter().find(|s| s.0 == style).map(|s| s.1)
}

fn pattern(world: &World, style: u8) -> Option<Vec<f32>> {
    world.get_resource::<LightStyles>()?.patterns.iter().find(|s| s.0 == style).map(|s| s.1.clone())
}

#[test]
fn a_relay_toggles_a_named_light_and_a_pattern_plays() {
    let mut sim = chimera_world::harness::Sim::new(Cs16Sim, MapSimPlugin::new(map()));
    sim.seconds(0.5);
    let w = sim.app.world();
    assert_eq!(lit(w, 32), Some(true), "a named light starts lit");
    assert_eq!(lit(w, 34), Some(false), "initially dark");
    assert_eq!(lit(w, 33), Some(true));
    assert_eq!(pattern(w, 33), Some(style_levels("mamamz")), "its own pattern plays");
    assert_eq!(pattern(w, 32), None, "a steady light plays none");
    sim.app.world_mut().write_message(LogicFire("relay".into()));
    sim.seconds(0.2);
    assert_eq!(lit(sim.app.world(), 32), Some(false), "toggled dark");
    sim.app.world_mut().write_message(LogicFire("relay".into()));
    sim.seconds(0.2);
    assert_eq!(lit(sim.app.world(), 32), Some(true), "and lit again");
}

#[test]
fn a_client_gets_the_servers_light_styles() {
    let m = map();
    let link = LinkConditions { latency: Duration::from_millis(40), jitter: Duration::ZERO, loss: 0.0 };
    let mut sim = NetSim::new(Cs16Sim, link, 3, 1, move |app| {
        app.add_plugins(MapSimPlugin::new(m.clone()));
    });
    sim.until_joined(900);
    sim.ticks(120);
    assert!(sim.server.app.world().get_resource::<GoldSrcLogic>().is_some());
    let c = sim.clients[0].app.world();
    assert!(c.get_resource::<GoldSrcLogic>().is_none(), "the client runs no logic");
    assert_eq!(lit(c, 32), Some(true));
    assert_eq!(lit(c, 34), Some(false), "the dark light is dark on the client");
    assert_eq!(pattern(c, 33), Some(style_levels("mamamz")), "the client plays the flicker");
    sim.server.app.world_mut().write_message(LogicFire("relay".into()));
    sim.ticks(120);
    assert_eq!(lit(sim.server.app.world(), 32), Some(false));
    assert_eq!(lit(sim.clients[0].app.world(), 32), Some(false), "the client sees the toggle");
    sim.server.app.world_mut().write_message(LogicFire("relay".into()));
    sim.ticks(120);
    assert_eq!(lit(sim.clients[0].app.world(), 32), Some(true), "and the next");
}
