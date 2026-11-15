//! specs/combat_arms/death_killcam.md's test cases: the camera math and
//! the phase machine headless (cases 1-3, 6-7, 9-21, 24-26 with the
//! machine; 12-17 the orbit), and the cases that need players in a match
//! on `CaSim` (4, 5, 8, 22, 23, 26, 27; the death screen's binding is
//! tests/ui/death_screen.rs, the name tags there too).

use bevy::prelude::*;
use combat_arms::game::death_cam::{
    machine::{Cand, DYING_TIME, Frame, Id, Input, Killer, Machine, NO_KILLER, Phase},
    math::{self, Clip, NoWalls, PullBack},
    screen::{self, Zone},
};
use combat_arms::game::death_cam::math::PITCH_LIMIT;

fn close(a: f32, b: f32, tol: f32) {
    assert!((a - b).abs() <= tol, "{a} != {b} (+-{tol})");
}

fn near(a: Vec3, b: Vec3, tol: f32) {
    assert!(a.distance(b) <= tol, "{a} != {b} (+-{tol})");
}

const BODY: Vec3 = Vec3::new(0.0, 50.0, 0.0);

// --- 1-2: the pull-back

/// 1. d = (0, 0, 1), P = (0, 50, 0), no walls: looks along -Z, level; the
/// camera at z = 4.5, 8.55, 12.195 after frames 1-3, 29.309 after frame
/// 10; never beyond 45 u from (0, 70, 0).
#[test]
fn case_01_pull_back_motion() {
    let mut pb = PullBack::begin(BODY);
    near(pb.at, Vec3::new(0.0, 70.0, 0.0), 1e-4);
    let mut z = Vec::new();
    for _ in 0..400 {
        pb.step(BODY, Vec3::Z, &NoWalls);
        z.push(pb.at);
        assert!(pb.at.distance(Vec3::new(0.0, 70.0, 0.0)) < 45.0 + 1e-3);
    }
    near(z[0], Vec3::new(0.0, 70.0, 4.5), 1e-4);
    near(z[1], Vec3::new(0.0, 70.0, 8.55), 1e-4);
    near(z[2], Vec3::new(0.0, 70.0, 12.195), 1e-4);
    near(z[9], Vec3::new(0.0, 70.0, 29.309), 1e-3);
    let (heading, pitch) = pb.look.unwrap();
    close(heading.abs(), std::f32::consts::PI, 1e-4);
    close(pitch, 0.0, 1e-5);
    // Not frozen by the 220 u rule (Quirk 2).
    assert!(!pb.frozen);
}

/// A wall plane at z = 20.
fn wall(from: Vec3, to: Vec3) -> Option<Vec3> {
    ((from.z - 20.0) * (to.z - 20.0) <= 0.0 && from.z != to.z).then(|| from + (to - from) * ((20.0 - from.z) / (to.z - from.z)))
}

/// 2. As 1 with a wall plane at z = 20: frames 1-4 as above (15.476 at
/// frame 4); frame 5's step crosses the wall: the camera is placed at
/// z = 0.9 x 20 = 18.0 and stays there.
#[test]
fn case_02_pull_back_into_a_wall() {
    let mut pb = PullBack::begin(BODY);
    let mut z = Vec::new();
    for _ in 0..40 {
        pb.step(BODY, Vec3::Z, &wall);
        z.push(pb.at.z);
    }
    for (got, want) in z.iter().zip([4.5, 8.55, 12.195, 15.4755]) {
        close(*got, want, 1e-3);
    }
    assert!(z[4..].iter().all(|v| (v - 18.0).abs() < 1e-4), "{z:?}");
    assert!(pb.frozen);
}

// --- A scene for the machine

struct Scene {
    players: Vec<Cand>,
    objects: Vec<Cand>,
    enemy_view: bool,
    any_team: bool,
    hunted: bool,
    counts: bool,
    wait_ms: f64,
    fast_ok: bool,
    my_team: u8,
    damage: Vec3,
    clip: Box<dyn Clip>,
    input: Input,
}

fn cand(id: Id, team: u8, alive: bool) -> Cand {
    Cand {
        id,
        team,
        alive,
        at: Vec3::new(id as f32 * 100.0, 50.0, 0.0),
    }
}

impl Scene {
    fn new(players: Vec<Cand>) -> Self {
        Self {
            players,
            objects: Vec::new(),
            enemy_view: false,
            any_team: false,
            hunted: false,
            counts: false,
            wait_ms: 7000.0,
            fast_ok: false,
            my_team: 1,
            damage: Vec3::Z,
            clip: Box::new(NoWalls),
            input: Input::default(),
        }
    }

    fn frame(&self, now: f64) -> Frame<'_> {
        Frame {
            now,
            dead: true,
            waiting: false,
            me: 1,
            my_team: self.my_team,
            body: BODY,
            damage: self.damage,
            players: &self.players,
            objects: &self.objects,
            enemy_view: self.enemy_view,
            any_team: self.any_team,
            hunted: self.hunted,
            counts: self.counts,
            wait_ms: self.wait_ms,
            fast_ok: self.fast_ok,
            view: (0.0, 0.0),
            input: self.input,
            clip: &*self.clip,
        }
    }

    fn set_alive(&mut self, id: Id, alive: bool) {
        self.players.iter_mut().find(|c| c.id == id).unwrap().alive = alive;
    }
}

/// The machine dead at t0 = 0 from `killer`, run to `t` in frames of
/// `step` seconds (the first frame at 0).
fn run(m: &mut Machine, s: &Scene, t: f64, step: f64) {
    let mut now = 0.0;
    while now < t - 1e-9 {
        m.frame(&s.frame(now));
        now += step;
    }
    m.frame(&s.frame(t));
}

fn dead_from(killer: Killer) -> Machine {
    let mut m = Machine::default();
    m.died(0.0, killer, (0.0, 0.0));
    m
}

fn killed_by(id: Id) -> Killer {
    Killer::Player { id, object: None }
}

/// 3. Killer known: t = 3499 ms is the pull-back, 3500 the watch.
#[test]
fn case_03_pull_back_length() {
    let s = Scene::new(vec![cand(3, 1, true), cand(5, 1, true)]);
    let mut m = dead_from(killed_by(4));
    m.frame(&s.frame(0.0));
    assert_eq!(m.phase, Phase::PullBack);
    m.frame(&s.frame(3.499));
    assert_eq!(m.phase, Phase::PullBack);
    m.frame(&s.frame(DYING_TIME));
    assert_eq!(m.phase, Phase::Watch);
}

/// 4. Enemy view on: killed by B (id 4, alive), teammates C (3) and D (5)
/// alive: at 3.5 s the camera orbits B; after 8 s it orbits D (the next
/// living teammate after id 4).
#[test]
fn case_04_enemy_view_on() {
    let mut s = Scene::new(vec![cand(3, 1, true), cand(4, 2, true), cand(5, 1, true)]);
    s.enemy_view = true;
    let mut m = dead_from(killed_by(4));
    run(&mut m, &s, 3.5, 1.0 / 60.0);
    assert_eq!((m.phase, m.target), (Phase::Watch, Some(4)));
    run_from(&mut m, &s, 3.5, 7.99, 1.0 / 60.0);
    assert_eq!(m.target, Some(4), "still on the killer before 8 s");
    run_from(&mut m, &s, 7.99, 8.02, 1.0 / 60.0);
    assert_eq!(m.target, Some(5));
}

/// More frames, from `from` to `t`.
fn run_from(m: &mut Machine, s: &Scene, from: f64, t: f64, step: f64) {
    let mut now = from + step;
    while now < t {
        m.frame(&s.frame(now));
        now += step;
    }
    m.frame(&s.frame(t));
}

/// 5. Enemy view off: the camera goes to D, never B.
#[test]
fn case_05_enemy_view_off() {
    let s = Scene::new(vec![cand(3, 1, true), cand(4, 2, true), cand(5, 1, true)]);
    let mut m = dead_from(killed_by(4));
    let mut seen = Vec::new();
    let mut now = 0.0;
    while now < 8.0 {
        m.frame(&s.frame(now));
        seen.push(m.target);
        now += 1.0 / 60.0;
    }
    assert_eq!(m.target, Some(5));
    assert!(!seen.contains(&Some(4)), "B was never watched");
}

/// 6. Respawn timing (TDM, wait 7000, no perk, no F7): the gauge starts at
/// the death, `timer` reads 06:53 with 6.53 s left, the first frame with
/// more than 7000 ms sends one request, the camera is put back at the
/// pull-back start in that frame and is on the next living teammate within
/// two frames; no second request.
#[test]
fn case_06_respawn_timing() {
    let mut s = Scene::new(vec![cand(3, 1, true), cand(5, 1, true)]);
    s.counts = true;
    let mut m = dead_from(killed_by(4));
    m.frame(&s.frame(0.0));
    let c = m.countdown.expect("the gauge starts at the death");
    close(c.started as f32, 0.0, 1e-9);
    assert_eq!(c.total(), 7);
    assert_eq!(c.text(0.47), "06:53");
    assert_eq!(c.gauge(0.47), 6);
    // Frames at 60 Hz until the request.
    let step = 1.0 / 60.0;
    let mut now = step;
    let mut requests = Vec::new();
    while now < 7.5 {
        let f = s.frame(now);
        m.player_update(&f);
        if let Some(fast) = m.request.take() {
            requests.push((now, fast));
            // The same frame: the pull-back's start, 20 u over the body.
            assert_eq!(m.phase, Phase::PullBack);
            near(m.pose.unwrap().at, BODY + Vec3::Y * 20.0, 1e-4);
            m.camera_update(&f);
            // Within the next two frames: the next living teammate.
            for k in 1..=2 {
                m.frame(&s.frame(now + k as f64 * step));
            }
            assert_eq!(m.phase, Phase::Watch, "after {now}");
            assert!(matches!(m.target, Some(3 | 5)), "a living teammate");
            now += 2.0 * step;
        } else {
            m.camera_update(&f);
        }
        now += step;
    }
    assert_eq!(requests.len(), 1, "{requests:?}");
    assert!(requests[0].0 > 7.0 && requests[0].0 < 7.0 + step + 1e-9, "{requests:?}");
    assert!(!requests[0].1, "not fast");
}

/// 7. Relentless (RespawnTime 5.6): the gauge's total is 5 s, 100 % and
/// 00:00 at 5.0 s; the request after 5.6 s.
#[test]
fn case_07_relentless() {
    let mut s = Scene::new(vec![cand(3, 1, true)]);
    s.counts = true;
    s.wait_ms = 5600.0;
    let mut m = dead_from(killed_by(4));
    m.frame(&s.frame(0.0));
    let c = m.countdown.unwrap();
    assert_eq!(c.total(), 5);
    assert_eq!((c.gauge(5.0), c.text(5.0)), (100, "00:00".to_string()));
    assert_eq!((c.gauge(4.99), c.text(4.99)), (99, "00:01".to_string()));
    assert!(!c.due(5.5) && !c.due(5.6) && c.due(5.61));
}

/// 8. Fast respawn (One Man Army, 2 left, dead 1.0 s, F7): the request
/// goes out with the fast flag on the next frame.
#[test]
fn case_08_fast_respawn() {
    let mut s = Scene::new(vec![cand(3, 7, true)]);
    s.counts = true;
    s.fast_ok = true;
    let mut m = dead_from(killed_by(4));
    run(&mut m, &s, 1.0, 1.0 / 60.0);
    assert_eq!(m.request, None);
    s.input.fast = true;
    m.frame(&s.frame(1.0 + 1.0 / 60.0));
    assert_eq!(m.request, Some(true));
    // Without the right (no uses left) F7 does nothing.
    let mut s = Scene::new(vec![cand(3, 7, true)]);
    s.counts = true;
    s.fast_ok = false;
    s.input.fast = true;
    let mut m = dead_from(killed_by(4));
    run(&mut m, &s, 1.0, 1.0 / 60.0);
    assert_eq!(m.request, None);
}

/// 9. Round modes: no gauge, no timer, no request ever; the pull-back
/// 3.5 s, then the watch.
#[test]
fn case_09_no_countdown_in_round_modes() {
    let s = Scene::new(vec![cand(3, 1, true), cand(5, 1, true)]);
    let mut m = dead_from(killed_by(4));
    let mut now = 0.0;
    while now < 30.0 {
        m.frame(&s.frame(now));
        assert!(m.countdown.is_none() && m.request.is_none() && m.text.is_none());
        now += 0.05;
    }
    assert_eq!(m.phase, Phase::Watch);
}

/// 10. Suicide: no pull-back, the watch on the first frame dead on the
/// next living teammate; the countdown runs.
#[test]
fn case_10_suicide() {
    let mut s = Scene::new(vec![cand(3, 1, true), cand(5, 1, true)]);
    s.counts = true;
    let mut m = dead_from(Killer::Suicide);
    m.frame(&s.frame(0.0));
    assert_eq!((m.phase, m.target), (Phase::Watch, Some(3)));
    assert!(m.countdown.is_some());
}

/// 11. Fall (no killer, id 255): the pull-back for 3.5 s, then the next
/// living teammate.
#[test]
fn case_11_fall_death() {
    assert_eq!(NO_KILLER, 255);
    let mut s = Scene::new(vec![cand(3, 1, true), cand(5, 1, true)]);
    s.counts = true;
    let mut m = dead_from(Killer::World);
    m.frame(&s.frame(0.0));
    assert_eq!(m.phase, Phase::PullBack);
    m.frame(&s.frame(3.499));
    assert_eq!(m.phase, Phase::PullBack);
    m.frame(&s.frame(3.5));
    assert_eq!((m.phase, m.target), (Phase::Watch, Some(3)));
}

// --- 12-17: the orbit

fn orbit(psi: f32, theta: f32, z: f32) -> math::Pose {
    math::orbit(BODY, psi, theta, z, &NoWalls)
}

/// 12. P_W = (0, 50, 0), psi = theta = z = 0: the camera at (-148.5, 151,
/// 0), heading +90 degrees, 28.61 degrees down.
#[test]
fn case_12_orbit_placement() {
    let p = orbit(0.0, 0.0, 0.0);
    near(p.at, Vec3::new(-148.5, 151.0, 0.0), 1e-3);
    close(p.heading.to_degrees(), 90.0, 1e-3);
    close(p.pitch.to_degrees(), 28.61, 0.01);
}

/// 13. psi = 90 degrees: (0, 151, -148.5), heading 0, pitch 28.61.
#[test]
fn case_13_orbit_yaw() {
    let p = orbit(90f32.to_radians(), 0.0, 0.0);
    near(p.at, Vec3::new(0.0, 151.0, -148.5), 1e-3);
    close(p.heading.to_degrees(), 0.0, 1e-3);
    close(p.pitch.to_degrees(), 28.61, 0.01);
}

/// 14. theta = 0.3: (-120.85, 185.91, 0), pitch 43.80; theta = -0.3:
/// (-162.88, 108.86, 0), pitch 13.42.
#[test]
fn case_14_orbit_pitch() {
    let p = orbit(0.0, 0.3, 0.0);
    near(p.at, Vec3::new(-120.85, 185.91, 0.0), 0.01);
    close(p.heading.to_degrees(), 90.0, 1e-3);
    close(p.pitch.to_degrees(), 43.80, 0.01);
    let p = orbit(0.0, -0.3, 0.0);
    near(p.at, Vec3::new(-162.88, 108.86, 0.0), 0.01);
    close(p.pitch.to_degrees(), 13.42, 0.01);
}

/// 15. psi = 45 degrees, theta = 0.3: (-85.46, 185.91, -85.46), heading
/// 45, pitch 43.80.
#[test]
fn case_15_orbit_yaw_and_pitch() {
    let p = orbit(45f32.to_radians(), 0.3, 0.0);
    near(p.at, Vec3::new(-85.46, 185.91, -85.46), 0.01);
    close(p.heading.to_degrees(), 45.0, 1e-3);
    close(p.pitch.to_degrees(), 43.80, 0.01);
}

/// 16. The mouse pushed far down: theta stops at 0.6854 rad; the camera at
/// (-69.95, 215.23, 0), pitch 64.28.
#[test]
fn case_16_pitch_clamp() {
    let (psi, theta) = math::turn(0.0, 0.0, Vec2::new(0.0, -100.0));
    close(theta, PITCH_LIMIT, 1e-6);
    close(PITCH_LIMIT, 0.6854, 1e-4);
    close(psi, 0.0, 1e-6);
    // Up too.
    close(math::turn(0.0, 0.0, Vec2::new(0.0, 100.0)).1, -PITCH_LIMIT, 1e-6);
    let p = orbit(0.0, theta, 0.0);
    near(p.at, Vec3::new(-69.95, 215.23, 0.0), 0.01);
    close(p.pitch.to_degrees(), 64.28, 0.01);
}

/// 17. Zoom: two notches away: z = 80, camera (-211.71, 185.48, 0); three
/// towards from 0: z clamped to -93.97, camera (-74.25, 110.5, 0). A new
/// target resets z.
#[test]
fn case_17_zoom() {
    let z = math::zoom(0.0, 2.0);
    close(z, 80.0, 1e-4);
    near(orbit(0.0, 0.0, z).at, Vec3::new(-211.71, 185.48, 0.0), 0.01);
    let z = math::zoom(0.0, -3.0);
    close(z, -93.97, 0.01);
    near(orbit(0.0, 0.0, z).at, Vec3::new(-74.25, 110.5, 0.0), 0.02);
    close(math::zoom(0.0, 100.0), 800.0, 1e-4);
    // The machine: the wheel zooms in the watch, a new target resets it.
    let mut s = Scene::new(vec![cand(3, 1, true), cand(5, 1, true)]);
    let mut m = dead_from(Killer::Suicide);
    m.frame(&s.frame(0.0));
    s.input.wheel = 2.0;
    m.frame(&s.frame(0.1));
    close(m.zoom, 80.0, 1e-4);
    s.input = Input { space: true, ..default() };
    m.frame(&s.frame(0.2));
    assert_eq!(m.target, Some(5));
    close(m.zoom, 0.0, 1e-6);
}

// --- 18-21: targets

/// 18. Watching id 3, teammates alive at 5 and 7: Space twice gives 5, 7;
/// a third press restarts at 3.
#[test]
fn case_18_cycling() {
    let mut s = Scene::new(vec![cand(3, 1, true), cand(5, 1, true), cand(7, 1, true)]);
    let mut m = dead_from(Killer::Suicide);
    m.frame(&s.frame(0.0));
    assert_eq!(m.target, Some(3));
    let mut got = Vec::new();
    for k in 1..=3 {
        s.input = Input { space: true, ..default() };
        m.frame(&s.frame(k as f64 * 0.1));
        got.push(m.target.unwrap());
    }
    assert_eq!(got, [5, 7, 3]);
    // Ctrl and Shift cycle too.
    s.input = Input { ctrl: true, ..default() };
    m.frame(&s.frame(1.0));
    s.input = Input { shift: true, ..default() };
    m.frame(&s.frame(1.1));
    assert_eq!(m.target, Some(7));
    // During the pull-back nothing happens.
    let mut m = dead_from(killed_by(9));
    s.input = Input { space: true, ..default() };
    m.frame(&s.frame(0.0));
    m.frame(&s.frame(0.1));
    assert_eq!(m.phase, Phase::PullBack);
    assert_eq!(m.target, None);
}

/// 19. Watching 5, which dies: the next frame the camera is on the next
/// living teammate after 5.
#[test]
fn case_19_target_dies() {
    let mut s = Scene::new(vec![cand(3, 1, true), cand(5, 1, true), cand(7, 1, true)]);
    let mut m = dead_from(Killer::Suicide);
    m.frame(&s.frame(0.0));
    s.input = Input { space: true, ..default() };
    m.frame(&s.frame(0.1));
    s.input = Input::default();
    assert_eq!(m.target, Some(5));
    s.set_alive(5, false);
    m.frame(&s.frame(0.2));
    assert_eq!(m.target, Some(7));
    // The own player as target is re-chosen too (it cannot be watched).
}

/// 20. The last teammate killed: with the enemy view on the camera moves
/// to the first living enemy in id order; with it off it holds its last
/// view and the target panel is hidden (no target).
#[test]
fn case_20_last_teammate() {
    for enemy_view in [true, false] {
        let mut s = Scene::new(vec![cand(2, 2, true), cand(3, 1, true), cand(4, 2, true)]);
        s.enemy_view = enemy_view;
        let mut m = dead_from(Killer::Suicide);
        m.frame(&s.frame(0.0));
        assert_eq!(m.target, Some(3));
        let held = m.pose;
        s.set_alive(3, false);
        m.frame(&s.frame(0.1));
        if enemy_view {
            assert_eq!(m.target, Some(2), "the first living enemy");
        } else {
            assert_eq!(m.target, None);
            assert_eq!(m.pose, held, "the view held");
            m.frame(&s.frame(0.2));
            assert_eq!(m.pose, held);
        }
    }
}

/// 21. Last Man Standing: living ids 2 (team 7) and 4 (team 9): the watch
/// cycles 2, 4, 2 whatever the teams.
#[test]
fn case_21_last_man_standing() {
    let mut s = Scene::new(vec![cand(2, 7, true), cand(4, 9, true)]);
    s.any_team = true;
    s.my_team = 5;
    let mut m = dead_from(Killer::Suicide);
    m.frame(&s.frame(0.0));
    let mut got = vec![m.target.unwrap()];
    for k in 1..=2 {
        s.input = Input { space: true, ..default() };
        m.frame(&s.frame(k as f64 * 0.1));
        got.push(m.target.unwrap());
    }
    assert_eq!(got, [2, 4, 2]);
}

// --- 22: the damage figure

/// 22. A rocket blast of 37.8: 75 and the body icon; a head shot of 140:
/// 140 and the head icon; 0.4: 1; a blast of 600: 999.
#[test]
fn case_22_damage_panel() {
    use chimera_core::core::{DamageKind, Hitgroup};
    assert_eq!(screen::damage_figure(37.8, screen::doubled(DamageKind::Blast)), 75);
    assert_eq!(screen::zone_of(Hitgroup::Chest, false), Zone::Body);
    assert_eq!(screen::damage_figure(140.0, screen::doubled(DamageKind::Bullet)), 140);
    assert_eq!(screen::zone_of(Hitgroup::Head, false), Zone::Head);
    assert_eq!(screen::damage_figure(0.4, false), 1);
    assert_eq!(screen::damage_figure(600.0, screen::doubled(DamageKind::Blast)), 999);
    assert_eq!(screen::doubled(DamageKind::Burn), true);
    // The other zones and strings.
    assert_eq!(screen::zone_of(Hitgroup::LeftArm, false), Zone::Arms);
    assert_eq!(screen::zone_of(Hitgroup::RightLeg, false), Zone::Leg);
    assert_eq!(screen::zone_of(Hitgroup::Stomach, true), Zone::Groin);
    assert_eq!(screen::zone_of(Hitgroup::Generic, false), Zone::Body);
    assert_eq!(Zone::Head.string(), "DamagePointIconHeadTex");
    assert_eq!(Zone::Groin.string(), "DamagePointIconNutTex");
    // The killer's numbers: three zero-padded digits.
    assert_eq!(screen::three_digits(42.9), "042");
    assert_eq!(screen::three_digits(-3.0), "000");
}

/// The game's frame (left-handed, Z forward) in the engine's: positions
/// mirror Z, a heading turns right, a positive pitch looks down.
#[test]
fn the_camera_in_the_engines_frame() {
    let q = math::to_rotation(std::f32::consts::FRAC_PI_2, 0.0);
    near(q * Vec3::NEG_Z, Vec3::X, 1e-5);
    near(math::to_rotation(0.0, 0.0) * Vec3::NEG_Z, Vec3::NEG_Z, 1e-5);
    near(math::to_rotation(std::f32::consts::PI, 0.0) * Vec3::NEG_Z, Vec3::Z, 1e-5);
    assert!((math::to_rotation(0.0, 0.5) * Vec3::NEG_Z).y < -0.4);
    let p = Vec3::new(1.0, 2.0, 3.0);
    near(math::to_world(p, 0.5), Vec3::new(0.5, 1.0, -1.5), 1e-6);
    near(math::from_world(math::to_world(p, 0.018), 0.018), p, 1e-4);
    // The orbit's pose: looking along +X from the -X side, level with
    // the anchor's side.
    let pose = orbit(0.0, 0.0, 0.0);
    let look = math::to_rotation(pose.heading, pose.pitch) * Vec3::NEG_Z;
    let to_anchor = (math::to_world(math::anchor(BODY), 1.0) - math::to_world(pose.at, 1.0)).normalize();
    near(look, to_anchor, 1e-4);
}

// --- 8: the tip banner

/// The guide data: the lowest `Order` among the records whose conditions
/// pass; one of its texts; none passing hides the banner.
#[test]
fn tips_from_the_guide_data() {
    use screen::{DeathFacts, RoomFacts, game_type, parse_guide, pick_tip};
    let doc = lithtech_formats::attributes::Document::parse(
        br#"
[InGame0] Order=0 DTXPath="a.dtx" Desc0="$A0" Desc1="$A1" ConditionType0=6 ConditionData0=1 ConditionType1=1 ConditionData1_0=1 ConditionData1_1=12
[InGame1] Desc0="$B0"
[InGame2] Order=0 Desc0="$C0" ConditionType0=3 ConditionData0=4
[InGame3] Enable=0 Order=0 Desc0="$D0"
"#,
    )
    .unwrap();
    let tips = parse_guide(&doc);
    assert_eq!(tips.len(), 4);
    assert_eq!((tips[1].order, tips[0].order), (255, 0));
    assert_eq!(tips[0].conditions.len(), 2);
    assert_eq!(tips[0].conditions[1].data, [1, 12]);
    let tdm = RoomFacts {
        mode_type: game_type("TDM"),
    };
    // A plain death in TDM: only the unconditioned record (order 255).
    let plain = pick_tip(&tips, &tdm, &DeathFacts::default(), 7).unwrap();
    assert_eq!(plain.text, "$B0");
    // A suicide in TDM (game type 1): record 0 has the lowest order and
    // one of its two texts.
    let suicide = DeathFacts {
        suicide: true,
        ..Default::default()
    };
    let t = pick_tip(&tips, &tdm, &suicide, 7).unwrap();
    assert_eq!(t.icon, "a.dtx");
    assert!(t.text == "$A0" || t.text == "$A1");
    let both: std::collections::HashSet<String> = (0..64u64).map(|r| pick_tip(&tips, &tdm, &suicide, r.wrapping_mul(0x9E37_79B9_7F4A_7C15)).unwrap().text).collect();
    assert_eq!(both.len(), 2, "uniform over the texts");
    // In Elimination Pro (12 is on the list) too; in S&D (2) it isn't.
    let tsv = RoomFacts {
        mode_type: game_type("TSV"),
    };
    assert!(pick_tip(&tips, &tsv, &suicide, 1).unwrap().text.starts_with("$A"));
    let tmm = RoomFacts {
        mode_type: game_type("TMM"),
    };
    assert_eq!(pick_tip(&tips, &tmm, &suicide, 1).unwrap().text, "$B0");
    // None passing: no banner.
    assert_eq!(pick_tip(&tips[3..], &tdm, &suicide, 1), None);
    assert_eq!(game_type("HG"), Some(29));
}

// --- 24-26: Hunted, held joiners

/// 24. Hunted, killed by an enemy: 0-3.5 s the hold (the view on the body,
/// phase held), 3.5-7.0 the pull-back (the view's angles reset every
/// frame), from 7.0 the watch on a living player; no countdown; at 11.5 s
/// the camera leaves the killer.
#[test]
fn case_24_hunted_timeline() {
    let mut s = Scene::new(vec![cand(3, 9, true), cand(4, 8, true), cand(6, 7, true)]);
    s.hunted = true;
    s.any_team = true;
    s.enemy_view = true;
    s.my_team = 5;
    let mut m = Machine::default();
    m.died(0.0, killed_by(4), (0.3, 0.1));
    let step = 1.0 / 60.0;
    let mut now = 0.0;
    let mut phases = Vec::new();
    while now < 12.0 {
        s.input.look = Vec2::new(0.2, 0.0);
        m.frame(&s.frame(now));
        phases.push((now, m.phase, m.target));
        if m.phase == Phase::PullBack {
            close(m.psi, 0.3, 1e-6);
            close(m.theta, 0.1, 1e-6);
        }
        assert!(m.countdown.is_none());
        now += step;
    }
    let at = |t: f64| phases.iter().find(|(n, ..)| *n >= t).unwrap();
    assert_eq!(at(0.0).1, Phase::Hold);
    assert_eq!(at(3.4).1, Phase::Hold);
    assert_eq!(at(3.6).1, Phase::PullBack);
    assert_eq!(at(6.9).1, Phase::PullBack);
    assert_eq!((at(7.1).1, at(7.1).2), (Phase::Watch, Some(4)), "on the killer");
    assert_eq!(at(11.4).2, Some(4));
    assert_ne!(at(11.6).2, Some(4), "off the killer after 11.5 s");
}

/// 25. Hunted, the watch, Shift (first time in the map): the camera 1000 u
/// above the body, the view pitched 90 degrees further down; the movement
/// keys fly it; Space returns to the orbit on the next target.
#[test]
fn case_25_hunted_free_camera() {
    let mut s = Scene::new(vec![cand(3, 9, true), cand(6, 7, true)]);
    s.hunted = true;
    s.any_team = true;
    s.my_team = 5;
    let mut m = dead_from(Killer::Suicide);
    m.frame(&s.frame(0.0));
    assert_eq!((m.phase, m.target), (Phase::Watch, Some(3)));
    let before = m.pose.unwrap();
    s.input = Input { shift: true, dt: 1.0 / 60.0, ..default() };
    m.frame(&s.frame(0.1));
    assert_eq!(m.phase, Phase::FreeStatic);
    let p = m.pose.unwrap();
    near(p.at, BODY + Vec3::Y * 1000.0, 1e-3);
    close(p.pitch, before.pitch + std::f32::consts::FRAC_PI_2, 1e-4);
    // Fly forward.
    s.input = Input { axis: Vec2::Y, dt: 1.0 / 60.0, ..default() };
    m.frame(&s.frame(0.2));
    let flown = m.pose.unwrap();
    assert!(flown.at.distance(p.at) > 1.0, "moved");
    // Space: back to the orbit on the next target.
    s.input = Input { space: true, dt: 1.0 / 60.0, ..default() };
    m.frame(&s.frame(0.3));
    assert_eq!((m.phase, m.target), (Phase::Watch, Some(6)));
    // The second time the static camera returns to where it was left.
    s.input = Input { shift: true, dt: 1.0 / 60.0, ..default() };
    m.frame(&s.frame(0.4));
    assert_eq!(m.phase, Phase::FreeStatic);
    near(m.pose.unwrap().at, flown.at, 1e-3);
}

/// 26. A held joiner: the free camera starts 50 u above the player's
/// position and flies; alive again clears it.
#[test]
fn case_26_held_joiner() {
    let s = Scene::new(vec![cand(3, 1, true)]);
    let mut m = Machine::default();
    let mut f = s.frame(0.0);
    f.dead = false;
    f.waiting = true;
    m.wait_frame(&f);
    near(m.pose.unwrap().at, BODY + Vec3::Y * 50.0, 1e-4);
    let mut s = s;
    s.input = Input { axis: Vec2::Y, dt: 1.0 / 60.0, ..default() };
    let mut f = s.frame(0.1);
    f.dead = false;
    f.waiting = true;
    m.wait_frame(&f);
    assert!(m.pose.unwrap().at.distance(BODY + Vec3::Y * 50.0) > 1.0);
    m.alive();
    assert!(m.pose.is_none() && !m.is_dead());
}

// --- Players in a match (CaSim)

mod world {
    use bevy::prelude::*;
    use chimera_core::core::{DamageKind, Dead, Health, LocalPlayer, MapView};
    use chimera_world::harness::Sim;
    use combat_arms::game::{
        death_cam::{
            CaDeathCam, CaDeathInput, CaFastRespawn, CaKillCamRoom, CaKillMe, CaSlot,
            machine::{Input, Phase},
            screen::FastView,
        },
        rules::spawns::CaHeld,
    };

    use crate::mode_kit::*;

    /// The local player (slot 2) on Alpha with teammates C (3) and D (5)
    /// and B (4) on Bravo, in `mode`, live (past the warm-up).
    #[allow(dead_code)]
    pub struct Rig {
        pub sim: Sim,
        pub me: Entity,
        pub c: Entity,
        pub b: Entity,
        pub d: Entity,
    }

    pub fn rig(mode: &str, enemy_view: i32) -> Rig {
        let mut sim = game(mode, 12);
        let me = join(&mut sim, JSF);
        let c = join(&mut sim, JSF);
        let b = join(&mut sim, CREED);
        let d = join(&mut sim, JSF);
        let w = sim.app.world_mut();
        for (e, slot, name) in [(me, 2, "Me"), (c, 3, "Charlie"), (b, 4, "Bravo"), (d, 5, "Delta")] {
            w.entity_mut(e).insert((CaSlot(slot), Name::new(name)));
        }
        w.entity_mut(me).insert(LocalPlayer);
        w.insert_resource(CaKillCamRoom(enemy_view));
        until_live(&mut sim);
        if m(&sim).warmup {
            past_warmup(&mut sim);
        }
        unprotect_all(&mut sim);
        Rig { sim, me, c, b, d }
    }

    impl Rig {
        pub fn cam(&self) -> &CaDeathCam {
            self.sim.app.world().resource::<CaDeathCam>()
        }

        /// Seconds since the local player died.
        pub fn since(&self) -> f64 {
            let died = self.sim.app.world().get::<Dead>(self.me).map(|d| d.since);
            now(&self.sim) - died.expect("dead")
        }

        /// Ticks until `done`, at most `secs`.
        pub fn until(&mut self, secs: f64, done: impl Fn(&Rig) -> bool) -> bool {
            let end = now(&self.sim) + secs;
            while now(&self.sim) < end {
                if done(self) {
                    return true;
                }
                self.sim.ticks(1);
            }
            done(self)
        }

        pub fn press(&mut self, input: Input) {
            self.sim.app.world_mut().insert_resource(CaDeathInput(input));
            self.sim.ticks(1);
            self.sim.app.world_mut().insert_resource(CaDeathInput::default());
        }
    }

    /// 4. Enemy view on, Elimination Pro: killed by B (health 42, armour
    /// 0): at 3.5 s the camera orbits B, the screen's killer panel is
    /// `CamOn` with `042` / `000`; after 8 s it orbits D.
    #[test]
    fn case_04_enemy_view_on() {
        let mut r = rig("TSV", 1);
        let w = r.sim.app.world_mut();
        w.get_mut::<Health>(r.b).unwrap().current = 0.42;
        w.entity_mut(r.b).insert(combat_arms::game::weapons::gear_armour(0.0, &Vec::new()));
        kill_now(&mut r.sim, r.me, r.b);
        assert!(r.cam().machine.is_dead());
        assert_eq!(r.cam().phase(), Phase::PullBack);
        assert!(r.cam().screen.respawn.is_none());
        assert!(r.until(5.0, |r| r.cam().phase() == Phase::Watch));
        let t = r.since();
        assert!((3.5..3.52).contains(&t), "{t}");
        assert_eq!(r.cam().machine.target, Some(4));
        assert_eq!(r.cam().watched, Some(r.b));
        let k = r.cam().screen.killer.clone().expect("the killer panel");
        assert_eq!((k.name.as_str(), k.cam_on, k.hp.as_str(), k.ap.as_str()), ("Bravo", true, "042", "000"));
        assert_eq!(r.cam().screen.target.as_ref().unwrap().name, "Bravo");
        assert!(r.until(6.0, |r| r.cam().machine.target != Some(4)));
        let t = r.since();
        assert!((8.0..8.02).contains(&t), "{t}");
        assert_eq!(r.cam().machine.target, Some(5));
    }

    /// 5. Enemy view off: D, never B; `CamOff`.
    #[test]
    fn case_05_enemy_view_off() {
        let mut r = rig("TSV", 0);
        kill_now(&mut r.sim, r.me, r.b);
        let mut seen = Vec::new();
        while r.since() < 3.6 {
            r.sim.ticks(1);
            seen.push(r.cam().machine.target);
        }
        assert_eq!(r.cam().machine.target, Some(5));
        assert!(!seen.contains(&Some(4)));
        let k = r.cam().screen.killer.clone().unwrap();
        assert!(!k.cam_on && k.name == "Bravo");
    }

    /// 6. TDM: the gauge starts at the death (total 7 s, `06:5x` after
    /// 0.47 s); the first frame with more than 7 s asks, the server
    /// accepts and the player is alive; the camera is back on the own
    /// view.
    #[test]
    fn case_06_respawn_timing() {
        let mut r = rig("TDM", -1);
        kill_now(&mut r.sim, r.me, r.b);
        let died = r.sim.app.world().get::<Dead>(r.me).unwrap().since;
        let c = r.cam().machine.countdown.expect("the countdown");
        assert!((c.started - died).abs() < 0.02, "{} {died}", c.started);
        assert_eq!(c.total(), 7);
        assert!(r.until(1.0, |r| r.since() >= 0.47));
        let s = r.cam().screen.respawn.clone().expect("the gauge");
        assert!(s.timer.starts_with("06:") && s.gauge >= 6, "{s:?}");
        assert!(r.until(8.0, |r| !dead(&r.sim, r.me)));
        let t = now(&r.sim) - died;
        assert!(t > 7.0 && t < 7.1, "{t}");
        r.sim.ticks(2);
        assert!(!r.cam().active(), "back on the own view");
    }

    /// 8. One Man Army, 2 fast respawns left: F7 after 1 s respawns at once
    /// and spends one; the panel showed 2 of 3 before.
    #[test]
    fn case_08_fast_respawn() {
        let mut r = rig("FFA", -1);
        r.sim.app.world_mut().entity_mut(r.me).insert(CaFastRespawn {
            left: 2,
            max: 3,
            premium: false,
        });
        kill_now(&mut r.sim, r.me, r.b);
        assert!(r.until(2.0, |r| r.since() >= 1.0));
        let s = r.cam().screen.respawn.clone().expect("the gauge");
        assert_eq!(s.fast, Some(FastView::Free { left: 2, max: 3 }));
        r.press(Input {
            fast: true,
            ..default()
        });
        r.sim.ticks(2);
        assert!(!dead(&r.sim, r.me), "respawned");
        assert_eq!(r.sim.app.world().get::<CaFastRespawn>(r.me).unwrap().left, 1);
    }

    /// 9. Elimination Pro: no gauge, timer or banner, no request ever.
    #[test]
    fn case_09_no_countdown() {
        let mut r = rig("TSV", 0);
        kill_now(&mut r.sim, r.me, r.b);
        r.sim.seconds(12.0);
        assert!(dead(&r.sim, r.me), "never asks");
        assert!(r.cam().machine.countdown.is_none());
        assert!(r.cam().screen.respawn.is_none() && r.cam().screen.banner.is_none());
        assert_eq!(r.cam().phase(), Phase::Watch);
    }

    /// 10. TDM, own grenade: no pull-back, the watch on a teammate from
    /// the first frame, no killer panel, the countdown runs.
    #[test]
    fn case_10_suicide() {
        let mut r = rig("TDM", -1);
        hit(&mut r.sim, r.me, Some(r.me), 50.0, DamageKind::Blast, Some("combat_arms:m67"));
        r.sim.ticks(2);
        assert!(dead(&r.sim, r.me));
        assert_eq!(r.cam().phase(), Phase::Watch);
        assert!(matches!(r.cam().machine.target, Some(3 | 5)));
        assert!(r.cam().screen.killer.is_none());
        assert!(r.cam().machine.countdown.is_some());
        assert_eq!(r.cam().screen.damage.as_ref().map(|d| d.text.as_str()), Some("999"));
    }

    /// 11. A fall with no killer: the pull-back for 3.5 s, then a teammate;
    /// no killer panel.
    #[test]
    fn case_11_fall() {
        let mut r = rig("TDM", -1);
        hit(&mut r.sim, r.me, None, 50.0, DamageKind::Fall, None);
        r.sim.ticks(2);
        assert_eq!(r.cam().phase(), Phase::PullBack);
        assert!(r.cam().screen.killer.is_none());
        assert!(r.until(5.0, |r| r.cam().phase() == Phase::Watch));
        assert!(r.since() >= 3.5);
        assert!(matches!(r.cam().machine.target, Some(3 | 5)));
    }

    /// 22. The damage panel from the world: a blast of 37.8 HP shows 75.
    #[test]
    fn case_22_damage_panel_from_the_world() {
        let mut r = rig("TSV", 0);
        // 20 HP left and no armour: the blast of 37.8 HP kills.
        let w = r.sim.app.world_mut();
        w.get_mut::<Health>(r.me).unwrap().current = 0.2;
        w.entity_mut(r.me).insert(combat_arms::game::weapons::gear_armour(0.0, &Vec::new()));
        hit(&mut r.sim, r.me, Some(r.b), 0.378, DamageKind::Blast, Some("combat_arms:m67"));
        r.sim.ticks(2);
        assert!(dead(&r.sim, r.me));
        let d = r.cam().screen.damage.clone().expect("the damage window");
        assert_eq!(d.text, "75");
        let w = r.cam().screen.weapon.clone().expect("the weapon panel");
        assert_eq!(w.id, "combat_arms:m67");
    }

    /// 23. B kills you three times in a match: at the third death, watching
    /// B, the target panel counts 3; watching D, none. Suicides don't
    /// count.
    #[test]
    fn case_23_kill_me_count() {
        let mut r = rig("TSV", 1);
        r.sim.app.world_mut().entity_mut(r.me).insert(CaKillMe(vec![(4, 2)]));
        kill_now(&mut r.sim, r.me, r.b);
        assert_eq!(r.sim.app.world().get::<CaKillMe>(r.me).unwrap().count(4), 3);
        assert!(r.until(5.0, |r| r.cam().phase() == Phase::Watch));
        assert_eq!(r.cam().machine.target, Some(4));
        assert_eq!(r.cam().screen.target.as_ref().unwrap().kills_you, Some(3));
        // Space: the next living teammate after B: D (5), who never did.
        r.press(Input {
            space: true,
            ..default()
        });
        assert_eq!(r.cam().machine.target, Some(5));
        assert_eq!(r.cam().screen.target.as_ref().unwrap().kills_you, None);
        // Not a suicide.
        let mut r = rig("TSV", 1);
        hit(&mut r.sim, r.me, Some(r.me), 50.0, DamageKind::Blast, Some("combat_arms:m67"));
        r.sim.ticks(2);
        assert!(r.sim.app.world().get::<CaKillMe>(r.me).is_none_or(|k| k.0.is_empty()));
    }

    /// 26. A held joiner: no death camera, a free camera 50 u over the
    /// player's position, no death screen; alive at the next setup.
    #[test]
    fn case_26_held_joiner() {
        let mut r = rig("TMM", 0);
        let me = r.me;
        let w = r.sim.app.world_mut();
        w.entity_mut(me).insert((CaHeld, Dead { since: f64::MIN }));
        w.get_mut::<Health>(me).unwrap().current = 0.0;
        r.sim.ticks(2);
        assert!(r.cam().waiting && !r.cam().screen_up && !r.cam().machine.is_dead());
        let at = r.sim.app.world().get::<Transform>(me).unwrap().translation;
        let pose = r.cam().pose.expect("a free camera").0;
        assert!((pose.y - (at.y + 50.0 * combat_arms::game::inventory::UNIT)).abs() < 0.01, "{pose} {at}");
        let w = r.sim.app.world_mut();
        w.entity_mut(me).remove::<(CaHeld, Dead)>();
        w.get_mut::<Health>(me).unwrap().current = 1.0;
        r.sim.ticks(2);
        assert!(!r.cam().waiting && r.cam().pose.is_none());
    }

    /// 27. A live map camera hides the death screen until the next setup.
    #[test]
    fn case_27_map_camera() {
        let mut r = rig("TMM", 0);
        kill_now(&mut r.sim, r.me, r.b);
        r.sim.ticks(2);
        assert!(r.cam().screen_up);
        r.sim.app.world_mut().entity_mut(r.me).insert(MapView {
            origin: Vec3::new(1.0, 2.0, 3.0),
            rotation: Quat::IDENTITY,
        });
        r.sim.ticks(2);
        assert!(!r.cam().screen_up, "hidden while the map camera is live");
        r.sim.app.world_mut().entity_mut(r.me).remove::<MapView>();
        r.sim.ticks(2);
        assert!(r.cam().screen_up);
    }

    /// The room's Kill Cam option writes `Enemydeadcam` at the match's
    /// start: Off by default, On in Hunted, the option over both.
    #[test]
    fn enemydeadcam_from_the_room_option() {
        use combat_arms::game::death_cam::CaEnemyCam;
        let enemy = |r: &Rig| r.sim.app.world().resource::<CaEnemyCam>().0;
        assert_eq!(enemy(&rig("TDM", -1)), 0);
        assert_eq!(enemy(&rig("HG", -1)), 1);
        assert_eq!(enemy(&rig("TDM", 1)), 1);
        assert_eq!(enemy(&rig("HG", 0)), 0);
    }
}

/// kill_streaks.md case 57 (A1, answered 2026-11-14) on Combat Arms'
/// simulation: the killer's death leaves the multi-kill count (a kill
/// 4.5 s after the first, a death between, is a double kill); the mode's
/// end of mission (the mode runtime's statistics reset, raised at each
/// match's start) starts it again.
#[test]
fn case_57_multi_kill_survives_death_until_the_mission_ends() {
    use chimera_sim::announce::KillCounter;
    use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
    let mut sim = Sim::new(combat_arms::harness::CaSim, GreyboxMapPlugin);
    let world = sim.app.world_mut();
    let (p, v, other) = (world.spawn_empty().id(), world.spawn_empty().id(), world.spawn_empty().id());
    let mut c = world.resource_mut::<KillCounter<Entity>>();
    assert_eq!(c.enemy_kill(p, v, 0.0, 5000, 2).multi, 1);
    // P killed at 1.0 by another, respawned at 3.0.
    c.enemy_kill(other, p, 1.0, 5000, 2);
    assert_eq!(c.enemy_kill(p, v, 4.5, 5000, 2).multi, 2, "DOUBLE KILL: the death reset nothing");
    world.trigger(chimera_sim::mode::ModeStep(chimera_sim::mode::FlowEvent::ResetStats));
    let mut c = world.resource_mut::<KillCounter<Entity>>();
    assert_eq!(c.enemy_kill(p, v, 6.0, 5000, 2).multi, 1, "the mission's end reset it");
}
