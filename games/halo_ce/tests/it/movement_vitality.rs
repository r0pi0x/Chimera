//! Halo's movement and vitality together (the glue between
//! specs/halo_ce/movement.md and health_shields.md), end to end on
//! `HaloSim` at 30 Hz on the movement tests' level: a fall as a damage
//! event (shield, then body, with the spec's numbers; the in-air kill; the
//! credit of a fall after an enemy's hit), and a stun a damage event sets
//! slowing the player's walking and jumping as movement.md states (speed
//! and jump both by 1 - 0.8 s_b: the turning penalty on the jump).

use bevy::prelude::*;
use chimera_core::core::{BodyRegion, Died, Health, Team};
use halo_ce::game::{
    damage::HaloHit,
    maps::space,
    movement::{self, MovementData},
    vitality::{DamageEffects, Stun, Vitality, ids},
};

use crate::movement::{T, floor};

fn vit(t: &T, e: Entity) -> Vitality {
    t.sim.app.world().get::<Vitality>(e).unwrap().clone()
}

fn close(a: f32, b: f32, eps: f32, what: &str) {
    assert!((a - b).abs() <= eps, "{what}: got {a}, want {b} (tolerance {eps})");
}

fn effects(t: &T) -> DamageEffects {
    t.sim.app.world().resource::<DamageEffects>().clone()
}

/// Whether the player's `Died` was sent in the tick just run, and by whom.
fn died(t: &T) -> Option<Option<Entity>> {
    let messages = t.sim.app.world().resource::<Messages<Died>>();
    messages.iter_current_update_messages().find(|d| d.entity == t.p).map(|d| d.attacker)
}

/// A player dropped from rest `h` above the floor, run until it is on the
/// ground (at most 200 ticks).
fn dropped(h: f32) -> T {
    let mut t = T::new(vec![floor()], Vec3::new(0.0, 0.0, h));
    t.ticks(2);
    for _ in 0..200 {
        t.tick();
        if !t.mv().airborne {
            return t;
        }
    }
    panic!("not grounded");
}

/// A plasma bolt's hit at scale `s` on the player, no instigator.
fn bolt(t: &mut T, s: f32) {
    let amount = effects(t).get(ids::PLASMA_RIFLE_BOLT).unwrap().amount(s, 0.0);
    let hit = HaloHit::direct(t.p, ids::PLASMA_RIFLE_BOLT, amount, s, None, Vec3::ZERO, Vec3::NEG_Z, None);
    t.sim.app.world_mut().write_message(hit);
}

/// movement.md "Landing and falling", through a shielded Spartan: the
/// shield (75 points) takes the fall first, the rest goes to the body
/// (75 points); the damage points are the spec's table's.
#[test]
fn a_fall_hurts_the_shield_then_the_body() {
    // H, damage points.
    let cases = [(1.0, 0.0), (3.0, 0.0), (3.1, 18.33), (4.0, 61.00), (5.0, 112.20)];
    for (h, points) in cases {
        let t = dropped(h);
        let v = vit(&t, t.p);
        assert!(!v.dead, "H = {h}: alive");
        let shield = (75.0f32 - points).max(0.0) / 75.0;
        let body = (75.0f32 - (points - 75.0).max(0.0)) / 75.0;
        close(v.shield, shield, 2e-4, &format!("H = {h}: shield"));
        close(v.body, body, 2e-4, &format!("H = {h}: body"));
        // The engine's side agrees: the body is `Health`.
        close(t.sim.app.world().get::<Health>(t.p).unwrap().current, v.body, 1e-6, "Health");
    }
    // The spec's exact case: the tick-42 landing, q_f 0.0574539, 18.33.
    let t = dropped(3.1);
    close(vit(&t, t.p).shield, 0.7555891, 1e-5, "H = 3.1: shield, health_shields.md's test case");
}

/// A fall from 6.5 WU and up (155 points) kills a full Spartan: 75 to the
/// shield, 80 to a body of 75 (health_shields.md: body -0.0667).
#[test]
fn a_hard_enough_fall_kills() {
    for h in [6.5, 16.5] {
        let mut t = T::new(vec![floor()], Vec3::new(0.0, 0.0, h));
        t.ticks(2);
        let mut seen = false;
        for _ in 0..200 {
            t.tick();
            seen |= died(&t).is_some();
            if !t.mv().airborne {
                break;
            }
        }
        let v = vit(&t, t.p);
        assert!(seen && v.dead, "H = {h}: dead on landing");
        close(v.shield, 0.0, 1e-6, "shield");
        close(v.body, -0.0666667, 1e-4, &format!("H = {h}: body"));
    }
}

/// movement.md: faster than 0.343 WU/t down kills in the air with
/// `globals\distance` (105): the shield goes and the body is left at 0.6
/// on tick 97, the second such tick kills (health_shields.md: 105 does not
/// kill, a second does).
#[test]
fn the_in_air_kill_damage_is_a_hit_too() {
    let mut t = T::new(vec![floor()], Vec3::new(0.0, 0.0, 30.0));
    t.ticks(96);
    let v = vit(&t, t.p);
    assert!(v.shield == 1.0 && v.body == 1.0, "nothing before tick 97");
    t.tick();
    assert!(t.mv().airborne);
    let v = vit(&t, t.p);
    close(v.shield, 0.0, 1e-6, "tick 97: shield");
    close(v.body, 0.6, 1e-5, "tick 97: body");
    assert!(!v.dead);
    t.tick();
    assert!(vit(&t, t.p).dead, "tick 98: the second 105 kills, in the air");
    assert!(t.mv().airborne, "still falling when it died");
}

/// A fall that kills within 6 s of an enemy's damage is the enemy's kill
/// (health_shields.md "Credit", multiplayer_rules.md); a stale hit or none
/// is nobody's. The victim is thrown up at 0.5 WU/t, so that its death
/// (the in-air kill, some 235 ticks later) is more than 180 ticks after a
/// shot at the start and less than 180 after one 100 ticks in.
#[test]
fn a_fall_after_enemy_damage_is_credited() {
    for (shot_at, credited) in [(Some(100), true), (Some(0), false), (None, false)] {
        let mut t = T::new(vec![floor()], Vec3::new(0.0, 0.0, 200.0));
        let height = MovementData::builtin().centre_above_origin(0.0);
        let enemy = t.sim.spawn_character(space::point([6.0, 0.0, height]), movement::ID);
        t.sim.app.world_mut().entity_mut(enemy).insert(Team(1));
        t.ticks(2);
        t.set_vel(Vec3::new(0.0, 0.0, 0.5));
        let amount = effects(&t).get(ids::ASSAULT_RIFLE_BULLET).unwrap().amount(1.0, 0.0);
        let mut credit = None;
        for n in 0..400 {
            if shot_at == Some(n) {
                let shot = HaloHit::direct(
                    t.p,
                    ids::ASSAULT_RIFLE_BULLET,
                    amount,
                    1.0,
                    Some(BodyRegion::Chest),
                    Vec3::ZERO,
                    Vec3::NEG_Z,
                    Some(enemy),
                );
                t.sim.app.world_mut().write_message(shot);
            }
            t.tick();
            if let Some(attacker) = died(&t) {
                credit = Some((n, attacker));
                break;
            }
        }
        let (n, attacker) = credit.expect("died");
        assert!(n > 200 && t.mv().airborne, "killed in the air, late: tick {n}");
        assert_eq!(attacker, credited.then_some(enemy), "the shot at {shot_at:?}, the death at tick {n}");
    }
}

/// movement.md "Damage stun": a real stun (a plasma bolt) scales the
/// wanted speed by 1 - 0.8 s_b while it lasts, then it drops to 0 at once;
/// nothing else of the movement changes (the first tick's acceleration is
/// the unstunned 0.0106667; the turn rate is the player's look, which the
/// movement does not scale). A bolt a tick for 12 ticks stacks the counter
/// to its cap of 37.
#[test]
fn a_stunned_player_walks_slower_while_it_lasts() {
    let mut t = T::standing();
    for _ in 0..12 {
        bolt(&mut t, 1.0);
        t.tick();
    }
    let s = *t.sim.app.world().get::<Stun>(t.p).unwrap();
    assert_eq!((s.value, s.ticks), (1.0, 37));
    t.throttle(1.0, 0.0);
    t.tick();
    close(t.speed(), 0.0106667, 2e-6, "the acceleration limit, not scaled");
    t.ticks(19);
    // Fully stunned: 0.2 of the run speed 0.075 WU/t, held.
    close(t.speed(), 0.015, 2e-6, "stunned run speed");
    t.ticks(20);
    assert_eq!(t.sim.app.world().get::<Stun>(t.p).unwrap().value, 0.0, "the stun has ended");
    t.ticks(30);
    close(t.speed(), 0.075, 2e-6, "run speed after the stun");
}

/// movement.md "Damage stun" and its quirk: the jump is scaled by the
/// turning penalty 0.8 (not the jumping penalty 1.0): a stun of 0.5 jumps
/// at 0.07 x 0.6 = 0.042 (the spec's test case), a full stun at 0.014.
#[test]
fn a_stunned_jump_uses_the_turning_penalty() {
    for (s, v_z) in [(0.5, 0.042), (1.0, 0.014)] {
        let mut t = T::standing();
        bolt(&mut t, s);
        t.tick();
        close(t.sim.app.world().get::<Stun>(t.p).unwrap().value, s, 1e-6, "the stun a bolt of that scale sets");
        t.jump(true);
        t.tick();
        close(t.vel().z, v_z, 2e-6, &format!("s_b = {s}: v_z"));
    }
}

// ---- Leaving the map (multiplayer_rules.md 6.9, "Test cases") ----

/// Ticks from the first airborne tick (fall tick 1) until the player's
/// death, and the fall tick of the first 105-point hit; `Died` names no
/// killer.
fn fall_ticks(t: &mut T, max: u32) -> (u32, u32) {
    let mut first_hit = None;
    for n in 1..=max {
        t.tick();
        let v = vit(t, t.p);
        if first_hit.is_none() && (v.shield < 1.0 || v.body < 1.0) {
            first_hit = Some(n);
        }
        if let Some(killer) = died(t) {
            assert_eq!(killer, None, "a fall's death has no killer");
            assert!(t.mv().airborne, "killed in the air");
            return (first_hit.expect("hit before the death"), n);
        }
    }
    panic!("no death in {max} ticks of falling");
}

/// A full Spartan walks off a ledge (Boarding Action's gap): 105 points
/// on one tick (shield 0, body 0.6), dead on the next, in the air, no
/// killer. multiplayer_rules.md 6.9 says ticks 97 and 98 of the fall, as
/// from rest; movement.md ("First-tick fall": the tick that walks off has
/// already moved the biped 1/128 WU down, the next adds gravity to v_z =
/// -0.0078125) makes it one tick sooner, 96 and 97, which the movement
/// does (the gap is that spec's open question 23). From rest the spec's
/// 97 and 98 hold (`the_in_air_kill_damage_is_a_hit_too`).
#[test]
fn walking_off_a_ledge_kills_on_the_97th_tick_of_the_fall() {
    let ledge = crate::movement::aabb(Vec3::new(-60.0, -60.0, -5.0), Vec3::new(0.3, 60.0, 0.0));
    let mut t = T::new(vec![ledge], Vec3::ZERO);
    t.ticks(3);
    assert!(!t.mv().airborne, "settled on the ledge");
    t.face(0.0);
    t.throttle(1.0, 0.0);
    for _ in 0..60 {
        t.tick();
        if t.mv().airborne {
            break;
        }
    }
    assert!(t.mv().airborne, "walked off");
    t.throttle(0.0, 0.0);
    // That tick was fall tick 1.
    let mut first = None;
    for n in 2..=120u32 {
        t.tick();
        let v = vit(&t, t.p);
        if first.is_none() && v.body < 1.0 {
            first = Some(n);
            close(v.shield, 0.0, 1e-6, "the first hit: shield");
            close(v.body, 0.6, 1e-5, "the first hit: body");
        }
        if let Some(killer) = died(&t) {
            assert_eq!(killer, None, "no killer");
            assert_eq!((first, n), (Some(96), 97), "first hit, death (fall ticks)");
            return;
        }
    }
    panic!("not killed: first hit {first:?}");
}

/// The same from rest under a full overshield (shield 3.0, 225 points):
/// three hits, dead on tick 99. The spec's 1.6 and 0.2 left after ticks 97
/// and 98 leave out the overshield's own drain (1/1350 a tick above 1,
/// health_shields.md), which takes 0.072 more by then.
#[test]
fn an_overshield_lasts_one_more_tick() {
    let mut t = T::new(vec![floor()], Vec3::new(0.0, 0.0, 30.0));
    // Fall tick 1 makes the body's vitality; the overshield on it after.
    t.tick();
    t.sim.app.world_mut().get_mut::<Vitality>(t.p).unwrap().shield = 3.0;
    t.ticks(95);
    assert_eq!(vit(&t, t.p).body, 1.0);
    t.tick();
    let drain = 96.0 / 1350.0;
    close(vit(&t, t.p).shield, 1.6 - drain, 2e-3, "tick 97: shield");
    t.tick();
    let v = vit(&t, t.p);
    close(v.shield, 0.2 - drain, 2e-3, "tick 98: shield");
    assert_eq!((v.body, v.dead), (1.0, false), "tick 98: the body untouched");
    t.tick();
    assert!(vit(&t, t.p).dead && died(&t) == Some(None), "tick 99: dead, no killer");
}

/// Jumping off (leaving at +0.07 WU/t): the first hit about 116 ticks
/// after the jump, dead on the next tick.
#[test]
fn jumping_off_kills_about_117_ticks_after_the_jump() {
    let mut t = T::new(vec![floor()], Vec3::new(0.0, 0.0, 30.0));
    t.set_vel(Vec3::new(0.0, 0.0, 0.07));
    let (first, dead) = fall_ticks(&mut t, 200);
    assert!((115..=117).contains(&first), "first hit on tick {first}");
    assert_eq!(dead, first + 1, "dead on the next tick");
}

/// Under a party app's 66.7 Hz tick the hits still come once a 30 Hz
/// tick: a fall from rest still kills about 98/30 s in.
#[test]
fn under_a_party_tick_a_fall_kills_at_the_same_time() {
    let hz = 1.0 / 0.015;
    let mut t = T::at_rate(vec![floor()], Vec3::new(0.0, 0.0, 30.0), hz);
    let (first, dead) = fall_ticks(&mut t, 400);
    let (first, dead) = (first as f64 * 0.015, dead as f64 * 0.015);
    assert!((96.0 / 30.0..=98.0 / 30.0).contains(&first), "first hit at {first} s");
    assert!(dead - first > 0.029 && dead - first < 0.05, "the second hit a 30 Hz tick later: {first} s, {dead} s");
}
