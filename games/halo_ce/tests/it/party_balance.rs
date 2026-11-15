//! Halo's balance across games (`game::cross_game`, data/
//! party_balance.toml) leaves Halo against Halo alone: every row of
//! specs/halo_ce/health_shields.md "Shots to kill" gives the same break
//! and kill, hit for hit the same shield, body and engine health, with
//! the switch on and off. (The cross-game numbers themselves are in
//! party_kit's tests/it/halo_balance.rs, where the other games' guns are.)

use bevy::prelude::*;
use chimera_core::core::{BodyRegion, Health};
use chimera_world::harness::Sim;
use halo_ce::game::{
    cross_game::CrossGame,
    damage::HaloHit,
    vitality::{DamageEffects, EffectId, Vitality, ids},
};

use super::vitality::{sim, spartan_at, vit};

/// One hit of the table: a direct hit on a region (None: no material),
/// an area hit, or a melee (scale, from behind).
#[derive(Clone, Copy, Debug)]
enum Hit {
    Direct(Option<BodyRegion>),
    Area,
    Melee(f32, bool),
}

/// Each hit's (shield, body, engine health).
type Seen = Vec<(f32, f32, f32)>;

/// Hits of `effect` (`rnd`) until the body dies, with the balance `cfg`:
/// (break, kill) and what each hit left.
fn run(cfg: CrossGame, effect: EffectId, rnd: f32, how: Hit) -> ((u32, u32), Seen) {
    let mut sim = sim();
    sim.app.insert_resource(cfg);
    let e = spartan_at(&mut sim, chimera_world::greybox::SPAWNS[0]);
    let mut seen = Vec::new();
    let mut broke = 0;
    for n in 1..=60 {
        hit(&mut sim, e, effect, rnd, how);
        let v = vit(&sim, e);
        seen.push((v.shield, v.body, sim.app.world().get::<Health>(e).unwrap().current));
        if broke == 0 && v.shield == 0.0 {
            broke = n;
        }
        if v.dead {
            return ((broke, n), seen);
        }
    }
    panic!("{effect:?}: no kill in 60 hits");
}

fn hit(sim: &mut Sim, e: Entity, effect: EffectId, rnd: f32, how: Hit) {
    let fx = sim.app.world().resource::<DamageEffects>().get(effect).unwrap().clone();
    let at = sim.position(e);
    let h = match how {
        Hit::Direct(region) => HaloHit::direct(e, effect, fx.amount_with(1.0, rnd), 1.0, region, at, Vec3::NEG_Z, None),
        Hit::Area => HaloHit::area(e, effect, fx.amount_with(1.0, rnd), 1.0, at, Vec3::NEG_Y, None),
        Hit::Melee(s, behind) => {
            let epicenter = if behind { at + Vec3::Z } else { at - Vec3::Z };
            HaloHit::melee(e, effect, fx.amount_with(s, rnd), s, epicenter, Vec3::Z, None)
        }
    };
    sim.app.world_mut().write_message(h);
    sim.ticks(1);
}

/// The spec's table (point blank, scale 1): (effect, draw, hit, break,
/// kill; a break of 0: the spec gives a range).
const TABLE: &[(EffectId, f32, Hit, u32, u32)] = &[
    (ids::PISTOL_BULLET, 25.0, Hit::Direct(Some(BodyRegion::Chest)), 3, 5),
    (ids::PISTOL_BULLET, 25.0, Hit::Direct(Some(BodyRegion::LeftShin)), 3, 6),
    (ids::PISTOL_BULLET, 25.0, Hit::Direct(Some(BodyRegion::Head)), 3, 3),
    (ids::ASSAULT_RIFLE_BULLET, 10.0, Hit::Direct(Some(BodyRegion::Chest)), 8, 16),
    (ids::ASSAULT_RIFLE_BULLET, 10.0, Hit::Direct(Some(BodyRegion::LeftShin)), 8, 17),
    (ids::ASSAULT_RIFLE_BULLET, 10.0, Hit::Direct(Some(BodyRegion::Head)), 8, 16),
    (ids::PLASMA_RIFLE_BOLT, 12.0, Hit::Direct(Some(BodyRegion::Chest)), 4, 15),
    (ids::PLASMA_RIFLE_BOLT, 14.0, Hit::Direct(Some(BodyRegion::Chest)), 3, 14),
    (ids::PLASMA_RIFLE_BOLT, 12.0, Hit::Direct(Some(BodyRegion::Head)), 4, 9),
    (ids::PLASMA_RIFLE_BOLT, 14.0, Hit::Direct(Some(BodyRegion::Head)), 3, 8),
    (ids::PLASMA_PISTOL_BOLT, 16.0, Hit::Direct(Some(BodyRegion::Chest)), 8, 16),
    (ids::PLASMA_PISTOL_BOLT, 20.0, Hit::Direct(Some(BodyRegion::Chest)), 7, 13),
    (ids::SHOTGUN_PELLET, 25.0, Hit::Direct(Some(BodyRegion::Chest)), 0, 9),
    (ids::SNIPER_BULLET, 101.0, Hit::Direct(Some(BodyRegion::Chest)), 1, 2),
    (ids::SNIPER_BULLET, 101.0, Hit::Direct(Some(BodyRegion::Head)), 1, 1),
    (ids::NEEDLE_DETONATION, 10.0, Hit::Direct(None), 8, 16),
    (ids::PISTOL_MELEE, 50.0, Hit::Melee(0.0, false), 2, 3),
    (ids::PISTOL_MELEE, 70.0, Hit::Melee(1.5, false), 1, 2),
    (ids::PISTOL_MELEE, 50.0, Hit::Melee(0.0, true), 0, 1),
];

#[test]
fn halo_against_halo_is_the_spec_s_table_with_the_switch_on_and_off() {
    for &(effect, rnd, how, broke, kill) in TABLE {
        let (on, on_seen) = run(CrossGame::load(), effect, rnd, how);
        let (off, off_seen) = run(CrossGame::off(), effect, rnd, how);
        let want = |got: (u32, u32)| (if broke == 0 { 0 } else { got.0 }, got.1) == (broke, kill);
        assert!(want(on), "{effect:?} {rnd} {how:?}: on {on:?}");
        assert!(want(off), "{effect:?} {rnd} {how:?}: off {off:?}");
        assert_eq!(on_seen, off_seen, "{effect:?} {rnd} {how:?}: hit for hit");
    }
    // A frag at the centre leaves the body at 0.4, both ways.
    for cfg in [CrossGame::load(), CrossGame::off()] {
        let mut sim = sim();
        sim.app.insert_resource(cfg);
        let e = spartan_at(&mut sim, chimera_world::greybox::SPAWNS[0]);
        hit(&mut sim, e, ids::FRAG_EXPLOSION, 120.0, Hit::Area);
        let v: Vitality = vit(&sim, e);
        assert!(v.shield == 0.0 && (v.body - 0.4).abs() < 1e-5 && !v.dead, "frag: {v:?}");
    }
}
