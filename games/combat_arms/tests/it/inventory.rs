//! specs/combat_arms/inventory.md's test cases, one test each (`t01_` is
//! test case 1), on `CaSim` and the greybox with the spec's stock loadout:
//! the M16A3 and M92FS as fixtures (tests/data/: the spec's clip, total,
//! draw, reload and post-sprint times; fire rate and damage ours), the
//! game's M9 (data/weapons/m9.toml) and an M67 on Chimera's
//! `grenade::Throwable` (400 ms draw, hide-when-empty).
//!
//! Times count from the tick that first sees an input (t = 0); the sim
//! ticks at the game's tick (15 ms), the spec's frames are 60 fps, and "+-f" tolerances are
//! the spec's frames (1/60 s).

use crate::common::{active, active_id};
use bevy::{ecs::system::RunSystemOnce, prelude::*};
use chimera_core::core::{Damage, DamageKind, Health, Hitgroup, MovementState, SimTick};
use chimera_sim::weapon::{
    Inventory, Magazine, RegisterWeapons, StartingWeapons, Weapon, WeaponEvent, WeaponEventKind,
    data::WeaponDataAppExt,
    grenade::{Flight, GrenadeEffect, Smoke, ThrowRule, Throwable},
};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
    map::loose::LooseItem,
};
use combat_arms::{
    game::{
        inventory::{self, CaDeathDrops, CaItem, CaSelection, Dropped, Knife, KnifePhase, UNIT},
        movement,
    },
    harness::CaSim,
};

const RIFLE: &str = "combat_arms:test_m16a3";
const PISTOL: &str = "combat_arms:test_m92fs";
const GRENADE: &str = "combat_arms:test_m67";
const KNIFE: &str = inventory::KNIFE;
/// The stock loadout (Test cases: common setup), in loadout order.
const STOCK: &[&str] = &[RIFLE, PISTOL, KNIFE, GRENADE];
/// One of the spec's frames (60 fps).
const F: f64 = 1.0 / 60.0;
/// Frames the knife's timing cases (16-25) leave out: their times count
/// the main swing from the pre-fire's end, but it starts at the update
/// after that (8.2 as answered, case 37), a 15 ms tick here.
const SWING_START: f64 = 1.0;

/// What weapons did, by tick.
#[derive(Resource, Default)]
struct Log(Vec<(u64, Entity, WeaponEventKind)>);

fn log_events(mut events: MessageReader<WeaponEvent>, tick: Res<SimTick>, mut log: ResMut<Log>) {
    for e in events.read() {
        log.0.push((tick.0, e.owner, e.kind.clone()));
    }
}

/// Damage dealt, by tick: (tick, target, points).
#[derive(Resource, Default)]
struct Hurt(Vec<(u64, Entity, f32)>);

fn log_damage(mut damage: MessageReader<Damage>, tick: Res<SimTick>, mut log: ResMut<Hurt>) {
    for d in damage.read() {
        log.0.push((tick.0, d.target, d.amount / 0.01));
    }
}

/// A second throwable, to be a different item in class 4.
const OTHER_GRENADE: &str = "combat_arms:test_xm84";

fn other_grenade(e: &mut EntityWorldMut) {
    grenade(e);
    e.get_mut::<Weapon>().unwrap().id = OTHER_GRENADE;
}

/// The M67 on Chimera's throwable part: 700 ms pin (pre-fire), the throw
/// 67 ms after the release, 433 ms of throw animation (67 + 366), drawn
/// in 400 ms; it hides when empty (inventory.md Constants).
fn grenade(e: &mut EntityWorldMut) {
    e.insert((
        Weapon {
            id: GRENADE,
            slot: 3,
            owner: None,
            draw_time: 0.4,
            max_speed: None,
        },
        CaItem {
            hide_when_empty: true,
            sprint_out: 0.267,
            ..default()
        },
        Throwable {
            count: 1,
            max: 1,
            pin_time: 0.7,
            throw_delay: 0.067,
            throw_time: 0.433,
            redraw_time: 0.4,
            fuse: 1.5,
            throw: ThrowRule {
                pitch_lift: 0.0,
                pitch_scale: 1.0,
                pitch_scale_up: 1.0,
                speed_per_deg: 0.1,
                speed_max: 10.0,
                forward: 0.2,
            },
            flight: Flight {
                half: 0.05,
                gravity: 9.8,
                elasticity: 0.45,
                player_elasticity: 0.3,
                elasticity_cap: 0.9,
                stop_speed: 0.5,
                floor_normal: 0.7,
                breakable_damage: 0.0,
                breakable_slowdown: 1.0,
                water_slowdown: 1.0,
                water_entry: 1.0,
                max_velocity: 50.0,
                check_interval: 0.1,
                spin_roll: 0.0,
                spin_pitch: 0.0,
            },
            effect: GrenadeEffect::Smoke(Smoke {
                max_radius: 1.0,
                expand_time: 1.0,
                fade_start: 2.0,
                fade_end: 3.0,
                pop_speed: 0.1,
                sight_radius: 0.75,
                sound: None,
            }),
            pin_sounds: Vec::new(),
            bounce_sound: None,
            water_sounds: None,
            pin: false,
            throw_at: None,
            redraw_at: None,
        },
    ));
}

fn sim_with(loadout: &[&'static str]) -> Sim {
    let loadout = loadout.to_vec();
    Sim::with(CaSim, move |app| {
        app.add_plugins(GreyboxMapPlugin);
        app.add_weapon_data(&[
            ("test_m16a3.toml", include_str!("../data/test_m16a3.toml")),
            ("test_m92fs.toml", include_str!("../data/test_m92fs.toml")),
            ("test_hud_rifle.toml", include_str!("../data/test_hud_rifle.toml")),
        ])
        .unwrap();
        app.register_weapon(GRENADE, grenade);
        app.register_weapon(OTHER_GRENADE, other_grenade);
        app.insert_resource(StartingWeapons {
            team: Vec::new(),
            all: loadout,
        });
        app.init_resource::<Log>()
            .init_resource::<Hurt>()
            .add_systems(FixedPostUpdate, (log_events, log_damage));
    })
}

/// A player with `loadout`, settled with its first draw done.
fn setup(loadout: &[&'static str]) -> (Sim, Entity) {
    let mut sim = sim_with(loadout);
    let p = player(&mut sim, greybox::SPAWNS[0]);
    (sim, p)
}

fn player(sim: &mut Sim, at: Vec3) -> Entity {
    let p = sim.spawn_character(at, movement::ID);
    sim.seconds(1.5);
    p
}

fn inv(sim: &Sim, p: Entity) -> Inventory {
    sim.app.world().get::<Inventory>(p).unwrap().clone()
}

fn sel(sim: &Sim, p: Entity) -> CaSelection {
    sim.app.world().get::<CaSelection>(p).unwrap().clone()
}

fn id(sim: &Sim, w: Entity) -> &'static str {
    sim.app.world().get::<Weapon>(w).unwrap().id
}

/// The first weapon `p` carries with this id.
fn carried(sim: &Sim, p: Entity, weapon: &str) -> Entity {
    inv(sim, p).weapons.into_iter().find(|w| id(sim, *w) == weapon).unwrap()
}

/// (clip, total) of a magazine weapon.
fn ammo(sim: &Sim, w: Entity) -> (u32, u32) {
    let m = sim.app.world().get::<Magazine>(w).unwrap();
    (m.clip, m.clip + m.reserve)
}

fn set_ammo(sim: &mut Sim, w: Entity, clip: u32, total: u32) {
    let mut m = sim.app.world_mut().get_mut::<Magazine>(w).unwrap();
    m.clip = clip;
    m.reserve = total - clip;
}

/// The tick that will see the next input: t = 0.
fn mark(sim: &Sim) -> u64 {
    sim.tick() + 1
}

fn dt(sim: &Sim) -> f64 {
    1.0 / sim.tick_hz()
}

/// The time of the last tick run, from `t0`.
fn now(sim: &Sim, t0: u64) -> f64 {
    (sim.tick() as f64 - t0 as f64) * dt(sim)
}

/// The time of `tick` from `t0`.
fn time_of(sim: &Sim, t0: u64, tick: u64) -> f64 {
    (tick as f64 - t0 as f64) * dt(sim)
}

/// Run until the next tick is the one at time `t` (so an input set now
/// is seen at `t`).
fn wait_until(sim: &mut Sim, t0: u64, t: f64) {
    let target = t0 + (t / dt(sim)).round() as u64;
    while sim.tick() + 1 < target {
        sim.ticks(1);
    }
}

/// Run tick by tick until `cond`; its time from `t0`.
fn until(sim: &mut Sim, t0: u64, limit: f64, cond: impl Fn(&Sim) -> bool) -> f64 {
    loop {
        sim.ticks(1);
        if cond(sim) {
            return now(sim, t0);
        }
        assert!(now(sim, t0) < limit, "nothing by {limit} s");
    }
}

/// Press class key `class` (1-6) for one tick.
/// The key is let go after its tick; the release is seen with the next
/// tick (so two presses in a row are two presses).
fn key(sim: &mut Sim, p: Entity, class: u8) {
    sim.intent(p).select = Some(class - 1);
    sim.ticks(1);
    sim.intent(p).select = None;
    release(sim, p);
}

/// Let the next tick see the release before another press.
fn release(sim: &mut Sim, p: Entity) {
    let _ = p;
    sim.ticks(1);
}

fn q(sim: &mut Sim, p: Entity) {
    sim.intent(p).last_weapon = true;
    sim.ticks(1);
    sim.intent(p).last_weapon = false;
    release(sim, p);
}

fn tap(sim: &mut Sim, p: Entity, f: impl Fn(&mut chimera_core::core::Intent, bool)) {
    f(&mut sim.intent(p), true);
    sim.ticks(1);
    f(&mut sim.intent(p), false);
}

/// Ticks at which `p`'s weapons logged events matching `pick`.
fn logged(sim: &Sim, p: Entity, pick: impl Fn(&WeaponEventKind) -> bool) -> Vec<u64> {
    sim.app
        .world()
        .resource::<Log>()
        .0
        .iter()
        .filter(|(_, o, k)| *o == p && pick(k))
        .map(|(t, ..)| *t)
        .collect()
}

fn swings(sim: &Sim, p: Entity) -> Vec<u64> {
    logged(sim, p, |k| matches!(k, WeaponEventKind::Swing { .. }))
}

fn knife_phase(sim: &Sim, p: Entity) -> KnifePhase {
    let k = carried(sim, p, KNIFE);
    sim.app.world().get::<Knife>(k).unwrap().state.phase
}

fn close(t: f64, want: f64, frames: f64) -> bool {
    (t - want).abs() <= frames * F + 1e-6
}

/// Hold fire until the active weapon's clip drops: the first shot's time.
fn first_shot(sim: &mut Sim, p: Entity, t0: u64, w: Entity) -> f64 {
    let before = ammo(sim, w).0;
    sim.intent(p).fire = true;
    until(sim, t0, 6.0, |s| ammo(s, w).0 < before)
}

// ---------------------------------------------------------------------------
// Draw times (1-5)

#[test]
fn t01_draw_rifle() {
    let (mut sim, p) = setup(STOCK);
    key(&mut sim, p, 2);
    sim.seconds(1.0);
    let pistol = carried(&sim, p, PISTOL);
    let pistol_ammo = ammo(&sim, pistol);
    let t0 = mark(&sim);
    sim.intent(p).fire = true;
    key(&mut sim, p, 1);
    assert_eq!(active_id(&sim, p), RIFLE, "in hand at t = 0");
    let rifle = carried(&sim, p, RIFLE);
    let t = first_shot(&mut sim, p, t0, rifle);
    // As corrected (Corrections 1): 3 updates after the draw's end.
    assert!(close(t, 1.25, 2.0), "first shot at {t}");
    assert_eq!(ammo(&sim, pistol), pistol_ammo);
}

#[test]
fn t02_draw_pistol() {
    let (mut sim, p) = setup(STOCK);
    let t0 = mark(&sim);
    key(&mut sim, p, 2);
    assert_eq!(active_id(&sim, p), PISTOL);
    let pistol = carried(&sim, p, PISTOL);
    let t = first_shot(&mut sim, p, t0, pistol);
    // As corrected: 700 ms.
    assert!(close(t, 0.7, 2.0), "ready at {t}");
}

#[test]
fn t03_draw_knife() {
    let (mut sim, p) = setup(STOCK);
    let t0 = mark(&sim);
    sim.intent(p).fire = true;
    key(&mut sim, p, 3);
    assert_eq!(active_id(&sim, p), KNIFE);
    let t = until(&mut sim, t0, 2.0, |s| !swings(s, p).is_empty());
    assert!(close(t, 0.45 + 0.155, 3.0), "first hit at {t}");
}

#[test]
fn t04_draw_grenade() {
    let (mut sim, p) = setup(STOCK);
    let t0 = mark(&sim);
    sim.intent(p).fire = true;
    key(&mut sim, p, 4);
    assert_eq!(active_id(&sim, p), GRENADE);
    let t = until(&mut sim, t0, 2.0, |s| {
        !logged(s, p, |k| matches!(k, WeaponEventKind::PinPulled)).is_empty()
    });
    assert!(close(t, 0.4, 2.0), "fire accepted at {t}");
}

#[test]
fn t05_no_redraw() {
    let (mut sim, p) = setup(STOCK);
    key(&mut sim, p, 2);
    sim.seconds(1.0);
    let t0 = mark(&sim);
    key(&mut sim, p, 1);
    let rifle = active(&sim, p).unwrap();
    wait_until(&mut sim, t0, 0.5);
    key(&mut sim, p, 1);
    assert_eq!(active(&sim, p), Some(rifle), "nothing changes");
    let t = first_shot(&mut sim, p, t0, rifle);
    // Ready 3 updates after the draw (Corrections 1).
    assert!(close(t, 1.25, 2.0), "ready at {t}");
}

// ---------------------------------------------------------------------------
// Class keys and Q (6-10)

#[test]
fn t06_cycle_primaries() {
    let (mut sim, p) = setup(&[RIFLE, PISTOL, KNIFE, GRENADE, RIFLE]);
    let weapons = inv(&sim, p).weapons;
    let (a, b) = (weapons[0], weapons[4]);
    assert_eq!(active(&sim, p), Some(a), "A drawn at spawn");
    key(&mut sim, p, 1);
    assert_eq!(active(&sim, p), Some(b));
    key(&mut sim, p, 1);
    assert_eq!(active(&sim, p), Some(a));
    key(&mut sim, p, 2);
    assert_eq!(active_id(&sim, p), PISTOL);
    key(&mut sim, p, 1);
    assert_eq!(active(&sim, p), Some(a), "the last primary used");
}

#[test]
fn t07_q_toggle() {
    let (mut sim, p) = setup(STOCK);
    let rifle = active(&sim, p).unwrap();
    q(&mut sim, p);
    assert_eq!(active(&sim, p), Some(rifle), "previous empty: nothing");
    key(&mut sim, p, 2);
    q(&mut sim, p);
    assert_eq!(active_id(&sim, p), RIFLE);
    q(&mut sim, p);
    assert_eq!(active_id(&sim, p), PISTOL);
    q(&mut sim, p);
    assert_eq!(active_id(&sim, p), RIFLE);
}

/// Throw the grenade in hand: hold fire past the pin, then let go.
fn throw(sim: &mut Sim, p: Entity) {
    sim.intent(p).fire = true;
    sim.seconds(1.0);
    sim.intent(p).fire = false;
    sim.ticks(1);
    let g = carried(sim, p, GRENADE);
    assert_eq!(sim.app.world().get::<Throwable>(g).unwrap().count, 0, "thrown");
}

#[test]
fn t08_q_after_the_last_grenade() {
    let (mut sim, p) = setup(STOCK);
    key(&mut sim, p, 4);
    sim.seconds(0.5);
    throw(&mut sim, p);
    let t0 = mark(&sim);
    until(&mut sim, t0, 2.0, |s| active(s, p).is_some_and(|a| id(s, a) == RIFLE));
    assert_eq!(sel(&sim, p).previous, None, "Q cleared by the auto-switch");
    q(&mut sim, p);
    sim.ticks(2);
    assert_eq!(active_id(&sim, p), RIFLE, "Q does nothing");
}

#[test]
fn t09_dead_q() {
    let (mut sim, p) = setup(STOCK);
    key(&mut sim, p, 4);
    sim.seconds(0.5);
    throw(&mut sim, p);
    let g = carried(&sim, p, GRENADE);
    // Its ammo used, the throw animation still playing.
    assert!(sim.app.world().get::<Throwable>(g).unwrap().redraw_at.is_some());
    key(&mut sim, p, 2);
    assert_eq!(active_id(&sim, p), PISTOL, "no auto-switch: the grenade isn't in hand");
    assert_eq!(sel(&sim, p).previous, Some(g), "previous: the spent M67");
    let pistol = active(&sim, p).unwrap();
    q(&mut sim, p);
    assert_eq!(active(&sim, p), Some(pistol), "Q: nothing");
    assert_eq!(sel(&sim, p).previous, Some(pistol), "Q overwrote previous");
    q(&mut sim, p);
    assert_eq!(active(&sim, p), Some(pistol), "still nothing");
    key(&mut sim, p, 1);
    assert_eq!(active_id(&sim, p), RIFLE);
    q(&mut sim, p);
    assert_eq!(active(&sim, p), Some(pistol));
}

#[test]
fn t10_empty_grenade_key() {
    let (mut sim, p) = setup(STOCK);
    let g = carried(&sim, p, GRENADE);
    sim.app.world_mut().get_mut::<Throwable>(g).unwrap().count = 0;
    key(&mut sim, p, 4);
    sim.ticks(1);
    assert_eq!(active_id(&sim, p), RIFLE, "nothing happens");
}

// ---------------------------------------------------------------------------
// What interrupts what (11-16)

#[test]
fn t11_switch_mid_reload() {
    let (mut sim, p) = setup(STOCK);
    let rifle = carried(&sim, p, RIFLE);
    set_ammo(&mut sim, rifle, 10, 100);
    let t0 = mark(&sim);
    tap(&mut sim, p, |i, on| i.reload = on);
    wait_until(&mut sim, t0, 1.0);
    key(&mut sim, p, 2);
    wait_until(&mut sim, t0, 2.0);
    key(&mut sim, p, 1);
    assert_eq!(ammo(&sim, rifle), (10, 100), "the reload never completed");
    let t = first_shot(&mut sim, p, t0, rifle);
    // As corrected: 3250 ms.
    assert!(close(t, 3.25, 2.0), "ready at {t}");
}

#[test]
fn t12_switch_to_an_empty_clip() {
    let (mut sim, p) = setup(STOCK);
    let rifle = carried(&sim, p, RIFLE);
    set_ammo(&mut sim, rifle, 0, 90);
    key(&mut sim, p, 2);
    sim.seconds(1.0);
    let t0 = mark(&sim);
    key(&mut sim, p, 1);
    assert_eq!(
        logged(&sim, p, |k| matches!(k, WeaponEventKind::ReloadStarted)),
        vec![t0],
        "the reload starts at once"
    );
    sim.intent(p).fire = true;
    let t = until(&mut sim, t0, 4.0, |s| ammo(s, rifle).0 > 0);
    assert!(close(t, 2.3, 2.0), "reloaded at {t}");
    assert_eq!(ammo(&sim, rifle), (30, 90));
    // As corrected: ready (fire held) at 2350 ms.
    let shot = until(&mut sim, t0, 4.0, |s| ammo(s, rifle).0 < 30);
    assert!(close(shot, 2.35, 2.0), "ready at {shot}");
}

#[test]
fn t13_reload_cancels_the_draw() {
    let (mut sim, p) = setup(STOCK);
    let rifle = carried(&sim, p, RIFLE);
    set_ammo(&mut sim, rifle, 20, 110);
    key(&mut sim, p, 2);
    sim.seconds(1.0);
    let t0 = mark(&sim);
    key(&mut sim, p, 1);
    wait_until(&mut sim, t0, 0.1);
    tap(&mut sim, p, |i, on| i.reload = on);
    let started = logged(&sim, p, |k| matches!(k, WeaponEventKind::ReloadStarted));
    assert_eq!(started.len(), 1);
    assert!(close(time_of(&sim, t0, started[0]), 0.1, 1.0));
    sim.intent(p).fire = true;
    let t = until(&mut sim, t0, 4.0, |s| ammo(s, rifle).0 == 30);
    assert!(close(t, 2.4, 2.0), "reloaded at {t}");
    assert_eq!(ammo(&sim, rifle), (30, 110));
    // As corrected: ready (fire held) at 2450 ms.
    let shot = until(&mut sim, t0, 4.0, |s| ammo(s, rifle).0 < 30);
    assert!(close(shot, 2.45, 2.0), "ready at {shot}");
}

#[test]
fn t14_fire_during_reload() {
    let (mut sim, p) = setup(STOCK);
    let rifle = carried(&sim, p, RIFLE);
    set_ammo(&mut sim, rifle, 5, 65);
    let t0 = mark(&sim);
    tap(&mut sim, p, |i, on| i.reload = on);
    wait_until(&mut sim, t0, 0.5);
    sim.intent(p).fire = true;
    let t = until(&mut sim, t0, 4.0, |s| ammo(s, rifle).0 != 5);
    assert!(close(t, 2.3, 1.0), "reloaded at {t}");
    assert_eq!(ammo(&sim, rifle), (30, 65));
    // As corrected: the first shot on the 3rd update after the one that
    // completes the reload, 2350 ms.
    let t = until(&mut sim, t0, 4.0, |s| ammo(s, rifle).0 != 30);
    assert!(close(t, 2.35, 1.0), "first shot at {t}");
    assert_eq!(ammo(&sim, rifle), (29, 64));
}

/// Sprint forward (the placeholder movement's sprint needs both).
fn sprint(sim: &mut Sim, p: Entity, on: bool) {
    let mut i = sim.intent(p);
    i.sprint = on;
    i.move_axis = if on { Vec2::Y } else { Vec2::ZERO };
}

#[test]
fn t15_sprint_during_reload() {
    let (mut sim, p) = setup(STOCK);
    let rifle = carried(&sim, p, RIFLE);
    set_ammo(&mut sim, rifle, 5, 65);
    let t0 = mark(&sim);
    tap(&mut sim, p, |i, on| i.reload = on);
    wait_until(&mut sim, t0, 1.0);
    sprint(&mut sim, p, true);
    sim.ticks(3);
    assert!(sim.state(p).sprinting, "sprinting");
    assert!(!sel(&sim, p).sprint_pose, "the reload goes on");
    let t = until(&mut sim, t0, 4.0, |s| ammo(s, rifle).0 == 30);
    assert!(close(t, 2.3, 1.0), "reloaded at {t}");
    assert_eq!(ammo(&sim, rifle), (30, 65));
    assert!(sel(&sim, p).sprint_pose, "then the sprint pose");
}

/// Sprint until the weapon in hand is in the sprint pose, then release
/// sprint holding fire: the time of the first shot (or knife hit).
fn leave_sprint(sim: &mut Sim, p: Entity, shot: impl Fn(&Sim) -> bool) -> f64 {
    sprint(sim, p, true);
    sim.seconds(0.5);
    assert!(sel(sim, p).sprint_pose, "in the sprint pose");
    sim.intent(p).fire = true;
    sim.ticks(2);
    assert!(!shot(sim), "fire ignored while sprinting");
    let t0 = mark(sim);
    sprint(sim, p, false);
    until(sim, t0, 2.0, shot)
}

#[test]
fn t16_leaving_sprint() {
    let (mut sim, p) = setup(STOCK);
    let rifle = carried(&sim, p, RIFLE);
    let t = leave_sprint(&mut sim, p, |s| ammo(s, rifle).0 < 30);
    // As corrected: 367 + 66.7 (4 updates) ms.
    assert!(close(t, 0.367 + 4.0 * F, 2.0), "rifle: first shot at {t}");

    let (mut sim, p) = setup(STOCK);
    key(&mut sim, p, 2);
    sim.seconds(1.0);
    let pistol = carried(&sim, p, PISTOL);
    let t = leave_sprint(&mut sim, p, |s| ammo(s, pistol).0 < 15);
    assert!(close(t, 0.4 + 4.0 * F, 2.0), "pistol: first shot at {t}");

    let (mut sim, p) = setup(STOCK);
    key(&mut sim, p, 3);
    sim.seconds(1.0);
    let t = leave_sprint(&mut sim, p, |s| !swings(s, p).is_empty());
    // No pre-fire (Corrections (2026-10-30) 1): the swing starts on the
    // update after the one that sets "firing".
    assert!(close(t, 0.167 + 0.155, 3.0 + SWING_START), "knife: first hit at {t}");
}

// ---------------------------------------------------------------------------
// The knife (17-25)

fn knife_out(loadout: &[&'static str]) -> (Sim, Entity) {
    let (mut sim, p) = setup(loadout);
    key(&mut sim, p, 3);
    sim.seconds(1.0);
    assert_eq!(knife_phase(&sim, p), KnifePhase::Idle);
    (sim, p)
}

#[test]
fn t17_knife_primary_timing() {
    let (mut sim, p) = knife_out(STOCK);
    let t0 = mark(&sim);
    sim.intent(p).fire = true;
    wait_until(&mut sim, t0, 0.7);
    sim.intent(p).fire = false;
    let idle = until(&mut sim, t0, 3.0, |s| knife_phase(s, p) == KnifePhase::Idle);
    let hits: Vec<f64> = swings(&sim, p).into_iter().map(|t| time_of(&sim, t0, t)).collect();
    assert_eq!(hits.len(), 4, "hits at {hits:?}");
    for (t, want) in hits.iter().zip([0.156, 0.403, 0.806, 1.053]) {
        assert!(close(*t, want, 3.0), "hit at {t}, want {want}: {hits:?}");
    }
    assert!(close(idle, 1.302, 3.0), "idle at {idle}");
}

#[test]
fn t18_knife_secondary_timing() {
    let (mut sim, p) = knife_out(STOCK);
    let t0 = mark(&sim);
    tap(&mut sim, p, |i, on| i.secondary = on);
    let t = until(&mut sim, t0, 2.0, |s| !swings(s, p).is_empty());
    assert!(close(t, 0.368, 3.0), "hit at {t}");
    // Not again until the stab ends: a click at 1 s does nothing.
    wait_until(&mut sim, t0, 1.0);
    tap(&mut sim, p, |i, on| i.secondary = on);
    let idle = until(&mut sim, t0, 3.0, |s| knife_phase(s, p) == KnifePhase::Idle);
    assert!(close(idle, 1.101, 3.0 + SWING_START), "fire possible again at {idle}");
    sim.seconds(1.0);
    assert_eq!(swings(&sim, p).len(), 1, "the second click was ignored");
}

#[test]
fn t19_both_buttons() {
    let (mut sim, p) = knife_out(STOCK);
    let t0 = mark(&sim);
    sim.intent(p).fire = true;
    sim.intent(p).secondary = true;
    let t = until(&mut sim, t0, 2.0, |s| !swings(s, p).is_empty());
    assert!(close(t, 0.156, 3.0), "hit at {t}");
    let k = carried(&sim, p, KNIFE);
    assert_eq!(
        sim.app.world().get::<Knife>(k).unwrap().state.attack,
        0,
        "a primary swing"
    );
    let secondary = logged(&sim, p, |k| matches!(k, WeaponEventKind::Swing { secondary: true, .. }));
    assert!(secondary.is_empty(), "not a stab");
}

/// A knife player looking down -Z and a target facing `yaw` whose
/// surface, at the attacker's eye height, is `surface` metres ahead of
/// the eye (health to spare: hits are read from the damage dealt).
fn knife_target(surface: f32, yaw: f32) -> (Sim, Entity, Entity) {
    let (mut sim, p) = knife_out(STOCK);
    // Where the knife's ray (from the eye, straight ahead) meets the
    // target's body, measured with the target 2 m ahead: then the target
    // is moved so that surface lies `surface` ahead.
    let probe = 2.0;
    let target = sim.spawn_character(sim.position(p) - Vec3::Z * probe, movement::ID);
    sim.intent(target).yaw = yaw;
    sim.app.world_mut().get_mut::<Health>(target).unwrap().current = 10.0;
    sim.seconds(0.5);
    let eye = sim.position(p) + sim.state(p).eye_offset;
    let hit = sim
        .app
        .world_mut()
        .run_system_once(move |q: avian3d::prelude::SpatialQuery| {
            let filter = avian3d::prelude::SpatialQueryFilter::from_excluded_entities([p]);
            q.cast_ray(eye, Dir3::NEG_Z, 10.0, true, &filter)
                .map(|h| (h.entity, h.distance))
        })
        .unwrap()
        .expect("the ray meets the target");
    assert_eq!(hit.0, target);
    let depth = (eye.z - sim.position(target).z) - hit.1;
    let at = sim.position(target).with_z(eye.z - surface - depth);
    sim.app.world_mut().get_mut::<Transform>(target).unwrap().translation = at;
    sim.seconds(0.5);
    (sim, p, target)
}

fn health(sim: &Sim, e: Entity) -> f32 {
    sim.app.world().get::<Health>(e).unwrap().current
}

/// One attack (`secondary` or not) on the target: the damage dealt, in
/// points.
fn one_hit(sim: &mut Sim, p: Entity, target: Entity, secondary: bool) -> f32 {
    let n = swings(sim, p).len();
    let from = sim.tick();
    if secondary {
        tap(sim, p, |i, on| i.secondary = on);
    } else {
        tap(sim, p, |i, on| i.fire = on);
    }
    let t0 = mark(sim);
    until(sim, t0, 2.0, |s| swings(s, p).len() > n);
    sim.ticks(1);
    let hurt = &sim.app.world().resource::<Hurt>().0;
    let hits: Vec<f32> = hurt
        .iter()
        .filter(|(t, e, _)| *t > from && *e == target)
        .map(|h| h.2)
        .collect();
    assert!(hits.len() <= 1, "one hit: {hits:?}");
    // Let the attack end before the next.
    sim.seconds(1.5);
    hits.first().copied().unwrap_or(0.0)
}

fn in_range(d: f32, lo: f32, hi: f32) -> bool {
    d >= lo - 1e-3 && d <= hi + 1e-3
}

#[test]
fn t20_knife_damage_front() {
    // Facing the attacker: yaw pi looks down +Z.
    let (mut sim, p, target) = knife_target(50.0 * UNIT, std::f32::consts::PI);
    let d = one_hit(&mut sim, p, target, false);
    assert!(in_range(d, 40.5, 49.5), "primary hit {d}");
    let d = one_hit(&mut sim, p, target, true);
    assert!(in_range(d, 67.5, 82.5), "secondary {d}");
}

#[test]
fn t21_knife_damage_behind() {
    // Facing away (down -Z, as the attacker).
    let (mut sim, p, target) = knife_target(50.0 * UNIT, 0.0);
    let d = one_hit(&mut sim, p, target, false);
    assert!(in_range(d, 52.65, 64.35), "primary hit {d}");
    let d = one_hit(&mut sim, p, target, true);
    assert!(in_range(d, 87.75, 107.25), "secondary {d}");
}

#[test]
fn t22_sector_edges() {
    let (f, r) = (Vec3::NEG_Z, Vec3::X);
    let d = |deg: f32| {
        let a = deg.to_radians();
        a.cos() * r + a.sin() * f
    };
    assert!(!inventory::behind(f, r, d(29.0)), "29: not behind");
    assert!(inventory::behind(f, r, d(31.0)), "31: behind");
    assert!(inventory::behind(f, r, d(179.0)), "179: behind");
    assert!(!inventory::behind(f, r, d(180.0)), "180: not behind");
    assert!(!inventory::behind(f, r, d(-30.0)), "-30: not behind");
}

#[test]
fn t23_knife_range() {
    for (surface, hits) in [(104.0, true), (106.0, false)] {
        let (mut sim, p, target) = knife_target(surface * UNIT, 0.0);
        let d = one_hit(&mut sim, p, target, false);
        assert_eq!(d > 0.0, hits, "surface at {surface} units: {d} damage");
    }
    // A wall at 60 units stops the ray before a target behind it.
    let (mut sim, p, target) = knife_target(90.0 * UNIT, 0.0);
    let eye = sim.position(p) + sim.state(p).eye_offset;
    sim.app.world_mut().spawn((
        avian3d::prelude::RigidBody::Static,
        avian3d::prelude::Collider::cuboid(2.0, 3.0, 0.02),
        Transform::from_translation(eye - Vec3::Z * (60.0 * UNIT + 0.01)),
    ));
    sim.ticks(2);
    let d = one_hit(&mut sim, p, target, false);
    assert_eq!(d, 0.0, "the wall took the hit");
}

#[test]
fn t24_swing_cut_by_a_switch() {
    let (mut sim, p) = knife_out(STOCK);
    let t0 = mark(&sim);
    tap(&mut sim, p, |i, on| i.fire = on);
    wait_until(&mut sim, t0, 0.25);
    key(&mut sim, p, 2);
    let pistol = carried(&sim, p, PISTOL);
    let t = first_shot(&mut sim, p, t0, pistol);
    let hits: Vec<f64> = swings(&sim, p).into_iter().map(|t| time_of(&sim, t0, t)).collect();
    assert_eq!(hits.len(), 1, "exactly one hit: {hits:?}");
    assert!(close(hits[0], 0.156, 3.0));
    // Ready 3 updates after the draw (Corrections 1).
    assert!(close(t, 0.95, 2.0), "pistol ready at {t}");
}

#[test]
fn t25_sprint_knife() {
    let (mut sim, p) = knife_out(STOCK);
    let t0 = mark(&sim);
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    wait_until(&mut sim, t0, 0.1);
    sprint(&mut sim, p, true);
    let mut sprinting_hits = 0;
    let mut pose = None;
    while now(&sim, t0) < 1.0 {
        let n = swings(&sim, p).len();
        sim.ticks(1);
        if swings(&sim, p).len() > n && sim.state(p).sprinting {
            sprinting_hits += 1;
        }
        if pose.is_none() && sel(&sim, p).sprint_pose {
            pose = Some(now(&sim, t0));
        }
    }
    let hits: Vec<f64> = swings(&sim, p).into_iter().map(|t| time_of(&sim, t0, t)).collect();
    assert_eq!(hits.len(), 2, "both hits land: {hits:?}");
    assert!(close(hits[0], 0.156, 3.0) && close(hits[1], 0.403, 3.0), "{hits:?}");
    assert_eq!(sprinting_hits, 2, "both while sprinting");
    let pose = pose.expect("no sprint pose");
    assert!(close(pose, 0.651, 3.0 + SWING_START), "sprint pose at {pose}");
}

// ---------------------------------------------------------------------------
// Drops and pickups (26-35)

fn kill(sim: &mut Sim, p: Entity) {
    let at = sim.position(p);
    sim.app.world_mut().write_message(Damage {
        target: p,
        attacker: None,
        amount: 10.0,
        point: at,
        dir: Vec3::NEG_Y,
        hitgroup: Hitgroup::Generic,
        kind: DamageKind::Generic,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(2);
    assert!(health(sim, p) <= 0.0);
}

fn items(sim: &mut Sim) -> Vec<(Entity, Dropped, String, Vec3)> {
    let w = sim.app.world_mut();
    w.query::<(Entity, &Dropped, &LooseItem, &Transform)>()
        .iter(w)
        .map(|(e, d, l, t)| (e, *d, l.0.clone(), t.translation))
        .collect()
}

/// The left foot (characters.md 3: the drop spot, on the spec's
/// skeleton) and the hull's width.
fn feet(sim: &Sim, p: Entity) -> (Vec3, f32) {
    let s: MovementState = sim.state(p);
    let w = sim.app.world();
    let pose = w.get::<combat_arms::game::characters::CaPose>(p).unwrap();
    let foot = pose.world(w.get::<Transform>(p).unwrap(), &s, UNIT, pose.left_foot.unwrap());
    (foot, s.hull_max.x - s.hull_min.x)
}

#[test]
fn t26_death_drop() {
    let (mut sim, p) = setup(STOCK);
    let rifle = carried(&sim, p, RIFLE);
    set_ammo(&mut sim, rifle, 12, 72);
    key(&mut sim, p, 2);
    sim.seconds(1.0);
    let (foot, width) = feet(&sim, p);
    kill(&mut sim, p);
    let t0 = mark(&sim);
    let dropped = items(&mut sim);
    assert_eq!(dropped.len(), 1, "one item: {dropped:?}");
    let (_, d, name, at) = &dropped[0];
    assert_eq!(name, RIFLE);
    assert_eq!((d.total, d.clip), (72, 12));
    assert_eq!(d.weapon, rifle);
    assert!(
        (at - foot).xz().length() <= width / 2.0,
        "at the left foot: {at} vs {foot}"
    );
    wait_until(&mut sim, t0, 19.9);
    sim.ticks(1);
    assert_eq!(items(&mut sim).len(), 1, "still there");
    sim.seconds(0.2);
    assert!(items(&mut sim).is_empty(), "gone after 20 s");
}

#[test]
fn t27_death_drop_exemptions() {
    let (mut sim, p) = setup(STOCK);
    sim.app.insert_resource(CaDeathDrops(false));
    kill(&mut sim, p);
    assert!(items(&mut sim).is_empty(), "snowball: no item");

    let (mut sim, p) = setup(STOCK);
    let rifle = carried(&sim, p, RIFLE);
    sim.app.world_mut().get_mut::<CaItem>(rifle).unwrap().drop_on_death = false;
    kill(&mut sim, p);
    assert!(items(&mut sim).is_empty(), "drop on death off: no item");
}

/// Gives the carried rifle a part the items do not have, so that picking
/// up a rifle item is the swap of section 11 and not the same-set case of
/// Corrections (2026-10-22) 1.
fn differ(sim: &mut Sim, p: Entity) {
    let rifle = carried(sim, p, RIFLE);
    let mut fitted = combat_arms::game::attachments::CaFitted::default();
    fitted.slots[combat_arms::game::attachments::SIGHT] = Some(1);
    fitted.given = true;
    sim.app.world_mut().entity_mut(rifle).insert(fitted);
}

/// An item of `weapon` lying `dist` metres ahead of `p`'s eye, at eye
/// height (looked at).
fn item_ahead(sim: &mut Sim, p: Entity, weapon: &str, dist: f32, total: u32, clip: u32) -> Entity {
    let eye = sim.position(p) + sim.state(p).eye_offset;
    let item = inventory::spawn_item(sim.app.world_mut(), weapon, eye - Vec3::Z * dist, total, clip).unwrap();
    sim.ticks(1);
    item
}

fn press_use(sim: &mut Sim, p: Entity) {
    tap(sim, p, |i, on| i.use_key = on);
    sim.ticks(1);
}

#[test]
fn t28_pickup_swap() {
    let (mut sim, p) = setup(STOCK);
    differ(&mut sim, p);
    let a = carried(&sim, p, RIFLE);
    set_ammo(&mut sim, a, 5, 35);
    let index = inv(&sim, p).weapons.iter().position(|w| *w == a).unwrap();
    let item = item_ahead(&mut sim, p, RIFLE, 80.0 * UNIT, 60, 30);
    let b = sim.app.world().get::<Dropped>(item).unwrap().weapon;
    assert_eq!(sel(&sim, p).pickup_target, Some(item), "the prompt");
    let (foot, width) = feet(&sim, p);
    let t0 = mark(&sim);
    sim.intent(p).use_key = true;
    sim.ticks(1);
    sim.intent(p).use_key = false;
    let dropped = items(&mut sim);
    assert_eq!(dropped.len(), 1, "A dropped, B removed: {dropped:?}");
    let (_, d, _, at) = &dropped[0];
    assert_eq!(d.weapon, a);
    assert_eq!((d.total, d.clip), (35, 5));
    let off = at - foot;
    assert!(
        off.x.abs() <= width && off.z.abs() <= width,
        "near the left foot: {off}"
    );
    assert_eq!(inv(&sim, p).weapons[index], b, "B in A's equip index");
    assert_eq!(ammo(&sim, b), (30, 60));
    assert_eq!(active(&sim, p), Some(b), "B drawn");
    let t = first_shot(&mut sim, p, t0, b);
    // 3 updates after the draw (Corrections 1).
    assert!(close(t, 1.25, 2.0), "ready after B's draw: {t}");
}

#[test]
fn t29_pickup_while_holding_a_pistol() {
    let (mut sim, p) = setup(STOCK);
    differ(&mut sim, p);
    let a = carried(&sim, p, RIFLE);
    set_ammo(&mut sim, a, 5, 35);
    key(&mut sim, p, 2);
    sim.seconds(1.0);
    let pistol = carried(&sim, p, PISTOL);
    let pistol_ammo = ammo(&sim, pistol);
    let item = item_ahead(&mut sim, p, RIFLE, 80.0 * UNIT, 60, 30);
    let b = sim.app.world().get::<Dropped>(item).unwrap().weapon;
    press_use(&mut sim, p);
    let dropped = items(&mut sim);
    assert_eq!(dropped.len(), 1);
    assert_eq!(dropped[0].1.weapon, a, "A dropped");
    assert_eq!(active(&sim, p), Some(b), "B drawn");
    assert_eq!(ammo(&sim, pistol), pistol_ammo);
}

#[test]
fn t30_pickup_out_of_range() {
    let (mut sim, p) = setup(STOCK);
    // Its surface 140 units away, its centre 150 (> 135).
    let item = item_ahead(&mut sim, p, RIFLE, 150.0 * UNIT, 60, 30);
    assert_eq!(sel(&sim, p).pickup_target, None, "no prompt");
    let rifle = carried(&sim, p, RIFLE);
    press_use(&mut sim, p);
    assert!(sim.app.world().get::<Dropped>(item).is_some(), "E did nothing");
    assert_eq!(active(&sim, p), Some(rifle));
}

#[test]
fn t31_pickup_clamp() {
    let (mut sim, p) = setup(STOCK);
    differ(&mut sim, p);
    let item = item_ahead(&mut sim, p, RIFLE, 80.0 * UNIT, 150, 30);
    let b = sim.app.world().get::<Dropped>(item).unwrap().weapon;
    press_use(&mut sim, p);
    assert_eq!(ammo(&sim, b).1, 120);
}

#[test]
fn t32_pickup_at_total_0() {
    let (mut sim, p) = setup(STOCK);
    differ(&mut sim, p);
    let item = item_ahead(&mut sim, p, RIFLE, 80.0 * UNIT, 0, 7);
    let b = sim.app.world().get::<Dropped>(item).unwrap().weapon;
    press_use(&mut sim, p);
    assert_eq!(active(&sim, p), Some(b));
    assert_eq!(ammo(&sim, b), (0, 0));
}

#[test]
fn t33_same_id_ammo_pickup() {
    let (mut sim, p) = setup(STOCK);
    let pistol = carried(&sim, p, PISTOL);
    set_ammo(&mut sim, pistol, 5, 20);
    let item = item_ahead(&mut sim, p, PISTOL, 80.0 * UNIT, 30, 15);
    press_use(&mut sim, p);
    assert_eq!(ammo(&sim, pistol), (15, 45), "total min(20 + 30, 45), clip refilled");
    assert!(sim.app.world().get_entity(item).is_err(), "the item is removed");
    // The case's "not drawn" is superseded by the "better weapon" rule's
    // answer (Open question 6, case 41): the pistol is drawn.
    sim.seconds(1.0);
    assert_eq!(active(&sim, p), Some(pistol), "the pistol is drawn");
}

#[test]
fn t34_auto_switch_fallback() {
    // No primary at all; the pistol is chosen at spawn.
    let (mut sim, p) = setup(&[PISTOL, KNIFE, GRENADE]);
    assert_eq!(active_id(&sim, p), PISTOL);
    assert_eq!(sel(&sim, p).last_chosen[inventory::PRIMARY as usize], None);
    key(&mut sim, p, 4);
    sim.seconds(0.5);
    throw(&mut sim, p);
    let g = carried(&sim, p, GRENADE);
    let end = sim.app.world().get::<Throwable>(g).unwrap().redraw_at.unwrap();
    let t0 = mark(&sim);
    let t = until(&mut sim, t0, 2.0, |s| active(s, p).is_some_and(|a| id(s, a) == PISTOL));
    let when = sim.app.world().resource::<chimera_core::core::SimClock>().now;
    assert!(
        (when - end).abs() <= 2.0 * dt(&sim),
        "drawn {t} s on, at the throw's end"
    );
}

#[test]
fn t35_respawn() {
    let (mut sim, p) = setup(STOCK);
    differ(&mut sim, p);
    let a = carried(&sim, p, RIFLE);
    set_ammo(&mut sim, a, 5, 35);
    let item = item_ahead(&mut sim, p, RIFLE, 80.0 * UNIT, 60, 30);
    let b = sim.app.world().get::<Dropped>(item).unwrap().weapon;
    press_use(&mut sim, p);
    assert_eq!(active(&sim, p), Some(b));
    kill(&mut sim, p);
    sim.app.world_mut().get_mut::<Health>(p).unwrap().current = 1.0;
    inventory::respawn_inventory(sim.app.world_mut(), p);
    sim.seconds(1.5);
    let weapons: Vec<&str> = inv(&sim, p).weapons.iter().map(|w| id(&sim, *w)).collect();
    assert_eq!(weapons, STOCK, "the loadout again");
    let rifle = active(&sim, p).unwrap();
    assert_eq!(id(&sim, rifle), RIFLE);
    assert_ne!(rifle, a);
    assert_ne!(rifle, b);
    assert_eq!(ammo(&sim, rifle), (30, 120), "full ammo");
    assert_eq!(sel(&sim, p).previous, None);
    q(&mut sim, p);
    sim.ticks(1);
    assert_eq!(active(&sim, p), Some(rifle), "Q does nothing");
}

#[test]
fn t36_rate_cap() {
    assert_eq!(inventory::played_rate(2.1), 1.0);
    assert_eq!(inventory::played_rate(1.9), 1.9);
    let mut sim = sim_with(STOCK);
    let doc = |rate: f32| {
        include_str!("../data/test_m16a3.toml")
            .replace(
                "combat_arms:test_m16a3",
                &format!("combat_arms:rate_{}", (rate * 10.0) as u32),
            )
            .replace("[ammo.\"combat_arms:test_556\"]\nmax = 90\n", "")
            .replace(
                "sprint_out = 0.367",
                &format!("sprint_out = 0.367\nselect_rate = {rate}"),
            )
    };
    let (fast, slow) = (doc(1.1), doc(0.9));
    sim.app
        .add_weapon_data(&[("rate_11.toml", &fast), ("rate_9.toml", &slow)])
        .unwrap();
    let p = player(&mut sim, greybox::SPAWNS[0]);
    for (weapon, draw) in [("combat_arms:rate_11", 1.2), ("combat_arms:rate_9", 1.2 / 1.9)] {
        let w = chimera_sim::weapon::give(sim.app.world_mut(), p, weapon).unwrap();
        let got = sim.app.world().get::<Weapon>(w).unwrap().draw_time;
        assert!((got - draw).abs() < 1e-4, "{weapon}: draw {got}, want {draw}");
    }
}

// ---------------------------------------------------------------------------
// The follow-up answers (37-44)

/// The first knife hit's time after a press at `hz` updates a second
/// (the weapon logic runs once an update: the spec's frames).
fn first_knife_hit(hz: f64) -> f64 {
    let mut sim = sim_with(STOCK);
    sim.set_tick_interval(1.0 / hz);
    let p = player(&mut sim, greybox::SPAWNS[0]);
    key(&mut sim, p, 3);
    sim.seconds(1.0);
    let t0 = mark(&sim);
    sim.intent(p).fire = true;
    let t = until(&mut sim, t0, 1.0, |s| !swings(s, p).is_empty());
    sim.intent(p).fire = false;
    t
}

/// 37 and 50 (Corrections 2026-10-30, 1). The weapon logic on the frame
/// clock: the press seen at u0, the swing from u1 (no pre-fire); the hit
/// on the first update with 155 ms of the swing played: 183.3 ms at 60
/// fps, 170 ms at 100 fps.
#[test]
fn t37_t50_weapon_logic_on_the_frame_clock() {
    for (hz, want) in [(60.0, 0.1833), (100.0, 0.170)] {
        let t = first_knife_hit(hz);
        assert!((t - want).abs() < 1e-3, "{hz} fps: first hit at {t}, want {want}");
    }
}

/// An item's box half-extents (its body's, else the placeholder's 10
/// units each way).
fn item_half(sim: &Sim, item: Entity) -> Vec3 {
    sim.app
        .world()
        .get::<chimera_world::map::PhysicsProp>(item)
        .map_or(Vec3::splat(10.0 * UNIT), |b| (b.bounds.1 - b.bounds.0) / 2.0)
}

/// 38. The fallback is not for weapons: a box the ray misses by 1 unit,
/// its centre 60 units away: no prompt; the ray entering the box at
/// 100.0 units: a prompt; at 100.5: none.
#[test]
fn t38_fallback_is_not_for_weapons() {
    let (mut sim, p) = setup(STOCK);
    let eye = sim.position(p) + sim.state(p).eye_offset;
    let item = inventory::spawn_item(sim.app.world_mut(), RIFLE, eye - Vec3::Z * 60.0 * UNIT, 60, 30).unwrap();
    sim.ticks(1);
    let half = item_half(&sim, item);
    let place = |sim: &mut Sim, at: Vec3| {
        sim.app.world_mut().get_mut::<Transform>(item).unwrap().translation = at;
        sim.ticks(1);
        sel(sim, p).pickup_target
    };
    let beside = eye - Vec3::Z * 60.0 * UNIT + Vec3::X * (half.x + 1.0 * UNIT);
    assert_eq!(place(&mut sim, beside), None, "missed by 1 unit, centre 60 away");
    let rifle = carried(&sim, p, RIFLE);
    press_use(&mut sim, p);
    assert!(sim.app.world().get::<Dropped>(item).is_some(), "E did nothing");
    assert_eq!(active(&sim, p), Some(rifle));
    let at = |units: f32| eye - Vec3::Z * (units * UNIT + half.z);
    assert_eq!(place(&mut sim, at(100.0)), Some(item), "entered at 100.0");
    assert_eq!(place(&mut sim, at(100.5)), None, "entered at 100.5");
}

/// 39. The fallback for bodies: a hit box centre 130 units away counts
/// (130 - 35 <= 100), 140 doesn't.
#[test]
fn t39_fallback_for_bodies() {
    assert!(inventory::body_fallback(130.0 * UNIT));
    assert!(!inventory::body_fallback(140.0 * UNIT));
}

/// 40. A spent grenade stays: same equip index at total 0, its key does
/// nothing, a respawn brings it back with 1.
#[test]
fn t40_spent_grenade() {
    let (mut sim, p) = setup(STOCK);
    let g = carried(&sim, p, GRENADE);
    let index = inv(&sim, p).weapons.iter().position(|w| *w == g).unwrap();
    key(&mut sim, p, 4);
    sim.seconds(0.5);
    throw(&mut sim, p);
    sim.seconds(2.0);
    assert_eq!(inv(&sim, p).weapons.get(index), Some(&g), "still in its equip index");
    assert_eq!(sim.app.world().get::<Throwable>(g).unwrap().count, 0);
    assert_eq!(active_id(&sim, p), RIFLE, "the auto-switch took the rifle");
    key(&mut sim, p, 4);
    sim.ticks(2);
    assert_eq!(active_id(&sim, p), RIFLE, "4: nothing");
    kill(&mut sim, p);
    sim.app.world_mut().get_mut::<Health>(p).unwrap().current = 1.0;
    inventory::respawn_inventory(sim.app.world_mut(), p);
    sim.seconds(1.5);
    let g = carried(&sim, p, GRENADE);
    assert_eq!(sim.app.world().get::<Throwable>(g).unwrap().count, 1, "back with 1");
    key(&mut sim, p, 4);
    sim.seconds(0.5);
    assert_eq!(active_id(&sim, p), GRENADE, "selectable again");
}

/// 41. A pickup always switches: a picked rifle is drawn over the
/// pistol; a same-id pistol ammo item picked with the rifle in hand
/// draws the pistol.
#[test]
fn t41_pickup_always_switches() {
    let (mut sim, p) = setup(STOCK);
    differ(&mut sim, p);
    key(&mut sim, p, 2);
    sim.seconds(1.0);
    let item = item_ahead(&mut sim, p, RIFLE, 80.0 * UNIT, 60, 30);
    let b = sim.app.world().get::<Dropped>(item).unwrap().weapon;
    press_use(&mut sim, p);
    assert_eq!(active(&sim, p), Some(b), "the rifle drawn");

    let (mut sim, p) = setup(STOCK);
    let pistol = carried(&sim, p, PISTOL);
    set_ammo(&mut sim, pistol, 5, 20);
    assert_eq!(active_id(&sim, p), RIFLE);
    item_ahead(&mut sim, p, PISTOL, 80.0 * UNIT, 10, 15);
    press_use(&mut sim, p);
    assert_eq!(ammo(&sim, pistol).1, 30, "the total rose to 30");
    assert_eq!(active(&sim, p), Some(pistol), "the pistol drawn");
}

/// 42. The "better weapon" rule on its own (command ids: the M4A1 1, the
/// MP5A4 0, the M16A3 9, any pistol after all primaries).
#[test]
fn t42_better_weapon_rule() {
    use inventory::better_weapon;
    assert!(!better_weapon(Some(1), 0, false), "MP5A4 under the M4A1: no switch");
    assert!(better_weapon(Some(1), 9, false), "M16A3: switch");
    assert!(better_weapon(Some(1), 40, false), "a pistol: switch");
    assert!(!better_weapon(Some(1), 9, true), "already carried: no switch");
    assert!(better_weapon(None, 0, true), "no weapon in hand: switch");
}

/// 43. The knife's ray through the zones: unarmoured head 270, torso 45,
/// shin 33.75, foot 22.5 (roll 1.0); the ray's cone n = 1; with the
/// default body gear a torso hit is 45 and uses no armour.
#[test]
fn t43_knife_ray_and_zones() {
    let (mut sim, p) = knife_out(STOCK);
    let k = carried(&sim, p, KNIFE);
    assert!(
        (sim.app.world().get::<Knife>(k).unwrap().perturb - 0.001).abs() < 1e-7,
        "n = 1"
    );
    sim.app.world_mut().get_mut::<Knife>(k).unwrap().spread = 0.0;
    // Facing the attacker, 70 units ahead (centre to centre).
    let target = sim.spawn_character(sim.position(p) - Vec3::Z * 70.0 * UNIT, movement::ID);
    sim.intent(target).yaw = std::f32::consts::PI;
    sim.app.world_mut().get_mut::<Health>(target).unwrap().current = 10.0;
    sim.seconds(0.5);
    let sphere = |sim: &Sim, name: &str| super::aim_points::point(sim.app.world(), p, target, name);
    let hit_at = |sim: &mut Sim, name: &str| {
        let d = sphere(sim, name) - (sim.position(p) + sim.state(p).eye_offset);
        let mut i = sim.intent(p);
        i.yaw = (-d.x).atan2(-d.z);
        i.pitch = d.y.atan2(d.xz().length());
        sim.ticks(1);
        one_hit(sim, p, target, false)
    };
    for (name, want) in [
        ("Face_Eyelid1", 270.0),
        ("Torso", 45.0),
        ("Right_legl1", 33.75),
        ("Right_foot", 22.5),
    ] {
        let got = hit_at(&mut sim, name);
        assert!((got - want).abs() < 1e-3, "{name}: {got}, want {want}");
    }
    let ap = sim
        .app
        .world()
        .get::<chimera_sim::weapon::Armor>(target)
        .map(combat_arms::game::weapons::points)
        .unwrap();
    assert_eq!(ap, 100.0, "the default body gear: melee uses no armour");
}

/// The knife's hit writes the full region (`core::RegionHit`) beside its
/// damage, as a gun's does: a groin hit reads as the stomach in the
/// damage, and the announcer's nutshot needs the groin.
#[test]
fn t43b_knife_hit_writes_its_full_region() {
    use chimera_core::core::{BodyRegion, RegionHit};
    #[derive(Resource, Default)]
    struct Regions(Vec<BodyRegion>);
    let (mut sim, p) = knife_out(STOCK);
    sim.app.init_resource::<Regions>().add_systems(
        FixedPostUpdate,
        |mut r: MessageReader<RegionHit>, mut out: ResMut<Regions>| out.0.extend(r.read().map(|h| h.region)),
    );
    let k = carried(&sim, p, KNIFE);
    sim.app.world_mut().get_mut::<Knife>(k).unwrap().spread = 0.0;
    let target = sim.spawn_character(sim.position(p) - Vec3::Z * 70.0 * UNIT, movement::ID);
    sim.intent(target).yaw = std::f32::consts::PI;
    sim.app.world_mut().get_mut::<Health>(target).unwrap().current = 10.0;
    sim.seconds(0.5);
    for (name, want, group) in [
        ("Nuts", BodyRegion::Groin, Hitgroup::Stomach),
        ("Face_Eyelid1", BodyRegion::Head, Hitgroup::Head),
    ] {
        sim.app.world_mut().resource_mut::<Regions>().0.clear();
        let d =
            super::aim_points::point(sim.app.world(), p, target, name) - (sim.position(p) + sim.state(p).eye_offset);
        let mut i = sim.intent(p);
        i.yaw = (-d.x).atan2(-d.z);
        i.pitch = d.y.atan2(d.xz().length());
        sim.ticks(1);
        assert!(one_hit(&mut sim, p, target, false) > 0.0, "{name} hit");
        // (The held trigger may swing again before the next read.)
        let seen = &sim.app.world().resource::<Regions>().0;
        assert!(!seen.is_empty() && seen.iter().all(|r| *r == want), "{name}: {seen:?}");
        assert_eq!(want.hitgroup(), group);
    }
}

/// 44. Drop positions: the death drop exactly at the left foot (the
/// spec's skeleton, characters.md 3; the hull's bottom centre without
/// one); the swap drop within 0.5 x the half-extents
/// (7.5 units) on X and Z, at the same height.
#[test]
fn t44_drop_positions() {
    let (mut sim, p) = setup(STOCK);
    differ(&mut sim, p);
    let (foot, _) = feet(&sim, p);
    kill(&mut sim, p);
    let dropped = items(&mut sim);
    let at = dropped[0].3;
    assert!(
        (at - foot).length() < 1e-4,
        "death drop at the left foot: {at} vs {foot}"
    );

    for _ in 0..3 {
        let (mut sim, p) = setup(STOCK);
        differ(&mut sim, p);
        let (foot, _) = feet(&sim, p);
        item_ahead(&mut sim, p, RIFLE, 80.0 * UNIT, 60, 30);
        press_use(&mut sim, p);
        let at = items(&mut sim)[0].3;
        let off = at - foot;
        assert!(
            off.x.abs() <= 7.5 * UNIT + 1e-5 && off.z.abs() <= 7.5 * UNIT + 1e-5,
            "swap drop: {off}"
        );
        assert!(off.y.abs() < 1e-4, "same height: {off}");
    }
}

// ---------------------------------------------------------------------------
// Corrections (2026-10-19): 45-49

/// A player with the stock loadout at 60 updates a second.
fn setup_60() -> (Sim, Entity) {
    let mut sim = sim_with(STOCK);
    sim.set_tick_interval(F);
    let p = player(&mut sim, greybox::SPAWNS[0]);
    (sim, p)
}

/// The update (from `t0`) on which `w`'s clip first satisfies `cond`.
fn update_of(sim: &mut Sim, t0: u64, w: Entity, cond: impl Fn((u32, u32)) -> bool) -> u64 {
    let w2 = w;
    until(sim, t0, 6.0, move |s| cond(ammo(s, w2)));
    sim.tick() - t0
}

/// 45. A shot after a reload, fire held: the update at 2300 ms completes
/// it (clip 30) and enters the firing state; the round on the update at
/// 2350 ms (clip 29, total 64).
#[test]
fn t45_shot_after_a_reload() {
    let (mut sim, p) = setup_60();
    let rifle = carried(&sim, p, RIFLE);
    set_ammo(&mut sim, rifle, 5, 65);
    let t0 = mark(&sim);
    tap(&mut sim, p, |i, on| i.reload = on);
    wait_until(&mut sim, t0, 0.5);
    sim.intent(p).fire = true;
    let full = update_of(&mut sim, t0, rifle, |a| a.0 == 30);
    assert_eq!(ammo(&sim, rifle), (30, 65));
    let round = update_of(&mut sim, t0, rifle, |a| a.0 < 30);
    assert_eq!(round - full, 3, "the 3rd update after: {full} then {round}");
    assert!(close(time_of(&sim, t0, t0 + round), 2.35, 1.0), "at {round}");
    assert_eq!(ammo(&sim, rifle), (29, 64));
}

/// 46. A shot after a draw: pistol in hand, 1 pressed with fire held →
/// nothing before the update at 1250 ms, the round on it.
#[test]
fn t46_shot_after_a_draw() {
    let (mut sim, p) = setup_60();
    key(&mut sim, p, 2);
    sim.seconds(1.0);
    let rifle = carried(&sim, p, RIFLE);
    let t0 = mark(&sim);
    sim.intent(p).fire = true;
    key(&mut sim, p, 1);
    let round = update_of(&mut sim, t0, rifle, |a| a.0 < 30);
    assert_eq!(round, 75, "1250 ms is update 75");
}

/// 47. A primary shotgun after a draw (1000 ms, fire held): the round on
/// the update at 1033.3 ms (2 updates).
#[test]
fn t47_shotgun_after_a_draw() {
    let (mut sim, p) = setup_60();
    sim.intent(p).fire = true;
    let t0 = mark(&sim);
    let w = chimera_sim::weapon::give(sim.app.world_mut(), p, "combat_arms:remington_870").unwrap();
    let round = update_of(&mut sim, t0, w, |a| a.0 < 7);
    assert_eq!(round, 62, "1033.3 ms is update 62");
}

/// 48. Leaving sprint: the update that sees the post-sprint pose end
/// returns to idle; firing, pre-fire and fire animation take the next
/// three; the round on the 4th after it (367 ms seen on the update at
/// 383.3, the round at 450 ms).
#[test]
fn t48_leaving_sprint() {
    let (mut sim, p) = setup_60();
    let rifle = carried(&sim, p, RIFLE);
    let t = leave_sprint(&mut sim, p, |s| ammo(s, rifle).0 < 30);
    assert!(close(t, 0.45, 1.0), "first shot at {t}");
}

/// 49. Backstab by the body: a standing victim whose legs are 44.7
/// degrees off its view; a stab behind it by the view but not by the
/// body deals the front's damage ([67.5, 82.5] around 75), not 97.5.
#[test]
fn t49_backstab_by_the_body() {
    let dir = Vec3::NEG_Z;
    let twist = 0.78;
    let rear = |yaw: f32| {
        let q = Quat::from_rotation_y(yaw);
        inventory::behind(q * Vec3::NEG_Z, q * Vec3::X, dir)
    };
    // The middle of the views that are behind by the view, not by the
    // body (clear of the sectors' edges).
    let views: Vec<f32> = (0..360)
        .map(|d| (d as f32).to_radians())
        .filter(|v| rear(*v) && !rear(v - twist))
        .collect();
    let view = views[views.len() / 2];
    let (mut sim, p, target) = knife_target(50.0 * UNIT, view);
    // The body yaw is kept by a Combat Arms weapon's spread.
    let w = chimera_sim::weapon::give(sim.app.world_mut(), target, "combat_arms:m4a1").unwrap();
    sim.seconds(1.5);
    {
        let mut s = sim
            .app
            .world_mut()
            .get_mut::<combat_arms::game::weapons::CaSpread>(w)
            .expect("its spread");
        s.body_yaw = Some(view - twist);
        s.prev_view = None;
    }
    let d = one_hit(&mut sim, p, target, true);
    assert!(in_range(d, 67.5, 82.5), "a stab from the front by the body: {d}");
}

// ---------------------------------------------------------------------------
// Corrections (2026-10-22): 1-3

/// Corrections 1(a): a primary item whose parts match the carried
/// primary's is no swap: the carried weapon keeps its entity and index, its
/// total and clip are set to the item's (down as well as up), nothing is
/// dropped and the item is gone.
#[test]
fn c1a_matching_set_keeps_the_weapon() {
    let (mut sim, p) = setup(STOCK);
    let a = carried(&sim, p, RIFLE);
    set_ammo(&mut sim, a, 10, 60);
    let index = inv(&sim, p).weapons.iter().position(|w| *w == a).unwrap();
    let item = item_ahead(&mut sim, p, RIFLE, 80.0 * UNIT, 20, 20);
    press_use(&mut sim, p);
    assert_eq!(inv(&sim, p).weapons[index], a, "same weapon, same equip index");
    assert_eq!(ammo(&sim, a), (20, 20), "set to the item's, not added");
    assert!(items(&mut sim).is_empty(), "nothing dropped, the item gone");
    assert!(sim.app.world().get_entity(item).is_err());
}

/// Corrections 1(b): the weapon itself is not compared (a faithful quirk):
/// a different bare rifle item leaves the carried one in hand with the
/// item's round count, clamped to the carried maximum.
#[test]
fn c1b_the_weapon_is_not_compared() {
    let (mut sim, p) = setup(STOCK);
    let a = carried(&sim, p, RIFLE);
    set_ammo(&mut sim, a, 10, 60);
    item_ahead(&mut sim, p, "combat_arms:test_hud_rifle", 80.0 * UNIT, 90, 30);
    press_use(&mut sim, p);
    assert_eq!(id(&sim, active(&sim, p).unwrap()), RIFLE, "still the M16A3");
    assert_eq!(ammo(&sim, a), (30, 90));
    assert!(items(&mut sim).is_empty(), "nothing dropped");
}

/// Corrections 1(c): a different fitted set is the normal swap.
#[test]
fn c1c_a_different_set_swaps() {
    let (mut sim, p) = setup(STOCK);
    let a = carried(&sim, p, RIFLE);
    set_ammo(&mut sim, a, 10, 60);
    differ(&mut sim, p);
    let item = item_ahead(&mut sim, p, "combat_arms:test_hud_rifle", 80.0 * UNIT, 90, 30);
    let b = sim.app.world().get::<Dropped>(item).unwrap().weapon;
    press_use(&mut sim, p);
    let dropped = items(&mut sim);
    assert_eq!(dropped.len(), 1);
    assert_eq!(
        (dropped[0].1.weapon, dropped[0].1.total, dropped[0].1.clip),
        (a, 60, 10)
    );
    assert_eq!(active(&sim, p), Some(b), "the item's rifle is drawn");
}

/// Corrections 2: a non-primary item whose id is not carried needs the
/// class's last chosen weapon: with the grenade never drawn since the
/// spawn the request does nothing; after drawing it the swap happens; a
/// same-id grenade item still adds either way.
#[test]
fn c2_no_pickup_into_an_unselected_class() {
    let (mut sim, p) = setup(STOCK);
    assert_eq!(sel(&sim, p).last_chosen[4], None, "the M67 was never drawn");
    let item = item_ahead(&mut sim, p, OTHER_GRENADE, 80.0 * UNIT, 2, 0);
    press_use(&mut sim, p);
    assert!(sim.app.world().get::<Dropped>(item).is_some(), "the item stays");
    assert_eq!(inv(&sim, p).weapons.len(), STOCK.len(), "nothing added");
    // A same-id item adds, drawn or not.
    sim.app.world_mut().despawn(item);
    let mine = carried(&sim, p, GRENADE);
    let same = item_ahead(&mut sim, p, GRENADE, 80.0 * UNIT, 1, 0);
    press_use(&mut sim, p);
    assert!(sim.app.world().get_entity(same).is_err());
    // Draw the grenade, go back to the rifle: now the swap works.
    key(&mut sim, p, 4);
    sim.seconds(1.0);
    key(&mut sim, p, 1);
    sim.seconds(1.0);
    assert_eq!(sel(&sim, p).last_chosen[4], Some(mine));
    let item = item_ahead(&mut sim, p, OTHER_GRENADE, 80.0 * UNIT, 2, 0);
    press_use(&mut sim, p);
    assert!(sim.app.world().get_entity(item).is_err(), "taken");
    let dropped = items(&mut sim);
    assert_eq!(dropped.len(), 1);
    assert_eq!(dropped[0].1.weapon, mine, "the M67 is dropped");
}

/// 51 (Corrections 2026-10-30, 2). A character model with select bonus
/// 0.25 draws the M9 (450 ms) in 360 ms; the same for an M67 (400 ms ->
/// 320 ms): the bonus reaches weapons without parts too.
#[test]
fn t51_select_bonus_on_a_knife_and_a_grenade() {
    use combat_arms::game::gear::{CaGearData, records::GearData};
    let mut g = GearData::built_in();
    for v in g.vests.values_mut() {
        v.select_ani_rate = 0.25;
    }
    let mut sim = Sim::with(CaSim, move |app| {
        app.add_plugins(GreyboxMapPlugin);
        app.insert_resource(CaGearData(std::sync::Arc::new(g.clone())));
    });
    sim.set_tick_interval(F);
    let p = player(&mut sim, greybox::SPAWNS[0]);
    for (id, want) in [
        ("combat_arms:m9_knife", 0.36),
        ("combat_arms:m67", 0.32),
        ("combat_arms:m4a1", 0.96),
    ] {
        let w = chimera_sim::weapon::give(sim.app.world_mut(), p, id).unwrap();
        sim.ticks(2);
        let got = sim.app.world().get::<Weapon>(w).unwrap().draw_time;
        assert!((got - want).abs() < 1e-4, "{id}: draw {got}, want {want}");
    }
}

/// special_weapons.md 46: the chainsaw's secondary at 15 updates a second,
/// the swing starting on an update: keys 334 and 386 pass in one advance,
/// as do 540 and 591, so 5 hits a swing, not 7.
#[test]
fn t46_hit_keys_in_one_advance_make_one_hit() {
    let mut sim = sim_with(&["combat_arms:chainsaw"]);
    sim.set_tick_interval(1.0 / 15.0);
    let p = player(&mut sim, greybox::SPAWNS[0]);
    sim.seconds(2.0);
    let before = swings(&sim, p).len();
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    sim.seconds(1.5);
    let n = swings(&sim, p).len() - before;
    assert_eq!(n, 5, "hits in one swing");
}

// ---------------------------------------------------------------------------
// The fist (special_weapons.md, Follow-up (2026-11-22): test cases 75-82)

/// The M9's knife set up as the fist (the install's `fist`: damage type
/// 49, 2 and 6 points a punch, `Range` 130), roll 1.0, against a target
/// `ahead` units in front of the eye facing `yaw`, with health to spare.
fn fist_target(ahead: f32, yaw: f32) -> (Sim, Entity, Entity) {
    let (mut sim, p) = knife_out(STOCK);
    let k = carried(&sim, p, KNIFE);
    {
        let mut e = sim.app.world_mut().entity_mut(k);
        let mut knife = e.get_mut::<Knife>().unwrap();
        knife.spread = 0.0;
        knife.damage_type = inventory::FIST_DAMAGE;
        knife.range = 130.0 * UNIT;
        knife.attacks[0].damage = 0.02;
        knife.attacks[1].damage = 0.06;
        // The knuckles' swings (F4): one hit key each.
        knife.attacks[0].length = 0.8;
        knife.attacks[0].hits = vec![0.412];
        knife.attacks[1].length = 1.0;
        knife.attacks[1].hits = vec![0.424];
    }
    let target = fist_victim(&mut sim, p, ahead, yaw);
    (sim, p, target)
}

fn fist_victim(sim: &mut Sim, p: Entity, ahead: f32, yaw: f32) -> Entity {
    let target = sim.spawn_character(sim.position(p) - Vec3::Z * ahead * UNIT, movement::ID);
    sim.intent(target).yaw = yaw;
    sim.app.world_mut().get_mut::<Health>(target).unwrap().current = 1.0e6;
    sim.seconds(0.5);
    target
}

fn punch_at(sim: &mut Sim, p: Entity, target: Entity, zone: &str, secondary: bool) -> f32 {
    let d = super::aim_points::point(sim.app.world(), p, target, zone) - (sim.position(p) + sim.state(p).eye_offset);
    let mut i = sim.intent(p);
    i.yaw = (-d.x).atan2(-d.z);
    i.pitch = d.y.atan2(d.xz().length());
    sim.ticks(1);
    one_hit(sim, p, target, secondary)
}

/// 75, 77. Punch damage: secondary to the chest 6, head 36, shin 4.5, foot
/// 3; primary chest 2, head 12; the default gear reduces nothing and no
/// armour point is used.
#[test]
fn t75_punch_damage_and_t77_no_armour() {
    let (mut sim, p, target) = fist_target(70.0, std::f32::consts::PI);
    for (zone, secondary, want) in [
        ("Torso", true, 6.0),
        ("Face_Eyelid1", true, 36.0),
        ("Right_legl1", true, 4.5),
        ("Right_foot", true, 3.0),
        ("Torso", false, 2.0),
        ("Face_Eyelid1", false, 12.0),
    ] {
        let got = punch_at(&mut sim, p, target, zone, secondary);
        assert!((got - want).abs() < 1e-3, "{zone} secondary {secondary}: {got}, want {want}");
    }
    let ap = sim.app.world().get::<chimera_sim::weapon::Armor>(target).map(combat_arms::game::weapons::points).unwrap();
    assert_eq!(ap, 100.0, "a punch uses no armour");
}

/// 76. From behind: secondary chest 7.8, head 46.8; primary head 15.6.
#[test]
fn t76_punch_from_behind() {
    let (mut sim, p, target) = fist_target(70.0, 0.0);
    for (zone, secondary, want) in [("Torso", true, 7.8), ("Face_Eyelid1", true, 46.8), ("Face_Eyelid1", false, 15.6)] {
        let got = punch_at(&mut sim, p, target, zone, secondary);
        assert!((got - want).abs() < 1e-3, "{zone} secondary {secondary}: {got}, want {want}");
    }
}

/// 80. A second victim on the ray within `Range` is hurt as well; beyond
/// it, or after a knife's swing, not.
#[test]
fn t80_punch_reaches_a_second_character() {
    let (mut sim, p, a) = fist_target(40.0, std::f32::consts::PI);
    let b = fist_victim(&mut sim, p, 90.0, std::f32::consts::PI);
    let c = fist_victim(&mut sim, p, 190.0, std::f32::consts::PI);
    sim.app.world_mut().resource_mut::<Hurt>().0.clear();
    let from = sim.tick();
    punch_at(&mut sim, p, a, "Torso", true);
    let hit = |sim: &Sim, e: Entity| sim.app.world().resource::<Hurt>().0.iter().any(|(t, v, _)| *t > from && *v == e);
    assert!(hit(&sim, a), "the first");
    assert!(hit(&sim, b), "the second, within 130 u");
    assert!(!hit(&sim, c), "the third, beyond the reach");
    // A knife's ray ends at its first intersection.
    let (mut sim, p, a) = {
        let (mut sim, p) = knife_out(STOCK);
        let a = fist_victim(&mut sim, p, 40.0, std::f32::consts::PI);
        (sim, p, a)
    };
    let b = fist_victim(&mut sim, p, 90.0, std::f32::consts::PI);
    let from = sim.tick();
    punch_at(&mut sim, p, a, "Torso", true);
    let hit = |sim: &Sim, e: Entity| sim.app.world().resource::<Hurt>().0.iter().any(|(t, v, _)| *t > from && *v == e);
    assert!(hit(&sim, a));
    assert!(!hit(&sim, b), "a knife never reaches a second character");
}

/// 81, 82. A punch is ordinary damage (not the stab kind): its death is
/// the ordinary choice and it plays no gun-kind hurt sound, but the head
/// sound; its impact sound is the surface's fist list.
#[test]
fn t81_t82_punch_kind_and_sounds() {
    use combat_arms::game::effects::{
        self,
        data::{AmmoKind, CaFxTables, ImpactFx},
    };
    #[derive(Resource, Default)]
    struct Kinds(Vec<DamageKind>);
    let (mut sim, p, target) = fist_target(70.0, std::f32::consts::PI);
    sim.app.init_resource::<Kinds>().add_systems(
        FixedPostUpdate,
        |mut r: MessageReader<Damage>, mut out: ResMut<Kinds>| out.0.extend(r.read().map(|d| d.kind)),
    );
    punch_at(&mut sim, p, target, "Face_Eyelid1", false);
    let kinds = &sim.app.world().resource::<Kinds>().0;
    assert_eq!(kinds, &[DamageKind::Generic], "one punch, not the stab kind");
    assert_eq!(effects::hurt_kinds(DamageKind::Generic, Hitgroup::Head, false)[0], None, "no gun sound");
    assert!(effects::hurt_kinds(DamageKind::Generic, Hitgroup::Head, false)[1].is_some(), "the head sound");
    assert!(effects::hurt_kinds(DamageKind::Melee, Hitgroup::Head, false)[0].is_some(), "a knife's gun sound");
    let tables = CaFxTables::built_in();
    let wood = tables.surface(tables.surface_id("Wood").unwrap());
    let record = ImpactFx { sound: "surface".into(), ..Default::default() };
    let fist = effects::impact_sound(&record, wood, AmmoKind::Fist, 0.0).unwrap();
    let melee = effects::impact_sound(&record, wood, AmmoKind::Melee, 0.0).unwrap();
    assert!(fist.contains("fist"), "{fist}");
    assert_ne!(fist, melee);
}

/// Every damage message of one swing (`fire` or alt-fire), the swing let
/// end: (tick, target, points).
fn swing_damage(sim: &mut Sim, p: Entity, target: Entity, zone: &str, secondary: bool) -> Vec<(u64, Entity, f32)> {
    let d = super::aim_points::point(sim.app.world(), p, target, zone) - (sim.position(p) + sim.state(p).eye_offset);
    let mut i = sim.intent(p);
    i.yaw = (-d.x).atan2(-d.z);
    i.pitch = d.y.atan2(d.xz().length());
    sim.ticks(1);
    sim.app.world_mut().resource_mut::<Hurt>().0.clear();
    if secondary {
        tap(sim, p, |i, on| i.secondary = on);
    } else {
        tap(sim, p, |i, on| i.fire = on);
    }
    sim.seconds(1.6);
    sim.app.world().resource::<Hurt>().0.clone()
}

/// Melee damage is dealt once per victim per hit key: a punch (one key)
/// on two characters in line is one message each, the spec's amounts (6
/// on the reported victim's chest; the one behind it 6 x its zone); the
/// M9's primary (keys at 155 and 402 ms) two on its victim, 45 each, a
/// key apart. (Two messages in the punch tests came from the M9's two
/// keys, which the fist set-up kept before it took the knuckles' swings.)
#[test]
fn melee_one_damage_per_victim_per_hit_key() {
    let (mut sim, p, a) = fist_target(40.0, std::f32::consts::PI);
    let b = fist_victim(&mut sim, p, 90.0, std::f32::consts::PI);
    for secondary in [true, false] {
        let hurt = swing_damage(&mut sim, p, a, "Torso", secondary);
        let on = |e: Entity| hurt.iter().filter(|h| h.1 == e).map(|h| h.2).collect::<Vec<_>>();
        let want = if secondary { 6.0 } else { 2.0 };
        assert_eq!(on(a).len(), 1, "secondary {secondary}: {hurt:?}");
        assert!((on(a)[0] - want).abs() < 1e-3, "secondary {secondary}: {hurt:?}");
        assert_eq!(on(b).len(), 1, "secondary {secondary}, behind: {hurt:?}");
        assert!(on(b)[0] > 0.0 && on(b)[0] <= want * 6.0 + 1e-3, "{hurt:?}");
        assert_eq!(hurt.len(), 2, "nothing else: {hurt:?}");
    }
    let (mut sim, p) = knife_out(STOCK);
    let k = carried(&sim, p, KNIFE);
    sim.app.world_mut().get_mut::<Knife>(k).unwrap().spread = 0.0;
    let a = fist_victim(&mut sim, p, 40.0, std::f32::consts::PI);
    let hurt = swing_damage(&mut sim, p, a, "Torso", false);
    assert_eq!(hurt.len(), 2, "one a key: {hurt:?}");
    assert!(hurt.iter().all(|h| h.1 == a && (h.2 - 45.0).abs() < 1e-3), "{hurt:?}");
    assert_ne!(hurt[0].0, hurt[1].0, "a key apart");
}

/// A static panel `units` thick across the lane `ahead` units in front of
/// `p`, of `surface`.
fn panel_ahead(sim: &mut Sim, p: Entity, ahead: f32, units: f32, surface: &str) {
    let at = sim.position(p) + sim.state(p).eye_offset - Vec3::Z * ahead * UNIT;
    sim.app.world_mut().spawn((
        Transform::from_translation(at),
        avian3d::prelude::RigidBody::Static,
        avian3d::prelude::Collider::cuboid(3.0, 3.0, units * UNIT),
        chimera_core::core::PropSurface(surface.into()),
    ));
    sim.ticks(1);
}

/// F2 (and 80's walk): a punch passes a shoot-through surface as a bullet
/// does: 6 units of wood between the fist and a chest 70 units away ->
/// 6 x Wood's 0.5 = 3 (the walk found it: no client hit, no behind
/// bonus); concrete stops it; a knife's swing stops at the wood.
#[test]
fn fist_punches_through_wood() {
    for (surface, want) in [("Wood", 3.0), ("Concrete", 0.0)] {
        let (mut sim, p, target) = fist_target(70.0, std::f32::consts::PI);
        panel_ahead(&mut sim, p, 35.0, 6.0, surface);
        let hurt = swing_damage(&mut sim, p, target, "Torso", true);
        let got: f32 = hurt.iter().filter(|h| h.1 == target).map(|h| h.2).sum();
        assert!((got - want).abs() < 1e-3, "{surface}: {hurt:?}, want {want}");
    }
    let (mut sim, p) = knife_out(STOCK);
    let k = carried(&sim, p, KNIFE);
    sim.app.world_mut().get_mut::<Knife>(k).unwrap().range = 130.0 * UNIT;
    let target = fist_victim(&mut sim, p, 70.0, std::f32::consts::PI);
    panel_ahead(&mut sim, p, 35.0, 6.0, "Wood");
    let hurt = swing_damage(&mut sim, p, target, "Torso", true);
    assert!(hurt.iter().all(|h| h.1 != target), "a knife stops at the wood: {hurt:?}");
}
