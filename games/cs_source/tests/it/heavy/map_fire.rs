//! Fire on de_dust2 (specs/source/fire.md test cases): the bomb blows at
//! site A, its `BombExplode` lights the nine `firea` fires 0.2 s later
//! (eight at A, one at B) and leaves `fireb` dark; a player standing in
//! the 512 fire at (864, 2744) takes 26 burn a second once it is full.
//! Skipped without a CS:S install.

use bevy::prelude::*;
use crate::stack::{
    console::Console,
    core::{Health, Team},
    games::{
        self, cs_source,
        cs_source::{
            TICK_INTERVAL,
            movement::{self, SourceMovementPlugin, to_engine},
            objectives::C4,
            weapons::CsWeaponsPlugin,
        },
    },
    harness::{CsSim, Sim},
    logic::Logic,
    map::{MapData, MapPlugin, fire::MapFires},
    mount::config::LocalConfig,
    objectives::bomb::{BombOutcome, BombState},
    weapon::give,
};

const UNIT: f32 = 0.0254;
/// The standing hull's centre above the feet, m (36 units).
const LIFT: f32 = 36.0 * UNIT;

fn load(name: &str) -> Option<MapData> {
    let installed = LocalConfig::load()
        .ok()?
        .game_path_in(cs_source::GAME, cs_source::INFO.steam.as_ref())
        .is_some_and(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    if !installed {
        eprintln!("skipping: no CS:S install configured");
        return None;
    }
    Some(games::load_map_lean(&format!("cs_source:{name}")).expect("load map"))
}

fn character(sim: &mut Sim, feet_units: Vec3, team: u8) -> Entity {
    let e = sim.spawn_character(to_engine(feet_units) + Vec3::Y * (LIFT + 0.05), movement::ID);
    sim.app.world_mut().entity_mut(e).insert(Team(team));
    e
}

/// Lit fires by name, with where they burn (entity space).
fn lit(sim: &Sim, name: &str) -> Vec<Vec3> {
    let logic = sim.app.world().resource::<Logic>();
    logic
        .world
        .fire_looks()
        .into_iter()
        .filter(|f| logic.world.get(f.id).is_some_and(|e| e.targetname.eq_ignore_ascii_case(name)))
        .map(|f| f.at)
        .collect()
}

#[test]
fn dust2_a_bomb_lights_the_a_fires_and_they_burn() {
    let Some(map) = load("de_dust2") else { return };
    let mut sim = Sim::new(CsSim, (MapPlugin::new(map), SourceMovementPlugin, CsWeaponsPlugin));
    sim.set_tick_interval(TICK_INTERVAL);
    sim.ticks(2);
    // Plant at A (as games/cs_source/tests/it/heavy/map_objectives.rs) with a 10 s timer.
    let t = character(&mut sim, Vec3::new(1160.0, 2480.0, 100.0), 1);
    sim.seconds(0.5);
    give(sim.app.world_mut(), t, C4).unwrap();
    sim.seconds(1.2);
    sim.app.world_mut().resource_mut::<Console>().submit("mp_c4timer 10");
    sim.ticks(1);
    sim.intent(t).fire = true;
    sim.seconds(3.1);
    sim.intent(t).fire = false;
    sim.app.world_mut().resource_mut::<Logic>().world.record = true;
    assert!(lit(&sim, "firea").is_empty() && lit(&sim, "fireb").is_empty(), "dark before");
    // Run to the explosion.
    let mut exploded = None;
    for _ in 0..1000 {
        sim.ticks(1);
        let logic = sim.app.world().resource::<Logic>();
        if let Some((tick, ..)) = logic.world.fired.iter().find(|(_, _, o)| o == "BombExplode") {
            exploded = Some(*tick);
            break;
        }
    }
    let exploded = exploded.expect("the bomb blew");
    assert_eq!(
        sim.app.world().resource::<BombState>().outcome,
        Some(BombOutcome::Exploded)
    );
    // StartFire 0.2 s later (14 ticks; the bomb fires its output after
    // this tick's queue, so it is delivered on the 15th tick from here).
    let mut waited = 0;
    while lit(&sim, "firea").is_empty() && waited < 100 {
        sim.ticks(1);
        waited += 1;
    }
    assert_eq!(waited, 15, "ticks from the explosion to the fires");
    let _ = exploded;
    let a = lit(&sim, "firea");
    assert_eq!(a.len(), 9, "nine firea fires");
    assert!(lit(&sim, "fireb").is_empty(), "site B's stay dark");
    assert!(
        a.iter().any(|p| (p.x + 1988.0).abs() < 1.0 && (p.y - 1704.0).abs() < 1.0),
        "one firea burns at B (quirk)"
    );
    // The 512 fire at (864, 2744) landed on the floor at 96.
    let big = a
        .iter()
        .find(|p| (p.x - 864.0).abs() < 1.0 && (p.y - 2744.0).abs() < 1.0)
        .expect("the 512 fire");
    assert!((big.z - 96.0).abs() < 1.0, "on the floor: {big}");
    assert_eq!(sim.app.world().resource::<MapFires>().fires.len(), 9, "drawn");

    // Full after 4.2 s; a CT standing at (1064, 2744) then takes 26 a
    // damage tick, every 1.05 s.
    sim.seconds(4.5);
    let ct = character(&mut sim, Vec3::new(1064.0, 2744.0, 100.0), 2);
    let health = |sim: &Sim| sim.app.world().get::<Health>(ct).unwrap().current;
    let mut drops = Vec::new();
    let mut last = health(&sim);
    for _ in 0..(2.2 / TICK_INTERVAL) as usize {
        sim.ticks(1);
        let h = health(&sim);
        if h < last {
            drops.push(((last - h) * 100.0).round());
        }
        last = h;
    }
    assert_eq!(drops, vec![26.0, 26.0], "burn per damage tick");
}

/// Where started fires land (CS:S, measured 2026-10-18 with
/// `tools/css_probe/firedrop.py`, specs/source/fire.md Q5): StartFire on
/// every env_fire of seven stock maps; each one's origin (x, y, z) and
/// the height it moved to (1/32 above what stopped the drop; one whose
/// origin is in a static prop stays put; one inside a brush drops out
/// of it).
const FIRE_DROPS: &[(&str, &[&str], &[(f32, f32, f32, f32)])] = &[
    (
        "de_dust",
        &["fire_a", "fire_b"],
        &[
            (2057.22, 455.20, 29.60, 4.60),
            (1968.0, 416.0, 120.0, 83.30),
            (1800.0, 464.0, 120.0, 0.14),
            (1960.0, 872.0, 104.0, 41.77),
            (360.0, -1544.0, 96.0, 64.00),
            (-64.0, -1832.0, 176.0, 63.99),
            (432.0, -1768.0, 256.0, 64.00),
            (24.0, -1904.0, 440.0, 288.03),
            (-248.0, -1176.0, 416.0, 384.03),
        ],
    ),
    (
        "de_train",
        &["fire_a", "fire_b"],
        &[(1232.98, -159.91, -104.0, -152.34), (-17.0, -1282.0, -250.0, -288.34)],
    ),
    (
        "de_dust2",
        &["firea", "fireb"],
        &[
            (864.0, 2744.0, 104.0, 96.03),
            (916.0, 2856.0, 304.0, 296.03),
            (644.0, 2944.0, 340.0, 328.03),
            (1148.0, 2456.0, 104.0, 96.03),
            (756.0, 2428.0, 104.0, 96.03),
            (540.0, 2584.0, 104.0, 96.03),
            (332.0, 2740.0, 104.0, 96.03),
            (560.0, 2792.0, 104.0, 96.03),
            (-1828.0, 2644.0, 40.0, 32.03),
            (-1412.0, 2660.0, 24.0, 24.00),
            (-1636.0, 2548.0, 24.0, 24.00),
            // Inside a solid block (z 0-112): CS:S's falls the whole 1024
            // (-992); ours stops at 21.1 on something CS:S's line trace
            // doesn't hit (not found yet; it never burns anyone either
            // way: spec Q9).
            (-1824.0, 1800.0, 32.0, 32.00),
            (-1988.0, 1704.0, 232.0, 224.03),
            (-1572.0, 1812.0, 16.0, 16.00),
            (-2020.0, 3092.0, 80.0, 32.03),
        ],
    ),
    (
        "de_chateau",
        &["bomb_a_flames", "bomb_b_flames"],
        &[
            (16.0, 160.0, 0.0, -7.96),
            (-16.0, 48.0, 0.0, -7.96),
            (272.0, 48.0, 0.0, -7.96),
            (240.0, 152.0, 0.0, -7.96),
            (2360.0, 1560.0, 320.0, 312.03),
            (1872.0, 1560.0, 360.0, 352.07),
            (1960.0, 984.0, 320.0, 312.03),
            (500.0, 16.0, 320.0, 312.06),
            (478.0, 538.0, 318.0, 268.06),
            (304.0, 552.0, 456.0, 450.03),
            (2432.0, 1312.0, 158.0, 154.03),
            (2216.0, 1010.0, 158.0, 154.03),
            (2366.0, 1462.0, 320.0, 312.03),
            (1874.0, 992.0, 376.0, 372.06),
        ],
    ),
    (
        "de_inferno",
        &["bomba_fire", "bombb_fire"],
        &[
            (573.0, 2489.0, 233.0, 160.03),
            (179.0, 3029.0, 240.0, 238.03),
            (203.0, 3132.0, 264.0, 160.03),
            (2119.0, 187.0, 237.0, 160.03),
            (2072.0, 688.0, 188.0, 188.00),
        ],
    ),
    (
        "de_nuke",
        &["bomba_fire"],
        &[(744.0, -816.0, -384.0, -384.00), (555.0, -1062.0, -392.0, -415.96)],
    ),
    (
        "de_tides",
        &["fire_a", "fire_b"],
        &[
            (573.0, -341.0, 39.0, 0.03),
            (-1051.0, -1632.0, -74.0, -103.96),
            (-1040.0, -1184.0, -73.0, -103.96),
        ],
    ),
];

#[test]
fn fires_land_where_css_puts_them() {
    let mut wrong = Vec::new();
    for (name, targets, fires) in FIRE_DROPS {
        let Some(map) = load(name) else { return };
        let mut sim = Sim::new(CsSim, MapPlugin::new(map));
        sim.set_tick_interval(TICK_INTERVAL);
        sim.ticks(2);
        for t in *targets {
            sim.app.world_mut().resource_mut::<Logic>().world.queue_input(
                t,
                "StartFire",
                crate::stack::logic::Value::Void,
                0.0,
                None,
            );
        }
        sim.ticks(2);
        let looks = sim.app.world().resource::<Logic>().world.fire_looks();
        for &(x, y, z, landed) in *fires {
            let found = looks
                .iter()
                .find(|f| (f.at.x - x).abs() < 1.0 && (f.at.y - y).abs() < 1.0)
                .map(|f| f.at.z);
            match found {
                Some(at) if (at - landed).abs() < 0.1 => {}
                other => wrong.push(format!("{name} ({x}, {y}, {z}): CS:S {landed}, ours {other:?}")),
            }
        }
    }
    assert!(wrong.is_empty(), "{} of the fires land elsewhere: {wrong:#?}", wrong.len());
}
