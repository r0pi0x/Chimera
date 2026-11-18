//! A gun game rung's Combat Arms character (`characters::CaRungLook`,
//! docs/DESIGN.md "Gun game rules") keeps only what is intrinsic to its
//! body: its body type (`FigureType`, gear_perks.md 2); its vest (armour,
//! protection, penalty, sprint and stamina bonuses, hit points: section 3)
//! is the baseline's (`gear::RUNG_VEST`, model 0's Normal). A stock body
//! and a specialist on a rung give the baseline's armour and movement; a
//! body whose body type has numbers of its own (test-written: the
//! install's two have the same) keeps them.

use std::sync::Arc;

use bevy::prelude::*;
use chimera_core::core::Health;
use chimera_sim::weapon::{Armor, ArmorPieces};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{
    game::{
        TICK_INTERVAL,
        characters::{CaRungLook, records::Selection},
        gear::{self, CaGearData},
        movement::{self, CaCharacter, MovementData},
    },
    harness::CaSim,
};

fn look(model: i32) -> Selection {
    Selection { model, ..default() }
}

/// What a player's gear came to: its movement terms, armour, pieces,
/// maximum health and armour pool.
#[derive(Debug, PartialEq)]
struct Stats {
    character: String,
    armour: String,
    max_health: f32,
    max_armour: f32,
}

/// A player in `look` (on a rung: `rung`), settled, after `tweak` on the
/// world (test data).
fn player(look: Selection, rung: bool, tweak: impl FnOnce(&mut World)) -> (Sim, Entity) {
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.set_tick_interval(TICK_INTERVAL);
    tweak(sim.app.world_mut());
    let p = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(p).insert(look);
    if rung {
        sim.app.world_mut().entity_mut(p).insert(CaRungLook(Some(look)));
    }
    sim.seconds(0.5);
    (sim, p)
}

fn stats(sim: &Sim, p: Entity) -> Stats {
    let w = sim.app.world();
    Stats {
        character: format!("{:?}", w.get::<CaCharacter>(p).expect("its gear derived")),
        armour: format!("{:?} {:?}", w.get::<Armor>(p).unwrap(), w.get::<ArmorPieces>(p).unwrap()),
        max_health: w.get::<Health>(p).unwrap().max,
        max_armour: gear::max_armour(w, p),
    }
}

fn baseline() -> Stats {
    let (sim, p) = player(look(0), false, |_| {});
    stats(&sim, p)
}

#[test]
fn a_stock_body_and_a_specialist_on_a_rung_play_as_the_baseline() {
    let base = baseline();
    assert_eq!(base.max_health, 1.0, "the baseline's 100 HP");
    // Heavy (piece 2, 0.30 protection, penalty 0.16) and Specialist01_M
    // (piece 52, penalty 0.03, stamina 0.2): their vests, off a rung.
    for body in [2, 22] {
        let (sim, p) = player(look(body), false, |_| {});
        assert_ne!(stats(&sim, p), base, "body {body} wears its own vest off a rung");
        let (sim, p) = player(look(body), true, |_| {});
        assert_eq!(stats(&sim, p), base, "body {body} on a rung: the baseline's armour and movement");
        assert_eq!(sim.app.world().get::<Selection>(p).map(|s| s.model), Some(body), "still drawn as itself");
    }
}

#[test]
fn a_body_type_of_its_own_is_kept_on_a_rung() {
    let base = baseline();
    // A body type with more sprint and stamina, and body 40 of it (the
    // specialist's vest numbers otherwise).
    let tweak = |w: &mut World| {
        let mut m = w.resource::<MovementData>().clone();
        let mut own = m.character.figure.clone();
        own.dash_speed = 1.25;
        own.max_stamina = 150.0;
        m.figures = vec![m.character.figure.clone(), m.character.figure.clone(), own];
        w.insert_resource(m);
        let mut data = (*w.resource::<CaGearData>().0).clone();
        let mut vest = data.vests[&22].clone();
        vest.figure = 2;
        data.vests.insert(40, vest);
        w.insert_resource(CaGearData(Arc::new(data)));
    };
    let (sim, p) = player(look(40), true, tweak);
    let s = stats(&sim, p);
    let c = sim.app.world().get::<CaCharacter>(p).unwrap();
    assert_eq!((c.figure.dash_speed, c.figure.max_stamina), (1.25, 150.0), "its own body type");
    assert_eq!(c.stamina_max(), 150.0, "its stamina, no vest bonus");
    assert_eq!((s.armour.clone(), s.max_health, s.max_armour), (base.armour, base.max_health, base.max_armour), "the baseline's armour");
    let b = player(look(0), false, |_| {});
    let bc = b.0.app.world().get::<CaCharacter>(b.1).unwrap();
    assert_eq!(format!("{:?}", c.gear), format!("{:?}", bc.gear), "the baseline vest's worn terms");
}

// ---------------------------------------------------------------------------
// game_modes.md Corrections (2026-10-30) 1 and 2: the infection's moment.

/// 109. The change effect's point: the floor straight below + 0.1 u (a
/// player 40 u above a floor at y 0 -> y 0.1 u; on a ledge's edge over a
/// floor 30 u lower, that lower floor + 0.1 u).
#[test]
fn t109_infection_point() {
    use combat_arms::game::{inventory::UNIT, rungs::infection_point};
    let at = infection_point(Vec3::new(1.0, 40.0 * UNIT, 2.0), Some(40.0 * UNIT));
    assert!((at - Vec3::new(1.0, 0.1 * UNIT, 2.0)).length() < 1e-6, "{at}");
    let at = infection_point(Vec3::new(0.0, 70.0 * UNIT, 0.0), Some(70.0 * UNIT));
    assert!((at.y - 0.1 * UNIT).abs() < 1e-6);
    assert_eq!(infection_point(Vec3::ONE, None), Vec3::ONE, "nothing below: where it stands");
}

/// 110. One infection sound at a time: another while one still plays
/// makes no sound; after it ends, the next plays.
#[test]
fn t110_one_infection_sound_at_a_time() {
    use combat_arms::game::rungs::InfectionSound;
    let mut s = InfectionSound::default();
    assert!(s.try_start(0.0, 1.5));
    assert!(!s.try_start(1.0, 1.5), "the first still plays");
    assert!(s.try_start(1.5, 1.5));
}
