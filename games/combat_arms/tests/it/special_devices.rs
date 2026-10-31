//! specs/combat_arms/special_weapons.md's test cases 20-34 and 36-39 (mines
//! and devices, crossbows, the melee weapons, and the round end), and the
//! network's, with `special_weapons`' helpers (an open world, players,
//! the objects). Cases 1-19 and 35 are there.

use std::time::Duration;

use avian3d::prelude::*;
use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Hitgroup, Health, Intent};
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_sim::weapon::{
    Inventory, StartingWeapons, Weapon,
    grenade::Launch,
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim, map::loose::ShownItem};
use combat_arms::{
    game::{
        TICK_INTERVAL,
        grenades::{self, CaDamageSphere, CaGrenade, CaGrenadeEffects, M67},
        movement,
        rules::state::{CaMatch, Phase},
        special::{self, CaBoltData, CaCameraReverse, CaDeviceData, CaObject, Kind, devices},
        weapons::{CaDamage, Location},
    },
    harness::CaSim,
};

use super::special_weapons::*;

const HP_: f32 = 0.01;

/// Case 20: a landed mine 150 u from an M67 blast (60 x m(150) = 53.3 >=
/// 49) is set off and blows 267 ms later, credited to the mine's owner.
#[test]
fn t20_mine_chain() {
    let mut sim = sim();
    let owner = far_player(&mut sim, 1);
    let thrower = player(&mut sim, 6000.0, 6000.0, 0.0, 2);
    let mine = landed(&mut sim, MINE, Some(owner), 0.0, 0.0);
    let centre = obj(&sim, mine).at();
    let w = weapon_of(&mut sim, M67, Some(thrower));
    let g = grenades::spawn_grenade(sim.app.world_mut(), w, Some(thrower), centre + Vec3::X * 150.0 * U, Vec3::ZERO)
        .unwrap();
    sim.app.world_mut().get_mut::<CaGrenade>(g).unwrap().params.fuse = 0.0;
    let (f0, weapon, ..) = until_boom(&mut sim, 1.0);
    assert_eq!(weapon, M67);
    let (f1, weapon, who, _) = until_boom(&mut sim, 2.0);
    assert_eq!((weapon, who), (MINE, Some(owner)), "credited to the mine's owner");
    let gap = (f1 - f0) as f64 * 0.05;
    assert!((0.267..=0.267 + 0.15).contains(&gap), "267 ms later: {gap}");
}

/// A raised platform top at `top` u: x from `x0` to `x1` u, z from `z0` to
/// `z1` u.
fn platform(sim: &mut Sim, x0: f32, x1: f32, z0: f32, z1: f32, top: f32) -> Entity {
    solid(sim, Vec3::new(x0 * U, -1.0, z0 * U), Vec3::new(x1 * U, top * U, z1 * U), None)
}

/// A player with his feet `feet` u above the floor at `x`, `z` (u).
fn player_on(sim: &mut Sim, x: f32, z: f32, feet: f32, team: u8) -> Entity {
    let p = sim.spawn_character(Vec3::new(x * U, feet * U + 1.0, z * U), movement::ID);
    sim.app.world_mut().entity_mut(p).insert(chimera_core::core::Team(team));
    sim.seconds(0.5);
    sim.app.world_mut().entity_mut(p).insert((
        Health { current: 1.0, max: 1.0 },
        combat_arms::game::weapons::gear_armour(0.0, &Vec::new()),
    ));
    p
}

/// A device dropped from `y` u at `x`, `z` (u) that has landed.
fn landed_from(sim: &mut Sim, id: &str, owner: Option<Entity>, x: f32, y: f32, z: f32) -> Entity {
    let e = device(sim, id, owner, Vec3::new(x, y, z), Vec3::ZERO);
    for _ in 0..400 {
        sim.ticks(1);
        if obj(sim, e).s.landed {
            return e;
        }
    }
    panic!("never landed");
}

/// Case 21: a landed hornet looks for targets only after `ready` (666 ms);
/// an enemy within 100 u whose centre is not below it sets it off; `active`
/// 534 ms; 150 / 150 twice. 110 u away or below it: no trigger.
#[test]
fn t21_hornet() {
    // 90 u away horizontally, higher: set off after the ready animation.
    let mut sim = sim();
    let owner = far_player(&mut sim, 1);
    let victim = player(&mut sim, 90.0, 0.0, 0.0, 2);
    set_health(&mut sim, victim, 1000.0);
    let hornet = landed(&mut sim, HORNET, Some(owner), 0.0, 0.0);
    let landed_at = obj(&sim, hornet).s.landed_at;
    while now(&sim) < landed_at + 0.62 {
        sim.ticks(1);
    }
    assert!(obj(&sim, hornet).s.triggered_at.is_none(), "not in the first 666 ms");
    let mut trig = None;
    let (f, _, _, at) = loop {
        sim.ticks(1);
        if let Some(b) = booms(&sim).first() {
            break *b;
        }
        if trig.is_none() {
            trig = obj(&sim, hornet).s.triggered_at;
        }
    };
    let trig = trig.expect("set off");
    assert!(trig >= landed_at + 0.666 - 1e-6 && trig <= landed_at + 0.666 + 0.1, "set off at {trig}");
    let blast = f as f64 * 0.05;
    assert!(blast >= trig + 0.534 - 1e-6 && blast <= trig + 0.534 + 0.05 + 1e-6, "blast at {blast}");
    sim.ticks(2);
    let (c, _) = body(&mut sim, victim);
    let d = dist_u(c, at * U);
    assert_hp(1000.0 - health(&sim, victim), 2.0 * 150.0 * m(150.0, d), 0.01, "150 / 150 twice");

    // 110 u away: no trigger.
    let mut sim = self::sim();
    let owner = far_player(&mut sim, 1);
    player(&mut sim, 110.0, 0.0, 0.0, 2);
    let hornet = landed(&mut sim, HORNET, Some(owner), 0.0, 0.0);
    wait(&mut sim, 3.0);
    assert!(obj(&sim, hornet).s.triggered_at.is_none(), "110 u: no trigger");

    // An enemy lower than the hornet: none.
    let mut sim = self::sim();
    let owner = far_player(&mut sim, 1);
    platform(&mut sim, -50.0, 50.0, -50.0, 50.0, 60.0);
    player(&mut sim, 90.0, 0.0, 0.0, 2);
    let e = landed_from(&mut sim, HORNET, Some(owner), 0.0, 72.0, 0.0);
    wait(&mut sim, 3.0);
    assert!(obj(&sim, e).pos.y > 60.0, "on the platform");
    assert!(obj(&sim, e).s.triggered_at.is_none(), "an enemy below it never sets it off");
}

/// Case 22: the claymore's cone: 120 / 500 within 80 degrees of its facing
/// (+Z), else 60 / 120; claymore type, applied twice. Victims all around it
/// in the open (a victim above has a smaller dot: the numbers come from the
/// vectors measured).
#[test]
fn t22_claymore_cone() {
    let mut sim = sim();
    let owner = far_player(&mut sim, 1);
    let spots = [(0.0, 100.0), (0.0, 300.0), (300.0, 0.0), (100.0, 0.0), (0.0, -50.0)];
    let victims: Vec<Entity> = spots.iter().map(|(x, z)| player(&mut sim, *x, *z, 0.0, 2)).collect();
    for v in &victims {
        set_health(&mut sim, *v, 1000.0);
    }
    let cm = device(&mut sim, CLAYMORE, Some(owner), Vec3::new(0.0, 10.0, 0.0), Vec3::ZERO);
    sim.ticks(8);
    assert!(obj(&sim, cm).s.landed, "put on the floor");
    sim.app.world_mut().get_mut::<CaObject>(cm).unwrap().s.facing = Vec3::Z;
    let at = obj(&sim, cm).at();
    sim.app.world_mut().get_mut::<CaObject>(cm).unwrap().s.marked = true;
    let (_, weapon, ..) = until_boom(&mut sim, 1.0);
    assert_eq!(weapon, CLAYMORE);
    sim.ticks(2);
    let cos80 = 80f32.to_radians().cos();
    let mut got = Vec::new();
    for (v, (x, z)) in victims.iter().zip(spots) {
        let (c, _) = body(&mut sim, *v);
        let d = dist_u(c, at);
        let dot = Vec3::Z.dot((c - at).normalize());
        let want = if d <= 500.0 && dot > cos80 {
            2.0 * 120.0 * m(500.0, d)
        } else if d <= 120.0 {
            2.0 * 60.0 * m(120.0, d)
        } else {
            0.0
        };
        let loss = 1000.0 - health(&sim, *v);
        assert_hp(loss, want, 0.01, &format!("victim at ({x}, {z}): dot {dot}, d {d}"));
        got.push(loss);
    }
    // The shape of it: the near front victim takes the cone's full part, the
    // far front one its falloff, the side 300 u away nothing, the near side
    // and the one behind the plain part.
    assert!(got[0] > 200.0, "front: {}", got[0]);
    assert!(got[1] > 100.0 && got[1] < 140.0, "far front: {}", got[1]);
    assert_eq!(got[2], 0.0, "side, beyond 120 u");
    assert!(got[3] > 0.0 && got[3] < 60.0, "side within 120 u: plain: {}", got[3]);
    assert!(got[4] > 60.0, "behind within 120 u: {}", got[4]);
}

/// Case 23: the detonator blows every claymore its owner placed, on the
/// frame after the fire message; his death does the same; his leaving
/// removes them without a blast.
#[test]
fn t23_claymore_detonator() {
    let place = |sim: &mut Sim, owner: Entity| -> [Entity; 2] {
        [
            device(sim, CLAYMORE, Some(owner), Vec3::new(0.0, 10.0, 0.0), Vec3::ZERO),
            device(sim, CLAYMORE, Some(owner), Vec3::new(80.0, 10.0, 0.0), Vec3::ZERO),
        ]
    };
    // The controller fired.
    let mut sim = sim();
    let owner = player(&mut sim, 3000.0, 3000.0, 0.0, 1);
    armed(&mut sim, owner, CLAYMORE_CTL);
    let cms = place(&mut sim, owner);
    sim.ticks(3);
    assert!(cms.iter().all(|c| alive(&sim, *c)));
    let press = frame(&sim);
    sim.intent(owner).fire = true;
    sim.ticks(1);
    sim.intent(owner).fire = false;
    sim.ticks(8);
    let b = booms(&sim);
    assert_eq!(b.len(), 2, "both blow: {b:?}");
    assert!(b.iter().all(|b| b.1 == CLAYMORE && b.0 <= press + 3), "on the frame after the message");
    // His death.
    let mut sim = self::sim();
    let owner = player(&mut sim, 3000.0, 3000.0, 0.0, 1);
    let killer = player(&mut sim, 6000.0, 6000.0, 0.0, 2);
    place(&mut sim, owner);
    sim.ticks(3);
    sim.app.world_mut().resource_mut::<Messages<Damage>>().write(Damage {
        target: owner,
        attacker: Some(killer),
        amount: 5.0,
        point: Vec3::ZERO,
        dir: Vec3::ZERO,
        hitgroup: Hitgroup::Generic,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(10);
    assert_eq!(health(&sim, owner), 0.0, "dead");
    assert_eq!(booms(&sim).len(), 2, "both blow when he dies");
    // He leaves.
    let mut sim = self::sim();
    let owner = player(&mut sim, 3000.0, 3000.0, 0.0, 1);
    let cms = place(&mut sim, owner);
    sim.ticks(3);
    sim.app.world_mut().despawn(owner);
    sim.ticks(10);
    assert!(booms(&sim).is_empty(), "no blast");
    assert!(cms.iter().all(|c| !alive(&sim, *c)), "removed");
}

/// Case 24: a remote grenade blows in every direction: 100 / 450 claymore
/// type, applied twice (200 within 112.5 u).
#[test]
fn t24_remote_grenade() {
    let mut sim = sim();
    let owner = player(&mut sim, 3000.0, 3000.0, 0.0, 1);
    armed(&mut sim, owner, REMOTE_CTL);
    let victims: Vec<Entity> = [(100.0, 0.0), (0.0, 200.0), (-390.0, 0.0)]
        .iter()
        .map(|(x, z)| player(&mut sim, *x, *z, 0.0, 2))
        .collect();
    for v in &victims {
        set_health(&mut sim, *v, 1000.0);
    }
    let r = landed(&mut sim, REMOTE, Some(owner), 0.0, 0.0);
    let at = obj(&sim, r).at();
    sim.ticks(3);
    sim.intent(owner).fire = true;
    sim.ticks(1);
    sim.intent(owner).fire = false;
    let (_, weapon, who, _) = until_boom(&mut sim, 1.0);
    assert_eq!((weapon, who), (REMOTE, Some(owner)));
    sim.ticks(2);
    for v in &victims {
        let (c, _) = body(&mut sim, *v);
        let d = dist_u(c, at);
        assert_hp(1000.0 - health(&sim, *v), 2.0 * 100.0 * m(450.0, d), 0.01, &format!("at {d}"));
    }
}

/// Case 25: caltrops bleed an enemy standing in the cylinder (50 u by 80
/// u): 3 HP/s refreshed every frame for 5 s, +0.4 movement penalty for 5 s;
/// 90 u above or 60 u away: nothing.
#[test]
fn t25_caltrops() {
    let mut sim = sim();
    let owner = far_player(&mut sim, 1);
    let victim = player(&mut sim, 30.0, 0.0, 0.0, 2);
    set_health(&mut sim, victim, 1000.0);
    let ct = landed(&mut sim, CALTROPS, Some(owner), 0.0, 0.0);
    let t0 = now(&sim);
    while now(&sim) - t0 < 3.0 {
        sim.ticks(1);
    }
    assert_hp(1000.0 - health(&sim, victim), 9.0, 1.0, "3 HP/s for 3 s");
    let p = sim.app.world().get::<movement::StunPenalty>(victim).expect("slowed");
    assert_eq!(p.penalty, 0.4);
    assert!(p.until > now(&sim) + 4.5, "for 5 s from the last damage");
    assert!(alive(&sim, ct));
    // He leaves: 5 s more of the entry (15 HP).
    put(&mut sim, victim, 400.0, 400.0);
    wait(&mut sim, 6.0);
    assert_hp(1000.0 - health(&sim, victim), 24.0, 1.0, "15 HP more after leaving");

    for (x, top, what) in [(40.0, 60.0, "90 u above"), (60.0, 0.0, "60 u away")] {
        let mut sim = self::sim();
        let owner = far_player(&mut sim, 1);
        if top > 0.0 {
            platform(&mut sim, 20.0, 60.0, -20.0, 20.0, top);
        }
        let victim = player_on(&mut sim, x, 0.0, top, 2);
        landed(&mut sim, CALTROPS, Some(owner), 0.0, 0.0);
        wait(&mut sim, 3.0);
        assert_eq!(health(&sim, victim), 100.0, "{what}");
    }
}

/// Case 26: a victim is counted when first damaged and again after more
/// than 1000 ms; three enemies in the caltrops count 3, 6, 9, 12 at about
/// 0, 1.05, 2.1, 3.15 s: at the 12th the caltrops disappear.
#[test]
fn t26_caltrops_count() {
    let mut sim = sim();
    let owner = far_player(&mut sim, 1);
    let vs: Vec<Entity> = (0..3).map(|i| player(&mut sim, 1000.0 + 100.0 * i as f32, 0.0, 0.0, 2)).collect();
    for v in &vs {
        set_health(&mut sim, *v, 1000.0);
    }
    let ct = landed(&mut sim, CALTROPS, Some(owner), 0.0, 0.0);
    for (v, (x, z)) in vs.iter().zip([(30.0, 0.0), (-15.0, 26.0), (-15.0, -26.0)]) {
        put(&mut sim, *v, x, z);
    }
    let mut counts: Vec<(f64, u32)> = Vec::new();
    let mut gone = None;
    for _ in 0..600 {
        sim.ticks(1);
        if !alive(&sim, ct) {
            gone = Some(now(&sim));
            break;
        }
        let c = obj(&sim, ct).s.counts;
        if counts.last().is_none_or(|l| l.1 != c) {
            counts.push((now(&sim), c));
        }
    }
    let gone = gone.expect("the caltrops disappeared");
    let first = counts.iter().find(|c| c.1 > 0).unwrap().0;
    assert!(counts.iter().any(|c| c.1 == 9), "nine counts before: {counts:?}");
    assert!(!counts.iter().any(|c| c.1 >= 12), "removed at the 12th: {counts:?}");
    assert!((gone - first - 3.15).abs() < 0.2, "gone at {} after the first count", gone - first);
    // Each victim is counted again more than 1 s after his last count: no
    // more than three counts in any 0.9 s (the three came in on frames of
    // their own).
    let times: Vec<f64> = counts.iter().map(|c| c.0 - first).collect();
    assert!(times.len() >= 9, "{times:?}");
    assert!(times.windows(4).all(|w| w[3] - w[0] > 0.9), "recounted too soon: {times:?}");
}

/// Case 27: a nerve mine is active from its first touch of any surface (a
/// wall); every frame an enemy within 250 u, through walls, gets the type
/// 44 effect (3, 3 s); a second one thrown removes the first.
#[test]
fn t27_nerve_mine() {
    let mut sim = sim();
    let owner = far_player(&mut sim, 1);
    solid(&mut sim, Vec3::new(-10.0, -1.0, 100.0 * U), Vec3::new(10.0, 10.0, 110.0 * U), None);
    let victim = player(&mut sim, 0.0, 200.0, 0.0, 2);
    let mate = player(&mut sim, 60.0, 200.0, 0.0, 1);
    let first = device(&mut sim, NERVE, Some(owner), Vec3::new(0.0, 60.0, 0.0), Vec3::new(0.0, 0.0, 800.0));
    for _ in 0..40 {
        sim.ticks(1);
        if obj(&sim, first).s.active {
            break;
        }
    }
    assert!(obj(&sim, first).s.active, "active from the wall touch");
    sim.ticks(8);
    let fx = sim.app.world().get::<CaCameraReverse>(victim).copied().expect("the enemy through the wall");
    assert_eq!(fx.amount, 3.0);
    assert!(fx.until > now(&sim) + 2.5, "3 s");
    assert!(sim.app.world().get::<CaCameraReverse>(mate).is_none(), "not a teammate");
    // A second one thrown by the weapon replaces the first.
    let w = weapon_of(&mut sim, NERVE, Some(owner));
    devices::launch(
        sim.app.world_mut(),
        Launch {
            weapon: w,
            thrower: owner,
            eye: Vec3::new(0.0, 1.0, 0.0),
            yaw: std::f32::consts::PI,
            pitch: 0.0,
            velocity: Vec3::ZERO,
            command: 3,
        },
    );
    sim.ticks(3);
    assert!(!alive(&sim, first), "the first is removed on its next update");
    let second = sim
        .app
        .world_mut()
        .query::<(Entity, &CaObject)>()
        .iter(sim.app.world())
        .filter(|(_, o)| o.weapon == NERVE)
        .count();
    assert_eq!(second, 1, "the second is there");
}

/// An enemy on a platform `rise` u high 500 u (horizontally) away from a
/// landed turret, and the turret (the exact variant: no aim error, no roll).
fn turret_with_target(sim: &mut Sim, x: f32, z: f32, feet: f32) -> (Entity, Entity, Entity) {
    let owner = far_player(sim, 1);
    if feet > 0.0 {
        platform(sim, x - 40.0, x + 40.0, z - 40.0, z + 40.0, feet);
    }
    let victim = player_on(sim, x, z, feet, 2);
    set_health(sim, victim, 10000.0);
    let t = landed(sim, "test:turret_exact", Some(owner), 0.0, 0.0);
    (owner, victim, t)
}

/// Case 28: a turret targets the nearest enemy in view within 800 u and
/// the shooting angles (up to 60 degrees above, 45 below): shots every
/// 0.13 s (a frame of 50 ms: every 0.15 s) of 12 damage, 100 rays, then it
/// is removed. 70 degrees above or 900 u away: not targeted.
#[test]
fn t28_turret_target() {
    // 20 degrees above the sensor at 500 u: its sensor is 22 u up, the
    // enemy's centre at 22 + 182 = 204 u: feet at about 164.
    let (_, victim, t) = turret_with_target(&mut sim(), 500.0, 0.0, 164.0);
    let _ = (victim, t);
    let mut sim = sim();
    let (owner, victim, turret) = turret_with_target(&mut sim, 500.0, 0.0, 164.0);
    let landed_at = obj(&sim, turret).s.landed_at;
    wait(&mut sim, 6.0);
    wait(&mut sim, 14.0);
    let shots: Vec<(f64, f32)> = hits(&sim)
        .into_iter()
        .filter(|h| h.1 == victim && h.2 == Some(owner))
        .map(|h| (h.0, h.3))
        .collect();
    assert_eq!(shots.len(), 100, "100 rays");
    assert!(shots[0].0 >= landed_at + 2.833, "set up first: {}", shots[0].0 - landed_at);
    for (t, d) in &shots {
        assert!(in_a_zone(*d, 12.0), "12 x a zone at {t}: {d}");
    }
    for w in shots.windows(2) {
        let gap = w[1].0 - w[0].0;
        assert!((0.13..=0.2).contains(&gap), "shots {gap} apart");
    }
    assert!(!alive(&sim, turret), "gone with its last ray");
    // The band at 850 u: 8.4.
    let w = weapon_of(&mut sim, "test:turret_exact", None);
    let d = sim.app.world().get::<CaDeviceData>(w).unwrap().clone();
    let gun = d.params.turret.as_ref().unwrap().gun.clone();
    let dmg = CaDamage::new(&gun, d.unit, d.hp);
    assert_hp(base_damage_at(&dmg, 850.0) / HP_, 8.4, 0.001, "850 u");
    assert_hp(base_damage_at(&dmg, 800.0) / HP_, 12.0, 0.001, "800 u");

    // 70 degrees above, 200 u away: out of the shooting angles.
    let mut sim = self::sim();
    let (_, victim, turret) = turret_with_target(&mut sim, 200.0, 0.0, 530.0);
    wait(&mut sim, 6.0);
    assert!(obj(&sim, turret).s.target.is_none(), "70 degrees: not targeted");
    assert_eq!(health(&sim, victim), 10000.0);
    // 900 u away: out of its search.
    let mut sim = self::sim();
    let (_, victim, turret) = turret_with_target(&mut sim, 900.0, 0.0, 0.0);
    wait(&mut sim, 6.0);
    assert!(obj(&sim, turret).s.target.is_none(), "900 u: not targeted");
    assert_eq!(health(&sim, victim), 10000.0);
}

fn base_damage_at(d: &CaDamage, u: f32) -> f32 {
    combat_arms::game::weapons::base_damage(d, 0.5, false, u * U)
}

/// Case 29: a kit lands at 0.3 s next to an injured teammate and heals 5
/// HP at 1.3 s; it is gone at 2.0 s (no second tick); an enemy is never
/// healed, a teammate at full health isn't and the pool stays.
#[test]
fn t29_first_aid_kit() {
    let mut sim = sim();
    let owner = player(&mut sim, 500.0, 500.0, 0.0, 1);
    let mate = player(&mut sim, 30.0, 0.0, 0.0, 1);
    let enemy = player(&mut sim, -30.0, 0.0, 0.0, 2);
    let healthy = player(&mut sim, 0.0, 40.0, 0.0, 1);
    set_health(&mut sim, mate, 50.0);
    set_health(&mut sim, enemy, 50.0);
    let kit = device(&mut sim, KIT, Some(owner), Vec3::new(0.0, 60.0, 0.0), Vec3::ZERO);
    let mut landed_step = None;
    let mut healed_at = None;
    let mut gone_at = None;
    let t0 = now(&sim);
    for _ in 0..400 {
        sim.ticks(1);
        let t = now(&sim) - t0;
        if !alive(&sim, kit) {
            gone_at = Some(t);
            break;
        }
        let o = obj(&sim, kit);
        if o.s.landed && landed_step.is_none() {
            landed_step = Some(o.steps);
        }
        if healed_at.is_none() && health(&sim, mate) > 50.0 {
            healed_at = Some(t);
        }
    }
    assert_eq!(landed_step, Some(6), "lands in step 6 (0.3 s)");
    let healed = healed_at.expect("healed");
    assert!((healed - 1.3).abs() < 0.1, "+5 HP at 1.3 s: {healed}");
    let gone = gone_at.expect("removed");
    assert!((gone - 2.0).abs() < 0.1, "removed at 2.0 s: {gone}");
    sim.seconds(1.0);
    assert_hp(health(&sim, mate), 55.0, 0.01, "one tick of 5");
    assert_eq!(health(&sim, enemy), 50.0, "an enemy beside it");
    assert_eq!(health(&sim, healthy), 100.0, "full health");
}

/// Case 30: a medkit heals 11 HP/s for 3 s, 33 in all, capped at the
/// maximum.
#[test]
fn t30_medkit() {
    for (start, want) in [(40.0, 73.0), (90.0, 100.0)] {
        let mut sim = sim();
        let p = player(&mut sim, 0.0, 0.0, 0.0, 1);
        set_health(&mut sim, p, start);
        armed(&mut sim, p, MEDKIT);
        sim.intent(p).fire = true;
        sim.ticks(1);
        sim.intent(p).fire = false;
        let mut began = None;
        for _ in 0..400 {
            sim.ticks(1);
            if health(&sim, p) > start + 0.01 {
                began = Some(now(&sim));
                break;
            }
        }
        let began = began.expect("heals");
        let mut at3 = 0.0;
        while now(&sim) < began + 3.0 {
            sim.ticks(1);
            at3 = health(&sim, p);
        }
        assert_hp(at3, want, 0.7, &format!("73 after 3 s from {start}"));
        wait(&mut sim, 2.0);
        assert_hp(health(&sim, p), want, 0.01, &format!("no more from {start}"));
    }
}

/// Case 31: the LGB designator: the marker touches the floor point P 1000
/// u away in 0.2 s; at 8 s one shell starts 3000 u above P and falls on it
/// (gravity with the quarter term); blast 110 / 1200, twice.
#[test]
fn t31_lgb() {
    let mut sim = sim();
    let owner = far_player(&mut sim, 1);
    let victims: Vec<Entity> = [(0.0, 1200.0), (600.0, 1000.0), (-1000.0, 1000.0)]
        .iter()
        .map(|(x, z)| player(&mut sim, *x, *z, 0.0, 2))
        .collect();
    for v in &victims {
        set_health(&mut sim, *v, 1000.0);
    }
    let from = Vec3::new(0.0, 60.0, 0.0);
    let p = Vec3::new(0.0, 0.0, 1000.0);
    let marker = device(&mut sim, LGB, Some(owner), from, (p - from).normalize() * 5000.0);
    let t0 = now(&sim);
    let mut touched = None;
    let mut spawned = None;
    let mut shell_frame = 0;
    let mut start = Vec3::ZERO;
    for _ in 0..1000 {
        sim.ticks(1);
        let t = now(&sim) - t0;
        if touched.is_none() && alive(&sim, marker) && obj(&sim, marker).s.strike_point.is_some() {
            touched = Some(t);
        }
        let cur_frame = frame(&sim);
        if spawned.is_none() {
            let w = sim.app.world_mut();
            let mut q = w.query::<&CaObject>();
            if let Some(o) = q.iter(w).find(|o| matches!(o.kind, Kind::Impact(_))) {
                spawned = Some(t);
                shell_frame = cur_frame;
                start = o.pos;
            }
        }
        if !booms(&sim).is_empty() {
            break;
        }
    }
    assert!(touched.expect("touched") < 0.35, "the marker touches the floor at about 0.2 s");
    assert!((spawned.expect("shell") - 8.0).abs() < 0.15, "at 8 s: {spawned:?}");
    assert_near(start, Vec3::new(0.0, 3000.0 + 2.0, 1000.0), 1.5, "3000 u above P");
    let (f, weapon, _, at) = booms(&sim)[0];
    assert_eq!(weapon, LGB);
    let fall = (f - shell_frame) as f64 * 0.05;
    assert!((1.1..=1.5).contains(&fall), "about 1.3 s later: {fall}");
    assert_near(Vec3::new(at.x, 0.0, at.z), Vec3::new(0.0, 0.0, 1000.0), 0.5, "on P");
    sim.ticks(2);
    for v in &victims {
        let (c, _) = body(&mut sim, *v);
        let d = dist_u(c, at * U);
        assert_hp(1000.0 - health(&sim, *v), 2.0 * 110.0 * m(1200.0, d), 0.01, &format!("at {d}"));
    }
}

/// Case 32: the napalm designator drops six shells 200 ms apart at P, P +
/// 200, ... P + 1000 along the course; each leaves a 350 u burning sphere
/// for 5 s (80 HP/s, 2 s entries).
#[test]
fn t32_napalm() {
    let mut sim = sim();
    let owner = far_player(&mut sim, 1);
    let victim = player(&mut sim, 100.0, 1000.0, 0.0, 2);
    set_health(&mut sim, victim, 10000.0);
    let from = Vec3::new(0.0, 60.0, 0.0);
    let p = Vec3::new(0.0, 0.0, 1000.0);
    device(&mut sim, NAPALM, Some(owner), from, (p - from).normalize() * 5000.0);
    let mut seen: Vec<(u64, Vec3)> = Vec::new();
    let mut known = std::collections::HashSet::new();
    for _ in 0..1000 {
        sim.ticks(1);
        let f = frame(&sim);
        let w = sim.app.world_mut();
        let mut q = w.query::<&CaObject>();
        for o in q.iter(w) {
            if matches!(o.kind, Kind::Impact(_)) && known.insert(o.seq) {
                seen.push((f, o.pos));
            }
        }
        if seen.len() == 6 && !w.query::<&CaDamageSphere>().iter(w).next().is_none() {
            // keep going until all six spheres exist
        }
        let spheres = w.query::<&CaDamageSphere>().iter(w).count();
        if spheres == 6 {
            break;
        }
    }
    assert_eq!(seen.len(), 6, "six shells");
    for (i, (f, pos)) in seen.iter().enumerate() {
        assert_eq!(f - seen[0].0, 4 * i as u64, "200 ms apart");
        assert!((pos.z - (1000.0 + 200.0 * i as f32)).abs() < 1.5, "shell {i} at z {}", pos.z);
        assert!((pos.y - 3002.0).abs() < 1.5, "from 3000 u up: {}", pos.y);
    }
    let w = sim.app.world_mut();
    let mut zs: Vec<f32> = w.query::<&CaDamageSphere>().iter(w).map(|s| s.centre.z / U).collect();
    zs.sort_by(|a, b| a.total_cmp(b));
    for (i, z) in zs.iter().enumerate() {
        assert!((z - (1000.0 + 200.0 * i as f32)).abs() < 2.0, "sphere {i} at {z}");
    }
    let s = w.query::<&CaDamageSphere>().iter(w).next().unwrap().clone();
    assert_eq!((s.params.radius, s.params.rate, s.params.lifetime, s.params.duration), (350.0, 80.0, 5.0, 2.0));
    sim.ticks(3);
    let entries = sim
        .app
        .world()
        .resource::<CaGrenadeEffects>()
        .over_time
        .iter()
        .filter(|o| o.victim == victim)
        .count();
    assert!(entries >= 1, "burning");
}

/// A crossbow bolt of `id` from `from` (u) at the point `to` (m).
fn bolt(sim: &mut Sim, id: &str, owner: Option<Entity>, from: Vec3, to_m: Vec3) -> Entity {
    let w = weapon_of(sim, id, owner);
    let d = sim.app.world().get::<CaBoltData>(w).unwrap().clone();
    let name = sim.app.world().get::<Weapon>(w).unwrap().id;
    let v = (to_m / U - from).normalize() * d.params.speed;
    let mut o = CaObject::new(name, owner, d.unit, d.hp, from, v, Kind::Bolt(d.params));
    o.s.origin = from;
    special::spawn_object(sim.app.world_mut(), o)
}

/// Case 33: a bolt fired from (100, 60, 0) at a torso 1000 u away sticks
/// after 4 steps; 105 x roll, x the band of the shooter's distance from the
/// map's origin; head x6. From (4500, 60, 0) the band is x0.35 (Quirk 4).
#[test]
fn t33_crossbow_bolt() {
    for (x, band) in [(100.0, 1.0), (4500.0, 0.35)] {
        // Torso.
        let mut sim = sim();
        let owner = far_player(&mut sim, 1);
        let victim = player(&mut sim, x, 1000.0, 0.0, 2);
        set_health(&mut sim, victim, 10000.0);
        let torso = spheres(&mut sim, victim)
            .into_iter()
            .filter(|s| s.2 == Location::Torso)
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        let b = bolt(&mut sim, BOW, Some(owner), Vec3::new(x, 60.0, 0.0), torso.0);
        let s = to_step(&mut sim, b, 4);
        assert!(s.s.landed, "stuck after 4 steps");
        sim.ticks(2);
        let lost = 10000.0 - health(&sim, victim);
        assert!(lost >= 94.5 * band - 0.01 && lost <= 115.5 * band + 0.01, "torso x{band}: {lost}");
        // Head x6.
        let mut sim = self::sim();
        let owner = far_player(&mut sim, 1);
        let victim = player(&mut sim, x, 1000.0, 0.0, 2);
        set_health(&mut sim, victim, 10000.0);
        let head = spheres(&mut sim, victim).into_iter().find(|s| s.2 == Location::Head && s.1 > 0.0).unwrap();
        bolt(&mut sim, BOW, Some(owner), Vec3::new(x, 60.0, 0.0), head.0);
        wait(&mut sim, 0.5);
        let lost = 10000.0 - health(&sim, victim);
        assert!(lost >= 567.0 * band - 0.01 && lost <= 693.0 * band + 0.01, "head x6 x{band}: {lost}");
    }
}

/// Case 34: an explosive bolt stuck in a wall blows `ExplosionDelayTime`
/// (1.743 s) after its controller is fired: 100 / 250 claymore type, twice;
/// its owner's death blows it at once.
#[test]
fn t34_explosive_bolt() {
    let setup = |sim: &mut Sim| -> (Entity, Entity, Entity) {
        solid(sim, Vec3::new(-10.0, -1.0, 300.0 * U), Vec3::new(10.0, 10.0, 310.0 * U), None);
        let owner = player(sim, 3000.0, 3000.0, 0.0, 1);
        armed(sim, owner, BOW_CTL);
        let victim = player(sim, 45.0, 250.0, 0.0, 2);
        set_health(sim, victim, 1000.0);
        let b = bolt(sim, BOW_EXPL, Some(owner), Vec3::new(0.0, 60.0, 0.0), Vec3::new(0.0, 60.0 * U, 400.0 * U));
        (owner, victim, b)
    };
    let mut sim = sim();
    let (owner, victim, b) = setup(&mut sim);
    sim.ticks(10);
    assert!(obj(&sim, b).s.landed, "stuck in the wall");
    let at = obj(&sim, b).at();
    wait(&mut sim, 3.0);
    assert!(alive(&sim, b), "waits for its controller");
    let t = now(&sim);
    sim.intent(owner).fire = true;
    sim.ticks(1);
    sim.intent(owner).fire = false;
    let (f, weapon, who, _) = until_boom(&mut sim, 3.0);
    assert_eq!((weapon, who), (BOW_EXPL, Some(owner)));
    let delay = f as f64 * 0.05 - t;
    assert!((delay - 1.743).abs() < 0.12, "about 1.743 s later: {delay}");
    sim.ticks(2);
    let (c, _) = body(&mut sim, victim);
    let d = dist_u(c, at);
    assert_hp(1000.0 - health(&sim, victim), 2.0 * 100.0 * m(250.0, d), 0.01, &format!("at {d}"));
    // The owner dies first: at once.
    let mut sim = self::sim();
    let (owner, _, b) = setup(&mut sim);
    let killer = player(&mut sim, 6000.0, 6000.0, 0.0, 2);
    sim.ticks(10);
    assert!(alive(&sim, b));
    sim.app.world_mut().resource_mut::<Messages<Damage>>().write(Damage {
        target: owner,
        attacker: Some(killer),
        amount: 5.0,
        point: Vec3::ZERO,
        dir: Vec3::ZERO,
        hitgroup: Hitgroup::Generic,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(10);
    assert!(!alive(&sim, b), "gone");
    assert_eq!(booms(&sim).len(), 1, "blown at once");
}

// ---------------------------------------------------------------------------
// Melee (35-37)

/// An attacker holding `id` at the origin looking down -Z and a target facing
/// `yaw` whose surface, at the attacker's eye height, is `surface` u ahead
/// of the eye (health to spare: hits are read from the damage dealt).
fn melee_target(id: &str, surface: f32, yaw: f32) -> (Sim, Entity, Entity) {
    let mut sim = sim();
    let p = player(&mut sim, 0.0, 0.0, 0.0, 1);
    armed(&mut sim, p, id);
    let probe = 2.0;
    let target = sim.spawn_character(sim.position(p) - Vec3::Z * probe, movement::ID);
    sim.intent(target).yaw = yaw;
    sim.app.world_mut().entity_mut(target).insert(chimera_core::core::Team(2));
    sim.app.world_mut().get_mut::<Health>(target).unwrap().current = 10.0;
    sim.seconds(0.5);
    let e = eye(&sim, p);
    let hit = sim
        .app
        .world_mut()
        .run_system_once(move |q: SpatialQuery| {
            let filter = SpatialQueryFilter::from_excluded_entities([p]);
            q.cast_ray(e, Dir3::NEG_Z, 10.0, true, &filter).map(|h| (h.entity, h.distance))
        })
        .unwrap()
        .expect("the ray meets the target");
    assert_eq!(hit.0, target);
    let depth = (e.z - sim.position(target).z) - hit.1;
    let at = sim.position(target).with_z(e.z - surface * U - depth);
    sim.app.world_mut().get_mut::<Transform>(target).unwrap().translation = at;
    sim.seconds(0.5);
    (sim, p, target)
}

use bevy::ecs::system::RunSystemOnce;

/// Damage `target` took, (time, points).
fn taken(sim: &Sim, target: Entity) -> Vec<(f64, f32)> {
    hits(sim).into_iter().filter(|h| h.1 == target).map(|h| (h.0, h.3)).collect()
}

/// Case 35: the Dom Pedro sword reaches 160 u: a victim whose surface is
/// 155 u away is hit (85 x the roll), 165 u away is not; the M9 misses at
/// 110 u. (Case 35's head x6 = 459-561 is the zone of the sphere the
/// level ray meets: read from the damage and its ratio.)
#[test]
fn t35_melee_reach() {
    for (id, surface, hits_it) in [
        ("combat_arms:dompedrosword", 155.0, true),
        ("combat_arms:dompedrosword", 165.0, false),
        (combat_arms::game::inventory::KNIFE, 110.0, false),
    ] {
        let (mut sim, p, target) = melee_target(id, surface, std::f32::consts::PI);
        sim.intent(p).fire = true;
        sim.ticks(1);
        sim.intent(p).fire = false;
        sim.seconds(1.5);
        let t = taken(&sim, target);
        assert_eq!(!t.is_empty(), hits_it, "{id} at {surface}: {t:?}");
        if hits_it {
            let ratio = t[0].1 / 85.0;
            let zone = [1.0f32, 6.0, 0.9, 0.75, 0.5].iter().any(|z| ratio >= z * 0.9 - 1e-3 && ratio <= z * 1.1 + 1e-3);
            assert!(zone, "85 x the roll x a zone: {}", t[0].1);
        }
    }
    // Head: the ray aimed at the head sphere: x6, 459-561.
    let (mut sim, p, target) = melee_target("combat_arms:dompedrosword", 155.0, std::f32::consts::PI);
    let e = eye(&sim, p);
    let head = spheres(&mut sim, target).into_iter().find(|s| s.2 == Location::Head && s.1 > 0.0).unwrap();
    let d = head.0 - e;
    sim.intent(p).pitch = d.y.atan2(d.xz().length());
    sim.ticks(2);
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    sim.seconds(1.5);
    let t = taken(&sim, target);
    assert!(!t.is_empty());
    // (The ray meets the first sphere on its line, not always the head's:
    // whichever it is, 85 x the roll x that zone.)
    let ratio = t[0].1 / 85.0;
    assert!([1.0f32, 6.0, 0.9, 0.75, 0.5].iter().any(|z| ratio >= z * 0.9 - 1e-3 && ratio <= z * 1.1 + 1e-3), "{}", t[0].1);
}

/// Whether `amount` is `base` times one of the hit table's zone factors
/// (the level ray meets whichever sphere is first on the line).
fn in_a_zone(amount: f32, base: f32) -> bool {
    [1.0f32, 0.9, 0.75, 0.5, 6.0].iter().any(|z| (amount - base * z).abs() < 0.01)
}

/// Case 36: a Machete's right button hits twice, 70 at 667 ms and 70 at
/// 1167 ms after the swing starts (roll 1.0, torso); from behind 91 each.
#[test]
fn t36_melee_double_secondary() {
    for (yaw, each) in [(std::f32::consts::PI, 70.0), (0.0, 91.0)] {
        let (mut sim, p, target) = melee_target("test:machete_exact", 100.0, yaw);
        let e = eye(&sim, p);
        let torso = spheres(&mut sim, target)
            .into_iter()
            .filter(|s| s.2 == Location::Torso)
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        let d = torso.0 - e;
        sim.intent(p).pitch = d.y.atan2(d.xz().length());
        sim.ticks(2);
        let t0 = now(&sim) + TICK_INTERVAL;
        sim.intent(p).secondary = true;
        sim.ticks(1);
        sim.intent(p).secondary = false;
        sim.seconds(2.0);
        let t = taken(&sim, target);
        assert_eq!(t.len(), 2, "two hits: {t:?}");
        for (k, (when, amount)) in t.iter().enumerate() {
            assert!(in_a_zone(*amount, each), "hit {k}: {amount}, expected {each} x a zone");
            assert_eq!(*amount, t[0].1, "both hits alike");
            let want = [0.667, 1.167][k];
            assert!((when - t0 - want).abs() < 0.06, "hit {k} at {}", when - t0);
        }
    }
}

/// Case 37: a Chainsaw's right button held hits 7 times a swing at 334,
/// 386, 437, 489, 540, 591, 643 ms, 15 each (105 a swing at roll 1.0), the
/// swing repeating every 900 ms.
#[test]
fn t37_chainsaw_secondary() {
    let (mut sim, p, target) = melee_target("test:chainsaw_exact", 100.0, std::f32::consts::PI);
    let e = eye(&sim, p);
    let torso = spheres(&mut sim, target)
        .into_iter()
        .filter(|s| s.2 == Location::Torso)
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap();
    let d = torso.0 - e;
    sim.intent(p).pitch = d.y.atan2(d.xz().length());
    sim.ticks(2);
    sim.intent(p).secondary = true;
    sim.seconds(2.0);
    sim.intent(p).secondary = false;
    sim.seconds(1.0);
    let t = taken(&sim, target);
    assert!(t.len() >= 14, "two swings: {}", t.len());
    for (_, amount) in &t {
        assert!(in_a_zone(*amount, 15.0) && *amount == t[0].1, "15 each x a zone: {amount}");
    }
    let first = t[0].0;
    let offsets = [0.0, 0.052, 0.103, 0.155, 0.206, 0.257, 0.309];
    for (k, off) in offsets.iter().enumerate() {
        assert!((t[k].0 - first - off).abs() < 0.04, "hit {k} at {}", t[k].0 - first);
        assert!((t[7 + k].0 - first - 0.9 - off).abs() < 0.06, "second swing hit {k} at {}", t[7 + k].0 - first);
    }
}

// ---------------------------------------------------------------------------
// The rest

/// Case 38: a LAW fired with an enemy 30 u behind the shooter: the enemy
/// takes nothing (no back-blast, no lock-on); the rocket goes along the aim.
#[test]
fn t38_no_lock_on_no_back_blast() {
    let mut sim = sim();
    solid(&mut sim, Vec3::new(-10.0, -1.0, 800.0 * U), Vec3::new(10.0, 10.0, 810.0 * U), None);
    let shooter = player(&mut sim, 0.0, 0.0, std::f32::consts::PI, 1);
    let behind = player(&mut sim, 0.0, -30.0, 0.0, 2);
    let aside = player(&mut sim, 600.0, 300.0, 0.0, 2);
    armed(&mut sim, shooter, LAW);
    sim.intent(shooter).fire = true;
    sim.ticks(1);
    sim.intent(shooter).fire = false;
    let (_, weapon, who, at) = until_boom(&mut sim, 2.0);
    assert_eq!((weapon, who), (LAW, Some(shooter)));
    assert!((at.z - 799.0).abs() < 1.5 && at.x.abs() < 2.0, "along the aim, at the wall: {at}");
    sim.ticks(3);
    assert_eq!(health(&sim, behind), 100.0, "nothing behind");
    assert_eq!(health(&sim, aside), 100.0, "nothing homed on");
    assert_eq!(health(&sim, shooter), 100.0);
}

/// Case 39: at a round's end a landed mine, a claymore, caltrops and a
/// burning sphere remove themselves on their next update; a rocket in
/// flight still detonates and damages.
#[test]
fn t39_round_end() {
    let mut sim = sim();
    let owner = far_player(&mut sim, 1);
    let victim = player(&mut sim, 0.0, 700.0, 0.0, 2);
    let mine = landed(&mut sim, MINE, Some(owner), 1000.0, 0.0);
    let claymore = landed(&mut sim, CLAYMORE, Some(owner), 1100.0, 0.0);
    let caltrops = landed(&mut sim, CALTROPS, Some(owner), 1200.0, 0.0);
    explode_at(&mut sim, M32_INCENDIARY, Some(owner), Vec3::new(-100.0, 0.0, 0.0));
    let spheres_before = sim.app.world_mut().query::<&CaDamageSphere>().iter(sim.app.world()).count();
    assert_eq!(spheres_before, 1);
    let rocket = projectile(&mut sim, LAW, Some(owner), Vec3::new(0.0, 40.0, 0.0), Vec3::new(0.0, 0.0, 1300.0));
    assert!(alive(&sim, rocket));
    let until = now(&sim) + 10.0;
    sim.app.world_mut().insert_resource(CaMatch {
        phase: Phase::RoundOver { until },
        ..default()
    });
    sim.ticks(10);
    assert!(!alive(&sim, mine) && !alive(&sim, claymore) && !alive(&sim, caltrops), "devices removed");
    assert_eq!(sim.app.world_mut().query::<&CaDamageSphere>().iter(sim.app.world()).count(), 0, "the sphere too");
    assert!(booms(&sim).iter().all(|b| b.1 != MINE && b.1 != CLAYMORE), "silently");
    let boom = until_boom(&mut sim, 2.0);
    assert_eq!(boom.1, LAW, "the rocket still detonates");
    sim.ticks(2);
    assert!(health(&sim, victim) < 100.0, "and damages");
}

const M32_INCENDIARY: &str = "combat_arms:m32_incendiarybomb";

/// Over the network: a client fires its LAW; both clients draw the rocket
/// in flight, the shot is predicted without error, and its blast's damage
/// reaches the victim's client.
#[test]
fn net_rocket_seen_and_damaging() {
    let link = LinkConditions {
        latency: Duration::from_millis(30),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(CaSim, link, 7, 2, |app| {
        app.add_plugins(GreyboxMapPlugin);
        app.insert_resource(StartingWeapons {
            team: Vec::new(),
            all: vec![combat_arms::game::weapons::RIFLE, combat_arms::game::inventory::KNIFE, M67, LAW],
        });
    });
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    sim.ticks(240);
    let me = sim.local_player(0).unwrap();
    // Key 5: the LAW.
    sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().select = Some(4);
    sim.ticks(1);
    sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().select = None;
    sim.ticks(120);
    let active = sim.clients[0].app.world().get::<Inventory>(me).unwrap().active.unwrap();
    assert_eq!(sim.clients[0].app.world().get::<Weapon>(active).unwrap().id, LAW, "drawn");
    sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().fire = true;
    sim.ticks(1);
    sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().fire = false;
    // Seen in flight on both clients.
    let mut seen = [false; 2];
    let ok = sim.until(300, |s| {
        for (i, c) in s.clients.iter_mut().enumerate() {
            let w = c.app.world_mut();
            // Drawn by its projectile's key (the record's model; none in
            // the built-in data).
            if w.query::<&ShownItem>().iter(w).any(|i| combat_arms::game::special::ProjectileParams::is_modelless(&i.0)) {
                seen[i] = true;
            }
        }
        seen == [true, true]
    });
    assert!(ok, "drawn on both clients: {seen:?}");
    for i in 0..2 {
        let g = sim.clients[i].app.world().resource::<chimera_net::net::predict::NetGraph>().clone();
        assert_eq!(g.errors, 0, "client {i} mispredicted the shot ({:?})", g.last_differing);
    }
    // Put it on the other player's feet on the server and let it go off.
    let victim = sim.character_of(1).unwrap();
    let at = sim.server.app.world().get::<Transform>(victim).unwrap().translation;
    {
        let w = sim.server.app.world_mut();
        let r = w.query_filtered::<Entity, With<CaObject>>().iter(w).next().expect("on the server");
        let mut o = w.get_mut::<CaObject>(r).unwrap();
        o.pos = (at + Vec3::new(0.5, 0.0, 0.0)) / U;
        o.velocity = Vec3::ZERO;
        o.destroyed = true;
    }
    let before = sim.server.app.world().get::<Health>(victim).unwrap().current;
    sim.ticks(300);
    let after = sim.server.app.world().get::<Health>(victim).unwrap().current;
    assert!(after < before, "the server's victim hurt");
    let theirs = sim.local_player(1).unwrap();
    let on_victim = sim.clients[1].app.world().get::<Health>(theirs).unwrap().current;
    assert_eq!(on_victim, after, "the victim's client has it");
}
