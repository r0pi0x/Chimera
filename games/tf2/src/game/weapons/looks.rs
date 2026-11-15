//! What TF2's guns look like going off (the brief's "basic visuals"):
//!
//! - **Explosions** (the rocket's, the grenade's, the sticky's:
//!   `blast::Tf2Explosion`, sent by the server to every client:
//!   `register`): TF2's own `ExplosionCore_MidAir` from
//!   `particles/explosion.pcf` (`StartParticleSystem`; loaded with TF2's
//!   maps: `PARTICLE_SYSTEMS`, `PARTICLE_FILES`) and, where the map's
//!   particle materials have Source's explosion sprites (CS:S's and
//!   HL2:DM's maps), Source's explosion (`source_engine::effects::
//!   explosions`), as HL2:DM draws its own.
//! - **Muzzle flashes**: each shot of a bullet gun (`Fired`) starts its
//!   TF2 muzzle system (`muzzle_pistol`, `muzzle_minigun`, ...:
//!   `MUZZLE`) a little ahead of the eye, under the gun (`MUZZLE_OFFSET`).
//!
//! Known shortcuts: the muzzle flash is drawn in the world, not on the
//! view model's `muzzle` attachment (the view-model layer has no particle
//! systems of its own); on another game's map TF2's `.pcf` systems aren't
//! loaded, so its muzzle flashes don't show there (its explosions draw as
//! Source's); no flames from the Flame Thrower, no scorch decals.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use chimera_sim::weapon::{Weapon, WeaponEvent, WeaponEventKind, aim_of};
use source_engine::effects::explosions::{self, FxMaterials};

use super::{blast::Tf2Explosion, damage::U};
use crate::map::{
    ParticleRng,
    particles::{ParticleMaterials, ParticleSet, Particles, WorldTracer},
    psys::StartParticleSystem,
};

/// TF2's explosion system.
pub const EXPLOSION: &str = "ExplosionCore_MidAir";

/// Each bullet gun's muzzle system (muzzle_flash.pcf).
pub const MUZZLE: &[(&str, &str)] = &[
    (super::SCATTERGUN, "muzzle_scattergun"),
    (super::SHOTGUN, "muzzle_shotgun"),
    (super::MINIGUN, "muzzle_minigun"),
    (super::PISTOL_SCOUT, "muzzle_pistol"),
    (super::SMG, "muzzle_smg"),
    (super::SNIPER_RIFLE, "muzzle_sniperrifle"),
    (super::REVOLVER, "muzzle_revolver"),
    (super::GRENADE_LAUNCHER, "muzzle_grenadelauncher"),
    (super::STICKY_LAUNCHER, "muzzle_pipelauncher"),
    (super::SYRINGE_GUN, "muzzle_syringe"),
];

/// Where a muzzle flash starts from the eye: forward, right, up (units).
pub const MUZZLE_OFFSET: Vec3 = Vec3::new(28.0, 11.0, -11.0);

/// The systems TF2's maps load besides their own.
pub const PARTICLE_SYSTEMS: &[&str] = &[
    EXPLOSION,
    "muzzle_scattergun",
    "muzzle_shotgun",
    "muzzle_minigun",
    "muzzle_pistol",
    "muzzle_smg",
    "muzzle_sniperrifle",
    "muzzle_revolver",
    "muzzle_grenadelauncher",
    "muzzle_pipelauncher",
    "muzzle_syringe",
];

/// The files they are in.
pub const PARTICLE_FILES: &[&str] = &["particles/explosion.pcf", "particles/muzzle_flash.pcf"];

/// An explosion on the wire (the server's `Tf2Explosion`, to clients).
#[derive(Message, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ExplosionMsg {
    pub at: Vec3,
}

#[derive(Resource)]
struct FxRng(ParticleRng);

impl Default for FxRng {
    fn default() -> Self {
        Self(ParticleRng::new(0x7F2_B1A5))
    }
}

/// Each explosion's looks (every machine that draws).
fn explosions(
    mut blasts: MessageReader<Tf2Explosion>,
    materials: Option<Res<ParticleMaterials>>,
    field: Option<Res<crate::map::LightField>>,
    particles: Option<ResMut<Particles>>,
    mut rng: ResMut<FxRng>,
    mut start: MessageWriter<StartParticleSystem>,
    world: WorldTracer,
) {
    let mats = materials.as_ref().map(|m| FxMaterials::new(&m.0));
    let mut particles = particles;
    for b in blasts.read() {
        start.write(StartParticleSystem { name: EXPLOSION.into(), position: b.at, forward: Vec3::Y });
        // Source's explosion where its sprites are loaded.
        if let Some(m) = &mats
            && m.fire.is_some()
            && let Some(particles) = particles.as_mut()
        {
            let a = explosions::blast_direction(b.at, &world);
            let l = explosions::light_at(field.as_deref(), b.at + a * 32.0 * U);
            for g in explosions::explosion(&mut rng.0, m, b.at, a, l, &world, Default::default()) {
                particles.add(g);
            }
        }
    }
}

/// Each shot's muzzle flash.
fn muzzle_flashes(mut events: MessageReader<WeaponEvent>, weapons: Query<&Weapon>, mut start: MessageWriter<StartParticleSystem>) {
    for e in events.read() {
        let WeaponEventKind::Fired { origin, yaw, pitch, .. } = e.kind else { continue };
        if !e.shown() {
            continue;
        }
        let Ok(weapon) = weapons.get(e.weapon).map(|w| w.id) else { continue };
        let Some((_, name)) = MUZZLE.iter().find(|(g, _)| *g == weapon) else { continue };
        let aim = aim_of(yaw, pitch);
        let o = MUZZLE_OFFSET * U;
        let forward = aim * Vec3::NEG_Z;
        let at = origin + forward * o.x + aim * Vec3::X * o.y + aim * Vec3::Y * o.z;
        start.write(StartParticleSystem { name: (*name).into(), position: at, forward });
    }
}

/// The server: this frame's explosions to every client.
fn forward(mut blasts: MessageReader<Tf2Explosion>, mut out: MessageWriter<bevy_replicon::prelude::ToClients<ExplosionMsg>>) {
    use bevy_replicon::prelude::*;
    for b in blasts.read() {
        out.write(ToClients { targets: SendTargets::CLIENTS_ONLY, message: ExplosionMsg { at: b.at } });
    }
}

/// A client: the server's explosions as its own.
fn receive(mut fx: MessageReader<ExplosionMsg>, mut out: MessageWriter<Tf2Explosion>) {
    for f in fx.read() {
        out.write(Tf2Explosion { at: f.at });
    }
}

/// The protocol: explosions, from the server.
fn register(app: &mut App) {
    use bevy_replicon::prelude::*;
    use crate::core::NetRole;
    app.add_server_message::<ExplosionMsg>(Channel::Unreliable)
        .add_systems(PostUpdate, forward.before(ServerSystems::Send).run_if(resource_equals(NetRole::Server)))
        .add_systems(PreUpdate, receive.after(ClientSystems::Receive).run_if(resource_equals(NetRole::Client)));
}

/// The looks, on any machine that draws (systems that only write
/// messages the presentation reads, harmless without it).
pub fn add(app: &mut App) {
    use chimera_net::net::parts::NetAppExt;
    app.init_resource::<FxRng>()
        .add_message::<StartParticleSystem>()
        .add_systems(Update, (explosions, muzzle_flashes).after(ParticleSet::Step).before(ParticleSet::Draw))
        .net_part("tf2_looks", register);
}
