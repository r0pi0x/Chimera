//! Halo's projectiles against the corrected projectiles.md, on
//! `game::weapons::flight` with the data's numbers: every projectile's
//! material response block is complete (each of its 33 elements, 31 for
//! the flame, stated once: "Complete material response tables"; no
//! stand-in element), and the speed clamp after a hit acts only on a
//! segment that decelerated (behaviour 3 step 7, "When the clamp acts").
//! The heavy `weapon_data` test holds every stated element to the disc's.

use bevy::math::Vec3;
use halo_ce::game::weapons::{
    ASSAULT_RIFLE, GUNS, PLASMA_PISTOL,
    flight::{Flight, Hit},
    grenades::GrenadeData,
    gun,
    params::{GRAVITY, Projectile},
    random::HaloRng,
};

/// Every projectile of Halo's data: the guns' (and the charged bolt) and
/// the grenades'.
fn projectiles() -> Vec<Projectile> {
    let mut out = Vec::new();
    for id in GUNS {
        let g = gun(id);
        out.push(g.projectile);
        if let Some(s) = g.secondary {
            out.push(s.projectile);
        }
    }
    let g = GrenadeData::load();
    out.extend([g.frag, g.plasma]);
    out
}

#[test]
fn every_projectiles_response_block_is_complete() {
    for p in projectiles() {
        let r = &p.responses;
        let want = if p.tag.ends_with("flame") { 31 } else { 33 };
        assert_eq!(r.count, want, "{}: the block's count", p.tag);
        assert_eq!(r.unstated, None, "{}: no stand-in element", p.tag);
        for t in 0..r.count {
            let n = r.elements.iter().filter(|e| e.types.contains(&t)).count();
            assert_eq!(n, 1, "{}: type {t} stated once", p.tag);
        }
        assert!(r.elements.iter().all(|e| e.types.iter().all(|t| *t < r.count)), "{}", p.tag);
    }
}

/// The spec's grenade rows that used to be dirt's stand-in.
#[test]
fn the_grenades_take_their_own_elements() {
    let g = GrenadeData::load();
    let fr = |p: &Projectile, t: u8| {
        let e = p.responses.get(Some(t));
        (e.parallel_friction.unwrap_or(0.0), e.perpendicular_friction.unwrap_or(0.0))
    };
    // Frag: rubber is 0.7 / 0.8 (dirt is 0.8 / 0.9), the jackal shield
    // 0.8 / 0.8, leaves 0.8 / 0.9 with its potential pass.
    assert_eq!(fr(&g.frag, 8), (0.7, 0.8));
    assert_eq!(fr(&g.frag, 16), (0.8, 0.8));
    assert_eq!(fr(&g.frag, 29), (0.8, 0.9));
    assert_eq!(fr(&g.frag, 1), (0.8, 0.9));
    // Plasma: the jackal shield reflects always at 0.8 / 0.8 (never
    // sticks); the engineer force field, monitor and hunter shield at
    // 0.9 / 0.95 without sticking; sand and wood stick to units.
    let p = &g.plasma;
    assert_eq!(fr(p, 16), (0.8, 0.8));
    assert!(!p.responses.get(Some(16)).only_against_units);
    for t in [18, 26, 32] {
        assert_eq!(fr(p, t), (0.9, 0.95), "type {t}");
        assert!(!p.responses.get(Some(t)).potential_applies(0.0, 0.1, true), "type {t}: no stick");
    }
    for t in [1, 4, 29] {
        assert!(p.responses.get(Some(t)).potential_applies(0.0, 0.1, true), "type {t}: sticks to units");
        assert!(!p.responses.get(Some(t)).potential_applies(0.0, 0.1, false), "type {t}: units only");
    }
    // The needle: glass reflects at 0.3 / 0.3 with noise 0.0873, hunter
    // armor attaches with 0.3 / 0.3, the elite shield attaches with 0.15.
    let needle = gun(halo_ce::game::weapons::NEEDLER).projectile;
    assert_eq!(fr(&needle, 9), (0.3, 0.3));
    assert_eq!(needle.responses.get(Some(9)).angular_noise, Some(0.0873));
    assert_eq!(fr(&needle, 12), (0.3, 0.3));
    assert_eq!(fr(&needle, 30), (0.15, 0.15));
}

/// The velocity the direct hit sees (`struck`) when `p` fired along +X
/// from the origin meets an object's wall at x = `wall` on update 1, and
/// that hit's fraction t.
fn struck_velocity(p: &Projectile, wall: f32) -> (Vec3, f32) {
    let mut f = Flight::new(p, Vec3::ZERO, Vec3::X, 0.0);
    let mut seen = None;
    let mut trace = |a: Vec3, b: Vec3, ignored: Option<u8>| {
        (ignored.is_none() && a.x <= wall && b.x > wall).then(|| {
            let t = (wall - a.x) / (b.x - a.x);
            Hit { t, point: a + (b - a) * t, normal: Vec3::NEG_X, object: Some(1), material: Some(21), unit: true }
        })
    };
    let mut struck = |h: &Hit<u8>, v: Vec3, _: f32| {
        seen = Some((v, h.t));
        None
    };
    f.update(p, &mut None, &mut HaloRng(1), None, &mut trace, &mut struck);
    seen.expect("the wall is met on update 1")
}

#[test]
fn the_speed_clamp_acts_only_on_a_decelerating_segment() {
    // The plasma pistol bolt decelerates from update 1: s' = s - a, and
    // after the hit at t the velocity is scaled by min((1 - t) a + s', s) / s'
    // (gravity's share for the unused part taken back first).
    let bolt = gun(PLASMA_PISTOL).projectile;
    let s = bolt.initial_velocity;
    let a = bolt.deceleration(bolt.air_damage_range);
    assert!(a > 0.0);
    let (v, t) = struck_velocity(&bolt, 0.4);
    let g = GRAVITY * bolt.air_gravity_scale;
    let s1 = s - a;
    let want = (Vec3::X * s1 - Vec3::Y * g * t) * (((1.0 - t) * a + s1).min(s) / s1);
    assert!((v - want).length() < 1e-6, "decelerating: {v} want {want}");
    // The assault rifle bullet (initial and final velocities equal) never
    // decelerates: the factor is 1 and the velocity is gravity's.
    let ar = gun(ASSAULT_RIFLE).projectile;
    let (v, t) = struck_velocity(&ar, 3.0);
    let g = GRAVITY * ar.air_gravity_scale;
    let want = Vec3::X * ar.initial_velocity - Vec3::Y * g * t;
    assert!((v - want).length() < 1e-6, "not decelerating: {v} want {want}");
}
