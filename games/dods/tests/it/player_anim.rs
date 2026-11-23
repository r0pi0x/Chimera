//! DoD:S's body animation (`dods::game::player_anim`) on a stand-in of the
//! shared player model (its activities' names, one bone each): a standing,
//! running, crouched, prone, crawling and deployed body plays the gun's
//! family of the shared model's activities, the sights its zoom, a jump
//! `ACT_HOP` until the ground, going prone the transition over the
//! stance's 1.2 s; a shot and a reload their gestures by stance; and in
//! the headless simulation, a DoD:S player with a gun in hand and a
//! stance gets its `Doing` from them (`follow`). The real model, and the
//! held gun in the hands: `heavy::player_anim`.

use std::sync::Arc;

use bevy::prelude::*;
use chimera_world::map::anim::{AnimSet, Animation, Animator, Sequence};
use dods::game::{
    player_anim::{BodyAnim, DodsPlayerAnim, Doing, Main},
    stance::Stance,
};
use source_engine::player_anim::Inputs;

/// A set with one sequence per activity (30 frames at 30 fps; the walk's
/// and run's ground speeds the model's: 58 and 220 units/s, so walk and
/// run split at 139).
fn set(activities: &[&str]) -> Arc<AnimSet> {
    let mut s = AnimSet {
        defaults: vec![(Quat::IDENTITY, Vec3::ZERO)],
        parents: vec![None],
        ..default()
    };
    for (i, act) in activities.iter().enumerate() {
        let speed = if act.contains("_WALK_") {
            58.0
        } else if act.contains("_RUN_") {
            220.0
        } else {
            0.0
        };
        s.animations.push(Animation {
            name: act.to_string(),
            fps: 30.0,
            frames: 37,
            speed,
            motion: Vec2::new(speed, 0.0),
            ..default()
        });
        s.sequences.push(Sequence {
            name: act.to_string(),
            activity: act.to_string(),
            activity_weight: 1,
            looping: true,
            grid: (1, 1),
            anims: vec![i],
            ..default()
        });
    }
    Arc::new(s)
}

fn all() -> Arc<AnimSet> {
    let mut acts = vec![
        "ACT_HOP",
        "ACT_GET_DOWN_STAND",
        "ACT_GET_DOWN_CROUCH",
        "ACT_GET_UP_STAND",
    ];
    let mut owned = Vec::new();
    for s in ["TOMMY", "RIFLE", "MG", "30CAL"] {
        for a in [
            "STAND_IDLE",
            "STAND_AIM",
            "STAND_ZOOM",
            "WALK_IDLE",
            "RUN_IDLE",
            "CROUCH_IDLE",
            "CROUCHWALK_IDLE",
            "PRONE_AIM",
            "PRONE_ZOOM",
            "PRONEWALK_IDLE",
            "PRIMARYATTACK",
            "PRIMARYATTACK_PRONE",
            "RELOAD",
            "RELOAD_CROUCH",
            "RELOAD_PRONE",
        ] {
            owned.push(format!("ACT_DOD_{a}_{s}"));
        }
    }
    for a in [
        "PRONE_DEPLOY_MG",
        "PRONE_DEPLOY_30CAL",
        "PRIMARYATTACK_PRONE_DEPLOYED_30CAL",
        "PRIMARYATTACK_PRONE_DEPLOYED_MG",
        "RELOAD_PRONE_DEPLOYED_MG",
        "RELOAD_BAR",
    ] {
        owned.push(format!("ACT_DOD_{a}"));
    }
    acts.extend(owned.iter().map(String::as_str));
    set(&acts)
}

fn gun(suffix: &str) -> Option<BodyAnim> {
    Some(BodyAnim {
        suffix: suffix.into(),
        reload: (suffix == "RIFLE").then(|| "BAR".into()),
    })
}

fn ground() -> Inputs {
    Inputs {
        on_ground: true,
        ..default()
    }
}

/// `s` stepped `n` ticks of 0.015 s with `i` and `doing`.
fn run(a: &mut Animator, s: &mut DodsPlayerAnim, i: Inputs, doing: Doing, n: usize, t: &mut f64) {
    s.doing = doing;
    for _ in 0..n {
        *t += 0.015;
        s.step(a, &i, Some("dods:weapon"), 0.015, *t);
    }
}

fn main_activity(a: &Animator) -> String {
    a.main.map(|m| a.set.sequences[m].activity.clone()).unwrap_or_default()
}

fn layer_activity(a: &Animator) -> Option<String> {
    a.layers
        .iter()
        .flatten()
        .next()
        .map(|l| a.set.sequences[l.sequence].activity.clone())
}

#[test]
fn stances_and_movement_play_the_gun_s_family() {
    let set = all();
    let cases: [(&str, Inputs, Stance, bool, &str); 8] = [
        ("standing", ground(), Stance::Up, false, "ACT_DOD_STAND_IDLE_TOMMY"),
        (
            "walking",
            Inputs {
                velocity: Vec2::new(60.0, 0.0),
                ..ground()
            },
            Stance::Up,
            false,
            "ACT_DOD_WALK_IDLE_TOMMY",
        ),
        (
            "running",
            Inputs {
                velocity: Vec2::new(220.0, 0.0),
                ..ground()
            },
            Stance::Up,
            false,
            "ACT_DOD_RUN_IDLE_TOMMY",
        ),
        (
            "crouched",
            Inputs {
                ducked: true,
                ..ground()
            },
            Stance::Up,
            false,
            "ACT_DOD_CROUCH_IDLE_TOMMY",
        ),
        (
            "crouch-walking",
            Inputs {
                ducked: true,
                velocity: Vec2::new(0.0, 73.0),
                ..ground()
            },
            Stance::Up,
            false,
            "ACT_DOD_CROUCHWALK_IDLE_TOMMY",
        ),
        ("prone", ground(), Stance::Prone, false, "ACT_DOD_PRONE_AIM_TOMMY"),
        (
            "crawling",
            Inputs {
                velocity: Vec2::new(50.0, 0.0),
                ..ground()
            },
            Stance::Prone,
            false,
            "ACT_DOD_PRONEWALK_IDLE_TOMMY",
        ),
        ("sighted", ground(), Stance::Up, true, "ACT_DOD_STAND_ZOOM_TOMMY"),
    ];
    for (what, i, stance, sighted, want) in cases {
        let (mut a, mut s, mut t) = (Animator::new(set.clone()), DodsPlayerAnim::default(), 0.0);
        run(
            &mut a,
            &mut s,
            i,
            Doing {
                stance,
                sighted,
                anim: gun("TOMMY"),
                ..default()
            },
            10,
            &mut t,
        );
        assert_eq!(main_activity(&a), want, "{what}");
    }
    // A rifle plays its own set; a gun the model lacks a set for, the
    // Thompson's.
    let (mut a, mut s, mut t) = (Animator::new(set.clone()), DodsPlayerAnim::default(), 0.0);
    run(
        &mut a,
        &mut s,
        Inputs {
            ducked: true,
            ..ground()
        },
        Doing {
            anim: gun("RIFLE"),
            ..default()
        },
        5,
        &mut t,
    );
    assert_eq!(main_activity(&a), "ACT_DOD_CROUCH_IDLE_RIFLE");
    run(
        &mut a,
        &mut s,
        ground(),
        Doing {
            anim: gun("BAZOOKA"),
            ..default()
        },
        5,
        &mut t,
    );
    assert_eq!(main_activity(&a), "ACT_DOD_STAND_IDLE_TOMMY");
}

#[test]
fn a_jump_hops_until_the_ground() {
    let set = all();
    let (mut a, mut s, mut t) = (Animator::new(set), DodsPlayerAnim::default(), 0.0);
    let d = Doing {
        anim: gun("TOMMY"),
        ..default()
    };
    run(&mut a, &mut s, ground(), d.clone(), 3, &mut t);
    run(
        &mut a,
        &mut s,
        Inputs {
            jumped: true,
            vertical: 200.0,
            ..default()
        },
        d.clone(),
        1,
        &mut t,
    );
    run(
        &mut a,
        &mut s,
        Inputs {
            vertical: -50.0,
            ..default()
        },
        d.clone(),
        20,
        &mut t,
    );
    assert_eq!(main_activity(&a), "ACT_HOP");
    assert_eq!(s.main, Some(Main::Jump));
    run(&mut a, &mut s, ground(), d, 1, &mut t);
    assert_eq!(main_activity(&a), "ACT_DOD_STAND_IDLE_TOMMY");
}

#[test]
fn going_prone_plays_the_transition_over_the_stance_s_time() {
    let set = all();
    let (mut a, mut s, mut t) = (Animator::new(set), DodsPlayerAnim::default(), 0.0);
    let d = |stance, from_eye_crouch| Doing {
        stance,
        from_crouch: from_eye_crouch,
        anim: gun("MG"),
        ..default()
    };
    run(&mut a, &mut s, ground(), d(Stance::Up, false), 3, &mut t);
    run(&mut a, &mut s, ground(), d(Stance::GoingDown, false), 1, &mut t);
    assert_eq!(main_activity(&a), "ACT_GET_DOWN_STAND");
    // 36 frames' length (1.2 s) over GO_PRONE (1.2 s): rate 1, halfway at
    // 40 ticks.
    run(&mut a, &mut s, ground(), d(Stance::GoingDown, false), 39, &mut t);
    assert!((a.cycle - 0.5).abs() < 0.02, "cycle {}", a.cycle);
    run(&mut a, &mut s, ground(), d(Stance::Prone, false), 1, &mut t);
    assert_eq!(main_activity(&a), "ACT_DOD_PRONE_AIM_MG");
    // Begun ducked: the crouch's.
    let (mut a, mut s) = (Animator::new(a.set.clone()), DodsPlayerAnim::default());
    run(&mut a, &mut s, ground(), d(Stance::GoingDown, true), 1, &mut t);
    assert_eq!(main_activity(&a), "ACT_GET_DOWN_CROUCH");
    run(&mut a, &mut s, ground(), d(Stance::GettingUp, false), 1, &mut t);
    assert_eq!(main_activity(&a), "ACT_GET_UP_STAND");
}

#[test]
fn deployed_fire_and_reload_gestures_by_stance() {
    let set = all();
    let (mut a, mut s, mut t) = (Animator::new(set.clone()), DodsPlayerAnim::default(), 0.0);
    let deployed = Doing {
        stance: Stance::Prone,
        deployed: true,
        anim: gun("MG"),
        ..default()
    };
    run(&mut a, &mut s, ground(), deployed.clone(), 3, &mut t);
    assert_eq!(main_activity(&a), "ACT_DOD_PRONE_DEPLOY_MG");
    run(
        &mut a,
        &mut s,
        Inputs {
            fired: true,
            ..ground()
        },
        deployed.clone(),
        1,
        &mut t,
    );
    assert_eq!(
        layer_activity(&a).as_deref(),
        Some("ACT_DOD_PRIMARYATTACK_PRONE_DEPLOYED_MG")
    );
    run(
        &mut a,
        &mut s,
        Inputs {
            reloaded: true,
            ..ground()
        },
        deployed,
        1,
        &mut t,
    );
    assert_eq!(layer_activity(&a).as_deref(), Some("ACT_DOD_RELOAD_PRONE_DEPLOYED_MG"));
    // The gesture plays once (37 frames: 1.2 s) and goes.
    run(
        &mut a,
        &mut s,
        ground(),
        Doing {
            stance: Stance::Prone,
            deployed: true,
            anim: gun("MG"),
            ..default()
        },
        90,
        &mut t,
    );
    assert_eq!(layer_activity(&a), None);
    // Standing: the plain shot; crouched: the crouch's reload; a reload of
    // its own (the BAR's on the rifle's set).
    let up = Doing {
        anim: gun("TOMMY"),
        ..default()
    };
    run(
        &mut a,
        &mut s,
        Inputs {
            fired: true,
            ..ground()
        },
        up.clone(),
        1,
        &mut t,
    );
    assert_eq!(layer_activity(&a).as_deref(), Some("ACT_DOD_PRIMARYATTACK_TOMMY"));
    run(
        &mut a,
        &mut s,
        Inputs {
            reloaded: true,
            ducked: true,
            ..ground()
        },
        up,
        1,
        &mut t,
    );
    assert_eq!(layer_activity(&a).as_deref(), Some("ACT_DOD_RELOAD_CROUCH_TOMMY"));
    run(
        &mut a,
        &mut s,
        Inputs {
            reloaded: true,
            ..ground()
        },
        Doing {
            anim: gun("RIFLE"),
            ..default()
        },
        1,
        &mut t,
    );
    assert_eq!(layer_activity(&a).as_deref(), Some("ACT_DOD_RELOAD_BAR"));
}

/// In the headless simulation: a DoD:S player's body (its animator the
/// stand-in set) holding the .30 cal follows its player, standing, prone
/// and deployed, and a deployed shot plays the deployed fire gesture.
#[test]
fn a_player_s_body_follows_its_stance_and_gun() {
    use chimera_core::core::ActionRegistry;
    use chimera_sim::weapon::{Inventory, give};
    use chimera_world::{
        greybox::{self, GreyboxMapPlugin},
        harness::Sim,
    };
    use dods::{
        game::{TICK_INTERVAL, movement, stance, weapons::CAL30},
        harness::DodsSim,
    };
    let mut sim = Sim::new(DodsSim, GreyboxMapPlugin);
    sim.set_tick_interval(TICK_INTERVAL);
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.app.world_mut().entity_mut(p).insert(Animator::new(all()));
    let w = give(sim.app.world_mut(), p, CAL30).unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
    sim.seconds(1.5);
    let body = |sim: &Sim| main_activity(sim.app.world().get::<Animator>(p).unwrap());
    assert_eq!(body(&sim), "ACT_DOD_STAND_IDLE_30CAL");
    let doing = |sim: &Sim| sim.app.world().get::<DodsPlayerAnim>(p).unwrap().doing.clone();
    assert_eq!(
        doing(&sim).anim.map(|a| a.suffix),
        Some("30CAL".into()),
        "the gun's set from its data"
    );
    // Prone: the transition, then the prone set.
    let bit = sim.app.world().resource::<ActionRegistry>().bit(stance::PRONE_ACTION);
    sim.intent(p).actions |= bit;
    sim.ticks(1);
    sim.intent(p).actions &= !bit;
    sim.ticks(2);
    assert_eq!(body(&sim), "ACT_GET_DOWN_STAND");
    sim.seconds(1.5);
    assert_eq!(body(&sim), "ACT_DOD_PRONE_AIM_30CAL");
    // Deployed by the alt fire, then firing.
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    sim.ticks(2);
    assert!(doing(&sim).deployed);
    assert_eq!(body(&sim), "ACT_DOD_PRONE_DEPLOY_30CAL");
    sim.intent(p).fire = true;
    sim.seconds(1.4);
    let a = sim.app.world().get::<Animator>(p).unwrap();
    assert_eq!(
        layer_activity(a).as_deref(),
        Some("ACT_DOD_PRIMARYATTACK_PRONE_DEPLOYED_30CAL")
    );
}
