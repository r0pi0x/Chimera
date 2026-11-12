//! Ladders across the games' movements (the user's call, 2026-11-12;
//! docs/DESIGN.md "Ladders across games"): every map's ladders are
//! published in one form (`core::MapLadders`: the map's ladder brushes and
//! HL2's ladder lines; a game's own moving volumes as `core::LadderZone`),
//! and each movement climbs any of them by its own rules. Headless on the
//! party's greybox (nobody's map), each character on its own movement:
//! HL2:DM's climbs the greybox's ladder brush and a Combat Arms ladder
//! volume as lines (`useable_ladders::lines_of_volume`); CS:S's and
//! Combat Arms' climb an HL2 ladder line (a `func_useableladder` of the
//! map's entities: a thin ladder post, a ladder volume around it). On a
//! map of HL2:DM's own, its movement leaves the ladder brushes solid, as
//! HL2:DM does. CS:S on a Combat Arms volume and Combat Arms on a brush:
//! cross_water_ladders.rs and combat_arms' movement_volumes.rs.

use bevy::prelude::*;
use chimera_core::core::MapGame;
use chimera_world::{
    greybox::{GreyboxMapPlugin, LADDER_X, NORTH_WALL_Z},
    harness::Sim,
    map::{MapEntities, MapEntity},
};
use combat_arms::{game::volumes::CaVolume, harness::CaSim};
use cs_source::game::movement::SourceMovement;
use party_kit::PartyPartsPlugin;

const CSS: &str = cs_source::game::movement::ID;
const CA: &str = combat_arms::game::movement::ID;
const HL2: &str = hl2dm::game::movement::ID;

/// The party's simulation on the greybox, no match: each character keeps
/// the movement it is given.
fn sim() -> Sim {
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin));
    sim.ticks(2);
    sim
}

/// A character of `movement` standing at `at` (engine, near the floor).
fn stand(sim: &mut Sim, movement: &'static str, at: Vec3) -> Entity {
    let p = sim.spawn_character(at, movement);
    sim.seconds(1.0);
    p
}

/// `p` walking forward looking `yaw` degrees (0: -Z, north) and `pitch`
/// up, for `secs`: whether a Source movement was on a ladder at some tick,
/// and the most it rose (m).
fn climb(sim: &mut Sim, p: Entity, yaw: f32, pitch: f32, secs: f64) -> (bool, f32) {
    let start = sim.position(p).y;
    let (mut on, mut rose) = (false, 0.0f32);
    for _ in 0..(secs * sim.tick_hz()) as u32 {
        let mut i = sim.intent(p);
        i.yaw = yaw.to_radians();
        i.pitch = pitch.to_radians();
        i.move_axis = Vec2::Y;
        sim.ticks(1);
        if let Some(m) = sim.app.world().get::<SourceMovement>(p) {
            on |= m.ladder.is_some() || m.useable.is_some();
        }
        rose = rose.max(sim.position(p).y - start);
    }
    (on, rose)
}

/// An HL2 ladder line on the greybox (a `func_useableladder` among the
/// map's entities), from `bottom` up `height` m (engine).
fn hl2_ladder(sim: &mut Sim, bottom: Vec3, height: f32) {
    let src = |v: Vec3| {
        let s = Vec3::new(v.x, -v.z, v.y) / 0.0254;
        format!("{} {} {}", s.x, s.y, s.z)
    };
    let ladder = MapEntity {
        keyvalues: vec![
            ("classname".to_string(), "func_useableladder".to_string()),
            ("origin".to_string(), src(bottom)),
            ("point0".to_string(), src(bottom)),
            ("point1".to_string(), src(bottom + Vec3::Y * height)),
        ],
        ..Default::default()
    };
    sim.app.world_mut().insert_resource(MapEntities { entities: vec![ladder].into(), scale: 0.0254 });
    sim.ticks(2);
}

/// An HL2 ladder line up the north wall, 18 units out from it as HL2:DM's
/// maps place them in front of their ladders' geometry (the wall), away
/// from the wall's ladder brush; and a spot in front of it.
const LINE_FOOT: Vec3 = Vec3::new(LADDER_X + 5.0, 0.0, NORTH_WALL_Z + 18.0 * 0.0254);
const BEFORE_LINE: Vec3 = Vec3::new(LADDER_X + 5.0, 1.0, NORTH_WALL_Z + 1.6);

/// In front of the north wall's ladder brush, facing it.
const BEFORE_BRUSH: Vec3 = Vec3::new(LADDER_X, 1.0, NORTH_WALL_Z + 1.4);

#[test]
fn hl2dm_movement_climbs_a_source_maps_ladder_brush() {
    let mut sim = sim();
    let p = stand(&mut sim, HL2, BEFORE_BRUSH);
    let (on, rose) = climb(&mut sim, p, 0.0, 45.0, 3.0);
    assert!(on, "on the ladder brush's line");
    assert!(rose > 3.0, "climbed {rose} m");
}

#[test]
fn hl2dm_movement_climbs_a_combat_arms_ladder_volume() {
    let mut sim = sim();
    let p = stand(&mut sim, HL2, Vec3::new(5.0, 1.0, 0.0));
    // A Combat Arms ladder north of the player, its face 1 m ahead, 10 m tall.
    let ladder = CaVolume::class_box("Ladder", Vec3::new(4.0, 0.0, -1.5), Vec3::new(6.0, 10.0, -1.0)).unwrap();
    sim.app.world_mut().spawn(ladder);
    sim.ticks(2);
    let (on, rose) = climb(&mut sim, p, 0.0, 45.0, 3.0);
    assert!(on, "on the volume's line");
    assert!(rose > 3.0, "climbed {rose} m");
}

#[test]
fn cs_source_movement_climbs_an_hl2_ladder_line() {
    let mut sim = sim();
    let p = stand(&mut sim, CSS, BEFORE_LINE);
    hl2_ladder(&mut sim, LINE_FOOT, 6.0);
    let (on, rose) = climb(&mut sim, p, 0.0, 45.0, 4.0);
    assert!(on, "on the line's ladder post");
    assert!(rose > 4.0, "climbed {rose} m");
}

#[test]
fn combat_arms_movement_climbs_an_hl2_ladder_line() {
    let mut sim = sim();
    let p = stand(&mut sim, CA, BEFORE_LINE);
    hl2_ladder(&mut sim, LINE_FOOT, 6.0);
    let (_, rose) = climb(&mut sim, p, 0.0, 45.0, 4.0);
    assert!(rose > 4.0, "climbed {rose} m");
}

#[test]
fn hl2dm_movement_on_its_own_map_leaves_ladder_brushes_solid() {
    let mut sim = sim();
    sim.app.world_mut().insert_resource(MapGame("hl2dm".into()));
    let p = stand(&mut sim, HL2, BEFORE_BRUSH);
    let (on, rose) = climb(&mut sim, p, 0.0, 45.0, 2.0);
    assert!(!on && rose < 0.5, "HL2:DM climbs only useable ladders on its maps: on {on}, rose {rose}");
}
