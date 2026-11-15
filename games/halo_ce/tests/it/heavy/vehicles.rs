//! Halo's vehicles on the disc's maps (`harness::HaloSim` with each map's
//! own props and entities): every multiplayer map's vehicle placements by
//! kind against vehicles.md's "Which maps hold which vehicles" (Blood Gulch
//! and Sidewinder 2 Warthogs, 4 Ghosts, 2 Scorpions; the other eleven
//! none); on Blood Gulch, Sidewinder and Hang Em High (no placements: one
//! of each kind made at three player starts) each vehicle rests on the
//! ground after 5 s and, driven by a seated player holding forward for
//! 10 s, stays inside the level and above its terrain; the seat cameras'
//! and the Warthog's overlays' cases on the disc. Passes with a note
//! without an install.

use std::collections::BTreeMap;

use avian3d::prelude::*;
use bevy::{ecs::system::RunSystemOnce, prelude::*};
use chimera_world::{
    harness::Sim,
    map::{MapData, MapSimPlugin},
};
use halo_ce::{
    game::{
        MULTIPLAYER_MAPS,
        maps::space,
        movement,
        vehicles::{HaloVehicle, Seated, VehicleDefs, camera::heading, level, place::Placed, spawn_vehicle},
    },
    harness::HaloSim,
};

use super::maps::{load, mount};

/// Every vehicle in the simulation.
fn vehicles(sim: &mut Sim) -> Vec<(Entity, HaloVehicle)> {
    let w = sim.app.world_mut();
    let mut q = w.query::<(Entity, &HaloVehicle)>();
    q.iter(w).map(|(e, v)| (e, v.clone())).collect()
}

/// The ground's height (WU) under a Halo point, by a ray from `above` WU
/// above it (under any overhang).
fn ground_under(sim: &mut Sim, p: Vec3, above: f32, skip: Entity) -> Option<f32> {
    let from = space::point([p.x, p.y, p.z + above]);
    sim.app
        .world_mut()
        .run_system_once(
            move |q: SpatialQuery,
                  characters: Query<(), With<movement::HaloMovement>>,
                  vehicles: Query<(), With<HaloVehicle>>| {
                let filter = SpatialQueryFilter::from_excluded_entities([skip]).with_mask(level::mask());
                q.cast_ray_predicate(from, Dir3::NEG_Y, 200.0, false, &filter, &|e| {
                    !characters.contains(e) && !vehicles.contains(e)
                })
                .map(|h| level::to_halo(from + Vec3::NEG_Y * h.distance).z)
            },
        )
        .unwrap()
}

/// Placements by vehicle document name.
fn kinds(map: &MapData) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for p in map.entities.iter().filter_map(Placed::from_entity) {
        *out.entry(p.vehicle).or_insert(0) += 1;
    }
    out
}

#[test]
fn every_maps_vehicle_placements_by_kind() {
    let Some(mount) = mount() else { return };
    for name in MULTIPLAYER_MAPS {
        let map = load(&mount, name, false);
        let got = kinds(&map);
        let want: BTreeMap<String, usize> = if matches!(*name, "bloodgulch" | "sidewinder") {
            [("warthog", 2), ("ghost", 4), ("scorpion", 2)]
                .map(|(k, n)| (k.to_string(), n))
                .into()
        } else {
            BTreeMap::new()
        };
        assert_eq!(got, want, "{name}");
    }
}

/// The level's bounds (engine space).
fn bounds(map: &MapData) -> (Vec3, Vec3) {
    map.collision_positions
        .iter()
        .fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), p| {
            (lo.min(Vec3::from(*p)), hi.max(Vec3::from(*p)))
        })
}

/// After 5 s every vehicle rests upright on the ground; then each, driven
/// forward by a seated player for 10 s, stays in the level and above the
/// terrain. `make`: vehicles to create (name, player start) first.
fn rest_and_drive(name: &str, make: &[(&str, usize)]) {
    let Some(mount) = mount() else { return };
    let map = load(&mount, name, false);
    let (lo, hi) = bounds(&map);
    let starts: Vec<(Vec3, f32)> = map
        .spawns
        .iter()
        .zip(&map.spawn_yaws)
        .map(|((p, _), y)| (level::to_halo(*p), heading(*y)))
        .collect();
    let mut sim = Sim::new(HaloSim, MapSimPlugin::new(map));
    for (kind, start) in make {
        let (at, facing) = starts[*start];
        spawn_vehicle(sim.app.world_mut(), kind, at + Vec3::Z * 0.5, facing).expect("a vehicle");
    }
    sim.seconds(5.0);
    let defs = sim.app.world().resource::<VehicleDefs>().clone();
    let found = vehicles(&mut sim);
    assert!(!found.is_empty(), "{name}: vehicles");
    for (e, v) in &found {
        let (d, phys) = &defs.0[v.def];
        let b = &v.drive.body;
        let origin = b.origin(phys.center_of_mass);
        let ground = ground_under(&mut sim, origin, 1.5, *e).expect("ground under");
        eprintln!(
            "{name}: {} at {origin}, ground {ground}, up {:?}, speed {}, touching {}, asleep {}",
            d.name,
            b.up,
            b.velocity.length(),
            v.drive.last.touching,
            v.drive.asleep
        );
        // Upright on its wheels (a slope tilts it: 25 degrees allowed).
        assert!(b.up.z > 0.89, "{}: upright: {:?}", d.name, b.up);
        assert!(b.velocity.length() < 0.01, "{}: at rest", d.name);
        assert!(
            origin.z - ground > -0.3 && origin.z - ground < 0.5,
            "{}: origin {} over ground {ground}",
            d.name,
            origin.z
        );
        assert!(v.drive.last.touching >= 3, "{}: on the ground", d.name);
    }
    // A driver in each, holding forward along its heading for 10 s.
    let mut drivers = Vec::new();
    for (e, v) in &found {
        let d = &defs.0[v.def].0;
        let seat = d.driver_seat().unwrap();
        let p = sim.spawn_character(
            space::point(v.drive.body.position.to_array()) + Vec3::Y * 3.0,
            movement::ID,
        );
        sim.app.world_mut().entity_mut(p).insert(Seated { vehicle: *e, seat });
        sim.app.world_mut().get_mut::<HaloVehicle>(*e).unwrap().riders[seat] = Some(p);
        let f = v.drive.body.forward;
        let mut i = sim.intent(p);
        i.yaw = space::yaw(f.y.atan2(f.x));
        i.move_axis = Vec2::new(0.0, 1.0);
        drivers.push((*e, v.drive.body.position, d.name.clone()));
    }
    let mut lowest: BTreeMap<Entity, f32> = BTreeMap::new();
    for _ in 0..30 {
        sim.seconds(1.0 / 3.0);
        for (e, _, n) in &drivers {
            let v = sim.app.world().get::<HaloVehicle>(*e).unwrap().clone();
            let phys = &defs.0[v.def].1;
            let o = v.drive.body.origin(phys.center_of_mass);
            // From the centre of mass down: the ground the vehicle is on,
            // not a rock or ledge it is pressed under.
            let c = v.drive.body.position;
            let ground = ground_under(&mut sim, c, 0.0, *e).unwrap_or(f32::MIN);
            let l = lowest.entry(*e).or_insert(f32::MAX);
            *l = l.min(c.z - ground);
            let engine = space::point(o.to_array());
            assert!(engine.cmpge(lo).all() && engine.cmple(hi).all(), "{name}: {n} left the level at {o}");
        }
    }
    for (e, start, n) in &drivers {
        let v = sim.app.world().get::<HaloVehicle>(*e).unwrap().clone();
        let gone = (v.drive.body.position - start).truncate().length();
        eprintln!("{name}: {n} drove {gone} WU, lowest centre of mass over ground {}", lowest[e]);
        assert!(gone > 3.0, "{name}: {n} drove {gone} WU in 10 s");
        assert!(lowest[e] > 0.0, "{name}: {n} through the terrain: {}", lowest[e]);
    }
}

#[test]
fn blood_gulchs_vehicles_rest_and_drive() {
    rest_and_drive("bloodgulch", &[]);
}

#[test]
fn sidewinders_vehicles_rest_and_drive() {
    rest_and_drive("sidewinder", &[]);
}

#[test]
fn vehicles_made_on_hang_em_high_rest_and_drive() {
    rest_and_drive("hangemhigh", &[("warthog", 0), ("ghost", 1), ("scorpion", 2)]);
}

/// A placed vehicle's markers (model frame, WU) by its document's name.
fn markers_of(map: &MapData, vehicle: &str) -> Option<std::collections::BTreeMap<String, [f32; 3]>> {
    map.entities
        .iter()
        .filter_map(Placed::from_entity)
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
    // The Ghost driver's `driver camera` is missing: the root node.
    use halo_ce::game::vehicles::camera::ROOT_NODE;
    let g = markers_of(&map, "ghost").expect("a placed Ghost");
    assert!(!g.contains_key("driver camera"));
    let hull = Vec3::from(g[ROOT_NODE]);
    near(hull, Vec3::new(0.3928, 0.0, 0.1711), "the Ghost's hull node");
    near(command(hull, o, 0.0, 0.0).camera, Vec3::new(-2.3596, 0.0, 0.8538), "Ghost camera");
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
        .filter_map(Placed::from_entity)
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

