//! DoD:S's movement on the headless harness (`harness::DodsSim`, the
//! greybox), from specs/dods/movement.md's and damage.md's test cases:
//! the run's acceleration and stop, the sprint (from rest 48, 90, 127.2
//! and the cost; its cap 230 + stamina and the 0.3 drain; 120 empty; the
//! run's cap 120 + stamina; no sprint backwards or below 80 u/s), the
//! regeneration by state, the jump (262.33, 268.33 ducked, no stamina
//! cost), the duck and the unduck's 0.21 s, getting up from prone (the
//! standing box at once and the rising cap; duck to a crouch), and the
//! hit slow's 120 cap. Speeds in units/s.

use bevy::prelude::*;
use chimera_core::core::{ActionRegistry, SimClock, Velocity};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use dods::{
    game::{
        TICK_INTERVAL, movement,
        stamina::DodsStamina,
        stance::{self, DodsStance, Stance},
        weapons::hit_slow::DodsSlowed,
    },
    harness::DodsSim,
};

const U: f32 = 0.0254;
const ORIGIN: f32 = 36.0 * U;

fn sim() -> Sim {
    let mut sim = Sim::new(DodsSim, GreyboxMapPlugin);
    sim.set_tick_interval(TICK_INTERVAL);
    sim
}

/// A settled player at the greybox's first spawn, looking down -Z.
fn player(sim: &mut Sim) -> Entity {
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.seconds(1.5);
    p
}

fn speed(sim: &Sim, p: Entity) -> f32 {
    sim.velocity(p).xz().length() / U
}

fn eye(sim: &Sim, p: Entity) -> f32 {
    (ORIGIN + sim.state(p).eye_offset.y) / U
}

fn hull_top(sim: &Sim, p: Entity) -> f32 {
    let s = sim.state(p);
    (s.hull_max.y - s.hull_min.y) / U
}

fn stamina(sim: &Sim, p: Entity) -> DodsStamina {
    *sim.app.world().get::<DodsStamina>(p).unwrap()
}

fn set_stamina(sim: &mut Sim, p: Entity, value: f32, sprinting: bool) {
    *sim.app.world_mut().get_mut::<DodsStamina>(p).unwrap() = DodsStamina { value, sprinting };
}

/// Moving forward (-Z) at `v` units/s.
fn set_speed(sim: &mut Sim, p: Entity, v: f32) {
    sim.app.world_mut().get_mut::<Velocity>(p).unwrap().0 = Vec3::NEG_Z * v * U;
}

fn near(a: f32, b: f32, tol: f32, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a} against {b}");
}

#[test]
fn the_run_accelerates_and_stops_as_measured() {
    let mut sim = sim();
    let p = player(&mut sim);
    sim.intent(p).move_axis = Vec2::Y;
    sim.ticks(1);
    near(speed(&sim, p), 33.0, 0.05, "1 tick");
    sim.ticks(2);
    near(speed(&sim, p), 85.8, 0.05, "3 ticks");
    set_speed(&mut sim, p, 220.0);
    sim.intent(p).move_axis = Vec2::ZERO;
    sim.ticks(1);
    near(speed(&sim, p), 193.6, 0.05, "released from 220");
    // Backwards: 220, no sprint, the stamina untouched.
    sim.intent(p).move_axis = Vec2::NEG_Y;
    sim.intent(p).sprint = true;
    sim.seconds(1.5);
    near(speed(&sim, p), 220.0, 0.5, "backwards with +speed");
    assert!(!stamina(&sim, p).sprinting);
    near(stamina(&sim, p).value, 100.0, 1e-3, "stamina");
}

#[test]
fn ducking_slows_to_a_third_from_the_first_tick() {
    let mut sim = sim();
    let p = player(&mut sim);
    sim.intent(p).crouch = true;
    sim.intent(p).move_axis = Vec2::Y;
    sim.ticks(1);
    near(speed(&sim, p), 11.0, 0.05, "ducked, 1 tick");
}

#[test]
fn the_sprint_from_rest_costs_15_then_drains_and_caps() {
    let mut sim = sim();
    let p = player(&mut sim);
    sim.intent(p).move_axis = Vec2::Y;
    sim.intent(p).sprint = true;
    sim.ticks(1);
    near(speed(&sim, p), 48.0, 0.05, "1 tick");
    sim.ticks(1);
    near(speed(&sim, p), 90.0, 0.05, "2 ticks");
    sim.ticks(1);
    near(speed(&sim, p), 127.2, 0.05, "3 ticks");
    assert!(stamina(&sim, p).sprinting, "the sprint starts at 90 u/s");
    near(stamina(&sim, p).value, 85.0, 1e-3, "its cost");
    sim.ticks(1);
    near(speed(&sim, p), 159.19, 0.05, "4 ticks");
    // Sprinting at 312 with 82.3 left: the cap 230 + 82.3, the drain 0.3.
    set_speed(&mut sim, p, 312.0);
    set_stamina(&mut sim, p, 82.3, true);
    sim.ticks(1);
    near(speed(&sim, p), 312.3, 0.05, "the cap");
    near(stamina(&sim, p).value, 82.0, 1e-3, "the drain");
    // Nearly empty: 0, then the 120 cap.
    set_stamina(&mut sim, p, 0.1, true);
    sim.ticks(2);
    near(stamina(&sim, p).value, 0.0, 1e-6, "empty");
    near(speed(&sim, p), 120.0, 0.05, "empty sprint");
    assert!(stamina(&sim, p).sprinting, "the sprint goes on at 0");
}

#[test]
fn letting_go_of_the_sprint_caps_the_run_at_120_plus_stamina() {
    let mut sim = sim();
    let p = player(&mut sim);
    sim.intent(p).move_axis = Vec2::Y;
    set_speed(&mut sim, p, 298.0);
    set_stamina(&mut sim, p, 67.9, true);
    sim.ticks(1);
    near(speed(&sim, p), 187.9, 0.05, "released at 67.9");
    near(stamina(&sim, p).value, 68.05, 1e-3, "the moving rate");
    // A long run at 50: 170 climbing with the stamina (+0.15 a tick).
    set_stamina(&mut sim, p, 50.0, false);
    sim.ticks(1);
    let st = stamina(&sim, p).value;
    sim.ticks(1);
    near(speed(&sim, p), 120.0 + st, 0.05, "120 + stamina");
    near(stamina(&sim, p).value - st, 0.15, 1e-3, "regenerating while running");
}

#[test]
fn no_sprint_below_80() {
    let mut sim = sim();
    let p = player(&mut sim);
    sim.intent(p).move_axis = Vec2::Y * (79.0 / 450.0);
    sim.intent(p).sprint = true;
    sim.seconds(1.5);
    near(speed(&sim, p), 79.0, 0.1, "wish 79");
    assert!(!stamina(&sim, p).sprinting);
}

#[test]
fn stamina_regenerates_by_state() {
    let mut sim = sim();
    let p = player(&mut sim);
    set_stamina(&mut sim, p, 20.0, false);
    sim.ticks(1);
    near(stamina(&sim, p).value, 20.9, 1e-3, "still");
    sim.intent(p).crouch = true;
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(0.6);
    set_stamina(&mut sim, p, 20.0, false);
    sim.ticks(1);
    near(stamina(&sim, p).value, 20.75, 1e-3, "ducked and moving");
}

#[test]
fn the_jump_is_262_33_and_costs_no_stamina() {
    let mut sim = sim();
    let p = player(&mut sim);
    let feet = sim.position(p).y;
    sim.intent(p).jump = true;
    sim.ticks(1);
    near(sim.velocity(p).y / U, 250.33, 0.05, "vz after the jump tick");
    near((sim.position(p).y - feet) / U, 3.845, 0.01, "the first tick's rise");
    let mut apex: f32 = 0.0;
    let mut air = 1;
    for _ in 0..60 {
        sim.ticks(1);
        apex = apex.max((sim.position(p).y - feet) / U);
        if sim.state(p).on_ground {
            break;
        }
        air += 1;
    }
    near(apex, 43.01, 0.1, "apex");
    assert_eq!(air, 43, "ticks in the air");
    near(stamina(&sim, p).value, 100.0, 1e-3, "no stamina cost");
    // Empty, the same jump.
    sim.intent(p).jump = false;
    sim.seconds(0.5);
    set_stamina(&mut sim, p, 0.0, false);
    sim.intent(p).jump = true;
    sim.ticks(1);
    near(sim.velocity(p).y / U, 250.33, 0.05, "empty");
}

#[test]
fn the_ducked_jump_is_268_33() {
    let mut sim = sim();
    let p = player(&mut sim);
    sim.intent(p).crouch = true;
    sim.seconds(1.0);
    sim.intent(p).jump = true;
    sim.ticks(1);
    near(sim.velocity(p).y / U, 256.33, 0.05, "vz after the ducked jump tick");
}

#[test]
fn ducking_takes_28_ticks_and_standing_up_14() {
    let mut sim = sim();
    let p = player(&mut sim);
    sim.intent(p).crouch = true;
    sim.ticks(27);
    near(hull_top(&sim, p), 72.0, 0.01, "still standing at 27");
    sim.ticks(1);
    near(eye(&sim, p), 34.0, 0.01, "eye at 28");
    near(hull_top(&sim, p), 45.0, 0.01, "box at 28");
    sim.seconds(0.5);
    sim.intent(p).crouch = false;
    sim.ticks(1);
    near(eye(&sim, p), 34.385, 0.1, "the release tick");
    sim.ticks(12);
    near(eye(&sim, p), 57.956, 0.1, "13 ticks");
    near(hull_top(&sim, p), 45.0, 0.01, "box at 13");
    sim.ticks(1);
    near(eye(&sim, p), 58.0, 0.01, "14 ticks");
    near(hull_top(&sim, p), 72.0, 0.01, "box at 14");
}

fn press_prone(sim: &mut Sim, p: Entity) {
    let bit = sim.app.world().resource::<ActionRegistry>().bit(stance::PRONE_ACTION);
    sim.intent(p).actions |= bit;
    sim.ticks(1);
    sim.intent(p).actions &= !bit;
}

fn prone_player(sim: &mut Sim) -> Entity {
    let p = player(sim);
    press_prone(sim, p);
    sim.ticks(79);
    sim.seconds(0.5);
    assert!(sim.app.world().get::<DodsStance>(p).unwrap().prone());
    p
}

#[test]
fn getting_up_stands_the_box_at_once_and_lets_the_player_move() {
    let mut sim = sim();
    let p = prone_player(&mut sim);
    sim.intent(p).move_axis = Vec2::Y;
    press_prone(&mut sim, p);
    near(hull_top(&sim, p), 72.0, 0.01, "the standing box on the press");
    near(speed(&sim, p), 7.5, 0.05, "1 tick");
    sim.ticks(27);
    near(speed(&sim, p), 95.09, 2.5, "28 ticks");
    sim.ticks(52);
    assert_eq!(sim.app.world().get::<DodsStance>(p).unwrap().stance, Stance::Up);
    near(speed(&sim, p), 220.0, 0.5, "80 ticks");
    near(eye(&sim, p), 58.0, 0.01, "eye");
}

#[test]
fn duck_while_prone_gets_up_to_a_crouch() {
    let mut sim = sim();
    let p = prone_player(&mut sim);
    sim.intent(p).crouch = true;
    sim.ticks(1);
    near(hull_top(&sim, p), 45.0, 0.01, "the ducked box on the press");
    sim.ticks(79);
    assert_eq!(sim.app.world().get::<DodsStance>(p).unwrap().stance, Stance::Up);
    near(eye(&sim, p), 34.0, 0.01, "ducked eye");
    sim.ticks(10);
    near(hull_top(&sim, p), 45.0, 0.01, "stays ducked with duck held");
    sim.intent(p).crouch = false;
    sim.ticks(14);
    near(hull_top(&sim, p), 72.0, 0.01, "stands once duck is let go");
}

#[test]
fn a_hit_caps_the_speed_at_120() {
    let mut sim = sim();
    let p = player(&mut sim);
    sim.intent(p).move_axis = Vec2::Y;
    set_speed(&mut sim, p, 220.0);
    let now = sim.app.world().resource::<SimClock>().now;
    sim.app.world_mut().get_mut::<DodsSlowed>(p).unwrap().until = now + 0.5;
    sim.ticks(1);
    near(speed(&sim, p), 120.0, 0.05, "the next tick");
    sim.ticks(30);
    near(speed(&sim, p), 120.0, 0.05, "held");
    sim.seconds(0.5);
    near(speed(&sim, p), 220.0, 0.5, "back to the run");
    // Ducked (73.3): unchanged.
    sim.intent(p).crouch = true;
    sim.seconds(1.0);
    let now = sim.app.world().resource::<SimClock>().now;
    sim.app.world_mut().get_mut::<DodsSlowed>(p).unwrap().until = now + 0.5;
    sim.ticks(5);
    near(speed(&sim, p), 73.33, 0.1, "ducked");
}
