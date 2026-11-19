//! The Flame Thrower's flame points and the stream drawn on them
//! (specs/tf2/flames.md; `weapons::flame`, `weapons::looks::flame_stream`)
//! on the greybox (`harness::Tf2Sim`):
//!
//! - the drawn copy (`FlameShown`) is the server's damage points
//!   (`FlameStream`) tick for tick, on the same machine, on the firing
//!   client (predicted) and on another client (from the server's rounds);
//! - the stream's control points are those points, the newest active,
//!   one `new_flame` per burst, stopped on release; the crit and
//!   under-water names;
//! - damage only where the points are (no cone): a target beside the
//!   stream takes nothing, every hit on one in it has a point's box on him;
//! - a wall: the points slide on it and climb, never through it.

use std::collections::HashMap;

use bevy::prelude::*;
use chimera_core::core::{Health, Intent};
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_sim::weapon::{self, Inventory, Weapon};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
    map::psys::{ParticleControlPoints, StartParticleSystem, StopParticleSystems},
};
use tf2::{
    game::{
        TICK_INTERVAL,
        weapons::{
            FLAMETHROWER,
            flame::{FlamePoint, FlameShown, FlameStream, Tf2Flame, segment_meets_box},
            looks::{GunLooks, particle_systems},
        },
    },
    harness::Tf2Sim,
};

use super::weapons::{hold_still, holding, sim, target};

const U: f32 = 0.0254;

/// Look level along -Z.
fn level(sim: &mut Sim, p: Entity) {
    let mut i = sim.intent(p);
    i.yaw = 0.0;
    i.pitch = 0.0;
}

fn points(world: &World, w: Entity) -> (Vec<FlamePoint>, Vec<FlamePoint>) {
    let damage = world.get::<FlameStream>(w).map(|s| s.points.points.clone()).unwrap_or_default();
    let shown = world.get::<FlameShown>(w).map(|s| s.points.points.clone()).unwrap_or_default();
    (damage, shown)
}

/// On one machine, the drawn points are the damage points tick for tick
/// (the same rounds, seeds and motion; frames are ticks here), and the
/// stream's control points are the drawn points (slot i at i + 1, the
/// newest active).
#[test]
fn the_drawn_points_are_the_damage_points_and_the_streams_control_points() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[1], FLAMETHROWER);
    level(&mut sim, p);
    let mut compared = 0;
    for k in 0..60 {
        sim.intent(p).fire = k < 40;
        sim.ticks(1);
        let world = sim.app.world();
        let (damage, shown) = points(world, w);
        assert_eq!(damage.len(), shown.len(), "tick {k}");
        for (a, b) in damage.iter().zip(&shown) {
            assert_eq!(a.slot, b.slot);
            assert!((a.at - b.at).length() < 1e-4, "tick {k}: {:?} vs {:?}", a.at, b.at);
            assert!((a.before - b.before).length() < 1e-4);
            compared += 1;
        }
        let cps = world.get::<ParticleControlPoints>(w).expect("the stream's control points");
        assert_eq!(cps.points.len(), shown.len());
        for (p, (n, c)) in shown.iter().zip(&cps.points) {
            assert_eq!(*n, p.slot as usize + 1);
            assert_eq!(c.position, p.at);
            assert!((c.radius / U - 12.0).abs() < 1e-4 && c.duration == p.life);
        }
        assert_eq!(cps.active, shown.last().map(|p| p.slot as usize + 1));
    }
    assert!(compared > 300, "{compared}");
}

/// One `new_flame` per burst, tagged with the gun (its control points'
/// owner), stopped when the burst ends; another on the next burst.
#[test]
fn each_burst_starts_one_new_flame_and_stops_it() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[1], FLAMETHROWER);
    level(&mut sim, p);
    let mut starts = sim.app.world().resource::<Messages<StartParticleSystem>>().get_cursor_current();
    let mut stops = sim.app.world().resource::<Messages<StopParticleSystems>>().get_cursor_current();
    let (mut started, mut stopped) = (Vec::new(), Vec::new());
    for k in 0..120 {
        sim.intent(p).fire = k < 30 || (60..80).contains(&k);
        sim.ticks(1);
        let world = sim.app.world();
        started.extend(starts.read(world.resource::<Messages<StartParticleSystem>>()).map(|s| (k, s.clone())));
        stopped.extend(stops.read(world.resource::<Messages<StopParticleSystems>>()).map(|s| (k, *s)));
    }
    let names: Vec<&str> = started.iter().map(|(_, s)| s.name.as_str()).collect();
    assert_eq!(names, ["new_flame", "new_flame"], "{started:?}");
    assert!(started.iter().all(|(_, s)| s.tag == Some(w) && s.emit_for.is_none()));
    assert_eq!(stopped.len(), 2, "{stopped:?}");
    assert!(stopped.iter().all(|(_, s)| s.tag == w));
    // Stopped a little after each release (`burst_gap` 0.1 s), before the
    // next burst.
    assert!(stopped[0].0 > 30 && stopped[0].0 < 60 && stopped[1].0 > 80, "{stopped:?}");
}

/// flames.md 3.1: the crit stream by team, under water's.
#[test]
fn the_stream_names_crit_and_under_water() {
    let looks = GunLooks::default();
    let s = looks.stream_of(FLAMETHROWER).expect("the Flame Thrower's stream");
    assert_eq!(s.system_for(false, false, false), "new_flame");
    assert_eq!(s.system_for(false, true, false), "new_flame", "the stream itself has no team");
    assert_eq!(s.system_for(true, false, false), "new_flame_crit_red");
    assert_eq!(s.system_for(true, true, false), "new_flame_crit_blue");
    assert_eq!(s.system_for(true, true, true), "flamethrower_underwater");
    for n in ["new_flame", "new_flame_crit_red", "new_flame_crit_blue", "flamethrower_underwater"] {
        assert!(particle_systems().contains(&n), "{n} loads");
    }
    assert!(!particle_systems().contains(&"flamethrower"), "the old system isn't used");
}

/// Damage only where the points are (flames.md 2: no cone): a target
/// 60 units beside the stream 100 ahead takes nothing; one in it is hit,
/// and at every hit a point's swept box is on him.
#[test]
fn damage_lands_only_where_the_points_are() {
    // Beside the stream.
    let mut sim = sim();
    let from = greybox::SPAWNS[1];
    let (p, _) = holding(&mut sim, from, FLAMETHROWER);
    let beside = sim.spawn_character(from + Vec3::new(60.0, 0.0, -100.0) * U, chimera_sim::movement::placeholder::ID);
    sim.seconds(1.0);
    level(&mut sim, p);
    let before = sim.app.world().get::<Health>(beside).unwrap().current;
    for _ in 0..90 {
        sim.intent(p).fire = true;
        hold_still(&mut sim, beside);
        sim.ticks(1);
    }
    assert_eq!(sim.app.world().get::<Health>(beside).unwrap().current, before, "no cone");

    // In it.
    let mut sim = super::weapons::sim();
    let (p, w) = holding(&mut sim, from, FLAMETHROWER);
    let near = target(&mut sim, from, 150.0);
    sim.app.world_mut().get_mut::<Health>(near).unwrap().current = 1000.0;
    level(&mut sim, p);
    let mut hits = 0;
    let mut last = sim.app.world().get::<Health>(near).unwrap().current;
    for _ in 0..90 {
        sim.intent(p).fire = true;
        hold_still(&mut sim, near);
        sim.ticks(1);
        let world = sim.app.world();
        let h = world.get::<Health>(near).unwrap().current;
        if h < last && world.get::<tf2::game::weapons::flame::Burning>(near).is_some_and(|b| b.left > 0.0) && last - h > 0.045 {
            // A flame hit (not the 0.04 of afterburn): a point's box on him.
            let (min, max) = victim_box(world, near);
            let half = Vec3::splat(12.0 * U);
            let (damage, _) = points(world, w);
            assert!(
                damage.iter().any(|pt| segment_meets_box(pt.before, pt.at, min - half, max + half)),
                "a hit with no point on him"
            );
            hits += 1;
        }
        last = h;
    }
    assert!(hits >= 5, "{hits}");
}

fn victim_box(world: &World, e: Entity) -> (Vec3, Vec3) {
    let aabb = world.get::<avian3d::prelude::ColliderAabb>(e).expect("a box");
    (aabb.min, aabb.max)
}

/// flames.md 1: points meeting a wall slide on it and climb (the drift
/// kept each update), never through it.
#[test]
fn points_slide_up_a_wall_never_through_it() {
    let mut sim = sim();
    // 150 units from the north wall's face.
    let from = Vec3::new(25.0, 1.0, greybox::NORTH_WALL_Z + 150.0 * U);
    let (p, w) = holding(&mut sim, from, FLAMETHROWER);
    level(&mut sim, p);
    let face = greybox::NORTH_WALL_Z + 12.0 * U;
    let (mut slid, mut climbing) = (0, 0);
    // The damage points' last upward speed and walls met, by birth.
    let mut last: HashMap<(u8, u64), (f32, u16)> = HashMap::new();
    for _ in 0..60 {
        sim.intent(p).fire = true;
        sim.ticks(1);
        let (damage, shown) = points(sim.app.world(), w);
        for pt in &damage {
            // Meeting the wall, its velocity keeps the drift: it gains
            // upward speed (TF2's slide quirk) where a free point loses
            // some to the drag.
            let key = (pt.slot, pt.born.to_bits());
            if let Some((vy, hits)) = last.get(&key)
                && pt.wall_hits > *hits
            {
                assert!(pt.velocity.y > *vy, "{} -> {}", vy / U, pt.velocity.y / U);
                climbing += 1;
            }
            last.insert(key, (pt.velocity.y, pt.wall_hits));
        }
        for pt in damage.iter().chain(&shown) {
            assert!(pt.at.z >= face - 1e-3, "through the wall: {}", (pt.at.z - greybox::NORTH_WALL_Z) / U);
            if pt.wall_hits > 0 {
                slid += 1;
                assert_eq!(pt.carried, Vec3::ZERO);
                assert!(pt.velocity.z > -1e-3, "nothing into the wall: {:?}", pt.velocity / U);
            }
        }
    }
    assert!(slid > 50 && climbing > 5, "{slid} {climbing}");
}

/// The visible points match the damage points tick for tick on the
/// server and the clients (flames.md 1): the firing client's own
/// (predicted) and another client's (from the server's rounds) are the
/// server's, point by point and update by update.
#[test]
fn every_client_draws_the_servers_points() {
    let link = LinkConditions { latency: std::time::Duration::from_millis(40), ..LinkConditions::default() };
    let mut sim = NetSim::new(Tf2Sim, link, 5, 0, |app| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.add_client(|_| {});
    sim.add_client(|_| {});
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(900);
    sim.until_looks_taken(600);
    let theirs = sim.character_of(0).unwrap();
    let sw = weapon::give(sim.server.app.world_mut(), theirs, FLAMETHROWER).unwrap();
    sim.server.app.world_mut().get_mut::<Inventory>(theirs).unwrap().wanted = Some(sw);
    sim.ticks(150);
    let me = sim.local_player(0).unwrap();
    let mine = sim.clients[0].app.world().get::<Inventory>(me).unwrap().active.expect("drawn");
    assert_eq!(sim.clients[0].app.world().get::<Weapon>(mine).unwrap().id, FLAMETHROWER);
    // Each machine's points by birth: (its slot, its birth) -> places.
    type Tracks = (HashMap<(u8, u64), usize>, Vec<Vec<Vec3>>);
    let mut tracks: [Tracks; 3] = Default::default();
    let record = |t: &mut Tracks, pts: &[FlamePoint]| {
        for pt in pts {
            let key = (pt.slot, pt.born.to_bits());
            let n = t.1.len();
            let i = *t.0.entry(key).or_insert(n);
            if i == t.1.len() {
                t.1.push(Vec::new());
            }
            t.1[i].push(pt.at);
        }
    };
    for k in 0..90 {
        {
            let me = sim.local_player(0).unwrap();
            let mut i = sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap();
            i.fire = k < 40;
            i.yaw = 0.3;
            i.pitch = -0.05;
        }
        sim.step();
        let (server, _) = points(sim.server.app.world(), sw);
        record(&mut tracks[0], &server);
        let (_, own) = points(sim.clients[0].app.world(), mine);
        record(&mut tracks[1], &own);
        // Client 1's copy of the shooter and the gun in his hands.
        let world = sim.clients[1].app.world_mut();
        let guns: Vec<Entity> = world.query_filtered::<Entity, With<Tf2Flame>>().iter(world).collect();
        let shown: Vec<FlamePoint> = guns.iter().flat_map(|g| points(world, *g).1).collect();
        record(&mut tracks[2], &shown);
    }
    let server = &tracks[0].1;
    assert!(server.len() >= 15, "{} points", server.len());
    for (who, t) in [("the firing client", &tracks[1].1), ("another client", &tracks[2].1)] {
        assert_eq!(t.len(), server.len(), "{who}: every point");
        for (i, (a, b)) in server.iter().zip(t).enumerate() {
            let n = a.len().min(b.len());
            assert!(n + 2 >= a.len(), "{who}: point {i} lived {} updates of the server's {}", b.len(), a.len());
            for k in 0..n {
                assert!((a[k] - b[k]).length() < 0.01 * U, "{who}: point {i} update {k}: {:?} vs {:?}", a[k] / U, b[k] / U);
            }
        }
    }
}
