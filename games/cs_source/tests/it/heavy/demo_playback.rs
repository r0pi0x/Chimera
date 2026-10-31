//! The sample demos (recorded on a CS:S dedicated server with SourceTV;
//! never committed: `$CHIMERA_DEMOS` or the workspace's `target/demos/`)
//! played on de_dust2 from the install (`games::cs_source::demo`):
//! every puppet where the demo has its player at the frames' ticks, as
//! recorded (place, look, team, health, weapon), the bots' recorded
//! shots traced again from their puppets, the deaths told, the flashbang
//! in flight, the planted bomb where it was planted. Skipped without the
//! install or the demos.

use std::sync::Arc;

use bevy::{ecs::message::Messages, prelude::*};

use crate::stack::{
    core::{Dead, Died, Health, Intent, Team},
    games::{
        self,
        cs_source::{
            self, TICK_INTERVAL,
            demo::{self, CsDemo, DemoPuppet, DemoThing, Thing},
            weapons::CsWeaponsPlugin,
        },
    },
    harness::{CsSim, Sim},
    map::{MapPlugin, loose::ShownItem},
    mount::config::LocalConfig,
    objectives::bomb::{BombState, PlantedBomb},
    weapon::{Inventory, Weapon, WeaponEvent, WeaponEventKind, grenade::SmokeCloud},
};

fn demo_file(name: &str) -> Option<CsDemo> {
    let dir = chimera_core::env::var_os("CHIMERA_DEMOS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/demos"));
    let path = dir.join(name);
    if !path.is_file() {
        eprintln!("skipping: no {}", path.display());
        return None;
    }
    let installed = LocalConfig::load()
        .ok()?
        .game_path_in(cs_source::GAME, cs_source::INFO.steam.as_ref())
        .is_some_and(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    if !installed {
        eprintln!("skipping: no CS:S install configured");
        return None;
    }
    Some(demo::load(&path).expect("read the demo"))
}

fn sim_for(demo: &CsDemo) -> Sim {
    let map = games::load_map_lean(&format!("cs_source:{}", demo.map())).expect("load the demo's map");
    let mut sim = Sim::new(CsSim, (MapPlugin::new(map), CsWeaponsPlugin, demo::DemoPlugin));
    sim.set_tick_interval(TICK_INTERVAL);
    sim
}

fn seek(sim: &mut Sim, tick: f64) {
    demo::seek(sim.app.world_mut(), tick);
    sim.app.update();
}

/// Each puppet against its player in the frame at `tick` (a frame's own
/// tick: nothing eased).
fn check_puppets(sim: &mut Sim, demo: &CsDemo, tick: i32) -> usize {
    let t = &demo.timeline;
    let frame = &t.frames[t.frame_at(tick as f64).unwrap()];
    assert_eq!(frame.tick, tick);
    let w = sim.app.world_mut();
    let puppets: Vec<(Entity, u32)> = w
        .query::<(Entity, &DemoPuppet)>()
        .iter(w)
        .map(|(e, p)| (e, p.0))
        .collect();
    let mut checked = 0;
    for (index, thing) in &frame.entities {
        let Thing::Player(p) = thing else { continue };
        let Some(team) = demo::team(p.team) else { continue };
        let (e, _) = *puppets
            .iter()
            .find(|(_, i)| i == index)
            .unwrap_or_else(|| panic!("no puppet for player {index} at {tick}"));
        assert_eq!(w.get::<Team>(e), Some(&team), "player {index}");
        assert_eq!(w.get::<Dead>(e).is_none(), p.alive, "player {index} at {tick}");
        let health = w.get::<Health>(e).unwrap().current;
        assert!(
            (health - p.health.max(0) as f32 / 100.0).abs() < 1e-6,
            "player {index}: {health}"
        );
        if !p.alive {
            continue;
        }
        let at = w.get::<Transform>(e).unwrap().translation;
        let expect = demo::character_origin(p.origin);
        assert!(at.distance(expect) < 1e-4, "player {index} at {tick}: {at} vs {expect}");
        let (yaw, pitch) = demo::look(p.pitch, p.yaw);
        let i = w.get::<Intent>(e).unwrap();
        assert!(
            (i.yaw - yaw).abs() < 1e-4 && (i.pitch - pitch).abs() < 1e-4,
            "player {index} look at {tick}: {} {} vs {yaw} {pitch}",
            i.yaw,
            i.pitch
        );
        // Holding its recorded weapon (our weapon of that class).
        if let Some(class) = p.weapon {
            let name = demo::weapon_name(demo.class(class)).unwrap();
            let held = w
                .get::<Inventory>(e)
                .and_then(|i| i.active)
                .and_then(|a| w.get::<Weapon>(a))
                .map(|w| w.id);
            assert!(
                held.is_some_and(|h| h.ends_with(&name)),
                "player {index} holds {held:?}, recorded {name}"
            );
        }
        checked += 1;
    }
    checked
}

#[test]
fn the_bot_round_plays_on_de_dust2() {
    let Some(demo) = demo_file("mashup_round_bots.dem") else {
        return;
    };
    let mut sim = sim_for(&demo);
    let demo = Arc::new(demo);
    demo::start(sim.app.world_mut(), demo.clone());
    sim.app.update();

    // Puppets where the players were, at frames through both rounds.
    let ticks: Vec<i32> = [200, 1002, 1210, 3000, 5386, 8682]
        .iter()
        .map(|t| demo.timeline.frames[demo.timeline.frame_at(*t as f64).unwrap()].tick)
        .collect();
    for tick in ticks {
        seek(&mut sim, tick as f64);
        let checked = check_puppets(&mut sim, &demo, tick);
        assert!(checked >= 2, "only {checked} living players checked at {tick}");
    }

    // Played on through the first fight (shots from 1002, Grant killed at
    // 1058): rounds traced from their shooters, the death told.
    seek(&mut sim, 990.0);
    let world = sim.app.world();
    let mut shots = world.resource::<Messages<WeaponEvent>>().get_cursor();
    let mut deaths = world.resource::<Messages<Died>>().get_cursor();
    let (mut shot, mut died) = (0, 0);
    let puppets = |w: &mut World| -> Vec<Entity> { w.query_filtered::<Entity, With<DemoPuppet>>().iter(w).collect() };
    for _ in 0..80 {
        sim.app.update();
        let w = sim.app.world_mut();
        let ps = puppets(w);
        shot += shots
            .read(w.resource::<Messages<WeaponEvent>>())
            .filter(|e| ps.contains(&e.owner) && matches!(e.kind, WeaponEventKind::Shot { .. }))
            .count();
        died += deaths.read(w.resource::<Messages<Died>>()).count();
    }
    let recorded = demo
        .timeline
        .frames
        .iter()
        .filter(|f| (991..=1060).contains(&f.tick))
        .flat_map(|f| &f.events)
        .filter(|e| matches!(e, demo::Happening::Shot { .. }))
        .count();
    assert!(recorded > 5, "{recorded}");
    assert!(shot >= recorded, "{shot} traced of {recorded} recorded");
    assert_eq!(died, 1, "Grant's death");

    // The flashbang in flight with its model.
    seek(&mut sim, 700.0);
    let w = sim.app.world_mut();
    let items: Vec<String> = w
        .query_filtered::<&ShownItem, With<DemoThing>>()
        .iter(w)
        .map(|s| s.0.clone())
        .collect();
    assert_eq!(items, ["cs_source:weapon_flashbang"]);

    // The bomb where it was planted (tick 8678), ticking.
    seek(&mut sim, 8700.0);
    let w = sim.app.world_mut();
    let (b, at) = w
        .query_filtered::<(Entity, &Transform), With<PlantedBomb>>()
        .single(w)
        .map(|(e, t)| (e, t.translation))
        .expect("the planted bomb");
    assert!(at.distance(demo::engine([-1480.5625, 2768.0, 4.09375])) < 1e-3, "{at}");
    assert_eq!(w.resource::<BombState>().planted, Some(b));
    // No smoke grenade in this round.
    assert_eq!(w.query::<&SmokeCloud>().iter(w).count(), 0);
}

#[test]
fn the_idle_probe_demo_plays() {
    let Some(demo) = demo_file("mashup_probe_test.dem") else {
        return;
    };
    let mut sim = sim_for(&demo);
    let demo = Arc::new(demo);
    demo::start(sim.app.world_mut(), demo.clone());
    for _ in 0..30 {
        sim.app.update();
    }
    let tick = demo.timeline.frames[demo.timeline.frame_at(200.0).unwrap()].tick;
    seek(&mut sim, tick as f64);
    assert!(check_puppets(&mut sim, &demo, tick) >= 2);
}
