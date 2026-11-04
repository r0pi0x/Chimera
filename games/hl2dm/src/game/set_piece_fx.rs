//! What the set pieces look like (specs/hl2dm/maps_entities.md 2, 8, 9),
//! drawn on every machine with Chimera's particles (`map::particles`):
//!
//! - **The headcrab canister** (`set_pieces`' shown models, on the host
//!   and on network clients alike): in flight its sprite trail
//!   (`sprites/smoke`, additive, colour (224, 224, 255), 32 ± 15 units wide
//!   growing to 200 over its 3 s life; in the 3D skybox before it enters
//!   the world, `SKY_CANISTER_MODEL`, drawn there at the sky's scale);
//!   landed and smoking, the smoke trail (8 puffs a second, life 2 s, grey
//!   0.7 to 0.6, size 32 to 64, from within 8 units, 0 to 8 units/s any
//!   way plus 32 to 64 up, alpha 0.35); its impact's explosion (`CanisterBlast`: the Source
//!   explosion of `source_engine::effects::explosions`, silent; none for
//!   flag 524288, so none on dm_runoff).
//! - **`env_flare`** (every frame while its part is on): unless flag 2,
//!   8 smoke puffs a second (`particle/particle_noisesphere`) 4 · scale
//!   up, rising at 40 to 48 units/s with ±16 sideways, life 1 s, colour
//!   (255, 48, 48), alpha 64 to 90 fading to 0, size 2 to 4 growing 8
//!   times; unless flag 1, a dynamic light of radius scale · U(110, 128)
//!   and colour (255, U{32..64}, same), renewed every frame.
//! - **The respawn light** (`RespawnLight`, from `PickupBack` on the host):
//!   sent to network clients too (`SetPieceFx`, net part
//!   `hl2dm_set_pieces`); `ui::lights` draws it.
//!
//! Known shortcuts: the canister's smoke rises from the model's centre (no
//! attachments on shown items: the `smoke` attachment isn't read), its
//! flight trail is a chain of fading sprites (not a beam) that stays where
//! it was left at the landing, and the dust cloud of its impact effects
//! isn't drawn; a flare's flame sprites don't jitter or shrink before a
//! burn-out (the stock flares burn forever), and its smoke doesn't move
//! 2 units toward the viewer.

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use chimera_sim::weapon::respawn::PickupBack;
use source_engine::effects::explosions::{self, FxMaterials};

use super::set_pieces::{CANISTER_MODEL, CanisterBlast, SKY_CANISTER_MODEL, sky_camera};
use crate::{
    core::NetRole,
    map::{
        EntityPart, MapEntities, ParticleRng,
        entities::entity_to_engine,
        loose::ShownItem,
        particles::{Fade, Motion, Particle, ParticleGroup, ParticleMaterials, ParticleSet, Particles, Ramp, WorldTracer},
    },
};

const U: f32 = 0.0254;

/// The materials these effects use (loaded with the map's; `sprites/smoke`
/// additive, `effects::ADDITIVE_SPRITES`).
pub const MATERIALS: &[&str] = &["sprites/smoke", "particle/particle_noisesphere", "particle/particle_smokegrenade"];

/// The canister's flight trail: colour, widths (units) and life (s).
pub const TRAIL_COLOR: Vec3 = Vec3::new(224.0 / 255.0, 224.0 / 255.0, 1.0);
pub const TRAIL_START_WIDTH: (f32, f32) = (17.0, 47.0);
pub const TRAIL_END_WIDTH: f32 = 200.0;
pub const TRAIL_LIFE: f32 = 3.0;
/// One trail sprite per this many units flown.
const TRAIL_STEP: f32 = 24.0;
/// The landed canister's smoke: puffs per second.
pub const SMOKE_RATE: f32 = 8.0;
/// A flare's smoke: puffs per second.
pub const FLARE_SMOKE_RATE: f32 = 8.0;

/// A respawned pickup's flash (maps_entities.md 2), on every machine.
#[derive(Message, Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RespawnLight {
    pub at: Vec3,
    /// Meters.
    pub radius: f32,
}

/// Server -> clients: the set pieces' one-off effects.
#[derive(Message, Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SetPieceFx {
    Blast(CanisterBlast),
    Light(RespawnLight),
}

/// Whether a respawned pickup's model has an attachment to flash at: the
/// AR2's energy ball box, the RPG round, the .357, AR2, shotgun, RPG and
/// S.L.A.M. world models.
pub fn has_attachment(item: Option<&super::pickups::Pickup>, weapon: Option<&str>) -> bool {
    use super::{
        pickups::Item,
        weapons::{AR2, MAGNUM, RPG, SHOTGUN},
    };
    if let Some(p) = item {
        return matches!(&p.item, Item::Ammo { ammo, .. } if ammo.ends_with("ar2altfire") || ammo.ends_with("rpg_round"));
    }
    weapon.is_some_and(|w| [MAGNUM, AR2, SHOTGUN, RPG, "hl2dm:weapon_slam"].contains(&w))
}

/// Pickups that came back with an attachment flash: radius 32 to 64
/// units, a little above the pickup's place (its model isn't drawn).
fn respawn_lights(
    mut back: MessageReader<PickupBack>,
    things: Query<(Option<&super::pickups::Pickup>, Option<&chimera_sim::weapon::Weapon>)>,
    mut rng: ResMut<FxRng>,
    mut out: MessageWriter<RespawnLight>,
) {
    for b in back.read() {
        if b.relocated {
            continue;
        }
        let Ok((item, weapon)) = things.get(b.entity) else { continue };
        if has_attachment(item, weapon.map(|w| w.id)) {
            out.write(RespawnLight { at: b.at + Vec3::Y * 0.2, radius: rng.0.float(32.0, 64.0) * U });
        }
    }
}

#[derive(Resource)]
struct FxRng(ParticleRng);

impl Default for FxRng {
    fn default() -> Self {
        Self(ParticleRng::new(0x5E7_91EC))
    }
}

/// Where each shown canister was last frame, and the parts of a sprite or
/// puff it owes.
#[derive(Default)]
struct Tracks(std::collections::BTreeMap<Entity, (Vec3, f32)>);

/// The canisters' trails and smoke this frame.
fn canisters(
    items: Query<(Entity, &ShownItem, &GlobalTransform)>,
    materials: Option<Res<ParticleMaterials>>,
    time: Res<Time>,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<FxRng>,
    mut tracks: Local<Tracks>,
    entities: Option<Res<MapEntities>>,
) {
    let Some(materials) = materials else { return };
    let sky_scale = sky_camera(entities.as_deref()).map_or(1.0, |(_, s)| s);
    let dt = time.delta_secs();
    let (trail_mat, smoke_mat) = (materials.0.find("sprites/smoke"), materials.0.find("particle/particle_smokegrenade"));
    let mut trail = ParticleGroup::new(Motion::default());
    let mut smoke = ParticleGroup::new(Motion::default());
    let mut seen = Vec::new();
    for (e, shown, at) in &items {
        let key = shown.0.as_str();
        let in_sky = key == SKY_CANISTER_MODEL;
        let flying = key == CANISTER_MODEL || in_sky;
        // In the 3D skybox its trail is drawn at the sky's scale (its
        // image as wide as in the world).
        let k = if in_sky { sky_scale } else { 1.0 };
        let smoking = key.starts_with("hl2dm:headcrabcanister_open");
        if !flying && !smoking {
            continue;
        }
        seen.push(e);
        let at = at.translation();
        let (last, owed) = tracks.0.entry(e).or_insert((at, 0.0));
        if flying && let Some(mat) = trail_mat {
            *owed += last.distance(at) * k / U / TRAIL_STEP;
            let n = (*owed as usize).min(64);
            *owed -= n as f32;
            for j in 0..n {
                let p = last.lerp(at, (j + 1) as f32 / n as f32);
                let start = rng.0.float(TRAIL_START_WIDTH.0, TRAIL_START_WIDTH.1);
                trail.particles.push(Particle {
                    color: TRAIL_COLOR,
                    fade: Fade::Ramp(Ramp::linear(1.0, 0.0)),
                    size: Ramp::linear(start / 2.0 * U / k, TRAIL_END_WIDTH / 2.0 * U / k),
                    ..Particle::new(p, TRAIL_LIFE, mat, 0.0)
                });
            }
        }
        if smoking && let Some(mat) = smoke_mat {
            *owed += SMOKE_RATE * dt;
            let n = (*owed as usize).min(8);
            *owed -= n as f32;
            for _ in 0..n {
                let offset = rng.0.vec3(-1.0, 1.0).normalize_or_zero() * rng.0.float(0.0, 8.0) * U;
                let drift = rng.0.vec3(-1.0, 1.0).normalize_or_zero() * rng.0.float(0.0, 8.0);
                let up = Vec3::Y * rng.0.float(32.0, 64.0);
                smoke.particles.push(Particle {
                    velocity: (drift + up) * U,
                    color: Vec3::splat(0.7),
                    color_fade: Some((Vec3::splat(0.6), 0.0, 1.0)),
                    alpha: 0.35,
                    fade: Fade::Ramp(Ramp::linear(1.0, 0.0)),
                    size: Ramp::linear(16.0 * U, 32.0 * U),
                    roll: rng.0.float(0.0, std::f32::consts::TAU),
                    ..Particle::new(at + offset, 2.0, mat, 0.0)
                });
            }
        }
        *last = at;
    }
    tracks.0.retain(|e, _| seen.contains(e));
    for g in [trail, smoke] {
        if !g.particles.is_empty() {
            particles.add(g);
        }
    }
}

/// A canister's impact effects: the Source explosion, silent.
fn blasts(
    mut blasts: MessageReader<CanisterBlast>,
    materials: Option<Res<ParticleMaterials>>,
    field: Option<Res<crate::map::LightField>>,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<FxRng>,
    world: WorldTracer,
) {
    let Some(materials) = materials else {
        blasts.clear();
        return;
    };
    let mats = FxMaterials::new(&materials.0);
    for b in blasts.read() {
        let a = explosions::blast_direction(b.at, &world);
        let l = explosions::light_at(field.as_deref(), b.at + a * 32.0 * U);
        for g in explosions::explosion(&mut rng.0, &mats, b.at, a, l, &world) {
            particles.add(g);
        }
    }
}

/// An env_flare's light (flag 1 off), one per flare entity index.
#[derive(Component)]
struct FlareLight(usize);

const SF_FLARE_NO_LIGHT: u32 = 1;
const SF_FLARE_NO_SMOKE: u32 = 2;
const SF_FLARE_START_OFF: u32 = 8;

/// The map's flares: their smoke and light while their part is on.
#[allow(clippy::too_many_arguments)]
fn flares(
    entities: Option<Res<MapEntities>>,
    parts: Query<&EntityPart>,
    mut lights: Query<(Entity, &FlareLight, &mut PointLight, &mut Visibility)>,
    materials: Option<Res<ParticleMaterials>>,
    time: Res<Time>,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<FxRng>,
    mut owed: Local<std::collections::BTreeMap<usize, f32>>,
    mut commands: Commands,
) {
    let Some(entities) = entities else { return };
    let dt = time.delta_secs();
    let smoke_mat = materials.as_ref().and_then(|m| m.0.find("particle/particle_noisesphere"));
    let mut smoke = ParticleGroup::new(Motion::default());
    let mut lit = Vec::new();
    for (i, e) in entities.entities.iter().enumerate() {
        if !e.classname().eq_ignore_ascii_case("env_flare") {
            continue;
        }
        let flags: u32 = e.get("spawnflags").and_then(|v| v.trim().parse().ok()).unwrap_or(0);
        let on = match parts.iter().find(|p| p.entity == i) {
            Some(p) => p.on && p.exists,
            None => flags & SF_FLARE_START_OFF == 0,
        };
        if !on {
            continue;
        }
        let b = e.get("scale").and_then(|v| v.trim().parse::<f32>().ok()).filter(|s| *s > 0.0).unwrap_or(1.0);
        let at = entity_to_engine(e.origin(), entities.scale);
        if flags & SF_FLARE_NO_SMOKE == 0
            && let Some(mat) = smoke_mat
        {
            let o = owed.entry(i).or_insert(0.0);
            *o += FLARE_SMOKE_RATE * dt;
            let n = (*o as usize).min(8);
            *o -= n as f32;
            for _ in 0..n {
                // Entity space: x, y sideways, z up.
                let v = Vec3::new(rng.0.float(-16.0, 16.0), rng.0.float(-16.0, 16.0), rng.0.float(40.0, 48.0));
                let size = rng.0.float(2.0, 4.0);
                smoke.particles.push(Particle {
                    velocity: entity_to_engine(v, U),
                    color: Vec3::new(1.0, 48.0 / 255.0, 48.0 / 255.0),
                    alpha: rng.0.float(64.0, 90.0) / 255.0,
                    fade: Fade::Ramp(Ramp::linear(1.0, 0.0)),
                    size: Ramp::linear(size * U, size * 8.0 * U),
                    ..Particle::new(at + Vec3::Y * 4.0 * b * U, 1.0, mat, 0.0)
                });
            }
        }
        if flags & SF_FLARE_NO_LIGHT == 0 {
            lit.push(i);
            let radius = b * rng.0.float(110.0, 128.0) * U;
            let g = rng.0.float(32.0, 64.0).floor() / 255.0;
            let colour = Color::srgb(1.0, g, g);
            match lights.iter_mut().find(|(_, f, _, _)| f.0 == i) {
                Some((_, _, mut l, mut vis)) => {
                    l.range = radius;
                    l.color = colour;
                    *vis = Visibility::Inherited;
                }
                None => {
                    commands.spawn((
                        Name::new("env_flare light"),
                        FlareLight(i),
                        crate::map::MapPart,
                        PointLight {
                            color: colour,
                            intensity: 40_000.0 * radius,
                            range: radius,
                            radius: 0.0,
                            shadow_maps_enabled: false,
                            ..default()
                        },
                        Transform::from_translation(at),
                    ));
                }
            }
        }
    }
    for (e, f, _, mut vis) in &mut lights {
        if !lit.contains(&f.0) {
            if f.0 >= entities.entities.len() {
                commands.entity(e).despawn();
            } else {
                *vis = Visibility::Hidden;
            }
        }
    }
    if !smoke.particles.is_empty() {
        particles.add(smoke);
    }
}

/// The server: this frame's blasts and respawn lights to every client.
fn forward(
    mut blasts: MessageReader<CanisterBlast>,
    mut lights: MessageReader<RespawnLight>,
    mut out: MessageWriter<ToClients<SetPieceFx>>,
) {
    let fx = blasts.read().map(|b| SetPieceFx::Blast(*b)).chain(lights.read().map(|l| SetPieceFx::Light(*l)));
    for message in fx {
        out.write(ToClients { targets: SendTargets::CLIENTS_ONLY, message });
    }
}

/// A client: the server's effects as its own messages.
fn receive(mut fx: MessageReader<SetPieceFx>, mut blasts: MessageWriter<CanisterBlast>, mut lights: MessageWriter<RespawnLight>) {
    for f in fx.read() {
        match f {
            SetPieceFx::Blast(b) => {
                blasts.write(*b);
            }
            SetPieceFx::Light(l) => {
                lights.write(*l);
            }
        }
    }
}

/// The protocol: the set pieces' effects, from the server.
fn register(app: &mut App) {
    app.add_server_message::<SetPieceFx>(Channel::Unreliable)
        .add_systems(
            PostUpdate,
            forward.before(ServerSystems::Send).run_if(resource_equals(NetRole::Server)),
        )
        .add_systems(
            PreUpdate,
            receive.after(ClientSystems::Receive).run_if(resource_equals(NetRole::Client)),
        );
}

pub struct Hl2SetPieceFxPlugin;

impl Plugin for Hl2SetPieceFxPlugin {
    fn build(&self, app: &mut App) {
        use chimera_net::net::parts::NetAppExt;
        app.init_resource::<FxRng>()
            .add_message::<CanisterBlast>()
            .add_message::<RespawnLight>()
            .add_message::<PickupBack>()
            .add_systems(FixedUpdate, respawn_lights.run_if(crate::core::authoritative))
            .add_systems(
                Update,
                (canisters, blasts, flares).after(ParticleSet::Step).before(ParticleSet::Draw),
            )
            .net_part("hl2dm_set_pieces", register);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_trail_is_the_specs() {
        // 32 ± 15 units wide at the start, 200 at the end, 3 s.
        assert_eq!(TRAIL_START_WIDTH, (17.0, 47.0));
        assert_eq!(TRAIL_END_WIDTH, 200.0);
        assert_eq!(TRAIL_LIFE, 3.0);
    }

    #[test]
    fn respawn_lights_flash_for_models_with_an_attachment() {
        use crate::game::weapons::{CROWBAR, RPG};
        assert!(has_attachment(None, Some(RPG)));
        assert!(!has_attachment(None, Some(CROWBAR)));
    }
}
