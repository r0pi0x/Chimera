//! CS:S-specific movement measured on the real game with the movement
//! probe (specs/cs_source/movement.md, "CS:S values"): jump stamina, the
//! bunny-hop speed cap, auto-bunnyhopping, and console variables. Flat
//! floor, CS:S tick (0.015 s), CS:S config.

use bevy::prelude::*;
use crate::stack::{
    core::Velocity,
    games::cs_source::{
        TICK_INTERVAL,
        movement::{self, SourceMovement, SourceMovementPlugin, to_engine, to_source},
    },
    harness::{CsSim, Sim},
    map::{MapBrush, MapBrushes},
};

fn test_map(mut commands: Commands) {
    let (a, b) = (
        to_engine(Vec3::new(-16384.0, -16384.0, -64.0)),
        to_engine(Vec3::new(16384.0, 16384.0, 0.0)),
    );
    commands.insert_resource(MapBrushes(vec![MapBrush::from_box(a.min(b), a.max(b))]));
}

struct TestMap;
impl Plugin for TestMap {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, test_map);
    }
}

struct Player {
    sim: Sim,
    p: Entity,
}

impl Player {
    fn new(cvars: &[(&str, &str)]) -> Self {
        let mut sim = Sim::new(CsSim, (TestMap, SourceMovementPlugin));
        sim.set_tick_interval(TICK_INTERVAL);
        let mut cfg = movement::config();
        for (n, v) in cvars {
            cfg.set_cvar(n, v).unwrap();
        }
        movement::set_config(sim.app.world_mut(), cfg);
        let p = sim.spawn_character(to_engine(Vec3::new(0.0, 0.0, 36.5)), movement::ID);
        sim.ticks(10);
        // Face Source yaw 0 (+X).
        sim.intent(p).yaw = -std::f32::consts::FRAC_PI_2;
        Self { sim, p }
    }
    fn vel(&self) -> Vec3 {
        to_source(self.sim.velocity(self.p))
    }
    fn speed(&self) -> f32 {
        self.vel().truncate().length()
    }
    fn set_vel(&mut self, v: Vec3) {
        self.sim.app.world_mut().get_mut::<Velocity>(self.p).unwrap().0 = to_engine(v);
    }
    fn on_ground(&self) -> bool {
        self.sim.app.world().get::<SourceMovement>(self.p).unwrap().on_ground
    }
    fn tick(&mut self) {
        self.sim.ticks(1);
    }
    /// Run at full speed, jump (forward still held), and return the speed
    /// on each tick from the landing on, with forward held or not.
    fn run_jump_land(&mut self, hold_after: bool, ticks_after: usize) -> Vec<f32> {
        self.sim.intent(self.p).move_axis = Vec2::Y;
        self.sim.ticks(60);
        self.sim.intent(self.p).jump = true;
        self.tick();
        self.sim.intent(self.p).jump = false;
        while !self.on_ground() {
            self.tick();
        }
        if !hold_after {
            self.sim.intent(self.p).move_axis = Vec2::ZERO;
        }
        (0..ticks_after)
            .map(|_| {
                self.tick();
                self.speed()
            })
            .collect()
    }
}

/// CS:S: after a running jump, landing with forward held dips to ~148 and
/// recovers to 250 within ~40 ticks (CS:S trace: 246.9, 225.4, 208.1, ...,
/// 148.4 minimum, 250 again by 30-40 ticks).
#[test]
fn landing_slowdown_and_recovery() {
    let mut pl = Player::new(&[]);
    let speeds = pl.run_jump_land(true, 60);
    let min = speeds.iter().cloned().fold(f32::MAX, f32::min);
    assert!((140.0..160.0).contains(&min), "lowest speed after landing {min}");
    assert!((speeds[59] - 250.0).abs() < 0.5, "recovered to {}", speeds[59]);
}

/// CS:S: releasing forward on landing stops within ~20 ticks (stamina
/// scales speed on top of friction); plain friction would take ~45.
#[test]
fn quick_stop_after_landing() {
    let mut pl = Player::new(&[]);
    let speeds = pl.run_jump_land(false, 40);
    let stop = speeds.iter().position(|s| *s < 0.5).expect("never stopped");
    assert!((14..=24).contains(&stop), "stopped after {stop} ticks ({speeds:?})");

    let mut plain = Player::new(&[]);
    movement::config_mut(plain.sim.app.world_mut())
        .jump_stamina = 0.0;
    let speeds = plain.run_jump_land(false, 80);
    let stop_plain = speeds.iter().position(|s| *s < 0.5).unwrap();
    assert!(stop_plain > stop + 10, "without stamina: {stop_plain} ticks");
}

/// CS:S: a jump 50 ticks after the previous one is weaker: stamina
/// 1315.79 - 750 = 565.79 ms scales the jump by 1 - 0.00019 x 565.79.
#[test]
fn repeated_jumps_are_lower() {
    let mut pl = Player::new(&[]);
    let mut takeoff = Vec::new();
    for _ in 0..2 {
        pl.sim.intent(pl.p).jump = true;
        pl.tick();
        takeoff.push(pl.vel().z);
        pl.sim.intent(pl.p).jump = false;
        pl.sim.ticks(49);
    }
    let r = 1.0 - 0.00019 * 565.789_4;
    // Standing jump: (vz after the first gravity half + impulse) x r, then
    // the extra and the second gravity halves (6 each at 0.015 s).
    let expected = ((2.0f32 * 800.0 * 57.0).sqrt() - 6.0) * r - 12.0;
    assert!(
        (takeoff[0] - ((2.0f32 * 800.0 * 57.0).sqrt() - 18.0)).abs() < 0.01,
        "{takeoff:?}"
    );
    assert!(
        (takeoff[1] - expected).abs() < 0.05,
        "second jump {} expected {expected}",
        takeoff[1]
    );
}

/// CS:S with sv_enablebunnyhopping 0: jumping caps speed at 286 (3D);
/// with 1, no cap.
#[test]
fn bunnyhop_cap() {
    for (setting, expected) in [("0", 285.94f32), ("1", 400.0 * 0.94)] {
        let mut pl = Player::new(&[("sv_enablebunnyhopping", setting)]);
        pl.set_vel(Vec3::new(400.0, 0.0, 0.0));
        pl.tick(); // one ground tick: friction 400 -> 376
        pl.sim.intent(pl.p).jump = true;
        pl.tick();
        let s = pl.speed();
        assert!(
            (s - expected).abs() < 0.1,
            "sv_enablebunnyhopping {setting}: {s}, expected {expected}"
        );
    }
}

/// sv_autobunnyhopping 1: holding jump jumps again on landing.
#[test]
fn auto_bunnyhopping() {
    for (setting, min_jumps) in [("0", 1), ("1", 3)] {
        let mut pl = Player::new(&[("sv_autobunnyhopping", setting)]);
        pl.sim.intent(pl.p).jump = true;
        let mut jumps = 0;
        let mut was_ground = true;
        for _ in 0..200 {
            pl.tick();
            if was_ground && !pl.on_ground() {
                jumps += 1;
            }
            was_ground = pl.on_ground();
        }
        if setting == "0" {
            assert_eq!(jumps, 1, "holding jump without autobhop");
        } else {
            assert!(jumps >= min_jumps, "autobhop jumped {jumps} times");
        }
    }
}

/// Console variables, as `--cvar` and `--exec` set them.
#[test]
fn console_variables() {
    let mut cfg = movement::config();
    cfg.set_cvar("sv_airaccelerate", "150").unwrap();
    assert_eq!(cfg.airaccelerate, 150.0);
    let problems = cfg.exec(
        "// surf server\nsv_airaccelerate \"1000\"\nsv_gravity 800 // default\nsv_enablebunnyhopping 1\nsv_cheats 1\n",
    );
    assert_eq!(cfg.airaccelerate, 1000.0);
    assert!(cfg.enable_bunnyhopping);
    assert_eq!(problems, vec!["unknown console variable sv_cheats".to_string()]);
    assert!(cfg.set_cvar("sv_friction", "fast").is_err());
}

/// CS:S walking (+speed), measured on the probe server with the knife: the
/// max speed drops to 250 x 0.52 = 130. From rest the first tick adds
/// 5 x 130 x 0.015 = 9.75, then 5.25 a tick below stopspeed, levelling off
/// at 130. Walking ducked holds at 3.3149: one tick of acceleration at
/// 250 x 0.52 x 0.34 = 44.2, which friction wipes every tick.
#[test]
fn walking() {
    let mut pl = Player::new(&[]);
    let i = pl.p;
    pl.sim.intent(i).walk = true;
    pl.sim.intent(i).move_axis = Vec2::Y;
    let expected = [9.75, 15.0, 20.25, 25.5, 30.75];
    for e in expected {
        pl.tick();
        assert!((pl.speed() - e).abs() < 1e-3, "walk speed {} vs {e}", pl.speed());
    }
    pl.sim.ticks(60);
    assert!((pl.speed() - 130.0).abs() < 1e-3, "walk top speed {}", pl.speed());

    pl.sim.intent(i).move_axis = Vec2::ZERO;
    pl.sim.ticks(40);
    pl.sim.intent(i).crouch = true;
    pl.sim.intent(i).move_axis = Vec2::Y;
    for _ in 0..30 {
        pl.tick();
        assert!((pl.speed() - 3.3149).abs() < 1e-3, "duck-walk speed {}", pl.speed());
    }
}

/// Measured (movecmp `bhop_scroll`, 2026-10-16): running at (-2.1734,
/// -250) and jumping with only a right strafe held, looking at yaw 267.5,
/// the jump tick gets the full air acceleration (10 x 250 x 0.015 = 37.5,
/// cut to what the 30 cap leaves): (-39.6377, -248.3642) after the tick.
/// The bot ducks on its own as it jumps; that doesn't slow this tick's
/// input as a held duck key would (x 0.34 gives (-22.55, -249.11)).
#[test]
fn a_strafe_on_the_jump_tick_gets_the_full_air_acceleration() {
    let mut pl = Player::new(&[]);
    let i = pl.p;
    pl.set_vel(Vec3::new(-2.1734, -250.0, 0.0));
    pl.sim.intent(i).yaw = 267.5f32.to_radians() - std::f32::consts::FRAC_PI_2;
    pl.sim.intent(i).move_axis = Vec2::X;
    pl.sim.intent(i).jump = true;
    pl.tick();
    let v = pl.vel();
    assert!(!pl.on_ground());
    assert!((v.x - -39.6377).abs() < 0.01 && (v.y - -248.3642).abs() < 0.01, "{v:?}");
}

impl Player {
    fn me(&self) -> SourceMovement {
        self.sim.app.world().get::<SourceMovement>(self.p).unwrap().clone()
    }
}

/// Measured (movecmp `duck_release_7`, 2026-10-16): duck held 7 ticks
/// from standing, then released. The duck timer keeps running: on the
/// release tick the eye drops to the unduck curve's 105 ms point (64 - 17
/// x S(1 - 105/200) = 56.137), then rises along it and the player stands 7
/// ticks later. Forward is slowed (x 0.34) up to that tick, then not.
#[test]
fn duck_released_partway_keeps_its_timer() {
    let mut pl = Player::new(&[]);
    let i = pl.p;
    pl.sim.intent(i).move_axis = Vec2::Y;
    pl.sim.intent(i).crouch = true;
    pl.sim.ticks(7);
    pl.sim.intent(i).crouch = false;
    let eyes = [56.1369, 58.0159, 59.7802, 61.3437, 62.6203, 63.5239, 63.9686];
    for e in eyes {
        pl.tick();
        let me = pl.me();
        assert!((me.eye - e).abs() < 1e-3, "eye {} vs {e}", me.eye);
        assert!(me.ducking, "still standing up");
    }
    let before = pl.speed();
    pl.tick();
    let me = pl.me();
    assert!(!me.ducking && !me.ducked && me.eye == 64.0, "stood up: {me:?}");
    // Below the stop speed friction takes 4.5 a tick; the tick it stands
    // up is still slowed (it started mid-duck): + 5 x 85 x 0.015.
    assert!((pl.speed() - before - (6.375 - 4.5)).abs() < 0.01, "{} -> {}", before, pl.speed());
    let before = pl.speed();
    pl.tick();
    assert!((pl.speed() - before - (18.75 - 4.5)).abs() < 0.01, "{} -> {}", before, pl.speed());
}

/// Measured (movecmp `unduck_jump_5`/`_10`, 2026-10-16): fully ducked and
/// walking at 85, duck released, and jump pressed while standing back up.
/// No jump, but the view drops back to the ducked eye (47), the jump's
/// stamina is spent (85 -> 65.44 that tick, against 85 without it), and
/// the player is standing on the next tick (eye 64), the jump key still
/// counting as held.
#[test]
fn a_jump_while_standing_up_ducks_and_costs_stamina() {
    let mut pl = Player::new(&[]);
    let i = pl.p;
    pl.sim.intent(i).move_axis = Vec2::Y;
    pl.sim.intent(i).crouch = true;
    pl.sim.ticks(60);
    assert!(pl.me().ducked && (pl.speed() - 85.0).abs() < 0.01, "{}", pl.speed());
    pl.sim.intent(i).crouch = false;
    pl.sim.ticks(5);
    assert!(pl.me().ducking && pl.me().eye > 50.0);
    pl.sim.intent(i).jump = true;
    pl.tick();
    let me = pl.me();
    assert!(pl.on_ground() && me.ducked && me.eye == 47.0, "{me:?}");
    assert!((pl.speed() - 65.4442).abs() < 0.15, "speed {}", pl.speed());
    pl.tick();
    let me = pl.me();
    assert!(pl.on_ground() && !me.ducked && !me.ducking && me.eye == 64.0, "{me:?}");
    assert!((pl.speed() - 51.6101).abs() < 0.15, "speed {}", pl.speed());
}

struct WallMap;
impl Plugin for WallMap {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, |mut commands: Commands| {
            let b = |lo: Vec3, hi: Vec3| {
                let (a, b) = (to_engine(lo), to_engine(hi));
                MapBrush::from_box(a.min(b), a.max(b))
            };
            commands.insert_resource(MapBrushes(vec![
                b(Vec3::new(-4096.0, -4096.0, -64.0), Vec3::new(4096.0, 4096.0, 0.0)),
                // A wall whose face (normal -y) is at y = 100.
                b(Vec3::new(-4096.0, 100.0, 0.0), Vec3::new(4096.0, 200.0, 200.0)),
            ]));
        });
    }
}

/// Measured (movecmp `wall_slide` on de_dust2 and the fuzz runs,
/// 2026-10-16): a move stopped by a surface leaves 1/32 unit/s of velocity
/// away from it. Running along a wall pressed into it at a slant, CS:S
/// logs 0.0312 along the wall's normal every tick, and the box settles
/// 0.0317 from the face (the 1/32 back-off plus 1/32 x 0.015 s), where the
/// shared code's clip leaves it at 0.03125 with no velocity off the wall.
#[test]
fn a_stopped_move_keeps_a_32nd_of_a_unit_away_from_the_wall() {
    let mut sim = Sim::new(CsSim, (WallMap, SourceMovementPlugin));
    sim.set_tick_interval(TICK_INTERVAL);
    movement::set_config(sim.app.world_mut(), movement::config());
    let p = sim.spawn_character(to_engine(Vec3::new(0.0, 60.0, 36.5)), movement::ID);
    sim.ticks(10);
    // Source yaw 70: mostly into the wall (+y), partly along it (+x).
    sim.intent(p).yaw = 70f32.to_radians() - std::f32::consts::FRAC_PI_2;
    sim.intent(p).move_axis = Vec2::Y;
    sim.ticks(40);
    let feet = to_source(sim.position(p)) - Vec3::Z * 36.0;
    let v = to_source(sim.velocity(p));
    assert!(v.x > 50.0, "slides along the wall: {v}");
    assert!((v.y + 1.0 / 32.0).abs() < 1e-4, "off the wall: {v}");
    let gap = 100.0 - (feet.y + 16.0);
    assert!((gap - (1.0 + 0.015) / 32.0).abs() < 2e-4, "gap {gap}");
}

/// cs_militia's back porch, in its own Source coordinates: the floor
/// (z -16) and the concrete wall whose sloped face (normal 0, 0.9231,
/// 0.3846: too steep to stand on) meets the floor at y 216.
struct PorchMap;
impl Plugin for PorchMap {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, |mut commands: Commands| {
            let b = |lo: Vec3, hi: Vec3| {
                let (a, b) = (to_engine(lo), to_engine(hi));
                MapBrush::from_box(a.min(b), a.max(b))
            };
            let mut wall = b(Vec3::new(-192.0, 80.0, -16.0), Vec3::new(136.0, 216.0, 80.0));
            // The face as a Source plane (n . p = d), in engine space.
            let n = Vec3::new(0.0, 0.9231, 0.3846);
            wall.planes.push((to_engine(n).normalize(), 193.231 * to_engine(Vec3::X).x));
            commands.insert_resource(MapBrushes(vec![
                b(Vec3::new(-192.0, 80.0, -180.0), Vec3::new(136.0, 216.0, -16.0)),
                b(Vec3::new(-144.0, 216.0, -18.0), Vec3::new(-56.0, 272.0, -16.0)),
                b(Vec3::new(-56.0, 216.0, -18.0), Vec3::new(120.0, 336.0, -16.0)),
                wall,
            ]));
        });
    }
}

/// Measured (movecmp `militia_porch_wall_right`, 2026-10-17): walking along
/// the porch wall into its sloped face, the clip against the face lifts the
/// feet about 0.105 units a tick while the floor still holds them. The tick
/// they end 2.0024 units above the floor (z -13.9976), CS:S's 2-unit ground
/// probe no longer finds it, though it ends only 0.0024 short of the
/// floor's plane, within the 1/32 back-off: a trace that ends in front of a
/// plane doesn't hit it. That tick the player is airborne, keeping the
/// clip's vz (1.0236 after the second half of gravity), and lands the next.
#[test]
fn a_floor_just_beyond_the_ground_probe_is_not_ground() {
    let mut sim = Sim::new(CsSim, (PorchMap, SourceMovementPlugin));
    sim.set_tick_interval(TICK_INTERVAL);
    movement::set_config(sim.app.world_mut(), movement::config());
    let p = sim.spawn_character(to_engine(Vec3::new(-69.83, 256.39, -15.97 + 36.0)), movement::ID);
    sim.app.world_mut().get_mut::<SourceMovement>(p).unwrap().on_ground = true;
    sim.intent(p).yaw = 245.27f32.to_radians() - std::f32::consts::FRAC_PI_2;
    sim.ticks(20);
    sim.intent(p).move_axis = Vec2::Y;
    let state = |sim: &Sim| {
        let me = sim.app.world().get::<SourceMovement>(p).unwrap();
        (to_source(sim.position(p)) - Vec3::Z * 36.0, to_source(sim.velocity(p)), me.on_ground)
    };
    // CS:S's ticks 50-52: feet, velocity, on the ground.
    let css = [
        (Vec3::new(-102.0175, 231.2438, -14.1028), Vec3::new(-109.6449, -2.8926, 0.0), true),
        (Vec3::new(-103.6811, 231.2, -13.9976), Vec3::new(-110.9101, -2.8926, 1.0236), false),
        (Vec3::new(-105.3448, 231.2201, -14.0458), Vec3::new(-110.9101, 1.3676, 0.0), true),
    ];
    sim.ticks(29);
    for (t, (feet, vel, ground)) in css.into_iter().enumerate() {
        sim.ticks(1);
        let (f, v, g) = state(&sim);
        assert!(
            f.distance(feet) < 0.02 && v.distance(vel) < 0.02 && g == ground,
            "tick {}: feet {f} v {v} ground {g}; CS:S {feet} {vel} {ground}",
            50 + t
        );
    }
}

/// A flat floor whose top (z 0) is a prop's surface (`MapBrush::surface`
/// set, as props' hulls are), or the world's.
struct TopMap(bool);
impl Plugin for TopMap {
    fn build(&self, app: &mut App) {
        let prop = self.0;
        app.add_systems(Startup, move |mut commands: Commands| {
            let (a, b) = (
                to_engine(Vec3::new(-512.0, -512.0, -64.0)),
                to_engine(Vec3::new(512.0, 512.0, 0.0)),
            );
            let mut top = MapBrush::from_box(a.min(b), a.max(b));
            if prop {
                top.surface = Some("wood".into());
            }
            commands.insert_resource(MapBrushes(vec![top]));
        });
    }
}

/// Measured (probe server, 2026-10-17; specs/cs_source/movement.md "Ground
/// detection"): a bot placed still above de_dust2's static prop at (-312,
/// 1416), whose flat top is at z -32, keeps the ground up to 2.031 units
/// above it and falls from 2.0315: the 2-unit probe still finds a prop's
/// surface it ends within the 1/32 back-off of (2 + 1/32 = 2.03125). A
/// world brush is not found 2.01 above (the porch measurement).
#[test]
fn a_prop_top_within_the_back_off_past_the_ground_probe_is_ground() {
    for (prop, gap, ground) in [(true, 2.025, true), (true, 2.04, false), (false, 2.01, false), (false, 1.99, true)] {
        let mut sim = Sim::new(CsSim, (TopMap(prop), SourceMovementPlugin));
        sim.set_tick_interval(TICK_INTERVAL);
        movement::set_config(sim.app.world_mut(), movement::config());
        let p = sim.spawn_character(to_engine(Vec3::new(0.0, 0.0, gap + 36.0)), movement::ID);
        sim.ticks(3);
        let feet = to_source(sim.position(p)).z - 36.0;
        if ground {
            assert!((feet - gap).abs() < 1e-3, "prop {prop} gap {gap}: held at {feet}");
        } else {
            assert!(feet < gap - 0.05, "prop {prop} gap {gap}: fell, at {feet}");
        }
    }
}
