//! The dissolve of a player the energy ball kills (specs/hl2dm/
//! effects_sounds.md 10, Constants "Dissolve"; the kill's side: projectiles.md
//! 5.3): the victim is marked `Dissolving` (it has no death voice,
//! `sounds`), its ragdoll takes a `Dissolve`, and for 2 s (134 ticks) the
//! body
//!
//! - darkens from white to black over the first second and fades out over
//!   the last 0.1 s (`look_at`: `RenderLook` on the ragdoll's drawn body),
//! - floats: every body gets 1.02 gravity upward, so it drifts up at
//!   0.02 g, and is slowed by up to 40 units/s a step toward 50 units/s
//!   (`lift`, `capped`),
//! - throws sparks and glows around every body part each frame (2 tries
//!   at 1/3 of an `effects/spark`; ⌊3p⌋ `effects/combinemuzzle2` glows, p
//!   the darkening's progress), shifted toward the camera by 8 − 32 (1 − p)
//!   units,
//!
//! and is gone at the 134th tick after the kill (`despawn_ragdoll`).
//!
//! The weapon the victim drops dissolves too (`dissolve_items`, the
//! server's): its loose entity loses `Loose` (nobody can take it) and is
//! marked `Hl2DissolvingItem` (replicated: net part `hl2dm_dissolve`);
//! every machine darkens and fades it as the body and throws the same
//! sparks and glows about its centre (`item_looks`, timed from when that
//! machine first sees it), with no float; it and its weapon are gone at
//! the 134th tick.
//!
//! Known shortcuts (docs/tech-debt.md): a client's dissolving weapon is
//! timed from when the server's mark reaches it (a latency late); the sparks are spread around
//! each body part's centre within 12 units across the screen, not by its
//! hitbox's extents; the ragdoll's drawn body takes its colour from
//! `RenderLook` (the characters' look), which fades a ragdoll adopted by
//! the render layer only. Why it's OK for now: the timing, the colour
//! curve, the float and the counts are the spec's.

use avian3d::prelude::{Gravity, LinearVelocity};
use bevy::prelude::*;

use chimera_sim::weapon::drop::Loose;

use super::{sounds::Dissolving, weapons};
use crate::{
    core::{DamageKind, Died, Health, SimClock},
    map::{
        ParticleRng, RenderLook,
        particles::{Fade, Motion, Particle, ParticleGroup, ParticleMaterials, ParticleSet, Particles, Ramp},
        ragdoll::{Ragdoll, RagdollBody, despawn_ragdoll, spawn_ragdolls},
    },
};

const U: f32 = 0.0254;

/// The dissolve's length in ticks (spec 10: the first tick at or after
/// 2.0 s), and in seconds.
pub const TICKS: u64 = 134;
pub const SECONDS: f32 = 2.0;
/// The darkening's length and when the fade out starts, seconds.
pub const DARKEN: f32 = 1.0;
pub const FADE_FROM: f32 = 1.9;
/// A body at most this fast (units/s), slowed by up to `CAP_STEP` a step.
pub const SPEED_CAP: f32 = 50.0;
pub const CAP_STEP: f32 = 40.0;
/// Upward acceleration, in gravities.
pub const LIFT: f32 = 1.02;

/// Whether `damage` kills by dissolving: the energy ball's touch (an
/// AR2's flyer, crush).
pub fn dissolves(weapon: Option<&str>, kind: DamageKind) -> bool {
    weapon == Some(weapons::AR2) && kind == DamageKind::Crush
}

/// A ragdoll being dissolved: since when (the kill's tick and the
/// render clock).
#[derive(Component, Clone, Copy, Debug)]
pub struct Dissolve {
    pub tick: u64,
    pub start: f64,
}

/// The body's colour and alpha `t` seconds in (spec 10, tests 10): rgb
/// (1 − t) over the first second, alpha 255 until 1.9 s then falling to 0
/// at 2.0.
pub fn look_at(t: f32) -> RenderLook {
    let c = ((1.0 - (t / DARKEN).clamp(0.0, 1.0)) * 255.0).round() as u8;
    let a = if t < FADE_FROM { 1.0 } else { (1.0 - (t - FADE_FROM) / (SECONDS - FADE_FROM)).clamp(0.0, 1.0) };
    RenderLook {
        color: [c; 3],
        alpha: (a * 255.0).round() as u8,
        mode: if a < 1.0 { crate::map::RENDER_TRANS_ALPHA } else { crate::map::RENDER_NORMAL },
    }
}

/// A body's speed after one physics step of the cap (meters/s; spec 10:
/// 130 units/s gives 90, then 50, then stays).
pub fn capped(v: Vec3) -> Vec3 {
    let (cap, step) = (SPEED_CAP * U, CAP_STEP * U);
    let speed = v.length();
    if speed <= cap {
        return v;
    }
    v * ((speed - step.min(speed - cap)) / speed)
}

/// The upward speed gained in a step of `dt` seconds at `gravity` (m/s²).
pub fn lift(gravity: f32, dt: f32) -> f32 {
    LIFT * gravity * dt
}

/// Mark the victims of the ball (before their death voice is chosen) and
/// let a respawned one die with a voice again.
pub fn mark_dissolved(
    mut died: MessageReader<Died>,
    alive: Query<&Health>,
    mut marked: Local<Vec<Entity>>,
    mut commands: Commands,
) {
    for d in died.read() {
        if dissolves(d.damage.weapon, d.damage.kind) && d.entity != Entity::PLACEHOLDER {
            commands.entity(d.entity).insert(Dissolving);
            marked.push(d.entity);
        }
    }
    // One of ours that lives again dies with a voice next time.
    marked.retain(|e| match alive.get(*e) {
        Ok(h) if h.current > 0.0 => {
            commands.entity(*e).remove::<Dissolving>();
            false
        }
        Ok(_) => true,
        Err(_) => false,
    });
}

/// A new ragdoll of a dissolving victim takes its `Dissolve`.
fn start_dissolves(
    ragdolls: Query<(Entity, &Ragdoll), Without<Dissolve>>,
    marked: Query<(), With<Dissolving>>,
    clock: Res<SimClock>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for (e, r) in &ragdolls {
        if marked.contains(r.owner) {
            commands.entity(e).insert(Dissolve { tick: clock.tick, start: time.elapsed_secs_f64() });
        }
    }
}

/// The float and the end: each tick every body of a dissolving ragdoll
/// gains its lift and is slowed toward the cap; at `TICKS` it is removed.
fn dissolve_ragdolls(
    ragdolls: Query<(Entity, &Ragdoll, &Dissolve)>,
    mut bodies: Query<&mut LinearVelocity, With<RagdollBody>>,
    gravity: Option<Res<Gravity>>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    let dt = clock.delta.as_secs_f32();
    let g = gravity.map_or(600.0 * U, |g| g.0.length());
    for (e, r, d) in &ragdolls {
        if clock.tick >= d.tick + TICKS {
            despawn_ragdoll(&mut commands, e, r);
            continue;
        }
        for b in &r.bodies {
            let Ok(mut v) = bodies.get_mut(*b) else { continue };
            v.0.y += lift(g, dt);
            v.0 = capped(v.0);
        }
    }
}

/// One frame's spark tries and glows around `centre` (a body part's or a
/// dropped weapon's) at the darkening's progress `p`, spread across the
/// screen and shifted toward the camera `cam` (its place, right and up).
fn sparks_around(
    g: &mut ParticleGroup,
    rng: &mut ParticleRng,
    spark: Option<usize>,
    glow: Option<usize>,
    centre: Vec3,
    cam: Option<(Vec3, Vec3, Vec3)>,
    p: f32,
) {
    let (towards, right, up) = match cam {
        Some((c, right, up)) => ((c - centre).normalize_or_zero(), right, up),
        None => (Vec3::ZERO, Vec3::X, Vec3::Y),
    };
    let place = |rng: &mut ParticleRng| {
        centre
            + (right * rng.float(-1.0, 1.0) + up * rng.float(-1.0, 1.0)) * 12.0 * U * 0.75
            + towards * (8.0 - 32.0 * (1.0 - p)) * U
    };
    if let Some(spark) = spark {
        for _ in 0..2 {
            if rng.float(0.0, 1.0) < 1.0 / 3.0 {
                let life = rng.float(0.4, 0.5);
                g.particles.push(Particle {
                    velocity: Vec3::new(rng.float(-4.0, 4.0), rng.float(16.0, 64.0), rng.float(-4.0, 4.0)) * U,
                    fade: Fade::Ramp(Ramp::linear(1.0, 0.0)),
                    size: Ramp::linear(rng.float(4.0, 6.0) * U, 0.0),
                    roll_speed: rng.float(-8.0, 8.0),
                    ..Particle::new(place(rng), life, spark, 0.0)
                });
            }
        }
    }
    if let Some(glow) = glow {
        for _ in 0..((3.0 * p) as usize) {
            g.particles.push(Particle {
                velocity: Vec3::new(rng.float(-4.0, 4.0), rng.float(-64.0, 128.0), rng.float(-4.0, 4.0)) * U,
                roll_speed: rng.float(-2.0, 2.0),
                ..Particle::new(place(rng), 0.1, glow, rng.float(8.0, 12.0) * U)
            });
        }
    }
}

/// The body's colour and the sparks, each frame.
#[allow(clippy::too_many_arguments)]
fn dissolve_looks(
    ragdolls: Query<(&Ragdoll, &Dissolve)>,
    bodies: Query<&GlobalTransform, With<RagdollBody>>,
    camera: Query<&GlobalTransform, With<Camera3d>>,
    materials: Option<Res<ParticleMaterials>>,
    time: Res<Time>,
    mut particles: ResMut<Particles>,
    mut rng: Local<Option<ParticleRng>>,
    mut commands: Commands,
) {
    let rng = rng.get_or_insert_with(|| ParticleRng::new(0xD155_01));
    let now = time.elapsed_secs_f64();
    let spark = materials.as_ref().and_then(|m| m.0.find("effects/spark"));
    let glow = materials.as_ref().and_then(|m| m.0.find("effects/combinemuzzle2"));
    let cam = camera.iter().next().map(|c| (c.translation(), c.right().as_vec3(), c.up().as_vec3()));
    let mut g = ParticleGroup::new(Motion::default());
    g.skip_first = false;
    g.capped = false;
    for (r, d) in &ragdolls {
        let t = (now - d.start) as f32;
        if let Some(v) = r.visual {
            if let Ok(mut e) = commands.get_entity(v) {
                e.insert(look_at(t));
            }
        }
        let p = (t / DARKEN).clamp(0.0, 1.0);
        for body in &r.bodies {
            let Ok(at) = bodies.get(*body) else { continue };
            sparks_around(&mut g, rng, spark, glow, at.translation(), cam, p);
        }
    }
    if !g.particles.is_empty() {
        particles.add(g);
    }
}

// ---------------------------------------------------------------------------
// The victim's dropped weapon

/// A dissolving dropped weapon's loose entity (the server's, replicated):
/// nobody can take it, it darkens and fades as a ragdoll does (no float)
/// and is gone at the 134th tick.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, serde::Serialize, serde::Deserialize)]
#[reflect(Component)]
pub struct Hl2DissolvingItem;

/// The server's: when the dissolve started and the weapon the loose
/// entity stands for.
#[derive(Component, Clone, Copy, Debug)]
pub struct ItemDissolve {
    pub tick: u64,
    pub weapon: Entity,
}

/// The server's: a weapon a dissolving victim dropped can't be taken
/// (its `Loose` goes) and dissolves; at `TICKS` it and its weapon go.
fn dissolve_items(
    dropped: Query<(Entity, &Loose), Added<Loose>>,
    dissolving: Query<(Entity, &ItemDissolve)>,
    marked: Query<(), With<Dissolving>>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    for (e, l) in &dropped {
        if l.dropper.is_some_and(|d| marked.contains(d)) {
            commands
                .entity(e)
                .remove::<Loose>()
                .insert((Hl2DissolvingItem, ItemDissolve { tick: clock.tick, weapon: l.weapon }));
        }
    }
    for (e, d) in &dissolving {
        if clock.tick >= d.tick + TICKS {
            commands.entity(e).despawn();
            if d.weapon != e
                && let Ok(mut w) = commands.get_entity(d.weapon)
            {
                w.despawn();
            }
        }
    }
}

/// Each machine: a dissolving weapon's colour, and its sparks and glows
/// around its centre, from when this machine first saw it dissolving.
#[allow(clippy::too_many_arguments)]
fn item_looks(
    items: Query<(Entity, &GlobalTransform), With<Hl2DissolvingItem>>,
    camera: Query<&GlobalTransform, With<Camera3d>>,
    materials: Option<Res<ParticleMaterials>>,
    time: Res<Time>,
    mut particles: ResMut<Particles>,
    mut started: Local<std::collections::BTreeMap<Entity, f64>>,
    mut rng: Local<Option<ParticleRng>>,
    mut commands: Commands,
) {
    let rng = rng.get_or_insert_with(|| ParticleRng::new(0xD155_02));
    let now = time.elapsed_secs_f64();
    started.retain(|e, _| items.contains(*e));
    let spark = materials.as_ref().and_then(|m| m.0.find("effects/spark"));
    let glow = materials.as_ref().and_then(|m| m.0.find("effects/combinemuzzle2"));
    let cam = camera.iter().next().map(|c| (c.translation(), c.right().as_vec3(), c.up().as_vec3()));
    let mut g = ParticleGroup::new(Motion::default());
    g.skip_first = false;
    g.capped = false;
    for (e, at) in &items {
        let start = *started.entry(e).or_insert(now);
        let t = (now - start) as f32;
        commands.entity(e).insert(look_at(t));
        sparks_around(&mut g, rng, spark, glow, at.translation(), cam, (t / DARKEN).clamp(0.0, 1.0));
    }
    if !g.particles.is_empty() {
        particles.add(g);
    }
}

pub struct Hl2DissolvePlugin;

impl Plugin for Hl2DissolvePlugin {
    fn build(&self, app: &mut App) {
        use chimera_net::net::parts::NetAppExt;
        app.register_type::<Hl2DissolvingItem>()
            .add_systems(
                FixedUpdate,
                (
                    // The victim is marked, then its new ragdoll takes the dissolve.
                    (mark_dissolved, start_dissolves, dissolve_ragdolls).chain().after(spawn_ragdolls),
                    dissolve_items.after(dissolve_ragdolls).run_if(crate::core::authoritative),
                ),
            )
            .add_systems(
                Update,
                (dissolve_looks, item_looks).after(ParticleSet::Step).before(ParticleSet::Draw),
            )
            .net_part("hl2dm_dissolve", register);
    }
}

/// The protocol: the dissolving dropped weapons, from the server.
fn register(app: &mut App) {
    use bevy_replicon::prelude::AppRuleExt;
    use chimera_net::net::ordered::OrderedAppExt;
    app.replicate::<Hl2DissolvingItem>().ordered::<Hl2DissolvingItem>();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec test 10: 0.5 s, 1.5 s, 1.95 s.
    #[test]
    fn the_body_darkens_then_fades() {
        let l = look_at(0.5);
        assert!(l.color == [128; 3] || l.color == [127; 3]);
        assert_eq!(l.alpha, 255);
        let l = look_at(1.5);
        assert_eq!((l.color, l.alpha), ([0; 3], 255));
        let l = look_at(1.95);
        assert!(l.color == [0; 3] && (l.alpha == 127 || l.alpha == 128), "{l:?}");
        assert_eq!(look_at(2.0).alpha, 0);
    }

    /// Spec test 12: 130 units/s becomes 90, then 50, and stays.
    #[test]
    fn the_speed_cap() {
        let v = capped(Vec3::X * 130.0 * U);
        assert!((v.length() / U - 90.0).abs() < 1e-3);
        let v = capped(v);
        assert!((v.length() / U - 50.0).abs() < 1e-3);
        assert!((capped(v).length() / U - 50.0).abs() < 1e-3);
        assert_eq!(capped(Vec3::X), Vec3::X * 1.0_f32.min(SPEED_CAP * U).min(1.0));
    }

    /// Spec test 11: at gravity 600 a second of lift nets +12 units/s.
    #[test]
    fn the_float() {
        let g = 600.0 * U;
        let net = lift(g, 1.0) - g;
        assert!((net / U - 12.0).abs() < 1e-3);
    }

    #[test]
    fn only_the_ball_dissolves() {
        assert!(dissolves(Some(weapons::AR2), DamageKind::Crush));
        assert!(!dissolves(Some(weapons::AR2), DamageKind::Bullet));
        assert!(!dissolves(Some(weapons::RPG), DamageKind::Blast));
        assert!(!dissolves(None, DamageKind::Crush));
    }
}
