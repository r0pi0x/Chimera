//! Halo's damage across games (docs/DESIGN.md "Hits across games";
//! `game::damage`, `game::cross_game`): another game's gun (the engine's
//! test kit) on a Halo character takes the shield first and only the
//! overflow reaches health, restarting the shield delay, counted the
//! cross-game table's incoming (`party`: the party's numbers, the switch
//! on or off; a campaign body's own entry); a Halo damage event on
//! another game's character is an unshielded Spartan's body hit,
//! normalized (a Halo point 1/75 of its health), through that
//! character's own armour.

use bevy::prelude::*;
use chimera_core::core::{BodyRegion, Health};
use chimera_sim::{
    movement::placeholder,
    testkit::{self, PISTOL},
    weapon::Armor,
};
use chimera_world::{greybox, harness::Sim};
use chimera_core::core::DamageScale;
use chimera_sim::weapon::balance::Balance;
use halo_ce::game::{
    damage::HaloHit,
    vitality::{DamageEffects, Vitality, ids},
};

use super::vitality::{sim, spartan_at, vit};

fn health(sim: &Sim, e: Entity) -> f32 {
    sim.app.world().get::<Health>(e).unwrap().current
}

/// The cross-game table with the party's Halo numbers (party_kit's
/// data/party_balance.toml: outgoing 0.75, incoming 1.5), the switch on,
/// else a raw match.
pub(crate) fn party(on: bool) -> Balance {
    let t = Balance::parse("[games.halo_ce]
outgoing = 0.75
incoming = 1.5
").unwrap();
    if on { t } else { t.raw() }
}

/// `shots` of the kit's pistol (20 points of 100 a hit, no armour ratio)
/// at the chest of a Halo character 5 m ahead, with the table `cfg`.
fn kit_pistol_on_a_spartan(shots: u32, cfg: Balance) -> (Sim, Entity) {
    let mut sim = sim();
    sim.app.insert_resource(cfg);
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
    // A raw match: the hit as it comes.
    let off = || party(false);
    let (sim, t) = kit_pistol_on_a_spartan(1, off());
    let v = vit(&sim, t);
    assert_eq!(health(&sim, t), 1.0, "the shield took it all");
    assert!((v.shield - 0.8).abs() < 1e-5, "shield {}", v.shield);
    assert_eq!(sim.app.world().get::<Armor>(t).unwrap().amount, v.shield);
    assert!(v.delay > 150 && v.delay < 180, "the delay restarted: {}", v.delay);
    // Six shots: five empty the shield, the sixth reaches the body.
    let (sim, t) = kit_pistol_on_a_spartan(6, off());
    let v = vit(&sim, t);
    assert_eq!(v.shield, 0.0);
    assert!((v.body - 0.8).abs() < 1e-4, "body {}", v.body);
    assert!((health(&sim, t) - 0.8).abs() < 1e-4);
    assert!(sim.app.world().get::<DamageScale>(t).is_none(), "off: no scale");
}

/// The party's table on (incoming 1.5): each 20-point
/// round counts 30 against the shield, then the body, the shield still
/// first; the shield's armour is in that unit (its fraction / 1.5), and
/// it recharges by Halo's rule.
#[test]
fn another_game_s_bullet_counts_the_incoming_scale() {
    let on = || party(true);
    assert_eq!(halo_ce::game::cross_game::incoming_of(Some(&on()), "halo_ce:spartan"), 1.5);
    let (sim, t) = kit_pistol_on_a_spartan(1, on());
    let v = vit(&sim, t);
    assert_eq!(health(&sim, t), 1.0, "the shield took it all");
    assert!((v.shield - 0.7).abs() < 0.016, "shield {}", v.shield);
    let armor = sim.app.world().get::<Armor>(t).unwrap().amount;
    assert!((armor * 1.5 - v.shield).abs() < 1e-5, "the shield's unit: {armor}");
    assert_eq!(sim.app.world().get::<DamageScale>(t), Some(&DamageScale(1.5)));
    // 4 rounds break it (120 counted); 4, 5, 6 take 20, 30, 30 of the body.
    for (shots, body) in [(3, 1.0), (4, 0.8), (5, 0.5), (6, 0.2)] {
        let (sim, t) = kit_pistol_on_a_spartan(shots, on());
        let v = vit(&sim, t);
        assert!((v.body - body).abs() < 0.016, "{shots}: body {}", v.body);
        assert!((health(&sim, t) - v.body).abs() < 1e-5);
        assert_eq!(v.shield == 0.0, shots >= 4, "{shots}: shield {}", v.shield);
    }
    // Seven kill (210 counted of 200); unscaled, seven would leave 60.
    let (sim, t) = kit_pistol_on_a_spartan(7, on());
    assert!(vit(&sim, t).dead && health(&sim, t) == 0.0);
    // The shield comes back by Halo's rule: full again 10 s later.
    let (mut sim, t) = kit_pistol_on_a_spartan(2, on());
    sim.seconds(10.5);
    let v = vit(&sim, t);
    assert_eq!(v.shield, 1.0);
    assert!((sim.app.world().get::<Armor>(t).unwrap().amount - 1.0 / 1.5).abs() < 1e-6);
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

/// A body's own incoming (`[bodies]`, by the sandbox's id) over Halo's:
/// the Spartan's at 2 makes a 20-point round count 40; the switch put
/// off at once converts the shield's unit, its value kept.
#[test]
fn a_body_s_own_incoming_and_a_change_keep_the_shield() {
    let mut t = party(true);
    t.bodies.insert("halo_ce:spartan".into(), 2.0);
    let (mut sim, e) = kit_pistol_on_a_spartan(1, t);
    let v = vit(&sim, e);
    assert!((v.shield - 0.6).abs() < 0.016, "shield {}", v.shield);
    assert_eq!(sim.app.world().get::<DamageScale>(e), Some(&DamageScale(2.0)));
    let shield = v.shield;
    sim.app.world_mut().resource_mut::<Balance>().on = false;
    sim.ticks(2);
    assert!(sim.app.world().get::<DamageScale>(e).is_none(), "raw: no scale");
    assert!((vit(&sim, e).shield - shield).abs() < 1e-5, "the shield's value kept");
    assert!((sim.app.world().get::<Armor>(e).unwrap().amount - shield).abs() < 1e-5, "in the raw unit");
}
