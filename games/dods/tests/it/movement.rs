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

/// DoD:S's golden run (docs/plans/active/shared-movement.md, section 7):
/// a scripted course on the greybox with a brush staircase added (a run
/// and a sprint, strafe jumps with a second press in the air, a duck
/// jump, the stairs, a fall with damage, the ladder and a jump off it, a
/// swim and a water jump out of the tank, a crouch walk, then
/// going prone, a crawl and getting up), every
/// tick's place, velocity, flags, eye, view roll, health and stamina
/// hashed. The hash is the one taken before the movement's code was moved
/// (the plan's step S0): a refactor that changes one float operation
/// changes it. A behaviour change re-takes it in its own commit, saying
/// why. The run prints its work counts (the step's instructions and the
/// whole tick's, per tick), which the plan's log keeps per step.
/// `CHIMERA_GOLDEN_TRACE=1` prints every tick's row, to find the first
/// tick two builds differ at. Asserted on Linux x86-64 only (elsewhere the
/// maths library's last bits may differ: printed, not asserted). The same
/// course runs in cs_source's, hl2dm's, tf2's and dods' tests/it/movement.rs.
#[test]
fn golden_run() {
    use chimera_core::core::{Health, MapBrush};
    use chimera_world::{harness::own_process, map::MapBrushes};
    use source_engine::source_movement::QuakeMovement;
    const GOLDEN: u64 = 0xcc08703f58eba75e;
    const TICKS: u64 = 1300;
    if !own_process("movement::golden_run") {
        return;
    }
    let mut sim = sim();
    sim.repeatable();
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.ticks(2);
    // A brush staircase east of the spawn, rising toward -Z: 8 steps of
    // 12 units (0.3048 m) by 24, swept exactly (the greybox's own solids
    // are physics colliders).
    const STAIRS: Vec3 = Vec3::new(30.0, 0.0, 10.0);
    let step = 12.0 * 0.0254;
    let mut brushes = sim.app.world().resource::<MapBrushes>().0.clone();
    for i in 0..8 {
        let z = STAIRS.z - 2.0 * step * i as f32;
        brushes.push(MapBrush::from_box(
            Vec3::new(STAIRS.x - 1.5, 0.0, z - if i == 7 { 4.0 } else { 2.0 * step }),
            Vec3::new(STAIRS.x + 1.5, step * (i + 1) as f32, z),
        ));
    }
    sim.app.world_mut().insert_resource(MapBrushes(brushes));

    let place = |sim: &mut Sim, at: Vec3| {
        let w = sim.app.world_mut();
        w.get_mut::<Transform>(p).unwrap().translation = at;
        if let Some(mut pos) = w.get_mut::<avian3d::prelude::Position>(p) {
            pos.0 = at;
        }
        w.get_mut::<Velocity>(p).unwrap().0 = Vec3::ZERO;
    };
    let tank = (greybox::TANK_MIN + greybox::TANK_MAX) / 2.0;
    let trace = std::env::var_os("CHIMERA_GOLDEN_TRACE").is_some();
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut add = |bits: u32| {
        for b in bits.to_le_bytes() {
            hash = (hash ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    let (mut step_work, mut tick_work, mut counted) = (0u64, 0u64, true);
    // What the course met: the air, a duck, a ladder, a head under water,
    // the stairs' top, fall damage.
    let mut met = [false; 6];
    for t in 0..TICKS {
        {
            let mut i = sim.intent(p);
            i.move_axis = Vec2::ZERO;
            (i.jump, i.crouch, i.sprint, i.walk, i.use_key) = (false, false, false, false, false);
            i.pitch = 0.0;
            match t {
                // Settle, then a run north (-Z), its last part sprinting.
                0..30 => {}
                30..130 => {
                    i.move_axis = Vec2::Y;
                    i.sprint = t >= 100;
                }
                // Strafe jumps: a jump every 40 ticks and a second press
                // in the air, strafing right while turning right.
                130..290 => {
                    i.move_axis = Vec2::new(1.0, if (t - 130) % 40 < 4 { 1.0 } else { 0.0 });
                    i.jump = matches!((t - 130) % 40, 0 | 14);
                    i.yaw -= 1.5f32.to_radians();
                }
                // A duck jump, then standing up.
                290..350 => {
                    i.move_axis = Vec2::Y;
                    i.jump = t == 292;
                    i.crouch = (296..330).contains(&t);
                }
                // Up the stairs and off their top.
                350..500 => i.move_axis = Vec2::Y,
                // A fall from 9 m, then a walk with the walk key held.
                500..620 => {
                    i.move_axis = Vec2::new(0.0, if t >= 590 { 1.0 } else { 0.0 });
                    i.walk = true;
                }
                // Up the north wall's ladder (a use press on the way, for
                // the movements that mount by it), a jump off it, away and
                // back to the wall.
                620..800 => {
                    i.move_axis = Vec2::new(0.0, if t < 650 || t >= 740 { 1.0 } else { -1.0 });
                    i.pitch = if t < 650 { 0.6 } else { 0.0 };
                    i.use_key = t == 622;
                    i.jump = t == 650;
                }
                // The tank: sinking, then a swim up and north and out over
                // its wall.
                800..1020 => {
                    if t >= 860 {
                        i.move_axis = Vec2::Y;
                        i.jump = true;
                        i.pitch = 0.3;
                    }
                }
                // A crouch walk, let go.
                _ => {
                    i.move_axis = Vec2::new(-1.0, 1.0);
                    i.crouch = t < 1060;
                }
            }
        }
        match t {
            290 | 350 | 500 | 620 | 800 | 1020 => sim.intent(p).yaw = 0.0,
            _ => {}
        }
        match t {
            350 => place(&mut sim, STAIRS + Vec3::new(0.0, 1.0, 3.0)),
            500 => place(&mut sim, Vec3::new(20.0, 10.0, 20.0)),
            620 => place(&mut sim, Vec3::new(greybox::LADDER_X, 1.0, greybox::NORTH_WALL_Z + 0.1 + 0.43)),
            800 => place(&mut sim, Vec3::new(tank.x, 4.5, tank.y)),
            1020 => place(&mut sim, greybox::SPAWNS[1]),
            _ => {}
        }
        // Prone at 1080, a crawl, up again at 1220.
        let prone = sim.app.world().resource::<ActionRegistry>().bit(stance::PRONE_ACTION);
        if matches!(t, 1080 | 1220) {
            sim.intent(p).actions |= prone;
        } else {
            sim.intent(p).actions &= !prone;
        }
        match sim.work(1) {
            Some(w) => {
                tick_work += w.total();
                step_work += w.crate_systems.get("chimera_movement Movement step").map_or(0, |v| v.0);
            }
            None => counted = false,
        }
        let (at, v, s) = (sim.position(p), sim.velocity(p), sim.state(p));
        let me = sim.app.world().get::<QuakeMovement>(p).unwrap().clone();
        let health = sim.app.world().get::<Health>(p).map_or(0.0, |h| h.current);
        for f in [at.x, at.y, at.z, v.x, v.y, v.z, s.eye_offset.y, s.view_roll, health, me.stamina] {
            add(f.to_bits());
        }
        add(u32::from(s.on_ground)
            | u32::from(s.crouching) << 1
            | u32::from(s.on_ladder) << 2
            | u32::from(me.ducked) << 3
            | u32::from(me.ducking) << 4
            | u32::from(me.water_level) << 8);
        if trace {
            eprintln!(
                "{t} at {at:?} v {v:?} eye {} roll {} hp {health} stamina {} ground {} crouch {} ladder {} ducked {} ducking {} water {}",
                s.eye_offset.y, s.view_roll, me.stamina, s.on_ground, s.crouching, s.on_ladder, me.ducked, me.ducking, me.water_level
            );
        }
        met[0] |= !s.on_ground;
        met[1] |= me.ducked;
        met[2] |= s.on_ladder;
        met[3] |= me.water_level == 3;
        met[4] |= (350..500).contains(&t) && at.y > 2.5;
        met[5] |= health < 1.0;
    }
    eprintln!("dods golden run: hash {hash:#018x}, met (air, duck, ladder, swim, stairs, fall damage) {met:?}");
    if counted {
        eprintln!(
            "dods golden run work: the step {} instructions a tick, the tick {} ({TICKS} ticks)",
            step_work / TICKS,
            tick_work / TICKS
        );
    } else {
        eprintln!("dods golden run work: no instruction counters here, not counted");
    }
    assert_eq!(met, [true; 6], "the course met (air, duck, ladder, swim, stairs, fall damage)");
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        assert_eq!(hash, GOLDEN, "the golden run's hash changed: {hash:#018x} (docs/plans/active/shared-movement.md 7)");
    } else {
        eprintln!("dods golden run: the hash is asserted on Linux x86-64 only");
    }
}
