//! TF2's movement and classes (specs/tf2/movement.md's test cases, their
//! numbers in the test names; classes.md test 1) on `Tf2Sim` with a flat
//! concrete floor at z = 0 in Source units: each player holds a TF2 gun,
//! which makes it that gun's class (the Scattergun a Scout, the Rocket
//! Launcher a Soldier, the Minigun a Heavy, the Shotgun an Engineer).
//! dt 0.015, gravity 800.

use bevy::prelude::*;
use chimera_core::core::{Health, Intent, MovementState, Velocity};
use chimera_sim::weapon::{Inventory, give};
use chimera_world::{harness::Sim, map::{MapBrush, MapBrushes}};
use source_engine::source_movement::{SourceMovement, SourceTraits, to_engine, to_source};
use tf2::{
    game::{
        TICK_INTERVAL,
        classes::{Classes, Tf2Class},
        movement::{self, ID},
        weapons::{MINIGUN, ROCKET_LAUNCHER, SCATTERGUN, SHOTGUN},
    },
    harness::Tf2Sim,
};

const TOL: f32 = 0.01;

fn floor(mut commands: Commands) {
    let (a, b) = (to_engine(Vec3::new(-8192.0, -8192.0, -64.0)), to_engine(Vec3::new(8192.0, 8192.0, 0.0)));
    commands.insert_resource(MapBrushes(vec![MapBrush {
        surface: Some("concrete".into()),
        ..MapBrush::from_box(a.min(b), a.max(b))
    }]));
}

struct Floor;
impl Plugin for Floor {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, floor);
    }
}

struct Player {
    sim: Sim,
    p: Entity,
}

impl Player {
    /// A player standing on the floor at the origin holding `gun`, settled,
    /// facing Source yaw 0 (+X).
    fn holding(gun: &str) -> Self {
        let mut sim = Sim::new(Tf2Sim, Floor);
        sim.set_tick_interval(TICK_INTERVAL);
        let p = sim.spawn_character(to_engine(Vec3::new(0.0, 0.0, 36.5)), ID);
        sim.ticks(2);
        let w = give(sim.app.world_mut(), p, gun).unwrap();
        sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
        let mut me = Self { sim, p };
        me.look(0.0);
        me.sim.seconds(1.0);
        assert!(me.ms().on_ground, "on the floor");
        me
    }

    fn look(&mut self, yaw: f32) {
        let mut i = self.sim.intent(self.p);
        i.yaw = yaw.to_radians() - std::f32::consts::FRAC_PI_2;
        i.pitch = 0.0;
    }

    fn intent(&mut self) -> Mut<'_, Intent> {
        self.sim.intent(self.p)
    }

    fn keys(&mut self, forward: f32, right: f32) {
        self.intent().move_axis = Vec2::new(right, forward);
    }

    fn feet(&self) -> Vec3 {
        to_source(self.sim.position(self.p)) - Vec3::Z * 36.0
    }

    fn vel(&self) -> Vec3 {
        to_source(self.sim.velocity(self.p))
    }

    fn set_vel(&mut self, v: Vec3) {
        self.sim.app.world_mut().get_mut::<Velocity>(self.p).unwrap().0 = to_engine(v);
    }

    fn speed(&self) -> f32 {
        self.vel().truncate().length()
    }

    fn me(&self) -> SourceMovement {
        self.sim.app.world().get::<SourceMovement>(self.p).unwrap().clone()
    }

    fn ms(&self) -> MovementState {
        self.sim.app.world().get::<MovementState>(self.p).unwrap().clone()
    }

    fn max(&self) -> f32 {
        self.sim.app.world().get::<SourceTraits>(self.p).unwrap().max_speed.unwrap()
    }

    fn class(&self) -> String {
        let w = self.sim.app.world();
        w.get::<Tf2Class>(self.p).unwrap().class(w.resource::<Classes>()).name.clone()
    }

    fn health(&self) -> Health {
        *self.sim.app.world().get::<Health>(self.p).unwrap()
    }

    fn tick(&mut self) {
        self.sim.ticks(1);
    }

    /// Jump pressed for one tick (then let go).
    fn press_jump(&mut self) {
        self.intent().jump = true;
        self.tick();
        self.intent().jump = false;
    }
}

/// Classes.md test 1, for the four guns' classes: the gun makes the class,
/// its max health and top speed.
#[test]
fn each_gun_makes_its_class_with_its_health_and_speed_classes_1() {
    for (gun, class, health, speed) in [
        (SCATTERGUN, "scout", 125.0, 400.0),
        (ROCKET_LAUNCHER, "soldier", 200.0, 240.0),
        (MINIGUN, "heavy", 300.0, 230.0),
        (SHOTGUN, "engineer", 125.0, 300.0),
    ] {
        let me = Player::holding(gun);
        assert_eq!(me.class(), class, "{gun}");
        let h = me.health();
        assert!((h.max * 100.0 - health).abs() < TOL && (h.current - h.max).abs() < 1e-6, "{class}: {h:?}");
        assert_eq!(me.max(), speed, "{class}");
        // The box: 48 wide, 82 tall (classes.md test 1).
        let s = me.ms();
        let size = to_source(s.hull_max - s.hull_min).abs();
        assert!((size.x - 48.0).abs() < TOL && (size.y - 48.0).abs() < TOL && (size.z - 82.0).abs() < TOL, "{size}");
    }
}

/// 1, 2, 22: from rest, holding forward: the Scout and the Heavy accelerate
/// by 0.15 x their top speed a tick (with friction above 100) to 400 and 230.
#[test]
fn the_scout_and_the_heavy_run_at_their_class_speeds_1_2_22() {
    let mut scout = Player::holding(SCATTERGUN);
    scout.keys(1.0, 0.0);
    let want = [60.0, 114.0, 167.16, 217.1304, 264.1026, 308.2564, 349.761, 388.7754, 400.0];
    for (k, w) in want.iter().enumerate() {
        scout.tick();
        assert!((scout.speed() - w).abs() < 0.02, "Scout tick {}: {} != {w}", k + 1, scout.speed());
    }
    scout.sim.seconds(0.5);
    assert!((scout.speed() - 400.0).abs() < TOL, "stays at 400: {}", scout.speed());

    let mut heavy = Player::holding(MINIGUN);
    heavy.keys(1.0, 0.0);
    let want = [34.5, 63.0, 91.5, 120.0, 147.3, 172.962, 197.0843, 219.7592, 230.0];
    for (k, w) in want.iter().enumerate() {
        heavy.tick();
        assert!((heavy.speed() - w).abs() < 0.02, "Heavy tick {}: {} != {w}", k + 1, heavy.speed());
    }
}

/// Class top speed, "Backwards": a Scout holding back runs at 0.9 x 400.
#[test]
fn backwards_is_nine_tenths_of_the_class_speed() {
    let mut me = Player::holding(SCATTERGUN);
    me.keys(-1.0, 0.0);
    me.sim.seconds(1.0);
    assert!((me.speed() - 360.0).abs() < TOL, "back {}", me.speed());
    let mut soldier = Player::holding(ROCKET_LAUNCHER);
    soldier.keys(-1.0, 0.0);
    soldier.sim.seconds(1.0);
    assert!((soldier.speed() - 216.0).abs() < TOL, "back {}", soldier.speed());
}

/// 3: a fully ducked Scout's wish is a third of 400: 20, 34, 48 ... 133.33.
#[test]
fn a_ducked_scout_walks_at_a_third_3() {
    let mut me = Player::holding(SCATTERGUN);
    me.intent().crouch = true;
    me.sim.seconds(0.5);
    assert!(me.me().ducked && !me.me().ducking, "fully ducked");
    me.keys(1.0, 0.0);
    for w in [20.0, 34.0, 48.0] {
        me.tick();
        assert!((me.speed() - w).abs() < 0.02, "{} != {w}", me.speed());
    }
    me.sim.seconds(1.0);
    assert!((me.speed() - 400.0 / 3.0).abs() < 0.02, "{}", me.speed());
}

/// 5: any class's standing jump: stored vz 271, 259, 247, the feet up
/// 4.155 then 8.13, the apex 50.04 at the end of tick 24.
#[test]
fn a_jump_peaks_at_50_units_5() {
    for gun in [SCATTERGUN, MINIGUN] {
        let mut me = Player::holding(gun);
        let z0 = me.feet().z;
        me.intent().jump = true;
        let mut zs = Vec::new();
        for k in 1..=30 {
            me.tick();
            if k <= 3 {
                assert!((me.vel().z - [271.0, 259.0, 247.0][k - 1]).abs() < TOL, "tick {k}: vz {}", me.vel().z);
            }
            zs.push(me.feet().z - z0);
        }
        me.intent().jump = false;
        assert!((zs[0] - 4.155).abs() < TOL && (zs[1] - 8.13).abs() < TOL, "{:?}", &zs[..2]);
        let (top, at) = zs.iter().enumerate().fold((0.0f32, 0), |(m, i), (k, z)| if *z > m { (*z, k + 1) } else { (m, i) });
        assert!((top - 50.04).abs() < 0.02 && at == 24, "apex {top} at tick {at}");
    }
}

/// 6: a duck pressed while rising lifts the feet 20 that tick; the apex
/// becomes 70 above the floor.
#[test]
fn an_air_duck_reaches_70_6() {
    let mut me = Player::holding(ROCKET_LAUNCHER);
    let z0 = me.feet().z;
    me.intent().jump = true;
    me.sim.ticks(8);
    me.intent().crouch = true;
    let before = me.feet().z;
    me.tick();
    let rise = me.feet().z - before;
    assert!((rise - (20.0 + 2.715)).abs() < 0.05, "rose {rise}");
    assert!((me.me().eye - 45.0).abs() < TOL && me.me().ducked);
    let mut top: f32 = 0.0;
    for _ in 0..30 {
        me.tick();
        top = top.max(me.feet().z - z0);
    }
    assert!((top - 70.04).abs() < 0.05, "apex {top}");
}

/// 7, 9: the Scout's air jump on tick 19 of a jump: stored vz 262.328,
/// 250.328, 238.328, the move of that tick rising 4.025; a second air
/// jump does nothing. 10: the Heavy has none.
#[test]
fn the_scouts_air_jump_7_9_10() {
    let mut me = Player::holding(SCATTERGUN);
    me.press_jump();
    me.sim.ticks(17);
    assert!((me.vel().z - 67.0).abs() < TOL, "before: {}", me.vel().z);
    let before = me.feet().z;
    me.intent().jump = true;
    me.tick();
    assert!((me.feet().z - before - 4.025).abs() < TOL, "rose {}", me.feet().z - before);
    for w in [262.328, 250.328, 238.328] {
        assert!((me.vel().z - w).abs() < TOL, "vz {} != {w}", me.vel().z);
        me.tick();
    }
    // Let go and press again: nothing.
    me.intent().jump = false;
    me.tick();
    let vz = me.vel().z;
    me.press_jump();
    assert!((me.vel().z - (vz - 12.0)).abs() < TOL, "a second air jump: {} after {vz}", me.vel().z);

    let mut heavy = Player::holding(MINIGUN);
    heavy.press_jump();
    heavy.sim.ticks(10);
    let vz = heavy.vel().z;
    heavy.press_jump();
    assert!((heavy.vel().z - (vz - 12.0)).abs() < TOL, "the Heavy jumped in the air: {}", heavy.vel().z);
}

/// 8: a Scout moving at (300, 0) in the air with no key held stops dead
/// horizontally on the air jump; with forward held it goes at 400 the way
/// he looks.
#[test]
fn the_air_jump_takes_the_keys_velocity_8() {
    let mut me = Player::holding(SCATTERGUN);
    me.press_jump();
    me.sim.ticks(5);
    me.set_vel(Vec3::new(300.0, 0.0, me.vel().z));
    me.press_jump();
    assert!(me.vel().truncate().length() < TOL, "kept {}", me.vel());
    assert!((me.vel().z - 262.328).abs() < TOL);

    let mut me = Player::holding(SCATTERGUN);
    me.press_jump();
    me.sim.ticks(5);
    me.set_vel(Vec3::new(300.0, 0.0, me.vel().z));
    me.look(90.0);
    me.keys(1.0, 0.0);
    me.press_jump();
    let v = me.vel().truncate();
    assert!((v - Vec2::new(0.0, 400.0)).length() < 0.05, "{v}");
}

/// 11: a Scout fully ducked on the ground can't jump.
#[test]
fn no_jump_fully_ducked_11() {
    let mut me = Player::holding(SCATTERGUN);
    me.intent().crouch = true;
    me.sim.seconds(0.5);
    me.intent().jump = true;
    for _ in 0..60 {
        me.tick();
        assert!(me.ms().on_ground && me.vel().z.abs() < TOL, "left the ground");
    }
}

/// 12-14: the anti-bunny-hop cap: a jump at 600 leaves at 1.2 x the class
/// speed (vz too: the cap includes the gravity half-step); below it
/// nothing changes.
#[test]
fn the_bunny_hop_cap_is_1_2_class_speeds_12_13_14() {
    for (gun, vx, vz) in [(SCATTERGUN, 479.976, 272.2), (ROCKET_LAUNCHER, 287.986, 274.12)] {
        let mut me = Player::holding(gun);
        me.set_vel(Vec3::new(600.0, 0.0, 0.0));
        me.press_jump();
        let v = me.vel();
        assert!((v.x - vx).abs() < 0.01 && (v.z - vz).abs() < 0.01, "{gun}: {v}");
    }
    let mut me = Player::holding(SCATTERGUN);
    me.set_vel(Vec3::new(450.0, 0.0, 0.0));
    me.press_jump();
    let v = me.vel();
    assert!((v.x - 450.0).abs() < 0.01 && (v.z - 271.0).abs() < 0.01, "{v}");
}

/// 15: a Scout's duck: the eye follows the smooth curve for 13 ticks and
/// is 45 with the 62 box on the 14th.
#[test]
fn the_duck_takes_a_fifth_of_a_second_15_16() {
    let mut me = Player::holding(SCATTERGUN);
    assert!((me.me().eye - 65.0).abs() < TOL, "eye {}", me.me().eye);
    me.intent().crouch = true;
    let want = [64.6794, 63.785, 62.4181, 60.68, 58.6719, 56.495, 54.2506, 52.04, 49.9644, 48.125, 46.6231, 45.56, 45.0369];
    for (k, w) in want.iter().enumerate() {
        me.tick();
        assert!((me.me().eye - w).abs() < 0.001, "tick {}: eye {} != {w}", k + 1, me.me().eye);
        assert!(!me.me().ducked, "box switched early at {}", k + 1);
    }
    me.tick();
    assert!(me.me().ducked && (me.me().eye - 45.0).abs() < TOL);
    let s = me.ms();
    assert!((to_source(s.hull_max - s.hull_min).abs().z - 62.0).abs() < TOL);
    // 16: the unduck mirrors it.
    me.sim.ticks(5);
    me.intent().crouch = false;
    me.tick();
    assert!((me.me().eye - 45.3206).abs() < 0.001, "unduck tick 1: {}", me.me().eye);
    me.sim.ticks(12);
    assert!((me.me().eye - 64.9631).abs() < 0.001 && me.me().ducked, "unduck tick 13: {}", me.me().eye);
    me.tick();
    assert!(!me.me().ducked && (me.me().eye - 65.0).abs() < TOL);
}

/// 18-20: fall damage with the spread off: max health x speed / 6000,
/// rounded, above 650.
#[test]
fn fall_damage_is_a_share_of_max_health_18_19_20() {
    let land = |gun: &str, f: f32| {
        let mut me = Player::holding(gun);
        movement::config_mut(me.sim.app.world_mut()).fall_spread = 0.0;
        // Feet a little above the floor, falling so the landing tick starts
        // at -f (its first half-step adds 6 to the fall).
        let at = to_engine(Vec3::new(0.0, 0.0, 36.0 + 2.5));
        let w = me.sim.app.world_mut();
        w.get_mut::<Transform>(me.p).unwrap().translation = at;
        if let Some(mut pos) = w.get_mut::<avian3d::prelude::Position>(me.p) {
            pos.0 = at;
        }
        me.set_vel(Vec3::new(0.0, 0.0, -f));
        let before = me.health().current;
        for _ in 0..3 {
            me.tick();
        }
        ((before - me.health().current) * 100.0).round()
    };
    assert_eq!(land(MINIGUN, 640.0), 0.0);
    // The landing reads the fall speed at the start of its tick: the
    // velocity set, then the tick before it adds 12.
    let heavy = |f: f32| (300.0 * f / 6000.0).round();
    let f = 680.0;
    let got = land(MINIGUN, f);
    assert!(got == heavy(f) || got == heavy(f + 6.0) || got == heavy(f + 12.0), "Heavy at {f}: {got}");
    let soldier = |f: f32| (200.0 * f / 6000.0).round();
    let f = 1384.0;
    let got = land(ROCKET_LAUNCHER, f);
    assert!((got - soldier(f)).abs() <= 1.0, "Soldier at {f}: {got} (want about {})", soldier(f));
}

/// 23: the Heavy spinning the Minigun (attack2) moves at 110; wound down,
/// 230 again.
#[test]
fn the_spun_heavy_moves_at_110_23() {
    let mut me = Player::holding(MINIGUN);
    me.intent().secondary = true;
    me.keys(1.0, 0.0);
    me.tick();
    me.tick();
    assert_eq!(me.max(), 110.0);
    me.sim.seconds(1.0);
    assert!((me.speed() - 110.0).abs() < TOL, "spun {}", me.speed());
    me.intent().secondary = false;
    me.sim.seconds(2.0);
    assert_eq!(me.max(), 230.0);
    assert!((me.speed() - 230.0).abs() < TOL, "wound down {}", me.speed());
}

/// Classes.md test 1, the eyes: each class's eye stands its height above
/// its feet (Scout 65, Soldier 68, Heavy 75, Engineer 68), measured as a
/// camera takes it (the body's place plus `MovementState::eye_offset`),
/// with the box's bottom at the feet; once it has walked (the stay-on-
/// ground snap; standing still it may hover up to 2 units, CS:S's
/// movement.md "Hovering", as the spawn left it) the feet are on the
/// floor and the eye is its height above the floor. No offset is shared
/// by every class (the Windows check found each 2.07 over the spec's
/// height on de_dust2: a box resting on the map, not the eye).
#[test]
fn each_class_eye_stands_its_height_above_the_floor_classes_1() {
    for (gun, eye) in [(SCATTERGUN, 65.0), (ROCKET_LAUNCHER, 68.0), (MINIGUN, 75.0), (SHOTGUN, 68.0)] {
        let mut me = Player::holding(gun);
        let camera = |me: &Player| to_source(me.sim.position(me.p) + me.ms().eye_offset);
        assert!((camera(&me).z - me.feet().z - eye).abs() < TOL, "{gun}: eye {} above the feet", camera(&me).z - me.feet().z);
        assert!(me.feet().z >= 0.0 && me.feet().z <= 2.0, "{gun}: hovering {}", me.feet().z);
        let s = me.ms();
        let at = me.sim.position(me.p);
        let bottom = to_source(at + s.hull_min).z.min(to_source(at + s.hull_max).z);
        assert!((bottom - me.feet().z).abs() < TOL, "{gun}: the box's bottom at {bottom}");
        me.keys(1.0, 0.0);
        me.sim.seconds(0.3);
        me.keys(0.0, 0.0);
        me.sim.seconds(1.0);
        assert!(me.feet().z.abs() < 0.05, "{gun}: feet at {} after walking", me.feet().z);
        assert!((camera(&me).z - eye).abs() < 0.05, "{gun}: eye {} above the floor, want {eye}", camera(&me).z);
    }
}
