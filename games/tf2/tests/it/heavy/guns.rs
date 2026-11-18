//! TF2's guns from the user's install (M2b): each gun's view model, the
//! class's arms with its `c_` weapon bone-merged (`view::load`; and on
//! each other class's arms that can hold it, `view::CLASS_SEQUENCES`), with the
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
    maps::add_guns(&mut materials, &|_| true, true, &mut kit, &mut warnings);
    assert!(warnings.is_empty(), "{warnings:?}");
    // Each gun on its own class's arms, and on each other class's that
    // can hold it (the Shotgun's three, the Pistol's Engineer).
    assert_eq!(kit.view_models.len(), weapons::GUNS.len() + view::CLASS_SEQUENCES.len());
    for gun in weapons::GUNS {
        assert!(kit.view_models.iter().any(|v| v.key == gun), "{gun}'s view model");
    }
    for (g, c, _) in view::CLASS_SEQUENCES {
        let key = chimera_sim::weapon::view_anim::variant_key(g, c);
        assert!(kit.view_models.iter().any(|v| v.key == key), "{key}'s view model");
    }
    for v in &kit.view_models {
        let gun = v.key.split_once('#').map_or(v.key.as_str(), |(g, _)| g);
        let set = v.animations.as_ref().expect("the arms' animations");
        assert!(v.attachments.iter().any(|a| a.name.eq_ignore_ascii_case("muzzle")), "{gun}: a muzzle");
        let q = view::sequences(&v.key).unwrap();
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
        assert!(weapons::GUNS.contains(&gun.as_str()) || weapons::MELEE.contains(&gun.as_str()), "{gun}: the set is named after the gun or the melee weapon");
        let set = sets::weapon_files(&mount, w, &scripts);
        // A match's load of this gun: on another game's map (the guest
        // dressing) and on TF2's own.
        let tracking = mount.track();
        let mut materials = MaterialLoader::guest(&mount, 0, 0);
        let (mut kit, mut warnings) = (chimera_world::map::GameKit::default(), Vec::new());
        maps::add_guns(&mut materials, &|g| g == gun, false, &mut kit, &mut warnings);
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
                Access::Folder(_) | Access::Held(_) => None,
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
    maps::add_guns(&mut materials, &|_| true, true, &mut kit, &mut warnings);
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
        // Its muzzle ahead and below the eye, but the Minigun's: it rests
        // upright in `m_idle`, barrels up, as the real game holds it
        // (view.rs).
        let m = muzzle.expect("a muzzle");
        assert!(v.key == tf2::game::weapons::MINIGUN || (m.y < 0.0 && m.z < -0.5), "{}: the muzzle at {m}", v.key);
    }
}

/// specs/tf2/viewmodel.md's test cases 1, 2, 6 and 7 to 17: each gun's
/// muzzle in the eye's space (forward, right, up, units) at frame 0 of its
/// idle (the spun Minigun's `m_spool_idle` too), the view model at the
/// eye with no offset, the weapon bone-merged; within 1 unit.
#[test]
fn the_muzzles_sit_where_the_spec_puts_them() {
    use bevy::prelude::*;
    use tf2::game::weapons::*;
    let Some(mount) = install_mount() else { return };
    let mut materials = MaterialLoader::guest(&mount, 0, 0);
    let (mut kit, mut warnings) = (chimera_world::map::GameKit::default(), Vec::new());
    maps::add_guns(&mut materials, &|_| true, true, &mut kit, &mut warnings);
    let cases: &[(&str, &str, [f32; 3])] = &[
        (MINIGUN, "m_idle", [55.32, 16.82, 6.78]),
        (MINIGUN, "m_spool_idle", [66.56, 4.86, -20.54]),
        (SCATTERGUN, "sg_idle", [44.29, 5.22, -4.28]),
        (PISTOL_SCOUT, "p_idle", [37.36, 5.06, -2.60]),
        (ROCKET_LAUNCHER, "dh_idle", [24.23, 10.78, -5.28]),
        (SHOTGUN, "fj_idle", [52.32, 8.06, -6.38]),
        (FLAMETHROWER, "ft_idle", [60.57, 10.80, -7.29]),
        (GRENADE_LAUNCHER, "g_idle", [58.16, 4.78, -5.08]),
        (STICKY_LAUNCHER, "sb_idle", [52.60, 8.56, -5.67]),
        (SNIPER_RIFLE, "idle", [69.45, 8.98, -7.44]),
        (SMG, "smg_idle", [52.93, 9.60, -6.10]),
        (SYRINGE_GUN, "sg_idle", [55.73, 8.11, -6.50]),
        (REVOLVER, "idle", [43.57, 6.08, -3.51]),
    ];
    let mut wrong = Vec::new();
    for (gun, sequence, want) in cases {
        let v = kit.view_models.iter().find(|v| v.key == *gun).unwrap_or_else(|| panic!("{gun}: no view model"));
        let set = v.animations.clone().unwrap();
        let mut a = chimera_world::map::anim::Animator::new(set.clone());
        a.play(set.sequence(sequence).unwrap_or_else(|| panic!("{gun}: no {sequence}")), 0.0);
        let pose = a.pose(0.0);
        let mut posed: Vec<Mat4> = Vec::new();
        for (i, b) in v.bones.iter().enumerate() {
            let local = Mat4::from_rotation_translation(pose[i].0, pose[i].1);
            posed.push(match b.parent {
                Some(p) => posed[p] * local,
                None => local,
            });
        }
        let root = v.root.to_matrix();
        let at = v.attachments.iter().find(|a| a.name == "muzzle").expect("a muzzle");
        let m = (root * posed[at.bone] * at.local.to_matrix()).transform_point3(Vec3::ZERO) / tf2::game::METERS_PER_UNIT;
        // Camera space (right, up, back) to the spec's (forward, right, up).
        let got = [-m.z, m.x, m.y];
        let off = (0..3).map(|i| (got[i] - want[i]).abs()).fold(0.0, f32::max);
        eprintln!("{gun} {sequence}: muzzle {got:?}, the spec's {want:?}");
        if off > 1.0 {
            wrong.push(format!("{gun} {sequence}: {got:?}, not {want:?}"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// Every sound a TF2 gun's data names (`weapons/*.toml`) is in the table a
/// guest dressing hands another game's map under TF2's prefix, with as many
/// waves as TF2's own script gives it (not another game's entry of the
/// same name: the Scout's pistol fired with HL2:DM's `Weapon_Pistol.Single`),
/// and the bare script names stay out of it.
#[test]
fn every_guns_sound_is_tf2s_own() {
    let Some(mount) = install_mount() else { return };
    let scripts = sets::sound_scripts(&mount);
    let mut map = chimera_world::map::MapData::default();
    maps::dress_guest_with(&mut map, tf2::game::BUNDLE.kept, false).unwrap();
    let mut named = std::collections::BTreeSet::new();
    for f in std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/data/weapons")).unwrap().flatten() {
        let text = std::fs::read_to_string(f.path()).unwrap();
        for part in text.lines().map(|l| l.split('#').next().unwrap_or("")).flat_map(|l| l.split('"').skip(1).step_by(2).map(str::to_string).collect::<Vec<_>>()) {
            if let Some(bare) = part.strip_prefix("tf2:") && bare.contains('.') && !bare.contains('/') {
                named.insert(bare.to_string());
            }
        }
    }
    assert!(named.len() > 80, "the weapon data names its sounds");
    for bare in &named {
        let own = maps::own_sound(bare);
        let want = scripts.waves(bare);
        assert!(!want.is_empty(), "{bare}: TF2's script has no waves");
        let entry = map.sounds.entry(&own).unwrap_or_else(|| panic!("{own}: no entry in the table a match plays from"));
        assert!(!entry.waves.is_empty(), "{own}: no clips");
        assert!(map.sounds.entry(bare).is_none(), "{bare}: a bare name would meet another game's");
    }
}

/// Each projectile model's nose (`MapHeldModel::nose`, the model's own
/// +X wherever its bone frame turned it) is its long axis in the shown
/// frame for the long ones (the rocket, the needle: a needle flew sideways
/// when the data named its axis, playtest 2026-11-18), so a flyer turned
/// by it flies nose first.
#[test]
fn each_projectile_models_nose_is_its_long_axis() {
    let Some(mount) = install_mount() else { return };
    let mut materials = MaterialLoader::guest(&mount, 0, 0);
    let (mut kit, mut warnings) = (chimera_world::map::GameKit::default(), Vec::new());
    maps::add_guns(&mut materials, &|_| true, false, &mut kit, &mut warnings);
    let root = bevy::prelude::Transform::from_rotation(
        bevy::prelude::Quat::from_rotation_y(std::f32::consts::FRAC_PI_2) * bevy::prelude::Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
    )
    .with_scale(bevy::prelude::Vec3::splat(0.0254));
    for (_, key, _) in weapons::PROJECTILES {
        let held = kit.held.iter().find(|h| h.key == *key).unwrap_or_else(|| panic!("{key}"));
        let a = chimera_world::map::loose::asset(held, root);
        let nose = a.nose.unwrap_or_else(|| panic!("{key}: a nose"));
        let long = a.half.max_element();
        let axis = bevy::prelude::Vec3::select(a.half.cmpeq(bevy::prelude::Vec3::splat(long)), bevy::prelude::Vec3::ONE, bevy::prelude::Vec3::ZERO);
        println!("{key}: nose {nose}, half {}", a.half);
        if *key == weapons::ROCKET_SHOWN || *key == "tf2:syringe" {
            assert!(nose.dot(axis.normalize()).abs() > 0.95, "{key}: nose {nose}, half {}", a.half);
        }
    }
}
