//! HL2:DM's projectile effects (specs/hl2dm/effects_sounds.md 8, 9, the
//! Explosions and Projectile visuals tables; specs/hl2dm/projectiles.md
//! 6.1, 9.5), drawn with Chimera's particles (`map::particles`):
//!
//! - **Explosions** (`explosions`, on `Detonated`): the temp entity's
//!   look (`source_engine::effects::explosions`: CS:S's smoke, embers,
//!   fireballs, sparks and flecks, the same effect in every Source game)
//!   and its sound `BaseExplosionEffect.Sound`, heard by each client where
//!   it is. The RPG's, satchel's and trip mine's go by the explosion
//!   entity's rule: a line from 8 above the point to 40 below that, drawn
//!   24 off what it hits (and where it hits nothing, at the point itself),
//!   and the server puts a `Scorch` where it hits (`scorches`; Chimera's
//!   decals reach the clients).
//! - **The shot satchel's pop** (`SatchelPop`, the server's, sent to
//!   clients too: net part `hl2dm_projectile_fx`): a scale-0 explosion,
//!   the flash and the debris (`explosions::pop`) and its sound.
//! - **The rocket** (`MISSILE_MODEL`, every frame): 8 flare sprites behind
//!   it and a smoke puff for each 4 units it travelled.
//! - **The energy ball** (`BALL_MODEL`, every frame): the animated body
//!   quad, the flickering halo and 8 blur halos behind; its bounce
//!   (`cball_bounce`: two flat quads and sparks) and its end (sparks and
//!   two rings of `sprites/lgtning` spreading to 1024 units).
//! - **The crossbow bolt** (`BOLT_MODEL`): 20 blur halos along its motion;
//!   stuck in the world, a glow fading over 3 s and sparks.
//! - **The SMG1 grenade** (`SMG1_GRENADE_MODEL`): `env_smoketrail`'s smoke.
//! - **The frag and the satchel**: a red glow (the frag's beats faster
//!   once warned) and the frag's red trail.
//! - **The RPG's laser dot**: a glow quad at the end of a trace from the
//!   current view (the owner's own), or at the server's dot (others');
//!   its sounds are the weapon's (H4).
//!
//! Every per-frame look is a one-frame group (`frame_group`) added after
//! the particles' step and before their drawing, from what the shown
//! projectile entities (`ShownItem`, on the host and on clients alike)
//! are doing; the per-event ones read the host's `FlyerHit`/`FlyerGone`
//! and every machine's `Detonated`.
//!
//! Known shortcuts (docs/tech-debt.md): the ball's bounce and end and the
//! stuck bolt's glow are drawn from the host's flyer messages, so a
//! client of a dedicated server sees the ball, the bolt's blur and the
//! trails but not those three; the RPG's beam and glow on the view model
//! (re-projected attachments) are not drawn, nor the held ball's model;
//! the ball's body quad takes its frames from the material's sheet at 30 a
//! second; the electric sparks are the surface effect's (sizes 1, 2 and 4
//! of the spec are one size); the frag's trail has one width; the scorch
//! lands on the world only (not on props or brush entities under the
//! blast); the satchel's glow
//! strobes at a steady rate. Why it's OK for now: the shapes, sizes,
//! lifetimes and counts are the spec's; the look needs the real game's
//! captures to compare.

use std::collections::BTreeMap;

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use chimera_sim::weapon::{
    Inventory, Weapon,
    grenade::Detonated,
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
        rpg::Hl2LaserDot,
        slam::SatchelPop,
    },
};
use crate::{
    core::{Intent, LocalPlayer, MovementState},
    map::{
        DecalGroup, ParticleRng, PlaceDecal, PlaySound,
        loose::ShownItem,
        particles::{
            Fade, Motion, Particle, ParticleGroup, ParticleMaterials, ParticleSet, Particles, Ramp, Shape,
            WorldTrace, WorldTracer,
        },
    },
    weapon::aim_of,
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
    world: WorldTracer,
) {
    let Some(materials) = materials else {
        detonated.clear();
        return;
    };
    let mats = FxMaterials::new(&materials.0);
    for d in detonated.read() {
        let Some(look) = look_of(d.weapon) else { continue };
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
            for g in explosions::explosion(&mut rng.0, &mats, at, a, l, &world) {
                particles.add(g);
            }
        } else if let Some(g) = flash_overlay(&mats, at) {
            particles.add(g);
        }
        play.write(PlaySound::at(EXPLOSION_SOUND, at));
    }
}

/// The line of the explosion entity's rule under `at` (spec 8): from 8
/// units above it to 40 below that; what it hits (point, normal).
pub fn entity_rule_hit(at: Vec3, world: &impl WorldTrace) -> Option<(Vec3, Vec3)> {
    let from = at + Vec3::Y * 8.0 * U;
    let to = from - Vec3::Y * 40.0 * U;
    world.trace(from, to).map(|(f, n)| (from + (to - from) * f, n))
}

/// The server's: an explosion by the entity rule (the RPG's, a satchel's,
/// a trip mine's) leaves a `Scorch` where its line hits (spec 8; none in
/// mid-air or on a wall well above the floor). Decals reach clients
/// through Chimera's.
fn scorches(mut detonated: MessageReader<Detonated>, world: WorldTracer, mut decals: MessageWriter<PlaceDecal>) {
    for d in detonated.read() {
        if !look_of(d.weapon).is_some_and(|l| l.entity_rule) {
            continue;
        }
        if let Some((point, normal)) = entity_rule_hit(d.at, &world) {
            decals.write(PlaceDecal {
                target: None,
                group: DecalGroup::Named(weapons::slam::SCORCH.into()),
                point,
                normal,
                dir: -normal,
                spin: true,
                round: false,
            });
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

/// The protocol: shot satchels' pops, from the server (a client's own
/// `SatchelPop` messages, which `pops` draws).
fn register(app: &mut App) {
    app.add_server_message::<SatchelPop>(Channel::Unreliable).add_systems(
        PostUpdate,
        forward_pops.before(ServerSystems::Send).run_if(resource_equals(crate::core::NetRole::Server)),
    );
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
// The ball's bounce and end, the bolt's sticking (the host's flyer messages)

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

#[allow(clippy::too_many_arguments)]
fn flyer_events(
    mut hits: MessageReader<FlyerHit>,
    mut gone: MessageReader<FlyerGone>,
    flyers: Query<&Flyer>,
    materials: Option<Res<ParticleMaterials>>,
    time: Res<Time>,
    mut particles: ResMut<Particles>,
    mut rings: ResMut<Rings>,
    mut rng: ResMut<FxRng>,
    world: WorldTracer,
) {
    let Some(materials) = materials else {
        hits.clear();
        gone.clear();
        return;
    };
    let ids = MaterialIds::new(&materials.0);
    let effect = EffectMaterials::new(&materials.0);
    let now = time.elapsed_secs_f64();
    for h in hits.read() {
        if h.weapon == weapons::AR2 && h.world {
            for g in ball_bounce(&ids, &mut rng.0, h.point, h.normal) {
                particles.add(g);
            }
            for g in impact_effects::electric_sparks(&mut rng.0, &effect, h.point, h.normal, &world) {
                particles.add(g);
            }
        }
        if h.weapon == weapons::CROSSBOW && h.world && flyers.get(h.flyer).is_ok_and(|f| f.stuck_until.is_some()) {
            if let Some(g) = bolt_glow(&ids, h.point) {
                particles.add(g);
            }
            for g in impact_effects::electric_sparks(&mut rng.0, &effect, h.point, h.normal, &world) {
                particles.add(g);
            }
        }
    }
    for g in gone.read() {
        if g.weapon == weapons::AR2 && g.expired {
            rings.0.extend(ball_rings(g.at, now));
            for p in impact_effects::electric_sparks(&mut rng.0, &effect, g.at, Vec3::Y, &world) {
                particles.add(p);
            }
        }
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

/// The ball's look this frame (spec 9): the body quad of half-size
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
) {
    let r = 10.0;
    if let Some(body) = m("effects/ar2_altfire1") {
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

/// The contact grenade's `env_smoketrail` (spec 9): 48 puffs a second for
/// 10 s, life 1 s, colour from 0.1 grey to black, size 12 to 48 units,
/// opacity 0.2, spawned within 4 units drifting 4 to 24.
fn smoke_trail(
    smoke: &mut ParticleGroup,
    rng: &mut ParticleRng,
    m: &impl Fn(&str) -> Option<usize>,
    at: Vec3,
    dt: f32,
    age: f64,
    carry: &mut f32,
) {
    if age > 10.0 {
        return;
    }
    let Some(mat) = m("particle/particle_smokegrenade") else { return };
    *carry += 48.0 * dt;
    let n = *carry as usize;
    *carry -= n as f32;
    for _ in 0..n.min(8) {
        smoke.particles.push(Particle {
            velocity: random_dir(rng) * rng.float(4.0, 24.0) * U,
            color: Vec3::splat(0.1),
            color_fade: Some((Vec3::ZERO, 0.0, 1.0)),
            alpha: 0.2,
            fade: Fade::Ramp(Ramp::linear(1.0, 0.0)),
            size: Ramp::linear(12.0 * U, 48.0 * U),
            roll: (rng.int(0, 360) as f32).to_radians(),
            ..Particle::new(at + random_dir(rng) * rng.float(0.0, 4.0) * U, 1.0, mat, 0.0)
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
    items: Query<(Entity, &ShownItem, &GlobalTransform)>,
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
    let mut seen = Vec::new();
    for (e, shown, at) in &items {
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
            BALL_MODEL => ball(&mut frame, &mut rng.0, &m, at, moved, t.seen, now),
            BOLT_MODEL => bolt(&mut frame, &m, at, moved),
            SMG1_GRENADE_MODEL => smoke_trail(&mut smoke, &mut rng.0, &m, at, dt, now - t.seen, &mut t.carry),
            SATCHEL_MODEL => glow_and_trail(&mut frame, &mut trail, &m, at, before, now - t.seen, true),
            _ => glow_and_trail(&mut frame, &mut trail, &m, at, before, now - t.seen, false),
        }
    }
    tracks.retain(|e, _| seen.contains(e));
    for g in [frame, smoke, trail] {
        if !g.particles.is_empty() {
            particles.add(g);
        }
    }
}

// ---------------------------------------------------------------------------
// The RPG's laser dot

/// Where the dot is drawn, 4 units off the surface along its normal, with
/// a width of 16 ± 4 units (spec 9).
fn dot(g: &mut ParticleGroup, rng: &mut ParticleRng, mat: usize, point: Vec3, normal: Vec3) {
    let half = (16.0 + rng.float(-4.0, 4.0)) / 2.0;
    g.particles.push(flash(mat, point + normal * 4.0 * U, half, 1.0, 1.0));
}

/// The owner's own dot from the current view (never lagging the
/// crosshair): a trace from the eye along the view, 56755.84 units; the
/// server's dot for everyone else's RPG.
#[allow(clippy::too_many_arguments)]
fn laser_dots(
    local: Query<(Entity, &Transform, &MovementState, &Intent, &Inventory), With<LocalPlayer>>,
    weapons_q: Query<(&Weapon, Option<&weapons::launch::Hl2Rounds>)>,
    dots: Query<(&Weapon, &Hl2LaserDot)>,
    flyers: Query<&Flyer>,
    materials: Option<Res<ParticleMaterials>>,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<FxRng>,
    world: WorldTracer,
) {
    let Some(mat) = materials.as_ref().and_then(|m| m.0.find("sprites/redglow1")) else { return };
    let mut g = frame_group();
    let mut mine = None;
    for (owner, at, state, intent, inv) in &local {
        let Some((w, rounds)) = inv.active.and_then(|a| weapons_q.get(a).ok()) else { continue };
        let missile = flyers.iter().any(|f| f.weapon == weapons::RPG && f.owner == Some(owner));
        if w.id != weapons::RPG || !(rounds.is_some_and(|r| r.count > 0) || missile) {
            continue;
        }
        mine = Some(owner);
        let eye = at.translation + state.eye_offset;
        let dir = aim_of(intent.yaw, intent.pitch) * Vec3::NEG_Z;
        let far = eye + dir * 56755.84 * U;
        if let Some((f, n)) = world.trace(eye, far) {
            dot(&mut g, &mut rng.0, mat, eye + (far - eye) * f, n);
        }
    }
    for (w, d) in &dots {
        if w.owner.is_some() && w.owner == mine {
            continue;
        }
        dot(&mut g, &mut rng.0, mat, d.at, d.normal);
    }
    if !g.particles.is_empty() {
        particles.add(g);
    }
}

pub struct Hl2ProjectileFxPlugin;

impl Plugin for Hl2ProjectileFxPlugin {
    fn build(&self, app: &mut App) {
        use chimera_net::net::parts::NetAppExt;
        app.init_resource::<FxRng>()
            .init_resource::<Rings>()
            .add_message::<SatchelPop>()
            .add_message::<PlaceDecal>()
            .add_systems(
                FixedUpdate,
                (
                    (explosions, flyer_events).after(crate::core::SimSet::Weapons),
                    scorches.after(crate::core::SimSet::Weapons).run_if(crate::core::authoritative),
                ),
            )
            .add_systems(Update, pops.after(ParticleSet::Step).before(ParticleSet::Draw))
            .net_part("hl2dm_projectile_fx", register)
            .add_systems(
                Update,
                (projectile_looks, laser_dots, draw_rings)
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
}
