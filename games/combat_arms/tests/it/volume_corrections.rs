//! specs/combat_arms/world_dat.md "Corrections (2026-10-22)" (1-7) on
//! `CaSim` with the movement tests' map and player: volume damage as
//! lasting damage at `Damage` x the step per second, players only for the
//! plain volumes, `DamageVolume` (heal, team, lock, bots), the damage kind
//! from `DamageType`, `EndlessFall`'s 0.5 s pending grid, the messages
//! `DamageVal` / `VELOCITY`, and the jump pads' world sound. The game's
//! tick here is 15 ms; the server's step (the hand-over's size) is 50 ms.

use bevy::prelude::*;
use chimera_core::core::{Bot, Damage, Health, SimClock, Team};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use combat_arms::{
    game::{
        TICK_INTERVAL,
        effects::sounds::CaSound,
        movement::{self, movement_data},
        objects::CaObjectMessage,
        volumes::{CaVolume, ENDLESS_FALL, PadSound},
        world,
    },
    harness::CaSim,
};
use lithtech_formats::world::{Object, Value};

use crate::{
    movement::{T, close, on},
    movement_volumes::{slab, volume},
    world_objects_play::{cuboid, prop, world_with},
};

const DT: f32 = 0.015;

/// Health lost by `p` so far (HP), from the logged damage.
fn lost(t: &T, p: Entity) -> f32 {
    t.damage()
        .iter()
        .filter(|d: &&Damage| d.target == p)
        .map(|d| d.amount * movement_data().health_points)
        .sum()
}

fn clock(t: &T) -> f64 {
    t.sim.app.world().resource::<SimClock>().now
}

/// 1: lasting damage: `Damage` x 0.05 per second (5: 0.25 HP after 1 s),
/// going on for 0.25 s after leaving.
#[test]
fn c1_volume_damage_is_lasting_and_slow() {
    let mut t = T::standing();
    let (lo, hi) = slab(-10.0, 1000.0);
    volume(&mut t, "PoisonGas", lo, hi, |_| {});
    t.ticks(67);
    close(lost(&t, t.p), 0.25 * 67.0 * DT, 0.003, "after a second inside");
    let inside = lost(&t, t.p);
    t.teleport(on(0.0, 5000.0, 0.0));
    t.ticks(60);
    close(lost(&t, t.p) - inside, 0.0625, 0.006, "the 0.25 s after leaving");
    // A Damage 400 volume: 20 HP per second.
    let mut t = T::standing();
    volume(&mut t, "VolumeBrush", lo, hi, |v| v.damage = 400.0);
    t.ticks(67);
    close(lost(&t, t.p), 20.0 * 67.0 * DT, 0.2, "400 per second at 1/20");
}

/// 1 and 4: an endless fall kills at once, of its own kind; a
/// `VolumeBrush` with `DamageType` `ENDLESS FALL` does the same.
#[test]
fn c1_c4_endless_fall_kind() {
    let (lo, hi) = slab(-10.0, 1000.0);
    for class in ["EndlessFall", "VolumeBrush"] {
        let mut t = T::standing();
        volume(&mut t, class, lo, hi, |v| {
            if class == "VolumeBrush" {
                v.damage = 1.0e8;
                *v = v.clone().typed("endless fall");
            }
        });
        t.tick();
        let d = t.damage();
        let d = d.iter().find(|d| d.target == t.p).expect("damaged");
        assert_eq!(d.weapon, Some(ENDLESS_FALL), "{class}");
        assert!(d.amount >= 1.0, "dead on the first application");
    }
}

/// 2: plain volumes hurt players only.
#[test]
fn c2_bots_take_nothing_from_plain_volumes() {
    let mut t = T::standing();
    let (lo, hi) = slab(-10.0, 1000.0);
    let s = movement_data().meters_per_unit;
    let bot = t.sim.spawn_character(on(100.0, 0.0, 0.0) * s, movement::ID);
    t.sim.app.world_mut().entity_mut(bot).insert(Bot);
    volume(&mut t, "EndlessFall", lo, hi, |_| {});
    t.ticks(5);
    assert!(t.damage().iter().all(|d| d.target != bot), "the bot is untouched");
    assert!(t.damage().iter().any(|d| d.target == t.p), "the player dies");
}

/// 3: `DamageVolume` heals by default (5 x 0.05 per second), filters by
/// team, locks, and takes bots.
#[test]
fn c3_damage_volume_heals() {
    let (lo, hi) = slab(-10.0, 1000.0);
    let hurt = |t: &mut T, e: Entity| {
        *t.sim.app.world_mut().get_mut::<Health>(e).unwrap() = Health { current: 0.5, max: 1.0 };
    };
    let health = |t: &T, e: Entity| t.sim.app.world().get::<Health>(e).unwrap().current;
    let run = |with: Box<dyn FnOnce(&mut CaVolume)>, team: u8| {
        let mut t = T::standing();
        t.insert(Team(team));
        let p = t.p;
        hurt(&mut t, p);
        volume(&mut t, "DamageVolume", lo, hi, with);
        t.ticks(67);
        health(&t, p) - 0.5
    };
    close(
        run(Box::new(|_| {}), 1),
        0.0025 * 67.0 * DT,
        0.0003,
        "a heal of 0.25 HP per second",
    );
    close(
        run(Box::new(|v| v.target_team = Some(2)), 1),
        0.0,
        1e-6,
        "the other team is unaffected",
    );
    assert!(
        run(Box::new(|v| v.target_team = Some(1)), 1) > 0.0,
        "its own team heals"
    );
    close(run(Box::new(|v| v.locked = true), 1), 0.0, 1e-6, "locked");
    let mut t = T::standing();
    let s = movement_data().meters_per_unit;
    let bot = t.sim.spawn_character(on(100.0, 0.0, 0.0) * s, movement::ID);
    t.sim.app.world_mut().entity_mut(bot).insert(Bot);
    hurt(&mut t, bot);
    volume(&mut t, "DamageVolume", lo, hi, |_| {});
    t.ticks(67);
    assert!(health(&t, bot) > 0.5, "bots heal too");
}

/// 5: `PendingSec` is checked on a 0.5 s grid.
#[test]
fn c5_pending_grid() {
    let (lo, hi) = (Vec3::new(-100.0, -10.0, -100.0), Vec3::new(100.0, 1000.0, 100.0));
    let mut t = T::standing();
    let out = on(3000.0, 0.0, 0.0);
    t.teleport(out);
    t.tick();
    let t0 = clock(&t);
    volume(&mut t, "EndlessFall", lo, hi, |v| v.pending = 1.0);
    // Enter at ~0.6 s, out at ~0.8, back at ~0.9.
    while clock(&t) - t0 < 0.6 {
        t.tick();
    }
    t.teleport(on(0.0, 0.0, 0.0));
    while clock(&t) - t0 < 0.8 {
        t.tick();
    }
    t.teleport(out);
    while clock(&t) - t0 < 0.9 {
        t.tick();
    }
    t.teleport(on(0.0, 0.0, 0.0));
    while clock(&t) - t0 < 1.95 {
        t.tick();
        assert!(
            t.damage().iter().all(|d| d.target != t.p),
            "not damaged by {}",
            clock(&t) - t0
        );
    }
    while clock(&t) - t0 < 2.1 {
        t.tick();
    }
    assert!(t.damage().iter().any(|d| d.target == t.p), "damaged once past 2.0 s");
}

/// 6: `DamageVal` and `VELOCITY` on a converted world.
#[test]
fn c6_messages_change_volumes() {
    let s = movement_data().meters_per_unit;
    let w = world_with(
        vec![
            cuboid("Gas", [-100.0, 0.0, -100.0], [100.0, 100.0, 100.0], false),
            cuboid("Pad", [500.0, 0.0, 500.0], [600.0, 50.0, 600.0], false),
        ],
        vec![
            Object {
                class: "VolumeBrush".into(),
                properties: vec![
                    prop("Name", Value::String("Gas".into())),
                    prop("Damage", Value::Real(400.0)),
                ],
            },
            Object {
                class: "JumpVolume".into(),
                properties: vec![
                    prop("Name", Value::String("Pad".into())),
                    prop("Velocity", Value::Real(1000.0)),
                ],
            },
        ],
    );
    let map = world::convert(&w, "gas", s, &|_| None);
    let feet = map.spawns[0].0;
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
    sim.set_tick_interval(TICK_INTERVAL);
    sim.ticks(3);
    let p = sim.spawn_character(feet + Vec3::Y, movement::ID);
    let health = |sim: &Sim| sim.app.world().get::<Health>(p).unwrap().current;
    sim.seconds(1.0);
    close(1.0 - health(&sim), 0.20, 0.03, "20 HP in a second (Damage 400)");
    let msg = |sim: &mut Sim, target: &str, message: &str| {
        sim.app.world_mut().write_message(CaObjectMessage {
            target: target.into(),
            message: message.into(),
        });
    };
    msg(&mut sim, "Gas", "DamageVal 0");
    sim.seconds(0.5);
    let after = health(&sim);
    sim.seconds(1.0);
    assert_eq!(health(&sim), after, "no more damage with DamageVal 0");
    msg(&mut sim, "Pad", "VELOCITY 2000");
    sim.seconds(0.2);
    let w = sim.app.world_mut();
    let speeds: Vec<f32> = w
        .query::<&CaVolume>()
        .iter(w)
        .filter_map(|v| v.jump_velocity)
        .map(|j| j.length())
        .collect();
    assert_eq!(speeds.len(), 1);
    close(speeds[0], 2000.0, 1.0, "the pad's new speed");
}

#[derive(Resource, Default)]
struct Heard(Vec<CaSound>);

/// 7: a launching pad plays its world sound at the launched player.
#[test]
fn c7_pad_sound() {
    let mut t = T::standing();
    t.sim
        .app
        .init_resource::<Heard>()
        .add_systems(Last, |mut r: MessageReader<CaSound>, mut h: ResMut<Heard>| {
            h.0.extend(r.read().cloned())
        });
    let (lo, hi) = slab(-10.0, 2000.0);
    volume(&mut t, "JumpVolume", lo, hi, |v| {
        v.jump_velocity = Some(Vec3::new(0.0, 800.0, 0.0));
        v.pad_sound = Some(PadSound {
            file: "pad.wav".into(),
            inner: 300.0,
            outer: 300.0,
            three_d: true,
        });
    });
    t.tick();
    let heard = t.sim.app.world().resource::<Heard>().0.clone();
    assert_eq!(heard.len(), 1, "{heard:?}");
    let h = &heard[0];
    assert_eq!(h.file, "pad.wav");
    assert!(h.server && h.at.is_some());
    assert_eq!(h.radii, (300.0, 300.0));
}
