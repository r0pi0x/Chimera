//! `points = "random_hidden"` (`chimera_sim::minigame::spawns`) on the
//! greybox: spawns spread over the map and out of the enemy's sight, at
//! the least distance, the same for a seed, on spots a character can stand
//! and walk from, in zones' and water's absence, and the map's spawn
//! points when the mesh has no areas; inside the map only: never on a
//! mesh generated for another walker (bots' climbs: de_aztec's spawn over
//! its wall), never in a solid or void leaf; off the areas' centred grid
//! (de_dust2's "seam": a row of eyes in the plane where two faces meet).

use avian3d::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};
use chimera_core::core::{Damage, DamageKind, Dead, Hitgroup, Intent, MapBrush, SOLID_LAYERS};
use chimera_sim::{
    minigame::{
        self, MinigameAppExt, MinigamePlugin,
        spawns::{self, SpawnHazardAppExt, SpawnSeed, SpawnSpots},
    },
    movement::placeholder,
    testkit,
};
use chimera_world::{
    greybox,
    harness::Sim,
    map::{
        nav::{NavArea, NavMesh},
        vis::{ActiveVisibility, MapVisibility},
    },
};

const DEF: &str = r#"
id = "kit:deathmatch"
name = "Kit deathmatch"
players = { min = 2, max = 4 }
maps = ["chimera:greybox"]
teams = "free_for_all"
time_limit = "60m"

[spawn]
respawn = { delay = "1s", protection = "0.5s" }
points = "random_hidden"
min_distance = 10.0

[rules]
module = "chimera:ladder"
ladder = ["kit:pistol", "kit:rifle", "kit:shotgun"]
demote_on = "melee_death"
"#;

fn started(def: &str, setup: impl FnOnce(&mut App)) -> Sim {
    let mut sim = Sim::with(chimera_sim::SimBasePlugins, |app| {
        app.add_plugins((greybox::GreyboxMapPlugin, MinigamePlugin));
        testkit::add_weapons(app);
        app.add_minigames(&[("kit_deathmatch.toml", def)]).expect("the definition reads");
        setup(app);
    });
    sim.ticks(5);
    minigame::start(sim.app.world_mut(), "kit:deathmatch").expect("it starts");
    sim
}

fn player(sim: &mut Sim) -> Entity {
    sim.spawn_character(greybox::SPAWNS[0], placeholder::ID)
}

/// `p` dies of the world and is back after the delay.
fn die_and_return(sim: &mut Sim, p: Entity) {
    let at = sim.position(p);
    sim.app.world_mut().write_message(Damage {
        target: p,
        attacker: None,
        amount: 500.0,
        point: at,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Chest,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(2);
    let w = sim.app.world();
    assert!(
        w.get::<Dead>(p).is_some(),
        "dead: health {:?} god {} in {:?} phase {:?} prot {:?} now {:?}",
        w.get::<chimera_core::core::Health>(p),
        w.get::<chimera_core::core::God>(p).is_some(),
        w.get::<minigame::InMinigame>(p).is_some(),
        w.resource::<chimera_sim::mode::Match>().phase,
        w.get::<minigame::Protected>(p),
        w.resource::<chimera_core::core::SimClock>().now
    );
    sim.seconds(1.7);
    assert!(sim.app.world().get::<Dead>(p).is_none(), "back");
}

/// Two players in, `n` respawns of the second: where it came back.
fn respawns(sim: &mut Sim, n: usize) -> (Entity, Entity, Vec<Vec3>) {
    let a = player(sim);
    let b = player(sim);
    sim.seconds(1.7);
    let mut at = Vec::new();
    for _ in 0..n {
        die_and_return(sim, b);
        at.push(sim.position(b));
    }
    (a, b, at)
}

/// Whether the line from `from` to `to` meets the solid world.
fn blocked(sim: &mut Sim, from: Vec3, to: Vec3) -> bool {
    let w = sim.app.world_mut();
    let characters: Vec<Entity> = w.query_filtered::<Entity, With<Intent>>().iter(w).collect();
    let mut state = SystemState::<SpatialQuery>::new(w);
    let q = state.get(w).unwrap();
    let filter = SpatialQueryFilter::from_mask(SOLID_LAYERS).with_excluded_entities(characters);
    let d = to - from;
    q.cast_ray(from, Dir3::new(d).unwrap(), d.length() - 0.05, true, &filter).is_some()
}

#[test]
fn spawns_spread_over_the_map_not_just_the_spawn_points() {
    let mut sim = started(DEF, |_| {});
    let (_, _, at) = respawns(&mut sim, 50);
    let cells: std::collections::BTreeSet<(i32, i32)> =
        at.iter().map(|p| ((p.x / 10.0).floor() as i32, (p.z / 10.0).floor() as i32)).collect();
    assert!(cells.len() >= 14, "{} 10 m cells of 64 in 50 spawns: {cells:?}", cells.len());
    let near_a_point = at
        .iter()
        .filter(|p| greybox::SPAWNS.iter().any(|s| s.xz().distance(p.xz()) < 2.0))
        .count();
    assert!(near_a_point < 10, "{near_a_point} of 50 at the map's own spawn points");
    let spots = sim.app.world().resource::<SpawnSpots>();
    assert!(spots.from_nav && spots.spots.len() > 100, "{} spots from nav: {}", spots.spots.len(), spots.from_nav);
}

#[test]
fn a_spawn_is_out_of_the_enemys_sight_and_far_enough() {
    let mut sim = started(DEF, |_| {});
    let (a, _, at) = respawns(&mut sim, 40);
    let eye = sim.position(a) + Vec3::Y * 0.7;
    let centre = sim.position(a);
    for p in at {
        assert!(p.distance(centre) >= 10.0 - 0.2, "{p} is {} m from the enemy", p.distance(centre));
        let feet = p - Vec3::Y * 0.9;
        assert!(blocked(&mut sim, eye, feet + Vec3::Y * 1.7), "{p}: head in sight of {eye}");
        assert!(blocked(&mut sim, eye, feet + Vec3::Y * 0.1), "{p}: feet in sight of {eye}");
    }
}

#[test]
fn the_same_seed_spawns_the_same_and_another_differently() {
    let run = |seed: u32| {
        let mut sim = started(DEF, |app| {
            app.insert_resource(SpawnSeed(seed));
        });
        respawns(&mut sim, 12).2
    };
    let (one, again, other) = (run(7), run(7), run(8));
    assert_eq!(one, again, "same seed");
    assert_ne!(one, other, "another seed");
}

#[test]
fn a_spawn_can_be_stood_on_and_walked_from() {
    let mut sim = started(DEF, |_| {});
    let (_, b, at) = respawns(&mut sim, 25);
    // Each spot, tried: b there, settled, then walking in four directions.
    for p in at {
        let mut best = 0.0f32;
        for yaw in [0.0f32, 1.57, 3.14, 4.71] {
            let w = sim.app.world_mut();
            w.entity_mut(b).insert((
                Transform::from_translation(p),
                avian3d::prelude::Position(p),
                chimera_core::core::Velocity(Vec3::ZERO),
            ));
            sim.intent(b).move_axis = Vec2::ZERO;
            sim.seconds(0.3);
            let still = sim.position(b);
            assert!((still.y - p.y).abs() < 0.3 && still.xz().distance(p.xz()) < 0.3, "{p} settled at {still}");
            {
                let mut i = sim.intent(b);
                i.yaw = yaw;
                i.move_axis = Vec2::new(0.0, 1.0);
            }
            sim.seconds(0.6);
            best = best.max(sim.position(b).xz().distance(still.xz()));
        }
        sim.intent(b).move_axis = Vec2::ZERO;
        assert!(best > 1.0, "stuck at {p}: moved {best} m");
    }
}

#[test]
fn without_areas_the_map_s_spawn_points_are_the_spots() {
    let mut sim = started(DEF, |app| {
        app.insert_resource(NavMesh::default());
    });
    let (_, _, at) = respawns(&mut sim, 12);
    for p in at {
        assert!(greybox::SPAWNS.iter().any(|s| s.xz().distance(p.xz()) < 0.3), "{p}: not at a spawn point");
    }
    assert!(!sim.app.world().resource::<SpawnSpots>().from_nav);
}

#[test]
fn it_never_fails_when_nothing_is_far_or_hidden() {
    // 500 m from an enemy: no spot qualifies; the farthest is taken.
    let mut sim = started(&DEF.replace("min_distance = 10.0", "min_distance = 500.0"), |_| {});
    let (a, _, at) = respawns(&mut sim, 6);
    let from = sim.position(a);
    assert_eq!(at.len(), 6);
    assert!(at.iter().all(|p| p.distance(from) > 20.0), "far from the enemy: {at:?}");
}

#[test]
fn zones_that_hurt_are_not_spawned_in() {
    // The west half of the floor is a hurting zone.
    fn zone(_: &mut World) -> Vec<MapBrush> {
        vec![MapBrush::from_box(Vec3::new(-50.0, -1.0, -50.0), Vec3::new(0.0, 5.0, 50.0))]
    }
    let mut sim = started(DEF, |app| {
        app.add_spawn_hazards(zone);
    });
    let (_, _, at) = respawns(&mut sim, 30);
    assert!(at.iter().all(|p| p.x > 0.0), "none in the zone: {at:?}");
    assert!(sim.app.world().resource::<SpawnSpots>().spots.iter().all(|s| s.x > 0.0));
}

#[test]
fn the_other_options_still_work() {
    for points in ["map", "far_from_enemies", "team"] {
        let def = DEF.replace("random_hidden", points);
        let mut sim = started(&def, |_| {});
        let (_, _, at) = respawns(&mut sim, 3);
        assert!(
            at.iter().all(|p| greybox::SPAWNS.iter().any(|s| s.xz().distance(p.xz()) < 0.3)),
            "{points}: {at:?}"
        );
    }
}

/// A level area of `NavMesh` from (x0, z0) to (x1, z1) at height `y`.
fn area(id: u32, x0: f32, z0: f32, x1: f32, z1: f32, y: f32) -> NavArea {
    NavArea {
        id,
        flags: 0,
        min: Vec2::new(x0, z0),
        max: Vec2::new(x1, z1),
        heights: [y; 4],
        center: Vec3::new((x0 + x1) / 2.0, y, (z0 + z1) / 2.0),
        links: Vec::new(),
        place: None,
        hiding: Vec::new(),
        visible: Vec::new(),
        encounters: Vec::new(),
    }
}

/// The top of the south wall (6 m up, 1 m wide): a character fits on it
/// but can't walk there from the spawns.
fn wall_top() -> NavMesh {
    NavMesh {
        areas: vec![area(1, -20.0, 40.0, 20.0, 41.0, 6.0)],
        ..default()
    }
}

#[test]
fn a_mesh_generated_for_another_walker_is_not_spawned_on() {
    // The map's own mesh is taken as it is (its top is a spot)...
    let mut sim = started(DEF, |app| {
        app.insert_resource(wall_top());
    });
    let (_, _, at) = respawns(&mut sim, 4);
    assert!(at.iter().all(|p| p.y > 5.0), "the map's own areas: {at:?}");
    // ...but one a walker made (bots', which climb ledges a player can't)
    // isn't: the spots are walked from the spawn points.
    let mut sim = started(DEF, |app| {
        app.insert_resource(NavMesh {
            generated: true,
            ..wall_top()
        });
    });
    let (_, _, at) = respawns(&mut sim, 12);
    assert!(at.iter().all(|p| p.y < 3.0), "on the floor: {at:?}");
    let spots = sim.app.world().resource::<SpawnSpots>().clone();
    assert!(spots.from_nav && spots.spots.len() > 80, "{} spots", spots.spots.len());
    assert!(spots.spots.iter().all(|s| s.y < 3.0));
}

#[test]
fn spots_in_solid_or_void_leaves_are_not_spawned_in() {
    // Visibility data whose west half (x < 0) is a solid leaf, as the void
    // outside a map's world is: no spot there.
    let vis = MapVisibility {
        planes: vec![(Vec3::X, 0.0)],
        nodes: vec![(0, [-1, -2])],
        leaf_clusters: vec![0, -1],
        cluster_count: 1,
        visible: vec![vec![1]],
        ..default()
    };
    let mut sim = started(DEF, |app| {
        app.insert_resource(ActiveVisibility(std::sync::Arc::new(vis)));
    });
    let (_, _, at) = respawns(&mut sim, 20);
    assert!(at.iter().all(|p| p.x >= 0.0), "none in the solid leaf: {at:?}");
    let spots = sim.app.world().resource::<SpawnSpots>().clone();
    assert!(spots.spots.len() > 40 && spots.spots.iter().all(|s| s.x >= 0.0));
}

/// A dead end on the greybox's floor: walls north, south and east of
/// (25, 20), open to the west. Returns the spot (feet).
fn dead_end(sim: &mut Sim) -> Vec3 {
    let w = sim.app.world_mut();
    for (size, at) in [
        (Vec3::new(42.0, 4.0, 0.5), Vec3::new(5.0, 2.0, 18.0)),
        (Vec3::new(42.0, 4.0, 0.5), Vec3::new(5.0, 2.0, 22.0)),
        (Vec3::new(0.5, 4.0, 4.0), Vec3::new(26.0, 2.0, 20.0)),
    ] {
        w.spawn((RigidBody::Static, Collider::cuboid(size.x, size.y, size.z), Transform::from_translation(at)));
    }
    Vec3::new(25.0, 0.0, 20.0)
}

/// The direction a yaw faces, flat.
fn heading(yaw: f32) -> Vec3 {
    Quat::from_rotation_y(yaw) * Vec3::NEG_Z
}

#[test]
fn a_spawn_faces_away_from_the_wall_in_front_and_varies_in_its_sector() {
    let mut sim = started(DEF, |_| {});
    let feet = dead_end(&mut sim);
    sim.ticks(3);
    let f = minigame::def::Facing::default();
    let yaws: Vec<f32> = (0..40)
        .map(|seed| {
            let (yaw, pitch) = spawns::facing_at(sim.app.world_mut(), feet, &f, seed).expect("physics");
            assert!((pitch + 2f32.to_radians()).abs() < 1e-4, "pitch a little down: {pitch}");
            yaw
        })
        .collect();
    for yaw in &yaws {
        // West is -X: within the jitter of it, never toward the wall at +X.
        let off = heading(*yaw).angle_between(Vec3::NEG_X).to_degrees();
        assert!(off <= f.jitter + 1.0, "{off} degrees off the open side");
    }
    let (lo, hi) = yaws.iter().fold((f32::MAX, f32::MIN), |(l, h), y| (l.min(*y), h.max(*y)));
    assert!((hi - lo).to_degrees() > 10.0, "varies over seeds: {} degrees", (hi - lo).to_degrees());
    // Without the jitter or the wall's reach the facing is the open side exactly.
    let still = minigame::def::Facing { jitter: 0.0, ..f };
    let (yaw, _) = spawns::facing_at(sim.app.world_mut(), feet, &still, 1).unwrap();
    assert!(heading(yaw).angle_between(Vec3::NEG_X).to_degrees() < 6.0);
    // Deterministic for a seed.
    let mut again = |s| spawns::facing_at(sim.app.world_mut(), feet, &f, s).unwrap();
    let (a, b, c) = (again(5), again(5), again(6));
    assert_eq!(a, b);
    assert_ne!(a.0, c.0);
}

#[test]
fn respawned_players_face_open_space_not_a_fixed_direction() {
    let mut sim = started(DEF, |_| {});
    let (_, b, _) = respawns(&mut sim, 12);
    let w = sim.app.world();
    let i = w.get::<Intent>(b).unwrap();
    // Pitched a little down, and (jittered) off the eight fixed headings.
    assert!((i.pitch + 2f32.to_radians()).abs() < 1e-3, "pitch {}", i.pitch);
    let on_grid = (i.yaw / std::f32::consts::FRAC_PI_4).fract().abs() < 1e-3;
    assert!(!on_grid, "yaw {} is a fixed heading", i.yaw);
}

#[test]
fn facing_costs_well_under_a_millisecond() {
    let mut sim = started(DEF, |_| {});
    let f = minigame::def::Facing::default();
    respawns(&mut sim, 1);
    let spots = sim.app.world().resource::<SpawnSpots>().spots.clone();
    assert!(spots.len() > 40);
    let w = sim.app.world_mut();
    spawns::facing_at(w, spots[0], &f, 0);
    let n = 300;
    let t = std::time::Instant::now();
    for k in 0..n {
        spawns::facing_at(w, spots[k % spots.len()], &f, k as u32);
    }
    let each = t.elapsed() / n as u32;
    println!("facing_at: {each:?} per spawn");
    assert!(each < std::time::Duration::from_millis(1), "{each:?} per spawn");
}

/// An area symmetric about x = 0 and z = 0, as nav areas often are about
/// a line of the map's own grid (where its faces meet: de_dust2's y 2304
/// "seam", 2026-10-30): centred, 1.5 m apart, every spot would sit on
/// x, z = 0 (mod 1.5 m), a whole row in each centre plane. Nudged, few
/// are within 1 cm of that lattice, none moves more than `SPOT_NUDGE`,
/// and all stay half a metre inside the area.
#[test]
fn spots_are_nudged_off_the_areas_centred_grid() {
    let mut sim = started(DEF, |app| {
        app.insert_resource(NavMesh {
            areas: vec![area(1, -8.5, -8.5, 8.5, 8.5, 0.0)],
            ..default()
        });
    });
    let spots = spawns::candidates(sim.app.world_mut());
    assert!(spots.len() > 50, "{} spots", spots.len());
    let off = |c: f32| {
        let r = c.rem_euclid(spawns::SPOT_GRID);
        r.min(spawns::SPOT_GRID - r)
    };
    let on = spots.iter().filter(|s| off(s.x) < 0.01 || off(s.z) < 0.01).count();
    assert!(on * 7 < spots.len(), "{on} of {} spots on the centred grid's lines", spots.len());
    assert!(
        spots.iter().all(|s| off(s.x) <= spawns::SPOT_NUDGE + 1e-4 && off(s.z) <= spawns::SPOT_NUDGE + 1e-4),
        "moved at most the nudge"
    );
    assert!(spots.iter().all(|s| s.x.abs() <= 8.0 + 1e-4 && s.z.abs() <= 8.0 + 1e-4), "inside the area");
}
