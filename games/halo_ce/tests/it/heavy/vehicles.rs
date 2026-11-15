//! Halo's vehicles on Blood Gulch (`harness::HaloSim` with the map's own
//! props and entities): the scenario's two Warthogs are placed, settle on
//! the ground at their placements (not sunk, not thrown), and a driver
//! holding the forward key toward the other base for ten seconds stays
//! inside the level's bounds and above its terrain. Passes with a note
//! without an install.

use avian3d::prelude::*;
use bevy::{ecs::system::RunSystemOnce, prelude::*};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use halo_ce::{
    game::{
        maps::space,
        movement,
        vehicles::{HaloVehicle, Seated, level},
    },
    harness::HaloSim,
};

use super::maps::{load, mount};

/// The spec's Blood Gulch Warthog placements (vehicles.md "Multiplayer").
const PLACEMENTS: [[f32; 3]; 2] = [[29.14, -90.83, 0.29], [102.31, -144.63, 0.58]];

fn vehicles(sim: &mut Sim) -> Vec<(Entity, HaloVehicle)> {
    let w = sim.app.world_mut();
    let mut q = w.query::<(Entity, &HaloVehicle)>();
    let mut out: Vec<(Entity, HaloVehicle)> = q.iter(w).map(|(e, v)| (e, v.clone())).collect();
    out.sort_by(|a, b| a.1.drive.body.position.x.total_cmp(&b.1.drive.body.position.x));
    out
}

/// The terrain's height (WU) under a Halo point, by a ray from high above.
fn ground_under(sim: &mut Sim, p: Vec3, skip: Entity) -> Option<f32> {
    let from = space::point([p.x, p.y, p.z + 20.0]);
    sim.app
        .world_mut()
        .run_system_once(
            move |q: SpatialQuery, characters: Query<(), With<movement::HaloMovement>>| {
                let filter = SpatialQueryFilter::from_excluded_entities([skip]).with_mask(level::mask());
                q.cast_ray_predicate(from, Dir3::NEG_Y, 200.0, false, &filter, &|e| !characters.contains(e))
                    .map(|h| level::to_halo(from + Vec3::NEG_Y * h.distance).z)
            },
        )
        .unwrap()
}

#[test]
fn blood_gulchs_warthogs_rest_and_drive() {
    let Some(mount) = mount() else { return };
    let map = load(&mount, "bloodgulch", false);
    let (lo, hi) = map
        .collision_positions
        .iter()
        .fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), p| {
            (lo.min(Vec3::from(*p)), hi.max(Vec3::from(*p)))
        });
    let placed: Vec<_> = map
        .entities
        .iter()
        .filter_map(halo_ce::game::vehicles::place::Placed::from_entity)
        .collect();
    eprintln!(
        "{} props, {} vehicle entities, warnings {:?}",
        map.props.len(),
        placed.len(),
        map.warnings
    );
    let mut sim = Sim::new(HaloSim, MapSimPlugin::new(map));
    sim.seconds(5.0);
    let found = vehicles(&mut sim);
    assert_eq!(found.len(), 2, "two Warthogs");
    let phys = halo_ce::game::vehicles::data::VehicleDef::warthog().physics_def();
    for ((e, v), place) in found.iter().zip(PLACEMENTS) {
        let b = &v.drive.body;
        let origin = b.origin(phys.center_of_mass);
        let ground = ground_under(&mut sim, origin, *e).expect("ground under");
        eprintln!(
            "Warthog at {origin} (placed {place:?}), ground {ground}, up {:?}, speed {}, touching {}, asleep {}",
            b.up,
            b.velocity.length(),
            v.drive.last.touching,
            v.drive.asleep
        );
        assert!(
            (origin.truncate() - Vec3::from(place).truncate()).length() < 0.5,
            "near its placement"
        );
        assert!(b.up.z > 0.9, "upright: {:?}", b.up);
        assert!(b.velocity.length() < 0.01, "at rest");
        // The model origin a little above or below the ground under it
        // (the spec's resting origin is 0.032 WU up on flat ground).
        assert!(
            origin.z - ground > -0.25 && origin.z - ground < 0.3,
            "origin {} over ground {ground}",
            origin.z
        );
        assert!(v.drive.last.touching >= 3, "on its wheels");
    }
    // A driver in the first, holding forward toward the second for 10 s.
    let (w, v) = found[0].clone();
    let target = found[1].1.drive.body.position;
    let heading = (target - v.drive.body.position).truncate();
    let heading = heading.y.atan2(heading.x);
    let p = sim.spawn_character(
        space::point(v.drive.body.position.to_array()) + Vec3::Y * 3.0,
        movement::ID,
    );
    sim.app.world_mut().entity_mut(p).insert(Seated { vehicle: w, seat: 0 });
    sim.app.world_mut().get_mut::<HaloVehicle>(w).unwrap().riders[0] = Some(p);
    {
        let mut i = sim.intent(p);
        i.yaw = space::yaw(heading);
        i.move_axis = Vec2::new(0.0, 1.0);
    }
    let start = v.drive.body.position;
    let mut lowest = f32::MAX;
    for _ in 0..30 {
        sim.seconds(1.0 / 3.0);
        let v = sim.app.world().get::<HaloVehicle>(w).unwrap().clone();
        let o = v.drive.body.origin(phys.center_of_mass);
        let ground = ground_under(&mut sim, o, w).unwrap_or(f32::MIN);
        lowest = lowest.min(o.z - ground);
        let engine = space::point(o.to_array());
        assert!(
            engine.cmpge(lo).all() && engine.cmple(hi).all(),
            "inside the level: {o}"
        );
    }
    let v = sim.app.world().get::<HaloVehicle>(w).unwrap().clone();
    let gone = (v.drive.body.position - start).truncate().length();
    eprintln!(
        "drove {gone} WU, lowest origin over ground {lowest}, up {:?}",
        v.drive.body.up
    );
    assert!(gone > 20.0, "drove {gone} WU in 10 s");
    assert!(lowest > -0.5, "never through the terrain: {lowest}");
}

/// A placed vehicle's markers (model frame, WU) by its document's name.
fn markers_of(map: &chimera_world::map::MapData, vehicle: &str) -> Option<std::collections::BTreeMap<String, [f32; 3]>> {
    map.entities
        .iter()
        .filter_map(halo_ce::game::vehicles::place::Placed::from_entity)
        .find(|p| p.vehicle == vehicle)
        .map(|p| p.markers)
}

#[test]
fn the_seat_cameras_on_the_disc() {
    use halo_ce::game::vehicles::camera::{command, track_point};
    let Some(mount) = mount() else { return };
    let map = load(&mount, "bloodgulch", false);
    let d = halo_ce::game::vehicles::data::VehicleDef::warthog();
    let m = markers_of(&map, "warthog").expect("a placed Warthog");
    let near = |a: Vec3, b: Vec3, what: &str| assert!((a - b).abs().max_element() < 2e-3, "{what}: {a} against {b}");
    // The gunner's focus `camera gunner`; level, turret forward.
    let gunner = Vec3::from(m["camera gunner"]);
    near(gunner, Vec3::new(-0.2235, 0.0, 0.8131), "camera gunner");
    let o = track_point(&d.tracks["warthog_driver_loose"].points, 0.0);
    near(command(gunner, o, 0.0, 0.0).camera, Vec3::new(-2.9759, 0.0, 1.4958), "gunner camera");
    // The passenger's eye.
    near(Vec3::from(m["camera rider"]), Vec3::new(0.0715, -0.1586, 0.8056), "camera rider");
}

#[test]
fn the_warthogs_overlays_on_the_disc() {
    use halo_ce::game::body::anims;
    use halo_ce::game::vehicles::look::{self, GROUND_SPEED, STEERING, VehicleLook};
    let Some(mount) = mount() else { return };
    let map = load(&mount, "bloodgulch", false);
    let placed = map
        .entities
        .iter()
        .filter_map(halo_ce::game::vehicles::place::Placed::from_entity)
        .find(|p| p.vehicle == "warthog")
        .expect("a placed Warthog");
    let rig = map.models[map.props[placed.prop].model].rig.clone().expect("a rigged model");
    let set = &rig.animations;
    // Four suspension elements, the tyres 11 to 14, E -0.31445, C -0.13452.
    let points: Vec<usize> = placed.suspension.iter().map(|s| s.point).collect();
    assert_eq!(points.len(), 4, "{points:?}");
    assert!(points.iter().all(|p| (11..=14).contains(p)), "{points:?}");
    for s in &placed.suspension {
        assert!((s.extension + 0.31445).abs() < 1e-4 && (s.compression + 0.13452).abs() < 1e-4, "{s:?}");
    }
    // The steering screen: 3 frames, 35 degrees per column; tau = 30
    // degrees is 6/7 of the way to the full-left frame.
    let steering = set.names[STEERING];
    assert_eq!(anims::frames(set, steering), 3);
    let bounds = anims::screen_bounds(&set.sequences[steering]).expect("a screen");
    let cell = bounds.locate(30f32.to_radians(), 0.0).unwrap();
    assert!((cell.u - 6.0 / 7.0).abs() < 1e-3, "{cell:?}");
    assert_eq!(anims::frames(set, set.names[GROUND_SPEED]), 4);
    // Turned and spun, the pose moves bones the bind pose has elsewhere.
    let still = look::posed(set, &VehicleLook::default(), 1.0);
    let turned = look::posed(
        set,
        &VehicleLook {
            turn: 30f32.to_radians(),
            wheel: 0.3,
            ..Default::default()
        },
        1.0,
    );
    let moved = still
        .iter()
        .zip(&turned)
        .filter(|(a, b)| a.0.angle_between(b.0) > 0.05)
        .count();
    assert!(moved >= 5, "the steering wheel and the four hubs or tyres: {moved} bones moved");
}

