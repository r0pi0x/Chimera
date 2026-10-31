//! Drawing the view model's attached effects (specs/combat_arms/
//! view_models.md 3; `game::view_model::attached`): each shown group of
//! the local player's view model, camera-relative in the view-model pass,
//! its keys at their sockets or nodes of the posed model
//! (`attached::key_place`):
//!
//! - `Sprite`: one camera-facing quad of the sprite's first frame,
//!   texture pixels × scale wide (half, facing the camera, as in the
//!   world), its scale and colour keys over the key's life (a key lasting
//!   the whole group repeats them: effects_sounds.md "Key curves");
//! - `ParticleSystem`: particles of its own, in the view-model camera's
//!   space: emitted every `EmissionInterval` (≤ 0: one burst) at the
//!   key's place by `Type` and radius, velocity box, life, gravity,
//!   friction and spin; each a camera-facing quad of side 4 s (s its
//!   scale keys over its life), colour and alpha by its colour keys.
//!
//! A group that hides clears its particles; a new draw starts them
//! afresh. The effect file's detail levels apply (`CaFx::detail`).
//!
//! Known shortcuts (docs/tech-debt.md): particles move in the view
//! model's camera space (as if `MoveParticlesWithSystem`; gravity along
//! the camera's up), so they turn with the view; `DynaLight`,
//! `LightningFX`, `LTBModel`, `PolyTrail` and `CamJitter` keys of
//! attached groups draw nothing; cone and cylinder emitters are circles;
//! keys aren't depth-sorted (additive keys don't care).

use std::collections::HashMap;

use bevy::{camera::visibility::RenderLayers, prelude::*};
use lithtech_formats::fxf::{FxKey, sample_keys};

use super::effects::FxImages;
use crate::{
    core::LocalPlayer,
    game::{
        effects::{
            fx::CaFx,
            library::{self, additive, shaded},
        },
        view_model::{
            VIEW_SCALE,
            attached::{CaAttachedFx, key_place},
        },
    },
    map::{ViewAnimator, ViewModelOffset, ViewModels, view_model::{VIEW_MODEL_LAYER, ViewModelCamera}},
    weapon::Inventory,
};

/// One quad of the pool.
#[derive(Component)]
pub struct AttachedQuad(usize);

/// A particle of an attached particle system (view-model camera space,
/// metres).
#[derive(Clone, Debug)]
struct VmParticle {
    effect: usize,
    key: usize,
    at: Vec3,
    velocity: Vec3,
    age: f32,
    life: f32,
    roll: f32,
    spin: f32,
}

/// What the effects keep between frames.
#[derive(Default)]
pub struct AttachedState {
    weapon: Option<Entity>,
    /// Per (effect, key): the next emission's group age, or `None` once a
    /// burst has gone.
    emit: HashMap<(usize, usize), Option<f32>>,
    particles: Vec<VmParticle>,
    rng: u32,
    /// Per effect: the model's placement when it was made (where its
    /// player-view keys stay).
    made: HashMap<usize, ViewModelOffset>,
}

impl AttachedState {
    fn random(&mut self) -> f32 {
        let mut x = self.rng.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.random()
    }
}

/// One quad to draw this frame.
struct Item {
    at: Vec3,
    size: f32,
    roll: f32,
    color: Vec4,
    texture: Option<String>,
    additive: bool,
}

/// Whether key `k` of a group `length` s long repeats (it lasts the whole
/// group, effects_sounds.md "The effect file").
fn loops(k: &FxKey, length: f32) -> bool {
    k.end - k.start >= length - 0.01
}

/// The key's curves' time 0..1 at group age `age`.
fn key_t(k: &FxKey, age: f32, length: f32) -> Option<f32> {
    let life = (k.end - k.start).max(1e-3);
    let a = age - k.start;
    if a < 0.0 {
        return None;
    }
    if loops(k, length) {
        Some((a / life).rem_euclid(1.0))
    } else if a <= life {
        Some(a / life)
    } else {
        None
    }
}

fn color_at(keys: &[(f32, [f32; 4])], t: f32) -> Vec4 {
    let ch = |i: usize| sample_keys(&keys.iter().map(|(k, c)| (*k, c[i])).collect::<Vec<_>>(), t, 1.0);
    Vec4::new(ch(0), ch(1), ch(2), ch(3))
}

/// Unit-length random direction.
fn random_dir(state: &mut AttachedState) -> Vec3 {
    loop {
        let v = Vec3::new(state.range(-1.0, 1.0), state.range(-1.0, 1.0), state.range(-1.0, 1.0));
        let l = v.length_squared();
        if l > 1e-4 && l <= 1.0 {
            return v / l.sqrt();
        }
    }
}

/// One emission of key `k` (effect `effect`, key index `ki`) at `place`.
fn emit(state: &mut AttachedState, k: &FxKey, effect: usize, ki: usize, place: Mat4) {
    let n = k.int("ParticlesPerEmission").unwrap_or(1).clamp(0, 64) as usize;
    let (r0, r1) = (k.float("MinRadius").unwrap_or(0.0), k.float("MaxRadius").unwrap_or(0.0));
    let plane = Vec3::from(k.vector("EmissionPlane").unwrap_or([0.0, 0.0, 1.0])).normalize_or(Vec3::Z);
    let (v0, v1) = (
        Vec3::from(k.vector("MinParticleVelocity").unwrap_or_default()),
        Vec3::from(k.vector("MaxParticleVelocity").unwrap_or_default()),
    );
    let (l0, l1) = (
        k.float("MinParticleLifeSpan").unwrap_or(1.0),
        k.float("MaxParticleLifeSpan").unwrap_or(1.0),
    );
    let spin = k.combo("RotateParticles") == Some(1);
    let (w0, w1) = (
        k.float("MinAngularVelocity").unwrap_or(0.0).to_radians(),
        k.float("MaxAngularVelocity").unwrap_or(0.0).to_radians(),
    );
    // The game's axes (left-handed) into the view camera's.
    let flip = |v: Vec3| Vec3::new(v.x, v.y, -v.z);
    for _ in 0..n {
        let r = state.range(r0, r1);
        let local = match k.combo("Type").unwrap_or(1) {
            0 => random_dir(state) * r,
            1 => Vec3::ZERO,
            _ => {
                let a = state.range(0.0, std::f32::consts::TAU);
                let (px, py) = plane.any_orthonormal_pair();
                (px * a.cos() + py * a.sin()) * r
            }
        };
        let v = Vec3::new(state.range(v0.x, v1.x), state.range(v0.y, v1.y), state.range(v0.z, v1.z));
        let life = state.range(l0, l1).max(1e-3);
        let roll = if spin { state.range(0.0, std::f32::consts::TAU) } else { 0.0 };
        let w = if spin { state.range(w0, w1) * if state.random() < 0.5 { -1.0 } else { 1.0 } } else { 0.0 };
        state.particles.push(VmParticle {
            effect,
            key: ki,
            at: place.transform_point3(flip(local)),
            velocity: place.transform_vector3(flip(v)),
            age: 0.0,
            life,
            roll,
            spin: w,
        });
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn attached_effects(
    time: Res<Time>,
    player: Option<Single<(&CaAttachedFx, &Inventory, Option<&ViewAnimator>, Option<&ViewModelOffset>), With<LocalPlayer>>>,
    models: Option<Res<ViewModels>>,
    camera: Option<Single<Entity, With<ViewModelCamera>>>,
    fx: Option<Res<CaFx>>,
    mut quads: Query<(&AttachedQuad, &mut Transform, &mut Visibility, &MeshMaterial3d<StandardMaterial>)>,
    server: Res<AssetServer>,
    mut images: ResMut<FxImages>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut state: Local<AttachedState>,
    mut commands: Commands,
) {
    let now = time.elapsed_secs_f64();
    let dt = time.delta_secs();
    let detail = fx.as_ref().map_or(crate::game::effects::rules::DEFAULT_FX_DETAIL, |f| f.detail);
    let lib = library::library();
    let mut items: Vec<Item> = Vec::new();
    let shown = player.as_ref().and_then(|p| {
        let (attached, inv, view, offset) = (p.0, p.1, p.2, p.3);
        crate::game::view_model::attached::of_active(attached, inv)?;
        let view = view.filter(|v| !v.hidden)?;
        let model = models.as_deref()?.get(view.key.as_deref()?)?;
        let pose = view.animator.as_ref()?.pose(now);
        Some((attached.clone(), model, pose, offset.cloned().unwrap_or_default()))
    });
    match (shown, lib.as_deref()) {
        (Some((attached, model, pose, offset)), Some(lib)) => {
            if state.weapon != attached.weapon {
                state.weapon = attached.weapon;
                state.emit.clear();
                state.particles.clear();
                state.made.clear();
            }
            // Hidden groups: their particles go, their emitters restart
            // where they are when shown again.
            let hidden: Vec<usize> = attached.effects.iter().enumerate().filter(|(_, e)| !e.shown).map(|(i, _)| i).collect();
            state.particles.retain(|p| !hidden.contains(&p.effect));
            state.emit.retain(|(e, _), _| !hidden.contains(e));
            for (i, e) in attached.shown() {
                let Some(g) = lib.group(&e.group) else { continue };
                let length = g.length().max(1e-3);
                let age = (now - e.made) as f32;
                for (ki, k) in g.keys.iter().enumerate() {
                    if !k.at_detail(detail) || shaded(k) {
                        continue;
                    }
                    let Some(t) = key_t(k, age, length) else { continue };
                    let attach = k.string("AttachName");
                    let made = state.made.entry(i).or_insert_with(|| offset.clone()).clone();
                    let place = key_place(
                        model,
                        &pose,
                        &offset,
                        attach,
                        Vec3::from(k.vector("Offset").unwrap_or_default()),
                        k.combo("UpdatePos"),
                        &made,
                    );
                    match k.kind.as_str() {
                        "Sprite" => {
                            let Some(sprite) = k.path("Sprite") else { continue };
                            let tex = lib.texture_of(k);
                            let px = tex.as_deref().and_then(|t| lib.texture_size(t)).map_or(64.0, |s| s.x as f32);
                            let s = sample_keys(&k.scale_keys(), t, 1.0);
                            let along = k.combo("Facing") == Some(1);
                            items.push(Item {
                                at: place.transform_point3(Vec3::ZERO),
                                size: px * s * if along { 1.0 } else { 0.5 } * VIEW_SCALE,
                                roll: 0.0,
                                color: color_at(&k.color_keys(), t),
                                texture: Some(sprite.to_string()),
                                additive: additive(k),
                            });
                        }
                        "ParticleSystem" => {
                            let interval = k.float("EmissionInterval").unwrap_or(-1.0);
                            let next = *state.emit.entry((i, ki)).or_insert(Some(age));
                            if let Some(mut at) = next {
                                if interval <= 0.0 {
                                    emit(&mut state, k, i, ki, place);
                                    state.emit.insert((i, ki), None);
                                } else {
                                    // At most a second's worth after a stall.
                                    at = at.max(age - 1.0);
                                    while at <= age {
                                        emit(&mut state, k, i, ki, place);
                                        at += interval;
                                    }
                                    state.emit.insert((i, ki), Some(at));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            // Particles: move, age, draw.
            let mut particles = std::mem::take(&mut state.particles);
            particles.retain_mut(|p| {
                p.age += dt;
                p.age < p.life
            });
            for p in &mut particles {
                let Some(e) = attached.effects.get(p.effect) else { continue };
                let Some(k) = lib.group(&e.group).and_then(|g| g.keys.get(p.key)) else { continue };
                let gravity = k.float("GravityAcceleration").unwrap_or(0.0);
                let friction = k.float("Friction").unwrap_or(0.0).clamp(0.0, 0.999);
                p.at += p.velocity * dt;
                p.velocity = p.velocity * (1.0 - friction).powf(dt) + Vec3::Y * gravity * VIEW_SCALE * dt;
                p.roll += p.spin * dt;
                let t = (p.age / p.life).clamp(0.0, 1.0);
                let s = sample_keys(&k.scale_keys(), t, 1.0);
                items.push(Item {
                    at: p.at,
                    size: 4.0 * s * VIEW_SCALE,
                    roll: p.roll,
                    color: color_at(&k.color_keys(), t),
                    texture: k.path("Texture").map(str::to_string),
                    additive: additive(k),
                });
            }
            state.particles = particles;
        }
        _ => {
            state.weapon = None;
            state.emit.clear();
            state.particles.clear();
        }
    }
    // The pool: one quad per item, made as needed under the view-model
    // camera; the rest hidden.
    let have = quads.iter().count();
    if items.len() > have
        && let Some(camera) = camera.as_deref()
    {
        for n in have..items.len() {
            let q = commands
                .spawn((
                    AttachedQuad(n),
                    Mesh3d(meshes.add(Rectangle::new(1.0, 1.0))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        unlit: true,
                        double_sided: true,
                        cull_mode: None,
                        alpha_mode: AlphaMode::Add,
                        ..default()
                    })),
                    Transform::default(),
                    Visibility::Hidden,
                    RenderLayers::layer(VIEW_MODEL_LAYER),
                    bevy::light::NotShadowCaster,
                ))
                .id();
            commands.entity(*camera).add_child(q);
        }
    }
    for (AttachedQuad(n), mut t, mut v, m) in &mut quads {
        let Some(item) = items.get(*n) else {
            if *v != Visibility::Hidden {
                *v = Visibility::Hidden;
            }
            continue;
        };
        *t = Transform::from_translation(item.at)
            .with_rotation(Quat::from_rotation_z(item.roll))
            .with_scale(Vec3::splat(item.size.max(1e-5)));
        *v = Visibility::Inherited;
        let tex = item.texture.as_deref().and_then(|p| images.get(p, &server));
        if let Some(mut mat) = materials.get_mut(&m.0) {
            let c = item.color;
            mat.base_color = if item.additive {
                Color::srgb(c.x * c.w, c.y * c.w, c.z * c.w)
            } else {
                Color::srgba(c.x, c.y, c.z, c.w)
            };
            mat.alpha_mode = if item.additive { AlphaMode::Add } else { AlphaMode::Blend };
            if mat.base_color_texture != tex {
                mat.base_color_texture = tex;
            }
        }
    }
}
