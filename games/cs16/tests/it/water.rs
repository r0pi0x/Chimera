//! A GoldSrc map's water leaf, converted (`goldsrc_engine::content::
//! water`, specs/goldsrc/bsp.md "Contents") and swum in by CS 1.6's
//! movement (specs/cs16/movement.md, "Water"): a hull 0 tree built in
//! code whose water leaf is the greybox's tank stands in for the tank's
//! own volume; under it to the eye the player is at water level 3, sinks
//! at no more than 60 x 0.8 = 48 units/s with no input (no gravity) and
//! rises at 100 with jump held; out of it, level 0.

use bevy::prelude::*;
use chimera_core::core::MapWater;
use chimera_movement::{QuakeMovement, to_engine, to_units};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
    map::MapData,
};
use cs16::{game::movement::ID, harness::Cs16Sim};
use goldsrc_formats::bsp::{
    Bsp, Child, Leaf, Model, Node, Plane,
    hull::{CONTENTS_EMPTY, CONTENTS_SOLID, CONTENTS_WATER},
};

/// The standing box's centre over its feet, units.
const ORIGIN: f32 = 36.0;

/// The tank's inside in units (x, y, z up): its floor at 0, its water's
/// surface at the greybox's depth.
fn tank_units() -> (Vec3, Vec3) {
    let a = to_units(Vec3::new(greybox::TANK_MIN.x, 0.0, greybox::TANK_MIN.y));
    let b = to_units(Vec3::new(
        greybox::TANK_MAX.x,
        greybox::TANK_WATER_DEPTH,
        greybox::TANK_MAX.y,
    ));
    (a.min(b), a.max(b))
}

/// A world whose hull 0 is one plane at the tank's surface, water below
/// and empty above, in bounds that cut it to the tank (the cut grows them
/// by 64 units).
fn tank_bsp() -> Bsp {
    let (lo, hi) = tank_units();
    let leaf = |contents| Leaf {
        contents,
        vis_offset: -1,
        mins: [0; 3],
        maxs: [0; 3],
        first_mark_surface: 0,
        mark_surface_count: 0,
        ambient: [0; 4],
    };
    Bsp {
        planes: vec![Plane {
            normal: [0.0, 0.0, 1.0],
            dist: hi.z,
            kind: 2,
        }],
        nodes: vec![Node {
            plane: 0,
            children: [Child::Leaf(1), Child::Leaf(2)],
            mins: [0; 3],
            maxs: [0; 3],
            first_face: 0,
            face_count: 0,
        }],
        leaves: vec![leaf(CONTENTS_SOLID), leaf(CONTENTS_EMPTY), leaf(CONTENTS_WATER)],
        models: vec![Model {
            mins: (lo + Vec3::splat(64.0)).to_array(),
            maxs: (hi - Vec3::new(64.0, 64.0, 0.0)).to_array(),
            origin: [0.0; 3],
            heads: [0; 4],
            vis_leaves: 0,
            first_face: 0,
            face_count: 0,
        }],
        ..Bsp::default()
    }
}

#[test]
fn a_converted_water_leaf_is_swum_in() {
    let mut data = MapData::default();
    assert_eq!(goldsrc_engine::content::water::add_world(&tank_bsp(), &mut data), 1);
    let mut sim = Sim::new(Cs16Sim, GreyboxMapPlugin);
    sim.set_tick_interval(cs16::game::TICK_INTERVAL);
    sim.ticks(5);
    // The converted volume in place of the tank's own.
    sim.app.world_mut().insert_resource(MapWater(data.water));
    let (lo, hi) = tank_units();
    let c = (lo + hi) / 2.0;
    let feet = Vec3::new(c.x, c.y, hi.z - 80.0);
    let p = sim.spawn_character(to_engine(feet + Vec3::Z * ORIGIN), ID);
    sim.ticks(1);
    let level = |sim: &Sim| sim.app.world().get::<QuakeMovement>(p).unwrap().water_level;
    assert_eq!(level(&sim), 3, "under water to the eye");
    // 40 commands: at full sink speed (accelerate 5 x 48 x 0.01 = 2.4 a
    // command), above the tank's floor 30 units down.
    sim.ticks(40);
    let vz = to_units(sim.velocity(p)).z;
    assert!(level(&sim) == 3 && (-48.5..-40.0).contains(&vz), "sinking at {vz}");
    sim.intent(p).jump = true;
    let z0 = to_units(sim.position(p)).z;
    sim.ticks(20);
    let (vz, z1) = (to_units(sim.velocity(p)).z, to_units(sim.position(p)).z);
    assert!(vz > 50.0 && z1 > z0 + 10.0, "rising at {vz}, {z0} -> {z1}");
    // Out of the water: dry.
    let p2 = sim.spawn_character(greybox::SPAWNS[0], ID);
    sim.ticks(2);
    assert_eq!(sim.app.world().get::<QuakeMovement>(p2).unwrap().water_level, 0);
}
