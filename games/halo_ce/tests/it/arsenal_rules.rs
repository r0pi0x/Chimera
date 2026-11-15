//! The M3 arsenal's rules on their own (`game::weapons::tick`, `flight`,
//! `explode`, `grenades`), tick by tick with the data's numbers: the test
//! cases of specs/halo_ce/weapons.md (the plasma pistol's charge and
//! overheat timelines, the sniper rifle's semi-automatic taps, zoom and
//! spread, the reloads of the sniper, needler, rocket launcher and
//! flamethrower) and projectiles.md (the level shots of (a) for the rocket,
//! needle, plasma pistol bolt, sniper bullet and flame; the grenades on
//! stone of (c); the area damage and pushes of (d); the needle's and
//! charged bolt's guidance of (e); the needle's timers and responses of
//! (f)). Their timelines in the game are `arsenal`'s scenario tests.

// The spec's numbers are written as it states them.
#![allow(clippy::excessive_precision)]

use bevy::math::Vec3;
use halo_ce::game::{
    vitality::{DamageEffects, ids},
    weapons::{
        FLAMETHROWER, NEEDLER, PLASMA_PISTOL, ROCKET_LAUNCHER, SNIPER_RIFLE,
        explode::{self, Candidate},
        flight::{End, Flight, Hit, Steer},
        grenades::{GrenadeData, Throw, ThrowEvent},
        gun,
        params::{Gun, Projectile},
        random::HaloRng,
        tick::{GunEvent, HaloGun, Inputs},
    },
};

const AIM: Vec3 = Vec3::NEG_Z;

fn held(fire: bool) -> Inputs {
    Inputs { fire, held: true, aim: AIM, ..Default::default() }
}

fn ready(g: &Gun) -> HaloGun {
    let mut w = HaloGun::new(g);
    w.in_hand = true;
    // Long in hand: the trigger was released.
    w.released_since_shot = true;
    w
}

/// (tick, event) of every tick of `input`.
fn run(g: &Gun, w: &mut HaloGun, ticks: u32, input: impl Fn(u32) -> Inputs) -> Vec<(u32, GunEvent)> {
    let mut out = Vec::new();
    for t in 0..ticks {
        for e in w.tick(g, &input(t)) {
            out.push((t, e));
        }
    }
    out
}

fn fired(log: &[(u32, GunEvent)], trigger: u8) -> Vec<u32> {
    log.iter().filter(|(_, e)| matches!(e, GunEvent::Fired { trigger: t, .. } if *t == trigger)).map(|(t, _)| *t).collect()
}

fn at(log: &[(u32, GunEvent)], want: &GunEvent) -> Vec<u32> {
    log.iter().filter(|(_, e)| e == want).map(|(t, _)| *t).collect()
}

// ---------------------------------------------------------------------------
// The plasma pistol (weapons.md 10, "Test cases").

#[test]
fn the_plasma_pistol_held_charges_and_never_fires() {
    let g = gun(PLASMA_PISTOL);
    let mut w = ready(&g);
    let log = run(&g, &mut w, 150, |_| held(true));
    assert!(fired(&log, 0).is_empty() && fired(&log, 1).is_empty(), "{log:?}");
    assert_eq!(at(&log, &GunEvent::Charged), [18]);
}

#[test]
fn the_plasma_pistol_tapped_fires_on_each_release_and_overheats() {
    let g = gun(PLASMA_PISTOL);
    let mut w = ready(&g);
    let mut heats = Vec::new();
    let mut log = Vec::new();
    for t in 0..60 {
        for e in w.tick(&g, &held(t % 2 == 0)) {
            if matches!(e, GunEvent::Fired { .. }) {
                heats.push(w.heat);
            }
            log.push((t, e));
        }
    }
    let shots = fired(&log, 0);
    assert_eq!(&shots[..9], &[1, 3, 5, 7, 9, 11, 13, 15, 17]);
    for (h, want) in heats.iter().zip([0.16, 0.2767, 0.3933, 0.51, 0.6267, 0.7433, 0.86, 0.9767, 1.0]) {
        assert!((h - want).abs() < 1e-3, "{heats:?}");
    }
    assert_eq!(at(&log, &GunEvent::Overheated), [18]);
    assert_eq!(at(&log, &GunEvent::OverheatCleared), [52]);
    assert_eq!(shots[9], 53);
}

#[test]
fn the_plasma_pistols_charge_and_its_release() {
    let g = gun(PLASMA_PISTOL);
    // Held 0 to 16, released on 17: a normal bolt on 17.
    let mut w = ready(&g);
    let log = run(&g, &mut w, 30, |t| held(t < 17));
    assert_eq!((fired(&log, 0), fired(&log, 1)), (vec![17], vec![]));
    // Held 0 to 17, released on 18: charged on 18, the charged bolt on 19,
    // overheated on 20, clear on 54.
    let mut w = ready(&g);
    let log = run(&g, &mut w, 70, |t| held(t < 18));
    assert_eq!(at(&log, &GunEvent::Charged), [18]);
    assert_eq!((fired(&log, 0), fired(&log, 1)), (vec![], vec![19]));
    assert_eq!(at(&log, &GunEvent::Overheated), [20]);
    assert_eq!(at(&log, &GunEvent::OverheatCleared), [54]);
    // Held 0 to 39: the charged bolt on 40 with heat 1 and battery 0.11,
    // overheated 41, clear 75 (heat 0.2417).
    let mut w = ready(&g);
    let mut log = Vec::new();
    let mut after = (0.0, 0.0);
    for t in 0..80 {
        for e in w.tick(&g, &held(t < 40)) {
            if matches!(e, GunEvent::Fired { trigger: 1, .. }) {
                after = (w.heat, w.age);
            }
            if e == GunEvent::OverheatCleared {
                assert!((w.heat - 0.2417).abs() < 1e-3, "{}", w.heat);
            }
            log.push((t, e));
        }
    }
    assert_eq!(fired(&log, 1), [40]);
    assert!((after.0 - 1.0).abs() < 1e-6 && (after.1 - 0.11).abs() < 1e-6, "{after:?}");
    assert_eq!(at(&log, &GunEvent::Overheated), [41]);
    assert_eq!(at(&log, &GunEvent::OverheatCleared), [75]);
}

#[test]
fn the_plasma_pistols_charge_holds_for_ever() {
    let g = gun(PLASMA_PISTOL);
    let mut w = ready(&g);
    let mut log = Vec::new();
    for t in 0..1540 {
        for e in w.tick(&g, &held(t < 1500)) {
            log.push((t, e));
        }
        match t {
            1367 => assert!((w.overcharged - 0.9992592).abs() < 1e-6, "{}", w.overcharged),
            1368..1500 => assert_eq!(w.overcharged, 1.0, "tick {t}"),
            _ => {}
        }
    }
    // Nothing on 1368 (the overcharged action is none), the charged bolt
    // on the release.
    assert_eq!((fired(&log, 0), fired(&log, 1)), (vec![], vec![1500]));
    assert_eq!(at(&log, &GunEvent::Overheated), [1501]);
    assert_eq!(at(&log, &GunEvent::OverheatCleared), [1535]);
    assert!(at(&log, &GunEvent::Detonated).is_empty());
}

// ---------------------------------------------------------------------------
// The sniper rifle and the others' reloads.

#[test]
fn the_sniper_rifle_is_semi_automatic_and_zooms_twice() {
    let g = gun(SNIPER_RIFLE);
    let mut w = ready(&g);
    let log = run(&g, &mut w, 150, |_| held(true));
    assert_eq!(fired(&log, 0), [0], "held: one shot");
    // Tapped every other tick: 0, 16, 32, 48 (15 ticks on the 2-tick
    // pattern), the reload after the fourth starting on 52.
    let mut w = ready(&g);
    let log = run(&g, &mut w, 60, |t| held(t % 2 == 0));
    assert_eq!(fired(&log, 0), [0, 16, 32, 48]);
    assert_eq!(log.iter().find(|(_, e)| matches!(e, GunEvent::ReloadStarted { .. })).map(|x| x.0), Some(52));
    // Zoom presses: 2x, 8x, none.
    let mut w = ready(&g);
    let mut levels = Vec::new();
    for k in 0..6 {
        w.tick(&g, &Inputs { zoom: k % 2 == 0, held: true, aim: AIM, ..Default::default() });
        if k % 2 == 0 {
            levels.push(g.magnification(w.zoom).round() as i32 * if w.zoom >= 0 { 1 } else { 0 });
        }
    }
    assert_eq!(levels, [2, 8, 0]);
}

#[test]
fn the_sniper_rifle_has_no_spread_zoomed_and_half_a_degree_unzoomed() {
    let g = gun(SNIPER_RIFLE);
    for zoomed in [true, false] {
        for _ in 0..20 {
            let mut w = ready(&g);
            w.rng = rand_seed();
            w.zoom = if zoomed { 0 } else { -1 };
            let ev = w.tick(&g, &held(true));
            let Some(GunEvent::Fired { projectiles, .. }) = ev.into_iter().find(|e| matches!(e, GunEvent::Fired { .. })) else { panic!() };
            let a = projectiles[0].0.angle_between(AIM).to_degrees();
            if zoomed {
                assert!(a < 1e-3, "{a}");
            } else {
                assert!(a <= 0.5 + 1e-3, "{a}");
            }
        }
    }
}

fn rand_seed() -> u32 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(1);
    N.fetch_add(0x9E37_79B9, Ordering::Relaxed)
}

#[test]
fn reloads_from_empty_take_the_reload_animation() {
    // weapons.md 11: (weapon, start, rounds in, first shot).
    for (id, start, rounds_in, next) in [(SNIPER_RIFLE, 4, 97, 99), (NEEDLER, 1, 70, 72), (ROCKET_LAUNCHER, 1, 125, 127), (FLAMETHROWER, 1, 104, 106)] {
        let g = gun(id);
        let mut w = ready(&g);
        w.loaded = 1;
        w.reserve = 50;
        let log = run(&g, &mut w, next + 2, |t| held(t == 0 || t >= next - 5));
        let first = |f: &dyn Fn(&GunEvent) -> bool| log.iter().find(|(_, e)| f(e)).map(|x| x.0);
        assert_eq!(first(&|e| matches!(e, GunEvent::ReloadStarted { .. })), Some(start), "{id}");
        assert_eq!(first(&|e| matches!(e, GunEvent::RoundsIn { .. })), Some(rounds_in), "{id}");
        assert_eq!(fired(&log, 0)[..2], [0, next], "{id}");
    }
}

// ---------------------------------------------------------------------------
// Projectiles: (a) level shots, (c) grenades, (e) guidance, (f) timers.

/// A flat stone floor at height 0 (if `floor`) and a character's wall at
/// x = `wall` (WU).
fn world(floor: bool, wall: Option<f32>) -> impl FnMut(Vec3, Vec3, Option<u8>) -> Option<Hit<u8>> {
    move |a: Vec3, b: Vec3, ignored: Option<u8>| {
        let mut best: Option<Hit<u8>> = None;
        if floor && b.y < 0.0 && a.y >= 0.0 {
            let t = a.y / (a.y - b.y);
            best = Some(Hit::level(t, a + (b - a) * t, Vec3::Y, Some(2)));
        }
        if let Some(x) = wall
            && ignored.is_none()
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

/// Updates of `p` from `from` with `v` until it ends (at most `max`):
/// the positions and speeds after each, its end, the hits (update, hit).
#[allow(clippy::type_complexity)]
fn fly(p: &Projectile, from: Vec3, v: Vec3, floor: bool, wall: Option<f32>, max: u32) -> (Vec<(Vec3, f32)>, Option<(u32, End)>, Vec<(u32, Hit<u8>)>) {
    let mut f = Flight::new(p, from, v.normalize_or(Vec3::X), 0.0);
    f.velocity = if p.initial_velocity == 0.0 { v } else { f.velocity };
    let mut trace = world(floor, wall);
    let mut out = Vec::new();
    let mut hits = Vec::new();
    let mut rng = HaloRng(7);
    let mut ignored = None;
    for k in 1..=max {
        let mut seen = Vec::new();
        let mut tr = |a: Vec3, b: Vec3, i: Option<u8>| {
            let h = trace(a, b, i);
            if let Some(h) = h {
                seen.push(h);
            }
            h
        };
        let mut struck = |_: &Hit<u8>, _: Vec3, _: f32| None;
        let m = f.update(p, &mut ignored, &mut rng, None, &mut tr, &mut struck);
        hits.extend(seen.into_iter().map(|h| (k, h)));
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

/// The spec's (x, z) (up z) in our axes (up y).
fn xz(x: f32, z: f32) -> Vec3 {
    Vec3::new(x, z, 0.0)
}

const MUZZLE: Vec3 = Vec3::new(0.0, 0.62, 0.0);

#[test]
fn the_arsenals_level_shots_fly_as_the_table_gives() {
    // Rocket: slowing from update 1, final speed after 273 (x 100.089),
    // explodes at 128 on update 358.
    let rocket = gun(ROCKET_LAUNCHER).projectile;
    let (pos, end, _) = fly(&rocket, MUZZLE, Vec3::X, true, None, 400);
    for (k, x, s) in [(1, 0.39988, 0.399756), (2, 0.79951, 0.399511), (5, 1.99694, 0.398778), (10, 3.98778, 0.397556)] {
        assert!(near(pos[k - 1].0, xz(x, 0.62), 1e-4) && (pos[k - 1].1 - s).abs() < 1e-5, "rocket update {k}: {:?}", pos[k - 1]);
    }
    assert!((pos[272].0.x - 100.089).abs() < 2e-3 && (pos[272].1 - 0.33).abs() < 0.004, "{:?}", pos[272]);
    assert_eq!(end, Some((358, End::Detonate)));
    assert!((pos[357].0.x - 128.0).abs() < 1e-3);
    // Needle: 0.13333 an update, the range's detonation at 20 on update 150.
    let needle = gun(NEEDLER).projectile;
    let (pos, end, _) = fly(&needle, MUZZLE, Vec3::X, true, None, 200);
    for (k, x) in [(1, 0.13333), (2, 0.26667), (5, 0.66667), (10, 1.33333)] {
        assert!(near(pos[k - 1].0, xz(x, 0.62), 1e-4), "needle update {k}: {:?}", pos[k - 1]);
    }
    // Update 151 in single precision (projectiles.md (a): 150 updates end
    // 1.3e-5 short of the range, the 151st moves that and detonates).
    let (k, e) = end.unwrap();
    assert!(k == 151 && e == End::Detonate, "{k}");
    assert!((pos[k as usize - 1].0.x - 20.0).abs() < 1e-3);
    // Plasma pistol bolt: gravity 0.1, the floor on update 65 at 39.7756.
    let bolt = gun(PLASMA_PISTOL).projectile;
    let (pos, end, hits) = fly(&bolt, MUZZLE, Vec3::X, true, None, 100);
    assert!(near(pos[0].0, xz(0.82963, 0.61982), 1e-4) && (pos[0].1 - 0.825926).abs() < 1e-5, "{:?}", pos[0]);
    assert!(near(pos[1].0, xz(1.65185, 0.61929), 1e-4) && near(pos[4].0, xz(4.07407, 0.61559), 1e-4));
    assert!(near(pos[9].0, xz(7.96296, 0.60264), 1e-4) && (pos[9].1 - 0.759267).abs() < 1e-5, "{:?}", pos[9]);
    assert_eq!(end, Some((65, End::Disappear)));
    assert!((hits[0].1.point.x - 39.7756).abs() < 2e-3, "{:?}", hits[0]);
    // Sniper bullet: 33.33 an update, its range at 1000 on update 31.
    let sniper = gun(SNIPER_RIFLE).projectile;
    let (pos, end, _) = fly(&sniper, MUZZLE, Vec3::X, true, None, 40);
    assert!(near(pos[0].0, xz(33.33333, 0.62), 1e-3) && near(pos[9].0, xz(333.33332, 0.62), 1e-2));
    assert_eq!(end, Some((31, End::Detonate)));
    // Flame: 0.33333 an update, explodes at 2.5 on update 8.
    let flame = gun(FLAMETHROWER).projectile;
    let (pos, end, _) = fly(&flame, MUZZLE, Vec3::X, true, None, 20);
    assert!(near(pos[0].0, xz(0.33333, 0.62), 1e-4) && near(pos[4].0, xz(1.66667, 0.62), 1e-4));
    assert_eq!(end, Some((8, End::Detonate)));
    assert!((pos[7].0.x - 2.5).abs() < 1e-4);
}

/// A grenade thrown at pitch `p` from 0.62 WU up: its bounces (update,
/// t, point, velocity after) and when and where it detonated.
#[allow(clippy::type_complexity)]
fn throw(p: &Projectile, pitch: f32, speed: f32) -> (Vec<(u32, f32, Vec3)>, Vec<(Vec3, Vec3)>, Option<(u32, End)>, Vec<(Vec3, f32)>) {
    let mut f = Flight::new(p, MUZZLE, Vec3::X, 0.0);
    f.velocity = Vec3::new(pitch.cos(), pitch.sin(), 0.0) * speed;
    let mut rng = HaloRng(3);
    let mut bounces = Vec::new();
    let mut after = Vec::new();
    let mut path = Vec::new();
    let mut ignored = None;
    for k in 1..=200 {
        let mut seen = Vec::new();
        let mut tr = |a: Vec3, b: Vec3, _: Option<u8>| {
            let h = (b.y < 0.0 && a.y >= 0.0).then(|| {
                let t = a.y / (a.y - b.y);
                Hit::level(t, a + (b - a) * t, Vec3::Y, Some(2))
            });
            if let Some(h) = h {
                seen.push((k, h.t, h.point));
            }
            h
        };
        let mut struck = |_: &Hit<u8>, _: Vec3, _: f32| None;
        let m = f.update(p, &mut ignored, &mut rng, None, &mut tr, &mut struck);
        if !seen.is_empty() {
            after.push((f.position, f.velocity));
        }
        bounces.extend(seen);
        path.push((f.position, f.speed()));
        if let Some(e) = m.end {
            return (bounces, after, Some((k, e)), path);
        }
    }
    (bounces, after, None, path)
}

#[test]
fn grenades_bounce_rest_and_detonate_on_stone_as_the_table_gives() {
    let g = GrenadeData::load();
    let v = g.throw.velocity / 30.0;
    // The velocity just after a bounce: gravity of the tick's rest taken
    // back.
    let bounced = |after: Vec3, t: f32| after + Vec3::Y * halo_ce::game::weapons::params::GRAVITY * (1.0 - t);
    // Frag, p = 0.2: before the first bounce, then 45, 61, 66; at rest on
    // 66; detonates on 81 at (15.75877, 0, 0).
    let (b, after, end, path) = throw(&g.frag, 0.2, v);
    assert!(near(path[0].0, xz(0.32669, 0.68444), 1e-4), "{:?}", path[0]);
    assert!(near(path[9].0, xz(3.26689, 1.10397), 1e-4) && near(path[19].0, xz(6.53378, 1.23143), 1e-4));
    assert!(near(path[29].0, xz(9.80067, 1.00236), 1e-4));
    let ticks: Vec<u32> = b.iter().map(|x| x.0).collect();
    assert_eq!(ticks, [45, 61, 66], "{b:?}");
    assert!((b[0].1 - 0.89501).abs() < 1e-3 && near(b[0].2, xz(14.6667, 0.0), 1e-3), "{b:?}");
    assert!(near(bounced(after[0].1, b[0].1), xz(0.065338, 0.028151), 1e-5), "{after:?}");
    assert!(near(b[1].2, xz(15.6976, 0.0), 1e-3) && near(bounced(after[1].1, b[1].1), xz(0.013068, 0.00843), 1e-5), "{after:?}");
    assert!(near(b[2].2, xz(15.75877, 0.0), 1e-3));
    assert_eq!(end, Some((81, End::Detonate)));
    assert!(near(path[80].0, xz(15.75877, 0.0), 1e-3));
    // Frag, p = 0: 19, 30, 34; detonates on 49.
    let (b, _, end, _) = throw(&g.frag, 0.0, v);
    assert_eq!(b.iter().map(|x| x.0).collect::<Vec<_>>(), [19, 30, 34]);
    assert!(near(b[0].2, xz(6.21449, 0.0), 1e-3) && near(b[2].2, xz(7.00352, 0.0), 1e-3), "{b:?}");
    assert_eq!(end, Some((49, End::Detonate)));
    // Dropped: 19, 30 (stopped); timer and arming reach 1 on 45.
    let (b, after, end, _) = throw(&g.frag, 0.0, 0.0);
    assert_eq!(b.iter().map(|x| x.0).collect::<Vec<_>>(), [19, 30]);
    assert!(near(bounced(after[0].1, b[0].1), xz(0.0, 0.01994), 1e-5), "{after:?}");
    assert_eq!(end, Some((45, End::Detonate)));
    // Plasma, p = 0.2: 45, 51 (stopped); 61 timer updates from 52: 112.
    let (b, after, end, _) = throw(&g.plasma, 0.2, v);
    assert_eq!(b.iter().map(|x| x.0).collect::<Vec<_>>(), [45, 51]);
    assert!(near(bounced(after[0].1, b[0].1), xz(0.016334, 0.009384), 1e-5), "{after:?}");
    assert_eq!(end, Some((112, End::Detonate)));
}

#[test]
fn a_plasma_grenade_sticks_to_a_spartan_and_a_frag_bounces_off() {
    let g = GrenadeData::load();
    // Thrown level at a Spartan 2 WU ahead, no floor.
    let (_, end, hits) = fly(&g.plasma, MUZZLE, Vec3::X * (10.0 / 30.0), false, Some(2.0), 200);
    let stuck_on = hits[0].0;
    assert_eq!(end, Some((stuck_on + 61, End::Detonate)), "61 updates after sticking");
    let mut f = Flight::new(&g.frag, MUZZLE, Vec3::X, 0.0);
    f.velocity = Vec3::X * (10.0 / 30.0);
    let mut w = world(false, Some(2.0));
    let mut ignored = None;
    for _ in 0..10 {
        f.update(&g.frag, &mut ignored, &mut HaloRng(1), None, &mut w, &mut |_, _, _| None);
    }
    assert!(!f.stuck && f.velocity.x < 0.0, "bounced back: {:?}", f.velocity);
    // 0.8 / 0.9 on the cyborg body: the normal speed kept at 20 %.
    assert!((f.velocity.x + 0.2 * 10.0 / 30.0).abs() < 1e-4, "{:?}", f.velocity);
}

#[test]
fn the_needle_sticks_and_detonates_23_updates_later() {
    let needle = gun(NEEDLER).projectile;
    let (_, end, hits) = fly(&needle, MUZZLE, Vec3::X, false, Some(1.0), 100);
    assert_eq!(end, Some((hits[0].0 + 23, End::Detonate)));
    // Into stone head-on (60 degrees): attached, harmless 23 updates on.
    let (_, end, hits) = fly(&needle, Vec3::new(0.0, 0.2, 0.0), Vec3::new(1.0, -(60f32.to_radians().tan()), 0.0), true, None, 100);
    assert_eq!(end, Some((hits[0].0 + 23, End::Detonate)));
    // Grazing stone at 20 degrees with the noise stubbed to 0: reflected,
    // 0.7 of each part kept, flying on (0.09333 > 0.06667).
    let mut grazing = needle.clone();
    for e in &mut grazing.responses.elements {
        e.angular_noise = None;
    }
    // Its speed kept as the bounce leaves it (no final-speed floor).
    grazing.final_velocity = 0.05;
    grazing.air_damage_range = [0.0, 0.0];
    let dir = Vec3::new(20f32.to_radians().cos(), -(20f32.to_radians().sin()), 0.0);
    let mut f = Flight::new(&grazing, Vec3::new(0.0, 0.01, 0.0), dir, 0.0);
    let v0 = f.velocity;
    let mut w = world(true, None);
    let mut ignored = None;
    let m = f.update(&grazing, &mut ignored, &mut HaloRng(1), None, &mut w, &mut |_, _, _| None);
    assert!(m.end.is_none() && !f.stuck);
    assert!((f.velocity.y - (-v0.y) * 0.7).abs() < 1e-5 && (f.velocity.x - v0.x * 0.7).abs() < 1e-5, "{:?}", f.velocity);
    assert!((f.speed() - 0.09333).abs() < 1e-4);
}

/// A needle (or the charged bolt) at `omega` per update toward a body
/// crossing at 2.25 WU/s 8 WU ahead: positions (spec x, y) and headings
/// (degrees) after each update.
fn chase(p: &Projectile, omega: f32, updates: u32) -> Vec<(f32, f32, f32)> {
    let mut f = Flight::new(p, MUZZLE, Vec3::X, 0.0);
    let mut out = Vec::new();
    let mut ignored: Option<u8> = None;
    for k in 1..=updates {
        let target = Vec3::new(8.0, 0.62, 0.075 * (k - 1) as f32);
        f.update(p, &mut ignored, &mut HaloRng(1), Some(Steer { target, omega }), &mut |_, _, _| None, &mut |_, _, _| None);
        out.push((f.position.x, f.position.z, f.velocity.z.atan2(f.velocity.x).to_degrees()));
    }
    out
}

#[test]
fn guidance_turns_by_the_full_rate_each_update() {
    let needle = gun(NEEDLER).projectile;
    // The multiplayer rate: the guided rate / 30 times the "guidance vs
    // player" factor of normal, 0.5 (projectiles.md 12): 1.5 degrees.
    let omega = needle.guided_angular_velocity / 30.0 * halo_ce::game::weapons::systems::GUIDANCE_VS_PLAYER;
    assert!((omega - 0.0261799).abs() < 1e-6, "{omega}");
    let c = chase(&needle, omega, 100);
    assert!(c[0].2.abs() < 1e-3, "{:?}", &c[..3]);
    // The headings after updates 2 to 12.
    for (k, h) in (2..=12).zip([1.5, 0.0, 1.5, 3.0, 1.5, 3.0, 4.5, 6.0, 4.5, 6.0, 7.5]) {
        assert!((c[k - 1].2 - h).abs() < 0.05, "update {k}: {:?}", c[k - 1]);
    }
    // The corrected table, in 32-bit positions with the Quirks 11 rule
    // (the needle's speed stays 0.13333334): positions to 0.01 WU,
    // headings to 0.05 degrees. The table's heading for update 60, 57.0, is
    // not its path's: the step from update 60 to 61 is at 55.5 degrees
    // (57.0 comes one update later); its positions agree to 1e-4.
    for (k, x, y, h) in [(1, 0.1333, 0.0, 0.0), (2, 0.2667, 0.0, 1.5), (10, 1.3317, 0.0488, 4.5), (20, 2.6502, 0.2400, 13.5), (40, 5.1343, 1.1786, 31.5), (60, 7.0611, 2.9878, 55.5)] {
        let (px, py, ph) = c[k - 1];
        assert!((px - x).abs() < 0.01 && (py - y).abs() < 0.01 && (ph - h).abs() < 0.05, "update {k}: {:?}", c[k - 1]);
    }
    // Never slowed by rounding.
    let mut f = Flight::new(&needle, MUZZLE, Vec3::X, 0.0);
    for k in 1..=100u32 {
        let target = Vec3::new(8.0, 0.62, 0.075 * (k - 1) as f32);
        f.update(&needle, &mut None::<u8>, &mut HaloRng(1), Some(Steer { target, omega }), &mut |_, _, _| None, &mut |_, _, _| None);
        assert!((f.speed() - needle.final_velocity).abs() < 1e-6, "update {k}: speed {}", f.speed());
    }
    // The AI's rate (factor 1, not in multiplayer) overshoots: 3, 0, 3
    // degrees after 2, 3, 4.
    let c = chase(&needle, 0.0523599, 60);
    for (k, h) in (2..=12).zip([3.0, 0.0, 3.0, 0.0, 3.0, 6.0, 3.0, 6.0, 3.0, 6.0, 9.0]) {
        assert!((c[k - 1].2 - h).abs() < 0.05, "update {k}: {:?}", c[k - 1]);
    }
    for (k, x, y, h) in [(10, 1.3311, 0.0558, 3.0), (20, 2.6497, 0.2435, 15.0), (40, 5.1323, 1.1815, 33.0), (60, 7.0677, 2.9798, 57.0)] {
        let (px, py, ph) = c[k - 1];
        assert!((px - x).abs() < 0.01 && (py - y).abs() < 0.01 && (ph - h).abs() < 0.05, "AI update {k}: {:?}", c[k - 1]);
    }
    // The charged bolt at the player rate (0.0084357).
    let bolt = gun(PLASMA_PISTOL).secondary.unwrap().projectile;
    let omega = bolt.guided_angular_velocity / 30.0 * halo_ce::game::weapons::systems::GUIDANCE_VS_PLAYER;
    assert!((omega - 0.0084357).abs() < 1e-6, "{omega}");
    let c = chase(&bolt, omega, 20);
    for (k, h) in [(2, 0.483), (10, 4.35), (20, 8.7)] {
        assert!((c[k - 1].2 - h).abs() < 0.02, "update {k}: {:?}", c[k - 1]);
    }
}

/// The wander offset's length by the distance between the centres
/// (projectiles.md 12 step 2): 1 WU at 12, 0.5 at 6, 0 at 1.5.
#[test]
fn the_wander_offset_grows_with_the_distance() {
    use halo_ce::game::weapons::flight::wandered;
    for (e, len) in [(12.0, 1.0), (6.0, 0.5), (1.5, 0.0), (10.0, 1.0)] {
        for tick in [0, 17, 4000] {
            let got = (wandered(Vec3::ZERO, e, tick, 5) - Vec3::ZERO).length();
            assert!((got - len).abs() < 1e-5, "e = {e}, tick {tick}: {got}");
        }
    }
}

// ---------------------------------------------------------------------------
// (d) Area damage and push.

fn blast(id: halo_ce::game::vitality::EffectId, d: f32) -> Option<(f32, f32, f32)> {
    let effects = DamageEffects::load();
    let fx = effects.get(id).unwrap();
    let c = [Candidate { object: 0u8, centre: Vec3::X * d, radius: 0.0, unit: true, owner: false }];
    let lo = explode::area(fx, Vec3::ZERO, &c, &mut |_, _| true, &mut || 0.0);
    let hi = explode::area(fx, Vec3::ZERO, &c, &mut |_, _| true, &mut || 1.0);
    lo.first().map(|h| (h.scale, h.amount, hi[0].amount))
}

#[test]
fn area_damage_falls_off_as_the_table_gives() {
    let close = |a: f32, b: f32| (a - b).abs() < 1e-3;
    for d in [0.0, 0.5, 1.0, 1.5] {
        assert!(blast(ids::FRAG_EXPLOSION, d).is_some_and(|x| close(x.1, 120.0)), "frag at {d}");
    }
    assert!(blast(ids::FRAG_EXPLOSION, 2.0).is_some_and(|x| close(x.0, 0.5) && close(x.1, 100.0)));
    for (d, want) in [(0.0, 120.0), (0.5, 120.0), (1.0, 106.667), (1.5, 93.333)] {
        assert!(blast(ids::PLASMA_GRENADE_EXPLOSION, d).is_some_and(|x| close(x.1, want)), "plasma at {d}");
    }
    assert!(blast(ids::PLASMA_GRENADE_EXPLOSION, 2.0).is_none());
    for (d, lo, hi) in [(0.0, 300.0, 330.0), (0.5, 300.0, 330.0), (1.0, 226.667, 246.667), (1.5, 153.333, 163.333)] {
        assert!(blast(ids::ROCKET_EXPLOSION, d).is_some_and(|x| close(x.1, lo) && close(x.2, hi)), "rocket at {d}: {:?}", blast(ids::ROCKET_EXPLOSION, d));
    }
    assert!(blast(ids::ROCKET_EXPLOSION, 2.0).is_none());
    assert!(blast(ids::NEEDLER_EXPLOSION, 0.5).is_some_and(|x| close(x.1, 60.0)));
    assert!(blast(ids::NEEDLER_EXPLOSION, 1.0).is_none());
}

#[test]
fn the_push_is_fixed_lifted_and_halved_on_a_living_biped() {
    let effects = DamageEffects::load();
    let x = GrenadeData::load().explosions;
    let frag = effects.get(ids::FRAG_EXPLOSION).unwrap();
    let rocket = effects.get(ids::ROCKET_EXPLOSION).unwrap();
    let p = explode::push(&x, frag, Vec3::X, x.biped_acceleration_scale, true).unwrap();
    assert!(near(p, Vec3::new(0.042556, 0.019150, 0.0), 1e-5) && (p.length() - 0.046667).abs() < 1e-5, "{p}");
    let p = explode::push(&x, frag, Vec3::ZERO, x.biped_acceleration_scale, true).unwrap();
    assert!(near(p, Vec3::new(0.0, 0.046667, 0.0), 1e-5));
    let p = explode::push(&x, rocket, Vec3::X, x.biped_acceleration_scale, true).unwrap();
    assert!(near(p, Vec3::new(0.063835, 0.028726, 0.0), 1e-5) && (p.length() - 0.07).abs() < 1e-5);
    let dead = explode::push(&x, rocket, Vec3::X, x.biped_acceleration_scale, false).unwrap();
    assert!((dead.length() - 0.14).abs() < 1e-5);
    let grenade = explode::push(&x, frag, Vec3::X, x.grenade_acceleration_scale, false).unwrap();
    assert!((grenade.length() - 0.26667).abs() < 1e-4);
    // At the cutoff radius or beyond: no event, no push.
    assert!(blast(ids::FRAG_EXPLOSION, 2.5).is_none());
}

#[test]
fn a_blocked_line_hides_a_candidate_but_the_core_probes_may_see_round() {
    let effects = DamageEffects::load();
    let frag = effects.get(ids::FRAG_EXPLOSION).unwrap();
    let c = [Candidate { object: 0u8, centre: Vec3::X, radius: 0.0, unit: true, owner: false }];
    // Every line blocked: unseen.
    assert!(explode::area(frag, Vec3::ZERO, &c, &mut |_, _| false, &mut || 0.0).is_empty());
    // Only the straight line blocked: a probe 0.4 WU aside sees it.
    let mut clear = |a: Vec3, b: Vec3| !(a.length() < 1e-6 && (b - Vec3::X).length() < 1e-6);
    assert_eq!(explode::area(frag, Vec3::ZERO, &c, &mut clear, &mut || 0.0).len(), 1);
}

// ---------------------------------------------------------------------------
// The throw.

#[test]
fn the_throw_creates_the_grenade_on_frame_2_and_releases_it_on_8() {
    let g = GrenadeData::load();
    let mut t = Throw { counts: [2, 0], ..Default::default() };
    let mut log = Vec::new();
    for k in 0..40 {
        for e in t.step(&g.throw, k == 0, true) {
            log.push((k, e));
        }
    }
    assert_eq!(log, [(0, ThrowEvent::Started), (2, ThrowEvent::Created(0)), (8, ThrowEvent::Released(0)), (37, ThrowEvent::Ended)]);
    assert_eq!(t.counts, [1, 0]);
    // From the camera (0, 0, 0.62), aim +X: 0.05 WU left, 10 WU/s, the
    // thrower's own speed not added.
    let (at, v) = halo_ce::game::weapons::grenades::release(&g.throw, MUZZLE, Vec3::X);
    assert!(near(at, Vec3::new(0.0, 0.62, -0.05), 1e-6) && near(v, Vec3::X * (1.0 / 3.0), 1e-6), "{at} {v}");
    // A throw cut at p = 0.5 with U = 0.03: 0.18167 along the aim.
    let cut = halo_ce::game::weapons::grenades::cut(&g.throw, v, Vec3::X, 0.5, (0.03 - 0.02) / (0.046667 - 0.02));
    assert!((cut.x - 0.18167).abs() < 1e-4, "{cut}");
    // Counts: 4 frags, capped 2 from 5 players and 1 from 9.
    assert_eq!((g.start(g.cap(4)), g.start(g.cap(6)), g.start(g.cap(12))), ([4, 0], [2, 0], [1, 0]));
}

#[test]
fn the_selected_grenade_kind_starts_on_frag_switches_to_the_other_when_it_has_any_and_follows_the_counts() {
    use halo_ce::game::weapons::grenades::{FRAG, PLASMA};
    let g = GrenadeData::load();
    // A new life: the frag; the plasma grenade only when that is all.
    assert_eq!(Throw::new([4, 0]).selected(), FRAG);
    assert_eq!(Throw::new([0, 3]).selected(), PLASMA);
    assert_eq!(Throw::new([2, 2]).selected(), FRAG);
    // The switch: nothing to change to, nothing happens; else the other.
    let mut t = Throw::new([4, 0]);
    t.switch();
    assert_eq!((t.selected(), t.current()), (FRAG, Some(FRAG)));
    t.counts[PLASMA] = 1;
    t.switch();
    assert_eq!((t.selected(), t.current()), (PLASMA, Some(PLASMA)));
    t.switch();
    assert_eq!(t.selected(), FRAG);
    // A press throws the selected kind, and the last of it passes the
    // choice to the other kind.
    t.switch();
    let mut log = Vec::new();
    for k in 0..40 {
        for e in t.step(&g.throw, k == 0, true) {
            log.push(e);
        }
    }
    assert!(log.contains(&ThrowEvent::Created(PLASMA)) && log.contains(&ThrowEvent::Released(PLASMA)));
    assert_eq!((t.counts, t.selected()), ([4, 0], FRAG));
    // An empty selected kind: no throw, the HUD still shows it.
    let mut t = Throw { counts: [0, 0], ..Throw::default() };
    assert_eq!((t.current(), t.step(&g.throw, true, true)), (None, vec![]));
    // A pickup of a kind while the selected one is out selects it; not else.
    t.counts[PLASMA] = 1;
    t.pick_up(PLASMA);
    assert_eq!(t.selected(), PLASMA);
    t.counts[FRAG] = 1;
    t.pick_up(FRAG);
    assert_eq!(t.selected(), PLASMA);
}
