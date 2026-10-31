//! The "Corrections (2026-10-19)" test cases of
//! specs/combat_arms/weapon_firing.md (50-69) and weapon_classes.md
//! (37-41), numbered as there, on Combat Arms' simulation (`CaSim`) and
//! the greybox; the weapons are the game's data and test variants of it
//! (`base = ...` with a few keys changed: no roll for "roll 1.0", no
//! spread or kick where a case aims at one body part).
//!
//! Units: world units are 1.8 cm, damage is read as hit points (health
//! x 100). Tolerances: 1e-3 HP for damage, 1e-7 rad for the kick.

use std::f32::consts::PI;

use avian3d::prelude::*;
use bevy::prelude::*;
use chimera_core::core::{Bot, Health, Intent, MovementState, PropSurface, SimClock};
use chimera_sim::weapon::{
    self, Inventory, Magazine, ViewPunch, WeaponEvent, WeaponEventKind, WeaponState, data::WeaponDataAppExt,
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{
    game::{
        TICK_INTERVAL, movement,
        weapons::{
            self as ca, CaKick, CaRecoil, CaSpread, ConeParams, DEFAULT_BODY_GEAR, FireMode, HitSphere,
            MALE, RecoilParams, SpreadParams, SpreadScales,
        },
    },
    harness::CaSim,
};

const U: f32 = 0.018;
const HP: f32 = 0.01;

/// The M4A1 with no roll, spread or kick, a longer reach.
const EXACT: &str = r#"
id = "test:m4a1_exact"
base = "combat_arms:m4a1"
[hitscan]
range = 4000.0
[hooks."combat_arms:damage"]
roll = 0.0
[hooks."combat_arms:spread"]
cone = { min = 0, max = 0, inc = 1.3, dec = 0.3 }
[hooks."combat_arms:recoil"]
pitch = 0.0
"#;

/// The M4A1 without the recoil's return (its kick only).
const STIFF: &str = r#"
id = "test:m4a1_stiff"
base = "combat_arms:m4a1"
[hooks."combat_arms:recoil"]
jitter_wait_recover = 0.0
jitter_recover = 0.0
"#;
const STIFF_M92FS: &str = r#"
id = "test:m92fs_stiff"
base = "combat_arms:beretta_m92fs"
[hooks."combat_arms:recoil"]
jitter_wait_recover = 0.0
jitter_recover = 0.0
"#;
const STIFF_MP5: &str = r#"
id = "test:mp5a4_stiff"
base = "combat_arms:mp5a4"
[hooks."combat_arms:recoil"]
jitter_wait_recover = 0.0
jitter_recover = 0.0
"#;

/// The Remington 870 with no spread, roll or kick: its 8 pellets on one
/// line; `fire_delay` as the data (absent: 30 ms).
const R870: &str = r#"
id = "test:r870_exact"
base = "combat_arms:remington_870"
[hooks."combat_arms:damage"]
roll = 0.0
[hooks."combat_arms:spread"]
cone = { min = 0, max = 0, inc = 5.0, dec = 1.0 }
[hooks."combat_arms:recoil"]
pitch = 0.0
"#;
const R870_NO_DELAY: &str = r#"
id = "test:r870_no_delay"
base = "test:r870_exact"
[hooks."combat_arms:damage"]
fire_delay = 0.0
"#;

/// The M4A1 with no roll, spread or kick, fireDelay 30 (case 53's single
/// ray).
const SPACED: &str = r#"
id = "test:m4a1_spaced"
base = "test:m4a1_exact"
[hooks."combat_arms:damage"]
fire_delay = 30.0
"#;

/// The frame-quantised M4A1 (`set` timing, case 64).
const FRAMES: &str = r#"
id = "test:m4a1_frames"
base = "combat_arms:m4a1"
[trigger]
timing = "set"
"#;

/// The minigun of weapon_classes.md (3.3): its spin-up is the pre-fire.
const MINIGUN: &str = r#"
id = "test:minigun_special"
base = "combat_arms:m60"
[ammo."test:minigun"]
max = 0
[trigger]
cycle = 0.034
latency = 1
[magazine]
clip = 500
ammo = "test:minigun"
[hooks."combat_arms:spin"]
prefire = 1.3
postfire = 1.333
loop_len = 0.17
reversible = true
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
            ("exact", EXACT),
            ("stiff", STIFF),
            ("stiff_m92fs", STIFF_M92FS),
            ("stiff_mp5", STIFF_MP5),
            ("r870", R870),
            ("r870_no_delay", R870_NO_DELAY),
            ("spaced", SPACED),
            ("frames", FRAMES),
            ("minigun", MINIGUN),
        ])
        .unwrap();
    steady_surfaces(&mut sim);
    sim
}

/// No exit re-perturb on the shoot-through surfaces (its whole-thousandth
/// draws are case 63's): a round through a panel stays on its line, as the
/// cases aim it.
pub fn steady_surfaces(sim: &mut Sim) {
    for (_, s) in sim.app.world_mut().resource_mut::<ca::CaSurfaces>().0.iter_mut() {
        s.perturb = 0.0;
    }
}

fn sim() -> Sim {
    sim_at(TICK_INTERVAL)
}

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

/// Give `p` the weapon `id` and wait for its draw.
fn arm(sim: &mut Sim, p: Entity, id: &str) -> Entity {
    let w = weapon::give(sim.app.world_mut(), p, id).expect("unknown weapon");
    sim.seconds(1.5);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(w), "{id} drawn");
    w
}

fn health(sim: &Sim, p: Entity) -> f32 {
    sim.app.world().get::<Health>(p).unwrap().current
}

fn mag(sim: &Sim, w: Entity) -> Magazine {
    sim.app.world().get::<Magazine>(w).unwrap().clone()
}

fn eye(sim: &Sim, p: Entity) -> Vec3 {
    sim.position(p) + sim.state(p).eye_offset
}

fn sphere(sim: &Sim, shooter: Entity, p: Entity, name: &str) -> Vec3 {
    super::aim_points::point(sim.app.world(), shooter, p, name)
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

fn aim(sim: &mut Sim, shooter: Entity, point: Vec3) {
    let d = point - eye(sim, shooter);
    let mut i = sim.intent(shooter);
    i.yaw = (-d.x).atan2(-d.z);
    i.pitch = d.y.atan2(d.xz().length());
}

/// Turn `p` to `yaw` with its body turned too (no twist).
fn face(sim: &mut Sim, p: Entity, yaw: f32) {
    sim.intent(p).yaw = yaw;
    set_body(sim, p, yaw);
}

/// `p`'s body yaw (its weapon's twist tracker).
fn set_body(sim: &mut Sim, p: Entity, yaw: f32) {
    let w = sim.app.world().get::<Inventory>(p).and_then(|i| i.active).unwrap();
    let mut s = sim.app.world_mut().get_mut::<CaSpread>(w).unwrap();
    s.body_yaw = Some(yaw);
    s.prev_view = None;
}

/// A one-tick pull and the chain's round landing; the HP `targets` lost.
fn pull(sim: &mut Sim, shooter: Entity, targets: &[Entity]) -> Vec<f32> {
    let before: Vec<f32> = targets.iter().map(|t| health(sim, *t)).collect();
    sim.intent(shooter).fire = true;
    sim.ticks(1);
    sim.intent(shooter).fire = false;
    sim.ticks(4);
    let lost = targets.iter().zip(before).map(|(t, b)| (b - health(sim, *t)) / HP).collect();
    sim.seconds(0.7);
    lost
}

/// Move `target` along -Z until the ray to its `name` sphere enters its
/// plain box `units` from `shooter`'s eye.
fn place_at(sim: &mut Sim, shooter: Entity, target: Entity, name: &str, units: f32) {
    for _ in 0..6 {
        let from = eye(sim, shooter);
        let dir = (sphere(sim, shooter, target, name) - from).normalize();
        let (min, max, _) = geometry(sim.app.world(), target);
        let t = ca::box_entry(min, max, from, dir, f32::INFINITY).unwrap();
        let err = t - units * U;
        sim.app.world_mut().get_mut::<Transform>(target).unwrap().translation.z += err / dir.z.abs();
        sim.ticks(1);
    }
}

/// A shooter looking down -Z and a target facing it `units` apart.
fn range_lane(sim: &mut Sim, id: &str, units: f32) -> (Entity, Entity) {
    let shooter = player(sim, Vec3::new(30.0, 1.0, 37.0), 0.0, 1.0);
    let target = player(sim, Vec3::new(30.0, 1.0, 37.0 - units * U), PI, 100.0);
    arm(sim, shooter, id);
    (shooter, target)
}

/// A panel of `surface`, `units` thick, across the lane at z.
fn panel(sim: &mut Sim, z: f32, units: f32, surface: &str) -> Entity {
    let e = sim
        .app
        .world_mut()
        .spawn((
            Transform::from_xyz(30.0, 1.5, z),
            RigidBody::Static,
            Collider::cuboid(3.0, 3.0, units * U),
            PropSurface(surface.into()),
        ))
        .id();
    sim.ticks(1);
    e
}

fn close(a: f32, b: f32, tol: f32, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a}, expected {b}");
}

/// The update index (from 0 = the press's) of each round `w` fired.
fn round_updates(sim: &Sim, w: Entity, press: f64, dt: f64) -> Vec<i64> {
    sim.app
        .world()
        .resource::<Log>()
        .0
        .iter()
        .filter(|(t, e, k)| *e == w && *t >= press - 1e-9 && matches!(k, WeaponEventKind::Fired { .. }))
        .map(|(t, ..)| ((t - press) / dt).round() as i64)
        .collect()
}

fn now(sim: &Sim) -> f64 {
    sim.app.world().resource::<SimClock>().now
}

/// The time of the next tick.
fn next_time(sim: &Sim) -> f64 {
    now(sim) + sim.tick_hz().recip()
}

// ---------------------------------------------------------------------------
// weapon_firing.md

/// 50. The kick carries over a switch: the M4A1 sprayed to its cap (J =
/// rad 5 degrees), then the M92FS's first round lifts the view 2.250
/// degrees, not 3.500, and J goes to 0.1069015.
#[test]
fn t50_kick_carries_over_a_switch() {
    let mut sim = sim();
    let p = player(&mut sim, Vec3::new(30.0, 1.0, 30.0), 0.0, 1.0);
    let rifle = arm(&mut sim, p, "test:m4a1_stiff");
    sim.intent(p).fire = true;
    while 30 - mag(&sim, rifle).clip < 9 {
        sim.ticks(1);
    }
    sim.intent(p).fire = false;
    let k = sim.app.world().get::<CaKick>(p).unwrap().clone();
    close(k.pitch, 5f32.to_radians(), 1e-6, "J at the M4A1's cap");
    let pistol = arm(&mut sim, p, "test:m92fs_stiff");
    let before = sim.app.world().get::<ViewPunch>(p).unwrap().0.x;
    sim.intent(p).fire = true;
    let clip = mag(&sim, pistol).clip;
    while mag(&sim, pistol).clip == clip {
        sim.ticks(1);
    }
    sim.intent(p).fire = false;
    let up = sim.app.world().get::<ViewPunch>(p).unwrap().0.x - before;
    close(up.to_degrees(), 2.250, 1e-3, "the pistol's first kick");
    close(sim.app.world().get::<CaKick>(p).unwrap().pitch, 0.1069015, 1e-6, "J");
}

/// 51. One wag direction per player: the MP5A4's first side kick after
/// the M4A1's goes the way the M4A1 left the flag, scaled by the J_y the
/// M4A1 left.
#[test]
fn t51_one_wag_direction_per_player() {
    let mut sim = sim();
    let p = player(&mut sim, Vec3::new(30.0, 1.0, 30.0), 0.0, 1.0);
    let rifle = arm(&mut sim, p, "test:m4a1_stiff");
    sim.intent(p).fire = true;
    while 30 - mag(&sim, rifle).clip < 2 {
        sim.ticks(1);
    }
    sim.intent(p).fire = false;
    let left = sim.app.world().get::<CaKick>(p).unwrap().clone();
    assert!(left.right.is_some() && left.yaw != 0.0, "the M4A1 wagged");
    let smg = arm(&mut sim, p, "test:mp5a4_stiff");
    let r = sim.app.world().get::<CaRecoil>(smg).unwrap().params;
    sim.intent(p).fire = true;
    // The first round with a side kick: its direction and size.
    let mut kick = left.clone();
    for _ in 0..200 {
        let yaw = sim.app.world().get::<ViewPunch>(p).unwrap().0.y;
        sim.ticks(1);
        let side = sim.app.world().get::<ViewPunch>(p).unwrap().0.y - yaw;
        if side != 0.0 {
            let right = kick.right.unwrap();
            let q = 1.0 - kick.yaw.abs() / r.max_wag.to_radians();
            let want = q * if right { r.wag_r } else { r.wag_l }.to_radians();
            close(side, want, 1e-6, "the MP5A4's first side kick");
            return;
        }
        kick = sim.app.world().get::<CaKick>(p).unwrap().clone();
    }
    panic!("no side kick");
}

/// 52. The return in burst mode uses the burst cap: 0.0020800 rad down
/// (0.0023680 with the normal cap); the same with no burst kick.
#[test]
fn t52_burst_return_cap() {
    let params = RecoilParams {
        max_pitch: 5.0,
        burst_max_pitch: 8.0,
        jitter_wait_recover: 0.1,
        jitter_dec_delay: 600.0,
        ..default()
    };
    for (burst, burst_pitch, down, left) in [
        (true, 1.0, 0.0020800, 0.0408479),
        (false, 1.0, 0.0023680, 0.0407039),
        (true, 0.0, 0.0020800, 0.0408479),
    ] {
        let r = CaRecoil::new(RecoilParams { burst_pitch, ..params });
        let mut k = CaKick {
            pitch: 0.0418879,
            last_kick: Some(9.99),
            ..default()
        };
        let (d, _) = r.recover(&mut k, 0.016, 10.0, burst);
        close(d, down, 1e-7, &format!("burst {burst}, burst kick {burst_pitch}"));
        close(k.pitch, left, 1e-7, "J");
    }
}

/// 53. The last pellet dropped: 8 rays of 9 at 500 units, fireDelay 30 →
/// 63 (one round spent); fireDelay 0 → 72. A single ray with fireDelay 30
/// at 105 ms: every round lands.
#[test]
fn t53_last_pellet_dropped() {
    for (id, want) in [("test:r870_exact", 63.0), ("test:r870_no_delay", 72.0)] {
        let mut sim = sim();
        let (shooter, target) = range_lane(&mut sim, id, 500.0);
        place_at(&mut sim, shooter, target, "Upper_torso", 500.0);
        let at = sphere(&sim, shooter, target, "Upper_torso");
        aim(&mut sim, shooter, at);
        let w = sim.app.world().get::<Inventory>(shooter).unwrap().active.unwrap();
        let total = |sim: &Sim| mag(sim, w).clip + mag(sim, w).reserve;
        let before = total(&sim);
        let lost = pull(&mut sim, shooter, &[target]);
        close(lost[0], want, 1e-2, id);
        assert_eq!(before - total(&sim), 1, "{id}: one round spent");
    }
    let mut sim = sim();
    let (shooter, target) = range_lane(&mut sim, "test:m4a1_spaced", 500.0);
    sim.app.world_mut().get_mut::<Health>(target).unwrap().current = 10.0;
    let at = sphere(&sim, shooter, target, "Upper_torso");
    aim(&mut sim, shooter, at);
    let w = sim.app.world().get::<Inventory>(shooter).unwrap().active.unwrap();
    let before = health(&sim, target);
    sim.intent(shooter).fire = true;
    while 30 - mag(&sim, w).clip < 5 {
        sim.ticks(1);
    }
    sim.intent(shooter).fire = false;
    sim.ticks(1);
    close((before - health(&sim, target)) / HP, 5.0 * 35.0, 1e-2, "every round lands");
}

/// 54. Rays aren't rate-limited between shots: 8 rays, fireDelay 100,
/// shots at 0 and 40 ms → rays 1-7 of each land, ray 8 of each dropped.
#[test]
fn t54_rays_not_rate_limited_between_shots() {
    let mut last = None;
    let first = ca::spacing(8, 0.0, 0.1, &mut last);
    let second = ca::spacing(8, 0.04, 0.1, &mut last);
    let want: Vec<bool> = (0..8).map(|i| i < 7).collect();
    assert_eq!(first, want, "shot 1");
    assert_eq!(second, want, "shot 2");
    // One ray: the old rule (case 47).
    let mut last = None;
    let rays: Vec<bool> = [0.0, 0.06, 0.11].iter().map(|t| ca::spacing(1, *t, 0.1, &mut last)[0]).collect();
    assert_eq!(rays, [true, false, true]);
}

/// 55. Four intersections: three 6-unit wood panels then a torso 1000
/// units away → 35 x 0.5^3 = 4.375; four panels → nothing.
#[test]
fn t55_four_intersections() {
    for (panels, want) in [(3, 4.375), (4, 0.0)] {
        let mut sim = sim();
        let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", 1000.0);
        place_at(&mut sim, shooter, target, "Upper_torso", 1000.0);
        let at = sphere(&sim, shooter, target, "Upper_torso");
        aim(&mut sim, shooter, at);
        for i in 0..panels {
            panel(&mut sim, 34.0 - i as f32, 6.0, "Wood");
        }
        let lost = pull(&mut sim, shooter, &[target]);
        close(lost[0], want, 1e-3, &format!("{panels} panels"));
    }
}

/// 56. No third victim: A, B, C in line at 2000, 2100, 2200 units, the
/// client hit on A → A 35, B 35, C 0.
#[test]
fn t56_no_third_victim() {
    let mut sim = sim();
    let (shooter, a) = range_lane(&mut sim, "test:m4a1_exact", 2000.0);
    place_at(&mut sim, shooter, a, "Upper_torso", 2000.0);
    let at_a = sim.position(a);
    let b = player(&mut sim, at_a - Vec3::Z * 100.0 * U, PI, 100.0);
    let c = player(&mut sim, at_a - Vec3::Z * 200.0 * U, PI, 100.0);
    let at = sphere(&sim, shooter, a, "Upper_torso");
    aim(&mut sim, shooter, at);
    let lost = pull(&mut sim, shooter, &[a, b, c]);
    close(lost[0], 35.0, 1e-3, "A");
    close(lost[1], 35.0, 1e-3, "B");
    close(lost[2], 0.0, 1e-3, "C");
}

/// 57. The server's sphere by priority: of the spheres within their
/// radius of the line, the highest priority; a tie, the later; none, no
/// hit. In play, through wood: a line in the plain box but within no
/// sphere deals nothing; one through the chest, 17.5.
#[test]
fn t57_server_sphere_by_priority() {
    let head = HitSphere {
        priority: 1.0,
        ..*ca::male("Head")
    };
    let chest = HitSphere {
        priority: 2.0,
        ..*ca::male("Upper_torso")
    };
    let spheres = [(Vec3::new(0.0, 10.0, 0.0), 4.0), (Vec3::new(0.5, 0.0, 0.0), 10.0)];
    // The line down through both, the head first.
    let down = Vec3::NEG_Y;
    let at = Vec3::new(0.0, 20.0, 0.0);
    assert_eq!(ca::priority_sphere(&spheres, &[head, chest], at, down), Some(1), "the chest by priority");
    let even = HitSphere { priority: 1.0, ..chest };
    assert_eq!(ca::priority_sphere(&spheres, &[head, even], at, down), Some(1), "a tie: the later");
    let rev = [spheres[1], spheres[0]];
    assert_eq!(ca::priority_sphere(&rev, &[even, head], at, down), Some(1), "a tie: the later (the head)");
    // Anywhere along the infinite line, not just ahead.
    assert_eq!(ca::priority_sphere(&spheres, &[head, chest], Vec3::new(0.0, -50.0, 0.0), down), Some(1));
    assert_eq!(ca::priority_sphere(&spheres, &[head, chest], Vec3::new(20.0, 20.0, 0.0), down), None, "none");

    // In play: through a wood panel, a line into the plain box near its
    // top corner, within no sphere's radius: no hit (the walk goes on 5
    // units past the entry and meets nothing else); at the chest, the
    // chest (17.5).
    for (corner, want) in [(true, 0.0), (false, 17.5)] {
        let mut sim = sim();
        let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", 1000.0);
        place_at(&mut sim, shooter, target, "Upper_torso", 1000.0);
        let (min, max, world) = geometry(sim.app.world(), target);
        let at = if corner {
            let p = Vec3::new(max.x - U, max.y - U, max.z);
            let from = eye(&sim, shooter);
            let dir = (p - from).normalize();
            assert!(ca::priority_sphere(&world, *MALE, p, dir).is_none(), "the corner is within no sphere");
            assert!(ca::box_entry(min, max, from, dir, f32::INFINITY).is_some(), "but in the box");
            p
        } else {
            sphere(&sim, shooter, target, "Upper_torso")
        };
        aim(&mut sim, shooter, at);
        panel(&mut sim, 32.0, 6.0, "Wood");
        let lost = pull(&mut sim, shooter, &[target]);
        close(lost[0], want, 1e-3, if corner { "the corner" } else { "the chest" });
    }
}

/// 58. Behind from the body: the victim's legs 44.7 degrees off its view;
/// a bullet behind by the view but not by the body: 35, not 45.5.
#[test]
fn t58_behind_from_the_body() {
    // The spec's numbers: body sector 0, view sector 11.
    let (vf, vr) = (Vec3::Z, Vec3::X);
    let (bf, br) = (Vec3::new(0.7071, 0.0, 0.7071), Vec3::new(0.7071, 0.0, -0.7071));
    let d = Vec3::new(-0.8660, 0.0, 0.5);
    assert_eq!(ca::sector(bf, br, d), 0);
    // 11 in the spec (150 degrees exactly: 10 or 11 in floats), behind.
    assert!(ca::is_behind(vf, vr, d));
    // In play: find a view yaw that is behind by the view, not by the
    // body 0.78 rad from it, for a round travelling along -Z.
    let dir = Vec3::NEG_Z;
    let twist = 0.78;
    let behind = |yaw: f32| {
        let q = Quat::from_rotation_y(yaw);
        ca::is_behind(q * Vec3::NEG_Z, q * Vec3::X, dir)
    };
    // The middle of the views that are behind by the view, not by the
    // body (clear of the sectors' edges).
    let views: Vec<f32> = (0..360)
        .map(|d| (d as f32).to_radians())
        .filter(|v| behind(*v) && !behind(v - twist))
        .collect();
    let view = views[views.len() / 2];
    let mut sim = sim();
    let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", 1000.0);
    sim.intent(target).yaw = view;
    set_body(&mut sim, target, view - twist);
    place_at(&mut sim, shooter, target, "Upper_torso", 1000.0);
    let at = sphere(&sim, shooter, target, "Upper_torso");
    aim(&mut sim, shooter, at);
    let lost = pull(&mut sim, shooter, &[target]);
    close(lost[0], 35.0, 1e-3, "not behind by the body");
    face(&mut sim, target, view);
    sim.ticks(1);
    let at = sphere(&sim, shooter, target, "Upper_torso");
    aim(&mut sim, shooter, at);
    let lost = pull(&mut sim, shooter, &[target]);
    close(lost[0], 45.5, 1e-3, "behind with the body turned too");
}

/// 59. AI damage: a bot's M4A1 at 100 units: torso 35 x 0.8 = 28, head
/// 168 → 50; armoured torso 23.8 (AP 95.8); 1500 after armour → 990.
#[test]
fn t59_ai_damage() {
    let s = ca::CaAiDamage::default();
    assert_eq!((s.factor, s.capped(1500.0 * s.factor, false), s.capped(216.0, true)), (0.8, 990.0, 216.0));
    for (part, armoured, want, ap) in [
        ("Upper_torso", false, 28.0, 0.0),
        ("Head", false, 50.0, 0.0),
        ("Upper_torso", true, 23.8, 95.8),
    ] {
        let mut sim = sim();
        let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", 100.0);
        sim.app.world_mut().entity_mut(shooter).insert(Bot);
        if armoured {
            sim.app.world_mut().entity_mut(target).insert(combat_arms::game::weapons::gear_armour(100.0, &vec![DEFAULT_BODY_GEAR]));
        }
        let at = sphere(&sim, shooter, target, part);
        aim(&mut sim, shooter, at);
        let lost = pull(&mut sim, shooter, &[target]);
        close(lost[0], want, 1e-3, part);
        close(sim.app.world().get::<chimera_sim::weapon::Armor>(target).map(combat_arms::game::weapons::points).unwrap(), ap, 1e-3, "AP");
    }
    // A bot shooting a bot: the factor, no cap.
    let mut sim = sim();
    let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", 100.0);
    sim.app.world_mut().entity_mut(shooter).insert(Bot);
    sim.app.world_mut().entity_mut(target).insert(Bot);
    let at = sphere(&sim, shooter, target, "Head");
    aim(&mut sim, shooter, at);
    let lost = pull(&mut sim, shooter, &[target]);
    close(lost[0], 168.0, 1e-2, "an AI victim isn't capped");
}

/// 60. The band at the widened box's entry: the plain box's front at
/// 2510 units, the widened one's 15 nearer → band 1.0: 35 (the plain
/// entry would give 24.5).
#[test]
fn t60_widened_entry_for_the_band() {
    let mut sim = sim();
    let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", 2510.0);
    place_at(&mut sim, shooter, target, "Upper_torso", 2510.0);
    let (min, max, _) = geometry(sim.app.world(), target);
    assert!((max - min).x / 2.0 / U > 10.0, "half-width over 10 units");
    let at = sphere(&sim, shooter, target, "Upper_torso");
    aim(&mut sim, shooter, at);
    let lost = pull(&mut sim, shooter, &[target]);
    close(lost[0], 35.0, 1e-3, "band at the widened entry");
}

/// 61. The near-miss drop: through the widened box and an arm sphere but
/// past the plain box, a wall next whose plane has the victim's origin
/// behind it → no player hit; with the origin in front of it → the arm.
#[test]
fn t61_near_miss_drop() {
    for (wall_dz, want) in [(0.1, 0.0), (-0.1, 31.5)] {
        let mut sim = sim();
        let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", 500.0);
        // An arm sphere outside the plain box.
        let (min, max, world) = geometry(sim.app.world(), target);
        let from = eye(&sim, shooter);
        let (i, at) = world
            .iter()
            .enumerate()
            .filter(|(i, (c, r))| *r > 0.0 && MALE[*i].location == ca::Location::Arm && (c.x > max.x || c.x < min.x))
            .map(|(i, (c, _))| (i, *c))
            .next()
            .expect("an arm outside the box");
        let dir = (at - from).normalize();
        assert!(ca::box_entry(min, max, from, dir, f32::INFINITY).is_none(), "misses the plain box ({})", MALE[i].name);
        aim(&mut sim, shooter, at);
        // A wall across the lane near the victim's origin.
        let z = sim.position(target).z + wall_dz;
        panel(&mut sim, z, 2.0, "Concrete");

        let lost = pull(&mut sim, shooter, &[target]);
        close(lost[0], want, 1e-3, &format!("wall {wall_dz} m from the origin"));
    }
}

/// 62. A board passes up to its limit + 1: wood (10) 10.5 units → 17.5;
/// 11.5 → stopped.
#[test]
fn t62_board_at_limit_plus_one() {
    for (thick, want) in [(10.5, 17.5), (11.5, 0.0)] {
        let mut sim = sim();
        let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", 1000.0);
        place_at(&mut sim, shooter, target, "Upper_torso", 1000.0);
        let at = sphere(&sim, shooter, target, "Upper_torso");
        aim(&mut sim, shooter, at);
        panel(&mut sim, 32.0, thick, "Wood");
        let lost = pull(&mut sim, shooter, &[target]);
        close(lost[0], want, 1e-3, &format!("{thick} units"));
    }
}

/// 63. The integer re-perturb: along (0,0,1), k_r 5 and k_u -5 →
/// (0.0049999, -0.0049999, 0.9999750); draws are whole numbers in -P..P.
#[test]
fn t63_integer_reperturb() {
    let d = ca::reperturb(Vec3::Z, 5, -5);
    close(d.x, 0.0049999, 1e-6, "x");
    close(d.y, -0.0049999, 1e-6, "y");
    close(d.z, 0.9999750, 1e-6, "z");
    let draws: std::collections::BTreeSet<i32> = (0..1000).map(|i| ca::whole_draw(i as f32 / 1000.0, 5)).collect();
    assert_eq!(draws.into_iter().collect::<Vec<_>>(), (-5..=5).collect::<Vec<_>>(), "11 offsets");
}

/// 64 (and weapon_classes.md 40, as corrected by its case 43). First-round
/// latency at 60 fps: the M4A1 (frame-quantised) on updates 3, 10, 17
/// after the press's; the Remington 870 on update 2; the M92FS on 2 (no
/// pre-fire for pistols).
#[test]
fn t64_first_round_latency() {
    for (id, want) in [
        ("test:m4a1_frames", vec![3, 10, 17]),
        ("combat_arms:remington_870", vec![2]),
        ("combat_arms:beretta_m92fs", vec![2]),
    ] {
        let mut sim = sim_at(1.0 / 60.0);
        let p = player(&mut sim, Vec3::new(30.0, 1.0, 30.0), 0.0, 1.0);
        let w = arm(&mut sim, p, id);
        let press = next_time(&sim);
        sim.intent(p).fire = true;
        sim.seconds(0.3);
        let mut got = round_updates(&sim, w, press, 1.0 / 60.0);
        got.truncate(want.len());
        assert_eq!(got, want, "{id}");
    }
}

/// 65. After a draw at 60 fps: the select animation seen done on the
/// update at 1200 ms → the first round on the update at 1250 ms.
#[test]
fn t65_after_a_draw() {
    let mut sim = sim_at(1.0 / 60.0);
    let p = player(&mut sim, Vec3::new(30.0, 1.0, 30.0), 0.0, 1.0);
    let pistol = arm(&mut sim, p, "combat_arms:beretta_m92fs");
    let _ = pistol;
    let rifle = sim
        .app
        .world()
        .get::<Inventory>(p)
        .unwrap()
        .weapons
        .iter()
        .copied()
        .find(|w| sim.app.world().get::<weapon::Weapon>(*w).unwrap().id == ca::RIFLE)
        .unwrap();
    sim.intent(p).fire = true;
    sim.intent(p).select = Some(0);
    let press = next_time(&sim);
    sim.ticks(1);
    sim.intent(p).select = None;
    sim.seconds(1.5);
    let got = round_updates(&sim, rifle, press, 1.0 / 60.0);
    // 1200 ms is update 72, the round 3 updates later.
    assert_eq!(got.first(), Some(&75), "{got:?}");
}

/// 66. The shot leaves before the frame's return: the M4A1's kick at t =
/// 0 (J 0.0113446, the view 0.0226892 up), returns on frames 16 to 64 ms;
/// a round on the 80 ms frame leaves 0.0155561 rad above the pre-kick
/// aim.
#[test]
fn t66_shot_direction() {
    let mut sim = sim_at(0.016);
    sim.app
        .add_weapon_data(&[(
            "every80",
            r#"
            id = "test:m4a1_every_80"
            base = "combat_arms:m4a1"
            [trigger]
            cycle = 0.08
            [hooks."combat_arms:spread"]
            cone = { min = 0, max = 0, inc = 1.3, dec = 0.3 }
            "#,
        )])
        .unwrap();
    let p = player(&mut sim, Vec3::new(30.0, 1.0, 30.0), 0.0, 1.0);
    let w = arm(&mut sim, p, "test:m4a1_every_80");
    sim.intent(p).fire = true;
    while 30 - mag(&sim, w).clip < 2 {
        sim.ticks(1);
    }
    let pitches: Vec<f32> = sim
        .app
        .world()
        .resource::<Log>()
        .0
        .iter()
        .filter_map(|(_, e, k)| match k {
            WeaponEventKind::Fired { pitch, .. } if *e == w => Some(*pitch),
            _ => None,
        })
        .collect();
    close(pitches[1] - pitches[0], 0.0155561, 2e-6, "the second round's lift");
}

/// 67. The zoomed jump target: zoomed, in a jump, run flag on → the plain
/// 0.7; off → the zoomed 0.3.
#[test]
fn t67_zoomed_jump_target() {
    let p = spread_params(|p| p.zoomed_state.jump = 0.3);
    assert_eq!(ca::jump_target(&p, FireMode::Zoomed, true), 0.7);
    assert_eq!(ca::jump_target(&p, FireMode::Zoomed, false), 0.3);
    assert_eq!(ca::jump_target(&p, FireMode::Normal, false), 0.7);
}

/// The M4A1's spread data, changed by `f`.
fn spread_params(f: impl FnOnce(&mut SpreadParams)) -> SpreadParams {
    let mut sim = sim();
    let p = player(&mut sim, Vec3::new(30.0, 1.0, 30.0), 0.0, 1.0);
    let w = sim.app.world().get::<Inventory>(p).unwrap().active.unwrap();
    let mut s = sim.app.world().get::<CaSpread>(w).unwrap().params.clone();
    f(&mut s);
    s
}

/// 68. The jump clamp is the plain JumpPerturb: zoomed rates (inc 1.3),
/// M 0.5, mu S 0.3 → 0.64625, under 0.7.
#[test]
fn t68_jump_clamp() {
    let params = spread_params(|p| p.zoomed_state.jump = 0.3);
    let mut s = CaSpread::new(params.clone(), U);
    let sc = SpreadScales::default();
    let cone = s.cone_params(FireMode::Zoomed);
    s.movement = 0.5;
    s.move_substep(&sc, cone, 0.3, 0.3, 0.0, true, params.state.jump);
    close(s.movement, 0.64625, 1e-5, "stays");
}

/// 69. Partial burst fields: burst min 0, max 60 on a plain 3/100 → the
/// limits 3/60, n = 3 at rest.
#[test]
fn t69_partial_burst_fields() {
    let params = spread_params(|p| {
        p.burst_cone = Some(ConeParams {
            min: 0,
            max: 60,
            inc: 0.0,
            dec: 0.0,
            zoomed: false,
        })
    });
    let s = CaSpread::new(params.clone(), U);
    let c = s.cone_params(FireMode::Burst);
    assert_eq!((c.min, c.max), (3, 60));
    assert_eq!((c.inc, c.dec), (params.cone.inc, params.cone.dec), "speeds from the plain ones");
    assert_eq!(s.cone(FireMode::Burst), 3.0);
}

// ---------------------------------------------------------------------------
// weapon_classes.md

/// 37. A shotgun with spacing: Remington_870 (`FireDelay` 30) at 500 →
/// 63, at 700 → 7 x 6.3 = 44.1; `FireDelay` 0 → 72.
#[test]
fn wc37_shotgun_with_spacing() {
    for (id, units, want) in [
        ("test:r870_exact", 500.0, 63.0),
        ("test:r870_exact", 700.0, 44.1),
        ("test:r870_no_delay", 500.0, 72.0),
    ] {
        let mut sim = sim();
        let (shooter, target) = range_lane(&mut sim, id, units);
        place_at(&mut sim, shooter, target, "Upper_torso", units);
        let at = sphere(&sim, shooter, target, "Upper_torso");
        aim(&mut sim, shooter, at);
        let lost = pull(&mut sim, shooter, &[target]);
        close(lost[0], want, 1e-2, &format!("{id} at {units}"));
    }
}

/// 38. The carry-over with spacing: only pellet 3 hits → reports 3-8,
/// the 8th dropped: 45; only 1 → 63; only 8 → 0; carry-over off, only 3
/// → 9.
#[test]
fn wc38_carry_over_with_spacing() {
    let hit = |pellet: usize| -> Vec<Option<u32>> { (0..8).map(|i| (i == pellet).then_some(1)).collect() };
    let lands = {
        let mut last = None;
        ca::spacing(8, 0.0, 0.03, &mut last)
    };
    let dealt = |real: Vec<Option<u32>>, on: bool| -> f32 {
        ca::reported_hits(&real, on)
            .iter()
            .zip(&lands)
            .filter(|(r, l)| r.is_some() && **l)
            .count() as f32
            * 9.0
    };
    assert_eq!(dealt(hit(2), true), 45.0);
    assert_eq!(dealt(hit(0), true), 63.0);
    assert_eq!(dealt(hit(7), true), 0.0);
    assert_eq!(dealt(hit(2), false), 9.0);
}

/// 39. The server's reload window: 2400 ms / (1 x 1.2) = 2000 ms; with a
/// rate of 2.4 the server's 2.0: 1000 ms. In play a shot inside it deals
/// nothing; after it, it does.
#[test]
fn wc39_server_reload_mark() {
    close(ca::server_reload_mark(2.4, 1.0, 0.8) as f32, 2.0, 1e-9, "rate 1");
    close(ca::server_reload_mark(2.4, 2.4, 0.8) as f32, 1.0, 1e-9, "rate 2.4");
    close(ca::server_reload_mark(2.4, 1.0, 1.0) as f32, 2.4, 1e-9, "ReloadCheckTime 1");
    // Within a tick of the mark's end (15 ms): 50 ms either side.
    for (after, want) in [(1.95, 0.0), (2.05, 35.0)] {
        let mut sim = sim();
        sim.app
            .add_weapon_data(&[(
                "r3p",
                r#"
                id = "test:m4a1_3p"
                base = "test:m4a1_exact"
                [hooks."combat_arms:damage"]
                reload_third_person = 2.4
                "#,
            )])
            .unwrap();
        let (shooter, target) = range_lane(&mut sim, "test:m4a1_3p", 500.0);
        let at = sphere(&sim, shooter, target, "Upper_torso");
        aim(&mut sim, shooter, at);
        let w = sim.app.world().get::<Inventory>(shooter).unwrap().active.unwrap();
        sim.app.world_mut().get_mut::<Magazine>(w).unwrap().clip = 10;
        sim.intent(shooter).reload = true;
        sim.ticks(1);
        sim.intent(shooter).reload = false;
        let started = now(&sim);
        // A modified client fires anyway: its reload cut short.
        while next_time(&sim) < started + after - 3.0 * sim.tick_hz().recip() {
            sim.ticks(1);
        }
        {
            let world = sim.app.world_mut();
            let mut st = world.get_mut::<WeaponState>(w).unwrap();
            st.reload_end = None;
            st.next_primary = 0.0;
            world.get_mut::<Inventory>(shooter).unwrap().next_attack = 0.0;
        }
        let lost = pull(&mut sim, shooter, &[target]);
        close(lost[0], want, 1e-3, &format!("{after} s after the reload message"));
    }
}

/// 40 (the minigun; 64 has the pistol and the shotgun). The spin-up
/// starts on the update after the press's; its first round on the
/// update after the one that sees 1300 ms of it done.
#[test]
fn wc40_minigun_latency() {
    let mut sim = sim_at(1.0 / 60.0);
    let p = player(&mut sim, Vec3::new(30.0, 1.0, 30.0), 0.0, 1.0);
    let w = arm(&mut sim, p, "test:minigun_special");
    let press = next_time(&sim);
    sim.intent(p).fire = true;
    sim.seconds(1.6);
    let got = round_updates(&sim, w, press, 1.0 / 60.0);
    // Spin-up from update 1; 1300 ms of it done on update 1 + 78; the
    // round on the next.
    assert_eq!(got.first(), Some(&80), "{got:?}");
}

/// 41. The pump re-press: pressed again during fire_0 and held, the second round on the update after the
/// one that sees the 600 ms fire_0 done (fire_0 from update 1, seen done
/// on 37, the round on 38: one cycle after the first round).
#[test]
fn wc41_pump_held() {
    let mut sim = sim_at(1.0 / 60.0);
    let p = player(&mut sim, Vec3::new(30.0, 1.0, 30.0), 0.0, 1.0);
    let w = arm(&mut sim, p, "combat_arms:remington_870");
    let press = next_time(&sim);
    sim.intent(p).fire = true;
    sim.ticks(1);
    // Let go, pressed again during fire_0 and held (semi-automatic).
    sim.intent(p).fire = false;
    sim.seconds(0.2);
    sim.intent(p).fire = true;
    sim.seconds(0.7);
    let got = round_updates(&sim, w, press, 1.0 / 60.0);
    // No extra state update inside the firing state: one cycle after the
    // first.
    assert_eq!(&got[..2], &[2, 38], "{got:?}");
}

// ---------------------------------------------------------------------------
// weapon_firing.md Follow-up (2026-10-31b) and effects_sounds.md Follow-up
// (2026-10-31b) 3: surface 0 (Unknown) is shot through.

/// A wall with no surface of its own (a level wall of surface type 0:
/// Unknown), `units` thick, across the lane at z.
fn bare_wall(sim: &mut Sim, z: f32, units: f32) {
    sim.app.world_mut().spawn((
        Transform::from_xyz(30.0, 1.5, z),
        RigidBody::Static,
        Collider::cuboid(3.0, 3.0, units * U),
    ));
    sim.ticks(1);
}

/// weapon_firing.md 70 and effects_sounds.md 95. A server ray through an
/// 8 u wall of surface 0, a torso behind it (1000 u, 500 u): damage x 0.6
/// and nothing else; a 12 u wall of surface 0 stops it.
#[test]
fn wf70_es95_unknown_wall() {
    for units in [1000.0, 500.0] {
        let mut clear = 0.0;
        for wall in [None, Some(8.0), Some(12.0)] {
            let mut sim = sim();
            let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", units);
            place_at(&mut sim, shooter, target, "Upper_torso", units);
            let at = sphere(&sim, shooter, target, "Upper_torso");
            aim(&mut sim, shooter, at);
            if let Some(w) = wall {
                bare_wall(&mut sim, 35.0, w);
            }
            let lost = pull(&mut sim, shooter, &[target])[0];
            match wall {
                None => {
                    assert!(lost > 0.0, "a clear lane hits");
                    clear = lost;
                }
                Some(8.0) => close(lost, clear * 0.6, 1e-3, &format!("8 u of Unknown at {units} u")),
                Some(_) => close(lost, 0.0, 1e-6, &format!("12 u of Unknown at {units} u")),
            }
        }
    }
}

/// effects_sounds.md 94. The surface table: Unknown (0) is shot through
/// (limit 10, re-perturb 10, range x 1.0, damage x 0.6), and the
/// shoot-through list is the 19 surfaces of Follow-up (2026-10-31b) 3.
#[test]
fn es94_unknown_in_the_surface_table() {
    let t = ca::CaSurfaces::built_in();
    let u = t.get("Unknown").expect("Unknown passes");
    close(u.thickness, 10.0 * U, 1e-6, "limit");
    close(u.perturb, 10.0, 0.0, "re-perturb");
    close(u.range, 1.0, 0.0, "range");
    close(u.damage, 0.6, 0.0, "damage");
    assert_eq!(u.id, 0);
    assert_eq!(t.ids(), vec![0, 1, 2, 3, 9, 23, 30, 32, 40, 50, 53, 65, 69, 76, 81, 100, 204, 206, 207]);
    for (name, limit, damage) in
        [("trussing", 10.0, 1.0), ("invisible metal", 0.0, 0.9), ("air", 0.0, 1.0), ("boss3_weak", 55.0, 0.3)]
    {
        let s = t.get(name).unwrap_or_else(|| panic!("{name} passes"));
        close(s.thickness, limit * U, 1e-6, name);
        close(s.damage, damage, 0.0, name);
    }
}
