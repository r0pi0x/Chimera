//! Spent shells thrown from a weapon's ejection port (any game; CS:S:
//! specs/cs_source/view_models.md 8) as data and motion: a shell type's
//! game, model, bounce sound and how it moves (`MapShell`,
//! `MapShellPhysics`: each type carries its own game's, so on a map with
//! several games' guns each gun's casings fly by its game's rules) and
//! one step of a shell's flight (`shell_step`, `shell_alpha`,
//! `bounce_volume`, `MapShellPhysics::sounds`), with what HL2's casings
//! add to CS:S's rules (specs/hl2dm/effects_sounds.md 5: random speed on
//! the world's axes, roll spin, a random life, a sure bounce sound when
//! landing hard, no shooter's velocity). `render::shells` throws and
//! draws them.
//!
//! Spent shells for any game: `MapShell`/`MapShellPhysics` from the game
//! (per type, keyed by the game and the type); a flight's step, fade and
//! bounce volume.

use bevy::prelude::*;

use super::MapModel;

/// A shell type: its game, its model (engine axes, meters; +X along the
/// shell), the sound entry it makes when it bounces and how it flies (its
/// game's rules).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MapShell {
    /// The game whose guns eject it, the prefix of their view models' keys
    /// (`cs_source`): a brass event of a gun of that game picks it.
    pub game: String,
    /// The type's name as weapon events give it, lower case (`762nato`).
    pub key: String,
    pub model: MapModel,
    pub bounce: Option<String>,
    pub physics: MapShellPhysics,
}

impl MapShell {
    /// Whether this is game `game`'s type `key` (any case).
    pub fn is(&self, game: &str, key: &str) -> bool {
        self.game.eq_ignore_ascii_case(game) && self.key.eq_ignore_ascii_case(key)
    }
}

/// Adds `shells` (a game's types) to `into`, each that `into` lacks (a
/// guest's dressing: the map's own game's stay).
pub fn add_shells(into: &mut Vec<MapShell>, shells: Vec<MapShell>) {
    for s in shells {
        if !into.iter().any(|t| t.is(&s.game, &s.key)) {
            into.push(s);
        }
    }
}

/// How shells move (meters, seconds, radians). Games fill it.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapShellPhysics {
    /// Downward acceleration while flying.
    pub gravity: f32,
    /// Velocity kept per bounce.
    pub damping: f32,
    /// A hit on a surface whose normal's up part is above this, moving
    /// down slower than `rest_steps` frames of gravity, stops the shell.
    pub rest_normal: f32,
    pub rest_steps: f32,
    /// Angles kept per bounce (spin is not damped).
    pub angle_damping: f32,
    /// The event's speed (game units) is scaled to meters by `unit` and by
    /// a random factor in `speed_factor`; random speed up to `up_jitter`
    /// along the port's up and `right_jitter` along its right is added.
    pub unit: f32,
    pub speed_factor: (f32, f32),
    pub up_jitter: f32,
    pub right_jitter: f32,
    /// Pitch and yaw spin up to this, either way (rad/s).
    pub spin: f32,
    /// Seconds before fading, and the fade.
    pub life: f32,
    pub fade: f32,
    /// Chance of a bounce sound per hit, and the vertical impact speed at
    /// which it plays at full volume.
    pub sound_chance: f32,
    pub sound_full_speed: f32,
    /// A hit at this vertical speed or faster always sounds (HL2's
    /// casings: 200 units/s); None: `sound_chance` alone.
    #[serde(default)]
    pub sound_sure_speed: Option<f32>,
    /// Random speed added along the world's axes, between these (y up;
    /// HL2's (-64, 0, -64) to (64, 64, 64) units/s).
    #[serde(default)]
    pub world_jitter: (Vec3, Vec3),
    /// Roll spin up to this, either way (rad/s; HL2 spins every axis).
    #[serde(default)]
    pub roll_spin: f32,
    /// Up to this much longer life, at random (s; HL2's 1 + U(0, 1)).
    #[serde(default)]
    pub life_jitter: f32,
    /// The shooter's velocity is added (CS:S's; HL2's not).
    #[serde(default)]
    pub owner_velocity: bool,
}

impl MapShellPhysics {
    /// Whether a hit at vertical speed `vz` sounds, given a draw `chance`
    /// in [0, 1).
    pub fn sounds(&self, vz: f32, chance: f32) -> bool {
        self.sound_sure_speed.is_some_and(|s| vz.abs() >= s) || chance < self.sound_chance
    }
}

/// A flying (or resting) shell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShellState {
    pub position: Vec3,
    pub velocity: Vec3,
    pub rotation: Quat,
    /// Pitch, yaw and roll rates (rad/s).
    pub spin: Vec3,
    pub age: f32,
    pub resting: bool,
    /// Bounced at least once (then drawn in the world).
    pub bounced: bool,
}

/// What happened to a shell this step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShellHit {
    None,
    /// It bounced; the vertical speed at impact.
    Bounced(f32),
    /// It came to rest; the vertical speed at impact.
    Rested(f32),
}

/// One step of `dt` seconds: move, spin and fall, or react to `hit` (the
/// first surface between the old and new position: point and normal).
/// Spec view_models.md 8, "Each frame".
pub fn shell_step(s: &mut ShellState, dt: f32, hit: Option<(Vec3, Vec3)>, physics: &MapShellPhysics) -> ShellHit {
    s.age += dt;
    if s.resting {
        return ShellHit::None;
    }
    let spin = s.spin * dt;
    s.rotation = Quat::from_rotation_y(spin.y) * s.rotation * Quat::from_rotation_z(spin.x) * Quat::from_rotation_x(spin.z);
    let Some((point, normal)) = hit else {
        s.position += s.velocity * dt;
        s.velocity.y -= physics.gravity * dt;
        return ShellHit::None;
    };
    s.position = point;
    s.bounced = true;
    let vz = s.velocity.y;
    let step = physics.gravity * dt;
    if normal.y > physics.rest_normal && vz <= 0.0 && vz >= -physics.rest_steps * step {
        s.velocity = Vec3::ZERO;
        s.spin = Vec3::ZERO;
        s.resting = true;
        // Lie flat: keep only the heading.
        let forward = s.rotation * Vec3::X;
        s.rotation = Quat::from_rotation_y(forward.z.atan2(forward.x) * -1.0);
        return ShellHit::Rested(vz);
    }
    s.velocity = (s.velocity - 2.0 * s.velocity.dot(normal) * normal) * physics.damping;
    s.rotation = Quat::IDENTITY.slerp(s.rotation, physics.angle_damping);
    ShellHit::Bounced(vz)
}

/// A shell's opacity at `age` (1, then fading to 0 over `fade`); None
/// once it is gone.
pub fn shell_alpha(age: f32, physics: &MapShellPhysics) -> Option<f32> {
    if age < physics.life {
        return Some(1.0);
    }
    let a = 1.0 - (age - physics.life) / physics.fade.max(1e-6);
    (a > 0.0).then_some(a)
}

/// The volume factor of a bounce at vertical impact speed `vz`.
pub fn bounce_volume(vz: f32, physics: &MapShellPhysics) -> f32 {
    (vz.abs() / physics.sound_full_speed.max(1e-6)).min(1.0)
}
