//! Effect groups played (specs/combat_arms/effects_sounds.md, "The effect
//! file"): a group of the install's effect file (`library::FxLibrary`)
//! started at a point with an effect frame (`effect_frame`: local +Z
//! along the surface normal or the shot), its keys created at their start
//! times, at the player's effect detail (`DetailLevel`):
//!
//! - `ParticleSystem`: bursts (`EmissionInterval` ≤ 0: one, on the key's
//!   first frame) or emissions every interval until the key ends, each
//!   `ParticlesPerEmission` particles of Chimera's pool
//!   (`map::particles`): start points by `Type` and radius, a per-axis
//!   velocity box in the effect's frame, life, size (scale keys s: a
//!   square of side 4 s), colour and alpha keys, gravity (positive up),
//!   friction per second, bounces (`PercentToBounce`, 75 % kept), spin;
//!   splats (`SplatPercent`, `SplatEffect`: the group at the world hit,
//!   flat against it, `KillOnSplat`) traced along each splatting
//!   particle's fall;
//! - `Sprite`: one particle facing the camera or lying along the normal,
//!   texture pixels × scale wide, for the key's length;
//! - `DynaLight`: a light (`FxLight`: radius and colour keys), drawn by
//!   the front end;
//! - `CamJitter`: an amplitude (scale keys) falling off with the camera's
//!   distance (`rules::jitter_falloff`), read by the front end;
//! - `CreateFX`: another group;
//! - on an entity (`attach`, `effects::attached`): each key placed every
//!   frame by its placement mode (fixed at the point it was made; follow
//!   the entity; socket or node attach at that socket: `place`), a
//!   looping group's whole-group keys never ending and its other keys
//!   replayed each pass, sprites drawn each frame where they are, trail
//!   keys (`PolyTrail`: `trail::Trail`'s points, width by `WidthStyle`,
//!   texture u scrolled by `UAdd`, colour keys over each section's life,
//!   as Chimera's `Shape::Ribbon`, ageing out after a smooth shut-down),
//!   hidden groups frozen and their trails cleared; shut down with
//!   smooth shut-down (particles live out) or at once (`kill_tagged`);
//!   a group made once on an entity (`attach_once`: a launcher's own
//!   FireFX, on the shooter); a projectile's flare and smoke trail
//!   (`flare`, `smoke`: segments of puffs, each puff on its own 10 s
//!   life, the segment fading as a whole).
//!
//! A key drawn through an effect shader (`library::shaded`: the
//! refraction keys of heat and blasts) draws nothing.
//!
//! Several jitters add up (`jitter_at`, Open question 24).
//!
//! Known shortcuts (docs/tech-debt.md): trail keys draw as camera-aligned
//! strips whatever their `Allignment`; a group not on an entity places
//! every key at the effect's point (the first-person muzzle flash is the
//! front end's own, at the view model's socket); multi-key scale and colour curves are
//! drawn as a ramp from the first key to the last (`rules::key_curve` is
//! the full curve), alpha as a fade-in/hold/fade-out window; cone and
//! cylinder emitters are circles (`rules::cone_start` is the cone); `AdditionalAcceleration` is left
//! out; a bouncing system's particles all bounce (Chimera's bounce planes
//! probed once, 75 % ± 10 % kept); friction is Chimera's linear damping at
//! the same per-second rate; `DisableZ` and `CastVisibleRay` sprites are
//! drawn depth-tested; no effect shaders (the refraction keys, whose
//! textures are distortion maps, are left out, not drawn as colour);
//! `LightningFX`, `LTBModel`, `PlaySound` and `Null` keys draw nothing,
//! as do trail keys of a group not on an entity; off-screen systems keep
//! ageing (Quirk 7 not reproduced).

use bevy::prelude::*;
use lithtech_formats::fxf::{FxKey, sample_keys};

use super::{
    library::{FxLibrary, additive, material_name, shaded},
    rules::jitter_falloff,
    trail::{Trail, TrailMode, TrailParams},
};
use crate::map::{
    ParticleRng,
    particles::{Bounce, Fade, MapParticles, Motion, Particle, ParticleGroup, Particles, Ramp, Shape, WorldTrace, probe_planes},
};

/// An effect's frame (columns: local X, Y, Z in world axes): local +Z
/// along `normal`, +Y from the world's up (world +X when the normal is
/// vertical).
pub fn effect_frame(normal: Vec3) -> Mat3 {
    let z = normal.normalize_or(Vec3::Y);
    let up = if z.cross(Vec3::Y).length_squared() < 1e-8 { Vec3::X } else { Vec3::Y };
    let y = (up - z * up.dot(z)).normalize_or(Vec3::X);
    let x = y.cross(z);
    Mat3::from_cols(x, y, z)
}

/// One key's progress.
#[derive(Clone, Debug, Default, PartialEq)]
struct KeyRun {
    started: bool,
    next_emit: f32,
    /// A trail key's points (`PolyTrail`) and what it draws with.
    trail: Trail,
    kit: Option<(TrailParams, usize)>,
}

/// A group playing.
#[derive(Clone, Debug)]
pub struct FxInstance {
    pub group: String,
    /// Metres, world: where it was made.
    pub at: Vec3,
    pub frame: Mat3,
    pub age: f32,
    /// On an entity (`attached`): its parent's pose, looping, shut-down.
    pub attach: Option<Attach>,
    /// Its particle groups' tag (`Particles::kill_tagged`).
    pub tag: u64,
    keys: Vec<KeyRun>,
    /// The looping pass its keys are in.
    pass: u32,
}

/// A group on an entity (effects_sounds.md "The effect file", placement
/// modes; special_weapons.md Follow-up 1; game_modes.md Corrections
/// (2026-10-30) 4).
#[derive(Clone, Debug, PartialEq)]
pub struct Attach {
    pub parent: Entity,
    /// Made looping: keys lasting the whole group never end, the others
    /// play again each pass.
    pub looping: bool,
    /// Its particle systems' live particles finish their lives when it
    /// is shut down (else every key goes at once).
    pub smooth: bool,
    pub pose: ParentPose,
    /// Hidden: nothing drawn or emitted, its clock frozen.
    pub hidden: bool,
    /// Made for one burst (a launcher's own FireFX): placed from the pose
    /// it was made with, living its length, not kept by a listing.
    pub once: bool,
}

/// Where a group's parent is this frame (metres; its frame's columns its
/// right, up and forward axes, `effect_frame`) and its sockets' points.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParentPose {
    pub at: Vec3,
    pub frame: Mat3,
    pub sockets: Vec<(String, Vec3)>,
}

impl ParentPose {
    /// A socket's point (case ignored).
    pub fn socket(&self, name: &str) -> Option<Vec3> {
        self.sockets.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|s| s.1)
    }
}

/// A light an effect made (front end: a point light).
#[derive(Clone, Debug, PartialEq)]
pub struct FxLight {
    pub at: Vec3,
    pub age: f32,
    pub life: f32,
    /// (time 0..1, radius u) and (time, colour rgb + opacity).
    pub radius: Vec<(f32, f32)>,
    pub color: Vec<(f32, [f32; 4])>,
}

impl FxLight {
    /// Its radius (u) now.
    pub fn radius_now(&self) -> f32 {
        sample_keys(&self.radius, self.t(), 0.0)
    }

    pub fn color_now(&self) -> Vec3 {
        let t = self.t();
        let ch = |i: usize| {
            let keys: Vec<(f32, f32)> = self.color.iter().map(|(k, c)| (*k, c[i])).collect();
            sample_keys(&keys, t, 1.0)
        };
        Vec3::new(ch(0), ch(1), ch(2))
    }

    fn t(&self) -> f32 {
        if self.life > 0.0 { (self.age / self.life).clamp(0.0, 1.0) } else { 1.0 }
    }
}

/// A camera jitter an effect made.
#[derive(Clone, Debug, PartialEq)]
pub struct FxJitter {
    pub at: Vec3,
    pub age: f32,
    pub life: f32,
    pub scale: Vec<(f32, f32)>,
    /// u.
    pub inner: f32,
    pub outer: f32,
}

impl FxJitter {
    /// The amplitude a (u) for a camera `d` units away: scale × falloff.
    pub fn amplitude(&self, d: f32) -> f32 {
        let t = if self.life > 0.0 { (self.age / self.life).clamp(0.0, 1.0) } else { 1.0 };
        sample_keys(&self.scale, t, 0.0) * jitter_falloff(d, self.inner, self.outer)
    }
}

/// The groups playing, their lights and jitters.
#[derive(Resource, Debug)]
pub struct CaFx {
    pub instances: Vec<FxInstance>,
    pub lights: Vec<FxLight>,
    pub jitters: Vec<FxJitter>,
    /// The player's effect detail (0 low, 1 medium, 2 high).
    pub detail: u8,
    /// Groups to start later (a splat when its particle lands): (in
    /// seconds, name, point, normal).
    pending: Vec<(f32, String, Vec3, Vec3)>,
    /// Trails of groups shut down smoothly, ageing out.
    fading: Vec<(Trail, TrailParams, usize)>,
    rng: ParticleRng,
    /// Metres per unit.
    pub unit: f32,
    /// The game's screen width in pixels (smoke puffs' size).
    pub screen_width: f32,
    /// The next instance's tag.
    next_tag: u64,
}

impl Default for CaFx {
    fn default() -> Self {
        Self {
            instances: Vec::new(),
            lights: Vec::new(),
            jitters: Vec::new(),
            detail: super::rules::DEFAULT_FX_DETAIL,
            pending: Vec::new(),
            fading: Vec::new(),
            rng: ParticleRng::new(0xCAF0),
            unit: super::super::inventory::UNIT,
            screen_width: DEFAULT_SCREEN_WIDTH,
            next_tag: TAG_BASE,
        }
    }
}

impl CaFx {
    /// Start group `name` at `at` (metres) with frame `frame`; false when
    /// the effect file has no such group (nothing plays).
    pub fn spawn(&mut self, lib: &FxLibrary, name: &str, at: Vec3, frame: Mat3) -> bool {
        self.make(lib, name, at, frame, None)
    }

    /// Start group `name` on `parent` (special_weapons.md Follow-up 1:
    /// made once where the parent is, its keys placed by their placement
    /// mode each frame from the pose `set_pose` gives); false when the
    /// effect file has no such group (nothing plays).
    pub fn attach(&mut self, lib: &FxLibrary, name: &str, parent: Entity, looping: bool, smooth: bool, pose: ParentPose) -> bool {
        let (at, frame) = (pose.at, pose.frame);
        self.make(
            lib,
            name,
            at,
            frame,
            Some(Attach {
                parent,
                looping,
                smooth,
                pose,
                hidden: false,
                once: false,
            }),
        )
    }

    /// Start group `name` once on `parent` (a launcher's own FireFX: the
    /// shooter's point and sockets as `pose` gives them): its keys place
    /// themselves by their modes from that pose and it ends with its length.
    pub fn attach_once(&mut self, lib: &FxLibrary, name: &str, parent: Entity, pose: ParentPose) -> bool {
        let (at, frame) = (pose.at, pose.frame);
        self.make(
            lib,
            name,
            at,
            frame,
            Some(Attach {
                parent,
                looping: false,
                smooth: true,
                pose,
                hidden: false,
                once: true,
            }),
        )
    }

    fn make(&mut self, lib: &FxLibrary, name: &str, at: Vec3, frame: Mat3, attach: Option<Attach>) -> bool {
        let Some(g) = lib.group(name) else { return false };
        self.next_tag += 1;
        self.instances.push(FxInstance {
            group: g.name.clone(),
            at,
            frame,
            age: 0.0,
            attach,
            tag: self.next_tag,
            keys: vec![KeyRun::default(); g.keys.len()],
            pass: 0,
        });
        true
    }

    /// The groups on `parent`.
    pub fn attached(&self, parent: Entity) -> impl Iterator<Item = &FxInstance> {
        self.instances.iter().filter(move |i| i.attach.as_ref().is_some_and(|a| a.parent == parent))
    }

    /// `parent`'s group `group` gets its pose and visibility for this
    /// frame.
    pub fn set_pose_of(&mut self, parent: Entity, group: &str, pose: &ParentPose, hidden: bool) {
        for i in self.instances.iter_mut().filter(|i| i.group.eq_ignore_ascii_case(group)) {
            if let Some(a) = i.attach.as_mut().filter(|a| a.parent == parent) {
                a.pose.clone_from(pose);
                a.hidden = hidden;
            }
        }
    }

    /// Shut down `parent`'s groups that `keep` refuses (special_weapons.md
    /// Follow-up 1): smooth ones stop emitting and their particles live
    /// out where they are; the others' particles go at once.
    pub fn shut_down(&mut self, particles: Option<&mut Particles>, mut keep: impl FnMut(Entity, &str) -> bool) {
        let mut gone: Vec<u64> = Vec::new();
        let mut fading = Vec::new();
        self.instances.retain_mut(|i| match &i.attach {
            Some(a) if !a.once && !keep(a.parent, &i.group) => {
                if a.smooth {
                    // Its trails stop growing and age out.
                    for run in &mut i.keys {
                        if let Some((params, material)) = run.kit.take() {
                            fading.push((std::mem::take(&mut run.trail), params, material));
                        }
                    }
                } else {
                    gone.push(i.tag);
                }
                false
            }
            _ => true,
        });
        self.fading.extend(fading);
        if let Some(p) = particles {
            for t in gone {
                p.kill_tagged(t);
            }
        }
    }

    /// The camera jitter's amplitude (u) for a camera at `cam` (metres).
    pub fn jitter_at(&self, cam: Vec3) -> f32 {
        self.jitters
            .iter()
            .map(|j| j.amplitude(j.at.distance(cam) / self.unit))
            .sum()
    }

    /// One frame: start and emit keys, age lights and jitters.
    pub fn step(
        &mut self,
        dt: f32,
        lib: &FxLibrary,
        particles: &mut Particles,
        materials: &MapParticles,
        world: &impl WorldTrace,
    ) {
        for p in &mut self.pending {
            p.0 -= dt;
        }
        let (due, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.pending).into_iter().partition(|p| p.0 <= 0.0);
        self.pending = later;
        for (_, name, at, normal) in due {
            let spin = Mat3::from_axis_angle(normal.normalize_or(Vec3::Y), self.rng.float(0.0, std::f32::consts::TAU));
            self.spawn(lib, &name, at, spin * effect_frame(normal));
        }
        for l in &mut self.lights {
            l.age += dt;
        }
        self.lights.retain(|l| l.age <= l.life);
        for j in &mut self.jitters {
            j.age += dt;
        }
        self.jitters.retain(|j| j.age <= j.life);
        let mut instances = std::mem::take(&mut self.instances);
        for inst in &mut instances {
            let Some(g) = lib.group(&inst.group) else { continue };
            if inst.attach.as_ref().is_some_and(|a| a.hidden) {
                // Frozen: no time passes for it, nothing drawn, its trails
                // start over when it is shown again.
                for run in &mut inst.keys {
                    run.trail.clear();
                }
                continue;
            }
            inst.age += dt;
            let length = g.length().max(1e-3);
            let looping = inst.attach.as_ref().is_some_and(|a| a.looping);
            if looping {
                let pass = (inst.age / length) as u32;
                if pass != inst.pass {
                    inst.pass = pass;
                    // A new pass: the keys shorter than the group play again.
                    for (k, run) in g.keys.iter().zip(inst.keys.iter_mut()) {
                        if !whole(k, length) {
                            *run = KeyRun::default();
                        }
                    }
                }
            }
            for (k, run) in g.keys.iter().zip(inst.keys.iter_mut()) {
                if !k.at_detail(self.detail) || shaded(k) {
                    continue;
                }
                // The key's clock: a looping whole-group key's never ends;
                // another key in a looping group runs within its pass.
                let (t, end) = match (looping, whole(k, length)) {
                    (true, true) => (inst.age, f32::INFINITY),
                    (true, false) => (inst.age - inst.pass as f32 * length, k.end),
                    _ => (inst.age, k.end),
                };
                if t < k.start {
                    continue;
                }
                let first = !run.started;
                run.started = true;
                let (base, frame) = place(inst.at, inst.frame, inst.attach.as_ref(), k);
                let attached = inst.attach.is_some();
                let tag = attached.then_some(inst.tag);
                // Keys drawn each frame where the parent is (attached
                // sprites, trails) stop at their end.
                let live = t <= end;
                match k.kind.as_str() {
                    "ParticleSystem" => {
                        let interval = k.float("EmissionInterval").unwrap_or(-1.0);
                        if interval <= 0.0 {
                            if first {
                                self.emit(k, base, frame, lib, particles, materials, world, tag);
                            }
                        } else {
                            if first {
                                run.next_emit = k.start;
                            }
                            // At most a second's worth after a stall.
                            run.next_emit = run.next_emit.max(t - 1.0);
                            while run.next_emit <= t.min(end) {
                                self.emit(k, base, frame, lib, particles, materials, world, tag);
                                run.next_emit += interval;
                            }
                        }
                    }
                    "Sprite" if attached => {
                        if live {
                            let u = key_time(k, t, looping && whole(k, length));
                            self.sprite_frame(k, base, frame, u, lib, particles, materials);
                        }
                    }
                    "Sprite" if first => self.sprite(k, base, frame, lib, particles, materials),
                    "PolyTrail" | "PolyTube" if attached => {
                        let at = base + frame * Vec3::from(k.vector("Offset").unwrap_or_default()) * self.unit;
                        let smooth = inst.attach.as_ref().is_some_and(|a| a.smooth);
                        self.trail(k, run, at, dt, live, smooth, lib, particles, materials);
                    }
                    "DynaLight" if first => self.lights.push(FxLight {
                        at: base + frame * Vec3::from(k.vector("Offset").unwrap_or_default()) * self.unit,
                        age: t - k.start,
                        life: (k.end - k.start).max(1e-3),
                        radius: k.scale_keys(),
                        color: k.color_keys(),
                    }),
                    "CamJitter" if first => self.jitters.push(FxJitter {
                        at: base,
                        age: t - k.start,
                        life: (k.end - k.start).max(1e-3),
                        scale: k.scale_keys(),
                        inner: k.float("InnerRadius").unwrap_or(0.0),
                        outer: k.float("OuterRadius").unwrap_or(0.0),
                    }),
                    "CreateFX" if first => {
                        if let Some(name) = k.string("FXName") {
                            self.pending.push((0.0, name.to_string(), base, frame.z_axis));
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut fading = std::mem::take(&mut self.fading);
        for (trail, params, material) in &mut fading {
            trail.update(params, dt, Vec3::ZERO, TrailMode::ShuttingDown);
            draw_trail(trail, params, *material, particles);
        }
        fading.retain(|(t, _, _)| t.drawn());
        self.fading = fading;
        // A group on its parent lives until shut down (`shut_down`; one
        // that doesn't loop is spent after its length and isn't made
        // again); any other ends with its length.
        instances.retain(|i| i.attach.as_ref().is_some_and(|a| !a.once) || lib.group(&i.group).is_some_and(|g| i.age <= g.length() + 0.05));
        // Groups started while stepping (none: `pending` holds those).
        instances.append(&mut self.instances);
        self.instances = instances;
    }

    /// One emission of a particle system.
    #[allow(clippy::too_many_arguments)]
    fn emit(
        &mut self,
        k: &FxKey,
        at: Vec3,
        frame: Mat3,
        lib: &FxLibrary,
        particles: &mut Particles,
        materials: &MapParticles,
        world: &impl WorldTrace,
        tag: Option<u64>,
    ) {
        let Some(texture) = lib.texture_of(k) else { return };
        let Some(material) = materials.find(&material_name(&texture, additive(k))) else { return };
        let u = self.unit;
        let n = k.int("ParticlesPerEmission").unwrap_or(1).max(0) as usize;
        let offset = Vec3::from(k.vector("Offset").unwrap_or_default());
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
        let gravity = k.float("GravityAcceleration").unwrap_or(0.0);
        let friction = k.float("Friction").unwrap_or(0.0);
        let bounce = k.float("PercentToBounce").unwrap_or(0.0);
        let spin = k.combo("RotateParticles") == Some(1);
        let (w0, w1) = (
            k.float("MinAngularVelocity").unwrap_or(0.0).to_radians(),
            k.float("MaxAngularVelocity").unwrap_or(0.0).to_radians(),
        );
        let splat = k.float("SplatPercent").unwrap_or(0.0);
        let splat_fx = k.string("SplatEffect").filter(|s| !s.is_empty()).map(str::to_string);
        let kill_on_splat = k.combo("KillOnSplat") == Some(1);
        let sizes = k.scale_keys();
        let size = |s: f32| 2.0 * s * u;
        let ramp = match (sizes.first(), sizes.last()) {
            (Some(a), Some(b)) => Ramp::linear(size(a.1), size(b.1)),
            _ => Ramp::constant(size(1.0)),
        };
        let (color, color_fade, alpha, fade) = color_curve(&k.color_keys());
        let mut group = ParticleGroup::new(Motion {
            gravity: -gravity * u,
            damping: if friction > 0.0 && friction < 1.0 { -(1.0 - friction).ln() } else { 0.0 },
            ..default()
        });
        let mut max_speed: f32 = 0.0;
        for _ in 0..n {
            let r = self.rng.float(r0, r1);
            let local = offset
                + match k.combo("Type").unwrap_or(1) {
                    0 => random_dir(&mut self.rng) * r,
                    1 => Vec3::ZERO,
                    _ => {
                        let a = self.rng.float(0.0, std::f32::consts::TAU);
                        let (px, py) = plane.any_orthonormal_pair();
                        (px * a.cos() + py * a.sin()) * r
                    }
                };
            let v = Vec3::new(
                self.rng.float(v0.x, v1.x),
                self.rng.float(v0.y, v1.y),
                self.rng.float(v0.z, v1.z),
            );
            let position = at + frame * local * u;
            let velocity = frame * v * u;
            max_speed = max_speed.max(velocity.length());
            let mut p = Particle::new(position, self.rng.float(l0, l1).max(1e-3), material, 0.0);
            p.velocity = velocity;
            p.size = ramp;
            p.color = color;
            p.color_fade = color_fade;
            p.alpha = alpha;
            p.fade = fade;
            if spin {
                p.roll = self.rng.float(0.0, std::f32::consts::TAU);
                p.roll_speed = self.rng.float(w0, w1) * if self.rng.random() < 0.5 { -1.0 } else { 1.0 };
            }
            if splat > 0.0
                && let Some(fx) = &splat_fx
                && self.rng.random() * 100.0 < splat
                && let Some((t, hit, normal)) = fall(position, velocity, gravity * u, p.life, world)
            {
                self.pending.push((t, fx.clone(), hit, normal));
                if kill_on_splat {
                    p.life = t.max(1e-3);
                }
            }
            group.particles.push(p);
        }
        if bounce > 0.0 {
            group.motion.bounce = Some(Bounce {
                keep: 0.75,
                land_normal_y: 0.7,
                land_speed: 0.0,
                planes: probe_planes(at, None, max_speed, -gravity * u, world),
            });
        }
        group.tag = tag;
        particles.add(group);
    }

    /// A sprite key: one particle for the key's length.
    #[allow(clippy::too_many_arguments)]
    fn sprite(
        &mut self,
        k: &FxKey,
        at: Vec3,
        frame: Mat3,
        lib: &FxLibrary,
        particles: &mut Particles,
        materials: &MapParticles,
    ) {
        let Some(texture) = lib.texture_of(k) else { return };
        let Some(material) = materials.find(&material_name(&texture, additive(k))) else { return };
        let u = self.unit;
        let px = lib.texture_size(&texture).map_or(64.0, |s| s.x as f32);
        let offset = Vec3::from(k.vector("Offset").unwrap_or_default());
        let sizes = k.scale_keys();
        let along = k.combo("Facing") == Some(1);
        // Full width px · k units; a camera-facing sprite's size is half.
        let width = |s: f32| px * s * u * if along { 1.0 } else { 0.5 };
        let ramp = match (sizes.first(), sizes.last()) {
            (Some(a), Some(b)) => Ramp::linear(width(a.1), width(b.1)),
            _ => Ramp::constant(width(1.0)),
        };
        let (color, color_fade, alpha, fade) = color_curve(&k.color_keys());
        let mut p = Particle::new(at + frame * offset * u, (k.end - k.start).max(0.01), material, 0.0);
        p.size = ramp;
        p.color = color;
        p.color_fade = color_fade;
        p.alpha = alpha;
        p.fade = fade;
        if along {
            let normal = frame * Vec3::from(k.vector("Normal").unwrap_or([0.0, 0.0, 1.0]));
            p.shape = Shape::Flat {
                normal: normal.normalize_or(Vec3::Y),
                yaw: self.rng.float(0.0, 360.0),
                yaw_speed: 0.0,
            };
            // Just off the surface.
            p.position += normal.normalize_or(Vec3::Y) * 0.002;
        }
        let mut group = ParticleGroup::new(Motion::default());
        group.particles.push(p);
        particles.add(group);
    }
}

impl CaFx {
    /// An attached sprite key this frame: one particle drawn once at its
    /// place, its scale and colour keys at key time `u`.
    #[allow(clippy::too_many_arguments)]
    fn sprite_frame(&mut self, k: &FxKey, at: Vec3, frame: Mat3, u: f32, lib: &FxLibrary, particles: &mut Particles, materials: &MapParticles) {
        let Some(texture) = lib.texture_of(k) else { return };
        let Some(material) = materials.find(&material_name(&texture, additive(k))) else { return };
        let px = lib.texture_size(&texture).map_or(64.0, |s| s.x as f32);
        let offset = Vec3::from(k.vector("Offset").unwrap_or_default());
        let s = sample_keys(&k.scale_keys(), u, 1.0);
        let c = color_at(&k.color_keys(), u);
        let mut p = Particle::new(at + frame * offset * self.unit, ONE_FRAME, material, px * s * self.unit * 0.5);
        p.color = c.truncate();
        p.alpha = c.w;
        particles.add(one_frame(p));
    }

    /// A trail key (`PolyTrail`, effects_sounds.md Follow-up (2026-10-31)
    /// 1) with the key at `at` (metres): its points (`trail::Trail`),
    /// drawn as strips between them. `live`: the key is running; past its
    /// end the points age out (`smooth` shut-down) or go at once.
    #[allow(clippy::too_many_arguments)]
    fn trail(&mut self, k: &FxKey, run: &mut KeyRun, at: Vec3, dt: f32, live: bool, smooth: bool, lib: &FxLibrary, particles: &mut Particles, materials: &MapParticles) {
        let Some(params) = TrailParams::from_key(k, self.unit) else { return };
        let Some(texture) = lib.texture_of(k) else { return };
        let Some(material) = materials.find(&material_name(&texture, additive(k))) else { return };
        if !live && !smooth {
            run.trail.clear();
            return;
        }
        let mode = if live { TrailMode::Active } else { TrailMode::ShuttingDown };
        run.trail.update(&params, dt, at, mode);
        draw_trail(&run.trail, &params, material, particles);
        run.kit = Some((params, material));
    }
}

/// A trail's strips this frame: a `Shape::Ribbon` per segment, the
/// strip's one half-width, each end's own colour, joints averaged.
fn draw_trail(trail: &Trail, params: &TrailParams, material: usize, particles: &mut Particles) {
    if !trail.drawn() {
        return;
    }
    let width = 2.0 * trail.half_width_m(params.unit);
    let pts = &trail.points;
    let mut group = one_frame_group();
    for i in 0..pts.len() - 1 {
        let (a, b) = (&pts[i], &pts[i + 1]);
        let mut p = Particle::new(a.at, ONE_FRAME, material, 0.0);
        p.shape = Shape::Ribbon {
            to: b.at,
            width: [width, width],
            u: (trail.drawn_u(i), trail.drawn_u(i + 1)),
            end_color: Some(b.color),
            prev: i.checked_sub(1).map(|j| pts[j].at),
            next: pts.get(i + 2).map(|q| q.at),
        };
        p.color = a.color.truncate();
        p.alpha = a.color.w;
        group.particles.push(p);
    }
    particles.add(group);
}

/// A projectile's smoke trail as it goes (special_weapons.md Follow-up
/// 3, effects_sounds.md Follow-up (2026-10-31) 2): its clock, its last
/// point (metres) and its segments.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct SmokeTrail {
    pub kind: u32,
    pub age: f32,
    pub last: Option<Vec3>,
    pub segments: Vec<SmokeSegment>,
}

/// One trail segment: made at `born` (trail clock), emitting puffs for
/// its type's time, then fading as a whole until it is removed; its
/// puffs are the particle groups tagged `tag`.
#[derive(Clone, Debug, PartialEq)]
pub struct SmokeSegment {
    pub born: f32,
    /// The trail clock at its last emission.
    pub last_emit: f32,
    pub tag: u64,
}

impl SmokeSegment {
    /// The segment's own alpha: 1 while it emits, then falling to 0 over
    /// `life` seconds after its last emission.
    pub fn alpha(&self, now: f32, emitting: f32, life: f32) -> f32 {
        if now - self.born < emitting {
            1.0
        } else {
            (1.0 - (now - self.last_emit) / life).clamp(0.0, 1.0)
        }
    }
}

/// A new trail segment every this many seconds.
pub const SMOKE_SEGMENT_INTERVAL: f32 = 0.1;

/// Each puff's total life (s), at the release build's detail.
pub const PUFF_LIFE: f32 = 10.0;

/// Emission steps a segment makes in one update (the release build's).
pub const PUFF_STEPS: usize = 2;

/// A puff's square at its birth is `4 * PUFF_RADIUS / screen width`
/// units wide (the trail's particle radius 2000 over the screen's width
/// in pixels, times 4).
pub const PUFF_RADIUS: f32 = 2000.0;

/// Screen width (pixels) assumed until the front end gives the real one.
pub const DEFAULT_SCREEN_WIDTH: f32 = 1024.0;

impl CaFx {
    /// A projectile's flare this frame (special_weapons.md Follow-up 3):
    /// an unlit camera-facing sprite (`sprite`'s first frame, texture
    /// pixels × `scale` wide) at `at` (metres).
    pub fn flare(&mut self, lib: &FxLibrary, sprite: &str, scale: f32, at: Vec3, particles: &mut Particles, materials: &MapParticles) {
        let Some(texture) = lib.sprites.get(&super::library::normalize(sprite)) else { return };
        let Some(material) = materials.find(&material_name(texture, true)) else { return };
        let px = lib.texture_size(texture).map_or(64.0, |s| s.x as f32);
        particles.add(one_frame(Particle::new(at, ONE_FRAME, material, px * scale * self.unit * 0.5)));
    }

    /// A smoke trail's frame (special_weapons.md Follow-up 3, effects_sounds.md
    /// Follow-up (2026-10-31) 2) with the projectile now at `at` (metres):
    /// a segment starts every 0.1 s and emits on every update for its
    /// type's time; an emitting segment fills the way from the last point
    /// to this one with puffs (`PUFF_STEPS` per update, none at low
    /// detail), ±0.5 u jitter, each grey (150..230 of 255), alpha 0.9 → 0
    /// and scale 1 → 15 over its own `PUFF_LIFE` s, a square of side
    /// 4 · (2000 / screen width) · scale u, drifting 5-6 u/s up and ±5 u/s
    /// along z. After it stops, a segment's whole alpha falls from 1 to 0
    /// over the type's life, then it and its puffs go. Textured with the
    /// type's sprite when the install has it, else plain squares.
    #[allow(clippy::too_many_arguments)]
    pub fn smoke(&mut self, lib: &FxLibrary, trail: &mut SmokeTrail, at: Vec3, dt: f32, particles: &mut Particles, materials: &MapParticles) {
        let look = super::library::smoke_trail(trail.kind);
        let first = trail.last.is_none();
        let from = trail.last.replace(at).unwrap_or(at);
        if first {
            self.next_tag += 1;
            trail.segments.push(SmokeSegment { born: 0.0, last_emit: 0.0, tag: self.next_tag });
        } else {
            trail.age += dt;
        }
        while trail.segments.last().is_some_and(|s| trail.age - s.born >= SMOKE_SEGMENT_INTERVAL - 1e-6) {
            let born = trail.segments.last().map_or(0.0, |s| s.born) + SMOKE_SEGMENT_INTERVAL;
            self.next_tag += 1;
            trail.segments.push(SmokeSegment { born, last_emit: born, tag: self.next_tag });
        }
        let texture = lib.sprites.get(&super::library::normalize(look.sprite)).and_then(|t| materials.find(&material_name(t, false)));
        let material = texture.or_else(|| materials.find(super::library::UNTEXTURED_PUFF));
        let u = self.unit;
        let half = PUFF_RADIUS / self.screen_width.max(1.0) * 2.0 * u;
        let now = trail.age;
        for seg in &mut trail.segments {
            if now - seg.born < look.emitting && !first && self.detail > 0 {
                if let Some(material) = material {
                    seg.last_emit = now;
                    let mut puffs = Vec::new();
                    for i in 0..PUFF_STEPS {
                        let jitter = Vec3::new(self.rng.float(-0.5, 0.5), self.rng.float(-0.5, 0.5), self.rng.float(-0.5, 0.5));
                        let p_at = from.lerp(at, (i + 1) as f32 / PUFF_STEPS as f32) + jitter * u;
                        let mut p = Particle::new(p_at, PUFF_LIFE + i as f32 * dt / PUFF_STEPS as f32, material, 0.0);
                        let grey = self.rng.float(150.0, 230.0) / 255.0;
                        p.color = Vec3::splat(grey);
                        p.alpha = 0.9 * seg.alpha(now, look.emitting, look.life);
                        p.fade = Fade::Ramp(Ramp::linear(1.0, 0.0));
                        p.size = Ramp::linear(half, half * 15.0);
                        p.velocity = Vec3::new(0.0, self.rng.float(5.0, 6.0), self.rng.float(-5.0, 5.0)) * u;
                        puffs.push(p);
                    }
                    match particles.groups.iter_mut().find(|g| g.tag == Some(seg.tag)) {
                        Some(g) => g.particles.extend(puffs),
                        None => {
                            let mut g = ParticleGroup::new(Motion::default());
                            g.capped = false;
                            g.tag = Some(seg.tag);
                            g.particles = puffs;
                            particles.add(g);
                        }
                    }
                }
            } else if let Some(g) = particles.groups.iter_mut().find(|g| g.tag == Some(seg.tag)) {
                // The whole segment fades: each puff's alpha is its own
                // times the segment's.
                let a = 0.9 * seg.alpha(now, look.emitting, look.life);
                for p in &mut g.particles {
                    p.alpha = a;
                }
            }
        }
        trail.segments.retain(|seg| {
            let alive = now - seg.born < look.emitting || now - seg.last_emit < look.life;
            if !alive {
                particles.kill_tagged(seg.tag);
            }
            alive
        });
    }
}

/// A one-frame particle's life (s): drawn on the frame it is added
/// (`ParticleGroup::skip_first`), gone at the next step.
pub const ONE_FRAME: f32 = 1e-6;

/// A group of particles drawn for one frame, not counted against the cap.
fn one_frame_group() -> ParticleGroup {
    let mut g = ParticleGroup::new(Motion::default());
    g.capped = false;
    g
}

fn one_frame(p: Particle) -> ParticleGroup {
    let mut g = one_frame_group();
    g.particles.push(p);
    g
}

/// Tags of attached groups' particles start here (other effects' groups
/// have none).
const TAG_BASE: u64 = 0xCA00_0000_0000;

/// Whether key `k` lasts its whole group (`length` s): it loops in a
/// looping group (effects_sounds.md "The effect file").
pub fn whole(k: &FxKey, length: f32) -> bool {
    k.end - k.start >= length - 0.01
}

/// A key's curve time 0..1 at its clock `t` (a repeating key's taken mod
/// 1: "Key curves").
fn key_time(k: &FxKey, t: f32, repeats: bool) -> f32 {
    let life = (k.end - k.start).max(1e-3);
    let u = (t - k.start) / life;
    if repeats { u.rem_euclid(1.0) } else { u.clamp(0.0, 1.0) }
}

pub(super) fn color_at(keys: &[(f32, [f32; 4])], t: f32) -> Vec4 {
    if keys.is_empty() {
        return Vec4::ONE;
    }
    let ch = |i: usize| sample_keys(&keys.iter().map(|(k, c)| (*k, c[i])).collect::<Vec<_>>(), t, 1.0);
    Vec4::new(ch(0), ch(1), ch(2), ch(3))
}

/// Where key `k` of a group made at `at` with `frame` is placed this frame, before its `Offset`
/// (which turns with the frame), and the frame (effects_sounds.md
/// "Placement modes in detail"): a group not on an entity at its point;
/// on one, fixed (0) at the point it was made, socket or node attach (3,
/// 4) at that socket of the parent (the parent's point when it has none
/// of that name), any other where the parent is; in the parent's frame.
fn place(at: Vec3, frame: Mat3, attach: Option<&Attach>, k: &FxKey) -> (Vec3, Mat3) {
    match attach {
        None => (at, frame),
        Some(a) => {
            let p = &a.pose;
            let base = match k.combo("UpdatePos").unwrap_or(0) {
                0 => at,
                3 | 4 => k.string("AttachName").and_then(|n| p.socket(n)).unwrap_or(p.at),
                _ => p.at,
            };
            (base, p.frame)
        }
    }
}

/// A random unit vector.
fn random_dir(rng: &mut ParticleRng) -> Vec3 {
    loop {
        let v = Vec3::new(rng.float(-1.0, 1.0), rng.float(-1.0, 1.0), rng.float(-1.0, 1.0));
        let l = v.length_squared();
        if l > 1e-4 && l <= 1.0 {
            return v / l.sqrt();
        }
    }
}

/// Colour keys as a particle's colour, colour fade, base alpha and alpha
/// fade: colour from the first key to the last; alpha its largest value,
/// faded in up to the first key that reaches it and out after the last.
pub fn color_curve(keys: &[(f32, [f32; 4])]) -> (Vec3, Option<(Vec3, f32, f32)>, f32, Fade) {
    let Some((t0, c0)) = keys.first().copied() else {
        return (Vec3::ONE, None, 1.0, Fade::Ramp(Ramp::constant(1.0)));
    };
    let (t1, c1) = keys.last().copied().unwrap_or((t0, c0));
    let color = Vec3::new(c0[0], c0[1], c0[2]);
    let end = Vec3::new(c1[0], c1[1], c1[2]);
    let fade_color = (color.distance(end) > 1e-3).then_some((end, t0, t1));
    let amax = keys.iter().map(|k| k.1[3]).fold(0.0, f32::max);
    if amax <= 0.0 {
        return (color, fade_color, 0.0, Fade::Ramp(Ramp::constant(0.0)));
    }
    let first_max = keys.iter().find(|k| k.1[3] >= amax - 1e-4).map_or(t0, |k| k.0);
    let last_max = keys.iter().rev().find(|k| k.1[3] >= amax - 1e-4).map_or(t1, |k| k.0);
    let start_alpha = c0[3] / amax;
    let end_alpha = c1[3] / amax;
    let fade = if end_alpha < 1.0 - 1e-4 && last_max < t1 {
        if end_alpha <= 1e-4 {
            Fade::Windows {
                start_alpha,
                fade_in: (t0, first_max),
                fade_out: (last_max, t1),
            }
        } else {
            Fade::Ramp(Ramp::linear(1.0, end_alpha).between(last_max, t1))
        }
    } else if start_alpha < 1.0 - 1e-4 {
        Fade::Windows {
            start_alpha,
            fade_in: (t0, first_max),
            fade_out: (2.0, 2.0),
        }
    } else {
        Fade::Ramp(Ramp::constant(1.0))
    };
    (color, fade_color, amax, fade)
}

/// Where a particle starting at `p` with velocity `v` (m/s) under gravity
/// `g` (m/s², positive up) first meets the world within `life` seconds:
/// (time, point, normal).
fn fall(p: Vec3, v: Vec3, g: f32, life: f32, world: &impl WorldTrace) -> Option<(f32, Vec3, Vec3)> {
    const STEP: f32 = 1.0 / 30.0;
    let (mut at, mut vel, mut t) = (p, v, 0.0);
    while t < life {
        let next = at + vel * STEP;
        if let Some((f, n)) = world.trace(at, next) {
            return Some((t + STEP * f, at.lerp(next, f), n));
        }
        at = next;
        vel.y += g * STEP;
        t += STEP;
    }
    None
}
