//! TF2's player animation on the user's install's class models
//! (specs/tf2/player_animation.md test cases 20-36): which sequence a
//! name or an activity finds in a merged model (1a: the include's over the
//! class model's forward-declared stubs, the user include's `user_ref`
//! over the workshop's, hidden sequences by activity, weighted picks); the
//! Heavy's Minigun at rest raised and spun level, its aim when spun, the
//! spin-up's and spin-down's layers over time (6); the spin gesture's own
//! sequence moving nothing (5a); the foot locks under an aim matrix (4a);
//! every class's rest stance (3); a Scout spinning the Minigun keeps his
//! normal set (open question 13, ours). Passes with a note without the
//! install.

use std::sync::Arc;

use bevy::prelude::*;
use chimera_world::map::anim::{AnimSet, Animator, BonePose, model_space};
use source_engine::{
    content::{anim, material::MaterialLoader},
    player_anim::Inputs,
};
use tf2::game::player_anim::Tf2PlayerAnim;

use super::maps::install_mount;

/// Each class's merged model, by its file's name (`demo` for the
/// Demoman).
fn sets(classes: &[&str]) -> Option<Vec<Arc<AnimSet>>> {
    let mount = install_mount()?;
    let materials = MaterialLoader::guest(&mount, 0, 0).lean(true);
    Some(
        classes
            .iter()
            .map(|c| {
                let path = format!("models/player/{c}.mdl");
                Arc::new(anim::load(&|p| materials.read(p), &path).unwrap_or_else(|e| panic!("{path}: {e}")))
            })
            .collect(),
    )
}

fn seq(set: &AnimSet, name: &str) -> usize {
    set.sequence(name).unwrap_or_else(|| panic!("no {name}"))
}

fn bone(set: &AnimSet, name: &str) -> usize {
    set.bone_names.iter().position(|b| b == name).unwrap_or_else(|| panic!("no bone {name}"))
}

/// `weapon_bone` in model space: origin, barrels (+Z), elevation
/// (degrees), muzzle.
fn gun(set: &AnimSet, pose: &[BonePose]) -> (Vec3, Vec3, f32, Vec3) {
    let (q, p) = model_space(pose, &set.parents)[bone(set, "weapon_bone")];
    let barrels = q * Vec3::Z;
    let elevation = barrels.z.asin().to_degrees();
    (p, barrels, elevation, p + q * Vec3::new(0.0, -7.76, 57.76))
}

/// A Heavy holding the Minigun stepped at 100 Hz for `seconds` with `i`,
/// spun from the start or not; returns the animator and state.
fn heavy_run(set: &Arc<AnimSet>, steps: &[(f32, bool, Inputs)]) -> (Animator, Tf2PlayerAnim, f64) {
    let mut a = Animator::new(set.clone());
    let mut s = Tf2PlayerAnim::default();
    s.class = Some("heavy".into());
    let mut t = 0.0f64;
    for (seconds, spun, i) in steps {
        s.spun = *spun;
        let n = (seconds / 0.01).round() as usize;
        for _ in 0..n {
            t += 0.01;
            s.step(&mut a, i, Some("tf2:tf_weapon_minigun"), 0.01, t);
        }
    }
    (a, s, t)
}

fn still() -> Inputs {
    Inputs { on_ground: true, ..default() }
}

/// Tests 20, 21, 24: names and activities land on the include's
/// sequences in the stubs' slots; the user include's `user_ref` wins.
#[test]
fn names_and_activities_find_the_includes_sequences_20_21_24() {
    let Some(sets) = sets(&["heavy"]) else { return };
    let set = &sets[0];
    let frames = |s: usize| set.animations[set.sequences[s].anims[0]].frames;
    let stand = seq(set, "stand_PRIMARY");
    assert_eq!(frames(stand), 61);
    let aim = seq(set, "PRIMARY_aimmatrix_deployed");
    assert_eq!(set.sequences[aim].grid, (3, 4));
    assert!(set.sequences[aim].anims.iter().all(|&a| set.animations[a].frames == 1));
    assert_eq!(frames(seq(set, "attackStand_PRIMARY_spoolup")), 16);
    // In the stubs' slots: heavy.mdl's own 259 sequences come first.
    for s in [stand, aim, seq(set, "attackStand_PRIMARY_spoolup")] {
        assert!(s < 259, "{} at {s}", set.sequences[s].name);
        assert!(!set.sequences[s].bone_weights.is_empty());
    }
    assert_eq!(set.activity("ACT_MP_STAND_PRIMARY"), Some(stand), "21");
    // 24: the user include (model 1) over the workshop's.
    let user = seq(set, "user_ref");
    assert_eq!(set.animations[set.sequences[user].anims[0]].base, 1);
}

/// Tests 25, 26: the Engineer's reload loops 2 : 1; the Heavy's hidden
/// crouch reload found by its activity.
#[test]
fn weighted_and_hidden_activities_25_26() {
    let Some(sets) = sets(&["engineer", "heavy"]) else { return };
    let set = &sets[0];
    let a = seq(set, "a_PRIMARY_reload_loop");
    let mut x = 12345u32;
    let n = 30_000;
    let mut hits = 0;
    for _ in 0..n {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        let roll = (x >> 8) as f32 / (1u32 << 24) as f32;
        if set.select_activity("ACT_MP_RELOAD_STAND_PRIMARY_LOOP", None, roll) == Some(a) {
            hits += 1;
        }
    }
    let share = hits as f32 / n as f32;
    assert!((share - 2.0 / 3.0).abs() < 0.01, "a_ {share}");
    let heavy = &sets[1];
    let found = heavy.activity("ACT_MP_RELOAD_CROUCH_SECONDARY").expect("26");
    assert!(heavy.sequences[found].name.eq_ignore_ascii_case("SECONDARY_crouch_reload"), "{}", heavy.sequences[found].name);
}

/// Tests 27, 28, 29: the Minigun raised at rest, level spun, and level
/// while firing.
#[test]
fn the_heavys_minigun_raised_at_rest_level_spun_27_28_29() {
    let Some(sets) = sets(&["heavy"]) else { return };
    let set = &sets[0];
    let (a, _, t) = heavy_run(set, &[(0.01, false, still())]);
    assert_eq!(a.main, Some(seq(set, "stand_PRIMARY")));
    let mut frame0 = a.clone();
    frame0.cycle = 0.0;
    let (p, barrels, elevation, muzzle) = gun(set, &frame0.pose(t));
    eprintln!("27: {p} {barrels} {elevation} {muzzle}");
    assert!(p.distance(Vec3::new(-5.1, -14.7, 43.8)) < 0.2, "27: weapon_bone {p}");
    assert!(barrels.distance(Vec3::new(0.81, 0.18, 0.56)) < 0.02, "27: barrels {barrels}");
    assert!(muzzle.distance(Vec3::new(46.0, -6.7, 70.5)) < 0.5, "27: muzzle {muzzle}");

    let (a, s, t) = heavy_run(set, &[(0.01, false, still()), (0.52, true, still())]);
    assert_eq!(a.main, Some(seq(set, "stand_deployed_PRIMARY")), "28");
    assert!(s.gestures.is_empty(), "28: the spin-up done at 0.5 s: {:?}", s.gestures);
    let mut frame0 = a.clone();
    frame0.cycle = 0.0;
    frame0.fading.clear();
    let (p, barrels, elevation, muzzle) = gun(set, &frame0.pose(t));
    eprintln!("28: {p} {barrels} {elevation} {muzzle}");
    assert!(p.distance(Vec3::new(-8.6, -13.7, 41.4)) < 0.2, "28: weapon_bone {p}");
    assert!(barrels.distance(Vec3::new(1.0, 0.02, 0.02)) < 0.02, "28: barrels {barrels}");
    assert!(muzzle.distance(Vec3::new(49.4, -14.2, 35.2)) < 0.5, "28: muzzle {muzzle}");

    // 29: firing for 2 s, sampled every 50 ms.
    let level = p;
    let mut a = a;
    let mut s = s;
    let mut t = t;
    for k in 0..200 {
        t += 0.01;
        s.step(&mut a, &Inputs { fired: true, ..still() }, Some("tf2:tf_weapon_minigun"), 0.01, t);
        if k % 5 == 0 {
            let (p, _, elevation, _) = gun(set, &a.pose(t));
            assert!((-0.5..=3.5).contains(&elevation), "29: {elevation} at {k}");
            assert!(p.distance(level) < 2.5, "29: weapon_bone {p} at {k}");
        }
    }
}

/// Test 30: spun, the aim follows the view more closely than at rest.
#[test]
fn the_spun_aim_30() {
    let Some(sets) = sets(&["heavy"]) else { return };
    let set = &sets[0];
    for (pitch, spun, want) in [(45.0, true, 48.0), (-45.0, true, -25.0), (90.0, true, 78.0), (45.0, false, 39.0), (-45.0, false, 25.0), (90.0, false, 47.0)] {
        let i = Inputs { eye_pitch: -pitch, ..still() };
        let (mut a, _, t) = heavy_run(set, &[(0.01, false, i), (1.0, spun, i)]);
        a.cycle = 0.0;
        a.fading.clear();
        a.layers.clear();
        let (_, _, elevation, _) = gun(set, &a.pose(t));
        eprintln!("30: pitch {pitch} spun {spun}: {elevation}");
        assert!((elevation - want).abs() < 2.0, "30: body_pitch {pitch} spun {spun}: {elevation}, want {want}");
    }
}

/// Tests 31, 32: the spin-up's layer over the wind-up, the spin-down's
/// over the wind-down.
#[test]
fn the_spin_layers_over_time_31_32() {
    let Some(sets) = sets(&["heavy"]) else { return };
    let set = &sets[0];
    let up = seq(set, "attackStand_PRIMARY_spoolup");
    for (time, want) in [(0.1, 19.0), (0.2, 5.0), (0.3, 2.0), (0.49, 8.0)] {
        let (a, s, t) = heavy_run(set, &[(1.0, false, still()), (time, true, still())]);
        assert!(s.gestures.iter().any(|g| g.sequence == up), "31: the spin-up at {time}");
        let (_, _, elevation, _) = gun(set, &a.pose(t));
        eprintln!("31: {time}: {elevation}");
        assert!((elevation - want).abs() < 2.0, "31: at {time}: {elevation}, want {want}");
    }
    let down = seq(set, "attackStand_PRIMARY_spooldown");
    for (time, want) in [(0.2, 9.0), (0.4, 20.0), (0.6, 33.0), (0.8, 35.0)] {
        let (a, s, t) = heavy_run(set, &[(1.0, false, still()), (1.0, true, still()), (time, false, still())]);
        assert_eq!(a.main, Some(seq(set, "stand_PRIMARY")), "32");
        assert!(s.gestures.iter().any(|g| g.sequence == down), "32: the spin-down at {time}");
        let (_, _, elevation, _) = gun(set, &a.pose(t));
        eprintln!("32: {time}: {elevation} (want {want})");
        // 3 degrees: the layer's bone weights (0.5 to 1 in the file, 1 in
        // the spec's timeline) let some of the main sequence through.
        assert!((elevation - want).abs() < 3.0, "32: at {time}: {elevation}, want {want}");
    }
    let (_, s, _) = heavy_run(set, &[(1.0, true, still()), (1.01, false, still())]);
    assert!(s.gestures.is_empty(), "32: the slot free at 1.0 s: {:?}", s.gestures);
}

/// Test 33: the spin gesture's own sequence moves nothing.
#[test]
fn the_spin_gestures_own_sequence_moves_nothing_33() {
    let Some(sets) = sets(&["heavy"]) else { return };
    let set = &sets[0];
    let s = seq(set, "attackStand_PRIMARY_spoolup");
    let up = &set.sequences[s];
    assert!(up.bone_weights.iter().all(|w| *w == 0.0), "33: every bone weight 0");
    // Its layer is what moves the gun: full-body and non-additive.
    let layer = &set.sequences[up.autolayers[0].sequence];
    // Every bone of the animation model weighted (0.5 to 1 in the file;
    // the spec says 1).
    let weighted = layer.bone_weights.iter().filter(|w| **w >= 0.5).count();
    assert!(!layer.delta && weighted >= 77, "33: {}: {weighted}", layer.name);
    assert!(up.autolayers[0].spline, "33: the splined ramp");
}

/// Test 34: the foot locks keep the Soldier's feet under his aim matrix.
#[test]
fn foot_locks_keep_the_feet_34() {
    let Some(sets) = sets(&["soldier"]) else { return };
    let set = &sets[0];
    let mut params = set.default_params();
    let pitch = set.param("body_pitch").unwrap();
    params[pitch] = set.params[pitch].encode(45.0);
    let stand = seq(set, "stand_PRIMARY");
    let feet = [bone(set, "bip_foot_r"), bone(set, "bip_foot_l")];
    let mut base = set.defaults.clone();
    let mut plain = set.params.iter().map(|p| p.default_value()).collect::<Vec<_>>();
    plain[pitch] = set.params[pitch].encode(0.0);
    set.accumulate(&mut base, stand, 0.0, 1.0, &plain);
    let base = model_space(&base, &set.parents);
    let (mut free, mut locked) = (set.defaults.clone(), set.defaults.clone());
    set.accumulate(&mut free, stand, 0.0, 1.0, &params);
    set.accumulate_locked(&mut locked, stand, 0.0, 1.0, &params);
    let (free, locked) = (model_space(&free, &set.parents), model_space(&locked, &set.parents));
    for f in feet {
        let moved = free[f].1.distance(base[f].1);
        let kept = locked[f].1.distance(base[f].1);
        eprintln!("34: {}: free {moved}, locked {kept}", set.bone_names[f]);
        assert!(moved > 5.0, "34: without the locks the aim moves the feet ({moved})");
        assert!(kept < 0.1, "34: {} moved {kept}", set.bone_names[f]);
    }
}

/// Test 35 (open question 12): every class's rest stance, `stand_<KIND>`
/// frame 0's feet (x forward, y left) as the spec's table.
#[test]
fn rest_stances_35() {
    #[rustfmt::skip]
    let table: [(&str, &str, [f32; 4]); 22] = [
        ("scout", "stand_PRIMARY", [-19.5, -5.0, 20.6, 10.9]),
        ("scout", "stand_SECONDARY", [-19.3, -6.1, 20.8, 9.7]),
        ("scout", "stand_MELEE", [-19.8, -5.1, 20.3, 10.8]),
        ("soldier", "stand_PRIMARY", [-6.5, -8.9, 13.7, 10.5]),
        ("soldier", "stand_SECONDARY", [-7.6, -10.6, 12.6, 8.7]),
        ("soldier", "stand_MELEE", [-6.5, -11.0, 13.7, 8.3]),
        ("pyro", "stand_PRIMARY", [-5.4, -10.7, 4.0, 9.4]),
        ("demo", "stand_PRIMARY", [-10.7, -9.9, 12.0, 8.0]),
        ("demo", "stand_MELEE", [-10.5, -8.0, 12.2, 10.5]),
        ("heavy", "stand_PRIMARY", [-7.1, -7.7, 6.7, 8.2]),
        ("heavy", "stand_SECONDARY", [-8.9, -6.2, 4.3, 7.3]),
        ("heavy", "stand_MELEE", [-7.5, -9.0, 7.8, 7.2]),
        ("heavy", "stand_deployed_PRIMARY", [-5.8, -7.5, 12.2, 8.4]),
        ("engineer", "stand_PRIMARY", [-6.0, -7.1, 7.8, 6.0]),
        ("engineer", "stand_MELEE", [-6.2, -7.7, 7.6, 5.4]),
        ("medic", "stand_PRIMARY", [-13.1, -7.5, 7.3, 5.1]),
        ("medic", "stand_SECONDARY", [-14.9, -8.9, 7.5, 7.6]),
        ("medic", "stand_MELEE", [-15.6, -9.7, 9.9, 8.9]),
        ("sniper", "stand_PRIMARY", [-9.9, -5.1, 5.6, 9.0]),
        ("spy", "stand_SECONDARY", [-10.6, -8.9, 5.9, 6.6]),
        ("spy", "stand_MELEE", [-10.0, -8.9, 6.5, 6.6]),
        ("pyro", "stand_SECONDARY", [-5.4, -10.7, 4.0, 9.4]),
    ];
    let classes = ["scout", "soldier", "pyro", "demo", "heavy", "engineer", "medic", "sniper", "spy"];
    let Some(sets) = sets(&classes) else { return };
    let mut wrong = Vec::new();
    for (class, name, [rx, ry, lx, ly]) in table {
        let set = &sets[classes.iter().position(|c| *c == class).unwrap()];
        // As drawn: the main sequence and its aim autolayers at a level view.
        let mut a = Animator::new(set.clone());
        a.ik_locks = true;
        a.main = Some(seq(set, name));
        for p in ["body_pitch", "body_yaw", "move_x", "move_y", "r_hand_grip", "r_arm"] {
            if let Some(i) = set.param(p) {
                a.set_param(i, 0.0);
            }
        }
        let pose = model_space(&a.pose(0.0), &set.parents);
        let r = pose[bone(set, "bip_foot_r")].1;
        let l = pose[bone(set, "bip_foot_l")].1;
        let got = [r.x, r.y, l.x, l.y];
        eprintln!("35: {class} {name}: R ({:.1}, {:.1}) L ({:.1}, {:.1})", r.x, r.y, l.x, l.y);
        if got.iter().zip([rx, ry, lx, ly]).any(|(g, w)| (g - w).abs() > 0.2) {
            wrong.push(format!("{class} {name}: {got:?}"));
        }
    }
    assert!(wrong.is_empty(), "35: {wrong:#?}");
}

/// Test 36 (open question 13, ours): a Scout spinning the Minigun keeps
/// his primary stand (the game shows the reference pose).
#[test]
fn a_scout_spinning_the_minigun_keeps_his_stand_36() {
    let Some(sets) = sets(&["scout"]) else { return };
    let set = &sets[0];
    let mut a = Animator::new(set.clone());
    let mut s = Tf2PlayerAnim::default();
    s.class = Some("scout".into());
    s.spun = true;
    s.step(&mut a, &still(), Some("tf2:tf_weapon_minigun"), 0.01, 0.01);
    assert_eq!(a.main, Some(seq(set, "stand_PRIMARY")));
}
