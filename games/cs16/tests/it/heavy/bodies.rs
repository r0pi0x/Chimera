//! The eight player models as bodies (`game::bodies`,
//! `goldsrc_engine::content::studio`) on the user's install: each reads
//! with meshes skinned to its 53 bones, its hitboxes (the shield's left
//! out), the gait sequences the bodies play and the pistol's aim; posed in
//! `idle1` it stands on its feet (lowest point near the origin) about a
//! player's height tall, ducked on `crouch_idle` and `crouchrun` too
//! (report f77f1f74), with its toes toward -Z (forward), and its bind
//! pose's mesh stands on the origin too. Passes with a note without an
//! install.

use bevy::prelude::*;
use chimera_world::map::{MapData, anim::Animator};
use cs16::game::{GAME, bodies, weapons};
use goldsrc_engine::content::studio;

use super::maps::mount;

#[test]
fn the_eight_player_models_are_bodies() {
    let Some(mount) = mount() else { return };
    let mut map = MapData::default();
    bodies::add_bodies(&mount, false, &mut map);
    assert_eq!(map.kit.characters.len(), 8, "{:?}", map.warnings);
    for (i, c) in map.kit.characters.iter().enumerate() {
        let path = bodies::CLASS_MODELS[i];
        assert_eq!(c.game, Some(GAME));
        assert!(!c.model.meshes.is_empty(), "{path}");
        assert!(c.hitboxes.len() >= 19 && c.hitboxes.len() == c.boxes.len(), "{path}: {} hitboxes", c.hitboxes.len());
        let set = c.animations.as_ref().expect("sequences");
        for s in ["idle1", "walk", "run", "crouch_idle", "crouchrun", "jump"] {
            assert!(set.sequence(s).is_some(), "{path}: no {s}");
        }
        // Every weapon's set: its aim and shot standing and ducked.
        for w in &weapons::weapons().weapons {
            let set_name = body_set(w);
            for crouching in [false, true] {
                for what in ["aim", "shoot"] {
                    let s = bodies::upper(what, &set_name, crouching);
                    assert!(set.sequence(&s).is_some(), "{path}: no {s} for {}", w.id);
                }
            }
        }
        // Looking up raises the hands (the pitch blends' direction).
        let aim = set.sequence("ref_aim_ak47").unwrap();
        let hand_height = |pitch: f32| {
            let mut a = Animator::new(set.clone());
            a.play(set.sequence("idle1").unwrap(), 0.0);
            a.layers = vec![Some(chimera_world::map::anim::Layer { sequence: aim, cycle: 0.0, weight: 1.0 })];
            let i = set.param(studio::PITCH_PARAM).unwrap();
            a.set_param(i, bodies::aim_param(pitch));
            let globals = chimera_world::map::anim::model_space(&a.pose(0.0), &set.parents);
            let at = |n: &str| {
                let i = set.bone_names.iter().position(|b| b == n).unwrap();
                c.root.rotation * globals[i].1 * c.root.scale
            };
            // How steeply the right forearm points (the gun follows it).
            (at("bip01 r hand") - at("bip01 r forearm")).normalize().y
        };
        let (up, down) = (hand_height(60.0), hand_height(-60.0));
        assert!(up > down + 0.3, "{path}: the right forearm's rise {up} looking up, {down} looking down");
        // Posed in idle1: feet near 0, a player tall (metres).
        let mut a = Animator::new(set.clone());
        a.play(set.sequence("idle1").unwrap(), 0.0);
        let pose = a.pose(0.0);
        let globals = chimera_world::map::anim::model_space(&pose, &set.parents);
        let points: Vec<Vec3> = globals.iter().map(|(_, p)| c.root.rotation * *p * c.root.scale).collect();
        let bone = |n: &str| set.bone_names.iter().position(|b| b == n).unwrap();
        let toe = points[bone("bip01 l toe0")] - points[bone("bip01 l foot")];
        eprintln!("{path}: left toe from the foot {toe}");
        assert!(toe.z < -2.0 * toe.x.abs(), "{path}: the feet point {toe}, not -Z");
        let lo = points.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        let hi = points.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        eprintln!("{path}: bones from {lo:.3} to {hi:.3} m, mesh {:?}, {} meshes, {} textures", c.model.bounds, c.model.meshes.len(), map.textures.len());
        assert!(lo > -0.1 && lo < 0.2, "{path}: lowest bone at {lo} m");
        assert!(hi > 1.2 && hi < 2.0, "{path}: highest bone at {hi} m");
        // Ducked (report f77f1f74): the feet on the ground too, on the
        // ducked gaits, frame 0 and halfway.
        for s in ["crouch_idle", "crouchrun"] {
            for cycle in [0.0, 0.5] {
                let mut a = Animator::new(set.clone());
                a.play(set.sequence(s).unwrap(), 0.0);
                a.cycle = cycle;
                let globals = chimera_world::map::anim::model_space(&a.pose(0.0), &set.parents);
                let lo = globals.iter().map(|(_, p)| (c.root.rotation * *p * c.root.scale).y).fold(f32::MAX, f32::min);
                eprintln!("{path}: {s} at cycle {cycle}: lowest bone at {lo:.3} m");
                assert!(lo > -0.1 && lo < 0.2, "{path}: {s} at cycle {cycle}: lowest bone at {lo} m");
            }
        }
        let mesh_lo = c.model.bounds.0.y;
        assert!(mesh_lo > -0.15 && mesh_lo < 0.1, "{path}: the mesh's feet at {mesh_lo} m");
        // The run's authored speed: 250 units a second (mdl.md).
        let run = set.sequence("run").unwrap();
        assert!((set.ground_speed(run, &a.params) - 250.0).abs() < 1.0);
    }
    // The textures read too.
    let full = studio::character(&mount, GAME, bodies::CLASS_MODELS[7], true).unwrap();
    assert!(full.textures.iter().all(|t| t.rgba8.len() == (t.width * t.height * 4) as usize));
}

/// A weapon's body set as its data names it (`[hooks."cs16:body_anim"]`).
fn body_set(w: &chimera_sim::weapon::data::WeaponData) -> String {
    let hook = w.hooks.get(bodies::BODY_ANIM_HOOK).unwrap_or_else(|| panic!("{}: no body set", w.id));
    let b: bodies::BodyAnim = hook.clone().try_into().unwrap();
    b.set
}
