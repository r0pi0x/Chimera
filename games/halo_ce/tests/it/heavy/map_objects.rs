//! Halo's map objects on the disc's maps (`game::maps::teleporters`,
//! `game::maps::collision`'s ladders): every map's teleporter count is the
//! scenario spec's table ("Netgame flags" by type), and on every map that
//! has them a player of Halo's movement set down on each source pad
//! arrives at its pair's target, standing on the ground inside the level
//! (the engine's `map::teleport` on `harness::HaloSim`); the maps with
//! climbable surfaces publish them as ladders (collision.md, "Surface
//! flags": boardingaction's 80); and every map loads with spawns for
//! free-for-all and both teams. Passes with a note without an install.

use bevy::math::Vec3;
use chimera_world::{
    harness::Sim,
    map::{MapSimPlugin, teleport::MapTeleporters},
};
use halo_ce::{
    game::{
        METERS_PER_UNIT, MULTIPLAYER_MAPS,
        movement::{self, MovementData},
    },
    harness::HaloSim,
};

use super::maps::{load, mount};

/// Teleporter sources per map (scenario.md, "Measured facts"), each with
/// its target.
const SOURCES: &[(&str, usize)] = &[
    ("beavercreek", 2),
    ("bloodgulch", 2),
    ("boardingaction", 8),
    ("carousel", 4),
    ("chillout", 3),
    ("damnation", 2),
    ("hangemhigh", 0),
    ("longest", 0),
    ("prisoner", 0),
    ("putput", 30),
    ("ratrace", 4),
    ("sidewinder", 4),
    ("wizard", 4),
];

fn centre_above_feet() -> f32 {
    MovementData::builtin().centre_above_origin(0.0) * METERS_PER_UNIT
}

#[test]
fn every_maps_teleporters_are_the_specs_count() {
    let Some(mount) = mount() else { return };
    assert_eq!(SOURCES.len(), MULTIPLAYER_MAPS.len());
    for (name, sources) in SOURCES {
        let map = load(&mount, name, false);
        let t = map.extras.get::<MapTeleporters>();
        let pads = t.map_or(0, |t| t.pads.len());
        eprintln!("{name}: {pads} teleporters");
        assert_eq!(pads, *sources, "{name}");
        if let Some(t) = t {
            assert!((t.radius - 0.5 * METERS_PER_UNIT).abs() < 1e-4 && (t.rearm - METERS_PER_UNIT).abs() < 1e-4);
            assert!(t.keep_velocity);
        }
    }
}

#[test]
fn a_player_on_each_source_arrives_on_the_ground_at_its_target() {
    let Some(mount) = mount() else { return };
    let mut failures = Vec::new();
    for (name, sources) in SOURCES.iter().filter(|(_, n)| *n > 0) {
        let map = load(&mount, name, false);
        let t = map.extras.get::<MapTeleporters>().cloned().unwrap();
        assert_eq!(t.pads.len(), *sources);
        let (lo, hi) = map
            .collision_positions
            .iter()
            .fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), p| {
                (lo.min(Vec3::from(*p)), hi.max(Vec3::from(*p)))
            });
        let mut sim = Sim::new(HaloSim, MapSimPlugin::new(map));
        sim.ticks(3);
        for (i, pad) in t.pads.iter().enumerate() {
            let p = sim.spawn_character(pad.source + Vec3::Y * (centre_above_feet() + 0.2), movement::ID);
            let mut arrived = false;
            for _ in 0..30 {
                sim.ticks(1);
                if sim.position(p).distance(pad.dest + Vec3::Y * centre_above_feet()) < 1.0 {
                    arrived = true;
                    break;
                }
            }
            sim.seconds(1.5);
            let at = sim.position(p);
            let feet = at - Vec3::Y * centre_above_feet();
            let ground = sim.state(p).on_ground;
            let inside = at.cmpge(lo).all() && at.cmple(hi).all();
            let off = (feet - pad.dest).length();
            eprintln!("{name} pad {i} from {}: arrived {arrived}, feet {feet} ({off:.2} m from the target), on the ground {ground}", pad.source);
            if !(arrived && ground && inside && off < 1.5) {
                failures.push(format!("{name} pad {i}: arrived {arrived}, at {feet}, target {}, ground {ground}, inside {inside}", pad.dest));
            }
            sim.app.world_mut().despawn(p);
            sim.ticks(1);
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn the_climbable_surfaces_are_ladders() {
    let Some(mount) = mount() else { return };
    let mut total = 0;
    for name in MULTIPLAYER_MAPS {
        let map = load(&mount, name, false);
        let ladders = map.collision_brushes.iter().filter(|b| b.ladder).count();
        eprintln!("{name}: {ladders} ladder brushes");
        assert_eq!(ladders, map.collision_brushes.len(), "{name}: ladders only");
        assert!(map.collision_brushes.iter().enumerate().all(|(i, _)| map.trace_skip.contains(&i)), "{name}: shots pass");
        assert!(map.playable.is_some(), "{name}: the played box");
        total += ladders;
        if *name == "boardingaction" {
            // collision.md: its 80 climbable surfaces, twins of 40 sheets.
            assert_eq!(ladders, 40);
        }
        if ["bloodgulch", "hangemhigh"].contains(name) {
            assert_eq!(ladders, 0, "{name}: no flagged surface");
        }
    }
    // collision.md: 120 two-sided climbable surfaces on the multiplayer maps.
    assert_eq!(total, 60);
}

/// All 13 maps load with spawns for free-for-all (every start) and for
/// both capture the flag teams (red `Team(1)`, blue `Team(2)`).
#[test]
fn every_map_has_spawns_for_free_for_all_and_both_teams() {
    let Some(mount) = mount() else { return };
    for name in MULTIPLAYER_MAPS {
        let map = load(&mount, name, false);
        let of = |t: Option<u8>| map.spawns.iter().filter(|(_, team)| team.map(|t| t.0) == t).count();
        let (free, red, blue) = (of(None), of(Some(1)), of(Some(2)));
        eprintln!("{name}: {} spawns: {free} slayer, {red} red, {blue} blue", map.spawns.len());
        assert_eq!(free + red + blue, map.spawns.len(), "{name}");
        assert!(free >= 8 && red > 0 && blue > 0, "{name}");
    }
}
