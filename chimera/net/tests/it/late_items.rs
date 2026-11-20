//! Items drawn late (`net::late`, specs/tf2/effects.md 11 and "Measured"):
//! on the server's own view (a listen host's, the sandbox's) a flyer
//! shown as a late key isn't drawn where it is; its copy is held at the
//! launch point until the drawn time (now less the delay, 0.1 s) reaches
//! it, then follows a delay behind, never behind the launch point; its
//! model shows after its key's hidden time (TF2's rocket 0.2 s; a key with
//! none at once).

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
fn a_late_item_holds_at_its_launch_then_follows_a_delay_behind() {
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
    let rocket = w.spawn((ShownItem("test:rocket".into()), Transform::from_translation(start), Fly)).id();
    let pipe = w.spawn((ShownItem("test:pipe".into()), Transform::from_translation(start), Fly)).id();
    let copy_of = |sim: &mut NetSim, of: Entity| {
        let w = sim.server.app.world_mut();
        let mut q = w.query::<(&LateCopy, &Transform, &Visibility)>();
        q.iter(w).find(|(c, ..)| c.of == of).map(|(_, t, v)| (t.translation, *v))
    };
    sim.ticks(2);
    // Its first place is where the item first was when frames drew it.
    let (first, vis) = copy_of(&mut sim, rocket).expect("a copy");
    assert!(first.x >= start.x && first.x < start.x + 1.0, "at the launch point, not behind it: {first}");
    assert_eq!(vis, Visibility::Hidden, "the rocket's model hidden at first");
    let (_, vis) = copy_of(&mut sim, pipe).unwrap();
    assert_ne!(vis, Visibility::Hidden, "a key with no hidden time shows at once");
    // Still held while the drawn time is before the launch.
    let tick = sim.server.app.world().resource::<Time<Fixed>>().timestep().as_secs_f32();
    let held = (0.08 / tick) as u64;
    sim.ticks(held.saturating_sub(2));
    let (at, _) = copy_of(&mut sim, rocket).unwrap();
    assert_eq!(at, first, "held at the launch");
    // Then a delay behind: 30 m/s x 0.1 s = 3 m behind the item.
    sim.ticks((0.25 / tick) as u64);
    let item = sim.server.app.world().get::<Transform>(rocket).unwrap().translation;
    let (at, vis) = copy_of(&mut sim, rocket).unwrap();
    assert!(at.x > first.x && at.x < item.x, "{at} behind {item}");
    assert!((item.x - at.x - 3.0).abs() < 0.6, "a delay behind: {} m", item.x - at.x);
    assert_ne!(vis, Visibility::Hidden, "shown after 0.2 s");
}
