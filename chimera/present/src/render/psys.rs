//! Running and drawing the data-driven particle systems
//! (`map::psys`: Source's `.pcf` effects, run by info_particle_system:
//! specs/source/particles_and_smoke.md 1-2). Simulation is in the game's
//! units (Source: inches, Z up); particles reach the pool
//! (`map::particles`) as one-frame groups in engine space.
//!
//! `Instance` (simulated in the game's units, one per start; stopping lets
//! particles finish), `SystemEmitter` per info_particle_system following its
//! entity, switched by `EntityPart`, drawn as one-frame pool groups (sprites,
//! sprite trails of specs/tf2/particles.md 5.1, ropes; a particle drawn by
//! every renderer its system lists; the material's overbright); `run_started_systems`
//! runs the one-shot systems games start (`StartParticleSystem`) to their
//! end, in the world pass (a first-person muzzle flash too, started at the
//! muzzle re-projected into the world view: specs/tf2/particles.md 6),
//! with control point 1 where it was given one (`StartParticleSystem::end`:
//! a tracer's end, "move particles between 2 control points" heading
//! there at its maximum speed and living until it arrives), stopped by
//! tag (`StopParticleSystems`: a gun's systems that run while it fires),
//! with control points 1-63 and an active one from the tag's
//! `ParticleControlPoints` each frame (TF2's flame points: "Assign target
//! CP" gives each new particle the active one, "Movement Follow CP" moves
//! it with its point and closes the gap at catch-up units a second, "Lifetime From Control Point
//! Life Time" gives it the point's duration; specs/tf2/flames.md 3.3),
//! "Remap Distance to Control Point to Scalar / Vector" each step (radius,
//! alpha, tint), and `RunningSystems` (what the started systems show, a
//! row per started system and per started child, for tests and probes;
//! the console's `psys_running` prints it, `psys_def` a definition).
//! Colour random's `tint_perc` blends each new particle's colour that
//! far toward the light at its control point (`ControlPoint::light`: the
//! map's `LightField` there, sampled when the system starts or its entity
//! moves; white without one). Timing and placement follow the particle
//! library's rules (specs/tf2/particles.md 9, `Instance::step_with`): a
//! system's creation frame runs with zero time (its initial particles
//! only, no emission, children not run); a child starts the frame after
//! with its points standing still; a continuous emitter's n particles of
//! a step are made at k/n of its window (never at its start), a burst at
//! its start time; each new particle where its control points were at its
//! creation time, on their straight path from last frame's places. A
//! tag's `ParticleModel` puts control point 0 at its origin and gives
//! "Position on Model Random" its hitboxes (best of 6 tries by the
//! bias), "Movement Lock to Bone" keeps each particle in its box, and a
//! view model's are drawn in the view-model pass (TF2's crit glow:
//! specs/tf2/effects.md 14.1). The operators follow the particle
//! library's formulas (specs/tf2/particles.md 10, every game): "Alpha Fade
//! and Decay" and "Radius Scale" write from the initial value inside
//! their windows and leave it outside, the radius by the rational bias;
//! "Color Fade" from the initial colour, eased; "Movement Basic"'s drag
//! per 1/30 s and every operator over the whole step (a particle made in
//! it moves v h); a sprite trail at least its minimum length and no wider
//! than long; a particle whose render alpha byte is 0 not drawn;
//! "Oscillate Scalar / Vector" adding rate × h × the parabolic wave to
//! the field; an emitter's own fade (10.11, `OpFade`) on its system's
//! clock at each sub-step's end: not run at strength 0, a continuous
//! rate times the strength (TF2's Minigun flash: the starflash and flare
//! stop at 0.5 s, the smoke ramps in over 2-4 s; its "fade oscillate"
//! not applied, `OpFade::strength`), and a system's cap
//! (`max_particles`, each child its own) taking new particles only as
//! old ones die. An info_particle_system's new effect is skipped ahead
//! (`Instance::skip_to`, 10.10, specs/source/particles_and_smoke.md 2.5):
//! no initial particles, a continuous emitter that ended before never
//! emits (TF2's `Explosion_Flashup` shows nothing that way, while a blast
//! started by a game draws it), a burst fires late.
//!
//! Known shortcuts (docs/tech-debt.md): Particle systems (`render::psys`,
//! `map::psys`, `source_engine::content::pcf`): the operator meanings are
//! interpretations where specs/tf2/particles.md 10 doesn't give them
//! (specs/cs_source/impact_effects.md 14, fire.md 7.3): movement is
//! integrated as a velocity (the library's previous-position form gives
//! the same unless a frame is cut into sub-steps), velocity noise is
//! uniform per particle, random force a uniform draw per step, twist a
//! tangential acceleration, an operator's own fade (10.11) is run on
//! emitters only (initializers', operators' and renderers' are read as
//! always 1, as in every system read: the loader warns of one that
//! isn't), an info_particle_system's
//! lateness is one server tick (the server's start time isn't sent, so a
//! client joining long after a start doesn't cancel its bursts), the client's particle multiplier is 1 and an
//! emitter's stop and restart (particles.md 9.2) aren't run (a stopped
//! system never restarts), animation rate without "as FPS" plays the sequence that many
//! times a second; "collision via traces" is a ray per step (its radius past
//! the step) against the solid world for started systems only (placed
//! ones don't collide), bounce and slide read as fractions of the speed
//! into and along the surface (its trace tolerances and collision group
//! aren't read); "Position on Model Random" and "Movement Lock to Bone" need the
//! game to give the model's boxes (`ParticleModel`, else at the point)
//! and ignore "force to be inside model"; the visibility,
//! cull and sleep settings aren't run (an unseen system freezes); control
//! points 1-63 (`cpointN`) of a placed system all sit on the entity;
//! "Movement Follow CP" carries a particle by its point's velocity and
//! closes the gap at catch-up units a second (fitted to TF2's flame
//! stream seen from the side; the operator itself isn't public), and a particle whose point is never set stays where
//! it was born; `$addself` sprite cards are
//! blended with the overbright factor, not part-added; `tint_perc` is read
//! as a blend in gamma space toward the light probe averaged over its six
//! directions and clamped to 0-1, sampled again when the entity moves a
//! unit (not the file's movement threshold; the tint clamps, always 0-255
//! in the install, aren't read); only binary DMX
//! encoding 2 is read; a system's look isn't compared with CS:S. Why it's OK
//! for now: Draws the maps' effects (7 maps, 307 systems) from their own data;
//! a reference capture would tune the meanings.

use bevy::prelude::*;

use crate::map::{
    EntityPart, ParticleRng, SkyboxCamera, ViewModelCamera, monitor,
    particles::{Particle, ParticleGroup, Particles, Ramp, Shape},
    psys::*,
    water,
};

/// A control point: position and orientation in game axes, and the light
/// there (linear, lightmap units: 1 = a texture's own brightness), which
/// `tint_perc` blends colours toward.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlPoint {
    pub position: Vec3,
    pub rotation: Quat,
    pub light: Vec3,
    /// How long what it stands for lives, s (0: not set):
    /// `Init::LifetimeFromCp`.
    pub duration: f32,
    /// How fast what it stands for moves (game units a second; zero when
    /// not set): `Op::FollowCp` carries its particles along with it.
    pub velocity: Vec3,
}

impl Default for ControlPoint {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            light: Vec3::ONE,
            duration: 0.0,
            velocity: Vec3::ZERO,
        }
    }
}

/// The light at a point for `tint_perc` (the game's point-lighting query,
/// as ropes take it): the probe averaged over its six axis directions,
/// clamped to 0-1.
pub fn point_light(probe: &crate::map::LightProbe) -> Vec3 {
    let sum: Vec3 = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z]
        .into_iter()
        .map(|d| probe.eval(d))
        .sum();
    (sum / 6.0).clamp(Vec3::ZERO, Vec3::ONE)
}

/// `color` blended `tint` of the way toward `light` (linear, clamped to
/// 0-1), in gamma space (specs/cs_source/impact_effects.md 14, as the
/// blood's mist and goop take it).
pub fn tint_toward_light(color: Vec3, light: Vec3, tint: f32) -> Vec3 {
    if tint <= 0.0 {
        return color;
    }
    color.lerp(light.clamp(Vec3::ZERO, Vec3::ONE).powf(1.0 / 2.2), tint.min(1.0))
}

#[derive(Clone, Debug)]
struct P {
    position: Vec3,
    velocity: Vec3,
    age: f32,
    life: f32,
    radius: f32,
    color: Vec3,
    alpha: f32,
    roll: f32,
    roll_speed: f32,
    sequence: u32,
    flip: bool,
    fade_in: f32,
    fade_out: f32,
    /// Its birth order (ropes).
    serial: u32,
    /// Its own control point (`Init::AssignTargetCp`).
    target: Option<u8>,
    /// What the distance remaps make of its radius and alpha (factors)
    /// and its tint (`Op::RemapDistance`).
    radius_mul: f32,
    alpha_mul: f32,
    tint: Option<Vec3>,
    /// The trail renderer's length attribute (field 10): seconds of its
    /// travel the trail spans.
    trail: f32,
    /// Its motion over its last step (units/s): what a sprite trail
    /// follows (`Render::Trail`, particles.md 5.1).
    motion: Vec3,
    /// The model hitbox it was placed on and its place in the box's unit
    /// cube (`Init::OnModel`, `Op::LockToBone`).
    bone: Option<(u16, Vec3)>,
    /// Its alpha and radius right after its initializers: what "Alpha
    /// Fade and Decay" and "Radius Scale" scale (particles.md 10.2-10.3).
    alpha0: f32,
    radius0: f32,
    /// Its first two oscillators' fixed random numbers (`Op::Oscillate`,
    /// particles.md 10.9): start, end, and (scalar) rate and frequency.
    osc: [[f32; 4]; 2],
}

/// "Movement Basic"'s drag is the fraction of the velocity lost per 1/30 s
/// whatever the step (specs/tf2/particles.md 10.5: the factor (1 −
/// drag)^(30 h) per step h).
pub const DRAG_STEPS: f32 = 30.0;

/// A particle whose render alpha byte, round(255 × alpha), is 0 is not
/// drawn (particles.md 10.8): alpha below this.
const ALPHA_DRAWN: f32 = 0.5 / 255.0;

/// The client's skip-ahead threshold (particles.md 10.10): an
/// info_particle_system's effect seen more than this late is skipped.
pub const SKIP_AFTER: f32 = 0.01;

/// An instantaneous burst whose start is more than this before a skip's
/// time is cancelled (particles.md 10.10).
const BURST_KEPT: f32 = 2.0;

/// The trail length (field 10) a particle has when nothing sets it, s.
const DEFAULT_TRAIL_LENGTH: f32 = 0.1;

impl P {
    /// Attribute `field` (the public numbering, specs/tf2/particles.md 7)
    /// as a number, for the remaps; `created` is its creation time (8).
    fn scalar(&self, field: u8, created: f32) -> Option<f32> {
        Some(match field {
            1 => self.life,
            3 => self.radius,
            4 => self.roll.to_radians(),
            5 => self.roll_speed.to_radians(),
            7 => self.alpha,
            8 => created,
            9 => self.sequence as f32,
            10 => self.trail,
            _ => return None,
        })
    }

    fn set_scalar(&mut self, field: u8, v: f32) {
        match field {
            1 => self.life = v,
            3 => self.radius = v,
            4 => self.roll = v.to_degrees(),
            5 => self.roll_speed = v.to_degrees(),
            7 => self.alpha = v,
            9 => self.sequence = v.max(0.0) as u32,
            10 => self.trail = v,
            _ => {}
        }
    }
}

/// A particle's colour (0-1, gamma, as the pool takes it) brightened by a
/// sprite card's `$overbrightfactor` `o`: the factor multiplies the colour
/// in linear space, after its gamma is undone (specs/tf2/particles.md 2:
/// the texel times O, the vertex colour taken to linear), so the gamma
/// colour handed on is the one whose linear value is o times this one's.
/// Multiplying the gamma colour instead made the flame core's 5 about 34
/// in linear: its sprites at a wall burnt white where real TF2's are
/// violet (tools/tf2ref).
pub fn overbright(color: Vec3, o: f32) -> Vec3 {
    if (o - 1.0).abs() < 1e-6 {
        return color;
    }
    let lin = Color::srgb(color.x, color.y, color.z).to_linear();
    let up = Color::LinearRgba(LinearRgba::rgb(lin.red * o, lin.green * o, lin.blue * o)).to_srgba();
    Vec3::new(up.red, up.green, up.blue)
}

/// `x` from [`a`, `b`] onto [0, 1], clamped (0 when the range is empty).
fn unlerp(x: f32, a: f32, b: f32) -> f32 {
    if (b - a).abs() < 1e-9 { 0.0 } else { ((x - a) / (b - a)).clamp(0.0, 1.0) }
}

/// Whether the system's age `age` is inside a remap's emitter-lifetime
/// window (a negative bound: none).
fn in_window(age: f32, start: f32, end: f32) -> bool {
    (start < 0.0 || age >= start) && (end < 0.0 || age <= end)
}

/// A running effect: its particles, its children and its clock.
#[derive(Clone, Debug)]
pub struct Instance {
    pub def: usize,
    pub age: f32,
    pub emitting: bool,
    particles: Vec<P>,
    /// Continuous emitters' carried fractions.
    carry: Vec<f32>,
    children: Vec<(f32, Option<Instance>, usize)>,
    serial: u32,
    started: bool,
    /// Where the locked-to control point was last step.
    lock_from: Option<Vec3>,
    /// Where its control points were at the end of its last step (game
    /// space): a step's new particles are made along each point's path
    /// from there (`step_with`).
    cp_from: Option<Vec<Vec3>>,
    /// The hitboxes of the model its control point 0 stands for (game
    /// space, from the unit cube: `set_model`).
    model: Vec<Mat4>,
    /// Instantaneous emitters whose burst a skip left due (`skip_to`):
    /// they fire in the next step at their start time.
    due: Vec<bool>,
    /// Its particles' fixed random numbers' seed (oscillators; 0 until
    /// its first oscillating particle).
    seed: u64,
}

/// The sheet sequences' frame counts of a material (for "lifetime from
/// sequence" and frame animation).
pub trait Sheets {
    fn frames(&self, material: usize, sequence: u32) -> usize;
    /// The frames of the material's sheet sequence; None when the
    /// material has no sheet (`Init::LifetimeFromSequence` then leaves
    /// the life alone).
    fn sheet_frames(&self, material: usize, sequence: u32) -> Option<usize> {
        Some(self.frames(material, sequence))
    }
    /// The material's colour multiplier (sprite cards' overbright).
    fn overbright(&self, _material: usize) -> f32 {
        1.0
    }
    /// The world particles collide with (`Op::Collide`): the first
    /// surface on the segment `from` to `to` (game units), its point and
    /// normal; None when nothing is there (or there's no world: tests).
    fn trace(&self, _from: Vec3, _to: Vec3) -> Option<(Vec3, Vec3)> {
        None
    }
}

/// `MaterialSheets` with the map's solid world for `Sheets::trace`
/// (characters left out: brass lands on the floor, not on the Heavy).
pub struct WorldSheets<'a, 'w, 's> {
    pub sheets: MaterialSheets<'a>,
    pub spatial: Option<&'a avian3d::prelude::SpatialQuery<'w, 's>>,
    pub filter: avian3d::prelude::SpatialQueryFilter,
    /// Meters per game unit.
    pub scale: f32,
}

impl Sheets for WorldSheets<'_, '_, '_> {
    fn frames(&self, material: usize, sequence: u32) -> usize {
        self.sheets.frames(material, sequence)
    }

    fn sheet_frames(&self, material: usize, sequence: u32) -> Option<usize> {
        self.sheets.sheet_frames(material, sequence)
    }

    fn overbright(&self, material: usize) -> f32 {
        self.sheets.overbright(material)
    }

    fn trace(&self, from: Vec3, to: Vec3) -> Option<(Vec3, Vec3)> {
        let to_engine = |v: Vec3| Vec3::new(v.x, v.z, -v.y) * self.scale;
        let (a, b) = (to_engine(from), to_engine(to));
        let (dir, length) = Dir3::new_and_length(b - a).ok()?;
        let hit = self.spatial?.cast_ray(a, dir, length, true, &self.filter)?;
        let at = from + (to - from) * (hit.distance / length);
        let n = hit.normal;
        Some((at, Vec3::new(n.x, -n.z, n.y).normalize_or_zero()))
    }
}

/// The map's particle materials as `Sheets`.
pub struct MaterialSheets<'a>(pub Option<&'a crate::map::particles::MapParticles>);

impl Sheets for MaterialSheets<'_> {
    fn frames(&self, material: usize, sequence: u32) -> usize {
        self.0
            .and_then(|ms| ms.materials.get(material))
            .and_then(|m| m.sequences.get(sequence as usize))
            .map_or(1, Vec::len)
    }

    fn sheet_frames(&self, material: usize, sequence: u32) -> Option<usize> {
        self.0
            .and_then(|ms| ms.materials.get(material))
            .and_then(|m| m.sequences.get(sequence as usize))
            .map(Vec::len)
            .filter(|n| *n > 0)
    }

    fn overbright(&self, material: usize) -> f32 {
        self.0
            .and_then(|ms| ms.materials.get(material))
            .map(|m| m.overbright)
            .filter(|o| *o > 0.0)
            .unwrap_or(1.0)
    }
}

impl<F: Fn(usize, u32) -> usize> Sheets for F {
    fn frames(&self, material: usize, sequence: u32) -> usize {
        self(material, sequence)
    }
}

/// A sprite trail's length (specs/tf2/particles.md 5.1): the speed of
/// A followed system's control-point turn (game space: x forward, y
/// left, z up) in its entity's frame `at`: its x along `Follow::forward`,
/// its z along `up` when given (an attachment's whole orientation), else
/// the shortest turn from x.
pub fn follow_turn(f: &crate::map::psys::Follow, at: &Transform) -> Quat {
    let game = |v: Vec3| Vec3::new(v.x, -v.z, v.y);
    let fw = game(at.rotation * f.forward).try_normalize().unwrap_or(Vec3::X);
    match f.up.map(|u| game(at.rotation * u)).and_then(|u| (u - fw * u.dot(fw)).try_normalize()) {
        Some(up) => Quat::from_mat3(&Mat3::from_cols(fw, up.cross(fw), up)),
        None => Quat::from_rotation_arc(Vec3::X, fw),
    }
}

/// A sprite trail's length (specs/tf2/particles.md 10.7): `motion` (its
/// last step's, units/s) x `trail` (s) x min(1, `age` / `fade_in`) (1 with
/// no fade-in), then at most `max` and at least `min` (the minimum wins
/// when they cross); None when it didn't move, or the unclamped length is
/// not positive (age 0 with a fade-in, or no trail length).
pub fn trail_length(motion: Vec3, trail: f32, age: f32, min: f32, max: f32, fade_in: f32) -> Option<f32> {
    let speed = motion.length();
    if speed <= 1e-4 {
        return None;
    }
    let f = if fade_in > 0.0 && age < fade_in { age / fade_in } else { 1.0 };
    let length = (speed * speed + 1e-10).sqrt() * trail * f;
    if length <= 0.0 {
        return None;
    }
    Some(length.min(max).max(min))
}

/// The ease of the particle operators: S(s) = 3s² − 2s³.
fn smooth(x: f32) -> f32 {
    x * x * (3.0 - 2.0 * x)
}

/// "Radius Scale"'s bias (particles.md 10.3): s / ((1/b − 2)(1 − s) + 1),
/// so g(0.5) = b; b 0.5 is linear.
pub fn rational_bias(s: f32, b: f32) -> f32 {
    if (b - 0.5).abs() < 1e-6 || b <= 0.0 {
        return s;
    }
    s / ((1.0 / b - 2.0) * (1.0 - s) + 1.0)
}

/// The oscillators' wave (particles.md 10.9): a parabolic stand-in for
/// sin(π x), period 2 in x.
pub fn wave(x: f32) -> f32 {
    // |x| less its whole periods (a truncating cast, not libm's fmod).
    let ax = x.abs();
    let f = ax - 2.0 * (ax * 0.5) as i64 as f32;
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    if f < 1.0 { sign * 4.0 * f * (1.0 - f) } else { -sign * 4.0 * (f - 1.0) * (2.0 - f) }
}

/// A particle's fixed random number in [0, 1) for one of its oscillators'
/// draws (particles.md 10.9: by particle, independent, for its life).
fn fixed_random(seed: u64, serial: u32, op: usize, k: usize) -> f32 {
    let mut x = seed ^ (u64::from(serial) << 20) ^ ((op as u64) << 8) ^ k as u64;
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x >> 40) as f32 / (1u64 << 24) as f32
}

fn window(t: f32, a: f32, b: f32) -> f32 {
    if b > a {
        ((t - a) / (b - a)).clamp(0.0, 1.0)
    } else if t >= a {
        1.0
    } else {
        0.0
    }
}

fn pick3(rng: &mut ParticleRng, a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(rng.float(a.x, b.x), rng.float(a.y, b.y), rng.float(a.z, b.z))
}

fn random_dir(rng: &mut ParticleRng) -> Vec3 {
    loop {
        let v = rng.vec3(-1.0, 1.0);
        let l = v.length_squared();
        if l > 1e-6 && l <= 1.0 {
            return v / l.sqrt();
        }
    }
}

impl Instance {
    pub fn new(defs: &[SystemDef], def: usize) -> Self {
        let d = &defs[def];
        Self {
            def,
            age: 0.0,
            emitting: true,
            particles: Vec::new(),
            carry: vec![0.0; d.emitters.len()],
            children: d.children.iter().map(|&(c, delay)| (delay, None, c)).collect(),
            serial: 0,
            started: false,
            lock_from: None,
            cp_from: None,
            model: Vec::new(),
            due: vec![false; d.emitters.len()],
            seed: 0,
        }
    }

    /// Skip it ahead to its clock `t` without simulating (particles.md
    /// 10.10: a late info_particle_system's effect): nothing when `t` is
    /// not past its clock. Its creation step never runs (no initial
    /// particles); its points stand still into the next step; an
    /// instantaneous burst whose start is in the skipped time fires in
    /// the next step at its start time unless `t` is more than 2 s past
    /// it; a continuous emitter just goes on from `t` (one that ended
    /// before never emits); children are skipped to `t` minus their
    /// delay.
    pub fn skip_to(&mut self, defs: &[SystemDef], t: f32) {
        if t <= self.age {
            return;
        }
        let d = &defs[self.def];
        for (due, e) in self.due.iter_mut().zip(&d.emitters) {
            if let Emitter::Instantaneously { start, .. } = *e {
                let unfired = if start <= 0.0 { self.age == 0.0 } else { self.age < start };
                if unfired && start <= t {
                    *due = t <= start.max(0.0) + BURST_KEPT;
                }
            }
        }
        self.age = t;
        self.started = true;
        self.cp_from = None;
        for (delay, c, def) in &mut self.children {
            if t - *delay > 0.0 && self.emitting {
                let child = c.get_or_insert_with(|| Instance::new(defs, *def));
                child.skip_to(defs, t - *delay);
            }
        }
    }

    /// The hitboxes (game space, each from the unit cube [−1, 1]³) of the
    /// model its control point 0 stands for this frame (`Init::OnModel`,
    /// `Op::LockToBone`), its children's too.
    pub fn set_model(&mut self, boxes: &[Mat4]) {
        if self.model != boxes {
            self.model.clear();
            self.model.extend_from_slice(boxes);
        }
        for c in self.children.iter_mut().filter_map(|c| c.1.as_mut()) {
            c.set_model(boxes);
        }
    }

    /// Particles alive in it and its children.
    pub fn count(&self) -> usize {
        self.particles.len()
            + self
                .children
                .iter()
                .filter_map(|c| c.1.as_ref())
                .map(Instance::count)
                .sum::<usize>()
    }

    /// The live particles' colours (0-1, before any colour fade), its
    /// children's too.
    pub fn colors(&self) -> Vec<Vec3> {
        let mut out: Vec<Vec3> = self.particles.iter().map(|p| p.color).collect();
        for c in self.children.iter().filter_map(|c| c.1.as_ref()) {
            out.extend(c.colors());
        }
        out
    }

    /// Its particles (its children's too) moved by `f` (positions) and
    /// their velocities turned by `turn`: carried along by what it follows
    /// (`StartParticleSystem::follow`). A locked-to control point's last
    /// place goes along, so the lock adds nothing of it again.
    fn carry(&mut self, f: &dyn Fn(Vec3) -> Vec3, turn: &dyn Fn(Vec3) -> Vec3) {
        for p in &mut self.particles {
            p.position = f(p.position);
            p.velocity = turn(p.velocity);
        }
        if let Some(l) = self.lock_from {
            self.lock_from = Some(f(l));
        }
        if let Some(from) = &mut self.cp_from {
            for p in from {
                *p = f(*p);
            }
        }
        for c in self.children.iter_mut().filter_map(|c| c.1.as_mut()) {
            c.carry(f, turn);
        }
    }

    /// Stop emitting (its children too); live particles finish.
    pub fn stop(&mut self) {
        self.emitting = false;
        for c in self.children.iter_mut().filter_map(|c| c.1.as_mut()) {
            c.stop();
        }
    }

    /// Nothing left to show or make.
    pub fn finished(&self, defs: &[SystemDef]) -> bool {
        let d = &defs[self.def];
        let emits = self.emitting
            && d.emitters.iter().enumerate().any(|(i, e)| {
                // An emitter whose own fade has ended never runs again.
                let faded = d.emitter_fades.get(i).and_then(|f| f.ends()).is_some_and(|end| self.age > end);
                !faded
                    && match *e {
                        Emitter::Continuously { start, duration, .. } => duration <= 0.0 || self.age < start + duration,
                        Emitter::Instantaneously { start, .. } => self.age <= start,
                    }
            });
        let children_left = self.children.iter().any(|(delay, c, _)| match c {
            Some(c) => !c.finished(defs),
            // Not started yet: due while it emits, a child at its
            // delay too (one at 0 waits out the parent's zero-time
            // creation frame, particles.md 9.1: a blast whose systems
            // are all children, TF2's `ExplosionCore_Wall`, once went
            // as finished in that frame and drew nothing).
            None => self.emitting && self.age <= *delay,
        });
        !emits && self.particles.is_empty() && !children_left
    }

    /// Advance `dt` (cut into steps of at most the system's largest step).
    pub fn step(
        &mut self,
        defs: &[SystemDef],
        cps: &[ControlPoint],
        dt: f32,
        rng: &mut ParticleRng,
        sheets: &impl Sheets,
    ) {
        self.step_with(defs, cps, None, dt, rng, sheets);
    }

    /// `step` with an active control point new particles take
    /// (`Init::AssignTargetCp`).
    ///
    /// One call is one frame (specs/tf2/particles.md 9.1). The frame a
    /// system is created in it is simulated with zero time: only its
    /// `initial_particles` are made, where its points are, no emitter
    /// emits and its children don't run (`dt` is not counted). Every
    /// later frame `dt` is stepped in sub-steps of at most the system's
    /// maximum step, then each child with the whole `dt` (a child's first
    /// frame sets it up and steps it at once, its points standing still);
    /// after them each point's previous place becomes its current one.
    /// A step's new particles are made where their points were at their
    /// creation times, the points moving straight from their previous
    /// places (9.3; with several sub-steps the frame's start still, the
    /// fraction of the sub-step: the spec's quirk).
    pub fn step_with(
        &mut self,
        defs: &[SystemDef],
        cps: &[ControlPoint],
        active: Option<usize>,
        dt: f32,
        rng: &mut ParticleRng,
        sheets: &impl Sheets,
    ) {
        self.frame(defs, cps, active, dt, rng, sheets, false);
    }

    #[allow(clippy::too_many_arguments)]
    fn frame(
        &mut self,
        defs: &[SystemDef],
        cps: &[ControlPoint],
        active: Option<usize>,
        dt: f32,
        rng: &mut ParticleRng,
        sheets: &impl Sheets,
        child: bool,
    ) {
        let now: Vec<Vec3> = cps.iter().map(|c| c.position).collect();
        if !self.started {
            // The first-frame setup: zero time, initial particles only.
            self.started = true;
            self.tick(defs, cps, cps, active, 0.0, rng, sheets, true);
            self.cp_from = Some(now.clone());
            if !child {
                return;
            }
        }
        let from = match self.cp_from.take() {
            Some(f) if f.len() == now.len() => f,
            _ => now.clone(),
        };
        let start: Vec<ControlPoint> = cps.iter().zip(&from).map(|(c, f)| ControlPoint { position: *f, ..*c }).collect();
        let max = if defs[self.def].max_step > 0.0 { defs[self.def].max_step } else { 0.1 };
        let mut left = dt.min(10.0 * max);
        while left > 1e-6 {
            let h = left.min(max);
            self.tick(defs, &start, cps, active, h, rng, sheets, false);
            left -= h;
        }
        // Children start after their delay (while the parent emits), and
        // run after the parent's steps with the whole frame.
        for (delay, c, def) in &mut self.children {
            if c.is_none() && self.emitting && self.age >= *delay {
                let mut child = Instance::new(defs, *def);
                child.model.clone_from(&self.model);
                *c = Some(child);
            }
        }
        for c in self.children.iter_mut().filter_map(|c| c.1.as_mut()) {
            c.frame(defs, cps, active, dt, rng, sheets, true);
        }
        self.cp_from = Some(now);
    }

    /// Each live particle's place (game space) and its own control point
    /// (`Init::AssignTargetCp`), its children's too.
    pub fn placed(&self) -> Vec<(Vec3, Option<usize>)> {
        let mut out: Vec<(Vec3, Option<usize>)> = self.particles.iter().map(|p| (p.position, p.target.map(usize::from))).collect();
        for c in self.children.iter().filter_map(|c| c.1.as_ref()) {
            out.extend(c.placed());
        }
        out
    }

    /// Its children started so far.
    pub fn child_instances(&self) -> impl Iterator<Item = &Instance> {
        self.children.iter().filter_map(|c| c.1.as_ref())
    }

    /// Its own live particles' radii as drawn (game units), not its
    /// children's.
    pub fn own_radii(&self, defs: &[SystemDef]) -> Vec<f32> {
        let d = &defs[self.def];
        self.particles.iter().map(|p| Self::radius(d, p)).collect()
    }

    /// Its own live particles' (position in game units, age, life, alpha
    /// as drawn), not its children's (probes).
    pub fn own_details(&self, defs: &[SystemDef]) -> Vec<(Vec3, f32, f32, f32)> {
        let d = &defs[self.def];
        self.particles.iter().map(|p| (p.position, p.age, p.life, Self::alpha(d, p))).collect()
    }

    /// Each live particle's radius as drawn (game units), its children's
    /// too.
    pub fn radii(&self, defs: &[SystemDef]) -> Vec<f32> {
        let d = &defs[self.def];
        let mut out: Vec<f32> = self.particles.iter().map(|p| Self::radius(d, p)).collect();
        for c in self.children.iter().filter_map(|c| c.1.as_ref()) {
            out.extend(c.radii(defs));
        }
        out
    }

    /// One sub-step of `dt` (0: the first-frame setup, `first`: initial
    /// particles only), the points moving from `cps_from` to `cps`.
    #[allow(clippy::too_many_arguments)]
    fn tick(
        &mut self,
        defs: &[SystemDef],
        cps_from: &[ControlPoint],
        cps: &[ControlPoint],
        active: Option<usize>,
        dt: f32,
        rng: &mut ParticleRng,
        sheets: &impl Sheets,
        first: bool,
    ) {
        let d = &defs[self.def];
        let cp = |i: usize| cps.get(i).or(cps.first()).copied().unwrap_or_default();
        let before = self.age;
        self.age += dt;
        // Emission: when each new particle is made (its creation time,
        // field 8; particles.md 9.2).
        let mut made: Vec<f32> = Vec::new();
        if first {
            made.extend(std::iter::repeat_n(before, d.initial_particles));
        } else if self.emitting {
            let t = self.age;
            for (i, e) in d.emitters.iter().enumerate() {
                // Its own fade at the sub-step's end (particles.md 10.11):
                // not run at strength 0, a continuous rate scaled by it.
                let strength = d.emitter_fades.get(i).map_or(1.0, |f| f.strength(t));
                if strength <= 0.0 {
                    continue;
                }
                match *e {
                    Emitter::Continuously { start, rate, duration } => {
                        let rate = rate * strength;
                        // The window: an endless one is the whole step
                        // once it has started (not cut at its start: the
                        // spec's quirk), a timed one the step's part of
                        // [start, start + duration].
                        let window = if duration <= 0.0 {
                            (start <= t).then_some((before, t))
                        } else if start > t || before > start + duration {
                            None
                        } else {
                            Some((before.max(start), t.min(start + duration)))
                        };
                        let Some((a, b)) = window else { continue };
                        // The whole step is its length exactly (the clock's
                        // sum would lose a 120/s emitter's second particle
                        // to rounding now and then).
                        let span = if a == before && b == t { dt } else { b - a };
                        self.carry[i] += rate.max(0.0) * span;
                        let n = self.carry[i].floor();
                        self.carry[i] -= n;
                        // The k-th of the step's n at a + k (b - a) / n:
                        // one share after the window's start, the last at
                        // its end.
                        let n = n as usize;
                        made.extend((1..=n).map(|k| (a + k as f32 * (b - a) / n as f32).min(b)));
                    }
                    Emitter::Instantaneously { start, count } => {
                        // All at its start time: at 0, the first emitting
                        // step's start (the point's previous place); left
                        // due by a skip, in the next step (`skip_to`).
                        let skipped = std::mem::take(&mut self.due[i]);
                        let due = skipped
                            || if start <= 0.0 {
                                before == 0.0
                            } else {
                                before < start && t >= start
                            };
                        if due {
                            let created = if skipped { start.max(0.0) } else { before.max(start) };
                            made.extend(std::iter::repeat_n(created, count as usize));
                        }
                    }
                }
            }
        }
        let room = d.max_particles.saturating_sub(self.particles.len());
        // Those made this step are made where their points were when they
        // were made: along each point's path over the step (`step_with`).
        let old = self.particles.len();
        for &created in made.iter().take(room) {
            // P(c) = prev + (now - prev) max(0, (c - (T - h)) / h); now
            // when the step is zero time (9.3).
            let t = if dt > 0.0 { ((created - before) / dt).max(0.0) } else { 1.0 };
            let then = |i: usize| {
                let now = cp(i);
                let was = cps_from.get(i).or(cps_from.first()).map_or(now.position, |c| c.position);
                ControlPoint { position: was.lerp(now.position, t), ..now }
            };
            let mut p = self.spawn(d, &then, active, rng, sheets, created);
            // Aged (and moved, below) only from its creation.
            p.age = before - created;
            Self::remap(d, &cp, &mut p);
            self.particles.push(p);
        }
        // Forces, movement, life.
        let decays = d
            .operators
            .iter()
            .any(|o| matches!(o, Op::Decay | Op::FadeAndDecay { .. }));
        let lock: Option<usize> = d.operators.iter().find_map(|o| match o {
            Op::LockToControlPoint { cp } => Some(*cp),
            _ => None,
        });
        let collide = d.operators.iter().find_map(|o| match *o {
            Op::Collide { bounce, slide, radius_scale, kill } => Some((bounce, slide, radius_scale, kill)),
            _ => None,
        });
        let mut killed = false;
        let mut from: Vec<Vec3> = self.particles.iter().map(|p| p.position).collect();
        let (clock, seed) = (self.age, self.seed);
        for p in &mut self.particles {
            // A particle made during this step is aged only by its part of
            // it, but the operators run over the whole step (particles.md
            // 10.5: its first motion is v h whenever in the step it was
            // made).
            p.age = p.age.max(0.0) + dt + p.age.min(0.0);
            let mut accel = Vec3::ZERO;
            let mut gravity = Vec3::ZERO;
            let mut drag = 0.0;
            let mut moves = false;
            for o in &d.operators {
                match *o {
                    Op::Movement { gravity: g, drag: k } => {
                        gravity += g;
                        drag = k;
                        moves = true;
                    }
                    Op::Pull { cp: c, force, falloff } => {
                        let to = cp(c).position - p.position;
                        let dist = to.length().max(1.0);
                        accel += to / dist * force / dist.powf(falloff);
                    }
                    Op::RandomForce { min, max } => accel += pick3(rng, min, max),
                    Op::Twist {
                        cp: c,
                        force,
                        axis,
                        local,
                    } => {
                        let point = cp(c);
                        let axis = if local { point.rotation * axis } else { axis }.normalize_or_zero();
                        let off = p.position - point.position;
                        accel += axis.cross(off).normalize_or_zero() * force;
                    }
                    _ => {}
                }
            }
            if moves {
                // x' = x + v h (1 − drag)^(30 h) + a h² (particles.md
                // 10.5), as a velocity: the drag per 1/30 s whatever the
                // step (`DRAG_STEPS`), not on this step's acceleration.
                if drag > 0.0 {
                    p.velocity *= (1.0 - drag).max(0.0).powf(dt * DRAG_STEPS);
                }
                p.velocity += (gravity + accel) * dt;
                for o in &d.operators {
                    if let Op::MaxVelocity { max } = *o
                        && max > 0.0
                    {
                        p.velocity = p.velocity.clamp_length_max(max);
                    }
                }
                let to = p.position + p.velocity * dt;
                // A ray reaching its radius (x the scale) past the step: it
                // stops that far off a surface.
                let reach = |scale: f32| (to - p.position).normalize_or_zero() * (p.radius * p.radius_mul * scale).max(0.01);
                match collide.and_then(|c| sheets.trace(p.position, to + reach(c.2)).map(|h| (c, h))) {
                    Some(((bounce, slide, radius_scale, kill), (at, normal))) => {
                        // On the surface: `slide` of its speed along it,
                        // `bounce` of its speed into it, reversed.
                        let into = p.velocity.dot(normal).min(0.0);
                        let along = p.velocity - normal * p.velocity.dot(normal);
                        p.velocity = along * slide - normal * into * bounce;
                        p.position = at + normal * (p.radius * p.radius_mul * radius_scale).max(0.01);
                        if kill {
                            p.life = -1.0;
                            killed = true;
                        }
                    }
                    None => p.position = to,
                }
            }
            let t = if p.life > 0.0 { p.age / p.life } else { 1.0 };
            let mut osc = 0;
            for (k, o) in d.operators.iter().enumerate() {
                match *o {
                    Op::RotationBasic => p.roll += p.roll_speed * dt,
                    Op::SpinRoll { rate } => p.roll += rate * dt,
                    Op::FadeAndDecay { start_alpha, end_alpha, fade_in, fade_out } if p.life > 0.0 => {
                        // Written only inside its windows, from the
                        // initial alpha; left as it is outside them
                        // (particles.md 10.2).
                        if fade_in.0 <= t && t < fade_in.1 {
                            let s = ((t - fade_in.0) / (fade_in.1 - fade_in.0)).clamp(0.0, 1.0);
                            p.alpha = p.alpha0 * (start_alpha + (1.0 - start_alpha) * smooth(s));
                        }
                        if fade_out.0 <= t && t < fade_out.1 {
                            let s = ((t - fade_out.0) / (fade_out.1 - fade_out.0)).clamp(0.0, 1.0);
                            p.alpha = p.alpha0 * (1.0 + (end_alpha - 1.0) * smooth(s));
                        }
                    }
                    Op::RadiusScale { start_scale, end_scale, start, end, bias, ease } if p.life > 0.0 && start < end => {
                        // From the initial radius inside its window, left
                        // as it is outside (particles.md 10.3).
                        if start <= t && t < end {
                            let s = (t - start) / (end - start);
                            let g = if ease { smooth(s) } else { rational_bias(s, bias) };
                            p.radius = p.radius0 * (start_scale + (end_scale - start_scale) * g);
                        }
                    }
                    Op::Oscillate {
                        field,
                        vector,
                        rate,
                        frequency,
                        proportional,
                        start,
                        end,
                        start_end_proportional,
                        multiplier,
                        phase,
                    } if p.life > 0.0 => {
                        // particles.md 10.9: the field's rate of change
                        // oscillates, sampled at the step's end.
                        // Its fixed random numbers: drawn at its birth for
                        // its first two (a vector's other axes, and any
                        // later, by its serial).
                        let r = |n: usize| fixed_random(seed, p.serial, k, n);
                        let drawn = p.osc.get(osc).copied();
                        osc += 1;
                        let [t1, t2, r0, f0] = drawn.unwrap_or_else(|| {
                            [start.0 + (start.1 - start.0) * r(0), end.0 + (end.1 - end.0) * r(1), rate.0.x + (rate.1.x - rate.0.x) * r(2), frequency.0.x + (frequency.1.x - frequency.0.x) * r(5)]
                        });
                        let at = if start_end_proportional { t } else { p.age };
                        if at < t1 || at >= t2 {
                            continue;
                        }
                        let axes = if vector { 3 } else { 1 };
                        let mut add = Vec3::ZERO;
                        for a in 0..axes {
                            let (rate, f) = if a == 0 {
                                (r0, f0)
                            } else {
                                (rate.0[a] + (rate.1[a] - rate.0[a]) * r(2 + a), frequency.0[a] + (frequency.1[a] - frequency.0[a]) * r(5 + a))
                            };
                            let x = if proportional {
                                f * multiplier * p.age / p.life + phase
                            } else {
                                f * (multiplier * clock + phase)
                            };
                            add[a] = rate * dt * wave(x);
                        }
                        match (vector, field) {
                            (false, 7) => p.alpha = (p.alpha + add.x).clamp(0.0, 1.0),
                            (false, f) => {
                                if let Some(v) = p.scalar(f, 0.0) {
                                    p.set_scalar(f, v + add.x);
                                }
                            }
                            (true, 0) => p.position += add,
                            (true, 6) => p.color = (p.color + add).clamp(Vec3::ZERO, Vec3::ONE),
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
        if let Some(c) = lock {
            let now = cp(c).position;
            if let Some(prev) = self.lock_from {
                // Carried with its point, where it was too: a sprite
                // trail streaks by the particle's own motion, not its
                // point's (real TF2's rocket: `rockettrail_burst`, locked
                // to the rocket at 1100 units/s, draws no streaks).
                let delta = now - prev;
                for (p, from) in self.particles.iter_mut().zip(from.iter_mut()).take(old) {
                    p.position += delta;
                    *from += delta;
                }
            }
            self.lock_from = Some(now);
        }
        // Kept on their hitboxes as the boxes move (`Op::LockToBone`).
        if d.operators.iter().any(|o| matches!(o, Op::LockToBone { .. })) && !self.model.is_empty() {
            for (p, from) in self.particles.iter_mut().zip(from.iter_mut()) {
                if let Some((i, u)) = p.bone
                    && let Some(m) = self.model.get(i as usize)
                {
                    let now = m.transform_point3(u);
                    *from += now - p.position;
                    p.position = now;
                }
            }
        }
        // Onto their own control points, and the distance remaps.
        for p in &mut self.particles {
            for o in &d.operators {
                if let Op::FollowCp { catch_up } = *o
                    && let Some(t) = p.target
                    && catch_up > 0.0
                {
                    // It moves with its point and closes the gap at
                    // `catch_up` units a second (flames.md Open
                    // questions 1, measured: Op::FollowCp).
                    let c = cp(t as usize);
                    p.position += c.velocity * dt;
                    let gap = c.position - p.position;
                    let close = catch_up * dt;
                    p.position = if gap.length() <= close { c.position } else { p.position + gap.normalize() * close };
                }
            }
            for o in &d.operators {
                if let Op::RemapScalar { input, input_min, input_max, output, output_min, output_max } = *o
                    && let Some(x) = p.scalar(input, 0.0)
                {
                    p.set_scalar(output, output_min + (output_max - output_min) * unlerp(x, input_min, input_max));
                }
            }
            Self::remap(d, &cp, p);
        }
        // Its motion over the step, per second (a sprite trail's
        // direction and speed: particles.md 10.7, k = 1/h).
        if dt > 1e-6 {
            for (p, from) in self.particles.iter_mut().zip(from) {
                p.motion = (p.position - from) / dt;
            }
        }
        if decays {
            // Dead at its life (the summed steps' rounding aside: a 0.1 s
            // particle lives exactly 6 steps at 60, particles.md 8.3.1).
            self.particles.retain(|p| p.age < p.life - 1e-6);
        }
        if killed {
            self.particles.retain(|p| p.life >= 0.0);
        }
    }

    /// The distance remaps (`Op::RemapDistance`) for where `p` is now.
    fn remap(d: &SystemDef, cp: &impl Fn(usize) -> ControlPoint, p: &mut P) {
        for o in &d.operators {
            let Op::RemapDistance { cp: c, field, near, far, low, high, scale_initial } = *o else { continue };
            let x = window(p.position.distance(cp(c).position), near, far);
            let v = low.lerp(high, x);
            match field {
                Field::Radius => p.radius_mul = if scale_initial { v.x } else { v.x / p.radius.max(1e-6) },
                Field::Alpha => p.alpha_mul = if scale_initial { v.x } else { v.x / p.alpha.max(1e-6) },
                Field::Tint => p.tint = Some(if scale_initial { p.color * v } else { v }),
            }
        }
    }

    fn spawn(
        &mut self,
        d: &SystemDef,
        cp: &impl Fn(usize) -> ControlPoint,
        active: Option<usize>,
        rng: &mut ParticleRng,
        sheets: &impl Sheets,
        created: f32,
    ) -> P {
        self.serial += 1;
        let mut p = P {
            position: cp(0).position,
            velocity: Vec3::ZERO,
            age: 0.0,
            life: 1.0,
            radius: d.radius,
            color: d.color,
            alpha: d.alpha,
            roll: d.rotation,
            roll_speed: d.rotation_speed,
            sequence: d.sequence,
            flip: false,
            fade_in: 0.0,
            fade_out: 0.0,
            serial: self.serial,
            target: None,
            radius_mul: 1.0,
            alpha_mul: 1.0,
            tint: None,
            trail: DEFAULT_TRAIL_LENGTH,
            motion: Vec3::ZERO,
            bone: None,
            alpha0: d.alpha,
            radius0: d.radius,
            osc: [[0.0; 4]; 2],
        };
        let mut k = 0;
        for o in &d.operators {
            if let Op::Oscillate { vector, rate, frequency, start, end, .. } = *o {
                if self.seed == 0 && (vector || k >= 2) {
                    self.seed = u64::from((rng.random() * 4_294_967_296.0) as u32) | 1 << 32;
                }
                if let Some(slot) = p.osc.get_mut(k) {
                    *slot = [rng.float(start.0, start.1), rng.float(end.0, end.1), rng.float(rate.0.x, rate.1.x), rng.float(frequency.0.x, frequency.1.x)];
                }
                k += 1;
            }
        }
        for i in &d.initializers {
            match *i {
                Init::Sphere {
                    cp: c,
                    distance,
                    bias,
                    bias_abs,
                    speed,
                    local_min,
                    local_max,
                } => {
                    let point = cp(c);
                    let mut dir = random_dir(rng) * bias;
                    for k in 0..3 {
                        if bias_abs[k] != 0.0 {
                            dir[k] = dir[k].abs();
                        }
                    }
                    let dir = dir.normalize_or_zero();
                    p.position = point.position + dir * distance.draw(rng);
                    p.velocity = dir * speed.draw(rng) + point.rotation * pick3(rng, local_min, local_max);
                }
                Init::Box { cp: c, min, max } => p.position = cp(c).position + pick3(rng, min, max),
                Init::OnModel { cp: c, scale, bias } if !self.model.is_empty() => {
                    // The best of the tries (effects.md 14.1): a random
                    // box, a point uniform in it, scored by U(0, 72) plus
                    // its offset from the point along the bias.
                    let at = cp(c).position;
                    let tries = if bias != Vec3::ZERO { 6 } else { 1 };
                    let mut best: Option<(f32, usize, Vec3, Vec3)> = None;
                    for _ in 0..tries {
                        let i = (rng.random() * self.model.len() as f32).min(self.model.len() as f32 - 1.0) as usize;
                        let u = rng.vec3(-1.0, 1.0) * scale;
                        let point = self.model[i].transform_point3(u);
                        let score = rng.float(0.0, 72.0) + (point - at).dot(bias);
                        if best.is_none_or(|b| score > b.0) {
                            best = Some((score, i, u, point));
                        }
                    }
                    if let Some((_, i, u, point)) = best {
                        p.position = point;
                        p.bone = Some((i as u16, u));
                    }
                }
                // No model given: at the point.
                Init::OnModel { cp: c, .. } => p.position = cp(c).position,
                Init::Offset {
                    cp: c,
                    min,
                    max,
                    local,
                    proportional,
                } => {
                    let mut off = pick3(rng, min, max);
                    if proportional {
                        off *= p.radius;
                    }
                    if local {
                        off = cp(c).rotation * off;
                    }
                    p.position += off;
                }
                Init::Velocity {
                    cp: c,
                    speed,
                    local_min,
                    local_max,
                } => {
                    p.velocity += random_dir(rng) * speed.draw(rng) + cp(c).rotation * pick3(rng, local_min, local_max);
                }
                Init::VelocityNoise { cp: c, min, max, local } => {
                    let v = pick3(rng, min, max);
                    p.velocity += if local { cp(c).rotation * v } else { v };
                }
                Init::Lifetime(pick) => p.life = pick.draw(rng),
                Init::LifetimeFromSequence { fps } => {
                    // A material without a sheet keeps the life it has:
                    // real TF2's Minigun brass (`effects/mini_casing`, no
                    // sheet; life 2 s, then this at 30 fps) flies for
                    // seconds, not one frame (specs/tf2/particles.md 7).
                    if let Some(m) = d.material
                        && fps > 0.0
                        && let Some(frames) = sheets.sheet_frames(m, p.sequence)
                    {
                        p.life = frames as f32 / fps;
                    }
                }
                Init::Radius(pick) => p.radius = pick.draw(rng),
                Init::Color { a, b, tint, cp: c } => {
                    p.color = tint_toward_light(a.lerp(b, rng.random()), cp(c).light, tint);
                }
                Init::Alpha(pick) => p.alpha = pick.draw(rng),
                Init::Rotation { initial, offset } => p.roll = initial + offset.draw(rng),
                Init::YawFlip { fraction } => p.flip = rng.random() < fraction,
                Init::Sequence { min, max } => p.sequence = rng.int(min as i32, max as i32).max(0) as u32,
                Init::MoveBetween { end, speed, start_offset } => {
                    let to = cp(end).position - p.position;
                    let distance = to.length();
                    let dir = to.normalize_or_zero();
                    if speed > 0.0 && distance > 0.0 {
                        let along = start_offset.clamp(0.0, distance);
                        p.position += dir * along;
                        p.velocity = dir * speed;
                        p.life = (distance - along) / speed;
                    }
                }
                Init::AssignTargetCp => p.target = active.and_then(|a| u8::try_from(a).ok()),
                Init::RemapScalar {
                    input,
                    input_min,
                    input_max,
                    output,
                    output_min,
                    output_max,
                    scale_initial,
                    in_range_only,
                    start,
                    end,
                } => {
                    let Some(x) = p.scalar(input, created) else { continue };
                    let inside = x >= input_min.min(input_max) && x <= input_min.max(input_max);
                    if !in_window(created, start, end) || (in_range_only && !inside) {
                        continue;
                    }
                    let v = output_min + (output_max - output_min) * unlerp(x, input_min, input_max);
                    let v = if scale_initial { p.scalar(output, created).unwrap_or(1.0) * v } else { v };
                    p.set_scalar(output, v);
                }
                Init::RemapToVector {
                    cp: c,
                    input,
                    input_min,
                    input_max,
                    output,
                    output_min,
                    output_max,
                    local,
                    scale_initial,
                    start,
                    end,
                } => {
                    let Some(x) = p.scalar(input, created) else { continue };
                    if !in_window(created, start, end) {
                        continue;
                    }
                    let v = output_min.lerp(output_max, unlerp(x, input_min, input_max));
                    match output {
                        0 => {
                            let point = cp(c);
                            let off = if local { point.rotation * v } else { v };
                            p.position = if scale_initial { p.position * v } else { point.position + off };
                        }
                        6 => p.color = if scale_initial { p.color * v } else { v },
                        _ => {}
                    }
                }
                Init::TrailLength(pick) => p.trail = pick.draw(rng),
                Init::LifetimeFromCp => {
                    if let Some(t) = p.target {
                        let life = cp(t as usize).duration;
                        if life > 0.0 {
                            p.life = life;
                        }
                    }
                }
            }
        }
        for o in &d.operators {
            match *o {
                Op::FadeIn { time, .. } => p.fade_in = time.draw(rng),
                Op::FadeOut { time, .. } => p.fade_out = time.draw(rng),
                _ => {}
            }
        }
        (p.alpha0, p.radius0) = (p.alpha, p.radius);
        p
    }

    /// What it shows now, as one-frame pool particles in engine space
    /// (`to_engine` maps a game-space point), facing `eye` (engine).
    pub fn draw(
        &self,
        defs: &[SystemDef],
        to_engine: &impl Fn(Vec3) -> Vec3,
        scale: f32,
        eye: Vec3,
        sheets: &impl Sheets,
        out: &mut Vec<Particle>,
    ) {
        let d = &defs[self.def];
        if let Some(material) = d.material {
            for render in d.renderers() {
                self.draw_with(d, render, material, to_engine, scale, eye, sheets, out);
            }
        }
        for child in self.children.iter().filter_map(|c| c.1.as_ref()) {
            child.draw(defs, to_engine, scale, eye, sheets, out);
        }
    }

    /// Its own particles drawn by one of its renderers.
    #[allow(clippy::too_many_arguments)]
    fn draw_with(
        &self,
        d: &SystemDef,
        render: &Render,
        material: usize,
        to_engine: &impl Fn(Vec3) -> Vec3,
        scale: f32,
        eye: Vec3,
        sheets: &impl Sheets,
        out: &mut Vec<Particle>,
    ) {
        if let Render::Rope = render {
            let mut order: Vec<&P> = self.particles.iter().collect();
            order.sort_by_key(|p| p.serial);
            for pair in order.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let (pa, pb) = (to_engine(a.position), to_engine(b.position));
                let mid = (pa + pb) * 0.5;
                let side = (pb - pa).cross(mid - eye).normalize_or_zero();
                let (ra, rb) = (Self::radius(d, a) * scale, Self::radius(d, b) * scale);
                let mut q = Particle::new(mid, 1.0, material, 0.0);
                q.color = Self::color(d, a);
                q.alpha = Self::alpha(d, a);
                q.shape = Shape::Quad {
                    corners: [pa - side * ra, pb - side * rb, pb + side * rb, pa + side * ra],
                    rect: [0.0, 0.0, 1.0, 1.0],
                };
                out.push(q);
            }
            return;
        }
        for p in &self.particles {
            let radius = Self::radius(d, p);
            let alpha = Self::alpha(d, p);
            // Its render alpha byte (particles.md 10.8): 0 is not drawn.
            if radius <= 0.0 || alpha < ALPHA_DRAWN {
                continue;
            }
            let alpha = (alpha * 255.0).round() / 255.0;
            let at = to_engine(p.position);
            let mut q = Particle::new(at, p.life.max(1e-3), material, radius * scale);
            q.size = Ramp::constant(radius * scale);
            q.age = p.age;
            q.color = overbright(Self::color(d, p), sheets.overbright(material));
            q.alpha = alpha;
            q.roll = p.roll.to_radians();
            // Yaw flip: the sprite mirrored (impact_effects.md 14).
            q.flip = p.flip;
            q.sequence = p.sequence as u16;
            match *render {
                Render::Sprites { rate, as_fps, fit_life, orientation } => {
                    let frames = sheets.frames(material, p.sequence).max(1) as f32;
                    q.fps = if fit_life {
                        frames / p.life.max(1e-3)
                    } else if as_fps {
                        rate
                    } else {
                        rate * frames
                    };
                    q.frame_loop = !fit_life;
                    if orientation != 0 {
                        q.shape = Shape::Flat { normal: Vec3::Y, yaw: p.roll, yaw_speed: 0.0 };
                        q.size = Ramp::constant(radius * scale * 2.0);
                    }
                }
                Render::Trail { min_length, max_length, length_fade_in } => {
                    // Back along its motion over its last step, as long
                    // as that speed x its trail length x the fade-in by
                    // age, then clamped; nothing when it didn't move
                    // (particles.md 10.7: a flame pilot locked to a still
                    // muzzle shows no streak).
                    let Some(length) = trail_length(p.motion, p.trail, p.age, min_length, max_length, length_fade_in) else {
                        continue;
                    };
                    let back = to_engine(p.position - p.motion.normalize() * length);
                    // The strip's full width is the radius, at most its
                    // length (10.7 step 5); the texture's v runs from the
                    // tail (0) to the head (1): TF2's `effects/tracer1` is
                    // bright at the bullet (Open questions 4).
                    let width = radius.min(length);
                    let side = (at - back).cross(at - eye).normalize_or_zero() * width * scale * 0.5;
                    q.position = (at + back) * 0.5;
                    q.shape = Shape::Quad {
                        corners: [at - side, back - side, back + side, at + side],
                        rect: [0.0, 0.0, 1.0, 1.0],
                    };
                }
                _ => {}
            }
            out.push(q);
        }
    }

    /// Its radius as drawn: what the operators left (`Op::RadiusScale`
    /// writes it each step) times the distance remap's factor.
    fn radius(_: &SystemDef, p: &P) -> f32 {
        p.radius * p.radius_mul
    }

    /// Its alpha as drawn (0-1): what the operators left (`Op::
    /// FadeAndDecay` writes it each step), the distance remap's factor and
    /// the random fades'.
    fn alpha(d: &SystemDef, p: &P) -> f32 {
        let mut a = p.alpha * p.alpha_mul;
        for o in &d.operators {
            match *o {
                Op::FadeIn { proportional, .. } => {
                    let time = if proportional { p.fade_in * p.life } else { p.fade_in };
                    if time > 0.0 {
                        a *= (p.age / time).clamp(0.0, 1.0);
                    }
                }
                Op::FadeOut { proportional, .. } => {
                    let time = if proportional { p.fade_out * p.life } else { p.fade_out };
                    if time > 0.0 {
                        a *= ((p.life - p.age) / time).clamp(0.0, 1.0);
                    }
                }
                _ => {}
            }
        }
        a.clamp(0.0, 1.0)
    }

    /// Its colour: "Color Fade" from its initial colour (or tint) toward
    /// the target by its life fraction's place in the window, clamped,
    /// eased (particles.md 10.4); a later fade overrides an earlier one.
    fn color(d: &SystemDef, p: &P) -> Vec3 {
        let c0 = p.tint.unwrap_or(p.color);
        if p.life <= 0.0 {
            return c0;
        }
        let t = p.age / p.life;
        let mut c = c0;
        for o in &d.operators {
            if let Op::ColorFade { color, start, end, ease } = *o
                && end != start
            {
                let mut x = ((t - start) / (end - start)).clamp(0.0, 1.0);
                if ease {
                    x = smooth(x);
                }
                c = c0 + (color - c0) * x;
            }
        }
        c
    }
}

/// An info_particle_system's effect: its running instances (a new one at
/// each start; stopped ones fade out), following its entity.
#[derive(Component, Debug)]
pub struct SystemEmitter {
    pub placed: PlacedSystem,
    pub instances: Vec<Instance>,
    was_on: bool,
    rng: ParticleRng,
    /// The light last sampled for its control point and where (game
    /// space): sampled again only when the entity moves.
    light: Option<(Vec3, Vec3)>,
}

impl SystemEmitter {
    pub fn new(placed: PlacedSystem, seed: u64) -> Self {
        Self {
            placed,
            instances: Vec::new(),
            was_on: false,
            rng: ParticleRng::new(seed),
            light: None,
        }
    }

    /// Turned on: a new instance, skipped ahead to `late` (how long after
    /// the server's start this client makes it) when that's over 0.01 s
    /// (specs/source/particles_and_smoke.md 2.5, specs/tf2/particles.md
    /// 10.10); turned off: every instance stops emitting (2.5).
    pub fn switch(&mut self, defs: &[SystemDef], on: bool, late: f32) {
        if on && !self.was_on {
            let mut i = Instance::new(defs, self.placed.system);
            if late > SKIP_AFTER {
                i.skip_to(defs, late);
            }
            self.instances.push(i);
        } else if !on && self.was_on {
            for i in &mut self.instances {
                i.stop();
            }
        }
        self.was_on = on;
    }

    pub fn count(&self) -> usize {
        self.instances.iter().map(Instance::count).sum()
    }
}

/// The loaded map's particle system definitions (`MapData::particle_systems`).
#[derive(Resource, Clone, Debug, Default)]
pub struct ParticleSystemDefs(pub std::sync::Arc<MapParticleSystems>);

/// Run and show every placed particle system near enough and not hidden by
/// visibility (hidden ones are frozen, as the game sleeps unseen effects).
#[allow(clippy::type_complexity)]
pub fn update_systems(
    time: Res<Time>,
    defs: Option<Res<ParticleSystemDefs>>,
    materials: Option<Res<crate::map::particles::ParticleMaterials>>,
    eyes: Query<
        &GlobalTransform,
        (
            With<Camera3d>,
            Without<monitor::ScreenCamera>,
            Without<SkyboxCamera>,
            Without<ViewModelCamera>,
            Without<water::WaterReflectionCamera>,
        ),
    >,
    mut emitters: Query<(
        &mut SystemEmitter,
        &GlobalTransform,
        Option<&EntityPart>,
        Option<&Visibility>,
    )>,
    light_field: Option<Res<crate::map::LightField>>,
    tick: Option<Res<Time<Fixed>>>,
    mut pool: ResMut<Particles>,
) {
    let Some(defs) = defs else { return };
    let defs = &defs.0;
    let dt = time.delta_secs().min(0.25);
    // How late this client makes a turned-on effect: the server's start
    // time isn't sent, so the least it can be, one tick (its update
    // arrives at least a tick after the start: particles_and_smoke.md
    // 2.5, inferred).
    let late = tick.map_or(1.0 / 66.0, |t| t.timestep().as_secs_f32());
    let eye = eyes.iter().next().map(|e| e.translation());
    let sheets = MaterialSheets(materials.as_ref().map(|m| &m.0));
    let scale = defs.scale.max(1e-6);
    // Engine space to game space and back (Source: Z up).
    let to_game = |v: Vec3| Vec3::new(v.x, -v.z, v.y) / scale;
    let to_engine = |v: Vec3| Vec3::new(v.x, v.z, -v.y) * scale;
    for (mut e, at, part, visibility) in &mut emitters {
        let exists = part.is_none_or(|p| p.exists);
        let on = exists && part.map_or(e.placed.start_on, |p| p.on);
        e.switch(&defs.defs, on, late);
        if !exists {
            e.instances.clear();
            continue;
        }
        if e.instances.is_empty() {
            continue;
        }
        let (_, rotation, origin) = at.to_scale_rotation_translation();
        let far = defs.defs[e.placed.system].max_draw_distance * scale;
        let near = eye.is_some_and(|eye| eye.distance(origin) <= far);
        if visibility == Some(&Visibility::Hidden) || !near {
            continue;
        }
        let position = to_game(origin);
        let e = &mut *e;
        let light = match (e.light, light_field.as_deref()) {
            (Some((at, light)), _) if at.distance_squared(position) < 1.0 => light,
            (_, Some(field)) => {
                let light = point_light(&(field.0.0)(origin));
                e.light = Some((position, light));
                light
            }
            (_, None) => Vec3::ONE,
        };
        let cp = ControlPoint {
            position,
            rotation: Quat::from_xyzw(rotation.x, -rotation.z, rotation.y, rotation.w),
            light,
            duration: 0.0,
            velocity: Vec3::ZERO,
        };
        let cps = [cp; MAX_CONTROL_POINTS];
        for i in &mut e.instances {
            i.step(&defs.defs, &cps, dt, &mut e.rng, &sheets);
        }
        e.instances.retain(|i| !i.finished(&defs.defs));
        let Some(eye) = eye else { continue };
        let mut out = Vec::new();
        for i in &e.instances {
            i.draw(&defs.defs, &to_engine, scale, eye, &sheets, &mut out);
        }
        if !out.is_empty() {
            let mut g = ParticleGroup::new(default());
            g.particles = out;
            g.capped = false;
            g.skip_first = true;
            g.kill_after = Some(0.0);
            pool.add(g);
        }
    }
}

/// One-shot systems games started (`StartParticleSystem`), with the
/// definitions they index (dropped when the map's change).
#[derive(Default)]
pub struct StartedSystems {
    defs: Option<std::sync::Arc<MapParticleSystems>>,
    running: Vec<Running>,
    rng: Option<ParticleRng>,
}

/// A started system: its instance, its control points (0, and 1 when
/// it was given an end), how long it emits, what it follows with its
/// frame last step (engine space), and what started it
/// (`StartParticleSystem::tag`).
struct Running {
    instance: Instance,
    cp: ControlPoint,
    end: Option<ControlPoint>,
    /// Control points its tag sets (`ParticleControlPoints`: game
    /// space, kept where last set) and the active one.
    points: Vec<(usize, ControlPoint)>,
    active: Option<usize>,
    emit_for: Option<f32>,
    follow: Option<(crate::map::psys::Follow, Transform)>,
    tag: Option<Entity>,
    /// Drawn in the view-model pass (its tag's `ParticleModel`).
    view_model: bool,
}

impl StartedSystems {
    /// Start `name` at `position` facing `forward` (game space) with
    /// `light` there (`ControlPoint::light`); false when the definitions
    /// lack it.
    pub fn start(
        &mut self,
        defs: &std::sync::Arc<MapParticleSystems>,
        name: &str,
        position: Vec3,
        forward: Vec3,
        light: Vec3,
    ) -> bool {
        self.start_for(defs, name, position, forward, light, None)
    }

    /// `start`, its emitters stopped `emit_for` seconds in (None: as its
    /// definition runs).
    pub fn start_for(
        &mut self,
        defs: &std::sync::Arc<MapParticleSystems>,
        name: &str,
        position: Vec3,
        forward: Vec3,
        light: Vec3,
        emit_for: Option<f32>,
    ) -> bool {
        if !self.defs.as_ref().is_some_and(|d| std::sync::Arc::ptr_eq(d, defs)) {
            self.defs = Some(defs.clone());
            self.running.clear();
        }
        let Some(def) = defs.find(name) else { return false };
        let rotation = Quat::from_rotation_arc(Vec3::X, forward.try_normalize().unwrap_or(Vec3::X));
        self.running.push(Running {
            instance: Instance::new(&defs.defs, def),
            cp: ControlPoint {
                position,
                rotation,
                light,
                duration: 0.0,
                velocity: Vec3::ZERO,
            },
            emit_for,
            follow: None,
            end: None,
            points: Vec::new(),
            active: None,
            tag: None,
            view_model: false,
        });
        true
    }

    /// Stop the emitters of the systems started with `tag`.
    pub fn stop_tagged(&mut self, tag: Entity) {
        for r in self.running.iter_mut().filter(|r| r.tag == Some(tag)) {
            r.instance.stop();
        }
    }

    /// Set the last started system's control point 1 (game space) and
    /// tag.
    pub fn set_last(&mut self, end: Option<Vec3>, tag: Option<Entity>) {
        if let Some(r) = self.running.last_mut() {
            r.end = end.map(|position| ControlPoint { position, ..r.cp });
            r.tag = tag;
        }
    }

    pub fn count(&self) -> usize {
        self.running.iter().map(|r| r.instance.count()).sum()
    }

    /// Set the control points of the systems started with `tag` (game
    /// space; one left out keeps its last place) and their active one.
    pub fn set_points(&mut self, tag: Entity, points: &[(usize, ControlPoint)], active: Option<usize>) {
        for r in self.running.iter_mut().filter(|r| r.tag == Some(tag)) {
            for (n, p) in points {
                match r.points.iter_mut().find(|(m, _)| m == n) {
                    Some(slot) => slot.1 = *p,
                    None => r.points.push((*n, *p)),
                }
            }
            r.active = active;
        }
    }

    /// The running systems' instances and tags.
    pub fn instances(&self) -> impl Iterator<Item = (&Instance, Option<Entity>)> {
        self.running.iter().map(|r| (&r.instance, r.tag))
    }

    /// Advance every started system by `dt`; finished ones go.
    pub fn step(&mut self, dt: f32, sheets: &impl Sheets) {
        let Some(defs) = self.defs.clone() else { return };
        let rng = self.rng.get_or_insert_with(|| ParticleRng::new(0x5_1A5));
        for Running { instance: i, cp, end, emit_for, points, active, .. } in &mut self.running {
            if emit_for.is_some_and(|t| i.age >= t) && i.emitting {
                i.stop();
            }
            let mut cps = [*cp; MAX_CONTROL_POINTS];
            if let Some(e) = end {
                cps[1] = *e;
            }
            for (n, p) in points.iter() {
                if let Some(c) = cps.get_mut(*n) {
                    *c = *p;
                }
            }
            i.step_with(&defs.defs, &cps, *active, dt, rng, sheets);
        }
        self.running.retain(|r| !r.instance.finished(&defs.defs));
    }
}

/// What the started systems are showing, kept each frame when the
/// resource exists (tests and probes insert it): each running system's
/// name, tag, whether it still emits, its particles' places (engine
/// space) with their own control points, and their radii (game units).
#[derive(Resource, Clone, Debug, Default)]
pub struct RunningSystems(pub Vec<RunningSystem>);

/// `psys_running`: the first call starts keeping `RunningSystems`, later
/// ones print it (each started system: name, emitting, particle count,
/// the first four particles' places in game units and radii, every one
/// with `all`). A probe for
/// effect captures (docs/OBSERVABILITY.md 6, "TF2 as the reference").
pub fn console(app: &mut App) {
    use chimera_core::console::ConsoleAppExt;
    app.console_command(
        "psys_running",
        "psys_running [all]: the started particle systems now (name, particles, the first four places in game units \
         and radii, or every one with `all`); the first call starts watching.",
        |w, args| {
            let shown_max = if args.iter().any(|a| a == "all") { usize::MAX } else { 4 };
            let scale = w.get_resource::<ParticleSystemDefs>().map_or(1.0, |d| d.0.scale.max(1e-6));
            let Some(shown) = w.get_resource::<RunningSystems>() else {
                w.init_resource::<RunningSystems>();
                return Ok(Some("psys_running: watching from now; run it again".into()));
            };
            let game = |v: Vec3| Vec3::new(v.x, -v.z, v.y) / scale;
            let mut out = format!("{} systems", shown.0.len());
            let all = shown_max == usize::MAX;
            // With `all`, each system's material (name, blend, depth
            // blend, overbright) and renderer: which child draws what.
            let looks = |name: &str| -> String {
                let Some(defs) = w.get_resource::<ParticleSystemDefs>() else { return String::new() };
                let name = name.rsplit(" > ").next().unwrap_or(name);
                let Some(d) = defs.0.find(name).map(|i| &defs.0.defs[i]) else { return String::new() };
                let mat = d.material.and_then(|m| w.get_resource::<crate::map::particles::ParticleMaterials>().and_then(|ms| ms.0.materials.get(m).cloned()));
                let mat = mat.map_or("no material".to_string(), |m| {
                    format!("{} {:?}{} ob {}", m.name, m.blend, if m.depth_blend { " depthblend" } else { "" }, m.overbright)
                });
                let render = d
                    .renderers()
                    .map(|r| match r {
                        crate::map::psys::Render::Sprites { orientation, .. } => format!("sprites o{orientation}"),
                        other => format!("{other:?}").split_whitespace().next().unwrap_or("").to_string(),
                    })
                    .collect::<Vec<_>>()
                    .join(" + ");
                format!(" [{mat}; {render}]")
            };
            for r in &shown.0.clone() {
                let looks = if all { looks(&r.name) } else { String::new() };
                out.push_str(&format!("\n{}{}: {} particles{looks}", r.name, if r.emitting { " (emitting)" } else { "" }, r.particles.len()));
                for ((p, cp), radius) in r.particles.iter().zip(&r.radii).take(shown_max) {
                    let g = game(*p);
                    out.push_str(&format!("\n  ({:.1} {:.1} {:.1}) r {radius:.2}", g.x, g.y, g.z));
                    if let (true, Some(cp)) = (all, cp) {
                        out.push_str(&format!(" cp {cp}"));
                    }
                }
                // With `all`, the control points set on its tag too.
                if let (true, Some(points)) = (all, r.tag.and_then(|t| w.get::<ParticleControlPoints>(t))) {
                    for (i, c) in &points.points {
                        out.push_str(&format!("\n  cp {i} at {:.1} {:.1} {:.1}", c.position.x, c.position.y, c.position.z));
                    }
                }
            }
            Ok(Some(out))
        },
    );
}

/// `psys_def <name>`: a loaded system definition as data (emitters,
/// initializers, operators, renderer, children by name) and its
/// material: what a capture's odd particle was told to do.
pub fn def_console(app: &mut App) {
    use chimera_core::console::ConsoleAppExt;
    app.console_command(
        "psys_def",
        "psys_def <name>: a loaded particle system's definition (its operators, renderer, material and children).",
        |w, args| {
            let Some(name) = args.first() else { return Ok(Some("psys_def <name>".into())) };
            let Some(defs) = w.get_resource::<ParticleSystemDefs>() else { return Ok(Some("no particle systems".into())) };
            let Some(d) = defs.0.find(name).map(|i| &defs.0.defs[i]) else { return Ok(Some(format!("no system {name}"))) };
            let material = d
                .material
                .and_then(|m| w.get_resource::<crate::map::particles::ParticleMaterials>().and_then(|ms| ms.0.materials.get(m).cloned()));
            let children: Vec<String> = d.children.iter().map(|&(c, delay)| format!("{} +{delay}s", defs.0.defs[c].name)).collect();
            let mut shown = d.clone();
            shown.children.clear();
            Ok(Some(format!("{shown:#?}\nmaterial: {material:#?}\nchildren: {children:?}")))
        },
    );
}

/// One running started system (`RunningSystems`).
#[derive(Clone, Debug, Default)]
pub struct RunningSystem {
    pub name: String,
    pub tag: Option<Entity>,
    pub emitting: bool,
    pub particles: Vec<(Vec3, Option<usize>)>,
    pub radii: Vec<f32>,
}

/// Run and show the one-shot systems games start (`StartParticleSystem`,
/// e.g. CS:S's slime splashes), each to its end.
#[allow(clippy::type_complexity)]
pub fn run_started_systems(
    time: Res<Time>,
    mut starts: MessageReader<StartParticleSystem>,
    mut stops: MessageReader<StopParticleSystems>,
    defs: Option<Res<ParticleSystemDefs>>,
    materials: Option<Res<crate::map::particles::ParticleMaterials>>,
    eyes: Query<
        &GlobalTransform,
        (
            With<Camera3d>,
            Without<monitor::ScreenCamera>,
            Without<SkyboxCamera>,
            Without<ViewModelCamera>,
            Without<water::WaterReflectionCamera>,
        ),
    >,
    mut started: Local<StartedSystems>,
    light_field: Option<Res<crate::map::LightField>>,
    mut pool: ResMut<Particles>,
    frames: Query<(&Transform, Option<&ChildOf>)>,
    (set_points, models): (Query<&ParticleControlPoints>, Query<&ParticleModel>),
    mut shown: Option<ResMut<RunningSystems>>,
    spatial: Option<avian3d::prelude::SpatialQuery>,
    characters: Query<Entity, With<crate::core::Intent>>,
) {
    let Some(defs) = defs else {
        starts.clear();
        stops.clear();
        return;
    };
    for s in stops.read() {
        started.stop_tagged(s.tag);
    }
    let defs = &defs.0;
    let scale = defs.scale.max(1e-6);
    let to_game = |v: Vec3| Vec3::new(v.x, -v.z, v.y) / scale;
    let to_engine = |v: Vec3| Vec3::new(v.x, v.z, -v.y) * scale;
    // A followed entity's frame this frame: its `Transform` under its
    // parent's (placed this frame; `GlobalTransform`s are last frame's).
    let frame = |e: Entity| -> Option<Transform> {
        let (t, parent) = frames.get(e).ok()?;
        Some(match parent.and_then(|p| frames.get(p.parent()).ok()) {
            Some((pt, _)) => pt.mul_transform(*t),
            None => *t,
        })
    };
    for s in starts.read() {
        let f = Vec3::new(s.forward.x, -s.forward.z, s.forward.y);
        let light = light_field
            .as_deref()
            .map_or(Vec3::ONE, |field| point_light(&(field.0.0)(s.position)));
        if !started.start_for(defs, &s.name, to_game(s.position), f, light, s.emit_for) {
            continue;
        }
        started.set_last(s.end.map(to_game), s.tag);
        let Some(last) = started.running.last_mut() else { continue };
        if let Some(follow) = s.follow
            && let Some(at) = frame(follow.entity)
        {
            // Placed by the frame now (the start's world place came from
            // the same eye, maybe a frame older).
            let p = at.transform_point(follow.local);
            last.cp.position = to_game(p);
            last.cp.rotation = follow_turn(&follow, &at);
            last.follow = Some((follow, at));
        }
    }
    if started.running.is_empty() {
        if let Some(shown) = shown.as_mut()
            && !shown.0.is_empty()
        {
            shown.0.clear();
        }
        return;
    }
    // Followed systems carried with their entity's frame since last step.
    for Running { instance: i, cp, follow, .. } in &mut started.running {
        let Some((f, last)) = follow else { continue };
        let Some(now) = frame(f.entity) else {
            // What it followed is gone (a flyer's trail): no more of it.
            i.stop();
            *follow = None;
            continue;
        };
        if now == *last {
            continue;
        }
        if f.carry {
            let moved = now.to_matrix() * last.to_matrix().inverse();
            let turn = now.rotation * last.rotation.inverse();
            let place = |g: Vec3| to_game(moved.transform_point3(to_engine(g)));
            let spin = |v: Vec3| {
                let e = turn * Vec3::new(v.x, v.z, -v.y);
                Vec3::new(e.x, -e.z, e.y)
            };
            i.carry(&place, &spin);
        }
        let p = now.transform_point(f.local);
        cp.position = to_game(p);
        cp.rotation = follow_turn(f, &now);
        *last = now;
    }
    // Control points the tags set (TF2's flame points).
    let tags: Vec<Entity> = started.running.iter().filter_map(|r| r.tag).collect();
    for tag in tags {
        let Ok(set) = set_points.get(tag) else { continue };
        let points: Vec<(usize, ControlPoint)> = set
            .points
            .iter()
            .map(|(n, p)| {
                let at = ControlPoint {
                    position: to_game(p.position),
                    duration: p.duration,
                    velocity: to_game(p.velocity),
                    ..ControlPoint::default()
                };
                (*n, at)
            })
            .collect();
        started.set_points(tag, &points, set.active);
    }
    // The model a tag's systems stand on (TF2's crit glow): control
    // point 0 at its origin, its hitboxes in game space.
    let to_game_m = Mat4::from_cols(Vec4::new(1.0, 0.0, 0.0, 0.0), Vec4::new(0.0, 0.0, 1.0, 0.0), Vec4::new(0.0, -1.0, 0.0, 0.0), Vec4::W)
        * Mat4::from_scale(Vec3::splat(1.0 / scale));
    for r in started.running.iter_mut() {
        let Some(m) = r.tag.and_then(|t| models.get(t).ok()) else { continue };
        r.cp.position = to_game(m.origin);
        r.view_model = m.view_model;
        let boxes: Vec<Mat4> = m.boxes.iter().map(|b| to_game_m * *b).collect();
        r.instance.set_model(&boxes);
    }
    let sheets = WorldSheets {
        sheets: MaterialSheets(materials.as_ref().map(|m| &m.0)),
        spatial: spatial.as_ref(),
        filter: avian3d::prelude::SpatialQueryFilter::from_excluded_entities(characters.iter()).with_mask(crate::core::SOLID_LAYERS),
        scale: defs.scale.max(1e-6),
    };
    started.step(time.delta_secs().min(0.25), &sheets);
    if let Some(shown) = shown.as_mut() {
        // Each started system, then each of its started children on its
        // own line (`root > child`, its own particles only): which child
        // draws what.
        fn rows(i: &Instance, path: &str, tag: Option<Entity>, defs: &[SystemDef], to_engine: &impl Fn(Vec3) -> Vec3, out: &mut Vec<RunningSystem>) {
            let d = &defs[i.def];
            out.push(RunningSystem {
                name: if path.is_empty() { d.name.clone() } else { format!("{path} > {}", d.name) },
                tag,
                emitting: i.emitting,
                particles: i.particles.iter().map(|p| (to_engine(p.position), p.target.map(usize::from))).collect(),
                radii: i.particles.iter().map(|p| Instance::radius(d, p)).collect(),
            });
            let path = if path.is_empty() { d.name.clone() } else { format!("{path} > {}", d.name) };
            for c in i.child_instances() {
                rows(c, &path, tag, defs, to_engine, out);
            }
        }
        let mut out = Vec::new();
        for r in &started.running {
            rows(&r.instance, "", r.tag, &defs.defs, &to_engine, &mut out);
        }
        shown.0 = out;
    }
    let Some(eye) = eyes.iter().next().map(|e| e.translation()) else {
        return;
    };
    // Drawn in the world pass with the world camera, first-person
    // effects too (specs/tf2/particles.md 6: placed at the view model's
    // muzzle re-projected into the world view, never drawn with the
    // view-model camera).
    // Those on a view model in the view-model pass (effects.md 14.1).
    for view_model in [false, true] {
        let mut out = Vec::new();
        for r in started.running.iter().filter(|r| r.view_model == view_model) {
            r.instance.draw(&defs.defs, &to_engine, scale, eye, &sheets, &mut out);
        }
        if !out.is_empty() {
            let mut g = ParticleGroup::new(default());
            g.particles = out;
            g.capped = false;
            g.skip_first = true;
            g.kill_after = Some(0.0);
            g.view_model = view_model;
            pool.add(g);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none(_: usize, _: u32) -> usize {
        1
    }

    /// Its creation frame (zero time, particles.md 9.1), so the next step
    /// emits.
    fn created(i: &mut Instance, defs: &[SystemDef], cps: &[ControlPoint]) {
        i.step(defs, cps, 0.0, &mut ParticleRng::new(0), &none);
    }

    fn system() -> SystemDef {
        SystemDef {
            name: "test".into(),
            material: Some(0),
            emitters: vec![Emitter::Continuously {
                start: 0.0,
                rate: 100.0,
                duration: 0.5,
            }],
            initializers: vec![Init::Lifetime(Pick::new(0.2, 0.2, 1.0))],
            operators: vec![
                Op::Movement {
                    gravity: Vec3::new(0.0, 0.0, 50.0),
                    drag: 0.0,
                },
                Op::Decay,
            ],
            renderer: Render::Sprites {
                rate: 1.0,
                as_fps: false,
                fit_life: false,
                orientation: 0,
            },
            ..default()
        }
    }

    #[test]
    fn continuous_emission_for_its_duration_then_dies_out() {
        let defs = vec![system()];
        let mut i = Instance::new(&defs, 0);
        let mut rng = ParticleRng::new(1);
        let cps = [ControlPoint::default()];
        created(&mut i, &defs, &cps);
        for _ in 0..10 {
            i.step(&defs, &cps, 0.01, &mut rng, &none);
        }
        // 100 a second for 0.1 s; none older than 0.2 s yet.
        assert!((9..=11).contains(&i.count()), "{}", i.count());
        for _ in 0..40 {
            i.step(&defs, &cps, 0.01, &mut rng, &none);
        }
        // At 0.5 s: the last 0.2 s worth alive.
        assert!((19..=21).contains(&i.count()), "{}", i.count());
        assert!(!i.finished(&defs));
        for _ in 0..30 {
            i.step(&defs, &cps, 0.01, &mut rng, &none);
        }
        assert_eq!(i.count(), 0);
        assert!(i.finished(&defs));
    }

    /// particles.md 9, test cases: the creation frame makes only the
    /// initial particle; the next frame's 128/s step makes two, half-way
    /// along the point's path and on its end (none on its start); the
    /// child's first step has its point standing still.
    #[test]
    fn a_new_system_makes_only_its_initial_particles_then_spreads_along_the_path_9() {
        let mut child = system();
        child.name = "child".into();
        child.operators.clear();
        child.initializers = vec![Init::Lifetime(Pick::new(10.0, 10.0, 1.0))];
        child.emitters = vec![Emitter::Continuously { start: 0.0, rate: 128.0, duration: 0.0 }];
        let mut def = child.clone();
        def.name = "parent".into();
        def.initial_particles = 1;
        def.children = vec![(1, 0.0)];
        let defs = vec![def, child];
        let mut rng = ParticleRng::new(1);
        let at = |x: f32| [ControlPoint { position: Vec3::new(x, 0.0, 0.0), ..default() }];
        let h = 1.0 / 60.0;
        let mut i = Instance::new(&defs, 0);
        i.step(&defs, &at(0.0), h, &mut rng, &none);
        assert_eq!(i.placed().len(), 1, "the initial particle only");
        assert_eq!(i.age, 0.0, "zero time");
        assert_eq!(i.child_instances().count(), 0, "children not run");
        // Next frame, the point moved 0 -> 60.
        i.step(&defs, &at(60.0), h, &mut rng, &none);
        let own = i.own_details(&defs);
        let new: Vec<(f32, f32)> = own[1..].iter().map(|d| (d.0.x, d.1)).collect();
        assert_eq!(new.len(), 2, "{new:?}");
        assert!((new[0].0 - 30.0).abs() < 1e-3 && (new[0].1 - h / 2.0).abs() < 1e-5, "{new:?}");
        assert!((new[1].0 - 60.0).abs() < 1e-3 && new[1].1.abs() < 1e-5, "{new:?}");
        let c = i.child_instances().next().expect("the child, a frame later");
        let xs: Vec<f32> = c.placed().iter().map(|p| p.0.x).collect();
        assert_eq!(xs, vec![60.0, 60.0], "no spread in its first step");
    }

    /// particles.md 10.11: an operator's strength by its system's clock.
    #[test]
    fn an_operators_own_fade_by_the_systems_clock_10_11() {
        let f = |i0, i1, o0, o1, p| OpFade { start_in: i0, end_in: i1, start_out: o0, end_out: o1, oscillate: p };
        // All 0: always 1.
        assert_eq!(f(0.0, 0.0, 0.0, 0.0, 0.0).strength(7.0), 1.0);
        // A hard stop at 0.5 (o0 = o1): 1 up to it, 0 after.
        let stop = f(0.0, 0.0, 0.5, 0.5, 0.0);
        assert_eq!((stop.strength(0.5), stop.strength(0.5001)), (1.0, 0.0));
        assert_eq!(stop.ends(), Some(0.5));
        // A fade-in over 2-4 s.
        let ramp = f(2.0, 4.0, 0.0, 0.0, 0.0);
        assert_eq!((ramp.strength(1.9), ramp.strength(3.0), ramp.strength(5.0)), (0.0, 0.5, 1.0));
        // In and out, the out's ends raised past the in's: 1 → 0 over 3-5.
        let both = f(0.0, 1.0, 3.0, 5.0, 0.0);
        assert!((both.strength(0.5) - 0.5).abs() < 1e-6 && (both.strength(4.0) - 0.5).abs() < 1e-6);
        assert_eq!(both.strength(5.5), 0.0);
        // The oscillation isn't applied (`OpFade::strength`): the
        // Minigun's starflash (0.12) stops at 0.5 s for good (8.3).
        let osc = f(0.0, 0.0, 0.5, 0.5, 0.12);
        assert_eq!((osc.strength(0.45), osc.strength(0.55), osc.strength(0.62)), (1.0, 0.0, 0.0));
    }

    /// particles.md 8.3, 8.3.1: `muzzle_minigun_constant`'s children by
    /// their data (rates, lives, caps, emitter fades), stepped at 60 a
    /// second: nothing in the creation frame, then per frame the core 2
    /// new, the starflash 4, 4, 4, 5, 4, 4 (alive to 25), the flare 2 (10
    /// alive from frame 5), the sparks 1, 3, 5 alive and held at their cap
    /// of 5; the starflash and flare make nothing after 0.5 s and are
    /// gone 0.1 s later; no smoke before 2 s, then some.
    #[test]
    fn the_minigun_flash_builds_up_by_its_caps_and_fades_8_3_1() {
        let child = |name: &str, rate: f32, life: (f32, f32), cap: usize, fade: OpFade| SystemDef {
            name: name.into(),
            max_particles: cap,
            emitters: vec![Emitter::Continuously { start: 0.0, rate, duration: 0.0 }],
            emitter_fades: vec![fade],
            initializers: vec![Init::Lifetime(Pick::new(life.0, life.1, 1.0))],
            operators: vec![Op::Decay],
            ..system()
        };
        let plain = OpFade::default();
        let stop = OpFade { start_out: 0.5, end_out: 0.5, ..plain };
        let parent = SystemDef {
            name: "muzzle_minigun_constant".into(),
            max_particles: 0,
            emitters: vec![],
            children: (1..=5).map(|c| (c, 0.0)).collect(),
            ..system()
        };
        let defs = vec![
            parent,
            child("core", 120.0, (0.1, 0.15), 24, plain),
            child("sparks", 100.0, (0.3, 0.6), 5, plain),
            child("starflash", 256.0, (0.1, 0.1), 32, stop),
            child("smoke", 12.0, (2.0, 3.0), 25, OpFade { start_in: 2.0, end_in: 4.0, ..plain }),
            child("flare", 128.0, (0.08, 0.08), 32, stop),
        ];
        let h = 1.0 / 60.0;
        let cps = [ControlPoint::default()];
        let mut rng = ParticleRng::new(3);
        let mut i = Instance::new(&defs, 0);
        i.step(&defs, &cps, h, &mut rng, &none);
        assert_eq!(i.count(), 0, "frame 0 shows nothing");
        // (new, alive) per child, per frame.
        let mut frames: Vec<Vec<(usize, usize)>> = Vec::new();
        for _ in 0..240 {
            i.step(&defs, &cps, h, &mut rng, &none);
            frames.push(
                i.child_instances()
                    .map(|c| {
                        let d = c.own_details(&defs);
                        (d.iter().filter(|p| p.1 < h * 0.999).count(), d.len())
                    })
                    .collect(),
            );
        }
        let of = |k: usize| frames.iter().map(|f| f[k]).collect::<Vec<_>>();
        let (core, sparks, star, smoke, flare) = (of(0), of(1), of(2), of(3), of(4));
        assert_eq!(core[..6].iter().map(|c| c.0).collect::<Vec<_>>(), [2; 6]);
        assert_eq!(core[..6].iter().map(|c| c.1).collect::<Vec<_>>(), [2, 4, 6, 8, 10, 12]);
        assert_eq!(star[..6].to_vec(), [(4, 4), (4, 8), (4, 12), (5, 17), (4, 21), (4, 25)]);
        assert!(star[6..9].iter().all(|s| (25..=26).contains(&s.1)), "{:?}", &star[6..9]);
        assert_eq!(flare[..6].to_vec(), [(2, 2), (2, 4), (2, 6), (2, 8), (2, 10), (2, 10)]);
        assert_eq!(sparks[..6].iter().map(|s| s.1).collect::<Vec<_>>(), [1, 3, 5, 5, 5, 5]);
        assert!(sparks.iter().all(|s| s.1 <= 5), "the sparks' cap");
        // Frame k ends at k/60 s: emitting to 0.5 s, none from frame 31.
        assert!(star[28].0 > 0 && flare[28].0 > 0);
        assert!(star[30..].iter().chain(&flare[30..]).all(|s| s.0 == 0), "nothing after 0.5 s");
        assert!(star[36..].iter().chain(&flare[36..]).all(|s| s.1 == 0), "gone 0.1 s later");
        assert!(smoke[..119].iter().all(|s| s.1 == 0), "no smoke before 2 s");
        assert!(smoke[239].1 > 0, "smoke by 4 s");
        assert!(!i.finished(&defs), "the core and sparks go on");
    }

    /// particles.md 9.2: 150/s makes 2, 3, 2, 3 a 1/60 s step; an endless
    /// emitter starting inside the step counts the whole step (120/s,
    /// start 0.005: at 1/120 and 1/60); `Explosion_Flashup`'s 3000/s for
    /// 0.01 s makes 30 in its first step, at 0.01 k / 30.
    #[test]
    fn continuous_counts_and_creation_times_follow_the_window_rule_9_2() {
        let h = 1.0 / 60.0;
        let one = |start: f32, rate: f32, duration: f32| {
            let mut d = system();
            d.operators.clear();
            d.initializers = vec![Init::Lifetime(Pick::new(10.0, 10.0, 1.0))];
            d.emitters = vec![Emitter::Continuously { start, rate, duration }];
            vec![d]
        };
        let mut rng = ParticleRng::new(1);
        let cps = [ControlPoint::default()];
        let defs = one(0.0, 150.0, 0.0);
        let mut i = Instance::new(&defs, 0);
        i.step(&defs, &cps, h, &mut rng, &none);
        let mut counts = Vec::new();
        for _ in 0..4 {
            let before = i.count();
            i.step(&defs, &cps, h, &mut rng, &none);
            counts.push(i.count() - before);
        }
        assert_eq!(counts, vec![2, 3, 2, 3]);
        let ages = |defs: &[SystemDef]| {
            let mut i = Instance::new(defs, 0);
            let mut rng = ParticleRng::new(1);
            i.step(defs, &cps, h, &mut rng, &none);
            i.step(defs, &cps, h, &mut rng, &none);
            i.own_details(defs).iter().map(|d| h - d.1).collect::<Vec<f32>>()
        };
        let made = ages(&one(0.005, 120.0, 0.0));
        assert_eq!(made.len(), 2);
        assert!((made[0] - h / 2.0).abs() < 1e-5 && (made[1] - h).abs() < 1e-5, "{made:?}");
        let made = ages(&one(0.0, 3000.0, 0.01));
        assert_eq!(made.len(), 30);
        for (k, c) in made.iter().enumerate() {
            assert!((c - 0.01 * (k + 1) as f32 / 30.0).abs() < 1e-5, "{k}: {c}");
        }
    }

    /// particles.md 9.2: a burst at 0 takes the first emitting step's
    /// start, so it is made where the point was (A), not where it went.
    #[test]
    fn a_burst_at_its_start_is_made_on_the_points_previous_place_9_2() {
        let mut d = system();
        d.operators.clear();
        d.emitters = vec![Emitter::Instantaneously { start: 0.0, count: 5 }];
        let defs = vec![d];
        let mut rng = ParticleRng::new(1);
        let at = |x: f32| [ControlPoint { position: Vec3::new(x, 0.0, 0.0), ..default() }];
        let mut i = Instance::new(&defs, 0);
        i.step(&defs, &at(0.0), 1.0 / 60.0, &mut rng, &none);
        assert_eq!(i.count(), 0);
        i.step(&defs, &at(40.0), 1.0 / 60.0, &mut rng, &none);
        let xs: Vec<f32> = i.placed().iter().map(|p| p.0.x).collect();
        assert_eq!(xs, vec![0.0; 5]);
    }

    /// effects.md 14.1: "Position on Model Random" with the (1, 1, 1)
    /// bias puts nearly every particle on the box far along +x +y +z (its
    /// score beats the other's by more than U(0, 72) can make up), inside
    /// it; "Movement Lock to Bone" keeps each where it is in its box as
    /// the box moves.
    #[test]
    fn particles_go_on_the_models_boxes_by_the_bias_and_follow_them_14_1() {
        let mut d = system();
        d.operators = vec![Op::LockToBone { cp: 0 }];
        d.initializers = vec![Init::Lifetime(Pick::new(10.0, 10.0, 1.0)), Init::OnModel { cp: 0, scale: 1.0, bias: Vec3::ONE }];
        d.emitters = vec![Emitter::Continuously { start: 0.0, rate: 600.0, duration: 0.0 }];
        let defs = vec![d];
        let near = Mat4::from_scale_rotation_translation(Vec3::splat(2.0), Quat::IDENTITY, Vec3::ZERO);
        let far = Mat4::from_scale_rotation_translation(Vec3::new(1.0, 2.0, 3.0), Quat::from_rotation_z(0.5), Vec3::splat(100.0));
        let mut i = Instance::new(&defs, 0);
        let mut rng = ParticleRng::new(9);
        let cps = [ControlPoint::default()];
        i.set_model(&[near, far]);
        created(&mut i, &defs, &cps);
        i.step(&defs, &cps, 0.1, &mut rng, &none);
        let placed = i.placed();
        assert_eq!(placed.len(), 60);
        let on_far = placed.iter().filter(|p| far.inverse().transform_point3(p.0).abs().max_element() <= 1.0 + 1e-4).count();
        assert!(on_far >= 55, "{on_far} of 60 on the far box");
        let on_near = placed.iter().filter(|p| near.inverse().transform_point3(p.0).abs().max_element() <= 1.0 + 1e-4).count();
        assert_eq!(on_far + on_near, 60, "all inside a box");
        // The far box moves 10 along x and turns: they go with it.
        let before: Vec<Vec3> = i.particles.iter().map(|p| far.inverse().transform_point3(p.position)).collect();
        let moved = Mat4::from_translation(Vec3::X * 10.0) * far * Mat4::from_rotation_x(0.3);
        i.set_model(&[near, moved]);
        i.step(&defs, &cps, 1.0 / 60.0, &mut rng, &none);
        for (p, b) in i.particles.iter().zip(before) {
            if b.abs().max_element() <= 1.0 + 1e-4 {
                let now = moved.inverse().transform_point3(p.position);
                assert!(now.abs_diff_eq(b, 1e-3), "{now} vs {b}");
            }
        }
    }

    /// "remap initial scalar" by creation time (specs/tf2/particles.md 8.1,
    /// the scattergun's flash): radius 8 to 5 over the system's first
    /// 0.015 s, then 3 to 0.5 until 0.05 s; life 0.1 to 0.05; placed 2 to
    /// 24 units out along the control point's axis; alpha 0.6 to 0.
    #[test]
    fn remaps_by_creation_time() {
        let remap = |output: u8, (input_min, input_max): (f32, f32), (output_min, output_max): (f32, f32), (start, end): (f32, f32)| Init::RemapScalar {
            input: 8,
            input_min,
            input_max,
            output,
            output_min,
            output_max,
            scale_initial: false,
            in_range_only: false,
            start,
            end,
        };
        let mut d = system();
        d.emitters = vec![Emitter::Continuously { start: 0.0, rate: 1000.0, duration: 0.05 }];
        d.operators = vec![Op::Decay];
        d.initializers = vec![
            Init::Radius(Pick::new(32.0, 32.0, 1.0)),
            Init::RemapToVector {
                cp: 0,
                input: 8,
                input_min: 0.0,
                input_max: 0.05,
                output: 0,
                output_min: Vec3::new(2.0, 0.0, 0.0),
                output_max: Vec3::new(24.0, 0.0, 0.0),
                local: true,
                scale_initial: false,
                start: -1.0,
                end: -1.0,
            },
            remap(1, (0.0, 0.05), (0.1, 0.05), (-1.0, -1.0)),
            remap(3, (0.0, 0.015), (8.0, 5.0), (0.0, 0.015)),
            remap(3, (0.015, 0.05), (3.0, 0.5), (0.015, 0.06)),
            remap(7, (0.0, 0.05), (0.6, 0.0), (-1.0, -1.0)),
        ];
        let defs = vec![d];
        let mut i = Instance::new(&defs, 0);
        let mut rng = ParticleRng::new(1);
        // The control point turned: its x axis is the world's y.
        let cps = [ControlPoint { rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_2), ..default() }];
        created(&mut i, &defs, &cps);
        i.step(&defs, &cps, 0.001, &mut rng, &none);
        let first = &i.particles[0];
        assert!((first.radius - 8.0).abs() < 0.3, "{}", first.radius);
        assert!((first.life - 0.1).abs() < 0.002, "{}", first.life);
        assert!((first.alpha - 0.6).abs() < 0.02, "{}", first.alpha);
        assert!((first.position - Vec3::new(0.0, 2.0, 0.0)).length() < 0.5, "{}", first.position);
        for _ in 0..39 {
            i.step(&defs, &cps, 0.001, &mut rng, &none);
        }
        // Made at 0.04 s: in the second window.
        let last = i.particles.last().unwrap();
        let want = 3.0 + (0.5 - 3.0) * (0.04 - 0.015) / 0.035;
        assert!((last.radius - want).abs() < 0.1, "{} vs {want}", last.radius);
        assert!((last.life - 0.06).abs() < 0.002, "{}", last.life);
        assert!(last.alpha < 0.14, "{}", last.alpha);
        assert!((last.position.y - (2.0 + 22.0 * 0.8)).abs() < 0.6, "{}", last.position);
        // None made in the first step was spawned at the step's end only:
        // creation times spread over the step (1000 a second, 1 ms steps).
        assert!(i.particles.len() >= 39, "{}", i.particles.len());
    }

    /// The overbright factor multiplies the linear colour: handed on in
    /// gamma, it comes back as o times the linear value.
    #[test]
    fn overbright_multiplies_the_linear_colour() {
        let c = Vec3::new(0.2, 0.2, 1.0);
        let up = overbright(c, 5.0);
        let back = Color::srgb(up.x, up.y, up.z).to_linear();
        let lin = Color::srgb(c.x, c.y, c.z).to_linear();
        assert!((back.red - lin.red * 5.0).abs() < 1e-3 && (back.blue - 5.0).abs() < 1e-2, "{back:?}");
        assert_eq!(overbright(c, 1.0), c);
    }

    /// Drag takes off the same over a second whatever the step: 0.1 per
    /// 1/30 s leaves 0.9^30 of the speed after one second.
    #[test]
    fn drag_does_not_depend_on_the_step() {
        let mut d = system();
        d.emitters = vec![Emitter::Instantaneously { start: 0.0, count: 1 }];
        d.initializers = vec![Init::Lifetime(Pick::new(5.0, 5.0, 1.0)), Init::VelocityNoise { cp: 0, min: Vec3::X * 100.0, max: Vec3::X * 100.0, local: false }];
        d.operators = vec![Op::Movement { gravity: Vec3::ZERO, drag: 0.1 }, Op::Decay];
        let defs = vec![d];
        let speed_after = |step: f32| {
            let mut i = Instance::new(&defs, 0);
            let mut rng = ParticleRng::new(1);
            let cps = [ControlPoint::default()];
            created(&mut i, &defs, &cps);
            i.step(&defs, &cps, step, &mut rng, &none);
            let mut t = step;
            while t < 1.0 - 1e-4 {
                i.step(&defs, &cps, step, &mut rng, &none);
                t += step;
            }
            i.particles[0].velocity.length()
        };
        // 0.1 per 1/30 s (particles.md 10.5).
        let want = 100.0 * 0.9f32.powi(30);
        for step in [1.0 / 60.0, 1.0 / 250.0, 0.0005] {
            let v = speed_after(step);
            assert!((v - want).abs() < want * 0.05, "step {step}: {v} vs {want}");
        }
    }

    #[test]
    fn gravity_moves_and_stop_lets_particles_finish() {
        let mut d = system();
        d.emitters = vec![Emitter::Instantaneously { start: 0.0, count: 5 }];
        d.initializers = vec![Init::Lifetime(Pick::new(1.0, 1.0, 1.0))];
        let defs = vec![d];
        let mut i = Instance::new(&defs, 0);
        let mut rng = ParticleRng::new(2);
        let cps = [ControlPoint::default()];
        created(&mut i, &defs, &cps);
        i.step(&defs, &cps, 0.1, &mut rng, &none);
        assert_eq!(i.count(), 5);
        i.stop();
        i.step(&defs, &cps, 0.5, &mut rng, &none);
        assert_eq!(i.count(), 5, "stopping keeps live particles");
        // Rising under gravity 50 up for 0.6 s.
        assert!(
            i.particles.iter().all(|p| p.position.z > 5.0),
            "{:?}",
            i.particles[0].position
        );
        i.step(&defs, &cps, 0.5, &mut rng, &none);
        assert_eq!(i.count(), 0);
    }

    /// One particle (alpha 1, radius `radius`, life `life`) made at 0 by
    /// a burst, run by `operators`; created (the zero-time frame) and
    /// ready to step.
    fn one(life: f32, radius: f32, operators: Vec<Op>) -> (Vec<SystemDef>, Instance) {
        let d = SystemDef {
            emitters: vec![Emitter::Instantaneously { start: 0.0, count: 1 }],
            initializers: vec![Init::Lifetime(Pick::new(life, life, 1.0)), Init::Radius(Pick::new(radius, radius, 1.0))],
            operators,
            max_step: 1.0,
            ..system()
        };
        let defs = vec![d];
        let mut i = Instance::new(&defs, 0);
        created(&mut i, &defs, &[ControlPoint::default()]);
        (defs, i)
    }

    fn step(defs: &[SystemDef], i: &mut Instance, h: f32) {
        i.step(defs, &[ControlPoint::default()], h, &mut ParticleRng::new(4), &none);
    }

    /// particles.md 10.2: start alpha 0, fade in 0.1-0.2, out 0.7-1:
    /// untouched before the fade-in (1), S(0.5) = 0.5 inside it, and left
    /// at its last in-window value after it (0.5, not 1).
    #[test]
    fn alpha_fade_and_decay_is_left_alone_outside_its_windows_10_2() {
        let fade = Op::FadeAndDecay { start_alpha: 0.0, end_alpha: 0.0, fade_in: (0.1, 0.2), fade_out: (0.7, 1.0) };
        let (defs, mut i) = one(1.0, 5.0, vec![fade.clone()]);
        let alpha = |i: &Instance| Instance::alpha(&defs[0], &i.particles[0]);
        step(&defs, &mut i, 0.05);
        assert!((alpha(&i) - 1.0).abs() < 1e-5, "t 0.05: {}", alpha(&i));
        step(&defs, &mut i, 0.1);
        assert!((alpha(&i) - 0.5).abs() < 1e-4, "t 0.15: {}", alpha(&i));
        step(&defs, &mut i, 0.1);
        assert!((alpha(&i) - 0.5).abs() < 1e-4, "t 0.25: {}", alpha(&i));
        // The fade-out rescales the initial alpha: S(0.5) of the way to 0.
        step(&defs, &mut i, 0.6);
        assert!((alpha(&i) - 0.5).abs() < 1e-4, "t 0.85: {}", alpha(&i));
        // Lifetime 0.01, age 0.0117: removed this step.
        let (defs, mut i) = one(0.01, 5.0, vec![fade]);
        step(&defs, &mut i, 0.0117);
        assert_eq!(i.count(), 0);
        // The loader's ordering: an end before its start raised to it,
        // then the starts (and the ends) swapped when the fade-in's is
        // later.
        assert_eq!(fade_windows((0.8, 0.9), (0.2, 0.1)), ((0.2, 0.2), (0.8, 0.9)));
    }

    /// particles.md 10.3: 3 -> 11, bias 0.9 (rational: g(0.05) = 0.321,
    /// not the power curve's 0.634), r0 2 at t 0.05: 11.1; outside the
    /// window the radius is left as it is.
    #[test]
    fn radius_scale_has_the_rational_bias_10_3() {
        assert!((rational_bias(0.05, 0.9) - 0.3214).abs() < 1e-3);
        assert!((rational_bias(0.5, 0.9) - 0.9).abs() < 1e-5 && rational_bias(0.3, 0.5) == 0.3);
        let scale = Op::RadiusScale { start_scale: 3.0, end_scale: 11.0, start: 0.0, end: 1.0, bias: 0.9, ease: false };
        let (defs, mut i) = one(1.0, 2.0, vec![scale]);
        step(&defs, &mut i, 0.05);
        assert!((i.own_radii(&defs)[0] - 11.14).abs() < 0.02, "{:?}", i.own_radii(&defs));
        let late = Op::RadiusScale { start_scale: 1.0, end_scale: 2.0, start: 0.0, end: 0.5, bias: 0.5, ease: false };
        let (defs, mut i) = one(1.0, 2.0, vec![late]);
        step(&defs, &mut i, 0.4);
        step(&defs, &mut i, 0.3);
        assert!((i.own_radii(&defs)[0] - 3.6).abs() < 1e-3, "kept at t 0.4's: {:?}", i.own_radii(&defs));
    }

    /// particles.md 10.4: from the initial colour, eased, held at the
    /// target after the end; equal times do nothing.
    #[test]
    fn color_fade_goes_from_the_initial_colour_10_4() {
        let fade = Op::ColorFade { color: Vec3::ZERO, start: 0.0, end: 0.5, ease: true };
        let (defs, mut i) = one(1.0, 5.0, vec![fade, Op::ColorFade { color: Vec3::X, start: 0.3, end: 0.3, ease: true }]);
        step(&defs, &mut i, 0.25);
        let c = Instance::color(&defs[0], &i.particles[0]);
        assert!((c - Vec3::splat(0.5)).length() < 1e-4, "{c}");
        step(&defs, &mut i, 0.5);
        assert_eq!(Instance::color(&defs[0], &i.particles[0]), Vec3::ZERO);
    }

    /// particles.md 10.5: drag 0.1 per 1/30 s: a step of 1/60 after one
    /// whose motion was (10, 0, 0) moves (9.49, 0, 0).
    #[test]
    fn movement_basics_drag_is_per_thirtieth_of_a_second_10_5() {
        let (mut defs, _) = one(5.0, 5.0, vec![Op::Movement { gravity: Vec3::ZERO, drag: 0.1 }]);
        defs[0].initializers.push(Init::VelocityNoise { cp: 0, min: Vec3::X * 600.0, max: Vec3::X * 600.0, local: false });
        let mut i = Instance::new(&defs, 0);
        created(&mut i, &defs, &[ControlPoint::default()]);
        let h = 1.0 / 60.0;
        step(&defs, &mut i, h);
        let first = i.particles[0].position.x;
        // Its first step: v h less this step's drag (10.6).
        assert!((first - 10.0 * 0.9f32.sqrt()).abs() < 1e-3, "{first}");
        step(&defs, &mut i, h);
        let second = i.particles[0].position.x - first;
        assert!((second / first - 0.9487).abs() < 1e-3, "{second} after {first}");
        assert!((i.particles[0].motion.x * h - second).abs() < 1e-3);
    }

    /// particles.md 10.7: L0 13.7 with min 32, max 512 is 32; min 32 over
    /// max 20 with L0 50 is 32 (the minimum wins); the strip is the
    /// radius wide but never wider than it is long; age 0 draws nothing.
    #[test]
    fn a_trail_is_at_least_its_minimum_and_no_wider_than_long_10_7() {
        let v = Vec3::new(137.0, 0.0, 0.0);
        assert!((trail_length(v, 0.1, 1.0, 32.0, 512.0, 0.0).unwrap() - 32.0).abs() < 1e-4);
        assert!((trail_length(Vec3::X * 500.0, 0.1, 1.0, 32.0, 20.0, 0.0).unwrap() - 32.0).abs() < 1e-4);
        assert_eq!(trail_length(v, 0.1, 0.0, 32.0, 512.0, 0.2), None, "age 0 with a fade-in");
        assert_eq!(trail_length(v, 0.0, 1.0, 32.0, 512.0, 0.0), None, "no trail length");
        let width = |radius: f32| {
            let (mut defs, _) = one(5.0, radius, vec![Op::Movement { gravity: Vec3::ZERO, drag: 0.0 }]);
            defs[0].initializers.push(Init::VelocityNoise { cp: 0, min: Vec3::X * 137.0, max: Vec3::X * 137.0, local: false });
            defs[0].initializers.push(Init::TrailLength(Pick::new(0.1, 0.1, 1.0)));
            defs[0].renderer = Render::Trail { min_length: 32.0, max_length: 512.0, length_fade_in: 0.0 };
            let mut i = Instance::new(&defs, 0);
            created(&mut i, &defs, &[ControlPoint::default()]);
            step(&defs, &mut i, 1.0 / 60.0);
            let mut out = Vec::new();
            i.draw(&defs, &|v| v, 1.0, Vec3::new(0.0, 0.0, 10_000.0), &none, &mut out);
            let Shape::Quad { corners, .. } = out[0].shape else { panic!() };
            (corners[0].distance(corners[1]), corners[0].distance(corners[3]))
        };
        let (long, wide) = width(22.0);
        assert!((long - 32.0).abs() < 1e-3 && (wide - 22.0).abs() < 1e-3, "{long} x {wide}");
        let (long, wide) = width(50.0);
        assert!((long - 32.0).abs() < 1e-3 && (wide - 32.0).abs() < 1e-3, "{long} x {wide}");
    }

    /// particles.md 10.8: the render alpha byte; 0 is not drawn, 1/255 is.
    #[test]
    fn a_particle_whose_alpha_byte_is_zero_is_not_drawn_10_8() {
        let drawn = |alpha: f32| {
            let (mut defs, _) = one(5.0, 5.0, vec![]);
            defs[0].initializers.push(Init::Alpha(Pick::new(alpha, alpha, 1.0)));
            let mut i = Instance::new(&defs, 0);
            created(&mut i, &defs, &[ControlPoint::default()]);
            step(&defs, &mut i, 1.0 / 60.0);
            let mut out = Vec::new();
            i.draw(&defs, &|v| v, 1.0, Vec3::new(0.0, 0.0, 10_000.0), &none, &mut out);
            out.first().map(|q| q.alpha)
        };
        assert_eq!(drawn(0.0015), None);
        assert_eq!(drawn(0.0025), Some(1.0 / 255.0));
    }

    /// particles.md 10.9: the wave (W(0.5) = 1, W(1.25) = -0.75), and the
    /// field's rate of change: on alpha with R 1.2 at x 0.5 a 1/60 step
    /// adds 0.02, clamped to 1 (0.99 -> 1); radius isn't clamped.
    #[test]
    fn oscillate_scalar_adds_rate_times_the_wave_10_9() {
        assert!((wave(0.5) - 1.0).abs() < 1e-6);
        assert!((wave(1.25) + 0.75).abs() < 1e-6);
        assert!((wave(-0.5) + 1.0).abs() < 1e-6 && (wave(2.5) - 1.0).abs() < 1e-6 && wave(0.0) == 0.0);
        let osc = |field: u8, rate: f32| Op::Oscillate {
            field,
            vector: false,
            rate: (Vec3::splat(rate), Vec3::splat(rate)),
            frequency: (Vec3::ONE, Vec3::ONE),
            proportional: false,
            start: (0.0, 0.0),
            end: (1.0, 1.0),
            start_end_proportional: true,
            // x = F (M T + phase) = 0.5 at any time.
            multiplier: 0.0,
            phase: 0.5,
        };
        let run = |op: Op, alpha: f32| {
            let (mut defs, _) = one(5.0, 5.0, vec![op]);
            defs[0].initializers.push(Init::Alpha(Pick::new(alpha, alpha, 1.0)));
            let mut i = Instance::new(&defs, 0);
            created(&mut i, &defs, &[ControlPoint::default()]);
            step(&defs, &mut i, 1.0 / 60.0);
            (i.particles[0].alpha, i.particles[0].radius)
        };
        assert_eq!(run(osc(7, 1.2), 0.99).0, 1.0);
        assert!((run(osc(7, 1.2), 0.5).0 - 0.52).abs() < 1e-5);
        assert!((run(osc(3, -600.0), 1.0).1 + 5.0).abs() < 1e-3, "not clamped");
    }

    /// particles.md 10.10: skipped to 0.04, a burst at 0 is made in the
    /// next step with creation time 0 (aged 0.04 + h), a 0.01 s
    /// continuous emitter makes nothing, and no initial particles; a
    /// burst more than 2 s before the skip is cancelled.
    #[test]
    fn a_skipped_system_fires_its_burst_late_and_its_short_emitter_never_10_10() {
        let d = SystemDef {
            initial_particles: 1,
            emitters: vec![
                Emitter::Instantaneously { start: 0.0, count: 4 },
                Emitter::Continuously { start: 0.0, rate: 3000.0, duration: 0.01 },
            ],
            initializers: vec![Init::Lifetime(Pick::new(1.0, 1.0, 1.0))],
            ..system()
        };
        let defs = vec![d];
        let mut i = Instance::new(&defs, 0);
        i.skip_to(&defs, 0.04);
        let h = 1.0 / 60.0;
        step(&defs, &mut i, h);
        assert_eq!(i.count(), 4, "the burst only");
        assert!(i.particles.iter().all(|p| (p.age - (0.04 + h)).abs() < 1e-5), "{}", i.particles[0].age);
        for _ in 0..10 {
            step(&defs, &mut i, h);
        }
        assert_eq!(i.count(), 4);
        let mut late = Instance::new(&defs, 0);
        late.skip_to(&defs, 2.5);
        step(&defs, &mut late, h);
        assert_eq!(late.count(), 0, "a burst 2.5 s back is cancelled");
        // Not skipped: the initial particle in the creation frame.
        let mut i = Instance::new(&defs, 0);
        created(&mut i, &defs, &[ControlPoint::default()]);
        assert_eq!(i.count(), 1);
        // A late info_particle_system's switch skips it (over 0.01 s).
        let placed = PlacedSystem { entity: None, position: Vec3::ZERO, rotation: Quat::IDENTITY, system: 0, start_on: true };
        let mut e = SystemEmitter::new(placed, 1);
        e.switch(&defs, true, 0.015);
        assert!(e.instances[0].age > 0.01 && e.instances[0].started);
    }

    /// `Explosion_Flashup` as its file has it (particles.md 10.1).
    fn flashup() -> SystemDef {
        SystemDef {
            name: "Explosion_Flashup".into(),
            material: Some(0),
            max_particles: 45,
            emitters: vec![Emitter::Continuously { start: 0.0, rate: 3000.0, duration: 0.01 }],
            initializers: vec![
                Init::Rotation { initial: 0.0, offset: Pick::new(0.0, 360.0, 1.0) },
                Init::Lifetime(Pick::new(0.0, 0.2, 1.0)),
                Init::Radius(Pick::new(1.0, 5.0, 1.0)),
                Init::Color { a: Vec3::new(191.0, 54.0, 0.0) / 255.0, b: Vec3::new(255.0, 42.0, 0.0) / 255.0, tint: 0.0, cp: 0 },
                Init::Sphere {
                    cp: 0,
                    distance: Pick::new(0.0, 0.0, 1.0),
                    bias: Vec3::ONE,
                    bias_abs: Vec3::ZERO,
                    speed: Pick::new(0.0, 0.0, 1.0),
                    local_min: Vec3::new(110.0, -840.0, -840.0),
                    local_max: Vec3::new(920.0, 840.0, 840.0),
                },
                Init::Alpha(Pick::new(1.0, 1.0, 1.0)),
                Init::TrailLength(Pick::new(0.3, 0.4, 1.0)),
            ],
            operators: vec![
                Op::RadiusScale { start_scale: 3.0, end_scale: 11.0, start: 0.0, end: 1.0, bias: 0.9, ease: false },
                Op::Movement { gravity: Vec3::ZERO, drag: 0.1 },
                Op::ColorFade { color: Vec3::new(135.0, 51.0, 0.0) / 255.0, start: 0.0, end: 0.7, ease: true },
                Op::SpinRoll { rate: 212.0 },
                Op::FadeAndDecay { start_alpha: 0.0, end_alpha: 0.0, fade_in: (0.1, 0.2), fade_out: (0.7, 1.0) },
            ],
            renderer: Render::Trail { min_length: 32.0, max_length: 512.0, length_fade_in: 0.2 },
            ..default()
        }
    }

    /// particles.md 10.1, in a blast (no skip): the first emitting step
    /// of 1/60 makes 30; those whose life is over 10 x their age are at
    /// full alpha (the fade-in hasn't started); every strip is 32 long
    /// (the minimum) and min(32, r0 (3 + 8 g(t))) wide. Started by an
    /// info_particle_system 0.04 s late it never makes one.
    #[test]
    fn the_flashup_draws_in_a_blast_and_nothing_when_skipped_10_1() {
        let defs = vec![flashup()];
        let cps = [ControlPoint::default()];
        let mut rng = ParticleRng::new(11);
        let mut i = Instance::new(&defs, 0);
        created(&mut i, &defs, &cps);
        i.step(&defs, &cps, 1.0 / 60.0, &mut rng, &none);
        assert_eq!(i.serial, 30, "30 made");
        let full = i.particles.iter().filter(|p| p.life > 10.0 * p.age).count();
        assert!((5..=20).contains(&full), "{full} at full alpha");
        for p in &i.particles {
            let a = Instance::alpha(&defs[0], p);
            if p.life > 10.0 * p.age {
                assert_eq!(a, 1.0, "life {} age {}", p.life, p.age);
            }
            let t = p.age / p.life;
            let r = p.radius0 * (3.0 + 8.0 * rational_bias(t, 0.9));
            assert!((Instance::radius(&defs[0], p) - r).abs() < 1e-3);
        }
        let mut out = Vec::new();
        i.draw(&defs, &|v| v, 1.0, Vec3::new(0.0, 0.0, 10_000.0), &none, &mut out);
        assert!(out.len() >= 5, "{} drawn", out.len());
        for q in &out {
            let Shape::Quad { corners, .. } = q.shape else { panic!() };
            let long = corners[0].distance(corners[1]);
            assert!((long - 32.0).abs() < 0.05 && corners[0].distance(corners[3]) <= 32.0 + 1e-3, "{long}");
        }
        let mut skipped = Instance::new(&defs, 0);
        skipped.skip_to(&defs, 0.04);
        for _ in 0..30 {
            skipped.step(&defs, &cps, 1.0 / 60.0, &mut rng, &none);
            assert_eq!(skipped.count(), 0);
        }
    }

    /// `tint_perc` blends the random colour that far toward the light at
    /// its control point, in gamma space; 0 leaves it alone.
    #[test]
    fn color_random_tints_toward_the_control_points_light() {
        let mut d = system();
        d.emitters = vec![Emitter::Instantaneously { start: 0.0, count: 4 }];
        d.initializers = vec![
            Init::Lifetime(Pick::new(1.0, 1.0, 1.0)),
            Init::Color {
                a: Vec3::Z,
                b: Vec3::Z,
                tint: 0.5,
                cp: 1,
            },
        ];
        let plain = SystemDef {
            initializers: vec![
                Init::Lifetime(Pick::new(1.0, 1.0, 1.0)),
                Init::Color {
                    a: Vec3::Z,
                    b: Vec3::Z,
                    tint: 0.0,
                    cp: 1,
                },
            ],
            ..d.clone()
        };
        let defs = vec![d, plain];
        let lit = ControlPoint {
            light: Vec3::new(1.0, 0.25, 0.0),
            ..default()
        };
        // Control point 1 carries the light; 0 is plain white.
        let cps = [ControlPoint::default(), lit];
        let mut rng = ParticleRng::new(4);
        let gamma = 0.25f32.powf(1.0 / 2.2);
        let want = Vec3::new(0.5, 0.5 * gamma, 0.5);
        let mut i = Instance::new(&defs, 0);
        created(&mut i, &defs, &cps);
        i.step(&defs, &cps, 0.05, &mut rng, &none);
        assert_eq!(i.count(), 4);
        for p in &i.particles {
            assert!(p.color.abs_diff_eq(want, 1e-5), "{}", p.color);
        }
        let mut i = Instance::new(&defs, 1);
        created(&mut i, &defs, &cps);
        i.step(&defs, &cps, 0.05, &mut rng, &none);
        assert!(i.particles.iter().all(|p| p.color == Vec3::Z));
        // Light past 1 counts as 1.
        assert_eq!(tint_toward_light(Vec3::ZERO, Vec3::splat(4.0), 0.1), Vec3::splat(0.1));
        // The point light: the probe's six directions averaged.
        let mut cube = [Vec3::ZERO; 6];
        cube[0] = Vec3::splat(0.6);
        let probe = crate::map::LightProbe {
            cube,
            lights: Vec::new(),
        };
        assert!(point_light(&probe).abs_diff_eq(Vec3::splat(0.1), 1e-6));
    }

    /// A started system with no emitters of its own, only a child at 0
    /// (TF2's blasts: `ExplosionCore_Wall`'s flash, embers, ...): its
    /// zero-time creation frame (particles.md 9.1) leaves it with nothing
    /// yet, and it must stay to start the child the next frame.
    #[test]
    fn a_started_system_of_children_only_survives_its_creation_frame_9_1() {
        let parent = SystemDef {
            name: "blast".into(),
            children: vec![(1, 0.0)],
            ..default()
        };
        let mut child = system();
        child.name = "flash".into();
        child.emitters = vec![Emitter::Instantaneously { start: 0.0, count: 5 }];
        let defs = std::sync::Arc::new(MapParticleSystems { defs: vec![parent, child], placed: Vec::new(), scale: 0.0254 });
        let mut s = StartedSystems::default();
        assert!(s.start(&defs, "blast", Vec3::ZERO, Vec3::Z, Vec3::ONE));
        s.step(1.0 / 60.0, &none);
        assert_eq!(s.running.len(), 1, "kept after its creation frame");
        s.step(1.0 / 60.0, &none);
        assert_eq!(s.count(), 5, "the child's burst the frame after");
    }

    #[test]
    fn children_start_after_their_delay() {
        let mut parent = system();
        parent.children = vec![(1, 0.3)];
        let child = system();
        let defs = vec![parent, child];
        let mut i = Instance::new(&defs, 0);
        let mut rng = ParticleRng::new(3);
        let cps = [ControlPoint::default()];
        created(&mut i, &defs, &cps);
        i.step(&defs, &cps, 0.2, &mut rng, &none);
        assert!(i.children[0].1.is_none());
        i.step(&defs, &cps, 0.15, &mut rng, &none);
        assert!(i.children[0].1.is_some());
    }

    #[test]
    fn started_systems_run_to_their_end_and_unknown_names_show_nothing() {
        let defs = std::sync::Arc::new(MapParticleSystems {
            defs: vec![system()],
            placed: Vec::new(),
            scale: 0.0254,
        });
        let mut started = StartedSystems::default();
        assert!(!started.start(&defs, "nothing", Vec3::ZERO, Vec3::Z, Vec3::ONE));
        assert!(started.start(&defs, "TEST", Vec3::ZERO, Vec3::Z, Vec3::ONE));
        // Pointing up: the frame's forward axis is +Z.
        let cp = &started.running[0].cp;
        assert!((cp.rotation * Vec3::X).abs_diff_eq(Vec3::Z, 1e-5));
        started.step(0.0, &none);
        started.step(0.1, &none);
        assert!(started.count() > 0);
        for _ in 0..20 {
            started.step(0.1, &none);
        }
        assert!(started.running.is_empty(), "finished and gone");
        // New definitions (another map) drop what ran on the old ones.
        started.start(&defs, "test", Vec3::ZERO, Vec3::Z, Vec3::ONE);
        let other = std::sync::Arc::new((*defs).clone());
        started.start(&other, "test", Vec3::ZERO, Vec3::Z, Vec3::ONE);
        assert_eq!(started.running.len(), 1);
    }

    /// A start with `emit_for` stops emitting then (TF2's flames, one
    /// piece per round): the 0.5 s emitter cut at 0.1 s makes fewer.
    #[test]
    fn started_system_emits_for_its_limit() {
        let defs = std::sync::Arc::new(MapParticleSystems {
            defs: vec![system()],
            placed: Vec::new(),
            scale: 0.0254,
        });
        let run = |emit_for| {
            let mut s = StartedSystems::default();
            assert!(s.start_for(&defs, "test", Vec3::ZERO, Vec3::Z, Vec3::ONE, emit_for));
            let mut made = 0;
            s.step(0.0, &none);
            for _ in 0..4 {
                s.step(0.05, &none);
                made = made.max(s.count());
            }
            (made, s.running[0].instance.emitting)
        };
        let (cut, emitting) = run(Some(0.1));
        let (full, _) = run(None);
        assert!(!emitting, "stopped past its limit");
        assert!(cut < full, "{cut} < {full}");
    }

    /// A tracer (specs/tf2/effects.md 6): "move particles between 2
    /// control points" sends its one particle from control point 0 to the
    /// end (`set_last`) at the operator's maximum speed; it dies there.
    #[test]
    fn a_tracer_travels_to_its_end_at_its_speed_and_dies_there() {
        let tracer = SystemDef {
            name: "tracer".into(),
            material: Some(0),
            emitters: vec![Emitter::Instantaneously { start: 0.0, count: 1 }],
            initializers: vec![Init::MoveBetween { end: 1, speed: 6000.0, start_offset: 0.0 }],
            operators: vec![Op::Movement { gravity: Vec3::ZERO, drag: 0.0 }, Op::Decay],
            renderer: Render::Trail { min_length: 0.0, max_length: 500.0, length_fade_in: 0.1 },
            ..default()
        };
        let defs = std::sync::Arc::new(MapParticleSystems { defs: vec![tracer], placed: Vec::new(), scale: 0.0254 });
        let mut s = StartedSystems::default();
        assert!(s.start(&defs, "tracer", Vec3::ZERO, Vec3::X, Vec3::ONE));
        s.set_last(Some(Vec3::new(1200.0, 0.0, 0.0)), None);
        s.step(0.0, &none);
        s.step(0.1, &none);
        let p = &s.running[0].instance.particles[0];
        assert!((p.velocity - Vec3::new(6000.0, 0.0, 0.0)).length() < 1e-2, "{:?}", p.velocity);
        assert!((p.life - 0.2).abs() < 1e-5, "1200 units at 6000/s: {}", p.life);
        assert!((p.position.x - 600.0).abs() < 1.0, "{:?}", p.position);
        s.step(0.15, &none);
        assert!(s.running.is_empty(), "arrived and gone");
    }

    /// A followed control point with an `up` takes the whole turn (its x
    /// the forward, its z the up, in game space); without, the shortest
    /// turn to the forward.
    #[test]
    fn a_followed_point_turns_with_its_up() {
        let f = crate::map::psys::Follow { entity: Entity::PLACEHOLDER, local: Vec3::ZERO, forward: Vec3::NEG_Z, carry: false, up: Some(Vec3::X) };
        let q = follow_turn(&f, &Transform::IDENTITY);
        assert!((q * Vec3::X).abs_diff_eq(Vec3::Y, 1e-5), "{}", q * Vec3::X);
        assert!((q * Vec3::Z).abs_diff_eq(Vec3::X, 1e-5), "{}", q * Vec3::Z);
        let q = follow_turn(&crate::map::psys::Follow { up: None, ..f }, &Transform::IDENTITY);
        assert!((q * Vec3::X).abs_diff_eq(Vec3::Y, 1e-5));
    }

    /// A sprite trail of a system no movement operator moves follows the
    /// particle's own motion, not its start velocity nor its locked-to
    /// point's: TF2's Flame Thrower pilot (locked to the muzzle, a forward
    /// speed of 800-1200, trail length 0.1 s) draws nothing while the gun
    /// is still (real TF2: no streak; ours drew one 100 units back past the
    /// eye), and nothing while the point moves (real TF2's rocket: its
    /// locked `rockettrail_burst` draws no streaks at 1100 units/s; ours
    /// drew them back to the eye).
    #[test]
    fn a_trail_no_movement_operator_moves_follows_its_motion() {
        let pilot = SystemDef {
            name: "pilot".into(),
            material: Some(0),
            radius: 7.0,
            emitters: vec![Emitter::Instantaneously { start: 0.0, count: 1 }],
            initializers: vec![
                Init::Sphere {
                    cp: 0,
                    distance: Pick::new(0.0, 0.0, 1.0),
                    bias: Vec3::ONE,
                    bias_abs: Vec3::ZERO,
                    speed: Pick::new(0.0, 0.0, 1.0),
                    local_min: Vec3::new(1000.0, 0.0, 0.0),
                    local_max: Vec3::new(1000.0, 0.0, 0.0),
                },
                Init::Lifetime(Pick::new(10.0, 10.0, 1.0)),
            ],
            operators: vec![Op::Decay, Op::LockToControlPoint { cp: 0 }],
            renderer: Render::Trail { min_length: 0.0, max_length: 2000.0, length_fade_in: 0.0 },
            ..default()
        };
        let defs = vec![pilot];
        let mut i = Instance::new(&defs, 0);
        let mut rng = ParticleRng::new(3);
        let eye = Vec3::new(0.0, 0.0, 10_000.0);
        let still = [ControlPoint::default()];
        created(&mut i, &defs, &still);
        i.step(&defs, &still, 0.02, &mut rng, &none);
        i.step(&defs, &still, 0.02, &mut rng, &none);
        let mut out = Vec::new();
        i.draw(&defs, &|v| v, 1.0, eye, &none, &mut out);
        assert!(out.is_empty(), "no streak while still: {} drawn", out.len());
        // The muzzle moves 2 units in 0.02 s: the particle goes with it,
        // and no streak.
        let moved = [ControlPoint { position: Vec3::new(0.0, 2.0, 0.0), ..default() }];
        i.step(&defs, &moved, 0.02, &mut rng, &none);
        let mut out = Vec::new();
        i.draw(&defs, &|v| v, 1.0, eye, &none, &mut out);
        assert!(out.is_empty(), "no streak by the point's motion: {} drawn", out.len());
        assert!((i.particles[0].position.y - 2.0).abs() < 1e-4, "carried: {:?}", i.particles[0].position);
    }

    /// A sprite trail's strip (specs/tf2/particles.md 5, measured on real
    /// TF2's tracers): as wide as the radius (2 units for radius 2), the
    /// texture's v = 1 (`effects/tracer1`'s bright end) at the head and
    /// v = 0 at the tail.
    #[test]
    fn a_trail_strip_is_its_radius_wide_and_its_texture_bright_end_leads() {
        let tracer = SystemDef {
            name: "tracer".into(),
            material: Some(0),
            radius: 2.0,
            emitters: vec![Emitter::Instantaneously { start: 0.0, count: 1 }],
            initializers: vec![Init::MoveBetween { end: 1, speed: 6000.0, start_offset: 0.0 }],
            operators: vec![Op::Movement { gravity: Vec3::ZERO, drag: 0.0 }, Op::Decay],
            renderer: Render::Trail { min_length: 0.0, max_length: 500.0, length_fade_in: 0.1 },
            ..default()
        };
        let defs = vec![tracer];
        let mut i = Instance::new(&defs, 0);
        let mut rng = ParticleRng::new(3);
        let cps = [ControlPoint::default(), ControlPoint { position: Vec3::new(3000.0, 0.0, 0.0), ..default() }];
        created(&mut i, &defs, &cps);
        i.step(&defs, &cps, 0.05, &mut rng, &none);
        let mut out = Vec::new();
        // Game units throughout; the eye far above, looking down.
        i.draw(&defs, &|v| v, 1.0, Vec3::new(0.0, 0.0, 10_000.0), &none, &mut out);
        let Shape::Quad { corners, .. } = out[0].shape else { panic!("{:?}", out[0].shape) };
        let head = i.particles[0].position;
        // corners: (u0, v1), (u0, v0), (u1, v0), (u1, v1).
        let v1_end = (corners[0] + corners[3]) * 0.5;
        let v0_end = (corners[1] + corners[2]) * 0.5;
        assert!(v1_end.distance(head) < 1e-3, "v = 1 at the head: {v1_end} vs {head}");
        assert!(v0_end.x < head.x - 100.0, "v = 0 back toward the gun: {v0_end}");
        assert!((corners[0].distance(corners[3]) - 2.0).abs() < 1e-3, "2 wide: {corners:?}");
        // The full width is the radius, but no wider than the strip is
        // long (particles.md 10.7 step 5).
        let mut wide = defs.clone();
        wide[0].radius = 400.0;
        let mut i = Instance::new(&wide, 0);
        created(&mut i, &wide, &cps);
        i.step(&wide, &cps, 0.05, &mut rng, &none);
        out.clear();
        i.draw(&wide, &|v| v, 1.0, Vec3::new(0.0, 0.0, 10_000.0), &none, &mut out);
        let Shape::Quad { corners, .. } = out[0].shape else { panic!() };
        let long = corners[0].distance(corners[1]);
        assert!(long > 100.0 && long < 400.0, "{long}");
        assert!((corners[0].distance(corners[3]) - long).abs() < 1e-2, "as wide as long: {corners:?}");
    }

    /// particles.md 5.1's test cases: length = this step's speed x the
    /// trail length x min(1, age / fade-in), then clamped.
    #[test]
    fn trail_lengths_follow_the_spec_cases() {
        let v = |s: f32| Vec3::new(s, 0.0, 0.0);
        let close = |a: Option<f32>, b: f32| a.is_some_and(|a| (a - b).abs() < 0.05);
        assert!(close(trail_length(v(1500.0), 0.08, 1.0 / 60.0, 0.0, 2000.0, 0.1), 20.0));
        assert!(close(trail_length(v(1500.0), 0.08, 0.2, 0.0, 2000.0, 0.1), 120.0));
        // `Explosion_CoreFlash`: clamped to its 122 after the fade-in.
        assert!(close(trail_length(v(1400.0), 0.4, 0.05, 2.0, 122.0, 0.2), 122.0));
        assert!(close(trail_length(v(1400.0), 0.4, 1.0 / 60.0, 2.0, 122.0, 0.2), 46.67));
        assert_eq!(trail_length(Vec3::ZERO, 0.1, 1.0, 2.0, 122.0, 0.0), None, "no motion, nothing");
    }

    /// particles.md 5.1: a particle locked to a still control point with
    /// no movement operator and a start speed of 1000 draws nothing; a
    /// moving one's strip is its radius after the radius operators wide
    /// (r0 0.8, x 3 -> 0 eased, bias 0.5, at life fraction 0.2: 2.15).
    #[test]
    fn a_still_trail_draws_nothing_and_its_width_is_the_scaled_radius() {
        let mut def = SystemDef {
            name: "embers".into(),
            material: Some(0),
            radius: 0.8,
            emitters: vec![Emitter::Instantaneously { start: 0.0, count: 1 }],
            initializers: vec![
                Init::Velocity { cp: 0, speed: Pick::new(0.0, 0.0, 1.0), local_min: Vec3::X * 1000.0, local_max: Vec3::X * 1000.0 },
                Init::Lifetime(Pick::new(1.0, 1.0, 1.0)),
            ],
            operators: vec![Op::Decay, Op::LockToControlPoint { cp: 0 }],
            renderer: Render::Trail { min_length: 0.0, max_length: 2000.0, length_fade_in: 0.1 },
            ..default()
        };
        let eye = Vec3::new(0.0, 0.0, 10_000.0);
        let mut rng = ParticleRng::new(5);
        let still = [ControlPoint::default()];
        let defs = vec![def.clone()];
        let mut i = Instance::new(&defs, 0);
        created(&mut i, &defs, &still);
        i.step(&defs, &still, 1.0 / 60.0, &mut rng, &none);
        i.step(&defs, &still, 1.0 / 60.0, &mut rng, &none);
        let mut out = Vec::new();
        i.draw(&defs, &|v| v, 1.0, eye, &none, &mut out);
        assert!(out.is_empty(), "{} drawn", out.len());
        def.operators = vec![
            Op::Decay,
            Op::Movement { gravity: Vec3::ZERO, drag: 0.0 },
            Op::RadiusScale { start_scale: 3.0, end_scale: 0.0, start: 0.0, end: 1.0, bias: 0.5, ease: true },
        ];
        let defs = vec![def];
        let mut i = Instance::new(&defs, 0);
        created(&mut i, &defs, &still);
        for _ in 0..12 {
            i.step(&defs, &still, 1.0 / 60.0, &mut rng, &none);
        }
        out.clear();
        i.draw(&defs, &|v| v, 1.0, eye, &none, &mut out);
        let Shape::Quad { corners, .. } = out[0].shape else { panic!() };
        assert!((corners[0].distance(corners[3]) - 2.15).abs() < 0.02, "{}", corners[0].distance(corners[3]));
    }

    /// particles.md 5: a system with two renderers draws each particle
    /// with both (`Explosion_FlyingEmbers`' trail and its head sprite).
    #[test]
    fn two_renderers_draw_each_particle_twice() {
        let def = SystemDef {
            name: "two".into(),
            material: Some(0),
            emitters: vec![Emitter::Instantaneously { start: 0.0, count: 3 }],
            initializers: vec![Init::Velocity { cp: 0, speed: Pick::new(500.0, 500.0, 1.0), local_min: Vec3::ZERO, local_max: Vec3::ZERO }],
            operators: vec![Op::Movement { gravity: Vec3::ZERO, drag: 0.0 }],
            renderer: Render::Trail { min_length: 0.0, max_length: 2000.0, length_fade_in: 0.0 },
            more_renderers: vec![Render::Sprites { rate: 1.0, as_fps: false, fit_life: false, orientation: 0 }],
            ..default()
        };
        let defs = vec![def];
        let mut i = Instance::new(&defs, 0);
        let mut rng = ParticleRng::new(5);
        created(&mut i, &defs, &[ControlPoint::default()]);
        i.step(&defs, &[ControlPoint::default()], 1.0 / 60.0, &mut rng, &none);
        let mut out = Vec::new();
        i.draw(&defs, &|v| v, 1.0, Vec3::new(0.0, 0.0, 10_000.0), &none, &mut out);
        assert_eq!(out.len(), 6);
        assert_eq!(out.iter().filter(|q| matches!(q.shape, Shape::Quad { .. })).count(), 3);
    }

    /// "Collision via traces" (`Op::Collide`): TF2's Minigun brass (bounce
    /// 0, slide 0, radius scale 1.5) lands on the floor and lies there; it
    /// fell through the bridge's floor before. With bounce 1 a particle
    /// comes back up; with `kill` it dies on the floor.
    #[test]
    fn particles_land_on_the_world_with_their_bounce_and_slide() {
        struct Floor;
        impl Sheets for Floor {
            fn frames(&self, _: usize, _: u32) -> usize {
                1
            }
            fn trace(&self, from: Vec3, to: Vec3) -> Option<(Vec3, Vec3)> {
                (from.z >= 0.0 && to.z < 0.0).then(|| (from + (to - from) * (from.z / (from.z - to.z)), Vec3::Z))
            }
        }
        let fall = |bounce: f32, kill: bool| {
            let mut brass = system();
            brass.radius = 2.0;
            brass.emitters = vec![Emitter::Instantaneously { start: 0.0, count: 1 }];
            brass.initializers = vec![Init::Lifetime(Pick::new(2.0, 2.0, 1.0))];
            brass.operators = vec![
                Op::Movement { gravity: Vec3::new(0.0, 0.0, -800.0), drag: 0.0 },
                Op::Collide { bounce, slide: 0.0, radius_scale: 1.5, kill },
                Op::Decay,
            ];
            let defs = vec![brass];
            let cps = [ControlPoint { position: Vec3::new(0.0, 0.0, 40.0), ..ControlPoint::default() }];
            let mut rng = ParticleRng::new(3);
            let mut i = Instance::new(&defs, 0);
            let mut lowest = f32::MAX;
            created(&mut i, &defs, &cps);
            for _ in 0..60 {
                i.step(&defs, &cps, 1.0 / 60.0, &mut rng, &Floor);
                if let Some(p) = i.particles.first() {
                    lowest = lowest.min(p.position.z);
                }
            }
            (i.particles.first().map(|p| (p.position, p.velocity)), lowest)
        };
        let (rest, lowest) = fall(0.0, false);
        let (at, velocity) = rest.expect("still there");
        assert!((at.z - 3.0).abs() < 0.2, "lies 1.5 x its radius up: {at}");
        assert!(lowest >= 2.9, "never under the floor: {lowest}");
        assert!(velocity.length() < 15.0, "at rest: {velocity}");
        let (bounced, _) = fall(1.0, false);
        assert!(bounced.is_some());
        let (gone, _) = fall(0.0, true);
        assert!(gone.is_none(), "killed on the floor");
    }

    /// "Lifetime from sequence" leaves the life alone on a material
    /// without a sheet (TF2's Minigun brass), and sets frames / fps with
    /// one.
    #[test]
    fn lifetime_from_sequence_needs_a_sheet() {
        struct NoSheet;
        impl Sheets for NoSheet {
            fn frames(&self, _: usize, _: u32) -> usize {
                1
            }
            fn sheet_frames(&self, _: usize, _: u32) -> Option<usize> {
                None
            }
        }
        let mut brass = system();
        brass.emitters = vec![Emitter::Instantaneously { start: 0.0, count: 1 }];
        brass.initializers = vec![Init::Lifetime(Pick::new(2.0, 2.0, 1.0)), Init::LifetimeFromSequence { fps: 30.0 }];
        let defs = vec![brass];
        let cps = [ControlPoint::default()];
        let mut rng = ParticleRng::new(3);
        let mut i = Instance::new(&defs, 0);
        created(&mut i, &defs, &cps);
        i.step(&defs, &cps, 0.01, &mut rng, &NoSheet);
        assert!((i.particles[0].life - 2.0).abs() < 1e-5, "{}", i.particles[0].life);
        let sixty = |_: usize, _: u32| 60usize;
        let mut i = Instance::new(&defs, 0);
        created(&mut i, &defs, &cps);
        i.step(&defs, &cps, 0.01, &mut rng, &sixty);
        assert!((i.particles[0].life - 2.0).abs() < 1e-5, "60 frames at 30 fps: {}", i.particles[0].life);
        let six = |_: usize, _: u32| 6usize;
        let mut i = Instance::new(&defs, 0);
        created(&mut i, &defs, &cps);
        i.step(&defs, &cps, 0.01, &mut rng, &six);
        assert!((i.particles[0].life - 0.2).abs() < 1e-5, "{}", i.particles[0].life);
    }

    /// A tagged start stops emitting on `stop_tagged` (TF2's Minigun's
    /// constant muzzle when it stops firing); others run on.
    #[test]
    fn tagged_systems_stop_emitting_and_others_go_on() {
        let mut forever = system();
        forever.emitters = vec![Emitter::Continuously { start: 0.0, rate: 100.0, duration: 0.0 }];
        let defs = std::sync::Arc::new(MapParticleSystems { defs: vec![forever], placed: Vec::new(), scale: 0.0254 });
        let mut s = StartedSystems::default();
        let gun = Entity::from_raw_u32(7).unwrap();
        s.start(&defs, "test", Vec3::ZERO, Vec3::X, Vec3::ONE);
        s.set_last(None, Some(gun));
        s.start(&defs, "test", Vec3::ZERO, Vec3::X, Vec3::ONE);
        s.step(0.0, &none);
        s.step(0.1, &none);
        s.stop_tagged(gun);
        assert!(!s.running[0].instance.emitting && s.running[1].instance.emitting);
        for _ in 0..10 {
            s.step(0.1, &none);
        }
        assert_eq!(s.running.len(), 1, "the stopped one's particles lived out");
    }

    #[test]
    fn emitter_switches_make_instances() {
        let defs = vec![system()];
        let placed = PlacedSystem {
            entity: None,
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            system: 0,
            start_on: false,
        };
        let mut e = SystemEmitter::new(placed, 1);
        e.switch(&defs, true, 0.0);
        e.switch(&defs, true, 0.0);
        assert_eq!(e.instances.len(), 1);
        e.switch(&defs, false, 0.0);
        assert!(!e.instances[0].emitting);
        // Started again: a new instance beside the fading one.
        e.switch(&defs, true, 0.0);
        assert_eq!(e.instances.len(), 2);
    }

    /// TF2's flame core (specs/tf2/flames.md 3.3): each sprite takes the
    /// active control point, lives its duration, trails it (catch-up
    /// 100), its radius 23 x (0.5 to 1 over 0..64 units from control
    /// point 0) x (0.4 to 1 over its life) and its tint (0.2, 0.2, 1) to
    /// white over 0..200 units.
    #[test]
    fn a_sprite_rides_its_control_point_with_the_flame_cores_remaps() {
        let core = SystemDef {
            name: "core".into(),
            material: Some(0),
            radius: 23.0,
            emitters: vec![Emitter::Continuously { start: 0.0, rate: 100.0, duration: 0.0 }],
            initializers: vec![Init::AssignTargetCp, Init::LifetimeFromCp],
            operators: vec![
                Op::Decay,
                Op::FollowCp { catch_up: 100.0 },
                Op::RemapDistance {
                    cp: 0,
                    field: Field::Radius,
                    near: 0.0,
                    far: 64.0,
                    low: Vec3::splat(0.5),
                    high: Vec3::splat(1.0),
                    scale_initial: true,
                },
                Op::RemapDistance {
                    cp: 0,
                    field: Field::Tint,
                    near: 0.0,
                    far: 200.0,
                    low: Vec3::new(0.2, 0.2, 1.0),
                    high: Vec3::ONE,
                    scale_initial: false,
                },
                Op::RadiusScale { start_scale: 0.4, end_scale: 1.0, start: 0.0, end: 1.0, bias: 0.5, ease: false },
            ],
            renderer: Render::Sprites { rate: 1.0, as_fps: false, fit_life: true, orientation: 0 },
            ..SystemDef::default()
        };
        let defs = vec![core];
        let mut rng = ParticleRng::new(3);
        let mut i = Instance::new(&defs, 0);
        let mut cps = [ControlPoint::default(); MAX_CONTROL_POINTS];
        cps[3] = ControlPoint { position: Vec3::new(120.0, 0.0, 0.0), duration: 0.6, ..ControlPoint::default() };
        // One sprite (0.01 s at 100/s), on control point 3.
        created(&mut i, &defs, &cps);
        i.step_with(&defs, &cps, Some(3), 0.01, &mut rng, &none);
        // Born at the muzzle (control point 0), it heads for its point at
        // 100 units a second: at most a unit on in 0.01 s.
        let (at, cp) = i.placed()[0];
        assert_eq!(cp, Some(3));
        assert!(at.y == 0.0 && at.z == 0.0 && (0.0..=1.0 + 1e-4).contains(&at.x), "{at}");
        let p = &i.particles[0];
        assert!((p.life - 0.6).abs() < 1e-6);
        // Half its life on, 120 from the muzzle: 23 x 1 x 0.7; tint 0.68.
        let mut q = p.clone();
        q.age = 0.3;
        // What the radius scale writes at half its life (0.4 + 0.6 x 0.5).
        q.radius = q.radius0 * 0.7;
        q.position = Vec3::new(120.0, 0.0, 0.0);
        Instance::remap(&defs[0], &|n| cps[n], &mut q);
        assert!((Instance::radius(&defs[0], &q) - 16.1).abs() < 1e-3, "{}", Instance::radius(&defs[0], &q));
        let c = Instance::color(&defs[0], &q);
        assert!((c - Vec3::new(0.68, 0.68, 1.0)).length() < 1e-4, "{c}");
        // At the muzzle at birth: 23 x 0.5 x 0.4, blue.
        let mut at_muzzle = Instance::new(&defs, 0);
        cps[3].position = Vec3::ZERO;
        created(&mut at_muzzle, &defs, &cps);
        at_muzzle.step_with(&defs, &cps, Some(3), 0.01, &mut rng, &none);
        let mut q = at_muzzle.particles[0].clone();
        q.age = 0.0;
        q.radius = q.radius0 * 0.4;
        assert!((Instance::radius(&defs[0], &q) - 4.6).abs() < 1e-3);
        assert!((Instance::color(&defs[0], &q) - Vec3::new(0.2, 0.2, 1.0)).length() < 1e-5);
        // The point moves: the sprite moves with it (its velocity) and
        // closes the gap at 100 units a second, so it trails it (TF2's
        // stream: sprites along the whole way from the muzzle).
        let mut trailing = Instance::new(&defs, 0);
        cps[3] = ControlPoint { position: Vec3::new(40.0, 0.0, 0.0), velocity: Vec3::new(1000.0, 0.0, 0.0), duration: 0.6, ..ControlPoint::default() };
        created(&mut trailing, &defs, &cps);
        trailing.step_with(&defs, &cps, Some(3), 0.01, &mut rng, &none);
        let start = trailing.particles[0].position.x;
        for _ in 0..20 {
            cps[3].position += cps[3].velocity * 0.01;
            trailing.step_with(&defs, &cps, None, 0.01, &mut rng, &none);
        }
        let x = trailing.particles[0].position.x;
        // 0.2 s at 1000 units a second, plus 20 units of the gap closed.
        assert!((x - (start + 200.0 + 20.0)).abs() < 1e-2, "{x} from {start}");
        assert!(x < cps[3].position.x, "it trails its point");
        // Close enough, it is on it.
        cps[3].velocity = Vec3::ZERO;
        cps[3].position = Vec3::new(x + 0.5, 0.0, 0.0);
        trailing.step_with(&defs, &cps, None, 0.01, &mut rng, &none);
        assert_eq!(trailing.particles[0].position, cps[3].position);
        // Without an active point a new sprite has none and stays put.
        let mut lone = Instance::new(&defs, 0);
        created(&mut lone, &defs, &cps);
        lone.step_with(&defs, &cps, None, 0.01, &mut rng, &none);
        assert_eq!(lone.placed(), vec![(Vec3::ZERO, None)]);
    }
}
