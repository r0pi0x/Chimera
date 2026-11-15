//! The library of Halo's weapon effects (`HaloFx`), compiled from a map
//! file's tags (`compile`; specs/halo_ce/effects.md "What is on the disc":
//! every multiplayer map holds the same tags, so the donor map's serve
//! every map, Halo's own included), and how it joins a map (`add`,
//! `dress`): its textures, decoded sounds, particle materials and decal
//! groups appended to the map's, the library (indices moved past the
//! map's own) in the map's memos.
//!
//! What is read, per gun (its weapon tag, `params::Gun::tag`): the
//! trigger's firing, misfire and empty effects; the weapon's ready and
//! overheated effects and zoom sounds; the first-person graph's sounds by
//! slot with their frames (ready, reload, melee, overheat, the shotgun's
//! steps and pump); the first-person and world models' bind-pose markers;
//! the light attachment (the muzzle light); the trigger's firing damage
//! effect's shake (the kick); the projectile's material responses'
//! default effects, its contrail (the tracer) and light attachments.
//! Per projectile (`FxProjectile`: every gun's triggers', the plasma
//! pistol's charged bolt among them, and the two grenades'): its default
//! and potential impact effects by material, its detonation, supercombine
//! and detonation-started effects, its contrail, light and effect
//! attachments (the rocket's exhaust, the grenades' trails) and whether it
//! has a model. Per item (the guns' weapon tags and every equipment tag):
//! its pickup sound. Through the effects: every sound, decal (and its
//! chain), light and particle (with its bitmap's sprite sequences and its
//! point physics).
//!
//! Known shortcuts: a particle's sprites come from its bitmap's first
//! page only (all the weapons' particle bitmaps on the disc are one page);
//! a decal's size is its radius bounds' mean (one variant per sprite
//! sequence) and additive decals (the plasma burn's glows) are left out:
//! the engine's decals multiply or blend; object parts, damage parts,
//! collision and death effects and material effects (the casings' clicks)
//! are not compiled; a particle system keeps each type's birth state (its
//! length and creation rate) and its particle states (`FxSystem`; the type
//! states after the birth are not kept); a sound plays its permutations at
//! random (no deck without repeats) at their mean gain.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

use bevy::prelude::*;
use blam_formats::{
    animation::{AnimationGraph, FirstPersonSlot},
    bitmap::Bitmap,
    cache::{MapFile, TagRef, class},
    damage_effect::DamageEffect,
    effect::{Contrail, Decal, Effect, Light, Particle, ParticleSystem, PointPhysics},
    model::Model,
    projectile::Projectile,
    sound::Sound,
    weapon::Weapon,
};

use super::{ByPath, name_of, sound::level};
use crate::map::{
    LoadContext, MapData, MapLoadOptions, MapMemos, MapTexture,
    decal::{DecalBlend, MapDecal},
    particles::{ParticleBlend, ParticleMaterial},
    sound::{Interval, MapSoundClip, MapSoundEntry},
};

/// A light an effect part or a weapon makes: colour at full intensity,
/// radius (WU), duration (ticks; the muzzle light's fall-off length).
#[derive(Clone, Debug, PartialEq)]
pub struct FxLight {
    pub color: [f32; 3],
    pub radius: f32,
    pub ticks: f32,
}

/// What a part makes.
#[derive(Clone, Debug, PartialEq)]
pub enum FxPartKind {
    /// A sound entry's name.
    Sound(String),
    /// A decal group's name (`MapDecals::groups`).
    Decal(String),
    Light(FxLight),
    /// A particle system (`HaloFx::systems`).
    System(usize),
    /// Not compiled (damage, objects).
    Other,
}

/// A particle system's particle state as drawn (`play::system`).
#[derive(Clone, Debug, PartialEq)]
pub struct FxSysState {
    /// Seconds.
    pub duration: (f32, f32),
    /// Index into the map's particle materials.
    pub material: Option<usize>,
    pub sequence: i16,
    /// The sprite's radius bounds (WU): the scale (WU per pixel) times half
    /// the sprite's larger side in pixels.
    pub radius: (f32, f32),
    pub color_lower: [f32; 4],
    pub color_upper: [f32; 4],
    pub additive: bool,
    pub animation_rate: (f32, f32),
}

/// A particle system's type: its creation (initial count, the physics and
/// its constants), its birth state's length and creation rate, its
/// particle states.
#[derive(Clone, Debug, PartialEq)]
pub struct FxSysType {
    pub flags: u32,
    pub initial: i16,
    pub creation: i16,
    pub constants: Vec<f32>,
    pub birth: (f32, f32),
    pub rate: f32,
    pub states: Vec<FxSysState>,
}

/// A particle system (`pctl`) as drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct FxSystem {
    pub path: String,
    pub types: Vec<FxSysType>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FxPart {
    pub location: i16,
    pub environment: i16,
    pub mode: i16,
    pub face_down: bool,
    pub kind: FxPartKind,
    /// Decals: the trace length (WU).
    pub velocity: (f32, f32),
    pub radius_modifier: (f32, f32),
}

/// A particle entry, the tag's numbers (WU, s, rad).
#[derive(Clone, Debug, PartialEq)]
pub struct FxEntry {
    pub location: i16,
    pub environment: i16,
    pub camera_mode: i16,
    pub offset: [f32; 3],
    pub direction: [f32; 3],
    /// Index into `HaloFx::particles`.
    pub particle: Option<usize>,
    pub flags: u32,
    pub distribution: i16,
    pub count: (i16, i16),
    pub distribution_radius: (f32, f32),
    pub velocity: (f32, f32),
    pub cone: f32,
    pub angular_velocity: (f32, f32),
    pub radius: (f32, f32),
    pub tint_lower: [f32; 4],
    pub tint_upper: [f32; 4],
}

#[derive(Clone, Debug, PartialEq)]
pub struct FxEvent {
    pub skip: f32,
    pub delay: (f32, f32),
    pub duration: (f32, f32),
    pub parts: Vec<FxPart>,
    pub entries: Vec<FxEntry>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FxEffect {
    pub path: String,
    pub locations: Vec<String>,
    pub events: Vec<FxEvent>,
}

/// A particle tag as drawn (`play`).
#[derive(Clone, Debug, PartialEq)]
pub struct FxParticle {
    pub path: String,
    /// Index into the map's particle materials (`MapData::particles`).
    pub material: Option<usize>,
    pub flags: u32,
    pub lifespan: (f32, f32),
    pub fade_in: f32,
    pub fade_out: f32,
    pub radius_animation: (f32, f32),
    pub animation_rate: (f32, f32),
    pub first_sequence: i16,
    pub initial: i16,
    pub looping: i16,
    pub additive: bool,
    pub physics: Option<PointPhysics>,
    /// 0 screen facing, 1 parallel to its direction, 2 perpendicular.
    pub orientation: i16,
    /// The first sprite's width over its height.
    pub aspect: f32,
}

/// A projectile's tracer: its contrail's first state (WU, colour) and
/// the material its ribbon draws with.
#[derive(Clone, Debug, PartialEq)]
pub struct FxTracer {
    pub width: f32,
    pub color: [f32; 4],
    pub material: Option<usize>,
    pub additive: bool,
}

/// One gun's presentation (by its weapon tag's path).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FxGun {
    /// Per firing effect entry (the plasma rifle's two barrels).
    pub firing: Vec<Option<usize>>,
    pub misfire: Vec<Option<usize>>,
    pub empty: Vec<Option<usize>>,
    pub ready: Option<usize>,
    pub overheated: Option<usize>,
    pub zoom_in: Option<String>,
    pub zoom_out: Option<String>,
    /// The muzzle light (the weapon's light attachment) and its marker.
    pub light: Option<(FxLight, String)>,
    /// First-person graph sounds by slot name (`ready`, `reload-empty`
    /// ...): the entry and the frame it starts on.
    pub anim_sounds: BTreeMap<String, (String, i16)>,
    /// First-person model markers in its bind pose (WU, camera frame:
    /// forward, left, up) and the world model's (its own frame).
    pub fp_markers: BTreeMap<String, [f32; 3]>,
    pub markers: BTreeMap<String, [f32; 3]>,
    /// The firing kick: the shake's rotation (rad) and ticks.
    pub kick: Option<(f32, u32)>,
    /// Impact effects by material type code (the default responses).
    pub impacts: Vec<Option<usize>>,
    pub tracer: Option<FxTracer>,
    /// The projectile's light attachment (the plasma bolt's glow).
    pub bolt_light: Option<FxLight>,
}

/// One projectile's presentation (by its tag's path): what the guns'
/// second triggers and the grenades fire too (the charged bolt, the frag).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FxProjectile {
    /// Impact effects by material type code (the default responses), and
    /// the potential responses' (the charged bolt's "charged bolt impact").
    pub impacts: Vec<Option<usize>>,
    pub potential: Vec<Option<usize>>,
    /// The detonation effect, the supercombine's, the countdown's (a plasma
    /// grenade stuck or landed: "detonation started").
    pub detonation: Option<usize>,
    pub super_detonation: Option<usize>,
    pub detonation_started: Option<usize>,
    pub tracer: Option<FxTracer>,
    /// Its light attachment (the bolt's glow, the grenade's light).
    pub light: Option<FxLight>,
    /// Its effect attachments by marker (the rocket's exhaust, the frag's
    /// smoke trail, the plasma grenade's glow), made again each Halo tick.
    pub attached: Vec<(usize, String)>,
    /// Its object model has meshes: drawn as `object_key(tag)`.
    pub model: bool,
}

/// The compiled library.
#[derive(Clone, Debug, Default)]
pub struct HaloFx {
    pub effects: Vec<FxEffect>,
    pub by_path: ByPath,
    pub particles: Vec<FxParticle>,
    pub systems: Vec<FxSystem>,
    /// By weapon tag path.
    pub guns: BTreeMap<String, FxGun>,
    /// By projectile tag path: every gun's triggers' and the grenades'.
    pub projectiles: BTreeMap<String, FxProjectile>,
    /// Equipment and weapon items' pickup sounds by tag path.
    pub pickups: BTreeMap<String, String>,
}

impl HaloFx {
    pub fn effect(&self, path: &str) -> Option<&FxEffect> {
        self.by_path.get(&path.to_ascii_lowercase()).map(|i| &self.effects[*i])
    }

    pub fn gun(&self, tag: &str) -> Option<&FxGun> {
        self.guns.get(&tag.to_ascii_lowercase())
    }

    pub fn projectile(&self, tag: &str) -> Option<&FxProjectile> {
        self.projectiles.get(&tag.to_ascii_lowercase())
    }
}

/// The library with what it brings to a map (textures, clips, entries,
/// materials, decals), indices into its own lists.
#[derive(Clone, Debug, Default)]
pub struct FxSet {
    pub library: HaloFx,
    pub textures: Vec<MapTexture>,
    pub clips: Vec<MapSoundClip>,
    pub entries: BTreeMap<String, MapSoundEntry>,
    pub materials: Vec<ParticleMaterial>,
    pub decals: Vec<(String, Vec<MapDecal>)>,
    pub warnings: Vec<String>,
}

struct Compiler<'a> {
    file: &'a MapFile,
    decode: bool,
    set: FxSet,
    /// Bitmap tag id to texture index.
    bitmaps: BTreeMap<u32, Option<usize>>,
    particles: BTreeMap<u32, usize>,
    systems: BTreeMap<u32, usize>,
    sounds: BTreeSet<u32>,
    decals: BTreeSet<u32>,
}

fn path(file: &MapFile, r: &TagRef) -> Option<String> {
    (!r.id.is_none()).then(|| file.tag(r.id).map(|t| t.path.to_ascii_lowercase())).flatten()
}

impl Compiler<'_> {
    fn warn(&mut self, w: String) {
        self.set.warnings.push(w);
    }

    /// The bitmap's first page as a texture, and its sprite sequences
    /// (u0, v0, u1, v1 per sprite).
    fn bitmap(&mut self, r: &TagRef) -> Option<(usize, Vec<Vec<[f32; 4]>>)> {
        let tag = self.file.tag(r.id)?;
        let bitmap = Bitmap::read(self.file, tag).map_err(|e| self.warn(format!("{}: {e}", tag.path))).ok()?;
        let seqs: Vec<Vec<[f32; 4]>> = bitmap
            .sequences
            .iter()
            .map(|s| {
                if s.sprites.is_empty() {
                    vec![[0.0, 0.0, 1.0, 1.0]]
                } else {
                    s.sprites.iter().filter(|p| p.bitmap_index == 0).map(|p| [p.left, p.top, p.right, p.bottom]).collect()
                }
            })
            .collect();
        if let Some(t) = self.bitmaps.get(&r.id.0) {
            return t.map(|t| (t, seqs));
        }
        let texture = (|| {
            let image = bitmap.images.first()?;
            let (width, height) = (image.width as u32, image.height as u32);
            let mut t = MapTexture { name: tag.path.replace('\\', "/"), srgb: true, mips: Vec::new(), width, height, rgba8: Vec::new(), bc: None };
            if self.decode {
                let level = image.decode_level(self.file, 0, 0).ok()?;
                t.rgba8 = level.rgba[..(level.width * level.height * 4) as usize].to_vec();
                t.mips = t.box_mips();
            }
            Some(t)
        })();
        let index = texture.map(|t| {
            self.set.textures.push(t);
            self.set.textures.len() - 1
        });
        self.bitmaps.insert(r.id.0, index);
        index.map(|t| (t, seqs))
    }

    fn sound(&mut self, r: &TagRef) -> Option<String> {
        let tag = self.file.tag(r.id)?;
        let name = name_of(&tag.path);
        if !self.sounds.insert(r.id.0) {
            return Some(name);
        }
        let s = match Sound::read(self.file, tag) {
            Ok(s) => s,
            Err(e) => {
                self.warn(format!("{}: {e}", tag.path));
                return None;
            }
        };
        let range = s.pitch_ranges.first()?;
        let mut waves = Vec::new();
        let mut gain = 0.0;
        for head in 0..range.actual_permutation_count.min(range.permutations.len()) {
            match s.decode_chain(self.file, 0, head) {
                Ok(pcm) => {
                    self.set.clips.push(MapSoundClip { rate: pcm.rate, channels: pcm.channels, samples: pcm.samples.into(), loop_start: None });
                    waves.push(self.set.clips.len() - 1);
                    gain += range.permutations[head].gain;
                }
                Err(e) => self.warn(format!("{}: permutation {head}: {e}", tag.path)),
            }
        }
        if waves.is_empty() {
            return None;
        }
        let (min, _) = s.distances();
        let gain = gain / waves.len() as f32 * s.gain_modifier * s.scale_one.gain.max(0.0);
        let pitch = s.random_pitch_bounds;
        let (lo, hi) = if pitch.upper > 0.0 { (pitch.lower, pitch.upper) } else { (1.0, 1.0) };
        self.set.entries.insert(name.clone(), MapSoundEntry {
            waves,
            volume: Interval::fixed(gain.clamp(0.0, 1.0)),
            pitch: Interval { start: lo * 100.0 * range.playback_rate.max(0.01), range: (hi - lo) * 100.0 * range.playback_rate.max(0.01) },
            level: level(min),
            channel: 0,
            dry: false,
        });
        Some(name)
    }

    fn decal(&mut self, r: &TagRef) -> Option<String> {
        let tag = self.file.tag(r.id)?;
        let name = name_of(&tag.path);
        if !self.decals.insert(r.id.0) {
            return Some(name);
        }
        let d = Decal::read(self.file, tag).map_err(|e| self.warn(format!("{}: {e}", tag.path))).ok()?;
        let blend = match d.blend {
            Decal::DOUBLE_MULTIPLY => DecalBlend::Modulate2x,
            Decal::ADD => return None,
            _ => DecalBlend::Alpha,
        };
        let (texture, seqs) = self.bitmap(&d.map)?;
        let size = (d.radius.lower + d.radius.upper) * super::super::METERS_PER_UNIT;
        let variants: Vec<MapDecal> = seqs
            .iter()
            .filter_map(|s| s.first())
            .map(|uv| MapDecal { texture, uv_min: Vec2::new(uv[0], uv[1]), uv_max: Vec2::new(uv[2], uv[3]), size: Vec2::splat(size), blend })
            .collect();
        self.set.decals.push((name.clone(), variants));
        Some(name)
    }

    fn light(&mut self, r: &TagRef) -> Option<FxLight> {
        let l = Light::read(self.file, self.file.tag(r.id)?).ok()?;
        let (color, radius) = l.at(1.0);
        Some(FxLight { color, radius, ticks: l.duration })
    }

    fn particle(&mut self, r: &TagRef) -> Option<usize> {
        if let Some(i) = self.particles.get(&r.id.0) {
            return Some(*i);
        }
        let tag = self.file.tag(r.id)?;
        let p = Particle::read(self.file, tag).map_err(|e| self.warn(format!("{}: {e}", tag.path))).ok()?;
        let physics = self.file.tag(p.physics.id).and_then(|t| PointPhysics::read(self.file, t).ok());
        let additive = p.shader.blend != blam_formats::effect::EffectShader::ALPHA_BLEND;
        let mut aspect = 1.0;
        let material = self.bitmap(&p.bitmap).map(|(texture, sequences)| {
            if let Some(r) = sequences.first().and_then(|s| s.first()) {
                let (w, h) = self.set.textures.get(texture).map_or((1.0, 1.0), |t| (t.width as f32, t.height as f32));
                aspect = ((r[2] - r[0]) * w / ((r[3] - r[1]) * h).max(1e-6)).max(1e-3);
            }
            self.set.materials.push(ParticleMaterial {
                name: name_of(&tag.path),
                texture: Some(texture),
                blend: if additive { ParticleBlend::Additive } else { ParticleBlend::Alpha },
                vertex_color: true,
                vertex_alpha: true,
                sequences,
                ..Default::default()
            });
            self.set.materials.len() - 1
        });
        self.set.library.particles.push(FxParticle {
            path: tag.path.to_ascii_lowercase(),
            material,
            flags: p.flags,
            lifespan: (p.lifespan.lower, p.lifespan.upper),
            fade_in: p.fade_in,
            fade_out: p.fade_out,
            radius_animation: (p.radius_animation.lower, p.radius_animation.upper),
            animation_rate: (p.animation_rate.lower, p.animation_rate.upper),
            first_sequence: p.first_sequence,
            initial: p.initial_sequences,
            looping: p.looping_sequences,
            additive,
            physics,
            orientation: p.orientation,
            aspect,
        });
        let i = self.set.library.particles.len() - 1;
        self.particles.insert(r.id.0, i);
        Some(i)
    }

    /// A particle system's types and states (module doc), each state's
    /// bitmap a particle material.
    fn system(&mut self, r: &TagRef) -> Option<usize> {
        if let Some(i) = self.systems.get(&r.id.0) {
            return Some(*i);
        }
        let tag = self.file.tag(r.id)?;
        let sys = ParticleSystem::read(self.file, tag).map_err(|e| self.warn(format!("{}: {e}", tag.path))).ok()?;
        let mut types = Vec::new();
        for t in &sys.types {
            if t.flags & blam_formats::effect::system::ParticleType::DISABLED != 0 {
                continue;
            }
            let mut states = Vec::new();
            for st in &t.particle_states {
                let additive = st.shader.blend != blam_formats::effect::EffectShader::ALPHA_BLEND;
                let (mut pixels, mut material) = (1.0, None);
                if let Some((texture, sequences)) = self.bitmap(&st.bitmap) {
                    let (w, h) = self.set.textures.get(texture).map_or((1.0, 1.0), |t| (t.width as f32, t.height as f32));
                    if let Some(r) = sequences.get(st.sequence.max(0) as usize).and_then(|s| s.first()) {
                        pixels = ((r[2] - r[0]) * w).max((r[3] - r[1]) * h).abs();
                    }
                    self.set.materials.push(ParticleMaterial {
                        name: format!("{}#{}", name_of(&tag.path), self.set.materials.len()),
                        texture: Some(texture),
                        blend: if additive { ParticleBlend::Additive } else { ParticleBlend::Alpha },
                        vertex_color: true,
                        vertex_alpha: true,
                        sequences,
                        ..Default::default()
                    });
                    material = Some(self.set.materials.len() - 1);
                }
                let argb = |c: &blam_formats::cache::RealArgb| [c.a, c.r, c.g, c.b];
                states.push(FxSysState {
                    duration: (st.duration.lower, st.duration.upper),
                    material,
                    sequence: st.sequence,
                    radius: (st.scale.lower * pixels / 2.0, st.scale.upper * pixels / 2.0),
                    color_lower: argb(&st.color_lower),
                    color_upper: argb(&st.color_upper),
                    additive,
                    animation_rate: (st.animation_rate.lower, st.animation_rate.upper),
                });
            }
            let birth = t.type_states.first();
            types.push(FxSysType {
                flags: t.flags,
                initial: t.initial_count,
                creation: t.creation_physics,
                constants: t.constants.clone(),
                birth: birth.map_or((0.0, 0.0), |b| (b.duration.lower, b.duration.upper)),
                rate: birth.map_or(0.0, |b| b.creation_rate),
                states,
            });
        }
        let lib = &mut self.set.library;
        lib.systems.push(FxSystem { path: tag.path.to_ascii_lowercase(), types });
        let i = lib.systems.len() - 1;
        self.systems.insert(r.id.0, i);
        Some(i)
    }

    fn effect(&mut self, r: &TagRef) -> Option<usize> {
        let tag = self.file.tag(r.id)?;
        let key = tag.path.to_ascii_lowercase();
        if let Some(i) = self.set.library.by_path.get(&key) {
            return Some(*i);
        }
        if tag.class == class::SOUND {
            // A firing effects entry may name a sound (the empty click,
            // the overheat): one event playing it at the origin.
            let entry = self.sound(r)?;
            let part = FxPart { location: 0, environment: 0, mode: 0, face_down: false, kind: FxPartKind::Sound(entry), velocity: (0.0, 0.0), radius_modifier: (0.0, 0.0) };
            let event = FxEvent { skip: 0.0, delay: (0.0, 0.0), duration: (0.0, 0.0), parts: vec![part], entries: Vec::new() };
            let lib = &mut self.set.library;
            lib.effects.push(FxEffect { path: key.clone(), locations: vec![String::new()], events: vec![event] });
            lib.by_path.insert(key, lib.effects.len() - 1);
            return Some(lib.effects.len() - 1);
        }
        let e = Effect::read(self.file, tag).map_err(|e| self.warn(format!("{}: {e}", tag.path))).ok()?;
        let mut events = Vec::new();
        for ev in &e.events {
            let mut parts = Vec::new();
            for p in &ev.parts {
                let kind = match p.base_class {
                    c if c == class::SOUND => self.sound(&p.kind).map(FxPartKind::Sound),
                    c if c == class::DECAL => {
                        let first = self.decal(&p.kind);
                        // The chain's next decal is made with it.
                        if let Some(next) = self.file.tag(p.kind.id).and_then(|t| Decal::read(self.file, t).ok()).map(|d| d.next) {
                            let _ = self.decal(&next);
                        }
                        first.map(FxPartKind::Decal)
                    }
                    c if c == class::LIGHT => self.light(&p.kind).map(FxPartKind::Light),
                    c if c == class::PARTICLE_SYSTEM => self.system(&p.kind).map(FxPartKind::System),
                    _ => Some(FxPartKind::Other),
                };
                let Some(kind) = kind else { continue };
                parts.push(FxPart {
                    location: p.location,
                    environment: p.environment,
                    mode: p.mode,
                    face_down: p.flags & 1 != 0,
                    kind,
                    velocity: (p.velocity.lower, p.velocity.upper),
                    radius_modifier: (p.radius_modifier.lower, p.radius_modifier.upper),
                });
            }
            let entries = ev
                .particles
                .iter()
                .map(|q| FxEntry {
                    location: q.location,
                    environment: q.environment,
                    camera_mode: q.camera_mode,
                    offset: q.offset,
                    direction: q.direction_vector,
                    particle: self.particle(&q.particle),
                    flags: q.flags,
                    distribution: q.distribution,
                    count: (q.count.lower, q.count.upper),
                    distribution_radius: (q.distribution_radius.lower, q.distribution_radius.upper),
                    velocity: (q.velocity.lower, q.velocity.upper),
                    cone: q.velocity_cone,
                    angular_velocity: (q.angular_velocity.lower, q.angular_velocity.upper),
                    radius: (q.radius.lower, q.radius.upper),
                    tint_lower: [q.tint_lower.a, q.tint_lower.r, q.tint_lower.g, q.tint_lower.b],
                    tint_upper: [q.tint_upper.a, q.tint_upper.r, q.tint_upper.g, q.tint_upper.b],
                })
                .collect();
            events.push(FxEvent {
                skip: ev.skip_fraction,
                delay: (ev.delay.lower, ev.delay.upper),
                duration: (ev.duration.lower, ev.duration.upper),
                parts,
                entries,
            });
        }
        let lib = &mut self.set.library;
        lib.effects.push(FxEffect { path: key.clone(), locations: e.locations, events });
        lib.by_path.insert(key, lib.effects.len() - 1);
        Some(lib.effects.len() - 1)
    }

    fn gun(&mut self, weapon_path: &str) -> Result<FxGun, String> {
        let file = self.file;
        let tag = file.find(class::WEAPON, weapon_path).ok_or_else(|| format!("no weapon `{weapon_path}`"))?;
        let w = Weapon::read(file, tag).map_err(|e| e.to_string())?;
        let mut g = FxGun::default();
        for t in &w.own.triggers {
            for f in &t.firing_effects {
                g.firing.push(self.effect(&f.firing_effect));
                g.misfire.push(self.effect(&f.misfire_effect));
                g.empty.push(self.effect(&f.empty_effect));
            }
            if g.kick.is_none()
                && let Some(d) = t.firing_effects.first().and_then(|f| file.tag(f.firing_damage.id)).and_then(|d| DamageEffect::read(file, d).ok())
                && d.camera_shaking_duration > 0.0
            {
                g.kick = Some((d.camera_shaking_random_rotation, ((d.camera_shaking_duration * 30.0) as u32).max(1)));
            }
        }
        g.ready = self.effect(&w.own.ready_effect);
        g.overheated = self.effect(&w.own.overheated);
        g.zoom_in = self.sound(&w.own.zoom_in_sound);
        g.zoom_out = self.sound(&w.own.zoom_out_sound);
        for a in &w.object.attachments {
            if a.attached.class == class::LIGHT && g.light.is_none() {
                g.light = self.light(&a.attached).map(|l| (l, a.marker_name.to_ascii_lowercase()));
            }
        }
        if let Some(graph) = file.tag(w.own.first_person_animations.id).and_then(|t| AnimationGraph::read(file, t).ok()) {
            for slot in FirstPersonSlot::ALL {
                let Some(anim) = graph.first_person_animation(*slot) else { continue };
                let Some(sound) = anim.sound.and_then(|i| graph.sounds.get(i)).map(|s| s.sound) else { continue };
                if let Some(entry) = self.sound(&sound) {
                    g.anim_sounds.insert(slot.label().to_string(), (entry, anim.sound_frame));
                }
            }
        }
        g.fp_markers = markers(file, &w.own.first_person_model);
        g.markers = markers(file, &w.object.model);
        if let Some(proj) = w.own.triggers.first().and_then(|t| file.tag(t.projectile.id)).and_then(|t| Projectile::read(file, t).ok()) {
            g.impacts = proj.own.material_responses.iter().map(|r| self.effect(&r.default_effect)).collect();
            for a in &proj.object.attachments {
                if a.attached.class == class::CONTRAIL && g.tracer.is_none() {
                    g.tracer = self.contrail(&a.attached);
                } else if a.attached.class == class::LIGHT && g.bolt_light.is_none() {
                    g.bolt_light = self.light(&a.attached);
                }
            }
        }
        Ok(g)
    }

    fn projectile(&mut self, path: &str) -> Result<FxProjectile, String> {
        let file = self.file;
        let tag = file.find(class::PROJECTILE, path).ok_or_else(|| format!("no projectile `{path}`"))?;
        let p = Projectile::read(file, tag).map_err(|e| format!("{path}: {e}"))?;
        let mut out = FxProjectile {
            impacts: p.own.material_responses.iter().map(|r| self.effect(&r.default_effect)).collect(),
            potential: p.own.material_responses.iter().map(|r| self.effect(&r.potential_effect)).collect(),
            detonation: self.effect(&p.own.effect),
            super_detonation: self.effect(&p.own.super_detonation),
            detonation_started: self.effect(&p.own.detonation_started),
            model: file.tag(p.object.model.id).is_some(),
            ..Default::default()
        };
        for a in &p.object.attachments {
            if a.attached.class == class::CONTRAIL && out.tracer.is_none() {
                out.tracer = self.contrail(&a.attached);
            } else if a.attached.class == class::LIGHT && out.light.is_none() {
                out.light = self.light(&a.attached);
            } else if a.attached.class == class::EFFECT
                && let Some(e) = self.effect(&a.attached)
            {
                out.attached.push((e, a.marker_name.to_ascii_lowercase()));
            }
        }
        Ok(out)
    }

    /// An item tag's pickup sound (equipment's, a weapon's).
    fn pickup(&mut self, path: &str) -> Option<String> {
        let file = self.file;
        if let Some(t) = file.find(class::EQUIPMENT, path) {
            let e = blam_formats::object::Equipment::read(file, t).ok()?;
            return self.sound(&e.own.pickup_sound);
        }
        let t = file.find(class::WEAPON, path)?;
        let w = Weapon::read(file, t).ok()?;
        self.sound(&w.own.pickup_sound)
    }

    fn contrail(&mut self, r: &TagRef) -> Option<FxTracer> {
        let c = Contrail::read(self.file, self.file.tag(r.id)?).ok()?;
        let s = c.point_states.first()?;
        let additive = c.shader.blend != blam_formats::effect::EffectShader::ALPHA_BLEND;
        let material = self.bitmap(&c.bitmap).map(|(texture, _)| {
            self.set.materials.push(ParticleMaterial {
                name: name_of(&path(self.file, r).unwrap_or_default()),
                texture: Some(texture),
                blend: if additive { ParticleBlend::Additive } else { ParticleBlend::Alpha },
                vertex_color: true,
                vertex_alpha: true,
                ..Default::default()
            });
            self.set.materials.len() - 1
        });
        let (a, b) = (s.color_lower, s.color_upper);
        Some(FxTracer { width: s.width, color: [b.r.max(a.r), b.g.max(a.g), b.b.max(a.b), b.a.max(a.a)], material, additive })
    }
}

/// A model's markers (the first instance of each) in its bind pose, in the
/// model's frame (WU).
fn markers(file: &MapFile, r: &TagRef) -> BTreeMap<String, [f32; 3]> {
    let Some(model) = file.tag(r.id).and_then(|t| Model::read(file, t).ok()) else { return BTreeMap::new() };
    use blam_formats::model::Transform as T;
    let mut world: Vec<T> = Vec::with_capacity(model.nodes.len());
    for n in &model.nodes {
        let parent = n.parent.and_then(|p| world.get(p).copied()).unwrap_or(T::IDENTITY);
        world.push(parent.then(&n.local()));
    }
    model
        .markers
        .iter()
        .filter_map(|m| {
            let i = m.instances.first()?;
            let node = world.get(i.node).copied().unwrap_or(T::IDENTITY);
            Some((m.name.to_ascii_lowercase(), node.apply_point(i.translation)))
        })
        .collect()
}

/// The library of `file`'s tags for the guns of weapon tag paths `guns`.
pub fn compile(file: &MapFile, guns: &[&str], decode: bool) -> FxSet {
    let mut c = Compiler { file, decode, set: FxSet::default(), bitmaps: BTreeMap::new(), particles: BTreeMap::new(), systems: BTreeMap::new(), sounds: BTreeSet::new(), decals: BTreeSet::new() };
    for g in guns {
        match c.gun(g) {
            Ok(gun) => {
                c.set.library.guns.insert(g.to_ascii_lowercase(), gun);
            }
            Err(e) => c.warn(e),
        }
    }
    for p in projectile_tags(file, guns) {
        match c.projectile(&p) {
            Ok(look) => {
                c.set.library.projectiles.insert(p, look);
            }
            Err(e) => c.warn(e),
        }
    }
    let items: Vec<String> = guns
        .iter()
        .map(|g| g.to_ascii_lowercase())
        .chain(file.of_class(class::EQUIPMENT).map(|t| t.path.to_ascii_lowercase()))
        .collect();
    for i in items {
        if let Some(sound) = c.pickup(&i) {
            c.set.library.pickups.insert(i, sound);
        }
    }
    c.set
}

/// The projectiles' tags (lower case): each gun's triggers' (`guns`: their
/// weapon tags) and the two grenades'.
pub fn projectile_tags(file: &MapFile, guns: &[&str]) -> BTreeSet<String> {
    let grenades = super::super::weapons::grenades::GrenadeData::load();
    guns.iter()
        .filter_map(|g| file.find(class::WEAPON, g).and_then(|t| Weapon::read(file, t).ok()))
        .flat_map(|w| w.own.triggers.into_iter().filter_map(|t| path(file, &t.projectile)))
        .chain([grenades.frag.tag.to_ascii_lowercase(), grenades.plasma.tag.to_ascii_lowercase()])
        .collect()
}

/// The nine guns' weapon tags.
pub fn gun_tags() -> Vec<String> {
    super::super::weapons::guns().into_iter().map(|(_, g)| g.tag).collect()
}

/// `set` joined to `map` (module doc).
pub fn add(map: &mut MapData, set: &FxSet) {
    let tex = map.textures.len();
    map.textures.extend(set.textures.iter().cloned());
    let mat = map.particles.materials.len();
    map.particles.materials.extend(set.materials.iter().cloned().map(|mut m| {
        m.texture = m.texture.map(|t| t + tex);
        m
    }));
    let sounds = Arc::make_mut(&mut map.sounds);
    let clip = sounds.clips.len();
    sounds.clips.extend(set.clips.iter().cloned());
    for (name, e) in &set.entries {
        let mut e = e.clone();
        e.waves.iter_mut().for_each(|w| *w += clip);
        sounds.entries.insert(name.clone(), e);
    }
    let decals = &mut map.kit.decals;
    for (name, variants) in &set.decals {
        let mut group = Vec::new();
        for v in variants {
            decals.decals.push(MapDecal { texture: v.texture + tex, ..v.clone() });
            group.push((1.0, decals.decals.len() - 1));
        }
        decals.groups.insert(name.clone(), group);
    }
    let mut library = set.library.clone();
    for p in &mut library.particles {
        p.material = p.material.map(|m| m + mat);
    }
    for s in library.systems.iter_mut().flat_map(|s| s.types.iter_mut()).flat_map(|t| t.states.iter_mut()) {
        s.material = s.material.map(|m| m + mat);
    }
    for g in library.guns.values_mut() {
        if let Some(t) = &mut g.tracer {
            t.material = t.material.map(|m| m + mat);
        }
    }
    for p in library.projectiles.values_mut() {
        if let Some(t) = &mut p.tracer {
            t.material = t.material.map(|m| m + mat);
        }
    }
    map.memos.insert(Arc::new(library));
    map.impacts.add(super::super::GAME);
    map.warnings.extend(set.warnings.iter().map(|w| format!("Halo's effects: {w}")));
}

/// The donor's sets an app's loads share (with and without textures).
#[derive(Default)]
struct Donor(Mutex<[Option<Arc<FxSet>>; 2]>);

/// The donor map's set for an app (made once per app and `decode`).
pub fn donor(context: &LoadContext, decode: bool) -> Result<Arc<FxSet>, String> {
    let memo = context.get_or_init(Donor::default);
    let mut slots = memo.0.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(s) = &slots[usize::from(decode)] {
        return Ok(s.clone());
    }
    let mount = super::super::maps::mount_for(context)?;
    let path = super::super::maps::map_path(super::super::body::load::DONOR_MAP);
    let bytes = mount.read(&path).map_err(|e| format!("{path}: {e}"))?;
    let file = MapFile::load(&bytes).map_err(|e| format!("{path}: {e}"))?;
    let tags = gun_tags();
    let set = Arc::new(compile(&file, &tags.iter().map(String::as_str).collect::<Vec<_>>(), decode));
    slots[usize::from(decode)] = Some(set.clone());
    Ok(set)
}

/// Halo's weapon effects on `map` (any game's), from the donor map.
pub fn dress(map: &mut MapData, options: &MapLoadOptions) -> Result<(), String> {
    let set = donor(&options.context, options.textures)?;
    add(map, &set);
    Ok(())
}

/// The library a map carries (its memos), for tools and tests.
pub fn of(memos: &MapMemos) -> Option<Arc<HaloFx>> {
    memos.get::<Arc<HaloFx>>().cloned()
}

/// Halo on the maps of other games: its players' body (`body::load`) and
/// its weapons' effects (`game::BUNDLE`'s guest dressing).
pub static GUEST: crate::map::MapDressing = crate::map::MapDressing {
    game: super::super::GAME,
    dress: |map, options| {
        // The Spartan and the guns in hand first (`view::load::GUEST`
        // dresses the body too), then the guns' sounds and effects.
        (super::super::view::load::GUEST.dress)(map, options)?;
        dress(map, options)
    },
};
