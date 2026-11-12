//! HL2:DM's projectile effects (specs/hl2dm/effects_sounds.md 8, 9, the
//! Explosions and Projectile visuals tables; specs/hl2dm/projectiles.md
//! 5.1, 6.1, 7.4, 7.5, 8.2, 9.5), drawn with Chimera's particles (`map::particles`):
//!
//! - **Explosions** (`explosions`, on `Detonated`): the temp entity's
//!   look (`source_engine::effects::explosions`: CS:S's smoke, embers,
//!   fireballs, sparks and flecks, the same effect in every Source game)
//!   and its sound `BaseExplosionEffect.Sound`, heard by each client where
//!   it is. The RPG's, satchel's and trip mine's go by the explosion
//!   entity's rule: a line from 8 above the point to 40 below that, drawn
//!   24 off what it hits (and where it hits nothing, at the point itself),
//!   and the server puts a `Scorch` where it hits (`scorches`; Chimera's
//!   decals reach the clients). Map logic's blasts (explosive props,
//!   `env_explosion`s) go by the same rule in HL2:DM's own app
//!   (`DrawsMapBlasts`).
//! - **The shot satchel's pop** (`SatchelPop`, the server's, sent to
//!   clients too: net part `hl2dm_projectile_fx`): a scale-0 explosion,
//!   the flash and the debris (`explosions::pop`) and its sound.
//! - **Crush and drowning flashes** (effects_sounds.md 7 "Blood",
//!   `damage_flashes`): the victim's screen flashes red (128, 0, 0) for
//!   crush damage, a prop's hit too, blue (0, 0, 128) for drowning;
//!   alpha 128, fade 1.0 s, hold 0.1 s.
//! - **The rocket** (`MISSILE_MODEL`, every frame): 8 flare sprites behind
//!   it and a smoke puff for each 4 units it travelled.
//! - **The energy ball** (`BALL_MODEL`, every frame): the animated body
//!   quad (none while it is held in a gravity gun: `Hl2BallHeld`, the
//!   server's `HeldBall` replicated), the flickering halo and 8 blur halos
//!   behind; its bounce (`cball_bounce`: two flat quads and sparks) and its
//!   end (sparks and two rings of `sprites/lgtning` spreading to 1024
//!   units).
//! - **The crossbow bolt** (`BOLT_MODEL`): 20 blur halos along its motion;
//!   stuck in the world, a glow fading over 3 s and sparks.
//! - **Bolts pinning ragdolls** (projectiles.md 7.5, `pin_ragdolls`): at a
//!   "BoltImpact" (a bolt stuck in the world, or the world met by a line
//!   128 units on from a body it hit) each machine pins the first ragdoll
//!   bone within 128 units back along the flight to the world at the
//!   impact (avian's ball-and-socket joint to a static anchor, the bone's
//!   mass doubled, `Weapon_Crossbow.BoltSkewer`), waiting up to 0.25 s for
//!   the victim's ragdoll; and shows a bolt 8 units back from it for 30 s.
//! - **The trip mine's beam** (`Hl2MineBeam`, the server's arming length
//!   replicated; `mine_beams`, every frame): from the mine along its normal
//!   for that length, red (255, 55, 52), brightness 64, 0.35 units wide,
//!   additive `effects/laser1`.
//! - **The SMG1 grenade** (`SMG1_GRENADE_MODEL`): `env_smoketrail`'s smoke
//!   (`smoke_trail`; V the grenade's velocity as the server last sent it,
//!   `Hl2FlyerVelocity`);
//!   going off, the server's scorch where a 60-unit line along its flight
//!   hits (`scorches`: `Scorch` on the world, `SmallScorch` on props and
//!   brush entities).
//! - **The frag and the satchel**: a red glow (the frag's beats faster
//!   once warned) and the frag's red trail.
//! - **The AR2 shooter's white fade** as the ball leaves (projectiles.md
//!   5.1: 255, 255, 255, alpha 64 over 0.1 s), on his machine.
//! - The RPG's laser (its dot, beam and glow) and its missile's ignition
//!   are `rpg_fx`'s.
//!
//! Every per-frame look is a one-frame group (`frame_group`) added after
//! the particles' step and before their drawing, from what the shown
//! projectile entities (`ShownItem`, on the host and on clients alike)
//! are doing. The one-off flyer effects (`FlyerFx`: the ball's bounce and
//! end, a bolt stuck, a bolt's impact) are made where flyers are
//! simulated, from `FlyerHit`/`FlyerGone`, and drawn on every machine: a
//! network server sends them to its clients with the satchels' pops (net
//! part `hl2dm_projectile_fx`, which also replicates `Hl2BallHeld` and
//! `Hl2MineBeam`).
//!
//! A blast's flags (`core::BlastFlags` on `Detonated`) leave parts out:
//! `env_explosion`'s spawnflags (dm_steamlab's 5 say no decal) and an
//! `explode_fire` prop's silence (the propane tank and the gas can, their
//! burst and no boom; prop_damage.md 7.5). The temp entity's scale is not
//! drawn, as the spec's client effect doesn't draw it (grenades.md 5.4: only
//! a scale of 0, the satchel's pop or "no fireball", shows).
//!
//! Known shortcuts (docs/tech-debt.md): a held ball's model isn't turned
//! to face the camera, and in flight its model is drawn too (every
//! `ShownItem` draws its meshes); the ball's bounce goes to every client
//! (the spec's is the PAS); the ball's body quad takes its frames from the
//! material's sheet at 30 a second; the electric sparks are the surface
//! effect's (sizes 1, 2 and 4 of the spec are one size), and a stuck
//! bolt's are drawn under water too; the frag's trail has one width; the
//! entity rule's scorch lands on the world only (not on props or brush
//! entities under the blast; the SMG1 grenade's does); the satchel's glow
//! strobes at a steady rate (the spec says "strobing" and gives no rate or
//! form); the trip mine's beam material is our choice (the spec says "a
//! laser sprite") and it is drawn at its arming length, not re-traced each
//! frame; a pin has no sky test (the 16-unit line), the pinned bone keeps
//! its distance from the impact (it isn't moved onto it) and the shown
//! bolt overlaps the stuck flyer's own model for its 2 s. Why it's OK for
//! now: the shapes, sizes, lifetimes and counts are the spec's; the look
//! needs the real game's captures to compare.

use std::collections::BTreeMap;

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use chimera_sim::weapon::{
    WeaponEvent, WeaponEventKind,
    grenade::{Detonated, EXPLOSION_WEAPON},
    projectile::{Flyer, FlyerGone, FlyerHit},
};
use source_engine::effects::{
    explosions::{self, FxMaterials},
    impact_effects::{self, EffectMaterials},
};

use super::{
    effects::flat_quad,
    weapons::{
        self,
        launch::{BALL_MODEL, BOLT_MODEL, MISSILE_MODEL, SATCHEL_MODEL, SMG1_GRENADE_MODEL},
        physcannon::HeldBall,
        slam::{Hl2Mine, SatchelPop},
    },
};
use crate::{
    core::{BlastFlags, DamageKind, DamageTaken, LocalPlayer, MovementState},
    map::{
        DecalGroup, ParticleRng, PlaceDecal, PlaySound,
        logic_view::{ScreenFade, ScreenFades},
        loose::ShownItem,
        ragdoll::RagdollBody,
        particles::{
            Fade, Motion, Particle, ParticleGroup, ParticleMaterials, ParticleSet, Particles, Ramp, Shape,
            WorldTrace, WorldTracer,
        },
    },
};

const U: f32 = 0.0254;

/// A particle that is drawn in one frame only.
const ONE_FRAME: f32 = 0.0005;

/// The materials these effects use (loaded with the map's).
pub const MATERIALS: &[&str] = &[
    "effects/ar2_altfire1",
    "effects/ar2_altfire1b",
    "effects/combinemuzzle1",
    "effects/combinemuzzle1_nocull",
    "effects/combinemuzzle2_nocull",
    "effects/muzzleflash1",
    "effects/muzzleflash2",
    "effects/muzzleflash3",
    "effects/muzzleflash4",
    "sprites/lgtning",
    "sprites/redglow1",
    "sprites/bluelaser1",
    "sprites/light_glow02_noz",
    "particle/particle_noisesphere",
    "particle/particle_smokegrenade",
    "effects/spark",
    "sprites/orangelight1",
];

/// The sound of every explosion, heard on each client.
pub const EXPLOSION_SOUND: &str = "BaseExplosionEffect.Sound";

/// What a game's explosion looks like (the spec's Explosions table): by
/// the weapon that went off.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// The core (smoke, fire, sparks, flecks) is drawn: the temp entity's
    /// scale is not 0.
    pub core: bool,
    /// The explosion entity's rule: drawn 24 off a surface found by a
    /// line from 8 above to 40 below.
    pub entity_rule: bool,
}

/// How the explosion of `weapon` looks: the RPG's and the S.L.A.M.'s by
/// the explosion entity's rule, the frag's and the SMG1 grenade's at their
/// own point; every one has its core.
pub fn look_of(weapon: &str) -> Option<Look> {
    match weapon {
        weapons::FRAG | weapons::SMG1 => Some(Look { core: true, entity_rule: false }),
        weapons::RPG | weapons::SLAM => Some(Look { core: true, entity_rule: true }),
        _ => None,
    }
}

/// Where an explosion by the entity rule is drawn (spec 8): `hit` is what
/// a line from 8 units above `at` to 40 below that found (its point and
/// normal); drawn 24 units off it, else at `at`.
pub fn entity_rule_point(at: Vec3, hit: Option<(Vec3, Vec3)>) -> Vec3 {
    hit.map_or(at, |(p, n)| p + n * 24.0 * U)
}

/// This app is HL2:DM's own: it draws map logic's blasts (explosive props,
/// `env_explosion`s). In a game of several games, the first game's rule
/// and its look serve them (CS:S's draws those of no game).
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct DrawsMapBlasts;

/// `look_of`, and map logic's blasts by the explosion entity's rule when
/// `map_blasts` (a client knows them as `EXPLOSION_WEAPON`: only guns are
/// on the wire).
pub fn look_here(weapon: &str, map_blasts: bool) -> Option<Look> {
    look_of(weapon).or_else(|| {
        (map_blasts && (weapon == EXPLOSION_WEAPON || weapon == super::physics::PHYSICS_WEAPON))
            .then_some(Look { core: true, entity_rule: true })
    })
}

#[derive(Resource)]
struct FxRng(ParticleRng);

impl Default for FxRng {
    fn default() -> Self {
        Self(ParticleRng::new(0x5_1F0C))
    }
}

/// A group that lives one frame: added after the particles' step and
/// drawn that frame.
fn frame_group() -> ParticleGroup {
    let mut g = ParticleGroup::new(Motion::default());
    g.skip_first = false;
    g.capped = false;
    g
}

/// A one-frame sprite of half-size `half` units, colour `grey`, alpha `a`.
fn flash(mat: usize, at: Vec3, half: f32, grey: f32, a: f32) -> Particle {
    Particle { color: Vec3::splat(grey), alpha: a, ..Particle::new(at, ONE_FRAME, mat, half * U) }
}

// ---------------------------------------------------------------------------
// Explosions

/// The flash overlay's glow alone: one screen-facing glow at `at`, fading
/// in 0.1 s (a pop's, or the core's own).
fn flash_overlay(materials: &FxMaterials, at: Vec3) -> Option<ParticleGroup> {
    let glow = materials.glow?;
    let mut g = ParticleGroup::new(Motion::default());
    g.particles.push(Particle {
        color: Vec3::new(1.0, 0.9, 0.7),
        fade: Fade::Ramp(Ramp::linear(1.0, 0.0)),
        size: Ramp::linear(96.0 * U, (96.0 + 1.6) * U),
        ..Particle::new(at, 0.1, glow, 0.0)
    });
    Some(g)
}

/// Light at `p` as the map's light field gives it, grey 0.6 without one.
fn light(field: Option<&crate::map::LightField>, p: Vec3) -> Vec3 {
    explosions::light_at(field, p)
}

#[allow(clippy::too_many_arguments)]
fn explosions(
    mut detonated: MessageReader<Detonated>,
    materials: Option<Res<ParticleMaterials>>,
    field: Option<Res<crate::map::LightField>>,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<FxRng>,
    mut play: MessageWriter<PlaySound>,
    map_blasts: Option<Res<DrawsMapBlasts>>,
    world: WorldTracer,
) {
    let Some(materials) = materials else {
        detonated.clear();
        return;
    };
    let mats = FxMaterials::new(&materials.0);
    for d in detonated.read() {
        let Some(look) = look_here(d.weapon, map_blasts.is_some()) else { continue };
        let at = if look.entity_rule {
            let hit = world
                .trace(d.at + Vec3::Y * 8.0 * U, d.at + Vec3::Y * (8.0 - 40.0) * U)
                .map(|(f, n)| (d.at + Vec3::Y * 8.0 * U - Vec3::Y * 40.0 * U * f, n));
            entity_rule_point(d.at, hit)
        } else {
            d.at
        };
        if look.core {
            let a = explosions::blast_direction(at, &world);
            let l = light(field.as_deref(), at + a * 32.0 * U);
            for g in explosions::explosion(&mut rng.0, &mats, at, a, l, &world, d.flags) {
                particles.add(g);
            }
        } else if let Some(g) = flash_overlay(&mats, at) {
            particles.add(g);
        }
        if !d.flags.has(BlastFlags::NO_SOUND) {
            play.write(PlaySound::at(EXPLOSION_SOUND, at));
        }
    }
}

/// The line of the explosion entity's rule under `at` (spec 8): from 8
/// units above it to 40 below that; what it hits (point, normal).
pub fn entity_rule_hit(at: Vec3, world: &impl WorldTrace) -> Option<(Vec3, Vec3)> {
    let from = at + Vec3::Y * 8.0 * U;
    let to = from - Vec3::Y * 40.0 * U;
    world.trace(from, to).map(|(f, n)| (from + (to - from) * f, n))
}

/// The SMG1 grenade's scorch line along its flight (projectiles.md 4.3),
/// units.
pub const FLIGHT_SCORCH_LINE: f32 = 60.0;

/// The decal group an explosion's scorch takes on what its line hit:
/// `Scorch` on the world, `SmallScorch` on anything else (4.3).
pub fn scorch_group(on_world: bool) -> &'static str {
    if on_world { weapons::slam::SCORCH } else { "SmallScorch" }
}

/// The SMG1 grenade's scorch line: from where it went off along its
/// flight (the velocity it last flew with), 60 units; none when it had
/// stopped (a zero direction makes no line).
pub fn flight_line(at: Vec3, velocity: Vec3) -> Option<(Vec3, Vec3)> {
    let dir = velocity.try_normalize()?;
    Some((at, at + dir * FLIGHT_SCORCH_LINE * U))
}

/// What a scorch line hits: anything solid but characters and the
/// exploding projectile; whether it is the world (a static body that is
/// not a prop).
#[derive(bevy::ecs::system::SystemParam)]
struct ScorchLine<'w, 's> {
    spatial: avian3d::prelude::SpatialQuery<'w, 's>,
    colliders: Query<'w, 's, &'static avian3d::prelude::ColliderOf>,
    characters: Query<'w, 's, (), With<MovementState>>,
    statics: Query<'w, 's, &'static avian3d::prelude::RigidBody, Without<crate::map::MapPropCollider>>,
}

impl ScorchLine<'_, '_> {
    /// Point, normal, body and whether the body is the world.
    fn trace(&self, from: Vec3, to: Vec3, skip: Entity) -> Option<(Vec3, Vec3, Entity, bool)> {
        let d = to - from;
        let dir = Dir3::new(d).ok()?;
        let filter = avian3d::prelude::SpatialQueryFilter::default().with_mask(crate::core::SOLID_LAYERS);
        let body = |c: Entity| self.colliders.get(c).map_or(c, |o| o.body);
        let h = self.spatial.cast_ray_predicate(from, dir, d.length(), true, &filter, &|c| {
            let b = body(c);
            b != skip && c != skip && !self.characters.contains(b)
        })?;
        let b = body(h.entity);
        let world = self.statics.get(b).is_ok_and(|r| r.is_static()) || self.statics.get(h.entity).is_ok_and(|r| r.is_static());
        Some((from + *dir * h.distance, h.normal, b, world))
    }
}

/// The server's scorches (Chimera's decals reach the clients):
/// - an explosion by the entity rule (the RPG's, a satchel's, a trip
///   mine's) leaves a `Scorch` where its line hits (spec 8; none in
///   mid-air or on a wall well above the floor);
/// - the SMG1 grenade's, where a 60-unit line along its flight from its
///   origin hits (projectiles.md 4.3): `Scorch` on the world,
///   `SmallScorch` on a prop or brush entity, nothing on a player.
fn scorches(
    mut detonated: MessageReader<Detonated>,
    world: WorldTracer,
    line: ScorchLine,
    flyers: Query<(Entity, &Flyer)>,
    map_blasts: Option<Res<DrawsMapBlasts>>,
    mut flights: Local<BTreeMap<Entity, Vec3>>,
    mut decals: MessageWriter<PlaceDecal>,
) {
    let mut place = |target: Option<Entity>, group: &str, point: Vec3, normal: Vec3| {
        decals.write(PlaceDecal {
            target,
            group: DecalGroup::Named(group.into()),
            point,
            normal,
            dir: -normal,
            spin: true,
            round: false,
        });
    };
    for d in detonated.read() {
        if d.weapon == weapons::SMG1 {
            // The velocity it flew with before it went off.
            let v = flights
                .get(&d.projectile)
                .copied()
                .or_else(|| flyers.get(d.projectile).ok().map(|(_, f)| f.velocity));
            let hit = v.and_then(|v| flight_line(d.at, v)).and_then(|(a, b)| line.trace(a, b, d.projectile));
            if let Some((point, normal, body, on_world)) = hit {
                place((!on_world).then_some(body), scorch_group(on_world), point, normal);
            }
            continue;
        }
        if !look_here(d.weapon, map_blasts.is_some()).is_some_and(|l| l.entity_rule)
            || d.flags.has(BlastFlags::NO_DECAL)
        {
            continue;
        }
        if let Some((point, normal)) = entity_rule_hit(d.at, &world) {
            place(None, weapons::slam::SCORCH, point, normal);
        }
    }
    flights.clear();
    for (e, f) in &flyers {
        if f.weapon == weapons::SMG1 {
            flights.insert(e, f.velocity);
        }
    }
}

/// A shot satchel's pop (spec test 40), on every machine: the flash and
/// the debris (no core), and `BaseExplosionEffect.Sound`.
fn pops(
    mut pops: MessageReader<SatchelPop>,
    materials: Option<Res<ParticleMaterials>>,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<FxRng>,
    mut play: MessageWriter<PlaySound>,
    world: WorldTracer,
) {
    for p in pops.read() {
        if let Some(m) = materials.as_ref() {
            let mats = FxMaterials::new(&m.0);
            let a = explosions::blast_direction(p.at, &world);
            for g in explosions::pop(&mut rng.0, &mats, p.at, a, &world) {
                particles.add(g);
            }
        }
        play.write(PlaySound::at(EXPLOSION_SOUND, p.at));
    }
}

/// The server: this tick's pops to every client.
fn forward_pops(mut pops: MessageReader<SatchelPop>, mut out: MessageWriter<ToClients<SatchelPop>>) {
    for p in pops.read() {
        out.write(ToClients { targets: SendTargets::CLIENTS_ONLY, message: *p });
    }
}

/// The protocol, from the server: shot satchels' pops and the flyers'
/// effects (a client's own `SatchelPop` and `FlyerFx` messages, which
/// `pops` and `draw_flyer_fx` draw), held balls and armed mines' beams.
fn register(app: &mut App) {
    use bevy_replicon::prelude::AppRuleExt;
    use chimera_net::net::ordered::OrderedAppExt;
    app.add_server_message::<SatchelPop>(Channel::Unreliable)
        .add_server_message::<FlyerFx>(Channel::Unreliable)
        .replicate::<Hl2BallHeld>()
        .ordered::<Hl2BallHeld>()
        .replicate::<Hl2MineBeam>()
        .ordered::<Hl2MineBeam>()
        .replicate::<Hl2FlyerVelocity>()
        .add_systems(
            PostUpdate,
            (forward_pops, forward_flyer_fx)
                .before(ServerSystems::Send)
                .run_if(resource_equals(crate::core::NetRole::Server)),
        );
}

/// The AR2 shooter's white screen fade as the ball leaves (projectiles.md
/// 5.1: 255, 255, 255, alpha 64 over 0.1 s), on his own machine (the
/// launch is predicted).
pub const BALL_FADE: ([u8; 3], u8, f32) = ([255, 255, 255], 64, 0.1);

fn shooter_fades(
    mut events: MessageReader<WeaponEvent>,
    local: Query<(), With<LocalPlayer>>,
    mut fades: Option<ResMut<ScreenFades>>,
    time: Res<Time>,
) {
    for e in events.read().filter(|e| e.shown()) {
        if !matches!(e.kind, WeaponEventKind::Action("ball")) || !local.contains(e.owner) {
            continue;
        }
        if let Some(f) = fades.as_mut() {
            let (color, alpha, duration) = BALL_FADE;
            f.show(
                ScreenFade {
                    duration: ScreenFade::quantize(duration),
                    hold: 0.0,
                    color,
                    alpha,
                    fade_in: true,
                    modulate: false,
                    stay_out: false,
                    purge: false,
                },
                time.elapsed_secs_f64(),
            );
        }
    }
}

/// The screen flash of crush damage (a prop's hit counts) and of drowning
/// (effects_sounds.md 7 "Blood"): colour, alpha, fade seconds, hold seconds.
pub const CRUSH_FLASH: ([u8; 3], u8, f32, f32) = ([128, 0, 0], 128, 1.0, 0.1);
pub const DROWN_FLASH: ([u8; 3], u8, f32, f32) = ([0, 0, 128], 128, 1.0, 0.1);

/// The flash a damage kind makes on its victim's screen, if any.
pub fn damage_flash(kind: DamageKind) -> Option<([u8; 3], u8, f32, f32)> {
    match kind {
        DamageKind::Crush => Some(CRUSH_FLASH),
        DamageKind::Drown => Some(DROWN_FLASH),
        _ => None,
    }
}

/// The local player's screen flashes red when crushed and blue when
/// drowning (`DamageTaken`: the server's report on a network client).
fn damage_flashes(
    mut taken: MessageReader<DamageTaken>,
    local: Query<(), With<LocalPlayer>>,
    mut fades: Option<ResMut<ScreenFades>>,
    time: Res<Time>,
) {
    for t in taken.read() {
        let Some((color, alpha, duration, hold)) = damage_flash(t.damage.kind) else { continue };
        if !local.contains(t.damage.target) || t.damage.amount <= 0.0 {
            continue;
        }
        if let Some(f) = fades.as_mut() {
            f.show(
                ScreenFade {
                    duration: ScreenFade::quantize(duration),
                    hold,
                    color,
                    alpha,
                    fade_in: true,
                    modulate: false,
                    stay_out: false,
                    purge: false,
                },
                time.elapsed_secs_f64(),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Rings (the ball's end)

/// An expanding ring lying flat: radius `from` to `to` meters over
/// `life` seconds, `width` meters wide, `alpha` at the start, fading.
#[derive(Clone, Copy, Debug)]
struct Ring {
    at: Vec3,
    from: f32,
    to: f32,
    life: f32,
    width: f32,
    alpha: f32,
    start: f64,
}

#[derive(Resource, Default)]
struct Rings(Vec<Ring>);

/// The ball's two rings (spec 9, `ball_explode`): radius 10 to 1024
/// units over 0.2 s and 0.5 s, 64 wide, alpha 32 and 64.
fn ball_rings(at: Vec3, now: f64) -> [Ring; 2] {
    let ring = |life: f32, alpha: f32| Ring { at, from: 10.0 * U, to: 1024.0 * U, life, width: 64.0 * U, alpha: alpha / 255.0, start: now };
    [ring(0.2, 32.0), ring(0.5, 64.0)]
}

const RING_SEGMENTS: usize = 48;

fn draw_rings(
    time: Res<Time>,
    mut rings: ResMut<Rings>,
    materials: Option<Res<ParticleMaterials>>,
    mut particles: ResMut<Particles>,
) {
    let now = time.elapsed_secs_f64();
    rings.0.retain(|r| now - r.start < r.life as f64);
    let Some(mat) = materials.as_ref().and_then(|m| m.0.find("sprites/lgtning")) else { return };
    let mut g = frame_group();
    for r in &rings.0 {
        let t = ((now - r.start) as f32 / r.life).clamp(0.0, 1.0);
        let radius = r.from + (r.to - r.from) * t;
        let at = |k: usize| {
            let a = k as f32 / RING_SEGMENTS as f32 * std::f32::consts::TAU;
            r.at + Vec3::new(a.cos(), 0.0, a.sin()) * radius
        };
        for k in 0..RING_SEGMENTS {
            g.particles.push(Particle {
                color: Vec3::new(1.0, 1.0, 225.0 / 255.0),
                alpha: r.alpha * (1.0 - t),
                shape: Shape::Ribbon {
                    to: at(k + 1),
                    width: [r.width, r.width],
                    u: (0.0, 1.0),
                    end_color: None,
                    prev: Some(at(k + RING_SEGMENTS - 1)).filter(|_| k > 0),
                    next: Some(at(k + 2)),
                },
                ..Particle::new(at(k), ONE_FRAME, mat, 1.0)
            });
        }
    }
    if !g.particles.is_empty() {
        particles.add(g);
    }
}

// ---------------------------------------------------------------------------
// The ball's bounce and end, the bolt's sticking (the server's `FlyerFx`)

/// The flyers' one-off effects, made where the flyers are simulated (the
/// host or the server, from `FlyerHit`/`FlyerGone`) and drawn on every
/// machine: a network server sends them to its clients (net part
/// `hl2dm_projectile_fx`), as it does the satchels' pops.
#[derive(Message, Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FlyerFx {
    /// The energy ball's bounce on the world (`cball_bounce`).
    BallBounce { point: Vec3, normal: Vec3 },
    /// The energy ball's end (`ball_explode`: sparks and two rings).
    BallEnd { at: Vec3 },
    /// A crossbow bolt stuck in the world: its glow and sparks; `dir` its
    /// flight's direction.
    BoltStuck { point: Vec3, normal: Vec3, dir: Vec3 },
    /// "BoltImpact" (projectiles.md 7.4-7.5): where a bolt stuck in the
    /// world, or where a line 128 units on from a body it hit meets the
    /// world; `dir` its flight's direction. Each machine pins a ragdoll
    /// behind it and shows a bolt there.
    BoltImpact { point: Vec3, dir: Vec3 },
}

/// How far on from a body a bolt hit the world is looked for, and how far
/// back from a bolt's impact a ragdoll bone is (projectiles.md 7.4, 7.5),
/// units.
pub const BOLT_PIN_REACH: f32 = 128.0;
/// The bolt shown at an impact: this far back along its flight, for this
/// long (s).
pub const PIN_BOLT_BACK: f32 = 8.0;
pub const PIN_BOLT_LIFE: f64 = 30.0;
pub const BOLT_SKEWER: &str = "Weapon_Crossbow.BoltSkewer";

/// `cball_bounce` (spec 9): two flat quads at `at` + 1 unit off the
/// surface `n`: 160 to 0 units over 0.25 s and 32 to 64 over 0.5 s.
fn ball_bounce(mats: &MaterialIds, rng: &mut ParticleRng, at: Vec3, n: Vec3) -> Vec<ParticleGroup> {
    let at = at + n * U;
    let mut out = Vec::new();
    if let Some(m) = mats.combine1_nocull {
        out.push(flat_quad(m, at, n, rng.int(0, 360) as f32, (160.0 * U, 0.0), 0.75, 0.4, 0.25));
    }
    if let Some(m) = mats.combine2_nocull {
        out.push(flat_quad(m, at, n, rng.int(0, 360) as f32, (32.0 * U, 64.0 * U), 0.75, 0.4, 0.5));
    }
    out
}

/// The bolt's glow where it stuck (spec 9, `bolt_stuck_glow`): white,
/// alpha 128, half 0.2 of a 64-unit sprite, fading over 3 s.
fn bolt_glow(mats: &MaterialIds, at: Vec3) -> Option<ParticleGroup> {
    let m = mats.glow_noz?;
    let mut g = ParticleGroup::new(Motion::default());
    g.particles.push(Particle {
        alpha: 0.5,
        fade: Fade::Ramp(Ramp::linear(1.0, 0.0)),
        ..Particle::new(at, 3.0, m, 0.2 * 32.0 * U)
    });
    Some(g)
}

/// The materials the effects look up once a frame.
struct MaterialIds {
    combine1_nocull: Option<usize>,
    combine2_nocull: Option<usize>,
    glow_noz: Option<usize>,
}

impl MaterialIds {
    fn new(p: &crate::map::particles::MapParticles) -> Self {
        Self {
            combine1_nocull: p.find("effects/combinemuzzle1_nocull"),
            combine2_nocull: p.find("effects/combinemuzzle2_nocull"),
            glow_noz: p.find("sprites/light_glow02_noz"),
        }
    }
}

/// The flyers' effects from the host's flyer messages (where flyers are
/// simulated): a world bounce of the ball, its end when its time ran out,
/// a bolt sticking in the world.
fn flyer_fx(
    mut hits: MessageReader<FlyerHit>,
    mut gone: MessageReader<FlyerGone>,
    flyers: Query<&Flyer>,
    world: WorldTracer,
    mut out: MessageWriter<FlyerFx>,
) {
    for h in hits.read() {
        if h.weapon == weapons::AR2 && h.world {
            out.write(FlyerFx::BallBounce { point: h.point, normal: h.normal });
        }
        if h.weapon != weapons::CROSSBOW {
            continue;
        }
        let dir = h.velocity.normalize_or_zero();
        if h.world && flyers.get(h.flyer).is_ok_and(|f| f.stuck_until.is_some()) {
            out.write(FlyerFx::BoltStuck { point: h.point, normal: h.normal, dir });
            out.write(FlyerFx::BoltImpact { point: h.point, dir });
        } else if h.character && dir != Vec3::ZERO {
            // A body: the world 128 units on along its flight.
            let to = h.point + dir * BOLT_PIN_REACH * U;
            if let Some((f, _)) = world.trace(h.point, to) {
                out.write(FlyerFx::BoltImpact { point: h.point + (to - h.point) * f, dir });
            }
        }
    }
    for g in gone.read() {
        if g.weapon == weapons::AR2 && g.expired {
            out.write(FlyerFx::BallEnd { at: g.at });
        }
    }
}

/// The flyers' effects drawn, on every machine.
fn draw_flyer_fx(
    mut fx: MessageReader<FlyerFx>,
    materials: Option<Res<ParticleMaterials>>,
    time: Res<Time>,
    mut particles: ResMut<Particles>,
    mut rings: ResMut<Rings>,
    mut rng: ResMut<FxRng>,
    mut pins: ResMut<Pins>,
    world: WorldTracer,
) {
    let now = time.elapsed_secs_f64();
    let all: Vec<FlyerFx> = fx.read().copied().collect();
    for f in &all {
        if let FlyerFx::BoltImpact { point, dir } = *f {
            pins.wanted.push(PinWanted { point, dir, until: now + PIN_WAIT });
            pins.bolts.push(PinBolt { at: point - dir * PIN_BOLT_BACK * U, dir, until: now + PIN_BOLT_LIFE, shown: None });
        }
    }
    let Some(materials) = materials else { return };
    let ids = MaterialIds::new(&materials.0);
    let effect = EffectMaterials::new(&materials.0);
    for f in all {
        let groups = match f {
            FlyerFx::BallBounce { point, normal } => {
                let mut g = ball_bounce(&ids, &mut rng.0, point, normal);
                g.extend(impact_effects::electric_sparks(&mut rng.0, &effect, point, normal, &world));
                g
            }
            FlyerFx::BoltStuck { point, normal, .. } => {
                let mut g: Vec<ParticleGroup> = bolt_glow(&ids, point).into_iter().collect();
                g.extend(impact_effects::electric_sparks(&mut rng.0, &effect, point, normal, &world));
                g
            }
            FlyerFx::BallEnd { at } => {
                rings.0.extend(ball_rings(at, now));
                impact_effects::electric_sparks(&mut rng.0, &effect, at, Vec3::Y, &world)
            }
            FlyerFx::BoltImpact { .. } => Vec::new(),
        };
        for g in groups {
            particles.add(g);
        }
    }
}

// ---------------------------------------------------------------------------
// Bolts pinning ragdolls (projectiles.md 7.5, each machine for its own)

/// How long an impact waits for a ragdoll to pin (the victim's ragdoll is
/// made as the bolt's kill reaches this machine), seconds.
pub const PIN_WAIT: f64 = 0.25;

struct PinWanted {
    point: Vec3,
    dir: Vec3,
    until: f64,
}

/// A bolt shown at an impact for 30 s (its `ShownItem`, this machine's).
struct PinBolt {
    at: Vec3,
    dir: Vec3,
    until: f64,
    shown: Option<Entity>,
}

/// A ragdoll body pinned to the world: the joint, its anchor and the body.
#[derive(Clone, Copy, Debug)]
pub struct Pin {
    pub joint: Entity,
    pub anchor: Entity,
    pub body: Entity,
}

/// The impacts waiting for a ragdoll, the bolts shown at them and the
/// pins made.
#[derive(Resource, Default)]
pub struct Pins {
    wanted: Vec<PinWanted>,
    bolts: Vec<PinBolt>,
    pub pins: Vec<Pin>,
}

/// The first ragdoll body a line from `point` back along the bolt's
/// flight `dir` crosses within 128 units: the body and where.
fn bone_behind(
    spatial: &avian3d::prelude::SpatialQuery,
    parts: &Query<&RagdollBody>,
    point: Vec3,
    dir: Vec3,
) -> Option<Entity> {
    let back = Dir3::new(-dir).ok()?;
    let filter = avian3d::prelude::SpatialQueryFilter::from_mask(crate::core::RAGDOLL_LAYER);
    spatial
        .ray_hits(point, back, BOLT_PIN_REACH * U, 16, true, &filter)
        .into_iter()
        .filter(|h| parts.contains(h.entity))
        .min_by(|a, b| a.distance.total_cmp(&b.distance))
        .map(|h| h.entity)
}

/// Each machine: an impact pins the first ragdoll bone behind it to the
/// world at the impact (a ball-and-socket joint, the bone's mass doubled,
/// `Weapon_Crossbow.BoltSkewer`); the bolts are shown for 30 s; pins
/// whose body is gone go too.
#[allow(clippy::too_many_arguments)]
fn pin_ragdolls(
    mut pins: ResMut<Pins>,
    spatial: avian3d::prelude::SpatialQuery,
    parts: Query<&RagdollBody>,
    masses: Query<&avian3d::prelude::Mass>,
    exists: Query<()>,
    time: Res<Time>,
    mut play: MessageWriter<PlaySound>,
    mut commands: Commands,
) {
    use avian3d::prelude::{Mass, RigidBody, SphericalJoint};
    let now = time.elapsed_secs_f64();
    let pins = &mut *pins;
    pins.pins.retain(|p| {
        let keep = exists.contains(p.body);
        if !keep {
            commands.entity(p.joint).try_despawn();
            commands.entity(p.anchor).try_despawn();
        }
        keep
    });
    let mut made = Vec::new();
    pins.wanted.retain(|w| {
        let Some(body) = bone_behind(&spatial, &parts, w.point, w.dir) else { return now < w.until };
        if pins.pins.iter().chain(&made).any(|p: &Pin| p.body == body) {
            return false;
        }
        let anchor = commands
            .spawn((Name::new("Bolt pin"), crate::map::MapPart, RigidBody::Static, Transform::from_translation(w.point)))
            .id();
        let joint = commands
            .spawn((Name::new("Bolt pin joint"), crate::map::MapPart, SphericalJoint::new(anchor, body).with_anchor(w.point)))
            .id();
        if let Ok(m) = masses.get(body) {
            commands.entity(body).try_insert(Mass(m.0 * 2.0));
        }
        play.write(PlaySound::at(BOLT_SKEWER, w.point));
        made.push(Pin { joint, anchor, body });
        false
    });
    pins.pins.extend(made);
    for b in &mut pins.bolts {
        if b.shown.is_none() {
            let turn = Quat::from_rotation_arc(Vec3::Y, b.dir.try_normalize().unwrap_or(Vec3::Y));
            b.shown = Some(
                commands
                    .spawn((
                        Name::new("Pinned bolt"),
                        crate::map::MapPart,
                        ShownItem(BOLT_MODEL.into()),
                        Transform::from_translation(b.at).with_rotation(turn),
                    ))
                    .id(),
            );
        }
    }
    pins.bolts.retain(|b| {
        let keep = now < b.until;
        if !keep && let Some(e) = b.shown {
            commands.entity(e).try_despawn();
        }
        keep
    });
}

/// The server: this tick's flyer effects to every client.
fn forward_flyer_fx(mut fx: MessageReader<FlyerFx>, mut out: MessageWriter<ToClients<FlyerFx>>) {
    for f in fx.read() {
        out.write(ToClients { targets: SendTargets::CLIENTS_ONLY, message: *f });
    }
}

// ---------------------------------------------------------------------------
// What the server marks for clients: a held ball, a trip mine's beam

/// A combine ball held in a gravity gun (the server's `HeldBall`,
/// replicated): drawn without its body quad (spec 9).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, serde::Serialize, serde::Deserialize)]
#[reflect(Component)]
pub struct Hl2BallHeld;

/// An armed trip mine's beam (the server's `Hl2Mine::beam`, replicated):
/// its length (meters) along the mine's normal (its model's −Z).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Reflect, serde::Serialize, serde::Deserialize)]
#[reflect(Component)]
pub struct Hl2MineBeam(pub f32);

/// A contact grenade's velocity as the server last had it (m/s,
/// replicated, unquantised): its smoke trail's V on a client, held from
/// one snapshot to the next (particles_and_smoke.md 5, "What V is on a
/// client").
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Reflect, serde::Serialize, serde::Deserialize)]
#[reflect(Component)]
pub struct Hl2FlyerVelocity(pub Vec3);

/// The server's: `Hl2BallHeld` follows `HeldBall`; an armed mine gets its
/// `Hl2MineBeam`; a contact grenade its `Hl2FlyerVelocity`.
#[allow(clippy::type_complexity)]
fn mark_for_clients(
    balls: Query<(Entity, Has<HeldBall>, Has<Hl2BallHeld>), With<Flyer>>,
    mines: Query<(Entity, &Hl2Mine), Without<Hl2MineBeam>>,
    mut grenades: Query<(Entity, &Flyer, Option<&mut Hl2FlyerVelocity>)>,
    mut commands: Commands,
) {
    for (e, f, v) in &mut grenades {
        if f.weapon != weapons::SMG1 {
            continue;
        }
        match v {
            Some(mut v) => {
                v.set_if_neq(Hl2FlyerVelocity(f.velocity));
            }
            None => {
                commands.entity(e).insert(Hl2FlyerVelocity(f.velocity));
            }
        }
    }
    for (e, held, marked) in &balls {
        if held && !marked {
            commands.entity(e).insert(Hl2BallHeld);
        } else if !held && marked {
            commands.entity(e).remove::<Hl2BallHeld>();
        }
    }
    for (e, m) in &mines {
        if let Some((len, _)) = m.beam {
            commands.entity(e).insert(Hl2MineBeam(len));
        }
    }
}

/// The trip mine's beam colour (255, 55, 52), brightness 64 (of 255) and
/// width (units; projectiles.md 8.2).
pub const MINE_BEAM_COLOR: Vec3 = Vec3::new(1.0, 55.0 / 255.0, 52.0 / 255.0);
pub const MINE_BEAM_BRIGHTNESS: f32 = 64.0 / 255.0;
pub const MINE_BEAM_WIDTH: f32 = 0.35;
/// Its material: a laser sprite (the spec names none; HL2:DM's laser
/// beam, the RPG's world-model one, additive).
pub const MINE_BEAM_MATERIAL: &str = super::rpg_fx::WORLD_BEAM;

/// Each armed trip mine's beam this frame: from the mine along its normal
/// for the length it armed with.
fn mine_beams(
    mines: Query<(&Hl2MineBeam, &GlobalTransform)>,
    materials: Option<Res<ParticleMaterials>>,
    mut particles: ResMut<Particles>,
) {
    let Some(mat) = materials.as_ref().and_then(|m| m.0.find(MINE_BEAM_MATERIAL)) else { return };
    let mut g = frame_group();
    for (beam, at) in &mines {
        let (_, rotation, from) = at.to_scale_rotation_translation();
        let to = from + rotation * Vec3::NEG_Z * beam.0;
        g.particles.push(Particle {
            color: MINE_BEAM_COLOR,
            alpha: MINE_BEAM_BRIGHTNESS,
            shape: Shape::Ribbon {
                to,
                width: [MINE_BEAM_WIDTH * U, MINE_BEAM_WIDTH * U],
                u: (0.0, 1.0),
                end_color: None,
                prev: None,
                next: None,
            },
            ..Particle::new(from, ONE_FRAME, mat, 1.0)
        });
    }
    if !g.particles.is_empty() {
        particles.add(g);
    }
}

// ---------------------------------------------------------------------------
// What the shown projectiles do each frame

/// What each shown projectile was doing last frame: where, when first
/// seen, the distance (or time) carried toward the next puff.
#[derive(Default)]
struct Track {
    at: Vec3,
    seen: f64,
    carry: f32,
}

/// A direction at random.
fn random_dir(rng: &mut ParticleRng) -> Vec3 {
    loop {
        let v = rng.vec3(-1.0, 1.0);
        if v.length_squared() > 0.01 && v.length_squared() <= 1.0 {
            return v.normalize();
        }
    }
}

fn smoke_material(rng: &mut ParticleRng, m: &impl Fn(&str) -> Option<usize>) -> Option<usize> {
    let names = ["particle/particle_smokegrenade", "particle/particle_noisesphere"];
    m(names[rng.int(0, 1) as usize]).or_else(|| m(names[0]))
}

/// The rocket's look this frame (spec 9): 8 flare sprites at 3·i units
/// behind, half-size ⌊U(5, 6)·(12 − i)/9·k⌋ with k from 1 to 2, alpha 1
/// to 0.5 over their 0.01 s; a smoke puff per 4 units travelled (at most
/// 50 a frame): life 0.5 + U(0.45, 0.55), drifting 2 to 16 units/s, grey
/// 0.65 of 0.75 to 1.25, size 8 to 32, alpha 0.2 swelling and fading.
fn rocket(
    g: &mut ParticleGroup,
    smoke: &mut ParticleGroup,
    rng: &mut ParticleRng,
    m: &impl Fn(&str) -> Option<usize>,
    at: Vec3,
    moved: Vec3,
    carry: &mut f32,
) {
    let dir = moved.normalize_or_zero();
    if dir == Vec3::ZERO {
        return;
    }
    let flares = ["effects/muzzleflash1", "effects/muzzleflash2", "effects/muzzleflash3", "effects/muzzleflash4"];
    for i in 1..=8 {
        let Some(mat) = m(flares[rng.int(0, 3) as usize]) else { break };
        let k = rng.float(1.0, 2.0);
        let half = (rng.float(5.0, 6.0) * (12 - i) as f32 / 9.0 * k).floor();
        g.particles.push(Particle {
            fade: Fade::Ramp(Ramp::linear(1.0, 0.5)),
            ..flash(mat, at - dir * (3.0 * i as f32) * U, half, 1.0, 1.0)
        });
    }
    *carry += moved.length();
    let step = 4.0 * U;
    let n = ((*carry / step) as usize).min(50);
    *carry -= n as f32 * step;
    for j in 0..n {
        let Some(mat) = smoke_material(rng, m) else { break };
        let p = at - moved * (j as f32 / n.max(1) as f32);
        let life = 0.5 + rng.float(0.45, 0.55);
        let grey = (0.65 * rng.float(0.75, 1.25)).clamp(0.0, 1.0);
        smoke.particles.push(Particle {
            velocity: random_dir(rng) * rng.float(2.0, 16.0) * U,
            color: Vec3::splat(grey),
            alpha: (0.2 * rng.float(0.75, 1.25)).clamp(0.0, 1.0),
            // sin(π·age/life) as a rise and fall.
            fade: Fade::Windows { start_alpha: 0.0, fade_in: (0.0, 0.5), fade_out: (0.5, 1.0) },
            size: Ramp::linear(8.0 * rng.float(0.75, 1.25) * U, 32.0 * rng.float(1.0, 1.25) * U),
            roll: (rng.int(0, 360) as f32).to_radians(),
            roll_speed: rng.float(-8.0, 8.0),
            ..Particle::new(p, life, mat, 0.0)
        });
    }
}

/// The ball's look this frame (spec 9): the body quad (not while `held`) of half-size
/// 10 + sin(25 t) units, rolled by its spawn time, the flicker halo
/// (1.5 to 2.5 times, grey 0.2 to 0.3) and 8 blur halos behind, stepping
/// back min(0.5 s, 4) units with grey `remap(s: 4 → 0, 32 → 1) · (1 − i/12)`.
#[allow(clippy::too_many_arguments)]
fn ball(
    g: &mut ParticleGroup,
    rng: &mut ParticleRng,
    m: &impl Fn(&str) -> Option<usize>,
    at: Vec3,
    moved: Vec3,
    seen: f64,
    now: f64,
    held: bool,
) {
    let r = 10.0;
    if let Some(body) = m("effects/ar2_altfire1").filter(|_| !held) {
        let t = (now - seen) as f32;
        g.particles.push(Particle {
            roll: seen as f32,
            age: t,
            life: t + ONE_FRAME,
            fps: 30.0,
            frame_loop: true,
            ..flash(body, at, r + (25.0 * t).sin(), 1.0, 1.0)
        });
    }
    if let Some(halo) = m("effects/combinemuzzle1") {
        let grey = rng.float(0.2, 0.3);
        g.particles.push(flash(halo, at, r * rng.float(1.5, 2.5), grey, 1.0));
    }
    let s = moved.length() / U;
    if let Some(blur) = m("effects/ar2_altfire1b") {
        let dir = moved.normalize_or_zero();
        let step = (0.5 * s).min(4.0);
        let strength = ((s.min(32.0) - 4.0) / 28.0).clamp(0.0, 1.0);
        if dir != Vec3::ZERO && strength > 0.0 {
            for i in 0..8 {
                let grey = strength * (1.0 - i as f32 / 12.0);
                g.particles.push(flash(blur, at - dir * step * i as f32 * U, r, grey, 1.0));
            }
        }
    }
}

/// The bolt's blur (spec 9): 20 halos of half-size 3 from 24 units ahead
/// along its motion, stepping back min(0.5 s, 4) units; grey 0.25 for the
/// first five, ramping to 0.05 at the 11th, 0.05 after.
fn bolt(g: &mut ParticleGroup, m: &impl Fn(&str) -> Option<usize>, at: Vec3, moved: Vec3) {
    let s = moved.length() / U;
    let dir = moved.normalize_or_zero();
    let Some(mat) = m("effects/muzzleflash1") else { return };
    if dir == Vec3::ZERO || s < 0.5 {
        return;
    }
    let step = (0.5 * s.min(32.0)).min(4.0);
    for i in 0..20 {
        let grey = match i {
            0..5 => 0.25,
            5..11 => 0.25 - 0.20 * (i - 4) as f32 / 7.0,
            _ => 0.05,
        };
        g.particles.push(flash(mat, at + dir * 24.0 * U - dir * step * i as f32 * U, 3.0, grey, 1.0));
    }
}

/// The contact grenade's `env_smoketrail` (spec 9;
/// particles_and_smoke.md 5): 48 puffs a second for 10 s, spread along the
/// path of this frame, life 1 s, half-size 12 to 48 units, darkening to
/// black, each either of the two smoke materials, opacity about 0.2 through
/// a sine, drifting 4 to 24 units/s.
///
/// V (particles_and_smoke.md 5, answered 2026-11-09) is the grenade's
/// velocity as the server last sent it (`Hl2FlyerVelocity`, held between
/// snapshots, none: 0), O its drawn origin: a frame's puffs are spread
/// over O − V·Δt + V·τ and each takes V on top of its drift.
#[allow(clippy::too_many_arguments)]
fn smoke_trail(
    smoke: &mut ParticleGroup,
    rng: &mut ParticleRng,
    m: &impl Fn(&str) -> Option<usize>,
    at: Vec3,
    v: Vec3,
    dt: f32,
    age: f64,
    carry: &mut f32,
) {
    if age > 10.0 {
        return;
    }
    let mats = [m("particle/particle_smokegrenade"), m("particle/particle_noisesphere")];
    *carry += 48.0 * dt;
    let n = *carry as usize;
    *carry -= n as f32;
    for k in 0..n {
        let Some(mat) = mats[rng.int(0, 1) as usize].or(mats[0]).or(mats[1]) else { return };
        let tau = dt * (k as f32 + rng.float(0.0, 1.0)) / n as f32;
        let along = at - v * dt + v * tau;
        let jitter = Vec3::new(rng.float(-4.0, 4.0), rng.float(-4.0, 4.0), rng.float(-4.0, 4.0)) * U;
        let drift = Vec3::new(rng.float(-1.0, 1.0), rng.float(-1.0, 1.0), rng.float(-1.0, 1.0)) * rng.float(4.0, 24.0) * U;
        let grey = (0.1 + rng.float(-0.2, 0.2)).clamp(0.0, 1.0);
        smoke.particles.push(Particle {
            velocity: drift + v,
            color: Vec3::splat(grey),
            color_fade: Some((Vec3::ZERO, 0.0, 1.0)),
            alpha: (rng.float(0.75, 1.25) * 0.2).clamp(0.0, 1.0),
            fade: Fade::Sine,
            size: Ramp::linear(12.0 * U, 48.0 * U),
            // The spec's quirk: a whole number of degrees used as radians.
            roll: rng.int(0, 360) as f32,
            roll_speed: rng.float(-1.0, 1.0),
            ..Particle::new(along + jitter, 1.0, mat, 0.0)
        });
    }
}

/// The frag's and the satchel's red glow (`sprites/redglow1`, scale 0.2):
/// a beat that quickens once the frag is warned (1 s), and the frag's red
/// trail (`sprites/bluelaser1`, half a second).
fn glow_and_trail(
    g: &mut ParticleGroup,
    trail: &mut ParticleGroup,
    m: &impl Fn(&str) -> Option<usize>,
    at: Vec3,
    before: Vec3,
    age: f64,
    satchel: bool,
) {
    if let Some(glow) = m("sprites/redglow1") {
        let period = if satchel || age >= 1.0 { 0.3 } else { 1.0 };
        let on = (age % period) < 0.1;
        g.particles.push(flash(glow, at + Vec3::Y * 2.0 * U, 6.4, 1.0, if on { 1.0 } else { 0.35 }));
    }
    if !satchel && let Some(beam) = m("sprites/bluelaser1") && before.distance(at) > 0.01 {
        trail.particles.push(Particle {
            color: Vec3::new(1.0, 0.1, 0.1),
            fade: Fade::Ramp(Ramp::linear(1.0, 0.0)),
            shape: Shape::Ribbon {
                to: at,
                width: [8.0 * U * 0.5, 4.0 * U * 0.5],
                u: (0.0, 1.0),
                end_color: None,
                prev: None,
                next: None,
            },
            ..Particle::new(before, 0.5, beam, 1.0)
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn projectile_looks(
    items: Query<(Entity, &ShownItem, &GlobalTransform, Has<Hl2BallHeld>, Option<&Hl2FlyerVelocity>)>,
    materials: Option<Res<ParticleMaterials>>,
    time: Res<Time>,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<FxRng>,
    mut tracks: Local<BTreeMap<Entity, Track>>,
) {
    let Some(materials) = materials else { return };
    let m = |name: &str| materials.0.find(name);
    let (now, dt) = (time.elapsed_secs_f64(), time.delta_secs());
    let mut frame = frame_group();
    let mut smoke = ParticleGroup::new(Motion { roll_decay: Some((8.0, 0.5)), ..default() });
    let mut trail = ParticleGroup::new(Motion::default());
    // The contact grenade's: invisible nearer than 64 units, whole from 128.
    let mut grenade_smoke = ParticleGroup::new(Motion {
        roll_decay: Some((8.0, 0.5)),
        near_fade: Some((64.0 * U, 128.0 * U)),
        ..default()
    });
    let mut seen = Vec::new();
    for (e, shown, at, held, velocity) in &items {
        let key = shown.0.as_str();
        if ![MISSILE_MODEL, BALL_MODEL, BOLT_MODEL, SMG1_GRENADE_MODEL, SATCHEL_MODEL, weapons::FRAG].contains(&key) {
            continue;
        }
        seen.push(e);
        let at = at.translation();
        let t = tracks.entry(e).or_insert_with(|| Track { at, seen: now, carry: 0.0 });
        let (before, moved) = (t.at, at - t.at);
        t.at = at;
        match key {
            MISSILE_MODEL => rocket(&mut frame, &mut smoke, &mut rng.0, &m, at, moved, &mut t.carry),
            BALL_MODEL => ball(&mut frame, &mut rng.0, &m, at, moved, t.seen, now, held),
            BOLT_MODEL => bolt(&mut frame, &m, at, moved),
            SMG1_GRENADE_MODEL => {
                let v = velocity.map_or(Vec3::ZERO, |v| v.0);
                smoke_trail(&mut grenade_smoke, &mut rng.0, &m, at, v, dt, now - t.seen, &mut t.carry)
            }
            SATCHEL_MODEL => glow_and_trail(&mut frame, &mut trail, &m, at, before, now - t.seen, true),
            _ => glow_and_trail(&mut frame, &mut trail, &m, at, before, now - t.seen, false),
        }
    }
    tracks.retain(|e, _| seen.contains(e));
    for g in [frame, smoke, grenade_smoke, trail] {
        if !g.particles.is_empty() {
            particles.add(g);
        }
    }
}

pub struct Hl2ProjectileFxPlugin;

impl Plugin for Hl2ProjectileFxPlugin {
    fn build(&self, app: &mut App) {
        use chimera_net::net::parts::NetAppExt;
        app.init_resource::<FxRng>()
            .init_resource::<Rings>()
            .init_resource::<Pins>()
            .register_type::<Hl2BallHeld>()
            .register_type::<Hl2MineBeam>()
            .register_type::<Hl2FlyerVelocity>()
            .add_message::<SatchelPop>()
            .add_message::<FlyerFx>()
            .add_message::<DamageTaken>()
            .add_message::<PlaceDecal>()
            .add_systems(
                FixedUpdate,
                (
                    (explosions, shooter_fades, damage_flashes).after(crate::core::SimSet::Weapons),
                    (scorches, flyer_fx, mark_for_clients)
                        .after(crate::core::SimSet::Weapons)
                        .run_if(crate::core::authoritative),
                ),
            )
            .net_part("hl2dm_projectile_fx", register)
            .add_systems(
                Update,
                (pops, (draw_flyer_fx, (draw_rings, pin_ragdolls)).chain(), projectile_looks, mine_beams)
                    .after(ParticleSet::Step)
                    .before(ParticleSet::Draw),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn who_draws_what() {
        assert!(look_of(weapons::FRAG).is_some_and(|l| l.core && !l.entity_rule));
        assert!(look_of(weapons::RPG).is_some_and(|l| l.entity_rule));
        assert!(look_of(weapons::AR2).is_none());
    }

    /// Spec test 41 and 42: a rocket on a wall well above the floor finds
    /// nothing and is drawn at its own point; on the floor it is drawn 24
    /// units up.
    #[test]
    fn the_entity_rule_draws_off_the_surface() {
        let at = Vec3::new(0.0, 100.0 * U, 0.0);
        assert_eq!(entity_rule_point(at, None), at);
        let floor = Vec3::ZERO;
        let p = entity_rule_point(Vec3::new(0.0, 0.5 * U, 0.0), Some((floor, Vec3::Y)));
        assert!((p.y - 24.0 * U).abs() < 1e-6);
    }

    #[test]
    fn the_rings() {
        let [a, b] = ball_rings(Vec3::ZERO, 0.0);
        assert!((a.life - 0.2).abs() < 1e-6 && (b.life - 0.5).abs() < 1e-6);
        assert!((a.to - 1024.0 * U).abs() < 1e-4 && (b.alpha - 64.0 / 255.0).abs() < 1e-6);
    }

    /// particles_and_smoke.md 5 ("What V is on a client"): the contact
    /// grenade's puffs spread over O − V·Δt … O and each takes V on top of
    /// its 4-24 units/s drift.
    #[test]
    fn grenade_smoke_takes_the_snapshot_velocity() {
        let mut g = ParticleGroup::new(Motion::default());
        let mut rng = ParticleRng::new(3);
        let m = |_: &str| Some(0usize);
        let (at, v, dt) = (Vec3::new(1.0, 2.0, 3.0), Vec3::new(1000.0 * U, 0.0, 0.0), 0.1);
        let mut carry = 0.0;
        smoke_trail(&mut g, &mut rng, &m, at, v, dt, 0.0, &mut carry);
        assert!(g.particles.len() >= 4, "48 a second: {}", g.particles.len());
        let jitter = 4.0 * U * 3f32.sqrt() + 1e-4;
        for p in &g.particles {
            // On the segment from O − V·Δt to O, give or take the jitter.
            let x = p.position.x;
            assert!(x >= at.x - v.x * dt - jitter && x <= at.x + jitter, "{x}");
            assert!((p.velocity - v).length() <= 24.0 * U * 3f32.sqrt() + 1e-4, "{}", p.velocity);
        }
    }
}
