//! A Halo player's vitality on `HaloSim` at 30 Hz, through the damage
//! seam (`game::damage::HaloHit`): every test case of
//! specs/halo_ce/health_shields.md ("Test cases") and its shots-to-kill
//! table, with the disc's numbers from data/vitality.toml and
//! data/damage.toml. A hit is written before a tick and applied in it,
//! before the victim's update of that tick (the spec's "tick 0").

// The spec's numbers are written as it states them.
#![allow(clippy::excessive_precision)]

use bevy::prelude::*;
use chimera_core::core::{BodyRegion, Died, Health, Intent};
use chimera_sim::weapon::Armor;
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use halo_ce::{
    game::{
        damage::HaloHit,
        movement,
        vitality::{
            ALIVE_AT_ZERO, DamageEffects, EffectId, GrantPowerup, Powerup, Spartan, Stun, Vitality, ids, melee_scale,
            unsuspecting,
        },
    },
    harness::HaloSim,
};

pub fn sim() -> Sim {
    Sim::new(HaloSim, GreyboxMapPlugin)
}

/// A fresh Spartan on the greybox, looking along -Z (yaw 0).
pub fn spartan(sim: &mut Sim) -> Entity {
    spartan_at(sim, greybox::SPAWNS[0])
}

pub fn spartan_at(sim: &mut Sim, at: Vec3) -> Entity {
    let e = sim.spawn_character(at, movement::ID);
    sim.ticks(2);
    assert!(sim.app.world().get::<Vitality>(e).is_some(), "a Halo character wears its vitality");
    e
}

pub fn vit(sim: &Sim, e: Entity) -> Vitality {
    sim.app.world().get::<Vitality>(e).unwrap().clone()
}

fn health(sim: &Sim, e: Entity) -> f32 {
    sim.app.world().get::<Health>(e).unwrap().current
}

fn effects(sim: &Sim) -> DamageEffects {
    sim.app.world().resource::<DamageEffects>().clone()
}

/// The places the spec's tests name.
#[derive(Clone, Copy)]
enum On {
    Head,
    Body,
    Legs,
    /// No material (attached damage, area).
    None,
}

impl On {
    fn region(self) -> Option<BodyRegion> {
        match self {
            On::Head => Some(BodyRegion::Head),
            On::Body => Some(BodyRegion::Chest),
            On::Legs => Some(BodyRegion::LeftShin),
            On::None => None,
        }
    }
}

/// A direct hit of `effect` with the upper bound's draw `rnd` (scale 1)
/// on `on`, applied in one tick.
fn shoot(sim: &mut Sim, e: Entity, effect: EffectId, rnd: f32, on: On) {
    let amount = effects(sim).get(effect).unwrap().amount_with(1.0, rnd);
    hit(sim, HaloHit::direct(e, effect, amount, 1.0, on.region(), Vec3::ZERO, Vec3::NEG_Z, None));
}

/// An area hit of `amount` at scale 1.
fn blast(sim: &mut Sim, e: Entity, effect: EffectId, amount: f32) {
    let at = sim.position(e);
    hit(sim, HaloHit::area(e, effect, amount, 1.0, at, Vec3::NEG_Y, None));
}

fn hit(sim: &mut Sim, h: HaloHit) {
    sim.app.world_mut().write_message(h);
    sim.ticks(1);
}

fn grant(sim: &mut Sim, e: Entity, powerup: Powerup) {
    sim.app.world_mut().write_message(GrantPowerup { target: e, powerup });
    sim.ticks(1);
}

/// Hits of `effect` (`rnd`) on `on` until the shield breaks and the body
/// dies: (break, kill), 1-based.
fn shots_to_kill(effect: EffectId, rnd: f32, on: On) -> (u32, u32) {
    let mut sim = sim();
    let e = spartan(&mut sim);
    let mut broke = 0;
    for n in 1..=60 {
        shoot(&mut sim, e, effect, rnd, on);
        let v = vit(&sim, e);
        if broke == 0 && v.shield == 0.0 {
            broke = n;
        }
        if v.dead {
            assert!(v.body < 0.0 || v.body == 0.0 && broke > 0);
            return (broke, n);
        }
    }
    panic!("no kill in 60 hits");
}

fn bits(x: f32) -> u32 {
    x.to_bits()
}

fn close(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() <= eps
}

#[test]
fn spawn() {
    let mut sim = sim();
    let e = spartan(&mut sim);
    let v = vit(&sim, e);
    let s = sim.app.world().resource::<Spartan>().clone();
    assert_eq!((v.body, v.shield), (1.0, 1.0));
    assert_eq!((s.model.maximum_body, s.model.maximum_shield), (75.0, 75.0));
    assert_eq!(health(&sim, e), 1.0);
    assert_eq!(sim.app.world().get::<Armor>(e).unwrap().amount, 1.0);
}

#[test]
fn pistol_three_headshots_by_f32_rounding() {
    let mut sim = sim();
    let e = spartan(&mut sim);
    shoot(&mut sim, e, ids::PISTOL_BULLET, 25.0, On::Head);
    let v = vit(&sim, e);
    assert_eq!((bits(v.shield), v.body), (0x3F2AAAAA, 1.0));
    shoot(&mut sim, e, ids::PISTOL_BULLET, 25.0, On::Head);
    assert_eq!(bits(vit(&sim, e).shield), 0x3EAAAAA9);
    shoot(&mut sim, e, ids::PISTOL_BULLET, 25.0, On::Head);
    let v = vit(&sim, e);
    assert_eq!(v.shield, 0.0);
    assert!(v.dead && close(v.body, -7.63e-8, 0.01e-8), "{}", v.body);
    assert_eq!(health(&sim, e), 0.0);
}

#[test]
fn pistol_body_and_legs() {
    let mut sim = sim();
    let e = spartan(&mut sim);
    let mut body = vec![];
    for _ in 0..5 {
        shoot(&mut sim, e, ids::PISTOL_BULLET, 25.0, On::Body);
        body.push(vit(&sim, e).body);
    }
    assert_eq!(&body[2..4], &[0.99999994, 0.49999994]);
    assert!(close(body[4], -0.00000006, 0.5e-8), "{}", body[4]);
    assert!(vit(&sim, e).dead);
    let mut sim = self::sim();
    let e = spartan(&mut sim);
    let mut legs = vec![];
    for _ in 0..6 {
        shoot(&mut sim, e, ids::PISTOL_BULLET, 25.0, On::Legs);
        legs.push(vit(&sim, e));
    }
    assert!(close(legs[3].body, 0.5999999, 1e-7) && close(legs[4].body, 0.1999999, 1e-7), "{legs:?}");
    assert!(!legs[4].dead && legs[5].dead);
}

#[test]
fn assault_rifle() {
    let mut sim = sim();
    let e = spartan(&mut sim);
    shoot(&mut sim, e, ids::ASSAULT_RIFLE_BULLET, 10.0, On::Body);
    let v = vit(&sim, e);
    assert_eq!(bits(v.shield), 0x3F5DDDDE);
    // The hit set the delay to 180; this tick's update counted it once.
    assert_eq!(v.delay, 179);
    for _ in 2..=7 {
        shoot(&mut sim, e, ids::ASSAULT_RIFLE_BULLET, 10.0, On::Body);
    }
    assert!(close(vit(&sim, e).shield, 0.06666669, 1e-7));
    shoot(&mut sim, e, ids::ASSAULT_RIFLE_BULLET, 10.0, On::Body);
    let v = vit(&sim, e);
    assert_eq!(v.shield, 0.0);
    assert!(close(v.body, 0.93333334, 1e-7), "{}", v.body);
    assert_eq!(shots_to_kill(ids::ASSAULT_RIFLE_BULLET, 10.0, On::Body), (8, 16));
    assert_eq!(shots_to_kill(ids::ASSAULT_RIFLE_BULLET, 10.0, On::Legs).1, 17);
    assert_eq!(shots_to_kill(ids::ASSAULT_RIFLE_BULLET, 10.0, On::Head), (8, 16));
}

#[test]
fn sniper() {
    for (on, body) in [(On::Body, 0.6533333), (On::Legs, 0.7226666)] {
        let mut sim = sim();
        let e = spartan(&mut sim);
        shoot(&mut sim, e, ids::SNIPER_BULLET, 101.0, on);
        let v = vit(&sim, e);
        assert_eq!(v.shield, 0.0);
        assert!(close(v.body, body, 1e-6), "{}", v.body);
        if matches!(on, On::Body) {
            shoot(&mut sim, e, ids::SNIPER_BULLET, 101.0, on);
            let v = vit(&sim, e);
            assert!(v.dead && close(v.body, -0.6933334, 1e-6), "{}", v.body);
        }
    }
    assert_eq!(shots_to_kill(ids::SNIPER_BULLET, 101.0, On::Head), (1, 1));
}

#[test]
fn plasma_rifle() {
    let mut sim = sim();
    let e = spartan(&mut sim);
    let mut seen = vec![];
    for _ in 0..4 {
        shoot(&mut sim, e, ids::PLASMA_RIFLE_BOLT, 12.0, On::Body);
        seen.push(vit(&sim, e));
    }
    for (v, want) in seen.iter().zip([0.68, 0.36, 0.04, 0.0]) {
        assert!(close(v.shield, want, 2e-7), "{seen:?}");
    }
    assert!(close(seen[3].body, 0.86, 2e-7), "{}", seen[3].body);
    // The head: double body damage once the shield is gone.
    let mut sim = self::sim();
    let e = spartan(&mut sim);
    for n in 1..=9 {
        shoot(&mut sim, e, ids::PLASMA_RIFLE_BOLT, 12.0, On::Head);
        let v = vit(&sim, e);
        if n == 4 {
            assert!(close(v.body, 0.72, 2e-7), "{}", v.body);
        }
        assert_eq!(v.dead, n == 9, "hit {n}: {}", v.body);
    }
    assert_eq!(shots_to_kill(ids::PLASMA_RIFLE_BOLT, 14.0, On::Body).1, 14);
    assert_eq!(shots_to_kill(ids::PLASMA_RIFLE_BOLT, 14.0, On::Head).1, 8);
    assert_eq!(shots_to_kill(ids::PLASMA_RIFLE_BOLT, 12.0, On::Body), (4, 15));
    assert_eq!(shots_to_kill(ids::PLASMA_RIFLE_BOLT, 14.0, On::Body).0, 3);
}

#[test]
fn plasma_pistol() {
    assert_eq!(shots_to_kill(ids::PLASMA_PISTOL_BOLT, 16.0, On::Body), (8, 16));
    assert_eq!(shots_to_kill(ids::PLASMA_PISTOL_BOLT, 20.0, On::Body), (7, 13));
    assert_eq!(shots_to_kill(ids::PLASMA_PISTOL_BOLT, 20.0, On::Head), (7, 13));
}

#[test]
fn charged_plasma_then_pistol() {
    let charged = |sim: &mut Sim| {
        let e = spartan(sim);
        shoot(sim, e, ids::CHARGED_BOLT, 70.0, On::Body);
        let v = vit(sim, e);
        assert_eq!((v.shield, v.body, v.delay), (0.0, 1.0, 179));
        e
    };
    let mut sim = sim();
    let e = charged(&mut sim);
    shoot(&mut sim, e, ids::PISTOL_BULLET, 25.0, On::Head);
    let v = vit(&sim, e);
    assert!(v.dead && close(v.body, -0.5, 1e-6), "{}", v.body);
    // Two body shots leave exactly 0: alive; the third kills.
    let mut sim = self::sim();
    let e = charged(&mut sim);
    shoot(&mut sim, e, ids::PISTOL_BULLET, 25.0, On::Body);
    shoot(&mut sim, e, ids::PISTOL_BULLET, 25.0, On::Body);
    let v = vit(&sim, e);
    assert_eq!((v.body, v.dead), (0.0, false));
    assert_eq!(health(&sim, e), ALIVE_AT_ZERO, "alive at exactly 0");
    shoot(&mut sim, e, ids::PISTOL_BULLET, 25.0, On::Body);
    assert!(vit(&sim, e).dead);
}

#[test]
fn shotgun_pellets() {
    for (rnd, kill) in [(18.0, 13), (21.5, 11), (25.0, 9)] {
        let (broke, killed) = shots_to_kill(ids::SHOTGUN_PELLET, rnd, On::Body);
        assert_eq!(killed, kill, "rnd {rnd}");
        assert!((6..=9).contains(&broke), "rnd {rnd}: break {broke}");
    }
}

#[test]
fn needles_and_supercombine() {
    assert_eq!(shots_to_kill(ids::NEEDLE_DETONATION, 10.0, On::None), (8, 16));
    let mut sim = sim();
    let e = spartan(&mut sim);
    blast(&mut sim, e, ids::NEEDLER_EXPLOSION, 60.0);
    let v = vit(&sim, e);
    assert_eq!((v.shield, v.body), (0.0, 1.0));
    for n in 1..=7 {
        shoot(&mut sim, e, ids::NEEDLE_DETONATION, 10.0, On::None);
        if n == 1 {
            assert!(close(vit(&sim, e).body, 0.8666667, 1e-6));
        }
    }
    let v = vit(&sim, e);
    assert!(!v.dead && close(v.body, 0.0666667, 1e-6), "supercombine does not kill alone: {}", v.body);
}

#[test]
fn grenades_and_rockets() {
    let mut sim = sim();
    let e = spartan(&mut sim);
    blast(&mut sim, e, ids::FRAG_EXPLOSION, 120.0);
    let v = vit(&sim, e);
    assert_eq!((v.shield, bits(v.body), v.dead), (0.0, bits(0.39999998), false));
    // A plasma grenade unstuck at the centre: the same.
    let mut sim = self::sim();
    let e = spartan(&mut sim);
    blast(&mut sim, e, ids::PLASMA_GRENADE_EXPLOSION, 120.0);
    assert!(close(vit(&sim, e).body, 0.4, 1e-6));
    // Stuck: attached (emp, no material) then the explosion.
    let mut sim = self::sim();
    let e = spartan(&mut sim);
    shoot(&mut sim, e, ids::PLASMA_GRENADE_ATTACHED, 120.0, On::None);
    assert!(close(vit(&sim, e).body, 0.4, 1e-6));
    blast(&mut sim, e, ids::PLASMA_GRENADE_EXPLOSION, 120.0);
    let v = vit(&sim, e);
    assert!(v.dead && close(v.body, -1.2, 1e-6), "{}", v.body);
    // A rocket at the centre kills a full Spartan.
    let mut sim = self::sim();
    let e = spartan(&mut sim);
    blast(&mut sim, e, ids::ROCKET_EXPLOSION, 300.0);
    let v = vit(&sim, e);
    assert!(v.dead && close(v.body, -2.0, 1e-5), "{}", v.body);
}

/// A Spartan with a full overshield (σ = 3.0 exactly, charge-up over).
fn overshielded(sim: &mut Sim) -> Entity {
    let e = spartan(sim);
    grant(sim, e, Powerup::Overshield);
    sim.ticks(61);
    assert_eq!(vit(sim, e).shield, 3.0);
    e
}

#[test]
fn overshield_hits() {
    let mut sim = sim();
    let e = overshielded(&mut sim);
    blast(&mut sim, e, ids::ROCKET_EXPLOSION, 300.0);
    let v = vit(&sim, e);
    assert_eq!((v.shield, v.body, v.dead), (0.0, 0.0, false), "a 300-point rocket leaves exactly 0");
    let mut sim = self::sim();
    let e = overshielded(&mut sim);
    blast(&mut sim, e, ids::ROCKET_EXPLOSION, 301.0);
    let v = vit(&sim, e);
    assert!(v.dead && close(v.body, -0.0133333, 1e-6), "{}", v.body);
    let mut sim = self::sim();
    let e = overshielded(&mut sim);
    shoot(&mut sim, e, ids::SNIPER_BULLET, 101.0, On::Head);
    let v = vit(&sim, e);
    // 1.6533333 after the hit, then the tick's drain (it runs every tick
    // while the shield is above 1).
    assert!(close(v.shield + 1.0 / 1350.0, 1.6533333, 1e-6) && v.body == 1.0, "{v:?}");
    let mut sim = self::sim();
    let e = overshielded(&mut sim);
    let mut n = 0;
    while !vit(&sim, e).dead {
        shoot(&mut sim, e, ids::ASSAULT_RIFLE_BULLET, 10.0, On::Body);
        n += 1;
    }
    assert_eq!(n, 30);
}

/// A melee of a gun (`pistol_melee`) on `e` at scale `s` with the draw
/// `rnd`, from `from` (an offset from its centre along its look: -1 in
/// front of it, +1 behind... as `epicenter = centre - from x look`).
fn melee(sim: &mut Sim, e: Entity, effect: EffectId, s: f32, rnd: f32, behind: bool) {
    let amount = effects(sim).get(effect).unwrap().amount_with(s, rnd);
    let centre = sim.position(e);
    let look = Vec3::NEG_Z;
    let epicenter = if behind { centre - look } else { centre + look };
    hit(sim, HaloHit::melee(e, effect, amount, s, epicenter, -look, None));
}

#[test]
fn melee_from_the_front() {
    let mut sim = sim();
    let e = spartan(&mut sim);
    melee(&mut sim, e, ids::PISTOL_MELEE, 0.0, 50.0, false);
    assert!(close(vit(&sim, e).shield, 0.25333333, 1e-7));
    melee(&mut sim, e, ids::PISTOL_MELEE, 0.0, 50.0, false);
    let v = vit(&sim, e);
    assert!(v.shield == 0.0 && close(v.body, 0.30933332, 1e-6), "37 excess x 1.4: {v:?}");
    melee(&mut sim, e, ids::PISTOL_MELEE, 0.0, 50.0, false);
    assert!(vit(&sim, e).dead);
    // Running (s = 1, rnd 60): 9 excess x 1.4.
    let mut sim = self::sim();
    let e = spartan(&mut sim);
    melee(&mut sim, e, ids::PISTOL_MELEE, 1.0, 60.0, false);
    let v = vit(&sim, e);
    assert!(v.shield == 0.0 && close(v.body, 0.832, 1e-6), "{v:?}");
    // A lunge (s = 1.5, rnd 50): T = 55.
    let s = melee_scale(sim.app.world().resource::<Spartan>(), 0.0, 16);
    assert_eq!(s, 1.5);
    assert_eq!(effects(&sim).get(ids::PISTOL_MELEE).unwrap().amount_with(s, 50.0), 55.0);
    let mut sim = self::sim();
    let e = spartan(&mut sim);
    melee(&mut sim, e, ids::PISTOL_MELEE, 1.5, 50.0, false);
    let v = vit(&sim, e);
    assert!(v.shield == 0.0 && close(v.body, 0.96266669, 1e-6), "{v:?}");
}

#[test]
fn melee_from_behind_kills_through_any_shield() {
    let mut sim = sim();
    let e = spartan(&mut sim);
    melee(&mut sim, e, ids::PISTOL_MELEE, 0.0, 50.0, true);
    assert!(vit(&sim, e).dead);
    // During the overshield's charge-up (invulnerable to the rest).
    let mut sim = self::sim();
    let e = spartan(&mut sim);
    grant(&mut sim, e, Powerup::Overshield);
    sim.ticks(5);
    assert!(vit(&sim, e).overcharging);
    shoot(&mut sim, e, ids::SNIPER_BULLET, 101.0, On::Head);
    assert_eq!(vit(&sim, e).body, 1.0, "invulnerable while charging");
    melee(&mut sim, e, ids::PISTOL_MELEE, 0.0, 50.0, true);
    assert!(vit(&sim, e).dead);
    // The rule: the attacker in the half-space behind the target.
    assert!(unsuspecting(Vec3::ZERO, Vec3::new(-0.5, 0.0, 0.0), Vec3::X));
    assert!(!unsuspecting(Vec3::ZERO, Vec3::new(0.0, 0.5, 0.0), Vec3::X));
}

#[test]
fn falling() {
    let mut sim = sim();
    let e = spartan(&mut sim);
    let fx = effects(&sim);
    let h = HaloHit::fall(&fx, e, 0.0574539);
    assert!(close(h.amount, 18.33, 0.01), "{}", h.amount);
    hit(&mut sim, h);
    assert!(close(vit(&sim, e).shield, 0.7555891, 1e-6));
    let mut sim = self::sim();
    let e = spartan(&mut sim);
    hit(&mut sim, HaloHit::fall(&fx, e, 1.0));
    let v = vit(&sim, e);
    assert!(v.dead && close(v.body, -0.0666667, 1e-6), "{}", v.body);
    let mut sim = self::sim();
    let e = spartan(&mut sim);
    hit(&mut sim, HaloHit::lethal_fall(&fx, e));
    assert!(close(vit(&sim, e).body, 0.6, 1e-6));
    hit(&mut sim, HaloHit::lethal_fall(&fx, e));
    assert!(vit(&sim, e).dead);
}

#[test]
fn shield_recharge_after_one_hit() {
    let mut sim = sim();
    let e = spartan(&mut sim);
    shoot(&mut sim, e, ids::ASSAULT_RIFLE_BULLET, 10.0, On::Body); // tick 0
    for tick in 1..=179u32 {
        let v = vit(&sim, e);
        assert_eq!((bits(v.shield), v.delay), (0x3F5DDDDE, 180 - tick), "tick {}", tick - 1);
        sim.ticks(1);
    }
    let v = vit(&sim, e);
    assert_eq!((bits(v.shield), v.delay), (0x3F5DDDDE, 0), "tick 179");
    let mut seen = vec![];
    for _ in 180..=196 {
        sim.ticks(1);
        seen.push(vit(&sim, e).shield);
    }
    assert!(close(seen[0], 0.875, 1e-7) && close(seen[1], 0.8833333, 1e-7), "{seen:?}");
    assert!(close(seen[15], 0.9999999, 1e-7) && seen[15] < 1.0 && seen[16] == 1.0, "{seen:?}");
}

#[test]
fn shield_recharge_after_a_break() {
    let mut sim = sim();
    let e = spartan(&mut sim);
    shoot(&mut sim, e, ids::SNIPER_BULLET, 101.0, On::Body); // tick 0
    assert!(vit(&sim, e).depleted);
    sim.ticks(179); // ticks 1 to 179
    assert_eq!(vit(&sim, e).shield, 0.0);
    sim.ticks(1); // tick 180
    let v = vit(&sim, e);
    assert_eq!((bits(v.shield), v.depleted, v.charging), (0x3C088889, false, true));
    sim.ticks(119); // tick 299
    assert!(close(vit(&sim, e).shield, 0.9999994, 1e-7) && vit(&sim, e).shield < 1.0);
    sim.ticks(1); // tick 300
    assert_eq!(vit(&sim, e).shield, 1.0);
    // A second hit at tick 250 stops the recharge: next increment 430.
    let mut sim = self::sim();
    let e = spartan(&mut sim);
    shoot(&mut sim, e, ids::SNIPER_BULLET, 101.0, On::Body); // tick 0
    sim.ticks(249); // to tick 249
    shoot(&mut sim, e, ids::ASSAULT_RIFLE_BULLET, 10.0, On::Body); // tick 250
    let kept = vit(&sim, e).shield;
    assert!(kept > 0.0);
    sim.ticks(179); // to tick 429
    assert_eq!(vit(&sim, e).shield, kept);
    sim.ticks(1); // tick 430
    assert!(vit(&sim, e).shield > kept);
}

#[test]
fn overshield_timeline() {
    let mut sim = sim();
    let e = spartan(&mut sim);
    grant(&mut sim, e, Powerup::Overshield); // tick N (end)
    let mut seen = vec![];
    for _ in 0..61 {
        sim.ticks(1);
        seen.push(vit(&sim, e).shield);
    }
    assert!(close(seen[0], 1.0333333, 1e-7) && close(seen[1], 1.0666666, 1e-7), "{seen:?}");
    assert!(close(seen[59], 2.9999981, 1e-6) && seen[59] < 3.0, "{}", seen[59]);
    assert_eq!(seen[60], 3.0);
    assert!(!vit(&sim, e).overcharging);
    sim.ticks(1);
    assert!(close(vit(&sim, e).shield, 2.9992592, 1e-7));
    sim.ticks(2699);
    assert_eq!(vit(&sim, e).shield, 1.0, "2700 ticks from 3.0 to 1.0");
    // From empty: 0.01, then 3.0 on the 90th update.
    let mut sim = self::sim();
    let e = spartan(&mut sim);
    shoot(&mut sim, e, ids::SNIPER_BULLET, 101.0, On::Body);
    grant(&mut sim, e, Powerup::Overshield);
    assert_eq!(vit(&sim, e).shield, 0.01);
    sim.ticks(89);
    assert!(vit(&sim, e).shield < 3.0);
    sim.ticks(1);
    assert_eq!(vit(&sim, e).shield, 3.0);
}

#[test]
fn health_pack_and_refused_pickups() {
    let mut sim = sim();
    let e = spartan(&mut sim);
    blast(&mut sim, e, ids::FRAG_EXPLOSION, 120.0);
    let shield = vit(&sim, e).shield;
    grant(&mut sim, e, Powerup::HealthPack);
    let v = vit(&sim, e);
    assert_eq!((v.body, v.shield), (1.0, shield));
    assert_eq!(health(&sim, e), 1.0);
    let taken = |sim: &Sim| sim.app.world().resource::<Messages<halo_ce::game::vitality::PowerupTaken>>().iter_current_update_messages().count();
    grant(&mut sim, e, Powerup::HealthPack);
    assert_eq!(taken(&sim), 0, "full health: refused");
    let mut sim = self::sim();
    let e = overshielded(&mut sim);
    sim.ticks(2000);
    assert!(vit(&sim, e).shield > 1.4);
    grant(&mut sim, e, Powerup::Overshield);
    assert!(!vit(&sim, e).overcharging, "shield above 1: refused");
}

#[test]
fn stun() {
    let mut sim = sim();
    let e = spartan(&mut sim);
    let stun = |sim: &Sim| *sim.app.world().get::<Stun>(e).unwrap();
    shoot(&mut sim, e, ids::PLASMA_RIFLE_BOLT, 12.0, On::Body);
    assert_eq!(stun(&sim), Stun { value: 1.0, ticks: 4 });
    sim.ticks(3);
    assert_eq!(stun(&sim).value, 1.0);
    sim.ticks(1);
    assert_eq!(stun(&sim), Stun { value: 0.0, ticks: 0 });
    let mut sim = self::sim();
    let e = spartan(&mut sim);
    let mut seen = vec![];
    for _ in 0..12 {
        shoot(&mut sim, e, ids::PLASMA_RIFLE_BOLT, 12.0, On::Body);
        seen.push(stun(&sim).ticks);
    }
    assert_eq!(seen, [4, 7, 10, 13, 16, 19, 22, 25, 28, 31, 34, 37]);
    shoot(&mut sim, e, ids::PLASMA_RIFLE_BOLT, 12.0, On::Body);
    assert_eq!(stun(&sim).ticks, 37, "capped");
}

#[test]
fn camouflage() {
    let mut sim = sim();
    let e = spartan(&mut sim);
    grant(&mut sim, e, Powerup::ActiveCamouflage);
    let v = vit(&sim, e);
    assert_eq!((v.camouflaged, v.camo_ticks, v.camo), (true, 1350, 0.0));
    // 120 steps of 1/120 in f32 leave 0.9999994 (as the shield's
    // recharge does); the 121st clamps to 1 (the spec's "120 ticks: 1.0"
    // is the exact sum).
    sim.ticks(120);
    assert!(close(vit(&sim, e).camo, 0.9999994, 1e-7), "{}", vit(&sim, e).camo);
    sim.ticks(1);
    assert_eq!(vit(&sim, e).camo, 1.0);
    // Firing a pistol (ding 0.65, regrowth 0.01): the weapons' side.
    {
        let spartan = sim.app.world().resource::<Spartan>().clone();
        let mut v = sim.app.world_mut().get_mut::<Vitality>(e).unwrap();
        v.fired(&spartan, Some(0.65), 0.01);
        assert!(close(v.camo, 0.35, 1e-6));
    }
    sim.ticks(1);
    assert!(close(vit(&sim, e).camo, 0.36, 1e-6));
    sim.ticks(70);
    assert_eq!(vit(&sim, e).camo, 1.0);
    shoot(&mut sim, e, ids::ASSAULT_RIFLE_BULLET, 10.0, On::Body);
    assert!(close(vit(&sim, e).camo, 0.75 + 1.0 / 120.0, 1e-6), "0.75 after the hit, then a tick's regrowth");
}

#[test]
fn recent_shield_damage_and_the_delay_restarting() {
    let mut sim = sim();
    let e = spartan(&mut sim);
    for _ in 0..3 {
        shoot(&mut sim, e, ids::ASSAULT_RIFLE_BULLET, 10.0, On::Body);
    }
    let v = vit(&sim, e);
    // Restarted to 180 by the third hit, counted once by its tick.
    assert_eq!(v.delay, 179);
    assert!(close(v.recent_shield_damage, 0.4, 1e-6), "{}", v.recent_shield_damage);
}

#[test]
fn a_death_is_the_engine_s_and_credited() {
    let mut sim = sim();
    let shooter = spartan_at(&mut sim, greybox::SPAWNS[0] + Vec3::X * 3.0);
    let e = spartan(&mut sim);
    let fx = effects(&sim);
    let amount = fx.get(ids::SNIPER_BULLET).unwrap().amount(1.0, 0.0);
    sim.app.world_mut().write_message(
        HaloHit::direct(e, ids::SNIPER_BULLET, amount, 1.0, Some(BodyRegion::Head), Vec3::ZERO, Vec3::NEG_Z, Some(shooter))
            .weapon("halo_ce:sniper_rifle"),
    );
    sim.ticks(1);
    let died: Vec<Died> = sim.app.world().resource::<Messages<Died>>().iter_current_update_messages().cloned().collect();
    let d = died.iter().find(|d| d.entity == e).expect("Died");
    assert_eq!((d.attacker, d.damage.weapon), (Some(shooter), Some("halo_ce:sniper_rifle")));
    // A fall within 6 s of an enemy's hit: the enemy's kill.
    let mut sim = self::sim();
    let enemy = spartan_at(&mut sim, greybox::SPAWNS[0] + Vec3::X * 3.0);
    let e = spartan(&mut sim);
    let amount = fx.get(ids::ASSAULT_RIFLE_BULLET).unwrap().amount(1.0, 0.0);
    hit(&mut sim, HaloHit::direct(e, ids::ASSAULT_RIFLE_BULLET, amount, 1.0, Some(BodyRegion::Chest), Vec3::ZERO, Vec3::NEG_Z, Some(enemy)));
    sim.ticks(30);
    hit(&mut sim, HaloHit::fall(&fx, e, 1.0));
    let died: Vec<Died> = sim.app.world().resource::<Messages<Died>>().iter_current_update_messages().cloned().collect();
    assert_eq!(died.iter().find(|d| d.entity == e).expect("Died").attacker, Some(enemy));
}

#[test]
fn intent_faces_where_the_tests_say() {
    // The melee tests put the attacker along -Z of yaw 0.
    let mut sim = sim();
    let e = spartan(&mut sim);
    assert_eq!(sim.app.world().get::<Intent>(e).unwrap().yaw, 0.0);
}

/// Melees of `effect` at scale `s` (`rnd`) from the front until the
/// shield breaks and the body dies: (break, kill).
fn melees_to_kill(effect: EffectId, s: f32, rnd: f32) -> (u32, u32) {
    let mut sim = sim();
    let e = spartan(&mut sim);
    let mut broke = 0;
    for n in 1..=10 {
        melee(&mut sim, e, effect, s, rnd, false);
        let v = vit(&sim, e);
        if broke == 0 && v.shield == 0.0 {
            broke = n;
        }
        if v.dead {
            return (broke, n);
        }
    }
    panic!("no kill");
}

/// The rest of the spec's "Shots to kill" table (break / kill).
#[test]
fn shots_to_kill_table() {
    assert_eq!(shots_to_kill(ids::PISTOL_BULLET, 25.0, On::Body), (3, 5));
    assert_eq!(shots_to_kill(ids::PISTOL_BULLET, 25.0, On::Legs), (3, 6));
    assert_eq!(shots_to_kill(ids::PISTOL_BULLET, 25.0, On::Head), (3, 3));
    assert_eq!(shots_to_kill(ids::SNIPER_BULLET, 101.0, On::Body), (1, 2));
    assert_eq!(shots_to_kill(ids::PLASMA_RIFLE_BOLT, 12.0, On::Head), (4, 9));
    assert_eq!(shots_to_kill(ids::PLASMA_RIFLE_BOLT, 14.0, On::Head), (3, 8));
    assert_eq!(melees_to_kill(ids::PISTOL_MELEE, 0.0, 50.0), (2, 3), "standing");
    for rnd in [50.0, 60.0] {
        let (broke, kill) = melees_to_kill(ids::PISTOL_MELEE, 1.0, rnd);
        assert!((1..=2).contains(&broke) && kill == 2, "running {rnd}: {broke} {kill}");
    }
    for rnd in [50.0, 60.0] {
        assert_eq!(melees_to_kill(ids::PISTOL_MELEE, 1.5, rnd), (1, 2), "lunging {rnd}");
    }
    assert_eq!(melees_to_kill(ids::BALL_MELEE, 1.0, 80.0), (1, 2), "ball");
    assert_eq!(melees_to_kill(ids::FLAG_MELEE, 0.0, 80.0), (1, 2), "flag");
}
