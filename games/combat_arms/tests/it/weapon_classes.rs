//! specs/combat_arms/weapon_classes.md's test cases, numbered as there, on
//! Combat Arms' simulation (`CaSim`) and the greybox: the class weapons of
//! the game's data (MP5A4, M60, M92FS, Remington 870, L96A1, Dragunov) and
//! test weapons made from them as data where a case names another weapon
//! (M107CQ, ASW338LM, the Desert Eagle, the minigun, the double barrel, dot
//! sights) or aims at one body part (no spread, no roll, no kick, no
//! firing recovery: "exact" and "steady" variants, as weapon_firing.rs
//! does).
//!
//! Times: "exact time" cases run on the game's 15 ms tick (one update a
//! tick; a press on a tick is processed on it, so "t" after `k` ticks
//! counting the press's own is k x 15 ms); a few run at 10 ms so the
//! case's times fall on ticks. Tolerances: one tick for times, 1e-3
//! degrees for the field of view and view angles, 1e-4 rad for the sway,
//! 1e-3 HP for damage (1e-2 for 8 pellets summed in f32), exact for cones (whole thousandths).

use std::f32::consts::PI;

use avian3d::prelude::*;
use bevy::prelude::*;
use chimera_core::core::{Health, Intent, MovementState, SimClock};
use chimera_sim::weapon::{
    self, Hitscan, Inventory, Magazine, ShellStage, SpreadShape, ViewPunch, WeaponEvent, WeaponEventKind, WeaponState,
    Zoomed, data::WeaponDataAppExt,
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{
    game::{
        TICK_INTERVAL,
        movement::{self, CaMovement},
        weapons::{
            self as ca, CaKick, CaRecoil, CaSpin,
 CaSpread, CaZoom, FireMode, MALE, PelletCarryOver, SpinState,
            SpreadScales, reported_hits, zoom,
        },
    },
    harness::CaSim,
};

const U: f32 = 0.018;
const HP: f32 = 0.01;
const L96A1: &str = "combat_arms:l96a1";
const SVD: &str = "combat_arms:dragunov_svd";
const R870: &str = "combat_arms:remington_870";
const M92FS: &str = "combat_arms:beretta_m92fs";
const MP5: &str = "combat_arms:mp5a4";
const M60: &str = "combat_arms:m60";

/// M107CQ: automatic, "Barrett Scope" (30, 10), ZoomTime 0.16 ("Other
/// weapons").
const M107CQ: &str = r#"
id = "test:m107cq"
base = "combat_arms:l96a1"
[trigger]
automatic = true
[hooks."combat_arms:zoom"]
zoom_time = 0.16
reload_after_fire = false
[hooks."combat_arms:attachments"]
accepts = ["Barrett Scope"]
defaults = { sight = "Barrett Scope" }
"#;

/// ASW338LM: a bolt sniper with the Barrett Scope, ZoomTime 0.15.
const ASW338LM: &str = r#"
id = "test:asw338lm"
base = "combat_arms:l96a1"
[hooks."combat_arms:zoom"]
zoom_time = 0.15
[hooks."combat_arms:attachments"]
accepts = ["Barrett Scope"]
defaults = { sight = "Barrett Scope" }
"#;

/// Dot sights: 50 and 75 degrees on a weapon with ZoomTime 0.15 (the
/// sight a default attachment: attachments.md).
const DOT50: &str = r#"
id = "test:dot50"
base = "combat_arms:m4a1"
[hooks."combat_arms:attachments"]
accepts = ["DotSight_A_AR"]
defaults = { sight = "DotSight_A_AR" }
"#;
const DOT75: &str = r#"
id = "test:dot75"
base = "combat_arms:m4a1"
[hooks."combat_arms:attachments"]
accepts = ["P90 sight"]
defaults = { sight = "P90 sight" }
"#;

/// The L96A1 with ten rounds a clip (case 11 fires six).
const L96A1_TEN: &str = r#"
id = "test:l96a1_ten"
base = "combat_arms:l96a1"
[magazine]
clip = 10
"#;

/// Desert Eagle: automatic, 70 damage, fire 600 ms, 7 / 14 (Other weapons).
const DEAGLE: &str = r#"
id = "test:desert_eagle"
base = "combat_arms:beretta_m92fs"
[ammo."test:50ae"]
max = 7
[trigger]
automatic = true
cycle = 0.6
[magazine]
clip = 7
ammo = "test:50ae"
[hooks."combat_arms:damage"]
damage = 70.0
"#;

/// Minigun_Special: Guntype 5, AccumPreFire 1, 500 / 500, 25 damage; the
/// spin of 3.3 (prefire 1300, postfire 1333, a 170 ms loop of 34 ms
/// rounds).
const MINIGUN: &str = r#"
id = "test:minigun_special"
base = "combat_arms:m60"
[ammo."test:minigun"]
max = 0
[trigger]
cycle = 0.034
# The spin-up is its pre-fire (Corrections 3).
latency = 1
[magazine]
clip = 500
ammo = "test:minigun"
[hooks."combat_arms:damage"]
damage = 25.0
[hooks."combat_arms:spin"]
prefire = 1.3
postfire = 1.333
loop_len = 0.17
reversible = true
"#;

/// Double_Barrel: 2 / 14, 8 x 14, DynamicPerturbFactor 1.3, 50/100,
/// bands 250/500/1000 at 1.0/0.6/0.2, fire 800 ms; alt-fire both barrels
/// 67 ms apart (5.8). Its growth per shot, 0.225, is PerturbIncSpeed 6.0
/// (0.0125 x 3 x 6); the cap and the recovery delay aren't given: 1.0 and
/// the M60's 100 ms (more than the 67 ms between barrels).
const DOUBLE_BARREL: &str = r#"
id = "test:double_barrel"
base = "combat_arms:m60"
[ammo."test:12ga_db"]
max = 12
[trigger]
automatic = false
cycle = 0.8
[magazine]
clip = 2
ammo = "test:12ga_db"
reload = 3.0
[hitscan]
pellets = 8
range = 1000.0
[hooks."combat_arms:damage"]
damage = 14.0
effect_range = [250.0, 500.0, 1000.0]
range_damage = [1.0, 0.6, 0.2]
[hooks."combat_arms:spread"]
cone = { min = 50, max = 100, inc = 6.0, dec = 1.0 }
dynamic_factor = 1.3
state = { fire = 1.0, fire_move = 1.0, fire_duck = 1.0, fire_move_duck = 1.0, walk = 0.0, run = 0.0, jump = 0.0 }
[hooks."combat_arms:recoil"]
pitch = 0.0
[hooks."combat_arms:alt_fire"]
rounds = 2
interval = 0.067
cycle = 0.8
"#;

/// Variants without firing recovery or kick (cone sequences "within
/// DecDelay").
const STEADY: &str = r#"
id = "test:m60_steady"
base = "combat_arms:m60"
[hooks."combat_arms:spread"]
dec_delay = 100000.0
[hooks."combat_arms:recoil"]
pitch = 0.0
"#;
const STEADY_M92FS: &str = r#"
id = "test:m92fs_steady"
base = "combat_arms:beretta_m92fs"
[hooks."combat_arms:spread"]
dec_delay = 100000.0
[hooks."combat_arms:recoil"]
pitch = 0.0
"#;
const STEADY_MP5: &str = r#"
id = "test:mp5a4_steady"
base = "combat_arms:mp5a4"
[hooks."combat_arms:spread"]
dec_delay = 100000.0
[hooks."combat_arms:recoil"]
pitch = 0.0
"#;

/// Variants with no spread, roll or kick: each pellet goes where aimed
/// and deals its base damage (the cases' "roll 1.0").
const EXACT: &str = r#"
id = "test:r870_exact"
base = "combat_arms:remington_870"
[hooks."combat_arms:damage"]
roll = 0.0
[hooks."combat_arms:spread"]
cone = { min = 0, max = 0, inc = 5.0, dec = 1.0 }
[hooks."combat_arms:recoil"]
pitch = 0.0
"#;
const EXACT_ONE: &str = r#"
id = "test:r870_one_pellet"
base = "test:r870_exact"
[hitscan]
pellets = 1
"#;
const EXACT_SEVEN: &str = r#"
id = "test:r870_seven_pellets"
base = "test:r870_exact"
[hitscan]
pellets = 7
"#;
const EXACT_MP5: &str = r#"
id = "test:mp5a4_exact"
base = "combat_arms:mp5a4"
[hooks."combat_arms:damage"]
roll = 0.0
[hooks."combat_arms:spread"]
cone = { min = 0, max = 0, inc = 0.8, dec = 0.37 }
[hooks."combat_arms:recoil"]
pitch = 0.0
"#;
const EXACT_L96A1: &str = r#"
id = "test:l96a1_exact"
base = "combat_arms:l96a1"
[hooks."combat_arms:damage"]
roll = 0.0
[hooks."combat_arms:spread"]
cone = { min = 0, max = 0, inc = 1.0, dec = 0.1 }
[hooks."combat_arms:recoil"]
pitch = 0.0
"#;
const EXACT_SVD: &str = r#"
id = "test:svd_exact"
base = "combat_arms:dragunov_svd"
[hooks."combat_arms:damage"]
roll = 0.0
[hooks."combat_arms:spread"]
cone = { min = 0, max = 0, inc = 1.0, dec = 0.1 }
[hooks."combat_arms:recoil"]
pitch = 0.0
"#;
/// The Remington 870 with a wider cone, so some pellets miss (case 23).
const WIDE_R870: &str = r#"
id = "test:r870_wide"
base = "test:r870_exact"
[hooks."combat_arms:spread"]
cone = { min = 30, max = 30, inc = 5.0, dec = 1.0 }
dynamic_factor = 1.0
"#;

/// What weapons did: (time, weapon, kind).
#[derive(Resource, Default)]
struct Log(Vec<(f64, Entity, WeaponEventKind)>);

fn log_events(mut events: MessageReader<WeaponEvent>, clock: Res<SimClock>, mut log: ResMut<Log>) {
    for e in events.read() {
        log.0.push((clock.now, e.weapon, e.kind.clone()));
    }
}

fn sim_at(tick: f64) -> Sim {
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins(GreyboxMapPlugin);
        app.init_resource::<Log>().add_systems(FixedPostUpdate, log_events);
    });
    sim.set_tick_interval(tick);
    sim.app
        .add_weapon_data(&[
            ("m107cq", M107CQ),
            ("asw338lm", ASW338LM),
            ("dot50", DOT50),
            ("dot75", DOT75),
            ("l96a1_ten", L96A1_TEN),
            ("deagle", DEAGLE),
            ("minigun", MINIGUN),
            ("double_barrel", DOUBLE_BARREL),
            ("steady", STEADY),
            ("steady_m92fs", STEADY_M92FS),
            ("steady_mp5", STEADY_MP5),
            ("exact", EXACT),
            ("exact_one", EXACT_ONE),
            ("exact_seven", EXACT_SEVEN),
            ("exact_mp5", EXACT_MP5),
            ("exact_l96a1", EXACT_L96A1),
            ("exact_svd", EXACT_SVD),
            ("wide", WIDE_R870),
        ])
        .unwrap();
    sim
}

fn new_sim() -> Sim {
    sim_at(TICK_INTERVAL)
}

fn now(sim: &Sim) -> f64 {
    sim.app.world().resource::<SimClock>().now
}

fn dt(sim: &Sim) -> f64 {
    // A hair over, for float slack in "within one tick".
    sim.tick_hz().recip() + 1e-6
}

/// A Combat Arms player at `at` looking along `yaw`, settled, with health
/// `health` (1.0 = 100 HP) and no armour.
fn player(sim: &mut Sim, at: Vec3, yaw: f32, health: f32) -> Entity {
    let p = sim.spawn_character(at, movement::ID);
    sim.intent(p).yaw = yaw;
    sim.seconds(0.5);
    sim.app.world_mut().entity_mut(p).insert((
        Health {
            current: health,
            max: health,
        },
        combat_arms::game::weapons::gear_armour(0.0, &Vec::new()),
    ));
    p
}

/// Give `p` the weapon `id`, drawn, with its meters at rest.
fn arm(sim: &mut Sim, p: Entity, id: &str) -> Entity {
    let w = weapon::give(sim.app.world_mut(), p, id).expect("unknown weapon");
    sim.seconds(3.0);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(w), "{id} drawn");
    w
}

/// A player holding `id` in the open.
fn holding(sim: &mut Sim, id: &str) -> (Entity, Entity) {
    let p = player(sim, Vec3::new(30.0, 1.0, 30.0), 0.0, 1.0);
    let w = arm(sim, p, id);
    (p, w)
}

fn cz(sim: &Sim, w: Entity) -> CaZoom {
    sim.app.world().get::<CaZoom>(w).unwrap().clone()
}

fn view(sim: &Sim, p: Entity) -> Option<Zoomed> {
    sim.app.world().get::<Zoomed>(p).copied()
}

fn mag(sim: &Sim, w: Entity) -> Magazine {
    sim.app.world().get::<Magazine>(w).unwrap().clone()
}

fn set_mag(sim: &mut Sim, w: Entity, clip: u32, reserve: u32) {
    let mut m = sim.app.world_mut().get_mut::<Magazine>(w).unwrap();
    m.clip = clip;
    m.reserve = reserve;
}

fn state(sim: &Sim, w: Entity) -> WeaponState {
    sim.app.world().get::<WeaponState>(w).unwrap().clone()
}

/// The cone (thousandths) the frame shows for the active weapon: the one a
/// round fired on the last tick used.
fn shown_cone(sim: &Sim, w: Entity) -> i32 {
    let SpreadShape::Disc { inaccuracy, .. } = sim.app.world().get::<Hitscan>(w).unwrap().spread else {
        panic!("a disc")
    };
    (inaccuracy * 1000.0).round() as i32
}

/// Attack2 down for one tick (the press is processed on it).
fn press_zoom(sim: &mut Sim, p: Entity) {
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
}

/// Zoom in and wait for the transition.
fn zoomed_in(sim: &mut Sim, p: Entity, w: Entity) {
    press_zoom(sim, p);
    for _ in 0..100 {
        if cz(sim, w).transition.is_none() {
            break;
        }
        sim.ticks(1);
    }
    assert!(cz(sim, w).level > 0 && cz(sim, w).transition.is_none(), "zoomed in");
}

/// Ticks until `done` (at most `max`): how many.
fn ticks_until(sim: &mut Sim, max: u32, mut done: impl FnMut(&Sim) -> bool) -> u32 {
    for k in 1..=max {
        sim.ticks(1);
        if done(sim) {
            return k;
        }
    }
    panic!("not within {max} ticks");
}

fn close(a: f64, b: f64, tol: f64, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a}, expected {b}");
}

/// The rounds `w` fired at or after `from`: their times.
fn rounds(sim: &Sim, w: Entity, from: f64) -> Vec<f64> {
    sim.app
        .world()
        .resource::<Log>()
        .0
        .iter()
        .filter(|(t, e, k)| *e == w && *t >= from - 1e-9 && matches!(k, WeaponEventKind::Fired { .. }))
        .map(|(t, ..)| *t)
        .collect()
}

// ---------------------------------------------------------------------------
// Zoom and scope (7)

/// 1. L96A1 zoom-in at exact time: 55 degrees at 0.09 s, 20 at 0.18 s (the
/// transition over); FovY 45.83 and 16.67; the overlay from t = 0.
#[test]
fn t01_zoom_in_fov() {
    let mut sim = new_sim();
    let (p, w) = holding(&mut sim, L96A1);
    press_zoom(&mut sim, p);
    assert!(view(&sim, p).is_some_and(|z| z.scope), "the overlay from the press");
    let mut fovs = vec![cz(&sim, w).fov];
    for _ in 1..12 {
        sim.ticks(1);
        fovs.push(cz(&sim, w).fov);
    }
    close(fovs[5] as f64, 55.0, 1e-3, "FOV at 0.09 s");
    close(zoom::vertical_fov(fovs[5]) as f64, 45.833, 1e-3, "FovY at 0.09 s");
    close(fovs[11] as f64, 20.0, 1e-3, "FOV at 0.18 s");
    close(zoom::vertical_fov(fovs[11]) as f64, 16.667, 1e-3, "FovY at the end");
    assert!(cz(&sim, w).transition.is_none(), "over at 0.18 s");
    assert_eq!(view(&sim, p).map(|z| z.fov), Some(fovs[11]), "the camera's FOV");
}

/// 2. At 60 fps: 90 - 6.4815 k after frame k (1..10), 20 after frame 11.
#[test]
fn t02_frame_quantised_zoom_in() {
    let mut sim = sim_at(1.0 / 60.0);
    let (p, w) = holding(&mut sim, L96A1);
    press_zoom(&mut sim, p);
    for k in 1..=11 {
        if k > 1 {
            sim.ticks(1);
        }
        let want = if k <= 10 { 90.0 - 6.4815 * k as f64 } else { 20.0 };
        close(cz(&sim, w).fov as f64, want, 1e-3, &format!("after frame {k}"));
    }
    assert!(cz(&sim, w).transition.is_none());
}

/// 3. Zoom out from L = 1: L = 0 at once, 55 degrees at 0.09 s, 90 and the
/// overlay gone at 0.18 s.
#[test]
fn t03_zoom_out() {
    let mut sim = new_sim();
    let (p, w) = holding(&mut sim, L96A1);
    zoomed_in(&mut sim, p, w);
    press_zoom(&mut sim, p);
    assert_eq!(cz(&sim, w).level, 0, "level 0 at once");
    sim.ticks(5);
    close(cz(&sim, w).fov as f64, 55.0, 1e-3, "at 0.09 s");
    assert!(view(&sim, p).is_some(), "the overlay stays while zooming out");
    sim.ticks(6);
    close(cz(&sim, w).fov as f64, 90.0, 1e-3, "at 0.18 s");
    assert!(view(&sim, p).is_none(), "the overlay gone at 0.18 s");
}

/// 4. Two levels (M107CQ, Barrett Scope, 0.16 s): 30 degrees in 0.16 s,
/// 10 in 0.04 s more, back to 90 in 0.1829 s.
#[test]
fn t04_two_level_scope() {
    let mut sim = new_sim();
    let (p, w) = holding(&mut sim, "test:m107cq");
    let tick = dt(&sim);
    for (target, secs) in [(30.0, 0.16), (10.0, 0.04), (90.0, 0.182857)] {
        press_zoom(&mut sim, p);
        let k = 1 + if cz(&sim, w).transition.is_some() {
            ticks_until(&mut sim, 100, |s| cz(s, w).transition.is_none())
        } else {
            0
        };
        close(cz(&sim, w).fov as f64, target, 1e-3, "arrived");
        close(k as f64 * tick, secs, tick, &format!("time to {target}"));
        sim.seconds(0.3);
    }
    assert_eq!(cz(&sim, w).level, 0);
}

/// 5. A press during the transition is dropped: L = 1, 20 degrees at 0.18 s.
#[test]
fn t05_press_during_a_transition() {
    let mut sim = new_sim();
    let (p, w) = holding(&mut sim, L96A1);
    press_zoom(&mut sim, p);
    sim.ticks(5);
    // 0.1 s.
    press_zoom(&mut sim, p);
    assert_eq!(cz(&sim, w).level, 1, "dropped");
    sim.ticks(5);
    assert_eq!(cz(&sim, w).level, 1);
    close(cz(&sim, w).fov as f64, 20.0, 1e-3, "at 0.18 s");
}

/// 6. Dot sights with ZoomTime 0.15: 50 degrees in 0.15 s, out in 0.0857;
/// 75 in 0.15, out in 0.0321.
#[test]
fn t06_dot_sight_speeds() {
    for (id, out) in [("test:dot50", 0.085714), ("test:dot75", 0.032143)] {
        let mut sim = new_sim();
        let (p, w) = holding(&mut sim, id);
        let tick = dt(&sim);
        for secs in [0.15, out] {
            press_zoom(&mut sim, p);
            let k = 1 + if cz(&sim, w).transition.is_some() {
                ticks_until(&mut sim, 100, |s| cz(s, w).transition.is_none())
            } else {
                0
            };
            close(k as f64 * tick, secs, tick, &format!("{id}: {secs}"));
            sim.seconds(0.2);
        }
    }
}

/// 7. Mouse scale: 1 at L = 0, 1/4 at L = 1, 1/7 at L = 2, switched with
/// the level on the press (already 1/4 mid zoom-in, 1 mid zoom-out).
#[test]
fn t07_mouse_scale() {
    let mut sim = new_sim();
    let (p, w) = holding(&mut sim, "test:m107cq");
    assert!(view(&sim, p).is_none(), "1.000 degree at L = 0: no zoom view");
    press_zoom(&mut sim, p);
    assert!(cz(&sim, w).fov > 30.0, "still zooming in");
    close(view(&sim, p).unwrap().sensitivity.unwrap() as f64, 0.25, 1e-6, "L = 1 mid zoom-in");
    sim.seconds(0.3);
    press_zoom(&mut sim, p);
    close(view(&sim, p).unwrap().sensitivity.unwrap() as f64, 1.0 / 7.0, 1e-6, "L = 2");
    close(1.0 * view(&sim, p).unwrap().sensitivity.unwrap() as f64, 0.143, 5e-4, "turned degrees");
    sim.seconds(0.3);
    press_zoom(&mut sim, p);
    assert!(cz(&sim, w).transition.is_some(), "zooming out");
    close(view(&sim, p).unwrap().sensitivity.unwrap() as f64, 1.0, 1e-6, "mid zoom-out");
}

/// 8. The no-scope cone is 100 whatever the meters; draws (1, 0) bend
/// forward by (0.1, -0.1).
#[test]
fn t08_no_scope_cone() {
    let mut sim = new_sim();
    let (_, w) = holding(&mut sim, L96A1);
    let mut s = sim.app.world().get::<CaSpread>(w).unwrap().clone();
    for (f, m) in [(0.0, 0.0), (0.5, 0.2), (1.0, 1.0)] {
        s.fire = f;
        s.movement = m;
        assert_eq!(s.cone(FireMode::Normal), 100.0);
    }
    let d = ca::square_dir(Quat::IDENTITY, 0.1, 1.0, 0.0);
    // The engine's frame: forward -Z, right +X, up +Y.
    let want = Vec3::new(0.1, -0.1, -1.0).normalize();
    assert!(d.distance(want) < 1e-6, "{d}");
}

/// 9. Quick-scope: zoom and fire on the same frame: n = 4 (FOV still 90).
#[test]
fn t09_quick_scope() {
    let mut sim = new_sim();
    let (p, w) = holding(&mut sim, L96A1);
    assert_eq!(shown_cone(&sim, w), 100);
    sim.intent(p).secondary = true;
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    // The round 3 updates after the press (Corrections 3), mid-transition.
    until_round(&mut sim, w);
    assert_eq!(shown_cone(&sim, w), 4, "the round's cone");
    assert!(cz(&sim, w).fov > 20.0, "FOV still in its transition: {}", cz(&sim, w).fov);
}

/// 10. Scoped and walking at full scoped speed (mu = 1): the movement
/// meter takes 27 sub-steps to 1.0 (n = 60); stopped, 19 back to n = 4.
#[test]
fn t10_scoped_while_moving() {
    let mut sim = new_sim();
    let (_, w) = holding(&mut sim, L96A1);
    let sc = *sim.app.world().resource::<SpreadScales>();
    let mut s = sim.app.world().get::<CaSpread>(w).unwrap().clone();
    s.fire = 0.0;
    s.movement = 0.0;
    let cone = s.cone_params(FireMode::Zoomed);
    let st = s.states(FireMode::Zoomed);
    for k in 1..=27 {
        s.move_substep(&sc, cone, st.walk, st.walk, 0.0, false, st.jump);
        if k == 26 {
            assert!(s.movement < 1.0);
        }
    }
    close(s.movement as f64, 1.0, 1e-5, "M after 27");
    assert_eq!(s.cone(FireMode::Zoomed), 60.0);
    for k in 1..=19 {
        s.move_substep(&sc, cone, st.walk, 0.0, 0.0, false, st.jump);
        if k == 18 {
            assert!(s.cone(FireMode::Zoomed) > 4.0, "not yet after 18");
        }
    }
    assert_eq!(s.cone(FireMode::Zoomed), 4.0, "after 19");
}

/// 11. Scoped follow-ups within 2400 ms: L96A1 (re-scoped each time) 4, 6,
/// 8, 10, 12, 14; Dragunov (stays zoomed) 2, 3, 4, 5, 6, 7.
#[test]
fn t11_scoped_follow_ups() {
    let mut sim = new_sim();
    let (p, w) = holding(&mut sim, "test:l96a1_ten");
    let mut cones = Vec::new();
    for _ in 0..6 {
        // Re-scope and fire on the same frame, once the bolt is done.
        ticks_until(&mut sim, 200, |s| !cz(s, w).bolt_busy && cz(s, w).transition.is_none());
        sim.intent(p).secondary = cz(&sim, w).level == 0;
        sim.intent(p).fire = true;
        sim.ticks(1);
        sim.intent(p).secondary = false;
        sim.intent(p).fire = false;
        until_round(&mut sim, w);
        cones.push(shown_cone(&sim, w));
    }
    assert_eq!(cones, [4, 6, 8, 10, 12, 14]);

    let mut sim = new_sim();
    let (p, w) = holding(&mut sim, SVD);
    zoomed_in(&mut sim, p, w);
    let mut cones = Vec::new();
    for _ in 0..6 {
        sim.intent(p).fire = true;
        sim.ticks(1);
        sim.intent(p).fire = false;
        until_round(&mut sim, w);
        cones.push(shown_cone(&sim, w));
        sim.seconds(0.45);
    }
    assert_eq!(cones, [2, 3, 4, 5, 6, 7]);
    assert_eq!(cz(&sim, w).level, 1, "stayed zoomed");
}

/// 12. Scoped speed: the reference character holding run, 188.1 u/s with
/// the L96A1; zoom pressed: 46.2 u/s from that frame.
#[test]
fn t12_scoped_speed() {
    let mut sim = new_sim();
    let (p, _) = holding(&mut sim, L96A1);
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(1.0);
    let limit = |s: &Sim| s.app.world().get::<CaMovement>(p).unwrap().speed_limit as f64;
    close(limit(&sim), 188.1, 1e-3, "running");
    press_zoom(&mut sim, p);
    close(limit(&sim), 46.2, 1e-3, "from the press's frame");
}

/// 13. Bolt cycle: a zoomed shot, the zoom-out on the first frame after
/// 300 ms, done 180 ms later; a press at 1000 ms dropped, at 1400 ms + one
/// update taken; fire held to 3000 ms: one round.
#[test]
fn t13_bolt_cycle_and_zoom_out() {
    let mut sim = sim_at(0.01);
    let (p, w) = holding(&mut sim, L96A1);
    zoomed_in(&mut sim, p, w);
    let clip = mag(&sim, w).clip;
    sim.intent(p).fire = true;
    // t = 0 at the round (3 updates after the press: Corrections 3).
    until_round(&mut sim, w);
    let t0 = now(&sim);
    ticks_until(&mut sim, 100, |s| cz(s, w).level == 0);
    close(now(&sim) - t0, 0.31, 1e-6, "zoom-out starts on the first frame after 300 ms");
    ticks_until(&mut sim, 100, |s| cz(s, w).transition.is_none());
    close(now(&sim) - t0, 0.49, 0.0101, "90 degrees about 180 ms later");
    assert!(view(&sim, p).is_none());
    // 1000 ms: dropped.
    while now(&sim) - t0 < 1.0 - 1e-9 {
        sim.ticks(1);
    }
    press_zoom(&mut sim, p);
    assert_eq!(cz(&sim, w).level, 0, "at 1000 ms: dropped");
    // The fire animation ends at 1400 ms: the update at 1400 still sees
    // it (it leaves the firing state on that update's weapon step); the
    // next press (after a release) is taken.
    while now(&sim) - t0 < 1.39 - 1e-9 {
        sim.ticks(1);
    }
    press_zoom(&mut sim, p);
    close(now(&sim) - t0, 1.4, 1e-6, "pressed at 1400 ms");
    assert_eq!(cz(&sim, w).level, 0, "at 1400 ms the weapon is still firing");
    sim.ticks(1);
    press_zoom(&mut sim, p);
    assert_eq!(cz(&sim, w).level, 1, "after it: taken");
    while now(&sim) - t0 < 3.0 {
        sim.ticks(1);
    }
    assert_eq!(mag(&sim, w).clip, clip - 1, "one round");
}

/// 14. A bolt with a two-level scope (ASW338LM, 0.15 s) at L = 1: after
/// 300 ms L = 2 (30 to 10 in 0.0375 s), then L = 0 (10 to 90 in 0.1714 s).
#[test]
fn t14_bolt_with_a_two_level_scope() {
    let mut sim = new_sim();
    let (p, w) = holding(&mut sim, "test:asw338lm");
    zoomed_in(&mut sim, p, w);
    let tick = dt(&sim);
    sim.intent(p).fire = true;
    until_round(&mut sim, w);
    sim.intent(p).fire = false;
    let t0 = now(&sim);
    ticks_until(&mut sim, 100, |s| cz(s, w).level == 2);
    assert!(now(&sim) - t0 > 0.3 && now(&sim) - t0 <= 0.3 + tick + 1e-9, "level 2 after 300 ms");
    let k = ticks_until(&mut sim, 100, |s| cz(s, w).level == 0);
    close(k as f64 * tick, 0.0375, tick, "30 to 10");
    close(cz(&sim, w).fov as f64, 10.0, 1e-3, "from 10");
    let k = ticks_until(&mut sim, 100, |s| cz(s, w).transition.is_none());
    close(k as f64 * tick, 0.171429, tick, "10 to 90");
}

/// 15. A semi sniper stays zoomed: still L = 1 at 1000 ms; a press at
/// 200 ms held fires at 400 ms.
#[test]
fn t15_semi_sniper_stays_zoomed() {
    let mut sim = sim_at(0.01);
    let (p, w) = holding(&mut sim, SVD);
    zoomed_in(&mut sim, p, w);
    sim.intent(p).fire = true;
    until_round(&mut sim, w);
    let t0 = now(&sim);
    sim.intent(p).fire = false;
    while now(&sim) - t0 < 0.2 - 1e-9 {
        sim.ticks(1);
    }
    sim.intent(p).fire = true;
    while now(&sim) - t0 < 1.0 - 1e-9 {
        sim.ticks(1);
    }
    let fired = rounds(&sim, w, t0);
    assert_eq!(fired.len(), 2, "{fired:?}");
    close(fired[1] - t0, 0.4, 1e-6, "second round");
    assert_eq!(cz(&sim, w).level, 1, "zoomed at 1000 ms");
}

/// 16. Reload while scoped (semi): zoom-out 0 to 0.1 s, the reload from
/// 0.1 s, clip 10 at 2.7 s.
#[test]
fn t16_reload_while_scoped_semi() {
    let mut sim = new_sim();
    let (p, w) = holding(&mut sim, SVD);
    zoomed_in(&mut sim, p, w);
    set_mag(&mut sim, w, 5, 35);
    let tick = dt(&sim);
    sim.intent(p).reload = true;
    sim.ticks(1);
    sim.intent(p).reload = false;
    let t0 = now(&sim);
    assert!(state(&sim, w).reload_end.is_none(), "not yet: zooming out");
    ticks_until(&mut sim, 100, |s| s.app.world().get::<WeaponState>(w).unwrap().reload_end.is_some());
    close(now(&sim) - t0, 0.1, tick, "the reload from 0.1 s");
    assert!(view(&sim, p).is_none(), "zoomed out");
    ticks_until(&mut sim, 400, |s| mag(s, w).clip == 10);
    close(now(&sim) - t0, 2.7, tick, "clip 10");
}

/// 17. Reload while scoped (bolt): zoom-out from 300 ms to 480 ms, the
/// reload from 480 ms, clip 5 at 3380 ms.
#[test]
fn t17_reload_while_scoped_bolt() {
    // At 10 ms, so "more than 300 ms" is 310 (one tick).
    let mut sim = sim_at(0.01);
    let (p, w) = holding(&mut sim, L96A1);
    zoomed_in(&mut sim, p, w);
    set_mag(&mut sim, w, 3, 17);
    let tick = dt(&sim);
    sim.intent(p).reload = true;
    sim.ticks(1);
    sim.intent(p).reload = false;
    let t0 = now(&sim);
    ticks_until(&mut sim, 100, |s| cz(s, w).level == 0);
    close(now(&sim) - t0, 0.3, tick, "zoom-out after 300 ms");
    ticks_until(&mut sim, 100, |s| s.app.world().get::<WeaponState>(w).unwrap().reload_end.is_some());
    close(now(&sim) - t0, 0.48, tick, "the reload from 480 ms");
    ticks_until(&mut sim, 400, |s| mag(s, w).clip == 5);
    close(now(&sim) - t0, 3.38, tick, "clip 5");
}

/// 18. A refused reload (full clip) leaves the zoom alone.
#[test]
fn t18_reload_refused_while_scoped() {
    let mut sim = new_sim();
    let (p, w) = holding(&mut sim, L96A1);
    zoomed_in(&mut sim, p, w);
    sim.intent(p).reload = true;
    sim.seconds(1.0);
    sim.intent(p).reload = false;
    assert_eq!(cz(&sim, w).level, 1);
    assert!(cz(&sim, w).bolt_armed.is_none());
    assert!(!state(&sim, w).reloading());
}

/// 19. Switching while zoomed (or mid-transition): 90 degrees on that frame,
/// no transition, the overlay gone; the pistol's 650 ms draw starts.
#[test]
fn t19_switch_while_zoomed() {
    for mid in [false, true] {
        let mut sim = new_sim();
        let p = player(&mut sim, Vec3::new(30.0, 1.0, 30.0), 0.0, 1.0);
        arm(&mut sim, p, M92FS);
        let w = arm(&mut sim, p, L96A1);
        if mid {
            press_zoom(&mut sim, p);
        } else {
            zoomed_in(&mut sim, p, w);
        }
        sim.intent(p).select = Some(1);
        sim.ticks(1);
        sim.intent(p).select = None;
        let inv = sim.app.world().get::<Inventory>(p).unwrap().clone();
        let pistol = inv.active.unwrap();
        assert_eq!(sim.app.world().get::<weapon::Weapon>(pistol).unwrap().id, M92FS);
        let z = cz(&sim, w);
        assert_eq!((z.level, z.fov, z.transition), (0, 90.0, None), "reset at once (mid {mid})");
        assert!(view(&sim, p).is_none(), "no overlay");
        close(inv.next_attack - now(&sim), 0.65, 1e-6, "the pistol's draw");
    }
}

/// 20. Sway: the aim drifts by the integral of (S / 1000) sin(t F / 10):
/// from t = 0 to pi, +0.0090 rad of yaw and +0.0022 rad of pitch (down);
/// over 2 pi, nothing.
#[test]
fn t20_sway_drift() {
    // The spec's numbers from the formula with the L96A1's sway.
    let drift = |s: f64, f: f64, a: f64, b: f64| s / 1000.0 * (10.0 / f) * ((a * f / 10.0).cos() - (b * f / 10.0).cos());
    close(drift(4.5, 10.0, 0.0, std::f64::consts::PI), 0.0090, 1e-4, "yaw over 0..pi");
    close(drift(1.1, 10.0, 0.0, std::f64::consts::PI), 0.0022, 1e-4, "pitch over 0..pi");
    close(drift(4.5, 10.0, 0.0, std::f64::consts::TAU), 0.0, 1e-9, "over 2 pi");
    let mut sim = new_sim();
    let (p, w) = holding(&mut sim, L96A1);
    zoomed_in(&mut sim, p, w);
    for span in [std::f64::consts::PI, std::f64::consts::TAU] {
        let before = sim.app.world().get::<ViewPunch>(p).map_or(Vec3::ZERO, |v| v.0);
        let tick = dt(&sim);
        let a = now(&sim) + tick;
        let n = (span / tick).round() as u64;
        sim.ticks(n);
        let b = now(&sim) + tick;
        let after = sim.app.world().get::<ViewPunch>(p).unwrap().0;
        // Combat Arms' yaw right and pitch down are the engine's -yaw, -pitch.
        close(-(after.y - before.y) as f64, drift(4.5, 10.0, a, b), 1e-4, "yaw");
        close(-(after.x - before.x) as f64, drift(1.1, 10.0, a, b), 1e-4, "pitch");
    }
}

// ---------------------------------------------------------------------------
// Shotguns (5)

/// Hold fire (as set) until `w` spends a round (its chain: at most 3
/// updates after the press's, weapon_classes.md Corrections 3); the
/// round's tick is the last one run.
fn until_round(sim: &mut Sim, w: Entity) {
    let clip = mag(sim, w).clip;
    ticks_until(sim, 6, |s| mag(s, w).clip < clip);
}

/// Fire one round with a one-tick pull, then let go until the chain's
/// round has gone (3 updates at most: Corrections 3).
fn fire_once(sim: &mut Sim, p: Entity) {
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    sim.ticks(4);
}

/// 21. The Remington 870's cone is 42 for every shot: standing, running,
/// airborne after a jump; shots 600 ms apart.
#[test]
fn t21_shotgun_cone_is_constant() {
    let mut sim = new_sim();
    let (p, w) = holding(&mut sim, R870);
    let mut cones = Vec::new();
    let mut shoot = |sim: &mut Sim| {
        sim.intent(p).fire = true;
        sim.ticks(1);
        sim.intent(p).fire = false;
        cones.push(shown_cone(sim, w));
        sim.seconds(0.6);
    };
    shoot(&mut sim);
    shoot(&mut sim);
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(0.5);
    shoot(&mut sim);
    shoot(&mut sim);
    sim.intent(p).jump = true;
    sim.ticks(3);
    sim.intent(p).jump = false;
    assert!(!sim.state(p).on_ground, "airborne");
    shoot(&mut sim);
    assert_eq!(cones, [42; 5]);
}

/// A shooter looking down -Z at a target facing it, `units` apart.
fn range_lane(sim: &mut Sim, id: &str, units: f32) -> (Entity, Entity) {
    let shooter = player(sim, Vec3::new(30.0, 1.0, 37.0), 0.0, 1.0);
    let target = player(sim, Vec3::new(30.0, 1.0, 37.0 - units * U), PI, 100.0);
    arm(sim, shooter, id);
    (shooter, target)
}

fn eye(sim: &Sim, p: Entity) -> Vec3 {
    sim.position(p) + sim.state(p).eye_offset
}

fn geometry(world: &World, p: Entity) -> (Vec3, Vec3, Vec<(Vec3, f32)>) {
    ca::body_geometry(
        world.get::<Transform>(p).unwrap(),
        world.get::<Intent>(p).unwrap(),
        world.get::<MovementState>(p).unwrap(),
        world.get::<ColliderAabb>(p),
        *MALE,
    )
    .unwrap()
}

fn sphere(sim: &Sim, shooter: Entity, p: Entity, name: &str) -> Vec3 {
    super::aim_points::point(sim.app.world(), shooter, p, name)
}

fn aim_at(sim: &mut Sim, shooter: Entity, point: Vec3) {
    let d = point - eye(sim, shooter);
    let mut i = sim.intent(shooter);
    i.yaw = (-d.x).atan2(-d.z);
    i.pitch = d.y.atan2(d.xz().length());
}

/// Move `target` along -Z until the ray to its `name` sphere enters its
/// box `units` from the shooter's eye.
fn place_at(sim: &mut Sim, shooter: Entity, target: Entity, name: &str, units: f32) {
    for _ in 0..6 {
        let from = eye(sim, shooter);
        let dir = (sphere(sim, shooter, target, name) - from).normalize();
        let (min, max, _) = geometry(sim.app.world(), target);
        // The reported point: the widened box's entry (weapon_firing.md
        // Corrections 8).
        let (c, half) = ((min + max) / 2.0, (max - min) / 2.0 * Vec3::new(ca::HITBOX_WIDEN, 1.0, ca::HITBOX_WIDEN));
        let t = ca::box_entry(c - half, c + half, from, dir, f32::INFINITY).unwrap();

        let err = t - units * U;
        sim.app.world_mut().get_mut::<Transform>(target).unwrap().translation.z += err / dir.z.abs();
        sim.ticks(1);
    }
}

/// Aim at `name` on `target`, fire one round: the hit points it lost.
fn shoot_at(sim: &mut Sim, shooter: Entity, target: Entity, name: &str) -> f64 {
    let at = sphere(sim, shooter, target, name);
    aim_at(sim, shooter, at);
    let before = sim.app.world().get::<Health>(target).unwrap().current;
    fire_once(sim, shooter);
    sim.ticks(1);
    let lost = (before - sim.app.world().get::<Health>(target).unwrap().current) / HP;
    sim.app.world_mut().get_mut::<Health>(target).unwrap().current = before;
    // Ready for the next round (the slowest cycle here is 1.4 s).
    sim.seconds(1.5);
    lost as f64
}

/// 22. Pellet damage and falloff: 8 on the torso at 500 / 600 / 700 / 900
/// units, as corrected (Corrections 1, case 37: the server drops the last
/// pellet at `FireDelay` 30) 7 land: 63 / 63 / 44.1 / 22.05 (the spec's
/// 72 / 72 / 50.4 / 25.2 counted 8); past 1000, nothing. One head pellet
/// and 7 torso at 500: 54 + 6 x 9 = 108 (the spec's 117).
#[test]
fn t22_pellet_damage_and_falloff() {
    let mut sim = new_sim();
    let (shooter, target) = range_lane(&mut sim, "test:r870_exact", 500.0);
    for (d, want) in [(500.0, 63.0), (600.0, 63.0), (700.0, 44.1), (900.0, 22.05), (1100.0, 0.0)] {
        place_at(&mut sim, shooter, target, "Torso", d);
        close(shoot_at(&mut sim, shooter, target, "Torso"), want, 1e-2, &format!("at {d}"));
    }
    let mut sim = new_sim();
    let (shooter, target) = range_lane(&mut sim, "test:r870_one_pellet", 500.0);
    place_at(&mut sim, shooter, target, "Torso", 500.0);
    let head = shoot_at(&mut sim, shooter, target, "Face_Eyelid1");
    arm(&mut sim, shooter, "test:r870_seven_pellets");
    let torso = shoot_at(&mut sim, shooter, target, "Torso");
    close(head, 54.0, 1e-3, "the head pellet");
    close(head + torso, 108.0, 1e-2, "one head and 7 torso (the 7th dropped)");
}

/// 23. Pellet hit carry-over: only pellet 3 hits (torso, 500 units): with
/// the option 6 hits, 54; without, 1, 9. Only pellet 1: 8 hits, 72; only
/// pellet 8: 9. And in play: a shot whose first real hit is pellet k deals
/// (8 - k + 1) pellets' worth with the option, its real hits' without.
#[test]
fn t23_pellet_carry_over() {
    let one = |i: usize| -> Vec<Option<()>> { (1..=8).map(|k| (k == i).then_some(())).collect() };
    let hits = |real: &[Option<()>], on: bool| reported_hits(real, on).iter().filter(|h| h.is_some()).count();
    assert_eq!((hits(&one(3), true), 9 * hits(&one(3), true)), (6, 54));
    assert_eq!((hits(&one(3), false), 9 * hits(&one(3), false)), (1, 9));
    assert_eq!(9 * hits(&one(1), true), 72);
    assert_eq!(9 * hits(&one(8), true), 9);

    // In play: the same shot (same seed) with and without the option;
    // each pellet's hit (the damage it reports), in pellet order.
    let run = |on: bool| -> Vec<Option<i64>> {
        let mut sim = new_sim();
        sim.app.insert_resource(PelletCarryOver(on));
        let (shooter, target) = range_lane(&mut sim, "test:r870_wide", 500.0);
        place_at(&mut sim, shooter, target, "Torso", 500.0);
        let at = sphere(&sim, shooter, target, "Torso");
        aim_at(&mut sim, shooter, at);
        sim.app.world_mut().resource_mut::<Log>().0.clear();
        fire_once(&mut sim, shooter);
        // A pellet's trace starts it (`Shot`); its hits follow.
        let mut pellets: Vec<Option<i64>> = Vec::new();
        for (_, _, k) in &sim.app.world().resource::<Log>().0 {
            match k {
                WeaponEventKind::Shot { .. } => pellets.push(None),
                WeaponEventKind::Hit { amount, .. } => {
                    *pellets.last_mut().unwrap() = Some((*amount as f64 / HP as f64 * 1000.0).round() as i64)
                }
                _ => {}
            }
        }
        pellets
    };
    let with = run(true);
    let without = run(false);
    assert_eq!(without.len(), 8);
    // The 8th report is the server's spacing drop either way (case 38).
    assert_eq!((with[7], without[7]), (None, None), "the last pellet dropped");
    let first = without[..7].iter().position(Option::is_some).expect("a pellet hits");
    let real = without.iter().filter(|h| h.is_some()).count();
    assert!(real < 7, "some pellets miss: {without:?}");
    assert_eq!(with[..7], reported_hits(&without[..7], true), "carried from pellet {}", first + 1);
    assert_eq!(with.iter().filter(|h| h.is_some()).count(), 7 - first);

}

/// The times (relative to `t0`) at which `w`'s clip changed, and what to.
fn clip_changes(sim: &mut Sim, w: Entity, t0: f64, until: f64) -> Vec<(f64, u32)> {
    let mut out = Vec::new();
    let mut last = mag(sim, w).clip;
    while now(sim) - t0 < until {
        sim.ticks(1);
        let c = mag(sim, w).clip;
        if c != last {
            out.push((now(sim) - t0, c));
            last = c;
        }
    }
    out
}

/// 24. Shell reload timing: clip 3 of 35 → 4 / 5 / 6 / 7 at 1033 / 1733 /
/// 2433 / 3133 ms, idle at 4033; from 0 with 7: the last at 5233, idle
/// at 6133.
#[test]
fn t24_shell_reload_timing() {
    let mut sim = new_sim();
    let (p, w) = holding(&mut sim, R870);
    let tick = dt(&sim);
    set_mag(&mut sim, w, 3, 32);
    sim.intent(p).reload = true;
    sim.ticks(1);
    sim.intent(p).reload = false;
    let t0 = now(&sim);
    let mut idle = None;
    let mut changes = Vec::new();
    let mut last = 3;
    while now(&sim) - t0 < 4.5 {
        sim.ticks(1);
        let c = mag(&sim, w).clip;
        if c != last {
            changes.push((now(&sim) - t0, c));
            last = c;
        }
        if idle.is_none() && state(&sim, w).shells == ShellStage::Idle {
            idle = Some(now(&sim) - t0);
        }
    }
    let want = [(1.033, 4), (1.733, 5), (2.433, 6), (3.133, 7)];
    assert_eq!(changes.len(), 4, "{changes:?}");
    for ((t, c), (wt, wc)) in changes.iter().zip(want) {
        assert_eq!(*c, wc);
        close(*t, wt, tick, &format!("clip {c}"));
    }
    close(idle.unwrap(), 4.033, tick, "idle");

    // From empty with 7: the automatic reload.
    let mut sim = new_sim();
    let (_, w) = holding(&mut sim, R870);
    set_mag(&mut sim, w, 0, 7);
    sim.ticks(1);
    let t0 = now(&sim);
    assert!(state(&sim, w).reloading(), "reloads by itself");
    let changes = clip_changes(&mut sim, w, t0, 6.5);
    assert_eq!(changes.len(), 7, "{changes:?}");
    close(changes[6].0, 5.233, tick, "the last shell");
    assert_eq!(mag(&sim, w).clip, 7);
    close(changes[0].0, 1.033, tick, "the first shell");
    // Idle 900 ms after the last.
    let mut sim = new_sim();
    let (_, w) = holding(&mut sim, R870);
    set_mag(&mut sim, w, 0, 7);
    sim.ticks(1);
    let t0 = now(&sim);
    ticks_until(&mut sim, 1000, |s| s.app.world().get::<WeaponState>(w).unwrap().shells == ShellStage::Idle);
    close(now(&sim) - t0, 6.133, tick, "idle");
}

/// 25. Fire at 1500 ms stops the reload of case 24 at once: clip 4 (the
/// loop in progress lost), one round fired: clip 3, total 34; no
/// post-reload.
#[test]
fn t25_shell_reload_interrupted() {
    let mut sim = new_sim();
    let (p, w) = holding(&mut sim, R870);
    set_mag(&mut sim, w, 3, 32);
    sim.intent(p).reload = true;
    sim.ticks(1);
    sim.intent(p).reload = false;
    let t0 = now(&sim);
    while now(&sim) - t0 < 1.5 - 1e-9 {
        sim.ticks(1);
    }
    assert_eq!(mag(&sim, w).clip, 4);
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    // The shotgun's chain: 2 updates (Corrections 3).
    until_round(&mut sim, w);
    let m = mag(&sim, w);
    assert_eq!((m.clip, m.clip + m.reserve), (3, 34));
    assert_eq!(rounds(&sim, w, t0).len(), 1, "one round at once");
    assert_eq!(state(&sim, w).shells, ShellStage::Idle, "no post-reload");
    sim.seconds(1.0);
    assert_eq!(mag(&sim, w).clip, 3, "nothing more goes in");
}

/// 26. An empty pump gun with fire held: a shell at 1033 ms fires at once,
/// the pump to 1633, the reload again: rounds at about 1033, 2666, 4299.
#[test]
fn t26_empty_pump_gun_trigger_held() {
    let mut sim = new_sim();
    let (p, w) = holding(&mut sim, R870);
    let tick = dt(&sim);
    set_mag(&mut sim, w, 0, 35);
    sim.intent(p).fire = true;
    sim.ticks(1);
    let t0 = now(&sim);
    while now(&sim) - t0 < 4.5 {
        sim.ticks(1);
    }
    let fired: Vec<f64> = rounds(&sim, w, t0).iter().map(|t| t - t0).collect();
    assert_eq!(fired.len(), 3, "{fired:?}");
    // Each shell's round its chain later: 2 updates (Corrections 3).
    for (i, (t, want)) in fired.iter().zip([1.033, 2.666, 4.299]).enumerate() {
        close(*t, want + 2.0 * (i + 1) as f64 * tick, tick, "a round");
    }
}

// ---------------------------------------------------------------------------
// Pistols (4)

/// 27. The M92FS: one round a press; a held re-press during the fire
/// animation fires as it ends (180 ms); a tap inside it is lost; a press
/// after it, during the 150 ms `postfire`, cuts that short and fires on
/// the update that sees it (view_models.md 7: no chain, "second round at
/// 250").
#[test]
fn t27_pistol_semi_auto() {
    // (press, release) pairs in ms, and the second round's time after the
    // first's (which comes the chain's 2 updates after its press).
    type Case = (&'static [(u32, u32)], Option<f64>);
    let cases: [Case; 4] = [
        (&[(0, 1000)], None),
        (&[(0, 50), (100, 1000)], Some(0.18)),
        (&[(0, 50), (100, 150)], None),
        (&[(0, 50), (250, 1000)], Some(0.23)),
    ];
    for (presses, second) in cases {
        let mut sim = sim_at(0.01);
        let (p, w) = holding(&mut sim, M92FS);
        let t0 = now(&sim) + 0.01;
        for ms in 0..1000u32 {
            let held = presses.iter().any(|(a, b)| (*a..*b).contains(&(ms - ms % 10)));
            if ms % 10 == 0 {
                sim.intent(p).fire = held;
                sim.ticks(1);
            }
        }
        let fired: Vec<f64> = rounds(&sim, w, t0).iter().map(|t| t - t0).collect();
        assert_eq!(fired.len(), 1 + second.is_some() as usize, "{presses:?}: {fired:?}");
        // The chain: 2 updates after the press (no pre-fire for pistols:
        // Corrections (2026-10-30) 1), and everything after it as much
        // later.
        close(fired[0], 0.02, 1e-6, "the first round");
        if let Some(s) = second {
            close(fired[1], s + 0.02, 0.0101, &format!("{presses:?}"));
        }
    }
}

/// 28. The Desert Eagle is automatic: held with 7, rounds at 0, 600, ...,
/// 3600 ms, then the automatic reload.
#[test]
fn t28_desert_eagle_hold() {
    let mut sim = new_sim();
    let (p, w) = holding(&mut sim, "test:desert_eagle");
    let tick = dt(&sim);
    sim.intent(p).fire = true;
    until_round(&mut sim, w);
    let t0 = now(&sim);
    sim.seconds(4.5);
    let fired: Vec<f64> = rounds(&sim, w, t0).iter().map(|t| t - t0).collect();
    assert_eq!(fired.len(), 7, "{fired:?}");
    for (i, t) in fired.iter().enumerate() {
        close(*t, 0.6 * i as f64, tick, "round");
    }
    let started = sim
        .app
        .world()
        .resource::<Log>()
        .0
        .iter()
        .any(|(t, e, k)| *e == w && *t > t0 + 3.6 && matches!(k, WeaponEventKind::ReloadStarted));
    assert!(started, "the automatic reload");
}

/// Hold fire and collect the cone of each of `n` rounds.
fn cones_held(sim: &mut Sim, p: Entity, w: Entity, n: usize) -> Vec<i32> {
    let mut cones = Vec::new();
    let mut clip = mag(sim, w).clip;
    sim.intent(p).fire = true;
    while cones.len() < n {
        sim.ticks(1);
        let now = mag(sim, w).clip;
        if now < clip {
            cones.push(shown_cone(sim, w));
            clip = now;
        }
    }
    sim.intent(p).fire = false;
    cones
}

/// Fresh kicks with no recovery, degrees up.
fn kicks(sim: &Sim, w: Entity, n: usize) -> Vec<f64> {
    let r = CaRecoil::new(sim.app.world().get::<CaRecoil>(w).unwrap().params);
    let mut k = CaKick::default();
    (0..n).map(|_| r.kick(&mut k, false, 0.0).to_degrees() as f64).collect()
}

/// 29. M92FS standing, every shot within DecDelay: 11, 14, 18, 23, 26, 30,
/// 31, 31; kicks 3.500, 3.063, 2.680, 2.345 degrees.
#[test]
fn t29_pistol_cones_and_kick() {
    let mut sim = new_sim();
    let (p, w) = holding(&mut sim, "test:m92fs_steady");
    let mut cones = Vec::new();
    for _ in 0..8 {
        // A press during the 150 ms post-fire fires on the update that
        // sees it (view_models.md 7), so the clip is read before the press.
        let clip = mag(&sim, w).clip;
        sim.intent(p).fire = true;
        sim.ticks(1);
        sim.intent(p).fire = false;
        if mag(&sim, w).clip == clip {
            until_round(&mut sim, w);
        }
        cones.push(shown_cone(&sim, w));
        sim.seconds(0.2);
    }
    assert_eq!(cones, [11, 14, 18, 23, 26, 30, 31, 31]);
    let (_, w) = holding(&mut sim, M92FS);
    for (got, want) in kicks(&sim, w, 4).iter().zip([3.5, 3.0625, 2.6797, 2.3449]) {
        close(*got, want, 1e-3, "kick");
    }
}

/// 30. MP5A4: cones 8, 10, 13, 16, 19, 21, 24, 24; running with F = 0,
/// 17; damage at 400 / 1000 / 2200 units 31.9 / 20.3 / 10.15; 30 rounds by
/// 2320 ms.
#[test]
fn t30_smg() {
    let mut sim = new_sim();
    let (p, w) = holding(&mut sim, "test:mp5a4_steady");
    assert_eq!(cones_held(&mut sim, p, w, 8), [8, 10, 13, 16, 19, 21, 24, 24]);

    let mut sim = new_sim();
    let (p, w) = holding(&mut sim, MP5);
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(1.0);
    assert!(sim.app.world().get::<CaMovement>(p).unwrap().run, "running");
    assert_eq!(shown_cone(&sim, w), 17);
    sim.intent(p).move_axis = Vec2::ZERO;

    let mut sim = new_sim();
    let (p, w) = holding(&mut sim, MP5);
    let tick = dt(&sim);
    sim.intent(p).fire = true;
    until_round(&mut sim, w);
    let t0 = now(&sim);
    sim.seconds(2.5);
    sim.intent(p).fire = false;
    let fired = rounds(&sim, w, t0);
    assert_eq!(fired.len(), 30, "the clip");
    close(fired[29] - t0, 2.32, tick, "the 30th round");

    let mut sim = new_sim();
    let (shooter, target) = range_lane(&mut sim, "test:mp5a4_exact", 400.0);
    for (d, want) in [(400.0, 31.9), (1000.0, 20.3), (2200.0, 10.15)] {
        place_at(&mut sim, shooter, target, "Torso", d);
        close(shoot_at(&mut sim, shooter, target, "Torso"), want, 1e-2, &format!("at {d}"));
    }
}

/// 31. M60 standing: 14, 19, 25, 31, 37, 37; crouched and still: 14, 19,
/// 25, 28, 28; kicks 3.000, 2.750, 2.521, 2.311 degrees.
#[test]
fn t31_lmg_crouch() {
    let mut sim = new_sim();
    let (p, w) = holding(&mut sim, "test:m60_steady");
    assert_eq!(cones_held(&mut sim, p, w, 6), [14, 19, 25, 31, 37, 37]);
    let mut sim = new_sim();
    let (p, w) = holding(&mut sim, "test:m60_steady");
    sim.intent(p).crouch = true;
    sim.seconds(1.0);
    assert_eq!(cones_held(&mut sim, p, w, 5), [14, 19, 25, 28, 28]);
    let (_, w) = holding(&mut sim, M60);
    for (got, want) in kicks(&sim, w, 4).iter().zip([3.0, 2.75, 2.5208, 2.3108]) {
        close(*got, want, 1e-3, "kick");
    }
}

/// 32. Minigun: held from idle, the first round at 1300 ms, then one every
/// 34 ms or so (5-6 a 170 ms loop); released at 650 ms, no round, the
/// spin-down from 666 ms of it, over at 1317 ms.
#[test]
fn t32_minigun_spin_up() {
    let mut sim = sim_at(0.01);
    let (p, w) = holding(&mut sim, "test:minigun_special");
    sim.intent(p).fire = true;
    // The press's update enters the firing state; the spin-up starts on
    // the next (t = 0), the round on the update after the one that sees
    // it done (Corrections 3).
    sim.ticks(2);
    let t0 = now(&sim);
    sim.seconds(1.5);
    sim.intent(p).fire = false;
    let fired: Vec<f64> = rounds(&sim, w, t0).iter().map(|t| t - t0).collect();
    close(fired[0], 1.31, 1e-6, "the first round");
    let in_loop = fired.iter().filter(|t| **t >= 1.31 - 1e-9 && **t < 1.48 - 1e-9).count();
    assert!((5..=6).contains(&in_loop), "{in_loop} in the first loop: {fired:?}");
    sim.seconds(3.0);

    sim.intent(p).fire = true;
    sim.ticks(2);
    let t0 = now(&sim);
    while now(&sim) - t0 < 0.64 - 1e-9 {
        sim.ticks(1);
    }
    sim.intent(p).fire = false;
    sim.ticks(1);
    let idle = sim.app.world().get::<CaSpin>(w).unwrap().idle_at().unwrap();
    close(idle - t0, 1.317, 1e-6, "spin-down over");
    sim.seconds(1.0);
    assert!(rounds(&sim, w, t0).is_empty(), "no round");
    assert_eq!(sim.app.world().get::<CaSpin>(w).unwrap().state, SpinState::Idle);
}

/// 33. Double barrel: n = 64 at rest (not 65); alt-fire with 2 rounds: two
/// 8-pellet shots 67 ms apart, the second at n = 82; with 1, one shot.
#[test]
fn t33_double_barrel() {
    let mut sim = sim_at(0.001);
    let (p, w) = holding(&mut sim, "test:double_barrel");
    assert_eq!(shown_cone(&sim, w), 64);
    sim.app.world_mut().resource_mut::<Log>().0.clear();
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    let t0 = now(&sim);
    let mut cones = vec![shown_cone(&sim, w)];
    while mag(&sim, w).clip > 0 {
        sim.ticks(1);
    }
    cones.push(shown_cone(&sim, w));
    let fired = rounds(&sim, w, t0);
    assert_eq!(fired.len(), 2);
    close(fired[1] - fired[0], 0.067, 1e-6, "67 ms apart");
    assert_eq!(cones, [64, 82]);
    let shots = sim
        .app
        .world()
        .resource::<Log>()
        .0
        .iter()
        .filter(|(_, e, k)| *e == w && matches!(k, WeaponEventKind::Shot { .. }))
        .count();
    assert_eq!(shots, 16, "8 pellets each");
    // One round left: one shot.
    sim.seconds(5.0);
    set_mag(&mut sim, w, 1, 12);
    sim.seconds(3.0);
    let t1 = now(&sim);
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    sim.seconds(0.5);
    assert_eq!(rounds(&sim, w, t1).len(), 1);
}

/// 34. Sniper kick: from rest 5.000 degrees up and 1.000 to the side; a
/// second shot with no recovery, 2.500 up.
#[test]
fn t34_sniper_kick() {
    let mut sim = new_sim();
    let (_, w) = holding(&mut sim, L96A1);
    let r = CaRecoil::new(sim.app.world().get::<CaRecoil>(w).unwrap().params);
    let mut k = CaKick::default();
    close(r.kick(&mut k, false, 0.0).to_degrees() as f64, 5.0, 1e-3, "first kick");
    close(r.wag(&mut k, 0.3).abs().to_degrees() as f64, 1.0, 1e-3, "side");
    close(r.kick(&mut k, false, 0.0).to_degrees() as f64, 2.5, 1e-3, "second kick");
}

/// 35. Damage: L96A1 torso at 3000 / 4200 / 4800 units 160 / 112 / 56, head
/// at 3000 960; Dragunov torso at 3000 100.
#[test]
fn t35_sniper_damage() {
    let mut sim = new_sim();
    let (shooter, target) = range_lane(&mut sim, "test:l96a1_exact", 3000.0);
    place_at(&mut sim, shooter, target, "Torso", 3000.0);
    close(shoot_at(&mut sim, shooter, target, "Torso"), 160.0, 1e-3, "torso at 3000");
    close(shoot_at(&mut sim, shooter, target, "Face_Eyelid1"), 960.0, 1e-3, "head at 3000");
    // Farther than the greybox's lane: the weapon's own damage part.
    let w = sim.app.world().get::<Inventory>(shooter).unwrap().active.unwrap();
    let dmg = sim.app.world().get::<ca::CaDamage>(w).unwrap().clone();
    for (d, want) in [(4200.0, 112.0), (4800.0, 56.0)] {
        close((ca::base_damage(&dmg, 0.5, false, d * U) / HP) as f64, want, 1e-3, &format!("at {d}"));
    }
    arm(&mut sim, shooter, "test:svd_exact");
    close(shoot_at(&mut sim, shooter, target, "Torso"), 100.0, 1e-3, "Dragunov at 3000");
}

/// 36. No scope, no zoom: the M4A1's zoom key changes nothing.
#[test]
fn t36_no_zoom_without_a_scope() {
    let mut sim = new_sim();
    let (p, w) = holding(&mut sim, ca::RIFLE);
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(1.0);
    let limit = sim.app.world().get::<CaMovement>(p).unwrap().speed_limit;
    let cone = shown_cone(&sim, w);
    press_zoom(&mut sim, p);
    sim.ticks(5);
    let z = cz(&sim, w);
    assert_eq!((z.level, z.fov), (0, 90.0));
    assert!(view(&sim, p).is_none());
    assert_eq!(sim.app.world().get::<CaMovement>(p).unwrap().speed_limit, limit);
    assert_eq!(shown_cone(&sim, w), cone, "the normal cone");
}

/// A party's spawn protection (Chimera's `core::SpawnProtected`;
/// game_modes.md Corrections 2026-10-28, 2 and case 105): a Combat Arms
/// gun's round on a protected player shows nothing (its traces carry no
/// hit: no blood, impact, ding or name-tag flash) and deals nothing; once
/// it is over the same round hits.
#[test]
fn a_spawn_protected_player_shows_and_takes_no_hit() {
    use chimera_core::core::SpawnProtected;
    let mut sim = new_sim();
    let (shooter, target) = range_lane(&mut sim, "test:r870_exact", 500.0);
    place_at(&mut sim, shooter, target, "Torso", 500.0);
    let t = now(&sim);
    sim.app.world_mut().entity_mut(target).insert((
        SpawnProtected {
            damage_until: t + 4.0,
            bullets_until: t + 3.0,
        },
        chimera_core::core::God,
    ));
    let shown = |sim: &Sim| {
        sim.app.world().resource::<Log>().0.iter().filter(|(_, _, k)| match k {
            WeaponEventKind::Shot { hit, .. } | WeaponEventKind::ShotContinued { hit, .. } => *hit == Some(target),
            WeaponEventKind::Hit { target: h, .. } => *h == target,
            _ => false,
        }).count()
    };
    sim.app.world_mut().resource_mut::<Log>().0.clear();
    let lost = shoot_at(&mut sim, shooter, target, "Torso");
    assert_eq!((lost, shown(&sim)), (0.0, 0), "protected: nothing shows, nothing lands");
    sim.app.world_mut().entity_mut(target).remove::<(SpawnProtected, chimera_core::core::God)>();
    sim.app.world_mut().resource_mut::<Log>().0.clear();
    let lost = shoot_at(&mut sim, shooter, target, "Torso");
    assert!(lost > 0.0 && shown(&sim) > 0, "after it: {lost}");
}
