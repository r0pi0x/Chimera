//! DoD:S's prone, its deployed machine guns and its stamina on the
//! headless harness (`harness::DodsSim`, the greybox), from
//! specs/dods/movement.md's and weapons.md's test cases: `+alt1` eases the
//! eye to 10 over 80 ticks with the standing box kept to the 79th and the
//! 24-high box on the 80th, no movement meanwhile; a crawl of 7.5, 9.0,
//! then 50 units/s, the sprint key changing nothing; getting up (a
//! stand-in time); the .30 cal deployed by the alt fire while prone, its
//! first shot no sooner than 1.2 s later, a shot every 7 ticks, its sights'
//! spread, no movement and no stamina spent; fired standing, 100 to 85 on
//! the first shot and 8.7 less a shot after; the machine guns in the
//! ladders' heavy class.

use bevy::prelude::*;
use chimera_core::core::{ActionRegistry, Velocity};
use chimera_sim::weapon::{Hitscan, Inventory, SpreadShape, Weapon, WeaponEvent, WeaponEventKind, give};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use dods::{
    game::{
        TICK_INTERVAL, movement,
        stamina::DodsStamina,
        stance::{self, DodsStance, Stance},
        weapons::{self, CAL30},
    },
    harness::DodsSim,
};

const U: f32 = 0.0254;
/// Source movement's origin above the feet (half the standing box), metres.
const ORIGIN: f32 = 36.0 * U;

fn sim() -> Sim {
    let mut sim = Sim::new(DodsSim, GreyboxMapPlugin);
    sim.set_tick_interval(TICK_INTERVAL);
    sim
}

/// A settled player at the greybox's first spawn.
fn player(sim: &mut Sim) -> Entity {
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.seconds(1.5);
    p
}

fn eye(sim: &Sim, p: Entity) -> f32 {
    (ORIGIN + sim.state(p).eye_offset.y) / U
}

fn hull_top(sim: &Sim, p: Entity) -> f32 {
    let s = sim.state(p);
    (s.hull_max.y - s.hull_min.y) / U
}

fn speed(sim: &Sim, p: Entity) -> f32 {
    sim.velocity(p).xz().length() / U
}

fn stance_of(sim: &Sim, p: Entity) -> DodsStance {
    *sim.app.world().get::<DodsStance>(p).unwrap()
}

/// `+alt1` held for one tick (a press), the tick run.
fn press_prone(sim: &mut Sim, p: Entity) {
    let bit = sim.app.world().resource::<ActionRegistry>().bit(stance::PRONE_ACTION);
    assert_ne!(bit, 0, "+alt1 registered");
    sim.intent(p).actions |= bit;
    sim.ticks(1);
    sim.intent(p).actions &= !bit;
}

/// A player gone prone (80 ticks) and settled.
fn prone_player(sim: &mut Sim) -> Entity {
    let p = player(sim);
    press_prone(sim, p);
    sim.ticks(79);
    assert!(stance_of(sim, p).prone());
    sim.seconds(0.5);
    p
}

#[test]
fn going_prone_eases_the_eye_for_80_ticks_then_lowers_the_box() {
    let mut sim = sim();
    let p = player(&mut sim);
    assert!((eye(&sim, p) - 58.0).abs() < 0.1);
    press_prone(&mut sim, p);
    assert!((eye(&sim, p) - 56.61).abs() < 0.05, "after the press's tick: {}", eye(&sim, p));
    sim.ticks(8);
    assert!((eye(&sim, p) - 46.38).abs() < 0.05, "tick 9: {}", eye(&sim, p));
    // Held forward meanwhile: no movement.
    sim.intent(p).move_axis = Vec2::Y;
    sim.ticks(70);
    assert!(speed(&sim, p) < 0.01, "moving while going prone: {}", speed(&sim, p));
    assert!((eye(&sim, p) - 10.0).abs() < 0.05, "tick 79: {}", eye(&sim, p));
    assert!((hull_top(&sim, p) - 72.0).abs() < 0.01, "still the standing box at 79");
    assert_eq!(stance_of(&sim, p).stance, Stance::GoingDown);
    sim.ticks(1);
    assert!(stance_of(&sim, p).prone());
    assert!((hull_top(&sim, p) - 24.0).abs() < 0.01, "prone box at 80: {}", hull_top(&sim, p));
    assert!((eye(&sim, p) - 10.0).abs() < 0.01);
}

#[test]
fn a_prone_player_crawls_at_50_and_sprint_changes_nothing() {
    let mut sim = sim();
    let p = prone_player(&mut sim);
    sim.app.world_mut().get_mut::<Velocity>(p).unwrap().0 = Vec3::ZERO;
    sim.intent(p).move_axis = Vec2::Y;
    sim.ticks(1);
    assert!((speed(&sim, p) - 7.5).abs() < 0.05, "1 tick: {}", speed(&sim, p));
    sim.ticks(1);
    assert!((speed(&sim, p) - 9.0).abs() < 0.05, "2 ticks: {}", speed(&sim, p));
    sim.seconds(2.0);
    assert!((speed(&sim, p) - 50.0).abs() < 0.5, "crawl: {}", speed(&sim, p));
    sim.intent(p).walk = true;
    sim.intent(p).sprint = true;
    sim.seconds(1.0);
    assert!((speed(&sim, p) - 50.0).abs() < 0.5, "sprint held: {}", speed(&sim, p));
    // No jump while prone.
    sim.intent(p).jump = true;
    sim.ticks(10);
    assert!(sim.state(p).on_ground);
}

#[test]
fn alt1_again_gets_up_to_the_standing_box() {
    let mut sim = sim();
    let p = prone_player(&mut sim);
    press_prone(&mut sim, p);
    assert_eq!(stance_of(&sim, p).stance, Stance::GettingUp);
    assert!((hull_top(&sim, p) - 24.0).abs() < 0.01, "the low box while getting up");
    sim.seconds(stance::GET_UP as f64 + 0.1);
    assert_eq!(stance_of(&sim, p).stance, Stance::Up);
    assert!((hull_top(&sim, p) - 72.0).abs() < 0.01, "standing: {}", hull_top(&sim, p));
    assert!((eye(&sim, p) - 58.0).abs() < 0.1, "eye {}", eye(&sim, p));
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(1.0);
    assert!((speed(&sim, p) - 220.0).abs() < 3.0, "runs again: {}", speed(&sim, p));
}

/// `id` in `p`'s hands, drawn.
fn hold(sim: &mut Sim, p: Entity, id: &str) -> Entity {
    let w = give(sim.app.world_mut(), p, id).unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
    sim.seconds(1.5);
    assert_eq!(chimera_sim::testkit::active_id(sim.app.world(), p), Some(sim.app.world().get::<Weapon>(w).unwrap().id));
    w
}

/// `n` ticks, `input` before each: the ticks the player's weapon fired on.
fn fire_ticks(sim: &mut Sim, p: Entity, n: u64, mut input: impl FnMut(&mut Sim, u64)) -> Vec<u64> {
    let mut cursor = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
    let mut out = Vec::new();
    for t in 0..n {
        input(sim, t);
        sim.ticks(1);
        let events = sim.app.world().resource::<Messages<WeaponEvent>>();
        for e in cursor.read(events) {
            if e.owner == p && e.shown() && matches!(e.kind, WeaponEventKind::Fired { .. }) {
                out.push(t);
            }
        }
    }
    out
}

fn stamina(sim: &Sim, p: Entity) -> f32 {
    sim.app.world().get::<DodsStamina>(p).unwrap().0
}

#[test]
fn the_30cal_deploys_prone_waits_1_2_s_and_fires_on_its_sights_without_stamina() {
    let mut sim = sim();
    let p = prone_player(&mut sim);
    let w = hold(&mut sim, p, CAL30);
    sim.intent(p).pitch = 1.2;
    assert!(!stance_of(&sim, p).deployed);
    // The alt fire's press deploys; fire held from that tick.
    let shots = fire_ticks(&mut sim, p, 120, |s, t| {
        s.intent(p).secondary = t == 0;
        s.intent(p).fire = true;
    });
    assert!(stance_of(&sim, p).deployed);
    let first = *shots.first().expect("it fires once deployed");
    assert!(first as f64 * TICK_INTERVAL >= 1.2 - 1e-3, "first shot {first} ticks after deploying");
    assert!(shots.windows(2).all(|w| w[1] - w[0] == 7), "a shot every 7 ticks: {shots:?}");
    assert_eq!(sim.app.world().get::<Hitscan>(w).unwrap().spread, SpreadShape::Cone(0.01), "the sights' spread");
    assert!((stamina(&sim, p) - 100.0).abs() < 1e-3, "deployed: no stamina spent");
    // Deployed: no crawl.
    sim.intent(p).fire = false;
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(0.5);
    assert!(speed(&sim, p) < 0.01, "moving deployed: {}", speed(&sim, p));
    // The alt fire again undeploys; getting up would too.
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    assert!(!stance_of(&sim, p).deployed);
    sim.seconds(0.5);
    assert!(speed(&sim, p) > 40.0, "crawls again: {}", speed(&sim, p));
}

#[test]
fn the_30cal_fired_standing_costs_15_stamina_a_shot_and_climbs_back() {
    let mut sim = sim();
    let p = player(&mut sim);
    let w = hold(&mut sim, p, CAL30);
    sim.intent(p).pitch = 1.2;
    assert!((stamina(&sim, p) - 100.0).abs() < 1e-3, "a fresh player's 100");
    let mut after = Vec::new();
    let shots = fire_ticks(&mut sim, p, 8, |s, t| {
        s.intent(p).fire = true;
        if t > 0 {
            after.push(stamina(s, p));
        }
    });
    after.push(stamina(&sim, p));
    assert_eq!(shots, vec![0, 7], "two shots, 7 ticks apart");
    assert!((after[0] - 85.0).abs() < 1e-3, "after the first shot: {}", after[0]);
    assert!((after[7] - 76.3).abs() < 1e-3, "8.7 less after the second: {}", after[7]);
    assert_eq!(sim.app.world().get::<Hitscan>(w).unwrap().spread, SpreadShape::Cone(0.2), "undeployed: the script's 0.2");
    // Standing, the alt fire deploys nothing.
    sim.intent(p).secondary = true;
    sim.ticks(1);
    assert!(!stance_of(&sim, p).deployed);
    sim.intent(p).fire = false;
    sim.intent(p).secondary = false;
    sim.seconds(1.0);
    assert!((stamina(&sim, p) - 100.0).abs() < 1e-3, "climbs back: {}", stamina(&sim, p));
}

#[test]
fn the_machine_guns_are_the_ladders_heavy_class() {
    let set = weapons::dods_weapons();
    for id in [CAL30, weapons::MG42] {
        let w = set.get(id).unwrap();
        assert_eq!(w.class, Some(chimera_core::game_parts::GunClass::Heavy), "{id}");
        assert!(w.hooks.contains_key(weapons::MG_HOOK), "{id}");
    }
}
