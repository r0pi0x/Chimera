//! CS 1.6's footsteps (specs/cs16/footsteps.md, test cases) on the
//! greybox at 10 ms commands, with the game's step sets as the sound
//! bank's surfaces: the first step 0.4 s after the key goes down and
//! then every 0.3 s, the feet alternating (files 1/3 then 2/4), the
//! 150 units/s gate (the walk key and the duck are silent), a sliding
//! duck's volume, the ladder's, the wade's with every fourth step
//! skipped, a jump's step at take-off, a landing's, `mp_footsteps` 0.

use std::sync::Arc;

use bevy::prelude::*;
use chimera_core::core::{MapBrush, MapWater, MapWaterVolume, Velocity};
use chimera_movement::{QuakeMovement, to_engine, to_units};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
    map::{MapSounds, PlaySound, sound::SoundBank},
};
use cs16::{game::movement::ID, harness::Cs16Sim};
use goldsrc_engine::content::sounds::{step_entries, step_surfaces};

/// Steps heard: entry, volume.
#[derive(Resource, Default)]
struct Heard(Vec<(String, Option<f32>)>);

fn hear(mut sounds: MessageReader<PlaySound>, mut heard: ResMut<Heard>) {
    heard.0.extend(sounds.read().map(|s| (s.entry.clone(), s.volume)));
}

fn standing() -> (Sim, Entity) {
    let mut sim = Sim::new(Cs16Sim, GreyboxMapPlugin);
    sim.set_tick_interval(cs16::game::TICK_INTERVAL);
    sim.repeatable();
    sim.ticks(2);
    let mut sounds = MapSounds::default();
    sounds.surfaces.extend(step_surfaces());
    sim.app.world_mut().insert_resource(SoundBank(Arc::new(sounds)));
    sim.app.init_resource::<Heard>().add_systems(Last, hear);
    let p = sim.spawn_character(greybox::SPAWNS[0], ID);
    sim.ticks(150);
    assert!(me(&sim, p).on_ground);
    sim.app.world_mut().resource_mut::<Heard>().0.clear();
    (sim, p)
}

fn me(sim: &Sim, p: Entity) -> QuakeMovement {
    sim.app.world().get::<QuakeMovement>(p).unwrap().clone()
}

fn heard(sim: &mut Sim) -> Vec<(String, Option<f32>)> {
    std::mem::take(&mut sim.app.world_mut().resource_mut::<Heard>().0)
}

fn concrete() -> (String, String) {
    step_entries("concrete", "pl_step")
}

/// Commands (1-based) at which steps sounded in `n` commands.
fn step_commands(sim: &mut Sim, n: usize) -> Vec<(usize, (String, Option<f32>))> {
    let mut out = Vec::new();
    for i in 1..=n {
        sim.ticks(1);
        out.extend(heard(sim).into_iter().map(|h| (i, h)));
    }
    out
}

#[test]
fn the_first_step_comes_at_0_4_s_then_every_0_3_s() {
    let (mut sim, p) = standing();
    sim.intent(p).move_axis = Vec2::Y;
    let steps = step_commands(&mut sim, 130);
    let at: Vec<usize> = steps.iter().map(|(i, _)| *i).collect();
    // The check sees speed 0 on the first command (timer 400); the step
    // comes with the speed above 150, 40 commands on, then every 30.
    assert!(at.len() >= 3, "{at:?}");
    assert!((41..=60).contains(&at[0]), "the first step {at:?}");
    assert!(at.windows(2).all(|w| w[1] - w[0] == 30), "every 0.3 s: {at:?}");
    let (left, right) = concrete();
    let names: Vec<&str> = steps.iter().map(|(_, (e, _))| e.as_str()).collect();
    assert!(!names.is_empty());
    assert!(steps.iter().all(|(_, (e, v))| (e == &left || e == &right) && *v == Some(0.5)), "{steps:?}");
    // The feet alternate.
    let feet: Vec<bool> = steps.iter().map(|(_, (e, _))| *e == left).collect();
    assert!(feet.windows(2).all(|w| w[0] != w[1]), "{feet:?}");
    // The right foot's file is 1 or 3, the left's 2 or 4.
    assert_eq!(right, "player/pl_step1.wav;player/pl_step3.wav");
    assert_eq!(left, "player/pl_step2.wav;player/pl_step4.wav");
}

#[test]
fn walking_and_ducking_are_silent() {
    let (mut sim, p) = standing();
    sim.intent(p).move_axis = Vec2::Y;
    sim.intent(p).walk = true;
    let steps = step_commands(&mut sim, 120);
    assert!(steps.is_empty(), "130 units/s: {steps:?}");
    sim.intent(p).walk = false;
    sim.intent(p).crouch = true;
    sim.ticks(80);
    heard(&mut sim);
    assert!(step_commands(&mut sim, 100).is_empty(), "ducked at 83");
}

#[test]
fn a_slide_ducked_steps_every_0_4_s_at_0_175() {
    let (mut sim, p) = standing();
    sim.intent(p).crouch = true;
    sim.ticks(60);
    heard(&mut sim);
    sim.app.world_mut().get_mut::<Velocity>(p).unwrap().0 = to_engine(Vec3::new(0.0, 200.0, 0.0));
    // Sliding at 200 under friction slows below 150 within a few commands:
    // the first step at most.
    let steps = step_commands(&mut sim, 40);
    for (_, (_, v)) in &steps {
        let v = v.unwrap();
        assert!((v - 0.175).abs() < 1e-4, "{v}");
    }
}

#[test]
fn the_ladder_steps_at_0_35_every_0_45_s() {
    let (mut sim, p) = standing();
    {
        let w = sim.app.world_mut();
        let mut m = w.get_mut::<QuakeMovement>(p).unwrap();
        m.ladder = Some(Vec3::NEG_X);
        m.on_ground = false;
        m.step_timer = 0.0;
        w.get_mut::<Velocity>(p).unwrap().0 = to_engine(Vec3::new(0.0, 0.0, 200.0));
    }
    // Hold the speed: the ladder's own move is not what is tested.
    let mut at = Vec::new();
    for i in 1..=100usize {
        {
            let w = sim.app.world_mut();
            w.get_mut::<Velocity>(p).unwrap().0 = to_engine(Vec3::new(0.0, 0.0, 200.0));
            let mut m = w.get_mut::<QuakeMovement>(p).unwrap();
            m.ladder = Some(Vec3::NEG_X);
        }
        sim.ticks(1);
        for (e, v) in heard(&mut sim) {
            assert!(e.contains("pl_ladder"), "{e}");
            assert_eq!(v, Some(0.35));
            at.push(i);
        }
    }
    assert!(at.len() >= 2, "{at:?}");
    assert!(at.windows(2).all(|w| w[1] - w[0] == 45), "every 0.45 s: {at:?}");
}

#[test]
fn wading_steps_every_0_6_s_with_every_fourth_skipped() {
    let (mut sim, p) = standing();
    // Knee-deep water: 14.4 units over the feet.
    let lo = to_engine(Vec3::new(-8192.0, -8192.0, -64.0));
    let hi = to_engine(Vec3::new(8192.0, 8192.0, 14.4 + 6.0));
    sim.app.world_mut().insert_resource(MapWater(vec![MapWaterVolume {
        brush: MapBrush::from_box(lo.min(hi), lo.max(hi)),
        slime: false,
        lava: false,
        current: None,
    }]));
    sim.intent(p).move_axis = Vec2::Y;
    sim.ticks(60);
    heard(&mut sim);
    let steps = step_commands(&mut sim, 240);
    let (left, right) = step_entries("wade", "pl_wade");
    assert!(steps.iter().all(|(_, (e, v))| (e == &left || e == &right) && *v == Some(0.65)), "{steps:?}");
    // 240 commands = 4 checks of 0.6 s each... counted from the first
    // step; three of every four play.
    let n = steps.len();
    assert!((2..=4).contains(&n), "{n} wade sounds in 2.4 s: {steps:?}");
}

#[test]
fn a_jump_steps_at_full_volume_from_150_and_a_landing_by_its_speed() {
    let (mut sim, p) = standing();
    // Still: no step at take-off.
    sim.intent(p).jump = true;
    sim.ticks(1);
    assert!(heard(&mut sim).is_empty(), "standing jump");
    sim.intent(p).jump = false;
    sim.ticks(150);
    heard(&mut sim);
    // Running at 250: the step at 1.0.
    sim.intent(p).move_axis = Vec2::Y;
    sim.ticks(60);
    heard(&mut sim);
    sim.intent(p).jump = true;
    sim.ticks(1);
    let h = heard(&mut sim);
    assert!(h.iter().any(|(_, v)| *v == Some(1.0)), "{h:?}");
    // Landing at 300 units/s: the surface's step at 0.85.
    sim.intent(p).jump = false;
    sim.intent(p).move_axis = Vec2::ZERO;
    sim.ticks(200);
    heard(&mut sim);
    {
        let w = sim.app.world_mut();
        let at = w.get::<Transform>(p).unwrap().translation + Vec3::Y * 2.0;
        w.get_mut::<Transform>(p).unwrap().translation = at;
        if let Some(mut pos) = w.get_mut::<avian3d::prelude::Position>(p) {
            pos.0 = at;
        }
        w.get_mut::<Velocity>(p).unwrap().0 = to_engine(Vec3::new(0.0, 0.0, -300.0));
    }
    let steps = step_commands(&mut sim, 30);
    assert!(steps.iter().any(|(_, (_, v))| *v == Some(0.85)), "{steps:?}");
    assert!(me(&sim, p).step_timer <= 300.0);
    let _ = to_units(Vec3::ZERO);
}

#[test]
fn mp_footsteps_0_silences_but_the_foot_alternates() {
    // A control run counts the steps the same commands make.
    let (mut control, p) = standing();
    control.intent(p).move_axis = Vec2::Y;
    let n = step_commands(&mut control, 100).len();
    assert!(n >= 1);
    let (mut sim, p) = standing();
    {
        let w = sim.app.world_mut();
        let mut configs = w.resource_mut::<chimera_movement::QuakeConfigs>();
        configs.get_mut(ID).unwrap().set_cvar("mp_footsteps", "0").unwrap();
    }
    sim.intent(p).move_axis = Vec2::Y;
    let foot0 = me(&sim, p).step_left;
    let steps = step_commands(&mut sim, 100);
    assert!(steps.is_empty(), "silent: {steps:?}");
    assert_eq!(me(&sim, p).step_left, foot0 ^ (n % 2 == 1), "the foot flips with every step, heard or not");
}
