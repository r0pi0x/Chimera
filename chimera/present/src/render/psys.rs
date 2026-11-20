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
//! specs/tf2/effects.md 14.1).
//!
//! Known shortcuts (docs/tech-debt.md): Particle systems (`render::psys`,
//! `map::psys`, `source_engine::content::pcf`): the operator meanings are
//! interpretations (specs/cs_source/impact_effects.md 14, fire.md 7.3; the
//! library isn't public): drag removes its fraction per step (steps of the
//! system's maximum time step), velocity noise is uniform per particle, random
//! force a uniform draw per step, twist a tangential acceleration, a sprite
//! trail's length is clamped after its fade-in by age (the order inferred,
//! particles.md 5.1), the client's particle multiplier is 1 and an
//! emitter's stop and restart (particles.md 9.2) aren't run (a stopped
//! system never restarts), animation rate without "as FPS" plays the sequence that many
//! times a second; "collision via traces" is a ray per step (its radius past
//! the step) against the solid world for started systems only (placed
//! ones don't collide), bounce and slide read as fractions of the speed
//! into and along the surface (its trace tolerances and collision group
//! aren't read); "Oscillate Scalar" runs a placeholder formula (radius
//! and alpha only: `Op::Oscillate`, specs/tf2/effects.md Open questions
//! 17); "Position on Model Random" and "Movement Lock to Bone" need the
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
    /// Its first two oscillators' drawn rate and frequency
    /// (`Op::Oscillate`).
    osc: [(f32, f32); 2],
}

/// "Movement Basic"'s drag is the fraction of the velocity taken off per
/// simulation step in Source, so its look changes with the frame rate (a
/// rocket's explosion captured in TF2 at 250 and at 62.5 frames a second
/// differed: tools/tf2ref, docs/plans/active/tf2-parity.md). Ours takes it
/// off per this many steps a second, as at 60 frames a second, at any step
/// size (a slowed or fast frame no longer stops the particles dead).
pub const DRAG_STEPS: f32 = 60.0;

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

/// `motion` (its last step's, units/s) x `trail` (s) x min(1, `age` /
/// `fade_in`) (1 with no fade-in), clamped to [`min`, `max`]; None when it
/// didn't move.
pub fn trail_length(motion: Vec3, trail: f32, age: f32, min: f32, max: f32, fade_in: f32) -> Option<f32> {
    let speed = motion.length();
    if speed <= 1e-4 {
        return None;
    }
    let f = if fade_in > 0.0 { (age / fade_in).clamp(0.0, 1.0) } else { 1.0 };
    Some((speed * trail * f).clamp(min, max.max(min)))
}

fn smooth(x: f32) -> f32 {
    x * x * (3.0 - 2.0 * x)
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
            && d.emitters.iter().any(|e| match *e {
                Emitter::Continuously { start, duration, .. } => duration <= 0.0 || self.age < start + duration,
                Emitter::Instantaneously { start, .. } => self.age <= start,
            });
        let children_left = self.children.iter().any(|(delay, c, _)| match c {
            Some(c) => !c.finished(defs),
            None => self.emitting && self.age < *delay,
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
                match *e {
                    Emitter::Continuously { start, rate, duration } => {
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
                        self.carry[i] += rate.max(0.0) * (b - a);
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
                        // step's start (the point's previous place).
                        let due = if start <= 0.0 {
                            before == 0.0
                        } else {
                            before < start && t >= start
                        };
                        if due {
                            made.extend(std::iter::repeat_n(before.max(start), count as usize));
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
        let mut lived: Vec<f32> = Vec::with_capacity(from.len());
        for p in &mut self.particles {
            // A particle made during this step lives only its part of it.
            let dt = dt + p.age.min(0.0);
            lived.push(dt);
            p.age = p.age.max(0.0) + dt;
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
                p.velocity += (gravity + accel) * dt;
                if drag > 0.0 {
                    // Per 1/60 s, whatever the step (`DRAG_STEPS`).
                    p.velocity *= (1.0 - drag).max(0.0).powf(dt * DRAG_STEPS);
                }
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
            let mut osc = 0;
            for o in &d.operators {
                match *o {
                    Op::RotationBasic => p.roll += p.roll_speed * dt,
                    Op::SpinRoll { rate } => p.roll += rate * dt,
                    Op::Oscillate { field, multiplier, phase, start, end, .. } => {
                        // A placeholder (effects.md Open questions 17).
                        let (rate, frequency) = p.osc.get(osc).copied().unwrap_or_default();
                        osc += 1;
                        let x = if p.life > 0.0 { p.age / p.life } else { 1.0 };
                        if x < start || x > end {
                            continue;
                        }
                        let change = rate * (std::f32::consts::TAU * (frequency * multiplier * p.age + phase)).sin() * dt;
                        match field {
                            3 => p.radius = (p.radius + change).max(0.0),
                            7 => p.alpha = (p.alpha + change / 255.0).clamp(0.0, 1.0),
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
        // Its motion over the part of the step it lived (a sprite
        // trail's direction and speed: particles.md 5.1).
        for ((p, from), lived) in self.particles.iter_mut().zip(from).zip(lived) {
            if lived > 1e-6 {
                p.motion = (p.position - from) / lived;
            }
        }
        if decays {
            self.particles.retain(|p| p.age < p.life);
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
            osc: [(0.0, 0.0); 2],
        };
        for (k, o) in d.operators.iter().filter(|o| matches!(o, Op::Oscillate { .. })).take(2).enumerate() {
            if let Op::Oscillate { rate, frequency, .. } = *o {
                p.osc[k] = (rng.float(rate.0, rate.1), rng.float(frequency.0, frequency.1));
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
            if radius <= 0.0 || alpha <= 0.001 {
                continue;
            }
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
                    // (particles.md 5.1: a flame pilot locked to a still
                    // muzzle shows no streak).
                    let Some(length) = trail_length(p.motion, p.trail, p.age, min_length, max_length, length_fade_in) else {
                        continue;
                    };
                    let back = to_engine(p.position - p.motion.normalize() * length);
                    // The radius is the strip's full width (5.1 step 3);
                    // the texture's v runs from the tail (0) to the head
                    // (1): TF2's `effects/tracer1` is bright at the
                    // bullet (Open questions 4).
                    let side = (at - back).cross(at - eye).normalize_or_zero() * radius * scale * 0.5;
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

    fn radius(d: &SystemDef, p: &P) -> f32 {
        let t = if p.life > 0.0 { p.age / p.life } else { 1.0 };
        let mut r = p.radius * p.radius_mul;
        for o in &d.operators {
            if let Op::RadiusScale {
                start_scale,
                end_scale,
                start,
                end,
                bias,
                ease,
            } = *o
            {
                let mut x = window(t, start, end);
                if ease {
                    x = smooth(x);
                }
                r *= start_scale + (end_scale - start_scale) * crate::map::particles::bias(x, bias);
            }
        }
        r
    }

    fn alpha(d: &SystemDef, p: &P) -> f32 {
        let t = if p.life > 0.0 { p.age / p.life } else { 1.0 };
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
                Op::FadeAndDecay {
                    start_alpha,
                    end_alpha,
                    fade_in,
                    fade_out,
                } => {
                    let fin = start_alpha + (1.0 - start_alpha) * window(t, fade_in.0, fade_in.1);
                    let fout = 1.0 + (end_alpha - 1.0) * window(t, fade_out.0, fade_out.1);
                    a *= fin * fout;
                }
                _ => {}
            }
        }
        a.clamp(0.0, 1.0)
    }

    fn color(d: &SystemDef, p: &P) -> Vec3 {
        let t = if p.life > 0.0 { p.age / p.life } else { 1.0 };
        let mut c = p.tint.unwrap_or(p.color);
        for o in &d.operators {
            if let Op::ColorFade {
                color,
                start,
                end,
                ease,
            } = *o
            {
                let mut x = window(t, start, end);
                if ease {
                    x = smooth(x);
                }
                c = c.lerp(color, x);
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

    /// Turned on: a new instance; turned off: every instance stops
    /// emitting (2.5).
    pub fn switch(&mut self, defs: &[SystemDef], on: bool) {
        if on && !self.was_on {
            self.instances.push(Instance::new(defs, self.placed.system));
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
    mut pool: ResMut<Particles>,
) {
    let Some(defs) = defs else { return };
    let defs = &defs.0;
    let dt = time.delta_secs().min(0.25);
    let eye = eyes.iter().next().map(|e| e.translation());
    let sheets = MaterialSheets(materials.as_ref().map(|m| &m.0));
    let scale = defs.scale.max(1e-6);
    // Engine space to game space and back (Source: Z up).
    let to_game = |v: Vec3| Vec3::new(v.x, -v.z, v.y) / scale;
    let to_engine = |v: Vec3| Vec3::new(v.x, v.z, -v.y) * scale;
    for (mut e, at, part, visibility) in &mut emitters {
        let exists = part.is_none_or(|p| p.exists);
        let on = exists && part.map_or(e.placed.start_on, |p| p.on);
        e.switch(&defs.defs, on);
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
    /// 1/60 s leaves 0.9^60 of the speed after one second.
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
        let want = 100.0 * 0.9f32.powi(60);
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

    #[test]
    fn fades_and_radius_scale() {
        let d = SystemDef {
            operators: vec![
                Op::FadeAndDecay {
                    start_alpha: 0.0,
                    end_alpha: 0.0,
                    fade_in: (0.0, 0.25),
                    fade_out: (0.5, 1.0),
                },
                Op::RadiusScale {
                    start_scale: 2.0,
                    end_scale: 20.0,
                    start: 0.0,
                    end: 1.0,
                    bias: 0.5,
                    ease: false,
                },
                Op::ColorFade {
                    color: Vec3::ZERO,
                    start: 0.0,
                    end: 1.0,
                    ease: false,
                },
            ],
            ..system()
        };
        let p = |age: f32| P {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            age,
            life: 1.0,
            radius: 5.0,
            color: Vec3::ONE,
            alpha: 1.0,
            roll: 0.0,
            roll_speed: 0.0,
            sequence: 0,
            flip: false,
            fade_in: 0.0,
            fade_out: 0.0,
            serial: 0,
            target: None,
            radius_mul: 1.0,
            alpha_mul: 1.0,
            tint: None,
            trail: DEFAULT_TRAIL_LENGTH,
            motion: Vec3::ZERO,
            bone: None,
            osc: [(0.0, 0.0); 2],
        };
        assert_eq!(Instance::alpha(&d, &p(0.0)), 0.0);
        assert!((Instance::alpha(&d, &p(0.125)) - 0.5).abs() < 1e-5);
        assert_eq!(Instance::alpha(&d, &p(0.4)), 1.0);
        assert!((Instance::alpha(&d, &p(0.75)) - 0.5).abs() < 1e-5);
        assert!((Instance::radius(&d, &p(0.5)) - 55.0).abs() < 1e-3);
        assert!((Instance::color(&d, &p(0.25)) - Vec3::splat(0.75)).length() < 1e-5);
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
        // The full width is the radius however short the strip
        // (particles.md 5.1 step 3: no cap by the length).
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
        assert!((corners[0].distance(corners[3]) - 400.0).abs() < 1e-2, "400 wide: {corners:?}");
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
        e.switch(&defs, true);
        e.switch(&defs, true);
        assert_eq!(e.instances.len(), 1);
        e.switch(&defs, false);
        assert!(!e.instances[0].emitting);
        // Started again: a new instance beside the fading one.
        e.switch(&defs, true);
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
