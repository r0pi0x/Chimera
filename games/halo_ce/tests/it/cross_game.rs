//! Halo's damage across games (docs/DESIGN.md "Hits across games";
//! `game::damage`): another game's gun (the engine's test kit) on a Halo
//! character takes the shield first and only the overflow reaches
//! health, restarting the shield delay; a Halo damage event on another
//! game's character is an unshielded Spartan's body hit, normalized (a
//! Halo point 1/75 of its health), through that character's own armour.

use bevy::prelude::*;
use chimera_core::core::{BodyRegion, Health};
use chimera_sim::{
    movement::placeholder,
    testkit::{self, PISTOL},
    weapon::Armor,
};
use chimera_world::{greybox, harness::Sim};
use halo_ce::game::{
    damage::HaloHit,
    vitality::{DamageEffects, Vitality, ids},
};

use super::vitality::{sim, spartan_at, vit};

fn health(sim: &Sim, e: Entity) -> f32 {
    sim.app.world().get::<Health>(e).unwrap().current
}

/// `shots` of the kit's pistol (20 points of 100 a hit, no armour ratio)
/// at the chest of a Halo character 5 m ahead.
fn kit_pistol_on_a_spartan(shots: u32) -> (Sim, Entity) {
    let mut sim = sim();
    testkit::add_weapons(&mut sim.app);
    let shooter = testkit::with_player(&mut sim, None, PISTOL);
    let t = spartan_at(&mut sim, greybox::SPAWNS[0] - Vec3::Z * 5.0);
    for _ in 0..shots {
        let point = sim.position(t) + Vec3::Y * (1.8 * 0.72 - 0.9);
        testkit::aim_at(&mut sim, shooter, point);
        sim.intent(shooter).fire = true;
        sim.ticks(1);
        sim.intent(shooter).fire = false;
        sim.seconds(0.6);
    }
    (sim, t)
}

#[test]
fn another_game_s_bullet_takes_the_shield_first() {
    let (sim, t) = kit_pistol_on_a_spartan(1);
    let v = vit(&sim, t);
    assert_eq!(health(&sim, t), 1.0, "the shield took it all");
    assert!((v.shield - 0.8).abs() < 1e-5, "shield {}", v.shield);
    assert_eq!(sim.app.world().get::<Armor>(t).unwrap().amount, v.shield);
    assert!(v.delay > 150 && v.delay < 180, "the delay restarted: {}", v.delay);
    // Six shots: five empty the shield, the sixth reaches the body.
    let (sim, t) = kit_pistol_on_a_spartan(6);
    let v = vit(&sim, t);
    assert_eq!(v.shield, 0.0);
    assert!((v.body - 0.8).abs() < 1e-4, "body {}", v.body);
    assert!((health(&sim, t) - 0.8).abs() < 1e-4);
}

/// A non-Halo character (the placeholder movement) at the greybox, with
/// CS:S's kevlar when `kevlar`.
fn other(sim: &mut Sim, kevlar: bool) -> Entity {
    let e = sim.spawn_character(greybox::SPAWNS[0], placeholder::ID);
    if kevlar {
        sim.app.world_mut().entity_mut(e).insert(Armor { amount: 1.0, helmet: false });
    }
    sim.ticks(2);
    assert!(sim.app.world().get::<Vitality>(e).is_none());
    e
}

fn pistol(sim: &mut Sim, e: Entity, region: BodyRegion) {
    let fx = sim.app.world().resource::<DamageEffects>().clone();
    let amount = fx.get(ids::PISTOL_BULLET).unwrap().amount(1.0, 0.0);
    sim.app
        .world_mut()
        .write_message(HaloHit::direct(e, ids::PISTOL_BULLET, amount, 1.0, Some(region), Vec3::ZERO, Vec3::NEG_Z, None));
    sim.ticks(1);
}

#[test]
fn a_halo_hit_on_another_game_s_character() {
    // 25 x 1.5 (cyborg) of 75: half its health.
    let mut sim = sim();
    let e = other(&mut sim, false);
    pistol(&mut sim, e, BodyRegion::Chest);
    assert!((health(&sim, e) - 0.5).abs() < 1e-6, "{}", health(&sim, e));
    // The head with no armour: a headshot kills.
    let mut sim = self::sim();
    let e = other(&mut sim, false);
    pistol(&mut sim, e, BodyRegion::Head);
    assert_eq!(health(&sim, e), 0.0);
    // Through kevlar (its own rule, ratio 1): half to health, a quarter
    // of the half from the armour.
    let mut sim = self::sim();
    let e = other(&mut sim, true);
    pistol(&mut sim, e, BodyRegion::Chest);
    assert!((health(&sim, e) - 0.75).abs() < 1e-6, "{}", health(&sim, e));
    assert!((sim.app.world().get::<Armor>(e).unwrap().amount - 0.875).abs() < 1e-6);
}
