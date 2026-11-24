//! CS 1.6's maps running their entities (`goldsrc_engine::logic`,
//! specs/goldsrc/entities.md's worked examples) on the user's install:
//! cs_office's button opens `door1`, de_nuke's four glass doors slide
//! apart together, cs_militia's rotating door swings away from the player
//! who walks into it, de_survivor's `CLIC` blinker flips its `+a` wall
//! every half second, and de_dust2's crates break when their bomb target
//! fires them (and not when shot). Passes with a note without an install.

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Hitgroup};
use chimera_world::{
    harness::Sim,
    map::{BrushTint, MapBrushEntity, MapData, MapSimPlugin, entities::FireEntityOutput},
};
use cs16::{game::movement, harness::Cs16Sim};
use goldsrc_engine::{
    content::{METERS_PER_UNIT, to_engine},
    logic::GoldSrcLogic,
};

use super::maps::{convert, mount};

/// Map `name` running in a simulation, a few ticks in.
fn sim_of(name: &str) -> Option<(Sim, MapData)> {
    let mount = mount()?;
    let map = convert(&mount, name, false);
    let mut sim = Sim::new(Cs16Sim, MapSimPlugin::new(map.clone()));
    sim.ticks(3);
    assert!(sim.app.world().get_resource::<GoldSrcLogic>().is_some(), "{name}: no logic");
    Some((sim, map))
}

/// The index of the entity with brush model `model`.
fn model(map: &MapData, model: &str) -> usize {
    map.entities.iter().position(|e| e.get("model") == Some(model)).unwrap_or_else(|| panic!("no {model}"))
}

fn named(map: &MapData, name: &str) -> Vec<usize> {
    (0..map.entities.len()).filter(|&i| map.entities[i].get("targetname") == Some(name)).collect()
}

fn logic(sim: &Sim) -> &GoldSrcLogic {
    sim.app.world().resource::<GoldSrcLogic>()
}

/// An entity's logic origin and angles (entity space).
fn pose(sim: &Sim, i: usize) -> (Vec3, Vec3) {
    let e = &logic(sim).world.ents[i];
    (e.origin, e.angles)
}

/// The bounds of an entity's volumes where it is now.
fn bounds(sim: &Sim, i: usize) -> (Vec3, Vec3) {
    logic(sim).world.bounds(i)
}

/// The node of map entity `i`.
fn node(sim: &mut Sim, i: usize) -> Entity {
    let w = sim.app.world_mut();
    let mut q = w.query::<(Entity, &MapBrushEntity)>();
    q.iter(w).find(|(_, n)| n.0 == i).map(|(e, _)| e).unwrap_or_else(|| panic!("no node for {i}"))
}

/// A character whose centre is at `at` (entity space).
fn put(sim: &mut Sim, at: Vec3) -> Entity {
    sim.spawn_character(to_engine(at.to_array()), movement::ID)
}

/// Turn a character to look at `target` from `from` (entity space).
fn look(sim: &mut Sim, p: Entity, from: Vec3, target: Vec3) {
    let d = target - from;
    let mut i = sim.intent(p);
    i.yaw = (d.y.atan2(d.x).to_degrees() - 90.0).to_radians();
    i.pitch = (d.z / d.length()).asin();
}

#[test]
fn cs_office_button_opens_door1() {
    let Some((mut sim, map)) = sim_of("cs_office") else { return };
    let button = model(&map, "*199");
    let door = model(&map, "*200");
    assert_eq!(named(&map, "door1"), vec![door]);
    let (lo, hi) = bounds(&sim, button);
    let centre = (lo + hi) / 2.0;
    let bsp = goldsrc_formats::bsp::Bsp::parse(&mount().unwrap().read("maps/cs_office.bsp").unwrap()).unwrap();
    // A free spot 40 units in front of the button's face (whichever side
    // is open), at its height.
    let spot = [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y]
        .into_iter()
        .map(|d| centre + d * 40.0)
        .find(|p| {
            !goldsrc_formats::bsp::hull::blocks(bsp.contents(0, 1, [p.x as f64, p.y as f64, p.z as f64]))
        })
        .expect("no free spot beside the button");
    let p = put(&mut sim, spot);
    sim.ticks(1);
    let at = goldsrc_engine::content::to_game(sim.position(p));
    let eye = Vec3::from(at) + Vec3::Z * 28.0;
    look(&mut sim, p, eye, centre);
    sim.intent(p).use_key = true;
    sim.ticks(2);
    sim.intent(p).use_key = false;
    assert_eq!(logic(&sim).world.ents[button].frame, 1, "pressed: the +a~sp_lock set shows");
    let n = node(&mut sim, button);
    assert_eq!(sim.app.world().get::<BrushTint>(n).map(|t| t.1), Some(1));
    // 1 s later it fires door1, which rises 108 units in 2.7 s.
    sim.seconds(0.9);
    assert_eq!(pose(&sim, door).0, Vec3::ZERO, "not yet");
    sim.seconds(0.2 + 2.7 + 0.1);
    assert!((pose(&sim, door).0 - Vec3::Z * 108.0).length() < 1e-3, "{:?}", pose(&sim, door));
    let n = node(&mut sim, door);
    let y = sim.app.world().get::<Transform>(n).unwrap().translation.y;
    assert!((y - 108.0 * METERS_PER_UNIT).abs() < 1e-3, "node at {y}");
}

#[test]
fn de_nuke_glass_doors_open_together() {
    let Some((mut sim, map)) = sim_of("de_nuke") else { return };
    let doors = named(&map, "GlassDoor2");
    assert_eq!(doors.len(), 4, "{doors:?}");
    let trigger = model(&map, "*40");
    let (lo, hi) = bounds(&sim, trigger);
    let centre = (lo + hi) / 2.0;
    // The trigger reaches through the closed doors (x 224..256, the doors
    // 236..244): enter it from the −X side, clear of them.
    put(&mut sim, Vec3::new(lo.x - 12.0, centre.y, lo.z + 37.0));
    sim.seconds(0.6);
    let half: Vec<f32> = doors.iter().map(|&d| pose(&sim, d).0.length()).collect();
    assert!(half.iter().all(|&m| (m - half[0]).abs() < 0.01 && m > 10.0 && m < 50.0), "{half:?}");
    sim.seconds(1.0);
    for &d in &doors {
        let moved = pose(&sim, d).0.length();
        assert!((moved - 62.0).abs() < 1e-3, "door {d} moved {moved}");
    }
}

#[test]
fn cs_militia_rotating_door_swings_away_from_the_player() {
    let Some((mut sim, map)) = sim_of("cs_militia") else { return };
    let door = model(&map, "*1");
    let hinge = logic(&sim).world.ents[door].origin;
    assert_eq!(hinge, Vec3::new(175.0, 162.0, -105.0));
    // On the +Y side, heading −Y into the door.
    let p = put(&mut sim, hinge + Vec3::new(30.0, 30.0, 0.0));
    sim.intent(p).yaw = std::f32::consts::PI;
    sim.intent(p).move_axis = Vec2::Y;
    let mut opened = None;
    for k in 0..200 {
        sim.ticks(1);
        if pose(&sim, door).1 != Vec3::ZERO {
            // It opened: stop walking (into it).
            sim.intent(p).move_axis = Vec2::ZERO;
            opened = Some(k);
            break;
        }
    }
    assert!(opened.is_some(), "the door never opened (player at {:?})", sim.position(p));
    sim.seconds(1.0);
    let yaw = pose(&sim, door).1.y;
    assert!((yaw + 90.0).abs() < 1e-3, "turned to {yaw}: should swing to −Y, away from the player");
}

#[test]
fn de_survivor_clic_blinker_toggles_its_wall() {
    let Some((mut sim, map)) = sim_of("de_survivor") else { return };
    let trigger = model(&map, "*53");
    let wall = model(&map, "*54");
    assert_eq!(map.entities[wall].get("targetname"), Some("CLIC"));
    // Inside its first volume (the trigger is several), near its floor.
    let v = &logic(&sim).world.ents[trigger].volumes[0];
    let (lo, hi) = (v.brush.min, v.brush.max);
    let p = put(&mut sim, Vec3::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0, lo.z + 37.0));
    // Until it is entered.
    let mut entered = false;
    for _ in 0..100 {
        sim.ticks(1);
        if logic(&sim).world.ents[wall].frame == 1 {
            entered = true;
            break;
        }
    }
    assert!(entered, "never fired (player at {:?})", sim.position(p));
    sim.app.world_mut().despawn(p);
    let n = node(&mut sim, wall);
    let mut frames = Vec::new();
    sim.seconds(0.25);
    for _ in 0..6 {
        frames.push(sim.app.world().get::<BrushTint>(n).map_or(0, |t| t.1));
        sim.seconds(0.5);
    }
    assert_eq!(frames, vec![1, 0, 1, 0, 1, 0]);
}

#[test]
fn de_dust2_crates_break_when_their_bomb_target_fires() {
    let Some((mut sim, map)) = sim_of("de_dust2") else { return };
    let a = named(&map, "tgt_a");
    let b = named(&map, "tgt_b");
    assert_eq!((a.len(), b.len()), (5, 5));
    let target = model(&map, "*42");
    assert_eq!(map.entities[target].get("target"), Some("tgt_a"));
    // Shot: nothing (Only Trigger).
    let crate_node = node(&mut sim, b[0]);
    sim.app.world_mut().write_message(Damage {
        target: crate_node,
        attacker: None,
        amount: 10.0,
        point: Vec3::ZERO,
        dir: Vec3::X,
        hitgroup: Hitgroup::Generic,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.app.world_mut().write_message(FireEntityOutput { map_index: target, output: "BombExplode".into(), activator: None });
    sim.seconds(0.2);
    for &i in &a {
        let n = node(&mut sim, i);
        assert!(logic(&sim).world.ents[i].removed, "crate {i} not broken");
        assert_eq!(sim.app.world().get::<Visibility>(n), Some(&Visibility::Hidden));
        assert!(sim.app.world().get::<avian3d::prelude::ColliderDisabled>(n).is_some());
    }
    for &i in &b {
        assert!(!logic(&sim).world.ents[i].removed, "crate {i} broke");
    }
}
