//! A CS 1.6 body animating on the harness (`game::bodies::drive`): a
//! stand-in body of this game (two bones, the gait sequences by the
//! player models' names and the pistol's aim) on the greybox stands on
//! `idle1` with the aim layered over it, runs on `run` while moving with
//! its cycle going round and its pose changing, and stands again when it
//! stops. The real models' sequences are read in `heavy::bodies`.

use std::sync::Arc;

use bevy::prelude::*;
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
    map::{
        MapCharacterModel, MapData, MapModel, MapSimPlugin,
        anim::{AnimSet, Animation, Animator, Sequence, Track},
    },
};
use cs16::{
    game::{GAME, movement},
    harness::Cs16Sim,
};

fn animation(name: &str, frames: usize, swing: f32, speed: f32) -> Animation {
    Animation {
        name: name.into(),
        fps: 30.0,
        frames,
        speed,
        tracks: vec![Track {
            bone: 1,
            rotation: Some((0..frames).map(|f| Quat::from_rotation_x(swing * (f as f32 / frames as f32 * 6.28).sin())).collect()),
            position: None,
        }],
        ..Default::default()
    }
}

fn sequence(name: &str, anim: usize, weights: Vec<f32>) -> Sequence {
    Sequence {
        name: name.into(),
        looping: true,
        grid: (1, 1),
        anims: vec![anim],
        bone_weights: weights,
        ..Default::default()
    }
}

fn stand_in() -> AnimSet {
    let defaults = vec![(Quat::IDENTITY, Vec3::ZERO), (Quat::IDENTITY, Vec3::Z * 10.0)];
    let mut set = AnimSet {
        bases: vec![defaults.clone()],
        defaults,
        parents: vec![None, Some(0)],
        bone_names: vec!["bip01".into(), "bip01 spine".into()],
        ..Default::default()
    };
    set.animations = vec![
        animation("idle1", 31, 0.05, 0.0),
        animation("walk", 31, 0.4, 60.0),
        animation("run", 31, 0.8, 250.0),
        animation("ref_aim_onehanded", 2, 0.0, 0.0),
    ];
    set.sequences = vec![
        sequence("idle1", 0, vec![1.0, 1.0]),
        sequence("walk", 1, vec![1.0, 1.0]),
        sequence("run", 2, vec![1.0, 1.0]),
        sequence("ref_aim_onehanded", 3, vec![0.0, 0.5]),
    ];
    for (i, s) in set.sequences.iter().enumerate() {
        set.names.insert(s.name.clone(), i);
    }
    set
}

#[test]
fn a_body_stands_runs_and_stands_again() {
    let body = MapCharacterModel {
        game: Some(GAME),
        team: None,
        name: None,
        class: None,
        model: MapModel::default(),
        hitboxes: Vec::new(),
        bones: Vec::new(),
        root: Transform::default(),
        animations: Some(Arc::new(stand_in())),
        boxes: Vec::new(),
        ragdoll: None,
    };
    let mut map = MapData::default();
    map.kit.characters.push(body);
    let mut sim = Sim::new(Cs16Sim, (GreyboxMapPlugin, MapSimPlugin::new(map)));
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.seconds(1.0);
    let read = |sim: &Sim| {
        let a = sim.app.world().get::<Animator>(p).expect("the body has an animator");
        let main = a.set.sequences[a.main.expect("something plays")].name.clone();
        let layer = a.layers.first().copied().flatten().map(|l| a.set.sequences[l.sequence].name.clone());
        (main, layer, a.cycle, a.pose(0.0)[1].0)
    };
    let (main, layer, _, _) = read(&sim);
    assert_eq!(main, "idle1");
    assert_eq!(layer.as_deref(), Some("ref_aim_onehanded"));
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(0.5);
    let (main, _, c0, q0) = read(&sim);
    assert_eq!(main, "run", "running at {} m/s", sim.velocity(p).length());
    sim.seconds(0.2);
    let (_, _, c1, q1) = read(&sim);
    assert_ne!(c0, c1, "the run's cycle goes round");
    assert!(q0.angle_between(q1) > 1e-3, "the pose changes as it runs");
    sim.intent(p).move_axis = Vec2::ZERO;
    sim.seconds(1.0);
    assert_eq!(read(&sim).0, "idle1");
}
