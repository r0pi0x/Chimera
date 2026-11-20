//! Items drawn late (`net::late`, specs/tf2/effects.md 11, 11.2): on the
//! server's own view (a listen host's, the sandbox's) a flyer shown as a
//! late key isn't drawn where it is; its copy is drawn at its first place
//! while the drawn time is before its arrival, then follows a delay behind
//! (0.1 s in whole ticks plus one); its model shows after its key's hidden
//! time (TF2's rocket 0.2 s; a key with none at once).

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::{core::SimSet, slots::Loadout};
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_sim::{SimBasePlugins, movement::placeholder};
use chimera_world::{
    greybox::GreyboxMapPlugin,
    map::loose::{DrawnLate, LateCopy, ShownItem},
};

/// Flies along +X at 30 m/s once made.
#[derive(Component)]
struct Fly;

fn fly(mut q: Query<&mut Transform, With<Fly>>, time: Res<Time>) {
    for mut t in &mut q {
        t.translation.x += 30.0 * time.delta_secs();
    }
}

fn setup(app: &mut App) {
    app.add_plugins(GreyboxMapPlugin)
        .insert_resource(Loadout { movement: placeholder::ID })
        .add_systems(FixedUpdate, fly.after(SimSet::Movement));
}

#[test]
fn a_late_item_starts_behind_its_launch_moving_then_follows_a_delay_behind() {
    let link = LinkConditions { latency: Duration::from_millis(20), jitter: Duration::ZERO, loss: 0.0 };
    let mut sim = NetSim::new(SimBasePlugins, link, 7, 1, setup);
    sim.until_joined(600);
    let w = sim.server.app.world_mut();
    {
        let mut late = w.resource_mut::<DrawnLate>();
        late.add("test:rocket".into(), 0.2);
        late.add("test:pipe".into(), 0.0);
    }
    let start = Vec3::new(1.0, 2.0, 3.0);
    let launch = chimera_net::net::NetLaunch { velocity: [30.0, 0.0, 0.0] };
    let rocket = w.spawn((ShownItem("test:rocket".into()), Transform::from_translation(start), Fly, launch)).id();
    let pipe = w.spawn((ShownItem("test:pipe".into()), Transform::from_translation(start), Fly)).id();
    let copy_of = |sim: &mut NetSim, of: Entity| {
        let w = sim.server.app.world_mut();
        let mut q = w.query::<(&LateCopy, &Transform, &Visibility)>();
        q.iter(w).find(|(c, ..)| c.of == of).map(|(_, t, v)| (t.translation, *v))
    };
    sim.ticks(2);
    let tick = sim.server.app.world().resource::<Time<Fixed>>().timestep().as_secs_f32();
    let delay = sim.server.app.world().resource::<chimera_world::map::loose::LateDelay>().0;
    assert!((delay - ((0.1 / tick).round() + 1.0) * tick).abs() < 1e-4, "whole ticks plus one: {delay}");
    // At its first place (where TF2 makes the trail that follows it),
    // held there while the drawn time is before its arrival.
    let (first, vis) = copy_of(&mut sim, rocket).expect("a copy");
    {
        let w = sim.server.app.world_mut();
        let mut q = w.query::<&LateCopy>();
        let c = q.iter(w).find(|c| c.of == rocket).unwrap();
        assert!(c.first.x >= start.x && c.first.x - start.x < 30.0 * 3.0 * tick, "its first place: {}", c.first);
        assert_eq!(first, c.first, "drawn there");
    }
    assert_eq!(vis, Visibility::Hidden, "the rocket's model hidden at first");
    let (_, vis) = copy_of(&mut sim, pipe).unwrap();
    assert_ne!(vis, Visibility::Hidden, "a key with no hidden time shows at once");
    sim.ticks(((delay - 2.0 * tick) / tick).floor() as u64);
    assert_eq!(copy_of(&mut sim, rocket).unwrap().0, first, "held until the drawn time reaches its arrival");
    // Then a delay behind: 30 m/s x the delay behind the item.
    sim.ticks((0.25 / tick) as u64);
    let item = sim.server.app.world().get::<Transform>(rocket).unwrap().translation;
    let (at, vis) = copy_of(&mut sim, rocket).unwrap();
    assert!(at.x < item.x, "{at} behind {item}");
    assert!((item.x - at.x - 30.0 * delay).abs() < 0.6, "a delay behind: {} m", item.x - at.x);
    assert_ne!(vis, Visibility::Hidden, "shown after 0.2 s");
}

/// effects.md 11.2: on the server's own view a flyer's copy comes as its
/// first snapshot would carry it (20 a second on the server's tick count),
/// its first place in whole units when its key asks.
#[test]
fn a_late_flyers_copy_comes_with_the_next_snapshot_in_whole_units() {
    let link = LinkConditions { latency: Duration::from_millis(20), jitter: Duration::ZERO, loss: 0.0 };
    let mut sim = NetSim::new(SimBasePlugins, link, 7, 1, setup);
    sim.until_joined(600);
    let w = sim.server.app.world_mut();
    {
        let mut late = w.resource_mut::<DrawnLate>();
        late.add("test:rocket".into(), 0.2);
        late.set_unit("test:rocket", 0.25);
    }
    let clock = *w.resource::<chimera_core::core::SimClock>();
    let tick = w.resource::<chimera_core::core::SimTick>().0;
    let dt = w.resource::<Time<Fixed>>().timestep().as_secs_f64();
    let flyer = chimera_sim::weapon::projectile::Flyer::new("test:gun", None, Vec3::X * 30.0, &clock);
    let born = flyer.born_tick;
    let rocket = w.spawn((ShownItem("test:rocket".into()), Transform::from_translation(Vec3::new(1.1, 2.0, 3.0)), Fly, flyer)).id();
    let snapshot = |t: u64| (t as f64 * dt * 20.0 + 1e-6).floor() as u64;
    let mut seen = None;
    for _ in 0..8 {
        sim.ticks(1);
        let w = sim.server.app.world_mut();
        let now = w.resource::<chimera_core::core::SimTick>().0;
        let mut q = w.query::<&LateCopy>();
        if let Some(c) = q.iter(w).find(|c| c.of == rocket) {
            seen = Some((now, c.first));
            break;
        }
        assert_eq!(snapshot(now), snapshot(born.max(tick)), "no copy before the next snapshot (tick {now})");
    }
    let (at, first) = seen.expect("a copy within a snapshot");
    assert!(snapshot(at) > snapshot(born.max(tick)), "at {at} born {born} tick {tick}");
    for v in first.to_array() {
        assert!((v / 0.25 - (v / 0.25).round()).abs() < 1e-4, "whole units: {first}");
    }
}
