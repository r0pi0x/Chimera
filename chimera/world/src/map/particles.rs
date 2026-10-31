//! Particles for any game: a CPU particle pool of camera-facing sprites,
//! trails (strips along the velocity) and flat quads, in groups that share
//! a motion rule (gravity, decay, damping, drag, bounce planes). Games
//! build groups with their own numbers and add them to `Particles`; the
//! pool steps them once per rendered frame and `render::particles`
//! draws them.
//!
//! The rules follow the "old style" client particles of
//! specs/cs_source/impact_effects.md section 4 (time step clamp, near
//! fade, bounce planes found once per group, the particle cap), which are
//! general enough for other games' simple effects. Engine space: meters,
//! Y up.
//!
//! Particles for any game: `Particles` (a CPU pool of groups that share a
//! motion rule: gravity, decay, damping, drag, bounce planes probed once per
//! group; a cap, a frame-step clamp, near fade, merging), `WorldTracer` (traces
//! against the world only), `MapParticles` materials (texture, blend, vertex
//! colour/alpha, sprite sheet, a sprite card's screen-size limits
//! `ScreenSize`), rolls that stop at an age and mirrored sprites, `Shape::Tumble` (3D shards: three spinning
//! angles, `tumble_axes`, settling flat, `settle_flat`); `ParticleSet`
//! (Step, Draw) lets games show
//! one-frame groups (smoke clouds) between stepping and drawing;
//! `Shape::Ribbon` (camera-facing trail sections) and tagged groups an
//! effect can cut at once (`ParticleGroup::tag`, `kill_tagged`).

use bevy::prelude::*;

use super::ParticleRng;

/// How a particle material is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ParticleBlend {
    /// Blended by alpha over the scene.
    #[default]
    Alpha,
    /// Added to the scene (colour x alpha).
    Additive,
}

/// A material particles can use: a texture, its blend, and which vertex
/// attributes it honours.
#[derive(Clone, Debug, Default)]
pub struct ParticleMaterial {
    pub name: String,
    /// Index into `MapData::textures`.
    pub texture: Option<usize>,
    pub blend: ParticleBlend,
    /// The particle's colour tints the texture (else white).
    pub vertex_color: bool,
    /// The particle's alpha fades it (else always opaque).
    pub vertex_alpha: bool,
    /// Drawn over everything (the game ignores depth). Not supported yet:
    /// drawn depth tested.
    pub no_depth: bool,
    /// Sprite sheet: sequences of frames, each frame a texture rectangle
    /// (min u, min v, max u, max v). Empty: the whole texture.
    pub sequences: Vec<Vec<[f32; 4]>>,
    /// Colour multiplier (sprite cards' `$overbrightfactor`; 0: none
    /// given, 1). Particle systems apply it (`psys`); hand-built effects
    /// use their own.
    pub overbright: f32,
    /// Sprite cards' screen-size limits (`ScreenSize`), for sprites.
    pub screen: ScreenSize,
}

/// A sprite card's limits on a sprite's size on screen
/// (specs/cs_source/impact_effects.md 14: `$maxsize` caps a sprite's
/// size to that fraction of the screen; `$startfadesize`/`$endfadesize`
/// fade it out as its screen size grows from the first fraction to the
/// second). A sprite's screen size is its width (twice its half-width)
/// over the view's height at its depth (our reading: the spec says "of
/// the screen"). Zero: no limit.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScreenSize {
    pub max: f32,
    pub fade_start: f32,
    pub fade_end: f32,
}

impl ScreenSize {
    /// No limit at all.
    pub fn is_none(&self) -> bool {
        self.max <= 0.0 && self.fade_end <= 0.0
    }

    /// A sprite of half-width `size` at view depth `depth` (both meters)
    /// under a vertical field of view whose half-angle has tangent
    /// `tan_half_fov`: its half-width capped by `max` and the alpha
    /// factor of the fade (1 below `fade_start`, 0 from `fade_end`,
    /// linear between: ours, the spec names no curve). The fade looks at
    /// the size before the cap.
    pub fn apply(&self, size: f32, depth: f32, tan_half_fov: f32) -> (f32, f32) {
        if self.is_none() || size <= 0.0 {
            return (size, 1.0);
        }
        // The view's height at this depth; at or behind the eye, any
        // sprite fills the screen.
        let view = 2.0 * depth.max(1e-4) * tan_half_fov.max(1e-4);
        let screen = 2.0 * size / view;
        let alpha = if self.fade_end > 0.0 {
            if self.fade_end > self.fade_start {
                1.0 - ((screen - self.fade_start) / (self.fade_end - self.fade_start)).clamp(0.0, 1.0)
            } else if screen >= self.fade_end {
                0.0
            } else {
                1.0
            }
        } else {
            1.0
        };
        let size = if self.max > 0.0 && screen > self.max {
            size * self.max / screen
        } else {
            size
        };
        (size, alpha)
    }
}

/// The particle materials a map's game uses, found by name.
#[derive(Clone, Debug, Default)]
pub struct MapParticles {
    pub materials: Vec<ParticleMaterial>,
}

impl MapParticles {
    pub fn find(&self, name: &str) -> Option<usize> {
        self.materials.iter().position(|m| m.name.eq_ignore_ascii_case(name))
    }
}

/// When the pool steps and draws each frame (`Update`): games that show
/// a group for one frame only (rebuilt every frame, e.g. a smoke cloud)
/// add it between the two, with `kill_after: Some(0.0)`.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParticleSet {
    Step,
    Draw,
}

/// The loaded map's particle materials, for games to find theirs.
#[derive(Resource, Clone, Debug, Default)]
pub struct ParticleMaterials(pub MapParticles);

/// `bias(t, b) = t^(ln b / ln 0.5)`: `bias(0.5, b) = b`; 0.5 is linear.
pub fn bias(t: f32, b: f32) -> f32 {
    if (b - 0.5).abs() < 1e-6 || t <= 0.0 {
        return t.max(0.0);
    }
    t.powf(b.ln() / 0.5f32.ln())
}

/// A value going from `start` to `end` over a particle's life (between
/// life fractions `from` and `to`, usually 0 and 1), through `bias`
/// (0.5: linear).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ramp {
    pub start: f32,
    pub end: f32,
    pub bias: f32,
    pub from: f32,
    pub to: f32,
}

impl Ramp {
    pub fn constant(v: f32) -> Self {
        Self::linear(v, v)
    }

    pub fn linear(start: f32, end: f32) -> Self {
        Self {
            start,
            end,
            bias: 0.5,
            from: 0.0,
            to: 1.0,
        }
    }

    pub fn with_bias(self, bias: f32) -> Self {
        Self { bias, ..self }
    }

    /// Only between life fractions `from` and `to`.
    pub fn between(self, from: f32, to: f32) -> Self {
        Self { from, to, ..self }
    }

    /// The value at life fraction `t`.
    pub fn at(&self, t: f32) -> f32 {
        let x = if self.to > self.from {
            ((t - self.from) / (self.to - self.from)).clamp(0.0, 1.0)
        } else if t >= self.from {
            1.0
        } else {
            0.0
        };
        self.start + (self.end - self.start) * bias(x, self.bias)
    }
}

/// How a particle's alpha changes with age (times its base alpha).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fade {
    Ramp(Ramp),
    /// `r = 1 - age/life`: `r` while `r >= 0.75`, then `r²` (the old-style
    /// dust puff).
    Dust,
    /// Full until the last `secs` seconds, then fading linearly.
    Tail(f32),
    /// `(1 - age/life)^p` (Source's `Bias(1 - t, b)` is this with
    /// `p = ln b / ln 0.5`).
    Power(f32),
    /// Fade in from `start_alpha` to 1 between life fractions `fade_in`,
    /// out to 0 between `fade_out` (particle-system alpha fade). An empty
    /// fade-in window holds `start_alpha` until it.
    Windows {
        start_alpha: f32,
        fade_in: (f32, f32),
        fade_out: (f32, f32),
    },
}

impl Fade {
    pub fn at(&self, age: f32, life: f32) -> f32 {
        let t = if life > 0.0 { age / life } else { 1.0 };
        match *self {
            Fade::Ramp(r) => r.at(t),
            Fade::Dust => {
                let r = (1.0 - t).max(0.0);
                if r >= 0.75 { r } else { r * r }
            }
            Fade::Tail(secs) => ((life - age) / secs).clamp(0.0, 1.0),
            Fade::Power(p) => (1.0 - t).clamp(0.0, 1.0).powf(p),
            Fade::Windows {
                start_alpha,
                fade_in,
                fade_out,
            } => {
                let window = |(a, b): (f32, f32)| {
                    if b > a {
                        ((t - a) / (b - a)).clamp(0.0, 1.0)
                    } else if t >= a {
                        1.0
                    } else {
                        0.0
                    }
                };
                let fade_in = start_alpha + (1.0 - start_alpha) * window(fade_in);
                let fade_out = 1.0 - window(fade_out);
                fade_in * fade_out
            }
        }
    }
}

/// What a particle looks like.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// A square facing the camera; `size` is its half-width.
    Sprite,
    /// A strip facing the camera from the particle along `velocity x
    /// length x max(0.01, 1 - age/life)` (`length` in seconds), `width`
    /// wide at most (meters).
    Trail { width: f32, length: f32 },
    /// A square in the plane with `normal`, turned `yaw` degrees about it;
    /// `size` is its full side.
    Flat { normal: Vec3, yaw: f32, yaw_speed: f32 },
    /// A square tumbling in 3D (old-style shards): turned by `angles`
    /// (pitch, yaw, roll in degrees, `tumble_axes`), its face across the
    /// turned side and up axes (the forward axis is its normal), one corner
    /// on the particle's position; `size` is its full side. All three
    /// angles turn at the particle's `roll_speed`, here in degrees a
    /// second (a bounce scales it by -0.25 and landing stops it, as for
    /// rolls). `settle`: on a frame it bounced, and every frame once it has
    /// stopped spinning, it is pulled toward lying flat (`settle_flat`).
    Tumble { angles: Vec3, settle: bool },
    /// A quad with these corners (bottom-left, top-left, top-right,
    /// bottom-right; engine space) showing this texture rectangle (min u,
    /// min v, max u, max v): a segment of a trail.
    Quad { corners: [Vec3; 4], rect: [f32; 4] },
    /// A strip facing the camera from the particle's position to `to`
    /// (engine space), `width` wide at each end (meters; the width runs
    /// across the strip, square to the line and to the eye), showing the
    /// texture's u from `u.0` at the particle to `u.1` at `to` and v
    /// across: a section of a ribbon trail.
    Ribbon { to: Vec3, width: [f32; 2], u: (f32, f32) },
    /// A bullet tracer (specs/cs_source/tracers.md 3.1): a streak `length`
    /// long whose head leaves `start` along `dir` at `speed` (m/s), both
    /// ends kept between `start` and `distance` along; a camera-facing core
    /// of half-width `width` and a dim outline twice as wide.
    Streak {
        start: Vec3,
        dir: Vec3,
        distance: f32,
        length: f32,
        width: f32,
        speed: f32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Particle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub age: f32,
    pub life: f32,
    /// Index into `MapParticles::materials`.
    pub material: usize,
    /// Sprite-sheet sequence, and its frame rate (0: first frame only).
    pub sequence: u16,
    pub fps: f32,
    /// The sequence starts over after its last frame (else holds it).
    pub frame_loop: bool,
    /// sRGB, 0-1 (above 1: overbright, for additive materials).
    pub color: Vec3,
    /// Blend toward this colour between these life fractions
    /// (particle-system colour fade).
    pub color_fade: Option<(Vec3, f32, f32)>,
    /// Base alpha, 0-1, times `fade`.
    pub alpha: f32,
    pub fade: Fade,
    /// Meters (see `Shape`).
    pub size: Ramp,
    /// Radians and radians per second.
    pub roll: f32,
    pub roll_speed: f32,
    /// Age (seconds) at which the roll stops turning (a particle system's
    /// spin stop time); infinite: never.
    pub roll_stop: f32,
    /// Drawn mirrored left to right (a particle system's yaw flip).
    pub flip: bool,
    pub shape: Shape,
}

impl Particle {
    /// A white, opaque, unmoving sprite of half-width `size`.
    pub fn new(position: Vec3, life: f32, material: usize, size: f32) -> Self {
        Self {
            position,
            velocity: Vec3::ZERO,
            age: 0.0,
            life,
            material,
            sequence: 0,
            fps: 0.0,
            frame_loop: false,
            color: Vec3::ONE,
            color_fade: None,
            alpha: 1.0,
            fade: Fade::Ramp(Ramp::constant(1.0)),
            size: Ramp::constant(size),
            roll: 0.0,
            roll_speed: 0.0,
            roll_stop: f32::INFINITY,
            flip: false,
            shape: Shape::Sprite,
        }
    }

    pub fn current_alpha(&self) -> f32 {
        (self.alpha * self.fade.at(self.age, self.life)).clamp(0.0, 1.0)
    }

    pub fn current_color(&self) -> Vec3 {
        match self.color_fade {
            Some((to, from_t, to_t)) => {
                let t = if self.life > 0.0 { self.age / self.life } else { 1.0 };
                let x = if to_t > from_t {
                    ((t - from_t) / (to_t - from_t)).clamp(0.0, 1.0)
                } else if t >= from_t {
                    1.0
                } else {
                    0.0
                };
                self.color.lerp(to, x)
            }
            None => self.color,
        }
    }

    pub fn current_size(&self) -> f32 {
        self.size.at(if self.life > 0.0 { self.age / self.life } else { 1.0 })
    }
}

/// Collision against a few planes found when the group was made (the
/// old-style "bounce planes"): particles crossing one re-trace the world
/// and bounce off or land on what they hit.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bounce {
    /// Speed kept after a bounce: a fresh draw in `keep ± 0.1` each time.
    pub keep: f32,
    /// A hit surface with normal y at least this, approached slower than
    /// `land_speed` (m/s, vertical), stops the particle dead.
    pub land_normal_y: f32,
    pub land_speed: f32,
    /// Plane normal and distance (`n·p = d`).
    pub planes: Vec<(Vec3, f32)>,
}

/// How a group's particles move each frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Motion {
    /// Downward acceleration, m/s² (negative: upward).
    pub gravity: f32,
    /// Velocity decays by `10^(-4·dt/time)` each frame, then is raised to
    /// at least `min_speed` along its pre-decay direction (old-style dust).
    pub decay: Option<(f32, f32)>,
    /// Velocity times `max(0, 1 - damping·dt)` each frame, after moving.
    pub damping: f32,
    /// Fraction of velocity removed per frame, before moving
    /// (particle-system drag).
    pub drag: f32,
    pub bounce: Option<Bounce>,
    /// Roll speed `ω += ω·(-rate·dt)`, then kept at least `floor` in size.
    pub roll_decay: Option<(f32, f32)>,
    /// Sprites fade in from distance `.0` to `.1` (meters) in front of the
    /// camera plane.
    pub near_fade: Option<(f32, f32)>,
    /// A liquid surface at this height (meters): sprites fade as they
    /// sink into it (alpha × the part of their half-size still above it)
    /// and go once wholly below.
    pub sink: Option<f32>,
}

impl Motion {
    /// Draw factor for a sprite of half-size `size` at `p` (near fade and
    /// sinking), given its depth in front of the camera.
    pub fn draw_factor(&self, depth: f32, p: &Particle) -> f32 {
        let near = self
            .near_fade
            .map_or(1.0, |(a, b)| ((depth - a) / (b - a)).clamp(0.0, 1.0));
        let sink = self.sink.map_or(1.0, |y| {
            let half = (p.current_size() / 2.0).max(1e-6);
            ((p.position.y - (y - half)) / half).clamp(0.0, 1.0)
        });
        near * sink
    }
}

/// Particles that move and are cut together.
#[derive(Clone, Debug, Default)]
pub struct ParticleGroup {
    pub particles: Vec<Particle>,
    pub motion: Motion,
    /// Counts against (and is cut to) the pool's cap.
    pub capped: bool,
    /// Not moved on the frame it is added (drawn at its spawn state once).
    pub skip_first: bool,
    /// The whole group goes this long after it was added, seconds.
    pub kill_after: Option<f32>,
    /// Joins a live group with the same key if their boxes together stay
    /// under `extent` across: (key, extent in meters, spawn box min, max).
    pub merge: Option<(u32, f32, Vec3, Vec3)>,
    /// Who made it, so it can be cut at once (`Particles::kill_tagged`:
    /// an effect shut down without letting its particles live out).
    pub tag: Option<u64>,
    age: f32,
    fresh: bool,
}

impl ParticleGroup {
    pub fn new(motion: Motion) -> Self {
        Self {
            motion,
            capped: true,
            skip_first: true,
            ..default()
        }
    }

    fn bounds(&self) -> Option<(Vec3, Vec3)> {
        self.particles.iter().fold(None, |acc, p| match acc {
            None => Some((p.position, p.position)),
            Some((lo, hi)) => Some((lo.min(p.position), hi.max(p.position))),
        })
    }
}

/// A world trace for bounce planes: the fraction along `from -> to` of the
/// first world surface hit and its normal; fraction 0 when `from` is in
/// solid.
pub trait WorldTrace {
    fn trace(&self, from: Vec3, to: Vec3) -> Option<(f32, Vec3)>;
}

impl<F: Fn(Vec3, Vec3) -> Option<(f32, Vec3)>> WorldTrace for F {
    fn trace(&self, from: Vec3, to: Vec3) -> Option<(f32, Vec3)> {
        self(from, to)
    }
}

/// No world: nothing to bounce off.
pub struct NoWorld;

impl WorldTrace for NoWorld {
    fn trace(&self, _: Vec3, _: Vec3) -> Option<(f32, Vec3)> {
        None
    }
}

/// Plane tolerance for crossing tests (0.01 game units).
const PLANE_EPSILON: f32 = 0.01 * 0.0254;

/// Find bounce planes from `origin`: along `aim` and its right vector and
/// the opposite (or ±X, ±Y, ±Z without an aim), each probe 8 steps of
/// 0.25 s at `speed` m/s, falling `½·g·(0.25·i)²` per step (spec section
/// 4: the drop uses the cumulative time each step, so the path falls
/// faster than a parabola). The first hit of each probe adds its plane.
pub fn probe_planes(
    origin: Vec3,
    aim: Option<Vec3>,
    speed: f32,
    gravity: f32,
    world: &impl WorldTrace,
) -> Vec<(Vec3, f32)> {
    const STEPS: u32 = 8;
    const STEP: f32 = 0.25;
    let dirs: Vec<Vec3> = match aim.and_then(|a| a.try_normalize()) {
        Some(a) => {
            let right = a.cross(Vec3::Y).try_normalize().unwrap_or(Vec3::X);
            vec![a, right, -right]
        }
        None => vec![Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z],
    };
    let mut planes: Vec<(Vec3, f32)> = Vec::new();
    for dir in dirs {
        let mut from = origin;
        for i in 1..=STEPS {
            let t = STEP * i as f32;
            let mut to = from + dir * speed * STEP;
            to.y -= 0.5 * gravity * t * t;
            if let Some((f, n)) = world.trace(from, to) {
                let at = from.lerp(to, f);
                let plane = (n, n.dot(at));
                if !planes
                    .iter()
                    .any(|(pn, pd)| pn.abs_diff_eq(n, 1e-3) && (pd - plane.1).abs() < 1e-3)
                {
                    planes.push(plane);
                }
                break;
            }
            from = to;
        }
    }
    planes
}

/// Every live particle group. Games add groups; the map steps and draws
/// them each rendered frame.
#[derive(Resource, Debug)]
pub struct Particles {
    pub groups: Vec<ParticleGroup>,
    /// Most capped particles alive at once; spawns past it are dropped.
    pub cap: usize,
    /// Largest time step, seconds (longer frames advance only this much).
    pub max_dt: f32,
    pub rng: ParticleRng,
}

impl Default for Particles {
    fn default() -> Self {
        Self {
            groups: Vec::new(),
            cap: 2048,
            max_dt: 0.1,
            rng: ParticleRng::new(0x5EED),
        }
    }
}

impl Particles {
    /// Live particles that count against the cap.
    pub fn capped_count(&self) -> usize {
        self.groups.iter().filter(|g| g.capped).map(|g| g.particles.len()).sum()
    }

    pub fn count(&self) -> usize {
        self.groups.iter().map(|g| g.particles.len()).sum()
    }

    /// Add a group, cut to the room left under the cap (the rest are
    /// dropped silently, in order). Returns how many particles were added.
    pub fn add(&mut self, mut group: ParticleGroup) -> usize {
        if group.capped {
            let room = self.cap.saturating_sub(self.capped_count());
            group.particles.truncate(room);
        }
        let n = group.particles.len();
        if n == 0 {
            return 0;
        }
        group.fresh = group.skip_first;
        group.age = 0.0;
        if let Some((key, extent, lo, hi)) = group.merge {
            let target = self.groups.iter_mut().find(|g| {
                g.merge.is_some_and(|m| m.0 == key)
                    && g.bounds()
                        .is_some_and(|(glo, ghi)| glo.min(lo).distance(ghi.max(hi)) < extent)
            });
            if let Some(g) = target {
                // The merged group takes the new impact's bounce planes.
                g.particles.append(&mut group.particles);
                g.motion.bounce = group.motion.bounce;
                return n;
            }
        }
        self.groups.push(group);
        n
    }

    /// Remove every group tagged `tag` at once.
    pub fn kill_tagged(&mut self, tag: u64) {
        self.groups.retain(|g| g.tag != Some(tag));
    }

    /// Advance everything by the frame time `dt` (clamped to `max_dt`).
    pub fn step(&mut self, dt: f32, world: &impl WorldTrace) {
        let dt = dt.clamp(0.0, self.max_dt);
        let rng = &mut self.rng;
        for g in &mut self.groups {
            g.age += dt;
            if std::mem::take(&mut g.fresh) {
                continue;
            }
            let motion = &g.motion;
            g.particles.retain_mut(|p| {
                step_particle(p, motion, dt, world, rng);
                let sunk = motion.sink.is_some_and(|y| p.position.y + p.current_size() < y);
                p.age < p.life && !sunk
            });
        }
        self.groups
            .retain(|g| !g.particles.is_empty() && g.kill_after.is_none_or(|k| g.age < k));
    }
}

/// One particle's frame: velocity rule, position, age, roll.
pub fn step_particle(p: &mut Particle, m: &Motion, dt: f32, world: &impl WorldTrace, rng: &mut ParticleRng) {
    let moving = !(m.bounce.is_some() && p.velocity == Vec3::ZERO);
    let mut struck = false;
    if moving {
        if let Some((time, floor)) = m.decay {
            let before = p.velocity;
            p.velocity *= (1e-4f32.ln() * dt / time).exp();
            if p.velocity.length() < floor {
                p.velocity = before.normalize_or_zero() * floor;
            }
        }
        p.velocity.y -= m.gravity * dt;
        if m.drag > 0.0 {
            p.velocity *= (1.0 - m.drag).max(0.0);
        }
        match &m.bounce {
            Some(b) => struck = bounce_move(p, b, dt, world, rng),
            None => p.position += p.velocity * dt,
        }
        if m.damping > 0.0 {
            p.velocity *= (1.0 - m.damping * dt).max(0.0);
        }
    }
    // The roll turns only until its stop time.
    let turning = (p.roll_stop - p.age).clamp(0.0, dt);
    p.age += dt;
    p.roll += p.roll_speed * turning;
    if let Some((rate, floor)) = m.roll_decay {
        p.roll_speed += p.roll_speed * (-rate * dt);
        if p.roll_speed.abs() < floor {
            p.roll_speed = floor.copysign(p.roll_speed);
        }
    }
    match &mut p.shape {
        Shape::Flat { yaw, yaw_speed, .. } => *yaw = (*yaw + *yaw_speed * dt).rem_euclid(360.0),
        Shape::Tumble { angles, settle } => {
            *angles += Vec3::splat(p.roll_speed * dt);
            if *settle && (struck || p.roll_speed == 0.0) {
                settle_flat(angles);
            }
        }
        _ => {}
    }
}

/// The axes a tumbling square's `angles` (pitch, yaw, roll, degrees)
/// turn it to, in engine space: (forward, side, up). Source's convention
/// in its axes (X forward, Y left, Z up: yaw about Z, then pitch about Y,
/// positive down, then roll about X), mapped to engine space (Source Z is
/// our Y); at zero angles forward is +X, the side (Source's left) -Z, up +Y.
pub fn tumble_axes(angles: Vec3) -> (Vec3, Vec3, Vec3) {
    let q = Quat::from_rotation_z(angles.y.to_radians())
        * Quat::from_rotation_y(angles.x.to_radians())
        * Quat::from_rotation_x(angles.z.to_radians());
    let engine = |v: Vec3| Vec3::new(v.x, v.z, -v.y);
    (engine(q * Vec3::X), engine(q * Vec3::Y), engine(q * Vec3::Z))
}

/// One frame's pull of a shard toward lying flat (specs/cs_source/
/// impact_effects.md 9): pitch `p <- 0.5p + 46` below 180 (settling at
/// 92), else `p <- 0.5p + 135` (settling at 270), unless already within
/// 0.5 of 90 or 270; the yaw becomes half the roll while the roll is over
/// 0.5 (a quirk, kept).
pub fn settle_flat(angles: &mut Vec3) {
    let p = angles.x;
    if p < 180.0 {
        if (p - 90.0).abs() > 0.5 {
            angles.x = 0.5 * p + 46.0;
        }
    } else if (p - 270.0).abs() > 0.5 {
        angles.x = 0.5 * p + 135.0;
    }
    if angles.z.abs() > 0.5 {
        angles.y = 0.5 * angles.z;
    }
}

/// Move with bounce planes (velocity already has this frame's gravity);
/// true when it hit something this frame.
fn bounce_move(p: &mut Particle, b: &Bounce, dt: f32, world: &impl WorldTrace, rng: &mut ParticleRng) -> bool {
    let to = p.position + p.velocity * dt;
    let crosses = b.planes.iter().any(|(n, d)| {
        let (a, z) = (n.dot(p.position) - d, n.dot(to) - d);
        a >= -PLANE_EPSILON && z < PLANE_EPSILON
    });
    let hit = if crosses { world.trace(p.position, to) } else { None };
    match hit {
        Some((f, _)) if f <= 0.0 => {
            p.velocity = Vec3::ZERO;
            p.roll_speed = 0.0;
            true
        }
        Some((f, n)) => {
            let draw = rng.float(b.keep - 0.1, b.keep + 0.1);
            bounce(p, f, n, dt, b, draw);
            true
        }
        None => {
            p.position = to;
            false
        }
    }
}

/// A particle hitting a surface with normal `n` at fraction `f` of this
/// frame's move: it lands (stops) on floors when slow enough, else
/// reflects keeping `keep` of its speed.
pub fn bounce(p: &mut Particle, f: f32, n: Vec3, dt: f32, b: &Bounce, keep: f32) {
    p.position += p.velocity * (f - 0.01) * dt;
    if n.y >= b.land_normal_y && p.velocity.y.abs() <= b.land_speed {
        p.velocity = Vec3::ZERO;
        p.roll_speed = 0.0;
    } else {
        p.velocity = (p.velocity - 2.0 * p.velocity.dot(n) * n) * keep;
        p.roll_speed *= -0.25;
    }
}

/// Traces against the world only: the map's static brushes and surfaces,
/// not props, characters or anything that moves (what bounce planes and
/// the game's "world brushes" traces see).
#[derive(bevy::ecs::system::SystemParam)]
pub struct WorldTracer<'w, 's> {
    query: avian3d::prelude::SpatialQuery<'w, 's>,
    statics: Query<'w, 's, &'static avian3d::prelude::RigidBody, Without<super::MapPropCollider>>,
}

impl WorldTrace for WorldTracer<'_, '_> {
    fn trace(&self, from: Vec3, to: Vec3) -> Option<(f32, Vec3)> {
        let d = to - from;
        let len = d.length();
        let dir = Dir3::new(d).ok()?;
        let filter = avian3d::prelude::SpatialQueryFilter::default().with_mask(crate::core::NOT_SHADOW);
        self.query
            .cast_ray_predicate(from, dir, len, true, &filter, &|e| {
                self.statics.get(e).is_ok_and(|b| b.is_static())
            })
            .map(|h| (h.distance / len, h.normal))
    }
}

/// Step the pool by the frame time against the world.
pub fn step_particles(time: Res<Time>, mut particles: ResMut<Particles>, world: WorldTracer) {
    if particles.groups.is_empty() {
        return;
    }
    particles.step(time.delta_secs(), &world);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moving_group(n: usize) -> ParticleGroup {
        let mut g = ParticleGroup::new(Motion::default());
        for _ in 0..n {
            let mut p = Particle::new(Vec3::ZERO, 10.0, 0, 1.0);
            p.velocity = Vec3::X;
            g.particles.push(p);
        }
        g
    }

    #[test]
    fn cap_cuts_new_groups() {
        let mut pool = Particles::default();
        assert_eq!(pool.add(moving_group(2000)), 2000);
        assert_eq!(pool.add(moving_group(100)), 48);
        assert_eq!(pool.add(moving_group(10)), 0);
        assert_eq!(pool.capped_count(), 2048);
        // Uncapped groups (particle systems) still go in.
        let mut free = moving_group(10);
        free.capped = false;
        assert_eq!(pool.add(free), 10);
    }

    #[test]
    fn long_frames_advance_at_most_max_dt_and_new_groups_wait_a_frame() {
        let mut pool = Particles::default();
        pool.add(moving_group(1));
        pool.step(0.25, &NoWorld);
        // Created this frame: drawn once where it spawned.
        assert_eq!(pool.groups[0].particles[0].position, Vec3::ZERO);
        pool.step(0.25, &NoWorld);
        let p = &pool.groups[0].particles[0];
        assert!((p.position.x - 0.1).abs() < 1e-6 && (p.age - 0.1).abs() < 1e-6);
        // A pool group moves on its first frame.
        let mut g = moving_group(1);
        g.skip_first = false;
        pool.add(g);
        pool.step(0.05, &NoWorld);
        assert!((pool.groups[1].particles[0].position.x - 0.05).abs() < 1e-6);
    }

    #[test]
    fn nearby_groups_merge_and_take_new_planes() {
        let mut pool = Particles::default();
        let with = |x: f32, plane: f32| {
            let mut g = moving_group(1);
            g.particles[0].position = Vec3::X * x;
            g.motion.bounce = Some(Bounce {
                planes: vec![(Vec3::Y, plane)],
                ..default()
            });
            let r = Vec3::splat(0.127);
            g.merge = Some((1, 3.048, Vec3::X * x - r, Vec3::X * x + r));
            g
        };
        pool.add(with(0.0, 1.0));
        pool.add(with(1.0, 2.0));
        assert_eq!(pool.groups.len(), 1);
        assert_eq!(pool.groups[0].particles.len(), 2);
        assert_eq!(pool.groups[0].motion.bounce.as_ref().unwrap().planes[0].1, 2.0);
        pool.add(with(10.0, 3.0));
        assert_eq!(pool.groups.len(), 2);
    }

    #[test]
    fn bounce_planes_from_probes() {
        // A floor at y = 0: probes from 1 m up fall onto it.
        let floor = |from: Vec3, to: Vec3| -> Option<(f32, Vec3)> {
            (to.y < 0.0 && from.y >= 0.0).then(|| (from.y / (from.y - to.y), Vec3::Y))
        };
        let planes = probe_planes(Vec3::Y, Some(Vec3::X), 2.0, 20.0, &floor);
        assert_eq!(planes, vec![(Vec3::Y, 0.0)]);
        // A particle crossing the plane re-traces and lands.
        let b = Bounce {
            keep: 0.3,
            land_normal_y: 0.5,
            land_speed: 1.2,
            planes,
        };
        let m = Motion {
            bounce: Some(b),
            ..default()
        };
        let mut p = Particle::new(Vec3::Y * 0.05, 3.0, 0, 0.01);
        p.velocity = Vec3::new(0.2, -1.0, 0.0);
        step_particle(&mut p, &m, 0.1, &floor, &mut ParticleRng::new(1));
        assert_eq!(p.velocity, Vec3::ZERO);
        assert!(p.position.y >= 0.0);
    }

    #[test]
    fn tumbling_shards_spin_all_three_angles_and_settle_flat() {
        // Pitch 90 faces down: the square lies flat.
        let (f, side, up) = tumble_axes(Vec3::new(90.0, 0.0, 0.0));
        assert!(f.abs_diff_eq(Vec3::NEG_Y, 1e-5), "{f}");
        assert!(side.abs_diff_eq(Vec3::NEG_Z, 1e-5) && up.abs_diff_eq(Vec3::X, 1e-5));
        // Spec section 9: below 180 toward 92, above toward 270; yaw half
        // the roll.
        let mut a = Vec3::new(10.0, 40.0, 30.0);
        settle_flat(&mut a);
        assert_eq!(a, Vec3::new(51.0, 15.0, 30.0));
        for _ in 0..40 {
            settle_flat(&mut a);
        }
        assert!((a.x - 92.0).abs() < 1e-3);
        let mut b = Vec3::new(300.0, 7.0, 0.2);
        settle_flat(&mut b);
        assert_eq!(b, Vec3::new(285.0, 7.0, 0.2));
        let mut c = Vec3::new(90.3, 0.0, 0.0);
        settle_flat(&mut c);
        assert_eq!(c.x, 90.3);
        // In flight every angle turns at the spin; once stopped (spin 0)
        // it settles each frame.
        let mut p = Particle::new(Vec3::ZERO, 5.0, 0, 0.1);
        p.velocity = Vec3::X;
        p.roll_speed = 100.0;
        p.shape = Shape::Tumble {
            angles: Vec3::new(10.0, 20.0, 30.0),
            settle: true,
        };
        let m = Motion::default();
        let mut rng = ParticleRng::new(1);
        step_particle(&mut p, &m, 0.1, &NoWorld, &mut rng);
        assert!(
            matches!(p.shape, Shape::Tumble { angles, .. } if angles.abs_diff_eq(Vec3::new(20.0, 30.0, 40.0), 1e-4))
        );
        p.roll_speed = 0.0;
        step_particle(&mut p, &m, 0.1, &NoWorld, &mut rng);
        assert!(
            matches!(p.shape, Shape::Tumble { angles, .. } if angles.abs_diff_eq(Vec3::new(56.0, 20.0, 40.0), 1e-4))
        );
    }

    #[test]
    fn ramps_and_bias() {
        assert!((bias(0.5, 0.7) - 0.7).abs() < 1e-6);
        let r = Ramp::linear(1.0, 0.75).between(0.5, 1.0);
        assert_eq!(r.at(0.25), 1.0);
        assert!((r.at(0.75) - 0.875).abs() < 1e-6);
        let w = Fade::Windows {
            start_alpha: 0.0,
            fade_in: (0.0, 0.125),
            fade_out: (0.25, 1.0),
        };
        assert_eq!(w.at(0.0, 1.0), 0.0);
        assert_eq!(w.at(0.2, 1.0), 1.0);
        assert!((w.at(0.625, 1.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn sprite_cards_cap_and_fade_by_screen_size() {
        // blood_core: $maxsize .25. A 45° half-angle: the view is twice
        // the depth high.
        let core = ScreenSize {
            max: 0.25,
            ..default()
        };
        // 1 m wide at 10 m: 0.05 of the screen, untouched.
        assert_eq!(core.apply(0.5, 10.0, 1.0), (0.5, 1.0));
        // 1 m wide at 1 m: half the screen, capped to a quarter.
        let (size, alpha) = core.apply(0.5, 1.0, 1.0);
        assert!((size - 0.25).abs() < 1e-6 && alpha == 1.0);
        // smoke1_nearcull2: fades from .075 to .1 of the screen.
        let smoke = ScreenSize {
            fade_start: 0.075,
            fade_end: 0.1,
            ..default()
        };
        assert_eq!(smoke.apply(0.05, 2.0, 1.0).1, 1.0); // 0.025
        assert!((smoke.apply(0.175, 2.0, 1.0).1 - 0.5).abs() < 1e-4); // 0.0875
        assert_eq!(smoke.apply(0.5, 2.0, 1.0), (0.5, 0.0)); // 0.25
        // At the eye a sprite fills the screen.
        assert_eq!(smoke.apply(0.01, 0.0, 1.0).1, 0.0);
        assert_eq!(ScreenSize::default().apply(3.0, 0.1, 1.0), (3.0, 1.0));
    }

    #[test]
    fn rolls_stop_at_their_stop_time() {
        let mut p = Particle::new(Vec3::ZERO, 5.0, 0, 0.1);
        p.roll_speed = 1.0;
        p.roll_stop = 0.25;
        let (m, mut rng) = (Motion::default(), ParticleRng::new(1));
        for _ in 0..4 {
            step_particle(&mut p, &m, 0.1, &NoWorld, &mut rng);
        }
        assert!((p.roll - 0.25).abs() < 1e-6, "{}", p.roll);
    }
}
