//! Items drawn late (`net::late`, specs/tf2/effects.md 11, 11.1): on the
//! server's own view (a listen host's, the sandbox's) a flyer shown as a
//! late key isn't drawn where it is; its copy starts on its launch line
//! behind its first place (the history seeded one second back along its
//! launch velocity) and moves from its first frame, never held, then
//! follows a delay behind (0.1 s in whole ticks plus one); its model shows
//! after its key's hidden time (TF2's rocket 0.2 s; a key with none at
//! once).

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
    // Behind its first place on its launch line, by about the launch
    // speed times the delay.
    let (first, vis) = copy_of(&mut sim, rocket).expect("a copy");
    assert!(first.x < start.x - 0.5 * 30.0 * delay, "behind the launch point: {first}");
    assert!((first.y - start.y).abs() < 1e-4 && (first.z - start.z).abs() < 1e-4, "on its line: {first}");
    assert_eq!(vis, Visibility::Hidden, "the rocket's model hidden at first");
    let (_, vis) = copy_of(&mut sim, pipe).unwrap();
    assert_ne!(vis, Visibility::Hidden, "a key with no hidden time shows at once");
    // Moving from its first frame on, never held.
    let mut last = first.x;
    for _ in 0..((0.08 / tick) as u64) {
        sim.ticks(1);
        let (at, _) = copy_of(&mut sim, rocket).unwrap();
        assert!(at.x > last, "moving: {at} after {last}");
        last = at.x;
    }
    // Then a delay behind: 30 m/s x the delay behind the item.
    sim.ticks((0.25 / tick) as u64);
    let item = sim.server.app.world().get::<Transform>(rocket).unwrap().translation;
    let (at, vis) = copy_of(&mut sim, rocket).unwrap();
    assert!(at.x < item.x, "{at} behind {item}");
    assert!((item.x - at.x - 30.0 * delay).abs() < 0.6, "a delay behind: {} m", item.x - at.x);
    assert_ne!(vis, Visibility::Hidden, "shown after 0.2 s");
}
