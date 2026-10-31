//! specs/combat_arms/characters.md's "install" test cases on the user's
//! Combat Arms install: the default male and female bodies with their
//! animation libraries (`CHARS_M_BODY`, V2-keyed), posed by `characters::
//! skeleton` and driven by `characters::driver`. The spec measured them on
//! the US reference client; the Steam libraries differ (Open question 8),
//! so a mismatch here is a number to re-measure, not necessarily a bug.
//! Passes with a note without an install or without the V2 key.

use std::sync::Arc;

use bevy::prelude::*;
use chimera_core::mount::config::LocalConfig;
use combat_arms::game::{
    GAME, INFO,
    characters::{
        driver::{Driver, Inputs},
        records,
        skeleton::{Posed, Rig},
    },
    model,
};
use lithtech_formats::anim::Tracker;

fn load(female: bool) -> Option<Rig> {
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else {
        eprintln!("skipping: no Combat Arms install configured");
        return None;
    };
    if !chimera_core::mount::served::usable("combat_arms", &install, "Game") {
        eprintln!("skipping: no Combat Arms install at {}", install.display());
        return None;
    }
    let mount = (INFO.open)(&install).unwrap();
    let file = |p: &str| mount.read(p).ok();
    let ca = combat_arms::game::data::CaData::install();
    let record = ca.characters().model(if female { 7 } else { 0 }).unwrap();
    match model::load(&record.file, &file) {
        Ok(l) => Some(Rig::new(l.model, l.list)),
        Err(e) => {
            eprintln!("skipping: {e} (CHARS_M_BODY needs the V2 key)");
            None
        }
    }
}

fn on(rig: &Rig, name: &str) -> Tracker {
    Tracker::on(rig.list.find(name).unwrap_or_else(|| panic!("no {name}")), true)
}

/// The drawn pose's frame: the rig's bind pose as `locals_rh` (what the
/// body's `Animator::fixed` gets) is the drawn skeleton's bind
/// (`model::bones`), node by node; and the standing pose stays near it
/// (no node more than a body height from its bind place).
#[test]
fn drawn_pose_matches_the_drawn_skeleton() {
    let Some(rig) = load(false) else { return };
    let bones = model::bones(&rig.model);
    let locals = rig.bind().locals_rh();
    assert_eq!(bones.len(), locals.len());
    for (j, (b, (q, p))) in bones.iter().zip(&locals).enumerate() {
        let dp = (b.position - *p).length();
        let dq = b.rotation.dot(*q).abs();
        assert!(dp < 0.01 && dq > 0.9999, "{} ({j}): bind {:?} {:?}, posed {p:?} {q:?}", b.name, b.position, b.rotation);
    }
    let globals = |l: &[(Quat, Vec3)]| -> Vec<Vec3> {
        let mut g: Vec<(Quat, Vec3)> = Vec::new();
        for (j, (q, p)) in l.iter().enumerate() {
            let x = match bones[j].parent {
                Some(par) => (g[par].0 * *q, g[par].1 + g[par].0 * *p),
                None => (*q, *p),
            };
            g.push(x);
        }
        g.into_iter().map(|x| x.1).collect()
    };
    let (bind, posed) = (globals(&locals), globals(&stand(&rig, 0.0).locals_rh()));
    for (j, (a, b)) in bind.iter().zip(&posed).enumerate() {
        assert!((*a - *b).length() < 80.0, "{}: bind {a}, standing {b}", bones[j].name);
    }
}

/// The default male body as the map carries it, skinned as the renderer
/// does (joints under the model's root, inverse binds from the same
/// chain): it stands on the body's origin, about a man tall, and in the
/// standing pose every vertex keeps its distance to the node that weighs
/// most on it (a mesh outside the root's frame swings about far-off
/// nodes: the body drawn torn apart, sunk to the waist).
#[test]
fn drawn_body_skins_in_its_frame() {
    use chimera_world::map::MapData;
    use combat_arms::game::characters::models;
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else { return };
    if !chimera_core::mount::served::usable("combat_arms", &install, "Game") {
        return;
    }
    let mount = (INFO.open)(&install).unwrap();
    let file = |p: &str| mount.read(p).ok();
    let mut map = MapData::default();
    models::add_character_models(&mut map, &combat_arms::game::data::CaData::install(), &file);
    let Some(body) = map.kit.characters.iter().find(|c| c.name.as_deref() == Some(models::body_name(0).as_str())) else {
        eprintln!("skipping: no character bodies (V2 key)");
        return;
    };
    let Some(rig) = load(false) else { return };
    let chain = |l: &[(Quat, Vec3)]| -> Vec<Mat4> {
        let mut g: Vec<Mat4> = Vec::new();
        for (j, (q, p)) in l.iter().enumerate() {
            let local = Mat4::from_rotation_translation(*q, *p);
            g.push(body.bones[j].parent.map_or(body.root.to_matrix() * local, |par| g[par] * local));
        }
        g
    };
    let bind = chain(&body.bones.iter().map(|b| (b.rotation, b.position)).collect::<Vec<_>>());
    let posed = chain(&stand(&rig, 0.0).locals_rh());
    let (mut low, mut high, mut worst) = (f32::MAX, f32::MIN, (0.0f32, String::new()));
    for (k, mesh) in body.model.meshes.iter().enumerate() {
        for (v, p) in mesh.positions.iter().enumerate() {
            let p = Vec3::from(*p);
            (low, high) = (low.min(p.y), high.max(p.y));
            let j = mesh.joints[v]
                .iter()
                .zip(mesh.joint_weights[v])
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap()
                .0;
            let j = *j as usize;
            let skinned: Vec3 = mesh.joints[v]
                .iter()
                .zip(mesh.joint_weights[v])
                .map(|(&n, w)| (posed[n as usize] * bind[n as usize].inverse()).transform_point3(p) * w)
                .sum();
            let at_bind = (bind[j].w_axis.truncate() - p).length();
            let at_pose = (posed[j].w_axis.truncate() - skinned).length();
            if (at_pose - at_bind).abs() > worst.0 {
                worst = ((at_pose - at_bind).abs(), format!("piece {k} vertex {v} on {}", body.bones[j].name));
            }
        }
    }
    eprintln!("body from {low} to {high} m; worst drift {worst:?}");
    assert!(low.abs() < 0.15, "feet at {low} m");
    assert!((1.2..2.0).contains(&high), "top at {high} m");
    assert!(worst.0 < 0.05, "{} drifts {} m from its node", worst.1, worst.0);
}

/// The layered pose of section 4 at t = 0 (stand: crouch weight 0).
fn stand(rig: &Rig, r: f32) -> Posed {
    let mut crouch = on(rig, "LC");
    crouch.multiplier = r;
    let pose = rig.layered(&Tracker::on(0, true), &on(rig, "URFRAmS"), &on(rig, "LST"), &crouch, r);
    rig.posed(&pose, r)
}

/// One animation alone on every node at `t` ms.
fn single(rig: &Rig, name: &str, t: u32) -> Posed {
    let mut tr = on(rig, name);
    tr.set_time(&rig.list, t);
    let pose: Vec<_> = (0..rig.model.nodes.len()).map(|j| tr.pose(&rig.list, j)).collect();
    rig.posed(&pose, 0.0)
}

fn near(got: Option<Vec3>, want: [f32; 3], what: &str) {
    let got = got.unwrap_or_else(|| panic!("{what}: missing"));
    assert!((got - Vec3::from(want)).abs().max_element() < 0.01, "{what}: {got} vs {want:?}");
}

fn aimed(rig: &Rig, pitch: f32, twist: f32, server: bool) -> Posed {
    let mut p = stand(rig, 0.0);
    p.aim(&records::builtin().skeleton(0).aim, pitch.to_radians(), twist.to_radians(), server);
    p
}

/// Cases 6-17's names and lengths on the install's library.
#[test]
fn install_states() {
    let Some(rig) = load(false) else { return };
    let mut d = Driver::new(Arc::new(rig.list.clone()));
    d.update(15, &Inputs::default());
    assert_eq!((d.name_of(&d.upper), d.name_of(&d.lower), d.name_of(&d.crouch)), ("URFRAmS", "LST", "LC"));
    for (name, len) in [("LST", 4000), ("LWF", 1000), ("LRF", 600), ("LCF", 1400), ("LSJ", 666), ("URFRFiS", 100), ("URFRSReS", 1866), ("D_HIT", 467), ("DT", 3033)] {
        let e = rig.list.find(name).unwrap_or_else(|| panic!("{name}"));
        assert_eq!(rig.list.entries[e].length(), len, "{name}");
    }
}

#[test]
fn t21_male_aim() {
    let Some(rig) = load(false) else { return };
    let p = aimed(&rig, 0.0, 0.0, false);
    near(p.sphere("Head"), [5.66, 29.72, 9.65], "Head");
    near(p.socket("RightHand").map(|s| s.0), [12.54, 22.90, 19.93], "RightHand");
    let p = aimed(&rig, 30.0, 0.0, false);
    near(p.sphere("Head"), [5.66, 24.68, 16.72], "Head down");
    near(p.socket("RightHand").map(|s| s.0), [12.54, 13.34, 22.19], "RightHand down");
    near(p.sphere("Face_Eyelid1"), [6.62, 28.50, 19.96], "Face_Eyelid1 down");
    near(aimed(&rig, -30.0, 0.0, false).sphere("Head"), [5.66, 30.98, 6.23], "Head up");
    let p = aimed(&rig, 0.0, 45.0, false);
    near(p.sphere("Head"), [10.30, 29.72, 3.59], "Head twisted");
    near(p.socket("RightHand").map(|s| s.0), [22.43, 22.90, 6.00], "RightHand twisted");
    near(aimed(&rig, 30.0, 0.0, true).sphere("Head"), [5.66, 27.45, 13.02], "server Head down");
}

#[test]
fn t22_female_aim() {
    let Some(rig) = load(true) else { return };
    let mut p = stand(&rig, 0.0);
    p.aim(&records::builtin().skeleton(1).aim, 30f32.to_radians(), 0.0, false);
    near(p.sphere("Bip01 Head"), [2.95, 24.74, 17.70], "Bip01 Head");
    near(p.sphere("Headfuck"), [4.07, 25.08, 20.87], "Headfuck");
}

#[test]
fn t23_to_27_sockets() {
    let Some(rig) = load(false) else { return };
    let bind = rig.bind();
    for s in &records::builtin().skeleton(0).sockets {
        near(bind.socket(&s.name).map(|p| p.0), s.bind, &s.name);
    }
    near(stand(&rig, 0.0).socket("RightHand").map(|s| s.0), [12.54, 22.90, 19.93], "RightHand stand");
    near(single(&rig, "LST", 0).socket("NAME_socket").map(|s| s.0), [9.70, 45.02, 13.52], "NAME_socket LST");
    near(stand(&rig, 0.0).socket("NAME_socket").map(|s| s.0), [8.63, 45.87, 11.51], "NAME_socket stand");
    near(stand(&rig, 1.0).socket("NAME_socket").map(|s| s.0), [6.13, 37.57, 15.21], "NAME_socket crouch");
    near(stand(&rig, 0.0).sphere("Left_foot"), [-10.02, -41.49, 13.52], "Left_foot stand");
    let Some(f) = load(true) else { return };
    near(stand(&f, 0.0).socket("NAME_socket").map(|s| s.0), [9.28, 43.71, 19.18], "female NAME_socket stand");
    near(stand(&f, 0.0).sphere("Left_foot"), [-7.73, -39.56, 12.91], "female Left_foot stand");
}

#[test]
fn t28_29_32_tables() {
    for female in [false, true] {
        let Some(rig) = load(female) else { return };
        let (bind, lst) = (rig.bind(), single(&rig, "LST", 0));
        let skel = records::builtin().skeleton(usize::from(female));
        for s in skel.spheres.iter().filter(|s| s.radius > 0.0 && !s.absent) {
            near(bind.sphere(&s.name), s.bind.unwrap(), &format!("{} bind", s.name));
            near(lst.sphere(&s.name), s.at.unwrap(), &format!("{} LST", s.name));
        }
    }
}

#[test]
fn t30_lrf() {
    let Some(rig) = load(false) else { return };
    let p = single(&rig, "LRF", 300);
    near(p.sphere("Left_foot"), [-3.53, -33.27, -23.60], "Left_foot");
    near(p.sphere("Right_foot"), [3.44, -40.27, 18.95], "Right_foot");
    near(p.sphere("Head"), [2.70, 28.39, 13.53], "Head");
    near(p.sphere("Right_legl1"), [4.26, -30.65, 13.80], "Right_legl1");
}

#[test]
fn t31_33_stand_and_crouch() {
    if let Some(rig) = load(false) {
        let (s, c) = (stand(&rig, 0.0), stand(&rig, 1.0));
        for (name, a, b) in [
            ("Head", [5.66, 29.72, 9.65], [3.21, 21.61, 12.05]),
            ("Torso", [-0.15, 5.91, -0.22], [-0.69, -1.65, 0.15]),
            ("Face_Eyelid1", [6.62, 34.74, 9.89], [4.24, 26.57, 12.72]),
            ("left_hand", [8.84, 23.46, 28.27], [1.39, 14.69, 30.63]),
        ] {
            near(s.sphere(name), a, name);
            near(c.sphere(name), b, name);
        }
    }
    if let Some(rig) = load(true) {
        let (s, c) = (stand(&rig, 0.0), stand(&rig, 1.0));
        near(s.sphere("Bip01 Head"), [2.95, 30.63, 11.32], "female stand");
        near(c.sphere("Bip01 Head"), [2.48, 15.39, 15.58], "female crouch");
        // Above the feet: 80.63 -> 55.39 (Quirk 7).
        assert!((s.sphere("Bip01 Head").unwrap().y - s.feet - 80.63).abs() < 0.01);
        assert!((c.sphere("Bip01 Head").unwrap().y - c.feet - 55.39).abs() < 0.01);
    }
}

#[test]
fn t34_35_counts() {
    let Some(rig) = load(false) else { return };
    let bind = rig.bind();
    let skel = records::builtin().skeleton(0);
    let found = skel.spheres.iter().filter(|s| s.radius > 0.0 && bind.sphere(&s.name).is_some()).count();
    assert_eq!(found, 33);
    if let Some(f) = load(true) {
        let bind = f.bind();
        let skel = records::builtin().skeleton(1);
        assert_eq!(skel.spheres.iter().filter(|s| s.radius > 0.0 && bind.sphere(&s.name).is_some()).count(), 35);
    }
}

/// The census's unkeyed models on the stock skeletons: the infected ship's
/// dead-body props (53 and 60 nodes) and the zombie render-style bodies.
/// Their nodes carry the hit tables' names: the posing pipeline (rig,
/// bind pose, spheres on nodes) runs on them without the V2 key, and the
/// male ones' bind pose is case 28's (the female prop's differs by up to
/// 2 u: another body on the same tree).
#[test]
fn unkeyed_bodies_on_the_stock_skeletons() {
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else { return };
    if !chimera_core::mount::served::usable("combat_arms", &install, "Game") {
        return;
    }
    let mount = (INFO.open)(&install).unwrap();
    let file = |p: &str| mount.read(p).ok();
    for (path, skeleton, nodes) in [
        ("PROPS\\INFECTEDSHIP\\INFECTED_SHIP_DEAD_BODY_MAN01_KE.LTB", 0, 53),
        ("PROPS\\INFECTEDSHIP\\INFECTED_SHIP_DEAD_BODY_WOMAN_KE.LTB", 1, 60),
        ("RS\\CM_ZOMBIE_TURKEYARMY.LTB", 0, 53),
    ] {
        let loaded = match model::load(path, &file) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("skipping {path}: {e}");
                continue;
            }
        };
        assert_eq!(loaded.model.nodes.len(), nodes, "{path}");
        let rig = Rig::new(loaded.model, loaded.list);
        let bind = rig.bind();
        let skel = records::builtin().skeleton(skeleton);
        let mut far = 0.0f32;
        for s in skel.spheres.iter().filter(|s| s.radius > 0.0 && !s.absent && s.socket.is_none()) {
            let at = bind.sphere(&s.name).unwrap_or_else(|| panic!("{path}: no node {}", s.name));
            far = far.max((at - Vec3::from(s.bind.unwrap())).length());
        }
        if skeleton == 0 {
            // Case 28's node spheres: the male bind pose is the default
            // body's.
            assert!(far < 0.02, "{path}: {far}");
        }
        eprintln!("{path}: every node sphere's node there; farthest from the default body's bind: {far:.2} u; sockets {:?}", rig.model.sockets.iter().map(|s| &s.name).collect::<Vec<_>>());
    }
}

/// On WAREHOUSE with the install's bodies (when they read): the map
/// carries the default male and female bodies as character models, a player wears the
/// default one (`BodyName`), its hit spheres come from the model's pose
/// (not the spec's skeleton), and its drawn pose is the driver's.
#[test]
fn warehouse_players_wear_their_bodies() {
    use chimera_world::{
        harness::Sim,
        map::{BodyName, MapSimPlugin, anim::Animator},
    };
    use combat_arms::{
        game::{
            characters::{CaAnim, CaPose, models},
            movement, world,
        },
        harness::CaSim,
    };
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else { return };
    if !chimera_core::mount::served::usable("combat_arms", &install, "Game") {
        return;
    }
    let mount = (INFO.open)(&install).unwrap();
    let map = world::load_from(&mount, "warehouse").unwrap();
    let names: Vec<_> = map.kit.characters.iter().filter_map(|c| c.name.clone()).collect();
    if !names.contains(&models::body_name(0)) {
        eprintln!("skipping: no character bodies ({:?})", map.warnings.iter().find(|w| w.contains("character")));
        return;
    }
    assert_eq!(names, [models::body_name(0), models::body_name(7)]);
    let spawn = map.spawns[0].0;
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
    sim.ticks(2);
    let p = sim.spawn_character(spawn + Vec3::Y, movement::ID);
    sim.seconds(1.0);
    let world = sim.app.world();
    assert_eq!(world.get::<BodyName>(p).map(|b| b.0.as_str()), Some(models::body_name(0).as_str()));
    assert!(world.get::<CaAnim>(p).unwrap().rig.is_some());
    let pose = world.get::<CaPose>(p).unwrap();
    assert!(pose.modelled);
    let found = pose.spheres.iter().filter(|s| s.is_some()).count();
    assert_eq!(found, 33, "the default male's 33 spheres");
    let animator = world.get::<Animator>(p).expect("the body's animator");
    assert!(animator.fixed.is_some(), "drawn from the driver's pose");
}

/// characters.md section 2 with the keys: each sex's character model
/// (the default male, record 0, and its female twin, record 7) has the
/// stock bodies (Normal, Light, Heavy) as its first body part, the faces
/// as the second (its default face first: 0, and 4 for the female) and
/// the head pieces as the third (none first; the female face's hair
/// among them), merged on its skeleton: every piece is in the drawn
/// model, textured, and the faces and head pieces reach the head (above
/// 1.3 m). The team-mark pieces (texture slot 2) draw in the transparent
/// mark style, blended: an enemy's mark, `Alpha.dtx`, shows nothing.
/// Goggles and backpacks (the pool's) are parts 3 and 4, rigid on one
/// node each: goggles at the face, a backpack behind the upper body.
/// Skips without the V1 and V2 keys.
#[test]
fn default_bodies_wear_face_and_head() {
    use chimera_world::map::{MapAlpha, MapData};
    use combat_arms::game::characters::models;
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else { return };
    if !chimera_core::mount::served::usable("combat_arms", &install, "Game") {
        return;
    }
    if !(combat_arms::game::mount::can_read("V1") && combat_arms::game::mount::can_read("V2")) {
        eprintln!("skipping: needs the V1 and V2 keys");
        return;
    }
    let mount = (INFO.open)(&install).unwrap();
    let file = |p: &str| mount.read(p).ok();
    let ca = combat_arms::game::data::CaData::install();
    let data = ca.characters();
    assert!(data.face(0).and_then(|f| f.file.as_ref()).is_some(), "the install's Face0 has a file");
    let mut map = MapData::default();
    models::add_character_models(&mut map, &combat_arms::game::data::CaData::install(), &file);
    eprintln!("warnings: {:?}", map.warnings);
    let groups = models::loaded_groups();
    for (id, face_id) in [(0, 0), (7, 4)] {
        assert_eq!(data.default_face(id), face_id);
        let group = groups.iter().find(|g| g.records.first() == Some(&id)).expect("the group");
        eprintln!("group {id}: {group:?}");
        assert_eq!(group.records.len(), 3, "the three stock bodies");
        assert_eq!(group.faces.first(), Some(&face_id));
        // The sex's four faces (the Steam install lacks one female face's
        // file: a warning).
        assert!(group.faces.len() >= 3, "the sex's faces");
        assert_eq!(group.heads.first(), Some(&-1));
        let body = map
            .kit.characters
            .iter()
            .find(|c| c.name.as_deref() == Some(models::body_name(id).as_str()))
            .unwrap_or_else(|| panic!("body {id} not loaded: {:?}", map.warnings));
        let meshes = &body.model.meshes;
        assert_eq!(body.model.body_parts, group.parts());
        for part in 0..3u16 {
            let n = group.parts()[part as usize];
            for choice in 0..n {
                // Head piece choice 0 is none: no meshes.
                let has = meshes.iter().any(|m| m.body == Some((part, choice)));
                assert_eq!(has, !(part == 2 && choice == 0), "part {part} choice {choice}");
            }
        }
        for (k, mesh) in meshes.iter().enumerate() {
            assert!(mesh.texture.is_some() || mesh.material.is_empty(), "body {id}: mesh {k} ({}) untextured", mesh.material);
            if matches!(mesh.body, Some((1 | 2, _))) {
                let top = mesh.positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
                assert!(top > 1.3, "body {id}: face or head mesh {k} tops out at {top} m");
            }
        }
        // Goggles and backpacks (parts 3 and 4): none first, then the
        // pool's; each rigid on one node, the goggles at the face, the
        // backpack on the upper body.
        assert_eq!((group.goggles.first(), group.backpacks.first()), (Some(&-1), Some(&-1)));
        assert!(group.goggles.len() > 1 && group.backpacks.len() > 1, "{group:?}");
        // Heights in the bind pose, against the default face's centre
        // (the female bind pose stands higher than the male one).
        let centre = |part: u16, choice: u16| {
            let points: Vec<Vec3> = meshes
                .iter()
                .filter(|m| m.body == Some((part, choice)))
                .flat_map(|m| m.positions.iter().map(|p| Vec3::from(*p)))
                .collect();
            points.iter().sum::<Vec3>() / points.len().max(1) as f32
        };
        let face = centre(1, 0);
        for (part, low, high) in [(3u16, -0.15f32, 0.15f32), (4, -0.6, -0.2)] {
            for choice in 1..group.parts()[part as usize] {
                let worn: Vec<_> = meshes.iter().filter(|m| m.body == Some((part, choice))).collect();
                assert!(!worn.is_empty(), "body {id}: part {part} choice {choice}");
                let at = centre(part, choice) - face;
                eprintln!("body {id}: part {part} choice {choice}: {} meshes, centre {at:.2} m from the face's", worn.len());
                assert!((low..high).contains(&at.y), "body {id}: part {part} choice {choice} at {at} m from the face");
                assert!(at.x.abs() < 0.1, "body {id}: part {part} choice {choice} off centre: {at}");
                // A backpack behind (the body faces -Z), goggles not.
                assert_eq!(at.z > 0.15, part == 4, "body {id}: part {part} choice {choice} at {at}");
                let node = worn[0].joints[0][0];
                assert!(worn.iter().all(|m| m.joints.iter().all(|j| j[0] == node)));
                assert!(worn.iter().all(|m| m.joint_weights.iter().all(|w| w[0] == 1.0)));
            }
        }
        assert!(body.model.skins.iter().all(|s| s.len() == meshes.len()));
        // The team marks: alpha-tested cut-outs in both families
        // (characters.md Corrections (2026-10-30) 2), the enemy's texture
        // (mark 0) clear, the teammate's (mark 1) not.
        let enemy = &body.model.skins[models::family(0, false) as usize];
        let mate = &body.model.skins[models::family(0, true) as usize];
        let marks: Vec<usize> = (0..meshes.len())
            .filter(|&k| mate[k].material.eq_ignore_ascii_case(&data.marks.teammate) && meshes[k].body == Some((0, 0)))
            .collect();
        assert!(!marks.is_empty(), "body {id}: no team-mark piece");
        for &k in &marks {
            assert!(matches!(enemy[k].alpha, MapAlpha::Mask(_)), "body {id}: mark {k}: {:?}", enemy[k].alpha);
            assert!(matches!(mate[k].alpha, MapAlpha::Mask(_)));
            assert!(enemy[k].material.eq_ignore_ascii_case(&data.marks.enemy));
            let opacity = |t: Option<usize>| {
                let t = &map.textures[t.expect("textured")];
                t.rgba8.chunks(4).map(|p| p[3] as f32).sum::<f32>() / (t.rgba8.len() / 4) as f32
            };
            let (clear, shown) = (opacity(enemy[k].texture), opacity(mate[k].texture));
            eprintln!("body {id}: mark {k}: enemy alpha {clear:.1}, teammate {shown:.1}");
            assert!(clear < 8.0 && shown > 64.0);
        }
    }
    // The female face comes with a head piece (its hair).
    assert!(data.face(4).and_then(|f| f.hair).is_some());
    // Other players hold their weapon's own third-person model (`HHModel`
    // with its `HHSkin#`), textured, for every weapon players are given.
    let ours = ca.builtin_weapon_ids();
    let with_model: Vec<_> = ca.weapon_set()
        .weapons
        .iter()
        .filter(|w| ours.contains(&w.id) && w.models.world.is_some())
        .map(|w| w.id.clone())
        .collect();
    assert!(with_model.len() >= 10, "{with_model:?}");
    for id in &with_model {
        let held = map.kit.held.iter().find(|h| &h.key == id).unwrap_or_else(|| panic!("{id}: no held model: {:?}", map.warnings));
        assert!(!held.model.meshes.is_empty(), "{id}");
        assert!(held.model.meshes.iter().any(|m| m.texture.is_some()), "{id}: untextured (the grey box)");
    }
}

/// Third-person weapons point the way the body faces (models_ltb.md
/// "Skinning": a model is drawn in its animated pose). Each `HHModel`'s
/// own `Hand` animation turns its root half a turn from the bind pose;
/// drawn in the bind pose, the guns were held backwards. For every held
/// rifle-class model with a `Flash` socket: its muzzle in the `Hand` pose (the
/// socket on its posed node) touches the drawn meshes, and on the
/// standing aim pose of both bodies (they face +Z, LithTech's frame) it
/// lies ahead of the grip.
#[test]
fn held_weapons_point_forward() {
    use chimera_world::map::MapData;
    use combat_arms::game::characters::models;
    use lithtech_formats::anim as lta;
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else { return };
    if !chimera_core::mount::served::usable("combat_arms", &install, "Game") {
        return;
    }
    let mount = (INFO.open)(&install).unwrap();
    let file = |p: &str| mount.read(p).ok();
    let ca = combat_arms::game::data::CaData::install();
    let mut map = MapData::default();
    models::add_character_models(&mut map, &ca, &file);
    if map.kit.held.is_empty() {
        eprintln!("skipping: no third-person weapons (V2 key)");
        return;
    }
    let hand_socket = ca.characters().skeleton(0).socket("RightHand").unwrap().clone();
    let at_socket = models::socket_local(hand_socket.rotation, hand_socket.offset);
    // Each held model's muzzle in the right hand's frame (right-handed).
    let mut muzzles = Vec::new();
    // (Grenades and launched projectiles in flight, `grenades::thrown_key`
    // and `special::data::PROJECTILE_KEY`, are in no hand.)
    let in_flight = |key: &str| key.ends_with("#thrown") || key.starts_with(combat_arms::game::special::data::PROJECTILE_KEY);
    for held in map.kit.held.iter().filter(|h| !in_flight(&h.key)) {
        assert!(held.bone.eq_ignore_ascii_case("Right_hand"), "{}: on {}", held.key, held.bone);
        let w = ca.weapon_set().weapons.iter().find(|w| w.id == held.key).unwrap();
        let path = w.models.world.as_deref().unwrap().to_lowercase();
        // The rifle stance (`URFRAmS`) holds rifles, SMGs, MGs and sniper
        // rifles; launchers, pistols and the rest have their own.
        if !["hh_ar_", "hh_smg_", "hh_mg_", "hh_sr_"].iter().any(|k| path.contains(k)) {
            continue;
        }
        let loaded = model::load(&path, &file).unwrap();
        let Some(flash) = loaded.model.sockets.iter().find(|s| s.name.eq_ignore_ascii_case("flash")) else { continue };
        let entry = model::rest_entry(&loaded, models::HELD_POSE).expect("an animation");
        assert!(entry.name().eq_ignore_ascii_case(models::HELD_POSE), "{}: {}", held.key, entry.name());
        let pose: Vec<_> = (0..loaded.model.nodes.len()).map(|j| lta::sample(entry, j, 0.0)).collect();
        let g = lta::globals(&loaded.model, &pose);
        let lh = lta::transform_point(&g[flash.node as usize], flash.position);
        let muzzle = at_socket.transform_point3(Vec3::from(lithtech_formats::ltb::to_rh_vec(lh)));
        let touch = held.model.meshes.iter().flat_map(|m| &m.positions).map(|p| (Vec3::from(*p) - muzzle).length()).fold(f32::MAX, f32::min);
        assert!(touch < 4.0, "{}: its muzzle is {touch} u from its drawn meshes", held.key);
        muzzles.push((held.key.clone(), muzzle));
    }
    assert!(muzzles.len() >= 5, "guns with a muzzle: {}", muzzles.len());
    for female in [false, true] {
        let Some(rig) = load(female) else { return };
        let posed = stand(&rig, 0.0);
        let (at, q) = posed.globals[posed.node("Right_hand").expect("Right_hand")];
        let grip = posed.socket("RightHand").expect("RightHand").0;
        for (key, m) in &muzzles {
            // Back to LithTech's frame (z mirrored), then posed.
            let d = (at + q * Vec3::new(m.x, m.y, -m.z) - grip).normalize();
            assert!(d.z > 0.5, "female {female}: {key}'s muzzle is {d:.2} from the grip (the body faces +Z)");
        }
    }
}

/// One level of detail per piece (models_ltb.md "Level-of-detail
/// choice": LOD 0 up close): the default male body's pieces have three
/// levels each, and its drawn meshes are exactly its pieces' LOD 0, in
/// order, vertex for vertex (never a lower level, never two at once).
#[test]
fn bodies_draw_one_level_per_piece() {
    use chimera_world::map::MapData;
    use combat_arms::game::characters::models;
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else { return };
    if !chimera_core::mount::served::usable("combat_arms", &install, "Game") {
        return;
    }
    let mount = (INFO.open)(&install).unwrap();
    let file = |p: &str| mount.read(p).ok();
    let ca = combat_arms::game::data::CaData::install();
    let mut map = MapData::default();
    models::add_character_models(&mut map, &ca, &file);
    let Some(body) = map.kit.characters.iter().find(|c| c.name.as_deref() == Some(models::body_name(0).as_str())) else {
        eprintln!("skipping: no character bodies (V2 key)");
        return;
    };
    let record = ca.characters().model(0).unwrap();
    let loaded = model::load_own(&record.file, &file).unwrap();
    assert!(loaded.model.pieces.iter().any(|p| p.lods.len() == 3), "the body has pieces with three levels");
    let drawn: Vec<usize> = body.model.meshes.iter().filter(|m| m.body == Some((0, 0))).map(|m| m.positions.len()).collect();
    let lod0: Vec<usize> = loaded
        .model
        .pieces
        .iter()
        .filter_map(|p| p.lods.first()?.mesh.geometry().map(|g| g.positions.len()))
        .collect();
    assert_eq!(drawn, lod0, "one mesh per piece, its LOD 0");
}

/// Grenades in flight are drawn with their projectile record's model
/// (grenades.md, Data: the M67's `G62` and so on, `ModelScale` 2, its own
/// skin) in its `World` pose, in no hand: twice the size of the held one
/// (the same file in the stock data), every piece textured.
#[test]
fn thrown_grenades_draw_their_projectile_model() {
    use chimera_world::map::MapData;
    use combat_arms::game::{characters::models, grenades};
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else { return };
    if !chimera_core::mount::served::usable("combat_arms", &install, "Game") {
        return;
    }
    let mount = (INFO.open)(&install).unwrap();
    let file = |p: &str| mount.read(p).ok();
    let ca = combat_arms::game::data::CaData::install();
    let mut map = MapData::default();
    models::add_character_models(&mut map, &ca, &file);
    if map.kit.held.is_empty() {
        eprintln!("skipping: no third-person weapons (V2 key)");
        return;
    }
    let size = |m: &chimera_world::map::MapModel| {
        let (lo, hi) = m.meshes.iter().flat_map(|m| &m.positions).fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| {
            (lo.min(Vec3::from(*p)), hi.max(Vec3::from(*p)))
        });
        (hi - lo).length()
    };
    for id in [grenades::M67, grenades::XM84, grenades::M18] {
        let key = grenades::thrown_key(id);
        let thrown = map.kit.held.iter().find(|h| h.key == key).unwrap_or_else(|| panic!("{key}: {:?}", map.warnings));
        assert!(thrown.bone.is_empty(), "{key}: in no hand");
        let untextured = thrown.model.meshes.iter().filter(|m| m.texture.is_none()).count();
        assert_eq!(untextured, 0, "{key}: untextured pieces");
        let held = map.kit.held.iter().find(|h| h.key == id).unwrap();
        let ratio = size(&thrown.model) / size(&held.model);
        eprintln!("{key}: {:.2} u across, {ratio:.2} x the held model", size(&thrown.model));
        assert!((1.6..2.4).contains(&ratio), "{key}: {ratio} x the held model");
    }
}
