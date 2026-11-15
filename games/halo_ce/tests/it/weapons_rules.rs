//! Halo's weapon rules on their own (`game::weapons::tick`, `flight`,
//! `random`, `params`), tick by tick with the data's numbers: the test
//! cases of specs/halo_ce/weapons.md and projectiles.md that are about
//! numbers inside a weapon or a projectile (error values, heat, rate of
//! fire, the generator, flight positions and speeds, direct-hit amounts,
//! the reload interruption, ammunition pickups). Their timelines in the
//! game are `weapons`' scenario tests.

// The spec's numbers are written as it states them.
#![allow(clippy::excessive_precision)]

use bevy::math::Vec3;
use halo_ce::game::{
    vitality::DamageEffects,
    weapons::{
        ASSAULT_RIFLE, PISTOL, PLASMA_RIFLE, SHOTGUN,
        flight::{End, Flight, Hit},
        gun,
        params::{Gun, effect_id, take_ammunition},
        random::{self, HaloRng},
        tick::{GunEvent, HaloGun, Inputs, MagazineMode},
    },
};

const AIM: Vec3 = Vec3::NEG_Z;

fn held(fire: bool) -> Inputs {
    Inputs { fire, held: true, aim: AIM, ..Default::default() }
}

/// A weapon long in hand (weapons.md "Test cases": idle 127, rate of fire
/// and error 0).
fn ready(g: &Gun) -> HaloGun {
    let mut w = HaloGun::new(g);
    w.in_hand = true;
    w
}

/// Run `ticks` with `input(t)`; the ticks a round left on, and the state
/// after each tick.
fn run(g: &Gun, w: &mut HaloGun, ticks: u32, input: impl Fn(u32) -> Inputs) -> (Vec<u32>, Vec<HaloGun>) {
    let mut shots = Vec::new();
    let mut states = Vec::new();
    for t in 0..ticks {
        let ev = w.tick(g, &input(t));
        if ev.iter().any(|e| matches!(e, GunEvent::Fired { .. })) {
            shots.push(t);
        }
        states.push(w.clone());
    }
    (shots, states)
}

fn deg(x: f32) -> f32 {
    x.to_degrees()
}

#[test]
fn the_data_parses_and_names_known_effects() {
    let effects = DamageEffects::load();
    for id in [ASSAULT_RIFLE, PISTOL, PLASMA_RIFLE, SHOTGUN] {
        let g = gun(id);
        assert!(effect_id(&g.melee_damage).is_some_and(|e| effects.get(e).is_some()), "{id}: melee effect");
        assert!(effect_id(&g.projectile.impact_damage).is_some_and(|e| effects.get(e).is_some()), "{id}: impact");
        assert!((g.camera_field_of_view.to_degrees() - 70.0).abs() < 1e-4, "{id}: the base's field of view");
    }
}

#[test]
fn the_generator_draws_as_the_spec_states() {
    let mut r = HaloRng(0);
    assert_eq!(r.int(), 15470);
    assert_eq!(r.0, 0x3C6E_F35F);
    let mut r = HaloRng(0);
    assert!((r.real() - 0.236057).abs() < 1e-6);
    let mut r = HaloRng(0x3C6E_F35F);
    assert_eq!(r.int_range(0, 1026), 285);
    assert_eq!(r.0, 0x4750_2932);
}

#[test]
fn there_are_1026_unit_directions_and_the_cone_keeps_its_bounds() {
    let d = random::directions();
    assert_eq!(d.len(), 1026);
    assert!(d.iter().all(|v| (v.length() - 1.0).abs() < 1e-5));
    let mut r = HaloRng(7);
    for _ in 0..2000 {
        let v = random::cone(&mut r, AIM, 0.01, 0.1);
        let a = v.angle_between(AIM);
        assert!(a <= 0.1 + 1e-4 && (a >= 0.01 - 1e-4 || a < 1e-6), "tilt {a}");
    }
}

#[test]
fn ticks_between_shots_round_up_in_single_precision() {
    let ar = gun(ASSAULT_RIFLE);
    assert_eq!(ar.trigger.required_ticks(0.0, 0.0, 0.0), 2.0);
    let pr = gun(PLASMA_RIFLE);
    let pen = pr.age.rate_of_fire_penalty;
    assert!((pr.trigger.required_ticks(1.0, 0.005, pen) - 3.003).abs() < 1e-4);
    assert_eq!(pr.trigger.required_ticks(1.0, 0.0, pen), 3.0);
    assert!((pr.trigger.required_ticks(0.0, 0.995, pen) - 5.1386).abs() < 1e-4);
}

#[test]
fn the_plasma_rifles_rate_of_fire_reaches_one_on_the_55th_held_tick() {
    let g = gun(PLASMA_RIFLE);
    let mut w = ready(&g);
    let (_, states) = run(&g, &mut w, 55, |_| held(true));
    assert!((states[53].rate_of_fire - 0.9999996).abs() < 1e-7, "{}", states[53].rate_of_fire);
    assert!(states[53].rate_of_fire < 1.0);
    assert_eq!(states[54].rate_of_fire, 1.0);
}

#[test]
fn the_assault_rifles_error_grows_and_decays() {
    let g = gun(ASSAULT_RIFLE);
    let mut w = ready(&g);
    // The limit of shots 1 to 10 (the error left by the tick before).
    let mut limits = Vec::new();
    for t in 0..20 {
        let before = w.theta_max(&g);
        if w.tick(&g, &held(true)).iter().any(|e| matches!(e, GunEvent::Fired { .. })) {
            limits.push(deg(before));
        }
        let _ = t;
    }
    let want = [2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 5.5, 6.0, 6.5];
    for (got, want) in limits.iter().zip(want) {
        assert!((got - want).abs() < 1e-3, "{limits:?}");
    }
    // Held 30 ticks, then released.
    let mut w = ready(&g);
    let (_, s) = run(&g, &mut w, 60, |t| held(t < 30));
    for (t, e) in [(0, 0.0556), (17, 1.0), (29, 1.0), (30, 0.9667), (35, 0.8), (45, 0.4667), (59, 0.0)] {
        assert!((s[t].error - e).abs() < 1e-3, "tick {t}: {}", s[t].error);
    }
    assert!((deg(s[30].theta_max(&g)) - 6.35).abs() < 0.01);
}

#[test]
fn the_pistols_error_and_its_zoom() {
    let g = gun(PISTOL);
    let mut w = ready(&g);
    let mut limits = Vec::new();
    for _ in 0..20 {
        let before = w.theta_max(&g);
        if w.tick(&g, &held(true)).iter().any(|e| matches!(e, GunEvent::Fired { .. })) {
            limits.push(deg(before));
        }
    }
    for (got, want) in limits.iter().zip([0.2, 1.28, 2.0]) {
        assert!((got - want).abs() < 1e-3, "{limits:?}");
    }
    let mut w = ready(&g);
    let (_, s) = run(&g, &mut w, 36, |t| held(t < 30));
    for (t, e) in [(30, 0.8333), (31, 0.6667), (32, 0.5), (35, 0.0)] {
        assert!((s[t].error - e).abs() < 1e-3, "tick {t}: {}", s[t].error);
    }
    // One 2x level: 70 degrees become 35.
    assert_eq!(g.magnification(0), 2.0);
    assert!(((g.camera_field_of_view / g.magnification(0)).to_degrees() - 35.0).abs() < 1e-3);
    let mut w = ready(&g);
    w.tick(&g, &Inputs { zoom: true, ..held(false) });
    assert_eq!(w.zoom, 0);
    w.tick(&g, &held(false));
    w.tick(&g, &Inputs { zoom: true, ..held(false) });
    assert_eq!(w.zoom, -1, "a second press leaves the zoom");
}

#[test]
fn the_plasma_rifles_error_heat_and_overheat() {
    let g = gun(PLASMA_RIFLE);
    let mut w = ready(&g);
    let mut limits = Vec::new();
    let mut heat = Vec::new();
    let mut shots = Vec::new();
    let mut overheated = None;
    let mut cleared = None;
    for t in 0..200 {
        let before = w.theta_max(&g);
        let ev = w.tick(&g, &held(true));
        if ev.iter().any(|e| matches!(e, GunEvent::Fired { .. })) {
            limits.push(deg(before));
            heat.push(w.heat);
            shots.push(t);
        }
        if ev.contains(&GunEvent::Overheated) {
            overheated = Some(t);
        }
        if ev.contains(&GunEvent::OverheatCleared) {
            // The heat as it cleared (before this tick's shot, if any).
            let shot = ev.iter().any(|e| matches!(e, GunEvent::Fired { .. }));
            cleared.get_or_insert((t, if shot { w.heat - g.trigger.heat_per_round } else { w.heat }));
        }
    }
    for (got, want) in limits.iter().zip([0.5, 0.8, 1.1, 1.34, 1.58, 1.82]) {
        assert!((got - want).abs() < 0.01, "{limits:?}");
    }
    for (n, want) in [(1, 0.08), (2, 0.1101), (5, 0.2204), (10, 0.4218), (20, 0.8276), (24, 0.9911), (25, 1.0)] {
        assert!((heat[n - 1] - want).abs() < 1e-3, "heat after shot {n}: {}", heat[n - 1]);
    }
    assert_eq!(&shots[..6], &[0, 5, 10, 14, 18, 22]);
    assert_eq!(shots[24], 98);
    assert_eq!(overheated, Some(99));
    let (t, h) = cleared.unwrap();
    assert_eq!(t, 175);
    assert!((h - 0.2492).abs() < 1e-3);
    assert_eq!(shots[25], 175, "held, it fires again on the tick it clears");
    // Held 90 ticks, released.
    let mut w = ready(&g);
    let (_, s) = run(&g, &mut w, 91, |t| held(t < 90));
    // 75 additions of the tag's 1/75 in single precision: 0.99999934,
    // "1" to the spec's four places.
    assert!((s[74].error - 1.0).abs() < 1e-5 && s[89].error == 1.0 && s[90].error == 0.0);
}

#[test]
fn a_reload_interrupted_past_half_finishes_else_is_lost() {
    let g = gun(ASSAULT_RIFLE);
    for (left, loaded) in [(43, 60), (44, 0)] {
        let mut w = ready(&g);
        w.loaded = 0;
        w.reserve = 180;
        w.magazine = MagazineMode::Reloading;
        w.magazine_timer = left;
        w.settle_reload(&g);
        assert_eq!(w.loaded, loaded, "{left} ticks left");
        assert_eq!(w.magazine, MagazineMode::Idle);
    }
}

#[test]
fn picking_up_a_duplicate_weapon_takes_its_reserve() {
    // multiplayer_rules.md "Ammunition from a duplicate weapon".
    assert_eq!(take_ammunition(100, 600, 180), (280, 0, true));
    assert_eq!(take_ammunition(500, 600, 180), (600, 80, false));
    assert_eq!(take_ammunition(600, 600, 180), (600, 180, false));
    assert_eq!(take_ammunition(100, 120, 48), (120, 28, false));
    assert_eq!(take_ammunition(50, 60, 12), (60, 2, false));
    // A new weapon's rounds (4.6, step 4).
    let new = |id| gun(id).magazine.unwrap().new_rounds();
    assert_eq!((new(ASSAULT_RIFLE), new(PISTOL), new(SHOTGUN)), ((60, 180), (12, 48), (12, 12)));
}

// ---------------------------------------------------------------------------
// Projectiles (projectiles.md "Test cases" (a), (b), behaviour 4).

/// A flat floor at height 0 and, if any, a wall at x = `wall` (an object:
/// a character), in WU.
fn world(floor: bool, wall: Option<f32>) -> impl FnMut(Vec3, Vec3, Option<u8>) -> Option<Hit<u8>> {
    move |a: Vec3, b: Vec3, _| {
        let mut best: Option<Hit<u8>> = None;
        if floor && b.y < 0.0 && a.y >= 0.0 {
            let t = a.y / (a.y - b.y);
            best = Some(Hit::level(t, a + (b - a) * t, Vec3::Y, Some(2)));
        }
        // A trace ending exactly on the wall does not hit it; one starting
        // on it hits at t = 0 (projectiles.md, Edge cases).
        if let Some(x) = wall
            && a.x <= x
            && b.x > x
        {
            let t = (x - a.x) / (b.x - a.x);
            if best.is_none_or(|h| t < h.t) {
                best = Some(Hit { t, point: a + (b - a) * t, normal: Vec3::NEG_X, object: Some(1), material: Some(21), unit: true });
            }
        }
        best
    }
}

/// Fired level along +X from 0.62 WU up, updates until it ends (at most
/// `max`): positions and speeds after each, how it ended, the hits
/// (update, t, speed at impact, scale).
#[allow(clippy::type_complexity)]
fn shoot(id: &str, floor: bool, wall: Option<f32>, max: u32) -> (Vec<(Vec3, f32)>, Option<(u32, End)>, Vec<(u32, f32, f32, f32)>) {
    let g = gun(id);
    let p = &g.projectile;
    let mut f = Flight::new(p, Vec3::new(0.0, 0.62, 0.0), Vec3::X, 0.0);
    let mut trace = world(floor, wall);
    let mut out = Vec::new();
    let mut hits = Vec::new();
    for k in 1..=max {
        let mut ignored = None;
        let mut struck = |h: &Hit<u8>, v: Vec3, s: f32| {
            hits.push((k, h.t, v.length(), s));
            // The shield took it (22: every one of these disappears).
            Some(22)
        };
        let m = f.update(p, &mut ignored, &mut HaloRng(0), None, &mut trace, &mut struck);
        out.push((f.position, f.speed()));
        if let Some(e) = m.end {
            return (out, Some((k, e)), hits);
        }
    }
    (out, None, hits)
}

fn near(a: Vec3, b: Vec3, tol: f32) -> bool {
    (a - b).abs().max_element() < tol
}

/// The spec's (x, y, z) up z in our axes (up y).
fn at(x: f32, z: f32) -> Vec3 {
    Vec3::new(x, z, 0.0)
}

#[test]
fn level_shots_fly_as_the_table_gives() {
    // Pistol bullet: update 1 (10, 0.61822), 2 (20, 0.61287); ends on update 4 at 40.
    let (pos, end, _) = shoot(PISTOL, true, None, 10);
    assert!(near(pos[0].0, at(10.0, 0.61822), 1e-4) && (pos[0].1 - 10.000001).abs() < 1e-4, "{:?}", pos[0]);
    assert!(near(pos[1].0, at(20.0, 0.61287), 1e-4));
    assert_eq!(end.map(|e| e.0), Some(4));
    assert!(near(pos[3].0, at(40.0, 0.59148), 1e-4), "{:?}", pos[3]);
    // Assault rifle bullet.
    let (pos, end, _) = shoot(ASSAULT_RIFLE, true, None, 10);
    assert!(near(pos[0].0, at(10.8, 0.61822), 1e-4) && near(pos[1].0, at(21.6, 0.61287), 1e-4));
    assert_eq!(end.map(|e| e.0), Some(4));
    assert!(near(pos[3].0, at(40.0, 0.59518), 1e-4), "{:?}", pos[3]);
    // Plasma rifle bolt: no gravity, slowing at once.
    let (pos, end, _) = shoot(PLASMA_RIFLE, true, None, 40);
    for (k, x, s) in [(1, 1.64931, 1.631944), (2, 3.26389, 1.597222), (5, 7.89931, 1.493056), (10, 14.93056, 1.319444)] {
        assert!(near(pos[k - 1].0, at(x, 0.62), 1e-4) && (pos[k - 1].1 - s).abs() < 1e-4, "update {k}: {:?}", pos[k - 1]);
    }
    assert!((pos[23].0.x - 30.0).abs() < 1e-3, "at the final speed after update 24: {:?}", pos[23]);
    let (k, e) = end.unwrap();
    assert!((25..=26).contains(&k) && e == End::Disappear, "ends on update {k}");
    assert!(pos.last().unwrap().0.x <= 30.826, "{:?}", &pos[22..]);
    // Shotgun pellet: full speed for three updates, then slowed.
    let (pos, end, _) = shoot(SHOTGUN, true, None, 20);
    assert!(near(pos[0].0, at(4.66667, 0.61822), 1e-4) && near(pos[1].0, at(9.33333, 0.61287), 1e-4));
    assert!((pos[3].0.x - 17.55624).abs() < 1e-3 && (pos[3].1 - 3.30001).abs() < 1e-4, "{:?}", pos[3]);
    assert!(near(pos[4].0, at(20.85624, 0.58111), 1e-3), "{:?}", pos[4]);
    assert!(near(pos[9].0, at(37.35596, 0.46308), 1e-3), "{:?}", pos[9]);
    assert_eq!(end.map(|e| e.0), Some(11));
    assert!((pos[10].0.x - 39.99938).abs() < 1e-3);
}

#[test]
fn bullets_reach_10_50_and_100_metres_on_the_tables_updates() {
    // Behaviour 4: hit at 3.28, 16.4, 32.8 WU (a wall there, no floor).
    for (id, want) in [(PISTOL, [1, 2, 4]), (ASSAULT_RIFLE, [1, 2, 4]), (SHOTGUN, [1, 4, 9]), (PLASMA_RIFLE, [3, 12, 0])] {
        for (d, want) in [3.28f32, 16.4, 32.8].into_iter().zip(want) {
            let (_, _, hits) = shoot(id, false, Some(d), 60);
            let got = hits.first().map_or(0, |h| h.0);
            assert_eq!(got, want, "{id} at {d} WU");
        }
    }
}

#[test]
fn a_direct_hits_amount_follows_the_speed_left() {
    let effects = DamageEffects::load();
    // (b): pistol at 5 (update 1, t 0.5) and 30 (update 4, t 0) does 25.
    let (_, _, h) = shoot(PISTOL, true, Some(5.0), 10);
    assert_eq!(h[0].0, 1);
    assert!((h[0].1 - 0.5).abs() < 1e-3 && h[0].3 == 1.0);
    let (_, _, h) = shoot(PISTOL, true, Some(30.0), 10);
    assert_eq!((h[0].0, h[0].3), (4, 1.0));
    assert!(h[0].1.abs() < 1e-4);
    let (_, _, h) = shoot(PISTOL, true, Some(60.0), 10);
    assert!(h.is_empty(), "gone at 40 WU");
    let pistol = effects.get(effect_id("pistol_bullet").unwrap()).unwrap();
    assert_eq!(pistol.amount(1.0, 0.5), 25.0);
    // Plasma rifle bolt at 5 and 20.
    let bolt = effects.get(effect_id("plasma_rifle_bolt").unwrap()).unwrap();
    for (d, k, t, speed, scale, lo, hi) in [(5.0, 4, 0.10112, 1.55899, 0.8708, 11.742, 13.483), (20.0, 15, 0.0597, 1.17848, 0.4142, 10.828, 11.657)] {
        let (_, _, h) = shoot(PLASMA_RIFLE, true, Some(d), 40);
        let (uk, ut, us, s) = h[0];
        assert_eq!(uk, k, "bolt at {d}");
        assert!((ut - t).abs() < 1e-3 && (us - speed).abs() < 1e-4 && (s - scale).abs() < 1e-4, "{:?}", h[0]);
        assert!((bolt.amount(s, 0.0) - lo).abs() < 1e-2 && (bolt.amount(s, 1.0) - hi).abs() < 1e-2);
    }
    // Pellet at 15: update 4, its speed restored to the start's, scale 1;
    // at 20: update 5 at the final speed, scale 0, 8 points.
    let pellet = effects.get(effect_id("shotgun_pellet").unwrap()).unwrap();
    let (_, _, h) = shoot(SHOTGUN, true, Some(15.0), 20);
    assert_eq!(h[0].0, 4);
    assert!((h[0].1 - 0.2812).abs() < 1e-3 && (h[0].2 - 4.66668).abs() < 1e-4 && h[0].3 == 1.0, "{:?}", h[0]);
    let (_, _, h) = shoot(SHOTGUN, true, Some(20.0), 20);
    assert_eq!(h[0].0, 5);
    assert!((h[0].1 - 0.74053).abs() < 1e-3 && (h[0].2 - 3.30001).abs() < 1e-4 && h[0].3 == 0.0, "{:?}", h[0]);
    assert_eq!(pellet.amount(0.0, 0.7), 8.0);
}
