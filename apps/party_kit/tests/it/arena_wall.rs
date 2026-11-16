//! An arena slice's wall against every game the party apps play
//! (docs/plans/active/arena-slices.md, A1; `chimera_world::map::play_area`):
//! headless on the party's greybox with a play area made the way a match
//! on a slice makes it (`play_area::apply`). Each game's movement (CS:S's,
//! HL2:DM's and TF2's Source movement, which meets the wall through its
//! physics casts, not its brush sweep; Combat Arms'; Halo's) walking and
//! jumping at the wall from inside stays inside, and nobody is put back;
//! each game's bullet and projectile (a rocket, a grenade) fired from
//! inside at a player beyond a slot in the play area (two walls between
//! them) stops at the wall: the player takes no damage, where without the
//! wall the same shots hurt them (the control). A character outside is
//! put back at a spawn point inside, the map's spawn points outside stop
//! being spawn points, and the map's teleporters with an end outside go.

use bevy::prelude::*;
use chimera_core::core::{Health, SpawnPoint};
use chimera_sim::minigame::ladder::Ladder;
use chimera_world::{
    greybox::GreyboxMapPlugin,
    harness::Sim,
    map::{
        play_area::{self, ArenaSlice, ArenaWall, PlayArea},
        teleport::{MapTeleporters, Teleporter},
    },
};
use combat_arms::harness::CaSim;
use party_kit::PartyPartsPlugin;

use crate::gun_game::{OPEN, line_up, shoot_until_dead};

const CSS: &str = cs_source::game::movement::ID;
const HL2: &str = hl2dm::game::movement::ID;
const TF2: &str = tf2::game::movement::ID;
const CA: &str = combat_arms::game::movement::ID;
const HALO: &str = halo_ce::game::movement::ID;

fn slice(points: &[[f32; 2]], seed: Vec3) -> ArenaSlice {
    ArenaSlice {
        map: "chimera:greybox".into(),
        id: "test".into(),
        name: "Greybox: Test".into(),
        points: points.iter().map(|p| Vec2::new(p[0], p[1])).collect(),
        seed,
    }
}

/// A box about the greybox's open middle: x -5..5, z -4..4.
fn square() -> ArenaSlice {
    slice(&[[-5.0, -4.0], [5.0, -4.0], [5.0, 4.0], [-5.0, 4.0]], Vec3::new(0.0, 0.1, 0.0))
}

/// `s` played from now (the spots made again, as on a map loaded for it).
fn play(sim: &mut Sim, s: ArenaSlice) {
    play_area::apply(sim.app.world_mut(), s).expect("the slice plays");
    sim.app.world_mut().remove_resource::<chimera_sim::minigame::spawns::SpawnSpots>();
    sim.ticks(3);
}

fn put_backs(sim: &Sim) -> u32 {
    sim.app.world().resource::<PlayArea>().put_backs
}

/// `p` running along `dir` (world x, z) and jumping for 3 s: the farthest
/// its centre got past the polygon (0: always inside).
fn run_at_wall(sim: &mut Sim, p: Entity, dir: Vec2) -> f32 {
    let yaw = (-dir.x).atan2(-dir.y);
    let mut worst: f32 = 0.0;
    for t in 0..(3.0 * sim.tick_hz()) as u32 {
        {
            let mut i = sim.intent(p);
            i.yaw = yaw;
            i.pitch = 0.0;
            i.move_axis = Vec2::Y;
            i.jump = t % 20 < 10;
        }
        sim.ticks(1);
        let at = sim.position(p);
        let poly = sim.app.world().resource::<PlayArea>().polygon;
        worst = worst.max(poly.outside_by(Vec2::new(at.x, at.z)));
    }
    sim.intent(p).move_axis = Vec2::ZERO;
    worst
}

/// The guard: every game's movement, walking and jumping at the wall (head
/// on and along a corner), stays inside, and nobody is put back.
#[test]
fn every_movement_stays_inside_the_wall() {
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin));
    sim.ticks(2);
    play(&mut sim, square());
    for movement in [CSS, HL2, TF2, CA, HALO] {
        for dir in [Vec2::new(0.0, -1.0), Vec2::new(1.0, 1.0).normalize()] {
            let p = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement);
            sim.seconds(0.5);
            let past = run_at_wall(&mut sim, p, dir);
            assert!(past == 0.0, "{movement} toward {dir}: {past} m past the wall");
            let at = sim.position(p);
            // It got to the wall (the run reached it).
            let poly = sim.app.world().resource::<PlayArea>().polygon;
            assert!(poly.edge_distance(Vec2::new(at.x, at.z)) < 1.5, "{movement} toward {dir}: stopped at {at}");
            sim.app.world_mut().despawn(p);
        }
    }
    // No map systems here (no put-back): an escape shows as `past`.
    assert_eq!(put_backs(&sim), 0);
}

/// Made: one wall piece per edge; the greybox's spawn points outside are
/// spawn points no more, one stands at the seed; a teleporter with an end
/// outside goes; a character put outside is put back inside, counted.
#[test]
fn the_play_area_holds_spawns_teleporters_and_characters() {
    // With the map's systems (`MapSimPlugin`: the put-back's), a match on.
    let mut sim = crate::halo_ladder::party_with(&ladder());
    sim.ticks(2);
    let inside = Vec3::new(1.0, 0.0, 1.0);
    let outside = Vec3::new(20.0, 0.0, 1.0);
    let pad = |source, dest| Teleporter {
        source,
        source_yaw: 0.0,
        dest,
        dest_yaw: 0.0,
    };
    sim.app.world_mut().insert_resource(MapTeleporters {
        pads: vec![pad(inside, -inside), pad(inside, outside), pad(outside, inside)],
        radius: 0.5,
        rearm: 1.0,
        ..default()
    });
    let s = square();
    play(&mut sim, s.clone());
    let w = sim.app.world_mut();
    assert_eq!(w.query::<&ArenaWall>().iter(w).count(), 4);
    let points: Vec<Vec3> = w
        .query_filtered::<&Transform, With<SpawnPoint>>()
        .iter(w)
        .map(|t| t.translation)
        .collect();
    let poly = s.polygon().unwrap();
    assert!(!points.is_empty() && points.iter().all(|p| poly.contains3(*p)), "{points:?}");
    assert!(points.iter().any(|p| (p.x - s.seed.x).abs() < 1e-4 && (p.z - s.seed.z).abs() < 1e-4), "one at the seed");
    assert_eq!(w.resource::<MapTeleporters>().pads.len(), 1, "only the pad inside");
    assert_eq!(put_backs(&sim), 0);
    // A player in the match (a fresh character is dead until the match
    // takes it in: the dead are left to their respawn).
    let p = crate::gun_game::soldier(&mut sim);
    sim.seconds(1.5);
    assert!(crate::gun_game::alive(&sim, p));
    crate::gun_game::place(&mut sim, p, Vec3::new(7.0, 1.0, 0.0));
    sim.ticks(3);
    let at = sim.position(p);
    assert!(poly.contains3(at), "put back inside: {at}, {} put back", put_backs(&sim));
    assert_eq!(put_backs(&sim), 1);
}

/// Shots of every game at a player beyond a slot of the play area (two
/// walls between): (gun, seconds of fire).
const SHOTS: [(&str, f64); 10] = [
    ("cs_source:weapon_ak47", 2.0),
    (cs_source::game::grenades::HEGRENADE, 4.0),
    (hl2dm::game::weapons::SMG1, 2.0),
    (hl2dm::game::weapons::RPG, 4.0),
    (tf2::game::weapons::SCATTERGUN, 2.0),
    (tf2::game::weapons::ROCKET_LAUNCHER, 3.0),
    ("combat_arms:m4a1", 2.0),
    ("combat_arms:rpg-7", 4.0),
    (halo_ce::game::weapons::PISTOL, 2.0),
    (halo_ce::game::weapons::ROCKET_LAUNCHER, 3.0),
];

/// How far south of `OPEN` the target stands.
const RANGE: f32 = 8.0;

/// The play area for the shots: x -10..10, z -14..4, with a slot from the
/// west edge to x 7 between z -5.5 and -4.5 (outside: a wall on each
/// side), between `OPEN` (z 0) and a player `RANGE` m south of it.
fn slotted() -> ArenaSlice {
    slice(
        &[
            [-10.0, -14.0],
            [10.0, -14.0],
            [10.0, 4.0],
            [-10.0, 4.0],
            [-10.0, -4.5],
            [7.0, -4.5],
            [7.0, -5.5],
            [-10.0, -5.5],
        ],
        Vec3::new(0.0, 0.1, 0.0),
    )
}

fn ladder() -> String {
    // A last rung no shot climbs past (a kill on the last would win).
    let mut guns: Vec<&str> = SHOTS.iter().map(|s| s.0).collect();
    guns.push("cs_source:weapon_glock");
    crate::hl2dm_parts::ladder_text(&guns, false).replace(
        "weapons = [\"cs_source\", \"hl2dm\", \"combat_arms\"]",
        "weapons = [\"cs_source\", \"hl2dm\", \"tf2\", \"halo_ce\", \"combat_arms\"]",
    )
}

/// Each gun fired at `b` from `a`: whether `b` was hurt.
fn hurt_by_each(sim: &mut Sim, a: Entity, b: Entity) -> Vec<(&'static str, bool)> {
    let ladder = sim.app.world().resource::<Ladder>().clone();
    let mut out = Vec::new();
    for (gun, secs) in SHOTS {
        let r = ladder.rungs.iter().position(|w| *w == gun).unwrap_or_else(|| panic!("{gun} on the ladder"));
        {
            let w = sim.app.world_mut();
            chimera_sim::minigame::ladder::set_rung(w, a, r, &ladder);
        }
        line_up(sim, a, b, RANGE);
        let before = sim.app.world().get::<Health>(b).map_or(0.0, |h| h.current);
        let died = shoot_until_dead(sim, a, b, secs);
        // Let a last rocket or grenade arrive.
        sim.seconds(1.5);
        let after = sim.app.world().get::<Health>(b).map_or(0.0, |h| h.current);
        let dead = died || !crate::gun_game::alive(sim, b);
        out.push((gun, dead || after < before - 1e-4));
        // Back to full for the next gun.
        if let Some(mut h) = sim.app.world_mut().get_mut::<Health>(b) {
            h.current = h.max;
        }
    }
    out
}

/// The guard: no game's bullet, rocket or grenade passes the wall; the
/// control: without it each hurts.
#[test]
fn every_games_shots_stop_at_the_wall() {
    let text = ladder();
    let mut sim = crate::halo_ladder::party_with(&text);
    let a = crate::gun_game::soldier(&mut sim);
    let b = crate::gun_game::soldier(&mut sim);
    sim.seconds(1.5);
    let control = hurt_by_each(&mut sim, a, b);
    let missed: Vec<_> = control.iter().filter(|(_, hurt)| !hurt).map(|(g, _)| *g).collect();
    assert!(missed.is_empty(), "without the wall these did not hurt (the control): {missed:?}");
    play(&mut sim, slotted());
    assert!(sim.app.world().resource::<PlayArea>().polygon.contains3(OPEN));
    let through: Vec<_> = hurt_by_each(&mut sim, a, b).into_iter().filter(|(_, hurt)| *hurt).map(|(g, _)| g).collect();
    assert!(through.is_empty(), "through the wall: {through:?}");
    assert_eq!(put_backs(&sim), 0);
}
