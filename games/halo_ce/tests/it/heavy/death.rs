//! A Halo player's death with the disc's Spartan (`game::body::death`,
//! animations.md "Deaths (every seat)", health_shields.md "Death",
//! multiplayer_rules.md 5.4): the death animations in the body's set with
//! the spec's frame counts and the landing death's sound at frame 2; on a
//! flat floor with `HaloSim`, a Spartan killed from the front in the chest
//! leaves a body playing the soft front chest kill where it stood, one
//! killed in the air the airborne death, falling onto the floor; the
//! player itself respawns elsewhere and the body stays; a body lying idle
//! more than 900 ticks goes. Passes with a note without an install.

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Hitgroup};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use halo_ce::{
    game::{
        body::{
            death::{HaloCorpse, death_frames, death_sound},
            load::{self, DONOR_MAP, Spartan},
        },
        maps, movement,
    },
    harness::HaloSim,
};

use super::maps::mount;

fn spartan() -> Option<Spartan> {
    let mount = mount()?;
    let bytes = mount.read(&maps::map_path(DONOR_MAP)).unwrap();
    let file = blam_formats::cache::MapFile::load(&bytes).unwrap();
    Some(load::spartan(&file, false).unwrap())
}

#[test]
fn the_deaths_are_the_specs() {
    let Some(body) = spartan() else { return };
    let set = body.character.animations.as_ref().unwrap();
    for (name, frames) in [
        ("stand airborne-dead", 37),
        ("stand landing-dead", 42),
        ("damage s-kill front gut", 69),
        ("damage s-kill front chest", 44),
        ("damage s-kill left gut", 30),
        ("damage s-kill right gut", 30),
        ("damage s-kill back gut", 43),
        ("damage h-kill front gut", 46),
        ("damage h-kill front head", 60),
        ("damage h-kill left gut", 41),
        ("damage h-kill left head", 44),
        ("damage h-kill right gut", 41),
        ("damage h-kill right head", 44),
        ("damage h-kill back gut", 40),
    ] {
        assert!(set.names.contains_key(name), "{name} missing");
        assert_eq!(death_frames(set, name), frames, "{name}");
    }
    let sound = death_sound(set, "stand landing-dead");
    eprintln!("landing-dead's sound: {sound:?}");
    assert_eq!(sound.map(|s| s.0), Some(2), "sound at frame 2");
    for name in ["damage s-kill front gut", "damage h-kill left head", "stand airborne-dead"] {
        eprintln!("{name}: sound {:?}", death_sound(set, name));
    }
    // Pings: the soft ones only (no h-ping on the Spartan).
    assert!(set.names.contains_key("damage s-ping front gut"));
    assert!(!set.names.contains_key("damage h-ping front gut"));
}

fn floor(body: &Spartan) -> chimera_world::map::MapData {
    let mut map = chimera_world::map::MapData {
        collision_positions: vec![
            [-100.0, 0.0, -100.0],
            [100.0, 0.0, -100.0],
            [100.0, 0.0, 100.0],
            [-100.0, 0.0, 100.0],
        ],
        collision_indices: vec![[0, 2, 1], [0, 3, 2]],
        spawns: vec![(Vec3::new(0.0, 0.1, 0.0), None)],
        ..Default::default()
    };
    load::add(&mut map, body);
    map
}

fn kill(sim: &mut Sim, target: Entity, dir: Vec3, hitgroup: Hitgroup) {
    let at = sim.position(target);
    sim.app.world_mut().write_message(Damage {
        target,
        attacker: None,
        amount: 50.0,
        point: at,
        dir,
        hitgroup,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(1);
}

fn corpses(sim: &mut Sim) -> Vec<(Entity, HaloCorpse, Vec3)> {
    let w = sim.app.world_mut();
    w.query::<(Entity, &HaloCorpse, &Transform)>().iter(w).map(|(e, c, t)| (e, c.clone(), t.translation)).collect()
}

#[test]
fn a_dead_spartan_leaves_a_body_that_lies_where_it_fell() {
    let Some(body) = spartan() else { return };
    let mut sim = Sim::new(HaloSim, MapSimPlugin::new(floor(&body)));
    sim.ticks(3);
    let p = sim.spawn_character(Vec3::new(0.0, 1.2, 0.0), movement::ID);
    sim.seconds(1.0);
    // Facing -Z; a shot travelling +Z came from the front.
    kill(&mut sim, p, Vec3::Z, Hitgroup::Chest);
    sim.ticks(2);
    let bodies = corpses(&mut sim);
    assert_eq!(bodies.len(), 1, "one body");
    let (e, c, at) = bodies[0].clone();
    assert_eq!(c.owner, p);
    assert_eq!(c.animation, "damage s-kill front chest");
    assert!(at.y.abs() < 0.05, "its feet on the floor: {at}");
    // Its animation runs out (44 frames) and it stays.
    sim.seconds(3.0);
    let (_, _, later) = corpses(&mut sim)[0].clone();
    assert!(later.distance(at) < 1e-3, "lying still: {later}");
    // At rest from the death (health_shields.md "How long a body lasts":
    // the count runs while it is at rest, its animation playing or not)
    // for more than 900 ticks: gone on the 901st, 30.03 s after the death.
    sim.seconds(26.7);
    assert!(sim.app.world().get_entity(e).is_ok(), "still there 29.8 s after the death");
    sim.seconds(0.5);
    assert!(sim.app.world().get_entity(e).is_err(), "gone after 900 ticks at rest");
}

#[test]
fn a_spartan_killed_in_the_air_falls_to_the_floor() {
    let Some(body) = spartan() else { return };
    let mut sim = Sim::new(HaloSim, MapSimPlugin::new(floor(&body)));
    sim.ticks(3);
    // Dropped from 7 m: still in the air when the shot lands.
    let p = sim.spawn_character(Vec3::new(0.0, 7.2, 0.0), movement::ID);
    sim.ticks(6);
    kill(&mut sim, p, Vec3::NEG_Z, Hitgroup::Head);
    sim.ticks(1);
    let (_, c, start) = corpses(&mut sim)[0].clone();
    assert_eq!(c.animation, "stand airborne-dead");
    assert!(start.y > 4.0, "dies in the air: {start}");
    sim.seconds(3.0);
    let (_, _, end) = corpses(&mut sim)[0].clone();
    assert!(end.y.abs() < 0.05, "lies on the floor: {end}");
}

