//! Player clips across games (docs/DESIGN.md "Clips across games"): a
//! Source map's player clips (`core::MapPlayerClips`, brushes among
//! `core::MapBrushes`) stop every game's movement, and nothing else.
//! Headless on the party's greybox with a player-clip wall added the way a
//! Source map's loader gives one: CS:S's and HL2:DM's movements (Source's,
//! sweeping the brushes) and Combat Arms' (sweeping colliders: the clips'
//! players-only collider, `core::publish_player_clips`) each walk into the
//! wall and stay in front of it, a Combat Arms computer player too (on its
//! own maps its collision blockers let it pass, not another game's clips);
//! without the clip list Combat Arms' walks through (the guard's control).
//! Shots, sight and other traces (`SOLID_LAYERS`, `NOT_SHADOW`) pass the
//! clips and a physics body falls through them. Grenades: cs_source's
//! cs_grenades.rs; the real maps (cs_militia, cs_havana):
//! heavy/clips_on_maps.rs.

use avian3d::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};
use chimera_core::core::{CHARACTER_FILTER, MapBrush, MapBrushes, MapPlayerClips, NOT_SHADOW, Npc, SOLID_LAYERS};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::harness::CaSim;
use party_kit::PartyPartsPlugin;

const CSS: &str = cs_source::game::movement::ID;
const CA: &str = combat_arms::game::movement::ID;
const HL2: &str = hl2dm::game::movement::ID;

/// The clip wall: 6 m wide, 4 m tall, its south face at z = 2.5, north of
/// the greybox's first spawn, nothing else near.
const WALL_MIN: Vec3 = Vec3::new(-3.0, 0.0, 2.0);
const WALL_MAX: Vec3 = Vec3::new(3.0, 4.0, 2.5);
/// Where each walker starts, facing the wall.
const START: Vec3 = Vec3::new(0.0, 1.0, 6.0);

/// The party's simulation on the greybox, its brushes with the clip wall
/// (listed as a player clip unless `listed` is false).
fn sim(listed: bool) -> Sim {
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin));
    sim.ticks(2);
    let w = sim.app.world_mut();
    let mut brushes = w.resource_mut::<MapBrushes>();
    brushes.0.push(MapBrush::from_box(WALL_MIN, WALL_MAX));
    let i = brushes.0.len() - 1;
    if listed {
        w.insert_resource(MapPlayerClips(vec![i]));
    } else {
        // Combat Arms' movement doesn't see the brushes: it walks through.
        // (Source's still sweeps them.)
        w.insert_resource(MapPlayerClips(Vec::new()));
    }
    sim.ticks(3);
    sim
}

/// `p` walking north (into the wall) for 2 s: the least z its centre
/// reached.
fn walk_north(sim: &mut Sim, p: Entity) -> f32 {
    let mut least = f32::MAX;
    for _ in 0..(2.0 * sim.tick_hz()) as u32 {
        let mut i = sim.intent(p);
        i.yaw = 0.0;
        i.pitch = 0.0;
        i.move_axis = Vec2::Y;
        sim.ticks(1);
        least = least.min(sim.position(p).z);
    }
    least
}

/// One character of `movement` at `START`, settled, walked into the wall.
fn walker(sim: &mut Sim, movement: &'static str, npc: bool) -> f32 {
    let p = sim.spawn_character(START, movement);
    if npc {
        sim.app.world_mut().entity_mut(p).insert(Npc);
    }
    sim.seconds(1.0);
    let z = walk_north(sim, p);
    sim.app.world_mut().despawn(p);
    z
}

/// The guard: every game's movement stops at a Source player clip.
#[test]
fn every_movement_stops_at_a_player_clip() {
    let mut sim = sim(true);
    for movement in [CSS, HL2, CA] {
        let z = walker(&mut sim, movement, false);
        assert!(z > WALL_MAX.z, "{movement} walked into the clip: centre at z {z}");
    }
}

/// On another game's map Combat Arms' computer players meet the clips
/// too (only its own maps' collision blockers let them pass).
#[test]
fn a_combat_arms_computer_player_stops_at_another_games_clip() {
    let mut sim = sim(true);
    let z = walker(&mut sim, CA, true);
    assert!(z > WALL_MAX.z, "walked into the clip: centre at z {z}");
}

/// The control: without the clip list Combat Arms' movement (which
/// doesn't sweep the brushes) walks through the wall, as it did on
/// cs_militia; Source's still stops (it sweeps the brushes).
#[test]
fn without_the_clip_list_combat_arms_walks_through() {
    let mut sim = sim(false);
    assert!(walker(&mut sim, CA, false) < WALL_MIN.z, "walked through");
    assert!(walker(&mut sim, CSS, false) > WALL_MAX.z, "Source sweeps the brush");
}

/// Shots, sight and other traces pass the clips; a character's sweep
/// meets them.
#[test]
fn traces_pass_player_clips_and_characters_meet_them() {
    let mut sim = sim(true);
    let w = sim.app.world_mut();
    let mut state: SystemState<SpatialQuery> = SystemState::new(w);
    let spatial = state.get(w).unwrap();
    let from = Vec3::new(0.0, 1.0, 5.0);
    let ray = |mask| {
        spatial
            .cast_ray(from, Dir3::NEG_Z, 4.0, true, &SpatialQueryFilter::from_mask(mask))
            .map(|h| h.distance)
    };
    assert_eq!(ray(SOLID_LAYERS), None, "a shot or sight line met the clip");
    assert_eq!(ray(NOT_SHADOW), None, "a trace of everything met the clip");
    let hit = ray(CHARACTER_FILTER).expect("a character's sweep meets the clip");
    assert!((hit - (from.z - WALL_MAX.z)).abs() < 0.01, "at its face: {hit}");
}

/// A physics body (a prop, a ragdoll's part, a loose item) falls through a
/// clip: the clip's collider touches no body.
#[test]
fn a_physics_body_falls_through_a_clip() {
    let mut sim = sim(true);
    let ball = sim
        .app
        .world_mut()
        .spawn((
            RigidBody::Dynamic,
            Collider::sphere(0.2),
            Transform::from_translation(Vec3::new(0.0, 6.0, 2.25)),
        ))
        .id();
    sim.seconds(1.5);
    let y = sim.app.world().get::<Transform>(ball).unwrap().translation.y;
    assert!(y < 0.5, "it rests on the clip's top (4 m): y {y}");
}
