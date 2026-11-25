//! How CS 1.6's grenades look going off (`game::weapons::looks`): an HE
//! blast of this game draws its explosion sprite once at the blast point,
//! a smoke one puff a second of its puff sprite, 21 times; another game's
//! blast draws nothing here. The sprites stand in as named materials (the
//! install's are read in `heavy::guns`).

use bevy::prelude::*;
use chimera_sim::weapon::grenade::{Detonated, GrenadeKind};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
    map::particles::{MapParticles, ParticleMaterial, ParticleMaterials, Particles},
};
use cs16::{
    game::weapons::{HEGRENADE, SMOKEGRENADE, looks},
    harness::Cs16Sim,
};

fn sim() -> Sim {
    let mut sim = Sim::new(Cs16Sim, GreyboxMapPlugin);
    sim.seconds(0.5);
    let material = |name: &str| ParticleMaterial { name: name.into(), ..Default::default() };
    sim.app.insert_resource(ParticleMaterials(MapParticles {
        materials: vec![material(looks::HE_SPRITE), material(looks::SMOKE_SPRITE)],
    }));
    sim.app.init_resource::<Particles>();
    sim
}

fn go_off(sim: &mut Sim, kind: GrenadeKind, weapon: &'static str, at: Vec3) {
    let projectile = sim.app.world_mut().spawn_empty().id();
    sim.app.world_mut().write_message(Detonated {
        kind,
        weapon,
        thrower: None,
        at,
        ground: None,
        projectile,
        shake: None,
        flags: Default::default(),
    });
}

/// The particles alive by material.
fn drawn(sim: &Sim, material: usize) -> Vec<Vec3> {
    let p = sim.app.world().resource::<Particles>();
    p.groups.iter().flat_map(|g| &g.particles).filter(|p| p.material == material).map(|p| p.position).collect()
}

#[test]
fn an_he_blast_draws_its_sprite() {
    let mut sim = sim();
    let at = greybox::SPAWNS[0] + Vec3::Y;
    go_off(&mut sim, GrenadeKind::Blast, "cs_source:weapon_hegrenade", at);
    sim.ticks(2);
    assert!(drawn(&sim, 0).is_empty(), "another game's blast draws its own");
    go_off(&mut sim, GrenadeKind::Blast, HEGRENADE, at);
    sim.ticks(2);
    let he = drawn(&sim, 0);
    assert_eq!(he.len(), 1, "one explosion sprite");
    assert!(he[0].distance(at) < 0.2, "at the blast: {}", he[0]);
    let p = &sim.app.world().resource::<Particles>().groups.last().unwrap().particles[0];
    assert!(p.fps > 0.0 && !p.frame_loop, "played once through its frames");
}

#[test]
fn a_smoke_puffs_every_second() {
    let mut sim = sim();
    let at = greybox::SPAWNS[0];
    go_off(&mut sim, GrenadeKind::Smoke, SMOKEGRENADE, at);
    sim.ticks(2);
    let first = drawn(&sim, 1).len();
    assert!(first > 0, "a puff at once");
    sim.seconds(2.05);
    let mut q = sim.app.world_mut().query::<&looks::SmokePuffs>();
    let puffs = q.iter(sim.app.world()).next().copied();
    assert_eq!(puffs.map(|p| p.left), Some(18), "three emissions in two seconds");
    assert!(drawn(&sim, 1).iter().all(|p| p.distance(at) < 5.0), "within the cloud");
}
