//! What a hurt HL2:DM player feels (specs/hl2dm/low_health.md 3): the
//! damage view punch. (The explosion hearing effect's cases are
//! `blast_rules::a_blast_rings_or_shocks_the_ears_near_it`; the side
//! shapes of the damage indicator are `tests/ui/damage_and_death.rs`.)

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Hitgroup};
use chimera_sim::weapon::ViewPunch;
use hl2dm::game::weapons::recoil::{DAMAGE_PITCH, damaged_punch, from_view, to_view};

use super::blast_rules::{floor, player, start};

fn hurt(sim: &mut chimera_world::harness::Sim, p: Entity, amount: f32) {
    let at = sim.app.world().get::<Transform>(p).unwrap().translation;
    sim.app.world_mut().write_message(Damage {
        target: p,
        attacker: None,
        amount,
        point: at,
        dir: Vec3::ZERO,
        hitgroup: Hitgroup::Generic,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
}

fn punch(sim: &chimera_world::harness::Sim, p: Entity) -> Vec3 {
    from_view(sim.app.world().get::<ViewPunch>(p).unwrap().0)
}

/// The punch's pitch is set, not added: replacing recoil's, keeping yaw
/// and roll; any amount; two hits in a tick do not build up.
#[test]
fn damage_sets_the_punch_pitch_to_minus_two_degrees() {
    let mut sim = start(floor());
    sim.seconds(0.3);
    let p = player(&mut sim, Vec3::new(0.0, 0.0, 0.0));
    sim.ticks(2);
    // Recoil had kicked it up 6 degrees and to the side.
    sim.app.world_mut().get_mut::<ViewPunch>(p).unwrap().0 = to_view(Vec3::new(-6.0, 3.0, 1.5));
    hurt(&mut sim, p, 0.01);
    sim.ticks(1);
    let v = punch(&sim, p);
    assert!((v.x - DAMAGE_PITCH).abs() < 1e-3, "pitch {v}");
    assert!((v.y - 3.0).abs() < 1e-3 && (v.z - 1.5).abs() < 1e-3, "yaw and roll kept: {v}");
    // A hit during heavy recoil reduces the kick (the quirk).
    sim.app.world_mut().get_mut::<ViewPunch>(p).unwrap().0 = to_view(Vec3::new(-12.0, 0.0, 0.0));
    hurt(&mut sim, p, 0.3);
    hurt(&mut sim, p, 0.3);
    sim.ticks(1);
    assert!((punch(&sim, p).x - DAMAGE_PITCH).abs() < 1e-3);
}

#[test]
fn the_punch_value_is_minus_two_in_source_degrees_and_up_in_the_view() {
    let v = damaged_punch(Vec3::new(0.2, 0.1, 0.05));
    assert!((from_view(v).x + 2.0).abs() < 1e-4);
    // Chimera's pitch is up-positive: a kick up of 2 degrees.
    assert!((v.x - 2f32.to_radians()).abs() < 1e-5);
    assert_eq!((v.y, v.z), (0.1, 0.05));
}
