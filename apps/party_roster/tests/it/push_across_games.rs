//! A TF2 gun's knockback on another game's body (`core::ExternalPush`,
//! specs/tf2/weapons.md 5.4.1): a Scattergun's push (60 points at scale 6:
//! 360 units/s) on a Combat Arms body and on a Halo body, both standing,
//! is cut at the body's first tick on the ground to 0.9 x its own max
//! speed (Halo's forward run speed: exactly that, where it used to ease
//! the push off over seconds; Combat Arms' speed limit, before its own
//! friction), and its small upward part (under TF2's 250) no longer lifts
//! the Combat Arms body off the ground.

use bevy::prelude::*;
use chimera_core::{
    core::{Damage, DamageKind, Hitgroup, Velocity},
    slots::GameIdentity,
};
use chimera_sim::weapon::{BulletHit, Inventory};
use chimera_world::harness::Sim;

use crate::play::{line_up, set_rung, soldier};

/// The guns: TF2's Scattergun (the shooter's), a body of each game to hit.
const GUNS: [&str; 3] = ["tf2:tf_weapon_scattergun", "combat_arms:m4a1", "halo_ce:assault_rifle"];

fn party() -> Sim {
    let text = crate::play::ladder_text(&GUNS, false).replace(
        "weapons = [\"cs_source\", \"hl2dm\", \"combat_arms\"]",
        "weapons = [\"cs_source\", \"hl2dm\", \"tf2\", \"halo_ce\", \"combat_arms\"]",
    );
    crate::halo_party::party_with(&text)
}

fn flat(sim: &Sim, e: Entity) -> f32 {
    let v = sim.app.world().get::<Velocity>(e).unwrap().0;
    Vec2::new(v.x, v.z).length()
}

/// `a`'s Scattergun hits `b` for 60 points (a whole shot at 512 units):
/// its push, then `b`'s horizontal speed (m/s) and footing each tick after.
fn shot(sim: &mut Sim, a: Entity, b: Entity) -> Vec<(f32, bool)> {
    let weapon = sim.app.world().get::<Inventory>(a).and_then(|i| i.active).expect("a gun in hand");
    let at = sim.position(b);
    let dir = (at - sim.position(a)).normalize();
    sim.app.world_mut().write_message(BulletHit {
        damage: Damage {
            target: b,
            attacker: Some(a),
            amount: 0.6,
            point: at,
            dir,
            hitgroup: Hitgroup::Chest,
            kind: DamageKind::Bullet,
            weapon: Some(GUNS[0]),
            force: Vec3::ZERO,
        },
        weapon,
        raw: 0.6,
        covered: false,
        helmet: false,
    });
    (0..6)
        .map(|_| {
            sim.ticks(1);
            (flat(sim, b), sim.state(b).on_ground)
        })
        .collect()
}

#[test]
fn a_tf2_push_on_combat_arms_and_halo_bodies_meets_their_ground_limit() {
    let mut sim = party();
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.5);
    set_rung(&mut sim, a, 0);
    for (rung, game) in [(1, "combat_arms"), (2, "halo_ce")] {
        set_rung(&mut sim, b, rung);
        sim.seconds(1.2);
        line_up(&mut sim, a, b, 3.0);
        let w = sim.app.world();
        assert_eq!(w.get::<GameIdentity>(b).map(|g| g.0.as_str()), Some(game), "the victim is {game}'s");
        assert!(sim.state(b).on_ground, "{game}: standing");
        // Its own max speed (m/s).
        let max = match game {
            "combat_arms" => {
                let d = w.resource::<combat_arms::game::movement::MovementData>();
                w.get::<combat_arms::game::movement::CaMovement>(b).unwrap().speed_limit * d.meters_per_unit
            }
            _ => w.resource::<halo_ce::game::movement::data::MovementData>().run_forward * halo_ce::game::METERS_PER_UNIT,
        };
        let ticks = shot(&mut sim, a, b);
        let speeds: Vec<f32> = ticks.iter().map(|t| t.0).collect();
        let push = speeds.iter().cloned().fold(0.0, f32::max);
        let i = speeds.iter().position(|&v| v == push).unwrap();
        println!("{game}: max {max}, speeds {speeds:?}");
        // 360 units/s (9.1 m/s) added, more than either can run.
        assert!(push > 8.5 && push > max, "{game}: pushed at {push}");
        let first = speeds[i + 1];
        if game == "halo_ce" {
            assert!((first - 0.9 * max).abs() < 0.01 * max, "{game}: first tick on the ground at {first}, limit {}", 0.9 * max);
        } else {
            assert!(first > 0.0 && first <= 0.9 * max * 1.01, "{game}: first tick on the ground at {first}, limit {}", 0.9 * max);
        }
        assert!(speeds[i + 1..].iter().all(|&v| v <= 0.9 * max * 1.01), "{game}: never past it after: {speeds:?}");
        assert!(ticks.iter().all(|t| t.1), "{game}: a push under 250 up leaves it standing: {ticks:?}");
    }
}
