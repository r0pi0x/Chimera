//! Halo's 30 Hz motion drawn at the frame rate (`game::eased`; docs/
//! backlog.md, "Halo: view models and animations at frame rate") on
//! synthetic animation data: a body's animation sampled at the frame's own
//! time (past the tick, on its keyframes), a change of state cross-fading
//! without a pop, a corpse's death and fall between ticks, positions still
//! eased between the last two ticks, a vehicle's wheel eased the short way
//! round, the motion tracker's sweep at the frame's time. The view model's
//! are in `view` (`the_view_is_sampled_at_the_frames_time`, the shot's
//! delay).

use std::{sync::Arc, time::Duration};

use bevy::{ecs::system::RunSystemOnce, prelude::*, time::TimeUpdateStrategy};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
    map::{
        BodyModel, CharacterModels, MapCharacterModel,
        anim::{AnimSet, Animation, Animator, BonePose, Sequence, Track},
        interp::{GameTicks, InterpPlugin},
    },
};
use halo_ce::{
    game::{
        GAME,
        body::{HaloBody, anims::key, death::{CorpseState, HaloCorpse}},
        eased::{HaloEased, ease},
        movement,
        vehicles::look::LookValues,
    },
    harness::HaloSim,
};

/// Bone 0 turned about Z by `z` and about X by `x`.
fn turn(z: f32, x: f32) -> Quat {
    Quat::from_rotation_z(z) * Quat::from_rotation_x(x)
}

/// A one-bone set: the unarmed idle turning bone 0 about Z by 0.05 a frame
/// (40 frames, looping), the forward move turning it about X by 0.05 a
/// frame from 1 rad, and a 40-frame death turning it about Z by 0.02 a
/// frame (`damage s-kill front gut`).
fn set() -> AnimSet {
    let anim = |name: &str, frames: usize, f: &dyn Fn(usize) -> Quat| Animation {
        name: name.into(),
        fps: 30.0,
        frames,
        tracks: vec![Track {
            bone: 0,
            rotation: Some((0..frames).map(f).collect()),
            position: None,
        }],
        ..Default::default()
    };
    let mut s = AnimSet {
        defaults: vec![(Quat::IDENTITY, Vec3::ZERO)],
        bases: vec![vec![(Quat::IDENTITY, Vec3::ZERO)]],
        parents: vec![None],
        ..Default::default()
    };
    s.animations.push(anim("bind", 1, &|_| Quat::IDENTITY));
    s.animations.push(anim("idle", 40, &|f| turn(0.05 * f as f32, 0.0)));
    s.animations.push(anim("move", 40, &|f| turn(0.0, 1.0 + 0.05 * f as f32)));
    s.animations.push(anim("death", 40, &|f| turn(0.02 * f as f32, 0.0)));
    for (name, a) in [
        ("bind".to_string(), 0),
        (key("stand", "unarmed", "idle"), 1),
        (key("stand", "unarmed", "move-front"), 2),
        ("damage s-kill front gut".to_string(), 3),
    ] {
        s.sequences.push(Sequence {
            name: name.clone(),
            anims: vec![a],
            grid: (1, 1),
            ..Default::default()
        });
        s.names.insert(name, s.sequences.len() - 1);
    }
    s
}

fn models() -> CharacterModels {
    CharacterModels(Arc::new(vec![MapCharacterModel {
        game: Some(GAME),
        team: None,
        name: None,
        class: None,
        model: Default::default(),
        hitboxes: Vec::new(),
        bones: Vec::new(),
        root: Transform::IDENTITY,
        animations: Some(Arc::new(set())),
        boxes: Vec::new(),
        ragdoll: None,
    }]))
}

fn sim() -> Sim {
    let mut sim = Sim::new(HaloSim, (GreyboxMapPlugin, InterpPlugin));
    sim.app.world_mut().insert_resource(models());
    sim
}

/// A Halo body as the map's would give it (the harness attaches none).
fn body(sim: &mut Sim) -> Entity {
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.app.world_mut().entity_mut(p).insert((
        chimera_core::slots::GameIdentity(GAME.into()),
        BodyModel(0),
        Animator::new(Arc::new(set())),
    ));
    p
}

/// A frame `f` of a tick past the last tick, drawn (`ease`).
fn frame(sim: &mut Sim, f: f32) {
    let w = sim.app.world_mut();
    let step = w.resource::<Time<Fixed>>().timestep();
    let o = w.resource::<Time<Fixed>>().overstep();
    w.resource_mut::<Time<Fixed>>().discard_overstep(o);
    w.resource_mut::<Time<Fixed>>().accumulate_overstep(step.mul_f32(f));
    w.run_system_once(ease).unwrap();
}

fn drawn(sim: &Sim, e: Entity) -> Vec<BonePose> {
    sim.app.world().get::<Animator>(e).unwrap().fixed.as_ref().unwrap().get().clone()
}

fn angle_z(q: Quat) -> f32 {
    let (axis, a) = q.to_axis_angle();
    a * axis.z.signum()
}

/// Sampled at the frame's time: past the tick the pose runs on along the
/// animation's keyframes (the idle at frame n + f), not between the last
/// two ticks; the next tick lays what was predicted.
#[test]
fn a_bodys_animation_is_sampled_past_its_tick() {
    let mut sim = sim();
    let p = body(&mut sim);
    sim.ticks(10);
    let laid = |sim: &Sim| sim.app.world().get::<HaloBody>(p).unwrap().drawn.latest().unwrap().get().clone();
    let now = angle_z(laid(&sim)[0].0);
    for f in [0.25f32, 0.5, 0.9] {
        frame(&mut sim, f);
        let got = angle_z(drawn(&sim, p)[0].0);
        assert!((got - (now + 0.05 * f)).abs() < 1e-4, "{f}: {got} vs {now} + {}", 0.05 * f);
    }
    frame(&mut sim, 0.0);
    sim.ticks(1);
    let next = angle_z(laid(&sim)[0].0);
    assert!((next - (now + 0.05)).abs() < 1e-4, "the next keyframe: {next}");
    assert!(sim.app.world().get::<HaloBody>(p).unwrap().drawn.latest().is_some());
}

/// A change of state the next tick brings (standing to running: the
/// game's 6-tick blend into the move) is drawn without a pop: at 144
/// frames a second, no frame moves the pose by more than half what a tick
/// moves it.
#[test]
fn a_change_of_state_cross_fades_without_a_pop() {
    let mut sim = sim();
    let p = body(&mut sim);
    sim.ticks(10);
    sim.app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 144.0)));
    let mut frames = Vec::new();
    let mut ticks = Vec::new();
    for k in 0..200 {
        sim.intent(p).move_axis = if k >= 20 { Vec2::Y } else { Vec2::ZERO };
        let before = sim.tick();
        sim.app.update();
        let pose = drawn(&sim, p)[0].0;
        frames.push(pose);
        if sim.tick() != before {
            ticks.push(sim.app.world().get::<HaloBody>(p).unwrap().drawn.latest().unwrap().get()[0].0);
        }
    }
    let state = sim.app.world().get::<HaloBody>(p).unwrap().anim.state;
    assert_eq!(state, halo_ce::game::body::anims::State::MoveFront, "running");
    let most = |v: &[Quat]| v.windows(2).map(|w| w[0].angle_between(w[1])).fold(0.0f32, f32::max);
    let (frame_step, tick_step) = (most(&frames), most(&ticks));
    assert!(frame_step <= 0.5 * tick_step, "a frame's step {frame_step} against a tick's {tick_step}");
}

/// A corpse's death plays at the frame's time and its fall is eased
/// between its last two ticks.
#[test]
fn a_corpse_moves_between_ticks() {
    let mut sim = sim();
    sim.ticks(2);
    let at = greybox::SPAWNS[0] + Vec3::Y * 4.0;
    let c = sim
        .app
        .world_mut()
        .spawn(HaloCorpse {
            owner: Entity::PLACEHOLDER,
            skin: 0,
            at,
            yaw: 0.0,
            velocity: Vec3::new(0.0, -0.1, 0.0),
            animation: "damage s-kill front gut".into(),
            body: Default::default(),
        })
        .id();
    sim.ticks(6);
    let w = sim.app.world();
    let laid = angle_z(w.get::<CorpseState>(c).unwrap().drawn.latest().unwrap().get()[0].0);
    let (prev, last) = {
        let e = &w.get::<HaloEased>(c).unwrap().0;
        let t = halo_ce::game::eased::laid(w.resource::<chimera_core::core::SimClock>());
        (e.at(t as f64 - 1.0).unwrap(), e.at(t as f64).unwrap())
    };
    assert!(prev.translation.y > last.translation.y + 1e-4, "falling: {prev:?} {last:?}");
    frame(&mut sim, 0.5);
    let got = angle_z(drawn(&sim, c)[0].0);
    assert!((got - (laid + 0.01)).abs() < 1e-4, "the death past its tick: {got} vs {laid}");
    let y = sim.app.world().get::<Transform>(c).unwrap().translation.y;
    assert!(y < prev.translation.y && y > last.translation.y, "between its ticks: {y}");
}

/// What the simulation moves (here: a shot shown in flight's like) is
/// still eased between its last two ticks, one tick behind at Halo's
/// own 30 Hz.
#[test]
fn positions_are_still_eased_between_ticks() {
    let mut sim = sim();
    let e = sim.app.world_mut().spawn((Transform::default(), HaloEased::default())).id();
    sim.app.add_systems(FixedUpdate, move |mut q: Query<&mut Transform, With<HaloEased>>| {
        if let Ok(mut t) = q.get_mut(e) {
            t.translation.x += 1.0;
        }
    });
    sim.ticks(5);
    // The simulation's (the frame after the tick drew the eased one).
    let x = sim.app.world().get::<HaloEased>(e).unwrap().0.latest().unwrap().translation.x;
    frame(&mut sim, 0.25);
    let drawn = sim.app.world().get::<Transform>(e).unwrap().translation.x;
    assert!((drawn - (x - 0.75)).abs() < 1e-4, "a quarter past the tick before: {drawn} vs {x}");
}

/// A wheel's position eased the short way round its circumference.
#[test]
fn a_wheel_turns_between_ticks_the_short_way() {
    let mut g = GameTicks::default();
    let v = |wheel: f32, s: f32| LookValues { wheel, turn: s, treads: [wheel, 0.0], suspension: vec![s] };
    g.lay(4, v(0.9, 0.0));
    g.lay(5, v(0.1, 1.0));
    let (a, b, f) = g.around(4.5).unwrap();
    let m = LookValues::between(a, b, f, 1.0);
    assert!(m.wheel.min(1.0 - m.wheel) < 1e-5, "across 0: {}", m.wheel);
    assert!((m.turn - 0.5).abs() < 1e-6 && (m.suspension[0] - 0.5).abs() < 1e-6);
}

/// The tracker's sweep at the frame's time: between its ticks' sizes.
#[test]
fn the_trackers_sweep_moves_between_ticks() {
    use halo_ce::ui::hud::rules::{sweep, sweep_at};
    let (a, b) = (sweep(10), sweep(11));
    let m = sweep_at(10.5);
    assert!(m < a.max(b) && m > a.min(b), "{a} {m} {b}");
    assert_eq!(sweep_at(10.0), a);
}
