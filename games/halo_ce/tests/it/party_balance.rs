//! The cross-game table (`chimera_sim::weapon::balance`, Halo's
//! mechanisms in `game::cross_game`) scales Halo against Halo too (Halo's
//! outgoing times a Halo body's incoming); with the switch off every row
//! of specs/halo_ce/health_shields.md "Shots to kill" gives its break and
//! kill. (The cross-game numbers themselves are in
//! party_kit's tests/it/halo_balance.rs, where the other games' guns are.)

use bevy::prelude::*;
use chimera_core::core::{BodyRegion, Health};
use chimera_world::harness::Sim;
use chimera_sim::weapon::balance::Balance;
use halo_ce::game::{
    cross_game::PowerHand,
    damage::HaloHit,
    vitality::{DamageEffects, EffectId, Vitality, ids},
};

use super::{
    cross_game::party,
    vitality::{sim, spartan_at, vit},
};

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
fn run(cfg: Balance, effect: EffectId, rnd: f32, how: Hit) -> ((u32, u32), Seen) {
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
fn halo_against_halo_is_the_spec_s_table_off_and_the_table_s_scale_on() {
    for &(effect, rnd, how, broke, kill) in TABLE {
        let (on, on_seen) = run(party(true), effect, rnd, how);
        let (off, off_seen) = run(party(false), effect, rnd, how);
        let want = |got: (u32, u32)| (if broke == 0 { 0 } else { got.0 }, got.1) == (broke, kill);
        assert!(want(off), "{effect:?} {rnd} {how:?}: off {off:?}");
        assert!(on.1 <= off.1, "{effect:?} {rnd} {how:?}: the table's 1.125 kills no later: {on:?} {off:?}");
        // The first hit takes 1.125 times what it takes raw, shield and body
        // together, where it is neither a kill nor clamped.
        let lost = |seen: &Seen| (1.0 - seen[0].0) + (1.0 - seen[0].1);
        if off.1 > 2 && off_seen[0].0 > 0.0 {
            let k = lost(&on_seen) / lost(&off_seen);
            assert!((k - 1.125).abs() < 0.02, "{effect:?} {rnd} {how:?}: first hit x{k}");
        }
    }
    // A frag at the centre leaves the body at 0.4, raw.
    for cfg in [party(false)] {
        let mut sim = sim();
        sim.app.insert_resource(cfg);
        let e = spartan_at(&mut sim, chimera_world::greybox::SPAWNS[0]);
        hit(&mut sim, e, ids::FRAG_EXPLOSION, 120.0, Hit::Area);
        let v: Vitality = vit(&sim, e);
        assert!(v.shield == 0.0 && (v.body - 0.4).abs() < 1e-5 && !v.dead, "frag: {v:?}");
    }
}

/// The ladder's power reader with its hand (data/power_hand.toml):
/// the plasma pistol tapped at 7 a second, overheating after its shots and
/// pausing 1.15 s (to 0.25 heat at 0.65 a second); the needler's needles
/// flying 12.2 m/s and detonating 0.75 s after they stick, their homing
/// taking the cone off at 10 m; the assault rifle in 0.2 s bursts.
#[test]
fn the_power_reader_counts_a_hand() {
    use chimera_sim::{
        minigame::power::{PowerCurve, PowerReaders, stats},
        weapon::data::WeaponCatalog,
    };
    let sim = sim();
    let w = sim.app.world();
    let catalog = w.resource::<WeaponCatalog>();
    let readers = w.get_resource::<PowerReaders>().cloned().unwrap_or_default();
    let curve = PowerCurve::shipped();
    let at = |id: &str| stats(catalog.get(id).unwrap(), &readers, &curve, 10.0);
    let hand = PowerHand::load();
    let pp = at(halo_ce::game::weapons::PLASMA_PISTOL);
    assert!((pp.interval - 1.0 / hand.hand_taps_per_second).abs() < 1e-6, "{pp:?}");
    // 0.16 a bolt, 0.65 / 7 lost between: the 14th overheats.
    assert_eq!(pp.clip, 14, "{pp:?}");
    assert!((pp.reload - 0.75 / 0.65).abs() < 1e-4, "{pp:?}");
    let ne = at(halo_ce::game::weapons::NEEDLER);
    // 10 m at 4 WU a second (3.28 WU), then the fuse.
    assert!((ne.latency - (10.0 / 3.048 / 4.0 + 0.75)).abs() < 0.01, "{ne:?}");
    assert_eq!((ne.spread, ne.follow), (0.0, Some(0.0)), "homed onto the target");
    let ar = at(halo_ce::game::weapons::ASSAULT_RIFLE);
    // 3 rounds 2 ticks apart, then 10 ticks for the error (0.33) to go.
    assert!((ar.interval - 16.0 / 3.0 / 30.0).abs() < 1e-5, "{ar:?}");
    let mean = (0.034_906_585_f32 + (0.113_446_4 - 0.034_906_585) / 9.0).tan();
    assert!((ar.follow.unwrap() - mean).abs() < 1e-4, "{ar:?}");
}
