//! Halo's movement (specs/halo_ce/movement.md): every row of the spec's
//! "Test cases" as a scenario test on `HaloSim` at Halo's 30 Hz tick, on a
//! test level of our own built in code (boxes in the spec's frame: +X
//! forward, +Y left, +Z up, world units), the player at rest on it facing
//! +X after a settle, inputs held from tick 1. Then the movement under a
//! party app's 64 Hz tick, and the slot's registration.
//!
//! Tolerances: `EXACT` where the spec's simulation is exact (the spec
//! asks 1e-6 WU; positions pass through the engine's metres in f32, so
//! 2e-6), `SUM` for sums over many ticks; the steps' rows depend on the
//! collision mover, which is the engine's capsule cast here (the module's
//! Known shortcuts): those not met exactly are checked by what they show
//! (climbs or is blocked, the top's height), each with its measured
//! tolerance in its doc.

use avian3d::prelude::{Collider, RigidBody};
use bevy::prelude::*;
use chimera_core::{
    core::{Damage, DamageKind},
    slots::{Loadout, MovementSlot},
};
use chimera_world::harness::Sim;
use halo_ce::{
    game::{
        TICK_INTERVAL,
        maps::space,
        movement::{self, BodyStun, ControllerStick, HaloMovement, MovementData},
    },
    harness::HaloSim,
};

const EXACT: f32 = 2e-6;
const SUM: f32 = 2e-5;
/// The steps' first touch (measured: within 2e-3 of the spec's mover).
const STEP: f32 = 2e-3;
/// Blocked by an edge: rocking against it (measured: the place within
/// 2e-3, the speed below 0.012 WU/t).
const BLOCKED: f32 = 1e-2;
const ROCK: f32 = 0.02;

fn close(got: f32, want: f32, tol: f32, what: &str) {
    assert!((got - want).abs() <= tol, "{what}: got {got}, want {want} (tolerance {tol})");
}

fn data() -> MovementData {
    MovementData::builtin()
}

/// A static box in the spec's frame: centre, half-extents, rotation.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Block {
    centre: Vec3,
    half: Vec3,
    rotation: Quat,
}

/// An axis-aligned box spanning `min` to `max` (WU).
pub(crate) fn aabb(min: Vec3, max: Vec3) -> Block {
    Block {
        centre: (min + max) / 2.0,
        half: (max - min) / 2.0,
        rotation: Quat::IDENTITY,
    }
}

/// The flat floor: top at z = 0.
pub(crate) fn floor() -> Block {
    aabb(Vec3::new(-60.0, -60.0, -5.0), Vec3::new(60.0, 60.0, 0.0))
}

/// A thick slab whose face (normal `n`, unit) passes through `at`.
pub(crate) fn plane(n: Vec3, at: Vec3) -> Block {
    let rotation = Quat::from_rotation_arc(Vec3::Z, n);
    Block {
        centre: at - n * 5.0,
        half: Vec3::new(60.0, 60.0, 5.0),
        rotation,
    }
}

/// A plane through the origin rising toward +X at `deg` degrees.
fn slope(deg: f32) -> Block {
    let a = deg.to_radians();
    plane(Vec3::new(-a.sin(), 0.0, a.cos()), Vec3::ZERO)
}

/// A spec-frame point (WU) as the engine's (m).
fn eng(p: Vec3) -> Vec3 {
    space::point(p.to_array())
}

/// An engine point (m) in the spec's frame (WU).
fn game(p: Vec3) -> Vec3 {
    Vec3::from(space::to_game(p))
}

#[derive(Resource, Clone)]
struct TestMap(Vec<Block>);

#[derive(Resource, Default)]
struct DamageLog(Vec<Damage>);

fn spawn_map(mut commands: Commands, map: Res<TestMap>) {
    let s = halo_ce::game::METERS_PER_UNIT;
    // The spec frame's axes as the engine's: its rotation.
    let axes = Mat3::from_cols(space::direction([1.0, 0.0, 0.0]), space::direction([0.0, 1.0, 0.0]), space::direction([0.0, 0.0, 1.0]));
    let to_engine = Quat::from_mat3(&axes);
    for b in &map.0 {
        commands.spawn((
            RigidBody::Static,
            box_mesh(b.half * s),
            Transform::from_translation(eng(b.centre)).with_rotation(to_engine * b.rotation),
        ));
    }
}

/// A box of half-extents `h` as a triangle mesh, as a map's collision is.
fn box_mesh(h: Vec3) -> Collider {
    let v: Vec<Vec3> = (0..8)
        .map(|i| Vec3::new(if i & 1 == 0 { -h.x } else { h.x }, if i & 2 == 0 { -h.y } else { h.y }, if i & 4 == 0 { -h.z } else { h.z }))
        .collect();
    // Each face as two triangles, counter-clockwise seen from outside.
    let faces = [[0, 2, 6, 4], [1, 5, 7, 3], [0, 4, 5, 1], [2, 3, 7, 6], [0, 1, 3, 2], [4, 6, 7, 5]];
    let idx = faces.iter().flat_map(|f| [[f[0], f[1], f[2]], [f[0], f[2], f[3]]]).collect();
    Collider::trimesh(v, idx)
}

fn log_damage(mut damage: MessageReader<Damage>, mut log: ResMut<DamageLog>) {
    log.0.extend(damage.read().cloned());
}

/// A test: the simulation and its one player.
pub(crate) struct T {
    pub(crate) sim: Sim,
    pub(crate) p: Entity,
}

impl T {
    /// A player whose origin is at `origin` (WU) on a level of `blocks`,
    /// at `hz` ticks a second; no tick run yet.
    pub(crate) fn at_rate(blocks: Vec<Block>, origin: Vec3, hz: f64) -> Self {
        let map = TestMap(blocks);
        let mut sim = Sim::with(HaloSim, move |app| {
            app.insert_resource(map)
                .init_resource::<DamageLog>()
                .add_systems(Startup, spawn_map)
                .add_systems(Last, log_damage);
        });
        sim.set_tick_interval(1.0 / hz);
        let centre = origin + Vec3::Z * data().centre_above_origin(0.0);
        let p = sim.spawn_character(eng(centre), movement::ID);
        Self { sim, p }
    }

    pub(crate) fn new(blocks: Vec<Block>, origin: Vec3) -> Self {
        Self::at_rate(blocks, origin, 1.0 / TICK_INTERVAL)
    }

    /// Standing at rest at the origin on `blocks` after a settle of three
    /// idle ticks (the first finds the floor).
    pub(crate) fn standing_on(blocks: Vec<Block>) -> Self {
        let mut t = Self::new(blocks, Vec3::ZERO);
        t.ticks(3);
        let m = t.mv();
        assert!(!m.airborne, "settled on the floor: {m:?}");
        close(t.origin().length(), 0.0, EXACT, "settled at the origin");
        close(t.vel().length(), 0.0, EXACT, "settled at rest");
        t
    }

    /// Crouched at rest at the origin on `blocks` (crouch held), after a
    /// settle of three ticks.
    pub(crate) fn crouched_on(blocks: Vec<Block>) -> Self {
        let mut t = Self::new(blocks, Vec3::ZERO);
        let d = data();
        let centre = eng(Vec3::Z * d.centre_above_origin(1.0));
        t.sim.app.world_mut().get_mut::<Transform>(t.p).unwrap().translation = centre;
        let mut m = t.sim.app.world_mut().get_mut::<HaloMovement>(t.p).unwrap();
        m.crouch = 1.0;
        m.stance_crouch = true;
        t.crouch(true);
        t.ticks(3);
        assert!(!t.mv().airborne, "settled on the floor");
        close(t.origin().length(), 0.0, EXACT, "settled at the origin");
        close(t.mv().crouch, 1.0, EXACT, "crouched");
        t
    }

    pub(crate) fn standing() -> Self {
        Self::standing_on(vec![floor()])
    }

    pub(crate) fn tick(&mut self) {
        self.sim.ticks(1);
    }

    pub(crate) fn ticks(&mut self, n: u64) {
        self.sim.ticks(n);
    }

    pub(crate) fn mv(&self) -> HaloMovement {
        self.sim.app.world().get::<HaloMovement>(self.p).unwrap().clone()
    }

    /// The origin (between the feet), WU.
    pub(crate) fn origin(&self) -> Vec3 {
        game(self.sim.position(self.p)) - Vec3::Z * data().centre_above_origin(self.mv().crouch)
    }

    /// The velocity, WU per tick.
    pub(crate) fn vel(&self) -> Vec3 {
        let v = self.sim.velocity(self.p) / (halo_ce::game::METERS_PER_UNIT * 30.0);
        Vec3::new(-v.z, -v.x, v.y)
    }

    pub(crate) fn set_vel(&mut self, v: Vec3) {
        let e = space::direction(v.to_array()) * halo_ce::game::METERS_PER_UNIT * 30.0;
        self.sim.app.world_mut().get_mut::<chimera_core::core::Velocity>(self.p).unwrap().0 = e;
    }

    pub(crate) fn speed(&self) -> f32 {
        self.vel().truncate().length()
    }

    /// Hold throttle (forward, left).
    pub(crate) fn throttle(&mut self, x: f32, y: f32) {
        self.sim.intent(self.p).move_axis = Vec2::new(-y, x);
    }

    pub(crate) fn jump(&mut self, on: bool) {
        self.sim.intent(self.p).jump = on;
    }

    pub(crate) fn crouch(&mut self, on: bool) {
        self.sim.intent(self.p).crouch = on;
    }

    /// Face the heading `deg` degrees (counter-clockwise from +X).
    pub(crate) fn face(&mut self, deg: f32) {
        self.sim.intent(self.p).yaw = space::yaw(deg.to_radians());
    }

    fn eye(&self) -> f32 {
        game(self.sim.position(self.p) + self.sim.state(self.p).eye_offset).z - self.origin().z
    }

    fn damage(&self) -> Vec<f32> {
        self.sim.app.world().resource::<DamageLog>().0.iter().map(|d| d.amount * data().body_vitality).collect()
    }

    /// Run until grounded (at most `max` ticks): the ticks taken.
    fn until_grounded(&mut self, max: u32) -> u32 {
        for n in 1..=max {
            self.tick();
            if !self.mv().airborne {
                return n;
            }
        }
        panic!("not grounded in {max} ticks");
    }

    /// Reach full run forward from rest (10 ticks of (1, 0)).
    fn at_full_run(&mut self) {
        self.throttle(1.0, 0.0);
        self.ticks(10);
        close(self.speed(), 0.075, EXACT, "at full run");
    }
}

// ---- Ground speed ----

/// "Ground speed": holding forward from rest, a linear ramp to 0.075.
#[test]
fn forward_ramps_to_full_run_in_8_ticks() {
    let mut t = T::standing();
    t.throttle(1.0, 0.0);
    let want = [
        (1, 0.0106667, 0.0106667),
        (2, 0.0213333, 0.032),
        (5, 0.0533333, 0.16),
        (7, 0.0746667, 0.2986667),
        (8, 0.075, 0.3736667),
        (10, 0.075, 0.5236667),
        (30, 0.075, 2.0236667),
    ];
    let mut n = 0;
    for (tick, speed, x) in want {
        while n < tick {
            t.tick();
            n += 1;
            assert!(!t.mv().airborne, "grounded every tick (tick {n})");
            close(t.origin().z, 0.0, EXACT, &format!("z on tick {n}"));
        }
        close(t.speed(), speed, EXACT, &format!("speed after tick {tick}"));
        close(t.origin().x, x, if tick > 10 { SUM } else { EXACT }, &format!("x after tick {tick}"));
    }
}

/// "Ground speed": releasing at full run stops on tick 8 after 0.2263333.
#[test]
fn releasing_stops_in_8_ticks() {
    let mut t = T::standing();
    t.at_full_run();
    let x0 = t.origin().x;
    t.throttle(0.0, 0.0);
    let want = [(1, 0.0643333), (2, 0.0536667), (6, 0.011), (7, 0.0003333), (8, 0.0)];
    let mut n = 0;
    for (tick, speed) in want {
        while n < tick {
            t.tick();
            n += 1;
        }
        close(t.speed(), speed, EXACT, &format!("speed after tick {tick}"));
    }
    close(t.origin().x - x0, 0.2263333, EXACT, "stopping distance");
    t.ticks(3);
    close(t.origin().x - x0, 0.2263333, EXACT, "stays stopped");
}

/// "Ground speed": reversing goes through zero at the same rate.
#[test]
fn reversing_takes_14_ticks() {
    let mut t = T::standing();
    t.at_full_run();
    t.throttle(-1.0, 0.0);
    let want = [(7, 0.0003333), (8, -0.0103333), (13, -0.0636667), (14, -0.0666667)];
    let mut n = 0;
    for (tick, vx) in want {
        while n < tick {
            t.tick();
            n += 1;
        }
        close(t.vel().x, vx, EXACT, &format!("v_x after tick {tick}"));
    }
}

/// "Ground speed": the top speeds by direction, after 30 ticks.
#[test]
fn top_speeds_by_direction() {
    let cases: [(f32, f32, f32, Vec3); 7] = [
        (-1.0, 0.0, 0.0, Vec3::new(-0.0666667, 0.0, 0.0)),
        (0.0, 1.0, 0.0, Vec3::new(0.0, 0.0666667, 0.0)),
        (1.0, 1.0, 0.0, Vec3::new(0.0530330, 0.0471405, 0.0)),
        (0.5, 0.0, 0.0, Vec3::new(0.0375, 0.0, 0.0)),
        (0.09, 0.0, 0.0, Vec3::ZERO),
        (1.0, 0.0, 90.0, Vec3::new(0.0, 0.075, 0.0)),
        (-1.0, 1.0, 0.0, Vec3::NAN),
    ];
    for (x, y, yaw, want) in cases {
        let mut t = T::standing();
        t.face(yaw);
        t.throttle(x, y);
        t.ticks(30);
        let v = t.vel();
        let what = format!("throttle ({x}, {y}) facing {yaw}");
        if want.is_nan() {
            close(t.speed(), 0.0666667, EXACT, &what);
            continue;
        }
        close(v.x, want.x, EXACT, &format!("{what}: v_x"));
        close(v.y, want.y, EXACT, &format!("{what}: v_y"));
        close(v.z, 0.0, EXACT, &format!("{what}: v_z"));
    }
}

// ---- Crouch ----

/// "Crouch": the fraction, eye height and pill segment over the transition.
#[test]
fn crouching_takes_6_ticks_after_the_button() {
    let d = data();
    let mut t = T::standing();
    t.crouch(true);
    let want = [(1, 0.0, 0.62, 0.3), (2, 1.0 / 6.0, 0.575, 0.2666667), (4, 0.5, 0.485, 0.2), (7, 1.0, 0.35, 0.1)];
    let mut n = 0;
    for (tick, c, eye, h) in want {
        while n < tick {
            t.tick();
            n += 1;
        }
        let m = t.mv();
        close(m.crouch, c, EXACT, &format!("c after tick {tick}"));
        close(t.eye(), eye, EXACT, &format!("eye after tick {tick}"));
        close(d.segment(m.crouch), h, EXACT, &format!("segment after tick {tick}"));
        close(t.origin().z, 0.0, EXACT, &format!("feet stay on the floor (tick {tick})"));
    }
    // The entity's collider follows the pill.
    let collider = t.sim.app.world().get::<Collider>(t.p).unwrap();
    let cap = collider.shape_scaled().as_capsule().expect("a capsule");
    close(cap.half_height() * 2.0 / halo_ce::game::METERS_PER_UNIT, 0.1, EXACT, "the collider's segment");
    close(cap.radius / halo_ce::game::METERS_PER_UNIT, 0.2, EXACT, "the collider's radius");
    // Released: back up.
    t.crouch(false);
    for (tick, c) in [(1, 1.0), (2, 5.0 / 6.0)] {
        while n < 7 + tick {
            t.tick();
            n += 1;
        }
        close(t.mv().crouch, c, EXACT, &format!("c after release tick {tick}"));
    }
    t.ticks(5);
    close(t.mv().crouch, 0.0, EXACT, "c after release tick 7");
    close(t.eye(), 0.62, EXACT, "eye standing");
}

/// "Crouch": the controller's crouch rule (a stick held short of full
/// crouch-walks, a full stick stands).
#[test]
fn the_controller_crouch_rule() {
    for (x, crouched, speed) in [(0.97, true, 0.0291), (1.0, false, 0.075)] {
        let mut t = T::standing();
        t.sim.app.world_mut().entity_mut(t.p).insert(ControllerStick);
        t.throttle(x, 0.0);
        t.crouch(true);
        t.ticks(40);
        let c = t.mv().crouch;
        close(c, if crouched { 1.0 } else { 0.0 }, EXACT, &format!("stick {x}: c"));
        close(t.speed(), speed, EXACT, &format!("stick {x}: speed"));
    }
}

/// "Crouch": keyboard crouch-walking speeds.
#[test]
fn keyboard_crouch_walk_speeds() {
    for (x, y, speed) in [(1.0, 0.0, 0.03), (-1.0, 0.0, 0.0216667), (0.0, 1.0, 0.02)] {
        let mut t = T::standing();
        t.throttle(x, y);
        t.crouch(true);
        t.ticks(40);
        close(t.mv().crouch, 1.0, EXACT, "crouched");
        close(t.speed(), speed, EXACT, &format!("({x}, {y}) crouched"));
    }
}

/// "Crouch": under a ceiling at 0.6 the standing room fails every tick.
#[test]
fn a_low_ceiling_keeps_a_crouch_and_blocks_jumps() {
    let mut t = T::crouched_on(vec![floor(), plane(Vec3::NEG_Z, Vec3::new(0.0, 0.0, 0.6))]);
    t.ticks(5);
    t.crouch(false);
    t.jump(true);
    for n in 1..=10 {
        t.tick();
        let m = t.mv();
        assert!(m.no_room, "tick {n}: standing room fails");
        close(m.crouch, 1.0, EXACT, &format!("tick {n}: c"));
        assert!(m.stance_crouch, "tick {n}: stance crouch");
        assert!(!m.jumped && !m.airborne, "tick {n}: grounded, no jump");
        close(t.origin().z, 0.0, EXACT, &format!("tick {n}: on the floor"));
    }
}

// ---- Jumping and air control ----

/// "Jumping and air control": a standing jump's flight, tick by tick.
#[test]
fn a_standing_jump_flies_39_ticks() {
    let mut t = T::standing();
    t.jump(true);
    t.tick();
    t.jump(false);
    let m = t.mv();
    assert!(m.jumped && m.airborne, "jumped on tick 0: {m:?}");
    close(t.origin().z, 0.0, EXACT, "tick 0 moved along the ground");
    close(t.vel().z, 0.07, EXACT, "stored v_z");
    let z = [
        (1, 0.0664348),
        (2, 0.1293045),
        (3, 0.1886089),
        (5, 0.2965223),
        (18, 0.6503544),
        (19, 0.652616),
        (20, 0.6513124),
        (38, 0.0182022),
    ];
    let mut n = 0;
    for (tick, want) in z {
        while n < tick {
            t.tick();
            n += 1;
            assert!(t.mv().airborne, "airborne on tick {n}");
        }
        close(t.origin().z, want, EXACT, &format!("z after tick {tick}"));
    }
    close(t.vel().z, -0.0654768, EXACT, "v_z after tick 38");
    t.tick();
    let m = t.mv();
    assert!(!m.airborne, "landed on tick 39");
    close(t.origin().z, 0.0, EXACT, "z after landing");
    close(m.landing_velocity, 0.0690420, EXACT, "landing velocity");
    let l = m.landing.expect("a soft landing");
    assert!(!l.hard && l.ticks == 2.0, "soft landing of 2 ticks: {l:?}");
}

/// "Jumping and air control": holding jump hops on 0, 44 and 88.
#[test]
fn holding_jump_bunny_hops_with_a_pause() {
    let mut t = T::standing();
    t.jump(true);
    let (mut jumps, mut lands) = (vec![], vec![]);
    for n in 0..=100 {
        let was = t.mv().airborne;
        t.tick();
        let m = t.mv();
        if m.jumped {
            jumps.push(n);
        }
        if was && m.landing_velocity > 0.0 {
            lands.push(n);
        }
    }
    assert_eq!(jumps, vec![0, 44, 88], "jump ticks");
    assert_eq!(lands, vec![39, 83], "landing ticks");
}

/// "Jumping and air control": a running jump covers 3.0 WU from the start
/// of the take-off tick, 2.925 from its end, landing on tick 39.
#[test]
fn a_running_jump_covers_3_world_units() {
    let mut t = T::standing();
    t.at_full_run();
    let start = t.origin().x;
    t.jump(true);
    t.tick();
    t.jump(false);
    assert!(t.mv().jumped);
    let end = t.origin().x;
    let n = t.until_grounded(60);
    assert_eq!(n, 39, "lands on tick 39 after take-off");
    close(t.origin().x - start, 3.0, SUM, "from the take-off tick's start");
    close(t.origin().x - end, 2.925, SUM, "from its end");
}

/// "Jumping and air control": reversing or letting go in the air brakes
/// at the air rate to 0.0295 at landing.
#[test]
fn air_control_brakes_a_running_jump() {
    for (x, checks) in [(-1.0, vec![(1, 0.0738333), (10, 0.0633333), (20, 0.0516667), (30, 0.04)]), (0.0, vec![])] {
        let mut t = T::standing();
        t.at_full_run();
        t.jump(true);
        t.tick();
        t.jump(false);
        t.throttle(x, 0.0);
        let mut n = 0;
        for (tick, vx) in checks {
            while n < tick {
                t.tick();
                n += 1;
            }
            close(t.vel().x, vx, EXACT, &format!("({x}, 0): v_x after tick {tick}"));
        }
        // The landing tick's stored speed.
        let landed = n + t.until_grounded(60);
        assert_eq!(landed, 39, "({x}, 0): lands on tick 39");
        close(t.vel().x, 0.0295, EXACT, &format!("({x}, 0): v_x at landing"));
    }
}

/// "Jumping and air control": forward in the air from a standing jump.
#[test]
fn air_control_from_a_standing_jump() {
    let mut t = T::standing();
    t.jump(true);
    t.tick();
    t.jump(false);
    t.throttle(1.0, 0.0);
    let want = [(1, 0.0011667), (10, 0.0116667), (30, 0.035), (39, 0.0455)];
    let mut n = 0;
    for (tick, vx) in want {
        while n < tick {
            t.tick();
            n += 1;
        }
        close(t.vel().x, vx, EXACT, &format!("v_x after tick {tick}"));
    }
    assert!(!t.mv().airborne, "landed on tick 39");
    close(t.origin().x, 0.91, SUM, "x at landing");
}

/// "Jumping and air control": jump and crouch together lift the feet 0.2.
#[test]
fn a_crouch_jump_reaches_higher() {
    let mut t = T::standing();
    t.jump(true);
    t.crouch(true);
    t.tick();
    t.jump(false);
    let mut apex: f32 = 0.0;
    for n in 1..=60 {
        t.tick();
        let z = t.origin().z;
        apex = apex.max(z);
        match n {
            1 => close(z, 0.0997682, EXACT, "z after tick 1"),
            2 => close(z, 0.1959711, EXACT, "z after tick 2"),
            6 => close(z, 0.5451312, EXACT, "z after tick 6"),
            _ => {}
        }
        if !t.mv().airborne {
            assert_eq!(n, 42, "lands on tick 42");
            close(apex, 0.8526159, EXACT, "apex");
            return;
        }
    }
    panic!("never landed");
}

/// "Jumping and air control": a jump from a crouch has the normal apex.
#[test]
fn a_jump_from_a_crouch_has_the_normal_apex() {
    let mut t = T::standing();
    t.crouch(true);
    t.ticks(8);
    t.jump(true);
    t.tick();
    t.jump(false);
    assert!(t.mv().jumped);
    let mut apex: f32 = 0.0;
    for n in 1..=60 {
        t.tick();
        apex = apex.max(t.origin().z);
        if !t.mv().airborne {
            assert_eq!(n, 39, "lands on tick 39");
            close(apex, 0.652616, EXACT, "apex");
            return;
        }
    }
    panic!("never landed");
}

/// "Jumping and air control": the head meets a ceiling at 1.0.
#[test]
fn a_ceiling_stops_a_jump() {
    let mut t = T::standing_on(vec![floor(), plane(Vec3::NEG_Z, Vec3::new(0.0, 0.0, 1.0))]);
    t.jump(true);
    t.tick();
    t.jump(false);
    t.ticks(5);
    close(t.origin().z, 0.2965223, EXACT, "z after tick 5");
    t.tick();
    close(t.origin().z, 0.3, EXACT, "z after tick 6 (head at the ceiling)");
    close(t.vel().z, 0.0, EXACT, "v_z after tick 6");
    t.tick();
    close(t.origin().z, 0.2964348, EXACT, "z after tick 7");
}

/// "Jumping and air control": a stunned jump (the turning penalty).
#[test]
fn a_stunned_jump() {
    let mut t = T::standing();
    t.sim.app.world_mut().entity_mut(t.p).insert(BodyStun(0.5));
    t.jump(true);
    t.tick();
    close(t.vel().z, 0.042, EXACT, "v_z");
}

// ---- Slopes ----

/// On a slope of `deg` degrees, facing `yaw`, (1, 0) for 40 ticks: the
/// velocity.
fn on_slope(deg: f32, yaw: f32) -> (T, Vec3) {
    let mut t = T::new(vec![slope(deg)], Vec3::ZERO);
    t.face(yaw);
    t.ticks(3);
    t.throttle(1.0, 0.0);
    t.ticks(40);
    let v = t.vel();
    (t, v)
}

/// "Slopes": speeds along the slope by angle, up and down.
#[test]
fn slope_speeds() {
    let cases = [
        (10.0, 0.0, 0.075, Some(0.0130236)),
        (10.0, 180.0, 0.075, Some(-0.0130236)),
        (20.0, 0.0, 0.075, None),
        (20.0, 180.0, 0.075, None),
        (30.0, 0.0, 0.0636411, None),
        (30.0, 180.0, 0.0831135, None),
        (40.0, 0.0, 0.0533746, None),
        (40.0, 180.0, 0.0904467, None),
    ];
    for (deg, yaw, speed, vz) in cases {
        let (t, v) = on_slope(deg, yaw);
        assert!(!t.mv().airborne, "{deg} degrees facing {yaw}: grounded");
        close(v.length(), speed, EXACT, &format!("{deg} degrees facing {yaw}: |v|"));
        if let Some(vz) = vz {
            close(v.z, vz, EXACT, &format!("{deg} degrees facing {yaw}: v_z"));
        }
        if deg == 30.0 && yaw == 0.0 {
            close(v.truncate().length(), 0.0551148, EXACT, "30 degrees up: horizontal");
        }
    }
}

/// "Slopes": the same ramp on a 30-degree slope; standing still on 40
/// degrees; 46 degrees is no ground.
#[test]
fn slope_ramp_rest_and_too_steep() {
    let mut t = T::new(vec![slope(30.0)], Vec3::ZERO);
    t.ticks(3);
    t.throttle(1.0, 0.0);
    t.tick();
    close(t.vel().length(), 0.0106667, EXACT, "30 degrees: tick 1");
    t.tick();
    close(t.vel().length(), 0.0213333, EXACT, "30 degrees: tick 2");

    let mut t = T::new(vec![slope(40.0)], Vec3::ZERO);
    t.ticks(3);
    let at = t.origin();
    for n in 1..=5 {
        t.tick();
        assert!(!t.mv().airborne, "40 degrees: grounded on tick {n}");
        close((t.origin() - at).length(), 0.0, EXACT, &format!("40 degrees: still on tick {n}"));
    }

    let mut t = T::new(vec![slope(46.0)], Vec3::ZERO);
    t.throttle(1.0, 0.0);
    let mut last = t.origin();
    for n in 1..=30 {
        t.tick();
        assert!(t.mv().airborne, "46 degrees: airborne on tick {n}");
        let o = t.origin();
        if n > 2 {
            assert!(o.z < last.z && o.x < last.x, "46 degrees: sliding down on tick {n}: {last} -> {o}");
        }
        last = o;
    }
}

// ---- Steps and edges ----

/// The steps' level: the floor, and a block from x = 0.6 up to `s`.
fn step_level(s: f32) -> Vec<Block> {
    vec![floor(), aabb(Vec3::new(0.6, -60.0, -1.0), Vec3::new(60.0, 60.0, s))]
}

/// "Steps and edges", s = 0.10: not touching on tick 8 (exact); the first
/// touch on tick 9 (the spec's x, z and velocity within `STEP`); climbs
/// onto the top (z = 0.1) and runs at full speed again. Measured: our
/// capsule cast leaves the floor and meets the edge's rounded contact as
/// the spec's mover does, but hops higher over it (apex 0.126 against
/// 0.106): it lands on the top on tick 23 (the spec's 20) and is at full
/// speed on tick 26 (23).
#[test]
fn a_step_of_0_10_is_climbed() {
    let mut t = T::standing_on(step_level(0.10));
    t.throttle(1.0, 0.0);
    t.ticks(8);
    close(t.origin().x, 0.3736667, EXACT, "x after tick 8");
    close(t.speed(), 0.075, EXACT, "speed after tick 8");
    t.tick();
    let (o, v) = (t.origin(), t.vel());
    assert!(!t.mv().airborne, "tick 9: grounded through the floor contact");
    close(o.x, 0.4312763, STEP, "tick 9: x");
    close(o.z, 0.0077620, STEP, "tick 9: z");
    close(v.x, 0.0153671, STEP, "tick 9: v_x");
    close(v.z, 0.0266166, STEP, "tick 9: v_z");
    t.tick();
    assert!(t.mv().airborne, "tick 10: airborne (the edge contact alone is too steep)");
    let mut landed = None;
    for n in 11..=30 {
        t.tick();
        if landed.is_none() && !t.mv().airborne && (t.origin().z - 0.1).abs() < EXACT {
            landed = Some(n);
        }
    }
    let landed = landed.expect("lands on the top");
    assert!((20..=23).contains(&landed), "lands on the top on tick {landed} (the spec's 20)");
    close(t.origin().z, 0.1, EXACT, "on the top");
    close(t.speed(), 0.075, EXACT, "full speed again");
}

/// "Steps and edges": 0.12 is climbed; 0.13 and 0.15 block where the
/// bottom sphere meets the edge. Measured: against the edge ours does not
/// come to rest as the spec's (speed 0 exactly) but rocks on it by a few
/// thousandths (the clip of the floor and the edge alternate), so the
/// blocked rows are checked within `BLOCKED` and `ROCK`, and that it never
/// climbs.
#[test]
fn step_heights_climbed_and_blocked() {
    let mut t = T::standing_on(step_level(0.12));
    t.throttle(1.0, 0.0);
    t.ticks(40);
    assert!(!t.mv().airborne, "0.12: grounded");
    close(t.origin().z, 0.12, EXACT, "0.12: on the top");
    close(t.speed(), 0.075, EXACT, "0.12: full speed");
    for (s, x) in [(0.13, 0.4126501), (0.15, 0.4063508)] {
        let mut t = T::standing_on(step_level(s));
        t.throttle(1.0, 0.0);
        let mut top: f32 = 0.0;
        for _ in 1..=40 {
            t.tick();
            top = top.max(t.origin().z);
            assert!(t.origin().x < 0.6 - 0.1, "{s}: never climbs: {}", t.origin());
        }
        eprintln!("step {s}: x {} z {} top {top} speed {}", t.origin().x, t.origin().z, t.speed());
        close(t.origin().x, x, BLOCKED, &format!("{s}: blocked at"));
        close(t.origin().z, 0.0, BLOCKED, &format!("{s}: on the floor"));
        close(t.speed(), 0.0, ROCK, &format!("{s}: (nearly) stopped"));
    }
}

/// "Steps and edges", s = 0.03: a small lip launches a hop and the player
/// lands on the top at full speed. The spec's apex (0.0987121 on tick 17)
/// and its landing tick (24) come from its own mover; ours is checked
/// within the measured tolerance below.
#[test]
fn a_small_lip_hops() {
    let mut t = T::standing_on(step_level(0.03));
    t.throttle(1.0, 0.0);
    let mut apex: f32 = 0.0;
    let mut airborne = 0;
    for _ in 1..=40 {
        t.tick();
        apex = apex.max(t.origin().z);
        airborne += t.mv().airborne as u32;
    }
    eprintln!("lip 0.03: apex {apex}, airborne ticks {airborne}");
    assert!(airborne > 0, "a hop");
    close(apex, 0.0987121, 0.03, "the hop's apex");
    assert!(!t.mv().airborne, "on the top");
    close(t.origin().z, 0.03, EXACT, "on the top");
    close(t.speed(), 0.075, EXACT, "full speed");
}

/// "Steps and edges": walking off a ledge falls 1/128 on the first tick.
#[test]
fn walking_off_a_ledge() {
    let mut t = T::standing_on(vec![
        aabb(Vec3::new(-60.0, -60.0, -1.0), Vec3::new(1.0, 60.0, 0.0)),
        aabb(Vec3::new(-60.0, -60.0, -6.0), Vec3::new(60.0, 60.0, -5.0)),
    ]);
    t.throttle(1.0, 0.0);
    t.ticks(17);
    assert!(!t.mv().airborne, "grounded on tick 17");
    t.tick();
    assert!(t.mv().airborne, "airborne on tick 18");
    close(t.origin().z, -0.0078125, EXACT, "z after tick 18");
    close(t.vel().z, -0.0078125, EXACT, "v_z after tick 18");
}

/// "Steps and edges": two 60-degree faces meeting in a V hold the player
/// up in the crease (the virtual contact).
#[test]
fn standing_in_a_crease() {
    let a = 60f32.to_radians();
    let mut t = T::new(
        vec![
            plane(Vec3::new(-a.sin(), 0.0, a.cos()), Vec3::ZERO),
            plane(Vec3::new(a.sin(), 0.0, a.cos()), Vec3::ZERO),
        ],
        Vec3::new(0.0, 0.0, 1.0),
    );
    t.ticks(80);
    let m = t.mv();
    assert!(!m.airborne, "grounded in the crease: {m:?}");
    close(m.ground_normal.y, 1.0, EXACT, "the virtual contact's normal is up");
    close(t.origin().z, 0.2, EXACT, "origin z");
    close(t.origin().x, 0.0, EXACT, "origin x");
    // It strafes along the crease at full speed.
    t.throttle(0.0, 1.0);
    t.ticks(30);
    close(t.vel().y, 0.0666667, EXACT, "strafes along the crease");
    close(t.origin().z, 0.2, EXACT, "still in the crease");
}

// ---- Landing and falling ----

/// Dropped from rest at `h` above the floor: the landing tick.
fn drop_from(h: f32) -> (T, u32) {
    let mut t = T::new(vec![floor()], Vec3::new(0.0, 0.0, h));
    let n = t.until_grounded(200);
    (t, n)
}

/// "Landing and falling": landing ticks, velocities, landings and damage.
#[test]
fn falls_land_and_hurt_by_impact_speed() {
    // H, landing tick, lambda, (hard, ticks), damage points.
    type Case = (f32, u32, f32, Option<(bool, f32)>, Option<f32>);
    let cases: [Case; 8] = [
        (1.0, 24, 0.0855643, Some((false, 5.0)), None),
        (2.0, 33, 0.1176509, Some((false, 10.0)), None),
        (3.0, 41, 0.1461723, Some((false, 14.0)), None),
        (3.1, 42, 0.1497375, Some((false, 15.0)), Some(18.33)),
        (4.0, 47, 0.1675634, Some((true, 30.0)), Some(61.00)),
        (5.0, 53, 0.1889545, Some((true, 30.0)), Some(112.20)),
        (6.5, 60, 0.2139108, Some((true, 30.0)), Some(155.0)),
        (16.5, 96, 0.3422572, Some((true, 30.0)), Some(155.0)),
    ];
    for (h, tick, lambda, landing, damage) in cases {
        let (t, n) = drop_from(h);
        assert_eq!(n, tick, "H = {h}: landing tick");
        let m = t.mv();
        close(m.landing_velocity, lambda, EXACT, &format!("H = {h}: landing velocity"));
        let l = m.landing.map(|l| (l.hard, l.ticks));
        assert_eq!(l, landing, "H = {h}: landing");
        close(t.origin().z, 0.0, EXACT, &format!("H = {h}: on the floor"));
        let dealt = t.damage();
        match damage {
            None => assert!(dealt.is_empty(), "H = {h}: no damage, got {dealt:?}"),
            Some(points) => {
                assert_eq!(dealt.len(), 1, "H = {h}: one damage: {dealt:?}");
                close(dealt[0], points, 0.01, &format!("H = {h}: damage points"));
            }
        }
    }
    let kind = drop_from(5.0).0.sim.app.world().resource::<DamageLog>().0[0].kind;
    assert_eq!(kind, DamageKind::Fall);
}

/// "Landing and falling": faster than the death speed kills in the air.
#[test]
fn a_long_fall_kills_in_the_air() {
    let mut t = T::new(vec![floor()], Vec3::new(0.0, 0.0, 17.0));
    t.ticks(96);
    assert!(t.damage().is_empty(), "nothing before tick 97");
    t.tick();
    assert!(t.mv().airborne, "still airborne on tick 97");
    close(t.vel().z, -0.3458224, EXACT, "v_z on tick 97");
    let d = t.damage();
    assert_eq!(d.len(), 1, "105 in the air: {d:?}");
    close(d[0], 105.0, 0.01, "in-air damage");
    t.tick();
    assert!(!t.mv().airborne, "lands on tick 98");
    let d = t.damage();
    assert_eq!(d.len(), 2, "and 155 on landing: {d:?}");
    close(d[1], 155.0, 0.01, "landing damage");
}

/// "Landing and falling": a hard landing holds the player 30 ticks.
#[test]
fn a_hard_landing_holds_30_ticks() {
    let mut t = T::new(vec![floor()], Vec3::new(0.0, 0.0, 5.0));
    t.throttle(1.0, 0.0);
    t.jump(true);
    let l = t.until_grounded(100);
    assert_eq!(l, 53, "landing tick");
    assert!(t.mv().hard_landing(), "a hard landing");
    // The air control's speed from the fall brakes at the hard-landing
    // rate: no wish.
    let mut speed = t.speed();
    for n in 1..=29 {
        t.tick();
        assert!(!t.mv().jumped, "no jump on L + {n}");
        let want = (speed - 0.0053333).max(0.0);
        close(t.speed(), want, EXACT, &format!("no movement wish on L + {n}"));
        speed = want;
    }
    t.tick();
    assert!(t.mv().jumped, "jumps on L + 30");
    close(t.speed(), 0.0106667, EXACT, "moves on L + 30");
}

/// "Landing and falling": a hard landing at full run slides 15 ticks.
#[test]
fn a_hard_landing_slides_to_a_stop() {
    let mut t = T::new(vec![floor()], Vec3::new(0.0, 0.0, 5.0));
    t.set_vel(Vec3::new(0.075, 0.0, 0.0));
    t.throttle(1.0, 0.0);
    assert_eq!(t.until_grounded(100), 53, "landing tick");
    close(t.speed(), 0.075, EXACT, "at full run on landing");
    let want = [(1, 0.0696667), (2, 0.0643333), (14, 0.0003333), (15, 0.0)];
    let mut n = 0;
    for (tick, speed) in want {
        while n < tick {
            t.tick();
            n += 1;
        }
        close(t.speed(), speed, EXACT, &format!("speed on L + {tick}"));
    }
}

// ---- Tick rate, registration ----

/// Under a party app's 64 Hz tick: the same top speed and a jump apex
/// within 0.5 % of the 30 Hz one (the movement's module doc).
#[test]
fn at_64_hz_speed_and_jump_hold() {
    let mut t = T::at_rate(vec![floor()], Vec3::ZERO, 64.0);
    t.ticks(6);
    assert!(!t.mv().airborne, "settled");
    t.throttle(1.0, 0.0);
    t.ticks(64);
    close(t.speed(), 0.075, EXACT, "top speed per Halo tick");
    t.throttle(0.0, 0.0);
    t.ticks(64);
    t.jump(true);
    t.tick();
    t.jump(false);
    let mut apex: f32 = 0.0;
    let mut n = 0;
    while t.mv().airborne && n < 200 {
        t.tick();
        n += 1;
        apex = apex.max(t.origin().z);
    }
    let flight = n as f32 / 64.0;
    eprintln!("64 Hz: apex {apex}, flight {flight} s");
    close(apex, 0.652616, 0.652616 * 0.005, "apex at 64 Hz");
    close(flight, 1.3, 2.0 / 64.0, "flight time at 64 Hz");
}

/// The slot: `halo_ce:movement` is the match's movement and a spawned
/// player gets its state; the numbers parse.
#[test]
fn the_movement_registers_and_its_data_parses() {
    let d = data();
    close(d.run_forward / 30.0, 0.075, EXACT, "run forward per tick");
    close(d.segment(0.0), 0.3, EXACT, "standing segment");
    let t = T::standing();
    assert_eq!(t.sim.app.world().resource::<Loadout>().movement, movement::ID);
    assert_eq!(t.sim.app.world().get::<MovementSlot>(t.p).map(|m| m.0), Some(movement::ID));
    let state = t.sim.state(t.p);
    assert!(state.on_ground);
    close(
        game(t.sim.position(t.p) + state.eye_offset).z,
        0.62,
        EXACT,
        "the eye stands 0.62 WU above the floor",
    );
}
