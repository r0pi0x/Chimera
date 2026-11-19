//! specs/combat_arms/game_modes.md's follow-up cases (63-86: the answers
//! to its open questions 4, 10, 12, 17-36), one test each where our
//! rules play them (`case_63_` is test case 63), on `CaSim` with the
//! greybox and the modes' map objects as `game::world` reads them.

use avian3d::prelude::ColliderAabb;
use bevy::prelude::*;
use chimera_core::core::{DamageKind, Health, Intent, MovementState};
use chimera_sim::weapon::{Inventory, Magazine};
use chimera_world::harness::Sim;
use combat_arms::game::{
    effects::bomb as bomb_fx,
    inventory::{HP, KNIFE},
    missions,
    rules::{
        CaMatch, Phase, bombing_run, hunted, mutation,
        objectives::BOMB,
        objects::{self, ObjBox},
        quarantine, seize, spawns,
    },
    weapons::{self as ca, MALE},
};

use crate::mode_kit::*;

fn site() -> Vec3 {
    Vec3::new(0.0, 1.0, 0.0)
}

/// S&D past its warm-up, site A placed, everyone unprotected.
fn game_sd(jsf: usize, creed: usize) -> (Sim, Vec<Entity>, Vec<Entity>) {
    let mut sim = game("TMM", 12);
    objects(&mut sim).bomb_zones = vec![bomb_zone("Doobic", 0, CREED, site())];
    let a: Vec<Entity> = (0..jsf).map(|_| join(&mut sim, JSF)).collect();
    let b: Vec<Entity> = (0..creed).map(|_| join(&mut sim, CREED)).collect();
    past_warmup(&mut sim);
    unprotect_all(&mut sim);
    sim.ticks(3);
    (sim, a, b)
}

/// 63. The intermission comes after every round: a round decided at t
/// ends 3 s later (bodies cleared), the intermission 3 s after that, then
/// the next round's setup.
#[test]
fn case_63_intermission_after_every_round() {
    let (mut sim, a, b) = game_sd(1, 1);
    let t = now(&sim);
    kill_now(&mut sim, b[0], a[0]);
    until(&mut sim, 1.0, |p| matches!(p, Phase::RoundOver { .. }));
    until(&mut sim, 4.0, |p| matches!(p, Phase::Intermission { .. }));
    let end = now(&sim);
    assert!((end - t - 3.05).abs() < 0.1, "round end at {:.2}", end - t);
    until(&mut sim, 4.0, |p| matches!(p, Phase::Setup { .. } | Phase::Live));
    assert!(
        (now(&sim) - t - 6.05).abs() < 0.15,
        "the next round at {:.2}",
        now(&sim) - t
    );
    assert_eq!(m(&sim).round_wins, [1, 0]);
}

/// 64. The detonation hurts nobody: both players on the site keep their
/// health; JSF wins; the planter's detonations +1; no kill or death; the
/// explosion show's place and time are every machine's.
#[test]
fn case_64_detonation_hurts_nobody() {
    let (mut sim, a, b) = game_sd(1, 1);
    select_bomb(&mut sim, a[0]);
    assert!(wait_for(&mut sim, 1.0, |s| in_hand(s, a[0]) == Some(BOMB)));
    sim.intent(a[0]).fire = true;
    hold_at(&mut sim, a[0], site(), 4.0);
    sim.intent(a[0]).fire = false;
    let planted = objectives(&sim).planted.expect("planted");
    let wait = planted.detonates - now(&sim);
    // Both stand on the site when it goes off.
    let end = now(&sim) + wait + 0.2;
    while now(&sim) < end {
        put(&mut sim, a[0], site());
        put(&mut sim, b[0], site() + Vec3::X);
        sim.ticks(1);
    }
    assert_eq!((health(&sim, a[0]), health(&sim, b[0])), (1.0, 1.0));
    assert_eq!(m(&sim).winner, Some(JSF));
    let s = stats(&sim, a[0]);
    assert_eq!((s.detonations, s.kills, s.deaths, s.suicides), (1, 0, 0, 0));
    assert_eq!(stats(&sim, b[0]).deaths, 0);
    let (at, when) = objectives(&sim).detonation.expect("the show");
    assert!(at.distance(site()) < 1e-3 && (when - planted.detonates).abs() < 0.1);
    assert_eq!(bomb_fx::SHOW.len(), 6);
    assert_eq!((bomb_fx::SHOW[0].0, bomb_fx::SHOW[5].0), (0.4, 5.3));
}

/// 65. The plant is the bomb's fire animation to its key (3.904 s) with
/// the planter's movement locked; let go at 3.0 s, nothing is kept: the
/// next press takes the whole 3.9 s again.
#[test]
fn case_65_plant_timing() {
    let (mut sim, a, _) = game_sd(1, 1);
    select_bomb(&mut sim, a[0]);
    assert!(wait_for(&mut sim, 1.0, |s| in_hand(s, a[0]) == Some(BOMB)));
    sim.intent(a[0]).fire = true;
    hold_at(&mut sim, a[0], site(), 1.0);
    // Movement is locked while it plays.
    sim.intent(a[0]).move_axis = Vec2::Y;
    sim.ticks(1);
    assert_eq!(sim.intent(a[0]).move_axis, Vec2::ZERO, "locked");
    hold_at(&mut sim, a[0], site(), 2.0);
    sim.intent(a[0]).fire = false;
    sim.ticks(2);
    assert!(objectives(&sim).planted.is_none(), "released at 3 s");
    sim.intent(a[0]).move_axis = Vec2::Y;
    sim.ticks(1);
    assert_eq!(sim.intent(a[0]).move_axis, Vec2::Y, "free again");
    sim.intent(a[0]).move_axis = Vec2::ZERO;
    let press = now(&sim);
    sim.intent(a[0]).fire = true;
    while objectives(&sim).planted.is_none() && now(&sim) < press + 5.0 {
        put(&mut sim, a[0], site());
        sim.ticks(1);
    }
    let took = now(&sim) - press;
    assert!((took - 3.904).abs() < 0.05, "planted after {took:.3} s");
}

/// 66. A partial defuse keeps nothing: 3 s held, let go, then 7 s more.
#[test]
fn case_66_partial_defuse() {
    let (mut sim, a, b) = game_sd(1, 1);
    select_bomb(&mut sim, a[0]);
    assert!(wait_for(&mut sim, 1.0, |s| in_hand(s, a[0]) == Some(BOMB)));
    sim.intent(a[0]).fire = true;
    hold_at(&mut sim, a[0], site(), 4.0);
    sim.intent(a[0]).fire = false;
    put(&mut sim, a[0], Vec3::new(10.0, 1.0, 10.0));
    assert!(objectives(&sim).planted.is_some());
    sim.intent(b[0]).use_key = true;
    hold_at(&mut sim, b[0], site(), 3.0);
    sim.intent(b[0]).use_key = false;
    hold_at(&mut sim, b[0], site(), 1.0);
    sim.intent(b[0]).use_key = true;
    hold_at(&mut sim, b[0], site(), 6.5);
    assert!(objectives(&sim).planted.is_some(), "not at 6.5 s of the second try");
    hold_at(&mut sim, b[0], site(), 0.6);
    assert!(objectives(&sim).planted.is_none(), "defused 7 s after the second press");
    assert_eq!(m(&sim).winner, Some(CREED));
    assert!(objectives(&sim).defused.is_some(), "its sound's place");
}

/// 67. The countdown's beeps by the time left.
#[test]
fn case_67_bomb_beeps() {
    assert_eq!(bomb_fx::beep(25.99), Some("UI_SND\\T9COUNT2.wav"));
    assert_eq!(bomb_fx::beep(5.99), Some("UI_SND\\T9COUNT4.wav"));
    assert_eq!(bomb_fx::beep(4.99), Some("UI_SND\\T9COUNT5.wav"));
    assert_eq!(bomb_fx::beep(35.0), Some("UI_SND\\T9COUNT1.wav"));
    assert_eq!(bomb_fx::beep(15.0), Some("UI_SND\\T9COUNT3.wav"));
    assert_eq!(bomb_fx::beep(0.0), None);
}

/// 68. Bombing Run and Seize and Secure spawn on their own families.
#[test]
fn case_68_spawn_families() {
    assert_eq!(bombing_run::MODE.spawns.family, Some("Sabotage"));
    assert_eq!(seize::MODE.spawns.family, Some("HeadQuaters"));
    // Bombing Run on its points.
    let mut sim = game("SBT", 12);
    let sab = Vec3::new(-6.0, 1.0, 4.0);
    let tdm = Vec3::new(6.0, 1.0, 4.0);
    objects(&mut sim).starts = vec![
        start("Team Death Match", Some(JSF), tdm),
        start("Sabotage", Some(JSF), sab),
    ];
    let p = join(&mut sim, JSF);
    join(&mut sim, CREED);
    until_live(&mut sim);
    assert!(at(&sim, p).xz().distance(sab.xz()) < 0.05, "at {}", at(&sim, p));
}

/// 69. Seize and Secure's flag pole is the `FlagPolePos` object's place.
#[test]
fn case_69_flag_pole() {
    use chimera_world::map::MapEntity;
    let e = MapEntity {
        keyvalues: vec![
            ("classname".into(), "GameBase".into()),
            ("targetname".into(), "FlagPolePos".into()),
            ("origin".into(), "279 -79 -1466".into()),
        ],
        ..default()
    };
    let o = objects::read(&[e], 1.0);
    assert!(o.flag_pole.is_some());
}

/// 70. Arms Race at G 30 (our copy of the default ladder): 7 kills with
/// r 50 or 79 the Kriss (337), r 80 your own; 8 kills no step; G 15's 14
/// kills the frag and the last stage.
#[test]
fn case_70_arms_race_table() {
    let mut sim = game("VWRM", 2);
    settings(&mut sim).goal = 30;
    let l = missions::ladders(sim.app.world());
    assert_eq!(missions::ladder_pick(&l, &[], 7, 50, 50, true), Some(337));
    assert_eq!(missions::ladder_pick(&l, &[], 7, 79, 50, true), Some(337));
    assert_eq!(missions::ladder_pick(&l, &[], 7, 80, 50, true), None);
    assert_eq!(missions::ladder_pick(&l, &[], 8, 0, 50, true), None, "no step at 8");
    settings(&mut sim).goal = 15;
    let l = missions::ladders(sim.app.world());
    assert_eq!(missions::ladder_pick(&l, &[], 14, 0, 50, true), Some(84));
    assert_eq!(l[0].next_kill(14), None, "the last stage");
    assert_eq!(
        missions::ladder_weapon(sim.app.world(), 84),
        Some("combat_arms:m67"),
        "our stand-in"
    );
}

/// 72. The VIP counts by team size.
#[test]
fn case_72_vip_counts() {
    let v = missions::VipData::stock();
    for g in [100, 140, 180] {
        let n: Vec<usize> = [1, 2, 4, 5, 7, 8, 12].iter().map(|&s| v.vips(g, s)).collect();
        assert_eq!(n, vec![0, 1, 1, 2, 2, 3, 3], "G {g}");
    }
    let n: Vec<usize> = [5, 6, 8].iter().map(|&s| v.vips(60, s)).collect();
    assert_eq!(n, vec![0, 1, 2]);
}

/// 73. Spy Hunt's stage 2: the camera hold (5 s), then the round timer at
/// `Stage2PlayTime` (180 s); a living hunter refilled (health, every
/// magazine, armour), a dead one respawned; the time-out goes to the
/// hunters.
#[test]
fn case_73_spy_hunt_stage_two() {
    let mut sim = game("BNT", 12);
    settings(&mut sim).objectives.intel_count = 1;
    let intel = Vec3::new(4.0, 1.0, 4.0);
    objects(&mut sim).drops = vec![drop_point("BountyHunter", intel)];
    let p: Vec<Entity> = (0..3).map(|_| join(&mut sim, JSF)).collect();
    past_warmup(&mut sim);
    unprotect_all(&mut sim);
    sim.ticks(3);
    // (Hunters are teammates: a fall.)
    hit(&mut sim, p[2], None, 2.0, DamageKind::Generic, None);
    sim.ticks(1);
    assert!(dead(&sim, p[2]));
    let w = sim.app.world().get::<Inventory>(p[0]).unwrap().active.unwrap();
    sim.app.world_mut().get_mut::<Magazine>(w).unwrap().clip = 0;
    sim.app.world_mut().get_mut::<Health>(p[0]).unwrap().current = 0.3;
    sim.app
        .world_mut()
        .get_mut::<chimera_sim::weapon::Armor>(p[0])
        .unwrap()
        .amount = 0.0 / 100.0;
    hold_at(&mut sim, p[1], intel, 0.1);
    let t = now(&sim);
    assert_eq!(objectives(&sim).stage, 2);
    let hold = objectives(&sim).camera_until.expect("the camera hold");
    assert!((hold - t - 5.0).abs() < 0.1);
    let ends = m(&sim).round_ends.unwrap();
    assert!((ends - t - 185.0).abs() < 0.1, "{:.2}", ends - t);
    assert_eq!(health(&sim, p[0]), 1.0);
    let mag = sim.app.world().get::<Magazine>(w).unwrap();
    assert_eq!((mag.clip, mag.reserve), (mag.size, mag.reserve_max));
    assert!(
        sim.app
            .world()
            .get::<chimera_sim::weapon::Armor>(p[0])
            .map(combat_arms::game::weapons::points)
            .unwrap()
            > 0.0,
        "armour back"
    );
    assert!(!dead(&sim, p[2]), "the dead respawned");
    sim.seconds(5.1);
    assert!(objectives(&sim).camera_until.is_none());
    until(&mut sim, 185.0, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).winner, Some(JSF), "the hunters on a time-out");
}

/// Quarantine past its warm-up and its host selection (20 s): the hosts
/// and the humans.
fn quarantine_live(n: usize) -> (Sim, Vec<Entity>, Vec<Entity>) {
    let mut sim = game("QRT", 12);
    let p: Vec<Entity> = (0..n).map(|_| join(&mut sim, JSF)).collect();
    until_live(&mut sim);
    sim.seconds(1.0);
    assert!(
        p.iter().all(|e| team(&sim, *e) == JSF),
        "nobody infected in the warm-up"
    );
    past_warmup(&mut sim);
    unprotect_all(&mut sim);
    sim.seconds(20.2);
    let (hosts, humans): (Vec<Entity>, Vec<Entity>) = p.iter().partition(|e| team(&sim, **e) == CREED);
    (sim, hosts, humans)
}

/// 74. A human killed by an infected is back at once where it died, on
/// Bravo, at full health, holding the infected's weapon (our knife).
#[test]
fn case_74_infection_in_place() {
    let (mut sim, hosts, humans) = quarantine_live(5);
    assert_eq!(hosts.len(), 2);
    let victim = humans[0];
    let spot = at(&sim, victim);
    kill(&mut sim, victim, hosts[0]);
    sim.ticks(1);
    assert_eq!(team(&sim, victim), CREED);
    assert!(!dead(&sim, victim));
    assert_eq!(health(&sim, victim), 1.0);
    assert!(at(&sim, victim).xz().distance(spot.xz()) < 0.05, "where it died");
    assert!(
        wait_for(&mut sim, 1.0, |s| weapons(s, victim) == vec![KNIFE]),
        "{:?}",
        weapons(&sim, victim)
    );
    assert_eq!(stats(&sim, hosts[0]).mission_points, 2);
}

/// 75. The infected's regeneration: 10 % of the maximum a second after
/// 3 s standing still, the first at once; a hit restarts the wait.
#[test]
fn case_75_infected_regeneration() {
    let (mut sim, hosts, _) = quarantine_live(5);
    let h = hosts[0];
    let spot = at(&sim, h);
    sim.app.world_mut().get_mut::<Health>(h).unwrap().current = 0.4;
    let t0 = now(&sim);
    let mut seen = Vec::new();
    let mut last = 0.4;
    while now(&sim) < t0 + 5.2 {
        put(&mut sim, h, spot);
        sim.ticks(1);
        let now_h = health(&sim, h);
        if now_h > last + 1e-6 {
            seen.push(((now(&sim) - t0) * 10.0).round() / 10.0);
        }
        last = now_h;
    }
    assert_eq!(seen.len(), 3, "{seen:?}");
    assert!((seen[0] - 3.0).abs() < 0.15 && (seen[2] - 5.0).abs() < 0.15, "{seen:?}");
    assert!((health(&sim, h) - 0.7).abs() < 1e-4, "+10 each: {}", health(&sim, h));
    // A hit restarts the wait.
    hit(&mut sim, h, None, 0.05, DamageKind::Generic, None);
    put(&mut sim, h, spot);
    sim.ticks(1);
    let hit_at = now(&sim);
    let before = health(&sim, h);
    while now(&sim) < hit_at + 2.8 {
        put(&mut sim, h, spot);
        sim.ticks(1);
    }
    assert!(health(&sim, h) <= before + 1e-6, "no heal within 3 s of the hit");
    while now(&sim) < hit_at + 3.2 {
        put(&mut sim, h, spot);
        sim.ticks(1);
    }
    assert!(health(&sim, h) > before + 0.05, "healing again");
}

/// 76. The mutation box's pick by condition.
#[test]
fn case_76_mutation_pick() {
    let sim = game("TDM", 2);
    let w = &sim
        .app
        .world()
        .resource::<combat_arms::game::rules::CaRuleSettings>()
        .objectives
        .mutation_weights;
    assert_eq!(mutation::condition_tdm(-20, 0, 100), 0, "behind by 20 %");
    assert_eq!(mutation::condition_tdm(20, 0, 100), 2);
    assert_eq!(mutation::condition_tdm(5, 0, 100), 1);
    assert_eq!(mutation::total(w, 0), 80);
    assert_eq!(mutation::pick(w, 0, 0), Some(0));
    assert_eq!(mutation::pick(w, 0, 9), Some(0), "item 0: 10 of 80");
    assert_eq!(w[7], [5, 10, 20], "item 7, invulnerable: 5 of 80, 20 when ahead");
    assert_eq!(mutation::condition_ffa(9, 11), 0);
    assert_eq!(mutation::condition_ffa(47, 53), 1);
    assert_eq!(mutation::condition_ffa(1, 1), 2);
}

/// The eye of `p` and the centre of its `Upper_torso` sphere as `from`
/// sees it.
fn eye(sim: &Sim, p: Entity) -> Vec3 {
    let w = sim.app.world();
    w.get::<Transform>(p).unwrap().translation + w.get::<MovementState>(p).unwrap().eye_offset
}

fn chest(sim: &Sim, target: Entity) -> Vec3 {
    let w = sim.app.world();
    let (_, _, spheres) = ca::body_geometry(
        w.get::<Transform>(target).unwrap(),
        w.get::<Intent>(target).unwrap(),
        w.get::<MovementState>(target).unwrap(),
        w.get::<ColliderAabb>(target),
        *MALE,
    )
    .unwrap();
    let i = MALE.iter().position(|s| s.name == "Upper_torso").unwrap();
    spheres[i].0
}

/// One M4A1 round from `shooter` at `target`'s chest: (hit points lost,
/// armour points used).
fn round_at(sim: &mut Sim, shooter: Entity, target: Entity) -> (f32, f32) {
    let d = chest(sim, target) - eye(sim, shooter);
    {
        let mut i = sim.intent(shooter);
        i.yaw = (-d.x).atan2(-d.z);
        i.pitch = d.y.atan2(d.xz().length());
    }
    sim.app.world_mut().get_mut::<Health>(target).unwrap().current = 1.0;
    let armour = sim
        .app
        .world()
        .get::<chimera_sim::weapon::Armor>(target)
        .map(combat_arms::game::weapons::points)
        .unwrap();
    sim.intent(shooter).fire = true;
    sim.ticks(1);
    sim.intent(shooter).fire = false;
    // The round goes the trigger's latency (3 updates) after the press.
    sim.ticks(4);
    let lost = (1.0 - health(sim, target)) / HP;
    let used = armour
        - sim
            .app
            .world()
            .get::<chimera_sim::weapon::Armor>(target)
            .map(combat_arms::game::weapons::points)
            .unwrap();
    sim.seconds(0.3);
    (lost, used)
}

/// 77. Friendly fire scales a teammate's round before armour and the zone
/// factor: a teammate's chest hit costs a quarter of an enemy's in health
/// and in armour.
#[test]
fn case_77_friendly_fire_before_armour() {
    let mut sim = game("TDM", 12);
    settings(&mut sim).friendly_fire = 1;
    let a = join(&mut sim, JSF);
    let mate = join(&mut sim, JSF);
    let enemy = join(&mut sim, CREED);
    until_live(&mut sim);
    unprotect_all(&mut sim);
    sim.seconds(1.5);
    let spot = Vec3::new(0.0, 1.0, -4.0);
    put(&mut sim, a, Vec3::new(0.0, 1.0, 0.0));
    put(&mut sim, enemy, Vec3::new(20.0, 1.0, 20.0));
    put(&mut sim, mate, spot);
    sim.ticks(2);
    let (mate_lost, mate_used) = round_at(&mut sim, a, mate);
    put(&mut sim, mate, Vec3::new(-20.0, 1.0, 20.0));
    put(&mut sim, enemy, spot);
    sim.ticks(2);
    let (enemy_lost, enemy_used) = round_at(&mut sim, a, enemy);
    assert!(enemy_lost > 0.0 && enemy_used > 0.0, "{enemy_lost} {enemy_used}");
    let ratio = mate_lost / enemy_lost;
    assert!((ratio - 0.25).abs() < 0.03, "health: {mate_lost} vs {enemy_lost}");
    let ratio = mate_used / enemy_used;
    assert!((ratio - 0.25).abs() < 0.03, "armour: {mate_used} vs {enemy_used}");
}

/// 79. CTF counts a capture made in the round-over hold (Quirk 13).
#[test]
fn case_79_capture_in_round_over() {
    let mut sim = game("CTF", 12);
    let home_a = Vec3::new(-8.0, 1.0, 8.0);
    let home_b = Vec3::new(8.0, 1.0, -8.0);
    let (fa, za) = flag_and_zone(JSF, home_a);
    let (fb, zb) = flag_and_zone(CREED, home_b);
    {
        let mut o = objects(&mut sim);
        o.flags = vec![fa, fb];
        o.flag_zones = vec![za, zb];
    }
    let a = join(&mut sim, JSF);
    join(&mut sim, CREED);
    until_live(&mut sim);
    unprotect_all(&mut sim);
    hold_at(&mut sim, a, home_b, 0.2);
    assert_eq!(carry(&sim, a).flag, Some(CREED));
    // The time runs out: Alpha's carrier captures in the hold.
    let t = now(&sim);
    sim.app.world_mut().resource_mut::<CaMatch>().round_ends = Some(t + 0.1);
    until(&mut sim, 1.0, |p| matches!(p, Phase::RoundOver { .. }));
    hold_at(&mut sim, a, home_a, 0.2);
    assert_eq!(m(&sim).score[0], 1, "the capture counted");
    assert_eq!(stats(&sim, a).captures, 1);
}

/// 81. Whom the dead watch: teammates first, anyone once none lives
/// (force 2) in the team round modes; anyone in Last Man Standing.
#[test]
fn case_81_spectating() {
    for (key, force) in [("TSV", 2), ("TMM", 2), ("LMS", 0), ("HG", 0)] {
        let mut sim = game(key, 2);
        sim.ticks(2);
        assert_eq!(
            sim.app
                .world()
                .resource::<chimera_core::core::SpectatorRules>()
                .force_camera,
            force,
            "{key}"
        );
    }
}

/// 84. Hunted's start: the fist only (our knife without the install).
#[test]
fn case_84_hunted_start() {
    let mut sim = game("HG", 12);
    assert_eq!(
        hunted::start_weapon(&combat_arms::game::data::CaData::builtin()),
        KNIFE,
        "without the install"
    );
    let p = join(&mut sim, JSF);
    join(&mut sim, JSF);
    until_live(&mut sim);
    assert!(
        wait_for(&mut sim, 1.0, |s| weapons(s, p) == vec![KNIFE]),
        "{:?}",
        weapons(&sim, p)
    );
}

/// 85. The occupied test with a crouched player's box.
#[test]
fn case_85_occupied_crouched() {
    let crouched = Vec3::new(15.0, 40.0, 15.0);
    assert!(spawns::occupied_by(Vec3::new(0.0, 79.9, 0.0), crouched));
    assert!(!spawns::occupied_by(Vec3::new(0.0, 80.0, 0.0), crouched));
    assert!(spawns::occupied(Vec3::new(0.0, 99.9, 0.0)), "standing: 100");
}

/// 86. `Dims` are half-extents: the plant test's widened box.
#[test]
fn case_86_box_half_extents() {
    let zone = ObjBox {
        centre: Vec3::new(284.7, 0.0, 0.0),
        half: Vec3::new(133.5, 64.0, 123.5),
    };
    assert!(zone.contains(Vec3::new(468.1, 0.0, 0.0), 50.0));
    assert!(!zone.contains(Vec3::new(468.3, 0.0, 0.0), 50.0));
}

/// Quarantine's numbers without the install: ours (`missions::quarantine`).
#[test]
fn quarantine_numbers_fall_back_to_ours() {
    let sim = game("QRT", 2);
    let n = missions::quarantine(sim.app.world());
    assert_eq!((n.min_hosts, n.heal, n.heal_wait, n.heal_period), (2, 0.1, 3.0, 1.0));
    assert!(n.infected_weapons.is_empty(), "our knife");
    assert_eq!(quarantine::hosts(8, n.selection_rate, n.min_hosts), 2);
}
