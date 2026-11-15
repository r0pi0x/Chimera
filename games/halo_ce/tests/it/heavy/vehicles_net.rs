//! Halo's vehicles in a network game on Blood Gulch (`NetSim`: Chimera's
//! server and two clients on a simulated 50 ms link, each with the map's
//! own props and entities): the first client's player walks to a Warthog's
//! driver seat and presses the use key, the second's takes the passenger
//! seat; the driver holds the forward key for four seconds. Every machine
//! knows who sits where (`vehicles::NetSeat` replicated, `Seated` and the
//! engine's `Carried` on each copy); the second client sees the Warthog
//! move; the driver's client views through the seat's camera and does not
//! fight the server (no prediction error while seated); the passenger's
//! own player stays in its seat of the Warthog it draws. Then the driver
//! gets out (no seat left on any machine) and the passenger's client
//! leaves (its seat empties on the server). Passes with a note without an
//! install.

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::core::{Carried, Intent, MapView};
use chimera_net::{
    harness::NetSim,
    net::{memory::LinkConditions, predict::NetGraph},
};
use chimera_world::map::{MapSimPlugin, PropIndex};
use halo_ce::{
    game::{
        TICK_INTERVAL,
        maps::space,
        vehicles::{HaloVehicle, NetSeat, Seated, VehicleDefs, level},
    },
    harness::HaloSim,
};

use super::maps::{load, mount};

/// The Warthog nearest the map's first placement, in a world: (entity,
/// its prop index).
fn warthog(world: &mut World) -> (Entity, usize) {
    let mut q = world.query::<(Entity, &PropIndex, &HaloVehicle)>();
    let mut all: Vec<(Entity, usize, f32)> = q
        .iter(world)
        .map(|(e, p, v)| (e, p.0, v.drive.body.position.x))
        .collect();
    all.sort_by(|a, b| a.2.total_cmp(&b.2));
    (all[0].0, all[0].1)
}

/// The vehicle on this machine with prop index `prop`.
fn by_prop(world: &mut World, prop: usize) -> Entity {
    let mut q = world.query::<(Entity, &PropIndex, &HaloVehicle)>();
    q.iter(world)
        .find(|(_, p, _)| p.0 == prop)
        .map(|(e, ..)| e)
        .expect("the Warthog's copy")
}

/// Seat `seat`'s marker in the world (engine metres), on the server.
fn seat_at(world: &World, w: Entity, seat: usize) -> Vec3 {
    let v = world.get::<HaloVehicle>(w).unwrap();
    let (d, phys) = &world.resource::<VehicleDefs>().0[v.def];
    level::to_engine(v.world(phys, v.seat_marker(d, seat)))
}

/// Put the server's character `e` at `at` (a teleport the client takes as
/// the server's).
fn teleport(world: &mut World, e: Entity, at: Vec3) {
    world.get_mut::<Transform>(e).unwrap().translation = at;
    if let Some(mut p) = world.get_mut::<avian3d::prelude::Position>(e) {
        p.0 = at;
    }
    world.get_mut::<chimera_core::core::Velocity>(e).unwrap().0 = Vec3::ZERO;
    chimera_core::core::rebase(world, e);
}

fn press_use(sim: &mut NetSim, i: usize) {
    let me = sim.local_player(i).unwrap();
    sim.clients[i].app.world_mut().get_mut::<Intent>(me).unwrap().use_key = true;
    sim.ticks(3);
    sim.clients[i].app.world_mut().get_mut::<Intent>(me).unwrap().use_key = false;
    sim.ticks(1);
}

/// Who sits where on a machine: (rider has `Seated`, `Carried`) per
/// character given.
fn seated_on(world: &World, e: Entity) -> (Option<Seated>, bool) {
    (world.get::<Seated>(e).copied(), world.get::<Carried>(e).is_some())
}

#[test]
fn a_warthog_in_a_network_game() {
    let Some(mount) = mount() else { return };
    let map = load(&mount, "bloodgulch", false);
    let link = LinkConditions {
        latency: Duration::from_millis(50),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(HaloSim, link, 7, 2, move |app| {
        app.add_plugins(MapSimPlugin::new(map.clone()));
    });
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(900);
    sim.until_looks_taken(300);
    sim.ticks(60);
    let (w, prop) = warthog(sim.server.app.world_mut());
    let driver = sim.character_of(0).unwrap();
    let passenger = sim.character_of(1).unwrap();

    // Walk-up: each player put beside its seat, then the use key.
    let at = seat_at(sim.server.app.world(), w, 0);
    teleport(sim.server.app.world_mut(), driver, at + Vec3::Y * 0.1);
    sim.ticks(20);
    press_use(&mut sim, 0);
    let at = seat_at(sim.server.app.world(), w, 1);
    teleport(sim.server.app.world_mut(), passenger, at + Vec3::Y * 0.1);
    sim.ticks(20);
    press_use(&mut sim, 1);
    sim.ticks(30);
    let s = sim.server.app.world();
    assert_eq!(
        s.get::<Seated>(driver).map(|s| (s.vehicle, s.seat)),
        Some((w, 0)),
        "the driver seated on the server"
    );
    assert_eq!(
        s.get::<Seated>(passenger).map(|s| (s.vehicle, s.seat)),
        Some((w, 1)),
        "the passenger seated"
    );
    assert_eq!(
        s.get::<HaloVehicle>(w).unwrap().riders[..2].to_vec(),
        vec![Some(driver), Some(passenger)]
    );
    assert!(s.get::<NetSeat>(driver).is_some() && s.get::<Carried>(driver).is_some());

    // Every machine knows who sits where.
    let mut riders = [Entity::PLACEHOLDER; 2];
    for c in 0..2 {
        let me = sim.local_player(c).unwrap();
        let wc = by_prop(sim.clients[c].app.world_mut(), prop);
        let world = sim.clients[c].app.world();
        let (seat, carried) = seated_on(world, me);
        assert_eq!(
            seat.map(|s| (s.vehicle, s.seat)),
            Some((wc, c)),
            "client {c}: its own player seated"
        );
        assert!(carried, "client {c}: its own player carried (its movement suspended)");
        riders[c] = me;
    }
    // The other's character, as each client sees it.
    for c in 0..2 {
        let world = sim.clients[c].app.world_mut();
        let mut q = world.query_filtered::<(Entity, &Seated), Without<chimera_core::core::LocalPlayer>>();
        let others: Vec<_> = q.iter(world).map(|(_, s)| s.seat).collect();
        assert_eq!(others, vec![1 - c], "client {c} sees the other seated");
    }

    // Drive: forward for four seconds.
    *sim.clients[0].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
    let start_server = sim.server.app.world().get::<Transform>(w).unwrap().translation;
    let wc1 = by_prop(sim.clients[1].app.world_mut(), prop);
    let start_seen = sim.clients[1].app.world().get::<Transform>(wc1).unwrap().translation;
    let mut worst_passenger: f32 = 0.0;
    let mut first: Option<Vec3> = None;
    let mut views = 0;
    for k in 0..120 {
        let me = riders[0];
        sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().move_axis = Vec2::Y;
        sim.step();
        if sim.clients[0].app.world().get::<MapView>(me).is_some() {
            views += 1;
        }
        if k > 30 {
            // The passenger's own player over its seat's marker on the
            // Warthog it draws (the marker from the drawn pose: upright
            // over it, at the same height all along).
            let world = sim.clients[1].app.world_mut();
            let wt = *world.get::<Transform>(wc1).unwrap();
            let v = world.get::<HaloVehicle>(wc1).unwrap().clone();
            let (d, phys) = &world.resource::<VehicleDefs>().0[v.def];
            let mut b = v.drive.body;
            halo_ce::game::vehicles::pose_from(&mut b, &wt, phys.center_of_mass);
            let mut drawn = v.clone();
            drawn.drive.body = b;
            let seat = level::to_engine(drawn.world(phys, drawn.seat_marker(d, 1)));
            let me = world.get::<Transform>(riders[1]).unwrap().translation;
            let over = me - seat;
            let first = *first.get_or_insert(over);
            worst_passenger = worst_passenger.max(over.distance(first));
        }
    }
    sim.clients[0]
        .app
        .world_mut()
        .get_mut::<Intent>(riders[0])
        .unwrap()
        .move_axis = Vec2::ZERO;
    let moved_server = sim
        .server
        .app
        .world()
        .get::<Transform>(w)
        .unwrap()
        .translation
        .distance(start_server);
    let moved_seen = sim.clients[1]
        .app
        .world()
        .get::<Transform>(wc1)
        .unwrap()
        .translation
        .distance(start_seen);
    let g = sim.clients[0].app.world().resource::<NetGraph>().clone();
    eprintln!(
        "server moved {moved_server} m, client 1 saw {moved_seen} m; driver's view {views}/120 ticks; \
         passenger off its drawn seat by at most {worst_passenger} m; driver's prediction: {} checked, {} errors, {} resyncs, {} taken as they came ({:?})",
        g.checked, g.errors, g.resyncs, g.taken_carried, g.last_differing
    );
    assert!(moved_server > 10.0, "the Warthog drove {moved_server} m");
    assert!(
        moved_seen > moved_server * 0.7,
        "the other client saw it move {moved_seen} m"
    );
    assert_eq!(views, 120, "the driver's client views through the seat camera");
    assert_eq!(
        g.errors, 0,
        "the driver's prediction fought the server ({:?})",
        g.last_differing
    );
    assert!(worst_passenger < 0.01, "the passenger {worst_passenger} m off its seat");

    // The driver gets out: no seat left anywhere.
    sim.ticks(60);
    press_use(&mut sim, 0);
    sim.ticks(30);
    let s = sim.server.app.world();
    assert!(
        seated_on(s, driver) == (None::<Seated>, false) && s.get::<NetSeat>(driver).is_none(),
        "the driver out on the server"
    );
    assert_eq!(s.get::<HaloVehicle>(w).unwrap().riders[0], None);
    let world = sim.clients[0].app.world();
    assert_eq!(
        seated_on(world, riders[0]),
        (None::<Seated>, false),
        "the driver out on its client"
    );
    let wc0 = by_prop(sim.clients[0].app.world_mut(), prop);
    assert_eq!(
        sim.clients[0].app.world().get::<HaloVehicle>(wc0).unwrap().riders[0],
        None
    );
    // Its movement is its own again: it walks.
    let before = sim.server.app.world().get::<Transform>(driver).unwrap().translation;
    sim.clients[0]
        .app
        .world_mut()
        .get_mut::<Intent>(riders[0])
        .unwrap()
        .move_axis = Vec2::Y;
    sim.ticks(30);
    let walked = sim
        .server
        .app
        .world()
        .get::<Transform>(driver)
        .unwrap()
        .translation
        .distance(before);
    assert!(walked > 1.0, "walked {walked} m after getting out");

    // The passenger's client leaves: its seat empties.
    sim.remove_client(1);
    sim.ticks(30);
    let s = sim.server.app.world();
    assert_eq!(
        s.get::<HaloVehicle>(w).unwrap().riders[1],
        None,
        "the leaver's seat empties"
    );
    let _ = space::point;
}
