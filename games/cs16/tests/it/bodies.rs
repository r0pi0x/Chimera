//! A CS 1.6 body animating on the harness (`game::bodies::drive`): a
//! stand-in body of this game (two bones, the gait sequences by the
//! player models' names and the pistol's aim) on the greybox stands on
//! `idle1` with the aim layered over it, runs on `run` while moving with
//! its cycle going round and its pose changing, and stands again when it
//! stops; holding the AK-47 (its held model, `map::Held`) it aims on
//! `ref_aim_ak47`, and a shot plays
//! `ref_shoot_ak47` over the aim to its end. The real models' sequences
//! are read in `heavy::bodies`.

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
use chimera_sim::weapon::{Inventory, give};
use cs16::{
    game::{GAME, TICK_INTERVAL, movement},
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
        animation("ref_aim_ak47", 2, 0.0, 0.0),
        animation("ref_shoot_ak47", 6, 0.3, 0.0),
    ];
    set.sequences = vec![
        sequence("idle1", 0, vec![1.0, 1.0]),
        sequence("walk", 1, vec![1.0, 1.0]),
        sequence("run", 2, vec![1.0, 1.0]),
        sequence("ref_aim_onehanded", 3, vec![0.0, 0.5]),
        sequence("ref_aim_ak47", 4, vec![0.0, 0.5]),
        Sequence { looping: false, ..sequence("ref_shoot_ak47", 5, vec![0.0, 0.5]) },
    ];
    for (i, s) in set.sequences.iter().enumerate() {
        set.names.insert(s.name.clone(), i);
    }
    set
}

fn stand_in_sim() -> Sim {
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
    Sim::new(Cs16Sim, (GreyboxMapPlugin, MapSimPlugin::new(map)))
}

/// The names of what `p`'s body plays: its main sequence and its layers.
fn playing(sim: &Sim, p: Entity) -> (String, Vec<String>) {
    let a = sim.app.world().get::<Animator>(p).expect("the body has an animator");
    let name = |s: usize| a.set.sequences[s].name.clone();
    (name(a.main.expect("something plays")), a.layers.iter().flatten().map(|l| name(l.sequence)).collect())
}

#[test]
fn a_body_stands_runs_and_stands_again() {
    let mut sim = stand_in_sim();
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

#[test]
fn a_body_plays_the_ak47s_shot() {
    let mut sim = stand_in_sim();
    sim.set_tick_interval(TICK_INTERVAL);
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.seconds(1.0);
    let w = give(sim.app.world_mut(), p, "cs16:weapon_ak47").unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
    sim.seconds(2.0);
    assert_eq!(playing(&sim, p).1, ["ref_aim_ak47"]);
    let held = sim.app.world().get::<chimera_world::map::Held>(p).and_then(|h| h.0.clone());
    assert_eq!(held.as_deref(), Some("cs16:weapon_ak47"), "the body holds the AK's held model");
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    sim.ticks(2);
    assert_eq!(playing(&sim, p).1, ["ref_aim_ak47", "ref_shoot_ak47"], "the shot plays over the aim");
    // 6 frames at 30 fps: done within a second.
    sim.seconds(1.0);
    assert_eq!(playing(&sim, p).1, ["ref_aim_ak47"]);
}
