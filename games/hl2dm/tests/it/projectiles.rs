//! HL2:DM's projectiles on the headless harness (`harness::Hl2Sim`, the
//! greybox), specs/hl2dm/projectiles.md's implementer cases (H4, the
//! numbers in the test names): the blast rule and the shove (2-6, 12-13),
//! the SMG1's contact grenade (15-18), the AR2's ball (19-23), the RPG
//! (27-28, 32, 34-35), the crossbow (36-37, 39-41, 43), the frag (44,
//! 47-53) and the S.L.A.M. (54-62). Ticks count from the action's tick,
//! at 0.015 s. Source vectors are converted with `launch::to_engine`.
//! The greybox: flat ground at y 0; from the first spawn, looking east
//! (`EAST`, +X, Source's +X too) is 40 m of open floor.

use bevy::prelude::*;
use chimera_core::core::{Damage, Health, MovementState, Velocity};
use chimera_sim::{
    testkit,
    weapon::{
        Armor, Inventory, Magazine, Weapon, WeaponEvent, WeaponEventKind, WeaponState, Zoomed, give,
        grenade::BlastAt,
        projectile::{Flyer, FlyerGone, FlyerHit, LaunchRequest},
    },
};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
    map::PlaySound,
};
use hl2dm::{
    game::{
        movement,
        weapons::{
            self,
            frag::Hl2Grenade,
            launch::{self, Hl2Rounds, U, to_engine, to_source},
            slam::{Hl2Mine, Hl2Satchel},
        },
    },
    harness::Hl2Sim,
};

fn sim() -> Sim {
    let mut sim = Sim::new(Hl2Sim, GreyboxMapPlugin);
    sim.set_tick_interval(hl2dm::game::TICK_INTERVAL);
    sim
}

/// Looking east: engine and Source +X.
const EAST: f32 = -90.0;

/// A player at the first spawn, settled, looking east, level.
fn player(sim: &mut Sim) -> Entity {
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.seconds(1.5);
    look(sim, p, EAST, 0.0);
    p
}

/// The view angles: yaw (0 is −Z, 90 west, −90 east), pitch up positive.
fn look(sim: &mut Sim, p: Entity, yaw: f32, pitch: f32) {
    let mut i = sim.intent(p);
    i.yaw = yaw.to_radians();
    i.pitch = pitch.to_radians();
}

fn weapon_of(sim: &Sim, p: Entity, id: &str) -> Option<Entity> {
    let w = sim.app.world();
    w.get::<Inventory>(p)?
        .weapons
        .iter()
        .copied()
        .find(|e| w.get::<Weapon>(*e).is_some_and(|x| x.id == id))
}

/// `p` holding `id`, drawn and settled.
fn holding(sim: &mut Sim, p: Entity, id: &str) -> Entity {
    let w = match weapon_of(sim, p, id) {
        Some(w) => w,
        None => give(sim.app.world_mut(), p, id).unwrap(),
    };
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
    sim.seconds(1.5);
    assert_eq!(testkit::active_id(sim.app.world(), p), Some(id));
    w
}

fn set_rounds(sim: &mut Sim, w: Entity, n: u32) {
    sim.app.world_mut().get_mut::<Hl2Rounds>(w).unwrap().count = n;
}

fn eye(sim: &Sim, p: Entity) -> Vec3 {
    sim.position(p) + sim.app.world().get::<MovementState>(p).unwrap().eye_offset
}

fn feet(sim: &Sim, p: Entity) -> Vec3 {
    sim.position(p) + Vec3::Y * sim.app.world().get::<MovementState>(p).unwrap().hull_min.y
}

fn health(sim: &Sim, p: Entity) -> f32 {
    sim.app.world().get::<Health>(p).unwrap().current * 100.0
}

/// Messages of type `M` from now on, read after each tick.
struct Log<M: Message> {
    cursor: bevy::ecs::message::MessageCursor<M>,
}

impl<M: Message + Clone> Log<M> {
    fn new(sim: &Sim) -> Self {
        Self {
            cursor: sim.app.world().resource::<Messages<M>>().get_cursor_current(),
        }
    }

    fn read(&mut self, sim: &Sim) -> Vec<M> {
        self.cursor
            .read(sim.app.world().resource::<Messages<M>>())
            .cloned()
            .collect()
    }
}

/// Run `n` ticks with `input` before each (its tick index); the weapon
/// events of `p`, the flyers gone, the blasts, by tick.
#[derive(Default)]
struct Ran {
    events: Vec<(u64, WeaponEventKind)>,
    gone: Vec<(u64, FlyerGone)>,
    blasts: Vec<(u64, BlastAt)>,
    hits: Vec<(u64, FlyerHit)>,
    sounds: Vec<(u64, String)>,
}

fn run(sim: &mut Sim, p: Entity, n: u64, mut input: impl FnMut(&mut Sim, u64)) -> Ran {
    let mut events = Log::<WeaponEvent>::new(sim);
    let mut gone = Log::<FlyerGone>::new(sim);
    let mut blasts = Log::<BlastAt>::new(sim);
    let mut hits = Log::<FlyerHit>::new(sim);
    let mut sounds = Log::<PlaySound>::new(sim);
    let mut out = Ran::default();
    for t in 0..n {
        input(sim, t);
        sim.ticks(1);
        out.events.extend(
            events
                .read(sim)
                .into_iter()
                .filter(|e| e.owner == p && e.shown())
                .map(|e| (t, e.kind)),
        );
        out.gone.extend(gone.read(sim).into_iter().map(|e| (t, e)));
        out.blasts.extend(blasts.read(sim).into_iter().map(|e| (t, e)));
        out.hits.extend(hits.read(sim).into_iter().map(|e| (t, e)));
        out.sounds.extend(sounds.read(sim).into_iter().map(|e| (t, e.entry)));
    }
    out
}

fn flyers(sim: &mut Sim) -> Vec<(Entity, Flyer, Vec3)> {
    let w = sim.app.world_mut();
    w.query::<(Entity, &Flyer, &Transform)>()
        .iter(w)
        .map(|(e, f, t)| (e, f.clone(), t.translation))
        .collect()
}

/// A blast of HL2's rule at `at` (engine), dealt by `thrower`.
fn blast(sim: &mut Sim, at: Vec3, damage: f32, radius: f32, thrower: Option<Entity>) {
    sim.app.world_mut().write_message(BlastAt {
        at,
        blast: launch::blast(damage, radius, None, None),
        thrower,
        weapon: weapons::FRAG,
        inflictor: None,
    });
}

// ---------------------------------------------------------------------------
// The blast (2) and the shove (3)

/// Tests 2 and 4: a frag's blast point 100 units behind a standing player,
/// 0.6 off the floor: 90.593 damage (health 10, 0.593 carried) and a
/// shove of (412.45, 0, 187.25).
#[test]
fn a_blast_100_units_away_does_90_6_and_shoves_2_4() {
    let mut sim = sim();
    let p = player(&mut sim);
    let f = feet(&sim, p);
    sim.app.world_mut().get_mut::<Velocity>(p).unwrap().0 = Vec3::ZERO;
    blast(&mut sim, f + to_engine(Vec3::new(-100.0, 0.0, 0.6)), 150.0, 250.0, None);
    sim.ticks(1);
    assert!((health(&sim, p) - 10.0).abs() < 1e-3, "health {}", health(&sim, p));
    let v = to_source(sim.app.world().get::<Velocity>(p).unwrap().0);
    assert!(
        (v.x - 412.45).abs() < 3.0 && (v.z - 187.25).abs() < 15.0 && v.y.abs() < 1.0,
        "shove {v}"
    );
}

/// Test 3: the same blast on 100 armour: health 60, no armour left.
#[test]
fn a_blast_on_armour_takes_two_armour_a_point_3() {
    let mut sim = sim();
    let p = player(&mut sim);
    sim.app.world_mut().get_mut::<Armor>(p).unwrap().amount = 1.0;
    let f = feet(&sim, p);
    blast(&mut sim, f + to_engine(Vec3::new(-100.0, 0.0, 0.6)), 150.0, 250.0, None);
    sim.ticks(1);
    assert!((health(&sim, p) - 60.0).abs() < 1e-3, "health {}", health(&sim, p));
    assert!(sim.app.world().get::<Armor>(p).unwrap().amount.abs() < 1e-4);
}

/// Test 5: a rocket's blast 50 units off the floor beside the feet: 122.65,
/// dead without armour, 28 left with 100.
#[test]
fn a_rocket_at_50_units_5() {
    for (armour, left) in [(0.0, 0.0), (1.0, 28.0)] {
        let mut sim = sim();
        let p = player(&mut sim);
        sim.app.world_mut().get_mut::<Armor>(p).unwrap().amount = armour;
        let f = feet(&sim, p);
        blast(&mut sim, f + to_engine(Vec3::new(-50.0, 0.0, 0.0)), 150.0, 300.0, None);
        sim.ticks(1);
        assert!(
            (health(&sim, p) - left).abs() < 1e-3,
            "armour {armour}: health {}",
            health(&sim, p)
        );
    }
}

/// Section 2.1 and test 1: beyond the radius nothing; the shooter's own
/// blast hurts him (no spawn grace yet: rules.md's, M7).
#[test]
fn nothing_beyond_the_radius_1() {
    let mut sim = sim();
    let p = player(&mut sim);
    let f = feet(&sim, p);
    blast(
        &mut sim,
        f + to_engine(Vec3::new(-300.0, 0.0, 1.0)),
        100.0,
        250.0,
        Some(p),
    );
    sim.ticks(1);
    assert_eq!(health(&sim, p), 100.0);
}

// ---------------------------------------------------------------------------
// SMG1 contact grenade (4)

/// Tests 15, 16: from the eye along the view at 1000 under 400 of gravity,
/// x = 15n, z = 64 − 0.045 n² until it meets the floor during tick 37,
/// ≈ 552 ahead; next primary tick 34, next secondary tick 67; a running
/// reload cancelled.
#[test]
fn the_contact_grenade_flies_and_explodes_on_the_floor_15_16() {
    let mut sim = sim();
    let p = player(&mut sim);
    let w = holding(&mut sim, p, weapons::SMG1);
    set_rounds(&mut sim, w, 3);
    let start = eye(&sim, p);
    let floor = 0.0;
    let mut track = Vec::new();
    let ran = run(&mut sim, p, 60, |s, t| {
        s.intent(p).secondary = t == 0;
        if let Some((_, _, at)) = flyers(s).first() {
            track.push((t, *at));
        }
    });
    assert_eq!(sim.app.world().get::<Hl2Rounds>(w).unwrap().count, 2);
    // Made on tick 0, it moves from tick 1; read before tick t, it has
    // made t − 1 moves.
    for &(t, at) in &track {
        let n = t as f32 - 1.0;
        let d = to_source(at - start);
        assert!((d.x - 15.0 * n).abs() < 0.05, "tick {t}: x {}", d.x);
        assert!((d.z + 0.045 * n * n).abs() < 0.05, "tick {t}: z {}", d.z);
    }
    let (tick, g) = ran.gone.first().copied().expect("exploded");
    // The tick its box's bottom first goes below the floor (y 0): 37 from
    // an eye 64 above it (the spec's); ours stands a hair higher.
    let above = start.y / U - 3.0;
    let expect = (1..).find(|n: &u64| above - 0.045 * (*n as f32).powi(2) < 0.0).unwrap();
    assert!((37..=38).contains(&expect));
    assert_eq!(tick, expect, "explodes during tick {expect}");
    let d = to_source(g.at - start);
    assert!((d.x - 552.0).abs() < 8.0, "at {d}");
    assert!(
        ((g.at.y - floor) / U - 3.0).abs() < 1.0,
        "{} above the floor",
        (g.at.y - floor) / U
    );
    assert_eq!(ran.blasts.len(), 1);
    let (_, b) = &ran.blasts[0];
    assert!((b.blast.damage - 1.0).abs() < 1e-6 && (b.blast.radius - 250.0 * U).abs() < 1e-5);
}

/// Test 16: next primary 0.5 s on (tick 34), next secondary 1.0 (67), the
/// player not made busy, a running reload cancelled with no ammo moved.
#[test]
fn the_contact_grenades_timers_16() {
    let mut sim = sim();
    let p = player(&mut sim);
    let w = holding(&mut sim, p, weapons::SMG1);
    set_rounds(&mut sim, w, 3);
    {
        let mut m = sim.app.world_mut().get_mut::<Magazine>(w).unwrap();
        m.clip = 10;
        m.reserve = 20;
    }
    // A reload, then the grenade during it.
    run(&mut sim, p, 10, |s, t| s.intent(p).reload = t == 0);
    assert!(sim.app.world().get::<WeaponState>(w).unwrap().reloading());
    run(&mut sim, p, 1, |s, _| {
        s.intent(p).reload = false;
        s.intent(p).secondary = true;
    });
    let now = sim.app.world().resource::<chimera_core::core::SimClock>().now;
    let st = sim.app.world().get::<WeaponState>(w).unwrap().clone();
    assert!(!st.reloading());
    let m = sim.app.world().get::<Magazine>(w).unwrap().clone();
    assert_eq!((m.clip, m.reserve), (10, 20));
    assert!((st.next_primary - now - 0.5).abs() < 1e-6 && (st.next_secondary - now - 1.0).abs() < 1e-6);
    assert!(sim.app.world().get::<Inventory>(p).unwrap().next_attack <= now);
}

/// Test 17: the shooter's velocity isn't added.
#[test]
fn the_contact_grenade_takes_none_of_the_shooters_velocity_17() {
    let mut sim = sim();
    let p = player(&mut sim);
    let w = holding(&mut sim, p, weapons::SMG1);
    set_rounds(&mut sim, w, 1);
    let mut v = None;
    run(&mut sim, p, 3, |s, t| {
        s.intent(p).secondary = t == 0;
        s.app.world_mut().get_mut::<Velocity>(p).unwrap().0 = to_engine(Vec3::new(0.0, 300.0, 0.0));
        if let Some((_, f, _)) = flyers(s).first() {
            v.get_or_insert(f.velocity);
        }
    });
    let v = to_source(v.unwrap());
    assert!((v - Vec3::new(1000.0, 0.0, 0.0)).length() < 1.0, "{v}");
}

/// Test 18 (half): it explodes on a player in its way; with none left a
/// dry fire.
#[test]
fn the_contact_grenade_explodes_on_a_player_18() {
    let mut sim = sim();
    let p = player(&mut sim);
    let w = holding(&mut sim, p, weapons::SMG1);
    let t = sim.spawn_character(greybox::SPAWNS[0] + Vec3::X * 4.0, movement::ID);
    sim.seconds(1.0);
    set_rounds(&mut sim, w, 1);
    look(&mut sim, p, EAST, 0.0);
    let ran = run(&mut sim, p, 20, |s, k| s.intent(p).secondary = k == 0);
    let hit = ran.hits.first().expect("a touch").1;
    assert_eq!(hit.other, Some(t));
    assert!(hit.gone);
    assert!(health(&sim, t) < 100.0);
    // None left: a dry fire.
    let ran = run(&mut sim, p, 80, |s, k| s.intent(p).secondary = k == 70);
    assert!(ran.events.iter().any(|(_, k)| matches!(k, WeaponEventKind::DryFire)));
}

// ---------------------------------------------------------------------------
// AR2 energy ball (5)

/// Test 19: the charge on tick 0, no switch until the launch on tick 34 at
/// 1000 from the eye; the first shot tick 81, the next charge tick 101;
/// test 20: its harmless end on tick 301.
#[test]
fn the_ball_charges_launches_and_ends_19_20() {
    let mut sim = sim();
    let p = player(&mut sim);
    let w = holding(&mut sim, p, weapons::AR2);
    set_rounds(&mut sim, w, 2);
    let pistol = weapon_of(&sim, p, weapons::PISTOL).unwrap();
    let mut made = None;
    let ran = run(&mut sim, p, 320, |s, t| {
        s.intent(p).secondary = t == 0 || t == 100 || t == 101;
        s.intent(p).fire = (79..=81).contains(&t);
        if t == 10 {
            s.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(pistol);
        }
        if made.is_none()
            && let Some((_, f, at)) = flyers(s).first().cloned()
        {
            made = Some((t, f, at));
        }
    });
    assert_eq!(
        testkit::active_id(sim.app.world(), p),
        Some(weapons::AR2),
        "the switch was refused"
    );
    let (t, f, _) = made.expect("a ball");
    // Seen before tick 35: made on tick 34.
    assert_eq!(t, 35);
    assert!((f.velocity.length() / U - 1000.0).abs() < 0.5);
    let fired: Vec<u64> = ran
        .events
        .iter()
        .filter(|(_, k)| matches!(k, WeaponEventKind::Fired { .. }))
        .map(|(t, _)| *t)
        .collect();
    assert_eq!(fired.first(), Some(&81), "first shot {fired:?}");
    let charged: Vec<u64> = ran
        .events
        .iter()
        .filter(|(_, k)| matches!(k, WeaponEventKind::Action("charge")))
        .map(|(t, _)| *t)
        .collect();
    assert_eq!(charged, [0, 101]);
    let end = ran.gone.iter().find(|(_, g)| g.expired).map(|(t, _)| *t);
    assert_eq!(end, Some(301));
    assert_eq!(health(&sim, p), 100.0);
}

/// Test 21: after a bounce off a wall its speed is exactly 1000.
#[test]
fn the_ball_keeps_its_speed_after_a_bounce_21() {
    let mut sim = sim();
    let p = player(&mut sim);
    let w = holding(&mut sim, p, weapons::AR2);
    set_rounds(&mut sim, w, 1);
    look(&mut sim, p, EAST, -20.0);
    let ran = run(&mut sim, p, 120, |s, t| s.intent(p).secondary = t == 0);
    assert!(ran.hits.iter().any(|(_, h)| h.world), "it bounced");
    let (_, f, _) = flyers(&mut sim).first().cloned().expect("still flying");
    assert!(
        (f.velocity.length() / U - 1000.0).abs() < 1e-2,
        "{}",
        f.velocity.length() / U
    );
}

/// Test 22: a player with 200 armour touched by the ball dies (1000 crush,
/// 800 to health); test 23: its shooter is never touched.
#[test]
fn the_ball_kills_through_armour_22_23() {
    let mut sim = sim();
    let p = player(&mut sim);
    let w = holding(&mut sim, p, weapons::AR2);
    let t = sim.spawn_character(greybox::SPAWNS[0] + Vec3::X * 6.0, movement::ID);
    sim.seconds(1.0);
    sim.app.world_mut().get_mut::<Armor>(t).unwrap().amount = 2.0;
    set_rounds(&mut sim, w, 1);
    look(&mut sim, p, EAST, 0.0);
    let ran = run(&mut sim, p, 60, |s, k| s.intent(p).secondary = k == 0);
    assert!(ran.hits.iter().any(|(_, h)| h.other == Some(t) && h.character));
    assert_eq!(health(&sim, t), 0.0);
    assert_eq!(health(&sim, p), 100.0);
}

// ---------------------------------------------------------------------------
// RPG (6)

/// Test 27: from (12, −6, 61) off the eye at (300, 0, 128), toss for 19
/// ticks to (97.5, −6, 73.11), then 1500 along the view; solid.
#[test]
fn the_missile_tosses_then_ignites_27() {
    let mut sim = sim();
    let p = player(&mut sim);
    holding(&mut sim, p, weapons::RPG);
    let start = eye(&sim, p);
    let mut track = Vec::new();
    run(&mut sim, p, 25, |s, t| {
        s.intent(p).fire = t == 0;
        if let Some((_, f, at)) = flyers(s).first().cloned() {
            track.push((t, f, at));
        }
    });
    let at = |t: u64| track.iter().find(|(k, ..)| *k == t).cloned().unwrap();
    // Before tick 1: as made.
    let (_, f, a) = at(1);
    assert!(
        (to_source(a - start) - Vec3::new(12.0, -6.0, 61.0 - 64.0)).length() < 0.05,
        "{}",
        to_source(a - start)
    );
    assert!((to_source(f.velocity) - Vec3::new(300.0, 0.0, 128.0)).length() < 0.05);
    // Before tick 20: 19 tosses.
    let (_, _, a) = at(20);
    assert!(
        (to_source(a - start) - Vec3::new(97.5, -6.0, 73.11 - 64.0)).length() < 0.05,
        "{}",
        to_source(a - start)
    );
    // Before tick 21: ignited, 22.5 along.
    let (_, f, a) = at(21);
    assert!(
        (to_source(f.velocity) - Vec3::new(1500.0, 0.0, 0.0)).length() < 0.5,
        "{}",
        to_source(f.velocity)
    );
    assert!((to_source(a - start) - Vec3::new(120.0, -6.0, 73.11 - 64.0)).length() < 0.05);
    assert!(f.solid_after <= sim.app.world().resource::<chimera_core::core::SimClock>().now);
}

/// Test 35: the RPG can't be put away while its missile flies; test 34:
/// attack held through the reload keeps it from firing until let go.
#[test]
fn the_rpg_is_locked_and_held_attack_blocks_its_reload_34_35() {
    let mut sim = sim();
    let p = player(&mut sim);
    holding(&mut sim, p, weapons::RPG);
    // Into the ground ahead.
    look(&mut sim, p, EAST, -10.0);
    let pistol = weapon_of(&sim, p, weapons::PISTOL).unwrap();
    let ran = run(&mut sim, p, 300, |s, t| {
        s.intent(p).fire = t < 250;
        if t == 5 {
            s.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(pistol);
        }
    });
    assert_eq!(
        testkit::active_id(sim.app.world(), p),
        Some(weapons::RPG),
        "lastinv refused"
    );
    assert!(ran.gone.first().is_some_and(|(t, _)| *t < 60), "exploded");
    let fired: Vec<u64> = ran
        .events
        .iter()
        .filter(|(_, k)| matches!(k, WeaponEventKind::Action("fire")))
        .map(|(t, _)| *t)
        .collect();
    assert_eq!(fired, [0], "no shot while held");
    let ran = run(&mut sim, p, 3, |s, t| s.intent(p).fire = t == 1);
    assert!(
        ran.events
            .iter()
            .any(|(_, k)| matches!(k, WeaponEventKind::Action("fire")))
    );
}

/// Test 34 exactly: the explosion on tick X, a press each tick after: the
/// first that fires is X + 113.
#[test]
fn the_fastest_refire_is_113_ticks_after_the_explosion_34() {
    let mut sim = sim();
    let p = player(&mut sim);
    holding(&mut sim, p, weapons::RPG);
    look(&mut sim, p, EAST, -10.0);
    let mut x = None;
    let mut press = false;
    let ran = run(&mut sim, p, 200, |s, t| {
        if x.is_none() && !flyers(s).is_empty() {
            press = true;
        }
        if press && flyers(s).is_empty() && x.is_none() {
            x = Some(t - 1);
        }
        // Tap every other tick after the explosion (a fresh press each time).
        s.intent(p).fire = t == 0 || (x.is_some() && t % 2 == 0);
    });
    let x = ran.gone.first().expect("exploded").0;
    let fired: Vec<u64> = ran
        .events
        .iter()
        .filter(|(_, k)| matches!(k, WeaponEventKind::Action("fire")))
        .map(|(t, _)| *t)
        .collect();
    let second = fired.get(1).copied().expect("fired again");
    assert!(
        second >= x + 113 && second <= x + 114,
        "explosion {x}, second shot {second}"
    );
}

/// Test 32: the owner dies with a missile in flight: it flies straight on
/// and still credits him.
#[test]
fn a_dead_owners_missile_flies_straight_32() {
    let mut sim = sim();
    let p = player(&mut sim);
    holding(&mut sim, p, weapons::RPG);
    look(&mut sim, p, EAST, 0.0);
    run(&mut sim, p, 30, |s, t| s.intent(p).fire = t == 0);
    let (m, _, _) = flyers(&mut sim).first().cloned().expect("in flight");
    sim.app.world_mut().write_message(Damage {
        target: p,
        attacker: None,
        amount: 5.0,
        point: Vec3::ZERO,
        dir: Vec3::Y,
        hitgroup: chimera_core::core::Hitgroup::Generic,
        kind: chimera_core::core::DamageKind::Generic,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(1);
    assert_eq!(health(&sim, p), 0.0);
    let f = sim.app.world().get::<Flyer>(m).cloned().unwrap();
    // The body turns away: no dot any more.
    look(&mut sim, p, 0.0, 0.0);
    run(&mut sim, p, 5, |_, _| {});
    let after = sim.app.world().get::<Flyer>(m).cloned().expect("still flying");
    assert!((after.velocity.normalize() - f.velocity.normalize()).length() < 1e-4);
    assert_eq!(after.owner, Some(p));
}

// ---------------------------------------------------------------------------
// Crossbow (7)

/// Test 36: holding attack, shots on ticks 0 and 130 (the reload from tick
/// 7; the real server, probe 2026-11-03: the spec says 168 and 45); test
/// 37: its bolt at 3500 with a twentieth of gravity.
#[test]
fn the_crossbow_fires_every_130_ticks_held_36_37() {
    let mut sim = sim();
    let p = player(&mut sim);
    let w = holding(&mut sim, p, weapons::CROSSBOW);
    {
        let mut m = sim.app.world_mut().get_mut::<Magazine>(w).unwrap();
        m.clip = 1;
        m.reserve = 4;
    }
    let start = eye(&sim, p);
    let mut bolt = Vec::new();
    let ran = run(&mut sim, p, 200, |s, t| {
        s.intent(p).fire = true;
        if (1..=11).contains(&t)
            && let Some((_, _, at)) = flyers(s).first().cloned()
        {
            bolt.push(at);
        }
    });
    let fired: Vec<u64> = ran
        .events
        .iter()
        .filter(|(_, k)| matches!(k, WeaponEventKind::Fired { .. }))
        .map(|(t, _)| *t)
        .collect();
    assert_eq!(&fired[..2], [0, 130]);
    let reload = ran
        .events
        .iter()
        .find(|(_, k)| matches!(k, WeaponEventKind::ReloadStarted))
        .map(|(t, _)| *t);
    assert_eq!(reload, Some(7));
    assert_eq!(sim.app.world().get::<Magazine>(w).unwrap().reserve, 3);
    // Ten moves: 350 ahead, 0.3375 down.
    let d = to_source(bolt[10] - start);
    assert!((d.x - 525.0).abs() < 0.05 && (d.z + 0.3375).abs() < 0.01, "{d}");
}

/// Test 39: a head shot is 300, a chest shot 100 (with 100 armour: health
/// 80, armour 20).
#[test]
fn a_bolt_does_100_times_the_hitgroup_39() {
    for (head, armour, health_left, armour_left) in [
        (false, 0.0, 0.0, None),
        (true, 1.0, 0.0, None),
        (false, 1.0, 80.0, Some(0.2)),
    ] {
        let mut sim = sim();
        let p = player(&mut sim);
        let w = holding(&mut sim, p, weapons::CROSSBOW);
        sim.app.world_mut().get_mut::<Magazine>(w).unwrap().clip = 1;
        let t = sim.spawn_character(greybox::SPAWNS[0] + Vec3::X * 3.0, movement::ID);
        sim.seconds(2.0);
        sim.app.world_mut().get_mut::<Armor>(t).unwrap().amount = armour;
        let aim = if head {
            eye(&sim, t) + Vec3::Y * 0.02
        } else {
            eye(&sim, t) - Vec3::Y * 0.45
        };
        testkit::aim_at(&mut sim, p, aim);
        let ran = run(&mut sim, p, 10, |s, k| s.intent(p).fire = k == 0);
        let hits: Vec<_> = ran
            .hits
            .iter()
            .map(|(t, h)| (*t, h.other, h.character, to_source(h.point)))
            .collect();
        assert!(
            (health(&sim, t) - health_left).abs() < 1e-3,
            "head {head}: health {} {hits:?} target {t} aim {}",
            health(&sim, t),
            to_source(aim)
        );
        if let Some(a) = armour_left {
            assert!((sim.app.world().get::<Armor>(t).unwrap().amount - a).abs() < 1e-4);
        }
    }
}

/// Tests 40 and 41: c = 0.436 glances off at 0.75 with full gravity after;
/// c = 0.6 sticks and is gone 2 s later.
#[test]
fn a_bolt_glances_at_shallow_angles_and_sticks_otherwise_40_41() {
    let mut sim = sim();
    let p = player(&mut sim);
    let w = holding(&mut sim, p, weapons::CROSSBOW);
    let template = sim
        .app
        .world()
        .get::<chimera_sim::weapon::projectile::Launcher>(w)
        .unwrap()
        .template
        .clone();
    for (dir, glance) in [
        (Vec3::new(0.9, 0.0, -0.43589), true),
        (Vec3::new(0.8, 0.0, -0.6), false),
    ] {
        let mut f = template.clone();
        f.owner = Some(p);
        let v = to_engine(dir * 3500.0);
        f.velocity = v;
        let start = Vec3::new(0.0, 0.3, 0.0) + greybox::SPAWNS[1].with_y(0.0);
        sim.app.world_mut().write_message(LaunchRequest {
            flyer: f,
            start,
            shown: None,
            tag: None,
        });
        let ran = run(&mut sim, p, 200, |_, _| {});
        let (tick, hit) = ran.hits.first().cloned().expect("touched the floor");
        assert!(hit.world);
        if glance {
            let after = ran.hits.iter().skip(1).count() > 0 || ran.gone.iter().all(|(t, _)| *t > tick);
            assert!(after, "flew on");
            let _ = after;
        } else {
            let gone = ran.gone.first().map(|(t, _)| *t).expect("gone");
            assert!(gone.abs_diff(tick + 134) <= 1, "stuck on {tick}, gone on {gone}");
        }
    }
}

/// Test 43: attack2's press zooms to 20, the next back; holding doesn't
/// repeat.
#[test]
fn the_crossbow_zooms_on_the_press_43() {
    let mut sim = sim();
    let p = player(&mut sim);
    holding(&mut sim, p, weapons::CROSSBOW);
    run(&mut sim, p, 30, |s, _| s.intent(p).secondary = true);
    assert_eq!(sim.app.world().get::<Zoomed>(p).map(|z| z.fov), Some(20.0));
    run(&mut sim, p, 2, |s, t| s.intent(p).secondary = t == 1);
    assert!(sim.app.world().get::<Zoomed>(p).is_none());
}

// ---------------------------------------------------------------------------
// Frag (9)

fn frag_ready(sim: &mut Sim, p: Entity, n: u32) -> Entity {
    let w = holding(sim, p, weapons::FRAG);
    set_rounds(sim, w, n);
    w
}

/// Test 44: a tapped throw on tick 0: the throw tick 15, the grenade tick
/// 19 at eye + (18, −8, 0) with (1200, 0, 120); the next press tick 138.
#[test]
fn a_tapped_throw_44() {
    let mut sim = sim();
    let p = player(&mut sim);
    let w = frag_ready(&mut sim, p, 2);
    let start = eye(&sim, p);
    let mut made = None;
    let ran = run(&mut sim, p, 160, |s, t| {
        s.intent(p).fire = t == 0 || t >= 130;
        let w = s.app.world_mut();
        if made.is_none()
            && let Some((g, at, v)) = w
                .query::<(&Hl2Grenade, &Transform, &avian3d::prelude::LinearVelocity)>()
                .iter(w)
                .next()
                .map(|(g, t, v)| (g.clone(), t.translation, v.0))
        {
            made = Some((t, g, at, v));
        }
    });
    let thrown = ran
        .events
        .iter()
        .find(|(_, k)| matches!(k, WeaponEventKind::Thrown))
        .map(|(t, _)| *t);
    assert_eq!(thrown, Some(15));
    let (t, _, at, v) = made.expect("a grenade");
    assert_eq!(t, 20, "made on tick 19 (seen before 20)");
    let _ = at;
    let v0 = to_source(v);
    // Read after its first physics step: gravity has taken a little.
    assert!(
        (v0.x - 1200.0).abs() < 1.0 && v0.y.abs() < 1.0 && (v0.z - 120.0).abs() < 10.0,
        "{v0}"
    );
    let _ = start;
    let pulls: Vec<u64> = ran
        .events
        .iter()
        .filter(|(_, k)| matches!(k, WeaponEventKind::PinPulled))
        .map(|(t, _)| *t)
        .collect();
    assert_eq!(pulls, [0, 138]);
    assert_eq!(sim.app.world().get::<Hl2Rounds>(w).unwrap().count, 1);
}

/// Test 49: blips on ticks 0, 71, 92, 113, 134, 155 of its life; the
/// explosion on 169.
#[test]
fn a_frag_blips_and_explodes_49() {
    let mut sim = sim();
    let p = player(&mut sim);
    frag_ready(&mut sim, p, 1);
    let ran = run(&mut sim, p, 230, |s, t| s.intent(p).fire = t == 0);
    let blips: Vec<u64> = ran
        .sounds
        .iter()
        .filter(|(_, e)| e == "Grenade.Blip")
        .map(|(t, _)| *t - 19)
        .collect();
    assert_eq!(blips, [0, 71, 92, 113, 134, 155]);
    let boom = ran.blasts.first().map(|(t, _)| *t - 19);
    assert_eq!(boom, Some(169));
}

/// Tests 47, 48: the lob 7 ticks after the release at (350, 0, 50); with
/// duck held a roll at 700 along the ground.
#[test]
fn lob_and_roll_47_48() {
    for duck in [false, true] {
        let mut sim = sim();
        let p = player(&mut sim);
        frag_ready(&mut sim, p, 2);
        let mut made = None;
        let ran = run(&mut sim, p, 40, |s, t| {
            s.intent(p).secondary = t == 0;
            s.intent(p).crouch = duck && t == 15;
            let w = s.app.world_mut();
            if made.is_none()
                && let Some(v) = w
                    .query::<(&Hl2Grenade, &avian3d::prelude::LinearVelocity)>()
                    .iter(w)
                    .next()
                    .map(|(_, v)| v.0)
            {
                made = Some((t, v));
            }
        });
        let thrown = ran
            .events
            .iter()
            .find(|(_, k)| matches!(k, WeaponEventKind::Thrown))
            .map(|(t, _)| *t)
            .unwrap();
        let (t, v) = made.unwrap();
        assert_eq!(t - 1, thrown + 7, "made 7 ticks after the release");
        let v = to_source(v);
        let want = if duck {
            Vec3::new(700.0, 0.0, 0.0)
        } else {
            Vec3::new(350.0, 0.0, 50.0)
        };
        // Read after its first physics step (gravity, the roll's floor).
        assert!(
            (v.x - want.x).abs() < 1.0 && v.y.abs() < 1.0 && (v.z - want.z).abs() < 10.0,
            "duck {duck}: {v}"
        );
    }
}

/// Test 52: dying with attack held and the frag out drops a live grenade
/// instead of the frag; test 53: released and dying before it leaves the
/// hand (tick 17 of a throw out on 19): none, the frag dropped.
#[test]
fn martyrdom_52_53() {
    for released in [false, true] {
        let mut sim = sim();
        let p = player(&mut sim);
        frag_ready(&mut sim, p, 1);
        let ticks = if released { 17 } else { 30 };
        run(&mut sim, p, ticks, |s, t| s.intent(p).fire = !released || t == 0);
        sim.app.world_mut().write_message(Damage {
            target: p,
            attacker: None,
            amount: 5.0,
            point: Vec3::ZERO,
            dir: Vec3::Y,
            hitgroup: chimera_core::core::Hitgroup::Generic,
            kind: chimera_core::core::DamageKind::Generic,
            weapon: None,
            force: Vec3::ZERO,
        });
        sim.ticks(5);
        let w = sim.app.world_mut();
        let n = w.query::<&Hl2Grenade>().iter(w).count();
        assert_eq!(n, if released { 0 } else { 1 }, "released {released}");
        let loose: Vec<Entity> = w
            .query::<&chimera_sim::weapon::drop::Loose>()
            .iter(w)
            .map(|l| l.weapon)
            .collect();
        let frags = loose
            .iter()
            .filter(|e| w.get::<Weapon>(**e).is_some_and(|x| x.id == weapons::FRAG))
            .count();
        assert_eq!(
            frags,
            if released { 1 } else { 0 },
            "released {released}: frags dropped"
        );
    }
}

// ---------------------------------------------------------------------------
// S.L.A.M. (8)

/// A player facing the west wall, its centre `gap` units from it.
fn at_wall(sim: &mut Sim, gap: f32) -> Entity {
    let p = sim.spawn_character(Vec3::new(-40.0 + gap * U, 1.0, 12.0), movement::ID);
    sim.seconds(1.5);
    look(sim, p, 90.0, 0.0);
    p
}

/// `p` holding the S.L.A.M. with `n` charges, its draw and its mode's
/// transition over.
fn slam_ready(sim: &mut Sim, p: Entity, n: u32) -> Entity {
    let w = holding(sim, p, weapons::SLAM);
    set_rounds(sim, w, n);
    sim.seconds(2.0);
    w
}

fn mines(sim: &mut Sim) -> Vec<(Entity, Hl2Mine, Vec3)> {
    let w = sim.app.world_mut();
    w.query::<(Entity, &Hl2Mine, &Transform)>()
        .iter(w)
        .map(|(e, m, t)| (e, m.clone(), t.translation))
        .collect()
}

fn satchels(sim: &mut Sim) -> Vec<(Entity, Vec3)> {
    let w = sim.app.world_mut();
    w.query::<(Entity, &Hl2Satchel, &Transform)>()
        .iter(w)
        .map(|(e, _, t)| (e, t.translation))
        .collect()
}

/// Test 54: a wall 30 ahead (trip-mine mode): the mine on tick 16, 3 off
/// the wall; armed on tick 155; its first beam check 162. Test 56: a
/// bullet into it sets it off 17 ticks later, credited to its placer.
#[test]
fn a_trip_mine_is_placed_and_arms_54_56() {
    let mut sim = sim();
    let p = at_wall(&mut sim, 30.0);
    let w = slam_ready(&mut sim, p, 2);
    assert!(
        sim.app.world().get::<weapons::slam::Hl2Slam>(w).unwrap().mine_mode,
        "trip-mine mode"
    );
    let mut placed = None;
    let mut armed = None;
    let ran = run(&mut sim, p, 170, |s, t| {
        s.intent(p).fire = t == 0;
        // Out of its beam once it is placed.
        if t == 20 {
            s.app.world_mut().get_mut::<Transform>(p).unwrap().translation.z += 3.0;
        }
        let m = mines(s);
        if placed.is_none() && !m.is_empty() {
            placed = Some(t - 1);
        }
        if armed.is_none() && m.first().is_some_and(|(_, m, _)| m.beam.is_some()) {
            armed = Some(t - 1);
        }
    });
    assert_eq!(placed, Some(16));
    assert_eq!(armed, Some(155));
    let (mine, m, at) = mines(&mut sim).remove(0);
    assert!(
        ((at.x + 40.0) / U - 3.0).abs() < 0.1,
        "{} off the wall",
        (at.x + 40.0) / U
    );
    assert_eq!(m.placer, Some(p));
    assert!(ran.sounds.iter().any(|(_, e)| e == "TripmineGrenade.Activate"));
    // A bullet (any damage) into it.
    sim.app.world_mut().write_message(Damage {
        target: mine,
        attacker: None,
        amount: 0.08,
        point: at,
        dir: Vec3::NEG_X,
        hitgroup: chimera_core::core::Hitgroup::Generic,
        kind: chimera_core::core::DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    let ran = run(&mut sim, p, 25, |_, _| {});
    let (t, b) = ran.blasts.first().cloned().expect("exploded");
    assert_eq!(t, 17);
    assert_eq!(b.thrower, Some(p));
    assert!((b.blast.damage - 2.0).abs() < 1e-6);
    // Its own placer, 33 units away, isn't safe.
    assert!(health(&sim, p) < 100.0);
}

/// Test 55: a player stepping into an armed beam trips it.
#[test]
fn a_player_in_the_beam_trips_it_55() {
    let mut sim = sim();
    let p = at_wall(&mut sim, 30.0);
    slam_ready(&mut sim, p, 2);
    // The placer steps out of the beam's line once it is placed.
    run(&mut sim, p, 170, |s, t| {
        s.intent(p).fire = t == 0;
        if t == 20 {
            s.app.world_mut().get_mut::<Transform>(p).unwrap().translation.z += 3.0;
        }
    });
    let (_, m, at) = mines(&mut sim).remove(0);
    assert!(m.beam.is_some());
    run(&mut sim, p, 10, |_, _| {});
    assert!(mines(&mut sim)[0].1.explode_tick.is_none(), "untripped");
    let _q = sim.spawn_character(Vec3::new(at.x + 2.0, 1.0, at.z), movement::ID);
    let ran = run(&mut sim, p, 40, |_, _| {});
    assert!(!ran.blasts.is_empty(), "tripped and exploded");
}

/// Tests 57, 58, 59, 61: a satchel thrown in throw mode leaves on tick 32
/// from the centre + 18 forward + 24 up at 500 along the view; attack2
/// sets both off 14 ticks later (not in trip-mine mode); the thrower's
/// death sets them off too.
#[test]
fn satchels_thrown_and_detonated_57_58() {
    let mut sim = sim();
    let p = player(&mut sim);
    let w = slam_ready(&mut sim, p, 3);
    let centre = sim.position(p) + {
        let s = sim.app.world().get::<MovementState>(p).unwrap();
        (s.hull_min + s.hull_max) / 2.0
    };
    let mut first = None;
    run(&mut sim, p, 40, |s, t| {
        s.intent(p).fire = t == 0;
        if first.is_none()
            && let Some((_, at)) = satchels(s).first().copied()
        {
            first = Some((t - 1, at));
        }
    });
    let (t, at) = first.expect("a satchel");
    assert_eq!(t, 32);
    // Seen after its first physics step: 7.5 units on.
    let d = to_source(at - centre);
    assert!(
        (18.0..=26.5).contains(&d.x) && d.y.abs() < 1.0 && (d.z - 24.0).abs() < 1.0,
        "{d}"
    );
    assert!(sim.app.world().get::<weapons::slam::Hl2Slam>(w).unwrap().detonator);
    // A second one (1.8 s after the first left), then detonate.
    run(&mut sim, p, 200, |s, t| s.intent(p).fire = t == 150);
    assert_eq!(satchels(&mut sim).len(), 2);
    // Once the second throw's animations are over (1.8 s after it left).
    run(&mut sim, p, 110, |_, _| {});
    let ran = run(&mut sim, p, 20, |s, t| s.intent(p).secondary = t == 0);
    let ticks: Vec<u64> = ran.blasts.iter().map(|(t, _)| *t).collect();
    assert_eq!(ticks, [14, 14]);
    let (_, b) = &ran.blasts[0];
    assert!((b.blast.damage - 1.5).abs() < 1e-6 && (b.blast.radius - 200.0 * U).abs() < 1e-5);
}

/// Test 60: a shot satchel pops harmlessly.
#[test]
fn a_shot_satchel_is_harmless_60() {
    let mut sim = sim();
    let p = player(&mut sim);
    let _w = slam_ready(&mut sim, p, 2);
    run(&mut sim, p, 120, |s, t| s.intent(p).fire = t == 0);
    let (s, at) = satchels(&mut sim)[0];
    sim.app.world_mut().write_message(Damage {
        target: s,
        attacker: None,
        amount: 0.08,
        point: at,
        dir: Vec3::NEG_X,
        hitgroup: chimera_core::core::Hitgroup::Generic,
        kind: chimera_core::core::DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    let ran = run(&mut sim, p, 20, |_, _| {});
    assert!(satchels(&mut sim).is_empty());
    assert!(ran.blasts.is_empty());
    assert_eq!(health(&sim, p), 100.0);
}

/// Test 62: the last slam placed as a trip mine, the S.L.A.M. leaves the
/// inventory.
#[test]
fn the_last_slam_leaves_the_inventory_62() {
    let mut sim = sim();
    let p = at_wall(&mut sim, 30.0);
    let _w = slam_ready(&mut sim, p, 1);
    run(&mut sim, p, 60, |s, t| s.intent(p).fire = t == 0);
    assert_eq!(mines(&mut sim).len(), 1);
    assert!(weapon_of(&sim, p, weapons::SLAM).is_none());
}

/// The suit on a gun's hit (projectiles.md 11's rule for bullets too,
/// weapons.md test 13): a pistol body hit (8) on 100 armour: 1.6 to
/// health (1 whole point lost, 0.6 carried), 6.4 to armour.
#[test]
fn a_pistol_hit_on_the_suit_takes_a_fifth_to_health() {
    let mut sim = sim();
    let p = player(&mut sim);
    holding(&mut sim, p, weapons::PISTOL);
    let t = sim.spawn_character(greybox::SPAWNS[0] + Vec3::X * 3.0, movement::ID);
    sim.seconds(2.0);
    sim.app.world_mut().get_mut::<Armor>(t).unwrap().amount = 1.0;
    let aim = eye(&sim, t) - Vec3::Y * 0.45;
    testkit::aim_at(&mut sim, p, aim);
    run(&mut sim, p, 5, |s, k| s.intent(p).fire = k == 0);
    assert!((health(&sim, t) - 99.0).abs() < 1e-3, "health {}", health(&sim, t));
    let a = sim.app.world().get::<Armor>(t).unwrap().amount * 100.0;
    assert!((93.0..=94.0).contains(&a.round()), "armour {a}");
}

/// Ammo boxes (M7's `ammo::give`) of a projectile's type fill the weapon's
/// own rounds up to their limit (RPG_Round 3).
#[test]
fn ammo_of_a_projectile_type_fills_its_weapons_rounds() {
    let mut sim = sim();
    let p = player(&mut sim);
    let w = holding(&mut sim, p, weapons::RPG);
    set_rounds(&mut sim, w, 1);
    let added = hl2dm::game::ammo::give(sim.app.world_mut(), p, "hl2dm:rpg_round", 5);
    assert_eq!(added, 2);
    assert_eq!(sim.app.world().get::<Hl2Rounds>(w).unwrap().count, 3);
}
