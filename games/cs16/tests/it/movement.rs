//! CS 1.6's movement (`cs16:movement`, specs/cs16/movement.md) on the
//! greybox at the game's 10 ms tick: the spec's test cases (the run up to
//! 250 and the stop, the jump's apex and air time, the landing slowdown,
//! the jump penalty's scale, the duck's 40 commands and the duck jump, no
//! standing up under a ceiling, fall damage and landing effects, the
//! bunny-hop cut, the ladder's speeds, the hit slowdown, the punch's
//! decay), the cvar defaults and the speed table, and the golden run
//! (`golden_run`: the course of the Source games' golden runs, every
//! tick hashed). On the greybox there are no clip hulls: these run on
//! Chimera's shapes (the hull traces are `heavy::movement`'s, on the
//! install).

use bevy::prelude::*;
use chimera_core::core::{Health, MapBrush, Velocity};
use chimera_movement::{QuakeMovement, to_units};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
    map::MapBrushes,
};
use cs16::{
    game::movement::{self, ID},
    harness::Cs16Sim,
};

const UNIT: f32 = 0.0254;

/// The transform over the feet, units.
const ORIGIN: f32 = 36.0;

fn standing() -> (Sim, Entity) {
    let mut sim = Sim::new(Cs16Sim, GreyboxMapPlugin);
    // A frame a tick: every `ticks(1)` is one 10 ms command.
    sim.set_tick_interval(cs16::game::TICK_INTERVAL);
    sim.repeatable();
    let p = sim.spawn_character(greybox::SPAWNS[0], ID);
    sim.ticks(150);
    assert!(me(&sim, p).on_ground, "settled on the floor");
    (sim, p)
}

fn me(sim: &Sim, p: Entity) -> QuakeMovement {
    sim.app.world().get::<QuakeMovement>(p).unwrap().clone()
}

/// Feet, units (Z up).
fn feet(sim: &Sim, p: Entity) -> Vec3 {
    to_units(sim.position(p)) - Vec3::Z * ORIGIN
}

/// Velocity, units/s (Z up).
fn vel(sim: &Sim, p: Entity) -> Vec3 {
    to_units(sim.velocity(p))
}

fn flat_speed(sim: &Sim, p: Entity) -> f32 {
    vel(sim, p).truncate().length()
}

fn set_vel(sim: &mut Sim, p: Entity, v: Vec3) {
    sim.app.world_mut().get_mut::<Velocity>(p).unwrap().0 = chimera_movement::to_engine(v);
}

fn state_mut(sim: &mut Sim, p: Entity) -> Mut<'_, QuakeMovement> {
    sim.app.world_mut().get_mut::<QuakeMovement>(p).unwrap()
}

/// Put the feet at `at` (engine space), still.
fn place(sim: &mut Sim, p: Entity, at: Vec3) {
    let to = at + Vec3::Y * ORIGIN * UNIT;
    let w = sim.app.world_mut();
    w.get_mut::<Transform>(p).unwrap().translation = to;
    if let Some(mut pos) = w.get_mut::<avian3d::prelude::Position>(p) {
        pos.0 = to;
    }
    w.get_mut::<Velocity>(p).unwrap().0 = Vec3::ZERO;
}

/// Running at the knife's 250, forward held.
fn running() -> (Sim, Entity) {
    let (mut sim, p) = standing();
    sim.intent(p).move_axis = Vec2::Y;
    sim.ticks(60);
    assert!((flat_speed(&sim, p) - 250.0).abs() < 1e-2, "{}", flat_speed(&sim, p));
    (sim, p)
}

#[test]
fn the_cvar_defaults_and_the_speed_table() {
    let c = movement::config();
    for (name, value) in [
        ("sv_accelerate", "5"),
        ("sv_airaccelerate", "10"),
        ("sv_friction", "4"),
        ("sv_stopspeed", "75"),
        ("edgefriction", "2"),
        ("sv_maxspeed", "900"),
        ("sv_stepsize", "18"),
        ("sv_gravity", "800"),
        ("sv_bounce", "1"),
        ("sv_maxvelocity", "2000"),
    ] {
        assert_eq!(c.get_cvar(name).as_deref(), Some(value), "{name}");
    }
    assert!(c.clip_hulls && c.command_msec && c.ground_snap && c.unduck_at_once);
    assert_eq!((c.stand_height, c.duck_height, c.eye_stand, c.eye_duck, c.half_width), (72.0, 36.0, 53.0, 30.0, 16.0));
    // The duck jump: the feet lifted 18, the box's centre where it was.
    assert_eq!((c.stand_height - c.duck_height) * c.air_duck_shift, 18.0);
    let n = movement::numbers();
    let speed = |id: Option<&str>, zoomed| movement::speed(&n, id, zoomed);
    assert_eq!(speed(None, false), 240.0);
    assert_eq!(speed(Some("cs16:weapon_standin"), false), 250.0, "what the table doesn't list: the knife's and pistols' 250");
    assert_eq!(speed(Some("cs16:weapon_ak47"), false), 221.0);
    assert_eq!(speed(Some("p90"), false), 245.0);
    assert_eq!((speed(Some("awp"), false), speed(Some("awp"), true)), (210.0, 150.0));
    assert_eq!((speed(Some("scout"), false), speed(Some("scout"), true)), (260.0, 220.0));
    assert_eq!((speed(Some("sg552"), false), speed(Some("sg552"), true)), (235.0, 200.0));
    assert_eq!(speed(Some("aug"), true), 240.0, "the AUG keeps its speed zoomed");
    assert_eq!(speed(Some("m249"), false), 220.0);
    // Every gun's own `max_speed` (what the weapon layer hands the
    // movement) is the table's.
    let mut checked = 0;
    for w in cs16::game::weapons::weapons().weapons {
        if let Some(s) = w.max_speed {
            assert_eq!(s, speed(Some(&w.id), false), "{}", w.id);
            checked += 1;
        }
    }
    assert!(checked >= 20, "{checked} guns state a speed");
}

#[test]
fn the_run_up_to_full_speed() {
    let (mut sim, p) = standing();
    assert!((sim.tick_hz() - 100.0).abs() < 1e-6, "10 ms commands");
    sim.intent(p).move_axis = Vec2::Y;
    let mut speeds = Vec::new();
    for _ in 0..45 {
        sim.ticks(1);
        speeds.push(flat_speed(&sim, p));
    }
    assert!((speeds[0] - 12.5).abs() < 1e-3, "1 command: {}", speeds[0]);
    assert!((speeds[1] - 22.0).abs() < 1e-3, "2 commands: {}", speeds[1]);
    assert!((speeds[9] - 97.31).abs() < 0.01, "10 commands: {}", speeds[9]);
    assert!(speeds[39] >= 249.0 && speeds[39] < 250.0, "40 commands: {}", speeds[39]);
    assert!((speeds[40] - 250.0).abs() < 1e-3, "41 commands: {}", speeds[40]);
    assert!(speeds[41..].iter().all(|s| (s - 250.0).abs() < 1e-3), "then steady: {:?}", &speeds[41..]);
    assert_eq!(me(&sim, p).msec_carry, 0.0, "10 ms ticks leave nothing over");
}

#[test]
fn the_stop() {
    let (mut sim, p) = running();
    sim.intent(p).move_axis = Vec2::ZERO;
    let mut speeds = Vec::new();
    for _ in 0..55 {
        sim.ticks(1);
        speeds.push(flat_speed(&sim, p));
    }
    assert!((speeds[0] - 240.0).abs() < 1e-2, "1 command: {}", speeds[0]);
    assert!((speeds[9] - 166.21).abs() < 0.01, "10 commands: {}", speeds[9]);
    assert!((speeds[29] - 73.46).abs() < 0.01, "30 commands: {}", speeds[29]);
    assert_eq!(speeds[54], 0.0, "55 commands");
    assert!(speeds[53] > 0.0, "not before: {}", speeds[53]);
}

#[test]
fn the_jump() {
    let (mut sim, p) = standing();
    let floor = feet(&sim, p).z;
    sim.intent(p).jump = true;
    let (mut apex, mut air) = (0.0f32, 0);
    for t in 0..120 {
        sim.ticks(1);
        sim.intent(p).jump = false;
        if !me(&sim, p).on_ground {
            air += 1;
            apex = apex.max(feet(&sim, p).z - floor);
        } else if t > 0 {
            break;
        }
    }
    assert!((apex - 44.99).abs() < 0.02, "apex {apex}");
    // The spec's table has 68 commands in the air and a landing at 275.67
    // (the floor met by the move itself). Its ground rule, which this is,
    // lands a command sooner: after the 67th command the feet are 0.22
    // units up, inside the 2-unit ground probe, so 66 commands end in the
    // air and the 67th lands, its fall speed the 259.67 it started with
    // (goldsrc.md, questions for the next spec session).
    assert_eq!(air, 66, "commands that ended in the air");
    assert!((me(&sim, p).last_landing_speed - 259.67).abs() < 0.01, "lands at {}", me(&sim, p).last_landing_speed);
    // The jump's timer has run 660 ms by the landing command.
    assert!((me(&sim, p).stamina - 655.79).abs() < 0.01, "{}", me(&sim, p).stamina);
    // Held, the key doesn't jump again.
    sim.intent(p).jump = true;
    sim.ticks(150);
    sim.ticks(1);
    assert!(me(&sim, p).on_ground, "no auto-jump");
}

#[test]
fn the_landing_slowdown() {
    // A jump at 250, the keys let go in the air: the first ground command
    // scales the speed by 1 - 0.00019 x timer (and friction's 0.96), and
    // each next one again. The timer is 645.79 there, a command more than
    // the spec's table's 635.79 (0.8792): the ground rule lands a command
    // sooner (`the_jump`).
    let (mut sim, p) = running();
    sim.intent(p).jump = true;
    sim.ticks(1);
    sim.intent(p).jump = false;
    sim.intent(p).move_axis = Vec2::ZERO;
    let mut on_ground = Vec::new();
    for _ in 0..100 {
        let before = (me(&sim, p).on_ground, flat_speed(&sim, p), me(&sim, p).stamina);
        sim.ticks(1);
        if before.0 {
            on_ground.push((before.1, flat_speed(&sim, p), me(&sim, p).stamina));
        }
        if on_ground.len() == 10 {
            break;
        }
    }
    let (before, after, timer) = on_ground[0];
    assert!((before - 250.0).abs() < 0.01, "no loss in the air: {before}");
    assert!((timer - 645.79).abs() < 0.01, "timer on the first ground command: {timer}");
    assert!((after - 250.0 * 0.96 * (1.0 - 0.000_19 * 645.79)).abs() < 0.05, "first ground command: {after}");
    // Ten ground commands: the penalty alone leaves about 31 % (the
    // spec's figure, from its timer; 30 % from this one), friction the
    // rest: each command's friction, then its scale.
    let (mut want, mut penalty) = (250.0f32, 1.0f32);
    for k in 0..10 {
        let r = 1.0 - 0.000_19 * (645.789_4 - 10.0 * k as f32);
        want = (want - want.max(75.0) * 4.0 * 0.01) * r;
        penalty *= r;
    }
    assert!((penalty - 0.30).abs() < 0.01, "the penalty's share: {penalty}");
    let after = on_ground[9].1;
    assert!((after - want).abs() < 0.05, "after 10 ground commands {after}, by the rules {want}");
}

#[test]
fn a_jump_under_the_penalty_is_lower() {
    let (mut sim, p) = standing();
    state_mut(&mut sim, p).stamina = 1315.789_4 + 10.0;
    sim.intent(p).jump = true;
    sim.ticks(1);
    // 268.33 x 0.75, less the command's gravity (two halves of 4).
    let vz = vel(&sim, p).z;
    assert!((vz - (268.328_16 * 0.75 - 8.0)).abs() < 0.01, "{vz}");
    assert!((me(&sim, p).stamina - 1315.789_4).abs() < 1e-3, "the timer set again");
}

#[test]
fn the_duck_takes_forty_commands() {
    let (mut sim, p) = running();
    let floor = feet(&sim, p).z;
    sim.intent(p).crouch = true;
    // The press's command.
    sim.ticks(1);
    for k in 1..40 {
        sim.ticks(1);
        let m = me(&sim, p);
        assert!(!m.ducked && m.ducking, "{k} commands after the press: still the standing box");
    }
    // The keys a third from the press: 250 x 0.333.
    assert!((flat_speed(&sim, p) - 83.25).abs() < 0.5, "slowed from the press: {}", flat_speed(&sim, p));
    assert!(me(&sim, p).eye < 53.0 && me(&sim, p).eye > 30.0, "the eye on its way: {}", me(&sim, p).eye);
    sim.ticks(1);
    let m = me(&sim, p);
    assert!(m.ducked && !m.ducking, "40 commands after the press: the ducked box");
    assert_eq!(m.eye, 30.0, "the eye at feet + 30");
    assert!((feet(&sim, p).z - floor).abs() < 0.05, "the feet stay on the floor (the origin 18 lower)");
    assert!(m.on_ground);
    // Let go: up at once.
    sim.intent(p).crouch = false;
    sim.ticks(1);
    let m = me(&sim, p);
    assert!(!m.ducked && !m.ducking && m.eye == 53.0 && m.on_ground, "{m:?}");
    assert!((feet(&sim, p).z - floor).abs() < 0.05);
}

#[test]
fn a_duck_let_go_early_hops() {
    // The original's duck tap: a duck let go before it completed stands
    // up 18 units higher.
    let (mut sim, p) = standing();
    let floor = feet(&sim, p).z;
    sim.intent(p).crouch = true;
    sim.ticks(5);
    sim.intent(p).crouch = false;
    sim.ticks(1);
    let m = me(&sim, p);
    assert!(!m.ducked && !m.ducking && !m.on_ground, "{m:?}");
    assert!((feet(&sim, p).z - floor - 18.0).abs() < 0.1, "{}", feet(&sim, p).z - floor);
}

#[test]
fn the_duck_jump() {
    let (mut sim, p) = standing();
    sim.intent(p).jump = true;
    sim.ticks(1);
    sim.intent(p).jump = false;
    sim.ticks(10);
    let (before, vz) = (feet(&sim, p).z, vel(&sim, p).z);
    sim.intent(p).crouch = true;
    sim.ticks(1);
    let m = me(&sim, p);
    assert!(m.ducked && !m.ducking, "ducked at once in the air");
    // The feet 18 higher (the box's centre unmoved), plus the command's rise.
    let rise = feet(&sim, p).z - before;
    assert!((rise - 18.0 - (vz - 4.0) * 0.01).abs() < 0.05, "feet rose {rise}");
    // Let go in the air: the standing box again, the feet 18 lower.
    let before = feet(&sim, p).z;
    let vz = vel(&sim, p).z;
    sim.intent(p).crouch = false;
    sim.ticks(1);
    assert!(!me(&sim, p).ducked);
    let drop = feet(&sim, p).z - before;
    assert!((drop + 18.0 - (vz - 4.0) * 0.01).abs() < 0.05, "feet dropped {drop}");
}

#[test]
fn no_standing_up_under_a_low_ceiling() {
    let (mut sim, p) = standing();
    sim.intent(p).crouch = true;
    sim.ticks(45);
    assert!(me(&sim, p).ducked);
    // A ceiling 60 units over the floor, above the player.
    let at = sim.position(p) - Vec3::Y * ORIGIN * UNIT;
    let mut brushes = sim.app.world().resource::<MapBrushes>().0.clone();
    brushes.push(MapBrush::from_box(
        Vec3::new(at.x - 2.0, at.y + 60.0 * UNIT, at.z - 2.0),
        Vec3::new(at.x + 2.0, at.y + 90.0 * UNIT, at.z + 2.0),
    ));
    sim.app.world_mut().insert_resource(MapBrushes(brushes));
    sim.intent(p).crouch = false;
    sim.ticks(30);
    let m = me(&sim, p);
    assert!(m.ducked && m.eye == 30.0, "stays ducked: {m:?}");
}

/// Land at `speed` units/s with `health` points; the health lost, the
/// view roll right after and the landing speed counted.
fn land(speed: f32) -> (f32, f32, f32) {
    let (mut sim, p) = standing();
    let floor = sim.position(p) - Vec3::Y * ORIGIN * UNIT;
    sim.app.world_mut().get_mut::<Health>(p).unwrap().current = 3.0;
    place(&mut sim, p, floor + Vec3::Y * 4.0 * UNIT);
    set_vel(&mut sim, p, Vec3::new(0.0, 0.0, -speed));
    sim.ticks(1);
    let m = me(&sim, p);
    assert!(m.on_ground, "landed");
    // The damage is applied by the health's systems within the tick or
    // the next.
    let roll = sim.state(p).view_roll.to_degrees();
    sim.ticks(1);
    let lost = (3.0 - sim.app.world().get::<Health>(p).unwrap().current) * 100.0;
    (lost, roll, m.last_landing_speed)
}

#[test]
fn fall_damage_and_landing_effects() {
    let (lost, roll, counted) = land(600.0);
    assert!((counted - 600.0).abs() < 0.01, "{counted}");
    assert!((lost - 20.0).abs() < 1e-3, "600 units/s: {lost} points");
    assert!((roll - 600.0 * 0.013).abs() < 0.01, "the roll kick: {roll}");
    let (lost, ..) = land(1100.0);
    assert!((lost - 125.0).abs() < 1e-3, "1100 units/s: {lost} points");
    let (lost, roll, _) = land(500.0);
    assert_eq!(lost, 0.0, "500 units/s is safe");
    assert!((roll - 6.5).abs() < 0.01, "but heard and felt: {roll}");
    let (lost, roll, _) = land(280.0);
    assert_eq!((lost, roll), (0.0, 0.0), "280 units/s: no sound, no punch");
}

#[test]
fn the_punch_decays_by_its_length() {
    let (mut sim, p) = standing();
    state_mut(&mut sim, p).punch_roll = 6.5;
    sim.ticks(1);
    // 6.5 - (10 + 0.5 x 6.5) x 0.01.
    assert!((me(&sim, p).punch_roll - 6.3675).abs() < 1e-4, "{}", me(&sim, p).punch_roll);
    state_mut(&mut sim, p).punch_roll = -0.05;
    sim.ticks(1);
    assert_eq!(me(&sim, p).punch_roll, 0.0);
}

#[test]
fn a_jump_above_the_cap_is_cut() {
    // 400 units/s with the knife's 250: over 1.2 x 250, so the take-off is
    // cut to 0.96 x 250 = 240 (the whole velocity, before the jump's).
    let (mut sim, p) = standing();
    set_vel(&mut sim, p, Vec3::new(0.0, 400.0, 0.0));
    sim.intent(p).jump = true;
    sim.ticks(1);
    let v = vel(&sim, p);
    assert!(!me(&sim, p).on_ground);
    assert!((v.truncate().length() - 240.0).abs() < 0.05, "{v}");
    assert!((v.z - (268.328_16 - 8.0)).abs() < 0.01, "the jump's own speed: {}", v.z);
    // Under the cap nothing is cut.
    let (mut sim, p) = standing();
    set_vel(&mut sim, p, Vec3::new(0.0, 290.0, 0.0));
    sim.intent(p).jump = true;
    sim.ticks(1);
    assert!((flat_speed(&sim, p) - 290.0).abs() < 0.05, "{}", flat_speed(&sim, p));
}

#[test]
fn the_ladder() {
    for (pitch, up) in [(0.0f32, 200.0f32), (45.0, 282.84)] {
        let (mut sim, p) = standing();
        place(&mut sim, p, Vec3::new(greybox::LADDER_X, 0.3, greybox::NORTH_WALL_Z + 0.1 + 0.43));
        sim.intent(p).yaw = 0.0;
        sim.intent(p).pitch = pitch.to_radians();
        sim.intent(p).move_axis = Vec2::Y;
        sim.ticks(40);
        let v = vel(&sim, p);
        assert!(sim.state(p).on_ladder, "pitch {pitch}: on the ladder");
        assert!((v.z - up).abs() < 0.5 && v.truncate().length() < 1.0, "pitch {pitch}: {v}");
    }
    // Jump: off it at 270 along its normal (the wall faces +Y in units'
    // -Y: away from the north wall).
    let (mut sim, p) = standing();
    place(&mut sim, p, Vec3::new(greybox::LADDER_X, 0.3, greybox::NORTH_WALL_Z + 0.1 + 0.43));
    sim.intent(p).move_axis = Vec2::Y;
    sim.ticks(40);
    sim.intent(p).move_axis = Vec2::ZERO;
    sim.intent(p).jump = true;
    sim.ticks(1);
    let v = vel(&sim, p);
    assert!(!sim.state(p).on_ladder && (v.truncate().length() - 270.0).abs() < 0.01 && v.y < 0.0, "{v}");
}

#[test]
fn a_hit_slows_and_the_ground_gives_it_back() {
    let (mut sim, p) = running();
    let n = movement::numbers();
    // A large flinch (an AK-47 to the chest): 0.65, no push at 250? The
    // victim is slower than 300, so the push applies too: 170 away.
    {
        let w = sim.app.world_mut();
        let mut q = w.query::<(&mut QuakeMovement, &mut Velocity)>();
        let (mut m, mut v) = q.get_mut(w, p).unwrap();
        movement::hit_slow(&n.hit, &mut m, &mut v, true, Vec3::ZERO);
    }
    assert_eq!(me(&sim, p).velocity_modifier, 0.65);
    sim.ticks(1);
    assert!((me(&sim, p).velocity_modifier - 0.66).abs() < 1e-6);
    assert!((flat_speed(&sim, p) - 250.0 * 0.66).abs() < 0.05, "next command: {}", flat_speed(&sim, p));
    // It gains 0.01 a ground command: 34 more to 1 (an f32's 0.9999996,
    // which the next command tops up).
    sim.ticks(33);
    assert!(me(&sim, p).velocity_modifier < 0.995);
    sim.ticks(2);
    assert_eq!(me(&sim, p).velocity_modifier, 1.0);
    sim.ticks(60);
    assert!((flat_speed(&sim, p) - 250.0).abs() < 0.01, "back at full speed");
    // A small one: 0.5; in the air nothing comes back.
    {
        let w = sim.app.world_mut();
        let mut q = w.query::<(&mut QuakeMovement, &mut Velocity)>();
        let (mut m, mut v) = q.get_mut(w, p).unwrap();
        movement::hit_slow(&n.hit, &mut m, &mut v, false, Vec3::ZERO);
    }
    sim.intent(p).jump = true;
    sim.ticks(1);
    sim.intent(p).jump = false;
    sim.ticks(20);
    assert!(!me(&sim, p).on_ground);
    assert_eq!(me(&sim, p).velocity_modifier, 0.5);
    // The push: 170 units/s away from the shooter below 300.
    let (mut sim, p) = standing();
    {
        let w = sim.app.world_mut();
        let mut q = w.query::<(&mut QuakeMovement, &mut Velocity)>();
        let (mut m, mut v) = q.get_mut(w, p).unwrap();
        movement::hit_slow(&n.hit, &mut m, &mut v, true, Vec3::X);
    }
    assert!((vel(&sim, p).x - 170.0).abs() < 0.01, "{}", vel(&sim, p));
}

#[test]
fn the_walk_key_scales_the_keys() {
    let (mut sim, p) = standing();
    sim.intent(p).move_axis = Vec2::Y;
    sim.intent(p).walk = true;
    sim.ticks(150);
    // The keys' 400 clipped to the 250 cap, then x 0.52 (movement.md
    // "Frame inputs"): 130, not 0.52 x 400.
    assert!((flat_speed(&sim, p) - 130.0).abs() < 0.05, "{}", flat_speed(&sim, p));
}

/// CS 1.6's golden run (docs/plans/active/shared-movement.md, section 7;
/// goldsrc.md, M2): the Source games' course on the greybox with a brush
/// staircase added, its phases half as long again for the 10 ms tick (a
/// run, strafe jumps with a second press in the air, a duck jump, the
/// stairs, a fall with damage, the ladder and a jump off it, a swim and a
/// water jump out of the tank, a crouch walk), every tick's place,
/// velocity, flags, eye, view roll, health and jump penalty hashed. The
/// hash was taken when the movement landed: a change of one float
/// operation changes it; a behaviour change re-takes it in its own
/// commit, saying why (re-taken 2026-11-26: the walk key's 0.52 after the
/// keys' clip, then the swim's GoldSrc rules: the sink, the up key). `CHIMERA_GOLDEN_TRACE=1` prints every tick's row.
/// Asserted on Linux x86-64 only. It prints the step's and the tick's
/// instructions a tick.
#[test]
fn golden_run() {
    use chimera_world::harness::own_process;
    const GOLDEN: u64 = 0x46cd_0acd_f21d_e07d;
    const TICKS: u64 = 1620;
    if !own_process("movement::golden_run") {
        return;
    }
    let mut sim = Sim::new(Cs16Sim, GreyboxMapPlugin);
    // A frame a tick: every `ticks(1)` is one 10 ms command.
    sim.set_tick_interval(cs16::game::TICK_INTERVAL);
    sim.repeatable();
    let p = sim.spawn_character(greybox::SPAWNS[0], ID);
    sim.ticks(2);
    // A brush staircase east of the spawn, rising toward -Z: 8 steps of
    // 12 units by 24, swept exactly (the greybox's own solids are physics
    // colliders).
    const STAIRS: Vec3 = Vec3::new(30.0, 0.0, 10.0);
    let step = 12.0 * UNIT;
    let mut brushes = sim.app.world().resource::<MapBrushes>().0.clone();
    for i in 0..8 {
        let z = STAIRS.z - 2.0 * step * i as f32;
        brushes.push(MapBrush::from_box(
            Vec3::new(STAIRS.x - 1.5, 0.0, z - if i == 7 { 4.0 } else { 2.0 * step }),
            Vec3::new(STAIRS.x + 1.5, step * (i + 1) as f32, z),
        ));
    }
    sim.app.world_mut().insert_resource(MapBrushes(brushes));

    let put = |sim: &mut Sim, at: Vec3| {
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
                // Settle, then a run north (-Z).
                0..45 => {}
                45..195 => i.move_axis = Vec2::Y,
                // Strafe jumps: a jump every 60 ticks and a second press
                // in the air, strafing right while turning right.
                195..435 => {
                    i.move_axis = Vec2::new(1.0, if (t - 195) % 60 < 6 { 1.0 } else { 0.0 });
                    i.jump = matches!((t - 195) % 60, 0 | 21);
                    i.yaw -= 1.0f32.to_radians();
                }
                // A duck jump, then standing up.
                435..525 => {
                    i.move_axis = Vec2::Y;
                    i.jump = t == 438;
                    i.crouch = (444..495).contains(&t);
                }
                // Up the stairs and off their top.
                525..750 => i.move_axis = Vec2::Y,
                // A fall from 9 m, then a walk with the walk key held.
                750..930 => {
                    i.move_axis = Vec2::new(0.0, if t >= 885 { 1.0 } else { 0.0 });
                    i.walk = true;
                }
                // Up the north wall's ladder, a jump off it, away and back
                // to the wall.
                930..1200 => {
                    i.move_axis = Vec2::new(0.0, if t < 975 || t >= 1110 { 1.0 } else { -1.0 });
                    i.pitch = if t < 975 { 0.6 } else { 0.0 };
                    i.jump = t == 975;
                }
                // The tank: sinking, then a swim up and north and out over
                // its wall.
                1200..1530 => {
                    if t >= 1290 {
                        i.move_axis = Vec2::Y;
                        i.jump = true;
                        i.pitch = 0.3;
                    }
                }
                // A crouch walk, let go.
                _ => {
                    i.move_axis = Vec2::new(-1.0, 1.0);
                    i.crouch = t < 1590;
                }
            }
        }
        match t {
            435 | 525 | 750 | 930 | 1200 | 1530 => sim.intent(p).yaw = 0.0,
            _ => {}
        }
        match t {
            525 => put(&mut sim, STAIRS + Vec3::new(0.0, 1.0, 3.0)),
            750 => put(&mut sim, Vec3::new(20.0, 10.0, 20.0)),
            930 => put(&mut sim, Vec3::new(greybox::LADDER_X, 1.0, greybox::NORTH_WALL_Z + 0.1 + 0.43)),
            1200 => put(&mut sim, Vec3::new(tank.x, 4.5, tank.y)),
            1530 => put(&mut sim, greybox::SPAWNS[1]),
            _ => {}
        }
        match sim.work(1) {
            Some(w) => {
                tick_work += w.total();
                step_work += w.crate_systems.get("chimera_movement Movement step").map_or(0, |v| v.0);
            }
            None => counted = false,
        }
        let (at, v, s) = (sim.position(p), sim.velocity(p), sim.state(p));
        let m = me(&sim, p);
        let health = sim.app.world().get::<Health>(p).map_or(0.0, |h| h.current);
        for f in [at.x, at.y, at.z, v.x, v.y, v.z, s.eye_offset.y, s.view_roll, health, m.stamina] {
            add(f.to_bits());
        }
        add(u32::from(s.on_ground)
            | u32::from(s.crouching) << 1
            | u32::from(s.on_ladder) << 2
            | u32::from(m.ducked) << 3
            | u32::from(m.ducking) << 4
            | u32::from(m.water_level) << 8);
        if trace {
            eprintln!(
                "{t} at {at:?} v {v:?} eye {} roll {} hp {health} penalty {} ground {} crouch {} ladder {} ducked {} ducking {} water {}",
                s.eye_offset.y, s.view_roll, m.stamina, s.on_ground, s.crouching, s.on_ladder, m.ducked, m.ducking, m.water_level
            );
        }
        met[0] |= !s.on_ground;
        met[1] |= m.ducked;
        met[2] |= s.on_ladder;
        met[3] |= m.water_level == 3;
        met[4] |= (525..750).contains(&t) && at.y > 2.5;
        met[5] |= health < 1.0;
    }
    eprintln!("cs16 golden run: hash {hash:#018x}, met (air, duck, ladder, swim, stairs, fall damage) {met:?}");
    if counted {
        eprintln!(
            "cs16 golden run work: the step {} instructions a tick, the tick {} ({TICKS} ticks)",
            step_work / TICKS,
            tick_work / TICKS
        );
    } else {
        eprintln!("cs16 golden run work: no instruction counters here, not counted");
    }
    assert_eq!(met, [true; 6], "the course met (air, duck, ladder, swim, stairs, fall damage)");
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        assert_eq!(hash, GOLDEN, "the golden run's hash changed: {hash:#018x} (docs/plans/active/shared-movement.md 7)");
    } else {
        eprintln!("cs16 golden run: the hash is asserted on Linux x86-64 only");
    }
}
