//! Water and ladders across the games' movements: a player holding a CS:S
//! gun on a Combat Arms map swims in its liquid volumes and climbs its
//! ladder volumes (Combat Arms hands them to Chimera as `WaterZone`s and
//! `LadderZone`s); the other way, a Combat Arms gun on a Source map swims
//! in the map's water (`combat_arms` `volumes`, `BrushWater`). Headless on
//! the party's greybox.

use bevy::prelude::*;
use chimera_core::core::{MapBrush, MapWater, MapWaterVolume};
use chimera_sim::testkit;
use chimera_world::harness::Sim;
use combat_arms::game::volumes::CaVolume;
use cs_source::game::movement::SourceMovement;

use crate::cross_game::holding;
use crate::gun_game::{party, place};

/// A player with CS:S's AK-47 (rung 6) standing at the greybox's middle.
fn css_player(sim: &mut Sim) -> Entity {
    let a = holding(sim, 6, "cs_source:weapon_ak47");
    // CS:S's origin is the box's middle, 0.9 m above the feet.
    place(sim, a, Vec3::new(0.0, 1.0, 0.0));
    sim.seconds(0.5);
    a
}

/// A liquid volume of Combat Arms' (m): `Water` from `lo` to `hi`.
fn ca_water(sim: &mut Sim, lo: Vec3, hi: Vec3) {
    sim.app.world_mut().spawn(CaVolume::liquid_box(lo, hi, 0.0, Vec3::ZERO));
}

fn level(sim: &Sim, e: Entity) -> u8 {
    sim.app.world().get::<SourceMovement>(e).expect("CS:S movement").water_level
}

#[test]
fn cs_source_movement_swims_in_a_combat_arms_water_volume() {
    let mut sim = party();
    let a = css_player(&mut sim);
    assert_eq!(level(&sim, a), 0, "dry on the plain map");
    // A pool around the player, 6 m deep above the floor.
    ca_water(&mut sim, Vec3::new(-20.0, -1.0, -20.0), Vec3::new(20.0, 6.0, 20.0));
    sim.seconds(0.5);
    assert!(level(&sim, a) >= 1, "in the water at level {}", level(&sim, a));
    // Out of the water again: the volume hidden, the level drops.
    let w = sim.app.world_mut();
    let mut q = w.query::<&mut CaVolume>();
    for mut v in q.iter_mut(w) {
        v.hidden = true;
    }
    sim.seconds(0.5);
    assert_eq!(level(&sim, a), 0, "a hidden volume is no water");
}

#[test]
fn cs_source_movement_climbs_a_combat_arms_ladder_volume() {
    let mut sim = party();
    let a = css_player(&mut sim);
    let before = sim.position(a).y;
    // A ladder in front of the player (-Z), its face 0.5 m ahead, 10 m tall.
    let ladder = CaVolume::class_box("Ladder", Vec3::new(-2.0, 0.0, -1.0), Vec3::new(2.0, 10.0, -0.5)).unwrap();
    sim.app.world_mut().spawn(ladder);
    sim.ticks(2);
    let at = sim.position(a) + Vec3::new(0.0, 6.0, -0.6);
    testkit::aim_at(&mut sim, a, at);
    sim.intent(a).move_axis = Vec2::Y;
    let mut climbing = false;
    for _ in 0..(3.0 * sim.tick_hz()) as u32 {
        sim.ticks(1);
        climbing |= sim.app.world().get::<SourceMovement>(a).unwrap().ladder.is_some();
    }
    assert!(climbing, "grabbed the ladder");
    assert!(sim.position(a).y > before + 1.5, "climbed from {before} to {}", sim.position(a).y);
}

#[test]
fn a_combat_arms_gun_swims_in_a_source_maps_water() {
    let mut sim = party();
    let a = holding(&mut sim, 7, "combat_arms:m4a1");
    let brush = MapBrush::from_box(Vec3::new(-20.0, -1.0, -20.0), Vec3::new(20.0, 6.0, 20.0));
    sim.app.world_mut().insert_resource(MapWater(vec![MapWaterVolume { brush, slime: false }]));
    place(&mut sim, a, Vec3::new(0.0, 5.0, 0.0));
    sim.seconds(0.4);
    // Swimming sinks slowly: a free fall of 5 m takes ~0.6 s.
    assert!(sim.position(a).y > 1.0, "floating down at {}", sim.position(a).y);
}
