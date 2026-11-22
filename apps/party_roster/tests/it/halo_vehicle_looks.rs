//! Halo's vehicles' looks in a party app (the party's simulation, `CaSim`
//! with `PartyPartsPlugin`, on the greybox; synthetic models, no disc): a
//! vehicle's joints are posed by Halo's overlays (`vehicles::look::pose`)
//! and by nothing else. The party has Combat Arms' props starter, which
//! gave every animated prop without a sequence its first one, so the
//! engine's prop animator played a Warthog's first overlay over its wheels
//! (the playtest of 2026-11-17: the wheels never turned, a Ghost showed no
//! animation); now the vehicle is `PosedByGame` and both leave it alone.
//! A Combat Arms soldier seated as the driver drives a Warthog with its
//! throttle held: the
//! wheel's joint turns with the ground speed, each frame Halo's pose,
//! whatever order the engine's prop animator runs in.

use std::sync::Arc;

use bevy::{ecs::system::RunSystemOnce, prelude::*};
use chimera_core::core::Carried;
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
    map::{
        PosedByGame, PropRig, PropSequence,
        anim::{AnimSet, Animation, Animator, Sequence, Track},
        animate_props,
    },
};
use combat_arms::harness::CaSim;
use halo_ce::game::vehicles::{
    self, HaloVehicle, HeldThrottle, Seated, VehicleDefs, level,
    look::{self, GROUND_SPEED, VehicleLook},
};
use party_roster::PartyPartsPlugin;

/// One bone, a 4-frame overlay turning it a quarter turn a frame about z
/// (the ground-speed overlay's shape).
fn spinning_set() -> AnimSet {
    let mut set = AnimSet {
        defaults: vec![(Quat::IDENTITY, Vec3::ZERO)],
        bases: vec![vec![(Quat::IDENTITY, Vec3::ZERO)]],
        parents: vec![None],
        ..Default::default()
    };
    set.animations.push(Animation {
        name: "spin".into(),
        fps: 30.0,
        frames: 4,
        delta: true,
        tracks: vec![Track {
            bone: 0,
            rotation: Some((0..4).map(|f| Quat::from_rotation_z(f as f32 * std::f32::consts::FRAC_PI_2)).collect()),
            position: None,
        }],
        ..Default::default()
    });
    set.sequences.push(Sequence {
        name: GROUND_SPEED.into(),
        anims: vec![0],
        grid: (1, 1),
        ..Default::default()
    });
    set.names.insert(GROUND_SPEED.into(), 0);
    set
}

/// Vehicle `name` on the greybox, its model rigged with the spinning set
/// (as the client's spawn rigs a vehicle's model): (vehicle, its joint).
fn rigged(sim: &mut Sim, name: &str, at: Vec3) -> (Entity, Entity) {
    let w = vehicles::spawn_vehicle(sim.app.world_mut(), name, level::to_halo(at), 0.0).unwrap();
    let joint = sim.app.world_mut().spawn(Transform::default()).id();
    sim.app.world_mut().entity_mut(w).insert(PropRig {
        animator: Animator::new(Arc::new(spinning_set())),
        joints: vec![joint],
        seen: None,
        default: None,
        done: false,
    });
    (w, joint)
}

/// The joint's rotation and Halo's pose for the vehicle's look now.
fn joint_and_halos(sim: &Sim, w: Entity, joint: Entity) -> (Quat, Quat) {
    let world = sim.app.world();
    let v = world.get::<HaloVehicle>(w).unwrap();
    let c = world.resource::<VehicleDefs>().0[v.def].0.wheel_circumference;
    let look = world.get::<VehicleLook>(w).unwrap();
    let want = look::posed(&world.get::<PropRig>(w).unwrap().animator.set, look, c)[0].0;
    (world.get::<Transform>(joint).unwrap().rotation, want)
}

#[test]
fn a_party_poses_halos_vehicles_by_their_overlays_only() {
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin));
    let base = greybox::SPAWNS[0] + Vec3::new(6.0, 0.0, 0.0);
    let (warthog, wheel) = rigged(&mut sim, "warthog", base);
    let (ghost, ghost_joint) = rigged(&mut sim, "ghost", base + Vec3::new(8.0, 0.0, 0.0));
    sim.seconds(1.0);
    for (w, name) in [(warthog, "warthog"), (ghost, "ghost")] {
        let e = sim.app.world().entity(w);
        assert!(e.contains::<PosedByGame>(), "the {name} is posed by its game");
        assert!(!e.contains::<PropSequence>(), "no other game's starter gave the {name} a sequence");
    }
    let _ = ghost_joint;

    // A Combat Arms soldier in the driver's seat, its throttle held.
    let seat = {
        let world = sim.app.world();
        let v = world.get::<HaloVehicle>(warthog).unwrap();
        let (d, phys) = &world.resource::<VehicleDefs>().0[v.def];
        level::to_engine(v.world(phys, v.seat_marker(d, 0)))
    };
    // Seated as the use key seats (`vehicles::sit`; the use key's search
    // is the heavy tests').
    let p = sim.spawn_character(seat + Vec3::Y * 0.9, combat_arms::game::movement::ID);
    sim.ticks(1);
    sim.app.world_mut().get_mut::<HaloVehicle>(warthog).unwrap().riders[0] = Some(p);
    sim.app.world_mut().entity_mut(p).insert((Seated { vehicle: warthog, seat: 0 }, Carried(warthog)));
    sim.ticks(1);
    assert!(sim.app.world().get::<Seated>(p).is_some(), "seated");
    sim.app.insert_resource(HeldThrottle(1.0));

    // The headless party runs no prop animator (the client's map does):
    // run it here after each frame.
    sim.app.world_mut().init_resource::<bevy::ecs::message::Messages<chimera_world::map::PropAnimationDone>>();
    let mut turned = 0.0f32;
    let mut last = joint_and_halos(&sim, warthog, wheel).0;
    for frame in 0..120 {
        sim.app.update();
        let (got, want) = joint_and_halos(&sim, warthog, wheel);
        assert!(got.angle_between(want) < 1e-4, "frame {frame}: the wheel at Halo's pose");
        // The engine's prop animator, run again after: nothing changes.
        sim.app.world_mut().run_system_once(animate_props).unwrap();
        let (again, _) = joint_and_halos(&sim, warthog, wheel);
        assert!(again.angle_between(want) < 1e-4, "frame {frame}: the prop animator left it alone");
        turned += got.angle_between(last);
        last = got;
    }
    let v = sim.app.world().get::<HaloVehicle>(warthog).unwrap();
    assert!(v.drive.body.velocity.length() > 0.01, "driven: {:?}", v.drive.body.velocity);
    assert!(turned > 1.0, "the wheel turned with the speed: {turned} rad");
}
