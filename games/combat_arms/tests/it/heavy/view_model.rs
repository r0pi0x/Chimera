//! Combat Arms' view models from the user's install (`game::view_model`),
//! specs/combat_arms/view_models.md's cases that need it (`t05_` is case
//! 5): every weapon whose data names one loads (the unkeyed pistol,
//! shotgun and machine-gun archives: M92FS, Remington 870, M60), skinned
//! where its texture archive is unkeyed (the 870); the M92FS's skeleton
//! and pieces (1, 2), its origin at `Pos` (4), its `flash` and `breach`
//! sockets through the animations, root translation included (5-7), both
//! by the game's own sampling and as drawn (the converted model), and its
//! state machine in play on the greybox; the M67's throw as drawn (every
//! node as the game samples it, the skinned arms intact); every rung look's
//! hands, infected too, whole and textured. The hand model (`CHARS_M_HAND`)
//! and the M4A1 (case 8) are V2-keyed: those parts note and skip. With
//! both keys every install weapon's view model loads on demand textured,
//! but for the pieces the install's data itself leaves without a skin
//! (`UNTEXTURED_IN_THE_DATA`: a piece on the `Hands` slot 0, a skin or
//! sprite frame that isn't in the install) and the one view model file
//! that isn't there. Passes with a note without `[games.combat_arms]
//! path` in chimera.local.toml.

use bevy::prelude::*;
use chimera_core::{core::Health, mount::config::LocalConfig};
use chimera_sim::weapon::{self, Magazine};
use chimera_world::{
    greybox::GreyboxMapPlugin,
    harness::Sim,
    map::{MapData, MapViewModel, ViewAnimator, ViewModels, view_model::attachment_transform},
};
use combat_arms::{
    game::{
        GAME, INFO, TICK_INTERVAL, model, movement,
        view_model::{self, CaViewModel, VIEW_SCALE, hands},
    },
    harness::CaSim,
};
use lithtech_formats::anim as lta;

const M92FS: &str = "combat_arms:beretta_m92fs";
const M92FS_PATH: &str = "guns_m_pv_pistol/pv_pst_m92fs_sh.ltb";
const POS: [f32; 3] = [-0.1, -0.2, 0.7];

fn mount() -> Option<chimera_core::mount::Mount> {
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else {
        eprintln!("skipping: no Combat Arms install configured");
        return None;
    };
    if !chimera_core::mount::served::usable("combat_arms", &install, "Game") {
        eprintln!("skipping: no Combat Arms install at {}", install.display());
        return None;
    }
    Some((INFO.open)(&install).unwrap())
}

/// The data's view models, loaded as a map would.
fn loaded() -> Option<(chimera_core::mount::Mount, MapData)> {
    let mount = mount()?;
    let file = |p: &str| mount.read(p).ok();
    let mut map = MapData::default();
    view_model::add_view_models(&mut map, &combat_arms::game::data::CaData::builtin(), &file);
    Some((mount, map))
}

#[test]
fn data_view_models_load_skinned() {
    let Some((mount, map)) = loaded() else { return };
    for w in &map.warnings {
        eprintln!("warning: {w}");
    }
    for v in &map.kit.view_models {
        let set = v.animations.as_ref().unwrap();
        eprintln!(
            "{}: {} meshes ({} textured), {} bones, sockets {:?}, animations {:?}",
            v.key,
            v.model.meshes.len(),
            v.model.meshes.iter().filter(|m| m.texture.is_some()).count(),
            v.bones.len(),
            v.attachments.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(),
            view_model::anim_infos(set)
                .iter()
                .map(|a| format!(
                    "{} {} ms{}",
                    a.name,
                    a.length,
                    a.keys
                        .iter()
                        .filter(|k| !k.1.is_empty())
                        .map(|k| format!(" [{} {}]", k.0, k.1))
                        .collect::<String>()
                ))
                .collect::<Vec<_>>()
        );
        // The machine's names: the stock ones every gun has.
        for name in ["select", "idle_0", "fire_0", "reload"] {
            assert!(set.sequence(name).is_some(), "{}: {name}", v.key);
        }
        // Flash and breach sockets (names without case).
        for s in ["flash", "breach"] {
            assert!(
                v.attachments.iter().any(|a| a.name.eq_ignore_ascii_case(s)),
                "{}: {s}",
                v.key
            );
        }
        let d = combat_arms::game::data::CaData::builtin()
            .weapon_set()
            .get(&v.key)
            .unwrap();
        let skins = d.models.view_skins.iter().filter(|s| !s.is_empty());
        let readable = skins.into_iter().all(|s| mount.read(s).is_ok());
        let textured = v.model.meshes.iter().all(|m| m.texture.is_some());
        assert_eq!(textured, readable, "{}: skins", v.key);
    }
    for id in [M92FS, "combat_arms:remington_870", "combat_arms:m60"] {
        assert!(map.kit.view_models.iter().any(|v| v.key == id), "{id}");
    }
}

/// With both keys (V1: the install's weapons file names each weapon's
/// `PVModelNormal`, `PVSkin#` and `Pos`; V2: the models and textures):
/// every weapon of ours loads its own first-person model with the hand
/// model attached, every piece textured.
#[test]
fn keyed_view_models_with_hands() {
    let Some(mount) = mount() else { return };
    if !(combat_arms::game::mount::can_read("V1") && combat_arms::game::mount::can_read("V2")) {
        eprintln!("skipping: needs the V1 and V2 keys");
        return;
    }
    let ca = combat_arms::game::data::CaData::install();
    if ca.attributes().is_none() {
        eprintln!("skipping: the install's attribute data doesn't read");
        return;
    }
    let file = |p: &str| mount.read(p).ok();
    let mut map = MapData::default();
    view_model::add_view_models(&mut map, &ca, &file);
    for w in &map.warnings {
        eprintln!("warning: {w}");
    }
    assert!(map.warnings.is_empty(), "{:?}", map.warnings);
    let set = ca.weapon_set();
    for id in [
        "m4a1",
        "mp5a4",
        "l96a1",
        "dragunov_svd",
        "m9_knife",
        "m67",
        "xm84",
        "m18",
        "m23f",
        "g7a",
        "beretta_m92fs",
        "m60",
        "remington_870",
    ] {
        let id = format!("combat_arms:{id}");
        let v = map
            .kit
            .view_models
            .iter()
            .find(|v| v.key == id)
            .unwrap_or_else(|| panic!("{id}: no view model"));
        let d = set.get(&id).unwrap();
        let untextured: Vec<&str> = v
            .model
            .meshes
            .iter()
            .filter(|m| m.texture.is_none())
            .map(|m| m.material.as_str())
            .collect();
        eprintln!(
            "{id}: {} ({} meshes, {} bones), skins {:?}, untextured {untextured:?}",
            d.models.view.as_deref().unwrap_or("-"),
            v.model.meshes.len(),
            v.bones.len(),
            d.models.view_skins
        );
        assert!(untextured.is_empty(), "{id}: untextured pieces {untextured:?}");
        let set = v.animations.as_ref().unwrap();
        for name in ["select", "idle_0", "fire_0"] {
            assert!(set.sequence(name).is_some(), "{id}: {name}");
        }
    }
}

/// Cases 1 and 2: the M92FS's 51 nodes (the gun's eight between the two
/// arms, under the right hand) and 5 pieces; the animation child's pieces
/// are not drawn. The hand model's side needs the V2 key.
#[test]
fn t01_t02_skeleton_and_pieces() {
    let Some(mount) = mount() else { return };
    let file = |p: &str| mount.read(p).ok();
    let m = model::load(M92FS_PATH, &file).unwrap();
    let names: Vec<&str> = m.model.nodes.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names.len(), 51);
    assert_eq!((names[0], names[23]), ("Scene Root", "Bip01 R Finger42"));
    assert_eq!((names[32], names[50]), ("Bip01 L Clavicle", "Bip01 L Finger42"));
    let gun = ["Box01", "Box05", "Box08", "Box06", "Box03", "Box10", "Box11", "Box12"];
    assert_eq!(names[24..32], gun);
    let r_hand = names.iter().position(|n| *n == "Bip01 R Hand").unwrap();
    for j in 24..32 {
        let mut p = m.model.nodes[j].parent;
        while let Some(i) = p.filter(|&i| i != r_hand) {
            p = m.model.nodes[i].parent;
        }
        assert_eq!(p, Some(r_hand), "{} under the right hand", names[j]);
    }
    let pieces: Vec<&str> = m.model.pieces.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(pieces, ["Object04", "Object05", "Object02", "Object06", "Object07"]);
    let mut map = MapData::default();
    let v = view_model::load(M92FS_PATH, M92FS, &[], POS, &mut map, &file).unwrap();
    assert_eq!(v.model.meshes.len(), 5, "the ANI child's pieces aren't drawn");
    // The hand model: keyed without the V2 key.
    let choice = hands::choose(
        &hands::builtin(),
        &hands::HandProfile {
            character: true,
            ..default()
        },
    )
    .unwrap();
    match view_model::load_hands(&choice, &mut map, &file) {
        Ok((meshes, bones)) => {
            assert_eq!(bones.len(), 43);
            assert_eq!(meshes.len(), 2);
            let map_bones = hands::match_bones(&bones, &v.bones).unwrap();
            let want: Vec<usize> = (0..24).chain(32..51).collect();
            assert_eq!(map_bones, want);
        }
        Err(e) => eprintln!("hand model not read (V2-keyed): {e}"),
    }
}

/// A socket of the M92FS in camera space (units, the game's axes) at `t`
/// ms of `anim`, by the game's own sampling (root translation included).
fn socket_exact(m: &model::LoadedModel, anim: &str, t: f32, socket: &str) -> Vec3 {
    let e = &m.list.entries[m.list.find(anim).unwrap()];
    let pose: Vec<_> = (0..m.model.nodes.len()).map(|j| lta::sample(e, j, t)).collect();
    let g = lta::globals(&m.model, &pose);
    let s = m
        .model
        .sockets
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(socket))
        .unwrap();
    Vec3::from(lta::transform_point(&g[s.node as usize], s.position)) + Vec3::from(POS)
}

/// The same as drawn: the converted model's attachment, posed by its
/// sequence at `t` ms, back in the game's units and axes.
fn socket_drawn(v: &MapViewModel, anim: &str, t: f32, socket: &str) -> Vec3 {
    let set = v.animations.as_ref().unwrap();
    let s = set.sequence(anim).unwrap();
    let mut animator = chimera_world::map::anim::Animator::new(set.clone());
    animator.main = Some(s);
    animator.cycle = (t / 1000.0 / set.duration(s)).clamp(0.0, 1.0);
    let pose = animator.pose(0.0);
    let i = v
        .attachments
        .iter()
        .position(|a| a.name.eq_ignore_ascii_case(socket))
        .unwrap();
    let m = attachment_transform(v, &pose, i, &Transform::IDENTITY).unwrap();
    let p = m.transform_point3(Vec3::ZERO) / VIEW_SCALE;
    Vec3::new(p.x, p.y, -p.z)
}

fn m92fs(mount: &chimera_core::mount::Mount) -> (model::LoadedModel, MapViewModel) {
    let file = |p: &str| mount.read(p).ok();
    let m = model::load(M92FS_PATH, &file).unwrap();
    let mut map = MapData::default();
    let v = view_model::load(M92FS_PATH, M92FS, &[], POS, &mut map, &file).unwrap();
    (m, v)
}

/// The 4:3 screen point of a camera-space point (5.1).
fn screen(p: Vec3) -> Vec2 {
    let k = 25f32.to_radians().tan();
    Vec2::new(p.x / (4.0 / 3.0 * p.z * k), p.y / (p.z * k))
}

fn close(a: Vec3, b: [f32; 3], tol: f32) -> bool {
    (a - Vec3::from(b)).length() < tol
}

#[test]
fn t04_rest_placement() {
    let Some(mount) = mount() else { return };
    let (_, v) = m92fs(&mount);
    let t = v.root.translation / VIEW_SCALE;
    assert!(close(Vec3::new(t.x, t.y, -t.z), POS, 1e-5), "{t}");
    assert_eq!(v.root.rotation, Quat::IDENTITY);
}

#[test]
fn t05_t06_t07_muzzle_socket() {
    let Some(mount) = mount() else { return };
    let (m, v) = m92fs(&mount);
    // (animation, ms, socket, camera space)
    let cases = [
        ("idle_0", 0.0, "flash", [0.1688, -0.4043, 2.3648]),
        ("idle_0", 0.0, "breach", [0.3409, -0.4217, 1.5837]),
        ("idle_0", 600.0, "flash", [0.1703, -0.4642, 2.3734]),
        ("fire_0", 0.0, "flash", [0.1688, -0.4043, 2.3648]),
        ("fire_0", 29.0, "flash", [0.2111, -0.4272, 2.2522]),
        ("fire_0", 52.0, "flash", [0.2032, -0.2787, 2.0992]),
        ("fire_0", 103.0, "flash", [0.1503, 0.0527, 1.8799]),
        ("fire_0", 180.0, "flash", [0.2164, -0.0832, 2.1452]),
        ("select", 0.0, "flash", [0.3420, -2.1896, 2.3924]),
        ("select", 650.0, "flash", [0.1688, -0.4043, 2.3648]),
    ];
    let mut bad = Vec::new();
    for (anim, t, socket, want) in cases {
        let exact = socket_exact(&m, anim, t, socket);
        let drawn = socket_drawn(&v, anim, t, socket);
        eprintln!("{anim} {t} {socket}: exact {exact}, drawn {drawn}, spec {want:?}");
        if !close(exact, want, 1e-3) {
            bad.push(format!("{anim} {t} {socket}: exact {exact} != {want:?}"));
        }
        // Drawn from the 30 fps resampling: within 0.02 units.
        if !close(drawn, want, 0.02) {
            bad.push(format!("{anim} {t} {socket}: drawn {drawn} != {want:?}"));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
    // Model space at rest (Pos taken off), and the screen points.
    let rest = socket_exact(&m, "idle_0", 0.0, "flash");
    assert!(close(rest - Vec3::from(POS), [0.2688, -0.2043, 1.6648], 1e-3));
    assert!(
        (screen(rest) - Vec2::new(0.1148, -0.3666)).length() < 1e-3,
        "{}",
        screen(rest)
    );
    let up = socket_exact(&m, "fire_0", 103.0, "flash");
    assert!(
        (screen(up) - Vec2::new(0.1286, 0.0602)).length() < 1e-3,
        "{}",
        screen(up)
    );
    assert!((screen(socket_exact(&m, "select", 0.0, "flash")).y + 1.963).abs() < 1e-3);
}

#[test]
fn t08_rifle_socket() {
    let Some(mount) = mount() else { return };
    let file = |p: &str| mount.read(p).ok();
    match model::load("guns_m_pv_ar/pv_ar_m4a1_sh.ltb", &file) {
        Ok(_) => eprintln!("the M4A1 reads: case 8 still to be written against it"),
        Err(e) => eprintln!("skipping case 8 (V2-keyed rifle archive): {e}"),
    }
}

fn playing(sim: &Sim, p: Entity) -> Option<String> {
    let vm = sim.app.world().get::<CaViewModel>(p)?;
    vm.machine.as_ref()?.playing().map(str::to_string)
}

/// Cases 22-23, 27 and 29 on the real M92FS in play (15 ms ticks): the
/// draw, a shot with its post-fire, the sprint pose, the last round's
/// reload; its sound keys fire.
#[test]
fn m92fs_in_play() {
    let Some((_, map)) = loaded() else { return };
    let m92fs = map.kit.view_models.iter().find(|v| v.key == M92FS).unwrap().clone();
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.set_tick_interval(TICK_INTERVAL);
    sim.app.insert_resource(ViewModels::of(vec![m92fs]));
    let p = sim.spawn_character(Vec3::new(30.0, 1.0, 30.0), movement::ID);
    sim.seconds(0.5);
    sim.app
        .world_mut()
        .entity_mut(p)
        .insert(Health { current: 1.0, max: 1.0 });
    let w = weapon::give(sim.app.world_mut(), p, M92FS).unwrap();
    let mut seen: Vec<String> = Vec::new();
    let run = |sim: &mut Sim, n: u32, seen: &mut Vec<String>| {
        for _ in 0..n {
            sim.ticks(1);
            if let Some(s) = playing(sim, p)
                && seen.last() != Some(&s)
            {
                seen.push(s);
            }
        }
    };
    run(&mut sim, 60, &mut seen);
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    run(&mut sim, 40, &mut seen);
    {
        let mut i = sim.intent(p);
        i.sprint = true;
        i.move_axis = Vec2::Y;
    }
    run(&mut sim, 60, &mut seen);
    {
        let mut i = sim.intent(p);
        i.sprint = false;
        i.move_axis = Vec2::ZERO;
    }
    run(&mut sim, 60, &mut seen);
    sim.app.world_mut().get_mut::<Magazine>(w).unwrap().clip = 1;
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    run(&mut sim, 200, &mut seen);
    eprintln!("M92FS: {seen:?}");
    let lower: Vec<String> = seen.iter().map(|s| s.to_lowercase()).collect();
    let want = [
        "select", "idle_0", "fire_0", "postfire", "idle_0", "prerush", "rush", "postrush", "idle_1", "fire_0",
        "postfire", "reload", "idle_0",
    ];
    let mut at = 0;
    for w in want {
        at += lower[at..]
            .iter()
            .position(|s| s == w)
            .unwrap_or_else(|| panic!("{w} after {at}: {lower:?}"))
            + 1;
    }
    assert_eq!(sim.app.world().get::<Magazine>(w).unwrap().clip, 15);
    assert_eq!(
        sim.app.world().get::<ViewAnimator>(p).unwrap().key.as_deref(),
        Some(M92FS)
    );
}

/// The M67's throw as drawn (the pin pull, the cocked loop, the throw):
/// every node where the game's own sampling puts it (within 0.1 u, the
/// 30 fps resampling), the hand model's weights summing to 1, and the
/// skinned arms and grenade intact at every 10 ms: no edge more than 4
/// times its bind length (the fingers' small edges reach about 3), no
/// vertex more than 10 u from the eye. The arm that sweeps past the eye
/// in the throw (a large sleeve on screen for a few frames) is the stock
/// animation, not a bad blend or skeleton.
#[test]
fn m67_throw_poses_follow_the_data() {
    let Some(mount) = mount() else { return };
    if !(combat_arms::game::mount::can_read("V1") && combat_arms::game::mount::can_read("V2")) {
        eprintln!("skipping: needs the V1 and V2 keys");
        return;
    }
    let ca = combat_arms::game::data::CaData::install();
    let file = |p: &str| mount.read(p).ok();
    let mut map = MapData::default();
    view_model::add_view_models(&mut map, &ca, &file);
    let v = map
        .kit
        .view_models
        .iter()
        .find(|v| v.key == "combat_arms:m67")
        .expect("the M67's view model");
    let models = &ca.weapon_set().get("combat_arms:m67").unwrap().models;
    let (path, pos) = (
        models.view.clone().unwrap(),
        Vec3::from(models.view_offset.unwrap_or_default()),
    );
    let m = model::load(&path, &file).unwrap();
    let set = v.animations.as_ref().unwrap();
    for mesh in &v.model.meshes {
        assert!(
            mesh.joint_weights
                .iter()
                .all(|w| (w.iter().sum::<f32>() - 1.0).abs() < 1e-3),
            "{}: weights sum to 1",
            mesh.material
        );
    }
    let inverse: Vec<Mat4> = hands::globals(&v.bones).iter().map(|g| g.inverse()).collect();
    let (root, root_inv) = (v.root.to_matrix(), v.root.to_matrix().inverse());
    let mut bad = Vec::new();
    for anim in ["prefire", "fire_0", "postfire"] {
        let e = &m.list.entries[m.list.find(anim).unwrap()];
        let s = set.sequence(anim).unwrap();
        let length = e.length() as f32;
        let mut t = 0.0;
        while t <= length {
            // The game's own sampling.
            let pose: Vec<_> = (0..m.model.nodes.len()).map(|j| lta::sample(e, j, t)).collect();
            let exact = lta::globals(&m.model, &pose);
            // As drawn.
            let mut animator = chimera_world::map::anim::Animator::new(set.clone());
            animator.main = Some(s);
            animator.cycle = (t / 1000.0 / set.duration(s)).clamp(0.0, 1.0);
            let mut drawn: Vec<Mat4> = Vec::with_capacity(v.bones.len());
            for (b, (q, p)) in v.bones.iter().zip(animator.pose(0.0)) {
                let local = Mat4::from_rotation_translation(q, p);
                drawn.push(b.parent.map_or(local, |p| drawn[p] * local));
            }
            for (j, node) in m.model.nodes.iter().enumerate() {
                let want = Vec3::from(lta::transform_point(&exact[j], [0.0; 3]));
                let d = (root * drawn[j]).transform_point3(Vec3::ZERO) / VIEW_SCALE;
                let got = Vec3::new(d.x, d.y, -d.z) - pos;
                if (got - want).length() > 0.1 {
                    bad.push(format!("{anim} {t} ms {}: drawn {got} != {want}", node.name));
                }
            }
            for mesh in &v.model.meshes {
                let posed: Vec<Vec3> = mesh
                    .positions
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let p = root_inv.transform_point3(Vec3::from(*p));
                        let blend = mesh.joints[i]
                            .iter()
                            .zip(mesh.joint_weights[i])
                            .map(|(j, w)| (drawn[*j as usize] * inverse[*j as usize]).transform_point3(p) * w)
                            .sum::<Vec3>();
                        root.transform_point3(blend)
                    })
                    .collect();
                if let Some(far) = posed.iter().find(|p| p.length() / VIEW_SCALE > 10.0) {
                    bad.push(format!(
                        "{anim} {t} ms {}: a vertex {} u from the eye",
                        mesh.material,
                        far.length() / VIEW_SCALE
                    ));
                }
                for tri in mesh.indices.chunks(3) {
                    for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
                        let (a, b) = (a as usize, b as usize);
                        let rest = (Vec3::from(mesh.positions[a]) - Vec3::from(mesh.positions[b])).length();
                        let now = (posed[a] - posed[b]).length();
                        if rest > 1e-4 && now > 4.0 * rest {
                            bad.push(format!("{anim} {t} ms {}: edge {a}-{b} {rest} -> {now}", mesh.material));
                        }
                    }
                }
            }
            t += 10.0;
        }
    }
    bad.truncate(20);
    assert!(bad.is_empty(), "{bad:#?}");
}

/// Every rung body's first-person hands (`hands::choose_in`), as itself
/// and infected (`CharacterData::infected_look`: the override's hand
/// record), load from the install with every piece textured: the
/// infected's sleeve fell back to the placeholder `Hands` and drew grey
/// (feedback 42750262: the Quarantine hand's arms).
#[test]
fn every_rung_looks_hands_load_textured() {
    let Some(mount) = mount() else { return };
    if !(combat_arms::game::mount::can_read("V1") && combat_arms::game::mount::can_read("V2")) {
        eprintln!("skipping: needs the V1 and V2 keys");
        return;
    }
    let ca = combat_arms::game::data::CaData::install();
    if ca.attributes().is_none() {
        eprintln!("skipping: the install's attribute data doesn't read");
        return;
    }
    let file = |p: &str| mount.read(p).ok();
    let ch = ca.characters();
    let mut bad = Vec::new();
    let mut checked = 0;
    for body in ch.rung_bodies() {
        // A body the install lacks is left out of the rungs (`rungs`).
        if ch.model(body).is_none_or(|m| mount.read(&m.file).is_err()) {
            continue;
        }
        for (what, look) in [("own", ch.default_look(body)), ("infected", ch.infected_look(body))] {
            let profile = hands::profile_of(ch, &look);
            let Some(choice) = hands::choose_in(ch, &profile) else { continue };
            checked += 1;
            let mut map = MapData::default();
            match view_model::load_hands(&choice, &mut map, &file) {
                Ok((meshes, _)) => {
                    let untextured: Vec<&str> = meshes.iter().filter(|m| m.texture.is_none()).map(|m| m.material.as_str()).collect();
                    if !untextured.is_empty() || meshes.is_empty() {
                        bad.push(format!("body {body} {what} (face {}): {choice:?}: untextured {untextured:?}", look.face));
                    }
                }
                Err(e) => bad.push(format!("body {body} {what} (face {}): {choice:?}: {e}", look.face)),
            }
        }
    }
    eprintln!("{checked} looks' hands checked");
    for b in &bad {
        eprintln!("{b}");
    }
    assert!(bad.is_empty(), "{} looks' hands don't load whole", bad.len());
}

/// What the install's data leaves untextured in first person, by view
/// model file and piece skin (the mesh's material: "" for a piece on
/// texture slot 0, whose skin is `PVSkin0`, the placeholder `Hands`: a
/// name without `.dtx` never resolves, textures_dtx.md "How other files
/// name a texture", so the game leaves the slot empty too, view_models.md
/// 2). Observed on the dev box's install (Steam build 26236938,
/// 2026-11-21); the game leaves an unresolved skin's slot empty
/// (textures_dtx.md), so these stay untextured by parity.
const UNTEXTURED_IN_THE_DATA: &[(&str, &str, &str)] = &[
    ("guns_m_pv_ar/pv_ar_tar21_camo_mark2_sh.ltb", "", "TAR-21 camo Mark2: piece MARK is on slot 0 (`Hands`)"),
    ("guns_m_pv_ar/pv_ar_tar21_mark2_sh.ltb", "", "TAR-21 Mark2: piece MARK is on slot 0 (`Hands`)"),
    ("guns_m_pv_melee/pv_melee_qrt_subtype_s_sh.ltb", "", "Quarantine subtype S hand: both pieces on slot 0 (`Hands`)"),
    ("guns_m_hh/cw_zhnd_theshining.ltb", "", "The Shining hand: its one piece is on slot 0 (`Hands`)"),
    ("guns_m_pv_sg/pv_aa-12_aa-12 holyfire.ltb", "", "AA-12 Holy Fire: its one piece is on slot 0 (`Hands`)"),
    ("guns_m_pv_sr/pv_optical buster.ltb", "", "Optical Buster (and Apollo, Photon): piece Magazine is on slot 0 (`Hands`)"),
    (
        "guns_m_pv_ar/pv_ar_mk18_mod1_ki.ltb",
        "guns_t_pv_ar/pv_ar_mk18_mod1_camo.dtx",
        "MK18 MOD1 camo: no such texture (only the shop icon ui_scr_itemimage/weapon/mk18_mod1_camo.dtx)",
    ),
    (
        "guns_m_pv_sr/pv_sr_sig50_ke.ltb",
        "guns_t_pv_ar/pv_opt_dt_scope_sambaqueen.dtx",
        "SIG50 Samba Queen's scope: the file is in GUNS_T_PV_SR, the data names GUNS_T_PV_AR",
    ),
    (
        "guns_m_pv_smg/pv_smg_p90tr_sh.ltb",
        "guns_t_pv_smg/pv_smg__p90tr2_tan.dtx",
        "Retro Summer P90TR: no such texture (nothing named p90tr2 anywhere)",
    ),
    (
        "guns_m_pv_smg/pv_smg_magpul_fmg9_sh.ltb",
        "guns_t_pv_smg/pv_smg_lightning fmg-9.spr",
        "Lightning star FMG-9: no such sprite (the frames' .dtx are there; the held one is hh_smg_lightningfmg-9.spr)",
    ),
    (
        "guns_m_pv_ar/pv_sr_m4a1_sniper rifle.ltb",
        "guns_t_pv_ar/pv_sr_m4a1_sniperrifle0.dtx",
        "Advanced star M4A1: no such texture (GUNS_T_PV_AR has `PV_SR_M4A1_Sniper Rifle.dtx`)",
    ),
    (
        "guns_m_pv_ar/pv_sr_m4a1_sniper rifle.ltb",
        "guns_t_pv_ar/pv_sr_m4a1_sniperrifle.spr",
        "Advanced starshing M4A1: no frame of the sprite (`Sniper Rifle0..2.dtx`) is in GUNS_T_PV_AR (only GUNS_T_HH's)",
    ),
    (
        "guns_m_pv_ar/pv_aug_a3_champions_league.ltb",
        "guns_t_pv_ar/pv_aug_a3_champions_lighting.dtx",
        "AUG A3 Champions Lightning: no such texture (only the held one; models_ltb.md Open question 8)",
    ),
];

/// View model files the install's data names that aren't there.
const MISSING_VIEW_MODELS: &[(&str, &str)] = &[(
    "guns_m_launcher/pv_lch_m134_minigun_sh.ltb",
    "Abnormal Fortress fire target: the file is in GUNS_M_PV_LAUNCHER",
)];

/// With both keys: every install weapon's view model (each model file and
/// skin list once) loads on demand with its hands, every piece textured,
/// but for what the data itself leaves untextured (`UNTEXTURED_IN_THE_DATA`,
/// `MISSING_VIEW_MODELS`). Skips without the install (the asset store
/// alone holds only what was packed).
#[test]
fn every_install_view_model_is_textured() {
    let Some(mount) = mount() else { return };
    let install = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)).unwrap();
    if chimera_core::mount::served::alone("combat_arms", &install) {
        eprintln!("skipping: the asset store stands in for the install");
        return;
    }
    if !(combat_arms::game::mount::can_read("V1") && combat_arms::game::mount::can_read("V2")) {
        eprintln!("skipping: needs the V1 and V2 keys");
        return;
    }
    let ca = combat_arms::game::data::CaData::install();
    if ca.attributes().and_then(|a| a.weapons.as_ref()).is_none() {
        eprintln!("skipping: the install's weapons file doesn't read");
        return;
    }
    let file = |p: &str| mount.read(p).ok();
    let mut seen = std::collections::HashSet::new();
    let mut used = std::collections::HashSet::new();
    let mut bad = Vec::new();
    for w in &ca.weapon_set().weapons {
        let Some(path) = w.models.view.as_deref() else { continue };
        if !seen.insert((path.to_string(), w.models.view_skins.clone())) {
            continue;
        }
        let loaded = match view_model::on_demand::load_weapon(&ca, &w.id, &file) {
            Ok(l) => l,
            Err(e) => {
                if !MISSING_VIEW_MODELS.iter().any(|(p, _)| *p == path) {
                    bad.push(e);
                }
                continue;
            }
        };
        bad.extend(loaded.warnings.iter().map(|e| format!("{}: {e}", w.id)));
        let names = &loaded.model.model.mesh_names;
        for (i, m) in loaded.model.model.meshes.iter().enumerate().filter(|(_, m)| m.texture.is_none()) {
            if UNTEXTURED_IN_THE_DATA.iter().any(|(p, s, _)| *p == path && *s == m.material) {
                used.insert((path.to_string(), m.material.clone()));
                continue;
            }
            bad.push(format!(
                "{} ({path}): piece {:?} untextured, skin {:?}",
                w.id,
                names.get(i),
                m.material
            ));
        }
    }
    eprintln!("{} view models", seen.len());
    // An install of another build may have fixed some: said, not failed.
    for (p, s, note) in UNTEXTURED_IN_THE_DATA {
        if !used.contains(&(p.to_string(), s.to_string())) {
            eprintln!("note: textured now: {note}");
        }
    }
    bad.truncate(30);
    assert!(bad.is_empty(), "{bad:#?}");
}
