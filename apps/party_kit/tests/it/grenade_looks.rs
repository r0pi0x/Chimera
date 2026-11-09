//! CS:S's grenades' looks in the party apps (`PartyPartsPlugin` adds
//! `cs_source::game::grenades::GrenadeLooksPlugin`): an HE thrown in the
//! gun game explodes with CS:S's particles (spec grenades.md 5.4); before,
//! only CS:S's own game drew them, so a party HE blew out a window with no
//! fireball at all. Another game's grenade is left to that game.

use bevy::prelude::*;
use chimera_sim::weapon::grenade::{Detonated, GrenadeKind};
use chimera_world::{
    greybox::GreyboxMapPlugin,
    harness::Sim,
    map::particles::{MapParticles, ParticleMaterial, ParticleMaterials, Particles},
};
use combat_arms::harness::CaSim;
use cs_source::game::grenades::{HEGRENADE, MATERIALS};
use party_kit::PartyPartsPlugin;

fn party() -> Sim {
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin));
    // Stand-in materials under the explosion's names (no install needed).
    let materials = MATERIALS
        .iter()
        .chain(&["particle/particle_noisesphere", "effects/fleck_cement1", "effects/fleck_cement2"])
        .map(|n| ParticleMaterial {
            name: n.to_string(),
            ..default()
        })
        .collect();
    sim.app.insert_resource(ParticleMaterials(MapParticles { materials }));
    sim
}

/// Live particles after a blast of `weapon` at the greybox's middle.
fn particles_after_blast(weapon: &'static str) -> usize {
    let mut sim = party();
    sim.ticks(2);
    sim.app.world_mut().resource_mut::<Particles>().groups.clear();
    sim.app.world_mut().write_message(Detonated {
        kind: GrenadeKind::Blast,
        weapon,
        thrower: None,
        at: Vec3::new(0.0, 1.0, 0.0),
        ground: None,
        projectile: Entity::PLACEHOLDER,
        shake: None,
        flags: Default::default(),
    });
    sim.ticks(2);
    sim.app.world().resource::<Particles>().count()
}

#[test]
fn a_cs_source_he_explodes_with_its_particles_in_the_party() {
    // Smoke puffs, embers, fireballs, sparks, the flash, cement flecks.
    let n = particles_after_blast(HEGRENADE);
    assert!(n > 60, "the HE's explosion: {n} particles");
    // A map's env_explosion (no game) too.
    assert!(particles_after_blast(chimera_sim::weapon::grenade::EXPLOSION_WEAPON) > 60);
}

#[test]
fn another_games_grenade_is_left_to_that_game() {
    assert_eq!(particles_after_blast("combat_arms:m67"), 0);
}
