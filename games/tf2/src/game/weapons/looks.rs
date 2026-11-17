//! What TF2's guns look like going off (the brief's "basic visuals"),
//! the same on every map, TF2's or another game's (its particle files and
//! impact looks come with its own sets: `maps::add_guest_looks` loads
//! them onto another game's map, `maps::extra_particles` with its own):
//!
//! - **Explosions** (the rocket's, the grenade's, the sticky's:
//!   `blast::Tf2Explosion`, sent by the server to every client:
//!   `register`): TF2's own `ExplosionCore_MidAir` from
//!   `particles/explosion.pcf` (`StartParticleSystem`), never the map's
//!   game's explosion (de_dust2 drew Source's grey puff instead).
//! - **Muzzle flashes**: each shot of a bullet gun or launcher (`Fired`)
//!   starts its TF2 muzzle system (`muzzle_pistol`, `muzzle_minigun`, ...:
//!   `MUZZLE`) at the gun's `muzzle` attachment (install.md 4: on the `c_`
//!   weapon's bones): the first-person view model's, re-projected into
//!   the world view (`map::attached::FirstPerson::world_point`), else the
//!   held model's (`HeldAttachment`), else a little ahead of the eye under
//!   the gun (`MUZZLE_OFFSET`).
//! - **Flames**: each round of the Flame Thrower (one every 0.08 s while
//!   attack is held) starts a piece of TF2's `flamethrower` system
//!   (`flamethrower_blue` for team 2, BLU's) at the muzzle along the aim,
//!   emitting for one round's interval (`FLAME_PIECE`), so the stream
//!   follows the aim and ends when firing does.
//! - **Impacts**: Source's impact effects, decals and impact sounds with
//!   TF2's style (`STYLE`): TF2 is an impact provider, so its guns' hits
//!   are TF2's own on any map that carries its looks (`maps::
//!   add_impact_looks`), never the map's game's or the app's base game's
//!   (Combat Arms' in the party apps). Which entry a surface plays is the
//!   map's sound scripts' (a name the map has, such as CS:S's
//!   `Concrete.BulletImpact` on de_dust2, plays the map's).
//!
//!   A first-person flash (not the flames) follows the eye it was
//!   started from (`StartParticleSystem::follow`): it stays on the gun
//!   drawn in the view while the player moves and turns (playtest
//!   2026-11-17: the flashes hung in the world behind a strafing player).
//!
//! Known shortcuts: the flash is drawn in the world pass at the
//! re-projected muzzle (the view-model layer has no particle systems of
//! its own), so the view model covers its back half; the flame is pieces
//! of TF2's system per round, not its own continuous emitter; no airblast
//! look, no scorch decals; TF2's impact style (ricochets, pellet
//! grouping) has no spec yet: no ricochet, every pellet's sound, as
//! HL2:DM's (specs/tf2 names no impact rules).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use chimera_core::core::Team;
use chimera_sim::weapon::{Weapon, WeaponEvent, WeaponEventKind, aim_of};

use super::{blast::Tf2Explosion, damage::U};
use crate::map::{
    ShowLocalBody,
    attached::{AnchorQuery, AttachPoint, attachment_point, first_person_shown},
    particles::ParticleSet,
    psys::{Follow, StartParticleSystem},
    view_model::{HeldAttachment, ViewAnimator, ViewModelCamera, ViewModelOffset, ViewModelSettings, ViewModels},
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

/// Where a muzzle flash starts from the eye when the gun's `muzzle`
/// attachment isn't drawn: forward, right, up (units).
pub const MUZZLE_OFFSET: Vec3 = Vec3::new(28.0, 11.0, -11.0);

/// The Flame Thrower's flames by team: RED's, BLU's (flamethrower.pcf).
pub const FLAMES: [&str; 2] = ["flamethrower", "flamethrower_blue"];

/// Seconds each round's piece of the flame emits: the round's interval
/// (0.08 s) and a little, so the pieces join.
pub const FLAME_PIECE: f32 = 0.09;

/// The gun's attachment the flashes and flames start from.
pub const MUZZLE_ATTACHMENT: &str = "muzzle";

/// TF2's impacts: Source's shared impact effects and sounds. No spec says
/// TF2's own rules yet (module doc): no ricochet, splashes of 4 to 8,
/// every pellet's impact sound, damage in whole points.
pub const STYLE: source_engine::effects::ImpactStyle = source_engine::effects::ImpactStyle {
    game: crate::game::GAME,
    ricochet: None,
    splash: (4.0, 8.0),
    group_radius: None,
    damage_unit: 1.0,
    flesh: "flesh",
    blood: source_engine::effects::BloodView::Damage,
    melee_decal: None,
};

/// The system a shot of `gun` starts and for how long it emits (None: as
/// its definition runs): its muzzle flash, or for the Flame Thrower a
/// piece of its team's flames (`blu`: team 2's).
pub fn shot_system(gun: &str, blu: bool) -> Option<(&'static str, Option<f32>)> {
    if gun == super::FLAMETHROWER {
        return Some((FLAMES[blu as usize], Some(FLAME_PIECE)));
    }
    MUZZLE.iter().find(|(g, _)| *g == gun).map(|(_, n)| (*n, None))
}

/// The systems TF2's guns start: loaded with its maps besides their own,
/// and onto other games' maps (`maps::add_guest_looks`).
pub const PARTICLE_SYSTEMS: &[&str] = &[
    EXPLOSION,
    FLAMES[0],
    FLAMES[1],
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
pub const PARTICLE_FILES: &[&str] = &["particles/explosion.pcf", "particles/muzzle_flash.pcf", "particles/flamethrower.pcf"];

/// An explosion on the wire (the server's `Tf2Explosion`, to clients).
#[derive(Message, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ExplosionMsg {
    pub at: Vec3,
}

/// Each explosion's looks (every machine that draws): TF2's system.
fn explosions(mut blasts: MessageReader<Tf2Explosion>, mut start: MessageWriter<StartParticleSystem>) {
    for b in blasts.read() {
        start.write(StartParticleSystem::new(EXPLOSION, b.at, Vec3::Y));
    }
}

/// Where a muzzle is drawn for each owner this frame (`MUZZLE_ATTACHMENT`):
/// the first-person view model's re-projected into the world, else the
/// held model's.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Muzzles<'w, 's> {
    time: Res<'w, Time>,
    owners: Query<
        'w,
        's,
        (
            Option<&'static ViewAnimator>,
            Option<&'static ViewModelOffset>,
            Option<&'static chimera_core::core::MapControls>,
        ),
    >,
    anchors: AnchorQuery<'w, 's>,
    vm_cameras: Query<'w, 's, &'static Projection, With<ViewModelCamera>>,
    held: Query<'w, 's, (&'static HeldAttachment, &'static GlobalTransform)>,
    models: Option<Res<'w, ViewModels>>,
    settings: Option<Res<'w, ViewModelSettings>>,
    third_person: Option<Res<'w, ShowLocalBody>>,
}

impl Muzzles<'_, '_> {
    /// `owner`'s muzzle (engine space), if its gun's is drawn.
    pub fn of(&self, owner: Entity) -> Option<Vec3> {
        self.placed(owner).map(|(at, _)| at)
    }

    /// `owner`'s muzzle (engine space), and on a first-person view model
    /// the eye it is drawn from with the muzzle in the eye's frame (a
    /// flash started there follows the gun drawn in the view).
    pub fn placed(&self, owner: Entity) -> Option<(Vec3, Option<(Entity, Vec3, Quat)>)> {
        let at = AttachPoint::named(MUZZLE_ATTACHMENT);
        let (view, offset, controls) = self.owners.get(owner).ok()?;
        let fp = first_person_shown(
            owner,
            view,
            offset,
            controls,
            &self.anchors,
            &self.vm_cameras,
            self.models.as_deref(),
            self.settings.as_deref().copied().unwrap_or_default(),
            self.third_person.as_ref().is_some_and(|t| t.0),
            self.time.elapsed_secs_f64(),
        );
        match fp {
            Some(f) => {
                let p = f.camera_point(&at)?;
                let p = f.fovs.map_or(p, |(w, v)| crate::map::view_model::reproject(p, w, v));
                Some((f.eye.transform_point(p), Some((f.anchor, p, f.eye.rotation()))))
            }
            None => attachment_point(&self.held, owner, &at).map(|p| (p, None)),
        }
    }
}

/// Each shot's muzzle flash, and each Flame Thrower round's flames.
fn muzzle_flashes(
    mut events: MessageReader<WeaponEvent>,
    weapons: Query<&Weapon>,
    teams: Query<&Team>,
    muzzles: Muzzles,
    mut start: MessageWriter<StartParticleSystem>,
) {
    for e in events.read() {
        let WeaponEventKind::Fired { origin, yaw, pitch, .. } = e.kind else { continue };
        if !e.shown() {
            continue;
        }
        let Ok(weapon) = weapons.get(e.weapon).map(|w| w.id) else { continue };
        let blu = teams.get(e.owner).is_ok_and(|t| t.0 == 2);
        let Some((name, emit_for)) = shot_system(weapon, blu) else { continue };
        let aim = aim_of(yaw, pitch);
        let forward = aim * Vec3::NEG_Z;
        let (at, eye) = muzzles.placed(e.owner).unwrap_or_else(|| {
            let o = MUZZLE_OFFSET * U;
            (origin + forward * o.x + aim * Vec3::X * o.y + aim * Vec3::Y * o.z, None)
        });
        // On a first-person gun the flash goes with the gun drawn in the
        // view (the eye's frame), not left where it started; the flames
        // stream into the world.
        let follow = eye.filter(|_| emit_for.is_none()).map(|(entity, local, turn)| Follow { entity, local, forward: turn.inverse() * forward });
        start.write(StartParticleSystem { emit_for, follow, ..StartParticleSystem::new(name, at, forward) });
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
    use crate::map::impact::SurfaceClassesAppExt;
    app.add_message::<StartParticleSystem>()
        .add_plugins((
            source_engine::effects::impact_effects::ImpactEffectsPlugin(STYLE),
            source_engine::effects::impacts::ImpactSoundsPlugin(STYLE),
        ))
        .register_surface_classes(source_engine::effects::impacts::surfaces(crate::game::GAME))
        .add_systems(
            Update,
            (explosions, muzzle_flashes)
                .after(ParticleSet::Step)
                .before(ParticleSet::Draw)
                .after(crate::map::attached::EyePlaced)
                .after(crate::map::DriveAnimation),
        )
        .net_part("tf2_looks", register);
}
