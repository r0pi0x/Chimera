//! The rules of specs/combat_arms/effects_sounds.md as plain functions and
//! small state (section numbers are the spec's), in the game's units
//! (world units, seconds) so the systems in `effects` and the tests share
//! them: sound gain, low-pass and radii (10), visibility gates (2), the
//! tracer cadence, motion and strip shape (4), the decal list (6), casings (8), the
//! fly-by test, footstep cadences, the server's hurt limit and the
//! hit-confirm queue (11), the camera jitter's falloff (9) and sound
//! slot draws.

use std::collections::HashMap;

use bevy::prelude::*;

pub use crate::map::sound::{low_pass_fraction, radius_gain};

// Constants (the spec's "Constants").

/// A point is seen when camera forward · direction to it ≥ this.
pub const VIEW_CONE_DOT: f32 = 0.6;
/// Beyond (u): no casing; own decals only while zoomed.
pub const FAR_FIRE_DIST: f32 = 1000.0;
pub const FAR_IMPACT_DIST: f32 = 1000.0;
/// Other players' impacts this close (u) count as seen behind you.
pub const NEAR_OTHER_IMPACT: f32 = 300.0;
/// No tracer for a shot this short or shorter (u).
pub const TRACER_MIN_DIST: f32 = 100.0;
/// `MarkSolidTime`, `MarkFadeTime` (s).
pub const MARK_SOLID_TIME: f32 = 3.0;
pub const MARK_FADE_TIME: f32 = 3.0;
/// The client's decal list (100 + 1).
pub const MARK_MAX: usize = 101;
/// The crowding rule: radius (u) and the most decals inside it.
pub const MARK_REGION: f32 = 10.0;
pub const MARK_REGION_MAX: usize = 10;
pub const TRACER_LIST_MAX: usize = 50;
pub const SHELL_LIST_MAX: usize = 600;
/// Casings: life (s), bounces, growth time (s) and factor.
pub const SHELL_LIFE: f32 = 3.0;
pub const SHELL_BOUNCES: u32 = 3;
pub const SHELL_GROW_TIME: f32 = 0.5;
pub const SHELL_GROW_FACTOR: f32 = 2.0;
/// Client object gravity (u/s², applied at half strength) and in water.
pub const SHELL_GRAVITY: f32 = -1000.0;
pub const SHELL_WATER_GRAVITY: f32 = -500.0;
/// Speed kept per bounce (water: 0.25); the casing lies this far off the
/// surface (u).
pub const SHELL_BOUNCE_DAMP: f32 = 0.5;
pub const SHELL_WATER_DAMP: f32 = 0.25;
pub const SHELL_SURFACE_GAP: f32 = 0.3;
/// A floor: normal y above this.
pub const SHELL_FLOOR_Y: f32 = 0.6;
/// The casing's start speeds (u/s): up, right, forward; spin (rad/s).
pub const SHELL_UP: (f32, f32) = (30.0, 75.0);
pub const SHELL_RIGHT: (f32, f32) = (50.0, 75.0);
pub const SHELL_FORWARD: (f32, f32) = (20.0, 50.0);
pub const SHELL_SPIN: (f32, f32) = (2.0, 10.0);
/// `FlyByRadius` (u) and the impact guard (× the impact's sound radius).
pub const FLYBY_RADIUS: f32 = 600.0;
pub const FLYBY_IMPACT_GUARD: f32 = 0.75;
/// Inner radius as a share of the outer when not given.
pub const INNER_RADIUS_FRACTION: f32 = 0.07;
/// The distance low-pass ramp (u).
pub const LOWPASS_START: f32 = 1200.0;
pub const LOWPASS_SPAN: f32 = 600.0;
/// Mixer voices and live sounds.
pub const VOICE_MAX: usize = 255;
/// `FootStepSoundRadius` (u).
pub const FOOTSTEP_RADIUS: f32 = 1200.0;
/// Pending hit-confirm sounds.
pub const DING_SLOTS: usize = 5;
/// Server sounds go to clients within this many outer radii.
pub const SERVER_SOUND_SEND: f32 = 2.0;
/// The release build's special-effects detail value (none of 0, 1, 2:
/// Quirk 9).
pub const WEAPON_FX_DETAIL: u8 = 3;
/// The effect file's detail level the player picks (0 low, 1 medium, 2
/// high); ours defaults to high.
pub const DEFAULT_FX_DETAIL: u8 = 2;
/// The muzzle flash's group length (s; `AK47_PV_Muzz`).
pub const FLASH_TIME: f32 = 0.05;
/// Own steps: the view sway's paces (cycles/s) and the "near the ground"
/// height (u) a step still counts within.
pub const STEP_PACE_RUN: f32 = 1.4;
pub const STEP_PACE_WALK: f32 = 0.6;
pub const STEP_GROUND_REACH: f32 = 150.0;

// ---------------------------------------------------------------------------
// 10. How a sound is played

/// The inner radius a caller gives none for: 7 % of the outer.
pub fn inner_radius(outer: f32) -> f32 {
    INNER_RADIUS_FRACTION * outer
}

/// g(d) with volume V (0..100): the squared-distance falloff between r and
/// R (section 10), times V/100.
pub fn gain(d: f32, r: f32, big_r: f32, volume: f32) -> f32 {
    radius_gain(r, big_r, d) * volume / 100.0
}

/// Where g reaches 0: sqrt(r² + (R − r)²).
pub fn zero_point(r: f32, big_r: f32) -> f32 {
    (r * r + (big_r - r) * (big_r - r)).sqrt()
}

/// The distance low-pass's cut-off (a fraction of the bandwidth) at d,
/// only for r < d < R (else 1).
pub fn low_pass(d: f32, r: f32, big_r: f32) -> f32 {
    if d <= r || d >= big_r {
        return 1.0;
    }
    low_pass_fraction(d, LOWPASS_START, LOWPASS_SPAN)
}

/// A server sound's radii as clients get them: R truncated to whole units,
/// r quantised to floor(255 r / R) / 255 of R.
pub fn server_radii(r: f32, big_r: f32) -> (f32, f32) {
    let big = big_r.trunc();
    if big <= 0.0 {
        return (0.0, big);
    }
    ((255.0 * r / big).floor() / 255.0 * big, big)
}

/// Whether a server sound of outer radius R reaches a client whose view
/// is `d` away (within 2R).
pub fn server_sends(d: f32, big_r: f32) -> bool {
    d <= SERVER_SOUND_SEND * big_r
}

/// The level Chimera's playback uses for a 3D sound of outer radius R
/// (u, inner 7 %), with the distance low-pass; `unit` metres per unit.
pub fn sound_level(big_r: f32, unit: f32) -> crate::map::SoundLevel {
    sound_level_radii(inner_radius(big_r), big_r, unit)
}

pub fn sound_level_radii(r: f32, big_r: f32, unit: f32) -> crate::map::SoundLevel {
    crate::map::SoundLevel::Radius {
        inner: r * unit,
        outer: big_r * unit,
        low_pass: Some((LOWPASS_START * unit, LOWPASS_SPAN * unit)),
    }
}

/// One of a sound list's slots, uniform among `slots` (4 for impacts,
/// 2 for footsteps) by `roll` in 0..1: an undefined or empty slot plays
/// nothing (Quirk 16).
pub fn draw_slot(list: &[String], slots: usize, roll: f32) -> Option<&str> {
    let i = ((roll * slots as f32) as usize).min(slots.saturating_sub(1));
    list.get(i).map(String::as_str).filter(|s| !s.is_empty())
}

/// One of a list, uniform (every entry defined).
pub fn draw_any(list: &[String], roll: f32) -> Option<&str> {
    if list.is_empty() {
        return None;
    }
    draw_slot(list, list.len(), roll)
}

// ---------------------------------------------------------------------------
// 2. Visibility gates

/// What one weapon effect may show, from the camera at that moment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Gates {
    pub fire_seen: bool,
    pub fire_far: bool,
    pub impact_seen: bool,
    pub impact_far: bool,
}

/// Whether `p` is inside the view cone of a camera at `cam` looking along
/// `forward` (unit); a point closer than 0.001 u counts as straight up.
pub fn seen(cam: Vec3, forward: Vec3, p: Vec3) -> bool {
    let to = p - cam;
    let dir = if to.length() < 0.001 { Vec3::Y } else { to.normalize() };
    forward.dot(dir) >= VIEW_CONE_DOT
}

/// A position as the game sends it in a fire-only weapon event
/// (special_weapons.md Corrections (2026-10-31) 1): each axis quantised
/// to a grid over the world's bounding box `(lo, hi)`, 2^19 cells on x
/// and z, 2^18 on y, and received as the cell's centre. Any one unit.
pub fn compress_position(p: Vec3, (lo, hi): (Vec3, Vec3)) -> Vec3 {
    let cells = Vec3::new(524_288.0, 262_144.0, 524_288.0);
    let size = (hi - lo).max(Vec3::splat(1e-6));
    let cell = ((p - lo) / size * cells).floor().clamp(Vec3::ZERO, cells - 1.0);
    lo + (cell + 0.5) / cells * size
}

/// The world origin clamped into the world's box: the impact position of
/// an event that never set one (a launcher's fire-only event).
pub fn unset_impact(world: Option<(Vec3, Vec3)>) -> Vec3 {
    world.map_or(Vec3::ZERO, |(lo, hi)| Vec3::ZERO.clamp(lo, hi))
}

/// The gates of section 2 (units). `others`: another player's shot (its
/// impact within 300 u counts as seen).
pub fn gates(cam: Vec3, forward: Vec3, fire: Vec3, impact: Vec3, others: bool) -> Gates {
    let impact_d = impact.distance(cam);
    Gates {
        fire_seen: seen(cam, forward, fire),
        fire_far: fire.distance(cam) > FAR_FIRE_DIST,
        impact_seen: seen(cam, forward, impact) || (others && impact_d <= NEAR_OTHER_IMPACT),
        impact_far: impact_d > FAR_IMPACT_DIST,
    }
}

// ---------------------------------------------------------------------------
// 4. Tracers

/// A weapon's tracer counter (starts at 0 when the weapon is made).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TracerCounter(pub u32);

impl TracerCounter {
    /// One ray at frequency F: c ← (c + 1) mod F (0 when F = 0); a tracer
    /// when the new c is 0.
    pub fn ray(&mut self, f: u32) -> bool {
        self.0 = if f == 0 { 0 } else { (self.0 + 1) % f };
        self.0 == 0
    }
}

/// A tracer in flight (units, seconds).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TracerPath {
    pub from: Vec3,
    pub dir: Vec3,
    /// Fire-to-impact distance D.
    pub distance: f32,
    pub velocity: f32,
    pub length: f32,
}

impl TracerPath {
    /// A tracer for a shot from `from` to `to` (None: D ≤ 100 u).
    pub fn new(from: Vec3, to: Vec3, velocity: f32, length: f32) -> Option<Self> {
        let d = to - from;
        let distance = d.length();
        (distance > TRACER_MIN_DIST && velocity > 0.0).then(|| Self {
            from,
            dir: d / distance,
            distance,
            velocity,
            length,
        })
    }

    /// Its life: D / v.
    pub fn life(&self) -> f32 {
        self.distance / self.velocity
    }

    /// Tail and head distances from the muzzle at time t (head clamped to
    /// the impact); None once its life is over.
    pub fn ends(&self, t: f32) -> Option<(f32, f32)> {
        if t > self.life() {
            return None;
        }
        let tail = (self.velocity * t).min(self.distance);
        Some((tail, (tail + self.length).min(self.distance)))
    }
}

/// A tracer's width across in world units: `Width`/2 (half-width
/// `Width`/4 at each end; Corrections (2026-10-30) 3). No floor, cap or
/// zoom compensation.
pub fn tracer_across(width: f32) -> f32 {
    width / 2.0
}

/// A tracer strip's width direction in camera space (x right, y up; depth
/// ignored) for its two ends `a`, `b` in camera space: perpendicular to
/// their screen-plane difference Δ, n = (Δy, −Δx, 0)/|Δ|, the same for
/// both ends (Corrections (2026-10-30) 3). None when the strip points
/// straight at the eye (Δ = 0: not seen).
pub fn tracer_side(a: Vec3, b: Vec3) -> Option<Vec3> {
    let d = Vec2::new(b.x - a.x, b.y - a.y);
    let len = d.length();
    (len > 1e-6).then(|| Vec3::new(d.y, -d.x, 0.0) / len)
}

/// A tracer's four corners (world) from tail `a` to head `b` (world) as
/// seen by a camera at `camera` (its world transform: camera space has
/// x right, y up): each end ± (w/2)·n with w = `tracer_across(width)` and
/// n of `tracer_side`, mapped back to the world. Order: tail +n, head +n,
/// head −n, tail −n. None: seen end-on.
pub fn tracer_corners(a: Vec3, b: Vec3, camera: &Transform, width: f32) -> Option<[Vec3; 4]> {
    let inv = camera.compute_affine().inverse();
    let n = tracer_side(inv.transform_point3(a), inv.transform_point3(b))?;
    let side = camera.rotation * n * (tracer_across(width) / 2.0);
    Some([a + side, b + side, b - side, a - side])
}

// ---------------------------------------------------------------------------
// 6. Decals

/// One bullet hole (units).
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub at: Vec3,
    pub normal: Vec3,
    /// Spin about the normal (radians).
    pub angle: f32,
    /// Width (u): 64 · scale.
    pub width: f32,
    /// The sprite (its texture path) and grey colour.
    pub sprite: String,
    pub grey: f32,
    pub age: f32,
    /// Added order (for "oldest").
    pub serial: u64,
}

impl Mark {
    /// α(t): 1 while solid, then min(α, (T_s + T_f − t) / T_s); None once
    /// gone.
    pub fn alpha(&self) -> Option<f32> {
        mark_alpha(self.age)
    }
}

/// α(t) of section 6 (the divisor is the solid time).
pub fn mark_alpha(t: f32) -> Option<f32> {
    if t > MARK_SOLID_TIME + MARK_FADE_TIME {
        None
    } else if t <= MARK_SOLID_TIME {
        Some(1.0)
    } else {
        Some(((MARK_SOLID_TIME + MARK_FADE_TIME - t) / MARK_SOLID_TIME).min(1.0))
    }
}

/// What adding a decal did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkAdded {
    Added,
    /// The crowding rule moved this one (index) to the new point.
    Moved(usize),
    /// The cap replaced the oldest.
    Replaced,
}

/// The client's decal list.
#[derive(Resource, Clone, Debug, Default)]
pub struct CaMarks {
    pub marks: Vec<Mark>,
    serial: u64,
}

impl CaMarks {
    /// Add a decal: crowding first (more than 10 within 10 u: the closest
    /// seen so far moves there, keeping its age, spin and size), then the
    /// cap (the one in the list longest goes).
    pub fn add(&mut self, mut mark: Mark) -> MarkAdded {
        let mut count = 0;
        let mut closest: Option<(usize, f32)> = None;
        for (i, m) in self.marks.iter().enumerate() {
            let d2 = m.at.distance_squared(mark.at);
            if d2 < MARK_REGION * MARK_REGION {
                count += 1;
                if closest.is_none_or(|(_, c)| d2 < c) {
                    closest = Some((i, d2));
                }
                if count > MARK_REGION_MAX {
                    let (j, _) = closest.unwrap();
                    self.marks[j].at = mark.at;
                    self.marks[j].normal = mark.normal;
                    return MarkAdded::Moved(j);
                }
            }
        }
        self.serial += 1;
        mark.serial = self.serial;
        if self.marks.len() >= MARK_MAX {
            let oldest = (0..self.marks.len()).min_by_key(|&i| self.marks[i].serial).unwrap();
            self.marks[oldest] = mark;
            return MarkAdded::Replaced;
        }
        self.marks.push(mark);
        MarkAdded::Added
    }

    /// Age them by the frame time (not while paused) and drop the gone.
    pub fn step(&mut self, dt: f32) {
        for m in &mut self.marks {
            m.age += dt;
        }
        self.marks.retain(|m| m.alpha().is_some());
    }
}

/// A decal's width (u) for ImpactFX `MarkScale` and a draw `u` in 0..1 of
/// the surface's scale range: 64 · MarkScale · U(min, max).
pub fn mark_width(mark_scale: f32, range: [f32; 2], u: f32) -> f32 {
    64.0 * mark_scale * (range[0] + (range[1] - range[0]) * u)
}

// ---------------------------------------------------------------------------
// 7. Blood

/// How many blobs splat: each blob's `SplatPercent` (0..100) against its
/// roll.
pub fn splats(percents: &[f32], rolls: &[f32]) -> usize {
    percents.iter().zip(rolls).filter(|(p, r)| **r * 100.0 < **p).count()
}

// ---------------------------------------------------------------------------
// 8. Shell casings

/// A casing's start velocity in its frame's axes (up, right, forward
/// draws in 0..1; `left`: a left-hand dual weapon throws to the left),
/// u/s, as (right, up, forward) components.
pub fn casing_velocity(rolls: [f32; 3], left: bool) -> Vec3 {
    let lerp = |(a, b): (f32, f32), u: f32| a + (b - a) * u;
    let side = if left { -1.0 } else { 1.0 };
    Vec3::new(
        side * lerp(SHELL_RIGHT, rolls[1]),
        lerp(SHELL_UP, rolls[0]),
        lerp(SHELL_FORWARD, rolls[2]),
    )
}

/// One casing (units, the world's axes: +Y up).
#[derive(Clone, Debug, PartialEq)]
pub struct Casing {
    pub at: Vec3,
    pub velocity: Vec3,
    /// Spin about up and right (rad/s) and the angles so far.
    pub spin: Vec2,
    pub angles: Vec2,
    /// Its turn so far (the engine compounds it: Corrections 2026-10-22,
    /// 11).
    pub orientation: Quat,
    pub bounces: u32,
    pub age: f32,
    pub resting: bool,
    pub in_water: bool,
    /// Rides with the first-person shooter (Quirk 15).
    pub first_person: bool,
}

/// What a casing's frame did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CasingStep {
    Flying,
    /// Its first bounce (the only one with a sound), on this surface.
    FirstBounce,
    Bounced,
    Rested,
    /// Gone: its life ended, or it hit sky or an invisible surface.
    Removed,
}

/// What lies between a casing's old and new position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CasingHit {
    pub at: Vec3,
    pub normal: Vec3,
    /// Sky or an invisible surface: the casing goes.
    pub removes: bool,
}

impl Casing {
    /// Its scale factor at its age: grows linearly to 2× over 0.5 s.
    pub fn scale(&self) -> f32 {
        1.0 + (SHELL_GROW_FACTOR - 1.0) * (self.age / SHELL_GROW_TIME).min(1.0)
    }

    /// One frame of Δ (section 8): x += v Δ, v += ½ g Δ; a hit between the
    /// old and new position (`trace`) bounces it 0.3 u off the surface,
    /// mirrored and damped, pitch spin × 0.75 and yaw spin × −0.75; it
    /// rests after 3 bounces, or at once on a floor when |v|² < 225/Δ.
    /// `carry`: the first-person shooter's displacement this frame.
    pub fn step(&mut self, dt: f32, carry: Vec3, trace: impl Fn(Vec3, Vec3) -> Option<CasingHit>) -> CasingStep {
        self.age += dt;
        if self.age >= SHELL_LIFE {
            return CasingStep::Removed;
        }
        if self.resting || dt <= 0.0 {
            return CasingStep::Flying;
        }
        let g = if self.in_water { SHELL_WATER_GRAVITY } else { SHELL_GRAVITY };
        let from = self.at;
        let mut to = from + self.velocity * dt;
        if self.first_person {
            to += carry;
        }
        self.velocity.y += 0.5 * g * dt;
        self.angles += self.spin * dt;
        // Each frame the current orientation is turned about its own up
        // axis by the yaw angle accumulated so far, then about its right
        // axis by the accumulated pitch: the turn per frame grows with age.
        self.orientation = self.orientation * Quat::from_rotation_y(self.angles.x) * Quat::from_rotation_x(self.angles.y);
        let Some(hit) = trace(from, to) else {
            self.at = to;
            return CasingStep::Flying;
        };
        if hit.removes {
            return CasingStep::Removed;
        }
        let n = hit.normal.normalize_or(Vec3::Y);
        self.at = hit.at + n * SHELL_SURFACE_GAP;
        // The rest test looks at the speed it arrives with (spec case 26).
        let floor_rest = n.y > SHELL_FLOOR_Y && rests(self.velocity, dt);
        let damp = if self.in_water { SHELL_WATER_DAMP } else { SHELL_BOUNCE_DAMP };
        self.velocity = (self.velocity - 2.0 * self.velocity.dot(n) * n) * damp;
        self.spin = Vec2::new(self.spin.x * -0.75, self.spin.y * 0.75);
        self.bounces += 1;
        let first = self.bounces == 1;
        if self.bounces >= SHELL_BOUNCES || floor_rest {
            self.resting = true;
            self.velocity = Vec3::ZERO;
            // On coming to rest it turns by the accumulated yaw once more.
            self.orientation *= Quat::from_rotation_y(self.angles.x);
            return if first { CasingStep::FirstBounce } else { CasingStep::Rested };
        }
        if first { CasingStep::FirstBounce } else { CasingStep::Bounced }
    }
}

/// The floor rest test: |v|² < 225/Δ.
pub fn rests(v: Vec3, dt: f32) -> bool {
    v.length_squared() < 225.0 / dt
}

// ---------------------------------------------------------------------------
// 11. Fly-by

/// Whether another player's bullet shot from `from` toward `to` flies by
/// a camera at `cam`: the camera not within 0.75 × the impact's sound
/// radius of the impact, and its distance to the shot's infinite line
/// under 600 u. Also the nearest point of the line (where we play it).
pub fn flyby(cam: Vec3, from: Vec3, to: Vec3, impact_radius: f32) -> Option<Vec3> {
    if cam.distance(to) <= FLYBY_IMPACT_GUARD * impact_radius {
        return None;
    }
    let dir = (to - from).normalize_or_zero();
    if dir == Vec3::ZERO {
        return None;
    }
    let nearest = from + dir * (cam - from).dot(dir);
    (nearest.distance(cam) < FLYBY_RADIUS).then_some(nearest)
}

/// The original's fly-by position (Quirk 6): the camera position
/// normalised to length 1 plus the camera-to-line vector.
pub fn flyby_quirk_position(cam: Vec3, nearest: Vec3) -> Vec3 {
    cam.normalize_or_zero() + (nearest - cam)
}

// ---------------------------------------------------------------------------
// 11. Footsteps

/// Which foot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Foot {
    Right,
    Left,
}

/// How long the view bob takes to ramp up once you move and to decay once
/// you stop (s).
pub const STEP_RAMP: f32 = 0.1;

/// Your own steps (first person; Corrections 2026-10-22, 5): the sway
/// phase φ advances by 2π·1.4·Δ with the run flag (2π·0.6 without) every
/// frame the view bob's amplitude is non-zero, and is reset to 0 on any
/// frame it is 0. The amplitude ramps up over 0.1 s once a movement key is
/// held (it is still 0 for the first two frames, so φ starts moving on the
/// third) and decays over 0.1 s once none is, so φ keeps running for 0.1 s
/// after you stop (the foot crossings of the decay use up that foot
/// without a sound). A right step sounds when φ enters (π/2, 3π/2], a left
/// one when it passes 3π/2; pressing a movement key (from rest), jumping
/// and landing each add one at once. Silent without the run flag.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct OwnSteps {
    pub phase: f32,
    pub was_moving: bool,
    pub next: Option<Foot>,
    /// The bob's amplitude as the seconds of ramp left (0..`STEP_RAMP`).
    pub ramp: f32,
    /// Frames of a start so far with the amplitude still 0.
    pub lag: u8,
}

/// One frame of your own movement for `OwnSteps`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StepInputs {
    pub dt: f32,
    /// A movement key is held (the control flag: no displacement needed,
    /// pushing into a wall counts).
    pub moving: bool,
    /// On the ground, on a ladder, or within 150 u above ground and not
    /// jumping.
    pub can_step: bool,
    /// The run flag (off walking or crouched).
    pub run: bool,
    pub jumped: bool,
    pub landed: bool,
    pub alive: bool,
}

impl OwnSteps {
    /// The steps this frame (feet in order).
    pub fn step(&mut self, i: &StepInputs) -> Vec<Foot> {
        use std::f32::consts::{FRAC_PI_2, PI, TAU};
        let mut out = Vec::new();
        let moving = i.moving && i.can_step && i.alive;
        let audible = i.run && i.alive;
        let foot = |me: &mut Self| {
            let f = me.next.unwrap_or(Foot::Right);
            me.next = Some(if f == Foot::Right { Foot::Left } else { Foot::Right });
            f
        };
        if moving && !self.was_moving && audible {
            let f = foot(self);
            out.push(f);
        }
        if (i.jumped || i.landed) && audible {
            let f = foot(self);
            out.push(f);
        }
        if self.ramp > 0.0 {
            let pace = TAU * if i.run { STEP_PACE_RUN } else { STEP_PACE_WALK };
            let before = self.phase;
            let after = before + pace * i.dt;
            // Crossings of π/2 (entering (π/2, 3π/2]) and 3π/2, over
            // possibly several turns.
            let mut k = (before / TAU).floor();
            while k * TAU <= after {
                for (mark, f) in [(FRAC_PI_2, Foot::Right), (1.5 * PI, Foot::Left)] {
                    let at = k * TAU + mark;
                    if before <= at && at < after {
                        // The decay's crossings use the foot up unheard.
                        if audible && moving {
                            out.push(f);
                        }
                        self.next = Some(if f == Foot::Right { Foot::Left } else { Foot::Right });
                    }
                }
                k += 1.0;
            }
            self.phase = after % TAU;
        } else {
            self.phase = 0.0;
        }
        // The amplitude for the next frame.
        if moving {
            if self.ramp > 0.0 {
                self.ramp = (self.ramp + i.dt).min(STEP_RAMP);
            } else {
                self.lag += 1;
                if self.lag >= 2 {
                    self.ramp = i.dt.min(STEP_RAMP);
                }
            }
        } else {
            self.ramp = (self.ramp - i.dt).max(0.0);
            if self.ramp <= 0.0 {
                self.lag = 0;
            }
        }
        self.was_moving = moving;
        out
    }
}

/// Others' footsteps: the stance their animation plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gait {
    Run,
    Sprint,
    Walk,
    Crouch,
    Jump,
    Still,
}

/// The footstep keys of a third-person animation (ms into a loop of
/// `loop_ms`): running two per 600 ms (50 and 350), sprinting two per
/// 500 ms, a jump one at 600 ms (the landing); walking and crouching
/// none (only footprint keys).
pub fn footstep_keys(g: Gait) -> (u32, &'static [u32]) {
    match g {
        Gait::Run => (600, &[50, 350]),
        // PLACEHOLDER times: the spec gives two per 500 ms loop, not when.
        Gait::Sprint => (500, &[50, 300]),
        Gait::Jump => (600, &[600]),
        Gait::Walk | Gait::Crouch | Gait::Still => (600, &[]),
    }
}

/// The footstep keys passed when an animation of gait `g` advances from
/// `t0` to `t1` seconds of play (at its rate already applied): each key
/// k crossed (t0 < k ≤ t1 in loop time).
pub fn footsteps_between(g: Gait, t0: f32, t1: f32) -> usize {
    let (length, keys) = footstep_keys(g);
    if keys.is_empty() || t1 <= t0 {
        return 0;
    }
    let l = length as f32 / 1000.0;
    let mut n = 0;
    let mut k = (t0 / l).floor();
    while k * l <= t1 {
        for &key in keys {
            let at = k * l + key as f32 / 1000.0;
            if t0 < at && at <= t1 {
                n += 1;
            }
        }
        k += 1.0;
    }
    n
}

// ---------------------------------------------------------------------------
// 11. Hurt and death (server), hit confirm

/// A hurt sound's kind: by damage type and by hit zone (each limited to
/// one at a time across the whole server).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HurtKind {
    Guns,
    Explosions,
    Burning,
    Gas,
    Falling,
    Head,
    Groin,
}

/// The server's hurt sounds playing, by kind (until when).
#[derive(Resource, Clone, Debug, Default)]
pub struct HurtLimiter(pub HashMap<HurtKind, f64>);

impl HurtLimiter {
    /// Start a sound of `kind` lasting `length` s at `now` when none of
    /// its kind still plays.
    pub fn try_start(&mut self, kind: HurtKind, now: f64, length: f64) -> bool {
        if self.0.get(&kind).is_some_and(|&end| now < end) {
            return false;
        }
        self.0.insert(kind, now + length);
        true
    }
}

/// Pending hit-confirm dings (at most 5; more in one frame are dropped).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DingQueue(pub usize);

impl DingQueue {
    pub fn push(&mut self) -> bool {
        if self.0 >= DING_SLOTS {
            return false;
        }
        self.0 += 1;
        true
    }

    /// The dings to play now (on the next character update: Quirk 11).
    pub fn drain(&mut self) -> usize {
        std::mem::take(&mut self.0)
    }
}

// ---------------------------------------------------------------------------
// 9. Camera jitter

/// The jitter's falloff at distance d: 1 within `inner`, 0 beyond
/// `outer`, linear in squared distance between.
pub fn jitter_falloff(d: f32, inner: f32, outer: f32) -> f32 {
    if d <= inner {
        1.0
    } else if d >= outer {
        0.0
    } else {
        1.0 - (d * d - inner * inner) / (outer * outer - inner * inner)
    }
}

/// Particle friction f per second: velocity × (1 − f)^Δ.
pub fn friction_factor(f: f32, dt: f32) -> f32 {
    (1.0 - f).max(0.0).powf(dt)
}

// ---------------------------------------------------------------------------
// The follow-up answers (Open questions 17-25): the effect file's details,
// sounds while they play, call-outs and map sounds, as rules.

/// A key curve's value (Open question 17, "The effect file"): keys of
/// (u, value) on the key's normalised time, linear between keys and held
/// after the last; a looping key starts over every `length` s.
pub fn key_curve(keys: &[(f32, f32)], t: f32, length: f32, looping: bool, default: f32) -> f32 {
    if length <= 0.0 {
        return lithtech_formats::fxf::sample_keys(keys, 0.0, default);
    }
    let t = if looping { t.rem_euclid(length) } else { t.min(length) };
    lithtech_formats::fxf::sample_keys(keys, t / length, default)
}

/// A cone emitter's start point (`Type` 3), in the effect's frame (+Z the
/// axis n): uniform in the cone of height `rmax` whose radius grows from 0
/// at the apex to `rmin` at that height (`u`: three draws in [0, 1)).
pub fn cone_start(rmin: f32, rmax: f32, u: [f32; 3]) -> Vec3 {
    let h = rmax * u[0].cbrt();
    let r = rmin * (h / rmax.max(1e-6)) * u[1].sqrt();
    let a = u[2] * std::f32::consts::TAU;
    Vec3::new(r * a.cos(), r * a.sin(), h)
}

/// One step of a particle against its bounce plane at height `plane`: a
/// marked particle whose step would cross it keeps its position and
/// bounces (its velocity reflected, 75 % kept); an unmarked one passes.
pub fn bounce_step(pos: Vec3, vel: Vec3, dt: f32, plane: f32, marked: bool) -> (Vec3, Vec3) {
    let next = pos + vel * dt;
    if marked && pos.y >= plane && next.y < plane {
        (pos, Vec3::new(vel.x, -vel.y, vel.z) * BOUNCE_KEEP)
    } else {
        (next, vel)
    }
}

/// What a bounce keeps of the speed.
pub const BOUNCE_KEEP: f32 = 0.75;

/// Where an effect key is (Open question 17, placement modes): attached to
/// a socket (mode 4), it follows the socket, its offset along the socket's
/// axes; fixed (mode 0), it stays at its creation point, only its offset
/// turning with the parent.
pub fn place_key(mode: u8, socket: Transform, created_at: Vec3, offset: Vec3, parent_turn: Quat) -> Vec3 {
    match mode {
        4 => socket.translation + socket.rotation * offset,
        _ => created_at + parent_turn * offset,
    }
}

/// A playing 3D sound's gain and low-pass (Open question 21): recomputed
/// each frame the listener or the source moved, only for r < d < R; the
/// last values kept outside that band.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayingSound {
    pub r: f32,
    pub big_r: f32,
    pub volume: f32,
    pub gain: f32,
    pub cutoff: f32,
}

impl PlayingSound {
    /// A sound started at distance `d`.
    pub fn start(d: f32, r: f32, big_r: f32, volume: f32) -> Self {
        let mut s = Self {
            r,
            big_r,
            volume,
            gain: volume / 100.0,
            cutoff: 1.0,
        };
        s.update(d);
        s
    }

    /// The listener or the source moved: at distance `d` now.
    pub fn update(&mut self, d: f32) {
        if d > self.r && d < self.big_r {
            self.gain = gain(d, self.r, self.big_r, self.volume);
            self.cutoff = low_pass(d, self.r, self.big_r);
        }
    }
}

/// How many sounds may live at once; a new one beyond is refused, no
/// playing one stopped (Open question 21).
pub const LIVE_SOUNDS: usize = 255;

/// The live sounds' count.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LiveSounds(pub usize);

impl LiveSounds {
    /// Start one: false (refused) at the cap.
    pub fn start(&mut self) -> bool {
        if self.0 >= LIVE_SOUNDS {
            return false;
        }
        self.0 += 1;
        true
    }

    pub fn ended(&mut self) {
        self.0 = self.0.saturating_sub(1);
    }
}

/// The call-outs radius (u, 11).
pub const CALLOUT_RADIUS: f32 = 1000.0;

/// A death's call-outs (11, Open question 22): `EnemyDown` by the killer
/// when the teams differ (none for a team kill or a suicide), `ManDown` by
/// the victim's living teammate nearest the victim within 1000 u. People
/// are (entity, team, position in u).
pub fn callouts(
    killer: Option<(Entity, u8, Vec3)>,
    victim: (Entity, u8, Vec3),
    living: &[(Entity, u8, Vec3)],
) -> (Option<Entity>, Option<Entity>) {
    let enemy_down = killer.filter(|k| k.1 != victim.1 && k.0 != victim.0).map(|k| k.0);
    let man_down = living
        .iter()
        .filter(|p| p.1 == victim.1 && p.0 != victim.0)
        .map(|p| (p.0, p.2.distance(victim.2)))
        .filter(|(_, d)| *d <= CALLOUT_RADIUS)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|p| p.0);
    (enemy_down, man_down)
}

/// A map `SoundFX` object's play state (11, Open question 22): `StartOn`
/// plays it at creation; ON and TRIGGER (re)start it from the beginning (a
/// playing copy stopped first); OFF and KILL stop it; TOGGLE stops a
/// playing one or starts a stopped one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SoundFxState {
    pub playing: bool,
    /// Times it was started from the beginning.
    pub starts: u32,
}

impl SoundFxState {
    pub fn created(start_on: bool) -> Self {
        let mut s = Self::default();
        if start_on {
            s.start();
        }
        s
    }

    fn start(&mut self) {
        self.playing = true;
        self.starts += 1;
    }

    /// A message to the object (case ignored).
    pub fn message(&mut self, msg: &str) {
        match msg.to_ascii_uppercase().as_str() {
            "ON" | "TRIGGER" => self.start(),
            "OFF" | "KILL" => self.playing = false,
            "TOGGLE" => {
                if self.playing {
                    self.playing = false;
                } else {
                    self.start();
                }
            }
            _ => {}
        }
    }
}
