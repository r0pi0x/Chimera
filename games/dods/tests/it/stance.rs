//! DoD:S's prone, its deployed machine guns and its stamina on the
//! headless harness (`harness::DodsSim`, the greybox), from
//! specs/dods/movement.md's and weapons.md's test cases: `+alt1` eases the
//! eye to 10 over 80 ticks with the standing box kept to the 79th and the
//! 24-high box on the 80th, no movement meanwhile; a crawl of 7.5, 9.0,
//! then 50 units/s, the sprint key changing nothing; getting up; the .30 cal deployed by the alt fire while prone, its
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
    assert!((hull_top(&sim, p) - 72.0).abs() < 0.01, "the standing box at once");
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
    sim.app.world().get::<DodsStamina>(p).unwrap().value
}

#[test]
fn the_30cal_deploys_prone_waits_1_2_s_and_fires_on_its_sights_without_stamina() {
    let mut sim = sim();
    let p = prone_player(&mut sim);
    let w = hold(&mut sim, p, CAL30);
    // At the sky, inside the deployed pitch limits (63 up).
    sim.intent(p).pitch = 1.0;
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

#[test]
fn the_30cal_reloads_only_deployed_and_waits_after_undeploying() {
    let mut sim = sim();
    let p = prone_player(&mut sim);
    let w = hold(&mut sim, p, CAL30);
    sim.intent(p).pitch = 1.0;
    let mag = |s: &Sim| s.app.world().get::<chimera_sim::weapon::Magazine>(w).unwrap().clip;
    // Undeployed: the reload key does nothing.
    sim.app.world_mut().get_mut::<chimera_sim::weapon::Magazine>(w).unwrap().clip = 0;
    sim.intent(p).reload = true;
    sim.seconds(5.0);
    sim.intent(p).reload = false;
    assert_eq!(mag(&sim), 0, "no reload undeployed");
    // Deployed: 150 rounds 4.6 s after the reload starts.
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    assert!(stance_of(&sim, p).deployed);
    let mut cursor = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
    let (mut started, mut full) = (None, None);
    for t in 0..700u64 {
        sim.intent(p).reload = true;
        sim.ticks(1);
        let events = sim.app.world().resource::<Messages<WeaponEvent>>();
        if cursor.read(events).any(|e| e.owner == p && matches!(e.kind, WeaponEventKind::ReloadStarted)) && started.is_none() {
            started = Some(t);
        }
        if mag(&sim) == 150 && full.is_none() {
            full = Some(t);
        }
    }
    sim.intent(p).reload = false;
    let took = (full.expect("full") - started.expect("a reload")) as f64 * TICK_INTERVAL;
    assert!((4.59..4.63).contains(&took), "the reload took {took} s");
    // Undeployed again: the next shot 1.3635 s on.
    let shots = fire_ticks(&mut sim, p, 120, |s, t| {
        s.intent(p).secondary = t == 0;
        s.intent(p).fire = true;
    });
    assert!(!stance_of(&sim, p).deployed);
    let first = *shots.first().expect("fires undeployed");
    assert!(first as f64 * TICK_INTERVAL >= 1.3635 - 1e-3 && (first as f64 * TICK_INTERVAL) < 1.4, "{first} ticks");
}

#[test]
fn turning_past_45_degrees_is_held_by_the_client_and_never_undeploys() {
    use chimera_core::core::ViewYawLimit;
    let mut sim = sim();
    let p = prone_player(&mut sim);
    hold(&mut sim, p, CAL30);
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    sim.ticks(1);
    assert!(stance_of(&sim, p).deployed);
    let yaw = sim.intent(p).yaw;
    let limit = *sim.app.world().get::<ViewYawLimit>(p).expect("the client's limit while deployed");
    assert!((limit.half - 45f32.to_radians()).abs() < 1e-5 && (limit.centre - yaw).abs() < 1e-5, "{limit:?}");
    // The server doesn't clamp or undeploy (weapons.md: turned to 60).
    sim.intent(p).yaw = yaw + 60f32.to_radians();
    sim.ticks(2);
    assert!(stance_of(&sim, p).deployed, "60 degrees: still deployed");
    // What the client holds the view at.
    assert!((limit.clamp(yaw + 60f32.to_radians()) - (yaw + 45f32.to_radians())).abs() < 1e-4);
    // Undeployed, the limit goes.
    sim.intent(p).yaw = yaw;
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    sim.ticks(1);
    assert!(sim.app.world().get::<ViewYawLimit>(p).is_none());
}

#[test]
fn looking_down_past_47_or_up_past_63_undeploys_staying_prone() {
    for (pitch, deployed) in [(-46.0f32, true), (-48.0, false), (62.0, true), (64.0, false)] {
        let mut sim = sim();
        let p = prone_player(&mut sim);
        hold(&mut sim, p, CAL30);
        sim.intent(p).secondary = true;
        sim.ticks(1);
        sim.intent(p).secondary = false;
        assert!(stance_of(&sim, p).deployed);
        sim.intent(p).pitch = pitch.to_radians();
        sim.ticks(2);
        assert_eq!(stance_of(&sim, p).deployed, deployed, "pitch {pitch}");
        assert!(stance_of(&sim, p).prone(), "still prone at {pitch}");
    }
}

/// A static box from `min` to `max` (m).
fn solid(sim: &mut Sim, min: Vec3, max: Vec3) {
    use avian3d::prelude::{Collider, RigidBody};
    let size = max - min;
    sim.app.world_mut().spawn((RigidBody::Static, Collider::cuboid(size.x, size.y, size.z), Transform::from_translation((min + max) / 2.0)));
    sim.ticks(1);
}

fn feet(sim: &Sim, p: Entity) -> Vec3 {
    sim.position(p) + Vec3::Y * sim.state(p).hull_min.y
}

/// A crate whose top is `top` units over `p`'s feet, `gap` units ahead
/// of its box (along -Z), 64 wide and deep.
fn crate_ahead(sim: &mut Sim, p: Entity, top: f32, gap: f32) {
    let f = feet(sim, p);
    let near = f.z - (16.0 + gap) * U;
    solid(sim, Vec3::new(f.x - 32.0 * U, f.y - 10.0 * U, near - 64.0 * U), Vec3::new(f.x + 32.0 * U, f.y + top * U, near));
}

/// weapons.md, "Alt fire", deploying on a ledge: the MG42 standing, a
/// 30-high ledge 2 units ahead: deployed, height 46, the eye to it;
/// crouched, a 38-high one: 54; a 56-high one standing: nothing.
#[test]
fn the_mg_deploys_on_a_ledge_standing_and_crouched() {
    for (top, crouched, want) in [(30.0f32, false, Some(46.0f32)), (38.0, true, Some(54.0)), (56.0, false, None), (40.0, false, Some(56.0))] {
        let mut sim = sim();
        let p = player(&mut sim);
        hold(&mut sim, p, weapons::MG42);
        sim.intent(p).yaw = 0.0;
        sim.intent(p).pitch = 0.0;
        if crouched {
            sim.intent(p).crouch = true;
            sim.seconds(0.6);
        }
        crate_ahead(&mut sim, p, top, 2.0);
        sim.intent(p).secondary = true;
        sim.ticks(1);
        sim.intent(p).secondary = false;
        sim.seconds(0.5);
        let s = stance_of(&sim, p);
        assert!(s.ledge.zip(want).map_or(s.ledge == want, |(a, b)| (a - b).abs() < 0.01), "top {top}: {:?}", s.ledge);
        assert_eq!(s.deployed, want.is_some(), "top {top}");
        if let Some(h) = want {
            assert!((eye(&sim, p) - h).abs() < 0.1, "top {top}: eye {}", eye(&sim, p));
            sim.intent(p).move_axis = Vec2::Y;
            sim.seconds(0.3);
            assert!(speed(&sim, p) < 0.01, "deployed on a ledge: no movement");
        }
    }
    // Too far: a 40-high ledge 22 units ahead of the box.
    let mut sim = sim();
    let p = player(&mut sim);
    hold(&mut sim, p, weapons::MG42);
    sim.intent(p).yaw = 0.0;
    crate_ahead(&mut sim, p, 40.0, 22.0);
    sim.intent(p).secondary = true;
    sim.ticks(1);
    assert!(!stance_of(&sim, p).deployed, "22 units ahead");
}

/// movement.md, "Prone": pressed 5 ticks into a jump, going prone starts
/// at once, the jump flies on with the standing box and lands; prone 80
/// ticks after the press.
#[test]
fn going_prone_in_the_air_finishes_the_jump() {
    let mut sim = sim();
    let p = player(&mut sim);
    let y0 = sim.position(p).y;
    sim.intent(p).jump = true;
    sim.ticks(5);
    sim.intent(p).jump = false;
    assert!(!sim.state(p).on_ground);
    press_prone(&mut sim, p);
    assert_eq!(stance_of(&sim, p).stance, Stance::GoingDown, "starts in the air");
    let mut top = 0.0f32;
    for _ in 0..30 {
        sim.ticks(1);
        top = top.max((sim.position(p).y - y0) / U);
    }
    assert!(top > 35.0, "the jump isn't cut: apex {top}");
    assert!((hull_top(&sim, p) - 72.0).abs() < 0.01, "the standing box in the air");
    sim.ticks(49);
    assert!(stance_of(&sim, p).prone(), "prone 80 ticks after the press");
    assert!(sim.state(p).on_ground);
}

/// movement.md, "Prone": pressed while sprinting the input stops counting
/// but the speed isn't cut (it decays by friction) and the sprint flag
/// stays while it is over 80.
#[test]
fn going_prone_sprinting_keeps_the_speed_and_the_flag() {
    let mut sim = sim();
    let p = player(&mut sim);
    sim.intent(p).move_axis = Vec2::Y;
    sim.intent(p).sprint = true;
    sim.seconds(1.0);
    let before = speed(&sim, p);
    assert!(before > 290.0, "sprinting {before}");
    press_prone(&mut sim, p);
    let after = speed(&sim, p);
    assert!(after > 250.0 && after < before, "not cut on the press: {before} then {after}");
    assert!(sim.app.world().get::<DodsStamina>(p).unwrap().sprinting, "the flag stays");
    let s0 = stamina(&sim, p);
    sim.ticks(1);
    assert!(stamina(&sim, p) < s0, "it drains");
    sim.ticks(78);
    assert!(stance_of(&sim, p).prone());
    assert!(!sim.app.world().get::<DodsStamina>(p).unwrap().sprinting, "ended below 80");
}

/// movement.md, "Prone": pressed ducked, the eye eases 34 → 10 with the
/// ducked box held until prone.
#[test]
fn going_prone_ducked_holds_the_ducked_box() {
    let mut sim = sim();
    let p = player(&mut sim);
    sim.intent(p).crouch = true;
    sim.seconds(0.6);
    sim.intent(p).crouch = false;
    let bit = sim.app.world().resource::<ActionRegistry>().bit(stance::PRONE_ACTION);
    sim.intent(p).actions |= bit;
    sim.intent(p).crouch = true;
    sim.ticks(1);
    sim.intent(p).actions &= !bit;
    sim.intent(p).crouch = false;
    sim.ticks(39);
    assert!((hull_top(&sim, p) - 45.0).abs() < 0.01, "ducked box at 40: {}", hull_top(&sim, p));
    let want = 34.0 - 24.0 * stance::prone_ease(40.0 / 80.0);
    assert!((eye(&sim, p) - want).abs() < 0.2, "eye {} against {want}", eye(&sim, p));
    sim.ticks(40);
    assert!(stance_of(&sim, p).prone());
}

/// movement.md, "Getting up": under a ceiling 50 up, `+alt1` is refused
/// and duck works; under 40, duck is refused too.
#[test]
fn a_low_ceiling_refuses_getting_up() {
    for (ceiling, stands, ducks) in [(50.0f32, false, true), (40.0, false, false), (80.0, true, true)] {
        for duck in [false, true] {
            let mut sim = sim();
            let p = prone_player(&mut sim);
            let f = feet(&sim, p);
            solid(&mut sim, f + Vec3::new(-100.0, ceiling, -100.0) * U, f + Vec3::new(100.0, ceiling + 10.0, 100.0) * U);
            if duck {
                sim.intent(p).crouch = true;
                sim.ticks(1);
                sim.intent(p).crouch = false;
            } else {
                press_prone(&mut sim, p);
            }
            let up = stance_of(&sim, p).stance == Stance::GettingUp;
            assert_eq!(up, if duck { ducks } else { stands }, "ceiling {ceiling}, duck {duck}");
            if !up {
                assert!(stance_of(&sim, p).prone() && (hull_top(&sim, p) - 24.0).abs() < 0.01, "nothing changes");
            }
        }
    }
}

/// weapons.md, "When a gun can't fire": crawling, held fire does nothing
/// until the crawl has stopped.
#[test]
fn no_shot_while_crawling() {
    let mut sim = sim();
    let p = prone_player(&mut sim);
    hold(&mut sim, p, weapons::THOMPSON);
    sim.intent(p).pitch = 0.6;
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(0.5);
    let mut stopped = None;
    let shots = fire_ticks(&mut sim, p, 60, |s, t| {
        s.intent(p).fire = true;
        s.intent(p).move_axis = if t < 20 { Vec2::Y } else { Vec2::ZERO };
        if t >= 20 && stopped.is_none() && speed(s, p) == 0.0 {
            stopped = Some(t);
        }
    });
    let first = *shots.first().expect("fires once stopped");
    assert!(first >= 20 && stopped.is_some_and(|s| first <= s + 1), "first {first}, stopped {stopped:?}");
}

/// movement.md, "View": with the Garand's sights up, at most 42 units/s.
#[test]
fn the_sights_cap_the_speed_at_42() {
    let mut sim = sim();
    let p = player(&mut sim);
    hold(&mut sim, p, weapons::GARAND);
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(1.0);
    assert!(speed(&sim, p) > 200.0);
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    sim.seconds(0.5);
    assert!((speed(&sim, p) - 42.0).abs() < 0.5, "sighted: {}", speed(&sim, p));
}

/// movement.md, "Stamina and sprint": sliding to a stop with no input it
/// regenerates at the moving rate (0.15 a tick) until the speed is 0, then
/// at the still rate (0.9).
#[test]
fn the_still_rate_only_at_speed_0() {
    let mut sim = sim();
    let p = player(&mut sim);
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(1.0);
    *sim.app.world_mut().get_mut::<DodsStamina>(p).unwrap() = DodsStamina { value: 20.0, sprinting: false };
    sim.intent(p).move_axis = Vec2::ZERO;
    sim.ticks(1);
    let a = stamina(&sim, p);
    sim.ticks(1);
    assert!((stamina(&sim, p) - a - 0.15).abs() < 1e-3, "sliding: {}", stamina(&sim, p) - a);
    sim.seconds(0.5);
    assert_eq!(speed(&sim, p), 0.0);
    let b = stamina(&sim, p);
    sim.ticks(1);
    assert!((stamina(&sim, p) - b - 0.9).abs() < 1e-3, "still: {}", stamina(&sim, p) - b);
}
