//! TF2's player animation on the user's install's class models
//! (specs/tf2/player_animation.md test cases 20-38): which sequence a
//! name or an activity finds in a merged model (1a: the include's over the
//! class model's forward-declared stubs, the user include's `user_ref`
//! over the workshop's, hidden sequences by activity, weighted picks); the
//! Heavy's Minigun at rest raised and spun level, its aim when spun, the
//! spin-up's and spin-down's layers over time (6); the spin gesture's own
//! sequence moving nothing (5a); the foot locks under an aim matrix and
//! the left hand's IK rule onto the Minigun, as read and as held (4a);
//! every class's rest stance (3); a Scout spinning the Minigun keeps his
//! normal set (open question 13, ours); every weapon's merged bones on
//! its class's body where the weapon model has them (7); tests 42-52, 56
//! and 60: the six pose parameters at 0 after a spawn, the Engineer's
//! pistol grip, the hand and arm libraries only where a host layers them,
//! the Wrench's tap and mesh axis, still idle feet, the feet yaw's
//! apparent spread with the knees bent toward the include's knee
//! directions, the foot-locked (and the Medic's real-time) noise layers.
//! Passes with a note without the install.

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
/// over the wind-down, with the layers' bone weights as the file has them
/// (0.5 to 1; 6, the 2026-11-19 implementation note). Forced to 1 they
/// give the first reading's timeline (9 / 20 / 33 / 35 degrees down), so
/// the weights are the whole difference.
#[test]
fn the_spin_layers_over_time_31_32() {
    let Some(mut sets) = sets(&["heavy", "heavy"]) else { return };
    let mut ones = sets.pop().unwrap();
    let set = &sets[0];
    let up = seq(set, "attackStand_PRIMARY_spoolup");
    for (time, want) in [(0.1, 20.8), (0.2, 5.6), (0.3, 1.8), (0.4, 5.0), (0.49, 7.5)] {
        let (a, s, t) = heavy_run(set, &[(1.0, false, still()), (time, true, still())]);
        assert!(s.gestures.iter().any(|g| g.sequence == up), "31: the spin-up at {time}");
        let (_, _, elevation, _) = gun(set, &a.pose(t));
        eprintln!("31: {time}: {elevation}");
        assert!((elevation - want).abs() < 1.0, "31: at {time}: {elevation}, want {want}");
    }
    let down = seq(set, "attackStand_PRIMARY_spooldown");
    for name in ["layer_attackStand_PRIMARY_spooldown", "layer_attackStand_PRIMARY_spoolup"] {
        let s = seq(&ones, name);
        let set = Arc::get_mut(&mut ones).expect("a fresh set");
        set.sequences[s].bone_weights.iter_mut().filter(|w| **w > 0.0).for_each(|w| *w = 1.0);
    }
    for (time, want, first) in [(0.2, 6.6, 9.0), (0.4, 18.2, 20.0), (0.6, 32.8, 33.0), (0.7, 36.9, 36.0), (0.8, 35.6, 35.0), (0.9, 33.6, 34.0)] {
        let (a, s, t) = heavy_run(set, &[(1.0, false, still()), (1.0, true, still()), (time, false, still())]);
        assert_eq!(a.main, Some(seq(set, "stand_PRIMARY")), "32");
        assert!(s.gestures.iter().any(|g| g.sequence == down), "32: the spin-down at {time}");
        let (_, _, elevation, _) = gun(set, &a.pose(t));
        let (a1, _, t1) = heavy_run(&ones, &[(1.0, false, still()), (1.0, true, still()), (time, false, still())]);
        let (_, _, at_one, _) = gun(&ones, &a1.pose(t1));
        eprintln!("32: {time}: {elevation} (weights 1: {at_one}, the first reading {first})");
        assert!((elevation - want).abs() < 1.0, "32: at {time}: {elevation}, want {want}");
        assert!((at_one - first).abs() < 1.5, "32: weights 1 at {time}: {at_one}, the first reading {first}");
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
    // Every bone of the animation model weighted (0.5 to 1, as the file
    // has them: 6).
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

/// The left hand's place in `weapon_bone`'s frame.
fn left_hand(set: &AnimSet, pose: &[BonePose]) -> (Quat, Vec3) {
    let g = model_space(pose, &set.parents);
    let (wq, wp) = g[bone(set, "weapon_bone")];
    let (hq, hp) = g[bone(set, "bip_hand_l")];
    (wq.inverse() * hq, wq.inverse() * (hp - wp))
}

/// Test 37 (4a): the aim cells' IK rule as read (the layout written in
/// 4a): every Heavy `PRIMARY` idle cell holds `lhand` on `weapon_bone`
/// at about (13.2, 4.0, 27.4), where the centre pose has the hand.
#[test]
fn the_aim_cells_left_hand_rule_as_read_37() {
    let Some(sets) = sets(&["heavy"]) else { return };
    let set = &sets[0];
    let lhand = set.ik_chains.iter().position(|c| c.name == "lhand").expect("lhand");
    let aim = &set.sequences[seq(set, "PRIMARY_aimmatrix_idle")];
    let wb = bone(set, "weapon_bone");
    for &a in &aim.anims {
        let t = set.animations[a].ik_targets.iter().find(|t| t.chain == lhand);
        let t = t.unwrap_or_else(|| panic!("37: {} has no lhand rule", set.animations[a].name));
        assert_eq!(t.bone, wb, "37: relative to weapon_bone");
        assert!(t.frames[0].1.distance(Vec3::new(13.2, 3.96, 27.42)) < 0.05, "37: {}: {}", set.animations[a].name, t.frames[0].1);
    }
    // The centre pose (identity cell) has the hand where the rule says.
    let mut a = Animator::new(set.clone());
    a.main = Some(seq(set, "stand_PRIMARY"));
    for p in ["body_pitch", "body_yaw", "move_x", "move_y"] {
        if let Some(i) = set.param(p) {
            a.set_param(i, 0.0);
        }
    }
    let (q, p) = left_hand(set, &a.pose(0.0));
    let (rq, rp) = set.animations[aim.anims[0]].ik_targets[0].frames[0];
    eprintln!("37: centre {p} {q}, rule {rp} {rq}");
    assert!(p.distance(rp) < 0.5, "37: place {p} against {rp}");
    assert!(q.angle_between(rq).to_degrees() < 3.0, "37: turn {} degrees off", q.angle_between(rq).to_degrees());
}

/// Test 38 (4a): off centre the rule keeps the left hand on the Minigun;
/// without it the blended cells move it.
#[test]
fn the_left_hand_stays_on_the_minigun_38() {
    let Some(sets) = sets(&["heavy"]) else { return };
    let set = &sets[0];
    let lhand = set.ik_chains.iter().position(|c| c.name == "lhand").unwrap();
    for spun in [false, true] {
        for (pitch, yaw) in [(30.0, 20.0), (-30.0, -40.0), (70.0, 10.0)] {
            let i = Inputs { eye_pitch: -pitch, eye_yaw: yaw, ..still() };
            let (mut a, _, t) = heavy_run(set, &[(0.01, false, Inputs { eye_yaw: 0.0, ..i }), (1.0, spun, i)]);
            a.fading.clear();
            a.layers.clear();
            let mut goals = vec![None; set.ik_chains.len()];
            set.rule_goals(a.main.unwrap(), a.cycle, 1.0, &a.params, &mut goals);
            let goal = goals[lhand].expect("38: a goal for the left hand");
            let held = left_hand(set, &a.pose(t));
            a.ik_rules = false;
            let free = left_hand(set, &a.pose(t));
            eprintln!("38: spun {spun} pitch {pitch} yaw {yaw}: held {} free {} goal {}", held.1, free.1, goal.place.1);
            assert!(held.1.distance(goal.place.1) < 0.1, "38: held {} against {}", held.1, goal.place.1);
            assert!(held.0.angle_between(goal.place.0).to_degrees() < 1.0, "38: the hand's turn");
        }
    }
}

/// 7 and 2: every TF2 weapon's bones that bone-merge onto its class's
/// body (by name, but `weapon_bone`) sit where the weapon model has them
/// against its `weapon_bone`, standing, crouched, running and in the air
/// (the body's pose for the weapon's kind keeps the gun whole). The
/// Grenade Launcher animates as a secondary and the Stickybomb Launcher
/// as a primary (their items' animation slots, weapons_more.md 1): with
/// the other's set the Grenade Launcher's `weapon_bone_1`, its barrel,
/// was turned 94 degrees across the Demoman's body.
#[test]
fn held_weapons_merge_whole_on_every_class() {
    let Some(mount) = install_mount() else { return };
    let materials = MaterialLoader::guest(&mount, 0, 0).lean(true);
    let weapons = tf2::game::weapons::tf2_weapons();
    let mut sets: std::collections::HashMap<String, Arc<AnimSet>> = Default::default();
    let mut bad = Vec::new();
    for (gun, arms) in tf2::game::weapons::arms_by_gun() {
        let Some(path) = weapons.weapons.iter().find(|d| d.id == gun).and_then(|d| d.models.world.clone()) else { continue };
        let set = sets
            .entry(arms.clone())
            .or_insert_with(|| Arc::new(anim::load(&|p| materials.read(p), &format!("models/player/{arms}.mdl")).unwrap()))
            .clone();
        let mdl = source_engine::content::props::read_mdl(&materials.read(&path).unwrap()).unwrap();
        let mut rest: Vec<(Quat, Vec3)> = Vec::new();
        for b in &mdl.bones {
            let q = Quat::from_xyzw(b.quaternion.x, b.quaternion.y, b.quaternion.z, b.quaternion.w);
            let p = Vec3::new(b.pos.x, b.pos.y, b.pos.z);
            rest.push(match usize::try_from(b.parent).ok().and_then(|p| rest.get(p)) {
                Some(&(pq, pp)) => (pq * q, pp + pq * p),
                None => (q, p),
            });
        }
        let cw = mdl.bones.iter().position(|b| b.name.eq_ignore_ascii_case("weapon_bone")).expect("weapon_bone");
        let pw = bone(&set, "weapon_bone");
        let moving = Inputs { velocity: Vec2::new(0.0, 200.0), ..still() };
        let air = Inputs { on_ground: false, ..still() };
        let ducked = Inputs { ducked: true, ..still() };
        for (what, i) in [("standing", still()), ("crouched", ducked), ("running", moving), ("in the air", air)] {
            let mut a = Animator::new(set.clone());
            let mut s = Tf2PlayerAnim::default();
            s.class = Some(if arms == "demo" { "demoman".into() } else { arms.clone() });
            let mut t = 0.0;
            for _ in 0..100 {
                t += 0.01;
                s.step(&mut a, &i, Some(gun), 0.01, t);
            }
            let posed = model_space(&a.pose(t), &set.parents);
            for (k, b) in mdl.bones.iter().enumerate() {
                let Some(pi) = set.bone_names.iter().position(|n| n.eq_ignore_ascii_case(&b.name)).filter(|_| k != cw) else { continue };
                let on_gun = (rest[cw].0.inverse() * rest[k].0, rest[cw].0.inverse() * (rest[k].1 - rest[cw].1));
                let on_body = (posed[pw].0.inverse() * posed[pi].0, posed[pw].0.inverse() * (posed[pi].1 - posed[pw].1));
                let (dpos, dang) = (on_gun.1.distance(on_body.1), on_gun.0.angle_between(on_body.0).to_degrees());
                // A hand's grip may slide a little (the Pyro's
                // `weapon_bone_L` 1.1 units running).
                if dpos > 1.5 || dang > 3.0 {
                    bad.push(format!("{gun} ({arms}, {what}): {} off by {dpos:.1} units, {dang:.0} degrees", b.name));
                }
            }
        }
    }
    assert!(bad.is_empty(), "merged bones off the gun:\n{}", bad.join("\n"));
}

/// 7: every held weapon's parts follow bones its class's body has (the
/// arms' own `vm_` bones are no body's: a part merged onto one was never
/// drawn, the Shovel's blade, the Shotgun's pump), and the parts on its
/// first bone are drawn whole.
#[test]
fn held_weapons_parts_follow_body_bones() {
    let Some(mount) = install_mount() else { return };
    let mut materials = MaterialLoader::guest(&mount, 0, 0).lean(true);
    let mut kit = chimera_world::map::GameKit::default();
    let mut warnings = Vec::new();
    tf2::game::maps::add_guns(&mut materials, &|_| true, false, &mut kit, &mut warnings);
    let mut bad = Vec::new();
    for (gun, arms) in tf2::game::weapons::arms_by_gun() {
        // The Fists are the Heavy's arms alone: nothing held.
        let Some(held) = kit.held.iter().find(|h| h.key == gun) else { continue };
        let set = anim::load(&|p| materials.read(p), &format!("models/player/{arms}.mdl")).unwrap();
        let has = |bone: &str| set.bone_names.iter().any(|n| n.eq_ignore_ascii_case(bone));
        // A body lacking a merged part's bone holds the model whole
        // (`render::bodies::attach_held`: the Revolver's `weapon_bone_4` on
        // the Spy): only the held bone and the arms' own `vm_` bones are
        // wrong.
        for bone in std::iter::once(&held.bone).chain(held.merged.iter().map(|(b, _)| b)) {
            let whole = bone != &held.bone && !bone.to_ascii_lowercase().starts_with("vm_");
            if !has(bone) && !whole {
                let tris: usize = held.merged.iter().filter(|(b, _)| b == bone).flat_map(|(_, m)| &m.meshes).map(|m| m.indices.len() / 3).sum();
                bad.push(format!("{gun}: on {bone} ({tris} triangles), which the {arms} body lacks"));
            }
        }
    }
    assert!(warnings.is_empty() && kit.held.len() > 15, "{warnings:?}");
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// An animator on `name` as drawn (the foot locks on), every pose
/// parameter at 0 but those given (in their own units).
fn posed_at(set: &Arc<AnimSet>, name: &str, cycle: f32, params: &[(&str, f32)], now: f64) -> Vec<BonePose> {
    let mut a = Animator::new(set.clone());
    a.ik_locks = true;
    a.main = Some(seq(set, name));
    a.cycle = cycle;
    for p in 0..set.params.len() {
        a.set_param(p, 0.0);
    }
    for (n, v) in params {
        a.set_param(set.param(n).unwrap_or_else(|| panic!("no {n}")), *v);
    }
    model_space(&a.pose(now), &set.parents)
}

/// Test 42 (4b): the merged model's six pose parameters, and at spawn
/// `r_hand_grip` and `r_arm` at 0 (the first cells), not the range's
/// middle.
#[test]
fn six_pose_parameters_at_zero_after_spawn_42() {
    let classes = ["scout", "soldier", "pyro", "demo", "heavy", "engineer", "medic", "sniper", "spy"];
    let Some(sets) = sets(&classes) else { return };
    for (class, set) in classes.iter().zip(&sets) {
        let mut names: Vec<String> = set.params.iter().map(|p| p.name.to_lowercase()).collect();
        names.sort();
        assert_eq!(names, ["body_pitch", "body_yaw", "move_x", "move_y", "r_arm", "r_hand_grip"], "42: {class}");
        let mut a = Animator::new(set.clone());
        let mut s = Tf2PlayerAnim::default();
        s.step(&mut a, &still(), Some("tf2:tf_weapon_shotgun_primary"), 0.01, 0.01);
        for n in ["r_hand_grip", "r_arm"] {
            let i = set.param(n).unwrap();
            let p = &set.params[i];
            let v = a.params[i] * (p.end - p.start) + p.start;
            assert!(v.abs() < 1e-4, "42: {class} {n} = {v}");
        }
    }
}

/// Tests 43, 44 (4b): the Engineer's pistol grip is `r_handposes_engineer`
/// on `stand_SECONDARY`: level at `r_hand_grip` 0, its second cell (the
/// gun turned 41 degrees up) at 1 or more; the hand bone itself stays.
#[test]
fn the_engineers_pistol_grip_43_44() {
    let Some(sets) = sets(&["engineer"]) else { return };
    let set = &sets[0];
    let (wb, hand) = (bone(set, "weapon_bone"), bone(set, "bip_hand_r"));
    let check = |grip: f32, want: Vec3, axis: Vec3| {
        let pose = posed_at(set, "stand_SECONDARY", 0.0, &[("r_hand_grip", grip)], 0.0);
        let (q, p) = pose[wb];
        eprintln!("43/44: grip {grip}: weapon_bone {p:.2} +Z {:.2}, hand {:.2}", q * Vec3::Z, pose[hand].1);
        assert!(p.distance(want) < 0.1, "grip {grip}: weapon_bone at {p}, not {want}");
        assert!((q * Vec3::Z).distance(axis) < 0.02, "grip {grip}: +Z {}, not {axis}", q * Vec3::Z);
        assert!(pose[hand].1.distance(Vec3::new(8.52, -12.57, 46.21)) < 0.1, "the hand stays: {}", pose[hand].1);
    };
    check(0.0, Vec3::new(12.96, -11.16, 46.01), Vec3::new(0.99, -0.05, -0.16));
    for grip in [1.0, 8.0, 16.0] {
        check(grip, Vec3::new(17.20, -9.91, 48.43), Vec3::new(0.72, -0.22, 0.66));
    }
}

/// Tests 45, 46 (4b): no stock stand carries a hand or arm library, so
/// `r_hand_grip` and `r_arm` change nothing there; the all-class melee
/// stand carries both: the grip moves the right fingers and
/// `weapon_bone`, the arm the right collar, arm, hand and weapon bones,
/// nothing else.
#[test]
fn the_libraries_act_only_where_layered_45_46() {
    let classes = ["scout", "soldier", "pyro", "demo", "heavy", "engineer", "medic", "sniper", "spy"];
    let Some(sets) = sets(&classes) else { return };
    let moved = |a: &[BonePose], b: &[BonePose]| -> Vec<usize> {
        (0..a.len()).filter(|&i| a[i].1.distance(b[i].1) > 0.01 || a[i].0.angle_between(b[i].0) > 0.2f32.to_radians()).collect()
    };
    for (class, set) in classes.iter().zip(&sets) {
        for kind in ["PRIMARY", "SECONDARY", "MELEE"] {
            let name = format!("stand_{kind}");
            if (*class == "engineer" && kind == "SECONDARY") || set.sequence(&name).is_none() {
                continue;
            }
            let rest = posed_at(set, &name, 0.3, &[], 0.0);
            let other = posed_at(set, &name, 0.3, &[("r_hand_grip", 16.0), ("r_arm", 3.0)], 0.0);
            let m = moved(&rest, &other);
            assert!(m.is_empty(), "45: {class} {name}: {:?} moved", m.iter().map(|&i| &set.bone_names[i]).collect::<Vec<_>>());
        }
    }
    let set = &sets[0];
    let name = "stand_MELEE_ALLCLASS";
    let rest = posed_at(set, name, 0.0, &[], 0.0);
    let names = |m: Vec<usize>| -> Vec<String> { m.into_iter().map(|i| set.bone_names[i].clone()).collect() };
    let grip = names(moved(&rest, &posed_at(set, name, 0.0, &[("r_hand_grip", 15.0)], 0.0)));
    let arm = names(moved(&rest, &posed_at(set, name, 0.0, &[("r_arm", 3.0)], 0.0)));
    eprintln!("46: grip moves {grip:?}\n46: arm moves {arm:?}");
    assert!(!grip.is_empty() && !arm.is_empty(), "46: both libraries act");
    let finger = |b: &str| b.ends_with("_r") && ["bip_thumb", "bip_index", "bip_middle", "bip_ring", "bip_pinky"].iter().any(|f| b.starts_with(f));
    // Children of moved bones move with them in model space: what moves
    // is the libraries' bones and what hangs from them.
    let under = |b: &str, roots: &[&str]| {
        let mut i = set.bone_names.iter().position(|n| n == b);
        while let Some(k) = i {
            if roots.contains(&set.bone_names[k].as_str()) {
                return true;
            }
            i = set.parents[k];
        }
        false
    };
    for b in &grip {
        assert!(finger(b) || under(b, &["weapon_bone"]), "46: the grip moved {b}");
    }
    for b in &arm {
        assert!(under(b, &["bip_collar_r", "weapon_bone", "weapon_bone_1", "prp_starter"]), "46: the arm moved {b}");
    }
}

/// Tests 49, 50 (4b): the Engineer's `stand_MELEE` taps the Wrench (its
/// mesh along `weapon_bone` +Y, -4.3 to 24.1): +Y's elevation 17, 50, 5
/// and 43 degrees at frames 0, 12, 24 and 40, the head (24 along +Y)
/// where the spec's table has it.
#[test]
fn the_wrenchs_tap_49_50() {
    let Some(sets) = sets(&["engineer"]) else { return };
    let set = &sets[0];
    let wb = bone(set, "weapon_bone");
    let frames = set.animations[set.sequences[seq(set, "stand_MELEE")].anims[0]].frames as f32;
    for (frame, elevation, head) in [
        (0.0, 17.0, Vec3::new(23.5, 3.7, 45.1)),
        (12.0, 50.0, Vec3::new(22.9, -3.9, 59.5)),
        (24.0, 5.0, Vec3::new(21.6, 6.0, 37.2)),
        (40.0, 43.0, Vec3::new(22.9, -1.2, 58.1)),
    ] {
        let (q, p) = posed_at(set, "stand_MELEE", frame / (frames - 1.0), &[], 0.0)[wb];
        let y = q * Vec3::Y;
        let got = y.z.asin().to_degrees();
        let at = p + y * 24.0;
        eprintln!("49: frame {frame}: +Y {y:.2}, {got:.0} degrees, head {at:.1}");
        assert!((got - elevation).abs() < 2.0, "49: frame {frame}: {got}");
        assert!(at.distance(head) < 0.5, "49: frame {frame}: head {at}");
    }
    let Some(mount) = install_mount() else { return };
    let mut materials = MaterialLoader::guest(&mount, 0, 0).lean(true);
    let mut kit = chimera_world::map::GameKit::default();
    tf2::game::maps::add_guns(&mut materials, &|_| true, false, &mut kit, &mut Vec::new());
    let wrench = kit.held.iter().find(|h| h.key == "tf2:tf_weapon_wrench").expect("the Wrench held");
    assert!(wrench.bone.eq_ignore_ascii_case("weapon_bone"), "{}", wrench.bone);
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for m in &wrench.model.meshes {
        for v in &m.positions {
            lo = lo.min(Vec3::from(*v));
            hi = hi.max(Vec3::from(*v));
        }
    }
    eprintln!("50: the Wrench's mesh from {lo:.1} to {hi:.1}");
    assert!((lo.y + 4.3).abs() < 0.3 && (hi.y - 24.1).abs() < 0.3, "50: along +Y: {lo} .. {hi}");
    assert!(hi.x - lo.x < 10.0 && hi.z - lo.z < 12.0, "50: thin across: {lo} .. {hi}");
}

/// Test 51 (4c 1): over every stand loop the feet stay within 0.3 units
/// of frame 0's.
#[test]
fn idle_feet_stay_still_51() {
    let classes = ["scout", "soldier", "pyro", "demo", "heavy", "engineer", "medic", "sniper", "spy"];
    let Some(sets) = sets(&classes) else { return };
    let mut bad = Vec::new();
    for (class, set) in classes.iter().zip(&sets) {
        let feet = [bone(set, "bip_foot_r"), bone(set, "bip_foot_l")];
        for kind in ["PRIMARY", "SECONDARY", "MELEE"] {
            let name = format!("stand_{kind}");
            let Some(s) = set.sequence(&name) else { continue };
            let frames = set.animations[set.sequences[s].anims[0]].frames.max(2);
            let first = posed_at(set, &name, 0.0, &[], 0.0);
            let mut worst = 0.0f32;
            for f in 0..frames {
                let pose = posed_at(set, &name, f as f32 / (frames - 1) as f32, &[], f as f64 / 30.0);
                for &b in &feet {
                    worst = worst.max(pose[b].1.distance(first[b].1));
                }
            }
            if worst > 0.3 {
                bad.push(format!("{class} {name}: {worst:.2}"));
            }
        }
    }
    assert!(bad.is_empty(), "51: feet moved: {bad:?}");
}

/// Test 52 (4c 4): the Engineer at rest with his eyes at 270 and his feet
/// at 315, 270 and 225 (`body_yaw` +45, 0, -45), the foot locks on: the
/// feet stay where the stand puts them in the feet's frame, so their
/// spread across the eyes is 19.0, 13.1 and -0.5.
#[test]
fn the_feet_yaw_changes_the_apparent_spread_52() {
    let Some(sets) = sets(&["engineer"]) else { return };
    let set = &sets[0];
    let feet = [bone(set, "bip_foot_r"), bone(set, "bip_foot_l")];
    let own = posed_at(set, "stand_MELEE", 0.0, &[], 0.0);
    for (theta, spread) in [(45.0f32, 19.0f32), (0.0, 13.1), (-45.0, -0.5)] {
        let pose = posed_at(set, "stand_MELEE", 0.0, &[("body_yaw", theta)], 0.0);
        for &b in &feet {
            assert!(pose[b].1.distance(own[b].1) < 0.1, "52: {theta}: {} moved {}", set.bone_names[b], pose[b].1.distance(own[b].1));
        }
        let d = pose[feet[1]].1 - pose[feet[0]].1;
        let t = theta.to_radians();
        let across = d.x * t.sin() + d.y * t.cos();
        eprintln!("52: body_yaw {theta}: spread across the eyes {across:.2}");
        assert!((across - spread).abs() < 0.3, "52: {theta}: {across}");
    }
}

/// Test 56 (4c 3): the noise layers are foot-locked (the feet within 0.1
/// of the stand's own at any cycle), and the Medic's is real time (its
/// cycle from the clock, 1.6 s a loop, whatever the stand's).
#[test]
fn noise_layers_are_foot_locked_56() {
    let Some(sets) = sets(&["scout", "medic"]) else { return };
    for (set, stand) in [(&sets[0], "stand_PRIMARY"), (&sets[1], "stand_SECONDARY")] {
        let noise = seq(set, "layer_idleNoise");
        assert!(set.sequences[seq(set, stand)].autolayers.iter().any(|l| l.sequence == noise), "{stand} layers the noise");
        let feet = [bone(set, "bip_foot_r"), bone(set, "bip_foot_l")];
        for k in 0..8 {
            let (cycle, now) = (k as f32 / 8.0, 0.37 * k as f64);
            let mut own = set.defaults.clone();
            set.sequence_pose(seq(set, stand), cycle, &set.params.iter().map(|p| p.encode(0.0)).collect::<Vec<_>>(), &mut own);
            let own = model_space(&own, &set.parents);
            let pose = posed_at(set, stand, cycle, &[], now);
            for &b in &feet {
                let d = pose[b].1.distance(own[b].1);
                assert!(d < 0.1, "56: {stand} at {cycle}, {now} s: {} {d}", set.bone_names[b]);
            }
        }
    }
    let medic = &sets[1];
    let noise = &medic.sequences[seq(medic, "layer_idleNoise")];
    assert!(noise.realtime && !sets[0].sequences[seq(&sets[0], "layer_idleNoise")].realtime, "56: the Medic's noise real time, the Scout's not");
    let period = 1.0 / medic.cycle_rate(seq(medic, "layer_idleNoise"), &medic.default_params());
    assert!((period - 1.6).abs() < 0.01, "56: {period} s a loop");
    // Real time: the same stand cycle at two clock times differs only by
    // the noise; at times a loop apart it is the same pose.
    let a = posed_at(medic, "stand_SECONDARY", 0.2, &[], 10.0);
    let b = posed_at(medic, "stand_SECONDARY", 0.2, &[], 10.0 + f64::from(period));
    let c = posed_at(medic, "stand_SECONDARY", 0.2, &[], 10.5);
    let pelvis = bone(medic, "bip_pelvis");
    assert!(a[pelvis].1.distance(b[pelvis].1) < 0.01, "a loop apart: the same");
    assert!(a[pelvis].0.angle_between(c[pelvis].0) > 0.0 || a[pelvis].1.distance(c[pelvis].1) > 0.0, "the clock moves the noise");
}

/// Test 60 (4a): merging the include copies its knee directions onto the
/// class model's foot chains.
#[test]
fn the_includes_knee_directions_60() {
    let Some(sets) = sets(&["engineer", "scout"]) else { return };
    let knee = |set: &AnimSet, name: &str| set.ik_chains.iter().find(|c| c.name == name).unwrap_or_else(|| panic!("no {name}")).knee;
    for (set, chain, want) in [
        (&sets[0], "rfoot", Vec3::new(0.600, -0.002, 0.800)),
        (&sets[0], "lfoot", Vec3::new(0.595, 0.002, 0.804)),
        (&sets[1], "rfoot", Vec3::new(0.709, -0.705, 0.002)),
        (&sets[1], "lfoot", Vec3::new(0.708, -0.002, 0.706)),
    ] {
        let got = knee(set, chain);
        eprintln!("60: {chain} {got:.3}");
        assert!(got.distance(want) < 0.005, "60: {chain}: {got}, not {want}");
    }
}
