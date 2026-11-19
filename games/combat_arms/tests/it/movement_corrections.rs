//! Combat Arms movement: the spec's correction cases (80-94; specs/
//! combat_arms/movement.md "Corrections (2026-10-19)", each case naming
//! the correction it checks) as scenario tests on `CaSim`, on
//! `movement`'s test map and player (`movement::T`). The volume ones
//! (56-79) are in movement_volumes.rs.

use bevy::prelude::*;
use combat_arms::game::{
    TICK_INTERVAL,
    movement::{self, ModeSpeed, MovementLocked, brake, deflect, movement_data, steep_cut},
};

use crate::{
    movement::{T, TOL, block, close, floor, on},
    movement_volumes::{s, set_vel, slab, still_water_volume},
};

const DT: f32 = 0.015;
/// e^(-k dt) with k = 5.
const E: f32 = 0.927_743_5;

/// Case 80 (Corr. 7): forward and back held from a steady run reset the
/// forward ramp on every tick; releasing back is a reversal too (500,
/// 590, 680). The same on the strafe axis.
#[test]
fn case_80_opposing_keys_then_release_back() {
    for strafe in [false, true] {
        let mut t = T::standing();
        let axis = |on: f32| if strafe { Vec2::new(on, 0.0) } else { Vec2::new(0.0, on) };
        t.keys().move_axis = axis(1.0);
        t.ticks(80);
        close(t.hspeed(), 307.7608, TOL, "steady");
        t.keys().move_axis = Vec2::ZERO;
        if strafe {
            t.keys().opposed.x = true;
        } else {
            t.keys().opposed.y = true;
        }
        for (speed, dist) in [(264.4069, 4.11862), (245.3018, 3.82102), (227.5771, 3.54493)] {
            let before = t.pos();
            t.tick();
            close(t.hspeed(), speed, TOL, &format!("strafe {strafe}: both held"));
            close((t.pos() - before).xz().length(), dist, TOL, "distance");
            assert_eq!(t.mv().accel, Vec3::ZERO);
        }
        t.keys().opposed = BVec2::FALSE;
        t.keys().move_axis = axis(1.0);
        for (accel, speed, dist) in [
            (500.0, 218.3588, 3.34366),
            (590.0, 211.1073, 3.22032),
            (680.0, 205.6803, 3.12540),
        ] {
            let before = t.pos();
            t.tick();
            close(t.mv().accel.length(), accel, TOL, &format!("strafe {strafe}: accel"));
            close(t.hspeed(), speed, TOL, "speed");
            close((t.pos() - before).xz().length(), dist, TOL, "distance");
        }
    }
}

/// Case 81 (Corr. 14): braking keeps the vertical speed unless the
/// horizontal one is already 5 or less.
#[test]
fn case_81_braking_keeps_the_vertical_speed() {
    let d = movement_data();
    let b = |x: f32| brake(Vec3::new(x, -11.547, 0.0), 285.0, DT, d);
    assert_eq!(b(20.0), Vec3::new(0.0, -11.547, 0.0));
    let v = b(60.0);
    close(v.x, 17.25, TOL, "slowed");
    close(v.y, -11.547, TOL, "vertical kept");
    assert_eq!(b(4.0), Vec3::ZERO);
}

/// Case 82 (Corr. 6): a pending jump in the air: no jump, the ramp
/// bypassed (3000 every tick), the jump factor frozen, and the jump on
/// landing without the key.
#[test]
fn case_82_pending_jump() {
    for j in [1.0f32, 0.6] {
        let mut t = T::standing();
        t.place_in_air(on(0.0, 100.0, 0.0));
        t.mv_mut().jump_pending = true;
        t.mv_mut().jump_factor = j;
        t.forward(true);
        let start = t.pos();
        let a = 3000.0 * j.min(1.0);
        for (tick, speed, dist) in [(1, 45.0, 0.16875), (2, 90.0, 1.0125), (3, 135.0, 2.53125)] {
            t.tick();
            let m = t.mv();
            close(m.accel.length(), a, TOL, &format!("j {j}: accel on tick {tick}"));
            close(t.hspeed(), speed * j, TOL, &format!("j {j}: speed after tick {tick}"));
            close((t.pos() - start).xz().length(), dist * j, TOL, "distance");
            close(t.vel().y, -16.5 * tick as f32, TOL, "falling as case 18");
            assert!(m.jump_pending && !m.in_jump);
            close(m.jump_factor, j, 1e-6, "no recovery");
        }
        t.forward(false);
        let mut ticks = 0;
        while !t.mv().in_jump {
            t.tick();
            ticks += 1;
            assert!(ticks < 100, "jumped on landing");
        }
        close(t.vel().y, 330.0 * j - 16.5, TOL, &format!("j {j}: the landing jump"));
        close(t.mv().jump_factor, 0.3, TOL, "j after");
    }
    // A request made in free movement is dropped.
    let mut t = T::standing();
    still_water_volume(&mut t, slab(0.0, 1000.0));
    t.place_in_air(Vec3::new(0.0, 500.0, 0.0));
    t.tick();
    t.mv_mut().jump_pending = true;
    t.tick();
    assert!(!t.mv().jump_pending, "dropped");
}

/// Case 82, the usual way: walking off a ledge and pressing jump on the
/// tick after the move that left it: no jump in the air, a jump on
/// landing without the key.
#[test]
fn case_82_late_jump_at_a_ledge() {
    // A 100 u ledge ending at z = -50.
    let ledge = block(Vec3::new(0.0, 50.0, 500.0), Vec3::new(500.0, 50.0, 550.0));
    let mut t = T::new(vec![floor(), ledge], on(0.0, 100.0, -20.0));
    t.tick();
    t.forward(true);
    let mut ticks = 0;
    while t.mv().standing.is_some() {
        t.tick();
        ticks += 1;
        assert!(ticks < 200, "walked off");
    }
    // The tick after the move that left it: the key reads last tick's
    // ground.
    t.keys().jump = true;
    t.tick();
    let m = t.mv();
    assert!(m.jump_pending && !m.in_jump, "pending in the air");
    t.keys().jump = false;
    let mut ticks = 0;
    while !t.mv().in_jump {
        t.tick();
        ticks += 1;
        assert!(ticks < 100, "jumped on landing");
    }
    assert!(t.feet() < 10.0, "on the floor below: {}", t.feet());
    close(t.vel().y, 330.0 - 16.5, TOL, "the landing jump");
}

/// Case 83 (Corr. 15): the start tick's forward axis is the unramped base
/// and the strafe axis ramped, then rescaled to 500.
#[test]
fn case_83_start_direction() {
    let mut t = T::standing();
    t.keys().move_axis = Vec2::new(1.0, 1.0);
    t.tick();
    let a = t.mv().accel;
    close(-a.z, 490.6023, TOL, "forward");
    close(a.x, 96.4851, TOL, "right");
    close(a.length(), 500.0, TOL, "|a|");
    close(-t.vel().z, 7.0897, TOL, "forward speed");
    close(t.vel().x, 1.3943, TOL, "right speed");
    t.tick();
    let a = t.mv().accel;
    close(-a.z, 590.0, TOL, "tick 2 forward");
    close(a.x, 680.0, TOL, "tick 2 right");
    // In a jump: forward + right pressed on tick 5 after the jump tick.
    let mut t = T::standing();
    t.keys().jump = true;
    t.tick();
    t.release_all();
    t.ticks(4);
    t.keys().move_axis = Vec2::new(1.0, 1.0);
    t.tick();
    let a = t.mv().accel;
    close(-a.z, 84.3039, TOL, "in a jump: forward");
    close(a.x, 165.7976, TOL, "in a jump: right");
}

/// Case 84 (Corr. 6, 9): jump pressed during a plant fires when the lock
/// ends, without the key.
#[test]
fn case_84_jump_pressed_during_a_plant() {
    let mut t = T::standing();
    t.insert(MovementLocked);
    t.tick();
    t.tick();
    t.keys().jump = true;
    t.tick();
    t.keys().jump = false;
    for _ in 4..=9 {
        t.tick();
        assert!(!t.mv().in_jump && t.mv().jump_pending, "no jump while locked");
    }
    t.sim.app.world_mut().entity_mut(t.p).remove::<MovementLocked>();
    t.tick();
    assert!(t.mv().in_jump, "jumped on tick 10");
    close(t.vel().y, 330.0 - 16.5, TOL, "the jump");
}

/// Case 85 (Corr. 8): a release and re-press inside the 100 ms hold is
/// one press: the flag on for ticks 1-7, one charge.
#[test]
fn case_85_crouch_taps_inside_the_release_hold() {
    let mut t = T::standing();
    t.steady_forward();
    let mut flags = Vec::new();
    for tick in 1..=10 {
        t.keys().crouch = matches!(tick, 1 | 3);
        t.tick();
        let m = t.mv();
        flags.push(m.crouch);
        if tick <= 3 {
            close(
                m.lockout,
                [0.6, 0.61, 0.62][tick - 1],
                1e-5,
                &format!("lock-out after tick {tick}"),
            );
            close(m.slowdown, 0.5, 1e-5, &format!("slowdown after tick {tick}"));
        }
    }
    assert!(flags[..7].iter().all(|f| *f), "on for ticks 1-7: {flags:?}");
    assert!(!flags[7], "off from tick 8: {flags:?}");
}

/// Case 86 (Corr. 16): released halfway under a low ceiling, the partly
/// crouched hull isn't tested for room: it stands up into the ceiling.
#[test]
fn case_86_standing_up_halfway_under_a_low_ceiling() {
    let mut t = T::standing();
    t.keys().crouch = true;
    t.tick();
    t.keys().crouch = false;
    // The ceiling 95 u above the floor, now that the hull fits under it.
    let s = s();
    t.sim.app.world_mut().spawn((
        avian3d::prelude::RigidBody::Static,
        avian3d::prelude::Collider::cuboid(1000.0 * s, 20.0 * s, 1000.0 * s),
        Transform::from_translation(Vec3::new(0.0, 105.0, 0.0) * s),
    ));
    let mut rate = vec![t.mv().crouch_rate];
    let mut half = vec![t.mv().half_height];
    let mut flag = vec![t.mv().crouch];
    for tick in 2..=11 {
        t.tick();
        if tick == 2 {
            // The ceiling is in the world's queries now.
            use bevy::ecs::system::RunSystemOnce;
            let up = t
                .sim
                .app
                .world_mut()
                .run_system_once(move |q: avian3d::prelude::SpatialQuery| {
                    let filter = avian3d::prelude::SpatialQueryFilter::default();
                    q.cast_ray(Vec3::new(300.0, 10.0, 300.0) * s, Dir3::Y, 200.0 * s, true, &filter)
                        .map(|h| h.distance / s)
                })
                .unwrap();
            assert!(up.is_some_and(|d| (d - 85.0).abs() < 0.1), "the ceiling at 95: {up:?}");
        }
        rate.push(t.mv().crouch_rate);
        half.push(t.mv().half_height);
        flag.push(t.mv().crouch);
    }
    assert!(flag[..7].iter().all(|f| *f), "held through tick 7: {flag:?}");
    close(rate[6], 0.913022, 1e-5, "rate after tick 7");
    // 50 - 10 r (the spec's 40.87698 is off by 0.007).
    close(half[6], 40.86978, 1e-3, "half-height after tick 7");
    assert!(!flag[7], "tick 8: cleared (not fully crouched, no room test)");
    for (i, (r, h)) in [
        (0.653407, 43.46593),
        (0.449814, 45.50186),
        (0.294467, 47.05533),
        (0.179589, 48.20411),
    ]
    .into_iter()
    .enumerate()
    {
        close(rate[7 + i], r, 1e-5, &format!("rate after tick {}", 8 + i));
        close(half[7 + i], h, 1e-3, &format!("half-height after tick {}", 8 + i));
    }
}

/// Case 87 (Corr. 13): the slide's 0.1 % deflection, and a corner stops
/// a move into it (the crease of two walls is vertical).
#[test]
fn case_87_slide_deflection_and_the_corner() {
    let o = deflect(Vec3::new(3.14510, 0.0, 3.14510), Vec3::new(0.0, 0.0, -1.0));
    close(o.x, 3.14510, 1e-5, "along the wall");
    close(o.y, 0.0, 1e-6, "y");
    close(o.z, -0.00315, 1e-5, "0.1 % off it");
    // Running diagonally into a 90 degree corner (walls at x = 100 and
    // z = -100): stopped there, no slide along either wall.
    let wall_x = block(Vec3::new(150.0, 100.0, 0.0), Vec3::new(50.0, 100.0, 500.0));
    let wall_z = block(Vec3::new(0.0, 100.0, -150.0), Vec3::new(500.0, 100.0, 50.0));
    let mut t = T::new(vec![floor(), wall_x, wall_z], on(0.0, 0.0, 0.0));
    t.keys().move_axis = Vec2::new(1.0, 1.0);
    t.ticks(100);
    let at = t.pos();
    t.ticks(20);
    assert!(
        (t.pos() - at).length() < 1e-3,
        "stopped in the corner: {at} -> {}",
        t.pos()
    );
    close(at.x, 100.0 - 15.0, 0.2, "against the x wall");
    close(at.z, -100.0 + 15.0, 0.2, "against the z wall");
}

/// Case 88 (Corr. 9): movement locked while running: halved and braked
/// each tick, at rest on the third; locked in the air the fall is
/// 2200 u/s^2; the crouch rate still follows the key.
#[test]
fn case_88_movement_locked_while_running() {
    let mut t = T::standing();
    t.steady_forward();
    close(t.hspeed(), 307.7608, TOL, "steady");
    t.insert(MovementLocked);
    t.release_all();
    for (tick, speed, dist) in [(1, 103.1005, 1.60598), (2, 8.1644, 0.12718)] {
        let before = t.pos();
        t.tick();
        assert!(t.mv().locked);
        close(t.hspeed(), speed, TOL, &format!("speed after tick {tick}"));
        close(
            (t.pos() - before).xz().length(),
            dist,
            TOL,
            &format!("distance on tick {tick}"),
        );
    }
    t.tick();
    assert_eq!(t.vel(), Vec3::ZERO, "at rest on tick 3");
    // The crouch rate follows the key.
    t.keys().crouch = true;
    t.tick();
    close(t.mv().crouch_rate, 0.259615, 1e-5, "crouching while locked");
    // In the air.
    let mut t = T::standing();
    t.place_in_air(Vec3::new(0.0, 500.0, 0.0));
    t.tick();
    t.place_in_air(Vec3::new(0.0, 500.0, 0.0));
    t.insert(MovementLocked);
    let start = t.pos().y;
    for (tick, speed, fallen) in [(1, -33.0, 0.12375), (2, -66.0, 0.7425), (3, -99.0, 1.85625)] {
        t.tick();
        close(t.vel().y, speed, TOL, &format!("air speed after tick {tick}"));
        close(start - t.pos().y, fallen, TOL, &format!("fallen after tick {tick}"));
    }
}

/// The bomb plant's action gauge locks the planter's movement.
#[test]
fn planting_locks_movement() {
    use combat_arms::game::rules::objectives::{ActionKind, CaAction};
    let mut t = T::standing();
    t.insert(CaAction {
        kind: ActionKind::Plant,
        held: 0.0,
        length: 4.0,
    });
    t.forward(true);
    t.ticks(5);
    assert!(t.mv().locked && !t.mv().forward);
    assert_eq!(t.hspeed(), 0.0, "doesn't move");
    t.sim.app.world_mut().entity_mut(t.p).remove::<CaAction>();
    t.ticks(5);
    assert!(t.hspeed() > 0.0, "free again");
}

/// Case 89 (Corr. 17): a move fully blocked in a corner: in a jump (not
/// on ground before the move) the horizontal speed goes, though the hull
/// stands after it; on walkable ground it stays (the vertical rule
/// applies instead).
#[test]
fn case_89_stuck_at_a_wall_in_a_jump() {
    for jumping in [true, false] {
        // Walls touching the hull's sides at +x and +z.
        let wall_x = block(Vec3::new(65.1, 100.0, 0.0), Vec3::new(50.0, 100.0, 500.0));
        let wall_z = block(Vec3::new(0.0, 100.0, 65.1), Vec3::new(500.0, 100.0, 50.0));
        let mut t = T::new(vec![floor(), wall_x, wall_z], on(0.0, 0.0, 0.0));
        t.tick();
        let at = t.pos();
        if jumping {
            t.mv_mut().in_jump = true;
            t.mv_mut().standing = None;
        }
        set_vel(&mut t, Vec3::new(100.0, -50.0, 100.0));
        t.tick();
        assert!((t.pos() - at).length() < 0.01, "blocked: {at} -> {}", t.pos());
        let v = t.vel();
        if jumping {
            assert_eq!(v.xz(), Vec2::ZERO, "in a jump: horizontal speed zeroed");
        } else {
            assert!(v.xz().length() > 1.0, "on ground: kept ({v})");
            assert_eq!(v.y, 0.0, "the vertical rule");
        }
    }
}

/// A slope rising toward -Z whose normal has `ny` as its Y.
fn slope(ny: f32) -> crate::movement::Block {
    crate::movement::tilted(Vec3::ZERO, Vec3::new(300.0, 10.0, 1000.0), ny.acos())
}

/// Case 90 (Corr. 18): the movement's on-ground needs n.y >= 0.707, the
/// integrator's walkable 0.7071: at 0.70705 a jump starts though the
/// integrator slides; at 0.70695 neither.
#[test]
fn case_90_the_on_ground_threshold() {
    for (ny, ground) in [(0.70705f32, true), (0.70695, false)] {
        let surface = (10.0 + (1.0 - ny * ny).sqrt() * 100.0) / ny;
        let mut t = T::new(vec![floor(), slope(ny)], Vec3::new(0.0, surface + 60.0, -100.0));
        for _ in 0..60 {
            if t.mv().standing.is_some() {
                break;
            }
            t.tick();
        }
        t.tick();
        let m = t.mv();
        assert!(
            m.standing.is_some_and(|s| (s.normal.y - ny).abs() < 1e-4),
            "{ny}: {:?}",
            m.standing
        );
        assert_eq!(m.on_ground, ground, "{ny}: on ground");
        close(m.friction_k, 5.0, 1e-6, "k");
        let y = t.pos().y;
        t.keys().jump = true;
        t.tick();
        assert_eq!(t.mv().in_jump, ground, "{ny}: jumps");
        if !ground {
            t.ticks(4);
            assert!(t.pos().y < y - 0.5, "{ny}: slides down");
        }
    }
}

/// Case 91 (Corr. 20): the gauge advances at most 100 ms a step: a 250 ms
/// tick drains 3.0 sprinting and refills 2.5.
#[test]
fn case_91_server_stamina_steps() {
    let mut t = T::standing();
    t.insert(movement_data().character.clone());
    t.sim.set_tick_interval(0.25);
    t.forward(true);
    t.keys().sprint = true;
    t.tick();
    assert!(t.mv().sprint);
    close(t.mv().stamina, 97.0, 1e-4, "drain");
    t.keys().sprint = false;
    t.tick();
    close(t.mv().stamina, 99.5, 1e-4, "refill");
    t.sim.set_tick_interval(TICK_INTERVAL);
}

/// Case 92 (Corr. 2): a thin ladder (x 0..10) acts when the hull's box
/// touches it: centre at x = -10 (box edge 5) climbs, at -15.5 not.
#[test]
fn case_92_ladder_by_the_box() {
    for (x, ladder) in [(-10.0, true), (-15.5, false)] {
        let mut t = T::new(vec![floor()], on(x, 0.0, 0.0));
        t.tick();
        crate::movement_volumes::volume(
            &mut t,
            "Ladder",
            Vec3::new(0.0, -100.0, -500.0),
            Vec3::new(10.0, 1000.0, 500.0),
            |_| {},
        );
        t.keys().pitch = std::f32::consts::FRAC_PI_2;
        t.forward(true);
        t.tick();
        assert_eq!(t.mv().on_ladder, ladder, "x {x}");
        if ladder {
            close(t.vel().y, 7.5, TOL, "climbing (case 56)");
        } else {
            assert_eq!(t.vel().y, 0.0);
        }
    }
}

/// Case 93 (Corr. 19): resting on another character: not on ground (no
/// braking, no jump), but ground friction with a flat normal.
#[test]
fn case_93_standing_on_another_player() {
    let mut t = T::standing();
    let s = s();
    let other = t.sim.spawn_character(on(0.0, 0.0, 0.0) * s, movement::ID);
    t.sim
        .app
        .world_mut()
        .entity_mut(other)
        .insert(movement_data().character.without_gear());
    t.place_in_air(on(0.0, 100.2, 0.0));
    t.ticks(30);
    let m = t.mv();
    assert!(
        m.standing.is_some_and(|g| g.character),
        "on the other's head: {:?} at {}",
        m.standing,
        t.pos()
    );
    assert!(!m.on_ground);
    set_vel(&mut t, Vec3::new(100.0, 0.0, 0.0));
    let before = t.pos();
    t.tick();
    close(t.vel().x, 100.0 * E, TOL, "friction, no braking");
    close(t.pos().x - before.x, 100.0 * (1.0 - E) / 5.0, TOL, "distance");
    t.keys().jump = true;
    t.tick();
    assert!(!t.mv().in_jump, "no jump off a head");
}

/// Case 94 (Corr. 13): up a steep slope only the level part of the move
/// is left (vertical part dropped); down-hill unchanged.
#[test]
fn case_94_up_a_steep_slope() {
    let n = Vec3::new(0.8, 0.6, 0.0);
    let w = movement_data().walkable_normal;
    let up = steep_cut(n, Vec3::new(-1.0, -0.5, 0.3), w);
    assert!((up - Vec3::new(0.0, 0.0, 0.3)).length() < 1e-6, "{up}");
    let down = Vec3::new(1.0, -0.5, 0.3);
    assert_eq!(steep_cut(n, down, w), down);
}

/// Corrections 21: a mode's speed factor scales the limit and the
/// stamina capacity.
#[test]
fn mode_speed_factors() {
    let mut t = T::standing();
    t.insert(ModeSpeed {
        speed: 0.8,
        stamina: 1.5,
    });
    t.insert(movement_data().character.clone());
    t.steady_forward();
    // The reference character's vest (P = 0.1): 285 x 0.9 x 0.8.
    close(t.mv().speed_limit, 205.2, TOL, "limit");
    t.ticks(400);
    close(t.mv().stamina, 150.0, TOL, "capacity");
}

/// Corrections 22: the knock-back's crouched variants read the crouch
/// flag: a tap held on by the release hold still stops the push.
#[test]
fn knock_back_reads_the_crouch_flag() {
    use movement::{CaInfected, HitKnockback};
    let mut t = T::standing();
    t.keys().crouch = true;
    t.tick();
    t.keys().crouch = false;
    let k = HitKnockback::hit(None, 40.0, 0.3, 0.0, Vec3::Z, 0.0).unwrap();
    t.insert((CaInfected, k));
    t.tick();
    assert!(t.mv().crouch, "held by the release delay");
    assert_eq!(t.mv().accel, Vec3::ZERO, "no push");
    assert_eq!(t.hspeed(), 0.0);
}
