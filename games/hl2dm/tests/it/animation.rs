//! HL2:DM's player animation state (`game::player_anim`, specs/hl2dm/
//! player_models.md 3-8; its test cases 3-34): the main sequence by
//! activity and weapon kind, restarted at cycle 0; the playback rate
//! against the run cycles' blended ground speed; `move_yaw` from the gait
//! yaw; the feet planted to 45 degrees or 3 s, then turning at 360
//! degrees/s; `aim_pitch` and `aim_yaw` in their ranges; the fire and
//! reload gestures; the head's turn toward a near player. On a stand-in of
//! the animation models' layout (`stand_in`: the 105 sequences' names,
//! activities, lengths, ground speeds and pose ranges of §1.2 and §2),
//! with no install; the drawn bodies on the harness at the end.

use std::sync::Arc;

use bevy::prelude::*;
use chimera_world::map::anim::{AnimSet, Animation, Animator, Axis, PoseParam, Sequence};
use hl2dm::game::{
    look::{self, Head},
    player_anim::{Hl2PlayerAnim, Ideal, Kind},
};
use source_engine::player_anim::Inputs;

const KINDS: [&str; 10] = ["pistol", "shotgun", "smg1", "ar2", "gravgun", "grenade", "rpg", "crossbow", "melee", "slam"];
const DT: f32 = 0.015;

fn suffix(kind: &str) -> String {
    if kind == "gravgun" { "PHYSGUN".into() } else { kind.to_uppercase() }
}

/// The male animation model's layout (§1.2, §2), merged after `ragdoll`.
pub fn stand_in() -> AnimSet {
    // No bones: the state is what's tested, not the pose.
    let mut set = AnimSet { bases: vec![Vec::new()], ..default() };
    let param = |name: &str, start: f32, end: f32| PoseParam {
        name: name.into(),
        start,
        end,
        looping: 0.0,
    };
    set.params = vec![
        param("aim_yaw", -49.4432, 52.5174),
        param("aim_pitch", -80.8630, 49.9848),
        param("move_yaw", -180.0, 180.0),
        param("body_yaw", -29.7342, 29.7343),
        param("spine_yaw", -30.7085, 30.7085),
        param("head_yaw", -44.8190, 44.8187),
        param("head_pitch", -20.0, 23.0),
        param("head_roll", -10.9314, 10.8524),
    ];
    let anim = |set: &mut AnimSet, frames: usize, fps: f32, motion: Vec2| {
        set.animations.push(Animation {
            name: format!("a{}", set.animations.len()),
            fps,
            frames,
            speed: motion.length(),
            motion,
            ..default()
        });
        set.animations.len() - 1
    };
    let seq = |set: &mut AnimSet, name: String, activity: String, anims: Vec<usize>, looping: bool, axis: Option<usize>| {
        let grid = (anims.len(), 1);
        let axes = [
            Axis {
                param: axis,
                keys: if anims.len() == 9 { (0..9).map(|k| -180.0 + 45.0 * k as f32).collect() } else { Vec::new() },
                range: (-180.0, 180.0),
            },
            Axis::default(),
        ];
        set.sequences.push(Sequence {
            name,
            activity,
            looping,
            grid,
            anims,
            axes,
            fade_in: 0.2,
            fade_out: 0.2,
            ..default()
        });
    };
    let still = anim(&mut set, 1, 30.0, Vec2::ZERO);
    seq(&mut set, "ragdoll".into(), "ACT_DIERAGDOLL".into(), vec![still], false, None);
    let r = anim(&mut set, 101, 30.0, Vec2::ZERO);
    seq(&mut set, "reference".into(), String::new(), vec![r], false, None);
    for k in 0..15 {
        seq(&mut set, format!("aim_matrix_{k}"), String::new(), vec![still], false, None);
    }
    for k in KINDS {
        let a = anim(&mut set, 30, 30.0, Vec2::ZERO);
        seq(&mut set, format!("idle_{k}"), format!("ACT_HL2MP_IDLE_{}", suffix(k)), vec![a], true, None);
    }
    // The run cells by `move_yaw` key: S, SE, E, NE, N, NW, W, SW, S.
    let run = [169.8302, 149.8501, 149.8501, 209.7868, 209.7902, 209.7868, 149.8501, 149.8501, 169.8302];
    let grid = |set: &mut AnimSet, speeds: &[f32; 9], frames: usize| -> Vec<usize> {
        (0..9)
            .map(|k| {
                let a = (-180.0 + 45.0 * k as f32).to_radians();
                anim(set, frames, 30.0, Vec2::new(a.cos(), a.sin()) * speeds[k])
            })
            .collect()
    };
    for k in KINDS {
        seq(&mut set, format!("run_{k}_mod"), String::new(), vec![still], false, None);
        let cells = grid(&mut set, &run, 19);
        seq(&mut set, format!("run_{k}"), format!("ACT_HL2MP_RUN_{}", suffix(k)), cells, true, Some(2));
    }
    for k in KINDS {
        let frames = if matches!(k, "grenade" | "melee" | "slam") { 1 } else { 30 };
        let a = anim(&mut set, frames, 30.0, Vec2::ZERO);
        seq(&mut set, format!("cidle_{k}"), format!("ACT_HL2MP_IDLE_CROUCH_{}", suffix(k)), vec![a], true, None);
    }
    for k in KINDS {
        seq(&mut set, format!("cwalk_{k}_mod"), String::new(), vec![still], false, None);
        let cells = grid(&mut set, &[60.0; 9], 31);
        seq(&mut set, format!("cwalk_{k}"), format!("ACT_HL2MP_WALK_CROUCH_{}", suffix(k)), cells, true, Some(2));
    }
    for (k, frames) in KINDS.iter().zip([9, 30, 5, 5, 15, 25, 21, 46, 19, 16]) {
        let fps = if *k == "melee" { 45.0 } else { 30.0 };
        let a = anim(&mut set, frames, fps, Vec2::ZERO);
        let act = format!("ACT_HL2MP_GESTURE_RANGE_ATTACK_{}", suffix(k));
        seq(&mut set, format!("range_{k}"), act, vec![a], false, None);
    }
    for k in KINDS {
        let a = anim(&mut set, 27, 30.0, Vec2::ZERO);
        seq(&mut set, format!("jump_{k}"), format!("ACT_HL2MP_JUMP_{}", suffix(k)), vec![a], false, None);
    }
    for (k, frames) in ["pistol", "shotgun", "smg1", "ar2"].iter().zip([41, 66, 66, 66]) {
        let a = anim(&mut set, frames, 30.0, Vec2::ZERO);
        let act = format!("ACT_HL2MP_GESTURE_RELOAD_{}", suffix(k));
        seq(&mut set, format!("reload_{k}"), act, vec![a], false, None);
    }
    for n in ["body_rot_z", "spine_rot_z", "head_rot_z", "head_rot_y", "head_rot_x"] {
        seq(&mut set, n.into(), String::new(), vec![still], false, None);
    }
    set
}

fn animator() -> Animator {
    Animator::new(Arc::new(stand_in()))
}

/// A body that has stood for a while (`T` > 0) facing yaw 0.
fn standing() -> (Hl2PlayerAnim, Animator, Inputs) {
    let (mut s, mut a) = (Hl2PlayerAnim::default(), animator());
    let i = Inputs { on_ground: true, ..default() };
    s.step(&mut a, &i, Some("hl2dm:weapon_pistol"), DT, 0.0, true);
    (s, a, i)
}

fn name(a: &Animator) -> &str {
    &a.set.sequences[a.main.unwrap()].name
}

fn param(a: &Animator, n: &str) -> f32 {
    let i = a.set.param(n).unwrap();
    let p = &a.set.params[i];
    p.start + a.params[i] * (p.end - p.start)
}

fn close(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() < eps
}

#[test]
fn the_stand_in_has_the_merged_indices() {
    // Test case 1's indices, which the heavy test checks on the install.
    let set = stand_in();
    let at = |n: &str| set.sequences.iter().position(|s| s.name == n).unwrap();
    assert_eq!(set.sequences.len(), 106);
    for (i, n) in [(17, "idle_pistol"), (21, "idle_gravgun"), (32, "run_smg1"), (36, "run_gravgun"), (77, "range_pistol"), (90, "jump_ar2")] {
        assert_eq!(at(n), i, "{n}");
    }
}

#[test]
fn idle_with_the_gravity_gun() {
    // Case 3.
    let (mut s, mut a) = (Hl2PlayerAnim::default(), animator());
    let i = Inputs { on_ground: true, ..default() };
    s.step(&mut a, &i, Some("hl2dm:weapon_physcannon"), DT, 0.0, true);
    assert_eq!(name(&a), "idle_gravgun");
    assert_eq!(a.main, Some(21));
    assert!(close(a.cycle, 0.015 / 0.9667, 1e-4), "{}", a.cycle);
    assert_eq!(s.rate, 1.0);
}

#[test]
fn running_plays_at_speed_over_the_run_cycles_ground_speed() {
    // Cases 4-8: forward, sprint, back, left, between N and NE.
    for (v, eye, cell, rate) in [
        (Vec2::new(190.0, 0.0), 0.0, 0.0f32, 0.90567f32),
        (Vec2::new(320.0, 0.0), 0.0, 0.0, 1.52533),
        (Vec2::new(-190.0, 0.0), 0.0, 180.0, 1.11876),
        (Vec2::from_angle(120f32.to_radians()) * 190.0, 30.0, 90.0, 1.26793),
        (Vec2::from_angle((-22.5f32).to_radians()) * 190.0, 0.0, -22.5, 0.98029),
    ] {
        let (mut s, mut a) = (Hl2PlayerAnim::default(), animator());
        let i = Inputs { on_ground: true, velocity: v, eye_yaw: eye, ..default() };
        for _ in 0..3 {
            s.step(&mut a, &i, Some("hl2dm:weapon_smg1"), DT, 0.0, true);
        }
        assert_eq!(name(&a), "run_smg1");
        let move_yaw = param(&a, "move_yaw");
        assert!(close(move_yaw.abs(), cell.abs(), 0.01), "move_yaw {move_yaw} want {cell}");
        assert!(close(s.rate, rate, 2e-4), "{v}: rate {} want {rate}", s.rate);
        // A tick's cycle at that rate (0.6 s cycles).
        let before = a.cycle;
        s.step(&mut a, &i, Some("hl2dm:weapon_smg1"), DT, 0.0, true);
        assert!(close((a.cycle - before).rem_euclid(1.0), DT / 0.6 * rate, 1e-4));
    }
}

#[test]
fn crouching() {
    // Cases 9-11.
    let (mut s, mut a) = (Hl2PlayerAnim::default(), animator());
    let i = Inputs { on_ground: true, ducked: true, velocity: Vec2::new(63.333, 0.0), ..default() };
    s.step(&mut a, &i, Some("hl2dm:weapon_pistol"), DT, 0.0, true);
    s.step(&mut a, &i, Some("hl2dm:weapon_pistol"), DT, 0.0, true);
    assert_eq!((name(&a), a.main), ("cwalk_pistol", Some(58)));
    assert!(close(s.rate, 1.05556, 1e-4));
    let (mut s, mut a) = (Hl2PlayerAnim::default(), animator());
    let i = Inputs { on_ground: true, ducked: true, ..default() };
    s.step(&mut a, &i, Some("hl2dm:weapon_frag"), DT, 0.0, true);
    assert_eq!((name(&a), a.main, a.cycle), ("cidle_grenade", Some(52), 0.0));
    // Still ducking (the flag not set yet): standing sequences.
    let (mut s, mut a) = (Hl2PlayerAnim::default(), animator());
    s.step(&mut a, &Inputs { on_ground: true, ..default() }, Some("hl2dm:weapon_pistol"), DT, 0.0, true);
    assert_eq!(name(&a), "idle_pistol");
}

#[test]
fn a_jump_holds_until_the_landing_tick() {
    // Cases 12-13.
    let (mut s, mut a, ground) = standing();
    let w = Some("hl2dm:weapon_ar2");
    s.step(&mut a, &ground, w, DT, 0.0, true);
    let jump = Inputs { jumped: true, vertical: 160.0, ..default() };
    s.step(&mut a, &jump, w, DT, 0.0, true);
    assert_eq!((name(&a), a.main), ("jump_ar2", Some(90)));
    assert!(close(a.cycle, 0.015 * 1.15385, 1e-4));
    let air = Inputs { vertical: 10.0, ..default() };
    for _ in 0..33 {
        s.step(&mut a, &air, w, DT, 0.0, true);
    }
    assert_eq!(name(&a), "jump_ar2");
    assert!(close(a.cycle, 34.0 * 0.015 * 1.15385, 1e-3), "{}", a.cycle);
    s.step(&mut a, &ground, w, DT, 0.0, true);
    assert_eq!((name(&a), a.main), ("idle_ar2", Some(20)));
    assert!(close(a.cycle, 0.015 / 0.9667, 1e-4));
}

#[test]
fn walking_off_a_ledge_keeps_running_and_dropping_straight_shows_idle() {
    // Cases 14-15.
    let (mut s, mut a) = (Hl2PlayerAnim::default(), animator());
    let w = Some("hl2dm:weapon_pistol");
    let run = Inputs { on_ground: true, velocity: Vec2::new(190.0, 0.0), ..default() };
    s.step(&mut a, &run, w, DT, 0.0, true);
    let air = Inputs { velocity: Vec2::new(190.0, 0.0), vertical: -50.0, ..default() };
    for _ in 0..10 {
        s.step(&mut a, &air, w, DT, 0.0, true);
        assert_eq!(name(&a), "run_pistol");
    }
    let (mut s, mut a) = (Hl2PlayerAnim::default(), animator());
    s.step(&mut a, &Inputs { vertical: -50.0, ..default() }, w, DT, 0.0, true);
    assert_eq!(name(&a), "idle_pistol");
}

#[test]
fn every_change_restarts_at_cycle_zero() {
    // Cases 16-17.
    let (mut s, mut a) = (Hl2PlayerAnim::default(), animator());
    let run = Inputs { on_ground: true, velocity: Vec2::new(190.0, 0.0), ..default() };
    for _ in 0..20 {
        s.step(&mut a, &run, Some("hl2dm:weapon_pistol"), DT, 0.0, true);
    }
    assert!(a.cycle > 0.3);
    s.step(&mut a, &run, Some("hl2dm:weapon_smg1"), DT, 0.0, true);
    assert_eq!(name(&a), "run_smg1");
    assert!(a.cycle < 0.03, "{}", a.cycle);
    for _ in 0..20 {
        s.step(&mut a, &run, Some("hl2dm:weapon_pistol"), DT, 0.0, true);
    }
    s.step(&mut a, &Inputs { on_ground: true, ..default() }, Some("hl2dm:weapon_pistol"), DT, 0.0, true);
    assert_eq!(name(&a), "idle_pistol");
    assert!(close(a.cycle, 0.015 / 0.9667, 1e-4));
}

#[test]
fn no_weapon_shows_the_reference_pose_and_frozen_players_stand() {
    // Cases 18-19.
    let (mut s, mut a) = (Hl2PlayerAnim::default(), animator());
    s.step(&mut a, &Inputs { on_ground: true, ..default() }, None, DT, 0.0, true);
    assert_eq!(a.main, Some(0));
    let (mut s, mut a) = (Hl2PlayerAnim::default(), animator());
    let frozen = Inputs { on_ground: true, frozen: true, jumped: true, velocity: Vec2::new(190.0, 0.0), ..default() };
    s.step(&mut a, &frozen, Some("hl2dm:weapon_pistol"), DT, 0.0, true);
    assert_eq!(name(&a), "idle_pistol");
    assert_eq!(s.ideal, Some(Ideal::Idle));
}

#[test]
fn fire_gestures_play_once_and_go() {
    // Case 20: range_pistol, 0.05625 a tick, weight 0 after the 18th
    // advance, removed on the 19th.
    let (mut s, mut a, i) = standing();
    let w = Some("hl2dm:weapon_pistol");
    s.step(&mut a, &Inputs { fired: true, ..i }, w, DT, 0.0, true);
    let layer = a.layers[0].unwrap();
    assert_eq!((layer.sequence, layer.weight), (77, 1.0));
    assert!(close(layer.cycle, 0.05625, 1e-4));
    for _ in 0..16 {
        s.step(&mut a, &i, w, DT, 0.0, true);
    }
    assert_eq!(a.layers[0].unwrap().weight, 1.0);
    s.step(&mut a, &i, w, DT, 0.0, true);
    assert_eq!(a.layers[0].unwrap().weight, 0.0, "the 18th advance ends it");
    s.step(&mut a, &i, w, DT, 0.0, true);
    assert!(a.layers.is_empty(), "removed on the 19th");
}

#[test]
fn full_auto_restarts_its_gesture() {
    // Case 21: one range_smg1 layer, restarted each shot (every 5 ticks).
    let (mut s, mut a, i) = standing();
    let w = Some("hl2dm:weapon_smg1");
    let mut most = 0.0f32;
    for t in 0..20 {
        s.step(&mut a, &Inputs { fired: t % 5 == 0, ..i }, w, DT, 0.0, true);
        assert_eq!(a.layers.len(), 1);
        most = most.max(a.layers[0].unwrap().cycle);
    }
    assert!(close(most, 0.5625, 1e-4), "{most}");
}

#[test]
fn some_weapons_ask_for_no_gesture() {
    // Cases 22-23.
    for w in ["hl2dm:weapon_crossbow", "hl2dm:weapon_physcannon"] {
        let (mut s, mut a, i) = standing();
        s.step(&mut a, &Inputs { fired: true, ..i }, Some(w), DT, 0.0, true);
        assert!(a.layers.is_empty(), "{w}");
    }
    let (mut s, mut a, i) = standing();
    s.step(&mut a, &Inputs { reloaded: true, ..i }, Some("hl2dm:weapon_smg1"), DT, 0.0, true);
    assert_eq!(a.layers[0].unwrap().sequence, 99);
    assert!(close(1.0 / a.set.cycle_rate(99, &a.params), 2.1667, 1e-3));
    for w in ["hl2dm:weapon_shotgun", "hl2dm:weapon_crossbow"] {
        let (mut s, mut a, i) = standing();
        s.step(&mut a, &Inputs { reloaded: true, reload_part: "_start", ..i }, Some(w), DT, 0.0, true);
        s.step(&mut a, &Inputs { reloaded: true, ..i }, Some(w), DT, 0.0, true);
        assert!(a.layers.is_empty(), "{w}");
    }
    assert_eq!(Kind::of("hl2dm:weapon_357"), Kind::Pistol);
    assert_eq!(Kind::of("hl2dm:weapon_stunstick"), Kind::Melee);
}

#[test]
fn dying_clears_the_layers() {
    // Case 24.
    use source_engine::player_anim::PlayerAnimRules;
    let (mut s, mut a, i) = standing();
    s.step(&mut a, &Inputs { fired: true, reloaded: true, ..i }, Some("hl2dm:weapon_ar2"), DT, 0.0, true);
    assert_eq!(s.gestures.len(), 2);
    s.dead();
    assert!(s.gestures.is_empty());
}

#[test]
fn the_feet_stay_planted_to_45_degrees() {
    // Cases 25-26.
    let (mut s, mut a, i) = standing();
    let w = Some("hl2dm:weapon_pistol");
    s.step(&mut a, &Inputs { eye_yaw: 30.0, ..i }, w, DT, 1.0, true);
    assert_eq!((param(&a, "aim_yaw").round(), s.feet_yaw), (30.0, 0.0));
    let (mut s, mut a, i) = standing();
    let mut torso = Vec::new();
    for t in 0..4 {
        s.step(&mut a, &Inputs { eye_yaw: 50.0, ..i }, w, DT, 1.0 + t as f64 * 0.015, true);
        torso.push(s.torso);
    }
    assert!(close(torso[0], 50.0, 1e-3) && close(param(&a, "aim_yaw"), 34.538, 0.01), "{torso:?}");
    assert!(close(torso[1], 44.6, 1e-3) && close(torso[2], 39.248, 1e-3) && close(torso[3], 34.538, 1e-3), "{torso:?}");
}

#[test]
fn the_feet_face_the_view_after_3_s() {
    // Case 27.
    let (mut s, mut a, i) = standing();
    let w = Some("hl2dm:weapon_pistol");
    let at = Inputs { eye_yaw: 30.0, ..i };
    s.step(&mut a, &at, w, DT, 1.0, true);
    s.step(&mut a, &at, w, DT, 3.9, true);
    assert_eq!(s.feet_yaw, 0.0);
    s.step(&mut a, &at, w, DT, 4.1, true);
    assert_eq!((s.goal_yaw, s.feet_yaw), (30.0, 0.0));
    s.step(&mut a, &at, w, DT, 4.115, true);
    assert!(close(s.torso, 26.4, 1e-3), "{}", s.torso);
    s.step(&mut a, &at, w, DT, 4.13, true);
    assert!(close(s.torso, 23.232, 1e-3), "{}", s.torso);
}

#[test]
fn moving_feet_follow_the_eyes() {
    // Case 28.
    let (mut s, mut a, i) = standing();
    s.step(&mut a, &Inputs { eye_yaw: 70.0, velocity: Vec2::new(1.5, 0.0), ..i }, Some("hl2dm:weapon_pistol"), DT, 1.0, true);
    assert_eq!((s.feet_yaw, s.torso), (70.0, 0.0));
}

#[test]
fn aim_is_clamped_by_code_and_pose_range() {
    // Cases 29-30.
    let (mut s, mut a, i) = standing();
    let w = Some("hl2dm:weapon_pistol");
    s.step(&mut a, &Inputs { eye_yaw: 80.0, ..i }, w, DT, 1.0, true);
    assert!(close(param(&a, "aim_yaw"), 52.5174, 1e-3));
    s.step(&mut a, &Inputs { eye_pitch: 60.0, ..i }, w, DT, 1.0, true);
    assert!(close(param(&a, "aim_pitch"), 49.9848, 1e-3));
    s.step(&mut a, &Inputs { eye_pitch: -85.0, ..i }, w, DT, 1.0, true);
    assert!(close(param(&a, "aim_pitch"), -80.863, 1e-3));
}

#[test]
fn the_gait_settles_toward_the_body_when_still() {
    // Case 31.
    let mut s = Hl2PlayerAnim::default();
    let move_yaw = s.gait(Vec2::ZERO, 90.0, 0.015);
    assert!(close(s.gait_yaw, 5.4, 1e-4) && close(move_yaw, -84.6, 1e-4));
}

#[test]
fn the_hitbox_yaw_lags_the_feet_by_a_tick() {
    // Case 32.
    use source_engine::player_anim::PlayerAnimRules;
    let (mut s, mut a, i) = standing();
    let w = Some("hl2dm:weapon_pistol");
    s.step(&mut a, &Inputs { eye_yaw: 50.0, ..i }, w, DT, 1.0, true);
    s.step(&mut a, &Inputs { eye_yaw: 50.0, ..i }, w, DT, 1.015, true);
    assert!(close(s.feet_yaw, 5.4, 1e-4));
    assert_eq!(s.feet_yaw(), 0.0, "tick N draws and traces the old yaw");
    s.step(&mut a, &Inputs { eye_yaw: 50.0, ..i }, w, DT, 1.03, true);
    assert!(close(s.feet_yaw(), 5.4, 1e-4));
}

#[test]
fn heads_turn_toward_a_near_player() {
    // Case 33: 30 degrees left, 1.3 a frame at 0.01 s, there on the 24th.
    let mut head = Head::default();
    let eye = Vec3::ZERO;
    let target = look::forward(30.0) * 100.0 * 0.0254;
    let mut frames = 0;
    while head.yaw < 30.0 - 1e-4 && frames < 100 {
        look::turn_head(&mut head, eye, target, 0.0, 0.01);
        frames += 1;
        if frames == 1 {
            assert!(close(head.yaw, 1.3, 1e-4));
        }
    }
    assert_eq!(frames, 24);
    let mut head = Head::default();
    for _ in 0..100 {
        look::turn_head(&mut head, eye, look::forward(60.0), 0.0, 0.01);
    }
    assert!(close(head.yaw, 44.8187, 1e-3));
}

#[test]
fn bodies_on_the_harness_run_and_jump() {
    use chimera_world::{
        greybox::{self, GreyboxMapPlugin},
        harness::Sim,
        map::{MapCharacterModel, MapData, MapModel, MapSimPlugin},
    };
    use hl2dm::{game::movement, harness::Hl2Sim};
    let body = MapCharacterModel {
        game: Some(hl2dm::game::GAME),
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
    // The greybox, and a map whose kit carries the body (as an HL2:DM
    // map's carries the 19).
    let mut map = MapData::default();
    map.kit.characters.push(body);
    let mut sim = Sim::new(Hl2Sim, (GreyboxMapPlugin, MapSimPlugin::new(map)));
    sim.set_tick_interval(hl2dm::game::TICK_INTERVAL);
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.seconds(1.0);
    let main = |sim: &mut Sim| {
        let a = sim.app.world().get::<Animator>(p).unwrap();
        a.set.sequences[a.main.unwrap()].name.clone()
    };
    assert!(sim.app.world().get::<Hl2PlayerAnim>(p).is_some());
    assert!(main(&mut sim).starts_with("idle_"), "{}", main(&mut sim));
    sim.intent(p).move_axis = Vec2::new(0.0, 1.0);
    sim.seconds(0.5);
    assert!(main(&mut sim).starts_with("run_"), "{}", main(&mut sim));
    assert!(sim.app.world().get::<Animator>(p).unwrap().autoplay);
}

#[test]
fn a_blink_shuts_in_32_ms_and_opens_over_the_rest() {
    // Case 35: 0, 0.76278, 0.31821, 0.07764, 0 at 0, 50, 100, 150, 200 ms
    // after the flip (time left 0.2 .. 0).
    use hl2dm::game::look::blink_weight;
    for (t, want) in [(0.0f32, 0.0f32), (0.05, 0.76278), (0.1, 0.31821), (0.15, 0.07764), (0.2, 0.0)] {
        assert!(close(blink_weight(0.2 - t), want, 1e-4), "{t}: {}", blink_weight(0.2 - t));
    }
    let peak = (0..200).map(|ms| (ms, blink_weight(0.2 - ms as f32 / 1000.0))).max_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
    assert!((31..=33).contains(&peak.0) && peak.1 > 0.99, "{peak:?}");
}
