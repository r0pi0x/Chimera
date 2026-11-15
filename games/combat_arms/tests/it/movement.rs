//! Combat Arms movement (specs/combat_arms/movement.md): its numbered test
//! cases as scenario tests on `CaSim`, each named `case_<n>_...`, plus the
//! edge cases the spec calls out. A test map of our own (a flat floor with
//! its top at y = 0, and per test boxes, ledges, stairs or slopes), the
//! game's 15 ms tick, a standing hull at rest, facing yaw 0 (engine -Z;
//! the spec's +Z), `P = 0` and no gear unless the case says otherwise.
//!
//! Values are world units (u, u/s); the engine's meters are divided by
//! `meters_per_unit`. Tolerances: `TOL` (0.001) for one tick's values,
//! `TOL_SUM` (0.005) for sums over many ticks (f32 rounding of meter
//! positions); the collision mover keeps a 0.1 u skin from surfaces (`SKIN`).

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind};
use chimera_world::harness::Sim;
use combat_arms::{
    game::{
        TICK_INTERVAL,
        movement::{self, CaCharacter, CaMovement, HeldMovePenalty, StaminaState, TickClock, movement_data},
    },
    harness::CaSim,
};

pub(crate) const TOL: f32 = 1e-3;
const TOL_SUM: f32 = 5e-3;
/// The collision mover's gap from surfaces (data/movement.toml `skin`).
const SKIN: f32 = 0.1 + 1e-3;

pub(crate) fn close(got: f32, want: f32, tol: f32, what: &str) {
    assert!((got - want).abs() <= tol, "{what}: got {got}, want {want} (tolerance {tol})");
}

/// A static box: centre and half-extents in u, optional rotation about X.
#[derive(Clone, Copy)]
pub(crate) struct Block {
    centre: Vec3,
    half: Vec3,
    tilt: f32,
}

pub(crate) fn block(centre: Vec3, half: Vec3) -> Block {
    Block { centre, half, tilt: 0.0 }
}

/// A box rotated by `tilt` (radians) about X.
pub(crate) fn tilted(centre: Vec3, half: Vec3, tilt: f32) -> Block {
    Block { centre, half, tilt }
}

/// The floor: top at y = 0, wide enough for every test.
pub(crate) fn floor() -> Block {
    block(Vec3::new(0.0, -50.0, 0.0), Vec3::new(20000.0, 50.0, 20000.0))
}

#[derive(Resource, Clone)]
struct TestMap(Vec<Block>);

#[derive(Resource, Default)]
struct DamageLog(Vec<Damage>);

fn spawn_map(mut commands: Commands, map: Res<TestMap>) {
    let s = movement_data().meters_per_unit;
    for b in &map.0 {
        commands.spawn((
            RigidBody::Static,
            avian3d::prelude::Collider::cuboid(b.half.x * 2.0 * s, b.half.y * 2.0 * s, b.half.z * 2.0 * s),
            Transform::from_translation(b.centre * s).with_rotation(Quat::from_rotation_x(b.tilt)),
        ));
    }
}

fn log_damage(mut damage: MessageReader<Damage>, mut log: ResMut<DamageLog>) {
    log.0.extend(damage.read().cloned());
}

use avian3d::prelude::RigidBody;

pub(crate) struct T {
    pub(crate) sim: Sim,
    pub(crate) p: Entity,
}

const STAND_H: f32 = 50.0;

/// A player's hull centre resting with its feet on a surface at `y`.
pub(crate) fn on(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y + STAND_H + movement_data().skin, z)
}

impl T {
    /// A player with its hull centre at `at` (u) on a map of `blocks`, no
    /// gear, after one tick at rest (it finds what it stands on).
    pub(crate) fn new(blocks: Vec<Block>, at: Vec3) -> Self {
        let map = TestMap(blocks);
        let mut sim = Sim::with(CaSim, move |app| {
            app.insert_resource(map)
                .init_resource::<DamageLog>()
                .add_systems(Startup, spawn_map)
                .add_systems(Last, log_damage);
        });
        sim.set_tick_interval(TICK_INTERVAL);
        let s = movement_data().meters_per_unit;
        let p = sim.spawn_character(at * s, movement::ID);
        let bare = movement_data().character.without_gear();
        sim.app.world_mut().entity_mut(p).insert(bare);
        let mut t = Self { sim, p };
        t.tick();
        // No held weapon's penalty (the spec's cases): the rifle drawn at
        // spawn set its own (`weapons::CaHandling`).
        t.sim.app.world_mut().entity_mut(p).insert(movement::HeldMovePenalty::default());
        t
    }

    /// Standing at rest on the floor (two ticks: the first finds the floor,
    /// the second counts it as ground).
    pub(crate) fn standing() -> Self {
        let mut t = Self::new(vec![floor()], on(0.0, 0.0, 0.0));
        t.tick();
        let m = t.mv();
        assert!(m.on_ground, "standing on the floor: {:?} at {}", m.standing, t.pos());
        t
    }

    pub(crate) fn tick(&mut self) {
        self.sim.ticks(1);
    }

    pub(crate) fn ticks(&mut self, n: u64) {
        self.sim.ticks(n);
    }

    pub(crate) fn pos(&self) -> Vec3 {
        self.sim.position(self.p) / movement_data().meters_per_unit
    }

    pub(crate) fn vel(&self) -> Vec3 {
        self.sim.velocity(self.p) / movement_data().meters_per_unit
    }

    pub(crate) fn hspeed(&self) -> f32 {
        self.vel().xz().length()
    }

    pub(crate) fn mv(&self) -> CaMovement {
        self.sim.app.world().get::<CaMovement>(self.p).unwrap().clone()
    }

    pub(crate) fn mv_mut(&mut self) -> Mut<'_, CaMovement> {
        self.sim.app.world_mut().get_mut::<CaMovement>(self.p).unwrap()
    }

    pub(crate) fn keys(&mut self) -> Mut<'_, chimera_core::core::Intent> {
        self.sim.intent(self.p)
    }

    pub(crate) fn forward(&mut self, on: bool) {
        self.keys().move_axis.y = if on { 1.0 } else { 0.0 };
    }

    pub(crate) fn release_all(&mut self) {
        let mut i = self.keys();
        i.move_axis = Vec2::ZERO;
        i.jump = false;
        i.crouch = false;
        i.sprint = false;
        i.walk = false;
    }

    pub(crate) fn insert(&mut self, c: impl Bundle) {
        self.sim.app.world_mut().entity_mut(self.p).insert(c);
    }

    pub(crate) fn damage(&self) -> Vec<Damage> {
        self.sim.app.world().resource::<DamageLog>().0.clone()
    }

    /// Hold forward until the run is steady (speed limit reached).
    pub(crate) fn steady_forward(&mut self) {
        self.forward(true);
        self.ticks(80);
    }

    /// Teleport the hull's centre to `at` (u), keeping velocity and what
    /// it stood on (a teleport resets nothing: spec "Falling and fall
    /// damage").
    pub(crate) fn teleport(&mut self, at: Vec3) {
        let s = movement_data().meters_per_unit;
        self.sim.app.world_mut().get_mut::<Transform>(self.p).unwrap().translation = at * s;
    }

    /// Put the hull's centre at `at` (u) at rest in the air: no velocity,
    /// standing on nothing.
    pub(crate) fn place_in_air(&mut self, at: Vec3) {
        self.teleport(at);
        self.sim.app.world_mut().get_mut::<chimera_core::core::Velocity>(self.p).unwrap().0 = Vec3::ZERO;
        self.mv_mut().standing = None;
    }

    /// The crouch key released clears the crouch flag at once (no 100 ms
    /// release delay), for cases that set the flag tick by tick.
    fn no_release_delay(&mut self) {
        self.sim.app.world_mut().resource_mut::<movement::MovementData>().crouch_release_delay = 0.0;
    }

    pub(crate) fn feet(&self) -> f32 {
        self.pos().y - self.mv().half_height
    }

    /// The eye's height above the hull's bottom (u).
    fn eye_above_feet(&self) -> f32 {
        let s = movement_data().meters_per_unit;
        self.sim.state(self.p).eye_offset.y / s + self.mv().half_height
    }
}

// --- Cases 1-4: running ---

#[test]
fn case_1_run_start_tick_1() {
    let mut t = T::standing();
    let start = t.pos();
    t.forward(true);
    t.tick();
    close(t.mv().accel.length(), 500.0, TOL, "accel");
    close(t.hspeed(), 7.2257, TOL, "speed");
    close(start.z - t.pos().z, 0.0549, TOL, "distance");
}

#[test]
fn case_2_run_ramp() {
    let mut t = T::standing();
    let start = t.pos();
    t.forward(true);
    let want = [
        (1, 500.0, 7.2257, 0.0549),
        (2, 590.0, 15.2298, 0.2240),
        (3, 680.0, 23.9563, 0.5187),
        (10, 1310.0, 101.3233, 6.8853),
    ];
    let mut n = 0;
    for (tick, accel, speed, dist) in want {
        t.ticks(tick - n);
        n = tick;
        close(t.mv().accel.length(), accel, TOL, &format!("accel after tick {tick}"));
        close(t.hspeed(), speed, TOL, &format!("speed after tick {tick}"));
        close(start.z - t.pos().z, dist, TOL, &format!("distance after tick {tick}"));
    }
}

#[test]
fn case_3_ramp_reaches_max() {
    let mut t = T::standing();
    t.forward(true);
    t.ticks(28);
    close(t.mv().accel.length(), 2930.0, TOL, "accel on tick 28");
    for tick in 29..40 {
        t.tick();
        close(t.mv().accel.length(), 3000.0, TOL, &format!("accel on tick {tick}"));
    }
}

#[test]
fn case_4_steady_run() {
    let mut t = T::standing();
    let start = t.pos();
    t.forward(true);
    t.ticks(60);
    close(start.z - t.pos().z, 208.9518, TOL_SUM, "distance after 60 ticks");
    t.ticks(20);
    let before = t.pos();
    t.tick();
    close(t.mv().speed_limit, 285.0, TOL, "limit");
    close(t.hspeed(), 307.7608, TOL, "end speed");
    close(before.z - t.pos().z, 4.44784, TOL, "distance per tick");
}

// --- Cases 5-8: stances and caps ---

#[test]
fn case_5_steady_walk() {
    let mut t = T::standing();
    t.keys().walk = true;
    t.steady_forward();
    let before = t.pos();
    t.tick();
    close(t.mv().speed_limit, 70.0, TOL, "limit");
    close(t.hspeed(), 108.296, TOL, "end speed");
    close(before.z - t.pos().z, 1.34081, TOL, "distance per tick");
}

#[test]
fn case_6_steady_crouch_walk() {
    let mut t = T::standing();
    t.keys().crouch = true;
    t.steady_forward();
    assert_eq!(t.mv().crouch_rate, 1.0);
    let before = t.pos();
    t.tick();
    close(t.mv().accel.length(), 1500.0, TOL, "accel");
    close(t.hspeed(), 68.0641, TOL, "end speed");
    close(before.z - t.pos().z, 0.88717, TOL, "distance per tick");
}

/// Steady per-tick distance and limit with the keys `setup` holds.
fn steady(t: &mut T, setup: impl Fn(&mut T)) -> (f32, f32) {
    t.release_all();
    t.ticks(30);
    setup(t);
    t.steady_forward();
    let before = t.pos();
    t.tick();
    (t.mv().speed_limit, before.z - t.pos().z)
}

#[test]
fn case_7_reference_loadout_caps() {
    let mut t = T::standing();
    // The reference character (the data's: default vest) with a rifle.
    t.insert((movement_data().character.clone(), HeldMovePenalty { penalty: 0.1, mod_factor: 0.0 }));
    type Case = (&'static str, fn(&mut T), f32, f32);
    let cases: [Case; 4] = [
        ("run", |_| {}, 228.0, 3.62412),
        ("sprint", |t| t.keys().sprint = true, 319.2, 4.94207),
        ("walk", |t| t.keys().walk = true, 56.0, 1.13849),
        ("crouch", |t| t.keys().crouch = true, 40.0, 0.74266),
    ];
    for (what, setup, limit, dist) in cases {
        // A full gauge for the sprint.
        t.mv_mut().stamina = 100.0;
        let (got_limit, got_dist) = steady(&mut t, setup);
        close(got_limit, limit, TOL, &format!("{what} limit"));
        close(got_dist, dist, TOL, &format!("{what} distance per tick"));
    }
}

#[test]
fn case_8_weapon_caps() {
    for (penalty, limit) in [(0.0, 285.0), (0.03, 276.45), (0.10, 256.5), (0.13, 247.95), (0.30, 199.5), (1.2, 0.0)] {
        let mut t = T::standing();
        t.insert(HeldMovePenalty { penalty, mod_factor: 0.0 });
        t.forward(true);
        t.tick();
        close(t.mv().speed_limit, limit, 0.01, &format!("run limit with penalty {penalty}"));
    }
}

// --- Cases 9-12: stopping and keys ---

#[test]
fn case_9_stop() {
    let mut t = T::standing();
    t.steady_forward();
    close(t.hspeed(), 307.7608, TOL, "speed at the end of the last held tick");
    t.forward(false);
    let start = t.pos();
    for (tick, want) in [224.7459, 168.8455, 116.9843, 68.8703, 24.2330].into_iter().enumerate() {
        t.tick();
        close(t.hspeed(), want, TOL, &format!("speed after tick {}", tick + 1));
    }
    close(start.z - t.pos().z, 9.4034, TOL, "distance over ticks 1-5");
    let at5 = t.pos();
    t.tick();
    assert_eq!(t.hspeed(), 0.0, "stopped on tick 6");
    assert_eq!(t.pos(), at5, "no move on tick 6");
}

/// Forward and back held together: an axis of 0 with `Intent::opposed`.
#[test]
fn case_10_opposing_keys() {
    let mut t = T::standing();
    let start = t.pos();
    t.keys().opposed.y = true;
    t.ticks(10);
    let m = t.mv();
    assert!(m.moving && m.forward && m.back, "both flags, moving");
    assert_eq!(m.accel, Vec3::ZERO);
    assert_eq!(t.hspeed(), 0.0);
    assert_eq!(t.pos(), start);
}

#[test]
fn case_11_diagonal() {
    let mut t = T::standing();
    t.keys().move_axis = Vec2::new(1.0, 1.0);
    t.ticks(80);
    let before = t.pos();
    t.tick();
    close(t.mv().accel.length(), 3000.0, TOL, "total accel");
    close(t.mv().speed_limit, 285.0, TOL, "limit");
    close((t.pos() - before).xz().length(), 4.44784, TOL, "distance per tick");
}

#[test]
fn case_12_reversal() {
    let mut t = T::standing();
    t.steady_forward();
    t.keys().move_axis.y = -1.0;
    t.tick();
    close(t.mv().accel.length(), 500.0, TOL, "back accel on tick 1");
    t.tick();
    close(t.mv().accel.length(), 590.0, TOL, "on tick 2");
}

// --- Cases 13-20: jumping and the air ---

#[test]
fn case_13_jump_arc() {
    let mut t = T::standing();
    let y0 = t.pos().y;
    t.keys().jump = true;
    let h = |t: &T| t.pos().y - y0;
    t.tick();
    close(h(&t), 4.888125, TOL, "height after tick 1");
    close(t.vel().y, 313.5, TOL, "vertical speed after tick 1");
    t.tick();
    close(h(&t), 9.52875, TOL, "height after tick 2");
    close(t.vel().y, 297.0, TOL, "after tick 2");
    t.ticks(18);
    close(h(&t), 50.7375, TOL_SUM, "apex after tick 20");
    close(t.vel().y, 0.0, TOL, "speed at the apex");
    t.tick();
    close(h(&t), 50.675625, TOL_SUM, "after tick 21");
    close(t.vel().y, -16.5, TOL, "after tick 21");
    t.ticks(19);
    close(h(&t), 2.475, TOL_SUM, "after tick 40");
    assert!(t.mv().standing.is_none());
    t.tick();
    close(h(&t), 0.0, TOL_SUM, "tick 41 reaches the floor");
    assert!(t.mv().in_jump, "the jump hasn't ended on the contact tick");
    t.tick();
    assert!(!t.mv().in_jump, "landing detected on tick 42");
    assert_eq!(t.vel().y, 0.0, "vertical speed zeroed when the move down is blocked");
}

#[test]
fn case_14_jump_factor() {
    let mut t = T::standing();
    t.keys().jump = true;
    t.tick(); // the jump (tick 0)
    // The value after tick n - 1 is the value at the start of tick n.
    close(t.mv().jump_factor, 0.3, 1e-5, "start of tick 1");
    t.tick();
    close(t.mv().jump_factor, 0.3 + 0.018, 1e-5, "start of tick 2");
    t.ticks(37);
    close(t.mv().jump_factor, 0.984, 1e-4, "start of tick 39");
    t.tick();
    close(t.mv().jump_factor, 1.002, 1e-4, "start of tick 40");
    t.tick();
    assert_eq!(t.mv().jump_factor, 1.0, "start of tick 41");
}

#[test]
fn case_15_air_accel_after_a_jump() {
    let mut t = T::standing();
    t.steady_forward();
    let before = t.pos();
    t.keys().jump = true;
    t.tick();
    close(t.hspeed(), 330.0, TOL, "jump tick's horizontal end speed");
    close(before.z - t.pos().z, 4.44375, TOL, "jump tick's distance");
    let before = t.pos();
    t.tick();
    close(t.mv().accel.length(), 900.0, TOL, "tick 1 accel");
    close(t.hspeed(), 298.5, TOL, "tick 1 end speed");
    close(before.z - t.pos().z, 4.325625, TOL, "tick 1 horizontal distance");
}

#[test]
fn case_16_hold_jump() {
    let mut t = T::standing();
    t.keys().jump = true;
    let mut jumps = 0;
    for _ in 0..120 {
        let was = t.mv().in_jump;
        t.tick();
        jumps += (!was && t.mv().in_jump) as u32;
    }
    assert_eq!(jumps, 1, "holding jump jumps once");

    // Case 13's timing: contact on tick 41, landing on 42; up on 43, the
    // press on 44 jumps.
    let mut t = T::standing();
    t.keys().jump = true;
    t.tick();
    t.keys().jump = false;
    t.ticks(41); // through tick 42
    assert!(!t.mv().in_jump);
    t.keys().jump = false;
    t.tick(); // 43: up on the ground
    t.keys().jump = true;
    t.tick(); // 44
    assert!(t.mv().in_jump, "the second jump on tick 44");

    // Released only during the jump, pressed again on 43: too early.
    let mut t = T::standing();
    t.keys().jump = true;
    t.tick();
    t.keys().jump = false;
    t.ticks(41);
    t.keys().jump = true;
    t.ticks(3);
    assert!(!t.mv().in_jump, "the key must be up for a grounded tick first");
}

#[test]
fn case_17_jump_while_standing_up() {
    let mut t = T::standing();
    t.keys().crouch = true;
    t.ticks(20);
    assert_eq!(t.mv().crouch_rate, 1.0);
    t.keys().crouch = false;
    t.keys().jump = true;
    for _ in 0..30 {
        t.tick();
        assert!(!t.mv().in_jump, "no jump while standing up nor after");
    }
    assert_eq!(t.mv().crouch_rate, 0.0);
    t.keys().jump = false;
    t.tick();
    t.keys().jump = true;
    t.tick();
    assert!(t.mv().in_jump, "a new press after standing jumps");
}

/// At rest high in the air after one tick there (a spawn would put the
/// player on the floor: case 47; this is a teleport), not in a jump.
fn in_the_air() -> T {
    let mut t = T::standing();
    t.place_in_air(Vec3::new(0.0, 5000.0, 0.0));
    t.tick();
    t
}

#[test]
fn case_18_air_fall() {
    // `in_the_air` ran one tick already: compare from a fresh start.
    let mut t = in_the_air();
    let y0 = t.pos().y + 0.061875; // undo the first tick (from rest: -0.061875)
    close(t.vel().y, -16.5, TOL, "after tick 1");
    t.ticks(19);
    close(y0 - t.pos().y, 48.2625, TOL_SUM, "fallen after 20 ticks");
    close(t.vel().y, -330.0, TOL, "after 20 ticks");
    t.ticks(29);
    close(y0 - t.pos().y, 294.0919, TOL_SUM, "after 49 ticks");
    t.tick();
    close(y0 - t.pos().y, 306.2813, TOL_SUM, "after 50 ticks");
    close(t.vel().y, -825.0, TOL, "after 50 ticks");
}

#[test]
fn case_19_air_control_from_rest() {
    let mut t = in_the_air();
    // A fresh start in the air: zero what the first tick did.
    t.sim.app.world_mut().get_mut::<chimera_core::core::Velocity>(t.p).unwrap().0 = Vec3::ZERO;
    let start = t.pos();
    t.forward(true);
    t.tick();
    assert!(!t.mv().in_jump);
    close(t.mv().accel.xz().length(), 500.0, TOL, "tick 1 accel");
    close(t.hspeed(), 7.5, TOL, "tick 1 speed");
    close(start.z - t.pos().z, 0.028125, TOL, "tick 1 distance");
    t.tick();
    close(t.mv().accel.xz().length(), 590.0, TOL, "tick 2 accel");
    close(t.hspeed(), 16.35, TOL, "tick 2 speed");
    close(start.z - t.pos().z, 0.173813, TOL, "distance after tick 2");
    t.tick();
    close(t.mv().accel.xz().length(), 680.0, TOL, "tick 3 accel");
    close(t.hspeed(), 26.55, TOL, "tick 3 speed");
}

/// Case 20. In a jump the crouched air accel is 1500 x 0.1 = 150, and like
/// every acceleration it is scaled by the jump factor while that is below
/// 1 (the case's "+2.25 u/s per tick" is for j = 1: its correction, case
/// 37).
#[test]
fn case_20_crouched_air_control() {
    let mut t = T::standing();
    t.keys().jump = true;
    t.tick();
    t.keys().jump = false;
    t.keys().crouch = true;
    t.tick();
    t.forward(true);
    t.tick(); // not the moving start: the jump flag started it
    for _ in 0..10 {
        let j = t.mv().jump_factor;
        let before = t.hspeed();
        t.tick();
        assert!(t.mv().in_jump);
        close(t.hspeed() - before, 150.0 * j * 0.015, TOL, "in a jump: +150 j dt per tick");
    }

    let mut t = in_the_air();
    t.keys().crouch = true;
    t.forward(true);
    // Crouched: no 500 start (case 38); +22.5 a tick while under the
    // crouch-blended limit (ticks 1-3).
    for _ in 0..3 {
        let before = t.hspeed();
        t.tick();
        assert!(!t.mv().in_jump);
        close(t.mv().accel.xz().length(), 1500.0, TOL, "walked off: 1500");
        close(t.hspeed() - before, 22.5, TOL, "walked off: +22.5 per tick");
    }
}

// --- Cases 21-26: crouching ---

#[test]
fn case_21_crouch_rate() {
    let mut t = T::standing();
    t.keys().crouch = true;
    for (n, want) in [0.259615, 0.463208, 0.618555, 0.733433, 0.815619, 0.872890, 0.913022, 0.943794, 0.972982, 1.0]
        .into_iter()
        .enumerate()
    {
        t.tick();
        close(t.mv().crouch_rate, want, 2e-6, &format!("rate after tick {} down", n + 1));
    }
    t.ticks(30);
    t.keys().crouch = false;
    for (n, want) in [0.740385, 0.536792, 0.381445, 0.266567, 0.184381, 0.127110, 0.086978, 0.056206, 0.027018, 0.0]
        .into_iter()
        .enumerate()
    {
        t.tick();
        close(t.mv().crouch_rate, want, 2e-6, &format!("rate after tick {} up", n + 1));
    }
}

#[test]
fn case_22_crouch_hull_and_speed() {
    let mut t = T::standing();
    let y0 = t.pos().y;
    t.keys().crouch = true;
    t.tick();
    close(t.mv().half_height, 47.40385, TOL, "half-height on tick 1");
    close(y0 - t.pos().y, 2.59615, TOL, "the centre moves down, feet stay");
    close(t.mv().speed_limit, 64.8077, TOL, "limit on tick 1");
    t.ticks(15);
    close(t.mv().half_height, 40.0, TOL, "fully crouched");
    close(y0 - t.pos().y, 10.0, TOL, "fully crouched centre");
    close(t.mv().speed_limit, 50.0, TOL, "limit when full");
}

#[test]
fn case_23_crouch_jump() {
    let jump = |crouch: bool| {
        let mut t = T::standing();
        t.keys().jump = true;
        t.ticks(19);
        t.keys().crouch = crouch;
        t.tick(); // tick 20, the apex
        t.ticks(9);
        (t.pos().y, t.mv().half_height)
    };
    let (plain, plain_h) = jump(false);
    let (crouched, crouched_h) = jump(true);
    close(crouched_h, 40.0, TOL, "half-height after 10 crouch ticks");
    close(crouched, plain, TOL_SUM, "the centre's path doesn't change");
    close((crouched - crouched_h) - (plain - plain_h), 10.0, TOL_SUM, "feet 10 u higher");
}

#[test]
fn case_24_crouch_tap_delay() {
    let mut t = T::standing();
    t.keys().crouch = true;
    t.tick(); // pressed at this tick's time
    t.keys().crouch = false;
    // Ticks 15, 30, ..., 90 ms after the press keep it; 105 ms clears it.
    for n in 1..=6 {
        t.tick();
        assert!(t.mv().crouch, "{} ms after the press: still set", n * 15);
    }
    t.tick();
    assert!(!t.mv().crouch, "105 ms after the press: cleared");
}

/// Keys of case 25: held 1-7, up 8-14, held 15-21, up 22-28, held 29-35,
/// up 36-42, held 43 on.
fn spam_key(tick: u32) -> bool {
    matches!(tick, 1..=7 | 15..=21 | 29..=35) || tick >= 43
}

#[test]
fn case_25_crouch_spam() {
    let mut t = T::standing();
    // Running (not still) before and during.
    t.steady_forward();
    let mut rate = Vec::new();
    let mut lockout = Vec::new();
    let mut slowdown = Vec::new();
    let mut locked = Vec::new();
    let mut flag = Vec::new();
    for tick in 1..=200 {
        t.keys().crouch = spam_key(tick);
        t.tick();
        let m = t.mv();
        rate.push(m.crouch_rate);
        lockout.push(m.lockout);
        slowdown.push(m.slowdown);
        locked.push(m.crouch_locked);
        flag.push(m.crouch);
    }
    let after = |v: &[f32], tick: usize| v[tick - 1];
    for (tick, want) in [(1, 0.6), (14, 0.73), (15, 0.33), (28, 0.46), (29, 0.06), (42, 0.19), (43, 0.0)] {
        close(after(&lockout, tick), want, 1e-4, &format!("lock-out after tick {tick}"));
    }
    assert!(!locked[41] && locked[42], "lock-out begins on tick 43");
    close(after(&slowdown, 1), 0.5, 1e-5, "slowdown after tick 1");
    close(after(&slowdown, 15), 0.0, 1e-5, "slowdown after tick 15");
    for (tick, want) in [(1, 0.259615), (7, 0.680425), (14, 0.163968), (15, 0.259615), (28, 0.207692), (43, 0.259615), (44, 0.0)] {
        close(after(&rate, tick), want, 1e-5, &format!("crouch rate after tick {tick}"));
    }
    assert!(flag[43], "tick 44: still set (rate not 0 at its start)");
    assert!(flag[44..].iter().take(50).all(|f| !f), "crouch flag cleared from tick 45 while held");
    // Lock-out ends after about 1.5 s on the ground (100 ticks of +0.01).
    let ends = locked.iter().enumerate().skip(43).find(|(_, l)| !**l).map(|(i, _)| i + 1);
    let ends = ends.expect("lock-out ended");
    assert!((142..=144).contains(&ends), "lock-out ends on tick {ends} (43 + 100, +-1)");
}

#[test]
fn case_26_crouch_spam_while_still() {
    let mut t = T::standing();
    t.ticks(20);
    for tick in 1..=60 {
        t.keys().crouch = spam_key(tick);
        t.tick();
        let m = t.mv();
        assert_eq!((m.slowdown, m.lockout, m.crouch_locked), (1.0, 1.0, false), "tick {tick}");
    }
}

// --- Cases 27-30: sprint ---

/// Holding sprint and forward drains 30 a second (0.45 a tick).
#[test]
fn case_27_sprint_drain() {
    let mut t = T::standing();
    t.insert(movement_data().character.clone());
    t.forward(true);
    t.keys().sprint = true;
    // 1.000 s is 66.7 ticks: 67 ticks drain 30.15 (tolerance: one tick).
    t.ticks(67);
    close(t.mv().stamina, 70.0, 0.45, "gauge after 1 s");
    assert!(t.mv().sprint);
    // The flag drops when the gauge is below 1: tick 221 (3.315 s).
    t.ticks(220 - 67);
    assert!(t.mv().sprint, "sprinting on tick 220 (gauge {})", t.mv().stamina);
    t.tick();
    t.tick();
    assert!(!t.mv().sprint, "sprint over after 3.3 s");
    assert_eq!(t.mv().stamina_state, StaminaState::Stopped);
    t.ticks(30);
    assert_eq!(t.mv().stamina, 0.0, "pinned at 0 while the key stays held");
}

#[test]
fn case_28_sprint_recover() {
    let mut t = T::standing();
    t.insert(movement_data().character.clone());
    t.mv_mut().stamina = 0.0;
    t.ticks(133); // 1.995 s
    close(t.mv().stamina, 50.0, 0.375, "gauge after 2 s");
    t.ticks(134); // 4.005 s
    assert_eq!(t.mv().stamina, 100.0, "full after 4 s");
    t.ticks(20);
    assert_eq!(t.mv().stamina, 100.0, "stays full");
}

#[test]
fn case_29_sprint_blocked() {
    type Blocked = (&'static str, fn(&mut T));
    let blocked: [Blocked; 4] = [
        ("walk key held", |t| t.keys().walk = true),
        ("crouch held", |t| t.keys().crouch = true),
        ("back held", |t| t.keys().move_axis = Vec2::new(0.0, -1.0)),
        ("strafe only", |t| t.keys().move_axis = Vec2::new(1.0, 0.0)),
    ];
    for (what, setup) in blocked {
        let mut t = T::standing();
        t.insert(movement_data().character.clone());
        t.forward(true);
        t.keys().sprint = true;
        setup(&mut t);
        t.ticks(5);
        assert!(!t.mv().sprint, "{what}: no sprint");
    }
    // Crouch rate above 0 (standing up): no sprint until it is 0.
    let mut t = T::standing();
    t.keys().crouch = true;
    t.ticks(15);
    t.keys().crouch = false;
    t.forward(true);
    t.keys().sprint = true;
    t.tick();
    assert!(t.mv().crouch_rate > 0.0 && !t.mv().sprint, "standing up: no sprint");
    t.ticks(15);
    assert!(t.mv().sprint, "sprint once fully up");
    // Zoomed: the flag may be on but the limit is the walk speed.
    // (The rifle with a 40 degree scope, zoomed in: weapon_classes.md 7.9.)
    let w = t.sim.app.world().get::<chimera_sim::weapon::Inventory>(t.p).unwrap().active.unwrap();
    t.sim.app.world_mut().get_mut::<combat_arms::game::weapons::CaZoom>(w).unwrap().scope =
        Some(combat_arms::game::weapons::Scope {
            name: "test".into(),
            fov: vec![40.0],
            sprite: String::new(),
            sprite_scale: 0.0,
            ..Default::default()
        });
    t.keys().secondary = true;
    t.tick();
    assert!(t.sim.app.world().get::<chimera_sim::weapon::Zoomed>(t.p).is_some(), "zoomed");
    assert!(t.mv().sprint);
    close(t.mv().speed_limit, 70.0, TOL, "zoomed limit");
}

#[test]
fn case_30_sprint_speed() {
    let mut t = T::standing();
    let mut c = movement_data().character.clone();
    for g in &mut c.gear {
        g.move_penalty = 0.0;
    }
    t.insert(c);
    t.keys().sprint = true;
    t.forward(true);
    t.ticks(60);
    let before = t.pos();
    t.tick();
    close(t.mv().speed_limit, 399.0, TOL, "sprint limit");
    close(t.hspeed(), 413.5236, TOL, "steady end speed");
    close(before.z - t.pos().z, 6.09529, TOL, "distance per tick");
}

// --- Cases 31-35: falls, steps, slopes ---

#[test]
fn case_31_fall_damage() {
    let data = movement_data();
    for (drop, want) in [(299.0, None), (300.0, Some(0.0)), (400.0, Some(50.0)), (500.0, Some(100.0)), (700.0, Some(200.0)), (1000.0, Some(350.0))] {
        assert_eq!(movement::fall_damage(drop, data), want, "drop {drop}");
    }
    // Landing from 400 u (placed there: a spawn would start on the floor):
    // crush damage, upward, from the player itself.
    let mut t = T::standing();
    t.place_in_air(on(0.0, 400.0, 0.0));
    t.ticks(100);
    let d = t.damage();
    assert_eq!(d.len(), 1, "one landing: {d:?}");
    close(d[0].amount * data.health_points, 50.0, 0.01, "damage from 400 u");
    assert_eq!((d[0].kind, d[0].dir, d[0].attacker), (DamageKind::Crush, Vec3::Y, Some(t.p)));
    // 299 u: none.
    let mut t = T::standing();
    t.place_in_air(on(0.0, 299.0, 0.0));
    t.ticks(100);
    assert!(t.damage().is_empty());
}

#[test]
fn case_32_jump_off_fall() {
    // A 300 u pillar, 40 u wide: jump and run off it.
    // The hull (radius 15) rests 1 u over the pillar's -Z edge (z = -20).
    let pillar = block(Vec3::new(0.0, 150.0, 0.0), Vec3::new(20.0, 150.0, 20.0));
    let mut t = T::new(vec![floor(), pillar], on(0.0, 300.0, -34.0));
    t.tick();
    assert!(t.mv().on_ground);
    t.keys().jump = true;
    t.forward(true);
    t.ticks(150);
    let d = t.damage();
    assert_eq!(d.len(), 1, "{d:?}");
    close(d[0].amount * movement_data().health_points, 25.36875, 0.01, "damage from the apex");
}

#[test]
fn case_33_stairs_down() {
    // 20 steps of 16 u going down toward -Z, 32 u deep each.
    let mut map = vec![floor()];
    for i in 0..20 {
        let top = (20 - i) as f32 * 16.0;
        map.push(block(Vec3::new(0.0, top / 2.0, -(i as f32) * 32.0), Vec3::new(100.0, top / 2.0, 16.0)));
    }
    let mut t = T::new(map, on(0.0, 320.0, 0.0));
    t.forward(true);
    t.ticks(400);
    assert!(t.pos().y < 100.0, "walked down: {}", t.pos());
    assert!(t.damage().is_empty(), "{:?}", t.damage());
}

#[test]
fn case_34_step_up() {
    for (height, climbs) in [(30.0, true), (31.0, false)] {
        let wall = block(Vec3::new(0.0, height / 2.0, -200.0), Vec3::new(200.0, height / 2.0, 100.0));
        let mut t = T::new(vec![floor(), wall], on(0.0, 0.0, 0.0));
        t.forward(true);
        // 0.6 s: up against the box, or on it (not yet off its far side).
        t.ticks(40);
        let feet = t.pos().y - t.mv().half_height;
        assert!(!t.mv().in_jump);
        if climbs {
            close(feet, height, 2.0 * SKIN, "on top of the 30 u box");
        } else {
            close(feet, 0.0, 2.0 * SKIN, "blocked by the 31 u box");
            assert!(t.pos().z > -86.0, "stopped at the box: {}", t.pos());
        }
    }
}

/// A slope rising toward -Z whose normal has `ny` as its Y.
fn slope(ny: f32) -> Block {
    let tilt = ny.acos();
    Block {
        centre: Vec3::new(0.0, 0.0, 0.0),
        half: Vec3::new(300.0, 10.0, 1000.0),
        tilt,
    }
}

#[test]
fn case_35_slope() {
    for (ny, walkable) in [(0.70f32, false), (0.71, true)] {
        // Above the slope's top face (n.y y + n.z z = 10) at z = -100.
        let surface = (10.0 + (1.0 - ny * ny).sqrt() * 100.0) / ny;
        let mut t = T::new(vec![floor(), slope(ny)], Vec3::new(0.0, surface + 100.0, -100.0));
        // Fall onto it, then a tick to count it.
        for _ in 0..60 {
            if t.mv().standing.is_some() {
                break;
            }
            t.tick();
        }
        if walkable {
            t.ticks(40);
        } else {
            t.tick();
        }
        let m = t.mv();
        let n = m.standing.map(|s| s.normal.y);
        assert!(n.is_some_and(|n| (n - ny).abs() < 1e-3), "on the slope ({ny}): {n:?}");
        if walkable {
            assert!(m.on_ground, "{ny}: ground");
            let at = t.pos();
            t.ticks(20);
            assert!((t.pos() - at).length() < 0.01, "{ny}: stays put");
            t.keys().jump = true;
            t.tick();
            assert!(t.mv().in_jump, "{ny}: jumps");
        } else {
            assert!(!m.on_ground, "{ny}: not ground");
            let at = t.pos();
            t.keys().jump = true;
            t.ticks(5);
            assert!(!t.mv().in_jump, "{ny}: jump ignored");
            assert!(t.pos().y < at.y - 1.0, "{ny}: slides down ({} -> {})", at.y, t.pos().y);
        }
    }
}

// --- Case 36: the tick clock ---

#[test]
fn case_36_tick_clock() {
    let mut c = TickClock::new(15);
    assert_eq!(c.frame(100.0), 6);
    let mut c = TickClock::new(15);
    assert_eq!(c.frame(200.0), 6, "the other 100 ms is dropped");
    let mut c = TickClock::new(15).with_remainder(10.0);
    assert_eq!(c.frame(20.0), 2);
    assert_eq!(c.fraction(), 0.0);
    assert_eq!(TICK_INTERVAL, 0.015, "the game's tick is the movement's");
}

// --- Edge cases the spec calls out ---

#[test]
fn edge_moving_start_in_the_air_uses_the_start_accel() {
    let mut t = T::standing();
    t.keys().jump = true;
    t.tick();
    t.keys().jump = false;
    t.tick();
    t.tick();
    // The moving flag is off now (no keys); forward starts it in the air.
    t.forward(true);
    t.tick();
    close(t.mv().accel.length(), 500.0 * 0.336, 0.01, "500 x the jump factor (0.336)");
}

#[test]
fn edge_crouch_release_without_headroom() {
    // A ceiling 90 u above the floor: crouched fits (80), standing (100) not.
    let ceiling = block(Vec3::new(0.0, 140.0, -400.0), Vec3::new(200.0, 50.0, 300.0));
    let mut t = T::new(vec![floor(), ceiling], on(0.0, 0.0, 0.0));
    t.keys().crouch = true;
    t.forward(true);
    t.ticks(300);
    assert!(t.pos().z < -150.0, "crouch-walked under: {}", t.pos());
    t.forward(false);
    t.keys().crouch = false;
    t.ticks(20);
    assert!(t.mv().crouch && t.mv().crouch_rate == 1.0, "stays crouched under the ceiling");
}

#[test]
fn edge_speed_limit_change_mid_air() {
    let mut t = T::standing();
    t.insert(movement_data().character.without_gear());
    t.keys().sprint = true;
    t.steady_forward();
    t.keys().jump = true;
    t.tick();
    t.keys().sprint = false;
    t.tick();
    close(t.mv().speed_limit, 285.0, TOL, "the run limit at once");
    assert!(t.hspeed() < 285.0 + 0.015 * 3000.0 * 0.3 + TOL, "clamped to it: {}", t.hspeed());
}

#[test]
fn edge_landing_keeps_horizontal_speed() {
    let mut t = T::standing();
    t.steady_forward();
    t.keys().jump = true;
    t.tick();
    t.keys().jump = false;
    t.ticks(40); // contact on tick 41
    let speed = t.hspeed();
    assert!(speed > 285.0, "air speed {speed}");
    t.tick();
    assert!(t.mv().on_ground);
    assert!(t.hspeed() > 285.0, "no landing speed loss: {}", t.hspeed());
}

#[test]
fn edge_penalty_is_clamped() {
    let mut t = T::standing();
    t.insert(HeldMovePenalty { penalty: -3.0, mod_factor: 0.0 });
    t.forward(true);
    t.tick();
    close(t.mv().speed_limit, 570.0, TOL, "at most double");
}

#[test]
fn the_entitys_collider_follows_the_hull() {
    let mut t = T::standing();
    let s = movement_data().meters_per_unit;
    let half = |t: &T| {
        t.sim
            .app
            .world()
            .get::<avian3d::prelude::Collider>(t.p)
            .unwrap()
            .shape_scaled()
            .as_cylinder()
            .map(|c| c.half_height / s)
    };
    close(half(&t).unwrap(), 50.0, TOL, "standing");
    t.keys().crouch = true;
    t.ticks(15);
    close(half(&t).unwrap(), 40.0, TOL, "crouched");
    let state = t.sim.state(t.p);
    assert!(state.crouching);
    close(state.hull_max.y / s, 40.0, TOL, "hull reported");
}

#[test]
fn the_data_parses() {
    let d = movement_data();
    assert_eq!(d.run_forward, 285.0);
    assert_eq!(d.character.stamina_max(), 100.0);
    let _: &CaCharacter = &d.character;
    // Body size: the 100 u standing hull is Chimera's character.
    let height = 2.0 * d.stand_half_extents[1] * d.meters_per_unit;
    assert!((height - chimera_core::character::CAPSULE_HEIGHT).abs() < 1e-5, "{height}");
    assert_eq!(d.meters_per_unit, combat_arms::game::inventory::UNIT, "inventory's UNIT");
}




// --- Cases 37-55: the follow-up answers ---

#[test]
fn case_37_crouched_air_control_after_a_jump() {
    let mut t = T::standing();
    t.keys().jump = true;
    t.tick(); // tick 0, the jump
    t.keys().jump = false;
    t.keys().crouch = true;
    t.forward(true);
    for n in 1..=40 {
        let before = t.hspeed();
        t.tick();
        let want = match n {
            1 => Some((45.0, 0.675)),
            20 => Some((96.3, 1.4445)),
            39 => Some((147.6, 2.214)),
            40 => Some((150.0, 2.25)),
            _ => None,
        };
        if let Some((accel, gain)) = want {
            assert!(t.mv().in_jump, "tick {n}: in the jump");
            close(t.mv().accel.length(), accel, 0.01, &format!("accel on tick {n}"));
            // The gain until the crouched limit (about 50 u/s) clamps it.
            if n <= 20 {
                close(t.hspeed() - before, gain, TOL, &format!("gain on tick {n}"));
            }
        }
    }
}

#[test]
fn case_38_crouched_start_from_rest() {
    let mut t = T::standing();
    t.keys().crouch = true;
    t.ticks(30);
    assert_eq!(t.mv().crouch_rate, 1.0);
    let start = t.pos();
    t.forward(true);
    for (n, want) in [21.6770, 41.7876, 60.4451].into_iter().enumerate() {
        t.tick();
        close(t.mv().accel.length(), 1500.0, TOL, &format!("accel on tick {}", n + 1));
        close(t.hspeed(), want, TOL, &format!("speed after tick {}", n + 1));
    }
    close(start.z - t.pos().z, 1.41097, TOL, "distance after tick 3");
    t.tick();
    close(t.hspeed(), 68.0641, TOL, "tick 4: clamped to 50, the steady end speed");
}

#[test]
fn case_39_jump_request_tick_bypasses_the_ramp() {
    let mut t = T::standing();
    t.tick(); // tick 0: no keys
    t.forward(true);
    t.keys().jump = true;
    t.tick();
    assert!(t.mv().in_jump);
    close(t.mv().accel.length(), 3000.0, TOL, "jump tick: 3000, no 500 start");
    close(t.hspeed(), 45.0, TOL, "jump tick: no friction");
    let before = t.hspeed();
    t.tick();
    close(t.mv().accel.length(), 177.0, 0.01, "tick 2: 590 x 0.3");
    close(t.hspeed() - before, 2.655, TOL, "tick 2's gain");
}

const RATES_DOWN: [f32; 5] = [0.259615, 0.463208, 0.618555, 0.733433, 0.815619];

#[test]
fn case_40_crouch_reversal_halfway() {
    let mut t = T::standing();
    t.no_release_delay();
    let feet = t.feet();
    let halves = [47.40385, 45.36792, 43.81445, 42.66567, 41.84381, 44.43997, 46.47589, 48.02936, 49.17814, 50.0];
    let rates = [RATES_DOWN.as_slice(), &[0.556003, 0.352411, 0.197064, 0.082186, 0.0]].concat();
    for n in 1..=10 {
        t.keys().crouch = n <= 5;
        t.tick();
        close(t.mv().crouch_rate, rates[n - 1], 2e-6, &format!("rate after tick {n}"));
        close(t.mv().half_height, halves[n - 1], TOL, &format!("half-height after tick {n}"));
        close(t.feet(), feet, TOL, &format!("feet after tick {n}"));
    }
}

#[test]
fn case_41_crouch_pressed_again_while_standing_up() {
    let mut t = T::standing();
    t.no_release_delay();
    let want = [0.556003, 0.352411, 0.612026, 0.815619, 0.970965, 1.0, 1.0, 1.0];
    for n in 1..=13 {
        t.keys().crouch = n <= 5 || n >= 8;
        t.tick();
        if n >= 6 {
            close(t.mv().crouch_rate, want[n - 6], 2e-6, &format!("rate after tick {n}"));
        }
    }
}

#[test]
fn case_42_standing_up_on_the_ground_keeps_the_feet() {
    let mut t = T::standing();
    t.keys().crouch = true;
    t.ticks(30);
    let feet = t.feet();
    close(feet, 0.0, 2.0 * SKIN, "crouched on the floor");
    t.keys().crouch = false;
    while t.mv().crouch_rate == 1.0 {
        t.tick();
    }
    close(t.mv().crouch_rate, 0.740385, 2e-6, "stand tick 1");
    close(t.mv().half_height, 42.59615, TOL, "half-height after stand tick 1");
    close(t.feet(), feet, TOL, "feet after stand tick 1");
    t.ticks(9);
    close(t.mv().half_height, 50.0, TOL, "after tick 10");
    close(t.feet(), feet, TOL, "feet after tick 10");
}

/// Crouched, at rest in the air at `at`.
fn crouched_in_air(at: Vec3) -> T {
    let mut t = T::standing();
    t.no_release_delay();
    t.keys().crouch = true;
    t.ticks(30);
    t.place_in_air(at);
    t
}

#[test]
fn case_43_standing_up_in_open_air() {
    // The same fall crouched and standing up: the centres match (gravity
    // aside, the growth keeps the centre), the feet drop 10.
    let at = Vec3::new(0.0, 2000.0, 0.0);
    let mut held = crouched_in_air(at);
    let mut up = crouched_in_air(at);
    up.keys().crouch = false;
    for n in 1..=10 {
        held.tick();
        up.tick();
        close(up.pos().y, held.pos().y, TOL, &format!("centre after tick {n}"));
    }
    close(up.mv().half_height, 50.0, TOL, "stood up");
    close(held.feet() - up.feet(), 10.0, TOL, "the feet dropped 10");
}

#[test]
fn case_44_standing_up_just_above_the_floor() {
    // Feet 1.0 u above the floor; the first stand tick grows the half-
    // height by 2.59615: the bottom ends on the floor and the centre rises
    // by the growth less the gap, instead of staying.
    let mut t = crouched_in_air(Vec3::new(0.0, 41.0, 0.0));
    close(t.feet(), 1.0, TOL, "1 u above the floor");
    let centre = t.pos().y;
    t.keys().crouch = false;
    t.tick();
    let grown = t.mv().half_height - 40.0;
    close(grown, 2.59615, TOL, "grown");
    close(t.feet(), 0.0, 2.0 * SKIN, "on the floor");
    close(t.pos().y - centre, grown - 1.0, 2.0 * SKIN, "the centre rose");
}

#[test]
fn case_45_not_a_step() {
    use avian3d::prelude::Collider;
    let s = movement_data().meters_per_unit;
    // A 20 u box ahead: a character, a moving object, plain world.
    for kind in ["character", "mover", "world"] {
        let mut t = T::standing();
        let mut e = t.sim.app.world_mut().spawn((
            Collider::cuboid(200.0 * s, 20.0 * s, 200.0 * s),
            Transform::from_translation(Vec3::new(0.0, 10.0, -200.0) * s),
        ));
        match kind {
            "character" => e.insert((RigidBody::Static, chimera_core::core::Intent::default())),
            "mover" => e.insert(RigidBody::Kinematic),
            _ => e.insert(RigidBody::Static),
        };
        t.tick();
        t.forward(true);
        t.ticks(40);
        if kind == "world" {
            close(t.feet(), 20.0, 2.0 * SKIN, "stepped onto plain world");
        } else {
            close(t.feet(), 0.0, 2.0 * SKIN, &format!("blocked by a {kind}"));
            assert!(t.pos().z > -86.0, "{kind}: stopped at the box: {}", t.pos());
        }
    }
}

#[test]
fn case_46_ledge_on_the_flat_bottom() {
    // A ledge 100 u high over z >= 0 (its edge at z = 0); the hull's
    // centre 14 u beyond it overlaps it by 1 u.
    let ledge = block(Vec3::new(0.0, 50.0, 200.0), Vec3::new(200.0, 50.0, 200.0));
    let mut t = T::new(vec![floor(), ledge], on(0.0, 100.0, -14.0));
    t.ticks(5);
    let m = t.mv();
    let n = m.standing.expect("stands on the ledge").normal;
    assert!((n - Vec3::Y).length() < 1e-3, "flat: {n}");
    assert!(m.on_ground);
    close(t.feet(), 100.0, 2.0 * SKIN, "on the ledge");
    t.keys().jump = true;
    t.tick();
    assert!(t.mv().in_jump, "jumps");
    // 15.2 u beyond: no overlap, down to the floor.
    let mut t = T::new(vec![floor(), ledge], on(0.0, 100.0, -15.2));
    t.ticks(60);
    close(t.feet(), 0.0, 2.0 * SKIN, "fell to the floor");
}

#[test]
fn case_47_air_spawn() {
    let mut t = T::new(vec![floor()], on(0.0, 200.0, 0.0));
    close(t.feet(), 0.0, 2.0 * SKIN, "starts on the floor");
    t.ticks(60);
    assert!(t.damage().is_empty(), "{:?}", t.damage());
}

#[test]
fn case_48_teleport_down_resets_nothing() {
    let upper = block(Vec3::new(0.0, -50.0, 0.0), Vec3::new(200.0, 50.0, 200.0));
    let lower = block(Vec3::new(2000.0, -450.0, 0.0), Vec3::new(500.0, 50.0, 500.0));
    let mut t = T::new(vec![upper, lower], on(0.0, 0.0, 0.0));
    t.tick();
    assert!(t.mv().on_ground);
    t.teleport(on(2000.0, -400.0, 0.0));
    t.ticks(3);
    let d = t.damage();
    assert_eq!(d.len(), 1, "{d:?}");
    close(d[0].amount * movement_data().health_points, 50.0, 0.01, "a 400 u drop");
}

#[test]
fn case_49_walk_and_sprint_together() {
    let mut t = T::standing();
    t.keys().walk = true;
    t.keys().sprint = true;
    t.steady_forward();
    let before = t.pos();
    t.tick();
    let m = t.mv();
    assert!(!m.sprint);
    assert_eq!(m.stamina_state, StaminaState::Recovering);
    assert_eq!(m.stamina, 100.0);
    close(m.speed_limit, 70.0, TOL, "limit");
    close(before.z - t.pos().z, 1.34081, TOL, "89.387 u/s");
}

#[test]
fn case_50_forward_back_and_sprint_drain() {
    let mut t = T::standing();
    t.keys().opposed.y = true;
    t.keys().sprint = true;
    // 1.000 s is 66.7 ticks: 67 ticks drain 30.15 (tolerance: one tick).
    t.ticks(67);
    let m = t.mv();
    assert!(!m.sprint, "no sprint");
    assert_eq!(m.stamina_state, StaminaState::Draining);
    close(m.speed_limit, 285.0, TOL, "run limit");
    assert_eq!(m.accel, Vec3::ZERO);
    close(m.stamina, 70.0, 0.45, "gauge after 1 s");
}

#[test]
fn case_51_opposing_keys_from_a_run() {
    let mut t = T::standing();
    t.steady_forward();
    close(t.hspeed(), 307.7608, TOL, "steady");
    t.keys().move_axis = Vec2::ZERO;
    t.keys().opposed.y = true;
    let before = t.pos();
    t.tick();
    close(t.hspeed(), 264.4069, TOL, "tick 1");
    close(before.z - t.pos().z, 4.11862, TOL, "tick 1 distance");
    assert_eq!(t.mv().accel, Vec3::ZERO);
    t.tick();
    close(t.hspeed(), 245.3018, TOL, "tick 2");
    t.tick();
    close(t.hspeed(), 227.5771, TOL, "tick 3: friction only, no braking");
}

#[test]
fn case_52_eye_height() {
    let d = movement_data();
    let mut t = T::standing();
    close(t.eye_above_feet(), 83.0, TOL, "standing");
    close(50.0 - 0.5 * 10.0 + movement::eye_above_centre(0.5, d), 71.5, TOL, "r = 0.5");
    let want = [77.0288, 72.3462, 68.7732, 66.1310, 64.2408];
    t.keys().crouch = true;
    for (n, want) in want.into_iter().enumerate() {
        t.tick();
        close(t.eye_above_feet(), want, TOL, &format!("tick {}", n + 1));
    }
    t.ticks(20);
    close(t.eye_above_feet(), 60.0, TOL, "crouched");
}

#[test]
fn case_53_camera_smoothing_up_a_step() {
    let d = movement_data();
    let mut cam = movement::CameraRise::default();
    cam.step(0.0, 1.0 / 60.0, false, d);
    let want = [5.0, 9.1667, 12.6389, 15.5324, 17.9437, 20.2770, 22.6103, 24.9437, 27.2770, 30.0];
    for (n, want) in want.into_iter().enumerate() {
        let h = cam.step(30.0, 1.0 / 60.0, false, d);
        close(h, want, 1e-3, &format!("frame {}", n + 1));
    }
    // Down at once; in a jump at once.
    assert_eq!(cam.step(10.0, 1.0 / 60.0, false, d), 10.0);
    assert_eq!(cam.step(40.0, 1.0 / 60.0, true, d), 40.0);
}

#[test]
fn case_54_unit_scale() {
    let d = movement_data();
    assert_eq!(movement::hud_metres(400.0, d), 10);
    assert_eq!(movement::hud_metres(419.0, d), 10);
}

#[test]
fn case_55_knock_back() {
    use movement::{CaInfected, HitKnockback};
    let push = |t: &mut T| {
        let k = HitKnockback::hit(None, 40.0, 0.3, 0.0, Vec3::Z, 0.0).unwrap();
        close(k.duration, 0.3, 1e-6, "0.3 s");
        t.insert((CaInfected, k));
    };
    let mut t = T::standing();
    push(&mut t);
    t.tick();
    let m = t.mv();
    close(m.accel.z, 1600.0, 0.01, "K^2 along the hit");
    assert!(!m.run, "run off");
    close(t.vel().z, 23.1221, TOL, "first tick's end speed");
    // Crouch held, no movement key: no velocity, no push.
    let mut t = T::standing();
    t.keys().crouch = true;
    push(&mut t);
    t.tick();
    assert_eq!(t.mv().accel, Vec3::ZERO);
    assert_eq!(t.hspeed(), 0.0);
    // Over after 0.3 s: 20 ticks of 15 ms.
    let mut t = T::standing();
    push(&mut t);
    t.ticks(21);
    assert_eq!(t.mv().accel, Vec3::ZERO, "over");
    // The M9 (duration 0): none; StiffDuration 0.5: 0.45 s.
    assert!(HitKnockback::hit(None, 20.0, 0.0, 0.0, Vec3::Z, 0.0).is_none());
    close(HitKnockback::hit(None, 40.0, 0.3, 0.5, Vec3::Z, 0.0).unwrap().duration, 0.45, 1e-6, "stiff");
}
