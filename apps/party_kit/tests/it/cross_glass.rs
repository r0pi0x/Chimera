//! Glass any game's character breaks by moving into it (playtest
//! 2026-10-28: a Combat Arms character couldn't break cs_office's
//! windows), on a small Source map in the party (`source_logic`
//! `breakables`, specs/source/breakables.md "Touch"): a cracked window
//! (a bullet broke it as an entity, its other panes standing) lets both
//! games' characters through and shatters the panes their box touches
//! (its panes' collider is for shots only, `core::TRACE_ONLY_LAYER`), and
//! a break-on-touch glass func_breakable breaks when either runs into it.
//! A Combat Arms round breaks both and goes on through (their nodes'
//! surface is glass, `map::BrushSurface`, which Combat Arms passes:
//! weapon_firing.md "Penetration").

use bevy::prelude::*;
use chimera_core::core::DamageKind;
use chimera_world::{
    harness::Sim,
    map::{MapBrush, MapData, MapEntity, MapHull, MapMesh, MapSimPlugin},
};
use combat_arms::{game::movement as ca_movement, harness::CaSim};
use cs_source::game::movement::{self as css_movement, to_engine, to_source};
use party_kit::PartyPartsPlugin;
use source_logic::logic::{EntId, Logic, classes::Class};

const SCALE: f32 = 0.0254;

fn hull(lo: Vec3, hi: Vec3) -> MapHull {
    let b = MapBrush::from_box(lo, hi);
    let points = (0..8)
        .map(|i| {
            Vec3::new(
                if i & 1 == 0 { lo.x } else { hi.x },
                if i & 2 == 0 { lo.y } else { hi.y },
                if i & 4 == 0 { lo.z } else { hi.z },
            )
        })
        .collect();
    MapHull {
        planes: b.planes,
        points,
    }
}

fn entity(pairs: &[(&str, &str)], hulls: Vec<MapHull>) -> MapEntity {
    MapEntity {
        keyvalues: pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        hulls,
        mover: true,
        physics: None,
    }
}

/// A floor at z = 0 (Source units); a 120 x 96 window of 10 x 8 glass
/// panes (entity 0) across x -60..60 at y = 200, standing on the floor;
/// a 64 x 64 break-on-touch glass func_breakable (entity 1) at x = -300,
/// y = 200, from 16 to 80 units up.
fn glass_map() -> MapData {
    let (a, b) = (
        to_engine(Vec3::new(-2048.0, -2048.0, -64.0)),
        to_engine(Vec3::new(2048.0, 2048.0, 0.0)),
    );
    let (a, b) = (a.min(b), a.max(b));
    MapData {
        name: "test:cross_glass".into(),
        // The floor for movement that sweeps brushes (CS:S's) and as a
        // collider (Combat Arms').
        collision_brushes: vec![MapBrush::from_box(a, b)],
        collision_hulls: vec![
            (0..8)
                .map(|k| [[a.x, b.x][k & 1], [a.y, b.y][(k >> 1) & 1], [a.z, b.z][(k >> 2) & 1]])
                .collect(),
        ],
        entities: vec![
            entity(
                &[
                    ("classname", "func_breakable_surf"),
                    ("health", "1"),
                    ("fragility", "100"),
                    ("surfacetype", "0"),
                    ("lowerleft", "60 200 0"),
                    ("lowerright", "-60 200 0"),
                    ("upperleft", "60 200 96"),
                    ("upperright", "-60 200 96"),
                ],
                vec![hull(Vec3::new(-60.0, 199.0, 0.0), Vec3::new(60.0, 201.0, 96.0))],
            ),
            entity(
                &[
                    ("classname", "func_breakable"),
                    ("spawnflags", "2"),
                    ("material", "0"),
                    ("health", "1"),
                    ("origin", "-300 200 48"),
                ],
                vec![hull(Vec3::new(-32.0, -2.0, -32.0), Vec3::new(32.0, 2.0, 32.0))],
            ),
        ],
        // Their drawn faces: glass (what a round finds there,
        // `map::BrushSurface`).
        meshes: (0..2).map(glass_face).collect(),
        entity_scale: SCALE,
        spawns: vec![(to_engine(Vec3::new(0.0, -300.0, 1.0)), None)],
        ..default()
    }
}

/// A drawn glass face of brush entity `entity` (one triangle; where it
/// is doesn't matter here).
fn glass_face(entity: usize) -> MapMesh {
    MapMesh {
        material: "glass/glasswindow001a".into(),
        entity: Some(entity),
        surface: Some("glass".into()),
        positions: vec![[0.0, 0.0, 0.0], [0.1, 0.0, 0.0], [0.0, 0.1, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        uvs: vec![[0.0, 0.0]; 3],
        indices: vec![0, 1, 2],
        ..default()
    }
}

fn party_on_glass() -> Sim {
    let mut sim = Sim::new(CaSim, (MapSimPlugin::new(glass_map()), PartyPartsPlugin));
    sim.ticks(2);
    sim
}

/// The logic entity of map entity `index`.
fn logic_id(sim: &Sim, index: usize) -> EntId {
    let logic = sim.app.world().resource::<Logic>();
    logic
        .world
        .ids()
        .into_iter()
        .find(|id| logic.world.get(*id).unwrap().map_index == Some(index))
        .expect("the breakable's logic entity")
}

/// The window's broken panes once it is broken as an entity.
fn broken_panes(sim: &Sim) -> Option<usize> {
    let id = logic_id(sim, 0);
    let logic = sim.app.world().resource::<Logic>();
    logic
        .world
        .window(id)
        .filter(|w| w.window_broken)
        .map(|w| w.broken_count())
}

/// A character of `movement` standing at Source (x, y) on the floor,
/// looking along Source +y (yaw 0, engine -Z).
fn runner(sim: &mut Sim, movement: &'static str, x: f32, y: f32) -> Entity {
    let p = sim.spawn_character(to_engine(Vec3::new(x, y, 0.0)) + Vec3::Y * 0.95, movement);
    sim.seconds(0.5);
    let mut i = sim.intent(p);
    i.yaw = 0.0;
    i.pitch = 0.0;
    p
}

#[test]
fn both_games_characters_run_through_a_cracked_window_shattering_it() {
    for movement in [css_movement::ID, ca_movement::ID] {
        let mut sim = party_on_glass();
        let p = runner(&mut sim, movement, 0.0, 100.0);
        // A bullet in the window's top corner pane cracks it.
        let id = logic_id(&sim, 0);
        sim.app.world_mut().resource_mut::<Logic>().world.damage(
            id,
            1.0,
            DamageKind::Bullet,
            None,
            Vec3::new(55.0, 200.0, 90.0),
            Vec3::Y,
        );
        sim.ticks(2);
        let cracked = broken_panes(&sim).expect("the bullet cracks the window");
        sim.intent(p).move_axis = Vec2::Y;
        sim.seconds(1.5);
        let y = to_source(sim.position(p)).y;
        let after = broken_panes(&sim).unwrap();
        assert!(y > 230.0, "{movement}: ran through the cracked window: at y {y}");
        assert!(
            after >= cracked + 4,
            "{movement}: shattered panes running through: {cracked} -> {after}"
        );
    }
}

/// Combat Arms' character keeps its speed into what stops it, so it breaks
/// the glass; CS:S's movement clips its velocity at the glass, and the
/// spec's "smoothed speed" (breakables.md "Touch flags") isn't defined
/// yet, so CS:S's doesn't (docs/tech-debt.md, `source_logic` `breakables`).
#[test]
fn a_combat_arms_character_breaks_touch_glass_running_into_it() {
    let mut sim = party_on_glass();
    assert!(!touch_glass_broken(&sim), "whole at the start");
    let p = runner(&mut sim, ca_movement::ID, -300.0, 100.0);
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(1.5);
    assert!(
        touch_glass_broken(&sim),
        "broke the touch glass, at {}",
        to_source(sim.position(p))
    );
}

/// Whether the touch glass (map entity 1) is broken: gone (deleted at
/// once, as in CS:S) or marked broken.
fn touch_glass_broken(sim: &Sim) -> bool {
    let logic = sim.app.world().resource::<Logic>();
    logic
        .world
        .ids()
        .into_iter()
        .filter_map(|id| logic.world.get(id))
        .find(|e| e.map_index == Some(1))
        .is_none_or(|e| matches!(&e.class, Class::Breakable(b) if b.broken))
}

// ---------------------------------------------------------------------------
// Combat Arms' rounds through CS:S's glass (specs/combat_arms/
// weapon_firing.md "Penetration": Glass passes, damage x 0.9).

/// A Combat Arms soldier at Source (x, y) holding `gun`, drawn.
fn gunner(sim: &mut Sim, gun: &str, x: f32, y: f32) -> Entity {
    let p = runner(sim, ca_movement::ID, x, y);
    chimera_sim::weapon::give(sim.app.world_mut(), p, gun).expect("the gun");
    sim.seconds(1.5);
    p
}

fn health(sim: &Sim, p: Entity) -> f32 {
    sim.app.world().get::<chimera_core::core::Health>(p).unwrap().current
}

/// One round from `p` at Source point `at`.
fn shoot(sim: &mut Sim, p: Entity, at: Vec3) {
    chimera_sim::testkit::aim_at(sim, p, to_engine(at));
    sim.ticks(1);
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    sim.ticks(10);
}

#[test]
fn a_combat_arms_round_breaks_a_window_pane_and_goes_through() {
    let mut sim = party_on_glass();
    let shooter = gunner(&mut sim, "combat_arms:m4a1", 0.0, 100.0);
    let target = runner(&mut sim, ca_movement::ID, 0.0, 320.0);
    let before = health(&sim, target);
    let chest = to_source(sim.position(target)) + Vec3::Z * 10.0;
    shoot(&mut sim, shooter, chest);
    let panes = broken_panes(&sim);
    let after = health(&sim, target);
    eprintln!("panes {panes:?}, target {before} -> {after}");
    assert!(panes.is_some_and(|n| n > 0), "the round broke the window: {panes:?}");
    assert!(after < before, "and went on into the soldier behind: {before} -> {after}");
}

#[test]
fn a_combat_arms_round_breaks_glass_and_goes_through() {
    let mut sim = party_on_glass();
    let shooter = gunner(&mut sim, "combat_arms:m4a1", -300.0, 100.0);
    let target = runner(&mut sim, ca_movement::ID, -300.0, 320.0);
    let before = health(&sim, target);
    let chest = to_source(sim.position(target)) + Vec3::Z * 10.0;
    shoot(&mut sim, shooter, chest);
    let after = health(&sim, target);
    eprintln!("glass broken {}, target {before} -> {after}", touch_glass_broken(&sim));
    assert!(touch_glass_broken(&sim), "the round broke the glass");
    assert!(after < before, "and went on into the soldier behind: {before} -> {after}");
}
