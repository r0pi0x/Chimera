//! An effect's life (specs/halo_ce/effects.md "Creating an effect",
//! "Event timing", "Parts", "Particle entries", "How a particle lives"):
//! a running effect (`Running`) at its location instances steps once a
//! 30 Hz tick (`step`, the spec's float32 phases, the 8-step cap, skip
//! draws), makes its parts when an event starts (sounds, decals traced
//! along the part's velocity, lights) and spreads its particle entries'
//! counts over the event's duration by their distribution function; each
//! particle (`particle`) becomes the engine's: its sprite sequence and
//! frame rate, its tint, its radius growing by the radius animation, its
//! fade in and out as the pool's fade windows, its point physics as the
//! group's motion (gravity by its buoyancy, drag by its friction over its
//! mass at the entry's mean radius, bounces off the level for those that
//! collide). What it makes is collected (`Made`) and written as the
//! engine's requests by the caller.
//!
//! An effect on a holder's gun (`Running::on_gun`) has its instances
//! placed on the gun again each tick, so parts and particles made later
//! in its life start where the marker is then.
//!
//! A particle system part (`system`) is made at once as a free system.
//!
//! Known shortcuts: a free particle system's particles are made at its
//! creation only (its creation rate counted over its birth state; an
//! attached one's rate runs on `emit`, the rocket's exhaust: the
//! flamethrower's stream is made once where it starts), ride the first
//! state's point physics (`system_motion`), and each particle state is its
//! own engine particle on the same path, weighted through the transitions
//! (`system_particle`: size, colour and alpha interpolate, the type's colour
//! multiplier applies; the minimum count's hurrying of the states, the
//! lighting of alpha-blended states and the sprite frames' animation
//! across states are not done); particles attached to a marker are made where the
//! marker was and then move freely; a particle parallel to its direction
//! is a strip of its sprite along its velocity (2R long, shortening over
//! its life as the pool's trails do), or along its direction at creation
//! when it barely moves (it keeps that length, not its radius animation);
//! one perpendicular to it is drawn screen-facing; scale A and B multiply nothing (the disc's
//! weapons' bits only scale tints and spins); the local generator is the
//! engine's particle generator, not Halo's recurrence; environments are
//! all "air" (no water test) and every effect is violent; visibility is
//! not tested; a particle's collision effect and material effect are not
//! made; split-screen division is off (one local player).

use bevy::prelude::*;

use super::{
    HaloLight,
    load::{FxEffect, FxEntry, FxParticle, FxPartKind, FxSysType, FxSystem, HaloFx},
    place::Frame,
};
use super::super::METERS_PER_UNIT;
use crate::map::{
    DecalGroup, ParticleRng, PlaceDecal, PlaySound,
    particles::{Bounce, Fade, Motion, Particle, ParticleCurves, ParticleGroup, Ramp, Shape, WorldTrace, probe_planes},
};

/// One location's instance: as the shooter sees it in first person and as
/// everyone else does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instance {
    pub first: Frame,
    pub third: Frame,
}

impl Instance {
    pub fn both(f: Frame) -> Self {
        Self { first: f, third: f }
    }
}

/// A running effect.
#[derive(Clone, Debug)]
pub struct Running {
    pub effect: usize,
    /// Per location of the effect, its instances (none: a marker the model
    /// lacks; its parts and particles are not made).
    pub instances: Vec<Vec<Instance>>,
    /// The effect belongs to the local player's first-person gun.
    pub first_person: bool,
    pub owner: Option<Entity>,
    /// Place its decals (the authority's job: decals are replicated).
    pub decals: bool,
    /// On a holder's gun: the gun's weapon tag and each location's marker
    /// name, its instances placed again on the gun each tick
    /// (`shots::follow_guns`).
    pub on_gun: Option<(String, Vec<String>)>,
    event: usize,
    in_duration: bool,
    t: f32,
    phase: f32,
    last: f32,
    counts: Vec<u8>,
    pub done: bool,
}

/// What effects made in a step: the engine's requests.
#[derive(Default, Debug)]
pub struct Made {
    pub sounds: Vec<PlaySound>,
    pub decals: Vec<PlaceDecal>,
    pub lights: Vec<HaloLight>,
    pub groups: Vec<ParticleGroup>,
}

/// The running effects and the local generator.
#[derive(Resource)]
pub struct Effects {
    pub running: Vec<Running>,
    pub rng: ParticleRng,
}

impl Default for Effects {
    fn default() -> Self {
        Self { running: Vec::new(), rng: ParticleRng::new(0x4A10) }
    }
}

/// How far behind its point a decal's trace starts, m.
const BACK: f32 = 0.05;

/// A draw in lo..hi (one number used even when they are equal).
fn range(rng: &mut ParticleRng, (lo, hi): (f32, f32)) -> f32 {
    lo + (hi - lo) * rng.random()
}

/// The distribution function G (the spec's table).
pub fn distribution(function: i16, f: f32) -> f32 {
    if f < 0.0 {
        return 0.0;
    }
    match function {
        0 => 1.0,
        1 => {
            if f < 1.0 {
                0.0
            } else {
                1.0
            }
        }
        3 => f * f,
        4 => (2.0 - f) * f,
        5 => (3.0 - 2.0 * f) * f * f,
        _ => f,
    }
}

/// A count drawn from bounds that may be stored upper first.
fn count(rng: &mut ParticleRng, (a, b): (i16, i16)) -> u8 {
    let (lo, hi) = (a.min(b) as f32, a.max(b) as f32);
    (lo + (hi - lo) * rng.random()).trunc().clamp(0.0, 255.0) as u8
}

impl Running {
    /// A new effect at `instances` (its delay drawn: the spec's creation).
    pub fn new(fx: &HaloFx, effect: usize, instances: Vec<Vec<Instance>>, rng: &mut ParticleRng) -> Running {
        let phase = fx.effects[effect].events.first().map_or(0.0, |e| range(rng, e.delay));
        Running {
            effect,
            instances,
            first_person: false,
            owner: None,
            decals: true,
            on_gun: None,
            event: 0,
            in_duration: false,
            t: 0.0,
            phase,
            last: -1.0,
            counts: Vec::new(),
            done: fx.effects[effect].events.is_empty(),
        }
    }

    /// One update of `dt` seconds (0 at creation; the spec's steps).
    pub fn step(&mut self, fx: &HaloFx, dt: f32, rng: &mut ParticleRng, world: &dyn Fn(Vec3, Vec3) -> Option<(f32, Vec3)>, out: &mut Made) {
        let effect = &fx.effects[self.effect];
        let mut dt = dt;
        for _ in 0..8 {
            if self.done || dt < 0.0 {
                return;
            }
            let completed = self.phase - self.t <= dt;
            if completed {
                dt -= self.phase - self.t;
                self.t = self.phase;
            } else {
                self.t += dt;
                dt = -1.0;
            }
            if self.in_duration {
                self.particles(fx, effect, rng, world, out);
                if completed {
                    let mut next = self.event + 1;
                    while next < effect.events.len() && rng.random() < effect.events[next].skip {
                        next += 1;
                    }
                    if next >= effect.events.len() {
                        self.done = true;
                        return;
                    }
                    self.event = next;
                    self.in_duration = false;
                    self.t = 0.0;
                    self.phase = range(rng, effect.events[next].delay);
                }
            } else if completed {
                let ev = &effect.events[self.event];
                self.in_duration = true;
                self.t = 0.0;
                self.last = -1.0;
                self.phase = range(rng, ev.duration);
                self.counts = ev.entries.iter().map(|e| count(rng, e.count)).collect();
                self.parts(fx, effect, rng, world, out);
            }
        }
    }

    fn parts(&self, fx: &HaloFx, effect: &FxEffect, rng: &mut ParticleRng, world: &dyn Fn(Vec3, Vec3) -> Option<(f32, Vec3)>, out: &mut Made) {
        for p in &effect.events[self.event].parts {
            if p.mode == 2 || p.environment == 2 || p.environment == 3 {
                continue;
            }
            let Some(list) = usize::try_from(p.location).ok().and_then(|l| self.instances.get(l)) else { continue };
            for inst in list {
                let mut f = inst.third;
                if p.face_down {
                    f = Frame { at: f.at, forward: Vec3::NEG_Y, up: Vec3::NEG_Z };
                }
                match &p.kind {
                    FxPartKind::Sound(entry) => out.sounds.push(PlaySound { source: self.owner, ..PlaySound::at(entry.clone(), f.at) }),
                    FxPartKind::Decal(group) => {
                        let length = range(rng, p.velocity) * METERS_PER_UNIT;
                        if !self.decals || length <= 0.0 {
                            continue;
                        }
                        // From just behind the point (an impact's point is
                        // on the surface the trace must find).
                        let from = f.at - f.forward * BACK;
                        let to = f.at + f.forward * length;
                        if let Some((s, normal)) = world(from, to) {
                            out.decals.push(PlaceDecal {
                                target: None,
                                group: DecalGroup::Named(group.clone()),
                                point: from.lerp(to, s),
                                normal,
                                dir: f.forward,
                                spin: true,
                                round: false,
                            });
                        }
                    }
                    FxPartKind::Light(l) => out.lights.push(HaloLight {
                        at: f.at,
                        color: Vec3::from(l.color),
                        radius: l.radius * METERS_PER_UNIT,
                        seconds: l.ticks.max(1.0) / 30.0,
                        falloff: l.falloff,
                        owner: self.owner,
                        follow: false,
                    }),
                    FxPartKind::System(i) => {
                        if let Some(sys) = fx.systems.get(*i) {
                            system(sys, &f, rng, out);
                        }
                    }
                    FxPartKind::Other => {}
                }
            }
        }
    }

    fn particles(&mut self, fx: &HaloFx, effect: &FxEffect, rng: &mut ParticleRng, world: &dyn Fn(Vec3, Vec3) -> Option<(f32, Vec3)>, out: &mut Made) {
        let f = if self.phase > 0.0 { (self.t / self.phase).min(1.0) } else { 1.0 };
        let ev = &effect.events[self.event];
        for (i, entry) in ev.entries.iter().enumerate() {
            let n = *self.counts.get(i).unwrap_or(&0) as f32;
            let k = (distribution(entry.distribution, f) * n).trunc() - (distribution(entry.distribution, self.last) * n).trunc();
            if k < 1.0 || entry.environment == 2 || entry.environment == 3 {
                continue;
            }
            // Which instances (the spec's camera modes).
            let first = match entry.camera_mode {
                1 => {
                    if !self.first_person {
                        continue;
                    }
                    true
                }
                2 => {
                    if self.first_person {
                        continue;
                    }
                    false
                }
                3 => self.first_person,
                _ => false,
            };
            let Some(def) = entry.particle.and_then(|p| fx.particles.get(p)) else { continue };
            let Some(list) = usize::try_from(entry.location).ok().and_then(|l| self.instances.get(l)) else { continue };
            for inst in list {
                let frame = if first { inst.first } else { inst.third };
                let mut g = ParticleGroup::new(motion(def, entry, frame.at, world));
                for _ in 0..k as usize {
                    if let Some(p) = particle(def, entry, &frame, rng) {
                        g.particles.push(p);
                    }
                }
                if !g.particles.is_empty() {
                    out.groups.push(g);
                }
            }
        }
        self.last = f;
    }
}

/// A random unit vector.
fn direction(rng: &mut ParticleRng) -> Vec3 {
    loop {
        let v = rng.vec3(-1.0, 1.0);
        let l = v.length_squared();
        if l > 1e-4 && l <= 1.0 {
            return v / l.sqrt();
        }
    }
}

/// `d` turned by a random angle within `cone` about a random axis.
fn in_cone(d: Vec3, cone: f32, rng: &mut ParticleRng) -> Vec3 {
    let a = rng.random() * cone;
    if a == 0.0 {
        return d;
    }
    let axis = d.cross(direction(rng)).normalize_or(d.any_orthonormal_vector());
    Quat::from_axis_angle(axis, a) * d
}

/// One particle of `entry` at instance `frame` (the spec's steps 1 to 8,
/// then creation): the engine's particle (metres).
pub fn particle(def: &FxParticle, entry: &FxEntry, frame: &Frame, rng: &mut ParticleRng) -> Option<Particle> {
    let material = def.material?;
    let e = range(rng, entry.distribution_radius);
    let o = direction(rng);
    let at = frame.point(entry.offset) + o * e * METERS_PER_UNIT;
    let speed = range(rng, entry.velocity);
    let dir = in_cone(frame.vector(entry.direction).normalize_or(frame.forward), entry.cone, rng);
    let radius = range(rng, entry.radius) * METERS_PER_UNIT;
    let spin = range(rng, entry.angular_velocity);
    let roll = if entry.flags & 2 != 0 { rng.random() * std::f32::consts::TAU } else { 0.0 };
    let q = rng.random();
    let (lo, hi) = (entry.tint_lower, entry.tint_upper);
    let color = Vec3::new(lo[1] + (hi[1] - lo[1]) * q, lo[2] + (hi[2] - lo[2]) * q, lo[3] + (hi[3] - lo[3]) * q);
    let alpha = lo[0] + (hi[0] - lo[0]) * q;
    let life = range(rng, def.lifespan).max(1.0 / 30.0);
    let (sequence, frame_loop) = if def.initial > 0 {
        (def.first_sequence + (rng.random() * def.initial as f32) as i16, false)
    } else {
        (def.first_sequence + (rng.random() * def.looping.max(1) as f32) as i16, true)
    };
    let fps = if def.flags & blam_formats::effect::Particle::ANIMATE_ONCE_PER_FRAME != 0 {
        30.0
    } else if def.animation_rate.1 > 0.0 {
        range(rng, def.animation_rate)
    } else {
        0.0
    };
    let fade_in = if def.fade_in > 0.0 { (0.0, (def.fade_in / life).min(1.0)) } else { (0.0, 0.0) };
    let fade_out = if def.fade_out > 0.0 { ((1.0 - def.fade_out / life).max(0.0), 1.0) } else { (2.0, 2.0) };
    // Parallel to its direction: a strip along it through the point, the
    // sprite's width across (its largest side spans 2R).
    let (long, across) = if def.aspect >= 1.0 { (radius, radius / def.aspect) } else { (radius * def.aspect, radius) };
    let v = speed * METERS_PER_UNIT;
    let (shape, at) = if def.orientation == 1 && v > 0.3 {
        // Moving (sparks): a strip along its velocity, 2R long.
        (Shape::Trail { width: 2.0 * across, length: 2.0 * long / v }, at)
    } else if def.orientation == 1 {
        (Shape::Ribbon { to: at + dir * long, width: [2.0 * across; 2], u: (0.0, 1.0), end_color: None, prev: None, next: None }, at - dir * long)
    } else {
        (Shape::Sprite, at)
    };
    Some(Particle {
        shape,
        velocity: dir * speed * METERS_PER_UNIT,
        sequence: sequence.max(0) as u16,
        fps,
        frame_loop,
        color,
        alpha: alpha.clamp(0.0, 1.0),
        fade: Fade::Windows { start_alpha: if def.fade_in > 0.0 { 0.0 } else { 1.0 }, fade_in, fade_out },
        size: Ramp::linear(radius * def.radius_animation.0, radius * def.radius_animation.1),
        roll,
        roll_speed: spin,
        ..Particle::new(at, life, material, radius)
    })
}

/// A free particle system made at `f` (effects.md "Particle systems"):
/// each type's initial count and its birth state's creation rate over the
/// birth's length (at most 128), created by its creation physics (default:
/// at the point; explosion: a random direction scaled by the xy and z
/// spreads, moving at that times the intensity; jet: along the forward,
/// spread by the spread fraction, at the constant velocity).
pub fn system(sys: &FxSystem, f: &Frame, rng: &mut ParticleRng, out: &mut Made) {
    for t in &sys.types {
        if t.states.is_empty() {
            continue;
        }
        let n = (t.initial.max(0) as f32 + t.rate * range(rng, t.birth)).round().clamp(0.0, 128.0) as usize;
        let mut g = ParticleGroup::new(system_motion(t));
        g.capped = false;
        // The type's colour multiplier over its own clock: its first state
        // lasts D, then it moves to the next state's over Tr.
        let (hold, change) = match t.type_states.first() {
            Some(s) if t.type_states.len() > 1 => (range(rng, s.duration), range(rng, s.transition)),
            _ => (f32::INFINITY, 0.0),
        };
        for _ in 0..n {
            let (offset, velocity) = creation(t, f, rng);
            system_particle(t, f.at + offset * METERS_PER_UNIT, velocity * METERS_PER_UNIT, (hold, change), rng, &mut g);
        }
        if !g.particles.is_empty() {
            out.groups.push(g);
        }
    }
}

/// A system's particles for a span of `dt` seconds of its creation rate
/// (an attached system's, the rocket's exhaust: the type's first state is
/// the one that makes them, looping): `rate x dt` particles, the fraction
/// by a draw, at most 128, each at `f` by the creation physics.
pub fn emit(sys: &FxSystem, f: &Frame, dt: f32, rng: &mut ParticleRng, out: &mut Made) {
    for t in &sys.types {
        if t.states.is_empty() || t.rate <= 0.0 {
            continue;
        }
        let x = t.rate * dt;
        let n = (x.floor() + if rng.random() < x.fract() { 1.0 } else { 0.0 }).clamp(0.0, 128.0) as usize;
        let mut g = ParticleGroup::new(system_motion(t));
        g.capped = false;
        for _ in 0..n {
            let (offset, velocity) = creation(t, f, rng);
            system_particle(t, f.at + offset * METERS_PER_UNIT, velocity * METERS_PER_UNIT, (f32::INFINITY, 0.0), rng, &mut g);
        }
        if !g.particles.is_empty() {
            out.groups.push(g);
        }
    }
}

/// A new particle's offset and velocity (WU, WU per second) by the type's
/// creation physics.
fn creation(t: &FxSysType, f: &Frame, rng: &mut ParticleRng) -> (Vec3, Vec3) {
    let c = |i: usize| t.constants.get(i).copied().unwrap_or(0.0);
    match t.creation {
        1 => {
            let d = direction(rng);
            // Halo's z spread is the engine's up (y).
            let o = Vec3::new(d.x * c(0), d.y * c(1), d.z * c(0));
            (o, o * c(2))
        }
        2 => {
            let spread = c(1).clamp(0.0, 1.0);
            (Vec3::ZERO, (f.forward * (1.0 - spread) + direction(rng) * spread) * c(0))
        }
        _ => (Vec3::ZERO, Vec3::ZERO),
    }
}

/// The group's motion from the first particle state's point physics, at
/// the type's radius times the multipliers: buoyancy as gravity (up),
/// friction over mass as the drag (effects.md "Point physics"; the later
/// states' own physics are not followed: the disc's states share the
/// first's until the particle has faded).
fn system_motion(t: &FxSysType) -> Motion {
    let Some(st) = t.states.first() else { return Motion::default() };
    let Some(ph) = &st.physics else { return Motion::default() };
    use blam_formats::effect::PointPhysics as P;
    let mult = |m: f32| if m == 0.0 { 1.0 } else { m };
    let r = (t.radius * mult(t.type_states.first().map_or(1.0, |s| s.radius_multiplier)) * mult(st.radius_multiplier)).max(1e-3);
    let b = if ph.flags & P::NO_GRAVITY != 0 { 0.0 } else { ph.air_buoyancy };
    let mass = (ph.mass_per_r3 + P::AIR_MASS_PER_R3) * r;
    Motion {
        gravity: -P::GRAVITY * b * METERS_PER_UNIT,
        damping: if mass > 0.0 { ph.air_friction / mass } else { 0.0 },
        ..Default::default()
    }
}

/// One particle of a system's type living through its particle states
/// (effects.md "Particle systems", Behaviour): each state lasts its draw,
/// then the particle transitions to the next over its draw (the last
/// state's end is its death); size, colour and alpha follow the states'
/// draws, interpolated through the transitions and multiplied by the type's
/// colour multiplier (`hold`, `change`: seconds of the type's first state
/// and of its change to the next); each state's sprite is one engine
/// particle on the same path, shown with the weight the state has (1 in its
/// state, falling through the transition out, rising through the one in).
fn system_particle(t: &FxSysType, at: Vec3, velocity: Vec3, (hold, change): (f32, f32), rng: &mut ParticleRng, g: &mut ParticleGroup) {
    let n = t.states.len();
    let (mut begin, mut end) = (vec![0.0f32; n], vec![0.0f32; n]);
    let mut clock = 0.0f32;
    for (k, st) in t.states.iter().enumerate() {
        begin[k] = clock;
        end[k] = clock + range(rng, st.duration).max(0.0);
        clock = end[k] + if k + 1 < n { range(rng, st.transition).max(0.0) } else { 0.0 };
    }
    let life = end[n - 1].max(1.0 / 30.0);
    let roll = rng.random() * std::f32::consts::TAU;
    // Each state's draws: radius (metres), colour and alpha, spin.
    let mut draws: Vec<(f32, [f32; 4], f32)> = Vec::with_capacity(n);
    for st in &t.states {
        let radius = range(rng, st.radius) * METERS_PER_UNIT;
        let q = rng.random();
        let (lo, hi) = (st.color_lower, st.color_upper);
        let rgb = |i: usize| lo[i] + (hi[i] - lo[i]) * q;
        let alpha = range(rng, (lo[0].min(hi[0]), lo[0].max(hi[0]))).clamp(0.0, 1.0);
        let spin = range(rng, st.rotation_rate);
        draws.push((radius, [rgb(1), rgb(2), rgb(3), alpha], spin));
    }
    // The type's multiplier (argb) at time `x`.
    let mult = |x: f32| -> [f32; 4] {
        let (Some(a), Some(b)) = (t.type_states.first(), t.type_states.get(1)) else { return [1.0; 4] };
        let u = if change > 0.0 { ((x - hold) / change).clamp(0.0, 1.0) } else if x >= hold { 1.0 } else { 0.0 };
        std::array::from_fn(|i| a.color[i] + (b.color[i] - a.color[i]) * u)
    };
    // The path's size and colour at time `x`.
    let look = |x: f32| -> (f32, [f32; 4]) {
        let mut k = 0;
        while k + 1 < n && x > end[k] && x >= begin[k + 1] {
            k += 1;
        }
        let (r0, c0, _) = draws[k];
        let (size, rgba) = if k + 1 < n && x > end[k] {
            let (r1, c1, _) = draws[k + 1];
            let u = ((x - end[k]) / (begin[k + 1] - end[k]).max(1e-6)).clamp(0.0, 1.0);
            (r0 + (r1 - r0) * u, std::array::from_fn(|i| c0[i] + (c1[i] - c0[i]) * u))
        } else {
            (r0, c0)
        };
        let m = mult(x);
        (size, [rgba[0] * m[1], rgba[1] * m[2], rgba[2] * m[3], rgba[3] * m[0]])
    };
    // The times anything bends at.
    let mut times: Vec<f32> = begin.iter().chain(&end).copied().chain([0.0, life]).collect();
    if hold.is_finite() {
        times.extend([hold, hold + change]);
    }
    times.retain(|x| *x >= 0.0 && *x <= life);
    times.sort_by(f32::total_cmp);
    times.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    let size: Vec<(f32, f32)> = times.iter().map(|x| (x / life, look(*x).0)).collect();
    for (k, st) in t.states.iter().enumerate() {
        let (Some(material), true) = (st.material, st.color_lower[0] > 0.0 || st.color_upper[0] > 0.0) else { continue };
        let weight = |x: f32| -> f32 {
            if x >= begin[k] && x <= end[k] {
                1.0
            } else if k > 0 && x > end[k - 1] && x < begin[k] {
                (x - end[k - 1]) / (begin[k] - end[k - 1]).max(1e-6)
            } else if k + 1 < n && x > end[k] && x < begin[k + 1] {
                1.0 - (x - end[k]) / (begin[k + 1] - end[k]).max(1e-6)
            } else {
                0.0
            }
        };
        let color: Vec<(f32, [f32; 4])> = times
            .iter()
            .map(|x| {
                let (_, c) = look(*x);
                (x / life, [c[0], c[1], c[2], c[3] * weight(*x)])
            })
            .collect();
        let first = color.first().map_or([1.0; 4], |c| c.1);
        g.particles.push(Particle {
            velocity,
            sequence: st.sequence.max(0) as u16,
            fps: range(rng, st.animation_rate),
            frame_loop: true,
            color: Vec3::new(first[0], first[1], first[2]),
            roll,
            roll_speed: draws[k].2,
            curves: Some(std::sync::Arc::new(ParticleCurves { color, size: size.clone(), frames: Vec::new(), frame_time: 0.0 })),
            ..Particle::new(at, life, material, size.first().map_or(0.01, |s| s.1.max(0.001)))
        });
    }
}

/// A particle's age-`a` fade and radius factor as the pool draws them
/// (for tests: the spec's fade and radius animation).
pub fn look_at(def: &FxParticle, life: f32, a: f32) -> (f32, f32) {
    let p = Particle {
        fade: Fade::Windows {
            start_alpha: if def.fade_in > 0.0 { 0.0 } else { 1.0 },
            fade_in: if def.fade_in > 0.0 { (0.0, (def.fade_in / life).min(1.0)) } else { (0.0, 0.0) },
            fade_out: if def.fade_out > 0.0 { ((1.0 - def.fade_out / life).max(0.0), 1.0) } else { (2.0, 2.0) },
        },
        size: Ramp::linear(def.radius_animation.0, def.radius_animation.1),
        age: a,
        ..Particle::new(Vec3::ZERO, life, 0, 1.0)
    };
    (p.current_alpha(), p.current_size())
}

/// The group's motion from the particle's point physics (module doc).
pub fn motion(def: &FxParticle, entry: &FxEntry, at: Vec3, world: &dyn Fn(Vec3, Vec3) -> Option<(f32, Vec3)>) -> Motion {
    let Some(ph) = &def.physics else { return Motion::default() };
    let b = if ph.flags & blam_formats::effect::PointPhysics::NO_GRAVITY != 0 { 0.0 } else { ph.air_buoyancy };
    let r = ((entry.radius.0 + entry.radius.1) / 2.0).max(1e-4);
    let mass = (ph.mass_per_r3 + blam_formats::effect::PointPhysics::AIR_MASS_PER_R3) * r;
    let damping = if mass > 0.0 { ph.air_friction / mass } else { 0.0 };
    let gravity = -blam_formats::effect::PointPhysics::GRAVITY * b * METERS_PER_UNIT;
    let bounce = (ph.flags & blam_formats::effect::PointPhysics::COLLIDES_WITH_STRUCTURES != 0).then(|| {
        struct W<'a>(&'a dyn Fn(Vec3, Vec3) -> Option<(f32, Vec3)>);
        impl WorldTrace for W<'_> {
            fn trace(&self, from: Vec3, to: Vec3) -> Option<(f32, Vec3)> {
                (self.0)(from, to)
            }
        }
        Bounce {
            keep: ph.elasticity,
            land_normal_y: 0.8,
            land_speed: 0.25 * METERS_PER_UNIT,
            planes: probe_planes(at, None, entry.velocity.1 * METERS_PER_UNIT, gravity.max(0.0), &W(world)),
        }
    });
    Motion { gravity, damping, bounce, ..Default::default() }
}

/// Start effect `effect` (an index) of the library.
pub fn start(effects: &mut Effects, fx: &HaloFx, effect: usize, instances: Vec<Vec<Instance>>, setup: impl FnOnce(&mut Running)) -> usize {
    let mut r = Running::new(fx, effect, instances, &mut effects.rng);
    setup(&mut r);
    effects.running.push(r);
    effects.running.len() - 1
}
