//! The 19 player models from the install (specs/hl2dm/player_models.md
//! 1, 2, 9, 10; test cases 1-2, P2-P5): each loads as a character body
//! through HL2:DM's animation models (`hl2mp/`'s include overrides reach
//! `models/player/male_anims.mdl` or `female_anims.mdl`, whatever the
//! include's case or slashes), with the animation model's 105 sequences
//! after the model's own (`ragdoll`; male_02 has three more of HL2's, so
//! its merged indices are 3 past the others', unlike the spec's 1.2), the
//! eight pose parameters in order and range, every
//! activity the animation state asks for, its hitbox set (17 or 18 boxes,
//! groups as §9), its ragdoll (15 bodies, 14 joints, the masses of §10.2)
//! and, on citizens, two eyeballs with their iris and the teeth. Skips
//! without an install.

use hl2dm::game::{
    bodies::{self, PLAYER_MODELS},
    weapons::inventory::ModelKind,
};

use super::map_stock::install_mount;

fn near(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() < eps
}

#[test]
fn every_model_loads_with_its_activities_and_hitbox_set() {
    let Some(mount) = install_mount() else { return };
    let mut wrong = Vec::new();
    for path in PLAYER_MODELS {
        let m = match bodies::load_one(&mount, path) {
            Ok(m) => m,
            Err(e) => {
                wrong.push(format!("{path}: {e}"));
                continue;
            }
        };
        let set = m.animations.as_ref().unwrap_or_else(|| panic!("{path}: no animations"));
        let names: Vec<&str> = set.sequences.iter().map(|s| s.name.as_str()).collect();
        // The model's own sequences come first (`ragdoll`; male_02 has
        // three more of HL2's), then the animation model's 105.
        let own = names.iter().position(|n| n.eq_ignore_ascii_case("reference")).unwrap_or(1);
        if names.len() != 105 + own || (own != 1 && !path.ends_with("male_02.mdl")) {
            wrong.push(format!("{path}: {} sequences: {:?}", names.len(), &names[..own.min(6)]));
        }
        for (i, n) in [
            (0, "ragdoll"),
            (1, "reference"),
            (17, "idle_pistol"),
            (21, "idle_gravgun"),
            (28, "run_pistol"),
            (32, "run_smg1"),
            (36, "run_gravgun"),
            (52, "cidle_grenade"),
            (58, "cwalk_pistol"),
            (77, "range_pistol"),
            (90, "jump_ar2"),
            (99, "reload_smg1"),
            (105, "head_rot_x"),
        ] {
            let i = if i == 0 { 0 } else { i + own - 1 };
            if !names.get(i).is_some_and(|x| x.eq_ignore_ascii_case(n)) {
                wrong.push(format!("{path}: merged {i} is {:?}, not {n}", names.get(i)));
            }
        }
        let params: Vec<&str> = set.params.iter().map(|p| p.name.as_str()).collect();
        let want = ["aim_yaw", "aim_pitch", "move_yaw", "body_yaw", "spine_yaw", "head_yaw", "head_pitch", "head_roll"];
        if params.len() != 8 || !params.iter().zip(want).all(|(a, b)| a.eq_ignore_ascii_case(b)) {
            wrong.push(format!("{path}: pose parameters {params:?}"));
        }
        let female = path.contains("female");
        let (lo, hi) = if female { (-50.8596, 56.8350) } else { (-49.4432, 52.5174) };
        if !near(set.params[0].start, lo, 1e-3) || !near(set.params[0].end, hi, 1e-3) {
            wrong.push(format!("{path}: aim_yaw {}..{}", set.params[0].start, set.params[0].end));
        }
        // Every activity of the state (§2-§4, §8) for every kind.
        for kind in ["PISTOL", "SHOTGUN", "SMG1", "AR2", "PHYSGUN", "GRENADE", "RPG", "CROSSBOW", "MELEE", "SLAM"] {
            for act in ["IDLE", "RUN", "IDLE_CROUCH", "WALK_CROUCH", "GESTURE_RANGE_ATTACK", "JUMP"] {
                let a = format!("ACT_HL2MP_{act}_{kind}");
                if set.activity(&a).is_none() {
                    wrong.push(format!("{path}: no {a}"));
                }
            }
        }
        for kind in ["PISTOL", "SHOTGUN", "SMG1", "AR2"] {
            if set.activity(&format!("ACT_HL2MP_GESTURE_RELOAD_{kind}")).is_none() {
                wrong.push(format!("{path}: no reload gesture for {kind}"));
            }
        }
        if set.activity("ACT_DIERAGDOLL") != Some(0) {
            wrong.push(format!("{path}: ragdoll isn't merged 0"));
        }
        // Hitboxes (§9): 17 (male citizens, police) or 18 (women, Combine).
        let kind = ModelKind::of(path);
        let boxes = match (kind, female) {
            (Some(ModelKind::Citizen), false) | (Some(ModelKind::MetroPolice), _) => 17,
            _ => 18,
        };
        if m.hitboxes.len() != boxes || m.boxes.len() != boxes {
            wrong.push(format!("{path}: {} hitboxes, want {boxes}", m.hitboxes.len()));
        }
        let heads = m.boxes.iter().filter(|b| b.group == chimera_core::core::Hitgroup::Head).count();
        if heads != 1 {
            wrong.push(format!("{path}: {heads} head boxes"));
        }
        if kind == Some(ModelKind::Combine) {
            // Pelvis and Spine2 are generic on the Combine; no stomach.
            if m.boxes.iter().any(|b| b.group == chimera_core::core::Hitgroup::Stomach) {
                wrong.push(format!("{path}: a stomach box"));
            }
            let generic = m.boxes.iter().filter(|b| b.group == chimera_core::core::Hitgroup::Generic).count();
            if generic != 2 {
                wrong.push(format!("{path}: {generic} generic boxes"));
            }
        }
        // The ragdoll (§10.2): 15 bodies, 14 joints, pelvis first.
        match &m.ragdoll {
            Some(r) => {
                if r.bodies.len() != 15 || r.joints.len() != 14 {
                    wrong.push(format!("{path}: ragdoll {} bodies {} joints", r.bodies.len(), r.joints.len()));
                }
                let pelvis = &m.bones[r.bodies[0].bone].name;
                if !pelvis.eq_ignore_ascii_case("ValveBiped.Bip01_Pelvis") {
                    wrong.push(format!("{path}: ragdoll body 0 is {pelvis}"));
                }
                let total: f32 = r.bodies.iter().map(|b| b.mass).sum();
                let want = match (kind, female) {
                    (Some(ModelKind::Combine), _) => 100.0,
                    (_, true) => 70.65,
                    _ => 90.0,
                };
                if !near(total, want, 0.05) {
                    wrong.push(format!("{path}: ragdoll mass {total}"));
                }
            }
            None => wrong.push(format!("{path}: no ragdoll")),
        }
        // Every mesh textured (the Combine's base textures sit in a
        // `">=dx90_20b"` block).
        for x in m.model.meshes.iter().filter(|x| x.texture.is_none()) {
            wrong.push(format!("{path}: {} untextured", x.material));
        }
        // Faces (§1.3, §11.4): citizens' two eyeballs.
        let eyes = m.model.meshes.iter().filter(|x| x.eye.is_some()).count();
        let want_eyes = if kind == Some(ModelKind::Citizen) { 2 } else { 0 };
        if eyes != want_eyes {
            wrong.push(format!("{path}: {eyes} eye meshes"));
        }
        if kind == Some(ModelKind::Citizen) {
            // The spec's 1.4706 (men) and 1.5152 (women) are two of the
            // values the models carry: 1.4706, 1.5152 and 1.5873 occur on
            // either.
            for e in m.model.meshes.iter().filter_map(|x| x.eye.as_ref()) {
                if !near(e.radius, 0.5, 1e-3) || !(1.4..1.6).contains(&e.iris_scale) || e.iris.is_none() {
                    wrong.push(format!("{path}: eye radius {} iris scale {} iris {:?}", e.radius, e.iris_scale, e.iris));
                }
            }
            if !m.model.meshes.iter().any(|x| x.teeth.is_some()) {
                wrong.push(format!("{path}: no Teeth mesh"));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn run_cycles_have_the_specs_ground_speeds() {
    // Constants and §5: N 209.7902, S 169.8302, E/W 149.8501, crouch 60,
    // and halfway N-NE the blended vector's 193.8205 (open question 2).
    let Some(mount) = install_mount() else { return };
    for path in ["models/humans/group03/male_01.mdl", "models/humans/group03/female_01.mdl", "models/combine_soldier.mdl"] {
        let m = bodies::load_one(&mount, path).unwrap();
        let set = m.animations.unwrap();
        let mut a = chimera_world::map::anim::Animator::new(set.clone());
        let run = set.sequence("run_smg1").unwrap();
        let crouch = set.sequence("cwalk_pistol").unwrap();
        let p = set.param("move_yaw").unwrap();
        for (yaw, want, s) in [
            (0.0, 209.7902, run),
            (180.0, 169.8302, run),
            (-90.0, 149.8501, run),
            (90.0, 149.8501, run),
            (-22.5, 193.8205, run),
            (0.0, 60.0, crouch),
            (-90.0, 60.0, crouch),
        ] {
            a.set_param(p, yaw);
            let g = set.ground_motion(s, &a.params);
            assert!(near(g, want, 0.01), "{path} move_yaw {yaw}: {g}, want {want}");
        }
        // Cycle rates: run 1/0.6, idle 1/0.9667, jump 1/0.8667.
        assert!(near(set.cycle_rate(run, &a.params), 1.0 / 0.6, 1e-3));
        let jump = set.sequence("jump_ar2").unwrap();
        assert!(near(set.cycle_rate(jump, &a.params), 1.0 / 0.8667, 1e-3));
    }
}


#[test]
fn eyes_look_out_of_the_face() {
    // `MapEye::forward` is the gaze at rest: from each eye's centre away
    // from the head bone, the way the face looks (a citizen's eyes sit in
    // front of the head bone's origin, along it).
    let Some(mount) = install_mount() else { return };
    for path in ["models/humans/group03/female_04.mdl", "models/humans/group03/male_07.mdl"] {
        let m = bodies::load_one(&mount, path).unwrap();
        for e in m.model.meshes.iter().filter_map(|x| x.eye.as_ref()) {
            assert!(e.origin.normalize().dot(e.forward) > 0.5, "{path}: eye at {} looks {}", e.origin, e.forward);
        }
    }
}
