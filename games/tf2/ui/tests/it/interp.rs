//! TF2's flyers drawn by the client: between ticks, as everything else is
//! (weapons_more.md 8, effects.md 12; the client's `ClientInterpPlugin`).

use bevy::prelude::*;
use chimera_sim::{testkit, weapon::{Inventory, Weapon, give, projectile::Flyer}};
use chimera_world::{greybox, harness::Sim};
use tf2::game::{TICK_INTERVAL, movement, weapons::SYRINGE_GUN};

const U: f32 = 0.0254;

/// A player at `at` holding `id`, drawn and settled.
fn holding(sim: &mut Sim, at: Vec3, id: &str) -> (Entity, Entity) {
    let p = sim.spawn_character(at, movement::ID);
    sim.seconds(1.0);
    let w = give(sim.app.world_mut(), p, id).unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
    sim.seconds(1.0);
    assert_eq!(testkit::active_id(sim.app.world(), p), Some(sim.app.world().get::<Weapon>(w).unwrap().id));
    (p, w)
}

/// Drawn between ticks as everything else is (`map::interp`, the
/// client's `ClientInterpPlugin`: every game's flyers are `Interpolated`),
/// not at their newest tick a tick ahead of the world: at frames of a
/// quarter tick, a needle's first drawn place is its start (16 forward, 6
/// right, 8 down of the eye: weapons_more.md 8, effects.md 12), and in
/// flight it is drawn between its last two ticks by the frame's fraction.
#[test]
fn a_needle_is_drawn_between_its_ticks_from_its_start_8() {
    use std::time::Duration;

    use bevy::time::TimeUpdateStrategy;
    use chimera_client::client::interp::ClientInterpPlugin;
    use chimera_world::{greybox::GreyboxMapPlugin, map::interp::Interpolated};

    let mut sim = Sim::new(tf2::harness::Tf2Sim, (GreyboxMapPlugin, ClientInterpPlugin));
    sim.set_tick_interval(TICK_INTERVAL);
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], SYRINGE_GUN);
    let eye = sim.position(p) + sim.state(p).eye_offset;
    let want = eye + Vec3::NEG_Z * 16.0 * U + Vec3::X * 6.0 * U - Vec3::Y * 8.0 * U;
    sim.app
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(TICK_INTERVAL / 4.0)));
    {
        let mut i = sim.intent(p);
        i.fire = true;
        i.yaw = 0.0;
        i.pitch = 0.0;
    }
    let mut first = None;
    let mut needle = None;
    let mut between = 0;
    for _ in 0..40 {
        sim.app.update();
        let w = sim.app.world_mut();
        let mut q = w.query::<(Entity, &Flyer, &Transform, &Interpolated)>();
        let found = match needle {
            Some(n) => q.get(w, n).ok(),
            None => q.iter(w).find(|(_, f, ..)| f.owner == Some(p)),
        };
        let Some((e, drawn, ticks)) = found.map(|(e, _, t, i)| (e, t.translation, i.ticks())) else {
            continue;
        };
        if needle.replace(e).is_none() {
            // One needle: attack let go once it is out.
            sim.intent(p).fire = false;
        }
        let w = sim.app.world();
        let (prev, cur) = ticks.expect("sampled at its first tick");
        let f = w.resource::<Time<Fixed>>().overstep_fraction();
        match first {
            None => {
                first = Some(drawn);
                assert!((drawn - want).length() < 0.1 * U, "first drawn at {drawn}, want its start {want}");
                assert_eq!(prev.translation, cur.translation, "not moved yet on its first drawn frame");
            }
            Some(_) => {
                let blend = prev.translation.lerp(cur.translation, f);
                assert!((drawn - blend).length() < 1e-4, "drawn {drawn}, want {blend} at {f}");
                if f > 0.1 && f < 0.9 && prev.translation != cur.translation {
                    // Strictly between its two ticks: behind the newest.
                    let (a, b) = (drawn.distance(prev.translation), drawn.distance(cur.translation));
                    assert!(a > 0.0 && b > 0.0, "drawn {drawn} at a tick ({prev:?} .. {cur:?})");
                    assert!((a + b - prev.translation.distance(cur.translation)).abs() < 1e-4);
                    between += 1;
                }
            }
        }
    }
    assert!(first.is_some(), "no needle");
    assert!(between > 10, "{between} frames between ticks");
}

