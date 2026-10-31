//! specs/combat_arms/weapon_firing.md's test cases, numbered as there, on
//! Combat Arms' simulation (`CaSim`) and the greybox: the M4A1 from the
//! game's data, and test weapons made from it as data (`base =
//! "combat_arms:m4a1"` with a few keys changed: no roll for "roll 1.0",
//! no spread or kick where a case aims at one body part, a burst mode for
//! the M16A3). Where a case states the meters' or the kick's values
//! directly (14-25), the weapon's own parts (`CaSpread`, `CaRecoil`, as
//! the data built them in the sim) are stepped as the systems step them.
//!
//! Units: the data's world units are 1.8 cm (`units` in m4a1.toml); damage
//! is read as hit points (health x 100). Tolerances: 1e-3 HP for damage,
//! 1e-5 for the meters, 1e-3 degrees for view angles, one tick for times.

use avian3d::prelude::*;
use bevy::prelude::*;
use chimera_core::core::{Health, Intent, MovementState, PropSurface};
use chimera_sim::weapon::{
    self, AltModes, Hitscan, Inventory, Magazine, SpreadShape, ViewPunch, WeaponState, data::WeaponDataAppExt,
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{
    game::{
        movement,
        weapons::{
            self as ca, CaDamage, CaKick, CaRecoil, CaSpread, DEFAULT_BODY_GEAR, DEFAULT_HEAD_GEAR, FireMode,
            HEAVY_BODY_GEAR, MALE, SpreadScales,
        },
    },
    harness::CaSim,
};

/// Metres per world unit (the data's).
const U: f32 = 0.018;
/// Health per hit point.
const HP: f32 = 0.01;

/// The M4A1 with no damage roll, no spread and no kick: every round goes
/// where it is aimed and deals its base damage (the cases' "roll 1.0").
/// Its range is longer so a case can put a body part at the M4A1's last
/// band edge (the ray must reach the part, not just the box).
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

/// The M4A1's roll with no spread or kick (case 3).
const ROLLING: &str = r#"
id = "test:m4a1_rolling"
base = "combat_arms:m4a1"
[hooks."combat_arms:spread"]
cone = { min = 0, max = 0, inc = 1.3, dec = 0.3 }
[hooks."combat_arms:recoil"]
pitch = 0.0
"#;

/// Case 2: a range mod of +10 % (3850).
const RANGE_MOD: &str = r#"
id = "test:m4a1_range_mod"
base = "test:m4a1_exact"
[hitscan]
range = 3850.0
"#;

fn sim() -> Sim {
    let mut sim = Sim::new(CaSim, GreyboxMapPlugin);
    // One update, one tick of the game's.
    sim.set_tick_interval(combat_arms::game::TICK_INTERVAL);
    sim.app
        .add_weapon_data(&[("exact", EXACT), ("rolling", ROLLING), ("range", RANGE_MOD)])
        .unwrap();
    // Rounds through panels stay on their line (no exit re-perturb).
    super::weapon_corrections::steady_surfaces(&mut sim);
    sim
}

/// A Combat Arms player at `at` looking along `yaw`, settled, with health
/// `health` (1.0 = 100 HP) and no armour.
fn player(sim: &mut Sim, at: Vec3, yaw: f32, health: f32) -> Entity {
    let p = sim.spawn_character(at, movement::ID);
    sim.intent(p).yaw = yaw;
    sim.seconds(0.5);
    let world = sim.app.world_mut();
    world.entity_mut(p).insert((
        Health {
            current: health,
            max: health,
        },
        combat_arms::game::weapons::gear_armour(0.0, &Vec::new()),
    ));
    p
}

/// Give `p` the weapon `id`, drawn and ready to fire.
fn arm(sim: &mut Sim, p: Entity, id: &str) -> Entity {
    let w = weapon::give(sim.app.world_mut(), p, id).expect("unknown weapon");
    sim.seconds(1.3);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(w));
    w
}

fn health(sim: &Sim, p: Entity) -> f32 {
    sim.app.world().get::<Health>(p).unwrap().current
}

fn armour(sim: &Sim, p: Entity) -> f32 {
    sim.app.world().get::<chimera_sim::weapon::Armor>(p).map(combat_arms::game::weapons::points).unwrap()
}

fn set_armour(sim: &mut Sim, p: Entity, points: f32, gear: Vec<ca::Gear>) {
    sim.app.world_mut().entity_mut(p).insert(combat_arms::game::weapons::gear_armour(points, &gear));
}

fn mag(sim: &Sim, w: Entity) -> Magazine {
    sim.app.world().get::<Magazine>(w).unwrap().clone()
}

fn set_mag(sim: &mut Sim, w: Entity, clip: u32, reserve: u32) {
    let mut m = sim.app.world_mut().get_mut::<Magazine>(w).unwrap();
    m.clip = clip;
    m.reserve = reserve;
}

fn eye(sim: &Sim, p: Entity) -> Vec3 {
    sim.position(p) + sim.state(p).eye_offset
}

/// Where `name`'s sphere of the male skeleton is on `p`.
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

/// Look from `shooter`'s eye at `point`.
fn aim(sim: &mut Sim, shooter: Entity, point: Vec3) {
    let d = point - eye(sim, shooter);
    let mut i = sim.intent(shooter);
    i.yaw = (-d.x).atan2(-d.z);
    i.pitch = d.y.atan2(d.xz().length());
}

/// Updates from the press to the first round (the M4A1's `latency`,
/// Corrections 10): the round goes on the 3rd update after the press's.
const LATENCY: u64 = 3;

/// Fire one round (a one-tick pull: the chain's round goes all the same)
/// and let it land; the hit points `target` lost.
fn shoot(sim: &mut Sim, shooter: Entity, target: Entity) -> f32 {
    let before = health(sim, target);
    sim.intent(shooter).fire = true;
    sim.ticks(1);
    sim.intent(shooter).fire = false;
    sim.ticks(LATENCY + 1);
    let lost = (before - health(sim, target)) / HP;
    // Ready for the next round.
    sim.seconds(0.2);
    lost
}

/// Aim at `name` on `target` and fire one round.
fn shoot_at(sim: &mut Sim, shooter: Entity, target: Entity, name: &str) -> f32 {
    let at = sphere(sim, shooter, target, name);
    aim(sim, shooter, at);
    shoot(sim, shooter, target)
}

/// Move `target` along -Z until the impact point (where the ray to its
/// `name` sphere enters its box) is `units` from `shooter`'s eye.
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
        // The physics moves its bounds (`ColliderAabb`).
        sim.ticks(1);
    }
}

/// Turn `p` to `yaw` with its body (legs) turned too: no upper-body
/// twist (Corrections 6).
fn face(sim: &mut Sim, p: Entity, yaw: f32) {
    sim.intent(p).yaw = yaw;
    if let Some(w) = sim.app.world().get::<Inventory>(p).and_then(|i| i.active)
        && let Some(mut s) = sim.app.world_mut().get_mut::<CaSpread>(w)
    {
        s.body_yaw = Some(yaw);
        s.prev_view = None;
    }
}

fn close(a: f32, b: f32, tol: f32, what: &str) {

    assert!((a - b).abs() <= tol, "{what}: {a}, expected {b}");
}

/// A shooter looking down -Z and a target facing it, `units` apart in the
/// open lane at x = 30, the shooter holding `id`.
fn range_lane(sim: &mut Sim, id: &str, units: f32) -> (Entity, Entity) {
    let shooter = player(sim, Vec3::new(30.0, 1.0, 37.0), 0.0, 1.0);
    let target = player(sim, Vec3::new(30.0, 1.0, 37.0 - units * U), std::f32::consts::PI, 100.0);
    arm(sim, shooter, id);
    (shooter, target)
}

// ---------------------------------------------------------------------------
// Damage

/// 1. Range bands: 35 to 2500 units, 24.5 to 3000, 12.25 to 3500, edges
/// inclusive at the top.
#[test]
fn t01_range_bands() {
    let mut sim = sim();
    let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", 100.0);
    for (d, want) in [
        (100.0, 35.0),
        (2500.0, 35.0),
        (2500.5, 24.5),
        (3000.0, 24.5),
        (3000.5, 12.25),
        (3500.0, 12.25),
    ] {
        place_at(&mut sim, shooter, target, "Torso", d);
        close(shoot_at(&mut sim, shooter, target, "Torso"), want, 1e-3, &format!("at {d} units"));
    }
}

/// 2. Beyond the last band (a range mod makes the ray 3850): full damage.
#[test]
fn t02_beyond_the_last_band() {
    let mut sim = sim();
    let (shooter, target) = range_lane(&mut sim, "test:m4a1_range_mod", 3600.0);
    place_at(&mut sim, shooter, target, "Torso", 3600.0);
    close(shoot_at(&mut sim, shooter, target, "Torso"), 35.0, 1e-3, "at 3600 units");
}

/// 3. The roll's bounds: 33.25 to 36.75 at d = 100.
#[test]
fn t03_random_roll_bounds() {
    let mut sim = sim();
    let (shooter, target) = range_lane(&mut sim, "test:m4a1_rolling", 100.0);
    place_at(&mut sim, shooter, target, "Torso", 100.0);
    let w = sim.app.world().get::<Inventory>(shooter).unwrap().active.unwrap();
    let dmg = sim.app.world().get::<CaDamage>(w).unwrap().clone();
    // The roll at its ends.
    close(ca::base_damage(&dmg, 0.0, false, 1.0) / HP, 33.25, 1e-3, "minimum");
    close(ca::base_damage(&dmg, 1.0, false, 1.0) / HP, 36.75, 1e-3, "maximum");
    // Rounds fired: all within, spread over the range.
    let mut hits = Vec::new();
    for i in 0..60 {
        if i % 25 == 0 {
            set_mag(&mut sim, w, 30, 90);
        }
        hits.push(shoot_at(&mut sim, shooter, target, "Torso"));
    }
    let (lo, hi) = hits.iter().fold((f32::MAX, f32::MIN), |(lo, hi), h| (lo.min(*h), hi.max(*h)));
    assert!(lo >= 33.25 - 1e-3 && hi <= 36.75 + 1e-3, "{hits:?}");
    assert!(lo < 33.8 && hi > 36.2, "60 rolls all near the middle: {lo}..{hi}");
}

/// 4. Zones, unarmoured, roll 1.0 (at 1000 units, the same first band as
/// the case's 100: from 1 m the rays to the feet would cross the shins).
#[test]
fn t04_zones() {
    let mut sim = sim();
    let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", 1000.0);
    for (name, want) in [
        ("Face_Eyelid1", 210.0),
        ("Neck", 35.0),
        ("Torso", 35.0),
        ("Nuts", 35.0),
        ("Right_pelvis", 31.5),
        ("Right_armu", 31.5),
        ("Right_hand", 31.5),
        ("Right_Legu", 31.5),
        ("Right_legl", 26.25),
        ("Right_foot", 17.5),
    ] {
        close(shoot_at(&mut sim, shooter, target, name), want, 1e-3, name);
    }
}

/// 5. The behind bonus by the bullet's direction in the victim's frame
/// (forward, right): sectors 9 and 11 get 1.3, 3 and 6 don't.
#[test]
fn t05_behind_bonus() {
    // The victim faces +Z (yaw pi): its right is -X.
    let (f, r) = (Vec3::Z, Vec3::NEG_X);
    for (dir, sector, want) in [
        (f, 9, 45.5),
        (-f, 3, 35.0),
        (0.95 * r + 0.312 * f, 6, 35.0),
        (-0.95 * r + 0.312 * f, 11, 45.5),
    ] {
        let dir = dir.normalize();
        assert_eq!(ca::sector(f, r, dir), sector, "{dir}");
        let mut sim = sim();
        let target = player(&mut sim, Vec3::new(30.0, 1.0, 20.0), std::f32::consts::PI, 100.0);
        let i = MALE.iter().position(|s| s.name == "Torso").unwrap();
        let chest = geometry(sim.app.world(), target).2[i].0;
        let from = (chest - dir * 8.0).with_y(1.0);
        let shooter = player(&mut sim, from, 0.0, 1.0);
        arm(&mut sim, shooter, "test:m4a1_exact");
        close(shoot_at(&mut sim, shooter, target, "Torso"), want, 1e-3, &format!("sector {sector}"));
    }
}

/// 6. Body armour: 29.75 through the default body gear, armour 94.75.
#[test]
fn t06_body_armour() {
    let mut sim = sim();
    let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", 100.0);
    set_armour(&mut sim, target, 100.0, vec![DEFAULT_BODY_GEAR]);
    sim.app.world_mut().get_mut::<Health>(target).unwrap().current = 1.0;
    close(shoot_at(&mut sim, shooter, target, "Torso"), 29.75, 1e-3, "damage");
    close(armour(&sim, target), 94.75, 1e-3, "armour");
    close(health(&sim, target) / HP, 70.25, 1e-3, "health");
}

/// 7. Armour and a headshot: (35 - 8.75) x 6 = 157.5, armour 91.25; from
/// 100 HP, dead.
#[test]
fn t07_armour_and_headshot() {
    for start in [2.0, 1.0] {
        let mut sim = sim();
        let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", 100.0);
        set_armour(&mut sim, target, 100.0, vec![DEFAULT_BODY_GEAR, DEFAULT_HEAD_GEAR]);
        sim.app.world_mut().get_mut::<Health>(target).unwrap().current = start;
        let lost = shoot_at(&mut sim, shooter, target, "Face_Eyelid1");
        close(armour(&sim, target), 91.25, 1e-3, "armour");
        if start > 1.0 {
            close(lost, 157.5, 1e-3, "damage");
        } else {
            assert_eq!(health(&sim, target), 0.0, "dead");
        }
    }
}

/// 8. Body gear doesn't cover the head: 210, armour stays 100.
#[test]
fn t08_body_gear_does_not_cover_the_head() {
    let mut sim = sim();
    let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", 100.0);
    set_armour(&mut sim, target, 100.0, vec![DEFAULT_BODY_GEAR]);
    close(shoot_at(&mut sim, shooter, target, "Face_Eyelid1"), 210.0, 1e-3, "damage");
    close(armour(&sim, target), 100.0, 1e-3, "armour");
}

/// 9. Armour runs out: 3 points take 3, the hit does 32, armour 0.
#[test]
fn t09_armour_runs_out() {
    let mut sim = sim();
    let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", 100.0);
    set_armour(&mut sim, target, 3.0, vec![DEFAULT_BODY_GEAR]);
    close(shoot_at(&mut sim, shooter, target, "Torso"), 32.0, 1e-3, "damage");
    close(armour(&sim, target), 0.0, 1e-3, "armour");
}

/// 10. No armour points, no protection, whatever the gear.
#[test]
fn t10_zero_armour_means_no_protection() {
    let mut sim = sim();
    let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", 100.0);
    set_armour(&mut sim, target, 0.0, vec![HEAVY_BODY_GEAR]);
    close(shoot_at(&mut sim, shooter, target, "Torso"), 35.0, 1e-3, "damage");
}

/// 11. Banded damage with armour at 2800 units: 24.5, 3.675 blocked,
/// 20.825 dealt, armour 96.325.
#[test]
fn t11_banded_damage_with_armour() {
    let mut sim = sim();
    let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", 2800.0);
    place_at(&mut sim, shooter, target, "Torso", 2800.0);
    set_armour(&mut sim, target, 100.0, vec![DEFAULT_BODY_GEAR]);
    close(shoot_at(&mut sim, shooter, target, "Torso"), 20.825, 1e-3, "damage");
    close(armour(&sim, target), 96.325, 1e-3, "armour");
}

/// 12. The death threshold: 36 HP hit for 35 is dead (1 HP left counts);
/// 36.01 lives with 1.01.
#[test]
fn t12_death_threshold() {
    for (start, alive) in [(36.0, false), (36.01, true)] {
        let mut sim = sim();
        let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", 100.0);
        sim.app.world_mut().get_mut::<Health>(target).unwrap().current = start * HP;
        shoot_at(&mut sim, shooter, target, "Torso");
        let left = health(&sim, target) / HP;
        if alive {
            close(left, 1.01, 1e-3, "left");
        } else {
            assert_eq!(left, 0.0, "from {start}: dead");
        }
    }
}

/// 13. Hits to kill: unarmoured 3 (65, 30, dead); with the default body
/// gear 4 (70.25, 40.5, 10.75, dead).
#[test]
fn t13_hits_to_kill() {
    for (armoured, steps) in [(false, &[65.0, 30.0, 0.0][..]), (true, &[70.25, 40.5, 10.75, 0.0][..])] {
        let mut sim = sim();
        let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", 100.0);
        sim.app.world_mut().get_mut::<Health>(target).unwrap().current = 1.0;
        if armoured {
            set_armour(&mut sim, target, 100.0, vec![DEFAULT_BODY_GEAR]);
        }
        for (i, want) in steps.iter().enumerate() {
            shoot_at(&mut sim, shooter, target, "Torso");
            close(health(&sim, target) / HP, *want, 1e-3, &format!("armoured {armoured}, hit {}", i + 1));
        }
    }
}

// ---------------------------------------------------------------------------
// Spread

/// The M4A1 of a player in the sim, and the scales.
fn m4a1(sim: &mut Sim) -> (Entity, Entity) {
    let p = player(sim, Vec3::new(30.0, 1.0, 30.0), 0.0, 1.0);
    let w = sim.app.world().get::<Inventory>(p).unwrap().active.unwrap();
    assert_eq!(sim.app.world().get::<weapon::Weapon>(w).unwrap().id, ca::RIFLE);
    (p, w)
}

/// The weapon's spread part, its meters at rest (the cases' start; the
/// player's landing at spawn moved them).
fn spread(sim: &Sim, w: Entity) -> CaSpread {
    let mut s = sim.app.world().get::<CaSpread>(w).unwrap().clone();
    s.fire = 0.0;
    s.movement = 0.0;
    s.last_shot = None;
    s
}

fn scales(sim: &Sim) -> SpreadScales {
    *sim.app.world().resource::<SpreadScales>()
}

/// 14. From rest, holding fire with no recovery between rounds: cones 3,
/// 7, 12, 17, 21, 26, 31, 32, 32; the meter capped at 0.30 after the 7th.
#[test]
fn t14_spread_from_rest() {
    let mut sim = sim();
    // The M4A1 without recovery (all rounds within DecDelay) or kick.
    sim.app
        .add_weapon_data(&[(
            "slow",
            r#"
            id = "test:m4a1_no_recovery"
            base = "combat_arms:m4a1"
            [hooks."combat_arms:spread"]
            dec_delay = 100000.0
            [hooks."combat_arms:recoil"]
            pitch = 0.0
            "#,
        )])
        .unwrap();
    let p = player(&mut sim, Vec3::new(30.0, 1.0, 30.0), 0.0, 1.0);
    let w = arm(&mut sim, p, "test:m4a1_no_recovery");
    sim.intent(p).fire = true;
    let mut cones = Vec::new();
    let mut meters = Vec::new();
    let mut clip = mag(&sim, w).clip;
    while cones.len() < 9 {
        // The cone the next round gets, as the frame's spread tells it.
        let SpreadShape::Disc { inaccuracy, .. } = sim.app.world().get::<Hitscan>(w).unwrap().spread else {
            panic!()
        };
        sim.ticks(1);
        let now = mag(&sim, w).clip;
        if now < clip {
            cones.push((inaccuracy * 1000.0).round() as i32);
            meters.push(sim.app.world().get::<CaSpread>(w).unwrap().fire);
            clip = now;
        }
    }
    assert_eq!(cones, [3, 7, 12, 17, 21, 26, 31, 32, 32]);
    close(meters[6], 0.30, 1e-5, "meter after the 7th");
}

/// 15. The firing cap by stance: moving, 0.36 grows to the cap 0.38 (next
/// cone 39); standing still with 0.36 left over, nothing changes until
/// the next round sets 0.30.
#[test]
fn t15_firing_cap_by_stance() {
    let mut sim = sim();
    let (_, w) = m4a1(&mut sim);
    let (sc, mut s) = (scales(&sim), spread(&sim, w));
    let (cone, st) = (s.cone_params(FireMode::Normal), s.states(FireMode::Normal));
    s.fire = 0.36;
    s.fire_substep(&sc, cone, true, st.fire_move, 0.0);
    close(s.fire, 0.38, 1e-5, "moving");
    close(s.cone(FireMode::Normal), 39.0, 0.0, "cone");
    s.fire = 0.36;
    s.fire_substep(&sc, cone, false, st.fire, 0.05);
    close(s.fire, 0.36, 1e-5, "still, no round");
    s.fire_substep(&sc, cone, true, st.fire, 0.06);
    close(s.fire, 0.30, 1e-5, "still, the next round");
}

/// 16. Recovery waits DecDelay: unchanged at 50 ms; as corrected (the
/// wait is strictly less than DecDelay, case 38), 0.28875 already at 95.
#[test]
fn t16_firing_recovery_delay() {
    let mut sim = sim();
    let (_, w) = m4a1(&mut sim);
    let (sc, mut s) = (scales(&sim), spread(&sim, w));
    let cone = s.cone_params(FireMode::Normal);
    s.fire = 0.30;
    s.last_shot = Some(0.0);
    s.fire_substep(&sc, cone, false, 0.30, 0.050);
    close(s.fire, 0.30, 1e-6, "at 50 ms");
    s.fire_substep(&sc, cone, false, 0.30, 0.095);
    close(s.fire, 0.28875, 1e-5, "at 95 ms");
}

/// 17. Recovery length: cone back to 3 after 26 sub-steps (0.0075), the
/// meter 0 after 27.
#[test]
fn t17_firing_recovery_length() {
    let mut sim = sim();
    let (_, w) = m4a1(&mut sim);
    let (sc, mut s) = (scales(&sim), spread(&sim, w));
    let cone = s.cone_params(FireMode::Normal);
    s.fire = 0.30;
    s.last_shot = Some(-1.0);
    for i in 1..=27 {
        s.fire_substep(&sc, cone, false, 0.30, i as f64 * ca::SPREAD_SUBSTEP);
        if i == 26 {
            close(s.fire, 0.0075, 1e-5, "after 26");
            close(s.cone(FireMode::Normal), 3.0, 0.0, "cone after 26");
        }
        if i == 25 {
            assert!(s.cone(FireMode::Normal) > 3.0, "back to 3 only at 26");
        }
    }
    close(s.fire, 0.0, 0.0, "after 27");
}

/// 18. The movement meter rising at a run: 0.04875 a step up to 0.25;
/// the cone then 27.
#[test]
fn t18_movement_rise() {
    let mut sim = sim();
    let (_, w) = m4a1(&mut sim);
    let (sc, mut s) = (scales(&sim), spread(&sim, w));
    let (cone, st) = (s.cone_params(FireMode::Normal), s.states(FireMode::Normal));
    for want in [0.04875, 0.0975, 0.14625, 0.195, 0.24375, 0.25] {
        s.move_substep(&sc, cone, st.run, st.run, 0.0, false, st.jump);
        close(s.movement, want, 1e-5, "rise");
    }
    close(s.cone(FireMode::Normal), 27.0, 0.0, "cone");
}

/// 19. The movement meter falling to rest: the spec's 11 steps, the cone
/// back to 3 at the 11th.
#[test]
fn t19_movement_fall() {
    let mut sim = sim();
    let (_, w) = m4a1(&mut sim);
    let (sc, mut s) = (scales(&sim), spread(&sim, w));
    let (cone, st) = (s.cone_params(FireMode::Normal), s.states(FireMode::Normal));
    s.movement = 0.25;
    let want = [
        0.21625, 0.1825, 0.14875, 0.115, 0.08125, 0.05688, 0.03981, 0.02787, 0.01951, 0.01366, 0.00956,
    ];
    for (i, want) in want.iter().enumerate() {
        s.move_substep(&sc, cone, st.run, 0.0, 0.0, false, st.jump);
        close(s.movement, *want, 1e-5, &format!("step {}", i + 1));
        if i == 9 {
            assert!(s.cone(FireMode::Normal) > 3.0);
        }
    }
    close(s.cone(FireMode::Normal), 3.0, 0.0, "cone");
}

/// 20. Turning snaps the meter (Q11): 0.2 rad of body yaw in one step
/// gives rho 0.296 and M 0; without the turn 0.16625 (as corrected, the
/// change is the body's: a mouse turn into the twist is none, case 42).
#[test]
fn t20_turn_snap() {
    let mut sim = sim();
    let (_, w) = m4a1(&mut sim);
    let (sc, base) = (scales(&sim), spread(&sim, w));
    let (cone, st) = (base.cone_params(FireMode::Normal), base.states(FireMode::Normal));
    let rho = CaSpread::turn(&sc, 0.0, 0.2);
    close(rho, 0.29575, 1e-4, "rho");
    let mut s = base.clone();
    s.movement = 0.20;
    s.move_substep(&sc, cone, st.run, 0.0, rho, false, st.jump);
    close(s.movement, 0.0, 1e-6, "turning");
    let mut s = base;
    s.movement = 0.20;
    s.move_substep(&sc, cone, st.run, 0.0, 0.0, false, st.jump);
    close(s.movement, 0.16625, 1e-5, "still");
}

/// 21. A jump: 15 steps to 0.70 (the 15th clamps), cone 70.
#[test]
fn t21_jump() {
    let mut sim = sim();
    let (_, w) = m4a1(&mut sim);
    let (sc, mut s) = (scales(&sim), spread(&sim, w));
    let (cone, st) = (s.cone_params(FireMode::Normal), s.states(FireMode::Normal));
    for i in 1..=15 {
        s.move_substep(&sc, cone, st.jump, st.jump, 0.0, true, st.jump);
        if i < 15 {
            close(s.movement, 0.04875 * i as f32, 1e-5, &format!("step {i}"));
        }
    }
    close(s.movement, 0.70, 1e-6, "after 15");
    close(s.cone(FireMode::Normal), 70.0, 0.0, "cone");
}

/// 22. The square's corner: n 32, draws 1 and 0 bend the aim 0.032 right
/// and 0.032 down: (0.03198, -0.03198, 0.99898) in the spec's frame
/// (forward +Z there, -Z here). The spec's side components are 0.032
/// over 1.000512; normalised over the whole vector (1.001023, which its
/// 0.99898 uses) they are 0.031967: 2e-5 tolerance there.
#[test]
fn t22_ray_deflection() {
    let d = ca::square_dir(Quat::IDENTITY, 0.032, 1.0, 0.0);
    close(d.x, 0.03198, 2e-5, "right");
    close(d.y, -0.03198, 2e-5, "up");
    close(-d.z, 0.99898, 1e-5, "forward");
}

// ---------------------------------------------------------------------------
// Recoil

/// The M4A1 without the recoil's return, held: the view's pitch and yaw
/// kick after each round (degrees), and the accumulators.
fn kicks(rounds: usize) -> Vec<(f32, f32, f32, f32)> {
    let mut sim = sim();
    sim.app
        .add_weapon_data(&[(
            "stiff",
            r#"
            id = "test:m4a1_no_return"
            base = "combat_arms:m4a1"
            [hooks."combat_arms:recoil"]
            jitter_wait_recover = 0.0
            jitter_recover = 0.0
            "#,
        )])
        .unwrap();
    let p = player(&mut sim, Vec3::new(30.0, 1.0, 30.0), 0.0, 1.0);
    let w = arm(&mut sim, p, "test:m4a1_no_return");
    sim.intent(p).fire = true;
    let mut out = Vec::new();
    let mut clip = mag(&sim, w).clip;
    while out.len() < rounds {
        sim.ticks(1);
        let now = mag(&sim, w).clip;
        if now < clip {
            clip = now;
            let punch = sim.app.world().get::<ViewPunch>(p).map_or(Vec3::ZERO, |p| p.0);
            let k = sim.app.world().get::<CaKick>(p).unwrap();
            out.push((punch.x.to_degrees(), punch.y.to_degrees(), k.pitch, k.yaw));
        }
    }
    out
}

/// 23. The kick sequence: 1.300, 1.261, 1.223, 1.187, 1.151, 1.116,
/// 1.083, 1.050, 0.629, 0, 0 degrees; 10.000 after the 9th; J 0.011345
/// then 0.022349 rad.
#[test]
fn t23_view_kick_sequence() {
    let k = kicks(11);
    let want = [1.300, 1.261, 1.223, 1.187, 1.151, 1.116, 1.083, 1.050, 0.629, 0.0, 0.0];
    let mut last = 0.0;
    for (i, w) in want.iter().enumerate() {
        close(k[i].0 - last, *w, 1e-3, &format!("round {}", i + 1));
        last = k[i].0;
    }
    close(k[8].0, 10.0, 1e-3, "after 9");
    close(k[0].2, 0.011345, 1e-6, "J after 1");
    close(k[1].2, 0.022349, 1e-6, "J after 2");
}

/// 24. The wag starts on the second round: none after the first (J
/// 0.0113 <= 0.02), exactly 1.2 degrees after the second, J_y 0.010472.
#[test]
fn t24_wag_start() {
    let k = kicks(2);
    close(k[0].1, 0.0, 0.0, "no side kick after 1");
    close(k[1].1.abs(), 1.2, 1e-3, "side kick after 2");
    close(k[1].3.abs(), 0.010472, 1e-6, "J_y after 2");
    // Half the view offset each (Q4).
    close(k[1].3 * 2.0, k[1].1.to_radians(), 1e-6, "J_y is half the yaw offset");
}

/// 25. The return over a 16 ms frame: 0.0018080 rad down 50 ms after the
/// kick (the waiting rate), 0.0036160 at 120 ms.
#[test]
fn t25_recoil_return() {
    let mut sim = sim();
    let (_, w) = m4a1(&mut sim);
    let r = sim.app.world().get::<CaRecoil>(w).unwrap().clone();
    for (since, down, left) in [(0.050, 0.0018080, 0.0104406), (0.120, 0.0036160, 0.0095366)] {
        let mut k = CaKick {
            pitch: 0.0113446,
            last_kick: Some(10.0 - since),
            ..default()
        };
        let (d, side) = r.recover(&mut k, 0.016, 10.0, false);
        close(d, down, 1e-7, &format!("{since} s after"));
        close(side, 0.0, 0.0, "no side");
        close(k.pitch, left, 1e-7, "J");
    }
}

// ---------------------------------------------------------------------------
// Trigger, ammo and reloads

/// Rounds fired so far by `w` from a full clip.
fn fired(sim: &Sim, w: Entity) -> u32 {
    30 - mag(sim, w).clip
}

/// 26. The fire cycle: exact time fires at 0, 105, 210, ... ms from the
/// first round (the 3rd update after the press, case 64), 30 by 3045 ms;
/// frame-quantised at 60 fps one round every 7 frames.
#[test]
fn t26_fire_cycle() {
    let mut sim = self::sim();
    let p = player(&mut sim, Vec3::new(30.0, 1.0, 30.0), 0.0, 1.0);
    let w = sim.app.world().get::<Inventory>(p).unwrap().active.unwrap();
    sim.seconds(1.0);
    let dt = 1.0 / sim.tick_hz();
    sim.intent(p).fire = true;
    let mut ticks = 0u32;
    while fired(&sim, w) < 30 {
        sim.ticks(1);
        // The press tick is 0, the first round's t = 0.
        let t = (ticks as f64 - LATENCY as f64) * dt;
        let want = if t < -1e-9 { 0 } else { ((t + 1e-6) / 0.105).floor() as u32 + 1 };
        assert_eq!(fired(&sim, w), want.min(30), "at {t} s");
        ticks += 1;
    }
    let t = (ticks as f64 - 1.0 - LATENCY as f64) * dt;
    assert!(t >= 3.045 && t < 3.045 + dt, "30th at {t} s");

    // Frame-quantised, 60 frames a second.
    let mut sim = self::sim();
    sim.app
        .add_weapon_data(&[(
            "set",
            "id = \"test:m4a1_frames\"\nbase = \"combat_arms:m4a1\"\n[trigger]\ntiming = \"set\"",
        )])
        .unwrap();
    sim.set_tick_interval(1.0 / 60.0);
    let p = player(&mut sim, Vec3::new(30.0, 1.0, 30.0), 0.0, 1.0);
    let w = arm(&mut sim, p, "test:m4a1_frames");
    sim.intent(p).fire = true;
    let mut at = Vec::new();
    for tick in 0..60 {
        let before = fired(&sim, w);
        sim.ticks(1);
        if fired(&sim, w) > before {
            at.push(tick);
        }
    }
    let gaps: Vec<i32> = at.windows(2).map(|p| p[1] - p[0]).collect();
    assert!(gaps.len() >= 7 && gaps.iter().all(|g| *g == 7), "{gaps:?}");
}

/// 27. Semi-automatic (`RapidFireCount` 1): held for 1 s, one round;
/// again on the next press.
#[test]
fn t27_semi_auto() {
    let mut sim = sim();
    sim.app
        .add_weapon_data(&[(
            "semi",
            "id = \"test:m4a1_semi\"\nbase = \"combat_arms:m4a1\"\n[trigger]\nautomatic = false",
        )])
        .unwrap();
    let p = player(&mut sim, Vec3::new(30.0, 1.0, 30.0), 0.0, 1.0);
    let w = arm(&mut sim, p, "test:m4a1_semi");
    sim.intent(p).fire = true;
    sim.seconds(1.0);
    assert_eq!(fired(&sim, w), 1);
    sim.intent(p).fire = false;
    sim.ticks(2);
    sim.intent(p).fire = true;
    sim.seconds(1.0);
    assert_eq!(fired(&sim, w), 2);
}

/// The M16A3 (the spec's burst example) on the M4A1's numbers: burst
/// mode with its cone and kick, 3 rounds a cycle apart, the next pull
/// 210 ms after the third (420 after the first), 300 ms after a toggle.
const M16A3: &str = r#"
id = "test:m16a3"
base = "combat_arms:m4a1"
[alt]
kind = "burst"
interval = 0.105
refire = 0.42
count = 3
step = 0.3
once_per_press = true
step_blocks_primary = true
[hooks."combat_arms:spread"]
burst_cone = { min = 3, max = 60, inc = 1.4, dec = 0.2 }
[hooks."combat_arms:recoil"]
burst_pitch = 1.6
burst_max_pitch = 8.0
"#;

/// An M16A3 in burst mode `wait` seconds after the toggle.
fn m16a3(wait: f64) -> (Sim, Entity, Entity) {
    let mut sim = sim();
    sim.app.add_weapon_data(&[("m16a3", M16A3)]).unwrap();
    let p = player(&mut sim, Vec3::new(30.0, 1.0, 30.0), 0.0, 1.0);
    let w = arm(&mut sim, p, "test:m16a3");
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    assert_eq!(sim.app.world().get::<AltModes>(w).unwrap().current, 1, "burst on");
    sim.seconds(wait);
    (sim, p, w)
}

/// Ticks for `secs`, rounded up (the first tick at or after).
fn ticks_for(sim: &Sim, secs: f64) -> u64 {
    (secs * sim.tick_hz() - 1e-6).ceil() as u64
}

/// 28. Bursts: held for 2 s, 3 rounds (0, 105, 210 ms from the first,
/// which is the 3rd update after the press) then none; a tap released at
/// 20 ms still 3; a press 90 ms after the third waits until 420 ms after
/// the first; a new press then gives the next burst.
#[test]
fn t28_burst() {
    let (mut sim, p, w) = m16a3(0.35);
    sim.intent(p).fire = true;
    let mut at = Vec::new();
    let n = ticks_for(&sim, 2.0);
    for tick in 0..n {
        let before = fired(&sim, w);
        sim.ticks(1);
        if fired(&sim, w) > before {
            at.push(tick);
        }
    }
    let want: Vec<u64> = [0.0, 0.105, 0.210].iter().map(|t| LATENCY + ticks_for(&sim, *t)).collect();
    assert_eq!(at, want, "held for 2 s");
    sim.intent(p).fire = false;

    // A tap, released at 20 ms (held on the ticks before it).
    let (mut sim, p, w) = m16a3(0.35);
    // The press tick is 0; `next` is the time of the tick to come from
    // the first round.
    let start = sim.tick() + LATENCY;
    let next = |sim: &Sim| (sim.tick() as f64 - start as f64) / sim.tick_hz();
    sim.intent(p).fire = true;
    while next(&sim) < 0.020 - LATENCY as f64 / sim.tick_hz() {
        sim.ticks(1);
    }
    sim.intent(p).fire = false;
    // Pressed again at 300 ms (90 after the third): nothing before 420.
    while next(&sim) < 0.300 {
        sim.ticks(1);
    }
    assert_eq!(fired(&sim, w), 3, "a tap");
    sim.intent(p).fire = true;
    while next(&sim) < 0.420 - 1e-6 {
        sim.ticks(1);
        assert_eq!(fired(&sim, w), 3, "before 420 ms");
    }
    // Still in the firing state (the lockout is the last cycle): the
    // held trigger fires when it ends, no chain.
    sim.ticks(1);
    assert_eq!(fired(&sim, w), 4, "the next burst from 420 ms");
}

/// 29. The toggle's lockout: toggled at 0, fire held from 200 ms, the
/// first round at 300 ms or later.
#[test]
fn t29_burst_toggle_lockout() {
    let (mut sim, p, w) = m16a3(0.0);
    // The toggle was the tick before.
    let toggled = sim.tick() - 1;
    while (sim.tick() - toggled) as f64 / sim.tick_hz() < 0.200 {
        sim.ticks(1);
    }
    sim.intent(p).fire = true;
    while fired(&sim, w) == 0 {
        sim.ticks(1);
    }
    let t = (sim.tick() - 1 - toggled) as f64 / sim.tick_hz();
    // The chain starts once the lockout is over (Corrections 10).
    let latency = LATENCY as f64 / sim.tick_hz();
    assert!(t >= 0.300 + latency - 1e-6 && t < 0.300 + latency + 1.0 / sim.tick_hz(), "first round at {t} s");
}

/// 30. Empty with a reserve: one round, the reload starts by itself about
/// 105 ms later, 2300 ms after that 30 in the clip and 60 in all; still
/// held, firing goes on.
#[test]
fn t30_empty_and_auto_reload() {
    let mut sim = sim();
    let p = player(&mut sim, Vec3::new(30.0, 1.0, 30.0), 0.0, 1.0);
    let w = sim.app.world().get::<Inventory>(p).unwrap().active.unwrap();
    sim.seconds(1.0);
    set_mag(&mut sim, w, 1, 60);
    sim.intent(p).fire = true;
    sim.ticks(LATENCY + 1);
    let m = mag(&sim, w);
    assert_eq!((m.clip, m.clip + m.reserve), (0, 60));
    let shot = sim.tick() - 1;
    while sim.app.world().get::<WeaponState>(w).unwrap().reload_end.is_none() {
        sim.ticks(1);
        assert!(sim.tick() - shot < 20, "no reload");
    }
    let started = (sim.tick() - 1 - shot) as f64 / sim.tick_hz();
    assert!((0.105..0.105 + 1.0 / sim.tick_hz()).contains(&started), "started at {started}");
    let begin = sim.tick() - 1;
    while mag(&sim, w).clip == 0 {
        sim.ticks(1);
    }
    let took = (sim.tick() - 1 - begin) as f64 / sim.tick_hz();
    assert!((2.3 - 1e-6..2.3 + 1.0 / sim.tick_hz()).contains(&took), "took {took}");
    let m = mag(&sim, w);
    // Held: the update that fills the clip enters the firing state, the
    // round goes 3 updates later (inventory.md case 45).
    assert_eq!(m.clip, 30);
    sim.ticks(LATENCY - 1);
    assert_eq!(mag(&sim, w).clip, 30, "not yet");
    sim.ticks(1);
    let m = mag(&sim, w);
    assert_eq!((m.clip, m.clip + m.reserve), (29, 59), "firing again");
}

fn reload(sim: &mut Sim, p: Entity) {
    sim.intent(p).reload = true;
    sim.ticks(1);
    sim.intent(p).reload = false;
}

/// 31. Partial reloads never lose rounds: 12/70 to 30/70; 12/20 to 20/20.
#[test]
fn t31_partial_reload() {
    for (clip, total, want) in [(12, 70, (30, 70)), (12, 20, (20, 20))] {
        let mut sim = sim();
        let (p, w) = m4a1(&mut sim);
        sim.seconds(1.0);
        set_mag(&mut sim, w, clip, total - clip);
        reload(&mut sim, p);
        sim.seconds(2.4);
        let m = mag(&sim, w);
        assert_eq!((m.clip, m.clip + m.reserve), want, "from {clip}/{total}");
    }
}

/// 32. Refused reloads: a full clip; nothing in reserve.
#[test]
fn t32_reload_refused() {
    for (clip, total) in [(30, 120), (25, 25)] {
        let mut sim = sim();
        let (p, w) = m4a1(&mut sim);
        sim.seconds(1.0);
        set_mag(&mut sim, w, clip, total - clip);
        reload(&mut sim, p);
        assert!(sim.app.world().get::<WeaponState>(w).unwrap().reload_end.is_none(), "{clip}/{total}");
        sim.seconds(2.4);
        let m = mag(&sim, w);
        assert_eq!((m.clip, m.clip + m.reserve), (clip, total));
    }
}

/// 33. Switching away abandons a reload: 5/95 stays 5/95 and no reload
/// starts on the way back (the clip isn't empty).
#[test]
fn t33_reload_cancelled_by_switch() {
    let mut sim = sim();
    let (p, w) = m4a1(&mut sim);
    sim.seconds(1.0);
    set_mag(&mut sim, w, 5, 90);
    reload(&mut sim, p);
    sim.seconds(2.0);
    let knife = sim
        .app
        .world()
        .get::<Inventory>(p)
        .unwrap()
        .weapons
        .iter()
        .copied()
        .find(|e| *e != w)
        .unwrap();
    let slot = sim.app.world().get::<weapon::Weapon>(knife).unwrap().slot;
    sim.intent(p).select = Some(slot);
    sim.ticks(2);
    sim.intent(p).select = None;
    sim.ticks(1);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(knife));
    sim.intent(p).select = Some(0);
    sim.ticks(2);
    sim.intent(p).select = None;
    sim.seconds(3.0);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(w));
    let m = mag(&sim, w);
    assert_eq!((m.clip, m.clip + m.reserve), (5, 95));
    assert!(sim.app.world().get::<WeaponState>(w).unwrap().reload_end.is_none());
}

/// 34. Fire held during a reload is ignored: none before 2300 ms; the
/// first round after takes the clip from 30 to 29.
#[test]
fn t34_fire_during_reload() {
    let mut sim = sim();
    let (p, w) = m4a1(&mut sim);
    sim.seconds(1.0);
    set_mag(&mut sim, w, 0, 90);
    // The empty clip reloads by itself on this tick.
    sim.ticks(1);
    let start = sim.tick() - 1;
    assert!(sim.app.world().get::<WeaponState>(w).unwrap().reload_end.is_some());
    sim.seconds(0.5);
    sim.intent(p).fire = true;
    while (sim.tick() - start) as f64 / sim.tick_hz() < 2.3 - 1e-6 {
        assert_eq!(mag(&sim, w).clip, 0, "no round during the reload");
        sim.ticks(1);
    }
    sim.ticks(LATENCY + 1);
    let m = mag(&sim, w);
    assert_eq!((m.clip, m.reserve), (29, 60));
}

/// 35. The server's spacing (`fireDelay` 100): rounds at 0 and 60 ms;
/// the second deals nothing, both are spent.
#[test]
fn t35_server_rate_limit() {
    let mut sim = sim();
    sim.app
        .add_weapon_data(&[(
            "fast",
            r#"
            id = "test:m4a1_fast"
            base = "test:m4a1_exact"
            [trigger]
            cycle = 0.06
            [hooks."combat_arms:damage"]
            fire_delay = 100.0
            "#,
        )])
        .unwrap();
    let (shooter, target) = range_lane(&mut sim, "test:m4a1_fast", 1000.0);
    let w = sim.app.world().get::<Inventory>(shooter).unwrap().active.unwrap();
    let at = sphere(&sim, shooter, target, "Torso");
    aim(&mut sim, shooter, at);
    let before = health(&sim, target);
    sim.intent(shooter).fire = true;
    while fired(&sim, w) < 2 {
        sim.ticks(1);
    }
    sim.intent(shooter).fire = false;
    sim.ticks(2);
    assert_eq!(fired(&sim, w), 2);
    close((before - health(&sim, target)) / HP, 35.0, 1e-3, "one round's damage");
}

/// A panel of `surface`, `units` thick, across the lane at z.
fn panel(sim: &mut Sim, z: f32, units: f32, surface: &str) {
    sim.app.world_mut().spawn((
        Transform::from_xyz(30.0, 1.5, z),
        RigidBody::Static,
        Collider::cuboid(3.0, 3.0, units * U),
        PropSurface(surface.into()),
    ));
    sim.ticks(1);
}

/// 36. Through 6 units of wood a torso 1000 units away takes 17.5; 12
/// units stop the round.
#[test]
fn t36_penetration() {
    for (thick, want) in [(6.0, 17.5), (12.0, 0.0)] {
        let mut sim = sim();
        let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", 1000.0);
        place_at(&mut sim, shooter, target, "Torso", 1000.0);
        panel(&mut sim, 32.0, thick, "Wood");
        close(shoot_at(&mut sim, shooter, target, "Torso"), want, 1e-3, &format!("{thick} units of wood"));
    }
}

/// 37. Glass then plywood: 35 x 0.9 x 0.6 = 18.9.
#[test]
fn t37_two_panels() {
    let mut sim = sim();
    let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", 1000.0);
    place_at(&mut sim, shooter, target, "Torso", 1000.0);
    panel(&mut sim, 33.0, 6.0, "Glass");
    panel(&mut sim, 31.0, 6.0, "Plywood");
    close(shoot_at(&mut sim, shooter, target, "Torso"), 18.9, 1e-3, "glass and plywood");
}

/// Glass passes a round at 0.9 of its damage; Bullet-Proof Glass (surface
/// 41) isn't in the shoot-through table and stops it, however thin.
#[test]
fn t37_glass_passes_and_bullet_proof_glass_stops() {
    for (surface, want) in [("Glass", 35.0 * 0.9), ("Bullet-Proof Glass", 0.0)] {
        let mut sim = sim();
        let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", 1000.0);
        place_at(&mut sim, shooter, target, "Torso", 1000.0);
        panel(&mut sim, 33.0, 1.0, surface);
        close(shoot_at(&mut sim, shooter, target, "Torso"), want, 1e-3, surface);
    }
}

/// The same for a brush entity's glass (`map::BrushSurface`): another
/// game's window by its name, a Combat Arms world model (a node) by its
/// type's number.
#[test]
fn t37_brush_entities_glass() {
    for (surface, want) in [("glass", 35.0 * 0.9), ("40", 35.0 * 0.9), ("41", 0.0)] {
        let mut sim = sim();
        let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", 1000.0);
        place_at(&mut sim, shooter, target, "Torso", 1000.0);
        sim.app.world_mut().spawn((
            Transform::from_xyz(30.0, 1.5, 33.0),
            RigidBody::Kinematic,
            Collider::cuboid(3.0, 3.0, U),
            chimera_world::map::BrushSurface(surface.into()),
        ));
        sim.ticks(1);
        close(shoot_at(&mut sim, shooter, target, "Torso"), want, 1e-3, surface);
    }
}

// ---------------------------------------------------------------------------
// The follow-up answers (38-49)

/// 38. Recovery at exactly DecDelay (corrects 16): 94 ms waits, 95 ms
/// recovers, and the next sub-step at any time recovers again.
#[test]
fn t38_recovery_at_exactly_dec_delay() {
    let mut sim = sim();
    let (_, w) = m4a1(&mut sim);
    let (sc, mut s) = (scales(&sim), spread(&sim, w));
    let cone = s.cone_params(FireMode::Normal);
    s.fire = 0.30;
    s.last_shot = Some(0.0);
    s.fire_substep(&sc, cone, false, 0.30, 0.094);
    close(s.fire, 0.30, 1e-6, "94 ms");
    s.fire_substep(&sc, cone, false, 0.30, 0.095);
    close(s.fire, 0.28875, 1e-5, "95 ms");
    assert_eq!(s.last_shot, None, "the stamp cleared");
    s.fire_substep(&sc, cone, false, 0.30, 0.0951);
    close(s.fire, 0.2775, 1e-5, "the next sub-step");
}

/// 39. Airborne rise after a jump: the decrease step at the increase
/// speed, capped at JumpPerturb.
#[test]
fn t39_airborne_rise() {
    let mut sim = sim();
    let (_, w) = m4a1(&mut sim);
    let (sc, mut s) = (scales(&sim), spread(&sim, w));
    let (cone, st) = (s.cone_params(FireMode::Normal), s.states(FireMode::Normal));
    s.movement = 0.50;
    s.move_substep(&sc, cone, st.jump, 0.20, 0.0, true, st.jump);
    close(s.movement, 0.64625, 1e-5, "sub-step 1");
    s.move_substep(&sc, cone, st.jump, 0.20, 0.0, true, st.jump);
    close(s.movement, 0.70, 1e-6, "sub-step 2, capped");
}

/// 40. A fall is not a jump: walked off (here placed in the air), falling
/// fast, run flag on: mu = 1, the run target 0.25, and the meter falls
/// from 0.50 as on the ground.
#[test]
fn t40_a_fall_is_not_a_jump() {
    let mut sim = sim();
    let (p, w) = m4a1(&mut sim);
    sim.app.world_mut().get_mut::<Transform>(p).unwrap().translation.y += 20.0;
    sim.app.world_mut().get_mut::<chimera_core::core::Velocity>(p).unwrap().0 = Vec3::NEG_Y * 400.0 * U;
    sim.app.world_mut().get_mut::<movement::CaMovement>(p).unwrap().standing = None;
    {
        let mut s = sim.app.world_mut().get_mut::<CaSpread>(w).unwrap();
        s.movement = 0.50;
        s.clock = 0.0;
    }
    sim.ticks(1);
    let m = sim.app.world().get::<movement::CaMovement>(p).unwrap().clone();
    assert!(!m.in_jump && m.run, "falling, not in a jump, run flag on");
    // 15 ms: one sub-step of 12.5 ms.
    close(sim.app.world().get::<CaSpread>(w).unwrap().movement, 0.46625, 1e-5, "it falls");
}

/// 41. The run flag at rest and crouched: standing still the run target
/// with mu 0 keeps M at 0; crouched and moving the walk target caps it
/// at 0.04 (cone 6).
#[test]
fn t41_run_flag_at_rest_and_crouched() {
    let mut sim = sim();
    let (p, w) = m4a1(&mut sim);
    sim.seconds(1.0);
    assert!(sim.app.world().get::<movement::CaMovement>(p).unwrap().run, "run flag on at rest");
    close(sim.app.world().get::<CaSpread>(w).unwrap().movement, 0.0, 1e-6, "still");
    sim.intent(p).crouch = true;
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(1.0);
    let s = sim.app.world().get::<CaSpread>(w).unwrap().clone();
    assert!(!sim.app.world().get::<movement::CaMovement>(p).unwrap().run, "crouched: run off");
    close(s.movement, 0.04, 1e-5, "WalkPerturb");
    close(s.cone(FireMode::Normal), 6.0, 0.0, "cone");
}

/// 42. The turn term and the twist (corrects 20): with no movement key a
/// 0.2 rad turn from an untwisted body measures 0; beyond the 45 degree
/// twist it measures 0.2 (M to 0); with a strafe key held, 0.2 (M rises).
#[test]
fn t42_turn_and_twist() {
    let mut sim = sim();
    let (_, w) = m4a1(&mut sim);
    let (sc, base) = (scales(&sim), spread(&sim, w));
    let (cone, st) = (base.cone_params(FireMode::Normal), base.states(FireMode::Normal));
    let quarter = std::f32::consts::FRAC_PI_4;
    // Untwisted, standing still.
    let mut s = base.clone();
    s.body_turn(0.0, 0.0, false);
    let turned = s.body_turn(0.2, 0.0, false);
    close(turned, 0.0, 1e-6, "into the twist");
    s.movement = 0.20;
    s.move_substep(&sc, cone, st.run, 0.0, CaSpread::turn(&sc, 0.0, turned), false, st.jump);
    close(s.movement, 0.16625, 1e-5, "no turn measured");
    // The twist at its limit: the same turn further measures 0.2.
    let mut s = base.clone();
    s.body_turn(0.0, 0.0, false);
    close(s.body_turn(quarter, 0.0, false), 0.0, 1e-6, "up to the limit");
    let turned = s.body_turn(quarter + 0.2, 0.0, false);
    close(turned, 0.2, 1e-5, "beyond the twist");
    let rho = CaSpread::turn(&sc, 0.0, turned);
    close(rho, 0.29575, 1e-4, "rho");
    s.movement = 0.20;
    s.move_substep(&sc, cone, st.run, 0.0, rho, false, st.jump);
    close(s.movement, 0.0, 1e-6, "snapped");
    // A strafe key held: no twist.
    let mut s = base;
    s.body_turn(0.0, 0.0, true);
    let rho = CaSpread::turn(&sc, 0.0, s.body_turn(0.2, 0.0, true));
    close(rho, 0.29575, 1e-4, "moving");
    s.movement = 0.20;
    s.move_substep(&sc, cone, st.run, 0.25, rho, false, st.jump);
    close(s.movement, 0.24875, 1e-5, "rises");
}

/// 43. A frame's turn on its sub-steps: all on the first.
#[test]
fn t43_turn_split_over_sub_steps() {
    let sim = sim();
    let sc = scales(&sim);
    let r = ca::substep_turns(&sc, 0.3, 3);
    close(r[0], 0.44362, 1e-4, "sub-step 1");
    assert_eq!(&r[1..], &[0.0, 0.0]);
}

/// 44. The recoil's kick is not a turn (pitch is never measured); its side
/// wag is.
#[test]
fn t44_recoil_kick_is_not_a_turn() {
    let mut sim = sim();
    let (_, w) = m4a1(&mut sim);
    let sc = scales(&sim);
    let mut s = spread(&sim, w);
    s.body_turn(0.0, 0.0, false);
    close(s.body_turn(0.0, 0.0, false), 0.0, 0.0, "a kick of 1.3 degrees up: no change");
    let wag = 1.2f32.to_radians();
    let rho = CaSpread::turn(&sc, 0.0, s.body_turn(0.0, wag, false));
    close(rho, 0.03097, 1e-4, "the side wag");
}

/// 45. mu against the penalised limit: the reference loadout's run limit
/// is 228 (P 0.2), so 114 u/s is mu 0.5, mu S 0.125.
#[test]
fn t45_mu_uses_the_penalised_limit() {
    let mut sim = sim();
    let (p, w) = m4a1(&mut sim);
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(1.0);
    let limit = sim.app.world().get::<movement::CaMovement>(p).unwrap().speed_limit;
    close(limit, 228.0, 1e-3, "the reference loadout's run limit");
    let mu = ca::movement_fraction(114.0, limit);
    close(mu, 0.5, 1e-6, "mu");
    let st = spread(&sim, w).states(FireMode::Normal);
    close(mu * st.run, 0.125, 1e-6, "mu S");
}

/// Two unarmoured victims in line behind each other; the shooter's round
/// hits the first's chest at `units`.
fn in_line(sim: &mut Sim, units: f32, first_yaw: f32) -> (Entity, Entity, Entity) {
    let (shooter, first) = range_lane(sim, "test:m4a1_exact", units);
    face(sim, first, first_yaw);
    place_at(sim, shooter, first, "Torso", units);
    let behind = sim.position(first) - Vec3::Z * 0.6;
    // Facing as the first, so the ray meets its torso as it met the
    // first's (the spec's skeleton holds an arm before the chest).
    let second = player(sim, behind, first_yaw, 100.0);
    let at = sphere(sim, shooter, first, "Torso");
    aim(sim, shooter, at);
    (shooter, first, second)
}

/// One round at what the shooter aims at: what each target lost.
fn one_round(sim: &mut Sim, shooter: Entity, targets: [Entity; 2]) -> [f32; 2] {
    let before = targets.map(|t| health(sim, t));
    sim.intent(shooter).fire = true;
    sim.ticks(1);
    sim.intent(shooter).fire = false;
    sim.ticks(LATENCY + 1);
    let lost = [0, 1].map(|i| (before[i] - health(sim, targets[i])) / HP);
    sim.seconds(0.2);
    lost
}

/// 46. The second victim: banded again at the first hit's distance, with
/// the first's behind bonus; its own armour.
#[test]
fn t46_second_victim() {
    let pi = std::f32::consts::PI;
    for (units, yaw, want) in [(2800.0, pi, [24.5, 17.15]), (2000.0, pi, [35.0, 35.0]), (2000.0, 0.0, [45.5, 45.5])] {
        let mut sim = sim();
        let (shooter, first, second) = in_line(&mut sim, units, yaw);
        let lost = one_round(&mut sim, shooter, [first, second]);
        close(lost[0], want[0], 1e-3, &format!("first at {units}, yaw {yaw}"));
        close(lost[1], want[1], 1e-3, &format!("second at {units}, yaw {yaw}"));
    }
    let mut sim = sim();
    let (shooter, first, second) = in_line(&mut sim, 2000.0, pi);
    set_armour(&mut sim, second, 100.0, vec![DEFAULT_BODY_GEAR]);
    let lost = one_round(&mut sim, shooter, [first, second]);
    close(lost[1], 29.75, 1e-3, "the second's own armour");
    close(armour(&sim, second), 94.75, 1e-3, "its armour points");
}

/// 47. The fire delay's clock: from the last round that passed; a dropped
/// one doesn't restart it; exactly fireDelay passes.
#[test]
fn t47_fire_delay_clock() {
    for (times, dealt) in [([0, 60, 110], [true, false, true]), ([0, 60, 100], [true, false, true]), ([0, 150, 200], [true, true, false])] {
        let mut sim = sim();
        sim.set_tick_interval(0.005);
        sim.app
            .add_weapon_data(&[(
                "spaced",
                r#"
                id = "test:m4a1_spaced"
                base = "test:m4a1_exact"
                [trigger]
                automatic = false
                cycle = 0.01
                [hooks."combat_arms:damage"]
                fire_delay = 100.0
                [hooks."combat_arms:recoil"]
                pitch = 0.0
                wag_r = 0.0
                wag_l = 0.0
                random_wag = 0.0
                "#,
            )])
            .unwrap();
        let (shooter, target) = range_lane(&mut sim, "test:m4a1_spaced", 1000.0);
        sim.app.world_mut().get_mut::<Health>(target).unwrap().current = 10.0;
        let w = sim.app.world().get::<Inventory>(shooter).unwrap().active.unwrap();
        let at = sphere(&sim, shooter, target, "Torso");
        aim(&mut sim, shooter, at);
        let t0 = sim.tick();
        for (i, t) in times.into_iter().enumerate() {
            while (sim.tick() - t0) * 5 < t {
                sim.ticks(1);
            }
            let before = health(&sim, target);
            sim.intent(shooter).fire = true;
            sim.ticks(1);
            sim.intent(shooter).fire = false;
            sim.ticks(LATENCY);
            assert_eq!(fired(&sim, w), i as u32 + 1, "{times:?}: round {} spent", i + 1);
            let lost = (before - health(&sim, target)) / HP;
            close(lost, if dealt[i] { 35.0 } else { 0.0 }, 1e-3, &format!("{times:?}: round {}", i + 1));
        }
    }
}

/// 48. Armour by damage type: burn halved (and used up), melee and
/// explosions not covered, the groin unprotected, bullets on the torso as
/// case 6.
#[test]
fn t48_armour_by_damage_type() {
    use ca::{ArmourHit, Location};
    let hit = |damage_type, location| ArmourHit {
        damage_type,
        location,
        zone: 1.0,
        infected: false,
    };
    for (what, h, d, want, ap) in [
        ("burn", hit(ca::BURN, Location::Torso), 10.0, 5.0, 95.0),
        ("melee", hit(ca::MELEE, Location::Torso), 45.0, 45.0, 100.0),
        ("explosion", hit(ca::EXPLOSION, Location::Torso), 50.0, 50.0, 100.0),
        ("bullet, groin", hit(ca::BULLET, Location::Groin), 35.0, 35.0, 100.0),
        ("bullet, torso", hit(ca::BULLET, Location::Torso), 35.0, 29.75, 94.75),
    ] {
        let (mut a, pieces) = ca::default_armour();
        close(ca::armour_and_zone(d, h, Some(&mut a), Some(&pieces), true), want, 1e-4, what);
        close(ca::points(&a), ap, 1e-4, &format!("{what}: AP"));
    }
    // A round on the groin in play.
    let mut sim = sim();
    let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", 1000.0);
    set_armour(&mut sim, target, 100.0, vec![DEFAULT_BODY_GEAR]);
    close(shoot_at(&mut sim, shooter, target, "Nuts"), 35.0, 1e-3, "Nuts");
    close(armour(&sim, target), 100.0, 1e-3, "no armour used");
}

/// 49. An infected victim's groin counts as torso.
#[test]
fn t49_groin_on_an_infected_victim() {
    let h = ca::ArmourHit {
        damage_type: ca::BULLET,
        location: ca::Location::Groin,
        zone: 1.0,
        infected: true,
    };
    let (mut a, pieces) = ca::default_armour();
    close(ca::armour_and_zone(35.0, h, Some(&mut a), Some(&pieces), true), 29.75, 1e-4, "damage");
    close(ca::points(&a), 94.75, 1e-4, "AP");
}

/// Case 55 of movement.md through a real round: an infected victim of
/// the other team is knocked back by the M4A1's ammo (K 40, 0.3 s); a
/// victim that isn't infected is not.
#[test]
fn knock_back_on_an_infected_victim() {
    use chimera_core::core::Team;
    for infected in [true, false] {
        let mut sim = sim();
        let (shooter, target) = range_lane(&mut sim, "combat_arms:m4a1", 1000.0);
        sim.app.world_mut().entity_mut(shooter).insert(Team(1));
        sim.app.world_mut().entity_mut(target).insert(Team(2));
        if infected {
            sim.app.world_mut().entity_mut(target).insert(movement::CaInfected);
        }
        shoot_at(&mut sim, shooter, target, "Torso");
        let k = sim.app.world().get::<movement::HitKnockback>(target).copied();
        if infected {
            let k = k.expect("knocked back");
            assert_eq!((k.strength, k.duration), (40.0, 0.3));
        } else {
            assert!(k.is_none(), "not infected: nothing");
        }
    }
}

// ---------------------------------------------------------------------------
// Parity pin (docs/DESIGN.md "Hits across games")

/// What the M4A1 (roll 1.0, 1000 units) deals on every zone of the male
/// skeleton through the default body and head gear, each shot on 100
/// fresh armour points: hit points lost and armour left. The code's
/// values before the generic hit path (2026-10-27; cases 4, 6, 7, 48):
/// they must not change.
#[test]
fn parity_pin_zones_through_armour() {
    let mut sim = sim();
    let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", 1000.0);
    for (name, lost, left) in [
        ("Face_Eyelid1", 157.5, 91.25),
        ("Neck", 29.75, 94.75),
        ("Torso", 29.75, 94.75),
        ("Nuts", 35.0, 100.0),
        ("Right_pelvis", 26.775, 94.75),
        ("Right_armu", 26.775, 94.75),
        ("Right_hand", 26.775, 94.75),
        ("Right_Legu", 26.775, 94.75),
        ("Right_legl", 22.3125, 94.75),
        ("Right_foot", 14.875, 94.75),
    ] {
        set_armour(&mut sim, target, 100.0, vec![DEFAULT_BODY_GEAR, DEFAULT_HEAD_GEAR]);
        close(shoot_at(&mut sim, shooter, target, name), lost, 1e-3, name);
        close(armour(&sim, target), left, 1e-3, &format!("{name} armour"));
    }
}
