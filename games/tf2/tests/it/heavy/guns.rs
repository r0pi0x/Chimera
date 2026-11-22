//! TF2's guns from the user's install (M2b): each gun's view model, the
//! class's arms with its `c_` weapon bone-merged (`view::load`; and on
//! each other class's arms that can hold it, `view::CLASS_SEQUENCES`), with the
//! spec's sequences (8.2) by name with their activities (the include's, not the
//! arms' stubs) and a muzzle on the gun, the posed gun in front of the eye;
//! each melee weapon's view model with its sequences (M4b);
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
    // can hold it (the Shotgun's three, the Pistol's Engineer); each melee
    // weapon on its class's arms (the Fists the arms alone).
    assert_eq!(kit.view_models.len(), weapons::GUNS.len() + view::CLASS_SEQUENCES.len() + weapons::MELEE.len());
    for melee in weapons::MELEE {
        let v = kit.view_models.iter().find(|v| v.key == melee).unwrap_or_else(|| panic!("{melee}'s view model"));
        let set = v.animations.as_ref().expect("the arms' animations");
        let q = view::melee_sequences(melee).unwrap();
        let backstab = q.backstab.iter().flatten();
        for name in [q.draw, q.idle, q.crit].into_iter().chain(q.swing.iter().copied()).chain(q.swing_alt.iter().copied()).chain(backstab.copied()) {
            assert!(set.sequence(name).is_some(), "{melee}: no sequence {name}");
        }
    }
    for gun in weapons::GUNS {
        assert!(kit.view_models.iter().any(|v| v.key == gun), "{gun}'s view model");
    }
    for (g, c, _) in view::CLASS_SEQUENCES {
        let key = chimera_sim::weapon::view_anim::variant_key(g, c);
        assert!(kit.view_models.iter().any(|v| v.key == key), "{key}'s view model");
    }
    for v in kit.view_models.iter().filter(|v| !weapons::MELEE.contains(&v.key.as_str())) {
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
    for v in kit.view_models.iter().filter(|v| !weapons::MELEE.contains(&v.key.as_str())) {
        let set = v.animations.clone().unwrap();
        let mut a = chimera_world::map::anim::Animator::new(set.clone());
        let q = view::sequences(&v.key).unwrap();
        a.play(set.sequence(q.idle.1).unwrap(), 0.0);
        let pose = a.pose(0.1);
        let reference = chimera_world::map::reference_world(&v.bones);
        let mut posed: Vec<Mat4> = Vec::new();
        for (i, b) in v.bones.iter().enumerate() {
            let (r, t) = pose.get(i).copied().unwrap_or((b.rotation, b.position));
            let local = Mat4::from_rotation_translation(r, t);
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
            let (r, t) = pose.get(i).copied().unwrap_or((b.rotation, b.position));
            let local = Mat4::from_rotation_translation(r, t);
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

/// The Demoman's launchers held at rest as TF2 holds them
/// (docs/plans/active/tf2-parity.md, "The Stickybomb Launcher's
/// first-person pose"): at frame 0 of `sb_idle` / `g_idle` the bones the
/// `c_` model is merged onto sit in the eye's space (forward, right, up,
/// units) where the arms' animation puts them, the barrel (`weapon_bone`'s
/// +Z) ahead and its +X (the roll: which way the drum and sight face) to
/// the left, and each merged bone keeps its place relative to
/// `weapon_bone` from the `c_` model's own reference pose: the gun is
/// drawn whole, not bent by the arms. Matches TF2's stock Stickybomb
/// Launcher side by side (tools/tf2ref/scenes/sticky_pose.tf2); the
/// difference once reported was the account's Quickiebomb Launcher.
#[test]
fn the_launchers_bones_sit_where_tf2_holds_them() {
    use bevy::prelude::*;
    use source_engine::content::props;
    use tf2::game::weapons::{GRENADE_LAUNCHER, STICKY_LAUNCHER};
    let Some(mount) = install_mount() else { return };
    let mut materials = MaterialLoader::guest(&mount, 0, 0);
    let (mut kit, mut warnings) = (chimera_world::map::GameKit::default(), Vec::new());
    maps::add_guns(&mut materials, &|g| g == STICKY_LAUNCHER || g == GRENADE_LAUNCHER, false, &mut kit, &mut warnings);
    // (bone, eye-space position) and weapon_bone's +Z and +X.
    type Case<'a> = (&'a str, &'a str, &'a str, &'a [(&'a str, [f32; 3])], [f32; 3], [f32; 3]);
    let cases: &[Case] = &[
        (
            STICKY_LAUNCHER,
            "models/weapons/c_models/c_stickybomb_launcher/c_stickybomb_launcher.mdl",
            "sb_idle",
            &[
                ("weapon_bone", [28.24, 12.15, -9.05]),
                ("weapon_bone_1", [30.74, 10.45, -5.96]),
                ("vm_weapon_bone", [29.95, 11.84, -9.58]),
                ("vm_weapon_bone_1", [36.65, 3.72, -8.95]),
            ],
            [0.99, -0.15, 0.06],
            [-0.16, -0.99, 0.07],
        ),
        (
            GRENADE_LAUNCHER,
            "models/weapons/c_models/c_grenadelauncher/c_grenadelauncher.mdl",
            "g_idle",
            &[
                ("weapon_bone", [24.77, 12.35, -10.39]),
                ("weapon_bone_1", [30.64, 10.76, -15.46]),
                ("weapon_bone_2", [33.96, 10.24, -9.65]),
                ("weapon_bone_3", [51.39, 6.46, -2.81]),
            ],
            [0.97, -0.22, 0.08],
            [-0.23, -0.97, 0.04],
        ),
    ];
    let unit = tf2::game::METERS_PER_UNIT;
    let mut wrong = Vec::new();
    for (gun, model, sequence, bones, barrel, roll) in cases {
        let v = kit.view_models.iter().find(|v| v.key == *gun).unwrap_or_else(|| panic!("{gun}: no view model"));
        let set = v.animations.clone().unwrap();
        let mut a = chimera_world::map::anim::Animator::new(set.clone());
        a.play(set.sequence(sequence).unwrap_or_else(|| panic!("{gun}: no {sequence}")), 0.0);
        let posed = chimera_world::map::view_model::posed_world(&v.bones, &a.pose(0.0));
        let root = v.root.to_matrix();
        let bone = |name: &str| v.bones.iter().position(|b| b.name.eq_ignore_ascii_case(name)).unwrap_or_else(|| panic!("{gun}: no {name}"));
        // Camera space (right, up, back) to the spec's (forward, right, up).
        let eye = |p: Vec3| [-p.z, p.x, p.y];
        let far = |got: [f32; 3], want: [f32; 3]| (0..3).map(|i| (got[i] - want[i]).abs()).fold(0.0, f32::max);
        for (name, want) in *bones {
            let got = eye((root * posed[bone(name)]).transform_point3(Vec3::ZERO) / unit);
            eprintln!("{gun} {sequence}: {name} at {got:?}");
            if far(got, *want) > 0.05 {
                wrong.push(format!("{gun} {sequence}: {name} at {got:?}, not {want:?}"));
            }
        }
        let wb = root * posed[bone("weapon_bone")];
        for (axis, want, what) in [(Vec3::Z, barrel, "+Z (the barrel)"), (Vec3::X, roll, "+X (the roll)")] {
            let got = eye(wb.transform_vector3(axis).normalize());
            if far(got, *want) > 0.01 {
                wrong.push(format!("{gun} {sequence}: weapon_bone's {what} along {got:?}, not {want:?}"));
            }
        }
        // The c_ model's own bones, rigid with weapon_bone as in its file.
        let weapon = props::load_view_model(&mut materials, model, gun, true).unwrap();
        let reference = chimera_world::map::reference_world(&weapon.bones);
        let own = |name: &str| weapon.bones.iter().position(|b| b.name.eq_ignore_ascii_case(name));
        let wb_own = reference[own("weapon_bone").unwrap()];
        for (name, _) in bones.iter().skip(1) {
            let Some(k) = own(name) else { continue };
            let (_, r_arms, t_arms) = (posed[bone("weapon_bone")].inverse() * posed[bone(name)]).to_scale_rotation_translation();
            let (_, r_own, t_own) = (wb_own.inverse() * reference[k]).to_scale_rotation_translation();
            let (dt, da) = ((t_arms - t_own).length(), r_arms.angle_between(r_own).to_degrees());
            if dt > 0.1 || da > 0.5 {
                wrong.push(format!("{gun} {sequence}: {name} {dt:.2} units and {da:.1} degrees off the c_ model's own place"));
            }
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
        if *key == weapons::ROCKET_SHOWN || *key == "tf2:tf_weapon_syringegun_medic/syringe" {
            assert!(nose.dot(axis.normalize()).abs() > 0.95, "{key}: nose {nose}, half {}", a.half);
        }
        if *key == weapons::ROCKET_SHOWN {
            // effects.md 1: its trail starts on the `trail` attachment,
            // which lies toward the tail (behind the bounds' centre).
            let trail = a.attachment("trail").expect("the rocket's `trail` attachment");
            println!("{key}: trail at {trail} m ({} units)", trail / 0.0254);
            assert!(trail.dot(nose) < 0.0, "behind the centre: {trail}, nose {nose}");
        }
    }
}

/// Every voice entry the players' voices can play (`voices::Tf2Voices::
/// entries`: pain, death and kill lines) names waves in the install's
/// sound scripts, the death entries the spec inferred included
/// (specs/tf2/voices.md open question 1).
#[test]
fn every_voice_entry_has_waves() {
    let Some(mount) = install_mount() else { return };
    let scripts = sets::sound_scripts(&mount);
    let missing: Vec<String> = tf2::game::voices::Tf2Voices::built_in()
        .entries()
        .into_iter()
        .filter(|e| scripts.waves(e).is_empty())
        .collect();
    assert!(missing.is_empty(), "no waves: {missing:?}");
}

/// effects.md 14.1: the crit glow's hosts carry their hitboxes as
/// `$hitbox` attachments: the Rocket Launcher's view model the Soldier
/// arms' 47 boxes (after its own attachments), its held model the
/// `c_rocketlauncher`'s 2; and the glow systems load with their model
/// placement and bone lock.
#[test]
fn the_crit_glows_hosts_carry_their_hitboxes_14_1() {
    use source_engine::content::props::HITBOX_ATTACHMENT;
    let Some(mount) = install_mount() else { return };
    let mut materials = MaterialLoader::guest(&mount, 0, 0);
    let (mut kit, mut warnings) = (chimera_world::map::GameKit::default(), Vec::new());
    maps::add_guns(&mut materials, &|g| g == weapons::ROCKET_LAUNCHER, false, &mut kit, &mut warnings);
    assert!(warnings.is_empty(), "{warnings:?}");
    let v = kit.view_models.iter().find(|v| v.key == weapons::ROCKET_LAUNCHER).expect("its view model");
    let boxes = v.attachments.iter().filter(|a| a.name == HITBOX_ATTACHMENT).count();
    assert_eq!(boxes, 47, "c_soldier_arms' hitbox set");
    let first = v.attachments.iter().position(|a| a.name == HITBOX_ATTACHMENT).unwrap();
    assert!(v.attachments[first..].iter().all(|a| a.name == HITBOX_ATTACHMENT), "after its own attachments");
    let h = kit.held.iter().find(|h| h.key == weapons::ROCKET_LAUNCHER).expect("its held model");
    let held: Vec<_> = h.attachments.iter().filter(|(n, _)| n == HITBOX_ATTACHMENT).collect();
    assert_eq!(held.len(), 2, "c_rocketlauncher's hitbox set");
    // The gun's box: -8.3..8.3, -7.6..15.0, -36.3..21.4 (half extents).
    let big = held.iter().map(|(_, t)| t.scale).fold(bevy::math::Vec3::ZERO, |a, s| if s.length() > a.length() { s } else { a });
    let mut halves = big.to_array();
    halves.sort_by(f32::total_cmp);
    assert!((halves[2] - 28.85).abs() < 0.2 && (halves[1] - 11.3).abs() < 0.2 && (halves[0] - 8.3).abs() < 0.2, "{big}");
}

/// specs/tf2/viewmodel_sounds.md 3-4: every stock reload's sound events
/// as the arms' sequences carry them (event 5004, its entry, at cycle ×
/// length seconds into the sequence), and every sound event of the
/// sequences our rules play names an entry the guns' sound lists load
/// (`maps::weapon_sounds`), so the owner hears each.
#[test]
fn the_view_models_sound_events_are_the_specs_reload_sounds() {
    let Some(mount) = install_mount() else { return };
    let mut materials = MaterialLoader::guest(&mount, 0, 0);
    let (mut kit, mut warnings) = (chimera_world::map::GameKit::default(), Vec::new());
    maps::add_guns(&mut materials, &|_| true, true, &mut kit, &mut warnings);
    let loaded: Vec<String> = maps::weapon_sounds().map(|s| maps::own_sound(&s).to_lowercase()).collect();
    // (sequence, [(seconds, entry)]) of a view model's sequence.
    let events = |key: &str, seq: &str| -> Vec<(f32, String)> {
        let v = kit.view_models.iter().find(|v| v.key == key).unwrap_or_else(|| panic!("{key}"));
        let set = v.animations.as_ref().unwrap();
        let i = set.sequence(seq).unwrap_or_else(|| panic!("{key}: {seq}"));
        let len = set.duration(i);
        set.sequences[i]
            .events
            .iter()
            .filter_map(|e| match view::event_effect(e) {
                Some(chimera_world::map::ViewModelEventKind::Sound { entry }) => Some((e.cycle * len, entry)),
                _ => None,
            })
            .collect()
    };
    let near = |got: &[(f32, String)], want: &[(f32, &str)]| {
        got.len() == want.len() && got.iter().zip(want).all(|((t, e), (wt, we))| (t - wt).abs() < 0.006 && e.eq_ignore_ascii_case(&format!("tf2:{we}")))
    };
    let check = |key: &str, seq: &str, want: &[(f32, &str)]| {
        let got = events(key, seq);
        assert!(near(&got, want), "{key} {seq}: {got:?}, the spec {want:?}");
    };
    use weapons::*;
    check(PISTOL_SCOUT, "p_reload", &[(0.033, "Weapon_Pistol.ReloadScout"), (0.600, "Weapon_Pistol.ClipIn")]);
    check(&format!("{PISTOL_SCOUT}#engineer"), "pstl_reload", &[(0.467, "Weapon_Pistol.ClipIn")]);
    check(SCATTERGUN, "sg_reload_loop", &[(0.033, "Weapon_Scatter_Gun.Reload")]);
    check(ROCKET_LAUNCHER, "dh_reload_loop", &[(0.067, "Weapon_RPG.Reload")]);
    check(GRENADE_LAUNCHER, "g_reload_start", &[(0.167, "Weapon_GrenadeLauncher.DrumOpen")]);
    check(GRENADE_LAUNCHER, "g_reload_loop", &[(0.200, "Weapon_GrenadeLauncher.DrumLoad")]);
    check(GRENADE_LAUNCHER, "g_reload_end", &[(0.167, "Weapon_GrenadeLauncher.DrumClose")]);
    check(STICKY_LAUNCHER, "sb_reload_loop", &[(0.067, "Weapon_StickyBombLauncher.BoltBack"), (0.400, "Weapon_StickyBombLauncher.BoltForward")]);
    check(SYRINGE_GUN, "sg_reload", &[(0.033, "Weapon_SyringeGun.Draw"), (0.300, "Weapon_SyringeGun.Reload1"), (0.600, "Weapon_SyringeGun.Reload2"), (0.700, "Weapon_SyringeGun.Reload3")]);
    check(SMG, "smg_reload", &[(0.267, "Weapon_SMG.ClipOut"), (0.767, "Weapon_SMG.ClipIn")]);
    check(REVOLVER, "reload", &[(0.033, "Weapon_Revolver.Reload1"), (0.400, "Weapon_Revolver.Reload2"), (0.933, "Weapon_Revolver.Reload3")]);
    for (key, l, lt, e, back, fwd) in [
        (format!("{SHOTGUN}#soldier"), "reload_loop", 0.133, "reload_end", 0.233, 0.367),
        (format!("{SHOTGUN}#pyro"), "reload_loop", 0.033, "reload_end", 0.400, 0.533),
        (format!("{SHOTGUN}#heavy"), "reload_loop", 0.067, "reload_end", 0.286, 0.429),
        (SHOTGUN.to_string(), "fj_reload_loop", 0.333, "fj_reload_end", 0.343, 0.486),
    ] {
        check(&key, l, &[(lt, "Weapon_Shotgun.Reload")]);
        check(&key, e, &[(back, "Weapon_ShotgunEngineer.Cock_Back"), (fwd, "Weapon_ShotgunEngineer.Cock_Forward")]);
    }
    check(SNIPER_RIFLE, "fire", &[(0.767, "Weapon_SniperRifle.BoltBack"), (0.967, "Weapon_SniperRifle.BoltForward")]);
    // Every sound event of every sequence our rules play is loaded.
    let mut missing = std::collections::BTreeSet::new();
    for v in &kit.view_models {
        let set = v.animations.as_ref().unwrap();
        let mut names: Vec<&str> = Vec::new();
        if let Some(q) = view::sequences(&v.key) {
            let all = [Some(q.draw), Some(q.idle), Some(q.fire), q.reload_start, q.reload_loop, q.reload_end, q.spin_up, q.spin_idle, q.spin_down, q.charge];
            names.extend(all.into_iter().flatten().map(|(_, n)| n));
        } else if let Some(q) = view::melee_sequences(&v.key) {
            names.extend([q.draw, q.idle, q.crit]);
            names.extend(q.swing.iter().copied());
        }
        for n in names {
            for (_, entry) in events(&v.key, n) {
                if !loaded.contains(&entry.to_lowercase()) {
                    missing.insert(format!("{} {n}: {entry}", v.key));
                }
            }
        }
    }
    assert!(missing.is_empty(), "not loaded: {missing:#?}");
    // Each draw sounds by its events alone (the data has no draw sounds).
    let mut silent = Vec::new();
    for v in &kit.view_models {
        let draw = view::sequences(&v.key).map(|q| q.draw.1).or_else(|| view::melee_sequences(&v.key).map(|q| q.draw));
        if let Some(d) = draw
            && events(&v.key, d).is_empty()
        {
            silent.push(format!("{} {d}", v.key));
        }
    }
    assert!(silent.is_empty(), "draws with no sound event: {silent:?}");
}
