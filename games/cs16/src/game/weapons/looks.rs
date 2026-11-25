//! How CS 1.6's grenades look when they go off, drawn from the game's own
//! sprites as particles (`goldsrc_engine::content::sprites::
//! particle_material`, loaded with the kit by `weapons::add_guns`):
//!
//! - **HE** (`Detonated`, a blast of this game): `sprites/zerogxplode.spr`
//!   (Half-Life's, through the `valve` folder) played once from its first
//!   frame to its last at the blast point, additive;
//! - **smoke** (`Detonated`, a smoke of this game): a puff of
//!   `sprites/gas_puff_01.spr` sprites every second, 21 times, as
//!   weapons.md "Smoke grenade" says the client emits it (`SmokePuffs`),
//!   spread within the cloud's radius (`grenades::smoke`), each fading in
//!   and out.
//!
//! Known shortcuts:
//! - No spec names these sprites or gives their size, rate and life
//!   (weapons.md open question 5; the HE's look is the client's): the
//!   sprites are the install's by name, the numbers below ours.
//! - The flashbang draws nothing of its own (its white screen is the
//!   blinding's).

use bevy::prelude::*;
use chimera_sim::weapon::grenade::{Detonated, GrenadeKind};

use crate::map::{
    ParticleRng,
    particles::{Fade, Motion, Particle, ParticleGroup, ParticleMaterials, Particles, Ramp},
};

/// The HE explosion's sprite.
pub const HE_SPRITE: &str = "sprites/zerogxplode.spr";
/// The smoke cloud's puff sprite.
pub const SMOKE_SPRITE: &str = "sprites/gas_puff_01.spr";

const UNIT: f32 = 0.0254;
/// The explosion's half-width (units; ours) and frame rate (ours).
const HE_HALF: f32 = 80.0;
const HE_FPS: f32 = 30.0;
/// The explosion sprite's frames (`zerogxplode.spr` has 15).
const HE_FRAMES: f32 = 15.0;
/// Puffs per emission, emissions, their spacing (weapons.md), each puff's
/// life and half-width (units; ours).
const PUFFS: usize = 6;
const EMISSIONS: u32 = 21;
const EMIT_EVERY: f32 = 1.0;
const PUFF_LIFE: f32 = 5.0;
const PUFF_HALF: f32 = 70.0;

/// The sprites a grenade's look draws (`add_guns` loads them).
pub fn sprites_of(id: &str) -> &'static [&'static str] {
    match id {
        super::HEGRENADE => &[HE_SPRITE],
        super::SMOKEGRENADE => &[SMOKE_SPRITE],
        _ => &[],
    }
}

/// A smoke cloud's emitter: where, the next emission's time, how many
/// are left.
#[derive(Component, Clone, Copy, Debug)]
pub struct SmokePuffs {
    pub at: Vec3,
    pub next: f32,
    pub left: u32,
}

fn ours(weapon: &str) -> bool {
    weapon.starts_with(super::super::GAME) && weapon.contains(':')
}

/// Each blast of this game's grenades draws its explosion, each smoke
/// starts its puffs (module doc).
pub fn detonations(
    mut detonated: MessageReader<Detonated>,
    materials: Option<Res<ParticleMaterials>>,
    particles: Option<ResMut<Particles>>,
    mut commands: Commands,
) {
    let (Some(materials), Some(mut particles)) = (materials, particles) else {
        detonated.clear();
        return;
    };
    for d in detonated.read().filter(|d| ours(d.weapon)) {
        match d.kind {
            GrenadeKind::Blast => {
                let Some(m) = materials.0.find(HE_SPRITE) else { continue };
                let mut p = Particle::new(d.at, HE_FRAMES / HE_FPS, m, HE_HALF * UNIT);
                p.fps = HE_FPS;
                let mut g = ParticleGroup::new(Motion::default());
                g.particles.push(p);
                particles.add(g);
            }
            GrenadeKind::Smoke => {
                commands.spawn((Name::new("cs16 smoke puffs"), SmokePuffs { at: d.at, next: 0.0, left: EMISSIONS }));
            }
            GrenadeKind::Flash => {}
        }
    }
}

/// The smoke clouds' puffs, one emission a second (module doc).
pub fn puffs(
    mut emitters: Query<(Entity, &mut SmokePuffs)>,
    materials: Option<Res<ParticleMaterials>>,
    particles: Option<ResMut<Particles>>,
    time: Res<Time>,
    mut rng: Local<Option<ParticleRng>>,
    mut commands: Commands,
) {
    let (Some(materials), Some(mut particles)) = (materials, particles) else { return };
    let Some(m) = materials.0.find(SMOKE_SPRITE) else { return };
    let rng = rng.get_or_insert_with(|| ParticleRng::new(0x5310CE));
    let radius = super::grenades::smoke().max_radius;
    let dt = time.delta_secs();
    for (e, mut s) in &mut emitters {
        s.next -= dt;
        if s.next > 0.0 {
            continue;
        }
        let mut g = ParticleGroup::new(Motion::default());
        g.capped = false;
        for _ in 0..PUFFS {
            let dir = Vec3::new(rng.float(-1.0, 1.0), rng.float(0.0, 0.6), rng.float(-1.0, 1.0));
            let at = s.at + dir * radius * 0.6 + Vec3::Y * PUFF_HALF * 0.5 * UNIT;
            let mut p = Particle::new(at, PUFF_LIFE, m, PUFF_HALF * UNIT);
            p.velocity = dir.normalize_or_zero() * 4.0 * UNIT;
            p.color = Vec3::splat(rng.float(0.75, 0.95));
            p.fade = Fade::Sine;
            p.size = Ramp::linear(PUFF_HALF * 0.8 * UNIT, PUFF_HALF * 1.2 * UNIT);
            p.roll = rng.float(0.0, std::f32::consts::TAU);
            p.roll_speed = rng.float(-0.2, 0.2);
            g.particles.push(p);
        }
        particles.add(g);
        s.left -= 1;
        s.next += EMIT_EVERY;
        if s.left == 0 {
            commands.entity(e).despawn();
        }
    }
}

/// The grenades' looks drawn.
pub struct Cs16GrenadeLooksPlugin;

impl Plugin for Cs16GrenadeLooksPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Detonated>().add_systems(Update, (detonations, puffs).chain());
    }
}
