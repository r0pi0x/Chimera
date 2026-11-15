//! TF2's guns from the user's install (M2b): each gun's view model, the
//! class's arms with its `c_` weapon bone-merged (`view::load`), with the
//! spec's sequences (8.2) by name with their activities (the include's, not the
//! arms' stubs) and a muzzle on the gun, the posed gun in front of the eye;
//! the held models and the rocket's model; the guns' sound entries in the
//! sound scripts; and each gun's `weapon/<id>` set covering everything a
//! match loads for it (`maps::add_guns` and its sounds): a client with the
//! set fetches nothing for the gun mid-match. Passes with a note without
//! the install.

use chimera_core::mount::Access;
use source_engine::content::material::MaterialLoader;
use tf2::game::{maps, sets, view, weapons};

use super::maps::install_mount;

#[test]
fn each_gun_shows_on_its_class_arms_with_the_specs_sequences() {
    let Some(mount) = install_mount() else { return };
    let mut materials = MaterialLoader::guest(&mount, 0, 0);
    let (mut kit, mut warnings) = (chimera_world::map::GameKit::default(), Vec::new());
    maps::add_guns(&mut materials, &|_| true, &mut kit, &mut warnings);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(kit.view_models.len(), weapons::GUNS.len());
    for gun in weapons::GUNS {
        let v = kit.view_models.iter().find(|v| v.key == gun).unwrap_or_else(|| panic!("{gun}'s view model"));
        let set = v.animations.as_ref().expect("the arms' animations");
        assert!(v.attachments.iter().any(|a| a.name.eq_ignore_ascii_case("muzzle")), "{gun}: a muzzle");
        let q = view::sequences(gun).unwrap();
        let named = [Some(q.draw), Some(q.idle), Some(q.fire), q.reload_start, q.reload_loop, q.reload_end, q.spin_up, q.spin_idle, q.spin_down, q.charge];
        for (activity, name) in named.into_iter().flatten() {
            let i = set.sequence(name).unwrap_or_else(|| panic!("{gun}: no sequence {name}"));
            let s = &set.sequences[i];
            assert!(s.activity.eq_ignore_ascii_case(activity), "{gun}: {name} is {}, the spec says {activity}", s.activity);
            assert!(set.animations[s.anims[0]].frames > 1, "{gun}: {name} is the include's, not a stub");
        }
        // The weapon's meshes on top of the arms'.
        assert!(v.model.meshes.len() >= 2, "{gun}: arms and gun");
        assert!(kit.held.iter().any(|h| h.key == gun), "{gun}: held");
    }
    for (_, key, _) in weapons::PROJECTILES {
        assert!(kit.held.iter().any(|h| h.key == *key), "{key}");
    }
    let scripts = sets::sound_scripts(&mount);
    for entry in maps::weapon_sounds() {
        assert!(!scripts.waves(&entry).is_empty(), "{entry}: no waves");
    }
}

#[test]
fn each_guns_set_covers_what_a_match_loads_for_it() {
    let Some(mount) = install_mount() else { return };
    let scripts = sets::sound_scripts(&mount);
    for w in sets::M2_WEAPONS {
        let gun = format!("tf2:{}", w.id);
        assert!(weapons::GUNS.contains(&gun.as_str()), "{gun}: the set is named after the gun");
        let set = sets::weapon_files(&mount, w, &scripts);
        // A match's load of this gun: on another game's map (the guest
        // dressing) and on TF2's own.
        let tracking = mount.track();
        let mut materials = MaterialLoader::guest(&mount, 0, 0);
        let (mut kit, mut warnings) = (chimera_world::map::GameKit::default(), Vec::new());
        maps::add_guns(&mut materials, &|g| g == gun, &mut kit, &mut warnings);
        for name in w.sounds {
            for wave in scripts.waves(name) {
                let _ = mount.read(&wave);
            }
        }
        let read: Vec<String> = tracking
            .finish()
            .into_iter()
            .filter_map(|a| match a {
                Access::File(p) | Access::Every(p) => Some(p),
                Access::Folder(_) => None,
            })
            .filter(|p| mount.contains(p))
            .collect();
        let uncovered: Vec<&String> = read.iter().filter(|p| !set.contains(p)).collect();
        assert!(uncovered.is_empty(), "weapon/{}: not in the set: {uncovered:?}", w.id);
        assert!(set.contains(&tf2::game::scripts::path_of(w.script)), "weapon/{}: its script", w.id);
    }
}

/// Each gun's idle on its class's arms puts the gun ahead of the eye
/// (camera space, -Z ahead), to the right and below: the arms' animation and the
/// bone merge (`map::merge_by_bone_name`) together.
#[test]
fn the_posed_gun_is_in_front_of_the_eye() {
    use bevy::prelude::*;
    let Some(mount) = install_mount() else { return };
    let mut materials = MaterialLoader::guest(&mount, 0, 0);
    let (mut kit, mut warnings) = (chimera_world::map::GameKit::default(), Vec::new());
    maps::add_guns(&mut materials, &|_| true, &mut kit, &mut warnings);
    for v in &kit.view_models {
        let set = v.animations.clone().unwrap();
        let mut a = chimera_world::map::anim::Animator::new(set.clone());
        let q = view::sequences(&v.key).unwrap();
        a.play(set.sequence(q.idle.1).unwrap(), 0.0);
        let pose = a.pose(0.1);
        let reference = chimera_world::map::reference_world(&v.bones);
        let mut posed: Vec<Mat4> = Vec::new();
        for (i, b) in v.bones.iter().enumerate() {
            let local = Mat4::from_rotation_translation(pose[i].0, pose[i].1);
            posed.push(match b.parent {
                Some(p) => posed[p] * local,
                None => local,
            });
        }
        let root = v.root.to_matrix();
        // The gun's meshes come after the arms'.
        let gun = v.model.meshes.last().unwrap();
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for (k, p) in gun.positions.iter().enumerate() {
            let j = gun.joints[k][0] as usize;
            let x = (root * posed[j] * reference[j].inverse() * root.inverse()).transform_point3(Vec3::from(*p));
            lo = lo.min(x);
            hi = hi.max(x);
        }
        let centre = (lo + hi) / 2.0;
        eprintln!("{}: the gun from {lo} to {hi}", v.key);
        if let Some(wb) = v.bones.iter().position(|b| b.name.eq_ignore_ascii_case("weapon_bone")) {
            let m = root * posed[wb];
            let z = m.transform_vector3(Vec3::Z).normalize();
            let y = m.transform_vector3(Vec3::Y).normalize();
            eprintln!("  weapon_bone at {} +Z {z} +Y {y}; parent {:?}", m.transform_point3(Vec3::ZERO), v.bones[wb].parent.map(|p| v.bones[p].name.clone()));
        }
        let mut muzzle = None;
        for a in &v.attachments {
            if a.name == "muzzle" {
                let m = root * posed[a.bone] * a.local.to_matrix();
                eprintln!("  muzzle on {} at {}", v.bones[a.bone].name, m.transform_point3(Vec3::ZERO));
                muzzle = Some(m.transform_point3(Vec3::ZERO));
            }
        }
        // Its front ahead (the Rocket Launcher rests on the shoulder and
        // reaches behind the eye), right of the view's centre, below it.
        assert!(lo.z < -0.3 && centre.x > 0.0 && centre.y < 0.0, "{}: the gun from {lo} to {hi}", v.key);
        // Its muzzle ahead and below the eye: the Minigun's `m_idle` held
        // it upright, the muzzle above the eye (view.rs).
        let m = muzzle.expect("a muzzle");
        assert!(m.y < 0.0 && m.z < -0.5, "{}: the muzzle at {m}", v.key);
    }
}
