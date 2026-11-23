//! DoD:S's body animation on the install's class body (the shared model's
//! sequences): every gun's set (`dods:body_anim`) has the activities its
//! body plays standing, crouched, prone and moving, and its reload's; a
//! prone, crouched and running body plays its family; and the held gun's
//! bone (the held model's, `ValveBiped.weapon_bone` or `_left`) is in a
//! hand in every stance but the crawl (whose files carry it along the
//! forearm, a few units off), which at the model's rest it isn't (48
//! units off).
//! Passes with a note without an install.

use std::sync::Arc;

use bevy::prelude::*;
use chimera_core::mount::served;
use chimera_world::map::{
    GameKit, KitLoad,
    anim::{AnimSet, Animator, model_space},
};
use dods::game::{
    GAME, INFO, bodies, mount,
    player_anim::{BodyAnim, DodsPlayerAnim, Doing},
    stance::Stance,
    weapons,
};
use source_engine::{
    content::{material::MaterialLoader, surfaceprops::SurfaceProps},
    player_anim::Inputs,
};

/// The default body's set and every gun's held bone and body set.
fn setup() -> Option<(Arc<AnimSet>, Vec<(String, String, BodyAnim)>)> {
    let path = INFO.install()?;
    if !served::usable(GAME, &path, "dod") {
        eprintln!("skipping: no DoD:S install");
        return None;
    }
    let mount = mount::open(&path).ok()?;
    let mut materials = MaterialLoader::guest(&mount, 0, 0).lean(true);
    let surfaces = SurfaceProps::load(&mut materials);
    let body = bodies::load(&mut materials, &surfaces, bodies::DEFAULT_MODEL).expect("the body");
    let (mut kit, mut warnings) = (GameKit::default(), Vec::new());
    let wants = KitLoad::Players(&["dods:"]);
    weapons::add_guns(&mut materials, &surfaces, &|g| wants.wants(g), &mut kit, &mut warnings);
    let set = weapons::dods_weapons();
    let mut guns = Vec::new();
    for h in &kit.held {
        let data = set
            .weapons
            .iter()
            .find(|d| d.id == h.key)
            .unwrap_or_else(|| panic!("{}'s data", h.key));
        // The explosives have no set of their own yet (the Thompson's,
        // `player_anim::FALLBACK`); every gun has one.
        let Some(anim) = data.hooks.get(dods::game::player_anim::BODY_ANIM_HOOK) else {
            assert!(weapons::EXPLOSIVES.contains(&h.key.as_str()), "{} has no dods:body_anim", h.key);
            continue;
        };
        let anim: BodyAnim = anim.clone().try_into().expect("dods:body_anim");
        guns.push((h.key.clone(), h.bone.clone(), anim));
    }
    Some((body.animations.clone().expect("animations"), guns))
}

fn ground() -> Inputs {
    Inputs {
        on_ground: true,
        ..default()
    }
}

/// The body posed after a second of `i` and `doing`.
fn posed(set: &Arc<AnimSet>, i: Inputs, doing: Doing) -> (Animator, DodsPlayerAnim, f64) {
    let (mut a, mut s, mut t) = (Animator::new(set.clone()), DodsPlayerAnim::default(), 0.0);
    s.doing = doing;
    for _ in 0..67 {
        t += 0.015;
        s.step(&mut a, &i, None, 0.015, t);
    }
    (a, s, t)
}

#[test]
fn every_gun_s_set_is_in_the_shared_model() {
    let Some((set, guns)) = setup() else { return };
    let mut missing = Vec::new();
    for (id, _, anim) in &guns {
        let s = &anim.suffix;
        let any = |acts: &[String]| acts.iter().any(|a| set.activity(a).is_some());
        for stance in ["STAND", "CROUCH", "RUN", "CROUCHWALK"] {
            if !any(&[
                format!("ACT_DOD_{stance}_IDLE_{s}"),
                format!("ACT_DOD_{stance}_AIM_{s}"),
            ]) {
                missing.push(format!("{id}: {stance} {s}"));
            }
        }
        if !any(&[format!("ACT_DOD_PRONE_AIM_{s}")]) || !any(&[format!("ACT_DOD_PRIMARYATTACK_{s}")]) {
            missing.push(format!("{id}: prone or attack {s}"));
        }
        if let Some(r) = &anim.reload
            && set.activity(&format!("ACT_DOD_RELOAD_{r}")).is_none()
        {
            missing.push(format!("{id}: reload {r}"));
        }
    }
    assert!(missing.is_empty(), "{missing:?}");
}

#[test]
fn prone_crouched_and_running_bodies_play_their_families() {
    let Some((set, _)) = setup() else { return };
    let tommy = || {
        Some(BodyAnim {
            suffix: "TOMMY".into(),
            reload: None,
        })
    };
    let cases = [
        ("prone", ground(), Stance::Prone, false, "ProneAim_TOMMY"),
        (
            "crawling",
            Inputs {
                velocity: Vec2::new(50.0, 0.0),
                ..ground()
            },
            Stance::Prone,
            false,
            "ProneWalkIdle_TOMMY",
        ),
        (
            "crouched",
            Inputs {
                ducked: true,
                ..ground()
            },
            Stance::Up,
            false,
            "CrouchIdle_TOMMY",
        ),
        (
            "running",
            Inputs {
                velocity: Vec2::new(220.0, 0.0),
                ..ground()
            },
            Stance::Up,
            false,
            "r_RunIdle_TOMMY",
        ),
    ];
    for (what, i, stance, deployed, want) in cases {
        let (a, ..) = posed(
            &set,
            i,
            Doing {
                stance,
                deployed,
                anim: tommy(),
                ..default()
            },
        );
        let got = a.main.map(|m| set.sequences[m].name.clone());
        assert_eq!(
            got.as_deref().map(str::to_ascii_lowercase),
            Some(want.to_ascii_lowercase()),
            "{what}"
        );
    }
    let mg = Some(BodyAnim {
        suffix: "MG".into(),
        reload: None,
    });
    let (a, ..) = posed(
        &set,
        ground(),
        Doing {
            stance: Stance::Prone,
            deployed: true,
            anim: mg,
            ..default()
        },
    );
    assert_eq!(
        a.main.map(|m| set.sequences[m].activity.as_str()),
        Some("ACT_DOD_PRONE_DEPLOY_MG")
    );
}

#[test]
fn the_held_gun_follows_the_hands() {
    let Some((set, guns)) = setup() else { return };
    let bone = |n: &str| {
        set.bone_names
            .iter()
            .position(|b| b.eq_ignore_ascii_case(n))
            .unwrap_or_else(|| panic!("no {n}"))
    };
    let hands = [bone("ValveBiped.Bip01_R_Hand"), bone("ValveBiped.Bip01_L_Hand")];
    let mut bad = Vec::new();
    for (id, held, anim) in &guns {
        let b = bone(held);
        let rest = model_space(&set.defaults, &set.parents);
        let off = |pose: &[(Quat, Vec3)]| {
            hands
                .iter()
                .map(|&h| pose[h].1.distance(pose[b].1))
                .fold(f32::MAX, f32::min)
        };
        let at_rest = off(&rest);
        let mg = matches!(anim.suffix.as_str(), "MG" | "30CAL");
        let cases = [
            ("standing", ground(), Stance::Up, false, false),
            ("sighted", ground(), Stance::Up, false, true),
            (
                "crouched",
                Inputs {
                    ducked: true,
                    ..ground()
                },
                Stance::Up,
                false,
                false,
            ),
            (
                "running",
                Inputs {
                    velocity: Vec2::new(220.0, 0.0),
                    ..ground()
                },
                Stance::Up,
                false,
                false,
            ),
            ("prone", ground(), Stance::Prone, false, false),
            (
                "crawling",
                Inputs {
                    velocity: Vec2::new(50.0, 0.0),
                    ..ground()
                },
                Stance::Prone,
                false,
                false,
            ),
            ("deployed", ground(), Stance::Prone, true, false),
        ];
        for (what, i, stance, deployed, sighted) in cases {
            if deployed && !mg {
                continue;
            }
            let (a, _, t) = posed(
                &set,
                i,
                Doing {
                    stance,
                    deployed,
                    sighted,
                    anim: Some(anim.clone()),
                    ..default()
                },
            );
            let d = off(&model_space(&a.pose(t), &set.parents));
            eprintln!("{id} {what}: {held} {d:.1} units from a hand (rest {at_rest:.1})");
            // In the hand; crawling, the files carry the gun along the
            // forearm, off the grip (4 to 19 units), still nowhere near its
            // rest.
            let most = if what == "crawling" { at_rest / 2.0 } else { 1.0 };
            if d > most {
                bad.push(format!("{id} {what}: {held} {d:.1} units from a hand"));
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
