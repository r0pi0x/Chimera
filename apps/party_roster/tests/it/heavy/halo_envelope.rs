//! A Halo player on other games' maps under the movement envelope rule
//! (`chimera_core::envelope`, Halo's `movement::body`; the user's installs):
//! from places the Spartan's own body was not walked to by `map_fitness
//! --walker 0.61,2.13,0.38` (de_dust2's steps, cs_office's low rooms, a
//! Combat Arms warehouse spawn), the map's own character walks along eight
//! headings for 3 s, and a Halo player follows each path (steering at its
//! points in turn) to within 0.75 m of its end (0.5 m in height). The
//! Spartan's own body (the map's envelope taken away) is followed too and
//! printed: measured 2026-11-15 it also made every one of these 24 paths
//! (Halo's mover rides edges and drops off stairs that the flood's
//! walker counts as walls), so the envelope's own effect is shown by the
//! synthetic levels (`halo_ce` `movement_envelope`), not here. Each test
//! passes with a note without the map's game's install.

use bevy::prelude::*;
use chimera_core::{
    envelope::MovementEnvelopes,
    mount::config::LocalConfig,
    slots::MovementRegistry,
};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use combat_arms::harness::CaSim;
use party_roster::PartyPartsPlugin;

/// The party on map `id` of `game` (Source's or Combat Arms' install);
/// None (with a note) without it.
fn party_on(game: &str, id: &str) -> Option<Sim> {
    let config = LocalConfig::load().ok()?;
    let there = if game == cs_source::game::GAME {
        config
            .game_path_in(cs_source::game::GAME, cs_source::game::INFO.steam.as_ref())
            .filter(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"))
    } else {
        config
            .game_path(combat_arms::game::GAME)
            .filter(|p| chimera_core::mount::served::usable("combat_arms", p, "Game"))
    };
    if there.is_none() {
        eprintln!("skipping: needs the {game} install");
        return None;
    }
    let map = party_roster::load_map(id, 0).unwrap();
    let mut sim = Sim::new(CaSim, (MapSimPlugin::new(map), PartyPartsPlugin));
    sim.ticks(2);
    Some(sim)
}

/// A character's feet, as its movement says (`slots::Footing`).
fn feet(sim: &Sim, movement: &str, e: Entity) -> Vec3 {
    let w = sim.app.world();
    let footing = w.resource::<MovementRegistry>().get(movement).and_then(|m| m.footing).expect("a footing");
    footing(w, e).expect("standing").feet
}

/// The feet of a character of `movement` put down at `at` (feet, engine)
/// and walked along `heading` (radians, an intent's yaw) for `secs`,
/// crouched if `crouch`, a tick at a time (None: it did not settle within
/// 0.3 m of `at` across, 1.5 m in height).
fn walk(sim: &mut Sim, movement: &'static str, at: Vec3, heading: f32, secs: f32, crouch: bool) -> Option<Vec<Vec3>> {
    let p = sim.spawn_character(at + Vec3::Y * if crouch { 0.7 } else { 1.0 }, movement);
    sim.intent(p).crouch = crouch;
    sim.seconds(1.0);
    let start = feet(sim, movement, p);
    if start.xz().distance(at.xz()) > 0.3 || (start.y - at.y).abs() > 1.5 {
        eprintln!("{movement} did not settle at {at}: {start}");
        sim.app.world_mut().despawn(p);
        return None;
    }
    {
        let mut i = sim.intent(p);
        i.yaw = heading;
        i.move_axis = Vec2::Y;
    }
    let dt = sim.app.world().resource::<chimera_core::core::SimClock>().delta.as_secs_f32();
    let mut path = vec![start];
    for _ in 0..(secs / dt) as u32 {
        sim.ticks(1);
        path.push(feet(sim, movement, p));
    }
    sim.app.world_mut().despawn(p);
    Some(path)
}

/// Whether a character of `movement` put down at `path[0]` and steering
/// at each of `path`'s points in turn (the next once within 0.5 m
/// across) comes within 0.75 m across and 0.5 m in height of its last,
/// within `secs`; and the feet it ended at.
fn follows(sim: &mut Sim, movement: &'static str, path: &[Vec3], secs: f32, crouch: bool) -> (bool, Vec3) {
    let start = path[0];
    let goal = *path.last().unwrap();
    let p = sim.spawn_character(start + Vec3::Y * if crouch { 0.7 } else { 1.0 }, movement);
    sim.intent(p).crouch = crouch;
    sim.seconds(1.0);
    let dt = sim.app.world().resource::<chimera_core::core::SimClock>().delta.as_secs_f32();
    let mut next = 1.min(path.len() - 1);
    let mut now = feet(sim, movement, p);
    let mut ok = false;
    for _ in 0..(secs / dt) as u32 {
        now = feet(sim, movement, p);
        if now.xz().distance(goal.xz()) < 0.75 && (now.y - goal.y).abs() < 0.5 {
            ok = true;
            break;
        }
        while next + 1 < path.len() && now.xz().distance(path[next].xz()) < 0.5 {
            next += 1;
        }
        let d = path[next] - now;
        let mut i = sim.intent(p);
        i.yaw = (-d.x).atan2(-d.z);
        i.move_axis = Vec2::Y;
        sim.ticks(1);
    }
    sim.app.world_mut().despawn(p);
    (ok, now)
}

/// From `at` (feet, engine) on map `id` of `game`, the map's own
/// character (`own`) walks along eight headings for `secs`, and a Halo
/// player follows each path it took 1.5 m or more, with the map's
/// envelope and with the Spartan's own body: (paths followed, the
/// headings (degrees) Halo missed, those the Spartan's own body missed).
fn compare(game: &str, id: &str, own: &'static str, at: Vec3, secs: f32, crouch: bool) -> Option<(usize, Vec<u32>, Vec<u32>)> {
    let mut sim = party_on(game, id)?;
    let halo = halo_ce::game::movement::ID;
    let mut tried = 0;
    let (mut missed, mut missed_alone) = (Vec::new(), Vec::new());
    for k in 0..8u32 {
        let heading = (k as f32 * 45.0).to_radians();
        let Some(theirs) = walk(&mut sim, own, at, heading, secs, crouch) else { continue };
        let goal = *theirs.last().unwrap();
        if goal.xz().distance(theirs[0].xz()) < 1.5 {
            continue;
        }
        tried += 1;
        let mut path = vec![theirs[0]];
        for q in &theirs {
            if q.distance(*path.last().unwrap()) > 0.5 {
                path.push(*q);
            }
        }
        path.push(goal);
        let (ours, ours_end) = follows(&mut sim, halo, &path, secs * 1.5 + 1.0, crouch);
        // The map's envelope taken away: the Spartan's own body.
        let envelopes = sim.app.world().resource::<MovementEnvelopes>().clone();
        sim.app.world_mut().resource_mut::<MovementEnvelopes>().0.retain(|(g, _)| *g != game);
        let (alone, alone_end) = follows(&mut sim, halo, &path, secs * 1.5 + 1.0, crouch);
        sim.app.world_mut().insert_resource(envelopes);
        eprintln!(
            "{id} from {at} heading {}: theirs to {goal}; Halo {ours} (at {ours_end}), the Spartan's own body {alone} (at {alone_end})",
            k * 45
        );
        if !ours {
            missed.push(k * 45);
        }
        if !alone {
            missed_alone.push(k * 45);
        }
    }
    Some((tried, missed, missed_alone))
}

fn check(result: Option<(usize, Vec<u32>, Vec<u32>)>) {
    let Some((tried, missed, alone)) = result else { return };
    assert!(tried > 0, "the map's character walked somewhere");
    assert!(missed.is_empty(), "the Halo player missed headings {missed:?} of {tried}");
    eprintln!("the Spartan's own body missed headings {alone:?} of {tried}");
}

/// Source's (x, y, z) units as engine feet.
fn source(x: f32, y: f32, z: f32) -> Vec3 {
    cs_source::game::movement::to_engine(Vec3::new(x, y, z))
}

/// de_dust2 from (-1939, 2062, 8) (Source units), the middle of a
/// place the Spartan's own body did not walk to (`map_fitness --walker
/// 0.61,2.13,0.38`: 177 m2, floors 0 to 0.8 m: steps).
#[test]
fn a_halo_player_follows_a_terrorist_on_de_dust2() {
    check(compare(cs_source::game::GAME, "cs_source:de_dust2", cs_source::game::movement::ID, source(-1939.0, 2062.0, 8.0), 3.0, false));
}

/// cs_office from (-732, 605, -160) (Source units), the middle of a low
/// place the Spartan's own body did not walk to (83 m2).
#[test]
fn a_halo_player_follows_a_counter_terrorist_in_cs_offices_low_rooms() {
    check(compare(cs_source::game::GAME, "cs_source:cs_office", cs_source::game::movement::ID, source(-732.0, 605.0, -160.0), 3.0, false));
}

/// Combat Arms' warehouse from the spawn point nearest (-13.9, 16.1) m
/// across (whose rooms the flood's Spartan could not leave), down to the
/// floor at -106.4 m among others.
#[test]
fn a_halo_player_follows_a_soldier_out_of_a_warehouse_spawn() {
    let Some(mut sim) = party_on(combat_arms::game::GAME, "combat_arms:warehouse") else { return };
    let w = sim.app.world_mut();
    let spawn = w
        .query_filtered::<&Transform, With<chimera_core::core::SpawnPoint>>()
        .iter(w)
        .map(|t| t.translation - Vec3::Y * chimera_world::map::SPAWN_LIFT)
        .min_by(|a, b| a.xz().distance(Vec2::new(-13.9, 16.1)).total_cmp(&b.xz().distance(Vec2::new(-13.9, 16.1))))
        .expect("a spawn point");
    drop(sim);
    check(compare(combat_arms::game::GAME, "combat_arms:warehouse", combat_arms::game::movement::ID, spawn, 3.0, false));
}
