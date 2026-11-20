//! specs/tf2/minigun.md's test cases 1-18 (`weapons::minigun`): the
//! Minigun's five states by buttons and ammo, its one sound at a time cut
//! at each change (the wind-up at 0.75 s, the wind-down when the barrel
//! stops), running dry and the auto-switch, the barrel by tick, and other
//! players deriving the same sounds from its replicated state. Ticks of
//! 0.015 s, the press on run tick 0.

use bevy::prelude::*;
use chimera_core::core::Health;
use chimera_sim::weapon::{Inventory, Magazine, Unselectable, Weapon, WeaponEventKind, give};
use chimera_world::{
    greybox,
    harness::Sim,
    map::{PlaySound, StopSound},
};
use tf2::game::weapons::{
    MINIGUN, SHOTGUN,
    minigun::{BARREL_MAX_SPEED, State, Tf2Minigun},
};

use super::weapons::{holding, sim};

const WIND_UP: &str = "tf2:Weapon_Minigun.WindUp";
const FIRE: &str = "tf2:Weapon_Minigun.Fire";
const CRIT: &str = "tf2:Weapon_Minigun.FireCrit";
const SPIN: &str = "tf2:Weapon_Minigun.Spin";
const EMPTY: &str = "tf2:Weapon_Minigun.ClipEmpty";
const WIND_DOWN: &str = "tf2:Weapon_Minigun.WindDown";

/// What one tick did: the gun's state, sounds started and stopped (by
/// entry) and whether it fired.
#[derive(Debug, Default, Clone)]
struct Tick {
    state: State,
    started: Vec<String>,
    stopped: Vec<String>,
    shot: bool,
    active: Option<Entity>,
}

/// A recorder of `p`'s Minigun `w`, tick by tick.
struct Rec {
    p: Entity,
    w: Entity,
    play: bevy::ecs::message::MessageCursor<PlaySound>,
    stop: bevy::ecs::message::MessageCursor<StopSound>,
    events: bevy::ecs::message::MessageCursor<chimera_sim::weapon::WeaponEvent>,
    ticks: Vec<Tick>,
}

impl Rec {
    fn new(sim: &Sim, p: Entity, w: Entity) -> Self {
        let world = sim.app.world();
        Self {
            p,
            w,
            play: world.resource::<Messages<PlaySound>>().get_cursor_current(),
            stop: world.resource::<Messages<StopSound>>().get_cursor_current(),
            events: world.resource::<Messages<chimera_sim::weapon::WeaponEvent>>().get_cursor_current(),
            ticks: Vec::new(),
        }
    }

    /// `n` ticks, `input(sim, tick)` setting attack and attack2 first.
    fn run(&mut self, sim: &mut Sim, n: usize, mut input: impl FnMut(&mut Sim, usize) -> (bool, bool)) {
        for _ in 0..n {
            let t = self.ticks.len();
            let (a, b) = input(sim, t);
            sim.intent(self.p).fire = a;
            sim.intent(self.p).secondary = b;
            sim.ticks(1);
            let world = sim.app.world();
            let p = self.p;
            let started = self.play.read(world.resource::<Messages<PlaySound>>()).filter(|s| s.source == Some(p)).map(|s| s.entry.clone()).collect();
            let stopped = self.stop.read(world.resource::<Messages<StopSound>>()).filter(|s| s.source == p).map(|s| s.entry.clone()).collect();
            let shot = self
                .events
                .read(world.resource::<Messages<chimera_sim::weapon::WeaponEvent>>())
                .any(|e| e.owner == p && e.shown() && matches!(e.kind, WeaponEventKind::Fired { .. }));
            self.ticks.push(Tick {
                state: world.get::<Tf2Minigun>(self.w).map_or(State::Idle, |g| g.state),
                started,
                stopped,
                shot,
                active: world.get::<Inventory>(p).and_then(|i| i.active),
            });
        }
    }

    /// The ticks on which `entry` started (or stopped).
    fn starts(&self, entry: &str) -> Vec<usize> {
        (0..self.ticks.len()).filter(|t| self.ticks[*t].started.iter().any(|e| e == entry)).collect()
    }
    fn stops(&self, entry: &str) -> Vec<usize> {
        (0..self.ticks.len()).filter(|t| self.ticks[*t].stopped.iter().any(|e| e == entry)).collect()
    }
    fn shots(&self) -> Vec<usize> {
        (0..self.ticks.len()).filter(|t| self.ticks[*t].shot).collect()
    }
    /// Every sound started, in order, with its tick.
    fn all_starts(&self) -> Vec<(usize, String)> {
        self.ticks.iter().enumerate().flat_map(|(t, k)| k.started.iter().map(move |e| (t, e.clone()))).collect()
    }
    fn state(&self, t: usize) -> State {
        self.ticks[t].state
    }
}

fn setup() -> (Sim, Entity, Entity) {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], MINIGUN);
    (sim, p, w)
}

/// A Heavy with a Shotgun drawn before the Minigun (its previous weapon).
fn setup_with_previous() -> (Sim, Entity, Entity, Entity) {
    let mut sim = sim();
    let (p, s) = holding(&mut sim, greybox::SPAWNS[0], SHOTGUN);
    let w = give(sim.app.world_mut(), p, MINIGUN).unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
    sim.seconds(1.0);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(w));
    (sim, p, w, s)
}

/// 1, 2: held from idle the wind-up, cut at tick 50 by the fire loop;
/// shots at 57 + 7k; released at tick 200 the last shot is 197, the fire
/// loop cut and the wind-down at 211, the wind-down cut at 411.
#[test]
fn held_then_released_1_2() {
    let (mut sim, p, w) = setup();
    let mut r = Rec::new(&sim, p, w);
    r.run(&mut sim, 450, |_, t| (t < 200, false));
    assert_eq!(r.starts(WIND_UP), [0]);
    assert_eq!(r.stops(WIND_UP), [50]);
    assert_eq!(r.starts(FIRE), [50]);
    let shots = r.shots();
    assert_eq!(shots.first(), Some(&57));
    assert!(shots.windows(2).all(|s| s[1] - s[0] == 7), "{shots:?}");
    assert_eq!(shots.last(), Some(&197));
    assert_eq!(r.stops(FIRE), [211]);
    assert_eq!(r.starts(WIND_DOWN), [211]);
    assert_eq!(r.stops(WIND_DOWN), [411]);
    assert_eq!(r.all_starts().len(), 3, "{:?}", r.all_starts());
}

/// 3: a 0.3 s tap: the wind-up completes (not cut at the release), the
/// wind-down from tick 50 to 100 (barrel 5 rad/s to 0); idle, full speed
/// and jumps from tick 50.
#[test]
fn a_tap_winds_up_fully_3() {
    let (mut sim, p, w) = setup();
    let mut r = Rec::new(&sim, p, w);
    let mut speed = Vec::new();
    r.run(&mut sim, 120, |s, t| {
        speed.push(s.app.world().get::<Weapon>(w).unwrap().max_speed);
        (t < 20, false)
    });
    assert_eq!(r.starts(WIND_UP), [0]);
    assert_eq!(r.stops(WIND_UP), [50]);
    assert_eq!(r.starts(WIND_DOWN), [50]);
    assert_eq!(r.stops(WIND_DOWN), [100]);
    assert_eq!(r.state(49), State::WindingUp);
    assert_eq!(r.state(50), State::Idle);
    assert!(r.shots().is_empty());
    // `speed[t]` is read before tick t: the tick after the wind-down.
    assert!(speed[50].is_some() && speed[51].is_none(), "{:?}", &speed[48..53]);
}

/// 4, 5, 6: attack2 spins from tick 50; attack pressed at 100 replaces
/// the spin with the fire loop and fires that tick; let go at 200 with
/// attack2 still held, the spin loop is back that tick; attack2 let go
/// after tick 249, the wind-down at 269 (0.3 s on).
#[test]
fn spin_fire_spin_4_5_6() {
    let (mut sim, p, w) = setup();
    let mut r = Rec::new(&sim, p, w);
    r.run(&mut sim, 300, |_, t| ((100..200).contains(&t), t < 250));
    assert_eq!(r.starts(SPIN), [50, 200]);
    assert_eq!(r.stops(WIND_UP), [50]);
    assert_eq!(r.starts(FIRE), [100]);
    assert_eq!(r.stops(SPIN), [100, 269]);
    assert_eq!(r.shots().first(), Some(&100), "fires on the press");
    assert_eq!(r.stops(FIRE), [200]);
    assert_eq!(r.state(268), State::Spinning);
    assert_eq!(r.starts(WIND_DOWN), [269]);
}

/// 4: attack2 held to tick 149: the spin loop to tick 169, then the
/// wind-down, cut at 338 (the barrel stops from 16.9 rad/s).
#[test]
fn spin_released_4() {
    let (mut sim, p, w) = setup();
    let mut r = Rec::new(&sim, p, w);
    r.run(&mut sim, 360, |_, t| (false, t < 150));
    assert_eq!(r.starts(SPIN), [50]);
    assert_eq!(r.stops(SPIN), [169]);
    assert_eq!(r.starts(WIND_DOWN), [169]);
    assert_eq!(r.stops(WIND_DOWN), [338]);
}

/// 7: pressed again during the wind-down: the wind-down cut and the
/// wind-up that tick, the fire loop 50 ticks on, the first shot 57 on.
#[test]
fn pressed_again_while_winding_down_7() {
    let (mut sim, p, w) = setup();
    let mut r = Rec::new(&sim, p, w);
    r.run(&mut sim, 400, |_, t| (t < 200 || t >= 300, false));
    assert_eq!(r.starts(WIND_DOWN), [211]);
    assert_eq!(r.stops(WIND_DOWN), [300]);
    assert_eq!(r.starts(WIND_UP), [0, 300]);
    assert_eq!(r.starts(FIRE), [50, 350]);
    assert!(r.shots().contains(&357) && !r.shots().contains(&350));
}

/// 8, 9: 200 shots (ticks 57 to 1450), dry at 1457 with the empty loop,
/// no sound started again to tick 2000; released then: the empty loop cut
/// and the wind-down at once; the previous weapon drawn about 1.33 s on;
/// the empty Minigun can't be drawn again.
#[test]
fn running_dry_and_the_switch_8_9() {
    let (mut sim, p, w, shotgun) = setup_with_previous();
    let mut r = Rec::new(&sim, p, w);
    r.run(&mut sim, 2200, |_, t| (t < 2000, false));
    let shots = r.shots();
    assert_eq!((shots.len(), shots[0], *shots.last().unwrap()), (200, 57, 1450));
    assert_eq!(r.stops(FIRE), [1457]);
    assert_eq!(r.starts(EMPTY), [1457]);
    assert_eq!(r.state(1999), State::Dry);
    assert!(r.all_starts().iter().all(|(t, _)| *t <= 1457 || *t >= 2000), "{:?}", r.all_starts());
    assert_eq!(r.stops(EMPTY), [2000]);
    assert_eq!(r.starts(WIND_DOWN), [2000]);
    let switched = (2000..2200).find(|t| r.ticks[*t].active == Some(shotgun)).expect("switched to the Shotgun");
    assert!((2085..=2093).contains(&switched), "switch at tick {switched}");
    assert!(sim.app.world().get::<Unselectable>(w).is_some(), "an empty Minigun can't be drawn");
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
    sim.ticks(5);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(shotgun));
}

/// 10: both held: dry at 1457; attack let go at 24 s, still dry with the
/// empty loop; attack2 let go at 25.5 s, the wind-down that tick, the
/// switch about 1.33 s later.
#[test]
fn dry_with_attack2_never_idles_10() {
    let (mut sim, p, w, shotgun) = setup_with_previous();
    let mut r = Rec::new(&sim, p, w);
    r.run(&mut sim, 1900, |_, t| (t < 1600, t < 1700));
    assert_eq!(r.starts(EMPTY), [1457]);
    assert_eq!(r.state(1699), State::Dry);
    assert_eq!(r.stops(EMPTY), [1700]);
    assert_eq!(r.starts(WIND_DOWN), [1700]);
    let switched = (1700..1900).find(|t| r.ticks[*t].active == Some(shotgun)).expect("switched");
    assert!((1785..=1793).contains(&switched), "switch at tick {switched}");
}

/// 11: empty from the start: attack2 or attack held a second does
/// nothing (no wind-up, no sound, full speed); no switch while held; the
/// switch after the release.
#[test]
fn empty_does_nothing_11() {
    let (mut sim, p, w, shotgun) = setup_with_previous();
    sim.app.world_mut().get_mut::<Magazine>(w).unwrap().clip = 0;
    let mut r = Rec::new(&sim, p, w);
    r.run(&mut sim, 140, |_, t| (t >= 70, t < 70));
    assert!(r.all_starts().is_empty(), "{:?}", r.all_starts());
    assert!(r.ticks.iter().all(|k| k.state == State::Idle && k.active == Some(w)));
    assert_eq!(sim.app.world().get::<Weapon>(w).unwrap().max_speed, None);
    r.run(&mut sim, 3, |_, _| (false, false));
    assert_eq!(r.ticks.last().unwrap().active, Some(shotgun), "switched once let go");
}

/// 12: dry, attack held, ammo arrives: the next tick firing (the fire
/// loop replaces the empty one), shots from the tick after.
#[test]
fn ammo_while_dry_12() {
    let (mut sim, p, w) = setup();
    let mut r = Rec::new(&sim, p, w);
    r.run(&mut sim, 1500, |_, _| (true, false));
    assert_eq!(r.state(1499), State::Dry);
    sim.app.world_mut().get_mut::<Magazine>(w).unwrap().clip = 100;
    r.run(&mut sim, 20, |_, _| (true, false));
    assert_eq!(r.state(1500), State::Firing);
    assert_eq!(r.starts(FIRE), [50, 1500]);
    assert_eq!(r.stops(EMPTY), [1500]);
    assert_eq!(r.shots().iter().filter(|t| **t >= 1500).next(), Some(&1501));
}

/// 13: the crit flag on for two seconds: the crit loop replaces the fire
/// loop from its start, then the fire loop it.
#[test]
fn crit_swaps_the_loop_13() {
    let (mut sim, p, w) = setup();
    let mut r = Rec::new(&sim, p, w);
    r.run(&mut sim, 600, |s, t| {
        s.app.world_mut().get_mut::<Tf2Minigun>(w).unwrap().crit = (333..467).contains(&t);
        (true, false)
    });
    assert_eq!(r.starts(CRIT), [333]);
    assert_eq!(r.stops(FIRE), [333]);
    assert_eq!(r.starts(FIRE), [50, 467]);
    assert_eq!(r.stops(CRIT), [467]);
}

/// 14: held 10 s: the fire loop started once at tick 50 and never
/// stopped or started again (it loops by its WAV).
#[test]
fn the_fire_loop_starts_once_14() {
    let (mut sim, p, w) = setup();
    let mut r = Rec::new(&sim, p, w);
    r.run(&mut sim, 667, |_, _| (true, false));
    assert_eq!(r.starts(FIRE), [50]);
    assert!(r.stops(FIRE).is_empty());
}

/// 15, 16: the barrel 0.1 rad/s faster a tick from the wind-up (5.0 at
/// tick 50, 20 at 200, the angle 0.0015 n(n+1)/2), then 0.1 slower a tick
/// from the wind-down, stopped 200 ticks on with the wind-down cut.
#[test]
fn the_barrel_by_tick_15_16() {
    let (mut sim, p, w) = setup();
    let mut r = Rec::new(&sim, p, w);
    let mut speed = Vec::new();
    r.run(&mut sim, 230, |s, t| {
        speed.push(s.app.world().get::<Tf2Minigun>(w).unwrap().barrel.speed);
        (t < 30, t >= 30 && t < 200)
    });
    // `speed[n + 1]` (read before tick n + 1) is after the updates of
    // ticks 1 to n: the wind-up on tick 0 sets the target for tick 1's.
    assert!((speed[51] - 5.0).abs() < 1e-3, "{}", speed[51]);
    assert!((speed[201] - BARREL_MAX_SPEED).abs() < 1e-3, "{}", speed[201]);
    // attack2 let go after tick 199: the wind-down at 219.
    assert_eq!(r.starts(WIND_DOWN), [219]);
    r.run(&mut sim, 200, |_, _| (false, false));
    assert_eq!(r.stops(WIND_DOWN), [419]);
    assert_eq!(sim.app.world().get::<Tf2Minigun>(w).unwrap().barrel.speed, 0.0);
}

/// 17 (weapons.md test 35): attack2 let go, the gun idles 20 ticks (0.3 s)
/// after the last tick it was held; then a jump works.
#[test]
fn a_released_spin_idles_after_0_3_s_17() {
    let (mut sim, p, w) = setup();
    let mut r = Rec::new(&sim, p, w);
    r.run(&mut sim, 100, |_, _| (false, true));
    let mut rose_early = false;
    r.run(&mut sim, 19, |s, k| {
        s.intent(p).jump = k % 2 == 0;
        rose_early |= s.velocity(p).y > 0.5;
        (false, false)
    });
    assert!(!rose_early, "no jump while it still turns");
    assert_eq!(r.state(118), State::Spinning);
    r.run(&mut sim, 1, |s, _| {
        s.intent(p).jump = false;
        (false, false)
    });
    assert_eq!(r.state(119), State::Idle, "20 ticks after the last held tick 99");
    let mut rose = false;
    r.run(&mut sim, 12, |s, k| {
        s.intent(p).jump = k == 120;
        rose |= s.velocity(p).y > 0.5;
        (false, false)
    });
    assert!(rose, "a jump once idle");
    let _ = sim.app.world().get::<Health>(p);
}

/// 18: another client hears the Heavy's Minigun from its replicated state
/// (the server sends no Minigun sound): the same sounds in the same order,
/// within the update latency.
#[test]
fn others_derive_the_sounds_from_the_state_18() {
    use chimera_core::core::Intent;
    use chimera_net::{harness::NetSim, net::memory::LinkConditions};
    use chimera_sim::weapon;
    let mut sim = NetSim::new(tf2::harness::Tf2Sim, LinkConditions::default(), 5, 0, |app| {
        app.add_plugins(greybox::GreyboxMapPlugin);
    });
    sim.add_client(|_| {});
    sim.add_client(|_| {});
    sim.set_frame(tf2::game::TICK_INTERVAL);
    sim.until_joined(900);
    let theirs = sim.character_of(0).unwrap();
    let sw = weapon::give(sim.server.app.world_mut(), theirs, MINIGUN).unwrap();
    sim.server.app.world_mut().get_mut::<Inventory>(theirs).unwrap().wanted = Some(sw);
    sim.ticks(150);
    let cursor = |s: &Sim| s.app.world().resource::<Messages<PlaySound>>().get_cursor_current();
    let (mut own, mut other) = (cursor(&sim.clients[0]), cursor(&sim.clients[1]));
    let (mut heard_own, mut heard_other) = (Vec::new(), Vec::new());
    for k in 0..500 {
        let me = sim.local_player(0).unwrap();
        sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().fire = k < 200;
        sim.step();
        let minigun = |e: &PlaySound| e.entry.starts_with("tf2:Weapon_Minigun.");
        heard_own.extend(own.read(sim.clients[0].app.world().resource::<Messages<PlaySound>>()).filter(|e| minigun(e)).map(|e| (k, e.entry.clone())));
        heard_other.extend(other.read(sim.clients[1].app.world().resource::<Messages<PlaySound>>()).filter(|e| minigun(e)).map(|e| (k, e.entry.clone())));
    }
    let names = |v: &[(usize, String)]| v.iter().map(|x| x.1.clone()).collect::<Vec<_>>();
    assert_eq!(names(&heard_own), [WIND_UP, FIRE, WIND_DOWN], "{heard_own:?}");
    assert_eq!(names(&heard_other), names(&heard_own), "{heard_other:?}");
    for (a, b) in heard_own.iter().zip(&heard_other) {
        assert!(b.0.abs_diff(a.0) <= 12, "{a:?} vs {b:?}");
    }
}
