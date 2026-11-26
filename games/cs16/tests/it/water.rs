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

// ---- Swimming's steady speeds, slime and lava, drowning (specs/cs16/
// movement.md "Water", water.md), in the greybox's tank with volumes of
// our own heights.

use chimera_core::core::{Health, MapBrush, MapWaterVolume, Velocity};
use chimera_world::map::GameSound;
use cs16::game::movement::{DOWN_ACTION, UP_ACTION};

/// The tank's floor (z 0): a volume to `top` units over it, of `kind`
/// (0 water, 1 slime, 2 lava), `current` along engine space.
fn volume(top: f32, kind: u8, current: Option<Vec3>) -> MapWaterVolume {
    let (lo, hi) = tank_units();
    let a = to_engine(Vec3::new(lo.x, lo.y, -64.0));
    let b = to_engine(Vec3::new(hi.x, hi.y, top));
    MapWaterVolume {
        brush: MapBrush::from_box(a.min(b), a.max(b)),
        slime: kind > 0,
        lava: kind == 2,
        current,
    }
}

#[derive(Resource, Default)]
struct Sounds(Vec<(String, Option<f32>, Option<u8>)>);

fn hear(mut sounds: MessageReader<GameSound>, mut heard: ResMut<Sounds>) {
    heard.0.extend(sounds.read().map(|s| (s.0.entry.clone(), s.0.volume, s.0.channel)));
}

/// A player standing on the tank's floor (feet z 0) in a volume `top`
/// units high.
fn in_tank(top: f32, kind: u8, current: Option<Vec3>) -> (Sim, Entity) {
    let mut sim = Sim::new(Cs16Sim, GreyboxMapPlugin);
    sim.set_tick_interval(cs16::game::TICK_INTERVAL);
    sim.repeatable();
    sim.ticks(5);
    sim.app.init_resource::<Sounds>().add_systems(Last, hear);
    // Our volume in place of the greybox's own tank water, before the
    // player is there.
    sim.app.world_mut().insert_resource(MapWater(vec![volume(top, kind, current)]));
    let (lo, hi) = tank_units();
    let c = (lo + hi) / 2.0;
    let p = sim.spawn_character(to_engine(Vec3::new(c.x, c.y, 0.5 + ORIGIN)), ID);
    sim.ticks(1);
    (sim, p)
}

fn hp(sim: &Sim, p: Entity) -> f32 {
    sim.app.world().get::<Health>(p).unwrap().current * 100.0
}

fn vel(sim: &Sim, p: Entity) -> Vec3 {
    to_units(sim.app.world().get::<Velocity>(p).unwrap().0)
}

/// Lift the player to `z` (feet), still, in the water.
fn lift(sim: &mut Sim, p: Entity, z: f32) {
    let w = sim.app.world_mut();
    let at = w.get::<Transform>(p).unwrap().translation;
    let to = Vec3::new(at.x, (z + ORIGIN) * 0.0254, at.z);
    w.get_mut::<Transform>(p).unwrap().translation = to;
    if let Some(mut pos) = w.get_mut::<avian3d::prelude::Position>(p) {
        pos.0 = to;
    }
    w.get_mut::<Velocity>(p).unwrap().0 = Vec3::ZERO;
}

#[test]
fn the_steady_swim_speeds() {
    // Deep water, the player lifted 60 units over the floor.
    let (mut sim, p) = in_tank(400.0, 0, None);
    lift(&mut sim, p, 150.0);
    sim.ticks(60);
    assert!((vel(&sim, p).z + 48.0).abs() < 0.05, "no input sinks at {}", vel(&sim, p).z);
    // Jump alone rises at 96 (slime 76.8, lava 48).
    for (kind, want) in [(0u8, 96.0f32), (1, 76.8), (2, 48.0)] {
        let (mut sim, p) = in_tank(400.0, kind, None);
        lift(&mut sim, p, 20.0);
        sim.intent(p).jump = true;
        sim.ticks(40);
        assert!((vel(&sim, p).z - want).abs() < 0.05, "kind {kind}: rises at {}", vel(&sim, p).z);
    }
    // The up key: 320 clipped to the 250 gun: 200 up; the down key 200 down.
    let (mut sim, p) = in_tank(400.0, 0, None);
    lift(&mut sim, p, 30.0);
    let bit = sim.app.world().resource::<chimera_core::core::ActionRegistry>().bit(UP_ACTION);
    sim.intent(p).actions = bit;
    sim.ticks(40);
    assert!((vel(&sim, p).z - 200.0).abs() < 0.05, "up key {}", vel(&sim, p).z);
    let bit = sim.app.world().resource::<chimera_core::core::ActionRegistry>().bit(DOWN_ACTION);
    sim.intent(p).actions = bit;
    sim.ticks(1);
    let (mut sim2, p2) = in_tank(400.0, 0, None);
    lift(&mut sim2, p2, 160.0);
    sim2.intent(p2).actions = sim2.app.world().resource::<chimera_core::core::ActionRegistry>().bit(DOWN_ACTION);
    sim2.ticks(50);
    assert!((vel(&sim2, p2).z + 200.0).abs() < 0.05, "down key {}", vel(&sim2, p2).z);
}

#[test]
fn forward_with_jump_settles_at_174_4() {
    let (mut sim, p) = in_tank(400.0, 0, None);
    lift(&mut sim, p, 40.0);
    sim.intent(p).move_axis = Vec2::Y;
    sim.intent(p).jump = true;
    sim.ticks(30);
    let v = vel(&sim, p);
    assert!((v.truncate().length() - 174.4).abs() < 0.5 && (v.z - 96.0).abs() < 0.05, "{v:?}");
}

#[test]
fn slime_and_lava_hurt_on_every_command() {
    // Slime to the knees (level 1: 4 a command), then to the eyes (level
    // 3: 12 a command); lava level 1 kills in 10 commands, level 3 in 4.
    let (mut sim, p) = in_tank(5.0, 1, None);
    sim.ticks(1);
    let before = hp(&sim, p);
    sim.ticks(1);
    assert!((before - hp(&sim, p) - 4.0).abs() < 0.01, "slime level 1: {} -> {}", before, hp(&sim, p));
    let (mut sim, p) = in_tank(5.0, 2, None);
    sim.ticks(14);
    assert!(hp(&sim, p) <= 0.0, "lava level 1 kills within 14 commands ({})", hp(&sim, p));
    let (mut sim, p) = in_tank(90.0, 2, None);
    sim.ticks(8);
    assert!(hp(&sim, p) <= 0.0, "lava level 3 within 8 commands ({})", hp(&sim, p));
    let (mut sim, p) = in_tank(90.0, 1, None);
    sim.ticks(14);
    assert!(hp(&sim, p) <= 0.0, "slime level 3 (12 a command) within 14 commands ({})", hp(&sim, p));
}

#[test]
fn drowning_starts_at_12_s_and_comes_back_10_every_2_s() {
    let (mut sim, p) = in_tank(90.0, 0, None);
    assert_eq!(hp(&sim, p), 100.0);
    sim.ticks(1190);
    assert_eq!(hp(&sim, p), 100.0, "11.9 s under: nothing");
    sim.ticks(20);
    assert!((hp(&sim, p) - 97.0).abs() < 0.01, "the first hit, 3: {}", hp(&sim, p));
    sim.ticks(100);
    assert!((hp(&sim, p) - 93.0).abs() < 0.01, "then 4: {}", hp(&sim, p));
    sim.ticks(100);
    assert!((hp(&sim, p) - 88.0).abs() < 0.01, "then 5: {}", hp(&sim, p));
    // Surface: the air gone, the gasp is pl_wade1; 12 owed comes back as 10
    // and 2 two seconds on.
    sim.app.world_mut().insert_resource(MapWater(vec![volume(5.0, 0, None)]));
    sim.ticks(3);
    let h = hp(&sim, p);
    assert!((h - 98.0).abs() < 0.01, "+10 at once: {h}");
    assert!(
        sim.app.world().resource::<Sounds>().0.iter().any(|(e, v, c)| e.contains("pl_wade1") && *v == Some(1.0) && *c == Some(2)),
        "the gasp"
    );
    sim.ticks(100);
    assert!((hp(&sim, p) - 98.0).abs() < 0.01, "not before 2 s");
    sim.ticks(120);
    assert!((hp(&sim, p) - 100.0).abs() < 0.01, "+2: {}", hp(&sim, p));
}

#[test]
fn a_short_dive_is_silent_a_longer_one_gasps_softer() {
    let gasps = |secs: u64| {
        let (mut sim, p) = in_tank(90.0, 0, None);
        sim.ticks(secs * 100);
        sim.app.world_mut().insert_resource(MapWater(vec![volume(5.0, 0, None)]));
        sim.ticks(5);
        let _ = p;
        sim.app
            .world()
            .resource::<Sounds>()
            .0
            .iter()
            .filter(|(e, ..)| e.contains("pl_wade"))
            .map(|(e, ..)| e.clone())
            .collect::<Vec<_>>()
    };
    assert!(gasps(2).is_empty(), "2 s under: no gasp");
    assert_eq!(gasps(5), ["player/pl_wade2.wav"], "5 s under");
}

#[test]
fn a_current_pushes_by_50_a_water_level() {
    // Level 1 on the floor in a current along +x: the first command moves
    // by 50 x 0.01 and the stored velocity ends 50 below (movement.md
    // "Currents").
    let dir = to_engine(Vec3::X).normalize();
    let (mut sim, p) = in_tank(5.0, 0, None);
    sim.ticks(30);
    sim.app.world_mut().insert_resource(MapWater(vec![volume(5.0, 0, Some(dir))]));
    let x0 = to_units(sim.position(p)).x;
    sim.ticks(1);
    let moved = to_units(sim.position(p)).x - x0;
    assert!((moved - 0.5).abs() < 0.05, "moved {moved}");
    assert!((vel(&sim, p).x + 50.0).abs() < 1.0, "velocity {}", vel(&sim, p).x);
}

#[test]
fn the_drawn_eye_keeps_4_units_from_the_surface() {
    // The surface 51 units over the feet: the eye (53) out of it by 2 is
    // drawn 4 over; with the surface 55 up it is under by 2, drawn 4 under.
    for (top, want) in [(51.0f32, 55.0f32), (55.0, 51.0)] {
        let (mut sim, p) = in_tank(top, 0, None);
        lift(&mut sim, p, 0.5);
        sim.ticks(1);
        let m = sim.app.world().get::<QuakeMovement>(p).unwrap().clone();
        assert!(m.water_level >= 2, "level {}", m.water_level);
        let ms = sim.app.world().get::<chimera_core::core::MovementState>(p).unwrap().clone();
        let drawn = (ms.eye_offset.y + ms.view_shift) / 0.0254 + ORIGIN;
        assert!((drawn - want).abs() <= 1.1, "surface {top}: drawn eye {drawn}, want {want}");
        // Shots and traces start at the real eye (53 over the feet).
        let real = ms.eye_offset.y / 0.0254 + ORIGIN;
        assert!((real - 53.0).abs() <= 0.1, "surface {top}: shot origin {real}");
        let view = chimera_world::map::interp::EyeView::of(&Default::default(), &ms);
        assert!((view.eye_offset - ms.eye_offset).length() < 1e-6, "the aim ray's eye is the real one");
        assert!((view.camera_offset().y - ms.eye_offset.y - ms.view_shift).abs() < 1e-6, "the camera's is shifted");
    }
}
