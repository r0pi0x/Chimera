//! TF2's stock melee weapons on the headless harness (`harness::Tf2Sim`,
//! the greybox), specs/tf2/weapons_stock.md's test cases (their numbers in
//! the test names): each weapon's damage, the smack delay (14 ticks; the
//! Knife's 7), the swing interval held, forced crits, the reach, a target
//! leaving and a turn onto one during the delay, the Knife's backstab (its
//! three angle conditions, 6 x the victim's current health, the credit
//! and the raised knife), the Fists' two hands, the deploy, the swing
//! sound alone on a miss, a switch cancelling the smack. Targets' health
//! raised so a hit's whole damage shows.

use bevy::prelude::*;
use chimera_core::core::{Damage, Health, Intent};
use chimera_sim::{
    testkit,
    weapon::{Backstab, Inventory, Weapon, WeaponEventKind, give, view_anim::ViewPose},
};
use chimera_world::{greybox, map::PlaySound};
use tf2::game::weapons::{
    self, BAT, BONESAW, BOTTLE, FIREAXE, FISTS, KNIFE, KUKRI, MELEE, SHOVEL, WRENCH,
    melee::{self, MeleeCrits, Tf2Melee, crit_chance},
};

use super::weapons::{health_points, holding, run, sim, target};

const U: f32 = 0.0254;

/// The ticks a swing started on.
fn started(events: &[(u64, WeaponEventKind)]) -> Vec<u64> {
    events.iter().filter(|(_, k)| matches!(k, WeaponEventKind::SwingStarted { .. })).map(|(t, _)| *t).collect()
}

/// The ticks something was hit on, and the damage (points).
fn hits(events: &[(u64, WeaponEventKind)]) -> Vec<(u64, f32)> {
    events
        .iter()
        .filter_map(|(t, k)| match k {
            WeaponEventKind::Hit { amount, .. } => Some((*t, (amount * 100.0).round())),
            _ => None,
        })
        .collect()
}

fn actions(events: &[(u64, WeaponEventKind)]) -> Vec<&'static str> {
    events
        .iter()
        .filter_map(|(_, k)| match k {
            WeaponEventKind::Action(a) => Some(*a),
            _ => None,
        })
        .collect()
}

/// `run`, and every message `M` written meanwhile (read each tick: they
/// last two updates), as `keep` makes it.
fn run_with<M: Message, T>(
    sim: &mut Sim,
    p: Entity,
    n: u64,
    mut input: impl FnMut(&mut Sim, u64),
    keep: impl Fn(&M) -> T,
) -> (Vec<(u64, WeaponEventKind)>, Vec<T>) {
    let mut cursor = sim.app.world().resource::<Messages<M>>().get_cursor_current();
    let mut out = Vec::new();
    let mut events = Vec::new();
    for k in 0..n {
        let ev = run(sim, p, 1, |s, _| input(s, k));
        events.extend(ev.into_iter().map(|(_, e)| (k, e)));
        out.extend(cursor.read(sim.app.world().resource::<Messages<M>>()).map(&keep));
    }
    (events, out)
}

fn force_crit(sim: &mut Sim, w: Entity, crit: bool) {
    sim.app.world_mut().get_mut::<Tf2Melee>(w).unwrap().force_crit = Some(crit);
}

use chimera_world::harness::Sim;

/// One press of attack at a target `units` ahead (centre to centre): what
/// the player's weapon did.
fn one_swing(id: &str, units: f32, crit: Option<bool>) -> Vec<(u64, WeaponEventKind)> {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, id);
    if let Some(c) = crit {
        force_crit(&mut sim, w, c);
    }
    let t = target(&mut sim, from, units);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    run(&mut sim, p, 30, |s, k| s.intent(p).fire = k == 0)
}

#[test]
fn bat_35_after_14_ticks_1() {
    let ev = one_swing(BAT, 50.0, Some(false));
    assert_eq!(started(&ev), [0]);
    assert_eq!(hits(&ev), [(14, 35.0)], "{ev:?}");
}

#[test]
fn the_65_damage_weapons_2() {
    for id in [SHOVEL, FIREAXE, BOTTLE, FISTS, WRENCH, BONESAW, KUKRI] {
        let ev = one_swing(id, 50.0, Some(false));
        assert_eq!(hits(&ev), [(14, 65.0)], "{id}: {ev:?}");
    }
}

#[test]
fn knife_40_after_7_ticks_3() {
    // The victim faces the Spy (yaw 180 degrees from the Spy's view).
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, KNIFE);
    let t = target(&mut sim, from, 50.0);
    sim.intent(t).yaw = std::f32::consts::PI;
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    let ev = run(&mut sim, p, 20, |s, k| s.intent(p).fire = k == 0);
    assert_eq!(hits(&ev), [(7, 40.0)], "{ev:?}");
}

/// Held, a swing every interval from the tick of the last (weapons_stock.md
/// 2: next attack = now + interval; the guns' timing as the SDK checked it,
/// weapons.md's corrections: from the firing tick): 0, 34, 68, 102 and 0,
/// 54, 108. The spec's cases 4 and 5 give 0, 34, 67, 100 and 0, 54, 107,
/// as if the interval counted from when the swing was due; its own rule
/// doesn't make those (a draft's arithmetic; for its probe check).
#[test]
fn held_attack_swings_every_interval_4_5() {
    for (id, want) in [(BAT, vec![0, 34, 68, 102]), (SHOVEL, vec![0, 54, 108])] {
        let mut sim = sim();
        let (p, _) = holding(&mut sim, greybox::SPAWNS[0], id);
        let ev = run(&mut sim, p, 134, |s, _| s.intent(p).fire = true);
        assert_eq!(started(&ev), want, "{id}");
    }
}

#[test]
fn forced_crits_6_7() {
    let ev = one_swing(BAT, 50.0, Some(true));
    assert_eq!(hits(&ev), [(14, 105.0)]);
    assert_eq!(actions(&ev), [melee::SWING_CRIT]);
    let ev = one_swing(SHOVEL, 50.0, Some(true));
    assert_eq!(hits(&ev), [(14, 195.0)]);
}

/// The crit swing's start plays the crit swing sound (the script's
/// `burst`), a plain one the swing sound; a miss plays nothing more (32).
#[test]
fn swing_sounds_6_32() {
    for (crit, want) in [(true, "Weapon_Bat.MissCrit"), (false, "Weapon_Bat.Miss")] {
        let mut sim = sim();
        let (p, w) = holding(&mut sim, greybox::SPAWNS[0], BAT);
        force_crit(&mut sim, w, crit);
        let (_, played) = run_with::<PlaySound, _>(&mut sim, p, 30, |s, k| s.intent(p).fire = k == 0, |s| s.entry.clone());
        let melee: Vec<&String> = played.iter().filter(|s| s.starts_with("Weapon_Bat")).collect();
        assert_eq!(melee, [want], "{played:?}");
    }
}

#[test]
fn reach_8() {
    // The gap from the eye to the victim's box face along the view: the
    // box is 48 wide, so the centre is 24 farther.
    for (gap, hit) in [(30.0, true), (47.0, true), (70.0, false)] {
        let ev = one_swing(BOTTLE, gap + 24.0, Some(false));
        assert_eq!(!hits(&ev).is_empty(), hit, "gap {gap}: {ev:?}");
    }
}

#[test]
fn a_target_leaving_during_the_delay_is_missed_9() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, SHOVEL);
    let t = target(&mut sim, from, 50.0);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    let ev = run(&mut sim, p, 30, |s, k| {
        s.intent(p).fire = k == 0;
        if k == 3 {
            s.app.world_mut().get_mut::<Transform>(t).unwrap().translation = from - Vec3::Z * 100.0 * U;
        }
    });
    assert!(hits(&ev).is_empty(), "{ev:?}");
    assert!(ev.iter().any(|(t, k)| *t == 14 && matches!(k, WeaponEventKind::Swing { hit: false, .. })), "{ev:?}");
}

#[test]
fn turning_onto_a_target_during_the_delay_hits_10() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, SHOVEL);
    // The target to the side (+X), the swing started facing -Z.
    let t = sim.spawn_character(from + Vec3::X * 50.0 * U, tf2::game::movement::ID);
    sim.seconds(1.0);
    sim.app.world_mut().get_mut::<Health>(t).unwrap().current = 10.0;
    sim.intent(p).yaw = 0.0;
    sim.intent(p).pitch = 0.0;
    let at = sim.position(t);
    let ev = run(&mut sim, p, 30, |s, k| {
        s.intent(p).fire = k == 0;
        if k == 5 {
            testkit::aim_at(s, p, at);
        }
    });
    assert_eq!(hits(&ev), [(14, 65.0)], "{ev:?}");
}

#[test]
fn melee_crit_chance_11() {
    for (d, want) in [(0.0, 0.15), (200.0, 0.2625), (400.0, 0.375), (800.0, 0.6), (2000.0, 0.6)] {
        assert!((crit_chance(d) - want).abs() < 1e-6, "{d}: {}", crit_chance(d));
    }
}

/// With random crits on, the Shovel's swings crit at about the base rate;
/// the Knife's never do (12).
#[test]
fn the_knife_never_rolls_a_crit_12() {
    for (id, crits) in [(KNIFE, false), (SHOVEL, true)] {
        let mut sim = sim();
        sim.app.insert_resource(MeleeCrits { random: true });
        let from = greybox::SPAWNS[0];
        let (p, _) = holding(&mut sim, from, id);
        let t = target(&mut sim, from, 50.0);
        sim.intent(t).yaw = std::f32::consts::PI;
        sim.app.world_mut().get_mut::<Health>(t).unwrap().current = 1000.0;
        let at = sim.position(t);
        testkit::aim_at(&mut sim, p, at);
        let ev = run(&mut sim, p, 54 * 60, |s, _| s.intent(p).fire = true);
        let h = hits(&ev);
        assert!(h.len() >= 55, "{id}: {}", h.len());
        let base = if id == KNIFE { 40.0 } else { 65.0 };
        let crit = h.iter().filter(|(_, d)| *d > base).count();
        assert!(h.iter().all(|(_, d)| *d == base || *d == base * 3.0), "{id}: {h:?}");
        if crits {
            assert!((3..=20).contains(&crit), "{id}: {crit} of {} crits", h.len());
        } else {
            assert_eq!(crit, 0, "{id}");
        }
    }
}

/// The yaw facing `dir` (flat).
fn yaw_of(dir: Vec3) -> f32 {
    (-dir.x).atan2(-dir.z)
}

fn facing(yaw: f32) -> Vec3 {
    Intent { yaw, ..default() }.yaw_rotation() * Vec3::NEG_Z
}

/// A Spy at the spawn facing -Z, its victim 50 units away (boxes 2 apart; the spec says 40, centres closer than the boxes allow) along `d` (the
/// direction from the Spy to it) facing `v`, with `hp` points: the damage
/// one stab dealt and whether it killed with the backstab's credit.
fn stab(d: Vec3, v: Vec3, hp: f32, pitch_down: bool) -> (f32, bool) {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, KNIFE);
    let t = sim.spawn_character(from + d.normalize() * 50.0 * U, tf2::game::movement::ID);
    sim.seconds(1.0);
    sim.app.world_mut().get_mut::<Health>(t).unwrap().current = hp / 100.0;
    sim.intent(t).yaw = yaw_of(v);
    sim.intent(p).yaw = 0.0;
    sim.intent(p).pitch = 0.0;
    if pitch_down {
        let feet = sim.position(t);
        testkit::aim_at(&mut sim, p, feet);
        sim.intent(p).yaw = 0.0;
    }
    let (ev, credits) = run_with::<Damage, _>(&mut sim, p, 20, |s, k| s.intent(p).fire = k == 0, |d| d.weapon);
    let credited = credits.contains(&Some(melee::BACKSTAB));
    let dealt = hits(&ev).first().map_or(0.0, |h| h.1);
    let dead = health_points(&sim, t) <= 0.0;
    (dealt, credited && dead)
}

#[test]
fn backstab_from_behind_13_14_15_20_21() {
    let behind = Vec3::NEG_Z;
    // 13: 6 x 125, a kill credited to the backstab.
    assert_eq!(stab(behind, behind, 125.0, false), (750.0, true));
    // 14, 15: 6 x the current health.
    assert_eq!(stab(behind, behind, 37.0, false), (222.0, true));
    assert_eq!(stab(behind, behind, 300.0, false), (1800.0, true));
    // 20: looking down (pitch never matters).
    assert_eq!(stab(behind, behind, 125.0, true), (750.0, true));
}

#[test]
fn backstab_angles_16_17() {
    // The victim turned 100 degrees from the Spy's view (a.v = cos 100),
    // the Spy behind it, 30 degrees off its line: a backstab; at 110
    // degrees (35 off) not.
    let (a, at) = (facing(0.0), |deg: f32| facing(deg.to_radians()));
    assert!((a.dot(at(100.0)) - 100f32.to_radians().cos()).abs() < 1e-5);
    assert_eq!(stab(at(30.0), at(100.0), 125.0, false), (750.0, true));
    assert_eq!(stab(at(35.0), at(110.0), 125.0, false), (40.0, false));
}

/// The rule's edges on exact vectors (18, 19): beside (d.v = 0) is no
/// backstab, nor 61 degrees off the line (a.d = 0.485).
#[test]
fn backstab_rule_edges_18_19() {
    let rule = Backstab::Behind { behind: 0.0, facing: 0.5, same_way: -0.3, health: 6.0, credit: None };
    let ahead = Vec3::NEG_Z;
    assert!(rule.behind(ahead, ahead, ahead), "13");
    assert!(!rule.behind(ahead, Vec3::X, ahead), "18: d.v = 0");
    let off = Quat::from_rotation_y(61f32.to_radians()) * ahead;
    assert!((ahead.dot(off) - 0.485).abs() < 1e-3);
    assert!(!rule.behind(ahead, off, off), "19: a.d = 0.485");
    // Pitch is flattened away.
    let down = (ahead + Vec3::NEG_Y * 1.7).normalize();
    assert!(rule.behind(down, ahead, ahead), "20");
}

#[test]
fn the_knife_raises_behind_a_victim_22() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, KNIFE);
    // A victim far ahead facing away, then the Spy right behind it, then
    // far again.
    let t = sim.spawn_character(from - Vec3::Z * 300.0 * U, tf2::game::movement::ID);
    sim.seconds(1.0);
    sim.intent(t).yaw = 0.0;
    sim.intent(p).yaw = 0.0;
    let pose = |s: &Sim| s.app.world().get::<ViewPose>(w).unwrap().0;
    let ev = run(&mut sim, p, 10, |_, _| {});
    assert!(actions(&ev).is_empty());
    assert_eq!(pose(&sim), 0);
    sim.app.world_mut().get_mut::<Transform>(t).unwrap().translation = from - Vec3::Z * 50.0 * U;
    let ev = run(&mut sim, p, 10, |_, _| {});
    assert_eq!(actions(&ev), [melee::BACKSTAB_UP]);
    assert_eq!(pose(&sim), 1);
    sim.app.world_mut().get_mut::<Transform>(t).unwrap().translation = from - Vec3::Z * 300.0 * U;
    let ev = run(&mut sim, p, 10, |_, _| {});
    assert_eq!(actions(&ev), [melee::BACKSTAB_DOWN]);
    assert_eq!(pose(&sim), 0);
}

#[test]
fn fists_left_and_right_23_24_25_26() {
    let setup = |crit: bool| {
        let mut sim = sim();
        let from = greybox::SPAWNS[0];
        let (p, w) = holding(&mut sim, from, FISTS);
        force_crit(&mut sim, w, crit);
        let t = target(&mut sim, from, 50.0);
        let at = sim.position(t);
        testkit::aim_at(&mut sim, p, at);
        (sim, p)
    };
    // 23: attack, the left fist.
    let (mut sim, p) = setup(false);
    let ev = run(&mut sim, p, 30, |s, k| s.intent(p).fire = k == 0);
    assert_eq!(actions(&ev), [melee::SWING]);
    assert!(ev.iter().any(|(_, k)| matches!(k, WeaponEventKind::SwingStarted { secondary: false })));
    assert_eq!(hits(&ev), [(14, 65.0)]);
    // 24: attack2, the right.
    let (mut sim, p) = setup(false);
    let ev = run(&mut sim, p, 30, |s, k| s.intent(p).secondary = k == 0);
    assert_eq!(actions(&ev), [melee::SWING_ALT]);
    assert_eq!(hits(&ev), [(14, 65.0)]);
    // 25: attack, then attack2 held from 0.3 s: the right waits for 0.8 s.
    let (mut sim, p) = setup(false);
    let ev = run(&mut sim, p, 80, |s, k| {
        s.intent(p).fire = k == 0;
        s.intent(p).secondary = k >= 20;
    });
    assert_eq!(started(&ev), [0, 54], "{ev:?}");
    // 26: a crit right: the uppercut, 195.
    let (mut sim, p) = setup(true);
    let ev = run(&mut sim, p, 30, |s, k| s.intent(p).secondary = k == 0);
    assert_eq!(actions(&ev), [melee::SWING_CRIT]);
    assert_eq!(hits(&ev), [(14, 195.0)]);
}

#[test]
fn the_first_swing_waits_out_the_deploy_30() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let p = sim.spawn_character(from, tf2::game::movement::ID);
    sim.seconds(1.0);
    let w = give(sim.app.world_mut(), p, BONESAW).unwrap();
    let ev = run(&mut sim, p, 60, |s, k| {
        if k == 0 {
            s.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
        }
        s.intent(p).fire = true;
    });
    assert_eq!(started(&ev), [34], "{ev:?}");
}

/// A switch during the delay cancels the smack (open question 3).
#[test]
fn a_switch_cancels_the_smack() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, shovel) = holding(&mut sim, from, SHOVEL);
    let other = give(sim.app.world_mut(), p, weapons::SCATTERGUN).unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(shovel);
    sim.seconds(1.0);
    let t = target(&mut sim, from, 50.0);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    let ev = run(&mut sim, p, 30, |s, k| {
        s.intent(p).fire = k == 0;
        if k == 5 {
            s.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(other);
        }
    });
    assert_eq!(started(&ev), [0]);
    assert!(hits(&ev).is_empty() && !ev.iter().any(|(_, k)| matches!(k, WeaponEventKind::Swing { .. })), "{ev:?}");
}

/// Every melee weapon gives and is drawn (and is its class's arms).
#[test]
fn all_nine_give() {
    for id in MELEE {
        let mut sim = sim();
        let (_, w) = holding(&mut sim, greybox::SPAWNS[0], id);
        assert_eq!(sim.app.world().get::<Weapon>(w).unwrap().id, id);
        assert!(sim.app.world().get::<Tf2Melee>(w).is_some(), "{id}");
    }
}
