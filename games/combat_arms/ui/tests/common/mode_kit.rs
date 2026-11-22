//! What the game mode scenario tests share (specs/combat_arms/
//! game_modes.md's common setup): the greybox with a row of start points
//! per team, players joining a team, hits and kills as `core::Damage`
//! messages (health 1.0 = 100 HP), the match's state, and map objects
//! (bomb sites, flags, flag zones, triggers, drop points, start points)
//! put into `CaMapObjects` as `game::world` would read them.

#![allow(dead_code)]

use avian3d::prelude::Position;
use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Dead, Health, Hitgroup, SimClock, SpawnPoint, Team};
use chimera_sim::weapon::{Inventory, Weapon};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{
    game::{
        inventory::UNIT,
        movement,
        rules::{
            CaMatch, CaRuleSettings, CaStats, Phase,
            objectives::{CaAction, CaCarry, CaObjectives},
            objects::{BombZone, CaMapObjects, DropPoint, FlagObject, FlagZone, ObjBox, StartPoint, UseTrigger},
            spawns::{self, CaProtected},
        },
        weapons::RIFLE,
    },
    harness::{CaSim, with_mode},
};

pub const JSF: u8 = 1;
pub const CREED: u8 = 2;

/// World units to metres.
pub fn u(x: f32) -> f32 {
    x * UNIT
}

/// Team `team`'s point `i` (1.5 m apart along x, a row per team).
pub fn point(team: u8, i: usize) -> Vec3 {
    Vec3::new(-10.0 + 1.5 * i as f32, 1.0, if team == JSF { 12.0 } else { -20.0 })
}

/// The greybox with `per_team` points a team (the greybox's own two
/// removed), playing `mode` from its start.
pub fn game(mode: &str, per_team: usize) -> Sim {
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
pub fn join(sim: &mut Sim, team: u8) -> Entity {
    let p = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(p).insert(Team(team));
    p
}

pub fn m(sim: &Sim) -> CaMatch {
    sim.app.world().resource::<CaMatch>().clone()
}

pub fn phase(sim: &Sim) -> Phase {
    m(sim).phase
}

pub fn now(sim: &Sim) -> f64 {
    sim.app.world().resource::<SimClock>().now
}

pub fn objectives(sim: &Sim) -> CaObjectives {
    sim.app.world().resource::<CaObjectives>().clone()
}

pub fn settings(sim: &mut Sim) -> Mut<'_, CaRuleSettings> {
    sim.app.world_mut().resource_mut::<CaRuleSettings>()
}

pub fn objects(sim: &mut Sim) -> Mut<'_, CaMapObjects> {
    sim.app.world_mut().resource_mut::<CaMapObjects>()
}

/// Run until the round is live (at most 1 s).
pub fn until_live(sim: &mut Sim) {
    for _ in 0..70 {
        if phase(sim) == Phase::Live {
            return;
        }
        sim.ticks(1);
    }
    panic!("not live: {:?}", phase(sim));
}

/// Run until `phase` matches (at most `secs`).
pub fn until(sim: &mut Sim, secs: f64, what: impl Fn(Phase) -> bool) {
    let end = now(sim) + secs;
    while now(sim) < end {
        if what(phase(sim)) {
            return;
        }
        sim.ticks(1);
    }
    panic!("still {:?}", phase(sim));
}

/// Run until `test` holds (at most `secs`); whether it did.
pub fn wait_for(sim: &mut Sim, secs: f64, test: impl Fn(&Sim) -> bool) -> bool {
    let end = now(sim) + secs;
    while now(sim) < end {
        if test(sim) {
            return true;
        }
        sim.ticks(1);
    }
    test(sim)
}

/// Past the warm-up: at the first real round's live phase (Hunted's 20 s
/// warm-up and the 6 s between rounds the longest).
pub fn past_warmup(sim: &mut Sim) {
    until_live(sim);
    assert!(m(sim).warmup, "a warm-up first");
    until(sim, 40.0, |p| matches!(p, Phase::Setup { .. }));
    until_live(sim);
    assert!(!m(sim).warmup);
}

/// Everyone's spawn protection ended (cases that hit fresh players).
pub fn unprotect_all(sim: &mut Sim) {
    let w = sim.app.world_mut();
    let all: Vec<Entity> = w.query_filtered::<Entity, With<CaProtected>>().iter(w).collect();
    for e in all {
        spawns::unprotect(w, e);
    }
}

/// A hit on `target` (health 1.0 = 100 HP) of `kind` by `attacker`.
pub fn hit(
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
pub fn kill(sim: &mut Sim, victim: Entity, killer: Entity) {
    hit(sim, victim, Some(killer), 50.0, DamageKind::Bullet, Some(RIFLE));
}

/// `killer` kills `victim`, and a tick runs.
pub fn kill_now(sim: &mut Sim, victim: Entity, killer: Entity) {
    kill(sim, victim, killer);
    sim.ticks(1);
}

pub fn health(sim: &Sim, p: Entity) -> f32 {
    sim.app.world().get::<Health>(p).unwrap().current
}

pub fn stats(sim: &Sim, p: Entity) -> CaStats {
    sim.app.world().get::<CaStats>(p).copied().unwrap_or_default()
}

pub fn dead(sim: &Sim, p: Entity) -> bool {
    sim.app.world().get::<Dead>(p).is_some()
}

pub fn team(sim: &Sim, p: Entity) -> u8 {
    sim.app.world().get::<Team>(p).unwrap().0
}

pub fn carry(sim: &Sim, p: Entity) -> CaCarry {
    sim.app.world().get::<CaCarry>(p).copied().unwrap_or_default()
}

pub fn action(sim: &Sim, p: Entity) -> Option<CaAction> {
    sim.app.world().get::<CaAction>(p).copied()
}

pub fn at(sim: &Sim, p: Entity) -> Vec3 {
    sim.position(p)
}

/// Put `p` at `to` (its transform and its body), at rest.
pub fn put(sim: &mut Sim, p: Entity, to: Vec3) {
    let w = sim.app.world_mut();
    w.get_mut::<Transform>(p).unwrap().translation = to;
    if let Some(mut pos) = w.get_mut::<Position>(p) {
        pos.0 = to;
    }
}

/// Keep `p` at `to` for `secs` (re-placed every tick, so the movement
/// doesn't carry it off).
pub fn hold_at(sim: &mut Sim, p: Entity, to: Vec3, secs: f64) {
    let end = now(sim) + secs;
    while now(sim) < end - 1e-9 {
        put(sim, p, to);
        sim.ticks(1);
    }
}

/// The weapon ids `p` carries.
pub fn weapons(sim: &Sim, p: Entity) -> Vec<&'static str> {
    let w = sim.app.world();
    w.get::<Inventory>(p)
        .map(|i| {
            i.weapons
                .iter()
                .filter_map(|e| w.get::<Weapon>(*e))
                .map(|w| w.id)
                .collect()
        })
        .unwrap_or_default()
}

/// The weapon in `p`'s hand.
pub fn in_hand(sim: &Sim, p: Entity) -> Option<&'static str> {
    let w = sim.app.world();
    w.get::<Inventory>(p)
        .and_then(|i| i.active)
        .and_then(|e| w.get::<Weapon>(e))
        .map(|w| w.id)
}

/// A box about `centre` of half-extents `half` (world units).
pub fn obj_box(centre: Vec3, half_units: Vec3) -> ObjBox {
    ObjBox {
        centre,
        half: half_units * UNIT,
    }
}

/// A bomb site of `family`, number `number`, defended by `defusers`, the
/// stock sites' box (130 x 64 x 125 u half-extents), `DefuseTime` 7,
/// `DetonateTime` 40.
pub fn bomb_zone(family: &str, number: i32, defusers: u8, centre: Vec3) -> BombZone {
    BombZone {
        family: family.into(),
        number,
        defusers,
        can_defuse: true,
        defuse_time: 7.0,
        detonate_time: 40.0,
        area: obj_box(centre, Vec3::new(130.0, 64.0, 125.0)),
    }
}

/// A team's flag at `home` (its default box) and its zone (128 x 100 x 128
/// u about the flag's base).
pub fn flag_and_zone(team: u8, home: Vec3) -> (FlagObject, FlagZone) {
    (
        FlagObject {
            name: format!("Flag{team}"),
            family: "Capturetheflag".into(),
            team,
            area: obj_box(home, Vec3::new(10.0, 25.0, 10.0)),
        },
        FlagZone {
            team,
            linked_flag: format!("Flag{team}"),
            area: obj_box(home, Vec3::new(64.0, 50.0, 64.0)),
        },
    )
}

/// A use trigger named `name` about `centre` (100 u half-extents).
pub fn trigger(name: &str, centre: Vec3) -> UseTrigger {
    UseTrigger {
        name: name.into(),
        area: obj_box(centre, Vec3::splat(100.0)),
    }
}

/// A drop point of `family`.
pub fn drop_point(family: &str, at: Vec3) -> DropPoint {
    DropPoint {
        family: family.into(),
        at,
    }
}

/// A start point of `family` for `team`.
pub fn start(family: &str, team: Option<u8>, at: Vec3) -> StartPoint {
    StartPoint {
        family: family.into(),
        team,
        spawn: 1,
        locked: false,
        at: Transform::from_translation(at),
    }
}

/// Select the objective class (key 6) for a tick.
pub fn select_bomb(sim: &mut Sim, p: Entity) {
    sim.intent(p).select = Some(5);
    sim.ticks(1);
    sim.intent(p).select = None;
    sim.ticks(1);
}
