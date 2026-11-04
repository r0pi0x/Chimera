//! The implementer test cases of specs/hl2dm/movement.md (H1), numbered
//! as in the spec ("row N"), on `Hl2Sim` with a small map in Source
//! units: a floor at z = 0 (concrete), the spec's test ladder (bottom
//! (0, 0, 8), top (0, 0, 264), a block behind it from x 17 whose top is a
//! ledge at 264, its dismount point D at (48, 0, 264)), and a deep pool
//! (water up to 200 over a floor at -200) at x -1500. dt 0.015, gravity
//! 600, the suit worn with 100 aux power.

use std::{collections::VecDeque, sync::Arc};


use bevy::prelude::*;
use chimera_core::core::{DamageRemainder, Health, Intent, MovementState, Velocity};
use chimera_sim::weapon::Armor;
use chimera_world::{
    harness::Sim,
    map::{MapBrush, MapBrushes, MapEntities, MapEntity, MapWater, MapWaterVolume, PlaySound, entities::FireEntityOutput},
};
use hl2dm::{
    game::{
        movement::{self, ID},
        suit::Suit,
    },
    harness::Hl2Sim,
};
use source_engine::source_movement::{SourceMovement, SourceSpeed, to_engine, to_source};

const UNIT: f32 = 0.0254;
const TOL: f32 = 0.01;

fn brush(lo: Vec3, hi: Vec3, surface: Option<&str>) -> MapBrush {
    let (a, b) = (to_engine(lo), to_engine(hi));
    MapBrush {
        surface: surface.map(String::from),
        ..MapBrush::from_box(a.min(b), a.max(b))
    }
}

/// The deep pool's centre.
const POOL: Vec3 = Vec3::new(-1500.0, 0.0, 0.0);
/// Where the ground tests run, away from the ladder.
const FIELD: Vec3 = Vec3::new(2000.0, -3000.0, 0.0);
/// The middle of the waist-deep water (40 deep over the floor).
const WADE: Vec3 = Vec3::new(5500.0, 0.0, 0.0);

fn entity(pairs: &[(&str, &str)]) -> MapEntity {
    MapEntity {
        keyvalues: pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        ..Default::default()
    }
}

#[derive(Resource, Clone, Copy)]
struct WithNode(bool);

fn test_map(mut commands: Commands, node: Res<WithNode>) {
    let brushes = vec![
        // The floor but the pool.
        brush(Vec3::new(-8192.0, -8192.0, -64.0), Vec3::new(-2000.0, 8192.0, 0.0), Some("concrete")),
        brush(Vec3::new(-1000.0, -8192.0, -64.0), Vec3::new(8192.0, 8192.0, 0.0), Some("concrete")),
        brush(Vec3::new(-2000.0, -8192.0, -264.0), Vec3::new(-1000.0, 8192.0, -200.0), Some("concrete")),
        // The block behind the ladder: its top is the ledge at 264.
        brush(Vec3::new(17.0, -200.0, 0.0), Vec3::new(400.0, 200.0, 264.0), Some("concrete")),
    ];
    commands.insert_resource(MapBrushes(brushes));
    commands.insert_resource(MapWater(vec![
        MapWaterVolume {
            brush: brush(Vec3::new(-2000.0, -8192.0, -200.0), Vec3::new(-1000.0, 8192.0, 200.0), None),
            slime: false,
        },
        // Waist-deep water over the floor.
        MapWaterVolume {
            brush: brush(Vec3::new(WADE.x - 1000.0, -8192.0, -64.0), Vec3::new(WADE.x + 1000.0, 8192.0, 40.0), None),
            slime: false,
        },
    ]));
    let mut entities = vec![entity(&[
        ("classname", "func_useableladder"),
        ("targetname", "ladder"),
        ("point0", "0 0 264"),
        ("point1", "0 0 8"),
    ])];
    if node.0 {
        entities.push(entity(&[("classname", "info_ladder_dismount"), ("origin", "48 0 264")]));
    }
    commands.insert_resource(MapEntities {
        entities: Arc::new(entities),
        scale: UNIT,
    });
}

struct TestMap(bool);
impl Plugin for TestMap {
    fn build(&self, app: &mut App) {
        app.insert_resource(WithNode(self.0)).add_systems(Startup, test_map);
    }
}

/// Sounds and outputs as they happen, with the tick.
#[derive(Resource, Default)]
struct Heard {
    tick: u64,
    sounds: VecDeque<(u64, String, Option<f32>)>,
    outputs: Vec<(u64, String)>,
}

fn listen(mut heard: ResMut<Heard>, mut sounds: MessageReader<PlaySound>, mut outputs: MessageReader<FireEntityOutput>) {
    let tick = heard.tick;
    for s in sounds.read() {
        heard.sounds.push_back((tick, s.entry.clone(), s.volume));
    }
    for o in outputs.read() {
        heard.outputs.push((tick, o.output.clone()));
    }
}

fn count(mut heard: ResMut<Heard>) {
    heard.tick += 1;
}

struct Player {
    sim: Sim,
    p: Entity,
}

impl Player {
    /// A player whose feet start at `feet` (Source units), not stepped.
    fn at(feet: Vec3) -> Self {
        Self::at_with(feet, true)
    }

    fn at_with(feet: Vec3, node: bool) -> Self {
        let mut sim = Sim::new(Hl2Sim, TestMap(node));
        // One fixed tick per update, at HL2:DM's 0.015 s.
        sim.set_tick_interval(0.015);
        sim.app
            .init_resource::<Heard>()
            .add_systems(FixedFirst, count)
            .add_systems(FixedLast, listen);
        let p = sim.spawn_character(to_engine(feet + Vec3::Z * 36.0), ID);
        let mut me = Self { sim, p };
        me.look(0.0, 0.0);
        // One tick to put the suit on and find the ground.
        me.sim.ticks(1);
        me.place(feet);
        me
    }

    /// Standing on the field's floor, settled, nothing held.
    fn on_floor() -> Self {
        let mut me = Self::at(FIELD + Vec3::Z * 0.5);
        me.sim.ticks(4);
        assert!(me.ms().on_ground, "should stand on the floor");
        me.clear_heard();
        me
    }

    fn place(&mut self, feet: Vec3) {
        let at = to_engine(feet + Vec3::Z * 36.0);
        let w = self.sim.app.world_mut();
        w.get_mut::<Transform>(self.p).unwrap().translation = at;
        if let Some(mut pos) = w.get_mut::<avian3d::prelude::Position>(self.p) {
            pos.0 = at;
        }
        w.get_mut::<Velocity>(self.p).unwrap().0 = Vec3::ZERO;
    }

    /// Face Source yaw and pitch (degrees; pitch positive looks down).
    fn look(&mut self, yaw: f32, pitch: f32) {
        let mut i = self.sim.intent(self.p);
        i.yaw = yaw.to_radians() - std::f32::consts::FRAC_PI_2;
        i.pitch = -pitch.to_radians();
    }

    fn intent(&mut self) -> Mut<'_, Intent> {
        self.sim.intent(self.p)
    }

    /// Forward/back and right/left keys.
    fn keys(&mut self, forward: f32, right: f32) {
        self.intent().move_axis = Vec2::new(right, forward);
    }

    /// Hold `+speed` (Chimera's shared `+speed` is walk and sprint).
    fn speed_key(&mut self, held: bool) {
        let mut i = self.intent();
        i.sprint = held;
        i.walk = held;
    }

    /// Hold `+walk` alone.
    fn walk_key(&mut self, held: bool) {
        self.intent().walk = held;
    }

    /// The `+walk` key's own action (its bit in `Intent::actions`).
    fn walk_action(&mut self, held: bool) {
        let bit = self.sim.app.world().resource::<chimera_core::core::ActionRegistry>().bit(movement::WALK_ACTION);
        assert_ne!(bit, 0, "the walk action is registered");
        let mut i = self.intent();
        if held {
            i.actions |= bit;
        } else {
            i.actions &= !bit;
        }
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

    fn me_mut(&mut self) -> Mut<'_, SourceMovement> {
        self.sim.app.world_mut().get_mut::<SourceMovement>(self.p).unwrap()
    }

    fn ms(&self) -> MovementState {
        self.sim.app.world().get::<MovementState>(self.p).unwrap().clone()
    }

    fn suit(&self) -> Suit {
        self.sim.app.world().get::<Suit>(self.p).unwrap().clone()
    }

    fn suit_mut(&mut self) -> Mut<'_, Suit> {
        self.sim.app.world_mut().get_mut::<Suit>(self.p).unwrap()
    }

    /// This tick's max speed (P).
    fn max(&self) -> f32 {
        self.sim.app.world().get::<SourceSpeed>(self.p).unwrap().0
    }

    fn tick(&mut self) {
        self.sim.ticks(1);
    }

    fn clear_heard(&mut self) {
        let mut h = self.sim.app.world_mut().resource_mut::<Heard>();
        h.sounds.clear();
        h.outputs.clear();
    }

    /// The sounds heard since the last clear: (entry, volume).
    fn heard(&self) -> Vec<(String, Option<f32>)> {
        self.sim.app.world().resource::<Heard>().sounds.iter().map(|(_, e, v)| (e.clone(), *v)).collect()
    }

    fn heard_ticks(&self, entry: &str) -> Vec<u64> {
        self.sim
            .app
            .world()
            .resource::<Heard>()
            .sounds
            .iter()
            .filter(|(_, e, _)| e == entry)
            .map(|(t, ..)| *t)
            .collect()
    }

    fn outputs(&self) -> Vec<String> {
        self.sim.app.world().resource::<Heard>().outputs.iter().map(|(_, o)| o.clone()).collect()
    }

    fn health(&self) -> f32 {
        self.sim.app.world().get::<Health>(self.p).unwrap().current * 100.0
    }
}

fn close(actual: f32, expected: f32, what: &str) {
    assert!((actual - expected).abs() <= TOL, "{what}: got {actual}, expected {expected}");
}

/// Rows 1-3: running, walking and sprinting from rest.
#[test]
fn ground_acceleration_by_walk_run_and_sprint() {
    let cases: [(&str, [f32; 9]); 3] = [
        ("run", [28.5, 51.0, 73.5, 96.0, 118.5, 139.89, 160.0, 178.9, 190.0]),
        ("walk", [22.5, 39.0, 55.5, 72.0, 88.5, 105.0, 121.2, 136.43, 150.0]),
        ("sprint", [48.0, 90.0, 132.0, 172.08, 209.76, 245.17, 278.46, 309.75, 320.0]),
    ];
    for (how, speeds) in cases {
        let mut pl = Player::on_floor();
        pl.keys(1.0, 0.0);
        match how {
            "walk" => pl.walk_key(true),
            "sprint" => pl.speed_key(true),
            _ => {}
        }
        for (k, s) in speeds.iter().enumerate() {
            pl.tick();
            close(pl.speed(), *s, &format!("{how} tick {}", k + 1));
        }
        pl.sim.ticks(5);
        close(pl.speed(), speeds[8], &format!("{how} stays"));
    }
}

/// Row 4: stopping from 190.
#[test]
fn friction_from_190() {
    let mut pl = Player::on_floor();
    pl.set_vel(Vec3::X * 190.0);
    let mut got = Vec::new();
    for _ in 0..27 {
        pl.tick();
        got.push(pl.speed());
    }
    for (tick, s) in [(1, 178.6), (10, 102.34), (11, 96.2), (12, 90.2), (26, 6.2), (27, 0.0)] {
        close(got[tick - 1], s, &format!("tick {tick}"));
    }
}

fn duck_fully(pl: &mut Player) {
    pl.intent().crouch = true;
    pl.sim.ticks(30);
    assert!(pl.me().ducked && !pl.me().ducking, "fully ducked");
}

/// Rows 5-7: ducked ground speed; a sprint started before the duck
/// carries on; `+walk` with duck held doesn't walk.
#[test]
fn ducked_speed_and_the_sprint_that_carries_on() {
    let mut pl = Player::on_floor();
    duck_fully(&mut pl);
    pl.keys(1.0, 0.0);
    let mut got = Vec::new();
    for _ in 0..17 {
        pl.tick();
        got.push(pl.speed());
    }
    for (tick, s) in [(1, 9.5), (2, 13.0), (3, 16.5), (16, 62.0), (17, 63.333)] {
        close(got[tick - 1], s, &format!("row 5 tick {tick}"));
    }

    // Row 6: +speed pressed standing still, then the duck, held throughout.
    let mut pl = Player::on_floor();
    pl.speed_key(true);
    pl.tick();
    assert!(pl.suit().sprinting);
    duck_fully(&mut pl);
    assert!(pl.suit().sprinting, "the sprint carries on ducked");
    pl.keys(1.0, 0.0);
    let mut got = Vec::new();
    for _ in 0..11 {
        pl.tick();
        got.push(pl.speed());
    }
    for (tick, s) in [(1, 16.0), (10, 106.0), (11, 106.667)] {
        close(got[tick - 1], s, &format!("row 6 tick {tick}"));
    }

    // Row 7: +walk and +duck held: running's 190, scaled.
    let mut pl = Player::on_floor();
    pl.walk_key(true);
    duck_fully(&mut pl);
    assert_eq!(pl.max(), 190.0);
    pl.keys(1.0, 0.0);
    pl.tick();
    close(pl.speed(), 9.5, "row 7 tick 1");
}

/// Rows 8-10: air acceleration.
#[test]
fn air_acceleration() {
    let mut pl = Player::at(FIELD + Vec3::Z * 500.0);
    pl.keys(1.0, 0.0);
    pl.tick();
    close(pl.speed(), 28.5, "row 8 tick 1");
    pl.tick();
    close(pl.speed(), 30.0, "row 8 tick 2");

    for (sprint, expected) in [(false, 192.126), (true, 192.354)] {
        let mut pl = Player::at(FIELD + Vec3::Z * 500.0);
        if sprint {
            pl.speed_key(true);
        }
        pl.set_vel(Vec3::X * 190.0);
        pl.keys(0.0, 1.0);
        pl.tick();
        close(pl.speed(), expected, &format!("air strafe sprint {sprint}"));
    }
}

/// Rows 11-13: the standing and ducked jumps.
#[test]
fn the_fixed_jump() {
    let mut pl = Player::on_floor();
    let z0 = pl.feet().z;
    pl.intent().jump = true;
    pl.tick();
    close(pl.feet().z - z0, 2.265, "row 11 z");
    close(pl.vel().z, 146.5, "row 11 vz");
    let mut zs = vec![pl.feet().z - z0];
    let mut grounded = None;
    for k in 2..=40 {
        pl.tick();
        zs.push(pl.feet().z - z0);
        if grounded.is_none() && pl.ms().on_ground {
            grounded = Some(k);
        }
    }
    close(zs[16], 20.145, "apex at the end of tick 17");
    assert!(zs[17] < zs[16]);
    assert_eq!(grounded, Some(34), "grounded at the end of tick 34");
    close(zs[33], 1.275, "feet 1.275 up");
    close(pl.me().last_landing_speed, 141.5, "fall speed");

    // Row 13: the ducked flag set: vz replaced.
    let mut pl = Player::on_floor();
    duck_fully(&mut pl);
    let z0 = pl.feet().z;
    pl.intent().jump = true;
    pl.tick();
    close(pl.vel().z, 151.0, "row 13 end of tick");
    let mut zs = vec![pl.feet().z - z0];
    let mut grounded = None;
    for k in 2..=40 {
        pl.tick();
        zs.push(pl.feet().z - z0);
        if grounded.is_none() && pl.ms().on_ground {
            grounded = Some(k);
        }
    }
    close(zs[17], 21.33, "row 13 apex at tick 18");
    assert_eq!(grounded, Some(35));
}

/// Rows 14-15: ducking in the air lifts the feet 36.
#[test]
fn air_duck_lifts_the_feet_36() {
    let mut pl = Player::on_floor();
    let z0 = pl.feet().z;
    pl.intent().jump = true;
    pl.sim.ticks(9);
    let before = pl.feet().z - z0;
    pl.intent().crouch = true;
    pl.tick();
    let me = pl.me();
    assert!(me.ducked);
    close(me.eye, 28.0, "row 14 eye");
    close(pl.ms().hull_max.y / UNIT + 36.0, 36.0, "box top 36");
    // Feet +36 before the move, then the move of tick 10 (151 - 81).
    close(pl.feet().z - z0, before + 36.0 + 70.0 * 0.015, "row 14 feet");
    let mut peak: f32 = 0.0;
    for _ in 0..20 {
        pl.tick();
        peak = peak.max(pl.feet().z - z0);
    }
    close(peak, 56.145, "row 14 peak");

    // Row 15: duck and jump on the same grounded tick.
    let mut pl = Player::on_floor();
    let z0 = pl.feet().z;
    pl.intent().jump = true;
    pl.intent().crouch = true;
    pl.tick();
    close(pl.vel().z, 151.0, "row 15: the jump replaces vz");
    let mut peak: f32 = 0.0;
    for _ in 0..30 {
        pl.tick();
        peak = peak.max(pl.feet().z - z0);
    }
    close(peak, 57.33, "row 15 peak");
}

/// Rows 16-19: no landing slowdown, no cap, no boost, no back-hop.
#[test]
fn hops_keep_their_speed() {
    let mut pl = Player::on_floor();
    pl.set_vel(Vec3::X * 190.0);
    pl.keys(1.0, 0.0);
    pl.intent().jump = true;
    pl.tick();
    pl.intent().jump = false;
    while !pl.ms().on_ground {
        pl.tick();
    }
    close(pl.speed(), 190.0, "row 16 landing");
    pl.tick();
    close(pl.speed(), 190.0, "row 16 the tick after");

    for (v, yaw) in [(600.0, 0.0), (400.0, 180.0)] {
        let mut pl = Player::on_floor();
        pl.look(yaw, 0.0);
        pl.set_vel(Vec3::X * v);
        pl.intent().jump = true;
        pl.tick();
        close(pl.speed(), v, &format!("rows 17-18 at {v}"));
    }
}

/// Row 20: one ideally strafed running hop.
#[test]
fn one_strafed_hop() {
    let mut pl = Player::on_floor();
    pl.set_vel(Vec3::X * 190.0);
    pl.intent().jump = true;
    // Perpendicular wish every tick: look along the velocity, press right.
    pl.keys(0.0, 1.0);
    pl.tick();
    pl.intent().jump = false;
    for _ in 0..40 {
        if pl.ms().on_ground {
            break;
        }
        let v = pl.vel();
        let yaw = v.y.atan2(v.x).to_degrees();
        pl.look(yaw, 0.0);
        pl.keys(0.0, 1.0);
        pl.tick();
    }
    close(pl.speed(), (190.0f32 * 190.0 + 16_194.23).sqrt(), "row 20 landing speed");
}

/// Rows 21-22: the duck and unduck curves.
#[test]
fn duck_and_unduck_times() {
    let mut pl = Player::on_floor();
    pl.intent().crouch = true;
    let mut eyes = Vec::new();
    for _ in 0..=27 {
        pl.tick();
        eyes.push((pl.me().eye, pl.me().ducked));
    }
    close(eyes[13].0, 46.675, "row 21 tick 13");
    close(eyes[26].0, 28.066, "row 21 tick 26");
    assert!(!eyes[26].1, "row 21: standing box at tick 26");
    assert!(eyes[27].1 && (eyes[27].0 - 28.0).abs() < TOL, "row 21: ducked box at tick 27");

    pl.intent().crouch = false;
    let mut eyes = Vec::new();
    for _ in 0..=14 {
        pl.tick();
        eyes.push((pl.me().eye, pl.me().ducked));
    }
    close(eyes[7].0, 47.349, "row 22 tick 7");
    close(eyes[13].0, 63.934, "row 22 tick 13");
    assert!(eyes[13].1, "row 22: still the ducked box at tick 13");
    assert!(!eyes[14].1 && (eyes[14].0 - 64.0).abs() < TOL, "row 22: standing at tick 14");
}

/// Rows 23-24: the sprint's aux power, from rest and standing still.
#[test]
fn sprint_drains_aux_only_while_moving() {
    let mut pl = Player::on_floor();
    pl.keys(1.0, 0.0);
    pl.speed_key(true);
    pl.tick();
    assert_eq!(pl.max(), 320.0);
    close(pl.suit().aux, 100.0, "row 23 tick 1");
    for n in 2..=10 {
        pl.tick();
        close(pl.suit().aux, 100.0 - 0.375 * (n as f32 - 1.0), &format!("row 23 tick {n}"));
    }
    assert_eq!(pl.heard_ticks("HL2Player.SprintStart").len(), 1, "SprintStart once");

    let mut pl = Player::on_floor();
    pl.speed_key(true);
    pl.sim.ticks(67);
    assert!(pl.suit().sprinting);
    close(pl.suit().aux, 100.0, "row 24");
}

/// Rows 25-26: the stop's cost, the recharge delay, running dry.
#[test]
fn sprint_stop_costs_and_running_dry() {
    let mut pl = Player::on_floor();
    pl.keys(1.0, 0.0);
    pl.speed_key(true);
    pl.sim.ticks(10);
    pl.suit_mut().aux = 50.0;
    pl.speed_key(false);
    pl.tick();
    close(pl.suit().aux, 47.5, "row 25 the release");
    pl.sim.ticks(33);
    close(pl.suit().aux, 47.5, "33 ticks later");
    pl.tick();
    close(pl.suit().aux, 47.6875, "34 ticks later");

    let mut pl = Player::on_floor();
    pl.keys(1.0, 0.0);
    pl.speed_key(true);
    let mut aux = Vec::new();
    for _ in 0..269 {
        pl.tick();
        aux.push((pl.suit().aux, pl.max()));
    }
    assert!((aux[266].0 - 0.25).abs() < 0.02, "row 26 tick 267: {}", aux[266].0);
    assert_eq!(aux[267].0, 0.0, "tick 268");
    assert_eq!(aux[268].1, 190.0, "tick 269: sprint off");
}

/// Rows 27-29: no power, ducked, and sprint over walk.
#[test]
fn when_a_sprint_starts() {
    let mut pl = Player::on_floor();
    pl.suit_mut().aux = 9.9;
    pl.keys(1.0, 0.0);
    pl.speed_key(true);
    pl.tick();
    assert_eq!(pl.max(), 190.0);
    assert_eq!(pl.heard_ticks("HL2Player.SprintNoPower").len(), 1);
    // Running doesn't drain; the recharge passes 10 while held.
    pl.sim.ticks(40);
    assert!(pl.suit().aux > 10.0, "{}", pl.suit().aux);
    assert_eq!(pl.max(), 190.0, "row 27: still no sprint");

    let mut pl = Player::on_floor();
    duck_fully(&mut pl);
    pl.clear_heard();
    pl.speed_key(true);
    pl.tick();
    assert!(!pl.suit().sprinting && pl.heard().is_empty(), "row 28: no sprint, no sound");
    pl.intent().crouch = false;
    pl.sim.ticks(20);
    assert!(!pl.suit().sprinting, "row 28: standing up while holding");

    let mut pl = Player::on_floor();
    pl.walk_key(true);
    pl.tick();
    assert_eq!(pl.max(), 150.0);
    pl.speed_key(true);
    pl.tick();
    assert_eq!(pl.max(), 320.0, "row 29: sprint beats walk");
}

/// The test ladder's player on the floor at `feet`, nothing held.
fn near_ladder(feet: Vec3, node: bool) -> Player {
    let mut pl = Player::at_with(feet, node);
    pl.sim.ticks(3);
    pl.clear_heard();
    pl
}

/// Rows 30-32, 44: the cone mount, the forced move, the climb and the
/// automatic dismount at the top.
#[test]
fn mount_climb_and_dismount() {
    let mut pl = near_ladder(Vec3::new(-20.0, 0.0, 0.0), true);
    let start = pl.feet();
    pl.keys(1.0, 0.0);
    pl.tick();
    assert!(pl.me().forced.is_some(), "row 30: a forced move starts");
    assert_eq!(pl.outputs(), ["OnPlayerGotOnLadder"]);
    assert!(pl.heard().iter().any(|(e, _)| e == "Ladder.StepRight"));
    let goal = Vec3::new(0.0, 0.0, 8.0);
    let t = (goal - start).length() / 190.0;
    for k in 1..=7 {
        pl.tick();
        let f = (k as f32 * 0.015 / t).min(1.0);
        let at = start + (goal - start) * f;
        assert!((pl.feet() - at).length() < 0.02, "row 31 tick {}: {} vs {at}", k + 1, pl.feet());
    }
    pl.tick();
    assert!(pl.me().useable.is_some(), "row 31: on the ladder at tick 9");
    close(pl.feet().z, 11.0, "row 31: climbs at once");
    // Row 44: up to the top, then off onto D's spot.
    pl.sim.ticks(84);
    close(pl.feet().z, 263.0, "row 44: z 263");
    assert!(pl.me().forced.is_some(), "row 44: the dismount starts");
    pl.sim.ticks(17);
    assert!(pl.me().forced.is_none() && pl.me().useable.is_none());
    let f = pl.feet();
    assert!((f - Vec3::new(48.0, 0.0, 265.0)).length() < 0.05, "row 44: on D's spot {f}");
    close(pl.vel().length(), 0.0, "row 44: still");

    // Row 32: sprinting, it arrives 5 ticks after the mount tick.
    let mut pl = near_ladder(Vec3::new(-20.0, 0.0, 0.0), true);
    pl.speed_key(true);
    pl.keys(1.0, 0.0);
    pl.tick();
    pl.sim.ticks(4);
    assert!(pl.me().forced.is_some());
    pl.tick();
    assert!(pl.me().useable.is_some(), "row 32: on at the 5th tick after the mount");
}

/// Rows 33-37: when a mount happens.
#[test]
fn mount_conditions() {
    // Row 33: strafing, not looking along: no mount.
    let mut pl = near_ladder(Vec3::new(-20.0, 0.0, 0.0), true);
    pl.keys(1.0, 1.0);
    pl.tick();
    assert!(pl.me().forced.is_none(), "row 33");
    // Row 34: looking 30 up: mounts.
    let mut pl = near_ladder(Vec3::new(-20.0, 0.0, 0.0), true);
    pl.look(0.0, -30.0);
    pl.keys(1.0, 1.0);
    pl.tick();
    assert!(pl.me().forced.is_some(), "row 34");
    // Row 35: the end-point mount.
    let mut pl = near_ladder(Vec3::new(-10.0, 0.0, 0.0), true);
    pl.look(90.0, -30.0);
    pl.keys(1.0, 0.0);
    pl.tick();
    assert!(pl.me().forced.is_some(), "row 35");
    // Row 36: +use from 50 units; arrives 18 ticks later; facing away no.
    for (yaw, useonly, mounts) in [(0.0, false, true), (180.0, false, false), (170.0, false, false), (170.0, true, true)] {
        let mut pl = near_ladder(Vec3::new(-50.0, 0.0, 0.0), true);
        movement::config_mut(pl.sim.app.world_mut()).ladder_use_only = useonly;
        pl.look(yaw, 0.0);
        pl.intent().use_key = true;
        pl.tick();
        assert_eq!(pl.me().forced.is_some(), mounts, "row 36 yaw {yaw} useonly {useonly}");
        if yaw == 0.0 {
            pl.intent().use_key = false;
            pl.sim.ticks(17);
            assert!(pl.me().forced.is_some());
            pl.tick();
            assert!(pl.me().useable.is_some(), "row 36: arrives 18 ticks later");
        }
    }
    // Row 37: use only: forward doesn't mount.
    let mut pl = near_ladder(Vec3::new(-20.0, 0.0, 0.0), true);
    movement::config_mut(pl.sim.app.world_mut()).ladder_use_only = true;
    pl.keys(1.0, 0.0);
    pl.sim.ticks(3);
    assert!(pl.me().forced.is_none(), "row 37");
}

/// On the ladder at `feet` (attached, nothing held).
fn on_ladder(feet: Vec3, node: bool) -> Player {
    let mut pl = Player::at_with(feet, node);
    pl.place(feet);
    pl.me_mut().useable = Some(0);
    pl.clear_heard();
    pl
}

/// Rows 38-40: which way the keys climb.
#[test]
fn climb_directions() {
    let cases: &[(&str, f32, f32, f32, f32, f32)] = &[
        // (what, forward, right, yaw, pitch, z after one tick)
        ("forward 0", 1.0, 0.0, 0.0, 0.0, 131.0),
        ("forward 17 down", 1.0, 0.0, 0.0, 17.0, 131.0),
        ("forward 18 down", 1.0, 0.0, 0.0, 18.0, 125.0),
        ("forward 89 up", 1.0, 0.0, 0.0, -89.0, 131.0),
        ("back 0", -1.0, 0.0, 0.0, 0.0, 131.0),
        ("back 17 up", -1.0, 0.0, 0.0, -17.0, 131.0),
        ("back 18 up", -1.0, 0.0, 0.0, -18.0, 125.0),
        ("right", 0.0, 1.0, 0.0, 0.0, 131.0),
        ("forward right 20 down", 1.0, 1.0, 0.0, 20.0, 131.0),
        ("forward 20 down", 1.0, 0.0, 0.0, 20.0, 125.0),
        ("none", 0.0, 0.0, 0.0, 0.0, 128.0),
    ];
    for (what, f, r, yaw, pitch, z) in cases {
        let mut pl = on_ladder(Vec3::new(0.0, 0.0, 128.0), true);
        pl.look(*yaw, *pitch);
        pl.keys(*f, *r);
        pl.tick();
        close(pl.feet().z, *z, what);
    }
}

/// Rows 41-43: jumping off and leaving by `+use`.
#[test]
fn jump_off_and_use_leave() {
    let mut pl = on_ladder(Vec3::new(0.0, 0.0, 128.0), true);
    pl.look(180.0, 0.0);
    pl.intent().jump = true;
    pl.tick();
    close(pl.feet().x, -3.0, "row 41 x");
    close(pl.feet().z, 128.6825, "row 41 z");
    let v = pl.vel();
    assert!((v - Vec3::new(-200.0, 0.0, 41.0)).length() < TOL, "row 41 v {v}");
    assert_eq!(pl.outputs(), ["OnPlayerGotOffLadder"]);

    let mut pl = on_ladder(Vec3::new(0.0, 0.0, 128.0), true);
    pl.look(0.0, 30.0);
    pl.keys(-1.0, 0.0);
    pl.intent().jump = true;
    pl.tick();
    close(pl.vel().z, 91.0, "row 42 end vz");
    close(pl.vel().x, -173.205, "row 42 vx");

    let mut pl = on_ladder(Vec3::new(0.0, 0.0, 128.0), false);
    pl.look(180.0, 0.0);
    pl.intent().use_key = true;
    pl.tick();
    assert!(pl.me().useable.is_none(), "row 43: off");
    let v = pl.vel();
    assert!((v - Vec3::new(-100.0, 0.0, 50.0)).length() < TOL, "row 43 v {v}");
    close(pl.feet().z, 128.0, "row 43: no move that tick");
    pl.intent().use_key = false;
    pl.tick();
    close(pl.feet().x, -1.5, "row 43 next x");
    close(pl.feet().z, 128.6825, "row 43 next z");
    close(pl.vel().z, 41.0, "row 43 next vz");
}

/// Row 45: no node at the top: stops; looking along: no dismount.
#[test]
fn the_top_without_a_node() {
    let mut pl = on_ladder(Vec3::new(0.0, 0.0, 257.0), false);
    pl.keys(1.0, 0.0);
    pl.sim.ticks(2);
    close(pl.feet().z, 263.0, "row 45: 263");
    pl.sim.ticks(3);
    close(pl.feet().z, 263.0, "row 45: put back, stays");
    assert!(pl.me().useable.is_some());

    let mut pl = on_ladder(Vec3::new(0.0, 0.0, 257.0), true);
    pl.look(0.0, -70.0);
    pl.keys(1.0, 0.0);
    pl.sim.ticks(5);
    close(pl.feet().z, 263.0, "row 45 along: stays at 263");
    assert!(pl.me().forced.is_none());
}

/// Rows 46-47: falling off and being pushed off.
#[test]
fn falling_off() {
    let mut pl = on_ladder(Vec3::new(0.0, 7.0, 128.0), true);
    pl.tick();
    assert!(pl.me().useable.is_some(), "row 46: no keys: stays");
    pl.keys(1.0, 0.0);
    pl.tick();
    assert!(pl.me().useable.is_none(), "row 46: forward: off");
    assert!(pl.feet().z < 128.0, "falls this tick");

    let mut pl = on_ladder(Vec3::new(0.0, 0.0, 128.0), true);
    pl.sim.app.world_mut().get_mut::<chimera_core::core::BaseVelocity>(pl.p).unwrap().velocity = to_engine(Vec3::Y * 5.0);
    pl.sim.app.world_mut().get_mut::<chimera_core::core::BaseVelocity>(pl.p).unwrap().touched = true;
    pl.tick();
    assert!(pl.me().useable.is_none(), "row 47");
}

/// Rows 49-53: falls from ledges; fixed and progressive damage.
#[test]
fn fall_damage() {
    let fall = |h: f32, progressive: bool| {
        let mut pl = Player::at(FIELD + Vec3::Z * h);
        if progressive {
            movement::config_mut(pl.sim.app.world_mut()).fall_fixed = None;
        }
        pl.sim.app.world_mut().get_mut::<Armor>(pl.p).unwrap().amount = 0.5;
        let mut tick = 0;
        while !pl.ms().on_ground {
            pl.tick();
            tick += 1;
            assert!(tick < 200);
        }
        let landing = pl.me().last_landing_speed;
        let roll = pl.me().punch_roll;
        let heard = pl.heard();
        pl.sim.ticks(2);
        (tick, landing, roll, pl.health(), heard, pl.sim.app.world().get::<Armor>(pl.p).unwrap().amount, pl)
    };
    let step = |heard: &[(String, Option<f32>)]| {
        heard.iter().find(|(e, _)| e.contains("RunFootstep")).and_then(|(_, v)| *v)
    };
    let (tick, speed, roll, health, heard, ..) = fall(128.0, false);
    assert_eq!(tick, 44, "row 49");
    close(speed, 387.0, "row 49 fall");
    close(roll, 5.031, "row 49 roll");
    close(health, 100.0, "row 49 health");
    assert_eq!(step(&heard), Some(0.85));

    let (tick, speed, roll, _, heard, ..) = fall(64.0, false);
    assert_eq!((tick, speed.round()), (31, 270.0), "row 50");
    assert!(roll == 0.0 && step(&heard).is_none(), "row 50: no effect");

    let (tick, speed, _, health, heard, ..) = fall(236.9, false);
    assert_eq!((tick, speed.round(), health), (59, 522.0, 100.0), "row 51 low");
    assert_eq!(step(&heard), Some(0.85));
    let (tick, speed, roll, health, heard, ..) = fall(245.0, false);
    assert_eq!((tick, speed.round(), health), (60, 531.0, 90.0), "row 51 high");
    close(roll, 6.903, "row 51 roll");
    assert_eq!(step(&heard), Some(1.0));

    let (tick, speed, roll, health, _, armour, _) = fall(512.0, false);
    assert_eq!((tick, speed.round(), health), (87, 774.0, 90.0), "row 52 fixed");
    close(roll, 10.062, "row 52 fixed roll");
    close(armour, 0.5, "armour untouched");
    let (_, _, roll, health, _, armour, pl) = fall(512.0, true);
    close(health, 38.0, "row 52 progressive: 62 of 62.5");
    let carried = pl.sim.app.world().get::<DamageRemainder>(pl.p).unwrap().0;
    close(carried, 0.5, "row 52: 0.5 carried");
    close(roll, 3.2175, "row 52 progressive roll");
    close(armour, 0.5, "armour untouched");

    let (tick, speed, _, health, ..) = fall(719.0, true);
    assert_eq!((tick, speed.round(), health), (104, 927.0, 0.0), "row 53: dead");
}

/// Rows 55-58: the model's footsteps.
#[test]
fn model_footsteps() {
    let mut pl = Player::on_floor();
    pl.keys(1.0, 0.0);
    pl.sim.ticks(70);
    let h = pl.sim.app.world().resource::<Heard>();
    let steps: Vec<(u64, String, Option<f32>)> =
        h.sounds.iter().filter(|(_, e, _)| e.contains("RunFootstep")).cloned().collect();
    let start = h.tick - 70;
    let ticks: Vec<u64> = steps.iter().map(|(t, ..)| t - start).collect();
    assert_eq!(&ticks[..3], &[5, 32, 59], "row 55 timing");
    assert_eq!(steps[0].1, "NPC_Citizen.RunFootstepLeft");
    assert_eq!(steps[1].1, "NPC_Citizen.RunFootstepRight");
    assert_eq!(steps[0].2, Some(0.2));

    // Row 56: sprinting steps every 20 ticks at 0.5.
    let mut pl = Player::on_floor();
    pl.keys(1.0, 0.0);
    pl.speed_key(true);
    pl.sim.ticks(40);
    pl.clear_heard();
    pl.sim.ticks(60);
    let vols: Vec<Option<f32>> = pl.heard().into_iter().filter(|(e, _)| e.contains("RunFootstep")).map(|(_, v)| v).collect();
    assert_eq!(vols, [Some(0.5); 3], "row 56");

    // Row 57: climbing steps every 30 ticks at 0.5.
    let mut pl = on_ladder(Vec3::new(0.0, 0.0, 20.0), true);
    pl.keys(1.0, 0.0);
    pl.sim.ticks(61);
    let vols: Vec<Option<f32>> = pl.heard().into_iter().filter(|(e, _)| e.contains("RunFootstep")).map(|(_, v)| v).collect();
    assert_eq!(vols.len(), 2, "row 57: {vols:?}");
    assert!(vols.iter().all(|v| *v == Some(0.5)));

    // Row 58: ducked: never.
    let mut pl = Player::on_floor();
    duck_fully(&mut pl);
    pl.clear_heard();
    pl.keys(1.0, 0.0);
    pl.sim.ticks(80);
    pl.intent().jump = true;
    pl.sim.ticks(60);
    assert!(!pl.heard().iter().any(|(e, _)| e.contains("RunFootstep")), "row 58");
}

/// Rows 59-61: swimming.
#[test]
fn swimming() {
    let under = POOL + Vec3::Z * 20.0;
    let mut pl = Player::at(under);
    pl.tick();
    assert_eq!(pl.me().water_level, 3);
    pl.place(under);
    pl.intent().jump = true;
    pl.tick();
    close(pl.vel().z, 116.8, "row 59 tick 1");
    pl.tick();
    close(pl.vel().z, 104.2, "row 59 tick 2");

    let mut pl = Player::at(under);
    pl.tick();
    pl.place(under);
    pl.keys(1.0, 0.0);
    let mut s = Vec::new();
    for _ in 0..9 {
        pl.tick();
        s.push(pl.vel().length());
    }
    for (tick, v) in [(1, 22.8), (2, 44.232), (3, 64.378), (9, 152.0)] {
        close(s[tick - 1], v, &format!("row 60 tick {tick}"));
    }

    let mut pl = Player::at(under);
    pl.tick();
    pl.place(under);
    let mut vz = Vec::new();
    for _ in 0..9 {
        pl.tick();
        vz.push(pl.vel().z);
    }
    for (tick, v) in [(1, -7.2), (2, -13.968), (9, -48.0)] {
        close(vz[tick - 1], v, &format!("row 61 tick {tick}"));
    }
}

/// The cvars HL2:DM shares with CS:S are its own (`hl2dm:`; lift C8).
#[test]
fn its_cvars_are_its_own() {
    let mut pl = Player::on_floor();
    chimera_core::console::execute(pl.sim.app.world_mut(), &["hl2dm:sv_gravity".into(), "300".into()], 0);
    let w = pl.sim.app.world();
    let cfg = w.resource::<source_engine::source_movement::SourceMovementConfigs>().get(ID).unwrap().gravity;
    assert_eq!(cfg, 300.0);
    assert_eq!(w.resource::<source_engine::source_movement::SourceMovementConfig>().gravity, 800.0, "CS:S's untouched");
}

/// Speed selection 4 (movement.md): ALT (`+walk`) and SHIFT (`+speed`)
/// with no aux power walk at 150: the walk key is its own, not Chimera's
/// shared walk that `+speed` sets too; SHIFT alone with no power runs.
#[test]
fn the_walk_key_with_shift_and_no_aux_power_walks() {
    let mut pl = Player::on_floor();
    pl.suit_mut().aux = 0.0;
    pl.keys(1.0, 0.0);
    pl.speed_key(true);
    pl.tick();
    assert_eq!(pl.max(), 190.0, "+speed alone, no power: runs");
    pl.walk_action(true);
    pl.tick();
    assert_eq!(pl.max(), 150.0, "+walk and +speed, no power: walks");
    assert!(!pl.suit().sprinting);
    pl.sim.ticks(40);
    close(pl.speed(), 150.0, "walking speed");
    // With power, the sprint still beats the walk.
    let mut pl = Player::on_floor();
    pl.keys(1.0, 0.0);
    pl.speed_key(true);
    pl.walk_action(true);
    pl.tick();
    assert_eq!(pl.max(), 320.0, "sprint beats walk");
}

/// Speed selection 5 and Jumping: the ground material's max speed and
/// jump factors (surface properties' maxspeedfactor, jumpfactor).
#[test]
fn the_ground_material_scales_speed_and_jump() {
    let mut pl = Player::on_floor();
    let mut sounds = chimera_world::map::MapSounds::default();
    sounds.surfaces.insert(
        "concrete".into(),
        chimera_world::map::sound::MapSurface {
            max_speed_factor: Some(0.5),
            jump_factor: Some(1.25),
            ..Default::default()
        },
    );
    pl.sim.app.world_mut().insert_resource(chimera_world::map::sound::SoundBank(Arc::new(sounds)));
    pl.keys(1.0, 0.0);
    pl.sim.ticks(60);
    close(pl.speed(), 95.0, "190 x 0.5");
    pl.keys(0.0, 0.0);
    pl.sim.ticks(60);
    pl.intent().jump = true;
    pl.tick();
    // 160 x 1.25 = 200, less the tick's gravity (row 11: 160 ends the
    // tick at 146.5).
    close(pl.vel().z, 200.0 - 13.5, "jump impulse x 1.25");
}

/// A brush entity under the field to land on: its velocity (Source
/// units/s) and whether it floats.
fn platform(pl: &mut Player, top: f32, vz: f32, floating: bool) {
    let lo = FIELD + Vec3::new(-200.0, -200.0, top - 16.0);
    let hi = FIELD + Vec3::new(200.0, 200.0, top);
    let mut e = pl.sim.app.world_mut().spawn(chimera_core::core::MovingSolid {
        brushes: vec![brush(lo, hi, Some("concrete"))],
        velocity: to_engine(Vec3::Z * vz),
        solid: true,
    });
    if floating {
        e.insert(chimera_world::map::buoyancy::Floating);
    }
}

/// The landing speed of a fall onto the field from 600 units/s, on
/// `ground` (None: the floor).
fn landing_at_600(ground: Option<(f32, bool)>) -> (f32, Vec<(String, Option<f32>)>) {
    let mut pl = Player::on_floor();
    let top = 100.0;
    if let Some((vz, floating)) = ground {
        platform(&mut pl, top, vz, floating);
    }
    let floor = if ground.is_some() { top } else { 0.0 };
    pl.place(FIELD + Vec3::Z * (floor + 30.0));
    pl.set_vel(Vec3::Z * -600.0);
    pl.clear_heard();
    for _ in 0..20 {
        pl.tick();
        if pl.ms().on_ground {
            break;
        }
    }
    assert!(pl.ms().on_ground, "landed");
    (pl.me().last_landing_speed, pl.heard())
}

/// Falling and landing, row 54: on a floating object 173 less (427:
/// 0.85, no damage), on ground going down its speed less.
#[test]
fn landing_on_floating_and_descending_ground() {
    let (plain, _) = landing_at_600(None);
    assert!(plain > 600.0 && plain < 640.0, "{plain}");
    let (float, heard) = landing_at_600(Some((0.0, true)));
    close(float, plain - 173.0, "floating: 173 off");
    let step = heard.iter().find(|(e, _)| e.contains("RunFootstep")).and_then(|(_, v)| *v);
    assert_eq!(step, Some(0.85), "427 is the 0.85 tier");
    let (down, _) = landing_at_600(Some((-100.0, false)));
    close(down, plain - 100.0, "descending at 100: 100 off");
    let (up, _) = landing_at_600(Some((100.0, false)));
    close(up, plain, "rising ground: nothing");
}

/// Falling and landing: `IgnoreFallDamage` spares the first landing of
/// its window and ends; `IgnoreFallDamageWithoutReset` spares every one.
#[test]
fn ignore_fall_damage_windows() {
    for reset in [true, false] {
        let mut pl = Player::at(FIELD + Vec3::Z * 245.0);
        let now = pl.sim.app.world().resource::<chimera_core::core::SimClock>().now;
        pl.sim
            .app
            .world_mut()
            .entity_mut(pl.p)
            .insert(chimera_core::core::IgnoreFallDamage { until: now + 10.0, reset });
        let fall = |pl: &mut Player| {
            pl.place(FIELD + Vec3::Z * 245.0);
            pl.sim.ticks(70);
            assert!(pl.ms().on_ground);
            pl.health()
        };
        assert_eq!(fall(&mut pl), 100.0, "the first fall is spared (reset {reset})");
        let second = fall(&mut pl);
        assert_eq!(second, if reset { 90.0 } else { 100.0 }, "the second (reset {reset})");
    }
}

/// Jumping: `toggle_duck` ducks until toggled again or, with
/// `xc_uncrouch_on_jump` 1, a jump press.
#[test]
fn toggle_duck_and_uncrouch_on_jump() {
    for uncrouch in [1u8, 0] {
        let mut pl = Player::on_floor();
        pl.sim.app.world_mut().resource_mut::<movement::UncrouchOnJump>().0 = uncrouch;
        pl.sim.app.world_mut().entity_mut(pl.p).insert(movement::DuckToggle { on: true, ..Default::default() });
        pl.sim.ticks(30);
        assert!(pl.me().ducked, "toggled: ducks");
        pl.intent().jump = true;
        pl.tick();
        pl.intent().jump = false;
        let on = pl.sim.app.world().get::<movement::DuckToggle>(pl.p).unwrap().on;
        assert_eq!(on, uncrouch == 0, "a jump un-toggles only with xc_uncrouch_on_jump 1");
        pl.sim.ticks(80);
        assert_eq!(pl.me().ducked, uncrouch == 0);
    }
}

/// Step sounds: waist deep (water level 2) a step every 600 ms at 0.65,
/// one attempt in four (on a counter every player shares) put off a tick.
#[test]
fn waist_deep_steps_skip_one_attempt_in_four() {
    let mut pl = Player::at(WADE + Vec3::Z * 0.5);
    pl.sim.ticks(4);
    assert_eq!(pl.me().water_level, 2, "waist deep");
    pl.keys(1.0, 0.0);
    pl.clear_heard();
    pl.sim.ticks(200);
    let steps: Vec<(u64, Option<f32>)> = pl
        .sim
        .app
        .world()
        .resource::<Heard>()
        .sounds
        .iter()
        .filter(|(_, e, _)| e.contains("RunFootstep"))
        .map(|(t, _, v)| (*t, *v))
        .collect();
    assert!(steps.len() >= 4, "{steps:?}");
    assert!(steps.iter().all(|(_, v)| *v == Some(0.65)), "{steps:?}");
    let gaps: Vec<u64> = steps.windows(2).map(|w| w[1].0 - w[0].0).collect();
    // Attempts 1, 5, 9 ... skip: after the first (delayed) step two come
    // 40 ticks apart, then one 41.
    assert_eq!(&gaps[..3], &[40, 40, 41], "{steps:?}");
}
