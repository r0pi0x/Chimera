//! Combat Arms' models from the user's install (`game::model`,
//! `game::props`): the headquarters flag (skinned, 111 keys) converts and
//! animates through Chimera's `AnimSet` as LithTech's sampling says; the
//! M92FS view model with its animation child converts; capture-the-flag
//! maps place their flags only with the V2 key; the development prop
//! goes in front of the first spawn. Passes with a note without
//! `[games.combat_arms] path` in chimera.local.toml.

use bevy::prelude::*;
use chimera_core::mount::config::LocalConfig;
use chimera_world::map::anim::BonePose;
use combat_arms::game::{GAME, INFO, model, movement, props, world};
use lithtech_formats::anim as lta;

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

/// The headquarters flag: its bones, skinned mesh and animation, sampled
/// by Chimera at the times LithTech's sampling gives, agree.
#[test]
fn hq_flag_converts_and_animates() {
    let Some(mount) = mount() else { return };
    let file = |p: &str| mount.read(p).ok();
    let loaded = model::load("PROPS\\HEADQUATERS\\HQ_FLAG01.LTB", &file).unwrap();
    let scale = movement::movement_data().meters_per_unit;
    let m = model::convert(&loaded, scale, &|_| String::new());
    let rig = m.rig.as_ref().unwrap();
    assert_eq!(rig.bones.len(), 5);
    assert_eq!(m.meshes.len(), 1);
    let mesh = &m.meshes[0];
    assert_eq!((mesh.positions.len(), mesh.indices.len()), (56, 180));
    // Bone sets: nodes 2-4 and 1-3; every vertex's weights sum to 1.
    assert!(mesh.joints.iter().flatten().all(|&j| j < 5));
    for w in &mesh.joint_weights {
        assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-4);
    }
    let set = &rig.animations;
    assert_eq!(set.sequences.len(), 1);
    assert_eq!(set.sequences[0].name, "HQ_flag01");
    let entry = &loaded.list.entries[0];
    let length = entry.length() as f32;
    let a = &set.animations[0];
    assert_eq!(a.frames, (length / 1000.0 * model::SAMPLE_FPS).ceil() as usize + 1);
    // At every resampled frame Chimera's pose is LithTech's sample,
    // converted; and the flag moves.
    let mut pose: Vec<BonePose> = set.defaults.clone();
    let mut moved = 0.0f32;
    for f in 0..a.frames {
        let cycle = f as f32 / (a.frames - 1) as f32;
        set.sample(0, cycle, &mut pose);
        for j in 1..5 {
            let (p, q) = lta::sample(entry, j, length * cycle);
            let want_p = Vec3::new(p[0], p[1], -p[2]);
            let want_q = Quat::from_xyzw(-q[0], -q[1], q[2], q[3]).normalize();
            assert!(pose[j].1.abs_diff_eq(want_p, 1e-3), "frame {f} node {j}");
            assert!(pose[j].0.angle_between(want_q) < 1e-3, "frame {f} node {j}");
            moved = moved.max((pose[j].0 * Vec3::X - set.defaults[j].0 * Vec3::X).length());
        }
    }
    eprintln!("hq flag: {} frames at {} fps, largest axis change from bind {moved}", a.frames, a.fps);
    assert!(moved > 0.05);
}

/// The M92FS first-person model (unkeyed) with its `ANI_` child: 14
/// sequences on 51 bones, its five rigid pieces.
#[test]
fn m92fs_view_model_converts() {
    let Some(mount) = mount() else { return };
    let file = |p: &str| mount.read(p).ok();
    let loaded = model::load("GUNS_M_PV_Pistol\\PV_PST_M92FS_SH.LTB", &file).unwrap();
    let m = model::convert(&loaded, 0.01, &|_| String::new());
    let rig = m.rig.as_ref().unwrap();
    assert_eq!((rig.bones.len(), m.meshes.len(), rig.animations.sequences.len()), (51, 5, 14));
    assert_eq!(rig.animations.sequence("select"), Some(1));
    // `select` is 650 ms.
    assert!((rig.animations.duration(1) - 0.65).abs() < 1e-3);
}

/// Capture-the-flag maps name their flags' model (`FlagItem`
/// `Filename`), in the keyed ATTACH_M: placed with the V2 key, skipped
/// with a warning without it.
#[test]
fn ctf_flags_need_the_v2_key() {
    let Some(mount) = mount() else { return };
    let map = world::load_from(&mount, "junk_flea2").unwrap();
    let flags = map
        .entities
        .iter()
        .filter(|e| e.classname() == "FlagItem")
        .count();
    let warned = map.warnings.iter().any(|w| w.starts_with("FlagItem model:"));
    eprintln!("junk_flea2: {flags} flags, warned {warned}");
    if mount.read("attach_m/flag01_ground.ltb").is_ok() {
        assert!(flags >= 2 && !warned);
    } else {
        assert!(flags == 0 && warned);
    }
}

/// `props::dev_prop` puts SNOWSTORM's waving flag (an effect model of
/// the map) 3 m ahead of the first spawn, lit, with its texture.
#[test]
fn dev_prop_in_front_of_the_spawn() {
    let Some(mount) = mount() else { return };
    let file = |p: &str| mount.read(p).ok();
    let mut map = world::load_from(&mount, "snowstorm").unwrap();
    let (props_before, textures_before) = (map.props.len(), map.textures.len());
    let bytes = mount.read(&world::find(&mount, "snowstorm").unwrap()).unwrap();
    let w = lithtech_formats::world::World::parse(&bytes).unwrap();
    props::dev_prop(
        &w,
        &mut map,
        "fx/lv_used/snowstorm/ss_flag01.ltb, fx/lv_used/snowstorm/ss_flag.dtx",
        movement::movement_data().meters_per_unit,
        &file,
    );
    assert_eq!(map.props.len(), props_before + 1);
    let p = map.props.last().unwrap();
    assert!((p.translation - map.spawns[0].0).length() > 2.9);
    assert!(map.models[p.model].rig.is_some());
    let m = &map.models[p.model];
    let rig = m.rig.as_ref().unwrap();
    eprintln!(
        "dev prop at {}: {} meshes, {} bones, sequences {:?}",
        p.translation,
        m.meshes.len(),
        rig.bones.len(),
        rig.animations.sequences.iter().map(|s| (&s.name, rig.animations.duration(0))).collect::<Vec<_>>()
    );
    assert_eq!(map.textures.len(), textures_before + 1);
    assert!(m.meshes.iter().all(|m| m.texture.is_some()));
}
