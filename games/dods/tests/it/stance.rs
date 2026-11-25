//! DoD:S's prone, its deployed machine guns and its stamina on the
//! headless harness (`harness::DodsSim`, the greybox), from
//! specs/dods/movement.md's and weapons.md's test cases: `+alt1` eases the
//! eye to 10 over 80 ticks with the standing box kept to the 79th and the
//! 24-high box on the 80th, the speed capped meanwhile by the smoothstep
//! between the stances' caps (running, sprinting, the keys let go, from
//! rest, steering; getting up running and sprinting); a crawl of 7.5, 9.0,
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
    // Held forward meanwhile: moving, under the falling cap (50.08 on the
    // 79th tick).
    sim.intent(p).move_axis = Vec2::Y;
    sim.ticks(70);
    assert!((speed(&sim, p) - 50.08).abs() < 0.05, "moving while going prone: {}", speed(&sim, p));
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
    assert_eq!(sim.app.world().get::<Hitscan>(w).unwrap().spread, SpreadShape::TemplateDisc(0.01), "the sights' spread");
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
    assert_eq!(sim.app.world().get::<Hitscan>(w).unwrap().spread, SpreadShape::TemplateDisc(0.2), "undeployed: the script's 0.2");
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

/// Report 4effebf3: a rung that changes the movement while the gun is
/// deployed (DoD:S's stance goes with it) takes the turn limit off too;
/// it was left on, and the next game's scope (first person, where the
/// client clamps) stopped the turn at the old span.
#[test]
fn a_change_of_movement_while_deployed_takes_the_limit_off() {
    use bevy::ecs::system::Command;
    use chimera_core::core::ViewYawLimit;
    let mut sim = sim();
    let p = prone_player(&mut sim);
    hold(&mut sim, p, CAL30);
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    sim.ticks(1);
    assert!(sim.app.world().get::<ViewYawLimit>(p).is_some(), "deployed: limited");
    chimera_core::slots::set_movement(p, chimera_sim::movement::placeholder::ID).apply(sim.app.world_mut());
    sim.ticks(2);
    assert!(sim.app.world().get::<ViewYawLimit>(p).is_none(), "the limit went with the stance");
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

/// A crate 64 wide and deep standing 24 over `p`'s feet (the view along
/// -Z): its right face `right_edge` units left of the origin, its near
/// face `near` units ahead of it (negative: behind), 64 further on.
fn crate_at(sim: &mut Sim, p: Entity, right_edge: f32, near: f32) {
    let f = feet(sim, p);
    let o = sim.position(p);
    // The view is -Z; left is -X.
    solid(
        sim,
        Vec3::new(o.x - (right_edge + 64.0) * U, f.y - 10.0 * U, o.z - (near + 64.0) * U),
        Vec3::new(o.x - right_edge * U, f.y + 24.0 * U, o.z - near * U),
    );
}

/// The MG42 deployed prone looking along -Z after `place` puts a crate:
/// None when refused, else the (left, right) limits in degrees.
fn deployed_limits(place: impl FnOnce(&mut Sim, Entity)) -> Option<(f32, f32)> {
    use chimera_core::core::ViewYawLimit;
    let mut sim = sim();
    let p = player(&mut sim);
    sim.intent(p).yaw = 0.0;
    sim.intent(p).pitch = 0.0;
    press_prone(&mut sim, p);
    sim.ticks(79);
    sim.seconds(0.5);
    hold(&mut sim, p, weapons::MG42);
    place(&mut sim, p);
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    sim.ticks(1);
    let s = stance_of(&sim, p);
    if !s.deployed {
        assert!(sim.app.world().get::<ViewYawLimit>(p).is_none());
        return None;
    }
    let l = *sim.app.world().get::<ViewYawLimit>(p).expect("the client's limit while deployed");
    let (left, right) = ((l.centre + l.half).to_degrees(), (l.half - l.centre).to_degrees());
    Some(((left * 100.0).round() / 100.0, (right * 100.0).round() / 100.0))
}

/// weapons.md's test cases, what narrows a deployed MG's yaw limit: a
/// crate beside (its centre level with the player) whose near face is 18 /
/// 20 / 22 / 24 units to the left: the left limit 30 / 35 / 40 / 45, the
/// right 45; a crate whose near face is 20 ahead, its edge 1 / 3 / 5 / 10
/// left of the aim line: refused / 0 / 5 / 15; a crate overlapping the
/// player's own box: 45 both ways.
#[test]
fn obstacles_beside_a_deployed_mg_narrow_its_yaw_limit_in_5_degree_steps() {
    assert_eq!(deployed_limits(|_, _| {}), Some((45.0, 45.0)), "in the open");
    for (s, want) in [(18.0f32, 30.0f32), (20.0, 35.0), (22.0, 40.0), (24.0, 45.0)] {
        assert_eq!(deployed_limits(|sim, p| crate_at(sim, p, s, -32.0)), Some((want, 45.0)), "near face {s} to the left");
    }
    for (e, want) in [(1.0f32, None), (3.0, Some(0.0f32)), (5.0, Some(5.0)), (10.0, Some(15.0))] {
        assert_eq!(deployed_limits(|sim, p| crate_at(sim, p, e, 20.0)), want.map(|w| (w, 45.0)), "edge {e} left of the aim, 20 ahead");
    }
    // 90 degrees left at 42 (its near face 10 to the left: inside the box).
    assert_eq!(deployed_limits(|sim, p| crate_at(sim, p, 10.0, -32.0)), Some((45.0, 45.0)), "inside the player's box");
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

fn set_stamina(sim: &mut Sim, p: Entity, value: f32, sprinting: bool) {
    *sim.app.world_mut().get_mut::<DodsStamina>(p).unwrap() = DodsStamina { value, sprinting };
}

/// Moving forward (-Z) at `v` units/s.
fn set_speed(sim: &mut Sim, p: Entity, v: f32) {
    sim.app.world_mut().get_mut::<Velocity>(p).unwrap().0 = Vec3::NEG_Z * v * U;
}

/// `+alt1` pressed on the next tick, then the speed (units/s) after each
/// of the `n` ticks from it (k = 1 first).
fn speeds_from_the_press(sim: &mut Sim, p: Entity, n: usize) -> Vec<f32> {
    press_prone(sim, p);
    let mut v = vec![speed(sim, p)];
    for _ in 1..n {
        sim.ticks(1);
        v.push(speed(sim, p));
    }
    v
}

fn near(a: f32, b: f32, tol: f32, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a} against {b}");
}

/// movement.md, "The speed cap while changing stance": running at 220
/// with forward held, going prone caps the speed at 220 + (50 − 220) ×
/// smoothstep(k/80): 212.69 on the 10th tick, 135.01 on the 40th, 50.08 on
/// the 79th, then the crawl's 50.
#[test]
fn going_prone_running_caps_the_speed_by_a_smoothstep() {
    let mut sim = sim();
    let p = player(&mut sim);
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(1.0);
    near(speed(&sim, p), 220.0, 0.01, "running");
    let v = speeds_from_the_press(&mut sim, p, 90);
    near(v[9], 212.69, 0.05, "10 ticks");
    near(v[39], 135.01, 0.05, "40 ticks");
    near(v[78], 50.08, 0.05, "79 ticks");
    near(v[79], 50.0, 0.01, "80 ticks");
    near(v[89], 50.0, 0.01, "crawling");
    for k in 1..80 {
        let cap = stance::stance_cap(220.0, 50.0, k as f32 / 80.0);
        assert!(v[k - 1] <= cap + 0.01, "tick {k}: {} over the cap {cap}", v[k - 1]);
    }
}

/// The same pressed sprinting at 297.9 with 67.6 stamina, forward and
/// `+speed` held: the cap falls from the sprint's 230 + stamina (297.49
/// on the first tick, 195.41 on the 34th, 50 on the 80th); the sprint flag
/// stays while the speed is a sprint's and is gone by the end.
#[test]
fn going_prone_sprinting_falls_from_the_sprint_cap() {
    let mut sim = sim();
    let p = player(&mut sim);
    sim.intent(p).move_axis = Vec2::Y;
    sim.intent(p).sprint = true;
    set_speed(&mut sim, p, 297.9);
    set_stamina(&mut sim, p, 67.6, true);
    press_prone(&mut sim, p);
    assert!(sim.app.world().get::<DodsStamina>(p).unwrap().sprinting, "the flag stays");
    let mut v = vec![speed(&sim, p)];
    for _ in 1..80 {
        sim.ticks(1);
        v.push(speed(&sim, p));
    }
    near(v[0], 297.49, 0.05, "1 tick");
    near(v[33], 195.41, 0.3, "34 ticks");
    // The flag ends once the speed is under 80 (on the 63rd tick here,
    // after 79.89 on the 62nd): the cap from then on falls from the run's
    // (the game's 74.12 on the 64th is the sprint's: its flag ended a tick
    // later, Known shortcut in `stance`).
    assert!(v[62] < 80.0 && v[61] >= 79.0, "the speed passes 80 about the 62nd tick: {:?}", &v[58..64]);
    near(v[79], 50.0, 0.01, "80 ticks");
    assert!(stance_of(&sim, p).prone());
    assert!(!sim.app.world().get::<DodsStamina>(p).unwrap().sprinting, "the flag ended below 80");
}

/// Sprinting with the keys let go on the press: the run's cap, 120 +
/// stamina (187.5), at once, then ordinary friction (187.54, 165.03).
#[test]
fn going_prone_with_the_keys_let_go_drops_to_the_run_cap() {
    let mut sim = sim();
    let p = player(&mut sim);
    set_speed(&mut sim, p, 298.0);
    set_stamina(&mut sim, p, 67.6, true);
    let v = speeds_from_the_press(&mut sim, p, 2);
    near(v[0], 187.54, 0.15, "1 tick");
    near(v[1], 165.03, 0.15, "2 ticks");
}

/// From rest with forward held from the press the player accelerates as
/// usual to the falling cap (108.18 on the 4th tick, 206.21 on the 14th,
/// 50.08 on the 79th), and a 90° turn on the press swings the velocity
/// round with the view (the keys steer as usual).
#[test]
fn going_prone_from_rest_moves_and_steers_under_the_cap() {
    let mut still = sim();
    let p = player(&mut still);
    still.intent(p).move_axis = Vec2::Y;
    let v = speeds_from_the_press(&mut still, p, 79);
    near(v[3], 108.18, 0.25, "4 ticks");
    near(v[13], 206.21, 0.05, "14 ticks");
    near(v[78], 50.08, 0.05, "79 ticks");

    let mut turned = sim();
    let p = player(&mut turned);
    turned.intent(p).move_axis = Vec2::Y;
    turned.seconds(1.0);
    turned.intent(p).yaw += std::f32::consts::FRAC_PI_2;
    speeds_from_the_press(&mut turned, p, 30);
    // Yaw 0 looks down -Z; a quarter turn left looks down -X.
    let dir = turned.velocity(p).xz().normalize();
    assert!(dir.dot(Vec2::NEG_X) > 0.98, "the velocity swung round: {dir}");
}

/// Getting up from prone with forward held: the cap rises from the
/// crawl's 50 to the run's 220 (109.84 on the 32nd tick, 166.21 on the
/// 50th, 220 on the 80th).
#[test]
fn getting_up_raises_the_cap_to_the_run() {
    let mut sim = sim();
    let p = prone_player(&mut sim);
    sim.intent(p).move_axis = Vec2::Y;
    let v = speeds_from_the_press(&mut sim, p, 80);
    near(v[31], 109.84, 0.05, "32 ticks");
    near(v[49], 166.21, 0.05, "50 ticks");
    near(v[79], 220.0, 0.01, "80 ticks");
    assert_eq!(stance_of(&sim, p).stance, Stance::Up);
}

/// Getting up with `+speed` held: the sprint starts once the speed passes
/// 80 and the cap rises to the sprint's 230 + stamina instead (226.53 on
/// the 50th tick with stamina 78.2: about that).
#[test]
fn getting_up_sprinting_rises_to_the_sprint_cap() {
    let mut sim = sim();
    let p = prone_player(&mut sim);
    sim.intent(p).move_axis = Vec2::Y;
    sim.intent(p).sprint = true;
    let v = speeds_from_the_press(&mut sim, p, 50);
    let st = *sim.app.world().get::<DodsStamina>(p).unwrap();
    assert!(st.sprinting, "the sprint started getting up");
    let cap = stance::stance_cap(50.0, 230.0 + st.value, 50.0 / 80.0);
    near(v[49], cap, 1.0, "50 ticks, under the sprint's cap");
    assert!(v[49] > 220.0, "past the run's: {}", v[49]);
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

/// movement.md's test case "Garand sighted, still, hold forward": 42 is
/// the wish speed, so from rest 10 x 42 x 0.015 = 6.3 a tick against the
/// 6 friction takes below 50: 6.3, then +0.3 a tick (7.8 on the 6th), 42.0
/// on the 120th (Garand and K98 alike).
#[test]
fn sighted_from_rest_accelerates_0_3_a_tick_to_42() {
    for id in [weapons::GARAND, weapons::K98] {
        let mut sim = sim();
        let p = player(&mut sim);
        hold(&mut sim, p, id);
        sim.intent(p).secondary = true;
        sim.ticks(1);
        sim.intent(p).secondary = false;
        sim.seconds(1.0);
        assert!(speed(&sim, p) < 0.01, "still");
        sim.intent(p).move_axis = Vec2::Y;
        let mut got = Vec::new();
        for _ in 0..120 {
            sim.ticks(1);
            got.push(speed(&sim, p));
        }
        for (tick, want) in [(1, 6.3f32), (2, 6.6), (6, 7.8), (60, 24.0), (119, 41.7), (120, 42.0)] {
            assert!((got[tick - 1] - want).abs() < 0.02, "{id}: tick {tick}: {} against {want}", got[tick - 1]);
        }
    }
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
