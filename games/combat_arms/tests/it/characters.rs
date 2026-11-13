//! specs/combat_arms/characters.md's test cases, one test each (`t01_` is
//! test case 1). The "install" cases measure the character archives,
//! which are V2-keyed here: they run on the install in `heavy::
//! characters` when a key is configured; here the same paths run on the
//! spec's own data (the measured skeleton as `skeleton::fallback`, the
//! spec's names and lengths as `driver::spec_library`) and on stand-in
//! models written in this file (nodes, sockets, weight sets and keyed
//! animations), so what the install would feed is checked end to end.
//!
//! The driver runs at the game's 15 ms tick unless a case says otherwise.

use std::sync::Arc;

use crate::common::sim;
use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Hitgroup, MovementState};
use chimera_sim::weapon::{self, Inventory, Magazine};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{
    game::{
        TICK_INTERVAL,
        characters::{
            CaAnim, CaBody, CaPose,
            death::{self, DeathInputs, DeathKind},
            driver::{self, Driver, Inputs, Jump, Posture},
            models,
            records::{self, Selection},
            skeleton::{self, Posed, Rig, Shape},
        },
        inventory::{self, UNIT},
        movement,
        weapons::{FEMALE, MALE, RIFLE},
    },
    harness::CaSim,
};
use lithtech_formats::{
    anim::AnimList,
    ltb::{self, Animation, Channel, Key, Model, Node, Socket, WeightSet},
};

const TICK: u32 = 15;

/// The character data as the scenario tests see it: ours only (the
/// install's records never merged in, whichever test reads it first).
fn builtin() -> &'static records::CharacterData {
    combat_arms::game::data::CaData::builtin_static().characters()
}

fn close(a: Vec3, b: Vec3, tol: f32) -> bool {
    (a - b).abs().max_element() <= tol
}

// ---------------------------------------------------------------------
// Model choice (data)

#[test]
fn t01_body_by_face() {
    let d = builtin();
    let (r, figure) = d.effective(&Selection::default());
    assert_eq!((r.file.as_str(), r.skeleton, figure), ("CHARS_M_BODY\\CM_VST_NM_DF_Nuts_JS.ltb", 0, 0));
    let (r, figure) = d.effective(&Selection {
        face: 5,
        ..default()
    });
    assert_eq!((r.id, r.file.as_str(), r.skeleton, figure), (7, "CHARS_M_BODY\\Nomal_W_vest.ltb", 1, 1));
    let (r, _) = d.effective(&Selection {
        model: 2,
        face: 4,
        ..default()
    });
    assert_eq!((r.id, r.file.as_str()), (9, "CHARS_M_BODY\\Heavy_W_vest.ltb"));
    // Male faces keep the record.
    assert_eq!(d.effective(&Selection { face: 3, ..default() }).0.id, 0);
}

#[test]
fn t02_defaults() {
    let d = builtin();
    for model in [-1, 3, 999] {
        assert_eq!(d.effective(&Selection { model, ..default() }).0.id, 0, "model {model}");
    }
    assert_eq!(d.head_file(0, 0), None);
    assert_eq!(d.head_file(0, 1), None);
}

#[test]
fn t03_camouflage_skins() {
    let d = builtin();
    let m = d.model(0).unwrap();
    assert_eq!(
        d.skins(m, 3),
        (Some("CHARS_T_BODY\\Nomal_vest_body_UR.dtx"), Some("CHARS_T_BODY\\Nomal_vest_leg_UR.dtx"))
    );
    assert_eq!(d.skins(m, 9), (Some("CHARS_T_BODY\\Nomal_vest_body.dtx"), Some("CHARS_T_BODY\\Nomal_vest_leg.dtx")));
    // The skin families: camouflage by two, the mark's bit.
    assert_eq!(models::family(3, true), 7);
    assert_eq!(models::family(9, false), 0);
}

/// Ours (characters.md 1, 2: each player's own look): bots draw their
/// look from the loadout pool, the same seed the same look, and a crowd
/// of them doesn't all look the same.
#[test]
fn bot_looks_vary() {
    let d = builtin();
    assert_eq!(d.bot_look(7), d.bot_look(7));
    let looks: Vec<Selection> = (0..16).map(|s| d.bot_look(s)).collect();
    let distinct = |f: fn(&Selection) -> i32| {
        let mut v: Vec<i32> = looks.iter().map(f).collect();
        v.sort();
        v.dedup();
        v.len()
    };
    assert!(distinct(|l| l.model) > 1, "{looks:?}");
    assert!(distinct(|l| l.face) > 1, "{looks:?}");
    assert!(distinct(|l| l.camouflage) > 1, "{looks:?}");
    for l in &looks {
        assert!(d.loadouts.models.contains(&l.model) && d.loadouts.faces.contains(&l.face));
        assert!(d.loadouts.heads.contains(&l.head) && d.loadouts.camouflages.contains(&l.camouflage));
    }
}

/// A body group's choices (characters.md 2 as Chimera's body parts): the
/// effective body record, the face (else the body's default) and the
/// head piece worn pick one choice of each part; the number is their
/// mixed radix.
#[test]
fn body_group_numbers() {
    let g = models::BodyGroup {
        skeleton: 0,
        records: vec![0, 1, 2],
        faces: vec![0, 1, 2, 3],
        heads: vec![-1, 2],
        ..default()
    };
    assert_eq!(g.parts(), [3, 4, 2, 1, 1]);
    assert_eq!(g.number(0, 0, -1), 0);
    assert_eq!(g.number(2, 0, -1), 2);
    assert_eq!(g.number(0, 3, -1), 9);
    assert_eq!(g.number(1, 2, 2), 1 + 3 * (2 + 4));
    // Not in the group: the first choice.
    assert_eq!(g.number(9, 7, 5), 0);
    // Goggles and a backpack (parts 3 and 4).
    let worn = models::BodyGroup {
        goggles: vec![-1, 3],
        backpacks: vec![-1, 0, 2],
        ..g.clone()
    };
    assert_eq!(worn.parts(), [3, 4, 2, 2, 3]);
    assert_eq!(worn.number_with(1, 2, 2, 3, 2), 1 + 3 * (2 + 4 * (1 + 2 * (1 + 2 * 2))));
    assert_eq!(worn.number_with(1, 2, 2, -1, -1), worn.number(1, 2, 2));
    assert_eq!(chimera_world::map::body_choice(&worn.parts(), 139, 4), 2);
    for (n, part, want) in [(19, 0, 1), (19, 1, 2), (19, 2, 1)] {
        assert_eq!(chimera_world::map::body_choice(&g.parts(), n, part), want);
    }
    let mut late = models::CaLateModels::default();
    late.0.register_group(g.clone());
    let loaded = models::Models::new(None, Some(&late));
    let d = builtin();
    // Model 1 (Light), face 2, head piece 2 (the combat helmet).
    let look = Selection {
        model: 1,
        face: 2,
        head: 2,
        ..default()
    };
    let head = if d.head_file(2, 0).is_some() { 2 } else { -1 };
    let ca = combat_arms::game::data::CaData::builtin();
    assert_eq!(models::drawn(&ca, loaded, &look), Some((models::body_name(0), g.number(1, 2, head))));
    // A female face: the female group, absent here.
    assert_eq!(models::drawn(&ca, loaded, &Selection { face: 5, ..default() }), None);
    assert_eq!(models::drawn(&ca, models::Models::default(), &look), None, "no group loaded");
}

#[test]
fn t04_team_marks() {
    let d = builtin();
    assert_eq!(d.team_mark(true, false), "CHARS_T_BODY\\RED_Tie.dtx");
    assert_eq!(d.team_mark(false, false), "CHARS_T_BODY\\Alpha.dtx");
    assert_eq!(d.team_mark(true, true), "CHARS_T_BODY\\Alpha.dtx");
}

#[test]
fn t05_head_pieces_and_goggles() {
    let d = builtin();
    assert_eq!(d.head_file(2, 0), Some("CHARS_M_HEAD\\Combat_Helmet.ltb"));
    assert_eq!(d.head_file(2, 1), Some("CHARS_M_HEAD\\W_Combat_Helmet.ltb"));
    assert_eq!(d.goggle_file(0, 1), Some(("Eyes", "ATTACH_M\\W_Glass.ltb")));
}

// ---------------------------------------------------------------------
// State -> animation, on the spec's names and lengths.

fn drv() -> Driver {
    Driver::new(driver::spec_library(false))
}

fn at(d: &Driver) -> (String, String, String) {
    (d.name_of(&d.upper).into(), d.name_of(&d.lower).into(), d.name_of(&d.crouch).into())
}

fn length(d: &Driver, t: &lithtech_formats::anim::Tracker) -> u32 {
    d.list.entries[t.current].length()
}

fn rifle() -> Inputs {
    Inputs {
        class: 0,
        ..default()
    }
}

fn run(d: &mut Driver, i: &Inputs, n: usize) {
    for _ in 0..n {
        d.update(TICK, i);
    }
}

#[test]
fn t06_standing_still() {
    let mut d = drv();
    let i = Inputs {
        crouch: 0.25,
        ..rifle()
    };
    run(&mut d, &i, 3);
    assert_eq!(at(&d), ("URFRAmS".into(), "LST".into(), "LC".into()));
    assert!(d.upper.looping && d.lower.looping);
    assert_eq!((d.upper.rate, d.lower.rate), (1.0, 1.0));
    assert_eq!(d.crouch.multiplier, 0.25);
}

#[test]
fn t07_walking() {
    let mut d = drv();
    run(&mut d, &Inputs { run: false, forward: true, ..rifle() }, 3);
    assert_eq!(at(&d).0, "URFWAmF");
    assert_eq!(at(&d).1, "LWF");
    let mut d = drv();
    run(&mut d, &Inputs { run: false, ..rifle() }, 3);
    assert_eq!(at(&d).0, "URFRAmS", "the walk name is missing");
}

#[test]
fn t08_running_directions() {
    for (keys, lower, upper) in [
        ((true, false, true, false), "LRFL", "URFRAmF"),
        ((false, true, false, true), "LRBR", "URFRAmB"),
        ((false, false, false, true), "LRR", "URFRAmR"),
    ] {
        let mut d = drv();
        let (forward, back, left, right) = keys;
        run(&mut d, &Inputs { forward, back, left, right, ..rifle() }, 2);
        assert_eq!((at(&d).1.as_str(), at(&d).0.as_str()), (lower, upper));
    }
    let mut d = drv();
    run(&mut d, &Inputs { forward: true, left: true, ..rifle() }, 1);
    assert_eq!(length(&d, &d.lower), 600);
}

#[test]
fn t09_sprint() {
    let mut d = drv();
    run(&mut d, &Inputs { sprint: true, forward: true, ..rifle() }, 2);
    assert_eq!((at(&d).1.as_str(), at(&d).0.as_str()), ("LRRF", "URFRRAmF"));
    assert_eq!(length(&d, &d.lower), 500);
    let mut d = drv();
    run(&mut d, &Inputs { sprint: true, forward: true, right: true, ..rifle() }, 2);
    assert_eq!(at(&d).1, "LRRR");
}

#[test]
fn t10_crouched() {
    let mut d = drv();
    run(&mut d, &Inputs { crouch: 1.0, crouch_key: true, run: false, ..rifle() }, 2);
    assert_eq!(at(&d), ("URFRAmS".into(), "LST".into(), "LC".into()));
    assert_eq!(d.crouch.multiplier, 1.0);
    let mut d = drv();
    run(&mut d, &Inputs { crouch: 1.0, crouch_key: true, run: false, forward: true, ..rifle() }, 2);
    assert_eq!(at(&d), ("URFWAmF".into(), "LWF".into(), "LCF".into()));
    assert_eq!(length(&d, &d.crouch), 1400);
}

#[test]
fn t11_jumps() {
    let mut d = drv();
    run(&mut d, &rifle(), 2);
    d.update(TICK, &Inputs { jumped: true, ..rifle() });
    assert_eq!(at(&d).1, "LSJ");
    assert!(!d.lower.looping);
    assert_eq!(length(&d, &d.lower), 666);
    // 666 ms in: still playing; done after 667.
    d.update(666, &rifle());
    assert_eq!(at(&d).1, "LSJ");
    assert!(!d.lower.done(&d.list));
    d.update(1, &rifle());
    assert_eq!(d.jump, Jump::Falling);
    assert_eq!(at(&d).1, "LST");
    // From running forward.
    let mut d = drv();
    let fwd = Inputs { forward: true, ..rifle() };
    run(&mut d, &fwd, 2);
    d.update(TICK, &Inputs { jumped: true, ..fwd.clone() });
    assert_eq!(at(&d).1, "LJJ");
    // Sprinting: the run jump, never LRRJF.
    let mut d = drv();
    let sprint = Inputs { sprint: true, forward: true, ..rifle() };
    run(&mut d, &sprint, 2);
    d.update(TICK, &Inputs { jumped: true, ..sprint.clone() });
    assert_eq!(at(&d).1, "LJJ");
    // Landing clears the jump.
    d.update(TICK, &Inputs { landed: true, ..sprint.clone() });
    assert_eq!(d.jump, Jump::None);
}

#[test]
fn t12_firing() {
    let mut d = drv();
    run(&mut d, &rifle(), 2);
    let fire = Inputs { fire: true, ..rifle() };
    d.update(TICK, &fire);
    assert_eq!(at(&d).0, "URFRFiS");
    assert!(!d.upper.looping);
    assert_eq!(length(&d, &d.upper), 100);
    // Held fire plays the animation once and holds its last frame
    // (Corrections 1): no restart while the posture and name stay.
    for _ in 1..=30 {
        d.update(TICK, &fire);
        assert_eq!(at(&d).0, "URFRFiS");
    }
    assert!(d.upper.done(&d.list));
    assert!(d.upper.time >= 100);
    // Running forward: no moving rifle fire in the library, the aim plays.
    let mut d = drv();
    let fwd = Inputs { forward: true, ..rifle() };
    run(&mut d, &fwd, 2);
    run(&mut d, &Inputs { fire: true, ..fwd.clone() }, 10);
    assert_eq!(at(&d).0, "URFRAmF");
    assert_eq!(d.last.posture, Posture::Fire);
}

#[test]
fn t13_reload_and_draw() {
    let mut d = drv();
    run(&mut d, &rifle(), 2);
    d.update(TICK, &Inputs { reloading: true, ..rifle() });
    assert_eq!(at(&d).0, "URFRSReS");
    assert_eq!(length(&d, &d.upper), 1866);
    // Started while moving: kept when the player stops.
    let mut d = drv();
    let fwd = Inputs { forward: true, ..rifle() };
    run(&mut d, &fwd, 2);
    d.update(TICK, &Inputs { reloading: true, ..fwd.clone() });
    assert_eq!(at(&d).0, "URFRReS");
    run(&mut d, &Inputs { reloading: true, ..rifle() }, 5);
    assert_eq!(at(&d).0, "URFRReS");
    // Draws.
    for (moving, sprint, want, len) in [(false, false, "URFRSSeS", Some(733)), (true, false, "URFRSeS", None), (true, true, "URFRRSeS", None)] {
        let mut d = drv();
        let base = Inputs { forward: moving, sprint, ..rifle() };
        run(&mut d, &base, 2);
        d.update(TICK, &Inputs { status: Some(driver::Status::Select), ..base.clone() });
        assert_eq!(at(&d).0, want);
        if let Some(l) = len {
            assert_eq!(length(&d, &d.upper), l);
        }
    }
}

#[test]
fn t14_pistol_and_knife() {
    let mut d = drv();
    let pistol = Inputs { class: 10, forward: true, ..default() };
    run(&mut d, &pistol, 2);
    d.update(TICK, &Inputs { fire: true, ..pistol.clone() });
    assert_eq!(at(&d).0, "UPRFiF");
    for keys in [(false, false, false, false), (true, false, false, false), (false, true, true, false), (false, false, false, true)] {
        let mut d = drv();
        let (forward, back, left, right) = keys;
        let knife = Inputs { class: 14, forward, back, left, right, ..default() };
        run(&mut d, &knife, 2);
        d.update(TICK, &Inputs { fire: true, ..knife.clone() });
        assert_eq!(at(&d).0, "UKRFiS", "{keys:?}");
        assert_eq!(length(&d, &d.upper), 500);
    }
    let mut d = drv();
    let knife = Inputs { class: 14, ..default() };
    run(&mut d, &knife, 2);
    d.update(TICK, &Inputs { alt_fire: true, ..knife.clone() });
    assert_eq!(at(&d).0, "UKRFiS2");
    assert_eq!(length(&d, &d.upper), 800);
}

#[test]
fn t15_grenade_throw() {
    let grenade = Inputs { class: 11, ..default() };
    let held = Inputs { fire: true, ..grenade.clone() };
    let mut d = drv();
    run(&mut d, &grenade, 2);
    d.update(TICK, &held);
    assert_eq!(at(&d).0, "UTRFiS_A");
    assert_eq!(length(&d, &d.upper), 267);
    // 267 ms later (done at 268): the 1 ms hold.
    let mut n = 0;
    while at(&d).0 == "UTRFiS_A" {
        d.update(TICK, &held);
        n += 1;
    }
    assert_eq!((at(&d).0.as_str(), n), ("UTRFiS_B", 18));
    run(&mut d, &held, 10);
    assert_eq!(at(&d).0, "UTRFiS_B", "held: never restarted");
    d.update(TICK, &grenade);
    assert_eq!(at(&d).0, "UTRFiS_C");
    assert_eq!(length(&d, &d.upper), 900);
    run(&mut d, &grenade, 59);
    assert_eq!(at(&d).0, "UTRFiS_C");
    run(&mut d, &grenade, 2);
    assert_eq!(at(&d).0, "UTRAmS");
    // The next grenade drawn meanwhile: its select.
    let mut d2 = drv();
    run(&mut d2, &grenade, 2);
    d2.update(TICK, &held);
    while at(&d2).0 != "UTRFiS_C" {
        d2.update(TICK, if at(&d2).0 == "UTRFiS_B" { &grenade } else { &held });
    }
    run(&mut d2, &grenade, 60);
    d2.update(TICK, &Inputs { status: Some(driver::Status::Select), ..grenade.clone() });
    assert_eq!(at(&d2).0, "UTRSSeS");
}

#[test]
fn t16_hit_reactions() {
    for (coin, name, len) in [(true, "D_HIT", 467), (false, "D_HITT", 400)] {
        let mut d = drv();
        run(&mut d, &rifle(), 2);
        let hit = Inputs { status: Some(driver::Status::Hit), damaged: true, coin, ..rifle() };
        d.update(TICK, &hit);
        assert_eq!(at(&d).0, name);
        assert_eq!(length(&d, &d.upper), len);
        assert_eq!(d.list.entries[d.upper.current].animation.interp_ms, 200);
        // Fire held meanwhile: no fire animation until the hit is done
        // (200 ms blend, then its length).
        let fire = Inputs { fire: true, damaged: true, ..rifle() };
        let mut ms = TICK;
        while at(&d).0 == name {
            d.update(TICK, &fire);
            ms += TICK;
        }
        assert_eq!(at(&d).0, "URFRFiS");
        assert!(ms > 200 + len, "{ms}");
    }
    // Spawn-protected: no reaction, the hit status sticks (Quirk 4).
    let mut d = drv();
    run(&mut d, &rifle(), 2);
    d.update(TICK, &Inputs { status: Some(driver::Status::Hit), damaged: true, protected: true, ..rifle() });
    run(&mut d, &Inputs { fire: true, ..rifle() }, 5);
    assert_eq!(at(&d).0, "URFRAmS");
}

#[test]
fn t17_rates() {
    assert_eq!(driver::upper_rate(1866, 1.0, 0), 1.0);
    assert!((driver::upper_rate(1866, 1.0, 2300) - 0.8113).abs() < 1e-4);
    // The reload at HHAniFrame_5 = 2300 lasts 2300 ms.
    let mut d = drv();
    run(&mut d, &rifle(), 2);
    let mut frames = vec![0; 10];
    frames[5] = 2300;
    let reload = Inputs { reloading: true, frames, ..rifle() };
    d.update(TICK, &reload);
    assert!((d.upper.rate - 0.8113).abs() < 1e-4);
    let mut ms = 0;
    // Done (and the aim back) once its 1866 ms ran at that rate.
    while d.name_of(&d.upper) == "URFRSReS" && ms < 10_000 {
        d.update(TICK, &reload);
        ms += TICK;
    }
    assert!((2295..=2320).contains(&ms), "{ms}");
    // The legs at 1.0 whatever the movement.
    let mut d = drv();
    run(&mut d, &Inputs { forward: true, sprint: true, ..rifle() }, 3);
    assert_eq!((d.lower.rate, d.crouch.rate), (1.0, 1.0));
}

// ---------------------------------------------------------------------
// Aim

/// The aim nodes in a chain up the body, each 10 units above the last,
/// a hand 20 units out from each shoulder.
fn aim_chain() -> Posed {
    let names = [
        ("Right_pelvis", None, [0.0, 0.0, 0.0]),
        ("Torso", Some(0), [0.0, 10.0, 0.0]),
        ("Upper_torso", Some(1), [0.0, 20.0, 0.0]),
        ("Neck", Some(2), [0.0, 30.0, 0.0]),
        ("Head", Some(3), [0.0, 35.0, 0.0]),
        ("Left_shoulder", Some(3), [-5.0, 28.0, 0.0]),
        ("Right_shoulder", Some(3), [5.0, 28.0, 0.0]),
        ("Right_hand", Some(6), [5.0, 28.0, 20.0]),
        ("Face", Some(4), [0.0, 40.0, 5.0]),
    ];
    Posed {
        shape: std::sync::Arc::new(Shape::new(
            names.iter().map(|n| n.0.to_string()).collect(),
            names.iter().map(|n| n.1).collect(),
            Vec::new(),
        )),
        globals: names.iter().map(|n| (Vec3::from(n.2), Quat::IDENTITY)).collect(),
        feet: -50.0,
    }
}

fn male_aim() -> Vec<String> {
    builtin().skeleton(0).aim.clone()
}

fn angle_of(p: &Posed, name: &str) -> f32 {
    p.globals[p.node(name).unwrap()].1.angle_between(Quat::IDENTITY).to_degrees()
}

#[test]
fn t18_twist_shares() {
    let mut p = aim_chain();
    p.aim(&male_aim(), 0.0, 45f32.to_radians(), false);
    assert!((angle_of(&p, "Torso") - 18.53).abs() < 0.01);
    assert!((angle_of(&p, "Upper_torso") - 45.0).abs() < 0.01);
    for n in ["Neck", "Head", "Right_shoulder", "Right_hand"] {
        assert!((angle_of(&p, n) - 45.0).abs() < 0.01, "{n} adds nothing");
    }
    assert_eq!(angle_of(&p, "Right_pelvis"), 0.0);
}

#[test]
fn t19_pitch_shares() {
    for (deg, arm, head, server_arm) in [(-20.0f32, 20.0, 21.0, 17.0), (20.0, 20.0, 25.0, 13.0)] {
        let mut p = aim_chain();
        p.aim(&male_aim(), deg.to_radians(), 0.0, false);
        assert!((angle_of(&p, "Right_hand") - arm).abs() < 0.01, "arms {deg}");
        assert!((angle_of(&p, "Face") - head).abs() < 0.01, "head {deg}");
        let mut p = aim_chain();
        p.aim(&male_aim(), deg.to_radians(), 0.0, true);
        assert!((angle_of(&p, "Right_hand") - server_arm).abs() < 0.01, "server arms {deg}");
        // Down bows forward (+Z in the spec's frame).
        let hand = p.globals[p.node("Right_hand").unwrap()].0;
        assert_eq!(hand.y < 28.0, deg > 0.0);
    }
}

#[test]
fn t20_clamp_and_bytes() {
    let mut a = aim_chain();
    let mut b = aim_chain();
    a.aim(&male_aim(), 60f32.to_radians(), 0.0, false);
    b.aim(&male_aim(), 45f32.to_radians(), 0.0, false);
    assert_eq!(a.globals, b.globals);
    assert_eq!(skeleton::aim_byte(30f32.to_radians()), 42);
    assert!((skeleton::from_aim_byte(42).to_degrees() - 29.764).abs() < 1e-3);
    assert_eq!(skeleton::aim_byte(-45f32.to_radians()), -63);
    assert!((skeleton::from_aim_byte(-63).to_degrees() + 44.646).abs() < 1e-3);
}

/// Cases 21 and 22 measure the install's poses: `heavy::characters`. On
/// the spec's skeleton the head and name tag pitch about the neck chain.
#[test]
fn t21_22_aim_moves_the_spec_skeleton() {
    for skel in [0, 1] {
        let s = builtin().skeleton(skel);
        let rest = skeleton::fallback(s, 1.0);
        let head = if skel == 0 { "Head" } else { "Bip01 Head" };
        let mut down = rest.clone();
        down.aim(&s.aim, 30f32.to_radians(), 0.0, false);
        let (a, b) = (rest.sphere(head).unwrap(), down.sphere(head).unwrap());
        assert!(b.y < a.y && b.z > a.z, "{a} -> {b}");
        let mut up = rest.clone();
        up.aim(&s.aim, -30f32.to_radians(), 0.0, false);
        assert!(up.sphere(head).unwrap().y > a.y - 1e-3);
        // Legs never move.
        assert_eq!(down.sphere("Left_foot"), rest.sphere("Left_foot"));
    }
}

// ---------------------------------------------------------------------
// Stand-in models: nodes at given bind positions (identity rotations),
// sockets, the two weight sets, constant animations of given lengths.

struct StandIn {
    nodes: Vec<(&'static str, Option<usize>, [f32; 3])>,
    sockets: Vec<(&'static str, usize, [f32; 3], [f32; 4])>,
}

fn translation(p: [f32; 3]) -> ltb::Mat4 {
    [[1.0, 0.0, 0.0, p[0]], [0.0, 1.0, 0.0, p[1]], [0.0, 0.0, 1.0, p[2]], [0.0, 0.0, 0.0, 1.0]]
}

impl StandIn {
    /// The model with `anims` (name, length ms, root translation).
    fn model(&self, anims: &[(&str, u32)]) -> Model {
        let n = self.nodes.len();
        let local = |j: usize| {
            let (_, parent, p) = self.nodes[j];
            let base = parent.map_or([0.0; 3], |q| self.nodes[q].2);
            [p[0] - base[0], p[1] - base[1], p[2] - base[2]]
        };
        Model {
            version: 25,
            counts: Default::default(),
            command: String::new(),
            radius: 50.0,
            pieces: Vec::new(),
            nodes: self
                .nodes
                .iter()
                .enumerate()
                .map(|(j, (name, parent, p))| Node {
                    name: name.to_string(),
                    stored_index: j as u16,
                    flags: 0,
                    bind: translation(*p),
                    parent: *parent,
                })
                .collect(),
            weight_sets: vec![
                WeightSet {
                    name: "upper".into(),
                    weights: vec![1.0; n],
                },
                WeightSet {
                    name: "Lower".into(),
                    weights: vec![0.0; n],
                },
            ],
            children: Vec::new(),
            animations: anims
                .iter()
                .map(|(name, len)| Animation {
                    dims: if name.starts_with("LC") { [15.0, 40.0, 15.0] } else { [15.0, 50.0, 15.0] },
                    name: name.to_string(),
                    compression: 0,
                    interp_ms: 0,
                    keys: vec![
                        Key {
                            time: 0,
                            string: String::new(),
                        },
                        Key {
                            time: *len,
                            string: String::new(),
                        },
                    ],
                    channels: (0..n)
                        .map(|j| Channel::Transform {
                            positions: vec![local(j)],
                            rotations: vec![[0.0, 0.0, 0.0, 1.0]],
                        })
                        .collect(),
                })
                .map(std::sync::Arc::new)
                .collect(),
            sockets: self
                .sockets
                .iter()
                .map(|(name, node, p, q)| Socket {
                    node: *node as u32,
                    name: name.to_string(),
                    rotation: *q,
                    position: *p,
                    scale: [1.0; 3],
                })
                .collect(),
            bindings: Vec::new(),
        }
    }

    fn rig(&self, anims: &[(&str, u32)]) -> Rig {
        let model = self.model(anims);
        let list = AnimList::build(&model, &[]);
        Rig::new(model, list)
    }
}

/// The default male's skeleton by the spec's bind positions: every node
/// sphere's node, the socket spheres' parents, and the sockets.
fn male_stand_in(with_nuts: bool) -> StandIn {
    let s = builtin().skeleton(0);
    let mut nodes: Vec<(&'static str, Option<usize>, [f32; 3])> = Vec::new();
    for n in &s.nodes {
        let parent = n.parent.as_ref().and_then(|p| nodes.iter().position(|m| m.0 == p.as_str()));
        let bind = s.sphere(&n.name).and_then(|sp| sp.bind).unwrap_or(n.at.unwrap_or_default());
        nodes.push((n.name.clone().leak(), parent, bind));
    }
    let mut sockets = Vec::new();
    for sp in &s.spheres {
        if let Some(place) = &sp.socket {
            if sp.name == "Nuts" && !with_nuts {
                continue;
            }
            let parent = nodes.iter().position(|m| m.0 == place.parent.as_str()).unwrap();
            sockets.push((sp.name.clone().leak() as &'static str, parent, place.offset, [0.0, 0.0, 0.0, 1.0]));
        }
    }
    for so in &s.sockets {
        let parent = nodes.iter().position(|m| m.0 == so.parent.as_str()).unwrap();
        sockets.push((so.name.clone().leak() as &'static str, parent, so.offset, so.rotation));
    }
    StandIn { nodes, sockets }
}

const STAND_IN_ANIMS: &[(&str, u32)] = &[("baseAnim", 4800), ("URFRAmS", 4000), ("LST", 4000), ("LC", 4000), ("LRF", 600), ("DT", 3033)];

// ---------------------------------------------------------------------
// Sockets

#[test]
fn t23_right_hand_socket() {
    let s = builtin().skeleton(0).socket("RightHand").unwrap();
    assert_eq!((s.parent.as_str(), s.offset, s.rotation), ("Right_hand", [7.085, 1.661, 1.277], [0.0573, -0.7151, -0.0818, 0.6918]));
    // On a stand-in, the socket is its node's transform times the offset.
    let rig = male_stand_in(true).rig(STAND_IN_ANIMS);
    let bind = rig.bind();
    let hand = bind.globals[bind.node("Right_hand").unwrap()].0;
    let (p, _) = bind.socket("RightHand").unwrap();
    assert!(close(p, hand + Vec3::from(s.offset), 1e-4));
    // And with the node turned: the offset turns with it.
    let mut turned = bind.clone();
    let j = turned.node("Right_hand").unwrap();
    turned.globals[j].1 = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let (p, _) = turned.socket("RightHand").unwrap();
    assert!(close(p, hand + Vec3::new(1.277, 1.661, -7.085), 1e-4), "{p}");
}

/// Case 24's "stand" pose on a player: the tag sits at P + (8.63, 45.87,
/// 11.51) turned by its yaw (our frame: Z mirrored).
#[test]
fn t24_name_socket() {
    let s = builtin().skeleton(0).socket("NAME_socket").unwrap();
    assert_eq!((s.parent.as_str(), s.offset), ("Head", [16.5, 0.0, 0.906]));
    assert_eq!(builtin().skeleton(1).socket("NAME_socket").unwrap().parent, "Bip01 Head");
    let mut sim = sim();
    let yaw = 0.7;
    let p = player(&mut sim, Vec3::new(0.0, 1.0, 12.0), yaw);
    let world = sim.app.world();
    let pose = world.get::<CaPose>(p).unwrap();
    let (t, state) = (world.get::<Transform>(p).unwrap(), world.get::<MovementState>(p).unwrap());
    let tag = pose.world(t, state, UNIT, pose.name_tag.unwrap());
    let want = t.translation + Quat::from_rotation_y(yaw) * (Vec3::new(8.63, 45.87, -11.51) * UNIT);
    assert!(close(tag, want, 1e-4), "{tag} vs {want}");
}

#[test]
#[allow(clippy::approx_constant)] // the data's quaternions as stored
fn t25_26_sockets() {
    let m = builtin().skeleton(0);
    let f = builtin().skeleton(1);
    let eyes = m.socket("Eyes").unwrap();
    assert_eq!((eyes.parent.as_str(), eyes.offset, eyes.bind), ("Face_Eyelid", [3.011, 0.0, 1.314], [0.03, 41.38, 5.68]));
    let eyes = f.socket("Eyes").unwrap();
    assert_eq!((eyes.parent.as_str(), eyes.offset, eyes.bind), ("Bip01 Head", [-85.255, 0.001, 18.748], [0.08, 2.14, 5.20]));
    let back = m.socket("1W_N").unwrap();
    assert_eq!((back.parent.as_str(), back.offset, back.rotation), ("Upper_torso", [-18.328, 0.0, 0.0], [0.0, 0.0, -0.7071, 0.7071]));
    let left = m.socket("LeftHand").unwrap();
    assert_eq!((left.parent.as_str(), left.offset, left.bind), ("left_hand", [0.0, 0.0, 0.0], [-31.19, 4.32, 2.14]));
    // On the stand-in: the left hand's socket is its node; the backpack's
    // offset turns by the node's turn and its own rotation composes.
    let rig = male_stand_in(true).rig(STAND_IN_ANIMS);
    let bind = rig.bind();
    let (lh, q) = bind.socket("LeftHand").unwrap();
    assert!(close(lh, Vec3::new(-31.19, 4.32, 2.14), 1e-4));
    assert!(q.angle_between(Quat::IDENTITY) < 1e-4, "(0, 0, 0, 0.7071) normalised is identity");
}

/// Case 27: the death drop at the left foot, as the stand pose has it.
#[test]
fn t27_death_drop_at_the_left_foot() {
    for (face, foot) in [(0, Vec3::new(-10.02, -41.49, 13.52)), (5, Vec3::new(-7.73, -39.56, 12.91))] {
        let mut sim = sim();
        let p = player(&mut sim, Vec3::new(0.0, 1.0, 12.0), 0.0);
        sim.app.world_mut().entity_mut(p).insert(Selection { face, ..default() });
        let w = weapon::give(sim.app.world_mut(), p, RIFLE).unwrap();
        sim.seconds(1.3);
        assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(w));
        assert_eq!(sim.app.world().get::<CaAnim>(p).unwrap().skeleton, usize::from(face == 5));
        let centre = sim.position(p);
        kill(&mut sim, p, Vec3::NEG_Z, Hitgroup::Chest, DamageKind::Bullet);
        sim.ticks(2);
        let world = sim.app.world_mut();
        let mut q = world.query::<(&Transform, &inventory::Dropped)>();
        let (t, _) = q.single(world).expect("the rifle dropped");
        let want = centre + Vec3::new(foot.x, foot.y, -foot.z) * UNIT;
        assert!(close(t.translation, want, 1e-3), "face {face}: {} vs {want}", t.translation);
    }
}

// ---------------------------------------------------------------------
// Hit spheres

#[test]
fn t28_bind_centres_on_a_stand_in() {
    let rig = male_stand_in(true).rig(STAND_IN_ANIMS);
    let bind = rig.bind();
    for s in &builtin().skeleton(0).spheres {
        if s.radius <= 0.0 || s.absent || s.socket.is_some() {
            continue;
        }
        let at = bind.sphere(&s.name).unwrap_or_else(|| panic!("{}", s.name));
        assert!(close(at, Vec3::from(s.bind.unwrap()), 1e-3), "{}: {at}", s.name);
    }
    // Socket spheres: their node's position plus the offset (identity
    // nodes here; the install's rotations in heavy::characters).
    let nuts = bind.sphere("Nuts").unwrap();
    assert!(close(nuts, Vec3::new(0.0, -7.70, 0.75) + Vec3::new(2.0, 0.0, 6.0), 1e-3));
    let t = MALE.iter().find(|s| s.name == "Upper_torso").unwrap();
    assert_eq!(t.radius, 10.0);
}

/// Case 29's poses (LST at t = 0) are the spec's skeleton: a player at
/// rest has every sphere there.
#[test]
fn t29_lst_centres() {
    let mut sim = sim();
    let p = player(&mut sim, Vec3::new(0.0, 1.0, 12.0), 0.0);
    let world = sim.app.world();
    let pose = world.get::<CaPose>(p).unwrap();
    let skel = builtin().skeleton(0);
    for (s, at) in skel.spheres.iter().zip(&pose.spheres) {
        match (s.at, at) {
            (Some(want), Some(got)) if s.radius > 0.0 => {
                let want = Vec3::new(want[0], want[1] + 50.0, -want[2]);
                assert!(close(*got, want, 1e-3), "{}: {got} vs {want}", s.name);
            }
            _ => assert!(s.radius <= 0.0 || s.absent || at.is_none(), "{}", s.name),
        }
    }
    let head = pose.spheres[skel.spheres.iter().position(|s| s.name == "Head").unwrap()].unwrap();
    assert!(close(head, Vec3::new(6.45, 79.03, -10.90), 1e-3));
}

/// Case 30 (`LRF` at 300 ms) is an install case: on a stand-in whose
/// `LRF` moves the left foot, the sphere follows the animation's time.
#[test]
fn t30_spheres_follow_the_animation() {
    let stand_in = male_stand_in(true);
    let mut model = stand_in.model(STAND_IN_ANIMS);
    let foot = model.node("Left_foot").unwrap();
    let lrf = model.animation("LRF").unwrap();
    let parent = stand_in.nodes[foot].1.unwrap();
    let base = Vec3::from(stand_in.nodes[foot].2) - Vec3::from(stand_in.nodes[parent].2);
    std::sync::Arc::make_mut(&mut model.animations[lrf]).channels[foot] = Channel::Transform {
        positions: vec![base.to_array(), (base + Vec3::new(0.0, 0.0, 20.0)).to_array()],
        rotations: vec![[0.0, 0.0, 0.0, 1.0]],
    };
    // Legs from the lower tracker: a Lower weight on the leg.
    for (j, w) in model.weight_sets[1].weights.iter_mut().enumerate() {
        *w = if j == foot { 1.0 } else { 0.0 };
    }
    let list = AnimList::build(&model, &[]);
    let rig = Rig::new(model, list);
    let mut d = Driver::new(rig.list.clone());
    let fwd = Inputs { forward: true, ..rifle() };
    d.update(0, &fwd);
    assert_eq!(d.name_of(&d.lower), "LRF");
    let base_pose = rig.posed(&rig.layered(&d.main, &d.upper, &d.lower, &d.crouch, 0.0), 0.0);
    d.update(300, &fwd);
    let later = rig.posed(&rig.layered(&d.main, &d.upper, &d.lower, &d.crouch, 0.0), 0.0);
    let moved = later.sphere("Left_foot").unwrap() - base_pose.sphere("Left_foot").unwrap();
    assert!(close(moved, Vec3::new(0.0, 0.0, 10.0), 1e-3), "{moved}");
}

/// Case 31 (male stand and crouch) is an install case; on the spec's
/// skeleton crouching lowers the head with the hull.
#[test]
fn t31_crouch_lowers_the_spheres() {
    let mut sim = sim();
    let p = player(&mut sim, Vec3::new(0.0, 1.0, 12.0), 0.0);
    let skel = builtin().skeleton(0);
    let head = skel.spheres.iter().position(|s| s.name == "Head").unwrap();
    let stand = sim.app.world().get::<CaPose>(p).unwrap().spheres[head].unwrap();
    sim.intent(p).crouch = true;
    sim.seconds(1.0);
    let crouched = sim.app.world().get::<CaPose>(p).unwrap().spheres[head].unwrap();
    let state = sim.state(p);
    let ratio = (state.hull_max.y - state.hull_min.y) / (100.0 * UNIT);
    assert!(ratio < 0.9, "crouched: {ratio}");
    assert!((crouched.y - stand.y * ratio).abs() < 1e-3, "{stand} -> {crouched}");
}

/// Case 32 (female bind and LST) on the spec's data, case 33 (female
/// crouch) in heavy::characters.
#[test]
fn t32_female_table() {
    let mut sim = sim();
    let p = player(&mut sim, Vec3::new(0.0, 1.0, 12.0), 0.0);
    sim.app.world_mut().entity_mut(p).insert(Selection { face: 6, ..default() });
    sim.ticks(2);
    let world = sim.app.world();
    let pose = world.get::<CaPose>(p).unwrap();
    assert_eq!(pose.skeleton, 1);
    let skel = builtin().skeleton(1);
    let i = skel.spheres.iter().position(|s| s.name == "Headfuck").unwrap();
    assert!(close(pose.spheres[i].unwrap(), Vec3::new(4.12, 83.37, -12.42), 1e-3));
    let i = skel.spheres.iter().position(|s| s.name == "Bip01 Head").unwrap();
    assert!(close(pose.spheres[i].unwrap(), Vec3::new(2.96, 81.08, -10.21), 1e-3));
    assert_eq!(skel.sphere("Headfuck").unwrap().bind, Some([0.08, 92.36, 3.53]));
    let hs = world.get::<combat_arms::game::weapons::HitSpheres>(p).unwrap();
    assert!(std::ptr::eq(hs.0, *FEMALE));
}

#[test]
fn t34_sphere_counts() {
    let count = |table: &[combat_arms::game::weapons::HitSphere]| table.iter().filter(|s| s.radius > 0.0).count();
    let d = builtin();
    assert_eq!(d.skeleton(0).spheres.iter().filter(|s| s.radius > 0.0).count(), 35);
    assert_eq!(count(&MALE), 33, "Left_Shld and Right_Shld never exist");
    assert_eq!(d.skeleton(1).spheres.iter().filter(|s| s.radius > 0.0).count(), 35);
    assert_eq!(count(&FEMALE), 35);
    // A stand-in with the default male's nodes and sockets: 33 found.
    let bind = male_stand_in(true).rig(STAND_IN_ANIMS).bind();
    let found = d.skeleton(0).spheres.iter().filter(|s| s.radius > 0.0 && bind.sphere(&s.name).is_some()).count();
    assert_eq!(found, 33);
}

#[test]
fn t35_no_groin_without_the_socket() {
    let bind = male_stand_in(false).rig(STAND_IN_ANIMS).bind();
    assert!(bind.sphere("Nuts").is_none());
    assert!(bind.sphere("Torso").is_some());
}

// ---------------------------------------------------------------------
// Death and bodies

fn death_of(kind: DeathKind, dir: Vec3, head: bool, groin: bool, coin: bool, female: bool) -> String {
    let list = driver::spec_library(female);
    death::choose(
        &DeathInputs {
            kind,
            dir,
            forward: Vec3::Z,
            right: Vec3::X,
            head,
            groin,
            coin,
            jumping: false,
            special: None,
        },
        |n| list.find(n).is_some(),
    )
}

#[test]
fn t36_front_deaths() {
    let d = Vec3::NEG_Z;
    assert_eq!(death_of(DeathKind::Other, d, true, false, true, false), "DH");
    assert_eq!(death_of(DeathKind::Other, d, true, false, false, false), "DH_1");
    assert_eq!(death_of(DeathKind::Other, d, false, false, true, false), "DT");
    assert_eq!(death_of(DeathKind::Other, d, false, false, false, false), "DT_1");
}

#[test]
fn t37_back_deaths() {
    let d = Vec3::Z;
    assert_eq!(death_of(DeathKind::Other, d, false, false, true, false), "DTB");
    assert_eq!(death_of(DeathKind::Other, d, false, false, false, false), "DTB_1");
    assert_eq!(death_of(DeathKind::Other, d, true, false, true, false), "DHB");
    assert_eq!(death_of(DeathKind::Other, d, true, false, false, false), "DHB_1");
}

#[test]
fn t38_side_deaths() {
    let a = Vec3::new(0.94, 0.0, -0.34);
    // Front-left: the head case `DLSH`, the body case the missing-handle
    // fallback `DC` (Corrections 2).
    assert_eq!(death_of(DeathKind::Other, a, false, false, true, false), "DC");
    assert_eq!(death_of(DeathKind::Other, a, true, false, true, false), "DLSH");
    let b = Vec3::new(0.94, 0.0, 0.34);
    assert_eq!(death_of(DeathKind::Other, b, false, false, true, false), "DLT");
    assert_eq!(death_of(DeathKind::Other, b, true, false, true, false), "DLSH");
    let c = Vec3::new(-0.94, 0.0, -0.34);
    assert_eq!(death_of(DeathKind::Other, c, false, false, true, false), "DRT");
    assert_eq!(death_of(DeathKind::Other, c, true, false, true, false), "DRSH");
}

#[test]
fn t39_other_deaths() {
    let d = Vec3::NEG_Z;
    assert_eq!(death_of(DeathKind::Explosion, d, false, false, true, false), "D_BO_F");
    assert_eq!(death_of(DeathKind::Melee, d, false, false, true, false), "D_S");
    assert_eq!(death_of(DeathKind::Poison, d, false, false, true, false), "D_P");
    assert_eq!(death_of(DeathKind::EndlessFall, d, false, false, true, false), "D_W");
    assert_eq!(death_of(DeathKind::Other, d, false, true, true, true), "DC", "female: no NS1");
    // Location 5 (BR data, ours): the groin deaths; location 2 (US): torso.
    assert_eq!(death_of(DeathKind::Other, d, false, true, true, false), "NS1");
    assert_eq!(death_of(DeathKind::Other, d, false, true, false, false), "NS2");
    assert_eq!(death_of(DeathKind::Other, d, false, false, true, false), "DT");
    // A jump forces the coin to 0.
    let list = driver::spec_library(false);
    let jumping = DeathInputs {
        kind: DeathKind::Other,
        dir: d,
        forward: Vec3::Z,
        right: Vec3::X,
        head: false,
        groin: false,
        coin: true,
        jumping: true,
        special: None,
    };
    assert_eq!(death::choose(&jumping, |n| list.find(n).is_some()), "DT_1");
    // The body turns away from the shot: yaw atan(dx/dz), + pi when dz > 0.
    assert!((death::jump_yaw(Vec3::new(0.0, 0.0, -1.0))).abs() < 1e-6);
    assert!((death::jump_yaw(Vec3::new(0.0, 0.0, 1.0)) - std::f32::consts::PI).abs() < 1e-6);
    assert!((death::jump_yaw(Vec3::new(1.0, 0.0, 0.0)) - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
}

#[test]
fn t40_crouched_deaths_stand() {
    // The crouched variants are never chosen: a crouched front torso
    // death is DT / DT_1.
    let d = Vec3::NEG_Z;
    assert_eq!(death_of(DeathKind::Other, d, false, false, true, false), "DT");
    assert_eq!(death_of(DeathKind::Other, d, false, false, false, false), "DT_1");
}

#[test]
fn t41_body_lifetime() {
    let tdm = builtin().body_removal(combat_arms::game::rules::TDM);
    assert_eq!(tdm, Some(5.0));
    let end = 3.033 + 0.001;
    assert!((death::removal_time(end, Some(2.0), tdm).unwrap() - 8.034).abs() < 1e-9);
    assert!((death::removal_time(end, Some(10.0), tdm).unwrap() - 15.0).abs() < 1e-9);
    assert_eq!(death::removal_time(end, None, tdm), None, "not before the respawn");
    assert!((death::removal_time(end, Some(10.0), Some(500.0)).unwrap() - 510.0).abs() < 1e-9, "Combat");
    assert_eq!(death::removal_time(end, Some(10.0), None), None, "other modes: never");
}

/// Case 42: the body 0.1 u above the dead player, without the weapon;
/// the death animation on its owner's main tracker.
#[test]
fn t42_body_creation() {
    let mut sim = sim();
    let p = player(&mut sim, Vec3::new(0.0, 1.0, 12.0), 0.0);
    weapon::give(sim.app.world_mut(), p, RIFLE).unwrap();
    sim.seconds(1.3);
    let centre = sim.position(p);
    // Shot from the front in the chest: DT or DT_1.
    kill(&mut sim, p, Vec3::Z, Hitgroup::Chest, DamageKind::Bullet);
    sim.ticks(2);
    let world = sim.app.world_mut();
    let mut q = world.query::<&CaBody>();
    let body = q.single(world).expect("a body").clone();
    assert!(close(body.at, centre + Vec3::Y * 0.1 * UNIT, 1e-4), "{} vs {centre}", body.at);
    assert!(["DT", "DT_1"].contains(&body.animation.as_str()), "{}", body.animation);
    let anim = world.get::<CaAnim>(p).unwrap();
    assert_eq!(anim.death.as_deref(), Some(body.animation.as_str()));
    assert_eq!(anim.driver.name_of(&anim.driver.main), body.animation);
    assert!(!anim.driver.main.looping);
}

// ---------------------------------------------------------------------
// Scenario: the driver on a player.

fn player(sim: &mut Sim, at: Vec3, yaw: f32) -> Entity {
    let p = sim.spawn_character(at, movement::ID);
    sim.intent(p).yaw = yaw;
    sim.seconds(0.5);
    sim.app.world_mut().entity_mut(p).insert(combat_arms::game::weapons::gear_armour(0.0, &Vec::new()));
    p
}

fn kill(sim: &mut Sim, p: Entity, dir: Vec3, hitgroup: Hitgroup, kind: DamageKind) {
    let point = sim.position(p);
    sim.app.world_mut().write_message(Damage {
        target: p,
        attacker: None,
        amount: 10.0,
        point,
        dir,
        hitgroup,
        kind,
        weapon: None,
        force: Vec3::ZERO,
    });
}

fn playing(sim: &Sim, p: Entity) -> (String, String, String) {
    let a = sim.app.world().get::<CaAnim>(p).unwrap();
    let (u, l, c) = a.playing();
    (u.into(), l.into(), c.into())
}

/// A player with the rifle: still, walking (the walk key), running,
/// sprinting, firing, reloading; the hit spheres pitch with the look.
#[test]
fn scenario_player_states() {
    let mut sim = sim();
    let p = player(&mut sim, Vec3::new(0.0, 1.0, 12.0), 0.0);
    let w = weapon::give(sim.app.world_mut(), p, RIFLE).unwrap();
    sim.seconds(1.3);
    assert_eq!(playing(&sim, p), ("URFRAmS".into(), "LST".into(), "LC".into()));
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(0.3);
    assert_eq!(playing(&sim, p).1, "LRF");
    assert_eq!(playing(&sim, p).0, "URFRAmF");
    sim.intent(p).walk = true;
    sim.seconds(0.3);
    assert_eq!(playing(&sim, p).1, "LWF");
    assert_eq!(playing(&sim, p).0, "URFWAmF");
    sim.intent(p).walk = false;
    sim.intent(p).sprint = true;
    sim.seconds(0.3);
    assert_eq!(playing(&sim, p).1, "LRRF");
    sim.intent(p).sprint = false;
    sim.intent(p).move_axis = Vec2::ZERO;
    sim.seconds(0.5);
    assert_eq!(playing(&sim, p).1, "LST");
    sim.intent(p).fire = true;
    sim.ticks(2);
    assert_eq!(playing(&sim, p).0, "URFRFiS");
    sim.intent(p).fire = false;
    sim.seconds(0.3);
    assert_eq!(playing(&sim, p).0, "URFRAmS");
    sim.app.world_mut().get_mut::<Magazine>(w).unwrap().clip = 10;
    sim.intent(p).reload = true;
    sim.ticks(2);
    sim.intent(p).reload = false;
    assert_eq!(playing(&sim, p).0, "URFRSReS");
    // Looking down lowers the head sphere.
    let head = builtin().skeleton(0).spheres.iter().position(|s| s.name == "Head").unwrap();
    let level = sim.app.world().get::<CaPose>(p).unwrap().spheres[head].unwrap();
    sim.intent(p).pitch = -0.5;
    sim.ticks(1);
    let down = sim.app.world().get::<CaPose>(p).unwrap().spheres[head].unwrap();
    assert!(down.y < level.y - 1.0, "{level} -> {down}");
}

/// With a body model (a stand-in registered for body 1, the "Light"
/// record): the pose is the model's, its spheres on its nodes, its
/// sockets the model's.
#[test]
fn scenario_modelled_body() {
    let file = &builtin().model(1).unwrap().file;
    let mut stand_in = male_stand_in(true);
    // Shift every node by 5 units on X so the model's pose is told from
    // the spec's skeleton.
    for n in &mut stand_in.nodes {
        n.2[0] += 5.0;
    }
    let mut sim = sim();
    models::register(sim.app.world_mut(), file, Arc::new(stand_in.rig(STAND_IN_ANIMS)));
    let p = player(&mut sim, Vec3::new(0.0, 1.0, 12.0), 0.0);
    sim.app.world_mut().entity_mut(p).insert(Selection { model: 1, ..default() });
    sim.ticks(3);
    let pose = sim.app.world().get::<CaPose>(p).unwrap().clone();
    assert!(pose.modelled);
    assert_eq!(playing(&sim, p), ("URFRAmS".into(), "LST".into(), "LC".into()));
    let skel = builtin().skeleton(0);
    let torso = skel.spheres.iter().position(|s| s.name == "Torso").unwrap();
    // The stand-in's bind Torso (5, 5.14, 0.74) with its feet at -50.
    assert!(close(pose.spheres[torso].unwrap(), Vec3::new(5.0, 55.14, -0.74), 1e-3), "{:?}", pose.spheres[torso]);
    let groin = skel.spheres.iter().position(|s| s.name == "Nuts").unwrap();
    assert!(pose.spheres[groin].is_some());
    let shoulder = skel.spheres.iter().position(|s| s.name == "Left_Shld").unwrap();
    assert!(pose.spheres[shoulder].is_none());
}

/// A character changing bodies keeps the body showing, on its own
/// skeleton, until the new one is ready (a load on demand: backlog
/// "changing Combat Arms characters hitches and garbles"): the new body's
/// skeleton (here of another shape, as a male's and a female's are) goes
/// on `CaAnim`, for the hit spheres, at once, but the pose going to the
/// drawing stays the old skeleton's until the new model is in; then both
/// change together.
#[test]
fn a_character_change_keeps_the_shown_body_on_its_own_skeleton() {
    use chimera_world::map::{BodyModel, BodyName, CharacterModels, MapBone, MapCharacterModel, anim::{AnimSet, Animator}};
    use combat_arms::game::{GAME, characters::{CaShownAnim, models::BodyGroup}, view_model::on_demand::LoadOnDemand};
    let model_of = |id: i32, stand_in: &StandIn| MapCharacterModel {
        game: Some(GAME),
        team: None,
        name: Some(models::body_name(id)),
        class: None,
        model: Default::default(),
        hitboxes: Vec::new(),
        bones: stand_in
            .nodes
            .iter()
            .map(|(name, parent, _)| MapBone { name: name.to_string(), parent: *parent, position: Vec3::ZERO, rotation: Quat::IDENTITY })
            .collect(),
        root: Transform::IDENTITY,
        animations: Some(Arc::new(AnimSet::default())),
        boxes: Vec::new(),
        ragdoll: None,
    };
    let (from, to) = (1, 2);
    let a = male_stand_in(true);
    // Another tree: one node more.
    let mut b = male_stand_in(true);
    b.nodes.push(("Extra_Node", Some(0), [0.0, 1.0, 0.0]));
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, chimera_world::map::MapSimPlugin::empty()));
    sim.set_tick_interval(TICK_INTERVAL);
    sim.app.init_resource::<LoadOnDemand>();
    let world = sim.app.world_mut();
    models::register(world, &builtin().model(from).unwrap().file, Arc::new(a.rig(STAND_IN_ANIMS)));
    models::late_in(world).0.register_group(BodyGroup { records: vec![from], ..default() });
    world.insert_resource(CharacterModels(Arc::new(vec![model_of(from, &a)])));
    let p = player(&mut sim, Vec3::new(0.0, 1.0, 12.0), 0.0);
    sim.app.world_mut().entity_mut(p).insert(Selection { model: from, ..default() });
    sim.ticks(6);
    // The drawing's pose goes with the model the character wears, always.
    let check = |sim: &Sim, what: &str| {
        let w = sim.app.world();
        let animator = w.get::<Animator>(p).unwrap();
        let worn = &w.resource::<CharacterModels>().0[w.get::<BodyModel>(p).unwrap().0];
        assert_eq!(animator.skeleton, Some(worn.skeleton_signature()), "{what}: the pose is the worn model's skeleton's");
        if let Some(pose) = &animator.fixed {
            assert_eq!(pose.len(), worn.bones.len(), "{what}: one pose per bone of the worn model");
        }
        worn.name.clone().unwrap()
    };
    let name = |sim: &Sim| sim.app.world().get::<BodyName>(p).map(|n| n.0.clone());
    assert_eq!(check(&sim, "before"), models::body_name(from));
    assert_eq!(name(&sim).as_deref(), Some(models::body_name(from).as_str()));
    // The change: its rig is known (stood in, or read), its model loading.
    let world = sim.app.world_mut();
    models::register(world, &builtin().model(to).unwrap().file, Arc::new(b.rig(STAND_IN_ANIMS)));
    world.entity_mut(p).insert(Selection { model: to, ..default() });
    for _ in 0..30 {
        sim.ticks(1);
        assert_eq!(check(&sim, "while it loads"), models::body_name(from));
    }
    let world = sim.app.world();
    assert!(world.get::<CaShownAnim>(p).is_some(), "the old body plays on");
    assert_eq!(world.get::<CaAnim>(p).unwrap().body, to, "the new body is the logic's at once");
    assert!(world.get::<Animator>(p).unwrap().fixed.is_some());
    // It moves: the old skeleton's driver runs while it shows.
    sim.intent(p).move_axis = Vec2::Y;
    sim.ticks(10);
    let shown = sim.app.world().get::<CaShownAnim>(p).unwrap();
    assert_eq!(shown.0.driver.name_of(&shown.0.driver.lower), "LRF", "the old body runs");
    // The model comes.
    let world = sim.app.world_mut();
    models::late_in(world).0.register_group(BodyGroup { records: vec![to], ..default() });
    let mut all = (*world.resource::<CharacterModels>().0).clone();
    all.push(model_of(to, &b));
    world.resource_mut::<CharacterModels>().0 = Arc::new(all);
    let mut swapped = false;
    for _ in 0..8 {
        sim.ticks(1);
        swapped = check(&sim, "the swap") == models::body_name(to);
        if swapped {
            break;
        }
    }
    assert!(swapped, "the new body comes");
    sim.ticks(2);
    assert!(sim.app.world().get::<CaShownAnim>(p).is_none(), "the old body's driver goes with it");
    assert_eq!(sim.app.world().get::<Animator>(p).unwrap().fixed.as_ref().unwrap().len(), b.nodes.len());
    // Back to a body whose model is here, from a character whose state was
    // never on it (it arrives from another game's body): that body's own
    // rig poses it from the start.
    let world = sim.app.world_mut();
    let mut only_a = (*world.resource::<CharacterModels>().0).clone();
    only_a.pop();
    world.resource_mut::<CharacterModels>().0 = Arc::new(only_a);
    world.entity_mut(p).insert(chimera_world::map::BodyName(models::body_name(from)));
    sim.ticks(4);
    assert_eq!(check(&sim, "the rig kept for what it wears"), models::body_name(from));
}

// ---------------------------------------------------------------------
// Network: other players' animation state comes from the server.

/// Client 1 runs forward; client 0 sees its trackers (`CaAnimNet`) and
/// plays them: the legs' `LRF`, the torso's `URFRAmF`.
#[test]
fn net_others_follow_the_server() {
    use chimera_net::{harness::NetSim, net::memory::LinkConditions};
    use combat_arms::game::characters::CaAnimNet;
    let link = LinkConditions {
        latency: std::time::Duration::from_millis(30),
        jitter: std::time::Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(CaSim, link, 7, 2, |app: &mut App| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    sim.ticks(240);
    let me = sim.local_player(0).unwrap();
    let other = sim.local_player(1).unwrap();
    let them = |sim: &mut NetSim| {
        let world = sim.clients[0].app.world_mut();
        let mut q = world.query::<(Entity, &CaAnimNet, &CaAnim)>();
        q.iter(world)
            .filter(|(e, ..)| *e != me)
            .map(|(_, n, a)| (n.lower.0.clone(), a.playing().1.to_string(), a.playing().0.to_string()))
            .next()
            .expect("the other player's state")
    };
    assert_eq!(them(&mut sim), ("LST".into(), "LST".into(), "URFRAmS".into()));
    sim.clients[1].app.world_mut().get_mut::<chimera_core::core::Intent>(other).unwrap().move_axis = Vec2::Y;
    sim.ticks(40);
    assert_eq!(them(&mut sim), ("LRF".into(), "LRF".into(), "URFRAmF".into()));
}

/// Looks over the network (characters.md 1, "The selection"): each
/// client sends its own look (`CaOwnLook`); the server validates it and
/// sends every player's to every client, so two clients with different
/// looks see each other as chosen, a change follows, and a bot's drawn
/// look reaches them too. An unknown face becomes none; an unknown body
/// is refused.
#[test]
fn net_looks_reach_every_client() {
    use chimera_net::{
        harness::NetSim,
        net::{NetCharacter, memory::LinkConditions},
    };
    use combat_arms::game::{bots, characters::CaOwnLook};
    let link = LinkConditions {
        latency: std::time::Duration::from_millis(30),
        jitter: std::time::Duration::ZERO,
        loss: 0.0,
    };
    let a = Selection {
        model: 2,
        face: 1,
        head: 2,
        camouflage: 3,
        goggle: 0,
        ..default()
    };
    let b = Selection {
        model: 1,
        face: 6,
        head: 0,
        camouflage: 5,
        ..default()
    };
    let mut sim = NetSim::new(CaSim, link, 7, 2, |app: &mut App| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.clients[0].app.world_mut().insert_resource(CaOwnLook(a));
    sim.clients[1].app.world_mut().insert_resource(CaOwnLook(b));
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    let bot = bots::add_bot(sim.server.app.world_mut(), None).expect("a bot");
    let bot_look = sim.server.app.world().get::<Selection>(bot).copied();
    sim.ticks(60);
    let bot_look = sim.server.app.world().get::<Selection>(bot).copied().or(bot_look).expect("the bot's look");
    let ids = [sim.client_id(0), sim.client_id(1)];
    // What client `i` sees of the player owned by `owner` (None: the bot).
    let seen = |sim: &mut NetSim, i: usize, owner: Option<u64>| -> Option<Selection> {
        let world = sim.clients[i].app.world_mut();
        let mut q = world.query::<(&NetCharacter, &Selection)>();
        q.iter(world).find(|(c, _)| c.owner == owner).map(|(_, s)| *s)
    };
    for i in 0..2 {
        assert_eq!(seen(&mut sim, i, Some(ids[0])), Some(a), "client {i} sees client 0");
        assert_eq!(seen(&mut sim, i, Some(ids[1])), Some(b), "client {i} sees client 1");
        assert_eq!(seen(&mut sim, i, None), Some(bot_look), "client {i} sees the bot");
    }
    // The body each client poses for the other: the female twin of the
    // Light body (face 6), the male Heavy (face 1).
    let other = sim.character_of(1).unwrap();
    assert_eq!(sim.server.app.world().get::<CaAnim>(other).unwrap().body, 8);
    {
        let world = sim.clients[0].app.world_mut();
        let mut q = world.query::<(&NetCharacter, &CaAnim)>();
        let (_, anim) = q.iter(world).find(|(c, _)| c.owner == Some(ids[1])).unwrap();
        assert_eq!((anim.body, anim.skeleton), (8, 1));
    }
    // A change on client 1 follows; an unknown face becomes none.
    sim.clients[1].app.world_mut().resource_mut::<CaOwnLook>().0 = Selection { face: 9999, ..b };
    sim.ticks(30);
    assert_eq!(seen(&mut sim, 0, Some(ids[1])), Some(Selection { face: -1, ..b }));
    // An unknown body is refused: the last look stays.
    sim.clients[1].app.world_mut().resource_mut::<CaOwnLook>().0 = Selection { model: 999, ..b };
    sim.ticks(30);
    assert_eq!(seen(&mut sim, 0, Some(ids[1])), Some(Selection { face: -1, ..b }));
}

/// Looks in play (characters.md 1, ours for bots): a bot wears the look
/// its name draws from the loadout pool, and keeps it; the local player
/// wears `CaOwnLook` (the console's `ca_model`, `ca_face`, `ca_head`,
/// `ca_camo`), changed at once; another player keeps the default.
#[test]
fn looks_in_play() {
    use chimera_core::core::{Bot, LocalPlayer};
    use combat_arms::game::characters::CaOwnLook;
    let mut sim = sim();
    let bot = sim.spawn_character(Vec3::new(0.0, 1.0, 10.0), movement::ID);
    let me = sim.spawn_character(Vec3::new(2.0, 1.0, 10.0), movement::ID);
    let other = sim.spawn_character(Vec3::new(4.0, 1.0, 10.0), movement::ID);
    sim.app.world_mut().entity_mut(bot).insert((Bot, Name::new("Bot Kilo")));
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    let mine = Selection {
        model: 2,
        face: 6,
        head: 2,
        camouflage: 3,
        ..default()
    };
    sim.app.world_mut().insert_resource(CaOwnLook(mine));
    sim.seconds(0.2);
    let look = |sim: &Sim, e: Entity| *sim.app.world().get::<Selection>(e).unwrap();
    let drawn = look(&sim, bot);
    let seed = "Bot Kilo".bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3));
    assert_eq!(drawn, builtin().bot_look(seed));
    assert_eq!(look(&sim, me), mine);
    assert_eq!(look(&sim, other), Selection::default());
    // The female face: the female body, its skeleton and hit table.
    assert_eq!(sim.app.world().get::<CaAnim>(me).unwrap().skeleton, 1);
    sim.app.world_mut().resource_mut::<CaOwnLook>().0.face = 1;
    sim.seconds(0.2);
    assert_eq!(look(&sim, me).face, 1);
    assert_eq!(sim.app.world().get::<CaAnim>(me).unwrap().skeleton, 0);
    sim.seconds(1.0);
    assert_eq!(look(&sim, bot), drawn, "a bot keeps its look");
}

// ---------------------------------------------------------------------
// Follow-up cases (Open questions 2, 5, 9, 13-22: cases 43-57).

/// 43. The female `Bip01 Neck` and right-hand socket, `LST` t = 0 (data).
#[test]
fn t43_female_neck_and_right_hand() {
    let s = builtin().skeleton(1);
    let neck = s.nodes.iter().find(|n| n.name == "Bip01 Neck").unwrap();
    assert_eq!(neck.at, Some([1.74, 27.99, 9.61]));
    let hand = s.socket("RightHand").unwrap();
    assert_eq!((hand.parent.as_str(), hand.offset), ("Right_hand", [7.08465, 1.66108, 1.27744]));
    assert_eq!(hand.at, Some([5.69, 23.62, 22.09]));
}

/// 44. The skeleton's root at the hull's bottom for every crouch r: the
/// root (-50 + 10 r) against the half-height 50 - 10 r.
#[test]
fn t44_body_against_the_hull() {
    let rig = male_stand_in(true).rig(STAND_IN_ANIMS);
    let pose: Vec<(ltb::Vec3, ltb::Quat)> = (0..rig.model.nodes.len()).map(|_| ([0.0; 3], [0.0, 0.0, 0.0, 1.0])).collect();
    for r in [0.0, 0.5, 1.0] {
        let feet = rig.posed(&pose, r).feet;
        let half = 50.0 - 10.0 * r;
        assert!((feet + half).abs() < 1e-4, "r {r}: root {feet}, hull bottom {}", -half);
    }
}

/// 45. Defaults without lobby data: the female face draws the female
/// body, the face's hair and its own face model; male faces no head.
#[test]
fn t45_defaults() {
    let d = builtin();
    let look = Selection { face: 4, ..default() };
    let (body, skeleton) = d.effective(&look);
    assert_eq!((body.id, body.file.as_str()), (7, "CHARS_M_BODY\\Nomal_W_vest.ltb"));
    let head = d.worn_head(0, 4, skeleton);
    assert_eq!(head, 23);
    assert_eq!(d.head_file(head, skeleton), Some("CHARS_M_HEAD\\Hair_W_A.ltb"));
    assert_eq!(d.face(4).and_then(|f| f.file.as_deref()), Some("CHARS_M_FACE\\Face_W_A.ltb"));
    assert_eq!((look.goggle, look.backpack), (-1, -1), "no goggle, no backpack");
    assert_eq!(d.head_file(d.worn_head(0, 0, 0), 0), None, "face 0: no head model");
    assert_eq!(d.face(0).and_then(|f| f.file.as_deref()), Some("CHARS_M_FACE\\Face_A.ltb"));
    assert_eq!(d.head_file(d.worn_head(0, 7, 1), 1), None, "face 7: none");
}

/// 46. The stock bodies' skins by camouflage index (data).
#[test]
fn t46_skins() {
    let d = builtin();
    let f8 = d.model(8).unwrap();
    assert_eq!(d.skins(f8, 0), (Some("CHARS_T_BODY\\F_Upper_Lev02_OD.dtx"), Some("CHARS_T_BODY\\F_Leg_Lev03_OD.dtx")));
    assert_eq!(d.skins(f8, 2), (Some("CHARS_T_BODY\\CW_VST_LT_SN_00_HM.DTX"), Some("CHARS_T_BODY\\CW_LG_HV_SN_00_HM.dtx")));
    let m1 = d.model(1).unwrap();
    assert_eq!(d.skins(m1, 6), (Some("CHARS_T_BODY\\CM_VST_LT_TG_00_JS.DTX"), Some("CHARS_T_BODY\\CM_LG_HV_TG_00_JS.dtx")));
}

/// 47. The digit classes' names.
#[test]
fn t47_digit_suffixes() {
    use driver::{Direction as D, Posture as P, upper_names};
    let run = |class, p, d| upper_names(class, p, d).0;
    let walk = |class, p, d| upper_names(class, p, d).1.unwrap();
    assert_eq!(run(24, P::Reload, D::None), "UPRSReS1");
    assert_eq!(run(24, P::Fire, D::None), "UPRFiS");
    assert_eq!(run(24, P::Select, D::None), "UPRSSeS");
    assert_eq!(run(26, P::Reload, D::F), "USMRReS2");
    assert_eq!(run(26, P::Fire, D::None), "USMRFiS");
    assert_eq!(run(27, P::Fire, D::None), "USGRFiS1");
    assert_eq!(run(27, P::Select, D::None), "USGRSSeS1");
    assert_eq!(walk(27, P::Fire, D::F), "USGWFiF1");
    assert_eq!(run(27, P::AltFire, D::None), run(27, P::Fire, D::None), "alt fire = fire");
    assert_eq!(run(29, P::Reload, D::None), "USGRSReS2");
    assert_eq!(run(29, P::Fire, D::F), "USGRFiF2");
}

/// 48-50. The server's reload timer.
#[test]
fn t48_50_server_reload_timer() {
    assert_eq!(driver::server_reload_timer(Some(1866), 1.0, 0), Some(1555));
    assert_eq!(driver::server_reload_timer(Some(1866), 2.5, 0), Some(777));
    assert_eq!(driver::server_reload_timer(Some(1866), 1.0, 2300), None, "49: the animation ends it");
    assert_eq!(driver::upper_rate(1866, 1.0, 2300), 1866.0 / 2300.0);
    assert_eq!(driver::server_reload_timer(None, 1.0, 0), Some(500), "50");
}

/// 48 on the server: an M4A1 reload's upper body leaves the reload pose
/// 1555 ms after it starts (the timer), before the weapon's 2300 ms.
#[test]
fn t48_server_reload_pose_ends_on_the_timer() {
    let mut sim = sim();
    let p = sim.spawn_character(Vec3::new(0.0, 1.0, 10.0), movement::ID);
    sim.seconds(1.5);
    let w = sim.app.world().get::<Inventory>(p).unwrap().active.unwrap();
    sim.app.world_mut().get_mut::<Magazine>(w).unwrap().clip = 10;
    sim.intent(p).reload = true;
    sim.ticks(2);
    sim.intent(p).reload = false;
    let posture = |sim: &Sim| sim.app.world().get::<CaAnim>(p).unwrap().driver.last.posture;
    assert_eq!(posture(&sim), Posture::Reload);
    sim.seconds(1.45);
    assert_eq!(posture(&sim), Posture::Reload, "before 1555 ms");
    sim.seconds(0.2);
    assert_ne!(posture(&sim), Posture::Reload, "after 1555 ms");
    assert!(sim.app.world().get::<weapon::WeaponState>(w).unwrap().reloading(), "the weapon still reloads");
}

/// 51. "Fire held": the server's raw flag, the own client's masked.
#[test]
fn t51_fire_held() {
    assert!(driver::fire_held(true, false, true, true, true, false), "server: raw while reloading");
    assert!(!driver::fire_held(true, false, false, true, true, false), "own client: masked");
    assert!(!driver::fire_held(true, true, true, true, false, false), "use held");
    // The server's driver keeps the reload pose while reloading with fire
    // held; when its timer lets the reload finish, the fire waits for the
    // reload animation's end (1866 ms), then fires.
    let mut d = drv();
    let i = Inputs {
        fire: true,
        reloading: true,
        ..rifle()
    };
    run(&mut d, &i, 3);
    assert_eq!(d.last.posture, Posture::Reload);
    let done = Inputs {
        reload_may_finish: true,
        ..i.clone()
    };
    run(&mut d, &done, 2);
    assert_eq!(d.last.posture, Posture::Reload, "the animation still plays");
    run(&mut d, &done, (1866 / TICK) as usize);
    assert_eq!(d.last.posture, Posture::Fire);
}

/// 52. The hit kick's offsets on ten notices, the eleventh none.
#[test]
fn t52_hit_kick() {
    let mut k = 0;
    let mut out = Vec::new();
    for _ in 0..11 {
        let (kick, next) = driver::hit_kick(k);
        out.push((kick * 10.0).round() as i32);
        k = next;
    }
    assert_eq!(out, vec![1, 2, 3, 4, 5, 4, 3, 2, 1, 0, 0]);
    assert_eq!(k, 0, "the counter restarted");
}

/// 53. An emote: the upper tracker keeps it, the legs return at once
/// (Corrections 9); pressing forward ends it.
#[test]
fn t53_emote() {
    let mut d = drv();
    run(&mut d, &rifle(), 3);
    assert!(!d.start_emote("Salute"), "the US male has no Salute");
    assert!(d.start_emote("Surrender"));
    run(&mut d, &rifle(), 2);
    assert_eq!((d.name_of(&d.upper), d.name_of(&d.lower)), ("Surrender", "LST"));
    run(&mut d, &Inputs { forward: true, ..rifle() }, 1);
    assert_eq!(d.emote, None);
    assert_ne!(d.name_of(&d.upper), "Surrender");
}

/// 54. The floor snap.
#[test]
fn t54_floor_snap() {
    use combat_arms::game::characters::snap;
    let at = Vec3::new(0.0, 300.0, 0.0);
    let p = snap(at, 200.0, 50.0, 0.1);
    assert!((p.y - 150.1).abs() < 1e-4);
    assert!((p.y + 0.1 - 150.2).abs() < 1e-4, "the body 0.1 u above");
    assert_eq!(snap(at, 40.0, 50.0, 0.1), at, "within the half-height: no move");
}

/// 55. A body neither stops shots nor has spheres.
#[test]
fn t55_corpses_dont_stop_shots() {
    let mut sim = sim();
    let p = sim.spawn_character(Vec3::new(0.0, 1.0, 10.0), movement::ID);
    sim.seconds(0.2);
    sim.app.world_mut().write_message(Damage {
        target: p,
        attacker: None,
        amount: 10.0,
        point: Vec3::ZERO,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Chest,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.seconds(0.2);
    let w = sim.app.world_mut();
    let bodies: Vec<Entity> = w.query_filtered::<Entity, With<CaBody>>().iter(w).collect();
    assert_eq!(bodies.len(), 1);
    let e = w.entity(bodies[0]);
    assert!(!e.contains::<avian3d::prelude::Collider>() && !e.contains::<combat_arms::game::weapons::HitSpheres>());
}

/// 56. Death types by damage kind.
#[test]
fn t56_death_types() {
    use combat_arms::game::volumes::ENDLESS_FALL;
    let catalog = chimera_sim::weapon::data::WeaponCatalog {
        sets: vec![combat_arms::game::data::CaData::builtin().weapon_set()],
    };
    assert_eq!(death::kind_of(&catalog, DamageKind::Generic, Some(ENDLESS_FALL)), DeathKind::EndlessFall);
    assert_eq!(death::kind_of(&catalog, DamageKind::Generic, Some("combat_arms:g7a")), DeathKind::Poison, "a gas grenade's lasting damage");
    assert_eq!(death::kind_of(&catalog, DamageKind::Bullet, Some(RIFLE)), DeathKind::Other);
    let pick = |kind| {
        death::choose(
            &DeathInputs {
                kind,
                dir: Vec3::Z,
                forward: Vec3::NEG_Z,
                right: Vec3::X,
                head: false,
                groin: false,
                coin: true,
                jumping: false,
                special: None,
            },
            |_| true,
        )
    };
    assert_eq!(pick(DeathKind::EndlessFall), "D_W");
    assert_eq!(pick(DeathKind::Poison), "D_P");
    assert_eq!(pick(DeathKind::Other), "DT");
    // Quirk 6's coin: the C library's first draw for a seed, mod 2.
    assert!(!death::coin(0), "seed 0: first draw 38");
    assert!(death::coin(1), "seed 1: first draw 41");
}

// ---------------------------------------------------------------------
// Corrections (2026-10-22): characters.md, one test each (`c01_` is
// correction 1).

fn sniper() -> Inputs {
    Inputs {
        class: 5,
        ..default()
    }
}

/// 1. A held fire button plays the fire animation once and holds its last
/// frame; a release for one update and a new press start it again; the
/// knife likewise; running forward keeps `URFRAmF`.
#[test]
fn c01_held_fire_plays_once() {
    let mut d = drv();
    run(&mut d, &rifle(), 2);
    let fire = Inputs { fire: true, ..rifle() };
    d.update(TICK, &fire);
    assert_eq!(at(&d).0, "URFRFiS");
    // 1000 ms held: no second start, the last frame held.
    let mut prev = d.upper.time;
    for _ in 0..66 {
        d.update(TICK, &fire);
        assert!(d.upper.time >= prev, "restarted at {prev}");
        prev = d.upper.time;
        assert_eq!(at(&d).0, "URFRFiS");
    }
    assert!(d.upper.done(&d.list));
    // Released for one update, pressed again: from 0.
    d.update(TICK, &rifle());
    assert_eq!(at(&d).0, "URFRAmS");
    d.update(TICK, &fire);
    assert_eq!(at(&d).0, "URFRFiS");
    assert!(d.upper.time <= TICK);
    // The knife, held 2 s.
    let mut d = drv();
    let knife = Inputs { class: 14, fire: true, ..default() };
    run(&mut d, &Inputs { class: 14, ..default() }, 2);
    d.update(TICK, &knife);
    assert_eq!(at(&d).0, "UKRFiS");
    let mut prev = d.upper.time;
    for _ in 0..133 {
        d.update(TICK, &knife);
        assert!(d.upper.time >= prev);
        prev = d.upper.time;
    }
    assert!(d.upper.done(&d.list));
    // Running forward with fire held: the looping aim.
    let mut d = drv();
    let run_fwd = Inputs { forward: true, ..rifle() };
    run(&mut d, &run_fwd, 3);
    run(&mut d, &Inputs { fire: true, ..run_fwd }, 20);
    assert_eq!(at(&d).0, "URFRAmF");
    assert!(d.upper.looping);
}

/// 2. A front-left body hit plays `DC`; with the jaw `DLSH`; the other
/// sides as before.
#[test]
fn c02_front_left_body_death_is_dc() {
    let a = Vec3::new(0.94, 0.0, -0.34);
    assert_eq!(death_of(DeathKind::Other, a, false, false, true, false), "DC");
    assert_eq!(death_of(DeathKind::Other, a, false, false, false, false), "DC");
    assert_eq!(death_of(DeathKind::Other, a, true, false, true, false), "DLSH");
    let b = Vec3::new(0.94, 0.0, 0.34);
    assert_eq!(death_of(DeathKind::Other, b, false, false, true, false), "DLT");
    assert_eq!(death_of(DeathKind::Other, b, true, false, true, false), "DLSH");
    let c = Vec3::new(-0.94, 0.0, -0.34);
    assert_eq!(death_of(DeathKind::Other, c, false, false, true, false), "DRT");
}

/// 3. Falling, on the driver: with the footing test failed the legs are
/// `LST` and the upper `URFRAmS` until the landing.
#[test]
fn c03_walking_off_a_drop_driver() {
    let mut d = drv();
    let fwd = Inputs { forward: true, ..rifle() };
    run(&mut d, &fwd, 5);
    assert_eq!((at(&d).1.as_str(), at(&d).0.as_str()), ("LRF", "URFRAmF"));
    let off = Inputs { no_footing: true, ..fwd.clone() };
    run(&mut d, &off, 3);
    assert_eq!(d.jump, Jump::Falling);
    assert_eq!((at(&d).1.as_str(), at(&d).0.as_str()), ("LST", "URFRAmS"));
    // Still falling once a floor is near again (the status stays) ...
    run(&mut d, &fwd, 3);
    assert_eq!(at(&d).1, "LST");
    // ... until the landing.
    run(&mut d, &Inputs { grounded: true, ..fwd.clone() }, 2);
    assert_eq!((at(&d).1.as_str(), at(&d).0.as_str()), ("LRF", "URFRAmF"));
}

/// 3. On the greybox: a run off a 300 u drop shows standing legs in the
/// air; an 80 u step (below the feet) keeps the run legs.
#[test]
fn c03_walking_off_a_drop_in_play() {
    let air = |drop: f32| {
        let mut sim = sim();
        let p = player(&mut sim, Vec3::new(0.0, 1.0, 12.0), 0.0);
        weapon::give(sim.app.world_mut(), p, RIFLE).unwrap();
        sim.seconds(1.3);
        sim.intent(p).move_axis = Vec2::Y;
        sim.seconds(0.4);
        assert_eq!(playing(&sim, p).1, "LRF");
        // The hull's centre `drop` u (below the feet) above where it stood.
        let w = sim.app.world_mut();
        let stood = w.get::<Transform>(p).unwrap().translation.y;
        w.get_mut::<Transform>(p).unwrap().translation.y = stood + drop * UNIT;
        w.get_mut::<chimera_core::core::Velocity>(p).unwrap().0.y = 0.0;
        w.get_mut::<movement::CaMovement>(p).unwrap().standing = None;
        sim.ticks(2);
        (sim, p)
    };
    let (sim, p) = air(300.0);
    assert!(!sim.state(p).on_ground);
    assert_eq!((playing(&sim, p).1.as_str(), playing(&sim, p).0.as_str()), ("LST", "URFRAmS"));
    let (mut sim, p) = air(300.0);
    sim.seconds(3.0);
    assert!(sim.state(p).on_ground, "landed");
    assert_eq!((playing(&sim, p).1.as_str(), playing(&sim, p).0.as_str()), ("LRF", "URFRAmF"));
    let (sim, p) = air(80.0);
    assert!(!sim.state(p).on_ground);
    assert_eq!(playing(&sim, p).1, "LRF");
}

/// 4. A bolt sniper's fire pose follows the accepted shots, not the
/// trigger.
#[test]
fn c04_sniper_fire_pose_follows_shots() {
    let mut d = drv();
    run(&mut d, &sniper(), 2);
    let held = Inputs { fire: true, ..sniper() };
    let shot = Inputs { shot: true, ..held.clone() };
    let mut starts = Vec::new();
    for k in 0..200 {
        // 3 s of 15 ms updates, shots at 0 and 1.5 s.
        let i = if k == 0 || k == 100 { &shot } else { &held };
        let before = (at(&d).0, d.upper.time);
        d.update(TICK, i);
        if at(&d).0.contains("Fi") && (before.0 != at(&d).0 || d.upper.time < before.1) {
            starts.push(k);
        }
        if k == 50 {
            assert_eq!(at(&d).0, "USNRAmS", "aim in between");
        }
    }
    assert_eq!(starts, vec![0, 100], "{starts:?}");
    // The button alone never fires.
    let mut d = drv();
    run(&mut d, &held, 20);
    assert_eq!(at(&d).0, "USNRAmS");
}

/// 5. No fire pose while the legs play the sprint.
#[test]
fn c05_no_fire_pose_while_sprinting() {
    let mut d = drv();
    let sprint = Inputs { sprint: true, forward: true, ..rifle() };
    run(&mut d, &sprint, 3);
    let held = Inputs { fire: true, ..sprint.clone() };
    run(&mut d, &held, 10);
    assert_eq!(d.last.posture, Posture::Aim);
    assert_eq!(at(&d).0, "URFRRAmF");
    assert!(d.upper.looping);
    // Alt fire too (the knife).
    let mut d = drv();
    let knife = Inputs { class: 14, sprint: true, forward: true, ..default() };
    run(&mut d, &knife, 3);
    run(&mut d, &Inputs { alt_fire: true, ..knife }, 5);
    assert_eq!(d.last.posture, Posture::Aim);
}

/// 6. The "ammo 0" is the total.
#[test]
fn c06_ammo_zero_is_the_total() {
    assert!(!driver::ammo_exhausted(0, 90));
    assert!(!driver::ammo_exhausted(5, 0));
    assert!(driver::ammo_exhausted(0, 0));
    let mut d = drv();
    run(&mut d, &rifle(), 2);
    let fire = Inputs { fire: true, empty: driver::ammo_exhausted(0, 90), ..rifle() };
    run(&mut d, &fire, 20);
    assert_eq!(d.last.posture, Posture::Fire, "clip 0, 90 in reserve: still fire");
    let none = Inputs { fire: true, empty: driver::ammo_exhausted(0, 0), ..rifle() };
    run(&mut d, &none, 1);
    assert_eq!(d.last.posture, Posture::Aim, "total 0, the animation done: aim");
}

/// 7. Crouching in the air clears the jump and fall status for good.
#[test]
fn c07_crouch_in_the_air_clears_the_jump() {
    let mut d = drv();
    let fwd = Inputs { forward: true, ..rifle() };
    run(&mut d, &fwd, 3);
    d.update(TICK, &Inputs { jumped: true, ..fwd.clone() });
    assert_eq!(at(&d).1, "LJJ");
    run(&mut d, &fwd, 12); // 200 ms
    assert_eq!(at(&d).1, "LJJ");
    let crouch = Inputs { crouch_key: true, crouch: 0.5, ..fwd.clone() };
    run(&mut d, &crouch, 13); // to 400 ms
    assert_eq!(d.jump, Jump::None);
    assert_eq!(at(&d).1, "LRF");
    assert_eq!(at(&d).2, "LCF");
    // Released in the air: the jump legs do not come back.
    run(&mut d, &fwd, 30);
    assert_eq!(at(&d).1, "LRF");
    // A drop (item 3) then crouch: locomotion at once.
    let mut d = drv();
    run(&mut d, &fwd, 3);
    run(&mut d, &Inputs { no_footing: true, ..fwd.clone() }, 2);
    assert_eq!(at(&d).1, "LST");
    d.update(TICK, &Inputs { no_footing: true, crouch_key: true, ..fwd.clone() });
    assert_eq!(at(&d).1, "LRF");
}

/// 8. Forward and back held together give backward; with a strafe the
/// forward key is tested first.
#[test]
fn c08_forward_and_back_give_backward() {
    use driver::Direction as D;
    assert_eq!(D::of(true, true, false, false), D::B);
    assert_eq!(D::of(true, true, false, true), D::FR);
    assert_eq!(D::of(true, true, true, false), D::FL);
    assert_eq!(D::of(false, false, true, true), D::L);
    let mut d = drv();
    run(&mut d, &Inputs { forward: true, back: true, ..rifle() }, 3);
    assert_eq!((at(&d).1.as_str(), at(&d).0.as_str()), ("LRB", "URFRAmB"));
    let mut d = drv();
    run(&mut d, &Inputs { forward: true, back: true, right: true, ..rifle() }, 3);
    assert_eq!(at(&d).1, "LRFR");
    let mut d = drv();
    run(&mut d, &Inputs { forward: true, back: true, left: true, ..rifle() }, 3);
    assert_eq!(at(&d).1, "LRFL");
}

/// 9. An emote is on the upper body only: the legs are back within one
/// update; the upper keeps the emote, and its last pose after it ends,
/// until the player moves, fires, reloads or switches.
#[test]
fn c09_emote_is_upper_body_only() {
    let mut d = drv();
    run(&mut d, &rifle(), 3);
    assert!(d.start_emote("Insult_Dance"));
    assert_eq!(d.name_of(&d.lower), "Insult_Dance");
    d.update(TICK, &rifle());
    assert_eq!((d.name_of(&d.upper), d.name_of(&d.lower)), ("Insult_Dance", "LST"));
    assert!(d.emote.is_some(), "the weapon stays hidden while it plays");
    // Until it ends, then its final frame stays.
    let len = d.list.entries[d.upper.current].length();
    run(&mut d, &rifle(), (len / TICK) as usize + 10);
    assert!(d.emote.is_none());
    assert_eq!(d.name_of(&d.upper), "Insult_Dance");
    run(&mut d, &rifle(), 50);
    assert_eq!(d.name_of(&d.upper), "Insult_Dance", "held until something changes");
    // Moving starts the aim again.
    d.update(TICK, &Inputs { forward: true, ..rifle() });
    assert_eq!(d.name_of(&d.upper), "URFRAmF");
    // Firing too.
    let mut d = drv();
    run(&mut d, &rifle(), 3);
    d.start_emote("Surrender");
    run(&mut d, &rifle(), 3);
    d.update(TICK, &Inputs { fire: true, ..rifle() });
    assert_eq!(d.name_of(&d.upper), "URFRFiS");
}

/// 10. "Died in a jump" is the jump legs' first ~667 ms and never turns
/// the body for a blast.
#[test]
fn c10_died_in_a_jump() {
    let body_after = |wait: f64, kind: DamageKind| {
        let mut sim = sim();
        let p = player(&mut sim, Vec3::new(0.0, 1.0, 12.0), 0.0);
        weapon::give(sim.app.world_mut(), p, RIFLE).unwrap();
        sim.seconds(1.3);
        sim.intent(p).jump = true;
        sim.ticks(1);
        sim.intent(p).jump = false;
        sim.seconds(wait);
        kill(&mut sim, p, Vec3::new(1.0, 0.0, 0.5), Hitgroup::Chest, kind);
        sim.ticks(2);
        let world = sim.app.world_mut();
        let mut q = world.query::<&CaBody>();
        q.single(world).expect("a body").clone()
    };
    let turned = |b: &CaBody| b.yaw.abs() > 0.1;
    let b = body_after(0.3, DamageKind::Bullet);
    assert!(turned(&b), "turned away from the shot: {}", b.yaw);
    let b = body_after(0.8, DamageKind::Bullet);
    assert!(!turned(&b), "falling: not turned: {}", b.yaw);
    let b = body_after(0.3, DamageKind::Blast);
    assert!(!turned(&b), "a blast never turns the body: {}", b.yaw);
}

/// 11. No team mark in Quarantine.
#[test]
fn c11_no_team_mark_in_quarantine() {
    assert!(models::shows_mark(true, false));
    assert!(!models::shows_mark(true, true));
    assert!(!models::shows_mark(false, false));
    let d = builtin();
    let family = |teammate, quarantine| models::family(3, models::shows_mark(teammate, quarantine));
    assert_eq!(family(true, false), 7);
    assert_eq!(family(true, true), family(false, false), "a teammate in Quarantine looks like an enemy");
    assert_eq!(d.team_mark(models::shows_mark(true, true), false), "CHARS_T_BODY\\Alpha.dtx");
    assert_eq!(d.team_mark(models::shows_mark(true, false), false), "CHARS_T_BODY\\RED_Tie.dtx");
}

/// 12. The body outlives its owner's respawn by 5 s (Team Deathmatch).
#[test]
fn c12_corpse_outlives_the_respawn() {
    use chimera_core::core::{Dead, Health, SimClock, Team};
    let mut sim = Sim::new(CaSim, GreyboxMapPlugin);
    sim.set_tick_interval(TICK_INTERVAL);
    combat_arms::harness::with_mode(sim.app.world_mut(), "TDM");
    let a = sim.spawn_character(Vec3::new(0.0, 1.0, 12.0), movement::ID);
    let b = sim.spawn_character(Vec3::new(0.0, 1.0, -20.0), movement::ID);
    sim.app.world_mut().entity_mut(a).insert(Team(1));
    sim.app.world_mut().entity_mut(b).insert(Team(2));
    sim.seconds(4.0);
    for p in [a, b] {
        combat_arms::game::rules::spawns::unprotect(sim.app.world_mut(), p);
    }
    sim.app.world_mut().write_message(Damage {
        target: a,
        attacker: Some(b),
        amount: 500.0,
        point: Vec3::ZERO,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Chest,
        kind: DamageKind::Bullet,
        weapon: Some(RIFLE),
        force: Vec3::ZERO,
    });
    sim.ticks(2);
    let bodies = |sim: &mut Sim| {
        let w = sim.app.world_mut();
        w.query::<&CaBody>().iter(w).filter(|b| b.owner == a).cloned().collect::<Vec<_>>()
    };
    let died_at = bodies(&mut sim)[0].since;
    // Until the owner respawns.
    let alive = |sim: &Sim| {
        let w = sim.app.world();
        w.get::<Dead>(a).is_none() && w.get::<Health>(a).is_some_and(|h| h.current > 0.0)
    };
    let mut guard = 0;
    while !alive(&sim) {
        sim.ticks(1);
        guard += 1;
        assert!(guard < 1500, "never respawned");
    }
    sim.ticks(2);
    let now = sim.app.world().resource::<SimClock>().now;
    let respawned = bodies(&mut sim)[0].respawned.expect("the respawn is noted");
    assert!(respawned > died_at + 1.0 && respawned <= now + 1e-6);
    // Still there 4 s after the respawn ...
    sim.seconds(4.0);
    assert_eq!(bodies(&mut sim).len(), 1, "the body outlives the respawn");
    // ... and gone 5 s after it.
    sim.seconds(1.5);
    assert!(bodies(&mut sim).is_empty(), "removed 5 s after the respawn");
}

/// 13. A leg set without the crouch name crouches on the legs' own
/// animation.
#[test]
fn c13_missing_crouch_name_falls_back_to_the_legs() {
    let mut list = (*driver::spec_library(false)).clone();
    list.entries.retain(|e| e.name() != "LCFR");
    let mut d = Driver::new(Arc::new(list));
    run(&mut d, &Inputs { forward: true, right: true, crouch: 0.5, ..rifle() }, 3);
    assert_eq!(at(&d).1, "LRFR");
    assert_eq!(at(&d).2, "LRFR", "the crouch tracker plays the lower name");
    assert_eq!(d.crouch.multiplier, 0.5);
    let mut d = Driver::new(driver::spec_library(false));
    run(&mut d, &Inputs { forward: true, right: true, ..rifle() }, 3);
    assert_eq!(at(&d).2, "LCFR");
}

// ---------------------------------------------------------------------------
// Corrections (2026-10-30): additional skins, model colour, cut-outs.

/// 58. A body record with `AdditionalSkins0` a and `AdditionalSkins1` b
/// at camouflage index 3: slot 5 a, slot 6 b (not affected by c).
#[test]
fn t58_additional_skins() {
    let data = builtin();
    let mut r = data.model.first().unwrap().clone();
    r.additional_skins = vec!["a.dtx".into(), "b.dtx".into()];
    assert_eq!(data.additional_skin(&r, 5), Some("a.dtx"));
    assert_eq!(data.additional_skin(&r, 6), Some("b.dtx"));
    assert_eq!(data.additional_skin(&r, 7), None);
}

/// 59. Model colour m = 1 with `ColorSkin1_UpperSkin0..` and no
/// `ColorSkin1_LowerSkin#`: slot 0 from the colour list (index c), slot 1
/// from `LowerSkin{c}`; m = -1: the base lists.
#[test]
fn t59_model_colour() {
    use combat_arms::game::data::attributes::ColourSkins;
    let data = builtin();
    let mut r = data.model.first().unwrap().clone();
    r.upper_skins = vec!["u0.dtx".into(), "u1.dtx".into()];
    r.lower_skins = vec!["l0.dtx".into(), "l1.dtx".into()];
    r.colour_skins = vec![
        ColourSkins::default(),
        ColourSkins {
            upper: vec!["red0.dtx".into(), "red1.dtx".into()],
            ..default()
        },
    ];
    assert_eq!(data.skins_in(&r, 1, 1), (Some("red1.dtx"), Some("l1.dtx")));
    assert_eq!(data.skins_in(&r, 1, -1), (Some("u1.dtx"), Some("l1.dtx")));
    assert_eq!(data.skins_in(&r, 1, 0), (Some("u1.dtx"), Some("l1.dtx")), "an empty colour: the base");
    assert_eq!(data.skins(&r, 1), (Some("u1.dtx"), Some("l1.dtx")), "ours: no model colour");
}

/// 60. The ghillie leaves (`RS\MARKTRANSPARENT.LTB`): alpha 158/255 isn't
/// drawn, 161/255 is, opaque, at the texture's colour (unlit).
#[test]
fn t60_leaves_cut_out() {
    use chimera_world::map::{MapAlpha, MapMesh};
    use combat_arms::game::render_style;
    let leaves = render_style::style("RS\\MARKTRANSPARENT.LTB");
    assert!(!leaves.keeps(158) && leaves.keeps(161));
    let mut m = MapMesh::default();
    render_style::apply(&mut m, leaves);
    assert!(matches!(m.alpha, MapAlpha::Mask(_)), "not blended: {:?}", m.alpha);
    assert!(m.unlit);
}

/// What older ladders loaded is forgotten as a map unloads, but for the
/// ladder just played and what's worn or held; a session's memory and each
/// map's building stop growing (docs/performance.md, "A session's frame
/// rate"), and what's forgotten may load again.
#[test]
fn a_map_unload_forgets_what_older_ladders_loaded() {
    use combat_arms::game::{
        characters::{
            models::body_name,
            preload::{Forgotten, LadderModels, forget_old},
        },
        view_model::on_demand::LoadOnDemand,
    };
    use chimera_world::map;

    let mut app = App::new();
    let w = app.world_mut();
    w.insert_resource(LoadOnDemand);
    w.insert_resource(LadderModels {
        bodies: vec![7],
        keys: vec!["combat_arms:kept".into()],
    });
    let body = |id: i32| {
        (
            Arc::new(map::MapCharacterModel {
                game: Some(combat_arms::game::GAME),
                team: None,
                name: Some(body_name(id)),
                class: None,
                model: Default::default(),
                hitboxes: Vec::new(),
                bones: Vec::new(),
                root: Transform::IDENTITY,
                animations: None,
                boxes: Vec::new(),
                ragdoll: None,
            }),
            Arc::new(Vec::new()),
        )
    };
    let held = |key: &str| {
        (
            Arc::new(map::MapHeldModel {
                key: key.into(),
                model: Default::default(),
                bone: String::new(),
                muzzle: None,
                attachments: Vec::new(),
                merged: Vec::new(),
                own: None,
            }),
            Arc::new(Vec::new()),
        )
    };
    w.insert_resource(map::LateCharacterModels(vec![body(7), body(8)]));
    w.insert_resource(map::LateHeldModels(vec![held("combat_arms:kept"), held("combat_arms:old"), held("combat_arms:in_hand")]));
    w.spawn(map::Held(Some("combat_arms:in_hand".into())));

    forget_old(w);

    let bodies: Vec<_> = w.resource::<map::LateCharacterModels>().0.iter().map(|(m, _)| m.name.clone().unwrap()).collect();
    assert_eq!(bodies, [body_name(7)]);
    let keys: Vec<_> = w.resource::<map::LateHeldModels>().0.iter().map(|(m, _)| m.key.clone()).collect();
    assert_eq!(keys, ["combat_arms:kept", "combat_arms:in_hand"]);
    assert_eq!(
        *w.resource::<Forgotten>(),
        Forgotten {
            bodies: vec![8],
            keys: vec!["combat_arms:old".into()],
        },
        "the loaders may load them again"
    );
}

// ---------------------------------------------------------------------
// Poses on demand (docs/performance.md, "Poses on demand")

/// One tick of what the pose decides: per character its hit spheres (the
/// client's view and the server's), sockets, legs' yaw, the hitboxes
/// other games' guns test, the drawn pose (a body with a model) and its
/// health.
type PoseTick = Vec<(Vec<Option<Vec3>>, Vec<Option<Vec3>>, [Option<Vec3>; 6], f32, Option<Vec<chimera_core::core::Hitbox>>, Option<Vec<(Quat, Vec3)>>, f32)>;

/// Two players with a model each (a stand-in), a duel of 300 ticks: they
/// walk, strafe, crouch, look up and down and fire the rifle at each other
/// (the second as a bot: its rounds trace the server's pose), the drawing's
/// models in (`Animator::fixed`). `every_tick`: `CaPoseEveryTick`, the
/// poses made in the tick as before. Each tick's health, and every
/// `sample`-th tick the poses read (the others left to the shots' demand).
fn duel_poses(every_tick: bool, sample: usize) -> (Vec<PoseTick>, usize) {
    use chimera_world::map::{CharacterModels, MapBone, MapCharacterModel, anim::{AnimSet, Animator}};
    use combat_arms::game::{GAME, characters::{CaPoseEveryTick, models::BodyGroup}};
    let stand_in = male_stand_in(true);
    let id = 1;
    let model = MapCharacterModel {
        game: Some(GAME),
        team: None,
        name: Some(models::body_name(id)),
        class: None,
        model: Default::default(),
        hitboxes: Vec::new(),
        bones: stand_in
            .nodes
            .iter()
            .map(|(name, parent, _)| MapBone { name: name.to_string(), parent: *parent, position: Vec3::ZERO, rotation: Quat::IDENTITY })
            .collect(),
        root: Transform::IDENTITY,
        animations: Some(Arc::new(AnimSet::default())),
        boxes: Vec::new(),
        ragdoll: None,
    };
    let mut sim = sim();
    if every_tick {
        sim.app.insert_resource(CaPoseEveryTick);
    }
    let world = sim.app.world_mut();
    models::register(world, &builtin().model(id).unwrap().file, Arc::new(stand_in.rig(STAND_IN_ANIMS)));
    models::late_in(world).0.register_group(BodyGroup { records: vec![id], ..default() });
    world.insert_resource(CharacterModels(Arc::new(vec![model])));
    let a = player(&mut sim, Vec3::new(0.0, 1.0, 12.0), std::f32::consts::PI);
    let b = player(&mut sim, Vec3::new(0.5, 1.0, 4.0), 0.0);
    for p in [a, b] {
        sim.app.world_mut().entity_mut(p).insert(Selection { model: id, ..default() });
        weapon::give(sim.app.world_mut(), p, RIFLE).unwrap();
    }
    sim.app.world_mut().entity_mut(b).insert(chimera_core::core::Bot);
    sim.seconds(1.3);
    let mut out = Vec::new();
    let mut unposed = 0;
    for i in 0..300usize {
        let phase = i as f32 * 0.05;
        for (k, p) in [a, b].into_iter().enumerate() {
            let mut intent = sim.intent(p);
            intent.pitch = 0.3 * (phase + k as f32).sin();
            intent.move_axis = Vec2::new((phase * 0.7).sin(), (phase * 0.3 + k as f32).cos()) * 0.6;
            intent.crouch = (i / 40 + k) % 3 == 0;
            intent.fire = (i + 7 * k) % 12 < 2;
            if i % 25 == 0 {
                intent.yaw += 0.05;
            }
        }
        sim.ticks(1);
        let w = sim.app.world();
        unposed += [a, b].iter().filter(|p| !w.get::<CaPose>(**p).unwrap().is_posed()).count();
        let tick = [a, b]
            .iter()
            .map(|&p| {
                let health = w.get::<chimera_core::core::Health>(p).unwrap().current;
                if i % sample != 0 {
                    return (Vec::new(), Vec::new(), [None; 6], 0.0, None, None, health);
                }
                let pose = w.get::<CaPose>(p).unwrap();
                let sockets = [pose.name_tag, pose.left_foot, pose.right_hand, pose.eyes[0], pose.flash[0], pose.flash_point()];
                (
                    pose.spheres.clone(),
                    pose.server_spheres().to_vec(),
                    sockets,
                    pose.body_yaw,
                    chimera_core::core::hitboxes_in(w, p).map(|h| h.0.clone()),
                    w.get::<Animator>(p).and_then(|a| a.fixed.as_ref()).map(|f| f.to_vec()),
                    health,
                )
            })
            .collect();
        out.push(tick);
    }
    (out, unposed)
}

/// The poses made only when something reads them give what making them
/// every tick gave, bit for bit: the hit spheres, the server's, the
/// sockets, the hitboxes, the drawn pose and every round's damage, over a
/// duel read every tick and one read every seventh tick (the shots'
/// demand alone in between).
#[test]
fn poses_on_demand_match_poses_every_tick() {
    let (eager, made_unposed) = duel_poses(true, 1);
    assert_eq!(made_unposed, 0, "CaPoseEveryTick makes every pose in the tick");
    for sample in [1, 7] {
        let (lazy, unposed) = duel_poses(false, sample);
        let hurt = lazy.last().unwrap().iter().any(|c| c.6 < 100.0);
        assert!(hurt, "the duel draws blood");
        for (i, (e, l)) in eager.iter().zip(&lazy).enumerate() {
            if i % sample == 0 {
                assert!(e == l, "tick {i} (read every {sample}): {e:?}\n vs {l:?}");
            } else {
                let health = |t: &PoseTick| t.iter().map(|c| c.6).collect::<Vec<_>>();
                assert_eq!(health(e), health(l), "tick {i}: the same damage");
            }
        }
        if sample > 1 {
            assert!(unposed > 0, "some ticks nobody read a pose: none was made");
        }
    }
}

/// The box a body's hitboxes stay in (`core::HitBound`, the broad phase's:
/// docs/performance.md, "A broad phase for the hitscan") holds every
/// sphere, tick after tick of a duel (walking, crouching, looking up and
/// down, firing, taking hits), for a body with a model (`CaPose::
/// sphere_reach`) and one on the spec's skeleton (`FallbackReach`); and
/// it is a body's size, not the map's (under 4 m on a side; printed).
#[test]
fn the_hit_bound_holds_every_sphere() {
    use chimera_world::map::{CharacterModels, MapBone, MapCharacterModel, anim::AnimSet};
    use combat_arms::game::{GAME, characters::models::BodyGroup};
    let stand_in = male_stand_in(true);
    let id = 1;
    let model = MapCharacterModel {
        game: Some(GAME),
        team: None,
        name: Some(models::body_name(id)),
        class: None,
        model: Default::default(),
        hitboxes: Vec::new(),
        bones: stand_in
            .nodes
            .iter()
            .map(|(name, parent, _)| MapBone { name: name.to_string(), parent: *parent, position: Vec3::ZERO, rotation: Quat::IDENTITY })
            .collect(),
        root: Transform::IDENTITY,
        animations: Some(Arc::new(AnimSet::default())),
        boxes: Vec::new(),
        ragdoll: None,
    };
    let mut sim = sim();
    let world = sim.app.world_mut();
    models::register(world, &builtin().model(id).unwrap().file, Arc::new(stand_in.rig(STAND_IN_ANIMS)));
    models::late_in(world).0.register_group(BodyGroup { records: vec![id], ..default() });
    world.insert_resource(CharacterModels(Arc::new(vec![model])));
    let a = player(&mut sim, Vec3::new(0.0, 1.0, 12.0), std::f32::consts::PI);
    let b = player(&mut sim, Vec3::new(0.5, 1.0, 4.0), 0.0);
    sim.app.world_mut().entity_mut(a).insert(Selection { model: id, ..default() });
    for p in [a, b] {
        weapon::give(sim.app.world_mut(), p, RIFLE).unwrap();
    }
    sim.seconds(1.3);
    let (mut modelled, mut fallback) = (0, 0);
    let mut sizes = [Vec3::ZERO; 2];
    for i in 0..300usize {
        let phase = i as f32 * 0.05;
        for (k, p) in [a, b].into_iter().enumerate() {
            let mut intent = sim.intent(p);
            intent.pitch = 0.7 * (phase + k as f32).sin();
            intent.move_axis = Vec2::new((phase * 0.7).sin(), (phase * 0.3 + k as f32).cos()) * 0.6;
            intent.crouch = (i / 40 + k) % 3 == 0;
            intent.fire = (i + 7 * k) % 12 < 2;
            if i % 25 == 0 {
                intent.yaw += 0.3;
            }
        }
        sim.ticks(1);
        let w = sim.app.world();
        for p in [a, b] {
            let lazy = w.get::<chimera_core::core::LazyHitboxes>(p).expect("hitboxes on demand");
            let bound = *lazy.1.as_ref().expect("a bound").get();
            let shapes = lazy.0.get();
            assert!(bound.holds(shapes), "tick {i}: {bound:?} doesn't hold {shapes:?}");
            let size = bound.max - bound.min;
            assert!(size.max_element() < 4.0, "tick {i}: a body's size: {size}");
            let pose = w.get::<CaPose>(p).unwrap();
            let k = if pose.sphere_reach().is_some() {
                modelled += 1;
                0
            } else {
                fallback += 1;
                1
            };
            sizes[k] = sizes[k].max(size);
        }
    }
    eprintln!("the largest bounds (m): with a model {}, on the spec's skeleton {}", sizes[0], sizes[1]);
    assert!(modelled == 300 && fallback == 300, "both kinds of body: {modelled} modelled, {fallback} on the spec's skeleton");
}

/// A character nobody reads is never posed: its pose, its hitboxes and
/// its drawn pose wait; read, each is what it would have been.
#[test]
fn a_pose_nobody_reads_is_not_made() {
    let mut sim = sim();
    let p = player(&mut sim, Vec3::new(0.0, 1.0, 12.0), 0.3);
    sim.ticks(2);
    let w = sim.app.world();
    let pose = w.get::<CaPose>(p).unwrap().clone();
    let boxes = w.get::<chimera_core::core::LazyHitboxes>(p).expect("hitboxes on demand").clone();
    assert!(!pose.is_posed() && !boxes.0.is_made(), "nothing asked");
    // Read: made once, and the hitboxes are the spheres where shots test them.
    let head = builtin().skeleton(0).spheres.iter().position(|s| s.name == "Head").unwrap();
    assert!(pose.spheres[head].is_some());
    assert!(pose.is_posed());
    assert_eq!(boxes.0.get().0.len(), builtin().skeleton(0).spheres.len());
    assert!(boxes.0.is_made());
}

/// A wait for a ladder's bodies ends when every one that can arrive has
/// (`preload::ladder_bodies_waiting`): a stock body a loaded group has is
/// there, not waited for as if it loaded on demand (the session tests timed
/// out on de_dust2's stock bodies 0 and 9 at 240 s), and a body whose load
/// failed is named (`BodiesOnDemand::failed`), not waited for.
#[test]
fn a_ladders_bodies_wait_only_for_what_can_arrive() {
    use bevy::tasks::{AsyncComputeTaskPool, TaskPool};
    use combat_arms::game::{
        characters::{
            bodies_on_demand::{self, BodiesOnDemand},
            preload::{LadderModels, ladder_bodies_waiting},
        },
        data::CaData,
    };

    AsyncComputeTaskPool::get_or_init(TaskPool::default);
    let mut app = App::new();
    let w = app.world_mut();
    w.insert_resource(CaData::builtin());
    w.insert_resource(chimera_core::tasks::WaitForTasks);
    w.init_resource::<BodiesOnDemand>();
    // A stock body, and a record no install has.
    w.insert_resource(LadderModels { bodies: vec![0, 99_999], keys: Vec::new() });
    assert_eq!(ladder_bodies_waiting(w), [0, 99_999], "nothing loaded yet");
    models::late_in(w).0.register_group(models::BodyGroup { records: vec![0, 1, 2], ..Default::default() });
    assert_eq!(ladder_bodies_waiting(w), [99_999], "the stock body's group is loaded");

    // Its load starts, then fails.
    w.run_system_cached(bodies_on_demand::load_on_demand).unwrap();
    assert!(w.resource::<BodiesOnDemand>().loading(99_999));
    assert_eq!(ladder_bodies_waiting(w), [99_999], "loading");
    w.run_system_cached(bodies_on_demand::load_on_demand).unwrap();
    assert!(w.resource::<BodiesOnDemand>().failed().contains_key(&99_999));
    assert!(ladder_bodies_waiting(w).is_empty(), "a failed body is not waited for");
}
