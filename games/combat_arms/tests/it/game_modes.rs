//! Combat Arms' game modes (specs/combat_arms/game_modes.md): its numbered
//! test cases for the shared rules (1-18), Team Death Match (19-26) and
//! Elimination Pro (27-33) on `CaSim` and the greybox, with the spec's
//! common setup: 12 JSF and 12 Creed `Team Death Match` points (P0..P11
//! per team, in map order), the fallback hull, `ObjectEnd` 3 s,
//! `MissionEnd` 3 s, `ObjectStart` 0 s, friendly fire off, protection
//! 3.0 / 4.0 s, respawn waits 7.0 / 5.0 s (data/modes.toml's defaults).
//!
//! Hits are `core::Damage` messages of the kind the weapon parts write
//! (bullets, blasts; health 1.0 = 100 HP), so a rifle hit of 40 is 0.40.
//! Elsewhere: case 10's One Man Army half (free_for_all.rs), case 13
//! (other_modes.rs), case 17 as S&D (search_destroy.rs) and the other
//! modes' cases (search_destroy.rs, capture_the_flag.rs, free_for_all.rs,
//! other_modes.rs). Not here: case 16's bodies (no ragdolls without
//! character models).

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Dead, FreezeTime, Health, Hitgroup, SimClock, SpawnPoint, Team};
use chimera_sim::weapon::{Inventory, Weapon};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{
    game::{
        grenades::M67,
        inventory::{self, Dropped},
        movement::{self, CaInfected, CaMovement, HitKnockback},
        rules::{
            self, CaMatch, CaMatchView, CaRuleSettings, CaStats, Phase,
            flow::{self, frozen_intent},
            scoring,
            spawns::{self, CaProtected, CaSpawnCursors},
        },
        weapons::RIFLE,
        world,
    },
    harness::{CaSim, with_mode},
};

const JSF: u8 = 1;
const CREED: u8 = 2;

/// Team `team`'s point `i` (1.5 m apart along x, a row per team).
fn point(team: u8, i: usize) -> Vec3 {
    Vec3::new(-10.0 + 1.5 * i as f32, 1.0, if team == JSF { 12.0 } else { -20.0 })
}

/// The greybox with `per_team` points a team (the greybox's own two
/// removed), playing `mode` from its start.
fn game(mode: &str, per_team: usize) -> Sim {
    let mut sim = Sim::new(CaSim, GreyboxMapPlugin);
    let w = sim.app.world_mut();
    let old: Vec<Entity> = w.query_filtered::<Entity, With<SpawnPoint>>().iter(w).collect();
    for e in old {
        w.despawn(e);
    }
    for (k, team) in [JSF, CREED].into_iter().enumerate() {
        for i in 0..per_team {
            w.spawn((
                Name::new(format!("Spawn {}", k * per_team + i)),
                SpawnPoint { team: Some(Team(team)) },
                Transform::from_translation(point(team, i)),
            ));
        }
    }
    with_mode(w, mode);
    sim
}

/// A player on `team`, somewhere off the points (the setup moves it).
fn join(sim: &mut Sim, team: u8) -> Entity {
    let p = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(p).insert(Team(team));
    p
}

fn m(sim: &Sim) -> CaMatch {
    sim.app.world().resource::<CaMatch>().clone()
}

fn phase(sim: &Sim) -> Phase {
    m(sim).phase
}

fn now(sim: &Sim) -> f64 {
    sim.app.world().resource::<SimClock>().now
}

/// Run until the round is live (at most 1 s).
fn until_live(sim: &mut Sim) {
    for _ in 0..70 {
        if phase(sim) == Phase::Live {
            return;
        }
        sim.ticks(1);
    }
    panic!("not live: {:?}", phase(sim));
}

/// Run until `phase` matches (at most `secs`).
fn until(sim: &mut Sim, secs: f64, what: impl Fn(Phase) -> bool) {
    let end = now(sim) + secs;
    while now(sim) < end {
        if what(phase(sim)) {
            return;
        }
        sim.ticks(1);
    }
    panic!("still {:?}", phase(sim));
}

/// Everyone's spawn protection ended (cases that hit fresh players).
fn unprotect_all(sim: &mut Sim) {
    let w = sim.app.world_mut();
    let all: Vec<Entity> = w.query_filtered::<Entity, With<CaProtected>>().iter(w).collect();
    for e in all {
        spawns::unprotect(w, e);
    }
}

/// A hit on `target` (health 1.0 = 100 HP) of `kind` by `attacker`.
fn hit(
    sim: &mut Sim,
    target: Entity,
    attacker: Option<Entity>,
    amount: f32,
    kind: DamageKind,
    weapon: Option<&'static str>,
) {
    sim.app.world_mut().write_message(Damage {
        target,
        attacker,
        amount,
        point: Vec3::ZERO,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Generic,
        kind,
        weapon,
        force: Vec3::ZERO,
    });
}

/// `killer` kills `victim` with a rifle round this tick.
fn kill(sim: &mut Sim, victim: Entity, killer: Entity) {
    hit(sim, victim, Some(killer), 50.0, DamageKind::Bullet, Some(RIFLE));
}

fn health(sim: &Sim, p: Entity) -> f32 {
    sim.app.world().get::<Health>(p).unwrap().current
}

fn stats(sim: &Sim, p: Entity) -> CaStats {
    sim.app.world().get::<CaStats>(p).copied().unwrap_or_default()
}

fn dead(sim: &Sim, p: Entity) -> bool {
    sim.app.world().get::<Dead>(p).is_some()
}

fn at(sim: &Sim, p: Entity) -> Vec3 {
    sim.position(p)
}

fn near(a: Vec3, b: Vec3) -> bool {
    a.xz().distance(b.xz()) < 0.05
}

fn cursors(sim: &Sim) -> [usize; 2] {
    sim.app.world().resource::<CaSpawnCursors>().0
}

fn settings(sim: &mut Sim) -> Mut<'_, CaRuleSettings> {
    sim.app.world_mut().resource_mut::<CaRuleSettings>()
}

// --- Shared

/// 1. Four JSF players spawn one after another at the round's setup:
/// P1, P2, P3, P4; the JSF cursor ends at 4, Creed's is unaffected.
#[test]
fn case_01_spawn_cycling() {
    let mut sim = game("TDM", 12);
    let jsf: Vec<Entity> = (0..4).map(|_| join(&mut sim, JSF)).collect();
    until_live(&mut sim);
    for (k, p) in jsf.iter().enumerate() {
        assert!(near(at(&sim, *p), point(JSF, k + 1)), "player {k} at {}", at(&sim, *p));
    }
    assert_eq!(cursors(&sim), [4, 0]);
}

/// 2. Cursor at 2, a living player standing on P3: the next JSF spawn
/// takes P4 (P3 skipped); cursor 4.
#[test]
fn case_02_occupied_skip() {
    let mut sim = game("TDM", 12);
    let p = join(&mut sim, JSF);
    let blocker = join(&mut sim, CREED);
    until_live(&mut sim);
    sim.app.world_mut().get_mut::<Transform>(blocker).unwrap().translation = point(JSF, 3);
    sim.app.world_mut().resource_mut::<CaSpawnCursors>().0[0] = 2;
    spawns::respawn(sim.app.world_mut(), p);
    assert!(near(at(&sim, p), point(JSF, 4)), "at {}", at(&sim, p));
    assert_eq!(cursors(&sim)[0], 4);
}

/// 3. The occupied test's bounds (world units, the fallback hull): a
/// player at the origin occupies a point at 29.9 on x but not 30.0, at
/// 99.9 on y but not 100.0; and through the respawn (metres).
#[test]
fn case_03_occupied_bounds() {
    assert!(spawns::occupied(Vec3::new(29.9, 0.0, 0.0)));
    assert!(!spawns::occupied(Vec3::new(30.0, 0.0, 0.0)));
    assert!(spawns::occupied(Vec3::new(0.0, 99.9, 0.0)));
    assert!(!spawns::occupied(Vec3::new(0.0, 100.0, 0.0)));
    // A blocker 29.9 u off P1: the spawn skips to P2; 30.1 u off: P1.
    for (off, want) in [(29.9, 2), (30.1, 1)] {
        let mut sim = game("TDM", 12);
        let p = join(&mut sim, JSF);
        let blocker = join(&mut sim, CREED);
        until_live(&mut sim);
        let to = point(JSF, 1) + Vec3::Z * off * inventory::UNIT;
        sim.app.world_mut().get_mut::<Transform>(blocker).unwrap().translation = to;
        sim.app.world_mut().resource_mut::<CaSpawnCursors>().0[0] = 0;
        spawns::respawn(sim.app.world_mut(), p);
        assert!(near(at(&sim, p), point(JSF, want)), "{off} u: at {}", at(&sim, p));
    }
}

/// 4. Two points, cursor 0, both occupied by unprotected players: the
/// spawn tries P1 and P0 and uses P0; the player on P0 dies by crush,
/// credited as its own suicide; cursor 0.
#[test]
fn case_04_all_occupied_telefrag() {
    let mut sim = game("TDM", 2);
    let p = join(&mut sim, JSF);
    let on0 = join(&mut sim, CREED);
    let on1 = join(&mut sim, CREED);
    until_live(&mut sim);
    unprotect_all(&mut sim);
    let w = sim.app.world_mut();
    w.get_mut::<Transform>(on0).unwrap().translation = point(JSF, 0);
    w.get_mut::<Transform>(on1).unwrap().translation = point(JSF, 1);
    w.resource_mut::<CaSpawnCursors>().0[0] = 0;
    spawns::respawn(w, p);
    assert!(near(at(&sim, p), point(JSF, 0)), "at {}", at(&sim, p));
    assert_eq!(cursors(&sim)[0], 0);
    sim.ticks(1);
    assert!(dead(&sim, on0), "telefragged");
    assert!(!dead(&sim, on1));
    let s = stats(&sim, on0);
    assert_eq!((s.deaths, s.suicides, s.friendly_kills), (1, 1, 1), "its own suicide");
    assert_eq!(m(&sim).score, [1, 0], "TDM: a point for the victim's enemy");
    assert!(!dead(&sim, p), "the spawned player is protected");
}

/// 5. Cursor at 11 (the last JSF point): the next spawn takes P0.
#[test]
fn case_05_wrap() {
    let mut sim = game("TDM", 12);
    let p = join(&mut sim, JSF);
    until_live(&mut sim);
    sim.app.world_mut().resource_mut::<CaSpawnCursors>().0[0] = 11;
    spawns::respawn(sim.app.world_mut(), p);
    assert!(near(at(&sim, p), point(JSF, 0)));
    assert_eq!(cursors(&sim)[0], 0);
}

/// 6. Spawn family: points whose `Spawn` isn't 1 (or locked, or of
/// another family) aren't used; with none left the map's first
/// `GameStartPoint` is. (Our maps keep the team modes' family, `Team
/// Death Match`: the case's `Doobic` points stand in as another family.)
#[test]
fn case_06_spawn_family() {
    use lithtech_formats::world::{Object, Property, Value, World};
    let prop = |name: &str, value: Value| Property {
        name: name.into(),
        flags: 0,
        len: 0,
        value,
    };
    let start = |x: f32, family: &str, spawn: i32, locked: bool| Object {
        class: "GameStartPoint".into(),
        properties: vec![
            prop("Pos", Value::Vector([x, 66.0, 0.0])),
            prop("GameType", Value::String(family.into())),
            prop("Teams", Value::String("JSF".into())),
            prop("Spawn", Value::Long(spawn as f32)),
            prop("Locked", Value::Bool(locked)),
        ],
    };
    assert!(world::qualifies(
        &start(0.0, "Team Death Match", 1, false),
        world::SPAWN_GAME_TYPE
    ));
    assert!(!world::qualifies(
        &start(0.0, "Team Death Match", 2, false),
        world::SPAWN_GAME_TYPE
    ));
    assert!(!world::qualifies(
        &start(0.0, "Team Death Match", 1, true),
        world::SPAWN_GAME_TYPE
    ));
    assert!(!world::qualifies(
        &start(0.0, "Doobic", 1, false),
        world::SPAWN_GAME_TYPE
    ));
    // A map with only Spawn 2 points: the first start point.
    let mut w = World::default();
    w.objects.push(start(100.0, "Doobic", 2, false));
    w.objects.push(start(200.0, "Team Death Match", 2, false));
    let map = world::convert(&w, "family", 0.01, &|_| None);
    assert_eq!(map.spawns.len(), 1);
    assert!(
        (map.spawns[0].0.x - 1.0).abs() < 1e-4,
        "the first point: {}",
        map.spawns[0].0
    );
    // With a qualifying point, only it.
    w.objects.push(start(300.0, "Team Death Match", 1, false));
    let map = world::convert(&w, "family", 0.01, &|_| None);
    assert_eq!(map.spawns.len(), 1);
    assert!((map.spawns[0].0.x - 3.0).abs() < 1e-4, "{}", map.spawns[0].0);
}

/// 7. TDM, a player dies at t = 0: it asks at the first frame after
/// 7.000 s, the server accepts (more than 5.0 s), and it is alive at
/// once at the next team point with full health and the loadout rebuilt.
#[test]
fn case_07_respawn_timing() {
    let mut sim = game("TDM", 12);
    let p = join(&mut sim, JSF);
    let enemy = join(&mut sim, CREED);
    until_live(&mut sim);
    unprotect_all(&mut sim);
    kill(&mut sim, p, enemy);
    sim.ticks(1);
    assert!(dead(&sim, p));
    let died = sim.app.world().get::<Dead>(p).unwrap().since;
    while now(&sim) - died <= 7.0 + 1e-9 {
        assert!(dead(&sim, p), "dead until 7 s: {:.3}", now(&sim) - died);
        sim.ticks(1);
    }
    assert!(!dead(&sim, p), "alive at the first tick after 7 s");
    assert!(now(&sim) - died < 7.0 + 0.016);
    assert_eq!(health(&sim, p), 1.0);
    assert!(near(at(&sim, p), point(JSF, 2)), "the next point: {}", at(&sim, p));
    sim.ticks(2);
    let w = sim.app.world();
    let ids: Vec<&str> = w
        .get::<Inventory>(p)
        .unwrap()
        .weapons
        .iter()
        .map(|e| w.get::<Weapon>(*e).unwrap().id)
        .collect();
    assert_eq!(ids, inventory::STARTING_WEAPONS, "the loadout rebuilt");
}

/// 8. A request 4.9 s after the death is refused; the player stays dead
/// (the client doesn't ask again).
#[test]
fn case_08_early_request() {
    let mut sim = game("TDM", 12);
    let p = join(&mut sim, JSF);
    let enemy = join(&mut sim, CREED);
    until_live(&mut sim);
    unprotect_all(&mut sim);
    kill(&mut sim, p, enemy);
    sim.ticks(1);
    let died = sim.app.world().get::<Dead>(p).unwrap().since;
    while now(&sim) - died < 4.9 - 1e-9 {
        sim.ticks(1);
    }
    assert!(spawns::request_respawn(sim.app.world_mut(), p).is_err(), "refused");
    sim.seconds(10.0);
    assert!(dead(&sim, p), "still dead");
}

/// 9. Protection: a rifle hit at 2.9 s and an M67 blast at 3.5 s do
/// nothing; a rifle hit at 4.05 s lands.
#[test]
fn case_09_protection() {
    let mut sim = game("TDM", 12);
    let p = join(&mut sim, JSF);
    let enemy = join(&mut sim, CREED);
    until_live(&mut sim);
    let since = sim.app.world().get::<CaProtected>(p).unwrap().since;
    let wait_to = |sim: &mut Sim, t: f64| {
        while now(sim) - since < t - 1e-9 {
            sim.ticks(1);
        }
    };
    wait_to(&mut sim, 2.9);
    hit(&mut sim, p, Some(enemy), 0.40, DamageKind::Bullet, Some(RIFLE));
    sim.ticks(1);
    assert_eq!(health(&sim, p), 1.0, "2.9 s: no damage");
    wait_to(&mut sim, 3.5);
    hit(&mut sim, p, Some(enemy), 0.80, DamageKind::Blast, Some(M67));
    sim.ticks(1);
    assert_eq!(health(&sim, p), 1.0, "3.5 s: no damage");
    wait_to(&mut sim, 4.05);
    hit(&mut sim, p, Some(enemy), 0.40, DamageKind::Bullet, Some(RIFLE));
    sim.ticks(1);
    assert!((health(&sim, p) - 0.6).abs() < 1e-5, "4.05 s: {}", health(&sim, p));
}

/// 10 (TDM half). A protected player who fires at 1.0 s and is hit at
/// 1.1 s takes no damage: firing ends protection only in the
/// free-for-all modes.
#[test]
fn case_10_firing_keeps_protection_in_tdm() {
    let mut sim = game("TDM", 12);
    let p = join(&mut sim, JSF);
    let enemy = join(&mut sim, CREED);
    until_live(&mut sim);
    sim.seconds(1.0);
    sim.intent(p).fire = true;
    sim.seconds(0.1);
    sim.intent(p).fire = false;
    hit(&mut sim, p, Some(enemy), 0.40, DamageKind::Bullet, Some(RIFLE));
    sim.ticks(1);
    assert_eq!(health(&sim, p), 1.0);
    assert!(sim.app.world().get::<CaProtected>(p).is_some());
}

/// 11 and 12. Friendly fire in TDM: on, a teammate's rifle 40 → 10, its
/// M67 80 → 80, your own M67 80 → 80; off, 0, 0 and 80.
#[test]
fn case_11_12_friendly_fire() {
    for (on, want) in [(1, [0.10, 0.80, 0.80]), (0, [0.0, 0.0, 0.80])] {
        let mut sim = game("TDM", 12);
        settings(&mut sim).friendly_fire = on;
        let a = join(&mut sim, JSF);
        let mate = join(&mut sim, JSF);
        until_live(&mut sim);
        unprotect_all(&mut sim);
        let cases = [
            (Some(mate), 0.40, DamageKind::Bullet, RIFLE),
            (Some(mate), 0.80, DamageKind::Blast, M67),
            (Some(a), 0.80, DamageKind::Blast, M67),
        ];
        for ((attacker, amount, kind, weapon), want) in cases.into_iter().zip(want) {
            sim.app.world_mut().get_mut::<Health>(a).unwrap().current = 1.0;
            hit(&mut sim, a, attacker, amount, kind, Some(weapon));
            sim.ticks(1);
            let took = 1.0 - health(&sim, a);
            assert!((took - want).abs() < 1e-5, "ff {on}, {kind:?} {amount}: took {took}");
        }
    }
}

/// 14. Assists: B (100 HP) takes 60 from A, then C kills B: A +1 assist,
/// C +1 kill, B +1 death. 49 from A: no assist.
#[test]
fn case_14_assist() {
    for (dealt, assists) in [(0.60, 1), (0.49, 0)] {
        let mut sim = game("TDM", 12);
        let a = join(&mut sim, JSF);
        let c = join(&mut sim, JSF);
        let b = join(&mut sim, CREED);
        until_live(&mut sim);
        unprotect_all(&mut sim);
        hit(&mut sim, b, Some(a), dealt, DamageKind::Bullet, Some(RIFLE));
        sim.ticks(1);
        kill(&mut sim, b, c);
        sim.ticks(1);
        assert_eq!(stats(&sim, a).assists, assists, "{dealt}");
        assert_eq!(stats(&sim, c).kills, 1);
        assert_eq!(stats(&sim, b).deaths, 1);
    }
}

/// 15. A deals 60 to B; an enemy of A heals B for 15 (sum 45); C kills
/// B: no assist.
#[test]
fn case_15_assist_cleared_by_heal() {
    let mut sim = game("TDM", 12);
    let a = join(&mut sim, JSF);
    let c = join(&mut sim, JSF);
    let b = join(&mut sim, CREED);
    let medic = join(&mut sim, CREED);
    until_live(&mut sim);
    unprotect_all(&mut sim);
    hit(&mut sim, b, Some(a), 0.60, DamageKind::Bullet, Some(RIFLE));
    sim.ticks(1);
    scoring::heal(sim.app.world_mut(), medic, b, 0.15);
    kill(&mut sim, b, c);
    sim.ticks(1);
    assert_eq!(stats(&sim, a).assists, 0);
    assert_eq!(stats(&sim, c).kills, 1);
}

/// 16. Round cleanup: rifles on the ground (three dropped, the victim's
/// death drop, and a map-placed shotgun's stand-in) when the deciding kill happens at t = 0 are all
/// gone at t = 3.0 s plus a frame, not before.
#[test]
fn case_16_round_cleanup() {
    let mut sim = game("TDM", 12);
    settings(&mut sim).goal = 1;
    let a = join(&mut sim, JSF);
    let b = join(&mut sim, CREED);
    until_live(&mut sim);
    unprotect_all(&mut sim);
    let w = sim.app.world_mut();
    for i in 0..3 {
        inventory::spawn_item(w, RIFLE, Vec3::new(i as f32, 0.5, 0.0), 90, 30).unwrap();
    }
    inventory::spawn_item(w, "combat_arms:remington_870", Vec3::new(4.0, 0.5, 0.0), 8, 8).unwrap();
    let items = |sim: &mut Sim| {
        let w = sim.app.world_mut();
        w.query::<&Dropped>().iter(w).count()
    };
    assert_eq!(items(&mut sim), 4);
    kill(&mut sim, b, a);
    sim.ticks(1);
    let t0 = now(&sim);
    assert_eq!(items(&mut sim), 5, "and the victim's death drop");
    sim.seconds(2.9);
    assert_eq!(items(&mut sim), 5, "still there before the round's end");
    while now(&sim) - t0 < 3.0 + 0.05 + 0.05 {
        sim.ticks(1);
    }
    assert_eq!(items(&mut sim), 0, "all gone at the round's end");
}

/// 17 (as TDM). A grenade death after the round is over counts for
/// nothing; the round's result is unchanged.
#[test]
fn case_17_grenade_after_round_over() {
    let mut sim = game("TDM", 12);
    settings(&mut sim).goal = 1;
    let a = join(&mut sim, JSF);
    let b = join(&mut sim, CREED);
    let b2 = join(&mut sim, CREED);
    until_live(&mut sim);
    unprotect_all(&mut sim);
    kill(&mut sim, b, a);
    until(&mut sim, 1.0, |p| matches!(p, Phase::RoundOver { .. }));
    sim.seconds(0.5);
    hit(&mut sim, a, Some(b2), 5.0, DamageKind::Blast, Some(M67));
    sim.ticks(1);
    assert!(dead(&sim, a), "the blast kills");
    assert_eq!(m(&sim).score, [1, 0]);
    assert_eq!(stats(&sim, b2).kills, 0);
    assert_eq!(stats(&sim, a).deaths, 0);
    assert_eq!(m(&sim).results.last().unwrap().winner, Some(JSF));
}

/// 18. Knock-back on an infected player (k = 135, d = 0.4 s): the push
/// replaces the acceleration, k² (1 − t) along the hit's horizontal
/// direction ((3, 1, 4): (0.6, 0, 0.8)), over at 0.4 s; crouched and
/// still, no velocity. (The grenade side, whom it applies to, is
/// grenades.md's.)
#[test]
fn case_18_knock_back() {
    let mut sim = Sim::new(CaSim, GreyboxMapPlugin);
    let p = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.seconds(0.5);
    let k = HitKnockback::hit(None, 135.0, 0.4, 0.0, Vec3::new(3.0, 1.0, 4.0), now(&sim)).unwrap();
    sim.app.world_mut().entity_mut(p).insert((CaInfected, k));
    sim.ticks(1);
    let started = sim.app.world().get::<HitKnockback>(p).unwrap().started.unwrap();
    sim.seconds(0.1);
    let t = (now(&sim) - started) as f32;
    let a = sim.app.world().get::<CaMovement>(p).unwrap().accel;
    let want = 135.0f32 * 135.0 * (1.0 - t);
    assert!(
        (a.length() - want).abs() < want * 1e-3,
        "|a| {} want {want} at t {t}",
        a.length()
    );
    assert!(
        a.y.abs() < 1e-3 && (a.x / a.length() - 0.6).abs() < 1e-3 && (a.z / a.length() - 0.8).abs() < 1e-3,
        "{a}"
    );
    sim.seconds(0.4);
    assert_eq!(sim.app.world().get::<CaMovement>(p).unwrap().accel, Vec3::ZERO, "over");
    // Crouched and still: no velocity.
    let k = HitKnockback::hit(None, 135.0, 0.4, 0.0, Vec3::new(3.0, 1.0, 4.0), now(&sim)).unwrap();
    sim.intent(p).crouch = true;
    sim.app.world_mut().entity_mut(p).insert(k);
    sim.ticks(3);
    assert_eq!(sim.velocity(p).xz(), Vec2::ZERO);
}

// --- M1 Team Death Match

/// 19. G = 100, Alpha 99, Bravo 50; an Alpha player kills a Bravo player
/// at t = 0: Alpha 100, round over at the next frame (winner Alpha),
/// round end at about 3.05 s, the match's result after `MissionEnd`.
#[test]
fn case_19_win() {
    let mut sim = game("TDM", 12);
    let a = join(&mut sim, JSF);
    let b = join(&mut sim, CREED);
    until_live(&mut sim);
    unprotect_all(&mut sim);
    sim.app.world_mut().resource_mut::<CaMatch>().score = [99, 50];
    kill(&mut sim, b, a);
    sim.ticks(1);
    let t0 = now(&sim);
    assert_eq!(m(&sim).score, [100, 50]);
    assert_eq!(m(&sim).winner, Some(JSF));
    until(&mut sim, 0.1, |p| matches!(p, Phase::RoundOver { .. }));
    assert!(now(&sim) - t0 <= 0.05 + 1e-6, "the next frame");
    until(&mut sim, 3.2, |p| matches!(p, Phase::Intermission { .. }));
    let ended = now(&sim) - t0;
    assert!((3.0..=3.11).contains(&ended), "round end at {ended:.3} s");
    until(&mut sim, 3.2, |p| p == Phase::MatchOver);
    let over = now(&sim) - t0;
    assert!((6.0..=6.16).contains(&over), "result at {over:.3} s");
    assert_eq!(m(&sim).result, Some(Some(JSF)));
    assert_eq!(stats(&sim, a).kills, 1);
}

/// 20. A team kill: Alpha 10, Bravo 10; an Alpha player kills an Alpha
/// teammate: Alpha 10, Bravo 11; the killer's friendly kills +1, the
/// victim's deaths +1, the killer's kills unchanged.
#[test]
fn case_20_team_kill() {
    let mut sim = game("TDM", 12);
    settings(&mut sim).friendly_fire = 1;
    let a = join(&mut sim, JSF);
    let mate = join(&mut sim, JSF);
    join(&mut sim, CREED);
    until_live(&mut sim);
    unprotect_all(&mut sim);
    sim.app.world_mut().resource_mut::<CaMatch>().score = [10, 10];
    kill(&mut sim, mate, a);
    sim.ticks(1);
    assert!(dead(&sim, mate));
    assert_eq!(m(&sim).score, [10, 11]);
    assert_eq!((stats(&sim, a).friendly_kills, stats(&sim, a).kills), (1, 0));
    assert_eq!(stats(&sim, mate).deaths, 1);
}

/// 21. A fall death (the movement's own crush damage, no weapon): Bravo
/// +1; the victim's deaths and suicides +1.
#[test]
fn case_21_fall_death() {
    let mut sim = game("TDM", 12);
    let a = join(&mut sim, JSF);
    join(&mut sim, CREED);
    until_live(&mut sim);
    unprotect_all(&mut sim);
    hit(&mut sim, a, Some(a), 2.0, DamageKind::Crush, None);
    sim.ticks(1);
    assert!(dead(&sim, a));
    assert_eq!(m(&sim).score, [0, 1]);
    let s = stats(&sim, a);
    assert_eq!((s.deaths, s.suicides, s.friendly_kills), (1, 1, 0));
}

/// 22. A Bravo player's own grenade kills it: Alpha +1; its deaths,
/// suicides and friendly kills +1.
#[test]
fn case_22_grenade_suicide() {
    let mut sim = game("TDM", 12);
    join(&mut sim, JSF);
    let b = join(&mut sim, CREED);
    until_live(&mut sim);
    unprotect_all(&mut sim);
    hit(&mut sim, b, Some(b), 2.0, DamageKind::Blast, Some(M67));
    sim.ticks(1);
    assert_eq!(m(&sim).score, [1, 0]);
    let s = stats(&sim, b);
    assert_eq!((s.deaths, s.suicides, s.friendly_kills), (1, 1, 1));
}

/// 23. Two kills in one frame at the limit: Alpha 99, Bravo 99; Alpha
/// kills (100), then Bravo kills (100): the winner stays Alpha, both
/// show 100.
#[test]
fn case_23_two_kills_at_the_limit() {
    let mut sim = game("TDM", 12);
    let a = join(&mut sim, JSF);
    let a2 = join(&mut sim, JSF);
    let b = join(&mut sim, CREED);
    let b2 = join(&mut sim, CREED);
    until_live(&mut sim);
    unprotect_all(&mut sim);
    sim.app.world_mut().resource_mut::<CaMatch>().score = [99, 99];
    kill(&mut sim, b, a);
    kill(&mut sim, a2, b2);
    sim.ticks(1);
    assert_eq!(m(&sim).score, [100, 100]);
    assert_eq!(m(&sim).winner, Some(JSF));
    until(&mut sim, 0.1, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).results.last().unwrap().winner, Some(JSF));
}

/// 24. Bravo's only player disconnects: Alpha wins at once; round over
/// at the next frame.
#[test]
fn case_24_last_player_leaves() {
    let mut sim = game("TDM", 12);
    join(&mut sim, JSF);
    let b = join(&mut sim, CREED);
    until_live(&mut sim);
    sim.app.world_mut().despawn(b);
    assert_eq!(m(&sim).winner, Some(JSF), "at once");
    until(&mut sim, 0.1, |p| matches!(p, Phase::RoundOver { .. }));
    until(&mut sim, 7.0, |p| p == Phase::MatchOver);
    assert_eq!(m(&sim).result, Some(Some(JSF)));
}

/// 25. After case 19's deciding kill, a Bravo player kills an Alpha
/// player at t = 1.0 s: the scores stay 100 - 50, no kill or death added.
#[test]
fn case_25_kill_during_round_over() {
    let mut sim = game("TDM", 12);
    let a = join(&mut sim, JSF);
    let b = join(&mut sim, CREED);
    let b2 = join(&mut sim, CREED);
    until_live(&mut sim);
    unprotect_all(&mut sim);
    sim.app.world_mut().resource_mut::<CaMatch>().score = [99, 50];
    kill(&mut sim, b, a);
    sim.ticks(1);
    sim.seconds(1.0);
    assert!(matches!(phase(&sim), Phase::RoundOver { .. }));
    kill(&mut sim, a, b2);
    sim.ticks(1);
    assert!(dead(&sim, a));
    assert_eq!(m(&sim).score, [100, 50]);
    assert_eq!((stats(&sim, b2).kills, stats(&sim, a).deaths), (0, 0));
}

/// 26. No timer: an hour live with no kills is still live, with no round
/// timer to show.
#[test]
fn case_26_no_timer() {
    let mut sim = game("TDM", 12);
    join(&mut sim, JSF);
    join(&mut sim, CREED);
    until_live(&mut sim);
    assert_eq!(m(&sim).round_ends, None);
    sim.set_tick_interval(0.5);
    sim.seconds(3600.0);
    assert_eq!(phase(&sim), Phase::Live);
    let view = sim.app.world().resource::<CaMatchView>();
    assert_eq!(view.round_ends, None);
    assert!(!view.mode().unwrap().timer_shown);
}

/// TDM's match flow: the first player starts it, the setup is one frame
/// with movement held, and the rule module playing is `combat_arms:tdm`.
#[test]
fn tdm_starts_with_a_one_frame_setup() {
    let mut sim = game("TDM", 12);
    assert_eq!(chimera_sim::rule_module::pick(sim.app.world()), Some(rules::TDM));
    assert_eq!(phase(&sim), Phase::Waiting);
    sim.seconds(0.2);
    assert_eq!(phase(&sim), Phase::Waiting, "nobody in");
    join(&mut sim, JSF);
    until(&mut sim, 0.2, |p| matches!(p, Phase::Setup { .. }));
    assert!(sim.app.world().resource::<FreezeTime>().0);
    let i = chimera_core::core::Intent {
        move_axis: Vec2::Y,
        jump: true,
        fire: true,
        ..default()
    };
    let held = frozen_intent(&i);
    assert_eq!((held.move_axis, held.jump, held.fire), (Vec2::ZERO, false, true));
    until(&mut sim, 0.1, |p| p == Phase::Live);
    assert!(!sim.app.world().resource::<FreezeTime>().0);
}

// --- M2 Elimination Pro

/// Elimination Pro's match past its warm-up, at round 1's live phase.
fn elimination(per_side: usize) -> (Sim, Vec<Entity>, Vec<Entity>) {
    let mut sim = game("TSV", 12);
    let alpha: Vec<Entity> = (0..per_side).map(|_| join(&mut sim, JSF)).collect();
    let bravo: Vec<Entity> = (0..per_side).map(|_| join(&mut sim, CREED)).collect();
    until_live(&mut sim);
    assert!(m(&sim).warmup);
    until(&mut sim, 20.0, |p| matches!(p, Phase::Setup { .. }));
    until_live(&mut sim);
    assert!(!m(&sim).warmup);
    unprotect_all(&mut sim);
    (sim, alpha, bravo)
}

/// 27. The warm-up: live for 10 s; a death at 2 s adds nothing and the
/// player is revived by 5 s; round 1 starts about 6 s after it ends.
#[test]
fn case_27_warm_up() {
    let mut sim = game("TSV", 12);
    let a = join(&mut sim, JSF);
    let b = join(&mut sim, CREED);
    until_live(&mut sim);
    let t0 = now(&sim);
    assert!(m(&sim).warmup);
    sim.seconds(2.0);
    unprotect_all(&mut sim);
    kill(&mut sim, b, a);
    sim.ticks(1);
    assert!(dead(&sim, b));
    assert_eq!((stats(&sim, a).kills, stats(&sim, b).deaths), (0, 0), "nothing counts");
    while now(&sim) - t0 < 5.0 + 0.05 {
        sim.ticks(1);
    }
    assert!(!dead(&sim, b), "revived by 5 s");
    until(&mut sim, 9.0, |p| matches!(p, Phase::RoundOver { .. }));
    let ended = now(&sim) - t0;
    assert!((10.0..=10.11).contains(&ended), "warm-up over at {ended:.3}");
    assert_eq!(m(&sim).results[0].winner, None);
    let t1 = now(&sim);
    until(&mut sim, 7.0, |p| p == Phase::Live);
    let gap = now(&sim) - t1;
    assert!((6.0..=6.21).contains(&gap), "round 1 after {gap:.3} s");
    assert!(!m(&sim).warmup);
    assert_eq!(m(&sim).round, 0, "the warm-up doesn't count");
}

/// 28. 3 v 3; Bravo loses its third player: Alpha's round wins +1, the
/// next round about 6 s later.
#[test]
fn case_28_elimination() {
    let (mut sim, alpha, bravo) = elimination(3);
    for b in &bravo {
        kill(&mut sim, *b, alpha[0]);
        sim.ticks(1);
    }
    until(&mut sim, 0.1, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).round_wins, [1, 0]);
    let t0 = now(&sim);
    until(&mut sim, 7.0, |p| p == Phase::Live);
    let gap = now(&sim) - t0;
    assert!((6.0..=6.21).contains(&gap), "{gap:.3}");
    assert_eq!(m(&sim).round, 1);
    assert!(bravo.iter().all(|b| !dead(&sim, *b)), "everyone back for the round");
}

/// 29. Time-out with Alpha 3 alive, Bravo 2: Alpha wins the round.
#[test]
fn case_29_time_out_by_alive_count() {
    let (mut sim, alpha, bravo) = elimination(3);
    kill(&mut sim, bravo[0], alpha[0]);
    sim.ticks(1);
    until(&mut sim, 181.0, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).results.last().unwrap().winner, Some(JSF));
    assert_eq!(m(&sim).round_wins, [1, 0]);
}

/// 30. Time-out tie-break: 2 v 2 alive, round kills 3 - 3, round deaths
/// Alpha 3, Bravo 2: Bravo wins.
#[test]
fn case_30_time_out_tie_break() {
    let (mut sim, ..) = elimination(2);
    {
        let mut m = sim.app.world_mut().resource_mut::<CaMatch>();
        m.round_kills = [3, 3];
        m.round_deaths = [3, 2];
    }
    until(&mut sim, 181.0, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).results.last().unwrap().winner, Some(CREED));
}

/// 31. Time-out draw: alive 2 - 2, kills 1 - 1, deaths 1 - 1: no winner,
/// win counts unchanged, the round number +1.
#[test]
fn case_31_time_out_draw() {
    let (mut sim, ..) = elimination(2);
    {
        let mut m = sim.app.world_mut().resource_mut::<CaMatch>();
        m.round_kills = [1, 1];
        m.round_deaths = [1, 1];
    }
    until(&mut sim, 181.0, |p| matches!(p, Phase::RoundOver { .. }));
    assert_eq!(m(&sim).results.last().unwrap().winner, None);
    assert_eq!(m(&sim).round_wins, [0, 0]);
    until(&mut sim, 4.0, |p| matches!(p, Phase::Intermission { .. }));
    assert_eq!(m(&sim).round, 1);
}

/// 32. G = 7: Alpha's 7th round win ends the match, Alpha the winner.
#[test]
fn case_32_match_end() {
    let (mut sim, alpha, bravo) = elimination(1);
    sim.app.world_mut().resource_mut::<CaMatch>().round_wins = [6, 2];
    kill(&mut sim, bravo[0], alpha[0]);
    sim.ticks(1);
    until(&mut sim, 7.0, |p| p == Phase::MatchOver);
    assert_eq!(m(&sim).round_wins, [7, 2]);
    assert_eq!(m(&sim).result, Some(Some(JSF)));
}

/// 33. Late join: 6 players, round live; a 7th joins: held until the next
/// round's setup. With 2 players in the joiner makes 3 (Corrections
/// 2026-10-22, 3): held too, once a second of live play has passed. A joiner
/// 0.6 s after the round went live spawns at once.
#[test]
fn case_33_late_join() {
    for (per_side, wait, held) in [(3, 3, true), (1, 3, true), (1, 4, false)] {
        let (mut sim, ..) = elimination(per_side);
        sim.app.world_mut().resource_mut::<CaRuleSettings>().late_join_wait = wait;
        sim.seconds(30.0);
        let late = join(&mut sim, JSF);
        sim.app
            .world_mut()
            .entity_mut(late)
            .insert(chimera_core::core::Connecting);
        flow::enter_game(sim.app.world_mut(), late);
        sim.ticks(1);
        assert_eq!(dead(&sim, late), held, "{} in, wait {wait}", 2 * per_side);
        if held {
            sim.app.world_mut().resource_mut::<CaMatch>().round_ends = Some(now(&sim));
            until(&mut sim, 8.0, |p| matches!(p, Phase::Setup { .. }));
            sim.ticks(1);
            assert!(!dead(&sim, late), "in with everyone at the setup");
        }
    }
    let (mut sim, ..) = elimination(3);
    sim.seconds(0.6);
    let late = join(&mut sim, JSF);
    sim.app
        .world_mut()
        .entity_mut(late)
        .insert(chimera_core::core::Connecting);
    flow::enter_game(sim.app.world_mut(), late);
    sim.ticks(1);
    assert!(!dead(&sim, late), "0.6 s after going live: at once");
}

/// Case 42's Elimination Pro half: a team that's empty at a round's end
/// ends the match, the other team the winner.
#[test]
fn an_empty_team_ends_elimination_pro() {
    let (mut sim, _alpha, bravo) = elimination(1);
    sim.app.world_mut().resource_mut::<CaMatch>().round_wins = [3, 3];
    sim.app.world_mut().despawn(bravo[0]);
    until(&mut sim, 7.0, |p| p == Phase::MatchOver);
    assert_eq!(m(&sim).result, Some(Some(JSF)));
}
